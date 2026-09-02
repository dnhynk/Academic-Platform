//! The views section 10 requires a calculation to keep separate.
//!
//! > 누적 GPA와 계산에 포함된 attempt proof / 학기별 GPA / 전공 GPA / 다전공별
//! > GPA / 총 취득학점과 GPA denominator의 차이 / 재수강 전후 시도와 어느 성적이
//! > 인정되었는지 / S/U, W, I, F, 교환·편입·인정학점 처리 이유
//!
//! Every one of those is a separate accessor here, and the two that are most
//! easily collapsed are separate *types*: an earned credit total and a
//! grade-point denominator are both credit quantities and are not the same
//! quantity. `credits_vs_denominator` is the acceptance row for that, and the
//! shipped corpus is built so the two differ by construction — an `S`, a `W`,
//! an `F`, and an excluded external attempt each move exactly one of them.
//!
//! Nothing here decides a policy question. Where the rule book has no row for
//! an attempt's term, or where the repeat-recognition rule is unstated, the
//! disposition is [`DispositionReason::RepeatRecognitionUnknown`] or
//! [`DispositionReason::ExternalPolicyUnknown`] and the affected averages are
//! [`GpaValue::Unknown`] naming the exact attempts. An average that silently
//! omitted them would be a different number presented as the same one.

use std::collections::{BTreeMap, BTreeSet};

use academic_domain::{AttemptId, Decimal};

use crate::{
    RecordError,
    attempt::{AttemptHistory, AttemptStatus, RepeatStatus},
    classify::{ClassificationRuleSet, ProgramId, RequirementCategory},
    decimal,
    facts::AttemptFacts,
    grade::{GradeSymbol, GradeTreatment},
    policy::{RecognitionDecision, RepeatRecognition, RuleBook},
    term::TermKey,
};

/// Why an attempt contributes what it does.
///
/// One reason per attempt, and the set is closed. This is the
/// `special_attempt_reason_matrix` vocabulary: every S/U, W, I, F, exchange,
/// transfer, and recognized attempt lands on exactly one of these, and the
/// matrix row asserts the mapping is total by walking the product of the
/// status, grade, and origin axes rather than by listing cases someone thought
/// of in advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DispositionReason {
    /// An ordinary graded attempt inside the average.
    Graded,
    /// A failed attempt: inside the average, earning no credit.
    FailedInDenominator,
    /// The recognized attempt of a repeat group, its grade capped by a ceiling.
    RepeatCeilingApplied,
    /// An earlier attempt a later one displaced.
    ReplacedByRepeat,
    /// `S` — credit earned, outside the average.
    SatisfactoryNotGraded,
    /// `U` — no credit, outside the average.
    UnsatisfactoryNotGraded,
    /// `W` — withdrawn.
    Withdrawn,
    /// `I` — the grade is not decided yet.
    IncompleteUnresolved,
    /// An external grade a dated policy row keeps out of the average.
    ExternalExcludedFromAverage,
    /// An external attempt whose term no policy row reaches.
    ExternalPolicyUnknown,
    /// External credits with no recorded recognition decision (`GATE-38-006`).
    RecognitionUndecided,
    /// A repeat group whose recognition rule no confirmed source states.
    RepeatRecognitionUnknown,
    /// Not settled: planned, registered, in progress, or cancelled.
    NotSettled,
}

impl DispositionReason {
    /// Every reason.
    pub const ALL: [Self; 13] = [
        Self::Graded,
        Self::FailedInDenominator,
        Self::RepeatCeilingApplied,
        Self::ReplacedByRepeat,
        Self::SatisfactoryNotGraded,
        Self::UnsatisfactoryNotGraded,
        Self::Withdrawn,
        Self::IncompleteUnresolved,
        Self::ExternalExcludedFromAverage,
        Self::ExternalPolicyUnknown,
        Self::RecognitionUndecided,
        Self::RepeatRecognitionUnknown,
        Self::NotSettled,
    ];

