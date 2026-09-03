//! The `P2-L3` transcription pipeline: provider-neutral, input-authorized, and
//! versioned over a raw layer nothing can write.
//!
//! Section 12.3 of the authoritative specification is the pipeline; section
//! 12.4 is the record it produces. This crate holds five things and nothing
//! else:
//!
//! * [`InputManifest`] -- the three input kinds section 12.3's first line
//!   names, each admitted against the capability token a capture journal's own
//!   header carries;
//! * [`ProviderContract`] -- the eight technical declarations section 12.3's
//!   last paragraph requires, with omission distinguished from a declared
//!   absence;
//! * [`SttPolicy`] -- default local, scoped remote, everything else blocked;
//! * [`RawResponseArchive`] -- every raw provider response, append-only, each
//!   sealed under `P2-G5`'s `Untrusted<IngestedDocument>`; and
//! * [`TranscriptLineage`] -- corrections as versions over an annotation layer,
//!   with the raw token the same value at every version.
//!
//! # What this crate is not
//!
//! **It records nothing and transcribes nothing.** There is no speech engine
//! in this repository, no implementation of [`SttProvider`] ships, and every
//! fixture in this crate's test tree is a committed literal. Nothing here opens
//! a microphone, a file or a socket.
//!
//! **It persists nothing.** There is no `academic-store` edge and this task
//! adds no migration: `0008`, `0010` and `0011` are still unclaimed and `0013`
//! is unwritten. The durable half of a capture is `P2-L2`'s chain-digested
//! chunk journal, which this crate reads and never writes.
//!
//! **It runs no sandbox.** `P2-G4` is a dependency of this task in the plan and
//! it is a *sequencing* one as built: `academic-worker` carries a sandbox probe
//! binary and `phase1-scaffold-policy.test.mjs` requires that **no** workspace
//! crate depend on that package, because the probe would then be reachable from
//! a default build. `P2-G2`'s precedent is to split rather than to weaken a
//! guard, so this crate is a sibling. What a provider process would need is
//! that crate's job descriptor, and nothing here forges one.
//!
//! **It stages and transmits nothing.** The scoped-remote route produces a
//! [`RemoteAdmission`]; turning one into bytes on a wire is
//! `academic-egress-boundary`'s, behind `P2-G1`'s broker. This crate names no
//! socket construct and implements no `OutboundTransport`.
//!
//! # Where the section 38 gate stands
//!
//! `GATE-38-019` -- cloud transcription per offering -- **stays open.** This
//! crate invents no default for it: [`SttPolicy::new`] approves no remote
//! provider, and there is no configuration file, environment variable or
//! fallback that could supply one.

mod annotation;
mod authorize;
mod compare;
mod fault;
mod pipeline;
mod provider;
mod response;
mod route;
mod transcript;
mod version;

pub use annotation::{Annotation, AnnotationKind, AnnotationLayer};
pub use authorize::{
    AuthorizationBinding, AuthorizedCapture, AuthorizedChunk, InputManifest, SuppliedMaterial,
};
pub use compare::{
    CompareFault, Divergence, ProviderRun, RetranscriptionComparison, Side, compare,
};
pub use fault::{CapabilityFault, DecodeFault, InputFault, PipelineFault, VersionFault};
pub use pipeline::{
    CompletedRun, DownstreamJob, JobHandle, LOCAL_ONLY_RETENTION, ProviderSelection, RunIdentity,
    RunOutcome, RunRecord, Stage, SttProvider, TranscriptionRequest, run,
};
pub use provider::{
    AudioFormat, CapabilityField, ChunkBoundary, ConfidenceSemantics, ContractDraft,
    ContractRegistry, FeatureClaim, ProviderContract, ProviderPlacement, Support,
    TimestampSemantics,
};
pub use response::{
    ArchiveFault, ArchivedResponse, ProviderResponse, RawResponseArchive, RawResponseId,
};
pub use route::{RemoteAdmission, RemoteProcessingApproval, RouteDenial, SttPolicy, SttRoute};
pub use transcript::{RESPONSE_BANNER, RawSegment, RawToken, RawTranscript, Speaker, decode};
pub use version::{
    AppliedCorrection, CorrectionAuthor, CorrectionCandidate, CorrectionStatus, EffectiveToken,
    LineageEffect, SettledCorrection, TokenAddress, TranscriptLineage, TranscriptSegment,
    TranscriptVersion, settles_corrections,
};
