//! One candidate route, its four section 16.4 roles, and the result the engine
//! hands back.
//!
//! ## Section 16.4's four roles
//!
//! `UI는 반드시 필요한 shared spine, 선택적 가지, 현재 무관한 주변, alternative
//! path를 구분한다`. [`PathRole`] is those four. They are computed here rather
//! than chosen by a surface, because which concepts every surviving candidate
//! needs is a property of the front and not of a rendering.
//!
//! ## The result cannot omit its disclosure
//!
//! [`CriticalPathResult`] holds a [`crate::disclosure::Disclosure`] **by
//! value**, not an `Option`, and its one constructor takes one. There is no
//! `Default` and no field is public. `five_disclosure_groups_are_always_present`
//! is therefore about a value that cannot be built without all five, and
//! `crates/critical-path/tests/compile_fail/` holds the compiled half.

use academic_domain::EntityId;
use serde::{Deserialize, Serialize};

use crate::{
    CriticalPathError,
    checkpoint::CheckpointDecision,
    constraint::{ConstraintFinding, ConstraintVerdict, RequiredInsertion},
    disclosure::Disclosure,
    hypergraph::SatisfyingSet,
    option::AcquisitionOption,
    pareto::ParetoFront,
    preference::{NamedStrategy, PreferenceSlider},
    vector::{BenefitVector, CostVector},
};

/// Section 16.4's four path roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PathRole {
    /// `반드시 필요한 shared spine`: on every surviving candidate.
    SharedSpine,
    /// `선택적 가지`: on some surviving candidate and not all.
    OptionalBranch,
    /// `현재 무관한 주변`: in the hypergraph and on no surviving candidate.
    IrrelevantPeriphery,
    /// `alternative path`: on a surviving candidate other than the ranked
    /// first.
    AlternativePath,
}

impl PathRole {
    /// The words section 16.4 uses, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::SharedSpine => "반드시 필요한 shared spine",
            Self::OptionalBranch => "선택적 가지",
            Self::IrrelevantPeriphery => "현재 무관한 주변",
            Self::AlternativePath => "alternative path",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SharedSpine => "SHARED_SPINE",
            Self::OptionalBranch => "OPTIONAL_BRANCH",
            Self::IrrelevantPeriphery => "IRRELEVANT_PERIPHERY",
            Self::AlternativePath => "ALTERNATIVE_PATH",
        }
    }
}

/// The four, in section 16.4's own order.
pub const PATH_ROLES: [PathRole; 4] = [
    PathRole::SharedSpine,
    PathRole::OptionalBranch,
    PathRole::IrrelevantPeriphery,
    PathRole::AlternativePath,
];

/// One step of a plan: a concept and the options for acquiring it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStep {
    concept: EntityId,
    options: Vec<AcquisitionOption>,
    required_before: Option<RequiredInsertion>,
}

impl PlanStep {
    /// Records one step.
    #[must_use]
    pub const fn of(
        concept: EntityId,
        options: Vec<AcquisitionOption>,
        required_before: Option<RequiredInsertion>,
    ) -> Self {
        Self {
            concept,
            options,
            required_before,
        }
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The ways of acquiring it. A course is one of them and never the
    /// acquisition itself.
    #[must_use]
    pub fn options(&self) -> &[AcquisitionOption] {
        &self.options
    }

    /// What a constraint requires before this step.
    #[must_use]
    pub const fn required_before(&self) -> Option<&RequiredInsertion> {
        self.required_before.as_ref()
    }
}

/// One route through the hypergraph, with its two vectors and its eight
/// constraint answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    satisfying_set: SatisfyingSet,
    steps: Vec<PlanStep>,
    cost: CostVector,
    benefit: BenefitVector,
    constraints: [ConstraintFinding; 8],
    checkpoint: CheckpointDecision,
}

impl Candidate {
    /// Assembles one candidate.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::CandidateStepLeavesTheSet`] when a step names a
    /// concept the satisfying set does not hold, and
    /// [`CriticalPathError::CandidateStepMissing`] when the set holds a concept
    /// no step covers. Both directions, because a plan whose steps and whose
    /// satisfying set disagree is a plan whose vectors are about a different
    /// route from the one it prints.
    pub fn of(
        satisfying_set: SatisfyingSet,
        steps: Vec<PlanStep>,
        cost: CostVector,
        benefit: BenefitVector,
        constraints: [ConstraintFinding; 8],
        checkpoint: CheckpointDecision,
    ) -> Result<Self, CriticalPathError> {
        for step in &steps {
            if !satisfying_set.holds(step.concept()) {
                return Err(CriticalPathError::CandidateStepLeavesTheSet);
            }
        }
        for concept in satisfying_set.concepts() {
            if !steps.iter().any(|step| step.concept() == *concept) {
                return Err(CriticalPathError::CandidateStepMissing);
            }
        }
        Ok(Self {
            satisfying_set,
            steps,
            cost,
            benefit,
            constraints,
            checkpoint,
        })
    }

