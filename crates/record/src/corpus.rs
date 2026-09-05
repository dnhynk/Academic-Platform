//! The deterministic synthetic corpus every fixture in this crate is built from.
//!
//! `CONTRIBUTING.md` rule 1 admits only synthetic fixtures and rule 5 admits a
//! golden fixture only through a deterministic builder. This module is that
//! builder. It reads no file, no clock, and no environment: the same call
//! produces the same attempt set, the same identifiers, and therefore the same
//! frozen-input digest on every platform and every run.
//!
//! **Nothing here is a real academic record.** The course codes are shaped like
//! official ones so the identifier rule is exercised, and they name no real
//! course; there is no student number, no name, and no institution. Admission
//! is closed, so no durable path for real data exists in any case.
//!
//! ## What the baseline set is built to separate
//!
//! Each row exists to move exactly one of the quantities section 10 requires a
//! reader to be able to tell apart:
//!
//! | row | what it separates |
//! |---|---|
//! | a `C0` in 2014 spring repeated to `A+` in 2015 spring | the repeat ceiling, and which grade was recognized |
//! | an `S` | earned credit that is not in the denominator |
//! | an `F` | denominator credit that is not earned |
//! | a `W` | neither |
//! | an exchange attempt in 2015 fall | recognized credit outside the average |
//! | a `REGISTERED` attempt in 2026 fall | a registration that raises no actual progress |
//! | one course classified 전선 under `cse` and 일선 under `stat` | multi-major |
//!
//! The resulting cumulative average is `33.9 / 12`, which is exactly `2.825` —
//! a tie at the second decimal digit. That is deliberate. Half-away-from-zero
//! rounding publishes `2.83`; `f64` cannot represent `2.825` and rounds the
//! same expression to `2.82`. The corpus therefore fails loudly if a floating
//! point value ever enters the path.

use academic_domain::{AttemptId, Decimal, EvidenceId};

use crate::{
    CanonicalIdentifier, RecordError,
    attempt::{AttemptHistory, CourseAttempt, RegistrationConfirmation, SettledStatus},
    classify::{ClassificationRule, ClassificationRuleSet, ProgramId, RequirementCategory},
    grade::{GradeSymbol, GradingScheme},
    policy::{
        AttemptOrigin, ExternalGradePolicyRow, PolicyBook, RecognitionDecision, RepeatPolicyRow,
        RepeatRecognition, RuleBook,
    },
    term::TermKey,
};

/// The primary programme in the synthetic corpus.
pub const PRIMARY_PROGRAM: &str = "cse";
/// The additional major in the synthetic corpus.
pub const ADDITIONAL_PROGRAM: &str = "stat";
/// The classification rule set the corpus is classified under.
pub const CLASSIFICATION_RULESET_ID: &str = "synthetic_classification_v1";

/// The repeated course.
pub const COURSE_REPEATED: &str = "M1522.000100";
/// The course classified differently by the two programmes.
pub const COURSE_SHARED: &str = "4190.101";
/// The `S`-graded course.
pub const COURSE_SATISFACTORY: &str = "L0442.000200";
/// The failed course.
pub const COURSE_FAILED: &str = "4190.210";
/// The withdrawn course.
pub const COURSE_WITHDRAWN: &str = "4190.310";
/// The additional major's required course.
pub const COURSE_ADDITIONAL: &str = "326.212";
/// The exchange course.
pub const COURSE_EXCHANGE: &str = "X0001.000100";
/// The course a registration exists for and no grade does.
pub const COURSE_REGISTERED: &str = "4190.408";

/// Builds the nth deterministic synthetic entity identifier.
///
/// A fixed UUIDv7 prefix with the index in the low bytes. No clock and no
/// randomness reach this: the identifier is a formatted constant, and the two
/// `uuid` constructors that would read one are named by the engine-harness
/// source scan rather than here -- that scan matches its API spellings
/// anywhere in a file, prose included, so naming them in a comment would trip
/// it. The scan is stricter than its own comment claims, and the right response
/// is to not spell them.
pub fn synthetic_attempt_id(index: u8) -> Result<AttemptId, RecordError> {
    Ok(format!("01900000-0000-7000-8000-0000000000{index:02x}").parse()?)
}

