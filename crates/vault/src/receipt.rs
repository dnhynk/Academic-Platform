//! Opaque sealed-object capability issued only by the ingest/read-back path.

use std::path::{Path, PathBuf};

use academic_domain::{ArtifactDescriptor, ArtifactId, ContentDigest};
use academic_store::SealedObjectReceipt;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedArtifactReceipt {
    descriptor: ArtifactDescriptor,
    object_path: PathBuf,
    disposition: SealDisposition,
}

impl SealedArtifactReceipt {
    pub(crate) fn new(
        descriptor: ArtifactDescriptor,
        object_path: PathBuf,
        disposition: SealDisposition,
    ) -> Self {
        Self {
            descriptor,
            object_path,
            disposition,
        }
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

impl SealedObjectReceipt for SealedArtifactReceipt {
    fn artifact_id(&self) -> ArtifactId {
        self.descriptor.id
    }

    fn content_digest(&self) -> ContentDigest {
        self.descriptor.content_digest
    }
}
