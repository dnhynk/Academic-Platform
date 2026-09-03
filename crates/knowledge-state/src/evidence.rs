//! Section 13.2's eight rows: what each kind of evidence may license, and no
//! more.
//!
//! ```text
//! transcript에서 meaningful teaching        → Exposed
//! 사용자 자신의 설명 + 자기 확인            → Understood
//! concept-specific 과제 풀이·실험 성공      → Practiced candidate
//! 직접 작성한 project code와 test           → Applied candidate
//! incident debugging에서 원인 규명·수정·검증 → Applied, transfer facet 강화
//! 서로 다른 맥락에서 반복 독립 수행·설계    → Fluent candidate
//! dependency/install/import만 존재          → mastery 승격 없음
//! 과목 grade                                → concept별 직접 승격 없음
//! ```
//!
//! [`CEILINGS`] carries all eight, each with the design document's own middle
//! and right cells verbatim. `evidence_ceilings_are_never_exceeded` parses that
//! table out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compares it against [`CEILINGS`] in both directions, so the row count is a
//! measurement rather than a number written twice.
//!
//! ## The word `candidate` is carried, not interpreted
//!
//! Three of the cells say `candidate` and three do not. Both spellings are kept
//! verbatim in [`CeilingRow::ceiling_cell`] so a reader can see the design's own
//! wording, and in this task the word carries **no rule beyond the ceiling**:
//! the ceiling is the highest level the row's evidence alone may support, for
//! all eight rows alike. `FLUENT`'s extra requirement comes from section 13.1's
//! `AI 단독 판정 금지, 반복된 강한 evidence와 사용자 확인 필요`, which is a
//! different sentence, and it is held by
//! [`crate::confirmation::FluentAuthorization`] rather than by this word.
//!
//! ## The last two rows are refused by two different mechanisms
//!
//! Because the two cells say different things.
//!
//! * `dependency/install/import만 존재 → mastery 승격 없음`. That evidence
//!   *does* name a concept — a `P2-R4` [`ConceptStance`] carries the concept in
//!   its key — so it is a [`ConceptEvidence`] variant whose ceiling is
//!   [`EvidenceCeiling::NoPromotion`]. It is admitted, retained, and shown, and
//!   it raises nothing. [`DependencyOnly::of_stance`] answers only for a stance
//!   whose `observed()` is absent, and `ObservedProof` exists only for a `P2-R2`
//!   finding at `EvidenceTier::Observed`, so the distinction between row four
//!   and row seven is `P2-R4`'s own and not a second ladder here.
//! * `과목 grade → concept별 직접 승격 없음`. A grade is not evidence *about a
//!   concept* at all, so [`CourseGradeSignal`] **has no concept field** and is
//!   not a [`ConceptEvidence`] variant. It cannot be attributed to a concept
//!   because there is nowhere to write the concept down. It is retained as a
//!   broad signal on the assertion, which is `REQ-13-019`'s *grade remains
//!   linked as broad signal*.
//!
//! ## A teaching site is not a string
//!
//! [`TeachingSite::in_document`] takes a `P2-L4` [`LectureDocument`] and a
//! [`NodeId`] and answers only when that document holds that node. A caller who
//! has only a name has no teaching evidence — the discipline `P2-R4` states as
//! *`CurrentBasis` cannot be built from a label*.

use academic_domain::{ContentDigest, EntityId, EvidenceId, MasteryLevel, TimestampMillis};
use academic_lecture_document::{DocumentId, LectureDocument, NodeId};
use academic_repository_classification::{ConceptStance, GoalScope, ObservedProof};
use serde::{Deserialize, Serialize};

use crate::{KnowledgeStateError, confirmation::UserConfirmation, ladder::FacetStrength};

/// Section 13.2's first column, as a closed set of eight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceKind {
    /// `transcript에서 meaningful teaching`.
    MeaningfulTeaching,
    /// `사용자 자신의 설명 + 자기 확인`.
    SelfExplanationConfirmed,
    /// `concept-specific 과제 풀이·실험 성공`.
    ConceptSpecificExercise,
    /// `직접 작성한 production/personal project code와 test`.
    AuthoredProjectCode,
    /// `incident debugging에서 원인 규명·수정·검증`.
    IncidentDebugging,
    /// `서로 다른 맥락에서 반복 독립 수행·설계`.
    RepeatedIndependentTransfer,
    /// `dependency/install/import만 존재`.
    DependencyPresenceOnly,
    /// `과목 grade`.
    CourseGrade,
}

