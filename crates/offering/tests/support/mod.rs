//! Shared fixtures for the `P2-U5` acceptance suite.
//!
//! Everything the suite needs that is not `academic_offering::corpus`'s: the
//! identifiers, the official readings, and the plan scenarios. The histories,
//! the recorded criteria and the calibration dataset are the product corpus's,
//! reused rather than transcribed, so a change there moves every fixture here
//! and the two cannot drift into disagreeing about one course.

#![allow(dead_code)]

use std::error::Error;

use academic_curriculum::{Capacity, CourseCode, InstructorName, Meeting, Weekday};
use academic_domain::{
    ClaimId, EntityId, EvidenceId, ScopeId, TimestampMillis, ValidInterval, engines::ProofStatus,
};
use academic_ingestion::{ConnectorId, SourceCategory};
use academic_model_run::CalibrationRegistry;
use academic_offering::{
    CancellationNotice, ClaimSubject, ConfirmationEvidence, CourseHistory, ForecastPolicy,
    ObservationWindow, OfferingError, OfficialListing, OfficialTermReading, Resolution, corpus,
    resolve,
};
use academic_record::{
    plan::{PlanScenario, PlanScenarioChoice},
    term::{Semester, TermKey},
};

pub type TestResult = Result<(), Box<dyn Error>>;

/// A version-seven, RFC-variant identifier derived from a suffix.
fn identifier(suffix: u32) -> String {
    format!("01900000-0000-7000-8000-0000{suffix:08x}")
}

pub fn entity(suffix: u32) -> Result<EntityId, Box<dyn Error>> {
    Ok(identifier(suffix).parse::<EntityId>()?)
}

pub fn scope(suffix: u32) -> Result<ScopeId, Box<dyn Error>> {
    Ok(identifier(suffix).parse::<ScopeId>()?)
}

pub fn claim(suffix: u32) -> Result<ClaimId, Box<dyn Error>> {
    Ok(identifier(suffix).parse::<ClaimId>()?)
}

pub fn evidence(suffix: u32) -> Result<EvidenceId, Box<dyn Error>> {
    Ok(identifier(suffix).parse::<EvidenceId>()?)
}

/// The instant every fixture treats as now.
#[must_use]
pub fn now() -> TimestampMillis {
    TimestampMillis::new(corpus::CORPUS_NOW_MILLIS)
}

/// The corpus's calibration registry, refreshed so it is fresh at [`now`].
pub fn registry() -> Result<CalibrationRegistry, OfferingError> {
    corpus::calibration_registry(fresh_refresh())
}

/// A refresh instant that leaves the dataset fresh at [`now`].
#[must_use]
pub const fn fresh_refresh() -> u64 {
    // The dataset's interval is one day; refreshing one hour ago leaves it
    // fresh and refreshing two days ago does not.
    (corpus::CORPUS_NOW_MILLIS - 3_600_000) as u64
}

/// A refresh instant that leaves the dataset stale at [`now`].
#[must_use]
pub const fn stale_refresh() -> u64 {
    (corpus::CORPUS_NOW_MILLIS - 172_800_000) as u64
}

/// The corpus's recorded criteria.
pub fn policy() -> Result<ForecastPolicy, OfferingError> {
    corpus::policy()
}

/// Resolves one corpus case with no official reading.
pub fn resolve_case(case: &str) -> Result<Resolution, Box<dyn Error>> {
    let history = corpus::history(case)?;
    let window = corpus::window(case)?;
    Ok(resolve(
        &history,
        window,
        None,
        Some(policy()?),
        &registry()?,
        now(),
    )?)
}

/// Resolves one corpus case with an official reading beside it.
pub fn resolve_case_with(
    case: &str,
    official: &OfficialTermReading,
) -> Result<Resolution, Box<dyn Error>> {
    let history = corpus::history(case)?;
    let window = corpus::window(case)?;
    Ok(resolve(
        &history,
        window,
        Some(official),
        Some(policy()?),
        &registry()?,
        now(),
    )?)
}

