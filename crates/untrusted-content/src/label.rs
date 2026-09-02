//! The trust label, and why it is a type rather than a field.
//!
//! A label kept as a boolean, a convention, or a review checklist survives
//! exactly as long as the next person remembers it. [`Untrusted`] is a wrapper
//! the compiler propagates instead: a value that came from outside this system
//! stays inside it, and the operation that would take it out is the one
//! enumerated below.
//!
//! # What is deliberately absent
//!
//! `Untrusted<T>` implements no `Deref`, no `DerefMut`, no `AsRef`, no `Borrow`,
//! no `Display`, no `ToString`, no `From<Untrusted<T>> for T`, and no
//! `Into<String>`. Those are the shapes that make a wrapper decorative: each
//! turns the wrapper into a plain value at a call site that reads like nothing
//! happened. `untrusted_has_no_unwrapping_trait_impl` in `tests/trust_scans.rs`
//! compares the crate's whole set of `impl` blocks whose self type is
//! `Untrusted<..>` against a pinned list, so a new one fails whatever it is
//! named. An `impl` in another crate is refused by the orphan rule instead:
//! both the trait and the type would be foreign there.
//!
//! ```compile_fail
//! # use academic_untrusted_content::{IngestedDocument, Untrusted};
//! fn strip(document: &Untrusted<IngestedDocument>) -> &IngestedDocument {
//!     document
//! }
//! ```
//!
//! ```compile_fail
//! # use academic_untrusted_content::{IngestedDocument, Untrusted};
//! fn strip(document: Untrusted<IngestedDocument>) -> String {
//!     document.into()
//! }
//! ```
//!
//! ```compile_fail
//! # use academic_untrusted_content::{IngestedDocument, Untrusted};
//! fn strip(document: &Untrusted<IngestedDocument>) -> String {
//!     format!("{document}")
//! }
//! ```
//!
//! # The one accessor, and why it is not public
//!
//! [`Untrusted::expose`] is `pub(crate)`. Outside this crate no function
//! returns the wrapped value, so no caller can spell one. Inside it, every call
//! site is named with a written reason in
//! `every_exposure_site_is_named_and_justified`, and the whole inventory is
//! compared rather than iterated, so a site anywhere else fails as an extra key.
//!
//! # `Debug`
//!
//! The implementation is hand-written and prints the provenance, the digest and
//! the byte count. It is written for every `T`, with no `T: Debug` bound, so
//! there is no instantiation whose payload a format string can reach.

use core::{fmt, marker::PhantomData};

use sha2::{Digest, Sha256};

/// Lowercase hexadecimal of `bytes`.
pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

/// Lowercase SHA-256 of `bytes`, hex encoded.
pub(crate) fn digest_of(bytes: &[u8]) -> String {
    hex_lower(&Sha256::digest(bytes))
}

/// Where a byte came from.
///
/// The six variants are the execution plan's own list for `P2-G5`: syllabus,
/// README, issue, code comment, review text, provider response. A seventh kind
/// of source is a change to this enum and to every `match` over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceKind {
    /// An official or unofficial course syllabus.
    Syllabus,
    /// A repository README or other project prose.
    Readme,
    /// An issue, pull request, or discussion body.
    Issue,
    /// A comment lifted out of source code.
    CodeComment,
    /// Free-text course or instructor review.
    ReviewText,
    /// Bytes a provider sent back.
    ProviderResponse,
}

