//! Section 17.6's last sentence: `따라서 ProjectSnapshot OBSERVES Concept과
//! User APPLIED Concept은 다른 Claim이다`.
//!
//! Two types, two identity namespaces, two provenance records, and no field of
//! either that is a field of the other. What makes them independent is not a
//! rule about how they are used — it is that neither is reachable from the
//! other except by an identifier a reader can follow.
//!
//! ## The two identities are domain-separated digests
//!
//! [`ClaimId`] admits 64 bytes, which is exactly one hexadecimal SHA-256
//! digest. A snapshot identifier is most of 64 bytes on its own, so an identity
//! built by joining the facts a claim binds and truncating to fit would drop
//! the last of them — and two claims differing only there would share one
//! identity, silently. `P2-R4` shipped exactly that defect and measured it:
//! its materialized requirement's identity was four facts joined with `.` and
//! cut to 64 bytes, so two requirements differing only in goal version
//! collided. It is the same shape this Run's `P2-A1` fifth audit raised as a
//! P1: content standing in for identity.
//!
//! So both identities here are digests, and the two domain strings are
//! different. Three things follow and
//! `two_claims_have_independent_ids_and_provenance` measures all three:
//!
//! * a project claim and a personal claim over the *same* snapshot, goal,
//!   version and concept have different identifiers, because the domain
//!   separator differs;
//! * two personal claims differing only in the last bound fact have different
//!   identifiers, because nothing is truncated; and
//! * neither identity is any part of the other's preimage, so one cannot be
//!   derived from the other by a reader who has only one of them.
//!
//! ## Rejecting one leaves the other
//!
//! [`PersonalApplicationClaim::rejected`] **consumes** the claim and returns a
//! new one, the way `P2-R4`'s requirement lifecycle does, and it touches
//! nothing else. A [`ProjectObservationClaim`] has no rejection at all: what
//! the snapshot contains is not a thing a judgement about the user can retract.
//! Section 13.2's own sentence for the other direction — `철회 event도 역사에
//! 남고 projection만 다시 계산한다` — is why the rejection is a new value with
//! the reason on it rather than a deletion.

use academic_domain::MasteryLevel;
use academic_policy::ContentDigest;
use academic_repository_analysis::{ArtifactScope, EvidenceTier, LadderRung, Locator};
use academic_repository_classification::{ClassificationKey, ObservedProof};
use academic_repository_correlation::EvidenceRelation;

use crate::{
    CompetencyError,
    contribution::{AuthoredWork, AuthorshipMode, ChangeId},
    generated::CodeOrigin,
    identity::{ExternalAuthorId, UserId},
    outcome::{CandidateSupport, OutcomeArtifact, OutcomeKind},
    rubric::{ChangedSite, RubricId},
};

/// The identity of one claim, of either kind.
///
/// One type for both, because the two are told apart by the namespace their
/// preimage was computed in and not by the shape of the string. A reader
/// holding a bare identifier cannot tell which kind it is, and that is
/// deliberate: it means nothing in this crate decides anything by looking at
/// one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimId {
    identifier: String,
}

