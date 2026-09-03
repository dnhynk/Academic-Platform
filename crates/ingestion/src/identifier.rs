//! The one restricted identifier this crate mints, and why the charset is narrow.
//!
//! Every identifier here is read out of, or names, a document that came from
//! outside this system. `academic_untrusted_content::SourceId` settles the
//! shape for that case: `[A-Za-z0-9._-]`, at most 64 bytes, so an identifier
//! cannot itself carry a directive into a rendered prompt's structural fields
//! or a newline into a canonical encoding. This module is that rule applied to
//! the four names this crate needs, rather than four copies of it.

use core::fmt;

/// Why a name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// The name was empty or longer than [`MAX_NAME_BYTES`].
    #[error("a name is 1..={MAX_NAME_BYTES} bytes")]
    Length,
    /// The name held a byte outside `[A-Za-z0-9._-]`.
    #[error("a name holds only ASCII letters, digits, '.', '_' and '-'")]
    Charset,
}

/// The longest name this crate accepts, matching `SourceId`'s bound.
pub const MAX_NAME_BYTES: usize = 64;

/// Whether `value` is a name this crate will hold.
fn is_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NAME_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

macro_rules! restricted_name {
    ($name:ident, $what:literal) => {
        #[doc = concat!("A ", $what, ", restricted to `[A-Za-z0-9._-]`.")]
        ///
        /// The payload is a named field rather than a tuple position, for the
        /// reason `academic_untrusted_content::SourceId` states: a tuple
        /// position is judged by its type alone in
        /// `tools/secret-debug-policy.test.mjs`, and a `String` newtype would
        /// be classified there as carrying plaintext.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name {
            name: String,
        }

        impl $name {
            #[doc = concat!("Validates and takes a ", $what, ".")]
            ///
            /// # Errors
            ///
            /// [`NameError`] when the value is empty, over
            /// [`MAX_NAME_BYTES`], or holds a byte outside `[A-Za-z0-9._-]`.
            pub fn new(value: impl Into<String>) -> Result<Self, NameError> {
                let value = value.into();
                if value.is_empty() || value.len() > MAX_NAME_BYTES {
                    return Err(NameError::Length);
                }
                if !is_name(&value) {
                    return Err(NameError::Charset);
                }
                Ok(Self { name: value })
            }

            /// The name. This crate's own metadata, not document content.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.name
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.name)
            }
        }
    };
}

restricted_name!(ConnectorId, "connector identifier");
restricted_name!(ProgramKey, "programme key");
restricted_name!(SectionPath, "document section path");
restricted_name!(DependentId, "dependent-node identifier");
