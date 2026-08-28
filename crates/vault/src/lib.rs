//! Crash-safe, synthetic-only Phase 1 artifact vault.
//!
//! The product composition root supplies a validated profile root. Its plaintext format is
//! deliberately unsuitable for real or production data; the purpose of this crate is to make
//! durable publication, exact-policy deduplication, and reconciliation executable before ADR-004
//! selects an encrypted object format.
//!
//! A sealed capability retains a no-follow handle, exact host object identity, and a shared
//! policy-namespace lease until Store finishes its SQL transaction. Every product-controlled
//! ingest/reconcile/remove/replace path uses the same lease namespace. Unix leases are advisory,
//! so an unrelated same-user process or stronger local attacker can ignore them; Store still
//! revalidates the retained handle and canonical path immediately before commit and treats any
//! missing, truncated, replaced, or identity-changed object as failure. Preventing a hostile
//! out-of-process filesystem mutation in the final instruction window is outside this local
//! cooperative boundary and remains part of the daemon/profile trust assumption.
//!
//! # Concurrency contract
//!
//! One [`Vault`] value is safe to share by reference across threads: [`Vault::ingest`],
//! [`Vault::verify_sealed_object`], and [`Vault::revalidate_sealed_object`] may run concurrently
//! on `&Vault`, for the same artifact or for different artifacts, without a caller-supplied lock.
//! Concurrent ingest of identical bytes into the same policy namespace publishes exactly one
//! object and adopts it for every other caller. A caller therefore never has to serialize ingest
//! to avoid spurious failures, receipt-less published objects, or leaked partials.
//!
//! [`Vault::reconcile`] carries no such guarantee. It is a startup pass that removes expired
//! partials and quarantines orphans, and no test exercises it concurrently with ingest, so the
//! daemon must run it before it accepts clients.

mod durability;
mod fault;
mod ingest;
pub mod layout;
mod receipt;
mod reconcile;

use std::{
    collections::BTreeMap,
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
};

use academic_domain::{ArtifactDescriptor, ArtifactId, DomainError, DomainId, VaultLocator};

pub use ingest::ArtifactIngestRequest;
pub use layout::VaultLayout;
pub use receipt::{SealDisposition, SealedArtifactReceipt, SealedObjectCapability};
pub use reconcile::{ReconcileOptions, ReconcileRecord, ReconcileReport, ReconcileState};

/// Disposable plaintext object format used only by synthetic Phase 1 work.
pub const VAULT_WRITE_FORMAT: &str = "PLAINTEXT_SYNTHETIC_V1";
/// Oldest readable object format during Phase 1.
pub const VAULT_MIN_READ_FORMAT: &str = "PLAINTEXT_SYNTHETIC_V1";
/// Version component included in the keyed locator input.
pub const VAULT_FORMAT_VERSION: u16 = 1;

/// Describes the deliberately non-production vault contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultFormatContract {
    pub read_format: &'static str,
    pub write_format: &'static str,
    pub encrypted: bool,
    pub production_data_allowed: bool,
}

/// Exact Phase 1 vault posture.
pub const PHASE1_VAULT_FORMAT: VaultFormatContract = VaultFormatContract {
    read_format: VAULT_MIN_READ_FORMAT,
    write_format: VAULT_WRITE_FORMAT,
    encrypted: false,
    production_data_allowed: false,
};

/// Stable error boundary for vault mutation and integrity checks.
#[derive(Debug)]
#[non_exhaustive]
pub enum VaultError {
    /// A filesystem operation failed.
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    /// Canonical domain validation failed.
    Domain(DomainError),
    /// No locator key was registered for the requested security domain.
    MissingDomainKey(DomainId),
    /// A domain key was empty.
    EmptyDomainKey(DomainId),
    /// Re-registering a domain attempted to change its in-memory key.
    DomainKeyConflict(DomainId),
    /// The descriptor locator did not match the keyed digest/media-type derivation.
    LocatorMismatch(ArtifactId),
    /// An existing object occupied the exact locator but did not contain the expected bytes.
    PathCollision(PathBuf),
    /// A just-published or referenced object failed exact read-back verification.
    IntegrityMismatch(PathBuf),
    /// Another product-controlled operation owns the object's exclusive namespace lease.
    LeaseUnavailable(PathBuf),
    /// A vault entry had an unsafe or unsupported physical shape.
    UnsafeEntry(PathBuf),
    /// The operating system could not provide random bytes for a unique ingest session.
    EntropyUnavailable,
    /// System time could not be represented in the stable millisecond form.
    ClockUnavailable,
    /// The streamed byte count exceeded the portable artifact contract.
    ArtifactTooLarge,
}

