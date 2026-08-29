//! The encrypted object vault: `AEAD_CHUNKED_V2` over the Phase 1 publish sequence.
//!
//! This lane changes what an object *is*, not how it becomes durable. The
//! crash-safe publish sequence, the domain-keyed locator namespace, the
//! exact-policy dedupe rule, the shared object lease, and the pre-commit
//! revalidation are the Phase 1 ones; only the byte format below them is new.
//!
//! # What this vault refuses
//!
//! It admits format `2` and nothing else. A descriptor naming format `1`
//! (`PLAINTEXT_SYNTHETIC_V1`) has no path in this namespace, so t068 §3.4's
//! "readers accept format 1 only inside a synthetic profile" is structural
//! here rather than a runtime check that could be forgotten: the plaintext
//! objects live under `vault/v1` with a `.obj` extension and this vault can
//! only spell `vault/v2` and `.aobj`.
//!
//! # Keys
//!
//! Every key comes from `P2-K1`'s schedule. `KEK_d` wraps each object's DEK and
//! authenticates its header; the locator HMAC key is
//! `HKDF-SHA-512(KEK_d, salt = profile_id, info = "academic-os/vault-locator/v1")`,
//! a sub-derivation of the same domain KEK rather than a new root key. This
//! vault generates one thing: the per-object DEK and base nonce.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use academic_crypto::{ArtifactDek, DomainKek, DomainLocatorKey, KEY_BYTES, ProfileId};
use academic_domain::{
    ArtifactDescriptor, ContentDigest, DomainId, MAX_SAFE_JSON_INTEGER, VaultLocator,
};
use sha2::{Digest as _, Sha256};

use crate::{
    ArtifactIngestRequest, SealDisposition, SealedObjectReceipt, SealedObjectVerifier, VaultError,
    VaultResult,
    durability::{self, LockedTemp, ObjectIdentity, SharedObjectLease},
    fault::{self, FaultPoint},
    integrity_mismatch,
    layout::{ObjectFormat, VaultLayout},
    now_millis,
    object::{
        self, HEADER_BYTES, ObjectFormatError, ObjectHeader, OpenedHeader, StreamingPrefix,
        TAG_BYTES,
    },
    reconcile::{ObjectNamespace, ReconcileOptions, ReconcileReport},
};

/// Format version every object in this namespace carries.
pub const ENCRYPTED_FORMAT_VERSION: u16 = object::OBJECT_FORMAT_VERSION;
/// Frozen name of the encrypted object format.
pub const ENCRYPTED_OBJECT_FORMAT: &str = "AEAD_CHUNKED_V2";
/// The `OB` rows of the Phase 2 fault matrix owned by `P2-K3`.
pub const PHASE2_OBJECT_FAULT_IDS: &[&str] = &[
    "OB01", "OB02", "OB03", "OB04", "OB05", "OB06", "OB07", "OB08", "OB09",
];

/// Per-domain object keys, derived once when a domain is registered.
///
/// `Debug` reveals only the domain count, and both keys zeroize on drop
/// because `academic-crypto` owns their types.
pub struct EncryptedDomainKeyring {
    profile: ProfileId,
    domains: BTreeMap<DomainId, DomainObjectKeys>,
}

struct DomainObjectKeys {
    kek: DomainKek,
    locator: DomainLocatorKey,
}

impl EncryptedDomainKeyring {
    /// Creates an empty keyring bound to one profile identity.
    #[must_use]
    pub const fn new(profile: ProfileId) -> Self {
        Self {
            profile,
            domains: BTreeMap::new(),
        }
    }

    /// Registers one domain's `KEK_d` and derives its locator key.
    ///
    /// The locator key is never supplied by a caller: deriving it here is what
    /// makes it impossible to register a KEK paired with a locator key from a
    /// different domain or profile.
    pub fn insert(&mut self, domain_id: DomainId, kek: DomainKek) -> VaultResult<()> {
        if self.domains.contains_key(&domain_id) {
            return Err(VaultError::DomainKeyConflict(domain_id));
        }
        let locator = kek
            .derive_locator_key(self.profile)
            .map_err(|_| VaultError::EmptyDomainKey(domain_id))?;
        self.domains
            .insert(domain_id, DomainObjectKeys { kek, locator });
        Ok(())
    }

    fn keys(&self, domain_id: DomainId) -> VaultResult<&DomainObjectKeys> {
        self.domains
            .get(&domain_id)
            .ok_or(VaultError::MissingDomainKey(domain_id))
    }
}

