//! Versioned, reference-derived admission for the complete Store user schema.
//!
//! The embedded migration is executed against an in-memory database to derive
//! the expected fingerprint. Candidate databases are inspected through the
//! same SQLite build. This keeps the migration as the single schema authority
//! while avoiding a second hand-maintained list of tables, indexes, triggers,
//! columns, or constraints.

use std::sync::OnceLock;

use academic_domain::ContentDigest;
use rusqlite::{Connection, OptionalExtension};

use crate::{
    STORE_SCHEMA_VERSION,
    error::{StoreError, StoreResult},
};

/// Version of the canonical fingerprint encoding below. Bump this only when
/// the encoding itself changes; physical schema changes remain versioned by
/// `STORE_SCHEMA_VERSION` and its ordered migration.
const STORE_SCHEMA_FINGERPRINT_VERSION: u32 = 1;

// SQLite reserves this literal prefix case-insensitively. Comparing the prefix
// directly keeps the underscore literal while admitting valid `sqliteX...`
// user object names.
//
// Only `CREATE` applies the reserved-prefix rejection; during schema load
// SQLite installs whatever `sqlite_schema` holds. A reserved-prefix trigger or
// view written directly into `sqlite_schema` therefore loads and fires, so the
// prefix exclusion is narrowed to the object kinds SQLite itself creates —
// `sqlite_sequence`, `sqlite_stat1`..`sqlite_stat4` and `sqlite_autoindex_*`,
// all of them tables or indexes. Every trigger and view is fingerprinted
// unconditionally.
const USER_SCHEMA_OBJECT_PREDICATE: &str = "(type NOT IN ('table', 'index') \
     OR substr(name, 1, length('sqlite_')) COLLATE NOCASE <> 'sqlite_')";

static EXPECTED_SCHEMA_FINGERPRINT: OnceLock<SchemaFingerprint> = OnceLock::new();

#[derive(Debug, Clone)]
struct SchemaFingerprint {
    canonical_bytes: Vec<u8>,
    digest: ContentDigest,
}

/// Verifies the complete user schema against a reference database produced by
/// the exact embedded migration.
pub(crate) fn verify_store_schema_fingerprint(
    connection: &Connection,
    migration_sql: &str,
) -> StoreResult<()> {
    let expected = expected_schema_fingerprint(migration_sql)?;
    let actual = schema_fingerprint(connection)?;
    if actual.canonical_bytes == expected.canonical_bytes {
        Ok(())
    } else {
        Err(StoreError::SchemaIdentityMismatch {
            component: "schema.structural_fingerprint.v1",
            expected: expected.digest.to_string(),
            actual: actual.digest.to_string(),
        })
    }
}

/// Counts every non-SQLite-created schema object. This is deliberately broader
/// than tables so a version-zero view, index, or trigger — including one whose
/// name carries SQLite's reserved prefix — cannot be migrated in place before
/// the exact current fingerprint rejects it.
pub(crate) fn user_schema_object_count(connection: &Connection) -> StoreResult<i64> {
    let query = format!("SELECT count(*) FROM sqlite_schema WHERE {USER_SCHEMA_OBJECT_PREDICATE}");
    connection
        .query_row(&query, [], |row| row.get(0))
        .map_err(StoreError::from)
}

fn expected_schema_fingerprint(migration_sql: &str) -> StoreResult<&'static SchemaFingerprint> {
    if let Some(fingerprint) = EXPECTED_SCHEMA_FINGERPRINT.get() {
        return Ok(fingerprint);
    }

    let reference = Connection::open_in_memory()?;
    reference.execute_batch(migration_sql)?;
    let computed = schema_fingerprint(&reference)?;

    // Concurrent first readers may derive the same immutable reference in
    // parallel. Whichever thread publishes first wins; all derivations are
    // byte-identical because they use the same migration and SQLite build.
    let _ = EXPECTED_SCHEMA_FINGERPRINT.set(computed);
    EXPECTED_SCHEMA_FINGERPRINT
        .get()
        .ok_or_else(|| StoreError::SchemaIdentityMismatch {
            component: "schema.structural_fingerprint.v1",
            expected: "initialized reference fingerprint".to_owned(),
            actual: "reference fingerprint initialization failed".to_owned(),
        })
}

