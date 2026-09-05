//! `P2-X3`'s acceptance evidence.
//!
//! Nine of `t068`'s ten named tests are here. The tenth,
//! `planner_has_no_registration_endpoint`, is an absence over this crate's whole
//! source and is in `tests/dashboard_scans.rs` beside the whole-set readers it
//! needs.
//!
//! **No window opens.** No Tauri runtime is linked and nothing here is evidence
//! that one is. Every test below drives typed values and compares them against
//! the design document's own text or against another crate's own answer.
//!
//! **No count is asserted.** Every enumeration is parsed out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compared with this
//! crate's in both directions and in order.

mod support;

#[path = "support/fixtures.rs"]
mod fixtures;

use std::collections::{BTreeMap, BTreeSet};

use academic_curriculum::{CourseCode, CurriculumCategory};
use academic_domain::{engines::ProofStatus, predicates::PredicateName};
use academic_record::{
    classify::ProgramId,
    corpus,
    plan::PlanScenarioChoice,
    term::TermKey,
    views::{GpaValue, RecordViews},
};
use academic_review::{DimensionBand, OfferingAggregate, ReviewDimension};

use academic_dashboard::{
    AcademicDashboard, AttemptTimeline, AuditState, AuditStateReading, BreakdownPart,
    CatalogIdentity, Connections, CourseDetail, CourseSection, CoverageEntry, CoverageReport,
    CoverageTab, DashboardError, DashboardLine, DashboardSection, FacetReading, GpaFigure,
    GpaProof, GpaScope, LifecycleFacet, OfferingRow, OpenGate, PlanSnapshot, PlannerBoard,
    PlannerDimension, RequirementBreakdown, ReviewSection, SecondaryPercentage, StaleInput,
    TimelineEntry,
};

use support::{TestResult, back_quoted, bullet_starting, bullets, fenced_text, section, spec};

// ---------------------------------------------------------------------------
// dashboard_shows_three_gpas_with_proof
// ---------------------------------------------------------------------------