/// One corpus case's history and window.
pub fn case(case: &str) -> Result<(CourseHistory, ObservationWindow), Box<dyn Error>> {
    Ok((corpus::history(case)?, corpus::window(case)?))
}

pub fn connector(name: &str) -> Result<ConnectorId, Box<dyn Error>> {
    Ok(ConnectorId::new(name)?)
}

pub fn course_code(value: &str) -> Result<CourseCode, Box<dyn Error>> {
    Ok(CourseCode::parse(value)?)
}

pub fn spring_2026() -> Result<TermKey, Box<dyn Error>> {
    Ok(TermKey::new(2026, Semester::Spring)?)
}

/// A registration-system reading of one course in 2026 spring, listing a
/// section with a real timetable and a real seat count.
pub fn registration_listing(
    course: &str,
    retrieved_at: TimestampMillis,
) -> Result<OfficialListing, Box<dyn Error>> {
    Ok(OfficialListing::new(
        SourceCategory::RegistrationSystem,
        connector("sugang.snu.ac.kr")?,
        retrieved_at,
        spring_2026()?,
        course_code(course)?,
        true,
    )
    .instructor(InstructorName::parse("Instructor A")?)
    .capacity(Capacity::new(60))
    .meeting(Meeting::new(Weekday::Tuesday, 570, 645)?))
}

/// A department-page reading of the same course and term.
pub fn department_listing(
    course: &str,
    retrieved_at: TimestampMillis,
    lists_a_section: bool,
) -> Result<OfficialListing, Box<dyn Error>> {
    Ok(OfficialListing::new(
        SourceCategory::DepartmentPage,
        connector("cse.snu.ac.kr")?,
        retrieved_at,
        spring_2026()?,
        course_code(course)?,
        lists_a_section,
    ))
}

/// A fresh confirmation of one course, with the cross sources supplied.
pub fn confirmation(
    course: &str,
    cross_sources: Vec<OfficialListing>,
) -> Result<ConfirmationEvidence, Box<dyn Error>> {
    let retrieved_at = TimestampMillis::new(corpus::CORPUS_NOW_MILLIS - 3_600_000);
    Ok(ConfirmationEvidence::from_registration_system(
        registration_listing(course, retrieved_at)?,
        cross_sources,
        corpus::verification_recency()?,
        now(),
    )?)
}

/// An official cancellation notice for one course in 2026 spring.
pub fn cancellation(course: &str) -> Result<CancellationNotice, Box<dyn Error>> {
    Ok(CancellationNotice::official(
        SourceCategory::RegistrationSystem,
        connector("sugang.snu.ac.kr")?,
        TimestampMillis::new(corpus::CORPUS_NOW_MILLIS - 1_800_000),
        spring_2026()?,
        course_code(course)?,
    )?)
}

/// A plan scenario naming one course for 2026 spring.
pub fn scenario_for(course: &str) -> Result<PlanScenario, Box<dyn Error>> {
    Ok(PlanScenario::new(
        entity(4001)?,
        "plan A",
        vec![PlanScenarioChoice::new(course, spring_2026()?)?],
    )?)
}

/// The claim subject every claim fixture uses.
pub fn subject() -> Result<ClaimSubject, Box<dyn Error>> {
    Ok(ClaimSubject {
        subject_entity_id: entity(5001)?,
        scope_id: scope(5002)?,
        valid_time: ValidInterval::new(
            TimestampMillis::new(corpus::CORPUS_NOW_MILLIS),
            Some(TimestampMillis::new(corpus::CORPUS_NOW_MILLIS + 1_000)),
        )?,
    })
}

/// One node of a forecast's proof tree, by rule suffix.
pub fn node_status(resolution: &Resolution, suffix: &str) -> Option<ProofStatus> {
    let forecast = resolution.forecast()?;
    forecast
        .outcome()
        .proof_tree
        .walk()
        .into_iter()
        .find(|node| node.rule_id.as_str().ends_with(suffix))
        .map(|node| node.status)
}
