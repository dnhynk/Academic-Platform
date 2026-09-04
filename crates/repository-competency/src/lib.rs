//! `P2-R5`: section 17.6, which is one sentence and one boundary.
//!
//! > `따라서 ProjectSnapshot OBSERVES Concept과 User APPLIED Concept은 다른
//! > Claim이다.`
//!
//! `P2-R4` produced the first. This crate produces the second, and the whole of
//! it is that the second is not the first with a different word on it. If that
//! separation fails the product tells a user they have a competency they do not
//! have, which section 24.3 names directly: `dependency를 사용했다는 이유만으로
//! competency를 채우지 않는다`.
//!
//! ## Using a repository is not a personal claim
//!
//! A [`ProjectObservationClaim`] is read out of a `P2-R4` [`ConceptStance`]'s
//! observed half and needs nothing from the user. A
//! [`PersonalApplicationClaim`] needs an [`AuthoredWork`] whose changed sites
//! meet that observation's own locators — and an [`AuthoredWork`] has one
//! producer, [`ContributionDraft::seal`], which applies section 17.6's checks.
//! A run with no contribution at all therefore publishes every project claim
//! and no personal one, and that is arithmetic rather than a rule:
//! [`promote`]'s personal loop iterates the works it was given.
//!
//! ## Section 17.6's five checks, and what holds each
//!
//! | Bullet | [`PromotionCheck`] | What holds it |
//! |---|---|---|
//! | `사용자 authorship 또는 실질적 기여` | `AUTHORSHIP` | [`AuthorshipMap::resolve`], whole-pair set membership |
//! | `단순 scaffold가 아닌 이해가 필요한 선택·수정` | `MEANINGFUL_CHANGE` | [`ScaffoldRubric`], versioned configuration |
//! | `test, explanation, debugging, review 등 결과 evidence` | `OUTCOME_EVIDENCE` | [`CandidateSupport`], which grades rather than blocks |
//! | `읽은 것인지 직접 구현한 것인지` | `READ_VERSUS_AUTHORED` | [`AuthorshipMode`] has no review value |
//! | `생성형 AI ... 검증·수정·설명했는지` | `GENERATED_CODE_WARRANT` | [`CodeOrigin::Generated`] holds the warrant |
//!
//! Four of the five block a promotion and the third grades it. That is not a
//! weaker check — section 17.6 asks for the outcome evidence to be *confirmed*,
//! and `outcome_artifact_strengthens_candidate` is what confirming it means:
//! the user's own meaningful change is already a candidate, and a test or a
//! diagnosis raises what is carrying it. An outcome with no authorship beside
//! it raises nothing, because there is no candidate for it to raise.
//!
//! `each_of_section_17_6_s_checks_changes_the_outcome` walks
//! [`PromotionCheck::ALL`] and fails only one check at a time, so no entry can
//! be registered without biting.
//!
//! ## It opens nothing
//!
//! Like the four crates below it. Every artifact arrives as an argument, no
//! path is read, and this crate holds no analyzed byte at all.
//! `the_competency_crate_touches_no_file_and_no_socket` compares the whole set
//! of its `use` items, the whole set of the paths it reaches through a crate
//! root, and the whole set of the macros it invokes against pinned inventories,
//! in both directions.
//!
//! ## It persists nothing
//!
//! No migration and no edge to `academic-store`, for `P2-R4`'s reason: a
//! promotion is a derivation over evidence two crates below already froze, and
//! a second copy of it in a database would be a second place for it to be
//! wrong.

pub mod claim;
pub mod contribution;
pub mod generated;
pub mod identity;
pub mod outcome;
pub mod rubric;

use academic_repository_classification::{ClassificationSet, ConceptStance};