impl EvidenceKind {
    /// Exhaustive order, in section 13.2's own row order.
    pub const ALL: [Self; 8] = [
        Self::MeaningfulTeaching,
        Self::SelfExplanationConfirmed,
        Self::ConceptSpecificExercise,
        Self::AuthoredProjectCode,
        Self::IncidentDebugging,
        Self::RepeatedIndependentTransfer,
        Self::DependencyPresenceOnly,
        Self::CourseGrade,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MeaningfulTeaching => "MEANINGFUL_TEACHING",
            Self::SelfExplanationConfirmed => "SELF_EXPLANATION_CONFIRMED",
            Self::ConceptSpecificExercise => "CONCEPT_SPECIFIC_EXERCISE",
            Self::AuthoredProjectCode => "AUTHORED_PROJECT_CODE",
            Self::IncidentDebugging => "INCIDENT_DEBUGGING",
            Self::RepeatedIndependentTransfer => "REPEATED_INDEPENDENT_TRANSFER",
            Self::DependencyPresenceOnly => "DEPENDENCY_PRESENCE_ONLY",
            Self::CourseGrade => "COURSE_GRADE",
        }
    }

    /// This row's automatic ceiling.
    ///
    /// Total, with no wildcard arm: a ninth kind has to answer this rather than
    /// inherit an answer.
    #[must_use]
    pub const fn ceiling(self) -> EvidenceCeiling {
        match self {
            Self::MeaningfulTeaching => EvidenceCeiling::UpTo(MasteryLevel::Exposed),
            Self::SelfExplanationConfirmed => EvidenceCeiling::UpTo(MasteryLevel::Understood),
            Self::ConceptSpecificExercise => EvidenceCeiling::UpTo(MasteryLevel::Practiced),
            Self::AuthoredProjectCode | Self::IncidentDebugging => {
                EvidenceCeiling::UpTo(MasteryLevel::Applied)
            }
            Self::RepeatedIndependentTransfer => EvidenceCeiling::UpTo(MasteryLevel::Fluent),
            Self::DependencyPresenceOnly | Self::CourseGrade => EvidenceCeiling::NoPromotion,
        }
    }
}

/// The highest mastery one row's evidence alone may support.
///
/// Two variants and no third. There is deliberately no `UpTo(Unseen)`: a row
/// that licenses nothing is not a row that licenses the floor, the way
/// `academic_domain::predicates::personal_mastery_ceiling` refuses a predicate
/// that bears no personal state rather than answering with a floor value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "level", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceCeiling {
    /// The row licenses no promotion at all.
    NoPromotion,
    /// The row licenses at most this level.
    UpTo(MasteryLevel),
}

impl EvidenceCeiling {
    /// Whether this ceiling admits `level`.
    ///
    /// [`Self::NoPromotion`] admits only `UNSEEN`, which is not a promotion.
    /// The comparison is over [`crate::ladder::rung`], which is section 13.1's
    /// own `Level` column, rather than over a discriminant this crate does not
    /// declare.
    #[must_use]
    pub const fn admits(self, level: MasteryLevel) -> bool {
        match self {
            Self::NoPromotion => matches!(level, MasteryLevel::Unseen),
            Self::UpTo(ceiling) => crate::ladder::rung(level) <= crate::ladder::rung(ceiling),
        }
    }
}

/// One row of section 13.2, with both of its text cells kept verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeilingRow {
    /// The row's evidence kind.
    pub kind: EvidenceKind,
    /// The design document's `허용되는 기본 해석` cell, verbatim.
    pub interpretation: &'static str,
    /// The design document's `자동 상한` cell, verbatim.
    pub ceiling_cell: &'static str,
    /// The ceiling that cell fixes.
    pub ceiling: EvidenceCeiling,
}

