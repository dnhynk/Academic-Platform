//! `P2-M2`'s named acceptance evidence.
//!
//! Six of the execution plan's seven named cases are here; the seventh,
//! `proposed_type_cannot_reach_canonical_writer`, is a compile failure and
//! lives in `tests/compile_fail.rs`.
//!
//! One of the six is renamed. The plan names
//! `four_dispositions_are_durable_and_audited`; there are three, and the test
//! below is `three_dispositions_are_durable_and_audited`. Section 3 of the
//! authoritative spec names approve, modify and reject and no fourth, and
//! ADR-003 froze exactly those three as `DecisionAction`. The rename is
//! recorded here, in `docs/contracts/proposal-review-queue.md`, and in the
//! pull request, so an audit looking for the plan's name finds where it went.

use std::collections::BTreeSet;

use academic_domain::{
    Actor, ClaimId, ConfidencePermille, DecisionAction, EntityId, EpistemicStatus,
};
use academic_proposal::{
    BatchingThresholds, DispositionRecord, DispositionState, ExplicitApproval, ImpactPermille,
    ProposalId, Proposed, ReviewQueue, RiskTier, UserDecision, Workflow, disposition_token,
};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A synthetic payload. Nothing here parses or fetches anything.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    subject: String,
}

impl Candidate {
    fn new(subject: &str) -> Self {
        Self {
            subject: subject.to_owned(),
        }
    }
}

/// A UUIDv7 built from a counter, so every identifier in this file is derived
/// rather than drawn from anywhere real.
fn synthetic_uuid(seed: u128) -> Result<Uuid, Box<dyn std::error::Error>> {
    let mut bytes = seed.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Uuid::from_bytes(bytes))
}

fn user(seed: u128) -> Result<Actor, Box<dyn std::error::Error>> {
    Ok(Actor::User {
        user_id: EntityId::try_from_uuid(synthetic_uuid(seed)?)?,
    })
}

fn claim(seed: u128) -> Result<ClaimId, Box<dyn std::error::Error>> {
    Ok(ClaimId::try_from_uuid(synthetic_uuid(seed)?)?)
}

fn decision() -> Result<UserDecision, Box<dyn std::error::Error>> {
    Ok(UserDecision::by(&user(0x11)?)?)
}

/// The three automatic actor variants of `academic-domain`.
///
/// Written as a `match` over the closed enum rather than as a literal list, so
/// a fifth variant fails to compile here until it is classified.
fn automatic_actors() -> Result<Vec<Actor>, Box<dyn std::error::Error>> {
    let candidates = [
        Actor::DeterministicEngine {
            name: "synthetic-engine".to_owned(),
            version: "1".to_owned(),
        },
        Actor::ModelRun {
            run_id: EntityId::try_from_uuid(synthetic_uuid(0x21)?)?,
        },
        Actor::Importer {
            name: "synthetic-importer".to_owned(),
            version: "1".to_owned(),
        },
        user(0x22)?,
    ];
    Ok(candidates
        .into_iter()
        .filter(|actor| match actor {
            Actor::User { .. } => false,
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                true
            }
        })
        .collect())
}

fn proposed(
    id: u64,
    tier: RiskTier,
    confidence: u16,
    impact: u16,
    subject: &str,
) -> Result<Proposed<Candidate>, Box<dyn std::error::Error>> {
    Ok(Proposed::new(
        ProposalId::new(id),
        tier,
        ConfidencePermille::new(confidence)?,
        ImpactPermille::new(impact)?,
        Candidate::new(subject),
    ))
}

fn queue_with(tier: RiskTier) -> Result<ReviewQueue<Candidate>, Box<dyn std::error::Error>> {
    let mut queue = ReviewQueue::new();
    queue.admit(proposed(1, tier, 900, 100, "b-plus-tree")?)?;
    Ok(queue)
}

// ---------------------------------------------------------------------------
// The tier-to-workflow mapping
// ---------------------------------------------------------------------------

