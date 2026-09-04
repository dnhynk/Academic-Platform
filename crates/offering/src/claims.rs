//! The two claims behind an offering status, and why one never becomes the
//! other.
//!
//! `t068` section 2.3-4: *Offering status (`CONFIRMED`/`HISTORICALLY_LIKELY`/
//! `UNCERTAIN`/`CANCELLED`) is **not** a claim status; it is an offering
//! aggregate field backed by `OFFICIAL_CONFIRMED` and `PREDICTION` claims.*
//! Section 30.1 writes what happens when both exist:
//!
//! ```text
//! Claim A: "Course X is offered in 2027-1"
//! status OFFICIAL_CONFIRMED · source official schedule · valid 2027-1
//!
//! Claim B: "Course X likely offered in 2027-1"
//! status PREDICTION · historical pattern · confidence .72
//!
//! When A arrives, B is not rewritten as official.
//! B becomes SUPERSEDED_FOR_DECISION while its prediction history remains.
//! ```
//!
//! # Two producers, and no path from one to the other
//!
//! [`forecast_claim`] takes a [`ScoredForecast`] and produces a `PREDICTION`
//! claim. [`confirmation_claim`] takes a
//! [`crate::source::ConfirmationEvidence`] and produces an `OFFICIAL_CONFIRMED`
//! one. Neither takes the other's argument and there is no function in this
//! crate that converts between them, so a forecast that wanted to be official
//! has nothing to hand `confirmation_claim` -- `ConfirmationEvidence` has
//! private fields and one constructor whose first argument is a
//! registration-system reading.
//!
//! The claim layer refuses the same thing a second time on its own terms.
//! `academic_domain::Claim::validate` pairs `EpistemicStatus::Prediction` with
//! `AuthorityClass::Prediction` alone, requires a confidence on it, and
//! requires prediction metadata on it and on nothing else. So a prediction
//! claim relabelled `OFFICIAL_CONFIRMED` fails validation twice over: on the
//! authority pairing, and on carrying a confidence and a window an official
//! fact may not have.
//!
//! # `SUPERSEDED_FOR_DECISION` is not an edit
//!
//! The canonical claim table is append-only twice over (`t068` section 2.3-2),
//! and `EpistemicStatus::Superseded` is lifecycle-terminal and derived.
//! Section 30.1's `SUPERSEDED_FOR_DECISION` is therefore **not** a new value
//! written onto the prediction row: the prediction's bytes never change. It is
//! a property of the *set* -- which claim a decision reads -- and
//! [`OfferingClaimSet::prediction_standing`] is where it is answered.
//! `prediction_official_parallel` observes the prediction claim byte-identical
//! before and after the official one arrives.
//!
//! `t001`'s `REQ-30-002` row records the exact supersession status naming as
//! open. This crate does not settle it by inventing an
//! `EpistemicStatus` variant; it answers the question at the level section
//! 30.1 asks it.
//!
//! # What this crate found one step out
//!
//! ADR-003's actor matrix in `academic_domain::Claim::validate_for_actor`
//! gives `AuthorityClass::Prediction` to `Actor::ModelRun` alone.
//! `Actor::DeterministicEngine` carries `AuthorityClass::DeterministicEngine`
//! and nothing else, so **a deterministic historical forecaster cannot sign
//! its own prediction claim as a deterministic engine** -- while section
//! 30.1's own example of a `PREDICTION` claim is a *historical pattern* and
//! not a model. This crate does not widen the matrix; it records the
//! divergence, and `a_forecast_claim_is_not_signable_by_a_deterministic_engine`
//! executes it so a later widening is a deliberate change rather than a
//! silent one.

use academic_domain::{
    AuthorityClass, Claim, ClaimId, ClaimObject, ConfidencePermille, EntityId, EpistemicStatus,
    EvidenceId, PredicateId, ScopeId, ValidInterval,
};

use crate::{
    error::OfferingError,
    forecast::ScoredForecast,
    source::{ConfirmationEvidence, OfferingAnnouncement},
};

/// The predicate an offering-status claim is asserted under.
pub const OFFERING_STATUS_PREDICATE: &str = "academic.offering.status";

