//! The one promotion an AI may never make, and the token that carries the
//! user's own decision.
//!
//! Section 13.1's sixth row: `FLUENT — AI 단독 판정 금지, 반복된 강한 evidence와
//! 사용자 확인 필요`. Section 13.4's last paragraph: `사용자가 직접 확인한
//! state는 AI가 낮추거나 높이지 못한다`.
//!
//! ## Three types, and each one is an absence
//!
//! * [`UserConfirmation`] has private fields, is neither `Clone` nor
//!   serializable, and its one constructor runs ADR-003's own
//!   `Claim::validate_for_actor`. Every automatic actor variant fails that
//!   matrix when it attempts the `USER_EXPLICIT`/`USER_CONFIRMED` pairing, and
//!   each automatic actor's own valid pairing is rejected as a confirmation.
//!   This is `P2-N1`'s `VerifiedCuratorApproval` shape for a different action.
//! * [`TransferRepetition`] cannot be built from one context. `반복` is more
//!   than one occurrence and `서로 다른 맥락` is more than one identity, so the
//!   constructor deduplicates the contexts it is given and answers [`None`]
//!   below two distinct ones. Repeating one context is not repetition.
//! * [`FluentAuthorization`] takes both of the above **by value**, and it is the
//!   only argument that makes `MasteryLevel::Fluent` reachable. The automatic
//!   path returns [`crate::ladder::AutomaticLevel`], which has no `Fluent`
//!   variant at all, so an automatic promotion to `FLUENT` is not a comparison
//!   that could be got wrong — it is a value that does not exist.
//!
//! ## Why `AiProposal` cannot become a confirmation
//!
//! It holds a `ModelRunId` and has no route to [`UserConfirmation`]: no `From`,
//! no constructor taking one, and nothing on it returns one. A model run that
//! wants to raise a user-confirmed state has the same options a model run that
//! wants to lower one has, and section 13.4 gives both the same answer — a
//! review card showing both sides. [`crate::conflict`] holds that card.

use std::collections::BTreeSet;

use academic_domain::{
    Actor, Claim, ClaimObject, EntityId, EpistemicStatus, EvidenceId, EvidenceItem, MasteryLevel,
    ModelRunId, ScopeId, TimestampMillis,
};

use crate::{KnowledgeStateError, evidence::ConceptEvidence};

/// The predicate a knowledge-state confirmation claim carries.
pub const STATE_CONFIRMATION_PREDICATE: &str = "knowledge.state.confirmed";

/// The sole accepted object value for a confirmation claim.
pub const STATE_CONFIRMATION_OBJECT: &str = "CONFIRM";

/// Proof that ADR-003 accepted a user-authored, user-confirmed state decision.
///
/// Fields are private, the type is not `Clone` and not serializable, and every
/// action that needs the user's own decision takes this type rather than an
/// [`Actor`] or a [`Claim`].
#[derive(Debug)]
pub struct UserConfirmation {
    user_id: EntityId,
    concept: EntityId,
    scope_id: ScopeId,
    level: MasteryLevel,
    confirmed_at: TimestampMillis,
}

