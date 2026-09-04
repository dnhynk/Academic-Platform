//! The transcript snapshot an audit is bound to, built out of `P2-U4`'s ledger.
//!
//! # There is no second attempt model here
//!
//! An attempt reaches a rule through `academic_record::CourseAttempt` and
//! through nothing else. That crate's two constructors take a confirmed
//! registration or a confirmed transcript row, neither accepts a
//! `PlanScenarioChoice`, and `AttemptStatus::Planned` has no producer. This
//! module declares no constructor of its own and takes the ledger by reference,
//! so the audit inherits those gates rather than restating them.
//!
//! # Whether a credit counts is `P2-U4`'s decision, not this crate's
//!
//! `RecordViews` publishes one `AttemptDisposition` per attempt, carrying a
//! `CreditContribution` of `Earned`, `NotEarned` or `Unknown` and one of
//! thirteen `DispositionReason`s. [`EntryAdmission`] is a total function of
//! that three-arm enum:
//!
//! | `CreditContribution` | admission |
//! |---|---|
//! | `Earned(credits)` | [`EntryAdmission::Counted`] with those credits |
//! | `NotEarned` | [`EntryAdmission::Excluded`], carrying the record's reason |
//! | `Unknown` | [`EntryAdmission::Pending`], carrying the record's reason |
//!
//! Totality is by arithmetic over that enum rather than by a list of cases
//! somebody thought of, and the reason a reader sees is the record engine's own
//! word rather than a second vocabulary that could disagree with it. A planned,
//! registered, in-progress or cancelled attempt is `NotSettled` there, so it
//! reaches `NotEarned` and is excluded -- `plan_excluded_from_actual_audit`'s
//! deepest layer is a decision `P2-U4` already made.
//!
//! An attempt whose contribution is `Unknown` -- an undecided external
//! recognition (`GATE-38-006`), a repeat group whose recognition rule no
//! confirmed source states, or an external term no dated policy row reaches --
//! is **known to be undecided**. It counts toward nothing, it is not silently
//! dropped, and it is reported as an exact missing check that blocks
//! `DETERMINATE`.
//!
//! # The audit computes no average
//!
//! [`TranscriptSnapshot::reading`] hands a `GPA_MINIMUM` rule the reading
//! `P2-U4` published, summed from the exact contributions its dispositions
//! carry. **Nothing here rounds.** The rule compares by cross-multiplication,
//! so no ratio is ever formed; the only divisions in this crate reduce an exact
//! decimal to whole units and each checks the remainder is zero first, so a
//! fractional credit is a typed refusal rather than a rounded one. The one
//! *rounding* site in the workspace stays
//! `academic_record::decimal::div_round_half_up`.

use std::collections::BTreeMap;

use academic_domain::{AttemptId, ContentDigest, CourseId, Decimal, EntityId};
use academic_record::{
    attempt::{AttemptHistory, AttemptStatus as RecordAttemptStatus},
    classify::{ClassificationRuleSet, ProgramId as RecordProgramId},
    decimal,
    facts::AttemptFacts,
    policy::RuleBook,
    term::{Semester, TermKey},
    views::{
        AttemptDisposition, AverageContribution, CreditContribution, DispositionReason, RecordViews,
    },
};
use academic_requirement::{
    AreaId, AttemptFact, AttemptStatus as RuleAttemptStatus, CreditAmount, CreditCategory,
    GpaReading, GpaScope, LanguageEvidence, TermOrdinal,
};

use crate::error::AuditError;

/// Section 11.2's own `scope:` identifier for the cumulative average.
///
/// The yaml writes `scope: ALL_GPA_ELIGIBLE`, so this is the specification's
/// spelling rather than this crate's.
pub const ALL_GPA_ELIGIBLE: &str = "ALL_GPA_ELIGIBLE";