impl VaultError {
    pub(crate) fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for VaultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} at {}: {source}", path.display()),
            Self::Domain(source) => source.fmt(formatter),
            Self::MissingDomainKey(domain_id) => {
                write!(
                    formatter,
                    "no vault locator key is registered for domain {domain_id}"
                )
            }
            Self::EmptyDomainKey(domain_id) => {
                write!(
                    formatter,
                    "vault locator key for domain {domain_id} is empty"
                )
            }
            Self::DomainKeyConflict(domain_id) => write!(
                formatter,
                "vault locator key for domain {domain_id} cannot change in an open vault"
            ),
            Self::LocatorMismatch(artifact_id) => write!(
                formatter,
                "artifact {artifact_id} has a locator that does not match its keyed descriptor"
            ),
            Self::PathCollision(path) => write!(
                formatter,
                "vault locator collision at {}; existing bytes were not overwritten",
                path.display()
            ),
            Self::IntegrityMismatch(path) => {
                write!(
                    formatter,
                    "vault object integrity mismatch at {}",
                    path.display()
                )
            }
            Self::LeaseUnavailable(path) => write!(
                formatter,
                "vault object namespace lease is unavailable at {}",
                path.display()
            ),
            Self::UnsafeEntry(path) => {
                write!(
                    formatter,
                    "unsafe or unsupported vault entry at {}",
                    path.display()
                )
            }
            Self::EntropyUnavailable => {
                formatter.write_str("vault ingest session entropy unavailable")
            }
            Self::ClockUnavailable => {
                formatter.write_str("system clock is unavailable for vault bookkeeping")
            }
            Self::ArtifactTooLarge => {
                formatter.write_str("artifact exceeds the portable exact byte-length range")
            }
        }
    }
}

impl Error for VaultError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Domain(source) => Some(source),
            _ => None,
        }
    }
}

impl From<DomainError> for VaultError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}

/// Result returned by vault operations.
pub type VaultResult<T> = Result<T, VaultError>;

/// In-memory locator keys keyed by physical security domain.
///
/// Keys are never persisted by the plaintext V1 vault and its `Debug` output reveals only the
/// domain count. A later key broker replaces this deliberately narrow Phase 1 boundary.
pub struct DomainKeyring {
    keys: BTreeMap<DomainId, Vec<u8>>,
}

impl DomainKeyring {
    /// Creates an empty keyring.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keys: BTreeMap::new(),
        }
    }

    /// Registers exactly one non-empty locator key for a domain.
    pub fn insert(&mut self, domain_id: DomainId, key: &[u8]) -> VaultResult<()> {
        if key.is_empty() {
            return Err(VaultError::EmptyDomainKey(domain_id));
        }
        if let Some(existing) = self.keys.get(&domain_id) {
            if existing == key {
                return Ok(());
            }
            return Err(VaultError::DomainKeyConflict(domain_id));
        }
        self.keys.insert(domain_id, key.to_vec());
        Ok(())
    }

    pub(crate) fn get(&self, domain_id: DomainId) -> VaultResult<&[u8]> {
        self.keys
            .get(&domain_id)
            .map(Vec::as_slice)
            .ok_or(VaultError::MissingDomainKey(domain_id))
    }
}

impl Default for DomainKeyring {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for DomainKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomainKeyring")
            .field("domain_count", &self.keys.len())
            .finish_non_exhaustive()
    }
}

impl Drop for DomainKeyring {
    fn drop(&mut self) {
        for key in self.keys.values_mut() {
            key.fill(0);
        }
    }
}

/// Open synthetic vault bound to one validated local profile.
///
/// Sharing one value by reference across threads is supported; see the crate-level concurrency
/// contract for exactly which operations that covers.
#[derive(Debug)]
pub struct Vault {
    layout: VaultLayout,
    keyring: DomainKeyring,
}

impl Vault {
    /// Opens the vault below a validated synthetic-only profile and durably initializes layout.
    pub fn open(profile_root: &Path, keyring: DomainKeyring) -> VaultResult<Self> {
        let layout = VaultLayout::new(profile_root);
        layout.initialize()?;
        Ok(Self { layout, keyring })
    }

