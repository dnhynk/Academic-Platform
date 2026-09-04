//! Section 15.1's `GapCase`, its root candidates, and the diagnostic step 5
//! attaches when there is more than one.
//!
//! ## Nothing here picks a winner
//!
//! Step 5 is `root 후보가 여러 개면 모두 유지하고 짧은 diagnostic activity를
//! 제안한다`. [`GapCase`] holds every candidate the descent found and
//! [`GapCase::roots`] returns **every** candidate at the shallowest depth,
//! returning two when two tie. There is no `best`, no `primary` and no `first`
//! accessor that would let a caller take one and drop the other.
//!
//! This repository has drawn that line three times already — `P2-R4`'s
//! `ClassificationConflict`, `P2-R3`'s `ImplementationDrift` and `P2-N2`'s
//! conflict card — and each time for the reason that applies here: a tie is
//! information about the evidence, and resolving it by rule discards that
//! information silently. So the tie carries a [`TieDiagnostic`] instead, whose
//! activity shape is section 15.2's own `사용자 확인 또는 diagnostic` and whose
//! question, when one is attached, is a `P2-N4` question this engine may
//! reference and may never resolve.
//!
//! ## The order is total and it never drops a candidate
//!
//! [`GapCase::candidates`] is sorted by depth, then by confidence descending,
//! then by identifier — the last only so that two candidates alike in every
//! respect still print in a stable order. Sorting is presentation; the set is
//! never trimmed by it.

use academic_domain::{
    ConfidencePermille, EntityId, EvidenceId, ScopeId, question::QuestionStatus,
};
use serde::{Deserialize, Serialize};

use crate::{
    GapError,
    explanation::{GapExplanation, MinimumRemediation, RemediationActivity},
    kind::GapKind,
    path::{AncestorImpact, BlockingPath},
    state::StateSnapshot,
};

/// Every root of a candidate list: the strong-deficit candidates at the
/// shallowest depth one was found, and every one of them that ties on
/// confidence there.
///
/// One implementation, used by [`GapCase::roots`] and by
/// [`crate::engine::search`], so the tie the engine detects and the tie the case
/// validates are the same tie.
#[must_use]
pub fn roots_of(candidates: &[RootCandidate]) -> Vec<&RootCandidate> {
    let strong: Vec<&RootCandidate> = candidates
        .iter()
        .filter(|candidate| candidate.is_strong_deficit())
        .collect();
    let Some(shallowest) = strong.iter().map(|candidate| candidate.depth()).min() else {
        return Vec::new();
    };
    let at_depth: Vec<&RootCandidate> = strong
        .into_iter()
        .filter(|candidate| candidate.depth() == shallowest)
        .collect();
    let Some(best) = at_depth
        .iter()
        .map(|candidate| candidate.confidence().value())
        .max()
    else {
        return Vec::new();
    };
    at_depth
        .into_iter()
        .filter(|candidate| candidate.confidence().value() == best)
        .collect()
}

/// One root candidate. Section 15.1's `rootCandidates` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootCandidate {
    concept: EntityId,
    kind: GapKind,
    blocking_path: BlockingPath,
    reason: String,
    evidence: Vec<EvidenceId>,
    confidence: ConfidencePermille,
    ancestor_impact: Vec<AncestorImpact>,
    explanation: GapExplanation,
}

impl RootCandidate {
    /// Records one candidate.
    ///
    /// # Errors
    ///
    /// [`GapError::CandidateExplainsAnotherConcept`] when the explanation is
    /// about a different concept or a different kind, and
    /// [`GapError::CandidateReasonMissing`] when the `reason` cell is blank.
    pub fn of(
        kind: GapKind,
        blocking_path: BlockingPath,
        reason: &str,
        evidence: Vec<EvidenceId>,
        confidence: ConfidencePermille,
        ancestor_impact: Vec<AncestorImpact>,
        explanation: GapExplanation,
    ) -> Result<Self, GapError> {
        let concept = blocking_path.tip();
        if explanation.subject() != concept || explanation.kind() != kind {
            return Err(GapError::CandidateExplainsAnotherConcept);
        }
        if reason.trim().is_empty() {
            return Err(GapError::CandidateReasonMissing);
        }
        Ok(Self {
            concept,
            kind,
            blocking_path,
            reason: reason.to_owned(),
            evidence,
            confidence,
            ancestor_impact,
            explanation,
        })
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Which of section 15.2's five kinds.
    #[must_use]
    pub const fn kind(&self) -> GapKind {
        self.kind
    }

    /// Section 15.1's `blockingPath`.
    #[must_use]
    pub const fn blocking_path(&self) -> &BlockingPath {
        &self.blocking_path
    }

    /// How many hops below the surface concept it sits.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.blocking_path.depth()
    }

