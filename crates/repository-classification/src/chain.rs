//! Section 18.2's five-step proof chain, as five types rather than as five
//! checks.
//!
//! ```text
//! current code/goal
//!   → concrete responsibility or failure scenario
//!   → mechanism that controls it
//!   → required concept
//!   → user's insufficient/uncertain evidence
//! ```
//!
//! ## Why a step cannot be missing
//!
//! Each arrow is a **constructor argument of the next type**, taken by value.
//! [`ConcreteNeed::shown_by`] takes a [`CurrentBasis`],
//! [`ControllingMechanism::controlling`] takes a [`ConcreteNeed`],
//! [`RequiredConcept::realizing`] takes a [`ControllingMechanism`], and
//! [`ProofChain::closed_by`] takes a [`RequiredConcept`] and a
//! [`UserEvidenceGap`]. None of the five has a public field, a `Default`, or a
//! second constructor. So a chain with a step left out is not a value that
//! validates badly — it is a program that does not compile, and
//! `crates/scenario/tests/compile_fail/` holds the committed diagnostics saying
//! so, one per step.
//!
//! That is the mechanism `P2-U1` used for a field with no setter, `P2-U2` for
//! an attestation gate that is a type, and `P2-R2` for a finding that cannot be
//! repository-wide. **An absence is stronger than a check** because nothing has
//! to remember to run it.
//!
//! ## Then why is there a [`ChainDraft`]
//!
//! Because section 18.4's fourth bullet is `AI는 제안하고 사용자는 확인·수정한다`,
//! and what a model proposes is not yet any of those five types. A draft is the
//! one door from an untyped proposal into a chain, and [`ChainDraft::seal`]
//! names the **first missing step** as a [`ChainStep`] code — `REQ-18-006`'s
//! *publish blocked with missing-step code*. Past that door there is no
//! incomplete chain to guard, because there is no incomplete chain.
//!
//! ## The fifth step has no `SUFFICIENT` value
//!
//! A concept whose evidence the user already has, current and confirmed, is not
//! a concept this project requires them to learn. [`UserEvidenceGap`] therefore
//! has exactly two variants, `INSUFFICIENT` and `UNCERTAIN`, and its one
//! constructor returns [`None`] for a state that is neither. A caller holding a
//! sufficient, fresh, user-confirmed state has nothing to pass to
//! [`ProofChain::closed_by`] — which is the refusal, expressed as a value that
//! does not exist rather than as a comparison somebody performs.
//!
//! ## A whole field is not a concept a project can require
//!
//! Section 18.2: `단지 backend라는 이유로 Distributed Systems 전체를 요구하지
//! 않는다`. Three things hold that here and none of them is a keyword list.
//!
//! * [`RequiredConcept::realizing`] refuses [`EntityKind::Field`] and
//!   [`EntityKind::Alias`]. Section 7.4's own vocabulary calls a `FIELD` *a
//!   broad area that carries no independent prerequisite of its own* and an
//!   `ALIAS` a surface form that *never carries evidence itself*; neither is a
//!   thing a failure scenario can be controlled by.
//! * [`CurrentBasis`] cannot be built from a label. Its two constructors take a
//!   `P2-R2` [`Finding`] — which is scoped to a symbol or a component and never
//!   to the repository — or an **approved** intent document. *This project is a
//!   backend* is neither, so a broad category has no basis value at all.
//! * One chain yields exactly one required concept. Requiring twelve concepts
//!   needs twelve chains, each with its own concrete need and its own sites in
//!   the snapshot.

use academic_domain::{EpistemicStatus, FreshnessBand, MasteryLevel, entity_registry::EntityKind};
use academic_repository_analysis::{EvidenceTier, Finding, FindingScope, Locator, SubjectId};
use academic_repository_correlation::{ApprovalStatus, DocumentId, IntentDocument};

use crate::{ClassificationError, scope::GoalScope};

