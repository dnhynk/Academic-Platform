//! Field-level checksum reconciliation, and the halt that localizes a mismatch.
//!
//! Section 29.3 requires a checksum across course code, term, credits and
//! grade. The contract this module fixes is what a *failure* looks like:
//!
//! - it names one row, by the ordinal that row has in the official document;
//! - inside that row it names the exact fields that disagree;
//! - it reports how many rows reconciled before it, so the halt is a position
//!   rather than a verdict on the whole document;
//! - and it carries no identity value, because a mismatch report is a second
//!   surface that reaches a screen and the student number must not be on it.
//!
//! A halt yields no [`ReconciledTranscript`], and [`ReconciledTranscript`] is
//! the only thing [`crate::claims`] will build a confirmed row from. That is
//! what makes "nothing confirmed" a property of the type rather than of a
//! caller remembering to check a boolean.

use sha2::{Digest as _, Sha256};

use crate::record::{NormalizedTranscript, TranscriptField};

/// Domain separator for a per-field checksum.
pub const FIELD_CHECKSUM_LABEL: &[u8] = b"ACADEMIC-TRANSCRIPT-FIELD-V1";
/// Domain separator for the identity-header checksum.
pub const IDENTITY_CHECKSUM_LABEL: &[u8] = b"ACADEMIC-TRANSCRIPT-IDENTITY-V1";

/// The checksum block one transcript is reconciled against.
///
/// It holds digests and counts only. A reference is derived from a second,
/// independent read of the same official document — the CSV export beside the
/// PDF, or the user's manual entry beside an OCR pass — so reconciliation
/// compares two readings rather than a reading against itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptChecksums {
    identity_digest: [u8; 32],
    rows: Vec<[[u8; 32]; 4]>,
}

impl TranscriptChecksums {
    /// Derives the checksum block of a normalized transcript.
    #[must_use]
    pub fn of(transcript: &NormalizedTranscript) -> Self {
        let identity = transcript.identity();
        let mut hasher = Sha256::new();
        hasher.update(IDENTITY_CHECKSUM_LABEL);
        for value in [
            identity.student_number(),
            identity.student_name(),
            identity.institution(),
            identity.issued_on(),
        ] {
            hasher.update(u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes());
            hasher.update(value.as_bytes());
        }
        let identity_digest = hasher.finalize().into();

        let rows = transcript
            .rows()
            .iter()
            .map(|row| {
                TranscriptField::ALL
                    .map(|field| field_digest(row.ordinal(), field, &row.field(field)))
            })
            .collect();
        Self {
            identity_digest,
            rows,
        }
    }

    /// Returns the identity-header digest.
    #[must_use]
    pub const fn identity_digest(&self) -> &[u8; 32] {
        &self.identity_digest
    }

    /// Returns how many rows the reference covers.
    #[must_use]
    pub fn row_count(&self) -> u32 {
        u32::try_from(self.rows.len()).unwrap_or(u32::MAX)
    }

    /// Returns one field's reference digest, if the reference covers that row.
    #[must_use]
    pub fn field_digest(&self, ordinal: u32, field: TranscriptField) -> Option<&[u8; 32]> {
        let row = self.rows.get(usize::try_from(ordinal).ok()?)?;
        let index = TranscriptField::ALL
            .iter()
            .position(|entry| *entry == field)?;
        row.get(index)
    }
}

/// Digest of one field of one row.
#[must_use]
pub fn field_digest(ordinal: u32, field: TranscriptField, value: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FIELD_CHECKSUM_LABEL);
    hasher.update(ordinal.to_le_bytes());
    hasher.update(field.as_str().as_bytes());
    hasher.update(u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes());
    hasher.update(value.as_bytes());
    hasher.finalize().into()
}

/// Why reconciliation stopped at one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HaltCause {
    /// The named fields of this row disagree between the two readings.
    ///
    /// Non-empty, in [`TranscriptField::ALL`] order.
    FieldsDisagree(Vec<TranscriptField>),
    /// The candidate has a row here and the reference does not.
    RowAbsentFromReference,
    /// The reference has a row here and the candidate does not.
    RowAbsentFromCandidate,
}

impl HaltCause {
    /// Returns the stable wire name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::FieldsDisagree(_) => "FIELDS_DISAGREE",
            Self::RowAbsentFromReference => "ROW_ABSENT_FROM_REFERENCE",
            Self::RowAbsentFromCandidate => "ROW_ABSENT_FROM_CANDIDATE",
        }
    }
}

/// A reconciliation that stopped at exactly one row.
///
/// The `IN03` outcome, as a value. It carries no field value and no identity
/// value: an ordinal, a cause, and a count. The caller already holds the
/// candidate transcript and renders the disputed values from it; the reference
/// side is a checksum and has no value to render, which is what sends the user
/// back to the official document rather than to the other import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationHalt {
    ordinal: u32,
    cause: HaltCause,
    rows_reconciled_before_halt: u32,
}

