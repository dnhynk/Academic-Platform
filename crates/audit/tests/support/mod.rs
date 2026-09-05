//! The deterministic synthetic corpus every `P2-U3` fixture is built from.
//!
//! `CONTRIBUTING.md` rule 1 admits only synthetic fixtures and rule 5 admits a
//! golden fixture only through a deterministic builder. This module is that
//! builder, and it is a **test** module rather than a product one for one
//! reason: a published `RuleSet` needs an `academic_ingestion::PublishedRules`,
//! whose only producer is `P2-U6`'s stage nine, and the documents that pipeline
//! reads are that crate's own test fixtures. Building the corpus in the product
//! tree would have meant either a second document builder or an
//! `academic-ingestion` fixture surface in a product crate.
//!
//! **Nothing here is a real academic record.** The transcript is
//! `academic_record::corpus`'s, reused rather than transcribed: the attempts,
//! the grades, the repeat group and the exchange row are `P2-U4`'s own
//! synthetic corpus, so a change there moves this corpus too and the two cannot
//! drift into disagreeing about one student.
//!
//! ## What the rule set is built to separate
//!
//! | rule | what it exercises |
//! |---|---|
//! | `total_credits` | a credit floor over every recognized category |
//! | `cse_major_total` | the same floor scoped to one category, so the category is load-bearing |
//! | `required_course_set` | a satisfied operand, an operand discharged by an equivalency, and one that is planned only |
//! | `equivalency_shared` | the substitution the third operand is discharged by |
//! | `seminar_choice` | a choice of `n` |
//! | `foreign_language_lectures` | a count over verified language evidence |
//! | `overall_gpa` | the reading `P2-U4` published, compared without dividing |
//! | `major_exclusive` | a ceiling two recognized attempts break, which is `CONFLICT` |
//!
//! Every rule in the baseline set reaches a definite verdict on the baseline
//! transcript, which is what lets `golden/baseline` be `DETERMINATE`. The rules
//! whose applicability no confirmed source states live in
//! [`open_gate_rules`] instead, because a set containing one can never be
//! determinate and a corpus in which nothing is ever determinate would not
//! show that the gate has two sides.

#![allow(dead_code)]

use std::error::Error;

use academic_domain::{
    ArtifactId, ContentDigest, CourseId, CurriculumVersionId, Decimal, EntityId, RequirementSetId,
    TimestampMillis, ValidInterval, engines::ProofStatus,
};
use academic_ingestion::{
    Acquisition, Appropriateness, ConflictCase, ContendingSource, IngestSeq, OfficialDocument,
    Publication, PublishedRules, RunOutcome, detect, stage,
};
use academic_record::{
    attempt::AttemptHistory,
    classify::{ClassificationRuleSet, ProgramId as RecordProgramId},
    corpus as record_corpus,
    plan::{PlanScenario, PlanScenarioChoice},
    policy::RuleBook,
    term::{Semester, TermKey},
};
use academic_requirement::{
    AcademicFacts, AdmissionYear, Applicability, AreaId, AttemptFact,
    AttemptStatus as RuleAttemptStatus, CreditAmount, CreditCategory, DoubleCountingPolicy,
    FixtureCase, GpaScope, InstructionLanguage, LanguageEvidence, OfficialExampleFixtures, Operand,
    ProgramId, RecognitionPolicy, ReviewAttestation, ReviewGate, RuleBody, RuleCandidate, RuleId,
    RuleSet, RuleSetDraft, RuleSetVersion, SyntheticTranscriptFixtures, TermOrdinal, ThesisGrading,
};

use academic_audit::{
    ALL_GPA_ELIGIBLE, AuditFacts, CatalogEntry, CourseFactsIndex, CourseRequirementFacts,
    DegreeMode, ExchangeOrTransfer, GraduationStandard, InstitutionId, RuleSetCatalog,
    RuleSetScope, RuleSourceIndex, RuleSourceSpan, SourceFreshnessPolicy, StudentProfile,
    TranscriptSnapshot, verdict::ConflictReference,
};

pub mod harness;

#[path = "../../../ingestion/tests/support/mod.rs"]
// `P2-U6`'s fixture module is written for that crate's own suite and offers
// more than this one uses, exactly as it does for `P2-U1` and `P2-U2`.
#[allow(dead_code)]
pub mod ingestion_support;

use ingestion_support::{
    BYLAW, CATALOGUE, DocumentFixture, RETRIEVED_AT, body, corpus as ingestion_corpus, manifest,
    permitting_ledger,
};

/// The rule identifier both conflicting documents carry.
///
/// `DocumentFixture::with_extra_rule` takes the identifier verbatim, so the
/// case `detect` opens names a rule the published set really has. A conflict
/// over a rule nobody published would be a case the audit correctly ignores,
/// which is the opposite of what `graduation_conflict_fail_closed` needs to
/// observe.
pub const CONTESTED_RULE: &str = "total_credits";

