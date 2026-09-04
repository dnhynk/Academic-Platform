//! Section 8.3's feature families, and the observation window they read.
//!
//! *역사 기반 예측은 최근 N개 학기의 단순 다수결이 아니다. 계절성(1/2학기),
//! 교과목 신설·폐지·대체, 교수자 변화, 최근 공지, 미개설 gap, 불규칙 특강 여부를
//! feature로 사용하고, Course별 calibrated probability와 표본 window를 남긴다.*
//!
//! # Six named features, seven named families
//!
//! That sentence names **six** things as features and then requires the sample
//! window to be recorded beside the probability. `t068` section 5's `P2-U5`
//! entry and `t001`'s `REQ-08-029` row both say **seven feature families** and
//! both resolve the seventh as the history window. This module implements
//! seven, and the seventh is the window itself: how many same-semester terms
//! the forecast actually got to read. That is a feature here rather than only
//! an output because two courses with the same seasonal rate and the same gap
//! are not equally predictable when one was read four times and the other
//! twice.
//!
//! `the_feature_families_are_section_8_3s_own` reads the sentence out of the
//! design document, splits it at *를 feature로 사용하고*, and compares the six
//! units against the first six families in order and in both directions; the
//! seventh is required to be the *표본 window* the same sentence writes. So the
//! divergence between six and seven is executed rather than asserted, and a
//! paraphrase on either side fails.
//!
//! # Every family moves the score, and that is measured
//!
//! Declaring a family and not using it is the failure this module is written
//! against. Each family's contribution is its own `match`, the whole mapping is
//! pinned in [`crate::forecast::FORECAST_RULE_SET`] so the rule-set hash covers
//! it, and `offering_feature_contract` varies **one family at a time** against
//! a fixed baseline and requires the raw score to move each time. A family
//! whose arm collapsed to a constant fails there rather than passing silently.
//!
//! # Seasonal by construction
//!
//! Section 8.3's first feature is 계절성(1/2학기), so the window a forecast for
//! 2027 spring reads is the **spring** terms of the history and not the last N
//! terms. [`ObservationWindow::seasonal_terms`] is that restriction, and it is
//! what makes the seasonal rate, the gap and the window depth all answers about
//! the same semester rather than about a mixed run. It is also what separates
//! this from the majority vote the specification refuses: a course that runs
//! every spring and never in autumn reads 1000 permille for spring and 0 for
//! autumn, where a vote over the last N terms reads 500 for both.

use academic_record::term::{Semester, TermKey};

use crate::{
    error::OfferingError,
    observation::{CourseHistory, CourseLifecycle, NoticeEffect, Offered},
};

/// Section 8.3's feature families.
///
/// The order is the specification's own: 계절성, 신설·폐지·대체, 교수자 변화,
/// 최근 공지, 미개설 gap, 불규칙 특강, and then the sample window the same
/// sentence requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeatureFamily {
    /// 계절성(1/2학기): how often this course ran in the target term's own
    /// semester.
    Seasonality,
    /// 교과목 신설·폐지·대체: the official course-change record's reading of
    /// this course against the target term.
    LifecycleStatus,
    /// 교수자 변화: how many distinct instructor sets taught the offered terms
    /// in the window.
    InstructorChange,
    /// 최근 공지: official notices issued inside the window.
    RecentNotices,
    /// 미개설 gap: how many same-semester terms have passed since the course
    /// last ran.
    OfferingGap,
    /// 불규칙 특강 여부: how many of the offered terms were one-off or
    /// special runs.
    IrregularSpecial,
    /// 표본 window: how many same-semester terms the window actually read.
    HistoryWindow,
}

