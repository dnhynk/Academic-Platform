//! The three dispositions, the user receipt each one needs, and the append-only
//! history that makes one reversible.
//!
//! # Why three and not four
//!
//! The execution plan names its acceptance test
//! `four_dispositions_are_durable_and_audited`. There are three.
//!
//! Section 3 of the authoritative spec, under the heading that says which
//! decisions the user owns, names exactly three things a user does with an AI
//! proposal: approve it, modify it, or reject it. No section names a fourth,
//! and [`DecisionAction`] -- which ADR-003 froze, and whose semantics the
//! ledger's resolver already replays in acceptance order -- has exactly those
//! three arms. So this module adds no vocabulary: the disposition a record
//! carries *is* a `DecisionAction`, and `Confirm`, `Replace` and `Reject` are
//! section 3's approve, modify and reject.
//!
//! A proposal nobody has decided on yet is *pending*, which is a state of the
//! queue and not a decision. It is [`DispositionState::Undisposed`], a variant
//! that is not a `DecisionAction` and has no conversion into one, so it cannot
//! reach anything that ranks user authority. `pending_is_not_a_disposition` in
//! `tests/compile_fail` is that fact as a compile error.

use academic_domain::{Actor, DecisionAction};
use sha2::{Digest, Sha256};

use crate::{error::WorkflowError, proposed::ProposalId};

/// The stable spelling of a disposition, as the frozen wire contract spells it.
///
/// `DecisionAction` carries `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`, so
/// these three are not a second vocabulary beside the enum's own -- they are
/// its own, and `the_disposition_tokens_are_the_frozen_serde_spellings`
/// compares each one against what serde emits rather than against a list
/// written twice.
#[must_use]
pub const fn disposition_token(action: &DecisionAction) -> &'static str {
    match action {
        DecisionAction::Confirm => "CONFIRM",
        DecisionAction::Reject => "REJECT",
        DecisionAction::Replace { .. } => "REPLACE",
    }
}

/// Whether this disposition lets the proposal's payload become a record.
///
/// Only `Confirm` does. `Reject` retains the proposal and writes nothing, and
/// `Replace` selects a different object -- ADR-003's rule is that a replacement
/// rejects A while selecting B -- so the model's payload is not what becomes
/// the record and [`crate::ReviewQueue::commit`] refuses it by name.
#[must_use]
pub const fn releases_the_payload(action: &DecisionAction) -> bool {
    matches!(action, DecisionAction::Confirm)
}

/// Proof that a human, and not an automatic actor, made a decision.
///
/// Private field, no `Default`, and one producer -- [`UserDecision::by`] --
/// which takes an [`Actor`] and refuses every variant that is not
/// [`Actor::User`]. The `match` is exhaustive over that closed enum rather than
/// a negated list, so a fifth actor variant added to `academic-domain` stops
/// this crate compiling until it says which side it is on.
///
/// This is the shape `P2-K6`'s verified admission receipt uses and the one
/// `VerifiedCuratorApproval` uses in `academic-domain`: the receipt cannot be
/// assembled with a struct literal from outside, so a caller cannot manufacture
/// the conclusion that a user decided. The admission receipt is named by
/// reference rather than spelled, because `no_environment_or_flag_override_exists`
/// in `crates/cli/src/main.rs` holds that type name to zero occurrences in every
/// package but `academic-admission` -- a rule that predates this crate and that
/// this crate has no reason to widen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDecision {
    user_id: u128,
}

impl UserDecision {
    /// Issues a receipt for a user actor.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::AutomaticActor`] for a deterministic engine, a model
    /// run, or an importer.
    pub fn by(actor: &Actor) -> Result<Self, WorkflowError> {
        match actor {
            Actor::User { user_id } => Ok(Self {
                user_id: u128::from_be_bytes(*user_id.as_bytes()),
            }),
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                Err(WorkflowError::AutomaticActor {
                    actor: actor.kind_name(),
                })
            }
        }
    }

    /// The deciding user's identifier.
    #[must_use]
    pub const fn user_id(&self) -> u128 {
        self.user_id
    }
}

/// A user approval that names one exact proposal.
///
/// Section 27.4's high-risk row requires an explicit approval, and what makes
/// it explicit is that the receipt carries the identity of the proposal it
/// approves. Private fields and one producer, [`ExplicitApproval::of`], which
/// takes a [`UserDecision`], so a blanket approval is not a value that exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitApproval {
    proposal_id: ProposalId,
    decision: UserDecision,
}

impl ExplicitApproval {
    /// Binds a user decision to one proposal.
    #[must_use]
    pub const fn of(proposal_id: ProposalId, decision: UserDecision) -> Self {
        Self {
            proposal_id,
            decision,
        }
    }