impl UserConfirmation {
    /// Verifies a confirmation of `level` for `concept` in `scope_id`.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::Domain`] when ADR-003's actor/authority/status
    /// matrix rejects the claim, and
    /// [`KnowledgeStateError::InvalidConfirmationAction`],
    /// [`KnowledgeStateError::ConfirmationSubjectMismatch`],
    /// [`KnowledgeStateError::ConfirmationScopeMismatch`],
    /// [`KnowledgeStateError::ConfirmationEvidenceMissing`] or
    /// [`KnowledgeStateError::ConfirmationLevelMismatch`] when the claim is not
    /// exactly this confirmation.
    pub fn verify(
        actor: &Actor,
        claim: &Claim,
        evidence: &EvidenceItem,
        concept: EntityId,
        level: MasteryLevel,
        confirmed_at: TimestampMillis,
    ) -> Result<Self, KnowledgeStateError> {
        claim.validate_for_actor(actor)?;
        if claim.epistemic_status != EpistemicStatus::UserConfirmed
            || claim.predicate_id.as_str() != STATE_CONFIRMATION_PREDICATE
        {
            return Err(KnowledgeStateError::InvalidConfirmationAction);
        }
        match &claim.object {
            ClaimObject::Mastery(confirmed) if *confirmed == level => {}
            ClaimObject::Mastery(_) => {
                return Err(KnowledgeStateError::ConfirmationLevelMismatch);
            }
            _ => return Err(KnowledgeStateError::InvalidConfirmationAction),
        }
        if claim.subject_entity_id != concept {
            return Err(KnowledgeStateError::ConfirmationSubjectMismatch);
        }
        if !claim.evidence_ids.contains(&evidence.id) {
            return Err(KnowledgeStateError::ConfirmationEvidenceMissing);
        }
        evidence.validate()?;
        let Actor::User { user_id } = actor else {
            // The matrix above already rejects this branch for a valid
            // confirmation claim; keeping the pattern match makes the resulting
            // token structurally user-only if that matrix ever grows.
            return Err(KnowledgeStateError::InvalidConfirmationAction);
        };
        Ok(Self {
            user_id: *user_id,
            concept,
            scope_id: claim.scope_id,
            level,
            confirmed_at,
        })
    }

    /// Which user.
    #[must_use]
    pub const fn user_id(&self) -> EntityId {
        self.user_id
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Which resolution scope.
    #[must_use]
    pub const fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    /// Which level the user confirmed.
    #[must_use]
    pub const fn level(&self) -> MasteryLevel {
        self.level
    }

    /// When.
    #[must_use]
    pub const fn confirmed_at(&self) -> TimestampMillis {
        self.confirmed_at
    }
}

/// One independent performance in one context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferContext {
    context: String,
    evidence: EvidenceId,
    independent: bool,
}

impl TransferContext {
    /// Records a performance the user carried out independently.
    ///
    /// `independent` is the row's `독립 수행` and is recorded rather than
    /// assumed: a context whose work was not independent is kept and is not
    /// counted.
    #[must_use]
    pub fn of(context: impl Into<String>, evidence: EvidenceId, independent: bool) -> Self {
        Self {
            context: context.into(),
            evidence,
            independent,
        }
    }

    /// The context identity.
    #[must_use]
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Which evidence item.
    #[must_use]
    pub const fn evidence(&self) -> EvidenceId {
        self.evidence
    }

    /// Whether the performance was independent.
    #[must_use]
    pub const fn is_independent(&self) -> bool {
        self.independent
    }
}

/// Section 13.2's sixth row: repeated independent performance across distinct
/// contexts.
///
/// The constructor deduplicates on context identity and counts only independent
/// performances. Two distinct contexts is the floor the design's own words fix:
/// `반복` is more than one occurrence and `서로 다른 맥락` is more than one
/// identity. Duplicate context identifiers do not count as repetition, which is
/// `REQ-13-017`'s *duplicate context IDs → repetition으로 세지 않음*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRepetition {
    contexts: Vec<TransferContext>,
}

impl TransferRepetition {
    /// The smallest number of distinct contexts `반복 ... 서로 다른 맥락` admits.
    pub const MINIMUM_DISTINCT_CONTEXTS: usize = 2;

    /// Answers only for two or more distinct, independent contexts.
    #[must_use]
    pub fn across(contexts: Vec<TransferContext>) -> Option<Self> {
        let distinct: BTreeSet<&str> = contexts
            .iter()
            .filter(|entry| entry.is_independent())
            .map(TransferContext::context)
            .collect();
        if distinct.len() < Self::MINIMUM_DISTINCT_CONTEXTS {
            return None;
        }
        Some(Self { contexts })
    }

    /// Every context offered, including the ones that were not counted.
    #[must_use]
    pub fn contexts(&self) -> &[TransferContext] {
        &self.contexts
    }

    /// How many distinct independent contexts were counted.
    #[must_use]
    pub fn distinct_independent_contexts(&self) -> usize {
        self.contexts
            .iter()
            .filter(|entry| entry.is_independent())
            .map(TransferContext::context)
            .collect::<BTreeSet<_>>()
            .len()
    }
}

