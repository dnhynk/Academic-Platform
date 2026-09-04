//! `P2-N8`'s three absence claims, and the frozen-input coverage behind them.
//!
//! An absence is not provable by grepping for the names somebody thought to
//! forbid. Every claim here is a **whole-set comparison**: the parse walks the
//! whole package, in any module, and the comparison fails on a missing entry
//! and on an extra one alike. A field added anywhere, spelling nothing anybody
//! wrote down, fails as a position nobody reviewed.
//!
//! Three of the four vocabularies these tests refuse are **derived from the
//! crate that owns them** rather than typed here: the mastery ladder is read
//! out of `P2-N2`'s own `ladder.rs`, `P2-U3`'s verdict vocabulary out of its
//! own `verdict.rs`, and the reachability of the canonical writer out of the
//! workspace manifests. A name added to any of them extends the guard here
//! without anybody editing this file, and each has a control that requires the
//! same reader to find those names where they do live.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
};

use academic_what_if::{
    PLAN_CHOICE_FIELDS, PLAN_INPUT_FIELDS, PlanChoiceField, PlanInputField, plan_inputs_digest,
};

use support::TestResult;

/// What one declared field position is for.
///
/// A closed set. A position that served none of these would have no entry to be
/// given, which is the point: the purpose column is not decoration, it is the
/// claim that every byte this crate stores belongs to one of section 22's
/// parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Purpose {
    /// An identifier, a digest or an instant that names something.
    Identity,
    /// Section 22.1's `assumptions`.
    Assumption,
    /// Something the caller freezes and the engine reads.
    FrozenInput,
    /// Section 22.2.
    DeterministicLane,
    /// Section 22.3.
    ProjectedLane,
    /// Section 22.4's ordering, which holds no plan value.
    ComparisonOrdering,
    /// Section 22.5's marking and its consent.
    StaleMarking,
    /// Section 22.5's calibration, whose subject is the model.
    ModelCalibration,
}

