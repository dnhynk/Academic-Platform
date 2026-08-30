//! The parts of `P2-K5` that touch real `AEAD_CHUNKED_V2` objects.
//!
//! Everything here needs the vault's encrypted lane, which is a non-default
//! feature, so it lives behind `rotation-engine`. The journal, the plan, the
//! vocabulary, and the revocation contract do not, and stay in the default lane.

use std::{fs, path::PathBuf};

use academic_crypto::{DomainKek, KEY_BYTES, ProfileId, VaultMasterKey};
use academic_domain::{ArtifactDescriptor, DomainId, VaultLocator};
use academic_vault::{
    ENCRYPTED_FORMAT_VERSION, EncryptedVault, VaultError,
    object::{self, HEADER_BYTES, ObjectFormatError},
    shred_key_slot_at,
};

use crate::{
    entry::{JournalEntry, UnitKind},
    fault::{self, FaultPoint},
    journal::{AppendOnlyJournal, JournalError},
    rotation::{
        CanonicalReference, CanonicalReferenceError, OpeningGeneration, RotationError, RotationPlan,
        RotationState, RotationUnit, StoreDatabaseError, StoreDatabaseExecutor, StoreDatabaseRekey,
        UnitProgress, store_database_target_id,
    },
    tombstone::{BackupTombstone, TombstoneError},
};

/// Why a rotation or a shred could not be carried out.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EngineError {
    /// The vault refused.
    #[error("the object vault refused: {0}")]
    Vault(#[from] VaultError),
    /// The journal refused.
    #[error("the journal refused: {0}")]
    Journal(#[from] JournalError),
    /// The rotation refused.
    #[error("the rotation refused: {0}")]
    Rotation(#[from] RotationError),
    /// A tombstone refused.
    #[error("the tombstone refused: {0}")]
    Tombstone(#[from] TombstoneError),
    /// A file could not be read.
    #[error("{operation} failed for {path}: {source}")]
    Io {
        /// What was being attempted.
        operation: &'static str,
        /// Path involved.
        path: PathBuf,
        /// Underlying failure.
        #[source]
        source: std::io::Error,
    },
    /// The plan holds a store database unit that was never rekeyed.
    ///
    /// The store database is planned and journalled here and rekeyed through
    /// [`StoreDatabaseExecutor`], which the encrypted portability lane binds to
    /// `P2-K2`'s `PRAGMA rekey`. A rotation that never ran that unit's executor
    /// stops rather than recording a migration that did not happen, and it
    /// stops by naming the database: "the executor never ran" and "the caller
    /// forgot a descriptor" are different facts and only one is fixable here.
    #[error(
        "unit {0} is the store database and this rotation never ran its \
         executor, so it will not record it as migrated"
    )]
    StoreDatabaseNotMigrated(String),
    /// A retirement was attempted for a unit that names no object.
    #[error("unit {0} names no object, so it has no superseded object to retire")]
    NotAnObjectUnit(String),

    /// A store database rotation was handed a unit that is not one.
    #[error("unit {0} is not the store database, so it cannot be rekeyed")]
    NotAStoreDatabaseUnit(String),
    /// The bound store database executor refused.
    #[error("unit {unit_id} could not be rekeyed: {source}")]
    StoreDatabaseExecutor {
        /// Unit that was being rekeyed.
        unit_id: String,
        /// What the executor reported.
        #[source]
        source: StoreDatabaseError,
    },
    /// The canonical reference could not be read.
    #[error("unit {unit_id} cannot retire its superseded object: {source}")]
    CanonicalReference {
        /// Unit that was being retired.
        unit_id: String,
        /// What the reference reported.
        #[source]
        source: CanonicalReferenceError,
    },
    /// A retirement named a superseded object that is not the unit's own.
    ///
    /// A unit identity is derived from its source locator, so the two are
    /// comparable without reading anything. Without this check the gates
    /// inspect one object and the positioned write destroys another.
    #[error(
        "unit {unit_id} cannot retire {superseded}: the object that unit \
         supersedes is {expected}"
    )]
    UnitDoesNotNameSupersededObject {
        /// Unit that was being retired.
        unit_id: String,
        /// Locator the caller passed as superseded.
        superseded: String,
        /// Locator the unit actually supersedes.
        expected: String,
    },
    /// The store resolves the artifact to something other than the migration target.
    ///
    /// Either the store row has not been written yet, or the reference moved
    /// somewhere this rotation did not put it. Both are refusals: the object
    /// the store still names must never be the one whose key slot is destroyed.
    #[error(
        "unit {unit_id} cannot retire its superseded object: the store resolves \
         the artifact to {resolved}, not to the {target} this rotation migrated it to"
    )]
    ReferenceIsNotTheMigrationTarget {
        /// Unit that was being retired.
        unit_id: String,
        /// Locator the store resolves to.
        resolved: String,
        /// Locator the journal recorded as this unit's migration target.
        target: String,
    },
    /// The store holds no descriptor for the artifact being retired.
    #[error(
        "unit {unit_id} cannot retire its superseded object: the store holds no \
         descriptor for that artifact"
    )]
    ArtifactAbsentFromStore {
        /// Unit that was being retired.
        unit_id: String,
    },
    /// The descriptor set does not cover the plan.
    #[error("the rotation plan names unit {0}, for which no descriptor was supplied")]
    DescriptorMissing(String),
    /// A retirement was attempted before the rotation recorded its completion.
    #[error(
        "unit {0} cannot retire its superseded object: the rotation has not recorded \
         its completion, so destroying it could leave an artifact no key opens"
    )]
    RotationNotComplete(String),
    /// A retirement was attempted for a unit whose reachability has not moved.
    #[error(
        "unit {0} cannot retire its superseded object: the journal does not record \
         the unit as migrated"
    )]
    UnitNotMigrated(String),
    /// A retirement was attempted while the canonical reference still names it.
    #[error(
        "unit {0} cannot retire its superseded object: the descriptor the store \
         resolves to is still the superseded one"
    )]
    ReferenceNotMoved(String),
}

