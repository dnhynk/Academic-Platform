//! Durable single-writer acceptance in the exact Phase 1 transaction order.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use academic_contracts::VerifiedBatch;
use academic_domain::{
    ArtifactDescriptor, ArtifactId, ClaimId, DecisionAction, EventPayload, EvidenceId,
    TimestampMillis, UnsignedBatch, VaultLocator,
};
use academic_ledger::LedgerError;
use academic_vault::{SealedObjectReceipt, SealedObjectVerifier};

#[cfg(not(feature = "sqlcipher-store"))]
use crate::connection::open_writer;
use crate::{
    connection::{PragmaSnapshot, WriterConnection, verify_admitted_schema_version},
    error::{StoreError, StoreResult},
    fault::{AcceptanceFaultInjector, AcceptanceFaultPoint, InjectedFault, NoFault},
    idempotency::{
        AcceptanceCommand, DurableAcceptanceReceipt, IdempotencyError, insert_command_receipt,
        lookup_command_receipt,
    },
    outbox::{OutboxError, insert_outbox},
    repository::{
        ClosureWriter, RepositoryError, find_batch, insert_batch, preflight_artifact_descriptor,
        preflight_claim_evidence, preflight_evidence_artifact, read_device_head,
        read_replica_state, update_heads,
    },
};

/// Successful command result, including whether SQL returned a stored replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceOutcome {
    pub receipt: DurableAcceptanceReceipt,
    pub replayed_request: bool,
    pub duplicate_batch: bool,
}

/// The one event schema v3 arm this build accepts.
///
/// `P2-K5` owns `RETENTION_ACTION_RECORDED`. Migration `0004` gives it a
/// closure table, migration `0005` gives it the typed columns a descriptor
/// migration needs, and [`crate::repository::ClosureWriter`] places its row, so
/// it has a complete write path. The other seventeen arms do not yet and stay
/// refused until each owner writes one.
///
/// It is admitted only in the encrypted lane, because only that lane's
/// `STORE_MIGRATION_SQL` applies the two migrations it depends on.
#[cfg(feature = "sqlcipher-store")]
const WRITABLE_V3_ARM: Option<&str> = Some("RETENTION_ACTION_RECORDED");
/// The plaintext synthetic lane carries neither closure table, so no v3 arm is
/// storable there.
#[cfg(not(feature = "sqlcipher-store"))]
const WRITABLE_V3_ARM: Option<&str> = None;

/// Sole owned product writer with only the authenticated acceptance operation exposed.
///
/// The raw SQLite writer and its construction functions are crate-private. This type is neither
/// cloneable nor convertible to a connection, and it exposes no arbitrary SQL surface.
pub struct AcceptanceStore {
    writer: WriterConnection,
    profile_root: PathBuf,
}

impl fmt::Debug for AcceptanceStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceptanceStore")
            .field("database_path", &self.writer.database_path())
            .field("profile_root", &self.profile_root)
            .finish_non_exhaustive()
    }
}

impl AcceptanceStore {
    #[cfg(not(feature = "sqlcipher-store"))]
    pub(crate) fn open(profile_root: &Path, database_path: &Path) -> StoreResult<Self> {
        Ok(Self {
            writer: open_writer(database_path)?,
            profile_root: profile_root.to_path_buf(),
        })
    }

    /// Opens the sole owned acceptance service against an encrypted profile.
    ///
    /// The raw store key is applied as the first statement on the writer
    /// handle, so the acceptance transaction, its authorizer, and its
    /// append-only triggers run over exactly the same schema as the
    /// plaintext lane and gain no bypass from encryption.
    #[cfg(feature = "sqlcipher-store")]
    pub(crate) fn open(
        profile_root: &Path,
        database_path: &Path,
        key: &academic_crypto::StoreKey,
    ) -> StoreResult<Self> {
        Ok(Self {
            writer: crate::connection::open_keyed_writer(database_path, key)?,
            profile_root: profile_root.to_path_buf(),
        })
    }