impl ReconciliationHalt {
    /// The document-order position of the row that stopped reconciliation.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Why that row stopped it.
    #[must_use]
    pub const fn cause(&self) -> &HaltCause {
        &self.cause
    }

    /// How many rows reconciled before the halting row.
    ///
    /// Equal to [`Self::ordinal`] by construction; it is reported separately
    /// because "the import stopped here" and "this many rows are known good"
    /// are two different things to a reader, and a future non-sequential
    /// strategy would keep the first and change the second.
    #[must_use]
    pub const fn rows_reconciled_before_halt(&self) -> u32 {
        self.rows_reconciled_before_halt
    }

    /// The disagreeing fields, when the cause is a field disagreement.
    #[must_use]
    pub fn disagreeing_fields(&self) -> &[TranscriptField] {
        match &self.cause {
            HaltCause::FieldsDisagree(fields) => fields,
            HaltCause::RowAbsentFromReference | HaltCause::RowAbsentFromCandidate => &[],
        }
    }
}

/// A transcript whose every row reconciled against the reference.
///
/// The only route to a confirmed row. It cannot be constructed by a caller: it
/// is returned by [`reconcile`] and by nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciledTranscript {
    transcript: NormalizedTranscript,
    reference_identity_digest: [u8; 32],
}

impl ReconciledTranscript {
    /// Returns the reconciled transcript.
    #[must_use]
    pub const fn transcript(&self) -> &NormalizedTranscript {
        &self.transcript
    }

    /// Returns the identity digest of the reference reading.
    #[must_use]
    pub const fn reference_identity_digest(&self) -> &[u8; 32] {
        &self.reference_identity_digest
    }
}

/// The result of reconciling one reading against another's checksum block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationOutcome {
    /// Every row agreed.
    Reconciled(Box<ReconciledTranscript>),
    /// Reconciliation stopped at one row. Nothing is confirmable.
    Halted(ReconciliationHalt),
}

impl ReconciliationOutcome {
    /// Returns the reconciled transcript, or `None` for a halt.
    #[must_use]
    pub fn reconciled(&self) -> Option<&ReconciledTranscript> {
        match self {
            Self::Reconciled(value) => Some(value),
            Self::Halted(_) => None,
        }
    }

    /// Returns the halt, or `None` for a complete reconciliation.
    #[must_use]
    pub const fn halt(&self) -> Option<&ReconciliationHalt> {
        match self {
            Self::Halted(halt) => Some(halt),
            Self::Reconciled(_) => None,
        }
    }
}

/// Reconciles a candidate reading against a reference checksum block.
///
/// Rows are walked in document order and the walk stops at the first row that
/// disagrees. It does not continue to collect every mismatch in the document:
/// section 29.3's contract is that the import halts at the exact row, and a
/// list of every downstream row that also disagreed is what turns a localized
/// failure back into a whole-document verdict.
///
/// The identity header is deliberately **not** a halt condition. Two readings
/// of the same document can spell a name differently — an OCR pass and a
/// hand-typed entry routinely do — and refusing the whole import for that would
/// discard four correct academic fields for a field that no downstream
/// calculation uses. The identity digests are both returned so a caller can
/// show the difference; nothing here decides it.
#[must_use]
pub fn reconcile(
    candidate: &NormalizedTranscript,
    reference: &TranscriptChecksums,
) -> ReconciliationOutcome {
    let candidate_rows = u32::try_from(candidate.rows().len()).unwrap_or(u32::MAX);
    let shared = candidate_rows.min(reference.row_count());
    for row in candidate.rows().iter().take(shared as usize) {
        let disagreeing: Vec<TranscriptField> = TranscriptField::ALL
            .into_iter()
            .filter(|field| {
                reference.field_digest(row.ordinal(), *field)
                    != Some(&field_digest(row.ordinal(), *field, &row.field(*field)))
            })
            .collect();
        if !disagreeing.is_empty() {
            return ReconciliationOutcome::Halted(ReconciliationHalt {
                ordinal: row.ordinal(),
                cause: HaltCause::FieldsDisagree(disagreeing),
                rows_reconciled_before_halt: row.ordinal(),
            });
        }
    }
    if candidate_rows != reference.row_count() {
        let cause = if candidate_rows > reference.row_count() {
            HaltCause::RowAbsentFromReference
        } else {
            HaltCause::RowAbsentFromCandidate
        };
        return ReconciliationOutcome::Halted(ReconciliationHalt {
            ordinal: shared,
            cause,
            rows_reconciled_before_halt: shared,
        });
    }
    ReconciliationOutcome::Reconciled(Box::new(ReconciledTranscript {
        transcript: candidate.clone(),
        reference_identity_digest: *reference.identity_digest(),
    }))
}
