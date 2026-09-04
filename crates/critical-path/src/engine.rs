//! Section 16's engine, in the order section 16.2 fixes: satisfy, cost,
//! constrain, **eliminate**, then order.
//!
//! ## The order is the contract
//!
//! `engine은 먼저 Pareto-dominated path를 제거하고, 남은 경로를 ... 이름으로
//! 보여준다`. [`plan`] runs those five stages once each, and the third and
//! fourth cannot be swapped: [`crate::preference::rank`] takes a
//! [`crate::pareto::ParetoFront`] and the only way to get one is
//! [`crate::pareto::ParetoFront::eliminate`].
//!
//! ## The engine decides nothing it was not given
//!
//! Every cost interval, every benefit interval, every constraint input and
//! every acquisition option arrives on [`PlanRequest`]. This engine chooses
//! *between* routes; it does not estimate what a concept costs, does not decide
//! whether an offering runs and does not decide what the user knows. Section
//! 16.2's four estimation input families are named on the estimate that arrives
//! ([`crate::vector::CostBasis`]), so an estimate that read none of them is
//! disclosed as a range with an empty basis rather than being computed here out
//! of nothing.
//!
//! ## It reads no clock, opens nothing and calls no model
//!
//! Every instant that reaches this engine already arrived inside a `P2-N3`
//! value or a caller-supplied day count. `no_clock_socket_or_file_reaches_this_crate`
//! in `crates/critical-path/tests/critical_path_scans.rs` compares the whole set
//! of this crate's `use` items, the whole set of the paths it reaches through a
//! crate root and the whole set of the macros it invokes against pinned
//! inventories in both directions.
//!
//! ## It is not a thirteenth `P2-C5` engine
//!
//! `P2-C5`'s registry is section 28's twelve table rows and nothing else, and
//! section 16 is not one of them. So this crate registers no engine id, adds no
//! directory under `testdata/engines/`, and proves its determinism the way the
//! reference engine does -- with `P2-C5`'s own [`crate::proof`] vocabulary over
//! a corpus of its own. `docs/contracts/critical-path.md` records the reading.

use std::collections::{BTreeMap, BTreeSet};

use academic_domain::{ContentDigest, EntityId, EvidenceId};
use academic_gap::GapCase;

use crate::{
    CriticalPathError,
    checkpoint::{CheckpointDecision, uncertain_edge_ratio_permille},
    constraint::{
        Constraint, ConstraintFinding, ConstraintInputs, ConstraintVerdict, RequiredInsertion,
        evaluate,
    },
    counterfactual::sensitivity_of,
    disclosure::{
        AlternativeRoute, Alternatives, ComputationSnapshot, CostAssumption, CostAssumptions,
        Disclosure, ExcludedRoute, ExclusionReason, Exclusions, UncertainEdge, UncertainEdges,
    },
    hypergraph::{EdgeStanding, PrerequisiteHypergraph, SatisfyingSet, satisfying_sets},
    option::AcquisitionOption,
    pareto::ParetoFront,
    plan::{Candidate, CriticalPathResult, PathRole, PlanStep},
    preference::{NAMED_STRATEGIES, NamedStrategy, PreferenceSlider, rank},
    vector::{
        BENEFIT_COMPONENTS, BenefitVector, COST_COMPONENTS, CostComponent, CostEstimate, CostVector,
    },
};

/// Everything one concept costs and yields, as the caller measured it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptEstimate {
    /// Which concept.
    pub concept: EntityId,
    /// Its seven cost axes.
    pub cost: CostVector,
    /// Its five benefit axes.
    pub benefit: BenefitVector,
    /// The ways of acquiring it. Never empty: a concept with no option is a
    /// concept the plan cannot schedule.
    pub options: Vec<AcquisitionOption>,
}

