//! Fixed-watermark projection builder and atomic activation runner.

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use academic_domain::{ContentDigest, DomainError, DomainId};
use academic_store::{SQLITE_APPLICATION_ID, STORE_SCHEMA_VERSION, connection::ReaderConnection};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::{
    PROJECTION_SCHEMA_VERSION,
    checksum::order_stable_checksum,
    fts::{
        SearchSourceRecord, load_search_sources, persisted_search_canonical_records,
        write_search_records,
    },
    generation::{
        ActiveGeneration, GenerationId, GenerationMetadata, GenerationState, ProjectionKind,
    },
    graph::{
        GraphSourceRecord, load_graph_sources, persisted_graph_canonical_records,
        write_graph_records,
    },
};

/// Application identity for the disposable projection sidecar (`ACPR`).
pub const PROJECTION_APPLICATION_ID: u32 = 0x4143_5052;
/// Physical version set by the ordered projection migration numbered 0002.
pub const PROJECTION_DATABASE_VERSION: u32 = 2;
/// Embedded projection-sidecar migration.
pub const MIGRATION_0002_SQL: &str =
    include_str!("../../../migrations/store/0002_phase1_projections.sql");
/// Stable Phase 1 builder algorithm identifier.
pub const PROJECTION_ALGORITHM_VERSION: &str = "phase1-full-generation-v1";

/// Deterministic projection failpoints used only through an injected test harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionFaultPoint {
    /// PR01: after a deterministic midpoint record write while still BUILDING.
    Pr01MidWrite,
    /// PR02: after checksum/count verification is durable but before activation.
    Pr02AfterChecksum,
    /// PR03: after active pointer update but before cursor update in one transaction.
    Pr03DuringActivation,
}

impl ProjectionFaultPoint {
    /// Stable evidence identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pr01MidWrite => "PR01",
            Self::Pr02AfterChecksum => "PR02",
            Self::Pr03DuringActivation => "PR03",
        }
    }
}

/// Test-only behavior boundary. Production callers use [`NoProjectionFault`].
pub trait ProjectionFaultInjector: fmt::Debug {
    /// Called only at one of the three fixed J1 ordering boundaries.
    fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()>;
}

/// Production/default runner with no fault behavior.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoProjectionFault;

impl ProjectionFaultInjector for NoProjectionFault {
    fn hit(&self, _point: ProjectionFaultPoint) -> ProjectionResult<()> {
        Ok(())
    }
}

/// Projection migration, source-integrity, build, or query failure.
#[derive(Debug)]
pub enum ProjectionError {
    Sqlite(rusqlite::Error),
    Domain(DomainError),
    Corrupt(String),
    InvalidCanonicalStore {
        component: &'static str,
        expected: String,
        actual: String,
    },
    UnsupportedSqliteBuild(&'static str),
    IntegerOverflow(u64),
    InvalidWatermark {
        requested: u64,
        latest: u64,
    },
    InvalidQuery(&'static str),
    InjectedFault(ProjectionFaultPoint),
    SystemClock,
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "projection SQLite error: {error}"),
            Self::Domain(error) => write!(formatter, "projection domain identity error: {error}"),
            Self::Corrupt(reason) => write!(formatter, "projection state is corrupt: {reason}"),
            Self::InvalidCanonicalStore {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "canonical store {component} mismatch: expected {expected}, observed {actual}"
            ),
            Self::UnsupportedSqliteBuild(reason) => {
                write!(formatter, "unsupported SQLite projection build: {reason}")
            }
            Self::IntegerOverflow(value) => {
                write!(
                    formatter,
                    "projection value {value} exceeds signed 64-bit SQLite"
                )
            }
            Self::InvalidWatermark { requested, latest } => write!(
                formatter,
                "requested source watermark {requested} exceeds latest canonical watermark {latest}"
            ),
            Self::InvalidQuery(reason) => write!(formatter, "invalid projection query: {reason}"),
            Self::InjectedFault(point) => {
                write!(formatter, "injected projection fault {}", point.as_str())
            }
            Self::SystemClock => {
                formatter.write_str("system clock is before Unix epoch or overflowed")
            }
        }
    }
}

impl Error for ProjectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::Corrupt(_)
            | Self::InvalidCanonicalStore { .. }
            | Self::UnsupportedSqliteBuild(_)
            | Self::IntegerOverflow(_)
            | Self::InvalidWatermark { .. }
            | Self::InvalidQuery(_)
            | Self::InjectedFault(_)
            | Self::SystemClock => None,
        }
    }
}

impl From<rusqlite::Error> for ProjectionError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// Result type for the projection boundary.
pub type ProjectionResult<T> = Result<T, ProjectionError>;

