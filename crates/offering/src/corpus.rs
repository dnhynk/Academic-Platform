//! The deterministic synthetic corpus every fixture in this crate is built
//! from.
//!
//! `CONTRIBUTING.md` rule 1 admits only synthetic fixtures and rule 5 admits a
//! golden fixture only through a deterministic builder. This module is that
//! builder. **Nothing here is a real academic record**: the course codes are
//! shaped like the catalogue's and name nothing in it, no term holds a real
//! reading, and no instructor is a person. The crate calls no connector, opens
//! no socket, and reads no clock; every instant below is a literal.
//!
//! # What the corpus is built to separate
//!
//! | case | what it exercises |
//! |---|---|
//! | `every_spring` | a reproducible seasonal pattern that is `HISTORICALLY_LIKELY` |
//! | `spring_only_asked_for_autumn` | the same history shape under its own code, read for the other semester -- which a majority vote over the last N terms could not tell apart |
//! | `sparse` | a window shorter than the recorded minimum |
//! | `irregular_only` | every observed run a one-off |
//! | `instructor_volatile` | a different instructor set on every run |
//! | `never_observed` | a course nobody has seen run |
//! | `gap_two` | the same seasonal rate as `every_other_spring` with the offerings at the far end of the window |
//! | `every_other_spring` | the same rate with the offerings at the near end |
//! | `retired` | an official retirement effective before the forecast term |
//! | `suspended_notice` | an official notice inside the window, which moves the standing |
//!
//! The pair `gap_two` / `every_other_spring` exists because a family that is
//! declared and unused is the failure this crate is written against: the two
//! agree on every other family and differ only in 미개설 gap, so a gap arm that
//! collapsed to a constant fails on them.

use academic_curriculum::{CourseCode, InstructorName};
use academic_domain::TimestampMillis;
use academic_model_run::{
    CalibrationBin, CalibrationDataset, CalibrationDatasetId, CalibrationRegistry, Digest32,
    ModelVersion, ProviderId, Purpose,
};
use academic_record::term::{Semester, TermKey};

use crate::{
    error::OfferingError,
    feature::ObservationWindow,
    forecast::{
        OFFERING_FORECAST_ENGINE_VERSION, OFFERING_FORECAST_PROVIDER, OFFERING_FORECAST_PURPOSE,
    },
    observation::{CourseHistory, CourseLifecycle, NoticeEffect, RecentNotice, TermObservation},
    policy::{ForecastPolicy, VerificationRecency},
};

/// The instant the corpus treats as "now".
///
/// A literal, not a clock reading: every fixture in this crate has to produce
/// the same bytes on every machine and in every year.
pub const CORPUS_NOW_MILLIS: i64 = 1_764_547_200_000;

/// One millisecond short of a year, as the corpus's recorded verification
/// bound.
pub const CORPUS_VERIFICATION_WITHIN_MILLIS: u64 = 86_400_000;

/// The corpus's recorded likely floor.
///
/// **Synthetic and user-confirmed.** No official source states this number;
/// `t001`'s `REQ-08-025` row records it as an open gate candidate. It is here
/// so a determinate case exists to check, and it is labelled rather than
/// presented as a default -- `ForecastPolicy` has none.
pub const CORPUS_LIKELY_FLOOR_PERMILLE: u16 = 600;

/// The corpus's recorded minimum window, in same-semester terms.
///
/// Synthetic and user-confirmed, for the same reason.
pub const CORPUS_MINIMUM_WINDOW_TERMS: u32 = 3;

/// The first term of every window the corpus builds.
///
/// # Errors
///
/// [`OfferingError`] when the term is refused, which the literal below is not.
pub fn window_start() -> Result<TermKey, OfferingError> {
    Ok(TermKey::new(2020, Semester::Spring)?)
}

