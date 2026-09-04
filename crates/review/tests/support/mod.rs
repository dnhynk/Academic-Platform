//! Synthetic reviews, composed here.
//!
//! Nothing in this module is a real review of a real course by a real person.
//! The texts are English sentences written for this repository, chosen so the
//! trigram sets the duplicate check compares are small enough to count by hand
//! -- the expected similarities in `duplicate_similarity_is_detected` are
//! computed from the definition in `crates/review/src/duplicate.rs` and written
//! into the test as literals, together with the intersection and union sizes
//! they came from.
//!
//! Every fixture returns a `Result`, the way `crates/ingestion`'s does: the
//! workspace lints deny `unwrap` and `expect`, and a fixture that panicked
//! would report a failure with no cause.

use std::error::Error;

use academic_curriculum::{InstructorName, TermCode};
use academic_domain::{ConfidencePermille, CourseId, OfferingId};
use academic_ingestion::{ConnectorId, RetrievalInstant, TermsStatus};
use academic_proposal::{Autosaved, ImpactPermille, ProposalId, Proposed, ReviewQueue, RiskTier};
use academic_review::{
    BiasDimension, BiasDisclosure, BiasDisclosureDraft, BiasFinding, BiasStrength, DimensionBand,
    DimensionReading, PermittedCollection, RawReviewText, ReviewDimension, ReviewExtraction,
    ReviewRecord, ReviewScope, SampleBias, SourceAccessMode, SourceTermsLedger, permit,
};

type Fixture<T> = Result<T, Box<dyn Error>>;

/// The synthetic review source every fixture collects from.
pub const SOURCE: &str = "synthetic.review.board";

/// A connector identifier for a fixture source.
pub fn source(name: &str) -> Fixture<ConnectorId> {
    Ok(ConnectorId::new(name)?)
}

/// A ledger that permits one source in one mode and nothing else.
pub fn ledger_permitting(name: &str, mode: SourceAccessMode) -> Fixture<SourceTermsLedger> {
    Ok(SourceTermsLedger::empty().recording(
        source(name)?,
        mode,
        TermsStatus::PermittedForDeclaredMethod,
    ))
}

/// A permitted collection for one source in one mode.
pub fn collection(name: &str, mode: SourceAccessMode) -> Fixture<PermittedCollection> {
    Ok(permit(
        &ledger_permitting(name, mode)?,
        &source(name)?,
        mode,
    )?)
}

/// A scope naming an offering, an instructor and a term.
pub fn scope(
    source_name: &str,
    offering: u32,
    instructor: &str,
    term: &str,
) -> Fixture<ReviewScope> {
    Ok(ReviewScope::new(
        source(source_name)?,
        Some(offering_id(offering)?),
        Some(InstructorName::parse(instructor)?),
        Some(TermCode::parse(term)?),
    ))
}

/// A fixed UUIDv7-shaped offering identifier from a small number, so two
/// fixtures name the same offering when they mean to.
///
/// Composed as text rather than through the `uuid` crate: this crate has no
/// edge to it, and an identifier a fixture types is one a reader can check
/// against the version and variant nibbles by eye.
pub fn offering_id(suffix: u32) -> Fixture<OfferingId> {
    Ok(format!("01900000-0000-7000-8000-0000{suffix:08x}").parse()?)
}

/// A fixed UUIDv7-shaped course identifier, composed the same way.
pub fn course_id(suffix: u32) -> Fixture<CourseId> {
    Ok(format!("01900000-0000-7000-8000-0001{suffix:08x}").parse()?)
}

/// Retains `text` with one provenance span over the whole of it.
pub fn retained(text: &str) -> Fixture<RawReviewText> {
    let digest = RawReviewText::digest_of(text.as_bytes());
    Ok(RawReviewText::retain(
        text,
        &[(0, text.len(), digest.as_str())],
    )?)
}

/// A complete extraction, every dimension at `band`, read from span zero.
pub fn extraction_at(band: DimensionBand) -> Fixture<ReviewExtraction> {
    let readings: Vec<DimensionReading> = ReviewDimension::ALL
        .into_iter()
        .map(|dimension| DimensionReading::new(dimension, band, 0))
        .collect();
    Ok(ReviewExtraction::read(&readings)?)
}

/// An extraction with one dimension moved to another band.
pub fn extraction_with(
    band: DimensionBand,
    moved: ReviewDimension,
    to: DimensionBand,
) -> Fixture<ReviewExtraction> {
    let readings: Vec<DimensionReading> = ReviewDimension::ALL
        .into_iter()
        .map(|dimension| {
            let at = if dimension == moved { to } else { band };
            DimensionReading::new(dimension, at, 0)
        })
        .collect();
    Ok(ReviewExtraction::read(&readings)?)
}

/// Puts `extraction` through `P2-M2`'s low-risk door, which is what makes the
/// record's status `AI_INFERRED` by type.
///
/// The identifier is the caller's, so two fixtures in one test do not collide.
pub fn autosaved(id: u64, extraction: ReviewExtraction) -> Fixture<Autosaved<ReviewExtraction>> {
    let mut queue: ReviewQueue<ReviewExtraction> = ReviewQueue::new();
    let proposal = ProposalId::new(id);
    queue.admit(Proposed::new(
        proposal,
        RiskTier::LowAutosave,
        ConfidencePermille::new(820)?,
        ImpactPermille::new(120)?,
        extraction,
    ))?;
    Ok(queue.autosave(proposal)?)
}

/// One synthetic review, collected under `mode`.
pub fn review(
    id: u64,
    scope: ReviewScope,
    text: &str,
    mode: SourceAccessMode,
    extraction: ReviewExtraction,
) -> Fixture<ReviewRecord> {
    Ok(ReviewRecord::collected(
        &collection(SOURCE, mode)?,
        scope,
        retained(text)?,
        RetrievalInstant::at(1_700_000_000 + id),
        autosaved(id, extraction)?,
        SampleBias::none(),
    ))
}

/// A complete disclosure with a chosen sample count and everything else `LOW`.
pub fn disclosure(sample_count: u32) -> Fixture<BiasDisclosure> {
    Ok(draft_disclosing(sample_count, &BiasDimension::ALL).build()?)
}

/// A draft disclosing exactly `dimensions`.
///
/// The test that drops one dimension at a time uses this, so what it drops is
/// a dimension and not a differently-built draft.
#[must_use]
pub fn draft_disclosing(sample_count: u32, dimensions: &[BiasDimension]) -> BiasDisclosureDraft {
    let mut draft = BiasDisclosureDraft::new();
    for dimension in dimensions {
        let measured = if *dimension == BiasDimension::SampleCount {
            sample_count
        } else {
            0
        };
        draft = draft.disclosing(BiasFinding::new(*dimension, measured, BiasStrength::Low));
    }
    draft
}