impl ClaimId {
    /// Validates and takes a claim identifier.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when it is empty, over 64 bytes,
    /// or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self {
            identifier: crate::identity::validated(value.into(), "claim")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// The domain string a project observation's identity is computed in.
const PROJECT_CLAIM_DOMAIN: &[u8] = b"academic-repository-competency-project-observes-v1\0";

/// The domain string a personal application's identity is computed in.
const PERSONAL_CLAIM_DOMAIN: &[u8] = b"academic-repository-competency-user-applied-v1\0";

/// A domain-separated digest over `parts`.
///
/// The zero separator holds because no part can contain a zero byte: every
/// identifier this crate admits is `[A-Za-z0-9._-]`, a snapshot identifier is
/// `academic-repository`'s own `snap_`-prefixed text, and a version is rendered
/// as decimal digits rather than as its little-endian bytes.
fn identity(domain: &[u8], parts: &[&str]) -> String {
    let mut preimage = domain.to_vec();
    for part in parts {
        preimage.extend_from_slice(part.as_bytes());
        preimage.push(0);
    }
    ContentDigest::of(&preimage).as_str().to_owned()
}

/// The identity of one `ProjectSnapshot OBSERVES Concept`.
pub(crate) fn project_claim_identity(
    snapshot_id: &str,
    goal: &str,
    version: u64,
    concept: &str,
) -> String {
    identity(
        PROJECT_CLAIM_DOMAIN,
        &[snapshot_id, goal, &version.to_string(), concept],
    )
}

/// The identity of one `User APPLIED Concept`.
///
/// Binds the user and the change beside the four the project claim binds: the
/// same user applying the same concept in two changes is two claims, and two
/// users cannot share one.
pub(crate) fn personal_claim_identity(
    user: &str,
    snapshot_id: &str,
    goal: &str,
    version: u64,
    concept: &str,
    change: &str,
) -> String {
    identity(
        PERSONAL_CLAIM_DOMAIN,
        &[
            user,
            snapshot_id,
            goal,
            &version.to_string(),
            concept,
            change,
        ],
    )
}

/// What the repository was observed to do, and how that was seen.
///
/// Every field comes out of `P2-R3`'s edge by way of `P2-R4`'s
/// [`ObservedProof`], so this record cannot disagree with the classification it
/// was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectProvenance {
    relation: EvidenceRelation,
    rung: LadderRung,
    tier: EvidenceTier,
    artifact_scope: ArtifactScope,
    locators: Vec<Locator>,
}

impl ProjectProvenance {
    /// Reads a `P2-R4` observation.
    pub(crate) fn of_proof(proof: &ObservedProof) -> Self {
        Self {
            relation: proof.relation(),
            rung: proof.rung(),
            tier: proof.tier(),
            artifact_scope: proof.artifact_scope(),
            locators: proof.locators().to_vec(),
        }
    }

    /// Which of section 17.5's relations observed the use.
    #[must_use]
    pub const fn relation(&self) -> EvidenceRelation {
        self.relation
    }

    /// Which of section 17.3's observations produced it.
    #[must_use]
    pub const fn rung(&self) -> LadderRung {
        self.rung
    }

    /// The evidence tier, always [`EvidenceTier::Observed`].
    #[must_use]
    pub const fn tier(&self) -> EvidenceTier {
        self.tier
    }

    /// Section 18.1's scope of the use.
    #[must_use]
    pub const fn artifact_scope(&self) -> ArtifactScope {
        self.artifact_scope
    }

    /// Section 17.4's locators, carried through unchanged.
    #[must_use]
    pub fn locators(&self) -> &[Locator] {
        &self.locators
    }
}

/// `ProjectSnapshot OBSERVES Concept`.
///
/// It says what the snapshot contains. It says nothing about the user, and
/// there is no field here that could: no author, no mode, no rubric, no
/// outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectObservationClaim {
    id: ClaimId,
    key: ClassificationKey,
    provenance: ProjectProvenance,
}

impl ProjectObservationClaim {
    /// Materializes one observation. Crate-private: [`crate::promote`] is the
    /// one producer.
    pub(crate) const fn seal(
        id: ClaimId,
        key: ClassificationKey,
        provenance: ProjectProvenance,
    ) -> Self {
        Self {
            id,
            key,
            provenance,
        }
    }

    /// Its own identity, in the project namespace.
    #[must_use]
    pub const fn id(&self) -> &ClaimId {
        &self.id
    }

    /// Snapshot, goal version and concept, from `P2-R4`.
    #[must_use]
    pub const fn key(&self) -> &ClassificationKey {
        &self.key
    }

    /// Which concept it is about.
    #[must_use]
    pub fn concept(&self) -> &str {
        self.key.concept()
    }

