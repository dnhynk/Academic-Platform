//! The encrypted schema-2 profile and its SQLCipher boundary.
//!
//! This module exists only under the non-default `sqlcipher-store` feature,
//! which cannot be enabled together with `bundled-sqlite`. It creates and
//! opens the t068 section 3.2 profile: a raw 32-byte key is applied before the
//! first page access, the Phase 1 migration plus migration `0003` establish
//! the schema-2 identity, and the SQLCipher settings are read back and
//! asserted at every open.
//!
//! # What this module is not
//!
//! It does not accept ADR-002, does not verify an admission receipt, and does
//! not emit an admitted posture. Creating an encrypted profile is not
//! permission to ingest a real byte; that gate is `P2-K6`'s admission
//! verifier, and until it passes the daemon serves the synthetic posture.
//!
//! It also does not derive or store a key. `SKEY_p` comes from
//! `academic_crypto::VaultMasterKey::derive_store_key`, and the recipient
//! records that wrap the Vault Master Key are `P2-K1`'s and `P2-K4`'s.

use std::{
    fs,
    path::{Path, PathBuf},
};

use academic_crypto::StoreKey;
use rusqlite::{Connection, OpenFlags};

use crate::{
    INCOMPLETE_PROFILE_MARKER, PROFILE_FORMAT_V2_MARKER, STORE_DATABASE_FILE,
    accept::AcceptanceStore,
    connection::{ReaderConnection, open_keyed_reader},
    error::{StoreError, StoreResult},
    migration::{MigrationStatus, migrate_open_connection_pre_listen},
    path_policy::{
        PathProbe, ProfileRootState, validate_created_profile_path, validate_existing_profile_path,
        validate_new_profile_path,
    },
    platform,
    profile::{
        probe_failure, read_bounded_file, require_regular_file, sync_directory,
        sync_parent_directory, write_new_synced_file,
    },
};

/// Storage mode recorded in the schema-2 identity singleton.
///
/// The singleton records the FORMAT, and this is a physical fact about how the
/// bytes are stored. `data_policy`, `production_data_allowed`, and
/// `product_network` are deliberately not recorded: t068 section 3.1 emits all
/// three only when `AdmissionVerifier::verify()` succeeds, so they are
/// `P2-K6`'s runtime output rather than anything true of this file. A stored
/// `data_policy = "REAL_PERSONAL_DATA_PERMITTED"` would have the file claim
/// that real personal data is permitted while no receipt exists anywhere.
pub const ENCRYPTED_STORE_STORAGE_MODE: &str = "SQLCIPHER_ENCRYPTED_PROFILE_V2";
/// Storage encryption recorded in the schema-2 identity singleton.
pub const ENCRYPTED_STORE_STORAGE_ENCRYPTION: &str =
    "SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000";

/// Exact contents of the `PROFILE_FORMAT_V2` marker.
pub const PROFILE_FORMAT_V2_MARKER_CONTENTS: &str = concat!(
    "ACADEMIC_PLATFORM_ENCRYPTED_PROFILE_FORMAT_V2\n",
    "format_uuid=67cb6d3ea27e4b53b1e727d46920e4f9\n",
    "schema_version=2\n",
    "schema_semver=2.0.0\n",
    "storage_mode=SQLCIPHER_ENCRYPTED_PROFILE_V2\n",
    "storage_encryption=SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000\n",
);

const INCOMPLETE_PROFILE_MARKER_CONTENTS: &str =
    "ACADEMIC_PLATFORM_PHASE2_ENCRYPTED_PROFILE_BOOTSTRAP_INCOMPLETE\n";

