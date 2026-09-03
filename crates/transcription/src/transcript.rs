//! Section 12.4's record, and the raw half of it that nothing can write.
//!
//! # Why the raw token has no write path
//!
//! [`RawToken`] has private fields, no `&mut self` method, no setter and no
//! `From`. [`RawSegment`] hands out `&[RawToken]` and never `&mut [RawToken]`,
//! and [`RawTranscript`] has no `&mut self` method at all. The one producer of
//! all three is [`decode`], whose call sites are counted, so a raw token that
//! exists came out of a provider response and nothing since has touched it.
//!
//! That is a claim about types, and `raw_token_write_protection` checks it as
//! one: it sweeps **every package in `crates/`** for a public signature that
//! takes a raw value and returns something mutable or a raw value built from
//! parts, because these types are public and another crate could otherwise
//! declare the accessor this one does not. `a_label_has_no_path_that_moves_a_mark`
//! in `academic-capture` is the shape.
//!
//! # Where a correction goes instead
//!
//! Onto [`crate::version::TranscriptLineage`], as a new version over an
//! annotation layer. A `TranscriptSegment` read at version *n* returns
//! [`EffectiveToken`]s, each of which carries the raw token it is derived from
//! **and** what the current version reads, so a reader can always see both. The
//! raw token is the same value at every version.

use core::fmt;

use academic_domain::{ContentDigest, LectureSessionId};
use academic_model_run::{ModelVersion, ProviderId, RawScore};

use crate::{
    authorize::be_len,
    fault::DecodeFault,
    provider::{
        CapabilityField, ConfidenceSemantics, ProviderContract, Support, TimestampSemantics,
    },
    response::{ProviderResponse, RawResponseId},
};

/// The banner every provider response opens with.
pub const RESPONSE_BANNER: &str = "academic-stt-response/1";

/// Who spoke, in section 12.4's own three shapes.
///
/// `student_unknown_2` in the specification is a student whose identity is not
/// resolved *and* who is distinguishable from another such student; the ordinal
/// is that distinction and nothing more. It is not an identity, and `P2-L5` is
/// where student voice and the PII hold live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Speaker {
    /// The instructor.
    Instructor,
    /// A student, distinguished from other students by an ordinal only.
    StudentUnknown(u32),
    /// The provider attributed the speech to nobody.
    Unresolved,
}

impl Speaker {
    /// Parses section 12.4's spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "instructor" => Some(Self::Instructor),
            "unresolved" => Some(Self::Unresolved),
            other => other
                .strip_prefix("student_unknown_")
                .and_then(|ordinal| ordinal.parse::<u32>().ok())
                .map(Self::StudentUnknown),
        }
    }

    /// The spelling section 12.4 uses.
    #[must_use]
    pub fn spelling(self) -> String {
        match self {
            Self::Instructor => "instructor".to_owned(),
            Self::StudentUnknown(ordinal) => format!("student_unknown_{ordinal}"),
            Self::Unresolved => "unresolved".to_owned(),
        }
    }

    /// Whether naming this speaker is an act of diarization.
    #[must_use]
    pub const fn is_attributed(self) -> bool {
        match self {
            Self::Instructor | Self::StudentUnknown(_) => true,
            Self::Unresolved => false,
        }
    }
}

/// One token exactly as a provider produced it.
///
/// Private fields, no mutating method, one producer.
#[derive(Clone, PartialEq, Eq)]
pub struct RawToken {
    text: String,
    start_nanos: Option<u64>,
    confidence: Option<RawScore>,
}

impl RawToken {
    /// The token's text, as the provider wrote it.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// When it starts, if the contract declared word timestamps.
    #[must_use]
    pub const fn start_nanos(&self) -> Option<u64> {
        self.start_nanos
    }

    /// The provider's own number, if the contract declared per-token
    /// confidence.
    ///
    /// It is an `academic_model_run::RawScore`, which implements neither
    /// `PartialOrd` nor `Ord` and has no accessor returning its units. So one
    /// provider's token confidence cannot be ranked against another's, which is
    /// `P2-M1`'s rule and not a second one written here.
    #[must_use]
    pub const fn confidence(&self) -> Option<&RawScore> {
        self.confidence.as_ref()
    }
}

