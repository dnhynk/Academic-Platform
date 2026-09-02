//! Time-bearing questions, their lifecycle, workspace, and growth descriptors.

use std::{collections::BTreeSet, num::NonZeroU64};

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{
    Actor, ArtifactId, Claim, ClaimObject, ContentDigest, DomainError, EntityId, EpistemicStatus,
    EvidenceId, EvidenceItem, LogicalPath, ScopeId, TimestampMillis,
};

/// Predicate carried by a direct user decision to resolve a question.
pub const QUESTION_RESOLUTION_PREDICATE: &str = "question.resolution.user";
/// Predicate carried by the user's approval of a completed pre-declared validation.
pub const QUESTION_VALIDATION_APPROVAL_PREDICATE: &str = "question.resolution.validation";
/// Fixed claim object for either resolution action.
pub const QUESTION_RESOLUTION_OBJECT: &str = "RESOLVE";

/// Failures at the question lifecycle and workspace boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuestionError {
    /// A required text value was empty.
    #[error("{0} must not be empty")]
    EmptyValue(&'static str),
    /// A question revision or lifecycle event moved backwards in time.
    #[error("question time sequence is not monotonic")]
    NonMonotonicTime,
    /// A lifecycle edge is not present in the normative transition table.
    #[error("question lifecycle transition {from:?}->{to:?} is not allowed")]
    TransitionNotAllowed {
        /// Current state.
        from: QuestionStatus,
        /// Requested state.
        to: QuestionStatus,
    },
    /// The supplied lifecycle definition differs from the normative definition.
    #[error("question lifecycle definition differs from the normative status and edge lists")]
    InvalidLifecycleDefinition,
    /// A partially resolved question omitted its supporting evidence.
    #[error("a partial resolution requires evidence")]
    PartialResolutionEvidenceMissing,
    /// The ADR-003 actor/authority/status matrix rejected a claim.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A resolution claim did not carry the exact action predicate and object.
    #[error("question resolution claim has the wrong predicate or object")]
    InvalidResolutionAction,
    /// A resolution claim named another question.
    #[error("question resolution claim names the wrong question")]
    ResolutionQuestionMismatch,
    /// A resolution claim named another resolution scope.
    #[error("question resolution claim names the wrong scope")]
    ResolutionScopeMismatch,
    /// A resolution claim did not cite the evidence supplied with it.
    #[error("question resolution claim does not cite its evidence")]
    ResolutionEvidenceMissing,
    /// A pre-declared validation was completed before it was declared.
    #[error("question validation completion predates its declaration")]
    ValidationCompletionPredatesDeclaration,
    /// The validation evidence does not match the result declared in advance.
    #[error("question validation evidence differs from the pre-declared result")]
    ValidationResultMismatch,
    /// User approval predates completion of the validation it approves.
    #[error("question validation approval predates validation completion")]
    ValidationApprovalPredatesCompletion,
    /// The persisted status metadata is incomplete or inconsistent.
    #[error("question status metadata is inconsistent")]
    InvalidStatusMetadata,
    /// An obsolescence record omitted evidence.
    #[error("question obsolescence requires evidence")]
    ObsolescenceEvidenceMissing,
    /// A reframe attempted to reuse the old question identity.
    #[error("a reframed question requires a new identity")]
    ReframeIdentityReused,
    /// An AI explanation was supplied without an explicit opt-in.
    #[error("AI explanation requires explicit opt-in")]
    AiExplanationNotRequested,
    /// An AI explanation belongs to another question.
    #[error("AI explanation names another question")]
    AiExplanationQuestionMismatch,
}

fn require_text(value: impl Into<String>, name: &'static str) -> Result<String, QuestionError> {
    let value = value.into();
    if value.trim().is_empty() {
        Err(QuestionError::EmptyValue(name))
    } else {
        Ok(value)
    }
}

/// Non-repository location text, such as `audio@42:18` or a page coordinate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ContextLocator(String);

impl ContextLocator {
    /// Parses a non-empty origin coordinate.
    pub fn parse(value: impl Into<String>) -> Result<Self, QuestionError> {
        Ok(Self(require_text(value, "question origin locator")?))
    }

    /// Returns the exact coordinate text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContextLocator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// The context in which a question first arose.
///
/// The repository variant has no generic locator form: snapshot, path, and
/// one-based line are mandatory fields of the variant itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionOrigin {
    /// A lecture recording or document coordinate.
    Lecture {
        entity: EntityId,
        locator: ContextLocator,
    },
    /// A course-material coordinate.
    CourseMaterial {
        entity: EntityId,
        locator: ContextLocator,
    },
    /// An assignment coordinate.
    Assignment {
        entity: EntityId,
        locator: ContextLocator,
    },
    /// A personal-study coordinate.
    PersonalStudy {
        entity: EntityId,
        locator: ContextLocator,
    },
    /// An immutable repository snapshot coordinate.
    Repository {
        entity: EntityId,
        snapshot: ContentDigest,
        path: LogicalPath,
        line: NonZeroU64,
    },
    /// A code-review coordinate.
    CodeReview {
        entity: EntityId,
        locator: ContextLocator,
    },
    /// A project-specification coordinate.
    ProjectSpec {
        entity: EntityId,
        locator: ContextLocator,
    },
    /// A concept-detail coordinate.
    ConceptDetail {
        entity: EntityId,
        locator: ContextLocator,
    },
}

