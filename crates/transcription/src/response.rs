//! The raw provider response, and the archive that keeps every one of them.
//!
//! # The bytes have one accessor and it is crate-private
//!
//! [`ProviderResponse::response_bytes`] is `pub(crate)`, so no caller outside
//! this crate can name it. Inside it, every call site is listed in
//! `RAW_BYTE_SITES` in `tests/transcription_scans.rs` with a written reason and
//! the **whole inventory** is compared, counted by identifier rather than by
//! spelling -- `ProviderResponse::response_bytes(r)` is the same call as
//! `r.response_bytes()` and a count of the second spelling would not see the
//! first. That is `P2-RF10`'s repair to `Untrusted::expose`'s inventory, copied
//! deliberately rather than reinvented.
//!
//! Crate-private stops a caller from calling it. It does not stop this crate
//! from calling it on a caller's behalf, so a second rule runs beside the
//! inventory: no `pub` signature in the workspace may take a
//! [`ProviderResponse`] or an [`ArchivedResponse`] and return a type naming
//! `str`, `String` or `u8`.
//!
//! # Two producers, and what each one proves
//!
//! [`ProviderResponse::from_local`] is the default route. It proves nothing
//! about egress and does not need to: a local provider transmits no byte, and
//! `academic_model_run::Transmission::LocalOnly` is what the run records.
//!
//! [`ProviderResponse::from_remote`] takes both a
//! [`crate::route::RemoteAdmission`] -- which only the scoped-remote arm of
//! `SttPolicy::route_for` produces -- and an
//! `academic_egress_boundary::AcceptedResponse`, whose one producer is
//! `EgressProxy::accept_response`. So a remote response that spent no grant,
//! passed no rulepack and reached no canary scan is not a value this crate can
//! be handed, and the reuse is the argument type rather than a comment.
//!
//! # The archive appends
//!
//! [`RawResponseArchive`] has one `&mut self` method and it pushes. There is no
//! removal, no replacement, and no `&mut` accessor into an entry, which is
//! ADR-003's rule for every canonical record rather than a second mechanism
//! invented here. A re-transcription adds an entry beside the first; both stay,
//! and `provider_retranscription_compare` reads the pair.

use academic_domain::ContentDigest;
use academic_egress_boundary::AcceptedResponse;
use academic_model_run::{ModelVersion, ProviderId};
use academic_untrusted_content::{
    IngestedDocument, SourceId, SourceIdError, SourceKind, Untrusted,
};

use crate::{provider::ProviderPlacement, route::RemoteAdmission};

/// A provider's answer, exactly as it arrived.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderResponse {
    provider: ProviderId,
    model_version: ModelVersion,
    placement: ProviderPlacement,
    provider_response_bytes: Vec<u8>,
    digest: ContentDigest,
}

impl ProviderResponse {
    /// Takes a local provider's answer.
    #[must_use]
    pub fn from_local(provider: ProviderId, model_version: ModelVersion, bytes: &[u8]) -> Self {
        Self {
            provider,
            model_version,
            placement: ProviderPlacement::Local,
            provider_response_bytes: bytes.to_vec(),
            digest: ContentDigest::sha256(bytes),
        }
    }

    /// Takes a remote provider's answer, under the admission that let the
    /// request leave and after the egress boundary accepted it.
    #[must_use]
    pub fn from_remote(admission: &RemoteAdmission, accepted: &AcceptedResponse) -> Self {
        let bytes = accepted.bytes();
        Self {
            provider: admission.provider().clone(),
            model_version: admission.model_version().clone(),
            placement: ProviderPlacement::Remote,
            provider_response_bytes: bytes.to_vec(),
            digest: ContentDigest::sha256(bytes),
        }
    }

    /// Which provider answered.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which exact model version answered.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// Where that provider ran.
    #[must_use]
    pub const fn placement(&self) -> ProviderPlacement {
        self.placement
    }

    /// SHA-256 over the response as it arrived.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many bytes it carries.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.provider_response_bytes.len()
    }

    /// The bytes. Crate-private; every call site is inventoried.
    pub(crate) fn response_bytes(&self) -> &[u8] {
        &self.provider_response_bytes
    }
}

