//! The deletion dependency plan and the four-word result vocabulary.
//!
//! # The plan enumerates classes, not findings
//!
//! [`DERIVATIVE_CLASSES`] is closed and every plan carries one node per class,
//! always, in registry order. A class with nothing to delete is a node saying
//! so with a reason; a class the resolver could not answer for is a node saying
//! *that*, and it makes the action `REPAIR_REQUIRED` rather than dropping out
//! of the report. There is no path that produces a plan with a missing class,
//! which is what `deletion_plan_enumerates_every_derivative_class` checks.
//!
//! # "Mostly deleted" is not a result
//!
//! [`RetentionOutcome::Complete`] is reachable only when nothing is left, and
//! [`RetentionOutcome::Partial`] carries an [`UnresolvedSet`], whose
//! constructor refuses an empty list. So a `PARTIAL` result always names the
//! exact locators that are still there, and a `COMPLETE` result cannot be
//! returned while anything remains.
//!
//! # `GATE-38-026`
//!
//! Deleting non-instructor voices from an **original** is a mechanism here and
//! a policy nowhere. [`OriginalVoiceAuthority`] has no `Default`, no constant,
//! and no constructor that invents one: a caller states who authorized the
//! removal and what evidence backs it, or it cannot build the subject at all.
//! Nothing in this crate decides whether such a removal should happen.

use serde::{Deserialize, Serialize};

/// What `GATE-38-026` leaves open, stated where the mechanism lives.
pub const GATE_38_026_STATEMENT: &str = "whether non-instructor voices may be removed from an original recording, \
     and under whose authority, is an open user decision (GATE-38-026); this \
     build implements the mechanism and selects no policy";

/// One class of thing a deletion has to reach.
///
/// The list is t068 section 5's, in its order, and it is closed: a new class is
/// a contract change, not a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum DerivativeClass {
    /// Lecture transcripts derived from the subject.
    Transcript,
    /// Vector embeddings computed over the subject or its transcript.
    Embedding,
    /// Canonical graph claims whose evidence is the subject.
    GraphClaim,
    /// Generated documents that quote or summarise the subject.
    Document,
    /// Local caches holding derived bytes.
    Cache,
    /// Replicas of the subject held elsewhere on this device.
    Replica,
    /// Backups whose expiry the deletion has to reach.
    BackupExpiry,
}

/// Every derivative class, in the order a plan reports them.
pub const DERIVATIVE_CLASSES: &[DerivativeClass] = &[
    DerivativeClass::Transcript,
    DerivativeClass::Embedding,
    DerivativeClass::GraphClaim,
    DerivativeClass::Document,
    DerivativeClass::Cache,
    DerivativeClass::Replica,
    DerivativeClass::BackupExpiry,
];

impl DerivativeClass {
    /// Returns the stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transcript => "TRANSCRIPT",
            Self::Embedding => "EMBEDDING",
            Self::GraphClaim => "GRAPH_CLAIM",
            Self::Document => "DOCUMENT",
            Self::Cache => "CACHE",
            Self::Replica => "REPLICA",
            Self::BackupExpiry => "BACKUP_EXPIRY",
        }
    }

    /// Returns how this class is deleted.
    #[must_use]
    pub const fn action_kind(self) -> ActionKind {
        match self {
            // A derivative that is itself a sealed object is deleted the same
            // way its subject is: by destroying its key slot.
            Self::Transcript | Self::Embedding | Self::GraphClaim | Self::Document => {
                ActionKind::CryptoShred
            }
            // A cache or replica is disposable by construction and is removed.
            Self::Cache | Self::Replica => ActionKind::Purge,
            // A backup is not reachable for editing; the deletion follows it
            // with a tombstone that re-deletes on restore.
            Self::BackupExpiry => ActionKind::BackupTombstone,
        }
    }
}

/// How one planned action deletes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ActionKind {
    /// Destroy the object's key slot.
    CryptoShred,
    /// Remove the file.
    Purge,
    /// Write a tombstone that re-deletes when the backup is restored.
    BackupTombstone,
}

/// Who authorized removing non-instructor voices from an original.
///
/// There is deliberately no `Default` and no named constant: `GATE-38-026` is
/// an open user decision and this build must not answer it. The fields are
/// what a caller has to state; the crate never fills one in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginalVoiceAuthority {
    authority_id: String,
    evidence_digest: String,
}

impl OriginalVoiceAuthority {
    /// Records an authority the caller obtained.
    #[must_use]
    pub fn new(authority_id: String, evidence_digest: String) -> Self {
        Self {
            authority_id,
            evidence_digest,
        }
    }

    /// Returns the authority identity.
    #[must_use]
    pub fn authority_id(&self) -> &str {
        &self.authority_id
    }

    /// Returns the digest of the evidence backing it.
    #[must_use]
    pub fn evidence_digest(&self) -> &str {
        &self.evidence_digest
    }
}

