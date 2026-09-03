//! What a speech-to-text provider has to declare before it may be used, and
//! what a declaration of "unsupported" costs the caller.
//!
//! Section 12.3's last paragraph lists twelve declarations. Eight of them are
//! technical and are this module's: audio format, chunk boundary, language
//! hints, vocabulary hints, word and segment timestamps, confidence semantics,
//! diarization, and math/code capability. The remaining four -- data retention,
//! training use, processing region and deletion receipt -- are **not** restated
//! here: `P2-G3`'s `provider_policy_snapshot` in `academic-policy` already owns
//! them, and a second copy beside it would be a second thing to keep true. What
//! this module takes from that half is [`RemoteProcessingApproval`] in
//! [`crate::route`], which names a snapshot rather than re-declaring its facts.
//!
//! **Omission is not a declaration.** [`ContractDraft`] uses `Option` for every
//! field for one reason: to tell a fact that was left out from a fact that was
//! declared absent. `declare` refuses the first and accepts the second, and a
//! declared absence then travels with the contract, so a caller that needs word
//! timestamps from a provider that declared none is refused by
//! [`ProviderContract::supports`] rather than silently given segment times.

use core::fmt;

use academic_model_run::{ModelVersion, ProviderId};

use crate::fault::CapabilityFault;

/// Where a provider runs.
///
/// The route is read off the contract rather than off the request, so "use the
/// remote one locally" is not a thing a caller can ask for. Two variants and no
/// third; [`crate::route::SttPolicy::route_for`] matches over the whole set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProviderPlacement {
    /// The provider runs on this machine and transmits nothing.
    Local,
    /// The provider runs somewhere else and reaching it is an egress.
    Remote,
}

impl ProviderPlacement {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Local, Self::Remote];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "LOCAL",
            Self::Remote => "REMOTE",
        }
    }
}

/// The eight technical declarations a contract has to carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityField {
    /// The audio container and sampling the provider accepts.
    AudioFormat,
    /// The window a chunk covers and how far consecutive chunks overlap.
    ChunkBoundary,
    /// Whether a caller may bias the decode with expected languages.
    LanguageHints,
    /// Whether a caller may bias the decode with a domain vocabulary.
    VocabularyHints,
    /// What a returned timestamp means.
    TimestampSemantics,
    /// What a returned confidence means.
    ConfidenceSemantics,
    /// Whether the provider attributes speech to speakers.
    Diarization,
    /// Whether the provider claims mathematical or code notation.
    MathAndCode,
}

impl CapabilityField {
    /// Exhaustive order. Every rule below iterates this rather than a literal
    /// count, so a ninth declaration adds a case instead of moving a number.
    pub const ALL: [Self; 8] = [
        Self::AudioFormat,
        Self::ChunkBoundary,
        Self::LanguageHints,
        Self::VocabularyHints,
        Self::TimestampSemantics,
        Self::ConfidenceSemantics,
        Self::Diarization,
        Self::MathAndCode,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AudioFormat => "AUDIO_FORMAT",
            Self::ChunkBoundary => "CHUNK_BOUNDARY",
            Self::LanguageHints => "LANGUAGE_HINTS",
            Self::VocabularyHints => "VOCABULARY_HINTS",
            Self::TimestampSemantics => "TIMESTAMP_SEMANTICS",
            Self::ConfidenceSemantics => "CONFIDENCE_SEMANTICS",
            Self::Diarization => "DIARIZATION",
            Self::MathAndCode => "MATH_AND_CODE",
        }
    }

    /// The section 12.3 phrase this declaration answers.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::AudioFormat => "audio format",
            Self::ChunkBoundary => "chunk boundary",
            Self::LanguageHints => "language hints",
            Self::VocabularyHints => "vocabulary hints",
            Self::TimestampSemantics => "word/segment timestamps",
            Self::ConfidenceSemantics => "confidence semantics",
            Self::Diarization => "diarization",
            Self::MathAndCode => "math/code capability",
        }
    }
}

impl fmt::Display for CapabilityField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The audio a provider accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormat {
    container: String,
    sample_rate_hz: u32,
    channels: u8,
}

impl AudioFormat {
    /// Declares a format.
    #[must_use]
    pub fn new(container: impl Into<String>, sample_rate_hz: u32, channels: u8) -> Self {
        Self {
            container: container.into(),
            sample_rate_hz,
            channels,
        }
    }

    /// The container spelling.
    #[must_use]
    pub fn container(&self) -> &str {
        &self.container
    }

    /// Samples per second.
    #[must_use]
    pub const fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// How many channels.
    #[must_use]
    pub const fn channels(&self) -> u8 {
        self.channels
    }
}

