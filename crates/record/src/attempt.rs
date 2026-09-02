//! The section 10 attempt model: every attempt preserved, never overwritten.
//!
//! "`TakenCourse` 하나로 재수강과 취소를 덮어쓰지 않고 매 시도를 보존한다." A
//! repeat is not an edit to the earlier attempt and a cancellation is not a
//! deletion of one. Both are new [`CourseAttempt`] values, and the earlier
//! attempt stays readable forever.
//!
//! [`AttemptHistory`] is how that holds structurally rather than by convention.
//! It has one mutator, [`AttemptHistory::append`], and no way to remove or edit
//! an entry: no `&mut` borrow of a stored attempt escapes, no field is public,
//! and there is no `remove`, `retain`, `clear`, or index-assignment path. A
//! correction is a new entry carrying an ADR-003 `SUPERSEDES` relation to the
//! one it corrects — the same mechanism `CONTRIBUTING.md` rule 2 requires of
//! every canonical assertion, reused rather than reinvented.
//!
//! [`AttemptHistory::current`] is the resolver projection over that ledger and
//! is the only thing that shrinks; [`AttemptHistory::all`] only ever grows.

use std::collections::BTreeSet;

use academic_domain::{AttemptId, ClaimRelation, ClaimRelationKind, Decimal, EvidenceId, ScopeId};

use crate::{
    RecordError,
    grade::GradeSymbol,
    policy::{AttemptOrigin, RecognitionDecision},
    term::TermKey,
};

/// The section 10 status set, complete and closed.
///
/// The eight values are the specification's own, in its order.
/// `attempt_grade_repeat_contract` pins the set so a ninth cannot be added
/// without the contract moving with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttemptStatus {
    /// A candidate that is not an academic fact yet.
    ///
    /// Present because the specification's schema names it. **No constructor in
    /// this crate produces it** — see [`CourseAttempt`], whose two constructors
    /// take a confirmed registration or a confirmed transcript row. A plan lives
    /// in `crate::plan` as a `PlanScenarioChoice` and never becomes an attempt
    /// without one of those.
    Planned,
    /// Registration confirmed.
    Registered,
    /// Under way.
    InProgress,
    /// Finished with a grade.
    Completed,
    /// 철회 — withdrawn after the deadline, recorded as `W`.
    Withdrawn,
    /// Cancelled before it became an attempt of record.
    Cancelled,
    /// 편입 — carried in on transfer.
    Transferred,
    /// 인정학점 — recognized from elsewhere.
    Recognized,
}

impl AttemptStatus {
    /// Every status, in the specification's order.
    pub const ALL: [Self; 8] = [
        Self::Planned,
        Self::Registered,
        Self::InProgress,
        Self::Completed,
        Self::Withdrawn,
        Self::Cancelled,
        Self::Transferred,
        Self::Recognized,
    ];

    /// Returns the contract spelling, which is also the frozen-input token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "PLANNED",
            Self::Registered => "REGISTERED",
            Self::InProgress => "IN_PROGRESS",
            Self::Completed => "COMPLETED",
            Self::Withdrawn => "WITHDRAWN",
            Self::Cancelled => "CANCELLED",
            Self::Transferred => "TRANSFERRED",
            Self::Recognized => "RECOGNIZED",
        }
    }

    /// Resolves a status from its spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == text)
    }

    /// Whether the attempt has an outcome the engines may read.
    ///
    /// A `PLANNED`, `REGISTERED`, `IN_PROGRESS`, or `CANCELLED` attempt has no
    /// settled outcome. Section 10 says a plan must never raise actual
    /// progress, and this is where that holds for the numeric engines:
    /// an unsettled attempt contributes to neither the earned total nor the
    /// average.
    #[must_use]
    pub const fn is_settled(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Withdrawn | Self::Transferred | Self::Recognized
        )
    }
}

/// The section 10 repeat status set, complete and closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RepeatStatus {
    /// The first attempt of a course.
    Original,
    /// A later attempt of a course already attempted.
    Repeat,
    /// An attempt a later one displaced under the repeat policy.
    Replaced,
    /// The course is not subject to repeat handling.
    NotApplicable,
}

impl RepeatStatus {
    /// Every repeat status, in the specification's order.
    pub const ALL: [Self; 4] = [
        Self::Original,
        Self::Repeat,
        Self::Replaced,
        Self::NotApplicable,
    ];