pub type TestResult = Result<(), Box<dyn Error>>;

/// The connector every fixture document below is collected by.
pub const CONNECTOR: &str = "snu.cse.official";
/// A second connector, so a conflict case has two sides that are really two.
pub const RIVAL_CONNECTOR: &str = "snu.registrar.official";

/// The rule identifiers the fixture official document itself carries.
///
/// `RuleSetDraft::include` refuses a rule whose `source_rule` the published
/// document does not name, so a fixture set can only bind to identifiers this
/// document really has. `CONTESTED_RULE` is one of them, which is what makes
/// `conflict_case`'s case a case about a rule these sets publish rather than a
/// string that happens to match.
pub const DOCUMENT_RULES: [&str; 12] = [
    "cse_major_total",
    "credit_floor",
    "equivalency_shared",
    "external_recognition_cap",
    "foreign_language_lectures",
    "life_respect_training",
    "major_exclusive",
    "overall_gpa",
    "required_course_set",
    "seminar_choice",
    "thesis_research",
    "total_credits",
];

/// The instant every evaluation is anchored to.
///
/// A hundred thousand seconds after `RETRIEVED_AT`, so the source has a
/// definite, small age and the freshness criterion below has something to be
/// about.
pub const AS_OF: TimestampMillis = TimestampMillis::new(1_772_100_000_000);

/// The freshness criterion the fixtures record.
///
/// **Synthetic and user-confirmed.** No source states a number, so the audit
/// has none by default and is `INDETERMINATE` without one; this is a value a
/// user recorded, labelled as such, so a determinate fixture exists to check.
pub const FRESHNESS: SourceFreshnessPolicy = SourceFreshnessPolicy::max_age_seconds(200_000);

/// A criterion the source is older than.
pub const STALE_FRESHNESS: SourceFreshnessPolicy = SourceFreshnessPolicy::max_age_seconds(10);

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

