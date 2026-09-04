//! Per-term forecast evaluation: Brier score, coverage, and abstention rate.
//!
//! Section 8.3: *예측 성능은 학기마다 Brier score/coverage와 abstention rate로
//! 검증한다.* Three numbers, per term.
//!
//! # Exact integers, no floating point
//!
//! A Brier score is a mean of squared errors, and computing it in binary
//! floating point would make the number depend on the platform that computed
//! it -- which is the opposite of what a calibration record is for. So the
//! score is carried as an exact rational: [`TermForecastMetrics::brier_numerator`]
//! is the sum of squared permille errors and
//! [`TermForecastMetrics::brier_denominator`] is how many forecasts it is a sum
//! over. Comparing one term against another, or against a threshold, is
//! cross-multiplication, exactly as `P2-U4` compares a grade-point average
//! without dividing. Nothing in this crate spells `f32`, `f64`, or a
//! floating-point literal.
//!
//! # Coverage and abstention are different questions
//!
//! *Abstention* is how often the forecaster declined to put a number on a
//! course: `abstained / total`. *Coverage* is how much of the term the
//! evaluation actually measured: `resolved / total`, where a forecast is
//! resolved when it was scored **and** the term's realized outcome was
//! recorded. They are not complements, and the gap between them is
//! [`TermForecastMetrics::missing_outcomes`] -- courses the forecaster spoke
//! about and nobody afterwards checked. `t001`'s `REQ-08-033` row asks for
//! exactly that: the three metrics *and* a report of missing outcomes.
//!
//! # An empty denominator is not a perfect score
//!
//! [`TermForecastMetrics::brier_numerator`] and its denominator are `None`
//! when nothing resolved. A Brier of zero over zero forecasts would read as a
//! flawless term, which is how a silently-degrading model stays invisible.

use academic_curriculum::CourseCode;
use academic_record::term::TermKey;

use crate::{
    error::OfferingError,
    forecast::{AbstentionReason, Forecast, ForecastVerdict},
};

/// What actually happened to a course in the term that was forecast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RealizedOutcome {
    /// The registration system listed a section.
    Offered,
    /// The registration system was read and listed none.
    NotOffered,
}

impl RealizedOutcome {
    /// Exhaustive listing.
    pub const ALL: [Self; 2] = [Self::Offered, Self::NotOffered];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Offered => "OFFERED",
            Self::NotOffered => "NOT_OFFERED",
        }
    }

    /// The permille a perfect forecast would have carried.
    #[must_use]
    pub const fn permille(self) -> i64 {
        match self {
            Self::Offered => 1000,
            Self::NotOffered => 0,
        }
    }
}

/// One course's forecast for one term, and what happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationEntry {
    course: CourseCode,
    scored_permille: Option<u16>,
    abstained: Option<AbstentionReason>,
    realized: Option<RealizedOutcome>,
}

impl EvaluationEntry {
    /// Reads one entry off a forecast and the outcome, when one was recorded.
    ///
    /// `realized` is an `Option` because *nobody checked* is a real state and
    /// is not the same as *it did not run*. Section 8.3's own sentence about
    /// never-observed courses is the same distinction one layer up.
    #[must_use]
    pub fn from_forecast(forecast: &Forecast, realized: Option<RealizedOutcome>) -> Self {
        let (scored_permille, abstained) = match forecast.verdict() {
            ForecastVerdict::Scored(scored) => (Some(scored.confidence().value()), None),
            ForecastVerdict::Abstained(reason) => (None, Some(*reason)),
        };
        Self {
            course: forecast.course().clone(),
            scored_permille,
            abstained,
            realized,
        }
    }

    /// The course.
    #[must_use]
    pub const fn course(&self) -> &CourseCode {
        &self.course
    }

    /// The calibrated permille, when the forecast produced one.
    #[must_use]
    pub const fn scored_permille(&self) -> Option<u16> {
        self.scored_permille
    }

    /// Why the forecast declined, when it did.
    #[must_use]
    pub const fn abstained(&self) -> Option<AbstentionReason> {
        self.abstained
    }

    /// What happened, when somebody recorded it.
    #[must_use]
    pub const fn realized(&self) -> Option<RealizedOutcome> {
        self.realized
    }
}

/// One term's evaluation set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermEvaluation {
    term: TermKey,
    entries: Vec<EvaluationEntry>,
}