/// The corpus's recorded criteria.
///
/// # Errors
///
/// [`OfferingError`] when the recorded numbers are out of range, which the
/// constants above are not.
pub fn policy() -> Result<ForecastPolicy, OfferingError> {
    ForecastPolicy::new(CORPUS_LIKELY_FLOOR_PERMILLE, CORPUS_MINIMUM_WINDOW_TERMS)
}

/// The corpus's recorded verification bound.
///
/// # Errors
///
/// [`OfferingError`] when the recorded bound is zero, which the constant above
/// is not.
pub fn verification_recency() -> Result<VerificationRecency, OfferingError> {
    VerificationRecency::new(CORPUS_VERIFICATION_WITHIN_MILLIS)
}

/// A calibration registry holding one fresh dataset for this forecaster.
///
/// The curve is monotone and covers the whole raw range, so an interpretation
/// failure in this crate's fixtures is an absent or stale dataset rather than
/// a score the curve does not reach. It is deliberately **not** the identity:
/// the raw scale is compressed at both ends, so a fixture that read the raw
/// number where it should read the calibrated one produces a different answer.
///
/// # Errors
///
/// [`OfferingError`] when `P2-M1` refuses the dataset, which the values below
/// do not cause.
pub fn calibration_registry(refreshed_at: u64) -> Result<CalibrationRegistry, OfferingError> {
    let mut registry = CalibrationRegistry::new();
    registry.register(calibration_dataset(refreshed_at)?)?;
    Ok(registry)
}

/// The one dataset [`calibration_registry`] holds.
///
/// # Errors
///
/// [`OfferingError`] when `P2-M1` refuses the dataset.
pub fn calibration_dataset(refreshed_at: u64) -> Result<CalibrationDataset, OfferingError> {
    let bins = vec![
        CalibrationBin::new(199, 20)?,
        CalibrationBin::new(399, 150)?,
        CalibrationBin::new(499, 330)?,
        CalibrationBin::new(599, 480)?,
        CalibrationBin::new(699, 620)?,
        CalibrationBin::new(799, 780)?,
        CalibrationBin::new(1000, 910)?,
    ];
    Ok(CalibrationDataset::new(
        CalibrationDatasetId::new("offering.forecast.synthetic.v1")?,
        ProviderId::new(OFFERING_FORECAST_PROVIDER)?,
        ModelVersion::new(OFFERING_FORECAST_ENGINE_VERSION.to_string())?,
        Purpose::new(OFFERING_FORECAST_PURPOSE)?,
        Digest32::of(b"offering.forecast.synthetic.v1"),
        512,
        refreshed_at,
        86_400_000,
        bins,
    )?)
}

/// Every named case in the corpus, in a fixed order.
pub const CASES: [&str; 10] = [
    "every_spring",
    "spring_only_asked_for_autumn",
    "sparse",
    "irregular_only",
    "instructor_volatile",
    "never_observed",
    "gap_two",
    "every_other_spring",
    "retired",
    "suspended_notice",
];

/// The window one case is forecast over.
///
/// # Errors
///
/// [`OfferingError`] when the case name is not one of [`CASES`], or when a
/// term or window is refused.
pub fn window(case: &str) -> Result<ObservationWindow, OfferingError> {
    let target = match case {
        "spring_only_asked_for_autumn" => TermKey::new(2026, Semester::Fall)?,
        _ => TermKey::new(2026, Semester::Spring)?,
    };
    ObservationWindow::new(window_start()?, target)
}

/// Builds one named case.
///
/// # Errors
///
/// [`OfferingError`] when the case name is not one of [`CASES`], or when a
/// term, course code or instructor name is refused.
pub fn history(case: &str) -> Result<CourseHistory, OfferingError> {
    match case {
        "every_spring" => every_spring("M9001.000100"),
        "spring_only_asked_for_autumn" => every_spring("M9001.001000"),
        "sparse" => sparse(),
        "irregular_only" => irregular_only(),
        "instructor_volatile" => instructor_volatile(),
        "never_observed" => never_observed(),
        "gap_two" => gap_two(),
        "every_other_spring" => every_other_spring(),
        "retired" => retired(),
        "suspended_notice" => suspended_notice(),
        other => Err(OfferingError::DuplicateObservation {
            course: other.to_owned(),
            term: "no such case".to_owned(),
        }),
    }
}

