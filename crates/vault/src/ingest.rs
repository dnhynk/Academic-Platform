//! Streaming hash/count, no-replace publish, exact dedupe, and read-back sealing.

use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use academic_domain::{
    ArtifactDescriptor, ArtifactId, Confidentiality, ContentDigest, DomainId,
    MAX_SAFE_JSON_INTEGER, MediaType, PermissionLineageId, RetentionClass, VaultLocator,
};
use sha2::{Digest as _, Sha256};

use crate::{
    SealDisposition, SealedArtifactReceipt, VAULT_FORMAT_VERSION, Vault, VaultError, VaultResult,
    durability,
    fault::{self, FaultPoint},
    integrity_mismatch, now_millis,
};

const STREAM_BUFFER_BYTES: usize = 64 * 1024;

/// Immutable policy and identity supplied before streaming begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactIngestRequest {
    artifact_id: ArtifactId,
    media_type: MediaType,
    domain_id: DomainId,
    confidentiality: Confidentiality,
    retention_class: RetentionClass,
    permission_lineage_id: PermissionLineageId,
}

impl ArtifactIngestRequest {
    /// Creates a request whose digest, length, and HMAC locator will be derived while streaming.
    #[must_use]
    pub const fn new(
        artifact_id: ArtifactId,
        media_type: MediaType,
        domain_id: DomainId,
        confidentiality: Confidentiality,
        retention_class: RetentionClass,
        permission_lineage_id: PermissionLineageId,
    ) -> Self {
        Self {
            artifact_id,
            media_type,
            domain_id,
            confidentiality,
            retention_class,
            permission_lineage_id,
        }
    }

    /// Returns the requested artifact ID.
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }

    fn descriptor(
        &self,
        content_digest: ContentDigest,
        byte_length: u64,
        vault_locator: VaultLocator,
    ) -> ArtifactDescriptor {
        ArtifactDescriptor {
            id: self.artifact_id,
            content_digest,
            media_type: self.media_type.clone(),
            byte_length,
            domain_id: self.domain_id,
            confidentiality: self.confidentiality,
            retention_class: self.retention_class,
            permission_lineage_id: self.permission_lineage_id,
            format_version: VAULT_FORMAT_VERSION,
            vault_locator,
            evidence_representations: Vec::new(),
        }
    }
}

pub(crate) fn ingest(
    vault: &Vault,
    request: &ArtifactIngestRequest,
    mut source: impl Read,
) -> VaultResult<SealedArtifactReceipt> {
    let (temp_path, mut temp) = create_temp(vault)?;
    fault::trip(FaultPoint::V01);

    let mut hasher = Sha256::new();
    let mut byte_length = 0_u64;
    let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
    let mut first_write = true;
    loop {
        let read = source.read(&mut buffer).map_err(|source_error| {
            VaultError::io("read artifact source", &temp_path, source_error)
        })?;
        if read == 0 {
            break;
        }
        temp.file_mut()
            .write_all(&buffer[..read])
            .map_err(|source_error| {
                VaultError::io("stream artifact into vault temp", &temp_path, source_error)
            })?;
        hasher.update(&buffer[..read]);
        byte_length = byte_length
            .checked_add(u64::try_from(read).map_err(|_| VaultError::ArtifactTooLarge)?)
            .ok_or(VaultError::ArtifactTooLarge)?;
        if byte_length > MAX_SAFE_JSON_INTEGER {
            return Err(VaultError::Domain(
                academic_domain::DomainError::InvalidEventPayload(
                    "artifact byte length exceeds the portable exact-integer range".to_owned(),
                ),
            ));
        }
        if first_write {
            first_write = false;
            fault::trip(FaultPoint::V02);
        }
    }

    temp.file_mut()
        .flush()
        .and_then(|()| temp.file_mut().sync_all())
        .map_err(|source_error| {
            VaultError::io("synchronize streamed vault temp", &temp_path, source_error)
        })?;
    fault::trip(FaultPoint::V03);

    let content_digest = ContentDigest::from_sha256_bytes(hasher.finalize().into());
    let key = vault.keyring.get(request.domain_id)?;
    let locator = VaultLocator::derive(
        key,
        VAULT_FORMAT_VERSION,
        &request.media_type,
        content_digest,
    )?;
    let descriptor = request.descriptor(content_digest, byte_length, locator);
    descriptor.validate()?;
    let object_path = vault.layout.ensure_object_parent(&descriptor)?;
    let object_parent = object_path
        .parent()
        .ok_or_else(|| VaultError::UnsafeEntry(object_path.clone()))?;
    durability::sync_directory(object_parent)?;
    fault::trip(FaultPoint::V04);

    let published = durability::publish_locked_no_replace(&mut temp, &temp_path, &object_path)?;
    if published {
        fault::trip(FaultPoint::V05);
        durability::sync_directory(object_parent)?;
        durability::sync_directory(vault.layout.temp_dir())?;
        drop(temp);
        verify_object(&object_path, content_digest, byte_length)?;
        let receipt =
            SealedArtifactReceipt::new(descriptor, object_path, SealDisposition::PublishedNew);
        fault::trip(FaultPoint::V06);
        return Ok(receipt);
    }

    match verify_object(&object_path, content_digest, byte_length) {
        Ok(()) => {}
        Err(VaultError::IntegrityMismatch(_)) => {
            return Err(VaultError::PathCollision(object_path));
        }
        Err(error) => return Err(error),
    }
    drop(temp);
    match fs::remove_file(&temp_path) {
        Ok(()) => {}
        Err(source_error) if source_error.kind() == io::ErrorKind::NotFound => {}
        Err(source_error) => {
            return Err(VaultError::io(
                "remove deduplicated vault temp",
                &temp_path,
                source_error,
            ));
        }
    }
    durability::sync_directory(vault.layout.temp_dir())?;
    let receipt =
        SealedArtifactReceipt::new(descriptor, object_path, SealDisposition::AdoptedExisting);
    fault::trip(FaultPoint::V06);
    Ok(receipt)
}

