//! Observation and verdict for one Phase 1 exit run.
//!
//! The harness in `crates/daemon/tests/phase1_exit.rs` performs the sequence.
//! This module is the part that decides what the sequence *showed*, and it is
//! deliberately separate so a fault's expected letter is never derived from the
//! same statement that produced the observation.
//!
//! Two invariants are checked before any letter is assigned, because they are
//! stronger than the letters and no fault is allowed to violate either:
//!
//! - **No normal-looking partial state.** Canonical counts are either exactly
//!   absent or exactly the reference set. Anything between the two is
//!   [`Completeness::Partial`] and fails the run outright, whatever the matrix
//!   says the letter should be.
//! - **No normal-looking reference to a missing or corrupt object.** Every
//!   canonical artifact reference must resolve through the vault. The
//!   deterministic export is that proof: it reads each referenced object back
//!   through the vault and refuses to publish if one is missing, so an export
//!   whose object count equals the canonical artifact count is a closure
//!   receipt rather than a restatement of the database.
//!
//! This file is included with `#[path]` by the crate that owns the harness
//! test, so `academic-test-support` keeps no dependency edge. It expects
//! `fault_driver` to be declared beside it at the test crate root.

#![allow(dead_code)]

use std::{
    collections::BTreeSet,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use academic_core::operations::ProfileDiagnosis;
use academic_portability::{
    PortabilityResult,
    verify::{
        CanonicalCounts, CanonicalDatabase, CanonicalWatermark, DeviceHeadRow,
        read_artifact_descriptors, read_canonical_rows,
    },
};

use super::fault_driver::{FaultSpec, FaultSubject, Outcome};
use super::process::ChildRecord;

/// Unpublished ingest temp files are `<ingest-session>.partial`.
const VAULT_TEMP_EXTENSION: &str = "partial";
/// Quarantined objects are `<timestamp>-<locator>.orphan`.
const VAULT_QUARANTINE_EXTENSION: &str = "orphan";
/// Published sealed objects are `<hmac-locator>.obj`.
const VAULT_OBJECT_EXTENSION: &str = "obj";

/// Reconciliation states that count as an explicit orphan disposition.
///
/// These are the `Debug` spellings of `academic_vault::ReconcileState`. The
/// harness reads them as text rather than taking a dependency edge on the vault
/// for one enum, and asserts membership in this frozen set, so a variant rename
/// fails the exit run loudly instead of silently widening what counts as
/// "disposed".
pub const EXPLICIT_ORPHAN_DISPOSITIONS: &[&str] = &[
    "ValidOrphan",
    "QuarantinedOrphan",
    "OrphanPendingGrace",
    "OrphanLeaseHeld",
];

/// Reconciliation states that mean the profile must not be served.
pub const REPAIR_REQUIRED_DISPOSITIONS: &[&str] = &[
    "ReferencedMissingRepairRequired",
    "ReferencedCorruptRepairRequired",
    "UnsafeEntry",
];

/// Every reconciliation state the vault can assign, as `Debug` spells them.
pub const KNOWN_RECONCILE_STATES: &[&str] = &[
    "TempLive",
    "TempExpiredRemoved",
    "OrphanPendingGrace",
    "OrphanLeaseHeld",
    "ValidOrphan",
    "QuarantinedOrphan",
    "ReferencedValid",
    "ReferencedMissingRepairRequired",
    "ReferencedCorruptRepairRequired",
    "UnsafeEntry",
];

/// Identity-free canonical facts of one profile.
///
/// The store's own semantic digest covers `schema_meta`, which carries a
/// per-profile format UUID, so it is comparable **within** one profile lineage
/// — source against its own restore — and never across two independently
/// created profiles. Everything else here is identity-free and is what the
/// cross-profile comparison uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFacts {
    /// Acceptance and revision watermarks.
    pub watermark: CanonicalWatermark,
    /// Canonical row counts.
    pub counts: CanonicalCounts,
    /// Device origin heads.
    pub device_heads: Vec<DeviceHeadRow>,
    /// Canonical semantic digest, including per-profile store identity.
    pub semantic_digest: String,
    /// Artifact identifiers the canonical rows reference.
    pub artifact_ids: Vec<String>,
}

