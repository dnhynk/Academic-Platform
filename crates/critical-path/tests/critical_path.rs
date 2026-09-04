//! `P2-N6`'s thirteen named acceptance rows.
//!
//! Four of them read `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compare what is in this crate against what is in the document, in both
//! directions: section 16.2's two vector blocks, section 16.3's bullet list and
//! section 16.5's closing sentence are **measurements** rather than counts
//! restated in a test.
//!
//! The same reads pick up two things the design document leaves open, and both
//! are recorded rather than resolved:
//! `the_four_strategy_names_are_introduced_as_examples` and
//! `the_eighth_constraint_is_the_checkpoint_rule`.

#[path = "common/mod.rs"]
mod common;

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_critical_path::{
    BENEFIT_COMPONENTS, BasisFamily, BenefitComponent, CONSTRAINTS, CheckpointDecision,
    ConceptEstimate, Constraint, ConstraintVerdict, CostBasis, CostComponent, CostEstimate,
    DISCLOSURE_GROUPS, Dominance, EdgeOutcome, EdgeStanding, EditedPlan, Exclusions, Hyperedge,
    NAMED_STRATEGIES, PATH_ROLES, ParetoFront, PathRole, PreferenceSlider, PrerequisiteHypergraph,
    RelationEdit, RequiredInsertion, STRATEGY_NAMES_ARE_EXAMPLES,
    UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE, UncertainEdges, Unit, VectorAxis, all_axes, dominance,
    edited, plan, rank, satisfying_sets, sensitivity, shortest_by_node_count,
    uncertain_edge_ratio_permille,
};
use academic_curriculum::{Meeting, OfferingStatus, Weekday};
use academic_domain::{EntityId, FreshnessBand};

use common::{
    Scenario, TestResult, all_concepts, benefit, benefit_except, buffer_pool, cost_except,
    course_for, database_offering, disk_page, entity, evidence_id, experiment_for, fan_out,
    flat_benefit, flat_cost, flat_estimates, measured, member, page_layout, permissive_constraints,
    random_io, reading_for, rule_set, section_16_1_graph, section_36_4_gap, slider_led_by,
    spec_order_slider, storage_hierarchy, unmeasured, with_estimate,
};

// ---------------------------------------------------------------------------
// Reading the design document.
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// The body of a `### ` section, from its heading to the next one.
fn section(page: &str, heading: &str) -> Result<String, Box<dyn Error>> {
    let start = page
        .find(heading)
        .ok_or_else(|| format!("the design document has no {heading}"))?;
    let rest = &page[start..];
    let end = rest[1..]
        .find("\n### ")
        .map_or(rest.len(), |offset| offset + 1);
    Ok(rest[..end].to_owned())
}

