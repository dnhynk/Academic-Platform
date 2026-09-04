//! `t068` section 5's twelve named acceptance tests for `P2-U5`, plus the
//! three halves they rest on: the independent oracle, byte-equal replay, and
//! the actor-matrix observation this task found one step out.
//!
//! Every fixture is `academic_offering::corpus`'s, which is synthetic by
//! construction: no connector runs, no socket opens, no clock is read, and no
//! course code below names a real catalogue entry.

mod support;

use std::{collections::BTreeMap, fs, path::PathBuf};

use academic_curriculum::OfferingStatus;
use academic_domain::{
    Actor, AuthorityClass, EpistemicStatus, PREDICTION_METADATA_VERSION_V1, TimestampMillis,
    engines::ProofStatus,
};
use academic_ingestion::SourceCategory;
use academic_model_run::CalibrationRegistry;
use academic_offering::{
    AbstentionReason, CourseHistory, CourseLifecycle, DecisionStanding, DeterminatePlan,
    FeatureFamily, FeatureVector, ForecastPolicy, ForecastVerdict, NoticeEffect, ObservationWindow,
    Offered, OfferingClaimSet, OfferingError, OfferingStanding, OfficialTermReading, PlanOutcome,
    PlanRefusal, RecentNotice, TermEvaluation, TermObservation, confirmation_claim, corpus,
    forecast, forecast_claim, metrics::EvaluationEntry, resolve,
};
use academic_record::term::{Semester, TermKey};
use support::TestResult;

// ---------------------------------------------------------------------------
// The independent oracle
// ---------------------------------------------------------------------------

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// `tools/offering-forecast-oracle.mjs`'s committed render, as rows.
fn oracle() -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(
        repository_root()
            .join("testdata")
            .join("offering-forecast")
            .join("oracle.expected"),
    )?;
    let mut rows = BTreeMap::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("oracle line has no '=': {line}"))?;
        rows.insert(key.to_owned(), value.to_owned());
    }
    Ok(rows)
}

fn expect(
    rows: &BTreeMap<String, String>,
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    rows.get(key)
        .cloned()
        .ok_or_else(|| format!("the oracle no longer carries {key}").into())
}

/// Every corpus case's score, probability, abstention and standing is the one
/// an independent transcription in another language derives.
///
/// The oracle is a second transcription of the histories, the rule set, the
/// calibration curve and the arithmetic. Without it, a comparison of the
/// engine's proof tree against the engine's own numbers would prove only that
/// the engine is deterministic.
#[test]
fn the_corpus_agrees_with_an_independent_oracle() -> TestResult {
    let rows = oracle()?;
    // The floor is what fails if the corpus shrinks to nothing: a comparison
    // over an empty case list satisfies every assertion inside the loop.
    assert!(corpus::CASES.len() >= 10);

    for case in corpus::CASES {
        let resolution = support::resolve_case(case)?;
        let forecast = resolution
            .forecast()
            .ok_or_else(|| format!("{case} produced no forecast"))?;

        assert_eq!(
            forecast.course().as_str(),
            expect(&rows, &format!("case.{case}.course"))?,
            "{case} is a different course than the oracle transcribed"
        );
        assert_eq!(
            forecast.raw_units().to_string(),
            expect(&rows, &format!("case.{case}.raw_units"))?,
            "{case} raw score"
        );
        assert_eq!(
            forecast.features().seasonal_terms().to_string(),
            expect(&rows, &format!("case.{case}.window.seasonal_terms"))?,
            "{case} window depth"
        );
        assert_eq!(
            forecast.features().positive_samples().to_string(),
            expect(&rows, &format!("case.{case}.window.positive_samples"))?,
            "{case} positive samples"
        );
        for family in FeatureFamily::ALL {
            let name = family.as_str().to_ascii_lowercase();
            let signal = forecast.features().signal(family);
            assert_eq!(
                signal.value().to_string(),
                expect(&rows, &format!("case.{case}.value.{name}"))?,
                "{case} {name} value"
            );
            assert_eq!(
                signal.contribution().to_string(),
                expect(&rows, &format!("case.{case}.contribution.{name}"))?,
                "{case} {name} contribution"
            );
        }

        let (permille, abstention) = match forecast.verdict() {
            ForecastVerdict::Scored(scored) => (scored.confidence().value().to_string(), "NONE"),
            ForecastVerdict::Abstained(reason) => ("ABSTAINED".to_owned(), reason.as_str()),
        };
        assert_eq!(
            permille,
            expect(&rows, &format!("case.{case}.calibrated_permille"))?,
            "{case} calibrated probability"
        );
        assert_eq!(
            abstention,
            expect(&rows, &format!("case.{case}.abstention"))?,
            "{case} abstention"
        );
        assert_eq!(
            resolution.standing().status().as_str(),
            expect(&rows, &format!("case.{case}.standing"))?,
            "{case} standing"
        );
    }
    Ok(())
}

