//! Stage one: the conditional request, and the transport this crate does not have.
//!
//! Section 29.2 asks for *low-frequency conditional fetch with hash diff*. The
//! conditional half is [`ConditionalRequest`], which carries the validators the
//! previous snapshot recorded; the hash half is [`crate::snapshot`].
//!
//! # There is no transport here
//!
//! [`ConditionalFetch`] is a trait the caller implements, exactly as
//! `academic-egress-boundary` takes its `OutboundTransport` from its caller.
//! This crate implements it nowhere, and the tests supply a fixture that answers
//! from committed bytes. `only_egress_crate_has_a_socket` is the workspace-wide
//! statement that no crate outside the two egress crates spells a socket;
//! `credentials_never_reach_a_general_crawler` is this crate's own, and
//! `this_crate_declares_three_product_edges` pins the manifest an HTTP client or
//! a decoder would have to be added to.
//!
//! # A request is not a function of a response
//!
//! Every constructor here takes a [`crate::manifest::DeclaredTarget`], which is
//! `&'static`, and a [`Validators`] built from a previous snapshot's recorded
//! header values. None of them takes a [`FetchOutcome`], a body, or anything
//! derived from one. `no_captcha_or_access_control_bypass_module_exists`
//! compares the whole set of signatures in this crate that produce or consume a
//! request against a pinned list, and requires each to name no response type, so
//! a challenge-response loop — which is what a bypass of an access control is —
//! fails as an extra key rather than as a missing token.

use academic_domain::ContentDigest;

use crate::{
    identifier::ConnectorId,
    manifest::{ConnectorManifest, CredentialBinding, DeclaredTarget, RetrievalInstant},
    terms::{Denial, DenialReason, deny},
};

/// The longest header value this crate will hold.
pub const MAX_HEADER_BYTES: usize = 128;

/// Why a header value was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HeaderError {
    /// Empty, or longer than [`MAX_HEADER_BYTES`].
    #[error("a header value is 1..={MAX_HEADER_BYTES} bytes")]
    Length,
    /// Held a byte outside printable ASCII.
    #[error("a header value holds only printable ASCII")]
    Charset,
}

/// One header value taken from a response and echoed back on the next request.
///
/// A validator is the one thing this crate reads out of a response and later
/// writes into a request, so it is the one place a source could steer a later
/// message. The charset is printable ASCII and the length is bounded, which
/// removes the separator a header injection needs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeaderValue {
    value: String,
}

impl HeaderValue {
    /// Validates and takes a header value.
    ///
    /// # Errors
    ///
    /// [`HeaderError`] when the value is empty, over [`MAX_HEADER_BYTES`], or
    /// holds a byte outside printable ASCII.
    pub fn new(value: impl Into<String>) -> Result<Self, HeaderError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_HEADER_BYTES {
            return Err(HeaderError::Length);
        }
        if !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
            return Err(HeaderError::Charset);
        }
        Ok(Self { value })
    }

    /// The value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// What the previous snapshot recorded that makes the next request conditional.
///
/// Both are absent on a first fetch, and [`Self::is_conditional`] says so.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Validators {
    entity_tag: Option<HeaderValue>,
    last_modified: Option<HeaderValue>,
}

impl Validators {
    /// No validators. A first fetch.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            entity_tag: None,
            last_modified: None,
        }
    }

    /// With an entity tag.
    #[must_use]
    pub fn with_entity_tag(mut self, value: HeaderValue) -> Self {
        self.entity_tag = Some(value);
        self
    }

    /// With a last-modified value.
    #[must_use]
    pub fn with_last_modified(mut self, value: HeaderValue) -> Self {
        self.last_modified = Some(value);
        self
    }

    /// The entity tag, if one was recorded.
    #[must_use]
    pub const fn entity_tag(&self) -> Option<&HeaderValue> {
        self.entity_tag.as_ref()
    }

    /// The last-modified value, if one was recorded.
    #[must_use]
    pub const fn last_modified(&self) -> Option<&HeaderValue> {
        self.last_modified.as_ref()
    }

    /// Whether this request can come back unmodified.
    #[must_use]
    pub const fn is_conditional(&self) -> bool {
        self.entity_tag.is_some() || self.last_modified.is_some()
    }
}

/// One request against one declared document.
///
/// Private fields and no public struct literal: the two constructors are the
/// only route, and both refuse a target the manifest does not declare.
#[derive(Debug)]
pub struct ConditionalRequest {
    connector: ConnectorId,
    target: DeclaredTarget,
    validators: Validators,
    credential: Option<CredentialBinding>,
}

impl ConditionalRequest {
    /// A request that presents nothing.
    ///
    /// # Errors
    ///
    /// [`Denial`] with [`DenialReason::UndeclaredTarget`] when the manifest
    /// does not declare `target`.
    pub fn anonymous(
        manifest: &ConnectorManifest,
        target: DeclaredTarget,
        validators: Validators,
    ) -> Result<Self, Denial> {
        if !manifest.declares(target) {
            return Err(deny(
                manifest.connector().clone(),
                DenialReason::UndeclaredTarget,
            ));
        }
        Ok(Self {
            connector: manifest.connector().clone(),
            target,
            validators,
            credential: None,
        })
    }