/// A half-open span of an original recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceSpan {
    /// Inclusive start, in milliseconds.
    pub start_ms: u64,
    /// Exclusive end, in milliseconds.
    pub end_ms: u64,
}

/// What a retention action is about.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SubjectScope {
    /// The whole object.
    WholeObject,
    /// Named spans inside an original, under a stated authority.
    ///
    /// The authority is not optional. `GATE-38-026` stays open because this
    /// crate makes the caller supply one rather than assuming one.
    VoiceSpansInOriginal {
        /// Who authorized it.
        authority: OriginalVoiceAuthority,
        /// The spans to remove.
        spans: Vec<VoiceSpan>,
    },
}

/// The object a retention action is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionSubject {
    locator: [u8; 32],
    scope: SubjectScope,
}

impl RetentionSubject {
    /// Names a whole object.
    #[must_use]
    pub const fn whole_object(locator: [u8; 32]) -> Self {
        Self {
            locator,
            scope: SubjectScope::WholeObject,
        }
    }

    /// Names spans inside an original under a stated authority.
    #[must_use]
    pub const fn voice_spans_in_original(
        locator: [u8; 32],
        authority: OriginalVoiceAuthority,
        spans: Vec<VoiceSpan>,
    ) -> Self {
        Self {
            locator,
            scope: SubjectScope::VoiceSpansInOriginal { authority, spans },
        }
    }

    /// Returns the subject locator.
    #[must_use]
    pub const fn locator(&self) -> &[u8; 32] {
        &self.locator
    }

    /// Returns the subject locator's hex spelling.
    #[must_use]
    pub fn locator_hex(&self) -> String {
        hex::encode(self.locator)
    }

    /// Returns the scope.
    #[must_use]
    pub const fn scope(&self) -> &SubjectScope {
        &self.scope
    }

    /// Returns what stays open when the subject is an original.
    ///
    /// A caller that renders a plan renders this beside it, so no operator can
    /// read a voice-scoped deletion as settled policy.
    #[must_use]
    pub const fn open_gate_statement(&self) -> Option<&'static str> {
        match self.scope {
            SubjectScope::WholeObject => None,
            SubjectScope::VoiceSpansInOriginal { .. } => Some(GATE_38_026_STATEMENT),
        }
    }
}

/// What a resolver could say about one class.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClassResolution {
    /// These exact locators belong to the class.
    Locators(Vec<[u8; 32]>),
    /// The class holds nothing for this subject, for a stated reason.
    NothingToDelete {
        /// Why the class is empty here.
        reason: String,
    },
    /// The class could not be answered for.
    ///
    /// This is `RB03`. It does not become an empty class: the deletion refuses
    /// to complete and names the node.
    Unresolved {
        /// Why the class could not be answered for.
        reason: String,
    },
}

/// Answers what belongs to each derivative class for one subject.
///
/// `P2-P2` supplies the real implementation once the transcript, embedding,
/// claim, document, cache, and replica subsystems exist. `P2-K5` fixes the
/// enumeration contract and the vocabulary its answers are reported in.
pub trait DerivativeResolver {
    /// Resolves one class for one subject.
    fn resolve(&self, class: DerivativeClass, subject: &RetentionSubject) -> ClassResolution;
}

/// One class's node in a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanNode {
    /// Which class this node is for.
    pub class: DerivativeClass,
    /// What the resolver said.
    pub resolution: ClassResolution,
}

impl PlanNode {
    /// Returns the actions this node contributes.
    #[must_use]
    pub fn actions(&self) -> Vec<PlannedAction> {
        match &self.resolution {
            ClassResolution::Locators(locators) => locators
                .iter()
                .map(|locator| PlannedAction {
                    class: self.class,
                    kind: self.class.action_kind(),
                    locator: *locator,
                })
                .collect(),
            ClassResolution::NothingToDelete { .. } | ClassResolution::Unresolved { .. } => {
                Vec::new()
            }
        }
    }
}

/// One thing the executor is asked to delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedAction {
    /// The class this action belongs to.
    pub class: DerivativeClass,
    /// How it deletes.
    pub kind: ActionKind,
    /// The locator it deletes.
    pub locator: [u8; 32],
}

impl PlannedAction {
    /// Returns the locator's hex spelling.
    #[must_use]
    pub fn locator_hex(&self) -> String {
        hex::encode(self.locator)
    }
}

/// Why a locator is still there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum UnresolvedReason {
    /// The planner could not resolve the class at all (`RB03`).
    NotResolved,
    /// The executor refused or failed (`RB04`).
    PurgeFailed,
    /// A backup tombstone could not be written (`RB02`).
    TombstoneWriteFailed,
    /// The key slot could not be destroyed.
    ShredFailed,
}