/// Reads the same identity as an `EntityId`.
///
/// `academic-record` types an attempt's identity as `AttemptId` and
/// `academic-requirement` types the same value as `EntityId`. Both are the
/// UUIDv7 the ledger assigned; the two crates differ only in which newtype they
/// wrap it in, and neither declares a conversion because neither depends on the
/// other. This is that conversion, and it is the only one in this crate: it
/// preserves the bytes exactly, so a proof leaf names the attempt a reader can
/// look up.
pub(crate) fn as_entity(attempt: AttemptId) -> Result<EntityId, AuditError> {
    EntityId::try_from_uuid(attempt.as_uuid())
        .map_err(|_| AuditError::CourseFactsAbsent { attempt })
}

/// Reads an `EntityId` back as the attempt identity it came from.
pub(crate) fn as_attempt(entity: EntityId) -> Option<AttemptId> {
    AttemptId::try_from_uuid(entity.as_uuid()).ok()
}

/// The curriculum facts one course carries into a requirement rule.
///
/// Supplied by the caller because none of them is the record's: which durable
/// `Course` a transcript code names is `P2-U1`'s identity decision, which
/// credit categories and general-education area a revision places it in is
/// `P2-U1`'s catalogue, and what language an offering was actually taught in is
/// the offering's evidence. This crate infers none of them.
///
/// `GATE-38-013` -- the engineering-common recognition list and its
/// required/elective distribution -- is `P2-U1`'s and stays open there. A
/// revision whose category is unconfirmed holds
/// `academic_curriculum::CurriculumCategory::Unknown`, and there is no
/// conversion from that value to a [`CreditCategory`] anywhere, so an
/// unconfirmed category cannot arrive here as a confirmed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseRequirementFacts {
    /// The durable course identity the transcript code names.
    pub course: CourseId,
    /// Every credit category the course counts under.
    pub categories: Vec<CreditCategory>,
    /// The general-education area it is recognized in, when it has one.
    pub area: Option<AreaId>,
    /// What is known about the language it was taught in.
    pub language: LanguageEvidence,
}

/// Which curriculum facts each transcript course code carries.
///
/// A map with no fallback. A code with no entry makes
/// [`TranscriptSnapshot::from_record`] refuse: an attempt the curriculum has
/// not placed cannot be counted under any category, and counting it under none
/// would be an audit over a transcript the engine only partly read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CourseFactsIndex {
    entries: BTreeMap<String, CourseRequirementFacts>,
}

impl CourseFactsIndex {
    /// An index that places no course.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Records the facts for one course code.
    #[must_use]
    pub fn with(mut self, course_code: impl Into<String>, facts: CourseRequirementFacts) -> Self {
        self.entries.insert(course_code.into(), facts);
        self
    }

    /// The facts for one course code, when they were recorded.
    #[must_use]
    pub fn facts(&self, course_code: &str) -> Option<&CourseRequirementFacts> {
        self.entries.get(course_code)
    }
}

/// Why an entry contributes what it does, in the record engine's own word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAdmission {
    /// The record engine earned these credits on this attempt.
    Counted {
        /// The credits earned.
        credits: CreditAmount,
        /// The record engine's reason.
        reason: DispositionReason,
    },
    /// The record engine earned no credits on this attempt, for this reason.
    Excluded {
        /// The record engine's reason.
        reason: DispositionReason,
    },
    /// Whether this attempt earns credit is not known, for this reason.
    ///
    /// Known to be unknown. It counts toward nothing and it is reported.
    Pending {
        /// The record engine's reason.
        reason: DispositionReason,
    },
}

impl EntryAdmission {
    /// The record engine's reason, whichever arm this is.
    #[must_use]
    pub const fn reason(self) -> DispositionReason {
        match self {
            Self::Counted { reason, .. } | Self::Excluded { reason } | Self::Pending { reason } => {
                reason
            }
        }
    }

    /// The stable token the drilldown and the frozen inputs spell.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Counted { .. } => "COUNTED",
            Self::Excluded { .. } => "EXCLUDED",
            Self::Pending { .. } => "PENDING",
        }
    }
}