/// Section 13.2's table, row for row and cell for cell.
pub const CEILINGS: [CeilingRow; 8] = [
    CeilingRow {
        kind: EvidenceKind::MeaningfulTeaching,
        interpretation: "접함",
        ceiling_cell: "Exposed",
        ceiling: EvidenceCeiling::UpTo(MasteryLevel::Exposed),
    },
    CeilingRow {
        kind: EvidenceKind::SelfExplanationConfirmed,
        interpretation: "설명 가능",
        ceiling_cell: "Understood",
        ceiling: EvidenceCeiling::UpTo(MasteryLevel::Understood),
    },
    CeilingRow {
        kind: EvidenceKind::ConceptSpecificExercise,
        interpretation: "구조화된 적용",
        ceiling_cell: "Practiced candidate",
        ceiling: EvidenceCeiling::UpTo(MasteryLevel::Practiced),
    },
    CeilingRow {
        kind: EvidenceKind::AuthoredProjectCode,
        interpretation: "현실 맥락 적용",
        ceiling_cell: "Applied candidate",
        ceiling: EvidenceCeiling::UpTo(MasteryLevel::Applied),
    },
    CeilingRow {
        kind: EvidenceKind::IncidentDebugging,
        interpretation: "진단과 적용",
        ceiling_cell: "Applied, transfer facet 강화",
        ceiling: EvidenceCeiling::UpTo(MasteryLevel::Applied),
    },
    CeilingRow {
        kind: EvidenceKind::RepeatedIndependentTransfer,
        interpretation: "전이 가능성",
        ceiling_cell: "Fluent candidate",
        ceiling: EvidenceCeiling::UpTo(MasteryLevel::Fluent),
    },
    CeilingRow {
        kind: EvidenceKind::DependencyPresenceOnly,
        interpretation: "기술 접점",
        ceiling_cell: "mastery 승격 없음",
        ceiling: EvidenceCeiling::NoPromotion,
    },
    CeilingRow {
        kind: EvidenceKind::CourseGrade,
        interpretation: "광범위한 performance signal",
        ceiling_cell: "concept별 직접 승격 없음",
        ceiling: EvidenceCeiling::NoPromotion,
    },
];

/// Section 13.2's first row: a node of a real `P2-L4` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeachingSite {
    document: String,
    node: String,
    document_digest: ContentDigest,
}

impl TeachingSite {
    /// Names a node of `document`.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::TeachingSiteNotInDocument`] when `document` holds
    /// no node with that identifier. A caller holding only a name has nothing
    /// to pass here.
    pub fn in_document(
        document: &LectureDocument,
        node: &NodeId,
    ) -> Result<Self, KnowledgeStateError> {
        if !document.nodes().iter().any(|held| held.id() == node) {
            return Err(KnowledgeStateError::TeachingSiteNotInDocument {
                document: document.id().as_str().to_owned(),
                node: node.as_str().to_owned(),
            });
        }
        Ok(Self {
            document: document.id().as_str().to_owned(),
            node: node.as_str().to_owned(),
            document_digest: document.digest(),
        })
    }

    /// Which document.
    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }

    /// Which node of it.
    #[must_use]
    pub fn node(&self) -> &str {
        &self.node
    }

    /// The document digest at the version this site was taken from.
    #[must_use]
    pub const fn document_digest(&self) -> &ContentDigest {
        &self.document_digest
    }
}

/// Section 13.2's second row: the user's own explanation plus their own
/// confirmation.
///
/// Both halves are required by the constructor. The confirmation is a
/// [`UserConfirmation`], which only `Actor::User` can mint, so `자기 확인`
/// cannot be supplied by a model run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfExplanation {
    artifact: EvidenceId,
    confirmed_at: TimestampMillis,
}

impl SelfExplanation {
    /// Records an explanation the user wrote and then confirmed.
    #[must_use]
    pub fn confirmed_by(artifact: EvidenceId, confirmation: &UserConfirmation) -> Self {
        Self {
            artifact,
            confirmed_at: confirmation.confirmed_at(),
        }
    }

    /// Which artifact holds the explanation.
    #[must_use]
    pub const fn artifact(&self) -> EvidenceId {
        self.artifact
    }

    /// When the user confirmed it.
    #[must_use]
    pub const fn confirmed_at(&self) -> TimestampMillis {
        self.confirmed_at
    }
}

/// Section 13.2's third row: one attempt at a concept-specific exercise.
///
/// The row's cell says `과제 풀이·실험 **성공**`, so an attempt that did not
/// succeed is not this row's evidence. It is still recorded: an unsuccessful
/// attempt reaches the assertion's contradicting-evidence list, which is what
/// keeps `UNSEEN` and *tried and failed* two different states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExerciseOutcome {
    artifact: EvidenceId,
    succeeded: bool,
}

impl ExerciseOutcome {
    /// Records a successful attempt.
    #[must_use]
    pub const fn succeeded(artifact: EvidenceId) -> Self {
        Self {
            artifact,
            succeeded: true,
        }
    }

    /// Records an attempt that did not succeed.
    #[must_use]
    pub const fn failed(artifact: EvidenceId) -> Self {
        Self {
            artifact,
            succeeded: false,
        }
    }

    /// Whether the attempt succeeded.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.succeeded
    }

    /// Which artifact holds it.
    #[must_use]
    pub const fn artifact(&self) -> EvidenceId {
        self.artifact
    }
}