fn schema_fingerprint(connection: &Connection) -> StoreResult<SchemaFingerprint> {
    let mut fingerprint = FingerprintEncoder::default();
    fingerprint.text("academic-store-user-schema");
    fingerprint.u32(STORE_SCHEMA_FINGERPRINT_VERSION);
    fingerprint.u32(STORE_SCHEMA_VERSION);

    // Database text encoding is fixed when the schema is first created and is
    // the only database-wide structural PRAGMA relevant to these definitions.
    // Volatile schema cookies and physical page-layout settings are excluded.
    let encoding = connection.query_row("PRAGMA encoding", [], |row| row.get::<_, String>(0))?;
    fingerprint.text(&encoding.to_ascii_lowercase());

    let object_query = format!(
        "SELECT type, name, tbl_name, sql FROM sqlite_schema \
         WHERE {USER_SCHEMA_OBJECT_PREDICATE} \
         ORDER BY type COLLATE BINARY, name COLLATE BINARY"
    );
    let mut statement = connection.prepare(&object_query)?;
    let objects = statement.query_map([], |row| {
        Ok(SchemaObject {
            object_type: row.get(0)?,
            name: row.get(1)?,
            table_name: row.get(2)?,
            sql: row.get(3)?,
        })
    })?;

    let mut object_count = 0_u64;
    for object in objects {
        let object = object?;
        object_count =
            object_count
                .checked_add(1)
                .ok_or_else(|| StoreError::SchemaIdentityMismatch {
                    component: "schema.object_count",
                    expected: "object count fitting unsigned 64-bit".to_owned(),
                    actual: "overflow".to_owned(),
                })?;
        fingerprint.marker("object");
        fingerprint.text(&object.object_type);
        fingerprint.text(&object.name);
        fingerprint.text(&object.table_name);
        fingerprint.optional_sql(object.sql.as_deref());

        if object.object_type == "table" {
            encode_table_structure(connection, &object.name, &mut fingerprint)?;
        }
    }
    fingerprint.marker("object-count");
    fingerprint.u64(object_count);

    let canonical_bytes = fingerprint.finish();
    let digest = ContentDigest::sha256(&canonical_bytes);
    Ok(SchemaFingerprint {
        canonical_bytes,
        digest,
    })
}

#[derive(Debug)]
struct SchemaObject {
    object_type: String,
    name: String,
    table_name: String,
    sql: Option<String>,
}