mod uuid_bytes {
    /// The minimal surface `parse_id` needs. `academic-domain` re-exports no
    /// `Uuid`, so identifiers are parsed from their canonical text.
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

pub fn course(suffix: u32) -> Result<CourseId, Box<dyn Error>> {
    Ok(parse_id!(CourseId, suffix)?)
}

pub fn entity(suffix: u32) -> Result<EntityId, Box<dyn Error>> {
    Ok(parse_id!(EntityId, suffix)?)
}

pub fn artifact() -> Result<ArtifactId, Box<dyn Error>> {
    Ok(parse_id!(ArtifactId, 700)?)
}

pub fn set_id() -> Result<RequirementSetId, Box<dyn Error>> {
    Ok(parse_id!(RequirementSetId, 900)?)
}

pub fn curriculum() -> Result<CurriculumVersionId, Box<dyn Error>> {
    Ok(parse_id!(CurriculumVersionId, 901)?)
}

fn reviewer(suffix: u32) -> Result<academic_domain::Actor, Box<dyn Error>> {
    Ok(academic_domain::Actor::User {
        user_id: entity(suffix)?,
    })
}

pub fn source_digest() -> ContentDigest {
    ContentDigest::sha256(b"official/cse/degree-requirements")
}

// ---------------------------------------------------------------------------
// Courses
// ---------------------------------------------------------------------------

/// The durable course identity behind each transcript code, plus the two the
/// transcript has no attempt for.
pub const COURSE_REPEATED: u32 = 1;
pub const COURSE_DATA_STRUCTURES: u32 = 2;
pub const COURSE_SATISFACTORY: u32 = 3;
pub const COURSE_FAILED: u32 = 4;
pub const COURSE_COMPUTING_OVERVIEW: u32 = 5;
pub const COURSE_EXCHANGE: u32 = 6;
pub const COURSE_WITHDRAWN: u32 = 7;
pub const COURSE_REGISTERED: u32 = 8;
/// Named by a rule and never attempted: section 11.3's *planned only* row.
pub const COURSE_ALGORITHMS: u32 = 10;
/// Named by a rule and discharged only through an equivalency.
pub const COURSE_DISCRETE_MATH: u32 = 13;
/// Named by the seminar choice and never attempted.
pub const COURSE_SEMINAR: u32 = 11;
/// Named by the thesis rule.
///
/// Deliberately the course the baseline transcript holds a **completed,
/// credited** attempt at. `GATE-38-012` says an unresolved applicability reads
/// `UNKNOWN` *whatever the record holds, including a completed thesis*, and a
/// thesis rule pointed at a course nobody took would demonstrate the weaker
/// claim that an absent attempt reads unknown.
pub const COURSE_THESIS: u32 = COURSE_SATISFACTORY;

pub fn all_recognized() -> Result<CreditCategory, Box<dyn Error>> {
    Ok(CreditCategory::new("ALL_RECOGNIZED")?)
}

pub fn cse_major() -> Result<CreditCategory, Box<dyn Error>> {
    Ok(CreditCategory::new("CSE_MAJOR")?)
}

pub fn external() -> Result<CreditCategory, Box<dyn Error>> {
    Ok(CreditCategory::new("EXTERNAL_RECOGNIZED")?)
}

/// What the curriculum records about each transcript course.
///
/// Supplied rather than inferred: which durable course a code names, which
/// categories it counts under, which area it sits in and what language it was
/// taught in are all `P2-U1`'s and the offering's, never the record's.
pub fn course_facts() -> Result<CourseFactsIndex, Box<dyn Error>> {
    let general = AreaId::new("GENERAL_EDUCATION")?;
    let index = CourseFactsIndex::new()
        .with(
            record_corpus::COURSE_REPEATED,
            CourseRequirementFacts {
                course: course(COURSE_REPEATED)?,
                categories: vec![all_recognized()?, cse_major()?],
                area: None,
                language: LanguageEvidence::Unverified,
            },
        )
        .with(
            record_corpus::COURSE_SHARED,
            CourseRequirementFacts {
                course: course(COURSE_DATA_STRUCTURES)?,
                categories: vec![all_recognized()?, cse_major()?],
                area: None,
                language: LanguageEvidence::Verified(InstructionLanguage::Foreign),
            },
        )
        .with(
            record_corpus::COURSE_SATISFACTORY,
            CourseRequirementFacts {
                course: course(COURSE_SATISFACTORY)?,
                categories: vec![all_recognized()?],
                area: Some(general.clone()),
                language: LanguageEvidence::Unverified,
            },
        )
        .with(
            record_corpus::COURSE_FAILED,
            CourseRequirementFacts {
                course: course(COURSE_FAILED)?,
                categories: vec![all_recognized()?, cse_major()?],
                area: None,
                language: LanguageEvidence::Unverified,
            },
        )
        .with(
            record_corpus::COURSE_ADDITIONAL,
            CourseRequirementFacts {
                course: course(COURSE_COMPUTING_OVERVIEW)?,
                categories: vec![all_recognized()?],
                area: Some(general),
                language: LanguageEvidence::Verified(InstructionLanguage::Foreign),
            },
        )
        .with(
            record_corpus::COURSE_EXCHANGE,
            CourseRequirementFacts {
                course: course(COURSE_EXCHANGE)?,
                categories: vec![all_recognized()?, external()?],
                area: None,
                language: LanguageEvidence::Unverified,
            },
        )
        .with(
            record_corpus::COURSE_WITHDRAWN,
            CourseRequirementFacts {
                course: course(COURSE_WITHDRAWN)?,
                categories: vec![all_recognized()?, cse_major()?],
                area: None,
                language: LanguageEvidence::Unverified,
            },
        )
        .with(
            record_corpus::COURSE_REGISTERED,
            CourseRequirementFacts {
                course: course(COURSE_REGISTERED)?,
                categories: vec![all_recognized()?, cse_major()?],
                area: None,
                language: LanguageEvidence::Unverified,
            },
        );
    Ok(index)
}

// ---------------------------------------------------------------------------
// The transcript, reused from `P2-U4`
// ---------------------------------------------------------------------------

pub fn classification() -> Result<ClassificationRuleSet, Box<dyn Error>> {
    Ok(record_corpus::classification_v1()?)
}

pub fn record_rules() -> Result<RuleBook, Box<dyn Error>> {
    Ok(record_corpus::baseline_rules()?)
}

pub fn primary_program() -> Result<RecordProgramId, Box<dyn Error>> {
    Ok(RecordProgramId::new(record_corpus::PRIMARY_PROGRAM)?)
}

/// The baseline transcript: `P2-U4`'s own nine-attempt corpus.
pub fn transcript() -> Result<TranscriptSnapshot, Box<dyn Error>> {
    snapshot(&record_corpus::baseline_history()?)
}

/// The transcript whose exchange attempt no dated policy row reaches.
pub fn transcript_with_undated_external() -> Result<TranscriptSnapshot, Box<dyn Error>> {
    snapshot(&record_corpus::history_with_undated_external()?)
}

/// The transcript that holds two settled attempts at one course in one term.
pub fn transcript_with_conflicting_records() -> Result<TranscriptSnapshot, Box<dyn Error>> {
    snapshot(&record_corpus::history_with_conflicting_records()?)
}

pub fn snapshot(history: &AttemptHistory) -> Result<TranscriptSnapshot, Box<dyn Error>> {
    Ok(TranscriptSnapshot::from_record(
        history,
        &classification()?,
        &record_rules()?,
        &primary_program()?,
        &course_facts()?,
    )?)
}

// ---------------------------------------------------------------------------
// The profile and the catalogue
// ---------------------------------------------------------------------------

pub fn university() -> Result<InstitutionId, Box<dyn Error>> {
    Ok(InstitutionId::new("SNU")?)
}

pub fn college() -> Result<InstitutionId, Box<dyn Error>> {
    Ok(InstitutionId::new("CollegeOfEngineering")?)
}

pub fn department() -> Result<InstitutionId, Box<dyn Error>> {
    Ok(InstitutionId::new("CSE")?)
}

pub fn standard() -> Result<GraduationStandard, Box<dyn Error>> {
    Ok(GraduationStandard::new("2026")?)
}

pub fn admission_year() -> Result<AdmissionYear, Box<dyn Error>> {
    Ok(AdmissionYear::new(2026)?)
}

/// A fully recorded profile. Section 11.1's eight inputs, all present.
pub fn profile() -> Result<StudentProfile, Box<dyn Error>> {
    Ok(StudentProfile::unrecorded()
        .with_university(university()?)
        .with_college(college()?)
        .with_department(department()?)
        .with_admission_year(admission_year()?)
        .with_graduation_standard(standard()?)
        .with_degree_mode(DegreeMode::SingleMajor)
        .with_additional_majors(Vec::new())
        .with_exchange_or_transfer(ExchangeOrTransfer::Declared)
        .with_exception_approvals(Vec::new()))
}

/// The scope section 11.1's yaml declares for the published set.
pub fn scope() -> Result<RuleSetScope, Box<dyn Error>> {
    Ok(RuleSetScope::new(
        university()?,
        college()?,
        department()?,
        admission_year()?,
        standard()?,
        standard()?,
        DegreeMode::SingleMajor,
    )?)
}

/// A catalogue holding exactly the one set that covers [`profile`].
pub fn catalog(rules: &RuleSet) -> Result<RuleSetCatalog, Box<dyn Error>> {
    Ok(RuleSetCatalog::new().with(CatalogEntry::new(scope()?, rules.clone())))
}

// ---------------------------------------------------------------------------
// The official source and the published rules
// ---------------------------------------------------------------------------

/// The document `official_source` reads: the dated fixture plus one rule per
/// entry of [`DOCUMENT_RULES`].
///
/// `document_rule_text` reads the rule bodies back out of this same value, so
/// the text a fixture candidate quotes and the text publication digests are one
/// thing rather than two that agree.
pub fn official_document() -> DocumentFixture {
    let mut fixture = DocumentFixture::dated();
    for rule in DOCUMENT_RULES {
        fixture = fixture.with_extra_rule("art-12", rule);
    }
    fixture
}

/// One completed `P2-U6` run's published rules.
///
/// The only route to a `PublishedRules`: the type's fields are private and its
/// producer is that crate's stage nine, which is reachable only from a dated
/// document. A rule set therefore cannot be founded on an
/// `UNSCOPED_OFFICIAL_SOURCE`, and the reuse is executed here rather than
/// asserted.
pub fn official_source() -> Result<PublishedRules, Box<dyn Error>> {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let known = ingestion_corpus()?;
    let fixture = official_document();
    let record = academic_ingestion::run(
        &manifest,
        &ledger,
        &known,
        RETRIEVED_AT,
        Acquisition::Import {
            target: CATALOGUE,
            outcome: body(fixture.bytes(), "\"v1\"")?,
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

fn draft(
    published: &PublishedRules,
    version: RuleSetVersion,
    supersedes: Option<RuleSetVersion>,
) -> Result<RuleSetDraft, Box<dyn Error>> {
    Ok(RuleSetDraft::from_official_source(
        published,
        set_id()?,
        curriculum()?,
        version,
        supersedes,
    ))
}

/// Runs one body through the review gate and admits it to a draft.
///
/// Every rule in every fixture below goes through this, so no audit here
/// evaluates a rule that skipped `P2-U2`'s two-attestation gate.
fn admit(
    draft: RuleSetDraft,
    id: &str,
    body: RuleBody,
    official: Vec<FixtureCase>,
    synthetic: Vec<FixtureCase>,
) -> Result<RuleSetDraft, Box<dyn Error>> {
    admit_as(draft, id, id, body, official, synthetic)
}

/// The official text of one rule of the fixture document.
///
/// Read back out of the document `official_source` publishes rather than
/// transcribed, because `RuleSetDraft::include` compares the digest of a
/// candidate's quoted span against the digest publication carries for the rule
/// it names: a fixture quoting anything else is refused, and a transcription
/// here would be a second copy of the document's text that could drift from it.
pub fn document_rule_text(rule: &str) -> Result<String, Box<dyn Error>> {
    Ok(official_document()
        .rule_text(rule)
        .ok_or_else(|| format!("the fixture document has no rule `{rule}`"))?
        .to_owned())
}

/// The same, with the identifier the official document gives the rule stated
/// separately from the identifier the reviewer chose inside the set.
///
/// The two are different namespaces. Every fixture above happens to spell them
/// alike, which is exactly the coincidence the conflict gate used to read as a
/// binding, so `a_source_conflict_is_applicable_by_the_document_identifier`
/// uses this to publish the same requirement under a different set identifier
/// and requires the conflict to still apply.
pub fn admit_as(
    draft: RuleSetDraft,
    id: &str,
    source_rule: &str,
    body: RuleBody,
    official: Vec<FixtureCase>,
    synthetic: Vec<FixtureCase>,
) -> Result<RuleSetDraft, Box<dyn Error>> {
    let rule = RuleId::new(id)?;
    let document_rule = academic_domain::engines::RuleId::new(source_rule)?;
    let candidate = RuleCandidate::extracted(
        rule.clone(),
        document_rule.clone(),
        body,
        academic_domain::Actor::ModelRun {
            run_id: entity(7_001)?,
        },
        document_rule_text(source_rule)?,
        source_digest(),
    );
    let reviewed = ReviewGate::admit(
        candidate,
        ReviewAttestation::file(reviewer(11)?, rule.clone(), document_rule.clone(), AS_OF),
        ReviewAttestation::file(reviewer(12)?, rule.clone(), document_rule, AS_OF),
    )?;
    let official = OfficialExampleFixtures::new(official, &rule)?;
    let synthetic = SyntheticTranscriptFixtures::new(synthetic, &rule)?;
    Ok(draft.include(reviewed, &official, &synthetic)?)
}

/// A fact set carrying the profile's admission year and nothing else.
fn empty_facts() -> Result<AcademicFacts, Box<dyn Error>> {
    Ok(AcademicFacts::new(AS_OF).with_admission_year(admission_year()?))
}

/// A fact set with one recognized attempt at `course_id`.
fn one_attempt(
    course_id: u32,
    credits: u16,
    categories: Vec<CreditCategory>,
    language: LanguageEvidence,
) -> Result<AcademicFacts, Box<dyn Error>> {
    Ok(empty_facts()?.with_attempt(AttemptFact {
        attempt: entity(5_000 + course_id)?,
        course: course(course_id)?,
        credits: CreditAmount::new(credits)?,
        categories,
        area: None,
        is_major: true,
        term: TermOrdinal::new(1),
        status: RuleAttemptStatus::Completed,
        language,
    }))
}

fn effective() -> ValidInterval {
    ValidInterval::open_ended(TimestampMillis::new(1_700_000_000_000))
}

/// The baseline rule set: eight rules, every one of them definite.
///
/// The order below is the admission order and it is load-bearing in one place:
/// `equivalency_shared` is admitted before `required_course_set`, because
/// `RuleSetDraft::include` evaluates each fixture case against the rules
/// admitted so far and a `COURSE_OR_EQUIVALENT` operand resolves through the
/// equivalencies already in the set.
pub fn baseline_rules() -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    Ok(add_baseline(draft(&published, RuleSetVersion::FIRST, None)?)?.publish()?)
}

/// The baseline eight, plus the three whose official fact is unconfirmed.
///
/// Every one of section 11.3's five leaf readings occurs in one tree over this
/// set and `transcript_with_conflicting_records`, which is what
/// `mixed_proof_tree` needs and what no single-status corpus could show.
pub fn mixed_rules() -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    let draft = add_baseline(draft(&published, RuleSetVersion::FIRST, None)?)?;
    Ok(add_open_gate(draft)?.publish()?)
}

fn add_baseline(mut draft: RuleSetDraft) -> Result<RuleSetDraft, Box<dyn Error>> {
    draft = admit(
        draft,
        "equivalency_shared",
        RuleBody::Equivalency {
            presented: course(COURSE_COMPUTING_OVERVIEW)?,
            counts_for: course(COURSE_DISCRETE_MATH)?,
            effective: effective(),
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::NotSatisfied)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_COMPUTING_OVERVIEW,
                3,
                vec![all_recognized()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Satisfied,
        )],
    )?;

    draft = admit(
        draft,
        "total_credits",
        RuleBody::CreditMinimum {
            category: all_recognized()?,
            threshold: CreditAmount::new(130)?,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_DATA_STRUCTURES,
                3,
                vec![all_recognized()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Needs,
        )],
    )?;

    draft = admit(
        draft,
        "cse_major_total",
        RuleBody::CreditMinimum {
            category: cse_major()?,
            threshold: CreditAmount::new(63)?,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_DATA_STRUCTURES,
                3,
                vec![cse_major()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Needs,
        )],
    )?;

    draft = admit(
        draft,
        "required_course_set",
        RuleBody::AllOf {
            operands: vec![
                Operand {
                    course: course(COURSE_DATA_STRUCTURES)?,
                    equivalent_admitted: false,
                },
                Operand {
                    course: course(COURSE_ALGORITHMS)?,
                    equivalent_admitted: false,
                },
                Operand {
                    course: course(COURSE_DISCRETE_MATH)?,
                    equivalent_admitted: true,
                },
            ],
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::NotSatisfied)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_DATA_STRUCTURES,
                3,
                vec![cse_major()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::NotSatisfied,
        )],
    )?;