/// How the provider expects audio to be cut, and how far consecutive windows
/// overlap.
///
/// The overlap is a declaration and not a convenience: section 34.1's STT-error
/// *prevention* cell names chunk overlap, and a provider that declares zero has
/// declared that a word on a boundary is at risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkBoundary {
    window_nanos: u64,
    overlap_nanos: u64,
}

impl ChunkBoundary {
    /// Declares a window and its overlap.
    ///
    /// # Errors
    ///
    /// [`CapabilityFault::ChunkBoundary`] for a zero window or an overlap that
    /// is not below it.
    pub const fn new(window_nanos: u64, overlap_nanos: u64) -> Result<Self, CapabilityFault> {
        if window_nanos == 0 || overlap_nanos >= window_nanos {
            return Err(CapabilityFault::ChunkBoundary);
        }
        Ok(Self {
            window_nanos,
            overlap_nanos,
        })
    }

    /// How long one window is.
    #[must_use]
    pub const fn window_nanos(&self) -> u64 {
        self.window_nanos
    }

    /// How far two consecutive windows overlap.
    #[must_use]
    pub const fn overlap_nanos(&self) -> u64 {
        self.overlap_nanos
    }
}

/// A declaration that is either present or explicitly absent.
///
/// This is the type that makes "unsupported" a value rather than a missing
/// field. `Option` in the draft says whether the *declaration* was made;
/// `Support` says what the declaration was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Support {
    /// The provider offers it.
    Offered,
    /// The provider says it does not.
    Unsupported,
}

impl Support {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Offered, Self::Unsupported];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Offered => "OFFERED",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}

/// What a returned timestamp is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimestampSemantics {
    /// Every token carries a start, and so does every segment.
    WordAndSegment,
    /// Only segments carry times.
    SegmentOnly,
    /// The provider returns no time at all.
    None,
}

impl TimestampSemantics {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::WordAndSegment, Self::SegmentOnly, Self::None];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WordAndSegment => "WORD_AND_SEGMENT",
            Self::SegmentOnly => "SEGMENT_ONLY",
            Self::None => "NONE",
        }
    }
}

/// What a returned confidence is attached to.
///
/// The number itself is never comparable across providers: a decoded
/// confidence becomes an `academic_model_run::RawScore`, which implements
/// neither `PartialOrd` nor `Ord` and has no accessor returning its units. What
/// this declaration says is what the provider *attached* it to, which is a
/// different question from what it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfidenceSemantics {
    /// Every token carries one.
    PerToken,
    /// Only segments carry one.
    PerSegment,
    /// The provider returns none.
    None,
}

impl ConfidenceSemantics {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::PerToken, Self::PerSegment, Self::None];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PerToken => "PER_TOKEN",
            Self::PerSegment => "PER_SEGMENT",
            Self::None => "NONE",
        }
    }
}

/// A capability a caller can depend on.
///
/// A request that depends on one the contract declares unsupported is refused
/// at the stage that reads the contract, which is what "explicit unsupported
/// capability that prevents dependent feature claims" means in `t001`'s
/// `REQ-12-021` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeatureClaim {
    /// Per-token start times.
    WordTimestamps,
    /// Per-token confidence.
    TokenConfidence,
    /// Speaker attribution.
    SpeakerLabels,
    /// Mathematical or code notation.
    MathAndCode,
    /// A domain vocabulary biasing the decode.
    VocabularyBias,
    /// An expected-language hint biasing the decode.
    LanguageBias,
}

impl FeatureClaim {
    /// Exhaustive order.
    pub const ALL: [Self; 6] = [
        Self::WordTimestamps,
        Self::TokenConfidence,
        Self::SpeakerLabels,
        Self::MathAndCode,
        Self::VocabularyBias,
        Self::LanguageBias,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WordTimestamps => "WORD_TIMESTAMPS",
            Self::TokenConfidence => "TOKEN_CONFIDENCE",
            Self::SpeakerLabels => "SPEAKER_LABELS",
            Self::MathAndCode => "MATH_AND_CODE",
            Self::VocabularyBias => "VOCABULARY_BIAS",
            Self::LanguageBias => "LANGUAGE_BIAS",
        }
    }

    /// Which declaration decides it.
    #[must_use]
    pub const fn decided_by(self) -> CapabilityField {
        match self {
            Self::WordTimestamps => CapabilityField::TimestampSemantics,
            Self::TokenConfidence => CapabilityField::ConfidenceSemantics,
            Self::SpeakerLabels => CapabilityField::Diarization,
            Self::MathAndCode => CapabilityField::MathAndCode,
            Self::VocabularyBias => CapabilityField::VocabularyHints,
            Self::LanguageBias => CapabilityField::LanguageHints,
        }
    }
}

