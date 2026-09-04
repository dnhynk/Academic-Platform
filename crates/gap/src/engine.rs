//! Section 15.2's six steps, run in order.
//!
//! ## The descent
//!
//! Breadth-first from the surface concept, across [`PrerequisiteEdge::blocks`]
//! edges only, visiting each concept once. Breadth-first is what makes step 4's
//! `최초의` well defined without a rule: the first time a concept is reached is
//! at its shallowest depth from the surface, so `최초의 강한 부족` is the
//! shallowest [`crate::kind::GapKind::is_strong_deficit`] node and needs no
//! tie-break — and when two sit at that depth,
//! [`crate::case::GapCase::roots`] returns both.
//!
//! ## Two places the descent refuses to go on
//!
//! *An unsettled identity.* A node whose `P2-C3` standing is not `Settled` is
//! routed to `ONTOLOGY_GAP` and **not descended through**. Its outgoing edges
//! were asserted about an identity that a split may have divided, so crossing
//! one would attribute a deeper concept's deficit to this goal through an edge
//! whose subject is ambiguous. `EquivalenceClass::SplitAmbiguous` is `P2-C3`'s
//! own `state cannot be attributed to any one successor`, and this is that
//! sentence applied to the edge as well as to the state. An unsettled *surface*
//! concept is refused outright: there is no sound reading of the goal at all.
//!
//! *A band that rests on a concept this search is blaming.* See
//! [`crate::state`]'s note. When a node's freshness was raised by a spillover
//! from a neighbour that lies on its own blocking path, the search refuses with
//! [`GapError::FreshnessRestsOnPathSpillover`] naming the neighbour and the
//! edge. It does not silently lower the band, and it does not silently keep it.
//!
//! ## What this engine does not do
//!
//! It computes no alternative *route*. Section 15.3 requires the `대체 경로`
//! field and this engine fills it with a closed graph reason, or with the routes
//! a caller supplies; deciding which of several routes satisfies a goal is an
//! AND/OR hypergraph question and `P2-N6` owns it
//! (`and_or_hypergraph_is_satisfied_not_shortest`). Nothing here treats sibling
//! prerequisites as alternatives, because siblings are conjunctive.
//!
//! It also proposes no remediation content. Which lecture segment, which
//! exercise and how many minutes are facts the system holds about a concept, not
//! judgements this engine makes, so they arrive on the [`ConceptReading`] and
//! this engine only checks that they are bounded, cited and shaped like the
//! response section 15.2's table gives the routed kind.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use academic_domain::{
    EntityId, EvidenceId, entity_registry::EntityKind, predicates::PrerequisiteStrength,
    question::QuestionStatus,
};
use academic_freshness::{FreshnessProjection, Spillover};

use crate::{
    GapError,
    case::{GapCase, RootCandidate, TieDiagnostic, roots_of},
    explanation::{
        AlternativePath, ExplanationParts, GapExplanation, LinkedContext, MinimumRemediation,
        NoAlternativeReason, RemediationActivity,
    },
    goal::ActiveGoal,
    graph::{PrerequisiteEdge, PrerequisiteGraph},
    path::{AncestorImpact, BlockingPath, PathStep},
    routing::{BranchStanding, route},
    state::{ConceptState, OfferedEvidence, StateSnapshot},
};

/// Everything the caller holds about one concept the descent may reach.
///
/// The four state dimensions arrive as the inputs `P2-N2` and `P2-N3` produce,
/// never as a ready-made verdict; the remediation arrives as content, never as
/// advice. See the module note.
#[derive(Debug, Clone)]
pub struct ConceptReading {
    /// Which concept.
    pub concept: EntityId,
    /// Its `P2-C3` tier.
    pub kind: EntityKind,
    /// Its `P2-C3` identity standing.
    pub identity: crate::node::IdentityStanding,
    /// Everything offered as evidence about it, before section 13.4's checks.
    pub offered: Vec<OfferedEvidence>,
    /// `P2-N3`'s projection for it.
    pub freshness: FreshnessProjection,
    /// The contributions that projection was built from.
    pub spillover: Vec<Spillover>,
    /// Section 15.1's `reason`: why this concept blocks the one above it.
    pub reason: String,
    /// How long the minimum remediation takes. Zero is refused.
    pub remediation_minutes: u16,
    /// What the minimum remediation is, in the user's own reading.
    pub remediation_description: String,
    /// What to read, run or answer. Empty is refused.
    pub remediation_sources: Vec<EvidenceId>,
    /// Section 15.3's `연결된 강의/프로젝트`.
    pub linked: LinkedContext,
    /// Routes around this concept, when the caller knows of any. `P2-N6` owns
    /// deciding between them; supplying them here only fills section 15.3's
    /// seventh field with something better than a reason code.
    pub alternative_routes: Vec<Vec<EntityId>>,
}