/// Two evaluations of one history agree byte for byte, and one under a
/// different rule-set hash does not.
///
/// Without the second half the first would pass on an encoding that ignored
/// the rule set, which is the exact hole `P2-C5` records for its own
/// equivalent.
#[test]
fn same_inputs_and_rule_hash_yield_byte_equal_results() -> TestResult {
    let (history, window) = support::case("every_spring")?;
    let policy = support::policy()?;
    let registry = support::registry()?;

    let first = forecast(&history, window, policy, &registry, support::now())?;
    let second = forecast(&history, window, policy, &registry, support::now())?;
    assert_eq!(first.canonical_bytes()?, second.canonical_bytes()?);

    let under_other_hash = first.outcome().canonical_bytes(
        academic_offering::OFFERING_FORECAST_ENGINE_ID,
        academic_domain::engines::RuleSetHash::new(academic_domain::ContentDigest::sha256(
            b"a different rule set",
        )),
        academic_domain::engines::EngineVersion::new(
            academic_offering::OFFERING_FORECAST_ENGINE_VERSION,
        )?,
        first.inputs(),
    );
    assert_ne!(first.canonical_bytes()?, under_other_hash);
    assert_eq!(first.outcome().result, second.outcome().result);

    // And the recorded criteria are frozen inputs, so the same history under a
    // different recorded floor is a different evaluation. Without this half the
    // two above would pass on an encoding that left the policy out -- the
    // canonical bytes would then say two forecasts agreed when they were
    // answering different questions.
    let stricter = ForecastPolicy::new(950, corpus::CORPUS_MINIMUM_WINDOW_TERMS)?;
    let under_stricter_floor = forecast(&history, window, stricter, &registry, support::now())?;
    assert_ne!(
        first.canonical_bytes()?,
        under_stricter_floor.canonical_bytes()?
    );
    let floor_key = academic_domain::engines::InputKey::new("policy.likely_floor_permille")?;
    assert!(first.inputs().get(&floor_key).is_some());
    let window_key = academic_domain::engines::InputKey::new("policy.minimum_window_terms")?;
    assert!(first.inputs().get(&window_key).is_some());
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-08-024`
// ---------------------------------------------------------------------------

/// Only a registration-system reading inside the recorded verification bound
/// becomes `CONFIRMED`, and a confirmed standing carries the real timetable and
/// the real seat count.
#[test]
fn offering_confirmed_contract() -> TestResult {
    let evidence = support::confirmation("M9001.000100", Vec::new())?;
    let official = OfficialTermReading::Confirmed(evidence);
    let resolution = support::resolve_case_with("every_spring", &official)?;

    let OfferingStanding::Confirmed(confirmed) = resolution.standing() else {
        return Err(format!(
            "a fresh registration reading is not CONFIRMED: {:?}",
            resolution.standing()
        )
        .into());
    };
    assert_eq!(resolution.standing().status(), OfferingStatus::Confirmed);
    assert_eq!(resolution.standing().ui_copy(), "공식 개설 확인 · 확인일");
    assert_eq!(
        resolution.standing().planner_treatment(),
        "실제 시간표·정원 사용"
    );
    assert_eq!(confirmed.verified_at(), support::now());

    // Section 8.3's Planner 취급 cell for this row is 실제 시간표·정원 사용, so
    // the seat carries both and neither is invented.
    let seat = confirmed.seat();
    assert_eq!(
        seat.capacity().map(academic_curriculum::Capacity::seats),
        Some(60)
    );
    assert_eq!(seat.meetings().len(), 1);
    assert_eq!(seat.term(), support::spring_2026()?);

    // A reading older than the recorded bound confirms nothing. The bound is
    // one day; this one is two.
    let stale = academic_offering::ConfirmationEvidence::from_registration_system(
        support::registration_listing(
            "M9001.000100",
            TimestampMillis::new(corpus::CORPUS_NOW_MILLIS - 172_800_000),
        )?,
        Vec::new(),
        corpus::verification_recency()?,
        support::now(),
    );
    assert!(matches!(stale, Err(OfferingError::VerificationStale)));

    // And a reading that found no section confirms nothing either: that is an
    // observation about the term, not a confirmation of one.
    let empty = academic_offering::ConfirmationEvidence::from_registration_system(
        academic_offering::OfficialListing::new(
            SourceCategory::RegistrationSystem,
            support::connector("sugang.snu.ac.kr")?,
            TimestampMillis::new(corpus::CORPUS_NOW_MILLIS - 3_600_000),
            support::spring_2026()?,
            support::course_code("M9001.000100")?,
            false,
        ),
        Vec::new(),
        corpus::verification_recency()?,
        support::now(),
    );
    assert!(matches!(empty, Err(OfferingError::BasisListsNoSection)));
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-08-025`
// ---------------------------------------------------------------------------

/// A reproducible seasonal pattern with no official future reading is
/// `HISTORICALLY_LIKELY`, says so in section 8.3's words, and the
/// determinate-plan validator refuses it.
#[test]
fn historical_likely_limits() -> TestResult {
    let resolution = support::resolve_case("every_spring")?;
    let OfferingStanding::HistoricallyLikely(likely) = resolution.standing() else {
        return Err(format!(
            "a six-term spring pattern is not HISTORICALLY_LIKELY: {:?}",
            resolution.standing()
        )
        .into());
    };
    assert_eq!(
        resolution.standing().status(),
        OfferingStatus::HistoricallyLikely
    );
    assert_eq!(resolution.standing().ui_copy(), "과거 패턴상 가능성");
    assert_eq!(
        resolution.standing().planner_treatment(),
        "placeholder만, 졸업계획 확정에 사용 금지"
    );
    assert!(likely.calibrated().confidence().value() >= corpus::CORPUS_LIKELY_FLOOR_PERMILLE);
    // The number a reader sees comes through `P2-M1`'s displayed type and
    // names the dataset that interpreted it.
    assert!(
        likely
            .displayed()
            .to_string()
            .contains("offering.forecast.synthetic.v1")
    );

    // The prohibition: no seat, so nothing to commit.
    assert!(resolution.standing().seat().is_none());
    let scenario = support::scenario_for("M9001.000100")?;
    let PlanOutcome::Indeterminate(indeterminate) = DeterminatePlan::commit(&scenario, Vec::new())
    else {
        return Err("a likely offering was committed into a determinate plan".into());
    };
    assert_eq!(indeterminate.refusals().len(), 1);
    assert_eq!(
        indeterminate.refusals().first().map(PlanRefusal::as_str),
        Some("NO_CONFIRMED_SEAT")
    );

    // And an official future reading takes the standing away from the
    // forecast, which is section 8.3's *미래 공식 공지 없음* requirement.
    let official =
        OfficialTermReading::Confirmed(support::confirmation("M9001.000100", Vec::new())?);
    let with_official = support::resolve_case_with("every_spring", &official)?;
    assert_eq!(with_official.standing().status(), OfferingStatus::Confirmed);
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-08-026`
// ---------------------------------------------------------------------------

/// Section 8.3's three `UNCERTAIN` grounds each reach `UNCERTAIN`, each names
/// itself, and a plan over one demands an alternative path.
#[test]
fn uncertain_offering_flow() -> TestResult {
    let grounds = [
        ("sparse", AbstentionReason::WindowBelowRecordedMinimum),
        ("irregular_only", AbstentionReason::IrregularOnly),
        ("instructor_volatile", AbstentionReason::InstructorVolatile),
    ];
    for (case, expected) in grounds {
        let resolution = support::resolve_case(case)?;
        let OfferingStanding::Uncertain(uncertain) = resolution.standing() else {
            return Err(format!("{case} is not UNCERTAIN: {:?}", resolution.standing()).into());
        };
        assert_eq!(uncertain.reason(), expected, "{case}");
        assert!(
            expected.spec_phrase().is_some(),
            "{case} names a ground section 8.3 does not write"
        );
        assert_eq!(resolution.standing().ui_copy(), "근거 부족");
        assert_eq!(
            resolution.standing().planner_treatment(),
            "경고와 대체 경로 요구"
        );
        assert!(resolution.standing().seat().is_none());
    }

    // The alternative path is a value the refusal carries, not a sentence in a
    // comment.
    let scenario = support::scenario_for("M9001.000200")?;
    let PlanOutcome::Indeterminate(indeterminate) = DeterminatePlan::commit(&scenario, Vec::new())
    else {
        return Err("an uncertain offering was committed into a determinate plan".into());
    };
    assert_eq!(
        indeterminate.alternative_paths_required(),
        vec!["M9001.000200"]
    );
    assert!(
        indeterminate
            .refusals()
            .first()
            .is_some_and(|refusal| refusal.action().contains("alternative"))
    );

    // With no recorded forecast policy there is no floor, so the standing is
    // `UNCERTAIN` naming the absence rather than a probability compared
    // against a default.
    let (history, window) = support::case("every_spring")?;
    let without_policy = resolve(
        &history,
        window,
        None,
        None,
        &support::registry()?,
        support::now(),
    )?;
    let OfferingStanding::Uncertain(uncertain) = without_policy.standing() else {
        return Err("an unrecorded forecast policy produced a standing".into());
    };
    assert_eq!(uncertain.reason(), AbstentionReason::ForecastPolicyAbsent);
    assert!(without_policy.forecast().is_none());

    // And with a stale calibration dataset the probability is refused rather
    // than shown uninterpreted -- `P2-M1`'s rung, applied here.
    let stale = corpus::calibration_registry(support::stale_refresh())?;
    let uncalibrated = resolve(
        &history,
        window,
        None,
        Some(support::policy()?),
        &stale,
        support::now(),
    )?;
    let OfferingStanding::Uncertain(uncertain) = uncalibrated.standing() else {
        return Err("a stale calibration dataset produced a likely standing".into());
    };
    assert_eq!(
        uncertain.reason(),
        AbstentionReason::NoFreshCalibrationDataset
    );
    assert!(uncertain.scored().is_none());
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-08-027`
// ---------------------------------------------------------------------------

/// An official cancellation makes the current selection impossible and leaves
/// an already-committed plan byte-identical.
#[test]
fn cancelled_offering_contract() -> TestResult {
    // A plan committed while the offering was confirmed.
    let confirmed =
        OfficialTermReading::Confirmed(support::confirmation("M9001.000100", Vec::new())?);
    let before = support::resolve_case_with("every_spring", &confirmed)?;
    let seat = before
        .standing()
        .seat()
        .ok_or("a confirmed standing produced no seat")?;
    let scenario = support::scenario_for("M9001.000100")?;
    let PlanOutcome::Determinate(committed) =
        DeterminatePlan::commit(&scenario, vec![seat.clone()])
    else {
        return Err("a confirmed offering was refused a determinate plan".into());
    };
    assert_eq!(committed.len(), 1);

    // The cancellation arrives.
    let cancelled = OfficialTermReading::Cancelled(support::cancellation("M9001.000100")?);
    let after = support::resolve_case_with("every_spring", &cancelled)?;
    assert_eq!(after.standing().status(), OfferingStatus::Cancelled);
    assert_eq!(after.standing().ui_copy(), "공식 취소");
    assert_eq!(
        after.standing().planner_treatment(),
        "선택 불가, 과거 scenario 보존"
    );

    // 선택 불가: no seat, so a new plan cannot be committed.
    assert!(after.standing().seat().is_none());
    assert!(matches!(
        DeterminatePlan::commit(&scenario, Vec::new()),
        PlanOutcome::Indeterminate(_)
    ));

    // 과거 scenario 보존: the plan committed earlier is unchanged, and
    // recommitting it from the same seat reproduces it exactly.
    let replayed = DeterminatePlan::commit(&scenario, vec![seat]);
    assert_eq!(replayed, PlanOutcome::Determinate(committed));

    // A cancellation from a level that does not publish offering changes is
    // refused rather than accepted as an official notice.
    let from_a_prediction = academic_offering::CancellationNotice::official(
        SourceCategory::HistoricalPrediction,
        support::connector("history")?,
        support::now(),
        support::spring_2026()?,
        support::course_code("M9001.000100")?,
    );
    assert!(matches!(
        from_a_prediction,
        Err(OfferingError::NotTheRegistrationSystem(_))
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-08-028`
// ---------------------------------------------------------------------------

/// The registration system is the basis and every other level of section 8.4 is
/// a cross source, whatever its number or its age.
#[test]
fn offering_source_authority() -> TestResult {
    // Every level of section 8.4, run through the one constructor. A level
    // added to `SourceCategory::ALL` arrives refused rather than unconsidered.
    let retrieved_at = TimestampMillis::new(corpus::CORPUS_NOW_MILLIS - 3_600_000);
    let mut admitted = Vec::new();
    for source in SourceCategory::ALL {
        let listing = academic_offering::OfficialListing::new(
            source,
            support::connector("connector")?,
            retrieved_at,
            support::spring_2026()?,
            support::course_code("M9001.000100")?,
            true,
        );
        let built = academic_offering::ConfirmationEvidence::from_registration_system(
            listing,
            Vec::new(),
            corpus::verification_recency()?,
            support::now(),
        );
        match built {
            Ok(_) => admitted.push(source),
            Err(OfferingError::NotTheRegistrationSystem(named)) => {
                assert_eq!(named, source.as_str());
            }
            Err(other) => {
                return Err(format!("{source:?} was refused for the wrong reason: {other}").into());
            }
        }
    }
    assert_eq!(admitted, vec![SourceCategory::RegistrationSystem]);

    // A stale department page disagreeing with a fresh registration reading is
    // disclosed and does not become the basis.
    let stale_page = support::department_listing(
        "M9001.000100",
        TimestampMillis::new(corpus::CORPUS_NOW_MILLIS - 2_592_000_000),
        false,
    )?;
    let evidence = support::confirmation("M9001.000100", vec![stale_page])?;
    assert_eq!(
        evidence.basis().source(),
        SourceCategory::RegistrationSystem
    );
    assert_eq!(evidence.disagreements().len(), 1);
    assert_eq!(
        evidence.disagreements().first().map(|found| found.source()),
        Some(SourceCategory::DepartmentPage)
    );
    assert_eq!(
        evidence
            .disagreements()
            .first()
            .map(academic_offering::CrossSourceDisagreement::said_a_section_exists),
        Some(false)
    );

    // And a *newer* department page does not win either: recency does not move
    // the basis any more than a list position does.
    let newer_page = support::department_listing("M9001.000100", support::now(), false)?;
    let evidence = support::confirmation("M9001.000100", vec![newer_page])?;
    assert_eq!(
        evidence.basis().source(),
        SourceCategory::RegistrationSystem
    );
    assert!(evidence.basis().retrieved_at() < support::now());
    assert_eq!(evidence.disagreements().len(), 1);

    // An agreeing cross source is consulted and produces no disagreement.
    let agreeing = support::department_listing("M9001.000100", support::now(), true)?;
    let evidence = support::confirmation("M9001.000100", vec![agreeing])?;
    assert_eq!(evidence.cross_sources().len(), 1);
    assert!(evidence.disagreements().is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-08-029`
// ---------------------------------------------------------------------------

/// All seven families are section 8.3's, and each one **moves the score**.
///
/// Each family gets a **pair** that differs in that family alone, so a family
/// that was declared and never reached the model fails here. The pair is the
/// unit rather than one shared baseline because two families cannot always be
/// varied against the same control: a course offered in every term of its
/// window has no gap to lengthen without also changing its seasonal rate, and
/// the pairs below keep every other family's contribution equal by
/// construction -- which the inner loop asserts rather than assumes.
///
/// The last block is the majority-vote refutation: two histories a vote over
/// the last N terms cannot tell apart get different answers.
#[test]
fn offering_feature_contract() -> TestResult {
    assert_eq!(FeatureFamily::ALL.len(), 7);
    let window = corpus::window("every_spring")?;

    let baseline = corpus::history("every_spring")?;
    let pairs: [(FeatureFamily, CourseHistory, CourseHistory); 7] = [
        (
            FeatureFamily::Seasonality,
            baseline.clone(),
            seasonality_variant()?,
        ),
        (
            FeatureFamily::LifecycleStatus,
            baseline.clone(),
            lifecycle_variant()?,
        ),
        (
            FeatureFamily::InstructorChange,
            baseline.clone(),
            instructor_variant()?,
        ),
        (
            FeatureFamily::RecentNotices,
            baseline.clone(),
            notice_variant()?,
        ),
        (
            FeatureFamily::OfferingGap,
            gap_at_the_far_end()?,
            gap_at_the_near_end()?,
        ),
        (
            FeatureFamily::IrregularSpecial,
            baseline.clone(),
            irregular_variant()?,
        ),
        (
            FeatureFamily::HistoryWindow,
            window_variant()?,
            baseline.clone(),
        ),
    ];

    for (family, control, variant) in &pairs {
        let control_vector = FeatureVector::extract(control, window);
        let variant_vector = FeatureVector::extract(variant, window);
        assert_ne!(
            variant_vector.raw_units(),
            control_vector.raw_units(),
            "{} does not move the score",
            family.as_str()
        );
        for other in FeatureFamily::ALL {
            if other == *family {
                assert_ne!(
                    variant_vector.signal(other).contribution(),
                    control_vector.signal(other).contribution(),
                    "{}'s own contribution did not move",
                    family.as_str()
                );
            } else {
                assert_eq!(
                    variant_vector.signal(other).contribution(),
                    control_vector.signal(other).contribution(),
                    "varying {} also moved {}",
                    family.as_str(),
                    other.as_str()
                );
            }
        }
        // Every family's value reaches the frozen inputs, so an explanation
        // says what was measured and not only what it was worth.
        let evaluated = forecast(
            variant,
            window,
            support::policy()?,
            &support::registry()?,
            support::now(),
        )?;
        let key = academic_domain::engines::InputKey::new(family.input_key())?;
        assert!(
            evaluated.inputs().get(&key).is_some(),
            "{} has no frozen input",
            family.as_str()
        );
    }

    // Section 8.3 refuses a majority vote. `gap_two` and `every_other_spring`
    // have the same seasonal rate, window depth, instructor set and notices,
    // and a vote over the last N terms would answer them identically.
    let gap_two = support::resolve_case("gap_two")?;
    let every_other = support::resolve_case("every_other_spring")?;
    let gap_vector =
        FeatureVector::extract(&corpus::history("gap_two")?, corpus::window("gap_two")?);
    let other_vector = FeatureVector::extract(
        &corpus::history("every_other_spring")?,
        corpus::window("every_other_spring")?,
    );
    assert_eq!(
        gap_vector.signal(FeatureFamily::Seasonality).value(),
        other_vector.signal(FeatureFamily::Seasonality).value()
    );
    assert_eq!(gap_vector.seasonal_terms(), other_vector.seasonal_terms());
    assert_eq!(
        gap_vector.positive_samples(),
        other_vector.positive_samples()
    );
    assert_ne!(
        gap_two.standing().status(),
        every_other.standing().status(),
        "a gap at the near end and a gap at the far end read the same"
    );

    // And the seasonal window is what makes that possible: the same history
    // asked for the other semester is a different question.
    let spring = support::resolve_case("every_spring")?;
    let autumn = support::resolve_case("spring_only_asked_for_autumn")?;
    assert_eq!(
        spring.standing().status(),
        OfferingStatus::HistoricallyLikely
    );
    assert_eq!(autumn.standing().status(), OfferingStatus::Uncertain);
    Ok(())
}

/// Six springs, one stable instructor, no notices, offered in the three
/// **oldest**: a gap of three.
fn gap_at_the_far_end() -> Result<CourseHistory, Box<dyn std::error::Error>> {
    springs([false, false, false, true, true, true])
}

/// The same six springs with the three offered ones at the **newest** end: a
/// gap of zero, and every other family unchanged.
fn gap_at_the_near_end() -> Result<CourseHistory, Box<dyn std::error::Error>> {
    springs([true, true, true, false, false, false])
}

/// Builds six spring terms, newest first, with one stable instructor.
fn springs(offered: [bool; 6]) -> Result<CourseHistory, Box<dyn std::error::Error>> {
    let mut history = CourseHistory::new(support::course_code("M9001.000100")?);
    for (index, (year, offered)) in [2025_u16, 2024, 2023, 2022, 2021, 2020]
        .into_iter()
        .zip(offered)
        .enumerate()
    {
        let index = i64::try_from(index)?;
        let term = TermKey::new(year, Semester::Spring)?;
        history.observe(if offered {
            TermObservation::offered(
                term,
                read_at(index),
                vec![academic_curriculum::InstructorName::parse("Instructor A")?],
                false,
            )
        } else {
            TermObservation::not_offered(term, read_at(index))
        })?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// Six springs, three offered, alternating: the same window depth, gap,
/// instructor set and notices as the baseline with half the seasonal rate.
fn seasonality_variant() -> Result<CourseHistory, Box<dyn std::error::Error>> {
    springs([true, false, true, false, true, false])
}

/// The baseline history under a catalogue entry that starts after the term.
fn lifecycle_variant() -> Result<CourseHistory, Box<dyn std::error::Error>> {
    let mut history = corpus::history("every_spring")?;
    history.set_lifecycle(CourseLifecycle::NewFrom(TermKey::new(
        2027,
        Semester::Spring,
    )?));
    Ok(history)
}

/// The baseline history with two instructor sets instead of one.
fn instructor_variant() -> Result<CourseHistory, Box<dyn std::error::Error>> {
    let mut history = CourseHistory::new(support::course_code("M9001.000100")?);
    for (index, (year, who)) in [
        (2025_u16, "Instructor A"),
        (2024, "Instructor A"),
        (2023, "Instructor A"),
        (2022, "Instructor A"),
        (2021, "Instructor A"),
        (2020, "Instructor Z"),
    ]
    .into_iter()
    .enumerate()
    {
        let index = i64::try_from(index)?;
        history.observe(TermObservation::offered(
            TermKey::new(year, Semester::Spring)?,
            read_at(index),
            vec![academic_curriculum::InstructorName::parse(who)?],
            false,
        ))?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// The baseline history with one official notice inside the window.
fn notice_variant() -> Result<CourseHistory, Box<dyn std::error::Error>> {
    let mut history = corpus::history("every_spring")?;
    history.notice(RecentNotice::new(
        TermKey::new(2024, Semester::Fall)?,
        NoticeEffect::CurriculumChange,
    ));
    Ok(history)
}

/// The baseline history with one of the six runs flagged as a special run.
fn irregular_variant() -> Result<CourseHistory, Box<dyn std::error::Error>> {
    let mut history = CourseHistory::new(support::course_code("M9001.000100")?);
    for (index, year) in [2025_u16, 2024, 2023, 2022, 2021, 2020]
        .into_iter()
        .enumerate()
    {
        let index = i64::try_from(index)?;
        history.observe(TermObservation::offered(
            TermKey::new(year, Semester::Spring)?,
            read_at(index),
            vec![academic_curriculum::InstructorName::parse("Instructor A")?],
            year == 2020,
        ))?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// Three springs instead of six, every one offered: the same rate, the same
/// gap, the same instructor, a shallower window.
fn window_variant() -> Result<CourseHistory, Box<dyn std::error::Error>> {
    let mut history = CourseHistory::new(support::course_code("M9001.000100")?);
    for (index, year) in [2025_u16, 2024, 2023].into_iter().enumerate() {
        let index = i64::try_from(index)?;
        history.observe(TermObservation::offered(
            TermKey::new(year, Semester::Spring)?,
            read_at(index),
            vec![academic_curriculum::InstructorName::parse("Instructor A")?],
            false,
        ))?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

fn read_at(index: i64) -> TimestampMillis {
    TimestampMillis::new(corpus::CORPUS_NOW_MILLIS - 15_552_000_000 * index)
}

// ---------------------------------------------------------------------------
// `REQ-08-030`
// ---------------------------------------------------------------------------

/// Every scored forecast carries its course, a calibrated probability with the
/// dataset that produced it, and the sample window -- in `§2.3-15`'s existing
/// `prediction_metadata` shape at version one.
#[test]
fn course_forecast_metadata() -> TestResult {
    let resolution = support::resolve_case("every_spring")?;
    let forecast = resolution.forecast().ok_or("no forecast")?;
    let ForecastVerdict::Scored(scored) = forecast.verdict() else {
        return Err("the every-spring pattern produced no probability".into());
    };

    assert_eq!(forecast.course().as_str(), "M9001.000100");
    assert_eq!(forecast.target_term(), support::spring_2026()?);

    // The existing shape, reused. Not a second window type.
    let metadata = scored.metadata();
    assert_eq!(metadata.version(), PREDICTION_METADATA_VERSION_V1);
    assert_eq!(metadata.positive_sample_count(), 6);
    let disclosed = metadata.observation_window();
    assert!(disclosed.from() < disclosed.to());
    // The window is the span of readings that actually happened, so its lower
    // bound is the oldest same-semester reading in the window.
    assert_eq!(disclosed.from(), read_at(5));
    assert_eq!(disclosed.to(), TimestampMillis::new(read_at(0).value() + 1));

    // The probability is calibrated, and the dataset it went through travels
    // with it.
    assert_eq!(
        scored.calibrated().dataset().as_str(),
        "offering.forecast.synthetic.v1"
    );
    assert!(scored.confidence().value() <= 1000);

    // With no dataset registered at all there is no probability to disclose.
    let (history, window) = support::case("every_spring")?;
    let empty = CalibrationRegistry::new();
    let uncalibrated = forecast_over(&history, window, &empty)?;
    assert!(matches!(
        uncalibrated.verdict(),
        ForecastVerdict::Abstained(AbstentionReason::NoFreshCalibrationDataset)
    ));
    Ok(())
}

fn forecast_over(
    history: &CourseHistory,
    window: ObservationWindow,
    registry: &CalibrationRegistry,
) -> Result<academic_offering::Forecast, Box<dyn std::error::Error>> {
    Ok(forecast(
        history,
        window,
        support::policy()?,
        registry,
        support::now(),
    )?)
}

// ---------------------------------------------------------------------------
// `REQ-08-031`, `REQ-30-002`
// ---------------------------------------------------------------------------

/// An official reading arriving activates a second claim; it never rewrites the
/// prediction.
#[test]
fn prediction_official_parallel() -> TestResult {
    let alone = support::resolve_case("every_spring")?;
    let ForecastVerdict::Scored(scored) = alone.forecast().ok_or("no forecast")?.verdict() else {
        return Err("the baseline produced no probability".into());
    };

    let subject = support::subject()?;
    let prediction = forecast_claim(
        support::claim(6001)?,
        &subject,
        scored,
        vec![support::evidence(6101)?],
    )?;
    assert_eq!(prediction.epistemic_status, EpistemicStatus::Prediction);
    assert_eq!(prediction.authority_class, AuthorityClass::Prediction);
    assert!(prediction.confidence.is_some());
    assert!(prediction.prediction_metadata.is_some());

    let set = OfferingClaimSet::predicted(prediction.clone());
    assert_eq!(
        set.prediction_standing(),
        Some(DecisionStanding::ActiveForDecision)
    );

    // The official claim arrives.
    let evidence = support::confirmation("M9001.000100", Vec::new())?;
    let official = confirmation_claim(
        support::claim(6002)?,
        &subject,
        &evidence,
        vec![support::evidence(6102)?],
    )?;
    assert_eq!(
        official.epistemic_status,
        EpistemicStatus::OfficialConfirmed
    );
    assert_eq!(official.authority_class, AuthorityClass::Official);
    // Section 30.5: an official fact carries no confidence, and `validate`
    // refuses one that does.
    assert!(official.confidence.is_none());
    assert!(official.prediction_metadata.is_none());
    assert_ne!(prediction.id, official.id);

    let after = set.official_arrived(official.clone());
    // Two claims, two statuses, and the prediction is byte-identical.
    assert_eq!(after.prediction(), Some(&prediction));
    assert_eq!(after.official_claim(), Some(&official));
    assert_eq!(
        after.prediction_standing(),
        Some(DecisionStanding::SupersededForDecision { by: official.id })
    );
    assert_eq!(
        after.official_standing(),
        Some(DecisionStanding::ActiveForDecision)
    );
    assert_eq!(
        after.prediction_standing().map(DecisionStanding::as_str),
        Some("SUPERSEDED_FOR_DECISION")
    );

    // And the forecast itself is byte-identical with and without the official
    // reading beside it: the standing moved, the prediction did not.
    let beside = support::resolve_case_with(
        "every_spring",
        &OfficialTermReading::Confirmed(support::confirmation("M9001.000100", Vec::new())?),
    )?;
    assert_eq!(beside.standing().status(), OfferingStatus::Confirmed);
    assert_eq!(
        beside
            .forecast()
            .map(|found| found.canonical_bytes())
            .transpose()?,
        alone
            .forecast()
            .map(|found| found.canonical_bytes())
            .transpose()?
    );
    Ok(())
}

/// ADR-003's actor matrix gives `AuthorityClass::Prediction` to `Actor::ModelRun`
/// alone, so a deterministic forecaster cannot sign its own prediction as a
/// deterministic engine.
///
/// Section 30.1's own example of a `PREDICTION` claim is a *historical
/// pattern*, not a model. This crate does not widen the matrix; it records the
/// divergence here so a later widening is deliberate.
#[test]
fn a_forecast_claim_is_not_signable_by_a_deterministic_engine() -> TestResult {
    let resolution = support::resolve_case("every_spring")?;
    let ForecastVerdict::Scored(scored) = resolution.forecast().ok_or("no forecast")?.verdict()
    else {
        return Err("the baseline produced no probability".into());
    };
    let claim = forecast_claim(
        support::claim(6003)?,
        &support::subject()?,
        scored,
        vec![support::evidence(6103)?],
    )?;

    let engine = Actor::DeterministicEngine {
        name: academic_offering::OFFERING_FORECAST_ENGINE_ID.to_owned(),
        version: academic_offering::OFFERING_FORECAST_ENGINE_VERSION.to_string(),
    };
    assert!(claim.validate_for_actor(&engine).is_err());

    let model_run = Actor::ModelRun {
        run_id: support::entity(6004)?,
    };
    assert!(claim.validate_for_actor(&model_run).is_ok());
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-08-032`
// ---------------------------------------------------------------------------

/// A course nobody has seen run abstains, and no negative official claim is
/// reachable from that abstention.
#[test]
fn zero_observation_semantics() -> TestResult {
    for case in ["never_observed", "spring_only_asked_for_autumn"] {
        let resolution = support::resolve_case(case)?;
        let OfferingStanding::Uncertain(uncertain) = resolution.standing() else {
            return Err(format!("{case} is not UNCERTAIN: {:?}", resolution.standing()).into());
        };
        assert_eq!(
            uncertain.reason(),
            AbstentionReason::NeverObserved,
            "{case}"
        );
        assert!(uncertain.scored().is_none(), "{case}");
        assert!(
            matches!(
                resolution
                    .forecast()
                    .map(academic_offering::Forecast::verdict),
                Some(ForecastVerdict::Abstained(AbstentionReason::NeverObserved))
            ),
            "{case}"
        );
        // Section 8.3: *미개설 확정이 아니다*. The window read four or six
        // terms and found none, and that is still not a claim that the course
        // will not run.
        assert!(
            resolution.standing().status() != OfferingStatus::Cancelled,
            "{case}"
        );
    }

    // The structural half: the disclosure a prediction claim needs cannot be
    // built with a zero positive-sample count, so there is no prediction to
    // relabel.
    let (history, window) = support::case("never_observed")?;
    let vector = FeatureVector::extract(&history, window);
    assert_eq!(vector.positive_samples(), 0);
    assert!(vector.seasonal_terms() > 0);
    let metadata = academic_domain::PredictionMetadata::new(
        academic_domain::PredictionObservationWindow::new(
            TimestampMillis::new(0),
            TimestampMillis::new(1),
        )?,
        vector.positive_samples(),
    );
    assert!(metadata.is_err());

    // A term nobody read and a term read and empty are different values, and
    // only the second reaches the seasonal rate.
    let unread = CourseHistory::new(support::course_code("M9001.000500")?);
    let unread_vector = FeatureVector::extract(&unread, window);
    assert_eq!(unread_vector.seasonal_terms(), 0);
    assert_ne!(unread_vector.seasonal_terms(), vector.seasonal_terms());
    assert_eq!(
        history
            .observations()
            .filter(|observation| observation.outcome() == Offered::No)
            .count(),
        4
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-08-033`
// ---------------------------------------------------------------------------

/// Per-term Brier score, coverage and abstention rate, each compared against
/// the independent oracle, with the missing outcomes reported.
#[test]
fn term_forecast_metrics() -> TestResult {
    let rows = oracle()?;
    let mut entries = Vec::new();
    for case in corpus::CASES {
        let resolution = support::resolve_case(case)?;
        let forecast = resolution.forecast().ok_or("no forecast")?;
        entries.push(EvaluationEntry::from_forecast(
            forecast,
            corpus::realized(case),
        ));
    }
    let evaluation = TermEvaluation::new(support::spring_2026()?, entries)?;
    let measured = evaluation.measure();

    assert_eq!(
        measured.total().to_string(),
        expect(&rows, "metrics.total")?
    );
    assert_eq!(
        measured.scored().to_string(),
        expect(&rows, "metrics.scored")?
    );
    assert_eq!(
        measured.abstained().to_string(),
        expect(&rows, "metrics.abstained")?
    );
    assert_eq!(
        measured.resolved().to_string(),
        expect(&rows, "metrics.resolved")?
    );
    assert_eq!(
        measured.abstention_permille().to_string(),
        expect(&rows, "metrics.abstention_permille")?
    );
    assert_eq!(
        measured.coverage_permille().to_string(),
        expect(&rows, "metrics.coverage_permille")?
    );
    assert_eq!(
        measured.brier_numerator().map(|value| value.to_string()),
        Some(expect(&rows, "metrics.brier_numerator")?)
    );
    assert_eq!(
        measured.brier_denominator().map(|value| value.to_string()),
        Some(expect(&rows, "metrics.brier_denominator")?)
    );
    assert_eq!(
        measured
            .brier_per_million_floor()
            .map(|value| value.to_string()),
        Some(expect(&rows, "metrics.brier_per_million_floor")?)
    );
    assert_eq!(
        measured
            .missing_outcomes()
            .iter()
            .map(|code| code.as_str().to_owned())
            .collect::<Vec<_>>()
            .join(","),
        expect(&rows, "metrics.missing_outcomes")?
    );

    // Coverage and abstention are not complements: the gap between them is the
    // course nobody checked.
    assert_ne!(
        measured.coverage_permille() + measured.abstention_permille(),
        1000
    );

    // A term in which nothing resolved has no Brier score, rather than a
    // perfect one.
    let unresolved = TermEvaluation::new(
        support::spring_2026()?,
        vec![EvaluationEntry::from_forecast(
            support::resolve_case("every_spring")?
                .forecast()
                .ok_or("no forecast")?,
            None,
        )],
    )?
    .measure();
    assert_eq!(unresolved.brier_numerator(), None);
    assert_eq!(unresolved.brier_denominator(), None);
    assert_eq!(unresolved.brier_per_million_floor(), None);

    // And an empty evaluation is refused rather than measured as three zeroes.
    assert!(matches!(
        TermEvaluation::new(support::spring_2026()?, Vec::new()),
        Err(OfferingError::EmptyEvaluation)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// `REQ-04-011`, `REQ-APA-011`
// ---------------------------------------------------------------------------

/// A current official listing and a history-only course get different statuses,
/// different types and different words on the screen.
#[test]
fn offering_epistemic_split() -> TestResult {
    let official =
        OfficialTermReading::Confirmed(support::confirmation("M9001.000100", Vec::new())?);
    let confirmed = support::resolve_case_with("every_spring", &official)?;
    let historical = support::resolve_case("every_other_spring")?;

    assert_eq!(confirmed.standing().status(), OfferingStatus::Confirmed);
    assert_eq!(
        historical.standing().status(),
        OfferingStatus::HistoricallyLikely
    );
    assert_ne!(
        confirmed.standing().ui_copy(),
        historical.standing().ui_copy()
    );
    assert_ne!(
        confirmed.standing().planner_treatment(),
        historical.standing().planner_treatment()
    );

    // Different types, not two labels on one. The confirmed arm carries an
    // evidence value the likely arm has no field for, and the likely arm
    // carries a calibrated probability the confirmed arm has none of.
    let OfferingStanding::Confirmed(confirmed_standing) = confirmed.standing() else {
        return Err("the official reading did not confirm".into());
    };
    let OfferingStanding::HistoricallyLikely(likely_standing) = historical.standing() else {
        return Err("the history-only course is not likely".into());
    };
    assert_eq!(
        confirmed_standing.evidence().basis().source(),
        SourceCategory::RegistrationSystem
    );
    assert!(likely_standing.calibrated().confidence().value() > 0);

    // The four rows are four distinct spellings, four distinct UI strings and
    // four distinct planner treatments.
    let cancelled = support::resolve_case_with(
        "every_spring",
        &OfficialTermReading::Cancelled(support::cancellation("M9001.000100")?),
    )?;
    let uncertain = support::resolve_case("sparse")?;
    let rows = [
        confirmed.standing(),
        historical.standing(),
        uncertain.standing(),
        cancelled.standing(),
    ];
    let statuses: Vec<&str> = rows.iter().map(|row| row.status().as_str()).collect();
    let copies: Vec<&str> = rows.iter().map(|row| row.ui_copy()).collect();
    let treatments: Vec<&str> = rows.iter().map(|row| row.planner_treatment()).collect();
    for list in [&statuses, &copies, &treatments] {
        let mut sorted = list.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 4, "two of section 8.3's rows read the same");
    }
    assert_eq!(
        statuses,
        vec!["CONFIRMED", "HISTORICALLY_LIKELY", "UNCERTAIN", "CANCELLED"]
    );

    // Only one of the four produces a seat.
    assert_eq!(rows.iter().filter(|row| row.seat().is_some()).count(), 1);

    // The proof tree says why: the calibration node is `UNKNOWN` on an
    // abstention and `SATISFIED` on a scored forecast, and `UNKNOWN` is a
    // value rather than a missing key.
    assert_eq!(
        support::node_status(&historical, ".calibration"),
        Some(ProofStatus::Satisfied)
    );
    assert_eq!(
        support::node_status(&uncertain, ".calibration"),
        Some(ProofStatus::Unknown)
    );
    assert_eq!(
        support::node_status(&uncertain, ".window"),
        Some(ProofStatus::NotSatisfied)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The plan prohibition
// ---------------------------------------------------------------------------

/// A likely offering has no seat, so there is no expression that puts one in a
/// determinate plan; only a confirmed one commits.
#[test]
fn historically_likely_cannot_enter_determinate_plan() -> TestResult {
    let likely = support::resolve_case("every_spring")?;
    assert_eq!(
        likely.standing().status(),
        OfferingStatus::HistoricallyLikely
    );
    assert!(likely.standing().seat().is_none());

    let scenario = support::scenario_for("M9001.000100")?;
    let PlanOutcome::Indeterminate(refused) = DeterminatePlan::commit(&scenario, Vec::new()) else {
        return Err("a likely offering committed".into());
    };
    assert_eq!(refused.refusals().len(), 1);
    assert_eq!(
        refused.refusals().first().map(PlanRefusal::course),
        Some("M9001.000100")
    );
    assert_eq!(refused.alternative_paths_required(), vec!["M9001.000100"]);

    // The same scenario with the same course *confirmed* commits, which is the
    // half that stops the refusal above being vacuous.
    let official =
        OfficialTermReading::Confirmed(support::confirmation("M9001.000100", Vec::new())?);
    let confirmed = support::resolve_case_with("every_spring", &official)?;
    let seat = confirmed.standing().seat().ok_or("no seat")?;
    let PlanOutcome::Determinate(plan) = DeterminatePlan::commit(&scenario, vec![seat.clone()])
    else {
        return Err("a confirmed offering was refused".into());
    };
    assert_eq!(plan.seats().len(), 1);
    assert_eq!(
        plan.seats().first().map(|seat| seat.course().as_str()),
        Some("M9001.000100")
    );

    // A seat for another term does not satisfy the choice, and the refusal
    // names both terms.
    let other_scenario = academic_record::plan::PlanScenario::new(
        support::entity(4002)?,
        "plan B",
        vec![academic_record::plan::PlanScenarioChoice::new(
            "M9001.000100",
            TermKey::new(2026, Semester::Fall)?,
        )?],
    )?;
    let PlanOutcome::Indeterminate(wrong_term) =
        DeterminatePlan::commit(&other_scenario, vec![seat])
    else {
        return Err("a seat for another term committed".into());
    };
    assert_eq!(
        wrong_term.refusals().first().map(PlanRefusal::as_str),
        Some("SEAT_FOR_ANOTHER_TERM")
    );

    // Every other standing has the same absence.
    for case in ["sparse", "never_observed", "gap_two"] {
        assert!(
            support::resolve_case(case)?.standing().seat().is_none(),
            "{case} produced a seat"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The one section 38 cell
// ---------------------------------------------------------------------------

/// With no reading recorded, nothing is `CONFIRMED` and no capacity, timetable
/// or instructor is carried over from a term that has passed.
#[test]
fn the_open_gate_holds_every_term() -> TestResult {
    let gate = academic_offering::OpenGate::CurrentTermOfferingFacts;
    assert_eq!(gate.identifier(), "GATE-38-017");
    assert!(!gate.statement().is_empty());

    for case in corpus::CASES {
        let resolution = support::resolve_case(case)?;
        assert_ne!(
            resolution.standing().status(),
            OfferingStatus::Confirmed,
            "{case} reached CONFIRMED with no registration reading"
        );
        assert!(resolution.standing().seat().is_none(), "{case}");
    }
    Ok(())
}

/// A forecast never reads its own term.
#[test]
fn the_window_excludes_the_term_it_forecasts() -> TestResult {
    let target = support::spring_2026()?;
    assert!(matches!(
        ObservationWindow::new(target, target),
        Err(OfferingError::EmptyWindow)
    ));
    let window = corpus::window("every_spring")?;
    assert!(!window.contains(window.to()));
    Ok(())
}

/// The forecast policy has no default, and neither does the verification bound.
#[test]
fn the_recorded_criteria_have_no_default() -> TestResult {
    assert!(ForecastPolicy::new(1001, 3).is_err());
    assert!(ForecastPolicy::new(600, 0).is_err());
    assert!(academic_offering::VerificationRecency::new(0).is_err());
    Ok(())
}
