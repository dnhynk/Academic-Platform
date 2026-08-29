//! Fixed-watermark projection builder and atomic activation runner.

use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use academic_domain::{ContentDigest, DomainError, DomainId, TimestampMillis};
use academic_store::{
    connection::{ReaderConnection, open_reader},
    error::StoreError,
    queries::{ProjectionSnapshotRequest, QueryError, projection_source_snapshot},
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::{
    PROJECTION_SCHEMA_VERSION,
    checksum::order_stable_checksum,
    fts::{
        SearchSourceRecord, load_search_sources, persisted_search_canonical_records,
        write_search_records,
    },
    generation::{
        ActiveGeneration, GenerationId, GenerationMetadata, GenerationState, ProjectionCoordinates,
        ProjectionKind,
    },
    graph::{
        GraphSourceRecord, load_graph_sources, persisted_graph_canonical_records,
        write_graph_records,
    },
    resolution::{CANONICAL_RESOLVER_VERSION, PredicatePolicies},
};

/// Application identity for the disposable projection sidecar (`ACPR`).
pub const PROJECTION_APPLICATION_ID: u32 = 0x4143_5052;
/// Source-ledger-bound physical sidecar version.
pub const PROJECTION_DATABASE_VERSION: u32 = 3;
/// Audited-base v2 migration, retained only for format detection and tests.
pub const MIGRATION_0002_SQL: &str =
    include_str!("../../../migrations/store/0002_phase1_projections.sql");
/// Exact pre-RF immediate-parent v2 migration, retained as a compatibility fixture.
const PARENT_MIGRATION_0002_SQL: &str =
    include_str!("../../../migrations/store/fixtures/0002_phase1_projections_parent.sql");
/// Current source-ledger-bound projection-sidecar migration.
pub const MIGRATION_0003_SQL: &str =
    include_str!("../../../migrations/store/0003_phase1_projections.sql");
/// Source-ledger-bound, coordinate-selectable Phase 1 builder algorithm identifier.
pub const PROJECTION_ALGORITHM_VERSION: &str = "phase1-full-generation-v3";
const PREVIOUS_PROJECTION_ALGORITHM_VERSION: &str = "phase1-full-generation-v2";
// Only `CREATE` applies SQLite's reserved-prefix rejection, so a `sqlite_`
// prefixed object written directly into `sqlite_schema` loads like any other
// and the prefix says nothing about ownership. The exclusion is the exact set
// of objects SQLite creates itself, and every other object is a user object
// regardless of its type or name — exactly as the canonical store admission
// does.
const USER_SCHEMA_OBJECT_PREDICATE: &str = "NOT ( \
     (type = 'table' AND name IN \
      ('sqlite_sequence', 'sqlite_stat1', 'sqlite_stat2', 'sqlite_stat3', 'sqlite_stat4')) \
     OR (type = 'index' AND name GLOB 'sqlite_autoindex_*'))";

mod fault_boundary {
    use super::{ProjectionResult, fmt};

    /// Deterministic projection failpoints available only through the explicit
    /// non-default fault-injection feature.
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
        #[cfg(feature = "phase1-fault-injection")]
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Pr01MidWrite => "PR01",
                Self::Pr02AfterChecksum => "PR02",
                Self::Pr03DuringActivation => "PR03",
            }
        }
    }

    /// Sidecar-only verification corruptions for the explicit test harness.
    #[cfg(feature = "phase1-fault-injection")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProjectionVerificationCorruption {
        /// Temporarily replace the selected named FTS table with the wrong tokenizer.
        WrongNamedTokenizer,
        /// Temporarily remove one candidate FTS row.
        MissingFtsRow,
        /// Temporarily replace candidate ordering material.
        WrongPersistedTiebreaker,
    }

    pub(crate) trait ProjectionFaultBehavior: fmt::Debug {
        fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()>;

        #[cfg(feature = "phase1-fault-injection")]
        fn verification_corruption(&self) -> Option<ProjectionVerificationCorruption> {
            None
        }
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub(crate) struct NoProjectionFault;

    impl ProjectionFaultBehavior for NoProjectionFault {
        fn hit(&self, _point: ProjectionFaultPoint) -> ProjectionResult<()> {
            Ok(())
        }
    }

    /// Test-only behavior boundary compiled only for the explicit fault harness.
    #[cfg(feature = "phase1-fault-injection")]
    pub trait ProjectionFaultInjector: fmt::Debug {
        /// Called only at one of the three fixed J1 ordering boundaries.
        fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()>;

        /// Requests one isolated sidecar mutation before VERIFIED.
        fn verification_corruption(&self) -> Option<ProjectionVerificationCorruption> {
            None
        }
    }

    #[cfg(feature = "phase1-fault-injection")]
    impl<T> ProjectionFaultBehavior for T
    where
        T: ProjectionFaultInjector + ?Sized,
    {
        fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()> {
            ProjectionFaultInjector::hit(self, point)
        }

        fn verification_corruption(&self) -> Option<ProjectionVerificationCorruption> {
            ProjectionFaultInjector::verification_corruption(self)
        }
    }
}

use fault_boundary::{NoProjectionFault, ProjectionFaultBehavior};
#[cfg(feature = "phase1-fault-injection")]
pub use fault_boundary::{
    ProjectionFaultInjector, ProjectionFaultPoint, ProjectionVerificationCorruption,
};

