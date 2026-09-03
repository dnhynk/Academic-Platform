//! Section 13.4's review card: both sides shown, neither rewritten.
//!
//! > 사용자가 직접 확인한 state는 AI가 낮추거나 높이지 못한다. 이후 모순
//! > evidence가 생기면 "사용자 확인과 새 evidence가 충돌"하는 review card를
//! > 만들고 양쪽을 보여준다.
//!
//! ## The same record two tasks already built
//!
//! `P2-R3`'s `ImplementationDrift` is *a record beside the edges rather than a
//! replacement for either*; `P2-R4`'s `ClassificationConflict` is that record
//! for a standing user override and a fresh proposal. A
//! [`KnowledgeStateConflict`] is the third instance of the same shape for a
//! third pair — a user-confirmed [`KnowledgeStateAssertion`] and an
//! [`AiProposal`] — and it is built by cloning both, never by editing either.
//!
//! ## The token is `P2-M3`'s
//!
//! `academic_ledger::ConflictReason::NewEvidenceConflict` and its canonical
//! spelling `NEW_EVIDENCE_CONFLICT` already exist, and section 34.2's row for
//! this failure names the same string as the display: `NEW_EVIDENCE_CONFLICT
//! 이지 자동 변경 아님`. This crate emits that value rather than a second
//! vocabulary, which is why `academic-ledger` is a product edge.
//!
//! ## Both directions
//!
//! `낮추거나 높이지 못한다` names two directions and
//! [`crate::confirmation::AdjustmentDirection`] has both, so a raise and a
//! lower reach the same answer through the same field rather than through two
//! code paths that could drift apart.

use academic_domain::{EntityId, MasteryLevel};
use academic_ledger::ConflictReason;

use crate::{
    assertion::KnowledgeStateAssertion,
    confirmation::{AdjustmentDirection, AiProposal},
};

/// A user-confirmed state and a model proposal that disagrees with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeStateConflict {
    reason: ConflictReason,
    concept: EntityId,
    direction: AdjustmentDirection,
    standing: KnowledgeStateAssertion,
    proposed: AiProposal,
}

impl KnowledgeStateConflict {
    /// Opens a card. Crate-private: [`crate::history`] is the one producer.
    pub(crate) fn seal(
        concept: EntityId,
        direction: AdjustmentDirection,
        standing: KnowledgeStateAssertion,
        proposed: AiProposal,
    ) -> Self {
        Self {
            reason: ConflictReason::NewEvidenceConflict,
            concept,
            direction,
            standing,
            proposed,
        }
    }

    /// `P2-M3`'s reason, which is the only one this crate emits.
    #[must_use]
    pub const fn reason(&self) -> ConflictReason {
        self.reason
    }

    /// The machine token, as `P2-M3` spells it.
    #[must_use]
    pub const fn reason_token(&self) -> &'static str {
        self.reason.canonical_token()
    }

    /// Which concept the two sides disagree about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Which way the proposal would have moved the state.
    #[must_use]
    pub const fn direction(&self) -> AdjustmentDirection {
        self.direction
    }

    /// The user's assertion, unchanged.
    #[must_use]
    pub const fn standing(&self) -> &KnowledgeStateAssertion {
        &self.standing
    }

    /// The model's proposal, unchanged and undiscarded.
    #[must_use]
    pub const fn proposed(&self) -> &AiProposal {
        &self.proposed
    }

    /// The level the user confirmed.
    #[must_use]
    pub const fn standing_level(&self) -> MasteryLevel {
        self.standing.mastery_level()
    }

    /// The level the model proposed.
    #[must_use]
    pub const fn proposed_level(&self) -> MasteryLevel {
        self.proposed.proposed()
    }
}
