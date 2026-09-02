//! Product-scale authority tables layered on the Phase 1 ledger resolver.
//!
//! This module supplies the six claim-type policies from design section 30.3.
//! It deliberately delegates decision replay, lifecycle handling, scope
//! isolation, and conflict selection to the Phase 1 resolver body.

use std::collections::BTreeSet;

use academic_domain::{
    AuthorityClass, ClaimId, ClaimObject, ContentDigest, EntityId, EpistemicStatus, PredicateId,
    ScopeId, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::resolver::{
    AuthorityPolicy, ResolutionClaim, ResolutionDecision, ResolutionQuery, ResolutionRelation,
    ResolutionResult, resolve_snapshot_with_authority_rank,
};

/// The canonical machine token emitted for contrary evidence.
pub const NEW_EVIDENCE_CONFLICT: &str = "NEW_EVIDENCE_CONFLICT";

/// Backward-compatible UI spelling accepted at presentation boundaries.
pub const NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE: &str = "NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE";

/// The six claim kinds whose authority order is fixed independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductClaimType {
    OfficialAcademicFact,
    PersonalIntent,
    MasteryQuestionResolution,
    CurrentImplementation,
    ProjectIntent,
    RelationPrerequisite,
}

impl ProductClaimType {
    /// Complete closed set, in design-section order.
    pub const ALL: [Self; 6] = [
        Self::OfficialAcademicFact,
        Self::PersonalIntent,
        Self::MasteryQuestionResolution,
        Self::CurrentImplementation,
        Self::ProjectIntent,
        Self::RelationPrerequisite,
    ];

    /// Returns this claim kind's complete precedence table.
    #[must_use]
    pub const fn authority_table(self) -> AuthorityTable {
        let ranks = match self {
            Self::OfficialAcademicFact => [800, 400, 600, 350, 500, 200, 100, 0],
            Self::PersonalIntent => [600, 800, 500, 400, 600, 200, 100, 0],
            Self::MasteryQuestionResolution => [350, 800, 700, 600, 400, 200, 100, 0],
            Self::CurrentImplementation => [400, 600, 800, 350, 400, 200, 100, 0],
            Self::ProjectIntent => [750, 600, 400, 350, 800, 200, 100, 0],
            Self::RelationPrerequisite => [700, 800, 600, 500, 800, 200, 100, 0],
        };
        AuthorityTable {
            claim_type: self,
            entries: [
                AuthorityRank::new(AuthorityClass::Official, ranks[0]),
                AuthorityRank::new(AuthorityClass::UserExplicit, ranks[1]),
                AuthorityRank::new(AuthorityClass::DirectObservation, ranks[2]),
                AuthorityRank::new(AuthorityClass::DeterministicEngine, ranks[3]),
                AuthorityRank::new(AuthorityClass::Curated, ranks[4]),
                AuthorityRank::new(AuthorityClass::ModelInference, ranks[5]),
                AuthorityRank::new(AuthorityClass::Prediction, ranks[6]),
                AuthorityRank::new(AuthorityClass::Unknown, ranks[7]),
            ],
        }
    }

    const fn phase1_query_policy(self) -> AuthorityPolicy {
        match self {
            Self::OfficialAcademicFact => AuthorityPolicy::OfficialFact,
            Self::CurrentImplementation => AuthorityPolicy::ImplementationObservation,
            Self::RelationPrerequisite => AuthorityPolicy::CuratedRelation,
            Self::PersonalIntent | Self::MasteryQuestionResolution | Self::ProjectIntent => {
                AuthorityPolicy::UserOwned
            }
        }
    }
}

/// One entry in a complete claim-type authority table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityRank {
    pub authority_class: AuthorityClass,
    pub rank: u16,
}

impl AuthorityRank {
    const fn new(authority_class: AuthorityClass, rank: u16) -> Self {
        Self {
            authority_class,
            rank,
        }
    }
}

/// Complete precedence table for one product claim kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityTable {
    pub claim_type: ProductClaimType,
    pub entries: [AuthorityRank; 8],
}

impl AuthorityTable {
    /// Returns the stable rank for every domain authority class.
    #[must_use]
    pub fn rank(self, authority: AuthorityClass) -> u16 {
        self.entries
            .iter()
            .find(|entry| entry.authority_class == authority)
            .map_or(0, |entry| entry.rank)
    }
}

/// Product coordinates select a claim-type table without weakening the
/// bitemporal and scope coordinates required by the Phase 1 query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductResolutionQuery {
    pub subject_entity_id: EntityId,
    pub scope_id: ScopeId,
    pub predicate_id: PredicateId,
    pub valid_at: TimestampMillis,
    pub known_at_accept_seq: u64,
    pub claim_type: ProductClaimType,
}

