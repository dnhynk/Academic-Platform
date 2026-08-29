#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_vault::{DomainKeyring, SealDisposition, Vault};
use synthetic_artifacts::{
    DOMAIN_ID, DOMAIN_KEY, SAMPLE_BYTES, SECOND_DOMAIN_ID, SECOND_DOMAIN_KEY, SyntheticTestRoot,
    create_private_test_root, ingest_request,
};

/// Ordinary caller spellings of one profile root.
///
/// On Windows a forward slash is a legal separator everywhere except inside the `\\?\` verbatim
/// namespace, so configuration text, a command-line argument, and anything that builds a path as
/// a string all reach the vault in these shapes. On Unix only the trailing-separator spelling
/// differs, which is exactly the point: the same rewrite must leave the Unix path alone.
fn respellings(root: &Path) -> Vec<PathBuf> {
    let native = root.to_string_lossy().into_owned();
    let mut spellings = vec![PathBuf::from(native.clone())];
    if native.contains('\\') {
        spellings.push(PathBuf::from(native.replace('\\', "/")));
        spellings.push(PathBuf::from(native.replacen('\\', "/", 1)));
    }
    spellings.push(PathBuf::from(format!(
        "{native}{}",
        std::path::MAIN_SEPARATOR
    )));
    spellings
}

fn open_vault(profile_root: &Path) -> Result<Vault, Box<dyn Error>> {
    let mut keyring = DomainKeyring::new();
    keyring.insert(DOMAIN_ID.parse()?, DOMAIN_KEY)?;
    keyring.insert(SECOND_DOMAIN_ID.parse()?, SECOND_DOMAIN_KEY)?;
    Ok(Vault::open(profile_root, keyring)?)
}

#[test]
fn every_caller_separator_spelling_reaches_the_same_vault_object() -> Result<(), Box<dyn Error>> {
    let root = SyntheticTestRoot::new("separator-spelling")?;
    create_private_test_root(root.path())?;

    let mut suffixes = Vec::new();
    for spelling in respellings(root.path()) {
        let vault = open_vault(&spelling)?;
        let receipt = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
        let expected = if suffixes.is_empty() {
            SealDisposition::PublishedNew
        } else {
            SealDisposition::AdoptedExisting
        };
        assert_eq!(
            receipt.disposition(),
            expected,
            "ingest through {}",
            spelling.display()
        );
        assert_eq!(fs::read(receipt.object_path())?, SAMPLE_BYTES);

        let normalized_root = spelling.to_string_lossy().replace('\\', "/");
        let normalized_object = receipt.object_path().to_string_lossy().replace('\\', "/");
        let suffix = normalized_object
            .strip_prefix(normalized_root.trim_end_matches('/'))
            .ok_or("the published object left the profile root")?
            .to_owned();
        suffixes.push(suffix);
    }

    assert!(
        suffixes.windows(2).all(|pair| pair[0] == pair[1]),
        "every spelling must address one object namespace, got {suffixes:?}"
    );
    Ok(())
}