/// Every field position this crate declares, and what each is for.
///
/// This is the exhaustive net. It is deliberately not a list of names to
/// refuse: it is the list of positions that exist. `P2-X2` established the
/// shape for section 25.2's hero metric, after its own injections showed that a
/// forbidden-name list passes for every spelling nobody predicted.
const REVIEWED_FIELD_POSITIONS: &[(&str, Purpose)] = &[
    (
        "Allocation.lines: Vec<AllocationLine>",
        Purpose::DeterministicLane,
    ),
    (
        "AllocationLine.category: CurriculumCategory",
        Purpose::DeterministicLane,
    ),
    (
        "AllocationLine.course: CourseId",
        Purpose::DeterministicLane,
    ),
    (
        "AllocationLine.credits: Credits",
        Purpose::DeterministicLane,
    ),
    (
        "AllocationLine.offering_id: OfferingId",
        Purpose::DeterministicLane,
    ),
    (
        "AssumedWorkload.range: WorkloadHoursRange",
        Purpose::Assumption,
    ),
    ("AssumedWorkload.source: ModelRunId", Purpose::Assumption),
    (
        "CalibrationEntry.concept_entity_id: EntityId",
        Purpose::ModelCalibration,
    ),
    (
        "CalibrationEntry.direction: ProjectionCalibration",
        Purpose::ModelCalibration,
    ),
    (
        "CalibrationEntry.kind: OpportunityKind",
        Purpose::ModelCalibration,
    ),
    ("CalibrationEntry.observed: bool", Purpose::ModelCalibration),
    (
        "CalibrationEntry.offering_id: OfferingId",
        Purpose::ModelCalibration,
    ),
    (
        "CalibrationEntry.projected: Option<LikelihoodBand>",
        Purpose::ModelCalibration,
    ),
    (
        "CatalogueRow.category: CurriculumCategory",
        Purpose::FrozenInput,
    ),
    ("CatalogueRow.course: CourseId", Purpose::FrozenInput),
    ("CatalogueRow.credits: Credits", Purpose::FrozenInput),
    (
        "CategoryContribution.category: CurriculumCategory",
        Purpose::DeterministicLane,
    ),
    (
        "CategoryContribution.credits: u16",
        Purpose::DeterministicLane,
    ),
    (
        "ComparisonView.order: Vec<usize>",
        Purpose::ComparisonOrdering,
    ),
    (
        "ComparisonView.plans: Vec<&'a PlanScenario>",
        Purpose::ComparisonOrdering,
    ),
    (
        "ComparisonView.priority: DimensionPriority",
        Purpose::ComparisonOrdering,
    ),
    (
        "CreditLoad.lines: Vec<(OfferingId, Credits)>",
        Purpose::DeterministicLane,
    ),
    ("CreditLoad.requested: u16", Purpose::DeterministicLane),
    (
        "DeterministicResults.allocation: Allocation",
        Purpose::DeterministicLane,
    ),
    (
        "DeterministicResults.credits: CreditLoad",
        Purpose::DeterministicLane,
    ),
    (
        "DeterministicResults.downstream: Vec<DownstreamUnlock>",
        Purpose::DeterministicLane,
    ),
    (
        "DeterministicResults.gpa: Option<GpaScenario>",
        Purpose::DeterministicLane,
    ),
    (
        "DeterministicResults.prerequisites: Vec<PrerequisiteStanding>",
        Purpose::DeterministicLane,
    ),
    (
        "DeterministicResults.rule_contribution: RuleContribution",
        Purpose::DeterministicLane,
    ),
    (
        "DeterministicResults.schedule: ScheduleConflicts",
        Purpose::DeterministicLane,
    ),
    (
        "DimensionMove.dimension: ComparisonDimension",
        Purpose::ComparisonOrdering,
    ),
    (
        "DimensionMove.from_rank: usize",
        Purpose::ComparisonOrdering,
    ),
    ("DimensionMove.to_rank: usize", Purpose::ComparisonOrdering),
    (
        "DimensionPriority.order: Vec<ComparisonDimension>",
        Purpose::ComparisonOrdering,
    ),
    (
        "DimensionQuantity.prefers_more: bool",
        Purpose::ComparisonOrdering,
    ),
    (
        "DimensionQuantity.primary: u32",
        Purpose::ComparisonOrdering,
    ),
    (
        "DimensionQuantity.secondary: u32",
        Purpose::ComparisonOrdering,
    ),
    ("DownstreamCourse.course: CourseId", Purpose::FrozenInput),
    (
        "DownstreamCourse.official_prerequisites: Vec<OfficialPrerequisite>",
        Purpose::FrozenInput,
    ),
    (
        "DownstreamUnlock.completion: HypotheticalCompletion",
        Purpose::DeterministicLane,
    ),
    (
        "DownstreamUnlock.course: CourseId",
        Purpose::DeterministicLane,
    ),
    (
        "DownstreamUnlock.standing: UnlockStanding",
        Purpose::DeterministicLane,
    ),
    ("FrozenPlan.plan: PlanScenario", Purpose::StaleMarking),
    ("FrozenPlan.stale: Vec<StaleInput>", Purpose::StaleMarking),
    (
        "GpaScenario.average: HypotheticalTermAverage",
        Purpose::DeterministicLane,
    ),
    (
        "GpaScenario.excluded: Vec<(OfferingId, AverageExclusion)>",
        Purpose::DeterministicLane,
    ),
    (
        "GpaScenario.included: Vec<OfferingId>",
        Purpose::DeterministicLane,
    ),
    ("GpaScenario.scale: u8", Purpose::DeterministicLane),
    ("GpaScenario.scheme_id: String", Purpose::DeterministicLane),
    (
        "HypotheticalGraduation.plan: &'a PlanScenario",
        Purpose::DeterministicLane,
    ),
    (
        "InformalReadiness.entries: Vec<InformalReadinessEntry>",
        Purpose::ProjectedLane,
    ),
    (
        "InformalReadinessEntry.band: LikelihoodBand",
        Purpose::ProjectedLane,
    ),
    (
        "InformalReadinessEntry.covered: Vec<EntityId>",
        Purpose::ProjectedLane,
    ),
    (
        "InformalReadinessEntry.downstream_concept: EntityId",
        Purpose::ProjectedLane,
    ),
    (
        "InformalReadinessEntry.uncovered: Vec<EntityId>",
        Purpose::ProjectedLane,
    ),
    (
        "InformalRecommendation.downstream_concept: EntityId",
        Purpose::FrozenInput,
    ),
    (
        "InformalRecommendation.recommended_concepts: Vec<EntityId>",
        Purpose::FrozenInput,
    ),
    (
        "ModelCalibrationReport.engine_version: u32",
        Purpose::ModelCalibration,
    ),
    (
        "ModelCalibrationReport.entries: Vec<CalibrationEntry>",
        Purpose::ModelCalibration,
    ),
    (
        "ModelCalibrationReport.inputs_digest: ContentDigest",
        Purpose::ModelCalibration,
    ),
    (
        "ModelCalibrationReport.model_run_id: ModelRunId",
        Purpose::ModelCalibration,
    ),
    (
        "ModelCalibrationReport.plan_id: EntityId",
        Purpose::ModelCalibration,
    ),
    (
        "ObservedOccasion.concept_entity_id: EntityId",
        Purpose::ModelCalibration,
    ),
    (
        "ObservedOccasion.kind: OpportunityKind",
        Purpose::ModelCalibration,
    ),
    (
        "ObservedOccasion.offering_id: OfferingId",
        Purpose::ModelCalibration,
    ),
    (
        "OfficialConditions.enrolment_limit: EnrolmentLimitStanding",
        Purpose::FrozenInput,
    ),
    (
        "OfficialConditions.official_prerequisites: Vec<OfficialPrerequisite>",
        Purpose::FrozenInput,
    ),
    (
        "PathCoverage.entries: Vec<PathCoverageEntry>",
        Purpose::ProjectedLane,
    ),
    (
        "PathCoverageEntry.concept: EntityId",
        Purpose::ProjectedLane,
    ),
    (
        "PathCoverageEntry.kind: OpportunityKind",
        Purpose::ProjectedLane,
    ),
    (
        "PathCoverageEntry.likelihood: LikelihoodBand",
        Purpose::ProjectedLane,
    ),
    (
        "PathCoverageEntry.offering_id: OfferingId",
        Purpose::ProjectedLane,
    ),
    ("PathCoverageEntry.role: PathRole", Purpose::ProjectedLane),
    (
        "PathCoverageTargets.targets: Vec<(EntityId, PathRole)>",
        Purpose::FrozenInput,
    ),
    (
        "PlanAssumptions.completion: HypotheticalCompletion",
        Purpose::Assumption,
    ),
    (
        "PlanAssumptions.coverage: ProbabilisticCoverage",
        Purpose::Assumption,
    ),
    (
        "PlanAssumptions.workload: AssumedWorkload",
        Purpose::Assumption,
    ),
    (
        "PlanChoice.assumed_weekly_hours: WorkloadHoursRange",
        Purpose::FrozenInput,
    ),
    (
        "PlanChoice.category: CurriculumCategory",
        Purpose::FrozenInput,
    ),
    ("PlanChoice.course: CourseId", Purpose::FrozenInput),
    ("PlanChoice.credits: Credits", Purpose::FrozenInput),
    (
        "PlanChoice.enrolment_limit: EnrolmentLimitStanding",
        Purpose::FrozenInput,
    ),
    ("PlanChoice.offering_id: OfferingId", Purpose::FrozenInput),
    (
        "PlanChoice.official_prerequisites: Vec<OfficialPrerequisite>",
        Purpose::FrozenInput,
    ),
    ("PlanChoice.seat: ConfirmedSeat", Purpose::FrozenInput),
    (
        "PlanChoice.syllabus_concepts: Vec<SyllabusConceptSignal>",
        Purpose::FrozenInput,
    ),
    (
        "PlanInputs.assumptions: PlanAssumptions",
        Purpose::FrozenInput,
    ),
    ("PlanInputs.basis: ScenarioBasis", Purpose::FrozenInput),
    ("PlanInputs.choices: Vec<PlanChoice>", Purpose::FrozenInput),
    (
        "PlanInputs.completed_courses: BTreeSet<CourseId>",
        Purpose::FrozenInput,
    ),
    (
        "PlanInputs.downstream_courses: Vec<DownstreamCourse>",
        Purpose::FrozenInput,
    ),
    (
        "PlanInputs.grade_assumptions: Option<StatedGradeAssumptions>",
        Purpose::FrozenInput,
    ),
    (
        "PlanInputs.grading_scheme: GradingScheme",
        Purpose::FrozenInput,
    ),
    (
        "PlanInputs.informal_recommendations: Vec<InformalRecommendation>",
        Purpose::FrozenInput,
    ),
    ("PlanInputs.model_run_id: ModelRunId", Purpose::FrozenInput),
    (
        "PlanInputs.path_targets: PathCoverageTargets",
        Purpose::FrozenInput,
    ),
    (
        "PlanInputs.relevance: Vec<RelevanceSignal>",
        Purpose::FrozenInput,
    ),
    ("PlanInputs.scenario_id: EntityId", Purpose::FrozenInput),
    (
        "PlanInputs.workload_bias: BiasDisclosure",
        Purpose::FrozenInput,
    ),
    (
        "PlanScenario.assumptions: PlanAssumptions",
        Purpose::Assumption,
    ),
    ("PlanScenario.basis: ScenarioBasis", Purpose::Identity),
    (
        "PlanScenario.choices: Vec<PlanChoice>",
        Purpose::FrozenInput,
    ),
    (
        "PlanScenario.deterministic: DeterministicResults",
        Purpose::DeterministicLane,
    ),
    ("PlanScenario.id: EntityId", Purpose::Identity),
    (
        "PlanScenario.inputs_digest: ContentDigest",
        Purpose::Identity,
    ),
    (
        "PlanScenario.projections: ProjectedResults",
        Purpose::ProjectedLane,
    ),
    (
        "PrerequisiteStanding.enrolment_limit: EnrolmentLimitStanding",
        Purpose::DeterministicLane,
    ),
    (
        "PrerequisiteStanding.offering_id: OfferingId",
        Purpose::DeterministicLane,
    ),
    (
        "PrerequisiteStanding.unmet: Vec<CourseId>",
        Purpose::DeterministicLane,
    ),
    (
        "ProjectedResults.opportunities: ScenarioProjection",
        Purpose::ProjectedLane,
    ),
    (
        "ProjectedResults.path_coverage: PathCoverage",
        Purpose::ProjectedLane,
    ),
    (
        "ProjectedResults.readiness: InformalReadiness",
        Purpose::ProjectedLane,
    ),
    (
        "ProjectedResults.relevance: RelevanceProjection",
        Purpose::ProjectedLane,
    ),
    (
        "ProjectedResults.workload: ProjectedWorkload",
        Purpose::ProjectedLane,
    ),
    (
        "ProjectedWorkload.band: WorkloadHoursRange",
        Purpose::ProjectedLane,
    ),
    (
        "ProjectedWorkload.bias: BiasDisclosure",
        Purpose::ProjectedLane,
    ),
    (
        "ProjectedWorkload.proposed: ProjectedWorkloadRange",
        Purpose::ProjectedLane,
    ),
    (
        "RecomputeConsent.covers: Vec<StaleInput>",
        Purpose::StaleMarking,
    ),
    (
        "RecomputeConsent.decision: UserDecision",
        Purpose::StaleMarking,
    ),
    ("RecomputeConsent.plan_id: EntityId", Purpose::StaleMarking),
    (
        "RelevanceEntry.band: LikelihoodBand",
        Purpose::ProjectedLane,
    ),
    (
        "RelevanceEntry.basis: OpportunityBasis",
        Purpose::ProjectedLane,
    ),
    (
        "RelevanceEntry.offering_id: OfferingId",
        Purpose::ProjectedLane,
    ),
    (
        "RelevanceEntry.subject: RelevanceSubject",
        Purpose::ProjectedLane,
    ),
    ("RelevanceEntry.target: EntityId", Purpose::ProjectedLane),
    (
        "RelevanceProjection.entries: Vec<RelevanceEntry>",
        Purpose::ProjectedLane,
    ),
    ("RelevanceSignal.band: LikelihoodBand", Purpose::FrozenInput),
    (
        "RelevanceSignal.basis: OpportunityBasis",
        Purpose::FrozenInput,
    ),
    (
        "RelevanceSignal.offering_id: OfferingId",
        Purpose::FrozenInput,
    ),
    (
        "RelevanceSignal.subject: RelevanceSubject",
        Purpose::FrozenInput,
    ),
    ("RelevanceSignal.target: EntityId", Purpose::FrozenInput),
    (
        "ReorderingExplanation.decisive: Option<ComparisonDimension>",
        Purpose::ComparisonOrdering,
    ),
    (
        "ReorderingExplanation.moved: Vec<DimensionMove>",
        Purpose::ComparisonOrdering,
    ),
    (
        "ReorderingExplanation.order_after: Vec<EntityId>",
        Purpose::ComparisonOrdering,
    ),
    (
        "ReorderingExplanation.order_before: Vec<EntityId>",
        Purpose::ComparisonOrdering,
    ),
    (
        "RuleContribution.completion: HypotheticalCompletion",
        Purpose::DeterministicLane,
    ),
    (
        "RuleContribution.contributions: Vec<CategoryContribution>",
        Purpose::DeterministicLane,
    ),
    (
        "ScenarioBasis.knowledge_state_as_of: TimestampMillis",
        Purpose::Identity,
    ),
    (
        "ScenarioBasis.offering_catalog_snapshot: ContentDigest",
        Purpose::Identity,
    ),
    (
        "ScenarioBasis.requirement_set_hash: ContentDigest",
        Purpose::Identity,
    ),
    (
        "ScenarioBasis.student_record_snapshot: ContentDigest",
        Purpose::Identity,
    ),
    (
        "ScheduleConflict.earlier_choice: OfferingId",
        Purpose::DeterministicLane,
    ),
    (
        "ScheduleConflict.from_minute: u16",
        Purpose::DeterministicLane,
    ),
    (
        "ScheduleConflict.later_choice: OfferingId",
        Purpose::DeterministicLane,
    ),
    (
        "ScheduleConflict.to_minute: u16",
        Purpose::DeterministicLane,
    ),
    (
        "ScheduleConflict.weekday: Weekday",
        Purpose::DeterministicLane,
    ),
    (
        "ScheduleConflicts.conflicts: Vec<ScheduleConflict>",
        Purpose::DeterministicLane,
    ),
    ("StaleInput.cause: StaleCause", Purpose::StaleMarking),
    (
        "StaleInput.observed_at: TimestampMillis",
        Purpose::StaleMarking,
    ),
    ("StaleInput.offering_id: OfferingId", Purpose::StaleMarking),
    (
        "StatedGradeAssumption.grade: GradeSymbol",
        Purpose::Assumption,
    ),
    (
        "StatedGradeAssumption.offering_id: OfferingId",
        Purpose::Assumption,
    ),
    (
        "StatedGradeAssumptions.stated: Vec<StatedGradeAssumption>",
        Purpose::Assumption,
    ),
];

