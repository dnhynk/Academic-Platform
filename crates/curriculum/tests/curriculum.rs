//! `P2-U1`'s named acceptance evidence, less the three that are compile
//! failures and the ones that are source scans.
//!
//! `course_boundary_rejects_offering_fields`,
//! `revision_boundary_rejects_section_fields` and
//! `offering_boundary_rejects_session_transcript` are in `tests/compile_fail/`,
//! because each is a statement that a field does not exist and a running test
//! cannot observe an absence. `tests/curriculum_scans.rs` holds the half that
//! reads the specification's own field lists back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares them, so
//! a name dropped from the Rust list fails against the specification.
//!
//! The official-source fixtures below are `P2-U6`'s own, included by `#[path]`
//! the way `crates/daemon/tests/phase1_exit.rs` includes the fault driver. A
//! `PublishedRules` has no public constructor, so the only way to obtain the
//! one `CurriculumPublication::from_official_source` takes is to run that
//! crate's pipeline — which is the point: the reuse is executed here rather
//! than asserted.

#[path = "../../ingestion/tests/support/mod.rs"]
// `P2-U6`'s fixture module is written for that crate's own suite and offers
// more than this one uses. A shared module reached by `#[path]` is dead code
// for whatever it is not asked for, exactly as `fault_driver.rs` is in
// `crates/daemon/tests/phase1_exit.rs`, which carries the same allowance.
#[allow(dead_code)]
mod support;

use std::{collections::BTreeSet, error::Error};

use academic_curriculum::{
    AdmissionCohort, CohortTransition, Course, CourseCode, CourseCodeReuse, CourseDraft,
    CourseOffering, CourseOfferingDraft, CourseRelations, CourseRevision, CourseRevisionDraft,
    CourseTitle, Credits, CurriculumCategory, CurriculumError, CurriculumLedger,
    CurriculumPublication, CurriculumPublisher, CurriculumVersion, CurriculumVersionDraft,
    EquivalenceRelation, GradingMode, IdentityDecision, InstructorName, Meeting, OfferingStatus,
    OfficialPrerequisite, OpenGate, PublicationStatus, PublishCheckpoint, PublishFaultInjector,
    RecommendedPrerequisite, ReplacementRelation, RetirementRelation, SectionCode, TermCode,
    TransitionArrangement, Weekday, unknown_readings,
};
use academic_domain::{
    CourseId, CourseRevisionId, CurriculumVersionId, DecisionId, EntityId, OfferingId,
    TimestampMillis, ValidInterval,
};
use academic_ingestion::{
    Acquisition, Appropriateness, IngestSeq, Publication, PublishedRules, RunOutcome,
};
use support::{
    CATALOGUE, DocumentFixture, RETRIEVED_AT, body, corpus, manifest, permitting_ledger,
};

type TestResult = Result<(), Box<dyn Error>>;

const CONNECTOR: &str = "snu.cse.official";

/// The instant every relation question below is asked at.
const AT: TimestampMillis = TimestampMillis::new(1_800_000_000_000);
/// An instant before every interval below opens.
const BEFORE: TimestampMillis = TimestampMillis::new(1_700_000_000_000);
/// An instant after every bounded interval below closes.
const AFTER: TimestampMillis = TimestampMillis::new(1_900_000_000_000);

fn open_interval() -> ValidInterval {
    ValidInterval::open_ended(TimestampMillis::new(1_750_000_000_000))
}

fn bounded_interval() -> Result<ValidInterval, Box<dyn Error>> {
    Ok(ValidInterval::new(
        TimestampMillis::new(1_750_000_000_000),
        Some(TimestampMillis::new(1_850_000_000_000)),
    )?)
}