    /// The proposal this approval names.
    #[must_use]
    pub const fn proposal_id(&self) -> ProposalId {
        self.proposal_id
    }

    /// The user decision behind it.
    #[must_use]
    pub const fn decision(&self) -> &UserDecision {
        &self.decision
    }
}

/// Position of one record in a proposal's append-only history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DispositionSeq(u32);

impl DispositionSeq {
    /// The first sequence number a history uses.
    ///
    /// One, not zero. `P2-G6` found a key that started at zero and could not
    /// distinguish "the first record" from "no record"; this starts at one for
    /// the same reason.
    pub const FIRST: Self = Self(1);

    /// The integer position.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// The next position.
    pub(crate) const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl core::fmt::Display for DispositionSeq {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// One entry in a proposal's disposition history.
///
/// Nothing edits one of these. An undo appends a record whose
/// [`DispositionRecord::supersedes`] names the record it reverses, exactly as
/// ADR-003 requires of every canonical correction, and both rows stay so the
/// history reads as what happened rather than as what is true now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispositionRecord {
    seq: DispositionSeq,
    proposal_id: ProposalId,
    disposition: DecisionAction,
    user_id: u128,
    decided_at: u64,
    supersedes: Option<DispositionSeq>,
    record_digest: [u8; 32],
}

impl DispositionRecord {
    pub(crate) fn new(
        seq: DispositionSeq,
        proposal_id: ProposalId,
        disposition: DecisionAction,
        decision: &UserDecision,
        decided_at: u64,
        supersedes: Option<DispositionSeq>,
    ) -> Self {
        let record_digest = disposition_digest(
            seq,
            proposal_id,
            &disposition,
            decision.user_id(),
            decided_at,
            supersedes,
        );
        Self {
            seq,
            proposal_id,
            disposition,
            user_id: decision.user_id(),
            decided_at,
            supersedes,
            record_digest,
        }
    }

    /// Where this record sits in the proposal's history.
    #[must_use]
    pub const fn seq(&self) -> DispositionSeq {
        self.seq
    }

    /// The proposal this record is about.
    #[must_use]
    pub const fn proposal_id(&self) -> ProposalId {
        self.proposal_id
    }

    /// What the user did, in the ledger's own frozen vocabulary.
    #[must_use]
    pub const fn disposition(&self) -> &DecisionAction {
        &self.disposition
    }

    /// Which user did it.
    #[must_use]
    pub const fn user_id(&self) -> u128 {
        self.user_id
    }

    /// When, in milliseconds.
    #[must_use]
    pub const fn decided_at(&self) -> u64 {
        self.decided_at
    }

    /// The earlier record this one reverses, if it is an undo.
    #[must_use]
    pub const fn supersedes(&self) -> Option<DispositionSeq> {
        self.supersedes
    }

    /// SHA-256 over every field above, in the order they are declared.
    ///
    /// `the_disposition_digest_covers_every_field` changes each field in turn
    /// and requires each change to move the digest, so a field added without
    /// being hashed fails rather than producing a digest that does not describe
    /// the record.
    #[must_use]
    pub const fn record_digest(&self) -> &[u8; 32] {
        &self.record_digest
    }
}

/// The digest a [`DispositionRecord`] carries.
///
/// `Replace` names the claim it selects, and that identifier is part of what
/// the user decided, so it is hashed. Hashing only the token would give a
/// `Replace` of one claim and a `Replace` of another the same digest.
fn disposition_digest(
    seq: DispositionSeq,
    proposal_id: ProposalId,
    disposition: &DecisionAction,
    user_id: u128,
    decided_at: u64,
    supersedes: Option<DispositionSeq>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"academic-proposal/disposition/1\n");
    hasher.update(seq.value().to_be_bytes());
    hasher.update(proposal_id.value().to_be_bytes());
    hasher.update(disposition_token(disposition).as_bytes());
    match disposition {
        DecisionAction::Replace {
            replacement_claim_id,
        } => {
            hasher.update([1_u8]);
            hasher.update(replacement_claim_id.as_bytes());
        }
        DecisionAction::Confirm | DecisionAction::Reject => hasher.update([0_u8]),
    }
    hasher.update(b"\n");
    hasher.update(user_id.to_be_bytes());
    hasher.update(decided_at.to_be_bytes());
    match supersedes {
        Some(superseded) => {
            hasher.update([1_u8]);
            hasher.update(superseded.value().to_be_bytes());
        }
        None => hasher.update([0_u8]),
    }
    hasher.finalize().into()
}
