//! The one free-text wrapper this crate has, and the identifier shape it uses.
//!
//! Every human sentence in this crate is a [`NonEmptyText`], so a field that a
//! person could leave blank is a value that does not exist rather than a check
//! somebody remembers to run. That is `P2-N5`'s discipline in
//! `crates/gap/src/explanation.rs` applied to section 20's own free-text
//! fields, and it is what lets `crates/build-learn`'s plan validator read
//! **structure** rather than words: see [`crate::validate`].

use serde::{Deserialize, Serialize};

use crate::BuildLearnError;

/// Text that is not blank.
///
/// Private field, one constructor, no `Default`. Whitespace-only text is
/// refused, not trimmed into emptiness later.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NonEmptyText(String);

impl NonEmptyText {
    /// Takes one non-blank sentence.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::EmptyText`] when it carries nothing but whitespace.
    pub fn new(value: impl Into<String>) -> Result<Self, BuildLearnError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(BuildLearnError::EmptyText);
        }
        Ok(Self(value))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NonEmptyText {
    type Error = BuildLearnError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<NonEmptyText> for String {
    fn from(value: NonEmptyText) -> Self {
        value.0
    }
}

/// An identifier this crate mints for one of its own parts.
///
/// `[A-Za-z0-9._-]` within 64 bytes, the shape `P2-R4`'s `SubjectId` uses, so a
/// criterion, a decision, an alternative or a responsibility can be named in a
/// serialized document and joined back without an escaping rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PartId(String);

impl PartId {
    /// Takes one identifier.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::InvalidIdentifier`] when it is empty, longer than 64
    /// bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, BuildLearnError> {
        let value = value.into();
        let shaped = !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        if !shaped {
            return Err(BuildLearnError::InvalidIdentifier(value));
        }
        Ok(Self(value))
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PartId {
    type Error = BuildLearnError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<PartId> for String {
    fn from(value: PartId) -> Self {
        value.0
    }
}