/// Builds the nth deterministic synthetic evidence identifier.
pub fn synthetic_evidence_id(index: u8) -> Result<EvidenceId, RecordError> {
    Ok(format!("01900000-0000-7000-8000-0000000001{index:02x}").parse()?)
}

fn credits(whole: i128) -> Result<Decimal, RecordError> {
    // Scale one, because an official transcript writes `3.0`. Keeping the
    // spelling the transcript uses is what lets a credit value reconciled at
    // import be compared here without a re-encoding.
    Ok(Decimal::new(whole * 10, 1)?)
}

/// The classification rule set the corpus is classified under.
///
/// `4190.101` is 전선 for `cse` and 일선 for `stat`: one attempt, two
/// categories, which is the whole multi-major case in one row.
pub fn classification_v1() -> Result<ClassificationRuleSet, RecordError> {
    let cse = ProgramId::new(PRIMARY_PROGRAM)?;
    let stat = ProgramId::new(ADDITIONAL_PROGRAM)?;
    ClassificationRuleSet::publish(
        CLASSIFICATION_RULESET_ID,
        vec![
            ClassificationRule {
                rule_id: "rule.cls.001".to_owned(),
                program: cse.clone(),
                course_code: COURSE_REPEATED.to_owned(),
                category: RequirementCategory::MajorRequired,
            },
            ClassificationRule {
                rule_id: "rule.cls.002".to_owned(),
                program: cse.clone(),
                course_code: COURSE_SHARED.to_owned(),
                category: RequirementCategory::MajorElective,
            },
            ClassificationRule {
                rule_id: "rule.cls.003".to_owned(),
                program: stat.clone(),
                course_code: COURSE_SHARED.to_owned(),
                category: RequirementCategory::FreeElective,
            },
            ClassificationRule {
                rule_id: "rule.cls.004".to_owned(),
                program: cse.clone(),
                course_code: COURSE_SATISFACTORY.to_owned(),
                category: RequirementCategory::GeneralElective,
            },
            ClassificationRule {
                rule_id: "rule.cls.005".to_owned(),
                program: cse.clone(),
                course_code: COURSE_FAILED.to_owned(),
                category: RequirementCategory::MajorElective,
            },
            ClassificationRule {
                rule_id: "rule.cls.006".to_owned(),
                program: cse,
                course_code: COURSE_WITHDRAWN.to_owned(),
                category: RequirementCategory::MajorElective,
            },
            ClassificationRule {
                rule_id: "rule.cls.007".to_owned(),
                program: stat,
                course_code: COURSE_ADDITIONAL.to_owned(),
                category: RequirementCategory::MajorRequired,
            },
        ],
    )
}

/// The policy book with a **user-confirmed** repeat-recognition rule.
///
/// [`PolicyBook::published_v1`] leaves recognition `UNKNOWN`, because no
/// official source in this repository states which attempt of a repeat group
/// counts. A user who has confirmed the current original supplies a row like
/// this one; the fixtures use it so a definite average exists to check. The
/// ceiling and its effective term are the published ones and are not synthetic.
pub fn confirmed_policy_v1() -> Result<PolicyBook, RecordError> {
    PolicyBook::new(
        vec![
            RepeatPolicyRow {
                row_id: CanonicalIdentifier::new("repeat.no_ceiling.pre_2015")?,
                effective_from: TermKey::parse("2000_SPRING")?,
                ceiling: None,
                recognition: RepeatRecognition::LatestAttempt,
                citation: "synthetic user-confirmed row. The published notice restricts a repeat \
                           grade from 2015 spring onward and says nothing about the period before, \
                           so `PolicyBook::published_v1` carries no row here at all and an earlier \
                           repeat resolves UNKNOWN. This row exists so the fixtures have a \
                           definite average on both sides of the ceiling's effective term."
                    .to_owned(),
            },
            RepeatPolicyRow {
                row_id: CanonicalIdentifier::new("repeat.ceiling.2015_spring")?,
                effective_from: TermKey::parse("2015_SPRING")?,
                ceiling: Some(GradeSymbol::AZero),
                recognition: RepeatRecognition::LatestAttempt,
                citation: "published ceiling from section 10; the recognition rule is a synthetic \
                           user-confirmed value and is UNKNOWN in the published book"
                    .to_owned(),
            },
        ],
        vec![ExternalGradePolicyRow {
            row_id: CanonicalIdentifier::new("external.excluded.2004_spring")?,
            effective_from: TermKey::parse("2004_SPRING")?,
            excluded_from_average: true,
            citation: "section 10, quoting the 평점환산기준표 유의사항".to_owned(),
        }],
    )
}