/// Which of section 18.2's five steps a draft was missing.
///
/// The wire spelling is the missing-step code `REQ-18-006` asks a blocked
/// publish to carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChainStep {
    /// `current code/goal`.
    CurrentBasis,
    /// `concrete responsibility or failure scenario`.
    ConcreteNeed,
    /// `mechanism that controls it`.
    ControllingMechanism,
    /// `required concept`.
    RequiredConcept,
    /// `user's insufficient/uncertain evidence`.
    UserEvidenceGap,
}

impl ChainStep {
    /// Exhaustive order, in section 18.2's own arrow order.
    pub const ALL: [Self; 5] = [
        Self::CurrentBasis,
        Self::ConcreteNeed,
        Self::ControllingMechanism,
        Self::RequiredConcept,
        Self::UserEvidenceGap,
    ];

    /// The missing-step code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CurrentBasis => "MISSING_CURRENT_BASIS",
            Self::ConcreteNeed => "MISSING_CONCRETE_NEED",
            Self::ControllingMechanism => "MISSING_CONTROLLING_MECHANISM",
            Self::RequiredConcept => "MISSING_REQUIRED_CONCEPT",
            Self::UserEvidenceGap => "MISSING_USER_EVIDENCE_GAP",
        }
    }
}

/// Step one: what the requirement is about right now.
///
/// Section 18.2 admits two, and they are different evidence, so they are two
/// variants rather than one struct with two optional halves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentBasis {
    /// `현재 구현`: a `P2-R2` finding over this snapshot.
    ///
    /// The finding's own scope is carried, so a requirement is no wider than
    /// the evidence that started it — `P2-R2` already refused a repository-wide
    /// scope and this inherits that refusal rather than restating it.
    CurrentCode {
        /// Which snapshot the finding is about.
        snapshot_id: String,
        /// The finding's subject.
        subject: String,
        /// Symbol or component. Never the repository.
        scope: FindingScope,
        /// Section 17.4's locators, carried through unchanged.
        sites: Vec<Locator>,
    },
    /// `이미 승인된 기능`: an approved specification or architecture decision.
    ApprovedGoal {
        /// Which snapshot the goal is being read against.
        snapshot_id: String,
        /// The goal and the version of it that is in force.
        goal: GoalScope,
        /// The document that approved it.
        document: DocumentId,
        /// Section 30.3 row five's `최신`.
        revision: u64,
    },
}

impl CurrentBasis {
    /// Takes a `P2-R2` finding as the current-code basis.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::PresentOnlyIsNotAnImplementation`] when the
    /// finding is at [`EvidenceTier::PresentOnly`]. Section 18.1's own example
    /// is a manifest naming `redis` with no import, call or configuration: that
    /// is a dependency entry, not an implementation to understand, maintain or
    /// debug, and section 36.5 has the user correct exactly such an entry as
    /// `template 잔재`.
    pub fn of_current_code(finding: &Finding) -> Result<Self, ClassificationError> {
        if finding.tier() == EvidenceTier::PresentOnly {
            return Err(ClassificationError::PresentOnlyIsNotAnImplementation(
                finding.subject().to_owned(),
            ));
        }
        Ok(Self::CurrentCode {
            snapshot_id: finding.snapshot_id().to_owned(),
            subject: finding.subject().to_owned(),
            scope: finding.scope().clone(),
            sites: finding.locators().to_vec(),
        })
    }

    /// Takes an approved goal as the basis.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::GoalIsNotApproved`] when the document's status is
    /// not [`ApprovalStatus::Approved`]. Section 18.2's words are `이미 승인된
    /// 기능`; a draft is not approved and a deprecated one was withdrawn.
    pub fn of_approved_goal(
        snapshot_id: impl Into<String>,
        goal: GoalScope,
        document: &IntentDocument,
    ) -> Result<Self, ClassificationError> {
        if document.status() != ApprovalStatus::Approved {
            return Err(ClassificationError::GoalIsNotApproved(
                document.id().as_str().to_owned(),
                document.status().as_str(),
            ));
        }
        Ok(Self::ApprovedGoal {
            snapshot_id: snapshot_id.into(),
            goal,
            document: document.id().clone(),
            revision: document.revision(),
        })
    }

