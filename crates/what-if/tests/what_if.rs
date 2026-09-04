//! `P2-N8`'s acceptance suite: the seven named rows that are about behaviour.
//!
//! The three that are absence claims — `plan_scenario_never_writes_actual_state`,
//! `no_mastery_delta_in_plan_output` and `no_default_recommendation_score` —
//! live in `what_if_scans.rs`, because an absence is proved by exhaustion over
//! this crate's own source and its manifest closure rather than by a value.
//!
//! Three habits hold throughout, each of them something an earlier task in this
//! run found missing from its own guards:
//!
//! * **no count is asserted.** Every list this crate declares is compared
//!   against the design document in both directions, and the number is whatever
//!   the document lists.
//! * **no oracle reads its expectation from the thing it checks.** Where a test
//!   needs to know what the answer should be, it recomputes it from the
//!   fixture, not from the value under test.
//! * **every loop has a floor**, so a parse that silently found nothing fails
//!   rather than passing over an empty set.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use academic_domain::{Actor, TimestampMillis};
use academic_proposal::UserDecision;
use academic_review::BiasDimension;
use academic_scenario::{
    LikelihoodBand, OpportunityKind, ProjectionCalibration, WorkloadHoursRange,
};
use academic_what_if::{
    BASIS_FIELDS, COMPARISON_DIMENSIONS, ComparisonDimension, DETERMINISTIC_LANE, DimensionLane,
    DimensionPriority, FrozenPlan, GRADUATION_MODES, GraduationMode, HypotheticalGraduation,
    LaneItem, ObservedOccasion, PROJECTED_LANE, RecomputeConsent, ReorderingExplanation,
    SCENARIO_KEYS, STALE_CAUSES, STALE_INPUT, ScenarioBasis, SectionView, StaleCause, StaleInput,
    UI_SECTIONS, UiSection, WhatIfError, calibrate, compare, simulate,
};

use support::TestResult;

// ---------------------------------------------------------------------------
// 1. scenario_basis_round_trip
// ---------------------------------------------------------------------------

/// The `key:` lines of one YAML block inside a fenced code span, at one exact
/// indentation.
fn yaml_keys(block: &str, indent: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in block.lines() {
        if !line.starts_with(indent) || line[indent.len()..].starts_with(' ') {
            continue;
        }
        let rest = &line[indent.len()..];
        let Some((key, _)) = rest.split_once(':') else {
            continue;
        };
        let key = key.trim_start_matches("- ").trim();
        if !key.is_empty()
            && key
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
        {
            found.push(key.to_owned());
        }
    }
    found
}

