//! `P2-U7` — transcript ingestion into the encrypted vault.
//!
//! Section 29.3 of the end-state design: PDF, CSV and manual-entry import of an
//! official transcript, OCR/import rows kept distinct from user-confirmed rows,
//! checksum reconciliation across course code, term, credits and grade, and
//! selective removal of student number and name for screen sharing and export.
//!
//! # The four contracts this crate fixes
//!
//! 1. **Import row and confirmed row are two linked claims.** [`claims`]. An
//!    importer or a model run asserts the first; only the user asserts the
//!    second; `Claim::validate_for_actor` is what makes the split unbypassable.
//! 2. **Reconciliation failure halts at the exact row.** [`reconcile`]. The
//!    halt names one ordinal and the fields inside it that disagree, and yields
//!    no value a confirmation can be built from.
//! 3. **Transcript originals are `RESTRICTED`/`USER_MANAGED` and encrypted.**
//!    [`vault`], behind the non-default `encrypted-vault` feature. It reuses
//!    ADR-004's `AEAD_CHUNKED_V2` and defines no object format of its own.
//! 4. **Redaction is a projection, never a source edit.** [`redaction`]. No
//!    type in this crate exposes a mutating method on a normalized transcript,
//!    and an export is built from a projection and from nothing else.
//!
//! # What is synthetic, and what is not permitted
//!
//! Every corpus this crate builds is synthetic and produced by the
//! deterministic builder in [`source`]. ADR-002 is unaccepted, the default lane
//! reports `storage_encryption=NONE`, `production_data_allowed` is `false`, and
//! `P2-K6` built an admission verifier without opening admission. So no real
//! academic record may be imported, and — separately from that policy — the
//! gate in [`admission`] refuses every profile-touching import in this
//! repository today.
//!
//! # What section 38 leaves open, and this crate does not close
//!
//! `GATE-38-005` (the current official transcript) and `GATE-38-007` (current
//! and planned enrollments) stay open. Both are user-supplied. Nothing here
//! infers a transcript, invents a default identity, guesses a term, or fills a
//! missing row: an absent field is [`TranscriptError::MalformedField`] with
//! reason `absent`, and a reconciliation with an unmatched row halts rather
//! than assuming one side.

use std::path::{Path, PathBuf};

use academic_domain::{ClaimId, DomainError, TranscriptVersionId};
use thiserror::Error;

pub mod admission;
pub mod claims;
mod fault;
pub mod reconcile;
pub mod record;
pub mod redaction;
pub mod session;
pub mod source;
#[cfg(feature = "encrypted-vault")]
pub mod vault;

pub use fault::{
    FAULT_ACTION_VARIABLE, FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE, FAULT_SELECTORS,
};