/// The same book with the repeat ceiling moved to a later term.
///
/// `repeat_ceiling_effective_date` evaluates the same attempt set under this
/// book and requires the published average to move, which it can only do if the
/// date is a row and not a constant.
pub fn confirmed_policy_ceiling_from(term: &str) -> Result<PolicyBook, RecordError> {
    PolicyBook::new(
        vec![
            RepeatPolicyRow {
                row_id: CanonicalIdentifier::new("repeat.no_ceiling.pre_2015")?,
                effective_from: TermKey::parse("2000_SPRING")?,
                ceiling: None,
                recognition: RepeatRecognition::LatestAttempt,
                citation: "synthetic: the no-ceiling row the baseline book carries".to_owned(),
            },
            RepeatPolicyRow {
                row_id: CanonicalIdentifier::new("repeat.ceiling.moved")?,
                effective_from: TermKey::parse(term)?,
                ceiling: Some(GradeSymbol::AZero),
                recognition: RepeatRecognition::LatestAttempt,
                citation: "synthetic: the published ceiling row with its effective term moved"
                    .to_owned(),
            },
        ],
        vec![ExternalGradePolicyRow {
            row_id: CanonicalIdentifier::new("external.excluded.2004_spring")?,
            effective_from: TermKey::parse("2004_SPRING")?,
            excluded_from_average: true,
            citation: "section 10, quoting the 평점환산기준표 유의사항".to_owned(),
        }],
    )
}

/// The rule book the golden fixtures are evaluated under.
pub fn baseline_rules() -> Result<RuleBook, RecordError> {
    RuleBook::new(
        GradingScheme::snu_4_3_v1()?,
        confirmed_policy_v1()?,
        CLASSIFICATION_RULESET_ID,
    )
}

/// The same rule book publishing three digits instead of two.
pub fn baseline_rules_scale3() -> Result<RuleBook, RecordError> {
    RuleBook::new(
        GradingScheme::snu_4_3_v2_scale3()?,
        confirmed_policy_v1()?,
        CLASSIFICATION_RULESET_ID,
    )
}

/// The rule book whose repeat-recognition rule is the published `UNKNOWN`.
pub fn published_rules() -> Result<RuleBook, RecordError> {
    RuleBook::new(
        GradingScheme::snu_4_3_v1()?,
        PolicyBook::published_v1()?,
        CLASSIFICATION_RULESET_ID,
    )
}

// One private helper with the eight fields a settled corpus row needs. Grouping
// them into a struct would put a second shape of `CourseAttempt` in the crate
// for the corpus builder's convenience, which is worse than the lint.
#[allow(clippy::too_many_arguments)]
fn settled(
    index: u8,
    course_code: &str,
    term: &str,
    status: SettledStatus,
    origin: AttemptOrigin,
    grade: Option<GradeSymbol>,
    attempted: i128,
    earned: i128,
) -> Result<CourseAttempt, RecordError> {
    CourseAttempt::from_confirmed_row(
        synthetic_attempt_id(index)?,
        course_code,
        TermKey::parse(term)?,
        status,
        origin,
        credits(attempted)?,
        credits(earned)?,
        grade,
        GradingScheme::snu_4_3_v1()?.id().to_owned(),
        vec![synthetic_evidence_id(index)?],
    )
}

