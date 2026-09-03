//! Every refusal this crate makes, in one closed vocabulary.

use crate::{provider::CapabilityField, route::RouteDenial};

/// Why an input was refused before anything read it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputFault {
    /// The journal's header names another capability token or another policy
    /// row than the capture that is opening the binding holds.
    #[error("that journal was not written by this capture")]
    JournalIsNotThisCapture,
    /// The journal was opened under a different capability token or a
    /// different policy row than the manifest binds to.
    #[error("the journal was recorded under another authorization")]
    ForeignJournal,
    /// No frame in the journal carries that sequence number.
    #[error("the journal has no frame {frame_seq}")]
    NoSuchFrame {
        /// The sequence number that was asked for.
        frame_seq: u32,
    },
    /// The frame is in the journal but holds another body kind.
    #[error("frame {frame_seq} is not the kind of input that was admitted")]
    WrongFrameKind {
        /// The sequence number that was asked for.
        frame_seq: u32,
    },
    /// A supplied material was attributed to something other than the user.
    #[error("supplied material is the user's own act and no other actor's")]
    MaterialNotUserSupplied,
    /// The material identifier was empty or held a byte outside the charset.
    #[error("a material identifier is 1..=64 bytes of [A-Za-z0-9._-]")]
    MaterialIdentifier,
    /// The same frame or material was admitted twice.
    #[error("the manifest already holds that input")]
    DuplicateInput,
    /// A job was planned over a manifest holding nothing.
    #[error("a transcription job reads at least one input")]
    EmptyManifest,
}

/// Why a provider capability contract was refused at registration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityFault {
    /// The draft left a declaration out. An omitted declaration is not the
    /// same as a declared `Unsupported`, which is why the draft uses `Option`.
    #[error("{0} was not declared")]
    Undeclared(CapabilityField),
    /// A declared identifier was empty.
    #[error("{0} was declared empty")]
    Empty(CapabilityField),
    /// The chunk boundary declared a zero-length window or overlap past it.
    #[error("a chunk boundary is a non-zero window with an overlap below it")]
    ChunkBoundary,
    /// Two contracts were registered for one provider and model version.
    #[error("that provider and model version already declared a contract")]
    AlreadyDeclared,
}

/// Why a pipeline stage halted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PipelineFault {
    /// An input was refused.
    #[error(transparent)]
    Input(#[from] InputFault),
    /// The route denied the transcription.
    #[error(transparent)]
    Route(#[from] RouteDenial),
    /// No contract is registered for the provider the route selected.
    #[error("no capability contract is registered for that provider and model version")]
    NoCapabilityContract,
    /// The declared contract does not cover what the request needs.
    #[error("{0} is declared unsupported and the request depends on it")]
    CapabilityUnsupported(CapabilityField),
    /// The provider failed.
    #[error("the provider returned no response")]
    ProviderFailed,
    /// The response's placement, provider or model version is not the one the
    /// route admitted.
    #[error("the response does not answer the route that was admitted")]
    RouteMismatch,
    /// A scoped-remote run recorded no transmission, so nothing reconciles
    /// against `egress_audit`.
    #[error("a scoped-remote run records the ranges the egress transmitted")]
    NoTransmissionRecord,
    /// A local run carried a transmission record, which would claim bytes left
    /// a machine nothing left.
    #[error("a local run transmits nothing and records no range")]
    LocalRunTransmitted,
    /// The response did not parse as the wire grammar.
    #[error(transparent)]
    Decode(#[from] DecodeFault),
    /// Sealing the raw response as untrusted content failed.
    #[error("the raw response could not be sealed as untrusted content")]
    NotSealable,
}

/// Why a provider response was refused by the wire grammar.
///
/// Each variant is produced by one case of
/// `a_malformed_provider_response_is_refused`, and each case is required to be
/// produced -- the set is compared, not counted. The grammar is closed on every
/// axis a partially-read response could become a partially-populated
/// transcript on: an unknown key, a missing key, a duplicate key, a field count
/// that is not the record's, every ordering rule between the numbers, and every
/// place the response could contradict the contract its provider declared.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeFault {
    /// The bytes were not UTF-8.
    #[error("a provider response is UTF-8")]
    NotUtf8,
    /// The first line was not the version banner.
    #[error("a provider response opens with the version banner")]
    Banner,
    /// A line named a key the grammar does not have.
    #[error("unknown key `{0}`")]
    UnknownKey(String),
    /// A record left a key out.
    #[error("missing key `{0}`")]
    MissingKey(&'static str),
    /// A record named a key twice.
    #[error("duplicate key `{0}`")]
    DuplicateKey(&'static str),
    /// A line did not hold the field count its record requires.
    #[error("a `{0}` line does not hold the fields its record requires")]
    FieldCount(&'static str),
    /// A number did not parse.
    #[error("`{0}` is not a number the grammar accepts")]
    NotANumber(String),
    /// A segment ended at or before it started.
    #[error("a segment ends after it starts")]
    SegmentInterval,
    /// A token started outside the segment that holds it.
    #[error("a token starts inside the segment that holds it")]
    TokenOutsideSegment,
    /// Two segments were out of order, or overlapped.
    #[error("segments arrive in order and do not overlap")]
    SegmentOrder,
    /// The response declared no segment at all.
    #[error("a provider response carries at least one segment")]
    NoSegments,
    /// A speaker spelling section 12.4 does not have.
    #[error("`{0}` is not one of section 12.4's speaker spellings")]
    UnknownSpeaker(String),
    /// The response carries something the provider's contract declared it does
    /// not produce, or omits something the contract declared it does.
    #[error("the response contradicts the declared {0}")]
    ContradictsDeclaration(CapabilityField),
}

/// Why a correction, an annotation, or a version was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VersionFault {
    /// The correction named a token no version of the transcript holds.
    #[error("no token at segment {segment} position {position}")]
    NoSuchToken {
        /// Which segment the correction named.
        segment: usize,
        /// Which token position inside it.
        position: usize,
    },
    /// The correction named a version that is not in this lineage.
    #[error("that version is not in this lineage")]
    ForeignVersion,
    /// The correction was applied to a version that a later one supersedes.
    #[error("corrections extend the newest version, not an earlier one")]
    NotTheNewestVersion,
    /// A correction proposed the text that is already there.
    #[error("a correction changes the text it names")]
    NoChange,
    /// The correction's replacement text was empty or held a control byte.
    #[error("replacement text is non-empty and holds no control byte")]
    ReplacementText,
    /// The annotation named a range no segment covers.
    #[error("no segment covers that annotation range")]
    AnnotationRange,
    /// The disposition recorded is not one that settles a correction.
    #[error("a correction is settled by a recorded disposition")]
    NotSettled,
}
