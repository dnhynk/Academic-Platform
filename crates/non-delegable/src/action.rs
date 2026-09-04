//! The whole set of actions this layer knows, and which side of section 27
//! each one falls on.
//!
//! # Why this is a whole set and not a list of forbidden names
//!
//! This run measured a name list letting five public functions through in
//! `P2-N7` because none of them spelled a listed name, and it measured a
//! `From` impl walking past every `pub fn` sweep in `P2-Y3`. A non-delegable
//! set stated as "these six tokens are refused" has the same shape and the
//! same hole: the action nobody predicted is not on the list.
//!
//! So the set here is **total**. [`Action`] is closed over section 27.1's ten
//! candidate-generation rows and the six actions the execution plan makes
//! non-delegable, and [`Action::delegability`] matches every one of the sixteen
//! individually. A seventeenth action does not compile until it says which side
//! it is on, and `the_spec_tables_are_this_action_set` reads sections 27.1 and
//! 27.4 back out of the design document and compares them in both directions,
//! so an action this crate invents and an action the document adds both fail.
//!
//! # The plan's six is not section 27.4's non-delegable row
//!
//! Section 27.4 has four rows. Its `non-delegable` row names **three** things:
//! `question resolved, career/course decision, permission attestation는
//! 사용자만`. Its `high risk` row names three more and requires `명시적 승인`
//! of them: `Knowledge State 승격, private data 외부 반출, official rule
//! publish`.
//!
//! The execution plan's non-delegable set is six: question resolution, mastery
//! promotion to user-confirmed, course and career decisions, permission
//! attestation, egress approval, and deletion confirmation. Two of those —
//! mastery promotion and egress approval — are section 27.4's **high-risk**
//! row, not its non-delegable row, and deletion confirmation is in neither: it
//! appears nowhere in section 27.
//!
//! That difference is carried rather than erased. Every [`NonDelegableAction`]
//! reports the section 27.4 row that places it through
//! [`NonDelegableAction::declared_tier`], and the two the document does not
//! place report [`None`]. What unifies all six is the narrower claim this
//! crate actually enforces and the execution plan actually states — *every
//! entry needs an authenticated user actor and an explicit decision event* —
//! and that is true of the high-risk row too, because `명시적 승인` is
//! `P2-M2`'s `ExplicitApproval`, which is built from a `UserDecision` that only
//! `Actor::User` can mint.
//!
//! `docs/contracts/non-delegable-actions.md` records the three readings.

use academic_proposal::RiskTier;

/// Section 27.1's ten rows: what an automatic actor may produce a candidate for.
///
/// One variant per table row, in the table's own order.
/// [`CandidateGeneration::spec_row`] returns the row's first cell verbatim, and
/// `the_spec_tables_are_this_action_set` compares the whole set against the
/// design document in both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CandidateGeneration {
    /// `STT/diarization/OCR`.
    SpeechAndOpticalRecognition,
    /// `transcript/syllabus concept extraction`.
    ConceptExtraction,
    /// `concept relation 발견`.
    ConceptRelationDiscovery,
    /// `next lecture prerequisite`.
    NextLecturePrerequisite,
    /// `repository semantic analysis`.
    RepositorySemanticAnalysis,
    /// `spec/code drift detection`.
    SpecCodeDriftDetection,
    /// `review clustering`.
    ReviewClustering,
    /// `Build → Learn decomposition`.
    BuildToLearnDecomposition,
    /// `Blind Spot explanation`.
    BlindSpotExplanation,
    /// `career mapping`.
    CareerMapping,
}

impl CandidateGeneration {
    /// Exhaustive order, section 27.1's own row order.
    pub const ALL: [Self; 10] = [
        Self::SpeechAndOpticalRecognition,
        Self::ConceptExtraction,
        Self::ConceptRelationDiscovery,
        Self::NextLecturePrerequisite,
        Self::RepositorySemanticAnalysis,
        Self::SpecCodeDriftDetection,
        Self::ReviewClustering,
        Self::BuildToLearnDecomposition,
        Self::BlindSpotExplanation,
        Self::CareerMapping,
    ];

