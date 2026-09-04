//! Everything one what-if run reads, and the digest that covers all of it.
//!
//! # The frozen inputs are the whole of what the engine reads
//!
//! There is no clock, no RNG, no file, no socket and no ambient state in this
//! crate. [`simulate`](crate::simulate) takes a [`PlanInputs`] and reads
//! nothing else, so two runs over equal inputs produce equal plans.
//!
//! That is easy to *say* and it is exactly the claim `P2-N6` found broken in
//! its own engine: its frozen inputs omitted the hypergraph, the constraint
//! inputs and the acquisition options, so two corpus cases with byte-identical
//! frozen inputs had different expected outputs. The guard here is therefore
//! structural rather than declared. [`PlanInputField`] and [`PlanChoiceField`]
//! are the fields [`plan_inputs_digest`] hashes, each in a total `match`, and
//! `frozen_inputs_are_the_whole_of_what_the_engine_reads` parses the field
//! declarations of every input type out of this file and compares them against
//! those enumerations **in both directions**, then varies each field in turn
//! and requires the digest to move. A field added to an input struct without a
//! digest arm fails as an undigested declaration; an arm added without a field
//! fails as a digest of something that does not exist.
//!
//! # A confirmed seat is the only timetable there is
//!
//! [`PlanChoice::of`] takes `P2-U5`'s [`ConfirmedSeat`] by value. See
//! [`crate::deterministic`]: a `HISTORICALLY_LIKELY` standing has no seat, so
//! the deterministic lane cannot be reached from a predicted offering at all.
//!
//! # The critical path arrives as `P2-N6` computed it
//!
//! [`PathCoverageTargets::from_critical_path`] reads
//! [`CriticalPathResult::roles`] and nothing else. This crate holds no second
//! notion of what is on a path and computes no path: section 22.3's *Critical
//! Path coverage 가능성* is an overlap between that engine's answer and the
//! published material of the plan's choices.

use std::collections::BTreeSet;

use academic_critical_path::{CriticalPathResult, PathRole};
use academic_curriculum::{Credits, CurriculumCategory, OfficialPrerequisite, Weekday};
use academic_domain::{ContentDigest, CourseId, EntityId, ModelRunId, OfferingId};
use academic_offering::ConfirmedSeat;
use academic_record::grade::GradingScheme;
use academic_review::BiasDisclosure;
use academic_scenario::{
    LikelihoodBand, OpportunityBasis, SyllabusConceptSignal, WorkloadHoursRange,
};
use sha2::{Digest, Sha256};

use crate::{
    assumption::{
        HypotheticalCompletion, PlanAssumptions, ProbabilisticCoverage, StatedGradeAssumptions,
    },
    basis::ScenarioBasis,
    deterministic::EnrolmentLimitStanding,
    error::WhatIfError,
};

/// Domain separator for the frozen-input digest.
const INPUTS_DIGEST_DOMAIN: &str = "academic-what-if/plan-inputs/v1";

/// Which of section 22.3's two relevance subjects a signal is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelevanceSubject {
    /// A project goal the user is working towards.
    Project,
    /// A career direction the user has recorded.
    Career,
}

/// Both, in section 22.3's own order — `project/career relevance`.
pub const RELEVANCE_SUBJECTS: [RelevanceSubject; 2] =
    [RelevanceSubject::Project, RelevanceSubject::Career];

impl RelevanceSubject {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "PROJECT",
            Self::Career => "CAREER",
        }
    }
}

/// One reading that a choice is relevant to a project goal or a career target.
///
/// The strength is `P2-C7`'s [`LikelihoodBand`] and the ground is that crate's
/// [`OpportunityBasis`]. Neither is a number, and there is no arithmetic
/// anywhere in this crate that combines two of them: section 22.4's closing
/// sentence refuses a single aggregate score, and a relevance percentage would
/// be the first ingredient of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelevanceSignal {
    offering_id: OfferingId,
    subject: RelevanceSubject,
    target: EntityId,
    band: LikelihoodBand,
    basis: OpportunityBasis,
}