/// The positions the crate exposes as `pub`.
///
/// Exactly the input struct's own fields. Every value the engine *produces* has
/// private fields and one crate-private producer, so a plan cannot be assembled
/// to say something the engine did not compute.
const REVIEWED_PUBLIC_POSITIONS: &[&str] = &[
    "PlanInputs.assumptions: PlanAssumptions",
    "PlanInputs.basis: ScenarioBasis",
    "PlanInputs.choices: Vec<PlanChoice>",
    "PlanInputs.completed_courses: BTreeSet<CourseId>",
    "PlanInputs.downstream_courses: Vec<DownstreamCourse>",
    "PlanInputs.grade_assumptions: Option<StatedGradeAssumptions>",
    "PlanInputs.grading_scheme: GradingScheme",
    "PlanInputs.informal_recommendations: Vec<InformalRecommendation>",
    "PlanInputs.model_run_id: ModelRunId",
    "PlanInputs.path_targets: PathCoverageTargets",
    "PlanInputs.relevance: Vec<RelevanceSignal>",
    "PlanInputs.scenario_id: EntityId",
    "PlanInputs.workload_bias: BiasDisclosure",
];

/// Every item this crate imports from outside itself.
///
/// The whole set, flattened out of every `use` in the package and compared in
/// both directions. `P2-N6` holds its own posture the same way, and the reason
/// is the one that test gives: a source grep for a forbidden name passes for a
/// crate that has not spelled it *yet*, and an inventory does not.
const REVIEWED_EXTERNAL_IMPORTS: &[&str] = &[
    "academic_critical_path::CriticalPathError",
    "academic_critical_path::CriticalPathResult",
    "academic_critical_path::PathRole",
    "academic_curriculum::Credits",
    "academic_curriculum::CurriculumCategory",
    "academic_curriculum::CurriculumError",
    "academic_curriculum::Meeting",
    "academic_curriculum::OfficialPrerequisite",
    "academic_curriculum::Weekday",
    "academic_domain::ContentDigest",
    "academic_domain::CourseId",
    "academic_domain::Decimal",
    "academic_domain::DomainError",
    "academic_domain::EntityId",
    "academic_domain::ModelRunId",
    "academic_domain::OfferingId",
    "academic_domain::TimestampMillis",
    "academic_offering::CancellationNotice",
    "academic_offering::ConfirmedSeat",
    "academic_proposal::UserDecision",
    "academic_record::RecordError",
    "academic_record::decimal",
    "academic_record::grade::GradeSymbol",
    "academic_record::grade::GradingScheme",
    "academic_review::BiasDisclosure",
    "academic_review::ReviewError",
    "academic_scenario::LikelihoodBand",
    "academic_scenario::OpportunityBasis",
    "academic_scenario::OpportunityKind",
    "academic_scenario::ProjectedWorkloadRange",
    "academic_scenario::ProjectionCalibration",
    "academic_scenario::ProposalProvenance",
    "academic_scenario::Proposed",
    "academic_scenario::ScenarioAssumption",
    "academic_scenario::ScenarioChoice",
    "academic_scenario::ScenarioError",
    "academic_scenario::ScenarioInputs",
    "academic_scenario::ScenarioProjection",
    "academic_scenario::SyllabusConceptSignal",
    "academic_scenario::WorkloadHoursRange",
    "academic_scenario::project",
    "serde::Deserialize",
    "serde::Serialize",
    "sha2::Digest",
    "sha2::Sha256",
    "std::cmp::Ordering",
    "std::collections::BTreeMap",
    "std::collections::BTreeSet",
    "thiserror::Error",
];

