//! Section 17.5's `ImplementationDrift`, and the four scopes that qualify one.
//!
//! ## Neither side is overwritten
//!
//! Section 17.5's sentence is exact: *둘은 같은 질문의 경쟁 답이 아니므로 한쪽으로
//! 덮지 않고 `ImplementationDrift`를 만든다*. Section 30.3 says the same thing
//! from each side — row four's conflict column is `spec은 intent lane에 보존`
//! and row five's is `code와 drift 생성`.
//!
//! So a drift is a record **beside** the edges rather than a replacement for
//! either, and it carries all three lanes' edges as they were. The intent view
//! of a subject still answers from the intent lane and the implementation view
//! still answers from the implementation lane;
//! `conflict_creates_drift_without_overwrite` observes both, in both
//! directions, against the same corpus with the conflicting side removed.
//!
//! ## Two kinds and no third
//!
//! Section 17.5 names `INTENDED_NOT_IMPLEMENTED` and
//! `IMPLEMENTED_NOT_DOCUMENTED` and nothing else, so [`DriftKind`] has two
//! variants. `ANALYSIS_CHANGED` is **not** a third: section 19 introduces it
//! for a difference between two runs, not for a disagreement inside one, and
//! [`crate::compare`] owns it.
//!
//! ## Four scopes, four different payloads
//!
//! Section 17.5's last sentence is `deprecated spec, feature flag, 미배포 code,
//! branch 차이도 scope로 구분한다`. They are four independent qualifiers and not
//! four values of one field:
//!
//! * they can hold at once — code behind a flag on an undeployed branch is all
//!   three — so one enumeration, which admits exactly one, would drop two of
//!   them;
//! * each is established by a different argument, and each carries a different
//!   payload, so no two can be built from the same evidence. A boolean per
//!   scope would carry none of it, and `deprecated_flagged_undeployed_branch_
//!   scopes_are_distinct` is written against exactly that collapse.

use crate::{
    artifact::{DeploymentTarget, DocumentId, FlagKey, FlagState},
    edge::RelationEdge,
};

/// Section 17.5's two drift results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DriftKind {
    /// A specification or architecture decision names the subject and no
    /// `PROJECT_CODE_USES` does.
    IntendedNotImplemented,
    /// A `PROJECT_CODE_USES` names the subject and no `PROJECT_DOC_EXPLAINS`
    /// does.
    ImplementedNotDocumented,
}

impl DriftKind {
    /// Exhaustive order, in section 17.5's diagram order.
    pub const ALL: [Self; 2] = [Self::IntendedNotImplemented, Self::ImplementedNotDocumented];

    /// The spelling section 17.5 uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IntendedNotImplemented => "INTENDED_NOT_IMPLEMENTED",
            Self::ImplementedNotDocumented => "IMPLEMENTED_NOT_DOCUMENTED",
        }
    }
}

/// Which of section 17.5's four scopes is present, for display and comparison.
///
/// A label only. The evidence for each lives on [`DriftScopes`], and the four
/// payloads there are different types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DriftScopeKind {
    /// `deprecated spec`.
    DeprecatedSpec,
    /// `feature flag`.
    FeatureFlag,
    /// `미배포 code`.
    UndeployedCode,
    /// `branch 차이`.
    BranchDifference,
}

impl DriftScopeKind {
    /// Exhaustive order, in section 17.5's own order.
    pub const ALL: [Self; 4] = [
        Self::DeprecatedSpec,
        Self::FeatureFlag,
        Self::UndeployedCode,
        Self::BranchDifference,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeprecatedSpec => "DEPRECATED_SPEC",
            Self::FeatureFlag => "FEATURE_FLAG",
            Self::UndeployedCode => "UNDEPLOYED_CODE",
            Self::BranchDifference => "BRANCH_DIFFERENCE",
        }
    }
}

/// The intent document that named the subject has been withdrawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecatedSpec {
    document: DocumentId,
    revision: u64,
}

impl DeprecatedSpec {
    pub(crate) const fn seal(document: DocumentId, revision: u64) -> Self {
        Self { document, revision }
    }

    /// Which document was withdrawn.
    #[must_use]
    pub const fn document(&self) -> &DocumentId {
        &self.document
    }

    /// Which revision of it.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

/// A feature flag gates the subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatingFlag {
    key: FlagKey,
    state: FlagState,
}

impl GatingFlag {
    pub(crate) const fn seal(key: FlagKey, state: FlagState) -> Self {
        Self { key, state }
    }

    /// Which flag.
    #[must_use]
    pub const fn key(&self) -> &FlagKey {
        &self.key
    }

    /// On or off. Carried rather than folded away: a subject gated by a flag
    /// that is on is a different reading from one gated by a flag that is off,
    /// and the drift is scoped either way.
    #[must_use]
    pub const fn state(&self) -> FlagState {
        self.state
    }
}

/// The snapshot holding the code is not the snapshot a target runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndeployedCode {
    target: DeploymentTarget,
    deployed_snapshot: String,
}

impl UndeployedCode {
    pub(crate) const fn seal(target: DeploymentTarget, deployed_snapshot: String) -> Self {
        Self {
            target,
            deployed_snapshot,
        }
    }