/// The object an offering claim asserts.
///
/// Section 8.3's four statuses are not claim statuses, so the claim's object is
/// the fact -- *this course runs in this term* -- and the epistemic status says
/// on what footing it is asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferingAssertion {
    /// The course runs in the term.
    Runs,
    /// The course does not run in the term.
    ///
    /// Reachable only from an official cancellation notice. A forecast has no
    /// producer for it, which is section 8.3's *미개설 확정이 아니다* held as
    /// an absence: a never-observed course produces no claim of any kind.
    DoesNotRun,
}

impl OfferingAssertion {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runs => "RUNS",
            Self::DoesNotRun => "DOES_NOT_RUN",
        }
    }
}

/// Who the claim is about and where it applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimSubject {
    /// The offering the claim is about.
    pub subject_entity_id: EntityId,
    /// The scope the claim applies in.
    pub scope_id: ScopeId,
    /// The interval the claim applies over.
    pub valid_time: ValidInterval,
}

/// Builds the `PREDICTION` claim behind a `HISTORICALLY_LIKELY` standing.
///
/// The confidence is the calibrated permille and the metadata is the disclosed
/// observation window, both taken off a [`ScoredForecast`] -- which is the only
/// value in this crate that holds either, and which cannot be built without
/// both.
///
/// # Errors
///
/// [`OfferingError`] when the predicate, the confidence or the assembled claim
/// is refused by the domain layer.
pub fn forecast_claim(
    id: ClaimId,
    subject: &ClaimSubject,
    scored: &ScoredForecast,
    evidence_ids: Vec<EvidenceId>,
) -> Result<Claim, OfferingError> {
    let claim = Claim {
        id,
        subject_entity_id: subject.subject_entity_id,
        predicate_id: PredicateId::parse(OFFERING_STATUS_PREDICATE)?,
        object: ClaimObject::Text(OfferingAssertion::Runs.as_str().to_owned()),
        scope_id: subject.scope_id,
        authority_class: AuthorityClass::Prediction,
        epistemic_status: EpistemicStatus::Prediction,
        confidence: Some(ConfidencePermille::new(scored.confidence().value())?),
        prediction_metadata: Some(scored.metadata()),
        valid_time: subject.valid_time,
        evidence_ids,
    };
    claim.validate()?;
    Ok(claim)
}

/// Builds the `OFFICIAL_CONFIRMED` claim behind a `CONFIRMED` standing.
///
/// It takes a [`ConfirmationEvidence`] and nothing else, carries no confidence
/// -- section 30.5: *공식 사실에는 AI confidence를 붙이지 않는다*, and
/// `Claim::validate` refuses one on an `OFFICIAL_CONFIRMED` claim -- and
/// carries no prediction metadata, which the same function refuses on
/// anything but a prediction.
///
/// # Errors
///
/// [`OfferingError`] when the predicate or the assembled claim is refused by
/// the domain layer.
pub fn confirmation_claim(
    id: ClaimId,
    subject: &ClaimSubject,
    evidence: &ConfirmationEvidence,
    evidence_ids: Vec<EvidenceId>,
) -> Result<Claim, OfferingError> {
    let assertion = if evidence.basis().lists_a_section() {
        OfferingAssertion::Runs
    } else {
        OfferingAssertion::DoesNotRun
    };
    let claim = Claim {
        id,
        subject_entity_id: subject.subject_entity_id,
        predicate_id: PredicateId::parse(OFFERING_STATUS_PREDICATE)?,
        object: ClaimObject::Text(assertion.as_str().to_owned()),
        scope_id: subject.scope_id,
        authority_class: AuthorityClass::Official,
        epistemic_status: EpistemicStatus::OfficialConfirmed,
        confidence: None,
        prediction_metadata: None,
        valid_time: subject.valid_time,
        evidence_ids,
    };
    claim.validate()?;
    Ok(claim)
}