    /// How the snapshot was seen to use it.
    #[must_use]
    pub const fn provenance(&self) -> &ProjectProvenance {
        &self.provenance
    }

    /// The predicate a reader is shown.
    #[must_use]
    pub const fn predicate(&self) -> &'static str {
        "OBSERVES"
    }
}

/// What the user did, and under which configuration it was judged.
///
/// Every field comes out of an [`AuthoredWork`] or out of the outcomes offered
/// beside it, and none of them comes out of the observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalProvenance {
    user: UserId,
    author: ExternalAuthorId,
    mapping_version: u64,
    change: ChangeId,
    mode: AuthorshipMode,
    origin: CodeOrigin,
    rubric: RubricId,
    rubric_version: u64,
    bearing_sites: Vec<ChangedSite>,
    outcomes: Vec<OutcomeKind>,
    observed_by: ClaimId,
}

impl PersonalProvenance {
    /// Whose claim it is.
    #[must_use]
    pub const fn user(&self) -> &UserId {
        &self.user
    }

    /// The external identity the mapping resolved to that user.
    #[must_use]
    pub const fn author(&self) -> &ExternalAuthorId {
        &self.author
    }

    /// Which version of the authorship mapping admitted it.
    #[must_use]
    pub const fn mapping_version(&self) -> u64 {
        self.mapping_version
    }

    /// Which change carried the work.
    #[must_use]
    pub const fn change(&self) -> &ChangeId {
        &self.change
    }

    /// What the user did. Never a review.
    #[must_use]
    pub const fn mode(&self) -> AuthorshipMode {
        self.mode
    }

    /// Where the code came from, with the warrant when it was generated.
    #[must_use]
    pub const fn origin(&self) -> &CodeOrigin {
        &self.origin
    }

    /// Which scaffold rubric judged the change.
    #[must_use]
    pub const fn rubric(&self) -> &RubricId {
        &self.rubric
    }

    /// Which version of it.
    #[must_use]
    pub const fn rubric_version(&self) -> u64 {
        self.rubric_version
    }

    /// The sites that rubric counted.
    #[must_use]
    pub fn bearing_sites(&self) -> &[ChangedSite] {
        &self.bearing_sites
    }

    /// Which result evidences were beside it, in enumeration order.
    #[must_use]
    pub fn outcomes(&self) -> &[OutcomeKind] {
        &self.outcomes
    }

    /// The project claim this was promoted from, by identifier.
    ///
    /// An identifier and not the value: the two claims are independent, and a
    /// personal claim that embedded the project one could be read as speaking
    /// for it.
    #[must_use]
    pub const fn observed_by(&self) -> &ClaimId {
        &self.observed_by
    }
}

/// Whether a personal claim stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimStanding {
    /// Section 13.2's `Applied candidate`: offered, not yet confirmed.
    Candidate,
    /// The user or a later review took it back.
    Rejected {
        /// Why.
        reason: RejectionReason,
        /// When.
        at: u64,
    },
}

impl ClaimStanding {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Candidate => "CANDIDATE",
            Self::Rejected { .. } => "REJECTED",
        }
    }
}

/// Why a personal claim was taken back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RejectionReason {
    /// The user says it was not their work after all.
    NotTheUsersWork,
    /// The user says the change did not need the understanding it was credited
    /// with.
    ChangeWasNotMeaningful,
    /// Section 13.2's `제출한 과제가 타인의 풀이를 복사한 것이라면 evidence를
    /// 철회할 수 있다`, one domain over.
    EvidenceWithdrawn,
}

impl RejectionReason {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [
        Self::NotTheUsersWork,
        Self::ChangeWasNotMeaningful,
        Self::EvidenceWithdrawn,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotTheUsersWork => "NOT_THE_USERS_WORK",
            Self::ChangeWasNotMeaningful => "CHANGE_WAS_NOT_MEANINGFUL",
            Self::EvidenceWithdrawn => "EVIDENCE_WITHDRAWN",
        }
    }
}

