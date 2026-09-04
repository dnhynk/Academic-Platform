//! The coding-assistant handoff: selected ranges only, recorded provenance, and
//! no competency.
//!
//! Section 33's last row keeps generated-code provenance and forbids treating
//! assistant use as competency. Section 35's first row rejects an app in which
//! the AI does the studying.
//!
//! ## Selected ranges only
//!
//! [`AssistantContext::minimize`] does not slice anything itself. It builds
//! `P2-G2`'s `StagingRequest` with the caller's selected symbols as the focus
//! and hands it to that crate's pipeline, which reduces a whole document to the
//! brace-balanced declarations of exactly those symbols, refuses a symbol the
//! document does not declare with `SCOPE_MISMATCH`, and scans what is left. So
//! "only the selected ranges" is the egress boundary's own minimization step
//! rather than a second implementation of it, and the bytes an assistant
//! receives are the previewed bytes because that crate re-hashes them at the
//! capability boundary.
//!
//! ## Provenance
//!
//! [`GeneratedCode::record`] takes `P2-M1`'s [`ModelRun`] by reference and
//! stores its record digest. There is no constructor that takes a run
//! identifier alone, so a record cannot name a run nobody wrote, and there is
//! no constructor that takes no run at all -- `generated_code_provenance_is_recorded`
//! reads every field back and `crates/integrations/tests/compile_fail/` holds
//! the diagnostic for the version that omits one.
//!
//! ## Why assistant use is not competency
//!
//! [`AssistantUse::eligibility`] is a total `match` returning
//! [`EvidenceEligibility`], whose only variant is `NotEvidence`. That is the
//! same shape `P2-R1` gave `TokenPermission::access`: an assistant use that
//! counted as evidence would have to return a variant this enum does not have.
//!
//! The structural half is stronger and is where the real claim lives. This
//! crate has **no edge of any kind** to `academic-competency` or
//! `academic-repository-competency`, and its product source names no mastery,
//! no rubric and no competency type. `assistant_use_is_not_competency` reads
//! the transitive product closure out of the workspace manifests and compares
//! the whole set of `academic_*` paths and `use` items this crate spells
//! against pinned inventories in both directions, so a reach for that
//! vocabulary appears as an extra key rather than as a token somebody
//! remembered to forbid. It is `P2-R5`'s rule -- unmodified generated code
//! makes no `APPLIED` claim -- and `P2-Y1`'s -- a dependency being present
//! fills no rubric cell -- expressed as a property of the dependency graph.
//!
//! [`ModelRun`]: academic_model_run::ModelRun

use academic_domain::TimestampMillis;
use academic_egress_boundary::{
    EgressDenial, EgressProxy, IdentifierPolicy, SourceDocument, StagedPayload, StagingRequest,
};
use academic_model_run::{Digest32, ModelRun, ModelRunId};

/// Why an assistant handoff was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AssistantError {
    /// The selection named no symbol.
    #[error("an assistant selection names at least one symbol")]
    EmptySelection,
    /// The selection named a symbol twice, or named an empty symbol.
    #[error("an assistant selection names each non-empty symbol once")]
    MalformedSelection,
}

/// What an assistant is being used for.
///
/// Five uses, and none of them is evidence of anything the user knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssistantUse {
    /// Selected context was handed over and nothing came back yet.
    ContextHandoff,
    /// Code was generated.
    GeneratedCode,
    /// An explanation of existing code was produced.
    Explanation,
    /// A refactor was proposed.
    Refactor,
    /// A test was drafted.
    TestDraft,
}

impl AssistantUse {
    /// Exhaustive order.
    pub const ALL: [Self; 5] = [
        Self::ContextHandoff,
        Self::GeneratedCode,
        Self::Explanation,
        Self::Refactor,
        Self::TestDraft,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContextHandoff => "CONTEXT_HANDOFF",
            Self::GeneratedCode => "GENERATED_CODE",
            Self::Explanation => "EXPLANATION",
            Self::Refactor => "REFACTOR",
            Self::TestDraft => "TEST_DRAFT",
        }
    }

    /// What this use is evidence of.
    ///
    /// Exhaustive, and every arm is [`EvidenceEligibility::NotEvidence`],
    /// because that is the only variant there is.
    #[must_use]
    pub const fn eligibility(self) -> EvidenceEligibility {
        match self {
            Self::ContextHandoff
            | Self::GeneratedCode
            | Self::Explanation
            | Self::Refactor
            | Self::TestDraft => EvidenceEligibility::NotEvidence,
        }
    }
}

/// What an assistant use may be admitted as.
///
/// One variant. A second one would have to be *returned by an arm* of
/// [`AssistantUse::eligibility`] to change anything, and
/// `assistant_use_is_not_competency` walks [`AssistantUse::ALL`] and reads this
/// enum's whole variant list out of this file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceEligibility {
    /// Not evidence. Section 33's boundary column for this row.
    NotEvidence,
}