impl CanonicalFacts {
    /// Reads the canonical facts of one profile without writing to it.
    pub fn read(profile_root: &Path) -> PortabilityResult<Self> {
        let database_path = profile_root.join(academic_store::STORE_DATABASE_FILE);
        let database = CanonicalDatabase::open_source(&database_path)?;
        let rows = read_canonical_rows(&database)?;
        let semantic_digest = hex_lower(rows.semantic_digest()?.as_bytes());
        let mut artifact_ids = read_artifact_descriptors(&database)?
            .into_iter()
            .map(|descriptor| descriptor.id.to_string())
            .collect::<Vec<_>>();
        artifact_ids.sort();
        Ok(Self {
            watermark: rows.watermark,
            counts: rows.counts,
            device_heads: rows.device_heads.clone(),
            semantic_digest,
            artifact_ids,
        })
    }

    /// Returns whether the canonical tables are completely empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.counts.batches == 0
            && self.counts.events == 0
            && self.counts.scopes == 0
            && self.counts.artifacts == 0
            && self.counts.evidence == 0
            && self.counts.claims == 0
            && self.counts.relations == 0
            && self.counts.decisions == 0
            && self.counts.outbox == 0
            && self.counts.command_receipts == 0
            && self.counts.device_heads == 0
            && self.watermark.accept_seq_head == 0
            && self.watermark.outbox_head == 0
            && self.watermark.profile_revision == 0
            && self.watermark.next_accept_seq == 1
    }

    /// Returns the device heads without their wall-clock update stamp.
    ///
    /// `updated_at_unix_ms` records when this replica accepted the batch, so it
    /// differs between two profiles that accepted the same signed bytes at
    /// different moments. It is replica bookkeeping, not canonical semantics,
    /// and comparing it across profiles would assert something false. The
    /// device identity, its next origin sequence, and the exact head batch and
    /// envelope digest are the parts that must agree.
    #[must_use]
    pub fn device_head_identities(&self) -> Vec<(&str, u64, &str, &str)> {
        self.device_heads
            .iter()
            .map(|head| {
                (
                    head.device_id.as_str(),
                    head.next_origin_seq,
                    head.head_batch_id.as_str(),
                    head.head_envelope_sha256.as_str(),
                )
            })
            .collect()
    }

    /// Returns whether every identity-free fact equals a reference profile's.
    ///
    /// Two things are excluded on purpose. The semantic digest covers
    /// `schema_meta`, and two independently created profiles mint different
    /// store identities; within one lineage use [`Self::matches_lineage`]
    /// instead. The device heads' wall-clock update stamp is replica
    /// bookkeeping; see [`Self::device_head_identities`].
    #[must_use]
    pub fn matches_reference(&self, reference: &Self) -> bool {
        self.watermark == reference.watermark
            && self.counts == reference.counts
            && self.device_head_identities() == reference.device_head_identities()
            && self.artifact_ids == reference.artifact_ids
    }

    /// Returns whether this profile is byte-for-byte the same canonical
    /// semantics as another in the same lineage, store identity included.
    #[must_use]
    pub fn matches_lineage(&self, other: &Self) -> bool {
        self.matches_reference(other) && self.semantic_digest == other.semantic_digest
    }

    /// Renders the receipt line for the canonical heads and counts.
    #[must_use]
    pub fn receipt_line(&self) -> String {
        format!(
            "accept_seq_head={} outbox_head={} revision={} next_accept_seq={} \
             batches={} events={} artifacts={} receipts={} outbox={} device_heads={} \
             semantic_digest={}",
            self.watermark.accept_seq_head,
            self.watermark.outbox_head,
            self.watermark.profile_revision,
            self.watermark.next_accept_seq,
            self.counts.batches,
            self.counts.events,
            self.counts.artifacts,
            self.counts.command_receipts,
            self.counts.outbox,
            self.counts.device_heads,
            self.semantic_digest
        )
    }
}

/// Whether one observed canonical state is empty, complete, or neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completeness {
    /// No canonical row exists and no sequence was consumed.
    Absent,
    /// Every canonical row of the reference acceptance exists.
    Complete,
    /// Something in between, which the contract never allows.
    Partial(String),
}

impl fmt::Display for Completeness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => formatter.write_str("ABSENT"),
            Self::Complete => formatter.write_str("COMPLETE"),
            Self::Partial(reason) => write!(formatter, "PARTIAL({reason})"),
        }
    }
}

