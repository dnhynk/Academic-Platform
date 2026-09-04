//! Section 22.3's lane: the seven things a plan may only *project*.
//!
//! # `ProjectedEvidenceOpportunity` is the only future-knowledge output
//!
//! Section 22.3 ends: *수강 완료를 mastery 증가로 projection하지 않는다. 미래
//! Knowledge State는 `ProjectedEvidenceOpportunity`로 표현한다.*
//!
//! This crate does not declare a second future-knowledge type. Sections 22.3's
//! first three bullets — exposure, practice and assessment opportunity — are
//! `P2-C7`'s [`academic_scenario::ScenarioProjection`] verbatim, produced by
//! that crate's own [`academic_scenario::project`]. The four remaining bullets
//! are about the *plan*, not about future knowledge: relevance is a reading the
//! user recorded, workload is a range with its bias, path coverage is an
//! overlap with `P2-N6`'s answer, and readiness is a count of which
//! informally recommended concepts the plan's opportunities touch. None of them
//! says what the user will know.
//!
//! The whole set of the crate's own identifiers is compared against a reviewed
//! inventory by `no_mastery_delta_in_plan_output`, with a control that requires
//! the same reader to find the mastery ladder in `P2-N2`'s own source. `P2-N3`
//! held section 1's fifth invariant by never naming `MasteryLevel` at all, and
//! that is what is held here: the absence of the vocabulary, not a rule about
//! it.
//!
//! # Workload is a range and it arrives with its bias
//!
//! Section 22.4: *workload는 강의평 표본 수, 시점, 선택 편향과 교수/학기 차이를
//! 함께 표시한다.* [`ProjectedWorkload`] takes `P2-U8`'s
//! [`BiasDisclosure`] **by value**, and that type's only producer names every
//! one of section 29.5's six dimensions. So a workload with no bias beside it
//! is not a value this crate can build, and the four facets section 22.4 lists
//! are four of the six it always carries.
//!
//! There is no point estimate anywhere: no midpoint, no expected value, no mean
//! and no single hour count. `workload_is_a_range_with_bias_metadata` compares
//! the whole method inventory of this module against a reviewed list, so a
//! collapsing accessor added later fails as an entry nobody wrote down.

use std::collections::BTreeSet;

use academic_critical_path::PathRole;
use academic_domain::{EntityId, OfferingId};
use academic_review::BiasDisclosure;
use academic_scenario::{
    LikelihoodBand, OpportunityBasis, OpportunityKind, ProjectedWorkloadRange, ScenarioProjection,
    WorkloadHoursRange,
};

use crate::{
    inputs::{RelevanceSignal, RelevanceSubject},
    lane::ProjectedItem,
};

/// One recorded relevance reading, carried into the plan's projections.
///
/// The simulator does not decide relevance; section 22.3 lists it as a
/// projection and the reading arrives frozen. What this type adds is that the
/// reading is *scoped to the plan*: [`RelevanceProjection::of`] refuses a
/// reading about an offering the plan does not choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelevanceEntry {
    offering_id: OfferingId,
    subject: RelevanceSubject,
    target: EntityId,
    band: LikelihoodBand,
    basis: OpportunityBasis,
}

impl RelevanceEntry {
    /// Which choice.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// Project or career.
    #[must_use]
    pub const fn subject(&self) -> RelevanceSubject {
        self.subject
    }

    /// The goal or direction.
    #[must_use]
    pub const fn target(&self) -> EntityId {
        self.target
    }

    /// The band.
    #[must_use]
    pub const fn band(&self) -> LikelihoodBand {
        self.band
    }

    /// What the reading rests on.
    #[must_use]
    pub const fn basis(&self) -> OpportunityBasis {
        self.basis
    }
}

/// Section 22.3's fourth bullet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelevanceProjection {
    entries: Vec<RelevanceEntry>,
}

impl RelevanceProjection {
    /// Carries the readings, in a stable order.
    pub(crate) fn of(signals: &[RelevanceSignal]) -> Self {
        let mut entries: Vec<RelevanceEntry> = signals
            .iter()
            .map(|signal| RelevanceEntry {
                offering_id: signal.offering_id(),
                subject: signal.subject(),
                target: signal.target(),
                band: signal.band(),
                basis: signal.basis(),
            })
            .collect();
        entries.sort_by_key(|entry| {
            (
                *entry.offering_id.as_bytes(),
                entry.subject,
                *entry.target.as_bytes(),
            )
        });
        Self { entries }
    }

    /// Every reading.
    #[must_use]
    pub fn entries(&self) -> &[RelevanceEntry] {
        &self.entries
    }
}

