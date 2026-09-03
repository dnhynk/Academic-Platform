//! Section 29.1's ordered ingestion contract, as nine types and nine functions.
//!
//! ```text
//! discover/fetch/import
//!   → policy and terms check
//!   → immutable raw snapshot + hash
//!   → source metadata and retrieval time
//!   → deterministic parse
//!   → schema validation
//!   → AI proposal where appropriate
//!   → reconciliation/entity resolution
//!   → claim publication or review queue
//! ```
//!
//! # What makes the order strict
//!
//! Each stage's output is a distinct type with private fields and exactly one
//! producer — the stage before it. A caller cannot reach stage six without a
//! [`Parsed`], and the only thing that makes a [`Parsed`] is
//! [`deterministic_parse`]. Skipping a stage is therefore a compile error, and
//! `tests/compile_fail/` observes it.
//!
//! Failing a stage stops the run: every stage returns `Result`, and [`run`]
//! records the stages it reached. `ingestion_stage_order_is_strict` walks
//! [`Stage::ALL`], arranges for each stage in turn to fail, and asserts that
//! nothing was published and that no later stage was reached. It enumerates the
//! stages; it does not assert how many there are.
//!
//! # Where the terms are consulted, and why three times
//!
//! Stage one refuses to *acquire* over a connector the ledger does not permit.
//! Stage two is section 29.1's recorded policy decision about the bytes that
//! were acquired. Stage nine reads the ledger once more, immediately before
//! publication, which is what makes `IN06` — a permission withdrawn during a
//! run — stop this run rather than the next one.

use academic_domain::engines::RuleId;
use academic_untrusted_content::{IngestError, IngestedDocument, SourceId, SourceKind, Untrusted};

use crate::{
    conflict::{ConflictCase, ContendingSource, detect},
    dating::Dating,
    document::{OfficialDocument, ParseError, SchemaError, parse, validate},
    fetch::{ConditionalFetch, ConditionalRequest, FetchOutcome},
    identifier::{ConnectorId, ProgramKey},
    manifest::{ConnectorManifest, DeclaredTarget, LastSuccess, RetrievalInstant},
    publish::{Publication, PublishableRules, QueueReason, ReviewQueued, publish},
    snapshot::{RawSnapshot, SnapshotError, store},
    terms::{Denial, DenialReason, TermsLedger, deny},
};

/// One stage of section 29.1's contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// Acquire the bytes.
    DiscoverFetchImport,
    /// Decide, and record, whether policy and terms permit using them.
    PolicyAndTermsCheck,
    /// Store them whole, with their hash.
    ImmutableRawSnapshot,
    /// Attach where they came from and when.
    SourceMetadataAndRetrievalTime,
    /// Read them, the same way every time.
    DeterministicParse,
    /// Check the reading against the schema.
    SchemaValidation,
    /// Ask a model, where a model is the right thing to ask.
    AiProposalWhereAppropriate,
    /// Match what was read against what is already known.
    ReconciliationAndEntityResolution,
    /// Publish claims, or hand the document to a person.
    ClaimPublicationOrReviewQueue,
}

impl Stage {
    /// Section 29.1's order.
    pub const ALL: [Self; 9] = [
        Self::DiscoverFetchImport,
        Self::PolicyAndTermsCheck,
        Self::ImmutableRawSnapshot,
        Self::SourceMetadataAndRetrievalTime,
        Self::DeterministicParse,
        Self::SchemaValidation,
        Self::AiProposalWhereAppropriate,
        Self::ReconciliationAndEntityResolution,
        Self::ClaimPublicationOrReviewQueue,
    ];