/// A provider's declared technical contract.
///
/// Private fields and one producer, [`ContractDraft::declare`]. A contract
/// therefore cannot exist with a declaration missing, which is what makes
/// `stt_capability_contract` a construction rule rather than a check somebody
/// has to remember to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderContract {
    provider: ProviderId,
    model_version: ModelVersion,
    placement: ProviderPlacement,
    audio_format: AudioFormat,
    chunk_boundary: ChunkBoundary,
    language_hints: Support,
    vocabulary_hints: Support,
    timestamp_semantics: TimestampSemantics,
    confidence_semantics: ConfidenceSemantics,
    diarization: Support,
    math_and_code: Support,
}

impl ProviderContract {
    /// Which provider declared it.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which exact model version.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// Where the provider runs.
    #[must_use]
    pub const fn placement(&self) -> ProviderPlacement {
        self.placement
    }

    /// The audio it accepts.
    #[must_use]
    pub const fn audio_format(&self) -> &AudioFormat {
        &self.audio_format
    }

    /// The window and overlap it expects.
    #[must_use]
    pub const fn chunk_boundary(&self) -> ChunkBoundary {
        self.chunk_boundary
    }

    /// Whether it takes language hints.
    #[must_use]
    pub const fn language_hints(&self) -> Support {
        self.language_hints
    }

    /// Whether it takes a vocabulary.
    #[must_use]
    pub const fn vocabulary_hints(&self) -> Support {
        self.vocabulary_hints
    }

    /// What its timestamps are attached to.
    #[must_use]
    pub const fn timestamp_semantics(&self) -> TimestampSemantics {
        self.timestamp_semantics
    }

    /// What its confidences are attached to.
    #[must_use]
    pub const fn confidence_semantics(&self) -> ConfidenceSemantics {
        self.confidence_semantics
    }

    /// Whether it attributes speech to speakers.
    #[must_use]
    pub const fn diarization(&self) -> Support {
        self.diarization
    }

    /// Whether it claims mathematical or code notation.
    #[must_use]
    pub const fn math_and_code(&self) -> Support {
        self.math_and_code
    }

    /// Whether a caller may depend on `claim`.
    ///
    /// A total `match` over [`FeatureClaim::ALL`], so a seventh claim stops
    /// this crate compiling until it names the declaration that decides it.
    #[must_use]
    pub const fn supports(&self, claim: FeatureClaim) -> bool {
        match claim {
            FeatureClaim::WordTimestamps => {
                matches!(self.timestamp_semantics, TimestampSemantics::WordAndSegment)
            }
            FeatureClaim::TokenConfidence => {
                matches!(self.confidence_semantics, ConfidenceSemantics::PerToken)
            }
            FeatureClaim::SpeakerLabels => matches!(self.diarization, Support::Offered),
            FeatureClaim::MathAndCode => matches!(self.math_and_code, Support::Offered),
            FeatureClaim::VocabularyBias => matches!(self.vocabulary_hints, Support::Offered),
            FeatureClaim::LanguageBias => matches!(self.language_hints, Support::Offered),
        }
    }
}

/// A contract under construction.
///
/// Every field is an `Option` so that an omitted declaration and a declared
/// absence are different values. [`ContractDraft::declare`] refuses the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractDraft {
    provider: ProviderId,
    model_version: ModelVersion,
    placement: ProviderPlacement,
    audio_format: Option<AudioFormat>,
    chunk_boundary: Option<ChunkBoundary>,
    language_hints: Option<Support>,
    vocabulary_hints: Option<Support>,
    timestamp_semantics: Option<TimestampSemantics>,
    confidence_semantics: Option<ConfidenceSemantics>,
    diarization: Option<Support>,
    math_and_code: Option<Support>,
}

impl ContractDraft {
    /// Opens a draft for one provider and model version.
    #[must_use]
    pub const fn for_provider(
        provider: ProviderId,
        model_version: ModelVersion,
        placement: ProviderPlacement,
    ) -> Self {
        Self {
            provider,
            model_version,
            placement,
            audio_format: None,
            chunk_boundary: None,
            language_hints: None,
            vocabulary_hints: None,
            timestamp_semantics: None,
            confidence_semantics: None,
            diarization: None,
            math_and_code: None,
        }
    }

    /// Declares the audio format.
    #[must_use]
    pub fn audio_format(mut self, value: AudioFormat) -> Self {
        self.audio_format = Some(value);
        self
    }

    /// Declares the chunk boundary.
    #[must_use]
    pub const fn chunk_boundary(mut self, value: ChunkBoundary) -> Self {
        self.chunk_boundary = Some(value);
        self
    }

    /// Declares whether language hints are taken.
    #[must_use]
    pub const fn language_hints(mut self, value: Support) -> Self {
        self.language_hints = Some(value);
        self
    }

