//! `P2-N5`'s ten named acceptance rows.
//!
//! Three of them read `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compare what is in this crate against what is in the document, in both
//! directions: section 15.2's five-row table, its step 3 sentence and section
//! 15.3's eight-field sentence are **measurements** rather than counts restated
//! in a test. The same reads pick up section 15.2 step 6's four informal names,
//! which is one fewer than the table has rows;
//! `the_step_six_prose_names_one_fewer_than_the_table` records that rather than
//! resolving it.
//!
//! The scenario is section 36.4's, because it is the design document's own
//! worked example of this engine. Its evidence comes from `P2-N2`'s fixture
//! module by `#[path]`, so a `TeachingSite` here names a node of a document
//! `P2-L4` produced.

#[path = "common/mod.rs"]
mod common;

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_domain::{
    ConfidencePermille, EntityId, FreshnessBand, MasteryLevel,
    entity_registry::EntityKind,
    predicates::{PredicateName, PrerequisiteStrength},
    question::QuestionStatus,
};
use academic_freshness::{CitedEdge, DatedEvidence, NeighborUse, Spillover};
use academic_gap::{
    ActiveGoal, AlternativePath, ConceptReading, ConceptState, DiagnosticOffer, EXPLANATION_FIELDS,
    ExplanationParts, GAP_KINDS, GapCase, GapCaseWire, GapError, GapExplanation, GapKind,
    GoalCriteria, IdentityStanding, LinkedContext, MinimumRemediation, NoAlternativeReason,
    PrerequisiteEdge, PrerequisiteGraph, RETRIEVAL_FLOOR, RemediationActivity, STATE_DIMENSIONS,
    STEP_SIX_INFORMAL_NAMES, SpecificityDefect, StateDimension, SuccessCriterion, blocking_floor,
    expand, gap_bearing, search,
};
use academic_knowledge_state::{
    ConceptEvidence, ConceptLink, EligibilityOutcome, EvidenceDossier, Outcome, Participation,
    SourceIntegrity,
};