/// Classifies one observed canonical state against the fault-free reference.
#[must_use]
pub fn completeness(observed: &CanonicalFacts, reference: &CanonicalFacts) -> Completeness {
    if observed.is_empty() {
        return Completeness::Absent;
    }
    if observed.matches_reference(reference) {
        return Completeness::Complete;
    }
    Completeness::Partial(format!(
        "observed [{}] is neither empty nor the reference [{}]",
        observed.receipt_line(),
        reference.receipt_line()
    ))
}

/// Physical vault residue observed at one instant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VaultInventory {
    /// Unpublished `*.partial` temp files.
    pub partials: Vec<String>,
    /// Quarantined `*.orphan` files.
    pub quarantined: Vec<String>,
    /// Published sealed `*.obj` files.
    pub sealed_objects: Vec<String>,
}

impl VaultInventory {
    /// Walks one profile's vault and records what is physically present.
    pub fn read(profile_root: &Path) -> io::Result<Self> {
        let vault = profile_root.join("vault");
        Ok(Self {
            partials: names_with_extension(&vault.join("tmp"), VAULT_TEMP_EXTENSION)?,
            quarantined: names_with_extension(
                &vault.join("quarantine"),
                VAULT_QUARANTINE_EXTENSION,
            )?,
            sealed_objects: names_with_extension(&vault.join("v1"), VAULT_OBJECT_EXTENSION)?,
        })
    }

    /// Returns whether any recoverable residue is present.
    #[must_use]
    pub fn has_residue(&self) -> bool {
        !self.partials.is_empty() || !self.quarantined.is_empty()
    }

    /// Renders the receipt line for physical vault state.
    #[must_use]
    pub fn receipt_line(&self) -> String {
        format!(
            "partials={} quarantined={} sealed_objects={}",
            self.partials.len(),
            self.quarantined.len(),
            self.sealed_objects.len()
        )
    }
}

/// Lists file names carrying one extension below a directory tree.
fn names_with_extension(root: &Path, extension: &str) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    collect_with_extension(root, extension, &mut names)?;
    names.sort();
    Ok(names)
}

fn collect_with_extension(
    directory: &Path,
    extension: &str,
    names: &mut Vec<String>,
) -> io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_with_extension(&path, extension, names)?;
        } else if path.extension().is_some_and(|value| value == extension) {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(())
}

/// Deep-doctor facts reduced to what a verdict needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorFacts {
    /// Whether the deep pass ran.
    pub deep: bool,
    /// Whether the synthetic-only marker file is present.
    pub synthetic_marker_present: bool,
    /// `PRAGMA integrity_check` result.
    pub integrity_check: Option<bool>,
    /// `PRAGMA foreign_key_check` result.
    pub foreign_key_check: Option<bool>,
    /// Unpublished vault temp entries the doctor observed.
    pub orphan_temp_entries: Vec<String>,
    /// Quarantined vault entries the doctor observed.
    pub quarantined_entries: Vec<String>,
    /// Every finding code, with its severity, in the order produced.
    pub findings: Vec<String>,
    /// Whether any finding demands repair before the profile is served.
    pub repair_required: bool,
    /// Whether every declared projection generation is active with zero lag.
    pub projections_current: bool,
}

impl DoctorFacts {
    /// Reduces one profile diagnosis to the facts a verdict needs.
    #[must_use]
    pub fn from_diagnosis(diagnosis: &ProfileDiagnosis) -> Self {
        Self {
            deep: diagnosis.deep,
            synthetic_marker_present: diagnosis.synthetic_marker_present,
            integrity_check: diagnosis.integrity_check,
            foreign_key_check: diagnosis.foreign_key_check,
            orphan_temp_entries: diagnosis.orphan_temp_entries.clone(),
            quarantined_entries: diagnosis.quarantined_entries.clone(),
            findings: diagnosis
                .findings
                .iter()
                .map(|finding| format!("{}:{:?}", finding.code, finding.severity))
                .collect(),
            repair_required: diagnosis.repair_required(),
            projections_current: !diagnosis.projections.is_empty()
                && diagnosis
                    .projections
                    .iter()
                    .all(|projection| projection.active && projection.lag == 0),
        }
    }

    /// Returns whether the physical and logical health checks all passed.
    #[must_use]
    pub fn health_checks_passed(&self) -> bool {
        self.synthetic_marker_present
            && self.integrity_check == Some(true)
            && self.foreign_key_check == Some(true)
    }

