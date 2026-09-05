//! `P2-U4` — the section 10 attempt model, GPA, credit accounting, and repeat policy.
//!
//! This crate turns a user-confirmed transcript into an append-only record of
//! every attempt, and computes the averages and credit totals section 10 asks
//! a calculation to keep apart. It builds on `P2-U7`: the confirmed row is
//! `academic-transcript`'s, and this crate re-derives none of it.
//!
//! ## What is fixed here
//!
//! - **Every attempt is preserved.** [`attempt::AttemptHistory`] has one
//!   mutator and no removal path. A repeat, a cancellation, and a correction
//!   are all new entries; the earlier attempt stays readable by identity
//!   forever. A correction carries ADR-003's `SUPERSEDES` relation, the same
//!   mechanism `CONTRIBUTING.md` rule 2 requires of every canonical assertion.
//! - **Classification is a versioned rule-engine output.**
//!   [`classify::RequirementClassification`] has no public constructor;
//!   `ClassificationRuleSet::classify` is the only thing that mints one, and
//!   the claim it travels as carries `AuthorityClass::DeterministicEngine`,
//!   which ADR-003's actor matrix refuses to a user actor.
//! - **The two dated policies are rows, not constants.** The 2015-spring repeat
//!   ceiling and the post-2004 external-grade exclusion live in
//!   [`policy::PolicyBook`], selected by the attempt's own term.
//! - **All arithmetic is exact.** [`decimal`] operates on
//!   `academic_domain::Decimal` and introduces no second numeric type. No
//!   `f32` or `f64` appears anywhere in this crate.
//!
//! ## What is deliberately not decided
//!
//! Section 38 leaves `GATE-38-005` (the official transcript), `GATE-38-006`
//! (transferred and exchange credit recognition), and `GATE-38-016` (external
//! recognition rules) open, and this crate keeps them open. Section 10 states
//! the repeat *ceiling* and says the eligibility rule, the 경과조치, and the
//! 동일·대체 mapping are a separate versioned policy whose current original must
//! be confirmed; it does not say which attempt of a repeat group is the
//! recognized one. So the shipped [`policy::PolicyBook::published_v1`] carries
//! [`policy::RepeatRecognition::Unknown`], external credits carry
//! [`policy::RecognitionDecision::Undecided`], and an engine that meets either
//! reports `UNKNOWN` naming the exact attempts rather than choosing a default.

#![forbid(unsafe_code)]

use academic_domain::{AttemptId, DomainError, EntityId, engines::EngineError};
use academic_transcript::TranscriptError;

pub mod attempt;
pub mod classify;
pub mod corpus;
pub mod decimal;
pub mod engine;
pub mod facts;
pub mod grade;
pub mod harness;
pub mod ingest;
pub mod plan;
pub mod policy;
pub mod term;
pub mod views;

/// The version both engines in this crate report.
pub const ENGINE_VERSION_TEXT: &str = "1";