    /// Returns the database path for read-only diagnostics and projection composition.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        self.writer.database_path()
    }

    /// Reads the exact configured writer PRAGMAs without exposing a SQL handle.
    pub fn pragma_snapshot(&self) -> StoreResult<PragmaSnapshot> {
        self.writer.pragma_snapshot()
    }

    /// Accepts an authenticated batch after concrete vault read-back, with faults disabled.
    pub fn accept_verified_batch<V: SealedObjectVerifier>(
        &mut self,
        verified: &VerifiedBatch,
        command: AcceptanceCommand<'_>,
        accepted_at: TimestampMillis,
        vault: &V,
    ) -> Result<AcceptanceOutcome, AcceptError> {
        self.ensure_vault_profile(vault)?;
        accept_verified_batch_with_faults(
            &mut self.writer,
            verified,
            command,
            accepted_at,
            vault,
            &NoFault,
        )
    }

    /// Deterministic DB01-DB07 harness over the same owned acceptance writer.
    ///
    /// The callback can stop or pause a test at a checkpoint, but it cannot alter verification,
    /// issue a vault capability, execute SQL, or reach a second writer.
    pub fn accept_verified_batch_with_faults<V, F>(
        &mut self,
        verified: &VerifiedBatch,
        command: AcceptanceCommand<'_>,
        accepted_at: TimestampMillis,
        vault: &V,
        faults: &F,
    ) -> Result<AcceptanceOutcome, AcceptError>
    where
        V: SealedObjectVerifier,
        F: AcceptanceFaultInjector,
    {
        self.ensure_vault_profile(vault)?;
        accept_verified_batch_with_faults(
            &mut self.writer,
            verified,
            command,
            accepted_at,
            vault,
            faults,
        )
    }

    /// Appends one descriptor migration, moving an artifact's object reference.
    ///
    /// This is the second half of a re-seal. The first half is an accepted
    /// `RETENTION_ACTION_RECORDED` event whose `source_digest` is
    /// `migration.record_digest()`; migration `0005`'s triggers refuse this
    /// insert unless that event exists and carries exactly that digest, so a
    /// reference cannot move without a canonical event authorizing precisely
    /// where it moved to.
    ///
    /// `migrated` is the descriptor as it is after the move. It is
    /// authenticated through the same `SealedObjectVerifier` seam acceptance
    /// uses before the row is written, so the store never starts resolving to
    /// an object it has not opened.
    ///
    /// Re-running with the same record is idempotent: the primary key refuses
    /// the second insert and the reference is already where it belongs, which
    /// is what a rotation resume needs.
    pub fn record_descriptor_migration<V: SealedObjectVerifier>(
        &mut self,
        migration: &crate::descriptor_migration::DescriptorMigration,
        migrated: &ArtifactDescriptor,
        vault: &V,
    ) -> StoreResult<()> {
        if vault.profile_root() != self.profile_root {
            return Err(StoreError::DescriptorMigrationRejected {
                reason: "the verifying vault belongs to a different profile",
                detail: vault.profile_root().display().to_string(),
            });
        }
        let authorization = self.writer.authorize_acceptance();
        let transaction = self.writer.begin_immediate()?;
        let outcome = crate::descriptor_migration::insert_descriptor_migration(
            &transaction,
            migration,
            migrated,
            vault,
        );
        match outcome {
            Ok(()) => {
                transaction.commit()?;
                drop(authorization);
                Ok(())
            }
            Err(error) => {
                drop(transaction);
                drop(authorization);
                Err(error)
            }
        }
    }

    /// Reads every recorded descriptor migration, in chain order.
    pub fn descriptor_migrations(
        &self,
    ) -> StoreResult<Vec<crate::descriptor_migration::DescriptorMigration>> {
        self.writer
            .with_preflight_reader(crate::descriptor_migration::read_descriptor_migrations)
    }

    /// Returns the descriptor the store now resolves one artifact to.
    ///
    /// The signed `artifact_descriptor` row walked to the end of its
    /// `artifact_descriptor_migration` chain — the same resolution acceptance's
    /// own preflight, a backup, and a restore use. `None` means the store holds
    /// no descriptor for that artifact at all.
    ///
    /// This is the read a retirement is gated on. Destroying a superseded
    /// object's key slot is irreversible, so "the canonical reference has
    /// already moved" has to be a fact read out of the store rather than an
    /// argument the caller states.
    pub fn resolved_artifact_descriptor(
        &self,
        artifact: ArtifactId,
    ) -> StoreResult<Option<ArtifactDescriptor>> {
        // `RepositoryError` is the normalized reader's vocabulary and this is a
        // store-level read, so the SQLite failure keeps its own type and every
        // other cause becomes the one thing it can honestly be called: the
        // stored descriptor row is not readable as a descriptor.
        let read = preflight_artifact_descriptor(&self.writer, artifact).map_err(|error| {
            match error {
                RepositoryError::Sqlite(source) => StoreError::Sqlite(source),
                _ => StoreError::InvalidProfileState {
                    path: self.profile_root.clone(),
                    reason: "a stored artifact descriptor row is not a readable descriptor",
                },
            }
        })?;
        let Some(descriptor) = read else {
            return Ok(None);
        };
        let mut resolved = [descriptor];
        self.writer.with_preflight_reader(|connection| {
            crate::descriptor_migration::resolve_with_stored_migrations(connection, &mut resolved)
        })?;
        let [descriptor] = resolved;
        Ok(Some(descriptor))
    }

    /// Returns every locator one artifact was reachable under before now.
    ///
    /// In chain order, oldest first. A deletion writes these into its backup
    /// tombstone: a locator is a function of the domain KEK, so a backup taken
    /// before a rotation holds the object under an older name and a tombstone
    /// naming only the current one would never reach it.
    pub fn superseded_locators(&self, artifact: ArtifactId) -> StoreResult<Vec<VaultLocator>> {
        self.writer.with_preflight_reader(|connection| {
            crate::descriptor_migration::superseded_locators(connection, artifact)
        })
    }


    /// Reports whether one artifact's chain has already recorded `locator`.
    ///
    /// The preflight a rotation orchestrator runs **before** it journals
    /// anything. A locator is a deterministic function of the generation, so
    /// rotating back to a generation the artifact has already been under would
    /// try to record a locator the chain holds; the row is refused, and by then
    /// the journal has already said the unit migrated. Asking first is how the
    /// journal and the store are kept from naming different objects.
    pub fn locator_is_already_in_chain(
        &self,
        artifact: ArtifactId,
        locator: &VaultLocator,
    ) -> StoreResult<bool> {
        self.writer.with_preflight_reader(|connection| {
            crate::descriptor_migration::locator_is_already_in_chain(connection, artifact, locator)
        })
    }


    /// Returns the next chain position for one artifact's reference.
    pub fn next_descriptor_migration_seq(&self, artifact: ArtifactId) -> StoreResult<u64> {
        self.writer.with_preflight_reader(|connection| {
            crate::descriptor_migration::next_migration_seq(connection, artifact)
        })
    }

    fn ensure_vault_profile<V: SealedObjectVerifier>(&self, vault: &V) -> Result<(), AcceptError> {
        if vault.profile_root() == self.profile_root {
            Ok(())
        } else {
            Err(AcceptError::VaultProfileMismatch {
                expected: self.profile_root.clone(),
                actual: vault.profile_root().to_path_buf(),
            })
        }
    }
}

