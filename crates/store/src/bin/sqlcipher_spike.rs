//! Local-only SQLCipher evidence harness for the Phase 1 synthetic store.
//!
//! Cargo's explicit binary target requires the non-default `sqlcipher-spike`
//! feature, so ordinary product and workspace builds cannot compile this lane.

mod enabled {
    use std::{
        collections::HashSet,
        error::Error,
        ffi::OsString,
        fmt,
        fs::{self, File, OpenOptions},
        io::{BufReader, Read, Seek, SeekFrom, Write},
        path::{Path, PathBuf},
        process, thread,
        time::Duration,
    };

    use academic_store::migration::{MigrationStatus, migrate_open_connection_pre_listen};
    use rusqlite::{
        Connection, OpenFlags, OptionalExtension, TransactionBehavior, backup::Backup, params,
    };

    const PRIMARY_KEY: &str = "8f59224e3dbf47d997851cb82ea94fd570d03cbb4dd6a91326e02c832ca7a551";
    const WRONG_KEY: &str = "9347f0226e71cf1154b4bc5badad58e79f5a9e979229441b8bca2404818d13ab";
    const REKEY_KEY: &str = "51a3b4e62d7fc491fb82711960b3a9dd940a285fa075837a9fe43ce8f1c7b026";
    const BACKUP_KEY: &str = "ae100f05e51f83e88f739416ddd6493e2ab237a1518fb4ee407f66a6b35b03d0";
    const RESTORE_KEY: &str = "7d48fe89c54fd339f0a2495f3137fc510097d72b046639bc51645033c415bfe0";
    const CANARY_FILE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/sqlcipher-canary/canaries.txt"
    );
    const BUILD_DIGEST: [u8; 32] = [0xe1; 32];
    const REKEY_COMPLETE_EXIT: i32 = 87;
    const DB_FAULT_EXIT_BASE: i32 = 100;
    const DB_FAULT_IDS: [&str; 7] = ["DB01", "DB02", "DB03", "DB04", "DB05", "DB06", "DB07"];
    const DB_FAULT_RESPONSE: [u8; 4] = [0xdb, 0xe1, 0x00, 0x07];
    const CANARY_CREATED_AT_BASE: i64 = 10_000;
    const REKEY_PAYLOAD_BYTES: i64 = 64 * 1024 * 1024;

    /// Error type for evidence-contract failures rather than SQLite failures.
    #[derive(Debug)]
    struct HarnessError {
        message: String,
    }

    impl HarnessError {
        fn new(message: impl Into<String>) -> Self {
            Self {
                message: message.into(),
            }
        }
    }

    impl fmt::Display for HarnessError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&self.message)
        }
    }

    impl Error for HarnessError {}

    /// Result used by the standalone spike and its integration tests.
    pub type SpikeResult<T> = Result<T, Box<dyn Error>>;

    /// Read-back of the SQLCipher build and active cryptographic defaults.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CipherSettings {
        pub cipher_version: String,
        pub sqlite_version: String,
        pub cipher_page_size: i64,
        pub kdf_iter: i64,
        pub cipher_hmac_algorithm: String,
        pub cipher_kdf_algorithm: String,
    }

    /// Read-back of the operational PRAGMAs frozen by S1.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct S1Pragmas {
        pub application_id: i64,
        pub user_version: i64,
        pub journal_mode: String,
        pub synchronous: i64,
        pub foreign_keys: i64,
        pub trusted_schema: i64,
        pub busy_timeout: i64,
        pub temp_store: i64,
        pub recursive_triggers: i64,
        pub query_only: i64,
    }

    /// One detected plaintext occurrence in an evidence artifact.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CanaryFinding {
        pub artifact: PathBuf,
        pub canary_index: usize,
        pub byte_offset: u64,
    }

    /// Aggregate result of streaming every file in an artifact directory.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScanSummary {
        pub files_scanned: u64,
        pub bytes_scanned: u64,
        pub findings: Vec<CanaryFinding>,
    }

    /// Machine-readable facts produced by one complete local evidence run.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HarnessReceipt {
        pub cipher: CipherSettings,
        pub pragmas: S1Pragmas,
        pub canary_count: usize,
        pub restored_canary_count: usize,
        pub scan: ScanSummary,
        pub artifact_root: PathBuf,
    }

    impl HarnessReceipt {
        /// Serializes the bounded receipt without adding a runtime JSON dependency.
        #[must_use]
        pub fn to_json(&self) -> String {
            format!(
                concat!(
                    "{{\"evidence_lane\":\"sqlcipher-spike\",",
                    "\"adr_002_accepted\":false,",
                    "\"production_data_allowed\":false,",
                    "\"cipher_version\":\"{}\",",
                    "\"sqlite_version\":\"{}\",",
                    "\"cipher_page_size\":{},",
                    "\"kdf_iter\":{},",
                    "\"cipher_hmac_algorithm\":\"{}\",",
                    "\"cipher_kdf_algorithm\":\"{}\",",
                    "\"application_id\":{},",
                    "\"user_version\":{},",
                    "\"journal_mode\":\"{}\",",
                    "\"synchronous\":{},",
                    "\"foreign_keys\":{},",
                    "\"trusted_schema\":{},",
                    "\"busy_timeout\":{},",
                    "\"temp_store\":{},",
                    "\"canary_count\":{},",
                    "\"restored_canary_count\":{},",
                    "\"files_scanned\":{},",
                    "\"bytes_scanned\":{},",
                    "\"plaintext_canary_hits\":{},",
                    "\"artifact_root\":\"{}\"}}"
                ),
                json_escape(&self.cipher.cipher_version),
                json_escape(&self.cipher.sqlite_version),
                self.cipher.cipher_page_size,
                self.cipher.kdf_iter,
                json_escape(&self.cipher.cipher_hmac_algorithm),
                json_escape(&self.cipher.cipher_kdf_algorithm),
                self.pragmas.application_id,
                self.pragmas.user_version,
                json_escape(&self.pragmas.journal_mode),
                self.pragmas.synchronous,
                self.pragmas.foreign_keys,
                self.pragmas.trusted_schema,
                self.pragmas.busy_timeout,
                self.pragmas.temp_store,
                self.canary_count,
                self.restored_canary_count,
                self.scan.files_scanned,
                self.scan.bytes_scanned,
                self.scan.findings.len(),
                json_escape(&self.artifact_root.display().to_string()),
            )
        }
    }

    struct KeyedConnection {
        connection: Connection,
        path: PathBuf,
        key_label: &'static str,
    }

    impl fmt::Debug for KeyedConnection {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("KeyedConnection")
                .field("path", &self.path)
                .field("key_label", &self.key_label)
                .finish_non_exhaustive()
        }
    }

    impl KeyedConnection {
        fn create(path: &Path, key: &'static str, key_label: &'static str) -> SpikeResult<Self> {
            if path.exists() {
                return Err(HarnessError::new(format!(
                    "refusing to create over existing database {}",
                    path.display()
                ))
                .into());
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX;
            let connection = Connection::open_with_flags(path, flags)?;
            apply_key_before_page_access(&connection, key)?;
            Ok(Self {
                connection,
                path: path.to_path_buf(),
                key_label,
            })
        }

        fn open(path: &Path, key: &'static str, key_label: &'static str) -> SpikeResult<Self> {
            let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
            let connection = Connection::open_with_flags(path, flags)?;
            apply_key_before_page_access(&connection, key)?;
            Ok(Self {
                connection,
                path: path.to_path_buf(),
                key_label,
            })
        }

        fn open_read_only(
            path: &Path,
            key: &'static str,
            key_label: &'static str,
        ) -> SpikeResult<Self> {
            let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
            let connection = Connection::open_with_flags(path, flags)?;
            apply_key_before_page_access(&connection, key)?;
            Ok(Self {
                connection,
                path: path.to_path_buf(),
                key_label,
            })
        }
    }

    fn apply_key_before_page_access(connection: &Connection, key: &str) -> SpikeResult<()> {
        validate_key(key)?;
        // This must remain the first SQLite statement after opening a handle.  Using
        // pragma_update also quotes the key as data instead of interpolating SQL.
        connection.pragma_update(None, "key", key)?;
        Ok(())
    }

    fn validate_key(key: &str) -> SpikeResult<()> {
        if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(HarnessError::new(
                "synthetic SQLCipher key must be exactly 64 hex characters",
            )
            .into());
        }
        Ok(())
    }

    fn cipher_settings(connection: &Connection) -> SpikeResult<CipherSettings> {
        let settings = CipherSettings {
            cipher_version: pragma_text(connection, "cipher_version")?,
            sqlite_version: connection
                .query_row("SELECT sqlite_version()", [], |row| row.get(0))?,
            cipher_page_size: pragma_decimal(connection, "cipher_page_size")?,
            kdf_iter: pragma_decimal(connection, "kdf_iter")?,
            cipher_hmac_algorithm: pragma_text(connection, "cipher_hmac_algorithm")?,
            cipher_kdf_algorithm: pragma_text(connection, "cipher_kdf_algorithm")?,
        };
        if settings.cipher_version.trim().is_empty() {
            return Err(HarnessError::new("PRAGMA cipher_version returned an empty value").into());
        }
        if !settings.cipher_version.starts_with("4.14.0") {
            return Err(HarnessError::new(format!(
                "locked SQLCipher version drifted: {}",
                settings.cipher_version
            ))
            .into());
        }
        if settings.cipher_page_size != 4096
            || settings.kdf_iter != 256_000
            || settings.cipher_hmac_algorithm != "HMAC_SHA512"
            || settings.cipher_kdf_algorithm != "PBKDF2_HMAC_SHA512"
        {
            return Err(HarnessError::new(format!(
                "unexpected SQLCipher 4 settings: {settings:?}"
            ))
            .into());
        }
        Ok(settings)
    }

    fn read_s1_pragmas(connection: &Connection) -> SpikeResult<S1Pragmas> {
        Ok(S1Pragmas {
            application_id: pragma_i64(connection, "application_id")?,
            user_version: pragma_i64(connection, "user_version")?,
            journal_mode: pragma_text(connection, "journal_mode")?.to_ascii_lowercase(),
            synchronous: pragma_i64(connection, "synchronous")?,
            foreign_keys: pragma_i64(connection, "foreign_keys")?,
            trusted_schema: pragma_i64(connection, "trusted_schema")?,
            busy_timeout: pragma_i64(connection, "busy_timeout")?,
            temp_store: pragma_i64(connection, "temp_store")?,
            recursive_triggers: pragma_i64(connection, "recursive_triggers")?,
            query_only: pragma_i64(connection, "query_only")?,
        })
    }

    fn verify_cipher_integrity(connection: &Connection) -> SpikeResult<()> {
        let mut cipher_integrity = connection.prepare("PRAGMA cipher_integrity_check")?;
        let mut rows = cipher_integrity.query([])?;
        while let Some(row) = rows.next()? {
            let value = row.get::<_, String>(0)?;
            if !value.eq_ignore_ascii_case("ok") {
                return Err(
                    HarnessError::new(format!("cipher_integrity_check returned {value}")).into(),
                );
            }
        }
        Ok(())
    }

    fn pragma_i64(connection: &Connection, name: &str) -> SpikeResult<i64> {
        Ok(connection.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))?)
    }

    fn pragma_text(connection: &Connection, name: &str) -> SpikeResult<String> {
        Ok(connection.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))?)
    }

    fn pragma_decimal(connection: &Connection, name: &str) -> SpikeResult<i64> {
        let value = pragma_text(connection, name)?;
        Ok(value.parse::<i64>().map_err(|error| {
            HarnessError::new(format!(
                "PRAGMA {name} returned non-decimal {value:?}: {error}"
            ))
        })?)
    }

    /// Loads and validates the committed synthetic canary corpus.
    pub fn load_canaries() -> SpikeResult<Vec<String>> {
        let text = fs::read_to_string(CANARY_FILE)?;
        let mut canaries = Vec::new();
        let mut unique = HashSet::new();
        for line in text.lines() {
            let candidate = line.trim();
            if candidate.is_empty() || candidate.starts_with('#') {
                continue;
            }
            if candidate.len() < 72
                || !candidate.is_ascii()
                || !candidate
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            {
                return Err(HarnessError::new(format!(
                    "invalid high-entropy canary in {CANARY_FILE}"
                ))
                .into());
            }
            if !unique.insert(candidate.to_owned()) {
                return Err(HarnessError::new("duplicate SQLCipher canary").into());
            }
            canaries.push(candidate.to_owned());
        }
        if canaries.len() < 5 {
            return Err(HarnessError::new(
                "SQLCipher canary corpus must contain at least five values",
            )
            .into());
        }
        Ok(canaries)
    }

    fn insert_canaries(connection: &mut Connection, canaries: &[String]) -> SpikeResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (index, canary) in canaries.iter().enumerate() {
            let ordinal = u8::try_from(index + 1)?;
            let client_instance_id = [ordinal; 16];
            let idempotency_key = [ordinal.wrapping_add(0x20); 32];
            let request_hash = [ordinal.wrapping_add(0x40); 32];
            let response_hash = [ordinal.wrapping_add(0x60); 32];
            transaction.execute(
                concat!(
                    "INSERT INTO command_receipt (",
                    "client_instance_id, idempotency_key, request_hash, expected_revision, ",
                    "committed_revision, response_bytes, response_hash, created_at",
                    ") VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7)"
                ),
                params![
                    client_instance_id.as_slice(),
                    idempotency_key.as_slice(),
                    request_hash.as_slice(),
                    i64::try_from(index + 1)?,
                    canary.as_bytes(),
                    response_hash.as_slice(),
                    CANARY_CREATED_AT_BASE + i64::try_from(index)?,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn exercise_memory_temp_store(connection: &Connection, canaries: &[String]) -> SpikeResult<()> {
        connection.execute_batch(
            "CREATE TEMP TABLE sqlcipher_canary_temp(value TEXT NOT NULL) STRICT;",
        )?;
        for canary in canaries {
            connection.execute(
                "INSERT INTO sqlcipher_canary_temp(value) VALUES (?1)",
                [canary],
            )?;
        }
        let count = connection.query_row(
            "SELECT count(*) FROM sqlcipher_canary_temp ORDER BY value DESC",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if count != i64::try_from(canaries.len())? {
            return Err(HarnessError::new("temporary canary sort lost rows").into());
        }
        connection.execute_batch("DROP TABLE temp.sqlcipher_canary_temp;")?;
        Ok(())
    }

    fn read_canaries(connection: &Connection) -> SpikeResult<Vec<Vec<u8>>> {
        let mut statement = connection.prepare(concat!(
            "SELECT response_bytes FROM command_receipt ",
            "WHERE created_at >= ?1 AND created_at < ?2 ORDER BY created_at"
        ))?;
        let end = CANARY_CREATED_AT_BASE + 1_000;
        let values = statement
            .query_map(params![CANARY_CREATED_AT_BASE, end], |row| {
                row.get::<_, Vec<u8>>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(values)
    }

    fn verify_canaries(connection: &Connection, expected: &[String]) -> SpikeResult<usize> {
        let actual = read_canaries(connection)?;
        let expected_bytes = expected
            .iter()
            .map(|canary| canary.as_bytes().to_vec())
            .collect::<Vec<_>>();
        if actual != expected_bytes {
            return Err(HarnessError::new(format!(
                "canary rows differ: expected {}, got {}",
                expected.len(),
                actual.len()
            ))
            .into());
        }
        Ok(actual.len())
    }

    fn initialize_database(
        path: &Path,
        include_rekey_payload: bool,
    ) -> SpikeResult<(KeyedConnection, CipherSettings, S1Pragmas)> {
        let mut keyed = KeyedConnection::create(path, PRIMARY_KEY, "primary")?;
        let cipher = cipher_settings(&keyed.connection)?;
        let status = migrate_open_connection_pre_listen(&mut keyed.connection, BUILD_DIGEST)?;
        if status != MigrationStatus::Applied {
            return Err(
                HarnessError::new("new SQLCipher database was not migrated from zero").into(),
            );
        }
        let pragmas = read_s1_pragmas(&keyed.connection)?;
        let canaries = load_canaries()?;
        insert_canaries(&mut keyed.connection, &canaries)?;
        if include_rekey_payload {
            insert_rekey_payload(&keyed.connection)?;
        }
        verify_canaries(&keyed.connection, &canaries)?;
        verify_cipher_integrity(&keyed.connection)?;
        Ok((keyed, cipher, pragmas))
    }

    fn verify_shared_current(connection: &mut Connection) -> SpikeResult<()> {
        let status = migrate_open_connection_pre_listen(connection, BUILD_DIGEST)?;
        if status != MigrationStatus::AlreadyCurrent {
            return Err(
                HarnessError::new("existing SQLCipher database unexpectedly migrated").into(),
            );
        }
        verify_cipher_integrity(connection)
    }

    /// Creates one exact-S1 encrypted database containing the canary corpus.
    pub fn create_initialized_database(
        path: &Path,
        include_rekey_payload: bool,
    ) -> SpikeResult<(CipherSettings, S1Pragmas)> {
        let (keyed, cipher, pragmas) = initialize_database(path, include_rekey_payload)?;
        keyed
            .connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        drop(keyed);
        Ok((cipher, pragmas))
    }

    fn insert_rekey_payload(connection: &Connection) -> SpikeResult<()> {
        let client_instance_id = [0xf1_u8; 16];
        let idempotency_key = [0xf2_u8; 32];
        let request_hash = [0xf3_u8; 32];
        let response_hash = [0xf4_u8; 32];
        connection.execute(
            concat!(
                "INSERT INTO command_receipt (",
                "client_instance_id, idempotency_key, request_hash, expected_revision, ",
                "committed_revision, response_bytes, response_hash, created_at",
                ") VALUES (?1, ?2, ?3, NULL, 999, zeroblob(?4), ?5, 99_999)"
            ),
            params![
                client_instance_id.as_slice(),
                idempotency_key.as_slice(),
                request_hash.as_slice(),
                REKEY_PAYLOAD_BYTES,
                response_hash.as_slice(),
            ],
        )?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    fn canary_count_with_key(
        path: &Path,
        key: &'static str,
        key_label: &'static str,
    ) -> SpikeResult<usize> {
        let keyed = KeyedConnection::open_read_only(path, key, key_label)?;
        verify_cipher_integrity(&keyed.connection)?;
        verify_canaries(&keyed.connection, &load_canaries()?)
    }

    /// Opens with the original synthetic key and returns the exact canary count.
    pub fn canary_count_with_primary_key(path: &Path) -> SpikeResult<usize> {
        canary_count_with_key(path, PRIMARY_KEY, "primary")
    }

    /// Opens with the post-rekey synthetic key and returns the exact canary count.
    pub fn canary_count_with_rekey_key(path: &Path) -> SpikeResult<usize> {
        canary_count_with_key(path, REKEY_KEY, "rekey")
    }

    /// Returns true only when the deliberately wrong key cannot read the schema.
    pub fn wrong_key_is_rejected(path: &Path) -> SpikeResult<bool> {
        let keyed = KeyedConnection::open_read_only(path, WRONG_KEY, "wrong")?;
        let result = keyed
            .connection
            .query_row("SELECT count(*) FROM schema_meta", [], |row| {
                row.get::<_, i64>(0)
            });
        Ok(result.is_err())
    }

    /// Copies the database and corrupts its encrypted header bytes.
    pub fn corrupt_header_copy(source: &Path, destination: &Path) -> SpikeResult<()> {
        if destination.exists() {
            return Err(HarnessError::new("corrupt-header destination already exists").into());
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(destination)?;
        let mut header = [0_u8; 32];
        file.read_exact(&mut header)?;
        for byte in &mut header {
            *byte ^= 0xa5;
        }
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&header)?;
        file.sync_all()?;
        Ok(())
    }

    /// Returns true only when the correct key cannot read a corrupted header.
    pub fn corrupt_header_is_rejected(path: &Path) -> SpikeResult<bool> {
        let keyed = KeyedConnection::open_read_only(path, PRIMARY_KEY, "primary")?;
        let result = keyed
            .connection
            .query_row("SELECT count(*) FROM schema_meta", [], |row| {
                row.get::<_, i64>(0)
            });
        Ok(result.is_err())
    }

    fn encrypted_online_backup(
        source: &Connection,
        destination: &Path,
        destination_key: &'static str,
        key_label: &'static str,
    ) -> SpikeResult<()> {
        let mut destination_connection =
            KeyedConnection::create(destination, destination_key, key_label)?;
        let backup = Backup::new(source, &mut destination_connection.connection)?;
        backup.run_to_completion(32, Duration::from_millis(1), None)?;
        drop(backup);
        verify_shared_current(&mut destination_connection.connection)?;
        Ok(())
    }

    /// Creates an encrypted online backup and independently restores it into a new empty DB.
    pub fn online_backup_and_empty_restore(
        source_path: &Path,
        backup_path: &Path,
        restore_path: &Path,
    ) -> SpikeResult<usize> {
        if backup_path.exists() || restore_path.exists() {
            return Err(HarnessError::new(
                "backup and restore destinations must both be new and empty",
            )
            .into());
        }
        let mut source = KeyedConnection::open(source_path, PRIMARY_KEY, "primary")?;
        verify_shared_current(&mut source.connection)?;
        encrypted_online_backup(&source.connection, backup_path, BACKUP_KEY, "backup")?;
        drop(source);

        let backup_source = KeyedConnection::open_read_only(backup_path, BACKUP_KEY, "backup")?;
        let mut restore = KeyedConnection::create(restore_path, RESTORE_KEY, "restore")?;
        let restore_operation = Backup::new(&backup_source.connection, &mut restore.connection)?;
        restore_operation.run_to_completion(32, Duration::from_millis(1), None)?;
        drop(restore_operation);
        verify_shared_current(&mut restore.connection)?;
        let count = verify_canaries(&restore.connection, &load_canaries()?)?;
        drop(restore);
        drop(backup_source);

        if canary_count_with_key(backup_path, PRIMARY_KEY, "primary").is_ok() {
            return Err(HarnessError::new("backup unexpectedly opened with source key").into());
        }
        if canary_count_with_key(restore_path, BACKUP_KEY, "backup").is_ok() {
            return Err(HarnessError::new("restore unexpectedly opened with backup key").into());
        }
        Ok(count)
    }

    fn companion_path(database: &Path, suffix: &str) -> PathBuf {
        let mut path = OsString::from(database.as_os_str());
        path.push(suffix);
        PathBuf::from(path)
    }

    fn copy_database_artifacts(database: &Path, destination: &Path) -> SpikeResult<Vec<PathBuf>> {
        if destination.exists() {
            return Err(HarnessError::new(format!(
                "artifact destination already exists: {}",
                destination.display()
            ))
            .into());
        }
        fs::create_dir_all(destination)?;
        let mut copied = Vec::new();
        for (source, name) in [
            (database.to_path_buf(), "database.sqlite3"),
            (companion_path(database, "-wal"), "database.sqlite3-wal"),
            (companion_path(database, "-shm"), "database.sqlite3-shm"),
            (
                companion_path(database, "-journal"),
                "database.sqlite3-journal",
            ),
        ] {
            if source.exists() {
                let target = destination.join(name);
                fs::copy(&source, &target)?;
                copied.push(target);
            }
        }
        if copied.is_empty() {
            return Err(HarnessError::new("no database artifacts were copied").into());
        }
        Ok(copied)
    }

    /// Copies DB/WAL/SHM/journal files as controlled crash evidence.
    pub fn capture_crash_artifacts(
        database: &Path,
        destination: &Path,
    ) -> SpikeResult<Vec<PathBuf>> {
        copy_database_artifacts(database, destination)
    }

    /// Creates a copied database set with the WAL deliberately cut mid-frame.
    pub fn make_truncated_wal_snapshot(
        database: &Path,
        destination: &Path,
    ) -> SpikeResult<PathBuf> {
        let copied = copy_database_artifacts(database, destination)?;
        let wal = destination.join("database.sqlite3-wal");
        if !copied.iter().any(|path| path == &wal) {
            return Err(HarnessError::new("WAL artifact was absent before truncation").into());
        }
        let length = fs::metadata(&wal)?.len();
        if length < 128 {
            return Err(HarnessError::new(format!(
                "WAL artifact was too small to truncate: {length} bytes"
            ))
            .into());
        }
        OpenOptions::new()
            .write(true)
            .open(&wal)?
            .set_len(length - 17)?;
        Ok(destination.join("database.sqlite3"))
    }

    /// Classifies a truncated-WAL snapshot as exact recovery, atomic old state, or fail-closed.
    pub fn truncated_wal_outcome(path: &Path) -> SpikeResult<String> {
        let mut keyed = match KeyedConnection::open(path, PRIMARY_KEY, "primary") {
            Ok(value) => value,
            Err(_) => return Ok("FAIL_CLOSED_ON_OPEN".to_owned()),
        };
        if verify_shared_current(&mut keyed.connection).is_err()
            || verify_canaries(&keyed.connection, &load_canaries()?).is_err()
        {
            return Ok("FAIL_CLOSED_ON_READ".to_owned());
        }
        match classify_db_fault_state(read_db_fault_state(&keyed.connection)?)? {
            DbFaultDisposition::Committed => Ok("RECOVERED_COMPLETE".to_owned()),
            DbFaultDisposition::RolledBack => Ok("ATOMIC_PREVIOUS_STATE".to_owned()),
        }
    }

    /// Streams an artifact tree and reports every raw canary byte occurrence.
    pub fn scan_artifacts(root: &Path, canaries: &[String]) -> SpikeResult<ScanSummary> {
        let mut files = Vec::new();
        collect_files(root, &mut files)?;
        files.sort();
        let mut summary = ScanSummary {
            files_scanned: 0,
            bytes_scanned: 0,
            findings: Vec::new(),
        };
        for path in files {
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(HarnessError::new(format!(
                    "artifact scan refuses symlink {}",
                    path.display()
                ))
                .into());
            }
            summary.files_scanned += 1;
            summary.bytes_scanned += metadata.len();
            scan_file(&path, canaries, &mut summary.findings)?;
        }
        Ok(summary)
    }

    fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> SpikeResult<()> {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                collect_files(&path, files)?;
            } else if file_type.is_file() || file_type.is_symlink() {
                files.push(path);
            }
        }
        Ok(())
    }

    fn scan_file(
        path: &Path,
        canaries: &[String],
        findings: &mut Vec<CanaryFinding>,
    ) -> SpikeResult<()> {
        let max_canary = canaries.iter().map(String::len).max().unwrap_or(0);
        let mut reader = BufReader::new(File::open(path)?);
        let mut chunk = vec![0_u8; 64 * 1024];
        let mut carry = Vec::new();
        let mut consumed = 0_u64;
        loop {
            let count = reader.read(&mut chunk)?;
            if count == 0 {
                break;
            }
            let carry_length = carry.len();
            carry.extend_from_slice(&chunk[..count]);
            let base = consumed.saturating_sub(u64::try_from(carry_length)?);
            for (canary_index, canary) in canaries.iter().enumerate() {
                for offset in find_all(&carry, canary.as_bytes()) {
                    findings.push(CanaryFinding {
                        artifact: path.to_path_buf(),
                        canary_index,
                        byte_offset: base + u64::try_from(offset)?,
                    });
                }
            }
            consumed += u64::try_from(count)?;
            if max_canary > 1 && carry.len() >= max_canary - 1 {
                let keep_from = carry.len() - (max_canary - 1);
                carry.drain(..keep_from);
            } else if max_canary <= 1 {
                carry.clear();
            }
        }
        Ok(())
    }

    fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
        if needle.is_empty() || needle.len() > haystack.len() {
            return Vec::new();
        }
        haystack
            .windows(needle.len())
            .enumerate()
            .filter_map(|(offset, window)| (window == needle).then_some(offset))
            .collect()
    }

    /// Executes migration, WAL/temp, backup/restore, crash-copy, and leakage evidence locally.
    pub fn run_full_harness(root: &Path) -> SpikeResult<HarnessReceipt> {
        if root.exists() {
            return Err(HarnessError::new(format!(
                "evidence root must not already exist: {}",
                root.display()
            ))
            .into());
        }
        fs::create_dir_all(root.join("database"))?;
        fs::create_dir_all(root.join("temp"))?;
        fs::create_dir_all(root.join("backup"))?;
        fs::create_dir_all(root.join("restore"))?;
        let database = root.join("database").join("academic.sqlite3");
        let backup = root.join("backup").join("academic-backup.sqlite3");
        let restore = root.join("restore").join("academic-restore.sqlite3");
        let canaries = load_canaries()?;

        let (keyed, cipher, pragmas) = initialize_database(&database, false)?;
        keyed
            .connection
            .execute_batch("PRAGMA wal_autocheckpoint = 0;")?;
        exercise_memory_temp_store(&keyed.connection, &canaries)?;
        keyed.connection.execute(
            "UPDATE replica_state SET profile_revision = profile_revision + 1",
            [],
        )?;
        capture_crash_artifacts(&database, &root.join("crash-artifacts"))?;
        encrypted_online_backup(&keyed.connection, &backup, BACKUP_KEY, "backup")?;
        let canary_count = verify_canaries(&keyed.connection, &canaries)?;
        let scan_while_wal_open = scan_artifacts(root, &canaries)?;
        if !scan_while_wal_open.findings.is_empty() {
            return Err(HarnessError::new(format!(
                "plaintext canary found while WAL/SHM were live: {:?}",
                scan_while_wal_open.findings
            ))
            .into());
        }
        drop(keyed);

        let backup_source = KeyedConnection::open_read_only(&backup, BACKUP_KEY, "backup")?;
        let mut restored = KeyedConnection::create(&restore, RESTORE_KEY, "restore")?;
        let operation = Backup::new(&backup_source.connection, &mut restored.connection)?;
        operation.run_to_completion(32, Duration::from_millis(1), None)?;
        drop(operation);
        verify_shared_current(&mut restored.connection)?;
        let restored_canary_count = verify_canaries(&restored.connection, &canaries)?;
        drop(restored);
        drop(backup_source);

        let scan = scan_artifacts(root, &canaries)?;
        if !scan.findings.is_empty() {
            return Err(HarnessError::new(format!(
                "plaintext canary found in final artifact set: {:?}",
                scan.findings
            ))
            .into());
        }
        Ok(HarnessReceipt {
            cipher,
            pragmas,
            canary_count,
            restored_canary_count,
            scan,
            artifact_root: root.to_path_buf(),
        })
    }

    /// Returns the sole recovery key label after an interrupted rekey.
    pub fn documented_recovery_key(path: &Path) -> SpikeResult<String> {
        let primary = canary_count_with_primary_key(path).is_ok();
        let rekey = canary_count_with_rekey_key(path).is_ok();
        match (primary, rekey) {
            (true, false) => Ok("PRIMARY_PRE_REKEY_KEY".to_owned()),
            (false, true) => Ok("NEW_POST_REKEY_KEY".to_owned()),
            (true, true) => {
                Err(HarnessError::new("both rekey recovery keys opened the database").into())
            }
            (false, false) => {
                Err(HarnessError::new("neither rekey recovery key opened the database").into())
            }
        }
    }

    fn db_fault_index(checkpoint: &str) -> SpikeResult<usize> {
        DB_FAULT_IDS
            .iter()
            .position(|candidate| candidate == &checkpoint)
            .ok_or_else(|| {
                HarnessError::new(format!("unknown DB fault checkpoint {checkpoint}")).into()
            })
    }

    fn exit_at_db_fault(checkpoint: &str, expected: &str) -> SpikeResult<()> {
        if checkpoint == expected {
            let index = i32::try_from(db_fault_index(checkpoint)?)?;
            process::exit(DB_FAULT_EXIT_BASE + index);
        }
        Ok(())
    }

    fn child_db_fault(database: &Path, checkpoint: &str) -> SpikeResult<()> {
        let _ = db_fault_index(checkpoint)?;
        let (keyed, _, _) = initialize_database(database, false)?;
        keyed.connection.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);\
             PRAGMA wal_autocheckpoint = 0;\
             BEGIN IMMEDIATE;",
        )?;
        exit_at_db_fault(checkpoint, "DB01")?;

        keyed.connection.execute_batch(
            "INSERT INTO command_receipt (\
                 client_instance_id, idempotency_key, request_hash, expected_revision,\
                 committed_revision, response_bytes, response_hash, created_at\
             ) VALUES (\
                 x'f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1',\
                 x'f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2',\
                 x'f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3',\
                 0, 1, x'dbe10007',\
                 x'f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4f4',\
                 20000\
             );\
             INSERT INTO ledger_batch (\
                 batch_id, signed_envelope, envelope_hash, deterministic_payload,\
                 deterministic_payload_hash, signing_public_key, signature, device_id,\
                 origin_seq_start, origin_seq_end, previous_batch_hash, origin_created_at,\
                 event_schema_version, accept_seq_start, accept_seq_end, accepted_at\
             ) VALUES (\
                 x'01010101010101010101010101010101', x'01', zeroblob(32), x'02',\
                 x'0404040404040404040404040404040404040404040404040404040404040404',\
                 zeroblob(32), zeroblob(64), x'07070707070707070707070707070707',\
                 1, 4, NULL, 20000, 1, 1, 4, 20001\
             );",
        )?;
        exit_at_db_fault(checkpoint, "DB02")?;

        keyed.connection.execute_batch(
            "INSERT INTO ledger_event (\
                 event_id, batch_id, origin_seq, origin_observed_at, accept_seq, actor_kind,\
                 actor_canonical, domain_id, event_kind, canonical_payload, payload_hash\
             ) VALUES\
             (x'11000000000000000000000000000001', x'01010101010101010101010101010101',\
              1, 20000, 1, 'DETERMINISTIC_ENGINE', x'01',\
              x'30303030303030303030303030303030', 'SCOPE_REGISTERED', x'01',\
              x'2100000000000000000000000000000000000000000000000000000000000001'),\
             (x'11000000000000000000000000000002', x'01010101010101010101010101010101',\
              2, 20000, 2, 'DETERMINISTIC_ENGINE', x'01',\
              x'30303030303030303030303030303030', 'ARTIFACT_REGISTERED', x'02',\
              x'2100000000000000000000000000000000000000000000000000000000000002'),\
             (x'11000000000000000000000000000003', x'01010101010101010101010101010101',\
              3, 20000, 3, 'DETERMINISTIC_ENGINE', x'01',\
              x'30303030303030303030303030303030', 'EVIDENCE_REGISTERED', x'03',\
              x'2100000000000000000000000000000000000000000000000000000000000003'),\
             (x'11000000000000000000000000000004', x'01010101010101010101010101010101',\
              4, 20000, 4, 'DETERMINISTIC_ENGINE', x'01',\
              x'30303030303030303030303030303030', 'CLAIM_ASSERTED', x'04',\
              x'2100000000000000000000000000000000000000000000000000000000000004');",
        )?;
        exit_at_db_fault(checkpoint, "DB03")?;

        keyed.connection.execute_batch(
            "INSERT INTO scope (scope_id, created_event_id, domain_id, label) VALUES (\
                 x'40404040404040404040404040404040',\
                 x'11000000000000000000000000000001',\
                 x'30303030303030303030303030303030', 'E1 synthetic scope'\
             );\
             INSERT INTO artifact_descriptor (\
                 artifact_id, registered_event_id, content_digest, media_type, byte_length,\
                 domain_id, confidentiality, retention_class, permission_lineage_id,\
                 format_version, vault_locator\
             ) VALUES (\
                 x'50505050505050505050505050505050',\
                 x'11000000000000000000000000000002',\
                 x'5151515151515151515151515151515151515151515151515151515151515151',\
                 'application/octet-stream', 1, x'30303030303030303030303030303030',\
                 'PUBLIC', 'EPHEMERAL', x'52525252525252525252525252525252', 1,\
                 x'5353535353535353535353535353535353535353535353535353535353535353'\
             );\
             INSERT INTO artifact_representation (\
                 artifact_id, representation_index, locator_kind, locator_payload,\
                 content_digest, byte_length\
             ) VALUES (\
                 x'50505050505050505050505050505050', 0, 'TEXT_BYTES', x'00',\
                 x'5454545454545454545454545454545454545454545454545454545454545454', 1\
             );\
             INSERT INTO evidence_item (\
                 evidence_id, registered_event_id, artifact_id, representation_index,\
                 excerpt_digest, evidence_role, evidence_strength, extraction_method,\
                 extractor_version\
             ) VALUES (\
                 x'60606060606060606060606060606060',\
                 x'11000000000000000000000000000003',\
                 x'50505050505050505050505050505050', 0,\
                 x'6161616161616161616161616161616161616161616161616161616161616161',\
                 'SUPPORTS', 'DIRECT', 'synthetic', 'e1'\
             );\
             INSERT INTO claim (\
                 claim_id, assertion_event_id, subject_entity_id, predicate_id, scope_id,\
                 object_kind, object_text, authority_class, epistemic_status,\
                 confidence_permille, valid_from\
             ) VALUES (\
                 x'70707070707070707070707070707070',\
                 x'11000000000000000000000000000004',\
                 x'71717171717171717171717171717171', 'academic.e1',\
                 x'40404040404040404040404040404040', 'TEXT', 'synthetic',\
                 'DIRECT_OBSERVATION', 'CODE_OBSERVED', 1000, 20000\
             );\
             INSERT INTO claim_evidence (claim_id, evidence_id, evidence_ordinal) VALUES (\
                 x'70707070707070707070707070707070',\
                 x'60606060606060606060606060606060', 0\
             );",
        )?;
        exit_at_db_fault(checkpoint, "DB04")?;

        keyed.connection.execute_batch(
            "INSERT INTO projection_outbox (\
                 outbox_seq, accepted_batch_id, accept_seq_start, accept_seq_end,\
                 canonical_revision, event_kind_mask, payload_digest, created_at\
             ) VALUES (\
                 1, x'01010101010101010101010101010101', 1, 4, 1,\
                 x'0000000000000001',\
                 x'f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5',\
                 20001\
             );",
        )?;
        exit_at_db_fault(checkpoint, "DB05")?;

        keyed.connection.execute_batch(
            "INSERT INTO device_head (\
                 device_id, next_origin_seq, head_batch_id, head_envelope_hash, updated_at\
             ) VALUES (\
                 x'07070707070707070707070707070707', 5,\
                 x'01010101010101010101010101010101', zeroblob(32), 20001\
             );\
             UPDATE replica_state SET next_accept_seq = 5, profile_revision = 1 \
             WHERE singleton = 1;",
        )?;
        exit_at_db_fault(checkpoint, "DB06")?;

        keyed.connection.execute_batch("COMMIT;")?;
        exit_at_db_fault(checkpoint, "DB07")?;
        Err(HarnessError::new("DB fault child passed its validated checkpoint").into())
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DbFaultDisposition {
        RolledBack,
        Committed,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct DbFaultState {
        counts: Vec<i64>,
        replica_state: (i64, i64),
        receipt: Option<Vec<u8>>,
    }

    fn read_db_fault_state(connection: &Connection) -> SpikeResult<DbFaultState> {
        let counts = [
            "ledger_batch",
            "ledger_event",
            "scope",
            "artifact_descriptor",
            "artifact_representation",
            "evidence_item",
            "claim",
            "claim_evidence",
            "projection_outbox",
            "device_head",
        ]
        .map(|table| {
            connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get::<_, i64>(0)
            })
        });
        let counts = counts.into_iter().collect::<Result<Vec<_>, _>>()?;
        let replica_state = connection.query_row(
            "SELECT next_accept_seq, profile_revision FROM replica_state WHERE singleton = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        let receipt = connection
            .query_row(
            concat!(
                "SELECT response_bytes FROM command_receipt ",
                "WHERE client_instance_id = x'f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1' ",
                "AND idempotency_key = x'f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2' ",
                "AND request_hash = x'f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3'"
            ),
            [],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()?;
        Ok(DbFaultState {
            counts,
            replica_state,
            receipt,
        })
    }

    fn classify_db_fault_state(state: DbFaultState) -> SpikeResult<DbFaultDisposition> {
        if state.counts == vec![0; state.counts.len()]
            && state.replica_state == (1, 0)
            && state.receipt.is_none()
        {
            return Ok(DbFaultDisposition::RolledBack);
        }
        if state.counts == vec![1, 4, 1, 1, 1, 1, 1, 1, 1, 1]
            && state.replica_state == (5, 1)
            && state.receipt.as_deref() == Some(DB_FAULT_RESPONSE.as_slice())
        {
            return Ok(DbFaultDisposition::Committed);
        }
        Err(HarnessError::new(format!("DB fault exposed partial state: {state:?}")).into())
    }

    /// Verifies rollback for DB01-DB06 or committed, replayable receipt state for DB07.
    pub fn db_fault_outcome(path: &Path, checkpoint: &str) -> SpikeResult<String> {
        let index = db_fault_index(checkpoint)?;
        let mut keyed = KeyedConnection::open(path, PRIMARY_KEY, "primary")?;
        verify_shared_current(&mut keyed.connection)?;
        let disposition = classify_db_fault_state(read_db_fault_state(&keyed.connection)?)?;
        match (index < 6, disposition) {
            (true, DbFaultDisposition::RolledBack) => {
                Ok("ROLLED_BACK_NO_SEQUENCE_CONSUMED".to_owned())
            }
            (false, DbFaultDisposition::Committed) => {
                Ok("COMMITTED_EXACT_RECEIPT_REPLAYABLE".to_owned())
            }
            _ => Err(HarnessError::new(format!(
                "{checkpoint} recovered with the wrong atomic disposition: {disposition:?}"
            ))
            .into()),
        }
    }

    fn write_marker(path: &Path, value: &str) -> SpikeResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
        file.write_all(value.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    fn child_wal_crash(database: &Path) -> SpikeResult<()> {
        child_db_fault(database, "DB07")
    }

    fn child_rekey(database: &Path, before: &Path, after: &Path) -> SpikeResult<()> {
        let mut keyed = KeyedConnection::open(database, PRIMARY_KEY, "primary")?;
        verify_canaries(&keyed.connection, &load_canaries()?)?;
        write_marker(before, "rekey-invoked-before-first-page-rewrite")?;
        keyed.connection.pragma_update(None, "rekey", REKEY_KEY)?;
        verify_shared_current(&mut keyed.connection)?;
        write_marker(after, "rekey-completed-before-acknowledgement")?;
        process::exit(REKEY_COMPLETE_EXIT);
    }

    fn json_escape(value: &str) -> String {
        let mut output = String::with_capacity(value.len());
        for character in value.chars() {
            match character {
                '"' => output.push_str("\\\""),
                '\\' => output.push_str("\\\\"),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                value if value.is_control() => {
                    output.push_str(&format!("\\u{:04x}", u32::from(value)))
                }
                value => output.push(value),
            }
        }
        output
    }

    fn path_argument(
        arguments: &mut impl Iterator<Item = OsString>,
        name: &str,
    ) -> SpikeResult<PathBuf> {
        arguments
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| HarnessError::new(format!("missing {name} path argument")).into())
    }

    /// Dispatches bounded local-only harness commands.
    pub fn run_cli() -> SpikeResult<()> {
        let mut arguments = std::env::args_os();
        let _binary = arguments.next();
        let Some(command) = arguments.next() else {
            println!(
                "{{\"feature_enabled\":true,\"evidence_only\":true,\"production_data_allowed\":false,\"adr_002_accepted\":false}}"
            );
            return Ok(());
        };
        match command.to_string_lossy().as_ref() {
            "run" => {
                let root = path_argument(&mut arguments, "artifact-root")?;
                if arguments.next().is_some() {
                    return Err(HarnessError::new("unexpected extra run argument").into());
                }
                let receipt = run_full_harness(&root)?;
                println!("{}", receipt.to_json());
                Ok(())
            }
            "child-wal-crash" => {
                let database = path_argument(&mut arguments, "database")?;
                child_wal_crash(&database)
            }
            "child-db-fault" => {
                let checkpoint = arguments
                    .next()
                    .ok_or_else(|| HarnessError::new("missing DB fault checkpoint"))?;
                let database = path_argument(&mut arguments, "database")?;
                if arguments.next().is_some() {
                    return Err(HarnessError::new("unexpected extra DB fault argument").into());
                }
                child_db_fault(&database, &checkpoint.to_string_lossy())
            }
            "child-rekey" => {
                let database = path_argument(&mut arguments, "database")?;
                let before = path_argument(&mut arguments, "before-marker")?;
                let after = path_argument(&mut arguments, "after-marker")?;
                child_rekey(&database, &before, &after)
            }
            "posture" => {
                println!(
                    "{{\"feature_enabled\":true,\"evidence_only\":true,\"production_data_allowed\":false,\"adr_002_accepted\":false}}"
                );
                Ok(())
            }
            other => {
                Err(HarnessError::new(format!("unknown SQLCipher spike command {other}")).into())
            }
        }
    }

    /// Polls a marker with a bounded wait; shared by the process-fault test.
    pub fn wait_for_marker(path: &Path, attempts: usize) -> bool {
        for _ in 0..attempts {
            if path.is_file() {
                return true;
            }
            thread::sleep(Duration::from_millis(2));
        }
        false
    }

    /// Exit code used by the deterministic WAL crash child.
    #[must_use]
    pub const fn wal_crash_exit_code() -> i32 {
        DB_FAULT_EXIT_BASE + 6
    }

    /// Exit code used by a child stopped at one of DB01-DB07.
    pub fn db_fault_exit_code(checkpoint: &str) -> SpikeResult<i32> {
        Ok(DB_FAULT_EXIT_BASE + i32::try_from(db_fault_index(checkpoint)?)?)
    }

    /// Exit code used when rekey completes before the parent kills it.
    #[must_use]
    pub const fn rekey_complete_exit_code() -> i32 {
        REKEY_COMPLETE_EXIT
    }
}

pub use enabled::*;

fn main() {
    if let Err(error) = enabled::run_cli() {
        eprintln!("sqlcipher-spike failed: {error}");
        std::process::exit(2);
    }
}