impl RelevanceSignal {
    /// Records one relevance reading.
    #[must_use]
    pub const fn of(
        offering_id: OfferingId,
        subject: RelevanceSubject,
        target: EntityId,
        band: LikelihoodBand,
        basis: OpportunityBasis,
    ) -> Self {
        Self {
            offering_id,
            subject,
            target,
            band,
            basis,
        }
    }

    /// Which choice the reading is about.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// Whether it is a project or a career reading.
    #[must_use]
    pub const fn subject(&self) -> RelevanceSubject {
        self.subject
    }

    /// The goal or direction it names.
    #[must_use]
    pub const fn target(&self) -> EntityId {
        self.target
    }

    /// How strongly the reading supports the relevance.
    #[must_use]
    pub const fn band(&self) -> LikelihoodBand {
        self.band
    }

    /// What the reading was inferred from.
    #[must_use]
    pub const fn basis(&self) -> OpportunityBasis {
        self.basis
    }
}

/// One piece of downstream knowledge nobody officially requires.
///
/// Deliberately not `academic_curriculum::RecommendedPrerequisite`: that type
/// is section 8.2's *course*-level instructor recommendation, and section
/// 22.3's seventh bullet is about *concepts*. Reusing it would have said the
/// two are the same claim, and `GATE-38-018` is open on how a recommended
/// prerequisite differs from an official one at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformalRecommendation {
    downstream_concept: EntityId,
    recommended_concepts: Vec<EntityId>,
}

impl InformalRecommendation {
    /// Records one downstream concept and the concepts informally recommended
    /// before it.
    #[must_use]
    pub fn of(downstream_concept: EntityId, recommended_concepts: Vec<EntityId>) -> Self {
        let mut recommended_concepts = recommended_concepts;
        recommended_concepts.sort_unstable();
        recommended_concepts.dedup();
        Self {
            downstream_concept,
            recommended_concepts,
        }
    }

    /// The downstream concept.
    #[must_use]
    pub const fn downstream_concept(&self) -> EntityId {
        self.downstream_concept
    }

    /// The concepts informally recommended before it, deduplicated and ordered.
    #[must_use]
    pub fn recommended_concepts(&self) -> &[EntityId] {
        &self.recommended_concepts
    }
}

/// The concepts `P2-N6` placed on a path, with the role it gave each.
///
/// Private field and two constructors: one that takes the pairs, and
/// [`Self::from_critical_path`], which reads them off that engine's own result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCoverageTargets {
    targets: Vec<(EntityId, PathRole)>,
}

impl PathCoverageTargets {
    /// Records the pairs, ordered on the concept.
    ///
    /// # Errors
    ///
    /// [`WhatIfError::DuplicatePathTarget`] when one concept is named twice.
    /// Two roles for one concept is a disagreement inside the path engine's
    /// answer, and nothing here picks one of them.
    pub fn of(targets: Vec<(EntityId, PathRole)>) -> Result<Self, WhatIfError> {
        let mut seen = BTreeSet::new();
        for (concept, _) in &targets {
            if !seen.insert(*concept) {
                return Err(WhatIfError::DuplicatePathTarget(*concept));
            }
        }
        let mut targets = targets;
        targets.sort_by_key(|(concept, _)| *concept);
        Ok(Self { targets })
    }

    /// Reads the roles `P2-N6` assigned.
    ///
    /// # Errors
    ///
    /// [`WhatIfError::DuplicatePathTarget`] when that engine's roles name one
    /// concept twice, which this crate reports rather than resolving.
    pub fn from_critical_path(result: &CriticalPathResult) -> Result<Self, WhatIfError> {
        Self::of(result.roles().to_vec())
    }

    /// The pairs, in concept order.
    #[must_use]
    pub fn targets(&self) -> &[(EntityId, PathRole)] {
        &self.targets
    }