/// Whether importance came directly from the user or from visible context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionImportance {
    /// Explicitly set by the user.
    UserSet,
    /// Derived from context and still presented as such.
    ContextDerived,
}

// QUESTION_STATUS_SCHEMA_BEGIN
/// The lifecycle states named by design specification section 14.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionStatus {
    Open,
    PartiallyResolved,
    Resolved,
    Reframed,
    Obsolete,
    Reopened,
}
// QUESTION_STATUS_SCHEMA_END

impl QuestionStatus {
    /// Complete ordered status vocabulary from the specification.
    pub const ALL: [Self; 6] = [
        Self::Open,
        Self::PartiallyResolved,
        Self::Resolved,
        Self::Reframed,
        Self::Obsolete,
        Self::Reopened,
    ];

    /// Returns the stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::PartiallyResolved => "PARTIALLY_RESOLVED",
            Self::Resolved => "RESOLVED",
            Self::Reframed => "REFRAMED",
            Self::Obsolete => "OBSOLETE",
            Self::Reopened => "REOPENED",
        }
    }
}

/// One allowed directed lifecycle edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuestionTransition {
    /// Source status.
    pub from: QuestionStatus,
    /// Destination status.
    pub to: QuestionStatus,
}

/// Exact edges drawn in design specification section 14.2.
pub const QUESTION_LIFECYCLE_TRANSITIONS: [QuestionTransition; 7] = [
    QuestionTransition {
        from: QuestionStatus::Open,
        to: QuestionStatus::PartiallyResolved,
    },
    QuestionTransition {
        from: QuestionStatus::Open,
        to: QuestionStatus::Reframed,
    },
    QuestionTransition {
        from: QuestionStatus::Open,
        to: QuestionStatus::Obsolete,
    },
    QuestionTransition {
        from: QuestionStatus::PartiallyResolved,
        to: QuestionStatus::Resolved,
    },
    QuestionTransition {
        from: QuestionStatus::PartiallyResolved,
        to: QuestionStatus::Reframed,
    },
    QuestionTransition {
        from: QuestionStatus::Resolved,
        to: QuestionStatus::Reopened,
    },
    QuestionTransition {
        from: QuestionStatus::Resolved,
        to: QuestionStatus::Reframed,
    },
];

/// Returns whether the exact directed edge appears in the normative table.
#[must_use]
pub fn lifecycle_transition_is_allowed(from: QuestionStatus, to: QuestionStatus) -> bool {
    QUESTION_LIFECYCLE_TRANSITIONS.contains(&QuestionTransition { from, to })
}

/// Validates a candidate lifecycle definition against both normative lists.
///
/// This is public so build-time contract tests can inject both missing allowed
/// edges and admitted non-edges into the same guard used to describe the table.
pub fn validate_lifecycle_definition(
    statuses: &[QuestionStatus],
    transitions: &[QuestionTransition],
) -> Result<(), QuestionError> {
    let observed_statuses = statuses.iter().copied().collect::<BTreeSet<_>>();
    let expected_statuses = QuestionStatus::ALL.into_iter().collect::<BTreeSet<_>>();
    let observed_transitions = transitions.iter().copied().collect::<BTreeSet<_>>();
    let expected_transitions = QUESTION_LIFECYCLE_TRANSITIONS
        .into_iter()
        .collect::<BTreeSet<_>>();
    if statuses.len() != observed_statuses.len()
        || observed_statuses != expected_statuses
        || transitions.len() != observed_transitions.len()
        || observed_transitions != expected_transitions
    {
        return Err(QuestionError::InvalidLifecycleDefinition);
    }
    Ok(())
}

/// One append-only wording revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuestionRevision {
    previous_text: String,
    replacement_text: String,
    revised_at: TimestampMillis,
}

impl QuestionRevision {
    /// Returns the text retained from before the revision.
    #[must_use]
    pub fn previous_text(&self) -> &str {
        &self.previous_text
    }

    /// Returns the replacement wording.
    #[must_use]
    pub fn replacement_text(&self) -> &str {
        &self.replacement_text
    }

    /// Returns when the wording revision occurred.
    #[must_use]
    pub const fn revised_at(&self) -> TimestampMillis {
        self.revised_at
    }
}

/// One append-only lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuestionLifecycleEvent {
    from: QuestionStatus,
    to: QuestionStatus,
    occurred_at: TimestampMillis,
    evidence_ids: Vec<EvidenceId>,
}

