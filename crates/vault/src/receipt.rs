//! Opaque sealed-object capability issued only by the ingest/read-back path.

use std::path::{Path, PathBuf};

use academic_domain::ArtifactDescriptor;

use crate::{VaultResult, ingest::LiveObjectEvidence, integrity_mismatch};

/// Whether sealing published new bytes or adopted an exact existing object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealDisposition {
    /// The no-replace publication installed this object.
    PublishedNew,
    /// Exact policy, media type, length, digest, and read-back matched an existing object.
    AdoptedExisting,
}

/// Opaque proof that exact artifact bytes are durably present and were read back.
///
/// Every field is private and the constructor is crate-private. External code can obtain this
/// capability only through [`crate::Vault::ingest`] or the sealed-object verifier.
///
/// ```compile_fail
/// use academic_vault::SealedArtifactReceipt;
///
/// // There is deliberately no public hash/ID constructor or public field literal.
/// let _forged = SealedArtifactReceipt {};
/// ```
#[derive(Debug)]
pub struct SealedArtifactReceipt {
    descriptor: ArtifactDescriptor,
    object_path: PathBuf,
    disposition: SealDisposition,
    live_evidence: LiveObjectEvidence,
}

/// Concrete vault-backed capability consumed by durable acceptance.
///
/// This alias deliberately has no constructor of its own. Safe downstream code can only obtain
/// a value after [`crate::Vault::ingest`] or [`crate::Vault::verify_sealed_object`] has read back
/// the exact bytes from the canonical policy-namespaced vault path.
pub type SealedObjectCapability = SealedArtifactReceipt;

impl SealedArtifactReceipt {
    pub(crate) fn new(
        descriptor: ArtifactDescriptor,
        object_path: PathBuf,
        disposition: SealDisposition,
        live_evidence: LiveObjectEvidence,
    ) -> Self {
        Self {
            descriptor,
            object_path,
            disposition,
            live_evidence,
        }
    }

    pub(crate) fn revalidate(&mut self, canonical_path: &Path) -> VaultResult<()> {
        if canonical_path != self.object_path {
            return Err(integrity_mismatch(canonical_path));
        }
        self.live_evidence.revalidate(
            canonical_path,
            self.descriptor.content_digest,
            self.descriptor.byte_length,
        )
    }

    /// Returns the exact immutable descriptor bound by the receipt.
    #[must_use]
    pub const fn descriptor(&self) -> &ArtifactDescriptor {
        &self.descriptor
    }

    /// Returns the canonical physical object path for audit and local verification.
    #[must_use]
    pub fn object_path(&self) -> &Path {
        &self.object_path
    }

    /// Reports whether this call published or adopted the exact sealed object.
    #[must_use]
    pub const fn disposition(&self) -> SealDisposition {
        self.disposition
    }
}