    /// Returns the contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Graded => "GRADED",
            Self::FailedInDenominator => "FAILED_IN_DENOMINATOR",
            Self::RepeatCeilingApplied => "REPEAT_CEILING_APPLIED",
            Self::ReplacedByRepeat => "REPLACED_BY_REPEAT",
            Self::SatisfactoryNotGraded => "SATISFACTORY_NOT_GRADED",
            Self::UnsatisfactoryNotGraded => "UNSATISFACTORY_NOT_GRADED",
            Self::Withdrawn => "WITHDRAWN",
            Self::IncompleteUnresolved => "INCOMPLETE_UNRESOLVED",
            Self::ExternalExcludedFromAverage => "EXTERNAL_EXCLUDED_FROM_AVERAGE",
            Self::ExternalPolicyUnknown => "EXTERNAL_POLICY_UNKNOWN",
            Self::RecognitionUndecided => "RECOGNITION_UNDECIDED",
            Self::RepeatRecognitionUnknown => "REPEAT_RECOGNITION_UNKNOWN",
            Self::NotSettled => "NOT_SETTLED",
        }
    }

    /// Resolves a reason from its spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.as_str() == text)
    }

    /// Whether the reason leaves a value the record does not have.
    ///
    /// An unknown is never folded into a zero: an average over an attempt set
    /// containing one is [`GpaValue::Unknown`] naming the attempts, rather than
    /// an average over the rest presented as the whole.
    #[must_use]
    pub const fn is_unknown(self) -> bool {
        matches!(
            self,
            Self::IncompleteUnresolved
                | Self::ExternalPolicyUnknown
                | Self::RecognitionUndecided
                | Self::RepeatRecognitionUnknown
        )
    }
}

/// What one attempt contributes to the grade-point average.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AverageContribution {
    /// Inside the average: these quality points over these credits.
    Included {
        /// The grade the average was computed from, after any ceiling.
        effective_grade: GradeSymbol,
        /// `credits × grade points`, exact.
        quality_points: Decimal,
        /// The credits this attempt puts in the denominator.
        denominator_credits: Decimal,
    },
    /// Outside the average, for a settled reason.
    Excluded,
    /// Whether it belongs in the average is not known.
    Unknown,
}

/// What one attempt contributes to the earned-credit total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditContribution {
    /// Credits earned.
    Earned(Decimal),
    /// No credits earned, for a settled reason.
    NotEarned,
    /// Whether the credits count is not known.
    Unknown,
}

/// One attempt's full disposition under one rule book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptDisposition {
    attempt_id: AttemptId,
    course_code: String,
    term: TermKey,
    recorded_grade: Option<GradeSymbol>,
    reason: DispositionReason,
    average: AverageContribution,
    credit: CreditContribution,
    policy_row_id: Option<String>,
}

impl AttemptDisposition {
    /// Returns the attempt this disposition is for.
    #[must_use]
    pub const fn attempt_id(&self) -> AttemptId {
        self.attempt_id
    }

    /// Returns the course code.
    #[must_use]
    pub fn course_code(&self) -> &str {
        &self.course_code
    }

    /// Returns the term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// Returns the grade as the transcript recorded it, before any ceiling.
    #[must_use]
    pub const fn recorded_grade(&self) -> Option<GradeSymbol> {
        self.recorded_grade
    }

    /// Returns why the attempt contributes what it does.
    #[must_use]
    pub const fn reason(&self) -> DispositionReason {
        self.reason
    }

    /// Returns the average contribution.
    #[must_use]
    pub const fn average(&self) -> AverageContribution {
        self.average
    }

    /// Returns the earned-credit contribution.
    #[must_use]
    pub const fn credit(&self) -> CreditContribution {
        self.credit
    }

    /// Returns the effective-dated policy row that decided this, if one did.
    #[must_use]
    pub fn policy_row_id(&self) -> Option<&str> {
        self.policy_row_id.as_deref()
    }
}

/// A published average, or the reason there is not one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpaValue {
    /// The average, at the scheme's published scale.
    Known(Decimal),
    /// No attempt in this view reached the denominator.
    NoGradedAttempts,
    /// At least one attempt's disposition is unknown; these are the attempts.
    Unknown(Vec<AttemptId>),
}

impl GpaValue {
    /// Returns the average when there is one.
    #[must_use]
    pub const fn known(&self) -> Option<Decimal> {
        match self {
            Self::Known(value) => Some(*value),
            Self::NoGradedAttempts | Self::Unknown(_) => None,
        }
    }
}