impl FeatureFamily {
    /// Every family, in section 8.3's order.
    pub const ALL: [Self; 7] = [
        Self::Seasonality,
        Self::LifecycleStatus,
        Self::InstructorChange,
        Self::RecentNotices,
        Self::OfferingGap,
        Self::IrregularSpecial,
        Self::HistoryWindow,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seasonality => "SEASONALITY",
            Self::LifecycleStatus => "LIFECYCLE_STATUS",
            Self::InstructorChange => "INSTRUCTOR_CHANGE",
            Self::RecentNotices => "RECENT_NOTICES",
            Self::OfferingGap => "OFFERING_GAP",
            Self::IrregularSpecial => "IRREGULAR_SPECIAL",
            Self::HistoryWindow => "HISTORY_WINDOW",
        }
    }

    /// The frozen-input key this family's value travels under.
    #[must_use]
    pub const fn input_key(self) -> &'static str {
        match self {
            Self::Seasonality => "feature.seasonality",
            Self::LifecycleStatus => "feature.lifecycle_status",
            Self::InstructorChange => "feature.instructor_change",
            Self::RecentNotices => "feature.recent_notices",
            Self::OfferingGap => "feature.offering_gap",
            Self::IrregularSpecial => "feature.irregular_special",
            Self::HistoryWindow => "feature.history_window",
        }
    }

    /// The words section 8.3 uses for this family, verbatim.
    ///
    /// Compared against the design document by
    /// `the_feature_families_are_section_8_3s_own`, so a paraphrase fails. The
    /// first six are the sentence's own comma-separated units; the seventh is
    /// the sample window the same sentence requires be recorded.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::Seasonality => "계절성(1/2학기)",
            Self::LifecycleStatus => "교과목 신설·폐지·대체",
            Self::InstructorChange => "교수자 변화",
            Self::RecentNotices => "최근 공지",
            Self::OfferingGap => "미개설 gap",
            Self::IrregularSpecial => "불규칙 특강 여부",
            Self::HistoryWindow => "표본 window",
        }
    }
}

/// The bounded stretch of history one forecast reads.
///
/// Half-open on the term axis: `[from, to)`, where `to` is the term being
/// forecast. A forecast that could read its own term would be reading the
/// answer, so [`Self::new`] refuses `from >= to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationWindow {
    from: TermKey,
    to: TermKey,
}

impl ObservationWindow {
    /// The only constructor.
    pub fn new(from: TermKey, to: TermKey) -> Result<Self, OfferingError> {
        if from >= to {
            return Err(OfferingError::EmptyWindow);
        }
        Ok(Self { from, to })
    }

    /// The inclusive first term.
    #[must_use]
    pub const fn from(self) -> TermKey {
        self.from
    }

    /// The exclusive last term, which is the term being forecast.
    #[must_use]
    pub const fn to(self) -> TermKey {
        self.to
    }

    /// Whether a term is inside the window.
    #[must_use]
    pub fn contains(self, term: TermKey) -> bool {
        self.from <= term && term < self.to
    }

    /// The window's terms in this history that share the forecast semester,
    /// oldest first.
    ///
    /// This is section 8.3's 계절성: a spring forecast is answered from spring
    /// terms.
    #[must_use]
    pub fn seasonal_terms(
        self,
        history: &CourseHistory,
    ) -> Vec<&crate::observation::TermObservation> {
        let semester = self.to.semester();
        history
            .observations()
            .filter(|observation| {
                self.contains(observation.term()) && observation.term().semester() == semester
            })
            .collect()
    }

    /// The forecast semester.
    #[must_use]
    pub const fn semester(self) -> Semester {
        self.to.semester()
    }
}

/// One family's reading of one history, and what it contributed.
///
/// `value` is the observable quantity the family measured; `contribution` is
/// what the pinned rule set turns that value into. Both travel: the value is a
/// frozen input, and the contribution appears in the proof tree, so an
/// explanation says *what was measured* as well as *what it was worth*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureSignal {
    family: FeatureFamily,
    value: i64,
    contribution: i32,
}

impl FeatureSignal {
    /// Which family.
    #[must_use]
    pub const fn family(self) -> FeatureFamily {
        self.family
    }

    /// The measured value.
    #[must_use]
    pub const fn value(self) -> i64 {
        self.value
    }

    /// What the rule set turned that value into.
    #[must_use]
    pub const fn contribution(self) -> i32 {
        self.contribution
    }
}

/// Every family's reading of one history, in section 8.3's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureVector {
    signals: [FeatureSignal; FeatureFamily::ALL.len()],
    window: ObservationWindow,
    seasonal_terms: u32,
    positive_samples: u32,
}