/// Projection migration, source-integrity, build, or query failure.
#[derive(Debug)]
pub enum ProjectionError {
    Sqlite(rusqlite::Error),
    Io(io::Error),
    Store(StoreError),
    CanonicalQuery(QueryError),
    Domain(DomainError),
    Corrupt(String),
    InvalidCanonicalStore {
        component: &'static str,
        expected: String,
        actual: String,
    },
    UnsupportedProjectionFormat {
        application_id: i64,
        user_version: i64,
        reason: &'static str,
    },
    UnsupportedSqliteBuild(&'static str),
    IntegerOverflow(u64),
    InvalidWatermark {
        requested: u64,
        latest: u64,
    },
    InvalidQuery(&'static str),
    InvalidPolicyRegistry(String),
    MissingPredicatePolicy(String),
    AuthorityMismatch(String),
    #[cfg(feature = "phase1-fault-injection")]
    InjectedFault(ProjectionFaultPoint),
    SystemClock,
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "projection SQLite error: {error}"),
            Self::Io(error) => write!(formatter, "projection sidecar I/O error: {error}"),
            Self::Store(error) => write!(formatter, "canonical store open error: {error}"),
            Self::CanonicalQuery(error) => {
                write!(formatter, "canonical projection snapshot error: {error}")
            }
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
            Self::UnsupportedProjectionFormat {
                application_id,
                user_version,
                reason,
            } => write!(
                formatter,
                "unsupported projection sidecar format: application_id={application_id}/user_version={user_version}: {reason}"
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
            Self::InvalidPolicyRegistry(reason) => {
                write!(formatter, "invalid projection policy registry: {reason}")
            }
            Self::MissingPredicatePolicy(predicate) => write!(
                formatter,
                "projection policy registry has no entry for predicate {predicate}"
            ),
            Self::AuthorityMismatch(reason) => {
                write!(formatter, "projection authority mismatch: {reason}")
            }
            #[cfg(feature = "phase1-fault-injection")]
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
            Self::Io(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::CanonicalQuery(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::Corrupt(_)
            | Self::InvalidCanonicalStore { .. }
            | Self::UnsupportedProjectionFormat { .. }
            | Self::UnsupportedSqliteBuild(_)
            | Self::IntegerOverflow(_)
            | Self::InvalidWatermark { .. }
            | Self::InvalidQuery(_)
            | Self::InvalidPolicyRegistry(_)
            | Self::MissingPredicatePolicy(_)
            | Self::AuthorityMismatch(_)
            | Self::SystemClock => None,
            #[cfg(feature = "phase1-fault-injection")]
            Self::InjectedFault(_) => None,
        }
    }
}

impl From<rusqlite::Error> for ProjectionError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<io::Error> for ProjectionError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<StoreError> for ProjectionError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<QueryError> for ProjectionError {
    fn from(error: QueryError) -> Self {
        match error {
            QueryError::KnownAtBeyondHead { requested, latest } => {
                Self::InvalidWatermark { requested, latest }
            }
            QueryError::MissingPredicatePolicy(predicate) => {
                Self::MissingPredicatePolicy(predicate)
            }
            other => Self::CanonicalQuery(other),
        }
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

    /// Resolves and atomically activates one generation at explicit known/valid
    /// coordinates under the exact versioned predicate policy registry.
    pub fn rebuild_at(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
    ) -> ProjectionResult<GenerationReceipt> {
        self.rebuild(
            kind,
            domain,
            coordinates,
            policies,
            true,
            &NoProjectionFault,
        )
    }

    /// Builds a VERIFIED, inactive generation at explicit bitemporal coordinates.
    pub fn build_inactive_at(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
    ) -> ProjectionResult<GenerationReceipt> {
        self.rebuild(
            kind,
            domain,
            coordinates,
            policies,
            false,
            &NoProjectionFault,
        )
    }

    /// Fault-harness entrypoint. The trait has no process-exit implementation in
    /// production code; integration tests provide one in a child process.
    #[cfg(feature = "phase1-fault-injection")]
    pub fn rebuild_at_with_faults<F: ProjectionFaultInjector + ?Sized>(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        faults: &F,
    ) -> ProjectionResult<GenerationReceipt> {
        self.rebuild(kind, domain, coordinates, policies, true, faults)
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

    /// Reads the active generation selected for one projection kind and domain.
    ///
    /// Deep doctor reports projection lag against the canonical outbox head, so
    /// this watermark read belongs to the product surface rather than the fault
    /// harness. It opens the sidecar read-only and returns owned metadata,
    /// never a connection.
    pub fn active_generation(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
    ) -> ProjectionResult<Option<ActiveGeneration>> {
        let connection = open_projection_reader(&self.projection_database_path)?;
        read_active_generation(&connection, kind, domain)
    }

    /// Reads immutable generation metadata for the explicit fault harness.
    #[cfg(feature = "phase1-fault-injection")]
    pub fn generation(&self, generation_id: GenerationId) -> ProjectionResult<GenerationMetadata> {
        let connection = open_projection_reader(&self.projection_database_path)?;
        read_generation_metadata(&connection, generation_id)
    }

    /// Reads active/cursor authority for adversarial tests without exposing SQL.
    #[cfg(feature = "phase1-fault-injection")]
    pub fn audit_active_generation(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
    ) -> ProjectionResult<Option<ActiveGeneration>> {
        let connection = open_projection_writer(&self.projection_database_path)?;
        read_active_generation(&connection, kind, domain)
    }

    /// Counts lifecycle rows for adversarial tests without exposing the sidecar connection.
    #[cfg(feature = "phase1-fault-injection")]
    pub fn audit_generation_state_count(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        state: GenerationState,
        excluding: Option<GenerationId>,
    ) -> ProjectionResult<u64> {
        let connection = open_projection_writer(&self.projection_database_path)?;
        let count = match excluding {
            Some(generation_id) => connection.query_row(
                concat!(
                    "SELECT count(*) FROM projection_generation WHERE projection_kind = ?1 ",
                    "AND security_domain = ?2 AND state = ?3 AND generation_id <> ?4"
                ),
                params![
                    kind.as_str(),
                    domain.as_bytes().as_slice(),
                    state.as_str(),
                    generation_id.as_bytes().as_slice()
                ],
                |row| row.get::<_, i64>(0),
            )?,
            None => connection.query_row(
                concat!(
                    "SELECT count(*) FROM projection_generation WHERE projection_kind = ?1 ",
                    "AND security_domain = ?2 AND state = ?3"
                ),
                params![kind.as_str(), domain.as_bytes().as_slice(), state.as_str()],
                |row| row.get::<_, i64>(0),
            )?,
        };
        nonnegative_u64(count, "projection generation state count")
    }

    fn rebuild<F: ProjectionFaultBehavior + ?Sized>(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        activate: bool,
        faults: &F,
    ) -> ProjectionResult<GenerationReceipt> {
        let mut canonical = open_reader(&self.canonical_database_path)?;
        let snapshot = projection_source_snapshot(
            &mut canonical,
            &ProjectionSnapshotRequest {
                domain_id: domain,
                valid_at: coordinates.valid_at,
                known_at_accept_seq: coordinates.known_at_accept_seq,
                predicate_policies: policies.entries(),
            },
        )?;
        let watermark = Watermark {
            source_accept_seq: coordinates.known_at_accept_seq,
            source_outbox_seq: snapshot.source_outbox_seq,
            source_ledger_digest: snapshot.source_ledger_digest,
        };
        let source_records = match kind {
            ProjectionKind::Graph => {
                SourceRecords::Graph(load_graph_sources(&snapshot.resolved_claims, domain)?)
            }
            ProjectionKind::Unicode61 | ProjectionKind::Trigram => {
                SourceRecords::Search(load_search_sources(
                    &snapshot.resolved_claims,
                    &snapshot.evidence_locators,
                    domain,
                )?)
            }
        };

        let built_at_unix_ms = unix_time_millis()?;
        let mut projection = open_projection_writer(&self.projection_database_path)?;
        let authority = BuildAuthority {
            kind,
            domain,
            watermark,
            coordinates,
            policies,
        };
        let generation_id = create_building_generation(
            &mut projection,
            &authority,
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

        let expected_records = source_records.verification_records();
        let expected_count = u64::try_from(expected_records.len())
            .map_err(|_| ProjectionError::Corrupt("projection record count overflow".to_owned()))?;
        let expected_checksum = order_stable_checksum(expected_records);
        let verification = verify_candidate_generation(
            &mut projection,
            generation_id,
            kind,
            coordinates,
            watermark,
            policies,
            faults,
        );
        let persisted_records = match verification {
            Ok(records) => records,
            Err(error) => {
                mark_failed(&mut projection, generation_id, &error.to_string())?;
                return Err(error);
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
        faults.hit(fault_boundary::ProjectionFaultPoint::Pr02AfterChecksum)?;

        let activated = if activate {
            activate_generation(
                &mut projection,
                generation_id,
                &authority,
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

fn verify_candidate_generation<F: ProjectionFaultBehavior + ?Sized>(
    projection: &mut Connection,
    generation_id: GenerationId,
    kind: ProjectionKind,
    coordinates: ProjectionCoordinates,
    watermark: Watermark,
    policies: &PredicatePolicies,
    faults: &F,
) -> ProjectionResult<Vec<Vec<u8>>> {
    #[cfg(not(feature = "phase1-fault-injection"))]
    let _ = faults;

    #[cfg(feature = "phase1-fault-injection")]
    if let Some(corruption) = faults.verification_corruption() {
        let transaction = projection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        apply_verification_corruption(&transaction, generation_id, kind, corruption)?;
        let verification = read_and_verify_candidate(
            &transaction,
            generation_id,
            kind,
            coordinates,
            watermark,
            policies,
        );
        transaction.rollback()?;
        return match verification {
            Err(error) => Err(error),
            Ok(_) => Err(ProjectionError::Corrupt(
                "injected projection corruption unexpectedly passed verification".to_owned(),
            )),
        };
    }

    read_and_verify_candidate(
        projection,
        generation_id,
        kind,
        coordinates,
        watermark,
        policies,
    )
}

fn read_and_verify_candidate(
    connection: &Connection,
    generation_id: GenerationId,
    kind: ProjectionKind,
    coordinates: ProjectionCoordinates,
    watermark: Watermark,
    policies: &PredicatePolicies,
) -> ProjectionResult<Vec<Vec<u8>>> {
    let persisted_records = match kind {
        ProjectionKind::Graph => persisted_graph_canonical_records(connection, generation_id)?,
        ProjectionKind::Unicode61 | ProjectionKind::Trigram => {
            persisted_search_canonical_records(connection, generation_id)?
        }
    };
    verify_generation_storage(
        connection,
        generation_id,
        kind,
        coordinates,
        watermark,
        policies,
    )?;
    Ok(persisted_records)
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

    fn verification_records(&self) -> Vec<Vec<u8>> {
        match self {
            Self::Graph(records) => records
                .iter()
                .map(GraphSourceRecord::verification_bytes)
                .collect(),
            Self::Search(records) => records
                .iter()
                .map(SearchSourceRecord::verification_bytes)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Watermark {
    pub(crate) source_accept_seq: u64,
    pub(crate) source_outbox_seq: u64,
    pub(crate) source_ledger_digest: ContentDigest,
}

#[derive(Debug, Clone, Copy)]
struct BuildAuthority<'a> {
    kind: ProjectionKind,
    domain: DomainId,
    watermark: Watermark,
    coordinates: ProjectionCoordinates,
    policies: &'a PredicatePolicies,
}

/// Creates or verifies the disposable projection database and executable FTS5
/// unicode61/trigram availability.
pub fn migrate_projection_database(path: &Path) -> ProjectionResult<()> {
    // Atomic creation prevents a missing-path check from admitting a sidecar
    // that appeared before the first SQLite writer open.
    if create_sidecar_file(path)? {
        return initialize_projection_database(path);
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)?;
    let format = existing_projection_format(&connection)?;

    match format {
        ExistingFormat::Empty => initialize_admitted_empty_database(path, connection),
        ExistingFormat::ExactParentV2 => {
            drop(connection);
            replace_projection_database(path)
        }
        ExistingFormat::Current => {
            verify_projection_schema(&connection)?;
            verify_fts5(&connection)?;
            match persisted_generation_format(&connection)? {
                PersistedGenerationFormat::Current => {
                    configure_admitted_current_database(path, connection)
                }
                PersistedGenerationFormat::ExactPreviousAlgorithm => {
                    drop(connection);
                    replace_projection_database(path)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExistingFormat {
    Empty,
    Current,
    ExactParentV2,
}

fn existing_projection_format(connection: &Connection) -> ProjectionResult<ExistingFormat> {
    let application_id = pragma_i64(connection, "application_id")?;
    let user_version = pragma_i64(connection, "user_version")?;
    if application_id == 0 && user_version == 0 && user_object_count(connection)? == 0 {
        Ok(ExistingFormat::Empty)
    } else if application_id == i64::from(PROJECTION_APPLICATION_ID)
        && user_version == i64::from(PROJECTION_DATABASE_VERSION)
    {
        Ok(ExistingFormat::Current)
    } else if application_id == i64::from(PROJECTION_APPLICATION_ID) && user_version == 2 {
        let actual_fingerprint = projection_schema_fingerprint(connection)?;
        let is_known_v2 = [PARENT_MIGRATION_0002_SQL, MIGRATION_0002_SQL]
            .into_iter()
            .map(migration_schema_fingerprint)
            .collect::<ProjectionResult<Vec<_>>>()?
            .into_iter()
            .any(|expected| actual_fingerprint == expected);
        if is_known_v2 {
            Ok(ExistingFormat::ExactParentV2)
        } else {
            Err(ProjectionError::UnsupportedProjectionFormat {
                application_id,
                user_version,
                reason: "version 2 does not exactly match a known disposable projection format",
            })
        }
    } else {
        Err(ProjectionError::UnsupportedProjectionFormat {
            application_id,
            user_version,
            reason: "database is neither an empty sidecar nor a supported projection format",
        })
    }
}

fn create_sidecar_file(path: &Path) -> ProjectionResult<bool> {
    match fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => {
            drop(file);
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn replace_projection_database(path: &Path) -> ProjectionResult<()> {
    remove_disposable_sidecar(path)?;
    let created = create_sidecar_file(path)?;
    if !created {
        return Err(ProjectionError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "projection sidecar appeared during disposable replacement",
        )));
    }
    initialize_projection_database(path)
}

fn initialize_projection_database(path: &Path) -> ProjectionResult<()> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut connection = Connection::open_with_flags(path, flags)?;
    initialize_projection_connection(&mut connection)
}

fn initialize_admitted_empty_database(path: &Path, admission: Connection) -> ProjectionResult<()> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut writer = match Connection::open_with_flags(path, flags) {
        Ok(connection) => connection,
        Err(error) => {
            drop(admission);
            return Err(error.into());
        }
    };
    let result = match existing_projection_format(&writer)? {
        ExistingFormat::Empty => initialize_projection_connection(&mut writer),
        ExistingFormat::Current | ExistingFormat::ExactParentV2 => Err(ProjectionError::Corrupt(
            "projection sidecar changed during empty-sidecar admission".to_owned(),
        )),
    };
    finish_writer_handoff(admission, writer, result)
}

fn initialize_projection_connection(connection: &mut Connection) -> ProjectionResult<()> {
    configure_projection_writer(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    transaction.execute_batch(MIGRATION_0003_SQL)?;
    transaction.commit()?;
    verify_projection_schema(connection)?;
    verify_fts5(connection)
}

fn configure_admitted_current_database(path: &Path, admission: Connection) -> ProjectionResult<()> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let writer = match Connection::open_with_flags(path, flags) {
        Ok(connection) => connection,
        Err(error) => {
            drop(admission);
            return Err(error.into());
        }
    };

    // Keep the read-only admission connection alive through writer
    // revalidation. On a failed handoff the writer is therefore not the last
    // connection that can checkpoint a recovered WAL.
    let result = (|| {
        if existing_projection_format(&writer)? != ExistingFormat::Current {
            return Err(ProjectionError::Corrupt(
                "projection sidecar changed during current-format admission".to_owned(),
            ));
        }
        verify_projection_schema(&writer)?;
        verify_fts5(&writer)?;
        if persisted_generation_format(&writer)? != PersistedGenerationFormat::Current {
            return Err(ProjectionError::Corrupt(
                "projection sidecar generation changed during current-format admission".to_owned(),
            ));
        }
        configure_projection_writer(&writer)
    })();

    finish_writer_handoff(admission, writer, result)
}

fn finish_writer_handoff(
    admission: Connection,
    writer: Connection,
    result: ProjectionResult<()>,
) -> ProjectionResult<()> {
    match result {
        Ok(()) => {
            drop(admission);
            drop(writer);
            Ok(())
        }
        Err(error) => {
            drop(writer);
            drop(admission);
            Err(error)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersistedGenerationFormat {
    Current,
    ExactPreviousAlgorithm,
}

fn persisted_generation_format(
    connection: &Connection,
) -> ProjectionResult<PersistedGenerationFormat> {
    let mut statement = connection.prepare(concat!(
        "SELECT projection_kind, schema_version, algorithm_version, tokenizer_version ",
        "FROM projection_generation ORDER BY generation_seq"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut current = 0_u64;
    let mut previous = 0_u64;
    for row in rows {
        let (kind, schema_version, algorithm_version, tokenizer_version) = row?;
        let kind = ProjectionKind::parse(&kind).ok_or_else(|| {
            ProjectionError::Corrupt(
                "projection generation has an unknown projection kind".to_owned(),
            )
        })?;
        if schema_version != i64::from(PROJECTION_SCHEMA_VERSION)
            || tokenizer_version != kind.tokenizer_version()
        {
            return Err(ProjectionError::Corrupt(
                "projection generation has incompatible schema or tokenizer provenance".to_owned(),
            ));
        }
        match algorithm_version.as_str() {
            PROJECTION_ALGORITHM_VERSION => {
                current = current.checked_add(1).ok_or_else(|| {
                    ProjectionError::Corrupt("projection generation count overflow".to_owned())
                })?
            }
            PREVIOUS_PROJECTION_ALGORITHM_VERSION => {
                previous = previous.checked_add(1).ok_or_else(|| {
                    ProjectionError::Corrupt("projection generation count overflow".to_owned())
                })?;
            }
            _ => {
                return Err(ProjectionError::Corrupt(
                    "projection generation has unknown algorithm provenance".to_owned(),
                ));
            }
        }
    }
    if previous == 0 {
        Ok(PersistedGenerationFormat::Current)
    } else if current == 0 {
        Ok(PersistedGenerationFormat::ExactPreviousAlgorithm)
    } else {
        Err(ProjectionError::Corrupt(
            "projection sidecar mixes incompatible ranking algorithms".to_owned(),
        ))
    }
}

fn migration_schema_fingerprint(sql: &str) -> ProjectionResult<Vec<(String, String, String)>> {
    let connection = Connection::open_in_memory()?;
    connection.execute_batch(sql)?;
    projection_schema_fingerprint(&connection)
}

fn projection_schema_fingerprint(
    connection: &Connection,
) -> ProjectionResult<Vec<(String, String, String)>> {
    let query = format!(
        "SELECT type, name, coalesce(sql, '') FROM sqlite_schema \
         WHERE {USER_SCHEMA_OBJECT_PREDICATE} ORDER BY type, name"
    );
    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ProjectionError::from)
}

fn remove_disposable_sidecar(path: &Path) -> ProjectionResult<()> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let candidate = if suffix.is_empty() {
            path.to_path_buf()
        } else {
            let mut value = path.as_os_str().to_os_string();
            value.push(suffix);
            PathBuf::from(value)
        };
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn write_records<F: ProjectionFaultBehavior + ?Sized>(
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
            faults.hit(fault_boundary::ProjectionFaultPoint::Pr01MidWrite)
        } else {
            Ok(())
        }
    };
    if source_records.len() == 0 {
        faults.hit(fault_boundary::ProjectionFaultPoint::Pr01MidWrite)?;
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

#[cfg(feature = "phase1-fault-injection")]
fn apply_verification_corruption(
    connection: &Connection,
    generation_id: GenerationId,
    kind: ProjectionKind,
    corruption: ProjectionVerificationCorruption,
) -> ProjectionResult<()> {
    let table = match kind {
        ProjectionKind::Unicode61 => "projection_search_unicode",
        ProjectionKind::Trigram => "projection_search_trigram",
        ProjectionKind::Graph => {
            return Err(ProjectionError::InvalidQuery(
                "FTS verification corruption requires a search generation",
            ));
        }
    };
    match corruption {
        ProjectionVerificationCorruption::WrongNamedTokenizer => {
            connection.execute_batch(&format!(
                "DROP TABLE {table}; CREATE VIRTUAL TABLE {table} USING fts5(\
                 body, content_id UNINDEXED, tokenize = 'ascii');"
            ))?;
        }
        ProjectionVerificationCorruption::MissingFtsRow => {
            connection.execute(
                &format!(
                    "DELETE FROM {table} WHERE rowid = (SELECT min(content_id) FROM \
                     projection_search_content WHERE generation_id = ?1)"
                ),
                [generation_id.as_bytes().as_slice()],
            )?;
        }
        ProjectionVerificationCorruption::WrongPersistedTiebreaker => {
            connection.execute(
                concat!(
                    "UPDATE projection_search_content SET stable_tiebreaker = zeroblob(32) ",
                    "WHERE generation_id = ?1"
                ),
                [generation_id.as_bytes().as_slice()],
            )?;
        }
    }
    Ok(())
}

fn create_building_generation(
    projection: &mut Connection,
    authority: &BuildAuthority<'_>,
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
        authority.kind,
        authority.domain,
        built_at_unix_ms,
        builder_binary_digest,
        effective_config_hash,
    );
    transaction.execute(
        concat!(
            "INSERT INTO projection_generation (generation_seq, generation_id, projection_kind, ",
            "schema_version, builder_binary_digest, algorithm_version, tokenizer_version, ",
            "effective_config_hash, known_at_accept_seq, valid_at_unix_ms, source_outbox_seq, ",
            "source_ledger_digest, resolver_version, policy_registry_version, ",
            "policy_registry_hash, security_domain, built_at_unix_ms, state) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ",
            "?15, ?16, ?17, 'BUILDING')"
        ),
        params![
            generation_seq,
            generation_id.as_bytes().as_slice(),
            authority.kind.as_str(),
            i64::from(PROJECTION_SCHEMA_VERSION),
            builder_binary_digest.as_bytes().as_slice(),
            PROJECTION_ALGORITHM_VERSION,
            authority.kind.tokenizer_version(),
            effective_config_hash.as_bytes().as_slice(),
            checked_i64(authority.coordinates.known_at_accept_seq)?,
            authority.coordinates.valid_at.value(),
            checked_i64(authority.watermark.source_outbox_seq)?,
            authority
                .watermark
                .source_ledger_digest
                .as_bytes()
                .as_slice(),
            CANONICAL_RESOLVER_VERSION,
            authority.policies.version(),
            authority.policies.canonical_hash().as_bytes().as_slice(),
            authority.domain.as_bytes().as_slice(),
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

fn activate_generation<F: ProjectionFaultBehavior + ?Sized>(
    projection: &mut Connection,
    generation_id: GenerationId,
    authority: &BuildAuthority<'_>,
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
            authority.kind.as_str(),
            authority.domain.as_bytes().as_slice()
        ],
        |row| row.get::<_, String>(0),
    )?;
    if state != GenerationState::Verified.as_str() {
        return Err(ProjectionError::Corrupt(
            "only a VERIFIED generation may be activated".to_owned(),
        ));
    }
    let candidate = read_generation_metadata(&transaction, generation_id)?;
    if candidate.kind != authority.kind
        || candidate.security_domain != authority.domain
        || candidate.coordinates != authority.coordinates
        || candidate.source_outbox_seq != authority.watermark.source_outbox_seq
        || candidate.source_ledger_digest != authority.watermark.source_ledger_digest
        || candidate.resolver_version != CANONICAL_RESOLVER_VERSION
        || candidate.policy_registry_version != authority.policies.version()
        || candidate.policy_registry_hash != authority.policies.canonical_hash()
    {
        return Err(ProjectionError::Corrupt(
            "VERIFIED generation authority does not match activation input".to_owned(),
        ));
    }
    let existing = read_active_generation(&transaction, authority.kind, authority.domain)?;
    let cursor_exists = transaction.query_row(
        concat!(
            "SELECT EXISTS(SELECT 1 FROM projection_cursor ",
            "WHERE projection_kind = ?1 AND security_domain = ?2)"
        ),
        params![
            authority.kind.as_str(),
            authority.domain.as_bytes().as_slice()
        ],
        |row| row.get::<_, bool>(0),
    )?;
    if existing.is_none() && cursor_exists {
        return Err(ProjectionError::Corrupt(
            "projection cursor exists without an active authority pointer".to_owned(),
        ));
    }
    if let Some(active) = existing
        && (active.source_outbox_seq > authority.watermark.source_outbox_seq
            || (active.source_outbox_seq == authority.watermark.source_outbox_seq
                && active.coordinates.known_at_accept_seq > authority.watermark.source_accept_seq))
    {
        return Ok(false);
    }
    transaction.execute(
        concat!(
            "INSERT INTO projection_active (projection_kind, security_domain, generation_id, ",
            "known_at_accept_seq, valid_at_unix_ms, source_outbox_seq, source_ledger_digest, ",
            "resolver_version, policy_registry_version, policy_registry_hash, ",
            "activated_at_unix_ms) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) ",
            "ON CONFLICT(projection_kind, security_domain) DO UPDATE SET ",
            "generation_id = excluded.generation_id, ",
            "known_at_accept_seq = excluded.known_at_accept_seq, ",
            "valid_at_unix_ms = excluded.valid_at_unix_ms, ",
            "source_outbox_seq = excluded.source_outbox_seq, ",
            "source_ledger_digest = excluded.source_ledger_digest, ",
            "resolver_version = excluded.resolver_version, ",
            "policy_registry_version = excluded.policy_registry_version, ",
            "policy_registry_hash = excluded.policy_registry_hash, ",
            "activated_at_unix_ms = excluded.activated_at_unix_ms"
        ),
        params![
            authority.kind.as_str(),
            authority.domain.as_bytes().as_slice(),
            generation_id.as_bytes().as_slice(),
            checked_i64(authority.coordinates.known_at_accept_seq)?,
            authority.coordinates.valid_at.value(),
            checked_i64(authority.watermark.source_outbox_seq)?,
            authority
                .watermark
                .source_ledger_digest
                .as_bytes()
                .as_slice(),
            CANONICAL_RESOLVER_VERSION,
            authority.policies.version(),
            authority.policies.canonical_hash().as_bytes().as_slice(),
            activated_at_unix_ms,
        ],
    )?;
    faults.hit(fault_boundary::ProjectionFaultPoint::Pr03DuringActivation)?;
    transaction.execute(
        concat!(
            "INSERT INTO projection_cursor (projection_kind, security_domain, last_outbox_seq, ",
            "source_ledger_digest, known_at_accept_seq, valid_at_unix_ms, resolver_version, ",
            "policy_registry_version, policy_registry_hash, updated_at_unix_ms) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) ",
            "ON CONFLICT(projection_kind, security_domain) DO UPDATE SET ",
            "last_outbox_seq = excluded.last_outbox_seq, ",
            "source_ledger_digest = excluded.source_ledger_digest, ",
            "known_at_accept_seq = excluded.known_at_accept_seq, ",
            "valid_at_unix_ms = excluded.valid_at_unix_ms, ",
            "resolver_version = excluded.resolver_version, ",
            "policy_registry_version = excluded.policy_registry_version, ",
            "policy_registry_hash = excluded.policy_registry_hash, ",
            "updated_at_unix_ms = excluded.updated_at_unix_ms"
        ),
        params![
            authority.kind.as_str(),
            authority.domain.as_bytes().as_slice(),
            checked_i64(authority.watermark.source_outbox_seq)?,
            authority
                .watermark
                .source_ledger_digest
                .as_bytes()
                .as_slice(),
            checked_i64(authority.coordinates.known_at_accept_seq)?,
            authority.coordinates.valid_at.value(),
            CANONICAL_RESOLVER_VERSION,
            authority.policies.version(),
            authority.policies.canonical_hash().as_bytes().as_slice(),
            activated_at_unix_ms,
        ],
    )?;
    transaction.commit()?;
    Ok(true)
}

pub(crate) fn open_projection_reader(path: &Path) -> ProjectionResult<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)?;
    connection.execute_batch(
        "PRAGMA query_only = ON; PRAGMA foreign_keys = ON; PRAGMA trusted_schema = OFF; \
         PRAGMA busy_timeout = 250; PRAGMA temp_store = MEMORY;",
    )?;
    verify_projection_schema(&connection)?;
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
    if projection_schema_fingerprint(connection)?
        != migration_schema_fingerprint(MIGRATION_0003_SQL)?
    {
        return Err(ProjectionError::Corrupt(format!(
            "projection schema does not exactly match physical version {PROJECTION_DATABASE_VERSION}"
        )));
    }
    for (name, columns) in [
        (
            "projection_generation",
            &[
                "generation_seq",
                "generation_id",
                "projection_kind",
                "schema_version",
                "builder_binary_digest",
                "algorithm_version",
                "tokenizer_version",
                "effective_config_hash",
                "known_at_accept_seq",
                "valid_at_unix_ms",
                "source_outbox_seq",
                "source_ledger_digest",
                "resolver_version",
                "policy_registry_version",
                "policy_registry_hash",
                "security_domain",
                "built_at_unix_ms",
                "state",
                "record_count",
                "canonical_checksum",
                "failure_reason",
            ][..],
        ),
        (
            "projection_active",
            &[
                "projection_kind",
                "security_domain",
                "generation_id",
                "known_at_accept_seq",
                "valid_at_unix_ms",
                "source_outbox_seq",
                "source_ledger_digest",
                "resolver_version",
                "policy_registry_version",
                "policy_registry_hash",
                "activated_at_unix_ms",
            ],
        ),
        (
            "projection_cursor",
            &[
                "projection_kind",
                "security_domain",
                "last_outbox_seq",
                "source_ledger_digest",
                "known_at_accept_seq",
                "valid_at_unix_ms",
                "resolver_version",
                "policy_registry_version",
                "policy_registry_hash",
                "updated_at_unix_ms",
            ],
        ),
        (
            "projection_graph_edge",
            &[
                "generation_id",
                "claim_id",
                "source_entity_id",
                "predicate_id",
                "target_entity_id",
                "scope_id",
                "security_domain",
                "authority_class",
                "epistemic_status",
                "authority_policy",
                "valid_from_unix_ms",
                "valid_to_unix_ms",
                "source_accept_seq",
                "stable_tiebreaker",
            ],
        ),
        (
            "projection_graph_edge_evidence",
            &[
                "generation_id",
                "claim_id",
                "evidence_ordinal",
                "evidence_id",
            ],
        ),
        (
            "projection_search_content",
            &[
                "content_id",
                "generation_id",
                "record_key",
                "claim_id",
                "evidence_id",
                "subject_entity_id",
                "predicate_id",
                "body",
                "artifact_id",
                "representation_index",
                "locator_kind",
                "locator_payload",
                "security_domain",
                "authority_class",
                "epistemic_status",
                "authority_policy",
                "valid_from_unix_ms",
                "valid_to_unix_ms",
                "source_accept_seq",
                "stable_tiebreaker",
            ],
        ),
        (
            "projection_exact_symbol",
            &["generation_id", "symbol", "content_id", "stable_tiebreaker"],
        ),
    ] {
        let count = connection.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )?;
        if count != 1 {
            return Err(ProjectionError::Corrupt(format!(
                "required projection object {name} count is {count}"
            )));
        }
        verify_table_columns(connection, name, columns)?;
    }
    verify_named_fts_schema(connection)
}

fn verify_table_columns(
    connection: &Connection,
    table: &str,
    expected: &[&str],
) -> ProjectionResult<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info('{table}')"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let actual = rows.collect::<Result<Vec<_>, _>>()?;
    if actual != expected {
        return Err(ProjectionError::Corrupt(format!(
            "projection table {table} columns do not exactly match version {PROJECTION_DATABASE_VERSION}: expected {expected:?}, observed {actual:?}"
        )));
    }
    Ok(())
}

fn verify_named_fts_schema(connection: &Connection) -> ProjectionResult<()> {
    let expected_schema = migration_schema_fingerprint(MIGRATION_0003_SQL)?;
    for name in ["projection_search_unicode", "projection_search_trigram"] {
        let expected = expected_schema
            .iter()
            .find(|(_, expected_name, _)| expected_name == name)
            .ok_or_else(|| {
                ProjectionError::Corrupt(format!(
                    "current migration does not define required named FTS object {name}"
                ))
            })?;
        let row = connection
            .query_row(
                "SELECT type, sql FROM sqlite_schema WHERE name = ?1",
                [name],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let Some((object_type, Some(actual_sql))) = row else {
            return Err(ProjectionError::Corrupt(format!(
                "required named FTS object {name} is absent or has no schema SQL"
            )));
        };
        if object_type != expected.0 || actual_sql != expected.2 {
            return Err(ProjectionError::Corrupt(format!(
                "named FTS object {name} has the wrong type, schema, or tokenizer"
            )));
        }
    }
    Ok(())
}

fn verify_fts5(connection: &Connection) -> ProjectionResult<()> {
    verify_named_fts_schema(connection)?;
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

fn verify_generation_storage(
    connection: &Connection,
    generation_id: GenerationId,
    kind: ProjectionKind,
    coordinates: ProjectionCoordinates,
    watermark: Watermark,
    policies: &PredicatePolicies,
) -> ProjectionResult<()> {
    verify_named_fts_schema(connection)?;
    let metadata = read_generation_metadata(connection, generation_id)?;
    if metadata.kind != kind
        || metadata.state != GenerationState::Building
        || metadata.schema_version != PROJECTION_SCHEMA_VERSION
        || metadata.algorithm_version != PROJECTION_ALGORITHM_VERSION
        || metadata.tokenizer_version != kind.tokenizer_version()
        || metadata.coordinates != coordinates
        || metadata.source_outbox_seq != watermark.source_outbox_seq
        || metadata.source_ledger_digest != watermark.source_ledger_digest
        || metadata.resolver_version != CANONICAL_RESOLVER_VERSION
        || metadata.policy_registry_version != policies.version()
        || metadata.policy_registry_hash != policies.canonical_hash()
    {
        return Err(ProjectionError::Corrupt(
            "BUILDING generation authority metadata does not match the verified build input"
                .to_owned(),
        ));
    }
    let generation = generation_id.as_bytes().as_slice();
    let policy_rows_sql = match kind {
        ProjectionKind::Graph => concat!(
            "SELECT predicate_id, authority_policy FROM projection_graph_edge ",
            "WHERE generation_id = ?1"
        ),
        ProjectionKind::Unicode61 | ProjectionKind::Trigram => concat!(
            "SELECT predicate_id, authority_policy FROM projection_search_content ",
            "WHERE generation_id = ?1"
        ),
    };
    let mut policy_statement = connection.prepare(policy_rows_sql)?;
    let policy_rows = policy_statement.query_map([generation], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in policy_rows {
        let (predicate, persisted_policy) = row?;
        let predicate =
            academic_domain::PredicateId::parse(predicate).map_err(ProjectionError::Domain)?;
        let expected_policy = policies.policy_for(&predicate)?;
        if persisted_policy != crate::resolution::authority_policy_name(expected_policy) {
            return Err(ProjectionError::Corrupt(format!(
                "record policy for {} does not match the bound registry",
                predicate.as_str()
            )));
        }
    }
    match kind {
        ProjectionKind::Graph => {
            let lexical_rows = connection.query_row(
                "SELECT count(*) FROM projection_search_content WHERE generation_id = ?1",
                [generation],
                |row| row.get::<_, i64>(0),
            )?;
            if lexical_rows != 0 {
                return Err(ProjectionError::Corrupt(
                    "graph generation contains lexical rows".to_owned(),
                ));
            }
        }
        ProjectionKind::Unicode61 | ProjectionKind::Trigram => {
            let (table, other) = if kind == ProjectionKind::Unicode61 {
                ("projection_search_unicode", "projection_search_trigram")
            } else {
                ("projection_search_trigram", "projection_search_unicode")
            };
            let expected = connection.query_row(
                "SELECT count(*) FROM projection_search_content WHERE generation_id = ?1",
                [generation],
                |row| row.get::<_, i64>(0),
            )?;
            let covered_sql = format!(
                "SELECT count(*) FROM projection_search_content c JOIN {table} f \
                 ON f.rowid = c.content_id AND f.content_id = c.content_id AND f.body = c.body \
                 WHERE c.generation_id = ?1"
            );
            let covered =
                connection.query_row(&covered_sql, [generation], |row| row.get::<_, i64>(0))?;
            let wrong_table_sql = format!(
                "SELECT count(*) FROM projection_search_content c JOIN {other} f \
                 ON f.rowid = c.content_id WHERE c.generation_id = ?1"
            );
            let wrong_table =
                connection.query_row(&wrong_table_sql, [generation], |row| row.get::<_, i64>(0))?;
            if covered != expected || wrong_table != 0 {
                return Err(ProjectionError::Corrupt(format!(
                    "{table} rowid/content coverage mismatch: expected {expected}, covered {covered}, opposite-table {wrong_table}"
                )));
            }
            let orphan_sql = format!(
                "SELECT count(*) FROM {table} f LEFT JOIN projection_search_content c \
                 ON c.content_id = f.rowid WHERE c.content_id IS NULL \
                 OR f.content_id <> f.rowid OR f.body <> c.body"
            );
            let orphan_rows = connection.query_row(&orphan_sql, [], |row| row.get::<_, i64>(0))?;
            if orphan_rows != 0 {
                return Err(ProjectionError::Corrupt(format!(
                    "{table} contains orphaned or mismatched rowid/content rows"
                )));
            }
            let symbol_mismatch = connection.query_row(
                concat!(
                    "SELECT count(*) FROM projection_search_content c ",
                    "LEFT JOIN projection_exact_symbol s ON s.generation_id = c.generation_id ",
                    "AND s.content_id = c.content_id WHERE c.generation_id = ?1 AND ",
                    "((c.predicate_id LIKE '%.symbol' AND ",
                    "(s.content_id IS NULL OR s.symbol <> c.body COLLATE BINARY ",
                    "OR s.stable_tiebreaker <> c.stable_tiebreaker)) OR ",
                    "(c.predicate_id NOT LIKE '%.symbol' AND s.content_id IS NOT NULL))"
                ),
                [generation],
                |row| row.get::<_, i64>(0),
            )?;
            if symbol_mismatch != 0 {
                return Err(ProjectionError::Corrupt(
                    "exact-symbol coverage or stable tiebreaker mismatch".to_owned(),
                ));
            }
            let integrity_sql = format!("INSERT INTO {table}({table}) VALUES('integrity-check')");
            connection.execute(&integrity_sql, [])?;
        }
    }
    Ok(())
}

pub(crate) fn read_active_generation(
    connection: &Connection,
    kind: ProjectionKind,
    domain: DomainId,
) -> ProjectionResult<Option<ActiveGeneration>> {
    #[derive(Debug)]
    struct RawActive {
        generation_id: Vec<u8>,
        active_known: i64,
        active_valid: i64,
        active_outbox: i64,
        active_source_digest: Vec<u8>,
        active_resolver: String,
        active_policy_version: String,
        active_policy_hash: Vec<u8>,
        record_count: Option<i64>,
        checksum: Option<Vec<u8>>,
        state: Option<String>,
        generation_known: Option<i64>,
        generation_valid: Option<i64>,
        generation_outbox: Option<i64>,
        generation_source_digest: Option<Vec<u8>>,
        generation_resolver: Option<String>,
        generation_policy_version: Option<String>,
        generation_policy_hash: Option<Vec<u8>>,
        cursor_known: Option<i64>,
        cursor_valid: Option<i64>,
        cursor_outbox: Option<i64>,
        cursor_source_digest: Option<Vec<u8>>,
        cursor_resolver: Option<String>,
        cursor_policy_version: Option<String>,
        cursor_policy_hash: Option<Vec<u8>>,
        generation_schema_version: Option<i64>,
        generation_algorithm_version: Option<String>,
        generation_tokenizer_version: Option<String>,
    }
    let row: Option<RawActive> = connection
        .query_row(
            concat!(
                "SELECT a.generation_id, a.known_at_accept_seq, a.valid_at_unix_ms, ",
                "a.source_outbox_seq, a.source_ledger_digest, a.resolver_version, ",
                "a.policy_registry_version, a.policy_registry_hash, g.record_count, ",
                "g.canonical_checksum, g.state, g.known_at_accept_seq, g.valid_at_unix_ms, ",
                "g.source_outbox_seq, g.source_ledger_digest, g.resolver_version, ",
                "g.policy_registry_version, g.policy_registry_hash, c.known_at_accept_seq, ",
                "c.valid_at_unix_ms, c.last_outbox_seq, c.source_ledger_digest, ",
                "c.resolver_version, c.policy_registry_version, c.policy_registry_hash, ",
                "g.schema_version, g.algorithm_version, g.tokenizer_version ",
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
                Ok(RawActive {
                    generation_id: row.get(0)?,
                    active_known: row.get(1)?,
                    active_valid: row.get(2)?,
                    active_outbox: row.get(3)?,
                    active_source_digest: row.get(4)?,
                    active_resolver: row.get(5)?,
                    active_policy_version: row.get(6)?,
                    active_policy_hash: row.get(7)?,
                    record_count: row.get(8)?,
                    checksum: row.get(9)?,
                    state: row.get(10)?,
                    generation_known: row.get(11)?,
                    generation_valid: row.get(12)?,
                    generation_outbox: row.get(13)?,
                    generation_source_digest: row.get(14)?,
                    generation_resolver: row.get(15)?,
                    generation_policy_version: row.get(16)?,
                    generation_policy_hash: row.get(17)?,
                    cursor_known: row.get(18)?,
                    cursor_valid: row.get(19)?,
                    cursor_outbox: row.get(20)?,
                    cursor_source_digest: row.get(21)?,
                    cursor_resolver: row.get(22)?,
                    cursor_policy_version: row.get(23)?,
                    cursor_policy_hash: row.get(24)?,
                    generation_schema_version: row.get(25)?,
                    generation_algorithm_version: row.get(26)?,
                    generation_tokenizer_version: row.get(27)?,
                })
            },
        )
        .optional()?;
    row.map(|row| {
        let record_count = row.record_count.ok_or_else(|| {
            ProjectionError::Corrupt("active generation has no record count".to_owned())
        })?;
        let checksum = row.checksum.ok_or_else(|| {
            ProjectionError::Corrupt("active generation has no checksum".to_owned())
        })?;
        let state = row.state.ok_or_else(|| {
            ProjectionError::Corrupt("active pointer has no generation".to_owned())
        })?;
        if state != GenerationState::Verified.as_str() {
            return Err(ProjectionError::Corrupt(
                "active pointer references a non-VERIFIED generation".to_owned(),
            ));
        }
        if row.generation_schema_version != Some(i64::from(PROJECTION_SCHEMA_VERSION))
            || row.generation_algorithm_version.as_deref() != Some(PROJECTION_ALGORITHM_VERSION)
            || row.generation_tokenizer_version.as_deref() != Some(kind.tokenizer_version())
        {
            return Err(ProjectionError::Corrupt(
                "active generation has incompatible schema, algorithm, or tokenizer provenance"
                    .to_owned(),
            ));
        }
        let generation_authority = (
            row.generation_known,
            row.generation_valid,
            row.generation_outbox,
            row.generation_source_digest.as_deref(),
            row.generation_resolver.as_deref(),
            row.generation_policy_version.as_deref(),
            row.generation_policy_hash.as_deref(),
        );
        let cursor_authority = (
            row.cursor_known,
            row.cursor_valid,
            row.cursor_outbox,
            row.cursor_source_digest.as_deref(),
            row.cursor_resolver.as_deref(),
            row.cursor_policy_version.as_deref(),
            row.cursor_policy_hash.as_deref(),
        );
        let active_authority = (
            Some(row.active_known),
            Some(row.active_valid),
            Some(row.active_outbox),
            Some(row.active_source_digest.as_slice()),
            Some(row.active_resolver.as_str()),
            Some(row.active_policy_version.as_str()),
            Some(row.active_policy_hash.as_slice()),
        );
        if active_authority != generation_authority || active_authority != cursor_authority {
            return Err(ProjectionError::Corrupt(
                "active generation, generation metadata, and cursor authority disagree".to_owned(),
            ));
        }
        Ok(ActiveGeneration {
            generation_id: GenerationId::from_bytes(fixed_bytes(
                row.generation_id,
                "active generation identifier",
            )?),
            kind,
            security_domain: domain,
            coordinates: ProjectionCoordinates::new(
                nonnegative_u64(row.active_known, "active known_at_accept_seq")?,
                TimestampMillis::new(row.active_valid),
            ),
            source_outbox_seq: nonnegative_u64(row.active_outbox, "active source outbox_seq")?,
            source_ledger_digest: ContentDigest::from_sha256_bytes(fixed_bytes(
                row.active_source_digest,
                "active source ledger digest",
            )?),
            resolver_version: row.active_resolver,
            policy_registry_version: row.active_policy_version,
            policy_registry_hash: ContentDigest::from_sha256_bytes(fixed_bytes(
                row.active_policy_hash,
                "active policy registry hash",
            )?),
            record_count: nonnegative_u64(record_count, "active record count")?,
            canonical_checksum: ContentDigest::from_sha256_bytes(fixed_bytes(
                checksum,
                "active canonical checksum",
            )?),
        })
    })
    .transpose()
}

pub(crate) fn read_verified_generation(
    connection: &Connection,
    kind: ProjectionKind,
    domain: DomainId,
    coordinates: ProjectionCoordinates,
    watermark: Watermark,
    policies: &PredicatePolicies,
) -> ProjectionResult<Option<ActiveGeneration>> {
    let generation_id = connection
        .query_row(
            concat!(
                "SELECT generation_id FROM projection_generation WHERE projection_kind = ?1 ",
                "AND security_domain = ?2 AND known_at_accept_seq = ?3 ",
                "AND valid_at_unix_ms = ?4 AND source_outbox_seq = ?5 ",
                "AND source_ledger_digest = ?6 AND resolver_version = ?7 ",
                "AND policy_registry_version = ?8 AND policy_registry_hash = ?9 ",
                "AND schema_version = ?10 AND algorithm_version = ?11 ",
                "AND tokenizer_version = ?12 AND state = 'VERIFIED' ",
                "ORDER BY generation_seq DESC LIMIT 1"
            ),
            params![
                kind.as_str(),
                domain.as_bytes().as_slice(),
                checked_i64(coordinates.known_at_accept_seq)?,
                coordinates.valid_at.value(),
                checked_i64(watermark.source_outbox_seq)?,
                watermark.source_ledger_digest.as_bytes().as_slice(),
                CANONICAL_RESOLVER_VERSION,
                policies.version(),
                policies.canonical_hash().as_bytes().as_slice(),
                i64::from(PROJECTION_SCHEMA_VERSION),
                PROJECTION_ALGORITHM_VERSION,
                kind.tokenizer_version(),
            ],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()?;
    let Some(generation_id) = generation_id else {
        return Ok(None);
    };
    let metadata = read_generation_metadata(
        connection,
        GenerationId::from_bytes(fixed_bytes(
            generation_id,
            "historical generation identifier",
        )?),
    )?;
    if metadata.state != GenerationState::Verified {
        return Err(ProjectionError::Corrupt(
            "historical generation selection returned a non-VERIFIED row".to_owned(),
        ));
    }
    let record_count = metadata.record_count.ok_or_else(|| {
        ProjectionError::Corrupt("VERIFIED historical generation has no record count".to_owned())
    })?;
    let canonical_checksum = metadata.canonical_checksum.ok_or_else(|| {
        ProjectionError::Corrupt("VERIFIED historical generation has no checksum".to_owned())
    })?;
    Ok(Some(ActiveGeneration {
        generation_id: metadata.generation_id,
        kind: metadata.kind,
        security_domain: metadata.security_domain,
        coordinates: metadata.coordinates,
        source_outbox_seq: metadata.source_outbox_seq,
        source_ledger_digest: metadata.source_ledger_digest,
        resolver_version: metadata.resolver_version,
        policy_registry_version: metadata.policy_registry_version,
        policy_registry_hash: metadata.policy_registry_hash,
        record_count,
        canonical_checksum,
    }))
}

fn read_generation_metadata(
    connection: &Connection,
    generation_id: GenerationId,
) -> ProjectionResult<GenerationMetadata> {
    #[derive(Debug)]
    struct RawGeneration {
        kind: String,
        schema_version: i64,
        builder_digest: Vec<u8>,
        algorithm_version: String,
        tokenizer_version: String,
        config_hash: Vec<u8>,
        known_at_accept_seq: i64,
        valid_at_unix_ms: i64,
        source_outbox_seq: i64,
        source_ledger_digest: Vec<u8>,
        resolver_version: String,
        policy_registry_version: String,
        policy_registry_hash: Vec<u8>,
        domain: Vec<u8>,
        built_at_unix_ms: i64,
        state: String,
        record_count: Option<i64>,
        checksum: Option<Vec<u8>>,
    }
    let row: RawGeneration = connection.query_row(
        concat!(
            "SELECT projection_kind, schema_version, builder_binary_digest, algorithm_version, ",
            "tokenizer_version, effective_config_hash, known_at_accept_seq, valid_at_unix_ms, ",
            "source_outbox_seq, source_ledger_digest, resolver_version, policy_registry_version, ",
            "policy_registry_hash, security_domain, built_at_unix_ms, state, record_count, ",
            "canonical_checksum ",
            "FROM projection_generation WHERE generation_id = ?1"
        ),
        [generation_id.as_bytes().as_slice()],
        |row| {
            Ok(RawGeneration {
                kind: row.get(0)?,
                schema_version: row.get(1)?,
                builder_digest: row.get(2)?,
                algorithm_version: row.get(3)?,
                tokenizer_version: row.get(4)?,
                config_hash: row.get(5)?,
                known_at_accept_seq: row.get(6)?,
                valid_at_unix_ms: row.get(7)?,
                source_outbox_seq: row.get(8)?,
                source_ledger_digest: row.get(9)?,
                resolver_version: row.get(10)?,
                policy_registry_version: row.get(11)?,
                policy_registry_hash: row.get(12)?,
                domain: row.get(13)?,
                built_at_unix_ms: row.get(14)?,
                state: row.get(15)?,
                record_count: row.get(16)?,
                checksum: row.get(17)?,
            })
        },
    )?;
    Ok(GenerationMetadata {
        generation_id,
        kind: ProjectionKind::parse(&row.kind)
            .ok_or_else(|| ProjectionError::Corrupt("unknown projection kind".to_owned()))?,
        schema_version: u32::try_from(row.schema_version).map_err(|_| {
            ProjectionError::Corrupt("invalid projection schema version".to_owned())
        })?,
        builder_binary_digest: ContentDigest::from_sha256_bytes(fixed_bytes(
            row.builder_digest,
            "builder binary digest",
        )?),
        algorithm_version: row.algorithm_version,
        tokenizer_version: row.tokenizer_version,
        effective_config_hash: ContentDigest::from_sha256_bytes(fixed_bytes(
            row.config_hash,
            "effective config hash",
        )?),
        coordinates: ProjectionCoordinates::new(
            nonnegative_u64(row.known_at_accept_seq, "generation known_at_accept_seq")?,
            TimestampMillis::new(row.valid_at_unix_ms),
        ),
        source_outbox_seq: nonnegative_u64(row.source_outbox_seq, "generation source outbox_seq")?,
        source_ledger_digest: ContentDigest::from_sha256_bytes(fixed_bytes(
            row.source_ledger_digest,
            "generation source ledger digest",
        )?),
        resolver_version: row.resolver_version,
        policy_registry_version: row.policy_registry_version,
        policy_registry_hash: ContentDigest::from_sha256_bytes(fixed_bytes(
            row.policy_registry_hash,
            "generation policy registry hash",
        )?),
        security_domain: id_from_bytes(row.domain, "generation security domain")?,
        built_at_unix_ms: row.built_at_unix_ms,
        state: GenerationState::parse(&row.state)
            .ok_or_else(|| ProjectionError::Corrupt("unknown generation state".to_owned()))?,
        record_count: row
            .record_count
            .map(|value| nonnegative_u64(value, "generation record count"))
            .transpose()?,
        canonical_checksum: row
            .checksum
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
    material.extend_from_slice(b"ACADEMIC_PROJECTION_GENERATION_V3");
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

fn pragma_i64(connection: &Connection, name: &'static str) -> ProjectionResult<i64> {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .map_err(ProjectionError::from)
}

fn user_object_count(connection: &Connection) -> ProjectionResult<i64> {
    let query = format!("SELECT count(*) FROM sqlite_schema WHERE {USER_SCHEMA_OBJECT_PREDICATE}");
    connection
        .query_row(&query, [], |row| row.get(0))
        .map_err(ProjectionError::from)
}

fn checked_i64(value: u64) -> ProjectionResult<i64> {
    i64::try_from(value).map_err(|_| ProjectionError::IntegerOverflow(value))
}

fn nonnegative_u64(value: i64, reason: &'static str) -> ProjectionResult<u64> {
    u64::try_from(value).map_err(|_| ProjectionError::Corrupt(reason.to_owned()))
}

pub(crate) fn fixed_bytes<const N: usize>(
    bytes: Vec<u8>,
    reason: &'static str,
) -> ProjectionResult<[u8; N]> {
    bytes
        .try_into()
        .map_err(|_| ProjectionError::Corrupt(reason.to_owned()))
}

pub(crate) fn id_from_bytes<T>(bytes: Vec<u8>, reason: &'static str) -> ProjectionResult<T>
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
