//! Section 12.5's allow-list of preservation transforms.
//!
//! # Why this is a closed set and not a list of refusals
//!
//! The specification names nine things a preservation rendering may do. A guard
//! written as a list of forbidden transforms answers "is this one of the things
//! we thought of", and this run has measured that shape failing five times in a
//! row. [`PreservationTransform`] is the other direction: it is a closed enum,
//! [`PreservationTransform::ALL`] is compared against the specification's own
//! sentence by `lossless_transform_allowlist`, and a transform that is not one
//! of the nine has no spelling a caller could write.
//!
//! # The name is the weaker half
//!
//! A closed enum stops a caller *naming* a deleting transform. It does not stop
//! a caller declaring [`PreservationTransform::Punctuation`] and handing over a
//! rendering with a word missing. So the allow-list runs beside a behavioural
//! rule that does not read the transform at all: a source mapping is admitted
//! only when every token the mapped span covers still occurs, in order, in the
//! rendered text. A deletion and a paraphrase both fail that rule under every
//! one of the nine.

/// One transform section 12.5 permits a preservation rendering to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PreservationTransform {
    /// The source order is the document order.
    OrderPreservation,
    /// Sentence punctuation the provider did not return.
    Punctuation,
    /// A heading introduced over a span.
    SectionHeading,
    /// A source instant rendered beside the text.
    Timestamp,
    /// Who spoke, rendered beside the text.
    SpeakerLabel,
    /// Mathematics and code set as mathematics and code.
    MathAndCodeFormatting,
    /// A term of art marked as one.
    TerminologyMarking,
    /// A repetition or an emphasis annotated as one.
    RepetitionAndEmphasisAnnotation,
    /// A capture placed beside the span it belongs to.
    CapturePlacement,
}

impl PreservationTransform {
    /// Every permitted transform, in the order section 12.5 lists them.
    pub const ALL: [Self; 9] = [
        Self::OrderPreservation,
        Self::Punctuation,
        Self::SectionHeading,
        Self::Timestamp,
        Self::SpeakerLabel,
        Self::MathAndCodeFormatting,
        Self::TerminologyMarking,
        Self::RepetitionAndEmphasisAnnotation,
        Self::CapturePlacement,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderPreservation => "ORDER_PRESERVATION",
            Self::Punctuation => "PUNCTUATION",
            Self::SectionHeading => "SECTION_HEADING",
            Self::Timestamp => "TIMESTAMP",
            Self::SpeakerLabel => "SPEAKER_LABEL",
            Self::MathAndCodeFormatting => "MATH_AND_CODE_FORMATTING",
            Self::TerminologyMarking => "TERMINOLOGY_MARKING",
            Self::RepetitionAndEmphasisAnnotation => "REPETITION_AND_EMPHASIS_ANNOTATION",
            Self::CapturePlacement => "CAPTURE_PLACEMENT",
        }
    }

    /// The phrase section 12.5 uses for this transform, verbatim.
    ///
    /// `lossless_transform_allowlist` reads the specification's sentence and
    /// requires these nine phrases to be exactly its comma-separated members,
    /// in order, so a tenth transform invented here fails against the
    /// specification rather than against a second list written here.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::OrderPreservation => "순서 보존",
            Self::Punctuation => "문장부호",
            Self::SectionHeading => "section heading",
            Self::Timestamp => "timestamp",
            Self::SpeakerLabel => "speaker",
            Self::MathAndCodeFormatting => "수식/코드 formatting",
            Self::TerminologyMarking => "전문용어 표시",
            Self::RepetitionAndEmphasisAnnotation => "반복·강조 annotation",
            Self::CapturePlacement => "capture 배치",
        }
    }

    /// Parses the contract spelling, and nothing else.
    ///
    /// Total over [`PreservationTransform::ALL`]: an unrecognised spelling is
    /// `None` rather than a default.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|transform| transform.as_str() == value)
    }
}
