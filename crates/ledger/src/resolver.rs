//! Pure bitemporal resolution over an owned query snapshot.
//!
//! The SQL store and the in-memory ledger both project their rows into these
//! records. Resolution therefore has one authority/decision implementation and
//! does not depend on a particular persistence engine.

use std::collections::BTreeSet;

use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, ClaimRelation, ClaimRelationKind,
    DecisionAction, EntityId, EpistemicStatus, FreshnessBand, MasteryLevel, PredicateId, ScopeId,
    TimestampMillis, UserDecision,
};
use serde::{Deserialize, Serialize};

/// Predicate-specific authority policy used instead of arrival-time LWW.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthorityPolicy {
    UserOwned,
    OfficialFact,
    ImplementationObservation,
    CuratedRelation,
}

/// Stable actor category retained by normalized SQL relation rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolverActorKind {
    User,
    DeterministicEngine,
    ModelRun,
    Importer,
}

impl From<&Actor> for ResolverActorKind {
    fn from(actor: &Actor) -> Self {
        match actor {
            Actor::User { .. } => Self::User,
            Actor::DeterministicEngine { .. } => Self::DeterministicEngine,
            Actor::ModelRun { .. } => Self::ModelRun,
            Actor::Importer { .. } => Self::Importer,
        }
    }
}

/// One accepted claim and its replica-local known-time coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionClaim {
    pub claim: Claim,
    pub accept_seq: u64,
}

/// One accepted claim relation and the author category that controls lifecycle effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionRelation {
    pub relation: ClaimRelation,
    pub accept_seq: u64,
    pub actor_kind: ResolverActorKind,
}

/// One accepted user decision and its replica-local known-time coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionDecision {
    pub decision: UserDecision,
    pub accept_seq: u64,
}

/// Coordinates for a bitemporal claim query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionQuery {
    pub subject_entity_id: EntityId,
    pub scope_id: ScopeId,
    pub predicate_id: PredicateId,
    pub valid_at: TimestampMillis,
    pub known_at_accept_seq: u64,
    pub policy: AuthorityPolicy,
}

/// Stable resolution result with lower-authority conflicts still visible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionResult {
    pub active_claim_ids: Vec<ClaimId>,
    pub conflicting_claim_ids: Vec<ClaimId>,
    pub rejected_claim_ids: Vec<ClaimId>,
}

/// Knowledge projection that cannot decay mastery when freshness changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeStateView {
    pub mastery: Option<MasteryLevel>,
    pub freshness: Option<FreshnessBand>,
    pub mastery_resolution: ResolutionResult,
    pub freshness_resolution: ResolutionResult,
}

