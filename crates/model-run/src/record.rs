//! The twelve fields section 27.3 of the authoritative spec gives a model run.
//!
//! Every field is a distinct type and every one is a constructor argument, so a
//! model execution that omits one is a compilation failure rather than a row
//! with a null in it. The field names are the spec's YAML keys in snake case,
//! which is what lets `model_run_requires_every_field` derive the expected set
//! from the spec text instead of transcribing it.

use core::fmt;

use sha2::{Digest, Sha256};

use crate::ModelRunError;

/// Domain separator for the canonical record encoding below.
const RECORD_DIGEST_DOMAIN: &[u8] = b"academic-model-run-record-v1\0";

/// A 32-byte digest: a prompt template, a redaction policy, or exact bytes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest32([u8; 32]);

impl Digest32 {
    /// Wraps an exact 32-byte digest.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Hashes bytes into a digest.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hexadecimal, the spelling `academic-policy` stores.
    #[must_use]
    pub fn to_lower_hex(&self) -> String {
        let mut rendered = String::with_capacity(64);
        for byte in self.0 {
            rendered.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
            rendered.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
        }
        rendered
    }
}

impl fmt::Debug for Digest32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_lower_hex())
    }
}

/// A 16-byte aggregate identifier, the width every canonical closure table uses.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ModelRunId([u8; 16]);

impl ModelRunId {
    /// Wraps an exact 16-byte identifier.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The raw identifier bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// A 16-byte artifact identifier.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ArtifactId([u8; 16]);

impl ArtifactId {
    /// Wraps an exact 16-byte identifier.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The raw identifier bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Declares a non-empty identifier newtype with one fallible constructor.
macro_rules! nonempty_text {
    ($name:ident, $what:literal) => {
        #[doc = concat!("The ", $what, " a model run records.")]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub struct $name(String);

        impl $name {
            #[doc = concat!("Constructs a non-empty ", $what, ".")]
            pub fn new(value: impl Into<String>) -> Result<Self, ModelRunError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(ModelRunError::EmptyField(stringify!($name)));
                }
                Ok(Self(value))
            }

            /// The recorded value.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

nonempty_text!(Purpose, "purpose");
nonempty_text!(ProviderId, "provider");
nonempty_text!(ModelVersion, "model version");
nonempty_text!(RetentionDeclaration, "retention declaration");
nonempty_text!(EgressGrantId, "egress grant identifier");

/// One artifact a model run read, with the digest of the bytes it read.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct InputArtifactRef {
    artifact_id: ArtifactId,
    content_digest: Digest32,
}

impl InputArtifactRef {
    /// Pairs an artifact with the digest of the exact bytes the run read.
    #[must_use]
    pub const fn new(artifact_id: ArtifactId, content_digest: Digest32) -> Self {
        Self {
            artifact_id,
            content_digest,
        }
    }

    /// The artifact identifier.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// The digest of the bytes read.
    #[must_use]
    pub const fn content_digest(&self) -> &Digest32 {
        &self.content_digest
    }
}

/// A model run's input artifacts. A run that read nothing has no provenance, so
/// the list is non-empty by construction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InputArtifactRefs(Vec<InputArtifactRef>);

impl InputArtifactRefs {
    /// Constructs a non-empty, ordinal-ordered input list.
    pub fn new(refs: Vec<InputArtifactRef>) -> Result<Self, ModelRunError> {
        if refs.is_empty() {
            return Err(ModelRunError::NoInputArtifacts);
        }
        Ok(Self(refs))
    }

    /// The inputs in ordinal order.
    #[must_use]
    pub fn as_slice(&self) -> &[InputArtifactRef] {
        &self.0
    }
}

/// One half-open byte range a model run transmitted.
///
/// The four values are the four `audit_artifact_range` carries, so a range here
/// and an audited range there compare directly.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TransmittedRange {
    object_id: String,
    start: u64,
    end: u64,
    content_digest: Digest32,
}

impl TransmittedRange {
    /// Constructs a non-empty half-open range over a named object.
    pub fn new(
        object_id: impl Into<String>,
        start: u64,
        end: u64,
        content_digest: Digest32,
    ) -> Result<Self, ModelRunError> {
        let object_id = object_id.into();
        if object_id.is_empty() || start >= end {
            return Err(ModelRunError::InvalidRange);
        }
        Ok(Self {
            object_id,
            start,
            end,
            content_digest,
        })
    }

