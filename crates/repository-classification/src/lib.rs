//! `P2-R4`: section 18's three classifications, and the proofs none of them may
//! be published without.
//!
//! `P2-R2` produced findings at one of section 17.3's five rungs, folded onto
//! `PRESENT_ONLY`/`POSSIBLE`/`OBSERVED`. `P2-R3` turned those into section
//! 17.5's seven typed relations and its two drift lanes. This crate is the
//! third step over the same evidence: which concepts this project **observes**,
//! which it **requires**, and which it **would benefit from** — and, for each
//! of the three, what a publication has to carry before the word may be used.
//!
//! ## It builds on the two below it and does not go around them
//!
//! `OBSERVED` is read out of a `P2-R3` [`RelationEdge`] whose evidence is a
//! `P2-R2` finding at [`EvidenceTier::Observed`]. There is no second ladder
//! here, no second tier vocabulary, and no route from repository bytes to a
//! classification that skips either crate. `REQUIRED`'s first step is a
//! `P2-R2` [`Finding`] or an **approved** `P2-R3` [`IntentDocument`], so a
//! requirement inherits `P2-R2`'s refusal of a repository-wide scope rather
//! than restating it.
//!
//! ## It opens nothing
//!
//! Like both crates below it. Every artifact arrives as an argument, no path is
//! read, and this crate holds no analyzed byte at all.
//! `the_classification_crate_touches_no_file_and_no_socket` compares the whole
//! set of its `use` items, the whole set of the paths it reaches through a
//! crate root, and the whole set of the macros it invokes against pinned
//! inventories, in both directions.
//!
//! ## The proofs are types, not checks
//!
//! Four absences carry section 18, and each of them is a value that does not
//! exist rather than a comparison somebody has to remember to run:
//!
//! | Section 18 rule | What holds it |
//! |---|---|
//! | `REQUIRED` needs all five chain steps | each step is a constructor argument of the next ([`chain`]) |
//! | a sufficient user is not a requirement | [`UserEvidenceGap`] has no `SUFFICIENT` variant |
//! | a whole field cannot be required | [`RequiredConcept::realizing`] refuses `FIELD` and `ALIAS` |
//! | `REQUIRED` and `WOULD_BENEFIT_FROM` cannot coexist | [`ConceptStance`]'s outlook is one slot ([`Outlook`]) |
//! | `OBSERVED` and `REQUIRED` may coexist | the observed half is a different field |
//! | a benefit needs a trigger and a trade-off | [`BenefitContract::new`] takes both, non-empty |
//!
//! `crates/scenario/tests/compile_fail/` holds the compiled half.
//!
//! ## A user override is never overwritten
//!
//! Section 18.4's fifth bullet, and `P2-R3`'s `ImplementationDrift` pattern
//! applied to a different pair: [`classify`] publishes the user's answer, keeps
//! the analysis's proposal, and records a [`ClassificationConflict`] beside
//! both. Nothing in this crate takes `&mut self`;
//! `no_public_function_mutates_in_place` holds that over the whole package.

pub mod benefit;
pub mod chain;
pub mod conflict;
pub mod migrate;
pub mod requirement;
pub mod scope;
pub mod stance;

use std::collections::{BTreeMap, BTreeSet};

use academic_domain::entity_registry::EntityKind;
use academic_policy::ContentDigest;
use academic_repository_analysis::{EvidenceTier, Finding, SubjectId};
use academic_repository_correlation::{Correlation, IntentDocument, RelationEdge};

pub use benefit::{
    BenefitContract, BenefitDimension, BenefitDraft, BenefitPart, TradeOff, Trigger, TriggerState,
};
pub use chain::{
    ChainDraft, ChainStep, ConcreteNeed, ControllingMechanism, CurrentBasis, NeedKind, ProofChain,
    RequiredConcept, UserEvidenceGap,
};
pub use conflict::{ClassificationConflict, OverrideDecision, UserOverride};
pub use migrate::{
    LocatorMigration, MigratedFinding, MigratedSite, MigrationOutcome, UnmatchedReason,
    migrate_locators,
};
pub use requirement::{
    LifecycleRow, ProjectConceptRequirement, RequirementId, ResolutionStatus, RetirementReason,
};
pub use scope::{ClassificationKey, GoalId, GoalScope};
pub use stance::{ClassificationLabel, ConceptStance, ObservedProof, Outlook};

