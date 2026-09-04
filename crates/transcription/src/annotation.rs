//! The annotation layer, which is separate so that nothing has to edit a raw
//! token to make a transcript readable.
//!
//! Section 12.4 names four things that live here and not in the token stream:
//! punctuation, paragraphs, speaker labels, and mathematical formatting. Each
//! is an [`Annotation`] naming a range of raw tokens and a rendering; none of
//! them holds a raw token, and applying or removing one cannot change
//! [`crate::RawTranscript::token_sequence_digest`], because the layer has no
//! reference to the transcript at all -- it takes one to validate a range and
//! keeps nothing.
//!
//! # This layer is the one that may be thrown away
//!
//! The raw transcript is append-only under ADR-003 and a correction is a new
//! version. The annotation layer is the opposite on purpose: it is derived, so
//! it can be removed and rebuilt, and `annotation_layer_separation` observes
//! exactly that -- apply each kind, remove each kind, rebuild the layer, and
//! find the raw token digest identical at every step and the rebuilt layer's
//! digest equal to the first one's.

use academic_domain::ContentDigest;

use crate::{authorize::be_len, fault::VersionFault, transcript::RawTranscript};

/// The four things section 12.4 puts outside the token stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnnotationKind {
    /// A punctuation mark rendered between or after tokens.
    Punctuation,
    /// A paragraph boundary.
    Paragraph,
    /// A label naming who is speaking.
    SpeakerLabel,
    /// Mathematical notation rendered over a token range.
    MathFormatting,
}

impl AnnotationKind {
    /// Exhaustive order. `annotation_layer_separation` drives this rather than
    /// a literal count, so a fifth kind adds a case.
    pub const ALL: [Self; 4] = [
        Self::Punctuation,
        Self::Paragraph,
        Self::SpeakerLabel,
        Self::MathFormatting,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Punctuation => "PUNCTUATION",
            Self::Paragraph => "PARAGRAPH",
            Self::SpeakerLabel => "SPEAKER_LABEL",
            Self::MathFormatting => "MATH_FORMATTING",
        }
    }

    /// The section 12.4 phrase this kind answers.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::Punctuation => "문장부호",
            Self::Paragraph => "문단",
            Self::SpeakerLabel => "화자 label",
            Self::MathFormatting => "수식 formatting",
        }
    }
}

/// One annotation over a half-open range of raw tokens.
#[derive(Clone, PartialEq, Eq)]
pub struct Annotation {
    kind: AnnotationKind,
    segment: usize,
    first_token: usize,
    token_count: usize,
    rendering: String,
}

// A rendering is written over a range of raw tokens, and section 12.4's
// `MathFormatting` kind renders notation the lecturer wrote. One
// caller-supplied `String` carries all four kinds and the type is public, so
// `S-10`'s decision here is the strengthening one this crate already made for
// `RawToken` and `AppliedCorrection`: the text reaches the formatter through a
// length only.
impl core::fmt::Debug for Annotation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Annotation")
            .field("kind", &self.kind)
            .field("segment", &self.segment)
            .field("first_token", &self.first_token)
            .field("token_count", &self.token_count)
            .field("rendering_byte_len", &self.rendering.len())
            .finish()
    }
}

impl Annotation {
    /// Names an annotation over `[first_token, first_token + token_count)` of
    /// one segment.
    #[must_use]
    pub fn over(
        kind: AnnotationKind,
        segment: usize,
        first_token: usize,
        token_count: usize,
        rendering: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            segment,
            first_token,
            token_count,
            rendering: rendering.into(),
        }
    }

    /// Which of the four kinds.
    #[must_use]
    pub const fn kind(&self) -> AnnotationKind {
        self.kind
    }

    /// Which segment.
    #[must_use]
    pub const fn segment(&self) -> usize {
        self.segment
    }

    /// The first raw token it covers.
    #[must_use]
    pub const fn first_token(&self) -> usize {
        self.first_token
    }

    /// How many raw tokens it covers.
    #[must_use]
    pub const fn token_count(&self) -> usize {
        self.token_count
    }

    /// What it renders as.
    #[must_use]
    pub fn rendering(&self) -> &str {
        &self.rendering
    }
}

/// Every annotation over one transcript.
///
/// Derived state. It can be emptied and rebuilt, which is what makes it the
/// layer a formatting change belongs in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationLayer {
    annotations: Vec<Annotation>,
}

impl AnnotationLayer {
    /// An empty layer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            annotations: Vec::new(),
        }
    }

    /// Adds one annotation, after checking its range against the raw
    /// transcript.
    ///
    /// The transcript is borrowed for the check and not kept: this type holds
    /// no raw token and no reference to one.
    ///
    /// # Errors
    ///
    /// [`VersionFault::AnnotationRange`] when no segment covers the range.
    pub fn apply(
        &mut self,
        raw: &RawTranscript,
        annotation: Annotation,
    ) -> Result<(), VersionFault> {
        let segment = raw
            .segments()
            .get(annotation.segment)
            .ok_or(VersionFault::AnnotationRange)?;
        let end = annotation
            .first_token
            .checked_add(annotation.token_count)
            .ok_or(VersionFault::AnnotationRange)?;
        if annotation.token_count == 0 || end > segment.tokens().len() {
            return Err(VersionFault::AnnotationRange);
        }
        self.annotations.push(annotation);
        Ok(())
    }

    /// Removes every annotation of one kind and reports how many went.
    #[must_use]
    pub fn remove_kind(&mut self, kind: AnnotationKind) -> usize {
        let before = self.annotations.len();
        self.annotations
            .retain(|annotation| annotation.kind != kind);
        before.saturating_sub(self.annotations.len())
    }

    /// Every annotation, in the order they were applied.
    #[must_use]
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Every annotation of one kind.
    #[must_use]
    pub fn of_kind(&self, kind: AnnotationKind) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|annotation| annotation.kind == kind)
            .collect()
    }

    /// Whether the layer holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }

    /// One digest over the whole layer, in application order.
    ///
    /// A rebuilt layer that applied the same annotations in the same order has
    /// the same digest, which is what "independently rebuildable" is checked
    /// against.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-transcription-annotation-layer-v1\0");
        material.extend_from_slice(&be_len(self.annotations.len()));
        for annotation in &self.annotations {
            material.extend_from_slice(annotation.kind.as_str().as_bytes());
            material.push(0);
            material.extend_from_slice(&be_len(annotation.segment));
            material.extend_from_slice(&be_len(annotation.first_token));
            material.extend_from_slice(&be_len(annotation.token_count));
            material.extend_from_slice(&be_len(annotation.rendering.len()));
            material.extend_from_slice(annotation.rendering.as_bytes());
        }
        ContentDigest::sha256(&material)
    }
}