    /// Returns the contract spelling, which is also the frozen-input token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Original => "ORIGINAL",
            Self::Repeat => "REPEAT",
            Self::Replaced => "REPLACED",
            Self::NotApplicable => "NOT_APPLICABLE",
        }
    }

    /// Resolves a repeat status from its spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == text)
    }
}

/// Proof that a registration was confirmed, and the values it confirmed.
///
/// This is the gate `registered_attempt_gate` names. A `CourseAttempt` with
/// status `REGISTERED` exists only where one of these does, and one of these
/// exists only where an official record or the user's own confirmation
/// supplied it — never where a plan scenario did. `crate::plan` holds no
/// method returning this type and none returning a `CourseAttempt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationConfirmation {
    course_code: String,
    term: TermKey,
    credits_attempted: Decimal,
    evidence_ids: Vec<EvidenceId>,
}

impl RegistrationConfirmation {
    /// Records a confirmed registration.
    ///
    /// At least one evidence identifier is required. A confirmation with
    /// nothing behind it is indistinguishable from an assumption, and an
    /// assumption is what section 10 forbids turning into academic history.
    pub fn new(
        course_code: impl Into<String>,
        term: TermKey,
        credits_attempted: Decimal,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, RecordError> {
        let course_code = course_code.into();
        if !crate::check_identifier(&course_code) {
            return Err(RecordError::MalformedCourseCode(course_code));
        }
        if evidence_ids.is_empty() {
            return Err(RecordError::RegistrationWithoutEvidence);
        }
        if credits_attempted.coefficient() < 0 {
            return Err(RecordError::NegativeCredits);
        }
        Ok(Self {
            course_code,
            term,
            credits_attempted,
            evidence_ids,
        })
    }

    /// Returns the confirmed course code.
    #[must_use]
    pub fn course_code(&self) -> &str {
        &self.course_code
    }

    /// Returns the confirmed term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// Returns the confirmed attempted credits.
    #[must_use]
    pub const fn credits_attempted(&self) -> Decimal {
        self.credits_attempted
    }

    /// Returns the evidence behind the confirmation.
    #[must_use]
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

/// One preserved attempt at one course.
///
/// Every field is private and there is no `&mut self` method. A value of this
/// type never changes after construction; a change is a new value plus a
/// `SUPERSEDES` relation in the [`AttemptHistory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseAttempt {
    id: AttemptId,
    course_code: String,
    term: TermKey,
    status: AttemptStatus,
    origin: AttemptOrigin,
    credits_attempted: Decimal,
    credits_earned: Decimal,
    grade: Option<GradeSymbol>,
    grading_scheme_id: String,
    repeat_of: Option<AttemptId>,
    repeat_status: RepeatStatus,
    recognition: RecognitionDecision,
    evidence_ids: Vec<EvidenceId>,
}

impl CourseAttempt {
    /// Builds a `REGISTERED` attempt from a confirmed registration.
    ///
    /// One of the two ways a `CourseAttempt` comes into existence. Note what it
    /// does *not* take: a status. A confirmed registration produces exactly
    /// `REGISTERED`, so no caller can name `PLANNED` here.
    pub fn from_confirmed_registration(
        id: AttemptId,
        confirmation: &RegistrationConfirmation,
        grading_scheme_id: impl Into<String>,
    ) -> Result<Self, RecordError> {
        Ok(Self {
            id,
            course_code: confirmation.course_code().to_owned(),
            term: confirmation.term(),
            status: AttemptStatus::Registered,
            origin: AttemptOrigin::Internal,
            credits_attempted: confirmation.credits_attempted(),
            credits_earned: crate::decimal::zero()?,
            grade: None,
            grading_scheme_id: grading_scheme_id.into(),
            repeat_of: None,
            repeat_status: RepeatStatus::NotApplicable,
            recognition: RecognitionDecision::Undecided,
            evidence_ids: confirmation.evidence_ids().to_vec(),
        })
    }

