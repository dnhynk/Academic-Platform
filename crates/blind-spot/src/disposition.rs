//! Section 23's four dispositions, the user action that records one, and the
//! ledger that carries them across a recomputation.
//!
//! The count is not a number this file chose. `four_dispositions_are_durable`
//! reads section 23's own bullet — `사용자가 …을 선택한다` — back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, splits it on its own
//! back-quoted spellings and compares them against [`DISPOSITIONS`] in both
//! directions.
//!
//! ## The schema example names a fifth spelling and it is not a fifth
//! disposition
//!
//! Section 23's `BlindSpotFinding` example ends with
//! `userDisposition: ACKNOWLEDGED_NOT_CURRENTLY_RELEVANT`, which is not one of
//! the four the bullet enumerates and which `REQ-23-014` does not name. The
//! bullet is the normative enumeration, because it is the half that fixes the
//! identifiers the user picks from. [`SCHEMA_EXAMPLE_DISPOSITION`] keeps the
//! example's spelling so the discrepancy is a measured value with a test on it
//! rather than one a later reader rediscovers, and
//! `docs/contracts/blind-spot-detector.md` records it. Nothing routes on it.
//!
//! ## A model run cannot record, change or clear one
//!
//! [`UserDispositionChoice::verify`] is the only constructor and it runs
//! ADR-003's actor matrix, the way `P2-N2`'s `UserConfirmation` and `P2-N3`'s
//! `RecallStatement` do. `academic_domain::Actor`'s automatic variants all fail
//! that matrix for a `USER_CONFIRMED` claim, so a model run holding every other
//! input still cannot produce this value — which is what makes `새로운 AI run이
//! 경고를 되살리지 않는다` a property of the type rather than a rule in a
//! recomputation path somebody has to remember to write.
//!
//! [`DispositionLedger`] has no removal method and no `&mut self` method. Every
//! operation that changes it consumes it and returns a new one, so a
//! recomputation that dropped a `NOT_RELEVANT` would have to build a ledger
//! without it rather than edit the one it was handed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use academic_domain::{
    Actor, Claim, ClaimObject, EntityId, EpistemicStatus, EvidenceItem, ScopeId, TimestampMillis,
};

use crate::BlindSpotError;

/// The predicate a disposition claim carries.
pub const DISPOSITION_PREDICATE: &str = "knowledge.blindspot.disposition";

/// The `userDisposition` spelling section 23's schema example exhibits.
///
/// Not one of [`DISPOSITIONS`]. See the module note.
pub const SCHEMA_EXAMPLE_DISPOSITION: &str = "ACKNOWLEDGED_NOT_CURRENTLY_RELEVANT";

/// Section 23's four dispositions, in the order its bullet writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserDisposition {
    /// `EXPLORE`. The one disposition that licenses a taste path.
    Explore,
    /// `LATER`.
    Later,
    /// `NOT_RELEVANT`. Section 39's `경고와 추천에서 제외한다`.
    NotRelevant,
    /// `HIDE_UNTIL`. Carries a deadline; see [`UserDispositionChoice::hidden_until`].
    HideUntil,
}

/// The four, in section 23's own order.
pub const DISPOSITIONS: [UserDisposition; 4] = [
    UserDisposition::Explore,
    UserDisposition::Later,
    UserDisposition::NotRelevant,
    UserDisposition::HideUntil,
];

impl UserDisposition {
    /// Stable spelling, identical to the bullet's own back-quoted name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Explore => "EXPLORE",
            Self::Later => "LATER",
            Self::NotRelevant => "NOT_RELEVANT",
            Self::HideUntil => "HIDE_UNTIL",
        }
    }

    /// Whether this disposition needs a deadline.
    ///
    /// Total, with no wildcard arm. Exactly one of the four does.
    #[must_use]
    pub const fn needs_deadline(self) -> bool {
        match self {
            Self::HideUntil => true,
            Self::Explore | Self::Later | Self::NotRelevant => false,
        }
    }
}

/// One disposition the user recorded, after ADR-003 accepted the claim.
///
/// Fields are private and the only constructor is [`UserDispositionChoice::verify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserDispositionChoice {
    user_id: EntityId,
    field: EntityId,
    scope_id: ScopeId,
    disposition: UserDisposition,
    hidden_until: Option<TimestampMillis>,
    chosen_at: TimestampMillis,
}