pub use claim::{
    ClaimId, ClaimStanding, PersonalApplicationClaim, PersonalProvenance, ProjectObservationClaim,
    ProjectProvenance, RejectionReason,
};
pub use contribution::{
    AuthoredWork, AuthorshipMode, ChangeId, ContributionDraft, ContributionKind, ContributionRecord,
};
pub use generated::{
    CodeOrigin, ExplainedByUser, GeneratedCodeWarrant, ModifiedByUser, OriginReport,
    VerifiedByUser, WarrantStep,
};
pub use identity::{AuthorshipMap, ExternalAuthorId, IdentitySource, UserId};
pub use outcome::{CandidateSupport, OutcomeArtifact, OutcomeKind};
pub use rubric::{ChangeKind, ChangeVerdict, ChangedSite, RubricId, ScaffoldRubric};

/// Which of section 17.6's five separate confirmations is being spoken about.
///
/// In the section's own bullet order. The count is not asserted as a number
/// here or in the code: `the_promotion_checks_are_section_17_6_s` reads the
/// bullets back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`
/// and compares their number against [`PromotionCheck::ALL`], so it is a
/// measurement of the design document rather than a restatement of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PromotionCheck {
    /// `해당 code/decision에 대한 사용자 authorship 또는 실질적 기여`.
    Authorship,
    /// `단순 scaffold가 아닌 이해가 필요한 선택·수정`.
    MeaningfulChange,
    /// `test, explanation, debugging, review 등 결과 evidence`.
    OutcomeEvidence,
    /// `다른 사람이 작성한 code를 읽은 것인지 직접 구현한 것인지`.
    ReadVersusAuthored,
    /// `생성형 AI가 작성한 code라면 사용자가 검증·수정·설명했는지`.
    GeneratedCodeWarrant,
}

impl PromotionCheck {
    /// Exhaustive order, in section 17.6's own bullet order.
    pub const ALL: [Self; 5] = [
        Self::Authorship,
        Self::MeaningfulChange,
        Self::OutcomeEvidence,
        Self::ReadVersusAuthored,
        Self::GeneratedCodeWarrant,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authorship => "AUTHORSHIP",
            Self::MeaningfulChange => "MEANINGFUL_CHANGE",
            Self::OutcomeEvidence => "OUTCOME_EVIDENCE",
            Self::ReadVersusAuthored => "READ_VERSUS_AUTHORED",
            Self::GeneratedCodeWarrant => "GENERATED_CODE_WARRANT",
        }
    }

    /// Whether failing this check stops a promotion.
    ///
    /// Total with no default arm. `OUTCOME_EVIDENCE` is the one that grades
    /// instead: see the crate documentation.
    #[must_use]
    pub const fn blocks(self) -> bool {
        match self {
            Self::Authorship
            | Self::MeaningfulChange
            | Self::ReadVersusAuthored
            | Self::GeneratedCodeWarrant => true,
            Self::OutcomeEvidence => false,
        }
    }
}