/// One attempt as the audit reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptEntry {
    attempt: AttemptId,
    course_code: String,
    course: CourseId,
    term: TermKey,
    record_status: RecordAttemptStatus,
    admission: EntryAdmission,
    categories: Vec<CreditCategory>,
    area: Option<AreaId>,
    is_major: bool,
    language: LanguageEvidence,
}

impl TranscriptEntry {
    /// Rebuilds one entry from frozen inputs.
    ///
    /// Crate-private, and the only other producer besides
    /// [`TranscriptSnapshot::from_record`]. It exists because the engine
    /// signature is a function of frozen inputs: a golden fixture and a product
    /// call have to reach the same values, and the only way for that to be true
    /// rather than asserted is for both to go through this decoding.
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn decoded(
        attempt: AttemptId,
        course_code: String,
        course: CourseId,
        term: TermKey,
        record_status: RecordAttemptStatus,
        admission: EntryAdmission,
        categories: Vec<CreditCategory>,
        area: Option<AreaId>,
        is_major: bool,
        language: LanguageEvidence,
    ) -> Self {
        Self {
            attempt,
            course_code,
            course,
            term,
            record_status,
            admission,
            categories,
            area,
            is_major,
            language,
        }
    }

    /// The attempt's identity.
    #[must_use]
    pub const fn attempt(&self) -> AttemptId {
        self.attempt
    }

    /// The course code the transcript printed.
    #[must_use]
    pub fn course_code(&self) -> &str {
        &self.course_code
    }

    /// The durable course identity.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// The term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// The record's own status for the attempt.
    #[must_use]
    pub const fn record_status(&self) -> RecordAttemptStatus {
        self.record_status
    }

    /// What the record engine decided about its credits.
    #[must_use]
    pub const fn admission(&self) -> EntryAdmission {
        self.admission
    }

    /// Whether the classification engine calls it a major course.
    #[must_use]
    pub const fn is_major(&self) -> bool {
        self.is_major
    }

    /// The credit categories the curriculum placed it in.
    #[must_use]
    pub fn categories(&self) -> &[CreditCategory] {
        &self.categories
    }

    /// The general-education area, when it has one.
    #[must_use]
    pub const fn area(&self) -> Option<&AreaId> {
        self.area.as_ref()
    }

    /// The language evidence.
    #[must_use]
    pub const fn language(&self) -> LanguageEvidence {
        self.language
    }

    /// The status a published rule reads.
    ///
    /// A rule reads exactly two bits off an attempt: whether it is recognized,
    /// which is `AttemptStatus::is_recognized`, and whether it is still open,
    /// which is what separates `InProgress` from `Withdrawn`. This is those two
    /// bits and nothing else. `Withdrawn` here means *settled and recognized
    /// nothing*, which is the only reading any published rule takes of it.
    #[must_use]
    pub const fn rule_status(&self) -> RuleAttemptStatus {
        match (self.admission, self.record_status) {
            (EntryAdmission::Counted { .. }, _) => RuleAttemptStatus::Completed,
            (_, RecordAttemptStatus::Planned) => RuleAttemptStatus::Planned,
            (_, RecordAttemptStatus::Registered | RecordAttemptStatus::InProgress) => {
                RuleAttemptStatus::InProgress
            }
            (
                _,
                RecordAttemptStatus::Completed
                | RecordAttemptStatus::Withdrawn
                | RecordAttemptStatus::Cancelled
                | RecordAttemptStatus::Transferred
                | RecordAttemptStatus::Recognized,
            ) => RuleAttemptStatus::Withdrawn,
        }
    }

    /// The fact a published rule evaluates against.
    pub fn as_rule_fact(&self) -> Result<AttemptFact, AuditError> {
        let credits = match self.admission {
            EntryAdmission::Counted { credits, .. } => credits,
            EntryAdmission::Excluded { .. } | EntryAdmission::Pending { .. } => {
                CreditAmount::new(0)?
            }
        };
        Ok(AttemptFact {
            attempt: as_entity(self.attempt)?,
            course: self.course,
            credits,
            categories: self.categories.clone(),
            area: self.area.clone(),
            is_major: self.is_major,
            term: term_ordinal(self.term),
            status: self.rule_status(),
            language: self.language,
        })
    }