impl QuestionLifecycleEvent {
    /// Returns the source state.
    #[must_use]
    pub const fn from(&self) -> QuestionStatus {
        self.from
    }

    /// Returns the destination state.
    #[must_use]
    pub const fn to(&self) -> QuestionStatus {
        self.to
    }

    /// Returns when the transition occurred.
    #[must_use]
    pub const fn occurred_at(&self) -> TimestampMillis {
        self.occurred_at
    }

    /// Returns the transition's evidence references.
    #[must_use]
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

/// A validation criterion and expected result fixed before execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PredeclaredQuestionValidation {
    validation_id: EntityId,
    question_id: EntityId,
    scope_id: ScopeId,
    expected_result_digest: ContentDigest,
    declared_at: TimestampMillis,
}

impl PredeclaredQuestionValidation {
    /// Declares the validation result that would be eligible for later approval.
    #[must_use]
    pub const fn new(
        validation_id: EntityId,
        question_id: EntityId,
        scope_id: ScopeId,
        expected_result_digest: ContentDigest,
        declared_at: TimestampMillis,
    ) -> Self {
        Self {
            validation_id,
            question_id,
            scope_id,
            expected_result_digest,
            declared_at,
        }
    }

    /// Returns the validation identity.
    #[must_use]
    pub const fn validation_id(&self) -> EntityId {
        self.validation_id
    }
}

/// Evidence that a pre-declared validation completed with its expected result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuestionValidationCompletion {
    declaration: PredeclaredQuestionValidation,
    completed_at: TimestampMillis,
    evidence: EvidenceItem,
}

impl QuestionValidationCompletion {
    /// Verifies completion against the declaration's fixed result digest.
    pub fn new(
        declaration: PredeclaredQuestionValidation,
        completed_at: TimestampMillis,
        evidence: EvidenceItem,
    ) -> Result<Self, QuestionError> {
        let value = Self {
            declaration,
            completed_at,
            evidence,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), QuestionError> {
        self.evidence.validate()?;
        if self.completed_at < self.declaration.declared_at {
            return Err(QuestionError::ValidationCompletionPredatesDeclaration);
        }
        if self.evidence.excerpt_digest != self.declaration.expected_result_digest {
            return Err(QuestionError::ValidationResultMismatch);
        }
        Ok(())
    }

    /// Returns when the validation completed.
    #[must_use]
    pub const fn completed_at(&self) -> TimestampMillis {
        self.completed_at
    }
}

/// Persisted evidence of the user action that resolved a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionResolutionDecision {
    /// A direct explicit user decision.
    UserDecision {
        actor: Actor,
        claim: Claim,
        evidence: EvidenceItem,
    },
    /// A completed pre-declared validation followed by explicit user approval.
    ValidatedThenApproved {
        completion: Box<QuestionValidationCompletion>,
        actor: Actor,
        claim: Claim,
        evidence: EvidenceItem,
    },
}

impl QuestionResolutionDecision {
    fn validate_for(&self, question_id: EntityId, scope_id: ScopeId) -> Result<(), QuestionError> {
        match self {
            Self::UserDecision {
                actor,
                claim,
                evidence,
            } => {
                verify_user_resolution_action(
                    actor,
                    claim,
                    evidence,
                    question_id,
                    scope_id,
                    QUESTION_RESOLUTION_PREDICATE,
                )?;
            }
            Self::ValidatedThenApproved {
                completion,
                actor,
                claim,
                evidence,
            } => {
                completion.validate()?;
                if completion.declaration.question_id != question_id
                    || completion.declaration.scope_id != scope_id
                {
                    return Err(QuestionError::ResolutionQuestionMismatch);
                }
                if claim.valid_time.from() < completion.completed_at {
                    return Err(QuestionError::ValidationApprovalPredatesCompletion);
                }
                verify_user_resolution_action(
                    actor,
                    claim,
                    evidence,
                    question_id,
                    scope_id,
                    QUESTION_VALIDATION_APPROVAL_PREDICATE,
                )?;
                if !claim.evidence_ids.contains(&completion.evidence.id) {
                    return Err(QuestionError::ResolutionEvidenceMissing);
                }
            }
        }
        Ok(())
    }
}

fn verify_user_resolution_action(
    actor: &Actor,
    claim: &Claim,
    evidence: &EvidenceItem,
    question_id: EntityId,
    scope_id: ScopeId,
    predicate: &'static str,
) -> Result<EntityId, QuestionError> {
    claim.validate_for_actor(actor)?;
    if claim.epistemic_status != EpistemicStatus::UserConfirmed
        || claim.predicate_id.as_str() != predicate
        || claim.object != ClaimObject::Text(QUESTION_RESOLUTION_OBJECT.to_owned())
    {
        return Err(QuestionError::InvalidResolutionAction);
    }
    if claim.subject_entity_id != question_id {
        return Err(QuestionError::ResolutionQuestionMismatch);
    }
    if claim.scope_id != scope_id {
        return Err(QuestionError::ResolutionScopeMismatch);
    }
    if !claim.evidence_ids.contains(&evidence.id) {
        return Err(QuestionError::ResolutionEvidenceMissing);
    }
    evidence.validate()?;
    let Actor::User { user_id } = actor else {
        return Err(QuestionError::InvalidResolutionAction);
    };
    Ok(*user_id)
}

