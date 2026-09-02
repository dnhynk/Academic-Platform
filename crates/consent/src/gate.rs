//! The two section 38 cells this task leaves open, stated where they bite.
//!
//! Neither has a default and neither can be given one here. `GATE-38-009` is a
//! user input that section 38.1 asks for every term; `GATE-38-019` is an
//! official fact section 38.2 asks the user to confirm for the offering. What
//! this crate supplies is the shape of the cell and the refusal that stands
//! while it is empty, which is what
//! [`unfilled_cells`](ConsentLedger::unfilled_cells) reports.
//!
//! There is deliberately no constant holding a "sensible" media set, no
//! `Default` on [`AuthorityGrant`](crate::AuthorityGrant), and no fallback that
//! reads one offering's answer for another. `academic-retention`'s
//! `OriginalVoiceAuthority` leaves `GATE-38-026` open the same way and for the
//! same reason.

use academic_domain::OfferingId;

use crate::{ConsentLedger, permission::TermKey, status::CaptureStatus};

/// A section 38 cell this task leaves for the user to fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum OpenGate {
    /// `GATE-38-009`: the offering's recording permission, confirmed per term.
    RecordingPermissionPerOffering,
    /// `GATE-38-019`: the offering's recording, filming, and local-versus-cloud
    /// transcription conditions.
    CaptureAndTranscriptionConditions,
}

impl OpenGate {
    /// The section 38 identifier.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::RecordingPermissionPerOffering => "GATE-38-009",
            Self::CaptureAndTranscriptionConditions => "GATE-38-019",
        }
    }

    /// What the cell leaves open, and what stands while it is empty.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::RecordingPermissionPerOffering => {
                "whether this offering permits capture, and on whose written \
                 authority, is a blocking user input confirmed every term \
                 (GATE-38-009); with no record the status is UNKNOWN and no \
                 capability is mintable"
            }
            Self::CaptureAndTranscriptionConditions => {
                "which media, and which local or external processing, this \
                 offering's conditions cover is an official fact the user \
                 confirms (GATE-38-019); an unconfirmed offering has empty \
                 allowed-media and allowed-processing sets, which match no \
                 request"
            }
        }
    }
}

/// One offering-and-term cell that is not filled, and which gate it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnfilledCell {
    offering_id: OfferingId,
    term: TermKey,
    gate: OpenGate,
}

impl UnfilledCell {
    /// Which offering.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// Which term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// Which gate.
    #[must_use]
    pub const fn gate(&self) -> OpenGate {
        self.gate
    }
}

impl ConsentLedger {
    /// Which of the two cells are empty for one offering and term.
    ///
    /// `GATE-38-009` is empty when no record answers the scope. `GATE-38-019`
    /// is empty when a record answers it and the grant lists no medium or no
    /// processing -- an authority who granted without stating what the grant
    /// covers has left the conditions unconfirmed, and an empty set is what
    /// that looks like from here rather than an error to raise at the user.
    ///
    /// A `PROHIBITED` scope reports neither: an authority answered, and there
    /// is no cell left for the user to fill.
    #[must_use]
    pub fn unfilled_cells(
        &self,
        offering_id: OfferingId,
        term: TermKey,
        at: u64,
    ) -> Vec<UnfilledCell> {
        let mut open = Vec::new();
        let status = self.status(offering_id, term, at);
        if matches!(status, CaptureStatus::Unknown | CaptureStatus::Expired) {
            open.push(UnfilledCell {
                offering_id,
                term,
                gate: OpenGate::RecordingPermissionPerOffering,
            });
        }
        let conditions_missing = self
            .records()
            .iter()
            .filter(|record| {
                record.scope().offering_id() == offering_id && record.scope().term() == term
            })
            .max_by_key(|record| record.permission_seq())
            .and_then(|record| record.grant())
            .is_some_and(|grant| {
                grant.allowed_media().is_empty() || grant.allowed_processing().is_empty()
            });
        if conditions_missing {
            open.push(UnfilledCell {
                offering_id,
                term,
                gate: OpenGate::CaptureAndTranscriptionConditions,
            });
        }
        open
    }
}
