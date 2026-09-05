//! Synthetic values for the four surfaces.
//!
//! Nothing here is a real course, a real offering, a real review or a real
//! grade. The attempt ledger is `academic-record`'s own synthetic corpus, so the
//! averages this suite compares against are that crate's answers over its own
//! fixture rather than numbers written here; the curriculum, offering and review
//! values are composed from `P2-U1`'s and `P2-U8`'s own drafts and constructors.

use std::error::Error;

use academic_curriculum::{
    Course, CourseCode, CourseDraft, CourseOffering, CourseOfferingDraft, CourseRevision,
    CourseRevisionDraft, CourseTitle, Credits, CurriculumCategory, GradingMode, InstructorName,
    OfferingStatus, SectionCode, TermCode,
};
use academic_domain::{
    ConfidencePermille, ContentDigest, CourseId, CourseRevisionId, CurriculumVersionId, EntityId,
    OfferingId, TimestampMillis, ValidInterval,
};
use academic_ingestion::{ConnectorId, RetrievalInstant, TermsStatus};
use academic_proposal::{Autosaved, ImpactPermille, ProposalId, Proposed, ReviewQueue, RiskTier};
use academic_review::{
    BiasDimension, BiasDisclosure, BiasDisclosureDraft, BiasFinding, BiasStrength, DimensionBand,
    DimensionReading, RawReviewText, ReviewDimension, ReviewExtraction, ReviewRecord, ReviewScope,
    SampleBias, SourceAccessMode, SourceTermsLedger, permit,
};

use academic_dashboard::{CandidateOffering, MeetingSlot, RequirementContribution, WorkloadRange};

pub type Fixture<T> = Result<T, Box<dyn Error>>;

/// The instant every dated fixture is anchored to. Not a clock reading.
pub const ORIGIN: i64 = 1_700_000_000_000;

/// The synthetic review source every review fixture collects from.
pub const SOURCE: &str = "synthetic.review.board";

/// A fixed UUIDv7-shaped identifier from a small number and a prefix nibble.
fn identifier(prefix: u8, suffix: u32) -> String {
    format!("01900000-0000-7000-8000-{prefix:04x}{suffix:08x}")
}

/// A course identifier.
pub fn course_id(suffix: u32) -> Fixture<CourseId> {
    Ok(identifier(1, suffix).parse()?)
}

/// A course-revision identifier.
pub fn revision_id(suffix: u32) -> Fixture<CourseRevisionId> {
    Ok(identifier(2, suffix).parse()?)
}

/// A curriculum-version identifier.
pub fn version_id(suffix: u32) -> Fixture<CurriculumVersionId> {
    Ok(identifier(3, suffix).parse()?)
}

/// An offering identifier.
pub fn offering_id(suffix: u32) -> Fixture<OfferingId> {
    Ok(identifier(4, suffix).parse()?)
}

/// A generic entity identifier, for a concept, competency, project or role.
pub fn entity_id(suffix: u32) -> Fixture<EntityId> {
    Ok(identifier(5, suffix).parse()?)
}

/// The validity window every revision fixture is published for.
pub fn interval() -> ValidInterval {
    ValidInterval::open_ended(TimestampMillis::new(ORIGIN))
}

/// One catalogue course.
pub fn course(suffix: u32, code: &str) -> Fixture<Course> {
    Ok(
        CourseDraft::new(course_id(suffix)?, CourseCode::parse(code)?)
            .canonical_identity(entity_id(suffix)?)
            .build()?,
    )
}

/// One revision of `course`, with a designed-coverage concept.
pub fn revision(
    suffix: u32,
    course: &Course,
    code: &str,
    credits: u8,
    category: CurriculumCategory,
) -> Fixture<CourseRevision> {
    Ok(CourseRevisionDraft::new(
        revision_id(suffix)?,
        course.id(),
        version_id(suffix)?,
        CourseCode::parse(code)?,
        interval(),
    )
    .title(CourseTitle::parse("데이터베이스")?)
    .credits(Credits::new(credits)?)
    .curriculum_category(category)
    .designed_concept(entity_id(suffix.saturating_add(100))?)
    .source_snapshot(ContentDigest::sha256(b"synthetic official catalogue page"))
    .build()?)
}