/// Major SQLCipher version the frozen `storage_encryption` string names.
///
/// The library pins the *cryptography*, not the patch level: the algorithm,
/// KDF, page size, and iteration count below are the whole of what
/// `SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000` claims, and each is
/// asserted exactly. A patch-level pin would refuse an identical SQLCipher 4
/// build for no security reason; drift in the observed version string is
/// caught by the acceptance tests instead, which assert the exact build.
pub const REQUIRED_CIPHER_MAJOR_VERSION: &str = "4";
/// SQLCipher page size asserted at every open.
pub const REQUIRED_CIPHER_PAGE_SIZE: i64 = 4096;
/// SQLCipher PBKDF2 iteration count asserted at every open.
pub const REQUIRED_KDF_ITER: i64 = 256_000;
/// SQLCipher page HMAC algorithm asserted at every open.
pub const REQUIRED_CIPHER_HMAC_ALGORITHM: &str = "HMAC_SHA512";
/// SQLCipher key-derivation algorithm asserted at every open.
pub const REQUIRED_CIPHER_KDF_ALGORITHM: &str = "PBKDF2_HMAC_SHA512";

/// The `EN` rows of the Phase 2 fault matrix owned by `P2-K2`.
pub const PHASE2_ENCRYPTED_STORE_FAULT_IDS: &[&str] =
    &["EN01", "EN02", "EN03", "EN04", "EN05", "EN06"];

/// SQLCipher settings read back from a live keyed connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CipherSettings {
    /// `PRAGMA cipher_version`, the full build string.
    pub cipher_version: String,
    /// `SELECT sqlite_version()` of the SQLCipher build.
    pub sqlite_version: String,
    /// `PRAGMA cipher_page_size`.
    pub cipher_page_size: i64,
    /// `PRAGMA kdf_iter`.
    pub kdf_iter: i64,
    /// `PRAGMA cipher_hmac_algorithm`.
    pub cipher_hmac_algorithm: String,
    /// `PRAGMA cipher_kdf_algorithm`.
    pub cipher_kdf_algorithm: String,
}

/// A complete encrypted profile whose marker, cipher settings, and schema-2
/// identity were all verified during this open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedProfile {
    root: PathBuf,
    database_path: PathBuf,
    migration_status: MigrationStatus,
    cipher: CipherSettings,
}

impl EncryptedProfile {
    /// Returns the profile root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the SQLCipher database path.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Reports whether this opening applied the migration set or found it current.
    #[must_use]
    pub const fn migration_status(&self) -> MigrationStatus {
        self.migration_status
    }

    /// Returns the settings observed on the connection that admitted this open.
    #[must_use]
    pub const fn cipher_settings(&self) -> &CipherSettings {
        &self.cipher
    }

    /// Opens the sole owned acceptance service over the encrypted database.
    pub fn open_acceptance_store(&self, key: &StoreKey) -> StoreResult<AcceptanceStore> {
        AcceptanceStore::open(&self.root, &self.database_path, key)
    }

    /// Opens a filesystem-read-only, SQLite-query-only keyed reader.
    pub fn open_reader(&self, key: &StoreKey) -> StoreResult<ReaderConnection> {
        open_keyed_reader(&self.database_path, key)
    }
}

/// A root whose incomplete marker is durably written but whose database is not
/// yet migrated.
#[derive(Debug)]
pub struct IncompleteEncryptedProfile {
    root: PathBuf,
}

impl IncompleteEncryptedProfile {
    /// Returns the root that startup must refuse until completion.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Keys, migrates, and verifies the database, then removes the marker last.
    pub fn complete<P: PathProbe + ?Sized>(
        self,
        probe: &P,
        key: &StoreKey,
        creating_build_digest: [u8; 32],
    ) -> StoreResult<EncryptedProfile> {
        validate_existing_profile_path(&self.root, probe)?;
        verify_format_marker(&self.root)?;
        verify_complete_incomplete_marker(&self.root)?;
        let database_path = self.root.join(STORE_DATABASE_FILE);

        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut connection = Connection::open_with_flags(&database_path, flags)?;
        apply_store_key(&connection, key, &database_path)?;
        read_and_verify_cipher_settings(&connection, &database_path)?;
        let migration_status =
            migrate_open_connection_pre_listen(&mut connection, creating_build_digest)?;
        // Read them again after the migration wrote pages: the settings this
        // profile records must describe the database as it now exists on disk,
        // not the configuration a still-empty file reported.
        let cipher = read_and_verify_cipher_settings(&connection, &database_path)?;
        drop(connection);

        let incomplete_path = self.root.join(INCOMPLETE_PROFILE_MARKER);
        fs::remove_file(&incomplete_path).map_err(|source| {
            StoreError::io("remove incomplete profile marker", &incomplete_path, source)
        })?;
        sync_directory(&self.root)?;
        Ok(EncryptedProfile {
            root: self.root,
            database_path,
            migration_status,
            cipher,
        })
    }
}