    /// A request that presents the connector's scoped credential.
    ///
    /// The binding is consumed, so it cannot be spent on a second request, and
    /// its connector must be the manifest's. Section 29.2's rule is these two
    /// conditions: a credential goes with one declared document of one declared
    /// connector, and there is no third constructor.
    ///
    /// # Errors
    ///
    /// [`Denial`] with [`DenialReason::UndeclaredTarget`] when the manifest
    /// does not declare `target`, or when the binding belongs to another
    /// connector.
    pub fn credentialed(
        manifest: &ConnectorManifest,
        binding: CredentialBinding,
        target: DeclaredTarget,
        validators: Validators,
    ) -> Result<Self, Denial> {
        if !manifest.declares(target) || binding.connector() != manifest.connector() {
            return Err(deny(
                manifest.connector().clone(),
                DenialReason::UndeclaredTarget,
            ));
        }
        Ok(Self {
            connector: manifest.connector().clone(),
            target,
            validators,
            credential: Some(binding),
        })
    }

    /// Which connector.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// Which declared document.
    #[must_use]
    pub const fn target(&self) -> DeclaredTarget {
        self.target
    }

    /// The validators the previous snapshot recorded.
    #[must_use]
    pub const fn validators(&self) -> &Validators {
        &self.validators
    }

    /// Whether a credential is presented.
    #[must_use]
    pub const fn presents_a_credential(&self) -> bool {
        self.credential.is_some()
    }
}

/// What a response said about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpMetadata {
    status: Option<u16>,
    entity_tag: Option<HeaderValue>,
    last_modified: Option<HeaderValue>,
    content_type: Option<HeaderValue>,
}

impl HttpMetadata {
    /// A response's metadata.
    #[must_use]
    pub const fn new(
        status: Option<u16>,
        entity_tag: Option<HeaderValue>,
        last_modified: Option<HeaderValue>,
        content_type: Option<HeaderValue>,
    ) -> Self {
        Self {
            status,
            entity_tag,
            last_modified,
            content_type,
        }
    }

    /// The status line's code, when the bytes came from a response.
    ///
    /// `None` for an import: a file a person handed over has no status line,
    /// and inventing one would put a number in the record that describes
    /// nothing. Nothing in this crate branches on this value.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        self.status
    }

    /// The entity tag.
    #[must_use]
    pub const fn entity_tag(&self) -> Option<&HeaderValue> {
        self.entity_tag.as_ref()
    }

    /// The last-modified value.
    #[must_use]
    pub const fn last_modified(&self) -> Option<&HeaderValue> {
        self.last_modified.as_ref()
    }

    /// The content type.
    #[must_use]
    pub const fn content_type(&self) -> Option<&HeaderValue> {
        self.content_type.as_ref()
    }

    /// The validators to send on the next request against the same document.
    #[must_use]
    pub fn next_validators(&self) -> Validators {
        let mut validators = Validators::none();
        if let Some(tag) = self.entity_tag.clone() {
            validators = validators.with_entity_tag(tag);
        }
        if let Some(modified) = self.last_modified.clone() {
            validators = validators.with_last_modified(modified);
        }
        validators
    }
}

/// What one fetch produced.
///
/// The transport reports the digest it computed while reading, beside the bytes
/// it assembled. `IN01` is the case where those two disagree, and
/// [`crate::snapshot::store`] is where that is caught.
///
/// `Debug` is hand-written for the reason [`crate::snapshot::RawSnapshot`]'s
/// is: the payload field is named `source_bytes`, which is in
/// `tools/secret-debug-policy.test.mjs`'s vocabulary, so a derived `Debug`
/// over it is refused by that net rather than by a rule this task invented.
#[derive(Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    /// The document is unchanged. Nothing is stored and no version is created.
    NotModified {
        /// When the source was asked.
        at: RetrievalInstant,
        /// What it said about itself.
        http: HttpMetadata,
    },
    /// The document was sent.
    Body {
        /// When the source was asked.
        at: RetrievalInstant,
        /// What it said about itself.
        http: HttpMetadata,
        /// The bytes, as assembled.
        source_bytes: Vec<u8>,
        /// The digest the transport computed while reading them.
        observed: ContentDigest,
    },
}

impl core::fmt::Debug for FetchOutcome {
    /// Prints what the source said and how many bytes arrived. Never the bytes,
    /// and never the digest of them.
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotModified { at, http } => formatter
                .debug_struct("NotModified")
                .field("at", at)
                .field("http", http)
                .finish(),
            Self::Body {
                at,
                http,
                source_bytes,
                observed: _,
            } => formatter
                .debug_struct("Body")
                .field("at", at)
                .field("http", http)
                .field("byte_len", &source_bytes.len())
                .finish(),
        }
    }
}

/// A source of bytes, supplied by the caller.
///
/// This crate implements it nowhere. The `Result` is the caller's transport
/// error, kept opaque here because nothing in this crate reads it beyond
/// stopping the run at stage one.
pub trait ConditionalFetch {
    /// Answers one conditional request.
    ///
    /// # Errors
    ///
    /// A description of why the caller's transport produced nothing. It is
    /// reported at stage one and the run stops there.
    fn fetch(&self, request: &ConditionalRequest) -> Result<FetchOutcome, String>;
}