// A provider response is the lecture's own words. The `S-10` decision for this
// field is made in the strengthening direction: the buffer reaches the
// formatter through a length only, and the type is registered in
// `SECRET_BEARING_TYPES` rather than exempted in `PUBLIC_BYTES`.
impl core::fmt::Debug for ProviderResponse {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ProviderResponse")
            .field("placement", &self.placement)
            .field("byte_len", &self.provider_response_bytes.len())
            .finish()
    }
}

/// Where one archived response sits in the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawResponseId(u32);

impl RawResponseId {
    /// Its position, from zero.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl core::fmt::Display for RawResponseId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "raw_response_{:04}", self.0)
    }
}

/// One retained raw provider response.
///
/// It keeps the response's identity and the `P2-G5` seal, and it hands out no
/// byte of the response itself. What a caller that wants to put a provider
/// response into a prompt gets is [`ArchivedResponse::labelled`], an
/// `Untrusted<IngestedDocument>` -- which implements no `Deref`, no `Display`
/// and no `Into`, and whose one accessor is private to `academic-untrusted-content`.
#[derive(Debug)]
pub struct ArchivedResponse {
    id: RawResponseId,
    provider: ProviderId,
    model_version: ModelVersion,
    placement: ProviderPlacement,
    digest: ContentDigest,
    byte_len: usize,
    labelled: Untrusted<IngestedDocument>,
}

impl ArchivedResponse {
    /// Its position in the archive.
    #[must_use]
    pub const fn id(&self) -> RawResponseId {
        self.id
    }

    /// Which provider answered.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which exact model version answered.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// Where that provider ran.
    #[must_use]
    pub const fn placement(&self) -> ProviderPlacement {
        self.placement
    }

    /// SHA-256 over the response as it arrived.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many bytes were retained.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// The response under `P2-G5`'s trust label, which is the only form it
    /// leaves this crate in.
    #[must_use]
    pub const fn labelled(&self) -> &Untrusted<IngestedDocument> {
        &self.labelled
    }
}

/// Why a response could not be retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArchiveFault {
    /// The identifier the seal needs was refused.
    #[error("the archive identifier was refused: {0}")]
    Identifier(#[from] SourceIdError),
    /// `academic-untrusted-content` refused the bytes.
    #[error("the response could not be sealed as untrusted content")]
    NotSealable,
}

/// Every raw provider response this profile has kept.
///
/// One `&mut self` method, and it only extends.
#[derive(Debug, Default)]
pub struct RawResponseArchive {
    entries: Vec<ArchivedResponse>,
}

impl RawResponseArchive {
    /// An empty archive.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Retains one response and seals it under `P2-G5`'s label.
    ///
    /// The one mutating operation in this type, and it appends.
    ///
    /// # Errors
    ///
    /// [`ArchiveFault::Identifier`] when the derived identifier is refused, and
    /// [`ArchiveFault::NotSealable`] when the bytes are not UTF-8 or are longer
    /// than `academic_untrusted_content::MAX_SOURCE_BYTES`.
    pub fn retain(&mut self, response: &ProviderResponse) -> Result<RawResponseId, ArchiveFault> {
        let id = RawResponseId(u32::try_from(self.entries.len()).unwrap_or(u32::MAX));
        let source_id = SourceId::new(id.to_string())?;
        // The first of two call sites of `response_bytes`. Sealing has to read
        // the bytes it hashes and wraps; what leaves is an
        // `Untrusted<IngestedDocument>`, which hands back no text.
        let labelled = academic_untrusted_content::ingest(
            source_id,
            SourceKind::ProviderResponse,
            u64::from(id.value()),
            response.response_bytes(),
        )
        .map_err(|_| ArchiveFault::NotSealable)?;
        self.entries.push(ArchivedResponse {
            id,
            provider: response.provider().clone(),
            model_version: response.model_version().clone(),
            placement: response.placement(),
            digest: *response.digest(),
            byte_len: response.byte_len(),
            labelled,
        });
        Ok(id)
    }

    /// Every retained response, in the order they arrived.
    #[must_use]
    pub fn entries(&self) -> &[ArchivedResponse] {
        &self.entries
    }

    /// One retained response.
    #[must_use]
    pub fn get(&self, id: RawResponseId) -> Option<&ArchivedResponse> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// How many responses are retained.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
