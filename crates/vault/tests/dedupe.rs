#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{error::Error, fs};

use academic_domain::RetentionClass;
use academic_vault::{SealDisposition, VaultError};
use synthetic_artifacts::{
    ARTIFACT_ID, DOMAIN_ID, PERMISSION_LINEAGE_ID, SAMPLE_BYTES, SECOND_ARTIFACT_ID,
    SECOND_DOMAIN_ID, SECOND_PERMISSION_LINEAGE_ID, ingest_request, open_test_vault, request_with,
};

#[test]
fn dedupe_requires_exact_policy_lineage() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("exact-policy-dedupe")?;
    let first = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let exact_retry = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let different_lineage = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            SECOND_PERMISSION_LINEAGE_ID,
        )?,
        SAMPLE_BYTES,
    )?;
    let different_retention = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::LegalHold,
            PERMISSION_LINEAGE_ID,
        )?,
        SAMPLE_BYTES,
    )?;

    assert_eq!(first.disposition(), SealDisposition::PublishedNew);
    assert_eq!(exact_retry.disposition(), SealDisposition::AdoptedExisting);
    assert_eq!(first.object_path(), exact_retry.object_path());
    assert_eq!(
        different_lineage.disposition(),
        SealDisposition::PublishedNew
    );
    assert_eq!(
        different_retention.disposition(),
        SealDisposition::PublishedNew
    );
    assert_ne!(first.object_path(), different_lineage.object_path());
    assert_ne!(first.object_path(), different_retention.object_path());
    Ok(())
}

#[test]
fn cross_domain_dedupe_is_rejected() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("cross-domain-dedupe")?;
    let first = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let second = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            SECOND_DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        SAMPLE_BYTES,
    )?;

    assert_eq!(second.disposition(), SealDisposition::PublishedNew);
    assert_ne!(first.object_path(), second.object_path());
    assert_ne!(
        first.descriptor().vault_locator,
        second.descriptor().vault_locator
    );
    Ok(())
}

#[test]
fn path_collision_never_overwrites_object() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("collision-no-overwrite")?;
    let request = request_with(
        ARTIFACT_ID,
        DOMAIN_ID,
        RetentionClass::UserManaged,
        PERMISSION_LINEAGE_ID,
    )?;
    let first = vault.ingest(&request, SAMPLE_BYTES)?;
    let collision_bytes = b"occupied locator with different bytes";
    fs::write(first.object_path(), collision_bytes)?;

    let result = vault.ingest(&request, SAMPLE_BYTES);
    assert!(matches!(result, Err(VaultError::PathCollision(_))));
    assert_eq!(fs::read(first.object_path())?, collision_bytes);
    Ok(())
}