    /// The role this path gave one concept, when it gave it one.
    #[must_use]
    pub fn role_of(&self, concept: EntityId) -> Option<PathRole> {
        self.targets
            .iter()
            .find(|(candidate, _)| *candidate == concept)
            .map(|(_, role)| *role)
    }
}

/// One downstream course whose official prerequisites the plan may complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownstreamCourse {
    course: CourseId,
    official_prerequisites: Vec<OfficialPrerequisite>,
}

impl DownstreamCourse {
    /// Records one downstream course and its official prerequisites.
    #[must_use]
    pub fn of(course: CourseId, official_prerequisites: Vec<OfficialPrerequisite>) -> Self {
        let mut official_prerequisites = official_prerequisites;
        official_prerequisites.sort_unstable();
        official_prerequisites.dedup();
        Self {
            course,
            official_prerequisites,
        }
    }

    /// The downstream course.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// Its official prerequisites.
    #[must_use]
    pub fn official_prerequisites(&self) -> &[OfficialPrerequisite] {
        &self.official_prerequisites
    }
}

/// Every field of [`PlanChoice`], as [`plan_inputs_digest`] hashes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlanChoiceField {
    /// `offering_id`.
    OfferingId,
    /// `course`.
    Course,
    /// `seat`.
    Seat,
    /// `credits`.
    Credits,
    /// `category`.
    Category,
    /// `official_prerequisites`.
    OfficialPrerequisites,
    /// `enrolment_limit`.
    EnrolmentLimit,
    /// `assumed_weekly_hours`.
    AssumedWeeklyHours,
    /// `syllabus_concepts`.
    SyllabusConcepts,
}

/// Every field of a choice, in declaration order.
pub const PLAN_CHOICE_FIELDS: [PlanChoiceField; 9] = [
    PlanChoiceField::OfferingId,
    PlanChoiceField::Course,
    PlanChoiceField::Seat,
    PlanChoiceField::Credits,
    PlanChoiceField::Category,
    PlanChoiceField::OfficialPrerequisites,
    PlanChoiceField::EnrolmentLimit,
    PlanChoiceField::AssumedWeeklyHours,
    PlanChoiceField::SyllabusConcepts,
];

impl PlanChoiceField {
    /// The field's own name, as this file declares it.
    #[must_use]
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::OfferingId => "offering_id",
            Self::Course => "course",
            Self::Seat => "seat",
            Self::Credits => "credits",
            Self::Category => "category",
            Self::OfficialPrerequisites => "official_prerequisites",
            Self::EnrolmentLimit => "enrolment_limit",
            Self::AssumedWeeklyHours => "assumed_weekly_hours",
            Self::SyllabusConcepts => "syllabus_concepts",
        }
    }
}

/// The catalogue row behind one choice.
///
/// A grouping rather than a type with rules of its own: the three values
/// arrive together from `P2-U1`'s catalogue and travel together into a choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogueRow {
    course: CourseId,
    credits: Credits,
    category: CurriculumCategory,
}

impl CatalogueRow {
    /// Records the row.
    #[must_use]
    pub const fn of(course: CourseId, credits: Credits, category: CurriculumCategory) -> Self {
        Self {
            course,
            credits,
            category,
        }
    }
}

/// Section 22.2's third bullet, as it arrives: `공식 선수과목·수강 제한`.
///
/// One value because the design document names them in one bullet, and because
/// a choice that carried prerequisites and no restriction reading would be a
/// choice whose restriction was never asked about — which
/// [`EnrolmentLimitStanding::Unknown`] says out loud instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialConditions {
    official_prerequisites: Vec<OfficialPrerequisite>,
    enrolment_limit: EnrolmentLimitStanding,
}

impl OfficialConditions {
    /// Records the official reading of one choice's conditions.
    #[must_use]
    pub fn of(
        official_prerequisites: Vec<OfficialPrerequisite>,
        enrolment_limit: EnrolmentLimitStanding,
    ) -> Self {
        let mut official_prerequisites = official_prerequisites;
        official_prerequisites.sort_unstable();
        official_prerequisites.dedup();
        Self {
            official_prerequisites,
            enrolment_limit,
        }
    }
}