impl std::fmt::Debug for EncryptedDomainKeyring {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EncryptedDomainKeyring")
            .field("domain_count", &self.domains.len())
            .finish_non_exhaustive()
    }
}

/// Live read-back evidence retained by one encrypted sealed capability.
#[derive(Debug)]
struct LiveSealedObject {
    file: File,
    identity: ObjectIdentity,
    _lease: SharedObjectLease,
}

/// Opaque proof that one encrypted object is durably present and was decrypted
/// back to the exact descriptor bytes.
///
/// Every field is private and the constructor is crate-private, so this is not
/// mintable outside the read-back path, exactly like its plaintext counterpart.
#[derive(Debug)]
pub struct SealedEncryptedObject {
    descriptor: ArtifactDescriptor,
    object_path: PathBuf,
    disposition: SealDisposition,
    live: LiveSealedObject,
}

impl SealedObjectReceipt for SealedEncryptedObject {
    fn descriptor(&self) -> &ArtifactDescriptor {
        &self.descriptor
    }

    fn object_path(&self) -> &Path {
        &self.object_path
    }

    fn disposition(&self) -> SealDisposition {
        self.disposition
    }
}

impl SealedEncryptedObject {
    /// Returns the exact immutable descriptor bound by the receipt.
    #[must_use]
    pub const fn descriptor(&self) -> &ArtifactDescriptor {
        &self.descriptor
    }

    /// Returns the canonical physical object path.
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

/// One re-seal of an existing object under fresh key material.
///
/// Re-sealing never edits an object in place. It writes a new object, verifies
/// the read-back, and hands the caller both receipts; reachability moves only
/// when the caller appends its descriptor-migration event. Until that event
/// commits, the superseded object stays reachable and the new one is an
/// unreferenced orphan that reconciliation quarantines.
#[derive(Debug)]
pub struct ResealOutcome {
    /// Read-back evidence for the newly written object.
    pub resealed: SealedEncryptedObject,
    /// The descriptor the caller's migration event supersedes.
    pub superseded: ArtifactDescriptor,
}

/// Open encrypted vault bound to one validated encrypted profile.
#[derive(Debug)]
pub struct EncryptedVault {
    layout: VaultLayout,
    keyring: EncryptedDomainKeyring,
    chunk_size: u32,
}

impl EncryptedVault {
    /// Opens the encrypted object namespace below a validated profile root.
    pub fn open(profile_root: &Path, keyring: EncryptedDomainKeyring) -> VaultResult<Self> {
        Self::open_with_chunk_size(profile_root, keyring, object::DEFAULT_CHUNK_SIZE)
    }

    /// Opens with an explicit chunk size.
    ///
    /// Only tests that must exercise multi-chunk geometry without writing
    /// gigabytes pass anything but the default; the chunk size is recorded in
    /// every header, so a reader never has to be told which one was used.
    pub fn open_with_chunk_size(
        profile_root: &Path,
        keyring: EncryptedDomainKeyring,
        chunk_size: u32,
    ) -> VaultResult<Self> {
        if chunk_size == 0 {
            return Err(VaultError::ObjectFormat(
                ObjectFormatError::MalformedHeader("chunk_size"),
            ));
        }
        let layout = VaultLayout::new(profile_root, ObjectFormat::AeadChunkedV2);
        layout.initialize()?;
        Ok(Self {
            layout,
            keyring,
            chunk_size,
        })
    }

    /// Returns the validated profile root every issued receipt is bound to.
    #[must_use]
    pub fn profile_root(&self) -> &Path {
        self.layout.profile_root()
    }

    /// Returns the physical layout rooted below this profile.
    #[must_use]
    pub const fn layout(&self) -> &VaultLayout {
        &self.layout
    }

    /// Returns the plaintext chunk size new objects are sealed with.
    #[must_use]
    pub const fn chunk_size(&self) -> u32 {
        self.chunk_size
    }

    /// Streams, seals, publishes, and reads back one encrypted artifact.
    pub fn ingest(
        &self,
        request: &ArtifactIngestRequest,
        source: impl Read,
    ) -> VaultResult<SealedEncryptedObject> {
        self.ingest_with_chunk_size(request, source, self.chunk_size)
    }

