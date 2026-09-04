//! `P2-L6`'s acceptance suite: section 12.7's four named rows, and eight more.
//!
//! Everything runs on the design document's own scenario. Section 36.4's chain
//! is section 12.7's example — `Buffer Pool` is `Tomorrow`, `Disk Page` is the
//! `blocking evidence` line at `mastery: Exposed`, and `Storage Hierarchy` sits
//! one hop below it — so no fixture here invents a lecture or a state. Bands
//! come from `P2-N3`'s `project`, mastery from `P2-N2`'s eligibility checks,
//! the descent from `P2-N5`'s `search`, and every claim from a `P2-G5`
//! `Proposal` that `adjudicate` produced over bytes this suite ingested.
//!
//! ## The four named rows
//!
//! 1. `expected_concept_source_matrix`
//! 2. `minimum_blocking_preparation`
//! 3. `prep_uncertainty_factorization`
//! 4. `morning_home_contract`
//!
//! Each is followed by the tests that keep it from being vacuous.

mod support;

use std::{collections::BTreeSet, error::Error};

use academic_domain::{
    ConfidencePermille, EntityId, EpistemicStatus, FreshnessBand, MasteryLevel,
    entity_registry::EntityKind, predicates::PrerequisiteStrength,
};
use academic_freshness::DatedEvidence;
use academic_gap::{ConceptReading, GapKind, PrerequisiteGraph, RETRIEVAL_FLOOR, search};
use academic_knowledge_state::EligibilityOutcome;
use academic_next_lecture::{
    CandidateParts, EXPECTED_CONCEPT_SOURCES, ExpectedConceptClaim, ExpectedConceptSource,
    HIGHEST_PREPARATION, LOWEST_PREPARATION, MaterialReference, MinimalityDefect, NextLectureError,
    PREP_AXES, PrepAxis, PrepUncertainty, PreparationBrief, PreparationCandidate,
    minimality_defects, propose,
};
use academic_untrusted_content::SourceKind;

use support::{
    TestResult, at, band_from, buffer_pool, case_over, claim_about, claim_from, disk_page,
    edge_confidence, ending_node, entity, evidence_id, exercise_evidence, exposure_evidence,
    full_dossier, goal_at, hard_edge, ingest_material, material_date, material_reference,
    material_text, offered, overlay_of, overlays, proposal_over, random_io, reading, requires,
    section, section_12_7_readings, specification, storage_hierarchy, unknown_band,
};

/// The concept nothing in section 12.7's minimum preparation may reach: it sits
/// **above** `Buffer Pool` rather than beneath it, which is what makes it the
/// `advanced replacement-policy survey` without this suite spelling the phrase.
fn replacement_policy() -> EntityId {
    entity("REPLACEMENT_POLICY")
}

/// Section 36.4's chain, which is section 12.7's own `Expected:` line.
fn graph() -> Result<PrerequisiteGraph, Box<dyn Error>> {
    Ok(PrerequisiteGraph::new()
        .with(hard_edge(buffer_pool(), disk_page(), "edge-bp-dp")?)
        .with(requires(
            disk_page(),
            storage_hierarchy(),
            PrerequisiteStrength::Strong,
            "edge-dp-sh",
        )?))
}

/// The concept the survey topic is itself a foundation for.
///
/// A descent has to start somewhere above the topic for the topic to be a
/// candidate at all, and this is that somewhere. It is never an expected
/// concept in any fixture.
fn query_optimization() -> EntityId {
    entity("QUERY_OPTIMIZATION")
}

/// The same chain with the survey topic hanging above tomorrow's lecture, and
/// one concept above that so the topic can be descended to.
fn graph_with_advanced() -> Result<PrerequisiteGraph, Box<dyn Error>> {
    Ok(graph()?
        .with(hard_edge(
            replacement_policy(),
            buffer_pool(),
            "edge-rp-bp",
        )?)
        .with(hard_edge(
            query_optimization(),
            replacement_policy(),
            "edge-qo-rp",
        )?))
}

// ---------------------------------------------------------------------------
// 1. expected_concept_source_matrix
// ---------------------------------------------------------------------------

/// Section 12.7's first sentence, read back and compared in both directions,
/// and then driven once per place.
///
/// Seven is not a number this suite chose. The sentence is split on the
/// document's own `, `, the cells are compared with `EXPECTED_CONCEPT_SOURCES`
/// in order and as sets, and then **every matched cell is removed from the
/// sentence and what is left is required to be separators**. An eighth place
/// leaves text behind and fails here rather than being folded into the nearest
/// arm, which is `P2-X2`'s reading of section 25.2's four permission words.
#[test]
fn expected_concept_source_matrix() -> TestResult {
    let page = specification()?;
    let block = section(&page, "### 12.7 ")?;
    let sentence = block
        .lines()
        .find(|line| line.contains("`ExpectedConceptClaim`"))
        .ok_or("section 12.7 does not name ExpectedConceptClaim")?;
    let listed = sentence
        .split_once("에서 `ExpectedConceptClaim`")
        .ok_or("section 12.7 does not extract from a list of places")?
        .0;
    let cells: Vec<&str> = listed.split(", ").map(str::trim).collect();

    // In the sentence's own order.
    let declared: Vec<&str> = EXPECTED_CONCEPT_SOURCES
        .iter()
        .map(|place| place.spec_token())
        .collect();
    assert_eq!(
        cells, declared,
        "section 12.7's places and EXPECTED_CONCEPT_SOURCES differ"
    );
    // And as sets, so a repeated cell cannot pass the ordered comparison by
    // accident.
    let designed: BTreeSet<&str> = cells.iter().copied().collect();
    let held: BTreeSet<&str> = declared.iter().copied().collect();
    assert_eq!(designed, held, "the two sets differ");
    assert_eq!(
        designed.len(),
        EXPECTED_CONCEPT_SOURCES.len(),
        "section 12.7 names a place twice"
    );

    // Nothing but separators is left over. This is what refuses an eighth
    // place, and it is what fixes `다음 title/slide` as one cell: reading its
    // slash as a separator would leave `다음 title` and `slide` unmatched.
    let mut remainder = listed.to_owned();
    for place in EXPECTED_CONCEPT_SOURCES {
        let token = place.spec_token();
        let at = remainder
            .find(token)
            .ok_or_else(|| format!("section 12.7 no longer names {token}"))?;
        remainder.replace_range(at..at + token.len(), "");
    }
    assert!(
        remainder
            .chars()
            .all(|character| character == ',' || character.is_whitespace()),
        "section 12.7's list holds a place nothing here accounts for: {remainder:?}"
    );

    // The parse is not vacuous: the seven cells together are most of the
    // sentence, and no cell is empty.
    assert!(cells.len() >= 2, "the split found one cell");
    assert!(
        cells.iter().all(|cell| !cell.is_empty()),
        "a parsed cell is empty"
    );

    // Every place is drivable, and each answer carries its own place, its own
    // material, its own date, and the one standing an extraction has.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for place in EXPECTED_CONCEPT_SOURCES {
        let claim = claim_from(place)?;
        assert_eq!(claim.material().source(), place);
        assert_eq!(claim.material().published(), material_date()?);
        assert_eq!(claim.concept(), buffer_pool());
        assert_eq!(claim.standing(), EpistemicStatus::AiInferred);
        assert!(
            !claim.citations().is_empty(),
            "{place:?} produced a claim citing nothing"
        );
        // The citation points into that place's own document and nobody
        // else's, so seven claims are seven materials rather than one repeated.
        for span in claim.citations() {
            assert_eq!(span.source_id(), claim.material().document());
        }
        assert!(
            seen.insert(claim.material().document().as_str().to_owned()),
            "two places resolved to one document"
        );
        // The seventh place cites the `P2-L4` node it ended at, and the other
        // six cite none.
        assert_eq!(
            claim.material().lecture_node().is_some(),
            place.is_recorded_by_this_system(),
            "{place:?} disagrees with its document-node rule"
        );
    }
    assert_eq!(seen.len(), EXPECTED_CONCEPT_SOURCES.len());
    Ok(())
}