    /// Builds a settled attempt from a user-confirmed transcript row.
    ///
    /// The other of the two. `P2-U7` produces the confirmed row; this crate
    /// does not re-derive one, does not parse a transcript, and does not accept
    /// an unreconciled import row — `academic_transcript::confirm_reconciled_rows`
    /// is the only thing that mints a confirmed row, and it mints one only from
    /// a reconciliation that agreed on all four fields.
    #[allow(clippy::too_many_arguments)]
    pub fn from_confirmed_row(
        id: AttemptId,
        course_code: impl Into<String>,
        term: TermKey,
        status: SettledStatus,
        origin: AttemptOrigin,
        credits_attempted: Decimal,
        credits_earned: Decimal,
        grade: Option<GradeSymbol>,
        grading_scheme_id: impl Into<String>,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, RecordError> {
        let course_code = course_code.into();
        if !crate::check_identifier(&course_code) {
            return Err(RecordError::MalformedCourseCode(course_code));
        }
        if credits_attempted.coefficient() < 0 || credits_earned.coefficient() < 0 {
            return Err(RecordError::NegativeCredits);
        }
        if evidence_ids.is_empty() {
            return Err(RecordError::AttemptWithoutEvidence);
        }
        Ok(Self {
            id,
            course_code,
            term,
            status: status.into_status(),
            origin,
            credits_attempted,
            credits_earned,
            grade,
            grading_scheme_id: grading_scheme_id.into(),
            repeat_of: None,
            repeat_status: RepeatStatus::NotApplicable,
            recognition: RecognitionDecision::Undecided,
            evidence_ids,
        })
    }

    /// Returns a copy marked as a repeat of `earlier`.
    ///
    /// Consuming `self` and returning a new value, not mutating in place: an
    /// attempt is immutable, and the caller appends the result to the history
    /// rather than editing what is already there.
    #[must_use]
    pub fn as_repeat_of(mut self, earlier: AttemptId) -> Self {
        self.repeat_of = Some(earlier);
        self.repeat_status = RepeatStatus::Repeat;
        self
    }

    /// Returns a copy marked as the original of a repeat group.
    #[must_use]
    pub fn as_original(mut self) -> Self {
        self.repeat_status = RepeatStatus::Original;
        self
    }

    /// Returns a copy carrying a recognition decision for external credits.
    #[must_use]
    pub fn with_recognition(mut self, decision: RecognitionDecision) -> Self {
        self.recognition = decision;
        self
    }

    /// Returns the attempt's identity.
    #[must_use]
    pub const fn id(&self) -> AttemptId {
        self.id
    }

    /// Returns the official course code.
    #[must_use]
    pub fn course_code(&self) -> &str {
        &self.course_code
    }

    /// Returns the term the attempt was taken in.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// Returns the attempt status.
    #[must_use]
    pub const fn status(&self) -> AttemptStatus {
        self.status
    }

    /// Returns where the credits were earned.
    #[must_use]
    pub const fn origin(&self) -> AttemptOrigin {
        self.origin
    }

    /// Returns the credits the attempt was taken for.
    #[must_use]
    pub const fn credits_attempted(&self) -> Decimal {
        self.credits_attempted
    }

    /// Returns the credits the attempt earned.
    #[must_use]
    pub const fn credits_earned(&self) -> Decimal {
        self.credits_earned
    }

    /// Returns the recorded grade, if the attempt has one.
    #[must_use]
    pub const fn grade(&self) -> Option<GradeSymbol> {
        self.grade
    }

    /// Returns the scheme the grade is to be read under.
    #[must_use]
    pub fn grading_scheme_id(&self) -> &str {
        &self.grading_scheme_id
    }

    /// Returns the earlier attempt this one repeats.
    #[must_use]
    pub const fn repeat_of(&self) -> Option<AttemptId> {
        self.repeat_of
    }

    /// Returns the repeat status.
    #[must_use]
    pub const fn repeat_status(&self) -> RepeatStatus {
        self.repeat_status
    }

    /// Returns the recognition decision for external credits.
    #[must_use]
    pub const fn recognition(&self) -> RecognitionDecision {
        self.recognition
    }

    /// Returns the evidence the attempt rests on.
    #[must_use]
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

/// The four statuses a confirmed transcript row may produce.
///
/// A separate type from [`AttemptStatus`] so that
/// [`CourseAttempt::from_confirmed_row`] cannot be handed `PLANNED`,
/// `REGISTERED`, `IN_PROGRESS`, or `CANCELLED`. The gate is the argument type
/// rather than a run-time check, so a caller that tries does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettledStatus {
    /// Finished with a grade.
    Completed,
    /// Withdrawn.
    Withdrawn,
    /// Carried in on transfer.
    Transferred,
    /// Recognized from elsewhere.
    Recognized,
}

impl SettledStatus {
    /// Every settled status.
    pub const ALL: [Self; 4] = [
        Self::Completed,
        Self::Withdrawn,
        Self::Transferred,
        Self::Recognized,
    ];