/// One published offering of `revision`.
pub fn offering(suffix: u32, revision: &CourseRevision, term: &str) -> Fixture<CourseOffering> {
    Ok(CourseOfferingDraft::new(
        offering_id(suffix)?,
        revision.id(),
        TermCode::parse(term)?,
        SectionCode::parse("SEC001")?,
        OfferingStatus::Confirmed,
        TimestampMillis::new(ORIGIN),
    )
    .instructor(InstructorName::parse("Kim")?)
    .grading_mode(GradingMode::Letter)
    .build())
}

/// A review scope naming an offering, an instructor and a term.
pub fn scope(offering: u32, instructor: &str, term: &str) -> Fixture<ReviewScope> {
    Ok(ReviewScope::new(
        ConnectorId::new(SOURCE)?,
        Some(offering_id(offering)?),
        Some(InstructorName::parse(instructor)?),
        Some(TermCode::parse(term)?),
    ))
}

/// One synthetic review, every dimension at `band`.
pub fn review(
    id: u64,
    scope: ReviewScope,
    text: &str,
    band: DimensionBand,
) -> Fixture<ReviewRecord> {
    let ledger = SourceTermsLedger::empty().recording(
        ConnectorId::new(SOURCE)?,
        SourceAccessMode::ManualPaste,
        TermsStatus::PermittedForDeclaredMethod,
    );
    let collection = permit(
        &ledger,
        &ConnectorId::new(SOURCE)?,
        SourceAccessMode::ManualPaste,
    )?;
    let digest = RawReviewText::digest_of(text.as_bytes());
    let retained = RawReviewText::retain(text, &[(0, text.len(), digest.as_str())])?;
    let readings: Vec<DimensionReading> = ReviewDimension::ALL
        .into_iter()
        .map(|dimension| DimensionReading::new(dimension, band, 0))
        .collect();
    let extraction = ReviewExtraction::read(&readings)?;
    let mut queue: ReviewQueue<ReviewExtraction> = ReviewQueue::new();
    let proposal = ProposalId::new(id);
    queue.admit(Proposed::new(
        proposal,
        RiskTier::LowAutosave,
        ConfidencePermille::new(820)?,
        ImpactPermille::new(120)?,
        extraction,
    ))?;
    let autosaved: Autosaved<ReviewExtraction> = queue.autosave(proposal)?;
    Ok(ReviewRecord::collected(
        &collection,
        scope,
        retained,
        RetrievalInstant::at(1_700_000_000 + id),
        autosaved,
        SampleBias::none(),
    ))
}

/// A complete bias disclosure over `sample_count` reviews.
pub fn disclosure(sample_count: u32) -> Fixture<BiasDisclosure> {
    let mut draft = BiasDisclosureDraft::new();
    for dimension in BiasDimension::ALL {
        let measured = if dimension == BiasDimension::SampleCount {
            sample_count
        } else {
            0
        };
        draft = draft.disclosing(BiasFinding::new(dimension, measured, BiasStrength::Low));
    }
    Ok(draft.build()?)
}

/// One planner candidate carrying a fact on each of section 25.5's six axes.
///
/// `suffix` reaches every axis, so two candidates built from two numbers differ
/// on all six. That is what makes a per-axis assertion able to fail: a fixture
/// whose axes were constant would make six readings that never move.
pub fn candidate(suffix: u32, code: &str, term: &str, start: u32) -> Fixture<CandidateOffering> {
    Ok(CandidateOffering::declaring(
        offering_id(suffix)?,
        CourseCode::parse(code)?,
        TermCode::parse(term)?,
        3,
        MeetingSlot::new(start, start.saturating_add(90))?,
        vec![CourseCode::parse("4190.101")?],
        vec![RequirementContribution::of(
            format!("major-required-{suffix}"),
            3,
            format!("node/{suffix}"),
        )?],
        vec![format!("concept/{suffix}")],
        vec![format!("project/{suffix}")],
        WorkloadRange::observed(
            6,
            10,
            vec![format!("review-sample/{suffix}")],
            vec![format!("instructor-differs/{suffix}")],
        )?,
        vec![CourseCode::parse("4190.408")?],
    ))
}