impl UserDispositionChoice {
    /// Verifies that `claim` is this user's own disposition for `field`.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::Domain`] when ADR-003's actor/authority/status matrix
    /// rejects the claim or its evidence;
    /// [`BlindSpotError::NotADispositionChoice`] when the claim is not one;
    /// [`BlindSpotError::DispositionSubjectMismatch`] when it names another
    /// field; [`BlindSpotError::DispositionEvidenceMissing`] when it does not
    /// cite the evidence offered; [`BlindSpotError::DeadlineRequired`] and
    /// [`BlindSpotError::DeadlineNotAllowed`] when the deadline does not match
    /// [`UserDisposition::needs_deadline`]; and
    /// [`BlindSpotError::DeadlineIsNotInTheFuture`] when a `HIDE_UNTIL` deadline
    /// does not outlast the instant it was chosen at.
    pub fn verify(
        actor: &Actor,
        claim: &Claim,
        evidence: &EvidenceItem,
        field: EntityId,
        disposition: UserDisposition,
        hidden_until: Option<TimestampMillis>,
        chosen_at: TimestampMillis,
    ) -> Result<Self, BlindSpotError> {
        claim.validate_for_actor(actor)?;
        if claim.epistemic_status != EpistemicStatus::UserConfirmed
            || claim.predicate_id.as_str() != DISPOSITION_PREDICATE
        {
            return Err(BlindSpotError::NotADispositionChoice);
        }
        match &claim.object {
            ClaimObject::Text(token) if token == disposition.as_str() => {}
            _ => return Err(BlindSpotError::NotADispositionChoice),
        }
        if claim.subject_entity_id != field {
            return Err(BlindSpotError::DispositionSubjectMismatch);
        }
        if !claim.evidence_ids.contains(&evidence.id) {
            return Err(BlindSpotError::DispositionEvidenceMissing);
        }
        evidence.validate()?;
        match (disposition.needs_deadline(), hidden_until) {
            (true, None) => return Err(BlindSpotError::DeadlineRequired),
            (false, Some(_)) => return Err(BlindSpotError::DeadlineNotAllowed(disposition)),
            (true, Some(until)) if until.value() <= chosen_at.value() => {
                return Err(BlindSpotError::DeadlineIsNotInTheFuture);
            }
            _ => {}
        }
        let Actor::User { user_id } = actor else {
            // The matrix above already rejects this branch for a valid
            // user-confirmed claim; keeping the pattern match makes the
            // resulting value structurally user-only if that matrix ever grows.
            return Err(BlindSpotError::NotADispositionChoice);
        };
        Ok(Self {
            user_id: *user_id,
            field,
            scope_id: claim.scope_id,
            disposition,
            hidden_until,
            chosen_at,
        })
    }

    /// Whose choice.
    #[must_use]
    pub const fn user_id(self) -> EntityId {
        self.user_id
    }

    /// Which field.
    #[must_use]
    pub const fn field(self) -> EntityId {
        self.field
    }

    /// Which resolution scope the claim was made under.
    #[must_use]
    pub const fn scope_id(self) -> ScopeId {
        self.scope_id
    }

    /// Which of the four.
    #[must_use]
    pub const fn disposition(self) -> UserDisposition {
        self.disposition
    }

    /// The `HIDE_UNTIL` deadline, absent for the other three.
    #[must_use]
    pub const fn hidden_until(self) -> Option<TimestampMillis> {
        self.hidden_until
    }

    /// When it was chosen.
    #[must_use]
    pub const fn chosen_at(self) -> TimestampMillis {
        self.chosen_at
    }

    /// Whether this choice still suppresses a warning at `as_of`.
    ///
    /// Total, with no wildcard arm. `NOT_RELEVANT` never stops suppressing —
    /// section 39's `경고와 추천에서 제외한다` has no expiry — and `HIDE_UNTIL`
    /// stops the moment the clock reaches its deadline, which is section 34.5's
    /// recovery column read forward rather than a second rule.
    #[must_use]
    pub fn suppresses_warning_at(self, as_of: TimestampMillis) -> bool {
        match self.disposition {
            UserDisposition::NotRelevant => true,
            UserDisposition::HideUntil => self
                .hidden_until
                .is_some_and(|until| as_of.value() < until.value()),
            UserDisposition::Explore | UserDisposition::Later => false,
        }
    }
}

/// Every disposition the user has recorded, keyed by field.
///
/// Append-only in both senses: there is no removal method, and every operation
/// that changes it consumes it and returns a new one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispositionLedger {
    entries: BTreeMap<EntityId, UserDispositionChoice>,
}

impl DispositionLedger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Records `choice`, replacing this field's earlier choice if any.
    ///
    /// Consumes the ledger and returns a new one.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::DispositionIsOlderThanTheOneItReplaces`] when the
    /// choice is not newer than the one already recorded for that field, so a
    /// re-run replaying an older claim cannot undo a later decision.
    pub fn record(mut self, choice: UserDispositionChoice) -> Result<Self, BlindSpotError> {
        if let Some(standing) = self.entries.get(&choice.field())
            && choice.chosen_at().value() <= standing.chosen_at().value()
        {
            return Err(BlindSpotError::DispositionIsOlderThanTheOneItReplaces);
        }
        self.entries.insert(choice.field(), choice);
        Ok(self)
    }

    /// This field's standing choice, if the user made one.
    #[must_use]
    pub fn standing(&self, field: EntityId) -> Option<&UserDispositionChoice> {
        self.entries.get(&field)
    }

    /// Every field the user has recorded a choice for, in identity order.
    #[must_use]
    pub fn fields(&self) -> Vec<EntityId> {
        self.entries.keys().copied().collect()
    }

    /// How many choices are recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no choice is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
