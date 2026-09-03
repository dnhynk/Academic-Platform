//! Section 13.1's `KnowledgeStateAssertion`, and why it has no setter.
//!
//! Section 13.4's fourth line is `new KnowledgeStateAssertion (never in-place
//! mutation)`, and section 6.3 says the same about the aggregate: `evidence는
//! 삭제하지 않고 새 assertion이 이전 것을 대체한다`.
//!
//! ## The absence, and what it is not
//!
//! [`KnowledgeStateAssertion`] has private fields, no `&mut self` method, no
//! `Default`, and no setter of any name. [`KnowledgeStateAssertion::revise`]
//! takes `&self` and returns a **new** value whose `supersedes` names the old
//! one; the old value is not consumed and not changed, so a caller that holds
//! version three still holds exactly version three's bytes after version four
//! exists. `assertion_is_never_mutated_in_place` observes both halves, and
//! `crates/knowledge-state/tests/compile_fail/` holds the compiled half: a
//! program that assigns a field and a program that calls a setter, each with a
//! committed diagnostic.
//!
//! ## Identity is a hash chain, not a truncation
//!
//! [`AssertionId`] is a SHA-256 over a **length-prefixed** preimage that
//! includes the version number and the predecessor's identity. Two consequences:
//!
//! * a value that spells a separator cannot collide with two values that do
//!   not — the identity-from-content collapse `P2-A1` found in `P2-R4` came
//!   from joining four fields and truncating them to 64 bytes, and this
//!   preimage joins nothing and truncates nothing; and
//! * a version binds its predecessor, so a history cannot be reordered and a
//!   middle version cannot be dropped without every later identity changing.
//!
//! ## Deserializing cannot smuggle a `FLUENT`
//!
//! `Deserialize` is `#[serde(try_from = "AssertionWire")]`, and the conversion
//! refuses `FLUENT` without a [`FluencyRecord`] and a [`FluencyRecord`] without
//! `FLUENT`. Derived field-by-field deserialization would have been a second
//! constructor that skipped [`crate::confirmation::FluentAuthorization`]
//! entirely, which is exactly the door the type was built to close.

use academic_domain::{
    ConfidencePermille, ContentDigest, EntityId, EvidenceId, FreshnessBand, MasteryLevel,
    TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::{
    KnowledgeStateError,
    confirmation::UserConfirmation,
    evidence::BroadSignal,
    ladder::{FacetProfile, level_token},
    projection::{EvidenceSufficiency, MasteryProjection, UnseenBasis},
};

/// The identity of one assertion version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssertionId(ContentDigest);

impl AssertionId {
    /// The digest, as bytes.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.0
    }
}

fn push_field(preimage: &mut Vec<u8>, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    preimage.extend_from_slice(&length.to_be_bytes());
    preimage.extend_from_slice(bytes);
}

/// What the user confirmed, kept on the assertion it confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmationRecord {
    user_id: EntityId,
    level: MasteryLevel,
    confirmed_at: TimestampMillis,
}

impl ConfirmationRecord {
    /// Records a verified confirmation.
    #[must_use]
    pub fn of(confirmation: &UserConfirmation) -> Self {
        Self {
            user_id: confirmation.user_id(),
            level: confirmation.level(),
            confirmed_at: confirmation.confirmed_at(),
        }
    }

    /// Which user.
    #[must_use]
    pub const fn user_id(&self) -> EntityId {
        self.user_id
    }

    /// Which level.
    #[must_use]
    pub const fn level(&self) -> MasteryLevel {
        self.level
    }

    /// When.
    #[must_use]
    pub const fn confirmed_at(&self) -> TimestampMillis {
        self.confirmed_at
    }
}

/// What authorized a `FLUENT` level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FluencyRecord {
    user_id: EntityId,
    distinct_contexts: usize,
    confirmed_at: TimestampMillis,
}

impl FluencyRecord {
    /// Which user confirmed.
    #[must_use]
    pub const fn user_id(&self) -> EntityId {
        self.user_id
    }

    /// How many distinct independent contexts the repetition carried.
    #[must_use]
    pub const fn distinct_contexts(&self) -> usize {
        self.distinct_contexts
    }