/// Section 22.3's fifth bullet: a range, its bias, and the sealed proposal.
///
/// The sealed [`ProjectedWorkloadRange`] is `P2-C7`'s proposal — it carries the
/// model run, the frozen-input digest and the engine version, and it has no
/// accessor that returns the range it proposes. The `band` beside it is the
/// same range in the clear, because section 22.4 renders `34–46 h/week` on the
/// screen. That is not a hole in the seal: the seal exists so that a *proposal*
/// cannot be lifted into a canonical write, and a rendered band is neither. The
/// two are built from one value by one crate-private constructor, so they
/// cannot disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedWorkload {
    proposed: ProjectedWorkloadRange,
    band: WorkloadHoursRange,
    bias: BiasDisclosure,
}

impl ProjectedWorkload {
    /// Seals the plan's summed range and binds it to its bias.
    pub(crate) const fn of(
        proposed: ProjectedWorkloadRange,
        band: WorkloadHoursRange,
        bias: BiasDisclosure,
    ) -> Self {
        Self {
            proposed,
            band,
            bias,
        }
    }

    /// The sealed proposal, with its provenance.
    #[must_use]
    pub const fn proposed(&self) -> &ProjectedWorkloadRange {
        &self.proposed
    }

    /// The weekly range section 22.4 renders.
    ///
    /// A range and nothing else. There is deliberately no accessor here that
    /// returns one number: see the module note.
    #[must_use]
    pub const fn band(&self) -> WorkloadHoursRange {
        self.band
    }

    /// The six disclosures section 29.5 requires of any review aggregate.
    #[must_use]
    pub const fn bias(&self) -> &BiasDisclosure {
        &self.bias
    }
}

/// One place the plan's published material meets `P2-N6`'s path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathCoverageEntry {
    concept: EntityId,
    role: PathRole,
    offering_id: OfferingId,
    kind: OpportunityKind,
    likelihood: LikelihoodBand,
}

impl PathCoverageEntry {
    pub(crate) const fn of(
        concept: EntityId,
        role: PathRole,
        offering_id: OfferingId,
        kind: OpportunityKind,
        likelihood: LikelihoodBand,
    ) -> Self {
        Self {
            concept,
            role,
            offering_id,
            kind,
            likelihood,
        }
    }

    /// The concept on the path.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The role that engine gave it.
    #[must_use]
    pub const fn role(&self) -> PathRole {
        self.role
    }

    /// The choice whose published material mentions it.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// Which opportunity kind the coverage is.
    #[must_use]
    pub const fn kind(&self) -> OpportunityKind {
        self.kind
    }

    /// How likely the opportunity is.
    #[must_use]
    pub const fn likelihood(&self) -> LikelihoodBand {
        self.likelihood
    }
}

/// Section 22.3's sixth bullet.
///
/// An *overlap*, not a recomputation. Every concept named here was placed on a
/// path by `P2-N6` and mentioned by a choice's published material; this crate
/// decides neither of those facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCoverage {
    entries: Vec<PathCoverageEntry>,
}

impl PathCoverage {
    pub(crate) fn of(entries: Vec<PathCoverageEntry>) -> Self {
        let mut entries = entries;
        entries.sort_unstable();
        Self { entries }
    }

    /// Every overlap, in a stable order.
    #[must_use]
    pub fn entries(&self) -> &[PathCoverageEntry] {
        &self.entries
    }

    /// Every overlap for one path role.
    #[must_use]
    pub fn for_role(&self, role: PathRole) -> Vec<PathCoverageEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.role() == role)
            .copied()
            .collect()
    }
}

/// One downstream concept, and how much of what is informally recommended
/// before it the plan would give an occasion to meet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformalReadinessEntry {
    downstream_concept: EntityId,
    covered: Vec<EntityId>,
    uncovered: Vec<EntityId>,
    band: LikelihoodBand,
}

impl InformalReadinessEntry {
    /// Splits one recommendation against the concepts the plan touches.
    ///
    /// The band is a coverage reading and not a fraction rendered as one: all
    /// covered is `High`, at least half is `Moderate`, at least one is `Low`,
    /// and none is `Unknown` — which is also what an empty recommendation
    /// reads as, because there is nothing to be ready from.
    pub(crate) fn of(
        downstream_concept: EntityId,
        recommended: &[EntityId],
        touched: &BTreeSet<EntityId>,
    ) -> Self {
        let (covered, uncovered): (Vec<EntityId>, Vec<EntityId>) = recommended
            .iter()
            .copied()
            .partition(|concept| touched.contains(concept));
        let band = if covered.is_empty() {
            LikelihoodBand::Unknown
        } else if uncovered.is_empty() {
            LikelihoodBand::High
        } else if covered.len() * 2 >= recommended.len() {
            LikelihoodBand::Moderate
        } else {
            LikelihoodBand::Low
        };
        Self {
            downstream_concept,
            covered,
            uncovered,
            band,
        }
    }