/// Why a classification was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ClassificationError {
    /// An identifier was empty, too long, or held a forbidden byte.
    #[error("the {0} identifier {1:?} is not [A-Za-z0-9._-] within 64 bytes")]
    InvalidIdentifier(&'static str, String),
    /// A draft offered fewer than section 18.2's five steps.
    #[error("the proof chain is incomplete: {}", .0.as_str())]
    ProofChainStepMissing(ChainStep),
    /// A draft offered fewer than section 18.3's parts.
    #[error("the benefit contract for {concept} is incomplete: {}", .part.as_str())]
    BenefitPartMissing {
        /// Which concept the contract was about.
        concept: String,
        /// Which part was not offered.
        part: BenefitPart,
    },
    /// Section 18.1's first row: a manifest entry is not an implementation.
    #[error("{0} is present in a manifest only; that is not an implementation to require against")]
    PresentOnlyIsNotAnImplementation(String),
    /// A goal basis named a document that is not approved.
    #[error("the goal document {0} is {1}, not APPROVED")]
    GoalIsNotApproved(String, &'static str),
    /// A responsibility or failure scenario had no site in the snapshot.
    #[error("the need {0} names no site in the snapshot; it is not concrete")]
    NeedHasNoSite(String),
    /// Section 18.2's `단지 backend라는 이유로 ... 전체를 요구하지 않는다`.
    #[error("{concept} is at ontology tier {}, which no project requires as a whole", .tier.as_str())]
    TierCannotBeRequired {
        /// Which concept was offered.
        concept: String,
        /// The tier it sits at, which is `FIELD` or `ALIAS`.
        tier: EntityKind,
    },
    /// Section 18.4's second bullet, refused rather than silently resolved.
    #[error(
        "{0} is both REQUIRED and WOULD_BENEFIT_FROM under goal {1} version {2}; section 18.4 \
         admits one of the two in one goal scope"
    )]
    RequiredAndBenefitInOneScope(String, String, u64),
    /// A chain was offered for a snapshot other than the one being classified.
    #[error("the proof chain for {0} is about snapshot {1}, not the classified one")]
    ChainIsAboutAnotherSnapshot(String, String),
    /// Two chains named one concept in one goal scope.
    #[error("{0} carries two proof chains under goal {1} version {2}")]
    DuplicateRequirement(String, String, u64),
    /// Two benefit contracts named one concept in one goal scope.
    #[error("{0} carries two benefit contracts under goal {1} version {2}")]
    DuplicateBenefit(String, String, u64),
    /// A lifecycle transition was attempted out of a terminal status.
    #[error("requirement {0} is already {1}")]
    RequirementAlreadySettled(String, &'static str),
    /// A satisfaction named no site in the snapshot that satisfied it.
    #[error("requirement {0} was satisfied with no evidence in the new snapshot")]
    SatisfactionHasNoEvidence(String),
    /// A replacement named the requirement it replaces.
    #[error("requirement {0} cannot replace itself")]
    RequirementReplacesItself(String),
}

impl From<academic_repository_analysis::AnalysisError> for ClassificationError {
    fn from(error: academic_repository_analysis::AnalysisError) -> Self {
        Self::InvalidIdentifier("subject", error.to_string())
    }
}

/// One classification request: one correlation, one goal version, the proofs.
///
/// Public fields, the way `P2-R3`'s `CorrelationInput` has them: this is the
/// argument list of [`classify`] and every field is required.
#[derive(Debug)]
pub struct ClassificationInput<'a> {
    /// `P2-R3`'s output over the snapshot being classified.
    pub correlation: &'a Correlation,
    /// The goal version every classification here is bound to.
    pub goal: &'a GoalScope,
    /// Complete five-step chains, one per `REQUIRED` candidate.
    pub required: &'a [ProofChain],
    /// Complete contracts, one per `WOULD_BENEFIT_FROM` candidate.
    pub beneficial: &'a [BenefitContract],
    /// Standing user decisions, which this run does not overwrite.
    pub overrides: &'a [UserOverride],
}

/// What one classification run produced.
///
/// No method takes `&mut self`, for the reason `P2-R3`'s `Correlation` has
/// none: a correction is a new run over new evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationSet {
    snapshot_id: String,
    goal: GoalScope,
    stances: Vec<ConceptStance>,
    conflicts: Vec<ClassificationConflict>,
    requirements: Vec<ProjectConceptRequirement>,
}

impl ClassificationSet {
    /// Which snapshot was classified.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Which goal version it was classified under.
    #[must_use]
    pub const fn goal(&self) -> &GoalScope {
        &self.goal
    }

    /// Every concept this run has something to say about, by concept order.
    #[must_use]
    pub fn stances(&self) -> &[ConceptStance] {
        &self.stances
    }

    /// One concept's stance, by the key that identifies it.
    #[must_use]
    pub fn stance(&self, concept: &str) -> Option<&ConceptStance> {
        self.stances
            .iter()
            .find(|stance| stance.key().concept() == concept)
    }

