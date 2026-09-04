//! Fixtures for the `P2-N6` acceptance suite.
//!
//! The scenario is section 36.4's chain under section 16.1's own shape. The
//! `GapCase` this engine plans around is produced by driving `P2-N5`'s real
//! `search` over `P2-N5`'s own fixtures — included here by `#[path]` rather
//! than restated, which is how `academic-gap` reaches `P2-N2`'s and
//! `academic-freshness` reaches the same module. So the evidence behind a plan
//! is a node of a `P2-L4` document that a real `P2-L2` capture and a real
//! `P2-L3` run produced, and the bands are `P2-N3`'s own `project` output.
//!
//! The hypergraph on top of it is section 16.1's:
//!
//! ```text
//! Buffer Pool
//!   REQUIRES ALL [Disk Page, Random I/O]
//!   Disk Page REQUIRES ONE OF
//!     ├─ [Storage Hierarchy]
//!     └─ [Fan Out, Page Layout]
//! ```
//!
//! Two mandatory members and one selectable branch, which is `REQ-16-001`'s own
//! acceptance shape. The two branches differ in size on purpose: the branch
//! with **fewer** nodes is the one a node-count answer would take, and several
//! tests turn that into the wrong answer.
//!
//! Nothing here reads a clock or opens a socket. Every instant is an offset
//! from `P2-N5`'s `ORIGIN`, every identifier is a SHA-256 of its own name with
//! the UUIDv7 nibbles set, and the one directory opened is the `tempfile` the
//! lecture fixture writes its capture journal into.

// Three targets include this module by `#[path]` -- the acceptance suite, the
// harness suite and the corpus example -- and each uses a different subset of
// what it re-exports, so an unused item here is a property of the caller rather
// than of this file.
#![allow(
    dead_code,
    unused_imports,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]

#[path = "../../../gap/tests/common/mod.rs"]
pub mod gap;

use std::error::Error;

use academic_critical_path::{
    AcquisitionOption, BENEFIT_COMPONENTS, BasisFamily, BenefitComponent, BenefitVector,
    COST_COMPONENTS, ConceptEstimate, ConstraintInputs, CostBasis, CostComponent, CostEstimate,
    CostVector, EdgeMember, EdgeStanding, Hyperedge, OfficialPrerequisiteStanding, Opportunity,
    OpportunityKind, PlanRequest, PreferenceSlider, PrerequisiteHypergraph, Unit, VectorAxis,
    all_axes,
};
use academic_curriculum::{Credits, OfferingStatus};
use academic_domain::{ContentDigest, EntityId, EvidenceId, FreshnessBand, OfferingId};
use academic_gap::{GapCase, PrerequisiteEdge, search};

pub use gap::{
    TestResult, buffer_pool, disk_page, entity, evidence_id, exposure_evidence, fan_out,
    full_dossier, offered, random_io, reading, section_36_4_graph, storage_hierarchy,
    understand_buffer_pool, unknown_band, uuid_of,
};

/// The fifth concept section 36.4 names: the `page-layout experiment`'s own
/// subject, and the second member of the larger branch.
#[must_use]
pub fn page_layout() -> EntityId {
    entity("concept-page-layout")
}

/// The offering section 36.7's `현재 Database Offering` stands for.
#[must_use]
pub fn database_offering() -> OfferingId {
    OfferingId::try_from_uuid(uuid_of("offering-database-2025-fall"))
        .unwrap_or_else(|error| unreachable!("{error}"))
}

/// A second offering, so an exclusion can name one without emptying the plan.
#[must_use]
pub fn storage_offering() -> OfferingId {
    OfferingId::try_from_uuid(uuid_of("offering-storage-2025-fall"))
        .unwrap_or_else(|error| unreachable!("{error}"))
}

/// The rule set every fixture run is pinned to.
#[must_use]
pub fn rule_set() -> ContentDigest {
    ContentDigest::sha256(b"P2-N6 fixture rule set")
}

/// A different rule set, for the half of the determinism contract that requires
/// different bytes under a different hash.
#[must_use]
pub fn other_rule_set() -> ContentDigest {
    ContentDigest::sha256(b"P2-N6 fixture rule set, revised")
}

// ---------------------------------------------------------------------------
// Vectors.
// ---------------------------------------------------------------------------