    /// The stage's line in section 29.1's block, verbatim.
    ///
    /// `the_stage_list_is_section_29_1s_own` reads the block out of the
    /// specification and requires these to be its lines, in this order.
    #[must_use]
    pub const fn spec_line(self) -> &'static str {
        match self {
            Self::DiscoverFetchImport => "discover/fetch/import",
            Self::PolicyAndTermsCheck => "policy and terms check",
            Self::ImmutableRawSnapshot => "immutable raw snapshot + hash",
            Self::SourceMetadataAndRetrievalTime => "source metadata and retrieval time",
            Self::DeterministicParse => "deterministic parse",
            Self::SchemaValidation => "schema validation",
            Self::AiProposalWhereAppropriate => "AI proposal where appropriate",
            Self::ReconciliationAndEntityResolution => "reconciliation/entity resolution",
            Self::ClaimPublicationOrReviewQueue => "claim publication or review queue",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DiscoverFetchImport => "DISCOVER_FETCH_IMPORT",
            Self::PolicyAndTermsCheck => "POLICY_AND_TERMS_CHECK",
            Self::ImmutableRawSnapshot => "IMMUTABLE_RAW_SNAPSHOT",
            Self::SourceMetadataAndRetrievalTime => "SOURCE_METADATA_AND_RETRIEVAL_TIME",
            Self::DeterministicParse => "DETERMINISTIC_PARSE",
            Self::SchemaValidation => "SCHEMA_VALIDATION",
            Self::AiProposalWhereAppropriate => "AI_PROPOSAL_WHERE_APPROPRIATE",
            Self::ReconciliationAndEntityResolution => "RECONCILIATION_AND_ENTITY_RESOLUTION",
            Self::ClaimPublicationOrReviewQueue => "CLAIM_PUBLICATION_OR_REVIEW_QUEUE",
        }
    }
}

/// Position in this profile's ingest order.
///
/// Origin order only. It is not the wall clock — that is
/// [`RetrievalInstant`] — and it is not valid time, which is
/// [`crate::dating::EffectiveDate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IngestSeq {
    seq: u64,
}

impl IngestSeq {
    /// A position.
    #[must_use]
    pub const fn at(seq: u64) -> Self {
        Self { seq }
    }

    /// The position.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.seq
    }
}

/// Why a stage did not produce its output.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FailureReason {
    /// The caller's transport produced nothing.
    #[error("the transport produced nothing: {0}")]
    Transport(String),
    /// Policy or terms refused.
    #[error("refused: {}", .0.reason().as_str())]
    Refused(Denial),
    /// The bytes did not become a snapshot.
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    /// The bytes did not parse.
    #[error(transparent)]
    Parse(#[from] ParseError),
    /// The reading failed the schema.
    #[error(transparent)]
    Schema(#[from] SchemaError),
    /// The document could not be sealed for a model.
    #[error(transparent)]
    Seal(#[from] IngestError),
    /// Entity resolution found no such programme.
    #[error("the corpus knows no programme {0}")]
    UnknownProgram(String),
}

/// The stage that failed, and why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{} failed: {reason}", .stage.as_str())]
pub struct StageFailure {
    stage: Stage,
    reason: FailureReason,
}

impl StageFailure {
    /// Which stage.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        self.stage
    }

    /// Why.
    #[must_use]
    pub const fn reason(&self) -> &FailureReason {
        &self.reason
    }

    /// The denial, when policy or terms refused.
    ///
    /// This is how a caller reaches the fallbacks: every [`Denial`] carries
    /// the whole of `Fallback::ALL`.
    #[must_use]
    pub const fn denial(&self) -> Option<&Denial> {
        match &self.reason {
            FailureReason::Refused(denial) => Some(denial),
            _ => None,
        }
    }
}

/// How the bytes were acquired.
pub enum Acquisition<'run> {
    /// A conditional request answered by the caller's transport.
    Fetch {
        /// The caller's transport. This crate implements none.
        transport: &'run dyn ConditionalFetch,
        /// The request, which names a declared document.
        request: ConditionalRequest,
    },
    /// Bytes a person handed over: a paste, an export, a saved file.
    ///
    /// This is what the four fallbacks produce. Nothing about it is automated,
    /// and the outcome it carries is built by the caller from what the person
    /// supplied.
    Import {
        /// Which declared document the bytes are.
        target: DeclaredTarget,
        /// The bytes and what is known about them.
        outcome: FetchOutcome,
    },
}