/// The diagnostic a caller offers for a tie, before the descent knows there is
/// one.
///
/// Section 15.2 step 5 says to *propose* a short diagnostic activity. What that
/// activity is, is content; that it is required, is this engine's rule. A search
/// that finds tied roots and was given no diagnostic fails with
/// [`GapError::TiedRootsNeedADiagnostic`] rather than choosing a root.
#[derive(Debug, Clone)]
pub struct DiagnosticOffer {
    /// How long it takes. Zero is refused.
    pub minutes: u16,
    /// What it is.
    pub description: String,
    /// What to read, run or answer.
    pub sources: Vec<EvidenceId>,
    /// A `P2-N4` question the diagnostic answers, with its current status. It is
    /// referenced and never resolved.
    pub question: Option<(EntityId, QuestionStatus)>,
}

/// Section 15.2 step 2: the subgraph reachable from the goal's surface concept
/// across blocking edges, breadth-first.
///
/// The first argument is an [`ActiveGoal`], which cannot exist without success
/// criteria, so step 1 has necessarily happened. There is no other entry point.
#[must_use]
pub fn expand(goal: &ActiveGoal, graph: &PrerequisiteGraph) -> Vec<BlockingPath> {
    let mut found = Vec::new();
    let mut seen: BTreeSet<[u8; 16]> = BTreeSet::new();
    seen.insert(*goal.surface_concept().as_bytes());
    let mut queue: VecDeque<BlockingPath> =
        VecDeque::from([BlockingPath::from_surface(goal.surface_concept())]);
    while let Some(path) = queue.pop_front() {
        for edge in graph.blocking_out_of(path.tip()) {
            if !seen.insert(*edge.prerequisite().as_bytes()) {
                continue;
            }
            let next = path.extended(edge);
            queue.push_back(next.clone());
            found.push(next);
        }
    }
    found
}

/// Section 15.2's six steps, and section 15.1's `GapCase`.
///
/// Returns `Ok(None)` when the descent found no gap at all. That is the answer
/// `low_mastery_without_goal_is_not_a_gap` protects from the other side: a low
/// state that no active goal's prerequisite path reaches produces nothing at
/// all, and there is no other function in this crate that produces a
/// [`GapCase`].
///
/// # Errors
///
/// [`GapError::SurfaceIdentityUnsettled`] when the goal's own surface concept
/// has an unsettled `P2-C3` identity; [`GapError::NoReadingForConcept`] when the
/// descent reaches a concept the caller supplied no reading for — this engine
/// never guesses a state; [`GapError::FreshnessRestsOnPathSpillover`] for the
/// contaminated band described in the module note;
/// [`GapError::TiedRootsNeedADiagnostic`] when several roots tie and no
/// diagnostic was offered; and every error [`ConceptState::overlay`] and
/// [`GapExplanation::of`] raise.
pub fn search(
    goal: &ActiveGoal,
    graph: &PrerequisiteGraph,
    readings: &[ConceptReading],
    diagnostic: Option<&DiagnosticOffer>,
) -> Result<Option<GapCase>, GapError> {
    let (candidates, snapshots) = descend(goal, graph, readings)?;
    if candidates.is_empty() {
        return Ok(None);
    }
    let roots = roots_of(&candidates);
    let attached = if roots.len() > 1 {
        let offer = diagnostic.ok_or(GapError::TiedRootsNeedADiagnostic)?;
        let mut tied: Vec<EntityId> = roots.iter().map(|root| root.concept()).collect();
        tied.sort_by_key(|id| id.as_uuid());
        Some(TieDiagnostic::of(
            tied,
            MinimumRemediation::of(
                offer.minutes,
                RemediationActivity::UserConfirmationOrDiagnostic,
                &offer.description,
                offer.sources.clone(),
            ),
            offer.question,
        )?)
    } else {
        None
    };
    Ok(Some(GapCase::of(
        goal.goal(),
        goal.scope(),
        goal.surface_concept(),
        candidates,
        snapshots,
        attached,
    )?))
}

