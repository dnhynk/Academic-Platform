//! Section 13.3's fifth and seventh inputs: what the user says about their own
//! recall, and what contradicts it.
//!
//! ## Raising freshness needs the user; lowering it does not
//!
//! The two inputs are not symmetric and they are not typed symmetrically.
//!
//! * [`RecallStatement`] is the user's own
//!   `“지금도 바로 사용할 수 있음/복습 필요” 확인`. It can **raise** a band to
//!   the top, so its one constructor runs ADR-003's actor matrix exactly as
//!   `P2-N2`'s `UserConfirmation` does: a model run attempting the
//!   `USER_EXPLICIT`/`USER_CONFIRMED` pairing fails that matrix, and there is no
//!   other way to build the value. An AI cannot say on the user's behalf that
//!   they can still use something.
//! * [`ContraryEvent`] is `설명 실패, 기억 안 남음 표시, 재학습 필요 event`. It
//!   only ever **lowers** a band, so it is an observation carrying its own
//!   evidence item rather than an authorization. Requiring the user's signature
//!   to record that an explanation failed would lose the observation, and losing
//!   it is the failure mode section 13.3's seventh bullet exists to prevent.
//!
//! Neither touches mastery. Nothing in this crate can name a mastery level, so
//! `모름` is not a state a recall failure can put a concept into — it is a
//! statement about retrieval and it stays one. That is section 1's fifth
//! invariant held by the dependency graph rather than by a rule.
//!
//! ## A statement is a claim about a band, in the vocabulary that already exists
//!
//! `academic_domain::ClaimObject::Freshness` is already the wire shape of a
//! claim about a band, so a recall statement is verified against a `Claim` whose
//! object is the band the statement means. There is no second vocabulary and no
//! second scale.

use academic_domain::{
    Actor, Claim, ClaimObject, EntityId, EpistemicStatus, EvidenceId, EvidenceItem, FreshnessBand,
    ScopeId, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::FreshnessError;

/// The predicate a recall statement claim carries.
pub const RECALL_STATEMENT_PREDICATE: &str = "knowledge.freshness.recall";

/// Section 13.3's two user statements, in the order the bullet writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserRecall {
    /// `지금도 바로 사용할 수 있음`
    CanUseNow,
    /// `복습 필요`
    NeedsReview,
}

impl UserRecall {
    /// Both, in the bullet's own order.
    pub const ALL: [Self; 2] = [Self::CanUseNow, Self::NeedsReview];

    /// The design document's own phrase, verbatim.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::CanUseNow => "지금도 바로 사용할 수 있음",
            Self::NeedsReview => "복습 필요",
        }
    }

    /// The band this statement claims.
    ///
    /// `지금도 바로 사용할 수 있음` is section 13.3's own definition of the
    /// axis — `즉시 인출 가능성` — asserted at its top. `복습 필요` is the user
    /// saying retrieval is not immediate, which is a ceiling rather than a
    /// verdict: it leaves `LOW` reachable and leaves `STALE` to elapsed time.
    #[must_use]
    pub const fn band(self) -> FreshnessBand {
        match self {
            Self::CanUseNow => FreshnessBand::VeryHigh,
            Self::NeedsReview => FreshnessBand::Low,
        }
    }

    /// Whether this statement can raise a band or only cap it.
    #[must_use]
    pub const fn raises(self) -> bool {
        matches!(self, Self::CanUseNow)
    }
}

/// Section 13.3's three contrary events, in the order the bullet lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContraryKind {
    /// `설명 실패`
    ExplanationFailure,
    /// `기억 안 남음 표시`
    NoMemoryMarked,
    /// `재학습 필요 event`
    RelearningNeeded,
}

impl ContraryKind {
    /// All three, in the bullet's own order.
    pub const ALL: [Self; 3] = [
        Self::ExplanationFailure,
        Self::NoMemoryMarked,
        Self::RelearningNeeded,
    ];

    /// The design document's own phrase, verbatim.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::ExplanationFailure => "설명 실패",
            Self::NoMemoryMarked => "기억 안 남음 표시",
            Self::RelearningNeeded => "재학습 필요 event",
        }
    }

    /// The highest band this event leaves reachable.
    ///
    /// The three phrases say different things and are capped differently. A
    /// failed explanation is a failure to produce, which leaves recognition
    /// intact — `LOW`. `기억 안 남음` and `재학습 필요` are the user reporting
    /// that nothing is retrievable at all, which is `STALE`: section 13.3's own
    /// example block spells out that `STALE` still means
    /// `과거 이해 evidence는 유지되지만`, so the strongest contrary evidence
    /// this axis admits still says nothing about what was learned.
    ///
    /// It is never `UNKNOWN`. `UNKNOWN` is the band for a concept nothing
    /// datable was recorded about, and a recall failure *is* a record.
    #[must_use]
    pub const fn ceiling(self) -> FreshnessBand {
        match self {
            Self::ExplanationFailure => FreshnessBand::Low,
            Self::NoMemoryMarked | Self::RelearningNeeded => FreshnessBand::Stale,
        }
    }
}

/// The user's own statement about their recall, after ADR-003 accepted it.
///
/// Fields are private and the only constructor is [`RecallStatement::verify`],
/// which runs `Claim::validate_for_actor`. `academic_domain::Actor`'s automatic
/// variants all fail that matrix for a `USER_CONFIRMED` claim, so a model run
/// holding every other input still cannot produce this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecallStatement {
    user_id: EntityId,
    concept: EntityId,
    scope_id: ScopeId,
    statement: UserRecall,
    stated_at: TimestampMillis,
}

