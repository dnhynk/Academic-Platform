//! The `P2-K3` named acceptance suite for `AEAD_CHUNKED_V2`.
//!
//! Every test here is one of the eleven rows t068 §5 names for this task, under
//! the exact name it gives. The `OB` crash matrix lives in `encrypted_crash.rs`
//! because it needs a child process.

#![cfg(feature = "aead-objects")]

#[path = "../../test-support/src/encrypted_artifacts.rs"]
mod encrypted_artifacts;
#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{
    error::Error,
    fs,
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    path::Path,
};

use academic_domain::{
    ArtifactDescriptor, Confidentiality, ContentDigest, MediaType, RetentionClass,
};
use academic_vault::{
    ENCRYPTED_FORMAT_VERSION, EncryptedVault, SealDisposition, SealedObjectVerifier, Vault,
    VaultError,
    object::{self, HEADER_BYTES, ObjectFormatError, TAG_BYTES},
};
use encrypted_artifacts::{
    create_master, deterministic_bytes, open_encrypted_vault, unlock_master,
};
use synthetic_artifacts::{
    ARTIFACT_ID, DOMAIN_ID, PERMISSION_LINEAGE_ID, SECOND_ARTIFACT_ID, SECOND_DOMAIN_ID,
    SECOND_PERMISSION_LINEAGE_ID, SyntheticTestRoot, create_private_test_root, request_with,
};

/// Small enough that a few kilobytes span many chunks.
const TEST_CHUNK_SIZE: u32 = 64;

/// Returns a live root guard and the encrypted vault rooted in it. The guard
/// must stay alive: dropping it removes the disposable profile.
fn open(
    label: &str,
    chunk_size: u32,
) -> Result<(SyntheticTestRoot, EncryptedVault), Box<dyn Error>> {
    let root = SyntheticTestRoot::new(label)?;
    create_private_test_root(root.path())?;
    let (master, _record) = create_master()?;
    let vault = open_encrypted_vault(
        root.path(),
        &master,
        &[DOMAIN_ID, SECOND_DOMAIN_ID],
        chunk_size,
    )?;
    Ok((root, vault))
}

/// Reads one object end to end, restoring the typed vault error.
///
/// A chunk-level failure reaches a `Read` caller as an `io::Error` carrying the
/// `VaultError` as its source, so a test can still tell an authentication
/// failure from a short read.
fn read_all(
    vault: &EncryptedVault,
    descriptor: &ArtifactDescriptor,
) -> Result<Vec<u8>, VaultError> {
    let mut reader = vault.open_reader(descriptor)?;
    let mut plaintext = Vec::new();
    match reader.read_to_end(&mut plaintext) {
        Ok(_) => Ok(plaintext),
        Err(error) => Err(into_vault_error(error)),
    }
}

fn into_vault_error(error: std::io::Error) -> VaultError {
    match error
        .into_inner()
        .map(|source| source.downcast::<VaultError>())
    {
        Some(Ok(typed)) => *typed,
        Some(Err(other)) => VaultError::UnsafeEntry(std::path::PathBuf::from(other.to_string())),
        None => VaultError::UnsafeEntry(std::path::PathBuf::from("<io error with no source>")),
    }
}

