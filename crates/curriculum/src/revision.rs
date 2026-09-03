//! Section 8.2's `CourseRevision`: the catalogue definition that is valid over
//! an interval.
//!
//! # What is not here
//!
//! Section 9's boundary table gives `CourseRevision` one row and states what it
//! does not contain: *특정 분반의 현실* — the reality of a particular section.
//! Every field of section 8.2's `CourseOffering` block is that reality, and not
//! one of them has a setter on [`CourseRevisionDraft`].
//! `tests/compile_fail/revision_boundary_rejects_section_fields.rs` observes it,
//! and the scan reads section 8.2's `CourseOffering` block out of the
//! specification so the Rust list cannot drift below it.
//!
//! # `GATE-38-018` is two lists, not one comparison
//!
//! Section 38.2 asks for *Course별 공식 prerequisite와 담당교수의 권장 선수지식
//! 차이* — the difference between a course's official prerequisite and the
//! instructor's recommended prior knowledge. That difference needs a reviewed
//! source, which does not exist yet, so this revision carries the two as
//! separate typed lists that are never compared here:
//! [`CourseRevision::official_prerequisites`] and
//! [`CourseRevision::recommended_prerequisites`]. There is no function from one
//! to the other and no field holding a verdict. See [`crate::gate`].
//!
//! `GATE-38-013` (the recognition list) and `GATE-38-014` (substitution rules)
//! bite on [`CurriculumCategory`] and on
//! [`crate::relation::EquivalenceRelation`]: a revision whose official category
//! has not been confirmed reads [`CurriculumCategory::Unknown`], which is the
//! absence of a record rather than a chosen value.

use academic_domain::{
    ContentDigest, CourseId, CourseRevisionId, CurriculumVersionId, EntityId, ValidInterval,
};

use crate::{
    error::CurriculumError,
    text::{CourseCode, CourseTitle},
};

/// Section 8.2's `curriculumCategory`.
///
/// `Unknown` is what an unconfirmed category reads as. `GATE-38-013` and
/// `GATE-38-014` are open, so a revision whose official classification has not
/// been confirmed against a reviewed source holds this value; nothing infers a
/// category from a code, a credit count, or a sibling revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CurriculumCategory {
    /// The official confirmation has not been recorded. Not a default value.
    Unknown,
    /// 전공필수.
    MajorRequired,
    /// 전공선택.
    MajorElective,
    /// 교양.
    GeneralStudies,
    /// 일반선택.
    GeneralElective,
    /// 비교과 / non-credit completion requirement.
    NonCredit,
}

impl CurriculumCategory {
    /// Exhaustive listing, `Unknown` first.
    pub const ALL: [Self; 6] = [
        Self::Unknown,
        Self::MajorRequired,
        Self::MajorElective,
        Self::GeneralStudies,
        Self::GeneralElective,
        Self::NonCredit,
    ];

    /// Stable spelling, which is also migration 0014's `CHECK` vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::MajorRequired => "MAJOR_REQUIRED",
            Self::MajorElective => "MAJOR_ELECTIVE",
            Self::GeneralStudies => "GENERAL_STUDIES",
            Self::GeneralElective => "GENERAL_ELECTIVE",
            Self::NonCredit => "NON_CREDIT",
        }
    }

    /// Whether the official classification has been recorded.
    #[must_use]
    pub const fn is_known(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// Credits as the catalogue states them: a whole number, never a float.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Credits(u8);

impl Credits {
    /// The only constructor. Zero is admitted: a non-credit completion
    /// requirement is a real catalogue row.
    pub fn new(value: u8) -> Result<Self, CurriculumError> {
        if value > 30 {
            return Err(CurriculumError::Malformed {
                field: "credits",
                reason: "a catalogue row does not carry more than 30 credits",
            });
        }
        Ok(Self(value))
    }

    /// The stated credits.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// One entry of section 8.2's `officialPrerequisiteRules`.
///
/// The prerequisite is a course identity, never a sentence: section 2.3-3 keeps
/// structured values out of free text, and `P2-C4`'s registry says the same of
/// every qualifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OfficialPrerequisite {
    course: CourseId,
}

impl OfficialPrerequisite {
    /// Records that the official catalogue names `course` as a prerequisite.
    #[must_use]
    pub const fn on(course: CourseId) -> Self {
        Self { course }
    }

    /// Which course.
    #[must_use]
    pub const fn course(self) -> CourseId {
        self.course
    }
}

/// One entry of section 8.2's `recommendedPrerequisiteClaims`.
///
/// A different type from [`OfficialPrerequisite`] on purpose. `GATE-38-018` is
/// the open question of how the two differ, and a shared type would have
/// answered it by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecommendedPrerequisite {
    course: CourseId,
}