/// Every failure this crate raises.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TranscriptError {
    /// A profile-touching import was attempted without a verified receipt.
    ///
    /// `code` is `academic_admission::AdmissionError::code`, carried through
    /// unchanged so the refusal reports the verifier's own reason rather than a
    /// second vocabulary that could drift from it.
    #[error("transcript import refused: admission verification failed ({code})")]
    AdmissionRefused {
        /// The admission verifier's stable error code.
        code: &'static str,
    },
    /// A source document did not match its declared format.
    #[error("{format} source is malformed: {reason}", format = format.as_str())]
    MalformedSource {
        /// The format the parse was declared under.
        format: source::TranscriptFormat,
        /// Why the parse refused.
        reason: &'static str,
    },
    /// A canonical field failed validation.
    #[error("transcript {field} is invalid: {reason}")]
    MalformedField {
        /// Which field.
        field: &'static str,
        /// Why it was refused.
        reason: &'static str,
    },
    /// A transcript declared more rows than the canonical limit permits.
    #[error("transcript has {actual} rows, more than the {maximum} limit")]
    TooManyRows {
        /// Rows offered.
        actual: usize,
        /// Rows permitted.
        maximum: usize,
    },
    /// Row ordinals were not contiguous from zero in document order.
    #[error("row at position {position} declares ordinal {ordinal}")]
    NonContiguousOrdinal {
        /// Position in the row list.
        position: u32,
        /// Ordinal the row declared.
        ordinal: u32,
    },
    /// A model-read import row carried no confidence.
    #[error("a model-read transcript row must carry a confidence value")]
    ModelReadNeedsConfidence,
    /// A deterministically parsed import row carried a confidence.
    #[error("a deterministically parsed transcript row must not carry a confidence value")]
    DeterministicReadCarriesConfidence,
    /// The caller supplied a different number of claim identities than rows.
    #[error("{rows} rows need {rows} claim identity pairs, got {ids}")]
    ClaimIdCountMismatch {
        /// Rows to build claims for.
        rows: usize,
        /// Identity pairs supplied.
        ids: usize,
    },
    /// The import and confirmed claim identities of one row were equal.
    #[error("the import and confirmed claims of one row must not share identity {0}")]
    ClaimIdsCollide(ClaimId),
    /// A domain value failed the canonical vocabulary's own validation.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// Another live session already holds this session's lease.
    #[error("import session {version_id} is already leased")]
    SessionLeaseHeld {
        /// The session that is leased.
        version_id: TranscriptVersionId,
    },
    /// A session was resumed that does not exist.
    #[error("import session {version_id} does not exist")]
    SessionAbsent {
        /// The session that was looked for.
        version_id: TranscriptVersionId,
    },
    /// A session was resumed after it published.
    #[error("import session {version_id} has already published its confirmed set")]
    SessionAlreadyPublished {
        /// The session that already published.
        version_id: TranscriptVersionId,
    },
    /// A session was published with nothing staged.
    #[error("import session {version_id} has nothing staged")]
    NothingStaged {
        /// The session that had nothing staged.
        version_id: TranscriptVersionId,
    },
    /// A transcript original was offered under the wrong storage policy.
    #[cfg(feature = "encrypted-vault")]
    #[error(
        "a transcript original must be sealed RESTRICTED/USER_MANAGED, got {confidentiality:?}/{retention_class:?}"
    )]
    OriginalPolicyMismatch {
        /// Confidentiality the request carried.
        confidentiality: academic_domain::Confidentiality,
        /// Retention class the request carried.
        retention_class: academic_domain::RetentionClass,
    },
    /// The encrypted vault refused a seal.
    #[cfg(feature = "encrypted-vault")]
    #[error(transparent)]
    Vault(#[from] academic_vault::VaultError),
    /// A filesystem operation failed.
    #[error("{operation} failed at {path}")]
    Io {
        /// What was being done.
        operation: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
}

impl TranscriptError {
    /// Returns a stable code for a refusal, for a report that must not print a value.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::AdmissionRefused { .. } => "ADMISSION_REFUSED",
            Self::MalformedSource { .. } => "MALFORMED_SOURCE",
            Self::MalformedField { .. } => "MALFORMED_FIELD",
            Self::TooManyRows { .. } => "TOO_MANY_ROWS",
            Self::NonContiguousOrdinal { .. } => "NON_CONTIGUOUS_ORDINAL",
            Self::ModelReadNeedsConfidence => "MODEL_READ_NEEDS_CONFIDENCE",
            Self::DeterministicReadCarriesConfidence => "DETERMINISTIC_READ_CARRIES_CONFIDENCE",
            Self::ClaimIdCountMismatch { .. } => "CLAIM_ID_COUNT_MISMATCH",
            Self::ClaimIdsCollide(_) => "CLAIM_IDS_COLLIDE",
            Self::Domain(_) => "DOMAIN",
            Self::SessionLeaseHeld { .. } => "SESSION_LEASE_HELD",
            Self::SessionAbsent { .. } => "SESSION_ABSENT",
            Self::SessionAlreadyPublished { .. } => "SESSION_ALREADY_PUBLISHED",
            Self::NothingStaged { .. } => "NOTHING_STAGED",
            #[cfg(feature = "encrypted-vault")]
            Self::OriginalPolicyMismatch { .. } => "ORIGINAL_POLICY_MISMATCH",
            #[cfg(feature = "encrypted-vault")]
            Self::Vault(_) => "VAULT",
            Self::Io { .. } => "IO",
        }
    }

    pub(crate) fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            operation: "transcript session file operation",
            path: path.to_path_buf(),
            source,
        }
    }
}