/// The one value that makes `MasteryLevel::Fluent` reachable.
///
/// Both halves of section 13.1's sentence, taken by value: the repeated strong
/// evidence and the user's own confirmation. Private fields, no `Default`, no
/// second constructor, and no `Clone` — an authorization cannot be kept and
/// reused for another concept, because it names the one it was built for.
#[derive(Debug)]
pub struct FluentAuthorization {
    concept: EntityId,
    user_id: EntityId,
    scope_id: ScopeId,
    distinct_contexts: usize,
    confirmed_at: TimestampMillis,
}

impl FluentAuthorization {
    /// Authorizes `FLUENT` for one concept.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::ConfirmationLevelMismatch`] when the confirmation
    /// is of some other level, and
    /// [`KnowledgeStateError::ConfirmationSubjectMismatch`] when it is about
    /// another concept.
    pub fn granted(
        repetition: TransferRepetition,
        confirmation: UserConfirmation,
        concept: EntityId,
    ) -> Result<Self, KnowledgeStateError> {
        if confirmation.level() != MasteryLevel::Fluent {
            return Err(KnowledgeStateError::ConfirmationLevelMismatch);
        }
        if confirmation.concept() != concept {
            return Err(KnowledgeStateError::ConfirmationSubjectMismatch);
        }
        Ok(Self {
            concept,
            user_id: confirmation.user_id(),
            scope_id: confirmation.scope_id(),
            distinct_contexts: repetition.distinct_independent_contexts(),
            confirmed_at: confirmation.confirmed_at(),
        })
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Which user confirmed it.
    #[must_use]
    pub const fn user_id(&self) -> EntityId {
        self.user_id
    }

    /// Which scope.
    #[must_use]
    pub const fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    /// How many distinct independent contexts the repetition carried.
    #[must_use]
    pub const fn distinct_contexts(&self) -> usize {
        self.distinct_contexts
    }

    /// When the user confirmed.
    #[must_use]
    pub const fn confirmed_at(&self) -> TimestampMillis {
        self.confirmed_at
    }
}

/// Which way a proposal would move a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdjustmentDirection {
    /// The proposal would raise the level.
    Raise,
    /// The proposal would lower the level.
    Lower,
    /// The proposal names the level the state already holds.
    Unchanged,
}

impl AdjustmentDirection {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::Raise, Self::Lower, Self::Unchanged];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raise => "RAISE",
            Self::Lower => "LOWER",
            Self::Unchanged => "UNCHANGED",
        }
    }

    /// Which direction `proposed` moves from `standing`.
    ///
    /// The comparison is over [`crate::ladder::rung`], which is section 13.1's
    /// own `Level` column, rather than over a discriminant this crate does not
    /// declare.
    #[must_use]
    pub const fn between(standing: MasteryLevel, proposed: MasteryLevel) -> Self {
        let standing = crate::ladder::rung(standing);
        let proposed = crate::ladder::rung(proposed);
        if proposed > standing {
            Self::Raise
        } else if proposed < standing {
            Self::Lower
        } else {
            Self::Unchanged
        }
    }
}

/// What a model run proposes about one concept.
///
/// It carries a `ModelRunId` and there is no route from here to a
/// [`UserConfirmation`]: no conversion, no constructor taking one, and nothing
/// on this type returns one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProposal {
    run_id: ModelRunId,
    concept: EntityId,
    proposed: MasteryLevel,
    evidence: Vec<ConceptEvidence>,
}

impl AiProposal {
    /// Records a model run's proposal.
    #[must_use]
    pub const fn of(
        run_id: ModelRunId,
        concept: EntityId,
        proposed: MasteryLevel,
        evidence: Vec<ConceptEvidence>,
    ) -> Self {
        Self {
            run_id,
            concept,
            proposed,
            evidence,
        }
    }

    /// Which model run.
    #[must_use]
    pub const fn run_id(&self) -> ModelRunId {
        self.run_id
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The level proposed.
    #[must_use]
    pub const fn proposed(&self) -> MasteryLevel {
        self.proposed
    }

    /// The evidence offered with it, unchanged.
    #[must_use]
    pub fn evidence(&self) -> &[ConceptEvidence] {
        &self.evidence
    }
}