// A token is a word the user's lecturer said. `S-10`'s decision for this crate
// is the strengthening one: the text reaches the formatter through a length
// only, and the type is registered in `SECRET_BEARING_TYPES`.
impl fmt::Debug for RawToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawToken")
            .field("byte_len", &self.text.len())
            .field("start_nanos", &self.start_nanos)
            .finish()
    }
}

/// One raw segment, as a provider produced it.
#[derive(Clone, PartialEq, Eq)]
pub struct RawSegment {
    id: String,
    start_nanos: u64,
    end_nanos: u64,
    speaker: Speaker,
    verbatim_text: String,
    tokens: Vec<RawToken>,
    source_audio_chunks: Vec<u32>,
}

impl RawSegment {
    /// The provider's own identifier for the segment.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// When it starts.
    #[must_use]
    pub const fn start_nanos(&self) -> u64 {
        self.start_nanos
    }

    /// When it ends, exclusive.
    #[must_use]
    pub const fn end_nanos(&self) -> u64 {
        self.end_nanos
    }

    /// Who spoke.
    #[must_use]
    pub const fn speaker(&self) -> Speaker {
        self.speaker
    }

    /// The verbatim text the provider returned for the whole segment.
    #[must_use]
    pub fn verbatim_text(&self) -> &str {
        &self.verbatim_text
    }

    /// Its tokens, in order. There is no `&mut` counterpart.
    #[must_use]
    pub fn tokens(&self) -> &[RawToken] {
        &self.tokens
    }

    /// The journal frame sequences this segment was transcribed from.
    #[must_use]
    pub fn source_audio_chunks(&self) -> &[u32] {
        &self.source_audio_chunks
    }
}

// The verbatim text is the lecture. Same decision as `RawToken`.
impl fmt::Debug for RawSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawSegment")
            .field("id", &self.id)
            .field("start_nanos", &self.start_nanos)
            .field("end_nanos", &self.end_nanos)
            .field("speaker", &self.speaker)
            .field("verbatim_byte_len", &self.verbatim_text.len())
            .field("token_count", &self.tokens.len())
            .field("source_audio_chunks", &self.source_audio_chunks)
            .finish()
    }
}

/// One provider's whole answer, decoded.
///
/// No `&mut self` method exists on this type. A correction is a new version in
/// [`crate::version::TranscriptLineage`], never an edit here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTranscript {
    lecture: LectureSessionId,
    provider: ProviderId,
    model_version: ModelVersion,
    raw_response: RawResponseId,
    input_digest: ContentDigest,
    segments: Vec<RawSegment>,
}

impl RawTranscript {
    /// Which lecture session.
    #[must_use]
    pub const fn lecture(&self) -> LectureSessionId {
        self.lecture
    }

    /// Which provider produced it.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which exact model version produced it.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// The archived raw response it was decoded from.
    #[must_use]
    pub const fn raw_response(&self) -> RawResponseId {
        self.raw_response
    }

    /// The digest of the input manifest the run read.
    #[must_use]
    pub const fn input_digest(&self) -> &ContentDigest {
        &self.input_digest
    }

    /// Its segments, in the order the provider returned them.
    #[must_use]
    pub fn segments(&self) -> &[RawSegment] {
        &self.segments
    }

    /// One digest over every raw token, in order.
    ///
    /// This is what `annotation_layer_separation` compares before and after an
    /// annotation is applied and removed. Length-prefixed rather than
    /// delimited, so a token whose text spells a separator cannot collide with
    /// two tokens that do not.
    #[must_use]
    pub fn token_sequence_digest(&self) -> ContentDigest {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-transcription-raw-tokens-v1\0");
        material.extend_from_slice(&be_len(self.segments.len()));
        for segment in &self.segments {
            material.extend_from_slice(&be_len(segment.id.len()));
            material.extend_from_slice(segment.id.as_bytes());
            material.extend_from_slice(&segment.start_nanos.to_be_bytes());
            material.extend_from_slice(&segment.end_nanos.to_be_bytes());
            material.extend_from_slice(&be_len(segment.tokens.len()));
            for token in &segment.tokens {
                material.extend_from_slice(&be_len(token.text.len()));
                material.extend_from_slice(token.text.as_bytes());
                material.extend_from_slice(&token.start_nanos.unwrap_or(u64::MAX).to_be_bytes());
            }
        }
        ContentDigest::sha256(&material)
    }
}

