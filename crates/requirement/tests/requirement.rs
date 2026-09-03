//! `P2-U2`'s named acceptance evidence, less the compile failures and the
//! source scans.
//!
//! `a_candidate_cannot_be_published`, `a_candidate_cannot_be_evaluated` and
//! `an_executable_rule_has_no_public_constructor` are in
//! `tests/compile_fail/`, because each is a statement that a route does not
//! exist and a running test cannot observe an absence.
//! `tests/requirement_scans.rs` holds `production_audit_no_llm` and the half
//! that reads section 11.2's own rule-type list back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`.
//!
//! The official-source fixtures below are `P2-U6`'s own, included by `#[path]`
//! the way `crates/curriculum/tests/curriculum.rs` includes them. A
//! `PublishedRules` has no public constructor, so the only way to obtain the
//! one `RuleSetDraft::from_official_source` takes is to run that crate's
//! pipeline -- which is the point: the reuse is executed here rather than
//! asserted.

#[path = "../../ingestion/tests/support/mod.rs"]
// `P2-U6`'s fixture module is written for that crate's own suite and offers
// more than this one uses, exactly as it does for `P2-U1`.
#[allow(dead_code)]
mod support;

use std::error::Error;

use academic_domain::{
    Actor, ContentDigest, CourseId, CurriculumVersionId, Decimal, EntityId, RequirementSetId,
    TimestampMillis, ValidInterval, engines::ProofStatus,
};
use academic_ingestion::{
    Acquisition, Appropriateness, IngestSeq, Publication, PublishedRules, RunOutcome,
};
use academic_requirement::{
    AcademicFacts, AdmissionYear, Applicability, ApprovalAuthority, ApprovalFact,
    ApprovalRequirement, AreaId, AreaRequirement, AttemptFact, AttemptStatus, CoRequisiteTiming,
    CountConstraint, CreditAmount, CreditCategory, DoubleCountingPolicy, FixtureCase, GpaReading,
    GpaScope, InstructionLanguage, LanguageEvidence, Measure, OfficialExampleFixtures, OpenGate,
    Operand, ProgramId, RecognitionPolicy, RequirementError, ReviewAttestation, ReviewGate,
    RuleBody, RuleCandidate, RuleId, RuleSet, RuleSetDraft, RuleSetLedger, RuleSetVersion,
    RuleType, SyntheticTranscriptFixtures, TermOrdinal, ThesisGrading, TrainingFact,
};
use support::{
    CATALOGUE, DocumentFixture, RETRIEVED_AT, body, corpus, manifest, permitting_ledger,
};

type TestResult = Result<(), Box<dyn Error>>;

const CONNECTOR: &str = "snu.cse.official";

/// The instant every evaluation below is anchored to.
const AT: TimestampMillis = TimestampMillis::new(1_800_000_000_000);
/// An instant before every interval below opens.
const BEFORE: TimestampMillis = TimestampMillis::new(1_700_000_000_000);
/// An instant after every bounded interval below closes.
const AFTER: TimestampMillis = TimestampMillis::new(1_900_000_000_000);

/// `academic-domain` re-exports no `Uuid`, so the identifiers are parsed from
/// their canonical text instead of built from bytes.
mod uuid_bytes {
    /// The minimal surface `parse_id` needs.
    #[derive(Debug, Clone, Copy)]
    pub struct Uuid([u8; 16]);

    impl Uuid {
        #[must_use]
        pub const fn from_bytes(bytes: [u8; 16]) -> Self {
            Self(bytes)
        }

        #[must_use]
        pub fn hyphenated(self) -> String {
            let hex: String = self.0.iter().map(|byte| format!("{byte:02x}")).collect();
            format!(
                "{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32]
            )
        }
    }
}

macro_rules! parse_id {
    ($kind:ty, $suffix:expr) => {{
        let mut bytes = [0_u8; 16];
        bytes[0..8].copy_from_slice(&[0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00]);
        bytes[8] = 0x80;
        bytes[12..16].copy_from_slice(&u32::to_be_bytes($suffix));
        let text = uuid_bytes::Uuid::from_bytes(bytes).hyphenated();
        text.parse::<$kind>()
    }};
}

fn course(suffix: u32) -> Result<CourseId, Box<dyn Error>> {
    Ok(parse_id!(CourseId, suffix)?)
}

fn entity(suffix: u32) -> Result<EntityId, Box<dyn Error>> {
    Ok(parse_id!(EntityId, suffix)?)
}

fn set_id() -> Result<RequirementSetId, Box<dyn Error>> {
    Ok(parse_id!(RequirementSetId, 900)?)
}

fn curriculum() -> Result<CurriculumVersionId, Box<dyn Error>> {
    Ok(parse_id!(CurriculumVersionId, 901)?)
}

fn reviewer(suffix: u32) -> Result<Actor, Box<dyn Error>> {
    Ok(Actor::User {
        user_id: entity(suffix)?,
    })
}

fn digest() -> ContentDigest {
    ContentDigest::sha256(b"official/cse/degree-requirements")
}

/// One completed `P2-U6` run's published rules.
///
/// This is the only route to a `PublishedRules`: the type's fields are private
/// and its producer is that crate's stage nine, which is reachable only from a
/// dated document. A rule set therefore cannot be founded on an
/// `UNSCOPED_OFFICIAL_SOURCE`.
fn official_source() -> Result<PublishedRules, Box<dyn Error>> {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let known = corpus()?;
    let record = academic_ingestion::run(
        &manifest,
        &ledger,
        &known,
        RETRIEVED_AT,
        Acquisition::Import {
            target: CATALOGUE,
            outcome: body(DocumentFixture::dated().bytes(), "\"v1\"")?,
        },
        IngestSeq::at(1),
        Appropriateness::NotAppropriate,
    );
    match record.outcome() {
        RunOutcome::Completed(Publication::Published(rules)) => Ok(rules.clone()),
        RunOutcome::Completed(Publication::Queued(queued)) => {
            Err(format!("the fixture document was queued: {:?}", queued.reason()).into())
        }
        RunOutcome::Halted(failure) => Err(Box::new(failure.clone())),
    }
}

fn draft(published: &PublishedRules) -> Result<RuleSetDraft, Box<dyn Error>> {
    Ok(RuleSetDraft::from_official_source(
        published,
        set_id()?,
        curriculum()?,
        RuleSetVersion::FIRST,
        None,
    ))
}

/// Runs one body through the review gate and admits it to a draft.
///
/// Every rule in every test below goes through this, so no test evaluates a
/// rule that skipped the gate -- the gate is exercised fourteen more times than
/// its own named test exercises it.
fn admit(
    draft: RuleSetDraft,
    id: &str,
    body: RuleBody,
    fixtures: (&[FixtureCase], &[FixtureCase]),
) -> Result<RuleSetDraft, Box<dyn Error>> {
    let rule = RuleId::new(id)?;
    let candidate = RuleCandidate::extracted(
        rule.clone(),
        body,
        Actor::ModelRun {
            run_id: entity(7_001)?,
        },
        "the official page states this requirement in a sentence".to_owned(),
        digest(),
    );
    let reviewed = ReviewGate::admit(
        candidate,
        ReviewAttestation::file(reviewer(11)?, rule.clone(), AT),
        ReviewAttestation::file(reviewer(12)?, rule.clone(), AT),
    )?;
    let official = OfficialExampleFixtures::new(fixtures.0.to_vec(), &rule)?;
    let synthetic = SyntheticTranscriptFixtures::new(fixtures.1.to_vec(), &rule)?;
    Ok(draft.include(reviewed, &official, &synthetic)?)
}

/// Builds a one-rule published set the fixture cases below evaluate against.
fn one_rule_set(
    id: &str,
    body: RuleBody,
    official: &[FixtureCase],
    synthetic: &[FixtureCase],
) -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    let draft = admit(draft(&published)?, id, body, (official, synthetic))?;
    Ok(draft.publish())
}

fn attempt(id: u32, course_id: CourseId, credits: u16) -> Result<AttemptFact, Box<dyn Error>> {
    Ok(AttemptFact {
        attempt: entity(id)?,
        course: course_id,
        credits: CreditAmount::new(credits)?,
        categories: Vec::new(),
        area: None,
        is_major: false,
        term: TermOrdinal::new(1),
        status: AttemptStatus::Completed,
        language: LanguageEvidence::Unverified,
    })
}

