//! Typed scenario failures.
//!
//! Every fallible path in this crate returns one of these. `clippy::panic`,
//! `unwrap_used`, and `expect_used` are denied workspace-wide, and a what-if
//! engine that panicked on a malformed assumption would take the process down
//! over a value the user typed.

use academic_domain::DomainError;
use thiserror::Error;

/// A rejected scenario input or a refused projection payload.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ScenarioError {
    /// A weekly-hours range was reversed or longer than a week.
    #[error("weekly workload range must satisfy low <= high <= {maximum}, got {low}..={high}")]
    InvalidWorkloadRange { low: u16, high: u16, maximum: u16 },
    /// A scenario named no offering to evaluate.
    #[error("scenario must choose at least one offering")]
    EmptyScenario,
    /// A scenario named the same offering twice.
    #[error("scenario chose offering {0} more than once")]
    DuplicateOfferingChoice(String),
    /// A choice carried no syllabus signal, so it can project nothing.
    #[error("offering {0} carried no syllabus concept signal")]
    EmptyConceptSignals(String),
    /// A concept appeared twice within one offering's signals.
    #[error("offering {offering} repeated concept {concept}")]
    DuplicateConceptSignal { offering: String, concept: String },
    /// A bounded domain value the engine builds was rejected by the domain.
    #[error(transparent)]
    Domain(#[from] DomainError),
}