    /// The first cell of this row of section 27.1, verbatim.
    #[must_use]
    pub const fn spec_row(self) -> &'static str {
        match self {
            Self::SpeechAndOpticalRecognition => "STT/diarization/OCR",
            Self::ConceptExtraction => "transcript/syllabus concept extraction",
            Self::ConceptRelationDiscovery => "concept relation 발견",
            Self::NextLecturePrerequisite => "next lecture prerequisite",
            Self::RepositorySemanticAnalysis => "repository semantic analysis",
            Self::SpecCodeDriftDetection => "spec/code drift detection",
            Self::ReviewClustering => "review clustering",
            Self::BuildToLearnDecomposition => "Build → Learn decomposition",
            Self::BlindSpotExplanation => "Blind Spot explanation",
            Self::CareerMapping => "career mapping",
        }
    }
}

/// The six actions no automatic actor may perform.
///
/// The execution plan's own enumeration, in the order its outcome sentence
/// states them. Every variant needs an authenticated user actor and an explicit
/// decision event; [`crate::DecisionEvent`] is that event and it has no
/// producer that takes an automatic actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NonDelegableAction {
    /// Moving a question to `RESOLVED`. Section 27.2's second bullet, section
    /// 27.4's non-delegable row, and section 14.2's `RESOLVED`는 사용자의 명시적
    /// decision.
    ResolveQuestion,
    /// Raising a knowledge state to a user-confirmed one. Section 27.4's
    /// high-risk row, and section 13.1's `FLUENT` row.
    ConfirmMastery,
    /// Deciding what to take and what to become. Section 27.2's third bullet
    /// and section 27.4's non-delegable row.
    DecideEnrollmentOrCareer,
    /// Attesting that a permission was given. Section 27.4's non-delegable row.
    AttestPermission,
    /// Approving that private bytes leave. Section 27.2's seventh bullet and
    /// section 27.4's high-risk row.
    ApproveEgress,
    /// Confirming that an artifact and its derivatives are destroyed. Section
    /// 27 does not name it; the execution plan does, and `P2-P2` built it.
    ConfirmDeletion,
}

impl NonDelegableAction {
    /// Exhaustive order, the execution plan's own.
    pub const ALL: [Self; 6] = [
        Self::ResolveQuestion,
        Self::ConfirmMastery,
        Self::DecideEnrollmentOrCareer,
        Self::AttestPermission,
        Self::ApproveEgress,
        Self::ConfirmDeletion,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResolveQuestion => "RESOLVE_QUESTION",
            Self::ConfirmMastery => "CONFIRM_MASTERY",
            Self::DecideEnrollmentOrCareer => "DECIDE_ENROLLMENT_OR_CAREER",
            Self::AttestPermission => "ATTEST_PERMISSION",
            Self::ApproveEgress => "APPROVE_EGRESS",
            Self::ConfirmDeletion => "CONFIRM_DELETION",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
    }

    /// The section 27.4 row that places this action, when one does.
    ///
    /// [`None`] is not a weaker answer. It is the measurement: section 27.4
    /// names six things across two rows and this set has six members, but they
    /// are not the same six. `the_spec_tables_are_this_action_set` reads the
    /// four rows out of the design document and checks each phrase below occurs
    /// in the row named here **and in no other row**, and that no row names
    /// deletion at all.
    #[must_use]
    pub const fn declared_tier(self) -> Option<RiskTier> {
        match self {
            Self::ResolveQuestion | Self::DecideEnrollmentOrCareer | Self::AttestPermission => {
                Some(RiskTier::NonDelegable)
            }
            Self::ConfirmMastery | Self::ApproveEgress => Some(RiskTier::HighApproval),
            Self::ConfirmDeletion => None,
        }
    }

