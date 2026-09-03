//! Three low-confidence span kinds, each with the context that reaches its
//! source.
//!
//! Section 25.13's fourth bullet is *`low-confidence transcript/math/code`*.
//! Section 34.1 fixes what each of the three has to show:
//!
//! | Row | Uncertainty display |
//! |---|---|
//! | `STT 오인식` | `token/segment underline, provider/version, 원음 듣기` |
//! | `수식·코드 전사 오류` | `UNVERIFIED_EQUATION/CODE, confidence와 source image` |
//!
//! # Context is a locator, not the text
//!
//! Section 25.7 says what the transcript context has to do: *`문단을 선택하면
//! 반드시 원 audio timestamp와 raw segment로 돌아갈 수 있다`*. That is a
//! requirement to *reach* the source, not to copy it, and this crate reaches it
//! with identifiers, a time range and a version — never with the transcribed
//! text or the source image bytes. §32.8's rule is the reason: the audit surface
//! does not copy sensitive originals into itself.
//!
//! Every span therefore carries a locator whose whole content is identifiers,
//! offsets and a digest, and `the_center_cannot_name_a_payload_byte` is what
//! says none of them can become bytes.

use academic_domain::{
    ConfidencePermille, ContentDigest, LectureDocumentId, LectureSessionId, TimestampMillis,
    TranscriptVersionId,
};

/// The three kinds section 25.13 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpanKind {
    /// A transcribed audio span the recogniser was unsure of.
    Transcript,
    /// An equation the transcription may have turned into prose.
    Math,
    /// A code span the transcription may have reflowed.
    Code,
}

impl SpanKind {
    /// Exhaustive listing, in section 25.13's own reading order.
    pub const ALL: [Self; 3] = [Self::Transcript, Self::Math, Self::Code];

    /// Section 25.13's own word for this kind.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::Transcript => "transcript",
            Self::Math => "math",
            Self::Code => "code",
        }
    }

    /// Section 34.1's uncertainty token for this kind.
    ///
    /// The two `수식·코드 전사 오류` tokens are the specification's own spellings.
    /// The transcript row displays an underline and a provider rather than a
    /// token, so its marker names what is uncertain — the recogniser's own
    /// per-segment confidence.
    #[must_use]
    pub const fn marker_token(self) -> &'static str {
        match self {
            Self::Transcript => "SEGMENT_CONFIDENCE_LOW",
            Self::Math => "UNVERIFIED_EQUATION",
            Self::Code => "UNVERIFIED_CODE",
        }
    }
}

/// Where a transcript span is, and what produced it.
///
/// The four fields are section 34.1's own display requirement: the session and
/// the version answer *`provider/version`*, and the millisecond range answers
/// *`원음 듣기`* by naming the audio interval a player seeks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptLocator {
    session: LectureSessionId,
    version: TranscriptVersionId,
    starts_at: TimestampMillis,
    ends_at: TimestampMillis,
}

impl TranscriptLocator {
    /// A transcript locator.
    #[must_use]
    pub const fn new(
        session: LectureSessionId,
        version: TranscriptVersionId,
        starts_at: TimestampMillis,
        ends_at: TimestampMillis,
    ) -> Self {
        Self {
            session,
            version,
            starts_at,
            ends_at,
        }
    }

    /// Which lecture session.
    #[must_use]
    pub const fn session(&self) -> LectureSessionId {
        self.session
    }

    /// Which transcript version. A corrected version is a second version, so
    /// naming it is what keeps a span attached to the reading it was found in.
    #[must_use]
    pub const fn version(&self) -> TranscriptVersionId {
        self.version
    }

    /// The audio instant the span starts at.
    #[must_use]
    pub const fn starts_at(&self) -> TimestampMillis {
        self.starts_at
    }

    /// The audio instant the span ends at.
    #[must_use]
    pub const fn ends_at(&self) -> TimestampMillis {
        self.ends_at
    }
}