/// One repeat group, before and after, and which grade was recognized.
///
/// Section 10 asks for exactly this: "재수강 전후 시도와 어느 성적이
/// 인정되었는지". The recognized attempt is named, the displaced ones are named,
/// and when no confirmed rule says which is which, `recognized` is `None` and
/// `recognition_rule` says why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatProof {
    /// The course the group is for.
    pub course_code: String,
    /// Every settled attempt in the group, in term order.
    pub attempts: Vec<AttemptId>,
    /// The attempt whose grade was recognized, if a rule decided.
    pub recognized: Option<AttemptId>,
    /// The attempts the recognized one displaced.
    pub displaced: Vec<AttemptId>,
    /// The rule that decided, or [`RepeatRecognition::Unknown`].
    pub recognition_rule: RepeatRecognition,
    /// The effective-dated policy row consulted, if one reached the term.
    pub policy_row_id: Option<String>,
    /// The ceiling that row imposed, if any.
    pub ceiling: Option<GradeSymbol>,
    /// Whether the ceiling actually lowered the recognized grade.
    pub ceiling_applied: bool,
}

/// Every view over one attempt set under one rule book.
///
/// Built once and read many times, because every view has to be a projection of
/// the *same* dispositions. Two views computed by two walks could disagree
/// about one attempt and nothing would notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordViews {
    dispositions: Vec<AttemptDisposition>,
    categories: BTreeMap<AttemptId, BTreeMap<ProgramId, RequirementCategory>>,
    repeat_proofs: Vec<RepeatProof>,
    published_scale: u8,
}

impl RecordViews {
    /// Computes every disposition and view over `history` under `rules`.
    pub fn compute(
        history: &AttemptHistory,
        rules: &RuleBook,
        classification: &ClassificationRuleSet,
    ) -> Result<Self, RecordError> {
        let facts: Vec<AttemptFacts> = history
            .current()
            .into_iter()
            .map(|attempt| AttemptFacts::from_attempt(attempt, classification))
            .collect();
        Self::from_facts(&facts, rules)
    }

    /// Computes every disposition and view over an already-frozen fact set.
    ///
    /// The deterministic engines call this after decoding their frozen inputs,
    /// so a golden fixture and a product call run the same arithmetic over the
    /// same values rather than over two paths that agree by inspection.
    pub fn from_facts(facts: &[AttemptFacts], rules: &RuleBook) -> Result<Self, RecordError> {
        let repeat_proofs = resolve_repeat_groups(facts, rules)?;
        let displaced: BTreeSet<AttemptId> = repeat_proofs
            .iter()
            .flat_map(|proof| proof.displaced.iter().copied())
            .collect();
        let undecided_groups: BTreeSet<AttemptId> = repeat_proofs
            .iter()
            .filter(|proof| proof.recognition_rule == RepeatRecognition::Unknown)
            .flat_map(|proof| proof.attempts.iter().copied())
            .collect();
        let ceilinged: BTreeMap<AttemptId, GradeSymbol> = repeat_proofs
            .iter()
            .filter(|proof| proof.ceiling_applied)
            .filter_map(|proof| proof.recognized.zip(proof.ceiling))
            .collect();

        let mut dispositions = Vec::with_capacity(facts.len());
        let mut categories = BTreeMap::new();
        for attempt in facts {
            dispositions.push(disposition_for(
                attempt,
                rules,
                &undecided_groups,
                &displaced,
                &ceilinged,
            )?);
            categories.insert(attempt.id, attempt.categories.clone());
        }
        dispositions.sort_by(|left, right| {
            left.term
                .cmp(&right.term)
                .then_with(|| left.course_code.cmp(&right.course_code))
                .then_with(|| left.attempt_id.cmp(&right.attempt_id))
        });

        Ok(Self {
            dispositions,
            categories,
            repeat_proofs,
            published_scale: rules.scheme().published_scale(),
        })
    }

    /// Returns every attempt's disposition, in term then course order.
    #[must_use]
    pub fn dispositions(&self) -> &[AttemptDisposition] {
        &self.dispositions
    }