/// The identifiers inside the `NAME(P) = <` block of section 16.2, one per
/// line, in the order the block writes them.
fn vector_block(body: &str, name: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let opener = format!("{name}(P) = <");
    let start = body
        .find(&opener)
        .ok_or_else(|| format!("section 16.2 has no {opener}"))?;
    let rest = &body[start + opener.len()..];
    let end = rest
        .find('>')
        .ok_or_else(|| format!("section 16.2's {name} block is not closed"))?;
    let found: Vec<String> = rest[..end]
        .lines()
        .map(|line| line.trim().trim_end_matches(',').trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    if found.is_empty() {
        return Err(format!("section 16.2's {name} block is empty").into());
    }
    Ok(found)
}

/// The `- ` bullets of a section, in order.
fn bullets(body: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let found: Vec<String> = body
        .lines()
        .filter_map(|line| line.strip_prefix("- ").map(|rest| rest.trim().to_owned()))
        .collect();
    if found.is_empty() {
        return Err("the section has no bullets".into());
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// `and_or_hypergraph_is_satisfied_not_shortest`
// ---------------------------------------------------------------------------

/// Section 16.1: the answer is a satisfied set, and a node-count walk is a
/// different and wrong answer on the same graph.
#[test]
fn and_or_hypergraph_is_satisfied_not_shortest() -> TestResult {
    let graph = section_16_1_graph(&[])?;
    let sets = satisfying_sets(&graph, buffer_pool())?;

    // `REQ-16-001`: both mandatory nodes, and one selectable branch.
    assert_eq!(
        sets.len(),
        2,
        "one `ONE OF` with two branches is two answers"
    );
    for set in &sets {
        assert!(set.holds(disk_page()), "a mandatory member is missing");
        assert!(set.holds(random_io()), "a mandatory member is missing");
        let takes_a = set.holds(storage_hierarchy());
        let takes_b = set.holds(fan_out()) && set.holds(page_layout());
        assert!(
            takes_a ^ takes_b,
            "a satisfying set takes exactly one branch, whole"
        );
        // A branch is taken whole: the larger branch's second member cannot be
        // dropped to make the set smaller.
        assert_eq!(
            set.holds(fan_out()),
            set.holds(page_layout()),
            "one member of a branch was taken without the other"
        );
    }

    // The naive answer is a *walk*, and its failure is the first one section
    // 34.5's `AND/OR 무시` names: it arrives at one member of the conjunction
    // and stops, so it never reaches the other. Its result is a strict subset
    // of a satisfying set — a plan that omits something the goal requires.
    let walk = shortest_by_node_count(&graph, buffer_pool());
    let walked: BTreeSet<EntityId> = walk.iter().copied().collect();
    assert!(
        !walked.contains(&random_io()),
        "the node-count walk reached both conjunction members, so this comparison \
         proves nothing"
    );
    let mut strictly_inside = 0_usize;
    for set in &sets {
        let members: BTreeSet<EntityId> = set.concepts().iter().copied().collect();
        assert_ne!(walked, members, "the naive walk equals a satisfying set");
        if walked.is_subset(&members) {
            strictly_inside += 1;
        }
    }
    assert_eq!(
        strictly_inside, 1,
        "the walk is not a strict subset of exactly one satisfying set"
    );

    // Node count does not decide. The smaller set is the one a count would
    // take; excluding one of its members leaves the *larger* set as the answer,
    // so the engine's answer is not a function of size.
    let scenario = Scenario::new()?;
    let mut constraints = permissive_constraints();
    constraints.user_excluded_concepts = vec![storage_hierarchy()];
    let request = academic_critical_path::PlanRequest {
        constraints: &constraints,
        ..scenario.request()
    };
    let result = plan(&request)?;
    let ranked = result.ranked();
    assert_eq!(ranked.len(), 1, "one route survived the exclusion");
    let survivor = ranked[0].candidate().satisfying_set();
    assert!(survivor.holds(fan_out()) && survivor.holds(page_layout()));
    assert_eq!(
        survivor.concepts().len(),
        5,
        "the surviving route is the larger one"
    );

    // And with nothing excluded, the smaller route does not automatically win:
    // both survive, because neither dominates the other on the vectors.
    let both = plan(&scenario.request())?;
    assert_eq!(both.ranked().len(), 2, "both branches are offered");
    Ok(())
}

// ---------------------------------------------------------------------------
// `cost_vector_has_seven_separate_components`
// ---------------------------------------------------------------------------

/// Section 16.2's `Cost(P)` block, measured in both directions, and the absence
/// of anything that would fold it.
#[test]
fn cost_vector_has_seven_separate_components() -> TestResult {
    let page = specification()?;
    let body = section(&page, "### 16.2 비용 벡터")?;
    let written = vector_block(&body, "Cost")?;

    assert_eq!(
        written.len(),
        7,
        "section 16.2's Cost(P) block does not have seven lines"
    );
    let declared: Vec<String> = academic_critical_path::COST_COMPONENTS
        .iter()
        .map(|component| component.spec_token().to_owned())
        .collect();
    assert_eq!(
        written, declared,
        "the cost axes and the design document's own lines disagree"
    );
    for axis in academic_critical_path::COST_COMPONENTS {
        assert!(
            written.contains(&axis.spec_token().to_owned()),
            "{} is declared and section 16.2 does not write it",
            axis.spec_token()
        );
    }

    // Seven values that are separately readable, and separately different.
    let vector = flat_cost(10)?;
    let mut seen = BTreeSet::new();
    for (index, axis) in academic_critical_path::COST_COMPONENTS
        .into_iter()
        .enumerate()
    {
        let moved = cost_except(
            10,
            axis,
            measured(
                axis,
                100 + u32::try_from(index)?,
                100 + u32::try_from(index)?,
            )?,
        )?;
        assert_ne!(
            moved.component(axis).low(),
            vector.component(axis).low(),
            "{} did not move when it was moved",
            axis.spec_token()
        );
        // Moving one axis moves nothing else, which is what `separate` means.
        for other in academic_critical_path::COST_COMPONENTS {
            if other != axis {
                assert_eq!(
                    moved.component(other),
                    vector.component(other),
                    "moving {} moved {}",
                    axis.spec_token(),
                    other.spec_token()
                );
            }
        }
        seen.insert(axis);
    }
    assert_eq!(seen.len(), 7, "an axis was visited twice");

    // Two vectors compare for **equality** and for nothing else. A runtime test
    // cannot observe the absence of an order, so the two halves that can are:
    // `crates/critical-path/tests/compile_fail/a_cost_vector_does_not_compare.rs`,
    // where `left < right` is a program that does not compile, and
    // `the_vectors_cannot_be_folded`, which reads the `#[derive(..)]` above each
    // declaration and requires neither `PartialOrd` nor `Ord` to be there.
    assert_eq!(vector, flat_cost(10)?);
    assert_ne!(vector, flat_cost(11)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// `benefit_vector_has_five_separate_components`
// ---------------------------------------------------------------------------

/// Section 16.2's `Benefit(P)` block, measured in both directions.
#[test]
fn benefit_vector_has_five_separate_components() -> TestResult {
    let page = specification()?;
    let body = section(&page, "### 16.2 비용 벡터")?;
    let written = vector_block(&body, "Benefit")?;

    assert_eq!(
        written.len(),
        5,
        "section 16.2's Benefit(P) block does not have five lines"
    );
    let declared: Vec<String> = BENEFIT_COMPONENTS
        .iter()
        .map(|component| component.spec_token().to_owned())
        .collect();
    assert_eq!(
        written, declared,
        "the benefit axes and the design document's own lines disagree"
    );
    for axis in BENEFIT_COMPONENTS {
        assert!(
            written.contains(&axis.spec_token().to_owned()),
            "{} is declared and section 16.2 does not write it",
            axis.spec_token()
        );
    }

    let vector = flat_benefit(10)?;
    for (index, axis) in BENEFIT_COMPONENTS.into_iter().enumerate() {
        let moved = benefit_except(
            10,
            axis,
            benefit(
                axis,
                200 + u32::try_from(index)?,
                200 + u32::try_from(index)?,
            )?,
        )?;
        assert_ne!(moved.component(axis).low(), vector.component(axis).low());
        for other in BENEFIT_COMPONENTS {
            if other != axis {
                assert_eq!(
                    moved.component(other),
                    vector.component(other),
                    "moving {} moved {}",
                    axis.spec_token(),
                    other.spec_token()
                );
            }
        }
    }

    // The two vectors are twelve separate axes between them, and a slider must
    // order all twelve.
    assert_eq!(all_axes().len(), 12);
    assert_eq!(
        all_axes().iter().collect::<BTreeSet<_>>().len(),
        12,
        "an axis appears twice in the whole-axis list"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// `slider_changes_order_not_facts`
// ---------------------------------------------------------------------------

/// Section 16.2: a preference reorders and rewrites nothing.
#[test]
fn slider_changes_order_not_facts() -> TestResult {
    let scenario = Scenario::new()?;
    // Two routes that differ on two axes in opposite directions, so no route
    // dominates and a preference decides between them.
    let estimates = separating_estimates()?;
    let request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    let under_spec = plan(&request)?;

    let reversed = common::reversed_slider()?;
    let flipped_request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        slider: &reversed,
        ..scenario.request()
    };
    let under_reversed = plan(&flipped_request)?;

    // The order changed.
    let first_of = |result: &academic_critical_path::CriticalPathResult| {
        result
            .ranked()
            .first()
            .map(|path| path.candidate().satisfying_set().concepts().to_vec())
    };
    assert_ne!(
        first_of(&under_spec),
        first_of(&under_reversed),
        "the two preferences produced the same first route, so this proves nothing"
    );

    // The facts did not. Every surviving route's two vectors are identical
    // under both preferences, axis by axis, on both interval ends and on the
    // basis each rests on.
    let sorted = |result: &academic_critical_path::CriticalPathResult| {
        let mut held: Vec<_> = result.front().candidates().to_vec();
        held.sort_by_key(|candidate| {
            candidate
                .satisfying_set()
                .concepts()
                .iter()
                .map(|id| id.as_uuid())
                .collect::<Vec<_>>()
        });
        held
    };
    let left = sorted(&under_spec);
    let right = sorted(&under_reversed);
    assert_eq!(left.len(), right.len());
    assert!(
        !left.is_empty(),
        "no route survived, so nothing is compared"
    );
    for (a, b) in left.iter().zip(right.iter()) {
        for axis in academic_critical_path::COST_COMPONENTS {
            assert_eq!(
                a.cost().component(axis),
                b.cost().component(axis),
                "a preference changed the {} axis",
                axis.spec_token()
            );
        }
        for axis in BENEFIT_COMPONENTS {
            assert_eq!(
                a.benefit().component(axis),
                b.benefit().component(axis),
                "a preference changed the {} axis",
                axis.spec_token()
            );
        }
        assert_eq!(a.constraints(), b.constraints());
        assert_eq!(a.satisfying_set(), b.satisfying_set());
    }

    // The Pareto front is a fact too: the same routes survive either way.
    assert_eq!(
        under_spec.front().candidates().len(),
        under_reversed.front().candidates().len()
    );

    // A preference reads **both** ends of an interval. Two routes that share a
    // low end and differ on the high one are separated by the high one, and a
    // ranker that read the low end alone would call them equal and fall through
    // to the identifier tie-break. Without this case, ordering by half an
    // interval is invisible to every behavioural test here.
    let straddling = straddling_estimates()?;
    let led = slider_led_by(VectorAxis::Cost {
        component: CostComponent::LearningEffort,
    })?;
    let straddled = plan(&academic_critical_path::PlanRequest {
        estimates: &straddling,
        slider: &led,
        ..scenario.request()
    })?;
    let ordered = straddled.ranked();
    assert_eq!(ordered.len(), 2, "both routes must survive to be ordered");
    let tighter = ordered[0]
        .candidate()
        .cost()
        .component(CostComponent::LearningEffort);
    let looser = ordered[1]
        .candidate()
        .cost()
        .component(CostComponent::LearningEffort);
    assert_eq!(
        tighter.low(),
        looser.low(),
        "the fixture routes do not share a low end, so this proves nothing"
    );
    assert!(
        tighter.high() < looser.high(),
        "the tighter interval did not rank first, so the high end is not read"
    );

    // A preference that drops an axis is refused rather than treated as
    // indifference.
    let mut short = all_axes();
    short.pop();
    assert!(matches!(
        PreferenceSlider::of(short),
        Err(academic_critical_path::CriticalPathError::SliderIsNotAPermutation)
    ));
    let mut doubled = all_axes();
    doubled.push(all_axes()[0]);
    assert!(matches!(
        PreferenceSlider::of(doubled),
        Err(academic_critical_path::CriticalPathError::SliderIsNotAPermutation)
    ));
    Ok(())
}

/// Two routes whose learning-effort intervals share a low end and differ on the
/// high one, so only the high end can separate them.
///
/// The smaller branch's concept carries `[20, 20]` and the larger branch's two
/// carry `[10, 10]` and `[10, 30]`, which sum to `[20, 40]`: the same low end,
/// a wider high one. Neither dominates -- the benefit axes are equal and the
/// cost axes agree on the low end -- so both survive elimination and the
/// preference is what orders them.
fn straddling_estimates() -> Result<Vec<ConceptEstimate>, Box<dyn Error>> {
    let mut estimates = flat_estimates()?;
    for (concept, low, high) in [
        (storage_hierarchy(), 20, 20),
        (fan_out(), 10, 10),
        (page_layout(), 10, 30),
    ] {
        estimates = with_estimate(
            estimates,
            ConceptEstimate {
                concept,
                cost: cost_except(
                    0,
                    CostComponent::LearningEffort,
                    measured(CostComponent::LearningEffort, low, high)?,
                )?,
                benefit: flat_benefit(10)?,
                options: vec![reading_for(concept, "straddle")?],
            },
        );
    }
    // The two mandatory concepts contribute equally to both routes, so the only
    // difference between the sums is the branch.
    for concept in [buffer_pool(), disk_page(), random_io()] {
        estimates = with_estimate(
            estimates,
            ConceptEstimate {
                concept,
                cost: cost_except(
                    0,
                    CostComponent::LearningEffort,
                    measured(CostComponent::LearningEffort, 0, 0)?,
                )?,
                benefit: flat_benefit(10)?,
                options: vec![reading_for(concept, "straddle")?],
            },
        );
    }
    Ok(estimates)
}

/// Two routes that differ in opposite directions on two axes, so neither
/// dominates and a preference is what separates them.
///
/// The smaller branch is cheaper in learning effort and worth less in immediate
/// project value; the larger branch is the reverse.
fn separating_estimates() -> Result<Vec<ConceptEstimate>, Box<dyn Error>> {
    let mut estimates = flat_estimates()?;
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: storage_hierarchy(),
            cost: cost_except(
                10,
                CostComponent::LearningEffort,
                measured(CostComponent::LearningEffort, 5, 5)?,
            )?,
            benefit: benefit_except(
                10,
                BenefitComponent::ImmediateProjectValue,
                benefit(BenefitComponent::ImmediateProjectValue, 1, 1)?,
            )?,
            options: vec![reading_for(storage_hierarchy(), "sh")?],
        },
    );
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: fan_out(),
            cost: cost_except(
                10,
                CostComponent::LearningEffort,
                measured(CostComponent::LearningEffort, 40, 40)?,
            )?,
            benefit: benefit_except(
                10,
                BenefitComponent::ImmediateProjectValue,
                benefit(BenefitComponent::ImmediateProjectValue, 400, 400)?,
            )?,
            options: vec![reading_for(fan_out(), "fo")?],
        },
    );
    Ok(estimates)
}

// ---------------------------------------------------------------------------
// `dominated_paths_are_removed_before_ranking`
// ---------------------------------------------------------------------------

/// Section 16.2's `먼저`: elimination runs first, and the ranker has no other
/// input.
#[test]
fn dominated_paths_are_removed_before_ranking() -> TestResult {
    let scenario = Scenario::new()?;
    // The larger branch is strictly worse: more cost on every axis and less
    // benefit on every axis. `REQ-16-005`'s `path D strictly dominated by A`.
    let mut estimates = flat_estimates()?;
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: fan_out(),
            cost: flat_cost(90)?,
            benefit: flat_benefit(1)?,
            options: vec![reading_for(fan_out(), "fo")?],
        },
    );
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: page_layout(),
            cost: flat_cost(90)?,
            benefit: flat_benefit(1)?,
            options: vec![reading_for(page_layout(), "pl")?],
        },
    );
    let request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    let result = plan(&request)?;

    assert_eq!(result.front().candidates().len(), 1, "one route survived");
    assert_eq!(result.front().dominated().len(), 1, "one route was removed");
    let removed = result.front().dominated()[0].candidate();
    assert!(removed.satisfying_set().holds(fan_out()));

    // The dominated route is absent from the ranking under **every**
    // preference, including one that would otherwise favour it.
    for slider in [
        spec_order_slider()?,
        common::reversed_slider()?,
        slider_led_by(VectorAxis::Cost {
            component: CostComponent::LearningEffort,
        })?,
        slider_led_by(VectorAxis::Benefit {
            component: BenefitComponent::ImmediateProjectValue,
        })?,
    ] {
        let ranked = rank(result.front(), &slider);
        assert_eq!(ranked.candidates().len(), 1);
        assert!(
            !ranked.candidates()[0].satisfying_set().holds(fan_out()),
            "a dominated route was ranked"
        );
    }

    // It is disclosed rather than discarded: section 16.5's third group names
    // it and says why.
    let excluded = result.disclosure().exclusions();
    assert!(matches!(excluded, Exclusions::Excluded { .. }));
    assert!(
        excluded.routes().iter().any(|route| {
            route.reason == academic_critical_path::ExclusionReason::ParetoDominated
                && route.concepts.contains(&fan_out())
        }),
        "the dominated route is not disclosed"
    );

    // Domination itself is preference-free and genuinely partial.
    let strong = &result.front().candidates()[0];
    assert_eq!(dominance(strong, removed), Dominance::LeftDominates);
    assert_eq!(dominance(removed, strong), Dominance::RightDominates);
    assert_eq!(dominance(strong, strong), Dominance::Incomparable);

    // Two candidates whose intervals cross on one axis are incomparable, so a
    // range does not become a point in order to be eliminated.
    let crossing = crossing_front()?;
    assert_eq!(crossing.candidates().len(), 2, "an overlap was eliminated");
    assert!(crossing.dominated().is_empty());

    // The two routes must be **identical** on every other axis, or they would be
    // incomparable for a reason that has nothing to do with the crossing and
    // this assertion would prove nothing.
    let (left, right) = (&crossing.candidates()[0], &crossing.candidates()[1]);
    for component in academic_critical_path::COST_COMPONENTS {
        if component == CostComponent::LearningEffort {
            continue;
        }
        assert_eq!(
            left.cost().component(component),
            right.cost().component(component),
            "the crossing fixture's routes also differ on {}",
            component.spec_token()
        );
    }
    for component in BENEFIT_COMPONENTS {
        assert_eq!(
            left.benefit().component(component),
            right.benefit().component(component),
            "the crossing fixture's routes also differ on {}",
            component.spec_token()
        );
    }
    let (a, b) = (
        left.cost().component(CostComponent::LearningEffort),
        right.cost().component(CostComponent::LearningEffort),
    );
    assert!(
        (a.low() < b.low() && b.high() < a.high()) || (b.low() < a.low() && a.high() < b.high()),
        "the crossing fixture's intervals do not cross: {:?} against {:?}",
        (a.low(), a.high()),
        (b.low(), b.high())
    );
    Ok(())
}