    /// The object the range names.
    #[must_use]
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// Inclusive start.
    #[must_use]
    pub const fn start(&self) -> u64 {
        self.start
    }

    /// Exclusive end.
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.end
    }

    /// Digest of the exact bytes named.
    #[must_use]
    pub const fn content_digest(&self) -> &Digest32 {
        &self.content_digest
    }

    /// The number of bytes this range covers.
    #[must_use]
    pub const fn length(&self) -> u64 {
        self.end - self.start
    }
}

/// What left the machine, recorded as a value rather than as an absence.
///
/// A local model transmits nothing, and `LocalOnly` is how a run says so. That
/// is not the same as an unset field: the reconciliation reads it and refuses a
/// `LocalOnly` run whose input bytes turn up in an egress audit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Transmission {
    /// Nothing left the machine.
    LocalOnly,
    /// Bytes left under one egress grant, in exactly these ranges.
    Egressed {
        /// The `egress_grant.grant_id` the transfer spent.
        grant_id: EgressGrantId,
        /// The exact ranges transmitted, in ordinal order, never empty.
        ranges: Vec<TransmittedRange>,
    },
}

impl Transmission {
    /// Constructs an egressed transmission, refusing an empty range list.
    ///
    /// An egress with no range is a transmission nothing describes, so the type
    /// refuses it rather than storing a claim the reconciliation cannot check.
    pub fn egressed(
        grant_id: EgressGrantId,
        ranges: Vec<TransmittedRange>,
    ) -> Result<Self, ModelRunError> {
        if ranges.is_empty() {
            return Err(ModelRunError::EgressWithoutRanges);
        }
        Ok(Self::Egressed { grant_id, ranges })
    }

    /// The stored `transmission_kind` spelling.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::LocalOnly => "LOCAL_ONLY",
            Self::Egressed { .. } => "EGRESSED",
        }
    }

    /// The transmitted ranges; empty exactly when nothing left the machine.
    #[must_use]
    pub fn ranges(&self) -> &[TransmittedRange] {
        match self {
            Self::LocalOnly => &[],
            Self::Egressed { ranges, .. } => ranges,
        }
    }

    /// The grant spent, when one was.
    #[must_use]
    pub const fn grant_id(&self) -> Option<&EgressGrantId> {
        match self {
            Self::LocalOnly => None,
            Self::Egressed { grant_id, .. } => Some(grant_id),
        }
    }
}

/// What a model run cost, in integer micro-units of a named currency.
///
/// Integers, not a floating-point amount, for the reason
/// `no_float_reaches_the_gpa_path` gives: a value that is summed and compared
/// has to be exact.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Cost {
    micros: u64,
    currency: String,
}

impl Cost {
    /// Constructs a cost from micro-units and a three-letter currency code.
    pub fn new(micros: u64, currency: impl Into<String>) -> Result<Self, ModelRunError> {
        let currency = currency.into();
        if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(ModelRunError::InvalidCurrency);
        }
        Ok(Self { micros, currency })
    }

    /// Micro-units spent.
    #[must_use]
    pub const fn micros(&self) -> u64 {
        self.micros
    }

    /// Three-letter currency code.
    #[must_use]
    pub fn currency(&self) -> &str {
        &self.currency
    }
}

/// One model execution, carrying the twelve fields section 27.3 fixes.
///
/// The fields are private and the only constructor takes all twelve, so a run
/// that omits one does not compile. `model_run_requires_every_field` compares
/// this field list against the spec's own YAML keys and against migration
/// `0007`'s storage sites, in both directions.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ModelRun {
    id: ModelRunId,
    purpose: Purpose,
    provider: ProviderId,
    model_version: ModelVersion,
    prompt_template_hash: Digest32,
    input_artifact_refs: InputArtifactRefs,
    transmitted_byte_ranges: Transmission,
    redaction_policy_hash: Digest32,
    output_artifact: ArtifactId,
    started_at: u64,
    cost: Cost,
    retention_declaration: RetentionDeclaration,
}