    fn ingest_with_chunk_size(
        &self,
        request: &ArtifactIngestRequest,
        mut source: impl Read,
        chunk_size: u32,
    ) -> VaultResult<SealedEncryptedObject> {
        let keys = self.keyring.keys(request.domain_id())?;

        let dek = ArtifactDek::generate().map_err(|_| VaultError::EntropyUnavailable)?;
        let mut base_nonce = [0_u8; object::BASE_NONCE_BYTES];
        getrandom::fill(&mut base_nonce).map_err(|_| VaultError::EntropyUnavailable)?;
        let prefix = StreamingPrefix::new(
            chunk_size,
            *request.artifact_id().as_bytes(),
            *request.domain_id().as_bytes(),
            request.retention_class(),
            *request.permission_lineage_id().as_bytes(),
            base_nonce,
        );

        let (temp_path, mut temp) = self.create_temp()?;
        fault::trip(FaultPoint::Ob01);

        // The header is written last: it carries the locator and the plaintext
        // length, neither of which exists until the stream ends. Reserving its
        // bytes now keeps every chunk at its final offset, so publication is a
        // rename rather than a rewrite.
        temp.file_mut()
            .write_all(&[0_u8; HEADER_BYTES])
            .map_err(|error| {
                VaultError::io("reserve encrypted object header", &temp_path, error)
            })?;

        // One chunk of lookahead. A chunk's `is_final` byte is AAD, so the
        // writer must know the source has ended *before* it seals the last
        // chunk; an exact multiple of `chunk_size` would otherwise seal its
        // last full chunk as non-final and never authenticate as the end.
        let mut hasher = Sha256::new();
        let mut plaintext_len = 0_u64;
        let mut index = 0_u64;
        let mut buffer = Vec::with_capacity(chunk_size as usize + TAG_BYTES);
        let mut current = vec![0_u8; chunk_size as usize];
        let mut lookahead = vec![0_u8; chunk_size as usize];
        let mut current_len = fill(&mut source, &mut current, &temp_path)?;
        let mut first_chunk = true;

        loop {
            let lookahead_len = fill(&mut source, &mut lookahead, &temp_path)?;
            let is_final = lookahead_len == 0;

            hasher.update(&current[..current_len]);
            plaintext_len = plaintext_len
                .checked_add(u64::try_from(current_len).map_err(|_| VaultError::ArtifactTooLarge)?)
                .ok_or(VaultError::ArtifactTooLarge)?;
            if plaintext_len > MAX_SAFE_JSON_INTEGER {
                return Err(VaultError::ArtifactTooLarge);
            }

            buffer.clear();
            buffer.extend_from_slice(&current[..current_len]);
            object::seal_chunk(dek.expose_secret(), &prefix, index, is_final, &mut buffer)?;
            temp.file_mut().write_all(&buffer).map_err(|error| {
                VaultError::io("stream encrypted chunk into vault temp", &temp_path, error)
            })?;
            if first_chunk {
                first_chunk = false;
                fault::trip(FaultPoint::Ob02);
            }
            if is_final {
                break;
            }
            std::mem::swap(&mut current, &mut lookahead);
            current_len = lookahead_len;
            index += 1;
        }
        current.fill(0);
        lookahead.fill(0);
        buffer.fill(0);

        let content_digest = ContentDigest::from_sha256_bytes(hasher.finalize().into());
        let locator = VaultLocator::derive(
            keys.locator.expose_secret(),
            ENCRYPTED_FORMAT_VERSION,
            request.media_type(),
            content_digest,
        )?;
        // The header is written in two stages so that t068 §7's `OB01`
        // ("before header write") and `OB03` ("before header tag") are two
        // different on-disk states. Stage one is the cleartext prefix `P`;
        // stage two is `wrapped_dek`, whose trailing 16 bytes are the tag.
        let wrap_aad = prefix.wrap_aad(*locator.as_bytes(), plaintext_len)?;
        temp.file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|error| VaultError::io("rewind encrypted object temp", &temp_path, error))?;
        temp.file_mut().write_all(&wrap_aad).map_err(|error| {
            VaultError::io("write encrypted object header prefix", &temp_path, error)
        })?;
        fault::trip(FaultPoint::Ob03);

        let wrapped_dek = prefix.seal_wrapped_dek(
            keys.kek.expose_secret(),
            dek.expose_secret(),
            &wrap_aad,
            *content_digest.as_bytes(),
        )?;
        temp.file_mut().write_all(&wrapped_dek).map_err(|error| {
            VaultError::io("write encrypted object header tag", &temp_path, error)
        })?;
        temp.file_mut()
            .flush()
            .and_then(|()| temp.file_mut().sync_all())
            .map_err(|error| {
                VaultError::io("synchronize encrypted object temp", &temp_path, error)
            })?;
        fault::trip(FaultPoint::Ob04);

        let descriptor = request.descriptor_for(
            content_digest,
            plaintext_len,
            locator,
            ENCRYPTED_FORMAT_VERSION,
        );
        descriptor.validate()?;
        self.publish(descriptor, temp, temp_path)
    }