/// Builds the `OFFICIAL_CONFIRMED` claim an announcement activates.
///
/// Section 8.3: *공식 향후 공지가 생기면 예측을 사실로 "승격"하지 않고 별도
/// official Claim을 활성화한다.* This is that separate claim. It is official
/// because a department announcing its own offering is an official fact, and it
/// is **not** a confirmation of the offering aggregate: no listing has been
/// verified, so no seat exists and the standing is `UNCERTAIN`. Section 2.3-4
/// is what makes those two answers compatible -- offering status is an
/// aggregate field, not a claim status.
///
/// The notice bounds the claim: an official claim that applied before the
/// notice announcing it would be an official fact backdated past its own
/// source, so [`OfferingError::ClaimPredatesItsNotice`] refuses one. That is
/// the whole of what the announcement argument does, and it is the reason the
/// argument is here -- a parameter that reached no part of the output would be
/// the defect `offering_feature_contract` refuses one level down.
///
/// # Errors
///
/// [`OfferingError::ClaimPredatesItsNotice`] when the claim's validity starts
/// before the notice was issued, and [`OfferingError`] when the predicate or
/// the assembled claim is refused by the domain layer.
pub fn announcement_claim(
    id: ClaimId,
    subject: &ClaimSubject,
    announcement: &OfferingAnnouncement,
    evidence_ids: Vec<EvidenceId>,
) -> Result<Claim, OfferingError> {
    if subject.valid_time.from() < announcement.issued_at() {
        return Err(OfferingError::ClaimPredatesItsNotice);
    }
    let claim = Claim {
        id,
        subject_entity_id: subject.subject_entity_id,
        predicate_id: PredicateId::parse(OFFERING_STATUS_PREDICATE)?,
        object: ClaimObject::Text(OfferingAssertion::Runs.as_str().to_owned()),
        scope_id: subject.scope_id,
        authority_class: AuthorityClass::Official,
        epistemic_status: EpistemicStatus::OfficialConfirmed,
        confidence: None,
        prediction_metadata: None,
        valid_time: subject.valid_time,
        evidence_ids,
    };
    claim.validate()?;
    Ok(claim)
}

/// Which claim a decision reads, and which one is history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionStanding {
    /// The claim a decision reads.
    ActiveForDecision,
    /// Section 30.1's `SUPERSEDED_FOR_DECISION`: an official claim arrived, so
    /// this prediction is no longer what a decision reads. Its row is
    /// unchanged and its history remains.
    SupersededForDecision {
        /// The official claim that took over the decision.
        by: ClaimId,
    },
}

impl DecisionStanding {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActiveForDecision => "ACTIVE_FOR_DECISION",
            Self::SupersededForDecision { .. } => "SUPERSEDED_FOR_DECISION",
        }
    }
}

/// The claims standing behind one offering, side by side.
///
/// Both are kept. There is no method here that removes one, edits one, or
/// changes one's `epistemic_status`: the set answers *which one a decision
/// reads* and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferingClaimSet {
    prediction: Option<Claim>,
    official: Option<Claim>,
}

impl OfferingClaimSet {
    /// A set holding only a prediction.
    #[must_use]
    pub const fn predicted(prediction: Claim) -> Self {
        Self {
            prediction: Some(prediction),
            official: None,
        }
    }

    /// A set holding only an official claim.
    #[must_use]
    pub const fn official(official: Claim) -> Self {
        Self {
            prediction: None,
            official: Some(official),
        }
    }

    /// Records that an official claim arrived beside an existing prediction.
    ///
    /// Takes `self` by value and returns a new set, so the arrival is an
    /// append rather than a mutation of the prediction. The prediction claim
    /// travels through unchanged, which is the whole of section 30.1's
    /// *B is not rewritten as official*.
    #[must_use]
    pub fn official_arrived(self, official: Claim) -> Self {
        Self {
            prediction: self.prediction,
            official: Some(official),
        }
    }

    /// The prediction claim, when one was made.
    #[must_use]
    pub const fn prediction(&self) -> Option<&Claim> {
        self.prediction.as_ref()
    }

    /// The official claim, when one arrived.
    #[must_use]
    pub const fn official_claim(&self) -> Option<&Claim> {
        self.official.as_ref()
    }

    /// What the prediction's standing is for a decision.
    ///
    /// `None` when there is no prediction to have a standing.
    #[must_use]
    pub fn prediction_standing(&self) -> Option<DecisionStanding> {
        self.prediction.as_ref()?;
        Some(match &self.official {
            Some(official) => DecisionStanding::SupersededForDecision { by: official.id },
            None => DecisionStanding::ActiveForDecision,
        })
    }

    /// What the official claim's standing is for a decision.
    ///
    /// An official claim is always what a decision reads: section 30.3's
    /// official-academic-fact column ranks `OFFICIAL` at 800 and `PREDICTION`
    /// at 100, and `academic-ledger` already resolves that. Nothing here
    /// re-decides it; this repeats the answer for the pair in hand.
    #[must_use]
    pub fn official_standing(&self) -> Option<DecisionStanding> {
        self.official.as_ref()?;
        Some(DecisionStanding::ActiveForDecision)
    }
}
