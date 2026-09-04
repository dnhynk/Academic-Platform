//! Every refusal this crate makes, typed.
//!
//! No boundary here panics (§2.3-11). A malformed identifier, an attempt the
//! course index does not cover, a rule the source index does not place, and a
//! grade-point denominator that is not a whole number of credits are each a
//! value a caller can match on, and each is the reason an audit did not run
//! rather than a verdict about a degree.

use academic_domain::{AttemptId, DomainError, engines::EngineError};
use academic_record::RecordError;
use academic_requirement::RequirementError;

/// What the graduation audit refuses, and why.
/// `RecordError` is neither `Clone` nor `Eq`, so neither is this. A test that
/// wants to say which refusal it got matches on the variant; comparing two
/// error values for equality is a thing no caller here needs and a derive that
/// forced the record crate to grow two traits for it would be this crate
/// reaching across a boundary for a convenience.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AuditError {
    /// An identifier carried a character the canonical encoding separates on.
    #[error("{kind} identifier is not admitted: {value}")]
    InvalidIdentifier {
        /// What kind of identifier was being built.
        kind: &'static str,
        /// The rejected text.
        value: String,
    },

    /// The transcript holds an attempt the course-facts index does not cover.
    ///
    /// Fail-closed: an attempt whose course the curriculum has not placed
    /// cannot be counted under any category, and counting it under none would
    /// be an audit over a transcript the engine only partly read.
    #[error("no course requirement facts are recorded for attempt {attempt}")]
    CourseFactsAbsent {
        /// The attempt with no entry.
        attempt: AttemptId,
    },

    /// The transcript holds an attempt whose course code is not identifier-shaped.
    #[error("attempt {attempt} carries a course code the frozen encoding cannot spell")]
    CourseCodeNotEncodable {
        /// The attempt.
        attempt: AttemptId,
    },

    /// Two attempts in one snapshot claim the same identity.
    #[error("attempt {attempt} appears twice in one transcript snapshot")]
    DuplicateAttempt {
        /// The repeated identity.
        attempt: AttemptId,
    },

    /// A grade-point denominator was not a whole number of credits.
    ///
    /// The rule comparison is over integers, so a fractional denominator is a
    /// refusal rather than a rounded one.
    #[error("the grade-point denominator {value} is not a whole number of credits")]
    FractionalDenominator {
        /// The rendered denominator.
        value: String,
    },

    /// The engine was handed a rule-set hash that is not the bound set's.
    #[error("the presented rule-set hash is not this audit's rule set")]
    RuleSetHashMismatch,

    /// A gate refused and the audit collected no outstanding check.
    ///
    /// This is a defect in `DegreeAudit::assemble`, not a state of the record,
    /// and it is returned as one. The arm exists because
    /// `IndeterminateVerdict::from_checks` answers `Option` and a `match` has
    /// to be total; what used to stand here was
    /// `MissingCheck::SourceFreshnessPolicyAbsent`, which told a user who had
    /// recorded the freshness criterion to record it. An audit that cannot say
    /// what it is waiting for is not a verdict, and inventing the nearest check
    /// to fill the hole is exactly the vague *정보 부족* section 11.1 forbids --
    /// with a false reason attached.
    ///
    /// It is unreachable on a published rule set: `RuleSetDraft::publish`
    /// refuses a set with no rule, every rule with no recorded span pushes
    /// `RuleSourceSpanAbsent`, every `UNKNOWN` and `CONFLICT` leaf pushes its
    /// own check, and both freshness arms push theirs -- so a refusing gate
    /// always leaves at least one check behind. `a_gate_that_refuses_always_
    /// names_a_check` walks the four refusals and observes that.
    #[error("a gate refused and the audit collected no outstanding check")]
    RefusedWithNoCheck,

    /// A frozen input the decoder requires is absent.
    #[error("the frozen inputs declare no {0}")]
    MissingEngineInput(&'static str),

    /// A frozen input carried a shape the decoder does not admit.
    #[error("a frozen input is malformed: {0}")]
    MalformedEngineInput(&'static str),

    /// A published rule concluded nothing because the rule itself is malformed.
    #[error(transparent)]
    Requirement(#[from] RequirementError),

    /// The record layer refused.
    #[error(transparent)]
    Record(#[from] RecordError),

    /// The proof tree or the frozen inputs were refused by the harness.
    #[error(transparent)]
    Engine(#[from] EngineError),

    /// A span, an interval or an identifier the domain owns was refused.
    #[error(transparent)]
    Domain(#[from] DomainError),
}