/// Where an equation or code span is, and which image backs it.
///
/// Section 34.1 requires the `source image` beside an `UNVERIFIED_EQUATION` or
/// `UNVERIFIED_CODE`. The image is named by the document it belongs to, the
/// page it is on and the digest of its bytes — the digest is what lets a viewer
/// fetch exactly that image and lets a reader verify it is the one the span was
/// found in, without this crate holding a pixel of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentRegionLocator {
    session: LectureSessionId,
    document: LectureDocumentId,
    page: u32,
    source_image: ContentDigest,
}

impl DocumentRegionLocator {
    /// A document-region locator.
    #[must_use]
    pub const fn new(
        session: LectureSessionId,
        document: LectureDocumentId,
        page: u32,
        source_image: ContentDigest,
    ) -> Self {
        Self {
            session,
            document,
            page,
            source_image,
        }
    }

    /// Which lecture session.
    #[must_use]
    pub const fn session(&self) -> LectureSessionId {
        self.session
    }

    /// Which document.
    #[must_use]
    pub const fn document(&self) -> LectureDocumentId {
        self.document
    }

    /// Which page.
    #[must_use]
    pub const fn page(&self) -> u32 {
        self.page
    }

    /// The digest of the source image, which is the whole of what this crate
    /// holds about it.
    #[must_use]
    pub const fn source_image(&self) -> ContentDigest {
        self.source_image
    }
}

/// One low-confidence span, with the context that reaches its source.
///
/// The kind is the variant, and the variant decides which locator the span
/// carries. A transcript span cannot carry a page and an equation span cannot
/// carry an audio range, because neither is a field of the other's arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowConfidenceSpan {
    /// An uncertain transcribed span, with the audio it came from.
    Transcript {
        /// Where in the audio.
        locator: TranscriptLocator,
        /// The recogniser's confidence for the span.
        confidence: ConfidencePermille,
    },
    /// An equation the transcription may have flattened.
    Math {
        /// Which page of which document, and the image behind it.
        locator: DocumentRegionLocator,
        /// The extractor's confidence for the span.
        confidence: ConfidencePermille,
    },
    /// A code span the transcription may have reflowed.
    Code {
        /// Which page of which document, and the image behind it.
        locator: DocumentRegionLocator,
        /// The extractor's confidence for the span.
        confidence: ConfidencePermille,
    },
}

impl LowConfidenceSpan {
    /// Which of the three kinds, read off the variant.
    #[must_use]
    pub const fn kind(&self) -> SpanKind {
        match self {
            Self::Transcript { .. } => SpanKind::Transcript,
            Self::Math { .. } => SpanKind::Math,
            Self::Code { .. } => SpanKind::Code,
        }
    }

    /// The confidence displayed beside the span.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        match self {
            Self::Transcript { confidence, .. }
            | Self::Math { confidence, .. }
            | Self::Code { confidence, .. } => *confidence,
        }
    }

    /// The lecture session every kind of span reaches back to.
    ///
    /// The three arms answer it from different locators, which is the point:
    /// there is one question a reader asks of every span, and three different
    /// contexts that answer it.
    #[must_use]
    pub const fn session(&self) -> LectureSessionId {
        match self {
            Self::Transcript { locator, .. } => locator.session(),
            Self::Math { locator, .. } | Self::Code { locator, .. } => locator.session(),
        }
    }
}

/// The low-confidence review queue.
///
/// Section 25.7 lists it as one queue on the lecture surface — *`Mark Moments,
/// low-confidence, equation/code review queue`* — so the three kinds are held
/// together and partitioned on read rather than kept in three lists.
#[derive(Debug, Clone, Default)]
pub struct LowConfidenceQueue {
    spans: Vec<LowConfidenceSpan>,
}

impl LowConfidenceQueue {
    /// An empty queue.
    #[must_use]
    pub const fn new() -> Self {
        Self { spans: Vec::new() }
    }

    /// Queues one span.
    pub fn queue(&mut self, span: LowConfidenceSpan) {
        self.spans.push(span);
    }

    /// Every span, in queueing order.
    #[must_use]
    pub fn spans(&self) -> &[LowConfidenceSpan] {
        &self.spans
    }

    /// Exactly the spans of one kind.
    #[must_use]
    pub fn of_kind(&self, kind: SpanKind) -> Vec<&LowConfidenceSpan> {
        self.spans
            .iter()
            .filter(|span| span.kind() == kind)
            .collect()
    }
}