impl RecommendedPrerequisite {
    /// Records that an instructor recommends `course` beforehand.
    #[must_use]
    pub const fn on(course: CourseId) -> Self {
        Self { course }
    }

    /// Which course.
    #[must_use]
    pub const fn course(self) -> CourseId {
        self.course
    }
}

/// Section 8.2's `CourseRevision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseRevision {
    id: CourseRevisionId,
    course: CourseId,
    curriculum_version: CurriculumVersionId,
    code: CourseCode,
    title: CourseTitle,
    credits: Credits,
    curriculum_category: CurriculumCategory,
    official_prerequisites: Vec<OfficialPrerequisite>,
    recommended_prerequisites: Vec<RecommendedPrerequisite>,
    designed_concept_coverage: Vec<EntityId>,
    designed_competency_coverage: Vec<EntityId>,
    valid_time: ValidInterval,
    source_snapshot: Option<ContentDigest>,
}

impl CourseRevision {
    /// This revision's identifier.
    #[must_use]
    pub const fn id(&self) -> CourseRevisionId {
        self.id
    }

    /// The durable course identity this revision defines.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// The curriculum version this revision was published under.
    #[must_use]
    pub const fn curriculum_version(&self) -> CurriculumVersionId {
        self.curriculum_version
    }

    /// The code as this revision prints it.
    #[must_use]
    pub const fn code(&self) -> &CourseCode {
        &self.code
    }

    /// Section 8.2's `titleKo`.
    #[must_use]
    pub const fn title(&self) -> &CourseTitle {
        &self.title
    }

    /// Section 8.2's `credits`.
    #[must_use]
    pub const fn credits(&self) -> Credits {
        self.credits
    }

    /// Section 8.2's `curriculumCategory`; `Unknown` while `GATE-38-013` is open.
    #[must_use]
    pub const fn curriculum_category(&self) -> CurriculumCategory {
        self.curriculum_category
    }

    /// Section 8.2's `officialPrerequisiteRules`.
    #[must_use]
    pub fn official_prerequisites(&self) -> &[OfficialPrerequisite] {
        &self.official_prerequisites
    }

    /// Section 8.2's `recommendedPrerequisiteClaims`.
    #[must_use]
    pub fn recommended_prerequisites(&self) -> &[RecommendedPrerequisite] {
        &self.recommended_prerequisites
    }

    /// Section 8.2's `designedConceptCoverage`.
    #[must_use]
    pub fn designed_concept_coverage(&self) -> &[EntityId] {
        &self.designed_concept_coverage
    }

    /// Section 8.2's `designedCompetencyCoverage`.
    #[must_use]
    pub fn designed_competency_coverage(&self) -> &[EntityId] {
        &self.designed_competency_coverage
    }

    /// The interval over which this catalogue definition is valid.
    #[must_use]
    pub const fn valid_time(&self) -> ValidInterval {
        self.valid_time
    }

    /// Section 8.2's `sourceSnapshot`.
    ///
    /// The same digest the event schema v3 registration frame carries as
    /// `source_digest`, which is why migration `0014` adds no column for it:
    /// migration `0004`'s `course_revision.source_digest` is where it is
    /// stored, and a second column would be a second source of truth.
    #[must_use]
    pub const fn source_snapshot(&self) -> Option<&ContentDigest> {
        self.source_snapshot.as_ref()
    }
}