    fn publish(
        &self,
        descriptor: ArtifactDescriptor,
        mut temp: LockedTemp,
        temp_path: PathBuf,
    ) -> VaultResult<SealedEncryptedObject> {
        let object_path = self.layout.ensure_object_parent(&descriptor)?;
        let lease_path = self.layout.ensure_lease_path(&descriptor)?;
        let lease = durability::acquire_shared_object_lease(&lease_path)?;
        let object_parent = object_path
            .parent()
            .ok_or_else(|| VaultError::UnsafeEntry(object_path.clone()))?;
        durability::sync_directory(object_parent)?;

        let published = durability::publish_locked_no_replace(&mut temp, &temp_path, &object_path)?;
        if published {
            fault::trip(FaultPoint::Ob05);
            durability::sync_directory(object_parent)?;
            durability::sync_directory(self.layout.temp_dir())?;
            drop(temp);
            let live = self.read_back(&object_path, &descriptor, lease)?;
            return Ok(SealedEncryptedObject {
                descriptor,
                object_path,
                disposition: SealDisposition::PublishedNew,
                live,
            });
        }

        // Exact-policy dedupe: the locator namespace already holds an object,
        // and it must decrypt to exactly the same plaintext identity.
        let live = match self.read_back(&object_path, &descriptor, lease) {
            Ok(live) => live,
            Err(VaultError::IntegrityMismatch(_) | VaultError::ObjectFormat(_)) => {
                return Err(VaultError::PathCollision(object_path));
            }
            Err(error) => return Err(error),
        };
        drop(temp);
        match fs::remove_file(&temp_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(VaultError::io(
                    "remove deduplicated encrypted vault temp",
                    &temp_path,
                    error,
                ));
            }
        }
        durability::sync_directory(self.layout.temp_dir())?;
        Ok(SealedEncryptedObject {
            descriptor,
            object_path,
            disposition: SealDisposition::AdoptedExisting,
            live,
        })
    }

    /// Re-seals one reachable object into `destination` under fresh key material.
    ///
    /// `destination` is a vault holding the keyring the object is moving *to*.
    /// For a `P2-K5` KEK rotation that is a different `KEK_d`, so the locator —
    /// which derives from it — changes and the re-sealed object lands on a new
    /// canonical path. Passing `self` re-seals in place under the same key,
    /// which lands on the same locator and adopts the existing bytes; that is
    /// the degenerate case, not the rotation one.
    ///
    /// The old object is left exactly as it is: nothing here edits an object in
    /// place, and nothing here moves reachability. The caller appends the
    /// descriptor-migration event, and only after that event commits may it
    /// quarantine the superseded object.
    pub fn reseal(
        &self,
        descriptor: &ArtifactDescriptor,
        destination: &Self,
    ) -> VaultResult<ResealOutcome> {
        let reader = self.open_reader(descriptor)?;
        let request = ArtifactIngestRequest::new(
            descriptor.id,
            descriptor.media_type.clone(),
            descriptor.domain_id,
            descriptor.confidentiality,
            descriptor.retention_class,
            descriptor.permission_lineage_id,
        );
        // The plaintext is streamed from the old object into the new one; it
        // never lands in a buffer sized by the artifact, so a multi-gigabyte
        // object re-seals in bounded memory.
        let resealed = destination.ingest(&request, reader)?;
        // OB09: a termination here has published the new object but not the
        // event that makes it reachable. The old object keeps its reference and
        // the new one is an unreferenced orphan reconciliation quarantines.
        fault::trip(FaultPoint::Ob09);
        Ok(ResealOutcome {
            resealed,
            superseded: descriptor.clone(),
        })
    }

    /// Reconciles temps, objects, quarantine, and authoritative references.
    pub fn reconcile(&self, options: &ReconcileOptions<'_>) -> VaultResult<ReconcileReport> {
        crate::reconcile::reconcile(self, options)
    }

    /// Crypto-shreds one object by destroying its key slot.
    ///
    /// This is the one operation in this vault that writes into an object that
    /// is already published, and it is deliberate: a crypto-shred that wrote a
    /// new file and left the old one would have destroyed nothing. The write
    /// covers exactly `[KEY_SLOT_OFFSET, HEADER_BYTES)` — the wrapped DEK,
    /// which is the only copy of the key this object was sealed under, and the
    /// only copy of its plaintext digest. Every other byte, the file itself,
    /// and its length are left exactly as they are.
    ///
    /// # What this claims, and what it does not
    ///
    /// It claims the ciphertext is unreadable: no key opens the object
    /// afterwards, not the domain KEK it was sealed under, not a rotated one,
    /// and not one recovered from a backup. It does **not** claim the file was
    /// deleted, that its bytes were overwritten, or that a copy taken earlier
    /// was reached. Copies inside a backup are reached by `P2-K5`'s backup
    /// tombstone, not by this call.
    ///
    /// `RB01` requires the outcome to be "shredded or intact". The slot write
    /// is one positioned write plus a sync, so a kill leaves the slot either
    /// untouched or destroyed; a kill *during* that write destroys the key but
    /// may leave the marker incomplete, which is why the caller journals its
    /// intent first and re-applies on resume. Re-applying is idempotent.
    pub fn shred_key_slot(
        &self,
        descriptor: &ArtifactDescriptor,
        tombstone_digest: &[u8; 32],
    ) -> VaultResult<ShredReceipt> {
        let path = self.validate_descriptor_locator(descriptor)?;
        shred_key_slot_at(&path, tombstone_digest)
    }

    /// Opens a seekable plaintext reader over one canonical encrypted object.
    ///
    /// The header is authenticated before the reader exists, so a wrong key, a
    /// wrong domain, or a tampered header fails here and no chunk is touched.
    pub fn open_reader(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> VaultResult<EncryptedObjectReader> {
        let path = self.validate_descriptor_locator(descriptor)?;
        let keys = self.keyring.keys(descriptor.domain_id)?;
        let mut file = durability::open_readonly_no_follow(&path)?;
        let opened = read_and_open_header(&mut file, &path, keys.kek.expose_secret())?;
        opened.header.require_matches(descriptor)?;
        require_sealed_length(&file, &path, &opened.header)?;
        Ok(EncryptedObjectReader::new(file, path, opened))
    }

    pub(crate) fn validate_descriptor_locator(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> VaultResult<PathBuf> {
        descriptor.validate()?;
        if descriptor.format_version != ENCRYPTED_FORMAT_VERSION {
            return Err(VaultError::LocatorMismatch(descriptor.id));
        }
        let keys = self.keyring.keys(descriptor.domain_id)?;
        let expected = VaultLocator::derive(
            keys.locator.expose_secret(),
            ENCRYPTED_FORMAT_VERSION,
            &descriptor.media_type,
            descriptor.content_digest,
        )?;
        if expected != descriptor.vault_locator {
            return Err(VaultError::LocatorMismatch(descriptor.id));
        }
        self.layout.object_path(descriptor)
    }

    fn create_temp(&self) -> VaultResult<(PathBuf, LockedTemp)> {
        for _ in 0..16 {
            let mut random = [0_u8; 16];
            getrandom::fill(&mut random).map_err(|_| VaultError::EntropyUnavailable)?;
            let session = format!("{:013}-{}", now_millis()?, crate::encode_hex(&random));
            let path = self.layout.temp_dir().join(format!("{session}.partial"));
            match durability::create_locked_temp(&path) {
                Ok(file) => return Ok((path, file)),
                Err(VaultError::Io { source, .. })
                    if source.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(VaultError::EntropyUnavailable)
    }

    fn read_back(
        &self,
        path: &Path,
        descriptor: &ArtifactDescriptor,
        lease: SharedObjectLease,
    ) -> VaultResult<LiveSealedObject> {
        let keys = self.keyring.keys(descriptor.domain_id)?;
        let (file, identity) = open_exact_object(path, descriptor, keys.kek.expose_secret())?;
        Ok(LiveSealedObject {
            file,
            identity,
            _lease: lease,
        })
    }
}

impl ObjectNamespace for EncryptedVault {
    fn layout(&self) -> &VaultLayout {
        Self::layout(self)
    }

    fn validate_descriptor_locator(&self, descriptor: &ArtifactDescriptor) -> VaultResult<PathBuf> {
        Self::validate_descriptor_locator(self, descriptor)
    }

    fn verify_object(&self, descriptor: &ArtifactDescriptor) -> VaultResult<()> {
        match SealedObjectVerifier::verify_sealed_object(self, descriptor) {
            Ok(_receipt) => Ok(()),
            // A file that is present but does not authenticate is the same
            // reconciliation outcome as one whose bytes do not match: corrupt,
            // repair-required. It is never silently valid.
            Err(VaultError::ObjectFormat(_)) => {
                let path = Self::validate_descriptor_locator(self, descriptor)
                    .unwrap_or_else(|_| self.layout.objects_root().to_path_buf());
                Err(integrity_mismatch(&path))
            }
            Err(error) => Err(error),
        }
    }
}

impl SealedObjectVerifier for EncryptedVault {
    type Receipt = SealedEncryptedObject;

    fn profile_root(&self) -> &Path {
        Self::profile_root(self)
    }

    fn verify_sealed_object(&self, descriptor: &ArtifactDescriptor) -> VaultResult<Self::Receipt> {
        let path = Self::validate_descriptor_locator(self, descriptor)?;
        let lease_path = self.layout.ensure_lease_path(descriptor)?;
        let lease = durability::acquire_shared_object_lease(&lease_path)?;
        let live = self.read_back(&path, descriptor, lease)?;
        Ok(SealedEncryptedObject {
            descriptor: descriptor.clone(),
            object_path: path,
            disposition: SealDisposition::AdoptedExisting,
            live,
        })
    }

    fn revalidate_sealed_object(&self, receipt: &mut Self::Receipt) -> VaultResult<()> {
        let canonical_path = Self::validate_descriptor_locator(self, &receipt.descriptor)?;
        if canonical_path != receipt.object_path {
            return Err(integrity_mismatch(&canonical_path));
        }
        let keys = self.keyring.keys(receipt.descriptor.domain_id)?;
        if durability::object_identity(&receipt.live.file, &canonical_path)?
            != receipt.live.identity
        {
            return Err(integrity_mismatch(&canonical_path));
        }
        verify_open_object(
            &mut receipt.live.file,
            &canonical_path,
            &receipt.descriptor,
            keys.kek.expose_secret(),
        )?;
        let (_reopened, reopened_identity) = open_exact_object(
            &canonical_path,
            &receipt.descriptor,
            keys.kek.expose_secret(),
        )?;
        if reopened_identity != receipt.live.identity {
            return Err(integrity_mismatch(&canonical_path));
        }
        Ok(())
    }
}

/// A seekable plaintext reader over one authenticated encrypted object.
///
/// Seeking resolves to a chunk index and an offset inside that chunk, so a seek
/// into a multi-gigabyte object reads exactly one chunk rather than the prefix
/// before it. Every chunk is authenticated as it is read; a reader never
/// returns a byte from a chunk whose tag did not verify.
pub struct EncryptedObjectReader {
    file: File,
    path: PathBuf,
    opened: OpenedHeader,
    position: u64,
    loaded: Option<u64>,
    chunk: Vec<u8>,
}

/// Prints no plaintext byte.
///
/// `chunk` holds the decrypted contents of whichever chunk was read last — up
/// to `chunk_size` bytes, a mebibyte by default — so the derived implementation
/// would put artifact plaintext into any log line, panic message, or audit row
/// that formatted a reader. That is the same defect `OpenedHeader` carried, and
/// `tools/secret-debug-policy.test.mjs` is what keeps it from returning.
impl std::fmt::Debug for EncryptedObjectReader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EncryptedObjectReader")
            .field("path", &self.path)
            .field("position", &self.position)
            .field("loaded_chunk", &self.loaded)
            .field("chunk", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Clears the decrypted chunk rather than leaving it in freed heap.
///
/// This is the same hand-written clear `OpenedHeader` and `DomainKeyring` use;
/// `academic-vault` carries no `zeroize` dependency and this crate's other key
/// buffers are cleared the same way.
impl Drop for EncryptedObjectReader {
    fn drop(&mut self) {
        self.chunk.fill(0);
    }
}

impl EncryptedObjectReader {
    fn new(file: File, path: PathBuf, opened: OpenedHeader) -> Self {
        Self {
            file,
            path,
            opened,
            position: 0,
            loaded: None,
            chunk: Vec::new(),
        }
    }

    /// Returns the canonical object path this reader is bound to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the exact plaintext length the authenticated header commits to.
    #[must_use]
    pub const fn plaintext_len(&self) -> u64 {
        self.opened.header.plaintext_len()
    }

    /// Returns the authenticated header.
    #[must_use]
    pub const fn header(&self) -> &ObjectHeader {
        &self.opened.header
    }

    /// Returns the logical plaintext digest recovered from the sealed header.
    #[must_use]
    pub const fn plaintext_digest(&self) -> &[u8; 32] {
        self.opened.plaintext_digest()
    }

    fn load_chunk(&mut self, index: u64) -> VaultResult<()> {
        if self.loaded == Some(index) {
            return Ok(());
        }
        let header = &self.opened.header;
        let length = header.chunk_plaintext_len(index) as usize + TAG_BYTES;
        self.file
            .seek(SeekFrom::Start(header.chunk_offset(index)))
            .map_err(|error| VaultError::io("seek encrypted object chunk", &self.path, error))?;
        // Cleared before the length changes: `clear` does not overwrite, and a
        // `resize` that grows past the capacity would otherwise copy the
        // previous chunk's plaintext into a new allocation and leave the old
        // one in freed heap.
        self.chunk.fill(0);
        self.chunk.clear();
        self.chunk.resize(length, 0);
        self.file
            .read_exact(&mut self.chunk)
            .map_err(|error| VaultError::io("read encrypted object chunk", &self.path, error))?;
        object::open_chunk(self.opened.expose_dek(), header, index, &mut self.chunk)?;
        self.loaded = Some(index);
        Ok(())
    }
}

impl Read for EncryptedObjectReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.position >= self.plaintext_len() {
            return Ok(0);
        }
        let chunk_size = u64::from(self.opened.header.chunk_size());
        let index = self.position / chunk_size;
        let offset = usize::try_from(self.position % chunk_size)
            .map_err(|_| io::Error::other("chunk offset exceeds this platform's addressing"))?;
        // The typed vault error travels as the `io::Error` source, so a caller
        // can still tell an authentication failure from a short read.
        self.load_chunk(index).map_err(io::Error::other)?;
        let available = self.chunk.len().saturating_sub(offset);
        let taken = available.min(out.len());
        out[..taken].copy_from_slice(&self.chunk[offset..offset + taken]);
        self.position += taken as u64;
        Ok(taken)
    }
}

impl Seek for EncryptedObjectReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let length = self.plaintext_len();
        let resolved = match position {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::End(offset) => i128::from(length) + i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
        };
        if resolved < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before the start of the object",
            ));
        }
        self.position = u64::try_from(resolved)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "seek beyond u64"))?;
        Ok(self.position)
    }
}