impl TermEvaluation {
    /// Records one term's entries.
    ///
    /// # Errors
    ///
    /// [`OfferingError::EmptyEvaluation`] when the set is empty. Three metrics
    /// over nothing are three divisions by zero, and reporting them as zero is
    /// how a term nobody evaluated reads as a term that went perfectly.
    pub fn new(term: TermKey, entries: Vec<EvaluationEntry>) -> Result<Self, OfferingError> {
        if entries.is_empty() {
            return Err(OfferingError::EmptyEvaluation);
        }
        Ok(Self { term, entries })
    }

    /// The term evaluated.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// Every entry.
    #[must_use]
    pub fn entries(&self) -> &[EvaluationEntry] {
        &self.entries
    }

    /// Computes the three metrics.
    #[must_use]
    pub fn measure(&self) -> TermForecastMetrics {
        let total = self.entries.len();
        let abstained = self
            .entries
            .iter()
            .filter(|entry| entry.abstained().is_some())
            .count();
        let scored = total.saturating_sub(abstained);

        let mut brier_numerator: u64 = 0;
        let mut resolved: usize = 0;
        let mut missing_outcomes: Vec<CourseCode> = Vec::new();
        for entry in &self.entries {
            match (entry.scored_permille(), entry.realized()) {
                (Some(permille), Some(realized)) => {
                    let error = i64::from(permille) - realized.permille();
                    let squared = u64::try_from(error.saturating_mul(error)).unwrap_or(0);
                    brier_numerator = brier_numerator.saturating_add(squared);
                    resolved = resolved.saturating_add(1);
                }
                (Some(_), None) => missing_outcomes.push(entry.course().clone()),
                (None, _) => {}
            }
        }

        TermForecastMetrics {
            term: self.term,
            total,
            scored,
            abstained,
            resolved,
            brier_numerator: if resolved == 0 {
                None
            } else {
                Some(brier_numerator)
            },
            missing_outcomes,
        }
    }
}

/// Section 8.3's three per-term numbers, plus what was not measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermForecastMetrics {
    term: TermKey,
    total: usize,
    scored: usize,
    abstained: usize,
    resolved: usize,
    brier_numerator: Option<u64>,
    missing_outcomes: Vec<CourseCode>,
}

impl TermForecastMetrics {
    /// The term measured.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// How many courses the evaluation covers.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.total
    }

    /// How many the forecaster put a number on.
    #[must_use]
    pub const fn scored(&self) -> usize {
        self.scored
    }

    /// How many it declined.
    #[must_use]
    pub const fn abstained(&self) -> usize {
        self.abstained
    }

    /// How many were scored **and** have a recorded outcome.
    #[must_use]
    pub const fn resolved(&self) -> usize {
        self.resolved
    }

    /// The sum of squared permille errors, `None` when nothing resolved.
    #[must_use]
    pub const fn brier_numerator(&self) -> Option<u64> {
        self.brier_numerator
    }

    /// How many forecasts the numerator is a sum over, `None` when none did.
    #[must_use]
    pub const fn brier_denominator(&self) -> Option<usize> {
        if self.resolved == 0 {
            None
        } else {
            Some(self.resolved)
        }
    }

    /// The Brier score scaled by a million, floored.
    ///
    /// A permille error squared is already the score times a million, so this
    /// is the mean of the numerator and nothing is rescaled. It is a floor and
    /// is documented as one: comparisons that must be exact use the numerator
    /// and the denominator.
    #[must_use]
    pub fn brier_per_million_floor(&self) -> Option<u64> {
        let numerator = self.brier_numerator?;
        let denominator = u64::try_from(self.resolved).ok()?;
        if denominator == 0 {
            return None;
        }
        Some(numerator / denominator)
    }

    /// `abstained / total`, in permille, floored.
    #[must_use]
    pub fn abstention_permille(&self) -> u32 {
        ratio_permille(self.abstained, self.total)
    }

    /// `resolved / total`, in permille, floored.
    #[must_use]
    pub fn coverage_permille(&self) -> u32 {
        ratio_permille(self.resolved, self.total)
    }

    /// Courses the forecaster scored and nobody afterwards checked.
    #[must_use]
    pub fn missing_outcomes(&self) -> &[CourseCode] {
        &self.missing_outcomes
    }
}

fn ratio_permille(part: usize, whole: usize) -> u32 {
    if whole == 0 {
        return 0;
    }
    let part = u64::try_from(part).unwrap_or(0);
    let whole = u64::try_from(whole).unwrap_or(1);
    u32::try_from(part.saturating_mul(1000) / whole).unwrap_or(0)
}
