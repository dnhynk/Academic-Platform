//! Section 29.5's `ReviewRecord`, field for field.
//!
//! The record the specification writes is:
//!
//! ```yaml
//! ReviewRecord:
//!   offering: ... | null
//!   instructor: ... | null
//!   term: ... | null
//!   rawArtifact: ...
//!   sourceAccessMode: PUBLIC | USER_PROVIDED_EXPORT | MANUAL_PASTE
//!   collectedAt: ...
//!   dimensions: ...
//!   extractionStatus: AI_INFERRED
//!   provenanceSpans: [...]
//!   sampleBias: ...
//! ```
//!
//! `the_record_fields_are_section_29_5s_own` reads that block out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares its keys
//! against this record's accessors in both directions, so a key the
//! specification names and this does not, or the reverse, fails.
//!
//! # `extractionStatus` is not a field
//!
//! The specification writes it as a literal, not a union: an extracted review
//! is `AI_INFERRED` and there is no other value. So it is not stored.
//! [`ReviewRecord`]'s one producer takes an
//! `academic_proposal::Autosaved<ReviewExtraction>`, whose
//! `EPISTEMIC_STATUS` is `AI_INFERRED`, and
//! [`ReviewRecord::extraction_status`] returns that constant. `P2-M2` owns the
//! rule and this crate reuses it: section 27.4's low-risk row is *save it and
//! mark it `AI_INFERRED`*, and `academic_proposal::ReviewQueue::autosave` is
//! the one door that produces an `Autosaved` — it serves `LOW_AUTOSAVE` alone
//! and takes no user decision.
//!
//! What that buys is that there is no argument anywhere in this crate a caller
//! could pass to make a review record claim a stronger status. A reading a user
//! has confirmed is not this type; it is a `P2-M2` `Approved` value, and
//! promoting one is that crate's contract rather than a second path here.
//!
//! # `rawArtifact` is held and never handed over
//!
//! See [`crate::text`]. The accessor returns `&RawReviewText`, which has no
//! route to a `String`, so `review.raw()` is not a step towards putting the
//! text in a bundle.

use academic_domain::EpistemicStatus;
use academic_ingestion::RetrievalInstant;
use academic_proposal::Autosaved;

use crate::{
    access::{PermittedCollection, SourceAccessMode},
    bias::BiasDimension,
    dimension::{DimensionBand, ReviewDimension, ReviewExtraction},
    scope::ReviewScope,
    text::{ProvenanceSpan, RawReviewText},
};

/// Section 29.5's `sampleBias` on a single record.
///
/// Which of the six aggregate disclosures *this one review* contributes to. It
/// is a set of [`BiasDimension`] rather than a free note so the per-record
/// field and the aggregate's disclosure are the same vocabulary: a review
/// flagged [`BiasDimension::ExtremeExperience`] here is counted under that
/// dimension there, and there is no third name for the same idea.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SampleBias {
    signals: Vec<BiasDimension>,
}

impl SampleBias {
    /// A record nobody flagged.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            signals: Vec::new(),
        }
    }

    /// Flags one dimension.
    #[must_use]
    pub fn flagging(mut self, dimension: BiasDimension) -> Self {
        if !self.signals.contains(&dimension) {
            self.signals.push(dimension);
            self.signals.sort_unstable();
        }
        self
    }

    /// Which dimensions are flagged, in [`BiasDimension::ALL`] order.
    #[must_use]
    pub fn signals(&self) -> &[BiasDimension] {
        &self.signals
    }

    /// Whether one dimension is flagged.
    #[must_use]
    pub fn flags(&self, dimension: BiasDimension) -> bool {
        self.signals.contains(&dimension)
    }
}

/// One review, as section 29.5 records it.
///
/// Private fields, no setter, and one producer, [`ReviewRecord::collected`].
/// `Debug` is hand-written and reaches the raw text only through
/// [`RawReviewText`]'s own hand-written one.
#[derive(Debug)]
pub struct ReviewRecord {
    scope: ReviewScope,
    raw_artifact: RawReviewText,
    source_access_mode: SourceAccessMode,
    collected_at: RetrievalInstant,
    dimensions: ReviewExtraction,
    sample_bias: SampleBias,
}

impl ReviewRecord {
    /// The one status a review record ever carries.
    ///
    /// `academic_proposal::Autosaved`'s constant, reused. Section 29.5 writes
    /// `extractionStatus: AI_INFERRED` as a literal and this is that literal,
    /// held in one place for the whole workspace.
    pub const EXTRACTION_STATUS: EpistemicStatus = Autosaved::<ReviewExtraction>::EPISTEMIC_STATUS;

    /// The one producer.
    ///
    /// `collection` is `P2-U6`'s recorded terms decision for this source *and*
    /// this access mode; there is no way to obtain one for a source nobody
    /// reviewed, so a record that exists is a record whose collection somebody
    /// permitted. `extraction` is `P2-M2`'s autosaved proposal, which is what
    /// makes the extraction status a type fact.
    ///
    /// The scope's source and the permitted collection's source are the same
    /// value by construction: the scope is built from the collection rather
    /// than beside it, so there is no pair of arguments that could disagree.
    #[must_use]
    pub fn collected(
        collection: &PermittedCollection,
        scope: ReviewScope,
        raw_artifact: RawReviewText,
        collected_at: RetrievalInstant,
        extraction: Autosaved<ReviewExtraction>,
        sample_bias: SampleBias,
    ) -> Self {
        Self {
            scope,
            raw_artifact,
            source_access_mode: collection.mode(),
            collected_at,
            dimensions: extraction.into_inner(),
            sample_bias,
        }
    }

    /// What this review is attached to.
    #[must_use]
    pub const fn scope(&self) -> &ReviewScope {
        &self.scope
    }

    /// The retained text. See [`crate::text`] for what can be done with it.
    #[must_use]
    pub const fn raw_artifact(&self) -> &RawReviewText {
        &self.raw_artifact
    }

    /// How the text was obtained.
    #[must_use]
    pub const fn source_access_mode(&self) -> SourceAccessMode {
        self.source_access_mode
    }

    /// When it was collected.
    #[must_use]
    pub const fn collected_at(&self) -> RetrievalInstant {
        self.collected_at
    }

    /// What was read out of it.
    #[must_use]
    pub const fn dimensions(&self) -> &ReviewExtraction {
        &self.dimensions
    }

    /// Always `AI_INFERRED`.
    #[must_use]
    pub const fn extraction_status(&self) -> EpistemicStatus {
        Self::EXTRACTION_STATUS
    }

    /// Where each reading was read from.
    #[must_use]
    pub fn provenance_spans(&self) -> &[ProvenanceSpan] {
        self.raw_artifact.spans()
    }

    /// What this one review is flagged for.
    #[must_use]
    pub const fn sample_bias(&self) -> &SampleBias {
        &self.sample_bias
    }

    /// The band this review read one dimension at.
    #[must_use]
    pub fn band(&self, dimension: ReviewDimension) -> DimensionBand {
        self.dimensions.band(dimension)
    }
}