/// One offering the plan chooses, and everything the engine reads about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanChoice {
    offering_id: OfferingId,
    course: CourseId,
    seat: ConfirmedSeat,
    credits: Credits,
    category: CurriculumCategory,
    official_prerequisites: Vec<OfficialPrerequisite>,
    enrolment_limit: EnrolmentLimitStanding,
    assumed_weekly_hours: WorkloadHoursRange,
    syllabus_concepts: Vec<SyllabusConceptSignal>,
}

impl PlanChoice {
    /// Records one choice.
    #[must_use]
    pub fn of(
        offering_id: OfferingId,
        seat: ConfirmedSeat,
        catalogue: CatalogueRow,
        conditions: OfficialConditions,
        assumed_weekly_hours: WorkloadHoursRange,
        syllabus_concepts: Vec<SyllabusConceptSignal>,
    ) -> Self {
        Self {
            offering_id,
            course: catalogue.course,
            seat,
            credits: catalogue.credits,
            category: catalogue.category,
            official_prerequisites: conditions.official_prerequisites,
            enrolment_limit: conditions.enrolment_limit,
            assumed_weekly_hours,
            syllabus_concepts,
        }
    }

    /// The offering.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// The course behind it.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// The confirmed seat, which is the only official timetable there is.
    #[must_use]
    pub const fn seat(&self) -> &ConfirmedSeat {
        &self.seat
    }

    /// The catalogue credits.
    #[must_use]
    pub const fn credits(&self) -> Credits {
        self.credits
    }

    /// The curriculum category the credits are allocated to.
    #[must_use]
    pub const fn category(&self) -> CurriculumCategory {
        self.category
    }

    /// The official prerequisites.
    #[must_use]
    pub fn official_prerequisites(&self) -> &[OfficialPrerequisite] {
        &self.official_prerequisites
    }

    /// The official enrolment restriction reading.
    #[must_use]
    pub const fn enrolment_limit(&self) -> EnrolmentLimitStanding {
        self.enrolment_limit
    }

    /// The assumed weekly hours for this choice.
    #[must_use]
    pub const fn assumed_weekly_hours(&self) -> WorkloadHoursRange {
        self.assumed_weekly_hours
    }

    /// The published-material concept signals.
    #[must_use]
    pub fn syllabus_concepts(&self) -> &[SyllabusConceptSignal] {
        &self.syllabus_concepts
    }
}

/// Every field of [`PlanInputs`], as [`plan_inputs_digest`] hashes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlanInputField {
    /// `scenario_id`.
    ScenarioId,
    /// `model_run_id`.
    ModelRunId,
    /// `basis`.
    Basis,
    /// `assumptions`.
    Assumptions,
    /// `choices`.
    Choices,
    /// `completed_courses`.
    CompletedCourses,
    /// `downstream_courses`.
    DownstreamCourses,
    /// `path_targets`.
    PathTargets,
    /// `relevance`.
    Relevance,
    /// `informal_recommendations`.
    InformalRecommendations,
    /// `workload_bias`.
    WorkloadBias,
    /// `grade_assumptions`.
    GradeAssumptions,
    /// `grading_scheme`.
    GradingScheme,
}

/// Every field of the inputs, in declaration order.
pub const PLAN_INPUT_FIELDS: [PlanInputField; 13] = [
    PlanInputField::ScenarioId,
    PlanInputField::ModelRunId,
    PlanInputField::Basis,
    PlanInputField::Assumptions,
    PlanInputField::Choices,
    PlanInputField::CompletedCourses,
    PlanInputField::DownstreamCourses,
    PlanInputField::PathTargets,
    PlanInputField::Relevance,
    PlanInputField::InformalRecommendations,
    PlanInputField::WorkloadBias,
    PlanInputField::GradeAssumptions,
    PlanInputField::GradingScheme,
];

