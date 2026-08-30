//! Where a `P2-K5` rotation reaches the store, in product code.
//!
//! `academic-retention` plans a rotation, journals it, and moves objects. Two
//! of the things a rotation has to do are facts about the *store*, and that
//! crate cannot read one: the encrypted store lane and the default lane are
//! mutually exclusive builds, so `academic-retention` has no store edge and a
//! test written there could only imitate one. This module is the binding, and
//! it lives here for the same reason `encrypted_rotation.rs` does — the
//! encrypted portability lane is the one place the store, the vault, and the
//! retention crate link into a single process.
//!
//! Two bindings, and one helper a deletion needs:
//!
//! * [`StoreDatabaseRekey`] executes the `STORE_DATABASE` unit through
//!   `P2-K2`'s `PRAGMA rekey`. Without it a rotation moves every object and
//!   leaves the database under the superseded key, and the profile's two halves
//!   are under two generations — which no backup of it can be restored from.
//! * [`StoreCanonicalReference`] answers what the store now resolves an
//!   artifact to, which is what a retirement's irreversible key-slot write is
//!   gated on.
//! * [`deletion_tombstone`] builds the tombstone a deletion writes into a
//!   backup, naming every locator the artifact has been reachable under.

use academic_crypto::{ProfileId, VaultMasterKey};
use academic_domain::{ArtifactDescriptor, ArtifactId};
use academic_retention::{
    BackupTombstone,
    rotation::{
        CanonicalReference, CanonicalReferenceError, KeyGeneration, StoreDatabaseError,
        StoreDatabaseExecutor, StoreDatabaseRekey as RekeyOutcome,
    },
};
use academic_store::{accept::AcceptanceStore, cipher, path_policy::PathProbe};

use crate::{PortabilityError, PortabilityResult};

/// The `STORE_DATABASE` rotation unit's executor, bound to `PRAGMA rekey`.
///
/// It holds the two Vault Master Keys the rotation names rather than two store
/// keys, so the pair it rekeys between is derived here from the same schedule
/// the rest of the rotation uses. A caller can still hand it two masters that
/// are not the ones its rotation plans, which is why it reports that pair
/// through `StoreDatabaseExecutor::generations` and the engine refuses it
/// against the plan before any page is rewritten.
#[derive(Debug)]
pub struct StoreDatabaseRekey<'a, P: PathProbe + ?Sized> {
    profile_root: &'a std::path::Path,
    probe: &'a P,
    profile_id: ProfileId,
    source: &'a VaultMasterKey,
    target: &'a VaultMasterKey,
}

impl<'a, P: PathProbe + ?Sized> StoreDatabaseRekey<'a, P> {
    /// Binds the executor to one profile and the rotation's two generations.
    #[must_use]
    pub const fn new(
        profile_root: &'a std::path::Path,
        probe: &'a P,
        profile_id: ProfileId,
        source: &'a VaultMasterKey,
        target: &'a VaultMasterKey,
    ) -> Self {
        Self {
            profile_root,
            probe,
            profile_id,
            source,
            target,
        }
    }
}

impl<P: PathProbe + ?Sized> StoreDatabaseExecutor for StoreDatabaseRekey<'_, P> {
    fn generations(&self) -> Result<(KeyGeneration, KeyGeneration), StoreDatabaseError> {
        let source = KeyGeneration::of(self.source, self.profile_id)
            .map_err(|error| StoreDatabaseError(error.to_string()))?;
        let target = KeyGeneration::of(self.target, self.profile_id)
            .map_err(|error| StoreDatabaseError(error.to_string()))?;
        Ok((source, target))
    }

    fn rekey_store_database(&self) -> Result<RekeyOutcome, StoreDatabaseError> {
        let current = self
            .source
            .derive_store_key(self.profile_id)
            .map_err(|source| StoreDatabaseError(source.to_string()))?;
        let next = self
            .target
            .derive_store_key(self.profile_id)
            .map_err(|source| StoreDatabaseError(source.to_string()))?;
        match cipher::rekey_encrypted_profile(self.profile_root, self.probe, &current, &next)
            .map_err(|source| StoreDatabaseError(source.to_string()))?
        {
            cipher::StoreRekeyOutcome::Rekeyed => Ok(RekeyOutcome::Rekeyed),
            cipher::StoreRekeyOutcome::AlreadyAtTarget => Ok(RekeyOutcome::AlreadyAtTarget),
        }
    }
}

/// The store's answer to "which object does this artifact resolve to now".
///
/// It walks the signed `artifact_descriptor` row through its
/// `artifact_descriptor_migration` chain, which is the same resolution
/// acceptance's preflight, a backup, and a restore use. A retirement is gated
/// on it because destroying a superseded object's key slot cannot be undone and
/// "the reference has already moved" must be read rather than asserted.
#[derive(Debug)]
pub struct StoreCanonicalReference<'a> {
    store: &'a AcceptanceStore,
}

impl<'a> StoreCanonicalReference<'a> {
    /// Binds the reference to one open acceptance store.
    #[must_use]
    pub const fn new(store: &'a AcceptanceStore) -> Self {
        Self { store }
    }
}

impl CanonicalReference for StoreCanonicalReference<'_> {
    fn resolved_locator(
        &self,
        artifact: ArtifactId,
    ) -> Result<Option<[u8; 32]>, CanonicalReferenceError> {
        let resolved = self
            .store
            .resolved_artifact_descriptor(artifact)
            .map_err(|source| CanonicalReferenceError(source.to_string()))?;
        Ok(resolved.map(|descriptor| *descriptor.vault_locator.as_bytes()))
    }
}

/// Builds the tombstone a deletion writes into a backup.
///
/// The tombstone names the locator being shredded now and every locator the
/// artifact's reference chain moved through before it. A locator is a function
/// of the domain KEK, so a rotation gives an artifact a new one; a backup taken
/// before the rotation holds the object under an older name, and a tombstone
/// naming only the current locator would leave that copy readable while
/// reporting nothing.
///
/// The record also names the artifact itself, which is what makes it reach that
/// artifact's copies and no others: a locator carries no lineage, so the same
/// bytes registered twice in one domain share one.
///
/// `descriptor` is the artifact as the store resolves it now — the object the
/// live shred destroys.
pub fn deletion_tombstone(
    store: &AcceptanceStore,
    action_id: String,
    descriptor: &ArtifactDescriptor,
    shredded_at_ms: u64,
) -> PortabilityResult<BackupTombstone> {
    let superseded: Vec<[u8; 32]> = store
        .superseded_locators(descriptor.id)
        .map_err(PortabilityError::Store)?
        .iter()
        .map(|locator| *locator.as_bytes())
        .collect();
    Ok(BackupTombstone::covering(
        action_id,
        descriptor.id,
        *descriptor.vault_locator.as_bytes(),
        &superseded,
        shredded_at_ms,
    ))
}