impl ModelRun {
    /// Records one model execution. Every section 27.3 field is an argument.
    #[expect(
        clippy::too_many_arguments,
        reason = "the arguments are the twelve section 27.3 fields; a shorter list would be a field this type does not require"
    )]
    #[must_use]
    pub const fn record(
        id: ModelRunId,
        purpose: Purpose,
        provider: ProviderId,
        model_version: ModelVersion,
        prompt_template_hash: Digest32,
        input_artifact_refs: InputArtifactRefs,
        transmitted_byte_ranges: Transmission,
        redaction_policy_hash: Digest32,
        output_artifact: ArtifactId,
        started_at: u64,
        cost: Cost,
        retention_declaration: RetentionDeclaration,
    ) -> Self {
        Self {
            id,
            purpose,
            provider,
            model_version,
            prompt_template_hash,
            input_artifact_refs,
            transmitted_byte_ranges,
            redaction_policy_hash,
            output_artifact,
            started_at,
            cost,
            retention_declaration,
        }
    }

    /// The run identifier.
    #[must_use]
    pub const fn id(&self) -> &ModelRunId {
        &self.id
    }

    /// The declared purpose.
    #[must_use]
    pub const fn purpose(&self) -> &Purpose {
        &self.purpose
    }

    /// The provider that executed the run.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// The exact model version.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// Digest of the prompt template used.
    #[must_use]
    pub const fn prompt_template_hash(&self) -> &Digest32 {
        &self.prompt_template_hash
    }

    /// The artifacts read.
    #[must_use]
    pub const fn input_artifact_refs(&self) -> &InputArtifactRefs {
        &self.input_artifact_refs
    }

    /// What left the machine.
    #[must_use]
    pub const fn transmitted_byte_ranges(&self) -> &Transmission {
        &self.transmitted_byte_ranges
    }

    /// Digest of the redaction policy that produced the transmitted bytes.
    #[must_use]
    pub const fn redaction_policy_hash(&self) -> &Digest32 {
        &self.redaction_policy_hash
    }

    /// The artifact the run produced.
    #[must_use]
    pub const fn output_artifact(&self) -> &ArtifactId {
        &self.output_artifact
    }

    /// Start time in milliseconds.
    #[must_use]
    pub const fn started_at(&self) -> u64 {
        self.started_at
    }

    /// What the run cost.
    #[must_use]
    pub const fn cost(&self) -> &Cost {
        &self.cost
    }

    /// The retention declaration the run was made under.
    #[must_use]
    pub const fn retention_declaration(&self) -> &RetentionDeclaration {
        &self.retention_declaration
    }

    /// Canonical digest over all twelve fields.
    ///
    /// This is the value migration `0007` requires to equal the
    /// `MODEL_RUN_RECORDED` event's `source_digest`, which is what binds the
    /// typed row to the signed event. It covers the child rows too, so a
    /// reader that rebuilds it from the persisted parent and children detects an
    /// edited child that the parent's trigger could not have seen.
    #[must_use]
    pub fn record_digest(&self) -> Digest32 {
        let mut hasher = Sha256::new();
        hasher.update(RECORD_DIGEST_DOMAIN);
        hasher.update(self.id.as_bytes());
        push_text(&mut hasher, self.purpose.as_str());
        push_text(&mut hasher, self.provider.as_str());
        push_text(&mut hasher, self.model_version.as_str());
        hasher.update(self.prompt_template_hash.as_bytes());
        push_count(&mut hasher, self.input_artifact_refs.as_slice().len());
        for input in self.input_artifact_refs.as_slice() {
            hasher.update(input.artifact_id().as_bytes());
            hasher.update(input.content_digest().as_bytes());
        }
        push_text(&mut hasher, self.transmitted_byte_ranges.kind());
        push_text(
            &mut hasher,
            self.transmitted_byte_ranges
                .grant_id()
                .map_or("", EgressGrantId::as_str),
        );
        push_count(&mut hasher, self.transmitted_byte_ranges.ranges().len());
        for range in self.transmitted_byte_ranges.ranges() {
            push_text(&mut hasher, range.object_id());
            hasher.update(range.start().to_be_bytes());
            hasher.update(range.end().to_be_bytes());
            hasher.update(range.content_digest().as_bytes());
        }
        hasher.update(self.redaction_policy_hash.as_bytes());
        hasher.update(self.output_artifact.as_bytes());
        hasher.update(self.started_at.to_be_bytes());
        hasher.update(self.cost.micros().to_be_bytes());
        push_text(&mut hasher, self.cost.currency());
        push_text(&mut hasher, self.retention_declaration.as_str());
        Digest32::from_bytes(hasher.finalize().into())
    }
}

fn push_text(hasher: &mut Sha256, value: &str) {
    push_count(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn push_count(hasher: &mut Sha256, count: usize) {
    hasher.update(u64::try_from(count).unwrap_or(u64::MAX).to_be_bytes());
}
