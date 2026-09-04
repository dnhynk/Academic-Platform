//! Section 22.5's fourth bullet: *폐강·교수 변경·syllabus 변경 시 scenario를
//! 자동 수정하지 않고 `STALE_INPUT`으로 표시하고 재계산 동의를 받는다.*
//!
//! Three verbs in one sentence, and each is a different guarantee.
//!
//! # 자동 수정하지 않고 — the plan is frozen, not corrected
//!
//! [`FrozenPlan::mark`] takes the plan **by value** and hands it back through
//! [`FrozenPlan::plan`] unchanged. There is no method on this type that edits a
//! plan, and [`crate::scenario::PlanScenario`] has no setter, no `&mut`
//! accessor and no interior mutability, so a stale marking physically cannot
//! rewrite the answer the user is looking at.
//! `stale_input_freezes_and_requires_consent` observes the plan byte-identical
//! across the marking.
//!
//! # `STALE_INPUT`으로 표시하고 — and never with an empty reason
//!
//! [`FrozenPlan::mark`] takes its first [`StaleInput`] as a **parameter** and
//! refuses an empty rest, so a plan frozen for no stated reason is not a value
//! that can be written. That is `P2-U3`'s shape for an indeterminate verdict:
//! *the audit is indeterminate and we cannot say why* is not a value.
//!
//! # 재계산 동의를 받는다 — and the consent is a human's
//!
//! [`FrozenPlan::recompute`] takes a [`RecomputeConsent`], whose one
//! constructor takes `P2-M2`'s `UserDecision`. That type's one producer,
//! `UserDecision::by`, refuses every `Actor` that is not a user — a
//! deterministic engine, a model run and an importer are each refused by name.
//! So a background job that recomputed a stale plan on its own is not a call
//! that fails; it is a call with no argument to make.
//!
//! The consent also names the plan and the exact stale inputs it covers.
//! [`FrozenPlan::recompute`] refuses a consent that names another plan and one
//! that leaves any stale input uncovered, because a blanket consent is the
//! thing section 22.5 is refusing when it says 동의를 받는다 rather than 알린다.

use std::collections::BTreeSet;

use academic_domain::{EntityId, OfferingId, TimestampMillis};
use academic_offering::CancellationNotice;
use academic_proposal::UserDecision;

use crate::{
    error::WhatIfError,
    inputs::PlanInputs,
    scenario::{PlanScenario, simulate},
};

/// One of the three changes section 22.5 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StaleCause {
    /// `폐강`.
    Cancellation,
    /// `교수 변경`.
    InstructorChange,
    /// `syllabus 변경`.
    SyllabusChange,
}

/// The three, in section 22.5's own order.
pub const STALE_CAUSES: [StaleCause; 3] = [
    StaleCause::Cancellation,
    StaleCause::InstructorChange,
    StaleCause::SyllabusChange,
];

impl StaleCause {
    /// The phrase section 22.5 writes, verbatim.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::Cancellation => "폐강",
            Self::InstructorChange => "교수 변경",
            Self::SyllabusChange => "syllabus 변경",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cancellation => "CANCELLATION",
            Self::InstructorChange => "INSTRUCTOR_CHANGE",
            Self::SyllabusChange => "SYLLABUS_CHANGE",
        }
    }
}

/// The marker section 22.5 names.
///
/// Spelled as the specification spells it so a reader grepping for
/// `STALE_INPUT` finds the type that implements it.
pub const STALE_INPUT: &str = "STALE_INPUT";

/// One frozen input that stopped being true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaleInput {
    offering_id: OfferingId,
    cause: StaleCause,
    observed_at: TimestampMillis,
}

impl StaleInput {
    /// Records one change against one of the plan's choices.
    #[must_use]
    pub const fn of(
        offering_id: OfferingId,
        cause: StaleCause,
        observed_at: TimestampMillis,
    ) -> Self {
        Self {
            offering_id,
            cause,
            observed_at,
        }
    }