/// Fail-closed acceptance error. Every variant before commit consumes no row,
/// acceptance sequence, device sequence, outbox sequence, or revision.
#[derive(Debug)]
pub enum AcceptError {
    Store(StoreError),
    Sqlite(rusqlite::Error),
    Idempotency(IdempotencyError),
    Repository(RepositoryError),
    Outbox(OutboxError),
    Ledger(LedgerError),
    Injected(InjectedFault),
    CommandEnvelopeMismatch,
    ExpectedRevisionConflict {
        expected: u64,
        actual: u64,
    },
    SealingFailed {
        artifact_id: ArtifactId,
        source: Box<dyn Error + Send + Sync>,
    },
    SealedReceiptMismatch {
        artifact_id: ArtifactId,
    },
    VaultProfileMismatch {
        expected: PathBuf,
        actual: PathBuf,
    },
    IntegerOverflow(u64),
    UnstorableEventKind(&'static str),
}

impl fmt::Display for AcceptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "store acceptance error: {error}"),
            Self::Sqlite(error) => write!(formatter, "SQLite acceptance error: {error}"),
            Self::Idempotency(error) => write!(formatter, "{error}"),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Outbox(error) => write!(formatter, "{error}"),
            Self::Ledger(error) => write!(formatter, "{error}"),
            Self::Injected(error) => write!(formatter, "{error}"),
            Self::CommandEnvelopeMismatch => {
                formatter.write_str("verified envelope differs from command source bytes")
            }
            Self::UnstorableEventKind(kind) => write!(
                formatter,
                "event kind {kind} has no canonical table in this store schema"
            ),
            Self::ExpectedRevisionConflict { expected, actual } => write!(
                formatter,
                "expected profile revision {expected}, observed {actual}"
            ),
            Self::SealingFailed {
                artifact_id,
                source,
            } => write!(formatter, "artifact {artifact_id} was not sealed: {source}"),
            Self::SealedReceiptMismatch { artifact_id } => write!(
                formatter,
                "sealed receipt does not match artifact descriptor {artifact_id}"
            ),
            Self::VaultProfileMismatch { expected, actual } => write!(
                formatter,
                "acceptance store profile {} does not own vault profile {}",
                expected.display(),
                actual.display()
            ),
            Self::IntegerOverflow(value) => write!(
                formatter,
                "acceptance value {value} exceeds the signed SQLite coordinate"
            ),
        }
    }
}

