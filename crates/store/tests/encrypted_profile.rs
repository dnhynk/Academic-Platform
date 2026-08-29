//! Acceptance evidence for the encrypted schema-2 store lane (`P2-K2`).
//!
//! Two of the named tests belong to the *plaintext* lane, because what they
//! assert is that a default build contains no SQLCipher. They are compiled
//! under `not(sqlcipher-store)` and run in the ordinary workspace test pass.
//! Everything else runs under
//! `cargo test -p academic-store --no-default-features --features sqlcipher-store`.

use std::fs;

#[cfg(feature = "sqlcipher-store")]
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

// The probe binary is the harness: the parent-side helpers here and the
// child-side failpoints it runs as separate processes are the same source, so a
// fault child cannot drift away from what the parent asserts about it.
#[cfg(feature = "sqlcipher-store")]
#[path = "../src/bin/sqlcipher_store_probe.rs"]
mod probe;

#[cfg(feature = "sqlcipher-store")]
static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "sqlcipher-store")]
#[derive(Debug)]
struct TempRoot {
    path: PathBuf,
}

#[cfg(feature = "sqlcipher-store")]
impl TempRoot {
    fn new(label: &str) -> std::io::Result<Self> {
        let sequence = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = temporary_base()?.join(format!(
            "academic-store-k2-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    #[cfg(feature = "sqlcipher-store")]
    fn workdir(&self) -> PathBuf {
        self.path.join("work")
    }
}

#[cfg(feature = "sqlcipher-store")]
impl Drop for TempRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!("test cleanup failed for {}: {error}", self.path.display());
        }
    }
}

/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the native facade
/// refuses to follow a link component, so the tests address the real directory.
#[cfg(all(unix, feature = "sqlcipher-store"))]
fn temporary_base() -> std::io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

/// Windows must not canonicalize: that yields the Win32 verbatim device
/// spelling the facade rejects, trading one refused spelling for another.
#[cfg(all(windows, feature = "sqlcipher-store"))]
fn temporary_base() -> std::io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

// ---------------------------------------------------------------------------
// Plaintext-lane evidence: a default build links no SQLCipher.
// ---------------------------------------------------------------------------

/// t068 section 2.3-13: the default product graph contains no SQLCipher and no
/// OpenSSL, and the running default binary proves it rather than only the
/// manifest claiming it.
#[cfg(not(feature = "sqlcipher-store"))]
#[test]
fn default_binary_does_not_link_sqlcipher() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    // Executable half. `PRAGMA cipher_version` is SQLCipher's own identity
    // statement: a plain SQLite build has no such pragma and returns no row, so
    // this is the linked library answering, not a manifest.
    let connection = rusqlite::Connection::open_in_memory()?;
    let cipher_version = connection
        .query_row("PRAGMA cipher_version", [], |row| row.get::<_, String>(0))
        .ok();
    assert_eq!(
        cipher_version, None,
        "the default lane linked a SQLCipher build"
    );
    assert!(
        connection
            .query_row("PRAGMA cipher_page_size", [], |row| row.get::<_, String>(0))
            .is_err()
    );

    // Scanned half. The default lane's own test executable must carry none of
    // the encrypted lane's frozen identity strings.
    //
    // The needles are read out of the committed migration rather than written
    // here. A literal in this file would be pooled into the very binary being
    // scanned -- splitting it does not help, because adjacent string constants
    // can be laid out contiguously again -- and the assertion would then be
    // satisfied by the test instead of by the library.
    let executable = fs::read(std::env::current_exe()?)?;
    for needle in frozen_schema_two_strings()? {
        assert_eq!(
            occurrences(&executable, needle.as_bytes()),
            0,
            "default binary carries the encrypted-lane string {needle}"
        );
    }

    // Graph half, on the crate that becomes the shipped daemon.
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tree = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(&repository)
        .args([
            "tree",
            "--locked",
            "--offline",
            "--package",
            "academic-daemon",
            "--edges",
            "features",
        ])
        .output()?;
    assert!(
        tree.status.success(),
        "default feature tree failed: {}",
        String::from_utf8_lossy(&tree.stderr)
    );
    let normalized = String::from_utf8(tree.stdout)?.to_ascii_lowercase();
    for forbidden in [
        "bundled-sqlcipher",
        "openssl-src",
        "openssl-sys",
        "sqlcipher-store",
    ] {
        assert!(
            !normalized.contains(forbidden),
            "default feature tree selected {forbidden}"
        );
    }
    Ok(())
}