/// `User APPLIED Concept`.
///
/// A **candidate**, which is section 13.2's own ceiling for `직접 작성한
/// production/personal project code와 test`. This crate produces no confirmed
/// state: section 13.4's `user accept / edit / leave unconfirmed / reject` is a
/// decision the user makes and not one an analyzer makes for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalApplicationClaim {
    id: ClaimId,
    key: ClassificationKey,
    provenance: PersonalProvenance,
    support: CandidateSupport,
    standing: ClaimStanding,
}

impl PersonalApplicationClaim {
    /// Materializes one promotion. Crate-private: [`crate::promote`] is the one
    /// producer.
    pub(crate) fn seal(
        id: ClaimId,
        key: ClassificationKey,
        work: &AuthoredWork,
        outcomes: &[&OutcomeArtifact],
        observed_by: ClaimId,
    ) -> Self {
        let mut kinds: Vec<OutcomeKind> = outcomes.iter().map(|item| item.kind()).collect();
        kinds.sort_unstable();
        kinds.dedup();
        Self {
            id,
            key,
            provenance: PersonalProvenance {
                user: work.user().clone(),
                author: work.author().clone(),
                mapping_version: work.mapping_version(),
                change: work.change().clone(),
                mode: work.mode(),
                origin: work.origin().clone(),
                rubric: work.verdict().rubric().clone(),
                rubric_version: work.verdict().version(),
                bearing_sites: work.bearing_sites().to_vec(),
                outcomes: kinds,
                observed_by,
            },
            support: CandidateSupport::of(outcomes),
            standing: ClaimStanding::Candidate,
        }
    }

    /// Its own identity, in the personal namespace.
    #[must_use]
    pub const fn id(&self) -> &ClaimId {
        &self.id
    }

    /// Snapshot, goal version and concept, the same three `P2-R4` bound.
    #[must_use]
    pub const fn key(&self) -> &ClassificationKey {
        &self.key
    }

    /// Which concept it is about.
    #[must_use]
    pub fn concept(&self) -> &str {
        self.key.concept()
    }

    /// What the user did, under which configuration, with what beside it.
    #[must_use]
    pub const fn provenance(&self) -> &PersonalProvenance {
        &self.provenance
    }

    /// How much is carrying it, in section 13.2's terms.
    #[must_use]
    pub const fn support(&self) -> CandidateSupport {
        self.support
    }

    /// Whether it stands.
    #[must_use]
    pub const fn standing(&self) -> &ClaimStanding {
        &self.standing
    }

    /// The predicate a reader is shown.
    #[must_use]
    pub const fn predicate(&self) -> &'static str {
        "APPLIED"
    }

    /// Section 13.1's mastery level this candidate is offered at.
    ///
    /// Always [`MasteryLevel::Applied`], and always as a candidate: section
    /// 13.1's own ceiling note for that level is `dependency 설치만으로 금지`,
    /// and its `FLUENT` row is `AI 단독 판정 금지`, so nothing here proposes a
    /// level above it.
    #[must_use]
    pub const fn offered_at(&self) -> MasteryLevel {
        MasteryLevel::Applied
    }

    /// Takes the claim back, consuming it.
    ///
    /// No `&mut self`, the way `P2-R4`'s requirement lifecycle has none: what
    /// comes back is a new value, and the project claim it was promoted from is
    /// not an argument here and cannot be reached from here.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::ClaimAlreadyRejected`] when it has already been taken
    /// back.
    pub fn rejected(self, reason: RejectionReason, at: u64) -> Result<Self, CompetencyError> {
        if let ClaimStanding::Rejected { .. } = self.standing {
            return Err(CompetencyError::ClaimAlreadyRejected(
                self.id.as_str().to_owned(),
            ));
        }
        Ok(Self {
            standing: ClaimStanding::Rejected { reason, at },
            ..self
        })
    }
}