impl PlanInputField {
    /// The field's own name, as this file declares it.
    #[must_use]
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::ScenarioId => "scenario_id",
            Self::ModelRunId => "model_run_id",
            Self::Basis => "basis",
            Self::Assumptions => "assumptions",
            Self::Choices => "choices",
            Self::CompletedCourses => "completed_courses",
            Self::DownstreamCourses => "downstream_courses",
            Self::PathTargets => "path_targets",
            Self::Relevance => "relevance",
            Self::InformalRecommendations => "informal_recommendations",
            Self::WorkloadBias => "workload_bias",
            Self::GradeAssumptions => "grade_assumptions",
            Self::GradingScheme => "grading_scheme",
        }
    }
}

/// Everything one what-if run reads.
///
/// Public fields on purpose: this is the *input*, and a caller assembles it.
/// Every output type in this crate has private fields and one producer, which
/// is the asymmetry that matters — a caller may say what the plan assumes and
/// may not say what the plan concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanInputs {
    /// Identity of the plan being simulated.
    pub scenario_id: EntityId,
    /// The model run behind the projected lane.
    pub model_run_id: ModelRunId,
    /// Section 22.1's `basedOn`.
    pub basis: ScenarioBasis,
    /// Section 22.1's `assumptions`.
    pub assumptions: PlanAssumptions,
    /// Section 22.1's `choices`.
    pub choices: Vec<PlanChoice>,
    /// The courses the record snapshot the basis names shows completed.
    pub completed_courses: BTreeSet<CourseId>,
    /// The downstream courses whose official prerequisites are being watched.
    pub downstream_courses: Vec<DownstreamCourse>,
    /// The concepts `P2-N6` placed on a path.
    pub path_targets: PathCoverageTargets,
    /// Project and career relevance readings.
    pub relevance: Vec<RelevanceSignal>,
    /// Informally recommended downstream knowledge.
    pub informal_recommendations: Vec<InformalRecommendation>,
    /// `P2-U8`'s six-dimension disclosure for the workload the plan assumes.
    pub workload_bias: BiasDisclosure,
    /// The grades the user stated, when the user stated any.
    pub grade_assumptions: Option<StatedGradeAssumptions>,
    /// The versioned grade table a stated grade is read through.
    pub grading_scheme: GradingScheme,
}

/// A length-delimited field accumulator.
///
/// A consuming builder rather than a `&mut` sink. Nothing in this crate takes a
/// mutable borrow of anything — that absence is what
/// `plan_scenario_never_writes_actual_state` measures over the whole package —
/// and a digest helper is not the place to open the first one.
struct FieldHasher(Sha256);

impl FieldHasher {
    fn opened(domain: &str) -> Self {
        Self(Sha256::new()).field(domain.as_bytes())
    }

    fn field(mut self, bytes: &[u8]) -> Self {
        self.0
            .update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
        self.0.update(bytes);
        self
    }

    fn count(self, value: usize) -> Self {
        self.field(&u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes())
    }

    fn sealed(self) -> ContentDigest {
        ContentDigest::from_sha256_bytes(self.0.finalize().into())
    }
}