/// A measured interval on one cost axis, in that axis's own unit.
pub fn measured(axis: CostComponent, low: u32, high: u32) -> Result<CostEstimate, Box<dyn Error>> {
    Ok(CostEstimate::of(
        low,
        high,
        axis.unit(),
        CostBasis::measured(&[
            BasisFamily::StateAndFreshness,
            BasisFamily::ConceptGranularity,
            BasisFamily::AvailableResource,
            BasisFamily::PastLearningSpeed,
        ])?,
    )?)
}

/// An interval on one cost axis that read none of section 16.2's four
/// families. Refused unless it is genuinely wide.
pub fn unmeasured(
    axis: CostComponent,
    low: u32,
    high: u32,
) -> Result<CostEstimate, Box<dyn Error>> {
    Ok(CostEstimate::of(
        low,
        high,
        axis.unit(),
        CostBasis::Unmeasured,
    )?)
}

/// A measured interval on one benefit axis.
pub fn benefit(
    axis: BenefitComponent,
    low: u32,
    high: u32,
) -> Result<CostEstimate, Box<dyn Error>> {
    Ok(CostEstimate::of(
        low,
        high,
        axis.unit(),
        CostBasis::measured(&[BasisFamily::StateAndFreshness])?,
    )?)
}

/// A cost vector whose every axis is the same measured point, scaled by the
/// axis's own unit so the units line up.
pub fn flat_cost(magnitude: u32) -> Result<CostVector, Box<dyn Error>> {
    let mut estimates = Vec::with_capacity(COST_COMPONENTS.len());
    for axis in COST_COMPONENTS {
        estimates.push(measured(axis, magnitude, magnitude)?);
    }
    let array: [CostEstimate; 7] = estimates
        .try_into()
        .map_err(|_| "the cost vector is not seven axes")?;
    Ok(CostVector::of(array)?)
}

/// A benefit vector whose every axis is the same measured point.
pub fn flat_benefit(magnitude: u32) -> Result<BenefitVector, Box<dyn Error>> {
    let mut estimates = Vec::with_capacity(BENEFIT_COMPONENTS.len());
    for axis in BENEFIT_COMPONENTS {
        estimates.push(benefit(axis, magnitude, magnitude)?);
    }
    let array: [CostEstimate; 5] = estimates
        .try_into()
        .map_err(|_| "the benefit vector is not five axes")?;
    Ok(BenefitVector::of(array)?)
}

/// A cost vector that is flat except on one named axis.
pub fn cost_except(
    magnitude: u32,
    axis: CostComponent,
    estimate: CostEstimate,
) -> Result<CostVector, Box<dyn Error>> {
    let mut estimates = Vec::with_capacity(COST_COMPONENTS.len());
    for component in COST_COMPONENTS {
        if component == axis {
            estimates.push(estimate.clone());
        } else {
            estimates.push(measured(component, magnitude, magnitude)?);
        }
    }
    let array: [CostEstimate; 7] = estimates
        .try_into()
        .map_err(|_| "the cost vector is not seven axes")?;
    Ok(CostVector::of(array)?)
}

/// A benefit vector that is flat except on one named axis.
pub fn benefit_except(
    magnitude: u32,
    axis: BenefitComponent,
    estimate: CostEstimate,
) -> Result<BenefitVector, Box<dyn Error>> {
    let mut estimates = Vec::with_capacity(BENEFIT_COMPONENTS.len());
    for component in BENEFIT_COMPONENTS {
        if component == axis {
            estimates.push(estimate.clone());
        } else {
            estimates.push(benefit(component, magnitude, magnitude)?);
        }
    }
    let array: [CostEstimate; 5] = estimates
        .try_into()
        .map_err(|_| "the benefit vector is not five axes")?;
    Ok(BenefitVector::of(array)?)
}

// ---------------------------------------------------------------------------
// Options.
// ---------------------------------------------------------------------------

/// One occasion toward `concept`.
#[must_use]
pub fn occasion(concept: EntityId, kind: OpportunityKind, tag: &str) -> Opportunity {
    Opportunity::of(concept, kind, evidence_id(tag))
}

/// Section 36.7's `현재 Database Offering`: a course that bundles an exposure
/// and a practice occasion toward `concept`.
pub fn course_for(
    concept: EntityId,
    offering: OfferingId,
    status: OfferingStatus,
    credits: u8,
    tag: &str,
) -> Result<AcquisitionOption, Box<dyn Error>> {
    Ok(AcquisitionOption::course(
        offering,
        status,
        Credits::new(credits)?,
        vec![
            occasion(
                concept,
                OpportunityKind::Exposure,
                &format!("{tag}-lecture"),
            ),
            occasion(
                concept,
                OpportunityKind::Practice,
                &format!("{tag}-problem"),
            ),
        ],
    )?)
}