/// The only route to a [`CourseRevision`].
#[derive(Debug, Clone)]
pub struct CourseRevisionDraft {
    id: CourseRevisionId,
    course: CourseId,
    curriculum_version: CurriculumVersionId,
    code: CourseCode,
    title: Option<CourseTitle>,
    credits: Option<Credits>,
    curriculum_category: CurriculumCategory,
    official_prerequisites: Vec<OfficialPrerequisite>,
    recommended_prerequisites: Vec<RecommendedPrerequisite>,
    designed_concept_coverage: Vec<EntityId>,
    designed_competency_coverage: Vec<EntityId>,
    valid_time: ValidInterval,
    source_snapshot: Option<ContentDigest>,
}

impl CourseRevisionDraft {
    /// Starts a draft bound to one course, one curriculum version, and one code.
    ///
    /// `curriculum_category` starts at [`CurriculumCategory::Unknown`], which is
    /// what an unconfirmed official classification reads as.
    #[must_use]
    pub const fn new(
        id: CourseRevisionId,
        course: CourseId,
        curriculum_version: CurriculumVersionId,
        code: CourseCode,
        valid_time: ValidInterval,
    ) -> Self {
        Self {
            id,
            course,
            curriculum_version,
            code,
            title: None,
            credits: None,
            curriculum_category: CurriculumCategory::Unknown,
            official_prerequisites: Vec::new(),
            recommended_prerequisites: Vec::new(),
            designed_concept_coverage: Vec::new(),
            designed_competency_coverage: Vec::new(),
            valid_time,
            source_snapshot: None,
        }
    }

    /// Records section 8.2's `titleKo`.
    #[must_use]
    pub fn title(mut self, title: CourseTitle) -> Self {
        self.title = Some(title);
        self
    }

    /// Records section 8.2's `credits`.
    #[must_use]
    pub const fn credits(mut self, credits: Credits) -> Self {
        self.credits = Some(credits);
        self
    }

    /// Records a confirmed official classification.
    #[must_use]
    pub const fn curriculum_category(mut self, category: CurriculumCategory) -> Self {
        self.curriculum_category = category;
        self
    }

    /// Appends one official prerequisite.
    #[must_use]
    pub fn official_prerequisite(mut self, entry: OfficialPrerequisite) -> Self {
        self.official_prerequisites.push(entry);
        self
    }

    /// Appends one recommended prerequisite.
    #[must_use]
    pub fn recommended_prerequisite(mut self, entry: RecommendedPrerequisite) -> Self {
        self.recommended_prerequisites.push(entry);
        self
    }

    /// Appends one designed concept coverage entry.
    #[must_use]
    pub fn designed_concept(mut self, concept: EntityId) -> Self {
        self.designed_concept_coverage.push(concept);
        self
    }

    /// Appends one designed competency coverage entry.
    #[must_use]
    pub fn designed_competency(mut self, competency: EntityId) -> Self {
        self.designed_competency_coverage.push(competency);
        self
    }

    /// Records section 8.2's `sourceSnapshot`.
    #[must_use]
    pub const fn source_snapshot(mut self, digest: ContentDigest) -> Self {
        self.source_snapshot = Some(digest);
        self
    }

    /// Builds the revision, naming the first unset attribute.
    pub fn build(self) -> Result<CourseRevision, CurriculumError> {
        let title = self.title.ok_or(CurriculumError::Missing {
            aggregate: "course revision",
            field: "title",
        })?;
        let credits = self.credits.ok_or(CurriculumError::Missing {
            aggregate: "course revision",
            field: "credits",
        })?;
        Ok(CourseRevision {
            id: self.id,
            course: self.course,
            curriculum_version: self.curriculum_version,
            code: self.code,
            title,
            credits,
            curriculum_category: self.curriculum_category,
            official_prerequisites: self.official_prerequisites,
            recommended_prerequisites: self.recommended_prerequisites,
            designed_concept_coverage: self.designed_concept_coverage,
            designed_competency_coverage: self.designed_competency_coverage,
            valid_time: self.valid_time,
            source_snapshot: self.source_snapshot,
        })
    }
}
