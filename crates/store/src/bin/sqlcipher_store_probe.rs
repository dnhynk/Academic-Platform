//! Local-only harness for the encrypted schema-2 store lane.
//!
//! Cargo's explicit binary target requires the non-default `sqlcipher-store`
//! feature, so ordinary product and workspace builds cannot compile this file
//! at all. It exists to give the `EN` and replayed `DB` fault rows a real
//! process to terminate; everything that does not need a separate process is
//! exercised in-process by `tests/encrypted_profile.rs` through the library.
//!
//! The library contains no environment lookup, no CLI switch, and no crash
//! switch. Every failpoint below is in this file, behind this feature.

// This module is compiled twice: once as this binary, which reaches only the
// child-process and receipt entry points, and once through a `#[path]` include
// in `tests/encrypted_profile.rs`, which reaches the parent-side helpers. Each
// build therefore leaves part of it unused, and neither half is dead.
#[allow(dead_code)]
pub mod enabled {
    use std::{
        collections::BTreeSet,
        error::Error,
        ffi::OsString,
        fmt, fs,
        path::{Path, PathBuf},
        process::{self, Command, Stdio},
        time::Duration,
    };

    use academic_crypto::{
        ProfileId, RECOVERY_ARGON2ID_V1, RecipientRecord, RecipientSet, RecoverySecret, StoreKey,
        UnlockThrottle, VaultMasterKey, create_recovery_recipient, unlock_with_recovery,
    };
    use academic_store::{
        STORE_DATABASE_FILE,
        cipher::{
            CipherSettings, EncryptedProfile, create_encrypted_profile, open_encrypted_profile,
        },
        path_policy::NativePathProbe,
    };
    use rusqlite::{Connection, OpenFlags, TransactionBehavior, backup::Backup, params};

    /// Deterministic synthetic recovery secret.
    ///
    /// This is not a key and never becomes one: it unwraps a Vault Master Key
    /// that this harness generated from operating-system randomness moments
    /// earlier, for a throwaway synthetic profile. The parent and the child
    /// process reach the same `SKEY_p` by unlocking the same recipient record,
    /// which is the product flow, rather than by passing key bytes between
    /// processes.
    const SYNTHETIC_RECOVERY_SECRET: [u8; 32] = [0x5a; 32];
    const SYNTHETIC_PROFILE_ID: [u8; 16] = [0x2a; 16];
    const SYNTHETIC_RECIPIENT_ID: [u8; 16] = [0x2b; 16];
    const BUILD_DIGEST: [u8; 32] = [0xe2; 32];

    /// File holding the wrapped Vault Master Key, outside the profile root.
    ///
    /// Only ciphertext is written. The `keys/recipients.cbor` location inside a
    /// profile belongs to `P2-K4` together with recovery-profile selection, so
    /// this harness keeps its record beside the profile instead of inventing
    /// that layout early.
    pub const RECIPIENT_FILE: &str = "recipients.cbor";
    /// Profile root created under a harness working directory.
    pub const PROFILE_DIRECTORY: &str = "profile";