    /// Returns the repeat proof for every course attempted more than once.
    #[must_use]
    pub fn repeat_proofs(&self) -> &[RepeatProof] {
        &self.repeat_proofs
    }

    /// Returns one attempt's category per programme.
    #[must_use]
    pub fn categories(
        &self,
        attempt_id: AttemptId,
    ) -> Option<&BTreeMap<ProgramId, RequirementCategory>> {
        self.categories.get(&attempt_id)
    }

    /// The cumulative average, over every attempt in the set.
    pub fn cumulative_gpa(&self) -> Result<GpaValue, RecordError> {
        self.average_over(self.dispositions.iter())
    }

    /// The attempts the cumulative average was computed from.
    ///
    /// This is the "계산에 포함된 attempt proof" half of the cumulative view: a
    /// number with no list of what went into it cannot be checked.
    #[must_use]
    pub fn cumulative_included(&self) -> Vec<AttemptId> {
        self.dispositions
            .iter()
            .filter(|disposition| {
                matches!(disposition.average, AverageContribution::Included { .. })
            })
            .map(AttemptDisposition::attempt_id)
            .collect()
    }

    /// Every term present in the attempt set, in order.
    #[must_use]
    pub fn terms(&self) -> Vec<TermKey> {
        let mut terms: Vec<TermKey> = self
            .dispositions
            .iter()
            .map(AttemptDisposition::term)
            .collect();
        terms.sort();
        terms.dedup();
        terms
    }

    /// One term's average.
    pub fn term_gpa(&self, term: TermKey) -> Result<GpaValue, RecordError> {
        self.average_over(
            self.dispositions
                .iter()
                .filter(|disposition| disposition.term == term),
        )
    }

    /// One programme's major average.
    ///
    /// Uses only attempts the rule set classified as 전필 or 전선 **for that
    /// programme**. The filter reads
    /// [`RequirementCategory::is_major`] on a category the rule engine
    /// produced; there is no user label anywhere on this path.
    pub fn major_gpa(&self, program: &ProgramId) -> Result<GpaValue, RecordError> {
        self.average_over(self.dispositions.iter().filter(|disposition| {
            self.categories
                .get(&disposition.attempt_id)
                .and_then(|categories| categories.get(program))
                .is_some_and(|category| category.is_major())
        }))
    }

    /// Every programme the classification rule set spoke for, in order.
    #[must_use]
    pub fn programs(&self) -> Vec<ProgramId> {
        let mut programs: Vec<ProgramId> = self
            .categories
            .values()
            .flat_map(BTreeMap::keys)
            .cloned()
            .collect();
        programs.sort();
        programs.dedup();
        programs
    }

    /// The total credits earned.
    ///
    /// **Not** the grade-point denominator. An `S` adds here and not there; an
    /// `F` adds there and not here. [`Self::gpa_denominator`] is the other
    /// quantity, and `credits_vs_denominator` requires them to differ on the
    /// shipped corpus.
    pub fn earned_credits(&self) -> Result<CreditTotal, RecordError> {
        let mut total = decimal::zero()?;
        let mut unknown = Vec::new();
        for disposition in &self.dispositions {
            match disposition.credit {
                CreditContribution::Earned(credits) => total = decimal::add(total, credits)?,
                CreditContribution::NotEarned => {}
                CreditContribution::Unknown => unknown.push(disposition.attempt_id),
            }
        }
        Ok(CreditTotal {
            partial: total,
            unknown,
        })
    }

    /// The credits in the grade-point denominator.
    pub fn gpa_denominator(&self) -> Result<CreditTotal, RecordError> {
        let mut total = decimal::zero()?;
        let mut unknown = Vec::new();
        for disposition in &self.dispositions {
            match disposition.average {
                AverageContribution::Included {
                    denominator_credits,
                    ..
                } => total = decimal::add(total, denominator_credits)?,
                AverageContribution::Excluded => {}
                AverageContribution::Unknown => unknown.push(disposition.attempt_id),
            }
        }
        Ok(CreditTotal {
            partial: total,
            unknown,
        })
    }