/// Three averages, each carrying the attempts `P2-U4`'s engine used.
///
/// Four halves, and none of them is a count.
///
/// 1. Section 25.4's first line is split on its own middle dot and the pieces
///    compared with `GpaScope::spec_word` position by position and as sets in
///    both directions, so a fourth scope in the document fails as a missing key
///    and a fourth arm here fails as an extra one.
/// 2. Every figure the screen publishes is compared against
///    `academic-record`'s **own** answer over the same corpus. The oracle is
///    that crate's engine and not this one: this crate has no arithmetic over
///    grades at all.
/// 3. The three proofs are required to differ from each other. Three figures
///    carrying one proof would satisfy "each has a proof" and say nothing.
/// 4. `GpaFigure::publish` is driven at each refusal: a `Known` average with an
///    empty proof, and an `Unknown` average whose proof does not name an
///    attempt the value itself names.
#[test]
fn dashboard_shows_three_gpas_with_proof() -> TestResult {
    let text = spec()?;
    let block = section(&text, "25.4 Academic Dashboard")?;
    let line = bullet_starting(&block, "누적")?;
    let spelled: Vec<&str> = line
        .split_whitespace()
        .next()
        .ok_or("section 25.4's first line is empty")?
        .split('·')
        .collect();
    let declared: Vec<&str> = GpaScope::ALL.into_iter().map(GpaScope::spec_word).collect();
    assert_eq!(
        spelled, declared,
        "section 25.4's averages and GpaScope::ALL have diverged"
    );
    assert_eq!(
        spelled.iter().copied().collect::<BTreeSet<_>>(),
        declared.iter().copied().collect::<BTreeSet<_>>(),
        "the two enumerations differ as sets"
    );
    // And the line really does ask each of them for a proof.
    assert!(
        line.contains("각 계산 proof"),
        "section 25.4 no longer asks each calculation for its proof"
    );

    // The oracle: `academic-record`'s own engine over its own corpus.
    let history = corpus::baseline_history()?;
    let rules = corpus::baseline_rules()?;
    let classification = corpus::classification_v1()?;
    let views = RecordViews::compute(&history, &rules, &classification)?;
    let term = TermKey::parse("2014_SPRING")?;
    let program = ProgramId::new(corpus::PRIMARY_PROGRAM)?;

    let figures = vec![
        GpaFigure::publish(
            GpaScope::Cumulative,
            views.cumulative_gpa()?,
            GpaProof::recording(
                views.cumulative_included(),
                dispositions_of(&views),
                views.repeat_proofs().to_vec(),
            ),
        )?,
        GpaFigure::publish(
            GpaScope::Term,
            views.term_gpa(term)?,
            GpaProof::recording(
                included_in_term(&views, term),
                dispositions_of(&views),
                views.repeat_proofs().to_vec(),
            ),
        )?,
        GpaFigure::publish(
            GpaScope::Major,
            views.major_gpa(&program)?,
            GpaProof::recording(
                included_in_major(&views, &program),
                dispositions_of(&views),
                views.repeat_proofs().to_vec(),
            ),
        )?,
    ];

    let screen = AcademicDashboard::assemble(filled_sections(figures, &history)?, &[], None)?;
    let published = screen
        .averages()
        .ok_or("the averages line is blocked in a screen with no open cell")?;
    assert_eq!(published.len(), GpaScope::ALL.len());
    for (index, scope) in GpaScope::ALL.into_iter().enumerate() {
        let figure = published
            .get(index)
            .ok_or_else(|| format!("no figure at position {index}"))?;
        assert_eq!(figure.scope(), scope, "the figures are out of order");
        assert_eq!(
            screen.average(scope).map(GpaFigure::value),
            Some(figure.value())
        );
    }

    // 2. Each published value is the record's own.
    assert_eq!(
        screen.average(GpaScope::Cumulative).map(GpaFigure::value),
        Some(&views.cumulative_gpa()?)
    );
    assert_eq!(
        screen.average(GpaScope::Term).map(GpaFigure::value),
        Some(&views.term_gpa(term)?)
    );
    assert_eq!(
        screen.average(GpaScope::Major).map(GpaFigure::value),
        Some(&views.major_gpa(&program)?)
    );

    // 3. And the three proofs are three proofs.
    let cumulative = screen
        .average(GpaScope::Cumulative)
        .ok_or("no cumulative figure")?;
    let scoped = screen.average(GpaScope::Term).ok_or("no term figure")?;
    let major = screen.average(GpaScope::Major).ok_or("no major figure")?;
    for figure in [cumulative, scoped, major] {
        assert!(
            !figure.proof().included().is_empty(),
            "the {} figure carries no included attempt",
            figure.scope()
        );
        assert!(
            !figure.proof().reasons().is_empty(),
            "the {} figure carries no disposition reason",
            figure.scope()
        );
    }
    let whole: BTreeSet<_> = cumulative.proof().included().iter().collect();
    let one_term: BTreeSet<_> = scoped.proof().included().iter().collect();
    let one_major: BTreeSet<_> = major.proof().included().iter().collect();
    assert!(
        one_term.is_subset(&whole) && one_term != whole,
        "the term proof is not a proper subset of the cumulative one"
    );
    assert!(
        one_major.is_subset(&whole) && one_major != whole,
        "the major proof is not a proper subset of the cumulative one"
    );
    assert_ne!(
        one_term, one_major,
        "the term and major proofs name the same attempts"
    );

    // 4. The refusals, driven.
    let empty = GpaFigure::publish(
        GpaScope::Cumulative,
        views.cumulative_gpa()?,
        GpaProof::recording(Vec::new(), Vec::new(), Vec::new()),
    );
    assert_eq!(
        empty.err(),
        Some(DashboardError::AverageWithoutProof {
            scope: GpaScope::Cumulative
        }),
        "a known average with no attempt behind it was published"
    );
    let missing = GpaFigure::publish(
        GpaScope::Term,
        GpaValue::Unknown(vec![corpus::synthetic_attempt_id(1)?]),
        GpaProof::recording(Vec::new(), Vec::new(), Vec::new()),
    );
    assert_eq!(
        missing.err(),
        Some(DashboardError::ProofOmitsUnknownAttempts {
            scope: GpaScope::Term,
            missing: 1
        }),
        "an unknown average was published without naming the attempt it is unknown for"
    );
    // And the same value with the attempt named is accepted, so the refusal is
    // about the omission rather than about the value.
    GpaFigure::publish(
        GpaScope::Term,
        GpaValue::Unknown(vec![corpus::synthetic_attempt_id(1)?]),
        GpaProof::recording(Vec::new(), dispositions_of(&views), Vec::new()),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// dashboard_no_composite
// ---------------------------------------------------------------------------

/// No card folds two figures, and the other half of the fold is unnameable.
///
/// Section 10's last paragraph: *Academic Dashboard에서 GPA chart와 Knowledge
/// Map을 같은 카드의 한 score로 합치지 않는다.* Section 36.9 closes with *한
/// 학기의 결과는 "Database 83%"가 아니라*, and section 35's table forbids the
/// 단순 GPA/졸업 계산기 from the other end.
///
/// Three halves.
///
/// 1. **The sentence is read out of the document** rather than restated here.
/// 2. **Each of the six sections holds one line's values.** The section-to-line
///    map is checked to be injective and total over `DashboardLine::ALL`, so a
///    card that carried two lines' values would have to be a seventh arm.
/// 3. **The other operand cannot be spelled.** That half is
///    `the_dashboard_surface_cannot_name_a_mastery` in `tests/dashboard_scans.rs`,
///    which reads the whole set of capitalized identifiers of every product file
///    and requires `MasteryLevel`, `KnowledgeState`, `ConceptReading`,
///    `FreshnessBand` and `FreshnessProjection` to be absent from it while
///    requiring two of them to be `academic-domain`'s, which *is* a product
///    edge.
#[test]
fn dashboard_no_composite() -> TestResult {
    let text = spec()?;
    let separation = text
        .lines()
        .find(|line| line.contains("같은 카드의 한 score로"))
        .ok_or("section 10 no longer forbids folding the two into one score")?;
    assert!(
        separation.contains("GPA chart") && separation.contains("Knowledge Map"),
        "section 10's sentence no longer names both halves"
    );
    assert!(
        text.contains("한 학기의 결과는 “Database 83%”가 아니라"),
        "section 36.9 no longer refuses the composite percentage"
    );

    let history = corpus::baseline_history()?;
    let rules = corpus::baseline_rules()?;
    let classification = corpus::classification_v1()?;
    let views = RecordViews::compute(&history, &rules, &classification)?;
    let figures = vec![GpaFigure::publish(
        GpaScope::Cumulative,
        views.cumulative_gpa()?,
        GpaProof::recording(
            views.cumulative_included(),
            dispositions_of(&views),
            views.repeat_proofs().to_vec(),
        ),
    )?];
    let screen = AcademicDashboard::assemble(filled_sections(figures, &history)?, &[], None)?;

    // Every section answers for exactly one line, and every line has exactly
    // one section. Both directions, so neither a doubled card nor a missing one
    // passes.
    let mut seen: BTreeMap<DashboardLine, usize> = BTreeMap::new();
    for (index, region) in screen.sections().iter().enumerate() {
        let line = region
            .line()
            .ok_or_else(|| format!("the section at {index} answers for no line"))?;
        assert!(
            seen.insert(line, index).is_none(),
            "two sections answer for {line:?}"
        );
        assert_eq!(
            DashboardLine::ALL.get(index),
            Some(&line),
            "the section at {index} is out of section 25.4's order"
        );
    }
    assert_eq!(
        seen.keys().copied().collect::<Vec<_>>(),
        DashboardLine::ALL.to_vec(),
        "the sections and section 25.4's lines have diverged"
    );

    // The averages arrive as figures rather than as a number, and the screen
    // offers no other route to them.
    let published = screen.averages().ok_or("the averages line is blocked")?;
    assert_eq!(published.len(), 1);
    assert!(
        matches!(
            published.first().map(GpaFigure::value),
            Some(GpaValue::Known(_))
        ),
        "the corpus no longer produces a known cumulative average, so this check is empty"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// audit_states_are_exactly_four
// ---------------------------------------------------------------------------

/// Four display words, five engine statuses, and the mapping between them.
///
/// The four are read out of section 25.4's own back quotes and compared with
/// `AuditState::ALL` in both directions **and in order**; then the words are
/// removed from the line and what is left is required to be punctuation, so a
/// fifth word in the document leaves text behind and fails rather than being
/// folded into the nearest arm. `P2-X2`'s `permission_status_is_exactly_four_values`
/// is the same shape one surface over.
///
/// Then the discrepancy, measured rather than asserted. `P2-U3`'s engine
/// publishes `academic_domain::engines::ProofStatus`, which has five arms, and
/// section 11.3's own rendered tree shows two of them on two lines. This test
/// requires:
///
/// * the mapping to be **total** over `ProofStatus::ALL`;
/// * exactly one display word to be the image of more than one status, and that
///   word to be `REMAINING`, and those statuses to be `NEEDS` and
///   `NOT_SATISFIED`;
/// * the three shared spellings to map to themselves;
/// * and no path through this crate to lose the engine's own status:
///   `AuditStateReading::engine_status` returns it for every one of the five,
///   and there is no constructor taking an `AuditState`.
///
/// `academic-audit` is a dev edge, so the five are compared against that
/// crate's own vocabulary rather than being made equal to it by construction.
#[test]
fn audit_states_are_exactly_four() -> TestResult {
    let text = spec()?;
    let block = section(&text, "25.4 Academic Dashboard")?;
    let line = bullet_starting(&block, "졸업 audit")?;
    let spelled = back_quoted(&line);
    let declared: Vec<String> = AuditState::ALL
        .into_iter()
        .map(|state| state.spec_word().to_owned())
        .collect();
    assert_eq!(
        spelled, declared,
        "section 25.4's audit states and AuditState::ALL have diverged"
    );
    assert_eq!(
        spelled.iter().collect::<BTreeSet<_>>(),
        declared.iter().collect::<BTreeSet<_>>(),
        "the two enumerations differ as sets"
    );

    // A fifth word in the line would leave text behind.
    let mut residue = line.clone();
    for word in &spelled {
        residue = residue.replace(&format!("`{word}`"), "");
    }
    let leftover: String = residue
        .replace("졸업 audit의", "")
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',' && *character != '.')
        .collect();
    assert!(
        leftover.is_empty(),
        "section 25.4's audit line holds something that is not one of the four: {leftover:?}"
    );

    // The mapping is total over the engine's five, and the collapse is exactly
    // where this crate says it is.
    let mut image: BTreeMap<AuditState, Vec<ProofStatus>> = BTreeMap::new();
    for status in ProofStatus::ALL {
        image
            .entry(AuditState::of(status))
            .or_default()
            .push(status);
    }
    assert_eq!(
        image.keys().copied().collect::<Vec<_>>(),
        AuditState::ALL.to_vec(),
        "the display words are not exactly the image of the engine's statuses"
    );
    let collapsed: Vec<(&AuditState, &Vec<ProofStatus>)> = image
        .iter()
        .filter(|(_, statuses)| statuses.len() > 1)
        .collect();
    assert_eq!(
        collapsed,
        vec![(
            &AuditState::Remaining,
            &vec![ProofStatus::Needs, ProofStatus::NotSatisfied]
        )],
        "the collapse is somewhere other than REMAINING over NEEDS and NOT_SATISFIED"
    );
    for status in [
        ProofStatus::Satisfied,
        ProofStatus::Unknown,
        ProofStatus::Conflict,
    ] {
        assert_eq!(
            AuditState::of(status).spec_word(),
            status.as_str(),
            "a status whose spelling section 25.4 shares was mapped somewhere else"
        );
    }

    // Nothing loses the status the word came from.
    for status in ProofStatus::ALL {
        let reading = AuditStateReading::of(status);
        assert_eq!(reading.engine_status(), status);
        assert_eq!(reading.state(), AuditState::of(status));
        assert_eq!(
            reading.is_evaluated(),
            !matches!(status, ProofStatus::Unknown | ProofStatus::Conflict)
        );
    }
    let needs = AuditStateReading::of(ProofStatus::Needs);
    let not_satisfied = AuditStateReading::of(ProofStatus::NotSatisfied);
    assert_eq!(needs.state(), not_satisfied.state());
    assert_ne!(
        needs, not_satisfied,
        "two readings that show one word are the same value, so the engine's \
         distinction is lost after all"
    );

    // And the engine really does publish five, read from `P2-U3`'s own crate.
    let audit_gates: Vec<&str> = academic_audit::OpenGate::ALL
        .into_iter()
        .map(academic_audit::OpenGate::identifier)
        .collect();
    assert!(
        audit_gates.contains(&"GATE-38-001"),
        "academic-audit no longer names the cell this surface forwards"
    );
    assert_eq!(
        ProofStatus::ALL
            .into_iter()
            .map(ProofStatus::as_str)
            .collect::<Vec<_>>(),
        vec!["SATISFIED", "NEEDS", "NOT_SATISFIED", "UNKNOWN", "CONFLICT"],
        "the engine's status vocabulary moved and this crate's mapping did not"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// attempt_timeline_preserves_six_lifecycle_facets
// ---------------------------------------------------------------------------

/// Six facets, each readable on every row, and none of them constant.
///
/// The six are split out of section 25.4's own line on its own slashes and
/// compared with `LifecycleFacet::spec_word` in order and as sets. Then three
/// properties that a timeline of six constants would fail:
///
/// 1. **Every facet varies.** Each of the six reads `Present` on at least one
///    row and not-`Present` on another. A facet that read the same everywhere
///    is one no assertion below could fail on.
/// 2. **They are independent.** No two facets have the same reading on every
///    row, so none of the six is a spelling of another.
/// 3. **A repeat preserves the earlier attempt.** Appending a repeat leaves the
///    original row's six readings identical, which is section 10's *재수강과
///    취소를 덮어쓰지 않고 매 시도를 보존한다*.
///
/// `예정` is why the timeline reads two sources: `academic-record` has **no**
/// constructor producing a `PLANNED` attempt, so a timeline over the ledger
/// alone would read that facet absent on every row forever. This test asserts
/// that emptiness would occur, by building the ledger-only timeline and
/// observing it.
#[test]
fn attempt_timeline_preserves_six_lifecycle_facets() -> TestResult {
    let text = spec()?;
    let block = section(&text, "25.4 Academic Dashboard")?;
    let line = bullet_starting(&block, "수강 시도 timeline")?;
    let listed = line
        .split_once(": ")
        .map(|(_, rest)| rest)
        .ok_or("section 25.4's timeline line no longer lists its facets")?;
    let spelled: Vec<&str> = listed.trim_end_matches('.').split('/').collect();
    let declared: Vec<&str> = LifecycleFacet::ALL
        .into_iter()
        .map(LifecycleFacet::spec_word)
        .collect();
    assert_eq!(
        spelled, declared,
        "section 25.4's facets and LifecycleFacet::ALL have diverged"
    );
    assert_eq!(
        spelled.iter().copied().collect::<BTreeSet<_>>(),
        declared.iter().copied().collect::<BTreeSet<_>>(),
        "the two enumerations differ as sets"
    );

    let history = corpus::baseline_history()?;
    let planned = vec![
        PlanScenarioChoice::new("4190.409", TermKey::parse("2027_SPRING")?)?,
        PlanScenarioChoice::new("4190.410", TermKey::parse("2027_SPRING")?)?,
    ];

    // The emptiness this design avoids, observed rather than described.
    let ledger_only = AttemptTimeline::of(&history, &[]);
    assert!(
        ledger_only
            .entries()
            .iter()
            .all(|entry| entry.facet(LifecycleFacet::Planned) != FacetReading::Present),
        "a PLANNED attempt reached the ledger, so the two-source timeline is unnecessary"
    );

    let timeline = AttemptTimeline::of(&history, &planned);
    assert_eq!(
        timeline.entries().len(),
        history.current().len() + planned.len(),
        "the timeline lost a row"
    );

    // 1. Every facet varies.
    for facet in LifecycleFacet::ALL {
        let readings: Vec<FacetReading> = timeline
            .entries()
            .iter()
            .map(|entry| entry.facet(facet))
            .collect();
        assert!(
            readings.contains(&FacetReading::Present),
            "no row reads {} as present, so that facet is a constant",
            facet.spec_word()
        );
        assert!(
            readings
                .iter()
                .any(|reading| *reading != FacetReading::Present),
            "every row reads {} as present, so that facet is a constant",
            facet.spec_word()
        );
    }

    // 2. And no two are the same reading everywhere.
    let profile = |facet: LifecycleFacet| -> Vec<FacetReading> {
        timeline
            .entries()
            .iter()
            .map(|entry| entry.facet(facet))
            .collect()
    };
    for (index, facet) in LifecycleFacet::ALL.into_iter().enumerate() {
        for other in LifecycleFacet::ALL.into_iter().skip(index + 1) {
            assert_ne!(
                profile(facet),
                profile(other),
                "{} and {} read the same on every row",
                facet.spec_word(),
                other.spec_word()
            );
        }
    }

    // 3. A repeat preserves what came before it.
    let original = corpus::synthetic_attempt_id(1)?;
    let before = timeline
        .entry_for(original)
        .ok_or("the corpus no longer holds the original of the repeat group")?
        .clone();
    let mut grown = corpus::baseline_history()?;
    let third = academic_record::attempt::CourseAttempt::from_confirmed_row(
        corpus::synthetic_attempt_id(20)?,
        corpus::COURSE_REPEATED,
        TermKey::parse("2016_SPRING")?,
        academic_record::attempt::SettledStatus::Completed,
        academic_record::policy::AttemptOrigin::Internal,
        academic_domain::Decimal::new(30, 1)?,
        academic_domain::Decimal::new(30, 1)?,
        Some(academic_record::grade::GradeSymbol::BZero),
        academic_record::grade::GradingScheme::snu_4_3_v1()?
            .id()
            .to_owned(),
        vec![corpus::synthetic_evidence_id(20)?],
    )?
    .as_repeat_of(original);
    grown.append(third)?;
    let after_timeline = AttemptTimeline::of(&grown, &planned);
    let after = after_timeline
        .entry_for(original)
        .ok_or("the original row is gone from the timeline after a repeat")?;
    assert_eq!(
        &before, after,
        "appending a repeat changed the earlier attempt's row"
    );
    for facet in LifecycleFacet::ALL {
        assert_eq!(
            before.facet(facet),
            after.facet(facet),
            "appending a repeat changed the earlier attempt's {} reading",
            facet.spec_word()
        );
    }
    assert!(
        after_timeline.entries().len() > timeline.entries().len(),
        "the repeat did not add a row"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// planner_reevaluates_six_dimensions_on_drag
// ---------------------------------------------------------------------------

/// Six axes, re-read from the whole board on every drag.
///
/// The six bullets are parsed out of section 25.5 and compared with
/// `PlannerDimension::spec_line` in order and as sets. Then four properties,
/// and the last three are what a board returning six constants would fail.
///
/// 1. **Every drag answers on all six.**
/// 2. **Every axis moves.** Placing a second candidate changes the reading on
///    each of the six, so none of them is a constant the placement never
///    reaches.
/// 3. **It is re-evaluation and not accumulation.** Removing what was placed
///    returns the earlier outcome **exactly**, so no reading survived the
///    removal.
/// 4. **A conflict is read from the board rather than from a candidate.** Two
///    candidates that overlap produce a conflict entry that neither produces
///    alone, which is the one axis whose value is not a union of per-candidate
///    facts.
#[test]
fn planner_reevaluates_six_dimensions_on_drag() -> TestResult {
    let text = spec()?;
    let block = section(&text, "25.5 Semester Planner")?;
    assert!(
        block.contains("과목을 끌어놓으면 다음을 즉시 재평가한다"),
        "section 25.5 no longer says a drag re-evaluates"
    );
    let spelled = bullets(&block);
    let declared: Vec<String> = PlannerDimension::ALL
        .into_iter()
        .map(|dimension| dimension.spec_line().to_owned())
        .collect();
    assert_eq!(
        spelled, declared,
        "section 25.5's axes and PlannerDimension::ALL have diverged"
    );
    assert_eq!(
        spelled.iter().collect::<BTreeSet<_>>(),
        declared.iter().collect::<BTreeSet<_>>(),
        "the two enumerations differ as sets"
    );

    let first = fixtures::candidate(1, "4190.101", "T20271", 540)?;
    let second = fixtures::candidate(2, "4190.210", "T20271", 900)?;
    let board = PlannerBoard::new();
    let (one, first_outcome) = board.place(first.clone())?;
    assert_eq!(first_outcome.readings().len(), PlannerDimension::ALL.len());
    for dimension in PlannerDimension::ALL {
        assert_eq!(first_outcome.reading(dimension).dimension(), dimension);
        assert!(
            !first_outcome.reading(dimension).entries().is_empty(),
            "{} answered with nothing after the first drag",
            dimension.spec_line()
        );
    }

    let (two, second_outcome) = one.place(second.clone())?;
    for dimension in PlannerDimension::ALL {
        assert_ne!(
            first_outcome.reading(dimension).entries(),
            second_outcome.reading(dimension).entries(),
            "{} did not move when a second course was dropped on the timetable",
            dimension.spec_line()
        );
    }

    // 3. Removing it returns the earlier answer exactly.
    let (back, back_outcome) = two.remove(second.offering());
    assert_eq!(back, one, "the board did not return to its earlier state");
    assert_eq!(
        back_outcome, first_outcome,
        "a reading survived the removal that produced it"
    );

    // 4. A conflict is a property of the pair.
    let overlapping = fixtures::candidate(3, "4190.310", "T20271", 570)?;
    let (_, alone) = PlannerBoard::new().place(overlapping.clone())?;
    let conflicts = |outcome: &academic_dashboard::DragOutcome| -> usize {
        outcome
            .reading(PlannerDimension::CreditsConflictsAndPrerequisites)
            .entries()
            .iter()
            .filter(|entry| entry.starts_with("conflict/"))
            .count()
    };
    assert_eq!(conflicts(&alone), 0);
    assert_eq!(conflicts(&first_outcome), 0);
    let (_, together) = one.place(overlapping)?;
    assert_eq!(
        conflicts(&together),
        1,
        "two overlapping meetings produced no conflict"
    );

    // The same offering twice is one placement.
    assert_eq!(
        one.place(first).err(),
        Some(DashboardError::OfferingIsAlreadyPlaced(
            fixtures::offering_id(1)?.to_string()
        ))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// plan_snapshot_is_immutable_and_stale_marked
// ---------------------------------------------------------------------------

/// A saved plan does not move, and a changed official reading is only named.
///
/// > 안 A/B/C를 고정 snapshot으로 저장하고, 공식 정보가 바뀌면 무엇이
/// > stale해졌는지만 표시한다.
///
/// The sentence is read out of the document, then:
///
/// 1. **The snapshot is unchanged by `restate`**, compared as a whole value
///    before and after rather than by trusting the signature.
/// 2. **Each kind of change is named**, one at a time, so a marking that
///    reported everything or nothing fails. A withdrawn offering, a credit
///    change, a timetable change and a prerequisite change each produce their
///    own entry and no other.
/// 3. **An unchanged reading marks nothing**, which is the control: a marking
///    that always fired would satisfy every assertion in 2.
/// 4. **The stale entry carries no replacement.** Every `StaleInput` arm holds
///    an offering identity and nothing else, so applying the change is not
///    something a caller could do from what it is given.
#[test]
fn plan_snapshot_is_immutable_and_stale_marked() -> TestResult {
    let text = spec()?;
    let block = section(&text, "25.5 Semester Planner")?;
    assert!(
        block.contains("무엇이 stale해졌는지만 표시한다"),
        "section 25.5 no longer limits the response to naming what went stale"
    );
    assert!(
        block.contains("고정 snapshot으로 저장"),
        "section 25.5 no longer fixes the plan as a snapshot"
    );

    let first = fixtures::candidate(1, "4190.101", "T20271", 540)?;
    let second = fixtures::candidate(2, "4190.210", "T20271", 900)?;
    let (board, _) = PlannerBoard::new().place(first.clone())?;
    let (board, _) = board.place(second.clone())?;
    let snapshot = PlanSnapshot::save("안 A", &board)?;
    let saved = snapshot.clone();

    // 3. The control first: an unchanged reading marks nothing.
    let current = vec![first.clone(), second.clone()];
    assert!(
        snapshot.restate(&current).is_current(),
        "an unchanged official reading was reported stale"
    );
    assert_eq!(snapshot, saved, "restate changed the snapshot");

    // 2. One change at a time.
    let gone = vec![first.clone()];
    assert_eq!(
        snapshot.restate(&gone).stale(),
        &[StaleInput::OfferingIsGone(second.offering())],
        "a withdrawn offering was not named, or something else was"
    );

    let moved_credits = fixtures::candidate(2, "4190.210", "T20271", 900)?;
    let moved_credits = academic_dashboard::CandidateOffering::declaring(
        moved_credits.offering(),
        moved_credits.course_code().clone(),
        moved_credits.term().clone(),
        4,
        moved_credits.meeting(),
        moved_credits.prerequisites().to_vec(),
        moved_credits.contributions().to_vec(),
        moved_credits.exposes().to_vec(),
        moved_credits.relevant_to().to_vec(),
        moved_credits.workload().clone(),
        moved_credits.unlocks().to_vec(),
    );
    assert_eq!(
        snapshot.restate(&[first.clone(), moved_credits]).stale(),
        &[StaleInput::CreditsMoved(second.offering())]
    );

    let moved_meeting = fixtures::candidate(2, "4190.210", "T20271", 960)?;
    assert_eq!(
        snapshot.restate(&[first.clone(), moved_meeting]).stale(),
        &[StaleInput::MeetingMoved(second.offering())]
    );

    let base = fixtures::candidate(2, "4190.210", "T20271", 900)?;
    let moved_prerequisites = academic_dashboard::CandidateOffering::declaring(
        base.offering(),
        base.course_code().clone(),
        base.term().clone(),
        base.credits(),
        base.meeting(),
        vec![CourseCode::parse("4190.999")?],
        base.contributions().to_vec(),
        base.exposes().to_vec(),
        base.relevant_to().to_vec(),
        base.workload().clone(),
        base.unlocks().to_vec(),
    );
    assert_eq!(
        snapshot
            .restate(&[first.clone(), moved_prerequisites])
            .stale(),
        &[StaleInput::PrerequisitesMoved(second.offering())]
    );

    // 1. And the snapshot is byte-for-byte what it was, after all of that.
    assert_eq!(snapshot, saved, "restate changed the snapshot");
    assert_eq!(snapshot.placed(), board.placed());
    assert_eq!(snapshot.outcome(), &board.evaluate());

    // 4. A stale entry carries an identity and no replacement value.
    for entry in snapshot.restate(&gone).stale() {
        match entry {
            StaleInput::OfferingIsGone(id)
            | StaleInput::CreditsMoved(id)
            | StaleInput::MeetingMoved(id)
            | StaleInput::PrerequisitesMoved(id) => {
                assert_eq!(*id, second.offering());
            }
        }
    }

    // And the two refusals section 25.5's own sentence implies.
    assert_eq!(
        PlanSnapshot::save("   ", &board).err(),
        Some(DashboardError::SnapshotWithoutLabel)
    );
    assert_eq!(
        PlanSnapshot::save("안 B", &PlannerBoard::new()).err(),
        Some(DashboardError::SnapshotOfAnEmptyBoard)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// coverage_tabs_are_non_overlapping
// ---------------------------------------------------------------------------

/// The four tabs partition the coverage evidence.
///
/// The four words are read out of section 25.6's own fenced block, together
/// with the parenthesis that says they do not overlap. Then the partition
/// itself, measured three ways:
///
/// 1. **Union.** Every entry of a report appears on exactly one tab, and the
///    tabs together hold every entry.
/// 2. **Disjointness.** Every pairwise intersection is empty, checked over all
///    six pairs rather than asserted.
/// 3. **One subject, three tabs.** The same concept under three predicates
///    appears once on each of three tabs and not at all on the fourth, so the
///    tabs partition the *evidence* rather than the concepts.
///
/// And the tab a predicate belongs to is `academic-domain`'s registry, not a
/// name: `CoverageTab::predicate` returns a `PredicateName`, the four are four
/// distinct arms of section 7.2's twenty, and a fifth predicate is refused by
/// `CoverageEntry::of` rather than landing on a default tab.
#[test]
fn coverage_tabs_are_non_overlapping() -> TestResult {
    let text = spec()?;
    let block = section(&text, "25.6 Course Detail")?;
    let fenced = fenced_text(&block)?;
    let line = fenced
        .lines()
        .find(|line| line.contains("DESIGNED"))
        .ok_or("section 25.6's fenced block no longer names the coverage tabs")?;
    assert!(
        line.contains("겹치지 않는 탭"),
        "section 25.6 no longer says the tabs do not overlap"
    );
    let spelled: Vec<&str> = line
        .split('(')
        .next()
        .unwrap_or(line)
        .split('/')
        .map(str::trim)
        .collect();
    let declared: Vec<&str> = CoverageTab::ALL
        .into_iter()
        .map(CoverageTab::spec_word)
        .collect();
    assert_eq!(
        spelled, declared,
        "section 25.6's tabs and CoverageTab::ALL have diverged"
    );

    // The four predicates are four distinct arms of section 7.2's registry.
    let predicates: BTreeSet<PredicateName> = CoverageTab::ALL
        .into_iter()
        .map(CoverageTab::predicate)
        .collect();
    assert_eq!(predicates.len(), CoverageTab::ALL.len());
    for tab in CoverageTab::ALL {
        assert_eq!(CoverageTab::of(tab.predicate()), Some(tab));
    }
    let unmapped: Vec<&str> = PredicateName::ALL
        .into_iter()
        .filter(|predicate| CoverageTab::of(*predicate).is_none())
        .map(PredicateName::as_str)
        .collect();
    assert_eq!(
        unmapped.len(),
        PredicateName::ALL.len() - CoverageTab::ALL.len(),
        "a predicate outside the four is on a tab"
    );
    assert!(
        unmapped.contains(&"APPLIED_IN"),
        "the control predicate is no longer outside the four"
    );

    let subject = fixtures::entity_id(7)?;
    let other = fixtures::entity_id(8)?;
    let mut entries = Vec::new();
    for tab in CoverageTab::ALL {
        entries.push(CoverageEntry::of(
            tab.predicate(),
            other,
            format!("evidence/{}", tab.spec_word()),
        )?);
    }
    // The same subject under three predicates and not the fourth.
    for tab in [
        CoverageTab::Designed,
        CoverageTab::Taught,
        CoverageTab::Practiced,
    ] {
        entries.push(CoverageEntry::of(
            tab.predicate(),
            subject,
            format!("shared/{}", tab.spec_word()),
        )?);
    }
    let report = CoverageReport::over(entries);

    // 1. Union.
    let mut union: Vec<&CoverageEntry> = Vec::new();
    for tab in CoverageTab::ALL {
        union.extend(report.tab(tab));
    }
    assert_eq!(
        union.len(),
        report.entries().len(),
        "the tabs together do not hold every entry"
    );
    for entry in report.entries() {
        let holders: Vec<CoverageTab> = CoverageTab::ALL
            .into_iter()
            .filter(|tab| report.tab(*tab).contains(&entry))
            .collect();
        assert_eq!(
            holders,
            vec![entry.tab()],
            "an entry is on {} tabs",
            holders.len()
        );
    }

    // 2. Disjointness, over all six pairs.
    for (index, tab) in CoverageTab::ALL.into_iter().enumerate() {
        let left: BTreeSet<&str> = report
            .tab(tab)
            .into_iter()
            .map(CoverageEntry::evidence)
            .collect();
        assert!(!left.is_empty(), "{} holds nothing", tab.spec_word());
        for other in CoverageTab::ALL.into_iter().skip(index + 1) {
            let right: BTreeSet<&str> = report
                .tab(other)
                .into_iter()
                .map(CoverageEntry::evidence)
                .collect();
            let shared: Vec<&&str> = left.intersection(&right).collect();
            assert!(
                shared.is_empty(),
                "{} and {} share {shared:?}",
                tab.spec_word(),
                other.spec_word()
            );
        }
    }

    // 3. One subject on three tabs and not the fourth.
    let on: Vec<CoverageTab> = CoverageTab::ALL
        .into_iter()
        .filter(|tab| {
            report
                .tab(*tab)
                .iter()
                .any(|entry| entry.subject() == subject)
        })
        .collect();
    assert_eq!(
        on,
        vec![
            CoverageTab::Designed,
            CoverageTab::Taught,
            CoverageTab::Practiced
        ],
        "the shared subject is not on exactly the three tabs its evidence names"
    );

    // A predicate no tab answers for is refused.
    assert_eq!(
        CoverageEntry::of(PredicateName::AppliedIn, subject, "elsewhere").err(),
        Some(DashboardError::PredicateIsNotACoverageTab("APPLIED_IN"))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// catalog_and_review_are_separate_sections
// ---------------------------------------------------------------------------

/// A catalogue fact and an offering review are two blocks, and neither holds
/// the other's vocabulary.
///
/// > Course catalog 정보와 특정 Offering review를 같은 속성처럼 보이지 않게 한다.
///
/// Five halves.
///
/// 1. **Section 25.6's six headings** are read out of its fenced block and
///    compared with `CourseSection::ALL` in order and as sets.
/// 2. **Both vocabularies are in this crate's closure.** `academic-curriculum`
///    and `academic-review` are both product edges, so keeping them apart is a
///    choice this surface makes rather than something the compiler made for it.
/// 3. **`CatalogIdentity` declares no review type.** Its whole field list is
///    read out of this crate's own source and compared with the six things
///    section 25.6's `Official identity` line names, in both directions, and
///    every review-dimension and aggregate spelling is required to be absent
///    from it.
/// 4. **`ReviewSection` names no course.** `ReviewScope` has no `CourseId`
///    field, no constructor taking one and no accessor returning one — read out
///    of `P2-U8`'s own source, the way that crate's
///    `scalar_is_not_a_course_property` reads `academic-curriculum`'s `Course`.
/// 5. **And nothing here reduces a distribution.** `P2-U8` drew that line; this
///    checks it has not been redrawn on this side, by requiring the aggregate a
///    review section carries to keep every dimension's distribution and by
///    requiring this crate's source to declare no conversion out of a band.
#[test]
fn catalog_and_review_are_separate_sections() -> TestResult {
    let text = spec()?;
    let block = section(&text, "25.6 Course Detail")?;
    assert!(
        block.contains("같은 속성처럼 보이지 않게 한다"),
        "section 25.6 no longer separates a catalogue fact from an offering review"
    );
    let fenced = fenced_text(&block)?;
    let headings: Vec<&str> = fenced
        .lines()
        .filter(|line| {
            !line.trim().is_empty()
                && !line.contains('·')
                && !line.contains('/')
                && !line.starts_with(' ')
        })
        .collect();
    let declared: Vec<&str> = CourseSection::ALL
        .into_iter()
        .map(CourseSection::spec_heading)
        .collect();
    assert_eq!(
        headings, declared,
        "section 25.6's blocks and CourseSection::ALL have diverged"
    );

    // 2. Both vocabularies are here.
    let manifest = support::repository_file("crates/dashboard/Cargo.toml")?;
    let (product, _) = manifest
        .split_once("[dev-dependencies]")
        .ok_or("the manifest has no dev-dependency section")?;
    for edge in ["academic-curriculum", "academic-review"] {
        assert!(
            product.contains(edge),
            "{edge} is not a product edge, so the separation below is not a choice"
        );
    }

    let course = fixtures::course(1, "4190.101")?;
    let revision =
        fixtures::revision(1, &course, "4190.101", 3, CurriculumCategory::MajorRequired)?;
    let offering = fixtures::offering(1, &revision, "T20271")?;
    let identity = CatalogIdentity::of(&course, &revision)?;

    let reviews = vec![
        fixtures::review(
            41,
            fixtures::scope(1, "Kim", "T20271")?,
            "the assignments were long and the lectures were slow",
            DimensionBand::High,
        )?,
        fixtures::review(
            42,
            fixtures::scope(1, "Kim", "T20271")?,
            "an even term with nothing surprising in it at all",
            DimensionBand::VeryLow,
        )?,
    ];
    let aggregate = OfferingAggregate::over(&reviews, fixtures::disclosure(2)?)?;
    let section_of_reviews = ReviewSection::scoped(fixtures::scope(1, "Kim", "T20271")?, aggregate);

    let detail = CourseDetail::assemble(
        identity,
        vec![OfferingRow::of(&offering)],
        CoverageReport::over(vec![CoverageEntry::of(
            PredicateName::DesignedToTeach,
            fixtures::entity_id(9)?,
            "syllabus/1",
        )?]),
        Vec::<TimelineEntry>::new(),
        Connections::linking(
            vec![CourseCode::parse("4190.100")?],
            vec![CourseCode::parse("4190.408")?],
            vec![fixtures::entity_id(10)?],
            vec![fixtures::entity_id(11)?],
            vec![fixtures::entity_id(12)?],
        ),
        vec![section_of_reviews],
    )?;

    // Every block holds rows, so the section comparison below is not over a
    // detail that happens to be empty.
    for block_name in CourseSection::ALL {
        if block_name == CourseSection::MyRecord {
            continue;
        }
        assert!(
            detail.rows_in(block_name) > 0,
            "{} holds nothing",
            block_name.spec_heading()
        );
    }

    // 3. `CatalogIdentity`'s whole field list, read from this crate's source.
    let module = support::repository_file("crates/dashboard/src/course.rs")?;
    let body = module
        .split_once("pub struct CatalogIdentity {")
        .ok_or("academic-dashboard no longer declares `pub struct CatalogIdentity`")?
        .1
        .split_once('}')
        .ok_or("the CatalogIdentity declaration is unterminated")?
        .0;
    let fields: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.split(':').next())
        .collect();
    assert_eq!(
        fields,
        vec![
            "course",
            "revision",
            "code",
            "credits",
            "category",
            "source_snapshot",
            "valid_time"
        ],
        "the catalogue identity's fields are not section 25.6's own line"
    );
    for spelling in ReviewDimension::ALL
        .into_iter()
        .map(ReviewDimension::as_str)
        .chain(["OfferingAggregate", "CourseAggregate", "DimensionBand"])
    {
        assert!(
            !body.contains(spelling),
            "the catalogue identity holds {spelling}, which is a review reading"
        );
    }

    // 4. `ReviewScope` names no course, read from `P2-U8`'s own source.
    let scope_module = support::repository_file("crates/review/src/scope.rs")?;
    let scope_body = scope_module
        .split_once("pub struct ReviewScope {")
        .ok_or("academic-review no longer declares `pub struct ReviewScope`")?
        .1
        .split_once('}')
        .ok_or("the ReviewScope declaration is unterminated")?
        .0;
    assert!(
        !scope_body.contains("CourseId"),
        "a review scope now names a course, so section 34's confusion row is open again"
    );
    assert!(
        detail
            .reviews()
            .iter()
            .all(|entry| entry.scope().offering().is_some()),
        "a review section is not scoped to an offering"
    );

    // 5. Nothing here reduces a distribution, and the distributions survive.
    for entry in detail.reviews() {
        for dimension in ReviewDimension::ALL {
            assert_eq!(
                entry.aggregate().distribution(dimension).total(),
                2,
                "{} lost a reading on the way to the section",
                dimension.as_str()
            );
        }
        assert_eq!(entry.disclosure().disclosed().len(), 6);
    }
    let low = detail
        .reviews()
        .first()
        .ok_or("no review section")?
        .aggregate()
        .distribution(ReviewDimension::Difficulty)
        .counts();
    assert!(
        low.iter().filter(|count| **count > 0).count() > 1,
        "the two reviews collapsed into one band, so the distribution says nothing"
    );
    let crate_source = support::repository_file("crates/dashboard/src/course.rs")?;
    for reduction in ["impl From<DimensionBand>", "fn score", "fn mean", "as f64"] {
        assert!(
            !crate_source.contains(reduction),
            "this crate declares {reduction}, which reduces a distribution to a value"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// percentage_is_secondary_with_breakdown
// ---------------------------------------------------------------------------

/// The bar exists only over its own parts, and never first.
///
/// > "졸업 72%"는 보조 시각화일 수 있으나 서로 대체 불가능한 requirement를 한
/// > 막대로 오해시키지 않도록 상세 breakdown이 항상 붙는다.
///
/// The sentence is read out of the document, then:
///
/// 1. **The breakdown is always there.** Every percentage this test can build
///    carries the parts it was computed from, and the number is a function of
///    them — two breakdowns with the same totals give the same bar, and one
///    part changed changes it.
/// 2. **A percentage with no breakdown is unrepresentable.**
///    `tests/compile_fail/a_percentage_is_not_built_from_a_number.rs` is the
///    compiled half; here the runtime refusals are driven, one per rule.
/// 3. **It is not on the screen's six lines.** The percentage is reached
///    through its own accessor, and the six sections are section 25.4's own six
///    lines with no seventh, so it cannot be first.
/// 4. **An unevaluated part stops the bar rather than being counted as zero.**
///    That is the same refusal `academic_record::views::GpaValue::Unknown`
///    makes one surface over, and it is driven for both `UNKNOWN` and
///    `CONFLICT`.
#[test]
fn percentage_is_secondary_with_breakdown() -> TestResult {
    let text = spec()?;
    let block = section(&text, "25.4 Academic Dashboard")?;
    let sentence = block
        .lines()
        .find(|line| line.contains("보조 시각화"))
        .ok_or("section 25.4 no longer calls the percentage secondary")?;
    assert!(
        sentence.contains("상세 breakdown이 항상 붙는다"),
        "section 25.4 no longer requires the breakdown to be attached"
    );
    assert!(
        sentence.contains("서로 대체 불가능한 requirement를 한 막대로"),
        "section 25.4 no longer warns about one bar over non-substitutable requirements"
    );

    let settled = AuditStateReading::of(ProofStatus::Needs);
    let met = AuditStateReading::of(ProofStatus::Satisfied);
    let parts = vec![
        BreakdownPart::of("총 취득학점", 93, 130, settled)?,
        BreakdownPart::of("전공 학점", 51, 63, settled)?,
        BreakdownPart::of("전공 필수", 12, 12, met)?,
    ];
    let breakdown = RequirementBreakdown::assemble(parts.clone())?;
    let bar = SecondaryPercentage::over(breakdown)?;

    // 1. The number is a function of the parts.
    assert_eq!(bar.breakdown().parts().len(), parts.len());
    let counted: u32 = parts.iter().map(BreakdownPart::counted).sum();
    let required: u32 = parts.iter().map(BreakdownPart::required).sum();
    assert_eq!(
        bar.permille(),
        u32::try_from(u64::from(counted) * 1000 / u64::from(required))?,
        "the bar is not the ratio of its own parts"
    );
    let moved = vec![
        BreakdownPart::of("총 취득학점", 100, 130, settled)?,
        BreakdownPart::of("전공 학점", 51, 63, settled)?,
        BreakdownPart::of("전공 필수", 12, 12, met)?,
    ];
    let moved_bar = SecondaryPercentage::over(RequirementBreakdown::assemble(moved)?)?;
    assert_ne!(
        bar.permille(),
        moved_bar.permille(),
        "one part changed and the bar did not"
    );
    // And every part is still reachable beside the number.
    for part in bar.breakdown().parts() {
        assert!(!part.label().is_empty());
        assert!(part.required() > 0);
        assert_eq!(
            part.reading().state(),
            AuditState::of(part.reading().engine_status())
        );
    }

    // 2. The runtime refusals, one per rule.
    assert_eq!(
        RequirementBreakdown::assemble(Vec::new()).err(),
        Some(DashboardError::PercentageWithoutBreakdown)
    );
    assert_eq!(
        RequirementBreakdown::assemble(vec![
            BreakdownPart::of("전공 학점", 51, 63, settled)?,
            BreakdownPart::of("전공 학점", 20, 30, settled)?,
        ])
        .err(),
        Some(DashboardError::BreakdownRepeatsARequirement {
            label: "전공 학점".to_owned()
        })
    );
    assert_eq!(
        BreakdownPart::of("무엇도 요구하지 않음", 0, 0, settled).err(),
        Some(DashboardError::BreakdownPartRequiresNothing {
            label: "무엇도 요구하지 않음".to_owned()
        })
    );
    assert_eq!(
        BreakdownPart::of("초과", 14, 12, settled).err(),
        Some(DashboardError::BreakdownPartOverflows {
            label: "초과".to_owned()
        })
    );
    assert_eq!(
        BreakdownPart::of("   ", 1, 2, settled).err(),
        Some(DashboardError::EmptyField("requirement label"))
    );

    // 4. An unevaluated part stops the bar, for both reasons.
    for status in [ProofStatus::Unknown, ProofStatus::Conflict] {
        let unsettled = RequirementBreakdown::assemble(vec![
            BreakdownPart::of("총 취득학점", 93, 130, settled)?,
            BreakdownPart::of("경과조치", 0, 3, AuditStateReading::of(status))?,
        ])?;
        assert_eq!(unsettled.unsettled().len(), 1);
        assert_eq!(
            SecondaryPercentage::over(unsettled).err(),
            Some(DashboardError::PercentageOverAnUnsettledPart { count: 1 }),
            "a bar was drawn over a part reading {}",
            status.as_str()
        );
    }

    // 3. And it is not one of the six lines.
    let history = corpus::baseline_history()?;
    let rules = corpus::baseline_rules()?;
    let classification = corpus::classification_v1()?;
    let views = RecordViews::compute(&history, &rules, &classification)?;
    let figures = vec![GpaFigure::publish(
        GpaScope::Cumulative,
        views.cumulative_gpa()?,
        GpaProof::recording(
            views.cumulative_included(),
            dispositions_of(&views),
            views.repeat_proofs().to_vec(),
        ),
    )?];
    let with_bar = AcademicDashboard::assemble(
        filled_sections(figures.clone(), &history)?,
        &[],
        Some(SecondaryPercentage::over(RequirementBreakdown::assemble(
            parts,
        )?)?),
    )?;
    assert_eq!(with_bar.sections().len(), DashboardLine::ALL.len());
    for region in with_bar.sections() {
        assert!(
            region.line().is_some(),
            "a section of the screen is not one of section 25.4's lines"
        );
    }
    let secondary = with_bar
        .secondary_percentage()
        .ok_or("the percentage was not carried")?;
    assert_eq!(secondary.breakdown().parts().len(), 3);
    let without = AcademicDashboard::assemble(filled_sections(figures, &history)?, &[], None)?;
    assert!(
        without.secondary_percentage().is_none(),
        "a screen with no percentage produced one"
    );
    assert_eq!(
        without.sections().len(),
        with_bar.sections().len(),
        "the percentage changed how many sections the screen has"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The section 38 cells
// ---------------------------------------------------------------------------

/// `GATE-38-001`–`GATE-38-006` block, `GATE-38-017` stays open.
///
/// Every identifier is derived from section 38's own numbering — section 38.1's
/// ten lines are `GATE-38-001` to `GATE-38-010` and section 38.2's eleven
/// bullets continue from `GATE-38-011` — rather than read from a table here, so
/// a renumbered section or a reordered line fails. `academic-audit` and
/// `academic-offering` derive their own the same way, and the five cells this
/// crate shares with the first are compared against that crate's answer rather
/// than made equal to it.
///
/// The cell `academic-audit` does **not** hold is the point of the comparison:
/// `GATE-38-005`, the official transcript. Section 25.4's averages are over the
/// imported record, so the import blocks this screen; the graduation engine is
/// handed an attempt set and never reads a transcript, so it does not block
/// there. A forwarded enumeration would have carried that hole onto this
/// surface.
#[test]
fn the_open_gates_are_section_38s_own() -> TestResult {
    let text = spec()?;
    let profile_block = section(&text, "38.1 사용자에게서 필요한 정보")?;
    let fenced = fenced_text(&profile_block)?;
    let lines: Vec<&str> = fenced
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split('=')
                .next()
                .unwrap_or(line)
                .split('#')
                .next()
                .unwrap_or(line)
                .trim()
        })
        .collect();
    assert!(
        lines.len() >= 6,
        "section 38.1 lists only {} lines",
        lines.len()
    );
    let official_block = section(&text, "38.2 공식적으로 추가 확인할 항목")?;
    let official = bullets(&official_block);
    assert!(
        official.len() >= 7,
        "section 38.2 lists only {} bullets",
        official.len()
    );

    for gate in OpenGate::ALL {
        let (position, expected) = if gate.blocks_the_dashboard() {
            let at = lines
                .iter()
                .position(|line| *line == gate.spec_line())
                .ok_or_else(|| format!("{} is not a line of section 38.1", gate.identifier()))?;
            (at + 1, gate.spec_line())
        } else {
            let at = official
                .iter()
                .position(|line| line == gate.spec_line())
                .ok_or_else(|| format!("{} is not a bullet of section 38.2", gate.identifier()))?;
            // Section 38.1's ten come first.
            (at + 1 + lines.len().max(10), gate.spec_line())
        };
        assert_eq!(
            gate.identifier(),
            format!("GATE-38-{position:03}"),
            "{expected} is section 38's item {position} and is identified otherwise"
        );
        assert!(
            !gate.statement().is_empty(),
            "{} says nothing about what is missing",
            gate.identifier()
        );
    }

    // The six that block are section 38.1's first six, in order.
    let blocking: Vec<&str> = OpenGate::BLOCKING
        .into_iter()
        .map(OpenGate::spec_line)
        .collect();
    assert_eq!(
        blocking,
        lines
            .iter()
            .take(OpenGate::BLOCKING.len())
            .copied()
            .collect::<Vec<_>>(),
        "the blocking cells are not section 38.1's first six"
    );
    assert!(
        OpenGate::ALL
            .into_iter()
            .filter(|gate| gate.blocks_the_dashboard())
            .count()
            == OpenGate::BLOCKING.len(),
        "BLOCKING and blocks_the_dashboard disagree"
    );

    // Five of the six are `P2-U3`'s, and one is not — measured, not asserted.
    let audit: BTreeSet<&str> = academic_audit::OpenGate::ALL
        .into_iter()
        .map(academic_audit::OpenGate::identifier)
        .collect();
    let mine: BTreeSet<&str> = OpenGate::BLOCKING
        .into_iter()
        .map(OpenGate::identifier)
        .collect();
    let shared: Vec<&&str> = mine.intersection(&audit).collect();
    assert_eq!(shared.len(), 5, "the shared cells moved");
    let only_here: Vec<&&str> = mine.difference(&audit).collect();
    assert_eq!(
        only_here,
        vec![&"GATE-38-005"],
        "the cell this surface adds is not the official transcript"
    );
    for gate in OpenGate::BLOCKING {
        if gate.identifier() == "GATE-38-005" {
            continue;
        }
        let matched = academic_audit::OpenGate::ALL
            .into_iter()
            .find(|other| other.identifier() == gate.identifier())
            .ok_or_else(|| format!("{} is not academic-audit's", gate.identifier()))?;
        assert_eq!(
            matched.spec_line(),
            gate.spec_line(),
            "two crates read {} out of two different lines",
            gate.identifier()
        );
    }

    // And `GATE-38-017` is `P2-U5`'s, still open, still not blocking.
    let offering_gate = academic_offering::OpenGate::CurrentTermOfferingFacts;
    assert_eq!(offering_gate.identifier(), "GATE-38-017");
    assert_eq!(
        offering_gate.spec_line(),
        OpenGate::CurrentTermOfferingFacts.spec_line(),
        "two crates read GATE-38-017 out of two different bullets"
    );
    assert!(!OpenGate::CurrentTermOfferingFacts.blocks_the_dashboard());
    Ok(())
}

/// An open cell shows the exact missing check instead of a number.
#[test]
fn an_open_cell_blocks_the_line_it_reaches() -> TestResult {
    let history = corpus::baseline_history()?;
    let rules = corpus::baseline_rules()?;
    let classification = corpus::classification_v1()?;
    let views = RecordViews::compute(&history, &rules, &classification)?;
    let figures = vec![GpaFigure::publish(
        GpaScope::Cumulative,
        views.cumulative_gpa()?,
        GpaProof::recording(
            views.cumulative_included(),
            dispositions_of(&views),
            views.repeat_proofs().to_vec(),
        ),
    )?];

    // Every blocking cell reaches at least one line, and blocks exactly the
    // lines that name it.
    for gate in OpenGate::BLOCKING {
        let screen = AcademicDashboard::assemble(
            filled_sections(figures.clone(), &history)?,
            &[gate],
            None,
        )?;
        let mut blocked = 0_usize;
        for line in DashboardLine::ALL {
            let expected = line.blocked_by().contains(&gate);
            let region = screen.section(line);
            let is_blocked = matches!(region, DashboardSection::Blocked(_));
            assert_eq!(
                is_blocked,
                expected,
                "{} {} {line:?}",
                gate.identifier(),
                if expected { "did not block" } else { "blocked" }
            );
            if is_blocked {
                assert_eq!(region, &DashboardSection::Blocked(gate));
                blocked += 1;
            }
        }
        assert!(
            blocked > 0,
            "{} blocks nothing, so surfacing it says nothing",
            gate.identifier()
        );
    }

    // `GATE-38-017` is not a blocking input and does not become one.
    let open = AcademicDashboard::assemble(
        filled_sections(figures.clone(), &history)?,
        &[OpenGate::CurrentTermOfferingFacts],
        None,
    )?;
    for line in DashboardLine::ALL {
        assert!(
            !matches!(open.section(line), DashboardSection::Blocked(_)),
            "the per-term offering cell blocked {line:?}"
        );
    }

    // And with nothing open, nothing is blocked.
    let clear = AcademicDashboard::assemble(filled_sections(figures, &history)?, &[], None)?;
    for line in DashboardLine::ALL {
        assert!(
            !matches!(clear.section(line), DashboardSection::Blocked(_)),
            "{line:?} is blocked with no open cell"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Every attempt's disposition reason, as the proof carries them.
fn dispositions_of(
    views: &RecordViews,
) -> Vec<(
    academic_domain::AttemptId,
    academic_record::views::DispositionReason,
)> {
    views
        .dispositions()
        .iter()
        .map(|disposition| (disposition.attempt_id(), disposition.reason()))
        .collect()
}

/// Whether one attempt reached the grade-point denominator.
fn is_included(disposition: &academic_record::views::AttemptDisposition) -> bool {
    matches!(
        disposition.average(),
        academic_record::views::AverageContribution::Included { .. }
    )
}

/// The attempts one term's average included.
fn included_in_term(views: &RecordViews, term: TermKey) -> Vec<academic_domain::AttemptId> {
    views
        .dispositions()
        .iter()
        .filter(|disposition| disposition.term() == term)
        .filter(|disposition| is_included(disposition))
        .map(academic_record::views::AttemptDisposition::attempt_id)
        .collect()
}

/// The attempts one programme's major average included.
fn included_in_major(views: &RecordViews, program: &ProgramId) -> Vec<academic_domain::AttemptId> {
    views
        .dispositions()
        .iter()
        .filter(|disposition| is_included(disposition))
        .filter(|disposition| {
            views
                .categories(disposition.attempt_id())
                .and_then(|categories| categories.get(program))
                .copied()
                .is_some_and(academic_record::classify::RequirementCategory::is_major)
        })
        .map(academic_record::views::AttemptDisposition::attempt_id)
        .collect()
}

/// The six sections, filled from one attempt set.
fn filled_sections(
    figures: Vec<GpaFigure>,
    history: &academic_record::attempt::AttemptHistory,
) -> Result<[DashboardSection; DashboardLine::ALL.len()], Box<dyn std::error::Error>> {
    Ok([
        DashboardSection::Averages(figures),
        DashboardSection::CreditsByCategory(vec![("MAJOR_REQUIRED".to_owned(), 12)]),
        DashboardSection::AppliedProfile(vec![("admission year".to_owned(), "2014".to_owned())]),
        DashboardSection::AuditStates(vec![(
            "rules.total_credits".to_owned(),
            AuditStateReading::of(ProofStatus::Needs),
        )]),
        DashboardSection::AttemptTimeline(AttemptTimeline::of(history, &[])),
        DashboardSection::SourceFreshness(vec![("snu.catalogue".to_owned(), 3)]),
    ])
}