/// `academic-domain` re-exports no `Uuid`, so the identifiers are parsed from
/// their canonical text instead of built from bytes.
mod uuid_bytes {
    /// The minimal surface `typed_id` needs.
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

fn course_id(suffix: u32) -> Result<CourseId, Box<dyn Error>> {
    Ok(parse_id!(CourseId, suffix)?)
}

fn revision_id(suffix: u32) -> Result<CourseRevisionId, Box<dyn Error>> {
    Ok(parse_id!(CourseRevisionId, suffix)?)
}

fn offering_id(suffix: u32) -> Result<OfferingId, Box<dyn Error>> {
    Ok(parse_id!(OfferingId, suffix)?)
}

fn version_id(suffix: u32) -> Result<CurriculumVersionId, Box<dyn Error>> {
    Ok(parse_id!(CurriculumVersionId, suffix)?)
}

fn decision_id(suffix: u32) -> Result<DecisionId, Box<dyn Error>> {
    Ok(parse_id!(DecisionId, suffix)?)
}

fn entity_id(suffix: u32) -> Result<EntityId, Box<dyn Error>> {
    Ok(parse_id!(EntityId, suffix)?)
}

// ---------------------------------------------------------------------------
// The official source every publication below is founded on
// ---------------------------------------------------------------------------

/// One completed `P2-U6` run's published rules.
///
/// This is the only route to a `PublishedRules`: the type's fields are private
/// and its producer is that crate's stage nine, which is reachable only from a
/// dated document. A `CurriculumPublication` therefore cannot be founded on an
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

fn version(suffix: u32) -> Result<CurriculumVersion, Box<dyn Error>> {
    Ok(CurriculumVersionDraft::new(
        version_id(suffix)?,
        (
            AdmissionCohort::parse("2026")?,
            AdmissionCohort::parse("2026")?,
        ),
        open_interval(),
    )
    .institution_segment("SNU")
    .institution_segment("CollegeOfEngineering")
    .institution_segment("CSE")
    .status(PublicationStatus::OfficialConfirmed)
    .build()?)
}

// ---------------------------------------------------------------------------
// one_course_two_revisions_three_offerings_do_not_leak
// ---------------------------------------------------------------------------

/// Every identity-bearing string one aggregate carries.
///
/// Read through the public accessors, so a field that grew an accessor appears
/// here and a field that has none is invisible to every aggregate including its
/// own — which is the boundary the compile-fail cases hold.
///
/// A *closed vocabulary* value is deliberately not a marker.
/// `CurriculumCategory`, `OfferingStatus` and `GradingMode` each admit a small
/// fixed set of spellings that many aggregates legitimately share — two
/// offerings with no recorded grading mode both read `UNKNOWN`, which is the
/// contract rather than a leak. Those reads are checked instead by
/// `vocabulary_reads_belong_to_their_own_aggregate`, per aggregate and by
/// equality against what the fixture set, so nothing about them is left
/// unchecked; what the marker sweep below carries is the fixture's own chosen
/// text and numbers, which are unique to one aggregate by construction.
fn course_markers(course: &Course) -> BTreeSet<String> {
    BTreeSet::from([course.code().as_str().to_owned()])
}

fn revision_markers(revision: &CourseRevision) -> BTreeSet<String> {
    let mut found = BTreeSet::from([
        revision.code().as_str().to_owned(),
        revision.title().as_str().to_owned(),
        format!("credits:{}", revision.credits().value()),
    ]);
    for entry in revision.official_prerequisites() {
        found.insert(format!("official-prerequisite:{:?}", entry.course()));
    }
    for entry in revision.recommended_prerequisites() {
        found.insert(format!("recommended-prerequisite:{:?}", entry.course()));
    }
    found
}

fn offering_markers(offering: &CourseOffering) -> BTreeSet<String> {
    let mut found = BTreeSet::from([
        offering.term().as_str().to_owned(),
        offering.section().as_str().to_owned(),
    ]);
    for instructor in offering.instructors() {
        found.insert(instructor.as_str().to_owned());
    }
    for meeting in offering.meetings() {
        found.insert(format!("{:?}{}", meeting.weekday(), meeting.from_minute()));
    }
    found
}

/// The subject: one course, two revisions, three offerings. Plus a second
/// course carrying its own revision and offering, because "does not leak" is
/// only observable against something there is to leak from.
struct LeakFixture {
    ledger: CurriculumLedger,
    course_a: CourseId,
    course_b: CourseId,
    revisions_a: [CourseRevisionId; 2],
    revision_b: CourseRevisionId,
    offerings_a: [OfferingId; 3],
    offering_b: OfferingId,
}

fn leak_fixture() -> Result<LeakFixture, Box<dyn Error>> {
    let published = official_source()?;
    let version = version(0x0001)?;
    let version_key = version.id();

    let course_a = course_id(0x0100)?;
    let course_b = course_id(0x0101)?;
    let revisions_a = [revision_id(0x0200)?, revision_id(0x0201)?];
    let revision_b = revision_id(0x0202)?;
    let offerings_a = [
        offering_id(0x0300)?,
        offering_id(0x0301)?,
        offering_id(0x0302)?,
    ];
    let offering_b = offering_id(0x0303)?;

    // Every marker below is unique across the six aggregates, so an attribute
    // that appeared under the wrong aggregate is a value that identifies which
    // one it escaped from.
    let publication = CurriculumPublication::from_official_source(&published, version)
        .with_course(
            CourseDraft::new(course_a, CourseCode::parse("MKA.000001")?)
                .canonical_identity(entity_id(0x0400)?)
                .build()?,
        )
        .with_course(
            CourseDraft::new(course_b, CourseCode::parse("MKB.000002")?)
                .canonical_identity(entity_id(0x0401)?)
                .build()?,
        )
        .with_revision(
            CourseRevisionDraft::new(
                revisions_a[0],
                course_a,
                version_key,
                CourseCode::parse("MKA.000001")?,
                open_interval(),
            )
            .title(CourseTitle::parse("REVAONE")?)
            .credits(Credits::new(3)?)
            .curriculum_category(CurriculumCategory::MajorElective)
            .official_prerequisite(OfficialPrerequisite::on(course_b))
            .build()?,
        )
        .with_revision(
            CourseRevisionDraft::new(
                revisions_a[1],
                course_a,
                version_key,
                CourseCode::parse("MKA.000001")?,
                open_interval(),
            )
            .title(CourseTitle::parse("REVATWO")?)
            .credits(Credits::new(4)?)
            .curriculum_category(CurriculumCategory::MajorRequired)
            .recommended_prerequisite(RecommendedPrerequisite::on(course_b))
            .build()?,
        )
        .with_revision(
            CourseRevisionDraft::new(
                revision_b,
                course_b,
                version_key,
                CourseCode::parse("MKB.000002")?,
                open_interval(),
            )
            .title(CourseTitle::parse("REVBONE")?)
            .credits(Credits::new(5)?)
            .curriculum_category(CurriculumCategory::GeneralStudies)
            .build()?,
        )
        .with_offering(
            CourseOfferingDraft::new(
                offerings_a[0],
                revisions_a[0],
                TermCode::parse("TERMAONE")?,
                SectionCode::parse("SECAONE")?,
                OfferingStatus::Confirmed,
                AT,
            )
            .instructor(InstructorName::parse("InstructorAOne")?)
            .meeting(Meeting::new(Weekday::Monday, 540, 615)?)
            .grading_mode(GradingMode::Letter)
            .build(),
        )
        .with_offering(
            CourseOfferingDraft::new(
                offerings_a[1],
                revisions_a[0],
                TermCode::parse("TERMATWO")?,
                SectionCode::parse("SECATWO")?,
                OfferingStatus::HistoricallyLikely,
                AT,
            )
            .instructor(InstructorName::parse("InstructorATwo")?)
            .meeting(Meeting::new(Weekday::Tuesday, 541, 616)?)
            .build(),
        )
        .with_offering(
            CourseOfferingDraft::new(
                offerings_a[2],
                revisions_a[1],
                TermCode::parse("TERMATRI")?,
                SectionCode::parse("SECATRI")?,
                OfferingStatus::Uncertain,
                AT,
            )
            .instructor(InstructorName::parse("InstructorATri")?)
            .meeting(Meeting::new(Weekday::Wednesday, 542, 617)?)
            .build(),
        )
        .with_offering(
            CourseOfferingDraft::new(
                offering_b,
                revision_b,
                TermCode::parse("TERMBONE")?,
                SectionCode::parse("SECBONE")?,
                OfferingStatus::Cancelled,
                AT,
            )
            .instructor(InstructorName::parse("InstructorBOne")?)
            .meeting(Meeting::new(Weekday::Thursday, 543, 618)?)
            .grading_mode(GradingMode::SatisfactoryUnsatisfactory)
            .build(),
        );

    let mut ledger = CurriculumLedger::new();
    CurriculumPublisher::new().publish(&mut ledger, publication)?;
    Ok(LeakFixture {
        ledger,
        course_a,
        course_b,
        revisions_a,
        revision_b,
        offerings_a,
        offering_b,
    })
}

/// `one_course_two_revisions_three_offerings_do_not_leak`.
///
/// Four directions, each walked over the whole set rather than spot-checked:
/// revision to course, offering to revision, offering to course, and sibling to
/// sibling. The marker sweep is the fifth and it is exhaustive over every
/// ordered pair of the six aggregates, so a leak in a direction nobody named is
/// still a failure.
#[test]
fn one_course_two_revisions_three_offerings_do_not_leak() -> TestResult {
    let fixture = leak_fixture()?;
    let ledger = &fixture.ledger;

    // --- revision -> course -------------------------------------------------
    // Every revision resolves to exactly the course it names, and the course it
    // names carries none of the revision's attributes.
    for revision in ledger.revisions() {
        let course = ledger
            .course(revision.course())
            .ok_or("a revision names a course the ledger does not hold")?;
        assert_eq!(
            course.id(),
            revision.course(),
            "a revision resolved to the wrong course"
        );
        let course_side = course_markers(course);
        let revision_side = revision_markers(revision);
        // The code is deliberately shared between a course and its revisions --
        // section 8.2 puts `courseCode` on both -- so only a non-code marker is
        // a leak.
        let unexpected: Vec<&String> = course_side
            .intersection(&revision_side)
            .filter(|marker| marker.as_str() != course.code().as_str())
            .collect();
        assert!(
            unexpected.is_empty(),
            "course {:?} carries revision attributes {unexpected:?}",
            course.id()
        );
    }
    assert_eq!(
        ledger
            .revisions_of(fixture.course_a)
            .into_iter()
            .map(CourseRevision::id)
            .collect::<BTreeSet<_>>(),
        fixture.revisions_a.into_iter().collect::<BTreeSet<_>>(),
        "course A's revision set is not its own"
    );
    assert_eq!(
        ledger
            .revisions_of(fixture.course_b)
            .into_iter()
            .map(CourseRevision::id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([fixture.revision_b]),
        "course B's revision set is not its own"
    );

    // --- offering -> revision ----------------------------------------------
    for offering in ledger.offerings() {
        let revision = ledger
            .revision(offering.course_revision())
            .ok_or("an offering names a revision the ledger does not hold")?;
        assert_eq!(revision.id(), offering.course_revision());
        assert!(
            revision_markers(revision)
                .intersection(&offering_markers(offering))
                .next()
                .is_none(),
            "revision {:?} and offering {:?} share an attribute",
            revision.id(),
            offering.id()
        );
    }
    assert_eq!(
        ledger
            .offerings_of(fixture.revisions_a[0])
            .into_iter()
            .map(CourseOffering::id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([fixture.offerings_a[0], fixture.offerings_a[1]]),
        "the first revision's offerings are not its own"
    );
    assert_eq!(
        ledger
            .offerings_of(fixture.revisions_a[1])
            .into_iter()
            .map(CourseOffering::id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([fixture.offerings_a[2]]),
        "the second revision's offerings are not its own"
    );
    assert_eq!(
        ledger
            .offerings_of(fixture.revision_b)
            .into_iter()
            .map(CourseOffering::id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([fixture.offering_b]),
        "course B's revision's offerings are not its own"
    );

    // --- offering -> course -------------------------------------------------
    assert_eq!(
        ledger
            .offerings_for_course(fixture.course_a)
            .into_iter()
            .map(CourseOffering::id)
            .collect::<BTreeSet<_>>(),
        fixture.offerings_a.into_iter().collect::<BTreeSet<_>>(),
        "course A's offerings are not its own"
    );
    assert_eq!(
        ledger
            .offerings_for_course(fixture.course_b)
            .into_iter()
            .map(CourseOffering::id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([fixture.offering_b]),
        "course B's offerings are not its own"
    );

    // --- sibling -> sibling, and every other ordered pair -------------------
    // Six aggregates, thirty ordered pairs, no pair skipped. Each aggregate's
    // markers are unique to it except the course code a revision shares with
    // its own course, which is excluded by identity rather than by spelling.
    let mut labelled: Vec<(String, BTreeSet<String>, Option<CourseId>)> = Vec::new();
    for course in ledger.courses() {
        labelled.push((
            format!("course {:?}", course.id()),
            course_markers(course),
            Some(course.id()),
        ));
    }
    for revision in ledger.revisions() {
        labelled.push((
            format!("revision {:?}", revision.id()),
            revision_markers(revision),
            Some(revision.course()),
        ));
    }
    for offering in ledger.offerings() {
        labelled.push((
            format!("offering {:?}", offering.id()),
            offering_markers(offering),
            None,
        ));
    }
    // One course with two revisions and three offerings, plus the second
    // course, its revision and its offering that make a leak observable: nine
    // aggregates and every ordered pair of them.
    assert_eq!(
        labelled.len(),
        9,
        "the fixture is not the one this test names"
    );

    let mut compared = 0_usize;
    for (left_index, (left_name, left_markers, left_course)) in labelled.iter().enumerate() {
        for (right_index, (right_name, right_markers, right_course)) in labelled.iter().enumerate()
        {
            if left_index == right_index {
                continue;
            }
            compared += 1;
            let shared: BTreeSet<&String> = left_markers.intersection(right_markers).collect();
            if shared.is_empty() {
                continue;
            }
            // The one permitted sharing: a revision prints the course code of
            // the course it belongs to, which section 8.2 puts on both.
            let same_course =
                matches!((left_course, right_course), (Some(left), Some(right)) if left == right);
            assert!(
                same_course,
                "{left_name} and {right_name} share {shared:?} across a boundary"
            );
        }
    }
    assert_eq!(
        compared, 72,
        "the ordered-pair walk did not cover the fixture"
    );
    Ok(())
}

/// The closed-vocabulary reads the marker sweep excludes, checked per
/// aggregate.
///
/// `CurriculumCategory`, `OfferingStatus` and `GradingMode` are small fixed
/// sets that different aggregates share by contract, so they cannot be checked
/// by uniqueness. They are checked by equality instead: each aggregate reads
/// back the value the fixture set on *it*, so a read that came from a sibling
/// or from the wrong aggregate kind fails here rather than going unexamined.
#[test]
fn vocabulary_reads_belong_to_their_own_aggregate() -> TestResult {
    let fixture = leak_fixture()?;
    let ledger = &fixture.ledger;

    let expected_categories = [
        (fixture.revisions_a[0], CurriculumCategory::MajorElective),
        (fixture.revisions_a[1], CurriculumCategory::MajorRequired),
        (fixture.revision_b, CurriculumCategory::GeneralStudies),
    ];
    for (id, category) in expected_categories {
        assert_eq!(
            ledger
                .revision(id)
                .ok_or("a fixture revision is missing")?
                .curriculum_category(),
            category,
            "revision {id:?} reads another revision's category"
        );
    }

    let expected_offerings = [
        (
            fixture.offerings_a[0],
            OfferingStatus::Confirmed,
            GradingMode::Letter,
        ),
        (
            fixture.offerings_a[1],
            OfferingStatus::HistoricallyLikely,
            GradingMode::Unknown,
        ),
        (
            fixture.offerings_a[2],
            OfferingStatus::Uncertain,
            GradingMode::Unknown,
        ),
        (
            fixture.offering_b,
            OfferingStatus::Cancelled,
            GradingMode::SatisfactoryUnsatisfactory,
        ),
    ];
    for (id, status, mode) in expected_offerings {
        let offering = ledger.offering(id).ok_or("a fixture offering is missing")?;
        assert_eq!(
            offering.official_status(),
            status,
            "offering {id:?} reads another offering's status"
        );
        assert_eq!(
            offering.grading_mode(),
            mode,
            "offering {id:?} reads another offering's grading mode"
        );
    }

    // Every vocabulary variant the fixture uses is distinct, so the equalities
    // above are not all satisfied by one value.
    assert_eq!(
        expected_categories
            .iter()
            .map(|(_, category)| category.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        expected_categories.len(),
        "the category fixture reuses a variant"
    );
    assert_eq!(
        expected_offerings
            .iter()
            .map(|(_, status, _)| status.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        expected_offerings.len(),
        "the status fixture reuses a variant"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// equivalence_is_directional_and_effective_dated
// ---------------------------------------------------------------------------

/// `equivalence_is_directional_and_effective_dated`.
#[test]
fn equivalence_is_directional_and_effective_dated() -> TestResult {
    let left = course_id(0x0500)?;
    let right = course_id(0x0501)?;
    let mut relations = CourseRelations::new();
    relations.record_equivalence(EquivalenceRelation::record(
        left,
        right,
        bounded_interval()?,
    )?);

    // Directional: the asserted direction holds and the reverse does not. The
    // second direction is a second assertion, not a consequence of the first.
    assert!(
        relations.equivalent(left, right, AT),
        "the asserted direction does not hold"
    );
    assert!(
        !relations.equivalent(right, left, AT),
        "A -> B equivalence implied B -> A"
    );

    // Effective-dated on both edges of the half-open interval.
    assert!(
        !relations.equivalent(left, right, BEFORE),
        "the equivalence held before its interval opened"
    );
    assert!(
        !relations.equivalent(left, right, AFTER),
        "the equivalence held after its interval closed"
    );
    assert!(
        relations.equivalent(left, right, TimestampMillis::new(1_750_000_000_000)),
        "the equivalence did not hold at its inclusive lower bound"
    );
    assert!(
        !relations.equivalent(left, right, TimestampMillis::new(1_850_000_000_000)),
        "the equivalence held at its exclusive upper bound"
    );

    // Recording the reverse is what makes the reverse true, and it leaves the
    // forward direction's interval alone.
    relations.record_equivalence(EquivalenceRelation::record(right, left, open_interval())?);
    assert!(relations.equivalent(right, left, AFTER));
    assert!(
        !relations.equivalent(left, right, AFTER),
        "recording the reverse widened the forward direction"
    );

    // A reflexive equivalence is refused: it would make every course
    // substitutable for itself by an assertion rather than by identity.
    assert!(matches!(
        EquivalenceRelation::record(left, left, open_interval()),
        Err(CurriculumError::Reflexive {
            relation: "equivalence"
        })
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// replacement_does_not_imply_identity
// ---------------------------------------------------------------------------

/// `replacement_does_not_imply_identity`.
///
/// Section 8.1's own example is *기하모델링 폐지·고급컴퓨터그래픽스 대체*: one
/// course ends and a different one is named in its place. Nothing about that
/// makes the two one course, makes either substitutable for the other, or
/// retires anything by itself.
#[test]
fn replacement_does_not_imply_identity() -> TestResult {
    let retired = course_id(0x0600)?;
    let replacement = course_id(0x0601)?;
    let mut relations = CourseRelations::new();
    relations.record_replacement(ReplacementRelation::record(
        retired,
        replacement,
        open_interval(),
    )?);

    assert_eq!(
        relations.replacements_for(retired, AT),
        BTreeSet::from([replacement]),
        "the replacement was not recorded"
    );

    // The whole of the claim: with a replacement recorded and no identity
    // decision, every identity question is UNKNOWN in both directions.
    for (earlier, later) in [(retired, replacement), (replacement, retired)] {
        assert_eq!(
            relations.same_course(earlier, later, AT),
            CourseCodeReuse::Unknown,
            "a replacement produced an identity verdict"
        );
    }
    // And it implies neither of the other two relations.
    assert!(
        !relations.equivalent(retired, replacement, AT),
        "a replacement produced an equivalence"
    );
    assert!(
        !relations.equivalent(replacement, retired, AT),
        "a replacement produced a reverse equivalence"
    );
    assert!(
        !relations.retired(retired, AT),
        "a replacement retired the course it replaced"
    );

    // An identity decision is what moves the identity answer, and it moves only
    // the ordered pair it addresses.
    relations.record_identity(IdentityDecision::record(
        retired,
        replacement,
        CourseCodeReuse::Distinct,
        decision_id(0x0602)?,
        open_interval(),
    )?);
    assert_eq!(
        relations.same_course(retired, replacement, AT),
        CourseCodeReuse::Distinct
    );
    assert_eq!(
        relations.same_course(replacement, retired, AT),
        CourseCodeReuse::Unknown,
        "a decision about one ordered pair answered the reverse"
    );

    // `UNKNOWN` is the absence of a decision and cannot be recorded as one.
    assert!(matches!(
        IdentityDecision::record(
            retired,
            replacement,
            CourseCodeReuse::Unknown,
            decision_id(0x0603)?,
            open_interval(),
        ),
        Err(CurriculumError::Malformed {
            field: "identity decision",
            ..
        })
    ));
    Ok(())
}

/// A course code shared by two rows is not an identity either.
///
/// The other half of `replacement_does_not_imply_identity`'s contract: section
/// 8.2's contract is that course-code reuse is an explicit decision rather than
/// an inference, and the strongest available inference is the code itself.
#[test]
fn a_shared_course_code_produces_no_identity_verdict() -> TestResult {
    let published = official_source()?;
    let version = version(0x0002)?;
    let earlier = course_id(0x0700)?;
    let later = course_id(0x0701)?;
    let code = CourseCode::parse("M1522.001800")?;

    let publication = CurriculumPublication::from_official_source(&published, version)
        .with_course(
            CourseDraft::new(earlier, code.clone())
                .canonical_identity(entity_id(0x0702)?)
                .build()?,
        )
        .with_course(
            CourseDraft::new(later, code)
                .canonical_identity(entity_id(0x0703)?)
                .build()?,
        );
    let mut ledger = CurriculumLedger::new();
    CurriculumPublisher::new().publish(&mut ledger, publication)?;

    assert_eq!(
        ledger.course(earlier).map(|course| course.code().as_str()),
        ledger.course(later).map(|course| course.code().as_str()),
        "the fixture does not share a code"
    );
    assert_eq!(
        ledger.same_course(earlier, later, AT),
        CourseCodeReuse::Unknown,
        "a shared course code produced an identity verdict"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// retired_course_with_no_replacement_is_representable
// ---------------------------------------------------------------------------

/// `retired_course_with_no_replacement_is_representable`.
///
/// Section 8.1's second example: *IT창업개론 폐지·대체 미지정*. The retirement
/// type has no replacement field and no constructor that takes one, so the case
/// is not a special path — it is the only shape a retirement has.
#[test]
fn retired_course_with_no_replacement_is_representable() -> TestResult {
    let published = official_source()?;
    let version = version(0x0003)?;
    let retired = course_id(0x0800)?;

    let publication = CurriculumPublication::from_official_source(&published, version)
        .with_course(
            CourseDraft::new(retired, CourseCode::parse("M1522.009900")?)
                .canonical_identity(entity_id(0x0801)?)
                .build()?,
        )
        .with_retirement(RetirementRelation::record(retired, open_interval()));

    let mut ledger = CurriculumLedger::new();
    let receipt = CurriculumPublisher::new().publish(&mut ledger, publication)?;
    assert_eq!(receipt.courses(), [retired]);

    assert!(
        ledger.retired(retired, AT),
        "the retirement was not recorded"
    );
    assert!(
        ledger.replacements_for(retired, AT).is_empty(),
        "a retirement with no replacement produced one"
    );
    assert!(
        !ledger.retired(retired, BEFORE),
        "the retirement held before its interval opened"
    );

    // Retirement is independent of the other three in both directions: a
    // retired course has no identity verdict and no equivalence, and a course
    // that is only replaced is not retired.
    let other = course_id(0x0802)?;
    assert_eq!(
        ledger.same_course(retired, other, AT),
        CourseCodeReuse::Unknown
    );
    assert!(!ledger.equivalent(retired, other, AT));
    Ok(())
}

// ---------------------------------------------------------------------------
// curriculum_publish_is_atomic_under_injected_failure
// ---------------------------------------------------------------------------

/// An injector that fails one checkpoint, on its `nth` arrival.
#[derive(Debug)]
struct FailAt {
    point: PublishCheckpoint,
    nth: std::cell::Cell<usize>,
    fail_on: usize,
}

impl FailAt {
    const fn new(point: PublishCheckpoint, fail_on: usize) -> Self {
        Self {
            point,
            nth: std::cell::Cell::new(0),
            fail_on,
        }
    }
}

impl PublishFaultInjector for FailAt {
    fn hit(&self, point: PublishCheckpoint) -> Result<(), CurriculumError> {
        if point != self.point {
            return Ok(());
        }
        let seen = self.nth.get() + 1;
        self.nth.set(seen);
        if seen == self.fail_on {
            return Err(CurriculumError::InjectedFault(point.as_str()));
        }
        Ok(())
    }
}

/// A publication carrying every aggregate kind, so every checkpoint is reached.
fn full_publication(
    published: &PublishedRules,
    version_suffix: u32,
) -> Result<CurriculumPublication, Box<dyn Error>> {
    let version = version(version_suffix)?;
    let version_key = version.id();
    let first = course_id(0x0900 + version_suffix)?;
    let second = course_id(0x0910 + version_suffix)?;
    let revision = revision_id(0x0920 + version_suffix)?;
    Ok(
        CurriculumPublication::from_official_source(published, version)
            .with_course(
                CourseDraft::new(first, CourseCode::parse("M1522.001800")?)
                    .canonical_identity(entity_id(0x0930 + version_suffix)?)
                    .build()?,
            )
            .with_course(
                CourseDraft::new(second, CourseCode::parse("M1522.001900")?)
                    .canonical_identity(entity_id(0x0940 + version_suffix)?)
                    .build()?,
            )
            .with_revision(
                CourseRevisionDraft::new(
                    revision,
                    first,
                    version_key,
                    CourseCode::parse("M1522.001800")?,
                    open_interval(),
                )
                .title(CourseTitle::parse("데이터베이스")?)
                .credits(Credits::new(3)?)
                .build()?,
            )
            .with_offering(
                CourseOfferingDraft::new(
                    offering_id(0x0950 + version_suffix)?,
                    revision,
                    TermCode::parse("2026_FALL")?,
                    SectionCode::parse("001")?,
                    OfferingStatus::Confirmed,
                    AT,
                )
                .build(),
            )
            // A second offering, so `AfterOffering` is reached more than once
            // and the arrival that leaves a half-written list behind exists.
            .with_offering(
                CourseOfferingDraft::new(
                    offering_id(0x0970 + version_suffix)?,
                    revision,
                    TermCode::parse("2026_FALL")?,
                    SectionCode::parse("002")?,
                    OfferingStatus::Confirmed,
                    AT,
                )
                .build(),
            )
            .with_identity(IdentityDecision::record(
                first,
                second,
                CourseCodeReuse::Distinct,
                decision_id(0x0960 + version_suffix)?,
                open_interval(),
            )?)
            .with_equivalence(EquivalenceRelation::record(first, second, open_interval())?)
            .with_replacement(ReplacementRelation::record(first, second, open_interval())?)
            .with_retirement(RetirementRelation::record(first, open_interval())),
    )
}

/// `curriculum_publish_is_atomic_under_injected_failure`.
///
/// Every checkpoint in `PublishCheckpoint::ALL` is failed in turn, against a
/// ledger that already holds one published version so a rewind to zero would
/// pass for the wrong reason. The assertion is whole-value equality: the ledger
/// after the failed publication is the value it was before it.
#[test]
fn curriculum_publish_is_atomic_under_injected_failure() -> TestResult {
    let published = official_source()?;

    // A ledger that is not empty, so "rewound" is distinguishable from
    // "cleared".
    let mut ledger = CurriculumLedger::new();
    CurriculumPublisher::new().publish(&mut ledger, full_publication(&published, 0x0001)?)?;
    let before = ledger.clone();
    assert!(!before.courses().is_empty(), "the base ledger is empty");

    let mut exercised: Vec<&'static str> = Vec::new();
    for point in PublishCheckpoint::ALL {
        // The loop checkpoints are reached more than once. Failing the second
        // arrival is what leaves a half-written aggregate list behind if the
        // rewind is wrong; failing the first would be indistinguishable from a
        // publication that never started.
        let fail_on = match point {
            PublishCheckpoint::AfterCourse
            | PublishCheckpoint::AfterOffering
            | PublishCheckpoint::AfterRelation => 2,
            _ => 1,
        };
        let injector = FailAt::new(point, fail_on);
        let publisher = CurriculumPublisher::with_faults(&injector);
        let outcome = publisher.publish(&mut ledger, full_publication(&published, 0x0002)?);

        match outcome {
            Err(CurriculumError::InjectedFault(name)) => {
                assert_eq!(name, point.as_str(), "a different checkpoint failed");
                exercised.push(name);
            }
            Err(other) => return Err(format!("{point:?} produced {other}").into()),
            Ok(_) => {
                // `AfterRelation` at the second arrival is reachable, and so is
                // every other row; a checkpoint that cannot be reached at the
                // requested arrival is a fixture error rather than a pass.
                return Err(format!("{point:?} was never reached at arrival {fail_on}").into());
            }
        }
        assert_eq!(
            ledger, before,
            "the ledger changed after a publication that failed at {point:?}"
        );
    }

    // Enumerated, not counted: every checkpoint the type declares was failed.
    let declared: Vec<&'static str> = PublishCheckpoint::ALL
        .iter()
        .map(|point| point.as_str())
        .collect();
    assert_eq!(
        exercised, declared,
        "a declared checkpoint was not exercised"
    );

    // And the same publication with no injected fault does publish, so the
    // ledger equality above is not the equality of a publication that could
    // never have succeeded.
    CurriculumPublisher::new().publish(&mut ledger, full_publication(&published, 0x0002)?)?;
    assert_ne!(ledger, before, "the uninjected publication changed nothing");
    Ok(())
}

/// A publication that names a parent it does not carry writes nothing either.
#[test]
fn a_dangling_publication_writes_nothing() -> TestResult {
    let published = official_source()?;
    let version = version(0x0004)?;
    let version_key = version.id();
    let mut ledger = CurriculumLedger::new();
    let before = ledger.clone();

    let publication = CurriculumPublication::from_official_source(&published, version)
        .with_revision(
            CourseRevisionDraft::new(
                revision_id(0x0A00)?,
                course_id(0x0A01)?,
                version_key,
                CourseCode::parse("M1522.001800")?,
                open_interval(),
            )
            .title(CourseTitle::parse("데이터베이스")?)
            .credits(Credits::new(3)?)
            .build()?,
        );
    assert!(matches!(
        CurriculumPublisher::new().publish(&mut ledger, publication),
        Err(CurriculumError::Dangling {
            child: "course revision",
            parent: "course"
        })
    ));
    assert_eq!(ledger, before, "a refused publication wrote something");
    Ok(())
}

// ---------------------------------------------------------------------------
// The transitional measure, and the three open gates
// ---------------------------------------------------------------------------

/// 경과조치 is independent of the three course-level relations.
///
/// Section 11.4 makes it the fourth independent rule. It moves an admission
/// cohort between curriculum versions, so no course relation can produce it and
/// it can produce none of them: it names no course at all.
#[test]
fn a_transition_arrangement_is_not_derived_from_a_course_relation() -> TestResult {
    let cohort = AdmissionCohort::parse("2025")?;
    let superseded = version_id(0x0B00)?;

    // A version that supersedes another and records no arrangement answers
    // UNKNOWN. `supersedes` is not a transition and does not stand in for one.
    let silent = CurriculumVersionDraft::new(
        version_id(0x0B01)?,
        (
            AdmissionCohort::parse("2026")?,
            AdmissionCohort::parse("2026")?,
        ),
        open_interval(),
    )
    .institution_segment("SNU")
    .supersedes(superseded)
    .build()?;
    assert_eq!(silent.supersedes(), Some(superseded));
    assert_eq!(
        silent.transition_for(&cohort),
        CohortTransition::Unknown,
        "supersession produced a cohort transition"
    );

    // The arrangement is what answers it, and it is recorded per cohort.
    let arranged = CurriculumVersionDraft::new(
        version_id(0x0B02)?,
        (
            AdmissionCohort::parse("2026")?,
            AdmissionCohort::parse("2026")?,
        ),
        open_interval(),
    )
    .institution_segment("SNU")
    .supersedes(superseded)
    .transition(TransitionArrangement::record(
        AdmissionCohort::parse("2025")?,
        CohortTransition::Stays,
        open_interval(),
    )?)
    .transition(TransitionArrangement::record(
        AdmissionCohort::parse("2026")?,
        CohortTransition::Moves,
        open_interval(),
    )?)
    .build()?;
    assert_eq!(arranged.transition_for(&cohort), CohortTransition::Stays);
    assert_eq!(
        arranged.transition_for(&AdmissionCohort::parse("2026")?),
        CohortTransition::Moves
    );
    assert_eq!(
        arranged.transition_for(&AdmissionCohort::parse("2024")?),
        CohortTransition::Unknown,
        "an unaddressed cohort was given an arrangement"
    );

    // `UNKNOWN` is the absence of an arrangement and cannot be recorded as one.
    assert!(matches!(
        TransitionArrangement::record(cohort, CohortTransition::Unknown, open_interval()),
        Err(CurriculumError::Malformed {
            field: "transition arrangement",
            ..
        })
    ));
    Ok(())
}

/// The three section 38 cells this task leaves open, and what stands while they
/// are empty.
#[test]
fn an_absent_official_fact_reads_unknown() -> TestResult {
    // The gates are enumerated and each names its own identifier.
    let identifiers: Vec<&str> = OpenGate::ALL.iter().map(|gate| gate.identifier()).collect();
    assert_eq!(
        identifiers,
        ["GATE-38-013", "GATE-38-014", "GATE-38-018"],
        "the open gates are not the three t068 section 5 names for P2-U1"
    );
    for gate in OpenGate::ALL {
        assert!(
            gate.statement().contains(gate.identifier()),
            "{} states no identifier",
            gate.identifier()
        );
    }

    // Every value meaning "no official record exists" spells UNKNOWN, and each
    // is what its own constructor starts at rather than a value a caller picked.
    for (owner, spelling) in unknown_readings() {
        assert_eq!(
            spelling, "UNKNOWN",
            "{owner}'s absent reading is not UNKNOWN"
        );
    }

    let revision = CourseRevisionDraft::new(
        revision_id(0x0C00)?,
        course_id(0x0C01)?,
        version_id(0x0C02)?,
        CourseCode::parse("M1522.001800")?,
        open_interval(),
    )
    .title(CourseTitle::parse("데이터베이스")?)
    .credits(Credits::new(3)?)
    .build()?;
    assert_eq!(
        revision.curriculum_category(),
        CurriculumCategory::Unknown,
        "an unconfirmed revision was given a category"
    );
    assert!(!revision.curriculum_category().is_known());

    let offering = CourseOfferingDraft::new(
        offering_id(0x0C03)?,
        revision.id(),
        TermCode::parse("2026_FALL")?,
        SectionCode::parse("001")?,
        OfferingStatus::Uncertain,
        AT,
    )
    .build();
    assert_eq!(
        offering.grading_mode(),
        GradingMode::Unknown,
        "an unconfirmed offering was given a grading mode"
    );
    Ok(())
}

/// `GATE-38-018`: the two prerequisite lists are separate and nothing compares
/// them.
#[test]
fn official_and_recommended_prerequisites_stay_two_lists() -> TestResult {
    let course = course_id(0x0D00)?;
    let official = course_id(0x0D01)?;
    let recommended = course_id(0x0D02)?;
    let revision = CourseRevisionDraft::new(
        revision_id(0x0D03)?,
        course,
        version_id(0x0D04)?,
        CourseCode::parse("M1522.001800")?,
        open_interval(),
    )
    .title(CourseTitle::parse("데이터베이스")?)
    .credits(Credits::new(3)?)
    .official_prerequisite(OfficialPrerequisite::on(official))
    .recommended_prerequisite(RecommendedPrerequisite::on(recommended))
    .build()?;

    assert_eq!(
        revision
            .official_prerequisites()
            .iter()
            .map(|entry| entry.course())
            .collect::<Vec<_>>(),
        vec![official]
    );
    assert_eq!(
        revision
            .recommended_prerequisites()
            .iter()
            .map(|entry| entry.course())
            .collect::<Vec<_>>(),
        vec![recommended]
    );
    // Neither list contains the other's entry. The comparison `GATE-38-018`
    // asks for needs a reviewed source and this crate performs none.
    assert!(
        !revision
            .official_prerequisites()
            .iter()
            .any(|entry| entry.course() == recommended)
    );
    assert!(
        !revision
            .recommended_prerequisites()
            .iter()
            .any(|entry| entry.course() == official)
    );
    Ok(())
}