/// The baseline attempt set.
///
/// Nine attempts across five terms. See the module documentation for what each
/// one is there to separate.
pub fn baseline_history() -> Result<AttemptHistory, RecordError> {
    let mut history = AttemptHistory::new();

    let original = settled(
        1,
        COURSE_REPEATED,
        "2014_SPRING",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(GradeSymbol::CZero),
        3,
        3,
    )?
    .as_original();
    let original_id = original.id();
    history.append(original)?;

    history.append(
        settled(
            2,
            COURSE_REPEATED,
            "2015_SPRING",
            SettledStatus::Completed,
            AttemptOrigin::Internal,
            Some(GradeSymbol::APlus),
            3,
            3,
        )?
        .as_repeat_of(original_id),
    )?;

    history.append(settled(
        3,
        COURSE_SHARED,
        "2014_SPRING",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(GradeSymbol::BPlus),
        3,
        3,
    )?)?;

    history.append(settled(
        4,
        COURSE_SATISFACTORY,
        "2014_SPRING",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(GradeSymbol::S),
        2,
        2,
    )?)?;

    history.append(settled(
        5,
        COURSE_FAILED,
        "2014_FALL",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(GradeSymbol::F),
        3,
        0,
    )?)?;

    history.append(settled(
        6,
        COURSE_WITHDRAWN,
        "2014_FALL",
        SettledStatus::Withdrawn,
        AttemptOrigin::Internal,
        Some(GradeSymbol::W),
        3,
        0,
    )?)?;

    history.append(settled(
        7,
        COURSE_ADDITIONAL,
        "2015_SPRING",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(GradeSymbol::AZero),
        3,
        3,
    )?)?;

    history.append(
        settled(
            8,
            COURSE_EXCHANGE,
            "2015_FALL",
            SettledStatus::Recognized,
            AttemptOrigin::Exchange,
            Some(GradeSymbol::BZero),
            3,
            3,
        )?
        .with_recognition(RecognitionDecision::Recognized),
    )?;

    let confirmation = RegistrationConfirmation::new(
        COURSE_REGISTERED,
        TermKey::parse("2026_FALL")?,
        credits(3)?,
        vec![synthetic_evidence_id(9)?],
    )?;
    history.append(CourseAttempt::from_confirmed_registration(
        synthetic_attempt_id(9)?,
        &confirmation,
        GradingScheme::snu_4_3_v1()?.id().to_owned(),
    )?)?;

    Ok(history)
}

/// The baseline set with the exchange attempt moved before the 2004 row.
///
/// No dated row reaches its term, so its disposition is
/// `EXTERNAL_POLICY_UNKNOWN` and every average over the set is `UNKNOWN`. This
/// is the `adverse/unknown` fixture, and it is a state the shipped rules
/// actually reach rather than one contrived for the directory.
pub fn history_with_undated_external() -> Result<AttemptHistory, RecordError> {
    let mut history = AttemptHistory::new();
    history.append(settled(
        3,
        COURSE_SHARED,
        "2003_FALL",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(GradeSymbol::BPlus),
        3,
        3,
    )?)?;
    history.append(
        settled(
            8,
            COURSE_EXCHANGE,
            "2003_FALL",
            SettledStatus::Recognized,
            AttemptOrigin::Exchange,
            Some(GradeSymbol::BZero),
            3,
            3,
        )?
        .with_recognition(RecognitionDecision::Recognized),
    )?;
    Ok(history)
}

/// Two settled attempts at one course in one term, neither a repeat.
///
/// The record disagrees with itself about what happened, which is what
/// `CONFLICT` means in the proof vocabulary. This is the `adverse/conflict`
/// fixture.
pub fn history_with_conflicting_records() -> Result<AttemptHistory, RecordError> {
    let mut history = AttemptHistory::new();
    history.append(settled(
        3,
        COURSE_SHARED,
        "2014_SPRING",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(GradeSymbol::BPlus),
        3,
        3,
    )?)?;
    history.append(settled(
        4,
        COURSE_SHARED,
        "2014_SPRING",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(GradeSymbol::CZero),
        3,
        3,
    )?)?;
    Ok(history)
}

/// One attempt at three credits with the given grade, and nothing else.
///
/// Used by `snu_grade_mapping_gpa` to drive each symbol through the whole
/// engine rather than reading the table back out of the scheme that declares
/// it.
pub fn single_grade_history(grade: GradeSymbol) -> Result<AttemptHistory, RecordError> {
    let mut history = AttemptHistory::new();
    history.append(settled(
        1,
        COURSE_SHARED,
        "2020_SPRING",
        SettledStatus::Completed,
        AttemptOrigin::Internal,
        Some(grade),
        3,
        if matches!(grade, GradeSymbol::F | GradeSymbol::U | GradeSymbol::W) {
            0
        } else {
            3
        },
    )?)?;
    Ok(history)
}