use common::{
    TestResult, at, band_from, buffer_pool, builds_on, disk_page, entity, evidence_id,
    exercise_evidence, exposure_evidence, failed_exercise_evidence, fan_out, full_dossier, offered,
    prior, random_io, reading, requires, scope, section_36_4_graph, storage_hierarchy,
    understand_buffer_pool, unknown_band, unresolved_authorship,
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

/// The whole of section 15, from its `## ` heading to the next one.
fn section_15(page: &str) -> Result<String, Box<dyn Error>> {
    let start = page
        .find("## 15. Gap & Prerequisite Engine")
        .ok_or("the design document has no section 15")?;
    let rest = &page[start..];
    let end = rest[1..]
        .find(
            "
## ",
        )
        .map_or(rest.len(), |offset| offset + 1);
    Ok(rest[..end].to_owned())
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

/// The rows of the one markdown table inside `body`.
fn table_rows(body: &str) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let mut rows = Vec::new();
    let mut seen_header = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            if rows.is_empty() {
                continue;
            }
            break;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect();
        if !seen_header {
            seen_header = true;
            continue;
        }
        if cells
            .iter()
            .all(|cell| cell.chars().all(|c| c == '-' || c == ':'))
        {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err("no table rows were found".into());
    }
    Ok(rows)
}

/// The numbered step whose text starts with `index. `.
fn numbered_step(body: &str, index: u8) -> Result<String, Box<dyn Error>> {
    let prefix = format!("{index}. ");
    body.lines()
        .find(|line| line.trim_start().starts_with(&prefix))
        .map(|line| line.trim_start()[prefix.len()..].trim().to_owned())
        .ok_or_else(|| format!("section 15.2 has no step {index}").into())
}

/// Every back-quoted spelling in `text`, in its order.
fn quoted(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

// ---------------------------------------------------------------------------
// 1. `low_mastery_without_goal_is_not_a_gap`
// ---------------------------------------------------------------------------

/// A concept at `UNSEEN` with nothing recorded, which is as low as a state gets.
fn lowest_state(concept: EntityId) -> Result<ConceptState, Box<dyn Error>> {
    Ok(ConceptState::overlay(
        concept,
        EntityKind::Concept,
        IdentityStanding::Settled,
        &[],
        &unknown_band(concept)?,
        &[],
    )?)
}

#[test]
fn low_mastery_without_goal_is_not_a_gap() -> TestResult {
    // The state is as low as section 13.1's ladder goes and its band is
    // `P2-N3`'s lowest.
    let state = lowest_state(disk_page())?;
    assert_eq!(state.mastery(), MasteryLevel::Unseen);
    assert_eq!(state.freshness(), FreshnessBand::Unknown);

    // No active goal reaches it: an empty graph gives the goal no edge to
    // descend, so section 15.2 step 2's expansion is empty and step 4 has
    // nothing to look at.
    let goal = understand_buffer_pool()?;
    let empty = PrerequisiteGraph::new();
    assert!(expand(&goal, &empty).is_empty());
    let found = search(
        &goal,
        &empty,
        &[reading(disk_page(), unknown_band(disk_page())?)],
        None,
    )?;
    assert!(
        found.is_none(),
        "a state no active goal's path reaches produced a gap"
    );

    // And the same lowest state, once a goal's `REQUIRES` edge does reach it,
    // *is* a gap — so the previous assertion is about the goal and not about a
    // state the engine could never report at all.
    let graph = PrerequisiteGraph::new().with(requires(
        buffer_pool(),
        disk_page(),
        PrerequisiteStrength::Hard,
        "edge-buffer-pool-disk-page",
    )?);
    let with_goal = search(
        &goal,
        &graph,
        &[reading(disk_page(), unknown_band(disk_page())?)],
        None,
    )?
    .ok_or("the reachable case produced no gap")?;
    assert_eq!(with_goal.candidates().len(), 1);
    assert_eq!(with_goal.candidates()[0].concept(), disk_page());

    // The whole crate offers no other route to a `GapCase`. The public surface
    // is read out of the source rather than recited: every free function that
    // returns one has `ActiveGoal` in its signature.
    let sources = crate_sources()?;
    let producers = producers_of_gap_case(&sources);
    assert!(
        !producers.is_empty(),
        "the reader found no producer of a GapCase at all"
    );
    for (name, signature) in &producers {
        assert!(
            signature.contains("ActiveGoal") || signature.contains("GapCaseWire"),
            "{name} produces a GapCase without an ActiveGoal: {signature}"
        );
    }
    Ok(())
}

/// Every product source of this crate, as `(path, code)`.
fn crate_sources() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for entry in fs::read_dir(&root)? {
        let path = entry?.path();
        if path.extension().is_some_and(|value| value == "rs") {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned();
            found.push((name, fs::read_to_string(&path)?));
        }
    }
    found.sort();
    Ok(found)
}

/// Every `pub fn` whose return type mentions `GapCase`, with its signature.
fn producers_of_gap_case(sources: &[(String, String)]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (path, code) in sources {
        for (index, _) in code.match_indices("pub fn ") {
            let rest = &code[index..];
            let end = rest.find(" {").unwrap_or(rest.len());
            let signature = rest[..end].replace('\n', " ");
            let after_arrow = signature.split("->").nth(1).unwrap_or_default();
            if after_arrow.contains("GapCase") {
                let name = signature
                    .trim_start_matches("pub fn ")
                    .split('(')
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                found.push((format!("{path}::{name}"), signature));
            }
        }
    }
    found
}

// ---------------------------------------------------------------------------
// 2. `gap_case_round_trip`
// ---------------------------------------------------------------------------

/// Section 36.4's case: `Buffer Pool` surface, `Disk Page` root one hop down,
/// `Storage Hierarchy` a second hop down.
fn section_36_4_case() -> Result<GapCase, Box<dyn Error>> {
    let goal = understand_buffer_pool()?;
    let graph = section_36_4_graph()?;
    let readings = vec![
        // `Buffer Pool` was met in a lecture; that is `EXPOSED` and the goal
        // wants `PRACTICED`, but the surface concept is never a candidate.
        with_evidence(
            reading(buffer_pool(), unknown_band(buffer_pool())?),
            vec![offered(
                exposure_evidence("bp-lecture")?,
                "bp-lecture",
                full_dossier(buffer_pool()),
            )],
        ),
        // `Disk Page` has one exposure and no performance: section 15.2's
        // `prerequisite 수행 evidence가 부족`.
        with_evidence(
            reading(disk_page(), unknown_band(disk_page())?),
            vec![offered(
                exposure_evidence("dp-lecture")?,
                "dp-lecture",
                full_dossier(disk_page()),
            )],
        ),
        // `Storage Hierarchy` likewise, one hop deeper.
        with_evidence(
            reading(storage_hierarchy(), unknown_band(storage_hierarchy())?),
            vec![offered(
                exposure_evidence("sh-lecture")?,
                "sh-lecture",
                full_dossier(storage_hierarchy()),
            )],
        ),
    ];
    search(&goal, &graph, &readings, None)?.ok_or_else(|| "section 36.4 produced no gap".into())
}

fn with_evidence(
    mut reading: ConceptReading,
    offered: Vec<academic_gap::OfferedEvidence>,
) -> ConceptReading {
    reading.offered = offered;
    reading
}

#[test]
fn gap_case_round_trip() -> TestResult {
    let case = section_36_4_case()?;
    let encoded = serde_json::to_string(&case)?;
    let decoded: GapCase = serde_json::from_str(&encoded)?;
    assert_eq!(decoded, case, "a GapCase did not survive a round trip");
    assert_eq!(serde_json::to_string(&decoded)?, encoded);

    // Section 15.1's own field names are on the wire.
    let value: serde_json::Value = serde_json::from_str(&encoded)?;
    for field in [
        "goal",
        "surfaceConcept",
        "rootCandidates",
        "userStateSnapshot",
    ] {
        assert!(
            value.get(field).is_some(),
            "the wire shape has no {field} field"
        );
    }

    // Section 15.1's `reason` cell is required and never blank: a candidate
    // whose reason is whitespace has no value of the type.
    let root = case.roots()[0];
    assert!(!root.reason().trim().is_empty());
    for blank in [
        "", "   ", "
	",
    ] {
        let refused = academic_gap::RootCandidate::of(
            root.kind(),
            root.blocking_path().clone(),
            blank,
            root.evidence().to_vec(),
            root.confidence(),
            root.ancestor_impact().to_vec(),
            root.explanation().clone(),
        );
        assert!(
            matches!(refused, Err(GapError::CandidateReasonMissing)),
            "a blank reason was accepted"
        );
    }
    // And a candidate whose explanation is about another kind is refused: the
    // explanation and the candidate cannot disagree about what they describe.
    let refused = academic_gap::RootCandidate::of(
        GapKind::FreshnessGap,
        root.blocking_path().clone(),
        root.reason(),
        root.evidence().to_vec(),
        root.confidence(),
        root.ancestor_impact().to_vec(),
        root.explanation().clone(),
    );
    assert!(matches!(
        refused,
        Err(GapError::CandidateExplainsAnotherConcept)
    ));

    // Deserialization re-runs the constructor's checks rather than trusting the
    // document: a case with no candidate is refused on the way in.
    let mut wire: GapCaseWire = serde_json::from_str(&encoded)?;
    wire.root_candidates.clear();
    let refused = serde_json::from_str::<GapCase>(&serde_json::to_string(&wire)?);
    assert!(
        refused.is_err(),
        "a candidate-less case was accepted on the wire"
    );

    // And a candidate whose blocking path leaves some other surface is refused
    // too, which is the check that keeps a decoded case internally consistent.
    let mut moved: GapCaseWire = serde_json::from_str(&encoded)?;
    moved.surface_concept = random_io();
    let refused = serde_json::from_str::<GapCase>(&serde_json::to_string(&moved)?);
    assert!(
        matches!(
            refused.map_err(|error| error.to_string()),
            Err(message) if message.contains("does not start at the surface concept")
        ),
        "a case whose candidates leave the surface was accepted on the wire"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `goal_criteria_required_before_expansion`
// ---------------------------------------------------------------------------

#[test]
fn goal_criteria_required_before_expansion() -> TestResult {
    // Step 1 is `활성 목표를 concept/competency success criteria로 명시한다`.
    let page = specification()?;
    let step_one = numbered_step(&section(&page, "### 15.2 ")?, 1)?;
    assert!(
        step_one.contains("success criteria"),
        "section 15.2 step 1 no longer names success criteria: {step_one}"
    );

    // An empty criteria list has no value of the type, so there is nothing to
    // declare a goal with.
    assert!(GoalCriteria::of(Vec::new()).is_none());

    // Every function in this crate that expands or searches takes an
    // `&ActiveGoal`, read out of the source rather than recited.
    let sources = crate_sources()?;
    let entries = entry_points(&sources);
    assert!(
        entries.len() >= 2,
        "the reader found only {entries:?} entry points"
    );
    for (name, signature) in &entries {
        assert!(
            signature.contains("goal: &ActiveGoal"),
            "{name} can be reached without an ActiveGoal: {signature}"
        );
    }

    // `ActiveGoal` has one constructor and it takes a `GoalCriteria` by value.
    let constructors: Vec<String> = sources
        .iter()
        .flat_map(|(_, code)| {
            code.match_indices("impl ActiveGoal {")
                .map(|(index, _)| code[index..].to_owned())
                .collect::<Vec<String>>()
        })
        .collect();
    let body = constructors.first().ok_or("ActiveGoal has no impl block")?;
    let end = body.find("\n}").unwrap_or(body.len());
    let block = &body[..end];
    let public_constructors: Vec<&str> = block
        .match_indices("pub fn ")
        .map(|(index, _)| {
            let rest = &block[index..];
            let stop = rest.find(" {").unwrap_or(rest.len());
            &rest[..stop]
        })
        .filter(|signature| signature.contains("-> Result<Self") || signature.contains("-> Self"))
        .collect();
    assert_eq!(
        public_constructors.len(),
        1,
        "ActiveGoal has {public_constructors:?} rather than one constructor"
    );
    assert!(
        public_constructors[0].contains("criteria: GoalCriteria"),
        "ActiveGoal's constructor does not take criteria by value: {}",
        public_constructors[0]
    );

    // Declaring one with criteria works, and the criteria are readable.
    let goal = understand_buffer_pool()?;
    assert_eq!(goal.criteria().criteria().len(), 1);
    assert_eq!(
        goal.criteria().required_level(buffer_pool()),
        Some(MasteryLevel::Practiced)
    );

    // A goal whose surface is a `FIELD` is refused, because section 7.4 says a
    // field carries no independent prerequisite to expand toward.
    let criteria = GoalCriteria::of(vec![SuccessCriterion::concept(
        buffer_pool(),
        EntityKind::Concept,
        MasteryLevel::Practiced,
    )?])
    .ok_or("empty criteria")?;
    let refused = ActiveGoal::declare(
        entity("goal-study-databases"),
        scope(),
        entity("DATABASE_SYSTEMS"),
        EntityKind::Field,
        criteria,
    );
    assert!(matches!(
        refused,
        Err(GapError::SurfaceConceptCarriesNoPrerequisite {
            kind: EntityKind::Field
        })
    ));
    Ok(())
}

/// Every `pub fn` in this crate whose name is `expand` or `search`.
fn entry_points(sources: &[(String, String)]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (path, code) in sources {
        for name in ["pub fn expand(", "pub fn search("] {
            if let Some(index) = code.find(name) {
                let rest = &code[index..];
                let end = rest.find(" {").unwrap_or(rest.len());
                found.push((path.clone(), rest[..end].replace('\n', " ")));
            }
        }
    }
    found
}

// ---------------------------------------------------------------------------
// 4. `weak_builds_on_is_excluded_or_conditional`
// ---------------------------------------------------------------------------

#[test]
fn weak_builds_on_is_excluded_or_conditional() -> TestResult {
    // `P2-C4`'s registry is what says which strengths each predicate admits, and
    // it is read here rather than restated: `REQUIRES` never carries `HELPFUL`
    // and `BUILDS_ON` never carries `HARD`, so `강한 BUILDS_ON` is `STRONG`.
    assert!(matches!(
        PrerequisiteEdge::admit(
            PredicateName::Requires,
            PrerequisiteStrength::Helpful,
            buffer_pool(),
            disk_page(),
            vec![evidence_id("e")],
        ),
        Err(GapError::StrengthNotAdmitted { .. })
    ));
    assert!(matches!(
        PrerequisiteEdge::admit(
            PredicateName::BuildsOn,
            PrerequisiteStrength::Hard,
            buffer_pool(),
            disk_page(),
            vec![evidence_id("e")],
        ),
        Err(GapError::StrengthNotAdmitted { .. })
    ));
    // And `RELATED_TO` is refused entirely, which is section 7.2's own
    // `path engine의 prerequisite로 사용 금지`.
    assert!(matches!(
        PrerequisiteEdge::admit(
            PredicateName::RelatedTo,
            PrerequisiteStrength::Strong,
            buffer_pool(),
            disk_page(),
            vec![evidence_id("e")],
        ),
        Err(GapError::NotATraversablePredicate("RELATED_TO"))
    ));

    // *Excluded.* A weak `BUILDS_ON` has no blocking floor, so the descent has
    // nothing to cross and produces no path through it.
    assert_eq!(blocking_floor(PrerequisiteStrength::Helpful), None);
    let weak = builds_on(
        buffer_pool(),
        random_io(),
        PrerequisiteStrength::Helpful,
        "edge-weak",
    )?;
    assert!(!weak.blocks());
    let goal = understand_buffer_pool()?;
    let only_weak = PrerequisiteGraph::new().with(weak);
    assert!(
        expand(&goal, &only_weak).is_empty(),
        "the descent crossed a helpful BUILDS_ON"
    );
    assert!(
        search(
            &goal,
            &only_weak,
            &[reading(random_io(), unknown_band(random_io())?)],
            None
        )?
        .is_none(),
        "a helpful BUILDS_ON produced a candidate"
    );

    // A strong `BUILDS_ON` does block, so the exclusion is about the strength
    // and not about the predicate.
    let strong = builds_on(
        buffer_pool(),
        random_io(),
        PrerequisiteStrength::Strong,
        "edge-strong",
    )?;
    assert!(strong.blocks());
    assert_eq!(strong.floor(), Some(MasteryLevel::Understood));
    let with_strong = PrerequisiteGraph::new().with(strong);
    assert_eq!(expand(&goal, &with_strong).len(), 1);

    // *Conditional.* Two helpful branches out of a descended node that no
    // success criterion names is section 15.2's `prerequisite가 갈림`, and it
    // routes to `CONTEXT_GAP` — while the branch targets themselves are still
    // never descended into.
    let branching = PrerequisiteGraph::new()
        .with(requires(
            buffer_pool(),
            disk_page(),
            PrerequisiteStrength::Hard,
            "edge-bp-dp",
        )?)
        .with(builds_on(
            disk_page(),
            random_io(),
            PrerequisiteStrength::Helpful,
            "edge-dp-rio",
        )?)
        .with(builds_on(
            disk_page(),
            fan_out(),
            PrerequisiteStrength::Helpful,
            "edge-dp-fo",
        )?);
    let paths = expand(&goal, &branching);
    assert_eq!(paths.len(), 1, "a helpful branch was descended");
    assert_eq!(paths[0].tip(), disk_page());

    let case = search(
        &goal,
        &branching,
        &[practised(disk_page(), unknown_band(disk_page())?)?],
        None,
    )?
    .ok_or("the branching graph produced no case")?;
    assert_eq!(case.candidates().len(), 1);
    assert_eq!(case.candidates()[0].kind(), GapKind::ContextGap);
    assert!(
        !case.candidates()[0].is_strong_deficit(),
        "a CONTEXT_GAP became a strong deficit"
    );

    // One helpful edge is not a branch: the same node, with the second helpful
    // edge removed, is no longer a `CONTEXT_GAP`.
    let single = PrerequisiteGraph::new()
        .with(requires(
            buffer_pool(),
            disk_page(),
            PrerequisiteStrength::Hard,
            "edge-bp-dp",
        )?)
        .with(builds_on(
            disk_page(),
            random_io(),
            PrerequisiteStrength::Helpful,
            "edge-dp-rio",
        )?);
    assert!(
        search(
            &goal,
            &single,
            &[practised(disk_page(), unknown_band(disk_page())?)?],
            None
        )?
        .is_none(),
        "one helpful edge was read as a branch"
    );
    Ok(())
}

/// A reading whose evidence puts the concept at `PRACTICED` with a `MODERATE`
/// band, so nothing but the branch can produce a gap.
fn practised(
    concept: EntityId,
    _unused: academic_freshness::FreshnessProjection,
) -> Result<ConceptReading, Box<dyn Error>> {
    let dated = dated_exercise(concept, "practised", 0)?;
    let band = band_from(concept, &[dated], &[], at(0), FreshnessBand::VeryHigh)?;
    Ok(with_evidence(
        reading(concept, band),
        vec![
            offered(
                exercise_evidence(&format!("{concept}-practised-a")),
                &format!("{concept}-practised-a"),
                full_dossier(concept),
            ),
            offered(
                exercise_evidence(&format!("{concept}-practised-b")),
                &format!("{concept}-practised-b"),
                full_dossier(concept),
            ),
        ],
    ))
}

fn dated_exercise(
    concept: EntityId,
    tag: &str,
    days_after: i64,
) -> Result<DatedEvidence, Box<dyn Error>> {
    let admitted = match EligibilityOutcome::admit(
        exercise_evidence(&format!("{concept}-{tag}-dated")),
        evidence_id(&format!("{concept}-{tag}-dated")),
        &full_dossier(concept),
    ) {
        EligibilityOutcome::Admitted(value) => value,
        EligibilityOutcome::Blocked(blocked) => {
            return Err(format!("the fixture was blocked: {:?}", blocked.reasons()).into());
        }
    };
    Ok(DatedEvidence::at(admitted, at(days_after)))
}

// ---------------------------------------------------------------------------
// 5. `four_state_dimensions_are_overlaid`
// ---------------------------------------------------------------------------

#[test]
fn four_state_dimensions_are_overlaid() -> TestResult {
    // Four is a measurement of step 3's sentence, compared in both directions.
    let page = specification()?;
    let step_three = numbered_step(&section(&page, "### 15.2 ")?, 3)?;
    for dimension in STATE_DIMENSIONS {
        assert!(
            step_three.contains(dimension.spec_token()),
            "step 3 does not name {}: {step_three}",
            dimension.spec_token()
        );
    }
    // The other direction: every noun step 3 lists is one of the four. The
    // sentence's own separators are `, ` and `와 `.
    let listed: Vec<String> = step_three
        .trim_start_matches("사용자 ")
        .split_once("를 overlay한다")
        .map(|(head, _)| head)
        .ok_or("step 3 has no overlay verb")?
        .replace("와 ", ", ")
        .split(", ")
        .map(|token| token.trim().to_owned())
        .collect();
    let declared: BTreeSet<&str> = STATE_DIMENSIONS
        .iter()
        .map(|dimension| dimension.spec_token())
        .collect();
    let designed: BTreeSet<&str> = listed.iter().map(String::as_str).collect();
    assert_eq!(
        designed, declared,
        "step 3's dimensions and STATE_DIMENSIONS differ"
    );
    assert_eq!(STATE_DIMENSIONS.len(), 4);

    // Each dimension decides an outcome by itself: three are held and the
    // fourth is moved.
    let goal = understand_buffer_pool()?;
    let graph = PrerequisiteGraph::new().with(requires(
        buffer_pool(),
        disk_page(),
        PrerequisiteStrength::Hard,
        "edge-bp-dp",
    )?);
    let concept = disk_page();

    // Baseline: practised, fresh, fully sufficient, nothing contradicting. The
    // goal's `HARD` edge needs `PRACTICED` and gets it, so there is no gap.
    let base = practised(concept, unknown_band(concept)?)?;
    assert!(
        search(&goal, &graph, std::slice::from_ref(&base), None)?.is_none(),
        "the baseline reading already produced a gap"
    );

    // Dimension one — mastery. The same two items, downgraded to exposure only,
    // fall below the `HARD` edge's `PRACTICED` floor.
    let mut moved = base.clone();
    moved.offered = vec![offered(
        exposure_evidence("dim-confidence-a")?,
        "dim-confidence-a",
        full_dossier(concept),
    )];
    assert_eq!(kind_of(&goal, &graph, &moved)?, Some(GapKind::MasteryGap));

    // Dimension two — freshness. Same evidence, same confidence, no
    // contradiction; only the band moves, and only below `RETRIEVAL_FLOOR`.
    let mut moved = base.clone();
    let stale = dated_exercise(concept, "practised", -400)?;
    moved.freshness = band_from(concept, &[stale], &[], at(0), FreshnessBand::Stale)?;
    assert_eq!(kind_of(&goal, &graph, &moved)?, Some(GapKind::FreshnessGap));
    // And the floor itself is what separates the two answers.
    let moderate = dated_exercise(concept, "practised", -120)?;
    let mut cleared = base.clone();
    cleared.freshness = band_from(concept, &[moderate], &[], at(0), FreshnessBand::Moderate)?;
    assert_eq!(academic_gap::RETRIEVAL_FLOOR, FreshnessBand::Moderate);
    assert_eq!(kind_of(&goal, &graph, &cleared)?, None);

    // Dimension three — confidence, and it has to be moved **alone**. Blocking
    // every item would empty the admitted set, and `P2-N2` would then report
    // `NO_EVIDENCE_RECORDED`, which is dimension one answering for dimension
    // three. So this reading keeps the mastery-gap case's own admitted item and
    // adds one item that could not be admitted: the level, the band and the
    // contradicting set are identical to the case above, and the only difference
    // is a sufficiency gap.
    let mut moved = base.clone();
    moved.offered = vec![
        offered(
            exposure_evidence("dim-confidence-a")?,
            "dim-confidence-a",
            full_dossier(concept),
        ),
        offered(
            exposure_evidence("dim-confidence-b")?,
            "dim-confidence-b",
            unresolved_authorship(concept),
        ),
    ];
    let overlaid = ConceptState::overlay(
        concept,
        EntityKind::Concept,
        IdentityStanding::Settled,
        &moved.offered,
        &moved.freshness,
        &[],
    )?;
    assert_eq!(
        overlaid.mastery(),
        MasteryLevel::Exposed,
        "the confidence case was supposed to have an admitted item"
    );
    assert_eq!(
        overlaid.unseen_basis(),
        None,
        "dimension one must not be able to answer for dimension three"
    );
    assert!(overlaid.contradicting().is_empty());
    assert!(
        overlaid
            .sufficiency_gaps()
            .contains(&academic_knowledge_state::SufficiencyGap::AuthorshipUnresolved)
    );
    assert_eq!(kind_of(&goal, &graph, &moved)?, Some(GapKind::EvidenceGap));

    // Dimension four — contradicting evidence. Mastery still clears the floor
    // and the band is still fresh; one failed attempt is on record.
    let mut moved = base.clone();
    moved.offered.push(offered(
        failed_exercise_evidence("dim-contradicting"),
        "dim-contradicting",
        full_dossier(concept),
    ));
    let state = ConceptState::overlay(
        concept,
        EntityKind::Concept,
        IdentityStanding::Settled,
        &moved.offered,
        &moved.freshness,
        &[],
    )?;
    assert_eq!(
        state.mastery(),
        MasteryLevel::Practiced,
        "the contradicting case was supposed to clear the floor on mastery"
    );
    assert_eq!(state.contradicting().len(), 1);
    assert_eq!(kind_of(&goal, &graph, &moved)?, Some(GapKind::MasteryGap));

    // The snapshot carries one reading per dimension, and each of the four
    // differs from the baseline in at least one of them.
    let snapshot = academic_gap::StateSnapshot::from(&state);
    assert_eq!(snapshot.concept, concept);
    assert_eq!(snapshot.mastery, MasteryLevel::Practiced);
    assert_eq!(snapshot.contradicting.len(), 1);
    assert!(snapshot.confidence.value() <= 1000);
    assert_eq!(StateDimension::Mastery.as_str(), "MASTERY");
    Ok(())
}

fn kind_of(
    goal: &ActiveGoal,
    graph: &PrerequisiteGraph,
    reading: &ConceptReading,
) -> Result<Option<GapKind>, Box<dyn Error>> {
    Ok(search(goal, graph, std::slice::from_ref(reading), None)?
        .map(|case| case.candidates()[0].kind()))
}

// ---------------------------------------------------------------------------
// 6. `first_strong_deficit_is_root_with_ancestor_impact`
// ---------------------------------------------------------------------------

#[test]
fn first_strong_deficit_is_root_with_ancestor_impact() -> TestResult {
    let case = section_36_4_case()?;

    // Section 15.1's block lists two candidates, one hop and two hops down.
    assert_eq!(case.candidates().len(), 2);
    assert_eq!(case.candidates()[0].concept(), disk_page());
    assert_eq!(case.candidates()[0].depth(), 1);
    assert_eq!(case.candidates()[1].concept(), storage_hierarchy());
    assert_eq!(case.candidates()[1].depth(), 2);

    // Section 36.4's own answer: the root is `Disk Page`, one hop below the
    // surface, and the deeper candidate stays a candidate rather than becoming
    // the root.
    let roots = case.roots();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].concept(), disk_page());
    assert!(roots[0].is_strong_deficit());
    assert_eq!(roots[0].kind(), GapKind::MasteryGap);
    assert!(case.diagnostic().is_none());

    // `그 조상 영향도`: the root names the ancestors above it, with the distance
    // and the weakest hop, and carries **no evidence of theirs**.
    let impact = roots[0].ancestor_impact();
    assert_eq!(impact.len(), 1);
    assert_eq!(impact[0].ancestor(), buffer_pool());
    assert_eq!(impact[0].hops_above_root(), 1);
    assert_eq!(impact[0].weakest_link(), PrerequisiteStrength::Hard);
    let encoded = serde_json::to_value(impact[0].clone())?;
    let fields: Vec<&String> = encoded
        .as_object()
        .ok_or("an ancestor impact is not an object")?
        .keys()
        .collect();
    assert_eq!(
        fields,
        vec!["ancestor", "hops_above_root", "weakest_link"],
        "an ancestor impact carries something other than the path fact"
    );

    // The deeper candidate's impact reaches both ancestors, and its weakest hop
    // is the `STRONG` one rather than the `HARD` one nearer the surface.
    let deeper = &case.candidates()[1];
    assert_eq!(deeper.ancestor_impact().len(), 2);
    assert_eq!(deeper.ancestor_impact()[0].ancestor(), buffer_pool());
    assert_eq!(deeper.ancestor_impact()[0].hops_above_root(), 2);
    assert_eq!(
        deeper.ancestor_impact()[0].weakest_link(),
        PrerequisiteStrength::Strong
    );

    // A deficit deeper than a *non*-strong one is still the root: the shallowest
    // strong deficit is what step 4 asks for, not the shallowest gap of any
    // kind. `Disk Page` is downgraded to an `EVIDENCE_GAP`, which is not a
    // strong deficit, and the root moves to `Storage Hierarchy`.
    let goal = understand_buffer_pool()?;
    let graph = section_36_4_graph()?;
    let readings = vec![
        with_evidence(
            reading(disk_page(), unknown_band(disk_page())?),
            vec![offered(
                exposure_evidence("dp-blocked")?,
                "dp-blocked",
                unresolved_authorship(disk_page()),
            )],
        ),
        with_evidence(
            reading(storage_hierarchy(), unknown_band(storage_hierarchy())?),
            vec![offered(
                exposure_evidence("sh-lecture-b")?,
                "sh-lecture-b",
                full_dossier(storage_hierarchy()),
            )],
        ),
    ];
    let moved = search(&goal, &graph, &readings, None)?.ok_or("no case")?;
    assert_eq!(moved.candidates()[0].kind(), GapKind::EvidenceGap);
    let roots = moved.roots();
    assert_eq!(roots.len(), 1);
    assert_eq!(
        roots[0].concept(),
        storage_hierarchy(),
        "the root did not move past a non-strong deficit"
    );
    assert_eq!(roots[0].depth(), 2);
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. `equal_candidates_are_both_retained_with_diagnostic`
// ---------------------------------------------------------------------------

/// Two prerequisites of the surface concept, alike in depth, kind and
/// confidence.
fn tied_case(
    diagnostic: Option<&DiagnosticOffer>,
) -> Result<Result<Option<GapCase>, GapError>, Box<dyn Error>> {
    let goal = understand_buffer_pool()?;
    let graph = PrerequisiteGraph::new()
        .with(requires(
            buffer_pool(),
            disk_page(),
            PrerequisiteStrength::Hard,
            "edge-bp-dp",
        )?)
        .with(requires(
            buffer_pool(),
            random_io(),
            PrerequisiteStrength::Hard,
            "edge-bp-rio",
        )?);
    let readings = vec![
        with_evidence(
            reading(disk_page(), unknown_band(disk_page())?),
            vec![
                offered(
                    exposure_evidence("tie-dp-a")?,
                    "tie-dp-a",
                    full_dossier(disk_page()),
                ),
                offered(
                    exposure_evidence("tie-dp-b")?,
                    "tie-dp-b",
                    full_dossier(disk_page()),
                ),
            ],
        ),
        with_evidence(
            reading(random_io(), unknown_band(random_io())?),
            vec![
                offered(
                    exposure_evidence("tie-rio-a")?,
                    "tie-rio-a",
                    full_dossier(random_io()),
                ),
                offered(
                    exposure_evidence("tie-rio-b")?,
                    "tie-rio-b",
                    full_dossier(random_io()),
                ),
            ],
        ),
    ];
    Ok(search(&goal, &graph, &readings, diagnostic))
}

#[test]
fn equal_candidates_are_both_retained_with_diagnostic() -> TestResult {
    // Without a diagnostic the engine refuses. It does not choose.
    let refused = tied_case(None)?;
    assert!(matches!(refused, Err(GapError::TiedRootsNeedADiagnostic)));

    let offer = DiagnosticOffer {
        minutes: 10,
        description: "두 후보를 가르는 10분짜리 확인".to_owned(),
        sources: vec![evidence_id("diagnostic-page-layout")],
        question: Some((entity("question-why-b-plus-tree"), QuestionStatus::Open)),
    };
    let case = tied_case(Some(&offer))?
        .map_err(|error| error.to_string())?
        .ok_or("no case")?;

    // Both are retained as roots.
    let roots = case.roots();
    assert_eq!(roots.len(), 2, "a tie was resolved rather than retained");
    let named: BTreeSet<EntityId> = roots.iter().map(|root| root.concept()).collect();
    assert_eq!(named, BTreeSet::from([disk_page(), random_io()]));
    assert_eq!(
        roots[0].confidence(),
        roots[1].confidence(),
        "the two roots were supposed to tie on confidence"
    );
    assert_eq!(roots[0].depth(), roots[1].depth());

    // And a diagnostic is attached, naming exactly those two, shaped like
    // section 15.2's own `사용자 확인 또는 diagnostic`.
    let diagnostic = case.diagnostic().ok_or("no diagnostic was attached")?;
    let tied: BTreeSet<EntityId> = diagnostic.tied().iter().copied().collect();
    assert_eq!(tied, named);
    assert_eq!(
        diagnostic.activity().activity(),
        RemediationActivity::UserConfirmationOrDiagnostic
    );
    assert_eq!(diagnostic.activity().minutes(), 10);
    assert!(!diagnostic.activity().sources().is_empty());

    // The `P2-N4` question it references is open and stays open: nothing in this
    // crate resolves one.
    assert_eq!(diagnostic.question_status(), Some(QuestionStatus::Open));
    let sources = crate_sources()?;
    for (path, code) in &sources {
        for forbidden in ["QuestionStatus::Resolved", "VerifiedQuestionResolution"] {
            assert!(
                !code.contains(forbidden),
                "{path} names {forbidden}, so this crate can close a question"
            );
        }
    }

    // A diagnostic whose activity is not section 15.2's `사용자 확인 또는
    // diagnostic` is refused, and so is one over fewer than two candidates.
    for shape in [
        RemediationActivity::FoundationalExplanationProblemOrExperiment,
        RemediationActivity::ShortRetrievalOrRefresher,
        RemediationActivity::MergeOrSenseCorrection,
        RemediationActivity::OptionsAndConditionsClarified,
    ] {
        let refused = academic_gap::TieDiagnostic::of(
            diagnostic.tied().to_vec(),
            MinimumRemediation::of(
                10,
                shape,
                "확인",
                vec![evidence_id("diagnostic-page-layout")],
            ),
            None,
        );
        assert!(
            matches!(refused, Err(GapError::DiagnosticIsNotADiagnostic)),
            "{shape:?} was accepted as a tie diagnostic"
        );
    }
    assert!(matches!(
        academic_gap::TieDiagnostic::of(vec![disk_page()], diagnostic.activity().clone(), None),
        Err(GapError::DiagnosticNeedsTwoCandidates)
    ));

    // A diagnostic over a resolved question is refused, so the reference cannot
    // point at work the user has already finished.
    let closed = DiagnosticOffer {
        question: Some((entity("question-why-b-plus-tree"), QuestionStatus::Resolved)),
        ..offer.clone()
    };
    assert!(matches!(
        tied_case(Some(&closed))?,
        Err(GapError::DiagnosticQuestionIsNotOpen)
    ));

    // The case survives the wire with both roots and the diagnostic intact.
    let decoded: GapCase = serde_json::from_str(&serde_json::to_string(&case)?)?;
    assert_eq!(decoded.roots().len(), 2);
    assert!(decoded.diagnostic().is_some());
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. `five_gap_types_route_correctly`
// ---------------------------------------------------------------------------

#[test]
fn five_gap_types_route_correctly() -> TestResult {
    // Five is a measurement of section 15.2's table, in both directions,
    // including both of its content columns.
    let page = specification()?;
    let rows = table_rows(&section(&page, "### 15.2 ")?)?;
    let designed: Vec<(String, String, String)> = rows
        .iter()
        .map(|cells| {
            (
                cells[0].trim_matches('`').to_owned(),
                cells[1].clone(),
                cells[2].clone(),
            )
        })
        .collect();
    assert_eq!(designed.len(), GAP_KINDS.len());
    for (index, kind) in GAP_KINDS.iter().enumerate() {
        assert_eq!(designed[index].0, kind.as_str());
        assert_eq!(designed[index].1, kind.meaning());
        assert_eq!(designed[index].2, kind.response());
    }
    let declared: BTreeSet<&str> = GAP_KINDS.iter().map(|kind| kind.as_str()).collect();
    let from_document: BTreeSet<&str> = designed.iter().map(|row| row.0.as_str()).collect();
    assert_eq!(declared, from_document);

    // Each of the five is reachable, and each routes from a different input.
    let goal = understand_buffer_pool()?;
    let graph = PrerequisiteGraph::new().with(requires(
        buffer_pool(),
        disk_page(),
        PrerequisiteStrength::Hard,
        "edge-bp-dp",
    )?);
    let concept = disk_page();

    // `MASTERY_GAP`: evidence exists and it is short of the floor.
    let mut mastery = reading(concept, unknown_band(concept)?);
    mastery.offered = vec![offered(
        exposure_evidence("route-mastery")?,
        "route-mastery",
        full_dossier(concept),
    )];
    assert_eq!(kind_of(&goal, &graph, &mastery)?, Some(GapKind::MasteryGap));

    // `FRESHNESS_GAP`: the floor is met and the band is not.
    let mut freshness = practised(concept, unknown_band(concept)?)?;
    let stale = dated_exercise(concept, "practised", -400)?;
    freshness.freshness = band_from(concept, &[stale], &[], at(0), FreshnessBand::Stale)?;
    assert_eq!(
        kind_of(&goal, &graph, &freshness)?,
        Some(GapKind::FreshnessGap)
    );

    // `EVIDENCE_GAP`: nothing was recorded at all.
    let evidence = reading(concept, unknown_band(concept)?);
    assert_eq!(
        kind_of(&goal, &graph, &evidence)?,
        Some(GapKind::EvidenceGap)
    );

    // `ONTOLOGY_GAP`: `P2-C3` says the identity is not attributable.
    let mut ontology = practised(concept, unknown_band(concept)?)?;
    ontology.identity = IdentityStanding::SplitAmbiguous {
        successors: vec![entity("DISK_PAGE_SENSE_A"), entity("DISK_PAGE_SENSE_B")],
    };
    ontology.remediation_description = "두 sense를 가르는 merge 검토".to_owned();
    assert_eq!(
        kind_of(&goal, &graph, &ontology)?,
        Some(GapKind::OntologyGap)
    );

    // `CONTEXT_GAP`: the goal has not chosen between two helpful branches.
    let branching = PrerequisiteGraph::new()
        .with(requires(
            buffer_pool(),
            disk_page(),
            PrerequisiteStrength::Hard,
            "edge-bp-dp",
        )?)
        .with(builds_on(
            disk_page(),
            random_io(),
            PrerequisiteStrength::Helpful,
            "edge-dp-rio",
        )?)
        .with(builds_on(
            disk_page(),
            fan_out(),
            PrerequisiteStrength::Helpful,
            "edge-dp-fo",
        )?);
    let context = practised(concept, unknown_band(concept)?)?;
    assert_eq!(
        kind_of(&goal, &branching, &context)?,
        Some(GapKind::ContextGap)
    );

    // The five are distinct: each of the five readings above produced a
    // different kind, so no two inputs collapse onto one answer.
    let produced: BTreeSet<GapKind> = [
        kind_of(&goal, &graph, &mastery)?,
        kind_of(&goal, &graph, &freshness)?,
        kind_of(&goal, &graph, &evidence)?,
        kind_of(&goal, &graph, &ontology)?,
        kind_of(&goal, &branching, &context)?,
    ]
    .into_iter()
    .flatten()
    .collect();
    assert_eq!(produced.len(), 5);

    // Exactly one of them is section 15.2 step 4's `강한 부족`.
    let strong: Vec<&str> = GAP_KINDS
        .iter()
        .filter(|kind| kind.is_strong_deficit())
        .map(|kind| kind.as_str())
        .collect();
    assert_eq!(strong, vec!["MASTERY_GAP"]);
    Ok(())
}

#[test]
fn the_step_six_prose_names_one_fewer_than_the_table() -> TestResult {
    // Section 15.2's sixth step names four informal kinds; the table below it
    // has five rows. Recorded rather than resolved: see `crate::kind`.
    let page = specification()?;
    let body = section(&page, "### 15.2 ")?;
    let step_six = numbered_step(&body, 6)?;
    for name in STEP_SIX_INFORMAL_NAMES {
        assert!(
            step_six.contains(name),
            "step 6 no longer names {name}: {step_six}"
        );
    }
    assert_eq!(STEP_SIX_INFORMAL_NAMES.len(), 4);
    assert_eq!(GAP_KINDS.len(), 5);
    assert_eq!(table_rows(&body)?.len(), 5);
    // `CONTEXT_GAP` is the row step 6 has no informal name for. Its own words
    // are what makes that checkable rather than asserted.
    assert!(
        !step_six.contains("context") && !step_six.contains("선택"),
        "step 6 now names the context kind: {step_six}"
    );

    // And step 6 is not the only prose in section 15. The record this crate
    // carries is that `CONTEXT_GAP` appears in the table and **in no prose
    // sentence at all**, which step 6 alone cannot say. Section 15's prose is
    // every line of 15.1, 15.2 and 15.3 that is not a table row, and each of
    // the five identifiers is looked for in both halves.
    let whole = section_15(&page)?;
    let (prose, table): (Vec<&str>, Vec<&str>) = whole
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .partition(|line| !line.starts_with('|'));
    let prose = prose.join(
        "
",
    );
    let table = table.join(
        "
",
    );
    let mut in_prose = Vec::new();
    for kind in GAP_KINDS {
        assert!(
            table.contains(kind.as_str()),
            "section 15's table no longer names {}",
            kind.as_str()
        );
        if prose.contains(kind.as_str()) {
            in_prose.push(kind.as_str());
        }
    }
    assert_eq!(
        in_prose,
        Vec::<&str>::new(),
        "section 15's prose now spells a gap identifier; the table was the only place any of them appeared: {in_prose:?}"
    );
    assert!(
        !prose.contains("맥락") && !prose.contains("context"),
        "section 15's prose now names the context kind, so step 6's four and the table's five may no longer be the mismatch this crate records"
    );

    // The four informal names live in the prose and nowhere in the table, which
    // is the other half of the same statement.
    for name in STEP_SIX_INFORMAL_NAMES {
        assert!(
            prose.contains(name),
            "section 15's prose no longer says {name}"
        );
        assert!(
            !table.contains(name),
            "section 15's table now says {name}, so the two halves have stopped being separate"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. `eight_field_explanation_is_complete`
// ---------------------------------------------------------------------------

#[test]
fn eight_field_explanation_is_complete() -> TestResult {
    // Eight is a measurement of section 15.3's own sentence, in both directions.
    let page = specification()?;
    let body = section(&page, "### 15.3 ")?;
    let sentence = body
        .lines()
        .find(|line| line.starts_with("모든 Gap 제안은"))
        .ok_or("section 15.3 has no explanation sentence")?;
    let designed = quoted(sentence);
    assert_eq!(designed.len(), EXPLANATION_FIELDS.len());
    for (index, field) in EXPLANATION_FIELDS.iter().enumerate() {
        assert_eq!(&designed[index], field);
    }
    let declared: BTreeSet<&str> = EXPLANATION_FIELDS.into_iter().collect();
    let from_document: BTreeSet<&str> = designed.iter().map(String::as_str).collect();
    assert_eq!(declared, from_document);
    assert_eq!(EXPLANATION_FIELDS.len(), 8);

    // A real explanation carries a value in each of the eight.
    let case = section_36_4_case()?;
    let explanation = case.roots()[0].explanation();
    assert_eq!(explanation.subject(), disk_page()); // 무엇
    assert_eq!(explanation.subject_kind(), EntityKind::Concept);
    assert_eq!(explanation.blocks().surface(), buffer_pool()); // 왜 막는가
    assert_eq!(explanation.blocks().steps().len(), 1);
    assert_eq!(
        explanation.blocks().steps()[0].strength(),
        PrerequisiteStrength::Hard
    );
    assert!(!explanation.evidence().is_empty()); // 근거
    assert!(explanation.confidence().value() <= 1000); // confidence
    assert_eq!(explanation.current_state().concept, disk_page()); // 현재 상태
    assert_eq!(explanation.remediation().minutes(), 25); // 최소 보강
    assert!(!explanation.remediation().sources().is_empty());
    assert_eq!(
        explanation.remediation().activity(),
        RemediationActivity::for_kind(GapKind::MasteryGap)
    );
    assert!(matches!(
        explanation.alternative(), // 대체 경로
        AlternativePath::None {
            reason: NoAlternativeReason::SoleHardPrerequisite
        }
    ));
    assert!(!explanation.linked().is_empty()); // 연결된 강의/프로젝트
    assert!(explanation.defects().is_empty());

    // Every field is on the wire, and the round trip keeps all eight.
    let encoded = serde_json::to_value(explanation.clone())?;
    let object = encoded.as_object().ok_or("not an object")?;
    for field in [
        "subject",
        "subject_kind",
        "blocks",
        "evidence",
        "confidence",
        "current_state",
        "remediation",
        "alternative",
        "linked",
    ] {
        assert!(object.contains_key(field), "the wire shape lacks {field}");
    }

    // And removing any one of them is a value that cannot be built: each defect
    // names the field of section 15.3 it is about.
    let covered: BTreeSet<&str> = SpecificityDefect::ALL
        .into_iter()
        .map(SpecificityDefect::field)
        .collect();
    for field in [
        "무엇",
        "왜 막는가",
        "근거",
        "최소 보강",
        "대체 경로",
        "연결된 강의/프로젝트",
    ] {
        assert!(covered.contains(field), "no defect is about {field}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 10. `generic_advice_fails_validation`
// ---------------------------------------------------------------------------

/// A recommendation a person would recognise as reasonable and a reader would
/// recognise as useless.
///
/// It uses **none** of section 15.3's own words. Its subject is a broad area, it
/// cites nothing, it states no duration, it names nothing to read, it offers no
/// alternative and it links no lecture and no project. Every one of those is a
/// structural fact and none of them is a phrase.
fn broad_recommendation() -> Result<ExplanationParts, Box<dyn Error>> {
    Ok(ExplanationParts {
        kind: GapKind::MasteryGap,
        subject: entity("DATABASE_SYSTEMS"),
        subject_kind: EntityKind::Field,
        blocks: academic_gap::BlockingPath::from_surface(buffer_pool()),
        evidence: Vec::new(),
        confidence: ConfidencePermille::new(900)?,
        current_state: academic_gap::StateSnapshot::from(&lowest_state(buffer_pool())?),
        remediation: MinimumRemediation::of(
            0,
            RemediationActivity::FoundationalExplanationProblemOrExperiment,
            "이 영역 전반에 대한 이해를 한 단계 끌어올리는 편이 좋겠습니다",
            Vec::new(),
        ),
        alternative: AlternativePath::Routes { routes: Vec::new() },
        linked: LinkedContext::default(),
    })
}

#[test]
fn generic_advice_fails_validation() -> TestResult {
    // Section 15.3's own example, so the rule being tested is the document's.
    let page = specification()?;
    let body = section(&page, "### 15.3 ")?;
    assert!(
        body.contains("너무 넓어 유효한 Gap 설명이 아니다"),
        "section 15.3 no longer refuses broad advice"
    );

    let parts = broad_recommendation()?;
    let refused = GapExplanation::of(parts.clone());
    let Err(GapError::NotSpecific(defects)) = refused else {
        return Err("a broad recommendation was accepted".into());
    };
    // Seven of the eight structural rules fire at once, and each names a field.
    assert_eq!(
        defects,
        vec![
            SpecificityDefect::SubjectCarriesNoPrerequisite,
            SpecificityDefect::BlockingPathDoesNotReachSubject,
            SpecificityDefect::NoEvidenceCited,
            SpecificityDefect::RemediationUnbounded,
            SpecificityDefect::RemediationUncited,
            SpecificityDefect::AlternativeIsEmpty,
            SpecificityDefect::NoLinkedContext,
        ]
    );

    // The refusal is not lexical. The same recommendation, reworded twice more
    // with no shared vocabulary, is refused identically.
    for wording in [
        "앞으로 몇 주에 걸쳐 이 분야의 기초를 다지시는 것을 권합니다",
        "관련 주제 전반을 폭넓게 살펴보시면 도움이 될 것입니다",
    ] {
        let mut reworded = parts.clone();
        reworded.remediation = MinimumRemediation::of(
            0,
            RemediationActivity::FoundationalExplanationProblemOrExperiment,
            wording,
            Vec::new(),
        );
        let Err(GapError::NotSpecific(again)) = GapExplanation::of(reworded) else {
            return Err(format!("`{wording}` was accepted").into());
        };
        assert_eq!(again, defects, "the refusal depended on the wording");
    }

    // The crate holds no vocabulary to have matched against. Section 15.3's
    // rejected sentence is Korean and so is every phrase a lexical validator
    // would need, so the measurement is: every non-ASCII string literal in this
    // crate's product sources is one of the design document's own cells. A
    // phrase list would appear below as an extra entry.
    let mut cells = 0_usize;
    for (path, code) in crate_sources()? {
        for literal in string_literals(&code) {
            if literal.is_ascii() {
                continue;
            }
            assert!(
                is_a_design_document_cell(&literal),
                "{path} holds the non-ASCII literal {literal:?}, which is not a cell                  the design document writes"
            );
            cells += 1;
        }
    }
    assert!(
        cells >= 18,
        "the reader found only {cells} design-document cells, so its silence about          anything else proves nothing"
    );

    // And the control on the other side: a specific explanation, in the same
    // shape, is accepted. The difference is not a word — it is a tier, a path, a
    // citation, a duration, a source and a link.
    let case = section_36_4_case()?;
    assert!(case.roots()[0].explanation().defects().is_empty());

    // Removing exactly one of those structural facts from the accepted
    // explanation is refused, one rule at a time.
    let good = accepted_parts()?;
    assert!(GapExplanation::of(good.clone()).is_ok());
    let mut mutated = good.clone();
    mutated.subject_kind = EntityKind::Field;
    assert_defect(&mutated, SpecificityDefect::SubjectCarriesNoPrerequisite);
    let mut mutated = good.clone();
    mutated.evidence.clear();
    assert_defect(&mutated, SpecificityDefect::NoEvidenceCited);
    let mut mutated = good.clone();
    mutated.remediation = MinimumRemediation::of(
        0,
        RemediationActivity::FoundationalExplanationProblemOrExperiment,
        "page layout을 직접 재는 실험",
        vec![evidence_id("lecture-segment-page-layout")],
    );
    assert_defect(&mutated, SpecificityDefect::RemediationUnbounded);
    let mut mutated = good.clone();
    mutated.remediation = MinimumRemediation::of(
        25,
        RemediationActivity::FoundationalExplanationProblemOrExperiment,
        "page layout을 직접 재는 25분짜리 실험",
        Vec::new(),
    );
    assert_defect(&mutated, SpecificityDefect::RemediationUncited);
    let mut mutated = good.clone();
    mutated.remediation = MinimumRemediation::of(
        25,
        RemediationActivity::OptionsAndConditionsClarified,
        "page layout을 직접 재는 25분짜리 실험",
        vec![evidence_id("lecture-segment-page-layout")],
    );
    assert_defect(&mutated, SpecificityDefect::RemediationDoesNotMatchKind);
    let mut mutated = good.clone();
    mutated.linked = LinkedContext::default();
    assert_defect(&mutated, SpecificityDefect::NoLinkedContext);
    let mut mutated = good;
    mutated.blocks = academic_gap::BlockingPath::from_surface(buffer_pool());
    assert_defect(&mutated, SpecificityDefect::BlockingPathDoesNotReachSubject);
    Ok(())
}

fn accepted_parts() -> Result<ExplanationParts, Box<dyn Error>> {
    let case = section_36_4_case()?;
    let explanation = case.roots()[0].explanation();
    Ok(ExplanationParts {
        kind: explanation.kind(),
        subject: explanation.subject(),
        subject_kind: explanation.subject_kind(),
        blocks: explanation.blocks().clone(),
        evidence: explanation.evidence().to_vec(),
        confidence: explanation.confidence(),
        current_state: explanation.current_state().clone(),
        remediation: explanation.remediation().clone(),
        alternative: explanation.alternative().clone(),
        linked: explanation.linked().clone(),
    })
}

fn assert_defect(parts: &ExplanationParts, expected: SpecificityDefect) {
    let Err(GapError::NotSpecific(found)) = GapExplanation::of(parts.clone()) else {
        unreachable!("expected a refusal naming {expected:?}")
    };
    assert!(
        found.contains(&expected),
        "expected {expected:?}, found {found:?}"
    );
}

/// Every double-quoted string literal in `code`, unescaped only enough to read.
fn string_literals(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    let mut in_line_comment = false;
    let mut in_block_comment = 0_usize;
    while index < chars.len() {
        let current = chars[index];
        let next = chars.get(index + 1).copied();
        if in_line_comment {
            if current == '\n' {
                in_line_comment = false;
            }
            index += 1;
            continue;
        }
        if in_block_comment > 0 {
            if current == '/' && next == Some('*') {
                in_block_comment += 1;
                index += 2;
                continue;
            }
            if current == '*' && next == Some('/') {
                in_block_comment -= 1;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if current == '/' && next == Some('/') {
            in_line_comment = true;
            index += 2;
            continue;
        }
        if current == '/' && next == Some('*') {
            in_block_comment = 1;
            index += 2;
            continue;
        }
        if current == '"' {
            let mut literal = String::new();
            index += 1;
            while index < chars.len() && chars[index] != '"' {
                if chars[index] == '\\' {
                    index += 1;
                }
                if index < chars.len() {
                    literal.push(chars[index]);
                }
                index += 1;
            }
            index += 1;
            found.push(literal);
            continue;
        }
        index += 1;
    }
    found
}

/// One of section 15.2's own table cells or section 15.3's own field names.
fn is_a_design_document_cell(literal: &str) -> bool {
    EXPLANATION_FIELDS.contains(&literal)
        || GAP_KINDS
            .iter()
            .any(|kind| kind.meaning() == literal || kind.response() == literal)
}

// ---------------------------------------------------------------------------
// The misattribution routes this task inherited.
// ---------------------------------------------------------------------------

#[test]
fn one_concepts_evidence_cannot_reach_another_concepts_deficit() -> TestResult {
    let concept = disk_page();
    let other = buffer_pool();

    // Route one. `academic_knowledge_state::project` accepts a slice about two
    // concepts and returns a projection carrying neither — observed first, then
    // refused here.
    let admitted_here = admit(exercise_evidence("mis-own"), "mis-own", concept)?;
    let admitted_elsewhere = admit(exercise_evidence("mis-other"), "mis-other", other)?;
    let mixed = academic_knowledge_state::project(&[admitted_here, admitted_elsewhere], &[])?;
    assert_eq!(
        mixed.level(),
        MasteryLevel::Practiced,
        "P2-N2's bare projection was supposed to accept the mixed slice"
    );
    assert_eq!(
        mixed.supporting().len(),
        2,
        "the mixed projection was supposed to carry both items"
    );
    // The same two items, offered to this crate for `concept`, are refused.
    let refused = ConceptState::overlay(
        concept,
        EntityKind::Concept,
        IdentityStanding::Settled,
        &[
            offered(
                exercise_evidence("mis-own"),
                "mis-own",
                full_dossier(concept),
            ),
            offered(
                exercise_evidence("mis-other"),
                "mis-other",
                full_dossier(other),
            ),
        ],
        &unknown_band(concept)?,
        &[],
    );
    assert!(matches!(
        refused,
        Err(GapError::EvidenceNamesAnotherConcept)
    ));

    // The two refusals above and below are two guards over disjoint halves. The
    // one above ran on an item `P2-N2` **admitted**, whose own `concept()` is
    // the answer; the one below runs on an item `P2-N2` **blocked**, which
    // does not retain a concept at all, so only the dossier still holds it.
    // Neither can stand in for the other.
    let blocked_elsewhere = EvidenceDossier::of(
        ConceptLink::Exact(other, EntityKind::Concept),
        Participation::Unknown,
        Outcome::Succeeded,
        SourceIntegrity::Verified(academic_domain::ContentDigest::sha256(b"artifact")),
    );
    let refused = ConceptState::overlay(
        concept,
        EntityKind::Concept,
        IdentityStanding::Settled,
        &[offered(
            exercise_evidence("mis-blocked"),
            "mis-blocked",
            blocked_elsewhere,
        )],
        &unknown_band(concept)?,
        &[],
    );
    assert!(matches!(
        refused,
        Err(GapError::EvidenceNamesAnotherConcept)
    ));

    // Route two. A projection about another concept, and a contribution computed
    // toward one.
    let refused = ConceptState::overlay(
        concept,
        EntityKind::Concept,
        IdentityStanding::Settled,
        &[],
        &unknown_band(other)?,
        &[],
    );
    assert!(matches!(
        refused,
        Err(GapError::FreshnessNamesAnotherConcept)
    ));
    Ok(())
}

fn admit(
    evidence: ConceptEvidence,
    tag: &str,
    concept: EntityId,
) -> Result<academic_knowledge_state::EligibleEvidence, Box<dyn Error>> {
    match EligibilityOutcome::admit(evidence, evidence_id(tag), &full_dossier(concept)) {
        EligibilityOutcome::Admitted(value) => Ok(value),
        EligibilityOutcome::Blocked(blocked) => {
            Err(format!("the fixture was blocked: {:?}", blocked.reasons()).into())
        }
    }
}

#[test]
fn a_band_raised_by_a_concept_on_the_blocking_path_is_refused() -> TestResult {
    // Section 36.4's own shape. `Buffer Pool` is the active surface, so it is the
    // concept the user is using now; `Disk Page` is one `REQUIRES` hop below it
    // and section 36.4's answer is that `Disk Page` **is** the root gap.
    let edge = CitedEdge::of(
        PredicateName::Requires,
        buffer_pool(),
        disk_page(),
        vec![evidence_id("edge-buffer-pool-disk-page")],
    )
    .ok_or("the spillover edge was refused")?;
    let recent = dated_exercise(buffer_pool(), "surface-use", 0)?;
    let use_ = NeighborUse::direct(edge, buffer_pool(), &[recent], prior(), at(0))
        .ok_or("the neighbour use was refused")?;
    let contribution =
        Spillover::toward(disk_page(), use_).ok_or("the contribution was refused")?;

    // Observed first: `P2-N3` licenses this, and it puts `Disk Page` at
    // `MODERATE` on the surface concept's evidence and nothing of its own.
    let contaminated = band_from(
        disk_page(),
        &[],
        std::slice::from_ref(&contribution),
        at(0),
        FreshnessBand::Moderate,
    )?;
    assert!(
        academic_freshness::rank(contaminated.band()) >= academic_freshness::rank(RETRIEVAL_FLOOR),
        "the contaminated band was supposed to clear the retrieval floor"
    );
    // Without the contribution the same concept reads `UNKNOWN`, so the band is
    // entirely the neighbour's.
    assert_eq!(unknown_band(disk_page())?.band(), FreshnessBand::Unknown);

    // The overlay itself accepts it — it does not know the path yet.
    let state = ConceptState::overlay(
        disk_page(),
        EntityKind::Concept,
        IdentityStanding::Settled,
        &[],
        &contaminated,
        std::slice::from_ref(&contribution),
    )?;
    assert_eq!(state.spillover_sources().len(), 1);
    assert_eq!(state.spillover_sources()[0].neighbor, buffer_pool());

    // The search refuses it, naming the neighbour and the edge, because
    // `Buffer Pool` is on `Disk Page`'s own blocking path.
    let goal = understand_buffer_pool()?;
    let graph = PrerequisiteGraph::new().with(requires(
        buffer_pool(),
        disk_page(),
        PrerequisiteStrength::Hard,
        "edge-buffer-pool-disk-page",
    )?);
    let mut contaminated_reading = reading(disk_page(), contaminated);
    contaminated_reading.spillover = vec![contribution.clone()];
    let refused = search(&goal, &graph, &[contaminated_reading], None);
    assert!(
        matches!(
            refused,
            Err(GapError::FreshnessRestsOnPathSpillover {
                concept,
                neighbor,
                predicate: "REQUIRES",
            }) if concept == disk_page() && neighbor == buffer_pool()
        ),
        "the contaminated band was accepted: {refused:?}"
    );

    // A contribution from a concept that is **not** on the path is untouched, so
    // the refusal is about the path and not about spillover as such.
    let off_path_edge = CitedEdge::of(
        PredicateName::RelatedTo,
        disk_page(),
        fan_out(),
        vec![evidence_id("edge-disk-page-fan-out")],
    )
    .ok_or("the off-path edge was refused")?;
    let off_path_use = NeighborUse::direct(
        off_path_edge,
        fan_out(),
        &[dated_exercise(fan_out(), "off-path", 0)?],
        prior(),
        at(0),
    )
    .ok_or("the off-path use was refused")?;
    let off_path =
        Spillover::toward(disk_page(), off_path_use).ok_or("the off-path contribution")?;
    let clean = band_from(
        disk_page(),
        &[],
        std::slice::from_ref(&off_path),
        at(0),
        FreshnessBand::Moderate,
    )?;
    let mut clean_reading = reading(disk_page(), clean);
    clean_reading.spillover = vec![off_path];
    let found = search(&goal, &graph, &[clean_reading], None)?;
    assert!(
        found.is_some(),
        "an off-path contribution was refused along with the on-path one"
    );
    assert_eq!(
        found
            .as_ref()
            .and_then(|case| case.candidates().first())
            .map(academic_gap::RootCandidate::kind),
        Some(GapKind::EvidenceGap),
        "the off-path case was supposed to route on its own empty evidence"
    );
    Ok(())
}

#[test]
fn a_projection_cannot_hide_a_contribution_it_used() -> TestResult {
    let off_path_edge = CitedEdge::of(
        PredicateName::RelatedTo,
        disk_page(),
        fan_out(),
        vec![evidence_id("edge-disk-page-fan-out")],
    )
    .ok_or("the edge was refused")?;
    let use_ = NeighborUse::direct(
        off_path_edge,
        fan_out(),
        &[dated_exercise(fan_out(), "hidden", 0)?],
        prior(),
        at(0),
    )
    .ok_or("the use was refused")?;
    let contribution = Spillover::toward(disk_page(), use_).ok_or("the contribution")?;
    let projected = band_from(
        disk_page(),
        &[],
        std::slice::from_ref(&contribution),
        at(0),
        FreshnessBand::Moderate,
    )?;

    // Declaring none of it is refused.
    let refused = ConceptState::overlay(
        disk_page(),
        EntityKind::Concept,
        IdentityStanding::Settled,
        &[],
        &projected,
        &[],
    );
    assert!(matches!(refused, Err(GapError::SpilloverNotDeclared)));

    // Declaring it is accepted.
    assert!(
        ConceptState::overlay(
            disk_page(),
            EntityKind::Concept,
            IdentityStanding::Settled,
            &[],
            &projected,
            std::slice::from_ref(&contribution),
        )
        .is_ok()
    );
    Ok(())
}

#[test]
fn an_unsettled_identity_stops_the_descent() -> TestResult {
    let goal = understand_buffer_pool()?;
    let graph = section_36_4_graph()?;
    let mut split = practised(disk_page(), unknown_band(disk_page())?)?;
    split.identity = IdentityStanding::SenseUnresolved {
        senses: vec![entity("DISK_PAGE_IO"), entity("DISK_PAGE_LAYOUT")],
    };
    split.remediation_description = "두 sense를 가르는 검토".to_owned();
    // `Storage Hierarchy` sits below it and would be a `MASTERY_GAP` if reached.
    let deeper = with_evidence(
        reading(storage_hierarchy(), unknown_band(storage_hierarchy())?),
        vec![offered(
            exposure_evidence("sh-below-split")?,
            "sh-below-split",
            full_dossier(storage_hierarchy()),
        )],
    );
    let case = search(&goal, &graph, &[split, deeper], None)?.ok_or("no case")?;
    assert_eq!(case.candidates().len(), 1);
    assert_eq!(case.candidates()[0].kind(), GapKind::OntologyGap);
    assert!(
        case.roots().is_empty(),
        "an ONTOLOGY_GAP became a root deficit"
    );
    Ok(())
}

#[test]
fn the_descent_never_guesses_a_state() -> TestResult {
    let goal = understand_buffer_pool()?;
    let graph = section_36_4_graph()?;
    // A reading for `Disk Page` and none for `Storage Hierarchy`.
    let only_one = vec![with_evidence(
        reading(disk_page(), unknown_band(disk_page())?),
        vec![offered(
            exposure_evidence("dp-alone")?,
            "dp-alone",
            full_dossier(disk_page()),
        )],
    )];
    let refused = search(&goal, &graph, &only_one, None);
    assert!(matches!(
        refused,
        Err(GapError::NoReadingForConcept(concept)) if concept == storage_hierarchy()
    ));
    Ok(())
}

#[test]
fn the_tiers_that_carry_no_prerequisite_are_the_two_p2_c3_names() -> TestResult {
    for kind in [
        EntityKind::Concept,
        EntityKind::ConceptSense,
        EntityKind::Operation,
    ] {
        assert!(gap_bearing(kind), "{kind:?} was refused as a gap subject");
    }
    for kind in [EntityKind::Field, EntityKind::Alias] {
        assert!(!gap_bearing(kind), "{kind:?} was admitted as a gap subject");
    }
    // A criterion naming one of the two is refused at the goal.
    assert!(matches!(
        SuccessCriterion::concept(
            entity("DATABASE_SYSTEMS"),
            EntityKind::Field,
            MasteryLevel::Practiced
        ),
        Err(GapError::CriterionSubjectCarriesNoPrerequisite { .. })
    ));
    Ok(())
}