    /// The phrase section 27.4 uses for this action inside the row
    /// [`NonDelegableAction::declared_tier`] names.
    ///
    /// Empty for the action no row names. The phrases are the document's own
    /// bytes, so a document that renames one fails this crate rather than
    /// drifting past it.
    #[must_use]
    pub const fn declared_phrase(self) -> &'static str {
        match self {
            Self::ResolveQuestion => "question resolved",
            Self::ConfirmMastery => "Knowledge State 승격",
            Self::DecideEnrollmentOrCareer => "career/course decision",
            Self::AttestPermission => "permission attestation",
            Self::ApproveEgress => "private data 외부 반출",
            Self::ConfirmDeletion => "",
        }
    }
}

/// Which side of section 27 an action falls on, carrying the action itself.
///
/// Two arms and no third. An action is either something an automatic actor may
/// produce a candidate for or something only an authenticated user may decide;
/// there is no "usually a user" arm for a caller to pick when it is unsure.
///
/// Each arm carries its own half of [`Action`], so the classification is what
/// the command layer dispatches on and there is no arm the layer has to call
/// impossible. A classification that answered only *which side* would have left
/// [`crate::authorise`] re-deriving the payload from an action it had already
/// classified, and that second derivation is a branch nothing tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Delegability {
    /// Section 27.1. An automatic actor may propose; a human confirms.
    AutomaticActorMayPropose(CandidateGeneration),
    /// Section 27.2 and 27.4. An authenticated user actor and an explicit
    /// decision event, or nothing.
    AuthenticatedUserOnly(NonDelegableAction),
}

/// Every action this layer knows.
///
/// Closed over both halves of section 27, so the classification below is total
/// rather than a filter with a permissive default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    /// One of section 27.1's ten rows.
    Generate(CandidateGeneration),
    /// One of the six the execution plan makes non-delegable.
    Decide(NonDelegableAction),
}

impl Action {
    /// Every action, generation rows first, in each half's own order.
    #[must_use]
    pub fn all() -> Vec<Self> {
        CandidateGeneration::ALL
            .into_iter()
            .map(Self::Generate)
            .chain(NonDelegableAction::ALL.into_iter().map(Self::Decide))
            .collect()
    }

    /// Which side of section 27 this action falls on.
    ///
    /// Every one of the sixteen is named individually. Matching on the outer
    /// arm alone would have let a seventeenth variant added inside either half
    /// inherit a side without anybody choosing it, which is the shape of the
    /// empty guard this run has met eighteen times. With the variants spelled
    /// out, a new one makes this `match` non-exhaustive and the crate stops
    /// compiling until it says which side it is on.
    #[must_use]
    pub const fn delegability(self) -> Delegability {
        match self {
            Self::Generate(
                generation @ (CandidateGeneration::SpeechAndOpticalRecognition
                | CandidateGeneration::ConceptExtraction
                | CandidateGeneration::ConceptRelationDiscovery
                | CandidateGeneration::NextLecturePrerequisite
                | CandidateGeneration::RepositorySemanticAnalysis
                | CandidateGeneration::SpecCodeDriftDetection
                | CandidateGeneration::ReviewClustering
                | CandidateGeneration::BuildToLearnDecomposition
                | CandidateGeneration::BlindSpotExplanation
                | CandidateGeneration::CareerMapping),
            ) => Delegability::AutomaticActorMayPropose(generation),
            Self::Decide(
                action @ (NonDelegableAction::ResolveQuestion
                | NonDelegableAction::ConfirmMastery
                | NonDelegableAction::DecideEnrollmentOrCareer
                | NonDelegableAction::AttestPermission
                | NonDelegableAction::ApproveEgress
                | NonDelegableAction::ConfirmDeletion),
            ) => Delegability::AuthenticatedUserOnly(action),
        }
    }

    /// The non-delegable action this is, when it is one.
    #[must_use]
    pub const fn non_delegable(self) -> Option<NonDelegableAction> {
        match self {
            Self::Decide(action) => Some(action),
            Self::Generate(_) => None,
        }
    }
}