/// Reads until `buffer` is full or the source ends, returning the byte count.
fn fill(source: &mut impl Read, buffer: &mut [u8], path: &Path) -> VaultResult<usize> {
    let mut filled = 0_usize;
    while filled < buffer.len() {
        let read = source
            .read(&mut buffer[filled..])
            .map_err(|error| VaultError::io("read artifact source", path, error))?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    Ok(filled)
}

fn read_and_open_header(
    file: &mut File,
    path: &Path,
    domain_kek: &[u8; KEY_BYTES],
) -> VaultResult<OpenedHeader> {
    let mut bytes = [0_u8; HEADER_BYTES];
    file.seek(SeekFrom::Start(0))
        .map_err(|error| VaultError::io("rewind encrypted object", path, error))?;
    match file.read_exact(&mut bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(VaultError::ObjectFormat(ObjectFormatError::Truncated));
        }
        Err(error) => {
            return Err(VaultError::io("read encrypted object header", path, error));
        }
    }
    fault::trip(FaultPoint::Ob08);
    Ok(object::open_header(&bytes, domain_kek)?)
}

fn require_sealed_length(file: &File, path: &Path, header: &ObjectHeader) -> VaultResult<()> {
    let metadata = file
        .metadata()
        .map_err(|error| VaultError::io("inspect encrypted object", path, error))?;
    if metadata.len() != header.sealed_len() {
        return Err(VaultError::ObjectFormat(ObjectFormatError::Truncated));
    }
    Ok(())
}