    draft = admit(
        draft,
        "seminar_choice",
        RuleBody::AtLeastNOf {
            n: 1,
            operands: vec![
                Operand {
                    course: course(COURSE_SEMINAR)?,
                    equivalent_admitted: false,
                },
                Operand {
                    course: course(COURSE_COMPUTING_OVERVIEW)?,
                    equivalent_admitted: false,
                },
            ],
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_COMPUTING_OVERVIEW,
                3,
                vec![all_recognized()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Satisfied,
        )],
    )?;

    draft = admit(
        draft,
        "foreign_language_lectures",
        RuleBody::LanguageOfInstruction {
            minimum: 3,
            language: InstructionLanguage::Foreign,
            exclusions: Vec::new(),
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_DATA_STRUCTURES,
                3,
                vec![cse_major()?],
                LanguageEvidence::Verified(InstructionLanguage::Foreign),
            )?,
            ProofStatus::Needs,
        )],
    )?;

    draft = admit(
        draft,
        "overall_gpa",
        RuleBody::GpaMinimum {
            scope: GpaScope::new(ALL_GPA_ELIGIBLE)?,
            threshold: Decimal::new(20, 1)?,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Unknown)],
        vec![FixtureCase::new(
            empty_facts()?.with_gpa(
                &GpaScope::new(ALL_GPA_ELIGIBLE)?,
                academic_requirement::GpaReading {
                    weighted_points: Decimal::new(339, 1)?,
                    denominator_credits: 12,
                },
            ),
            ProofStatus::Satisfied,
        )],
    )?;

    draft = admit(
        draft,
        "major_exclusive",
        RuleBody::MutuallyExclusive {
            members: vec![course(COURSE_DATA_STRUCTURES)?, course(COURSE_WITHDRAWN)?],
            policy: DoubleCountingPolicy::AtMost(1),
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Satisfied)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_DATA_STRUCTURES,
                3,
                vec![cse_major()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Satisfied,
        )],
    )?;

    Ok(draft)
}