/// Creates a secure root and writes both markers before any database work.
pub fn prepare_encrypted_profile<P: PathProbe + ?Sized>(
    root: &Path,
    probe: &P,
) -> StoreResult<IncompleteEncryptedProfile> {
    let validated = validate_new_profile_path(root, probe)?;
    if validated.root_state() == ProfileRootState::Missing {
        platform::create_profile_directory(root).map_err(probe_failure)?;
    }
    validate_created_profile_path(root, probe)?;

    let incomplete_path = root.join(INCOMPLETE_PROFILE_MARKER);
    write_new_synced_file(
        &incomplete_path,
        INCOMPLETE_PROFILE_MARKER_CONTENTS.as_bytes(),
    )?;
    sync_directory(root)?;

    let format_path = root.join(PROFILE_FORMAT_V2_MARKER);
    write_new_synced_file(&format_path, PROFILE_FORMAT_V2_MARKER_CONTENTS.as_bytes())?;
    sync_directory(root)?;
    Ok(IncompleteEncryptedProfile {
        root: root.to_path_buf(),
    })
}

/// Creates, keys, and migrates a new encrypted profile.
pub fn create_encrypted_profile<P: PathProbe + ?Sized>(
    root: &Path,
    probe: &P,
    key: &StoreKey,
    creating_build_digest: [u8; 32],
) -> StoreResult<EncryptedProfile> {
    prepare_encrypted_profile(root, probe)?.complete(probe, key, creating_build_digest)
}

/// Opens a complete encrypted profile, refusing an interrupted bootstrap first.
///
/// Every open re-reads and re-asserts the SQLCipher settings, so a profile that
/// was created under one cryptographic profile cannot be opened under another.
pub fn open_encrypted_profile<P: PathProbe + ?Sized>(
    root: &Path,
    probe: &P,
    key: &StoreKey,
) -> StoreResult<EncryptedProfile> {
    validate_existing_profile_path(root, probe)?;
    let incomplete_path = root.join(INCOMPLETE_PROFILE_MARKER);
    match fs::symlink_metadata(&incomplete_path) {
        Ok(_) => return Err(StoreError::IncompleteProfile(root.to_path_buf())),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(StoreError::io(
                "inspect incomplete profile marker",
                incomplete_path,
                source,
            ));
        }
    }
    verify_format_marker(root)?;
    let database_path = root.join(STORE_DATABASE_FILE);
    require_regular_file(&database_path)?;

    // One keyed open, not two: the writer applies the key, admits the schema,
    // and then answers for its own cipher settings. A separate read-back
    // connection would pay a second 256,000-iteration key derivation to learn
    // what this handle already knows.
    let writer = crate::connection::open_keyed_writer(&database_path, key)?;
    let cipher = writer.cipher_settings()?;
    verify_cipher_settings(&cipher)?;
    drop(writer);
    Ok(EncryptedProfile {
        root: root.to_path_buf(),
        database_path,
        migration_status: MigrationStatus::AlreadyCurrent,
        cipher,
    })
}