fn open_exact_object(
    path: &Path,
    descriptor: &ArtifactDescriptor,
    domain_kek: &[u8; KEY_BYTES],
) -> VaultResult<(File, ObjectIdentity)> {
    let metadata = match durability::symlink_metadata_no_follow(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(integrity_mismatch(path));
        }
        Err(error) => {
            return Err(VaultError::io(
                "inspect sealed encrypted object",
                path,
                error,
            ));
        }
    };
    if !metadata.file_type().is_file() || crate::layout::is_link_or_reparse(&metadata) {
        return Err(integrity_mismatch(path));
    }
    let mut file = durability::open_readonly_no_follow(path)?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| VaultError::io("inspect opened sealed encrypted object", path, error))?;
    if !opened_metadata.file_type().is_file() || crate::layout::is_link_or_reparse(&opened_metadata)
    {
        return Err(integrity_mismatch(path));
    }
    let identity = durability::object_identity(&file, path)?;
    verify_open_object(&mut file, path, descriptor, domain_kek)?;
    Ok((file, identity))
}

/// Decrypts a live object end to end and confirms it is exactly the descriptor.
fn verify_open_object(
    file: &mut File,
    path: &Path,
    descriptor: &ArtifactDescriptor,
    domain_kek: &[u8; KEY_BYTES],
) -> VaultResult<()> {
    let opened = read_and_open_header(file, path, domain_kek)?;
    opened.header.require_matches(descriptor)?;
    require_sealed_length(file, path, &opened.header)?;

    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = Vec::new();
    for index in 0..opened.header.chunk_count() {
        let length = opened.header.chunk_plaintext_len(index) as usize + TAG_BYTES;
        file.seek(SeekFrom::Start(opened.header.chunk_offset(index)))
            .map_err(|error| VaultError::io("seek encrypted object chunk", path, error))?;
        buffer.clear();
        buffer.resize(length, 0);
        file.read_exact(&mut buffer)
            .map_err(|error| VaultError::io("read encrypted object chunk", path, error))?;
        object::open_chunk(opened.expose_dek(), &opened.header, index, &mut buffer)?;
        hasher.update(&buffer);
        observed = observed
            .checked_add(u64::try_from(buffer.len()).map_err(|_| VaultError::ArtifactTooLarge)?)
            .ok_or(VaultError::ArtifactTooLarge)?;
    }
    buffer.fill(0);

    if observed != descriptor.byte_length {
        return Err(integrity_mismatch(path));
    }
    let digest = ContentDigest::from_sha256_bytes(hasher.finalize().into());
    object::require_plaintext_digest(opened.plaintext_digest(), digest)?;
    if digest != descriptor.content_digest {
        return Err(integrity_mismatch(path));
    }
    Ok(())
}