/// A second published version, superseding the first with a lower floor.
///
/// `historic_audit_replay` needs two versions that differ in what they conclude
/// and are both addressable by their own hash.
pub fn revised_rules() -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    let mut draft = draft(
        &published,
        RuleSetVersion::new(2),
        Some(RuleSetVersion::FIRST),
    )?;
    draft = admit(
        draft,
        "total_credits",
        RuleBody::CreditMinimum {
            category: all_recognized()?,
            threshold: CreditAmount::new(12)?,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_DATA_STRUCTURES,
                3,
                vec![all_recognized()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Needs,
        )],
    )?;
    Ok(draft.publish()?)
}

/// One `total_credits` floor at `threshold`, and nothing else.
///
/// Every other argument -- set id, curriculum version, version, supersession,
/// official source, rule identifier, rule type, source digest and both fixture
/// classes -- is the same for every threshold, so two sets built here differ in
/// the credit threshold and in nothing else. That is the pair
/// `historic_audit_replay` needs: its own `baseline_rules`/`revised_rules` pair
/// differs in the version number, the supersession and the rule list as well,
/// so an `assert_ne!` over their hashes is satisfied without the threshold
/// moving a byte -- which is how a `rule_set_hash` that did not cover any rule
/// body passed that assertion.
pub fn credit_floor_rules(threshold: u16) -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    let draft = admit(
        draft(&published, RuleSetVersion::FIRST, None)?,
        "total_credits",
        RuleBody::CreditMinimum {
            category: all_recognized()?,
            threshold: CreditAmount::new(threshold)?,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
    )?;
    Ok(draft.publish()?)
}