/// Non-forgeable authority consumed by [`Question::resolve`].
///
/// It can only be built after the existing ADR-003 claim matrix verifies an
/// explicit user action. A model proposal, deterministic result, or importer
/// claim does not produce this type.
#[derive(Debug)]
pub struct VerifiedQuestionResolution {
    decision: QuestionResolutionDecision,
}

impl VerifiedQuestionResolution {
    /// Verifies a direct explicit user decision.
    pub fn user_decision(
        question: &Question,
        actor: &Actor,
        claim: &Claim,
        evidence: &EvidenceItem,
    ) -> Result<Self, QuestionError> {
        verify_user_resolution_action(
            actor,
            claim,
            evidence,
            question.id,
            question.scope_id,
            QUESTION_RESOLUTION_PREDICATE,
        )?;
        Ok(Self {
            decision: QuestionResolutionDecision::UserDecision {
                actor: actor.clone(),
                claim: claim.clone(),
                evidence: evidence.clone(),
            },
        })
    }

    /// Verifies user approval after a matching pre-declared validation completes.
    pub fn validated_then_approved(
        question: &Question,
        completion: QuestionValidationCompletion,
        actor: &Actor,
        claim: &Claim,
        evidence: &EvidenceItem,
    ) -> Result<Self, QuestionError> {
        let decision = QuestionResolutionDecision::ValidatedThenApproved {
            completion: Box::new(completion),
            actor: actor.clone(),
            claim: claim.clone(),
            evidence: evidence.clone(),
        };
        decision.validate_for(question.id, question.scope_id)?;
        Ok(Self { decision })
    }
}

/// Why a question is no longer valid, excluding any reason of avoidance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObsolescenceReasonCode {
    /// The question depended on a false premise.
    FalsePremise,
    /// A technology change invalidated the question.
    TechnologyChanged,
    /// The governing context was superseded.
    ContextSuperseded,
    /// The source that made the question meaningful was retracted.
    SourceRetracted,
}

/// Typed reason and evidence needed for `OBSOLETE`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuestionObsolescence {
    reason: ObsolescenceReasonCode,
    evidence_ids: Vec<EvidenceId>,
}

impl QuestionObsolescence {
    /// Constructs an evidenced obsolescence record.
    pub fn new(
        reason: ObsolescenceReasonCode,
        evidence_ids: impl IntoIterator<Item = EvidenceId>,
    ) -> Result<Self, QuestionError> {
        let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
        if evidence_ids.is_empty() {
            return Err(QuestionError::ObsolescenceEvidenceMissing);
        }
        Ok(Self {
            reason,
            evidence_ids,
        })
    }

    fn validate(&self) -> Result<(), QuestionError> {
        if self.evidence_ids.is_empty() {
            Err(QuestionError::ObsolescenceEvidenceMissing)
        } else {
            Ok(())
        }
    }

    /// Returns the reason code.
    #[must_use]
    pub const fn reason(&self) -> ObsolescenceReasonCode {
        self.reason
    }

    /// Returns the supporting evidence identities.
    #[must_use]
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

/// Reasons a user may defer a still-valid question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionDeferralReason {
    /// The user does not want to answer now.
    NotNow,
    /// The user lacks attention for the question now.
    AttentionUnavailable,
    /// The user deliberately deferred work to a later context.
    DeferredToLaterContext,
}

/// A deferral that leaves question validity and lifecycle status unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestionDeferral {
    reason: QuestionDeferralReason,
    deferred_at: TimestampMillis,
}

impl QuestionDeferral {
    /// Records avoidance or deferral without manufacturing obsolescence.
    #[must_use]
    pub const fn new(reason: QuestionDeferralReason, deferred_at: TimestampMillis) -> Self {
        Self {
            reason,
            deferred_at,
        }
    }

    /// Returns the deferral reason.
    #[must_use]
    pub const fn reason(self) -> QuestionDeferralReason {
        self.reason
    }

    /// Returns when the deferral was recorded.
    #[must_use]
    pub const fn deferred_at(self) -> TimestampMillis {
        self.deferred_at
    }
}

/// A time-bearing question aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    id: EntityId,
    scope_id: ScopeId,
    canonical_text: String,
    created_at: TimestampMillis,
    origin: QuestionOrigin,
    related_concept_claims: Vec<EntityId>,
    status: QuestionStatus,
    importance: QuestionImportance,
    revisions: Vec<QuestionRevision>,
    lifecycle: Vec<QuestionLifecycleEvent>,
    resolution_decision: Option<QuestionResolutionDecision>,
    obsolescence: Option<QuestionObsolescence>,
    reframed_as: Option<EntityId>,
}