#[test]
fn scenario_basis_round_trip() -> TestResult {
    let specification = support::specification()?;
    let block = support::section(&specification, "### 22.1 사실과 가정의 격리")?;

    // The document's own `basedOn:` keys, at the block's four-space level.
    let start = block
        .find("  basedOn:")
        .ok_or("section 22.1 has no basedOn block")?;
    let rest = &block[start..];
    let end = rest
        .find("\n  choices:")
        .ok_or("section 22.1's basedOn block does not end at choices")?;
    let declared = yaml_keys(&rest[..end], "    ");
    assert!(
        !declared.is_empty(),
        "section 22.1's basedOn block parsed to no keys at all"
    );

    let carried: Vec<String> = BASIS_FIELDS
        .into_iter()
        .map(|field| field.spec_key().to_owned())
        .collect();
    assert_eq!(
        declared, carried,
        "BASIS_FIELDS is not section 22.1's basedOn block, in its own order"
    );

    // The top-level keys of the same block are the six `PlanScenario` carries.
    let scenario_start = block
        .find("PlanScenario:")
        .ok_or("section 22.1 has no PlanScenario block")?;
    let scenario_end = block[scenario_start..]
        .find("\n```")
        .ok_or("section 22.1's PlanScenario block is unterminated")?;
    let top_level = yaml_keys(&block[scenario_start..scenario_start + scenario_end], "  ");
    let scenario_keys: Vec<String> = SCENARIO_KEYS
        .into_iter()
        .map(|key| key.spec_key().to_owned())
        .collect();
    assert_eq!(
        top_level, scenario_keys,
        "SCENARIO_KEYS is not section 22.1's PlanScenario block, in its own order"
    );

    // The wire form is exactly the four fields, and the round trip is exact.
    let basis = support::basis();
    let encoded = serde_json::to_value(basis)?;
    let object = encoded
        .as_object()
        .ok_or("a scenario basis did not encode as an object")?;
    let encoded_keys: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let wire_keys: BTreeSet<&str> = BASIS_FIELDS.into_iter().map(|f| f.wire_key()).collect();
    assert_eq!(
        encoded_keys, wire_keys,
        "the encoded basis and BASIS_FIELDS disagree about the wire form"
    );
    let decoded: ScenarioBasis = serde_json::from_value(encoded.clone())?;
    assert_eq!(decoded, basis, "a basis did not survive its own round trip");
    assert_eq!(
        decoded.digest(),
        basis.digest(),
        "a round-tripped basis digests differently"
    );

    // Every field is inside the digest, one variation at a time.
    let mut moved = BTreeSet::new();
    for field in BASIS_FIELDS {
        let varied = match field {
            academic_what_if::BasisField::StudentRecordSnapshot => ScenarioBasis::of(
                support::digest(0xEE),
                basis.requirement_set_hash(),
                basis.offering_catalog_snapshot(),
                basis.knowledge_state_as_of(),
            ),
            academic_what_if::BasisField::RequirementSetHash => ScenarioBasis::of(
                basis.student_record_snapshot(),
                support::digest(0xEE),
                basis.offering_catalog_snapshot(),
                basis.knowledge_state_as_of(),
            ),
            academic_what_if::BasisField::OfferingCatalogSnapshot => ScenarioBasis::of(
                basis.student_record_snapshot(),
                basis.requirement_set_hash(),
                support::digest(0xEE),
                basis.knowledge_state_as_of(),
            ),
            academic_what_if::BasisField::KnowledgeStateAsOf => ScenarioBasis::of(
                basis.student_record_snapshot(),
                basis.requirement_set_hash(),
                basis.offering_catalog_snapshot(),
                TimestampMillis::new(9_999),
            ),
        };
        assert_ne!(
            varied.digest(),
            basis.digest(),
            "changing {} left the basis digest alone",
            field.spec_key()
        );
        moved.insert(field);
    }
    assert_eq!(
        moved.len(),
        BASIS_FIELDS.len(),
        "not every basis field was varied"
    );

    // Two digests that swapped a pair of same-shaped fields differ. That is the
    // field *order*, not the keying: an injection that removed the key from
    // every field left this observation unchanged, and the sentence that once
    // credited it to the key was wrong. What holds the keying is the pin below.
    let swapped = ScenarioBasis::of(
        basis.requirement_set_hash(),
        basis.student_record_snapshot(),
        basis.offering_catalog_snapshot(),
        basis.knowledge_state_as_of(),
    );
    assert_ne!(
        swapped.digest(),
        basis.digest(),
        "two swapped basis fields collide, so the digest is not ordered"
    );

    // The digest of a known basis, pinned. Every other observation in this test
    // recomputes the digest with the same code, so a change to the encoding — a
    // dropped key, a dropped length prefix, a reordered field — moves nothing
    // any of them can see. This is the one assertion that does, and the cost of
    // changing the encoding is changing it here.
    assert_eq!(
        basis.digest().to_string(),
        "sha256:40b8b47de7f5e2b3700eacfb69b2a65667616f892f1736e8804e710d8bba95d1",
        "the scenario basis encoding changed"
    );

    // An unknown field and a missing field are both refusals.
    let mut extra = object.clone();
    extra.insert("mastery_level".to_owned(), serde_json::json!("FLUENT"));
    assert!(
        serde_json::from_value::<ScenarioBasis>(serde_json::Value::Object(extra)).is_err(),
        "an unknown field was accepted into a scenario basis"
    );
    for field in BASIS_FIELDS {
        let mut short = object.clone();
        short.remove(field.wire_key());
        assert!(
            serde_json::from_value::<ScenarioBasis>(serde_json::Value::Object(short)).is_err(),
            "a basis missing {} was accepted",
            field.wire_key()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. deterministic_and_projected_are_separate_types_and_sections
// ---------------------------------------------------------------------------

#[test]
fn deterministic_and_projected_are_separate_types_and_sections() -> TestResult {
    let specification = support::specification()?;

    let deterministic_block = support::section(&specification, "### 22.2 결정론적 결과")?;
    let declared_deterministic = support::bullets(&deterministic_block);
    assert!(
        !declared_deterministic.is_empty(),
        "section 22.2 parsed to no bullets at all"
    );
    let carried_deterministic: Vec<String> = DETERMINISTIC_LANE
        .into_iter()
        .map(|item| item.spec_phrase().to_owned())
        .collect();
    assert_eq!(
        declared_deterministic, carried_deterministic,
        "DETERMINISTIC_LANE is not section 22.2's bullet list, in its own order"
    );

    let projected_block = support::section(&specification, "### 22.3 확률적/가정 결과")?;
    let declared_projected = support::bullets(&projected_block);
    assert!(
        !declared_projected.is_empty(),
        "section 22.3 parsed to no bullets at all"
    );
    let carried_projected: Vec<String> = PROJECTED_LANE
        .into_iter()
        .map(|item| item.spec_phrase().to_owned())
        .collect();
    assert_eq!(
        declared_projected, carried_projected,
        "PROJECTED_LANE is not section 22.3's bullet list, in its own order"
    );

    // The two lists are disjoint as text, so a bullet moved from one section to
    // the other fails above rather than appearing in both.
    let overlap: BTreeSet<&String> = declared_deterministic
        .iter()
        .filter(|phrase| declared_projected.contains(phrase))
        .collect();
    assert!(
        overlap.is_empty(),
        "a bullet appears in both lanes: {overlap:?}"
    );

    // Section 22.1's own sentence, and the two section keys it names.
    let block = support::section(&specification, "### 22.1 사실과 가정의 격리")?;
    assert!(
        block.contains("UI section과 데이터 type 모두에서 분리한다"),
        "section 22.1 no longer separates the lanes in both the UI and the type"
    );
    for section in UI_SECTIONS {
        assert!(
            block.contains(section.spec_key()),
            "section 22.1 does not name {}",
            section.spec_key()
        );
    }

    // The item-to-section mapping is a partition.
    let mut by_section: BTreeMap<UiSection, Vec<LaneItem>> = BTreeMap::new();
    for item in DETERMINISTIC_LANE.into_iter().map(LaneItem::Deterministic) {
        by_section.entry(item.section()).or_default().push(item);
    }
    for item in PROJECTED_LANE.into_iter().map(LaneItem::Projected) {
        by_section.entry(item.section()).or_default().push(item);
    }
    assert_eq!(
        by_section.keys().copied().collect::<Vec<_>>(),
        UI_SECTIONS.to_vec(),
        "the lane items do not partition into exactly the two sections"
    );
    for section in UI_SECTIONS {
        assert_eq!(
            by_section.get(&section).cloned().unwrap_or_default(),
            section.items(),
            "{} renders a different item list than the partition",
            section.spec_key()
        );
    }

    // The two rendered views borrow two different types, and each reports its
    // own section.
    let plan = simulate(&support::plan_a()?)?;
    let sections = plan.sections();
    assert_eq!(
        sections.map(|view| view.section()),
        UI_SECTIONS,
        "a plan does not render section 22.1's two sections in its own order"
    );
    match plan.section(UiSection::DeterministicResults) {
        SectionView::DeterministicResults(results) => {
            assert!(std::ptr::eq(results, plan.deterministic()));
        }
        SectionView::Projections(_) => {
            return Err("the deterministic section rendered a projection".into());
        }
    }
    match plan.section(UiSection::Projections) {
        SectionView::Projections(results) => {
            assert!(std::ptr::eq(results, plan.projections()));
        }
        SectionView::DeterministicResults(_) => {
            return Err("the projections section rendered a deterministic result".into());
        }
    }

    // Each result reports only its own lane's items, and the two are disjoint.
    let produced_deterministic: BTreeSet<_> = plan.deterministic().produced().into_iter().collect();
    let produced_projected: BTreeSet<_> = plan.projections().produced().into_iter().collect();
    assert!(
        produced_deterministic.is_subset(&DETERMINISTIC_LANE.into_iter().collect()),
        "a deterministic result reported an item outside section 22.2"
    );
    assert!(
        produced_projected.is_subset(&PROJECTED_LANE.into_iter().collect()),
        "a projected result reported an item outside section 22.3"
    );
    assert!(
        !produced_deterministic.is_empty() && !produced_projected.is_empty(),
        "one of the two lanes produced nothing at all"
    );

    // The struct bodies name none of each other's types. Read out of the source
    // rather than asserted, with a control: the same reader must find each
    // module naming its own types.
    let deterministic_source = support::module_source("deterministic")?;
    let projected_source = support::module_source("projected")?;
    let deterministic_body =
        support::block_of(&deterministic_source, "pub struct DeterministicResults {")?;
    let projected_body = support::block_of(&projected_source, "pub struct ProjectedResults {")?;
    let deterministic_types: BTreeSet<String> = support::field_declarations(&deterministic_body)
        .into_iter()
        .map(|(_, kind)| kind)
        .collect();
    let projected_types: BTreeSet<String> = support::field_declarations(&projected_body)
        .into_iter()
        .map(|(_, kind)| kind)
        .collect();
    assert!(
        !deterministic_types.is_empty() && !projected_types.is_empty(),
        "one of the two result bodies parsed to no fields"
    );
    // The two sets of type names are derived, not listed: everything
    // `projected.rs` declares plus everything it imports from `P2-C7`, against
    // everything `deterministic.rs` declares. A projected type smuggled into
    // the deterministic body under any name at all fails here, and so does the
    // reverse.
    let projected_vocabulary: BTreeSet<String> = support::declared_type_names(&projected_source)
        .union(&support::imported_leaf_names(
            &projected_source,
            "academic_scenario",
        ))
        .cloned()
        .collect();
    let deterministic_vocabulary = support::declared_type_names(&deterministic_source);
    assert!(
        projected_vocabulary.len() >= 8 && deterministic_vocabulary.len() >= 8,
        "one of the two lane vocabularies parsed to too few names: {} and {}",
        projected_vocabulary.len(),
        deterministic_vocabulary.len()
    );
    for kind in &deterministic_types {
        let spoken: Vec<&String> = projected_vocabulary
            .iter()
            .filter(|name| support::names_type(kind, name))
            .collect();
        assert!(
            spoken.is_empty(),
            "DeterministicResults holds the projected vocabulary {spoken:?} in {kind}"
        );
    }
    for kind in &projected_types {
        let spoken: Vec<&String> = deterministic_vocabulary
            .iter()
            .filter(|name| support::names_type(kind, name))
            .collect();
        assert!(
            spoken.is_empty(),
            "ProjectedResults holds the deterministic vocabulary {spoken:?} in {kind}"
        );
    }
    // The controls: each body must name its own lane's vocabulary, or the two
    // emptinesses above are a reader that matches nothing.
    let deterministic_own = deterministic_types
        .iter()
        .filter(|kind| {
            deterministic_vocabulary
                .iter()
                .any(|name| support::names_type(kind, name))
        })
        .count();
    let projected_own = projected_types
        .iter()
        .filter(|kind| {
            projected_vocabulary
                .iter()
                .any(|name| support::names_type(kind, name))
        })
        .count();
    assert!(
        deterministic_own >= 6 && projected_own >= 4,
        "the lane control found only {deterministic_own} and {projected_own} own-vocabulary fields"
    );

    // The seventh deterministic bullet is reported when it is produced and not
    // otherwise, so `produced` cannot become a constant list.
    let mut with_grades = support::plan_a()?;
    with_grades.grade_assumptions = Some(support::stated_grades()?);
    let graded = simulate(&with_grades)?;
    assert!(graded.deterministic().gpa().is_some());
    assert!(
        graded
            .deterministic()
            .produced()
            .contains(&academic_what_if::DeterministicItem::GpaUnderStatedGradeAssumptions),
        "a plan with a GPA does not report section 22.2's seventh bullet"
    );
    assert!(plan.deterministic().gpa().is_none());
    assert!(
        !plan
            .deterministic()
            .produced()
            .contains(&academic_what_if::DeterministicItem::GpaUnderStatedGradeAssumptions),
        "a plan with no stated grade reports section 22.2's seventh bullet anyway"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. workload_is_a_range_with_bias_metadata
// ---------------------------------------------------------------------------

#[test]
fn workload_is_a_range_with_bias_metadata() -> TestResult {
    let inputs = support::plan_a()?;
    let plan = simulate(&inputs)?;
    let workload = plan.projections().workload();

    // The band is the sum of the plan's own assumed ranges, recomputed here
    // from the fixture rather than read back out of the value under test.
    let mut expected = WorkloadHoursRange::new(0, 0)?;
    for choice in &inputs.choices {
        expected = expected.saturating_add(choice.assumed_weekly_hours());
    }
    assert_eq!(workload.band(), expected);
    assert!(
        workload.band().low_hours() < workload.band().high_hours(),
        "a projected workload collapsed to a single number"
    );

    // The sealed proposal carries the same range. Read through `P2-C7`'s own
    // wire form, which is the only route out of the seal and is the route that
    // crate documents.
    let sealed = serde_json::to_value(workload.proposed())?;
    let range = sealed
        .get("range")
        .ok_or("a sealed workload has no range on the wire")?;
    assert_eq!(
        range.get("low_hours").and_then(serde_json::Value::as_u64),
        Some(u64::from(expected.low_hours()))
    );
    assert_eq!(
        range.get("high_hours").and_then(serde_json::Value::as_u64),
        Some(u64::from(expected.high_hours()))
    );

    // The provenance is this engine's, over this plan's frozen inputs.
    let provenance = workload.proposed().provenance();
    assert_eq!(provenance.model_run_id(), inputs.model_run_id);
    assert_eq!(provenance.inputs_digest(), plan.inputs_digest());
    assert_eq!(
        provenance.engine_version(),
        academic_what_if::WHAT_IF_ENGINE_VERSION
    );

    // The bias is `P2-U8`'s six, whole and in that crate's own order.
    let disclosed: Vec<BiasDimension> = workload.bias().disclosed();
    assert_eq!(
        disclosed,
        BiasDimension::ALL.to_vec(),
        "a projected workload discloses a different bias set than section 29.5's"
    );

    // Section 22.4's own sentence names four of the six. Parsed out of the
    // document and mapped, in both directions, so a rewritten sentence fails.
    let specification = support::specification()?;
    let comparison = support::section(&specification, "### 22.4 비교 UX")?;
    let sentence = comparison
        .lines()
        .find(|line| line.contains("workload는"))
        .ok_or("section 22.4 no longer says what a workload is displayed with")?;
    let facets = [
        ("표본 수", BiasDimension::SampleCount),
        ("시점", BiasDimension::Recency),
        ("선택 편향", BiasDimension::SelfSelection),
        ("교수/학기 차이", BiasDimension::InstructorTermMix),
    ];
    let mut named = BTreeSet::new();
    for (phrase, dimension) in facets {
        assert!(
            sentence.contains(phrase),
            "section 22.4's workload sentence no longer names {phrase}"
        );
        named.insert(dimension);
    }
    assert!(
        named.is_subset(&BiasDimension::ALL.into_iter().collect()),
        "section 22.4 names a workload facet section 29.5 does not disclose"
    );
    for dimension in named {
        let finding = workload.bias().finding(dimension);
        assert_eq!(finding.dimension(), dimension);
    }

    // There is no point accessor. The whole method inventory of the type is
    // compared against the reviewed one, so a `midpoint` added later fails as
    // an entry nobody wrote down rather than as a forbidden name.
    let source = support::module_source("projected")?;
    let block = support::block_of(&source, "impl ProjectedWorkload {")?;
    let methods: BTreeSet<String> = support::function_names(&block).into_iter().collect();
    let reviewed: BTreeSet<String> = ["of", "proposed", "band", "bias"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        methods, reviewed,
        "the projected workload's method inventory changed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. stale_input_freezes_and_requires_consent
// ---------------------------------------------------------------------------

#[test]
fn stale_input_freezes_and_requires_consent() -> TestResult {
    // The three causes are section 22.5's own, read out of its sentence.
    let specification = support::specification()?;
    let block = support::section(&specification, "### 22.5 과대예측 방지")?;
    let sentence = support::bullets(&block)
        .into_iter()
        .find(|bullet| bullet.contains(STALE_INPUT))
        .ok_or("section 22.5 no longer names STALE_INPUT")?;
    let head = sentence
        .split_once(" 시 scenario")
        .ok_or("section 22.5's stale sentence changed shape")?
        .0;
    let declared: Vec<&str> = head.split('·').map(str::trim).collect();
    let carried: Vec<&str> = STALE_CAUSES
        .into_iter()
        .map(StaleCause::spec_phrase)
        .collect();
    assert_eq!(
        declared, carried,
        "STALE_CAUSES is not section 22.5's own list, in its own order"
    );
    assert!(
        sentence.contains("자동 수정하지 않고") && sentence.contains("재계산 동의를 받는다"),
        "section 22.5 no longer asks for a freeze and a consent"
    );

    let inputs = support::plan_a()?;
    let plan = simulate(&inputs)?;
    let stale = StaleInput::of(
        inputs
            .choices
            .first()
            .ok_or("the fixture plan has no choice")?
            .offering_id(),
        StaleCause::Cancellation,
        TimestampMillis::new(8_000),
    );

    // Freezing does not correct: the plan comes back exactly as it went in.
    let frozen = FrozenPlan::mark(plan.clone(), stale, Vec::new());
    assert_eq!(*frozen.plan(), plan, "freezing a plan changed it");
    assert_eq!(frozen.marker(), STALE_INPUT);
    assert_eq!(frozen.stale(), &[stale]);

    // Only a user decision opens the door, and the whole actor set is walked so
    // the refusal is not a list somebody could forget to extend.
    let user = support::entity(7001)?;
    let actors = [
        Actor::User { user_id: user },
        Actor::DeterministicEngine {
            name: "WHAT_IF".to_owned(),
            version: "1".to_owned(),
        },
        Actor::ModelRun {
            run_id: support::entity(7003)?,
        },
        Actor::Importer {
            name: "TRANSCRIPT".to_owned(),
            version: "1".to_owned(),
        },
    ];
    let mut admitted = 0_usize;
    for actor in &actors {
        match (UserDecision::by(actor), actor) {
            (Ok(_), Actor::User { .. }) => admitted += 1,
            (Ok(_), other) => {
                return Err(format!("{other:?} produced a user decision").into());
            }
            (Err(_), Actor::User { .. }) => {
                return Err("a user actor was refused a decision".into());
            }
            (Err(_), _) => {}
        }
    }
    assert_eq!(admitted, 1, "more than one actor kind opened the consent");

    let decision = UserDecision::by(&Actor::User { user_id: user })?;

    // A consent that names another plan is refused.
    let wrong_plan = RecomputeConsent::of(support::entity(4999)?, decision.clone(), vec![stale]);
    let refused = support::refusal(
        FrozenPlan::mark(plan.clone(), stale, Vec::new()).recompute(&wrong_plan, &inputs),
    )?;
    assert!(matches!(refused, WhatIfError::ConsentNamesAnotherPlan));

    // A consent that covers only some of the stale inputs is refused.
    let second = StaleInput::of(
        inputs
            .choices
            .get(1)
            .ok_or("the fixture plan has one choice")?
            .offering_id(),
        StaleCause::SyllabusChange,
        TimestampMillis::new(8_100),
    );
    let partial = RecomputeConsent::of(plan.id(), decision.clone(), vec![stale]);
    let refused = support::refusal(
        FrozenPlan::mark(plan.clone(), stale, vec![second]).recompute(&partial, &inputs),
    )?;
    assert!(matches!(refused, WhatIfError::ConsentIsIncomplete));

    // A complete consent recomputes, and the recomputation is the engine's.
    let complete = RecomputeConsent::of(plan.id(), decision, vec![stale, second]);
    let recomputed =
        FrozenPlan::mark(plan.clone(), stale, vec![second]).recompute(&complete, &inputs)?;
    assert_eq!(
        recomputed, plan,
        "a consented recomputation over the same inputs differed"
    );

    // The whole function inventory of section 22.5's module, private ones
    // included. A second route out of a frozen plan — a `refresh`, a `retry`, a
    // `rebuild` — fails here as an entry nobody wrote down rather than as a
    // forbidden name.
    let source = support::module_source("stale")?;
    let declared: BTreeSet<String> = support::function_names(&source).into_iter().collect();
    let reviewed: BTreeSet<String> = [
        "as_str",
        "cause",
        "covers",
        "decision",
        "from_cancellation",
        "mark",
        "marker",
        "observed_at",
        "of",
        "offering_id",
        "plan",
        "plan_id",
        "recompute",
        "spec_phrase",
        "stale",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        declared, reviewed,
        "section 22.5's module declares a different function set"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. hypothetical_and_actual_graduation_modes_are_distinct
// ---------------------------------------------------------------------------

#[test]
fn hypothetical_and_actual_graduation_modes_are_distinct() -> TestResult {
    let specification = support::specification()?;
    let block = support::section(&specification, "### 22.5 과대예측 방지")?;
    assert!(
        support::bullets(&block)
            .iter()
            .any(|bullet| bullet.contains("hypothetical mode와 actual mode를 명확히 분리한다")),
        "section 22.5 no longer separates the two graduation modes"
    );

    // The two modes disagree on both questions, walked as a whole set.
    let owners: BTreeSet<&str> = GRADUATION_MODES
        .into_iter()
        .map(GraduationMode::owner_package)
        .collect();
    assert_eq!(
        owners.len(),
        GRADUATION_MODES.len(),
        "two graduation modes claim one owner package"
    );
    let concluding: Vec<GraduationMode> = GRADUATION_MODES
        .into_iter()
        .filter(|mode| mode.concludes_graduation())
        .collect();
    assert_eq!(
        concluding,
        vec![GraduationMode::Actual],
        "the hypothetical mode claims it can conclude a graduation"
    );
    assert_eq!(
        GraduationMode::Hypothetical.owner_package(),
        "academic-what-if"
    );
    assert_eq!(GraduationMode::Actual.owner_package(), "academic-audit");

    // `P2-U3`'s crate is not reachable from this package through an edge of
    // any kind, so `academic_audit::` is not a path that resolves here. The
    // control: a package that does reach it must be reported as reaching it.
    let closure = support::declared_closure("academic-what-if")?;
    assert!(
        !closure.contains("academic-audit"),
        "the plan crate reaches the graduation audit"
    );
    assert!(
        !support::declared_closure("academic-audit")?.contains("academic-what-if"),
        "the graduation audit reaches the plan crate"
    );
    assert!(
        support::declared_closure("academic-export")?.contains("academic-audit"),
        "the closure control failed: academic-export does not reach the audit"
    );

    // Nor is any of that crate's verdict vocabulary spelled here. The names are
    // read out of `P2-U3`'s own `verdict.rs` rather than typed, so a witness or
    // a verdict added there extends this guard without anybody editing this
    // test.
    let verdict = support::strip_non_code(&fs::read_to_string(
        support::workspace_root()
            .join("crates")
            .join("audit")
            .join("src")
            .join("verdict.rs"),
    )?);
    let mut vocabulary = BTreeSet::new();
    for line in verdict.lines() {
        for keyword in ["pub enum ", "pub struct ", "pub type "] {
            let Some(rest) = line.strip_prefix(keyword) else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if !name.is_empty() {
                vocabulary.insert(name);
            }
            break;
        }
    }
    assert!(
        vocabulary.len() >= 5,
        "P2-U3's verdict vocabulary parsed to only {} names",
        vocabulary.len()
    );
    for path in support::product_sources()? {
        let code = support::strip_non_code(&fs::read_to_string(&path)?);
        let names = support::identifiers(&code);
        let spoken: Vec<&String> = vocabulary.intersection(&names).collect();
        assert!(
            spoken.is_empty(),
            "{} names P2-U3's verdict vocabulary: {spoken:?}",
            path.display()
        );
    }
    // The control: the same reader over `P2-U3`'s own engine must find them.
    let engine = support::strip_non_code(&fs::read_to_string(
        support::workspace_root()
            .join("crates")
            .join("audit")
            .join("src")
            .join("engine.rs"),
    )?);
    let engine_names = support::identifiers(&engine);
    let hits = vocabulary.intersection(&engine_names).count();
    assert!(
        hits >= 3,
        "the verdict control found only {hits} names in P2-U3's own engine"
    );

    // One function in the whole package returns a `GraduationMode`, and it is
    // the accessor that returns the constant. A second producer — a
    // constructor parameter, a setter, a converter — would be a route to the
    // actual mode from inside this crate, and it fails here as an extra name.
    let mut producers = BTreeSet::new();
    for path in support::product_sources()? {
        for line in support::strip_non_code(&fs::read_to_string(&path)?).lines() {
            if !line.contains("-> GraduationMode") {
                continue;
            }
            let Some(at) = line.find("fn ") else {
                continue;
            };
            producers.insert(
                line[at + 3..]
                    .chars()
                    .take_while(|character| character.is_alphanumeric() || *character == '_')
                    .collect::<String>(),
            );
        }
    }
    assert_eq!(
        producers,
        ["mode".to_owned()].into_iter().collect::<BTreeSet<_>>(),
        "this crate has a second producer of a graduation mode"
    );

    // Every reading this crate produces is in the hypothetical mode, and it
    // carries the assumption and the banner with it.
    let inputs = support::plan_a()?;
    let plan = simulate(&inputs)?;
    let reading = HypotheticalGraduation::of(&plan);
    assert_eq!(reading.mode(), GraduationMode::Hypothetical);
    assert_eq!(HypotheticalGraduation::MODE, GraduationMode::Hypothetical);
    assert!(!HypotheticalGraduation::BANNER.is_empty());
    assert_eq!(
        reading.completion(),
        academic_what_if::HypotheticalCompletion
    );
    assert!(
        !reading.contributions().is_empty(),
        "a hypothetical reading contributed nothing at all"
    );
    // The proof opens onto the choices the totals were added from, recomputed
    // here from the fixture rather than read off the value under test.
    let mut expected: BTreeMap<_, u16> = BTreeMap::new();
    for choice in &inputs.choices {
        *expected.entry(choice.category()).or_default() += u16::from(choice.credits().value());
    }
    for contribution in reading.contributions() {
        assert_eq!(
            expected.get(&contribution.category()).copied(),
            Some(contribution.credits()),
            "a category contribution disagrees with the plan's own choices"
        );
        assert!(
            !reading
                .proof()
                .proof_for(contribution.category())
                .is_empty(),
            "a contribution has no allocation proof behind it"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. reordering_explains_the_changed_weight
// ---------------------------------------------------------------------------

/// Section 22.4's table, as `(dimension label, certainty label)` pairs.
fn comparison_table(block: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || trimmed.starts_with("|---") {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 4 || cells[0] == "차원" {
            continue;
        }
        found.push((cells[0].to_owned(), cells[3].to_owned()));
    }
    found
}

#[test]
fn reordering_explains_the_changed_weight() -> TestResult {
    let specification = support::specification()?;
    let block = support::section(&specification, "### 22.4 비교 UX")?;
    let declared = comparison_table(&block);
    assert!(
        !declared.is_empty(),
        "section 22.4's table parsed to no rows"
    );
    let carried: Vec<(String, String)> = COMPARISON_DIMENSIONS
        .into_iter()
        .map(|dimension| {
            (
                dimension.spec_label().to_owned(),
                dimension.spec_certainty().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        declared, carried,
        "COMPARISON_DIMENSIONS is not section 22.4's table, in its own order"
    );
    assert!(
        block.contains("왜 정렬이 바뀌었는지 보여준다"),
        "section 22.4 no longer asks why the order changed"
    );

    // The `Mixed` cell is the row whose two halves sit in two different lanes.
    let mixed: Vec<ComparisonDimension> = COMPARISON_DIMENSIONS
        .into_iter()
        .filter(|dimension| dimension.lane() == DimensionLane::Mixed)
        .collect();
    assert_eq!(mixed, vec![ComparisonDimension::DownstreamRoute]);
    for dimension in COMPARISON_DIMENSIONS {
        let expected = match dimension.spec_certainty() {
            "Deterministic if completed" | "Official schedule" => DimensionLane::Deterministic,
            "Mixed" => DimensionLane::Mixed,
            _ => DimensionLane::Projected,
        };
        assert_eq!(
            dimension.lane(),
            expected,
            "{} sits in a lane its certainty column does not support",
            dimension.spec_label()
        );
    }

    let plan_a = simulate(&support::plan_a()?)?;
    let plan_b = simulate(&support::plan_b()?)?;
    let plans = [&plan_a, &plan_b];

    // A priority that leads with the timetable, and one that leads with the
    // critical path. Plan B conflicts with itself and covers less of the path,
    // so the two orders differ.
    let timetable_first = DimensionPriority::of(vec![
        ComparisonDimension::Timetable,
        ComparisonDimension::CriticalPath,
        ComparisonDimension::GraduationRuleContribution,
        ComparisonDimension::ProjectGap,
        ComparisonDimension::Workload,
        ComparisonDimension::DownstreamRoute,
    ])?;
    let workload_first = DimensionPriority::of(vec![
        ComparisonDimension::Workload,
        ComparisonDimension::Timetable,
        ComparisonDimension::CriticalPath,
        ComparisonDimension::GraduationRuleContribution,
        ComparisonDimension::ProjectGap,
        ComparisonDimension::DownstreamRoute,
    ])?;

    let before = compare(&plans, &timetable_first)?;
    let after = compare(&plans, &workload_first)?;
    assert_eq!(before.ranked_ids().len(), 2);
    assert_ne!(
        before.ranked_ids(),
        after.ranked_ids(),
        "the fixture no longer reorders when the weight moves"
    );

    let explanation = ReorderingExplanation::between(&plans, &timetable_first, &workload_first)?;
    assert!(
        !explanation.moved().is_empty(),
        "a reordering explanation named no changed weight"
    );
    // Every named move agrees with the two priorities, recomputed here.
    for moved in explanation.moved() {
        assert_eq!(
            timetable_first.rank_of(moved.dimension()),
            Some(moved.from_rank())
        );
        assert_eq!(
            workload_first.rank_of(moved.dimension()),
            Some(moved.to_rank())
        );
        assert_ne!(moved.from_rank(), moved.to_rank());
    }
    // And every dimension that did move is named.
    for dimension in COMPARISON_DIMENSIONS {
        let moved = timetable_first.rank_of(dimension) != workload_first.rank_of(dimension);
        let named = explanation
            .moved()
            .iter()
            .any(|entry| entry.dimension() == dimension);
        assert_eq!(
            moved,
            named,
            "{} moved without being named, or was named without moving",
            dimension.spec_label()
        );
    }
    assert!(explanation.order_changed());
    assert_eq!(explanation.order_before(), before.ranked_ids());
    assert_eq!(explanation.order_after(), after.ranked_ids());
    assert_eq!(
        explanation.decisive(),
        Some(ComparisonDimension::Workload),
        "the reordering did not name the weight that decided the new leader"
    );

    // The reordering rewrote no fact: the plans are byte-identical afterwards.
    assert_eq!(plan_a, simulate(&support::plan_a()?)?);
    assert_eq!(plan_b, simulate(&support::plan_b()?)?);

    // An unchanged priority has no changed weight to name.
    let refused = support::refusal(ReorderingExplanation::between(
        &plans,
        &timetable_first,
        &timetable_first,
    ))?;
    assert!(matches!(refused, WhatIfError::PriorityDidNotChange));

    // A priority that is not a complete permutation is refused.
    let short = support::refusal(DimensionPriority::of(vec![ComparisonDimension::Workload]))?;
    assert!(matches!(short, WhatIfError::PriorityIsNotAPermutation));
    Ok(())
}

// ---------------------------------------------------------------------------
// 10. end_of_term_calibration_emits_no_user_score
// ---------------------------------------------------------------------------

#[test]
fn end_of_term_calibration_emits_no_user_score() -> TestResult {
    let specification = support::specification()?;
    let block = support::section(&specification, "### 22.5 과대예측 방지")?;
    assert!(
        support::bullets(&block).iter().any(|bullet| {
            bullet.contains("모델을 calibration하되 사용자를 평가하지 않는다")
        }),
        "section 22.5 no longer says the calibration evaluates the model and not the user"
    );

    let inputs = support::plan_a()?;
    let plan = simulate(&inputs)?;
    let projected = plan.projections().opportunities().opportunities.clone();
    assert!(
        !projected.is_empty(),
        "the fixture plan projected no opportunity to calibrate"
    );

    // Observe every projected occasion except one, and one that was never
    // projected at all.
    let mut observed: Vec<ObservedOccasion> = projected
        .iter()
        .skip(1)
        .map(|opportunity| {
            ObservedOccasion::of(
                opportunity.offering_id,
                opportunity.concept_entity_id,
                opportunity.kind,
            )
        })
        .collect();
    let surprise = ObservedOccasion::of(
        inputs
            .choices
            .first()
            .ok_or("the fixture plan has no choice")?
            .offering_id(),
        support::entity(2999)?,
        OpportunityKind::Assessment,
    );
    observed.push(surprise);

    let report = calibrate(&plan, plan.id(), &observed)?;
    assert_eq!(report.plan_id(), plan.id());
    assert_eq!(report.inputs_digest(), plan.inputs_digest());
    assert_eq!(
        report.engine_version(),
        academic_what_if::WHAT_IF_ENGINE_VERSION
    );
    assert_eq!(report.model_run_id(), inputs.model_run_id);

    // The surprise is an under-projection, and the dropped one is judged by the
    // band the plan gave it — recomputed here from the projection rather than
    // read back off the entry.
    let dropped = projected.first().ok_or("no projected opportunity")?;
    let entry = report
        .entries()
        .iter()
        .find(|entry| {
            entry.offering_id() == dropped.offering_id
                && entry.concept_entity_id() == dropped.concept_entity_id
                && entry.kind() == dropped.kind
        })
        .ok_or("the dropped opportunity has no calibration entry")?;
    assert!(!entry.observed());
    let expected = match dropped.likelihood {
        LikelihoodBand::Moderate | LikelihoodBand::High => ProjectionCalibration::Overprojected,
        LikelihoodBand::Low | LikelihoodBand::Unknown => ProjectionCalibration::Matched,
    };
    assert_eq!(entry.direction(), expected);
    let surprise_entry = report
        .entries()
        .iter()
        .find(|entry| {
            entry.concept_entity_id() == surprise.concept_entity_id()
                && entry.kind() == surprise.kind()
        })
        .ok_or("the unprojected occasion has no calibration entry")?;
    assert_eq!(surprise_entry.projected(), None);
    assert!(surprise_entry.observed());
    assert_eq!(
        surprise_entry.direction(),
        ProjectionCalibration::Underprojected
    );
    // The under-projection count, recomputed from the fixture rather than read
    // back off the report: an occasion that happened under a band that did not
    // expect it, plus every occasion that happened and was never projected.
    let expected_under = observed
        .iter()
        .filter(|occasion| {
            projected
                .iter()
                .find(|opportunity| {
                    opportunity.offering_id == occasion.offering_id()
                        && opportunity.concept_entity_id == occasion.concept_entity_id()
                        && opportunity.kind == occasion.kind()
                })
                .is_none_or(|opportunity| {
                    matches!(
                        opportunity.likelihood,
                        LikelihoodBand::Low | LikelihoodBand::Unknown
                    )
                })
        })
        .count();
    assert!(
        expected_under >= 1,
        "the fixture produced no under-projection to count"
    );
    assert_eq!(
        report.count_of(ProjectionCalibration::Underprojected),
        expected_under
    );
    let total: usize = [
        ProjectionCalibration::Underprojected,
        ProjectionCalibration::Matched,
        ProjectionCalibration::Overprojected,
    ]
    .into_iter()
    .map(|direction| report.count_of(direction))
    .sum();
    assert_eq!(total, report.entries().len());

    // A calibration of another plan is refused rather than answered.
    let refused = support::refusal(calibrate(&plan, support::entity(4999)?, &observed))?;
    assert!(matches!(refused, WhatIfError::CalibrationNamesAnotherPlan));

    // The report's whole field inventory names the model, the plan or an
    // occasion, and nothing else. Read out of the source, compared both ways.
    let source = support::module_source("calibration")?;
    let body = support::block_of(&source, "pub struct ModelCalibrationReport {")?;
    let fields: BTreeSet<String> = support::field_declarations(&body)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let reviewed: BTreeSet<String> = [
        "plan_id",
        "engine_version",
        "inputs_digest",
        "model_run_id",
        "entries",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        fields, reviewed,
        "the calibration report's field inventory changed"
    );
    let entry_body = support::block_of(&source, "pub struct CalibrationEntry {")?;
    let entry_fields: BTreeSet<String> = support::field_declarations(&entry_body)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let reviewed_entry: BTreeSet<String> = [
        "offering_id",
        "concept_entity_id",
        "kind",
        "projected",
        "observed",
        "direction",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        entry_fields, reviewed_entry,
        "the calibration entry's field inventory changed"
    );

    // And the whole function inventory of the module, private ones included: a
    // `user_score`, an `accuracy` or a `grade_for` fails as an entry nobody
    // wrote down.
    let declared: BTreeSet<String> = support::function_names(&source).into_iter().collect();
    let reviewed_functions: BTreeSet<String> = [
        "calibrate",
        "concept_entity_id",
        "count_of",
        "direction",
        "direction_of",
        "engine_version",
        "entries",
        "inputs_digest",
        "kind",
        "model_run_id",
        "observed",
        "of",
        "offering_id",
        "plan_id",
        "projected",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        declared, reviewed_functions,
        "section 22.5's calibration module declares a different function set"
    );
    Ok(())
}