/// Removes only a provably incomplete encrypted profile.
///
/// This never performs recursive deletion. Any unknown entry or link makes
/// cleanup fail closed, so `KY08`-shaped interrupted bootstraps stay safe to
/// remove without a recursive delete reaching real data.
pub fn remove_incomplete_encrypted_profile<P: PathProbe + ?Sized>(
    root: &Path,
    probe: &P,
) -> StoreResult<()> {
    validate_existing_profile_path(root, probe)?;
    verify_removable_incomplete_marker(root)?;
    let mut entries =
        fs::read_dir(root).map_err(|source| StoreError::io("enumerate profile", root, source))?;
    for result in &mut entries {
        let entry = result.map_err(|source| StoreError::io("read profile entry", root, source))?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| StoreError::InvalidProfileState {
                path: entry.path(),
                reason: "incomplete profile contains a non-Unicode entry",
            })?;
        if !known_encrypted_profile_files().contains(&name) {
            return Err(StoreError::InvalidProfileState {
                path: entry.path(),
                reason: "incomplete profile contains an unrecognized entry",
            });
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| {
            StoreError::io("inspect incomplete profile entry", entry.path(), source)
        })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(StoreError::InvalidProfileState {
                path: entry.path(),
                reason: "incomplete profile entry is not a regular file",
            });
        }
    }
    for name in known_encrypted_profile_files() {
        let path = root.join(name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(StoreError::io(
                    "remove incomplete profile file",
                    path,
                    source,
                ));
            }
        }
    }
    sync_directory(root)?;
    fs::remove_dir(root)
        .map_err(|source| StoreError::io("remove empty incomplete profile", root, source))?;
    if let Some(parent) = root.parent() {
        sync_parent_directory(parent)?;
    }
    Ok(())
}

/// Renders the keying statement `pragma_update(None, "key", "x'<hex>'")` emits.
///
/// A PRAGMA value cannot be a bound parameter — SQLite parses it at prepare
/// time — so rusqlite renders the value into SQL text either way. Building the
/// text here instead means the buffer holding it belongs to this crate and can
/// be overwritten; `Sql`, the `String` rusqlite would have used, cannot be.
///
/// The bytes are the same ones `pragma_update` produces: `PRAGMA key=`, then
/// the value as a SQL string literal with each inner quote doubled. Its content
/// is SQLCipher's `x'...'` raw-key form, which is what makes SQLCipher take the
/// 32 raw bytes directly instead of running the passphrase KDF over the hex
/// text. `hex` is 64 lowercase hex characters rendered nibble by nibble by
/// `StoreKey::expose_raw_hex`, so it structurally cannot carry a quote of its
/// own.
fn key_statement(hex: &str) -> String {
    let mut rendered = String::with_capacity(hex.len() + KEY_STATEMENT_OVERHEAD);
    rendered.push_str("PRAGMA key='x''");
    rendered.push_str(hex);
    rendered.push_str("'''");
    rendered
}

/// Characters `key_statement` adds around the hex.
const KEY_STATEMENT_OVERHEAD: usize = 18;

/// Applies the raw store key as the first statement issued on a fresh handle.
///
/// SQLCipher requires the key before the first page is touched.
///
/// **The key hex reaches SQLite as SQL text, not as a bound parameter**, because
/// `PRAGMA` takes no parameter. What this function controls it clears: the hex
/// `expose_raw_hex` renders lives in a zeroizing buffer, and the statement built
/// around it is overwritten before it is freed. What it does not control is
/// SQLite's own copy — `sqlite3_prepare_v2` copies the statement text into the
/// prepared statement, and that copy is freed without being cleared when the
/// statement is finalized. No key byte reaches disk on either route; the lane's
/// byte-level canary scan of a keyed database reports zero plaintext hits.
///
/// Public because the encrypted portability lane opens its own read-only
/// snapshot handle after the guarded reader has already admitted the database,
/// exactly as the plaintext lane does. It grants no capability the caller did
/// not already have: the argument is a `StoreKey`, which only the `P2-K1`
/// schedule produces.
pub fn apply_store_key(
    connection: &Connection,
    key: &StoreKey,
    database_path: &Path,
) -> StoreResult<()> {
    let hex = key.expose_raw_hex();
    let statement = key_statement(hex.as_str());
    // Installing the key can itself touch page one, so a wrong key may surface
    // here; it may equally surface at the next statement. Both routes translate
    // through the same helper, so no caller has to interpret a raw SQLite code
    // whichever one fires.
    let outcome = connection.execute_batch(&statement);
    // Cleared before the allocation is returned, and before any `?` can leave
    // the function while it still holds the key text. `academic-store` carries
    // no `zeroize` dependency; this is the same hand-written clear the vault
    // uses for its key buffers.
    let mut spent = statement.into_bytes();
    spent.fill(0);
    outcome.map_err(|error| locked_if_undecryptable(StoreError::Sqlite(error), database_path))
}