impl EvidenceEligibility {
    /// Exhaustive order.
    pub const ALL: [Self; 1] = [Self::NotEvidence];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotEvidence => "NOT_EVIDENCE",
        }
    }
}

/// The symbols a user explicitly selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantSelection {
    symbols: Vec<String>,
}

impl AssistantSelection {
    /// Takes the selected symbol names.
    ///
    /// # Errors
    ///
    /// [`AssistantError::EmptySelection`] when the list is empty and
    /// [`AssistantError::MalformedSelection`] when a name is empty or repeated.
    /// A repeat is refused rather than deduplicated, because a selection is
    /// what the user pointed at and silently changing it is the thing this type
    /// exists to prevent.
    pub fn new(symbols: Vec<String>) -> Result<Self, AssistantError> {
        if symbols.is_empty() {
            return Err(AssistantError::EmptySelection);
        }
        let mut sorted = symbols.clone();
        sorted.sort();
        sorted.dedup();
        if sorted.len() != symbols.len() || symbols.iter().any(String::is_empty) {
            return Err(AssistantError::MalformedSelection);
        }
        Ok(Self { symbols })
    }

    /// The selected symbols, in the order the user gave them.
    #[must_use]
    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }
}

/// The staged context one assistant call receives.
#[derive(Debug)]
pub struct AssistantContext {
    selection: AssistantSelection,
    staged: StagedPayload,
}

impl AssistantContext {
    /// Minimizes `document` to `selection` through `P2-G2` and stages it.
    ///
    /// Everything that decides which bytes survive is that crate's: the size
    /// bound, the classification, the structural minimization, both scans and
    /// the redaction. This function chooses the focus and nothing else.
    ///
    /// # Errors
    ///
    /// The `P2-G2` denial for whichever step refused, unchanged.
    pub fn minimize(
        proxy: &EgressProxy<'_>,
        document: &SourceDocument,
        selection: &AssistantSelection,
        identifier_policy: &IdentifierPolicy,
        max_bytes: u64,
    ) -> Result<Self, EgressDenial> {
        let staged = proxy.stage(&StagingRequest {
            document,
            focus: selection.symbols(),
            identifier_policy,
            max_bytes,
        })?;
        Ok(Self {
            selection: selection.clone(),
            staged,
        })
    }

    /// The selection this context was built for.
    #[must_use]
    pub const fn selection(&self) -> &AssistantSelection {
        &self.selection
    }

    /// The staged payload, whose preview is the buffer a transport writes.
    #[must_use]
    pub const fn staged(&self) -> &StagedPayload {
        &self.staged
    }
}

/// Code an assistant produced, and where it came from.
///
/// Six private fields and one constructor taking a `P2-M1` [`ModelRun`] by
/// reference, so a record without provenance is not a value that can exist.
///
/// [`ModelRun`]: academic_model_run::ModelRun
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCode {
    model_run: ModelRunId,
    run_digest: Digest32,
    context_digest: Digest32,
    output_digest: Digest32,
    produced_at: TimestampMillis,
    use_kind: AssistantUse,
}

impl GeneratedCode {
    /// Records generated bytes against the run that produced them.
    ///
    /// The context digest is taken from the staged preview rather than from an
    /// argument, so what is recorded is the bytes the assistant actually
    /// received.
    #[must_use]
    pub fn record(
        run: &ModelRun,
        context: &AssistantContext,
        output: &[u8],
        produced_at: TimestampMillis,
        use_kind: AssistantUse,
    ) -> Self {
        Self {
            model_run: *run.id(),
            run_digest: run.record_digest(),
            context_digest: Digest32::of(context.staged().preview().bytes()),
            output_digest: Digest32::of(output),
            produced_at,
            use_kind,
        }
    }

    /// The `P2-M1` run this code came out of.
    #[must_use]
    pub const fn model_run(&self) -> ModelRunId {
        self.model_run
    }

    /// That run's canonical record digest.
    #[must_use]
    pub const fn run_digest(&self) -> Digest32 {
        self.run_digest
    }

    /// Digest of the bytes the assistant received.
    #[must_use]
    pub const fn context_digest(&self) -> Digest32 {
        self.context_digest
    }

    /// Digest of the bytes it produced.
    #[must_use]
    pub const fn output_digest(&self) -> Digest32 {
        self.output_digest
    }

    /// When.
    #[must_use]
    pub const fn produced_at(&self) -> TimestampMillis {
        self.produced_at
    }

    /// What the assistant was used for.
    #[must_use]
    pub const fn use_kind(&self) -> AssistantUse {
        self.use_kind
    }

    /// What this record is evidence of. Never a competency.
    #[must_use]
    pub const fn eligibility(&self) -> EvidenceEligibility {
        self.use_kind.eligibility()
    }
}
