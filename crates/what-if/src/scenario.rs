//! Section 22.1's `PlanScenario`, and the pure engine that produces one.
//!
//! ```yaml
//! PlanScenario:
//!   id: plan_2027_spring_A
//!   basedOn: ...
//!   choices: [...]
//!   assumptions: [...]
//!   deterministicResults: ...
//!   projections: ...
//! ```
//!
//! # A plan is not an attempt, and this crate cannot make one
//!
//! `P2-U4` already separated the two: its `PlanScenarioChoice` has no route to
//! a `CourseAttempt`, and `AttemptStatus::Planned` has no producer. This crate
//! adds the outer half of the same absence. It has no edge of any kind to
//! `academic-store` or `academic-store-platform`, so the canonical writer is
//! not nameable here at all; nothing in it opens a file, a socket or a clock;
//! and every value it produces has private fields and one crate-private
//! producer, so a plan cannot be assembled to say something the engine did not
//! compute.
//!
//! `plan_scenario_never_writes_actual_state` measures all of that: it walks
//! the workspace manifests from this package and requires both writer crates to
//! be unreachable, compares this crate's whole `use` inventory and whole set of
//! crate-root paths against a reviewed list in both directions, and runs
//! `crates/what-if/tests/compile_fail/` for the expressions that must not
//! compile.
//!
//! # The seventh field section 22.1 does not name
//!
//! [`PlanScenario`] carries the six keys of the block above and one more,
//! [`PlanScenario::inputs_digest`]. Section 22.1 does not list it. It is here
//! because a plan that cannot say which frozen inputs produced it cannot be
//! shown to be stale, cannot be recomputed reproducibly, and cannot be
//! calibrated against the term that followed it — all three of which section
//! 22.5 requires. The reading is recorded in
//! `docs/contracts/what-if-simulator.md` rather than left for a later reader to
//! rediscover.

use std::collections::BTreeSet;

use academic_domain::{ContentDigest, EntityId};
use academic_scenario::{
    OpportunityKind, ProposalProvenance, Proposed, ScenarioAssumption, ScenarioChoice,
    ScenarioInputs, WorkloadHoursRange, project,
};

use crate::{
    assumption::{
        ASSUMPTION_KINDS, AssumptionKind, HypotheticalCompletion, PlanAssumptions,
        ProbabilisticCoverage,
    },
    basis::ScenarioBasis,
    deterministic::{
        Allocation, CreditLoad, DeterministicResults, DownstreamUnlock, GpaScenario,
        PrerequisiteStanding, RuleContribution, ScheduleConflicts,
    },
    error::WhatIfError,
    inputs::{PlanChoice, PlanInputs, plan_inputs_digest},
    lane::{SectionView, UiSection},
    projected::{
        InformalReadiness, InformalReadinessEntry, PathCoverage, PathCoverageEntry,
        ProjectedResults, ProjectedWorkload, RelevanceProjection,
    },
};

/// One key of section 22.1's `PlanScenario` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScenarioKey {
    /// `id`.
    Id,
    /// `basedOn`.
    BasedOn,
    /// `choices`.
    Choices,
    /// `assumptions`.
    Assumptions,
    /// `deterministicResults`.
    DeterministicResults,
    /// `projections`.
    Projections,
}

/// The keys, in section 22.1's own order.
pub const SCENARIO_KEYS: [ScenarioKey; 6] = [
    ScenarioKey::Id,
    ScenarioKey::BasedOn,
    ScenarioKey::Choices,
    ScenarioKey::Assumptions,
    ScenarioKey::DeterministicResults,
    ScenarioKey::Projections,
];

impl ScenarioKey {
    /// The key section 22.1 writes, verbatim.
    #[must_use]
    pub const fn spec_key(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::BasedOn => "basedOn",
            Self::Choices => "choices",
            Self::Assumptions => "assumptions",
            Self::DeterministicResults => "deterministicResults",
            Self::Projections => "projections",
        }
    }

    /// The field of [`PlanScenario`] that holds it.
    #[must_use]
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::BasedOn => "basis",
            Self::Choices => "choices",
            Self::Assumptions => "assumptions",
            Self::DeterministicResults => "deterministic",
            Self::Projections => "projections",
        }
    }
}

