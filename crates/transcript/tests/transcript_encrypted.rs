//! `transcript_original_is_ciphertext_at_rest` — the `P2-U7` encrypted-lane row.
//!
//! It runs under the non-default `encrypted-vault` feature, which selects
//! `academic-vault/aead-objects`. No object format is defined here: the seal is
//! ADR-004's `AEAD_CHUNKED_V2` through the vault's public ingest, and this
//! suite measures what a profile holds afterwards.
//!
//! The measurement follows this repository's canary convention. The corpus is
//! built entirely from the committed tokens in
//! `testdata/transcript-canary/canaries.txt`, every file below the profile root
//! is streamed, and the result is reported as the same three counts an
//! admission receipt's platform row carries — `canary_file_count`,
//! `canary_byte_count`, `canary_hit_count` — because a scan that reports only
//! "no hits" cannot be told from a scan that read nothing.

#![cfg(all(feature = "encrypted-vault", feature = "phase2-fault-injection"))]

#[path = "../../test-support/src/encrypted_artifacts.rs"]
mod encrypted_artifacts;
mod support;

use std::{error::Error, fs, io::Read as _};

use academic_domain::{
    ArtifactId, Confidentiality, DomainId, MediaType, PermissionLineageId, RetentionClass,
};
use academic_transcript::{
    admission::AdmittedImport,
    source::build_synthetic_transcript_pdf,
    vault::{
        TRANSCRIPT_CONFIDENTIALITY, TRANSCRIPT_RETENTION_CLASS, store_transcript_original,
        transcript_ingest_request,
    },
};
use academic_vault::{ArtifactIngestRequest, ENCRYPTED_OBJECT_FORMAT, EncryptedVault};
use encrypted_artifacts::{create_master, keyring_for};
use support::{TestRoot, canaries, refusal, scan_for_canaries, synthetic_transcript};

type TestResult = Result<(), Box<dyn Error>>;

const ARTIFACT: &str = "01900000-0000-7000-8000-0000000007d1";
const DOMAIN: &str = "01900000-0000-7000-8000-0000000007d2";
const PERMISSION_LINEAGE: &str = "01900000-0000-7000-8000-0000000007d3";

/// Every byte of a transcript original is ciphertext once it is at rest.
#[test]
fn transcript_original_is_ciphertext_at_rest() -> TestResult {
    let root = TestRoot::new("ciphertext-at-rest")?;
    let (master, _record) = create_master()?;
    let vault = EncryptedVault::open(root.path(), keyring_for(&master, &[DOMAIN])?)?;

    let transcript = synthetic_transcript()?;
    let original = build_synthetic_transcript_pdf(&transcript).bytes;

    // The corpus is what makes the scan evidence: every committed canary is in
    // the bytes being sealed, so a zero hit count after sealing is a statement
    // about the vault rather than about the corpus.
    for token in canaries() {
        assert!(
            support::contains(&original, token.as_bytes()),
            "the original does not carry {token}, so a clean scan proves nothing"
        );
    }

    let request = transcript_ingest_request(
        ARTIFACT.parse::<ArtifactId>()?,
        MediaType::parse("application/pdf")?,
        DOMAIN.parse::<DomainId>()?,
        PERMISSION_LINEAGE.parse::<PermissionLineageId>()?,
    );
    assert_eq!(request.confidentiality(), TRANSCRIPT_CONFIDENTIALITY);
    assert_eq!(request.retention_class(), TRANSCRIPT_RETENTION_CLASS);
    assert_eq!(request.confidentiality(), Confidentiality::Restricted);
    assert_eq!(request.retention_class(), RetentionClass::UserManaged);

    let admitted = AdmittedImport::for_fault_injection_only();
    let sealed = store_transcript_original(&admitted, &vault, &request, &original)?;
    let descriptor = sealed.descriptor().clone();
    assert_eq!(descriptor.confidentiality, Confidentiality::Restricted);
    assert_eq!(descriptor.retention_class, RetentionClass::UserManaged);
    assert_eq!(
        descriptor.format_version, 2,
        "the original was not sealed in the ADR-004 object format"
    );
    assert_eq!(ENCRYPTED_OBJECT_FORMAT, "AEAD_CHUNKED_V2");

    // The whole profile, not only the object: an index, a temporary, or a
    // journal beside the object would be just as much of a leak.
    let summary = scan_for_canaries(root.path())?;
    assert!(
        summary.canary_file_count > 0,
        "the scan read no file, so its result is not evidence"
    );
    assert!(
        summary.canary_byte_count >= original.len() as u64,
        "the scan read {} bytes for a {}-byte original",
        summary.canary_byte_count,
        original.len()
    );
    assert_eq!(
        summary.canary_hit_count(),
        0,
        "plaintext canaries survived sealing: {:?}",
        summary.findings
    );

    // Encryption, not destruction: the exact original comes back.
    let mut reader = vault.open_reader(&descriptor)?;
    let mut round_trip = Vec::new();
    reader.read_to_end(&mut round_trip)?;
    assert_eq!(
        round_trip, original,
        "the sealed original did not read back"
    );

    // Injection — a plaintext sidecar inside the profile. A "keep the source
    // beside the object for provenance" change would look like this, and it is
    // one directory outside the object the row names.
    let sidecar = root.path().join("transcript-original.pdf");
    fs::write(&sidecar, &original)?;
    let leaked = scan_for_canaries(root.path())?;
    assert!(
        leaked.canary_hit_count() > 0,
        "the scan passed a profile holding the plaintext original"
    );
    fs::remove_file(&sidecar)?;
    assert_eq!(
        scan_for_canaries(root.path())?.canary_hit_count(),
        0,
        "removing the injected sidecar did not restore a clean scan"
    );
    Ok(())
}

/// A transcript original offered under looser policy labels is refused.
///
/// Before any byte is written, so a caller that assembled its own request
/// cannot downgrade a transcript to `PERSONAL`/`COURSE_TERM` by passing one.
#[test]
fn transcript_original_refuses_a_looser_storage_policy() -> TestResult {
    let root = TestRoot::new("policy-refusal")?;
    let (master, _record) = create_master()?;
    let vault = EncryptedVault::open(root.path(), keyring_for(&master, &[DOMAIN])?)?;
    let original = build_synthetic_transcript_pdf(&synthetic_transcript()?).bytes;
    let admitted = AdmittedImport::for_fault_injection_only();

    for (confidentiality, retention_class) in [
        (Confidentiality::Personal, RetentionClass::UserManaged),
        (Confidentiality::Restricted, RetentionClass::CourseTerm),
        (Confidentiality::Public, RetentionClass::Ephemeral),
    ] {
        let request = ArtifactIngestRequest::new(
            ARTIFACT.parse::<ArtifactId>()?,
            MediaType::parse("application/pdf")?,
            DOMAIN.parse::<DomainId>()?,
            confidentiality,
            retention_class,
            PERMISSION_LINEAGE.parse::<PermissionLineageId>()?,
        );
        let error = refusal(
            store_transcript_original(&admitted, &vault, &request, &original),
            "a looser storage policy sealed a transcript original",
        )?;
        assert_eq!(error.code(), "ORIGINAL_POLICY_MISMATCH");
    }

    // Nothing was written by the three refusals.
    assert_eq!(scan_for_canaries(root.path())?.canary_hit_count(), 0);

    // The capability the three calls above used came from the fault lane. The
    // real one cannot be obtained for this profile, so outside this lane
    // `store_transcript_original` has no argument to be called with.
    assert!(AdmittedImport::open(root.path()).is_err());
    Ok(())
}