fn create_temp(vault: &Vault) -> VaultResult<(PathBuf, durability::LockedTemp)> {
    for _ in 0..16 {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|_| VaultError::EntropyUnavailable)?;
        let session = format!("{:013}-{}", now_millis()?, crate::encode_hex(&random));
        let path = vault.layout.temp_dir().join(format!("{session}.partial"));
        match durability::create_locked_temp(&path) {
            Ok(file) => return Ok((path, file)),
            Err(VaultError::Io { source, .. }) if source.kind() == io::ErrorKind::AlreadyExists => {
            }
            Err(error) => return Err(error),
        }
    }
    Err(VaultError::EntropyUnavailable)
}

pub(crate) fn verify_object(
    path: &Path,
    expected_digest: ContentDigest,
    expected_length: u64,
) -> VaultResult<()> {
    let metadata = match durability::symlink_metadata_no_follow(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(integrity_mismatch(path));
        }
        Err(source) => return Err(VaultError::io("inspect sealed vault object", path, source)),
    };
    if !metadata.file_type().is_file()
        || crate::layout::is_link_or_reparse(&metadata)
        || metadata.len() != expected_length
    {
        return Err(integrity_mismatch(path));
    }
    let file = durability::open_readonly_no_follow(path)?;
    let opened_metadata = file
        .metadata()
        .map_err(|source| VaultError::io("inspect opened sealed vault object", path, source))?;
    if !opened_metadata.file_type().is_file()
        || crate::layout::is_link_or_reparse(&opened_metadata)
        || opened_metadata.len() != expected_length
    {
        return Err(integrity_mismatch(path));
    }
    let (actual_digest, actual_length) = hash_reader(file, path)?;
    if actual_digest != expected_digest || actual_length != expected_length {
        return Err(integrity_mismatch(path));
    }
    Ok(())
}

pub(crate) fn hash_reader(mut source: impl Read, path: &Path) -> VaultResult<(ContentDigest, u64)> {
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| VaultError::io("hash vault object read-back", path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        length = length
            .checked_add(u64::try_from(read).map_err(|_| VaultError::ArtifactTooLarge)?)
            .ok_or(VaultError::ArtifactTooLarge)?;
    }
    Ok((
        ContentDigest::from_sha256_bytes(hasher.finalize().into()),
        length,
    ))
}