/// One simulated plan.
///
/// Private fields and one producer, [`simulate`]. There is no setter, no
/// `&mut` accessor and no interior mutability, which is what lets
/// [`crate::comparison::compare`] and [`crate::graduation::HypotheticalGraduation`]
/// borrow a plan and be unable to change it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanScenario {
    id: EntityId,
    basis: ScenarioBasis,
    choices: Vec<PlanChoice>,
    assumptions: PlanAssumptions,
    deterministic: DeterministicResults,
    projections: ProjectedResults,
    inputs_digest: ContentDigest,
}

impl PlanScenario {
    /// Section 22.1's `id`.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        self.id
    }

    /// Section 22.1's `basedOn`.
    #[must_use]
    pub const fn basis(&self) -> &ScenarioBasis {
        &self.basis
    }

    /// Section 22.1's `choices`.
    #[must_use]
    pub fn choices(&self) -> &[PlanChoice] {
        &self.choices
    }

    /// Section 22.1's `assumptions`.
    #[must_use]
    pub const fn assumptions(&self) -> &PlanAssumptions {
        &self.assumptions
    }

    /// Section 22.1's `deterministicResults`.
    #[must_use]
    pub const fn deterministic(&self) -> &DeterministicResults {
        &self.deterministic
    }

    /// Section 22.1's `projections`.
    #[must_use]
    pub const fn projections(&self) -> &ProjectedResults {
        &self.projections
    }

    /// The frozen inputs this plan was computed from.
    #[must_use]
    pub const fn inputs_digest(&self) -> ContentDigest {
        self.inputs_digest
    }

    /// The two sections section 22.1 separates, in its own order.
    ///
    /// The two views borrow two different types, so a renderer walking this
    /// list cannot put a projection under the deterministic heading. See
    /// [`crate::lane`].
    #[must_use]
    pub fn sections(&self) -> [SectionView<'_>; 2] {
        [
            SectionView::DeterministicResults(&self.deterministic),
            SectionView::Projections(&self.projections),
        ]
    }

    /// The view for one section.
    #[must_use]
    pub fn section(&self, section: UiSection) -> SectionView<'_> {
        match section {
            UiSection::DeterministicResults => {
                SectionView::DeterministicResults(&self.deterministic)
            }
            UiSection::Projections => SectionView::Projections(&self.projections),
        }
    }
}