/// One credit floor, with the set-local identifier and the document identifier
/// stated separately.
///
/// The pair `a_source_conflict_is_applicable_by_the_document_identifier` needs:
/// the same requirement, read from the same document rule, published under two
/// different names inside the set.
pub fn credit_floor_named(
    id: &str,
    source_rule: &str,
    threshold: u16,
) -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    let draft = admit_as(
        draft(&published, RuleSetVersion::FIRST, None)?,
        id,
        source_rule,
        RuleBody::CreditMinimum {
            category: all_recognized()?,
            threshold: CreditAmount::new(threshold)?,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
    )?;
    Ok(draft.publish()?)
}

/// The same credit floor, with the document rule the body was **read from**
/// stated separately from the document rule the extraction **claims**.
///
/// `credit_floor_named` quotes the rule it names, which is a truthful
/// publication. This one quotes `read_from` and labels the result `labelled`,
/// so the two differ exactly when the claim is false --
/// `one_body_cannot_be_published_under_every_document_identifier` sweeps every
/// identifier the document carries and needs both cases from one builder.
pub fn credit_floor_read_from(
    labelled: &str,
    read_from: &str,
    threshold: u16,
) -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    let rule = RuleId::new("credit_floor")?;
    let document_rule = academic_domain::engines::RuleId::new(labelled)?;
    let candidate = RuleCandidate::extracted(
        rule.clone(),
        document_rule.clone(),
        RuleBody::CreditMinimum {
            category: all_recognized()?,
            threshold: CreditAmount::new(threshold)?,
        },
        academic_domain::Actor::ModelRun {
            run_id: entity(7_001)?,
        },
        document_rule_text(read_from)?,
        source_digest(),
    );
    let reviewed = ReviewGate::admit(
        candidate,
        ReviewAttestation::file(reviewer(11)?, rule.clone(), document_rule.clone(), AS_OF),
        ReviewAttestation::file(reviewer(12)?, rule.clone(), document_rule, AS_OF),
    )?;
    let official = OfficialExampleFixtures::new(
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        &rule,
    )?;
    let synthetic = SyntheticTranscriptFixtures::new(
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        &rule,
    )?;
    Ok(draft(&published, RuleSetVersion::FIRST, None)?
        .include(reviewed, &official, &synthetic)?
        .publish()?)
}

