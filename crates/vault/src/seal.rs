//! The store↔vault seam.
//!
//! t068 section 2.3-8 fixes this seam as a trait pair rather than a concrete
//! type: `academic-store` consumes read-back evidence through
//! [`SealedObjectVerifier`] and [`SealedObjectReceipt`], so adding an encrypted
//! vault gives the store no byte or hash bypass and no second acceptance path.
//!
//! Neither trait can mint evidence. [`SealedObjectReceipt`] is read-only, and
//! the only implementors are the crate-private receipt types this crate issues
//! after it has read an object back from its canonical policy-namespaced path.
//! A downstream crate can name both traits and still construct nothing.

use std::path::Path;

use academic_domain::ArtifactDescriptor;

use crate::{SealDisposition, VaultResult};

/// Read-back evidence a vault issues for exactly one canonical object.
pub trait SealedObjectReceipt {
    /// Returns the exact immutable descriptor the read-back was verified against.
    fn descriptor(&self) -> &ArtifactDescriptor;

    /// Returns the canonical physical object path for audit and local verification.
    fn object_path(&self) -> &Path;

    /// Reports whether sealing published new bytes or adopted an exact existing object.
    fn disposition(&self) -> SealDisposition;
}

/// The object-integrity authority a canonical writer defers to.
///
/// Implementors read an object back from its canonical path before issuing a
/// receipt, and revalidate the live object immediately before the writer
/// commits. A caller that holds only this trait cannot skip either step.
pub trait SealedObjectVerifier {
    /// Evidence this verifier issues.
    type Receipt: SealedObjectReceipt;

    /// Returns the validated profile root every issued receipt is bound to.
    fn profile_root(&self) -> &Path;

    /// Reads back the exact canonical object and issues a non-mintable receipt.
    fn verify_sealed_object(&self, descriptor: &ArtifactDescriptor) -> VaultResult<Self::Receipt>;

    /// Revalidates one live receipt against this vault immediately before commit.
    fn revalidate_sealed_object(&self, receipt: &mut Self::Receipt) -> VaultResult<()>;
}
