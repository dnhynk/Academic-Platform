//! What "a privileged action" means here, enumerated rather than counted.
//!
//! `injection_corpus_produces_zero_privileged_actions` is worth nothing if
//! "privileged action" is left to the reader. The fifteen variants below are
//! the list the assertion walks, one assertion per variant, and the test names
//! each one rather than comparing a total.
//!
//! Eleven of them are `academic_policy::ProcessCapability`'s variants, one for
//! one. `privileged_actions_cover_every_process_capability` maps the two through
//! a compiler-checked witness `match`, so a capability added to `P2-G7`'s closed
//! enum stops that suite compiling until it appears here too. The other four are
//! privileged in this repository without being a process capability: installing
//! a policy snapshot, minting a grant, consuming one, and publishing a proposal.
//!
//! # Why there is no `AcceptStagedOutput` variant
//!
//! Accepting a sandboxed worker's staged output is privileged, and it is
//! `P2-G4`'s `StagingAuthority::accept`. It is not on this list because the
//! control run below could not perform it: `only_egress_crate_has_a_socket`
//! refuses a workspace crate that depends on `academic-worker` by any edge kind,
//! so this crate's tests cannot reach that boundary. A variant with no control
//! is a zero nobody observed, which is the shape this task exists to refuse.
//! What that boundary refuses is `P2-G4`'s
//! `pj02_output_that_fails_validation_is_quarantined_not_accepted`.
//!
//! # What the enumeration is, and is not
//!
//! It is the list the corpus test asserts zero of, and it is checked against the
//! capability enum so it cannot quietly fall behind. It is not a claim that this
//! crate could perform any of them: [`crate::adjudicate`] takes an index and an
//! output and nothing else, `academic-policy` is a dev edge rather than a
//! product one, and both facts are pinned. The [`ActionLedger`] is what makes
//! the zero observed rather than assumed, and the control run in the same test
//! -- an operator performing each of the fifteen through the same ledger -- is
//! what makes the zero mean something.

/// One action this repository treats as privileged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrivilegedAction {
    /// Obtain a microphone or camera handle.
    CaptureDevice,
    /// Write into the core-owned staging boundary.
    WriteStagedArtifact,
    /// Read named artifact ranges.
    ReadArtifactRange,
    /// Write a search projection.
    WriteSearchIndex,
    /// Run repository analysis.
    AnalyzeRepository,
    /// Borrow a scoped connector credential.
    BorrowConnectorCredential,
    /// Stage a payload for the egress process.
    StageExternalPayload,
    /// Open the outbound product socket.
    OpenOutboundSocket,
    /// Submit claim identifiers and provenance to the core.
    CreateClaim,
    /// Assemble an export.
    AssembleExport,
    /// Read raw key material.
    ReadKeyMaterial,
    /// Install a policy snapshot.
    InstallPolicySnapshot,
    /// Evaluate a permission request into a grant.
    MintCapability,
    /// Consume a capability at the runtime boundary.
    ConsumeCapability,
    /// Publish a proposal into the review queue.
    PublishProposal,
}

impl PrivilegedAction {
    /// Exhaustive order. The corpus test asserts one count per entry.
    pub const ALL: [Self; 15] = [
        Self::CaptureDevice,
        Self::WriteStagedArtifact,
        Self::ReadArtifactRange,
        Self::WriteSearchIndex,
        Self::AnalyzeRepository,
        Self::BorrowConnectorCredential,
        Self::StageExternalPayload,
        Self::OpenOutboundSocket,
        Self::CreateClaim,
        Self::AssembleExport,
        Self::ReadKeyMaterial,
        Self::InstallPolicySnapshot,
        Self::MintCapability,
        Self::ConsumeCapability,
        Self::PublishProposal,
    ];

    /// Stable spelling. The eleven capability variants use `P2-G7`'s own
    /// spelling so the two enumerations can be compared as text as well as
    /// through the witness `match`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CaptureDevice => "CAPTURE_DEVICE",
            Self::WriteStagedArtifact => "WRITE_STAGED_ARTIFACT",
            Self::ReadArtifactRange => "READ_ARTIFACT_RANGE",
            Self::WriteSearchIndex => "WRITE_SEARCH_INDEX",
            Self::AnalyzeRepository => "ANALYZE_REPOSITORY",
            Self::BorrowConnectorCredential => "BORROW_CONNECTOR_CREDENTIAL",
            Self::StageExternalPayload => "STAGE_EXTERNAL_PAYLOAD",
            Self::OpenOutboundSocket => "OPEN_OUTBOUND_SOCKET",
            Self::CreateClaim => "CREATE_CLAIM",
            Self::AssembleExport => "ASSEMBLE_EXPORT",
            Self::ReadKeyMaterial => "READ_KEY_MATERIAL",
            Self::InstallPolicySnapshot => "INSTALL_POLICY_SNAPSHOT",
            Self::MintCapability => "MINT_CAPABILITY",
            Self::ConsumeCapability => "CONSUME_CAPABILITY",
            Self::PublishProposal => "PUBLISH_PROPOSAL",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
    }
}

/// A count per [`PrivilegedAction`].
///
/// The ledger observes; it authorizes nothing. A caller that holds one still
/// needs the real broker, worker, or queue to perform the action it records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionLedger {
    counts: Vec<(PrivilegedAction, usize)>,
}

impl ActionLedger {
    /// A ledger with every count at zero.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counts: PrivilegedAction::ALL
                .into_iter()
                .map(|action| (action, 0))
                .collect(),
        }
    }

    /// Records one occurrence.
    pub fn record(&mut self, action: PrivilegedAction) {
        if let Some(entry) = self
            .counts
            .iter_mut()
            .find(|(recorded, _)| *recorded == action)
        {
            entry.1 = entry.1.saturating_add(1);
        } else {
            self.counts.push((action, 1));
        }
    }

    /// How many times `action` was recorded.
    #[must_use]
    pub fn count(&self, action: PrivilegedAction) -> usize {
        self.counts
            .iter()
            .find(|(recorded, _)| *recorded == action)
            .map_or(0, |(_, count)| *count)
    }

    /// Every recorded action with a non-zero count, in enumeration order.
    #[must_use]
    pub fn recorded(&self) -> Vec<(PrivilegedAction, usize)> {
        self.counts
            .iter()
            .filter(|(_, count)| *count > 0)
            .copied()
            .collect()
    }
}
