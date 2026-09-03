//! The artifacts correlation reads beside code, and the identifiers that name
//! them.
//!
//! Section 17.3's fourth stage is `spec ↔ ADR ↔ code ↔ config ↔ test ↔
//! runtime/incident`. `P2-R2` produced the code, config and test half as
//! [`academic_repository_analysis::Finding`]s. The other half — a
//! specification, an architecture decision, a document describing current
//! behaviour, an incident record — is **an argument**, for the reason
//! `P2-R2`'s runtime trace is one: this crate opens nothing, and a document is
//! prose that `P2-R2`'s coverage matrix already reports `NotApplicable` for
//! every index kind.
//!
//! What a caller hands over is therefore never text. A document names the
//! subjects it is about as [`SubjectId`]s — values this system chose — and a
//! locator into the snapshot's own manifest. Matching is *untrusted text
//! selects from a trusted set* and the trusted half is what survives, which is
//! the same rule `P2-R2`'s `Subject` needles follow one step down.

use academic_repository_analysis::SubjectId;

use crate::CorrelationError;

/// Whether `value` is an identifier this system may hold and hand back.
///
/// `[A-Za-z0-9._-]` within 64 bytes: the same shape `SubjectId` accepts, so a
/// document identifier and a subject identifier cannot differ in what they
/// admit.
fn validated(value: String, field: &'static str) -> Result<String, CorrelationError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(value)
    } else {
        Err(CorrelationError::InvalidIdentifier(field, value))
    }
}

/// Names one specification, architecture decision or behaviour document.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId {
    identifier: String,
}

impl DocumentId {
    /// Validates and takes a document identifier.
    ///
    /// # Errors
    ///
    /// [`CorrelationError::InvalidIdentifier`] when it is empty, over 64 bytes,
    /// or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, CorrelationError> {
        Ok(Self {
            identifier: validated(value.into(), "document")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// Names one incident record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncidentId {
    identifier: String,
}

impl IncidentId {
    /// Validates and takes an incident identifier.
    ///
    /// # Errors
    ///
    /// [`CorrelationError::InvalidIdentifier`] on the same three conditions
    /// [`DocumentId::new`] refuses.
    pub fn new(value: impl Into<String>) -> Result<Self, CorrelationError> {
        Ok(Self {
            identifier: validated(value.into(), "incident")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// Names one feature flag.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlagKey {
    identifier: String,
}

impl FlagKey {
    /// Validates and takes a flag key.
    ///
    /// # Errors
    ///
    /// [`CorrelationError::InvalidIdentifier`] on the same three conditions
    /// [`DocumentId::new`] refuses.
    pub fn new(value: impl Into<String>) -> Result<Self, CorrelationError> {
        Ok(Self {
            identifier: validated(value.into(), "flag")?,
        })
    }

    /// The key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// Names one deployment target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeploymentTarget {
    identifier: String,
}

impl DeploymentTarget {
    /// Validates and takes a deployment target name.
    ///
    /// # Errors
    ///
    /// [`CorrelationError::InvalidIdentifier`] on the same three conditions
    /// [`DocumentId::new`] refuses.
    pub fn new(value: impl Into<String>) -> Result<Self, CorrelationError> {
        Ok(Self {
            identifier: validated(value.into(), "deployment")?,
        })
    }

    /// The target name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// Which of section 30.3 row five's two authorities a document is.
///
/// Row five reads `승인된 최신 spec/ADR`. The two are named together there and
/// rank the same, so this distinguishes them for display and for section 17.5's
/// two different relations rather than for precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntentDocumentKind {
    /// A product or technical specification.
    Specification,
    /// An architecture decision record.
    ArchitectureDecision,
}

impl IntentDocumentKind {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Specification, Self::ArchitectureDecision];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Specification => "SPECIFICATION",
            Self::ArchitectureDecision => "ARCHITECTURE_DECISION",
        }
    }

    /// The section 17.5 relation a document of this kind produces.
    #[must_use]
    pub const fn relation(self) -> crate::EvidenceRelation {
        match self {
            Self::Specification => crate::EvidenceRelation::SpecMentions,
            Self::ArchitectureDecision => crate::EvidenceRelation::ArchitectureRequires,
        }
    }
}

/// Section 30.3 row five's `승인된` and section 17.5's `유효한`.
///
/// Row five's authority is not *a* spec but an **approved, latest** one. A
/// draft has not been approved and a deprecated one is no longer the latest
/// word, so neither is that row's authority; [`crate::authority`] is where that
/// becomes an authority class, and `DEPRECATED_SPEC` is one of section 17.5's
/// four drift scopes rather than a reason to drop the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ApprovalStatus {
    /// Written and not approved.
    Draft,
    /// Approved and current.
    Approved,
    /// Approved once and withdrawn.
    Deprecated,
}

impl ApprovalStatus {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::Draft, Self::Approved, Self::Deprecated];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Approved => "APPROVED",
            Self::Deprecated => "DEPRECATED",
        }
    }
}

/// A specification or architecture decision, as an argument.
///
/// The `path` is a row of the snapshot's own manifest, checked at
/// [`crate::correlate`] the way `P2-R2` checks an analyzed unit's path: a
/// document that is not in the snapshot is not evidence about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentDocument {
    id: DocumentId,
    kind: IntentDocumentKind,
    status: ApprovalStatus,
    revision: u64,
    branch: Option<String>,
    path: String,
    mentions: Vec<SubjectId>,
}