/// Section 36.7's `external reading`.
pub fn reading_for(concept: EntityId, tag: &str) -> Result<AcquisitionOption, Box<dyn Error>> {
    Ok(AcquisitionOption::self_study(vec![occasion(
        concept,
        OpportunityKind::Exposure,
        &format!("{tag}-chapter"),
    )])?)
}

/// Section 36.4's `page-layout experiment`.
pub fn experiment_for(concept: EntityId, tag: &str) -> Result<AcquisitionOption, Box<dyn Error>> {
    Ok(AcquisitionOption::project_work(
        entity("project-a"),
        vec![occasion(
            concept,
            OpportunityKind::Practice,
            &format!("{tag}-experiment"),
        )],
    )?)
}

// ---------------------------------------------------------------------------
// The hypergraph.
// ---------------------------------------------------------------------------

/// One `REQUIRES` member at `HARD`, with its standing.
pub fn member(
    dependent: EntityId,
    prerequisite: EntityId,
    standing: EdgeStanding,
    tag: &str,
) -> Result<EdgeMember, Box<dyn Error>> {
    Ok(EdgeMember::of(
        PrerequisiteEdge::admit(
            academic_domain::predicates::PredicateName::Requires,
            academic_domain::predicates::PrerequisiteStrength::Hard,
            dependent,
            prerequisite,
            vec![evidence_id(tag)],
        )?,
        standing,
    ))
}

/// Section 16.1's shape over section 36.4's concepts. See the module note.
///
/// `standings` names which members are [`EdgeStanding::Uncertain`]; everything
/// else is settled, so a fixture that wants a checkpoint asks for one by naming
/// edges rather than by setting a ratio.
pub fn section_16_1_graph(
    uncertain: &[(EntityId, EntityId)],
) -> Result<PrerequisiteHypergraph, Box<dyn Error>> {
    let standing = |dependent: EntityId, prerequisite: EntityId| {
        if uncertain.contains(&(dependent, prerequisite)) {
            EdgeStanding::Uncertain
        } else {
            EdgeStanding::Settled
        }
    };
    Ok(PrerequisiteHypergraph::new()
        .with(Hyperedge::requires_all(
            buffer_pool(),
            vec![
                member(
                    buffer_pool(),
                    disk_page(),
                    standing(buffer_pool(), disk_page()),
                    "edge-buffer-pool-disk-page",
                )?,
                member(
                    buffer_pool(),
                    random_io(),
                    standing(buffer_pool(), random_io()),
                    "edge-buffer-pool-random-io",
                )?,
            ],
        )?)
        .with(Hyperedge::requires_one_of(
            disk_page(),
            vec![
                vec![member(
                    disk_page(),
                    storage_hierarchy(),
                    standing(disk_page(), storage_hierarchy()),
                    "edge-disk-page-storage-hierarchy",
                )?],
                vec![
                    member(
                        disk_page(),
                        fan_out(),
                        standing(disk_page(), fan_out()),
                        "edge-disk-page-fan-out",
                    )?,
                    member(
                        disk_page(),
                        page_layout(),
                        standing(disk_page(), page_layout()),
                        "edge-disk-page-page-layout",
                    )?,
                ],
            ],
        )?))
}

/// Every concept the fixture hypergraph can reach.
#[must_use]
pub fn all_concepts() -> Vec<EntityId> {
    vec![
        buffer_pool(),
        disk_page(),
        random_io(),
        storage_hierarchy(),
        fan_out(),
        page_layout(),
    ]
}

// ---------------------------------------------------------------------------
// The `P2-N5` input.
// ---------------------------------------------------------------------------

/// Section 36.4's own gap, produced by driving `P2-N5`'s real `search`.
///
/// Not fabricated: the goal is `P2-N5`'s `ActiveGoal`, the graph is its
/// `PrerequisiteGraph`, the evidence is a node of a `P2-L4` document and the
/// bands come from `P2-N3`'s `project`.
pub fn section_36_4_gap() -> Result<GapCase, Box<dyn Error>> {
    let goal = understand_buffer_pool()?;
    let graph = section_36_4_graph()?;
    let mut readings = Vec::new();
    for (concept, tag) in [
        (buffer_pool(), "bp-lecture"),
        (disk_page(), "dp-lecture"),
        (storage_hierarchy(), "sh-lecture"),
    ] {
        let mut one = reading(concept, unknown_band(concept)?);
        one.offered = vec![offered(exposure_evidence(tag)?, tag, full_dossier(concept))];
        readings.push(one);
    }
    search(&goal, &graph, &readings, None)?.ok_or_else(|| "section 36.4 produced no gap".into())
}