/// The plaintext half of section 2.3-1: a default build has exactly one
/// migration, speaks schema 1, and cannot express a schema-2 identity.
#[cfg(not(feature = "sqlcipher-store"))]
#[test]
fn phase1_profile_cannot_be_converted() -> Result<(), Box<dyn std::error::Error>> {
    use academic_store::migration::{MIGRATION_0001_SQL, STORE_MIGRATION_SQL};

    assert_eq!(academic_store::STORE_SCHEMA_VERSION, 1);
    assert_eq!(academic_store::STORE_SCHEMA_SEMVER, "1.0.0");
    assert_eq!(academic_store::STORE_MINIMUM_READER_PROTOCOL, (1, 0));
    assert_eq!(academic_store::STORE_MINIMUM_WRITER_PROTOCOL, (1, 0));
    assert_eq!(
        STORE_MIGRATION_SQL.len(),
        1,
        "the plaintext lane must not embed the Phase 2 identity migration"
    );
    assert_eq!(STORE_MIGRATION_SQL[0], MIGRATION_0001_SQL);

    // The Phase 1 singleton is CHECK-pinned to schema 1, so a schema-2 row is
    // not merely disallowed by convention: SQLite refuses to store it. The
    // values come out of the committed migration, so what is proved rejected is
    // the frozen schema-2 identity and not a copy of it that could drift.
    let connection = rusqlite::Connection::open_in_memory()?;
    connection.execute_batch(MIGRATION_0001_SQL)?;
    let rejected = connection.execute(
        "INSERT INTO schema_meta (\
             singleton, format_uuid, schema_version, schema_semver,\
             minimum_reader_protocol_major, minimum_reader_protocol_minor,\
             minimum_writer_protocol_major, minimum_writer_protocol_minor,\
             data_policy, storage_mode, storage_encryption,\
             production_data_allowed, product_network, creating_build_digest, created_at_unix_ms\
         ) VALUES (1, ?1, 2, '2.0.0', 2, 0, 2, 0, ?2, ?3, ?4, 0, 'NONE', ?5, 1)",
        rusqlite::params![
            hex_bytes(&frozen_check_literal("format_uuid = x'")?),
            // The schema-2 singleton has no `data_policy`, so there is no
            // schema-2 value to try here. The Phase 1 one is supplied to keep
            // this row otherwise valid, which makes the rejection below come
            // from the identity columns rather than from a NOT NULL.
            "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
            frozen_check_literal("storage_mode = '")?,
            frozen_check_literal("storage_encryption = '")?,
            vec![0_u8; 32],
        ],
    );
    let message =
        must_fail(rejected, "a Phase 1 singleton accepted a schema-2 identity")?.to_string();
    assert!(
        message.contains("CHECK constraint failed"),
        "unexpected rejection: {message}"
    );

    // The substitution is refused in both directions, and structurally rather
    // than only by value: the schema-2 singleton drops three columns the Phase 1
    // one requires, so a Phase 1 row cannot be written into it either.
    let migration_0003 = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../migrations/store/0003_phase2_encrypted_identity.sql"),
    )?;
    // Statements only. The header comment names all three columns to explain
    // why they are absent, and a scan of the whole file would read that
    // explanation as the thing it forbids.
    let statements = migration_0003
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for absent in ["data_policy", "production_data_allowed", "product_network"] {
        assert!(
            !statements.contains(absent),
            "migration 0003 still records {absent}; the schema-2 singleton must \
             describe the format and leave the posture to the admission verifier"
        );
    }

    // And no source file offers a conversion entry point.
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for path in rust_sources(&source_root)? {
        let text = fs::read_to_string(&path)?;
        for forbidden in [
            "fn upgrade_profile",
            "fn convert_profile",
            "fn migrate_schema_1_to_2",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} declares a profile conversion entry point",
                path.display()
            );
        }
    }
    Ok(())
}

/// The frozen schema-2 identity strings, read from the committed migration.
#[cfg(not(feature = "sqlcipher-store"))]
fn frozen_schema_two_strings() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(vec![
        frozen_check_literal("storage_mode = '")?,
        frozen_check_literal("storage_encryption = '")?,
    ])
}

/// Extracts the single-quoted value a migration `CHECK` pins after `prefix`.
#[cfg(not(feature = "sqlcipher-store"))]
fn frozen_check_literal(prefix: &str) -> Result<String, Box<dyn std::error::Error>> {
    let migration = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../migrations/store/0003_phase2_encrypted_identity.sql"),
    )?;
    let start = migration
        .find(prefix)
        .ok_or_else(|| format!("migration 0003 does not pin {prefix}"))?
        + prefix.len();
    let rest = &migration[start..];
    let end = rest
        .find('\'')
        .ok_or_else(|| format!("migration 0003 has an unterminated literal after {prefix}"))?;
    Ok(rest[..end].to_owned())
}

#[cfg(not(feature = "sqlcipher-store"))]
fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap_or_default())
        .collect()
}

#[cfg(not(feature = "sqlcipher-store"))]
use std::path::{Path, PathBuf};