/// Returns `descriptor` with the locator a keyring holding `kek` would give it.
///
/// The locator derives from the domain KEK, so a rotation moves every object to
/// a new path. That is what makes the source and target objects two files
/// rather than one file rewritten in place.
pub fn rebind_locator(
    descriptor: &ArtifactDescriptor,
    kek: &DomainKek,
    profile: ProfileId,
) -> Result<ArtifactDescriptor, EngineError> {
    let locator_key = kek
        .derive_locator_key(profile)
        .map_err(|_| EngineError::Rotation(RotationError::KeySchedule))?;
    let locator = VaultLocator::derive(
        locator_key.expose_secret(),
        ENCRYPTED_FORMAT_VERSION,
        &descriptor.media_type,
        descriptor.content_digest,
    )
    .map_err(VaultError::from)?;
    let mut rebound = descriptor.clone();
    rebound.vault_locator = locator;
    Ok(rebound)
}

/// What a probe of one object under one key observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HeaderProbe {
    /// The key authenticated the header.
    Opened,
    /// The header is present but this key does not open it.
    WrongKey,
    /// The key slot was destroyed by a crypto-shred.
    Shredded,
    /// There is no object at that path.
    Missing,
    /// The file is present but is not a readable header.
    Unreadable,
}

/// Probes one object file under one domain key, producing no plaintext.
///
/// The header tag is the whole of the check: it verifies before any chunk is
/// touched, so a wrong key, a wrong domain, and a tampered header all fail here.
pub fn probe_header(path: &std::path::Path, kek: &DomainKek) -> HeaderProbe {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return HeaderProbe::Missing;
        }
        Err(_) => return HeaderProbe::Unreadable,
    };
    if bytes.len() < HEADER_BYTES {
        return HeaderProbe::Unreadable;
    }
    let key: &[u8; KEY_BYTES] = kek.expose_secret();
    match object::open_header(&bytes[..HEADER_BYTES], key) {
        Ok(_) => HeaderProbe::Opened,
        Err(ObjectFormatError::Shredded) => HeaderProbe::Shredded,
        Err(ObjectFormatError::Aead) => HeaderProbe::WrongKey,
        Err(_) => HeaderProbe::Unreadable,
    }
}