impl core::fmt::Debug for Acquisition<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Fetch { request, .. } => formatter
                .debug_struct("Fetch")
                .field("request", request)
                .finish_non_exhaustive(),
            Self::Import { target, .. } => formatter
                .debug_struct("Import")
                .field("target", target)
                .finish_non_exhaustive(),
        }
    }
}

/// Stage one's output.
#[derive(Debug)]
pub struct Fetched {
    connector: ConnectorId,
    target: DeclaredTarget,
    outcome: FetchOutcome,
}

impl Fetched {
    /// What the source answered.
    #[must_use]
    pub const fn outcome(&self) -> &FetchOutcome {
        &self.outcome
    }
}

/// Stage two's output.
#[derive(Debug)]
pub struct TermsCleared {
    fetched: Fetched,
}

/// Stage three's output.
#[derive(Debug)]
pub struct Snapshotted {
    snapshot: RawSnapshot,
}

impl Snapshotted {
    /// The stored snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &RawSnapshot {
        &self.snapshot
    }
}

/// Stage four's output.
#[derive(Debug)]
pub struct Described {
    snapshot: RawSnapshot,
    ingest_seq: IngestSeq,
}

impl Described {
    /// The stored snapshot, with its metadata attached.
    #[must_use]
    pub const fn snapshot(&self) -> &RawSnapshot {
        &self.snapshot
    }

    /// Where this document sits in the ingest order.
    #[must_use]
    pub const fn ingest_seq(&self) -> IngestSeq {
        self.ingest_seq
    }

    /// Takes the snapshot out, for a caller that wants the record rather than
    /// the rest of the pipeline.
    #[must_use]
    pub fn into_snapshot(self) -> RawSnapshot {
        self.snapshot
    }
}

/// Stage five's output.
#[derive(Debug)]
pub struct Parsed {
    described: Described,
    document: OfficialDocument,
}

impl Parsed {
    /// The reading.
    #[must_use]
    pub const fn document(&self) -> &OfficialDocument {
        &self.document
    }

    /// The snapshot it was read from.
    #[must_use]
    pub const fn snapshot(&self) -> &RawSnapshot {
        self.described.snapshot()
    }
}

/// Stage six's output.
#[derive(Debug)]
pub struct Validated {
    parsed: Parsed,
}

impl Validated {
    /// The reading.
    #[must_use]
    pub const fn document(&self) -> &OfficialDocument {
        self.parsed.document()
    }
}

/// Whether a model is the right thing to ask about this document.
#[derive(Debug, Clone)]
pub enum Appropriateness {
    /// It is not. A regulation this crate's parser reads deterministically has
    /// nothing a model should be asked, and the stage produces no proposal.
    NotAppropriate,
    /// It is. The snapshot is sealed with `P2-G5`'s label and the caller's
    /// declared source kind, and what the model may then be shown is decided by
    /// that crate rather than this one.
    SealForModel {
        /// The identifier the sealed document is indexed under.
        source_id: SourceId,
        /// Which of `P2-G5`'s source kinds these bytes are.
        kind: SourceKind,
    },
}

/// Stage seven's output.
#[derive(Debug)]
pub struct Proposed {
    validated: Validated,
    sealed: Option<Untrusted<IngestedDocument>>,
}

impl Proposed {
    /// The reading.
    #[must_use]
    pub const fn document(&self) -> &OfficialDocument {
        self.validated.document()
    }

    /// The sealed document, when a model was the right thing to ask.
    ///
    /// The label is `P2-G5`'s. This crate does not unwrap it and could not:
    /// the accessor is `pub(crate)` to `academic-untrusted-content`.
    #[must_use]
    pub const fn sealed(&self) -> Option<&Untrusted<IngestedDocument>> {
        self.sealed.as_ref()
    }
}