    /// The exact quality-point numerator the cumulative average divides.
    pub fn quality_points(&self) -> Result<Decimal, RecordError> {
        let mut total = decimal::zero()?;
        for disposition in &self.dispositions {
            if let AverageContribution::Included { quality_points, .. } = disposition.average {
                total = decimal::add(total, quality_points)?;
            }
        }
        Ok(total)
    }

    /// Returns the scale averages are published to.
    #[must_use]
    pub const fn published_scale(&self) -> u8 {
        self.published_scale
    }

    fn average_over<'a>(
        &self,
        dispositions: impl Iterator<Item = &'a AttemptDisposition>,
    ) -> Result<GpaValue, RecordError> {
        let mut numerator = decimal::zero()?;
        let mut denominator = decimal::zero()?;
        let mut unknown = Vec::new();
        let mut graded = false;
        for disposition in dispositions {
            match disposition.average {
                AverageContribution::Included {
                    quality_points,
                    denominator_credits,
                    ..
                } => {
                    graded = true;
                    numerator = decimal::add(numerator, quality_points)?;
                    denominator = decimal::add(denominator, denominator_credits)?;
                }
                AverageContribution::Excluded => {}
                AverageContribution::Unknown => unknown.push(disposition.attempt_id),
            }
        }
        if !unknown.is_empty() {
            return Ok(GpaValue::Unknown(unknown));
        }
        if !graded || decimal::is_zero(denominator) {
            return Ok(GpaValue::NoGradedAttempts);
        }
        Ok(GpaValue::Known(decimal::div_round_half_up(
            numerator,
            denominator,
            self.published_scale,
        )?))
    }
}

/// A credit total, and the attempts whose contribution is not known.
///
/// The fields are private and the two numbers have different names on purpose.
/// A struct with a public `total` beside a list of pending attempts is read as
/// a total: a caller writes `.total`, gets the sum of everything that *was*
/// known, and presents it as the whole. That is the same defect the engines
/// avoid by publishing no derived value under `CONFLICT`, one step outside
/// them, so it is closed the same way — [`Self::complete`] returns `None` while
/// anything is pending, and a caller that wants the partial sum has to say
/// [`Self::partial`] and has thereby said what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditTotal {
    partial: Decimal,
    unknown: Vec<AttemptId>,
}

impl CreditTotal {
    /// The total, when every attempt's contribution to it is known.
    #[must_use]
    pub fn complete(&self) -> Option<Decimal> {
        self.unknown.is_empty().then_some(self.partial)
    }

    /// The sum of the contributions that are known, whatever is pending.
    #[must_use]
    pub const fn partial(&self) -> Decimal {
        self.partial
    }

    /// The attempts whose contribution is not known.
    #[must_use]
    pub fn unknown(&self) -> &[AttemptId] {
        &self.unknown
    }
}

/// Resolves every course attempted more than once into a [`RepeatProof`].
fn resolve_repeat_groups(
    facts: &[AttemptFacts],
    rules: &RuleBook,
) -> Result<Vec<RepeatProof>, RecordError> {
    let mut grouped: BTreeMap<String, Vec<&AttemptFacts>> = BTreeMap::new();
    for attempt in facts {
        if attempt.status.is_settled() {
            grouped
                .entry(attempt.course_code.clone())
                .or_default()
                .push(attempt);
        }
    }

    let mut proofs = Vec::new();
    for (course_code, mut group) in grouped {
        let is_repeat_group = group.len() > 1
            && group.iter().any(|attempt| {
                matches!(
                    attempt.repeat_status,
                    RepeatStatus::Repeat | RepeatStatus::Replaced
                )
            });
        if !is_repeat_group {
            continue;
        }
        group.sort_by(|left, right| {
            left.term
                .cmp(&right.term)
                .then_with(|| left.id.cmp(&right.id))
        });

        // The row is selected on the *latest* attempt's term: a ceiling that
        // applies "from courses taken in 2015 spring onward" is a statement
        // about the repeat, not about the course it repeats.
        let latest = *group.last().ok_or(RecordError::EmptyRepeatGroup)?;
        let row = rules.policies().repeat_row_at(latest.term);
        let recognition = row.map_or(RepeatRecognition::Unknown, |row| row.recognition);
        let ceiling = row.and_then(|row| row.ceiling);

        let recognized = match recognition {
            RepeatRecognition::Unknown => None,
            RepeatRecognition::LatestAttempt => Some(latest),
            RepeatRecognition::HighestAttempt => highest_graded(&group, rules)?,
        };
        let ceiling_applied = match (recognized, ceiling) {
            (Some(attempt), Some(ceiling)) => attempt
                .grade
                .is_some_and(|grade| exceeds_ceiling(grade, ceiling, rules)),
            _ => false,
        };
        let displaced = recognized.map_or_else(Vec::new, |winner| {
            group
                .iter()
                .filter(|attempt| attempt.id != winner.id)
                .map(|attempt| attempt.id)
                .collect()
        });

        proofs.push(RepeatProof {
            course_code,
            attempts: group.iter().map(|attempt| attempt.id).collect(),
            recognized: recognized.map(|attempt| attempt.id),
            displaced,
            recognition_rule: recognition,
            policy_row_id: row.map(|row| row.row_id.clone()),
            ceiling,
            ceiling_applied,
        });
    }
    Ok(proofs)
}