/// Digests the frozen inputs with length-delimited, keyed fields.
///
/// Length delimiting stops two different input sets from digesting alike by
/// moving a byte across a field boundary, and the field key binds the digest to
/// the names of the fields rather than only to their order — see
/// [`crate::basis`] for what the key is and is not evidence for.
///
/// `frozen_inputs_are_the_whole_of_what_the_engine_reads` pins the digest of
/// the acceptance suite's own plan, because every other observation there
/// recomputes it with this same code and so cannot see a change to the
/// encoding.
#[must_use]
pub fn plan_inputs_digest(inputs: &PlanInputs) -> ContentDigest {
    let mut hasher = FieldHasher::opened(INPUTS_DIGEST_DOMAIN);
    for entry in PLAN_INPUT_FIELDS {
        hasher = hasher.field(entry.field_name().as_bytes());
        hasher = match entry {
            PlanInputField::ScenarioId => hasher.field(inputs.scenario_id.as_bytes()),
            PlanInputField::ModelRunId => hasher.field(inputs.model_run_id.as_bytes()),
            PlanInputField::Basis => hasher.field(inputs.basis.digest().as_bytes()),
            PlanInputField::Assumptions => hasher
                .field(
                    &inputs
                        .assumptions
                        .workload()
                        .range()
                        .low_hours()
                        .to_be_bytes(),
                )
                .field(
                    &inputs
                        .assumptions
                        .workload()
                        .range()
                        .high_hours()
                        .to_be_bytes(),
                )
                .field(inputs.assumptions.workload().source().as_bytes())
                .field(HypotheticalCompletion::SPEC_VALUE.as_bytes())
                .field(ProbabilisticCoverage::SPEC_VALUE.as_bytes()),
            PlanInputField::Choices => {
                let mut inner = hasher.count(inputs.choices.len());
                for choice in &inputs.choices {
                    inner = digest_choice(inner, choice);
                }
                inner
            }
            PlanInputField::CompletedCourses => {
                let mut inner = hasher.count(inputs.completed_courses.len());
                for course in &inputs.completed_courses {
                    inner = inner.field(course.as_bytes());
                }
                inner
            }
            PlanInputField::DownstreamCourses => {
                let mut inner = hasher.count(inputs.downstream_courses.len());
                for downstream in &inputs.downstream_courses {
                    inner = inner
                        .field(downstream.course().as_bytes())
                        .count(downstream.official_prerequisites().len());
                    for prerequisite in downstream.official_prerequisites() {
                        inner = inner.field(prerequisite.course().as_bytes());
                    }
                }
                inner
            }
            PlanInputField::PathTargets => {
                let mut inner = hasher.count(inputs.path_targets.targets().len());
                for (concept, role) in inputs.path_targets.targets() {
                    inner = inner
                        .field(concept.as_bytes())
                        .field(role.as_str().as_bytes());
                }
                inner
            }
            PlanInputField::Relevance => {
                let mut inner = hasher.count(inputs.relevance.len());
                for signal in &inputs.relevance {
                    inner = inner
                        .field(signal.offering_id().as_bytes())
                        .field(signal.subject().as_str().as_bytes())
                        .field(signal.target().as_bytes())
                        .field(band_tag(signal.band()).as_bytes())
                        .field(basis_tag(signal.basis()).as_bytes());
                }
                inner
            }
            PlanInputField::InformalRecommendations => {
                let mut inner = hasher.count(inputs.informal_recommendations.len());
                for recommendation in &inputs.informal_recommendations {
                    inner = inner
                        .field(recommendation.downstream_concept().as_bytes())
                        .count(recommendation.recommended_concepts().len());
                    for concept in recommendation.recommended_concepts() {
                        inner = inner.field(concept.as_bytes());
                    }
                }
                inner
            }
            PlanInputField::WorkloadBias => {
                let mut inner = hasher.count(inputs.workload_bias.findings().len());
                for finding in inputs.workload_bias.findings() {
                    inner = inner
                        .field(finding.dimension().as_str().as_bytes())
                        .field(&finding.measured().to_be_bytes())
                        .field(finding.strength().as_str().as_bytes());
                }
                inner
            }
            PlanInputField::GradeAssumptions => match &inputs.grade_assumptions {
                None => hasher.field(b"none"),
                Some(stated) => {
                    let mut inner = hasher.field(b"stated").count(stated.stated().len());
                    for assumption in stated.stated() {
                        inner = inner
                            .field(assumption.offering_id().as_bytes())
                            .field(assumption.grade().as_token().as_bytes());
                    }
                    inner
                }
            },
            PlanInputField::GradingScheme => hasher
                .field(inputs.grading_scheme.id().as_bytes())
                .field(inputs.grading_scheme.canonical_text().as_bytes()),
        };
    }
    hasher.sealed()
}