/// Steps 2 through 6, without assembling the case.
fn descend(
    goal: &ActiveGoal,
    graph: &PrerequisiteGraph,
    readings: &[ConceptReading],
) -> Result<(Vec<RootCandidate>, Vec<StateSnapshot>), GapError> {
    let by_concept: BTreeMap<[u8; 16], &ConceptReading> = readings
        .iter()
        .map(|reading| (*reading.concept.as_bytes(), reading))
        .collect();

    let mut candidates: Vec<RootCandidate> = Vec::new();
    let mut snapshots: Vec<StateSnapshot> = Vec::new();
    let mut unsettled: BTreeSet<[u8; 16]> = BTreeSet::new();

    if let Some(reading) = by_concept.get(goal.surface_concept().as_bytes()) {
        let state = overlay_of(reading)?;
        if !state.identity().is_settled() {
            return Err(GapError::SurfaceIdentityUnsettled(goal.surface_concept()));
        }
        snapshots.push(StateSnapshot::from(&state));
    }

    for path in expand(goal, graph) {
        // A node below an unsettled identity was reached through an edge whose
        // subject is ambiguous, so it is not judged at all.
        if path
            .steps()
            .iter()
            .any(|step| unsettled.contains(step.advanced().as_bytes()))
        {
            continue;
        }
        let tip = path.tip();
        let reading = by_concept
            .get(tip.as_bytes())
            .copied()
            .ok_or(GapError::NoReadingForConcept(tip))?;
        let state = overlay_of(reading)?;
        require_band_is_not_from_the_path(&state, &path)?;
        snapshots.push(StateSnapshot::from(&state));

        if !state.identity().is_settled() {
            unsettled.insert(*tip.as_bytes());
        }

        let edge = edge_of(graph, &path)?;
        let floor = edge.floor().ok_or(GapError::NonBlockingEdgeOnPath)?;
        let branch = branch_standing(goal, graph, tip);
        let Some(kind) = route(&state, floor, &branch) else {
            continue;
        };

        let mut evidence: Vec<EvidenceId> = edge.evidence().to_vec();
        evidence.extend_from_slice(state.supporting());
        evidence.extend_from_slice(state.contradicting());
        evidence.sort_by_key(|id| id.as_uuid());
        evidence.dedup();

        let explanation = GapExplanation::of(ExplanationParts {
            kind,
            subject: tip,
            subject_kind: state.kind(),
            blocks: path.clone(),
            evidence: evidence.clone(),
            confidence: state.confidence(),
            current_state: StateSnapshot::from(&state),
            remediation: MinimumRemediation::of(
                reading.remediation_minutes,
                RemediationActivity::for_kind(kind),
                &reading.remediation_description,
                reading.remediation_sources.clone(),
            ),
            alternative: alternative_of(reading, edge),
            linked: reading.linked.clone(),
        })?;

        candidates.push(RootCandidate::of(
            kind,
            path.clone(),
            &reading.reason,
            evidence,
            state.confidence(),
            ancestors_of(&path),
            explanation,
        )?);
    }

    Ok((candidates, snapshots))
}

fn overlay_of(reading: &ConceptReading) -> Result<ConceptState, GapError> {
    ConceptState::overlay(
        reading.concept,
        reading.kind,
        reading.identity.clone(),
        &reading.offered,
        &reading.freshness,
        &reading.spillover,
    )
}

/// Refuses a band raised by a neighbour that lies on this node's own blocking
/// path. See the [`crate::state`] module note.
fn require_band_is_not_from_the_path(
    state: &ConceptState,
    path: &BlockingPath,
) -> Result<(), GapError> {
    for source in state.spillover_sources() {
        if path.holds(source.neighbor) {
            return Err(GapError::FreshnessRestsOnPathSpillover {
                concept: state.concept(),
                neighbor: source.neighbor,
                predicate: source.predicate.as_str(),
            });
        }
    }
    Ok(())
}

/// The edge the last hop of `path` was made across.
fn edge_of<'a>(
    graph: &'a PrerequisiteGraph,
    path: &BlockingPath,
) -> Result<&'a PrerequisiteEdge, GapError> {
    let step = path.steps().last().ok_or(GapError::NonBlockingEdgeOnPath)?;
    graph
        .edges()
        .iter()
        .find(|edge| {
            edge.advanced() == step.advanced()
                && edge.prerequisite() == step.prerequisite()
                && edge.predicate() == step.predicate()
                && edge.strength() == step.strength()
        })
        .ok_or(GapError::NonBlockingEdgeOnPath)
}

/// Whether the goal has chosen among this node's helpful branches.
fn branch_standing(
    goal: &ActiveGoal,
    graph: &PrerequisiteGraph,
    concept: EntityId,
) -> BranchStanding {
    let options: Vec<EntityId> = graph
        .helpful_out_of(concept)
        .into_iter()
        .filter(|option| !goal.criteria().names(*option))
        .collect();
    if options.len() >= 2 {
        BranchStanding::Unchosen { options }
    } else {
        BranchStanding::Settled
    }
}

/// Section 15.3's `대체 경로`.
fn alternative_of(reading: &ConceptReading, edge: &PrerequisiteEdge) -> AlternativePath {
    if reading.alternative_routes.is_empty() {
        AlternativePath::None {
            reason: if edge.strength() == PrerequisiteStrength::Hard {
                NoAlternativeReason::SoleHardPrerequisite
            } else {
                NoAlternativeReason::NoOtherAdmittedEdge
            },
        }
    } else {
        AlternativePath::Routes {
            routes: reading.alternative_routes.clone(),
        }
    }
}

/// Section 15.2 step 4's `조상 영향도`, from the surface down to the root's
/// parent.
fn ancestors_of(path: &BlockingPath) -> Vec<AncestorImpact> {
    let concepts = path.concepts();
    let steps = path.steps();
    let mut found = Vec::new();
    for (index, ancestor) in concepts.iter().enumerate().take(concepts.len() - 1) {
        let weakest = steps[index..]
            .iter()
            .map(PathStep::strength)
            .min()
            .unwrap_or(PrerequisiteStrength::Helpful);
        found.push(AncestorImpact::of(
            *ancestor,
            concepts.len() - 1 - index,
            weakest,
        ));
    }
    found
}