/// Existing provenance identity associated with one accepted claim.
///
/// `source_digest` is the canonical upstream provenance digest. It is not an
/// artifact `content_digest`, and absence does not establish independence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimSourceProvenance {
    pub claim_id: ClaimId,
    pub source_digest: Option<ContentDigest>,
}

/// Bounded reasons by which an upstream provenance layer may establish that
/// two differently digested sources are independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IndependenceBasis {
    DistinctSignedOrigins,
    SeparateDirectObservations,
}

/// Explicit pairwise attestation supplied by the provenance layer.
///
/// Digest inequality alone is not an attestation. The resolver also refuses an
/// attestation when either digest is absent or the two digests are equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceIndependenceAttestation {
    pub first_claim_id: ClaimId,
    pub second_claim_id: ClaimId,
    pub basis: IndependenceBasis,
}

/// Relation inference level after fail-closed source de-duplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationSupportTier {
    SingleSourceInference,
    CorroboratedInference,
}

/// Stable explanations for why a relation inference did or did not promote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CorroborationReasonCode {
    SingleSource,
    DuplicateUpstreamSource,
    MissingSourceDigest,
    AmbiguousSourceMetadata,
    IndependenceUnestablished,
    IndependentSourcesEstablished,
}

/// Per-object source assessment returned for relation/prerequisite queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationSupportAssessment {
    pub object: ClaimObject,
    pub claim_ids: Vec<ClaimId>,
    pub tier: RelationSupportTier,
    pub reason_codes: Vec<CorroborationReasonCode>,
}

/// Canonical conflict reason; both accepted spellings parse to this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConflictReason {
    NewEvidenceConflict,
}

impl ConflictReason {
    /// Canonical machine spelling emitted by this resolver.
    #[must_use]
    pub const fn canonical_token(self) -> &'static str {
        NEW_EVIDENCE_CONFLICT
    }

    /// Parses either the canonical token or its retained UI alias.
    #[must_use]
    pub fn from_token(value: &str) -> Option<Self> {
        matches!(
            value,
            NEW_EVIDENCE_CONFLICT | NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE
        )
        .then_some(Self::NewEvidenceConflict)
    }
}

/// Visible conflict emitted instead of overwriting a competing claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictCard {
    pub reason: ConflictReason,
    pub subject_entity_id: EntityId,
    pub predicate_id: PredicateId,
    pub scope_id: ScopeId,
    pub active_claim_ids: Vec<ClaimId>,
    pub conflicting_claim_ids: Vec<ClaimId>,
}

/// Product result containing the unchanged Phase 1 resolution plus product
/// conflict and relation-source disclosures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductResolutionResult {
    pub resolution: ResolutionResult,
    pub conflict_cards: Vec<ConflictCard>,
    pub relation_support: Vec<RelationSupportAssessment>,
}

/// Resolves one product claim type through the Phase 1 decision/lifecycle core.
#[must_use]
pub fn resolve_product_snapshot(
    query: &ProductResolutionQuery,
    claims: &[ResolutionClaim],
    relations: &[ResolutionRelation],
    decisions: &[ResolutionDecision],
    provenance: &[ClaimSourceProvenance],
    independence: &[SourceIndependenceAttestation],
) -> ProductResolutionResult {
    let relation_support = if query.claim_type == ProductClaimType::RelationPrerequisite {
        assess_relation_support(query, claims, provenance, independence)
    } else {
        Vec::new()
    };
    let corroborated_claims: BTreeSet<ClaimId> = relation_support
        .iter()
        .filter(|assessment| assessment.tier == RelationSupportTier::CorroboratedInference)
        .flat_map(|assessment| assessment.claim_ids.iter().copied())
        .collect();
    let table = query.claim_type.authority_table();
    let phase1_query = ResolutionQuery {
        subject_entity_id: query.subject_entity_id,
        scope_id: query.scope_id,
        predicate_id: query.predicate_id.clone(),
        valid_at: query.valid_at,
        known_at_accept_seq: query.known_at_accept_seq,
        policy: query.claim_type.phase1_query_policy(),
    };
    let resolution = resolve_snapshot_with_authority_rank(
        &phase1_query,
        claims,
        relations,
        decisions,
        |claim| {
            if query.claim_type == ProductClaimType::RelationPrerequisite
                && claim.authority_class == AuthorityClass::ModelInference
                && corroborated_claims.contains(&claim.id)
            {
                table.rank(AuthorityClass::DeterministicEngine)
            } else {
                table.rank(claim.authority_class)
            }
        },
        table.rank(AuthorityClass::UserExplicit),
    );
    let conflict_cards = (!resolution.conflicting_claim_ids.is_empty())
        .then(|| ConflictCard {
            reason: ConflictReason::NewEvidenceConflict,
            subject_entity_id: query.subject_entity_id,
            predicate_id: query.predicate_id.clone(),
            scope_id: query.scope_id,
            active_claim_ids: resolution.active_claim_ids.clone(),
            conflicting_claim_ids: resolution.conflicting_claim_ids.clone(),
        })
        .into_iter()
        .collect();
    ProductResolutionResult {
        resolution,
        conflict_cards,
        relation_support,
    }
}