impl SourceKind {
    /// Exhaustive order, used by the corpus completeness rule.
    pub const ALL: [Self; 6] = [
        Self::Syllabus,
        Self::Readme,
        Self::Issue,
        Self::CodeComment,
        Self::ReviewText,
        Self::ProviderResponse,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syllabus => "SYLLABUS",
            Self::Readme => "README",
            Self::Issue => "ISSUE",
            Self::CodeComment => "CODE_COMMENT",
            Self::ReviewText => "REVIEW_TEXT",
            Self::ProviderResponse => "PROVIDER_RESPONSE",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// A caller-chosen identifier for one ingested document.
///
/// The charset is restricted so an identifier cannot itself carry a directive
/// into the rendered prompt's structural fields.
///
/// The payload is a named field rather than a tuple position on purpose.
/// `tools/secret-debug-policy.test.mjs` judges a tuple position by its type
/// alone -- it has no name to judge by, which is what its own comment says --
/// so a `String` newtype is classified as carrying plaintext, and everything
/// holding one inherits that. An identifier the caller chose, restricted to
/// `[A-Za-z0-9._-]`, is not plaintext, and the field name is the signal that
/// net says a tuple position cannot give it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId {
    identifier: String,
}

/// Why an identifier was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SourceIdError {
    /// The identifier was empty or longer than 64 bytes.
    #[error("a source identifier is 1..=64 bytes")]
    Length,
    /// The identifier held a byte outside `[A-Za-z0-9._-]`.
    #[error("a source identifier holds only ASCII letters, digits, '.', '_' and '-'")]
    Charset,
}

impl SourceId {
    /// Validates and takes an identifier.
    ///
    /// # Errors
    ///
    /// [`SourceIdError`] when the identifier is empty, over 64 bytes, or holds
    /// a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, SourceIdError> {
        let value = value.into();
        if value.is_empty() || value.len() > 64 {
            return Err(SourceIdError::Length);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(SourceIdError::Charset);
        }
        Ok(Self { identifier: value })
    }

    /// The identifier. It is this crate's own metadata, not ingested content.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// Everything known about a wrapped value that is not the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    source_id: SourceId,
    kind: SourceKind,
    ingest_seq: u64,
}

impl Provenance {
    /// Names a source at a position in the ingest order.
    #[must_use]
    pub const fn new(source_id: SourceId, kind: SourceKind, ingest_seq: u64) -> Self {
        Self {
            source_id,
            kind,
            ingest_seq,
        }
    }

    /// Which document.
    #[must_use]
    pub const fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Which kind of source.
    #[must_use]
    pub const fn kind(&self) -> SourceKind {
        self.kind
    }

    /// Position in this profile's ingest order. Origin order only: it is not a
    /// wall clock and it is not valid time.
    #[must_use]
    pub const fn ingest_seq(&self) -> u64 {
        self.ingest_seq
    }
}

/// A value that came from outside this system.
///
/// See the module documentation for what this type deliberately does not
/// implement and where the one accessor is enumerated.
pub struct Untrusted<T> {
    value: T,
    provenance: Provenance,
    digest: String,
    byte_len: usize,
    // The marker documents that the wrapper is about `T` and keeps the struct
    // from being built by a caller that lists its fields positionally.
    sealed: PhantomData<fn() -> T>,
}

impl<T> Untrusted<T> {
    /// Wraps `value`, recording the digest and length of the bytes it was
    /// parsed from.
    pub(crate) fn seal(value: T, provenance: Provenance, bytes: &[u8]) -> Self {
        Self {
            value,
            provenance,
            digest: digest_of(bytes),
            byte_len: bytes.len(),
            sealed: PhantomData,
        }
    }

    /// The one accessor. `pub(crate)`; every call site is enumerated in
    /// `every_exposure_site_is_named_and_justified`.
    pub(crate) const fn expose(&self) -> &T {
        &self.value
    }

    /// Where the bytes came from.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Lowercase SHA-256 of the bytes this value was parsed from.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Length in bytes of what was parsed.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }
}

impl<T> fmt::Debug for Untrusted<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Untrusted")
            .field("provenance", &self.provenance)
            .field("digest", &self.digest)
            .field("byte_len", &self.byte_len)
            .field(
                "value",
                &format_args!("<untrusted:{} bytes>", self.byte_len),
            )
            .finish()
    }
}