/// A front of two candidates whose learning-effort intervals overlap without
/// either containing the other.
fn crossing_front() -> Result<ParetoFront, Box<dyn Error>> {
    let scenario = Scenario::new()?;
    let mut estimates = flat_estimates()?;
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: storage_hierarchy(),
            cost: cost_except(
                10,
                CostComponent::LearningEffort,
                measured(CostComponent::LearningEffort, 10, 40)?,
            )?,
            benefit: flat_benefit(10)?,
            options: vec![reading_for(storage_hierarchy(), "sh")?],
        },
    );
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: fan_out(),
            cost: cost_except(
                10,
                CostComponent::LearningEffort,
                measured(CostComponent::LearningEffort, 20, 30)?,
            )?,
            benefit: flat_benefit(10)?,
            options: vec![reading_for(fan_out(), "fo")?],
        },
    );
    // The larger branch's second concept contributes **zero** on every axis, so
    // the two routes agree on all eleven other axes and differ only where the
    // intervals cross. Without that, the two routes would differ on every flat
    // axis simply because one has more concepts, each route would beat the
    // other somewhere, and the pair would be incomparable for a reason that has
    // nothing to do with the crossing -- which is how this fixture would pass an
    // engine that had collapsed the interval to one end.
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: page_layout(),
            cost: cost_except(
                0,
                CostComponent::LearningEffort,
                measured(CostComponent::LearningEffort, 0, 0)?,
            )?,
            benefit: flat_benefit(0)?,
            options: vec![reading_for(page_layout(), "pl")?],
        },
    );
    let request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    Ok(plan(&request)?.front().clone())
}

// ---------------------------------------------------------------------------
// `named_strategies_do_not_alter_vectors`
// ---------------------------------------------------------------------------

/// Section 16.2's four names: an ordering and nothing else.
#[test]
fn named_strategies_do_not_alter_vectors() -> TestResult {
    let scenario = Scenario::new()?;
    let estimates = separating_estimates()?;
    let request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    let baseline = plan(&request)?;
    let front = baseline.front().clone();
    assert!(
        front.candidates().len() >= 2,
        "one route cannot be reordered"
    );

    // Snapshot every axis of every surviving route before any strategy runs.
    let before: Vec<_> = front.candidates().to_vec();

    let mut orders = BTreeSet::new();
    for strategy in NAMED_STRATEGIES {
        let slider = strategy.slider()?;
        let ranked = rank(&front, &slider);
        orders.insert(ranked.order().to_vec());

        // The front is byte-identical afterwards: a strategy wrote nothing.
        assert_eq!(
            front.candidates(),
            before.as_slice(),
            "{} changed the front",
            strategy.as_str()
        );
        assert_eq!(ranked.front().candidates(), before.as_slice());
        for candidate in ranked.candidates() {
            for axis in academic_critical_path::COST_COMPONENTS {
                let original = before
                    .iter()
                    .find(|held| held.satisfying_set() == candidate.satisfying_set())
                    .ok_or("a ranked candidate is not on the front")?;
                assert_eq!(
                    candidate.cost().component(axis),
                    original.cost().component(axis),
                    "{} changed the {} axis",
                    strategy.as_str(),
                    axis.spec_token()
                );
            }
        }
    }

    // The four names are four sliders and at least two distinct orders, so the
    // names are doing something observable while changing nothing.
    assert_eq!(NAMED_STRATEGIES.len(), 4);
    assert!(
        orders.len() >= 2,
        "all four strategies produced one order, so the comparison is vacuous"
    );

    // Every strategy's slider is a complete permutation.
    for strategy in NAMED_STRATEGIES {
        let slider = strategy.slider()?;
        assert_eq!(slider.order().len(), 12);
        assert_eq!(
            slider.order().iter().collect::<BTreeSet<_>>().len(),
            12,
            "{} repeats an axis",
            strategy.as_str()
        );
    }
    Ok(())
}

/// Section 16.2 introduces the four with `같은`, which is `such as`.
///
/// Recorded rather than resolved: the count is a measurement of an open list,
/// and `REQ-16-006`'s acceptance evidence is what fixes it at four.
#[test]
fn the_four_strategy_names_are_introduced_as_examples() -> TestResult {
    let page = specification()?;
    let body = section(&page, "### 16.2 비용 벡터")?;

    for strategy in NAMED_STRATEGIES {
        assert!(
            body.contains(strategy.spec_token()),
            "section 16.2 does not write {}",
            strategy.spec_token()
        );
    }
    let last = NAMED_STRATEGIES[NAMED_STRATEGIES.len() - 1].spec_token();
    let after = body
        .split(last)
        .nth(1)
        .ok_or("section 16.2's strategy sentence is not shaped as expected")?;
    assert!(
        after.trim_start().starts_with("” 같은"),
        "section 16.2 no longer hedges the four names with 같은; \
         the recorded reading in docs/contracts/critical-path.md is stale"
    );
    assert!(STRATEGY_NAMES_ARE_EXAMPLES.contains("같은"));
    Ok(())
}

// ---------------------------------------------------------------------------
// `unknown_cost_is_a_range`
// ---------------------------------------------------------------------------

