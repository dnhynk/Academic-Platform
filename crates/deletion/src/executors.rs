//! The executors that actually delete, and the one seam that needs the vault.
//!
//! Two of the three action kinds are filesystem work and are done here, in the
//! default lane, on every platform: a cache or replica is a file that is
//! removed, and a backup expiry is a tombstone written into a backup through
//! `P2-K5`'s own `write_into_backup`. Neither needs a key.
//!
//! The third is the crypto-shred, which is `academic-vault`'s positioned write
//! into an object's key slot. That is behind this crate's `deletion-engine`
//! feature for the reason `P2-K5` states: the object namespace is the vault's
//! non-default lane, and a default product build must resolve neither it nor
//! the key schedule through here. [`KeySlotShredder`] is the seam;
//! [`crate::engine::VaultShredder`] is the implementation the lane binds.
//!
//! # `RB02` and `RB04` are real here
//!
//! `RB04` is a purge that partially fails. [`FilesystemExecutor`] calls
//! `std::fs::remove_file`, so a replica path that is a non-empty directory
//! fails with the operating system's own error on both hosts — not with an
//! injected one.
//!
//! `RB02` is a backup tombstone write that fails. The write is
//! `academic_retention::tombstone::write_into_backup`, which creates the
//! tombstone directory first, so a backup root that is a file fails at
//! `create_dir_all` with the operating system's own error. `RB02` is reported
//! as `TombstoneWriteFailed`, which `settle` turns into `REPAIR_REQUIRED`
//! rather than `PARTIAL`: a deletion whose tombstone did not land will not
//! re-apply on restore.

use std::{collections::BTreeMap, path::PathBuf};

use academic_retention::{
    ActionKind, AppendOnlyJournal, BackupTombstone, DerivativeClass, ExecutionFailure,
    UnresolvedReason, tombstone,
};

use crate::target::DeletionTarget;

/// Destroys one object's key slot.
///
/// The one thing this crate cannot do without the vault. An implementation is
/// handed in, so the default lane compiles and runs the other two kinds.
pub trait KeySlotShredder {
    /// Destroys the key slot of `target` and records it, or says why it did not.
    fn shred(
        &mut self,
        journal: &mut AppendOnlyJournal,
        target: &DeletionTarget,
        tombstone: &BackupTombstone,
    ) -> Result<(), ExecutionFailure>;
}

/// Where each artifact's bytes and backups are.
///
/// Supplied by the caller because this crate holds no store and no layout: a
/// path it derived itself would be a second layout beside the vault's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeletionPaths {
    purge: BTreeMap<DeletionTarget, PathBuf>,
    backups: BTreeMap<DeletionTarget, PathBuf>,
}

impl DeletionPaths {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            purge: BTreeMap::new(),
            backups: BTreeMap::new(),
        }
    }

    /// Records the file a cache or replica target is stored in.
    pub fn purge_at(&mut self, target: DeletionTarget, path: PathBuf) {
        self.purge.insert(target, path);
    }

    /// Records the backup root a backup-expiry target's tombstone goes into.
    pub fn backup_at(&mut self, target: DeletionTarget, path: PathBuf) {
        self.backups.insert(target, path);
    }

    /// The file one target is stored in.
    #[must_use]
    pub fn purge_path(&self, target: &DeletionTarget) -> Option<&PathBuf> {
        self.purge.get(target)
    }

    /// The backup root one target's tombstone goes into.
    #[must_use]
    pub fn backup_root(&self, target: &DeletionTarget) -> Option<&PathBuf> {
        self.backups.get(target)
    }
}

/// The product executor: real purges, real tombstones, and a shredder seam.
#[derive(Debug)]
pub struct FilesystemExecutor<'s, S: KeySlotShredder + ?Sized + std::fmt::Debug> {
    shredder: &'s mut S,
    paths: DeletionPaths,
    action_id: String,
    shredded_at_ms: u64,
    tombstones: Vec<BackupTombstone>,
    purged: Vec<PathBuf>,
}

impl<'s, S: KeySlotShredder + ?Sized + std::fmt::Debug> FilesystemExecutor<'s, S> {
    /// Builds an executor over one action's identity.
    ///
    /// `shredded_at_ms` is the caller's instant. Nothing here reads a clock, so
    /// one deletion replays to the same bytes.
    #[must_use]
    pub const fn new(
        shredder: &'s mut S,
        paths: DeletionPaths,
        action_id: String,
        shredded_at_ms: u64,
    ) -> Self {
        Self {
            shredder,
            paths,
            action_id,
            shredded_at_ms,
            tombstones: Vec::new(),
            purged: Vec::new(),
        }
    }

    /// Every tombstone this run wrote or tried to write, in plan order.
    #[must_use]
    pub fn tombstones(&self) -> &[BackupTombstone] {
        &self.tombstones
    }

    /// Every file this run removed, in plan order.
    #[must_use]
    pub fn purged(&self) -> &[PathBuf] {
        &self.purged
    }

    fn tombstone_for(&self, target: &DeletionTarget) -> BackupTombstone {
        BackupTombstone::new(
            self.action_id.clone(),
            target.artifact(),
            *target.locator(),
            self.shredded_at_ms,
        )
    }
}

impl<S: KeySlotShredder + ?Sized + std::fmt::Debug> crate::execute::TargetExecutor
    for FilesystemExecutor<'_, S>
{
    fn execute(
        &mut self,
        journal: &mut AppendOnlyJournal,
        kind: ActionKind,
        _class: DerivativeClass,
        target: &DeletionTarget,
    ) -> Result<(), ExecutionFailure> {
        match kind {
            ActionKind::CryptoShred => {
                let stone = self.tombstone_for(target);
                let result = self.shredder.shred(journal, target, &stone);
                self.tombstones.push(stone);
                result
            }
            ActionKind::Purge => {
                let path = self
                    .paths
                    .purge_path(target)
                    .ok_or_else(|| ExecutionFailure {
                        reason: UnresolvedReason::PurgeFailed,
                        detail: format!("no path is recorded for {}", target.to_row()),
                    })?;
                std::fs::remove_file(path).map_err(|source| ExecutionFailure {
                    reason: UnresolvedReason::PurgeFailed,
                    detail: format!("{} could not be removed: {source}", path.display()),
                })?;
                self.purged.push(path.clone());
                Ok(())
            }
            ActionKind::BackupTombstone => {
                let stone = self.tombstone_for(target);
                let root = self
                    .paths
                    .backup_root(target)
                    .ok_or_else(|| ExecutionFailure {
                        reason: UnresolvedReason::TombstoneWriteFailed,
                        detail: format!("no backup root is recorded for {}", target.to_row()),
                    })?
                    .clone();
                let written = tombstone::write_into_backup(&root, &stone);
                self.tombstones.push(stone);
                written.map_err(|source| ExecutionFailure {
                    reason: UnresolvedReason::TombstoneWriteFailed,
                    detail: format!("{} refused the tombstone: {source}", root.display()),
                })?;
                Ok(())
            }
            // `ActionKind` is `#[non_exhaustive]`, so a kind added to `P2-K5`
            // reaches this arm. It fails rather than succeeding: an executor
            // that returned `Ok` for a way of deleting it does not implement
            // would report `COMPLETE` for a deletion that did nothing, which is
            // the "mostly deleted" result that contract refuses to have.
            unknown => Err(ExecutionFailure {
                reason: UnresolvedReason::NotResolved,
                detail: format!(
                    "{} is deleted by {unknown:?}, which this executor does not perform",
                    target.to_row()
                ),
            }),
        }
    }
}