/// Each of the two document-node rules, in the direction that fails.
///
/// The pair is what makes the seventh place *the seventh place* rather than a
/// name: a lecture ending with no node is refused, and any other place carrying
/// one is refused, so `PRIOR_LECTURE_ENDING` cannot be relabelled onto a
/// syllabus without the value ceasing to exist.
#[test]
fn only_the_prior_lecture_ending_cites_a_document_node() -> TestResult {
    for place in EXPECTED_CONCEPT_SOURCES {
        let material = ingest_material(
            &format!("node-rule-{}", place.as_str().to_lowercase()),
            SourceKind::Readme,
            &material_text(place),
        )?;
        let with_node = MaterialReference::of(
            place,
            material.id.clone(),
            material_date()?,
            Some(ending_node()?),
        );
        let without = MaterialReference::of(place, material.id.clone(), material_date()?, None);
        if place.is_recorded_by_this_system() {
            assert!(with_node.is_ok(), "{place:?} refused its own node");
            assert!(
                matches!(
                    without,
                    Err(NextLectureError::PriorLectureEndingNeedsItsDocumentNode)
                ),
                "{place:?} was admitted with no document node"
            );
        } else {
            assert!(without.is_ok(), "{place:?} was refused without a node");
            assert!(
                matches!(
                    with_node,
                    Err(NextLectureError::OnlyThePriorLectureEndingCitesADocumentNode { .. })
                ),
                "{place:?} was admitted carrying a document node"
            );
        }
    }
    Ok(())
}

/// A claim whose declared material is not the one the model quoted is refused.
///
/// Without this, `ExpectedConceptSource` would be a label a caller writes on any
/// citation at all, and the seven-place matrix would be seven names over one
/// document.
#[test]
fn a_claim_quotes_the_material_it_names() -> TestResult {
    let quoted = ingest_material(
        "quoted-syllabus",
        SourceKind::Syllabus,
        &material_text(ExpectedConceptSource::Syllabus),
    )?;
    let other = ingest_material(
        "unquoted-notice",
        SourceKind::Readme,
        &material_text(ExpectedConceptSource::Notice),
    )?;
    let proposal = proposal_over(&quoted, 0, 31, "week 6 covers buffer management")?;

    // The control: the material the proposal actually cites is admitted.
    assert!(
        ExpectedConceptClaim::extract(
            buffer_pool(),
            EntityKind::Concept,
            material_reference(ExpectedConceptSource::Syllabus, quoted.id.clone())?,
            &proposal,
            ConfidencePermille::new(720)?,
        )
        .is_ok(),
        "the quoted material was refused"
    );
    // And a second document nothing in the proposal points into is not.
    let refused = ExpectedConceptClaim::extract(
        buffer_pool(),
        EntityKind::Concept,
        material_reference(ExpectedConceptSource::Notice, other.id.clone())?,
        &proposal,
        ConfidencePermille::new(720)?,
    );
    assert!(
        matches!(
            refused,
            Err(NextLectureError::ClaimDoesNotQuoteItsMaterial { .. })
        ),
        "a claim was admitted over a material nothing cited"
    );

    // A tier that carries no prerequisite of its own is refused where the claim
    // is made, in `P2-C3`'s own words rather than by a name list here.
    let field = ExpectedConceptClaim::extract(
        buffer_pool(),
        EntityKind::Field,
        material_reference(ExpectedConceptSource::Syllabus, quoted.id)?,
        &proposal,
        ConfidencePermille::new(720)?,
    );
    assert!(
        matches!(
            field,
            Err(NextLectureError::ExpectedConceptCarriesNoPrerequisite { .. })
        ),
        "a FIELD was admitted as an expected concept"
    );
    Ok(())
}