/// Section 16.2's `근거가 없으면 범위로 표시한다`, as a constructor refusal.
#[test]
fn unknown_cost_is_a_range() -> TestResult {
    // A basis that read nothing cannot be a point.
    assert!(matches!(
        CostEstimate::of(30, 30, Unit::Minutes, CostBasis::Unmeasured),
        Err(academic_critical_path::CriticalPathError::UnmeasuredEstimateIsAPoint)
    ));
    let ranged = CostEstimate::of(30, 90, Unit::Minutes, CostBasis::Unmeasured)?;
    assert!(ranged.is_range());
    assert!(ranged.basis().families().is_empty());

    // A basis that claims to be measured has to name a family.
    assert!(matches!(
        CostBasis::measured(&[]),
        Err(academic_critical_path::CriticalPathError::MeasuredBasisNamesNoFamily)
    ));

    // Interval addition widens and never narrows, and a measured estimate added
    // to an unmeasured one does not launder the basis.
    let measured_one = measured(CostComponent::LearningEffort, 10, 10)?;
    let summed = measured_one.plus(&ranged)?;
    assert_eq!(summed.low(), 40);
    assert_eq!(summed.high(), 100);
    assert!(
        !summed.basis().is_measured(),
        "an unmeasured half was laundered"
    );
    assert!(summed.is_range());

    // A whole plan whose one axis is unmeasured stays a range end to end, and
    // the disclosure names the empty basis rather than a number.
    let scenario = Scenario::new()?;
    let mut estimates = flat_estimates()?;
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: disk_page(),
            cost: cost_except(
                10,
                CostComponent::LearningEffort,
                unmeasured(CostComponent::LearningEffort, 30, 90)?,
            )?,
            benefit: flat_benefit(10)?,
            options: vec![reading_for(disk_page(), "dp")?],
        },
    );
    let request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    let result = plan(&request)?;
    let first = result.ranked().first().ok_or("no route survived")?;
    let axis = first
        .candidate()
        .cost()
        .component(CostComponent::LearningEffort);
    assert!(
        axis.is_range(),
        "an unmeasured axis collapsed to a point along the path"
    );
    assert!(!axis.basis().is_measured());

    let disclosed = result
        .disclosure()
        .cost_assumptions()
        .entries()
        .iter()
        .find(|entry| entry.axis == CostComponent::LearningEffort)
        .ok_or("the learning-effort assumption is not disclosed")?;
    assert!(
        disclosed.low < disclosed.high,
        "a disclosed unknown is a point"
    );
    assert!(disclosed.families.is_empty());

    // A measured estimate names which of section 16.2's four families it read,
    // and all four are reachable.
    assert_eq!(academic_critical_path::BASIS_FAMILIES.len(), 4);
    let page = specification()?;
    let body = section(&page, "### 16.2 비용 벡터")?;
    for family in academic_critical_path::BASIS_FAMILIES {
        assert!(
            body.contains(family.spec_token()),
            "section 16.2 does not name {}",
            family.spec_token()
        );
    }
    let partial = CostBasis::measured(&[BasisFamily::PastLearningSpeed])?;
    assert_eq!(partial.families(), &[BasisFamily::PastLearningSpeed]);
    Ok(())
}

// ---------------------------------------------------------------------------
// `course_is_an_acquisition_option`
// ---------------------------------------------------------------------------

/// Section 16.2's last sentence, and section 36.7's worked case.
#[test]
fn course_is_an_acquisition_option() -> TestResult {
    // A course bundles exposure and practice. One that bundles neither is being
    // modelled as an acquisition and is refused.
    let course = course_for(
        disk_page(),
        database_offering(),
        OfferingStatus::Confirmed,
        3,
        "db",
    )?;
    assert_eq!(course.as_str(), "COURSE");
    assert_eq!(
        course.supplies().len(),
        2,
        "a course bundles more than one occasion"
    );
    assert!(matches!(
        academic_critical_path::AcquisitionOption::course(
            database_offering(),
            OfferingStatus::Confirmed,
            academic_curriculum::Credits::new(3)?,
            vec![common::occasion(
                disk_page(),
                academic_critical_path::OpportunityKind::Exposure,
                "only-exposure",
            )],
        ),
        Err(academic_critical_path::CriticalPathError::CourseIsNotABundle)
    ));

    // Everything it hands out is an occasion. None of them is evidence, none
    // carries a mastery, and the option itself answers no state question.
    for opportunity in course.supplies() {
        assert_eq!(opportunity.concept(), disk_page());
        let encoded = serde_json::to_value(opportunity.clone())?;
        let fields: Vec<&String> = encoded
            .as_object()
            .ok_or("an opportunity is not an object")?
            .keys()
            .collect();
        assert_eq!(
            fields,
            vec!["concept", "kind", "source"],
            "an opportunity carries something other than an occasion"
        );
    }

    // A plan that takes the course still lists the concept as one the goal
    // needs. Taking it changed no state.
    let scenario = Scenario::new()?;
    let mut estimates = flat_estimates()?;
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: disk_page(),
            cost: flat_cost(10)?,
            benefit: flat_benefit(10)?,
            options: vec![
                course.clone(),
                reading_for(disk_page(), "dp")?,
                experiment_for(disk_page(), "dp")?,
            ],
        },
    );
    let request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    let result = plan(&request)?;
    let first = result.ranked().first().ok_or("no route survived")?;
    assert!(
        first.candidate().satisfying_set().holds(disk_page()),
        "taking a course removed the concept from the plan"
    );
    let step = first
        .candidate()
        .steps()
        .iter()
        .find(|step| step.concept() == disk_page())
        .ok_or("the plan has no step for the concept")?;
    assert_eq!(
        step.options().len(),
        3,
        "section 36.7's three ways of acquiring one concept"
    );
    assert_eq!(
        step.options()
            .iter()
            .filter(|option| option.offering().is_some())
            .count(),
        1,
        "a course is one option among several"
    );

    // The offering's own standing is read from `P2-U1` and never decided here.
    assert_eq!(course.offering_status(), Some(OfferingStatus::Confirmed));
    assert_eq!(course.credits(), 3);
    // Nothing that is not a course costs credits.
    assert_eq!(reading_for(disk_page(), "dp")?.credits(), 0);
    assert_eq!(experiment_for(disk_page(), "dp")?.offering(), None);
    Ok(())
}

// ---------------------------------------------------------------------------
// `eight_constraints_are_enforced`
// ---------------------------------------------------------------------------

/// Section 16.3's eight bullets, measured in both directions and each observed
/// refusing a route.
#[test]
fn eight_constraints_are_enforced() -> TestResult {
    let page = specification()?;
    let body = section(&page, "### 16.3 제약")?;
    let written = bullets(&body)?;

    assert_eq!(written.len(), 8, "section 16.3 does not have eight bullets");
    let declared: Vec<String> = CONSTRAINTS
        .iter()
        .map(|constraint| constraint.spec_bullet().to_owned())
        .collect();
    assert_eq!(
        written, declared,
        "the constraints and section 16.3's own bullets disagree"
    );

    // Every route carries all eight answers, always, in the design document's
    // own order.
    let scenario = Scenario::new()?;
    let baseline = plan(&scenario.request())?;
    for candidate in baseline.front().candidates() {
        let answers: Vec<Constraint> = candidate
            .constraints()
            .iter()
            .map(academic_critical_path::ConstraintFinding::constraint)
            .collect();
        assert_eq!(answers, CONSTRAINTS.to_vec(), "an answer is out of order");
        assert!(candidate.is_feasible());
    }

    // Each of the eight refuses or amends a route when its own input says so,
    // and refuses **only** its own: a fixture that violated two would not show
    // which one bit.
    let refusals: Vec<(Constraint, ConstraintVerdict)> = vec![
        (
            Constraint::HardPrerequisiteSatisfaction,
            ConstraintVerdict::Satisfied,
        ),
        (
            Constraint::OfferingStandingAndOfficialPrerequisite,
            ConstraintVerdict::Violated,
        ),
        (
            Constraint::TimetableAndCreditLimit,
            ConstraintVerdict::Violated,
        ),
        (Constraint::DeadlineOrHorizon, ConstraintVerdict::Violated),
        (
            Constraint::PrivacyExcludedResource,
            ConstraintVerdict::Violated,
        ),
        (Constraint::UserExclusion, ConstraintVerdict::Violated),
        (
            Constraint::StaleRefreshRequirement,
            ConstraintVerdict::SatisfiedWithInsertion,
        ),
        (
            Constraint::UncertainEdgeCheckpoint,
            ConstraintVerdict::SatisfiedWithInsertion,
        ),
    ];
    assert_eq!(refusals.len(), CONSTRAINTS.len());

    for (constraint, expected) in refusals {
        let (graph, estimates, constraints) = violating(constraint)?;
        let gap_case = section_36_4_gap()?;
        let slider = spec_order_slider()?;
        let request = academic_critical_path::PlanRequest {
            gap_case: &gap_case,
            graph: &graph,
            estimates: &estimates,
            constraints: &constraints,
            slider: &slider,
            rule_set_hash: rule_set(),
            engine_version: 1,
        };
        let result = plan(&request)?;

        if expected.admits() {
            // A constraint that admits leaves the route on the front, where its
            // own verdict is readable.
            let observed = result.front().candidates();
            assert!(
                !observed.is_empty(),
                "{} admits and produced no route",
                constraint.as_str()
            );
            assert!(
                observed
                    .iter()
                    .any(|candidate| candidate.verdict_of(constraint) == expected),
                "{} did not reach {} on its own fixture",
                constraint.as_str(),
                expected.as_str()
            );
        } else {
            // A constraint that refuses takes the route out of the front
            // entirely, so where it is visible is section 16.5's third
            // disclosure group -- which is the whole reason that group exists.
            assert!(
                result
                    .front()
                    .candidates()
                    .iter()
                    .all(|candidate| candidate.verdict_of(constraint).admits()),
                "{} refused a route and the route reached the front",
                constraint.as_str()
            );
            let excluded = result.disclosure().exclusions().routes();
            assert!(
                !excluded.is_empty(),
                "{} refused every route and none is disclosed",
                constraint.as_str()
            );
            assert!(
                excluded
                    .iter()
                    .any(|route| route.constraint == Some(constraint)),
                "{} refused a route and the disclosure names another constraint: {:?}",
                constraint.as_str(),
                excluded
                    .iter()
                    .map(|route| route.constraint)
                    .collect::<Vec<_>>()
            );
        }

        // Whatever the verdict, all eight were answered.
        for candidate in result.front().candidates() {
            assert_eq!(candidate.constraints().len(), CONSTRAINTS.len());
        }
    }

    // The baseline scenario refuses nothing, so a violating fixture's exclusion
    // is a measurement and not something every run produces.
    assert_eq!(
        baseline.disclosure().exclusions(),
        &Exclusions::NoneExcluded,
        "the baseline already excludes a route, so the fixtures above prove less"
    );
    Ok(())
}

