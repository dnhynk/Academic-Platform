//! Which authority class a piece of correlation evidence may enter section
//! 30.3's table as.
//!
//! ## This crate adds no rank and no ordering
//!
//! Section 30.3's six rows are already implemented:
//! [`academic_ledger::ProductClaimType`] is that table, `CurrentImplementation`
//! is row four and `ProjectIntent` is row five, and
//! [`academic_ledger::AuthorityTable::rank`] is the comparison. Full decision
//! replay — scope filtering, acceptance order, terminal statuses, equal-rank
//! conflict cards — stays in `academic_ledger::resolve_product_snapshot`, and
//! nothing here reimplements any of it.
//!
//! What this module adds is the half section 30.3 states and no rank table can:
//! the **qualifiers** on the two rows' authority lists.
//!
//! | Section 30.3 row | active view 우선순위 | The qualifier this module holds |
//! |---|---|---|
//! | 현재 구현 | 같은 snapshot의 runtime/config/code direct evidence > user clarification > AI | `같은 snapshot` |
//! | project intent | 승인된 최신 spec/ADR > user clarification > AI | `승인된` and `최신` |
//!
//! A direct observation of another snapshot is not row four's authority, and it
//! does not become one by being direct: it is admitted at
//! [`AuthorityClass::Unknown`], which the table already ranks below a user
//! clarification. A draft, a deprecated, or a superseded document is not row
//! five's authority for the same reason.
//!
//! ## The lanes do not lend each other authority
//!
//! Row four's conflict column is `spec은 intent lane에 보존` and row five's is
//! `code와 drift 생성`. So a document is admitted at `Unknown` for the
//! implementation question and a direct observation is admitted at `Unknown`
//! for the intent question. Neither is dropped — it is listed, at rank zero,
//! and the disagreement becomes a [`crate::ImplementationDrift`] rather than
//! either lane's answer.

use academic_domain::AuthorityClass;
use academic_ledger::AuthorityTable;

use crate::{
    CorrelationError,
    artifact::{ApprovalStatus, DocumentId, IntentDocumentKind},
    relation::AuthorityLane,
};

/// One kind of answer either of section 30.3's two rows can receive.
///
/// The four arms are the four things those two rows name between them:
/// same-snapshot direct evidence, an approved and latest spec or ADR, a user
/// clarification, and AI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerSource {
    /// Row four's `runtime/config/code direct evidence`, and the snapshot it
    /// was taken against.
    DirectEvidence {
        /// Which snapshot. Row four's `같은 snapshot` is a comparison against
        /// the question's own snapshot, not a property of the evidence.
        snapshot_id: String,
    },
    /// Row five's `spec/ADR`, with what makes one `승인된` and `최신`.
    IntentDocument {
        /// Which document.
        document: DocumentId,
        /// Specification or architecture decision. Row five names both and
        /// ranks them the same.
        kind: IntentDocumentKind,
        /// `승인된`.
        status: ApprovalStatus,
        /// `최신`.
        revision: u64,
    },
    /// Both rows' `user clarification`.
    UserClarification,
    /// Both rows' `AI`.
    ModelInference,
}

/// One candidate answer, with the caller's own name for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    id: String,
    source: AnswerSource,
}

impl Candidate {
    /// Names one candidate answer.
    #[must_use]
    pub fn new(id: impl Into<String>, source: AnswerSource) -> Self {
        Self {
            id: id.into(),
            source,
        }
    }

    /// The caller's name for it.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What kind of answer it is.
    #[must_use]
    pub const fn source(&self) -> &AnswerSource {
        &self.source
    }
}

/// One candidate, the class it was admitted at, and the table's rank for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedCandidate {
    id: String,
    authority: AuthorityClass,
    rank: u16,
}

impl RankedCandidate {
    /// The caller's name for the candidate.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The class it was admitted at.
    #[must_use]
    pub const fn authority(&self) -> AuthorityClass {
        self.authority
    }

    /// `academic-ledger`'s rank for that class in this row's table.
    #[must_use]
    pub const fn rank(&self) -> u16 {
        self.rank
    }
}

/// One lane's active view over a set of candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneAnswer {
    lane: AuthorityLane,
    table: AuthorityTable,
    ranked: Vec<RankedCandidate>,
}

impl LaneAnswer {
    /// Which lane this answers for.
    #[must_use]
    pub const fn lane(&self) -> AuthorityLane {
        self.lane
    }

    /// The section 30.3 row's table, as `academic-ledger` holds it.
    #[must_use]
    pub const fn table(&self) -> AuthorityTable {
        self.table
    }