/// The standing is `AI_INFERRED`, and the crate has no way to say otherwise.
///
/// Three whole sets rather than a list of names: every public signature that
/// mentions `EpistemicStatus`, every variant of it any product file spells, and
/// every public return type, required to name no `academic_knowledge_state`
/// type at all. The last is the one that matters — a crate that could produce
/// the evidence a mastery promotion reads would have made the candidate
/// standing a formality.
#[test]
fn an_extracted_claim_is_never_confirmed() -> TestResult {
    assert_eq!(ExpectedConceptClaim::STANDING, EpistemicStatus::AiInferred);
    assert_eq!(
        claim_from(ExpectedConceptSource::Syllabus)?.standing(),
        EpistemicStatus::AiInferred
    );

    let mut mentioning: Vec<(String, String, String)> = Vec::new();
    let mut variants: BTreeSet<String> = BTreeSet::new();
    let mut promoting: Vec<(String, String)> = Vec::new();
    for (path, code) in support::product_code()? {
        for (owner, name, signature) in support::public_signatures_with_owner(&code) {
            if signature.contains("EpistemicStatus") {
                mentioning.push((path.clone(), owner, name.clone()));
            }
            if signature
                .split_once("->")
                .is_some_and(|(_, returns)| returns.contains("academic_knowledge_state"))
            {
                promoting.push((path.clone(), name));
            }
        }
        for spelled in support::absolute_paths(&code) {
            if let Some(variant) = spelled.strip_prefix("EpistemicStatus::") {
                variants.insert(variant.to_owned());
            }
        }
        for at in code.match_indices("EpistemicStatus::") {
            let variant: String = code[at.0 + "EpistemicStatus::".len()..]
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if !variant.is_empty() {
                variants.insert(variant);
            }
        }
    }
    assert_eq!(
        mentioning,
        vec![(
            "crates/next-lecture/src/claim.rs".to_owned(),
            "ExpectedConceptClaim".to_owned(),
            "standing".to_owned(),
        )],
        "a second public signature names EpistemicStatus"
    );
    assert_eq!(
        variants,
        BTreeSet::from(["AiInferred".to_owned()]),
        "a product file spells an EpistemicStatus variant that is not AiInferred"
    );
    assert_eq!(
        promoting,
        Vec::<(String, String)>::new(),
        "a public signature returns a knowledge-state type"
    );
    // The scanner is not vacuous: it found the accessor it was supposed to,
    // and the crate's public surface is not empty.
    let signatures: usize = support::product_code()?
        .iter()
        .map(|(_, code)| support::public_signatures(code).len())
        .sum();
    assert!(
        signatures >= 30,
        "the signature scanner found only {signatures} public functions"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. minimum_blocking_preparation
// ---------------------------------------------------------------------------

/// Section 12.7's `최소 기초만` and its `not included` line, driven with
/// proposals that spell none of the document's words.
///
/// Three shapes, each fluent and each entirely reasonable-sounding, each
/// refused on a graph fact and not on a phrase:
///
/// * a foundation the descent from tomorrow's concept never reached;
/// * a topic the graph puts **above** tomorrow's concept;
/// * a concept the descent reached and routed to a kind that is not
///   `강한 부족`.
///
/// The first two are distinguishable in the answer, which is what makes the
/// refusal attributable rather than a single blanket `too broad`.
#[test]
fn minimum_blocking_preparation() -> TestResult {
    let graph = graph()?;
    let readings = section_12_7_readings()?;
    let goal = goal_at(buffer_pool(), "goal-tomorrow-buffer-management")?;
    let case = case_over(&goal, &graph, &readings)?;
    let claim = claim_from(ExpectedConceptSource::NextTitleOrSlide)?;
    let states = overlays(&readings)?;

    // The control: section 12.7's own candidate is admitted with no defect.
    let root = case
        .candidates()
        .iter()
        .find(|candidate| candidate.concept() == disk_page())
        .ok_or("the descent found no Disk Page candidate")?;
    assert_eq!(minimality_defects(&claim, root, &graph), Vec::new());
    let state = states
        .iter()
        .find(|state| state.concept() == disk_page())
        .ok_or("no overlay for Disk Page")?;
    assert!(
        PreparationCandidate::of(
            CandidateParts {
                claim: &claim,
                root,
                state,
                edge_confidence: edge_confidence()?,
            },
            &graph,
        )
        .is_ok(),
        "the minimum preparation was refused"
    );

    // Shape one: a lucid, well-cited proposal for a foundation the descent
    // never reached. `Random IO` is a real concept with a real preparation, and
    // it is not on any blocking descent from tomorrow's lecture.
    let elsewhere =
        PrerequisiteGraph::new().with(hard_edge(random_io(), storage_hierarchy(), "edge-rio-sh")?);
    let elsewhere_goal = goal_at(random_io(), "goal-random-io")?;
    let elsewhere_readings = vec![
        reading(random_io(), unknown_band(random_io())?),
        support::blocked_reading(storage_hierarchy(), "elsewhere-storage")?,
    ];
    let elsewhere_case = case_over(&elsewhere_goal, &elsewhere, &elsewhere_readings)?;
    let unreached = elsewhere_case
        .candidates()
        .first()
        .ok_or("the elsewhere descent produced no candidate")?;
    assert_eq!(
        minimality_defects(&claim, unreached, &graph),
        vec![MinimalityDefect::NotReachedFromTheExpectedConcept],
        "an unrelated foundation was not refused, or was refused for the wrong reason"
    );

    // Shape two: a proposal one level *up*. Its own preparation is impeccable,
    // and the graph holds tomorrow's concept as a prerequisite **of** it, so
    // the answer carries a second defect the shape above does not have.
    let advanced_graph = graph_with_advanced()?;
    let advanced_goal = goal_at(query_optimization(), "goal-query-optimization")?;
    let mut advanced_readings = section_12_7_readings()?;
    advanced_readings.push(reading(
        query_optimization(),
        unknown_band(query_optimization())?,
    ));
    advanced_readings.push(support::blocked_reading(
        replacement_policy(),
        "advanced-replacement",
    )?);
    let advanced_case = case_over(&advanced_goal, &advanced_graph, &advanced_readings)?;
    let survey = advanced_case
        .candidates()
        .iter()
        .find(|candidate| candidate.concept() == replacement_policy())
        .ok_or("the advanced descent produced no candidate for the survey topic")?;
    let survey_claim = claim_about(ExpectedConceptSource::TextbookChapter, buffer_pool())?;

    // The control for shape two: over the *same* graph, an ordinary descent
    // from tomorrow's concept still admits its own foundation, so the extra
    // edge alone did not refuse everything.
    let ordinary_goal = goal_at(buffer_pool(), "goal-ordinary-over-advanced")?;
    let ordinary_case = case_over(&ordinary_goal, &advanced_graph, &advanced_readings)?;
    let ordinary = ordinary_case
        .candidates()
        .iter()
        .find(|candidate| candidate.concept() == disk_page())
        .ok_or("the ordinary descent produced no Disk Page candidate")?;
    assert_eq!(
        minimality_defects(&survey_claim, ordinary, &advanced_graph),
        Vec::new()
    );

    assert_eq!(
        minimality_defects(&survey_claim, survey, &advanced_graph),
        vec![
            MinimalityDefect::NotReachedFromTheExpectedConcept,
            MinimalityDefect::BeyondTheExpectedConcept,
        ],
        "an advanced survey was not distinguished from an unrelated topic"
    );

    // Shape three: a concept the descent *did* reach, whose own reading is a
    // recall question rather than a missing foundation. Section 15.2 calls that
    // `즉시 사용 불확실`, and section 12.7 asks for what is `가능성이 큰`.
    let (fresh_graph, fresh_case, fresh_readings) = freshness_case()?;
    let stale = fresh_case
        .candidates()
        .iter()
        .find(|candidate| candidate.concept() == disk_page())
        .ok_or("the freshness descent produced no Disk Page candidate")?;
    assert_eq!(stale.kind(), GapKind::FreshnessGap);
    assert_eq!(
        minimality_defects(&claim, stale, &fresh_graph),
        vec![MinimalityDefect::NotALikelyBlock],
        "a refresher was proposed as a blocking foundation"
    );
    let fresh_states = overlays(&fresh_readings)?;
    let fresh_state = fresh_states
        .iter()
        .find(|state| state.concept() == disk_page())
        .ok_or("no overlay for the stale Disk Page")?;
    let refused = PreparationCandidate::of(
        CandidateParts {
            claim: &claim,
            root: stale,
            state: fresh_state,
            edge_confidence: edge_confidence()?,
        },
        &fresh_graph,
    );
    assert!(
        matches!(refused, Err(NextLectureError::NotMinimal(defects))
            if defects == vec![MinimalityDefect::NotALikelyBlock]),
        "the constructor admitted a refresher"
    );

    // Every defect is reachable, so none of the three is a branch nothing
    // exercises.
    let exercised: BTreeSet<MinimalityDefect> = [
        minimality_defects(&claim, unreached, &graph),
        minimality_defects(&survey_claim, survey, &advanced_graph),
        minimality_defects(&claim, stale, &fresh_graph),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert_eq!(
        exercised,
        BTreeSet::from(MinimalityDefect::ALL),
        "a minimality defect was never produced by any injection"
    );
    Ok(())
}

/// A `Disk Page` that is practised but not retrievable, so `P2-N5` routes it to
/// `FRESHNESS_GAP` rather than to a missing foundation.
#[allow(clippy::type_complexity)]
fn freshness_case() -> Result<
    (
        PrerequisiteGraph,
        academic_gap::GapCase,
        Vec<ConceptReading>,
    ),
    Box<dyn Error>,
> {
    let graph = PrerequisiteGraph::new().with(requires(
        buffer_pool(),
        disk_page(),
        PrerequisiteStrength::Strong,
        "edge-bp-dp-strong",
    )?);
    let concept = disk_page();
    let stale = dated_exercise(concept, "stale", -400)?;
    let mut page = reading(
        concept,
        band_from(concept, &[stale], &[], at(0), FreshnessBand::Stale)?,
    );
    page.offered = vec![
        offered(
            exercise_evidence(&format!("{concept}-fresh-a")),
            &format!("{concept}-fresh-a"),
            full_dossier(concept),
        ),
        offered(
            exercise_evidence(&format!("{concept}-fresh-b")),
            &format!("{concept}-fresh-b"),
            full_dossier(concept),
        ),
    ];
    let mut buffer = reading(buffer_pool(), unknown_band(buffer_pool())?);
    buffer.offered = vec![offered(
        exposure_evidence("lecture-buffer-pool-fresh")?,
        "evidence-buffer-pool-fresh",
        full_dossier(buffer_pool()),
    )];
    let readings = vec![buffer, page];
    let goal = goal_at(buffer_pool(), "goal-freshness")?;
    let case = search(&goal, &graph, &readings, None)?
        .ok_or("the freshness fixture produced no gap at all")?;
    // The reading is what the fixture says it is: below the retrieval floor,
    // and at or above the edge's own mastery floor.
    assert!(
        academic_freshness::rank(FreshnessBand::Stale) < academic_freshness::rank(RETRIEVAL_FLOOR)
    );
    Ok((graph, case, readings))
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

/// This crate refuses breadth on structure, so it holds no phrase to match.
///
/// Two whole sets, neither a list of forbidden words.
///
/// **Every non-ASCII string literal** in every product file is required to
/// occur in the design document verbatim. A validator that grew a list of broad
/// Korean phrases would put a literal here the document does not hold, whatever
/// the list was called.
///
/// **Every method any product file calls** is compared with a pinned inventory
/// in both directions. That is the half a literal check cannot make: an
/// English-language phrase list would be ASCII and would pass the first set, and
/// it could only be *used* through a comparison — `contains`, `starts_with`,
/// `eq_ignore_ascii_case`, `to_lowercase`, `matches` — so the inventory is what
/// refuses it. `P2-R2` measured why the other shape fails: a list of eleven
/// forbidden names beside a whole-set comparison, and the eleven were the half
/// that let an edit through.
#[test]
fn the_next_lecture_crate_holds_no_phrase_list() -> TestResult {
    let page = specification()?;
    let mut cells: Vec<(String, String)> = Vec::new();
    for path in support::crate_product_sources()? {
        let source = std::fs::read_to_string(&path)?;
        for literal in string_literals(&source) {
            if literal.is_ascii() {
                continue;
            }
            cells.push((support::relative(&path), literal));
        }
    }
    assert!(
        cells.len() >= 9,
        "the literal reader found only {} cells",
        cells.len()
    );
    for (path, literal) in &cells {
        assert!(
            page.contains(literal.as_str()),
            "{path} holds a phrase the design document does not: {literal:?}"
        );
    }

    // Every method this crate's product code calls, as one set.
    let mut called: BTreeSet<String> = BTreeSet::new();
    for (_, code) in support::product_code()? {
        called.extend(method_calls(&code));
    }
    let expected: BTreeSet<String> = [
        // Reading a `P2-G5` proposal's citations and copying them.
        "support",
        "source_id",
        "cloned",
        "collect",
        "iter",
        "filter",
        "is_empty",
        "to_vec",
        "to_owned",
        "as_ref",
        "clone",
        // Reading a `P2-N5` candidate, its path and its explanation.
        "concept",
        "kind",
        "blocking_path",
        "explanation",
        "remediation",
        "reason",
        "is_strong_deficit",
        "surface",
        "steps",
        "last",
        "advanced",
        "prerequisite",
        "predicate",
        "strength",
        "evidence",
        "blocking_out_of",
        "into_iter",
        "find",
        "candidates",
        "surface_concept",
        "confidence",
        "mastery",
        "freshness",
        "supporting",
        "contradicting",
        "citations",
        "material",
        "document",
        "source",
        "is_recorded_by_this_system",
        // Structure.
        "push",
        "contains",
        "len",
        "enumerate",
        "any",
        "as_bytes",
        "pop",
        "ok_or",
        "map",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        called, expected,
        "this crate's product code calls a method the inventory does not hold"
    );
    // `contains` is in the set and it is not a text comparison: every call of
    // it is enumerated with the receiver it is called on, and each has to be a
    // range or a slice of identifiers rather than a string.
    let mut receivers: Vec<(String, String)> = Vec::new();
    for (path, code) in support::product_code()? {
        for at in code.match_indices(".contains(") {
            let before = &code[..at.0];
            // The receiver runs back to the first character that cannot be part
            // of one expression. `..=` and a parenthesised range are part of it;
            // whitespace, a separator and a unary `!` are not.
            let start = before
                .rfind(|character: char| {
                    !(character.is_alphanumeric()
                        || matches!(character, '_' | '.' | '=' | '(' | ')'))
                })
                .map_or(0, |offset| offset + 1);
            receivers.push((path.clone(), before[start..].to_owned()));
        }
    }
    assert_eq!(
        receivers,
        vec![
            (
                "crates/next-lecture/src/brief.rs".to_owned(),
                "(LOWEST_PREPARATION..=HIGHEST_PREPARATION)".to_owned(),
            ),
            (
                "crates/next-lecture/src/minimality.rs".to_owned(),
                "seen".to_owned(),
            ),
        ],
        "a `contains` call in this crate reads something other than a range or an identity set"
    );
    Ok(())
}

/// Every method name `code` calls, as one set.
fn method_calls(code: &str) -> BTreeSet<String> {
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    for (at, _) in code.match_indices('.') {
        let mut end = at + 1;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if end == at + 1 || bytes.get(end) != Some(&b'(') {
            continue;
        }
        let name = &code[at + 1..end];
        if name.chars().next().is_some_and(char::is_numeric) {
            continue;
        }
        found.insert(name.to_owned());
    }
    found
}

/// Every double-quoted string literal in `source`, unescaped enough to compare.
fn string_literals(source: &str) -> Vec<String> {
    let characters: Vec<char> = source.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        if characters[index] == '/' && characters.get(index + 1) == Some(&'/') {
            while index < characters.len() && characters[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if characters[index] != '"' {
            index += 1;
            continue;
        }
        let mut literal = String::new();
        index += 1;
        while index < characters.len() {
            match characters[index] {
                '\\' => {
                    index += 2;
                }
                '"' => {
                    index += 1;
                    break;
                }
                character => {
                    literal.push(character);
                    index += 1;
                }
            }
        }
        if !literal.is_empty() {
            found.push(literal);
        }
    }
    found
}

// ---------------------------------------------------------------------------
// 3. prep_uncertainty_factorization
// ---------------------------------------------------------------------------

/// Section 12.7's last sentence, measured, and then observed as three readings
/// that no operation folds.
///
/// The three axes are read back out of the document in both directions, and
/// then the three are shown to be **separately movable**: each axis's evidence
/// is moved on its own and only that axis's answer changes. The absence half is
/// a whole-set classification of every public signature whose return type names
/// `ConfidencePermille`; each must be an accessor on one of the three reading
/// types, so a `PrepUncertainty::confidence` added later is a new entry with an
/// owner the answer does not hold.
#[test]
fn prep_uncertainty_factorization() -> TestResult {
    let page = specification()?;
    let block = section(&page, "### 12.7 ")?;
    let sentence = block
        .lines()
        .find(|line| line.contains("각각의 근거와 confidence를 분리한다"))
        .ok_or("section 12.7 no longer separates the three")?;
    let listed = sentence
        .split_once("가 모두 불확실할 수 있으므로")
        .ok_or("section 12.7's last sentence changed shape")?
        .0;
    let cells: Vec<&str> = listed.split(", ").map(str::trim).collect();
    let declared: Vec<&str> = PREP_AXES.iter().map(|axis| axis.spec_token()).collect();
    assert_eq!(cells, declared, "section 12.7's axes and PREP_AXES differ");
    let mut remainder = listed.to_owned();
    for axis in PREP_AXES {
        let token = axis.spec_token();
        let at = remainder
            .find(token)
            .ok_or_else(|| format!("section 12.7 no longer names {token}"))?;
        remainder.replace_range(at..at + token.len(), "");
    }
    assert!(
        remainder
            .chars()
            .all(|character| character == ',' || character.is_whitespace()),
        "section 12.7's last sentence holds an axis nothing here accounts for: {remainder:?}"
    );
    assert_eq!(PREP_AXES.len(), 3);

    // The three readings, from one real candidate.
    let graph = graph()?;
    let readings = section_12_7_readings()?;
    let goal = goal_at(buffer_pool(), "goal-factorization")?;
    let case = case_over(&goal, &graph, &readings)?;
    let claim = claim_from(ExpectedConceptSource::Syllabus)?;
    let states = overlays(&readings)?;
    let candidate = candidate_for(&claim, &case, &states, &graph, disk_page())?;
    let uncertainty = candidate.uncertainty();

    // Each axis is about what it says it is about.
    assert_eq!(uncertainty.expected_concept().concept(), buffer_pool());
    assert_eq!(uncertainty.prerequisite_edge().advanced(), buffer_pool());
    assert_eq!(uncertainty.prerequisite_edge().prerequisite(), disk_page());
    assert_eq!(uncertainty.user_state().concept(), disk_page());

    // Each carries its own evidence, and the three sets are drawn from three
    // different owners: spans of an untrusted document, the edge's own cited
    // items, and the overlay's own. The first is not even the same type as the
    // other two, which is why no list here can be all three at once.
    assert!(!uncertainty.expected_concept().citations().is_empty());
    assert!(!uncertainty.prerequisite_edge().evidence().is_empty());
    assert!(!uncertainty.user_state().supporting().is_empty());
    let edge_items: BTreeSet<_> = uncertainty
        .prerequisite_edge()
        .evidence()
        .iter()
        .copied()
        .collect();
    let state_items: BTreeSet<_> = uncertainty
        .user_state()
        .supporting()
        .iter()
        .chain(uncertainty.user_state().contradicting())
        .copied()
        .collect();
    assert!(
        edge_items.is_disjoint(&state_items),
        "the edge axis and the state axis cite one set"
    );

    // Each carries its own confidence, and the three differ, so no reader can
    // be seeing one number three times.
    let three = [
        uncertainty.expected_concept().confidence().value(),
        uncertainty.prerequisite_edge().confidence().value(),
        uncertainty.user_state().confidence().value(),
    ];
    assert_eq!(
        three.iter().collect::<BTreeSet<_>>().len(),
        3,
        "two axes answered with one confidence: {three:?}"
    );
    assert_eq!(three[0], claim.confidence().value());
    assert_eq!(three[1], edge_confidence()?.value());

    // Moving one axis moves one answer. The claim's confidence is changed and
    // nothing else is, and only axis one moves.
    let moved_claim = claim_about(ExpectedConceptSource::Syllabus, buffer_pool())?;
    let louder = ExpectedConceptClaim::extract(
        buffer_pool(),
        EntityKind::Concept,
        moved_claim.material().clone(),
        &proposal_over(
            &ingest_material(
                "material-syllabus-louder",
                SourceKind::Syllabus,
                &material_text(ExpectedConceptSource::Syllabus),
            )?,
            0,
            31,
            "week 6 covers buffer management",
        )?,
        ConfidencePermille::new(410)?,
    );
    // The material the louder proposal quotes is a second document, so the
    // extraction refuses -- which is itself the binding under test elsewhere.
    assert!(matches!(
        louder,
        Err(NextLectureError::ClaimDoesNotQuoteItsMaterial { .. })
    ));

    // So the axis is moved the way a caller would: a claim over its own
    // material at a different confidence.
    let quieter = quieter_claim(ConfidencePermille::new(410)?)?;
    let moved = candidate_for(&quieter, &case, &states, &graph, disk_page())?;
    assert_eq!(
        moved.uncertainty().expected_concept().confidence().value(),
        410
    );
    assert_eq!(
        moved.uncertainty().prerequisite_edge().confidence(),
        uncertainty.prerequisite_edge().confidence(),
        "moving the claim moved the edge axis"
    );
    assert_eq!(
        moved.uncertainty().user_state().confidence(),
        uncertainty.user_state().confidence(),
        "moving the claim moved the state axis"
    );

    // And the edge axis moves alone too.
    let root = case
        .candidates()
        .iter()
        .find(|candidate| candidate.concept() == disk_page())
        .ok_or("no Disk Page candidate")?;
    let state = states
        .iter()
        .find(|state| state.concept() == disk_page())
        .ok_or("no Disk Page overlay")?;
    let other_edge = PreparationCandidate::of(
        CandidateParts {
            claim: &claim,
            root,
            state,
            edge_confidence: ConfidencePermille::new(215)?,
        },
        &graph,
    )?;
    assert_eq!(
        other_edge
            .uncertainty()
            .prerequisite_edge()
            .confidence()
            .value(),
        215
    );
    assert_eq!(
        other_edge.uncertainty().expected_concept().confidence(),
        uncertainty.expected_concept().confidence()
    );
    assert_eq!(
        other_edge.uncertainty().user_state().confidence(),
        uncertainty.user_state().confidence()
    );

    // Two axes that disagree about which concept they read are not a
    // factorization of one uncertainty.
    let wrong_state = states
        .iter()
        .find(|state| state.concept() == storage_hierarchy())
        .ok_or("no Storage Hierarchy overlay")?;
    let refused = PreparationCandidate::of(
        CandidateParts {
            claim: &claim,
            root,
            state: wrong_state,
            edge_confidence: edge_confidence()?,
        },
        &graph,
    );
    assert!(
        matches!(
            refused,
            Err(NextLectureError::CandidateStateIsAboutAnotherConcept { .. })
        ),
        "a candidate was built over another concept's overlay"
    );

    // `PrepUncertainty::factor` is public, so its own agreement rule is driven
    // directly rather than only through the candidate constructor. Deleting it
    // left this whole suite passing until this case existed: the candidate
    // constructor's own concept check fires first for every path a candidate
    // takes, so it was masking a guard on the value it builds.
    let edge = graph
        .edges()
        .iter()
        .find(|edge| edge.prerequisite() == disk_page())
        .ok_or("the fixture graph holds no edge into Disk Page")?;
    let hierarchy = states
        .iter()
        .find(|state| state.concept() == storage_hierarchy())
        .ok_or("no Storage Hierarchy overlay")?;
    assert!(
        matches!(
            PrepUncertainty::factor(&claim, edge, edge_confidence()?, hierarchy),
            Err(NextLectureError::AxesDescribeDifferentConcepts { .. })
        ),
        "two axes about two concepts were factored as one uncertainty"
    );
    // The control: the same call over the overlay the edge really runs into.
    assert!(
        PrepUncertainty::factor(&claim, edge, edge_confidence()?, state).is_ok(),
        "the matching pair was refused"
    );

    // The absence half, as a whole-set classification.
    let owners: BTreeSet<&str> = BTreeSet::from([
        "ExpectedConceptReading",
        "PrerequisiteEdgeReading",
        "UserStateReading",
        "ExpectedConceptClaim",
    ]);
    let mut answered: Vec<(String, String, String)> = Vec::new();
    for (path, code) in support::product_code()? {
        for (owner, name, signature) in support::public_signatures_with_owner(&code) {
            let Some((_, returns)) = signature.split_once("->") else {
                continue;
            };
            if returns.contains("ConfidencePermille") {
                answered.push((path.clone(), owner, name));
            }
        }
    }
    assert!(
        !answered.is_empty(),
        "the return-type scanner found no confidence accessor at all"
    );
    for (path, owner, name) in &answered {
        assert!(
            owners.contains(owner.as_str()),
            "{path}::{owner}::{name} answers with a confidence and is not one axis"
        );
    }
    // And no public signature answers with several confidences at once.
    for (path, code) in support::product_code()? {
        for (name, signature) in support::public_signatures(&code) {
            let Some((_, returns)) = signature.split_once("->") else {
                continue;
            };
            assert!(
                !(returns.contains("ConfidencePermille")
                    && (returns.contains('[') || returns.contains("Vec") || returns.contains('('))),
                "{path}::{name} answers with a collection of confidences"
            );
        }
    }
    // Every axis of the three is named by one of the three reading types, in
    // both directions.
    let named: BTreeSet<PrepAxis> = BTreeSet::from([
        academic_next_lecture::ExpectedConceptReading::AXIS,
        academic_next_lecture::PrerequisiteEdgeReading::AXIS,
        academic_next_lecture::UserStateReading::AXIS,
    ]);
    assert_eq!(named, BTreeSet::from(PREP_AXES));
    Ok(())
}

/// A claim over its own material at a chosen confidence.
fn quieter_claim(confidence: ConfidencePermille) -> Result<ExpectedConceptClaim, Box<dyn Error>> {
    let material = ingest_material(
        "material-syllabus",
        SourceKind::Syllabus,
        &material_text(ExpectedConceptSource::Syllabus),
    )?;
    let proposal = proposal_over(&material, 0, 31, "week 6 covers buffer management")?;
    let reference = material_reference(ExpectedConceptSource::Syllabus, material.id.clone())?;
    Ok(ExpectedConceptClaim::extract(
        buffer_pool(),
        EntityKind::Concept,
        reference,
        &proposal,
        confidence,
    )?)
}

/// One candidate for `concept`, built from a real case and its overlays.
fn candidate_for(
    claim: &ExpectedConceptClaim,
    case: &academic_gap::GapCase,
    states: &[academic_gap::ConceptState],
    graph: &PrerequisiteGraph,
    concept: EntityId,
) -> Result<PreparationCandidate, Box<dyn Error>> {
    let root = case
        .candidates()
        .iter()
        .find(|candidate| candidate.concept() == concept)
        .ok_or("the descent produced no candidate for that concept")?;
    let state = states
        .iter()
        .find(|state| state.concept() == concept)
        .ok_or("no overlay for that concept")?;
    Ok(PreparationCandidate::of(
        CandidateParts {
            claim,
            root,
            state,
            edge_confidence: edge_confidence()?,
        },
        graph,
    )?)
}

// ---------------------------------------------------------------------------
// 4. morning_home_contract
// ---------------------------------------------------------------------------

/// Section 4's `선수개념 1–3개` and section 25.2's `최대 1–3개`, both parsed,
/// required to agree with each other, with this crate's bound and with
/// `P2-X2`'s.
///
/// Then the constructor is driven at every count from zero to two past the
/// upper bound and each answer is judged against the **parsed** bound rather
/// than against a hard-coded expectation, which is `P2-X2`'s own shape.
#[test]
fn morning_home_contract() -> TestResult {
    let page = specification()?;

    // Section 4's morning line.
    let morning = support::top_section(&page, "## 4. End-State Experience")?
        .lines()
        .find(|line| line.contains("선수개념"))
        .ok_or("section 4 no longer shows prerequisites in the morning")?
        .to_owned();
    let (low_four, high_four) = bound_in(&morning, "선수개념 ")?;

    // Section 25.2's second numbered line.
    let home = section(&page, "### 25.2 ")?;
    let second = home
        .lines()
        .find(|line| line.starts_with("2. "))
        .ok_or("section 25.2 has no second numbered line")?
        .to_owned();
    let (low_home, high_home) = bound_in(&second, "최대 ")?;

    assert_eq!(
        (low_four, high_four),
        (low_home, high_home),
        "section 4 and section 25.2 bound the morning differently"
    );
    assert_eq!(
        low_four, LOWEST_PREPARATION,
        "the lower bound is not the document's"
    );
    assert_eq!(
        high_four, HIGHEST_PREPARATION,
        "the upper bound is not the document's"
    );
    assert!(
        low_four >= 1 && high_four > low_four,
        "the parsed bound is degenerate"
    );

    // And `P2-X2` offers the same card, so the two crates cannot drift apart.
    assert_eq!(
        (LOWEST_PREPARATION, HIGHEST_PREPARATION),
        (academic_home::LOWEST_BRIEF, academic_home::HIGHEST_BRIEF),
        "this crate's bound and P2-X2's have diverged"
    );

    // Section 25.2 asks for both a reason and a time by name, and a candidate
    // carries both by construction.
    assert!(second.contains("\u{201c}왜 지금\u{201d}"));
    assert!(second.contains("예상 시간"));

    let graph = graph()?;
    let readings = section_12_7_readings()?;
    let goal = goal_at(buffer_pool(), "goal-morning")?;
    let case = case_over(&goal, &graph, &readings)?;
    let claim = claim_from(ExpectedConceptSource::Syllabus)?;
    let states = overlays(&readings)?;
    let candidate = candidate_for(&claim, &case, &states, &graph, disk_page())?;

    // `왜 지금`: the descent from tomorrow's concept to this one, with a
    // strength on every hop, ending at the candidate.
    assert_eq!(candidate.why_now().surface(), buffer_pool());
    assert_eq!(candidate.why_now().tip(), disk_page());
    assert!(!candidate.why_now().steps().is_empty());
    // `예상 시간`: a positive number of minutes with something cited beside it.
    assert!(candidate.preparation().minutes() > 0);
    assert!(!candidate.preparation().sources().is_empty());
    assert!(!candidate.reason().trim().is_empty());

    // Every count from zero to two past the bound, judged by the parsed bound.
    for count in 0..=(high_four + 2) {
        let mut candidates = Vec::new();
        for index in 0..count {
            // Distinct concepts, so the count is what is being judged and not
            // the repetition rule.
            candidates.push(distinct_candidate(index)?);
        }
        let assembled = PreparationBrief::assemble(buffer_pool(), candidates);
        let within = (low_four..=high_four).contains(&count);
        match (within, assembled) {
            (true, Ok(brief)) => {
                assert_eq!(brief.candidates().len(), count);
                for item in brief.candidates() {
                    assert!(item.preparation().minutes() > 0);
                    assert!(!item.uncertainty().expected_concept().citations().is_empty());
                }
            }
            (false, Err(NextLectureError::PreparationCountOutOfBounds { count: reported })) => {
                assert_eq!(reported, count);
            }
            (true, Err(error)) => {
                return Err(format!("{count} candidates were refused: {error}").into());
            }
            (false, Ok(_)) => {
                return Err(format!("{count} candidates were admitted").into());
            }
            (false, Err(error)) => {
                return Err(format!("{count} candidates were refused as {error}").into());
            }
        }
    }

    // The same concept twice is refused, so three slots are three foundations.
    let twice =
        PreparationBrief::assemble(buffer_pool(), vec![candidate.clone(), candidate.clone()]);
    assert!(
        matches!(twice, Err(NextLectureError::CandidateRepeatsAnother { .. })),
        "one foundation filled two of the morning's slots"
    );
    Ok(())
}

/// Splits a `1–3` bound out of `line`, after `anchor`, on the document's own en
/// dash.
fn bound_in(line: &str, anchor: &str) -> Result<(usize, usize), Box<dyn Error>> {
    let after = line
        .split_once(anchor)
        .ok_or_else(|| format!("the line does not bound the morning after {anchor:?}"))?
        .1;
    let phrase = after
        .split_once('개')
        .ok_or("the bound is not counted in 개")?
        .0;
    let (low, high) = phrase
        .split_once('\u{2013}')
        .ok_or("the bound is not written as a range")?;
    Ok((low.trim().parse()?, high.trim().parse()?))
}

/// One admissible candidate whose concept is `index` hops from the others.
fn distinct_candidate(index: usize) -> Result<PreparationCandidate, Box<dyn Error>> {
    let tomorrow = entity(&format!("tomorrow-{index}"));
    let foundation = entity(&format!("foundation-{index}"));
    let graph =
        PrerequisiteGraph::new().with(hard_edge(tomorrow, foundation, &format!("edge-{index}"))?);
    let goal = goal_at(tomorrow, &format!("goal-{index}"))?;
    let readings = vec![
        reading(tomorrow, unknown_band(tomorrow)?),
        support::blocked_reading(foundation, &format!("distinct-{index}"))?,
    ];
    let case =
        search(&goal, &graph, &readings, None)?.ok_or("the distinct fixture produced no gap")?;
    let claim = claim_about(ExpectedConceptSource::Syllabus, tomorrow)?;
    let states = overlays(&readings)?;
    candidate_for(&claim, &case, &states, &graph, foundation)
}

// ---------------------------------------------------------------------------
// The entry point
// ---------------------------------------------------------------------------

/// `propose` reads a `P2-N5` case and neither ranks nor hides.
///
/// A case with more strong deficits than the morning has room for is refused
/// with the count, which is section 25.2's `자동 중요도 순으로 숨기지 않고`; a
/// case with none answers `Ok(None)` rather than an empty card; and a case
/// about another concept is refused outright.
#[test]
fn propose_refuses_to_rank_and_refuses_to_invent() -> TestResult {
    let graph = graph()?;
    let readings = section_12_7_readings()?;
    let goal = goal_at(buffer_pool(), "goal-propose")?;
    let case = case_over(&goal, &graph, &readings)?;
    let claim = claim_from(ExpectedConceptSource::Assignment)?;
    let states = overlays(&readings)?;

    let brief = propose(&claim, &case, &graph, &states, edge_confidence()?)?
        .ok_or("the section 12.7 fixture produced no brief")?;
    assert_eq!(brief.lecture_concept(), buffer_pool());
    assert!(!brief.candidates().is_empty());
    assert!(brief.candidates().len() <= HIGHEST_PREPARATION);
    assert!(
        brief
            .candidates()
            .iter()
            .all(|candidate| candidate.kind() == GapKind::MasteryGap)
    );

    // A case about another concept.
    let elsewhere = claim_about(ExpectedConceptSource::Notice, storage_hierarchy())?;
    assert!(matches!(
        propose(&elsewhere, &case, &graph, &states, edge_confidence()?),
        Err(NextLectureError::CaseIsAboutAnotherConcept { .. })
    ));

    // A missing overlay is refused rather than guessed.
    let short: Vec<_> = states
        .iter()
        .filter(|state| state.concept() != disk_page())
        .cloned()
        .collect();
    assert!(matches!(
        propose(&claim, &case, &graph, &short, edge_confidence()?),
        Err(NextLectureError::NoStateForConcept(_))
    ));

    // Four blocking foundations at once: refused with the count, and no three
    // of them are chosen.
    let (wide_graph, wide_case, wide_readings, wide_claim) = four_foundations()?;
    let wide_states = overlays(&wide_readings)?;
    let refused = propose(
        &wide_claim,
        &wide_case,
        &wide_graph,
        &wide_states,
        edge_confidence()?,
    );
    assert!(
        matches!(
            refused,
            Err(NextLectureError::TooManyBlockingFoundations { count }) if count == 4
        ),
        "four blocking foundations were not refused with their count"
    );

    // Nothing to prepare: no card.
    let (calm_graph, calm_readings, calm_claim) = nothing_to_prepare()?;
    let calm_goal = goal_at(buffer_pool(), "goal-calm")?;
    let calm_case = search(&calm_goal, &calm_graph, &calm_readings, None)?;
    match calm_case {
        None => {}
        Some(case) => {
            let calm_states = overlays(&calm_readings)?;
            assert_eq!(
                propose(
                    &calm_claim,
                    &case,
                    &calm_graph,
                    &calm_states,
                    edge_confidence()?
                )?,
                None,
                "a case with no strong deficit produced a card"
            );
        }
    }
    Ok(())
}

/// A lecture with four distinct blocking foundations one hop below it.
#[allow(clippy::type_complexity)]
fn four_foundations() -> Result<
    (
        PrerequisiteGraph,
        academic_gap::GapCase,
        Vec<ConceptReading>,
        ExpectedConceptClaim,
    ),
    Box<dyn Error>,
> {
    let tomorrow = entity("wide-tomorrow");
    let mut graph = PrerequisiteGraph::new();
    let mut readings = vec![reading(tomorrow, unknown_band(tomorrow)?)];
    for index in 0..4 {
        let foundation = entity(&format!("wide-foundation-{index}"));
        graph = graph.with(hard_edge(
            tomorrow,
            foundation,
            &format!("edge-wide-{index}"),
        )?);
        readings.push(support::blocked_reading(
            foundation,
            &format!("wide-{index}"),
        )?);
    }
    let goal = goal_at(tomorrow, "goal-wide")?;
    // Four roots at one depth is section 15.2 step 5's tie, and `P2-N5` refuses
    // to build a case for one without a diagnostic. Offering one is what lets
    // this fixture reach the question it is about: `propose` refuses to pick
    // three of four, which is a different refusal one level up.
    let offer = academic_gap::DiagnosticOffer {
        minutes: 12,
        description: "네 후보를 가르는 짧은 확인 문항".to_owned(),
        sources: vec![evidence_id("wide-diagnostic")],
        question: None,
    };
    let case = search(&goal, &graph, &readings, Some(&offer))?
        .ok_or("the wide fixture produced no gap")?;
    let claim = claim_about(ExpectedConceptSource::LmsMaterial, tomorrow)?;
    Ok((graph, case, readings, claim))
}

/// A lecture whose one prerequisite is already practised and retrievable.
#[allow(clippy::type_complexity)]
fn nothing_to_prepare()
-> Result<(PrerequisiteGraph, Vec<ConceptReading>, ExpectedConceptClaim), Box<dyn Error>> {
    let graph = PrerequisiteGraph::new().with(requires(
        buffer_pool(),
        disk_page(),
        PrerequisiteStrength::Strong,
        "edge-calm",
    )?);
    let concept = disk_page();
    let recent = dated_exercise(concept, "recent", 0)?;
    let mut page = reading(
        concept,
        band_from(concept, &[recent], &[], at(0), FreshnessBand::VeryHigh)?,
    );
    page.offered = vec![
        offered(
            exercise_evidence(&format!("{concept}-calm-a")),
            &format!("{concept}-calm-a"),
            full_dossier(concept),
        ),
        offered(
            exercise_evidence(&format!("{concept}-calm-b")),
            &format!("{concept}-calm-b"),
            full_dossier(concept),
        ),
    ];
    let mut buffer = reading(buffer_pool(), unknown_band(buffer_pool())?);
    buffer.offered = vec![offered(
        exposure_evidence("lecture-buffer-pool-calm")?,
        "evidence-buffer-pool-calm",
        full_dossier(buffer_pool()),
    )];
    let claim = claim_about(ExpectedConceptSource::Syllabus, buffer_pool())?;
    Ok((graph, vec![buffer, page], claim))
}

/// A candidate's overlay and its descent cannot disagree, and the graph the
/// validator reads has to hold the edge the path names.
#[test]
fn a_candidate_needs_the_graph_its_path_came_from() -> TestResult {
    let graph = graph()?;
    let readings = section_12_7_readings()?;
    let goal = goal_at(buffer_pool(), "goal-graph-binding")?;
    let case = case_over(&goal, &graph, &readings)?;
    let claim = claim_from(ExpectedConceptSource::Syllabus)?;
    let states = overlays(&readings)?;
    let root = case
        .candidates()
        .iter()
        .find(|candidate| candidate.concept() == storage_hierarchy())
        .ok_or("the descent found no Storage Hierarchy candidate")?;
    let state = states
        .iter()
        .find(|state| state.concept() == storage_hierarchy())
        .ok_or("no Storage Hierarchy overlay")?;

    // The control: over the graph the descent ran on, it is admitted.
    assert!(
        PreparationCandidate::of(
            CandidateParts {
                claim: &claim,
                root,
                state,
                edge_confidence: edge_confidence()?,
            },
            &graph,
        )
        .is_ok()
    );
    // Over a graph missing the deepest hop, the edge axis has nothing to read
    // and the candidate is refused rather than built without one.
    let shallow =
        PrerequisiteGraph::new().with(hard_edge(buffer_pool(), disk_page(), "edge-bp-dp")?);
    let refused = PreparationCandidate::of(
        CandidateParts {
            claim: &claim,
            root,
            state,
            edge_confidence: edge_confidence()?,
        },
        &shallow,
    );
    assert!(
        matches!(
            refused,
            Err(NextLectureError::NotMinimal(_) | NextLectureError::NoEdgeForTheDeepestStep)
        ),
        "a candidate was built over a graph that does not hold its path"
    );
    Ok(())
}

/// The three extractors this suite classifies with each find what they should.
///
/// An extractor that always answered the empty set would satisfy every
/// whole-set comparison above, which is the third of this repository's three
/// empty-scan shapes.
#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    let sample = r#"
pub struct Sample;
impl Sample {
    pub const fn confidence(&self) -> ConfidencePermille { self.value }
    pub fn standing(&self) -> EpistemicStatus { EpistemicStatus::AiInferred }
}
"#;
    let code = support::strip_non_code(sample);
    let signatures = support::public_signatures_with_owner(&code);
    assert_eq!(
        signatures
            .iter()
            .map(|(owner, name, _)| (owner.as_str(), name.as_str()))
            .collect::<Vec<_>>(),
        vec![("Sample", "confidence"), ("Sample", "standing")],
        "the signature extractor missed a sample it must find"
    );
    assert!(
        signatures
            .iter()
            .any(|(_, _, signature)| signature.contains("EpistemicStatus"))
    );
    assert!(support::absolute_paths(&code).is_empty());
    assert_eq!(
        string_literals("let a = \"first\"; // \"comment\"\nlet b = \"second\";"),
        vec!["first".to_owned(), "second".to_owned()],
        "the literal reader read a comment or missed a literal"
    );
    // The product walk finds this crate's files and not its tests.
    let sources = support::crate_product_sources()?;
    assert!(
        sources.len() >= 6,
        "the product walk found {} files",
        sources.len()
    );
    assert!(
        sources
            .iter()
            .all(|path| !support::relative(path).contains("/tests/")),
        "the product walk descended into tests"
    );
    Ok(())
}

/// Section 12.7's `blocking evidence` line and this suite's fixture are the
/// same reading, so the suite runs on the design document's own scenario.
#[test]
fn the_fixture_is_section_12_7s_own_example() -> TestResult {
    let page = specification()?;
    let block = section(&page, "### 12.7 ")?;
    assert!(block.contains("Tomorrow: Database / Buffer Management"));
    assert!(block.contains("Expected: Disk Page, Buffer Pool, Replacement"));
    assert!(block.contains("Disk Page       mastery: Exposed, freshness: Low"));
    assert!(block.contains("full lecture preview, advanced replacement-policy survey"));

    let readings = section_12_7_readings()?;
    let page_reading = readings
        .iter()
        .find(|reading| reading.concept == disk_page())
        .ok_or("the fixture has no Disk Page reading")?;
    let state = overlay_of(page_reading)?;
    assert_eq!(
        state.mastery(),
        MasteryLevel::Exposed,
        "the fixture's Disk Page is not at the document's own rung"
    );
    // The document's `freshness: Low` and the fixture's `UNKNOWN` are both
    // below `RETRIEVAL_FLOOR`, which is the only thing the descent reads a band
    // for. Recorded rather than glossed: the fixture has no dated item for the
    // concept, and `P2-N3` answers `UNKNOWN` for that rather than `LOW`.
    assert!(
        academic_freshness::rank(state.freshness()) < academic_freshness::rank(RETRIEVAL_FLOOR)
    );
    Ok(())
}