/// Which of the two generations opens one unit's reachable object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningObservation {
    /// Only the source generation's key opens it. This is correct before the
    /// unit migrates.
    OnlySource,
    /// Only the target generation's key opens it. This is correct after.
    OnlyTarget,
    /// Both keys open it. A violation: the rotation did not change the key.
    Both,
    /// Neither key opens it. A violation: reachability moved to something that
    /// is not there, or the object is gone.
    Neither,
}

impl OpeningObservation {
    /// Reports whether the invariant holds for this observation.
    #[must_use]
    pub const fn is_exactly_one(self) -> bool {
        matches!(self, Self::OnlySource | Self::OnlyTarget)
    }

    /// Reports whether the observation agrees with what the journal says.
    #[must_use]
    pub const fn agrees_with(self, generation: OpeningGeneration) -> bool {
        matches!(
            (self, generation),
            (Self::OnlySource, OpeningGeneration::Source)
                | (Self::OnlyTarget, OpeningGeneration::Target)
        )
    }
}

/// The two keyed views a rotation moves an object between.
///
/// Both KEKs are one domain's. A rotation spans every domain, but the invariant
/// is checked one object at a time and an object belongs to exactly one domain,
/// which the descriptor already names.
#[derive(Debug)]
pub struct RotationKeys<'a> {
    /// Profile identity both generations are salted with.
    pub profile: ProfileId,
    /// The domain KEK of the generation being rotated away from.
    pub source_kek: &'a DomainKek,
    /// The domain KEK of the generation being rotated to.
    pub target_kek: &'a DomainKek,
}

/// Observes which generation opens one unit's **reachable** object.
///
/// The reachable object is decided by the journal, never by which files happen
/// to exist: before `UnitMigrated` the source object is reachable and the
/// target object — if it exists at all — is an unreferenced orphan the vault's
/// reconciliation quarantines. Probing "whichever file is there" would report
/// `Both` for every correctly interrupted rotation, which is why the journal is
/// the authority.
pub fn observe_reachable_opening(
    vault: &EncryptedVault,
    keys: &RotationKeys<'_>,
    source_descriptor: &ArtifactDescriptor,
    progress: UnitProgress,
) -> Result<OpeningObservation, EngineError> {
    // Both generations live in one profile's `vault/v2` tree: the locator
    // derives from the domain KEK, so a rotation moves an object to a new path
    // rather than rewriting it. Either vault's layout resolves both paths,
    // because a path is a function of the descriptor and not of the keyring.
    let layout = vault.layout();
    let reachable = match progress.opening_generation() {
        OpeningGeneration::Source => {
            rebind_locator(source_descriptor, keys.source_kek, keys.profile)?
        }
        OpeningGeneration::Target => {
            rebind_locator(source_descriptor, keys.target_kek, keys.profile)?
        }
    };
    let path = layout.object_path(&reachable)?;
    let source_opens = probe_header(&path, keys.source_kek) == HeaderProbe::Opened;
    let target_opens = probe_header(&path, keys.target_kek) == HeaderProbe::Opened;
    Ok(match (source_opens, target_opens) {
        (true, false) => OpeningObservation::OnlySource,
        (false, true) => OpeningObservation::OnlyTarget,
        (true, true) => OpeningObservation::Both,
        (false, false) => OpeningObservation::Neither,
    })
}

/// One rotation in progress over a source and a target object namespace.
#[derive(Debug)]
pub struct RotationEngine<'a> {
    plan: &'a RotationPlan,
    source: &'a EncryptedVault,
    target: &'a EncryptedVault,
}

impl<'a> RotationEngine<'a> {
    /// Binds a plan to the two vaults it moves objects between.
    #[must_use]
    pub const fn new(
        plan: &'a RotationPlan,
        source: &'a EncryptedVault,
        target: &'a EncryptedVault,
    ) -> Self {
        Self {
            plan,
            source,
            target,
        }
    }