// ---------------------------------------------------------------------------
// Estimates and requests.
// ---------------------------------------------------------------------------

/// One estimate per concept, all flat, so a test that wants to observe one axis
/// changes one axis.
pub fn flat_estimates() -> Result<Vec<ConceptEstimate>, Box<dyn Error>> {
    let mut found = Vec::new();
    for concept in all_concepts() {
        found.push(ConceptEstimate {
            concept,
            cost: flat_cost(10)?,
            benefit: flat_benefit(10)?,
            options: vec![reading_for(concept, "flat")?],
        });
    }
    Ok(found)
}

/// Replaces one concept's estimate.
pub fn with_estimate(
    mut estimates: Vec<ConceptEstimate>,
    replacement: ConceptEstimate,
) -> Vec<ConceptEstimate> {
    estimates.retain(|held| held.concept != replacement.concept);
    estimates.push(replacement);
    estimates
}

/// Constraint inputs under which every fixture route is feasible.
#[must_use]
pub fn permissive_constraints() -> ConstraintInputs {
    ConstraintInputs {
        hard_prerequisites_met: all_concepts(),
        official_prerequisites: vec![
            (database_offering(), OfficialPrerequisiteStanding::Met),
            (storage_offering(), OfficialPrerequisiteStanding::Met),
        ],
        committed_meetings: Vec::new(),
        offering_meetings: Vec::new(),
        committed_credits: 0,
        credit_limit: 21,
        horizon_days: 3650,
        privacy_excluded_sources: Vec::new(),
        user_excluded_concepts: Vec::new(),
        user_excluded_offerings: Vec::new(),
        bands: all_concepts()
            .into_iter()
            .map(|concept| (concept, FreshnessBand::High))
            .collect(),
    }
}

/// A slider in section 16.2's own axis order: the seven cost axes, then the
/// five benefit axes.
pub fn spec_order_slider() -> Result<PreferenceSlider, Box<dyn Error>> {
    Ok(PreferenceSlider::of(all_axes())?)
}

/// A slider that reverses section 16.2's order.
pub fn reversed_slider() -> Result<PreferenceSlider, Box<dyn Error>> {
    let mut order = all_axes();
    order.reverse();
    Ok(PreferenceSlider::of(order)?)
}

/// A slider that puts one axis first and keeps the rest in section 16.2's
/// order.
pub fn slider_led_by(axis: VectorAxis) -> Result<PreferenceSlider, Box<dyn Error>> {
    let mut order = vec![axis];
    order.extend(all_axes().into_iter().filter(|held| *held != axis));
    Ok(PreferenceSlider::of(order)?)
}

/// Everything one run needs, so a test changes one field.
pub struct Scenario {
    pub gap_case: GapCase,
    pub graph: PrerequisiteHypergraph,
    pub estimates: Vec<ConceptEstimate>,
    pub constraints: ConstraintInputs,
    pub slider: PreferenceSlider,
}

impl Scenario {
    /// The default scenario: settled edges, flat estimates, nothing excluded.
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            gap_case: section_36_4_gap()?,
            graph: section_16_1_graph(&[])?,
            estimates: flat_estimates()?,
            constraints: permissive_constraints(),
            slider: spec_order_slider()?,
        })
    }

    /// The request this scenario makes.
    #[must_use]
    pub fn request(&self) -> PlanRequest<'_> {
        PlanRequest {
            gap_case: &self.gap_case,
            graph: &self.graph,
            estimates: &self.estimates,
            constraints: &self.constraints,
            slider: &self.slider,
            rule_set_hash: rule_set(),
            engine_version: 1,
        }
    }
}

/// Every evidence identity the fixture options cite, in identifier order.
#[must_use]
pub fn fixture_sources() -> Vec<EvidenceId> {
    let mut found = vec![evidence_id("flat-chapter")];
    found.sort_by_key(|id| id.as_uuid());
    found
}

/// One unit, so a test can name a mismatch without importing the enumeration.
#[must_use]
pub const fn minutes() -> Unit {
    Unit::Minutes
}