type Fixture = (
    PrerequisiteHypergraph,
    Vec<ConceptEstimate>,
    academic_critical_path::ConstraintInputs,
);

/// A fixture that violates exactly one of section 16.3's eight.
fn violating(constraint: Constraint) -> Result<Fixture, Box<dyn Error>> {
    let mut graph = section_16_1_graph(&[])?;
    let mut estimates = flat_estimates()?;
    let mut inputs = permissive_constraints();
    match constraint {
        Constraint::HardPrerequisiteSatisfaction => {
            // A satisfiable graph is what this constraint answers `SATISFIED`
            // to; the plan itself is the remediation for an unmet prerequisite.
            inputs.hard_prerequisites_met = Vec::new();
        }
        Constraint::OfferingStandingAndOfficialPrerequisite => {
            estimates = with_estimate(
                estimates,
                ConceptEstimate {
                    concept: disk_page(),
                    cost: flat_cost(10)?,
                    benefit: flat_benefit(10)?,
                    options: vec![course_for(
                        disk_page(),
                        database_offering(),
                        OfferingStatus::Cancelled,
                        3,
                        "db",
                    )?],
                },
            );
        }
        Constraint::TimetableAndCreditLimit => {
            estimates = with_estimate(
                estimates,
                ConceptEstimate {
                    concept: disk_page(),
                    cost: flat_cost(10)?,
                    benefit: flat_benefit(10)?,
                    options: vec![course_for(
                        disk_page(),
                        database_offering(),
                        OfferingStatus::Confirmed,
                        3,
                        "db",
                    )?],
                },
            );
            inputs.offering_meetings = vec![(
                database_offering(),
                vec![Meeting::new(Weekday::Monday, 540, 630)?],
            )];
            inputs.committed_meetings = vec![Meeting::new(Weekday::Monday, 600, 690)?];
        }
        Constraint::DeadlineOrHorizon => {
            // The interval **straddles** the horizon: its low end fits and its
            // high end does not. A fixture whose two ends were equal would pass
            // an engine that read the low end, which is the false precision
            // section 16.2 refuses -- a route that fits only if every unknown
            // resolves favourably.
            estimates = with_estimate(
                estimates,
                ConceptEstimate {
                    concept: disk_page(),
                    cost: cost_except(
                        10,
                        CostComponent::CalendarDelay,
                        unmeasured(CostComponent::CalendarDelay, 1, 400)?,
                    )?,
                    benefit: flat_benefit(10)?,
                    options: vec![reading_for(disk_page(), "dp")?],
                },
            );
            inputs.horizon_days = 60;
        }
        Constraint::PrivacyExcludedResource => {
            inputs.privacy_excluded_sources = vec![evidence_id("flat-chapter")];
        }
        Constraint::UserExclusion => {
            inputs.user_excluded_concepts = all_concepts();
        }
        Constraint::StaleRefreshRequirement => {
            // One concept is `STALE` and one is `UNKNOWN`. The second is the
            // control: `P2-N3` says `UNKNOWN` is the band for a concept about
            // which nothing datable was ever admitted, so it is **not** stale
            // and must not draw a refresh -- and without it in the fixture, an
            // engine that read both bands as stale would look identical here.
            // `the_unknown_band_draws_no_refresh` reads the other half.
            inputs.bands = all_concepts()
                .into_iter()
                .map(|concept| {
                    (
                        concept,
                        if concept == disk_page() {
                            FreshnessBand::Stale
                        } else if concept == random_io() {
                            FreshnessBand::Unknown
                        } else {
                            FreshnessBand::High
                        },
                    )
                })
                .collect();
        }
        Constraint::UncertainEdgeCheckpoint => {
            graph = section_16_1_graph(&[
                (buffer_pool(), disk_page()),
                (buffer_pool(), random_io()),
                (disk_page(), storage_hierarchy()),
                (disk_page(), fan_out()),
                (disk_page(), page_layout()),
            ])?;
        }
    }
    Ok((graph, estimates, inputs))
}

/// `t068` names the eighth constraint and the checkpoint rule as two acceptance
/// rows. Section 16.3 has eight bullets and the last one *is* the checkpoint.
///
/// Recorded so a later reader does not add a ninth constraint.
#[test]
fn the_eighth_constraint_is_the_checkpoint_rule() -> TestResult {
    let page = specification()?;
    let written = bullets(&section(&page, "### 16.3 제약")?)?;
    let last = written.last().ok_or("section 16.3 has no bullets")?;
    assert!(
        last.contains("diagnostic checkpoint"),
        "section 16.3's last bullet is no longer the checkpoint rule; \
         the recorded reading in docs/contracts/critical-path.md is stale"
    );
    assert_eq!(
        CONSTRAINTS[CONSTRAINTS.len() - 1],
        Constraint::UncertainEdgeCheckpoint
    );
    assert_eq!(
        CONSTRAINTS[CONSTRAINTS.len() - 1].spec_bullet(),
        last.as_str()
    );
    // And there is no ninth.
    assert_eq!(written.len(), CONSTRAINTS.len());
    Ok(())
}

// ---------------------------------------------------------------------------
// `uncertain_edge_ratio_inserts_diagnostic_checkpoint`
// ---------------------------------------------------------------------------