/// The rules whose applicability or ceiling no confirmed source states.
///
/// Kept out of the baseline set on purpose: a set containing one can never be
/// `DETERMINATE`, so the baseline would never show the gate's other side.
pub fn open_gate_rules() -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    Ok(add_open_gate(draft(&published, RuleSetVersion::FIRST, None)?)?.publish()?)
}

fn add_open_gate(mut draft: RuleSetDraft) -> Result<RuleSetDraft, Box<dyn Error>> {
    draft = admit(
        draft,
        "thesis_research",
        RuleBody::ThesisResearch {
            course: course(COURSE_THESIS)?,
            credits: CreditAmount::new(3)?,
            grading: ThesisGrading::SatisfactoryUnsatisfactory,
            applicability: Applicability::Unknown,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Unknown)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_THESIS,
                3,
                vec![cse_major()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Unknown,
        )],
    )?;

    draft = admit(
        draft,
        "life_respect_training",
        RuleBody::NonCreditTraining {
            program: ProgramId::new("life_respect")?,
            applicability: Applicability::Unknown,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Unknown)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_DATA_STRUCTURES,
                3,
                vec![cse_major()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Unknown,
        )],
    )?;

    draft = admit(
        draft,
        "external_recognition_cap",
        RuleBody::MaximumRecognition {
            category: external()?,
            policy: RecognitionPolicy::Unknown,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Unknown)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_EXCHANGE,
                3,
                vec![external()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Unknown,
        )],
    )?;

    Ok(draft)
}

/// A one-rule set every attempt of the baseline transcript satisfies.
///
/// `determinate_three_gate` needs a 졸업 가능 outcome, and reaching one over a
/// hundred-and-thirty-credit floor would need a synthetic transcript built for
/// no other purpose. The floor here is twelve, which the baseline transcript's
/// fourteen earned credits clears.
pub fn satisfiable_rules() -> Result<RuleSet, Box<dyn Error>> {
    let published = official_source()?;
    let draft = admit(
        draft(&published, RuleSetVersion::FIRST, None)?,
        "total_credits",
        RuleBody::CreditMinimum {
            category: all_recognized()?,
            threshold: CreditAmount::new(12)?,
        },
        vec![FixtureCase::new(empty_facts()?, ProofStatus::Needs)],
        vec![FixtureCase::new(
            one_attempt(
                COURSE_DATA_STRUCTURES,
                3,
                vec![all_recognized()?],
                LanguageEvidence::Unverified,
            )?,
            ProofStatus::Needs,
        )],
    )?;
    Ok(draft.publish()?)
}

// ---------------------------------------------------------------------------
// Source placements
// ---------------------------------------------------------------------------

/// Where each rule of a set was read from.
///
/// One page per rule, numbered in publication order, and one paragraph span per
/// page. Synthetic, and labelled as such: no official document was read.
pub fn sources(rules: &RuleSet) -> Result<RuleSourceIndex, Box<dyn Error>> {
    let mut index = RuleSourceIndex::new();
    for (position, (rule, _)) in rules.rules().enumerate() {
        let page = u32::try_from(position + 1)?;
        let start = u64::from(page) * 1_000;
        index = index.with(
            rule.clone(),
            RuleSourceSpan::new(artifact()?, source_digest(), page, start, start + 400)?,
        );
    }
    Ok(index)
}

