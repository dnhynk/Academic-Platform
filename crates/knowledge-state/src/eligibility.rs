//! Section 13.4's four deterministic checks, and the value that only exists
//! when all four passed.
//!
//! ```text
//! deterministic eligibility checks
//!   ├─ exact concept linked?
//!   ├─ user authorship/participation known?
//!   ├─ outcome known?
//!   └─ source integrity valid?
//! ```
//!
//! `eligibility_four_checks_block_with_reason_codes` reads those four lines out
//! of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares them
//! against [`EligibilityCheck::ALL`] in both directions, so *four* is a
//! measurement of the design document rather than a number in a test.
//!
//! ## Every check is a question about knowing, so `UNKNOWN` blocks
//!
//! Three of the four lines end in `known?` and the fourth in `valid?`. An
//! absent answer is therefore not a neutral state to be filled in later: it is
//! the answer *no*, and each of the four answer types has an explicit `Unknown`
//! variant that blocks with its own reason code. This is section 3's rule for
//! the student profile — `알 수 없는 필드는 빈 문자열이 아니라 UNKNOWN으로
//! 저장한다` — applied to evidence.
//!
//! ## A blocked item is not a silent one
//!
//! [`EligibilityOutcome::Blocked`] carries **every** failing check, not the
//! first. A dossier that fails three checks reports three codes, because
//! `REQ-13-036`'s executable row is *한 조건씩 false → corresponding reason code
//! and promotion block* and a reader repairing evidence needs the whole list.
//!
//! ## Then the proof is by value
//!
//! [`EligibleEvidence`] has private fields and one constructor, and that
//! constructor is [`EligibilityOutcome::admit`]. Nothing else in this crate can
//! build one, and the projection takes `&[EligibleEvidence]`. So evidence that
//! failed a check is not evidence a later layer must remember to filter — it is
//! evidence that has no value of the type the projection accepts.

use academic_domain::{ContentDigest, EntityId, EvidenceId, entity_registry::EntityKind};
use serde::{Deserialize, Serialize};

use crate::evidence::{ConceptEvidence, EvidenceKind};

/// Section 13.4's four checks, in the diagram's own order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EligibilityCheck {
    /// `exact concept linked?`
    ExactConceptLink,
    /// `user authorship/participation known?`
    AuthorshipOrParticipation,
    /// `outcome known?`
    Outcome,
    /// `source integrity valid?`
    SourceIntegrity,
}

impl EligibilityCheck {
    /// Exhaustive order, in the diagram's own order.
    pub const ALL: [Self; 4] = [
        Self::ExactConceptLink,
        Self::AuthorshipOrParticipation,
        Self::Outcome,
        Self::SourceIntegrity,
    ];

    /// The design document's own line for this check, verbatim.
    #[must_use]
    pub const fn question(self) -> &'static str {
        match self {
            Self::ExactConceptLink => "exact concept linked?",
            Self::AuthorshipOrParticipation => "user authorship/participation known?",
            Self::Outcome => "outcome known?",
            Self::SourceIntegrity => "source integrity valid?",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactConceptLink => "EXACT_CONCEPT_LINK",
            Self::AuthorshipOrParticipation => "AUTHORSHIP_OR_PARTICIPATION",
            Self::Outcome => "OUTCOME",
            Self::SourceIntegrity => "SOURCE_INTEGRITY",
        }
    }
}

/// Why one check refused.
///
/// Two codes per check and no shared code, so a reader is told which of the two
/// repairs applies: supply the missing answer, or correct a wrong one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EligibilityReasonCode {
    /// No concept was linked at all.
    ConceptLinkAbsent,
    /// A concept was named but the ontology could not resolve it exactly.
    ConceptLinkAmbiguous,
    /// The link named a section 7.4 tier that carries no personal state.
    ConceptLinkTierNotLearnable,
    /// Authorship or participation was not recorded.
    AuthorshipUnknown,
    /// The work is recorded as somebody else's.
    AuthorshipThirdParty,
    /// The outcome was not recorded.
    OutcomeUnknown,
    /// The source could not be verified.
    SourceIntegrityUnknown,
    /// The source was verified and did not match.
    SourceIntegrityBroken,
}

impl EligibilityReasonCode {
    /// Exhaustive order, grouped by the check that raises it.
    pub const ALL: [Self; 8] = [
        Self::ConceptLinkAbsent,
        Self::ConceptLinkAmbiguous,
        Self::ConceptLinkTierNotLearnable,
        Self::AuthorshipUnknown,
        Self::AuthorshipThirdParty,
        Self::OutcomeUnknown,
        Self::SourceIntegrityUnknown,
        Self::SourceIntegrityBroken,
    ];

