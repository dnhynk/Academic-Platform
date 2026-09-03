//! Section 18.4's `ProjectConceptRequirement`, and the lifecycle it exists for.
//!
//! Section 18.4's last paragraph:
//!
//! > 각 `REQUIRED` finding은 단순 edge 외에 `ProjectConceptRequirement` entity로도
//! > materialize한다. 이 entity가 project goal, snapshot, concrete
//! > responsibility/failure scenario, concept, 현재 사용자 state와 resolution
//! > status를 묶으므로, code가 바뀐 뒤 requirement가 충족·소멸·대체된 이력을
//! > 추적할 수 있다.
//!
//! Six things bound together, and the reason for binding them is the seventh:
//! the history. So this type carries all six as fields with no `Option` among
//! them — a requirement missing its goal, its snapshot, its need, its concept,
//! the user's state or its status has no representation — and it carries the
//! history as a list that only grows.
//!
//! ## The history only grows
//!
//! `CONTRIBUTING.md` rule 2: *canonical events, claims, evidence links, and
//! decisions are append-only. A correction is a new event plus an explicit
//! relation or decision.* So no transition takes `&mut self`. Each one
//! **consumes** the requirement and returns a new one whose history is the old
//! one plus a row, which is why `REQ-18-018`'s *without deleting A* is
//! structural: the value that recorded the earlier status is still the value
//! the caller had, and the new one carries its own record of every status it
//! has been through.
//!
//! ## A terminal status is terminal
//!
//! [`ResolutionStatus`] has one open value and three terminal ones, and the
//! three carry different payloads because they are established by different
//! evidence:
//!
//! * `SATISFIED` carries the snapshot the fix appeared in **and the locators
//!   that show it**, because section 18.4's reason for the entity is `code가
//!   바뀐 뒤` — a satisfaction with no site in the new snapshot is a claim
//!   about code nobody looked at;
//! * `RETIRED` carries the snapshot and a reason, because a requirement that
//!   disappeared because its code was deleted and one that disappeared because
//!   the goal changed are different histories;
//! * `REPLACED` carries the snapshot **and the successor's identity**, so a
//!   replacement with nothing to replace it has no representation. Section
//!   36.6's `Path A`/`Path B` is exactly this: one mechanism gives way to
//!   another and the reader has to be able to follow the arrow.
//!
//! A second transition out of a terminal status is refused, and the refusal
//! names which status the requirement is already in.

use academic_repository_analysis::Locator;

use crate::{
    ClassificationError,
    chain::{ConcreteNeed, ProofChain, UserEvidenceGap},
    scope::{ClassificationKey, GoalScope, validated},
};

/// Names one materialized requirement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequirementId {
    identifier: String,
}

impl RequirementId {
    /// Validates and takes a requirement identifier.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::InvalidIdentifier`] when it is empty, over 64
    /// bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, ClassificationError> {
        Ok(Self {
            identifier: validated(value.into(), "requirement")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// Why a requirement stopped applying without being met.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RetirementReason {
    /// The code the requirement was raised over is no longer in the snapshot.
    BasisRemoved,
    /// The goal version the requirement was raised under is no longer in force.
    GoalWithdrawn,
    /// The user showed the evidence the fifth step said they lacked.
    UserEvidenceSupplied,
}

impl RetirementReason {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [
        Self::BasisRemoved,
        Self::GoalWithdrawn,
        Self::UserEvidenceSupplied,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BasisRemoved => "BASIS_REMOVED",
            Self::GoalWithdrawn => "GOAL_WITHDRAWN",
            Self::UserEvidenceSupplied => "USER_EVIDENCE_SUPPLIED",
        }
    }
}

/// Section 18.4's `resolution status`: one open value and three terminal ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionStatus {
    /// The requirement stands.
    Open,
    /// `충족`: the code that raised it now controls the need.
    Satisfied {
        /// The snapshot the fix appeared in.
        snapshot_id: String,
        /// Where the fix is, in that snapshot.
        evidence: Vec<Locator>,
    },
    /// `소멸`: it stopped applying without being met.
    Retired {
        /// The snapshot it stopped applying in.
        snapshot_id: String,
        /// Why.
        reason: RetirementReason,
    },
    /// `대체`: another requirement took its place.
    Replaced {
        /// The snapshot the replacement appeared in.
        snapshot_id: String,
        /// The successor. A replacement without one has no representation.
        by: RequirementId,
    },
}

impl ResolutionStatus {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Satisfied { .. } => "SATISFIED",
            Self::Retired { .. } => "RETIRED",
            Self::Replaced { .. } => "REPLACED",
        }
    }

    /// Whether a further transition is possible.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }
}

/// One row of the history: the status this requirement moved into, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleRow {
    status: ResolutionStatus,
    at: u64,
}

impl LifecycleRow {
    /// The status entered.
    #[must_use]
    pub const fn status(&self) -> &ResolutionStatus {
        &self.status
    }