/// The public functions section 22.4's module declares.
/// The functions section 22.4's module declares, public and private alike.
///
/// Private ones are in the list on purpose. `read` and `compare_on` are the two
/// that touch a plan's numbers, and both are per-dimension: `read` produces one
/// dimension's reading and `compare_on` compares two of them on one dimension.
/// A private `total`, `weighted` or `score` would fail here as an entry nobody
/// wrote down, which is the whole point of comparing the set rather than
/// refusing a list of names.
const REVIEWED_COMPARISON_FUNCTIONS: &[&str] = &[
    "as_str",
    "between",
    "compare",
    "compare_on",
    "count",
    "decisive",
    "decisive_dimension",
    "dimension",
    "from_rank",
    "lane",
    "moved",
    "of",
    "order",
    "order_after",
    "order_before",
    "order_changed",
    "priority",
    "rank_of",
    "ranked",
    "ranked_ids",
    "read",
    "spec_certainty",
    "spec_label",
    "to_rank",
];

// ---------------------------------------------------------------------------
// the inventory itself
// ---------------------------------------------------------------------------

#[test]
fn every_declared_field_position_is_reviewed() -> TestResult {
    let declared = support::declared_field_positions()?;
    let reviewed: BTreeMap<&str, Purpose> = REVIEWED_FIELD_POSITIONS.iter().copied().collect();
    assert_eq!(
        reviewed.len(),
        REVIEWED_FIELD_POSITIONS.len(),
        "the reviewed inventory names one position twice"
    );
    let declared_keys: BTreeSet<&str> = declared.keys().map(String::as_str).collect();
    let reviewed_keys: BTreeSet<&str> = reviewed.keys().copied().collect();
    let unreviewed: Vec<&&str> = declared_keys.difference(&reviewed_keys).collect();
    assert!(
        unreviewed.is_empty(),
        "this crate declares field positions nobody reviewed: {unreviewed:?}"
    );
    let vanished: Vec<&&str> = reviewed_keys.difference(&declared_keys).collect();
    assert!(
        vanished.is_empty(),
        "the reviewed inventory names positions this crate no longer declares: {vanished:?}"
    );

    // Exactly the input struct's fields are public.
    let public: BTreeSet<&str> = declared
        .iter()
        .filter(|(_, visibility)| visibility.as_str() == "pub")
        .map(|(position, _)| position.as_str())
        .collect();
    let reviewed_public: BTreeSet<&str> = REVIEWED_PUBLIC_POSITIONS.iter().copied().collect();
    assert_eq!(
        public, reviewed_public,
        "the set of publicly assignable field positions changed"
    );

    // Every purpose in the closed set is actually used, so an arm nobody uses
    // is not quietly carried as a category a later position could hide in.
    let used: BTreeSet<Purpose> = reviewed.values().copied().collect();
    let all = [
        Purpose::Identity,
        Purpose::Assumption,
        Purpose::FrozenInput,
        Purpose::DeterministicLane,
        Purpose::ProjectedLane,
        Purpose::ComparisonOrdering,
        Purpose::StaleMarking,
        Purpose::ModelCalibration,
    ];
    assert_eq!(
        used,
        all.into_iter().collect::<BTreeSet<_>>(),
        "a reviewed purpose is declared and never used"
    );

    // The calibration module holds nothing but calibration positions, which is
    // the structural half of `end_of_term_calibration_emits_no_user_score`: a
    // field about a person would have to be given some other purpose, and any
    // other purpose inside those three types fails here.
    for (position, purpose) in REVIEWED_FIELD_POSITIONS {
        let calibration_type = position.starts_with("ModelCalibrationReport.")
            || position.starts_with("CalibrationEntry.")
            || position.starts_with("ObservedOccasion.");
        assert_eq!(
            calibration_type,
            *purpose == Purpose::ModelCalibration,
            "{position} and its purpose disagree about the calibration boundary"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. plan_scenario_never_writes_actual_state
// ---------------------------------------------------------------------------

/// Every `use ...;` statement of one source, with comments removed.
fn use_statements(source: &str) -> Vec<String> {
    let code = support::strip_non_code(source);
    let mut found = Vec::new();
    let mut rest = code.as_str();
    while let Some(at) = rest.find("use ") {
        let boundary = rest[..at]
            .chars()
            .next_back()
            .is_none_or(|character| !character.is_alphanumeric() && character != '_');
        let after = &rest[at + 4..];
        let Some(end) = after.find(';') else {
            break;
        };
        if boundary {
            found.push(
                after[..end]
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        rest = &after[end..];
    }
    found
}

/// The leaves of one `use` path, with every brace group expanded.
fn flatten_use(path: &str) -> Vec<String> {
    let trimmed = path.trim();
    let (Some(open), Some(close)) = (trimmed.find('{'), trimmed.rfind('}')) else {
        return vec![trimmed.replace(' ', "")];
    };
    let head = trimmed[..open].trim().trim_end_matches(':').to_owned();
    let mut groups: Vec<String> = Vec::new();
    let mut depth = 0_usize;
    let mut current = String::new();
    for character in trimmed[open + 1..close].chars() {
        match character {
            '{' => {
                depth += 1;
                current.push(character);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if depth == 0 => groups.push(std::mem::take(&mut current)),
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        groups.push(current);
    }
    groups
        .into_iter()
        .filter(|group| !group.trim().is_empty())
        .flat_map(|group| {
            flatten_use(&group)
                .into_iter()
                .map(|tail| format!("{head}::{tail}").replace(' ', ""))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn plan_scenario_never_writes_actual_state() -> TestResult {
    // The canonical writers are unreachable through an edge of any kind.
    let closure = support::declared_closure("academic-what-if")?;
    assert!(
        !closure.is_empty(),
        "the closure walk found no dependency at all"
    );
    for writer in ["academic-store", "academic-store-platform"] {
        assert!(
            !closure.contains(writer),
            "academic-what-if reaches the canonical writer {writer}"
        );
        assert!(
            !support::declared_closure(writer)?.contains("academic-what-if"),
            "{writer} reaches the plan crate"
        );
    }
    // The control: the same walk from a package that does reach the writer must
    // say so, or the assertions above are a walk that reaches nothing.
    assert!(
        support::declared_closure("academic-projections")?.contains("academic-store"),
        "the closure control failed: academic-projections does not reach the writer"
    );

    // The whole import inventory, compared both ways. This crate's own modules
    // are read out of `lib.rs` rather than listed, so a module added later is
    // recognised as this crate's without anybody editing this test, and a
    // package added later is not.
    let own_modules: BTreeSet<String> =
        fs::read_to_string(support::crate_root().join("src").join("lib.rs"))?
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub mod "))
            .filter_map(|rest| rest.strip_suffix(';'))
            .map(str::to_owned)
            .collect();
    assert!(
        own_modules.len() > 5,
        "the module walk found only {} modules",
        own_modules.len()
    );
    let mut imported = BTreeSet::new();
    for path in support::product_sources()? {
        let source = fs::read_to_string(&path)?;
        for statement in use_statements(&source) {
            for leaf in flatten_use(&statement) {
                let root = leaf.split("::").next().unwrap_or_default().to_owned();
                if root != "crate" && !own_modules.contains(&root) {
                    imported.insert(leaf);
                }
            }
        }
    }
    let reviewed: BTreeSet<String> = REVIEWED_EXTERNAL_IMPORTS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    assert_eq!(
        imported, reviewed,
        "this crate's external import inventory changed"
    );

    // Nothing in the package can mutate anything it holds, and nothing opens a
    // file, a socket, a process or a clock. The constructs are compared as
    // whole identifiers over stripped code; the two mutable-borrow spellings
    // are matched as text because they are not identifiers.
    let mutation = [
        "RefCell",
        "Cell",
        "Mutex",
        "RwLock",
        "UnsafeCell",
        "OnceCell",
        "OnceLock",
    ];
    let ambient = [
        "fs",
        "File",
        "net",
        "TcpStream",
        "UdpSocket",
        "Command",
        "process",
        "env",
        "SystemTime",
        "Instant",
        "thread",
        "include_str",
        "include_bytes",
    ];
    for path in support::product_sources()? {
        let code = support::strip_non_code(&fs::read_to_string(&path)?);
        let names = support::identifiers(&code);
        for name in mutation.iter().chain(ambient.iter()) {
            assert!(!names.contains(*name), "{} names {name}", path.display());
        }
        assert!(
            !code.contains("&mut ") && !code.contains("static mut"),
            "{} takes a mutable borrow",
            path.display()
        );
    }
    // The control: the same reader over the workspace's own writer must find
    // the ambient names, or the zero above is a reader that reads nothing.
    let writer = support::strip_non_code(&fs::read_to_string(
        support::workspace_root()
            .join("crates")
            .join("store")
            .join("src")
            .join("profile.rs"),
    )?);
    let writer_names = support::identifiers(&writer);
    let hits = ambient
        .iter()
        .filter(|name| writer_names.contains(**name))
        .count();
    assert!(
        hits >= 2,
        "the ambient-name control found only {hits} names in the writer crate"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. no_mastery_delta_in_plan_output
// ---------------------------------------------------------------------------

/// `P2-N2`'s ladder vocabulary, read out of that crate's own source.
///
/// Derived rather than typed here. A rung, a facet or a helper added to
/// `crates/knowledge-state/src/ladder.rs` extends this guard without anybody
/// editing this file, which is the difference between an inventory and the
/// forbidden-name list `P2-X2`'s injections showed passes for every spelling
/// nobody predicted.
///
/// Item names only, never enum variants: `FacetStrength` spells `Moderate`,
/// which is also a `LikelihoodBand` this crate legitimately carries, and a
/// guard that refused it would be one somebody weakens until it finds nothing.
/// The mastery *rungs* are added from `academic-domain`'s own enumeration
/// instead, where they are unambiguous.
fn ladder_vocabulary() -> Result<BTreeSet<String>, Box<dyn Error>> {
    let ladder = fs::read_to_string(
        support::workspace_root()
            .join("crates")
            .join("knowledge-state")
            .join("src")
            .join("ladder.rs"),
    )?;
    let mut found = BTreeSet::new();
    for line in support::strip_non_code(&ladder).lines() {
        // Longest first, and the loop stops at the first match: `pub const `
        // is a prefix of `pub const fn `, and matching it first would take the
        // name of a function to be the word `fn`.
        for keyword in [
            "pub const fn ",
            "pub const ",
            "pub enum ",
            "pub struct ",
            "pub fn ",
            "pub type ",
        ] {
            let Some(rest) = line.strip_prefix(keyword) else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if !name.is_empty() {
                found.insert(name);
            }
            break;
        }
    }
    // The six rungs, read out of `academic-domain`'s own `MasteryLevel`.
    let domain = fs::read_to_string(
        support::workspace_root()
            .join("crates")
            .join("domain")
            .join("src")
            .join("lib.rs"),
    )?;
    let block = support::block_of(&domain, "pub enum MasteryLevel {")?;
    let mut rungs = 0_usize;
    for line in block.lines().skip(1) {
        let name: String = line
            .trim()
            .trim_end_matches(',')
            .chars()
            .take_while(|character| character.is_alphanumeric())
            .collect();
        if name.is_empty() || !name.starts_with(char::is_uppercase) {
            continue;
        }
        rungs += 1;
        found.insert(name);
    }
    assert!(rungs >= 5, "MasteryLevel parsed to only {rungs} rungs");
    found.insert("MasteryLevel".to_owned());
    assert!(
        found.len() >= 10,
        "the ladder vocabulary parsed to only {} names",
        found.len()
    );
    Ok(found)
}

#[test]
fn no_mastery_delta_in_plan_output() -> TestResult {
    let forbidden = ladder_vocabulary()?;

    // Not one of them appears anywhere in this package's code.
    for path in support::product_sources()? {
        let code = support::strip_non_code(&fs::read_to_string(&path)?);
        let names = support::identifiers(&code);
        let spoken: Vec<&String> = forbidden.intersection(&names).collect();
        assert!(
            spoken.is_empty(),
            "{} names the mastery ladder: {spoken:?}",
            path.display()
        );
    }

    // Nor in the reviewed field inventory, on either side of the colon — a
    // field named `mastery_delta` and a field typed `MasteryLevel` both fail.
    for (position, _) in REVIEWED_FIELD_POSITIONS {
        for name in &forbidden {
            assert!(
                !position.contains(name.as_str()),
                "the field position {position} names {name}"
            );
        }
        let lowered = position.to_ascii_lowercase();
        assert!(
            !lowered.contains("mastery"),
            "the field position {position} is about mastery"
        );
    }

    // The control. The same reader over `P2-C7`'s own proposal module, which
    // exists precisely to seal a projected mastery, must find several of them —
    // otherwise the zero above is a reader that reads nothing.
    let sealed = support::strip_non_code(&fs::read_to_string(
        support::workspace_root()
            .join("crates")
            .join("scenario")
            .join("src")
            .join("proposed.rs"),
    )?);
    let sealed_names = support::identifiers(&sealed);
    let hits = forbidden.intersection(&sealed_names).count();
    assert!(
        hits >= 4,
        "the ladder control found only {hits} names in P2-C7's proposal module"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. no_default_recommendation_score
// ---------------------------------------------------------------------------

#[test]
fn no_default_recommendation_score() -> TestResult {
    let specification = support::specification()?;
    let block = support::section(&specification, "### 22.4 비교 UX")?;
    assert!(
        block.contains("하나의 “추천 점수”를 기본으로 표시하지 않는다"),
        "section 22.4 no longer refuses a default recommendation score"
    );

    // There is no `Default` anywhere in the package: not derived, not
    // implemented. A neutral importance ordering shipped as a default would be
    // this product answering the importance question on the user's behalf.
    for path in support::product_sources()? {
        let code = support::strip_non_code(&fs::read_to_string(&path)?);
        assert!(
            !code.contains("Default"),
            "{} declares a Default",
            path.display()
        );
    }
    // The control: the same reader over `P2-U8`'s own draft, which does derive
    // one, must find it.
    let draft = support::strip_non_code(&fs::read_to_string(
        support::workspace_root()
            .join("crates")
            .join("review")
            .join("src")
            .join("bias.rs"),
    )?);
    assert!(
        draft.contains("Default"),
        "the Default control failed: P2-U8's bias draft no longer derives one"
    );

    // Section 22.4's module declares exactly the reviewed functions. A
    // `total`, a `score` or a `weighted` added there fails as an entry nobody
    // wrote down rather than as a forbidden name.
    let source = support::module_source("comparison")?;
    let mut declared: Vec<String> = support::function_names(&source);
    declared.sort_unstable();
    declared.dedup();
    let reviewed: Vec<String> = {
        let mut reviewed: Vec<String> = REVIEWED_COMPARISON_FUNCTIONS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect();
        reviewed.sort_unstable();
        reviewed.dedup();
        reviewed
    };
    let extra: Vec<&String> = declared.iter().filter(|f| !reviewed.contains(f)).collect();
    let missing: Vec<&String> = reviewed.iter().filter(|f| !declared.contains(f)).collect();
    assert!(
        extra.is_empty(),
        "section 22.4's module declares functions nobody reviewed: {extra:?}"
    );
    assert!(
        missing.is_empty(),
        "section 22.4's module no longer declares: {missing:?}"
    );

    // The whole set of constants section 22.4's module declares. A neutral
    // ordering shipped as a `const` rather than as a `Default` would be the
    // same refusal by another route, and it spells none of the words a
    // forbidden-name list would hold.
    let constants: BTreeSet<String> = source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("pub const ")
                .or_else(|| line.trim().strip_prefix("const "))
        })
        .filter(|rest| !rest.starts_with("fn "))
        .map(|rest| {
            rest.chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .collect();
    assert_eq!(
        constants,
        ["COMPARISON_DIMENSIONS".to_owned()]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "section 22.4's module declares a different constant set"
    );

    // No field position is a cross-dimension aggregate. The only numeric
    // positions the inventory holds are per-dimension readings inside a private
    // type, ranks, credits, a published scale and an engine version — each of
    // which is checked by name here so that a new numeric position has to be
    // added to this list as well as to the inventory.
    let numeric: BTreeSet<&str> = REVIEWED_FIELD_POSITIONS
        .iter()
        .filter(|(position, _)| {
            [
                ": u8",
                ": u16",
                ": u32",
                ": u64",
                ": usize",
                ": i64",
                ": Decimal",
            ]
            .iter()
            .any(|suffix| position.ends_with(suffix))
        })
        .map(|(position, _)| *position)
        .collect();
    let reviewed_numeric: BTreeSet<&str> = [
        "CategoryContribution.credits: u16",
        "CreditLoad.requested: u16",
        "DimensionMove.from_rank: usize",
        "DimensionMove.to_rank: usize",
        "DimensionQuantity.primary: u32",
        "DimensionQuantity.secondary: u32",
        "GpaScenario.scale: u8",
        "ModelCalibrationReport.engine_version: u32",
        "ScheduleConflict.from_minute: u16",
        "ScheduleConflict.to_minute: u16",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        numeric, reviewed_numeric,
        "this crate's numeric field positions changed"
    );

    // Every function in the package that hands out a bare number, compared in
    // both directions. `P2-N7` measured why this is not the same guard as the
    // field inventory above: a method summing every axis of a `P2-N6` cost
    // vector and named `as_one_number` spells none of the twelve folding
    // spellings that suite refuses, declares no field, adds no `use` item, and
    // passed that whole suite and `clippy` alike. A scalar this API hands out
    // appears here as an extra key whatever it is called.
    //
    // Each of the entries below is an ordinal, a count, a credit value, a
    // minute, a published scale or an engine version. None of them combines two
    // section 22.4 dimensions, and none of them stands for a plan.
    let reviewed_numeric_returns: BTreeSet<String> = [
        "calibration.rs count_of -> usize",
        "calibration.rs engine_version -> u32",
        "comparison.rs count -> u32",
        "comparison.rs from_rank -> usize",
        "comparison.rs order -> usize",
        "comparison.rs rank_of -> usize",
        "comparison.rs to_rank -> usize",
        "deterministic.rs credits -> u16",
        "deterministic.rs from_minute -> u16",
        "deterministic.rs overlap -> u16",
        "deterministic.rs requested -> u16",
        "deterministic.rs scale -> u8",
        "deterministic.rs to_minute -> u16",
        "inputs.rs weekday_index -> u8",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut declared_numeric = BTreeSet::new();
    for path in support::product_sources()? {
        let module = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        declared_numeric.extend(support::numeric_returns(
            &module,
            &fs::read_to_string(&path)?,
        ));
    }
    assert_eq!(
        declared_numeric, reviewed_numeric_returns,
        "the set of functions handing out a bare number changed"
    );
    // The control: the same reader over `P2-N6`'s own vector module must find
    // the two interval ends, or the comparison above is a reader that reads
    // nothing.
    let vector = support::numeric_returns(
        "vector.rs",
        &fs::read_to_string(
            support::workspace_root()
                .join("crates")
                .join("critical-path")
                .join("src")
                .join("vector.rs"),
        )?,
    );
    assert!(
        vector.len() >= 2,
        "the numeric-return control found only {} in P2-N6's vector module",
        vector.len()
    );

    // And behaviourally: two importance orderings produce two orders over one
    // unchanged pair of plans, and there is no third value standing for either.
    let plan_a = academic_what_if::simulate(&support::plan_a()?)?;
    let plan_b = academic_what_if::simulate(&support::plan_b()?)?;
    let plans = [&plan_a, &plan_b];
    let mut orders = BTreeSet::new();
    for leading in academic_what_if::COMPARISON_DIMENSIONS {
        let mut order = vec![leading];
        order.extend(
            academic_what_if::COMPARISON_DIMENSIONS
                .into_iter()
                .filter(|dimension| *dimension != leading),
        );
        let view =
            academic_what_if::compare(&plans, &academic_what_if::DimensionPriority::of(order)?)?;
        orders.insert(view.ranked_ids());
    }
    assert!(
        orders.len() > 1,
        "every importance ordering produced one order, so something is aggregating them"
    );
    assert_eq!(plan_a, academic_what_if::simulate(&support::plan_a()?)?);
    assert_eq!(plan_b, academic_what_if::simulate(&support::plan_b()?)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// the frozen inputs
// ---------------------------------------------------------------------------

#[test]
fn frozen_inputs_are_the_whole_of_what_the_engine_reads() -> TestResult {
    // Every field the two input types declare has a digest arm, and every
    // digest arm names a field that exists. This is the guard for the defect
    // `P2-N6` found in its own engine: frozen inputs that omitted three of the
    // things the engine read, so two cases with byte-identical inputs had
    // different answers.
    let declared = support::declared_field_positions()?;
    for (type_name, arms) in [
        (
            "PlanInputs",
            PLAN_INPUT_FIELDS
                .into_iter()
                .map(PlanInputField::field_name)
                .collect::<BTreeSet<_>>(),
        ),
        (
            "PlanChoice",
            PLAN_CHOICE_FIELDS
                .into_iter()
                .map(PlanChoiceField::field_name)
                .collect::<BTreeSet<_>>(),
        ),
    ] {
        let fields: BTreeSet<&str> = declared
            .keys()
            .filter_map(|position| position.strip_prefix(&format!("{type_name}.")))
            .filter_map(|rest| rest.split_once(": ").map(|(name, _)| name))
            .collect();
        assert!(!fields.is_empty(), "{type_name} parsed to no fields");
        assert_eq!(
            fields, arms,
            "{type_name} and its digest arms disagree about the field set"
        );
    }

    // Varying any one field moves the digest, and the plan with it.
    let base = support::plan_a()?;
    let base_digest = plan_inputs_digest(&base);
    let mut moved = 0_usize;
    for variant in support::input_variants()? {
        let (name, inputs) = variant;
        assert_ne!(
            plan_inputs_digest(&inputs),
            base_digest,
            "varying {name} left the frozen-input digest alone"
        );
        moved += 1;
    }
    assert_eq!(
        moved,
        PLAN_INPUT_FIELDS.len(),
        "not every frozen input field was varied"
    );

    // And the same, one field of one choice at a time.
    let mut choice_moved = 0_usize;
    for (name, inputs) in support::choice_variants()? {
        assert_ne!(
            plan_inputs_digest(&inputs),
            base_digest,
            "varying a choice's {name} left the frozen-input digest alone"
        );
        choice_moved += 1;
    }
    assert_eq!(
        choice_moved,
        PLAN_CHOICE_FIELDS.len(),
        "not every choice field was varied"
    );

    // Equal inputs give equal plans, digest included.
    let first = academic_what_if::simulate(&base)?;
    let second = academic_what_if::simulate(&support::plan_a()?)?;
    assert_eq!(first, second);
    assert_eq!(first.inputs_digest(), base_digest);

    // The digest of the fixture plan, pinned. Every observation above
    // recomputes it with the same code, so a change to the encoding — a dropped
    // field key, a dropped length prefix, a reordered arm — moves nothing any
    // of them can see. This is the one assertion that does, and the cost of
    // changing the encoding is changing it here.
    assert_eq!(
        base_digest.to_string(),
        "sha256:331194491cf86c086d03c0f892b899864d6cb6a04aa88b9631a17154d76a1936",
        "the frozen-input encoding changed"
    );
    Ok(())
}