/// Section 13.2's fourth row: a `P2-R4` stance that observed a use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectUse {
    concept: String,
    goal: GoalScope,
    snapshot: String,
    proof: ObservedProof,
}

impl ProjectUse {
    /// Reads a stance, and answers only for one carrying an `OBSERVED` proof.
    ///
    /// The proof is `P2-R4`'s and is carried unchanged. There is no second
    /// ladder here: `ObservedProof` exists only for a `P2-R2` finding at
    /// `EvidenceTier::Observed`, so a manifest-only stance has none and this
    /// constructor answers [`None`] for it.
    #[must_use]
    pub fn of_stance(stance: &ConceptStance) -> Option<Self> {
        stance.observed().map(|proof| Self {
            concept: stance.key().concept().to_owned(),
            goal: stance.key().goal().clone(),
            snapshot: stance.key().snapshot_id().to_owned(),
            proof: proof.clone(),
        })
    }

    /// Which concept the stance is about.
    #[must_use]
    pub fn concept(&self) -> &str {
        &self.concept
    }

    /// Which goal version.
    #[must_use]
    pub const fn goal(&self) -> &GoalScope {
        &self.goal
    }

    /// Which snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &str {
        &self.snapshot
    }

    /// `P2-R4`'s proof, unchanged.
    #[must_use]
    pub const fn proof(&self) -> &ObservedProof {
        &self.proof
    }
}

/// Section 13.2's seventh row: a stance that named a concept and observed
/// nothing.
///
/// The complement of [`ProjectUse::of_stance`] over the same input, which is
/// what makes the pair exhaustive over a stance rather than two heuristics that
/// might both answer or neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyOnly {
    concept: String,
    goal: GoalScope,
    snapshot: String,
}

impl DependencyOnly {
    /// Reads a stance, and answers only for one with no `OBSERVED` proof.
    #[must_use]
    pub fn of_stance(stance: &ConceptStance) -> Option<Self> {
        if stance.observed().is_some() {
            return None;
        }
        Some(Self {
            concept: stance.key().concept().to_owned(),
            goal: stance.key().goal().clone(),
            snapshot: stance.key().snapshot_id().to_owned(),
        })
    }

    /// Which concept.
    #[must_use]
    pub fn concept(&self) -> &str {
        &self.concept
    }

    /// Which goal version.
    #[must_use]
    pub const fn goal(&self) -> &GoalScope {
        &self.goal
    }

    /// Which snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &str {
        &self.snapshot
    }
}

/// Section 13.2's fifth row: an incident whose cause was found, fixed and
/// verified.
///
/// All three parts are constructor arguments, because the cell names all three.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentRepair {
    incident: EvidenceId,
    root_cause: EvidenceId,
    fix: EvidenceId,
    verification: EvidenceId,
}

impl IncidentRepair {
    /// Records `원인 규명·수정·검증` as three separate evidence items.
    #[must_use]
    pub const fn of(
        incident: EvidenceId,
        root_cause: EvidenceId,
        fix: EvidenceId,
        verification: EvidenceId,
    ) -> Self {
        Self {
            incident,
            root_cause,
            fix,
            verification,
        }
    }

    /// Which incident.
    #[must_use]
    pub const fn incident(&self) -> EvidenceId {
        self.incident
    }

    /// The cause finding.
    #[must_use]
    pub const fn root_cause(&self) -> EvidenceId {
        self.root_cause
    }

    /// The repair.
    #[must_use]
    pub const fn fix(&self) -> EvidenceId {
        self.fix
    }

    /// The verification of the repair.
    #[must_use]
    pub const fn verification(&self) -> EvidenceId {
        self.verification
    }

    /// How much this row strengthens the transfer facet.
    ///
    /// The cell is `Applied, transfer facet 강화`, so the row raises the facet
    /// rather than setting it: see [`crate::ladder::FacetProfile::with_transfer_at_least`].
    #[must_use]
    pub const fn transfer_strengthening(&self) -> FacetStrength {
        FacetStrength::Moderate
    }
}

/// Section 13.2's eighth row, and the one type here with no concept field.
///
/// `과목 grade → concept별 직접 승격 없음`. A grade is a course-level signal, so
/// there is nowhere on this value to write a concept down and no
/// [`ConceptEvidence`] variant that holds one. It is retained on the assertion
/// as a broad signal and never enters a promotion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseGradeSignal {
    course: String,
    term: String,
    grade: String,
    artifact: EvidenceId,
}

