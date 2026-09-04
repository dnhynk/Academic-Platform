//! `P2-R6`'s acceptance suite: section 20's Build → Learn mode and section 21's
//! course ↔ project mapping.
//!
//! The nine tests `t068` names are the nine `#[test]` functions with its own
//! names. Everything else here is either a control for one of them — a fixture
//! shown to be non-empty, a scanner shown to bite on a sample that has the
//! shape — or one of section 21's own rules.
//!
//! ## Every count is read out of the design document
//!
//! `six_input_kinds_normalize` and `five_readiness_categories_map_exactly` parse
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` at run time and compare
//! it against the crate's enumerations in both directions. They fail if the
//! document stops saying what it says today, which is `P2-N3`'s and `P2-N6`'s
//! discipline and the answer to the six count mismatches this Run has found.

#[path = "support/mod.rs"]
mod support;

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_build_learn::{
    ActualCoverage, ArchitectureBranch, BranchGroup, CHECKPOINT_STAGES, ChannelComparison,
    ConceptRequirement, CourseProjectMapping, CoverageEvidenceKind, DesignedCoverage,
    EnrolmentStanding, EvidenceTask, GoalInput, INPUT_KINDS, InputKind, LearningItem,
    MAPPING_STATUSES, MOTIVATIONS, MappingEvidence, MappingStatus, Motivation, MotivationDisplay,
    NonEmptyText, ObservableCriterion, PartId, PersonalEvidenceStanding, PlanDefect, PlanDraft,
    PlanStep, ProjectGoal, READINESS_CATEGORIES, RESOLUTION_ORDER, ROW_WITHOUT_A_SHORT_NAME,
    ReadinessCategory, ReadinessFinding, RequirementCondition, RequirementOrigin,
    ResponsibilityDecomposition, SHORT_NAMES, SuccessCriteria, TechnologySlate,
    UnresolvedDecisions, categorize, normalize, validate,
};
use academic_domain::entity_registry::EntityKind;

use support::{
    TestResult, central_ordering, crdt_fundamentals, editor_branch, editor_constraints,
    editor_criteria, editor_decisions, editor_decomposition, editor_edges, editor_goal,
    editor_input, editor_responsibilities, experiment, failure_model, id, implementation, learning,
    ot_fundamentals, peer_merge, ready_state, realtime_collaboration, settled, shared_state,
    stale_state, text, thin_state, three_motivations,
};

// ---------------------------------------------------------------------------
// Reading the design document.
// ---------------------------------------------------------------------------

fn design_document() -> Result<String, Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .ok_or("the workspace root is not two levels above this crate")?
        .join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md");
    Ok(fs::read_to_string(root)?)
}

/// Section 20 alone, from its heading to the next one.
fn section_20() -> Result<String, Box<dyn Error>> {
    let document = design_document()?;
    let start = document
        .find("## 20. Build → Learn Mode")
        .ok_or("section 20's heading is not in the design document")?;
    let rest = &document[start..];
    let end = rest[3..]
        .find("\n## ")
        .map_or(rest.len(), |offset| offset + 3);
    Ok(rest[..end].to_owned())
}

/// Section 21 alone, from its heading to the next one.
fn section_21() -> Result<String, Box<dyn Error>> {
    let document = design_document()?;
    let start = document
        .find("## 21. SNU Course ↔ Project Mapping")
        .ok_or("section 21's heading is not in the design document")?;
    let rest = &document[start..];
    let end = rest[3..]
        .find("\n## ")
        .map_or(rest.len(), |offset| offset + 3);
    Ok(rest[..end].to_owned())
}

/// The `범주` column of section 20.2's result table, in row order.
fn category_rows(section: &str) -> Vec<String> {
    section
        .lines()
        .filter(|line| line.starts_with('|'))
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            (cells.len() == 3).then(|| cells[0].to_owned())
        })
        .filter(|cell| !cell.is_empty() && !cell.starts_with("---") && cell != "범주")
        .collect()
}

/// The `뜻` column of the same table, in the same order.
fn meaning_rows(section: &str) -> Vec<String> {
    section
        .lines()
        .filter(|line| line.starts_with('|'))
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            (cells.len() == 3).then(|| cells[1].to_owned())
        })
        .filter(|cell| !cell.is_empty() && !cell.starts_with("---") && cell != "뜻")
        .collect()
}

// ---------------------------------------------------------------------------
// 1. `six_input_kinds_normalize`
// ---------------------------------------------------------------------------

/// Six is a measurement of section 20.1's own sentence, both ways.
#[test]
fn six_input_kinds_normalize() -> TestResult {
    let section = section_20()?;

    // The sentence, parsed rather than transcribed: everything between
    // `사용자는 ` and ` 중 하나를 입력한다`, split on the list's own comma.
    let sentence = section
        .lines()
        .find(|line| line.contains("중 하나를 입력한다"))
        .ok_or("section 20.1's input sentence is not in the design document")?;
    let start = sentence
        .find("사용자는 ")
        .ok_or("the sentence does not begin the way it did")?
        + "사용자는 ".chars().count()
        + "사용자는 ".len()
        - "사용자는 ".chars().count();
    let listed: Vec<String> = sentence[start..]
        .split(" 중 하나를 입력한다")
        .next()
        .unwrap_or_default()
        .split(", ")
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();

    let declared: Vec<String> = INPUT_KINDS
        .iter()
        .map(|kind| kind.spec_token().to_owned())
        .collect();
    assert_eq!(
        listed, declared,
        "the design document's input list and INPUT_KINDS disagree"
    );

    // The scanner is not vacuous: it found a list, and the list is the length
    // the enumeration is, so neither side is empty.
    assert_eq!(listed.len(), 6, "the parsed list is {listed:?}");

    // And every one of the six normalises, retaining its source kind. One value
    // per kind, built through the public constructor, so a seventh variant added
    // later has no arm here and fails to compile.
    let inputs = [
        editor_input()?,
        GoalInput::ProjectGoalDocument {
            text: text("a goal document the user already wrote")?,
        },
        GoalInput::InitialSpec {
            title: text("the collaborative editor specification")?,
            statements: vec![text("edits are ordered")?],
        },
        GoalInput::EmptyRepository {
            snapshot_id: text("snap_empty")?,
            intended: text("the editor, from nothing")?,
        },
        GoalInput::InProgressRepository {
            snapshot_id: text("snap_abc1234")?,
            wanted: text("offline editing on top of what is there")?,
        },
        GoalInput::ArchitectureIdea {
            sketch: text("one ordering service, many thin clients")?,
        },
    ];
    let mut seen = Vec::new();
    for input in &inputs {
        let intent = normalize(input)?;
        assert_eq!(
            intent.source(),
            input.kind(),
            "normalisation dropped the source kind"
        );
        assert!(
            !intent.capability().as_str().is_empty(),
            "normalisation produced an empty capability"
        );
        seen.push(intent.source());
    }
    assert_eq!(
        seen,
        INPUT_KINDS.to_vec(),
        "one input kind was not exercised"
    );

    // The two repository kinds retain the snapshot they were taken over, and the
    // four others have none to retain. Stated over the whole set rather than for
    // the two, so a seventh kind has to answer it.
    for input in &inputs {
        let intent = normalize(input)?;
        assert_eq!(
            intent.snapshot_id().is_some(),
            matches!(
                input.kind(),
                InputKind::EmptyRepository | InputKind::InProgressRepository
            ),
            "{} disagrees with its input about a snapshot",
            input.kind().as_str()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. `criteria_and_choices_precede_technology`
// ---------------------------------------------------------------------------

/// The technology list is downstream of the criteria, and that is the type.
#[test]
fn criteria_and_choices_precede_technology() -> TestResult {
    // The first link: an empty criteria list has no value at all.
    assert!(
        SuccessCriteria::of(Vec::new()).is_none(),
        "an empty success-criteria list produced a value"
    );

    // The second: the slate is derived from the goal's own open decisions, and
    // every entry names the decision it hangs off. There is no entry that is
    // not conditional, because there is no other source.
    let goal = editor_goal()?;
    let slate = TechnologySlate::under(&goal);
    let named: Vec<(String, String)> = slate
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.decision().as_str().to_owned(),
                entry.name().as_str().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        named,
        vec![
            ("ordering".to_owned(), "central ordering".to_owned()),
            ("ordering".to_owned(), "peer/offline merge".to_owned()),
            ("merge-algorithm".to_owned(), "OT".to_owned()),
            ("merge-algorithm".to_owned(), "CRDT".to_owned()),
        ],
        "the slate is not the goal's own alternatives"
    );
    for entry in slate.entries() {
        let decision = goal
            .unresolved_decisions()
            .decision(entry.decision())
            .ok_or("a slate entry names a decision the goal does not hold")?;
        assert!(
            decision.alternative(entry.alternative()).is_some(),
            "a slate entry names an alternative the decision does not offer"
        );
    }

    // The third: with no open decision there is no technology list at all. This
    // is the half that would be vacuous if the slate had a second producer, and
    // `the_only_producer_of_a_technology_slate_takes_a_goal` in the scan suite
    // compares the whole set of producers against one.
    let input = editor_input()?;
    let intent = normalize(&input)?;
    let no_choices = ProjectGoal::state(
        &intent,
        editor_criteria()?,
        editor_constraints()?,
        UnresolvedDecisions::of(Vec::new()),
    )?;
    assert!(
        TechnologySlate::under(&no_choices).is_empty(),
        "a goal with no open decision produced a technology list"
    );

    // The control: the same goal with its decisions produces four entries, so
    // the emptiness above is a property of the goal and not of the reader.
    assert_eq!(slate.entries().len(), 4);
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `goal_schema_separates_four_groups`
// ---------------------------------------------------------------------------

/// The four groups are four keys, four types, and no route between two of them.
#[test]
fn goal_schema_separates_four_groups() -> TestResult {
    let section = section_20()?;

    // The four keys, parsed out of section 20.1's own YAML block.
    let block = section
        .split("```yaml")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .ok_or("section 20.1's ProjectGoal block is not in the design document")?;
    let keys: Vec<String> = block
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("    "))
        .filter_map(|line| line.trim().split(':').next().map(str::to_owned))
        .filter(|key| !key.is_empty())
        .collect();
    assert_eq!(
        keys,
        vec![
            "text",
            "successCriteria",
            "constraints",
            "unresolvedDecisions"
        ],
        "section 20.1's ProjectGoal keys changed"
    );

    // The serialized document's key set is exactly those four plus the source
    // kind normalisation retains, compared as a whole set in both directions.
    let goal = editor_goal()?;
    let wire = serde_json::to_value(&goal)?;
    let object = wire
        .as_object()
        .ok_or("a goal did not serialize as an object")?;
    let found: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    assert_eq!(
        found,
        [
            "source",
            "text",
            "success_criteria",
            "constraints",
            "unresolved_decisions"
        ]
        .into_iter()
        .collect::<BTreeSet<&str>>(),
        "the goal's serialized key set changed"
    );

    // The groups hold different things and none is empty, so no assertion below
    // is about an absent group.
    assert_eq!(goal.success_criteria().criteria().len(), 3);
    assert_eq!(goal.constraints().constraints().len(), 2);
    assert_eq!(goal.unresolved_decisions().decisions().len(), 2);

    // The separation that matters: an unresolved decision cannot be serialized
    // as a constraint. A constraint's serialized key set has no key an
    // alternative list could arrive under, and the two sets are disjoint on
    // exactly that key.
    let constraint = serde_json::to_value(&goal.constraints().constraints()[0])?;
    let decision = serde_json::to_value(&goal.unresolved_decisions().decisions()[0])?;
    let constraint_keys: BTreeSet<&str> = constraint
        .as_object()
        .ok_or("a constraint did not serialize as an object")?
        .keys()
        .map(String::as_str)
        .collect();
    let decision_keys: BTreeSet<&str> = decision
        .as_object()
        .ok_or("a decision did not serialize as an object")?
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        constraint_keys,
        ["id", "statement"].into_iter().collect::<BTreeSet<&str>>()
    );
    assert_eq!(
        decision_keys,
        ["id", "question", "alternatives"]
            .into_iter()
            .collect::<BTreeSet<&str>>()
    );
    assert!(
        !constraint_keys.contains("alternatives"),
        "a constraint has a key an alternative list could arrive under"
    );

    // And the four groups survive a round trip separately: a document whose
    // groups were merged would not come back equal.
    let back: ProjectGoal = serde_json::from_value(wire)?;
    assert_eq!(back, goal, "the goal did not round trip");

    // The observability half. A criterion states what would be watched, and a
    // criteria list with none of them is not a value: both fields are
    // `NonEmptyText`, so the refusal is at construction.
    assert!(
        academic_build_learn::NonEmptyText::new("   ").is_err(),
        "blank text produced a value"
    );
    for criterion in goal.success_criteria().criteria() {
        assert!(!criterion.observed_by().as_str().trim().is_empty());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. `responsibilities_precede_architecture_branch`
// ---------------------------------------------------------------------------

/// The branch is derived from a decomposition it owns, and from nothing else.
#[test]
fn responsibilities_precede_architecture_branch() -> TestResult {
    let branch = editor_branch()?;

    // The decomposition the branch holds is the one the goal was decomposed
    // into: the branch cannot have been built before it existed, because it
    // took it by value.
    assert_eq!(
        branch.decomposition().responsibilities().len(),
        editor_responsibilities()?.len()
    );
    assert_eq!(branch.goal(), &editor_goal()?);

    // Every criterion of the goal is served, and every responsibility serves one
    // the goal holds. The second half is refused at construction.
    let served: BTreeSet<&str> = branch
        .decomposition()
        .responsibilities()
        .iter()
        .map(|item| item.serves().as_str())
        .collect();
    let stated: BTreeSet<&str> = branch
        .goal()
        .success_criteria()
        .criteria()
        .iter()
        .map(|item| item.id().as_str())
        .collect();
    assert_eq!(served, stated, "the decomposition and the goal disagree");

    // A criterion nobody serves is refused, and the refusal names it. Built by
    // adding a criterion rather than by removing a responsibility, so the
    // control below is about the same decomposition.
    let input = editor_input()?;
    let intent = normalize(&input)?;
    let mut criteria: Vec<ObservableCriterion> = editor_criteria()?.into();
    criteria.push(ObservableCriterion::state(
        id("offline")?,
        text("an offline client rejoins without a manual merge")?,
        text("a client is disconnected for a day and rejoins")?,
    ));
    let wider = ProjectGoal::state(
        &intent,
        SuccessCriteria::of(criteria).ok_or("the widened criteria were empty")?,
        editor_constraints()?,
        editor_decisions()?,
    )?;
    let refused = ResponsibilityDecomposition::decompose(wider, editor_responsibilities()?);
    assert!(
        matches!(
            refused,
            Err(academic_build_learn::BuildLearnError::CriterionHasNoResponsibility(ref name))
                if name == "offline"
        ),
        "an unserved criterion was admitted: {refused:?}"
    );

    // A requirement naming a responsibility the decomposition does not hold is
    // refused too, which is what keeps the arrow one-directional: the branch
    // cannot introduce a responsibility of its own.
    let stray = ArchitectureBranch::of(
        editor_decomposition()?,
        realtime_collaboration(),
        vec![ConceptRequirement::always(
            shared_state(),
            EntityKind::Concept,
            id("not-a-responsibility")?,
        )?],
        Vec::new(),
    );
    assert!(
        matches!(
            stray,
            Err(academic_build_learn::BuildLearnError::RequirementServesNoResponsibility { .. })
        ),
        "a requirement serving nothing was admitted: {stray:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. `and_or_branches_are_conditional`
// ---------------------------------------------------------------------------

/// Every `OR` member is conditional by construction, and the answer is a set.
#[test]
fn and_or_branches_are_conditional() -> TestResult {
    let branch = editor_branch()?;

    // Both halves are non-empty first, so neither assertion below is vacuous.
    assert_eq!(branch.conjunction().len(), 2);
    assert_eq!(branch.disjunctions().len(), 2);
    assert_eq!(branch.requirements().len(), 6);

    // The whole set of `OR` members is conditional, and each names the decision
    // and alternative its group is. Compared as a set rather than checked per
    // member, so a member added under any name has to be in it.
    let conditional: BTreeSet<(String, String)> = branch
        .disjunctions()
        .iter()
        .flatten()
        .flat_map(|group| group.members())
        .map(|requirement| match requirement.condition() {
            RequirementCondition::Conditional {
                decision,
                alternative,
            } => (
                decision.as_str().to_owned(),
                alternative.as_str().to_owned(),
            ),
            RequirementCondition::Unconditional => {
                unreachable!("an OR member is unconditional")
            }
        })
        .collect();
    assert_eq!(
        conditional,
        [
            ("ordering".to_owned(), "central".to_owned()),
            ("ordering".to_owned(), "peer".to_owned()),
            ("merge-algorithm".to_owned(), "ot".to_owned()),
            ("merge-algorithm".to_owned(), "crdt".to_owned()),
        ]
        .into_iter()
        .collect::<BTreeSet<(String, String)>>(),
        "the OR members' conditions changed"
    );

    // And the whole set of `AND` members is unconditional.
    for requirement in branch.conjunction() {
        assert_eq!(
            requirement.condition(),
            &RequirementCondition::Unconditional,
            "an AND member is conditional"
        );
    }

    // A branch naming a decision the goal did not leave open is refused, and so
    // is one naming an alternative that decision does not offer. Neither is a
    // choice the user stated.
    let invented = BranchGroup::of(
        id("not-a-decision")?,
        id("central")?,
        vec![(central_ordering(), EntityKind::Concept, id("merge-order")?)],
    )?;
    let refused = ArchitectureBranch::of(
        editor_decomposition()?,
        realtime_collaboration(),
        Vec::new(),
        vec![invented],
    );
    assert!(
        matches!(
            refused,
            Err(academic_build_learn::BuildLearnError::BranchNamesNoDecision(ref name))
                if name == "not-a-decision"
        ),
        "an invented decision was admitted: {refused:?}"
    );

    // A decision the goal left open with only one branch is refused, for the
    // reason `P2-N6`'s `requires_one_of` gives: a `ONE OF` with one branch is a
    // conjunction wearing the other shape's name.
    let one_sided = ArchitectureBranch::of(
        editor_decomposition()?,
        realtime_collaboration(),
        Vec::new(),
        vec![BranchGroup::of(
            id("ordering")?,
            id("central")?,
            vec![(central_ordering(), EntityKind::Concept, id("merge-order")?)],
        )?],
    );
    assert!(
        matches!(
            one_sided,
            Err(academic_build_learn::BuildLearnError::DecisionHasOneBranch(
                _
            ))
        ),
        "a one-branch disjunction was admitted: {one_sided:?}"
    );

    // The structure is section 16.1's, and the answer over it is satisfaction.
    // One conjunction plus one disjunction per open decision.
    let edges = editor_edges()?;
    let hypergraph = branch.hypergraph(&edges, settled())?;
    assert_eq!(hypergraph.len(), 3, "the hyperedge shapes changed");
    assert!(matches!(
        hypergraph[0],
        academic_critical_path::Hyperedge::RequiresAll { .. }
    ));
    assert!(matches!(
        hypergraph[1],
        academic_critical_path::Hyperedge::RequiresOneOf { .. }
    ));
    assert!(matches!(
        hypergraph[2],
        academic_critical_path::Hyperedge::RequiresOneOf { .. }
    ));

    // Four satisfying sets — one per combination of the two decisions — each
    // holding both mandatory members and one member of each branch. That is the
    // answer a shortest path does not give: a set, not a walk.
    let sets = branch.satisfying_sets(&edges, settled())?;
    assert_eq!(sets.len(), 4, "the satisfying sets changed");
    for set in &sets {
        let concepts: BTreeSet<_> = set.concepts().iter().copied().collect();
        assert!(concepts.contains(&shared_state()));
        assert!(concepts.contains(&failure_model()));
        assert_eq!(
            concepts
                .iter()
                .filter(|concept| **concept == central_ordering() || **concept == peer_merge())
                .count(),
            1,
            "a satisfying set took both ordering branches or neither"
        );
        assert_eq!(
            concepts
                .iter()
                .filter(|concept| {
                    **concept == ot_fundamentals() || **concept == crdt_fundamentals()
                })
                .count(),
            1,
            "a satisfying set took both algorithm branches or neither"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. `five_readiness_categories_map_exactly`
// ---------------------------------------------------------------------------

/// The drawing's five map onto five of the table's six, and the sixth is named.
#[test]
fn five_readiness_categories_map_exactly() -> TestResult {
    let section = section_20()?;

    // The drawing's line, parsed. Five names.
    let drawn = section
        .lines()
        .find(|line| line.contains("later-scale"))
        .ok_or("the reverse-path drawing's readiness line is not in the design document")?;
    let short: Vec<String> = drawn
        .split('/')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        short,
        SHORT_NAMES
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect::<Vec<String>>(),
        "the drawing's readiness names and SHORT_NAMES disagree"
    );

    // The table's rows, parsed. Six names and six meanings.
    let rows = category_rows(&section);
    assert_eq!(
        rows,
        READINESS_CATEGORIES
            .iter()
            .map(|category| category.spec_token().to_owned())
            .collect::<Vec<String>>(),
        "the table's rows and READINESS_CATEGORIES disagree"
    );
    let meanings = meaning_rows(&section);
    assert_eq!(
        meanings,
        READINESS_CATEGORIES
            .iter()
            .map(|category| category.meaning_token().to_owned())
            .collect::<Vec<String>>(),
        "the table's meanings and READINESS_CATEGORIES disagree"
    );

    // The mapping: an order-preserving injection of the five into the six, with
    // the one row the drawing does not name identified. Both counts are read
    // above, so neither five nor six is written here.
    let mapped: Vec<String> = SHORT_NAMES
        .iter()
        .map(|(_, category)| category.spec_token().to_owned())
        .collect();
    let positions: Vec<usize> = mapped
        .iter()
        .map(|token| {
            rows.iter()
                .position(|row| row == token)
                .unwrap_or_else(|| unreachable!("{token} is not a table row"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "the drawing's five are not in the table's order: {positions:?}"
    );
    let residue: Vec<&String> = rows.iter().filter(|row| !mapped.contains(row)).collect();
    assert_eq!(
        residue,
        vec![&ROW_WITHOUT_A_SHORT_NAME.spec_token().to_owned()],
        "the row the drawing does not name changed"
    );
    assert_eq!(short.len() + residue.len(), rows.len());

    // `short_name` agrees with the pairing in both directions, over the whole
    // enumeration rather than over the five.
    for category in READINESS_CATEGORIES {
        assert_eq!(
            category.short_name().is_some(),
            category != ROW_WITHOUT_A_SHORT_NAME,
            "{} disagrees about having a short name",
            category.as_str()
        );
    }

    // And the rule lands on each of the six, over real `P2-N5` overlays. Every
    // row is reached, so no arm of `categorize` is unexercised.
    let branch = editor_branch()?;
    let conjunction: Vec<&ConceptRequirement> = branch.conjunction().iter().collect();
    let conditional = branch.disjunctions()[1][1].members();

    let ready = categorize(
        conjunction[0],
        RequirementOrigin::DefinesSuccessCriterion {
            criterion: id("converge")?,
        },
        &ready_state(shared_state())?,
    );
    assert_eq!(ready.category(), ReadinessCategory::AlreadyReady);

    let refresh = categorize(
        conjunction[0],
        RequirementOrigin::DefinesSuccessCriterion {
            criterion: id("converge")?,
        },
        &stale_state(shared_state())?,
    );
    assert_eq!(refresh.category(), ReadinessCategory::RefreshNeeded);

    let weak = categorize(
        conjunction[1],
        RequirementOrigin::PrerequisiteNeighbour {
            of_concept: id("merge-order")?,
        },
        &thin_state(failure_model())?,
    );
    assert_eq!(weak.category(), ReadinessCategory::CurrentlyWeak);

    let direct = categorize(
        conjunction[1],
        RequirementOrigin::DefinesSuccessCriterion {
            criterion: id("reconnect")?,
        },
        &thin_state(failure_model())?,
    );
    assert_eq!(
        direct.category(),
        ReadinessCategory::DirectImplementationNeed
    );

    let choice = categorize(
        &conditional[0],
        RequirementOrigin::DefinesSuccessCriterion {
            criterion: id("converge")?,
        },
        &thin_state(crdt_fundamentals())?,
    );
    assert_eq!(choice.category(), ReadinessCategory::ConditionalOnChoice);

    let later = categorize(
        conjunction[0],
        RequirementOrigin::BenefitTrigger(Box::new(support_benefit()?)),
        &thin_state(shared_state())?,
    );
    assert_eq!(later.category(), ReadinessCategory::LaterScale);

    let reached: BTreeSet<ReadinessCategory> = [&ready, &refresh, &weak, &direct, &choice, &later]
        .iter()
        .map(|finding| finding.category())
        .collect();
    assert_eq!(
        reached,
        READINESS_CATEGORIES.into_iter().collect::<BTreeSet<_>>(),
        "a readiness row was never reached"
    );

    // The resolution order is what makes the six mutually exclusive, and it is
    // pinned as data rather than described. Its image is the whole enumeration.
    assert_eq!(
        RESOLUTION_ORDER
            .iter()
            .map(|(_, category)| *category)
            .collect::<BTreeSet<_>>(),
        READINESS_CATEGORIES.into_iter().collect::<BTreeSet<_>>(),
        "the resolution order does not cover the six rows"
    );

    // The overlays are read and not recomputed: the finding carries `P2-N2`'s
    // rung and `P2-N3`'s band unchanged.
    let state = stale_state(shared_state())?;
    assert_eq!(refresh.mastery(), state.mastery());
    assert_eq!(refresh.freshness(), state.freshness());
    assert_eq!(
        refresh.sufficiency_gap_count(),
        state.sufficiency_gaps().len()
    );
    Ok(())
}

/// A `P2-R4` benefit contract, built through that crate's own builder.
fn support_benefit() -> Result<academic_repository_classification::BenefitContract, Box<dyn Error>>
{
    use academic_repository_classification::{
        BenefitDimension, BenefitDraft, TradeOff, Trigger, TriggerState,
    };
    let subject = academic_repository_analysis::SubjectId::new("replication")?;
    Ok(BenefitDraft::new()
        .with_concept(&subject)
        .with_triggers(vec![Trigger::new("second-region-deployed")?])
        .with_state(TriggerState::NotMet)
        .with_benefit(BenefitDimension::Resilience)
        .with_tradeoffs(vec![TradeOff::new("cross-region-write-latency")?])
        .seal()?)
}

// ---------------------------------------------------------------------------
// 7. `learning_item_requires_evidence_task_and_checkpoint`
// ---------------------------------------------------------------------------

/// Both parts arrive by value, and the checkpoint's four stages are ordered.
#[test]
fn learning_item_requires_evidence_task_and_checkpoint() -> TestResult {
    let section = section_20()?;

    // The four-stage example, parsed out of section 20.2's own sentence.
    let example = section
        .lines()
        .find(|line| line.contains("선택 승인"))
        .ok_or("section 20.2's checkpoint example is not in the design document")?;
    for stage in CHECKPOINT_STAGES {
        assert!(
            example.contains(stage.spec_token()),
            "the example no longer names {}",
            stage.as_str()
        );
    }
    // In the example's own order, and each after the one before.
    let positions: Vec<usize> = CHECKPOINT_STAGES
        .iter()
        .map(|stage| {
            example
                .find(stage.spec_token())
                .unwrap_or_else(|| unreachable!("{} is not in the example", stage.as_str()))
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "the four stages are not in the example's order: {positions:?}"
    );

    // The item carries both, and the item holds what it was given.
    let item = learning("learn-crdt", crdt_fundamentals(), "build-merge")?;
    assert_eq!(item.concept(), crdt_fundamentals());
    assert!(!item.evidence_task().runs().as_str().is_empty());
    assert!(!item.evidence_task().shows().as_str().is_empty());
    assert_eq!(item.checkpoint().returns_to().as_str(), "build-merge");

    // The four stages are reachable only through the chain, and the value at the
    // end names all four. The compile-fail suite holds the other half: a
    // `SelectionApproved` built without a `SimulationPassed` does not compile.
    assert_eq!(item.checkpoint().approved().stages(), CHECKPOINT_STAGES);
    let approved = item.checkpoint().approved();
    assert!(
        !approved
            .simulation()
            .explanation()
            .reading()
            .source()
            .as_str()
            .is_empty(),
        "the chain lost the reading it started from"
    );

    // The scan suite compares the whole set of public functions returning a
    // `LearningItem` against exactly one. Here the fixture shows that one
    // producer exists and that it takes both, so that comparison is not over an
    // empty set.
    let task = EvidenceTask::of(text("run the harness")?, text("that both orders agree")?);
    let second = LearningItem::plan(
        id("learn-ot")?,
        ot_fundamentals(),
        task,
        item.checkpoint().clone(),
    );
    assert_ne!(second.id(), item.id());
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. `lecture_list_only_plan_fails_validation`
// ---------------------------------------------------------------------------

/// A plan that only studies is refused, and the same plan with a build is not.
#[test]
fn lecture_list_only_plan_fails_validation() -> TestResult {
    let branch = editor_branch()?;
    let findings = every_requirement_needs_acquisition(&branch)?;

    // The plan section 20.2 refuses: six well-formed learning items, each with a
    // real evidence task and a real four-stage checkpoint, and nothing built.
    // Every checkpoint returns to another learning step, so the dangling-target
    // guard is *not* what refuses it — the three that do are the three that are
    // about building.
    let mut only_learning = Vec::new();
    for (index, requirement) in branch.requirements().iter().enumerate() {
        only_learning.push(PlanStep::Learning(learning(
            &format!("study-{index}"),
            requirement.concept(),
            "study-0",
        )?));
    }
    let draft = PlanDraft {
        branch: &branch,
        findings: &findings,
        steps: &only_learning,
        motivations: &[],
    };
    let verdict = validate(&draft);
    assert!(!verdict.is_accepted(), "a study-only plan was accepted");
    let kinds: BTreeSet<&str> = verdict.defects().iter().map(PlanDefect::as_str).collect();
    assert!(kinds.contains("NO_IMPLEMENTATION_STEP"));
    assert!(kinds.contains("CRITERION_REACHED_BY_NO_IMPLEMENTATION"));
    assert!(kinds.contains("CHECKPOINT_RETURNS_TO_NON_IMPLEMENTATION"));
    assert!(
        !kinds.contains("CHECKPOINT_RETURNS_TO_NO_STEP"),
        "the fixture's checkpoints dangle, so the three above are not what refused it"
    );

    // Every criterion of the goal is named, not just the first: a validator that
    // stopped at one would report one.
    let unreached: BTreeSet<String> = verdict
        .defects()
        .iter()
        .filter_map(|defect| match defect {
            PlanDefect::CriterionReachedByNoImplementation { criterion } => {
                Some(criterion.as_str().to_owned())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        unreached,
        ["converge", "reconnect", "latency"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>()
    );

    // The same plan with three implementation steps, one experiment, and every
    // checkpoint returning to a build, is accepted. So the refusal above is
    // about what the plan is missing and not about the fixture's shape.
    let mut interleaved = vec![
        implementation("build-merge", "converge")?,
        implementation("build-ack", "reconnect")?,
        implementation("build-budget", "latency")?,
        experiment("spike-ordering", "ordering")?,
    ];
    for (index, requirement) in branch.requirements().iter().enumerate() {
        interleaved.push(PlanStep::Learning(learning(
            &format!("study-{index}"),
            requirement.concept(),
            "build-merge",
        )?));
    }
    let accepted = validate(&PlanDraft {
        branch: &branch,
        findings: &findings,
        steps: &interleaved,
        motivations: &[],
    });
    assert!(
        accepted.is_accepted(),
        "an interleaved plan was refused: {:?}",
        accepted.defects()
    );
    assert_eq!(
        accepted
            .plan()
            .ok_or("an accepted verdict held no plan")?
            .steps()
            .len(),
        interleaved.len()
    );
    Ok(())
}

/// **The no-forbidden-word control.**
///
/// A plan whose every step is a course, whose wording uses none of the design
/// document's words and none of the crate's identifiers, and which is fluent and
/// plausible. It is refused for exactly the same three structural reasons — so
/// the validator is reading structure and not text.
#[test]
fn a_fluent_lecture_list_plan_fails_validation() -> TestResult {
    let branch = editor_branch()?;
    let findings = every_requirement_needs_acquisition(&branch)?;

    let mut steps = Vec::new();
    for (index, requirement) in branch.requirements().iter().enumerate() {
        let stage = academic_build_learn::SelectionApproved::after(
            academic_build_learn::SimulationPassed::after(
                academic_build_learn::ExplainedByHand::after(
                    academic_build_learn::ReadingDone::of(text(
                        "Weeks 1-3 of the graduate seminar, with the recommended monograph",
                    )?),
                    text("the seminar's worked example, reproduced on paper")?,
                ),
                text("the seminar's third problem set, submitted and marked")?,
            ),
            id("merge-algorithm")?,
            id("crdt")?,
        );
        steps.push(PlanStep::Learning(LearningItem::plan(
            id(&format!("seminar-{index}"))?,
            requirement.concept(),
            EvidenceTask::of(
                text("attend the seminar and submit its third problem set")?,
                text("a marked problem set at or above the seminar's pass line")?,
            ),
            academic_build_learn::ReturnCheckpoint::of(stage, id("seminar-0")?),
        )));
    }

    let verdict = validate(&PlanDraft {
        branch: &branch,
        findings: &findings,
        steps: &steps,
        motivations: &[],
    });
    assert!(!verdict.is_accepted(), "a fluent course list was accepted");
    let kinds: BTreeSet<&str> = verdict.defects().iter().map(PlanDefect::as_str).collect();
    assert_eq!(
        kinds,
        [
            "NO_IMPLEMENTATION_STEP",
            "CRITERION_REACHED_BY_NO_IMPLEMENTATION",
            "CHECKPOINT_RETURNS_TO_NON_IMPLEMENTATION",
        ]
        .into_iter()
        .collect::<BTreeSet<&str>>(),
        "the fluent plan was refused for a different set of reasons"
    );
    Ok(())
}

/// One finding per requirement, all needing acquisition, over real overlays.
fn every_requirement_needs_acquisition(
    branch: &ArchitectureBranch,
) -> Result<Vec<ReadinessFinding>, Box<dyn Error>> {
    let mut found = Vec::new();
    for requirement in branch.requirements() {
        found.push(categorize(
            requirement,
            RequirementOrigin::PrerequisiteNeighbour {
                of_concept: id("merge-order")?,
            },
            &thin_state(requirement.concept())?,
        ));
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// 9. `motivation_edges_are_shown_in_parallel`
// ---------------------------------------------------------------------------

/// Three rows, three reasons, one order, and nothing that adds them.
#[test]
fn motivation_edges_are_shown_in_parallel() -> TestResult {
    let section = section_20()?;

    // The three labels, parsed out of section 20.3's own sentence.
    let sentence = section
        .lines()
        .find(|line| line.contains("motivation edge를 복수로 가질 수 있다"))
        .ok_or("section 20.3's motivation sentence is not in the design document")?;
    let listed: Vec<String> = sentence
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    assert_eq!(
        listed,
        MOTIVATIONS
            .iter()
            .map(|motivation| motivation.spec_token().to_owned())
            .collect::<Vec<String>>(),
        "the design document's motivation labels and MOTIVATIONS disagree"
    );

    // The display hands back one row per edge, each with its own reason, in the
    // enumeration's order whatever order the edges arrived in. The fixture's
    // edges arrive PROJECT, SCHOOL, ROLE on purpose.
    let concept = crdt_fundamentals();
    let edges = three_motivations(concept)?;
    assert_eq!(
        edges
            .iter()
            .map(|edge| edge.motivation())
            .collect::<Vec<Motivation>>(),
        vec![Motivation::Project, Motivation::School, Motivation::Role]
    );
    let display = MotivationDisplay::of(concept, &edges)?;
    assert_eq!(
        display
            .rows()
            .iter()
            .map(|row| row.motivation())
            .collect::<Vec<Motivation>>(),
        MOTIVATIONS.to_vec(),
        "the rows are not in the design document's order"
    );

    // Three reasons, all different, all the ones the edges carried. A display
    // that had folded them would have fewer rows or a shared reason.
    let reasons: BTreeSet<&str> = display
        .rows()
        .iter()
        .map(|row| row.reason().as_str())
        .collect();
    assert_eq!(reasons.len(), 3, "two rows share a reason");
    for edge in &edges {
        let row = display
            .rows()
            .iter()
            .find(|row| row.motivation() == edge.motivation())
            .ok_or("a motivation edge produced no row")?;
        assert_eq!(row.reason(), edge.reason(), "a row lost its own reason");
    }

    // A concept with one edge shows one row and not a third of something. There
    // is no total to be a fraction of.
    let one = MotivationDisplay::of(concept, &edges[..1])?;
    assert_eq!(one.rows().len(), 1);
    assert!(one.carries(Motivation::Project));
    assert!(!one.carries(Motivation::School));

    // Two edges under one label are refused rather than joined: joining them is
    // the only way one row could come to stand for two reasons.
    let doubled = vec![
        edges[0].clone(),
        academic_build_learn::MotivationEdge::of(
            Motivation::Project,
            concept,
            text("and also because of the second project")?,
        ),
    ];
    assert!(
        matches!(
            MotivationDisplay::of(concept, &doubled),
            Err(academic_build_learn::BuildLearnError::DuplicateMotivationEdge("PROJECT"))
        ),
        "two edges under one label were joined"
    );

    // An edge about a different concept is refused, so a display cannot gather
    // reasons from more than one subject into one column.
    let stray = vec![academic_build_learn::MotivationEdge::of(
        Motivation::School,
        ot_fundamentals(),
        text("a reason about another concept")?,
    )];
    assert!(matches!(
        MotivationDisplay::of(concept, &stray),
        Err(academic_build_learn::BuildLearnError::MotivationEdgeIsAboutAnotherConcept { .. })
    ));

    // The serialized display is three labelled rows and no total. Compared as a
    // whole key set, so a `score` added later is an extra key rather than an
    // invisible addition.
    let wire = serde_json::to_value(&display)?;
    let object = wire
        .as_object()
        .ok_or("a display did not serialize as an object")?;
    assert_eq!(
        object
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<&str>>(),
        ["concept", "rows"].into_iter().collect::<BTreeSet<&str>>(),
        "the display's serialized key set changed"
    );
    let rows = object["rows"]
        .as_array()
        .ok_or("the rows did not serialize as an array")?;
    assert_eq!(rows.len(), 3);
    for row in rows {
        assert_eq!(
            row.as_object()
                .ok_or("a row did not serialize as an object")?
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<&str>>(),
            ["motivation", "reason"]
                .into_iter()
                .collect::<BTreeSet<&str>>(),
            "a row's serialized key set changed"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Section 21.
// ---------------------------------------------------------------------------

/// Section 21.2's six statuses are the design document's six, both ways.
#[test]
fn six_mapping_statuses_are_the_design_documents() -> TestResult {
    let section = section_21()?;
    let listed: Vec<String> = section
        .lines()
        .filter(|line| line.starts_with("- `"))
        .filter_map(|line| line.split('`').nth(1).map(str::to_owned))
        .collect();
    assert_eq!(
        listed,
        MAPPING_STATUSES
            .iter()
            .map(|status| status.as_str().to_owned())
            .collect::<Vec<String>>(),
        "section 21.2's statuses and MAPPING_STATUSES disagree"
    );
    assert_eq!(listed.len(), 6, "the parsed status list is {listed:?}");

    // The two that assert a particular offering covers the subject are exactly
    // the two whose bullet names an offering or an enrolment, stated over the
    // whole enumeration.
    let bound: BTreeSet<&str> = MAPPING_STATUSES
        .into_iter()
        .filter(|status| status.requires_actual_coverage())
        .map(MappingStatus::as_str)
        .collect();
    assert_eq!(
        bound,
        ["CAN_BE_SUPPORTED_BY_CURRENT_COURSE", "CONFIRMED_NEXT_TERM"]
            .into_iter()
            .collect::<BTreeSet<&str>>()
    );
    Ok(())
}

/// A course that merely exists cannot produce a current-support status.
///
/// REQ-21-014 and REQ-36-038, held as a value that does not exist.
#[test]
fn a_course_that_exists_is_not_offering_coverage() -> TestResult {
    let subject = crdt_fundamentals();
    for status in MAPPING_STATUSES
        .into_iter()
        .filter(|status| status.requires_actual_coverage())
    {
        let refused = CourseProjectMapping::publish(
            subject,
            None,
            None,
            status,
            text("the catalog lists the course")?,
        );
        assert!(
            matches!(
                refused,
                Err(academic_build_learn::BuildLearnError::StatusRequiresActualCoverage(_))
            ),
            "{} was published with no observed coverage",
            status.as_str()
        );
    }

    // The four that do not assert it publish without one, so the refusal above
    // is about the two and not about the constructor refusing everything.
    let mut published = 0_usize;
    for status in MAPPING_STATUSES
        .into_iter()
        .filter(|status| !status.requires_actual_coverage())
    {
        let mapping =
            CourseProjectMapping::publish(subject, None, None, status, text("a reason")?)?;
        assert_eq!(mapping.status(), status);
        assert!(mapping.actual().is_none());
        published += 1;
    }
    assert_eq!(published, 4, "the control published {published} mappings");
    Ok(())
}

/// Title keyword matching cannot reach a coverage claim at all.
///
/// REQ-21-003. A designed coverage is a list of identities read off `P2-U1`'s
/// own revision, so a course whose title is `데이터베이스` and whose
/// `designed_concept_coverage` is empty designs nothing — and an actual coverage
/// needs a sighting, which a title is not.
#[test]
fn a_title_is_not_a_coverage_claim() -> TestResult {
    let subject = crdt_fundamentals();

    // A revision that names nothing covers nothing, whatever it is called.
    let revision = support::title_only_revision()?;
    let designed = DesignedCoverage::of(&revision);
    assert!(designed.concepts().is_empty());
    assert!(designed.competencies().is_empty());
    assert!(!designed.designs(subject));

    // The control: a revision that does name the subject designs it. So the
    // negative above is about the revision and not about the reader.
    let naming = support::covering_revision(subject)?;
    assert!(DesignedCoverage::of(&naming).designs(subject));

    // And an actual coverage with no sighting has no value.
    let offering = support::offering_for(&revision)?;
    assert!(matches!(
        ActualCoverage::observed(&offering, subject, Vec::new(), true),
        Err(academic_build_learn::BuildLearnError::CoverageHasNoEvidence(_))
    ));
    Ok(())
}

/// The six statuses are decided from evidence, and each is reachable.
#[test]
fn each_mapping_status_is_reached_by_its_own_evidence() -> TestResult {
    let subject = crdt_fundamentals();
    let revision = support::covering_revision(subject)?;
    let offering = support::offering_for(&revision)?;
    let upcoming = ActualCoverage::observed(
        &offering,
        subject,
        vec![(
            CoverageEvidenceKind::Syllabus,
            support::evidence_id("syllabus-week-9"),
        )],
        true,
    )?;
    let past = ActualCoverage::observed(
        &offering,
        subject,
        vec![(
            CoverageEvidenceKind::Lecture,
            support::evidence_id("lecture-week-2"),
        )],
        false,
    )?;

    let cases: Vec<(MappingStatus, MappingEvidence<'_>)> = vec![
        (
            MappingStatus::CanBeSupportedByCurrentCourse,
            MappingEvidence {
                enrolment: EnrolmentStanding::Enrolled,
                personal: PersonalEvidenceStanding::Weak,
                actual: Some(&upcoming),
                offering_status: Some(academic_curriculum::OfferingStatus::Confirmed),
                experiment_is_more_direct: false,
            },
        ),
        (
            MappingStatus::PreviouslyTakenEvidenceWeak,
            MappingEvidence {
                enrolment: EnrolmentStanding::Completed,
                personal: PersonalEvidenceStanding::Weak,
                actual: Some(&past),
                offering_status: None,
                experiment_is_more_direct: false,
            },
        ),
        (
            MappingStatus::ConfirmedNextTerm,
            MappingEvidence {
                enrolment: EnrolmentStanding::Neither,
                personal: PersonalEvidenceStanding::Weak,
                actual: Some(&upcoming),
                offering_status: Some(academic_curriculum::OfferingStatus::Confirmed),
                experiment_is_more_direct: false,
            },
        ),
        (
            MappingStatus::HistoricallyAvailable,
            MappingEvidence {
                enrolment: EnrolmentStanding::Neither,
                personal: PersonalEvidenceStanding::Weak,
                actual: None,
                offering_status: Some(academic_curriculum::OfferingStatus::HistoricallyLikely),
                experiment_is_more_direct: false,
            },
        ),
        (
            MappingStatus::ExternalOrExperimentBetter,
            MappingEvidence {
                enrolment: EnrolmentStanding::Neither,
                personal: PersonalEvidenceStanding::Weak,
                actual: None,
                offering_status: None,
                experiment_is_more_direct: true,
            },
        ),
        (
            MappingStatus::NoDirectCourseMatch,
            MappingEvidence {
                enrolment: EnrolmentStanding::Neither,
                personal: PersonalEvidenceStanding::Weak,
                actual: None,
                offering_status: None,
                experiment_is_more_direct: false,
            },
        ),
    ];
    let mut reached = BTreeSet::new();
    for (expected, evidence) in &cases {
        let found = MappingStatus::for_evidence(evidence);
        assert_eq!(
            found,
            *expected,
            "the evidence for {} produced {}",
            expected.as_str(),
            found.as_str()
        );
        reached.insert(found);
    }
    assert_eq!(
        reached,
        MAPPING_STATUSES.into_iter().collect::<BTreeSet<_>>(),
        "a mapping status was never reached"
    );

    // An enrolment with coverage that is *not* upcoming is not current support:
    // section 21.2's first bullet says `실제 upcoming coverage 근거가 있음`.
    assert_ne!(
        MappingStatus::for_evidence(&MappingEvidence {
            enrolment: EnrolmentStanding::Enrolled,
            personal: PersonalEvidenceStanding::Sufficient,
            actual: Some(&past),
            offering_status: None,
            experiment_is_more_direct: false,
        }),
        MappingStatus::CanBeSupportedByCurrentCourse
    );
    Ok(())
}

/// The two effects of one channel are two values, and nothing folds them.
///
/// REQ-21-013, and `P2-N6`'s vector rule one layer up.
#[test]
fn the_two_channel_effects_are_kept_apart() -> TestResult {
    let subject = crdt_fundamentals();
    let comparison = ChannelComparison::of(
        subject,
        support::estimate(1, 2)?,
        support::estimate(30, 60)?,
    );
    assert_eq!(comparison.immediate_gap().low(), 1);
    assert_eq!(comparison.breadth().low(), 30);
    assert_ne!(comparison.immediate_gap(), comparison.breadth());

    // The serialized comparison carries both and no third number.
    let wire = serde_json::to_value(&comparison)?;
    assert_eq!(
        wire.as_object()
            .ok_or("a comparison did not serialize as an object")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<&str>>(),
        ["subject", "immediate_gap", "breadth"]
            .into_iter()
            .collect::<BTreeSet<&str>>(),
        "the comparison's serialized key set changed"
    );
    Ok(())
}

/// Every part identifier this crate takes is the shape it says it admits.
///
/// A whole-set classification rather than a list of rejected spellings: every
/// ASCII byte is offered inside an otherwise legal identifier and required to
/// be admitted **exactly** when this test's own independent predicate says it
/// belongs, in both directions, and the length bound is asserted on both sides.
///
/// It is here because `P2-A5` measured the gap: adding `+` to the character
/// class left this crate's whole suite green, and the length bound was never
/// measured at all. `PartId` is the name a criterion, a decision, an
/// alternative and a responsibility are joined back by across a serialized
/// document, so what it admits is what that join has to survive. It is the
/// port of `P2-R5`'s `every_identifier_is_the_shape_this_crate_admits`.
#[test]
fn every_part_identifier_is_the_shape_this_crate_admits() -> TestResult {
    // Written here rather than read from the crate, so the two are independent.
    let belongs =
        |byte: u8| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-';

    for byte in 0_u8..=127 {
        let candidate = format!("a{}b", char::from(byte));
        let taken = PartId::new(candidate.clone()).is_ok();
        assert_eq!(
            taken,
            belongs(byte),
            "byte {byte} in {candidate:?} is admitted {taken} and belongs {}",
            belongs(byte)
        );
    }

    // Beyond ASCII, where a byte-wise reader and a character-wise one disagree.
    for outside in ["개념", "a개념b", "a\u{00e9}b", "a\u{1f600}b"] {
        assert!(
            matches!(
                PartId::new(outside),
                Err(academic_build_learn::BuildLearnError::InvalidIdentifier(_))
            ),
            "{outside:?} was admitted as a part identifier"
        );
    }

    // The length boundary, on both sides of it, and the empty value.
    assert!(PartId::new("a".repeat(64)).is_ok());
    for refused in [String::new(), "a".repeat(65)] {
        let length = refused.len();
        assert!(
            matches!(
                PartId::new(refused),
                Err(academic_build_learn::BuildLearnError::InvalidIdentifier(_))
            ),
            "a {length}-byte identifier was admitted"
        );
    }

    // The other text wrapper is a different rule and stays one: `NonEmptyText`
    // refuses blankness and admits everything else, so a byte this test refuses
    // for `PartId` is not thereby refused for a sentence.
    assert!(NonEmptyText::new("a+b").is_ok());
    assert!(matches!(
        NonEmptyText::new("   "),
        Err(academic_build_learn::BuildLearnError::EmptyText)
    ));
    Ok(())
}