    /// Which of the four checks raised it.
    ///
    /// Total, with no wildcard arm: a ninth code has to name its check.
    #[must_use]
    pub const fn check(self) -> EligibilityCheck {
        match self {
            Self::ConceptLinkAbsent
            | Self::ConceptLinkAmbiguous
            | Self::ConceptLinkTierNotLearnable => EligibilityCheck::ExactConceptLink,
            Self::AuthorshipUnknown | Self::AuthorshipThirdParty => {
                EligibilityCheck::AuthorshipOrParticipation
            }
            Self::OutcomeUnknown => EligibilityCheck::Outcome,
            Self::SourceIntegrityUnknown | Self::SourceIntegrityBroken => {
                EligibilityCheck::SourceIntegrity
            }
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConceptLinkAbsent => "CONCEPT_LINK_ABSENT",
            Self::ConceptLinkAmbiguous => "CONCEPT_LINK_AMBIGUOUS",
            Self::ConceptLinkTierNotLearnable => "CONCEPT_LINK_TIER_NOT_LEARNABLE",
            Self::AuthorshipUnknown => "AUTHORSHIP_UNKNOWN",
            Self::AuthorshipThirdParty => "AUTHORSHIP_THIRD_PARTY",
            Self::OutcomeUnknown => "OUTCOME_UNKNOWN",
            Self::SourceIntegrityUnknown => "SOURCE_INTEGRITY_UNKNOWN",
            Self::SourceIntegrityBroken => "SOURCE_INTEGRITY_BROKEN",
        }
    }
}

/// Check one's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConceptLink {
    /// The ontology resolved the mention to exactly one entity at this tier.
    Exact(EntityId, EntityKind),
    /// More than one reading remained. Section 6.4's unresolved mention.
    Ambiguous,
    /// Nothing was linked.
    Absent,
}

/// Check two's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Participation {
    /// The user wrote it.
    Authored,
    /// The user took part in it.
    Participated,
    /// Somebody else's work.
    ThirdParty,
    /// Not recorded.
    Unknown,
}

/// Check three's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The attempt succeeded.
    Succeeded,
    /// The attempt did not succeed. This is a *known* outcome and passes the
    /// check; whether it promotes anything is section 13.2's question, not this
    /// one's.
    Failed,
    /// Not recorded.
    Unknown,
}

/// Check four's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceIntegrity {
    /// The artifact's bytes hashed to the recorded digest.
    Verified(ContentDigest),
    /// They did not.
    Broken,
    /// Not checked.
    Unknown,
}

/// The four answers for one piece of evidence.
///
/// No `Default` and all four fields required by the constructor, so a dossier
/// that never answered a question is not a dossier with a hole in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceDossier {
    concept_link: ConceptLink,
    participation: Participation,
    outcome: Outcome,
    integrity: SourceIntegrity,
}

impl EvidenceDossier {
    /// Records all four answers.
    #[must_use]
    pub const fn of(
        concept_link: ConceptLink,
        participation: Participation,
        outcome: Outcome,
        integrity: SourceIntegrity,
    ) -> Self {
        Self {
            concept_link,
            participation,
            outcome,
            integrity,
        }
    }

    /// Check one's answer.
    #[must_use]
    pub const fn concept_link(&self) -> ConceptLink {
        self.concept_link
    }

    /// Check two's answer.
    #[must_use]
    pub const fn participation(&self) -> Participation {
        self.participation
    }

    /// Check three's answer.
    #[must_use]
    pub const fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// Check four's answer.
    #[must_use]
    pub const fn integrity(&self) -> &SourceIntegrity {
        &self.integrity
    }

    fn concept_reason(&self) -> Option<EligibilityReasonCode> {
        match self.concept_link {
            ConceptLink::Absent => Some(EligibilityReasonCode::ConceptLinkAbsent),
            ConceptLink::Ambiguous => Some(EligibilityReasonCode::ConceptLinkAmbiguous),
            // Section 7.4: a `FIELD` carries no independent prerequisite of its
            // own and an `ALIAS` never carries evidence itself, so neither is a
            // thing a person holds a mastery of. `P2-R4` refuses the same two
            // tiers for the same reason.
            ConceptLink::Exact(_, EntityKind::Field | EntityKind::Alias) => {
                Some(EligibilityReasonCode::ConceptLinkTierNotLearnable)
            }
            ConceptLink::Exact(
                _,
                EntityKind::Concept | EntityKind::ConceptSense | EntityKind::Operation,
            ) => None,
        }
    }

    const fn participation_reason(&self) -> Option<EligibilityReasonCode> {
        match self.participation {
            Participation::Authored | Participation::Participated => None,
            Participation::ThirdParty => Some(EligibilityReasonCode::AuthorshipThirdParty),
            Participation::Unknown => Some(EligibilityReasonCode::AuthorshipUnknown),
        }
    }

