//! The typed facts a published rule reads, and nothing else.
//!
//! This is the frozen input half of section 28's engine signature, at the
//! granularity one rule needs. It carries identifiers, integers, exact
//! decimals and enumerations. It carries no sentence, so there is nothing here
//! for a model to interpret and nothing a rule could interpret if it wanted to.
//!
//! # Absence is a value, and it is not zero
//!
//! [`AcademicFacts::admission_year`] is an `Option`. `None` is not "assume the
//! current cohort": section 8.1 leaves the user's admission year unrecorded and
//! section 11.4 closes with *이 문서는 ... 개인의 "남은 학점"을 산출하지
//! 않는다*. Every rule scoped by admission year returns
//! `ProofStatus::Unknown` against a `None`, which is `GATE-38-011`.
//!
//! A withdrawn attempt is likewise not an absent one. It is present with
//! [`AttemptStatus::Withdrawn`], so a co-requisite can distinguish "never
//! taken" from "taken and dropped" -- `REQ-11-010`'s four cases.

use std::collections::BTreeMap;

use academic_domain::{CourseId, Decimal, EntityId, TimestampMillis};

use crate::dsl::{
    AdmissionYear, ApprovalAuthority, AreaId, CreditAmount, CreditCategory, InstructionLanguage,
    ProgramId, RuleId,
};

/// One academic term, ordered so a co-requisite can compare two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TermOrdinal(u32);

impl TermOrdinal {
    /// Constructs a term ordinal. Larger is later.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the ordinal.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// What became of one attempt.
///
/// Section 10's attempt model is `P2-U4`'s and lives in `academic-record`. What
/// a rule needs is narrower: whether the attempt produced a recognized result,
/// whether it is only planned, and whether it was withdrawn. Planned work never
/// satisfies anything, which is section 11.3's *planned only ... NOT_SATISFIED*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttemptStatus {
    /// Completed with a recognized result.
    Completed,
    /// Registered and not yet resolved.
    InProgress,
    /// Withdrawn. It happened and it recognized nothing.
    Withdrawn,
    /// Planned only. Section 11.3: planned work is never satisfied work.
    Planned,
}

impl AttemptStatus {
    /// Whether this status recognizes credit toward a requirement.
    #[must_use]
    pub const fn is_recognized(self) -> bool {
        matches!(self, Self::Completed)
    }
}

/// Whether a course's language of instruction was verified.
///
/// Section 8.1 records the foreign-language lecture requirement as an official
/// fact about courses; whether a particular offering was actually taught in
/// that language is evidence, and an unverified reading is not a negative one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageEvidence {
    /// A confirmed source states the language.
    Verified(InstructionLanguage),
    /// No confirmed source states it.
    Unverified,
}

/// One attempt, as a rule reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptFact {
    /// The attempt's own identity, so a proof leaf can name which one it used.
    pub attempt: EntityId,
    /// The course attempted.
    pub course: CourseId,
    /// Its credits.
    pub credits: CreditAmount,
    /// Which credit categories it counts under. A course can sit in more than
    /// one; whether it may count twice is `MUTUALLY_EXCLUSIVE`'s question and
    /// `GATE-38-015`'s.
    pub categories: Vec<CreditCategory>,
    /// The general-education area it is recognized in, when it has one.
    pub area: Option<AreaId>,
    /// Whether it is a major course, for `atLeastMajorCourses`.
    pub is_major: bool,
    /// The term it was attempted in.
    pub term: TermOrdinal,
    /// What became of it.
    pub status: AttemptStatus,
    /// What is known about its language of instruction.
    pub language: LanguageEvidence,
}

/// One non-credit training completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingFact {
    /// The programme completed.
    pub program: ProgramId,
    /// When it was completed.
    pub completed_at: TimestampMillis,
}