/// Why a promotion was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompetencyError {
    /// An identifier was empty, too long, or held a forbidden byte.
    #[error("the {0} identifier {1:?} is not [A-Za-z0-9._-] within 64 bytes")]
    InvalidIdentifier(&'static str, String),
    /// Section 17.6's first bullet: the mapping does not say this is the user.
    #[error(
        "change {change}'s {namespace} identity is not in the user's authorship mapping at \
         version {mapping_version}"
    )]
    AuthorIsNotTheUser {
        /// Which change.
        change: String,
        /// Which namespace the unmapped identity was in.
        namespace: &'static str,
        /// Which mapping version was consulted.
        mapping_version: u64,
    },
    /// Section 17.6's fourth bullet: reading is not implementing.
    #[error("change {change} is {} , which is not authorship", .kind.as_str())]
    ContributionIsNotAuthorship {
        /// Which change.
        change: String,
        /// What the connector reported the person did.
        kind: ContributionKind,
    },
    /// Section 17.6's second bullet, under the rubric that judged it.
    #[error(
        "change {change} carries {bearing_sites} understanding-bearing sites and rubric {rubric} \
         version {version} wants {required}"
    )]
    ChangeIsScaffoldOnly {
        /// Which change.
        change: String,
        /// Which rubric decided.
        rubric: String,
        /// Which version of it.
        version: u64,
        /// How many sites it counted.
        bearing_sites: u32,
        /// How many it wanted.
        required: u32,
    },
    /// Section 17.6's fifth bullet.
    #[error("change {change} is generated code with no warrant: {}", .first_missing.as_str())]
    GeneratedCodeHasNoWarrant {
        /// Which change.
        change: String,
        /// The first of the three the user had not done.
        first_missing: WarrantStep,
    },
    /// A warrant step named no place in the snapshot.
    #[error("the warrant step {} names no site", .0.as_str())]
    WarrantStepHasNoSite(WarrantStep),
    /// A warrant step carried no words.
    #[error("the warrant step {} carries no note", .0.as_str())]
    WarrantStepHasNoNote(WarrantStep),
    /// A rubric that requires nothing decides nothing.
    #[error("rubric {0} version {1} requires no understanding-bearing site")]
    RubricAdmitsNothing(String, u64),
    /// A work was offered against a snapshot other than the classified one.
    #[error("the work on change {0} is about snapshot {1}, not the classified one")]
    WorkIsAboutAnotherSnapshot(String, String),
    /// A work was offered under a mapping that is not the run's user.
    #[error("the work on change {0} belongs to user {1}, not {2}")]
    WorkBelongsToAnotherUser(String, String, String),
    /// Two works named one concept under one goal scope.
    #[error("{0} carries two authored works under goal {1} version {2}")]
    DuplicatePromotion(String, String, u64),
    /// A claim was taken back twice.
    #[error("claim {0} is already rejected")]
    ClaimAlreadyRejected(String),
}

/// One promotion request: one classification, one user, the evidence.
///
/// Public fields, the way `P2-R4`'s `ClassificationInput` has them: this is the
/// argument list of [`promote`] and every field is required.
#[derive(Debug)]
pub struct PromotionInput<'a> {
    /// `P2-R4`'s output over the snapshot being promoted from.
    pub classification: &'a ClassificationSet,
    /// Whose personal claims this run is about.
    pub user: &'a UserId,
    /// The sealed contributions, each already past section 17.6's checks.
    pub works: &'a [AuthoredWork],
    /// Section 17.6's third bullet, which grades rather than admits.
    pub outcomes: &'a [OutcomeArtifact],
}

/// What one promotion run produced.
///
/// Two lists, and neither is derived from the other. No method takes
/// `&mut self`, for `P2-R4`'s reason: a correction is a new run over new
/// evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionSet {
    snapshot_id: String,
    project: Vec<ProjectObservationClaim>,
    personal: Vec<PersonalApplicationClaim>,
}

impl PromotionSet {
    /// Which snapshot was promoted from.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Every `ProjectSnapshot OBSERVES Concept`, in concept order.
    #[must_use]
    pub fn project_claims(&self) -> &[ProjectObservationClaim] {
        &self.project
    }

    /// Every `User APPLIED Concept`, in concept order.
    #[must_use]
    pub fn personal_claims(&self) -> &[PersonalApplicationClaim] {
        &self.personal
    }

    /// The project claim for one concept.
    #[must_use]
    pub fn project_claim(&self, concept: &str) -> Option<&ProjectObservationClaim> {
        self.project.iter().find(|item| item.concept() == concept)
    }

    /// The personal claim for one concept.
    #[must_use]
    pub fn personal_claim(&self, concept: &str) -> Option<&PersonalApplicationClaim> {
        self.personal.iter().find(|item| item.concept() == concept)
    }
}