impl<'de> Deserialize<'de> for Question {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "camelCase")]
        struct WireQuestion {
            id: EntityId,
            scope_id: ScopeId,
            canonical_text: String,
            created_at: TimestampMillis,
            origin: QuestionOrigin,
            related_concept_claims: Vec<EntityId>,
            status: QuestionStatus,
            importance: QuestionImportance,
            revisions: Vec<QuestionRevision>,
            lifecycle: Vec<QuestionLifecycleEvent>,
            resolution_decision: Option<QuestionResolutionDecision>,
            obsolescence: Option<QuestionObsolescence>,
            reframed_as: Option<EntityId>,
        }

        let value = WireQuestion::deserialize(deserializer)?;
        let question = Self {
            id: value.id,
            scope_id: value.scope_id,
            canonical_text: value.canonical_text,
            created_at: value.created_at,
            origin: value.origin,
            related_concept_claims: value.related_concept_claims,
            status: value.status,
            importance: value.importance,
            revisions: value.revisions,
            lifecycle: value.lifecycle,
            resolution_decision: value.resolution_decision,
            obsolescence: value.obsolescence,
            reframed_as: value.reframed_as,
        };
        question.validate().map_err(de::Error::custom)?;
        Ok(question)
    }
}

impl Question {
    /// Constructs a new open question at an exact origin and instant.
    pub fn new(
        id: EntityId,
        scope_id: ScopeId,
        canonical_text: impl Into<String>,
        created_at: TimestampMillis,
        origin: QuestionOrigin,
        related_concept_claims: impl IntoIterator<Item = EntityId>,
        importance: QuestionImportance,
    ) -> Result<Self, QuestionError> {
        Ok(Self {
            id,
            scope_id,
            canonical_text: require_text(canonical_text, "question canonical text")?,
            created_at,
            origin,
            related_concept_claims: related_concept_claims.into_iter().collect(),
            status: QuestionStatus::Open,
            importance,
            revisions: Vec::new(),
            lifecycle: Vec::new(),
            resolution_decision: None,
            obsolescence: None,
            reframed_as: None,
        })
    }

