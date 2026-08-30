//! How a re-sealed object's canonical reference moves without an edit.
//!
//! A `P2-K5` rotation derives a new domain KEK, so the domain-keyed locator —
//! and with it the object's canonical path — changes. The locator that the
//! store holds is inside the signed `ARTIFACT_REGISTERED` payload and
//! `artifact_descriptor` is INSERT-only twice over, so the new locator cannot
//! be written over the old one. It is appended beside it instead, in
//! `artifact_descriptor_migration`, and readers resolve the current locator by
//! walking that chain from the signed one.
//!
//! # What authorizes one migration
//!
//! The existing event schema v3 arm `RETENTION_ACTION_RECORDED`. Its
//! registration frame carries an optional provenance digest, and that digest is
//! the [`DescriptorMigration::record_digest`] of the exact locator pair below.
//! Migration `0005`'s triggers refuse a row whose digest is not the one its
//! retention action carries and refuse a row that does not continue the chain,
//! so a reference cannot move without an accepted canonical event naming
//! precisely where it moved to.
//!
//! # Ordering, and what a kill between the two leaves
//!
//! The event is accepted first and the typed row is written second. A kill
//! between them leaves the reference where it was: the chain is shorter than
//! the event history, [`resolve_descriptors`] still yields the superseded
//! locator, and the superseded object is still the reachable one. Re-running
//! [`record_descriptor_migration`] completes it. The opposite order would
//! briefly point the store at an object no accepted event had authorized.

use academic_domain::{ArtifactDescriptor, ArtifactId, ContentDigest, VaultLocator};
use academic_vault::{SealedObjectVerifier, VaultError};
use rusqlite::{Connection, OptionalExtension as _, params};

use crate::error::{StoreError, StoreResult};

fn rejected(reason: &'static str) -> StoreError {
    StoreError::DescriptorMigrationRejected {
        reason,
        detail: String::new(),
    }
}

/// Domain separator for the record digest a retention action authorizes.
pub const DESCRIPTOR_MIGRATION_DIGEST_DOMAIN: &[u8] = b"academic-os/descriptor-migration/v1";

/// One appended move of one artifact's canonical object reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorMigration {
    /// Aggregate identity of the `RETENTION_ACTION_RECORDED` event authorizing it.
    pub retention_action_id: [u8; 16],
    /// Opaque identifier bytes of the artifact whose reference moves.
    ///
    /// Bytes rather than [`ArtifactId`] because this is read back out of the
    /// column the store wrote and is only ever compared with a descriptor's
    /// own bytes; nothing here re-validates an identifier the acceptance
    /// transaction already validated.
    pub artifact_id: [u8; 16],
    /// Position in this artifact's chain; the first migration is `1`.
    pub migration_seq: u64,
    /// Locator this migration supersedes.
    pub superseded_locator: VaultLocator,
    /// Locator the object moved to.
    pub vault_locator: VaultLocator,
    /// Object format version at the new locator.
    pub format_version: u16,
}

impl DescriptorMigration {
    /// Returns the digest the authorizing event must carry as its `source_digest`.
    ///
    /// Every field that decides where the reference points is inside it, so an
    /// event authorizes one exact move rather than "some move of this object".
    #[must_use]
    pub fn record_digest(&self) -> ContentDigest {
        let mut bytes = Vec::with_capacity(128);
        bytes.extend_from_slice(DESCRIPTOR_MIGRATION_DIGEST_DOMAIN);
        bytes.push(0);
        bytes.extend_from_slice(&self.retention_action_id);
        bytes.extend_from_slice(&self.artifact_id);
        bytes.extend_from_slice(&self.migration_seq.to_le_bytes());
        bytes.extend_from_slice(self.superseded_locator.as_bytes().as_slice());
        bytes.extend_from_slice(self.vault_locator.as_bytes().as_slice());
        bytes.extend_from_slice(&self.format_version.to_le_bytes());
        ContentDigest::sha256(&bytes)
    }

    /// Builds the record for one artifact's next move.
    #[must_use]
    pub fn of(
        retention_action_id: [u8; 16],
        descriptor: &ArtifactDescriptor,
        migration_seq: u64,
        migrated: &ArtifactDescriptor,
    ) -> Self {
        Self {
            retention_action_id,
            artifact_id: *descriptor.id.as_bytes(),
            migration_seq,
            superseded_locator: descriptor.vault_locator.clone(),
            vault_locator: migrated.vault_locator.clone(),
            format_version: migrated.format_version,
        }
    }

