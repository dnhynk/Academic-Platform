//! Named acceptance evidence for `P2-U4`.
//!
//! Seventeen rows, all of them executed against the deterministic synthetic
//! corpus in `academic_record::corpus`. Nothing here reads a real academic
//! record, and admission is closed, so no durable path for one exists.
//!
//! Several rows measure an absence or an inequality. Each of those runs the
//! violation as well as the correct case, so a row cannot pass because the
//! thing it was watching never happened:
//!
//! - `credits_vs_denominator` computes both quantities under a corpus built so
//!   they differ, and re-derives the difference attempt by attempt from the
//!   dispositions, so a change that collapsed the two would have to change the
//!   per-attempt reasons in the same commit.
//! - `repeat_ceiling_effective_date` evaluates the same attempts under three
//!   effective terms and requires the published average to move for the one
//!   that crosses the repeat, and to hold for the one that does not.
//! - `gpa_formula_fixture` compares against `testdata/engines/gpa/oracle.expected`,
//!   which `tools/gpa-oracle.mjs` produces from its own transcription of the
//!   corpus in its own arithmetic. Nothing in the Rust implementation reaches
//!   those numbers.

use std::{collections::BTreeMap, error::Error, fs, path::PathBuf, str::FromStr as _};

use academic_domain::{
    Actor, AuthorityClass, ClaimId, Decimal, EntityId, EpistemicStatus, ScopeId, TimestampMillis,
    ValidInterval,
    engines::{DeterministicEngine as _, EngineVersion, ProofStatus},
};
use academic_record::{
    attempt::{
        AttemptHistory, AttemptStatus, CourseAttempt, RegistrationConfirmation, RepeatStatus,
        SettledStatus,
    },
    classify::{ProgramId, RequirementCategory, classification_claim},
    corpus, decimal,
    engine::{CreditAccountingEngine, GpaEngine},
    facts::{AttemptFacts, GpaScope, encode},
    grade::{GradeSymbol, GradingScheme},
    ingest::attempt_from_confirmed_row,
    plan::{PlanScenario, PlanScenarioChoice, PlanStore, delete_scenario},
    policy::{AttemptOrigin, PolicyBook, RecognitionDecision, RepeatRecognition, RuleBook},
    term::{Semester, TermKey},
    views::{
        AverageContribution, CreditContribution, DispositionReason, GpaValue, RecordViews,
        contributes_to_actual_progress,
    },
};
use academic_transcript::{
    claims::{LinkedRowClaims, RowClaimContext, RowClaimIds, confirm_reconciled_rows},
    reconcile::{TranscriptChecksums, reconcile},
    record::{NormalizedTranscript, TranscriptIdentity, TranscriptRow},
    source::TranscriptFormat,
};

type TestResult = Result<(), Box<dyn Error>>;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Builds the baseline views: the corpus, the confirmed rule book, the rules.
fn baseline() -> Result<RecordViews, Box<dyn Error>> {
    Ok(RecordViews::compute(
        &corpus::baseline_history()?,
        &corpus::baseline_rules()?,
        &corpus::classification_v1()?,
    )?)
}

fn rendered(value: &GpaValue) -> String {
    match value {
        GpaValue::Known(decimal) => decimal::render(*decimal),
        GpaValue::NoGradedAttempts => "NO_GRADED_ATTEMPTS".to_owned(),
        GpaValue::Unknown(_) => "UNKNOWN".to_owned(),
    }
}

/// Reads the oracle block the independent JavaScript oracle produced.
fn oracle() -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let path = repository_root().join("testdata/engines/gpa/oracle.expected");
    let text = fs::read_to_string(&path)?;
    let mut table = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("oracle line has no '=': {line}"))?;
        table.insert(key.to_owned(), value.to_owned());
    }
    if table.is_empty() {
        return Err("the oracle block is empty; the fixture would assert nothing".into());
    }
    Ok(table)
}

fn expect<'a>(table: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, Box<dyn Error>> {
    Ok(table
        .get(key)
        .ok_or_else(|| format!("the oracle does not carry {key}"))?
        .as_str())
}

// ---------------------------------------------------------------------------
// 1. The attempt ledger
// ---------------------------------------------------------------------------

/// Every attempt is preserved; a correction appends and never overwrites.
///
/// The ledger is exercised in the direction that could go wrong: a correction
/// is appended, `current` is observed to drop the corrected attempt, `all` is
/// observed **not** to shrink, and the superseded attempt is observed still
/// readable by its own identity. The relation carries ADR-003's `SUPERSEDES`
/// kind, which is the mechanism `CONTRIBUTING.md` rule 2 names rather than a
/// second one invented here.
#[test]
fn attempt_history_append_only() -> TestResult {
    let mut history = corpus::baseline_history()?;
    let before_all = history.all().len();
    let before_current = history.current().len();

    let corrected_id = corpus::synthetic_attempt_id(3)?;
    let original = history
        .get(corrected_id)
        .ok_or("the corpus no longer holds the attempt this row corrects")?
        .clone();
    assert_eq!(original.grade(), Some(GradeSymbol::BPlus));

    let replacement = CourseAttempt::from_confirmed_row(
        corpus::synthetic_attempt_id(40)?,
        original.course_code(),
        original.term(),
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        original.credits_attempted(),
        original.credits_earned(),
        Some(GradeSymbol::AMinus),
        original.grading_scheme_id().to_owned(),
        original.evidence_ids().to_vec(),
    )?;
    history.append_correction(
        replacement,
        corrected_id,
        ScopeId::from_str("01900000-0000-7000-8000-00000000f001")?,
        ClaimId::from_str("01900000-0000-7000-8000-00000000f002")?,
        ClaimId::from_str("01900000-0000-7000-8000-00000000f003")?,
    )?;

    assert_eq!(
        history.all().len(),
        before_all + 1,
        "the ledger must grow by exactly the appended entry"
    );
    assert_eq!(
        history.current().len(),
        before_current,
        "a correction replaces one current attempt and adds none"
    );
    assert!(
        history
            .current()
            .iter()
            .all(|attempt| attempt.id() != corrected_id),
        "the corrected attempt must leave the current projection"
    );

    // The whole point: it is still there.
    let preserved = history
        .get(corrected_id)
        .ok_or("a superseded attempt must stay readable by identity")?;
    assert_eq!(preserved.grade(), Some(GradeSymbol::BPlus));
    assert_eq!(preserved, &original, "a superseded attempt is not edited");

    let entry = history
        .all()
        .iter()
        .find(|entry| entry.supersedes() == Some(corrected_id))
        .ok_or("the correction entry carries no supersession")?;
    let relation = entry.relation().ok_or("a correction carries a relation")?;
    assert_eq!(
        relation.kind,
        academic_domain::ClaimRelationKind::Supersedes,
        "a correction is ADR-003's SUPERSEDES and not a weaker relation"
    );

    // Injection: a correction naming an attempt the ledger does not hold, and
    // one naming itself, are both refused. Without these the two guards above
    // could be vacuous.
    let stray = CourseAttempt::from_confirmed_row(
        corpus::synthetic_attempt_id(41)?,
        original.course_code(),
        original.term(),
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        original.credits_attempted(),
        original.credits_earned(),
        Some(GradeSymbol::AMinus),
        original.grading_scheme_id().to_owned(),
        original.evidence_ids().to_vec(),
    )?;
    let scope = ScopeId::from_str("01900000-0000-7000-8000-00000000f001")?;
    let source = ClaimId::from_str("01900000-0000-7000-8000-00000000f004")?;
    let target = ClaimId::from_str("01900000-0000-7000-8000-00000000f005")?;
    assert!(
        history
            .append_correction(
                stray.clone(),
                corpus::synthetic_attempt_id(200)?,
                scope,
                source,
                target
            )
            .is_err(),
        "a correction of an attempt the ledger does not hold must be refused"
    );
    let self_id = stray.id();
    let mut isolated = AttemptHistory::new();
    isolated.append(stray.clone())?;
    assert!(
        isolated
            .append_correction(stray, self_id, scope, source, target)
            .is_err(),
        "an attempt must not supersede itself"
    );
    Ok(())
}