/// What is already known, for stage eight to reconcile against.
#[derive(Debug, Clone, Default)]
pub struct Corpus {
    programs: Vec<ProgramKey>,
    contenders: Vec<ContendingSource>,
}

impl Corpus {
    /// An empty corpus. It knows no programme, so every document fails stage
    /// eight until one is declared.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            programs: Vec::new(),
            contenders: Vec::new(),
        }
    }

    /// Declares a programme this profile knows.
    #[must_use]
    pub fn knowing(mut self, program: ProgramKey) -> Self {
        self.programs.push(program);
        self
    }

    /// Adds an already-published source that may disagree with a new one.
    #[must_use]
    pub fn with_contender(mut self, contender: ContendingSource) -> Self {
        self.contenders.push(contender);
        self
    }
}

/// Stage eight's output.
#[derive(Debug)]
pub struct Reconciled {
    proposed: Proposed,
    connector: ConnectorId,
    retrieved_at: RetrievalInstant,
    conflicts: Vec<ConflictCase>,
}

impl Reconciled {
    /// The reading.
    #[must_use]
    pub const fn document(&self) -> &OfficialDocument {
        self.proposed.document()
    }

    /// The open conflict cases this document raised.
    #[must_use]
    pub fn conflicts(&self) -> &[ConflictCase] {
        &self.conflicts
    }

    /// The value stage nine needs to publish, when the document states when it
    /// applies.
    ///
    /// `None` for `UNSCOPED_OFFICIAL_SOURCE`. The only producer of
    /// [`PublishableRules`], which is the only argument [`publish`] takes.
    #[must_use]
    pub fn publishable(&self) -> Option<PublishableRules<'_>> {
        match self.document().dating() {
            Dating::Unscoped => None,
            Dating::Effective(effective) => Some(PublishableRules::new(
                self.document(),
                &self.connector,
                effective,
                self.retrieved_at,
            )),
        }
    }
}

/// Stage one. Acquires the bytes.
///
/// A fetch is refused when the ledger does not permit one over this connector,
/// and again when the declared cadence does not permit one yet: section 29.2
/// asks for a *low-frequency* conditional fetch, and a cadence nothing compares
/// against a clock is a declaration rather than a limit.
///
/// An import is refused by neither. A person handing over a file they already
/// have is what the four fallbacks are, and a cadence is a rule about how often
/// this system asks a source — not about how often a person may hand over a
/// file.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::DiscoverFetchImport`] when the terms do not
/// permit a fetch, when the declared cadence does not permit one at `now`, or
/// when the caller's transport produced nothing.
pub fn discover_fetch_import(
    manifest: &ConnectorManifest,
    ledger: &TermsLedger,
    now: RetrievalInstant,
    acquisition: Acquisition<'_>,
) -> Result<Fetched, StageFailure> {
    let connector = manifest.connector().clone();
    match acquisition {
        Acquisition::Fetch { transport, request } => {
            let status = ledger.status(&connector);
            if !status.permits_a_fetch() {
                return Err(StageFailure {
                    stage: Stage::DiscoverFetchImport,
                    reason: FailureReason::Refused(deny(connector, reason_for(status))),
                });
            }
            // The cadence. `LastSuccess::Never` has nothing to count from, and
            // `AllowedFrequency::OnUserRequestOnly` has no schedule to be early
            // for -- a run *is* the user asking. Both permit.
            if let LastSuccess::At(last) = manifest.last_success()
                && let Some(earliest) = manifest.allowed_frequency().earliest_next(last)
                && now.seconds() < earliest.seconds()
            {
                return Err(StageFailure {
                    stage: Stage::DiscoverFetchImport,
                    reason: FailureReason::Refused(deny(connector, DenialReason::TooSoon)),
                });
            }
            let target = request.target();
            let outcome = transport.fetch(&request).map_err(|detail| StageFailure {
                stage: Stage::DiscoverFetchImport,
                reason: FailureReason::Transport(detail),
            })?;
            Ok(Fetched {
                connector,
                target,
                outcome,
            })
        }
        Acquisition::Import { target, outcome } => Ok(Fetched {
            connector,
            target,
            outcome,
        }),
    }
}