fn code(value: &str) -> Result<CourseCode, OfferingError> {
    Ok(CourseCode::parse(value)?)
}

fn teacher(value: &str) -> Result<InstructorName, OfferingError> {
    Ok(InstructorName::parse(value)?)
}

/// Readings are spaced one term apart on the instant axis so the disclosed
/// window is a real span. The spacing is a literal, not a term-to-date table:
/// it says when somebody read, not when a term ran.
fn read_at(index: i64) -> TimestampMillis {
    TimestampMillis::new(CORPUS_NOW_MILLIS - 15_552_000_000 * index)
}

/// Offered every spring from 2020 through 2025, one stable instructor, both
/// autumns read and empty.
fn every_spring(course: &str) -> Result<CourseHistory, OfferingError> {
    let mut history = CourseHistory::new(code(course)?);
    for (index, year) in [2025_u16, 2024, 2023, 2022, 2021, 2020]
        .into_iter()
        .enumerate()
    {
        let index = i64::try_from(index).unwrap_or(0);
        history.observe(TermObservation::offered(
            TermKey::new(year, Semester::Spring)?,
            read_at(index),
            vec![teacher("Instructor A")?],
            false,
        ))?;
        history.observe(TermObservation::not_offered(
            TermKey::new(year, Semester::Fall)?,
            read_at(index),
        ))?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// Two spring terms read, both offered: a window shorter than the recorded
/// minimum.
fn sparse() -> Result<CourseHistory, OfferingError> {
    let mut history = CourseHistory::new(code("M9001.000200")?);
    for (index, year) in [2025_u16, 2024].into_iter().enumerate() {
        let index = i64::try_from(index).unwrap_or(0);
        history.observe(TermObservation::offered(
            TermKey::new(year, Semester::Spring)?,
            read_at(index),
            vec![teacher("Instructor B")?],
            false,
        ))?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// Four spring terms read, three offered, every offered one a special run.
fn irregular_only() -> Result<CourseHistory, OfferingError> {
    let mut history = CourseHistory::new(code("M9001.000300")?);
    for (index, year) in [2025_u16, 2023, 2021].into_iter().enumerate() {
        let index = i64::try_from(index).unwrap_or(0);
        history.observe(TermObservation::offered(
            TermKey::new(year, Semester::Spring)?,
            read_at(index),
            vec![teacher("Instructor C")?],
            true,
        ))?;
    }
    history.observe(TermObservation::not_offered(
        TermKey::new(2024, Semester::Spring)?,
        read_at(3),
    ))?;
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// Four spring terms read, four offered, a different instructor set each time.
fn instructor_volatile() -> Result<CourseHistory, OfferingError> {
    let mut history = CourseHistory::new(code("M9001.000400")?);
    for (index, (year, name)) in [
        (2025_u16, "Instructor D"),
        (2024, "Instructor E"),
        (2023, "Instructor F"),
        (2022, "Instructor G"),
    ]
    .into_iter()
    .enumerate()
    {
        let index = i64::try_from(index).unwrap_or(0);
        history.observe(TermObservation::offered(
            TermKey::new(year, Semester::Spring)?,
            read_at(index),
            vec![teacher(name)?],
            false,
        ))?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// Four spring terms read, none offered. Section 8.3's
/// *한 번도 관찰하지 못한* case.
fn never_observed() -> Result<CourseHistory, OfferingError> {
    let mut history = CourseHistory::new(code("M9001.000500")?);
    for (index, year) in [2025_u16, 2024, 2023, 2022].into_iter().enumerate() {
        let index = i64::try_from(index).unwrap_or(0);
        history.observe(TermObservation::not_offered(
            TermKey::new(year, Semester::Spring)?,
            read_at(index),
        ))?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// Four spring terms read, two offered, and the two offered ones are the
/// **oldest**: the same seasonal rate as [`every_other_spring`] with a gap of
/// two at the near end.
fn gap_two() -> Result<CourseHistory, OfferingError> {
    let mut history = CourseHistory::new(code("M9001.000600")?);
    for (index, (year, offered)) in [(2025_u16, false), (2024, false), (2023, true), (2022, true)]
        .into_iter()
        .enumerate()
    {
        let index = i64::try_from(index).unwrap_or(0);
        let term = TermKey::new(year, Semester::Spring)?;
        history.observe(if offered {
            TermObservation::offered(term, read_at(index), vec![teacher("Instructor H")?], false)
        } else {
            TermObservation::not_offered(term, read_at(index))
        })?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// Four spring terms read, two offered, and the two offered ones are the
/// **newest**: the same seasonal rate as [`gap_two`] with a gap of zero.
fn every_other_spring() -> Result<CourseHistory, OfferingError> {
    every_other_spring_coded("M9001.000700")
}

fn every_other_spring_coded(course: &str) -> Result<CourseHistory, OfferingError> {
    let mut history = CourseHistory::new(code(course)?);
    for (index, (year, offered)) in [(2025_u16, true), (2024, true), (2023, false), (2022, false)]
        .into_iter()
        .enumerate()
    {
        let index = i64::try_from(index).unwrap_or(0);
        let term = TermKey::new(year, Semester::Spring)?;
        history.observe(if offered {
            TermObservation::offered(term, read_at(index), vec![teacher("Instructor H")?], false)
        } else {
            TermObservation::not_offered(term, read_at(index))
        })?;
    }
    history.set_lifecycle(CourseLifecycle::Established);
    Ok(history)
}

/// [`every_spring`]'s history under an official retirement effective before the
/// forecast term.
fn retired() -> Result<CourseHistory, OfferingError> {
    let mut history = every_spring("M9001.000800")?;
    history.set_lifecycle(CourseLifecycle::RetiredFrom(TermKey::new(
        2026,
        Semester::Spring,
    )?));
    Ok(history)
}

/// [`every_other_spring`]'s history with one official suspension notice inside
/// the window.
///
/// Built on the every-other case rather than the every-spring one on purpose:
/// the notice has to be able to move the standing, not only the score. Without
/// it the history reads `HISTORICALLY_LIKELY`; with it the calibrated
/// probability falls under the recorded floor and the standing is `UNCERTAIN`.
fn suspended_notice() -> Result<CourseHistory, OfferingError> {
    let mut history = every_other_spring_coded("M9001.000900")?;
    history.notice(RecentNotice::new(
        TermKey::new(2025, Semester::Fall)?,
        NoticeEffect::OfferingSuspended,
    ));
    Ok(history)
}

/// What actually happened to each case in the forecast term.
///
/// Synthetic, and recorded here so the independent oracle transcribes the same
/// outcomes rather than reading them back out of the engine. `None` is a course
/// nobody checked afterwards, which is the row
/// `TermForecastMetrics::missing_outcomes` reports and the reason coverage and
/// abstention are two different numbers.
#[must_use]
pub const fn realized(case: &str) -> Option<crate::metrics::RealizedOutcome> {
    use crate::metrics::RealizedOutcome::{NotOffered, Offered};
    match case.as_bytes() {
        b"every_spring" | b"sparse" | b"instructor_volatile" | b"every_other_spring" => {
            Some(Offered)
        }
        b"spring_only_asked_for_autumn"
        | b"irregular_only"
        | b"never_observed"
        | b"gap_two"
        | b"retired" => Some(NotOffered),
        // `suspended_notice` is the one nobody checked.
        _ => None,
    }
}
