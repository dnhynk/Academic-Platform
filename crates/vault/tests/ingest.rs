#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{error::Error, fs};

use academic_vault::{SealDisposition, SealedArtifactReceipt};
use synthetic_artifacts::{SAMPLE_BYTES, ingest_request, open_test_vault};

#[test]
fn sealed_before_reference_type_state() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("sealed-before-reference")?;
    let request = ingest_request()?;
    let receipt = vault.ingest(&request, SAMPLE_BYTES)?;

    assert_eq!(receipt.descriptor().id, request.artifact_id());
    assert_eq!(
        receipt.descriptor().content_digest,
        academic_domain::ContentDigest::sha256(SAMPLE_BYTES)
    );
    assert_eq!(receipt.disposition(), SealDisposition::PublishedNew);
    assert_eq!(fs::read(receipt.object_path())?, SAMPLE_BYTES);

    let verified: SealedArtifactReceipt = vault.verify_sealed_object(receipt.descriptor())?;
    assert_eq!(verified.descriptor(), receipt.descriptor());
    assert_eq!(verified.disposition(), SealDisposition::AdoptedExisting);
    Ok(())
}

#[test]
fn hmac_locator_is_not_plain_digest() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("keyed-locator")?;
    let receipt = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let descriptor = receipt.descriptor();

    assert_ne!(
        descriptor.vault_locator.as_bytes(),
        descriptor.content_digest.as_bytes()
    );
    let path_text = receipt.object_path().to_string_lossy();
    let digest_hex = descriptor.content_digest.to_string();
    let raw_digest = digest_hex
        .strip_prefix("sha256:")
        .ok_or("digest lost its canonical prefix")?;
    assert!(!path_text.contains(raw_digest));
    assert!(path_text.contains(&descriptor.domain_id.to_string()));
    assert!(path_text.contains(&descriptor.permission_lineage_id.to_string()));
    assert!(path_text.contains("USER_MANAGED"));
    Ok(())
}