/// Exposes the artifact-id accessor family the encrypted lane needs.
impl ArtifactIngestRequest {
    pub(crate) fn descriptor_for(
        &self,
        content_digest: ContentDigest,
        byte_length: u64,
        vault_locator: VaultLocator,
        format_version: u16,
    ) -> ArtifactDescriptor {
        ArtifactDescriptor {
            id: self.artifact_id(),
            content_digest,
            media_type: self.media_type().clone(),
            byte_length,
            domain_id: self.domain_id(),
            confidentiality: self.confidentiality(),
            retention_class: self.retention_class(),
            permission_lineage_id: self.permission_lineage_id(),
            format_version,
            vault_locator,
            evidence_representations: Vec::new(),
        }
    }
}

/// Evidence that one object's key slot is destroyed and durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShredReceipt {
    object_path: PathBuf,
    locator: [u8; 32],
    was_already_shredded: bool,
}

impl ShredReceipt {
    /// Returns the object whose key slot was destroyed.
    #[must_use]
    pub fn object_path(&self) -> &Path {
        &self.object_path
    }

    /// Returns the object's cleartext locator.
    #[must_use]
    pub const fn locator(&self) -> &[u8; 32] {
        &self.locator
    }

    /// Reports whether the slot was already destroyed before this call.
    ///
    /// Re-applying a shred is not an error: a resumed retention action has to
    /// be able to finish one it started.
    #[must_use]
    pub const fn was_already_shredded(&self) -> bool {
        self.was_already_shredded
    }
}