fn assess_relation_support(
    query: &ProductResolutionQuery,
    claims: &[ResolutionClaim],
    provenance: &[ClaimSourceProvenance],
    independence: &[SourceIndependenceAttestation],
) -> Vec<RelationSupportAssessment> {
    let mut groups: Vec<(ClaimObject, Vec<ClaimId>)> = Vec::new();
    for record in claims.iter().filter(|record| {
        let claim = &record.claim;
        record.accept_seq <= query.known_at_accept_seq
            && claim.validate().is_ok()
            && claim.subject_entity_id == query.subject_entity_id
            && claim.predicate_id == query.predicate_id
            && claim.scope_id == query.scope_id
            && claim.valid_time.contains(query.valid_at)
            && claim.authority_class == AuthorityClass::ModelInference
            && !matches!(
                claim.epistemic_status,
                EpistemicStatus::Disputed | EpistemicStatus::Superseded
            )
    }) {
        if let Some((_, claim_ids)) = groups
            .iter_mut()
            .find(|(object, _)| object == &record.claim.object)
        {
            claim_ids.push(record.claim.id);
        } else {
            groups.push((record.claim.object.clone(), vec![record.claim.id]));
        }
    }

    groups
        .into_iter()
        .map(|(object, mut claim_ids)| {
            claim_ids.sort_unstable();
            let mut reasons = BTreeSet::new();
            let digests: Vec<DigestLookup> = claim_ids
                .iter()
                .map(|claim_id| source_digest(*claim_id, provenance))
                .collect();
            if claim_ids.len() == 1 {
                reasons.insert(CorroborationReasonCode::SingleSource);
            }
            if digests.contains(&DigestLookup::Missing) {
                reasons.insert(CorroborationReasonCode::MissingSourceDigest);
            }
            if digests.contains(&DigestLookup::Ambiguous) {
                reasons.insert(CorroborationReasonCode::AmbiguousSourceMetadata);
            }
            let present: Vec<ContentDigest> = digests
                .iter()
                .filter_map(|digest| match digest {
                    DigestLookup::Present(value) => Some(*value),
                    DigestLookup::Missing | DigestLookup::Ambiguous => None,
                })
                .collect();
            let distinct: BTreeSet<ContentDigest> = present.iter().copied().collect();
            if distinct.len() < present.len() {
                reasons.insert(CorroborationReasonCode::DuplicateUpstreamSource);
            }
            let established = independent_pair_exists(&claim_ids, &digests, independence);
            let tier = if established {
                reasons.insert(CorroborationReasonCode::IndependentSourcesEstablished);
                RelationSupportTier::CorroboratedInference
            } else {
                if claim_ids.len() > 1 {
                    reasons.insert(CorroborationReasonCode::IndependenceUnestablished);
                }
                RelationSupportTier::SingleSourceInference
            };
            RelationSupportAssessment {
                object,
                claim_ids,
                tier,
                reason_codes: reasons.into_iter().collect(),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DigestLookup {
    Missing,
    Present(ContentDigest),
    Ambiguous,
}

fn source_digest(claim_id: ClaimId, provenance: &[ClaimSourceProvenance]) -> DigestLookup {
    let mut matches = provenance
        .iter()
        .filter(|source| source.claim_id == claim_id);
    let Some(first) = matches.next() else {
        return DigestLookup::Missing;
    };
    if matches.next().is_some() {
        return DigestLookup::Ambiguous;
    }
    first
        .source_digest
        .map_or(DigestLookup::Missing, DigestLookup::Present)
}

fn independent_pair_exists(
    claim_ids: &[ClaimId],
    digests: &[DigestLookup],
    independence: &[SourceIndependenceAttestation],
) -> bool {
    claim_ids.iter().enumerate().any(|(first_index, first)| {
        claim_ids
            .iter()
            .enumerate()
            .skip(first_index + 1)
            .any(|(second_index, second)| {
                let independently_digested = matches!(
                    (digests[first_index], digests[second_index]),
                    (DigestLookup::Present(left), DigestLookup::Present(right)) if left != right
                );
                independently_digested
                    && independence.iter().any(|attestation| {
                        (attestation.first_claim_id == *first
                            && attestation.second_claim_id == *second)
                            || (attestation.first_claim_id == *second
                                && attestation.second_claim_id == *first)
                    })
            })
    })
}