/// One run of section 16's engine.
#[derive(Debug, Clone)]
pub struct PlanRequest<'a> {
    /// The `P2-N5` case this plan answers. Its goal is the plan's goal and its
    /// surface concept is the concept the hypergraph is solved from, so a plan
    /// with no evidence-backed prerequisite deficit behind it is not a value
    /// this engine can be called with.
    pub gap_case: &'a GapCase,
    /// Section 16.1's hypergraph.
    pub graph: &'a PrerequisiteHypergraph,
    /// One estimate per concept the hypergraph can reach.
    pub estimates: &'a [ConceptEstimate],
    /// Section 16.3's inputs.
    pub constraints: &'a ConstraintInputs,
    /// The user's preference. No default: see [`PreferenceSlider`].
    pub slider: &'a PreferenceSlider,
    /// The published rule set this run is pinned to, and the engine's own
    /// version. Both go into the disclosure's snapshot, so two answers that
    /// differ can be told apart by what produced them.
    pub rule_set_hash: ContentDigest,
    /// The engine version.
    pub engine_version: u16,
}

/// Section 16's engine.
///
/// # Errors
///
/// [`CriticalPathError::NoEstimateForConcept`] when a satisfying set reaches a
/// concept the caller supplied no estimate for -- this engine never invents one;
/// [`CriticalPathError::ConceptHasNoAcquisitionOption`] for an estimate with no
/// option; and everything [`satisfying_sets`], [`evaluate`], [`Candidate::of`],
/// [`CostVector::plus`] and [`CriticalPathResult::of`] raise.
pub fn plan(request: &PlanRequest<'_>) -> Result<CriticalPathResult, CriticalPathError> {
    let goal_concept = request.gap_case.surface_concept();
    let sets = satisfying_sets(request.graph, goal_concept)?;

    let by_concept: BTreeMap<[u8; 16], &ConceptEstimate> = request
        .estimates
        .iter()
        .map(|estimate| (*estimate.concept.as_bytes(), estimate))
        .collect();

    let mut candidates = Vec::new();
    for set in &sets {
        candidates.push(candidate_for(set, &by_concept, request.constraints)?);
    }

    // Section 16.3 first: an infeasible route is not a route, and eliminating
    // against it would let a refused candidate dominate a feasible one.
    let (feasible, refused): (Vec<Candidate>, Vec<Candidate>) = candidates
        .into_iter()
        .partition(academic_partition_is_feasible);

    let front = ParetoFront::eliminate(feasible);
    let ranking = rank(&front, request.slider);

    let ranked_candidates: Vec<Candidate> = ranking
        .candidates()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let roles = roles_of(request.graph, &ranked_candidates);

    let ranked = ranked_candidates
        .iter()
        .enumerate()
        .map(|(position, candidate)| {
            CriticalPathResult::ranked_path(
                candidate.clone(),
                position,
                if position == 0 {
                    PathRole::SharedSpine
                } else {
                    PathRole::AlternativePath
                },
                strategy_for(candidate, &ranked_candidates),
            )
        })
        .collect();

    let disclosure = disclose(request, &sets, &front, &refused, &ranked_candidates)?;

    CriticalPathResult::of(
        request.gap_case.goal(),
        front,
        ranked,
        roles,
        request.slider.clone(),
        disclosure,
    )
}

/// A free function rather than a closure so the partition reads as the rule it
/// is: section 16.3 decides membership before section 16.2 decides dominance.
fn academic_partition_is_feasible(candidate: &Candidate) -> bool {
    candidate.is_feasible()
}

fn candidate_for(
    set: &SatisfyingSet,
    by_concept: &BTreeMap<[u8; 16], &ConceptEstimate>,
    inputs: &ConstraintInputs,
) -> Result<Candidate, CriticalPathError> {
    let mut cost: Option<CostVector> = None;
    let mut benefit_parts: Vec<[CostEstimate; 5]> = Vec::new();
    let mut options: Vec<AcquisitionOption> = Vec::new();
    let mut steps: Vec<PlanStep> = Vec::new();

    for concept in set.concepts() {
        let estimate = by_concept
            .get(concept.as_bytes())
            .copied()
            .ok_or(CriticalPathError::NoEstimateForConcept(*concept))?;
        if estimate.options.is_empty() {
            return Err(CriticalPathError::ConceptHasNoAcquisitionOption(*concept));
        }
        cost = Some(match cost {
            Some(held) => held.plus(&estimate.cost)?,
            None => estimate.cost.clone(),
        });
        benefit_parts.push(
            BENEFIT_COMPONENTS.map(|component| estimate.benefit.component(component).clone()),
        );
        options.extend(estimate.options.iter().cloned());
        steps.push(PlanStep::of(*concept, estimate.options.clone(), None));
    }

    let cost = cost.ok_or(CriticalPathError::EmptySatisfyingSet)?;
    let benefit = fold_benefit(benefit_parts)?;

    let delay_high = cost.component(CostComponent::CalendarDelay).high();
    let findings = evaluate(set, &options, inputs, delay_high)?;
    let checkpoint = CheckpointDecision::for_ratio(uncertain_edge_ratio_permille(set));

    // Every insertion a constraint requires is attached to the plan's first
    // step, which is what `REQ-16-017`'s `before branch commitment` means: the
    // user reaches it before studying anything on the route.
    let insertion = first_insertion(&findings);
    if let (Some(first), Some(required)) = (steps.first_mut(), insertion) {
        *first = PlanStep::of(first.concept(), first.options().to_vec(), Some(required));
    }

    Candidate::of(set.clone(), steps, cost, benefit, findings, checkpoint)
}