fn encode_table_structure(
    connection: &Connection,
    table: &str,
    fingerprint: &mut FingerprintEncoder,
) -> StoreResult<()> {
    fingerprint.marker("table-structure");
    fingerprint.text(table);

    let table_list = connection
        .query_row(
            concat!(
                "SELECT type, ncol, wr, strict FROM pragma_table_list ",
                "WHERE schema = 'main' AND name = ?1"
            ),
            [table],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;
    fingerprint.marker("pragma-table-list");
    match table_list {
        Some((table_type, column_count, without_rowid, strict)) => {
            fingerprint.bool(true);
            fingerprint.text(&table_type);
            fingerprint.i64(column_count);
            fingerprint.i64(without_rowid);
            fingerprint.i64(strict);
        }
        None => fingerprint.bool(false),
    }

    let mut columns = connection.prepare(concat!(
        "SELECT cid, name, type, \"notnull\", dflt_value, pk, hidden ",
        "FROM pragma_table_xinfo(?1) ORDER BY cid"
    ))?;
    let column_rows = columns.query_map([table], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;
    for column in column_rows {
        let (cid, name, declared_type, not_null, default_value, primary_key, hidden) = column?;
        fingerprint.marker("pragma-table-xinfo");
        fingerprint.i64(cid);
        fingerprint.text(&name);
        fingerprint.text(&declared_type.to_ascii_lowercase());
        fingerprint.i64(not_null);
        fingerprint.optional_sql(default_value.as_deref());
        fingerprint.i64(primary_key);
        fingerprint.i64(hidden);
    }

    let mut foreign_keys = connection.prepare(concat!(
        "SELECT id, seq, \"table\", \"from\", \"to\", on_update, on_delete, match ",
        "FROM pragma_foreign_key_list(?1) ORDER BY id, seq"
    ))?;
    let foreign_key_rows = foreign_keys.query_map([table], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
        ))
    })?;
    for foreign_key in foreign_key_rows {
        let (id, sequence, target, source_column, target_column, on_update, on_delete, match_name) =
            foreign_key?;
        fingerprint.marker("pragma-foreign-key-list");
        fingerprint.i64(id);
        fingerprint.i64(sequence);
        fingerprint.text(&target);
        fingerprint.text(&source_column);
        fingerprint.optional_text(target_column.as_deref());
        fingerprint.text(&on_update.to_ascii_lowercase());
        fingerprint.text(&on_delete.to_ascii_lowercase());
        fingerprint.text(&match_name.to_ascii_lowercase());
    }

    // Include implicit UNIQUE/PRIMARY KEY indexes as well as explicit user
    // indexes. The sqlite_schema definition captures expressions/predicates;
    // these PRAGMAs independently bind uniqueness, origin, key order,
    // collation, descending flags, and auxiliary rowid columns.
    let mut indexes = connection.prepare(concat!(
        "SELECT name, \"unique\", origin, partial FROM pragma_index_list(?1) ",
        "ORDER BY name COLLATE BINARY"
    ))?;
    let index_rows = indexes.query_map([table], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    for index in index_rows {
        let (index_name, unique, origin, partial) = index?;
        fingerprint.marker("pragma-index-list");
        fingerprint.text(&index_name);
        fingerprint.i64(unique);
        fingerprint.text(&origin.to_ascii_lowercase());
        fingerprint.i64(partial);

        let mut index_columns = connection.prepare(concat!(
            "SELECT seqno, cid, name, \"desc\", coll, key FROM pragma_index_xinfo(?1) ",
            "ORDER BY seqno"
        ))?;
        let index_column_rows = index_columns.query_map([index_name.as_str()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        for index_column in index_column_rows {
            let (sequence, cid, name, descending, collation, key) = index_column?;
            fingerprint.marker("pragma-index-xinfo");
            fingerprint.i64(sequence);
            fingerprint.i64(cid);
            fingerprint.optional_text(name.as_deref());
            fingerprint.i64(descending);
            fingerprint.optional_text(collation.as_deref().map(str::to_ascii_lowercase).as_deref());
            fingerprint.i64(key);
        }
    }

    Ok(())
}

#[derive(Debug, Default)]
struct FingerprintEncoder {
    bytes: Vec<u8>,
}

impl FingerprintEncoder {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn marker(&mut self, marker: &str) {
        self.bytes.push(b'm');
        self.raw_bytes(marker.as_bytes());
    }

    fn text(&mut self, value: &str) {
        self.bytes.push(b't');
        self.raw_bytes(value.as_bytes());
    }

    fn optional_text(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.bytes.push(1);
                self.text(value);
            }
            None => self.bytes.push(0),
        }
    }

    fn optional_sql(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.bytes.push(1);
                let canonical = canonical_sql(value);
                self.bytes.push(b's');
                self.raw_bytes(&canonical);
            }
            None => self.bytes.push(0),
        }
    }

    fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn raw_bytes(&mut self, value: &[u8]) {
        let length = value.len() as u64;
        self.u64(length);
        self.bytes.extend_from_slice(value);
    }
}