/// Stage two. Records whether policy and terms permit using what arrived.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::PolicyAndTermsCheck`] when the ledger does not
/// permit the connector, or when the target is not one the manifest declares.
pub fn policy_and_terms_check(
    fetched: Fetched,
    manifest: &ConnectorManifest,
    ledger: &TermsLedger,
) -> Result<TermsCleared, StageFailure> {
    if !manifest.declares(fetched.target) {
        return Err(StageFailure {
            stage: Stage::PolicyAndTermsCheck,
            reason: FailureReason::Refused(deny(
                fetched.connector.clone(),
                DenialReason::UndeclaredTarget,
            )),
        });
    }
    let status = ledger.status(&fetched.connector);
    if !status.permits_a_fetch() {
        return Err(StageFailure {
            stage: Stage::PolicyAndTermsCheck,
            reason: FailureReason::Refused(deny(fetched.connector.clone(), reason_for(status))),
        });
    }
    Ok(TermsCleared { fetched })
}

/// Stage three. Stores the bytes whole, under their hash.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::ImmutableRawSnapshot`] for `IN01`, and for a
/// not-modified response, which creates no version.
pub fn immutable_raw_snapshot(
    cleared: TermsCleared,
    manifest: &ConnectorManifest,
) -> Result<Snapshotted, StageFailure> {
    let Fetched {
        connector,
        target,
        outcome,
    } = cleared.fetched;
    let snapshot =
        store(connector, target, manifest.parser_version(), outcome).map_err(|error| {
            StageFailure {
                stage: Stage::ImmutableRawSnapshot,
                reason: FailureReason::Snapshot(error),
            }
        })?;
    Ok(Snapshotted { snapshot })
}

/// Stage four. Attaches where the bytes came from and when.
///
/// The declaration's own verification date is checked here against the
/// retrieval instant: a connector that keeps succeeding against terms nobody
/// re-read is the failure section 29.1 asks the `next verification` field to
/// catch, and this is where the field is read.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::SourceMetadataAndRetrievalTime`] when the
/// connector's declaration is overdue at the retrieval instant.
pub fn source_metadata_and_retrieval_time(
    snapshotted: Snapshotted,
    manifest: &ConnectorManifest,
    ingest_seq: IngestSeq,
) -> Result<Described, StageFailure> {
    let snapshot = snapshotted.snapshot;
    if manifest
        .next_verification()
        .is_overdue(snapshot.retrieved_at())
    {
        return Err(StageFailure {
            stage: Stage::SourceMetadataAndRetrievalTime,
            reason: FailureReason::Refused(deny(
                snapshot.connector().clone(),
                DenialReason::DeclarationOverdue,
            )),
        });
    }
    Ok(Described {
        snapshot,
        ingest_seq,
    })
}

/// Stage five. Reads the bytes, the same way every time.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::DeterministicParse`] for any [`ParseError`].
pub fn deterministic_parse(described: Described) -> Result<Parsed, StageFailure> {
    let document = parse(described.snapshot()).map_err(|error| StageFailure {
        stage: Stage::DeterministicParse,
        reason: FailureReason::Parse(error),
    })?;
    Ok(Parsed {
        described,
        document,
    })
}

/// Stage six. Checks the reading against the schema.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::SchemaValidation`] for any [`SchemaError`].
pub fn schema_validation(parsed: Parsed) -> Result<Validated, StageFailure> {
    validate(parsed.document()).map_err(|error| StageFailure {
        stage: Stage::SchemaValidation,
        reason: FailureReason::Schema(error),
    })?;
    Ok(Validated { parsed })
}