    /// Which target.
    #[must_use]
    pub const fn target(&self) -> &DeploymentTarget {
        &self.target
    }

    /// Which snapshot that target is running instead.
    #[must_use]
    pub fn deployed_snapshot(&self) -> &str {
        &self.deployed_snapshot
    }
}

/// The intent was approved against a branch this snapshot is not on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchDifference {
    intent_branch: String,
    snapshot_branch: Option<String>,
}

impl BranchDifference {
    pub(crate) const fn seal(intent_branch: String, snapshot_branch: Option<String>) -> Self {
        Self {
            intent_branch,
            snapshot_branch,
        }
    }

    /// The branch the intent document names.
    #[must_use]
    pub fn intent_branch(&self) -> &str {
        &self.intent_branch
    }

    /// The branch the snapshot is on, when it is on one.
    #[must_use]
    pub fn snapshot_branch(&self) -> Option<&str> {
        self.snapshot_branch.as_deref()
    }
}

/// Section 17.5's four drift scopes, each independently present or absent.
///
/// Four `Option`s of four different types rather than one enumeration or four
/// booleans. See the module documentation for why.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DriftScopes {
    deprecated_spec: Option<DeprecatedSpec>,
    feature_flag: Option<GatingFlag>,
    undeployed_code: Option<UndeployedCode>,
    branch_difference: Option<BranchDifference>,
}

impl DriftScopes {
    pub(crate) const fn seal(
        deprecated_spec: Option<DeprecatedSpec>,
        feature_flag: Option<GatingFlag>,
        undeployed_code: Option<UndeployedCode>,
        branch_difference: Option<BranchDifference>,
    ) -> Self {
        Self {
            deprecated_spec,
            feature_flag,
            undeployed_code,
            branch_difference,
        }
    }

    /// The withdrawn intent document, when there is one.
    #[must_use]
    pub const fn deprecated_spec(&self) -> Option<&DeprecatedSpec> {
        self.deprecated_spec.as_ref()
    }

    /// The gating flag, when there is one.
    #[must_use]
    pub const fn feature_flag(&self) -> Option<&GatingFlag> {
        self.feature_flag.as_ref()
    }

    /// The target running another snapshot, when there is one.
    #[must_use]
    pub const fn undeployed_code(&self) -> Option<&UndeployedCode> {
        self.undeployed_code.as_ref()
    }

    /// The branch the intent names and this snapshot is not on.
    #[must_use]
    pub const fn branch_difference(&self) -> Option<&BranchDifference> {
        self.branch_difference.as_ref()
    }

    /// Which scopes are present, as labels.
    ///
    /// Total over [`DriftScopeKind::ALL`] by construction: a fifth scope has to
    /// be added to that array and to the match below, and the match has no
    /// default arm.
    #[must_use]
    pub fn present(&self) -> Vec<DriftScopeKind> {
        DriftScopeKind::ALL
            .into_iter()
            .filter(|kind| match kind {
                DriftScopeKind::DeprecatedSpec => self.deprecated_spec.is_some(),
                DriftScopeKind::FeatureFlag => self.feature_flag.is_some(),
                DriftScopeKind::UndeployedCode => self.undeployed_code.is_some(),
                DriftScopeKind::BranchDifference => self.branch_difference.is_some(),
            })
            .collect()
    }
}

/// Section 17.5's `ImplementationDrift`.
///
/// It carries every edge the subject has, in the lane each belongs to, and it
/// replaces none of them. See the module documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementationDrift {
    kind: DriftKind,
    subject: String,
    snapshot_id: String,
    intent_side: Vec<RelationEdge>,
    implementation_side: Vec<RelationEdge>,
    description_side: Vec<RelationEdge>,
    scopes: DriftScopes,
}

impl ImplementationDrift {
    /// The one constructor, crate-private and called only from
    /// [`crate::correlate`].
    pub(crate) const fn seal(
        kind: DriftKind,
        subject: String,
        snapshot_id: String,
        intent_side: Vec<RelationEdge>,
        implementation_side: Vec<RelationEdge>,
        description_side: Vec<RelationEdge>,
        scopes: DriftScopes,
    ) -> Self {
        Self {
            kind,
            subject,
            snapshot_id,
            intent_side,
            implementation_side,
            description_side,
            scopes,
        }
    }

    /// Which of section 17.5's two results this is.
    #[must_use]
    pub const fn kind(&self) -> DriftKind {
        self.kind
    }

    /// What the drift is about.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Which snapshot it is about.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// The intent lane's edges, unchanged.
    #[must_use]
    pub fn intent_side(&self) -> &[RelationEdge] {
        &self.intent_side
    }

    /// The implementation lane's edges, unchanged.
    #[must_use]
    pub fn implementation_side(&self) -> &[RelationEdge] {
        &self.implementation_side
    }

    /// The description lane's edges, unchanged.
    #[must_use]
    pub fn description_side(&self) -> &[RelationEdge] {
        &self.description_side
    }

    /// Section 17.5's four scopes.
    #[must_use]
    pub const fn scopes(&self) -> &DriftScopes {
        &self.scopes
    }
}