impl IntentDocument {
    /// Records one intent document and the subjects it names.
    ///
    /// `branch` is the branch the document was approved against. `None` means
    /// the document names none, which is not a branch difference — an absent
    /// statement is not a contrary one.
    #[must_use]
    pub fn new(
        id: DocumentId,
        kind: IntentDocumentKind,
        status: ApprovalStatus,
        revision: u64,
        branch: Option<String>,
        path: impl Into<String>,
        mentions: Vec<SubjectId>,
    ) -> Self {
        Self {
            id,
            kind,
            status,
            revision,
            branch,
            path: path.into(),
            mentions,
        }
    }

    /// Which document it is.
    #[must_use]
    pub const fn id(&self) -> &DocumentId {
        &self.id
    }

    /// Specification or architecture decision.
    #[must_use]
    pub const fn kind(&self) -> IntentDocumentKind {
        self.kind
    }

    /// Draft, approved or deprecated.
    #[must_use]
    pub const fn status(&self) -> ApprovalStatus {
        self.status
    }

    /// Section 30.3 row five's `최신`: the highest revision wins.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// The branch this intent was approved against, when it names one.
    #[must_use]
    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    /// Where the document sits in the snapshot's manifest.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The subjects it names.
    #[must_use]
    pub fn mentions(&self) -> &[SubjectId] {
        &self.mentions
    }
}

/// Section 17.5's `문서가 현재 동작을 설명`, as an argument.
///
/// Deliberately not an [`IntentDocument`] with a fourth status. A document that
/// describes what the system does is not a weaker approval; it is an answer to
/// a different question, and its **absence** beside a `PROJECT_CODE_USES` is
/// what section 17.5's second diagram calls `IMPLEMENTED_NOT_DOCUMENTED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorDocument {
    id: DocumentId,
    path: String,
    explains: Vec<SubjectId>,
}

impl BehaviorDocument {
    /// Records one behaviour document and the subjects it explains.
    #[must_use]
    pub fn new(id: DocumentId, path: impl Into<String>, explains: Vec<SubjectId>) -> Self {
        Self {
            id,
            path: path.into(),
            explains,
        }
    }

    /// Which document it is.
    #[must_use]
    pub const fn id(&self) -> &DocumentId {
        &self.id
    }

    /// Where the document sits in the snapshot's manifest.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The subjects it explains.
    #[must_use]
    pub fn explains(&self) -> &[SubjectId] {
        &self.explains
    }
}

/// Section 17.5's `incident가 failure mode를 드러냄`, as an argument.
///
/// Bound to a snapshot for the reason `P2-R2`'s `RuntimeTrace` is: an incident
/// against another snapshot is not evidence about this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidentRecord {
    id: IncidentId,
    snapshot_id: String,
    occurred_at: u64,
    exposed: Vec<SubjectId>,
}

impl IncidentRecord {
    /// Records one incident and the subjects whose failure mode it exposed.
    #[must_use]
    pub fn new(
        id: IncidentId,
        snapshot_id: impl Into<String>,
        occurred_at: u64,
        exposed: Vec<SubjectId>,
    ) -> Self {
        Self {
            id,
            snapshot_id: snapshot_id.into(),
            occurred_at,
            exposed,
        }
    }

    /// Which incident it is.
    #[must_use]
    pub const fn id(&self) -> &IncidentId {
        &self.id
    }

    /// Which snapshot it was recorded against.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// When it happened.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }

    /// The subjects it exposed.
    #[must_use]
    pub fn exposed(&self) -> &[SubjectId] {
        &self.exposed
    }
}

/// Whether a flag is on or off in the configuration this snapshot ships.
///
/// A state and not a boolean field on the drift: section 17.5 keeps `feature
/// flag` as its own scope, and a reader told a drift is flag-scoped needs the
/// flag and its state, not the fact that some flag was involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlagState {
    /// The gated code does not run.
    Off,
    /// The gated code runs.
    On,
}

impl FlagState {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Off, Self::On];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::On => "ON",
        }
    }
}

/// One feature flag and the subjects it gates, as an argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlagRecord {
    key: FlagKey,
    state: FlagState,
    gates: Vec<SubjectId>,
}

impl FeatureFlagRecord {
    /// Records one flag, its state, and what it gates.
    #[must_use]
    pub fn new(key: FlagKey, state: FlagState, gates: Vec<SubjectId>) -> Self {
        Self { key, state, gates }
    }

    /// Which flag it is.
    #[must_use]
    pub const fn key(&self) -> &FlagKey {
        &self.key
    }

    /// On or off.
    #[must_use]
    pub const fn state(&self) -> FlagState {
        self.state
    }

    /// The subjects it gates.
    #[must_use]
    pub fn gates(&self) -> &[SubjectId] {
        &self.gates
    }
}

/// Which snapshot one deployment target is running, as an argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentRecord {
    target: DeploymentTarget,
    deployed_snapshot: String,
}

impl DeploymentRecord {
    /// Records that one target runs one snapshot.
    #[must_use]
    pub fn new(target: DeploymentTarget, deployed_snapshot: impl Into<String>) -> Self {
        Self {
            target,
            deployed_snapshot: deployed_snapshot.into(),
        }
    }

    /// Which target it is.
    #[must_use]
    pub const fn target(&self) -> &DeploymentTarget {
        &self.target
    }

    /// Which snapshot that target is running.
    #[must_use]
    pub fn deployed_snapshot(&self) -> &str {
        &self.deployed_snapshot
    }
}