    /// Every candidate, strongest first. Nothing is dropped: a candidate this
    /// row has no authority for is here at rank zero.
    #[must_use]
    pub fn ranked(&self) -> &[RankedCandidate] {
        &self.ranked
    }

    /// The active view's answer, or `None` when the strongest rank is shared.
    ///
    /// A shared top rank is a conflict rather than a winner, which is the rule
    /// `docs/contracts/product-authority-resolution.md` already states for
    /// equal-rank claims; this does not resolve one.
    #[must_use]
    pub fn winner(&self) -> Option<&RankedCandidate> {
        let first = self.ranked.first()?;
        match self.ranked.get(1) {
            Some(second) if second.rank == first.rank => None,
            _ => Some(first),
        }
    }
}

/// Section 30.3's active view for one lane, over one snapshot.
///
/// # Errors
///
/// [`CorrelationError::LaneHasNoAuthorityRow`] for [`AuthorityLane::Description`]:
/// section 30.3 has no row for a document that describes current behaviour, so
/// there is no precedence to compute and inventing one is what
/// `seven_relation_types_are_distinct` is written against.
pub fn active_view(
    lane: AuthorityLane,
    snapshot_id: &str,
    candidates: &[Candidate],
) -> Result<LaneAnswer, CorrelationError> {
    let claim_type = lane
        .claim_type()
        .ok_or(CorrelationError::LaneHasNoAuthorityRow(lane))?;
    let table = claim_type.authority_table();

    // Row five's `최신`: among the candidates, the highest revision that is
    // also `승인된`. A document below it has been superseded, and a superseded
    // document is not the latest word even though it was approved once.
    let latest_approved = candidates
        .iter()
        .filter_map(|candidate| match candidate.source() {
            AnswerSource::IntentDocument {
                status: ApprovalStatus::Approved,
                revision,
                ..
            } => Some(*revision),
            AnswerSource::IntentDocument { .. }
            | AnswerSource::DirectEvidence { .. }
            | AnswerSource::UserClarification
            | AnswerSource::ModelInference => None,
        })
        .max();

    let mut ranked: Vec<RankedCandidate> = candidates
        .iter()
        .map(|candidate| {
            let authority = admit(lane, snapshot_id, latest_approved, candidate.source());
            RankedCandidate {
                id: candidate.id().to_owned(),
                authority,
                rank: table.rank(authority),
            }
        })
        .collect();
    // Descending by rank, then by the caller's name, so the order is total and
    // a tie stays adjacent for `winner` to see.
    ranked.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(LaneAnswer {
        lane,
        table,
        ranked,
    })
}

/// Which authority class one source is admitted at, for one lane.
///
/// Total over [`AuthorityLane`] × [`AnswerSource`] with no default arm.
fn admit(
    lane: AuthorityLane,
    snapshot_id: &str,
    latest_approved: Option<u64>,
    source: &AnswerSource,
) -> AuthorityClass {
    match (lane, source) {
        // Row four's `같은 snapshot`. Direct evidence about another snapshot is
        // evidence about another snapshot.
        (
            AuthorityLane::Implementation,
            AnswerSource::DirectEvidence {
                snapshot_id: observed,
            },
        ) => {
            if observed == snapshot_id {
                AuthorityClass::DirectObservation
            } else {
                AuthorityClass::Unknown
            }
        }
        // Row five's `승인된 최신`. Draft, deprecated, and superseded each fail
        // it, and each for its own reason.
        (
            AuthorityLane::Intent,
            AnswerSource::IntentDocument {
                status, revision, ..
            },
        ) => {
            if *status == ApprovalStatus::Approved && latest_approved == Some(*revision) {
                AuthorityClass::Curated
            } else {
                AuthorityClass::Unknown
            }
        }
        // The lanes do not lend each other authority: row four's conflict
        // column preserves a spec in the intent lane rather than answering with
        // it, and row five's makes code a drift rather than an intent.
        (AuthorityLane::Implementation, AnswerSource::IntentDocument { .. })
        | (AuthorityLane::Intent, AnswerSource::DirectEvidence { .. }) => AuthorityClass::Unknown,
        (
            AuthorityLane::Implementation | AuthorityLane::Intent,
            AnswerSource::UserClarification,
        ) => AuthorityClass::UserExplicit,
        (AuthorityLane::Implementation | AuthorityLane::Intent, AnswerSource::ModelInference) => {
            AuthorityClass::ModelInference
        }
        // `active_view` refuses this lane before reaching here.
        (AuthorityLane::Description, _) => AuthorityClass::Unknown,
    }
}