    /// Renders the receipt line for the deep doctor.
    #[must_use]
    pub fn receipt_line(&self) -> String {
        format!(
            "deep={} marker={} integrity={:?} foreign_keys={:?} orphan_temps={} quarantined={} \
             repair_required={} projections_current={} findings=[{}]",
            self.deep,
            self.synthetic_marker_present,
            self.integrity_check,
            self.foreign_key_check,
            self.orphan_temp_entries.len(),
            self.quarantined_entries.len(),
            self.repair_required,
            self.projections_current,
            self.findings.join(",")
        )
    }
}

/// What the restart's own reconciliation decided.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileFacts {
    /// Every state the pass assigned, as the vault's `Debug` spells it.
    pub states: Vec<String>,
}

impl ReconcileFacts {
    /// Returns whether any state is outside the frozen vocabulary.
    #[must_use]
    pub fn unknown_states(&self) -> Vec<String> {
        self.states
            .iter()
            .filter(|state| !KNOWN_RECONCILE_STATES.contains(&state.as_str()))
            .cloned()
            .collect()
    }

    /// Returns whether the pass explicitly disposed of an orphan.
    #[must_use]
    pub fn disposed_an_orphan(&self) -> bool {
        self.states
            .iter()
            .any(|state| EXPLICIT_ORPHAN_DISPOSITIONS.contains(&state.as_str()))
    }

    /// Returns whether the pass refused to serve the profile.
    #[must_use]
    pub fn repair_required(&self) -> bool {
        self.states
            .iter()
            .any(|state| REPAIR_REQUIRED_DISPOSITIONS.contains(&state.as_str()))
    }

    /// Renders the receipt line for the explicit orphan disposition.
    #[must_use]
    pub fn receipt_line(&self) -> String {
        let unique = self.states.iter().collect::<BTreeSet<_>>();
        format!(
            "records={} states=[{}]",
            self.states.len(),
            unique
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

/// One idempotent-retry exchange over local IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryFacts {
    /// `ACCEPTED`, `DUPLICATE`, `REJECTED`, or `UNSPECIFIED`.
    pub status: String,
    /// Stable rejection reason, empty when the command was not rejected.
    pub reason: String,
    /// Canonical revision the response reported.
    pub profile_revision: u64,
    /// Lowercase hex of the immutable receipt identifier.
    pub receipt_id: String,
    /// Lowercase hex of the response digest.
    pub response_digest: String,
    /// Accepted sequence range, when the response carried one.
    pub acceptance_range: Option<(u64, u64)>,
}

impl RetryFacts {
    /// Returns whether the daemon replayed a stored receipt.
    #[must_use]
    pub fn is_duplicate(&self) -> bool {
        self.status == "DUPLICATE"
    }

    /// Returns whether the daemon accepted the command for the first time.
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        self.status == "ACCEPTED"
    }

    /// Returns whether two exchanges returned the identical receipt.
    #[must_use]
    pub fn is_identical_to(&self, other: &Self) -> bool {
        self.receipt_id == other.receipt_id
            && self.response_digest == other.response_digest
            && self.profile_revision == other.profile_revision
            && self.acceptance_range == other.acceptance_range
    }

    /// Renders the receipt line for one retry exchange.
    #[must_use]
    pub fn receipt_line(&self) -> String {
        format!(
            "status={} reason={} revision={} receipt_id={} response_digest={} range={:?}",
            self.status,
            if self.reason.is_empty() {
                "-"
            } else {
                self.reason.as_str()
            },
            self.profile_revision,
            self.receipt_id,
            self.response_digest,
            self.acceptance_range
        )
    }
}

/// What the fault's own subject was observed to have become.
///
/// The harness fills exactly the variant its fault's `FaultSubject` names, so
/// the oracle never has to guess which of several overlapping physical facts
/// the matrix row is talking about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectDisposition {
    /// An interrupted seal's temp file.
    VaultTemp {
        /// Unpublished `*.partial` files still present after the restart.
        partials_after_restart: usize,
        /// Whether the canonical tables are empty after the restart.
        canonical_absent: bool,
    },
    /// A complete sealed object with no canonical row referencing it.
    SealedObject {
        /// Whether a sealed object existed before the restart.
        sealed_before_restart: bool,
        /// Whether reconciliation recorded an explicit disposition for it.
        explicit_disposition: bool,
        /// Whether the canonical tables are empty after the restart.
        canonical_absent: bool,
    },
    /// The atomic canonical acceptance unit.
    CanonicalTransaction {
        /// Whether the transaction is absent, complete, or neither.
        completeness: Completeness,
        /// Whether every canonical artifact reference resolved on export.
        object_closure_holds: bool,
    },
    /// A generation that was still building or activating.
    ProjectionGeneration {
        /// Whether a generation is queryable as active while disagreeing with
        /// the canonical outbox head.
        ///
        /// This is the measurable form of "a killed generation never becomes
        /// queryable". `PR01` and `PR02` are killed before activation, so no
        /// pointer may exist at all; `PR03` is killed inside the activation
        /// transaction, where the matrix allows either the old or the new
        /// pointer as long as pointer and cursor agree. Both readings collapse
        /// to the same check: any generation the profile will serve must match
        /// the canonical head exactly.
        inconsistent_generation_active: bool,
        /// Whether a clean rebuild afterwards activated every generation.
        clean_rebuild_activated: bool,
        /// Whether the canonical tables were left untouched by the fault.
        canonical_unchanged: bool,
    },
    /// An unpublished backup directory.
    BackupDirectory {
        /// Whether the interrupted destination carries a manifest.
        destination_published: bool,
        /// Whether every unpublished staging root was discoverable and removable.
        unpublished_recoverable: bool,
        /// Whether the source profile is unchanged in canonical terms.
        source_unchanged: bool,
        /// Whether a fresh backup into a new destination published cleanly.
        fresh_publication_succeeded: bool,
    },
    /// An unpublished restore destination.
    RestoreDestination {
        /// Whether the interrupted destination is a publishable profile.
        destination_publishable: bool,
        /// Whether every unpublished staging root was discoverable and removable.
        unpublished_recoverable: bool,
        /// Whether the source profile is unchanged in canonical terms.
        source_unchanged: bool,
        /// Whether the backup the restore was reading is unchanged.
        backup_unchanged: bool,
        /// Whether a fresh restore into a new empty root published cleanly.
        fresh_publication_succeeded: bool,
    },
    /// A complete request that was read but never admitted.
    QueuedRequest {
        /// Whether the canonical tables are empty after the restart.
        canonical_absent: bool,
        /// Whether the client's retry was admitted as a fresh command.
        retry_admitted_fresh: bool,
        /// Whether the retry carried the identical idempotency key.
        same_idempotency_key: bool,
    },
}