    /// Every conflict a user override opened against a proposal.
    #[must_use]
    pub fn conflicts(&self) -> &[ClassificationConflict] {
        &self.conflicts
    }

    /// Section 18.4's entity, one per published `REQUIRED`.
    #[must_use]
    pub fn requirements(&self) -> &[ProjectConceptRequirement] {
        &self.requirements
    }

    /// Every concept carrying `label`, in concept order.
    #[must_use]
    pub fn labelled(&self, label: ClassificationLabel) -> Vec<&ConceptStance> {
        self.stances
            .iter()
            .filter(|stance| stance.labels().contains(&label))
            .collect()
    }
}

/// Section 18, over one correlated snapshot and one goal version.
///
/// # Errors
///
/// [`ClassificationError::ChainIsAboutAnotherSnapshot`] when a chain was read
/// over a different snapshot; [`ClassificationError::DuplicateRequirement`] and
/// [`ClassificationError::DuplicateBenefit`] when one concept carries two of
/// one kind; and [`ClassificationError::RequiredAndBenefitInOneScope`] when one
/// concept carries one of each. The last is section 18.4's second bullet, and
/// it is a refusal rather than a precedence rule on purpose: choosing between
/// them would be the automatic reclassification the fifth bullet forbids.
pub fn classify(input: &ClassificationInput<'_>) -> Result<ClassificationSet, ClassificationError> {
    let snapshot_id = input.correlation.snapshot_id();
    let goal_name = input.goal.goal().as_str().to_owned();
    let version = input.goal.version();

    let mut required: BTreeMap<String, &ProofChain> = BTreeMap::new();
    for chain in input.required {
        if chain.snapshot_id() != snapshot_id {
            return Err(ClassificationError::ChainIsAboutAnotherSnapshot(
                chain.concept().concept().to_owned(),
                chain.snapshot_id().to_owned(),
            ));
        }
        let concept = chain.concept().concept().to_owned();
        if required.insert(concept.clone(), chain).is_some() {
            return Err(ClassificationError::DuplicateRequirement(
                concept, goal_name, version,
            ));
        }
    }

    let mut beneficial: BTreeMap<String, &BenefitContract> = BTreeMap::new();
    for contract in input.beneficial {
        let concept = contract.concept().to_owned();
        if beneficial.insert(concept.clone(), contract).is_some() {
            return Err(ClassificationError::DuplicateBenefit(
                concept, goal_name, version,
            ));
        }
    }

    // Section 18.4's second bullet, before anything is published. Both maps are
    // still separate here; the single slot they are about to be folded into is
    // what makes the coexistence unrepresentable, and this is what makes the
    // attempt visible instead of silently dropping one of the two.
    if let Some(concept) = required.keys().find(|name| beneficial.contains_key(*name)) {
        return Err(ClassificationError::RequiredAndBenefitInOneScope(
            concept.clone(),
            goal_name,
            version,
        ));
    }

    let observed = observed_by_concept(input.correlation);

    let mut concepts: BTreeSet<&str> = BTreeSet::new();
    concepts.extend(observed.keys().map(String::as_str));
    concepts.extend(required.keys().map(String::as_str));
    concepts.extend(beneficial.keys().map(String::as_str));

    let mut stances = Vec::new();
    let mut conflicts = Vec::new();
    let mut requirements = Vec::new();
    for concept in concepts {
        let key = ClassificationKey::seal(
            snapshot_id.to_owned(),
            input.goal.clone(),
            concept.to_owned(),
        );
        let proposed = required
            .get(concept)
            .map(|chain| Outlook::Required((*chain).clone()))
            .or_else(|| {
                beneficial
                    .get(concept)
                    .map(|contract| Outlook::Beneficial((*contract).clone()))
            });

        // The override is the later decision about the same subject, so it is
        // what the published stance carries; the proposal is kept as the
        // conflict's second side. Neither is rewritten.
        let standing = input
            .overrides
            .iter()
            .filter(|item| item.governs(&key))
            .max_by_key(|item| item.asserted_at());
        let published = match (standing, proposed) {
            (Some(user), Some(proposal)) if user.decision().contradicts(proposal.label()) => {
                conflicts.push(ClassificationConflict::seal(
                    key.clone(),
                    (*user).clone(),
                    proposal,
                ));
                None
            }
            (_, proposal) => proposal,
        };

        if let Some(Outlook::Required(chain)) = &published {
            requirements.push(ProjectConceptRequirement::materialize(
                RequirementId::new(requirement_identity(
                    snapshot_id,
                    &goal_name,
                    version,
                    concept,
                ))?,
                key.clone(),
                chain,
                0,
            ));
        }
        stances.push(ConceptStance::seal(
            key,
            observed.get(concept).cloned(),
            published,
        ));
    }

    Ok(ClassificationSet {
        snapshot_id: snapshot_id.to_owned(),
        goal: input.goal.clone(),
        stances,
        conflicts,
        requirements,
    })
}