/// Verified build receipt. It is not query authority unless `activated` is true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationReceipt {
    pub metadata: GenerationMetadata,
    pub activated: bool,
}

/// Consumer for one verified canonical store and one disposable sidecar.
#[derive(Debug, Clone)]
pub struct ProjectionRunner {
    canonical_database_path: PathBuf,
    projection_database_path: PathBuf,
    builder_binary_digest: ContentDigest,
    effective_config_hash: ContentDigest,
}

impl ProjectionRunner {
    /// Creates or verifies the projection sidecar after the canonical store has
    /// already passed the store crate's read-only schema boundary.
    pub fn open(
        canonical_reader: &ReaderConnection,
        projection_database_path: impl AsRef<Path>,
        builder_binary_digest: ContentDigest,
        effective_config_hash: ContentDigest,
    ) -> ProjectionResult<Self> {
        let projection_database_path = projection_database_path.as_ref().to_path_buf();
        if projection_database_path == canonical_reader.database_path() {
            return Err(ProjectionError::InvalidCanonicalStore {
                component: "projection sidecar path",
                expected: "a database distinct from the canonical store".to_owned(),
                actual: projection_database_path.display().to_string(),
            });
        }
        migrate_projection_database(&projection_database_path)?;
        Ok(Self {
            canonical_database_path: canonical_reader.database_path().to_path_buf(),
            projection_database_path,
            builder_binary_digest,
            effective_config_hash,
        })
    }

    /// Returns the canonical database path without exposing a connection.
    #[must_use]
    pub fn canonical_database_path(&self) -> &Path {
        &self.canonical_database_path
    }

    /// Returns the disposable projection database path.
    #[must_use]
    pub fn projection_database_path(&self) -> &Path {
        &self.projection_database_path
    }

    /// Consumes the latest committed outbox watermark and atomically activates
    /// a complete new generation for one security domain.
    pub fn rebuild_latest(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
    ) -> ProjectionResult<GenerationReceipt> {
        self.rebuild(kind, domain, BuildTarget::Latest, true, &NoProjectionFault)
    }

    /// Builds a VERIFIED, inactive generation at an explicit historical
    /// `accept_seq` watermark for time-travel comparison.
    pub fn build_as_known(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        source_accept_seq: u64,
    ) -> ProjectionResult<GenerationReceipt> {
        self.rebuild(
            kind,
            domain,
            BuildTarget::AsKnown(source_accept_seq),
            false,
            &NoProjectionFault,
        )
    }

    /// Fault-harness entrypoint. The trait has no process-exit implementation in
    /// production code; integration tests provide one in a child process.
    #[doc(hidden)]
    pub fn rebuild_latest_with_faults<F: ProjectionFaultInjector + ?Sized>(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        faults: &F,
    ) -> ProjectionResult<GenerationReceipt> {
        self.rebuild(kind, domain, BuildTarget::Latest, true, faults)
    }