/// The score a history with nothing to say about it starts from.
///
/// Five hundred permille is *no information*, not *even odds measured*. Every
/// family moves it away from there, and a history in which every family is
/// silent produces exactly this -- which the abstention floor then refuses,
/// because a score nothing moved is not a forecast.
pub const BASE_RAW_UNITS: i32 = 500;

/// The widest a raw score can be.
pub const MAX_RAW_UNITS: i32 = 1000;

impl FeatureVector {
    /// Reads every family off one history.
    #[must_use]
    pub fn extract(history: &CourseHistory, window: ObservationWindow) -> Self {
        let seasonal = window.seasonal_terms(history);
        let seasonal_terms = u32::try_from(seasonal.len()).unwrap_or(u32::MAX);
        let positive_samples = u32::try_from(
            seasonal
                .iter()
                .filter(|observation| observation.outcome() == Offered::Yes)
                .count(),
        )
        .unwrap_or(u32::MAX);

        let signals = [
            seasonality(seasonal_terms, positive_samples),
            lifecycle_status(history.lifecycle(), window.to()),
            instructor_change(&seasonal),
            recent_notices(history, window),
            offering_gap(&seasonal),
            irregular_special(&seasonal),
            history_window(seasonal_terms),
        ];

        Self {
            signals,
            window,
            seasonal_terms,
            positive_samples,
        }
    }

    /// Every signal, in section 8.3's order.
    #[must_use]
    pub const fn signals(&self) -> &[FeatureSignal; FeatureFamily::ALL.len()] {
        &self.signals
    }

    /// One family's signal.
    #[must_use]
    pub fn signal(&self, family: FeatureFamily) -> FeatureSignal {
        // `FeatureVector` is built from `FeatureFamily::ALL` in order, so the
        // position is the family. The fallback is the first signal rather than
        // a panic, and it is unreachable because both arrays are built from the
        // same constant.
        let position = FeatureFamily::ALL
            .iter()
            .position(|candidate| *candidate == family)
            .unwrap_or(0);
        self.signals
            .get(position)
            .copied()
            .unwrap_or(self.signals[0])
    }

    /// The window this vector was read over.
    #[must_use]
    pub const fn window(&self) -> ObservationWindow {
        self.window
    }

    /// How many same-semester terms the window actually read.
    #[must_use]
    pub const fn seasonal_terms(&self) -> u32 {
        self.seasonal_terms
    }

    /// How many of those held a section.
    ///
    /// This is the `positive_sample_count` of
    /// `academic_domain::PredictionMetadata`, which refuses zero -- so a
    /// never-observed course has no metadata to disclose and therefore no
    /// prediction to make.
    #[must_use]
    pub const fn positive_samples(&self) -> u32 {
        self.positive_samples
    }

    /// The raw score every contribution sums to, clamped to the unit range.
    #[must_use]
    pub fn raw_units(&self) -> u32 {
        let total = self
            .signals
            .iter()
            .fold(BASE_RAW_UNITS, |accumulated, signal| {
                accumulated.saturating_add(signal.contribution())
            })
            .clamp(0, MAX_RAW_UNITS);
        u32::try_from(total).unwrap_or(0)
    }
}

fn signal(family: FeatureFamily, value: i64, contribution: i32) -> FeatureSignal {
    FeatureSignal {
        family,
        value,
        contribution,
    }
}

/// 계절성: the offered rate over same-semester terms, in permille.
fn seasonality(seasonal_terms: u32, positive_samples: u32) -> FeatureSignal {
    if seasonal_terms == 0 {
        return signal(FeatureFamily::Seasonality, 0, 0);
    }
    let rate = i64::from(positive_samples) * 1000 / i64::from(seasonal_terms);
    let contribution = i32::try_from((rate - 500) * 2 / 5).unwrap_or(0);
    signal(FeatureFamily::Seasonality, rate, contribution)
}