impl CourseGradeSignal {
    /// Records a course grade.
    #[must_use]
    pub fn recorded(
        course: impl Into<String>,
        term: impl Into<String>,
        grade: impl Into<String>,
        artifact: EvidenceId,
    ) -> Self {
        Self {
            course: course.into(),
            term: term.into(),
            grade: grade.into(),
            artifact,
        }
    }

    /// Which course.
    #[must_use]
    pub fn course(&self) -> &str {
        &self.course
    }

    /// Which term.
    #[must_use]
    pub fn term(&self) -> &str {
        &self.term
    }

    /// The grade as recorded.
    #[must_use]
    pub fn grade(&self) -> &str {
        &self.grade
    }

    /// Which artifact holds it.
    #[must_use]
    pub const fn artifact(&self) -> EvidenceId {
        self.artifact
    }

    /// This signal's row in section 13.2.
    #[must_use]
    pub const fn kind(&self) -> EvidenceKind {
        EvidenceKind::CourseGrade
    }
}

/// One piece of evidence about one concept.
///
/// Seven variants for section 13.2's first seven rows. The eighth row has no
/// variant, because a grade cannot be attributed to a concept: see
/// [`CourseGradeSignal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConceptEvidence {
    /// Row one.
    MeaningfulTeaching(TeachingSite),
    /// Row two.
    SelfExplanation(SelfExplanation),
    /// Row three.
    ConceptExercise(ExerciseOutcome),
    /// Row four.
    AuthoredProjectCode(ProjectUse),
    /// Row five.
    IncidentDebugging(IncidentRepair),
    /// Row six. The repetition proof is `P2-N2`'s
    /// [`crate::confirmation::TransferRepetition`].
    RepeatedTransfer(crate::confirmation::TransferRepetition),
    /// Row seven.
    DependencyPresence(DependencyOnly),
}

impl ConceptEvidence {
    /// Which of section 13.2's rows this is.
    ///
    /// Total, with no wildcard arm.
    #[must_use]
    pub const fn kind(&self) -> EvidenceKind {
        match self {
            Self::MeaningfulTeaching(_) => EvidenceKind::MeaningfulTeaching,
            Self::SelfExplanation(_) => EvidenceKind::SelfExplanationConfirmed,
            Self::ConceptExercise(_) => EvidenceKind::ConceptSpecificExercise,
            Self::AuthoredProjectCode(_) => EvidenceKind::AuthoredProjectCode,
            Self::IncidentDebugging(_) => EvidenceKind::IncidentDebugging,
            Self::RepeatedTransfer(_) => EvidenceKind::RepeatedIndependentTransfer,
            Self::DependencyPresence(_) => EvidenceKind::DependencyPresenceOnly,
        }
    }

    /// This evidence's automatic ceiling, which is its row's.
    #[must_use]
    pub const fn ceiling(&self) -> EvidenceCeiling {
        self.kind().ceiling()
    }

    /// Whether this item contradicts a promotion rather than supporting one.
    ///
    /// Only an unsuccessful exercise attempt does, and it is the reason
    /// `UNSEEN` and *tried and failed* are two different projections.
    #[must_use]
    pub const fn contradicts(&self) -> bool {
        match self {
            Self::ConceptExercise(outcome) => !outcome.is_success(),
            Self::MeaningfulTeaching(_)
            | Self::SelfExplanation(_)
            | Self::AuthoredProjectCode(_)
            | Self::IncidentDebugging(_)
            | Self::RepeatedTransfer(_)
            | Self::DependencyPresence(_) => false,
        }
    }
}

/// A grade or another course-wide signal, kept beside the concept graph.
///
/// `REQ-13-019`: the grade is retained and linked, and it promotes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BroadSignal {
    signal: CourseGradeSignal,
}

impl BroadSignal {
    /// Wraps a grade so it can be carried on an assertion without a concept.
    #[must_use]
    pub const fn of_grade(signal: CourseGradeSignal) -> Self {
        Self { signal }
    }

    /// The grade, unchanged.
    #[must_use]
    pub const fn grade(&self) -> &CourseGradeSignal {
        &self.signal
    }
}

/// The concept a piece of evidence is about, as the ontology answered it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConceptRef {
    id: EntityId,
}

impl ConceptRef {
    /// Names a concept.
    #[must_use]
    pub const fn new(id: EntityId) -> Self {
        Self { id }
    }

    /// The stable identity.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        self.id
    }
}

/// A document identifier carried for disclosure, as `P2-L4` spells it.
#[must_use]
pub fn document_name(id: &DocumentId) -> String {
    id.as_str().to_owned()
}