/// Destroys the key slot of the object at `path`, without needing any key.
///
/// This is the entry point a restore uses: re-applying a backup tombstone to a
/// restored object happens before anything is unlocked, so it cannot go through
/// a keyed vault handle.
pub fn shred_key_slot_at(path: &Path, tombstone_digest: &[u8; 32]) -> VaultResult<ShredReceipt> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| VaultError::io("open object for key-slot shred", path, error))?;
    let mut header = [0_u8; HEADER_BYTES];
    file.read_exact(&mut header)
        .map_err(|error| VaultError::io("read object header for key-slot shred", path, error))?;
    let locator = object::read_locator(&header).map_err(VaultError::ObjectFormat)?;
    if object::is_shredded_header(&header) {
        return Ok(ShredReceipt {
            object_path: path.to_path_buf(),
            locator,
            was_already_shredded: true,
        });
    }

    fault::trip(FaultPoint::Rb01);

    let slot = object::shredded_key_slot(tombstone_digest);
    file.seek(SeekFrom::Start(object::KEY_SLOT_OFFSET as u64))
        .map_err(|error| VaultError::io("seek to object key slot", path, error))?;
    file.write_all(&slot)
        .map_err(|error| VaultError::io("destroy object key slot", path, error))?;
    file.sync_all()
        .map_err(|error| VaultError::io("synchronize destroyed key slot", path, error))?;
    Ok(ShredReceipt {
        object_path: path.to_path_buf(),
        locator,
        was_already_shredded: false,
    })
}