/// Turns a provider response into a raw transcript, or refuses it.
///
/// The whole path from bytes to segments. It reads the response, the contract
/// that provider declared, and the identity of the run; it takes no ledger, no
/// policy, no transport and no filesystem path.
///
/// # Errors
///
/// A [`DecodeFault`] naming the first rule the response broke.
pub fn decode(
    response: &ProviderResponse,
    contract: &ProviderContract,
    lecture: LectureSessionId,
    raw_response: RawResponseId,
    input_digest: ContentDigest,
) -> Result<RawTranscript, DecodeFault> {
    // The second of two call sites of `response_bytes`. The grammar has to read
    // the bytes it validates; what leaves this function is a closed record of
    // segments and tokens, and no byte of the response itself.
    let text = core::str::from_utf8(response.response_bytes()).map_err(|_| DecodeFault::NotUtf8)?;
    let body = text.strip_suffix('\n').ok_or(DecodeFault::Banner)?;
    let mut lines = body.lines();
    if lines.next() != Some(RESPONSE_BANNER) {
        return Err(DecodeFault::Banner);
    }

    let mut segments: Vec<RawSegment> = Vec::new();
    let mut open: Option<OpenSegment> = None;
    for line in lines {
        let (key, value) = line.split_once(": ").ok_or_else(|| {
            DecodeFault::UnknownKey(line.split(':').next().unwrap_or(line).to_owned())
        })?;
        match key {
            "segment" => {
                if let Some(previous) = open.take() {
                    segments.push(previous.close()?);
                }
                open = Some(OpenSegment::parse(value, contract)?);
            }
            "verbatim" => {
                let segment = open.as_mut().ok_or(DecodeFault::MissingKey("segment"))?;
                if segment.verbatim_text.is_some() {
                    return Err(DecodeFault::DuplicateKey("verbatim"));
                }
                if value.is_empty() || value.chars().any(char::is_control) {
                    return Err(DecodeFault::FieldCount("verbatim"));
                }
                segment.verbatim_text = Some(value.to_owned());
            }
            "word" => {
                let segment = open.as_mut().ok_or(DecodeFault::MissingKey("segment"))?;
                let token = parse_token(value, contract, response)?;
                if let Some(start) = token.start_nanos
                    && (start < segment.start_nanos || start >= segment.end_nanos)
                {
                    return Err(DecodeFault::TokenOutsideSegment);
                }
                segment.tokens.push(token);
            }
            other => return Err(DecodeFault::UnknownKey(other.to_owned())),
        }
    }
    if let Some(previous) = open.take() {
        segments.push(previous.close()?);
    }
    if segments.is_empty() {
        return Err(DecodeFault::NoSegments);
    }
    for pair in segments.windows(2) {
        let [earlier, later] = pair else {
            continue;
        };
        if later.start_nanos < earlier.end_nanos {
            return Err(DecodeFault::SegmentOrder);
        }
    }
    Ok(RawTranscript {
        lecture,
        provider: response.provider().clone(),
        model_version: response.model_version().clone(),
        raw_response,
        input_digest,
        segments,
    })
}

/// A segment whose header has been read and whose body has not closed.
struct OpenSegment {
    id: String,
    start_nanos: u64,
    end_nanos: u64,
    speaker: Speaker,
    verbatim_text: Option<String>,
    tokens: Vec<RawToken>,
    source_audio_chunks: Vec<u32>,
}