    /// Which snapshot this basis was read over.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        match self {
            Self::CurrentCode { snapshot_id, .. } | Self::ApprovedGoal { snapshot_id, .. } => {
                snapshot_id
            }
        }
    }

    /// The finding scope this basis carries, when it came from code.
    #[must_use]
    pub const fn finding_scope(&self) -> Option<&FindingScope> {
        match self {
            Self::CurrentCode { scope, .. } => Some(scope),
            Self::ApprovedGoal { .. } => None,
        }
    }

    /// The goal this basis is under, when it came from an approved goal.
    #[must_use]
    pub const fn goal(&self) -> Option<&GoalScope> {
        match self {
            Self::ApprovedGoal { goal, .. } => Some(goal),
            Self::CurrentCode { .. } => None,
        }
    }

    /// Stable spelling of which arm this is.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CurrentCode { .. } => "CURRENT_CODE",
            Self::ApprovedGoal { .. } => "APPROVED_GOAL",
        }
    }
}

/// Which of section 18.2's two second-step shapes a need is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NeedKind {
    /// `concrete responsibility`: something this code is answerable for.
    Responsibility,
    /// `failure scenario`: something that goes wrong if nothing controls it.
    FailureScenario,
}

impl NeedKind {
    /// Exhaustive order, in section 18.2's own order.
    pub const ALL: [Self; 2] = [Self::Responsibility, Self::FailureScenario];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Responsibility => "RESPONSIBILITY",
            Self::FailureScenario => "FAILURE_SCENARIO",
        }
    }
}

/// Step two, holding step one.
///
/// `concrete` is the load-bearing word. A need with no site in the snapshot is
/// a sentence about the project rather than an observation of it, so
/// [`ConcreteNeed::shown_by`] refuses an empty site list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteNeed {
    basis: CurrentBasis,
    kind: NeedKind,
    name: String,
    sites: Vec<Locator>,
}

impl ConcreteNeed {
    /// Derives a need from the basis that shows it.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::NeedHasNoSite`] when `sites` is empty.
    pub fn shown_by(
        basis: CurrentBasis,
        kind: NeedKind,
        name: &SubjectId,
        sites: Vec<Locator>,
    ) -> Result<Self, ClassificationError> {
        if sites.is_empty() {
            return Err(ClassificationError::NeedHasNoSite(name.as_str().to_owned()));
        }
        Ok(Self {
            basis,
            kind,
            name: name.as_str().to_owned(),
            sites,
        })
    }

    /// Step one.
    #[must_use]
    pub const fn basis(&self) -> &CurrentBasis {
        &self.basis
    }

    /// Responsibility or failure scenario.
    #[must_use]
    pub const fn kind(&self) -> NeedKind {
        self.kind
    }

    /// What the need is called, as the caller's own identifier.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where in the snapshot it is visible.
    #[must_use]
    pub fn sites(&self) -> &[Locator] {
        &self.sites
    }
}

/// Step three, holding step two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllingMechanism {
    need: ConcreteNeed,
    name: String,
}

impl ControllingMechanism {
    /// Names the mechanism that controls `need`.
    ///
    /// The need is taken by value, so a mechanism cannot be recorded as
    /// controlling one thing and then read as controlling another.
    #[must_use]
    pub fn controlling(need: ConcreteNeed, name: &SubjectId) -> Self {
        Self {
            need,
            name: name.as_str().to_owned(),
        }
    }

    /// Step two.
    #[must_use]
    pub const fn need(&self) -> &ConcreteNeed {
        &self.need
    }

    /// What the mechanism is called.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Step four, holding step three.
///
/// The tier is section 7.4's, supplied by the caller the way a [`SubjectId`]
/// is: this crate holds no entity registry and reads no ontology. What it holds
/// is that there is **no** [`RequiredConcept`] anywhere in a program whose tier
/// is `FIELD` or `ALIAS`, because the one constructor refuses both and the
/// fields are private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredConcept {
    mechanism: ControllingMechanism,
    concept: String,
    tier: EntityKind,
}