    /// Reports whether this migration names `descriptor`'s artifact.
    #[must_use]
    pub fn names(&self, descriptor: &ArtifactDescriptor) -> bool {
        self.artifact_id == *descriptor.id.as_bytes()
    }

    /// Applies this migration's locator and format to `descriptor`.
    pub fn apply_to(&self, descriptor: &mut ArtifactDescriptor) {
        descriptor.vault_locator = self.vault_locator.clone();
        descriptor.format_version = self.format_version;
    }
}

/// Reads every recorded descriptor migration in chain order.
///
/// Ordered by artifact and then by sequence, which is the order
/// [`resolve_descriptors`] walks.
pub fn read_descriptor_migrations(
    connection: &Connection,
) -> StoreResult<Vec<DescriptorMigration>> {
    if !migration_table_exists(connection)? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(concat!(
        "SELECT retention_action_id, artifact_id, migration_seq, superseded_locator, ",
        "vault_locator, format_version FROM artifact_descriptor_migration ",
        "ORDER BY artifact_id, migration_seq"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, Vec<u8>>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;
    let mut migrations = Vec::new();
    for row in rows {
        let (action, artifact, sequence, superseded, moved, format) = row?;
        migrations.push(DescriptorMigration {
            retention_action_id: identifier(&action)?,
            artifact_id: identifier(&artifact)?,
            migration_seq: u64::try_from(sequence)
                .map_err(|_| rejected("a stored migration sequence is negative"))?,
            superseded_locator: locator(&superseded)?,
            vault_locator: locator(&moved)?,
            format_version: u16::try_from(format)
                .map_err(|_| rejected("a stored migration format version is out of range"))?,
        });
    }
    Ok(migrations)
}

/// Applies every recorded migration to the descriptors it names.
///
/// A descriptor with no migration is returned unchanged, and a descriptor with
/// several is walked to the end of its chain. The chain's continuity is
/// enforced by migration `0005`; this refuses anyway rather than silently
/// resolving to a fork, because the reader and the writer are separate
/// processes and this is the read that decides which object is reachable.
pub fn resolve_descriptors(
    descriptors: &mut [ArtifactDescriptor],
    migrations: &[DescriptorMigration],
) -> StoreResult<()> {
    for descriptor in descriptors.iter_mut() {
        let chain: Vec<&DescriptorMigration> = migrations
            .iter()
            .filter(|migration| migration.names(descriptor))
            .collect();
        let mut sequence = 0_u64;
        for migration in chain {
            if migration.migration_seq != sequence.saturating_add(1)
                || migration.superseded_locator != descriptor.vault_locator
            {
                return Err(rejected(
                    "a stored migration does not continue the artifact's reference chain",
                ));
            }
            migration.apply_to(descriptor);
            sequence = migration.migration_seq;
        }
    }
    Ok(())
}

/// Reads the descriptors a caller already holds and resolves them in one step.
pub fn resolve_with_stored_migrations(
    connection: &Connection,
    descriptors: &mut [ArtifactDescriptor],
) -> StoreResult<()> {
    let migrations = read_descriptor_migrations(connection)?;
    resolve_descriptors(descriptors, &migrations)
}

/// Returns every locator one artifact was reachable under before now.
///
/// In chain order, oldest first: the locator the signed row named, then each
/// one a migration superseded. It does not include the locator the chain now
/// resolves to.
///
/// A deletion writes these into its backup tombstone. A locator is a function
/// of the domain KEK, so a rotation gives an artifact a new one and a backup
/// taken before the rotation holds the object under an older name; a tombstone
/// naming only the current locator would leave that copy readable.
pub fn superseded_locators(
    connection: &Connection,
    artifact: ArtifactId,
) -> StoreResult<Vec<VaultLocator>> {
    let migrations = read_descriptor_migrations(connection)?;
    Ok(migrations
        .iter()
        .filter(|migration| migration.artifact_id == *artifact.as_bytes())
        .map(|migration| migration.superseded_locator.clone())
        .collect())
}

/// Reports whether one artifact's chain has already recorded `locator`.
///
/// The chain refuses to record the same destination twice — migration `0005`
/// carries `UNIQUE (artifact_id, vault_locator)` and `UNIQUE (artifact_id,
/// superseded_locator)`, which is what stops a chain from forking or looping.
/// A locator is a deterministic function of the generation, so rotating back to
/// a generation the artifact has already been under would try to record a
/// locator that is already in its chain.
///
/// A rotation orchestrator calls this **before** the journal records anything.
/// The journal moves first and the store row second, so discovering the refusal
/// at the insert leaves a journal that says the unit migrated and a store that
/// still resolves to the superseded object — a divergence no kill was needed to
/// produce. See the re-rotation section of the rotation contract.
pub fn locator_is_already_in_chain(
    connection: &Connection,
    artifact: ArtifactId,
    locator: &VaultLocator,
) -> StoreResult<bool> {
    let migrations = read_descriptor_migrations(connection)?;
    Ok(migrations.iter().any(|migration| {
        migration.artifact_id == *artifact.as_bytes()
            && (migration.vault_locator == *locator || migration.superseded_locator == *locator)
    }))
}

/// Returns the next chain position for one artifact.
pub fn next_migration_seq(connection: &Connection, artifact: ArtifactId) -> StoreResult<u64> {
    if !migration_table_exists(connection)? {
        return Ok(1);
    }
    let highest: Option<i64> = connection
        .query_row(
            "SELECT max(migration_seq) FROM artifact_descriptor_migration WHERE artifact_id = ?1",
            [artifact.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(u64::try_from(highest.unwrap_or(0))
        .map_err(|_| rejected("a stored migration sequence is negative"))?
        .saturating_add(1))
}

/// Verifies the migrated object through the store-vault seam and appends the row.
///
/// The verifier is the same `SealedObjectVerifier` acceptance uses, so a
/// reference only moves to an object this build has authenticated end to end.
/// The insert is refused by migration `0005`'s triggers unless an accepted
/// `RETENTION_ACTION_RECORDED` event carries this record's digest, so the two
/// checks are independent: one says the bytes are there and open, the other
/// says a canonical event authorized exactly this move.
pub(crate) fn insert_descriptor_migration<V: SealedObjectVerifier>(
    connection: &Connection,
    migration: &DescriptorMigration,
    migrated: &ArtifactDescriptor,
    vault: &V,
) -> StoreResult<()> {
    if !migration.names(migrated)
        || migrated.vault_locator != migration.vault_locator
        || migrated.format_version != migration.format_version
    {
        return Err(rejected(
            "the migrated descriptor is not the one the migration record names",
        ));
    }
    if locator_is_already_in_chain(connection, migrated.id, &migration.vault_locator)? {
        return Err(StoreError::DescriptorMigrationRejected {
            reason: "the artifact's reference chain has already recorded this locator",
            detail: format!(
                "artifact {} has already been reachable under {}; a chain records \
                 each locator once, so a rotation back to a generation this \
                 artifact has already been under cannot be recorded",
                migrated.id, migration.vault_locator,
            ),
        });
    }

    vault
        .verify_sealed_object(migrated)
        .map_err(
            |source: VaultError| StoreError::DescriptorMigrationRejected {
                reason: "the vault did not authenticate the object at the new locator",
                detail: source.to_string(),
            },
        )?;
    connection.execute(
        concat!(
            "INSERT INTO artifact_descriptor_migration (retention_action_id, artifact_id, ",
            "migration_seq, superseded_locator, vault_locator, format_version, record_digest) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        ),
        params![
            migration.retention_action_id.as_slice(),
            migration.artifact_id.as_slice(),
            i64::try_from(migration.migration_seq)
                .map_err(|_| rejected("the migration sequence does not fit SQLite's integer"))?,
            migration.superseded_locator.as_bytes().as_slice(),
            migration.vault_locator.as_bytes().as_slice(),
            i64::from(migration.format_version),
            migration.record_digest().as_bytes().as_slice(),
        ],
    )?;
    Ok(())
}

fn migration_table_exists(connection: &Connection) -> StoreResult<bool> {
    let found: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            ["artifact_descriptor_migration"],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

fn identifier(bytes: &[u8]) -> StoreResult<[u8; 16]> {
    <[u8; 16]>::try_from(bytes).map_err(|_| rejected("a stored identifier is not sixteen bytes"))
}

fn locator(bytes: &[u8]) -> StoreResult<VaultLocator> {
    let raw = <[u8; 32]>::try_from(bytes)
        .map_err(|_| rejected("a stored vault locator is not thirty-two bytes"))?;
    Ok(VaultLocator::from_bytes(raw))
}