    /// The categories rendered in a fixed order.
    fn category_text(&self) -> String {
        let mut names: Vec<&str> = self.categories.iter().map(CreditCategory::as_str).collect();
        names.sort_unstable();
        if names.is_empty() {
            "none".to_owned()
        } else {
            names.join(",")
        }
    }

    fn canonical_text(&self) -> String {
        format!(
            "{} {} {} {} {} {} major={} area={} language={} categories={}",
            self.attempt,
            self.course_code,
            self.course,
            self.term.canonical_text(),
            self.record_status.as_str(),
            self.admission_text(),
            u8::from(self.is_major),
            self.area
                .as_ref()
                .map_or_else(|| "none".to_owned(), |area| area.as_str().to_owned()),
            language_token(self.language),
            self.category_text()
        )
    }

    fn admission_text(&self) -> String {
        match self.admission {
            EntryAdmission::Counted { credits, reason } => {
                format!("COUNTED/{}/{}", credits.get(), reason_token(reason))
            }
            EntryAdmission::Excluded { reason } => format!("EXCLUDED/{}", reason_token(reason)),
            EntryAdmission::Pending { reason } => format!("PENDING/{}", reason_token(reason)),
        }
    }
}

/// The frozen transcript one audit is bound to.
///
/// Built once from the ledger and read immutably. There is no method that adds
/// an entry after construction, so two audits over the same value cannot see
/// different transcripts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptSnapshot {
    entries: Vec<TranscriptEntry>,
    readings: BTreeMap<String, GpaReading>,
}

impl TranscriptSnapshot {
    /// Reduces `P2-U4`'s ledger to what a rule reads.
    ///
    /// The views are built here, from this ledger and this rule book, rather
    /// than accepted as an argument: views of a different attempt set would be
    /// a transcript whose entries and whose credits came from two places.
    pub fn from_record(
        history: &AttemptHistory,
        classification: &ClassificationRuleSet,
        rules: &RuleBook,
        primary_program: &RecordProgramId,
        courses: &CourseFactsIndex,
    ) -> Result<Self, AuditError> {
        let attempt_facts: Vec<AttemptFacts> = history
            .current()
            .into_iter()
            .map(|attempt| AttemptFacts::from_attempt(attempt, classification))
            .collect();
        let views = RecordViews::from_facts(&attempt_facts, rules)?;

        let mut entries: Vec<TranscriptEntry> = Vec::new();
        let mut seen: BTreeMap<AttemptId, ()> = BTreeMap::new();
        for facts in &attempt_facts {
            let id = facts.id;
            if seen.insert(id, ()).is_some() {
                return Err(AuditError::DuplicateAttempt { attempt: id });
            }
            let disposition = views
                .dispositions()
                .iter()
                .find(|disposition| disposition.attempt_id() == id)
                .ok_or(AuditError::CourseFactsAbsent { attempt: id })?;
            let course = courses
                .facts(&facts.course_code)
                .ok_or(AuditError::CourseFactsAbsent { attempt: id })?;
            let reason = disposition.reason();
            let admission = match disposition.credit() {
                CreditContribution::Earned(credits) => EntryAdmission::Counted {
                    credits: whole_credits(credits, id)?,
                    reason,
                },
                CreditContribution::NotEarned => EntryAdmission::Excluded { reason },
                CreditContribution::Unknown => EntryAdmission::Pending { reason },
            };
            entries.push(TranscriptEntry {
                attempt: id,
                course_code: facts.course_code.clone(),
                course: course.course,
                term: facts.term,
                record_status: facts.status,
                admission,
                categories: course.categories.clone(),
                area: course.area.clone(),
                is_major: facts.is_major_for(primary_program),
                language: course.language,
            });
        }
        entries.sort_by(|left, right| {
            left.term
                .cmp(&right.term)
                .then_with(|| left.course_code.cmp(&right.course_code))
                .then_with(|| left.attempt.as_bytes().cmp(right.attempt.as_bytes()))
        });

        let mut readings: BTreeMap<String, GpaReading> = BTreeMap::new();
        if let Some(reading) = reading_over(views.dispositions().iter())? {
            readings.insert(ALL_GPA_ELIGIBLE.to_owned(), reading);
        }
        for program in views.programs() {
            let major: Vec<AttemptId> = attempt_facts
                .iter()
                .filter(|facts| facts.is_major_for(&program))
                .map(|facts| facts.id)
                .collect();
            let selected = views
                .dispositions()
                .iter()
                .filter(|disposition| major.contains(&disposition.attempt_id()));
            if let Some(reading) = reading_over(selected)? {
                readings.insert(format!("MAJOR.{}", program.as_str()), reading);
            }
        }

        Ok(Self { entries, readings })
    }