/// One locator a deletion did not reach, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedLocator {
    /// Which class it belongs to.
    pub class: DerivativeClass,
    /// The locator, or the resolver's node name when there is no locator yet.
    pub locator: String,
    /// Why it is still there.
    pub reason: UnresolvedReason,
    /// The executor's or resolver's own words.
    pub detail: String,
}

impl UnresolvedLocator {
    /// Renders the row a report shows.
    #[must_use]
    pub fn to_row(&self) -> String {
        format!(
            "{}/{}: {} ({})",
            self.class.as_str(),
            self.locator,
            self.detail,
            match self.reason {
                UnresolvedReason::NotResolved => "NOT_RESOLVED",
                UnresolvedReason::PurgeFailed => "PURGE_FAILED",
                UnresolvedReason::TombstoneWriteFailed => "TOMBSTONE_WRITE_FAILED",
                UnresolvedReason::ShredFailed => "SHRED_FAILED",
            }
        )
    }
}

/// A non-empty list of unreached locators.
///
/// The field is private and the constructor refuses an empty list, so no
/// `PARTIAL` or `REPAIR_REQUIRED` result can exist without naming what is left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedSet(Vec<UnresolvedLocator>);

impl UnresolvedSet {
    /// Builds a set, refusing an empty one.
    #[must_use]
    pub fn new(locators: Vec<UnresolvedLocator>) -> Option<Self> {
        (!locators.is_empty()).then_some(Self(locators))
    }

    /// Returns the exact locators, in plan order.
    #[must_use]
    pub fn locators(&self) -> &[UnresolvedLocator] {
        &self.0
    }

    /// Returns the rendered rows, in plan order.
    #[must_use]
    pub fn rows(&self) -> Vec<String> {
        self.0.iter().map(UnresolvedLocator::to_row).collect()
    }
}

/// The four words a retention action can end in.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RetentionOutcome {
    /// A plan exists and nothing has been executed.
    Planned,
    /// Every planned action succeeded and nothing is left.
    Complete,
    /// Some actions succeeded; these exact locators are still there.
    Partial(UnresolvedSet),
    /// The action cannot be completed without an operator; these are why.
    RepairRequired(UnresolvedSet),
}

impl RetentionOutcome {
    /// Returns the stable external spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Planned => "PLANNED",
            Self::Complete => "COMPLETE",
            Self::Partial(_) => "PARTIAL",
            Self::RepairRequired(_) => "REPAIR_REQUIRED",
        }
    }

    /// Returns the exact unreached locators, empty only for `COMPLETE`.
    #[must_use]
    pub fn unresolved(&self) -> &[UnresolvedLocator] {
        match self {
            Self::Planned | Self::Complete => &[],
            Self::Partial(set) | Self::RepairRequired(set) => set.locators(),
        }
    }
}

/// The whole vocabulary, so a surface can prove it renders all of it.
pub const RETENTION_OUTCOMES: &[&str] = &["PLANNED", "COMPLETE", "PARTIAL", "REPAIR_REQUIRED"];

/// A deletion plan: one node per class, always.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionPlan {
    subject: RetentionSubject,
    nodes: Vec<PlanNode>,
}

impl DeletionPlan {
    /// Builds a plan by asking the resolver about every class in registry order.
    pub fn build<R: DerivativeResolver + ?Sized>(subject: RetentionSubject, resolver: &R) -> Self {
        let nodes = DERIVATIVE_CLASSES
            .iter()
            .map(|class| PlanNode {
                class: *class,
                resolution: resolver.resolve(*class, &subject),
            })
            .collect();
        Self { subject, nodes }
    }

    /// Returns the subject.
    #[must_use]
    pub const fn subject(&self) -> &RetentionSubject {
        &self.subject
    }

    /// Returns one node per class, in registry order.
    #[must_use]
    pub fn nodes(&self) -> &[PlanNode] {
        &self.nodes
    }

    /// Returns every class the plan enumerated, in registry order.
    #[must_use]
    pub fn enumerated_classes(&self) -> Vec<DerivativeClass> {
        self.nodes.iter().map(|node| node.class).collect()
    }

    /// Returns every action, in class order then resolver order.
    #[must_use]
    pub fn actions(&self) -> Vec<PlannedAction> {
        self.nodes.iter().flat_map(PlanNode::actions).collect()
    }

    /// Returns the nodes the resolver could not answer for (`RB03`).
    #[must_use]
    pub fn unresolved_nodes(&self) -> Vec<UnresolvedLocator> {
        self.nodes
            .iter()
            .filter_map(|node| match &node.resolution {
                ClassResolution::Unresolved { reason } => Some(UnresolvedLocator {
                    class: node.class,
                    locator: format!("<class {}>", node.class.as_str()),
                    reason: UnresolvedReason::NotResolved,
                    detail: reason.clone(),
                }),
                _ => None,
            })
            .collect()
    }
}