/// Returns the group's highest-graded attempt, or `None` if none is graded.
fn highest_graded<'a>(
    group: &[&'a AttemptFacts],
    rules: &RuleBook,
) -> Result<Option<&'a AttemptFacts>, RecordError> {
    let mut best: Option<(&'a AttemptFacts, Decimal)> = None;
    for attempt in group {
        let Some(grade) = attempt.grade else {
            continue;
        };
        let Some(points) = rules.scheme().treatment(grade).grade_points() else {
            continue;
        };
        let replace = match best {
            None => true,
            Some((_, current)) => decimal::compare(points, current)?.is_gt(),
        };
        if replace {
            best = Some((attempt, points));
        }
    }
    Ok(best.map(|(attempt, _)| attempt))
}

/// Whether `grade` is worth strictly more than `ceiling` under the scheme.
fn exceeds_ceiling(grade: GradeSymbol, ceiling: GradeSymbol, rules: &RuleBook) -> bool {
    let scheme = rules.scheme();
    match (
        scheme.treatment(grade).grade_points(),
        scheme.treatment(ceiling).grade_points(),
    ) {
        (Some(actual), Some(limit)) => {
            decimal::compare(actual, limit).is_ok_and(core::cmp::Ordering::is_gt)
        }
        _ => false,
    }
}