impl SubjectDisposition {
    /// Returns the letter this subject's disposition earns.
    ///
    /// `None` means the disposition is not one the matrix allows, which the
    /// caller must report as a failure rather than round to a letter.
    #[must_use]
    pub fn letter(&self) -> Option<Outcome> {
        match self {
            // The temp never became a reference and nothing survived it.
            Self::VaultTemp {
                partials_after_restart,
                canonical_absent,
            } => {
                (*canonical_absent && *partials_after_restart == 0).then_some(Outcome::NoReference)
            }
            // The sealed object is real, so it must be explicitly disposed of
            // rather than left lying unreferenced.
            Self::SealedObject {
                sealed_before_restart,
                explicit_disposition,
                canonical_absent,
            } => (*sealed_before_restart && *explicit_disposition && *canonical_absent)
                .then_some(Outcome::Quarantine),
            // Either the whole transaction is there or none of it is.
            Self::CanonicalTransaction {
                completeness,
                object_closure_holds,
            } => match completeness {
                Completeness::Absent => Some(Outcome::NoReference),
                Completeness::Complete if *object_closure_holds => Some(Outcome::Complete),
                Completeness::Complete | Completeness::Partial(_) => None,
            },
            // No killed generation may be queryable, and the profile must still
            // support a clean rebuild afterwards.
            Self::ProjectionGeneration {
                inconsistent_generation_active,
                clean_rebuild_activated,
                canonical_unchanged,
            } => (!*inconsistent_generation_active
                && *clean_rebuild_activated
                && *canonical_unchanged)
                .then_some(Outcome::NoReference),
            Self::BackupDirectory {
                destination_published,
                unpublished_recoverable,
                source_unchanged,
                fresh_publication_succeeded,
            } => (!*destination_published
                && *unpublished_recoverable
                && *source_unchanged
                && *fresh_publication_succeeded)
                .then_some(Outcome::NoReference),
            Self::RestoreDestination {
                destination_publishable,
                unpublished_recoverable,
                source_unchanged,
                backup_unchanged,
                fresh_publication_succeeded,
            } => (!*destination_publishable
                && *unpublished_recoverable
                && *source_unchanged
                && *backup_unchanged
                && *fresh_publication_succeeded)
                .then_some(Outcome::NoReference),
            Self::QueuedRequest {
                canonical_absent,
                retry_admitted_fresh,
                same_idempotency_key,
            } => (*canonical_absent && *retry_admitted_fresh && *same_idempotency_key)
                .then_some(Outcome::NoReference),
        }
    }