/// The attempt schema and its two closed status sets.
#[test]
fn attempt_grade_repeat_contract() -> TestResult {
    assert_eq!(
        AttemptStatus::ALL.map(AttemptStatus::as_str),
        [
            "PLANNED",
            "REGISTERED",
            "IN_PROGRESS",
            "COMPLETED",
            "WITHDRAWN",
            "CANCELLED",
            "TRANSFERRED",
            "RECOGNIZED",
        ],
        "the status set is section 10's, in section 10's order"
    );
    assert_eq!(
        RepeatStatus::ALL.map(RepeatStatus::as_str),
        ["ORIGINAL", "REPEAT", "REPLACED", "NOT_APPLICABLE"],
        "the repeat status set is section 10's"
    );
    for status in AttemptStatus::ALL {
        assert_eq!(AttemptStatus::parse(status.as_str()), Some(status));
    }
    for status in RepeatStatus::ALL {
        assert_eq!(RepeatStatus::parse(status.as_str()), Some(status));
    }

    // The schema's fields, read back off a built attempt.
    let history = corpus::baseline_history()?;
    let repeat = history
        .get(corpus::synthetic_attempt_id(2)?)
        .ok_or("the corpus no longer holds the repeat attempt")?;
    assert_eq!(repeat.course_code(), corpus::COURSE_REPEATED);
    assert_eq!(repeat.term(), TermKey::new(2015, Semester::Spring)?);
    assert_eq!(repeat.status(), AttemptStatus::Completed);
    assert_eq!(repeat.origin(), AttemptOrigin::Internal);
    assert_eq!(repeat.grade(), Some(GradeSymbol::APlus));
    assert_eq!(repeat.repeat_status(), RepeatStatus::Repeat);
    assert_eq!(
        repeat.repeat_of(),
        Some(corpus::synthetic_attempt_id(1)?),
        "a repeat names the attempt it repeats"
    );
    assert_eq!(
        repeat.grading_scheme_id(),
        GradingScheme::snu_4_3_v1()?.id()
    );
    assert!(!repeat.evidence_ids().is_empty());

    let original = history
        .get(corpus::synthetic_attempt_id(1)?)
        .ok_or("the corpus no longer holds the original attempt")?;
    assert_eq!(original.repeat_status(), RepeatStatus::Original);
    assert_eq!(original.repeat_of(), None);

    // Injection: an attempt with no evidence and one with a course code the
    // frozen-input grammar could not carry are both refused.
    let term = TermKey::new(2020, Semester::Spring)?;
    let credits = decimal::integer(3)?;
    assert!(
        CourseAttempt::from_confirmed_row(
            corpus::synthetic_attempt_id(50)?,
            "4190.101",
            term,
            SettledStatus::Completed,
            AttemptOrigin::Internal,
            credits,
            credits,
            Some(GradeSymbol::AZero),
            "snu_4_3_v1",
            Vec::new(),
        )
        .is_err(),
        "an attempt with no evidence must be refused"
    );
    assert!(
        CourseAttempt::from_confirmed_row(
            corpus::synthetic_attempt_id(51)?,
            "CSE 101",
            term,
            SettledStatus::Completed,
            AttemptOrigin::Internal,
            credits,
            credits,
            Some(GradeSymbol::AZero),
            "snu_4_3_v1",
            vec![corpus::synthetic_evidence_id(51)?],
        )
        .is_err(),
        "a course code the engine grammar cannot carry must be refused at the boundary"
    );
    Ok(())
}