fn status_of(
    set: &RuleSet,
    id: &str,
    facts: &AcademicFacts,
) -> Result<ProofStatus, Box<dyn Error>> {
    Ok(set.evaluate(&RuleId::new(id)?, facts)?.status)
}

// ---------------------------------------------------------------------------
// dsl_credit_minimum -- REQ-11-004
// ---------------------------------------------------------------------------

/// Below, equal and above the threshold, with the proof carrying the numerator,
/// the threshold and the category.
///
/// The category half is what makes this more than a sum: an attempt outside the
/// category must not move the numerator, so the third fixture below adds twelve
/// uncategorised credits and requires the reading not to change.
#[test]
fn dsl_credit_minimum() -> TestResult {
    let category = CreditCategory::new("CSE_MAJOR")?;
    let body = RuleBody::CreditMinimum {
        category: category.clone(),
        threshold: CreditAmount::new(63)?,
    };
    let major = |credits: u16, id: u32| -> Result<AttemptFact, Box<dyn Error>> {
        let mut fact = attempt(id, course(id)?, credits)?;
        fact.categories = vec![category.clone()];
        Ok(fact)
    };

    let below = AcademicFacts::new(AT).with_attempt(major(51, 1)?);
    let equal = AcademicFacts::new(AT).with_attempt(major(63, 2)?);
    let above = AcademicFacts::new(AT).with_attempt(major(66, 3)?);
    let outside = AcademicFacts::new(AT)
        .with_attempt(major(63, 4)?)
        .with_attempt(attempt(5, course(5)?, 12)?);

    let set = one_rule_set(
        "cse_major_total",
        body,
        &[FixtureCase::new(equal.clone(), ProofStatus::Satisfied)],
        &[
            FixtureCase::new(below.clone(), ProofStatus::Needs),
            FixtureCase::new(above.clone(), ProofStatus::Satisfied),
        ],
    )?;

    let outcome = set.evaluate(&RuleId::new("cse_major_total")?, &below)?;
    assert_eq!(outcome.status, ProofStatus::Needs);
    assert_eq!(
        outcome.measure,
        Some(Measure::Credits {
            attained: 51,
            required: 63
        }),
        "section 11.3 opens a number into both its halves"
    );
    assert_eq!(outcome.rule_type, RuleType::CreditMinimum);
    assert_eq!(outcome.used_attempts, vec![entity(1)?]);

    assert_eq!(
        status_of(&set, "cse_major_total", &equal)?,
        ProofStatus::Satisfied
    );
    assert_eq!(
        status_of(&set, "cse_major_total", &above)?,
        ProofStatus::Satisfied
    );

    // The category is load-bearing, not decoration: twelve credits outside it
    // leave the numerator exactly where it was.
    let outside_outcome = set.evaluate(&RuleId::new("cse_major_total")?, &outside)?;
    assert_eq!(
        outside_outcome.measure,
        Some(Measure::Credits {
            attained: 63,
            required: 63
        }),
        "an attempt outside the category moved the numerator"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_required_course_set -- REQ-11-005
// ---------------------------------------------------------------------------

/// Direct, equivalent and missing operands, leaf by leaf.
///
/// The equivalent branch is what proves `COURSE_OR_EQUIVALENT` is more than a
/// course reference: the second operand is discharged by a course the student
/// never took, through an `EQUIVALENCY` rule published in the same set, and the
/// leaf names that rule.
#[test]
fn dsl_required_course_set() -> TestResult {
    let discrete = course(101)?;
    let structures = course(102)?;
    let structures_old = course(103)?;

    let body = RuleBody::AllOf {
        operands: vec![
            Operand {
                course: discrete,
                equivalent_admitted: true,
            },
            Operand {
                course: structures,
                equivalent_admitted: true,
            },
        ],
    };

    let both_direct = AcademicFacts::new(AT)
        .with_attempt(attempt(1, discrete, 3)?)
        .with_attempt(attempt(2, structures, 3)?);
    let one_missing = AcademicFacts::new(AT).with_attempt(attempt(1, discrete, 3)?);
    let by_equivalent = AcademicFacts::new(AT)
        .with_attempt(attempt(1, discrete, 3)?)
        .with_attempt(attempt(3, structures_old, 3)?);

    let published = official_source()?;
    let mut set = draft(&published)?;
    set = admit(
        set,
        "structures_equivalency",
        RuleBody::Equivalency {
            presented: structures_old,
            counts_for: structures,
            effective: ValidInterval::open_ended(BEFORE),
        },
        (
            &[FixtureCase::new(
                by_equivalent.clone(),
                ProofStatus::Satisfied,
            )],
            &[FixtureCase::new(
                AcademicFacts::new(AT),
                ProofStatus::NotSatisfied,
            )],
        ),
    )?;
    set = admit(
        set,
        "required_course_set",
        body,
        (
            &[FixtureCase::new(
                both_direct.clone(),
                ProofStatus::Satisfied,
            )],
            &[
                FixtureCase::new(one_missing.clone(), ProofStatus::NotSatisfied),
                FixtureCase::new(by_equivalent.clone(), ProofStatus::Satisfied),
            ],
        ),
    )?;
    let set = set.publish();
    let rule = RuleId::new("required_course_set")?;

    let direct = set.evaluate(&rule, &both_direct)?;
    assert_eq!(direct.status, ProofStatus::Satisfied);
    assert_eq!(
        direct.equivalencies_applied,
        Vec::<RuleId>::new(),
        "a direct attempt applies no equivalency, and the empty list is the decision"
    );

    let missing = set.evaluate(&rule, &one_missing)?;
    assert_eq!(
        missing.status,
        ProofStatus::NotSatisfied,
        "section 11.3 spells a named course that was not taken NOT_SATISFIED"
    );
    assert_eq!(
        missing.measure,
        Some(Measure::Count {
            attained: 1,
            required: 2
        })
    );

    let substituted = set.evaluate(&rule, &by_equivalent)?;
    assert_eq!(substituted.status, ProofStatus::Satisfied);
    assert_eq!(
        substituted.equivalencies_applied,
        vec![RuleId::new("structures_equivalency")?],
        "section 11.3 requires every leaf to carry its equivalency decision"
    );
    assert_eq!(substituted.used_attempts, vec![entity(1)?, entity(3)?]);

    // An operand that does not admit an equivalent is not discharged by one.
    let strict = one_rule_set(
        "strict_set",
        RuleBody::AllOf {
            operands: vec![Operand {
                course: structures,
                equivalent_admitted: false,
            }],
        },
        &[FixtureCase::new(
            AcademicFacts::new(AT).with_attempt(attempt(2, structures, 3)?),
            ProofStatus::Satisfied,
        )],
        &[FixtureCase::new(
            by_equivalent.clone(),
            ProofStatus::NotSatisfied,
        )],
    )?;
    assert_eq!(
        status_of(&strict, "strict_set", &by_equivalent)?,
        ProofStatus::NotSatisfied,
        "an operand that does not admit an equivalent was discharged by one"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_at_least_n -- REQ-11-006
// ---------------------------------------------------------------------------

/// `n - 1`, `n` and `n + 1` satisfied operands, against `n = 2` of three.
#[test]
fn dsl_at_least_n() -> TestResult {
    let seminar = course(201)?;
    let overview = course(202)?;
    let colloquium = course(203)?;
    let body = RuleBody::AtLeastNOf {
        n: 2,
        operands: vec![seminar, overview, colloquium]
            .into_iter()
            .map(|course| Operand {
                course,
                equivalent_admitted: false,
            })
            .collect(),
    };

    let one = AcademicFacts::new(AT).with_attempt(attempt(1, seminar, 1)?);
    let two = one.clone().with_attempt(attempt(2, overview, 1)?);
    let three = two.clone().with_attempt(attempt(3, colloquium, 1)?);

    let set = one_rule_set(
        "seminar_choice",
        body,
        &[FixtureCase::new(two.clone(), ProofStatus::Satisfied)],
        &[
            FixtureCase::new(one.clone(), ProofStatus::Needs),
            FixtureCase::new(three.clone(), ProofStatus::Satisfied),
        ],
    )?;
    let rule = RuleId::new("seminar_choice")?;

    let short = set.evaluate(&rule, &one)?;
    assert_eq!(short.status, ProofStatus::Needs);
    assert_eq!(
        short.measure,
        Some(Measure::Count {
            attained: 1,
            required: 2
        })
    );
    assert_eq!(
        status_of(&set, "seminar_choice", &two)?,
        ProofStatus::Satisfied
    );

    let over = set.evaluate(&rule, &three)?;
    assert_eq!(over.status, ProofStatus::Satisfied);
    assert_eq!(
        over.measure,
        Some(Measure::Count {
            attained: 3,
            required: 2
        }),
        "a satisfied choice rule still reports what it found"
    );

    // A parent satisfied over unsatisfied children is legitimate here, which is
    // exactly why `docs/contracts/engine-harness.md` refuses to impose a fold
    // from children to parent.
    assert_eq!(
        RuleBody::AtLeastNOf {
            n: 4,
            operands: Vec::new()
        }
        .compile(&rule),
        Err(RequirementError::MalformedRule {
            rule: "seminar_choice".to_owned(),
            reason: "AT_LEAST_N_OF asks for more than it offers",
        })
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_count_constraints -- REQ-11-007
// ---------------------------------------------------------------------------

/// Section 8.1's foreign-language matrix: three courses, at least one major,
/// from admission year 2008, with College English excluded from 2012.
///
/// The cohort is varied as well as the counts, because the exclusion is scoped
/// by admission year and a matrix that held the cohort fixed would never
/// observe it.
#[test]
fn dsl_count_constraints() -> TestResult {
    let college_english = course(301)?;
    let major_one = course(302)?;
    let elective = course(303)?;
    let body = RuleBody::CountWithConstraints {
        minimum: 3,
        constraints: vec![
            CountConstraint::AtLeastMajorCourses(1),
            CountConstraint::ExcludedFromAdmissionYear {
                course: college_english,
                from: AdmissionYear::new(2012)?,
            },
        ],
        counted: vec![college_english, major_one, elective],
    };

    let three = |year: Option<u16>| -> Result<AcademicFacts, Box<dyn Error>> {
        let mut major = attempt(2, major_one, 3)?;
        major.is_major = true;
        let facts = AcademicFacts::new(AT)
            .with_attempt(attempt(1, college_english, 3)?)
            .with_attempt(major)
            .with_attempt(attempt(3, elective, 3)?);
        Ok(match year {
            Some(year) => facts.with_admission_year(AdmissionYear::new(year)?),
            None => facts,
        })
    };

    let before_exclusion = three(Some(2011))?;
    let after_exclusion = three(Some(2012))?;
    let no_cohort = three(None)?;
    let no_major = AcademicFacts::new(AT)
        .with_admission_year(AdmissionYear::new(2011)?)
        .with_attempt(attempt(1, college_english, 3)?)
        .with_attempt(attempt(3, elective, 3)?);

    let set = one_rule_set(
        "foreign_language_lectures",
        body,
        &[FixtureCase::new(
            before_exclusion.clone(),
            ProofStatus::Satisfied,
        )],
        &[
            FixtureCase::new(after_exclusion.clone(), ProofStatus::Needs),
            FixtureCase::new(no_major.clone(), ProofStatus::Needs),
            FixtureCase::new(no_cohort.clone(), ProofStatus::Unknown),
        ],
    )?;
    let rule = RuleId::new("foreign_language_lectures")?;

    let met = set.evaluate(&rule, &before_exclusion)?;
    assert_eq!(met.status, ProofStatus::Satisfied);
    assert_eq!(
        met.measure,
        Some(Measure::Count {
            attained: 3,
            required: 3
        })
    );

    // The same three courses, one cohort later: College English stops counting.
    let excluded = set.evaluate(&rule, &after_exclusion)?;
    assert_eq!(excluded.status, ProofStatus::Needs);
    assert_eq!(
        excluded.measure,
        Some(Measure::Count {
            attained: 2,
            required: 3
        }),
        "the 2012 exclusion did not move the count"
    );

    // Three courses and no major course is a shortfall the total does not show.
    assert_eq!(
        status_of(&set, "foreign_language_lectures", &no_major)?,
        ProofStatus::Needs
    );

    // No recorded cohort is GATE-38-011, not a cohort the exclusion misses.
    let unknown = set.evaluate(&rule, &no_cohort)?;
    assert_eq!(unknown.status, ProofStatus::Unknown);
    assert_eq!(unknown.open_gate, Some(OpenGate::CohortApplicability));
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_gpa_minimum -- REQ-11-008
// ---------------------------------------------------------------------------

/// Below, equal and above 2.0, with the exact numerator and denominator on the
/// leaf and an excluded attempt out of the denominator.
#[test]
fn dsl_gpa_minimum() -> TestResult {
    let scope = GpaScope::new("ALL_GPA_ELIGIBLE")?;
    let body = RuleBody::GpaMinimum {
        scope: scope.clone(),
        threshold: Decimal::new(20, 1)?,
    };
    let reading = |points: i128, credits: u32| -> Result<GpaReading, Box<dyn Error>> {
        Ok(GpaReading {
            weighted_points: Decimal::new(points, 1)?,
            denominator_credits: credits,
        })
    };

    // 199/10 over 10 credits is 1.99; 200/10 is exactly 2.0; 250/10 is 2.5.
    let below = AcademicFacts::new(AT).with_gpa(&scope, reading(199, 10)?);
    let equal = AcademicFacts::new(AT).with_gpa(&scope, reading(200, 10)?);
    let above = AcademicFacts::new(AT).with_gpa(&scope, reading(250, 10)?);
    let absent = AcademicFacts::new(AT);
    let empty = AcademicFacts::new(AT).with_gpa(&scope, reading(0, 0)?);

    let set = one_rule_set(
        "overall_gpa",
        body,
        &[FixtureCase::new(equal.clone(), ProofStatus::Satisfied)],
        &[
            FixtureCase::new(below.clone(), ProofStatus::Needs),
            FixtureCase::new(above.clone(), ProofStatus::Satisfied),
            FixtureCase::new(absent.clone(), ProofStatus::Unknown),
        ],
    )?;
    let rule = RuleId::new("overall_gpa")?;

    let short = set.evaluate(&rule, &below)?;
    assert_eq!(short.status, ProofStatus::Needs);
    assert_eq!(
        short.measure,
        Some(Measure::GradePoint {
            weighted_points: Decimal::new(199, 1)?,
            denominator_credits: 10,
            threshold: Decimal::new(20, 1)?,
        }),
        "the leaf must carry the exact reading, not a rounded one"
    );

    // Exactly at the threshold passes. The comparison is a cross-multiplication
    // and never a division, so 200/10 versus 2.0 is an integer identity.
    assert_eq!(
        status_of(&set, "overall_gpa", &equal)?,
        ProofStatus::Satisfied
    );
    assert_eq!(
        status_of(&set, "overall_gpa", &above)?,
        ProofStatus::Satisfied
    );

    // No reading, and a reading over no credits, are both UNKNOWN. An average
    // over an empty denominator is not zero and is not a failure.
    assert_eq!(
        status_of(&set, "overall_gpa", &absent)?,
        ProofStatus::Unknown
    );
    assert_eq!(
        status_of(&set, "overall_gpa", &empty)?,
        ProofStatus::Unknown
    );

    // The threshold's scale does not change the verdict: 2.0 and 2.00 are the
    // same requirement, which a float comparison would not guarantee.
    let wider = one_rule_set(
        "overall_gpa_wide",
        RuleBody::GpaMinimum {
            scope: scope.clone(),
            threshold: Decimal::new(200, 2)?,
        },
        &[FixtureCase::new(equal.clone(), ProofStatus::Satisfied)],
        &[FixtureCase::new(below.clone(), ProofStatus::Needs)],
    )?;
    assert_eq!(
        status_of(&wider, "overall_gpa_wide", &equal)?,
        ProofStatus::Satisfied
    );
    assert_eq!(
        status_of(&wider, "overall_gpa_wide", &below)?,
        ProofStatus::Needs
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_area_distribution -- REQ-11-009
// ---------------------------------------------------------------------------

/// Section 8.1's 2026 general-education areas: the total is met and each area
/// is omitted in turn, and every omission produces its own unmet reading.
#[test]
fn dsl_area_distribution() -> TestResult {
    let areas: Vec<(AreaId, u16)> = vec![
        (AreaId::new("WRITING_AND_SPEAKING")?, 4),
        (AreaId::new("FOREIGN_LANGUAGE")?, 6),
        (AreaId::new("MATHEMATICS")?, 16),
        (AreaId::new("SCIENCE_CHOICE_REQUIRED")?, 8),
        (AreaId::new("COMPUTING")?, 3),
    ];
    let body = RuleBody::AreaDistribution {
        areas: areas
            .iter()
            .map(|(area, credits)| {
                Ok(AreaRequirement {
                    area: area.clone(),
                    credits: CreditAmount::new(*credits)?,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?,
    };

    let facts_with = |skip: Option<usize>| -> Result<AcademicFacts, Box<dyn Error>> {
        let mut facts = AcademicFacts::new(AT);
        for (index, (area, credits)) in areas.iter().enumerate() {
            if Some(index) == skip {
                continue;
            }
            let mut fact = attempt(
                400 + u32::try_from(index)?,
                course(400 + u32::try_from(index)?)?,
                *credits,
            )?;
            fact.area = Some(area.clone());
            facts = facts.with_attempt(fact);
        }
        Ok(facts)
    };

    let complete = facts_with(None)?;
    let missing_first = facts_with(Some(0))?;

    let set = one_rule_set(
        "general_education_areas",
        body,
        &[FixtureCase::new(complete.clone(), ProofStatus::Satisfied)],
        &[FixtureCase::new(missing_first.clone(), ProofStatus::Needs)],
    )?;
    let rule = RuleId::new("general_education_areas")?;

    assert_eq!(
        status_of(&set, "general_education_areas", &complete)?,
        ProofStatus::Satisfied
    );

    // Omit each area in turn. A total that is met says nothing about the
    // distribution, which is the whole reason this rule type exists.
    for index in 0..areas.len() {
        let facts = facts_with(Some(index))?;
        let outcome = set.evaluate(&rule, &facts)?;
        assert_eq!(
            outcome.status,
            ProofStatus::Needs,
            "omitting area {index} left the distribution satisfied"
        );
        assert_eq!(
            outcome.measure,
            Some(Measure::Count {
                attained: u32::try_from(areas.len())? - 1,
                required: u32::try_from(areas.len())?,
            })
        );
    }

    // Credits in the wrong area do not fill the right one, even when the sum
    // over everything is larger than the sum the rule asks for.
    let mut piled = attempt(499, course(499)?, 60)?;
    piled.area = Some(areas[2].0.clone());
    let lopsided = AcademicFacts::new(AT).with_attempt(piled);
    let outcome = set.evaluate(&rule, &lopsided)?;
    assert_eq!(
        outcome.status,
        ProofStatus::Needs,
        "sixty credits in one area satisfied five areas"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_corequisite -- REQ-11-010
// ---------------------------------------------------------------------------

/// Same term, an earlier term, absent, and withdrawn, under both timings.
#[test]
fn dsl_corequisite() -> TestResult {
    let lecture = course(501)?;
    let lab = course(502)?;

    let pair =
        |lab_term: Option<u32>, status: AttemptStatus| -> Result<AcademicFacts, Box<dyn Error>> {
            let mut subject = attempt(1, lecture, 3)?;
            subject.term = TermOrdinal::new(2);
            let mut facts = AcademicFacts::new(AT).with_attempt(subject);
            if let Some(term) = lab_term {
                let mut companion = attempt(2, lab, 1)?;
                companion.term = TermOrdinal::new(term);
                companion.status = status;
                facts = facts.with_attempt(companion);
            }
            Ok(facts)
        };

    let same_term = pair(Some(2), AttemptStatus::Completed)?;
    let earlier = pair(Some(1), AttemptStatus::Completed)?;
    let absent = pair(None, AttemptStatus::Completed)?;
    let withdrawn = pair(Some(2), AttemptStatus::Withdrawn)?;

    let strict = one_rule_set(
        "lab_corequisite",
        RuleBody::CoRequisite {
            subject: lecture,
            companion: lab,
            timing: CoRequisiteTiming::SameTerm,
        },
        &[FixtureCase::new(same_term.clone(), ProofStatus::Satisfied)],
        &[
            FixtureCase::new(earlier.clone(), ProofStatus::NotSatisfied),
            FixtureCase::new(absent.clone(), ProofStatus::NotSatisfied),
            FixtureCase::new(withdrawn.clone(), ProofStatus::NotSatisfied),
        ],
    )?;

    assert_eq!(
        status_of(&strict, "lab_corequisite", &same_term)?,
        ProofStatus::Satisfied
    );
    assert_eq!(
        status_of(&strict, "lab_corequisite", &earlier)?,
        ProofStatus::NotSatisfied,
        "SameTerm admitted an earlier term"
    );
    assert_eq!(
        status_of(&strict, "lab_corequisite", &absent)?,
        ProofStatus::NotSatisfied
    );
    assert_eq!(
        status_of(&strict, "lab_corequisite", &withdrawn)?,
        ProofStatus::NotSatisfied,
        "a withdrawn companion recognized nothing and must not discharge the pair"
    );

    // The looser timing is a different official rule, not a fallback this crate
    // applies when the strict one fails. `REQ-11-010`'s gate candidate is that
    // the semantics come from the source, so both are representable and neither
    // is a default.
    let loose = one_rule_set(
        "lab_corequisite_loose",
        RuleBody::CoRequisite {
            subject: lecture,
            companion: lab,
            timing: CoRequisiteTiming::SameTermOrEarlier,
        },
        &[FixtureCase::new(earlier.clone(), ProofStatus::Satisfied)],
        &[FixtureCase::new(absent.clone(), ProofStatus::NotSatisfied)],
    )?;
    assert_eq!(
        status_of(&loose, "lab_corequisite_loose", &earlier)?,
        ProofStatus::Satisfied
    );
    assert_eq!(
        status_of(&loose, "lab_corequisite_loose", &same_term)?,
        ProofStatus::Satisfied
    );

    // A course is not its own co-requisite.
    assert!(matches!(
        RuleBody::CoRequisite {
            subject: lecture,
            companion: lecture,
            timing: CoRequisiteTiming::SameTerm,
        }
        .compile(&RuleId::new("self_pair")?),
        Err(RequirementError::MalformedRule { .. })
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_mutually_exclusive -- REQ-11-011
// ---------------------------------------------------------------------------

/// Both exclusive attempts present, and the unconfirmed ceiling that is
/// `GATE-38-015`.
#[test]
fn dsl_mutually_exclusive() -> TestResult {
    let modelling = course(601)?;
    let graphics = course(602)?;
    let both = AcademicFacts::new(AT)
        .with_attempt(attempt(1, modelling, 3)?)
        .with_attempt(attempt(2, graphics, 3)?);
    let one = AcademicFacts::new(AT).with_attempt(attempt(1, modelling, 3)?);

    let set = one_rule_set(
        "geometry_exclusion",
        RuleBody::MutuallyExclusive {
            members: vec![modelling, graphics],
            policy: DoubleCountingPolicy::AtMost(1),
        },
        &[FixtureCase::new(one.clone(), ProofStatus::Satisfied)],
        &[FixtureCase::new(both.clone(), ProofStatus::Conflict)],
    )?;
    let rule = RuleId::new("geometry_exclusion")?;

    assert_eq!(
        status_of(&set, "geometry_exclusion", &one)?,
        ProofStatus::Satisfied
    );
    let conflict = set.evaluate(&rule, &both)?;
    assert_eq!(
        conflict.status,
        ProofStatus::Conflict,
        "two recognitions the official rule admits one of is a conflict, not a shortfall"
    );
    assert_eq!(
        conflict.measure,
        Some(Measure::Count {
            attained: 2,
            required: 1
        })
    );
    assert_eq!(
        conflict.used_attempts,
        vec![entity(1)?, entity(2)?],
        "the leaf must name both recognitions that cannot stand together"
    );

    // With no confirmed ceiling the rule is UNKNOWN, and nothing infers one
    // from the member count.
    let open = one_rule_set(
        "double_counting_unconfirmed",
        RuleBody::MutuallyExclusive {
            members: vec![modelling, graphics],
            policy: DoubleCountingPolicy::Unknown,
        },
        &[FixtureCase::new(both.clone(), ProofStatus::Unknown)],
        &[FixtureCase::new(one.clone(), ProofStatus::Unknown)],
    )?;
    let unknown = open.evaluate(&RuleId::new("double_counting_unconfirmed")?, &both)?;
    assert_eq!(unknown.status, ProofStatus::Unknown);
    assert_eq!(unknown.open_gate, Some(OpenGate::MultiMajorDoubleCounting));
    assert_eq!(unknown.measure, None, "an unknown rule measured nothing");
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_equivalency -- REQ-11-012
// ---------------------------------------------------------------------------

/// Directional and effective-dated: `A -> B` does not give `B -> A`, and the
/// substitution holds only inside the interval.
#[test]
fn dsl_equivalency() -> TestResult {
    let old = course(701)?;
    let new = course(702)?;
    let interval = ValidInterval::new(BEFORE, Some(AFTER))?;

    let took_old = AcademicFacts::new(AT).with_attempt(attempt(1, old, 3)?);
    let took_new = AcademicFacts::new(AT).with_attempt(attempt(2, new, 3)?);
    let took_old_late = AcademicFacts::new(AFTER).with_attempt(attempt(1, old, 3)?);

    let published = official_source()?;
    let mut set = draft(&published)?;
    set = admit(
        set,
        "modelling_replaced_by_graphics",
        RuleBody::Equivalency {
            presented: old,
            counts_for: new,
            effective: interval,
        },
        (
            &[FixtureCase::new(took_old.clone(), ProofStatus::Satisfied)],
            &[
                FixtureCase::new(took_new.clone(), ProofStatus::NotSatisfied),
                FixtureCase::new(took_old_late.clone(), ProofStatus::NotSatisfied),
            ],
        ),
    )?;
    set = admit(
        set,
        "requires_new",
        RuleBody::AllOf {
            operands: vec![Operand {
                course: new,
                equivalent_admitted: true,
            }],
        },
        (
            &[FixtureCase::new(took_old.clone(), ProofStatus::Satisfied)],
            &[FixtureCase::new(
                took_old_late.clone(),
                ProofStatus::NotSatisfied,
            )],
        ),
    )?;
    set = admit(
        set,
        "requires_old",
        RuleBody::AllOf {
            operands: vec![Operand {
                course: old,
                equivalent_admitted: true,
            }],
        },
        (
            &[FixtureCase::new(took_old.clone(), ProofStatus::Satisfied)],
            &[FixtureCase::new(
                took_new.clone(),
                ProofStatus::NotSatisfied,
            )],
        ),
    )?;
    let set = set.publish();

    // Forward: the old course is presented for the new one, inside the interval.
    let forward = set.evaluate(&RuleId::new("requires_new")?, &took_old)?;
    assert_eq!(forward.status, ProofStatus::Satisfied);
    assert_eq!(
        forward.equivalencies_applied,
        vec![RuleId::new("modelling_replaced_by_graphics")?]
    );

    // Reverse: the new course does not satisfy a requirement for the old one.
    // `A -> B` is one row and `B -> A` would be a second rule, never a property
    // of this one.
    assert_eq!(
        status_of(&set, "requires_old", &took_new)?,
        ProofStatus::NotSatisfied,
        "the equivalency answered the reverse direction"
    );

    // Outside the interval the substitution is not live, so the same attempt
    // stops discharging the same operand.
    assert_eq!(
        status_of(&set, "requires_new", &took_old_late)?,
        ProofStatus::NotSatisfied,
        "the substitution survived the end of its interval"
    );

    // Both edges of the interval, on the equivalency rule's own verdict.
    let rule = RuleId::new("modelling_replaced_by_graphics")?;
    assert_eq!(
        set.evaluate(
            &rule,
            &AcademicFacts::new(BEFORE).with_attempt(attempt(1, old, 3)?)
        )?
        .status,
        ProofStatus::Satisfied,
        "the interval is closed at its lower bound"
    );
    assert_eq!(
        set.evaluate(
            &rule,
            &AcademicFacts::new(AFTER).with_attempt(attempt(1, old, 3)?)
        )?
        .status,
        ProofStatus::NotSatisfied,
        "the interval is open at its upper bound"
    );

    // A course is not an equivalency for itself.
    assert!(matches!(
        RuleBody::Equivalency {
            presented: old,
            counts_for: old,
            effective: interval,
        }
        .compile(&rule),
        Err(RequirementError::MalformedRule { .. })
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_maximum_recognition -- REQ-11-013
// ---------------------------------------------------------------------------

/// External credits below, at and above the cap, and the unconfirmed cap that
/// is `GATE-38-016`.
#[test]
fn dsl_maximum_recognition() -> TestResult {
    let category = CreditCategory::new("EXTERNAL_TRANSFER")?;
    let external = |credits: u16, id: u32| -> Result<AttemptFact, Box<dyn Error>> {
        let mut fact = attempt(id, course(id)?, credits)?;
        fact.categories = vec![category.clone()];
        Ok(fact)
    };

    let below = AcademicFacts::new(AT).with_attempt(external(20, 1)?);
    let at_cap = AcademicFacts::new(AT).with_attempt(external(30, 2)?);
    let above = AcademicFacts::new(AT).with_attempt(external(45, 3)?);

    let set = one_rule_set(
        "external_recognition_cap",
        RuleBody::MaximumRecognition {
            category: category.clone(),
            policy: RecognitionPolicy::CappedAt(CreditAmount::new(30)?),
        },
        &[FixtureCase::new(at_cap.clone(), ProofStatus::Satisfied)],
        &[
            FixtureCase::new(below.clone(), ProofStatus::Satisfied),
            FixtureCase::new(above.clone(), ProofStatus::Satisfied),
        ],
    )?;
    let rule = RuleId::new("external_recognition_cap")?;

    assert_eq!(
        set.evaluate(&rule, &below)?.measure,
        Some(Measure::Credits {
            attained: 20,
            required: 30
        })
    );
    assert_eq!(
        set.evaluate(&rule, &at_cap)?.measure,
        Some(Measure::Credits {
            attained: 30,
            required: 30
        })
    );
    // Forty-five presented against a thirty cap counts thirty. The excess does
    // not count and does not fail the rule; the leaf shows both halves so the
    // audit can say which credits were excluded and why.
    assert_eq!(
        set.evaluate(&rule, &above)?.measure,
        Some(Measure::Credits {
            attained: 30,
            required: 30
        }),
        "credits above the cap were recognized"
    );

    let open = one_rule_set(
        "external_recognition_unconfirmed",
        RuleBody::MaximumRecognition {
            category,
            policy: RecognitionPolicy::Unknown,
        },
        &[FixtureCase::new(above.clone(), ProofStatus::Unknown)],
        &[FixtureCase::new(below.clone(), ProofStatus::Unknown)],
    )?;
    let unknown = open.evaluate(&RuleId::new("external_recognition_unconfirmed")?, &above)?;
    assert_eq!(unknown.status, ProofStatus::Unknown);
    assert_eq!(unknown.open_gate, Some(OpenGate::ExternalCreditRecognition));
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_noncredit_training -- REQ-11-014
// ---------------------------------------------------------------------------

/// Section 8.1's life-respect education: required from 2016, completed or not,
/// and not applicable before -- with no effect on any credit sum.
#[test]
fn dsl_noncredit_training() -> TestResult {
    let program = ProgramId::new("LIFE_RESPECT_EDUCATION")?;
    let body = RuleBody::NonCreditTraining {
        program: program.clone(),
        applicability: Applicability::FromAdmissionYear(AdmissionYear::new(2016)?),
    };

    let completion = TrainingFact {
        program: program.clone(),
        completed_at: AT,
    };
    let completed = AcademicFacts::new(AT)
        .with_admission_year(AdmissionYear::new(2016)?)
        .with_training(completion.clone());
    let missing = AcademicFacts::new(AT).with_admission_year(AdmissionYear::new(2016)?);
    let earlier_cohort = AcademicFacts::new(AT).with_admission_year(AdmissionYear::new(2015)?);
    let no_cohort = AcademicFacts::new(AT).with_training(completion);

    let set = one_rule_set(
        "life_respect_education",
        body,
        &[FixtureCase::new(completed.clone(), ProofStatus::Satisfied)],
        &[
            FixtureCase::new(missing.clone(), ProofStatus::NotSatisfied),
            FixtureCase::new(earlier_cohort.clone(), ProofStatus::Satisfied),
            FixtureCase::new(no_cohort.clone(), ProofStatus::Unknown),
        ],
    )?;

    assert_eq!(
        status_of(&set, "life_respect_education", &completed)?,
        ProofStatus::Satisfied
    );
    assert_eq!(
        status_of(&set, "life_respect_education", &missing)?,
        ProofStatus::NotSatisfied
    );
    // A 2015 entrant is outside the rule's scope. Not applicable is not a
    // failure and not a credit.
    assert_eq!(
        status_of(&set, "life_respect_education", &earlier_cohort)?,
        ProofStatus::Satisfied
    );
    // With no recorded cohort the rule cannot say, which is GATE-38-011.
    let unknown = set.evaluate(&RuleId::new("life_respect_education")?, &no_cohort)?;
    assert_eq!(unknown.status, ProofStatus::Unknown);
    assert_eq!(unknown.open_gate, Some(OpenGate::CohortApplicability));

    // The rule affects no credit sum: a credit minimum over the same facts
    // reads the same with and without the completion.
    let category = CreditCategory::new("ALL_RECOGNIZED")?;
    let credits = one_rule_set(
        "total_credits",
        RuleBody::CreditMinimum {
            category,
            threshold: CreditAmount::new(130)?,
        },
        &[FixtureCase::new(completed.clone(), ProofStatus::Needs)],
        &[FixtureCase::new(missing.clone(), ProofStatus::Needs)],
    )?;
    let with_training = credits.evaluate(&RuleId::new("total_credits")?, &completed)?;
    let without = credits.evaluate(&RuleId::new("total_credits")?, &missing)?;
    assert_eq!(
        with_training.measure, without.measure,
        "a non-credit completion moved a credit sum"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_language_instruction -- REQ-11-015
// ---------------------------------------------------------------------------

/// Verified and unverified language, with an admission-year exclusion.
#[test]
fn dsl_language_instruction() -> TestResult {
    let excluded_course = course(801)?;
    let body = RuleBody::LanguageOfInstruction {
        minimum: 3,
        language: InstructionLanguage::Foreign,
        exclusions: vec![CountConstraint::ExcludedFromAdmissionYear {
            course: excluded_course,
            from: AdmissionYear::new(2012)?,
        }],
    };

    let taught_in = |id: u32,
                     course_id: CourseId,
                     evidence: LanguageEvidence|
     -> Result<AttemptFact, Box<dyn Error>> {
        let mut fact = attempt(id, course_id, 3)?;
        fact.language = evidence;
        Ok(fact)
    };

    let three_verified = |year: u16| -> Result<AcademicFacts, Box<dyn Error>> {
        Ok(AcademicFacts::new(AT)
            .with_admission_year(AdmissionYear::new(year)?)
            .with_attempt(taught_in(
                1,
                excluded_course,
                LanguageEvidence::Verified(InstructionLanguage::Foreign),
            )?)
            .with_attempt(taught_in(
                2,
                course(802)?,
                LanguageEvidence::Verified(InstructionLanguage::Foreign),
            )?)
            .with_attempt(taught_in(
                3,
                course(803)?,
                LanguageEvidence::Verified(InstructionLanguage::Foreign),
            )?))
    };

    let before = three_verified(2011)?;
    let after = three_verified(2012)?;
    let unverified = AcademicFacts::new(AT)
        .with_admission_year(AdmissionYear::new(2011)?)
        .with_attempt(taught_in(
            2,
            course(802)?,
            LanguageEvidence::Verified(InstructionLanguage::Foreign),
        )?)
        .with_attempt(taught_in(3, course(803)?, LanguageEvidence::Unverified)?)
        .with_attempt(taught_in(
            4,
            course(804)?,
            LanguageEvidence::Verified(InstructionLanguage::Korean),
        )?);

    let set = one_rule_set(
        "foreign_language_instruction",
        body,
        &[FixtureCase::new(before.clone(), ProofStatus::Satisfied)],
        &[
            FixtureCase::new(after.clone(), ProofStatus::Needs),
            FixtureCase::new(unverified.clone(), ProofStatus::Needs),
        ],
    )?;
    let rule = RuleId::new("foreign_language_instruction")?;

    assert_eq!(
        status_of(&set, "foreign_language_instruction", &before)?,
        ProofStatus::Satisfied
    );
    assert_eq!(
        set.evaluate(&rule, &after)?.measure,
        Some(Measure::Count {
            attained: 2,
            required: 3
        }),
        "the 2012 exclusion did not remove the excluded course"
    );

    // An unverified language does not count, and a verified Korean one does
    // not count either. Neither is a negative reading of the requirement.
    let partial = set.evaluate(&rule, &unverified)?;
    assert_eq!(partial.status, ProofStatus::Needs);
    assert_eq!(
        partial.measure,
        Some(Measure::Count {
            attained: 1,
            required: 3
        }),
        "an unverified or wrong-language attempt was counted"
    );
    assert_eq!(partial.used_attempts, vec![entity(2)?]);
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_thesis_research -- REQ-11-016
// ---------------------------------------------------------------------------

/// Section 8.1's 2027-1 thesis research: applicable, inapplicable, and the
/// unresolved cohort that is `GATE-38-012`.
#[test]
fn dsl_thesis_research() -> TestResult {
    let thesis = course(901)?;
    let scoped = RuleBody::ThesisResearch {
        course: thesis,
        credits: CreditAmount::new(3)?,
        grading: ThesisGrading::SatisfactoryUnsatisfactory,
        applicability: Applicability::FromAdmissionYear(AdmissionYear::new(2027)?),
    };

    let completed = AcademicFacts::new(AT)
        .with_admission_year(AdmissionYear::new(2027)?)
        .with_attempt(attempt(1, thesis, 3)?);
    let not_done = AcademicFacts::new(AT).with_admission_year(AdmissionYear::new(2027)?);
    let earlier = AcademicFacts::new(AT).with_admission_year(AdmissionYear::new(2026)?);

    let set = one_rule_set(
        "thesis_research",
        scoped,
        &[FixtureCase::new(completed.clone(), ProofStatus::Satisfied)],
        &[
            FixtureCase::new(not_done.clone(), ProofStatus::NotSatisfied),
            FixtureCase::new(earlier.clone(), ProofStatus::Satisfied),
        ],
    )?;
    assert_eq!(
        status_of(&set, "thesis_research", &completed)?,
        ProofStatus::Satisfied
    );
    assert_eq!(
        status_of(&set, "thesis_research", &not_done)?,
        ProofStatus::NotSatisfied
    );
    assert_eq!(
        status_of(&set, "thesis_research", &earlier)?,
        ProofStatus::Satisfied
    );

    // The scope the specification actually leaves open. Section 8.1: the exact
    // applicability and the transitional arrangement need a departmental notice
    // and an administrative confirmation. Until then the rule says UNKNOWN,
    // whatever the record holds -- including a completed thesis.
    let unresolved = one_rule_set(
        "thesis_research_unresolved",
        RuleBody::ThesisResearch {
            course: thesis,
            credits: CreditAmount::new(3)?,
            grading: ThesisGrading::SatisfactoryUnsatisfactory,
            applicability: Applicability::Unknown,
        },
        &[FixtureCase::new(completed.clone(), ProofStatus::Unknown)],
        &[
            FixtureCase::new(not_done.clone(), ProofStatus::Unknown),
            FixtureCase::new(earlier.clone(), ProofStatus::Unknown),
        ],
    )?;
    let rule = RuleId::new("thesis_research_unresolved")?;
    for facts in [&completed, &not_done, &earlier] {
        let outcome = unresolved.evaluate(&rule, facts)?;
        assert_eq!(
            outcome.status,
            ProofStatus::Unknown,
            "an unresolved thesis scope produced a verdict"
        );
        assert_eq!(outcome.open_gate, Some(OpenGate::ThesisRuleScope));
    }

    // A scoped rule against an unrecorded cohort is the other open cell.
    let no_cohort = AcademicFacts::new(AT).with_attempt(attempt(1, thesis, 3)?);
    let outcome = set.evaluate(&RuleId::new("thesis_research")?, &no_cohort)?;
    assert_eq!(outcome.status, ProofStatus::Unknown);
    assert_eq!(outcome.open_gate, Some(OpenGate::CohortApplicability));
    Ok(())
}

// ---------------------------------------------------------------------------
// dsl_exception_approval -- REQ-11-017
// ---------------------------------------------------------------------------

/// A valid, scoped approval alters only its target leaf; a missing, expired or
/// wrong-authority one alters nothing.
#[test]
fn dsl_exception_approval() -> TestResult {
    let target = RuleId::new("cse_major_total")?;
    let authority = ApprovalAuthority::new("CSE_ADMIN_OFFICE")?;
    let other_authority = ApprovalAuthority::new("STUDENT_UNION")?;
    let category = CreditCategory::new("CSE_MAJOR")?;

    let approval = |rule: &RuleId,
                    authority: &ApprovalAuthority,
                    expires: Option<TimestampMillis>| ApprovalFact {
        rule: rule.clone(),
        authority: authority.clone(),
        issued_at: AT,
        expires_at: expires,
    };

    let valid = AcademicFacts::new(AT).with_approval(approval(&target, &authority, None));
    let none = AcademicFacts::new(AT);
    let expired = AcademicFacts::new(AT).with_approval(approval(&target, &authority, Some(BEFORE)));
    let wrong_authority =
        AcademicFacts::new(AT).with_approval(approval(&target, &other_authority, None));
    let wrong_rule = AcademicFacts::new(AT).with_approval(approval(
        &RuleId::new("overall_gpa")?,
        &authority,
        None,
    ));

    let published = official_source()?;
    let mut set = draft(&published)?;
    set = admit(
        set,
        "cse_major_total",
        RuleBody::CreditMinimum {
            category,
            threshold: CreditAmount::new(63)?,
        },
        (
            &[FixtureCase::new(valid.clone(), ProofStatus::Needs)],
            &[FixtureCase::new(none.clone(), ProofStatus::Needs)],
        ),
    )?;
    set = admit(
        set,
        "major_total_exception",
        RuleBody::ExceptionApproval {
            target: target.clone(),
            approval: ApprovalRequirement {
                authority: authority.clone(),
                valid_within: ValidInterval::new(BEFORE, Some(AFTER))?,
            },
        },
        (
            &[FixtureCase::new(valid.clone(), ProofStatus::Satisfied)],
            &[
                FixtureCase::new(none.clone(), ProofStatus::NotSatisfied),
                FixtureCase::new(expired.clone(), ProofStatus::NotSatisfied),
                FixtureCase::new(wrong_authority.clone(), ProofStatus::NotSatisfied),
                FixtureCase::new(wrong_rule.clone(), ProofStatus::NotSatisfied),
            ],
        ),
    )?;
    let set = set.publish();
    let exception = RuleId::new("major_total_exception")?;

    assert_eq!(
        set.evaluate(&exception, &valid)?.status,
        ProofStatus::Satisfied
    );
    for (label, facts) in [
        ("no approval", &none),
        ("an expired approval", &expired),
        ("a wrong-authority approval", &wrong_authority),
        ("an approval for another rule", &wrong_rule),
    ] {
        assert_eq!(
            set.evaluate(&exception, facts)?.status,
            ProofStatus::NotSatisfied,
            "{label} was admitted"
        );
    }

    // Only the target leaf moves. The credit minimum reads the same whether the
    // approval is present or not: an approval is its own leaf, and what an
    // audit does with it is `P2-U3`'s composition, not a silent rewrite here.
    let with_approval = set.evaluate(&target, &valid)?;
    let without = set.evaluate(&target, &none)?;
    assert_eq!(
        with_approval, without,
        "an exception approval silently altered a rule other than its target"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// rule_candidate_review_gate
// ---------------------------------------------------------------------------

/// A model candidate cannot publish or run; a candidate two people attested to
/// becomes executable in a new version.
///
/// The three compile-fail cases hold the half a running test cannot: that there
/// is no expression taking a `RuleCandidate` to anything executable. What is
/// here is the half that *can* run -- every way the gate refuses, and the one
/// way it admits.
#[test]
fn rule_candidate_review_gate() -> TestResult {
    let rule = RuleId::new("total_credits")?;
    let body = RuleBody::CreditMinimum {
        category: CreditCategory::new("ALL_RECOGNIZED")?,
        threshold: CreditAmount::new(130)?,
    };
    let candidate = || -> Result<RuleCandidate, Box<dyn Error>> {
        Ok(RuleCandidate::extracted(
            rule.clone(),
            body.clone(),
            Actor::ModelRun {
                run_id: entity(7_001)?,
            },
            "the page says at least 130 credits".to_owned(),
            digest(),
        ))
    };

    // One person twice is one review recorded twice.
    assert_eq!(
        ReviewGate::admit(
            candidate()?,
            ReviewAttestation::file(reviewer(11)?, rule.clone(), AT),
            ReviewAttestation::file(reviewer(11)?, rule.clone(), AT),
        ),
        Err(RequirementError::OneReviewerTwice)
    );

    // A model attesting to a model's candidate is the gate reviewing itself.
    for actor in [
        Actor::ModelRun {
            run_id: entity(7_002)?,
        },
        Actor::DeterministicEngine {
            name: "audit".to_owned(),
            version: "1".to_owned(),
        },
        Actor::Importer {
            name: "csv".to_owned(),
            version: "1".to_owned(),
        },
    ] {
        assert!(
            matches!(
                ReviewGate::admit(
                    candidate()?,
                    ReviewAttestation::file(actor.clone(), rule.clone(), AT),
                    ReviewAttestation::file(reviewer(12)?, rule.clone(), AT),
                ),
                Err(RequirementError::ReviewerIsNotAUser { .. })
            ),
            "{} was admitted as a reviewer",
            actor.kind_name()
        );
    }

    // An attestation filed against another candidate does not carry over.
    assert!(matches!(
        ReviewGate::admit(
            candidate()?,
            ReviewAttestation::file(reviewer(11)?, RuleId::new("other_rule")?, AT),
            ReviewAttestation::file(reviewer(12)?, rule.clone(), AT),
        ),
        Err(RequirementError::AttestationNamesAnotherCandidate { .. })
    ));

    // A malformed body is refused at the gate, not at evaluation: a rule that
    // reached a proof tree and then said it could not be read would be an audit
    // that failed halfway.
    assert!(matches!(
        ReviewGate::admit(
            RuleCandidate::extracted(
                rule.clone(),
                RuleBody::AllOf {
                    operands: Vec::new()
                },
                Actor::ModelRun {
                    run_id: entity(7_003)?
                },
                "the page says something".to_owned(),
                digest(),
            ),
            ReviewAttestation::file(reviewer(11)?, rule.clone(), AT),
            ReviewAttestation::file(reviewer(12)?, rule.clone(), AT),
        ),
        Err(RequirementError::MalformedRule { .. })
    ));

    // Two different people, both users, both naming this candidate: admitted,
    // and the reviewed rule carries both attestations.
    let reviewed = ReviewGate::admit(
        candidate()?,
        ReviewAttestation::file(reviewer(11)?, rule.clone(), AT),
        ReviewAttestation::file(reviewer(12)?, rule.clone(), AT),
    )?;
    let (first, second) = reviewed.attestations();
    assert_ne!(
        first.reviewer(),
        second.reviewer(),
        "the gate admitted two attestations from one reviewer"
    );
    assert_eq!(first.candidate(), &rule);
    assert_eq!(second.candidate(), &rule);

    // And the admitted rule executes, which is the other half: the gate is a
    // door, not a wall.
    let facts = AcademicFacts::new(AT);
    let published = official_source()?;
    let set = draft(&published)?
        .include(
            reviewed,
            &OfficialExampleFixtures::new(
                vec![FixtureCase::new(facts.clone(), ProofStatus::Needs)],
                &rule,
            )?,
            &SyntheticTranscriptFixtures::new(
                vec![FixtureCase::new(facts.clone(), ProofStatus::Needs)],
                &rule,
            )?,
        )?
        .publish();
    assert_eq!(set.evaluate(&rule, &facts)?.status, ProofStatus::Needs);
    Ok(())
}

// ---------------------------------------------------------------------------
// ruleset_immutable_publish
// ---------------------------------------------------------------------------

/// Updating a rule leaves the old version's hash and content unchanged, and the
/// new version is linked as its successor.
#[test]
fn ruleset_immutable_publish() -> TestResult {
    let rule = RuleId::new("total_credits")?;
    let category = CreditCategory::new("ALL_RECOGNIZED")?;
    let facts = AcademicFacts::new(AT);
    let published = official_source()?;

    let build = |threshold: u16,
                 version: RuleSetVersion,
                 supersedes: Option<RuleSetVersion>|
     -> Result<RuleSet, Box<dyn Error>> {
        let draft = RuleSetDraft::from_official_source(
            &published,
            set_id()?,
            curriculum()?,
            version,
            supersedes,
        );
        Ok(admit(
            draft,
            "total_credits",
            RuleBody::CreditMinimum {
                category: category.clone(),
                threshold: CreditAmount::new(threshold)?,
            },
            (
                &[FixtureCase::new(facts.clone(), ProofStatus::Needs)],
                &[FixtureCase::new(facts.clone(), ProofStatus::Needs)],
            ),
        )?
        .publish())
    };

    let first = build(130, RuleSetVersion::FIRST, None)?;
    let first_hash = first.rule_set_hash();
    let first_text = first.canonical_text();

    let mut ledger = RuleSetLedger::new();
    ledger.publish(first.clone())?;

    let second = build(132, RuleSetVersion::new(2), Some(RuleSetVersion::FIRST))?;
    let second_hash = second.rule_set_hash();
    ledger.publish(second)?;

    // The old version is still there, unchanged, addressable by its own hash.
    let stored = ledger
        .version(RuleSetVersion::FIRST)
        .ok_or("the first version left the ledger")?;
    assert_eq!(
        stored, &first,
        "publishing a successor changed the predecessor"
    );
    assert_eq!(stored.rule_set_hash(), first_hash);
    assert_eq!(stored.canonical_text(), first_text);
    assert_eq!(
        ledger.by_hash(first_hash).map(RuleSet::version),
        Some(RuleSetVersion::FIRST),
        "a historical audit could not replay by rule hash"
    );

    // A changed rule is a changed hash. Without this the first assertion would
    // pass on a hash that ignored the rules.
    assert_ne!(
        first_hash, second_hash,
        "two versions with different thresholds hashed alike"
    );
    assert_eq!(
        ledger.current().map(RuleSet::version),
        Some(RuleSetVersion::new(2))
    );
    assert_eq!(
        ledger.current().and_then(RuleSet::supersedes),
        Some(RuleSetVersion::FIRST),
        "the successor does not name what it replaces"
    );

    // Republishing a version number is the edit section 11.4 forbids.
    assert!(matches!(
        ledger.publish(build(140, RuleSetVersion::FIRST, None)?),
        Err(RequirementError::VersionAlreadyPublished { .. })
    ));

    // So is superseding anything but the head, which would fork the history a
    // replay walks.
    assert!(matches!(
        ledger.publish(build(
            140,
            RuleSetVersion::new(3),
            Some(RuleSetVersion::FIRST)
        )?),
        Err(RequirementError::SupersedesTheWrongVersion { .. })
    ));

    // And the impacted-rule report is over the published sets, in both
    // directions.
    assert_eq!(
        ledger.changed_rules(RuleSetVersion::FIRST, RuleSetVersion::new(2)),
        vec![rule.clone()],
        "the threshold change was not reported as an impacted rule"
    );

    // A set is immutable as a value too: the same content hashes the same
    // however it was built, so a hash is a function of the set and not of the
    // order the draft was filled in.
    let rebuilt = build(130, RuleSetVersion::FIRST, None)?;
    assert_eq!(rebuilt.rule_set_hash(), first_hash);
    Ok(())
}

// ---------------------------------------------------------------------------
// new_rule_release_gate_requires_official_and_synthetic_fixtures
// ---------------------------------------------------------------------------

/// Section 11.4: *새 rule은 공식 예시와 synthetic transcript fixture로 회귀
/// 검증한다*.
///
/// Both classes are two parameters, so a release with one of them is not a call
/// that can be written. What is testable is that an *empty* class is refused,
/// that a fixture disagreeing with the rule is refused, and that both present
/// and agreeing admits the rule.
#[test]
fn new_rule_release_gate_requires_official_and_synthetic_fixtures() -> TestResult {
    let rule = RuleId::new("total_credits")?;
    let body = RuleBody::CreditMinimum {
        category: CreditCategory::new("ALL_RECOGNIZED")?,
        threshold: CreditAmount::new(130)?,
    };
    let facts = AcademicFacts::new(AT);
    let reviewed = || -> Result<_, Box<dyn Error>> {
        Ok(ReviewGate::admit(
            RuleCandidate::extracted(
                rule.clone(),
                body.clone(),
                Actor::ModelRun {
                    run_id: entity(7_001)?,
                },
                "the page says at least 130 credits".to_owned(),
                digest(),
            ),
            ReviewAttestation::file(reviewer(11)?, rule.clone(), AT),
            ReviewAttestation::file(reviewer(12)?, rule.clone(), AT),
        )?)
    };

    // An empty class is a class that proves nothing, and it is refused at
    // construction rather than looped over.
    assert!(matches!(
        OfficialExampleFixtures::new(Vec::new(), &rule),
        Err(RequirementError::ReleaseFixturesMissing {
            missing: "no official example fixture",
            ..
        })
    ));
    assert!(matches!(
        SyntheticTranscriptFixtures::new(Vec::new(), &rule),
        Err(RequirementError::ReleaseFixturesMissing {
            missing: "no synthetic transcript fixture",
            ..
        })
    ));

    let agreeing = FixtureCase::new(facts.clone(), ProofStatus::Needs);
    let disagreeing = FixtureCase::new(facts.clone(), ProofStatus::Satisfied);
    let published = official_source()?;

    // A fixture that merely exists proves nothing. Each case is evaluated
    // against the rule and has to land where it says it will, so a fixture that
    // disagrees with the rule stops the release.
    for (label, official, synthetic) in [
        (
            "an official example that disagrees",
            vec![disagreeing.clone()],
            vec![agreeing.clone()],
        ),
        (
            "a synthetic transcript that disagrees",
            vec![agreeing.clone()],
            vec![disagreeing.clone()],
        ),
    ] {
        let refused = draft(&published)?.include(
            reviewed()?,
            &OfficialExampleFixtures::new(official, &rule)?,
            &SyntheticTranscriptFixtures::new(synthetic, &rule)?,
        );
        assert!(
            matches!(
                refused,
                Err(RequirementError::ReleaseFixturesMissing {
                    missing: "a regression fixture disagrees with the rule",
                    ..
                })
            ),
            "{label} was released"
        );
    }

    // Both classes present and both agreeing: the rule is admitted and runs.
    let set = draft(&published)?
        .include(
            reviewed()?,
            &OfficialExampleFixtures::new(vec![agreeing.clone()], &rule)?,
            &SyntheticTranscriptFixtures::new(vec![agreeing], &rule)?,
        )?
        .publish();
    assert_eq!(set.evaluate(&rule, &facts)?.status, ProofStatus::Needs);
    Ok(())
}