    /// Rebuilds a snapshot from frozen inputs.
    ///
    /// Crate-private for the reason [`TranscriptEntry::decoded`] gives. The
    /// entries arrive in the order [`encode`](crate::facts::encode) wrote them,
    /// which is the order `from_record` sorted them into.
    pub(crate) const fn decoded(
        entries: Vec<TranscriptEntry>,
        readings: BTreeMap<String, GpaReading>,
    ) -> Self {
        Self { entries, readings }
    }

    /// Every entry, in term order.
    #[must_use]
    pub fn entries(&self) -> &[TranscriptEntry] {
        &self.entries
    }

    /// Every entry whose credit contribution is not known.
    ///
    /// Each is an exact missing check. An empty list is what `DETERMINATE`
    /// needs from this input.
    #[must_use]
    pub fn pending(&self) -> Vec<&TranscriptEntry> {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.admission, EntryAdmission::Pending { .. }))
            .collect()
    }

    /// The grade-point reading for one scope, when `P2-U4` published one.
    ///
    /// The audit computes no average. Section 11.2's own `scope:` identifier
    /// `ALL_GPA_ELIGIBLE` carries the cumulative reading and
    /// `MAJOR.<programme>` carries one programme's; a rule naming a scope no
    /// reading covers evaluates to `UNKNOWN`, which is what an absent reading
    /// means and is never a pass.
    #[must_use]
    pub fn reading(&self, scope: &GpaScope) -> Option<GpaReading> {
        self.readings.get(scope.as_str()).copied()
    }

    /// Every published reading, by scope identifier.
    pub fn readings(&self) -> impl Iterator<Item = (&String, &GpaReading)> {
        self.readings.iter()
    }

    /// The canonical text this snapshot's digest is taken over.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let mut rendered = String::new();
        for entry in &self.entries {
            rendered.push_str(&entry.canonical_text());
            rendered.push('\n');
        }
        for (scope, reading) in &self.readings {
            rendered.push_str(&format!(
                "gpa {scope} {}/{} over {}\n",
                reading.weighted_points.coefficient(),
                reading.weighted_points.scale(),
                reading.denominator_credits
            ));
        }
        rendered
    }

    /// The digest section 6's `DegreeAuditAggregate` binds the transcript by.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(self.canonical_text().as_bytes())
    }
}

/// Sums one selection of dispositions into the reading a rule is handed.
///
/// `None` when any selected attempt's average contribution is unknown, or when
/// none reaches the denominator: publishing a reading over a set the record
/// engine could not resolve would hand a rule a number the record itself
/// withheld.
fn reading_over<'a>(
    dispositions: impl Iterator<Item = &'a AttemptDisposition>,
) -> Result<Option<GpaReading>, AuditError> {
    let mut weighted = decimal::zero()?;
    let mut denominator = decimal::zero()?;
    let mut graded = false;
    for disposition in dispositions {
        match disposition.average() {
            AverageContribution::Included {
                quality_points,
                denominator_credits,
                ..
            } => {
                graded = true;
                weighted = decimal::add(weighted, quality_points)?;
                denominator = decimal::add(denominator, denominator_credits)?;
            }
            AverageContribution::Excluded => {}
            AverageContribution::Unknown => return Ok(None),
        }
    }
    if !graded {
        return Ok(None);
    }
    Ok(Some(GpaReading {
        weighted_points: weighted,
        denominator_credits: whole_denominator(denominator)?,
    }))
}