    /// Opens the rotation by journalling its plan.
    ///
    /// Nothing is moved here. A kill immediately after leaves a journal that
    /// enumerates every unit as remaining, which is exactly what a resume needs.
    pub fn begin(&self, journal: &mut AppendOnlyJournal) -> Result<(), EngineError> {
        journal.append(self.plan.started_entry())?;
        Ok(())
    }

    /// Moves one object unit, in the order the invariant depends on.
    ///
    /// 1. re-seal into the target namespace; the vault publishes, reads the
    ///    object back, and authenticates it before returning;
    /// 2. append `UnitResealed`, which says the target object is durable and
    ///    verified but **not** reachable;
    /// 3. append `UnitMigrated`, which moves reachability.
    ///
    /// The source object is not touched. It becomes unreferenced at step 3 and
    /// the vault's own reconciliation quarantines it after the grace window,
    /// which is why a kill between the steps can never leave an artifact that
    /// neither key opens.
    pub fn rotate_object(
        &self,
        journal: &mut AppendOnlyJournal,
        unit: &RotationUnit,
        source_descriptor: &ArtifactDescriptor,
    ) -> Result<ArtifactDescriptor, EngineError> {
        fault::trip(FaultPoint::Ky03BeforeReseal);
        let outcome = self.source.reseal(source_descriptor, self.target)?;
        let resealed = outcome.resealed.descriptor().clone();
        fault::trip(FaultPoint::Ky03AfterReseal);

        journal.append(JournalEntry::UnitResealed {
            rotation_id: self.plan.rotation_id().to_hex(),
            unit_id: unit.unit_id_hex(),
            target_locator: hex::encode(resealed.vault_locator.as_bytes()),
        })?;
        fault::trip(FaultPoint::Ky03AfterResealRecord);

        journal.append(JournalEntry::UnitMigrated {
            rotation_id: self.plan.rotation_id().to_hex(),
            unit_id: unit.unit_id_hex(),
        })?;
        fault::trip(FaultPoint::Ky03AfterMigrateRecord);
        Ok(resealed)
    }

    /// Rekeys the profile database unit, in the order the invariant depends on.
    ///
    /// `SKEY_p` and `KEK_d` are both functions of the Vault Master Key, so a
    /// rotation that moves every object and leaves the database behind produces
    /// a profile whose two halves are under two generations. That profile still
    /// works locally — both keys are in the one process that holds the master —
    /// and it is not restorable, because a restore re-derives both halves from
    /// the single master it recovered from the backup. This unit is what stops
    /// that, and `executor` is the store lane's `PRAGMA rekey`.
    ///
    /// The order differs from an object unit's for one reason: a rekey rewrites
    /// pages in place, so there is no second file that can be durable and
    /// verified while the first one is still reachable. What takes the place of
    /// the read-back is the executor's own re-open under the target key, which
    /// it performs before returning. `UnitResealed` is therefore appended after
    /// the database has been proved to open under the target generation, and
    /// `UnitMigrated` immediately after it.
    ///
    /// A kill during the rekey leaves exactly one working key — that is fault
    /// `EN01`, executed in the `encrypted-store-lane` job — and a journal that
    /// still says the source generation is in force. The resume re-runs this
    /// method, the executor reports [`StoreDatabaseRekey::AlreadyAtTarget`],
    /// and the two records catch up. A kill between the two records is the same
    /// window an object unit has, and it is repaired the same way.
    pub fn rotate_store_database(
        &self,
        journal: &mut AppendOnlyJournal,
        unit: &RotationUnit,
        executor: &dyn StoreDatabaseExecutor,
    ) -> Result<StoreDatabaseRekey, EngineError> {
        if unit.kind() != UnitKind::StoreDatabase {
            return Err(EngineError::NotAStoreDatabaseUnit(unit.unit_id_hex()));
        }
        let outcome = executor.rekey_store_database().map_err(|source| {
            EngineError::StoreDatabaseExecutor {
                unit_id: unit.unit_id_hex(),
                source,
            }
        })?;

        journal.append(JournalEntry::UnitResealed {
            rotation_id: self.plan.rotation_id().to_hex(),
            unit_id: unit.unit_id_hex(),
            target_locator: hex::encode(store_database_target_id(
                self.plan.profile_id(),
                self.plan.target(),
            )),
        })?;
        journal.append(JournalEntry::UnitMigrated {
            rotation_id: self.plan.rotation_id().to_hex(),
            unit_id: unit.unit_id_hex(),
        })?;
        Ok(outcome)
    }


