//! The consolidated review queue.
//!
//! Four entry points settle a proposal, one per [`Workflow`], and each one
//! names the workflow it serves. [`ReviewQueue::require`] is the single place
//! that compares an entry's tier against the workflow a caller reached for, so
//! the tier-to-workflow mapping is executed once and every path goes through
//! it: a `MEDIUM_REVIEW` proposal handed to the autosave door is refused by the
//! same code that refuses a `LOW_AUTOSAVE` proposal handed to the explicit
//! approval door.

use std::collections::BTreeMap;

use academic_domain::DecisionAction;

use crate::{
    batching::{Batch, BatchKey, BatchingThresholds},
    disposition::{
        DispositionRecord, DispositionSeq, ExplicitApproval, UserDecision, releases_the_payload,
    },
    error::{DispositionState, WorkflowError},
    proposed::{Approved, Autosaved, ProposalId, Proposed},
    tier::{RiskTier, Workflow},
};

/// One queued proposal and everything the queue knows about it.
///
/// `payload` becomes `None` once the value has left through one of the three
/// release sites. The entry itself never goes away: a rejected proposal keeps
/// its payload and its whole history, which is what
/// `rejected_proposal_is_retained` observes.
#[derive(Debug)]
struct Entry<T> {
    tier: RiskTier,
    payload: Option<Proposed<T>>,
    committed: bool,
}

/// A proposal queue with a per-proposal append-only disposition history.
///
/// The history is one list in acceptance order, not a per-entry list, so the
/// order two decisions were made in is recorded rather than reconstructed. It
/// is append-only: [`ReviewQueue::undo`] pushes a record that names the one it
/// reverses and removes nothing.
#[derive(Debug)]
pub struct ReviewQueue<T> {
    entries: BTreeMap<ProposalId, Entry<T>>,
    history: Vec<DispositionRecord>,
    next_seq: DispositionSeq,
}