/// Section 17.6, over one classified snapshot and one user.
///
/// # Errors
///
/// [`CompetencyError::WorkIsAboutAnotherSnapshot`] when a work was read over a
/// different snapshot; [`CompetencyError::WorkBelongsToAnotherUser`] when a
/// work's mapping resolved somebody else; and
/// [`CompetencyError::DuplicatePromotion`] when two works would promote one
/// concept in one goal scope. The last is a refusal rather than a choice, for
/// section 18.4's reason one task over: picking between two pieces of evidence
/// about the same subject is a judgement, and this crate does not make it.
pub fn promote(input: &PromotionInput<'_>) -> Result<PromotionSet, CompetencyError> {
    let snapshot_id = input.classification.snapshot_id();
    let goal = input.classification.goal();
    let goal_name = goal.goal().as_str().to_owned();
    let version = goal.version();

    for work in input.works {
        if work.snapshot_id() != snapshot_id {
            return Err(CompetencyError::WorkIsAboutAnotherSnapshot(
                work.change().as_str().to_owned(),
                work.snapshot_id().to_owned(),
            ));
        }
        if work.user() != input.user {
            return Err(CompetencyError::WorkBelongsToAnotherUser(
                work.change().as_str().to_owned(),
                work.user().as_str().to_owned(),
                input.user.as_str().to_owned(),
            ));
        }
    }

    let mut project = Vec::new();
    let mut personal = Vec::new();

    for stance in input.classification.stances() {
        // The project claim is read from the observed half and from nothing
        // else. A stance with an outlook and no observation says what the user
        // has to learn, not what the snapshot contains, so it produces neither
        // claim here.
        let Some(proof) = stance.observed() else {
            continue;
        };
        let concept = stance.key().concept();
        let project_id = ClaimId::new(claim::project_claim_identity(
            snapshot_id,
            &goal_name,
            version,
            concept,
        ))?;
        project.push(ProjectObservationClaim::seal(
            project_id.clone(),
            stance.key().clone(),
            ProjectProvenance::of_proof(proof),
        ));

        // The personal claim needs a work whose own sites meet this
        // observation's. Authoring something else in a repository that observes
        // a concept is not authoring that concept's use.
        // One refusal and not two. A second one, keyed on the concept, would
        // refuse two proofs for one concept — and `P2-R4` cannot produce them:
        // `ClassificationSet` has one construction site and it iterates a
        // `BTreeSet` of concept names, so a concept is one stance. `P2-A5`
        // measured a check of that shape here as one nothing could reach, and
        // `one_concept_is_one_stance_however_many_routes_reach_it` in `P2-R4`'s
        // own suite is where the fact it rested on is now observed instead.
        let mut touching = input
            .works
            .iter()
            .filter(|work| work.touches(proof.locators()));
        let Some(work) = touching.next() else {
            continue;
        };
        if touching.next().is_some() {
            return Err(CompetencyError::DuplicatePromotion(
                concept.to_owned(),
                goal_name,
                version,
            ));
        }

        let outcomes: Vec<&OutcomeArtifact> = input
            .outcomes
            .iter()
            .filter(|item| item.concept() == concept && item.change() == work.change())
            .collect();
        let personal_id = ClaimId::new(claim::personal_claim_identity(
            input.user.as_str(),
            snapshot_id,
            &goal_name,
            version,
            concept,
            work.change().as_str(),
        ))?;
        personal.push(PersonalApplicationClaim::seal(
            personal_id,
            stance.key().clone(),
            work,
            &outcomes,
            project_id,
        ));
    }

    Ok(PromotionSet {
        snapshot_id: snapshot_id.to_owned(),
        project,
        personal,
    })
}

/// Every stance of `classification` that carries an observation, in its order.
///
/// The set [`promote`] reads its project claims from. Exported so a reader can
/// see what the promotion is over without running it.
#[must_use]
pub fn observing_stances(classification: &ClassificationSet) -> Vec<&ConceptStance> {
    classification
        .stances()
        .iter()
        .filter(|stance| stance.observed().is_some())
        .collect()
}

/// Whether a repository observation alone would promote anything.
///
/// Always `false`, and it is a function rather than a sentence in a comment so
/// that a caller asking section 17.6's own question gets this crate's answer
/// rather than guessing from the absence of an API.
/// `repo_use_alone_creates_no_personal_claim` observes the same fact by running
/// a promotion with no contribution at all.
#[must_use]
pub const fn observation_alone_promotes() -> bool {
    false
}