    /// Records that every planned unit migrated.
    ///
    /// Refuses while any unit is still remaining, so a `RotationCompleted`
    /// record can never be read as covering a unit that did not move.
    pub fn complete(&self, journal: &mut AppendOnlyJournal) -> Result<(), EngineError> {
        let Some(state) = RotationState::replay(journal.entries())? else {
            return Err(EngineError::Rotation(RotationError::EmptyPlan));
        };
        if let Some(unit) = state.remaining().first() {
            // The store database is named separately, because "this build
            // cannot run it" and "the caller forgot a descriptor" are different
            // facts and only one of them is fixable here.
            return Err(match unit.unit.kind() {
                UnitKind::StoreDatabase => {
                    EngineError::StoreDatabaseNotMigrated(unit.unit.unit_id_hex())
                }
                UnitKind::Object => EngineError::DescriptorMissing(unit.unit.unit_id_hex()),
            });
        }
        let unit_count = u64::try_from(self.plan.units().len()).unwrap_or(u64::MAX);
        journal.append(JournalEntry::RotationCompleted {
            rotation_id: self.plan.rotation_id().to_hex(),
            unit_count,
        })?;
        Ok(())
    }
}

/// Domain separator for the digest a retirement's destroyed key slot names.
pub const RETIREMENT_DIGEST_DOMAIN: &[u8] = b"academic-os/rotation-retirement/v1";

/// Returns the digest that labels one unit's retired source object.
///
/// It names the rotation, the unit, and both locators, so the marker left in
/// the retired file points at the exact move that made it unreferenced. It is
/// deliberately not a [`BackupTombstone`] digest: a retirement is not a
/// deletion of the artifact, and a restore must not re-apply it to a backup
/// copy of an artifact that still exists.
#[must_use]
pub fn retirement_digest(
    rotation_id: &str,
    unit_id: &str,
    source_locator: &[u8; 32],
    target_locator: &[u8; 32],
) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(RETIREMENT_DIGEST_DOMAIN);
    hasher.update([0]);
    hasher.update(rotation_id.as_bytes());
    hasher.update([0]);
    hasher.update(unit_id.as_bytes());
    hasher.update([0]);
    hasher.update(source_locator);
    hasher.update(target_locator);
    hasher.finalize().into()
}