/// Reads the SQLCipher settings from a keyed connection.
///
/// On an existing database these reads touch page one, so on a wrong key they
/// fail with `SQLITE_NOTADB` rather than returning wrong values. Callers that
/// need the fail-closed outcome go through `read_and_verify_cipher_settings`,
/// which translates that code; this entry point returns the raw error so a
/// harness can see exactly what SQLCipher said.
pub fn read_cipher_settings(connection: &Connection) -> StoreResult<CipherSettings> {
    Ok(CipherSettings {
        cipher_version: pragma_text(connection, "cipher_version")?,
        sqlite_version: connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?,
        cipher_page_size: pragma_decimal(connection, "cipher_page_size")?,
        kdf_iter: pragma_decimal(connection, "kdf_iter")?,
        cipher_hmac_algorithm: pragma_text(connection, "cipher_hmac_algorithm")?,
        cipher_kdf_algorithm: pragma_text(connection, "cipher_kdf_algorithm")?,
    })
}

/// Runs `PRAGMA cipher_integrity_check` and returns every reported problem.
///
/// SQLCipher names the exact page whose HMAC does not verify, which is what
/// makes an `EN03` outcome repair-required rather than merely "broken": the
/// report identifies what to repair. An empty result means every page
/// authenticated.
pub fn cipher_integrity_report(connection: &Connection) -> StoreResult<Vec<String>> {
    let mut statement = connection.prepare("PRAGMA cipher_integrity_check")?;
    let mut rows = statement.query([])?;
    let mut problems = Vec::new();
    while let Some(row) = rows.next()? {
        let value = row.get::<_, String>(0)?;
        if !value.eq_ignore_ascii_case("ok") {
            problems.push(value);
        }
    }
    Ok(problems)
}

/// Asserts every frozen SQLCipher setting named by `storage_encryption`.
pub fn verify_cipher_settings(settings: &CipherSettings) -> StoreResult<()> {
    let major = settings
        .cipher_version
        .split('.')
        .next()
        .unwrap_or_default()
        .trim();
    if major != REQUIRED_CIPHER_MAJOR_VERSION {
        return Err(StoreError::CipherSettingMismatch {
            setting: "cipher_version",
            expected: format!("major version {REQUIRED_CIPHER_MAJOR_VERSION}"),
            actual: settings.cipher_version.clone(),
        });
    }
    cipher_exact(
        "cipher_page_size",
        REQUIRED_CIPHER_PAGE_SIZE.to_string(),
        settings.cipher_page_size.to_string(),
    )?;
    cipher_exact(
        "kdf_iter",
        REQUIRED_KDF_ITER.to_string(),
        settings.kdf_iter.to_string(),
    )?;
    cipher_exact(
        "cipher_hmac_algorithm",
        REQUIRED_CIPHER_HMAC_ALGORITHM.to_owned(),
        settings.cipher_hmac_algorithm.clone(),
    )?;
    cipher_exact(
        "cipher_kdf_algorithm",
        REQUIRED_CIPHER_KDF_ALGORITHM.to_owned(),
        settings.cipher_kdf_algorithm.clone(),
    )
}