    /// Records the cancellation half from `P2-U5`'s own official notice.
    ///
    /// The notice's one constructor already refused every source level that
    /// does not publish offering changes, so a cancellation reaching this
    /// function came from the registration system or the department page. This
    /// crate does not re-decide that; it reads the value.
    #[must_use]
    pub fn from_cancellation(offering_id: OfferingId, notice: &CancellationNotice) -> Self {
        Self {
            offering_id,
            cause: StaleCause::Cancellation,
            observed_at: notice.issued_at(),
        }
    }

    /// Which choice went stale.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// Why.
    #[must_use]
    pub const fn cause(&self) -> StaleCause {
        self.cause
    }

    /// When the change was observed.
    #[must_use]
    pub const fn observed_at(&self) -> TimestampMillis {
        self.observed_at
    }
}

/// A user's consent to recompute one plan over the changes that made it stale.
///
/// Private fields, one constructor, no `Default`. See the module note: the
/// [`UserDecision`] is what makes the consent a human's, and the plan identity
/// and the covered inputs are what make it specific.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecomputeConsent {
    plan_id: EntityId,
    decision: UserDecision,
    covers: Vec<StaleInput>,
}

impl RecomputeConsent {
    /// Binds one user decision to one plan and to the changes it covers.
    #[must_use]
    pub fn of(plan_id: EntityId, decision: UserDecision, covers: Vec<StaleInput>) -> Self {
        let mut covers = covers;
        covers.sort_unstable();
        covers.dedup();
        Self {
            plan_id,
            decision,
            covers,
        }
    }

    /// The plan this consent is about.
    #[must_use]
    pub const fn plan_id(&self) -> EntityId {
        self.plan_id
    }

    /// The user decision behind it.
    #[must_use]
    pub const fn decision(&self) -> &UserDecision {
        &self.decision
    }

    /// The stale inputs it covers.
    #[must_use]
    pub fn covers(&self) -> &[StaleInput] {
        &self.covers
    }
}

/// A plan whose inputs changed, held exactly as it was.
///
/// The plan inside is the plan that was handed in. Nothing here corrects it,
/// re-runs it, or hides it: section 22.5 asks for a marking and a consent, and
/// a silently corrected plan would be the failure that marking exists to
/// prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenPlan {
    plan: PlanScenario,
    stale: Vec<StaleInput>,
}

impl FrozenPlan {
    /// Freezes one plan against at least one stale input.
    ///
    /// The first cause is a parameter and the rest are the remainder, so an
    /// empty reason list is not a call that can be written.
    #[must_use]
    pub fn mark(plan: PlanScenario, first: StaleInput, rest: Vec<StaleInput>) -> Self {
        let mut stale = vec![first];
        stale.extend(rest);
        stale.sort_unstable();
        stale.dedup();
        Self { plan, stale }
    }

    /// The plan, exactly as it was before the marking.
    #[must_use]
    pub const fn plan(&self) -> &PlanScenario {
        &self.plan
    }

    /// Every change that froze it.
    #[must_use]
    pub fn stale(&self) -> &[StaleInput] {
        &self.stale
    }

    /// The marker a reader is shown.
    #[must_use]
    pub const fn marker(&self) -> &'static str {
        STALE_INPUT
    }

    /// Recomputes the plan over fresh inputs, under a user's consent.
    ///
    /// The frozen plan is consumed: after a consented recomputation the stale
    /// plan is not still available beside the new one under the same name.
    ///
    /// # Errors
    ///
    /// [`WhatIfError::ConsentNamesAnotherPlan`] when the consent is about a
    /// different plan, [`WhatIfError::ConsentIsIncomplete`] when it does not
    /// cover every stale input, and every error [`simulate`] raises.
    pub fn recompute(
        self,
        consent: &RecomputeConsent,
        inputs: &PlanInputs,
    ) -> Result<PlanScenario, WhatIfError> {
        if consent.plan_id() != self.plan.id() {
            return Err(WhatIfError::ConsentNamesAnotherPlan);
        }
        let covered: BTreeSet<StaleInput> = consent.covers().iter().copied().collect();
        if !self.stale.iter().all(|input| covered.contains(input)) {
            return Err(WhatIfError::ConsentIsIncomplete);
        }
        simulate(inputs)
    }
}