/// Destroys the key slot of one completed rotation's superseded object.
///
/// This is the garbage collection `ADR-004` leaves open, and it is what makes
/// t068 section 5's `P2-K5` sentence — exactly one of the old and the new key
/// opens any object — true of the files on disk rather than only of the object
/// the journal calls reachable. Until a unit is retired, a holder of the
/// superseded generation's key still opens the superseded copy in the live
/// tree, including a recipient the rotation revoked.
///
/// Every argument is bound to something already recorded, because a retirement
/// is a positioned write that cannot be undone and this function is, until a
/// rotation orchestrator exists, the only collection point there is:
///
/// 1. the rotation recorded its completion and this unit recorded its migration;
/// 2. `superseded` is the object **this unit** supersedes — the unit identity is
///    derived from its source locator, so the two are comparable here without
///    reading anything;
/// 3. the canonical reference the store resolves that artifact to is the
///    `target_locator` this unit's `UnitResealed` record named, which is how a
///    caller that has not yet written the store row is refused rather than
///    believed.
///
/// Together those refuse the three orderings that destroy live data: retiring
/// before the store row exists, retiring another artifact's live object under a
/// rotated unit, and retiring the object the rotation moved *to*.
///
/// A kill during the marker write is safe for the same reason a shred is: the
/// write is positioned and idempotent, and re-running reaches the same state.
/// A kill before the journal record leaves an object already retired and a
/// journal that does not say so, which a resume repairs by re-running.
pub fn retire_superseded_object(
    journal: &mut AppendOnlyJournal,
    vault: &EncryptedVault,
    unit: &RotationUnit,
    superseded: &ArtifactDescriptor,
    reference: &dyn CanonicalReference,
) -> Result<[u8; 32], EngineError> {
    let Some(state) = RotationState::replay(journal.entries())? else {
        return Err(EngineError::Rotation(RotationError::EmptyPlan));
    };
    if !state.is_complete() {
        return Err(EngineError::RotationNotComplete(unit.unit_id_hex()));
    }
    let Some(migrated) = state
        .migrated()
        .into_iter()
        .find(|migrated| migrated.unit.unit_id_hex() == unit.unit_id_hex())
    else {
        return Err(EngineError::UnitNotMigrated(unit.unit_id_hex()));
    };

    let expected_source = unit
        .source_locator()
        .ok_or_else(|| EngineError::NotAnObjectUnit(unit.unit_id_hex()))?;
    if expected_source != superseded.vault_locator.as_bytes() {
        return Err(EngineError::UnitDoesNotNameSupersededObject {
            unit_id: unit.unit_id_hex(),
            superseded: hex::encode(superseded.vault_locator.as_bytes()),
            expected: hex::encode(expected_source),
        });
    }

    let target_locator = migrated
        .target_locator
        .clone()
        .ok_or_else(|| EngineError::UnitNotMigrated(unit.unit_id_hex()))?;
    let resolved = reference
        .resolved_locator(superseded.id)
        .map_err(|source| EngineError::CanonicalReference {
            unit_id: unit.unit_id_hex(),
            source,
        })?
        .ok_or_else(|| EngineError::ArtifactAbsentFromStore {
            unit_id: unit.unit_id_hex(),
        })?;
    if hex::encode(resolved) != target_locator {
        return Err(EngineError::ReferenceIsNotTheMigrationTarget {
            unit_id: unit.unit_id_hex(),
            resolved: hex::encode(resolved),
            target: target_locator,
        });
    }
    if resolved == *superseded.vault_locator.as_bytes() {
        return Err(EngineError::ReferenceNotMoved(unit.unit_id_hex()));
    }

    let digest = retirement_digest(
        state.rotation_id(),
        &unit.unit_id_hex(),
        superseded.vault_locator.as_bytes(),
        &resolved,
    );
    let path = vault.layout().object_path(superseded)?;
    shred_key_slot_at(&path, &digest)?;
    journal.append(JournalEntry::UnitSourceRetired {
        rotation_id: state.rotation_id().to_owned(),
        unit_id: unit.unit_id_hex(),
        source_locator: hex::encode(superseded.vault_locator.as_bytes()),
        retirement_digest: hex::encode(digest),
    })?;
    Ok(digest)
}

/// Crypto-shreds one object and records the tombstone that authorized it.
///
/// The journal is a log of facts, so `ArtifactShredded` is appended after the
/// slot is destroyed rather than before. What makes a kill in between safe is
/// that the action's `RetentionPlanned` record is already durable — `settle`
/// writes it before anything is deleted — and that
/// [`EncryptedVault::shred_key_slot`] is idempotent, so a resume that re-runs
/// an unsettled action reaches the same state.
///
/// `RB01`'s "shredded or intact" therefore holds materially at every point: a
/// kill before the write leaves the object intact, a kill after it leaves the
/// object shredded, and a kill *during* the 80-byte write has already destroyed
/// the key even if the marker is incomplete — which a re-application repairs
/// into a properly labelled shred.
pub fn shred_with_tombstone(
    journal: &mut AppendOnlyJournal,
    vault: &EncryptedVault,
    descriptor: &ArtifactDescriptor,
    tombstone: &BackupTombstone,
) -> Result<(), EngineError> {
    let digest = tombstone.digest();
    let receipt = vault.shred_key_slot(descriptor, &digest)?;
    journal.append(JournalEntry::ArtifactShredded {
        action_id: tombstone.action_id.clone(),
        locator: hex::encode(receipt.locator()),
        tombstone_digest: tombstone.digest_hex(),
    })?;
    Ok(())
}