    /// Section 15.1's `reason`.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Section 15.1's `evidence`.
    #[must_use]
    pub fn evidence(&self) -> &[EvidenceId] {
        &self.evidence
    }

    /// Section 15.1's `confidence`.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }

    /// Section 15.2 step 4's `조상 영향도`.
    #[must_use]
    pub fn ancestor_impact(&self) -> &[AncestorImpact] {
        &self.ancestor_impact
    }

    /// Section 15.3's eight fields.
    #[must_use]
    pub const fn explanation(&self) -> &GapExplanation {
        &self.explanation
    }

    /// Whether this candidate is section 15.2 step 4's `강한 부족`.
    #[must_use]
    pub const fn is_strong_deficit(&self) -> bool {
        self.kind.is_strong_deficit()
    }
}

/// Section 15.2 step 5's `짧은 diagnostic activity`, attached when more than one
/// candidate is a root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TieDiagnostic {
    tied: Vec<EntityId>,
    activity: MinimumRemediation,
    question: Option<EntityId>,
    question_status: Option<QuestionStatus>,
}

impl TieDiagnostic {
    /// Proposes one diagnostic over the tied candidates.
    ///
    /// # Errors
    ///
    /// [`GapError::DiagnosticNeedsTwoCandidates`] for fewer than two;
    /// [`GapError::DiagnosticIsNotADiagnostic`] when the activity is not section
    /// 15.2's `사용자 확인 또는 diagnostic`; and
    /// [`GapError::DiagnosticQuestionIsNotOpen`] when a referenced `P2-N4`
    /// question is in any state but `OPEN` or `REOPENED` — a diagnostic that
    /// pointed at a resolved question would be proposing work the user has
    /// already finished, and this engine has no way to reopen one.
    pub fn of(
        tied: Vec<EntityId>,
        activity: MinimumRemediation,
        question: Option<(EntityId, QuestionStatus)>,
    ) -> Result<Self, GapError> {
        if tied.len() < 2 {
            return Err(GapError::DiagnosticNeedsTwoCandidates);
        }
        if activity.activity() != RemediationActivity::UserConfirmationOrDiagnostic {
            return Err(GapError::DiagnosticIsNotADiagnostic);
        }
        if let Some((_, status)) = question
            && !matches!(status, QuestionStatus::Open | QuestionStatus::Reopened)
        {
            return Err(GapError::DiagnosticQuestionIsNotOpen);
        }
        Ok(Self {
            tied,
            activity,
            question: question.map(|(id, _)| id),
            question_status: question.map(|(_, status)| status),
        })
    }

    /// The candidates the diagnostic separates, in identifier order.
    #[must_use]
    pub fn tied(&self) -> &[EntityId] {
        &self.tied
    }

    /// The activity itself.
    #[must_use]
    pub const fn activity(&self) -> &MinimumRemediation {
        &self.activity
    }

    /// The `P2-N4` question this diagnostic answers, when one is attached.
    ///
    /// Referenced and never resolved: `P2-N4`'s `resolution_requires_user_decision`
    /// makes `RESOLVED` a user decision, and nothing in this crate constructs a
    /// question, a lifecycle event or a resolution.
    #[must_use]
    pub const fn question(&self) -> Option<EntityId> {
        self.question
    }

    /// That question's status at the time the case was built.
    #[must_use]
    pub const fn question_status(&self) -> Option<QuestionStatus> {
        self.question_status
    }
}

/// Section 15.1's `GapCase`.
///
/// `Deserialize` goes through [`GapCaseWire`], which re-runs every validation
/// the constructor runs, so a hand-written document cannot introduce a case with
/// no candidate, a tie with no diagnostic, or an explanation about another
/// concept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "GapCaseWire", into = "GapCaseWire")]
pub struct GapCase {
    goal: EntityId,
    scope: ScopeId,
    surface_concept: EntityId,
    candidates: Vec<RootCandidate>,
    user_state_snapshot: Vec<StateSnapshot>,
    diagnostic: Option<TieDiagnostic>,
}

/// The wire shape of a [`GapCase`], with section 15.1's own field names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapCaseWire {
    /// Section 15.1's `goal`.
    pub goal: EntityId,
    /// The resolution scope the goal was declared under.
    pub scope: ScopeId,
    /// Section 15.1's `surfaceConcept`.
    #[serde(rename = "surfaceConcept")]
    pub surface_concept: EntityId,
    /// Section 15.1's `rootCandidates`.
    #[serde(rename = "rootCandidates")]
    pub root_candidates: Vec<RootCandidate>,
    /// Section 15.1's `userStateSnapshot`, one entry per concept the descent
    /// overlaid.
    #[serde(rename = "userStateSnapshot")]
    pub user_state_snapshot: Vec<StateSnapshot>,
    /// Section 15.2 step 5's diagnostic, when the roots tie.
    pub diagnostic: Option<TieDiagnostic>,
}