/// Canonicalizes SQLite DDL lexically: comments and whitespace are removed,
/// bare words/quoted identifiers are ASCII-case-folded, quote spellings are
/// normalized, and token boundaries remain length-delimited. String literal
/// contents and operators remain exact. Structural PRAGMAs provide the
/// independent semantic cross-check for columns, keys, and indexes.
fn canonical_sql(sql: &str) -> Vec<u8> {
    let input = sql.as_bytes();
    let mut output = Vec::with_capacity(input.len());
    let mut cursor = 0_usize;

    while cursor < input.len() {
        let byte = input[cursor];
        if byte.is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if byte == b'-' && input.get(cursor + 1) == Some(&b'-') {
            cursor += 2;
            while cursor < input.len() && !matches!(input[cursor], b'\n' | b'\r') {
                cursor += 1;
            }
            continue;
        }
        if byte == b'/' && input.get(cursor + 1) == Some(&b'*') {
            cursor += 2;
            while cursor + 1 < input.len() && !(input[cursor] == b'*' && input[cursor + 1] == b'/')
            {
                cursor += 1;
            }
            cursor = (cursor + 2).min(input.len());
            continue;
        }

        if (byte == b'x' || byte == b'X') && input.get(cursor + 1) == Some(&b'\'') {
            let (mut value, next) = quoted_token(input, cursor + 1, b'\'', b'\'');
            value.make_ascii_lowercase();
            push_sql_token(&mut output, b'b', &value);
            cursor = next;
            continue;
        }
        if byte == b'\'' {
            let (value, next) = quoted_token(input, cursor, b'\'', b'\'');
            push_sql_token(&mut output, b's', &value);
            cursor = next;
            continue;
        }
        if matches!(byte, b'"' | b'`' | b'[') {
            let closing = if byte == b'[' { b']' } else { byte };
            let (mut value, next) = quoted_token(input, cursor, byte, closing);
            value.make_ascii_lowercase();
            push_sql_token(&mut output, b'i', &value);
            cursor = next;
            continue;
        }
        if is_bare_word_byte(byte) {
            let start = cursor;
            cursor += 1;
            while cursor < input.len() && is_bare_word_byte(input[cursor]) {
                cursor += 1;
            }
            let mut value = input[start..cursor].to_vec();
            value.make_ascii_lowercase();
            let kind = if value.first().is_some_and(u8::is_ascii_digit) {
                b'n'
            } else {
                b'i'
            };
            push_sql_token(&mut output, kind, &value);
            continue;
        }

        let operator_length = if cursor + 2 <= input.len()
            && matches!(
                &input[cursor..cursor + 2],
                b"||" | b"->" | b"<<" | b">>" | b"<=" | b">=" | b"==" | b"!=" | b"<>"
            ) {
            if cursor + 3 <= input.len() && &input[cursor..cursor + 3] == b"->>" {
                3
            } else {
                2
            }
        } else {
            1
        };
        push_sql_token(&mut output, b'o', &input[cursor..cursor + operator_length]);
        cursor += operator_length;
    }

    output
}

fn quoted_token(input: &[u8], start: usize, opening: u8, closing: u8) -> (Vec<u8>, usize) {
    debug_assert_eq!(input[start], opening);
    let mut output = Vec::new();
    let mut cursor = start + 1;
    while cursor < input.len() {
        if input[cursor] == closing {
            if input.get(cursor + 1) == Some(&closing) {
                output.push(closing);
                cursor += 2;
                continue;
            }
            return (output, cursor + 1);
        }
        output.push(input[cursor]);
        cursor += 1;
    }
    // sqlite_schema should contain parseable SQL. Retaining an explicit marker
    // makes a writable_schema-corrupted unterminated token fail the fingerprint
    // deterministically rather than panicking.
    output.push(0xff);
    (output, cursor)
}

fn is_bare_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || !byte.is_ascii()
}

fn push_sql_token(output: &mut Vec<u8>, kind: u8, value: &[u8]) {
    output.push(kind);
    let length = value.len() as u64;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
}

#[cfg(test)]
mod tests {
    use super::canonical_sql;

    #[test]
    fn ddl_canonicalization_ignores_formatting_comments_and_identifier_case() {
        let left = "CREATE TABLE Example (value TEXT CHECK(value = 'Case')) STRICT";
        let right = concat!(
            "/* versioned schema */ create table \"EXAMPLE\"(\n",
            " VALUE text -- same column\n",
            " check ( VALUE='Case' ) ) strict"
        );
        assert_eq!(canonical_sql(left), canonical_sql(right));
    }

    #[test]
    fn ddl_canonicalization_preserves_semantic_literals_and_operators() {
        assert_ne!(
            canonical_sql("CREATE TABLE t(v INTEGER CHECK(v >= 1)) STRICT"),
            canonical_sql("CREATE TABLE t(v INTEGER CHECK(v > 1)) STRICT")
        );
        assert_ne!(
            canonical_sql("CREATE TRIGGER x AFTER INSERT ON t BEGIN SELECT 'A'; END"),
            canonical_sql("CREATE TRIGGER x AFTER INSERT ON t BEGIN SELECT 'a'; END")
        );
    }
}