    /// Returns the stable graph identity.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        self.id
    }

    /// Returns the resolution scope.
    #[must_use]
    pub const fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    /// Returns the current wording.
    #[must_use]
    pub fn canonical_text(&self) -> &str {
        &self.canonical_text
    }

    /// Returns the immutable creation instant.
    #[must_use]
    pub const fn created_at(&self) -> TimestampMillis {
        self.created_at
    }

    /// Returns the exact origin coordinate.
    #[must_use]
    pub const fn origin(&self) -> &QuestionOrigin {
        &self.origin
    }

    /// Returns related concept-claim identities.
    #[must_use]
    pub fn related_concept_claims(&self) -> &[EntityId] {
        &self.related_concept_claims
    }

    /// Returns the current lifecycle status.
    #[must_use]
    pub const fn status(&self) -> QuestionStatus {
        self.status
    }

    /// Returns append-only wording revisions.
    #[must_use]
    pub fn revisions(&self) -> &[QuestionRevision] {
        &self.revisions
    }

    /// Returns append-only lifecycle events.
    #[must_use]
    pub fn lifecycle(&self) -> &[QuestionLifecycleEvent] {
        &self.lifecycle
    }

    /// Appends a wording revision while retaining the previous text.
    pub fn revise(
        mut self,
        replacement_text: impl Into<String>,
        revised_at: TimestampMillis,
    ) -> Result<Self, QuestionError> {
        self.validate()?;
        let replacement_text = require_text(replacement_text, "question replacement text")?;
        if revised_at < self.latest_time() {
            return Err(QuestionError::NonMonotonicTime);
        }
        self.revisions.push(QuestionRevision {
            previous_text: self.canonical_text,
            replacement_text: replacement_text.clone(),
            revised_at,
        });
        self.canonical_text = replacement_text;
        Ok(self)
    }

    /// Records a supported partial resolution.
    pub fn partially_resolve(
        self,
        occurred_at: TimestampMillis,
        evidence_ids: impl IntoIterator<Item = EvidenceId>,
    ) -> Result<Self, QuestionError> {
        let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
        if evidence_ids.is_empty() {
            return Err(QuestionError::PartialResolutionEvidenceMissing);
        }
        self.transitioned(QuestionStatus::PartiallyResolved, occurred_at, evidence_ids)
    }

    /// Resolves a partial question with previously verified user authority.
    pub fn resolve(
        self,
        resolution: VerifiedQuestionResolution,
        occurred_at: TimestampMillis,
    ) -> Result<Self, QuestionError> {
        resolution.decision.validate_for(self.id, self.scope_id)?;
        let mut resolved = self.transitioned(QuestionStatus::Resolved, occurred_at, Vec::new())?;
        resolved.resolution_decision = Some(resolution.decision);
        resolved.validate()?;
        Ok(resolved)
    }

    /// Marks a still-valid resolved question as reopened.
    pub fn reopen(self, occurred_at: TimestampMillis) -> Result<Self, QuestionError> {
        self.transitioned(QuestionStatus::Reopened, occurred_at, Vec::new())
    }

    /// Marks an open question obsolete with typed reason and evidence.
    ///
    /// Avoidance is represented by [`QuestionDeferral`], which is not accepted
    /// by this method:
    ///
    /// ```compile_fail
    /// # use academic_domain::{EntityId, ScopeId, TimestampMillis};
    /// # use academic_domain::question::{Question, QuestionDeferral};
    /// fn avoidance_is_not_obsolescence(question: Question, deferral: QuestionDeferral) {
    ///     let _ = question.mark_obsolete(deferral, TimestampMillis::new(1));
    /// }
    /// ```
    pub fn mark_obsolete(
        self,
        obsolescence: QuestionObsolescence,
        occurred_at: TimestampMillis,
    ) -> Result<Self, QuestionError> {
        obsolescence.validate()?;
        let evidence_ids = obsolescence.evidence_ids.clone();
        let mut obsolete =
            self.transitioned(QuestionStatus::Obsolete, occurred_at, evidence_ids)?;
        obsolete.obsolescence = Some(obsolescence);
        obsolete.validate()?;
        Ok(obsolete)
    }

    /// Creates a new open question and links the preserved original to it.
    pub fn reframe(
        self,
        replacement_id: EntityId,
        replacement_text: impl Into<String>,
        occurred_at: TimestampMillis,
    ) -> Result<QuestionReframe, QuestionError> {
        if replacement_id == self.id {
            return Err(QuestionError::ReframeIdentityReused);
        }
        let replacement = Self::new(
            replacement_id,
            self.scope_id,
            replacement_text,
            occurred_at,
            self.origin.clone(),
            self.related_concept_claims.clone(),
            self.importance,
        )?;
        let original_id = self.id;
        let mut original = self.transitioned(QuestionStatus::Reframed, occurred_at, Vec::new())?;
        original.reframed_as = Some(replacement_id);
        original.validate()?;
        Ok(QuestionReframe {
            original,
            replacement,
            relation: QuestionRelation {
                from: original_id,
                to: replacement_id,
                kind: QuestionRelationKind::ReframedAs,
                created_at: occurred_at,
            },
        })
    }

    /// Creates an AI resolution candidate without changing this question.
    pub fn propose_resolution(
        &self,
        explanation: GeneratedExplanation,
        evidence_ids: impl IntoIterator<Item = EvidenceId>,
    ) -> Result<ResolutionCandidate, QuestionError> {
        if explanation.question_id != self.id {
            return Err(QuestionError::AiExplanationQuestionMismatch);
        }
        Ok(ResolutionCandidate {
            question_id: self.id,
            explanation,
            evidence_ids: evidence_ids.into_iter().collect(),
        })
    }

    fn transitioned(
        mut self,
        to: QuestionStatus,
        occurred_at: TimestampMillis,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, QuestionError> {
        self.validate()?;
        if !lifecycle_transition_is_allowed(self.status, to) {
            return Err(QuestionError::TransitionNotAllowed {
                from: self.status,
                to,
            });
        }
        if occurred_at < self.latest_time() {
            return Err(QuestionError::NonMonotonicTime);
        }
        self.lifecycle.push(QuestionLifecycleEvent {
            from: self.status,
            to,
            occurred_at,
            evidence_ids,
        });
        self.status = to;
        Ok(self)
    }

    fn latest_time(&self) -> TimestampMillis {
        self.revisions
            .iter()
            .map(QuestionRevision::revised_at)
            .chain(
                self.lifecycle
                    .iter()
                    .map(QuestionLifecycleEvent::occurred_at),
            )
            .max()
            .unwrap_or(self.created_at)
    }

    fn validate(&self) -> Result<(), QuestionError> {
        require_text(&self.canonical_text, "question canonical text")?;
        let mut prior_text = None;
        let mut revision_time = self.created_at;
        for revision in &self.revisions {
            require_text(&revision.previous_text, "question revision previous text")?;
            require_text(
                &revision.replacement_text,
                "question revision replacement text",
            )?;
            if revision.revised_at < revision_time {
                return Err(QuestionError::NonMonotonicTime);
            }
            if prior_text
                .as_ref()
                .is_some_and(|text| text != &revision.previous_text)
            {
                return Err(QuestionError::InvalidStatusMetadata);
            }
            prior_text = Some(revision.replacement_text.clone());
            revision_time = revision.revised_at;
        }
        if prior_text
            .as_ref()
            .is_some_and(|text| text != &self.canonical_text)
        {
            return Err(QuestionError::InvalidStatusMetadata);
        }

        let mut current = QuestionStatus::Open;
        let mut lifecycle_time = self.created_at;
        for event in &self.lifecycle {
            if event.from != current
                || !lifecycle_transition_is_allowed(event.from, event.to)
                || event.occurred_at < lifecycle_time
            {
                return Err(QuestionError::InvalidLifecycleDefinition);
            }
            if event.to == QuestionStatus::PartiallyResolved && event.evidence_ids.is_empty() {
                return Err(QuestionError::PartialResolutionEvidenceMissing);
            }
            current = event.to;
            lifecycle_time = event.occurred_at;
        }
        if current != self.status {
            return Err(QuestionError::InvalidStatusMetadata);
        }
        if let Some(decision) = &self.resolution_decision {
            decision.validate_for(self.id, self.scope_id)?;
        }
        if self.status == QuestionStatus::Resolved && self.resolution_decision.is_none() {
            return Err(QuestionError::InvalidStatusMetadata);
        }
        match (self.status, &self.obsolescence) {
            (QuestionStatus::Obsolete, Some(record)) => record.validate()?,
            (QuestionStatus::Obsolete, None) | (_, Some(_)) => {
                return Err(QuestionError::InvalidStatusMetadata);
            }
            (_, None) => {}
        }
        match (self.status, self.reframed_as) {
            (QuestionStatus::Reframed, Some(target)) if target != self.id => {}
            (QuestionStatus::Reframed, _) | (_, Some(_)) => {
                return Err(QuestionError::InvalidStatusMetadata);
            }
            (_, None) => {}
        }
        Ok(())
    }
}