/// The strongest observation `P2-R3` recorded for each concept.
///
/// `P2-R3` emits one edge per relation per subject, so a subject observed in
/// production code and exercised by a test has two. The one a stance carries is
/// the one whose artifact scope speaks most strongly about what runs, which is
/// `P2-R2`'s own [`academic_repository_analysis::ArtifactScope::max`] order
/// rather than a second ranking written here.
fn observed_by_concept(correlation: &Correlation) -> BTreeMap<String, ObservedProof> {
    let mut found: BTreeMap<String, ObservedProof> = BTreeMap::new();
    for edge in correlation.relations() {
        let Some(proof) = ObservedProof::of_edge(edge) else {
            continue;
        };
        let entry = found.entry(edge.subject().to_owned());
        match entry {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(proof);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                if proof.artifact_scope().max(slot.get().artifact_scope()) == proof.artifact_scope()
                    && proof.artifact_scope() != slot.get().artifact_scope()
                {
                    slot.insert(proof);
                }
            }
        }
    }
    found
}

/// The identity a materialized requirement gets.
///
/// Derived from the four things section 18.4 says the entity binds and that
/// this run already fixed — the snapshot, the goal, its version and the concept
/// — so re-running the same classification over the same evidence produces the
/// same identity rather than a second entity for one requirement.
///
/// A **digest**, and deliberately not a joined string. `RequirementId` admits
/// 64 bytes, a snapshot identifier is most of that on its own, and a joined
/// identity truncated to fit would drop the goal version and the concept —
/// making two requirements that differ only there one identity. That is this
/// Run's `P2-A1` P1 defect in a second place: content standing in for identity,
/// with the collision silent. `classification_is_snapshot_and_goal_scoped`
/// measures it, and measured it failing before this function was a digest.
///
/// The separator argument holds because no part can contain a zero byte: a goal
/// identifier and a concept are `[A-Za-z0-9._-]`, a snapshot identifier is
/// `academic-repository`'s own `snap_`-prefixed text, and the version is
/// rendered as decimal digits rather than as its little-endian bytes.
fn requirement_identity(snapshot_id: &str, goal: &str, version: u64, concept: &str) -> String {
    let mut preimage = b"academic-repository-classification-requirement-v1\0".to_vec();
    for part in [snapshot_id, goal, &version.to_string(), concept] {
        preimage.extend_from_slice(part.as_bytes());
        preimage.push(0);
    }
    ContentDigest::of(&preimage).as_str().to_owned()
}

/// Reads the `OBSERVED` half of a stance without classifying anything else.
///
/// Section 18.1 alone, for a caller that has a correlation and wants the
/// observation without offering a goal. It is the same
/// [`ObservedProof::of_edge`] the classifier uses, so the two cannot disagree.
#[must_use]
pub fn observed_concepts(correlation: &Correlation) -> Vec<(String, ObservedProof)> {
    observed_by_concept(correlation).into_iter().collect()
}

/// Whether a finding is one a `REQUIRED` chain may be founded on.
///
/// Section 18.1's first row as a predicate, exported because a caller
/// assembling a [`ChainDraft`] needs the same answer
/// [`CurrentBasis::of_current_code`] gives without building the basis to find
/// out. Total over [`EvidenceTier`] with no default arm.
#[must_use]
pub const fn can_found_a_requirement(finding: &Finding) -> bool {
    match finding.tier() {
        EvidenceTier::PresentOnly => false,
        EvidenceTier::Possible | EvidenceTier::Observed => true,
    }
}

/// Every subject an approved intent document names, as a chain basis may use it.
///
/// A convenience over `P2-R3`'s own document type that spells the approval rule
/// once: a draft or a deprecated document names nothing here.
#[must_use]
pub fn approved_goal_subjects(document: &IntentDocument) -> Vec<SubjectId> {
    if document.status() == academic_repository_correlation::ApprovalStatus::Approved {
        document.mentions().to_vec()
    } else {
        Vec::new()
    }
}

/// Every edge of `correlation` that observed a use, in the correlation's order.
///
/// The set [`observed_by_concept`] reduces. Exported so a reader can see every
/// observation rather than the strongest one per concept.
#[must_use]
pub fn observing_edges(correlation: &Correlation) -> Vec<&RelationEdge> {
    correlation
        .relations()
        .iter()
        .filter(|edge| ObservedProof::of_edge(edge).is_some())
        .collect()
}