/// The same placements with one rule left out.
pub fn sources_missing(rules: &RuleSet, omitted: &str) -> Result<RuleSourceIndex, Box<dyn Error>> {
    let mut index = RuleSourceIndex::new();
    for (position, (rule, _)) in rules.rules().enumerate() {
        if rule.as_str() == omitted {
            continue;
        }
        let page = u32::try_from(position + 1)?;
        let start = u64::from(page) * 1_000;
        index = index.with(
            rule.clone(),
            RuleSourceSpan::new(artifact()?, source_digest(), page, start, start + 400)?,
        );
    }
    Ok(index)
}

// ---------------------------------------------------------------------------
// Conflict cases
// ---------------------------------------------------------------------------

/// Two official documents that really disagree about `total_credits`.
///
/// Built by `academic_ingestion::detect`, so the five dimensions section 8.4
/// compares are compared by the crate that owns them and the case is opened
/// only because the two documents genuinely differ.
pub fn unresolved_conflict() -> Result<ConflictReference, Box<dyn Error>> {
    Ok(ConflictReference::of(&conflict_case()?))
}

pub fn conflict_case() -> Result<ConflictCase, Box<dyn Error>> {
    let left_document =
        parse_document(&DocumentFixture::dated().with_extra_rule("art-12", CONTESTED_RULE))?;
    let right_document = parse_document(
        &DocumentFixture::dated()
            .with_extra_rule("art-12", CONTESTED_RULE)
            .with_rule_text(CONTESTED_RULE, "a different number of credits")
            .issued_by("UNIVERSITY_REGULATION"),
    )?;
    let rule = academic_domain::engines::RuleId::new(CONTESTED_RULE)?;
    let left = ContendingSource::from_document(
        ingestion_support::connector(CONNECTOR)?,
        CATALOGUE,
        &left_document,
        &rule,
    )
    .ok_or("the left fixture document carries no contested rule")?;
    let right = ContendingSource::from_document(
        ingestion_support::connector(RIVAL_CONNECTOR)?,
        BYLAW,
        &right_document,
        &rule,
    )
    .ok_or("the right fixture document carries no contested rule")?;
    detect(left, right).ok_or_else(|| "the two fixture documents do not disagree".into())
}

/// Runs one fixture document through `P2-U6`'s stages one to five.
fn parse_document(fixture: &DocumentFixture) -> Result<OfficialDocument, Box<dyn Error>> {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let fetched = stage::discover_fetch_import(
        &manifest,
        &ledger,
        RETRIEVED_AT,
        Acquisition::Import {
            target: CATALOGUE,
            outcome: body(fixture.bytes(), "\"v1\"")?,
        },
    )?;
    let cleared = stage::policy_and_terms_check(fetched, &manifest, &ledger)?;
    let snapshotted = stage::immutable_raw_snapshot(cleared, &manifest)?;
    let described =
        stage::source_metadata_and_retrieval_time(snapshotted, &manifest, IngestSeq::at(1))?;
    Ok(academic_ingestion::document::parse(
        &described.into_snapshot(),
    )?)
}

// ---------------------------------------------------------------------------
// Audit inputs
// ---------------------------------------------------------------------------

/// The facts one audit reads, with every field explicit.
pub fn audit_facts(
    transcript: TranscriptSnapshot,
    sources: RuleSourceIndex,
    conflicts: Vec<ConflictReference>,
    freshness: Option<SourceFreshnessPolicy>,
) -> Result<AuditFacts, Box<dyn Error>> {
    surveyed_facts(transcript, sources, Some(conflicts), freshness)
}

/// The same, with the conflict survey itself optional.
///
/// `None` is an audit whose conflict store nobody read, which is a different
/// fact from a store that was read and held nothing -- and the one the bare
/// `Vec` above could not spell.
pub fn surveyed_facts(
    transcript: TranscriptSnapshot,
    sources: RuleSourceIndex,
    conflicts: Option<Vec<ConflictReference>>,
    freshness: Option<SourceFreshnessPolicy>,
) -> Result<AuditFacts, Box<dyn Error>> {
    Ok(AuditFacts {
        as_of: AS_OF,
        profile: profile()?,
        transcript,
        sources,
        conflicts,
        freshness,
    })
}

/// The plan that names section 11.3's *planned only* course.
pub fn plan() -> Result<PlanScenario, Box<dyn Error>> {
    Ok(PlanScenario::new(
        entity(6_001)?,
        "2027-1 plan A",
        vec![PlanScenarioChoice::new(
            PLANNED_COURSE_CODE,
            TermKey::new(2027, Semester::Spring)?,
        )?],
    )?)
}

/// The transcript code of the course the plan proposes and the ledger has no
/// attempt for.
pub const PLANNED_COURSE_CODE: &str = "4190.409";
