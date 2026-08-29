//! Streaming hash/count, no-replace publish, exact dedupe, and read-back sealing.

use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
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

/// Live exact-object evidence retained by an opaque sealed capability.
#[derive(Debug)]
pub(crate) struct LiveObjectEvidence {
    file: File,
    identity: durability::ObjectIdentity,
    _lease: durability::SharedObjectLease,
}

impl LiveObjectEvidence {
    pub(crate) fn revalidate(
        &mut self,
        path: &Path,
        expected_digest: ContentDigest,
        expected_length: u64,
    ) -> VaultResult<()> {
        if durability::object_identity(&self.file, path)? != self.identity {
            return Err(integrity_mismatch(path));
        }
        verify_open_file(&mut self.file, path, expected_digest, expected_length)?;

        let (_canonical_file, canonical_identity) =
            open_exact_object(path, expected_digest, expected_length)?;
        if canonical_identity != self.identity {
            return Err(integrity_mismatch(path));
        }
        Ok(())
    }
}

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

    /// Returns the requested media type.
    #[must_use]
    pub const fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    /// Returns the requested security domain.
    #[must_use]
    pub const fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    /// Returns the requested confidentiality label.
    #[must_use]
    pub const fn confidentiality(&self) -> Confidentiality {
        self.confidentiality
    }

    /// Returns the requested retention class.
    #[must_use]
    pub const fn retention_class(&self) -> RetentionClass {
        self.retention_class
    }

    /// Returns the requested permission lineage.
    #[must_use]
    pub const fn permission_lineage_id(&self) -> PermissionLineageId {
        self.permission_lineage_id
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
    let lease_path = vault.layout.ensure_lease_path(&descriptor)?;
    let lease = durability::acquire_shared_object_lease(&lease_path)?;
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
        let live_evidence =
            verify_object_with_lease(&object_path, content_digest, byte_length, lease)?;
        let receipt = SealedArtifactReceipt::new(
            descriptor,
            object_path,
            SealDisposition::PublishedNew,
            live_evidence,
        );
        fault::trip(FaultPoint::V06);
        return Ok(receipt);
    }

    let live_evidence =
        match verify_object_with_lease(&object_path, content_digest, byte_length, lease) {
            Ok(evidence) => evidence,
            Err(VaultError::IntegrityMismatch(_)) => {
                return Err(VaultError::PathCollision(object_path));
            }
            Err(error) => return Err(error),
        };
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
    let receipt = SealedArtifactReceipt::new(
        descriptor,
        object_path,
        SealDisposition::AdoptedExisting,
        live_evidence,
    );
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

pub(crate) fn verify_object_with_lease(
    path: &Path,
    expected_digest: ContentDigest,
    expected_length: u64,
    lease: durability::SharedObjectLease,
) -> VaultResult<LiveObjectEvidence> {
    let (file, identity) = open_exact_object(path, expected_digest, expected_length)?;
    Ok(LiveObjectEvidence {
        file,
        identity,
        _lease: lease,
    })
}

fn open_exact_object(
    path: &Path,
    expected_digest: ContentDigest,
    expected_length: u64,
) -> VaultResult<(File, durability::ObjectIdentity)> {
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
    let mut file = durability::open_readonly_no_follow(path)?;
    let opened_metadata = file
        .metadata()
        .map_err(|source| VaultError::io("inspect opened sealed vault object", path, source))?;
    if !opened_metadata.file_type().is_file()
        || crate::layout::is_link_or_reparse(&opened_metadata)
        || opened_metadata.len() != expected_length
    {
        return Err(integrity_mismatch(path));
    }
    let identity = durability::object_identity(&file, path)?;
    verify_open_file(&mut file, path, expected_digest, expected_length)?;
    Ok((file, identity))
}

fn verify_open_file(
    file: &mut File,
    path: &Path,
    expected_digest: ContentDigest,
    expected_length: u64,
) -> VaultResult<()> {
    let before = file
        .metadata()
        .map_err(|source| VaultError::io("inspect live sealed vault object", path, source))?;
    if !before.file_type().is_file()
        || crate::layout::is_link_or_reparse(&before)
        || before.len() != expected_length
    {
        return Err(integrity_mismatch(path));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|source| VaultError::io("rewind live sealed vault object", path, source))?;
    let (actual_digest, actual_length) = hash_reader(&mut *file, path)?;
    let after = file
        .metadata()
        .map_err(|source| VaultError::io("reinspect live sealed vault object", path, source))?;
    if actual_digest != expected_digest || actual_length != expected_length {
        return Err(integrity_mismatch(path));
    }
    if !after.file_type().is_file()
        || crate::layout::is_link_or_reparse(&after)
        || after.len() != expected_length
    {
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