impl RequiredConcept {
    /// Names the concept the mechanism belongs to.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::TierCannotBeRequired`] for [`EntityKind::Field`]
    /// and [`EntityKind::Alias`]. See the module documentation for why those
    /// two and not the others.
    pub fn realizing(
        mechanism: ControllingMechanism,
        concept: &SubjectId,
        tier: EntityKind,
    ) -> Result<Self, ClassificationError> {
        match tier {
            EntityKind::Field | EntityKind::Alias => {
                Err(ClassificationError::TierCannotBeRequired {
                    concept: concept.as_str().to_owned(),
                    tier,
                })
            }
            EntityKind::Concept | EntityKind::ConceptSense | EntityKind::Operation => Ok(Self {
                mechanism,
                concept: concept.as_str().to_owned(),
                tier,
            }),
        }
    }

    /// Step three.
    #[must_use]
    pub const fn mechanism(&self) -> &ControllingMechanism {
        &self.mechanism
    }

    /// The concept.
    #[must_use]
    pub fn concept(&self) -> &str {
        &self.concept
    }

    /// Its ontology tier, which is never `FIELD` and never `ALIAS`.
    #[must_use]
    pub const fn tier(&self) -> EntityKind {
        self.tier
    }
}

/// Step five: what the user's own evidence for the concept is missing.
///
/// Two variants and no third. See the module documentation for why there is no
/// `SUFFICIENT` value to pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserEvidenceGap {
    /// The user has not applied the concept: section 17.6's `User APPLIED
    /// Concept` claim does not exist yet.
    Insufficient {
        /// The observed depth, which is below [`MasteryLevel::Applied`].
        mastery: MasteryLevel,
    },
    /// The user has applied it, and the claim is stale or is not their own.
    ///
    /// Section 4 names the first half: `2년 전 배운 Virtual Memory는 mastery가
    /// 유지된 채 freshness가 STALE로 보일 수 있고`. The second half is section
    /// 18.4's `AI는 제안하고 사용자는 확인·수정한다` read from the other side —
    /// a depth nobody confirmed is a depth the system inferred.
    Uncertain {
        /// The observed depth, which is [`MasteryLevel::Applied`] or above.
        mastery: MasteryLevel,
        /// The retrieval readiness beside it.
        freshness: FreshnessBand,
        /// Who established the depth.
        status: EpistemicStatus,
    },
}

impl UserEvidenceGap {
    /// Reads a knowledge state and reports the gap in it, if there is one.
    ///
    /// [`None`] is the answer for a state that is applied, fresh and
    /// user-confirmed. It is not an error: a concept the user demonstrably
    /// knows is a concept this project does not require them to learn, and the
    /// caller's chain simply cannot be closed.
    #[must_use]
    pub fn of(
        mastery: MasteryLevel,
        freshness: FreshnessBand,
        status: EpistemicStatus,
    ) -> Option<Self> {
        if mastery < MasteryLevel::Applied {
            return Some(Self::Insufficient { mastery });
        }
        let fresh = freshness >= FreshnessBand::Moderate;
        let confirmed = status == EpistemicStatus::UserConfirmed;
        if fresh && confirmed {
            None
        } else {
            Some(Self::Uncertain {
                mastery,
                freshness,
                status,
            })
        }
    }

    /// Stable spelling of which arm this is.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Insufficient { .. } => "INSUFFICIENT",
            Self::Uncertain { .. } => "UNCERTAIN",
        }
    }

    /// The depth either arm carries.
    #[must_use]
    pub const fn mastery(&self) -> MasteryLevel {
        match self {
            Self::Insufficient { mastery } | Self::Uncertain { mastery, .. } => *mastery,
        }
    }
}

/// The complete five-step chain.
///
/// Private fields, no `Default`, and one constructor that takes the fourth step
/// by value. Every step below it is reachable through [`ProofChain::concept`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofChain {
    concept: RequiredConcept,
    gap: UserEvidenceGap,
}

impl ProofChain {
    /// Closes the chain with the user's own evidence gap.
    #[must_use]
    pub const fn closed_by(concept: RequiredConcept, gap: UserEvidenceGap) -> Self {
        Self { concept, gap }
    }

    /// Step four, and through it steps three, two and one.
    #[must_use]
    pub const fn concept(&self) -> &RequiredConcept {
        &self.concept
    }