    /// When it was entered, in milliseconds.
    #[must_use]
    pub const fn at(&self) -> u64 {
        self.at
    }
}

/// Section 18.4's entity: six bound facts and the history over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConceptRequirement {
    id: RequirementId,
    key: ClassificationKey,
    need: ConcreteNeed,
    user_state: UserEvidenceGap,
    status: ResolutionStatus,
    history: Vec<LifecycleRow>,
}

impl ProjectConceptRequirement {
    /// Materializes one `REQUIRED` finding as an entity.
    ///
    /// Every fact but the identity and the time comes out of the chain, so the
    /// entity cannot disagree with the finding it materializes: the goal and
    /// the snapshot are the key's, the concrete responsibility or failure
    /// scenario is step two, the concept is step four, and the current user
    /// state is step five.
    #[must_use]
    pub fn materialize(
        id: RequirementId,
        key: ClassificationKey,
        chain: &ProofChain,
        at: u64,
    ) -> Self {
        Self {
            id,
            key,
            need: chain.need().clone(),
            user_state: chain.gap(),
            status: ResolutionStatus::Open,
            history: vec![LifecycleRow {
                status: ResolutionStatus::Open,
                at,
            }],
        }
    }

    /// Which requirement this is.
    #[must_use]
    pub const fn id(&self) -> &RequirementId {
        &self.id
    }

    /// Snapshot, goal version and concept.
    #[must_use]
    pub const fn key(&self) -> &ClassificationKey {
        &self.key
    }

    /// The goal version the requirement was raised under.
    #[must_use]
    pub const fn goal(&self) -> &GoalScope {
        self.key.goal()
    }

    /// The concrete responsibility or failure scenario, as step two recorded it.
    #[must_use]
    pub const fn need(&self) -> &ConcreteNeed {
        &self.need
    }

    /// The user's state at the time the requirement was raised.
    #[must_use]
    pub const fn user_state(&self) -> UserEvidenceGap {
        self.user_state
    }

    /// Where it stands now.
    #[must_use]
    pub const fn status(&self) -> &ResolutionStatus {
        &self.status
    }

    /// Every status it has been in, oldest first.
    #[must_use]
    pub fn history(&self) -> &[LifecycleRow] {
        &self.history
    }

    /// Moves into a terminal status, consuming the value and appending a row.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::RequirementAlreadySettled`] when the requirement
    /// is not [`ResolutionStatus::Open`], naming the status it is already in.
    fn settle(self, status: ResolutionStatus, at: u64) -> Result<Self, ClassificationError> {
        if !self.status.is_open() {
            return Err(ClassificationError::RequirementAlreadySettled(
                self.id.as_str().to_owned(),
                self.status.as_str(),
            ));
        }
        let mut history = self.history;
        history.push(LifecycleRow {
            status: status.clone(),
            at,
        });
        Ok(Self {
            id: self.id,
            key: self.key,
            need: self.need,
            user_state: self.user_state,
            status,
            history,
        })
    }

    /// `충족`: the need is now controlled, and here is where.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::SatisfactionHasNoEvidence`] when `evidence` is
    /// empty, and [`ClassificationError::RequirementAlreadySettled`] when this
    /// requirement has already been settled.
    pub fn satisfied(
        self,
        snapshot_id: impl Into<String>,
        evidence: Vec<Locator>,
        at: u64,
    ) -> Result<Self, ClassificationError> {
        if evidence.is_empty() {
            return Err(ClassificationError::SatisfactionHasNoEvidence(
                self.id.as_str().to_owned(),
            ));
        }
        self.settle(
            ResolutionStatus::Satisfied {
                snapshot_id: snapshot_id.into(),
                evidence,
            },
            at,
        )
    }

    /// `소멸`: it stopped applying, for one of three reasons.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::RequirementAlreadySettled`].
    pub fn retired(
        self,
        snapshot_id: impl Into<String>,
        reason: RetirementReason,
        at: u64,
    ) -> Result<Self, ClassificationError> {
        self.settle(
            ResolutionStatus::Retired {
                snapshot_id: snapshot_id.into(),
                reason,
            },
            at,
        )
    }

    /// `대체`: another requirement took its place.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::RequirementReplacesItself`] when `by` is this
    /// requirement's own identity, and
    /// [`ClassificationError::RequirementAlreadySettled`].
    pub fn replaced(
        self,
        snapshot_id: impl Into<String>,
        by: RequirementId,
        at: u64,
    ) -> Result<Self, ClassificationError> {
        if by == self.id {
            return Err(ClassificationError::RequirementReplacesItself(
                self.id.as_str().to_owned(),
            ));
        }
        self.settle(
            ResolutionStatus::Replaced {
                snapshot_id: snapshot_id.into(),
                by,
            },
            at,
        )
    }
}