/// One recorded exception approval.
///
/// It names the rule it alters. An approval that names no rule alters nothing,
/// which is why `rule` is not an `Option`: there is no shape here for a blanket
/// approval, and `REQ-11-017`'s *alters only target leaf* is that absence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalFact {
    /// The exact rule this approval alters.
    pub rule: RuleId,
    /// The office that issued it.
    pub authority: ApprovalAuthority,
    /// When it was issued.
    pub issued_at: TimestampMillis,
    /// When it stops applying, when it does.
    pub expires_at: Option<TimestampMillis>,
}

/// The grade-point reading one `GPA_MINIMUM` scope resolves to.
///
/// Both halves are exact integers and the comparison below is done by
/// cross-multiplication, so no division and no float enters the graduation
/// path. `academic-record` owns the grade-point engine; this is the reading a
/// rule is handed, not a second computation of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpaReading {
    /// The sum of grade points times credits, exact.
    pub weighted_points: Decimal,
    /// The credits in the denominator.
    pub denominator_credits: u32,
}

/// The frozen facts one evaluation reads.
///
/// Built once and read immutably. There is no method that adds a fact after
/// construction, so two evaluations over the same value cannot diverge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcademicFacts {
    admission_year: Option<AdmissionYear>,
    as_of: TimestampMillis,
    attempts: Vec<AttemptFact>,
    trainings: Vec<TrainingFact>,
    approvals: Vec<ApprovalFact>,
    gpa: BTreeMap<String, GpaReading>,
}

impl AcademicFacts {
    /// Builds a frozen fact set.
    #[must_use]
    pub fn new(as_of: TimestampMillis) -> Self {
        Self {
            admission_year: None,
            as_of,
            attempts: Vec::new(),
            trainings: Vec::new(),
            approvals: Vec::new(),
            gpa: BTreeMap::new(),
        }
    }

    /// Records the admission year. Not calling this leaves it unknown, which is
    /// `GATE-38-011` and is what section 8.1 currently holds.
    #[must_use]
    pub fn with_admission_year(mut self, year: AdmissionYear) -> Self {
        self.admission_year = Some(year);
        self
    }

    /// Adds one attempt.
    #[must_use]
    pub fn with_attempt(mut self, attempt: AttemptFact) -> Self {
        self.attempts.push(attempt);
        self
    }

    /// Adds one non-credit training completion.
    #[must_use]
    pub fn with_training(mut self, training: TrainingFact) -> Self {
        self.trainings.push(training);
        self
    }

    /// Adds one exception approval.
    #[must_use]
    pub fn with_approval(mut self, approval: ApprovalFact) -> Self {
        self.approvals.push(approval);
        self
    }

    /// Records the grade-point reading for one scope.
    #[must_use]
    pub fn with_gpa(mut self, scope: &crate::dsl::GpaScope, reading: GpaReading) -> Self {
        self.gpa.insert(scope.as_str().to_owned(), reading);
        self
    }

    /// The admission year, or `None` when no official record supplies one.
    #[must_use]
    pub const fn admission_year(&self) -> Option<AdmissionYear> {
        self.admission_year
    }

    /// The instant the evaluation is anchored to. It is an argument, never a
    /// clock read: section 28's engines have no clock.
    #[must_use]
    pub const fn as_of(&self) -> TimestampMillis {
        self.as_of
    }

    /// Every attempt.
    #[must_use]
    pub fn attempts(&self) -> &[AttemptFact] {
        &self.attempts
    }

    /// Every non-credit training completion.
    #[must_use]
    pub fn trainings(&self) -> &[TrainingFact] {
        &self.trainings
    }

    /// Every recorded exception approval.
    #[must_use]
    pub fn approvals(&self) -> &[ApprovalFact] {
        &self.approvals
    }

    /// The grade-point reading for one scope, when one was supplied.
    #[must_use]
    pub fn gpa(&self, scope: &crate::dsl::GpaScope) -> Option<GpaReading> {
        self.gpa.get(scope.as_str()).copied()
    }
}