impl GapCase {
    /// Assembles one case.
    ///
    /// # Errors
    ///
    /// [`GapError::CaseHasNoCandidate`] for an empty candidate list;
    /// [`GapError::CandidateLeavesSurface`] when a candidate's path starts
    /// somewhere other than the surface concept;
    /// [`GapError::TiedRootsNeedADiagnostic`] when more than one candidate is a
    /// root and no diagnostic is attached; and
    /// [`GapError::DiagnosticDoesNotNameTheRoots`] when one is attached whose
    /// tied set is not exactly the roots.
    pub fn of(
        goal: EntityId,
        scope: ScopeId,
        surface_concept: EntityId,
        mut candidates: Vec<RootCandidate>,
        user_state_snapshot: Vec<StateSnapshot>,
        diagnostic: Option<TieDiagnostic>,
    ) -> Result<Self, GapError> {
        if candidates.is_empty() {
            return Err(GapError::CaseHasNoCandidate);
        }
        for candidate in &candidates {
            if candidate.blocking_path().surface() != surface_concept {
                return Err(GapError::CandidateLeavesSurface);
            }
        }
        candidates.sort_by(|left, right| {
            left.depth()
                .cmp(&right.depth())
                .then(right.confidence().value().cmp(&left.confidence().value()))
                .then(left.concept().as_uuid().cmp(&right.concept().as_uuid()))
        });
        let value = Self {
            goal,
            scope,
            surface_concept,
            candidates,
            user_state_snapshot,
            diagnostic,
        };
        let roots = value.roots();
        match (roots.len(), value.diagnostic.as_ref()) {
            (0 | 1, None) => Ok(value),
            (0 | 1, Some(_)) => Err(GapError::DiagnosticDoesNotNameTheRoots),
            (_, None) => Err(GapError::TiedRootsNeedADiagnostic),
            (_, Some(attached)) => {
                let mut named: Vec<EntityId> = roots.iter().map(|root| root.concept()).collect();
                named.sort_by_key(|id| id.as_uuid());
                let mut tied = attached.tied().to_vec();
                tied.sort_by_key(|id| id.as_uuid());
                if named == tied {
                    Ok(value)
                } else {
                    Err(GapError::DiagnosticDoesNotNameTheRoots)
                }
            }
        }
    }

    /// Section 15.1's `goal`.
    #[must_use]
    pub const fn goal(&self) -> EntityId {
        self.goal
    }

    /// The resolution scope.
    #[must_use]
    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Section 15.1's `surfaceConcept`.
    #[must_use]
    pub const fn surface_concept(&self) -> EntityId {
        self.surface_concept
    }

    /// Every candidate the descent found, shallowest and most confident first.
    #[must_use]
    pub fn candidates(&self) -> &[RootCandidate] {
        &self.candidates
    }

    /// The roots: every strong-deficit candidate at the shallowest depth a
    /// strong deficit was found, and every one of them when several tie.
    ///
    /// Section 15.2 step 4 is `표면 concept에서 아래로 내려가며 최초의 강한
    /// 부족`, so a root is the *first* strong deficit going down. When two sit at
    /// that same depth with the same confidence, both are returned; nothing here
    /// chooses.
    #[must_use]
    pub fn roots(&self) -> Vec<&RootCandidate> {
        roots_of(&self.candidates)
    }

    /// Section 15.1's `userStateSnapshot`.
    #[must_use]
    pub fn user_state_snapshot(&self) -> &[StateSnapshot] {
        &self.user_state_snapshot
    }

    /// Section 15.2 step 5's diagnostic, when the roots tie.
    #[must_use]
    pub const fn diagnostic(&self) -> Option<&TieDiagnostic> {
        self.diagnostic.as_ref()
    }
}

impl TryFrom<GapCaseWire> for GapCase {
    type Error = GapError;

    fn try_from(wire: GapCaseWire) -> Result<Self, Self::Error> {
        Self::of(
            wire.goal,
            wire.scope,
            wire.surface_concept,
            wire.root_candidates,
            wire.user_state_snapshot,
            wire.diagnostic,
        )
    }
}

impl From<GapCase> for GapCaseWire {
    fn from(value: GapCase) -> Self {
        Self {
            goal: value.goal,
            scope: value.scope,
            surface_concept: value.surface_concept,
            root_candidates: value.candidates,
            user_state_snapshot: value.user_state_snapshot,
            diagnostic: value.diagnostic,
        }
    }
}