    /// Renders the receipt line for the subject's disposition.
    #[must_use]
    pub fn receipt_line(&self) -> String {
        format!("{self:?}")
    }
}

/// Everything one fault run observed, in the order the sequence produced it.
#[derive(Debug, Clone)]
pub struct FaultEvidence {
    /// Fault identifier this run exercised.
    pub fault_id: String,
    /// Bounded record of the killed child.
    pub child: ChildRecord,
    /// Whether the child's ready marker named exactly this fault.
    pub ready_marker_matched: bool,
    /// Physical vault state observed after the kill, before the restart.
    pub pre_restart_vault: VaultInventory,
    /// Canonical state observed after the kill, before the restart.
    pub pre_restart_canonical: CanonicalFacts,
    /// What the restart's reconciliation decided.
    pub reconcile: ReconcileFacts,
    /// Canonical state observed after the restart, before any retry.
    pub post_restart_canonical: CanonicalFacts,
    /// Deep doctor after the restart, before any retry.
    pub post_restart_doctor: DoctorFacts,
    /// Disposition of the artifact this fault's termination point protects.
    pub subject: SubjectDisposition,
    /// First idempotent retry over local IPC.
    pub first_retry: RetryFacts,
    /// Second idempotent retry over local IPC.
    pub second_retry: RetryFacts,
    /// Canonical state after both retries.
    pub post_retry_canonical: CanonicalFacts,
    /// Number of reachable objects the deterministic export published.
    pub exported_object_count: usize,
    /// Semantic digests of the two exports, which must be equal.
    pub export_semantic_digests: (String, String),
    /// Whether the two exports produced identical per-file hash manifests.
    pub export_file_manifests_match: bool,
    /// Semantic digest the restore reported for the new empty profile.
    pub restored_semantic_digest: String,
    /// Canonical state of the restored profile.
    pub restored_canonical: CanonicalFacts,
    /// Deep doctor on the restored profile after its projection rebuild.
    pub restored_doctor: DoctorFacts,
}

impl FaultEvidence {
    /// Returns whether the canonical transaction is complete after the restart.
    #[must_use]
    pub fn committed(&self, reference: &CanonicalFacts) -> bool {
        completeness(&self.post_restart_canonical, reference) == Completeness::Complete
    }

    /// Returns whether the retry replayed a stored receipt unchanged.
    ///
    /// Three independent things must hold, and each rules out a different way
    /// of faking the letter: the daemon must answer `DUPLICATE` rather than
    /// accepting the batch a second time, two separate exchanges must return a
    /// byte-identical receipt, and the retries must leave every canonical
    /// watermark, count, and digest exactly as the restart found them.
    #[must_use]
    pub fn retry_replayed_original_receipt(&self) -> bool {
        self.first_retry.is_duplicate()
            && self.second_retry.is_duplicate()
            && self.first_retry.is_identical_to(&self.second_retry)
            && self.post_retry_canonical == self.post_restart_canonical
    }

    /// Computes the outcome letters this run actually produced.
    ///
    /// The primary letter is the disposition of the fault's own subject. `R` is
    /// added only when that subject is the canonical transaction *and* the
    /// transaction had already committed, which is exactly the lost-acknowledgement
    /// shape the two `C+R` rows describe.
    ///
    /// The restriction matters. Every fault after the ingest stage retries
    /// against a profile that already holds the fixture, so its retry always
    /// replays a stored receipt. That is worth asserting — and
    /// `phase1_exit.rs` asserts it for all twenty-six — but it is not what those
    /// rows are about, and letting it add an `R` would hand every later row a
    /// letter its termination point never earned.
    #[must_use]
    pub fn observed(&self, spec: &FaultSpec, reference: &CanonicalFacts) -> Vec<Outcome> {
        let mut observed = Vec::new();
        if let Some(letter) = self.subject.letter() {
            observed.push(letter);
        }
        if spec.subject == FaultSubject::CanonicalTransaction
            && self.committed(reference)
            && self.retry_replayed_original_receipt()
        {
            observed.push(Outcome::IdempotentRetry);
        }
        observed
    }