/// Turns a term into the ordinal a co-requisite compares.
///
/// `TermKey`'s own ordering is year then session, and this is that ordering as
/// an integer: four sessions to a year, in the order the academic year runs.
fn term_ordinal(term: TermKey) -> TermOrdinal {
    let session = match term.semester() {
        Semester::Spring => 0,
        Semester::Summer => 1,
        Semester::Fall => 2,
        Semester::Winter => 3,
    };
    TermOrdinal::new(u32::from(term.year()) * 4 + session)
}

/// Reads an exact decimal credit total as a whole number of credits.
///
/// A requirement threshold is a whole number, so a fractional contribution is a
/// refusal rather than a rounded one.
fn whole_credits(credits: Decimal, attempt: AttemptId) -> Result<CreditAmount, AuditError> {
    let whole = whole_units(credits).ok_or_else(|| AuditError::FractionalDenominator {
        value: rendered(credits),
    })?;
    let _ = attempt;
    let whole = u16::try_from(whole).map_err(|_| AuditError::FractionalDenominator {
        value: rendered(credits),
    })?;
    Ok(CreditAmount::new(whole)?)
}

fn whole_denominator(credits: Decimal) -> Result<u32, AuditError> {
    let whole = whole_units(credits).ok_or_else(|| AuditError::FractionalDenominator {
        value: rendered(credits),
    })?;
    u32::try_from(whole).map_err(|_| AuditError::FractionalDenominator {
        value: rendered(credits),
    })
}

/// The exact whole number a decimal holds, or `None` when it holds a fraction.
fn whole_units(value: Decimal) -> Option<i128> {
    let divisor = 10_i128.checked_pow(u32::from(value.scale()))?;
    if value.coefficient() % divisor == 0 {
        Some(value.coefficient() / divisor)
    } else {
        None
    }
}

fn rendered(value: Decimal) -> String {
    format!("{}/{}", value.coefficient(), value.scale())
}

/// The stable token for one of the record engine's thirteen reasons.
pub(crate) const fn reason_token(reason: DispositionReason) -> &'static str {
    match reason {
        DispositionReason::Graded => "GRADED",
        DispositionReason::FailedInDenominator => "FAILED_IN_DENOMINATOR",
        DispositionReason::RepeatCeilingApplied => "REPEAT_CEILING_APPLIED",
        DispositionReason::ReplacedByRepeat => "REPLACED_BY_REPEAT",
        DispositionReason::SatisfactoryNotGraded => "SATISFACTORY_NOT_GRADED",
        DispositionReason::UnsatisfactoryNotGraded => "UNSATISFACTORY_NOT_GRADED",
        DispositionReason::Withdrawn => "WITHDRAWN",
        DispositionReason::IncompleteUnresolved => "INCOMPLETE_UNRESOLVED",
        DispositionReason::ExternalExcludedFromAverage => "EXTERNAL_EXCLUDED_FROM_AVERAGE",
        DispositionReason::ExternalPolicyUnknown => "EXTERNAL_POLICY_UNKNOWN",
        DispositionReason::RecognitionUndecided => "RECOGNITION_UNDECIDED",
        DispositionReason::RepeatRecognitionUnknown => "REPEAT_RECOGNITION_UNKNOWN",
        DispositionReason::NotSettled => "NOT_SETTLED",
    }
}

/// The stable token for one attempt's language evidence.
pub(crate) const fn language_token(language: LanguageEvidence) -> &'static str {
    match language {
        LanguageEvidence::Verified(value) => value.as_str(),
        LanguageEvidence::Unverified => "UNVERIFIED",
    }
}