/// Question-to-question relation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionRelationKind {
    /// The old wording was preserved and a new question was created.
    ReframedAs,
}

/// A typed question graph edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuestionRelation {
    from: EntityId,
    to: EntityId,
    kind: QuestionRelationKind,
    created_at: TimestampMillis,
}

impl QuestionRelation {
    /// Returns the source identity.
    #[must_use]
    pub const fn from(&self) -> EntityId {
        self.from
    }

    /// Returns the target identity.
    #[must_use]
    pub const fn to(&self) -> EntityId {
        self.to
    }

    /// Returns the relation kind.
    #[must_use]
    pub const fn kind(&self) -> QuestionRelationKind {
        self.kind
    }
}

/// Output of an append-only reframe operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionReframe {
    original: Question,
    replacement: Question,
    relation: QuestionRelation,
}

impl QuestionReframe {
    /// Returns the preserved original question in `REFRAMED` state.
    #[must_use]
    pub const fn original(&self) -> &Question {
        &self.original
    }

    /// Returns the newly created open question.
    #[must_use]
    pub const fn replacement(&self) -> &Question {
        &self.replacement
    }

    /// Returns the `REFRAMED_AS` edge.
    #[must_use]
    pub const fn relation(&self) -> &QuestionRelation {
        &self.relation
    }
}

/// A model-generated artifact kept distinct from source material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GeneratedExplanation {
    artifact_id: ArtifactId,
    question_id: EntityId,
    model_run_id: EntityId,
    created_at: TimestampMillis,
}

impl GeneratedExplanation {
    /// Records the generated artifact and the question it discusses.
    #[must_use]
    pub const fn new(
        artifact_id: ArtifactId,
        question_id: EntityId,
        model_run_id: EntityId,
        created_at: TimestampMillis,
    ) -> Self {
        Self {
            artifact_id,
            question_id,
            model_run_id,
            created_at,
        }
    }

    /// Returns its artifact identity.
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }

    /// Tests whether an evidence item points at this generated artifact.
    #[must_use]
    pub fn is_resolution_evidence(&self, evidence: &EvidenceItem) -> bool {
        evidence.artifact_id == self.artifact_id && evidence.validate().is_ok()
    }
}

/// AI-proposed material that does not carry lifecycle authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionCandidate {
    question_id: EntityId,
    explanation: GeneratedExplanation,
    evidence_ids: Vec<EvidenceId>,
}

impl ResolutionCandidate {
    /// Returns the question this candidate discusses.
    #[must_use]
    pub const fn question_id(&self) -> EntityId {
        self.question_id
    }

    /// Returns the distinct generated artifact.
    #[must_use]
    pub const fn explanation(&self) -> &GeneratedExplanation {
        &self.explanation
    }

    /// Returns additional evidence references proposed by AI.
    #[must_use]
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

/// The six regions, in their required default order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionWorkspaceRegion {
    OriginContext,
    ConceptsAndPrerequisites,
    RelevantEvidence,
    RecurrenceLocations,
    ResolutionSources,
    AiExplanation,
}

/// Exact question-workspace region order from section 14.3.
pub const QUESTION_WORKSPACE_REGION_ORDER: [QuestionWorkspaceRegion; 6] = [
    QuestionWorkspaceRegion::OriginContext,
    QuestionWorkspaceRegion::ConceptsAndPrerequisites,
    QuestionWorkspaceRegion::RelevantEvidence,
    QuestionWorkspaceRegion::RecurrenceLocations,
    QuestionWorkspaceRegion::ResolutionSources,
    QuestionWorkspaceRegion::AiExplanation,
];