impl RecallStatement {
    /// Verifies that `claim` is this user's own recall statement about
    /// `concept`.
    ///
    /// # Errors
    ///
    /// [`FreshnessError::Domain`] when ADR-003's actor/authority/status matrix
    /// rejects the claim or its evidence, [`FreshnessError::NotARecallStatement`]
    /// when the claim is not one, [`FreshnessError::RecallSubjectMismatch`] when
    /// it names another concept, [`FreshnessError::RecallEvidenceMissing`] when
    /// it does not cite the evidence offered, and
    /// [`FreshnessError::RecallBandMismatch`] when the claimed band is not the
    /// one the statement means.
    pub fn verify(
        actor: &Actor,
        claim: &Claim,
        evidence: &EvidenceItem,
        concept: EntityId,
        statement: UserRecall,
        stated_at: TimestampMillis,
    ) -> Result<Self, FreshnessError> {
        claim.validate_for_actor(actor)?;
        if claim.epistemic_status != EpistemicStatus::UserConfirmed
            || claim.predicate_id.as_str() != RECALL_STATEMENT_PREDICATE
        {
            return Err(FreshnessError::NotARecallStatement);
        }
        match &claim.object {
            ClaimObject::Freshness(band) if *band == statement.band() => {}
            ClaimObject::Freshness(_) => return Err(FreshnessError::RecallBandMismatch),
            _ => return Err(FreshnessError::NotARecallStatement),
        }
        if claim.subject_entity_id != concept {
            return Err(FreshnessError::RecallSubjectMismatch);
        }
        if !claim.evidence_ids.contains(&evidence.id) {
            return Err(FreshnessError::RecallEvidenceMissing);
        }
        evidence.validate()?;
        let Actor::User { user_id } = actor else {
            // The matrix above already rejects this branch for a valid
            // user-confirmed claim; keeping the pattern match makes the
            // resulting value structurally user-only if that matrix ever grows.
            return Err(FreshnessError::NotARecallStatement);
        };
        Ok(Self {
            user_id: *user_id,
            concept,
            scope_id: claim.scope_id,
            statement,
            stated_at,
        })
    }

    /// Whose statement.
    #[must_use]
    pub const fn user_id(&self) -> EntityId {
        self.user_id
    }

    /// About which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// In which resolution scope.
    #[must_use]
    pub const fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    /// Which of the two statements.
    #[must_use]
    pub const fn statement(&self) -> UserRecall {
        self.statement
    }

    /// When it was made.
    #[must_use]
    pub const fn stated_at(&self) -> TimestampMillis {
        self.stated_at
    }
}

/// One `설명 실패, 기억 안 남음 표시, 재학습 필요 event`, with its own evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContraryEvent {
    kind: ContraryKind,
    concept: EntityId,
    evidence_id: EvidenceId,
    observed_at: TimestampMillis,
}

impl ContraryEvent {
    /// Records one contrary event about `concept`.
    #[must_use]
    pub const fn of(
        kind: ContraryKind,
        concept: EntityId,
        evidence_id: EvidenceId,
        observed_at: TimestampMillis,
    ) -> Self {
        Self {
            kind,
            concept,
            evidence_id,
            observed_at,
        }
    }

    /// Which of the three.
    #[must_use]
    pub const fn kind(&self) -> ContraryKind {
        self.kind
    }

    /// About which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Which evidence item records it.
    #[must_use]
    pub const fn evidence_id(&self) -> EvidenceId {
        self.evidence_id
    }

    /// When it was observed.
    #[must_use]
    pub const fn observed_at(&self) -> TimestampMillis {
        self.observed_at
    }
}

/// Which way one recall datum points, which is all calibration reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecallDirection {
    /// The user retrieved it.
    Retained,
    /// They did not.
    NotRetained,
}

/// One datum of the user's own recall record, which is the only thing
/// [`crate::persistence::RetentionPrior::calibrate`] reads.
///
/// Private fields and two constructors, both of which take a value that only
/// the user or a direct observation can produce. **There is no constructor
/// taking a band, a projection or a spillover contribution**, which is what
/// stops calibration from becoming a second path by which one concept's state
/// reaches another's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecallCheck {
    concept: EntityId,
    direction: RecallDirection,
    checked_at: TimestampMillis,
}

impl RecallCheck {
    /// The user's own statement, as a calibration datum.
    #[must_use]
    pub const fn from_statement(statement: &RecallStatement) -> Self {
        Self {
            concept: statement.concept(),
            direction: match statement.statement() {
                UserRecall::CanUseNow => RecallDirection::Retained,
                UserRecall::NeedsReview => RecallDirection::NotRetained,
            },
            checked_at: statement.stated_at(),
        }
    }

    /// A contrary event, as a calibration datum.
    #[must_use]
    pub const fn from_contrary(event: &ContraryEvent) -> Self {
        Self {
            concept: event.concept(),
            direction: RecallDirection::NotRetained,
            checked_at: event.observed_at(),
        }
    }

    /// About which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Which way it points.
    #[must_use]
    pub const fn direction(&self) -> RecallDirection {
        self.direction
    }

    /// When.
    #[must_use]
    pub const fn checked_at(&self) -> TimestampMillis {
        self.checked_at
    }
}