    /// Returns the validated profile root to which every issued capability is physically bound.
    #[must_use]
    pub fn profile_root(&self) -> &Path {
        self.layout.profile_root()
    }

    /// Returns the physical layout rooted below this profile.
    #[must_use]
    pub const fn layout(&self) -> &VaultLayout {
        &self.layout
    }

    /// Streams, seals, publishes, and reads back one exact synthetic artifact.
    ///
    /// Concurrent calls on a shared `&Vault` are supported; the caller must not serialize them.
    pub fn ingest(
        &self,
        request: &ArtifactIngestRequest,
        source: impl io::Read,
    ) -> VaultResult<SealedArtifactReceipt> {
        ingest::ingest(self, request, source)
    }

    /// Reconciles temp files, sealed objects, quarantine, and authoritative references.
    ///
    /// This is a startup pass. It carries no concurrency guarantee against a running ingest.
    pub fn reconcile(&self, options: &ReconcileOptions<'_>) -> VaultResult<ReconcileReport> {
        reconcile::reconcile(self, options)
    }

    pub(crate) fn derive_locator(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> VaultResult<VaultLocator> {
        let key = self.keyring.get(descriptor.domain_id)?;
        VaultLocator::derive(
            key,
            VAULT_FORMAT_VERSION,
            &descriptor.media_type,
            descriptor.content_digest,
        )
        .map_err(Into::into)
    }

    pub(crate) fn validate_descriptor_locator(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> VaultResult<PathBuf> {
        descriptor.validate()?;
        if descriptor.format_version != VAULT_FORMAT_VERSION
            || self.derive_locator(descriptor)? != descriptor.vault_locator
        {
            return Err(VaultError::LocatorMismatch(descriptor.id));
        }
        self.layout.object_path(descriptor)
    }

    /// Reads back the exact canonical object and issues a non-mintable concrete capability.
    pub fn verify_sealed_object(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> VaultResult<SealedObjectCapability> {
        let path = self.validate_descriptor_locator(descriptor)?;
        let lease_path = self.layout.ensure_lease_path(descriptor)?;
        let lease = durability::acquire_shared_object_lease(&lease_path)?;
        let evidence = ingest::verify_object_with_lease(
            &path,
            descriptor.content_digest,
            descriptor.byte_length,
            lease,
        )?;
        Ok(SealedArtifactReceipt::new(
            descriptor.clone(),
            path,
            SealDisposition::AdoptedExisting,
            evidence,
        ))
    }

    /// Revalidates one live capability against this vault immediately before SQL commit.
    ///
    /// The retained no-follow handle must still contain the exact digest/length, and reopening the
    /// canonical path under the same shared lease must resolve to that same host object identity.
    pub fn revalidate_sealed_object(
        &self,
        capability: &mut SealedObjectCapability,
    ) -> VaultResult<()> {
        let canonical_path = self.validate_descriptor_locator(capability.descriptor())?;
        capability.revalidate(&canonical_path)
    }
}

pub(crate) fn now_millis() -> VaultResult<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| VaultError::ClockUnavailable)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| VaultError::ClockUnavailable)
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

pub(crate) fn integrity_mismatch(path: &Path) -> VaultError {
    VaultError::IntegrityMismatch(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_vault_format_never_claims_acceptance() {
        const {
            assert!(!PHASE1_VAULT_FORMAT.encrypted);
            assert!(!PHASE1_VAULT_FORMAT.production_data_allowed);
        }
        assert!(VAULT_WRITE_FORMAT.contains("PLAINTEXT_SYNTHETIC"));
    }

    #[test]
    fn keyring_debug_never_exposes_key_material() -> VaultResult<()> {
        let domain_id: DomainId = "01900000-0000-7000-8000-000000000001".parse()?;
        let mut keyring = DomainKeyring::new();
        keyring.insert(domain_id, b"do-not-print-this-key")?;
        let rendered = format!("{keyring:?}");
        assert!(rendered.contains("domain_count"));
        assert!(!rendered.contains("do-not-print-this-key"));
        Ok(())
    }

    #[test]
    fn local_hex_encoder_is_lowercase_and_exact() {
        assert_eq!(encode_hex(&[0x00, 0x1f, 0xa5, 0xff]), "001fa5ff");
    }

    #[test]
    fn digest_type_remains_exact() {
        let digest = academic_domain::ContentDigest::sha256(b"synthetic");
        assert_eq!(digest.as_bytes().len(), 32);
    }
}