impl<T> Default for ReviewQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ReviewQueue<T> {
    /// An empty queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            history: Vec::new(),
            next_seq: DispositionSeq::FIRST,
        }
    }

    /// Puts a proposal in the queue.
    ///
    /// Admission is not a disposition and records nothing: what a proposal
    /// needs before it becomes a record is decided by its tier, at one of the
    /// four doors below.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::DuplicateProposal`] when the identifier is in use.
    pub fn admit(&mut self, proposed: Proposed<T>) -> Result<(), WorkflowError> {
        let id = proposed.id();
        if self.entries.contains_key(&id) {
            return Err(WorkflowError::DuplicateProposal(id));
        }
        self.entries.insert(
            id,
            Entry {
                tier: proposed.tier(),
                payload: Some(proposed),
                committed: false,
            },
        );
        Ok(())
    }

    /// How many proposals the queue holds, settled ones included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the queue holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The identifiers the queue holds, in ascending order.
    #[must_use]
    pub fn identifiers(&self) -> Vec<ProposalId> {
        self.entries.keys().copied().collect()
    }

    /// The tier a queued proposal was classified into.
    #[must_use]
    pub fn tier_of(&self, id: ProposalId) -> Option<RiskTier> {
        self.entries.get(&id).map(|entry| entry.tier)
    }

    /// The whole disposition history, in the order the decisions were made.
    ///
    /// Nothing is ever removed from it, so a rejection and its later undo are
    /// both here and the pair is the record of what happened.
    #[must_use]
    pub fn history(&self) -> &[DispositionRecord] {
        &self.history
    }

    /// One proposal's history, in the order the decisions were made.
    #[must_use]
    pub fn history_of(&self, id: ProposalId) -> Vec<&DispositionRecord> {
        self.history
            .iter()
            .filter(|record| record.proposal_id() == id)
            .collect()
    }

    /// What a proposal's history currently says.
    ///
    /// The last record that is not itself superseded by a later undo. An undo
    /// records the reversal and leaves the entry undisposed again.
    #[must_use]
    pub fn state_of(&self, id: ProposalId) -> DispositionState {
        let mut state = DispositionState::Undisposed;
        for record in self.history_of(id) {
            state = if record.supersedes().is_some() {
                DispositionState::Undisposed
            } else {
                DispositionState::Recorded(record.disposition().clone())
            };
        }
        state
    }

    /// Whether the proposal's payload has left the queue.
    #[must_use]
    pub fn is_committed(&self, id: ProposalId) -> bool {
        self.entries.get(&id).is_some_and(|entry| entry.committed)
    }

    /// The proposals no disposition currently applies to.
    ///
    /// Pending is a state of the queue, not a decision. An entry is pending
    /// when its history carries no record, or when the last record was undone,
    /// and it stops being pending the moment a user records something -- which
    /// is why there is no disposition that means "not yet".
    #[must_use]
    pub fn pending(&self) -> Vec<ProposalId> {
        self.entries
            .keys()
            .copied()
            .filter(|id| self.state_of(*id) == DispositionState::Undisposed)
            .filter(|id| !self.is_committed(*id))
            .collect()
    }

    /// The one place a tier is compared against the workflow a caller reached
    /// for.
    ///
    /// Every door below calls it with the workflow it serves, so the four rows
    /// of section 27.4 are enforced by one comparison rather than by four
    /// hand-written conditions that could drift apart.
    fn require(&self, id: ProposalId, attempted: Workflow) -> Result<&Entry<T>, WorkflowError> {
        let entry = self
            .entries
            .get(&id)
            .ok_or(WorkflowError::NoSuchProposal(id))?;
        let required = entry.tier.workflow();
        if required == attempted {
            Ok(entry)
        } else {
            Err(WorkflowError::WrongWorkflow {
                tier: entry.tier,
                required,
                attempted,
            })
        }
    }

    // -- Workflow::AutosaveAsAiInferred ------------------------------------

    /// Saves a low-risk proposal without a human.
    ///
    /// The one door that takes no [`UserDecision`], and the only tier it
    /// serves is `LOW_AUTOSAVE`. What comes back is an [`Autosaved`], whose
    /// epistemic status is a constant, so section 27.4's "save it but mark it
    /// `AI_INFERRED`" is what the type says rather than what a later layer
    /// remembers to do.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::WrongWorkflow`] for any other tier,
    /// [`WorkflowError::NoSuchProposal`] for an unknown identifier, and
    /// [`WorkflowError::AlreadyCommitted`] for a payload that has left.
    pub fn autosave(&mut self, id: ProposalId) -> Result<Autosaved<T>, WorkflowError> {
        self.require(id, Workflow::AutosaveAsAiInferred)?;
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(WorkflowError::NoSuchProposal(id))?;
        let payload = entry
            .payload
            .take()
            .ok_or(WorkflowError::AlreadyCommitted(id))?;
        entry.committed = true;
        // Release site 1 of 3. A low-risk proposal is saved without a human by
        // section 27.4's own rule, and the value it becomes is `AI_INFERRED`
        // and can be nothing else.
        Ok(Autosaved::new(id, payload.release()))
    }

    // -- Workflow::QueueAndUndo -------------------------------------------

    /// Records a user's disposition of a medium-risk proposal.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::WrongWorkflow`] for any other tier, and
    /// [`WorkflowError::AlreadyCommitted`] once the payload has left.
    pub fn review(
        &mut self,
        id: ProposalId,
        disposition: DecisionAction,
        decision: &UserDecision,
        at: u64,
    ) -> Result<&DispositionRecord, WorkflowError> {
        self.require(id, Workflow::QueueAndUndo)?;
        self.record(id, disposition, decision, at, None)
    }

    // -- Workflow::ExplicitApproval ---------------------------------------

    /// Approves a high-risk proposal against an approval that names it.
    ///
    /// Section 27.4's high-risk row needs an explicit approval, so this door
    /// takes an [`ExplicitApproval`] rather than a bare decision and refuses
    /// one that names a different proposal. It is the only door the workflow
    /// comparison lets a `HIGH_APPROVAL` entry through, so there is no other
    /// route by which a `CONFIRM` reaches this tier: [`ReviewQueue::review`]
    /// and [`ReviewQueue::decide`] both refuse it with
    /// [`WorkflowError::WrongWorkflow`] before looking at anything else.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::WrongWorkflow`] for any other tier,
    /// [`WorkflowError::ApprovalNamesAnotherProposal`] for a mismatched
    /// approval, and [`WorkflowError::AlreadyCommitted`] for a payload that has
    /// left.
    pub fn approve(
        &mut self,
        id: ProposalId,
        approval: &ExplicitApproval,
        at: u64,
    ) -> Result<Approved<T>, WorkflowError> {
        self.require(id, Workflow::ExplicitApproval)?;
        if approval.proposal_id() != id {
            return Err(WorkflowError::ApprovalNamesAnotherProposal {
                named: approval.proposal_id(),
                target: id,
            });
        }
        self.record(id, DecisionAction::Confirm, approval.decision(), at, None)?;
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(WorkflowError::NoSuchProposal(id))?;
        let payload = entry
            .payload
            .take()
            .ok_or(WorkflowError::AlreadyCommitted(id))?;
        entry.committed = true;
        // Release site 2 of 3. The disposition above is already in the history
        // and the approval named this exact proposal, so what leaves is a value
        // a user approved by identity rather than in bulk.
        Ok(Approved::new(id, payload.release()))
    }

    // -- Workflow::UserOnly ------------------------------------------------

    /// Records the user's own decision on a non-delegable proposal.
    ///
    /// Section 27.4's fourth row is the user's alone. This door takes a
    /// [`UserDecision`], which only [`UserDecision::by`] issues and only for
    /// [`academic_domain::Actor::User`], and it is the only door the workflow
    /// comparison lets a `NON_DELEGABLE` entry through.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::WrongWorkflow`] for any other tier, and
    /// [`WorkflowError::AlreadyCommitted`] once the payload has left.
    pub fn decide(
        &mut self,
        id: ProposalId,
        disposition: DecisionAction,
        decision: &UserDecision,
        at: u64,
    ) -> Result<&DispositionRecord, WorkflowError> {
        self.require(id, Workflow::UserOnly)?;
        self.record(id, disposition, decision, at, None)
    }

    // -- Undo and commit, for every tier that has a human in it -------------

    /// Reverses the current disposition by appending a record that names it.
    ///
    /// Nothing is edited and nothing is removed, which is ADR-003's rule for
    /// every canonical correction. The entry returns to pending and both
    /// records stay in the history.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::NothingToUndo`] when no disposition currently applies,
    /// and [`WorkflowError::AlreadyCommitted`] once the payload has left --
    /// undoing a commit is not this queue's to offer, because the value is
    /// already somewhere else.
    pub fn undo(
        &mut self,
        id: ProposalId,
        decision: &UserDecision,
        at: u64,
    ) -> Result<&DispositionRecord, WorkflowError> {
        if self.is_committed(id) {
            return Err(WorkflowError::AlreadyCommitted(id));
        }
        let (target, reversed) = self
            .open_record(id)
            .ok_or(WorkflowError::NothingToUndo(id))?;
        self.record(id, reversed, decision, at, Some(target))
    }

    /// The record a further undo would reverse, if there is one.
    ///
    /// The same walk `state_of` performs, returning the record's identity as
    /// well as its disposition. An undo record carries the disposition it
    /// reverses and a `supersedes` naming it, so the pair reads as "this
    /// reverses the APPROVE at sequence four" rather than needing a fifth
    /// disposition that means nothing on its own.
    fn open_record(&self, id: ProposalId) -> Option<(DispositionSeq, DecisionAction)> {
        let mut open = None;
        for record in self.history_of(id) {
            open = if record.supersedes().is_some() {
                None
            } else {
                Some((record.seq(), record.disposition().clone()))
            };
        }
        open
    }

    /// Hands out the payload of a proposal a user has confirmed.
    ///
    /// The release path for the two tiers whose door records a disposition
    /// without releasing anything: `MEDIUM_REVIEW` and `NON_DELEGABLE`.
    /// `HIGH_APPROVAL` releases through [`ReviewQueue::approve`] instead, and
    /// `LOW_AUTOSAVE` through [`ReviewQueue::autosave`], so this refuses both.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::NotConfirmed`] when the current disposition is not
    /// `CONFIRM`, [`WorkflowError::AlreadyCommitted`] for a payload that has
    /// left, and [`WorkflowError::WrongWorkflow`] for the two tiers that
    /// release elsewhere.
    pub fn commit(&mut self, id: ProposalId) -> Result<Approved<T>, WorkflowError> {
        let entry = self
            .entries
            .get(&id)
            .ok_or(WorkflowError::NoSuchProposal(id))?;
        let required = entry.tier.workflow();
        if !matches!(required, Workflow::QueueAndUndo | Workflow::UserOnly) {
            return Err(WorkflowError::WrongWorkflow {
                tier: entry.tier,
                required,
                attempted: Workflow::QueueAndUndo,
            });
        }
        let current = self.state_of(id);
        let confirmed = matches!(
            &current,
            DispositionState::Recorded(action) if releases_the_payload(action)
        );
        if !confirmed {
            return Err(WorkflowError::NotConfirmed {
                proposal: id,
                current,
            });
        }
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(WorkflowError::NoSuchProposal(id))?;
        let payload = entry
            .payload
            .take()
            .ok_or(WorkflowError::AlreadyCommitted(id))?;
        entry.committed = true;
        // Release site 3 of 3. The history already carries a user CONFIRM for
        // this exact proposal, checked immediately above, so what leaves is a
        // value a user decided on rather than one a model wrote.
        Ok(Approved::new(id, payload.release()))
    }

    /// Appends one record and returns it.
    fn record(
        &mut self,
        id: ProposalId,
        disposition: DecisionAction,
        decision: &UserDecision,
        at: u64,
        supersedes: Option<DispositionSeq>,
    ) -> Result<&DispositionRecord, WorkflowError> {
        let committed = self
            .entries
            .get(&id)
            .ok_or(WorkflowError::NoSuchProposal(id))?
            .committed;
        if committed {
            return Err(WorkflowError::AlreadyCommitted(id));
        }
        let seq = self.next_seq;
        self.next_seq = seq.next();
        self.history.push(DispositionRecord::new(
            seq,
            id,
            disposition,
            decision,
            at,
            supersedes,
        ));
        self.history.last().ok_or(WorkflowError::NoSuchProposal(id))
    }

    // -- Batching ----------------------------------------------------------

    /// Groups every pending proposal into batches under one configuration.
    ///
    /// The result is a partition of [`ReviewQueue::pending`]: every pending
    /// identifier appears in exactly one batch, and no batch holds anything
    /// that is not pending. `high_volume_proposals_are_batched_without_loss`
    /// asserts that as set equality in both directions rather than as a count.
    ///
    /// A `NON_DELEGABLE` proposal gets a batch of its own. Grouping one with
    /// anything else is the bulk-approval shortcut section 27.2 refuses, and a
    /// singleton batch is how it stays in the partition without being grouped.
    #[must_use]
    pub fn batches(&self, thresholds: &BatchingThresholds) -> Vec<Batch> {
        let mut grouped: BTreeMap<BatchKey, Vec<ProposalId>> = BTreeMap::new();
        for id in self.pending() {
            let Some(entry) = self.entries.get(&id) else {
                continue;
            };
            let Some(payload) = entry.payload.as_ref() else {
                continue;
            };
            let key = BatchKey {
                thresholds_version: thresholds.version(),
                tier: entry.tier,
                confidence_band: thresholds.confidence_band(payload.confidence()),
                impact_band: thresholds.impact_band(payload.impact()),
                singleton: (entry.tier == RiskTier::NonDelegable).then_some(id),
            };
            grouped.entry(key).or_default().push(id);
        }
        grouped
            .into_iter()
            .map(|(key, members)| Batch::new(key, members))
            .collect()
    }
}