    /// Renders the receipt block for this fault.
    #[must_use]
    pub fn receipt_lines(&self, spec: &FaultSpec, reference: &CanonicalFacts) -> Vec<String> {
        vec![
            format!(
                "{} owner={} stage={} subject={} expected={} observed={}",
                self.fault_id,
                spec.owner.as_str(),
                spec.stage.as_str(),
                spec.subject.as_str(),
                Outcome::render(spec.expected),
                Outcome::render(&self.observed(spec, reference))
            ),
            format!("  child {}", self.child.receipt_line()),
            format!(
                "  pre-restart vault {}",
                self.pre_restart_vault.receipt_line()
            ),
            format!(
                "  pre-restart canonical {}",
                self.pre_restart_canonical.receipt_line()
            ),
            format!("  reconcile {}", self.reconcile.receipt_line()),
            format!(
                "  post-restart canonical {}",
                self.post_restart_canonical.receipt_line()
            ),
            format!("  deep doctor {}", self.post_restart_doctor.receipt_line()),
            format!("  subject {}", self.subject.receipt_line()),
            format!("  retry 1 {}", self.first_retry.receipt_line()),
            format!("  retry 2 {}", self.second_retry.receipt_line()),
            format!(
                "  exports objects={} digests_equal={} files_equal={}",
                self.exported_object_count,
                self.export_semantic_digests.0 == self.export_semantic_digests.1,
                self.export_file_manifests_match
            ),
            format!(
                "  restored canonical {}",
                self.restored_canonical.receipt_line()
            ),
            format!(
                "  restore digest={} projections_current={} findings=[{}]",
                self.restored_semantic_digest,
                self.restored_doctor.projections_current,
                self.restored_doctor.findings.join(",")
            ),
        ]
    }
}

/// Marker prefix for one machine-readable fault row.
///
/// `tools/phase1-exit.mjs` reads these out of the harness's own output rather
/// than re-deriving anything, so the receipt it assembles and the assertions
/// the suite made cannot disagree.
pub const RESULT_ROW_PREFIX: &str = "PHASE1_EXIT_ROW ";
/// Marker prefix for one machine-readable named-test row.
///
/// A test prints its row as its last statement, so the row exists only when
/// every assertion before it held. A panicking test emits nothing and the
/// reader reports it as a failure rather than inferring one from log shape.
pub const RESULT_TEST_PREFIX: &str = "PHASE1_EXIT_TEST ";
/// Marker prefix for the single machine-readable summary row.
pub const RESULT_SUMMARY_PREFIX: &str = "PHASE1_EXIT_SUMMARY ";
/// Version of the normalized result schema those rows carry.
pub const RESULT_SCHEMA: &str = "learning-platform.phase1-exit-result.v1";

/// Escapes one string for embedding in the normalized JSON rows.
///
/// The harness has no JSON dependency and must not gain one for six fields, so
/// this covers exactly what the emitted values can contain: quotes, backslashes,
/// the two-character escapes, and any other C0 control as `\u00XX`.
#[must_use]
pub fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control < ' ' => {
                escaped.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => escaped.push(other),
        }
    }
    escaped
}

/// Renders one JSON object from already-ordered string and integer members.
#[must_use]
pub fn json_object(members: &[(&str, JsonValue<'_>)]) -> String {
    let body = members
        .iter()
        .map(|(name, value)| format!("\"{}\":{}", json_escape(name), value.render()))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{body}}}")
}

/// The small value vocabulary the normalized rows need.
#[derive(Debug, Clone, Copy)]
pub enum JsonValue<'a> {
    /// A JSON string.
    Text(&'a str),
    /// A JSON number.
    Number(u64),
    /// A JSON boolean.
    Bool(bool),
}

impl JsonValue<'_> {
    fn render(self) -> String {
        match self {
            Self::Text(value) => format!("\"{}\"", json_escape(value)),
            Self::Number(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }
}

/// Lowercase hexadecimal of a byte slice.
#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

/// Returns whether a published directory carries the named manifest file.
#[must_use]
pub fn is_published(directory: &Path, manifest_file: &str) -> bool {
    directory.join(manifest_file).is_file()
}

/// Lists every entry directly below a directory, or nothing when it is absent.
pub fn direct_entries(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    paths.sort();
    Ok(paths)
}
