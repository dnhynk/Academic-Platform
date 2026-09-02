//! Sealing a transcript original into the encrypted vault.
//!
//! This module adds **no object format**. It composes ADR-004's
//! `AEAD_CHUNKED_V2` through `academic-vault`'s public `EncryptedVault::ingest`
//! and fixes two things on top of it:
//!
//! - the confidentiality and retention labels a transcript original must carry
//!   — `RESTRICTED` and `USER_MANAGED`, from section 32.2's `Z1` and section
//!   29.3 — are hard-coded in [`transcript_ingest_request`] and re-checked in
//!   [`seal_transcript_original`], so a caller that assembled its own request
//!   with looser labels is refused rather than obeyed;
//! - the single sealing entry point [`store_transcript_original`], which takes
//!   an [`AdmittedImport`] and has no ungated twin. Admission is closed, so it
//!   is unreachable in this repository except through the fault lane's
//!   `AdmittedImport::for_fault_injection_only`; that is the one hole, and it
//!   is compiled only by a test-only feature.

use academic_domain::{
    ArtifactId, Confidentiality, DomainId, MediaType, PermissionLineageId, RetentionClass,
};
use academic_vault::{ArtifactIngestRequest, EncryptedVault, SealedEncryptedObject};

use crate::{TranscriptError, admission::AdmittedImport};

/// Confidentiality every transcript original is sealed under.
pub const TRANSCRIPT_CONFIDENTIALITY: Confidentiality = Confidentiality::Restricted;
/// Retention class every transcript original is sealed under.
pub const TRANSCRIPT_RETENTION_CLASS: RetentionClass = RetentionClass::UserManaged;

/// Builds the ingest request a transcript original must be sealed under.
///
/// The two policy labels are not parameters. A caller chooses the artifact, the
/// media type, the domain and the permission lineage; it does not choose
/// whether its transcript is restricted.
pub fn transcript_ingest_request(
    artifact_id: ArtifactId,
    media_type: MediaType,
    domain_id: DomainId,
    permission_lineage_id: PermissionLineageId,
) -> ArtifactIngestRequest {
    ArtifactIngestRequest::new(
        artifact_id,
        media_type,
        domain_id,
        TRANSCRIPT_CONFIDENTIALITY,
        TRANSCRIPT_RETENTION_CLASS,
        permission_lineage_id,
    )
}

/// Seals a transcript original into a profile as an `AEAD_CHUNKED_V2` object.
///
/// Requires an [`AdmittedImport`]; see [`crate::admission`] for why that makes
/// it unreachable in this repository today.
///
/// # Errors
///
/// Refuses a request whose confidentiality or retention class is not the pair
/// above, before any byte is written.
pub fn store_transcript_original(
    _admitted: &AdmittedImport,
    vault: &EncryptedVault,
    request: &ArtifactIngestRequest,
    original: &[u8],
) -> Result<SealedEncryptedObject, TranscriptError> {
    if request.confidentiality() != TRANSCRIPT_CONFIDENTIALITY
        || request.retention_class() != TRANSCRIPT_RETENTION_CLASS
    {
        return Err(TranscriptError::OriginalPolicyMismatch {
            confidentiality: request.confidentiality(),
            retention_class: request.retention_class(),
        });
    }
    Ok(vault.ingest(request, original)?)
}