fn first_insertion(findings: &[ConstraintFinding; 8]) -> Option<RequiredInsertion> {
    findings
        .iter()
        .find_map(|finding| finding.insertion().cloned())
}

/// Axis-wise interval addition over the benefit vectors of a set's concepts.
fn fold_benefit(parts: Vec<[CostEstimate; 5]>) -> Result<BenefitVector, CriticalPathError> {
    let mut iterator = parts.into_iter();
    let mut held = iterator
        .next()
        .ok_or(CriticalPathError::EmptySatisfyingSet)?;
    for next in iterator {
        let mut summed = Vec::with_capacity(BENEFIT_COMPONENTS.len());
        for (left, right) in held.iter().zip(next.iter()) {
            summed.push(left.plus(right)?);
        }
        held = summed
            .try_into()
            .map_err(|_| CriticalPathError::AxisCountChanged)?;
    }
    BenefitVector::of(held)
}

/// Section 16.4's four roles over every concept the hypergraph mentions.
fn roles_of(graph: &PrerequisiteHypergraph, ranked: &[Candidate]) -> Vec<(EntityId, PathRole)> {
    let mut mentioned: BTreeSet<[u8; 16]> = BTreeSet::new();
    let mut order: Vec<EntityId> = Vec::new();
    for member in graph.all_members() {
        for concept in [member.dependent(), member.concept()] {
            if mentioned.insert(*concept.as_bytes()) {
                order.push(concept);
            }
        }
    }
    order.sort_by_key(|id| id.as_uuid());

    order
        .into_iter()
        .map(|concept| {
            let on_any = ranked
                .iter()
                .any(|candidate| candidate.satisfying_set().holds(concept));
            let on_all = !ranked.is_empty()
                && ranked
                    .iter()
                    .all(|candidate| candidate.satisfying_set().holds(concept));
            let on_first = ranked
                .first()
                .is_some_and(|candidate| candidate.satisfying_set().holds(concept));
            let role = if on_all {
                PathRole::SharedSpine
            } else if on_first {
                PathRole::OptionalBranch
            } else if on_any {
                PathRole::AlternativePath
            } else {
                PathRole::IrrelevantPeriphery
            };
            (concept, role)
        })
        .collect()
}

/// The section 16.2 name a route is shown under, when one fits.
///
/// A route earns a name when ordering the whole surviving set under that
/// strategy's own slider puts it first. So a name is a **report of an
/// ordering** and never an input to one, which is why
/// `named_strategies_do_not_alter_vectors` holds: computing this reads the
/// vectors and writes nothing.
fn strategy_for(candidate: &Candidate, ranked: &[Candidate]) -> Option<NamedStrategy> {
    NAMED_STRATEGIES.into_iter().find(|strategy| {
        strategy.slider().is_ok_and(|slider| {
            let front = ParetoFront::eliminate(ranked.to_vec());
            rank(&front, &slider)
                .candidates()
                .first()
                .is_some_and(|first| *first == candidate)
        })
    })
}