    /// Widens to the full status set.
    #[must_use]
    pub const fn into_status(self) -> AttemptStatus {
        match self {
            Self::Completed => AttemptStatus::Completed,
            Self::Withdrawn => AttemptStatus::Withdrawn,
            Self::Transferred => AttemptStatus::Transferred,
            Self::Recognized => AttemptStatus::Recognized,
        }
    }
}

/// One appended entry: an attempt, and the correction relation if it is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptEntry {
    attempt: CourseAttempt,
    supersedes: Option<AttemptId>,
    relation: Option<ClaimRelation>,
}

impl AttemptEntry {
    /// Returns the attempt.
    #[must_use]
    pub const fn attempt(&self) -> &CourseAttempt {
        &self.attempt
    }

    /// Returns the attempt this entry corrects, if any.
    #[must_use]
    pub const fn supersedes(&self) -> Option<AttemptId> {
        self.supersedes
    }

    /// Returns the ADR-003 relation carrying the correction.
    #[must_use]
    pub const fn relation(&self) -> Option<&ClaimRelation> {
        self.relation.as_ref()
    }
}

/// The append-only attempt ledger.
///
/// One mutator. No removal, no in-place edit, no `&mut` borrow of a stored
/// attempt. `attempt_history_append_only` executes that: it appends a
/// correction, observes `current` change and `all` not shrink, and observes the
/// superseded attempt still readable by identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttemptHistory {
    entries: Vec<AttemptEntry>,
}

impl AttemptHistory {
    /// Builds an empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Appends an attempt. The only mutator this type has.
    pub fn append(&mut self, attempt: CourseAttempt) -> Result<(), RecordError> {
        self.push(attempt, None, None)
    }

    /// Appends a correction of an attempt already in the ledger.
    ///
    /// The relation is ADR-003's `SUPERSEDES`, built here rather than accepted
    /// from the caller so its kind cannot be something weaker. Both claims are
    /// user-confirmed attempt assertions, which is the nonterminal
    /// authority/status pair ADR-003's fail-closed matrix admits for a
    /// state-removing relation authored by the user.
    pub fn append_correction(
        &mut self,
        attempt: CourseAttempt,
        supersedes: AttemptId,
        scope_id: ScopeId,
        source_claim_id: academic_domain::ClaimId,
        target_claim_id: academic_domain::ClaimId,
    ) -> Result<(), RecordError> {
        if !self.contains(supersedes) {
            return Err(RecordError::SupersedesUnknownAttempt(supersedes));
        }
        if attempt.id() == supersedes {
            return Err(RecordError::AttemptSupersedesItself(supersedes));
        }
        let relation = ClaimRelation {
            source_claim_id,
            target_claim_id,
            kind: ClaimRelationKind::Supersedes,
            scope_id,
        };
        self.push(attempt, Some(supersedes), Some(relation))
    }

    fn push(
        &mut self,
        attempt: CourseAttempt,
        supersedes: Option<AttemptId>,
        relation: Option<ClaimRelation>,
    ) -> Result<(), RecordError> {
        if self.contains(attempt.id()) {
            return Err(RecordError::DuplicateAttemptId(attempt.id()));
        }
        self.entries.push(AttemptEntry {
            attempt,
            supersedes,
            relation,
        });
        Ok(())
    }

    /// Whether an attempt identity is anywhere in the ledger.
    #[must_use]
    pub fn contains(&self, id: AttemptId) -> bool {
        self.entries.iter().any(|entry| entry.attempt.id() == id)
    }

    /// Returns every entry ever appended, in append order.
    ///
    /// This is the half that never shrinks.
    #[must_use]
    pub fn all(&self) -> &[AttemptEntry] {
        &self.entries
    }

    /// Returns one attempt by identity, superseded or not.
    #[must_use]
    pub fn get(&self, id: AttemptId) -> Option<&CourseAttempt> {
        self.entries
            .iter()
            .map(AttemptEntry::attempt)
            .find(|attempt| attempt.id() == id)
    }

    /// Returns the attempts no later entry superseded, in append order.
    ///
    /// This is the resolver projection ADR-003 calls current state. It is
    /// derived on every call rather than stored, so there is no second copy to
    /// fall out of step with the ledger.
    #[must_use]
    pub fn current(&self) -> Vec<&CourseAttempt> {
        let superseded: BTreeSet<AttemptId> = self
            .entries
            .iter()
            .filter_map(AttemptEntry::supersedes)
            .collect();
        self.entries
            .iter()
            .map(AttemptEntry::attempt)
            .filter(|attempt| !superseded.contains(&attempt.id()))
            .collect()
    }
}