    /// When.
    #[must_use]
    pub const fn confirmed_at(&self) -> TimestampMillis {
        self.confirmed_at
    }
}

/// Section 13.1's assertion, at one version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AssertionWire", into = "AssertionWire")]
pub struct KnowledgeStateAssertion {
    id: AssertionId,
    version: u32,
    supersedes: Option<AssertionId>,
    concept: EntityId,
    as_of: TimestampMillis,
    mastery_level: MasteryLevel,
    facets: FacetProfile,
    estimate_confidence: EvidenceSufficiency,
    freshness_band: FreshnessBand,
    freshness_confidence: ConfidencePermille,
    confirmation: Option<ConfirmationRecord>,
    fluency: Option<FluencyRecord>,
    unseen_basis: Option<UnseenBasis>,
    evidence: Vec<EvidenceId>,
    contradicting_evidence: Vec<EvidenceId>,
    broad_signals: Vec<BroadSignal>,
}

impl KnowledgeStateAssertion {
    /// Opens version one for a concept.
    ///
    /// `freshness_band` and `freshness_confidence` arrive from `P2-N3`: this
    /// task computes no freshness and applies no decay. They are carried
    /// because section 13.1's schema carries them, and because carrying them in
    /// separate fields is what section 1's fifth invariant — `Mastery와
    /// Freshness를 합치지 않는다` — looks like in a type.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::FluencyRecordMissing`] when the projection
    /// reached `FLUENT` without an authorization having produced it.
    pub fn open(
        concept: EntityId,
        as_of: TimestampMillis,
        projection: &MasteryProjection,
        facets: FacetProfile,
        freshness_band: FreshnessBand,
        freshness_confidence: ConfidencePermille,
        broad_signals: Vec<BroadSignal>,
    ) -> Result<Self, KnowledgeStateError> {
        Self::seal(
            1,
            None,
            concept,
            as_of,
            projection,
            facets,
            freshness_band,
            freshness_confidence,
            None,
            broad_signals,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn seal(
        version: u32,
        supersedes: Option<AssertionId>,
        concept: EntityId,
        as_of: TimestampMillis,
        projection: &MasteryProjection,
        facets: FacetProfile,
        freshness_band: FreshnessBand,
        freshness_confidence: ConfidencePermille,
        confirmation: Option<ConfirmationRecord>,
        broad_signals: Vec<BroadSignal>,
    ) -> Result<Self, KnowledgeStateError> {
        let mastery_level = projection.level();
        let fluency = match (mastery_level, projection.fluency_contexts(), &confirmation) {
            (MasteryLevel::Fluent, Some(contexts), Some(record)) => Some(FluencyRecord {
                user_id: record.user_id(),
                distinct_contexts: contexts,
                confirmed_at: record.confirmed_at(),
            }),
            (MasteryLevel::Fluent, _, _) => return Err(KnowledgeStateError::FluencyRecordMissing),
            _ => None,
        };
        let mut assertion = Self {
            id: AssertionId(ContentDigest::sha256(b"")),
            version,
            supersedes,
            concept,
            as_of,
            mastery_level,
            facets,
            estimate_confidence: projection.sufficiency().clone(),
            freshness_band,
            freshness_confidence,
            confirmation,
            fluency,
            unseen_basis: projection.unseen_basis(),
            evidence: projection.supporting().to_vec(),
            contradicting_evidence: projection.contradicting().to_vec(),
            broad_signals,
        };
        assertion.id = AssertionId(assertion.identity_digest());
        Ok(assertion)
    }

    fn identity_digest(&self) -> ContentDigest {
        let mut preimage = b"academic-knowledge-state-assertion-v1\0".to_vec();
        push_field(&mut preimage, &self.version.to_be_bytes());
        push_field(
            &mut preimage,
            self.supersedes
                .map_or_else(Vec::new, |id| id.0.as_bytes().to_vec())
                .as_slice(),
        );
        push_field(&mut preimage, self.concept.to_string().as_bytes());
        push_field(&mut preimage, &self.as_of.value().to_be_bytes());
        push_field(&mut preimage, level_token(self.mastery_level).as_bytes());
        for facet in crate::ladder::MasteryFacet::ALL {
            push_field(&mut preimage, facet.key().as_bytes());
            push_field(
                &mut preimage,
                self.facets.strength(facet).as_str().as_bytes(),
            );
        }
        push_field(
            &mut preimage,
            &self.estimate_confidence.permille().value().to_be_bytes(),
        );
        push_field(&mut preimage, &self.freshness_confidence.value().to_be_bytes());
        for id in &self.evidence {
            push_field(&mut preimage, id.to_string().as_bytes());
        }
        push_field(&mut preimage, b"\0contradicting\0");
        for id in &self.contradicting_evidence {
            push_field(&mut preimage, id.to_string().as_bytes());
        }
        ContentDigest::sha256(&preimage)
    }

    /// This version's identity.
    #[must_use]
    pub const fn id(&self) -> AssertionId {
        self.id
    }

    /// Which version. Version one is the first.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// The version this one replaces, when it replaces one.
    #[must_use]
    pub const fn supersedes(&self) -> Option<AssertionId> {
        self.supersedes
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The valid-time coordinate.
    #[must_use]
    pub const fn as_of(&self) -> TimestampMillis {
        self.as_of
    }

    /// The projected level.
    #[must_use]
    pub const fn mastery_level(&self) -> MasteryLevel {
        self.mastery_level
    }

    /// All five facets.
    #[must_use]
    pub const fn facets(&self) -> FacetProfile {
        self.facets
    }

    /// Section 13.1's `estimateConfidence`, which is evidence sufficiency.
    #[must_use]
    pub const fn estimate_confidence(&self) -> &EvidenceSufficiency {
        &self.estimate_confidence
    }

    /// `P2-N3`'s band, carried and never merged with the level.
    #[must_use]
    pub const fn freshness_band(&self) -> FreshnessBand {
        self.freshness_band
    }

    /// `P2-N3`'s confidence in that band.
    #[must_use]
    pub const fn freshness_confidence(&self) -> ConfidencePermille {
        self.freshness_confidence
    }

    /// Whether the user confirmed this state.
    #[must_use]
    pub const fn user_confirmed(&self) -> bool {
        self.confirmation.is_some()
    }

    /// What the user confirmed, when they did.
    #[must_use]
    pub const fn confirmation(&self) -> Option<&ConfirmationRecord> {
        self.confirmation.as_ref()
    }

    /// What authorized `FLUENT`, when the level is `FLUENT`.
    #[must_use]
    pub const fn fluency(&self) -> Option<&FluencyRecord> {
        self.fluency.as_ref()
    }

    /// Why the level is `UNSEEN`, when it is.
    #[must_use]
    pub const fn unseen_basis(&self) -> Option<UnseenBasis> {
        self.unseen_basis
    }

    /// The supporting evidence.
    #[must_use]
    pub fn evidence(&self) -> &[EvidenceId] {
        &self.evidence
    }

    /// The contradicting evidence, which is retained and never deleted.
    #[must_use]
    pub fn contradicting_evidence(&self) -> &[EvidenceId] {
        &self.contradicting_evidence
    }

    /// Course-wide signals kept beside the concept, promoting nothing.
    #[must_use]
    pub fn broad_signals(&self) -> &[BroadSignal] {
        &self.broad_signals
    }

    /// Produces the next version. This value is unchanged.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::FluencyRecordMissing`] when the new projection
    /// reached `FLUENT` without an authorization having produced it.
    pub fn revise(
        &self,
        as_of: TimestampMillis,
        projection: &MasteryProjection,
        facets: FacetProfile,
        freshness_band: FreshnessBand,
        freshness_confidence: ConfidencePermille,
        broad_signals: Vec<BroadSignal>,
    ) -> Result<Self, KnowledgeStateError> {
        Self::seal(
            self.version.saturating_add(1),
            Some(self.id),
            self.concept,
            as_of,
            projection,
            facets,
            freshness_band,
            freshness_confidence,
            self.confirmation,
            broad_signals,
        )
    }

    /// Produces the next version carrying the user's confirmation.
    ///
    /// The confirmation is taken by reference and its record is copied, so the
    /// caller keeps the token; what matters is that only a
    /// [`UserConfirmation`] — which an automatic actor cannot mint — reaches
    /// this method at all.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::ConfirmationSubjectMismatch`] when the
    /// confirmation is about another concept,
    /// [`KnowledgeStateError::ConfirmationLevelMismatch`] when it names a level
    /// the projection does not hold, and
    /// [`KnowledgeStateError::FluencyRecordMissing`] as above.
    pub fn confirmed(
        &self,
        as_of: TimestampMillis,
        projection: &MasteryProjection,
        facets: FacetProfile,
        freshness_band: FreshnessBand,
        freshness_confidence: ConfidencePermille,
        confirmation: &UserConfirmation,
    ) -> Result<Self, KnowledgeStateError> {
        if confirmation.concept() != self.concept {
            return Err(KnowledgeStateError::ConfirmationSubjectMismatch);
        }
        if confirmation.level() != projection.level() {
            return Err(KnowledgeStateError::ConfirmationLevelMismatch);
        }
        Self::seal(
            self.version.saturating_add(1),
            Some(self.id),
            self.concept,
            as_of,
            projection,
            facets,
            freshness_band,
            freshness_confidence,
            Some(ConfirmationRecord::of(confirmation)),
            self.broad_signals.clone(),
        )
    }
}

/// The serialized shape, and the one door back in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionWire {
    id: AssertionId,
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    supersedes: Option<AssertionId>,
    concept: EntityId,
    as_of: TimestampMillis,
    mastery_level: MasteryLevel,
    facets: FacetProfile,
    estimate_confidence: EvidenceSufficiency,
    freshness_band: FreshnessBand,
    freshness_confidence: ConfidencePermille,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    confirmation: Option<ConfirmationRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fluency: Option<FluencyRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unseen_basis: Option<UnseenBasis>,
    evidence: Vec<EvidenceId>,
    contradicting_evidence: Vec<EvidenceId>,
    broad_signals: Vec<BroadSignal>,
}

impl From<KnowledgeStateAssertion> for AssertionWire {
    fn from(value: KnowledgeStateAssertion) -> Self {
        Self {
            id: value.id,
            version: value.version,
            supersedes: value.supersedes,
            concept: value.concept,
            as_of: value.as_of,
            mastery_level: value.mastery_level,
            facets: value.facets,
            estimate_confidence: value.estimate_confidence,
            freshness_band: value.freshness_band,
            freshness_confidence: value.freshness_confidence,
            confirmation: value.confirmation,
            fluency: value.fluency,
            unseen_basis: value.unseen_basis,
            evidence: value.evidence,
            contradicting_evidence: value.contradicting_evidence,
            broad_signals: value.broad_signals,
        }
    }
}

impl TryFrom<AssertionWire> for KnowledgeStateAssertion {
    type Error = KnowledgeStateError;

    fn try_from(value: AssertionWire) -> Result<Self, Self::Error> {
        match (value.mastery_level, value.fluency.is_some()) {
            (MasteryLevel::Fluent, false) => {
                return Err(KnowledgeStateError::FluencyRecordMissing);
            }
            (level, true) if level != MasteryLevel::Fluent => {
                return Err(KnowledgeStateError::FluencyRecordNotFluent);
            }
            _ => {}
        }
        let restored = Self {
            id: value.id,
            version: value.version,
            supersedes: value.supersedes,
            concept: value.concept,
            as_of: value.as_of,
            mastery_level: value.mastery_level,
            facets: value.facets,
            estimate_confidence: value.estimate_confidence,
            freshness_band: value.freshness_band,
            freshness_confidence: value.freshness_confidence,
            confirmation: value.confirmation,
            fluency: value.fluency,
            unseen_basis: value.unseen_basis,
            evidence: value.evidence,
            contradicting_evidence: value.contradicting_evidence,
            broad_signals: value.broad_signals,
        };
        if restored.identity_digest() != *restored.id.digest() {
            return Err(KnowledgeStateError::AssertionIdentityMismatch);
        }
        Ok(restored)
    }
}
