//! `P2-U6`: official source ingestion and change propagation.
//!
//! This crate holds section 29.1's ordered ingestion contract and the four
//! things section 29.2 and section 8.4 say about official sources:
//!
//! * [`stage`] — the nine stages, as nine types and nine functions. A stage's
//!   output is the next stage's input and has one producer, so the order is a
//!   compile error to break and a failed stage stops the run.
//! * [`manifest`] — what a connector declares, with section 29.1's nine fields
//!   required one at a time, and the two rules that are types: a fetch target
//!   is `&'static`, and a credential is bound to one connector's declaration.
//! * [`dating`] and [`publish`] — the effective-date parser, and the fact that
//!   an `UNSCOPED_OFFICIAL_SOURCE` cannot be published because the publisher's
//!   argument type has no value for it.
//! * [`conflict`] — section 8.4's five dimensions, and the winner that does not
//!   exist: a case is unresolved until a person decides, and a dependent audit
//!   is `INDETERMINATE` while it is.
//!
//! [`diff`] and [`graph`] are the change-propagation half: which rules a new
//! reading moves, and which requirements, scenarios and course mappings those
//! rules move in turn.
//!
//! # What this crate is not
//!
//! **It is not a crawler and it holds no transport.** [`fetch::ConditionalFetch`]
//! is a trait the caller implements, the way `academic-egress-boundary` takes
//! its `OutboundTransport`, and this crate implements it nowhere. Nothing here
//! spells a socket construct — `only_egress_crate_has_a_socket` is the
//! workspace-wide statement of that — and this crate's resolved closure
//! intersected with the socket-capable crates is `["libc"]`, reaching it through
//! `academic-domain`, which is the row that scan pins for it.
//!
//! **It does not run a live connector.** `GATE-38-020` is open: which access
//! methods and which frequency each source permits is a user and legal
//! decision, and a connector with no recorded review reads as `UNREVIEWED`,
//! which denies. Every test here is driven by synthetic fixtures.
//!
//! **It contains no browser automation.** `GATE-38-027` is open: where a
//! user-performed export ends and a browser-assisted capture begins is
//! undecided. Phase 2 ships manual import and user-provided export, and the
//! four fallbacks a denial offers are four things a person does.
//!
//! **It declares no trust label of its own.** Bytes leave a snapshot as
//! `academic_untrusted_content::Untrusted<IngestedDocument>` and by no other
//! public route. `P2-G5` owns that label and this crate reuses it.
//!
//! **It persists nothing.** There is no store edge and no migration. The typed
//! rows an ingestion writes belong to whichever aggregate owner writes them;
//! this crate produces the values.

pub mod conflict;
pub mod dating;
pub mod diff;
pub mod document;
pub mod fetch;
pub mod gate;
pub mod graph;
pub mod identifier;
pub mod manifest;
pub mod publish;
pub mod snapshot;
pub mod stage;
pub mod terms;

pub use conflict::{
    AuditDisposition, ConflictCase, ConflictDimension, ContendingSource, DateComparison,
    DimensionFinding, DimensionOutcome, Resolution, Side, UserResolution, detect,
};
pub use dating::{
    Date, DateRelation, Dating, EffectiveDate, IssuanceDate, UNSCOPED_OFFICIAL_SOURCE,
};
pub use diff::{DocumentChange, RuleChange, SourceDiff};
pub use document::{
    AdmissionYear, CohortRange, HierarchyRelation, LegalAuthority, OfficialDocument, ParseError,
    ParsedRule, SUPERIOR_PAIRS, SchemaError, ScopeRelation, TargetScope, TransitionRelation,
    TransitionalMeasures,
};
pub use fetch::{
    ConditionalFetch, ConditionalRequest, FetchOutcome, HeaderValue, HttpMetadata, Validators,
};
pub use gate::{OpenGate, phase2_shipped_fallbacks, unreviewed_status};
pub use graph::{Dependency, DependencyGraph, DependentKind, DependentNode, Invalidation};
pub use identifier::{ConnectorId, DependentId, NameError, ProgramKey, SectionPath};
pub use manifest::{
    AllowedFrequency, AuthenticationMethod, Completeness, ConnectorManifest, CredentialBinding,
    DeclaredTarget, LastSuccess, ManifestDraft, ManifestError, ManifestField, NextVerification,
    ParserVersion, PersonalDataClass, RetrievalInstant, SourceCategory, SourceOwnership,
};
pub use publish::{
    Publication, PublishableRules, PublishedRules, QueueReason, ReviewQueued, publish,
};
pub use snapshot::{RawSnapshot, SnapshotError};
pub use stage::{
    Acquisition, Appropriateness, Corpus, FailureReason, IngestSeq, RunOutcome, RunRecord, Stage,
    StageFailure, run,
};
pub use terms::{Denial, DenialReason, DenialRoute, Fallback, TermsLedger, TermsStatus};