    /// Committed synthetic canary corpus for the encrypted lane.
    pub const CANARY_FILE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/sqlcipher-canary/store-v2-canaries.txt"
    );
    /// Directory the receipt run copies every scanned artifact into.
    pub const ARTIFACT_DIRECTORY: &str = "artifacts";
    const CANARY_CREATED_AT_BASE: i64 = 30_000;

    const DB_FAULT_IDS: [&str; 7] = ["DB01", "DB02", "DB03", "DB04", "DB05", "DB06", "DB07"];
    const DB_FAULT_EXIT_BASE: i32 = 100;
    const REKEY_STARTED_EXIT: i32 = 87;
    const WAL_CRASH_EXIT: i32 = 88;

    /// Error type for evidence-contract failures rather than SQLite failures.
    #[derive(Debug)]
    pub struct HarnessError {
        message: String,
    }

    impl HarnessError {
        pub fn new(message: impl Into<String>) -> Self {
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

    /// Result used by the probe binary and its integration tests.
    pub type ProbeResult<T> = Result<T, Box<dyn Error>>;

    fn synthetic_profile() -> ProfileId {
        ProfileId::from_bytes(SYNTHETIC_PROFILE_ID)
    }

    fn synthetic_secret() -> RecoverySecret {
        RecoverySecret::from_entropy(SYNTHETIC_RECOVERY_SECRET)
    }

    /// Generates a Vault Master Key, wraps it for the synthetic recovery
    /// recipient, writes the record, and returns the derived `SKEY_p`.
    pub fn provision(workdir: &Path) -> ProbeResult<StoreKey> {
        fs::create_dir_all(workdir)?;
        let master = VaultMasterKey::generate()?;
        let record = create_recovery_recipient(
            &master,
            synthetic_profile(),
            SYNTHETIC_RECIPIENT_ID,
            &synthetic_secret(),
            RECOVERY_ARGON2ID_V1,
        )?;
        let mut set = RecipientSet::new(synthetic_profile());
        set.push(record);
        fs::write(workdir.join(RECIPIENT_FILE), set.to_canonical_cbor()?)?;
        Ok(master.derive_store_key(synthetic_profile())?)
    }

    /// Reads the recipient record and unlocks the same `SKEY_p`.
    pub fn unlock(workdir: &Path) -> ProbeResult<StoreKey> {
        let bytes = fs::read(workdir.join(RECIPIENT_FILE))?;
        let set = RecipientSet::from_canonical_cbor(&bytes)?;
        let record: &RecipientRecord = set
            .records()
            .first()
            .ok_or_else(|| HarnessError::new("recipient set is empty"))?;
        let mut throttle = UnlockThrottle::new();
        let master = unlock_with_recovery(
            record,
            synthetic_profile(),
            &synthetic_secret(),
            &mut throttle,
            0,
        )?;
        Ok(master.derive_store_key(synthetic_profile())?)
    }

    /// Returns the profile root inside a harness working directory.
    #[must_use]
    pub fn profile_root(workdir: &Path) -> PathBuf {
        workdir.join(PROFILE_DIRECTORY)
    }

    /// Creates a fresh encrypted profile inside a harness working directory.
    pub fn create_profile(workdir: &Path, key: &StoreKey) -> ProbeResult<EncryptedProfile> {
        Ok(create_encrypted_profile(
            &profile_root(workdir),
            &NativePathProbe::default(),
            key,
            BUILD_DIGEST,
        )?)
    }

    /// Opens the encrypted profile inside a harness working directory.
    pub fn open_profile(workdir: &Path, key: &StoreKey) -> ProbeResult<EncryptedProfile> {
        Ok(open_encrypted_profile(
            &profile_root(workdir),
            &NativePathProbe::default(),
            key,
        )?)
    }

    /// Path of the SQLCipher database inside a harness working directory.
    #[must_use]
    pub fn database_path(workdir: &Path) -> PathBuf {
        profile_root(workdir).join(STORE_DATABASE_FILE)
    }

    /// Opens a keyed handle with the key applied before the first page access.
    pub fn open_keyed(path: &Path, key: &StoreKey) -> ProbeResult<Connection> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let connection = Connection::open_with_flags(path, flags)?;
        apply_raw_key(&connection, key)?;
        Ok(connection)
    }

    /// Applies a raw 32-byte key as `PRAGMA key = "x'<64 hex>'"`.
    pub fn apply_raw_key(connection: &Connection, key: &StoreKey) -> ProbeResult<()> {
        let hex = key.expose_raw_hex();
        let literal = format!("x'{}'", hex.as_str());
        connection.pragma_update(None, "key", literal.as_str())?;
        Ok(())
    }

    /// Reports whether a keyed handle can actually authenticate page one.
    pub fn page_one_authenticates(path: &Path, key: &StoreKey) -> ProbeResult<bool> {
        let Ok(connection) = open_keyed(path, key) else {
            return Ok(false);
        };
        Ok(connection
            .query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
                row.get::<_, i64>(0)
            })
            .is_ok())
    }

    /// Reads back the cipher settings of a keyed handle.
    pub fn observed_cipher_settings(workdir: &Path, key: &StoreKey) -> ProbeResult<CipherSettings> {
        let connection = open_keyed(&database_path(workdir), key)?;
        Ok(academic_store::cipher::read_cipher_settings(&connection)?)
    }

    /// Canonical row counts a parent uses to classify a fault outcome.
    pub fn canonical_counts(workdir: &Path, key: &StoreKey) -> ProbeResult<Vec<(String, i64)>> {
        let connection = open_keyed(&database_path(workdir), key)?;
        let mut counts = Vec::new();
        for table in CANONICAL_COUNT_TABLES {
            let value: i64 =
                connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?;
            counts.push(((*table).to_owned(), value));
        }
        Ok(counts)
    }

    /// Tables whose row counts distinguish a complete acceptance from a partial one.
    pub const CANONICAL_COUNT_TABLES: &[&str] = &[
        "command_receipt",
        "ledger_batch",
        "ledger_event",
        "scope",
        "artifact_descriptor",
        "evidence_item",
        "claim",
        "claim_evidence",
        "projection_outbox",
        "device_head",
    ];

    /// Streams every file under `root` counting occurrences of each pattern.
    pub fn scan_for(root: &Path, needles: &[Vec<u8>]) -> ProbeResult<(u64, u64, usize)> {
        let mut files = Vec::new();
        collect_files(root, &mut files)?;
        files.sort();
        let mut file_count = 0_u64;
        let mut byte_count = 0_u64;
        let mut hits = 0_usize;
        for path in files {
            let bytes = fs::read(&path)?;
            file_count += 1;
            byte_count += u64::try_from(bytes.len())?;
            for needle in needles {
                hits += count_occurrences(&bytes, needle);
            }
        }
        Ok((file_count, byte_count, hits))
    }

    fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> ProbeResult<()> {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                collect_files(&path, files)?;
            } else {
                files.push(path);
            }
        }
        Ok(())
    }

    fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
        if needle.is_empty() || haystack.len() < needle.len() {
            return 0;
        }
        let mut found = 0;
        for window in haystack.windows(needle.len()) {
            if window == needle {
                found += 1;
            }
        }
        found
    }

    fn db_fault_index(checkpoint: &str) -> ProbeResult<usize> {
        DB_FAULT_IDS
            .iter()
            .position(|candidate| candidate == &checkpoint)
            .ok_or_else(|| {
                HarnessError::new(format!("unknown DB fault checkpoint {checkpoint}")).into()
            })
    }

    /// Exit code the parent expects when a child died at `checkpoint`.
    pub fn db_fault_exit_code(checkpoint: &str) -> ProbeResult<i32> {
        Ok(DB_FAULT_EXIT_BASE + i32::try_from(db_fault_index(checkpoint)?)?)
    }

    /// Exit code the parent expects when a rekey child died mid-rekey.
    #[must_use]
    pub const fn rekey_started_exit_code() -> i32 {
        REKEY_STARTED_EXIT
    }

    /// Exit code the parent expects when a WAL child died with frames unchecked.
    #[must_use]
    pub const fn wal_crash_exit_code() -> i32 {
        WAL_CRASH_EXIT
    }

    fn exit_at_db_fault(checkpoint: &str, expected: &str) -> ProbeResult<()> {
        if checkpoint == expected {
            process::exit(DB_FAULT_EXIT_BASE + i32::try_from(db_fault_index(checkpoint)?)?);
        }
        Ok(())
    }

    /// Runs the canonical acceptance ordering inside one transaction over the
    /// encrypted database and terminates the process at `checkpoint`.
    ///
    /// The insert sequence mirrors the Phase 1 `DB01`-`DB07` ordering exactly,
    /// so what is re-run under encryption is the same boundary set rather than
    /// a different one that happens to share the names.
    fn child_db_fault(workdir: &Path, checkpoint: &str) -> ProbeResult<()> {
        let _ = db_fault_index(checkpoint)?;
        let key = unlock(workdir)?;
        let connection = open_keyed(&database_path(workdir), &key)?;
        connection.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);\
             PRAGMA wal_autocheckpoint = 0;\
             BEGIN IMMEDIATE;",
        )?;
        exit_at_db_fault(checkpoint, "DB01")?;

        connection.execute_batch(CANONICAL_RECEIPT_AND_BATCH)?;
        exit_at_db_fault(checkpoint, "DB02")?;

        connection.execute_batch(CANONICAL_EVENTS)?;
        exit_at_db_fault(checkpoint, "DB03")?;

        connection.execute_batch(CANONICAL_AGGREGATES)?;
        exit_at_db_fault(checkpoint, "DB04")?;

        connection.execute_batch(CANONICAL_OUTBOX)?;
        exit_at_db_fault(checkpoint, "DB05")?;

        connection.execute_batch(CANONICAL_HEADS)?;
        exit_at_db_fault(checkpoint, "DB06")?;

        connection.execute_batch("COMMIT;")?;
        exit_at_db_fault(checkpoint, "DB07")?;
        Err(HarnessError::new("DB fault child passed its validated checkpoint").into())
    }

    /// `EN01`: terminates the process while a store rekey is in flight.
    ///
    /// The child writes `marker` immediately before issuing `PRAGMA rekey` so
    /// the parent can prove the rekey had started, then exits without letting
    /// SQLite finish rewriting every page.
    fn child_rekey(workdir: &Path, marker: &Path) -> ProbeResult<()> {
        let key = unlock(workdir)?;
        let connection = open_keyed(&database_path(workdir), &key)?;
        // A rekey rewrites every page, so a large payload guarantees the kill
        // lands inside the rewrite rather than after it.
        connection.execute_batch(
            "PRAGMA wal_autocheckpoint = 0;\
             CREATE TABLE IF NOT EXISTS temp_rekey_payload (id INTEGER PRIMARY KEY, blob BLOB);\
             INSERT INTO temp_rekey_payload (blob)\
             WITH RECURSIVE counter(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM counter LIMIT 4096)\
             SELECT zeroblob(16384) FROM counter;\
             PRAGMA wal_checkpoint(TRUNCATE);",
        )?;
        fs::write(marker, b"rekey-started\n")?;
        let rekey = format!("x'{}'", REKEY_TARGET_HEX);
        // The rekey is started on a background-free connection and the process
        // is killed from under it; whichever key survives is what the parent
        // has to document.
        let _ = connection.pragma_update(None, "rekey", rekey.as_str());
        process::exit(REKEY_STARTED_EXIT);
    }

    /// The second raw key `EN01` rekeys towards. Synthetic and local-only.
    pub const REKEY_TARGET_HEX: &str =
        "51a3b4e62d7fc491fb82711960b3a9dd940a285fa075837a9fe43ce8f1c7b026";

    /// `EN04`: leaves committed frames in the write-ahead log, then dies.
    fn child_wal_crash(workdir: &Path) -> ProbeResult<()> {
        let key = unlock(workdir)?;
        let connection = open_keyed(&database_path(workdir), &key)?;
        connection.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);\
             PRAGMA wal_autocheckpoint = 0;\
             BEGIN IMMEDIATE;",
        )?;
        connection.execute_batch(CANONICAL_RECEIPT_AND_BATCH)?;
        connection.execute_batch(CANONICAL_EVENTS)?;
        connection.execute_batch(CANONICAL_AGGREGATES)?;
        connection.execute_batch(CANONICAL_OUTBOX)?;
        connection.execute_batch(CANONICAL_HEADS)?;
        connection.execute_batch("COMMIT;")?;
        process::exit(WAL_CRASH_EXIT);
    }

    const CANONICAL_RECEIPT_AND_BATCH: &str = "\
        INSERT INTO command_receipt (\
            client_instance_id, idempotency_key, request_hash, expected_revision,\
            committed_revision, response_bytes, response_hash, created_at\
        ) VALUES (\
            x'f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1',\
            x'f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2',\
            x'f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3',\
            0, 1, x'dbe20007',\
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
        );";

    const CANONICAL_EVENTS: &str = "\
        INSERT INTO ledger_event (\
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
         x'2100000000000000000000000000000000000000000000000000000000000004');";

    const CANONICAL_AGGREGATES: &str = "\
        INSERT INTO scope (scope_id, created_event_id, domain_id, label) VALUES (\
            x'40404040404040404040404040404040',\
            x'11000000000000000000000000000001',\
            x'30303030303030303030303030303030', 'K2 synthetic scope'\
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
            'SUPPORTS', 'DIRECT', 'synthetic', 'k2'\
        );\
        INSERT INTO claim (\
            claim_id, assertion_event_id, subject_entity_id, predicate_id, scope_id,\
            object_kind, object_text, authority_class, epistemic_status,\
            confidence_permille, valid_from\
        ) VALUES (\
            x'70707070707070707070707070707070',\
            x'11000000000000000000000000000004',\
            x'71717171717171717171717171717171', 'academic.k2',\
            x'40404040404040404040404040404040', 'TEXT', 'synthetic',\
            'DIRECT_OBSERVATION', 'CODE_OBSERVED', 1000, 20000\
        );\
        INSERT INTO claim_evidence (claim_id, evidence_id, evidence_ordinal) VALUES (\
            x'70707070707070707070707070707070',\
            x'60606060606060606060606060606060', 0\
        );";

    const CANONICAL_OUTBOX: &str = "\
        INSERT INTO projection_outbox (\
            outbox_seq, accepted_batch_id, accept_seq_start, accept_seq_end,\
            canonical_revision, event_kind_mask, payload_digest, created_at\
        ) VALUES (\
            1, x'01010101010101010101010101010101', 1, 4, 1,\
            x'0000000000000001',\
            x'f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5',\
            20001\
        );";

    const CANONICAL_HEADS: &str = "\
        INSERT INTO device_head (\
            device_id, next_origin_seq, head_batch_id, head_envelope_hash, updated_at\
        ) VALUES (\
            x'07070707070707070707070707070707', 5,\
            x'01010101010101010101010101010101', zeroblob(32), 20001\
        );\
        UPDATE replica_state SET next_accept_seq = 5, profile_revision = 1 \
        WHERE singleton = 1;";

    /// Loads and validates the committed synthetic canary corpus.
    pub fn load_canaries() -> ProbeResult<Vec<String>> {
        let text = fs::read_to_string(CANARY_FILE)?;
        let mut canaries = Vec::new();
        let mut unique = BTreeSet::new();
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
                return Err(HarnessError::new("duplicate encrypted-lane canary").into());
            }
            canaries.push(candidate.to_owned());
        }
        if canaries.len() < 5 {
            return Err(HarnessError::new(
                "encrypted-lane canary corpus must contain at least five values",
            )
            .into());
        }
        Ok(canaries)
    }

    /// Writes every canary into the canonical store and into a temp table.
    ///
    /// `temp_store = MEMORY` is part of the Phase 1 connection policy, so the
    /// temp table exercises the path that would spill plaintext to disk if that
    /// policy were ever relaxed.
    pub fn write_canaries(connection: &mut Connection, canaries: &[String]) -> ProbeResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (index, canary) in canaries.iter().enumerate() {
            let ordinal = u8::try_from(index + 1)?;
            transaction.execute(
                concat!(
                    "INSERT INTO command_receipt (",
                    "client_instance_id, idempotency_key, request_hash, expected_revision, ",
                    "committed_revision, response_bytes, response_hash, created_at",
                    ") VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7)"
                ),
                params![
                    [ordinal; 16].as_slice(),
                    [ordinal.wrapping_add(0x20); 32].as_slice(),
                    [ordinal.wrapping_add(0x40); 32].as_slice(),
                    i64::try_from(index + 1)?,
                    canary.as_bytes(),
                    [ordinal.wrapping_add(0x60); 32].as_slice(),
                    CANARY_CREATED_AT_BASE + i64::try_from(index)?,
                ],
            )?;
        }
        transaction.commit()?;
        connection
            .execute_batch("CREATE TEMP TABLE store_canary_temp(value TEXT NOT NULL) STRICT;")?;
        for canary in canaries {
            connection.execute(
                "INSERT INTO temp.store_canary_temp (value) VALUES (?1)",
                params![canary.as_str()],
            )?;
        }
        let observed: i64 =
            connection.query_row("SELECT count(*) FROM temp.store_canary_temp", [], |row| {
                row.get(0)
            })?;
        if observed != i64::try_from(canaries.len())? {
            return Err(HarnessError::new("temp canary round-trip lost rows").into());
        }
        connection.execute_batch("DROP TABLE temp.store_canary_temp;")?;
        Ok(())
    }

    /// Counts the canaries readable through a keyed connection.
    pub fn readable_canary_count(
        connection: &Connection,
        canaries: &[String],
    ) -> ProbeResult<usize> {
        let mut statement =
            connection.prepare("SELECT response_bytes FROM command_receipt ORDER BY created_at")?;
        let mut rows = statement.query([])?;
        let mut found = 0;
        while let Some(row) = rows.next()? {
            let bytes = row.get::<_, Vec<u8>>(0)?;
            if canaries.iter().any(|canary| canary.as_bytes() == bytes) {
                found += 1;
            }
        }
        Ok(found)
    }

    /// Writes an encrypted online backup under a second, independent key.
    pub fn encrypted_backup(source: &Connection, destination: &Path) -> ProbeResult<()> {
        if destination.exists() {
            return Err(HarnessError::new("refusing to overwrite an existing backup").into());
        }
        let mut target = Connection::open(destination)?;
        target.pragma_update(None, "key", format!("x'{BACKUP_KEY_HEX}'").as_str())?;
        let backup = Backup::new(source, &mut target)?;
        backup.run_to_completion(64, Duration::from_millis(0), None)?;
        Ok(())
    }

    /// The independent raw key an encrypted backup is written under.
    pub const BACKUP_KEY_HEX: &str =
        "ae100f05e51f83e88f739416ddd6493e2ab237a1518fb4ee407f66a6b35b03d0";

    fn companion(database: &Path, suffix: &str) -> PathBuf {
        let mut name = database.as_os_str().to_os_string();
        name.push(suffix);
        PathBuf::from(name)
    }

    /// Copies the database and every sidecar into `destination`.
    pub fn copy_artifacts(
        database: &Path,
        destination: &Path,
        label: &str,
    ) -> ProbeResult<Vec<PathBuf>> {
        fs::create_dir_all(destination)?;
        let mut copied = Vec::new();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let source = companion(database, suffix);
            if !source.is_file() {
                continue;
            }
            let name = source
                .file_name()
                .ok_or_else(|| HarnessError::new("artifact has no file name"))?;
            let target = destination.join(format!("{label}-{}", name.to_string_lossy()));
            fs::copy(&source, &target)?;
            copied.push(target);
        }
        Ok(copied)
    }

    /// Runs one complete encrypted-lane evidence pass and prints its receipt.
    ///
    /// Nothing here accepts ADR-002. The receipt records observed facts and
    /// states that production data remains forbidden.
    pub fn run_receipt(workdir: &Path) -> ProbeResult<()> {
        if workdir.exists() {
            return Err(HarnessError::new("receipt workdir must not already exist").into());
        }
        let canaries = load_canaries()?;
        let key = provision(workdir)?;
        let profile = create_profile(workdir, &key)?;
        let database = profile.database_path().to_path_buf();
        let artifacts = workdir.join(ARTIFACT_DIRECTORY);
        fs::create_dir_all(&artifacts)?;

        let mut connection = open_keyed(&database, &key)?;
        // Leave the write-ahead log uncheckpointed so the scan sees frames that
        // still hold the committed rows rather than an empty sidecar.
        connection.execute_batch("PRAGMA wal_autocheckpoint = 0;")?;
        write_canaries(&mut connection, &canaries)?;
        let readable = readable_canary_count(&connection, &canaries)?;
        let cipher = academic_store::cipher::read_cipher_settings(&connection)?;
        encrypted_backup(&connection, &artifacts.join("backup.sqlite3"))?;
        copy_artifacts(&database, &artifacts, "live")?;
        drop(connection);

        let crash = Command::new(std::env::current_exe()?)
            .arg("child-wal-crash")
            .arg(workdir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if crash.code() != Some(wal_crash_exit_code()) {
            return Err(HarnessError::new(format!(
                "WAL crash child exited with {:?}",
                crash.code()
            ))
            .into());
        }
        copy_artifacts(&database, &artifacts, "crash")?;

        let needles: Vec<Vec<u8>> = canaries
            .iter()
            .map(|canary| canary.as_bytes().to_vec())
            .collect();
        let (files, bytes, hits) = scan_for(&artifacts, &needles)?;
        let identity = {
            let reopened = open_keyed(&database, &key)?;
            academic_store::migration::read_schema_identity(&reopened)?
        };

        println!(
            "{{\"lane\":\"sqlcipher-store\",\"adr_002_accepted\":false,\
             \"production_data_allowed\":false,\
             \"cipher_version\":\"{}\",\"sqlite_version\":\"{}\",\
             \"cipher_page_size\":{},\"kdf_iter\":{},\
             \"cipher_hmac_algorithm\":\"{}\",\"cipher_kdf_algorithm\":\"{}\",\
             \"schema_version\":{},\"schema_semver\":\"{}\",\
             \"minimum_reader_protocol\":\"{}.{}\",\"minimum_writer_protocol\":\"{}.{}\",\
             \"data_policy\":\"{}\",\"storage_mode\":\"{}\",\"storage_encryption\":\"{}\",\
             \"canary_count\":{},\"readable_canary_count\":{},\
             \"files_scanned\":{},\"bytes_scanned\":{},\"plaintext_canary_hits\":{}}}",
            cipher.cipher_version,
            cipher.sqlite_version,
            cipher.cipher_page_size,
            cipher.kdf_iter,
            cipher.cipher_hmac_algorithm,
            cipher.cipher_kdf_algorithm,
            identity.schema_version,
            identity.schema_semver,
            identity.minimum_reader_protocol.0,
            identity.minimum_reader_protocol.1,
            identity.minimum_writer_protocol.0,
            identity.minimum_writer_protocol.1,
            identity.data_policy,
            identity.storage_mode,
            identity.storage_encryption,
            canaries.len(),
            readable,
            files,
            bytes,
            hits,
        );
        Ok(())
    }

    fn path_argument(
        arguments: &mut impl Iterator<Item = OsString>,
        name: &str,
    ) -> ProbeResult<PathBuf> {
        arguments
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| HarnessError::new(format!("missing {name} path argument")).into())
    }

    /// Dispatches the bounded local-only harness commands.
    pub fn run_cli() -> ProbeResult<()> {
        let mut arguments = std::env::args_os();
        let _binary = arguments.next();
        let Some(command) = arguments.next() else {
            print_posture();
            return Ok(());
        };
        match command.to_string_lossy().as_ref() {
            "posture" => {
                print_posture();
                Ok(())
            }
            "run" => {
                let workdir = path_argument(&mut arguments, "workdir")?;
                if arguments.next().is_some() {
                    return Err(HarnessError::new("unexpected extra argument").into());
                }
                run_receipt(&workdir)
            }
            "child-db-fault" => {
                let checkpoint = arguments
                    .next()
                    .ok_or_else(|| HarnessError::new("missing DB fault checkpoint"))?;
                let workdir = path_argument(&mut arguments, "workdir")?;
                if arguments.next().is_some() {
                    return Err(HarnessError::new("unexpected extra argument").into());
                }
                child_db_fault(&workdir, &checkpoint.to_string_lossy())
            }
            "child-rekey" => {
                let workdir = path_argument(&mut arguments, "workdir")?;
                let marker = path_argument(&mut arguments, "marker")?;
                if arguments.next().is_some() {
                    return Err(HarnessError::new("unexpected extra argument").into());
                }
                child_rekey(&workdir, &marker)
            }
            "child-wal-crash" => {
                let workdir = path_argument(&mut arguments, "workdir")?;
                if arguments.next().is_some() {
                    return Err(HarnessError::new("unexpected extra argument").into());
                }
                child_wal_crash(&workdir)
            }
            other => Err(HarnessError::new(format!("unknown probe command {other}")).into()),
        }
    }

    fn print_posture() {
        println!(
            "{{\"lane\":\"sqlcipher-store\",\"evidence_only\":true,\
             \"production_data_allowed\":false,\"adr_002_accepted\":false}}"
        );
    }
}

// Unused when this file is included into the integration test, which supplies
// its own `main`; the binary target is where it runs.
#[allow(dead_code)]
fn main() {
    if let Err(error) = enabled::run_cli() {
        eprintln!("sqlcipher-store probe failed: {error}");
        std::process::exit(1);
    }
}