impl OpenSegment {
    fn parse(value: &str, contract: &ProviderContract) -> Result<Self, DecodeFault> {
        let fields: Vec<&str> = value.split(' ').filter(|field| !field.is_empty()).collect();
        let [id, start, end, speaker, chunks] = fields.as_slice() else {
            return Err(DecodeFault::FieldCount("segment"));
        };
        let start_nanos: u64 = start
            .parse()
            .map_err(|_| DecodeFault::NotANumber((*start).to_owned()))?;
        let end_nanos: u64 = end
            .parse()
            .map_err(|_| DecodeFault::NotANumber((*end).to_owned()))?;
        if end_nanos <= start_nanos {
            return Err(DecodeFault::SegmentInterval);
        }
        let speaker = Speaker::parse(speaker)
            .ok_or_else(|| DecodeFault::UnknownSpeaker((*speaker).to_owned()))?;
        // A provider that declared no diarization may not attribute speech.
        if speaker.is_attributed() && contract.diarization() != Support::Offered {
            return Err(DecodeFault::ContradictsDeclaration(
                CapabilityField::Diarization,
            ));
        }
        let mut source_audio_chunks = Vec::new();
        for chunk in chunks.split(',') {
            source_audio_chunks.push(
                chunk
                    .parse::<u32>()
                    .map_err(|_| DecodeFault::NotANumber(chunk.to_owned()))?,
            );
        }
        if source_audio_chunks.is_empty() {
            return Err(DecodeFault::FieldCount("segment"));
        }
        Ok(Self {
            id: (*id).to_owned(),
            start_nanos,
            end_nanos,
            speaker,
            verbatim_text: None,
            tokens: Vec::new(),
            source_audio_chunks,
        })
    }

    fn close(self) -> Result<RawSegment, DecodeFault> {
        let verbatim_text = self
            .verbatim_text
            .ok_or(DecodeFault::MissingKey("verbatim"))?;
        if self.tokens.is_empty() {
            return Err(DecodeFault::MissingKey("word"));
        }
        Ok(RawSegment {
            id: self.id,
            start_nanos: self.start_nanos,
            end_nanos: self.end_nanos,
            speaker: self.speaker,
            verbatim_text,
            tokens: self.tokens,
            source_audio_chunks: self.source_audio_chunks,
        })
    }
}

fn parse_token(
    value: &str,
    contract: &ProviderContract,
    response: &ProviderResponse,
) -> Result<RawToken, DecodeFault> {
    let mut fields = value.splitn(3, ' ');
    let start = fields.next().ok_or(DecodeFault::FieldCount("word"))?;
    let confidence = fields.next().ok_or(DecodeFault::FieldCount("word"))?;
    let text = fields.next().ok_or(DecodeFault::FieldCount("word"))?;
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(DecodeFault::FieldCount("word"));
    }

    let start_nanos = match start {
        "-" => None,
        other => Some(
            other
                .parse::<u64>()
                .map_err(|_| DecodeFault::NotANumber(other.to_owned()))?,
        ),
    };
    // A start on a token the contract said carries none, or none on a token the
    // contract said carries one, is a provider whose declaration and whose
    // answer disagree. `REQ-12-025`'s failure mode is exactly that going
    // unnoticed on a provider swap.
    let declares_word_times = contract.timestamp_semantics() == TimestampSemantics::WordAndSegment;
    if start_nanos.is_some() != declares_word_times {
        return Err(DecodeFault::ContradictsDeclaration(
            CapabilityField::TimestampSemantics,
        ));
    }

    let confidence_units = match confidence {
        "-" => None,
        other => Some(
            other
                .parse::<u32>()
                .map_err(|_| DecodeFault::NotANumber(other.to_owned()))?,
        ),
    };
    let declares_token_confidence =
        contract.confidence_semantics() == ConfidenceSemantics::PerToken;
    if confidence_units.is_some() != declares_token_confidence {
        return Err(DecodeFault::ContradictsDeclaration(
            CapabilityField::ConfidenceSemantics,
        ));
    }

    Ok(RawToken {
        text: text.to_owned(),
        start_nanos,
        confidence: confidence_units.map(|units| {
            RawScore::new(
                response.provider().clone(),
                response.model_version().clone(),
                units,
            )
        }),
    })
}