    /// Drops every disposable generation, pointer, cursor, and FTS row for one
    /// kind/domain. Canonical rows and outbox are on a read-only connection and
    /// are structurally outside this transaction.
    pub fn drop_projection(&self, kind: ProjectionKind, domain: DomainId) -> ProjectionResult<()> {
        let mut connection = open_projection_writer(&self.projection_database_path)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let generation_filter = concat!(
            "SELECT generation_id FROM projection_generation ",
            "WHERE projection_kind = ?1 AND security_domain = ?2"
        );
        for table in ["projection_search_unicode", "projection_search_trigram"] {
            let sql = format!(
                "DELETE FROM {table} WHERE rowid IN (SELECT content_id FROM \
                 projection_search_content WHERE generation_id IN ({generation_filter}))"
            );
            transaction.execute(&sql, params![kind.as_str(), domain.as_bytes().as_slice()])?;
        }
        transaction.execute(
            "DELETE FROM projection_active WHERE projection_kind = ?1 AND security_domain = ?2",
            params![kind.as_str(), domain.as_bytes().as_slice()],
        )?;
        transaction.execute(
            "DELETE FROM projection_cursor WHERE projection_kind = ?1 AND security_domain = ?2",
            params![kind.as_str(), domain.as_bytes().as_slice()],
        )?;
        transaction.execute(
            concat!(
                "DELETE FROM projection_generation ",
                "WHERE projection_kind = ?1 AND security_domain = ?2"
            ),
            params![kind.as_str(), domain.as_bytes().as_slice()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Reads immutable generation metadata for audit and fault verification.
    pub fn generation(&self, generation_id: GenerationId) -> ProjectionResult<GenerationMetadata> {
        let connection = open_projection_reader(&self.projection_database_path)?;
        read_generation_metadata(&connection, generation_id)
    }

    fn rebuild<F: ProjectionFaultInjector + ?Sized>(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        target: BuildTarget,
        activate: bool,
        faults: &F,
    ) -> ProjectionResult<GenerationReceipt> {
        let mut canonical = open_canonical_reader(&self.canonical_database_path)?;
        let canonical_transaction =
            canonical.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let latest = latest_watermark(&canonical_transaction)?;
        let watermark = match target {
            BuildTarget::Latest => latest,
            BuildTarget::AsKnown(requested) => {
                if requested > latest.source_accept_seq {
                    return Err(ProjectionError::InvalidWatermark {
                        requested,
                        latest: latest.source_accept_seq,
                    });
                }
                Watermark {
                    source_accept_seq: requested,
                    source_outbox_seq: outbox_at_or_before(&canonical_transaction, requested)?,
                }
            }
        };
        let source_records = match kind {
            ProjectionKind::Graph => SourceRecords::Graph(load_graph_sources(
                &canonical_transaction,
                domain,
                watermark.source_accept_seq,
            )?),
            ProjectionKind::Unicode61 | ProjectionKind::Trigram => SourceRecords::Search(
                load_search_sources(&canonical_transaction, domain, watermark.source_accept_seq)?,
            ),
        };
        canonical_transaction.commit()?;

        let built_at_unix_ms = unix_time_millis()?;
        let mut projection = open_projection_writer(&self.projection_database_path)?;
        let generation_id = create_building_generation(
            &mut projection,
            kind,
            domain,
            watermark,
            self.builder_binary_digest,
            self.effective_config_hash,
            built_at_unix_ms,
        )?;

        let write_result = write_records(
            &mut projection,
            kind,
            generation_id,
            &source_records,
            faults,
        );
        if let Err(error) = write_result {
            mark_failed(&mut projection, generation_id, &error.to_string())?;
            return Err(error);
        }

        let expected_records = source_records.canonical_records();
        let expected_count = u64::try_from(expected_records.len())
            .map_err(|_| ProjectionError::Corrupt("projection record count overflow".to_owned()))?;
        let expected_checksum = order_stable_checksum(expected_records);
        let persisted_records = match kind {
            ProjectionKind::Graph => persisted_graph_canonical_records(&projection, generation_id)?,
            ProjectionKind::Unicode61 | ProjectionKind::Trigram => {
                persisted_search_canonical_records(&projection, generation_id)?
            }
        };
        let actual_count = u64::try_from(persisted_records.len())
            .map_err(|_| ProjectionError::Corrupt("persisted record count overflow".to_owned()))?;
        let actual_checksum = order_stable_checksum(persisted_records);
        if actual_count != expected_count || actual_checksum != expected_checksum {
            let error = ProjectionError::Corrupt(format!(
                "generation verification mismatch: expected {expected_count}/{expected_checksum}, observed {actual_count}/{actual_checksum}"
            ));
            mark_failed(&mut projection, generation_id, &error.to_string())?;
            return Err(error);
        }
        mark_verified(
            &mut projection,
            generation_id,
            expected_count,
            expected_checksum,
        )?;
        faults.hit(ProjectionFaultPoint::Pr02AfterChecksum)?;

        let activated = if activate {
            activate_generation(
                &mut projection,
                generation_id,
                kind,
                domain,
                watermark,
                built_at_unix_ms,
                faults,
            )?
        } else {
            false
        };
        Ok(GenerationReceipt {
            metadata: read_generation_metadata(&projection, generation_id)?,
            activated,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum BuildTarget {
    Latest,
    AsKnown(u64),
}

#[derive(Debug)]
enum SourceRecords {
    Graph(Vec<GraphSourceRecord>),
    Search(Vec<SearchSourceRecord>),
}

impl SourceRecords {
    fn len(&self) -> usize {
        match self {
            Self::Graph(records) => records.len(),
            Self::Search(records) => records.len(),
        }
    }

    fn canonical_records(&self) -> Vec<Vec<u8>> {
        match self {
            Self::Graph(records) => records
                .iter()
                .map(GraphSourceRecord::canonical_bytes)
                .collect(),
            Self::Search(records) => records
                .iter()
                .map(SearchSourceRecord::canonical_bytes)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Watermark {
    pub(crate) source_accept_seq: u64,
    pub(crate) source_outbox_seq: u64,
}

/// Creates or verifies the disposable projection database and executable FTS5
/// unicode61/trigram availability.
pub fn migrate_projection_database(path: &Path) -> ProjectionResult<()> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut connection = Connection::open_with_flags(path, flags)?;
    let application_id = pragma_i64(&connection, "application_id")?;
    let user_version = pragma_i64(&connection, "user_version")?;
    if application_id == i64::from(PROJECTION_APPLICATION_ID)
        && user_version == i64::from(PROJECTION_DATABASE_VERSION)
    {
        configure_projection_writer(&connection)?;
        verify_projection_schema(&connection)?;
        verify_fts5(&connection)?;
        return Ok(());
    }
    if application_id != 0 || user_version != 0 || user_object_count(&connection)? != 0 {
        return Err(ProjectionError::InvalidCanonicalStore {
            component: "projection sidecar identity",
            expected: format!(
                "new database or application_id={PROJECTION_APPLICATION_ID}/user_version={PROJECTION_DATABASE_VERSION}"
            ),
            actual: format!("application_id={application_id}/user_version={user_version}"),
        });
    }
    configure_projection_writer(&connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    transaction.execute_batch(MIGRATION_0002_SQL)?;
    transaction.commit()?;
    verify_projection_schema(&connection)?;
    verify_fts5(&connection)
}

fn write_records<F: ProjectionFaultInjector + ?Sized>(
    projection: &mut Connection,
    kind: ProjectionKind,
    generation_id: GenerationId,
    source_records: &SourceRecords,
    faults: &F,
) -> ProjectionResult<()> {
    let transaction = projection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let midpoint = source_records.len().div_ceil(2).max(1);
    let hit_midpoint = |written: usize, _total: usize| {
        if written == midpoint {
            faults.hit(ProjectionFaultPoint::Pr01MidWrite)
        } else {
            Ok(())
        }
    };
    if source_records.len() == 0 {
        faults.hit(ProjectionFaultPoint::Pr01MidWrite)?;
    }
    match source_records {
        SourceRecords::Graph(records) => {
            write_graph_records(&transaction, generation_id, records, hit_midpoint)?;
        }
        SourceRecords::Search(records) => {
            write_search_records(&transaction, kind, generation_id, records, hit_midpoint)?;
        }
    }
    transaction.commit()?;
    Ok(())
}

fn create_building_generation(
    projection: &mut Connection,
    kind: ProjectionKind,
    domain: DomainId,
    watermark: Watermark,
    builder_binary_digest: ContentDigest,
    effective_config_hash: ContentDigest,
    built_at_unix_ms: i64,
) -> ProjectionResult<GenerationId> {
    let transaction = projection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let generation_seq = transaction.query_row(
        "SELECT coalesce(max(generation_seq), 0) + 1 FROM projection_generation",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    if generation_seq < 1 {
        return Err(ProjectionError::Corrupt(
            "generation sequence did not advance".to_owned(),
        ));
    }
    let generation_id = derive_generation_id(
        generation_seq,
        kind,
        domain,
        built_at_unix_ms,
        builder_binary_digest,
        effective_config_hash,
    );
    transaction.execute(
        concat!(
            "INSERT INTO projection_generation (generation_seq, generation_id, projection_kind, ",
            "schema_version, builder_binary_digest, algorithm_version, tokenizer_version, ",
            "effective_config_hash, source_accept_seq, source_outbox_seq, security_domain, ",
            "built_at_unix_ms, state) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'BUILDING')"
        ),
        params![
            generation_seq,
            generation_id.as_bytes().as_slice(),
            kind.as_str(),
            i64::from(PROJECTION_SCHEMA_VERSION),
            builder_binary_digest.as_bytes().as_slice(),
            PROJECTION_ALGORITHM_VERSION,
            kind.tokenizer_version(),
            effective_config_hash.as_bytes().as_slice(),
            checked_i64(watermark.source_accept_seq)?,
            checked_i64(watermark.source_outbox_seq)?,
            domain.as_bytes().as_slice(),
            built_at_unix_ms,
        ],
    )?;
    transaction.commit()?;
    Ok(generation_id)
}

fn mark_failed(
    projection: &mut Connection,
    generation_id: GenerationId,
    reason: &str,
) -> ProjectionResult<()> {
    let changed = projection.execute(
        concat!(
            "UPDATE projection_generation SET state = 'FAILED', failure_reason = ?2 ",
            "WHERE generation_id = ?1 AND state = 'BUILDING'"
        ),
        params![generation_id.as_bytes().as_slice(), reason],
    )?;
    if changed != 1 {
        return Err(ProjectionError::Corrupt(
            "generation was not BUILDING while marking it failed".to_owned(),
        ));
    }
    Ok(())
}

fn mark_verified(
    projection: &mut Connection,
    generation_id: GenerationId,
    record_count: u64,
    checksum: ContentDigest,
) -> ProjectionResult<()> {
    let changed = projection.execute(
        concat!(
            "UPDATE projection_generation SET state = 'VERIFIED', record_count = ?2, ",
            "canonical_checksum = ?3, failure_reason = NULL ",
            "WHERE generation_id = ?1 AND state = 'BUILDING'"
        ),
        params![
            generation_id.as_bytes().as_slice(),
            checked_i64(record_count)?,
            checksum.as_bytes().as_slice()
        ],
    )?;
    if changed != 1 {
        return Err(ProjectionError::Corrupt(
            "generation was not BUILDING during verification".to_owned(),
        ));
    }
    Ok(())
}

fn activate_generation<F: ProjectionFaultInjector + ?Sized>(
    projection: &mut Connection,
    generation_id: GenerationId,
    kind: ProjectionKind,
    domain: DomainId,
    watermark: Watermark,
    activated_at_unix_ms: i64,
    faults: &F,
) -> ProjectionResult<bool> {
    let transaction = projection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let state = transaction.query_row(
        concat!(
            "SELECT state FROM projection_generation WHERE generation_id = ?1 ",
            "AND projection_kind = ?2 AND security_domain = ?3"
        ),
        params![
            generation_id.as_bytes().as_slice(),
            kind.as_str(),
            domain.as_bytes().as_slice()
        ],
        |row| row.get::<_, String>(0),
    )?;
    if state != GenerationState::Verified.as_str() {
        return Err(ProjectionError::Corrupt(
            "only a VERIFIED generation may be activated".to_owned(),
        ));
    }
    let active_watermark = transaction
        .query_row(
            concat!(
                "SELECT source_accept_seq, source_outbox_seq FROM projection_active ",
                "WHERE projection_kind = ?1 AND security_domain = ?2"
            ),
            params![kind.as_str(), domain.as_bytes().as_slice()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let cursor_watermark = transaction
        .query_row(
            concat!(
                "SELECT source_accept_seq, last_outbox_seq FROM projection_cursor ",
                "WHERE projection_kind = ?1 AND security_domain = ?2"
            ),
            params![kind.as_str(), domain.as_bytes().as_slice()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    match (active_watermark, cursor_watermark) {
        (None, None) => {}
        (Some(active), Some(cursor)) if active == cursor => {
            let active_accept = nonnegative_u64(active.0, "active source accept_seq")?;
            let active_outbox = nonnegative_u64(active.1, "active source outbox_seq")?;
            if active_outbox > watermark.source_outbox_seq
                || (active_outbox == watermark.source_outbox_seq
                    && active_accept > watermark.source_accept_seq)
            {
                return Ok(false);
            }
        }
        _ => {
            return Err(ProjectionError::Corrupt(
                "active generation pointer and cursor do not agree".to_owned(),
            ));
        }
    }
    transaction.execute(
        concat!(
            "INSERT INTO projection_active (projection_kind, security_domain, generation_id, ",
            "source_accept_seq, source_outbox_seq, activated_at_unix_ms) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6) ",
            "ON CONFLICT(projection_kind, security_domain) DO UPDATE SET ",
            "generation_id = excluded.generation_id, source_accept_seq = excluded.source_accept_seq, ",
            "source_outbox_seq = excluded.source_outbox_seq, ",
            "activated_at_unix_ms = excluded.activated_at_unix_ms"
        ),
        params![
            kind.as_str(),
            domain.as_bytes().as_slice(),
            generation_id.as_bytes().as_slice(),
            checked_i64(watermark.source_accept_seq)?,
            checked_i64(watermark.source_outbox_seq)?,
            activated_at_unix_ms,
        ],
    )?;
    faults.hit(ProjectionFaultPoint::Pr03DuringActivation)?;
    transaction.execute(
        concat!(
            "INSERT INTO projection_cursor (projection_kind, security_domain, last_outbox_seq, ",
            "source_accept_seq, updated_at_unix_ms) VALUES (?1, ?2, ?3, ?4, ?5) ",
            "ON CONFLICT(projection_kind, security_domain) DO UPDATE SET ",
            "last_outbox_seq = excluded.last_outbox_seq, ",
            "source_accept_seq = excluded.source_accept_seq, ",
            "updated_at_unix_ms = excluded.updated_at_unix_ms"
        ),
        params![
            kind.as_str(),
            domain.as_bytes().as_slice(),
            checked_i64(watermark.source_outbox_seq)?,
            checked_i64(watermark.source_accept_seq)?,
            activated_at_unix_ms,
        ],
    )?;
    transaction.commit()?;
    Ok(true)
}

pub(crate) fn open_canonical_reader(path: &Path) -> ProjectionResult<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)?;
    connection.execute_batch(
        "PRAGMA query_only = ON; PRAGMA foreign_keys = ON; PRAGMA trusted_schema = OFF; \
         PRAGMA busy_timeout = 250; PRAGMA temp_store = MEMORY;",
    )?;
    let application_id = pragma_i64(&connection, "application_id")?;
    let user_version = pragma_i64(&connection, "user_version")?;
    exact_canonical(
        "application_id",
        i64::from(SQLITE_APPLICATION_ID),
        application_id,
    )?;
    exact_canonical(
        "user_version",
        i64::from(STORE_SCHEMA_VERSION),
        user_version,
    )?;
    Ok(connection)
}

pub(crate) fn open_projection_reader(path: &Path) -> ProjectionResult<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)?;
    connection.execute_batch(
        "PRAGMA query_only = ON; PRAGMA foreign_keys = ON; PRAGMA trusted_schema = OFF; \
         PRAGMA busy_timeout = 250; PRAGMA temp_store = MEMORY;",
    )?;
    verify_projection_identity(&connection)?;
    Ok(connection)
}

fn open_projection_writer(path: &Path) -> ProjectionResult<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)?;
    verify_projection_identity(&connection)?;
    configure_projection_writer(&connection)?;
    Ok(connection)
}

fn configure_projection_writer(connection: &Connection) -> ProjectionResult<()> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON; \
         PRAGMA trusted_schema = OFF; PRAGMA busy_timeout = 250; PRAGMA temp_store = MEMORY; \
         PRAGMA query_only = OFF;",
    )?;
    Ok(())
}

fn verify_projection_identity(connection: &Connection) -> ProjectionResult<()> {
    let application_id = pragma_i64(connection, "application_id")?;
    let user_version = pragma_i64(connection, "user_version")?;
    if application_id != i64::from(PROJECTION_APPLICATION_ID)
        || user_version != i64::from(PROJECTION_DATABASE_VERSION)
    {
        return Err(ProjectionError::InvalidCanonicalStore {
            component: "projection sidecar identity",
            expected: format!(
                "application_id={PROJECTION_APPLICATION_ID}/user_version={PROJECTION_DATABASE_VERSION}"
            ),
            actual: format!("application_id={application_id}/user_version={user_version}"),
        });
    }
    Ok(())
}

fn verify_projection_schema(connection: &Connection) -> ProjectionResult<()> {
    verify_projection_identity(connection)?;
    for name in [
        "projection_generation",
        "projection_active",
        "projection_cursor",
        "projection_graph_edge",
        "projection_graph_edge_evidence",
        "projection_search_content",
        "projection_exact_symbol",
        "projection_search_unicode",
        "projection_search_trigram",
    ] {
        let count = connection.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )?;
        if count != 1 {
            return Err(ProjectionError::Corrupt(format!(
                "required projection object {name} count is {count}"
            )));
        }
    }
    Ok(())
}

fn verify_fts5(connection: &Connection) -> ProjectionResult<()> {
    let enabled = connection
        .query_row(
            "SELECT 1 FROM pragma_compile_options WHERE compile_options = 'ENABLE_FTS5'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if enabled != Some(1) {
        return Err(ProjectionError::UnsupportedSqliteBuild(
            "ENABLE_FTS5 compile option is absent",
        ));
    }
    connection.execute_batch(concat!(
        "SAVEPOINT projection_fts_probe;",
        "CREATE VIRTUAL TABLE temp.projection_unicode_probe USING fts5(body, tokenize='unicode61');",
        "CREATE VIRTUAL TABLE temp.projection_trigram_probe USING fts5(body, tokenize='trigram');",
        "INSERT INTO temp.projection_unicode_probe(body) VALUES ('합성 트랜잭션 probe');",
        "INSERT INTO temp.projection_trigram_probe(body) VALUES ('OrderService.updateStatus');"
    ))?;
    let unicode_matches = connection.query_row(
        "SELECT count(*) FROM temp.projection_unicode_probe WHERE body MATCH '트랜잭션'",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    let trigram_matches = connection.query_row(
        "SELECT count(*) FROM temp.projection_trigram_probe WHERE body MATCH 'updateStatus'",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    connection.execute_batch(concat!(
        "DROP TABLE temp.projection_unicode_probe;",
        "DROP TABLE temp.projection_trigram_probe;",
        "RELEASE projection_fts_probe;"
    ))?;
    if unicode_matches != 1 || trigram_matches != 1 {
        return Err(ProjectionError::UnsupportedSqliteBuild(
            "FTS5 unicode61/trigram executable probe failed",
        ));
    }
    Ok(())
}

pub(crate) fn latest_watermark(connection: &Connection) -> ProjectionResult<Watermark> {
    let row = connection
        .query_row(
            concat!(
                "SELECT accept_seq_end, outbox_seq FROM projection_outbox ",
                "ORDER BY outbox_seq DESC LIMIT 1"
            ),
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    row.map_or(
        Ok(Watermark {
            source_accept_seq: 0,
            source_outbox_seq: 0,
        }),
        |row| {
            Ok(Watermark {
                source_accept_seq: nonnegative_u64(row.0, "latest source accept_seq")?,
                source_outbox_seq: nonnegative_u64(row.1, "latest source outbox_seq")?,
            })
        },
    )
}

fn outbox_at_or_before(connection: &Connection, watermark: u64) -> ProjectionResult<u64> {
    let value = connection.query_row(
        concat!(
            "SELECT coalesce(max(outbox_seq), 0) FROM projection_outbox ",
            "WHERE accept_seq_end <= ?1"
        ),
        [checked_i64(watermark)?],
        |row| row.get::<_, i64>(0),
    )?;
    nonnegative_u64(value, "historical source outbox_seq")
}

pub(crate) fn read_active_generation(
    connection: &Connection,
    kind: ProjectionKind,
    domain: DomainId,
) -> ProjectionResult<Option<ActiveGeneration>> {
    type Raw = (
        Vec<u8>,
        i64,
        i64,
        Option<i64>,
        Option<Vec<u8>>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    );
    let row: Option<Raw> = connection
        .query_row(
            concat!(
                "SELECT a.generation_id, a.source_accept_seq, a.source_outbox_seq, ",
                "g.record_count, g.canonical_checksum, g.state, g.source_accept_seq, ",
                "g.source_outbox_seq, c.source_accept_seq, c.last_outbox_seq ",
                "FROM projection_active a LEFT JOIN projection_generation g ",
                "ON g.generation_id = a.generation_id ",
                "AND g.projection_kind = a.projection_kind ",
                "AND g.security_domain = a.security_domain ",
                "LEFT JOIN projection_cursor c ON c.projection_kind = a.projection_kind ",
                "AND c.security_domain = a.security_domain ",
                "WHERE a.projection_kind = ?1 AND a.security_domain = ?2"
            ),
            params![kind.as_str(), domain.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            },
        )
        .optional()?;
    row.map(|row| {
        let (
            record_count,
            checksum,
            state,
            generation_accept,
            generation_outbox,
            cursor_accept,
            cursor_outbox,
        ) = match (row.3, row.4, row.5, row.6, row.7, row.8, row.9) {
            (
                Some(count),
                Some(checksum),
                Some(state),
                Some(generation_accept),
                Some(generation_outbox),
                Some(cursor_accept),
                Some(cursor_outbox),
            ) => (
                count,
                checksum,
                state,
                generation_accept,
                generation_outbox,
                cursor_accept,
                cursor_outbox,
            ),
            _ => {
                return Err(ProjectionError::Corrupt(
                    "active pointer is missing VERIFIED generation or cursor authority".to_owned(),
                ));
            }
        };
        if state != GenerationState::Verified.as_str() {
            return Err(ProjectionError::Corrupt(
                "active pointer references a non-VERIFIED generation".to_owned(),
            ));
        }
        if (row.1, row.2) != (generation_accept, generation_outbox)
            || (row.1, row.2) != (cursor_accept, cursor_outbox)
        {
            return Err(ProjectionError::Corrupt(
                "active generation, generation metadata, and cursor watermarks disagree".to_owned(),
            ));
        }
        Ok(ActiveGeneration {
            generation_id: GenerationId::from_bytes(fixed_bytes(
                row.0,
                "active generation identifier",
            )?),
            kind,
            security_domain: domain,
            source_accept_seq: nonnegative_u64(row.1, "active source accept_seq")?,
            source_outbox_seq: nonnegative_u64(row.2, "active source outbox_seq")?,
            record_count: nonnegative_u64(record_count, "active record count")?,
            canonical_checksum: ContentDigest::from_sha256_bytes(fixed_bytes(
                checksum,
                "active canonical checksum",
            )?),
        })
    })
    .transpose()
}

fn read_generation_metadata(
    connection: &Connection,
    generation_id: GenerationId,
) -> ProjectionResult<GenerationMetadata> {
    type Raw = (
        String,
        i64,
        Vec<u8>,
        String,
        String,
        Vec<u8>,
        i64,
        i64,
        Vec<u8>,
        i64,
        String,
        Option<i64>,
        Option<Vec<u8>>,
    );
    let row: Raw = connection.query_row(
        concat!(
            "SELECT projection_kind, schema_version, builder_binary_digest, algorithm_version, ",
            "tokenizer_version, effective_config_hash, source_accept_seq, source_outbox_seq, ",
            "security_domain, built_at_unix_ms, state, record_count, canonical_checksum ",
            "FROM projection_generation WHERE generation_id = ?1"
        ),
        [generation_id.as_bytes().as_slice()],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
            ))
        },
    )?;
    Ok(GenerationMetadata {
        generation_id,
        kind: ProjectionKind::parse(&row.0)
            .ok_or_else(|| ProjectionError::Corrupt("unknown projection kind".to_owned()))?,
        schema_version: u32::try_from(row.1).map_err(|_| {
            ProjectionError::Corrupt("invalid projection schema version".to_owned())
        })?,
        builder_binary_digest: ContentDigest::from_sha256_bytes(fixed_bytes(
            row.2,
            "builder binary digest",
        )?),
        algorithm_version: row.3,
        tokenizer_version: row.4,
        effective_config_hash: ContentDigest::from_sha256_bytes(fixed_bytes(
            row.5,
            "effective config hash",
        )?),
        source_accept_seq: nonnegative_u64(row.6, "generation source accept_seq")?,
        source_outbox_seq: nonnegative_u64(row.7, "generation source outbox_seq")?,
        security_domain: id_from_bytes(row.8, "generation security domain")?,
        built_at_unix_ms: row.9,
        state: GenerationState::parse(&row.10)
            .ok_or_else(|| ProjectionError::Corrupt("unknown generation state".to_owned()))?,
        record_count: row
            .11
            .map(|value| nonnegative_u64(value, "generation record count"))
            .transpose()?,
        canonical_checksum: row
            .12
            .map(|value| {
                fixed_bytes(value, "generation checksum").map(ContentDigest::from_sha256_bytes)
            })
            .transpose()?,
    })
}

fn derive_generation_id(
    generation_seq: i64,
    kind: ProjectionKind,
    domain: DomainId,
    built_at_unix_ms: i64,
    builder_binary_digest: ContentDigest,
    effective_config_hash: ContentDigest,
) -> GenerationId {
    let mut material = Vec::new();
    material.extend_from_slice(b"ACADEMIC_PROJECTION_GENERATION_V1");
    material.extend_from_slice(&generation_seq.to_be_bytes());
    material.extend_from_slice(kind.as_str().as_bytes());
    material.extend_from_slice(domain.as_bytes());
    material.extend_from_slice(&built_at_unix_ms.to_be_bytes());
    material.extend_from_slice(builder_binary_digest.as_bytes());
    material.extend_from_slice(effective_config_hash.as_bytes());
    let digest = ContentDigest::sha256(&material);
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    GenerationId::from_bytes(bytes)
}

fn exact_canonical(component: &'static str, expected: i64, actual: i64) -> ProjectionResult<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(ProjectionError::InvalidCanonicalStore {
            component,
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn pragma_i64(connection: &Connection, name: &'static str) -> ProjectionResult<i64> {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .map_err(ProjectionError::from)
}

fn user_object_count(connection: &Connection) -> ProjectionResult<i64> {
    connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(ProjectionError::from)
}

fn checked_i64(value: u64) -> ProjectionResult<i64> {
    i64::try_from(value).map_err(|_| ProjectionError::IntegerOverflow(value))
}

fn nonnegative_u64(value: i64, reason: &'static str) -> ProjectionResult<u64> {
    u64::try_from(value).map_err(|_| ProjectionError::Corrupt(reason.to_owned()))
}

fn fixed_bytes<const N: usize>(bytes: Vec<u8>, reason: &'static str) -> ProjectionResult<[u8; N]> {
    bytes
        .try_into()
        .map_err(|_| ProjectionError::Corrupt(reason.to_owned()))
}

fn id_from_bytes<T>(bytes: Vec<u8>, reason: &'static str) -> ProjectionResult<T>
where
    T: FromStr<Err = DomainError>,
{
    let bytes = fixed_bytes::<16>(bytes, reason)?;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
    .parse()
    .map_err(ProjectionError::Domain)
}

fn unix_time_millis() -> ProjectionResult<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProjectionError::SystemClock)?;
    i64::try_from(duration.as_millis()).map_err(|_| ProjectionError::SystemClock)
}