/// Computes one attempt's disposition.
fn disposition_for(
    attempt: &AttemptFacts,
    rules: &RuleBook,
    undecided_groups: &BTreeSet<AttemptId>,
    displaced: &BTreeSet<AttemptId>,
    ceilinged: &BTreeMap<AttemptId, GradeSymbol>,
) -> Result<AttemptDisposition, RecordError> {
    let build = |reason: DispositionReason,
                 average: AverageContribution,
                 credit: CreditContribution,
                 policy_row_id: Option<String>| {
        Ok(AttemptDisposition {
            attempt_id: attempt.id,
            course_code: attempt.course_code.clone(),
            term: attempt.term,
            recorded_grade: attempt.grade,
            reason,
            average,
            credit,
            policy_row_id,
        })
    };

    // A plan never raises actual progress, and neither does a registration.
    if !attempt.status.is_settled() {
        return build(
            DispositionReason::NotSettled,
            AverageContribution::Excluded,
            CreditContribution::NotEarned,
            None,
        );
    }

    // A repeat group nothing confirmed decides leaves every attempt in it
    // unknown, including the one that would have won: which grade counts is
    // exactly what is not known.
    if undecided_groups.contains(&attempt.id) {
        return build(
            DispositionReason::RepeatRecognitionUnknown,
            AverageContribution::Unknown,
            CreditContribution::Unknown,
            rules
                .policies()
                .repeat_row_at(attempt.term)
                .map(|row| row.row_id.clone()),
        );
    }

    if displaced.contains(&attempt.id) {
        return build(
            DispositionReason::ReplacedByRepeat,
            AverageContribution::Excluded,
            CreditContribution::NotEarned,
            rules
                .policies()
                .repeat_row_at(attempt.term)
                .map(|row| row.row_id.clone()),
        );
    }

    let Some(grade) = attempt.grade else {
        // A settled attempt with no grade symbol is not a shape any import
        // format produces; treating it as unknown is the fail-closed reading.
        return build(
            DispositionReason::IncompleteUnresolved,
            AverageContribution::Unknown,
            CreditContribution::Unknown,
            None,
        );
    };
    let treatment = rules.scheme().treatment(grade);

    if treatment.is_unresolved() {
        return build(
            DispositionReason::IncompleteUnresolved,
            AverageContribution::Unknown,
            CreditContribution::Unknown,
            None,
        );
    }

    // External origin: a dated row decides the average, a user decision decides
    // the credits, and each may be absent independently.
    if attempt.origin.is_external() {
        let Some(row) = rules.policies().external_row_at(attempt.term) else {
            return build(
                DispositionReason::ExternalPolicyUnknown,
                AverageContribution::Unknown,
                CreditContribution::Unknown,
                None,
            );
        };
        let credit = match attempt.recognition {
            RecognitionDecision::Recognized => CreditContribution::Earned(attempt.credits_earned),
            RecognitionDecision::NotRecognized => CreditContribution::NotEarned,
            RecognitionDecision::Undecided => CreditContribution::Unknown,
        };
        let undecided = matches!(credit, CreditContribution::Unknown);
        if row.excluded_from_average {
            let reason = if undecided {
                DispositionReason::RecognitionUndecided
            } else {
                DispositionReason::ExternalExcludedFromAverage
            };
            return build(
                reason,
                AverageContribution::Excluded,
                credit,
                Some(row.row_id.clone()),
            );
        }
        // A row that does not exclude puts the grade back on the ordinary
        // path, but the credits still wait on their own decision.
        let average = average_contribution(attempt, grade, treatment, ceilinged, rules)?;
        let reason = if undecided {
            DispositionReason::RecognitionUndecided
        } else {
            DispositionReason::Graded
        };
        return build(reason, average, credit, Some(row.row_id.clone()));
    }

    let credit = if treatment.earns_credit() {
        CreditContribution::Earned(attempt.credits_earned)
    } else {
        CreditContribution::NotEarned
    };

    if !treatment.participates_in_average() {
        let reason = match grade {
            GradeSymbol::S => DispositionReason::SatisfactoryNotGraded,
            GradeSymbol::U => DispositionReason::UnsatisfactoryNotGraded,
            _ => DispositionReason::Withdrawn,
        };
        return build(reason, AverageContribution::Excluded, credit, None);
    }

    let average = average_contribution(attempt, grade, treatment, ceilinged, rules)?;
    let capped = ceilinged.contains_key(&attempt.id);
    let reason = if capped {
        DispositionReason::RepeatCeilingApplied
    } else if grade == GradeSymbol::F {
        DispositionReason::FailedInDenominator
    } else {
        DispositionReason::Graded
    };
    let policy_row_id = if capped {
        rules
            .policies()
            .repeat_row_at(attempt.term)
            .map(|row| row.row_id.clone())
    } else {
        None
    };
    build(reason, average, credit, policy_row_id)
}

/// Builds the average contribution, applying a repeat ceiling where one bound.
fn average_contribution(
    attempt: &AttemptFacts,
    grade: GradeSymbol,
    treatment: GradeTreatment,
    ceilinged: &BTreeMap<AttemptId, GradeSymbol>,
    rules: &RuleBook,
) -> Result<AverageContribution, RecordError> {
    let effective_grade = ceilinged.get(&attempt.id).copied().unwrap_or(grade);
    let effective_treatment = if effective_grade == grade {
        treatment
    } else {
        rules.scheme().treatment(effective_grade)
    };
    let Some(points) = effective_treatment.grade_points() else {
        return Ok(AverageContribution::Unknown);
    };
    let denominator_credits = attempt.credits_attempted;
    Ok(AverageContribution::Included {
        effective_grade,
        quality_points: decimal::mul(denominator_credits, points)?,
        denominator_credits,
    })
}

/// Whether an attempt status is one the numeric engines read at all.
///
/// Exposed so `registered_attempt_gate` can enumerate the status set and assert
/// the partition rather than spot-check two values.
#[must_use]
pub const fn contributes_to_actual_progress(status: AttemptStatus) -> bool {
    status.is_settled()
}