/// Reads back the settings and proves the key actually decrypts a page.
///
/// The page read is the wrong-key and corrupt-header boundary: SQLCipher only
/// reports a bad key when it fails to authenticate page one, so a successful
/// settings read alone proves nothing. The failure is translated into
/// [`StoreError::EncryptedStoreLocked`] so no caller has to interpret a raw
/// SQLite code, and so the message never distinguishes "wrong key" from
/// "corrupt page one" in a way that would oracle key material.
fn read_and_verify_cipher_settings(
    connection: &Connection,
    database_path: &Path,
) -> StoreResult<CipherSettings> {
    // Reading the settings already touches page one on an existing database, so
    // a SQLite-level failure here is the cipher refusing the key, not a broken
    // pragma. A `CipherSettingMismatch` is a different fact and passes through
    // unchanged so a drifted parameter is never reported as a wrong key.
    let settings = read_cipher_settings(connection)
        .map_err(|error| locked_if_undecryptable(error, database_path))?;
    verify_cipher_settings(&settings)?;
    connection
        .query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| locked_if_undecryptable(StoreError::Sqlite(error), database_path))?;
    Ok(settings)
}

/// Translates SQLite's "file is not a database" into the fail-closed outcome.
///
/// SQLCipher reports a key that cannot authenticate page one as `SQLITE_NOTADB`,
/// and it can raise it at any statement that first touches a page: installing
/// the key, reading a cipher setting, or admitting the schema. Every keyed entry
/// point routes its failure through here so no caller has to interpret a raw
/// SQLite code, and so an unrelated I/O failure is not relabelled as a wrong
/// key.
///
/// The reason is deliberately the same for a wrong key and for a destroyed
/// page one: distinguishing them would tell a caller something about the key.
pub(crate) fn locked_if_undecryptable(error: StoreError, database_path: &Path) -> StoreError {
    let undecryptable = matches!(
        &error,
        StoreError::Sqlite(rusqlite::Error::SqliteFailure(inner, _))
            if inner.code == rusqlite::ErrorCode::NotADatabase
    );
    if undecryptable {
        StoreError::EncryptedStoreLocked {
            path: database_path.to_path_buf(),
            reason: "page one did not authenticate under the supplied store key",
        }
    } else {
        error
    }
}

fn cipher_exact(setting: &'static str, expected: String, actual: String) -> StoreResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::CipherSettingMismatch {
            setting,
            expected,
            actual,
        })
    }
}

fn pragma_text(connection: &Connection, name: &str) -> StoreResult<String> {
    Ok(connection.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))?)
}

fn pragma_decimal(connection: &Connection, name: &str) -> StoreResult<i64> {
    let value = pragma_text(connection, name)?;
    value
        .trim()
        .parse::<i64>()
        .map_err(|_| StoreError::CipherSettingMismatch {
            setting: "cipher_pragma_decimal",
            expected: "a decimal integer".to_owned(),
            actual: value,
        })
}

fn verify_format_marker(root: &Path) -> StoreResult<()> {
    // Section 3.2: `PROFILE_FORMAT_V2` and the Phase 1 plaintext marker are
    // mutually exclusive, and startup refuses a profile carrying both. The
    // plaintext lane enforces the same rule from its own side.
    let synthetic = root.join(crate::SYNTHETIC_PROFILE_MARKER);
    if fs::symlink_metadata(&synthetic).is_ok() {
        return Err(StoreError::ConflictingProfileFormat(root.to_path_buf()));
    }
    let path = root.join(PROFILE_FORMAT_V2_MARKER);
    let contents = read_bounded_file(&path, PROFILE_FORMAT_V2_MARKER_CONTENTS.len() + 1)?;
    if contents == PROFILE_FORMAT_V2_MARKER_CONTENTS.as_bytes() {
        Ok(())
    } else {
        Err(StoreError::InvalidPolicyMarker(path))
    }
}

fn verify_complete_incomplete_marker(root: &Path) -> StoreResult<()> {
    let path = root.join(INCOMPLETE_PROFILE_MARKER);
    let contents = read_bounded_file(&path, INCOMPLETE_PROFILE_MARKER_CONTENTS.len() + 1)
        .map_err(|_| StoreError::IncompleteProfile(root.to_path_buf()))?;
    if contents == INCOMPLETE_PROFILE_MARKER_CONTENTS.as_bytes() {
        Ok(())
    } else {
        Err(StoreError::IncompleteProfile(root.to_path_buf()))
    }
}

