//! Synthetic-profile operations shared by the headless CLI.
//!
//! The CLI must never open the canonical writer, never hold a raw SQLite
//! connection, and never learn the fixture locator key. This module is the one
//! place that composes the already-admitted V1, S2, J1, and B1 boundaries into
//! read-only diagnostics and the deterministic export, backup, and restore
//! surfaces, so `academic-cli` keeps a thin dependency edge and no write
//! capability.
//!
//! Ingest stays bound to the sole repository-allowlisted fixture. Read-only
//! operations also consume the explicitly imported synthetic detail profile,
//! verified against this build's independent synthetic authorization.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use academic_contracts::{DeviceAuthorization, verify_signed_batch};
use academic_domain::{
    ArtifactDescriptor, ContentDigest, DomainId, EventPayload, PredicateId, TimestampMillis,
};
use academic_portability::{
    PortabilityError,
    backup::{BackupReceipt, backup_profile},
    export::{ExportReceipt, export_profile},
    restore::{
        PROJECTION_SIDECAR_FILE, ProjectionRebuildTarget, RestoreMaterial, RestoreReceipt,
        restore_profile_with_material,
    },
    verify::{CanonicalDatabase, read_canonical_rows},
};
use academic_projections::{
    generation::{ProjectionCoordinates, ProjectionKind},
    resolution::{AuthorityPolicy, PredicatePolicies},
    runner::ProjectionRunner,
};
use academic_rpc::generated::{MutableRequest, SyntheticIngestCommand, mutable_request};
use academic_store::{
    SYNTHETIC_PROFILE_MARKER,
    connection::open_reader,
    error::StoreError,
    path_policy::{NativePathProbe, PathPolicyViolation},
    queries::{QueryError, canonical_snapshot},
};
use academic_vault::{DomainKeyring, Vault};
use serde::Serialize;

use crate::{
    CoreError, fixture_device_authorization, immutable_v2_fixture_document,
    local_service::{FIXTURE_LOCATOR_KEY, PHASE1_SYNTHETIC_FIXTURE_ID},
};

/// Deterministic open-export contract name reported by the CLI.
pub const EXPORT_FORMAT: &str = academic_portability::PHASE1_EXPORT_FORMAT;
/// Plaintext synthetic-backup contract name reported by the CLI.
pub const BACKUP_FORMAT: &str = academic_portability::PHASE1_BACKUP_FORMAT;
/// Unavoidable statement that the Phase 1 backup format protects nothing.
pub const BACKUP_PLAINTEXT_WARNING: &str = academic_portability::PHASE1_BACKUP_PLAINTEXT_WARNING;

/// Capability the synthetic-ingest command is bound to.
pub const SYNTHETIC_INGEST_CAPABILITY: &str = "learning-platform.local.synthetic-ingest.v1";
/// Read-only capability a diagnostic or export handshake negotiates.
pub const DIAGNOSTICS_CAPABILITY: &str = "learning-platform.local.diagnostics.v1";
/// Read-only capability an export handshake negotiates.
pub const SYNTHETIC_EXPORT_CAPABILITY: &str = "learning-platform.local.synthetic-export.v1";

/// Domain separators for the deterministic ingest request identifiers.
const CLI_CLIENT_INSTANCE_DOMAIN: &[u8] = b"learning-platform.cli.client-instance.v1";
const CLI_INGEST_IDEMPOTENCY_DOMAIN: &[u8] = b"learning-platform.cli.ingest-idempotency.v1/";
const CLI_INGEST_REQUEST_DOMAIN: &[u8] = b"learning-platform.cli.ingest-request.v1/";

/// Versioned predicate-policy registry used by every Phase 1 projection build.
pub const PHASE1_POLICY_REGISTRY_VERSION: &str = "phase1-fixture-policies-v1";

/// Valid-time coordinate every Phase 1 projection generation is evaluated at.
///
/// It is the fixture's final valid instant, so a rebuilt generation observes
/// the complete corpus instead of a mid-history slice.
pub const PHASE1_PROJECTION_VALID_AT: TimestampMillis = crate::FINAL_VALID_AT;

/// Projection generations a Phase 1 profile owns.
pub const PHASE1_PROJECTION_KINDS: &[ProjectionKind] = &[
    ProjectionKind::Graph,
    ProjectionKind::Unicode61,
    ProjectionKind::Trigram,
];