fn disclose(
    request: &PlanRequest<'_>,
    sets: &[SatisfyingSet],
    front: &ParetoFront,
    refused: &[Candidate],
    ranked: &[Candidate],
) -> Result<Disclosure, CriticalPathError> {
    let snapshot = ComputationSnapshot {
        goal: request.gap_case.goal(),
        frozen_inputs: crate::proof::frozen_inputs(request)?.digest(),
        engine_version: request.engine_version,
        rule_set_hash: request.rule_set_hash,
        hyperedge_member_count: request.graph.all_members().len(),
        candidate_count: sets.len(),
    };

    let assumptions = ranked.first().map_or_else(
        || {
            // No route survived, so there is no route's cost to disclose. The
            // group still cannot be empty, so it carries the hypergraph's own
            // axes at zero width with no basis -- which is a statement that
            // nothing was measured, not a measurement of nothing.
            COST_COMPONENTS
                .into_iter()
                .map(|axis| CostAssumption {
                    axis,
                    low: 0,
                    high: 0,
                    families: Vec::new(),
                })
                .collect::<Vec<_>>()
        },
        |candidate| {
            COST_COMPONENTS
                .into_iter()
                .map(|axis| {
                    let estimate = candidate.cost().component(axis);
                    CostAssumption {
                        axis,
                        low: estimate.low(),
                        high: estimate.high(),
                        families: estimate.basis().families().to_vec(),
                    }
                })
                .collect::<Vec<_>>()
        },
    );

    let mut excluded: Vec<ExcludedRoute> = front
        .dominated()
        .iter()
        .map(|entry| ExcludedRoute {
            concepts: entry.candidate().satisfying_set().concepts().to_vec(),
            reason: ExclusionReason::ParetoDominated,
            constraint: None,
        })
        .collect();
    for candidate in refused {
        let refusal = candidate.refusals().first().map(|finding| {
            (
                finding.constraint(),
                match finding.verdict() {
                    ConstraintVerdict::Unknown => ExclusionReason::ConstraintUnknown,
                    ConstraintVerdict::Violated
                    | ConstraintVerdict::Satisfied
                    | ConstraintVerdict::SatisfiedWithInsertion => {
                        ExclusionReason::ConstraintViolated
                    }
                },
            )
        });
        let (constraint, reason) = refusal.map_or(
            (None::<Constraint>, ExclusionReason::ConstraintViolated),
            |(constraint, reason)| (Some(constraint), reason),
        );
        excluded.push(ExcludedRoute {
            concepts: candidate.satisfying_set().concepts().to_vec(),
            reason,
            constraint,
        });
    }

    let uncertain = match ranked.first() {
        None => UncertainEdges::of(Vec::new(), 0),
        Some(candidate) => {
            let mut edges = Vec::new();
            for member in candidate.satisfying_set().members() {
                if member.standing() != EdgeStanding::Uncertain {
                    continue;
                }
                let previewed = sensitivity_of(
                    request.graph,
                    request.gap_case.surface_concept(),
                    member,
                    sets,
                )?;
                edges.push(UncertainEdge {
                    dependent: member.dependent(),
                    prerequisite: member.concept(),
                    predicate: member.edge().predicate().as_str().to_owned(),
                    if_removed: previewed.outcome(),
                });
            }
            UncertainEdges::of(
                edges,
                uncertain_edge_ratio_permille(candidate.satisfying_set()),
            )
        }
    };

    let alternatives = Alternatives::of(
        ranked
            .iter()
            .enumerate()
            .skip(1)
            .map(|(position, candidate)| AlternativeRoute {
                concepts: candidate.satisfying_set().concepts().to_vec(),
                rank: position,
                strategy: strategy_for(candidate, ranked),
                sources: sources_of(candidate),
            })
            .collect(),
        ranked.len(),
    );

    Ok(Disclosure::of(
        snapshot,
        CostAssumptions::of(assumptions)?,
        Exclusions::of(excluded),
        uncertain,
        alternatives,
    ))
}

fn sources_of(candidate: &Candidate) -> Vec<EvidenceId> {
    let mut found: Vec<EvidenceId> = candidate
        .steps()
        .iter()
        .flat_map(|step| step.options().iter().flat_map(AcquisitionOption::supplies))
        .map(crate::option::Opportunity::source)
        .collect();
    found.sort_by_key(|id| id.as_uuid());
    found.dedup();
    found
}