    /// The concepts this route satisfies the goal with.
    #[must_use]
    pub const fn satisfying_set(&self) -> &SatisfyingSet {
        &self.satisfying_set
    }

    /// The steps, in the order they are to be walked.
    #[must_use]
    pub fn steps(&self) -> &[PlanStep] {
        &self.steps
    }

    /// Section 16.2's `Cost(P)`.
    #[must_use]
    pub const fn cost(&self) -> &CostVector {
        &self.cost
    }

    /// Section 16.2's `Benefit(P)`.
    #[must_use]
    pub const fn benefit(&self) -> &BenefitVector {
        &self.benefit
    }

    /// All eight of section 16.3's answers, in [`crate::constraint::CONSTRAINTS`]
    /// order.
    #[must_use]
    pub const fn constraints(&self) -> &[ConstraintFinding; 8] {
        &self.constraints
    }

    /// Whether every constraint admits this candidate.
    #[must_use]
    pub fn is_feasible(&self) -> bool {
        self.constraints
            .iter()
            .all(|finding| finding.verdict().admits())
    }

    /// The constraints that refused it, in [`crate::constraint::CONSTRAINTS`]
    /// order.
    #[must_use]
    pub fn refusals(&self) -> Vec<&ConstraintFinding> {
        self.constraints
            .iter()
            .filter(|finding| !finding.verdict().admits())
            .collect()
    }

    /// Whether the eighth constraint inserted a diagnostic checkpoint.
    #[must_use]
    pub const fn checkpoint(&self) -> CheckpointDecision {
        self.checkpoint
    }

    /// The verdict of one named constraint.
    #[must_use]
    pub fn verdict_of(&self, constraint: crate::constraint::Constraint) -> ConstraintVerdict {
        self.constraints
            .iter()
            .find(|finding| finding.constraint() == constraint)
            .map_or(ConstraintVerdict::Unknown, ConstraintFinding::verdict)
    }
}

/// One surviving route as the engine reports it: its rank, its name and its
/// role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedPath {
    candidate: Candidate,
    rank: usize,
    role: PathRole,
    strategy: Option<NamedStrategy>,
}

impl RankedPath {
    /// The route.
    #[must_use]
    pub const fn candidate(&self) -> &Candidate {
        &self.candidate
    }

    /// Its position under the preference this result was produced with.
    #[must_use]
    pub const fn rank(&self) -> usize {
        self.rank
    }

    /// Its section 16.4 role.
    #[must_use]
    pub const fn role(&self) -> PathRole {
        self.role
    }

    /// The section 16.2 name it is shown under, when one fits.
    #[must_use]
    pub const fn strategy(&self) -> Option<NamedStrategy> {
        self.strategy
    }
}

/// What section 16 hands back.
///
/// Holds its disclosure by value. See the module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriticalPathResult {
    goal: EntityId,
    front: ParetoFront,
    ranked: Vec<RankedPath>,
    roles: Vec<(EntityId, PathRole)>,
    slider: PreferenceSlider,
    disclosure: Disclosure,
}

impl CriticalPathResult {
    /// Assembles one result.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::RankedPathIsNotOnTheFront`] when a ranked path is
    /// not one of the front's survivors, which would let a dominated route be
    /// reported as a recommendation.
    pub fn of(
        goal: EntityId,
        front: ParetoFront,
        ranked: Vec<RankedPath>,
        roles: Vec<(EntityId, PathRole)>,
        slider: PreferenceSlider,
        disclosure: Disclosure,
    ) -> Result<Self, CriticalPathError> {
        for path in &ranked {
            if !front
                .candidates()
                .iter()
                .any(|candidate| candidate == path.candidate())
            {
                return Err(CriticalPathError::RankedPathIsNotOnTheFront);
            }
        }
        Ok(Self {
            goal,
            front,
            ranked,
            roles,
            slider,
            disclosure,
        })
    }

    /// Which goal this plan is for.
    #[must_use]
    pub const fn goal(&self) -> EntityId {
        self.goal
    }

    /// The surviving candidates and the dominated ones.
    #[must_use]
    pub const fn front(&self) -> &ParetoFront {
        &self.front
    }

    /// The survivors in preference order.
    #[must_use]
    pub fn ranked(&self) -> &[RankedPath] {
        &self.ranked
    }

    /// Every concept the hypergraph holds and its section 16.4 role, in
    /// identifier order.
    #[must_use]
    pub fn roles(&self) -> &[(EntityId, PathRole)] {
        &self.roles
    }

    /// The preference the order was produced under.
    #[must_use]
    pub const fn slider(&self) -> &PreferenceSlider {
        &self.slider
    }

    /// Section 16.5's five groups. Never absent.
    #[must_use]
    pub const fn disclosure(&self) -> &Disclosure {
        &self.disclosure
    }

    /// Builds one ranked path. Crate-internal so a caller cannot claim a rank.
    pub(crate) const fn ranked_path(
        candidate: Candidate,
        rank: usize,
        role: PathRole,
        strategy: Option<NamedStrategy>,
    ) -> RankedPath {
        RankedPath {
            candidate,
            rank,
            role,
            strategy,
        }
    }
}