/// Every way this crate refuses.
///
/// Typed, total, and never a panic: a malformed attempt, an unrepresentable
/// quotient, and a rule-set hash that does not belong to the engine are all
/// values a caller can match on.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RecordError {
    /// An exact operation would not fit in the canonical coefficient.
    #[error("decimal arithmetic overflowed the canonical coefficient")]
    DecimalOverflow,
    /// A scale past the canonical maximum of eighteen.
    #[error("decimal scale {0} is past the canonical maximum")]
    DecimalScaleTooLarge(u8),
    /// A narrowing rescale would have dropped a non-zero digit.
    #[error("value is not exactly representable at scale {scale}")]
    DecimalNotExactlyRepresentable {
        /// The scale that could not hold the value.
        scale: u8,
    },
    /// A grade-point average over an empty denominator.
    #[error("cannot divide by zero")]
    DivisionByZero,
    /// An academic year outside the four-digit range.
    #[error("term year {0} is outside the representable range")]
    TermYearOutOfRange(u16),
    /// A term spelling the canonical parser does not accept.
    #[error("malformed term: {0}")]
    MalformedTerm(String),
    /// A transcript term spelling no confirmed source maps to a session.
    #[error("no confirmed source says which session `{0}` is; supply the canonical spelling")]
    UnconfirmedTermSpelling(String),
    /// A grading scheme left a symbol unmapped.
    #[error("grading scheme does not map {}", .0.as_str())]
    GradingSchemeIncomplete(grade::GradeSymbol),
    /// Two policy rows share an effective term.
    #[error("two policy rows are effective from {0}")]
    DuplicatePolicyEffectiveTerm(String),
    /// A required field was empty.
    #[error("{0} must not be empty")]
    EmptyField(&'static str),
    /// A course code that is not identifier-shaped.
    ///
    /// The code travels through a deterministic engine's frozen inputs as a
    /// `ref:` value, whose grammar is ASCII alphanumerics, `.`, `_`, and `-`.
    /// Refusing at the boundary keeps one spelling rule rather than two, and a
    /// refusal names the field instead of silently re-spelling a course.
    #[error("course code is not identifier-shaped: {0}")]
    MalformedCourseCode(String),
    /// A registration confirmation with no evidence behind it.
    #[error("a registration confirmation needs at least one evidence identifier")]
    RegistrationWithoutEvidence,
    /// An attempt with no evidence behind it.
    #[error("an attempt needs at least one evidence identifier")]
    AttemptWithoutEvidence,
    /// A negative credit quantity.
    #[error("credits must not be negative")]
    NegativeCredits,
    /// A confirmation was minted for a different line of the document.
    #[error("row ordinal {row} was handed a confirmation for ordinal {confirmation}")]
    ConfirmationOrdinalMismatch {
        /// The row's document-order position.
        row: u32,
        /// The confirmation's.
        confirmation: u32,
    },
    /// A confirmation whose four fields are not this row's.
    #[error("the confirmation's fields are not this row's")]
    ConfirmationIsForAnotherRow,
    /// An attempt built from something that is not a user-confirmed row.
    #[error("an attempt needs a user-confirmed row, not an import row")]
    AttemptNeedsAConfirmedRow,
    /// A grade symbol outside the closed set.
    #[error("unknown grade symbol: {0}")]
    UnknownGradeSymbol(String),
    /// A correction naming an attempt the ledger does not hold.
    #[error("correction supersedes unknown attempt {0}")]
    SupersedesUnknownAttempt(AttemptId),
    /// A correction naming itself.
    #[error("attempt {0} cannot supersede itself")]
    AttemptSupersedesItself(AttemptId),
    /// Two entries with one identity.
    #[error("attempt {0} is already in the ledger")]
    DuplicateAttemptId(AttemptId),
    /// A programme identity that is not identifier-shaped.
    #[error("malformed programme id: {0}")]
    MalformedProgramId(String),
    /// An identifier the rule book's canonical text could not render.
    ///
    /// The rendering separates fields with a space, an `=` and a newline, so a
    /// value holding one of them would let two different books render alike.
    #[error("identifier is not canonical-text shaped: {0}")]
    MalformedCanonicalIdentifier(String),
    /// Two classification rules for one programme and course.
    #[error("programme {program} classifies {course_code} twice")]
    DuplicateClassificationRule {
        /// The programme.
        program: String,
        /// The course code.
        course_code: String,
    },
    /// A repeat group with no attempts, which grouping cannot produce.
    #[error("a repeat group has no attempts")]
    EmptyRepeatGroup,
    /// More attempts than the frozen-input encoding can count.
    #[error("the attempt set is larger than the frozen-input encoding can carry")]
    TooManyAttempts,
    /// A frozen input the engine requires was absent.
    #[error("missing frozen engine input: {0}")]
    MissingEngineInput(&'static str),
    /// A frozen input the engine could not read.
    #[error("malformed frozen engine input: {0}")]
    MalformedEngineInput(&'static str),
    /// An evaluation presented a hash that is not the engine's rule book's.
    #[error("the presented rule-set hash is not this engine's rule book")]
    RuleSetHashMismatch,
    /// An attempt in the frozen inputs has no computed disposition.
    #[error("an attempt in the frozen inputs has no disposition")]
    DispositionMissing,
    /// A duplicate plan scenario identity.
    #[error("plan scenario {0} is already stored")]
    DuplicateScenarioId(EntityId),
    /// A plan scenario the store does not hold.
    #[error("plan scenario {0} is not stored")]
    UnknownScenarioId(EntityId),
    /// A canonical domain value was invalid.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A transcript value was invalid.
    #[error(transparent)]
    Transcript(#[from] TranscriptError),
    /// The deterministic engine harness refused a value.
    #[error(transparent)]
    Engine(#[from] EngineError),
}

impl RecordError {
    /// Narrows to the harness error type the `DeterministicEngine` trait returns.
    ///
    /// The trait's signature is the harness contract's and cannot carry this
    /// crate's error, so a refusal that is not already an `EngineError` becomes
    /// a `MalformedInput` naming the class. Callers that need the exact reason
    /// use `evaluate_record`, which returns the typed error.
    #[must_use]
    pub fn into_engine_error(self) -> EngineError {
        match self {
            Self::Engine(error) => error,
            Self::Domain(error) => EngineError::Domain(error),
            Self::RuleSetHashMismatch => {
                EngineError::MalformedInput("rule-set hash is not this engine's rule book")
            }
            Self::MissingEngineInput(_) => {
                EngineError::MalformedInput("a required input is absent")
            }
            _ => EngineError::MalformedInput("the frozen inputs are not a valid attempt set"),
        }
    }
}

/// An identifier the rule book's canonical text can render unambiguously.
///
/// [`policy::RuleBook::canonical_text`] separates its fields with a space, an
/// `=` and a newline, and its SHA-256 is the `rule_set_hash` every replay is
/// keyed by. A field rendered into it that could hold one of those three
/// characters makes the encoding ambiguous, and ambiguous is not a synonym for
/// unlikely: a `row_id` holding a newline and the rendering of the row above it
/// makes two rule books with different row sets render the same bytes, so a
/// recorded average replays under a book that is not its own and is accepted.
///
/// The four positions the rule book renders therefore carry this type rather
/// than a `String`. There is no public field and no constructor that skips
/// [`CanonicalIdentifier::new`], so the charset is a property of the value
/// rather than a check somebody has to remember at each site.
///
/// The charset is [`check_identifier`]'s, which admits none of the three.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalIdentifier(String);

impl CanonicalIdentifier {
    /// Builds an identifier, refusing a spelling the canonical text could not
    /// carry without ambiguity.
    pub fn new(value: impl Into<String>) -> Result<Self, RecordError> {
        let value = value.into();
        if check_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(RecordError::MalformedCanonicalIdentifier(value))
        }
    }

    /// Returns the identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CanonicalIdentifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Refuses a value the frozen-input grammar could not carry.
///
/// One rule, used by every identifier-shaped field in this crate, so a course
/// code and a programme id cannot drift into two spellings.
pub(crate) fn check_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