/// Which door each workflow is, driven against one queued proposal.
///
/// A tier is admitted, then all four doors are tried. Only the door whose
/// workflow equals the tier's may succeed, so the four rows of section 27.4 are
/// tested exhaustively rather than one row at a time: swapping any two rows of
/// `RiskTier::workflow` moves two cells off the diagonal and fails here.
fn try_door(
    tier: RiskTier,
    door: Workflow,
) -> Result<Result<(), academic_proposal::WorkflowError>, Box<dyn std::error::Error>> {
    let mut queue = queue_with(tier)?;
    let id = ProposalId::new(1);
    let decided = decision()?;
    let outcome = match door {
        Workflow::AutosaveAsAiInferred => queue.autosave(id).map(|_| ()),
        Workflow::QueueAndUndo => queue
            .review(id, DecisionAction::Confirm, &decided, 10)
            .map(|_| ()),
        Workflow::ExplicitApproval => queue
            .approve(id, &ExplicitApproval::of(id, decided.clone()), 10)
            .map(|_| ()),
        Workflow::UserOnly => queue
            .decide(id, DecisionAction::Confirm, &decided, 10)
            .map(|_| ()),
    };
    Ok(outcome)
}

#[test]
fn every_tier_reaches_only_its_own_workflow() -> TestResult {
    let mut accepted: Vec<(RiskTier, Workflow)> = Vec::new();
    for tier in RiskTier::ALL {
        for door in Workflow::ALL {
            let outcome = try_door(tier, door)?;
            if outcome.is_ok() {
                accepted.push((tier, door));
            } else {
                // Every refusal is the workflow comparison, not an incidental
                // failure that would make an off-diagonal cell pass for the
                // wrong reason.
                assert!(
                    matches!(
                        outcome,
                        Err(academic_proposal::WorkflowError::WrongWorkflow { .. })
                    ),
                    "{tier} at the {door} door was refused for the wrong reason: {outcome:?}"
                );
            }
        }
    }
    assert_eq!(
        accepted,
        vec![
            (RiskTier::LowAutosave, Workflow::AutosaveAsAiInferred),
            (RiskTier::MediumReview, Workflow::QueueAndUndo),
            (RiskTier::HighApproval, Workflow::ExplicitApproval),
            (RiskTier::NonDelegable, Workflow::UserOnly),
        ],
        "the accepted cells are not exactly the four rows of section 27.4"
    );
    Ok(())
}