/// Section 16.3's eighth bullet, on both sides of its threshold.
#[test]
fn uncertain_edge_ratio_inserts_diagnostic_checkpoint() -> TestResult {
    // Below the threshold: no checkpoint. The smaller branch's route has three
    // members and one of them uncertain, which is 333 permille.
    let settled = section_16_1_graph(&[])?;
    let settled_sets = satisfying_sets(&settled, buffer_pool())?;
    for set in &settled_sets {
        assert_eq!(uncertain_edge_ratio_permille(set), 0);
        assert_eq!(
            CheckpointDecision::for_ratio(uncertain_edge_ratio_permille(set)),
            CheckpointDecision::BelowThreshold
        );
    }

    let scenario = Scenario::new()?;
    let below = plan(&scenario.request())?;
    for path in below.ranked() {
        assert_eq!(
            path.candidate().checkpoint(),
            CheckpointDecision::BelowThreshold
        );
        assert_eq!(
            path.candidate()
                .verdict_of(Constraint::UncertainEdgeCheckpoint),
            ConstraintVerdict::Satisfied
        );
    }
    assert_eq!(
        below.disclosure().uncertain_edges(),
        &UncertainEdges::AllSettled
    );

    // Above it: every member uncertain is 1000 permille, and the checkpoint is
    // inserted before the plan's first step.
    let uncertain = section_16_1_graph(&[
        (buffer_pool(), disk_page()),
        (buffer_pool(), random_io()),
        (disk_page(), storage_hierarchy()),
        (disk_page(), fan_out()),
        (disk_page(), page_layout()),
    ])?;
    let gap_case = section_36_4_gap()?;
    let estimates = flat_estimates()?;
    let constraints = permissive_constraints();
    let slider = spec_order_slider()?;
    let request = academic_critical_path::PlanRequest {
        gap_case: &gap_case,
        graph: &uncertain,
        estimates: &estimates,
        constraints: &constraints,
        slider: &slider,
        rule_set_hash: rule_set(),
        engine_version: 1,
    };
    let above = plan(&request)?;
    let first = above.ranked().first().ok_or("no route survived")?;
    assert_eq!(
        uncertain_edge_ratio_permille(first.candidate().satisfying_set()),
        1000
    );
    assert_eq!(first.candidate().checkpoint(), CheckpointDecision::Insert);
    assert_eq!(
        first
            .candidate()
            .verdict_of(Constraint::UncertainEdgeCheckpoint),
        ConstraintVerdict::SatisfiedWithInsertion
    );
    let step = first
        .candidate()
        .steps()
        .first()
        .ok_or("the plan has no first step")?;
    assert!(
        matches!(
            step.required_before(),
            Some(RequiredInsertion::DiagnosticCheckpoint { ratio_permille })
                if *ratio_permille == 1000
        ),
        "the checkpoint is not before the first step"
    );

    // The threshold is strict: `넘을 때` and not `또는 같을 때`.
    assert_eq!(
        CheckpointDecision::for_ratio(UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE),
        CheckpointDecision::BelowThreshold
    );
    assert_eq!(
        CheckpointDecision::for_ratio(UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE + 1),
        CheckpointDecision::Insert
    );

    // A route with a settled and an uncertain member measures the fraction of
    // its own members, not of the whole graph.
    let mixed = section_16_1_graph(&[(disk_page(), storage_hierarchy())])?;
    let mixed_sets = satisfying_sets(&mixed, buffer_pool())?;
    let ratios: BTreeSet<u16> = mixed_sets
        .iter()
        .map(uncertain_edge_ratio_permille)
        .collect();
    assert!(
        ratios.len() >= 2,
        "the denominator is not the route's own members"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// `counterfactual_shows_edge_sensitivity`
// ---------------------------------------------------------------------------

/// Section 16.4's `이 edge가 틀리면 무엇이 바뀌는가`, recomputed.
#[test]
fn counterfactual_shows_edge_sensitivity() -> TestResult {
    let graph = section_16_1_graph(&[])?;
    let previews = sensitivity(&graph, buffer_pool())?;
    assert_eq!(
        previews.len(),
        graph.all_members().len(),
        "every relation gets a preview"
    );

    let outcome_for = |dependent: EntityId, prerequisite: EntityId| {
        previews
            .iter()
            .find(|preview| {
                preview.dependent() == dependent && preview.prerequisite() == prerequisite
            })
            .map(academic_critical_path::EdgeSensitivity::outcome)
    };

    // Removing a member of the conjunction removes the concept from every
    // route: the answer changes shape.
    assert_eq!(
        outcome_for(buffer_pool(), random_io()),
        Some(EdgeOutcome::RoutesLoseAConcept)
    );
    let random_io_preview = previews
        .iter()
        .find(|preview| preview.prerequisite() == random_io())
        .ok_or("no preview for the conjunction member")?;
    assert!(
        random_io_preview
            .concepts_no_longer_needed()
            .contains(&random_io())
    );

    // Removing the sole member of one branch collapses the disjunction to the
    // other branch. Nothing that *every* route needed stops being needed --
    // the surviving branch requires more, not less -- so the answer is that the
    // choice is gone rather than that a concept is: `FewerRoutes`, and the two
    // outcomes are kept apart precisely so that difference is visible.
    assert_eq!(
        outcome_for(disk_page(), storage_hierarchy()),
        Some(EdgeOutcome::FewerRoutes)
    );
    let branch = previews
        .iter()
        .find(|preview| preview.prerequisite() == storage_hierarchy())
        .ok_or("no preview for the branch member")?;
    assert_eq!(branch.routes_before(), 2);
    assert_eq!(branch.routes_after(), 1);
    assert!(
        branch.concepts_no_longer_needed().is_empty(),
        "collapsing a branch reported a concept as no longer needed"
    );

    // Removing the only member of the whole graph's root conjunction leaves the
    // goal satisfiable by itself, which is a different answer again.
    let single = PrerequisiteHypergraph::new().with(Hyperedge::requires_all(
        buffer_pool(),
        vec![member(
            buffer_pool(),
            disk_page(),
            EdgeStanding::Uncertain,
            "edge-buffer-pool-disk-page",
        )?],
    )?);
    let sole = sensitivity(&single, buffer_pool())?;
    assert_eq!(sole.len(), 1);
    assert_eq!(sole[0].outcome(), EdgeOutcome::RoutesLoseAConcept);
    assert_eq!(sole[0].routes_after(), 1);

    // The preview is computed and not described: the same solver runs on the
    // reduced graph, and its answer matches what the preview reports.
    for preview in &previews {
        let removed = graph
            .all_members()
            .into_iter()
            .find(|held| {
                held.dependent() == preview.dependent() && held.concept() == preview.prerequisite()
            })
            .ok_or("a preview names a relation the graph does not hold")?;
        let reduced = academic_critical_path::without(&graph, removed)?;
        assert_eq!(
            satisfying_sets(&reduced, buffer_pool())?.len(),
            preview.routes_after(),
            "a preview disagrees with the solver it claims to have run"
        );
    }

    // The original graph is untouched: a preview is not an edit.
    assert_eq!(graph, section_16_1_graph(&[])?);

    // Each uncertain relation's preview reaches the disclosure.
    let uncertain = section_16_1_graph(&[(buffer_pool(), random_io())])?;
    let gap_case = section_36_4_gap()?;
    let estimates = flat_estimates()?;
    let constraints = permissive_constraints();
    let slider = spec_order_slider()?;
    let result = plan(&academic_critical_path::PlanRequest {
        gap_case: &gap_case,
        graph: &uncertain,
        estimates: &estimates,
        constraints: &constraints,
        slider: &slider,
        rule_set_hash: rule_set(),
        engine_version: 1,
    })?;
    let disclosed = result.disclosure().uncertain_edges();
    assert_eq!(disclosed.edges().len(), 1);
    assert_eq!(disclosed.edges()[0].prerequisite, random_io());
    assert_eq!(
        disclosed.edges()[0].if_removed,
        EdgeOutcome::RoutesLoseAConcept
    );
    assert_eq!(disclosed.edges()[0].predicate, "REQUIRES");
    Ok(())
}

// ---------------------------------------------------------------------------
// `user_relation_edit_recomputes_and_preserves_base`
// ---------------------------------------------------------------------------

/// Section 16.4's `제거·추가해 다시 계산`, and section 34.5's `old path 보존`.
#[test]
fn user_relation_edit_recomputes_and_preserves_base() -> TestResult {
    let scenario = Scenario::new()?;
    let base = plan(&scenario.request())?;
    assert_eq!(base.ranked().len(), 2);
    let base_routes: Vec<Vec<EntityId>> = base
        .ranked()
        .iter()
        .map(|path| path.candidate().satisfying_set().concepts().to_vec())
        .collect();

    // The user says the `Random I/O` requirement is wrong.
    let removed = scenario
        .graph
        .all_members()
        .into_iter()
        .find(|member| member.concept() == random_io())
        .cloned()
        .ok_or("the fixture graph has no Random I/O member")?;
    let edit = RelationEdit::Remove { member: removed };
    let edited_graph = edited(&scenario.graph, &edit)?;
    let recomputed = plan(&academic_critical_path::PlanRequest {
        graph: &edited_graph,
        ..scenario.request()
    })?;

    let plan_one = EditedPlan::of(
        base.clone(),
        scenario.graph.clone(),
        edit,
        recomputed,
        edited_graph.clone(),
    )?;

    // The recomputation changed something.
    assert!(plan_one.changed(), "the edit changed nothing");
    for route in plan_one.recomputed().ranked() {
        assert!(
            !route.candidate().satisfying_set().holds(random_io()),
            "the removed relation still constrains the plan"
        );
    }

    // The base survived byte for byte.
    assert_eq!(plan_one.base(), &base);
    assert_eq!(plan_one.base_graph(), &scenario.graph);
    let preserved: Vec<Vec<EntityId>> = plan_one
        .base()
        .ranked()
        .iter()
        .map(|path| path.candidate().satisfying_set().concepts().to_vec())
        .collect();
    assert_eq!(preserved, base_routes, "the base was overwritten");

    // A **second** edit still answers with the original base, not the previous
    // recomputation. That is the append-only half.
    let second_member = edited_graph
        .all_members()
        .into_iter()
        .find(|member| member.concept() == page_layout())
        .cloned()
        .ok_or("the edited graph has no Page Layout member")?;
    let second = RelationEdit::Remove {
        member: second_member,
    };
    let twice_graph = edited(&edited_graph, &second)?;
    let twice = plan(&academic_critical_path::PlanRequest {
        graph: &twice_graph,
        ..scenario.request()
    })?;
    let plan_two = plan_one.apply(second, twice, twice_graph)?;

    assert_eq!(
        plan_two.edits().len(),
        2,
        "the edits are a list, not a pointer"
    );
    assert_eq!(plan_two.base(), &base, "the second edit moved the base");
    assert_eq!(plan_two.base_graph(), &scenario.graph);
    assert_ne!(plan_two.recomputed(), plan_one.recomputed());

    // Adding a relation is the other half of section 16.4's sentence.
    let addition = RelationEdit::Add {
        hyperedge: Hyperedge::requires_all(
            storage_hierarchy(),
            vec![member(
                storage_hierarchy(),
                page_layout(),
                EdgeStanding::Uncertain,
                "edge-storage-hierarchy-page-layout",
            )?],
        )?,
    };
    let added_graph = edited(&scenario.graph, &addition)?;
    let added = plan(&academic_critical_path::PlanRequest {
        graph: &added_graph,
        ..scenario.request()
    })?;
    let plan_three = EditedPlan::of(
        base.clone(),
        scenario.graph.clone(),
        addition,
        added,
        added_graph,
    )?;
    assert!(plan_three.changed());
    assert_eq!(plan_three.base(), &base);
    let with_addition = plan_three
        .recomputed()
        .ranked()
        .iter()
        .find(|path| path.candidate().satisfying_set().holds(storage_hierarchy()))
        .ok_or("the added relation's route is gone")?;
    assert!(
        with_addition
            .candidate()
            .satisfying_set()
            .holds(page_layout()),
        "the added relation did not reach the recomputation"
    );

    // An edit is a relation change, never a goal change.
    let other_goal = plan(&academic_critical_path::PlanRequest {
        graph: &scenario.graph,
        ..scenario.request()
    })?;
    assert_eq!(other_goal.goal(), base.goal());
    Ok(())
}

// ---------------------------------------------------------------------------
// `five_disclosure_groups_are_always_present`
// ---------------------------------------------------------------------------

/// Section 16.5's closing sentence, measured, and present on every answer this
/// engine can produce.
#[test]
fn five_disclosure_groups_are_always_present() -> TestResult {
    let page = specification()?;
    let body = section(&page, "### 16.5 출력의 한계")?;
    let sentence = body
        .lines()
        .find(|line| line.contains("항상 노출된다"))
        .ok_or("section 16.5 no longer states what is always disclosed")?;

    assert_eq!(DISCLOSURE_GROUPS.len(), 5);
    for group in DISCLOSURE_GROUPS {
        assert!(
            sentence.contains(group.spec_token()),
            "section 16.5's sentence does not name {}",
            group.spec_token()
        );
    }
    // The other direction: the sentence names nothing this crate does not hold.
    // Its list is comma-separated up to the verb.
    let listed: Vec<String> = sentence
        .split("이 항상 노출된다")
        .next()
        .unwrap_or_default()
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .split(',')
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect();
    assert_eq!(
        listed.len(),
        5,
        "section 16.5 lists a number other than five"
    );
    let declared: BTreeSet<&str> = DISCLOSURE_GROUPS
        .iter()
        .map(|group| group.spec_token())
        .collect();
    for item in &listed {
        assert!(
            declared.contains(item.as_str()),
            "section 16.5 names {item} and this crate does not hold it"
        );
    }

    // Every answer carries all five, including the ones where three of them are
    // legitimately empty and say so.
    let scenarios: Vec<academic_critical_path::CriticalPathResult> = vec![
        plan(&Scenario::new()?.request())?,
        sole_route_result()?,
        no_feasible_route_result()?,
    ];
    for result in &scenarios {
        let disclosure = result.disclosure();
        assert_eq!(disclosure.snapshot().goal, result.goal());
        assert!(
            !disclosure.cost_assumptions().entries().is_empty(),
            "the cost-assumption group is empty"
        );
        assert_eq!(
            disclosure.cost_assumptions().entries().len(),
            academic_critical_path::COST_COMPONENTS.len(),
            "one entry per cost axis"
        );
        // Each of the three that may be empty states its emptiness rather than
        // showing an ambiguous empty list.
        match disclosure.exclusions() {
            Exclusions::NoneExcluded => assert!(disclosure.exclusions().routes().is_empty()),
            Exclusions::Excluded { routes } => assert!(!routes.is_empty()),
        }
        match disclosure.uncertain_edges() {
            UncertainEdges::AllSettled => {
                assert!(disclosure.uncertain_edges().edges().is_empty());
            }
            UncertainEdges::Uncertain { edges, .. } => assert!(!edges.is_empty()),
        }
        match disclosure.alternatives() {
            academic_critical_path::Alternatives::NoFeasibleRoute => {
                assert!(result.ranked().is_empty());
            }
            academic_critical_path::Alternatives::SoleSurvivingRoute => {
                assert_eq!(result.ranked().len(), 1);
            }
            academic_critical_path::Alternatives::Routes { routes } => {
                assert_eq!(routes.len(), result.ranked().len() - 1);
            }
        }
        // A group that "has entries" is a narrower question than presence, and
        // the snapshot always has entries.
        assert!(
            disclosure
                .group_has_entries(academic_critical_path::DisclosureGroup::ComputationSnapshot)
        );
        assert!(
            disclosure.group_has_entries(academic_critical_path::DisclosureGroup::CostAssumptions)
        );
    }

    // The three cases really are three: an answer with alternatives, one with a
    // sole route and one with none.
    assert!(matches!(
        scenarios[0].disclosure().alternatives(),
        academic_critical_path::Alternatives::Routes { .. }
    ));
    assert!(matches!(
        scenarios[1].disclosure().alternatives(),
        academic_critical_path::Alternatives::SoleSurvivingRoute
    ));
    assert!(matches!(
        scenarios[2].disclosure().alternatives(),
        academic_critical_path::Alternatives::NoFeasibleRoute
    ));

    // Section 16.4's four roles are all reachable and all distinct.
    let roles: BTreeSet<PathRole> = scenarios[0].roles().iter().map(|(_, role)| *role).collect();
    assert!(roles.contains(&PathRole::SharedSpine));
    assert!(roles.contains(&PathRole::AlternativePath));
    assert_eq!(PATH_ROLES.len(), 4);
    let body_16_4 = section(&page, "### 16.4 여러 경로 표현")?;
    for role in PATH_ROLES {
        assert!(
            body_16_4.contains(role.spec_token()),
            "section 16.4 does not name {}",
            role.spec_token()
        );
    }
    Ok(())
}

/// A run where exactly one route survives the constraints.
fn sole_route_result() -> Result<academic_critical_path::CriticalPathResult, Box<dyn Error>> {
    let gap_case = section_36_4_gap()?;
    let graph = section_16_1_graph(&[])?;
    let estimates = flat_estimates()?;
    let mut constraints = permissive_constraints();
    constraints.user_excluded_concepts = vec![storage_hierarchy()];
    let slider = spec_order_slider()?;
    Ok(plan(&academic_critical_path::PlanRequest {
        gap_case: &gap_case,
        graph: &graph,
        estimates: &estimates,
        constraints: &constraints,
        slider: &slider,
        rule_set_hash: rule_set(),
        engine_version: 1,
    })?)
}

/// A run where every route is refused.
fn no_feasible_route_result() -> Result<academic_critical_path::CriticalPathResult, Box<dyn Error>>
{
    let gap_case = section_36_4_gap()?;
    let graph = section_16_1_graph(&[])?;
    let estimates = flat_estimates()?;
    let mut constraints = permissive_constraints();
    constraints.horizon_days = 0;
    let slider = spec_order_slider()?;
    Ok(plan(&academic_critical_path::PlanRequest {
        gap_case: &gap_case,
        graph: &graph,
        estimates: &estimates,
        constraints: &constraints,
        slider: &slider,
        rule_set_hash: rule_set(),
        engine_version: 1,
    })?)
}

// ---------------------------------------------------------------------------
// Beyond the thirteen.
// ---------------------------------------------------------------------------

/// The engine never borrows a neighbour's estimate.
///
/// `P2-N5` closed the misattribution one hop out from `P2-N3`, which closed it
/// one hop out from `P2-N2`. This engine's own boundary is the same shape: a
/// route that reaches a concept with no estimate is refused by name rather than
/// filled in from an adjacent concept.
#[test]
fn one_concepts_estimate_cannot_answer_for_another() -> TestResult {
    let scenario = Scenario::new()?;
    let estimates: Vec<ConceptEstimate> = flat_estimates()?
        .into_iter()
        .filter(|estimate| estimate.concept != page_layout())
        .collect();
    let request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    assert!(matches!(
        plan(&request),
        Err(academic_critical_path::CriticalPathError::NoEstimateForConcept(concept))
            if concept == page_layout()
    ));

    // And a concept with no way of acquiring it is refused rather than
    // scheduled with nothing to do.
    let mut estimates = flat_estimates()?;
    estimates = with_estimate(
        estimates,
        ConceptEstimate {
            concept: page_layout(),
            cost: flat_cost(10)?,
            benefit: flat_benefit(10)?,
            options: Vec::new(),
        },
    );
    let request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    assert!(matches!(
        plan(&request),
        Err(academic_critical_path::CriticalPathError::ConceptHasNoAcquisitionOption(concept))
            if concept == page_layout()
    ));
    Ok(())
}

/// A hyperedge cannot be built out of a predicate `P2-C4` refuses.
///
/// The allowlist is that crate's `prerequisite` column and there is none here:
/// eighteen of section 7.2's twenty predicates have no `PrerequisiteEdge` value
/// at all, so `RELATED_TO` cannot reach a hyperedge.
#[test]
fn a_hyperedge_member_is_a_traversable_predicate() -> TestResult {
    assert!(matches!(
        academic_gap::PrerequisiteEdge::admit(
            academic_domain::predicates::PredicateName::RelatedTo,
            academic_domain::predicates::PrerequisiteStrength::Hard,
            buffer_pool(),
            disk_page(),
            vec![evidence_id("edge")],
        ),
        Err(academic_gap::GapError::NotATraversablePredicate(_))
    ));

    // A member stated about another concept is refused.
    assert!(matches!(
        Hyperedge::requires_all(
            buffer_pool(),
            vec![member(
                disk_page(),
                storage_hierarchy(),
                EdgeStanding::Settled,
                "wrong-target",
            )?],
        ),
        Err(academic_critical_path::CriticalPathError::HyperedgeMemberLeavesTarget)
    ));

    // A `ONE OF` with one branch is a conjunction wearing the other name.
    assert!(matches!(
        Hyperedge::requires_one_of(
            buffer_pool(),
            vec![vec![member(
                buffer_pool(),
                disk_page(),
                EdgeStanding::Settled,
                "one-branch",
            )?]],
        ),
        Err(academic_critical_path::CriticalPathError::DisjunctionHasOneBranch)
    ));
    Ok(())
}

/// A plan is for the goal `P2-N5` found, and a plan with no gap behind it is
/// not a value this engine can be called with.
#[test]
fn a_plan_answers_a_gap_case() -> TestResult {
    let scenario = Scenario::new()?;
    let result = plan(&scenario.request())?;
    assert_eq!(result.goal(), scenario.gap_case.goal());
    assert_eq!(
        result.disclosure().snapshot().goal,
        scenario.gap_case.goal()
    );
    // The hypergraph is solved from the gap's own surface concept.
    for path in result.ranked() {
        assert!(
            path.candidate()
                .satisfying_set()
                .holds(scenario.gap_case.surface_concept())
        );
    }
    Ok(())
}

/// `P2-N3`'s `UNKNOWN` band is not §16.3's `stale`, and the refresh insertion
/// names only the concepts that are.
///
/// Without this the stale fixture would pass an engine that read both bands as
/// stale, because the two would be indistinguishable in its output.
#[test]
fn the_unknown_band_draws_no_refresh() -> TestResult {
    let scenario = Scenario::new()?;
    let mut constraints = permissive_constraints();
    constraints.bands = all_concepts()
        .into_iter()
        .map(|concept| {
            (
                concept,
                if concept == disk_page() {
                    FreshnessBand::Stale
                } else {
                    FreshnessBand::Unknown
                },
            )
        })
        .collect();
    let result = plan(&academic_critical_path::PlanRequest {
        constraints: &constraints,
        ..scenario.request()
    })?;
    let first = result.ranked().first().ok_or("no route survived")?;
    let finding = first
        .candidate()
        .constraints()
        .iter()
        .find(|entry| entry.constraint() == Constraint::StaleRefreshRequirement)
        .ok_or("the stale constraint was not answered")?;
    assert_eq!(finding.verdict(), ConstraintVerdict::SatisfiedWithInsertion);
    assert_eq!(
        finding.subjects(),
        &[disk_page()],
        "a band other than STALE drew a refresh"
    );
    assert!(matches!(
        finding.insertion(),
        Some(RequiredInsertion::MinimumRefresh { concepts }) if concepts == &[disk_page()]
    ));

    // Every band, one at a time, against `P2-N3`'s own six.
    for band in academic_freshness::BANDS {
        assert_eq!(
            academic_critical_path::is_stale(band),
            band == FreshnessBand::Stale,
            "{band:?} disagrees with section 16.3's one stale band"
        );
    }
    Ok(())
}

/// A candidate's steps and its satisfying set are checked against each other in
/// both directions.
///
/// Neither refusal is reachable through `plan`, which always builds the two
/// together, so both are observed here through the public constructor — which
/// is where a caller other than the engine would reach them. `P2-N5` found the
/// same shape in `RootCandidate::of` and recorded it as `N5-I17`.
#[test]
fn a_candidate_and_its_satisfying_set_must_agree() -> TestResult {
    let scenario = Scenario::new()?;
    let result = plan(&scenario.request())?;
    let candidate = result
        .front()
        .candidates()
        .first()
        .ok_or("no route survived")?;

    // A step outside the set.
    let mut wandering = candidate.steps().to_vec();
    wandering.push(academic_critical_path::PlanStep::of(
        entity("concept-not-on-this-route"),
        vec![reading_for(entity("concept-not-on-this-route"), "stray")?],
        None,
    ));
    assert!(matches!(
        academic_critical_path::Candidate::of(
            candidate.satisfying_set().clone(),
            wandering,
            candidate.cost().clone(),
            candidate.benefit().clone(),
            candidate.constraints().clone(),
            candidate.checkpoint(),
        ),
        Err(academic_critical_path::CriticalPathError::CandidateStepLeavesTheSet)
    ));

    // A set member with no step.
    let mut short = candidate.steps().to_vec();
    short.pop();
    assert!(matches!(
        academic_critical_path::Candidate::of(
            candidate.satisfying_set().clone(),
            short,
            candidate.cost().clone(),
            candidate.benefit().clone(),
            candidate.constraints().clone(),
            candidate.checkpoint(),
        ),
        Err(academic_critical_path::CriticalPathError::CandidateStepMissing)
    ));
    Ok(())
}

/// An `Unknown` constraint input is neither a pass nor a fail.
///
/// §28's graduation-audit invariant is `unknown을 pass/fail로 강제하지 않음`,
/// and this is the same refusal in this engine. The fixture is section 36.7's
/// own shape: an offering the registrar admits, whose *standing* is
/// `HISTORICALLY_LIKELY` rather than `CONFIRMED`. Every other constraint says
/// `SATISFIED`, so the route is refused for exactly one reason and the
/// disclosure names it.
///
/// Without this case, folding an unknown into whatever the other input said
/// would be invisible to every behavioural test: the `CANCELLED` fixture
/// already reaches `VIOLATED` through the other half of the same function.
#[test]
fn an_unknown_constraint_input_is_not_a_pass() -> TestResult {
    for (status, expected) in [
        (OfferingStatus::Confirmed, ConstraintVerdict::Satisfied),
        (
            OfferingStatus::HistoricallyLikely,
            ConstraintVerdict::Unknown,
        ),
        (OfferingStatus::Uncertain, ConstraintVerdict::Unknown),
        (OfferingStatus::Cancelled, ConstraintVerdict::Violated),
    ] {
        let gap_case = section_36_4_gap()?;
        let graph = section_16_1_graph(&[])?;
        let estimates = with_estimate(
            flat_estimates()?,
            ConceptEstimate {
                concept: disk_page(),
                cost: flat_cost(10)?,
                benefit: flat_benefit(10)?,
                options: vec![course_for(
                    disk_page(),
                    database_offering(),
                    status,
                    3,
                    "db",
                )?],
            },
        );
        let constraints = permissive_constraints();
        let slider = spec_order_slider()?;
        let result = plan(&academic_critical_path::PlanRequest {
            gap_case: &gap_case,
            graph: &graph,
            estimates: &estimates,
            constraints: &constraints,
            slider: &slider,
            rule_set_hash: rule_set(),
            engine_version: 1,
        })?;

        if expected.admits() {
            let first = result.ranked().first().ok_or("no route survived")?;
            assert_eq!(
                first
                    .candidate()
                    .verdict_of(Constraint::OfferingStandingAndOfficialPrerequisite),
                expected,
                "{status:?} reached the wrong verdict"
            );
        } else {
            // Both non-admitting verdicts take the route off the front, and the
            // disclosure is where they are told apart: an `Unknown` route is
            // `CONSTRAINT_UNKNOWN` and a refused one is `CONSTRAINT_VIOLATED`.
            let excluded = result.disclosure().exclusions().routes();
            assert!(!excluded.is_empty(), "{status:?} disclosed no exclusion");
            let reason = if expected == ConstraintVerdict::Unknown {
                academic_critical_path::ExclusionReason::ConstraintUnknown
            } else {
                academic_critical_path::ExclusionReason::ConstraintViolated
            };
            assert!(
                excluded.iter().any(|route| {
                    route.reason == reason
                        && route.constraint
                            == Some(Constraint::OfferingStandingAndOfficialPrerequisite)
                }),
                "{status:?} was disclosed as {:?} rather than {reason:?}",
                excluded
                    .iter()
                    .map(|route| route.reason)
                    .collect::<Vec<_>>()
            );
        }
    }

    // The registrar's own answer is the other input, and its `Unknown` is a
    // value too: §28's `OFFICIAL_PREREQUISITE` engine is `PLANNED`.
    let gap_case = section_36_4_gap()?;
    let graph = section_16_1_graph(&[])?;
    let estimates = with_estimate(
        flat_estimates()?,
        ConceptEstimate {
            concept: disk_page(),
            cost: flat_cost(10)?,
            benefit: flat_benefit(10)?,
            options: vec![course_for(
                disk_page(),
                database_offering(),
                OfferingStatus::Confirmed,
                3,
                "db",
            )?],
        },
    );
    let mut constraints = permissive_constraints();
    constraints.official_prerequisites = Vec::new();
    let slider = spec_order_slider()?;
    let result = plan(&academic_critical_path::PlanRequest {
        gap_case: &gap_case,
        graph: &graph,
        estimates: &estimates,
        constraints: &constraints,
        slider: &slider,
        rule_set_hash: rule_set(),
        engine_version: 1,
    })?;
    assert!(
        result.ranked().is_empty(),
        "a course whose registrar prerequisites nobody evaluated was recommended"
    );
    assert!(
        result
            .disclosure()
            .exclusions()
            .routes()
            .iter()
            .any(|route| {
                route.reason == academic_critical_path::ExclusionReason::ConstraintUnknown
            }),
        "an unevaluated registrar prerequisite was not disclosed as unknown"
    );
    Ok(())
}