#[cfg(not(feature = "sqlcipher-store"))]
fn rust_sources(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            sources.extend(rust_sources(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
    Ok(sources)
}

/// Returns the error of a result that was required to fail.
///
/// `clippy::expect_used` and `clippy::panic` are `deny` workspace-wide, so a
/// test states the expectation as a value and lets `?` report it.
fn must_fail<T, E>(
    result: Result<T, E>,
    expectation: &'static str,
) -> Result<E, Box<dyn std::error::Error>> {
    match result {
        Ok(_) => Err(std::io::Error::other(expectation).into()),
        Err(error) => Ok(error),
    }
}

fn occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

// ---------------------------------------------------------------------------
// Encrypted-lane evidence.
// ---------------------------------------------------------------------------

#[cfg(feature = "sqlcipher-store")]
mod encrypted {
    use super::{TempRoot, must_fail, occurrences, probe::enabled as harness};
    use std::{
        error::Error,
        fs,
        path::Path,
        process::{Command, Stdio},
    };

    use academic_store::{
        PROFILE_FORMAT_V2_MARKER, STORE_DATABASE_FILE, SYNTHETIC_PROFILE_MARKER,
        cipher::{
            self, ENCRYPTED_STORE_STORAGE_ENCRYPTION, ENCRYPTED_STORE_STORAGE_MODE,
            PROFILE_FORMAT_V2_MARKER_CONTENTS, REQUIRED_CIPHER_HMAC_ALGORITHM,
            REQUIRED_CIPHER_KDF_ALGORITHM, REQUIRED_CIPHER_PAGE_SIZE, REQUIRED_KDF_ITER,
            open_encrypted_profile,
        },
        error::StoreError,
        migration::{
            MIGRATION_0001_SQL, MIGRATION_0003_SQL, MIGRATION_0004_SQL, MIGRATION_0005_SQL,
            STORE_MIGRATION_SQL,
        },
        path_policy::{
            PathEvidence, PathProbe, PathProbeFailure, ProfileAccess, ProfileRootState,
            StorageLocality,
        },
    };
    use rusqlite::{Connection, OpenFlags};

    /// The exact SQLCipher build this evidence lane was produced against.
    ///
    /// This is a drift detector, not a security control: the library asserts the
    /// algorithms and parameters, and this asserts which build produced the
    /// receipt. A deliberate toolchain move updates it in the same commit that
    /// records the new evidence.
    const OBSERVED_CIPHER_VERSION: &str = "4.14.0 community";
    const OBSERVED_SQLITE_VERSION: &str = "3.51.3";

    fn probe_binary() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_BIN_EXE_sqlcipher_store_probe"))
    }

    #[test]
    fn encrypted_profile_v2_is_created_only_by_cipher_lane() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("create")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;

        // The marker names the format and the schema version, and it is the
        // encrypted one, not the plaintext one.
        let marker = fs::read_to_string(profile.root().join(PROFILE_FORMAT_V2_MARKER))?;
        assert_eq!(marker, PROFILE_FORMAT_V2_MARKER_CONTENTS);
        assert!(!profile.root().join(SYNTHETIC_PROFILE_MARKER).exists());

        // The identity singleton carries exactly the frozen schema-2 identity.
        let connection = harness::open_keyed(profile.database_path(), &key)?;
        let identity = academic_store::migration::read_schema_identity(&connection)?;
        assert_eq!(identity.schema_version, 2);
        assert_eq!(identity.schema_semver, "2.0.0");
        assert_eq!(identity.minimum_reader_protocol, (2, 0));
        assert_eq!(identity.minimum_writer_protocol, (2, 0));
        assert_eq!(identity.storage_mode, ENCRYPTED_STORE_STORAGE_MODE);
        assert_eq!(
            identity.storage_encryption,
            ENCRYPTED_STORE_STORAGE_ENCRYPTION
        );
        assert_eq!(
            identity.format_uuid,
            academic_store::STORE_FORMAT_UUID,
            "the encrypted lane wrote a different format UUID"
        );
        let user_version: i64 =
            connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(user_version, 2);
        // The singleton records the format and nothing about posture. A stored
        // `data_policy` would have the file claim that real personal data is
        // permitted while no admission receipt exists anywhere, so its absence
        // is asserted against the live schema rather than only against the
        // migration text.
        let columns = {
            let mut statement = connection.prepare("PRAGMA table_info(schema_meta)")?;
            let mut rows = statement.query([])?;
            let mut names = Vec::new();
            while let Some(row) = rows.next()? {
                names.push(row.get::<_, String>(1)?);
            }
            names
        };
        assert_eq!(
            columns,
            vec![
                "singleton",
                "format_uuid",
                "schema_version",
                "schema_semver",
                "minimum_reader_protocol_major",
                "minimum_reader_protocol_minor",
                "minimum_writer_protocol_major",
                "minimum_writer_protocol_minor",
                "storage_mode",
                "storage_encryption",
                "creating_build_digest",
                "created_at_unix_ms",
            ],
            "the schema-2 singleton is not exactly the format-fact column set"
        );

        let application_id: i64 =
            connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
        assert_eq!(
            application_id,
            i64::from(academic_store::SQLITE_APPLICATION_ID)
        );

        // The lane runs the Phase 1 migration and the aggregate migration as
        // well: the canonical tables and their append-only triggers are present
        // and still bite.
        assert_eq!(STORE_MIGRATION_SQL.len(), 4);
        assert_eq!(STORE_MIGRATION_SQL[0], MIGRATION_0001_SQL);
        assert_eq!(STORE_MIGRATION_SQL[1], MIGRATION_0003_SQL);
        assert_eq!(STORE_MIGRATION_SQL[2], MIGRATION_0004_SQL);
        assert_eq!(STORE_MIGRATION_SQL[3], MIGRATION_0005_SQL);
        let append_only = must_fail(
            connection.execute(
                "UPDATE schema_meta SET schema_semver = '2.0.1' WHERE singleton = 1",
                [],
            ),
            "the schema-2 singleton accepted an update",
        )?;
        assert!(
            append_only
                .to_string()
                .contains("canonical table is append-only"),
            "unexpected trigger message: {append_only}"
        );
        drop(connection);

        // Reopening re-verifies everything.
        let reopened = open_encrypted_profile(
            profile.root(),
            &academic_store::path_policy::NativePathProbe::default(),
            &key,
        )?;
        assert_eq!(reopened.database_path(), profile.database_path());
        Ok(())
    }

    #[test]
    fn phase1_profile_cannot_be_converted() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("noconvert")?;
        let legacy = root.path.join("phase1.sqlite3");

        // A schema-1 database, built by hand because this binary has no Phase 1
        // profile API to build one with. SQLCipher with no key is plain SQLite,
        // so these bytes are exactly what a Phase 1 profile holds.
        {
            let connection = Connection::open(&legacy)?;
            connection.execute_batch(MIGRATION_0001_SQL)?;
            connection.execute(
                "INSERT INTO schema_meta (\
                     singleton, format_uuid, schema_version, schema_semver,\
                     minimum_reader_protocol_major, minimum_reader_protocol_minor,\
                     minimum_writer_protocol_major, minimum_writer_protocol_minor,\
                     data_policy, storage_mode, storage_encryption,\
                     production_data_allowed, product_network, creating_build_digest,\
                     created_at_unix_ms\
                 ) VALUES (1, ?1, 1, '1.0.0', 1, 0, 1, 0, ?2, ?3, 'NONE', 0, 'NONE', ?4, 1)",
                rusqlite::params![
                    vec![
                        0x9e_u8, 0x4e, 0xb5, 0x3c, 0xcc, 0xb1, 0x4b, 0x2a, 0x8b, 0xe1, 0x3d, 0x32,
                        0xdb, 0x16, 0x6e, 0xe4
                    ],
                    "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
                    "PLAINTEXT_TEMPORARY_SQLITE",
                    vec![0x82_u8; 32],
                ],
            )?;
            connection.pragma_update(None, "application_id", 0x4143_4144_u32)?;
            connection.pragma_update(None, "user_version", 1_u32)?;
        }

        // The migration runner refuses it rather than upgrading it. This is the
        // whole of the no-conversion rule: the only entry point that could write
        // a schema-2 identity admits an empty database or an exactly-current
        // one, and a schema-1 database is neither.
        let mut opened = Connection::open(&legacy)?;
        let refused = must_fail(
            academic_store::migration::migrate_open_connection_pre_listen(&mut opened, [0x82; 32]),
            "the encrypted lane migrated a Phase 1 database",
        )?;
        let StoreError::UnsupportedMigrationState {
            application_id,
            user_version,
        } = refused
        else {
            return Err(format!("unexpected refusal: {refused}").into());
        };
        assert_eq!(
            application_id,
            i64::from(academic_store::SQLITE_APPLICATION_ID)
        );
        assert_eq!(user_version, 1);
        drop(opened);

        // The bytes are untouched: nothing was half-migrated on the way out.
        let after = Connection::open(&legacy)?;
        let still_one: i64 = after.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(still_one, 1);
        let semver: String = after.query_row(
            "SELECT schema_semver FROM schema_meta WHERE singleton = 1",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(semver, "1.0.0");
        drop(after);

        // A profile root carrying the Phase 1 marker is refused outright, and so
        // is one carrying both markers.
        let both = root.path.join("both");
        fs::create_dir(&both)?;
        fs::write(both.join(SYNTHETIC_PROFILE_MARKER), b"x")?;
        fs::write(
            both.join(PROFILE_FORMAT_V2_MARKER),
            PROFILE_FORMAT_V2_MARKER_CONTENTS,
        )?;
        fs::write(both.join(STORE_DATABASE_FILE), b"")?;
        let key = harness::provision(&root.workdir())?;
        // A probe that reports a safe location, so what this asserts is the
        // marker conflict rather than the directory mode of a scratch path.
        let conflict = open_encrypted_profile(
            &both,
            &FixedProbe {
                evidence: existing_local_evidence(&both),
            },
            &key,
        );
        let conflict = must_fail(conflict, "a profile carrying both markers was opened")?;
        assert!(
            matches!(conflict, StoreError::ConflictingProfileFormat(_)),
            "unexpected error: {conflict}"
        );
        Ok(())
    }

    #[test]
    fn cipher_settings_are_read_back_and_asserted() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("settings")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;

        // Creation read them back.
        let created = profile.cipher_settings().clone();
        assert_eq!(created.cipher_version, OBSERVED_CIPHER_VERSION);
        assert_eq!(created.sqlite_version, OBSERVED_SQLITE_VERSION);
        assert_eq!(created.cipher_page_size, REQUIRED_CIPHER_PAGE_SIZE);
        assert_eq!(created.kdf_iter, REQUIRED_KDF_ITER);
        assert_eq!(
            created.cipher_hmac_algorithm,
            REQUIRED_CIPHER_HMAC_ALGORITHM
        );
        assert_eq!(created.cipher_kdf_algorithm, REQUIRED_CIPHER_KDF_ALGORITHM);

        // And every subsequent open reads them back again, rather than trusting
        // what creation saw.
        for _ in 0..3 {
            let reopened = harness::open_profile(&workdir, &key)?;
            assert_eq!(*reopened.cipher_settings(), created);
        }

        // The assertion is a real gate: a settings record that disagrees with the
        // frozen parameter set is rejected.
        let mut weakened = created.clone();
        weakened.kdf_iter = 1_000;
        let rejected = must_fail(
            cipher::verify_cipher_settings(&weakened),
            "a weakened KDF was accepted",
        )?;
        assert!(
            matches!(
                rejected,
                StoreError::CipherSettingMismatch {
                    setting: "kdf_iter",
                    ..
                }
            ),
            "unexpected error: {rejected}"
        );
        let mut wrong_hmac = created.clone();
        wrong_hmac.cipher_hmac_algorithm = "HMAC_SHA1".to_owned();
        assert!(cipher::verify_cipher_settings(&wrong_hmac).is_err());
        let mut wrong_major = created;
        wrong_major.cipher_version = "3.4.2 community".to_owned();
        assert!(cipher::verify_cipher_settings(&wrong_major).is_err());
        Ok(())
    }

    /// `EN02`: a wrong store key leaves the profile locked and `schema_meta`
    /// unreadable. No weaker key is tried and no plaintext is produced.
    #[test]
    fn wrong_store_key_fails_closed() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("wrongkey")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;

        // A second, independent provisioning yields a different `SKEY_p`.
        let other = root.path.join("other");
        let wrong = harness::provision(&other)?;
        assert_ne!(
            key.expose_raw_hex().as_str(),
            wrong.expose_raw_hex().as_str()
        );

        let locked = open_encrypted_profile(
            profile.root(),
            &academic_store::path_policy::NativePathProbe::default(),
            &wrong,
        );
        let locked = must_fail(locked, "a wrong store key opened the profile")?;
        assert!(
            matches!(locked, StoreError::EncryptedStoreLocked { .. }),
            "unexpected error: {locked}"
        );
        assert!(
            !locked.to_string().contains(wrong.expose_raw_hex().as_str()),
            "the failure message leaked key material"
        );

        // `schema_meta` is unreadable under the wrong key, and readable under
        // the right one, so the refusal is the cipher and not a code path.
        assert!(!harness::page_one_authenticates(
            profile.database_path(),
            &wrong
        )?);
        assert!(harness::page_one_authenticates(
            profile.database_path(),
            &key
        )?);
        Ok(())
    }

    /// `EN03`: a corrupt cipher header is repair-required, and the page that
    /// failed is reported.
    #[test]
    fn corrupt_cipher_header_is_repair_required() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("corrupt")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;
        let database = profile.database_path().to_path_buf();
        {
            let connection = harness::open_keyed(&database, &key)?;
            connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        }

        // A healthy database reports nothing to repair.
        {
            let connection = harness::open_keyed(&database, &key)?;
            assert!(cipher::cipher_integrity_report(&connection)?.is_empty());
        }

        // Corrupt a page that is not page one, so the database still opens and
        // SQLCipher can name the exact page whose HMAC failed.
        let page = REQUIRED_CIPHER_PAGE_SIZE as usize;
        let mut bytes = fs::read(&database)?;
        assert!(
            bytes.len() > page * 3,
            "database is too small to corrupt page 3"
        );
        for byte in bytes.iter_mut().skip(page * 2).take(64) {
            *byte ^= 0xFF;
        }
        let corrupt = root.path.join("corrupt.sqlite3");
        fs::write(&corrupt, &bytes)?;

        let connection = harness::open_keyed(&corrupt, &key)?;
        let problems = cipher::cipher_integrity_report(&connection)?;
        assert!(!problems.is_empty(), "corruption was not reported");
        assert!(
            problems.iter().any(|problem| problem.contains("page 3")),
            "the report did not name the corrupted page: {problems:?}"
        );
        drop(connection);

        // Corrupting page one instead destroys the key check itself: the profile
        // stays locked and no plaintext is produced.
        let mut header = fs::read(&database)?;
        for byte in header.iter_mut().take(48) {
            *byte ^= 0xFF;
        }
        let broken_header = root.path.join("header.sqlite3");
        fs::write(&broken_header, &header)?;
        assert!(!harness::page_one_authenticates(&broken_header, &key)?);
        Ok(())
    }

    /// `EN05`: a cipher downgrade and a downgraded schema identity are both
    /// refused with an explicit version reason.
    #[test]
    fn cipher_downgrade_is_rejected() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("downgrade")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;
        let database = profile.database_path().to_path_buf();

        // A SQLCipher 3 compatibility handle cannot authenticate a SQLCipher 4
        // database: the KDF, HMAC, and page layout all differ.
        let downgraded = Connection::open_with_flags(
            &database,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let hex = key.expose_raw_hex();
        downgraded.pragma_update(None, "key", format!("x'{}'", hex.as_str()).as_str())?;
        downgraded.execute_batch("PRAGMA cipher_compatibility = 3;")?;
        assert!(
            downgraded
                .query_row("SELECT count(*) FROM sqlite_schema", [], |row| row
                    .get::<_, i64>(0))
                .is_err(),
            "a SQLCipher 3 compatibility handle read a SQLCipher 4 database"
        );
        drop(downgraded);

        // A database whose recorded schema version was pushed back to 1 — what an
        // older binary's identity would look like — is refused, and the error
        // names the component and both versions.
        let copy = root.path.join("older.sqlite3");
        fs::copy(&database, &copy)?;
        {
            let connection = harness::open_keyed(&copy, &key)?;
            connection.pragma_update(None, "user_version", 1_u32)?;
        }
        let refused = must_fail(
            academic_store::connection::open_keyed_reader(&copy, &key),
            "a downgraded schema identity was admitted",
        )?;
        let message = refused.to_string();
        assert!(
            message.contains("user_version") || message.contains("schema"),
            "the refusal gave no version reason: {message}"
        );
        Ok(())
    }

    /// The zero-canary scan across database, WAL, SHM, temp, backup, and crash
    /// artifacts, run through the same receipt the evidence lane emits.
    #[test]
    fn zero_canary_in_db_wal_shm_temp_backup_crash() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("canary")?;
        let workdir = root.workdir();
        let output = Command::new(probe_binary())
            .arg("run")
            .arg(&workdir)
            .output()?;
        assert!(
            output.status.success(),
            "receipt run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let receipt = String::from_utf8(output.stdout)?;
        assert!(receipt.contains("\"plaintext_canary_hits\":0"), "{receipt}");
        assert!(receipt.contains("\"adr_002_accepted\":false"), "{receipt}");
        assert!(
            receipt.contains("\"production_data_allowed\":false"),
            "{receipt}"
        );
        assert!(receipt.contains("\"schema_version\":2"), "{receipt}");

        // The scan is a real scan: the same corpus planted in a plaintext file
        // under the scanned root is found.
        let canaries = harness::load_canaries()?;
        let needles: Vec<Vec<u8>> = canaries
            .iter()
            .map(|canary| canary.as_bytes().to_vec())
            .collect();
        let artifacts = workdir.join(harness::ARTIFACT_DIRECTORY);
        let (files, bytes, hits) = harness::scan_for(&artifacts, &needles)?;
        assert!(files >= 4, "too few artifacts scanned: {files}");
        assert!(bytes > 0);
        assert_eq!(hits, 0, "plaintext canary found in an encrypted artifact");

        fs::write(
            artifacts.join("negative-control.txt"),
            canaries[0].as_bytes(),
        )?;
        let (_, _, control_hits) = harness::scan_for(&artifacts, &needles)?;
        assert_eq!(
            control_hits, 1,
            "the scanner failed to find a planted plaintext canary"
        );
        Ok(())
    }

    /// `EN06`: storage exhaustion during commit and during checkpoint aborts the
    /// transaction with an actionable error and leaves no partial state.
    #[test]
    fn disk_full_during_checkpoint_has_no_partial_commit() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("diskfull")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;
        let database = profile.database_path().to_path_buf();

        let before = harness::canonical_counts(&workdir, &key)?;

        // Bound the storage the database may occupy at exactly what it already
        // uses. Every further page allocation fails the way an exhausted volume
        // fails, at the same SQLite boundary and with the same `SQLITE_FULL`
        // result code, without needing a privileged filesystem.
        let connection = harness::open_keyed(&database, &key)?;
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA wal_autocheckpoint = 0;")?;
        let pages: i64 = connection.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        connection.pragma_update(None, "max_page_count", pages)?;

        connection.execute_batch("BEGIN IMMEDIATE;")?;
        let mut full = None;
        for round in 0..512_u16 {
            let ordinal = u8::try_from(round % 256).unwrap_or_default();
            let attempt = connection.execute(
                concat!(
                    "INSERT INTO command_receipt (",
                    "client_instance_id, idempotency_key, request_hash, expected_revision, ",
                    "committed_revision, response_bytes, response_hash, created_at",
                    ") VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7)"
                ),
                rusqlite::params![
                    [ordinal; 16].as_slice(),
                    unique_key(round, 0x11).as_slice(),
                    unique_key(round, 0x22).as_slice(),
                    i64::from(round) + 1,
                    vec![0xAB_u8; 4096],
                    unique_key(round, 0x33).as_slice(),
                    40_000 + i64::from(round),
                ],
            );
            if let Err(error) = attempt {
                full = Some(error);
                break;
            }
        }
        let error = full.ok_or("storage never became exhausted")?;
        assert!(
            error.to_string().contains("full"),
            "unexpected exhaustion error: {error}"
        );
        // Nothing of the aborted transaction survives. SQLite may have already
        // rolled the transaction back itself when it ran out of room, so an
        // explicit rollback that finds no active transaction is the same
        // outcome, not a different one.
        if let Err(rollback) = connection.execute_batch("ROLLBACK;") {
            assert!(
                rollback.to_string().contains("no transaction is active"),
                "unexpected rollback failure: {rollback}"
            );
        }
        drop(connection);

        let after = harness::canonical_counts(&workdir, &key)?;
        assert_eq!(before, after, "an exhausted commit left partial rows");

        // The profile still opens, its identity still verifies, and every page
        // still authenticates.
        let reopened = open_encrypted_profile(
            profile.root(),
            &academic_store::path_policy::NativePathProbe::default(),
            &key,
        )?;
        let verify = harness::open_keyed(reopened.database_path(), &key)?;
        assert!(cipher::cipher_integrity_report(&verify)?.is_empty());
        let integrity: String = verify.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        assert_eq!(integrity, "ok");
        Ok(())
    }

    /// A 32-byte value that is unique per round, so a bounded-storage insert
    /// loop never trips a UNIQUE constraint before it trips exhaustion.
    fn unique_key(round: u16, tag: u8) -> [u8; 32] {
        let mut bytes = [tag; 32];
        bytes[0..2].copy_from_slice(&round.to_be_bytes());
        bytes
    }

    /// A network share and a consumer sync folder are both refused for a
    /// schema-2 profile, at creation and at open.
    #[test]
    fn network_share_and_sync_folder_are_rejected_v2() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("paths")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;

        let remote = evidence(StorageLocality::Remote, false);
        let sync = evidence(StorageLocality::Local, true);
        for (label, probe) in [("remote", remote), ("sync", sync)] {
            let target = root.path.join(format!("{label}-profile"));
            let refused = cipher::create_encrypted_profile(
                &target,
                &FixedProbe {
                    evidence: probe.clone(),
                },
                &key,
                [0xE2; 32],
            );
            let refused = must_fail(refused, "an unsafe location accepted a schema-2 profile")?;
            assert!(
                matches!(refused, StoreError::UnsafeProfilePath(_)),
                "{label}: unexpected error {refused}"
            );
            assert!(
                !target.exists(),
                "{label}: a refused location still had a profile created"
            );
        }

        // The same refusal applies to opening an already-created profile whose
        // location later reports as remote.
        let profile = harness::create_profile(&workdir, &key)?;
        let refused = open_encrypted_profile(
            profile.root(),
            &FixedProbe {
                evidence: evidence(StorageLocality::Remote, false),
            },
            &key,
        );
        let refused = must_fail(refused, "a remote location opened a schema-2 profile")?;
        assert!(matches!(refused, StoreError::UnsafeProfilePath(_)));
        Ok(())
    }

    /// Evidence describing an existing, local, owner-only profile root.
    fn existing_local_evidence(root: &Path) -> PathEvidence {
        PathEvidence {
            canonical_existing_ancestor: root.to_path_buf(),
            root_state: ProfileRootState::NonEmptyDirectory,
            storage_locality: StorageLocality::Local,
            access: ProfileAccess::OwnerOnly,
            has_symlink_or_reparse_component: false,
            is_sync_folder: false,
            has_git_ancestor: false,
            final_identity_matches: true,
        }
    }

    fn evidence(locality: StorageLocality, is_sync_folder: bool) -> PathEvidence {
        PathEvidence {
            canonical_existing_ancestor: std::path::PathBuf::from("/"),
            root_state: ProfileRootState::Missing,
            storage_locality: locality,
            access: ProfileAccess::OwnerOnly,
            has_symlink_or_reparse_component: false,
            is_sync_folder,
            has_git_ancestor: false,
            final_identity_matches: true,
        }
    }

    #[derive(Debug, Clone)]
    struct FixedProbe {
        evidence: PathEvidence,
    }

    impl PathProbe for FixedProbe {
        fn inspect(&self, _requested_root: &Path) -> Result<PathEvidence, PathProbeFailure> {
            Ok(self.evidence.clone())
        }
    }

    /// t068 section 2.3-13, encrypted side: this binary links SQLCipher and
    /// contains none of the plaintext lane's profile machinery.
    #[test]
    fn cipher_binary_does_not_link_plaintext_lane() -> Result<(), Box<dyn Error>> {
        // The linked library is SQLCipher, and it answers for itself.
        let connection = Connection::open_in_memory()?;
        let version: String =
            connection.query_row("PRAGMA cipher_version", [], |row| row.get(0))?;
        assert_eq!(version, OBSERVED_CIPHER_VERSION);

        // The plaintext feature is off, and the compile-time guard that keeps it
        // off is present in the library source.
        const {
            assert!(!cfg!(feature = "bundled-sqlite"));
        }
        let lib = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))?;
        assert!(
            lib.contains(
                "#[cfg(all(feature = \"bundled-sqlite\", feature = \"sqlcipher-store\"))]"
            )
        );
        assert!(lib.contains("compile_error!"));

        // Scanned half, against the probe binary rather than this test binary so
        // the assertions below are not satisfied by this file's own source text.
        // The needles are the strings only the plaintext lane's *Rust* code
        // produces. The Phase 1 migration text is embedded in both lanes by
        // design — it is the shared canonical schema — so its `CHECK` spellings
        // are deliberately not used as needles.
        let binary = fs::read(probe_binary())?;
        for needle in [
            "PLAINTEXT SYNTHETIC-ONLY PROFILE".to_owned(),
            format!("storage_mode=PLAINTEXT_TEMPORARY{}", "_SQLITE"),
            format!(
                "ACADEMIC_PLATFORM_PHASE1_PROFILE_BOOTSTRAP{}",
                "_INCOMPLETE"
            ),
        ] {
            assert_eq!(
                occurrences(&binary, needle.as_bytes()),
                0,
                "the encrypted binary carries the plaintext-lane string {needle}"
            );
        }
        assert!(
            occurrences(&binary, ENCRYPTED_STORE_STORAGE_MODE.as_bytes()) > 0,
            "the encrypted binary does not carry its own storage mode"
        );

        // And the plaintext profile API is genuinely absent from this build.
        let profile_source =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/profile.rs"))?;
        for gated in [
            "pub fn create_synthetic_profile",
            "pub fn open_synthetic_profile",
            "pub fn write_policy_banner",
            "pub fn prepare_synthetic_profile",
            "pub fn validate_synthetic_manifest",
            "pub struct SyntheticProfile",
        ] {
            assert!(
                item_is_lane_gated(&profile_source, gated),
                "{gated} is not gated out of the encrypted lane"
            );
        }
        Ok(())
    }

    /// Reports whether the item declared by `declaration` carries the lane gate
    /// somewhere in its own attribute and doc block.
    fn item_is_lane_gated(source: &str, declaration: &str) -> bool {
        let Some(position) = source.find(declaration) else {
            return false;
        };
        for line in source[..position].lines().rev() {
            let trimmed = line.trim();
            if trimmed == "#[cfg(not(feature = \"sqlcipher-store\"))]" {
                return true;
            }
            if !trimmed.starts_with("///") && !trimmed.starts_with("#[") {
                return false;
            }
        }
        false
    }

    /// `EN01`: a process killed mid-rekey leaves exactly one key that opens the
    /// database, and the surviving key is documented.
    #[test]
    fn store_rekey_kill_leaves_exactly_one_working_key() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("rekey")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;
        let marker = root.path.join("rekey-started");

        let status = Command::new(probe_binary())
            .arg("child-rekey")
            .arg(&workdir)
            .arg(&marker)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        assert_eq!(
            status.code(),
            Some(harness::rekey_started_exit_code()),
            "the rekey child did not reach its failpoint"
        );
        assert!(marker.is_file(), "the rekey child never started a rekey");

        let original_opens = harness::page_one_authenticates(profile.database_path(), &key)?;
        let rekeyed = rekey_target_key();
        let rekeyed_opens = database_opens_with_hex(profile.database_path(), &rekeyed)?;
        assert!(
            original_opens ^ rekeyed_opens,
            "exactly one of the old and new keys must open the database \
             (old={original_opens}, new={rekeyed_opens})"
        );
        Ok(())
    }

    fn rekey_target_key() -> String {
        harness::REKEY_TARGET_HEX.to_owned()
    }

    fn database_opens_with_hex(database: &Path, hex: &str) -> Result<bool, Box<dyn Error>> {
        let connection = Connection::open_with_flags(
            database,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.pragma_update(None, "key", format!("x'{hex}'").as_str())?;
        Ok(connection
            .query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
                row.get::<_, i64>(0)
            })
            .is_ok())
    }

    /// `EN04`: a write-ahead log truncated mid-frame yields the complete old
    /// state or a locked profile, never a partial canonical row set.
    #[test]
    fn truncated_wal_frame_is_complete_old_state_or_locked() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("wal")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;
        let database = profile.database_path().to_path_buf();
        let before = harness::canonical_counts(&workdir, &key)?;

        let status = Command::new(probe_binary())
            .arg("child-wal-crash")
            .arg(&workdir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        assert_eq!(status.code(), Some(harness::wal_crash_exit_code()));

        let wal = wal_path(&database);
        let wal_bytes = fs::read(&wal)?;
        assert!(!wal_bytes.is_empty(), "the child left no write-ahead log");
        // Cut the log inside its last frame rather than at a frame boundary.
        let truncated = wal_bytes.len() - (wal_bytes.len() / 3).max(17);
        fs::write(&wal, &wal_bytes[..truncated])?;

        match harness::canonical_counts(&workdir, &key) {
            Ok(after) => {
                let complete = after.iter().all(|(_, count)| *count > 0);
                assert!(
                    after == before || complete,
                    "a truncated log produced a partial canonical row set: \
                     before={before:?} after={after:?}"
                );
            }
            Err(error) => {
                // A refusal is the other admitted outcome; what is forbidden is a
                // readable half-applied state.
                assert!(!error.to_string().is_empty());
            }
        }
        Ok(())
    }

    fn wal_path(database: &Path) -> std::path::PathBuf {
        let mut name = database.as_os_str().to_os_string();
        name.push("-wal");
        std::path::PathBuf::from(name)
    }

    /// An interrupted bootstrap is refused by startup and can be removed.
    ///
    /// The removal never recurses: an unrecognized entry makes it fail closed
    /// rather than deleting a directory whose contents it does not know.
    #[test]
    fn incomplete_encrypted_profile_is_refused_and_removable() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("incomplete")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let probe = academic_store::path_policy::NativePathProbe::default();
        let target = harness::profile_root(&workdir);

        let incomplete = cipher::prepare_encrypted_profile(&target, &probe)?;
        assert_eq!(incomplete.root(), target);
        assert!(target.join(PROFILE_FORMAT_V2_MARKER).is_file());

        let refused = must_fail(
            open_encrypted_profile(&target, &probe, &key),
            "startup admitted an interrupted bootstrap",
        )?;
        assert!(
            matches!(refused, StoreError::IncompleteProfile(_)),
            "unexpected error: {refused}"
        );

        // An entry the removal does not recognise stops it, so it can never
        // become a recursive delete of an unknown directory.
        fs::write(target.join("unexpected.txt"), b"x")?;
        let stopped = must_fail(
            cipher::remove_incomplete_encrypted_profile(&target, &probe),
            "cleanup removed a profile holding an unrecognized entry",
        )?;
        assert!(
            matches!(stopped, StoreError::InvalidProfileState { .. }),
            "unexpected error: {stopped}"
        );
        assert!(target.is_dir());

        fs::remove_file(target.join("unexpected.txt"))?;
        cipher::remove_incomplete_encrypted_profile(&target, &probe)?;
        assert!(!target.exists());
        Ok(())
    }

    /// The `EN` fault inventory this task owns, so a row cannot be dropped
    /// silently.
    #[test]
    fn encrypted_store_fault_inventory_is_exact() {
        assert_eq!(
            cipher::PHASE2_ENCRYPTED_STORE_FAULT_IDS,
            ["EN01", "EN02", "EN03", "EN04", "EN05", "EN06"]
        );
    }

    /// A profile created with `0004` in the chain closes and reopens.
    ///
    /// Admission derives its reference fingerprint from `STORE_MIGRATION_SQL`
    /// and compares by exact structural equality, so a migration applied to a
    /// profile but left out of that set makes the profile unopenable with
    /// `SchemaIdentityMismatch { component: "schema.structural_fingerprint.v1" }`.
    /// This is the round trip that binds the two together for `0004`.
    ///
    /// The lib-level 0004 suite builds its schema-2 base from the migration text.
    /// This builds it the way a profile does -- the real pre-listen runner over a
    /// real keyed connection -- so the order runs end to end rather than
    /// reconstructed.
    #[test]
    fn profile_carrying_0004_is_admitted_on_reopen() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("order")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;
        assert_eq!(
            profile.migration_status(),
            academic_store::migration::MigrationStatus::Applied
        );

        // Creation applied `0001`, `0003`, and `0004` in that order: the
        // identity is schema 2 and the aggregate tables are already there.
        let mut connection = harness::open_keyed(profile.database_path(), &key)?;
        let identity = academic_store::migration::read_schema_identity(&connection)?;
        assert_eq!(identity.schema_version, 2);
        for table in ["curriculum_version", "snapshot", "retention_action"] {
            assert!(
                !table_missing(&connection, table)?,
                "creation left {table} out of the profile"
            );
        }

        // `0004` is a delta on the canonical core, not on the profile format, so
        // the frozen schema-2 identity is exactly what `0003` wrote.
        let user_version: i64 =
            connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(user_version, 2, "0004 moved the physical schema version");
        assert_eq!(identity.schema_semver, "2.0.0");
        assert_eq!(identity.minimum_reader_protocol, (2, 0));
        assert_eq!(identity.minimum_writer_protocol, (2, 0));
        assert_eq!(identity.format_uuid, academic_store::STORE_FORMAT_UUID);

        // Forward-only: the aggregate migration refuses to run a second time on
        // a profile that already carries it, and every page still authenticates.
        let reapplied =
            academic_store::migration::apply_aggregate_migration_pre_listen(&mut connection);
        assert!(
            reapplied.is_err(),
            "migration 0004 re-applied itself on an encrypted profile"
        );
        assert!(cipher::cipher_integrity_report(&connection)?.is_empty());
        drop(connection);

        // The round trip. Admission is exact structural equality against the
        // reference `STORE_MIGRATION_SQL` produces, so a profile carrying the
        // aggregates being admitted here is the whole observation.
        let probe = academic_store::path_policy::NativePathProbe::default();
        let reopened = open_encrypted_profile(profile.root(), &probe, &key)?;
        assert_eq!(reopened.database_path(), profile.database_path());
        assert_eq!(
            reopened.migration_status(),
            academic_store::migration::MigrationStatus::AlreadyCurrent
        );
        let after = harness::open_keyed(reopened.database_path(), &key)?;
        assert_eq!(
            academic_store::migration::read_schema_identity(&after)?,
            identity,
            "reopening changed the schema-2 identity"
        );
        assert!(!table_missing(&after, "curriculum_version")?);

        // Admitting `0004` is not the same as admitting anything: the
        // fingerprint is still exact, so one object the migration set does not
        // create is refused by the same component that used to refuse `0004`.
        // Failing closed on a schema this binary was not admitted against is the
        // behaviour that must not change.
        after.execute_batch("CREATE TABLE unadmitted_probe (probe_id INTEGER) STRICT;")?;
        drop(after);
        drop(reopened);
        let refused = must_fail(
            open_encrypted_profile(profile.root(), &probe, &key),
            "a profile carrying an object outside the migration set was opened",
        )?;
        assert!(
            matches!(
                refused,
                StoreError::SchemaIdentityMismatch {
                    component: "schema.structural_fingerprint.v1",
                    ..
                }
            ),
            "unexpected refusal: {refused}"
        );
        Ok(())
    }

    fn table_missing(connection: &Connection, table: &str) -> Result<bool, Box<dyn Error>> {
        let count: i64 = connection.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        Ok(count == 0)
    }

    /// The one owned acceptance writer opens over the encrypted profile.
    ///
    /// The `DB01`-`DB07` replay below drives the canonical insert ordering
    /// directly, because a killed child cannot carry a `VerifiedBatch` across a
    /// process boundary. This covers the other half: that the guarded writer
    /// -- keyed connection, schema-2 admission, canonical authorizer -- comes up
    /// over an encrypted profile at all, and reports the schema-2 identity and
    /// the frozen Phase 1 connection policy.
    #[test]
    fn acceptance_store_opens_over_the_encrypted_profile() -> Result<(), Box<dyn Error>> {
        let root = TempRoot::new("acceptance")?;
        let workdir = root.workdir();
        let key = harness::provision(&workdir)?;
        let profile = harness::create_profile(&workdir, &key)?;

        let store = profile.open_acceptance_store(&key)?;
        assert_eq!(store.database_path(), profile.database_path());
        let pragmas = store.pragma_snapshot()?;
        assert_eq!(
            pragmas.application_id,
            i64::from(academic_store::SQLITE_APPLICATION_ID)
        );
        assert_eq!(pragmas.user_version, 2);
        assert_eq!(pragmas.journal_mode, "wal");
        assert_eq!(pragmas.synchronous, 2);
        assert!(pragmas.foreign_keys);
        assert!(!pragmas.trusted_schema);
        assert!(!pragmas.query_only);
        assert!(pragmas.recursive_triggers);

        // A wrong key does not reach the writer at all.
        let other = root.path.join("other-acceptance");
        let wrong = harness::provision(&other)?;
        let locked = must_fail(
            profile.open_acceptance_store(&wrong),
            "a wrong store key opened the acceptance writer",
        )?;
        assert!(
            matches!(locked, StoreError::EncryptedStoreLocked { .. }),
            "unexpected error: {locked}"
        );
        Ok(())
    }

    /// `DB01`-`DB07` replayed under the cipher lane: a process killed at each
    /// acceptance-transaction boundary never leaves a committed partial state.
    #[test]
    fn db_faults_replay_under_the_cipher_lane() -> Result<(), Box<dyn Error>> {
        for checkpoint in ["DB01", "DB02", "DB03", "DB04", "DB05", "DB06", "DB07"] {
            let root = TempRoot::new(&format!("db-{checkpoint}"))?;
            let workdir = root.workdir();
            let key = harness::provision(&workdir)?;
            harness::create_profile(&workdir, &key)?;
            let empty = harness::canonical_counts(&workdir, &key)?;

            let status = Command::new(probe_binary())
                .arg("child-db-fault")
                .arg(checkpoint)
                .arg(&workdir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            assert_eq!(
                status.code(),
                Some(harness::db_fault_exit_code(checkpoint)?),
                "{checkpoint}: the child did not reach its failpoint"
            );

            let after = harness::canonical_counts(&workdir, &key)?;
            if checkpoint == "DB07" {
                // The commit had already returned, so the complete set is durable.
                assert!(
                    after.iter().all(|(_, count)| *count > 0),
                    "DB07: a committed acceptance was lost: {after:?}"
                );
            } else {
                assert_eq!(
                    after, empty,
                    "{checkpoint}: an uncommitted acceptance became visible"
                );
            }

            // Whatever the outcome, the profile still opens and every page still
            // authenticates: encryption did not turn a kill into corruption.
            let reopened = harness::open_profile(&workdir, &key)?;
            let connection = harness::open_keyed(reopened.database_path(), &key)?;
            assert!(
                cipher::cipher_integrity_report(&connection)?.is_empty(),
                "{checkpoint}: the database no longer authenticates"
            );
        }
        Ok(())
    }
}
