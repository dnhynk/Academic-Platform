//! The identifiers this crate issues, and the one rule that admits them.
//!
//! ## Every byte is classified, and nothing is searched for
//!
//! [`validated`] admits `[A-Za-z0-9._-]` within [`MAX_IDENTIFIER`] bytes by
//! asking every byte of the value whether it is in the class. It does not look
//! for a listed character, because a name list admits every byte nobody thought
//! to list. `P2-Y1` measured the cost of the other shape — the same rule stated
//! as a comment beside a `validated` that never ran it — and `P2-Y2` measured
//! that twelve of the fifteen crates carrying this rule have no test that walks
//! the byte range at all, and that widening `P2-R4`'s class by one byte leaves
//! that crate's whole suite green. `the_identifier_rule_is_executed_over_every_byte`
//! walks all 256 single-byte values here in both directions, so this crate is
//! not the thirteenth.
//!
//! ## An evidence locator is not a criterion and not a competency
//!
//! Section 24.4 ends every navigation direction at a performance criterion
//! **and** an evidence locator, which are two different things a reader opens.
//! [`EvidenceLocatorId`] is this crate's, `CriterionId` is `P2-Y1`'s, and
//! `CompetencyId` is `P2-Y1`'s: there is no conversion between any two of them
//! in either direction, and none is built here.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ReadinessError;

/// Longest identifier this crate admits, in bytes.
pub const MAX_IDENTIFIER: usize = 64;

/// Checks one identifier against `[A-Za-z0-9._-]` within [`MAX_IDENTIFIER`].
///
/// Every byte is classified. Nothing here searches for a listed character, so a
/// byte nobody enumerated is refused because it was not admitted rather than
/// admitted because it was not listed.
pub(crate) fn validated(value: String, what: &'static str) -> Result<String, ReadinessError> {
    let legal = !value.is_empty()
        && value.len() <= MAX_IDENTIFIER
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if legal {
        Ok(value)
    } else {
        Err(ReadinessError::InvalidIdentifier(what, value))
    }
}

/// Checks one piece of prose for emptiness.
///
/// A rubric line, a source citation and a weight's reason are the user's own
/// words, so they are not identifiers and are not reshaped. What is refused is
/// the empty one, because a disclosure nobody wrote discloses nothing and
/// section 24.3 makes the disclosure the condition of the score existing.
pub(crate) fn non_empty(value: String, what: &'static str) -> Result<String, ReadinessError> {
    if value.trim().is_empty() {
        Err(ReadinessError::EmptyText(what))
    } else {
        Ok(value)
    }
}

/// Where a reader opens one piece of evidence.
///
/// Section 24.4's `실제 개인 evidence`. Not a description of the evidence and
/// not a claim about it: the handle somebody follows to the artifact, lecture
/// segment, commit or incident the cell was settled by.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EvidenceLocatorId(String);

impl EvidenceLocatorId {
    /// Checks one locator identifier.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::InvalidIdentifier`] when it is empty, longer than
    /// [`MAX_IDENTIFIER`] bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, ReadinessError> {
        validated(value.into(), "evidence locator").map(Self)
    }

    /// Its text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EvidenceLocatorId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<String> for EvidenceLocatorId {
    type Error = ReadinessError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<EvidenceLocatorId> for String {
    fn from(value: EvidenceLocatorId) -> Self {
        value.0
    }
}

/// One node a navigation direction may start from.
///
/// Section 24.4 names four starting points — a concept, a goal or role, a
/// project and a course — and this is the identifier of whichever one a walk
/// began at. It is deliberately **not** any of the four identity types those
/// crates issue: a walk is a query, and a query that could only be phrased in
/// one crate's identity would make three of the four directions unreachable
/// from a caller holding the other three.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StartingPointId(String);

impl StartingPointId {
    /// Checks one starting-point identifier.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::InvalidIdentifier`] when it is empty, longer than
    /// [`MAX_IDENTIFIER`] bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, ReadinessError> {
        validated(value.into(), "starting point").map(Self)
    }

    /// Its text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StartingPointId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<String> for StartingPointId {
    type Error = ReadinessError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<StartingPointId> for String {
    fn from(value: StartingPointId) -> Self {
        value.0
    }
}