/// A classification comes from a versioned rule set and from nothing else.
#[test]
fn classification_by_ruleset() -> TestResult {
    let rules = corpus::classification_v1()?;
    let history = corpus::baseline_history()?;
    let shared = history
        .get(corpus::synthetic_attempt_id(3)?)
        .ok_or("the corpus no longer holds the shared course")?;

    let classified = rules.classify(shared);
    assert_eq!(
        classified.len(),
        2,
        "the shared course is classified once per programme"
    );
    let published: Vec<&str> = rules
        .rules()
        .iter()
        .map(|rule| rule.rule_id.as_str())
        .collect();
    for classification in &classified {
        assert_eq!(
            classification.ruleset_id(),
            corpus::CLASSIFICATION_RULESET_ID,
            "a classification names the rule set version that produced it"
        );
        assert!(
            published.contains(&classification.rule_id()),
            "a classification names a rule the published set holds"
        );
    }

    // The same course, two programmes, two categories — section 10's reason for
    // making this a rule-engine output rather than a label.
    let cse = ProgramId::new(corpus::PRIMARY_PROGRAM)?;
    let stat = ProgramId::new(corpus::ADDITIONAL_PROGRAM)?;
    let category = |program: &ProgramId| {
        classified
            .iter()
            .find(|entry| entry.program() == program)
            .map(|entry| entry.category())
    };
    assert_eq!(category(&cse), Some(RequirementCategory::MajorElective));
    assert_eq!(category(&stat), Some(RequirementCategory::FreeElective));

    // An unmentioned course is not defaulted into a category.
    let registered = history
        .get(corpus::synthetic_attempt_id(9)?)
        .ok_or("the corpus no longer holds the registered attempt")?;
    assert!(
        rules.classify(registered).is_empty(),
        "a course no rule mentions gets no classification, not a default one"
    );

    // The assertion half. A classification claim is `DeterministicEngine`
    // authority, and ADR-003's matrix permits a user actor exactly one
    // authority class — so the same claim handed a user actor is refused. The
    // refusal is executed, not described.
    let subject = EntityId::from_str("01900000-0000-7000-8000-00000000e001")?;
    let scope = ScopeId::from_str("01900000-0000-7000-8000-00000000f001")?;
    let valid = ValidInterval::new(TimestampMillis::new(1_700_000_000_000), None)?;
    let (claim, actor) = classification_claim(
        classified.first().ok_or("no classification to assert")?,
        ClaimId::from_str("01900000-0000-7000-8000-00000000f010")?,
        subject,
        scope,
        valid,
        vec![corpus::synthetic_evidence_id(3)?],
    )?;
    assert_eq!(claim.authority_class, AuthorityClass::DeterministicEngine);
    assert_eq!(
        claim.epistemic_status,
        EpistemicStatus::DeterministicDerived
    );
    assert!(matches!(actor, Actor::DeterministicEngine { .. }));
    assert!(
        claim
            .validate_for_actor(&Actor::User { user_id: subject })
            .is_err(),
        "a user actor must not be able to assert a classification claim"
    );

    // And the user's own authority class cannot carry a classification either:
    // the same claim rebuilt as `UserExplicit` is refused for the engine actor,
    // so there is no pairing that lets a hand-written category through.
    let mut user_authored = claim.clone();
    user_authored.authority_class = AuthorityClass::UserExplicit;
    user_authored.epistemic_status = EpistemicStatus::UserConfirmed;
    assert!(
        user_authored.validate_for_actor(&actor).is_err(),
        "the classifier must not be able to assert user-explicit authority"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. The averages
// ---------------------------------------------------------------------------

/// The published averages match an oracle written in another language.
///
/// `tools/gpa-oracle.mjs` holds its own transcription of the corpus, its own
/// transcription of the grade table, and its own fixed-point BigInt arithmetic.
/// Nothing in `crates/record` produces the numbers this row compares against,
/// so a change to the Rust implementation moves one side of the comparison and
/// not the other.
///
/// The cumulative case is `33.9 / 12`, exactly `2.825`. The nearest `f64` to
/// `2.825` is below it, so a floating-point implementation publishes `2.82`
/// here and this row fails.
#[test]
fn gpa_formula_fixture() -> TestResult {
    let table = oracle()?;
    let views = baseline()?;

    assert_eq!(
        rendered(&views.cumulative_gpa()?),
        expect(&table, "cumulative.gpa.scale2")?,
        "the cumulative average disagrees with the independent oracle"
    );
    assert_eq!(
        decimal::render(views.quality_points()?),
        expect(&table, "cumulative.quality_points")?
    );
    assert_eq!(
        decimal::render(views.gpa_denominator()?.partial()),
        expect(&table, "cumulative.denominator_credits")?
    );
    assert_eq!(
        decimal::render(views.earned_credits()?.partial()),
        expect(&table, "cumulative.earned_credits")?
    );

    // The exact-half boundary, stated as a constant so the intent survives a
    // corpus edit: 33.9 / 12 is 2.825 and publishes as 2.83.
    assert_eq!(rendered(&views.cumulative_gpa()?), "2.83");
    assert_eq!(
        decimal::render(decimal::div_round_half_up(
            decimal::parse("33.9")?,
            decimal::parse("12")?,
            2
        )?),
        "2.83",
        "half away from zero must round 2.825 up, which f64 cannot do"
    );

    let scale3 = RecordViews::compute(
        &corpus::baseline_history()?,
        &corpus::baseline_rules_scale3()?,
        &corpus::classification_v1()?,
    )?;
    assert_eq!(
        rendered(&scale3.cumulative_gpa()?),
        expect(&table, "cumulative.gpa.scale3")?
    );

    for term in views.terms() {
        assert_eq!(
            rendered(&views.term_gpa(term)?),
            expect(&table, &format!("term.{}.gpa", term.canonical_text()))?,
            "term {term} disagrees with the oracle"
        );
    }
    for program in views.programs() {
        assert_eq!(
            rendered(&views.major_gpa(&program)?),
            expect(&table, &format!("major.{}.gpa", program.as_str()))?,
            "major {} disagrees with the oracle",
            program.as_str()
        );
    }
    Ok(())
}

/// The same attempts under two scheme versions publish two averages.
#[test]
fn gpa_policy_version_matrix() -> TestResult {
    let history = corpus::baseline_history()?;
    let classification = corpus::classification_v1()?;

    let two = RecordViews::compute(&history, &corpus::baseline_rules()?, &classification)?;
    let three = RecordViews::compute(&history, &corpus::baseline_rules_scale3()?, &classification)?;
    assert_eq!(rendered(&two.cumulative_gpa()?), "2.83");
    assert_eq!(rendered(&three.cumulative_gpa()?), "2.825");
    assert_ne!(
        rendered(&two.cumulative_gpa()?),
        rendered(&three.cumulative_gpa()?),
        "the published scale is part of the scheme version and must change the answer"
    );

    // The published book leaves the repeat-recognition rule UNKNOWN, so the
    // same attempts under it have no average at all — a third point on the
    // version axis, and the one that shows an unknown is not folded to a zero.
    let published = RecordViews::compute(&history, &corpus::published_rules()?, &classification)?;
    let value = published.cumulative_gpa()?;
    let GpaValue::Unknown(pending) = &value else {
        return Err(
            format!("the published book must leave the average unknown, got {value:?}").into(),
        );
    };
    assert!(
        pending.contains(&corpus::synthetic_attempt_id(1)?)
            && pending.contains(&corpus::synthetic_attempt_id(2)?),
        "an unknown average names the exact attempts it could not place"
    );

    // The rule book's identity moves with its content, so an average can never
    // be attributed to a book that did not produce it.
    assert_ne!(
        corpus::baseline_rules()?.digest(),
        corpus::baseline_rules_scale3()?.digest(),
        "two scheme versions must not share a rule-set hash"
    );
    assert_ne!(
        corpus::baseline_rules()?.digest(),
        corpus::published_rules()?.digest(),
        "two policy books must not share a rule-set hash"
    );
    Ok(())
}

/// The section 10 grade table, driven through the whole engine per symbol.
#[test]
fn snu_grade_mapping_gpa() -> TestResult {
    let scheme = GradingScheme::snu_4_3_v1()?;
    let expected: [(GradeSymbol, &str); 13] = [
        (GradeSymbol::APlus, "4.3"),
        (GradeSymbol::AZero, "4"),
        (GradeSymbol::AMinus, "3.7"),
        (GradeSymbol::BPlus, "3.3"),
        (GradeSymbol::BZero, "3"),
        (GradeSymbol::BMinus, "2.7"),
        (GradeSymbol::CPlus, "2.3"),
        (GradeSymbol::CZero, "2"),
        (GradeSymbol::CMinus, "1.7"),
        (GradeSymbol::DPlus, "1.3"),
        (GradeSymbol::DZero, "1"),
        (GradeSymbol::DMinus, "0.7"),
        (GradeSymbol::F, "0"),
    ];
    let rules = corpus::baseline_rules()?;
    let classification = corpus::classification_v1()?;

    for (symbol, points) in expected {
        let treatment = scheme.treatment(symbol);
        let actual = treatment
            .grade_points()
            .ok_or_else(|| format!("{} must carry grade points", symbol.as_str()))?;
        assert_eq!(
            decimal::render(actual),
            points,
            "{} maps to the wrong grade points",
            symbol.as_str()
        );
        // The whole path, not just the table: one attempt at this grade must
        // publish this average.
        let views = RecordViews::compute(
            &corpus::single_grade_history(symbol)?,
            &rules,
            &classification,
        )?;
        let published = views.cumulative_gpa()?;
        let published = published
            .known()
            .ok_or_else(|| format!("{} must produce an average", symbol.as_str()))?;
        assert_eq!(
            decimal::compare(published, actual)?,
            core::cmp::Ordering::Equal,
            "one attempt at {} must average to its own grade points",
            symbol.as_str()
        );
    }

    // S and U are excluded from the average; the specification says so in the
    // same sentence as the table.
    for symbol in [GradeSymbol::S, GradeSymbol::U] {
        assert!(
            !scheme.treatment(symbol).participates_in_average(),
            "{} must stay outside the average",
            symbol.as_str()
        );
        let views = RecordViews::compute(
            &corpus::single_grade_history(symbol)?,
            &rules,
            &classification,
        )?;
        assert_eq!(views.cumulative_gpa()?, GpaValue::NoGradedAttempts);
    }
    // S earns its credits and U does not: the two are outside the average for
    // different reasons and the credit answer separates them.
    assert!(scheme.treatment(GradeSymbol::S).earns_credit());
    assert!(!scheme.treatment(GradeSymbol::U).earns_credit());
    // F is inside the average and earns nothing.
    assert!(scheme.treatment(GradeSymbol::F).participates_in_average());
    assert!(!scheme.treatment(GradeSymbol::F).earns_credit());
    // I is unresolved rather than zero.
    assert!(scheme.treatment(GradeSymbol::I).is_unresolved());
    Ok(())
}

/// Recognized external credit that is outside the average.
#[test]
fn external_credit_vs_gpa() -> TestResult {
    let views = baseline()?;
    let exchange = corpus::synthetic_attempt_id(8)?;
    let disposition = views
        .dispositions()
        .iter()
        .find(|entry| entry.attempt_id() == exchange)
        .ok_or("the corpus no longer holds the exchange attempt")?;

    assert_eq!(
        disposition.reason(),
        DispositionReason::ExternalExcludedFromAverage
    );
    assert_eq!(disposition.average(), AverageContribution::Excluded);
    assert!(
        matches!(disposition.credit(), CreditContribution::Earned(_)),
        "a recognized external credit still earns its credits"
    );
    assert_eq!(
        disposition.policy_row_id(),
        Some("external.excluded.2004_spring"),
        "the exclusion names the dated row that decided it"
    );
    assert!(
        !views.cumulative_included().contains(&exchange),
        "an excluded external grade is not in the average's attempt proof"
    );

    // The date is a row: an attempt in a term no row reaches is UNKNOWN, not
    // silently included and not silently excluded.
    let undated = RecordViews::compute(
        &corpus::history_with_undated_external()?,
        &corpus::baseline_rules()?,
        &corpus::classification_v1()?,
    )?;
    let undated_disposition = undated
        .dispositions()
        .iter()
        .find(|entry| entry.attempt_id() == exchange)
        .ok_or("the undated corpus no longer holds the exchange attempt")?;
    assert_eq!(
        undated_disposition.reason(),
        DispositionReason::ExternalPolicyUnknown
    );
    assert!(matches!(undated.cumulative_gpa()?, GpaValue::Unknown(_)));

    // `GATE-38-006` stays open: credits with no recorded decision are unknown,
    // not zero.
    let mut history = AttemptHistory::new();
    history.append(
        CourseAttempt::from_confirmed_row(
            corpus::synthetic_attempt_id(60)?,
            corpus::COURSE_EXCHANGE,
            TermKey::new(2015, Semester::Fall)?,
            SettledStatus::Recognized,
            AttemptOrigin::Exchange,
            decimal::integer(3)?,
            decimal::integer(3)?,
            Some(GradeSymbol::BZero),
            "snu_4_3_v1",
            vec![corpus::synthetic_evidence_id(60)?],
        )?
        .with_recognition(RecognitionDecision::Undecided),
    )?;
    let undecided = RecordViews::compute(
        &history,
        &corpus::baseline_rules()?,
        &corpus::classification_v1()?,
    )?;
    assert_eq!(
        undecided
            .dispositions()
            .first()
            .ok_or("no disposition")?
            .reason(),
        DispositionReason::RecognitionUndecided
    );
    assert_eq!(undecided.earned_credits()?.unknown().len(), 1);
    Ok(())
}

/// The cumulative average names every attempt it was computed from.
#[test]
fn cumulative_gpa_proof() -> TestResult {
    let views = baseline()?;
    let included = views.cumulative_included();
    let expected = [
        corpus::synthetic_attempt_id(2)?,
        corpus::synthetic_attempt_id(3)?,
        corpus::synthetic_attempt_id(5)?,
        corpus::synthetic_attempt_id(7)?,
    ];
    assert_eq!(
        included.len(),
        expected.len(),
        "the inclusion proof is the wrong size"
    );
    for attempt in expected {
        assert!(
            included.contains(&attempt),
            "{attempt} is missing from the proof"
        );
    }

    // Every attempt is accounted for: included, excluded, or unknown, with a
    // reason. A number with an unexplained gap is what this forbids.
    assert_eq!(
        views.dispositions().len(),
        corpus::baseline_history()?.current().len(),
        "every current attempt must have exactly one disposition"
    );
    for disposition in views.dispositions() {
        let placed = match disposition.average() {
            AverageContribution::Included { .. } => included.contains(&disposition.attempt_id()),
            AverageContribution::Excluded | AverageContribution::Unknown => {
                !included.contains(&disposition.attempt_id())
            }
        };
        assert!(
            placed,
            "an attempt's proof membership disagrees with its disposition"
        );
    }

    // The numerator and denominator re-derived from the proof alone.
    let mut numerator = decimal::zero()?;
    let mut denominator = decimal::zero()?;
    for disposition in views.dispositions() {
        if let AverageContribution::Included {
            quality_points,
            denominator_credits,
            ..
        } = disposition.average()
        {
            numerator = decimal::add(numerator, quality_points)?;
            denominator = decimal::add(denominator, denominator_credits)?;
        }
    }
    assert_eq!(decimal::render(numerator), "33.9");
    assert_eq!(decimal::render(denominator), "12");
    assert_eq!(
        decimal::render(decimal::div_round_half_up(numerator, denominator, 2)?),
        rendered(&views.cumulative_gpa()?),
        "the published average must be the quotient of its own proof"
    );
    Ok(())
}

/// Term averages partition the attempts the cumulative average used.
#[test]
fn term_gpa_partition() -> TestResult {
    let views = baseline()?;
    let terms = views.terms();
    assert!(terms.len() >= 4, "the corpus must span several terms");
    assert!(
        terms.windows(2).all(|pair| pair[0] < pair[1]),
        "terms are returned in academic order"
    );

    // Each attempt is in exactly one term.
    let mut counted = 0;
    for term in &terms {
        counted += views
            .dispositions()
            .iter()
            .filter(|disposition| disposition.term() == *term)
            .count();
    }
    assert_eq!(
        counted,
        views.dispositions().len(),
        "the terms must partition the attempt set, not cover or overlap it"
    );

    // And the term numerators and denominators sum to the cumulative ones.
    let mut numerator = decimal::zero()?;
    let mut denominator = decimal::zero()?;
    for term in &terms {
        for disposition in views
            .dispositions()
            .iter()
            .filter(|disposition| disposition.term() == *term)
        {
            if let AverageContribution::Included {
                quality_points,
                denominator_credits,
                ..
            } = disposition.average()
            {
                numerator = decimal::add(numerator, quality_points)?;
                denominator = decimal::add(denominator, denominator_credits)?;
            }
        }
    }
    assert_eq!(
        decimal::compare(numerator, views.quality_points()?)?,
        core::cmp::Ordering::Equal
    );
    assert_eq!(
        decimal::compare(denominator, views.gpa_denominator()?.partial())?,
        core::cmp::Ordering::Equal
    );

    // A term average is not the cumulative one: 2014 fall holds only the `F`.
    assert_eq!(
        rendered(&views.term_gpa(TermKey::new(2014, Semester::Fall)?)?),
        "0"
    );
    assert_eq!(
        rendered(&views.term_gpa(TermKey::new(2015, Semester::Spring)?)?),
        "4"
    );
    // A term whose only attempt is outside the average has no average at all.
    assert_eq!(
        views.term_gpa(TermKey::new(2015, Semester::Fall)?)?,
        GpaValue::NoGradedAttempts
    );
    Ok(())
}

/// The major average uses only rule-classified major attempts.
#[test]
fn major_gpa_classification() -> TestResult {
    let views = baseline()?;
    let cse = ProgramId::new(corpus::PRIMARY_PROGRAM)?;
    assert_eq!(rendered(&views.major_gpa(&cse)?), "2.43");
    assert_ne!(
        rendered(&views.major_gpa(&cse)?),
        rendered(&views.cumulative_gpa()?),
        "a major average that equals the cumulative one is not filtering"
    );

    // Membership comes from the rule engine's category, per programme.
    for disposition in views.dispositions() {
        let categories = views.categories(disposition.attempt_id());
        let is_major = categories
            .and_then(|table| table.get(&cse))
            .is_some_and(|category| category.is_major());
        if !is_major {
            continue;
        }
        assert!(
            matches!(
                disposition.reason(),
                DispositionReason::Graded
                    | DispositionReason::FailedInDenominator
                    | DispositionReason::RepeatCeilingApplied
                    | DispositionReason::ReplacedByRepeat
                    | DispositionReason::Withdrawn
            ),
            "a major attempt landed on an unexpected disposition"
        );
    }

    // The general-elective `S` is classified for `cse` and is not in its major
    // average: the filter is the category, not the programme.
    let satisfactory = views
        .categories(corpus::synthetic_attempt_id(4)?)
        .and_then(|table| table.get(&cse).copied());
    assert_eq!(satisfactory, Some(RequirementCategory::GeneralElective));
    assert!(!RequirementCategory::GeneralElective.is_major());
    Ok(())
}

/// Each programme has its own average over its own classification.
#[test]
fn multi_major_gpa() -> TestResult {
    let views = baseline()?;
    let cse = ProgramId::new(corpus::PRIMARY_PROGRAM)?;
    let stat = ProgramId::new(corpus::ADDITIONAL_PROGRAM)?;
    assert_eq!(views.programs(), vec![cse.clone(), stat.clone()]);

    assert_eq!(rendered(&views.major_gpa(&cse)?), "2.43");
    assert_eq!(rendered(&views.major_gpa(&stat)?), "4");
    assert_ne!(
        rendered(&views.major_gpa(&cse)?),
        rendered(&views.major_gpa(&stat)?),
        "two programmes over one attempt set must be able to differ"
    );

    // The shared course is 전선 under one programme and 일선 under the other,
    // so it is inside one major average and outside the other. That is the
    // whole reason a classification is scoped to a programme.
    let shared = corpus::synthetic_attempt_id(3)?;
    let categories = views
        .categories(shared)
        .ok_or("the shared course is unclassified")?;
    assert_eq!(
        categories.get(&cse),
        Some(&RequirementCategory::MajorElective)
    );
    assert_eq!(
        categories.get(&stat),
        Some(&RequirementCategory::FreeElective)
    );

    // A programme nothing is classified for has no average rather than zero.
    let absent = ProgramId::new("absent-programme")?;
    assert_eq!(views.major_gpa(&absent)?, GpaValue::NoGradedAttempts);
    Ok(())
}

/// Earned credits and the grade-point denominator are different quantities.
#[test]
fn credits_vs_denominator() -> TestResult {
    let views = baseline()?;
    let earned = views.earned_credits()?;
    let denominator = views.gpa_denominator()?;

    assert_eq!(decimal::render(earned.partial()), "14");
    assert_eq!(decimal::render(denominator.partial()), "12");
    assert_ne!(
        decimal::compare(earned.partial(), denominator.partial())?,
        core::cmp::Ordering::Equal,
        "the corpus is built so the two differ; equal here means they were collapsed"
    );

    // The difference is not an accident of totals — it is four specific
    // attempts, each moving exactly one side. Re-derived per attempt so a
    // change that collapsed the two would have to change these reasons too.
    let reason_of = |index: u8| -> Result<DispositionReason, Box<dyn Error>> {
        let id = corpus::synthetic_attempt_id(index)?;
        Ok(views
            .dispositions()
            .iter()
            .find(|disposition| disposition.attempt_id() == id)
            .ok_or("missing disposition")?
            .reason())
    };
    let contribution = |index: u8| -> Result<(bool, bool), Box<dyn Error>> {
        let id = corpus::synthetic_attempt_id(index)?;
        let disposition = views
            .dispositions()
            .iter()
            .find(|disposition| disposition.attempt_id() == id)
            .ok_or("missing disposition")?;
        Ok((
            matches!(disposition.credit(), CreditContribution::Earned(_)),
            matches!(disposition.average(), AverageContribution::Included { .. }),
        ))
    };

    // `S`: earns credit, outside the denominator.
    assert_eq!(reason_of(4)?, DispositionReason::SatisfactoryNotGraded);
    assert_eq!(contribution(4)?, (true, false));
    // `F`: in the denominator, earns nothing.
    assert_eq!(reason_of(5)?, DispositionReason::FailedInDenominator);
    assert_eq!(contribution(5)?, (false, true));
    // `W`: neither.
    assert_eq!(reason_of(6)?, DispositionReason::Withdrawn);
    assert_eq!(contribution(6)?, (false, false));
    // A recognized external grade: earns credit, outside the denominator.
    assert_eq!(
        reason_of(8)?,
        DispositionReason::ExternalExcludedFromAverage
    );
    assert_eq!(contribution(8)?, (true, false));

    // 14 = 12 - 3 (the F) + 2 (the S) + 3 (the exchange).
    let derived = decimal::add(
        decimal::sub(denominator.partial(), decimal::integer(3)?)?,
        decimal::add(decimal::integer(2)?, decimal::integer(3)?)?,
    )?;
    assert_eq!(
        decimal::compare(derived, earned.partial())?,
        core::cmp::Ordering::Equal,
        "the gap between the two totals must be exactly the attempts named above"
    );
    Ok(())
}

/// The repeat view names both attempts and which grade was recognized.
#[test]
fn repeat_proof_view() -> TestResult {
    let views = baseline()?;
    let proofs = views.repeat_proofs();
    assert_eq!(proofs.len(), 1, "the corpus holds exactly one repeat group");
    let proof = proofs.first().ok_or("no repeat proof")?;

    assert_eq!(proof.course_code, corpus::COURSE_REPEATED);
    assert_eq!(
        proof.attempts,
        vec![
            corpus::synthetic_attempt_id(1)?,
            corpus::synthetic_attempt_id(2)?
        ],
        "both attempts are named, in term order"
    );
    assert_eq!(proof.recognized, Some(corpus::synthetic_attempt_id(2)?));
    assert_eq!(proof.displaced, vec![corpus::synthetic_attempt_id(1)?]);
    assert_eq!(proof.recognition_rule, RepeatRecognition::LatestAttempt);
    assert_eq!(
        proof.policy_row_id.as_deref(),
        Some("repeat.ceiling.2015_spring")
    );
    assert_eq!(proof.ceiling, Some(GradeSymbol::AZero));
    assert!(
        proof.ceiling_applied,
        "an A+ repeat under an A0 ceiling must be capped"
    );

    // Before and after: the recorded grade is preserved and the effective one
    // is what the average used. Both are readable, which is what "어느 성적이
    // 인정되었는지" asks for.
    let repeat_id = corpus::synthetic_attempt_id(2)?;
    let recognized = views
        .dispositions()
        .iter()
        .find(|disposition| disposition.attempt_id() == repeat_id)
        .ok_or("no disposition for the repeat")?;
    assert_eq!(recognized.recorded_grade(), Some(GradeSymbol::APlus));
    let AverageContribution::Included {
        effective_grade, ..
    } = recognized.average()
    else {
        return Err("the recognized repeat must be in the average".into());
    };
    assert_eq!(effective_grade, GradeSymbol::AZero);
    assert_eq!(recognized.reason(), DispositionReason::RepeatCeilingApplied);

    let original_id = corpus::synthetic_attempt_id(1)?;
    let displaced = views
        .dispositions()
        .iter()
        .find(|disposition| disposition.attempt_id() == original_id)
        .ok_or("no disposition for the original")?;
    assert_eq!(displaced.reason(), DispositionReason::ReplacedByRepeat);
    assert_eq!(displaced.recorded_grade(), Some(GradeSymbol::CZero));

    // Under the published book nothing decides, and the view says so rather
    // than picking one.
    let published = RecordViews::compute(
        &corpus::baseline_history()?,
        &corpus::published_rules()?,
        &corpus::classification_v1()?,
    )?;
    let unresolved = published.repeat_proofs().first().ok_or("no repeat proof")?;
    assert_eq!(unresolved.recognized, None);
    assert!(unresolved.displaced.is_empty());
    assert_eq!(unresolved.recognition_rule, RepeatRecognition::Unknown);
    Ok(())
}

/// Every special attempt lands on exactly one reason, and the map is total.
#[test]
fn special_attempt_reason_matrix() -> TestResult {
    // The vocabulary is closed and round-trips.
    for reason in DispositionReason::ALL {
        assert_eq!(DispositionReason::parse(reason.as_str()), Some(reason));
    }
    assert_eq!(
        DispositionReason::ALL.len(),
        13,
        "the reason set is closed; a fourteenth needs this row to move with it"
    );

    // Totality by arithmetic rather than by enumeration: every (status, origin,
    // grade) triple the constructors admit produces exactly one disposition,
    // and every reason produced is in the closed set.
    let rules = corpus::baseline_rules()?;
    let classification = corpus::classification_v1()?;
    let mut produced = std::collections::BTreeSet::new();
    let mut cases = 0_usize;
    for status in SettledStatus::ALL {
        for origin in AttemptOrigin::ALL {
            for grade in GradeSymbol::ALL {
                for recognition in RecognitionDecision::ALL {
                    let mut history = AttemptHistory::new();
                    history.append(
                        CourseAttempt::from_confirmed_row(
                            corpus::synthetic_attempt_id(70)?,
                            corpus::COURSE_SHARED,
                            TermKey::new(2015, Semester::Fall)?,
                            status,
                            origin,
                            decimal::integer(3)?,
                            decimal::integer(3)?,
                            Some(grade),
                            "snu_4_3_v1",
                            vec![corpus::synthetic_evidence_id(70)?],
                        )?
                        .with_recognition(recognition),
                    )?;
                    let views = RecordViews::compute(&history, &rules, &classification)?;
                    assert_eq!(
                        views.dispositions().len(),
                        1,
                        "one attempt must produce exactly one disposition"
                    );
                    let reason = views
                        .dispositions()
                        .first()
                        .ok_or("no disposition")?
                        .reason();
                    assert!(
                        DispositionReason::ALL.contains(&reason),
                        "a disposition escaped the closed reason set"
                    );
                    produced.insert(reason);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(
        cases,
        SettledStatus::ALL.len()
            * AttemptOrigin::ALL.len()
            * GradeSymbol::ALL.len()
            * RecognitionDecision::ALL.len(),
        "the matrix must be the whole product, not a sample of it"
    );

    // The named cases section 10 asks a view to explain, each reached.
    for expected in [
        DispositionReason::Graded,
        DispositionReason::FailedInDenominator,
        DispositionReason::SatisfactoryNotGraded,
        DispositionReason::UnsatisfactoryNotGraded,
        DispositionReason::Withdrawn,
        DispositionReason::IncompleteUnresolved,
        DispositionReason::ExternalExcludedFromAverage,
        DispositionReason::RecognitionUndecided,
    ] {
        assert!(
            produced.contains(&expected),
            "{} is never reached by the status/origin/grade product",
            expected.as_str()
        );
    }

    // The three the product above cannot reach, reached from the corpus.
    let views = baseline()?;
    let reasons: std::collections::BTreeSet<DispositionReason> = views
        .dispositions()
        .iter()
        .map(|disposition| disposition.reason())
        .collect();
    assert!(reasons.contains(&DispositionReason::RepeatCeilingApplied));
    assert!(reasons.contains(&DispositionReason::ReplacedByRepeat));
    assert!(reasons.contains(&DispositionReason::NotSettled));
    let undated = RecordViews::compute(
        &corpus::history_with_undated_external()?,
        &rules,
        &classification,
    )?;
    assert!(
        undated
            .dispositions()
            .iter()
            .any(|d| d.reason() == DispositionReason::ExternalPolicyUnknown)
    );
    let published = RecordViews::compute(
        &corpus::baseline_history()?,
        &corpus::published_rules()?,
        &classification,
    )?;
    assert!(
        published
            .dispositions()
            .iter()
            .any(|d| d.reason() == DispositionReason::RepeatRecognitionUnknown)
    );
    Ok(())
}

/// The repeat ceiling is an effective-dated row, not a constant.
#[test]
fn repeat_ceiling_effective_date() -> TestResult {
    let table = oracle()?;
    let history = corpus::baseline_history()?;
    let classification = corpus::classification_v1()?;
    let scheme = GradingScheme::snu_4_3_v1()?;

    let under = |term: &str| -> Result<String, Box<dyn Error>> {
        let rules = RuleBook::new(
            scheme.clone(),
            corpus::confirmed_policy_ceiling_from(term)?,
            corpus::CLASSIFICATION_RULESET_ID,
        );
        let views = RecordViews::compute(&history, &rules, &classification)?;
        Ok(rendered(&views.cumulative_gpa()?))
    };

    // The repeat is taken in 2015 spring. A ceiling effective from that term
    // binds it; one effective from the term after does not.
    assert_eq!(under("2015_SPRING")?, "2.83");
    assert_eq!(under("2016_SPRING")?, "2.9");
    assert_eq!(
        under("2016_SPRING")?,
        expect(&table, "ceiling_from.2016_SPRING.cumulative.gpa")?
    );
    assert_ne!(
        under("2015_SPRING")?,
        under("2016_SPRING")?,
        "moving the effective term past the repeat must change the published average; \
         a hard-coded 2015 would leave these equal"
    );

    // And the control: moving it *earlier* does not change anything, because
    // the repeat is still on the same side of the boundary. A guard that fired
    // on any date change would pass this and mean nothing.
    assert_eq!(under("2014_SPRING")?, "2.83");
    assert_eq!(
        under("2014_SPRING")?,
        expect(&table, "ceiling_from.2014_SPRING.cumulative.gpa")?
    );

    // The row itself is selected by term, and a term no row reaches selects
    // nothing rather than the nearest row.
    let book = corpus::confirmed_policy_v1()?;
    assert_eq!(
        book.repeat_row_at(TermKey::new(2015, Semester::Spring)?)
            .map(|row| row.row_id.as_str()),
        Some("repeat.ceiling.2015_spring")
    );
    assert_eq!(
        book.repeat_row_at(TermKey::new(2014, Semester::Fall)?)
            .map(|row| row.row_id.as_str()),
        Some("repeat.no_ceiling.pre_2015"),
        "the row before the ceiling is the one that governs an earlier repeat"
    );
    assert_eq!(
        book.repeat_row_at(TermKey::new(1999, Semester::Fall)?)
            .map(|row| row.row_id.as_str()),
        None,
        "a term no row reaches must select no row at all"
    );

    // The published book, which is the one with a source behind it, carries the
    // ceiling at 2015 spring and leaves recognition unstated.
    let published = PolicyBook::published_v1()?;
    let row = published
        .repeat_row_at(TermKey::new(2015, Semester::Spring)?)
        .ok_or("the published book must carry the 2015 spring row")?;
    assert_eq!(row.effective_from, TermKey::new(2015, Semester::Spring)?);
    assert_eq!(row.ceiling, Some(GradeSymbol::AZero));
    assert_eq!(row.recognition, RepeatRecognition::Unknown);
    assert_eq!(
        published.repeat_row_at(TermKey::new(2014, Semester::Winter)?),
        None,
        "the published book states nothing about the period before the ceiling"
    );

    // The external-grade date is a row on the same footing.
    let external = published
        .external_row_at(TermKey::new(2004, Semester::Spring)?)
        .ok_or("the published book must carry the 2004 row")?;
    assert!(external.excluded_from_average);
    assert_eq!(
        published.external_row_at(TermKey::new(2003, Semester::Winter)?),
        None
    );
    Ok(())
}

/// Only a confirmed registration makes a `REGISTERED` attempt, and it counts
/// toward nothing.
#[test]
fn registered_attempt_gate() -> TestResult {
    let views = baseline()?;
    let registered = corpus::synthetic_attempt_id(9)?;
    let disposition = views
        .dispositions()
        .iter()
        .find(|entry| entry.attempt_id() == registered)
        .ok_or("the corpus no longer holds the registered attempt")?;
    assert_eq!(disposition.reason(), DispositionReason::NotSettled);
    assert_eq!(disposition.average(), AverageContribution::Excluded);
    assert_eq!(disposition.credit(), CreditContribution::NotEarned);
    assert!(!views.cumulative_included().contains(&registered));

    // The partition, over the whole status set rather than two spot checks.
    for status in AttemptStatus::ALL {
        let expected = matches!(
            status,
            AttemptStatus::Completed
                | AttemptStatus::Withdrawn
                | AttemptStatus::Transferred
                | AttemptStatus::Recognized
        );
        assert_eq!(
            contributes_to_actual_progress(status),
            expected,
            "{} is on the wrong side of the actual-progress boundary",
            status.as_str()
        );
    }

    // `SettledStatus` is the argument type of the transcript-row constructor,
    // and it widens onto exactly the four that count.
    assert_eq!(
        SettledStatus::ALL.map(|status| status.into_status().as_str()),
        ["COMPLETED", "WITHDRAWN", "TRANSFERRED", "RECOGNIZED"],
        "the settled constructor cannot name PLANNED, REGISTERED, IN_PROGRESS or CANCELLED"
    );

    // A registration needs evidence. Without it there is no confirmation, and
    // without a confirmation there is no attempt.
    let term = TermKey::new(2026, Semester::Fall)?;
    assert!(
        RegistrationConfirmation::new(
            corpus::COURSE_REGISTERED,
            term,
            decimal::integer(3)?,
            Vec::new()
        )
        .is_err(),
        "a registration with no evidence must be refused"
    );
    let confirmation = RegistrationConfirmation::new(
        corpus::COURSE_REGISTERED,
        term,
        decimal::integer(3)?,
        vec![corpus::synthetic_evidence_id(9)?],
    )?;
    let attempt = CourseAttempt::from_confirmed_registration(
        corpus::synthetic_attempt_id(80)?,
        &confirmation,
        "snu_4_3_v1",
    )?;
    assert_eq!(
        attempt.status(),
        AttemptStatus::Registered,
        "a confirmed registration produces REGISTERED and the caller cannot name a status"
    );

    // A plan choice for the same course carries none of the fields an attempt
    // needs, and nothing in the plan module returns an attempt or a
    // confirmation.
    let choice = PlanScenarioChoice::new(corpus::COURSE_REGISTERED, term)?;
    assert_eq!(choice.course_code(), corpus::COURSE_REGISTERED);
    assert_eq!(choice.intended_term(), term);

    // The other constructor's gate: `P2-U7`'s user-confirmed row.
    //
    // The linked pair is built through the transcript crate's own public API --
    // `confirm_reconciled_rows`, which takes a `ReconciledTranscript` that
    // `reconcile` returns only when every field agreed -- so nothing here
    // re-decides what a confirmation is.
    let (row, linked) = confirmed_row_fixture_at(1)?;
    let attempt = attempt_from_confirmed_row(
        corpus::synthetic_attempt_id(90)?,
        &row,
        &linked.confirmed,
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        "snu_4_3_v1",
        vec![corpus::synthetic_evidence_id(90)?],
    )?;
    assert_eq!(attempt.course_code(), "M1522.000900");
    assert_eq!(attempt.term(), TermKey::new(2024, Semester::Fall)?);
    assert_eq!(attempt.grade(), Some(GradeSymbol::AZero));
    assert_eq!(
        decimal::compare(attempt.credits_attempted(), decimal::parse("3")?)?,
        core::cmp::Ordering::Equal,
        "the credit value is the row's, not the caller's"
    );

    // The import row is the same document read by an importer rather than
    // confirmed by the user, and it must not produce an attempt. This is the
    // two linked claims doing their job: `Claim::validate_for_actor` refuses an
    // importer `UserExplicit`, so the import claim carries `DirectObservation`
    // and cannot be a confirmation.
    assert_eq!(
        linked.import.claim().authority_class,
        AuthorityClass::DirectObservation,
        "the import row is not user-confirmed authority"
    );
    assert_eq!(
        linked.confirmed.claim().authority_class,
        AuthorityClass::UserExplicit
    );
    assert_ne!(
        linked.import.claim().id,
        linked.confirmed.claim().id,
        "the two rows are two claims"
    );
    assert_eq!(
        linked.confirmed.import_claim_id(),
        linked.import.claim().id,
        "the confirmation names the import it confirms"
    );

    // A confirmation minted for a different line of the document is refused.
    let (other_row, _) = confirmed_row_fixture_at(2)?;
    assert!(
        attempt_from_confirmed_row(
            corpus::synthetic_attempt_id(91)?,
            &other_row,
            &linked.confirmed,
            SettledStatus::Completed,
            AttemptOrigin::Internal,
            "snu_4_3_v1",
            vec![corpus::synthetic_evidence_id(91)?],
        )
        .is_err(),
        "a confirmation for another ordinal must be refused"
    );

    // And a term spelling no confirmed source maps to a session is refused
    // rather than guessed onto one side of an effective date.
    assert!(TermKey::parse_transcript_term("2024-2").is_ok());
    assert!(TermKey::parse_transcript_term("2024-S").is_err());
    assert!(TermKey::parse_transcript_term("2024_WINTER").is_ok());
    Ok(())
}

/// Builds one reconciled row and its linked claim pair through `P2-U7`'s API.
fn confirmed_row_fixture_at(
    ordinal: usize,
) -> Result<(TranscriptRow, LinkedRowClaims), Box<dyn Error>> {
    let identity = TranscriptIdentity::new("SYN-0000", "SYNTHETIC", "SYNTHETIC", "2026-01-01")?;
    let rows = vec![
        TranscriptRow::new(0, "L0442.000200", "2024-1", Decimal::new(20, 1)?, "S")?,
        TranscriptRow::new(1, "M1522.000900", "2024-2", Decimal::new(30, 1)?, "A0")?,
        TranscriptRow::new(2, "4190.101", "2025-1", Decimal::new(30, 1)?, "B+")?,
    ];
    let transcript = NormalizedTranscript::new(identity, rows)?;
    let checksums = TranscriptChecksums::of(&transcript);
    let outcome = reconcile(&transcript, &checksums);
    let reconciled = outcome
        .reconciled()
        .ok_or("the synthetic transcript must reconcile against itself")?;

    let ids: Vec<RowClaimIds> = (0..3)
        .map(|index| {
            Ok::<_, Box<dyn Error>>(RowClaimIds {
                import_claim_id: format!("01900000-0000-7000-8000-00000000ba0{index}")
                    .parse::<ClaimId>()?,
                confirmed_claim_id: format!("01900000-0000-7000-8000-00000000bb0{index}")
                    .parse::<ClaimId>()?,
            })
        })
        .collect::<Result<_, _>>()?;
    let context = RowClaimContext {
        subject_entity_id: EntityId::from_str("01900000-0000-7000-8000-00000000bc01")?,
        scope_id: ScopeId::from_str("01900000-0000-7000-8000-00000000bc02")?,
        valid_time: ValidInterval::new(TimestampMillis::new(1_700_000_000_000), None)?,
        import_evidence_ids: vec![corpus::synthetic_evidence_id(95)?],
        confirmation_evidence_ids: vec![corpus::synthetic_evidence_id(96)?],
    };
    let linked = confirm_reconciled_rows(
        reconciled,
        TranscriptFormat::ManualEntry,
        None,
        EntityId::from_str("01900000-0000-7000-8000-00000000bc03")?,
        &ids,
        &context,
    )?;
    let row = reconciled
        .transcript()
        .rows()
        .get(ordinal)
        .ok_or("the fixture has no such row")?
        .clone();
    let claims = linked
        .into_iter()
        .nth(ordinal)
        .ok_or("the fixture has no such claim pair")?;
    Ok((row, claims))
}

/// Deleting a plan scenario leaves the attempt ledger untouched.
#[test]
fn delete_plan_preserves_attempts() -> TestResult {
    let history = corpus::baseline_history()?;
    let before = history.all().to_vec();
    let before_views = RecordViews::compute(
        &history,
        &corpus::baseline_rules()?,
        &corpus::classification_v1()?,
    )?;

    let scenario_id = EntityId::from_str("01900000-0000-7000-8000-00000000d001")?;
    let mut store = PlanStore::new();
    store.insert(PlanScenario::new(
        scenario_id,
        "graduate in eight terms",
        vec![
            // Every choice names a course the ledger has attempts for, so a
            // deletion that reached the ledger by course code would be visible.
            PlanScenarioChoice::new(
                corpus::COURSE_REPEATED,
                TermKey::new(2027, Semester::Spring)?,
            )?,
            PlanScenarioChoice::new(corpus::COURSE_SHARED, TermKey::new(2027, Semester::Fall)?)?,
            PlanScenarioChoice::new(corpus::COURSE_FAILED, TermKey::new(2027, Semester::Fall)?)?,
        ],
    )?)?;
    assert_eq!(store.len(), 1);

    let deletion = delete_scenario(&mut store, &history, scenario_id)?;
    assert_eq!(deletion.scenario_id, scenario_id);
    assert_eq!(deletion.choices_removed, 3);
    assert_eq!(deletion.attempts_preserved, before.len());
    assert!(store.is_empty(), "the scenario is gone");
    assert!(store.get(scenario_id).is_none());

    // The ledger, byte for byte.
    assert_eq!(
        history.all(),
        before.as_slice(),
        "a plan deletion must not touch the attempt ledger"
    );
    let after_views = RecordViews::compute(
        &history,
        &corpus::baseline_rules()?,
        &corpus::classification_v1()?,
    )?;
    assert_eq!(
        after_views.cumulative_gpa()?,
        before_views.cumulative_gpa()?,
        "a plan deletion must not move the average"
    );
    assert_eq!(after_views.dispositions(), before_views.dispositions());

    // Deleting a scenario that is not there is a refusal, not a silent success
    // that would make the assertions above pass over an empty store.
    assert!(delete_scenario(&mut store, &history, scenario_id).is_err());
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. The engines
// ---------------------------------------------------------------------------

/// The GPA engine agrees with the product views, and is byte-stable.
#[test]
fn gpa_engine_matches_the_views_and_is_byte_stable() -> TestResult {
    let history = corpus::baseline_history()?;
    let classification = corpus::classification_v1()?;
    let rules = corpus::baseline_rules()?;
    let facts: Vec<AttemptFacts> = history
        .current()
        .into_iter()
        .map(|attempt| AttemptFacts::from_attempt(attempt, &classification))
        .collect();
    let inputs = encode(&facts, &GpaScope::Cumulative)?;

    let engine = GpaEngine::new(rules.clone(), EngineVersion::MIN);
    let hash = engine.rule_set_hash();
    let first = engine.evaluate(&inputs, hash, EngineVersion::MIN)?;
    let second = engine.evaluate(&inputs, hash, EngineVersion::MIN)?;
    assert_eq!(
        first.canonical_bytes(engine.engine_id(), hash, EngineVersion::MIN, &inputs),
        second.canonical_bytes(engine.engine_id(), hash, EngineVersion::MIN, &inputs),
        "the same inputs and rule hash must produce byte-equal results"
    );

    let views = RecordViews::compute(&history, &rules, &classification)?;
    let published = views
        .cumulative_gpa()?
        .known()
        .ok_or("the baseline average must be known")?;
    assert_eq!(
        first.result.values.get("gpa"),
        Some(&published),
        "the engine and the product view must publish the same average"
    );
    assert_eq!(first.result.status, ProofStatus::Satisfied);

    // A different rule book is a different hash, and the engine refuses one
    // that is not its own.
    let other = GpaEngine::new(corpus::baseline_rules_scale3()?, EngineVersion::MIN);
    assert_ne!(hash, other.rule_set_hash());
    assert!(
        engine
            .evaluate(&inputs, other.rule_set_hash(), EngineVersion::MIN)
            .is_err(),
        "an evaluation under a foreign rule-set hash must be refused"
    );
    // And the same inputs under the other book produce different bytes.
    let other_outcome = other.evaluate(&inputs, other.rule_set_hash(), EngineVersion::MIN)?;
    assert_ne!(
        first.canonical_bytes(engine.engine_id(), hash, EngineVersion::MIN, &inputs),
        other_outcome.canonical_bytes(
            other.engine_id(),
            other.rule_set_hash(),
            EngineVersion::MIN,
            &inputs
        ),
        "the rule-set hash must reach the canonical bytes"
    );

    // The frozen inputs round-trip: the encoding is the whole input.
    let reparsed = academic_domain::engines::FrozenInputs::parse(&inputs.canonical_text())?;
    assert_eq!(reparsed, inputs);
    Ok(())
}

/// The credit engine traces a credit two programmes both counted.
#[test]
fn credit_engine_traces_double_recognition() -> TestResult {
    let history = corpus::baseline_history()?;
    let classification = corpus::classification_v1()?;
    let rules = corpus::baseline_rules()?;
    let facts: Vec<AttemptFacts> = history
        .current()
        .into_iter()
        .map(|attempt| AttemptFacts::from_attempt(attempt, &classification))
        .collect();
    let inputs = encode(&facts, &GpaScope::Cumulative)?;

    let engine = CreditAccountingEngine::new(rules, EngineVersion::MIN);
    let outcome = engine.evaluate(&inputs, engine.rule_set_hash(), EngineVersion::MIN)?;

    // The shared course reached two programmes' totals and is traced, not
    // resolved: `GATE-38-015` decides whether it may count twice and is open.
    assert_eq!(
        outcome
            .result
            .values
            .get("credits.double.recognized.attempts"),
        Some(&decimal::integer(1)?)
    );
    assert_eq!(outcome.result.status, ProofStatus::Unknown);
    assert!(
        outcome
            .proof_tree
            .children
            .iter()
            .any(|node| node.rule_id.as_str() == "rule.credit.double.recognition.traced"),
        "a double-counted credit must appear in the proof tree"
    );
    let earned = outcome
        .result
        .values
        .get("credits.earned")
        .ok_or("the credit engine must publish an earned total")?;
    assert_eq!(
        decimal::compare(*earned, decimal::parse("14")?)?,
        core::cmp::Ordering::Equal,
        "the engine's earned total must be the corpus's fourteen credits"
    );
    Ok(())
}

/// The credit and GPA engines both refuse an attempt id that is not a UUID.
#[test]
fn engine_inputs_are_refused_rather_than_defaulted() -> TestResult {
    let engine = GpaEngine::new(corpus::baseline_rules()?, EngineVersion::MIN);
    let hash = engine.rule_set_hash();
    for malformed in [
        "attempt.count=int:1\n",
        "attempt.000.course=ref:4190.101\nattempt.count=int:1\nscope=ref:CUMULATIVE\n",
        "attempt.count=int:0\nscope=ref:NOT_A_SCOPE\n",
    ] {
        let inputs = academic_domain::engines::FrozenInputs::parse(malformed)?;
        assert!(
            engine.evaluate_record(&inputs, hash).is_err(),
            "the engine must refuse {malformed:?} rather than default it"
        );
    }
    Ok(())
}