impl Error for AcceptError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            Self::Idempotency(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::Outbox(error) => Some(error),
            Self::Ledger(error) => Some(error),
            Self::Injected(error) => Some(error),
            Self::SealingFailed { source, .. } => Some(source.as_ref()),
            Self::CommandEnvelopeMismatch
            | Self::ExpectedRevisionConflict { .. }
            | Self::SealedReceiptMismatch { .. }
            | Self::VaultProfileMismatch { .. }
            | Self::IntegerOverflow(_)
            | Self::UnstorableEventKind(_) => None,
        }
    }
}

impl From<StoreError> for AcceptError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<rusqlite::Error> for AcceptError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<IdempotencyError> for AcceptError {
    fn from(error: IdempotencyError) -> Self {
        Self::Idempotency(error)
    }
}

impl From<RepositoryError> for AcceptError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<OutboxError> for AcceptError {
    fn from(error: OutboxError) -> Self {
        Self::Outbox(error)
    }
}

impl From<LedgerError> for AcceptError {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

impl From<InjectedFault> for AcceptError {
    fn from(error: InjectedFault) -> Self {
        Self::Injected(error)
    }
}

/// Same acceptance boundary with an explicit test-harness callback.
///
/// No environment variable or command-line switch can reach this capability.
fn accept_verified_batch_with_faults<V, F>(
    writer: &mut WriterConnection,
    verified: &VerifiedBatch,
    command: AcceptanceCommand<'_>,
    accepted_at: TimestampMillis,
    vault: &V,
    faults: &F,
) -> Result<AcceptanceOutcome, AcceptError>
where
    V: SealedObjectVerifier,
    F: AcceptanceFaultInjector,
{
    if command.envelope_bytes != verified.source_envelope() {
        return Err(AcceptError::CommandEnvelopeMismatch);
    }

    // Event schema v3 arms are readable and verifiable, but an arm whose owner
    // has not written its closure path yet is refused with a typed error before
    // any SQL runs, so nothing is consumed and the reason is named.
    //
    // On a schema-1 profile the refusal covers every arm, and that is the only
    // fail-closed answer available: migration 0004 adds the closure tables and
    // widens the `ledger_event.event_kind` CHECK together, and the plaintext
    // lane's `STORE_MIGRATION_SQL` does not apply it, so a v3 arm has neither a
    // table nor an admitted kind there.
    for event in &verified.batch().events {
        if event.payload.registration().is_some() && Some(event.payload.kind()) != WRITABLE_V3_ARM {
            return Err(AcceptError::UnstorableEventKind(event.payload.kind()));
        }
    }

    // Resolve the complete transitive artifact-reference closure before SQL.
    // Opaque receipt values remain alive and are required by normalized writes
    // through commit; an artifact-id set alone is not an acceptance capability.
    let descriptors = preflight_artifact_closure(writer, verified.batch())?;
    let mut sealed_receipts = BTreeMap::<ArtifactId, V::Receipt>::new();
    for descriptor in descriptors.into_values() {
        let receipt = vault.verify_sealed_object(&descriptor).map_err(|source| {
            AcceptError::SealingFailed {
                artifact_id: descriptor.id,
                source: Box::new(source),
            }
        })?;
        if receipt.descriptor() != &descriptor {
            return Err(AcceptError::SealedReceiptMismatch {
                artifact_id: descriptor.id,
            });
        }
        sealed_receipts.insert(descriptor.id, receipt);
    }

    // The structural fingerprint was verified once, when this writer was
    // admitted. Re-comparing SQLite's schema cookie under the write lock binds
    // every committed acceptance to that exact schema, so a schema changed
    // under the running writer fails the acceptance closed instead of silently
    // altering the durable closure of a receipted batch.
    let admitted_schema_version = writer.admitted_schema_version();
    let _authorization = writer.authorize_acceptance();
    let transaction = writer.begin_immediate()?;
    verify_admitted_schema_version(&transaction, admitted_schema_version)?;
    faults.hit(AcceptanceFaultPoint::Db01)?;

    if let Some(receipt) = lookup_command_receipt(&transaction, &command)? {
        revalidate_sealed_receipts(vault, &mut sealed_receipts)?;
        transaction.commit()?;
        return Ok(AcceptanceOutcome {
            receipt,
            replayed_request: true,
            duplicate_batch: false,
        });
    }

    let state = read_replica_state(&transaction)?;
    if let Some(expected) = command.expected_revision
        && expected != state.profile_revision
    {
        return Err(AcceptError::ExpectedRevisionConflict {
            expected,
            actual: state.profile_revision,
        });
    }

    let batch = verified.batch();
    if let Some(stored) = find_batch(&transaction, batch.batch_id)? {
        if stored.envelope_hash != verified.envelope_hash()
            || stored.signed_envelope != verified.source_envelope()
        {
            return Err(LedgerError::BatchIdCollision.into());
        }
        let receipt = DurableAcceptanceReceipt::new(
            batch.batch_id,
            stored.envelope_hash,
            stored.accept_seq_start,
            stored.accept_seq_end,
            stored.committed_revision,
        );
        insert_command_receipt(&transaction, &command, &receipt, accepted_at)?;
        faults.hit(AcceptanceFaultPoint::Db02)?;
        revalidate_sealed_receipts(vault, &mut sealed_receipts)?;
        transaction.commit()?;
        faults.hit(AcceptanceFaultPoint::Db07)?;
        return Ok(AcceptanceOutcome {
            receipt,
            replayed_request: false,
            duplicate_batch: true,
        });
    }

    let device_head = read_device_head(&transaction, batch.device_id)?;
    match device_head {
        None => {
            if batch.origin_seq_start != 1 || batch.previous_batch_hash.is_some() {
                return Err(LedgerError::InvalidChainStart.into());
            }
        }
        Some(head) => {
            if batch.origin_seq_start > head.next_origin_seq {
                return Err(LedgerError::OriginGap {
                    expected: head.next_origin_seq,
                    actual: batch.origin_seq_start,
                }
                .into());
            }
            if batch.origin_seq_start < head.next_origin_seq
                || batch.previous_batch_hash != Some(head.envelope_hash)
            {
                return Err(LedgerError::DeviceFork.into());
            }
        }
    }

    let event_count =
        u64::try_from(batch.events.len()).map_err(|_| AcceptError::IntegerOverflow(u64::MAX))?;
    let accept_seq_start = state.next_accept_seq;
    let next_accept_seq = accept_seq_start
        .checked_add(event_count)
        .ok_or(LedgerError::AcceptSequenceExhausted)?;
    ensure_sqlite_coordinate(next_accept_seq)?;
    let accept_seq_end = next_accept_seq
        .checked_sub(1)
        .ok_or(LedgerError::AcceptSequenceExhausted)?;
    let committed_revision = state
        .profile_revision
        .checked_add(1)
        .ok_or(LedgerError::AcceptSequenceExhausted)?;
    ensure_sqlite_coordinate(committed_revision)?;
    let receipt = DurableAcceptanceReceipt::new(
        batch.batch_id,
        verified.envelope_hash(),
        accept_seq_start,
        accept_seq_end,
        committed_revision,
    );

    insert_batch(
        &transaction,
        verified,
        accept_seq_start,
        accept_seq_end,
        accepted_at,
    )?;
    insert_command_receipt(&transaction, &command, &receipt, accepted_at)?;
    faults.hit(AcceptanceFaultPoint::Db02)?;

    let midpoint = batch.events.len() / 2;
    let mut closure = ClosureWriter::new(&transaction, batch, &sealed_receipts);
    for (index, event) in batch.events.iter().enumerate() {
        let offset = u64::try_from(index).map_err(|_| AcceptError::IntegerOverflow(u64::MAX))?;
        let accept_seq = accept_seq_start
            .checked_add(offset)
            .ok_or(LedgerError::AcceptSequenceExhausted)?;
        closure.append_event(event, accept_seq)?;
        if index == midpoint {
            faults.hit(AcceptanceFaultPoint::Db03)?;
        }
    }
    faults.hit(AcceptanceFaultPoint::Db04)?;
    drop(closure);

    insert_outbox(
        &transaction,
        verified,
        accept_seq_start,
        accept_seq_end,
        committed_revision,
        accepted_at,
    )?;
    faults.hit(AcceptanceFaultPoint::Db05)?;

    update_heads(
        &transaction,
        batch,
        verified.envelope_hash(),
        next_accept_seq,
        committed_revision,
        accepted_at,
        device_head.is_some(),
    )?;
    faults.hit(AcceptanceFaultPoint::Db06)?;

    revalidate_sealed_receipts(vault, &mut sealed_receipts)?;
    transaction.commit()?;
    faults.hit(AcceptanceFaultPoint::Db07)?;
    Ok(AcceptanceOutcome {
        receipt,
        replayed_request: false,
        duplicate_batch: false,
    })
}

fn revalidate_sealed_receipts<V: SealedObjectVerifier>(
    vault: &V,
    sealed_receipts: &mut BTreeMap<ArtifactId, V::Receipt>,
) -> Result<(), AcceptError> {
    for (artifact_id, capability) in sealed_receipts {
        vault
            .revalidate_sealed_object(capability)
            .map_err(|source| AcceptError::SealingFailed {
                artifact_id: *artifact_id,
                source: Box::new(source),
            })?;
    }
    Ok(())
}

fn ensure_sqlite_coordinate(value: u64) -> Result<(), AcceptError> {
    i64::try_from(value)
        .map(|_| ())
        .map_err(|_| AcceptError::IntegerOverflow(value))
}

/// Resolves the complete transitive artifact-reference closure of one batch.
///
/// The descriptors this returns are the ones the vault is asked to
/// authenticate, so they must name the object the *current* key opens. An
/// `artifact_descriptor` row is inside a signed `ARTIFACT_REGISTERED` payload
/// and is INSERT-only twice over, so a rotation cannot write the new locator
/// over the old one; it appends the move to `artifact_descriptor_migration`
/// instead. This resolution walks that chain, which is what
/// `read_artifact_descriptors` does for backup and restore. Without it, every
/// acceptance whose closure reaches a rotated artifact is refused — under the
/// new key because the signed row names the superseded object, and after the
/// superseded object is retired under either key.
///
/// Descriptors that came out of this batch's own `ARTIFACT_REGISTERED` events
/// are resolved with the rest. An artifact registered in the batch being
/// accepted has no chain, so the walk is a no-op for it; passing it through the
/// same call is what keeps "resolved" from meaning two things here.
fn preflight_artifact_closure(
    writer: &WriterConnection,
    batch: &UnsignedBatch,
) -> Result<BTreeMap<ArtifactId, ArtifactDescriptor>, AcceptError> {
    let mut descriptors = BTreeMap::new();
    let mut new_evidence = BTreeMap::<EvidenceId, ArtifactId>::new();
    let mut new_claims = BTreeMap::<ClaimId, Vec<EvidenceId>>::new();
    for event in &batch.events {
        match &event.payload {
            EventPayload::ArtifactRegistered(descriptor) => {
                descriptors.insert(descriptor.id, descriptor.clone());
            }
            EventPayload::EvidenceRegistered(evidence) => {
                new_evidence.insert(evidence.id, evidence.artifact_id);
            }
            EventPayload::ClaimAsserted(claim) => {
                new_claims.insert(claim.id, claim.evidence_ids.clone());
            }
            EventPayload::ScopeRegistered(_)
            | EventPayload::ClaimRelated(_)
            | EventPayload::DecisionRecorded(_)
            | EventPayload::CurriculumVersionPublished(_)
            | EventPayload::CourseRevisionPublished(_)
            | EventPayload::OfferingObserved(_)
            | EventPayload::AttemptRecorded(_)
            | EventPayload::RequirementSetPublished(_)
            | EventPayload::AuditComputed(_)
            | EventPayload::CapturePermissionRecorded(_)
            | EventPayload::LectureSessionRecorded(_)
            | EventPayload::TranscriptVersionAdded(_)
            | EventPayload::LectureDocumentPublished(_)
            | EventPayload::SnapshotRegistered(_)
            | EventPayload::FindingPublished(_)
            | EventPayload::ModelRunRecorded(_)
            | EventPayload::ProposalDisposed(_)
            | EventPayload::EgressDecided(_)
            | EventPayload::ConsentRecorded(_)
            | EventPayload::EntityIdentityChanged(_)
            | EventPayload::RetentionActionRecorded(_) => {}
        }
    }

    let mut artifact_ids: BTreeSet<ArtifactId> = descriptors.keys().copied().collect();
    artifact_ids.extend(new_evidence.values().copied());
    let mut evidence_ids = BTreeSet::<EvidenceId>::new();
    let mut claim_ids = BTreeSet::<ClaimId>::new();
    for event in &batch.events {
        match &event.payload {
            EventPayload::ClaimAsserted(claim) => {
                evidence_ids.extend(claim.evidence_ids.iter().copied());
            }
            EventPayload::ClaimRelated(relation) => {
                claim_ids.insert(relation.source_claim_id);
                claim_ids.insert(relation.target_claim_id);
            }
            EventPayload::DecisionRecorded(decision) => {
                claim_ids.insert(decision.target_claim_id);
                if let DecisionAction::Replace {
                    replacement_claim_id,
                } = &decision.action
                {
                    claim_ids.insert(*replacement_claim_id);
                }
                evidence_ids.extend(decision.rationale_evidence_ids.iter().copied());
            }
            EventPayload::ScopeRegistered(_)
            | EventPayload::ArtifactRegistered(_)
            | EventPayload::EvidenceRegistered(_)
            | EventPayload::CurriculumVersionPublished(_)
            | EventPayload::CourseRevisionPublished(_)
            | EventPayload::OfferingObserved(_)
            | EventPayload::AttemptRecorded(_)
            | EventPayload::RequirementSetPublished(_)
            | EventPayload::AuditComputed(_)
            | EventPayload::CapturePermissionRecorded(_)
            | EventPayload::LectureSessionRecorded(_)
            | EventPayload::TranscriptVersionAdded(_)
            | EventPayload::LectureDocumentPublished(_)
            | EventPayload::SnapshotRegistered(_)
            | EventPayload::FindingPublished(_)
            | EventPayload::ModelRunRecorded(_)
            | EventPayload::ProposalDisposed(_)
            | EventPayload::EgressDecided(_)
            | EventPayload::ConsentRecorded(_)
            | EventPayload::EntityIdentityChanged(_)
            | EventPayload::RetentionActionRecorded(_) => {}
        }
    }

    for claim_id in claim_ids {
        if let Some(claim_evidence) = new_claims.get(&claim_id) {
            evidence_ids.extend(claim_evidence.iter().copied());
            continue;
        }
        let Some(claim_evidence) = preflight_claim_evidence(writer, claim_id)? else {
            return Err(LedgerError::UnknownClaim(claim_id).into());
        };
        evidence_ids.extend(claim_evidence);
    }

    for evidence_id in evidence_ids {
        if let Some(artifact_id) = new_evidence.get(&evidence_id) {
            artifact_ids.insert(*artifact_id);
            continue;
        }
        let Some(artifact_id) = preflight_evidence_artifact(writer, evidence_id)? else {
            return Err(LedgerError::UnknownEvidence(evidence_id).into());
        };
        artifact_ids.insert(artifact_id);
    }

    for artifact_id in artifact_ids {
        if descriptors.contains_key(&artifact_id) {
            continue;
        }
        let Some(descriptor) = preflight_artifact_descriptor(writer, artifact_id)? else {
            return Err(LedgerError::UnknownArtifact(artifact_id).into());
        };
        descriptors.insert(artifact_id, descriptor);
    }

    let mut resolved: Vec<ArtifactDescriptor> = descriptors.into_values().collect();
    writer.with_preflight_reader(|connection| {
        crate::descriptor_migration::resolve_with_stored_migrations(connection, &mut resolved)
    })?;
    let descriptors = resolved
        .into_iter()
        .map(|descriptor| (descriptor.id, descriptor))
        .collect();

    Ok(descriptors)
}