/// Simulates one plan.
///
/// Deterministic and total: identical inputs always yield an identical plan,
/// and every rejection is a typed error rather than a panic.
///
/// # Errors
///
/// [`WhatIfError::EmptyPlan`] for a plan with no choice,
/// [`WhatIfError::DuplicateChoice`] for one offering named twice,
/// [`WhatIfError::RelevanceOutsidePlan`] for a relevance reading about an
/// offering the plan does not choose, every error
/// [`GpaScenario::under`](crate::deterministic::GpaScenario) raises, and every
/// error `P2-C7`'s own engine raises.
pub fn simulate(inputs: &PlanInputs) -> Result<PlanScenario, WhatIfError> {
    if inputs.choices.is_empty() {
        return Err(WhatIfError::EmptyPlan);
    }
    let mut seen = BTreeSet::new();
    for choice in &inputs.choices {
        if !seen.insert(choice.offering_id()) {
            return Err(WhatIfError::DuplicateChoice(choice.offering_id()));
        }
    }
    for signal in &inputs.relevance {
        if !seen.contains(&signal.offering_id()) {
            return Err(WhatIfError::RelevanceOutsidePlan(signal.offering_id()));
        }
    }

    let inputs_digest = plan_inputs_digest(inputs);
    let projection = project(&scenario_inputs(inputs))?;

    let planned: BTreeSet<_> = inputs.choices.iter().map(PlanChoice::course).collect();
    let mut prerequisites = Vec::with_capacity(inputs.choices.len());
    for choice in &inputs.choices {
        prerequisites.push(PrerequisiteStanding::of(choice, &inputs.completed_courses));
    }
    let mut downstream = Vec::with_capacity(inputs.downstream_courses.len());
    for course in &inputs.downstream_courses {
        downstream.push(DownstreamUnlock::of(
            course,
            &inputs.completed_courses,
            &planned,
            inputs.assumptions.completion(),
        ));
    }
    let gpa = match &inputs.grade_assumptions {
        None => None,
        Some(stated) => Some(GpaScenario::under(
            &inputs.choices,
            stated,
            &inputs.grading_scheme,
        )?),
    };
    let deterministic = DeterministicResults::of(
        CreditLoad::of(&inputs.choices),
        ScheduleConflicts::of(&inputs.choices),
        prerequisites,
        RuleContribution::under(&inputs.choices, inputs.assumptions.completion()),
        Allocation::of(&inputs.choices),
        downstream,
        gpa,
    );

    let mut band = WorkloadHoursRange::new(0, 0)?;
    for choice in &inputs.choices {
        band = band.saturating_add(choice.assumed_weekly_hours());
    }
    let proposed = Proposed::new(
        band,
        ProposalProvenance::new(
            inputs.model_run_id,
            inputs_digest,
            crate::WHAT_IF_ENGINE_VERSION,
            inputs.basis.knowledge_state_as_of(),
        ),
    );
    let workload = ProjectedWorkload::of(proposed, band, inputs.workload_bias.clone());

    let mut coverage = Vec::new();
    for opportunity in &projection.opportunities {
        if let Some(role) = inputs.path_targets.role_of(opportunity.concept_entity_id) {
            coverage.push(PathCoverageEntry::of(
                opportunity.concept_entity_id,
                role,
                opportunity.offering_id,
                opportunity.kind,
                opportunity.likelihood,
            ));
        }
    }

    let touched: BTreeSet<EntityId> = projection
        .opportunities
        .iter()
        .filter(|opportunity| {
            matches!(
                opportunity.kind,
                OpportunityKind::Exposure | OpportunityKind::Practice
            )
        })
        .map(|opportunity| opportunity.concept_entity_id)
        .collect();
    let readiness = InformalReadiness::of(
        inputs
            .informal_recommendations
            .iter()
            .map(|recommendation| {
                InformalReadinessEntry::of(
                    recommendation.downstream_concept(),
                    recommendation.recommended_concepts(),
                    &touched,
                )
            })
            .collect(),
    );

    let projections = ProjectedResults::of(
        projection,
        RelevanceProjection::of(&inputs.relevance),
        workload,
        PathCoverage::of(coverage),
        readiness,
    );

    Ok(PlanScenario {
        id: inputs.scenario_id,
        basis: inputs.basis,
        choices: inputs.choices.clone(),
        assumptions: inputs.assumptions,
        deterministic,
        projections,
        inputs_digest,
    })
}

/// Builds `P2-C7`'s own input from this plan's.
///
/// The first three bullets of section 22.3 are that crate's output and nothing
/// here recomputes them: exposure, practice and assessment opportunity arrive
/// from [`academic_scenario::project`], and the assumption names carried across
/// are section 22.1's own keys.
fn scenario_inputs(inputs: &PlanInputs) -> ScenarioInputs {
    ScenarioInputs {
        scenario_id: inputs.scenario_id,
        model_run_id: inputs.model_run_id,
        knowledge_state_as_of: inputs.basis.knowledge_state_as_of(),
        requirement_set_digest: inputs.basis.requirement_set_hash(),
        offering_catalog_digest: inputs.basis.offering_catalog_snapshot(),
        choices: inputs
            .choices
            .iter()
            .map(|choice| ScenarioChoice {
                offering_id: choice.offering_id(),
                credit_units: u16::from(choice.credits().value()),
                assumed_weekly_hours: choice.assumed_weekly_hours(),
                syllabus_concepts: choice.syllabus_concepts().to_vec(),
            })
            .collect(),
        assumptions: ASSUMPTION_KINDS
            .into_iter()
            .map(|kind| ScenarioAssumption {
                name: kind.spec_key().to_owned(),
                value: assumption_value(inputs, kind),
            })
            .collect(),
    }
}

fn assumption_value(inputs: &PlanInputs, kind: AssumptionKind) -> String {
    match kind {
        AssumptionKind::WorkloadHoursRange => format!(
            "{}..={} from {}",
            inputs.assumptions.workload().range().low_hours(),
            inputs.assumptions.workload().range().high_hours(),
            inputs.assumptions.workload().source()
        ),
        AssumptionKind::CompletionStatus => HypotheticalCompletion::SPEC_VALUE.to_owned(),
        AssumptionKind::ExpectedCoverage => ProbabilisticCoverage::SPEC_VALUE.to_owned(),
    }
}