fn verify_removable_incomplete_marker(root: &Path) -> StoreResult<()> {
    let path = root.join(INCOMPLETE_PROFILE_MARKER);
    let contents = read_bounded_file(&path, INCOMPLETE_PROFILE_MARKER_CONTENTS.len() + 1)
        .map_err(|_| StoreError::IncompleteProfile(root.to_path_buf()))?;
    if INCOMPLETE_PROFILE_MARKER_CONTENTS
        .as_bytes()
        .starts_with(&contents)
    {
        Ok(())
    } else {
        Err(StoreError::IncompleteProfile(root.to_path_buf()))
    }
}

fn known_encrypted_profile_files() -> [&'static str; 6] {
    [
        PROFILE_FORMAT_V2_MARKER,
        STORE_DATABASE_FILE,
        concat!("academic-platform.sqlite3", "-wal"),
        concat!("academic-platform.sqlite3", "-shm"),
        concat!("academic-platform.sqlite3", "-journal"),
        INCOMPLETE_PROFILE_MARKER,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{STORE_FORMAT_UUID, STORE_SCHEMA_SEMVER, STORE_SCHEMA_VERSION};

    #[test]
    fn frozen_schema_two_identity_matches_migration_0003() {
        let migration = crate::migration::MIGRATION_0003_SQL;
        assert!(migration.contains("format_uuid = x'67cb6d3ea27e4b53b1e727d46920e4f9'"));
        assert!(migration.contains("CHECK (schema_version = 2)"));
        assert!(migration.contains("CHECK (schema_semver = '2.0.0')"));
        assert!(migration.contains("CHECK (minimum_reader_protocol_major = 2)"));
        assert!(migration.contains("CHECK (minimum_writer_protocol_major = 2)"));
        assert!(migration.contains(&format!(
            "CHECK (storage_mode = '{ENCRYPTED_STORE_STORAGE_MODE}')"
        )));
        assert!(migration.contains(&format!(
            "storage_encryption = '{ENCRYPTED_STORE_STORAGE_ENCRYPTION}'"
        )));

        assert_eq!(STORE_SCHEMA_VERSION, 2);
        assert_eq!(STORE_SCHEMA_SEMVER, "2.0.0");
        let rendered: String = STORE_FORMAT_UUID
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(rendered, "67cb6d3ea27e4b53b1e727d46920e4f9");
        assert!(PROFILE_FORMAT_V2_MARKER_CONTENTS.contains(&format!("format_uuid={rendered}\n")));
    }

    #[test]
    fn the_key_statement_is_the_raw_key_form_with_inner_quotes_doubled() {
        let hex = "a".repeat(64);
        let rendered = key_statement(&hex);
        // Byte for byte what `pragma_update(None, "key", "x'<hex>'")` emits:
        // `PRAGMA key=` with no spaces, then the value as a SQL string literal.
        assert_eq!(rendered, format!("PRAGMA key='x''{hex}'''"));
        assert_eq!(rendered.len(), hex.len() + KEY_STATEMENT_OVERHEAD);
        // The literal's content — what SQLCipher sees after unescaping — is the
        // `x'...'` raw-key form, not a passphrase.
        assert!(rendered.starts_with("PRAGMA key='x''"));
        assert!(rendered.ends_with("'''"));
    }

    #[test]
    fn storage_encryption_string_names_every_asserted_setting() {
        assert!(ENCRYPTED_STORE_STORAGE_ENCRYPTION.contains("SQLCIPHER_4"));
        assert!(ENCRYPTED_STORE_STORAGE_ENCRYPTION.contains("HMAC_SHA512"));
        assert!(ENCRYPTED_STORE_STORAGE_ENCRYPTION.contains("PBKDF2"));
        assert!(ENCRYPTED_STORE_STORAGE_ENCRYPTION.ends_with("256000"));
        assert_eq!(REQUIRED_KDF_ITER, 256_000);
        assert_eq!(REQUIRED_CIPHER_HMAC_ALGORITHM, "HMAC_SHA512");
        assert_eq!(REQUIRED_CIPHER_KDF_ALGORITHM, "PBKDF2_HMAC_SHA512");
    }
}