#[test]
fn the_mapping_is_a_bijection_onto_the_four_workflows() -> TestResult {
    let mapped: Vec<Workflow> = RiskTier::ALL.into_iter().map(RiskTier::workflow).collect();
    assert_eq!(
        mapped,
        Workflow::ALL.to_vec(),
        "the tier-to-workflow mapping is not onto the four workflows in order"
    );
    // Enumerated, not counted: each row is named against the section it comes
    // from, so a row that quietly changed its workflow fails by name.
    for (tier, workflow) in [
        (RiskTier::LowAutosave, Workflow::AutosaveAsAiInferred),
        (RiskTier::MediumReview, Workflow::QueueAndUndo),
        (RiskTier::HighApproval, Workflow::ExplicitApproval),
        (RiskTier::NonDelegable, Workflow::UserOnly),
    ] {
        assert_eq!(
            tier.workflow(),
            workflow,
            "{tier} maps to the wrong workflow"
        );
        assert_eq!(RiskTier::parse(tier.as_str()), Some(tier));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// low_risk_autosave_is_marked_ai_inferred
// ---------------------------------------------------------------------------

#[test]
fn low_risk_autosave_is_marked_ai_inferred() -> TestResult {
    let mut queue = queue_with(RiskTier::LowAutosave)?;
    let id = ProposalId::new(1);
    let saved = queue.autosave(id)?;

    assert_eq!(saved.id(), id);
    assert_eq!(saved.epistemic_status(), EpistemicStatus::AiInferred);
    // The status is a constant on the type rather than a field, so there is no
    // argument a caller could pass that would make it anything else. That is
    // the half a run-time assertion cannot reach.
    assert_eq!(
        <academic_proposal::Autosaved<Candidate>>::EPISTEMIC_STATUS,
        EpistemicStatus::AiInferred
    );
    assert_ne!(
        <academic_proposal::Autosaved<Candidate>>::EPISTEMIC_STATUS,
        <academic_proposal::Approved<Candidate>>::EPISTEMIC_STATUS,
        "autosave and approval must not produce the same epistemic status"
    );
    assert_eq!(saved.into_inner(), Candidate::new("b-plus-tree"));

    // Nothing human happened, so nothing is in the history. An autosave that
    // recorded a user decision would be manufacturing a user.
    assert!(queue.history().is_empty());
    assert_eq!(queue.state_of(id), DispositionState::Undisposed);

    // The other three tiers cannot reach this door.
    for tier in [
        RiskTier::MediumReview,
        RiskTier::HighApproval,
        RiskTier::NonDelegable,
    ] {
        let mut other = queue_with(tier)?;
        assert!(
            matches!(
                other.autosave(ProposalId::new(1)),
                Err(academic_proposal::WorkflowError::WrongWorkflow { .. })
            ),
            "{tier} reached the autosave door"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// medium_risk_requires_queue_and_undo
// ---------------------------------------------------------------------------

#[test]
fn medium_risk_requires_queue_and_undo() -> TestResult {
    let mut queue = queue_with(RiskTier::MediumReview)?;
    let id = ProposalId::new(1);
    let decided = decision()?;

    // Queue: it is pending until a user records something, and it cannot be
    // saved without one.
    assert_eq!(queue.pending(), vec![id]);
    assert!(matches!(
        queue.autosave(id),
        Err(academic_proposal::WorkflowError::WrongWorkflow { .. })
    ));
    assert!(matches!(
        queue.commit(id),
        Err(academic_proposal::WorkflowError::NotConfirmed { .. })
    ));

    let first = queue
        .review(id, DecisionAction::Reject, &decided, 10)?
        .seq();
    assert_eq!(
        queue.state_of(id),
        DispositionState::Recorded(DecisionAction::Reject)
    );
    assert!(queue.pending().is_empty());

    // Undo: reversible, and the reversal is an append rather than a deletion.
    let undone = queue.undo(id, &decided, 20)?;
    assert_eq!(undone.supersedes(), Some(first));
    assert_eq!(undone.disposition(), &DecisionAction::Reject);
    assert_eq!(queue.state_of(id), DispositionState::Undisposed);
    assert_eq!(
        queue.pending(),
        vec![id],
        "an undone entry returns to the queue"
    );
    assert_eq!(
        queue.history_of(id).len(),
        2,
        "the undo replaced the record instead of appending beside it"
    );

    // And the proposal is still usable afterwards.
    queue.review(id, DecisionAction::Confirm, &decided, 30)?;
    let approved = queue.commit(id)?;
    assert_eq!(approved.epistemic_status(), EpistemicStatus::UserConfirmed);
    assert_eq!(approved.into_inner(), Candidate::new("b-plus-tree"));
    assert_eq!(queue.history_of(id).len(), 3);

    // An undo with nothing open is refused rather than silently recorded.
    let mut fresh = queue_with(RiskTier::MediumReview)?;
    assert!(matches!(
        fresh.undo(ProposalId::new(1), &decided, 10),
        Err(academic_proposal::WorkflowError::NothingToUndo(_))
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// high_risk_requires_explicit_approval
// ---------------------------------------------------------------------------

#[test]
fn high_risk_requires_explicit_approval() -> TestResult {
    let id = ProposalId::new(1);
    let other = ProposalId::new(2);
    let decided = decision()?;

    // The ordinary review and user-only doors do not reach this tier at all,
    // so there is no way to record a confirmation for it without an approval.
    let mut queue = queue_with(RiskTier::HighApproval)?;
    for outcome in [
        queue
            .review(id, DecisionAction::Confirm, &decided, 10)
            .map(|_| ()),
        queue
            .decide(id, DecisionAction::Confirm, &decided, 10)
            .map(|_| ()),
        queue.autosave(id).map(|_| ()),
    ] {
        assert!(
            matches!(
                outcome,
                Err(academic_proposal::WorkflowError::WrongWorkflow { .. })
            ),
            "a high-risk proposal was settled without an explicit approval: {outcome:?}"
        );
    }
    assert!(queue.history().is_empty());

    // An approval that names a different proposal is refused: "explicit" means
    // it carries the identity of what it approves.
    assert!(matches!(
        queue.approve(id, &ExplicitApproval::of(other, decided.clone()), 10),
        Err(academic_proposal::WorkflowError::ApprovalNamesAnotherProposal { .. })
    ));
    assert!(
        queue.history().is_empty(),
        "a refused approval was recorded"
    );

    // The approval that does name it succeeds, records the confirmation, and
    // releases the payload.
    let approval = ExplicitApproval::of(id, decided.clone());
    let approved = queue.approve(id, &approval, 20)?;
    assert_eq!(approved.epistemic_status(), EpistemicStatus::UserConfirmed);
    assert_eq!(approved.into_inner(), Candidate::new("b-plus-tree"));
    assert_eq!(
        queue.state_of(id),
        DispositionState::Recorded(DecisionAction::Confirm)
    );
    assert!(queue.is_committed(id));

    // An approval needs a user decision, and a user decision needs a user.
    for actor in automatic_actors()? {
        assert!(
            matches!(
                UserDecision::by(&actor),
                Err(academic_proposal::WorkflowError::AutomaticActor { .. })
            ),
            "{} minted a user decision",
            actor.kind_name()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// non-delegable
// ---------------------------------------------------------------------------

#[test]
fn non_delegable_has_no_automatic_actor_path() -> TestResult {
    let id = ProposalId::new(1);
    let decided = decision()?;

    // Every public entry point that can settle a `NON_DELEGABLE` proposal, and
    // what stops an automatic actor reaching it. The list is compared against
    // the crate's whole public settlement surface by
    // `every_settlement_door_is_named` in `tests/proposal_scans.rs`, so a fifth
    // door added without a row here fails there.
    let mut queue = queue_with(RiskTier::NonDelegable)?;
    // `autosave` -- the one door that takes no user decision -- is refused by
    // the workflow comparison before it can look at anything else.
    assert!(matches!(
        queue.autosave(id),
        Err(academic_proposal::WorkflowError::WrongWorkflow { .. })
    ));
    // `review` and `approve` are other tiers' doors, refused the same way.
    assert!(matches!(
        queue.review(id, DecisionAction::Confirm, &decided, 10),
        Err(academic_proposal::WorkflowError::WrongWorkflow { .. })
    ));
    assert!(matches!(
        queue.approve(id, &ExplicitApproval::of(id, decided.clone()), 10),
        Err(academic_proposal::WorkflowError::WrongWorkflow { .. })
    ));
    // `decide` is this tier's own door and it takes a `UserDecision`, which
    // only a user actor can mint.
    for actor in automatic_actors()? {
        assert!(matches!(
            UserDecision::by(&actor),
            Err(academic_proposal::WorkflowError::AutomaticActor { .. })
        ));
    }
    // `commit` releases the payload, and it needs a recorded confirmation that
    // only `decide` can have put there.
    assert!(matches!(
        queue.commit(id),
        Err(academic_proposal::WorkflowError::NotConfirmed { .. })
    ));
    // `undo` needs something open, and cannot open one.
    assert!(matches!(
        queue.undo(id, &decided, 10),
        Err(academic_proposal::WorkflowError::NothingToUndo(_))
    ));

    // Nothing above recorded anything, so no automatic actor moved the entry.
    assert!(queue.history().is_empty());
    assert_eq!(queue.pending(), vec![id]);

    // The user can, which is what makes the refusals above attributable to the
    // actor rather than to the queue being inert.
    queue.decide(id, DecisionAction::Confirm, &decided, 30)?;
    let approved = queue.commit(id)?;
    assert_eq!(approved.epistemic_status(), EpistemicStatus::UserConfirmed);
    Ok(())
}

// ---------------------------------------------------------------------------
// three_dispositions_are_durable_and_audited
// (the plan calls this `four_dispositions_are_durable_and_audited`)
// ---------------------------------------------------------------------------

#[test]
fn three_dispositions_are_durable_and_audited() -> TestResult {
    let decided = decision()?;
    // Enumerated, one row per user-owned action section 3 names. Nothing here
    // asserts a count: the assertion below compares this list against the
    // frozen `DecisionAction` vocabulary in both directions, so a fourth arm
    // added to that enum fails as a missing row and a row removed here fails as
    // a missing token.
    let enumerated = [
        ("CONFIRM", DecisionAction::Confirm, "approve"),
        (
            "REPLACE",
            DecisionAction::Replace {
                replacement_claim_id: claim(0x31)?,
            },
            "modify",
        ),
        ("REJECT", DecisionAction::Reject, "reject"),
    ];

    let tokens: BTreeSet<&str> = enumerated.iter().map(|(token, _, _)| *token).collect();
    let frozen: BTreeSet<&str> = ["CONFIRM", "REPLACE", "REJECT"].into_iter().collect();
    assert_eq!(
        tokens, frozen,
        "the enumerated dispositions are not the frozen DecisionAction arms"
    );

    // The spelling is the frozen serde one, which
    // `the_disposition_tokens_are_the_frozen_serde_spellings` below establishes
    // against serde itself rather than against a list written twice.

    // Durable: each one is recorded, survives every later decision, and the
    // history keeps them in the order they were made.
    let mut queue: ReviewQueue<Candidate> = ReviewQueue::new();
    for (index, (_, action, _)) in enumerated.iter().enumerate() {
        let id = ProposalId::new(index as u64 + 1);
        queue.admit(Proposed::new(
            id,
            RiskTier::MediumReview,
            ConfidencePermille::new(500)?,
            ImpactPermille::new(500)?,
            Candidate::new("subject"),
        ))?;
        queue.review(id, action.clone(), &decided, 100 + index as u64)?;
    }
    let recorded: Vec<&DecisionAction> = queue
        .history()
        .iter()
        .map(DispositionRecord::disposition)
        .collect();
    let expected: Vec<&DecisionAction> = enumerated.iter().map(|(_, action, _)| action).collect();
    assert_eq!(recorded, expected, "the history is not what was recorded");

    // Audited: each record carries a digest over every field, and the digests
    // are distinct because the records are.
    let digests: BTreeSet<[u8; 32]> = queue
        .history()
        .iter()
        .map(|record| *record.record_digest())
        .collect();
    assert_eq!(
        digests.len(),
        queue.history().len(),
        "two disposition records share a digest"
    );
    for record in queue.history() {
        assert_eq!(record.user_id(), decided.user_id());
        assert!(record.decided_at() >= 100);
        assert_ne!(*record.record_digest(), [0_u8; 32]);
    }
    Ok(())
}

#[test]
fn the_disposition_tokens_are_the_frozen_serde_spellings() -> TestResult {
    // `DecisionAction` carries `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`,
    // so the three tokens this crate stores are that enum's own spellings and
    // not a second vocabulary beside them. Comparing against a literal list
    // would restate them; comparing against what serde emits is what makes the
    // reuse checkable, and it is what fails the day the wire spelling moves.
    for (action, spec_word) in [
        (DecisionAction::Confirm, "approve"),
        (
            DecisionAction::Replace {
                replacement_claim_id: claim(0x61)?,
            },
            "modify",
        ),
        (DecisionAction::Reject, "reject"),
    ] {
        let encoded = serde_json::to_value(&action)?;
        let spelled = encoded
            .get("action")
            .and_then(serde_json::Value::as_str)
            .ok_or("DecisionAction no longer serializes with an `action` tag")?;
        assert_eq!(
            disposition_token(&action),
            spelled,
            "the token for {spec_word} is not what serde spells"
        );
    }
    Ok(())
}

#[test]
fn the_disposition_digest_covers_every_field() -> TestResult {
    let decided = decision()?;
    let other = UserDecision::by(&user(0x99)?)?;

    // The baseline, and one variant per field of `DispositionRecord` that a
    // caller can move. Each variant must produce a different digest; a field
    // left out of the hash makes its row equal the baseline and fails.
    let baseline = record(ProposalId::new(1), DecisionAction::Confirm, &decided, 10)?;
    let variants = [
        (
            "proposal_id",
            record(ProposalId::new(2), DecisionAction::Confirm, &decided, 10)?,
        ),
        (
            "disposition",
            record(ProposalId::new(1), DecisionAction::Reject, &decided, 10)?,
        ),
        (
            "replacement_claim_id",
            record(
                ProposalId::new(1),
                DecisionAction::Replace {
                    replacement_claim_id: claim(0x41)?,
                },
                &decided,
                10,
            )?,
        ),
        (
            "replacement_claim_id (a second claim)",
            record(
                ProposalId::new(1),
                DecisionAction::Replace {
                    replacement_claim_id: claim(0x42)?,
                },
                &decided,
                10,
            )?,
        ),
        (
            "user_id",
            record(ProposalId::new(1), DecisionAction::Confirm, &other, 10)?,
        ),
        (
            "decided_at",
            record(ProposalId::new(1), DecisionAction::Confirm, &decided, 11)?,
        ),
    ];
    let mut seen: BTreeSet<[u8; 32]> = BTreeSet::new();
    seen.insert(baseline);
    for (field, digest) in variants {
        assert!(
            seen.insert(digest),
            "changing {field} did not change the record digest"
        );
    }

    // `seq` and `supersedes` move together with an undo, which is the only way
    // a caller reaches them.
    let mut queue = queue_with(RiskTier::MediumReview)?;
    let id = ProposalId::new(1);
    let first = *queue
        .review(id, DecisionAction::Confirm, &decided, 10)?
        .record_digest();
    let second = *queue.undo(id, &decided, 10)?.record_digest();
    assert_ne!(
        first, second,
        "an undo at the same timestamp with the same disposition produced the same digest"
    );
    Ok(())
}

/// The digest of a record made under a fresh queue, for the field sweep above.
fn record(
    id: ProposalId,
    action: DecisionAction,
    decided: &UserDecision,
    at: u64,
) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    let mut queue: ReviewQueue<Candidate> = ReviewQueue::new();
    queue.admit(Proposed::new(
        id,
        RiskTier::MediumReview,
        ConfidencePermille::new(500)?,
        ImpactPermille::new(500)?,
        Candidate::new("subject"),
    ))?;
    Ok(*queue.review(id, action, decided, at)?.record_digest())
}

// ---------------------------------------------------------------------------
// rejected_proposal_is_retained
// ---------------------------------------------------------------------------

#[test]
fn rejected_proposal_is_retained() -> TestResult {
    let mut queue = queue_with(RiskTier::MediumReview)?;
    let id = ProposalId::new(1);
    let decided = decision()?;

    queue.review(id, DecisionAction::Reject, &decided, 10)?;

    // The entry is still there. A rejection is a decision recorded against a
    // proposal, not the removal of one -- ADR-003's append-only rule, and the
    // reason the queue has no method that removes an entry at all.
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.identifiers(), vec![id]);
    assert_eq!(queue.tier_of(id), Some(RiskTier::MediumReview));
    assert_eq!(
        queue.state_of(id),
        DispositionState::Recorded(DecisionAction::Reject)
    );
    assert!(!queue.is_committed(id), "a rejection released the payload");

    // And so is the record of the rejection, after later decisions.
    queue.undo(id, &decided, 20)?;
    queue.review(id, DecisionAction::Confirm, &decided, 30)?;
    let history: Vec<&DecisionAction> = queue
        .history_of(id)
        .into_iter()
        .map(DispositionRecord::disposition)
        .collect();
    assert_eq!(
        history,
        vec![
            &DecisionAction::Reject,
            &DecisionAction::Reject,
            &DecisionAction::Confirm
        ],
        "the rejection did not survive the decisions that followed it"
    );
    assert_eq!(queue.history_of(id)[0].supersedes(), None);
    assert_eq!(
        queue.history_of(id)[1].supersedes(),
        Some(queue.history_of(id)[0].seq()),
        "the undo does not name the rejection it reverses"
    );

    // The payload is still in the queue too: a rejection retains what was
    // proposed, so a later reader can see what was refused.
    queue.undo(id, &decided, 40)?;
    queue.review(id, DecisionAction::Confirm, &decided, 50)?;
    assert_eq!(
        queue.commit(id)?.into_inner(),
        Candidate::new("b-plus-tree")
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// high_volume_proposals_are_batched_without_loss
// ---------------------------------------------------------------------------

/// A batcher with a defect, run beside the product one.
///
/// It keys a map on the band pair and *overwrites* rather than appending --
/// the shape a batcher acquires when the grouping value is treated as one
/// member instead of a list. It is here so the assertion below is observed
/// failing on an implementation that loses items, rather than passing over one
/// that cannot.
fn lossy_batches(
    queue: &ReviewQueue<Candidate>,
    thresholds: &BatchingThresholds,
) -> Vec<ProposalId> {
    use std::collections::BTreeMap;
    let mut grouped: BTreeMap<(u32, u8, u8), ProposalId> = BTreeMap::new();
    for batch in queue.batches(thresholds) {
        for member in batch.members() {
            grouped.insert(
                (
                    batch.key().thresholds_version,
                    batch.key().confidence_band,
                    batch.key().impact_band,
                ),
                *member,
            );
        }
    }
    grouped.into_values().collect()
}

#[test]
fn high_volume_proposals_are_batched_without_loss() -> TestResult {
    let thresholds = BatchingThresholds::shipped();
    let mut queue: ReviewQueue<Candidate> = ReviewQueue::new();
    let mut admitted: BTreeSet<ProposalId> = BTreeSet::new();

    // Four hundred proposals spread over every tier and both band axes,
    // including values exactly on each cut so a boundary that fell through
    // would leave a hole.
    for index in 0..400_u64 {
        let id = ProposalId::new(index + 1);
        let tier = RiskTier::ALL[(index % 4) as usize];
        let confidence = [0, 499, 500, 799, 800, 1000][(index % 6) as usize];
        let impact = [0, 299, 300, 699, 700, 1000][(index % 6) as usize];
        queue.admit(Proposed::new(
            id,
            tier,
            ConfidencePermille::new(confidence)?,
            ImpactPermille::new(impact)?,
            Candidate::new("subject"),
        ))?;
        admitted.insert(id);
    }

    let batches = queue.batches(&thresholds);
    let mut batched: Vec<ProposalId> = Vec::new();
    for batch in &batches {
        assert!(!batch.members().is_empty(), "an empty batch was emitted");
        assert_eq!(batch.key().thresholds_version, thresholds.version());
        batched.extend_from_slice(batch.members());
    }
    let batched_set: BTreeSet<ProposalId> = batched.iter().copied().collect();

    // Set equality in both directions, and no duplication. A count would pass
    // an implementation that dropped one item and emitted another twice.
    assert_eq!(
        batched_set, admitted,
        "the batches are not the set of pending proposals"
    );
    assert_eq!(
        batched.len(),
        batched_set.len(),
        "a proposal appeared in more than one batch"
    );
    assert_eq!(queue.pending().len(), admitted.len());

    // Every member of a batch really does share the batch key.
    for batch in &batches {
        for member in batch.members() {
            assert_eq!(queue.tier_of(*member), Some(batch.key().tier));
        }
        if batch.key().tier == RiskTier::NonDelegable {
            assert_eq!(
                batch.members().len(),
                1,
                "a non-delegable proposal was grouped with another"
            );
            assert_eq!(batch.key().singleton, Some(batch.members()[0]));
        } else {
            assert_eq!(batch.key().singleton, None);
        }
    }

    // The injection: the same assertion, run against a batcher that loses
    // items, fails. Without this the equality above would pass over any
    // implementation, including one that could not lose anything.
    let lossy: BTreeSet<ProposalId> = lossy_batches(&queue, &thresholds).into_iter().collect();
    assert_ne!(
        lossy, admitted,
        "the lossy control did not lose anything, so the set equality above proves nothing"
    );
    assert!(lossy.len() < admitted.len());

    // Settling a proposal takes it out of the pending set, and the partition
    // follows: the remaining batches are exactly the rest.
    let decided = decision()?;
    let settled = ProposalId::new(2);
    assert_eq!(queue.tier_of(settled), Some(RiskTier::MediumReview));
    queue.review(settled, DecisionAction::Reject, &decided, 10)?;
    let after: BTreeSet<ProposalId> = queue
        .batches(&thresholds)
        .iter()
        .flat_map(|batch| batch.members().to_vec())
        .collect();
    let mut expected = admitted.clone();
    expected.remove(&settled);
    assert_eq!(after, expected);
    Ok(())
}

#[test]
fn the_batching_thresholds_are_versioned_configuration() -> TestResult {
    let shipped = BatchingThresholds::shipped();
    assert_eq!(shipped.version(), BatchingThresholds::SHIPPED_VERSION);
    assert_eq!(shipped.confidence_cuts(), [500, 800]);
    assert_eq!(shipped.impact_cuts(), [300, 700]);

    // A configuration whose contents change produces a different digest, so
    // the version is checkable against the contents rather than a label.
    let moved = BatchingThresholds::new(shipped.version(), vec![400, 800], vec![300, 700])?;
    assert_eq!(moved.version(), shipped.version());
    assert_ne!(
        moved.digest(),
        shipped.digest(),
        "two different configurations under one version share a digest"
    );
    let renumbered = BatchingThresholds::new(2, vec![500, 800], vec![300, 700])?;
    assert_ne!(renumbered.digest(), shipped.digest());

    // A batch computed under one version is not a batch computed under
    // another, because the key carries the version.
    let mut queue = queue_with(RiskTier::MediumReview)?;
    queue.admit(proposed(2, RiskTier::MediumReview, 900, 100, "other")?)?;
    let under_shipped = queue.batches(&shipped);
    let under_renumbered = queue.batches(&renumbered);
    assert_eq!(under_shipped.len(), under_renumbered.len());
    assert_ne!(
        under_shipped[0].key(),
        under_renumbered[0].key(),
        "the batch key does not carry the configuration version"
    );

    // A configuration that does not divide is refused rather than silently
    // producing one band.
    for (cuts, impact) in [
        (Vec::new(), vec![300]),
        (vec![300], Vec::new()),
        (vec![0], vec![300]),
        (vec![1001], vec![300]),
        (vec![500, 500], vec![300]),
        (vec![800, 500], vec![300]),
    ] {
        assert!(
            BatchingThresholds::new(1, cuts.clone(), impact.clone()).is_err(),
            "a configuration with confidence {cuts:?} and impact {impact:?} was accepted"
        );
    }
    Ok(())
}

#[test]
fn a_band_edge_belongs_to_the_band_above_it() -> TestResult {
    let thresholds = BatchingThresholds::shipped();
    for (value, band) in [(0, 0), (499, 0), (500, 1), (799, 1), (800, 2), (1000, 2)] {
        assert_eq!(
            thresholds.confidence_band(ConfidencePermille::new(value)?),
            band,
            "confidence {value} is not in band {band}"
        );
    }
    for (value, band) in [(0, 0), (299, 0), (300, 1), (699, 1), (700, 2), (1000, 2)] {
        assert_eq!(
            thresholds.impact_band(ImpactPermille::new(value)?),
            band,
            "impact {value} is not in band {band}"
        );
    }
    assert!(ImpactPermille::new(1001).is_err());
    Ok(())
}

// ---------------------------------------------------------------------------
// The boundary itself
// ---------------------------------------------------------------------------

#[test]
fn the_proposed_wrapper_prints_no_payload() -> TestResult {
    let secret = "SYNTHETIC-CANARY-1c9a4f";
    let wrapped = proposed(1, RiskTier::MediumReview, 900, 100, secret)?;
    let printed = format!("{wrapped:?}");
    assert!(
        !printed.contains(secret),
        "the Debug of a Proposed printed its payload: {printed}"
    );
    assert!(printed.contains("MediumReview"));
    assert!(printed.contains("900"));
    assert!(printed.contains("100"));
    Ok(())
}

#[test]
fn a_committed_payload_does_not_come_out_twice() -> TestResult {
    let mut queue = queue_with(RiskTier::LowAutosave)?;
    let id = ProposalId::new(1);
    queue.autosave(id)?;
    assert!(matches!(
        queue.autosave(id),
        Err(academic_proposal::WorkflowError::AlreadyCommitted(_))
    ));
    assert!(matches!(
        queue.undo(id, &decision()?, 10),
        Err(academic_proposal::WorkflowError::AlreadyCommitted(_))
    ));

    let mut medium = queue_with(RiskTier::MediumReview)?;
    let decided = decision()?;
    medium.review(id, DecisionAction::Confirm, &decided, 10)?;
    medium.commit(id)?;
    assert!(matches!(
        medium.commit(id),
        Err(academic_proposal::WorkflowError::AlreadyCommitted(_))
    ));
    assert!(matches!(
        medium.review(id, DecisionAction::Reject, &decided, 20),
        Err(academic_proposal::WorkflowError::AlreadyCommitted(_))
    ));
    Ok(())
}

#[test]
fn a_replacement_does_not_release_the_proposal_payload() -> TestResult {
    // ADR-003 has a replacement reject the target and select a different
    // object, so the model's own candidate is not what becomes the record.
    let mut queue = queue_with(RiskTier::MediumReview)?;
    let id = ProposalId::new(1);
    let decided = decision()?;
    let action = DecisionAction::Replace {
        replacement_claim_id: claim(0x51)?,
    };
    queue.review(id, action.clone(), &decided, 10)?;
    assert_eq!(queue.state_of(id), DispositionState::Recorded(action));
    assert!(matches!(
        queue.commit(id),
        Err(academic_proposal::WorkflowError::NotConfirmed { .. })
    ));
    assert!(!queue.is_committed(id));
    Ok(())
}

#[test]
fn an_identifier_is_admitted_once() -> TestResult {
    let mut queue = queue_with(RiskTier::MediumReview)?;
    assert!(matches!(
        queue.admit(proposed(1, RiskTier::LowAutosave, 100, 100, "collision")?),
        Err(academic_proposal::WorkflowError::DuplicateProposal(_))
    ));
    assert_eq!(
        queue.tier_of(ProposalId::new(1)),
        Some(RiskTier::MediumReview)
    );
    assert!(matches!(
        queue.autosave(ProposalId::new(7)),
        Err(academic_proposal::WorkflowError::NoSuchProposal(_))
    ));
    Ok(())
}