/// Re-applies every tombstone a backup carries to a restored object tree.
///
/// This is the restore half of a deletion. It needs **no key**: the locator is
/// cleartext at a fixed header offset and destroying a key slot is a positioned
/// write, so a restore onto a fresh machine re-deletes before anything is
/// unlocked.
///
/// A tombstone names the locator the object had when it was shredded and every
/// locator the artifact's reference chain moved through before that. A locator
/// is a function of the domain KEK, so a rotation gives the same artifact a new
/// one; a backup taken before the rotation holds the object under an older
/// name, and matching on the current locator alone would leave that copy
/// readable. Whichever of an artifact's names a backup happens to hold, one
/// tombstone reaches it.
///
/// Only the 208-byte header is read. The locator is inside it and nothing else
/// here looks at the ciphertext, so an object of any size costs one header.
///
/// Returns the locators it re-deleted, sorted, and the tombstones that matched
/// no object in the tree.
pub fn apply_tombstones(
    objects_root: &std::path::Path,
    tombstones: &[BackupTombstone],
) -> Result<AppliedTombstones, EngineError> {
    let mut wanted = std::collections::BTreeMap::new();
    for tombstone in tombstones {
        for locator in tombstone.covered_locators()? {
            wanted.insert(locator, tombstone);
        }
    }
    let mut applied = Vec::new();
    let mut reached: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut objects = Vec::new();
    collect_objects(objects_root, &mut objects)?;
    for path in objects {
        let mut header = [0_u8; HEADER_BYTES];
        {
            use std::io::Read as _;
            let Ok(mut file) = fs::File::open(&path) else {
                continue;
            };
            if file.read_exact(&mut header).is_err() {
                continue;
            }
        }
        let Ok(locator) = object::read_locator(&header) else {
            continue;
        };
        if let Some(tombstone) = wanted.remove(&locator) {
            shred_key_slot_at(&path, &tombstone.digest())?;
            applied.push(hex::encode(locator));
            reached.insert(tombstone.locator.clone());
        }
    }
    applied.sort();
    let mut absent: Vec<String> = wanted
        .values()
        .map(|value| value.locator.clone())
        .filter(|locator| !reached.contains(locator))
        .collect();
    absent.sort();
    absent.dedup();
    Ok(AppliedTombstones { applied, absent })
}

/// What a tombstone application reached, and what it did not find.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedTombstones {
    /// Locators re-deleted, sorted.
    pub applied: Vec<String>,
    /// Tombstones that reached no object in the tree, named by the locator the
    /// live shred destroyed, sorted.
    ///
    /// A tombstone that matched under one of the artifact's earlier names is
    /// not here: it reached its object. This list is what a restore receipt
    /// carries so a deletion that could not be re-applied is reported rather
    /// than dropped.
    pub absent: Vec<String>,
}

fn collect_objects(root: &std::path::Path, into: &mut Vec<PathBuf>) -> Result<(), EngineError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(EngineError::Io {
                operation: "enumerate restored objects",
                path: root.to_path_buf(),
                source,
            });
        }
    };
    for entry in entries {
        let entry = entry.map_err(|source| EngineError::Io {
            operation: "read restored object entry",
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| EngineError::Io {
            operation: "inspect restored object entry",
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_objects(&path, into)?;
        } else if metadata.is_file() {
            into.push(path);
        }
    }
    Ok(())
}

/// Derives both generations' domain KEKs for one domain.
pub fn domain_keks(
    source: &VaultMasterKey,
    target: &VaultMasterKey,
    profile: ProfileId,
    domain: DomainId,
) -> Result<(DomainKek, DomainKek), EngineError> {
    let crypto_domain = academic_crypto::DomainId::from_bytes(*domain.as_bytes());
    let source_kek = source
        .derive_domain_kek(profile, crypto_domain)
        .map_err(|_| EngineError::Rotation(RotationError::KeySchedule))?;
    let target_kek = target
        .derive_domain_kek(profile, crypto_domain)
        .map_err(|_| EngineError::Rotation(RotationError::KeySchedule))?;
    Ok((source_kek, target_kek))
}