/// Explicit display preference for the optional AI explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiExplanationPreference {
    Hidden,
    Requested,
}

/// Ordered question workspace with opt-in AI content in region six.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionWorkspace {
    question_id: EntityId,
    preference: AiExplanationPreference,
    ai_explanation: Option<GeneratedExplanation>,
}

impl QuestionWorkspace {
    /// Builds a workspace and enforces the explanation opt-in boundary.
    pub fn new(
        question_id: EntityId,
        preference: AiExplanationPreference,
        ai_explanation: Option<GeneratedExplanation>,
    ) -> Result<Self, QuestionError> {
        if preference == AiExplanationPreference::Hidden && ai_explanation.is_some() {
            return Err(QuestionError::AiExplanationNotRequested);
        }
        if ai_explanation
            .as_ref()
            .is_some_and(|explanation| explanation.question_id != question_id)
        {
            return Err(QuestionError::AiExplanationQuestionMismatch);
        }
        Ok(Self {
            question_id,
            preference,
            ai_explanation,
        })
    }

    /// Returns the exact default region ordering.
    #[must_use]
    pub const fn regions(&self) -> &[QuestionWorkspaceRegion; 6] {
        &QUESTION_WORKSPACE_REGION_ORDER
    }

    /// Returns explanation content only in the explicitly requested state.
    #[must_use]
    pub fn ai_explanation(&self) -> Option<&GeneratedExplanation> {
        match self.preference {
            AiExplanationPreference::Hidden => None,
            AiExplanationPreference::Requested => self.ai_explanation.as_ref(),
        }
    }

    /// Returns the workspace's question identity.
    #[must_use]
    pub const fn question_id(&self) -> EntityId {
        self.question_id
    }
}

// QUESTION_GROWTH_SCHEMA_BEGIN
/// How far the question's subject reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionTargetScope {
    Term,
    Mechanism,
    TradeOff,
    SystemBoundary,
}

/// Which prerequisite layer the question reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionPrerequisiteDepth {
    BasicDefinition,
    Interaction,
    FailureMode,
}

/// The comparison structure visible in the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionComparisonQuality {
    WhatIsIt,
    WhyAInsteadOfB,
}

/// Which operating assumptions are explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionConditionSpecificity {
    Unconditional,
    WorkloadAssumptions,
    FailureAssumptions,
}

/// Which concrete evidence kind initiated the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionEvidenceUse {
    Abstract,
    Code,
    Trace,
    Experiment,
}

/// Categorical growth description plus separately evidenced reuse breadth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuestionGrowthDescriptors {
    pub target_scope: QuestionTargetScope,
    pub prerequisite_depth: QuestionPrerequisiteDepth,
    pub comparison_quality: QuestionComparisonQuality,
    pub condition_specificity: QuestionConditionSpecificity,
    pub evidence_use: QuestionEvidenceUse,
    pub reuse: ReuseSummary,
    pub evidence_ids: Vec<EvidenceId>,
}
// QUESTION_GROWTH_SCHEMA_END

/// A destination to which one answer was transferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReuseTarget {
    Project { id: EntityId },
    Concept { id: EntityId },
}

/// Why a possible transfer is not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UncountedReuseReason {
    TargetIdentityMissing,
    TargetKindUnresolved,
}

/// One observed answer transfer, identified or conservatively uncounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReuseObservation {
    Identified(ReuseTarget),
    Uncounted(UncountedReuseReason),
}

/// Deduplicated destination breadth and surfaced default-deny exclusions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReuseSummary {
    targets: BTreeSet<ReuseTarget>,
    uncounted_reasons: Vec<UncountedReuseReason>,
}

impl ReuseSummary {
    /// Deduplicates transfers by their typed project or concept destination.
    #[must_use]
    pub fn from_observations(observations: impl IntoIterator<Item = ReuseObservation>) -> Self {
        let mut targets = BTreeSet::new();
        let mut uncounted_reasons = Vec::new();
        for observation in observations {
            match observation {
                ReuseObservation::Identified(target) => {
                    targets.insert(target);
                }
                ReuseObservation::Uncounted(reason) => uncounted_reasons.push(reason),
            }
        }
        Self {
            targets,
            uncounted_reasons,
        }
    }

    /// Returns the number of distinct identified destinations.
    #[must_use]
    pub fn reuse_count(&self) -> usize {
        self.targets.len()
    }

    /// Returns the exact distinct destinations included in the count.
    #[must_use]
    pub const fn targets(&self) -> &BTreeSet<ReuseTarget> {
        &self.targets
    }

    /// Returns exclusions whose destination identity or kind was unresolved.
    #[must_use]
    pub fn uncounted_reasons(&self) -> &[UncountedReuseReason] {
        &self.uncounted_reasons
    }
}