/// Stage seven. Asks a model, where a model is the right thing to ask.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::AiProposalWhereAppropriate`] when the snapshot
/// cannot be sealed — bytes that are not UTF-8, or longer than
/// `academic_untrusted_content::MAX_SOURCE_BYTES`.
pub fn ai_proposal_where_appropriate(
    validated: Validated,
    appropriateness: Appropriateness,
) -> Result<Proposed, StageFailure> {
    let sealed = match appropriateness {
        Appropriateness::NotAppropriate => None,
        Appropriateness::SealForModel { source_id, kind } => {
            let seq = validated.parsed.described.ingest_seq().get();
            Some(
                validated
                    .parsed
                    .snapshot()
                    .seal(source_id, kind, seq)
                    .map_err(|error| StageFailure {
                        stage: Stage::AiProposalWhereAppropriate,
                        reason: FailureReason::Seal(error),
                    })?,
            )
        }
    };
    Ok(Proposed { validated, sealed })
}

/// Stage eight. Matches the reading against what is already known.
///
/// Every already-published source that speaks about the same rule, over a scope
/// that is not disjoint, with different text, opens a [`ConflictCase`]. Nothing
/// here picks between them.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::ReconciliationAndEntityResolution`] when the
/// corpus knows no such programme, which is an entity that did not resolve.
pub fn reconciliation_and_entity_resolution(
    proposed: Proposed,
    corpus: &Corpus,
) -> Result<Reconciled, StageFailure> {
    let document = proposed.document();
    let program = document.scope().program();
    if !corpus.programs.contains(program) {
        return Err(StageFailure {
            stage: Stage::ReconciliationAndEntityResolution,
            reason: FailureReason::UnknownProgram(program.as_str().to_owned()),
        });
    }

    let snapshot = proposed.validated.parsed.snapshot();
    let connector = snapshot.connector().clone();
    let retrieved_at = snapshot.retrieved_at();
    let target = snapshot.target();

    let mut conflicts = Vec::new();
    for rule in document.rules() {
        let Some(mine) =
            ContendingSource::from_document(connector.clone(), target, document, rule.id())
        else {
            continue;
        };
        for theirs in &corpus.contenders {
            if let Some(case) = detect(mine.clone(), theirs.clone()) {
                conflicts.push(case);
            }
        }
    }

    Ok(Reconciled {
        proposed,
        connector,
        retrieved_at,
        conflicts,
    })
}

/// Stage nine. Publishes claims, or hands the document to a person.
///
/// # Errors
///
/// [`StageFailure`] at [`Stage::ClaimPublicationOrReviewQueue`] when the
/// ledger no longer permits the connector — `IN06`, a permission withdrawn
/// during the run. The denial carries the four fallbacks and disables the
/// connector.
pub fn claim_publication_or_review_queue(
    reconciled: Reconciled,
    ledger: &TermsLedger,
) -> Result<Publication, StageFailure> {
    let status = ledger.status(&reconciled.connector);
    if !status.permits_a_fetch() {
        return Err(StageFailure {
            stage: Stage::ClaimPublicationOrReviewQueue,
            reason: FailureReason::Refused(deny(reconciled.connector.clone(), reason_for(status))),
        });
    }

    let rules: Vec<RuleId> = reconciled
        .document()
        .rules()
        .iter()
        .map(|rule| rule.id().clone())
        .collect();

    if !reconciled.conflicts.is_empty() {
        return Ok(Publication::Queued(ReviewQueued::new(
            reconciled.connector.clone(),
            QueueReason::UnresolvedConflict,
            rules,
            reconciled.conflicts.clone(),
        )));
    }

    match reconciled.publishable() {
        Some(publishable) => Ok(Publication::Published(publish(publishable))),
        None => Ok(Publication::Queued(ReviewQueued::new(
            reconciled.connector.clone(),
            QueueReason::UnscopedOfficialSource,
            rules,
            Vec::new(),
        ))),
    }
}