fn digest_choice(hasher: FieldHasher, choice: &PlanChoice) -> FieldHasher {
    let mut hasher = hasher;
    for entry in PLAN_CHOICE_FIELDS {
        hasher = hasher.field(entry.field_name().as_bytes());
        hasher = match entry {
            PlanChoiceField::OfferingId => hasher.field(choice.offering_id().as_bytes()),
            PlanChoiceField::Course => hasher.field(choice.course().as_bytes()),
            PlanChoiceField::Seat => {
                let seat = choice.seat();
                let mut inner = hasher
                    .field(seat.course().as_str().as_bytes())
                    .field(seat.term().canonical_text().as_bytes())
                    .field(&seat.verified_at().value().to_be_bytes());
                inner = match seat.capacity() {
                    None => inner.field(b"no-capacity"),
                    Some(capacity) => inner.field(&capacity.seats().to_be_bytes()),
                };
                inner = inner.count(seat.meetings().len());
                for meeting in seat.meetings() {
                    inner = inner
                        .field(&weekday_index(meeting.weekday()).to_be_bytes())
                        .field(&meeting.from_minute().to_be_bytes())
                        .field(&meeting.to_minute().to_be_bytes());
                }
                inner
            }
            PlanChoiceField::Credits => hasher.field(&choice.credits().value().to_be_bytes()),
            PlanChoiceField::Category => hasher.field(choice.category().as_str().as_bytes()),
            PlanChoiceField::OfficialPrerequisites => {
                let mut inner = hasher.count(choice.official_prerequisites().len());
                for prerequisite in choice.official_prerequisites() {
                    inner = inner.field(prerequisite.course().as_bytes());
                }
                inner
            }
            PlanChoiceField::EnrolmentLimit => {
                hasher.field(choice.enrolment_limit().as_str().as_bytes())
            }
            PlanChoiceField::AssumedWeeklyHours => hasher
                .field(&choice.assumed_weekly_hours().low_hours().to_be_bytes())
                .field(&choice.assumed_weekly_hours().high_hours().to_be_bytes()),
            PlanChoiceField::SyllabusConcepts => {
                let mut inner = hasher.count(choice.syllabus_concepts().len());
                for signal in choice.syllabus_concepts() {
                    inner = inner
                        .field(signal.concept_entity_id.as_bytes())
                        .field(basis_tag(signal.basis).as_bytes())
                        .field(&signal.coverage_permille.to_be_bytes())
                        .field(if signal.assessed { b"true" } else { b"false" });
                }
                inner
            }
        };
    }
    hasher
}

/// A weekday's position in `P2-U1`'s own enumeration.
///
/// Derived from `Weekday::ALL` rather than from a second token table here, so
/// this crate holds no second spelling of a weekday. The position is stable
/// because that enumeration is section 8.2's and is ordered by the week.
fn weekday_index(weekday: Weekday) -> u8 {
    u8::try_from(
        Weekday::ALL
            .iter()
            .position(|candidate| *candidate == weekday)
            .unwrap_or(usize::from(u8::MAX)),
    )
    .unwrap_or(u8::MAX)
}

/// Stable wire tags for the two `P2-C7` enumerations the digest covers.
///
/// Spelled here rather than taken from `Debug`, for the reason that crate's own
/// envelope gives: a rename of a Rust variant must not silently change a digest
/// another build verifies. Both are total `match`es, so a variant added there
/// stops this crate compiling until it says what the new one hashes as.
const fn band_tag(band: LikelihoodBand) -> &'static str {
    match band {
        LikelihoodBand::Unknown => "UNKNOWN",
        LikelihoodBand::Low => "LOW",
        LikelihoodBand::Moderate => "MODERATE",
        LikelihoodBand::High => "HIGH",
    }
}

const fn basis_tag(basis: OpportunityBasis) -> &'static str {
    match basis {
        OpportunityBasis::Syllabus => "SYLLABUS",
        OpportunityBasis::AssignmentBrief => "ASSIGNMENT_BRIEF",
        OpportunityBasis::AssessmentPlan => "ASSESSMENT_PLAN",
        OpportunityBasis::HistoricalOffering => "HISTORICAL_OFFERING",
    }
}
