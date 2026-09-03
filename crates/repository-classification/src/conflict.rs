//! Section 18.4's fifth bullet: a user override and new evidence open a
//! conflict rather than reclassifying anything.
//!
//! > 새 evidence가 사용자 override와 충돌하면 자동 재분류하지 않고
//! > `ClassificationConflict`를 연다.
//!
//! and section 36.5, from the user's side:
//!
//! > 사용자는 dependency 하나가 template 잔재라고 정정한다. 이 override는 다음
//! > 분석에서도 유지된다.
//!
//! ## Neither side is rewritten, and that is a shape
//!
//! `P2-R3` fixed this pattern for the two authority lanes: an
//! `ImplementationDrift` is *a record beside the edges rather than a
//! replacement for either*, and it carries both sides as they were. A
//! [`ClassificationConflict`] is the same record for a different pair — a
//! standing [`UserOverride`] and the [`Outlook`] a fresh analysis proposed —
//! and it is built by cloning both, never by editing either.
//!
//! Two things follow in the output of [`crate::classify`], and
//! `user_override_creates_conflict_not_reclassification` observes both:
//!
//! * the published stance keeps the **user's** answer, because
//!   `CONTRIBUTING.md` rule 2's *a correction is a new event* makes the
//!   override the later decision about the same subject; and
//! * the proposal is not discarded — it is the conflict's second side, so a
//!   reader can see what the analysis said and why it did not take effect.
//!
//! ## An override outlives the snapshot it was made about
//!
//! Section 36.5's `다음 분석에서도 유지된다` is why a [`UserOverride`] is keyed on
//! the goal and the concept and records the snapshot it was **made from**
//! rather than being keyed on it. A classification is snapshot-scoped
//! ([`crate::ClassificationKey`]); a user's decision is not, and keying the
//! decision on the snapshot would silently expire it at the next capture — the
//! exact failure `REQ-36-029` is written against.

use crate::{
    ClassificationError,
    scope::{ClassificationKey, GoalScope, validated},
    stance::{ClassificationLabel, Outlook},
};

/// What the user decided about one concept in one goal.
///
/// Three values and no fourth. `NotRequired` and `NotBeneficial` are the two
/// corrections section 18.4 anticipates — the user striking a classification an
/// analysis proposed — and `Required` is the user raising one the analysis did
/// not. Each is a decision about the *outlook* slot; there is no override of
/// `OBSERVED`, because `OBSERVED` is a statement about what the snapshot
/// contains and a user correction to that is a correction to the snapshot's own
/// evidence, which is `P2-R2`'s subject and not this one's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OverrideDecision {
    /// The user says this concept is not required for this goal.
    NotRequired,
    /// The user says this concept is not a conditional benefit for this goal.
    NotBeneficial,
    /// The user says this concept is required, whatever the analysis found.
    Required,
}

impl OverrideDecision {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::NotRequired, Self::NotBeneficial, Self::Required];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "NOT_REQUIRED",
            Self::NotBeneficial => "NOT_BENEFICIAL",
            Self::Required => "REQUIRED",
        }
    }

    /// Which proposed label this decision contradicts.
    ///
    /// Total over the three, with no default arm: a fourth decision has to
    /// answer this rather than inherit an answer.
    #[must_use]
    pub const fn contradicts(self, proposed: ClassificationLabel) -> bool {
        match (self, proposed) {
            (Self::NotRequired, ClassificationLabel::Required)
            | (Self::NotBeneficial, ClassificationLabel::WouldBenefitFrom)
            | (Self::Required, ClassificationLabel::WouldBenefitFrom) => true,
            (Self::NotRequired, ClassificationLabel::WouldBenefitFrom)
            | (Self::NotBeneficial, ClassificationLabel::Required)
            | (Self::Required, ClassificationLabel::Required)
            | (
                Self::NotRequired | Self::NotBeneficial | Self::Required,
                ClassificationLabel::Observed,
            ) => false,
        }
    }
}

/// One standing user decision about one concept in one goal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserOverride {
    goal: GoalScope,
    concept: String,
    decision: OverrideDecision,
    asserted_from_snapshot: String,
    asserted_at: u64,
}

impl UserOverride {
    /// Records a decision the user made.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::InvalidIdentifier`] when the concept is empty,
    /// over 64 bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(
        goal: GoalScope,
        concept: impl Into<String>,
        decision: OverrideDecision,
        asserted_from_snapshot: impl Into<String>,
        asserted_at: u64,
    ) -> Result<Self, ClassificationError> {
        Ok(Self {
            goal,
            concept: validated(concept.into(), "concept")?,
            decision,
            asserted_from_snapshot: asserted_from_snapshot.into(),
            asserted_at,
        })
    }

    /// Which goal version the decision was made under.
    #[must_use]
    pub const fn goal(&self) -> &GoalScope {
        &self.goal
    }

    /// Which concept it is about.
    #[must_use]
    pub fn concept(&self) -> &str {
        &self.concept
    }

    /// What the user decided.
    #[must_use]
    pub const fn decision(&self) -> OverrideDecision {
        self.decision
    }

    /// Which snapshot the user was looking at. Not the snapshot it applies to.
    #[must_use]
    pub fn asserted_from_snapshot(&self) -> &str {
        &self.asserted_from_snapshot
    }

    /// When the decision was made, in milliseconds.
    #[must_use]
    pub const fn asserted_at(&self) -> u64 {
        self.asserted_at
    }

    /// Whether this override governs a classification at `key`.
    ///
    /// The goal and the concept, and deliberately not the snapshot: see the
    /// module documentation.
    #[must_use]
    pub fn governs(&self, key: &ClassificationKey) -> bool {
        self.goal == *key.goal() && self.concept == key.concept()
    }
}

/// The record a disagreement produces, beside both sides and replacing neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationConflict {
    key: ClassificationKey,
    standing: UserOverride,
    proposed: Outlook,
}

impl ClassificationConflict {
    /// Opens a conflict. Crate-private: [`crate::classify`] is the one producer.
    pub(crate) const fn seal(
        key: ClassificationKey,
        standing: UserOverride,
        proposed: Outlook,
    ) -> Self {
        Self {
            key,
            standing,
            proposed,
        }
    }

    /// Which classification the two sides disagree about.
    #[must_use]
    pub const fn key(&self) -> &ClassificationKey {
        &self.key
    }

    /// The user's decision, unchanged.
    #[must_use]
    pub const fn standing_override(&self) -> &UserOverride {
        &self.standing
    }

    /// What the analysis proposed, unchanged and undiscarded.
    #[must_use]
    pub const fn proposed(&self) -> &Outlook {
        &self.proposed
    }

    /// The label the analysis proposed.
    #[must_use]
    pub const fn proposed_label(&self) -> ClassificationLabel {
        self.proposed.label()
    }
}