/// 교과목 신설·폐지·대체, read against the term being forecast.
fn lifecycle_status(lifecycle: &CourseLifecycle, target: TermKey) -> FeatureSignal {
    let (value, contribution) = match lifecycle {
        CourseLifecycle::Unknown => (0, 0),
        CourseLifecycle::Established => (1, 0),
        CourseLifecycle::NewFrom(first) if *first <= target => (2, 60),
        // The catalogue says the course begins after the term being forecast,
        // so it cannot run in it. That is an official fact about the course,
        // not a pattern in the history.
        CourseLifecycle::NewFrom(_) => (3, -500),
        CourseLifecycle::RetiredFrom(from) | CourseLifecycle::ReplacedFrom { from, .. }
            if *from > target =>
        {
            (4, -40)
        }
        CourseLifecycle::RetiredFrom(_) | CourseLifecycle::ReplacedFrom { .. } => (5, -500),
    };
    signal(FeatureFamily::LifecycleStatus, value, contribution)
}

/// 교수자 변화: distinct instructor sets among the offered terms.
fn instructor_change(seasonal: &[&crate::observation::TermObservation]) -> FeatureSignal {
    let mut sets: Vec<Vec<&str>> = Vec::new();
    for observation in seasonal {
        if observation.outcome() != Offered::Yes {
            continue;
        }
        let mut names: Vec<&str> = observation
            .instructors()
            .iter()
            .map(academic_curriculum::InstructorName::as_str)
            .collect();
        names.sort_unstable();
        if !sets.contains(&names) {
            sets.push(names);
        }
    }
    let distinct = i64::try_from(sets.len()).unwrap_or(i64::MAX);
    let contribution = match distinct {
        0 => 0,
        1 => 60,
        2 => -60,
        _ => -120,
    };
    signal(FeatureFamily::InstructorChange, distinct, contribution)
}

/// 최근 공지: official notices issued inside the window.
fn recent_notices(history: &CourseHistory, window: ObservationWindow) -> FeatureSignal {
    let mut value: i64 = 0;
    for notice in history.notices() {
        if !window.contains(notice.issued_in()) {
            continue;
        }
        value += match notice.effect() {
            NoticeEffect::OfferingAnnounced => 80,
            NoticeEffect::OfferingSuspended => -200,
            NoticeEffect::CurriculumChange => -60,
        };
    }
    let contribution = i32::try_from(value.clamp(-300, 300)).unwrap_or(0);
    signal(FeatureFamily::RecentNotices, value, contribution)
}

/// 미개설 gap: same-semester terms since the course last ran.
///
/// A course never observed as offered has a gap the width of the window, which
/// is the largest gap it can have -- but the forecast abstains on that history
/// before the score is read, so this arm never decides anything on its own.
fn offering_gap(seasonal: &[&crate::observation::TermObservation]) -> FeatureSignal {
    let mut gap: i64 = 0;
    for observation in seasonal.iter().rev() {
        if observation.outcome() == Offered::Yes {
            break;
        }
        gap += 1;
    }
    let contribution = match gap {
        0 => 60,
        1 => -60,
        2 => -160,
        _ => -260,
    };
    signal(FeatureFamily::OfferingGap, gap, contribution)
}

/// 불규칙 특강 여부: how many offered terms were one-off runs.
fn irregular_special(seasonal: &[&crate::observation::TermObservation]) -> FeatureSignal {
    let offered = seasonal
        .iter()
        .filter(|observation| observation.outcome() == Offered::Yes)
        .count();
    let irregular = seasonal
        .iter()
        .filter(|observation| observation.outcome() == Offered::Yes && observation.is_irregular())
        .count();
    let value = i64::try_from(irregular).unwrap_or(i64::MAX);
    let contribution = if irregular == 0 {
        0
    } else if irregular == offered {
        -200
    } else {
        -100
    };
    signal(FeatureFamily::IrregularSpecial, value, contribution)
}

/// 표본 window: how many same-semester terms were read.
fn history_window(seasonal_terms: u32) -> FeatureSignal {
    let value = i64::from(seasonal_terms);
    let contribution = match value {
        0 | 1 => -150,
        2 => -40,
        3 => 30,
        _ => 80,
    };
    signal(FeatureFamily::HistoryWindow, value, contribution)
}