    const fn outcome_reason(&self) -> Option<EligibilityReasonCode> {
        match self.outcome {
            Outcome::Succeeded | Outcome::Failed => None,
            Outcome::Unknown => Some(EligibilityReasonCode::OutcomeUnknown),
        }
    }

    const fn integrity_reason(&self) -> Option<EligibilityReasonCode> {
        match self.integrity {
            SourceIntegrity::Verified(_) => None,
            SourceIntegrity::Broken => Some(EligibilityReasonCode::SourceIntegrityBroken),
            SourceIntegrity::Unknown => Some(EligibilityReasonCode::SourceIntegrityUnknown),
        }
    }

    /// Every failing check's code, in [`EligibilityCheck::ALL`] order.
    #[must_use]
    pub fn blocking_reasons(&self) -> Vec<EligibilityReasonCode> {
        [
            self.concept_reason(),
            self.participation_reason(),
            self.outcome_reason(),
            self.integrity_reason(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// Evidence that passed all four checks.
///
/// Private fields and one producer, [`EligibilityOutcome::admit`]. The
/// projection takes a slice of these, so blocked evidence has no representation
/// the projection accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibleEvidence {
    evidence: ConceptEvidence,
    concept: EntityId,
    tier: EntityKind,
    evidence_id: EvidenceId,
    outcome: Outcome,
}

impl EligibleEvidence {
    /// The evidence, unchanged.
    #[must_use]
    pub const fn evidence(&self) -> &ConceptEvidence {
        &self.evidence
    }

    /// Which concept it is about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The section 7.4 tier the ontology resolved.
    #[must_use]
    pub const fn tier(&self) -> EntityKind {
        self.tier
    }

    /// Which evidence item.
    #[must_use]
    pub const fn evidence_id(&self) -> EvidenceId {
        self.evidence_id
    }

    /// The recorded outcome.
    #[must_use]
    pub const fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// Which of section 13.2's rows this is.
    #[must_use]
    pub const fn kind(&self) -> EvidenceKind {
        self.evidence.kind()
    }
}

/// Evidence that did not, with every failing check's code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedEvidence {
    evidence: ConceptEvidence,
    evidence_id: EvidenceId,
    reasons: Vec<EligibilityReasonCode>,
}

impl BlockedEvidence {
    /// The evidence, unchanged and undiscarded.
    #[must_use]
    pub const fn evidence(&self) -> &ConceptEvidence {
        &self.evidence
    }

    /// Which evidence item.
    #[must_use]
    pub const fn evidence_id(&self) -> EvidenceId {
        self.evidence_id
    }

    /// Every failing check's code, in [`EligibilityCheck::ALL`] order.
    #[must_use]
    pub fn reasons(&self) -> &[EligibilityReasonCode] {
        &self.reasons
    }

    /// Which checks failed.
    #[must_use]
    pub fn failed_checks(&self) -> Vec<EligibilityCheck> {
        let mut checks: Vec<EligibilityCheck> =
            self.reasons.iter().map(|code| code.check()).collect();
        checks.dedup();
        checks
    }
}

/// The result of running section 13.4's four checks over one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EligibilityOutcome {
    /// All four passed.
    Admitted(EligibleEvidence),
    /// At least one did not.
    Blocked(BlockedEvidence),
}

impl EligibilityOutcome {
    /// Runs all four checks over `evidence` and `dossier`.
    ///
    /// Every check is evaluated; the result carries all of the failing codes
    /// rather than the first.
    #[must_use]
    pub fn admit(
        evidence: ConceptEvidence,
        evidence_id: EvidenceId,
        dossier: &EvidenceDossier,
    ) -> Self {
        let reasons = dossier.blocking_reasons();
        if !reasons.is_empty() {
            return Self::Blocked(BlockedEvidence {
                evidence,
                evidence_id,
                reasons,
            });
        }
        let ConceptLink::Exact(concept, tier) = dossier.concept_link() else {
            // `blocking_reasons` already refused every other link state; keeping
            // the pattern match makes an admitted item structurally
            // exactly-linked if that function ever grows a case.
            return Self::Blocked(BlockedEvidence {
                evidence,
                evidence_id,
                reasons: vec![EligibilityReasonCode::ConceptLinkAbsent],
            });
        };
        Self::Admitted(EligibleEvidence {
            evidence,
            concept,
            tier,
            evidence_id,
            outcome: dossier.outcome(),
        })
    }

    /// The admitted evidence, when there is any.
    #[must_use]
    pub const fn admitted(&self) -> Option<&EligibleEvidence> {
        match self {
            Self::Admitted(evidence) => Some(evidence),
            Self::Blocked(_) => None,
        }
    }

    /// The blocked evidence, when there is any.
    #[must_use]
    pub const fn blocked(&self) -> Option<&BlockedEvidence> {
        match self {
            Self::Blocked(blocked) => Some(blocked),
            Self::Admitted(_) => None,
        }
    }
}