/// Overwrites `length` bytes at `offset` in a sealed object.
fn patch(path: &Path, offset: u64, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut file = fs::OpenOptions::new().write(true).open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 0. a destroyed key slot survives the rotation its row cannot follow
// ---------------------------------------------------------------------------

/// A crypto-shredded object is still resolvable under a generation that cannot
/// derive its locator — and nothing else is.
///
/// A locator is a function of `KEK_d` and a shred destroys the only copy of the
/// object's DEK, so a shredded object can never be re-sealed and its descriptor
/// keeps the locator of the generation it was destroyed under while every other
/// descriptor moves. `validate_descriptor_locator` re-derives that locator and
/// refuses before it reads a byte, which is right for a live object and would
/// make a profile that ever deleted an artifact permanently un-backupable.
///
/// What stands in for the keyed check is the marker plus the whole cleartext
/// identity, and the three refusals below are what keep that from being a way
/// to copy anything a descriptor points at.
#[test]
fn a_shredded_object_resolves_under_a_generation_that_cannot_derive_its_locator()
-> Result<(), Box<dyn Error>> {
    let root = SyntheticTestRoot::new("shredded-across-generations")?;
    create_private_test_root(root.path())?;
    let (master, _record) = create_master()?;
    let vault = open_encrypted_vault(
        root.path(),
        &master,
        &[DOMAIN_ID, SECOND_DOMAIN_ID],
        TEST_CHUNK_SIZE,
    )?;

    let receipt = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        deterministic_bytes(200).as_slice(),
    )?;
    let shredded = receipt.descriptor().clone();
    drop(receipt);
    let live = vault
        .ingest(
            &request_with(
                SECOND_ARTIFACT_ID,
                DOMAIN_ID,
                RetentionClass::UserManaged,
                PERMISSION_LINEAGE_ID,
            )?,
            deterministic_bytes(300).as_slice(),
        )?
        .descriptor()
        .clone();
    vault.shred_key_slot(&shredded, &[0x5a; 32])?;

    // A second generation over the same tree: a rotation that has moved the
    // keyring but could not move this row.
    let (rotated_master, _rotated_record) = create_master()?;
    let rotated = open_encrypted_vault(
        root.path(),
        &rotated_master,
        &[DOMAIN_ID, SECOND_DOMAIN_ID],
        TEST_CHUNK_SIZE,
    )?;
    assert!(matches!(
        rotated.verify_sealed_object(&shredded),
        Err(VaultError::LocatorMismatch(_))
    ));
    assert_eq!(
        rotated.verify_shredded_object(&shredded)?,
        rotated.layout().object_path(&shredded)?
    );

    // The object that was never shredded is refused as the mismatch it is,
    // under both vaults.
    assert!(matches!(
        rotated.verify_shredded_object(&live),
        Err(VaultError::LocatorMismatch(_))
    ));
    assert!(matches!(
        vault.verify_shredded_object(&live),
        Err(VaultError::LocatorMismatch(_))
    ));

    // The path is a function of the domain, the retention class, the lineage,
    // and the locator, so those four cannot disagree with the header of the
    // file the path reaches. The artifact identity and the plaintext length can,
    // and the cleartext identity check is what refuses them.
    let mut relabelled = shredded.clone();
    relabelled.id = live.id;
    assert!(matches!(
        rotated.verify_shredded_object(&relabelled),
        Err(VaultError::ObjectFormat(
            ObjectFormatError::IdentityMismatch("artifact_id")
        ))
    ));
    let mut relengthed = shredded.clone();
    relengthed.byte_length = shredded.byte_length + 1;
    assert!(matches!(
        rotated.verify_shredded_object(&relengthed),
        Err(VaultError::ObjectFormat(
            ObjectFormatError::IdentityMismatch("plaintext_len")
        ))
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// 1. object_header_tag_binds_identity_and_domain
// ---------------------------------------------------------------------------

#[test]
fn object_header_tag_binds_identity_and_domain() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open("header-binds-identity", TEST_CHUNK_SIZE)?;
    let bytes = deterministic_bytes(200);
    let receipt = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let descriptor = receipt.descriptor().clone();
    let path = receipt.object_path().to_path_buf();
    drop(receipt);

    // The header authenticates every identity field. Flipping one bit in each
    // of them must fail the wrap tag rather than yield a differently-labelled
    // object. Offsets come from the frozen layout in `object.rs`.
    let identity_offsets: [(&str, u64); 6] = [
        ("artifact_id", 13),
        ("domain_id", 29),
        ("retention_class", 45),
        ("permission_lineage_id", 46),
        ("locator", 86),
        ("plaintext_len", 118),
    ];
    let original = fs::read(&path)?;
    for (field, offset) in identity_offsets {
        let mut flipped = original[offset as usize];
        flipped ^= 0x01;
        patch(&path, offset, &[flipped])?;

        let result = read_all(&vault, &descriptor);
        assert!(
            matches!(
                result,
                Err(VaultError::ObjectFormat(
                    ObjectFormatError::Aead
                        | ObjectFormatError::MalformedHeader(_)
                        | ObjectFormatError::IdentityMismatch(_)
                ))
            ),
            "tampering with {field} produced {result:?} instead of a header failure"
        );

        // Restoring the byte makes the same read succeed again, so the check
        // is the tag and not some unrelated refusal.
        patch(&path, offset, &[original[offset as usize]])?;
        assert_eq!(read_all(&vault, &descriptor)?, bytes);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. chunk_aad_binds_index_and_finality
// ---------------------------------------------------------------------------

#[test]
fn chunk_aad_binds_index_and_finality() -> Result<(), Box<dyn Error>> {
    // Two chunks of identical plaintext. If the index were not AAD they would
    // be byte-identical and interchangeable; they must not be.
    let repeated = vec![0x5A_u8; usize::try_from(TEST_CHUNK_SIZE)? * 2];
    let (_root, vault) = open("chunk-aad-binds", TEST_CHUNK_SIZE)?;
    let receipt = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        repeated.as_slice(),
    )?;
    let descriptor = receipt.descriptor().clone();
    let path = receipt.object_path().to_path_buf();
    drop(receipt);

    let sealed = fs::read(&path)?;
    let record = usize::try_from(TEST_CHUNK_SIZE)? + TAG_BYTES;
    let first = &sealed[HEADER_BYTES..HEADER_BYTES + record];
    let second = &sealed[HEADER_BYTES + record..HEADER_BYTES + 2 * record];
    assert_ne!(
        first, second,
        "two chunks of identical plaintext sealed to identical bytes"
    );

    // Finality is AAD too: the last chunk's `is_final` byte is 1 and every
    // other chunk's is 0, so a reader that mistook one for the other fails.
    let header = vault.open_reader(&descriptor)?;
    assert_eq!(header.header().chunk_count(), 2);
    assert_eq!(header.header().chunk_aad(0)[44], 0);
    assert_eq!(header.header().chunk_aad(1)[44], 1);
    assert_ne!(header.header().chunk_aad(0), header.header().chunk_aad(1));
    drop(header);

    // Swapping the two chunks keeps the file length and the header intact, so
    // only the AAD can catch it.
    let mut swapped = sealed.clone();
    swapped[HEADER_BYTES..HEADER_BYTES + record].copy_from_slice(second);
    swapped[HEADER_BYTES + record..HEADER_BYTES + 2 * record].copy_from_slice(first);
    fs::write(&path, &swapped)?;
    assert!(matches!(
        read_all(&vault, &descriptor),
        Err(VaultError::ObjectFormat(ObjectFormatError::Aead))
    ));

    fs::write(&path, &sealed)?;
    assert_eq!(read_all(&vault, &descriptor)?, repeated);
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. truncated_object_is_repair_required
// ---------------------------------------------------------------------------

#[test]
fn truncated_object_is_repair_required() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open("truncated-object", TEST_CHUNK_SIZE)?;
    let bytes = deterministic_bytes(300);
    let receipt = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let descriptor = receipt.descriptor().clone();
    let path = receipt.object_path().to_path_buf();
    drop(receipt);

    let sealed = fs::read(&path)?;
    for cut in [
        sealed.len() - 1,
        sealed.len() - TAG_BYTES,
        HEADER_BYTES + usize::try_from(TEST_CHUNK_SIZE)? + TAG_BYTES,
        HEADER_BYTES,
        HEADER_BYTES - 1,
        0,
    ] {
        let file = fs::OpenOptions::new().write(true).open(&path)?;
        file.set_len(u64::try_from(cut)?)?;
        file.sync_all()?;
        drop(file);

        let result = read_all(&vault, &descriptor);
        assert!(
            result.is_err(),
            "truncating to {cut} bytes still produced plaintext"
        );
        // No partial plaintext escapes, and the verifier reports the object as
        // corrupt rather than absent: repair-required, not "not found".
        let verified = SealedObjectVerifier::verify_sealed_object(&vault, &descriptor);
        assert!(
            matches!(
                verified,
                Err(VaultError::ObjectFormat(_) | VaultError::IntegrityMismatch(_))
            ),
            "truncating to {cut} bytes produced {verified:?}"
        );
    }

    // Restoring the exact bytes makes the same object readable again.
    fs::write(&path, &sealed)?;
    assert_eq!(read_all(&vault, &descriptor)?, bytes);
    SealedObjectVerifier::verify_sealed_object(&vault, &descriptor)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. reordered_chunk_fails_before_plaintext
// ---------------------------------------------------------------------------

#[test]
fn reordered_chunk_fails_before_plaintext() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open("reordered-chunk", TEST_CHUNK_SIZE)?;
    let chunk = usize::try_from(TEST_CHUNK_SIZE)?;
    let bytes = deterministic_bytes(chunk * 4);
    let receipt = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let descriptor = receipt.descriptor().clone();
    let path = receipt.object_path().to_path_buf();
    drop(receipt);

    let sealed = fs::read(&path)?;
    let record = chunk + TAG_BYTES;
    let mut reordered = sealed.clone();
    let first = HEADER_BYTES;
    let third = HEADER_BYTES + 2 * record;
    let mut buffer = vec![0_u8; record];
    buffer.copy_from_slice(&sealed[first..first + record]);
    reordered[first..first + record].copy_from_slice(&sealed[third..third + record]);
    reordered[third..third + record].copy_from_slice(&buffer);
    fs::write(&path, &reordered)?;

    // The reader must fail at the first bad chunk without returning any of the
    // plaintext it could otherwise have produced from chunk 0.
    let mut reader = vault.open_reader(&descriptor)?;
    let mut out = vec![0_u8; chunk];
    let outcome = reader.read(&mut out);
    assert!(outcome.is_err(), "a reordered chunk produced plaintext");
    assert_eq!(out, vec![0_u8; chunk], "the read buffer was written into");
    drop(reader);
    assert!(read_all(&vault, &descriptor).is_err());

    fs::write(&path, &sealed)?;
    assert_eq!(read_all(&vault, &descriptor)?, bytes);
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. spliced_chunk_from_other_object_fails
// ---------------------------------------------------------------------------

#[test]
fn spliced_chunk_from_other_object_fails() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open("spliced-chunk", TEST_CHUNK_SIZE)?;
    let chunk = usize::try_from(TEST_CHUNK_SIZE)?;
    // Identical plaintext in two different artifacts. Only the per-object base
    // nonce and identity separate their chunks.
    let bytes = deterministic_bytes(chunk * 3);

    let donor = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            SECOND_PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let donor_path = donor.object_path().to_path_buf();
    drop(donor);

    let target = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let descriptor = target.descriptor().clone();
    let target_path = target.object_path().to_path_buf();
    drop(target);

    assert_ne!(donor_path, target_path);
    let donor_bytes = fs::read(&donor_path)?;
    let target_bytes = fs::read(&target_path)?;
    let record = chunk + TAG_BYTES;
    assert_ne!(
        donor_bytes[HEADER_BYTES..HEADER_BYTES + record],
        target_bytes[HEADER_BYTES..HEADER_BYTES + record],
        "two objects with identical plaintext produced identical chunk bytes"
    );

    let mut spliced = target_bytes.clone();
    spliced[HEADER_BYTES..HEADER_BYTES + record]
        .copy_from_slice(&donor_bytes[HEADER_BYTES..HEADER_BYTES + record]);
    fs::write(&target_path, &spliced)?;
    assert!(matches!(
        read_all(&vault, &descriptor),
        Err(VaultError::ObjectFormat(ObjectFormatError::Aead))
    ));

    fs::write(&target_path, &target_bytes)?;
    assert_eq!(read_all(&vault, &descriptor)?, bytes);

    // The hard case: a donor that shares every identity field in the header's
    // streaming prefix -- artifact, domain, retention, lineage, chunk size --
    // and differs only in its plaintext, and therefore only in its per-object
    // DEK and base nonce. Nothing but that per-object randomness separates the
    // two, so this is what proves the randomness is load-bearing rather than
    // the identity fields doing the work.
    let twin_bytes = deterministic_bytes(chunk * 3)
        .into_iter()
        .map(|byte| byte ^ 0xff)
        .collect::<Vec<_>>();
    assert_eq!(twin_bytes.len(), bytes.len());
    let twin = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        twin_bytes.as_slice(),
    )?;
    let twin_path = twin.object_path().to_path_buf();
    drop(twin);
    assert_ne!(twin_path, target_path);

    let twin_sealed = fs::read(&twin_path)?;
    // Same header geometry, so the splice is byte-aligned.
    assert_eq!(twin_sealed.len(), target_bytes.len());
    let mut twin_spliced = target_bytes.clone();
    twin_spliced[HEADER_BYTES..HEADER_BYTES + record]
        .copy_from_slice(&twin_sealed[HEADER_BYTES..HEADER_BYTES + record]);
    fs::write(&target_path, &twin_spliced)?;
    assert!(
        matches!(
            read_all(&vault, &descriptor),
            Err(VaultError::ObjectFormat(ObjectFormatError::Aead))
        ),
        "a chunk from an object with identical header identity was accepted"
    );

    fs::write(&target_path, &target_bytes)?;
    assert_eq!(read_all(&vault, &descriptor)?, bytes);
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. wrong_domain_kek_fails_at_header
// ---------------------------------------------------------------------------

#[test]
fn wrong_domain_kek_fails_at_header() -> Result<(), Box<dyn Error>> {
    let root = SyntheticTestRoot::new("wrong-domain-kek")?;
    create_private_test_root(root.path())?;
    let (master, record) = create_master()?;
    let vault = open_encrypted_vault(
        root.path(),
        &master,
        &[DOMAIN_ID, SECOND_DOMAIN_ID],
        TEST_CHUNK_SIZE,
    )?;
    let bytes = deterministic_bytes(150);
    let receipt = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let descriptor = receipt.descriptor().clone();
    let path = receipt.object_path().to_path_buf();
    drop(receipt);

    // Open the object bytes directly under the wrong domain's KEK: this is the
    // header gate itself, with no vault path validation in front of it.
    let header = &fs::read(&path)?[..HEADER_BYTES];
    let right = master.derive_domain_kek(
        encrypted_artifacts::profile_id(),
        academic_crypto::DomainId::from_bytes(
            *DOMAIN_ID.parse::<academic_domain::DomainId>()?.as_bytes(),
        ),
    )?;
    let wrong = master.derive_domain_kek(
        encrypted_artifacts::profile_id(),
        academic_crypto::DomainId::from_bytes(
            *SECOND_DOMAIN_ID
                .parse::<academic_domain::DomainId>()?
                .as_bytes(),
        ),
    )?;
    assert!(object::open_header(header, right.expose_secret()).is_ok());
    assert_eq!(
        object::open_header(header, wrong.expose_secret()).err(),
        Some(ObjectFormatError::Aead),
        "the wrong domain KEK opened the header"
    );

    // A key from a different profile fails the same way: the profile identity
    // is the HKDF salt.
    let other_master = unlock_master(&record)?;
    let other = other_master.derive_domain_kek(
        academic_crypto::ProfileId::from_bytes([0x5F; academic_crypto::IDENTIFIER_BYTES]),
        academic_crypto::DomainId::from_bytes(
            *DOMAIN_ID.parse::<academic_domain::DomainId>()?.as_bytes(),
        ),
    )?;
    assert_eq!(
        object::open_header(header, other.expose_secret()).err(),
        Some(ObjectFormatError::Aead)
    );

    // Through the vault, a domain whose key is absent never reaches the file.
    let mut foreign =
        academic_vault::EncryptedDomainKeyring::new(encrypted_artifacts::profile_id());
    foreign.insert(SECOND_DOMAIN_ID.parse()?, wrong)?;
    let foreign_vault =
        EncryptedVault::open_with_chunk_size(root.path(), foreign, TEST_CHUNK_SIZE)?;
    assert!(matches!(
        foreign_vault.open_reader(&descriptor),
        Err(VaultError::MissingDomainKey(_))
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. seek_in_multi_gb_object_is_exact
// ---------------------------------------------------------------------------

#[test]
fn seek_in_multi_gb_object_is_exact() -> Result<(), Box<dyn Error>> {
    // Two halves, because a hosted runner must not be asked for six gigabytes
    // of disk. The first half is executed byte-for-byte over a real sealed
    // object with many chunks; the second is the multi-gigabyte chunk
    // arithmetic itself, which is a pure function of the header and is what
    // actually has to survive a 64-bit offset.
    let (_root, vault) = open("seek-exact", TEST_CHUNK_SIZE)?;
    let chunk = usize::try_from(TEST_CHUNK_SIZE)?;
    let bytes = deterministic_bytes(chunk * 40 + 7);
    let receipt = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let descriptor = receipt.descriptor().clone();
    drop(receipt);

    let mut reader = vault.open_reader(&descriptor)?;
    assert_eq!(reader.plaintext_len(), u64::try_from(bytes.len())?);
    for offset in [
        0_usize,
        1,
        chunk - 1,
        chunk,
        chunk + 1,
        chunk * 17 + 3,
        chunk * 40,
        bytes.len() - 1,
    ] {
        reader.seek(SeekFrom::Start(u64::try_from(offset)?))?;
        let mut out = vec![0_u8; (bytes.len() - offset).min(chunk * 2 + 5)];
        reader.read_exact(&mut out)?;
        assert_eq!(out, bytes[offset..offset + out.len()], "seek to {offset}");
    }

    // Seeking from the end and from the current position resolve identically.
    reader.seek(SeekFrom::End(-1))?;
    let mut last = [0_u8; 1];
    reader.read_exact(&mut last)?;
    assert_eq!(last[0], bytes[bytes.len() - 1]);
    reader.seek(SeekFrom::Start(10))?;
    reader.seek(SeekFrom::Current(5))?;
    let mut here = [0_u8; 1];
    reader.read_exact(&mut here)?;
    assert_eq!(here[0], bytes[15]);

    // Reading past the end yields nothing rather than an error or a wrap.
    reader.seek(SeekFrom::End(0))?;
    let mut past = [0_u8; 8];
    assert_eq!(reader.read(&mut past)?, 0);
    drop(reader);

    // The multi-gigabyte half: chunk index, offset, and file position for a
    // 6 GiB object at the default 1 MiB chunk size. NOT EXECUTED against a
    // real 6 GiB file; this is the arithmetic a seek into one resolves through.
    let six_gib = 6_u64 * 1024 * 1024 * 1024;
    let default_chunk = u64::from(object::DEFAULT_CHUNK_SIZE);
    let header = object::ObjectHeader::geometry(object::DEFAULT_CHUNK_SIZE, six_gib);
    assert_eq!(header.chunk_count(), six_gib / default_chunk);
    let last_index = header.chunk_count() - 1;
    assert_eq!(
        header.chunk_plaintext_len(last_index),
        object::DEFAULT_CHUNK_SIZE
    );
    assert_eq!(
        header.chunk_offset(last_index),
        u64::try_from(HEADER_BYTES)? + last_index * (default_chunk + u64::try_from(TAG_BYTES)?)
    );
    assert_eq!(
        header.sealed_len(),
        u64::try_from(HEADER_BYTES)? + six_gib + header.chunk_count() * u64::try_from(TAG_BYTES)?
    );
    // The 6 GiB file position exceeds u32 and must not have wrapped.
    assert!(header.chunk_offset(last_index) > u64::from(u32::MAX));
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. zero_byte_and_small_object_vectors
// ---------------------------------------------------------------------------

#[test]
fn zero_byte_and_small_object_vectors() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open("small-object-vectors", TEST_CHUNK_SIZE)?;
    let chunk = usize::try_from(TEST_CHUNK_SIZE)?;
    let identities = [
        ARTIFACT_ID,
        SECOND_ARTIFACT_ID,
        "01900000-0000-7000-8000-000000000103",
        "01900000-0000-7000-8000-000000000104",
        "01900000-0000-7000-8000-000000000105",
        "01900000-0000-7000-8000-000000000106",
    ];
    let lineages = [
        PERMISSION_LINEAGE_ID,
        SECOND_PERMISSION_LINEAGE_ID,
        "01900000-0000-7000-8000-000000000303",
        "01900000-0000-7000-8000-000000000304",
        "01900000-0000-7000-8000-000000000305",
        "01900000-0000-7000-8000-000000000306",
    ];
    // 0, 1, one byte short of a chunk, exactly a chunk, one past a chunk, and
    // an exact multiple. The exact-multiple cases are the ones a naive writer
    // gets wrong: the last full chunk is the final chunk, and there is no
    // trailing empty one.
    for (index, length) in [0, 1, chunk - 1, chunk, chunk + 1, chunk * 3]
        .into_iter()
        .enumerate()
    {
        let bytes = deterministic_bytes(length);
        let receipt = vault.ingest(
            &request_with(
                identities[index],
                DOMAIN_ID,
                RetentionClass::UserManaged,
                lineages[index],
            )?,
            bytes.as_slice(),
        )?;
        let descriptor = receipt.descriptor().clone();
        assert_eq!(receipt.disposition(), SealDisposition::PublishedNew);
        assert_eq!(descriptor.byte_length, u64::try_from(length)?);
        assert_eq!(descriptor.content_digest, ContentDigest::sha256(&bytes));
        assert_eq!(descriptor.format_version, ENCRYPTED_FORMAT_VERSION);

        let sealed_len = fs::metadata(receipt.object_path())?.len();
        drop(receipt);

        let reader = vault.open_reader(&descriptor)?;
        let expected_chunks = if length == 0 {
            1
        } else {
            u64::try_from(length.div_ceil(chunk))?
        };
        assert_eq!(
            reader.header().chunk_count(),
            expected_chunks,
            "len {length}"
        );
        assert_eq!(reader.header().sealed_len(), sealed_len, "len {length}");
        assert_eq!(
            sealed_len,
            u64::try_from(HEADER_BYTES + length)? + expected_chunks * u64::try_from(TAG_BYTES)?,
            "len {length}"
        );
        drop(reader);

        assert_eq!(read_all(&vault, &descriptor)?, bytes, "len {length}");
        SealedObjectVerifier::verify_sealed_object(&vault, &descriptor)?;

        // A zero-byte object is still an authenticated object, not an empty
        // file: its single final chunk carries a tag.
        if length == 0 {
            assert_eq!(sealed_len, u64::try_from(HEADER_BYTES + TAG_BYTES)?);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. format_n_minus_1_reader_corpus
// ---------------------------------------------------------------------------

#[test]
fn format_n_minus_1_reader_corpus() -> Result<(), Box<dyn Error>> {
    // The committed corpus holds one object of each format plus the frozen
    // byte vectors of the current one. It is read here, never regenerated, so
    // a format change that alters a byte fails this test.
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("testdata")
        .join("aead-chunked-v2");
    let vectors = fs::read_to_string(corpus.join("vectors.txt"))?;
    let sealed = fs::read(corpus.join("format-2.aobj"))?;
    let plaintext_v1 = fs::read(corpus.join("format-1.obj"))?;

    let field = |name: &str| -> Result<Vec<u8>, Box<dyn Error>> {
        let line = vectors
            .lines()
            .find(|line| line.starts_with(&format!("{name}=")))
            .ok_or_else(|| format!("corpus vector {name} is missing"))?;
        let hex = line
            .split_once('=')
            .map(|(_, value)| value.trim())
            .unwrap_or_default();
        decode_hex(hex)
    };

    // N: the current format opens under its committed key and yields the
    // committed plaintext, byte for byte.
    let kek: [u8; 32] = field("domain_kek")?
        .try_into()
        .map_err(|_| "corpus domain_kek is not 32 bytes")?;
    let opened = object::open_header(&sealed, &kek)?;
    assert_eq!(
        opened.header.plaintext_len(),
        u64::try_from(plaintext_v1.len())?
    );

    let mut recovered = Vec::new();
    for index in 0..opened.header.chunk_count() {
        let start = usize::try_from(opened.header.chunk_offset(index))?;
        let length = usize::try_from(opened.header.chunk_plaintext_len(index))? + TAG_BYTES;
        let mut chunk = sealed[start..start + length].to_vec();
        object::open_chunk(opened.expose_dek(), &opened.header, index, &mut chunk)?;
        recovered.extend_from_slice(&chunk);
    }
    assert_eq!(
        recovered, plaintext_v1,
        "the committed format-2 object drifted"
    );
    assert_eq!(
        ContentDigest::sha256(&recovered).as_bytes(),
        opened.plaintext_digest(),
        "the sealed digest no longer describes the sealed plaintext"
    );

    // The frozen header bytes. A layout change moves one of these.
    assert_eq!(&sealed[..4], b"ACOB");
    assert_eq!(sealed[..HEADER_BYTES].to_vec(), field("header")?);
    assert_eq!(
        sealed[..object::WRAP_AAD_BYTES].to_vec(),
        field("wrap_aad")?
    );
    assert_eq!(
        sealed[object::WRAP_AAD_BYTES..HEADER_BYTES].to_vec(),
        field("wrapped_dek")?
    );
    // `header_tag` is the trailing 16 bytes of `wrapped_dek`, not a field.
    assert_eq!(
        sealed[HEADER_BYTES - TAG_BYTES..HEADER_BYTES].to_vec(),
        field("header_tag")?
    );
    // `header_len` counts the header after the eight-byte prefix: 208 on disk,
    // 200 in the field.
    assert_eq!(text_field(&vectors, "header_len_field")?, "200");
    assert_eq!(text_field(&vectors, "header_total_bytes")?, "208");
    assert_eq!(
        u16::from_le_bytes([sealed[6], sealed[7]]),
        object::HEADER_LEN_FIELD
    );

    // The chunk nonce XORs LE64(i + 1) into the trailing eight bytes of the
    // base nonce and touches nothing else. The two committed nonces are what
    // fix that, so a change of position fails here rather than silently
    // producing objects an older reader cannot open.
    let base_nonce = field("base_nonce")?;
    for (index, name) in [(0_u64, "chunk_nonce_0"), (1, "chunk_nonce_1")] {
        let nonce = field(name)?;
        assert_eq!(nonce.len(), object::BASE_NONCE_BYTES);
        assert_eq!(nonce[..16], base_nonce[..16], "{name} touched bytes 0..16");
        let mut expected = base_nonce.clone();
        for (slot, byte) in expected[16..24].iter_mut().zip((index + 1).to_le_bytes()) {
            *slot ^= byte;
        }
        assert_eq!(nonce, expected, "{name}");
    }

    // The zero-length object: exactly one chunk, `len_0 = 0`, `is_final = 1`.
    let empty = fs::read(corpus.join("format-2-empty.aobj"))?;
    assert_eq!(empty, field("empty_sealed_object")?);
    let empty_opened = object::open_header(&empty, &kek)?;
    assert_eq!(empty_opened.header.plaintext_len(), 0);
    assert_eq!(empty_opened.header.chunk_count(), 1);
    assert_eq!(empty_opened.header.chunk_plaintext_len(0), 0);
    assert_eq!(empty_opened.header.chunk_aad(0)[44], 1);
    assert_eq!(
        empty_opened.header.chunk_aad(0).to_vec(),
        field("empty_chunk_aad_0")?
    );
    assert_eq!(
        empty_opened.header.sealed_len(),
        u64::try_from(HEADER_BYTES + TAG_BYTES)?
    );
    assert_eq!(
        u64::try_from(empty.len())?,
        empty_opened.header.sealed_len()
    );
    let mut empty_chunk = empty[HEADER_BYTES..].to_vec();
    object::open_chunk(
        empty_opened.expose_dek(),
        &empty_opened.header,
        0,
        &mut empty_chunk,
    )?;
    assert!(empty_chunk.is_empty());
    assert_eq!(
        ContentDigest::sha256(&[]).as_bytes(),
        empty_opened.plaintext_digest()
    );
    assert_eq!(
        u64::try_from(sealed.len())?,
        opened.header.sealed_len(),
        "the committed object length no longer matches its own header"
    );
    assert_eq!(
        opened.header.chunk_count(),
        2,
        "the corpus lost its second chunk"
    );

    // N-1: the plaintext synthetic object is readable only by the synthetic
    // vault, and the two namespaces cannot reach each other's objects.
    let root = SyntheticTestRoot::new("format-corpus")?;
    create_private_test_root(root.path())?;
    let mut keyring = academic_vault::DomainKeyring::new();
    keyring.insert(DOMAIN_ID.parse()?, synthetic_artifacts::DOMAIN_KEY)?;
    let plaintext_vault = Vault::open(root.path(), keyring)?;
    let v1_receipt = plaintext_vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        plaintext_v1.as_slice(),
    )?;
    let v1_descriptor = v1_receipt.descriptor().clone();
    assert_eq!(v1_descriptor.format_version, 1);
    assert_eq!(fs::read(v1_receipt.object_path())?, plaintext_v1);
    assert_eq!(
        v1_receipt
            .object_path()
            .extension()
            .and_then(|e| e.to_str()),
        Some("obj")
    );
    drop(v1_receipt);

    let (master, _record) = create_master()?;
    let encrypted_vault =
        open_encrypted_vault(root.path(), &master, &[DOMAIN_ID], TEST_CHUNK_SIZE)?;
    let v2_receipt = encrypted_vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        plaintext_v1.as_slice(),
    )?;
    let v2_descriptor = v2_receipt.descriptor().clone();
    assert_eq!(v2_descriptor.format_version, ENCRYPTED_FORMAT_VERSION);
    assert_eq!(
        v2_receipt
            .object_path()
            .extension()
            .and_then(|e| e.to_str()),
        Some("aobj")
    );
    drop(v2_receipt);

    // A format-1 descriptor has no path in the encrypted namespace, and a
    // format-2 descriptor has none in the synthetic one. Neither reader can be
    // pointed at the other format's object.
    assert!(matches!(
        SealedObjectVerifier::verify_sealed_object(&encrypted_vault, &v1_descriptor),
        Err(VaultError::LocatorMismatch(_))
    ));
    assert!(matches!(
        SealedObjectVerifier::verify_sealed_object(&plaintext_vault, &v2_descriptor),
        Err(VaultError::LocatorMismatch(_))
    ));
    // Both objects remain readable in their own namespace at the same time.
    assert_eq!(
        fs::read(
            SealedObjectVerifier::verify_sealed_object(&plaintext_vault, &v1_descriptor)?
                .object_path()
        )?,
        plaintext_v1
    );
    assert_eq!(read_all(&encrypted_vault, &v2_descriptor)?, plaintext_v1);
    Ok(())
}

fn text_field<'vectors>(
    vectors: &'vectors str,
    name: &str,
) -> Result<&'vectors str, Box<dyn Error>> {
    vectors
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}=")))
        .map(str::trim)
        .ok_or_else(|| format!("corpus vector {name} is missing").into())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if !value.len().is_multiple_of(2) {
        return Err("odd-length hex in the committed corpus".into());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks(2) {
        let text = std::str::from_utf8(pair)?;
        bytes.push(u8::from_str_radix(text, 16)?);
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// 10. cross_policy_dedupe_still_rejected
// ---------------------------------------------------------------------------

#[test]
fn cross_policy_dedupe_still_rejected() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open("cross-policy-dedupe", TEST_CHUNK_SIZE)?;
    let bytes = deterministic_bytes(120);

    let first = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let retry = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    // Exact policy: same domain, retention, lineage, media type, and bytes.
    assert_eq!(first.disposition(), SealDisposition::PublishedNew);
    assert_eq!(retry.disposition(), SealDisposition::AdoptedExisting);
    assert_eq!(first.object_path(), retry.object_path());

    let other_lineage = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            SECOND_PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let other_retention = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::LegalHold,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let other_domain = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            SECOND_DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;

    // Nothing crosses a policy boundary: three separate objects, three separate
    // paths, and no adoption.
    for other in [&other_lineage, &other_retention, &other_domain] {
        assert_eq!(other.disposition(), SealDisposition::PublishedNew);
        assert_ne!(other.object_path(), first.object_path());
    }

    // The locator is domain-keyed, so the same bytes in a different domain do
    // not even land on the same filename. This is what keeps a directory
    // listing from exposing global plaintext equality.
    assert_ne!(
        other_domain.descriptor().vault_locator,
        first.descriptor().vault_locator,
        "the locator became globally comparable"
    );
    assert_eq!(
        other_lineage.descriptor().vault_locator,
        first.descriptor().vault_locator,
        "a same-domain locator stopped being a function of the bytes"
    );

    // And the ciphertext of two objects with identical plaintext differs, so
    // convergent dedupe is not reachable by comparing files either.
    assert_ne!(
        fs::read(first.object_path())?,
        fs::read(other_lineage.object_path())?
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 11. encrypted_vault_implements_sealed_verifier_without_bypass
// ---------------------------------------------------------------------------

#[test]
fn encrypted_vault_implements_sealed_verifier_without_bypass() -> Result<(), Box<dyn Error>> {
    fn seam_only<V: SealedObjectVerifier>(
        verifier: &V,
        descriptor: &ArtifactDescriptor,
    ) -> Result<V::Receipt, VaultError> {
        // Everything a canonical writer may do with a vault. There is no method
        // here that returns bytes, a digest, or a path it did not read back.
        assert!(verifier.profile_root().is_absolute());
        verifier.verify_sealed_object(descriptor)
    }

    let (_root, vault) = open("sealed-verifier-seam", TEST_CHUNK_SIZE)?;
    let bytes = deterministic_bytes(90);
    let receipt = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let descriptor = receipt.descriptor().clone();
    let path = receipt.object_path().to_path_buf();
    drop(receipt);

    // The same generic code drives the plaintext vault and the encrypted one.
    let mut sealed = seam_only(&vault, &descriptor)?;
    assert_eq!(sealed.descriptor(), &descriptor);
    assert_eq!(sealed.disposition(), SealDisposition::AdoptedExisting);
    SealedObjectVerifier::revalidate_sealed_object(&vault, &mut sealed)?;

    // A descriptor whose declared digest does not describe the sealed bytes is
    // refused, so the store cannot register a descriptor the vault did not
    // verify. This is the byte bypass the seam exists to prevent.
    let mut lying = descriptor.clone();
    lying.content_digest = ContentDigest::sha256(b"different bytes");
    assert!(seam_only(&vault, &lying).is_err());

    let mut wrong_length = descriptor.clone();
    wrong_length.byte_length += 1;
    assert!(seam_only(&vault, &wrong_length).is_err());

    let mut wrong_media = descriptor.clone();
    wrong_media.media_type = MediaType::parse("text/plain")?;
    assert!(seam_only(&vault, &wrong_media).is_err());

    let mut wrong_confidentiality = descriptor.clone();
    wrong_confidentiality.confidentiality = Confidentiality::Public;
    // Confidentiality is not part of the locator, so this one must still
    // verify: the seam does not silently accept a descriptor field it never
    // checks, it accepts exactly the ones the object binds.
    seam_only(&vault, &wrong_confidentiality)?;

    // Mutating the object under a live receipt is caught at revalidation, which
    // is the last gate before a canonical commit.
    let original = fs::read(&path)?;
    let mut corrupted = original.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0x01;
    fs::write(&path, &corrupted)?;
    assert!(SealedObjectVerifier::revalidate_sealed_object(&vault, &mut sealed).is_err());
    fs::write(&path, &original)?;

    // The receipt exposes only a descriptor, a path, and a disposition; the
    // trait has no accessor for plaintext or key material.
    Ok(())
}