/// Resolves one query from immutable claim/relation/decision snapshots.
#[must_use]
pub fn resolve_snapshot(
    query: &ResolutionQuery,
    claims: &[ResolutionClaim],
    relations: &[ResolutionRelation],
    decisions: &[ResolutionDecision],
) -> ResolutionResult {
    let mut candidates: Vec<(&Claim, u64)> = claims
        .iter()
        .filter_map(|record| {
            let claim = &record.claim;
            (record.accept_seq <= query.known_at_accept_seq
                && claim.validate().is_ok()
                && claim.subject_entity_id == query.subject_entity_id
                && claim.predicate_id == query.predicate_id
                && claim.scope_id == query.scope_id
                && claim.valid_time.contains(query.valid_at))
            .then_some((claim, record.accept_seq))
        })
        .collect();
    candidates.sort_by_key(|(claim, accepted)| (*accepted, claim.id));

    let candidate_ids: BTreeSet<ClaimId> = candidates.iter().map(|(claim, _)| claim.id).collect();
    let mut applicable_decisions: Vec<(&UserDecision, u64)> = decisions
        .iter()
        .filter_map(|record| {
            let decision = &record.decision;
            (record.accept_seq <= query.known_at_accept_seq
                && decision.resolution_slot.subject_entity_id == query.subject_entity_id
                && decision.resolution_slot.predicate_id == query.predicate_id
                && decision.resolution_slot.scope_id == query.scope_id
                && decision.valid_time.contains(query.valid_at))
            .then_some((decision, record.accept_seq))
        })
        .collect();
    applicable_decisions.sort_by_key(|(decision, accept_seq)| (*accept_seq, decision.id));
    let has_user_override = !applicable_decisions.is_empty();
    let mut rejected_objects: Vec<ClaimObject> = Vec::new();
    let mut chosen_object: Option<ClaimObject> = None;
    for (decision, _) in applicable_decisions {
        match &decision.action {
            DecisionAction::Confirm => {
                rejected_objects.retain(|object| object != &decision.target_object);
                chosen_object = Some(decision.target_object.clone());
            }
            DecisionAction::Reject => {
                if !rejected_objects.contains(&decision.target_object) {
                    rejected_objects.push(decision.target_object.clone());
                }
                if chosen_object.as_ref() == Some(&decision.target_object) {
                    chosen_object = None;
                }
            }
            DecisionAction::Replace {
                replacement_claim_id,
            } => {
                if !rejected_objects.contains(&decision.target_object) {
                    rejected_objects.push(decision.target_object.clone());
                }
                if let Some(replacement) = claims
                    .iter()
                    .find(|record| record.claim.id == *replacement_claim_id)
                    .map(|record| &record.claim)
                {
                    rejected_objects.retain(|object| object != &replacement.object);
                    chosen_object = Some(replacement.object.clone());
                }
            }
        }
    }

    let mut rejected: BTreeSet<ClaimId> = candidates
        .iter()
        .filter(|(claim, _)| claim.epistemic_status == EpistemicStatus::Superseded)
        .map(|(claim, _)| claim.id)
        .collect();
    let lifecycle_conflicts: BTreeSet<ClaimId> = candidates
        .iter()
        .filter(|(claim, _)| claim.epistemic_status == EpistemicStatus::Disputed)
        .map(|(claim, _)| claim.id)
        .collect();

    for record in relations.iter().filter(|record| {
        record.accept_seq <= query.known_at_accept_seq
            && record.relation.scope_id == query.scope_id
            && candidate_ids.contains(&record.relation.source_claim_id)
            && candidate_ids.contains(&record.relation.target_claim_id)
            && matches!(
                record.relation.kind,
                ClaimRelationKind::Supersedes | ClaimRelationKind::Retracts
            )
    }) {
        let Some(source) = claims
            .iter()
            .find(|claim| claim.claim.id == record.relation.source_claim_id)
            .map(|claim| &claim.claim)
        else {
            continue;
        };
        let Some(target) = claims
            .iter()
            .find(|claim| claim.claim.id == record.relation.target_claim_id)
            .map(|claim| &claim.claim)
        else {
            continue;
        };
        if chosen_object
            .as_ref()
            .is_some_and(|object| object == &target.object)
        {
            continue;
        }
        if relation_effect_is_authorized_for_kind(
            record.actor_kind,
            record.relation.kind,
            source,
            target,
        ) {
            rejected.insert(target.id);
        }
    }

    rejected.extend(
        candidates
            .iter()
            .filter(|(claim, _)| rejected_objects.contains(&claim.object))
            .map(|(claim, _)| claim.id),
    );

    let user_decision_rank =
        has_user_override.then(|| authority_rank(query.policy, AuthorityClass::UserExplicit));
    let eligible: Vec<(&Claim, u64, u16)> = candidates
        .into_iter()
        .filter(|(claim, _)| {
            !rejected.contains(&claim.id)
                && !matches!(
                    claim.epistemic_status,
                    EpistemicStatus::Disputed | EpistemicStatus::Superseded
                )
        })
        .map(|(claim, accepted)| {
            let original_rank = authority_rank(query.policy, claim.authority_class);
            let effective_rank = if chosen_object.as_ref() == Some(&claim.object) {
                user_decision_rank.map_or(original_rank, |decision_rank| {
                    original_rank.max(decision_rank)
                })
            } else {
                original_rank
            };
            (claim, accepted, effective_rank)
        })
        .collect();

    let activation_candidates: Vec<&(&Claim, u64, u16)> = eligible
        .iter()
        .filter(|(_, _, rank)| user_decision_rank.is_none_or(|minimum| *rank >= minimum))
        .collect();
    let Some(max_rank) = activation_candidates.iter().map(|(_, _, rank)| *rank).max() else {
        let mut conflicting_claim_ids = lifecycle_conflicts;
        conflicting_claim_ids.extend(eligible.iter().map(|(claim, _, _)| claim.id));
        return ResolutionResult {
            active_claim_ids: Vec::new(),
            conflicting_claim_ids: conflicting_claim_ids.into_iter().collect(),
            rejected_claim_ids: rejected.into_iter().collect(),
        };
    };
    let top_ranked: Vec<&Claim> = activation_candidates
        .iter()
        .filter(|(_, _, rank)| *rank == max_rank)
        .map(|(claim, _, _)| *claim)
        .collect();
    let mut top_objects: Vec<&ClaimObject> = Vec::new();
    for claim in &top_ranked {
        if !top_objects.contains(&&claim.object) {
            top_objects.push(&claim.object);
        }
    }
    let equal_rank_conflict = top_objects.len() > 1;
    let active_claim_ids = if equal_rank_conflict {
        Vec::new()
    } else {
        top_ranked.iter().map(|claim| claim.id).collect()
    };
    let mut conflicting_claim_ids: BTreeSet<ClaimId> = if equal_rank_conflict {
        eligible.iter().map(|(claim, _, _)| claim.id).collect()
    } else {
        eligible
            .iter()
            .filter(|(claim, _, _)| !top_ranked.contains(claim))
            .filter(|(claim, _, _)| !top_objects.contains(&&claim.object))
            .map(|(claim, _, _)| claim.id)
            .collect()
    };
    conflicting_claim_ids.extend(lifecycle_conflicts);

    ResolutionResult {
        active_claim_ids,
        conflicting_claim_ids: conflicting_claim_ids.into_iter().collect(),
        rejected_claim_ids: rejected.into_iter().collect(),
    }
}