    /// Step five.
    #[must_use]
    pub const fn gap(&self) -> UserEvidenceGap {
        self.gap
    }

    /// Step three, for a reader that does not want to walk.
    #[must_use]
    pub const fn mechanism(&self) -> &ControllingMechanism {
        self.concept.mechanism()
    }

    /// Step two.
    #[must_use]
    pub const fn need(&self) -> &ConcreteNeed {
        self.concept.mechanism().need()
    }

    /// Step one.
    #[must_use]
    pub const fn basis(&self) -> &CurrentBasis {
        self.concept.mechanism().need().basis()
    }

    /// Which snapshot the whole chain was read over.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        self.basis().snapshot_id()
    }
}

/// What one model-authored or imported chain offers, before any step is typed.
///
/// The one door from an untyped proposal into a [`ProofChain`], and the only
/// place a missing step is a value rather than a compile error. Each `with_`
/// method takes exactly what the matching link constructor takes beside its
/// predecessor.
#[derive(Debug, Clone, Default)]
pub struct ChainDraft {
    basis: Option<CurrentBasis>,
    need: Option<DraftNeed>,
    mechanism: Option<String>,
    concept: Option<DraftConcept>,
    gap: Option<UserEvidenceGap>,
}

/// Step two's payload as a draft carries it, before it is linked to step one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DraftNeed {
    kind: NeedKind,
    name: String,
    sites: Vec<Locator>,
}

/// Step four's payload as a draft carries it, before its tier is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DraftConcept {
    concept: String,
    tier: EntityKind,
}

impl ChainDraft {
    /// An empty draft, which [`ChainDraft::seal`] refuses at step one.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Offers step one.
    #[must_use]
    pub fn with_basis(mut self, basis: CurrentBasis) -> Self {
        self.basis = Some(basis);
        self
    }

    /// Offers step two.
    #[must_use]
    pub fn with_need(mut self, kind: NeedKind, name: &SubjectId, sites: Vec<Locator>) -> Self {
        self.need = Some(DraftNeed {
            kind,
            name: name.as_str().to_owned(),
            sites,
        });
        self
    }

    /// Offers step three.
    #[must_use]
    pub fn with_mechanism(mut self, name: &SubjectId) -> Self {
        self.mechanism = Some(name.as_str().to_owned());
        self
    }

    /// Offers step four.
    #[must_use]
    pub fn with_concept(mut self, concept: &SubjectId, tier: EntityKind) -> Self {
        self.concept = Some(DraftConcept {
            concept: concept.as_str().to_owned(),
            tier,
        });
        self
    }

    /// Offers step five.
    #[must_use]
    pub const fn with_gap(mut self, gap: UserEvidenceGap) -> Self {
        self.gap = Some(gap);
        self
    }

    /// Links the five steps, or names the first one that is not there.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::ProofChainStepMissing`] carrying the
    /// [`ChainStep`] whose code a blocked publish shows, and every refusal the
    /// link constructors themselves raise.
    pub fn seal(self) -> Result<ProofChain, ClassificationError> {
        let missing = ClassificationError::ProofChainStepMissing;
        let basis = self.basis.ok_or(missing(ChainStep::CurrentBasis))?;
        let need = self.need.ok_or(missing(ChainStep::ConcreteNeed))?;
        let mechanism = self
            .mechanism
            .ok_or(missing(ChainStep::ControllingMechanism))?;
        let concept = self.concept.ok_or(missing(ChainStep::RequiredConcept))?;
        let gap = self.gap.ok_or(missing(ChainStep::UserEvidenceGap))?;

        let linked_need =
            ConcreteNeed::shown_by(basis, need.kind, &SubjectId::new(need.name)?, need.sites)?;
        let linked_mechanism =
            ControllingMechanism::controlling(linked_need, &SubjectId::new(mechanism)?);
        let linked_concept = RequiredConcept::realizing(
            linked_mechanism,
            &SubjectId::new(concept.concept)?,
            concept.tier,
        )?;
        Ok(ProofChain::closed_by(linked_concept, gap))
    }
}