/// Every predicate the allowlisted fixture asserts, with its frozen authority
/// policy.
///
/// `fixture_predicate_policies` fails closed when the fixture asserts a
/// predicate absent from this table, so a fixture change cannot silently
/// acquire a defaulted resolution policy.
///
/// - `academic.course.offering` is a model-authored prediction, so it resolves
///   as an implementation observation and never outranks a user decision.
/// - `academic.deadline` is an official record; `summarize_replay` already
///   resolves it as [`AuthorityPolicy::OfficialFact`] and this table agrees.
/// - `knowledge.freshness` is derived by the implementation from observations.
/// - `knowledge.mastery` carries the fixture's explicit user decisions, so it
///   is user-owned and a later inference cannot displace it.
const PHASE1_PREDICATE_POLICIES: &[(&str, AuthorityPolicy)] = &[
    (
        "academic.course.offering",
        AuthorityPolicy::ImplementationObservation,
    ),
    ("academic.deadline", AuthorityPolicy::OfficialFact),
    (
        "knowledge.freshness",
        AuthorityPolicy::ImplementationObservation,
    ),
    ("knowledge.mastery", AuthorityPolicy::UserOwned),
];

/// Failure raised by one synthetic-profile operation.
#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    /// A canonical read failed.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// A store boundary failed.
    #[error(transparent)]
    Store(#[from] academic_store::error::StoreError),
    /// A vault boundary failed.
    #[error(transparent)]
    Vault(#[from] academic_vault::VaultError),
    /// A projection boundary failed.
    #[error(transparent)]
    Projection(#[from] academic_projections::runner::ProjectionError),
    /// An export, backup, or restore failed.
    #[error(transparent)]
    Portability(#[from] PortabilityError),
    /// The fixture or its trust anchor drifted.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// A signed envelope failed verification under its independent anchor.
    #[error(transparent)]
    Contract(#[from] academic_contracts::ContractError),
    /// A domain value could not be reconstructed.
    #[error(transparent)]
    Domain(#[from] academic_domain::DomainError),
    /// A P1 request could not be validated or digested.
    #[error(transparent)]
    Rpc(#[from] academic_rpc::RpcError),
    /// Fixture envelope hex was malformed.
    #[error("fixture envelope hex is invalid: {0}")]
    Hex(#[from] hex::FromHexError),
    /// The profile holds canonical state outside the synthetic allowlist.
    #[error("synthetic profile invariant violated: {0}")]
    UnexpectedState(&'static str),
    /// A filesystem boundary failed.
    #[error("{operation} failed for {path}: {source}")]
    Io {
        /// Stable operation label.
        operation: &'static str,
        /// Relevant local path.
        path: PathBuf,
        /// Native error.
        #[source]
        source: std::io::Error,
    },
}

/// Why one operation failed, in the vocabulary a caller can branch on.
///
/// This lives beside the errors rather than in the CLI so the mapping stays
/// exhaustive when a new failure is introduced here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// The synthetic-only data policy refused the request.
    PolicyDenied,
    /// A destination or an expected watermark conflicted.
    Conflict,
    /// The profile or artefact must be repaired before it can be used.
    RepairRequired,
    /// This build cannot consume the artefact at all.
    Incompatible,
    /// The named location is not one this build may use as a profile.
    PathRejected,
    /// None of the above describes the failure.
    Internal,
}

impl OperationError {
    /// Classifies this failure for a caller that must branch on the reason.
    #[must_use]
    pub fn classify(&self) -> FailureClass {
        match self {
            Self::Portability(error) => classify_portability(error),
            Self::Store(error) => classify_store(error),
            Self::UnexpectedState(_) | Self::Vault(_) => FailureClass::RepairRequired,
            Self::Contract(_) => FailureClass::PolicyDenied,
            _ => FailureClass::Internal,
        }
    }
}

/// Classifies a store-boundary failure.
///
/// A profile path policy refusal is a decision about the location the caller
/// named, not a fault in this build, and a caller has to be able to tell the
/// two apart: retrying a rejected path is pointless, retrying an internal
/// failure is not. `POLICY_DENIED` cannot carry it, because that class is the
/// synthetic-only *data* policy and a caller branches on it to mean exactly
/// that.
fn classify_store(error: &StoreError) -> FailureClass {
    match error {
        StoreError::UnsafeProfilePath(_) => FailureClass::PathRejected,
        _ => FailureClass::Internal,
    }
}

fn classify_portability(error: &PortabilityError) -> FailureClass {
    match error {
        // The location the caller named failed the profile path policy.
        PortabilityError::Store(error) => classify_store(error),
        // The synthetic-only policy block in a manifest did not match.
        PortabilityError::ManifestRejected { .. } => FailureClass::PolicyDenied,
        // The caller aimed at a destination that is already occupied.
        PortabilityError::DestinationExists(_) | PortabilityError::DestinationNotEmpty(_) => {
            FailureClass::Conflict
        }
        // The source advanced while the artefact was being produced.
        PortabilityError::WatermarkMoved { .. } => FailureClass::Conflict,
        // The artefact cannot be consumed by this build at all.
        PortabilityError::PathTooLong { .. } | PortabilityError::UnsafeEntry(_) => {
            FailureClass::Incompatible
        }
        // The bytes on disk disagree with what the manifest promised.
        PortabilityError::IntegrityMismatch { .. }
        | PortabilityError::MissingObject { .. }
        | PortabilityError::MissingAuthorization { .. }
        | PortabilityError::ReplayMismatch { .. }
        | PortabilityError::DatabaseCheckFailed { .. } => FailureClass::RepairRequired,
        _ => FailureClass::Internal,
    }
}

fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> OperationError {
    OperationError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

/// Canonical facts read out of the sole allowlisted fixture.
#[derive(Debug, Clone)]
struct FixtureMaterial {
    descriptors: Vec<ArtifactDescriptor>,
    predicates: BTreeSet<PredicateId>,
}

fn fixture_material() -> Result<FixtureMaterial, OperationError> {
    let document = immutable_v2_fixture_document()?;
    if document.name != PHASE1_SYNTHETIC_FIXTURE_ID {
        return Err(OperationError::UnexpectedState(
            "fixture identifier drifted",
        ));
    }
    let envelope = hex::decode(&document.signed_batch_cbor_hex)?;
    let verified = verify_signed_batch(&envelope, &fixture_device_authorization()?)?;
    let mut descriptors = Vec::new();
    let mut predicates = BTreeSet::new();
    for event in &verified.batch().events {
        match &event.payload {
            EventPayload::ArtifactRegistered(descriptor) => descriptors.push(descriptor.clone()),
            EventPayload::ClaimAsserted(claim) => {
                predicates.insert(claim.predicate_id.clone());
            }
            EventPayload::DecisionRecorded(decision) => {
                predicates.insert(decision.resolution_slot.predicate_id.clone());
            }
            _ => {}
        }
    }
    if descriptors.is_empty() {
        return Err(OperationError::UnexpectedState(
            "fixture has no artifact closure",
        ));
    }
    Ok(FixtureMaterial {
        descriptors,
        predicates,
    })
}

fn fixture_domains(material: &FixtureMaterial) -> Vec<DomainId> {
    material
        .descriptors
        .iter()
        .map(|descriptor| descriptor.domain_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Domain and predicate closure derived from independently verified signed history.
struct SyntheticMaterial {
    descriptors: Vec<ArtifactDescriptor>,
    domains: BTreeSet<DomainId>,
    policies: PredicatePolicies,
    head: u64,
}

fn verified_synthetic_material(
    database: &CanonicalDatabase,
) -> Result<SyntheticMaterial, OperationError> {
    let _snapshot = database.begin_read()?;
    let rows = read_canonical_rows(database)?;
    rows.schema.policy.require_phase1()?;
    academic_portability::verify::replay_signed_batches(database, &fixture_authorizations()?)?;
    let mut reader = open_reader(database.path())?;
    let history = academic_store::queries::signed_history_snapshot(&mut reader)?;
    if history.accept_seq_head != rows.watermark.accept_seq_head {
        return Err(OperationError::UnexpectedState(
            "profile advanced during closure verification",
        ));
    }
    let authorization = fixture_device_authorization()?;
    let mut core = crate::Core::new();
    for batch in history.batches {
        core.accept_signed_batch(&batch.envelope, &authorization)?;
    }
    let mut entries: BTreeMap<PredicateId, AuthorityPolicy> = PHASE1_PREDICATE_POLICIES
        .iter()
        .map(|(predicate, policy)| Ok((PredicateId::parse(*predicate)?, *policy)))
        .collect::<Result<_, academic_domain::DomainError>>()?;
    for (predicate, policy) in [
        (
            crate::details::CORPUS_PREDICATE,
            AuthorityPolicy::ImplementationObservation,
        ),
        (
            crate::details::RELATION_PREDICATE,
            AuthorityPolicy::ImplementationObservation,
        ),
        (
            crate::details::DISPOSITION_PREDICATE,
            AuthorityPolicy::UserOwned,
        ),
    ] {
        entries.insert(PredicateId::parse(predicate)?, policy);
    }
    let mut descriptors = Vec::new();
    let mut domains = BTreeSet::new();
    for accepted in core.ledger().accepted_events() {
        domains.insert(accepted.event.domain_id);
        match &accepted.event.payload {
            EventPayload::ArtifactRegistered(descriptor) => descriptors.push(descriptor.clone()),
            EventPayload::ClaimAsserted(claim) if !entries.contains_key(&claim.predicate_id) => {
                return Err(OperationError::UnexpectedState(
                    "synthetic claim has no admitted predicate policy",
                ));
            }
            _ => {}
        }
    }
    let mut stored = academic_portability::verify::read_artifact_descriptors(database)?;
    descriptors.sort_by_key(|value| value.id);
    stored.sort_by_key(|value| value.id);
    if descriptors != stored {
        return Err(OperationError::UnexpectedState(
            "artifact closure differs from signed history",
        ));
    }
    Ok(SyntheticMaterial {
        descriptors,
        domains,
        policies: PredicatePolicies::new("synthetic-detail-policies-v1", entries)?,
        head: history.accept_seq_head,
    })
}

impl SyntheticMaterial {
    fn keyring(&self) -> Result<DomainKeyring, OperationError> {
        let mut keyring = DomainKeyring::new();
        for domain in &self.domains {
            keyring.insert(*domain, FIXTURE_LOCATOR_KEY)?;
        }
        Ok(keyring)
    }

    fn targets(&self) -> Vec<ProjectionRebuildTarget> {
        self.domains
            .iter()
            .flat_map(|domain| {
                PHASE1_PROJECTION_KINDS
                    .iter()
                    .map(|kind| ProjectionRebuildTarget {
                        kind: *kind,
                        domain: *domain,
                        coordinates: ProjectionCoordinates::new(
                            self.head,
                            PHASE1_PROJECTION_VALID_AT,
                        ),
                    })
            })
            .collect()
    }
}

/// Returns the frozen policy registry, failing closed on an unlisted predicate.
pub fn fixture_predicate_policies() -> Result<PredicatePolicies, OperationError> {
    let material = fixture_material()?;
    let mut entries = BTreeMap::new();
    for (predicate, policy) in PHASE1_PREDICATE_POLICIES {
        entries.insert(PredicateId::parse(*predicate)?, *policy);
    }
    if material
        .predicates
        .iter()
        .any(|predicate| !entries.contains_key(predicate))
    {
        return Err(OperationError::UnexpectedState(
            "fixture asserts a predicate with no frozen authority policy",
        ));
    }
    Ok(PredicatePolicies::new(
        PHASE1_POLICY_REGISTRY_VERSION,
        entries,
    )?)
}

/// Returns the stable builder identity bound into every Phase 1 generation.
#[must_use]
pub fn projection_builder_digest() -> ContentDigest {
    ContentDigest::sha256(
        concat!(
            "learning-platform.phase1.projection-builder.v1/",
            env!("CARGO_PKG_VERSION")
        )
        .as_bytes(),
    )
}

/// Returns the stable effective-configuration hash for Phase 1 generations.
#[must_use]
pub fn projection_config_hash() -> ContentDigest {
    ContentDigest::sha256(
        format!(
            "learning-platform.phase1.projection-config.v1/{PHASE1_POLICY_REGISTRY_VERSION}/{}",
            PHASE1_PROJECTION_VALID_AT.value()
        )
        .as_bytes(),
    )
}

/// Returns every generation a restore must rebuild at one acceptance watermark.
pub fn projection_targets(
    known_at_accept_seq: u64,
) -> Result<Vec<ProjectionRebuildTarget>, OperationError> {
    let material = fixture_material()?;
    let coordinates = ProjectionCoordinates::new(known_at_accept_seq, PHASE1_PROJECTION_VALID_AT);
    let mut targets = Vec::new();
    for domain in fixture_domains(&material) {
        for kind in PHASE1_PROJECTION_KINDS {
            targets.push(ProjectionRebuildTarget {
                kind: *kind,
                domain,
                coordinates,
            });
        }
    }
    Ok(targets)
}

/// Returns the independent trust anchors a replay or restore must be given.
///
/// A signing key carried inside a stored envelope authenticates nothing, so the
/// anchor always comes from the build rather than from restored bytes.
pub fn fixture_authorizations() -> Result<Vec<DeviceAuthorization>, OperationError> {
    Ok(vec![fixture_device_authorization()?])
}

/// Builds the canonical Phase 1 synthetic-ingest request.
///
/// Every identifier is derived deterministically from the fixture name, so a
/// repeated ingest presents the same `(client_instance_id, idempotency_key)`
/// pair and the daemon returns the original stored receipt instead of
/// accepting the batch twice. `fixture_id` is passed through unchanged and is
/// checked against the allowlist by both the caller and the daemon; it is not
/// a path and never selects a file.
pub fn synthetic_ingest_request(
    fixture_id: &str,
    expected_profile_revision: Option<u64>,
) -> Result<MutableRequest, OperationError> {
    let client_instance = ContentDigest::sha256(CLI_CLIENT_INSTANCE_DOMAIN);
    let idempotency =
        ContentDigest::sha256(&[CLI_INGEST_IDEMPOTENCY_DOMAIN, fixture_id.as_bytes()].concat());
    let request =
        ContentDigest::sha256(&[CLI_INGEST_REQUEST_DOMAIN, fixture_id.as_bytes()].concat());
    let mut request = MutableRequest {
        request_id: request.as_bytes()[..16].to_vec(),
        client_instance_id: client_instance.as_bytes()[..16].to_vec(),
        idempotency_key: idempotency.as_bytes().to_vec(),
        request_digest: vec![0; 32],
        expected_profile_revision,
        capability_id: SYNTHETIC_INGEST_CAPABILITY.to_owned(),
        command: Some(mutable_request::Command::SyntheticIngest(
            SyntheticIngestCommand {
                synthetic_fixture_id: fixture_id.to_owned(),
            },
        )),
    };
    request.request_digest = crate::local_service::mutable_request_digest(&request)
        .map_err(OperationError::Rpc)?
        .as_bytes()
        .to_vec();
    Ok(request)
}

/// Whether a profile already existed or was created by this call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProfileDisposition {
    /// A complete profile was already present and was opened.
    Opened,
    /// The root was missing or empty, so a new throwaway profile was created.
    Created,
}

/// Returns the creating-build identity stamped into a new profile.
#[must_use]
pub fn creating_build_digest() -> [u8; 32] {
    *ContentDigest::sha256(
        concat!(
            "learning-platform.phase1.cli-build.v1/",
            env!("CARGO_PKG_VERSION")
        )
        .as_bytes(),
    )
    .as_bytes()
}

/// Opens a synthetic profile, creating a new throwaway one when none exists.
///
/// Creation is only ever performed against a missing or empty root, and it goes
/// through the same fail-closed path policy as every other profile: a root that
/// is remote, inside a repository, inside a known sync folder, reached through a
/// link, or of unknown storage capability is refused before any file is made.
/// The synthetic-only marker file is written by that path, not by this one.
pub fn ensure_synthetic_profile(profile_root: &Path) -> Result<ProfileDisposition, OperationError> {
    let probe = NativePathProbe::default();
    let exists = profile_root
        .join(academic_store::STORE_DATABASE_FILE)
        .is_file();
    if exists {
        academic_store::profile::open_synthetic_profile(profile_root, &probe)?;
        return Ok(ProfileDisposition::Opened);
    }
    academic_store::profile::create_synthetic_profile(
        profile_root,
        &probe,
        creating_build_digest(),
    )?;
    Ok(ProfileDisposition::Created)
}

/// One deep-doctor finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Severity class consumed by the exit-code mapping.
    pub severity: Severity,
    /// Human-readable detail without any profile content.
    pub detail: String,
}

/// Severity of one doctor finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    /// Recorded for operator attention; the profile remains usable.
    Warning,
    /// The profile must be repaired before it is served or published.
    RepairRequired,
}

/// Canonical watermarks and counts read from one profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanonicalState {
    /// Highest assigned replica-local acceptance sequence.
    pub accept_seq_head: u64,
    /// Highest assigned projection outbox sequence.
    pub outbox_head: u64,
    /// Current profile revision.
    pub profile_revision: u64,
    /// Accepted canonical batches.
    pub batches: u64,
    /// Accepted canonical events.
    pub events: u64,
    /// Registered artifact descriptors.
    pub artifacts: u64,
    /// Stored immutable command receipts.
    pub command_receipts: u64,
    /// Distinct device origin chains.
    pub devices: u64,
}

/// Physical store identity read back from the profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreIdentity {
    /// Numeric physical schema version.
    pub schema_version: u32,
    /// Semantic schema version.
    pub schema_semver: String,
    /// Storage mode recorded by `schema_meta`.
    pub storage_mode: String,
    /// At-rest encryption recorded by `schema_meta`.
    pub storage_encryption: String,
}

/// Watermark of one active projection generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionWatermark {
    /// Generation kind.
    pub kind: String,
    /// Security domain the generation covers.
    pub domain: String,
    /// Outbox sequence the active generation was built from, when one exists.
    pub source_outbox_seq: Option<u64>,
    /// Distance between the canonical outbox head and this generation.
    pub lag: u64,
    /// Whether an active generation exists at all.
    pub active: bool,
}

/// Complete read-only diagnosis of one synthetic profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileDiagnosis {
    /// Whether the deep pass ran.
    pub deep: bool,
    /// Whether the synthetic-only marker file is present.
    pub synthetic_marker_present: bool,
    /// Physical store identity.
    pub store: StoreIdentity,
    /// Canonical watermarks and counts.
    pub canonical: CanonicalState,
    /// `PRAGMA integrity_check` result, when the deep pass ran.
    pub integrity_check: Option<bool>,
    /// `PRAGMA foreign_key_check` result, when the deep pass ran.
    pub foreign_key_check: Option<bool>,
    /// Unpublished vault temp entries observed.
    pub orphan_temp_entries: Vec<String>,
    /// Quarantined vault entries observed.
    pub quarantined_entries: Vec<String>,
    /// Per-generation projection watermarks.
    pub projections: Vec<ProjectionWatermark>,
    /// Every finding, ordered as produced.
    pub findings: Vec<Finding>,
}

impl ProfileDiagnosis {
    /// Returns whether any finding demands repair before the profile is served.
    #[must_use]
    pub fn repair_required(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == Severity::RepairRequired)
    }
}

/// Unpublished ingest temp files are `<ingest-session>.partial`.
const VAULT_TEMP_EXTENSION: &str = "partial";
/// Quarantined objects are `<timestamp>-<locator>.orphan`.
const VAULT_QUARANTINE_EXTENSION: &str = "orphan";

/// Lists the vault entries of one physical kind inside a namespace directory.
///
/// Only files carrying the documented extension are reported. The directory
/// also holds the vault's own barrier marker, which is structure rather than
/// residue and must never be counted as an orphan.
fn vault_entry_names(directory: &Path, extension: &str) -> Result<Vec<String>, OperationError> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error("read vault directory", directory, error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| io_error("read vault entry", directory, error))?;
        let path = entry.path();
        if path.extension().is_some_and(|value| value == extension) {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// Reads one synthetic profile and reports its health without writing to it.
///
/// The shallow pass reads store identity, canonical watermarks, and the
/// synthetic marker. The deep pass adds `integrity_check`, `foreign_key_check`,
/// unpublished vault temp entries, quarantine disposition, and projection lag
/// against the canonical outbox head.
pub fn diagnose_profile(
    profile_root: &Path,
    deep: bool,
) -> Result<ProfileDiagnosis, OperationError> {
    let database_path = profile_root.join(academic_store::STORE_DATABASE_FILE);
    // A directory that is not a profile is a refused location, not a fault. It
    // is checked here rather than being read out of SQLite's "unable to open
    // database file", which a caller cannot distinguish from a real open
    // failure and which carries no reason it can branch on.
    if !database_path.is_file() {
        return Err(OperationError::Store(StoreError::UnsafeProfilePath(
            PathPolicyViolation::MissingStoreDatabase,
        )));
    }
    let database = CanonicalDatabase::open_source(&database_path)?;
    let rows = read_canonical_rows(&database)?;
    rows.schema.policy.require_phase1()?;
    let material = verified_synthetic_material(&database)?;

    let mut findings = Vec::new();
    let marker = profile_root.join(SYNTHETIC_PROFILE_MARKER);
    let synthetic_marker_present = marker.is_file();
    if !synthetic_marker_present {
        findings.push(Finding {
            code: "SYNTHETIC_MARKER_MISSING",
            severity: Severity::RepairRequired,
            detail: format!("{SYNTHETIC_PROFILE_MARKER} is absent from the profile root"),
        });
    }

    let reader = open_reader(&database_path)?;
    let snapshot = canonical_snapshot(&reader)?;
    let canonical = CanonicalState {
        accept_seq_head: snapshot.accept_seq_head,
        outbox_head: snapshot.outbox_head,
        profile_revision: snapshot.profile_revision,
        batches: snapshot.batch_count,
        events: snapshot.event_count,
        artifacts: snapshot.artifact_count,
        command_receipts: snapshot.receipt_count,
        devices: snapshot.device_count,
    };
    let store = StoreIdentity {
        schema_version: rows.schema.schema_version,
        schema_semver: rows.schema.schema_semver.clone(),
        storage_mode: rows.schema.policy.storage_mode.clone(),
        storage_encryption: rows.schema.policy.storage_encryption.clone(),
    };

    let mut integrity_check = None;
    let mut foreign_key_check = None;
    let mut orphan_temp_entries = Vec::new();
    let mut quarantined_entries = Vec::new();
    let mut projections = Vec::new();

    if deep {
        let integrity = database.integrity_check().is_ok();
        integrity_check = Some(integrity);
        if !integrity {
            findings.push(Finding {
                code: "INTEGRITY_CHECK_FAILED",
                severity: Severity::RepairRequired,
                detail: "PRAGMA integrity_check did not return ok".to_owned(),
            });
        }
        let foreign_keys = database.foreign_key_check().is_ok();
        foreign_key_check = Some(foreign_keys);
        if !foreign_keys {
            findings.push(Finding {
                code: "FOREIGN_KEY_CHECK_FAILED",
                severity: Severity::RepairRequired,
                detail: "PRAGMA foreign_key_check reported violations".to_owned(),
            });
        }

        let vault = Vault::open(profile_root, material.keyring()?)?;
        for descriptor in &material.descriptors {
            let _ = vault.verify_sealed_object(descriptor)?;
        }
        orphan_temp_entries = vault_entry_names(vault.layout().temp_dir(), VAULT_TEMP_EXTENSION)?;
        if !orphan_temp_entries.is_empty() {
            findings.push(Finding {
                code: "ORPHAN_TEMP_PRESENT",
                severity: Severity::RepairRequired,
                detail: format!(
                    "{} unpublished vault temp entries remain",
                    orphan_temp_entries.len()
                ),
            });
        }
        quarantined_entries =
            vault_entry_names(vault.layout().quarantine_dir(), VAULT_QUARANTINE_EXTENSION)?;
        if !quarantined_entries.is_empty() {
            findings.push(Finding {
                code: "QUARANTINED_OBJECTS_PRESENT",
                severity: Severity::Warning,
                detail: format!(
                    "{} quarantined vault entries await disposition",
                    quarantined_entries.len()
                ),
            });
        }

        let sidecar = profile_root.join(PROJECTION_SIDECAR_FILE);
        let runner = ProjectionRunner::open(
            &reader,
            &sidecar,
            projection_builder_digest(),
            projection_config_hash(),
        )?;
        for domain in material.domains {
            for kind in PHASE1_PROJECTION_KINDS {
                let active = runner.active_generation(*kind, domain)?;
                let source_outbox_seq = active.as_ref().map(|value| value.source_outbox_seq);
                let lag = snapshot
                    .outbox_head
                    .saturating_sub(source_outbox_seq.unwrap_or(0));
                if lag > 0 {
                    findings.push(Finding {
                        code: "PROJECTION_LAG",
                        severity: Severity::Warning,
                        detail: format!(
                            "{} lags the canonical outbox head by {lag}",
                            kind.as_str()
                        ),
                    });
                }
                projections.push(ProjectionWatermark {
                    kind: kind.as_str().to_owned(),
                    domain: domain.to_string(),
                    source_outbox_seq,
                    lag,
                    active: active.is_some(),
                });
            }
        }
    }

    Ok(ProfileDiagnosis {
        deep,
        synthetic_marker_present,
        store,
        canonical,
        integrity_check,
        foreign_key_check,
        orphan_temp_entries,
        quarantined_entries,
        projections,
        findings,
    })
}

/// Writes one deterministic export directory for a synthetic profile.
pub fn export_synthetic_profile(
    profile_root: &Path,
    destination: &Path,
) -> Result<ExportReceipt, OperationError> {
    academic_store::profile::require_profile_format(profile_root)?;
    let database =
        CanonicalDatabase::open_source(&profile_root.join(academic_store::STORE_DATABASE_FILE))?;
    let material = verified_synthetic_material(&database)?;
    Ok(export_profile(
        profile_root,
        destination,
        material.keyring()?,
    )?)
}

/// Publishes one plaintext synthetic backup directory.
pub fn backup_synthetic_profile(
    profile_root: &Path,
    destination: &Path,
) -> Result<BackupReceipt, OperationError> {
    academic_store::profile::require_profile_format(profile_root)?;
    let database =
        CanonicalDatabase::open_source(&profile_root.join(academic_store::STORE_DATABASE_FILE))?;
    let material = verified_synthetic_material(&database)?;
    Ok(backup_profile(
        profile_root,
        destination,
        material.keyring()?,
    )?)
}

/// Restores one verified backup into a new empty profile directory.
///
/// The destination must be new and empty, the trust anchors come from this
/// build rather than from the restored bytes, and every projection generation
/// is rebuilt from empty before the profile is published.
pub fn restore_synthetic_profile(
    backup_root: &Path,
    destination: &Path,
) -> Result<RestoreReceipt, OperationError> {
    let authorizations = fixture_authorizations()?;
    Ok(restore_profile_with_material(
        backup_root,
        destination,
        &NativePathProbe::default(),
        &authorizations,
        |database| {
            synthetic_restore_material(database).map_err(|error| PortabilityError::ReplayMismatch {
                subject: "synthetic restore material",
                detail: error.to_string(),
            })
        },
    )?)
}

fn synthetic_restore_material(
    database: &CanonicalDatabase,
) -> Result<RestoreMaterial, OperationError> {
    let material = verified_synthetic_material(database)?;
    Ok(RestoreMaterial {
        keyring: material.keyring()?,
        projections: material.targets(),
        predicate_policies: Some(material.policies),
        projection_builder_digest: projection_builder_digest(),
        projection_config_hash: projection_config_hash(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_policy_table_covers_exactly_the_fixture_predicates()
    -> Result<(), Box<dyn std::error::Error>> {
        let material = fixture_material()?;
        let frozen = PHASE1_PREDICATE_POLICIES
            .iter()
            .map(|(predicate, _)| (*predicate).to_owned())
            .collect::<BTreeSet<_>>();
        let asserted = material
            .predicates
            .iter()
            .map(|predicate| predicate.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            frozen, asserted,
            "the frozen authority-policy table must match the fixture exactly"
        );
        fixture_predicate_policies()?;
        Ok(())
    }

    #[test]
    fn capability_constants_are_declared_by_the_frozen_protocol() {
        for capability in [
            SYNTHETIC_INGEST_CAPABILITY,
            DIAGNOSTICS_CAPABILITY,
            SYNTHETIC_EXPORT_CAPABILITY,
        ] {
            assert!(
                academic_rpc::PHASE1_CAPABILITY_IDS.contains(&capability),
                "{capability} is not a declared Phase 1 capability"
            );
        }
    }

    #[test]
    fn a_repeated_ingest_request_is_byte_identical() -> Result<(), Box<dyn std::error::Error>> {
        let first = synthetic_ingest_request(PHASE1_SYNTHETIC_FIXTURE_ID, None)?;
        let second = synthetic_ingest_request(PHASE1_SYNTHETIC_FIXTURE_ID, None)?;
        assert_eq!(first, second, "a retry must present the same receipt key");
        assert_eq!(first.idempotency_key.len(), 32);
        assert_eq!(first.request_id.len(), 16);
        assert_eq!(first.client_instance_id.len(), 16);
        let other = synthetic_ingest_request("some-other-fixture", None)?;
        assert_ne!(
            first.idempotency_key, other.idempotency_key,
            "a different fixture name must not reuse the receipt key"
        );
        Ok(())
    }

    #[test]
    fn projection_identity_is_stable_across_calls() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(projection_builder_digest(), projection_builder_digest());
        assert_eq!(projection_config_hash(), projection_config_hash());
        assert_ne!(
            projection_builder_digest(),
            projection_config_hash(),
            "builder identity and configuration identity must stay distinct"
        );
        Ok(())
    }

    #[test]
    fn projection_targets_cover_every_kind_in_every_fixture_domain()
    -> Result<(), Box<dyn std::error::Error>> {
        let material = fixture_material()?;
        let targets = projection_targets(7)?;
        assert_eq!(
            targets.len(),
            fixture_domains(&material).len() * PHASE1_PROJECTION_KINDS.len()
        );
        assert!(
            targets
                .iter()
                .all(|target| target.coordinates.known_at_accept_seq == 7)
        );
        Ok(())
    }
}