    /// Declares whether a vocabulary is taken.
    #[must_use]
    pub const fn vocabulary_hints(mut self, value: Support) -> Self {
        self.vocabulary_hints = Some(value);
        self
    }

    /// Declares what a timestamp is attached to.
    #[must_use]
    pub const fn timestamp_semantics(mut self, value: TimestampSemantics) -> Self {
        self.timestamp_semantics = Some(value);
        self
    }

    /// Declares what a confidence is attached to.
    #[must_use]
    pub const fn confidence_semantics(mut self, value: ConfidenceSemantics) -> Self {
        self.confidence_semantics = Some(value);
        self
    }

    /// Declares whether speech is attributed to speakers.
    #[must_use]
    pub const fn diarization(mut self, value: Support) -> Self {
        self.diarization = Some(value);
        self
    }

    /// Declares whether mathematical or code notation is claimed.
    #[must_use]
    pub const fn math_and_code(mut self, value: Support) -> Self {
        self.math_and_code = Some(value);
        self
    }

    /// Closes the draft into a contract.
    ///
    /// # Errors
    ///
    /// [`CapabilityFault::Undeclared`] naming the first field of
    /// [`CapabilityField::ALL`] that was left out, and
    /// [`CapabilityFault::Empty`] when the audio container is empty.
    pub fn declare(self) -> Result<ProviderContract, CapabilityFault> {
        // The order is `CapabilityField::ALL`'s, so the field a caller is told
        // about is the first missing one in the declared order rather than the
        // first one this function happens to read.
        let audio_format = self
            .audio_format
            .ok_or(CapabilityFault::Undeclared(CapabilityField::AudioFormat))?;
        if audio_format.container.trim().is_empty() {
            return Err(CapabilityFault::Empty(CapabilityField::AudioFormat));
        }
        let chunk_boundary = self
            .chunk_boundary
            .ok_or(CapabilityFault::Undeclared(CapabilityField::ChunkBoundary))?;
        let language_hints = self
            .language_hints
            .ok_or(CapabilityFault::Undeclared(CapabilityField::LanguageHints))?;
        let vocabulary_hints = self.vocabulary_hints.ok_or(CapabilityFault::Undeclared(
            CapabilityField::VocabularyHints,
        ))?;
        let timestamp_semantics = self.timestamp_semantics.ok_or(CapabilityFault::Undeclared(
            CapabilityField::TimestampSemantics,
        ))?;
        let confidence_semantics = self
            .confidence_semantics
            .ok_or(CapabilityFault::Undeclared(
                CapabilityField::ConfidenceSemantics,
            ))?;
        let diarization = self
            .diarization
            .ok_or(CapabilityFault::Undeclared(CapabilityField::Diarization))?;
        let math_and_code = self
            .math_and_code
            .ok_or(CapabilityFault::Undeclared(CapabilityField::MathAndCode))?;
        Ok(ProviderContract {
            provider: self.provider,
            model_version: self.model_version,
            placement: self.placement,
            audio_format,
            chunk_boundary,
            language_hints,
            vocabulary_hints,
            timestamp_semantics,
            confidence_semantics,
            diarization,
            math_and_code,
        })
    }
}

/// Every declared contract, keyed on the exact provider and model version.
///
/// One contract per key. Section 12.3 requires the exact model version to be
/// preserved so a re-transcription can be compared against the run before it,
/// and a registry keyed on the provider alone would have made two versions of
/// one vendor's model indistinguishable at exactly the moment the comparison
/// matters.
#[derive(Debug, Clone, Default)]
pub struct ContractRegistry {
    contracts: Vec<ProviderContract>,
}

impl ContractRegistry {
    /// An empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            contracts: Vec::new(),
        }
    }

    /// Registers a declared contract.
    ///
    /// # Errors
    ///
    /// [`CapabilityFault::AlreadyDeclared`] when that provider and model
    /// version already declared one.
    pub fn declare(&mut self, contract: ProviderContract) -> Result<(), CapabilityFault> {
        if self
            .get(contract.provider(), contract.model_version())
            .is_some()
        {
            return Err(CapabilityFault::AlreadyDeclared);
        }
        self.contracts.push(contract);
        Ok(())
    }

    /// The contract for one provider and model version, if one is declared.
    #[must_use]
    pub fn get(
        &self,
        provider: &ProviderId,
        model_version: &ModelVersion,
    ) -> Option<&ProviderContract> {
        self.contracts.iter().find(|contract| {
            contract.provider() == provider && contract.model_version() == model_version
        })
    }

    /// Every declared contract, in declaration order.
    #[must_use]
    pub fn contracts(&self) -> &[ProviderContract] {
        &self.contracts
    }
}