    /// The downstream concept.
    #[must_use]
    pub const fn downstream_concept(&self) -> EntityId {
        self.downstream_concept
    }

    /// The recommended concepts the plan gives an occasion to meet.
    #[must_use]
    pub fn covered(&self) -> &[EntityId] {
        &self.covered
    }

    /// The recommended concepts it does not.
    #[must_use]
    pub fn uncovered(&self) -> &[EntityId] {
        &self.uncovered
    }

    /// How ready the plan would leave the user for the downstream concept.
    ///
    /// A band rather than a fraction, and `P2-C7`'s band rather than a second
    /// one. Readiness is informal by section 22.3's own words — *비공식 권장
    /// 지식* — and a percentage would read as a measurement of something
    /// nobody measured.
    #[must_use]
    pub const fn band(&self) -> LikelihoodBand {
        self.band
    }
}

/// Section 22.3's seventh bullet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformalReadiness {
    entries: Vec<InformalReadinessEntry>,
}

impl InformalReadiness {
    pub(crate) fn of(entries: Vec<InformalReadinessEntry>) -> Self {
        let mut entries = entries;
        entries.sort_by_key(|entry| *entry.downstream_concept.as_bytes());
        Self { entries }
    }

    /// Every downstream concept, in a stable order.
    #[must_use]
    pub fn entries(&self) -> &[InformalReadinessEntry] {
        &self.entries
    }
}

/// Section 22.1's `projections`.
///
/// Note what the type does not hold: no mastery level, no freshness band, no
/// claim, no grade, no credit total and no allocation. That is the data-type
/// half of section 22.1's split from the projected side, and it is the same
/// absence [`crate::deterministic::DeterministicResults`] carries from the
/// deterministic side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedResults {
    opportunities: ScenarioProjection,
    relevance: RelevanceProjection,
    workload: ProjectedWorkload,
    path_coverage: PathCoverage,
    readiness: InformalReadiness,
}

impl ProjectedResults {
    pub(crate) const fn of(
        opportunities: ScenarioProjection,
        relevance: RelevanceProjection,
        workload: ProjectedWorkload,
        path_coverage: PathCoverage,
        readiness: InformalReadiness,
    ) -> Self {
        Self {
            opportunities,
            relevance,
            workload,
            path_coverage,
            readiness,
        }
    }

    /// `P2-C7`'s projection: section 22.3's first three bullets, unmodified.
    #[must_use]
    pub const fn opportunities(&self) -> &ScenarioProjection {
        &self.opportunities
    }

    /// Section 22.3's fourth bullet.
    #[must_use]
    pub const fn relevance(&self) -> &RelevanceProjection {
        &self.relevance
    }

    /// Section 22.3's fifth bullet.
    #[must_use]
    pub const fn workload(&self) -> &ProjectedWorkload {
        &self.workload
    }

    /// Section 22.3's sixth bullet.
    #[must_use]
    pub const fn path_coverage(&self) -> &PathCoverage {
        &self.path_coverage
    }

    /// Section 22.3's seventh bullet.
    #[must_use]
    pub const fn readiness(&self) -> &InformalReadiness {
        &self.readiness
    }

    /// Which of section 22.3's bullets this result carries a value for.
    ///
    /// Derived from the values rather than returned as a constant. An
    /// assessment opportunity, for instance, appears only where a choice's
    /// published material states that a concept is assessed — inferring one
    /// from coverage is the overprediction of section 22.5, one step removed —
    /// so a plan over material that assesses nothing reports six bullets and
    /// not seven.
    #[must_use]
    pub fn produced(&self) -> Vec<ProjectedItem> {
        let mut produced = Vec::new();
        for (kind, item) in [
            (
                OpportunityKind::Exposure,
                ProjectedItem::SyllabusExposureOpportunity,
            ),
            (
                OpportunityKind::Practice,
                ProjectedItem::AssignmentPracticeOpportunity,
            ),
            (
                OpportunityKind::Assessment,
                ProjectedItem::AssessmentOpportunity,
            ),
        ] {
            if self
                .opportunities
                .opportunities
                .iter()
                .any(|opportunity| opportunity.kind == kind)
            {
                produced.push(item);
            }
        }
        if !self.relevance.entries().is_empty() {
            produced.push(ProjectedItem::ProjectCareerRelevance);
        }
        produced.push(ProjectedItem::WorkloadRangeAndReviewBias);
        if !self.path_coverage.entries().is_empty() {
            produced.push(ProjectedItem::CriticalPathCoverage);
        }
        if !self.readiness.entries().is_empty() {
            produced.push(ProjectedItem::InformalDownstreamReadiness);
        }
        produced.sort_unstable();
        produced
    }
}