pub(crate) fn authority_rank(policy: AuthorityPolicy, authority: AuthorityClass) -> u16 {
    match policy {
        AuthorityPolicy::UserOwned => match authority {
            AuthorityClass::UserExplicit => 800,
            AuthorityClass::DirectObservation => 600,
            AuthorityClass::DeterministicEngine => 500,
            AuthorityClass::Curated => 400,
            AuthorityClass::Official => 350,
            AuthorityClass::ModelInference => 200,
            AuthorityClass::Prediction => 100,
            AuthorityClass::Unknown => 0,
        },
        AuthorityPolicy::OfficialFact => match authority {
            AuthorityClass::Official => 800,
            AuthorityClass::DirectObservation => 600,
            AuthorityClass::Curated => 500,
            AuthorityClass::UserExplicit => 400,
            AuthorityClass::DeterministicEngine => 350,
            AuthorityClass::ModelInference => 200,
            AuthorityClass::Prediction => 100,
            AuthorityClass::Unknown => 0,
        },
        AuthorityPolicy::ImplementationObservation => match authority {
            AuthorityClass::DirectObservation => 800,
            AuthorityClass::UserExplicit => 600,
            AuthorityClass::Official | AuthorityClass::Curated => 400,
            AuthorityClass::DeterministicEngine => 350,
            AuthorityClass::ModelInference => 200,
            AuthorityClass::Prediction => 100,
            AuthorityClass::Unknown => 0,
        },
        AuthorityPolicy::CuratedRelation => match authority {
            AuthorityClass::Curated | AuthorityClass::UserExplicit => 800,
            AuthorityClass::Official => 700,
            AuthorityClass::DirectObservation => 600,
            AuthorityClass::DeterministicEngine => 500,
            AuthorityClass::ModelInference => 200,
            AuthorityClass::Prediction => 100,
            AuthorityClass::Unknown => 0,
        },
    }
}

/// Applies the relation authority matrix using only the actor category stored in SQL.
#[must_use]
pub fn relation_effect_is_authorized_for_kind(
    actor_kind: ResolverActorKind,
    kind: ClaimRelationKind,
    source: &Claim,
    target: &Claim,
) -> bool {
    if !matches!(
        kind,
        ClaimRelationKind::Supersedes | ClaimRelationKind::Retracts
    ) {
        return true;
    }
    if matches!(
        source.epistemic_status,
        EpistemicStatus::Disputed | EpistemicStatus::Superseded
    ) || matches!(
        target.epistemic_status,
        EpistemicStatus::Disputed | EpistemicStatus::Superseded
    ) {
        return false;
    }
    if source.authority_class != target.authority_class
        || source.epistemic_status != target.epistemic_status
    {
        return false;
    }
    matches!(
        (actor_kind, source.authority_class, source.epistemic_status),
        (
            ResolverActorKind::User,
            AuthorityClass::UserExplicit,
            EpistemicStatus::UserConfirmed
        ) | (
            ResolverActorKind::DeterministicEngine,
            AuthorityClass::DeterministicEngine,
            EpistemicStatus::DeterministicDerived
        ) | (
            ResolverActorKind::ModelRun,
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred
        ) | (
            ResolverActorKind::ModelRun,
            AuthorityClass::Prediction,
            EpistemicStatus::Prediction
        ) | (
            ResolverActorKind::Importer,
            AuthorityClass::Official,
            EpistemicStatus::OfficialConfirmed
        ) | (
            ResolverActorKind::Importer,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved
        )
    )
}

pub(crate) fn relation_effect_is_authorized(
    actor: &Actor,
    kind: ClaimRelationKind,
    source: &Claim,
    target: &Claim,
) -> bool {
    relation_effect_is_authorized_for_kind(actor.into(), kind, source, target)
}