/// What one run did.
#[derive(Debug)]
pub struct RunRecord {
    reached: Vec<Stage>,
    outcome: RunOutcome,
}

impl RunRecord {
    /// The stages this run reached, in order.
    #[must_use]
    pub fn reached(&self) -> &[Stage] {
        &self.reached
    }

    /// What it produced.
    #[must_use]
    pub const fn outcome(&self) -> &RunOutcome {
        &self.outcome
    }

    /// What it published, if anything.
    ///
    /// `None` for every halted run and for every run that queued instead. This
    /// is what `ingestion_stage_order_is_strict` reads.
    #[must_use]
    pub const fn published(&self) -> Option<&crate::publish::PublishedRules> {
        match &self.outcome {
            RunOutcome::Completed(publication) => publication.published(),
            RunOutcome::Halted(_) => None,
        }
    }

    /// The failure, when the run halted.
    #[must_use]
    pub const fn failure(&self) -> Option<&StageFailure> {
        match &self.outcome {
            RunOutcome::Halted(failure) => Some(failure),
            RunOutcome::Completed(_) => None,
        }
    }
}

/// How a run ended.
#[derive(Debug)]
pub enum RunOutcome {
    /// It reached stage nine.
    Completed(Publication),
    /// A stage failed and no later stage ran.
    Halted(StageFailure),
}

/// Runs the nine stages in section 29.1's order.
///
/// The first failure ends the run. The record names the stages that were
/// reached, so a caller — and the acceptance test — can see the prefix rather
/// than infer it.
pub fn run(
    manifest: &ConnectorManifest,
    ledger: &TermsLedger,
    corpus: &Corpus,
    now: RetrievalInstant,
    acquisition: Acquisition<'_>,
    ingest_seq: IngestSeq,
    appropriateness: Appropriateness,
) -> RunRecord {
    let mut reached = Vec::new();

    macro_rules! step {
        ($stage:expr, $call:expr) => {{
            reached.push($stage);
            match $call {
                Ok(value) => value,
                Err(failure) => {
                    return RunRecord {
                        reached,
                        outcome: RunOutcome::Halted(failure),
                    };
                }
            }
        }};
    }

    let fetched = step!(
        Stage::DiscoverFetchImport,
        discover_fetch_import(manifest, ledger, now, acquisition)
    );
    let cleared = step!(
        Stage::PolicyAndTermsCheck,
        policy_and_terms_check(fetched, manifest, ledger)
    );
    let snapshotted = step!(
        Stage::ImmutableRawSnapshot,
        immutable_raw_snapshot(cleared, manifest)
    );
    let described = step!(
        Stage::SourceMetadataAndRetrievalTime,
        source_metadata_and_retrieval_time(snapshotted, manifest, ingest_seq)
    );
    let parsed = step!(Stage::DeterministicParse, deterministic_parse(described));
    let validated = step!(Stage::SchemaValidation, schema_validation(parsed));
    let proposed = step!(
        Stage::AiProposalWhereAppropriate,
        ai_proposal_where_appropriate(validated, appropriateness)
    );
    let reconciled = step!(
        Stage::ReconciliationAndEntityResolution,
        reconciliation_and_entity_resolution(proposed, corpus)
    );
    let publication = step!(
        Stage::ClaimPublicationOrReviewQueue,
        claim_publication_or_review_queue(reconciled, ledger)
    );

    RunRecord {
        reached,
        outcome: RunOutcome::Completed(publication),
    }
}

/// The denial reason one terms status produces.
fn reason_for(status: crate::terms::TermsStatus) -> DenialReason {
    match status {
        crate::terms::TermsStatus::PermittedForDeclaredMethod
        | crate::terms::TermsStatus::Unreviewed => DenialReason::TermsUnreviewed,
        crate::terms::TermsStatus::Refused => DenialReason::TermsRefuse,
        crate::terms::TermsStatus::Revoked => DenialReason::TermsRevoked,
    }
}
