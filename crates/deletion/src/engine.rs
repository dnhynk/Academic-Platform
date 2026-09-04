//! The half that reaches real `AEAD_CHUNKED_V2` objects.
//!
//! Compiled only by the `deletion-engine` feature, which selects
//! `academic-retention`'s own non-default object lane. Two things live here and
//! nothing else does: the shredder that destroys a real key slot, and the index
//! that reads a real object tree.
//!
//! # The shred is `P2-K5`'s, called and not re-implemented
//!
//! [`academic_retention::engine::shred_with_tombstone`] destroys the slot and
//! appends `ArtifactShredded`. `RB01` — a kill during the 80-byte positioned
//! write — is `academic-vault`'s failpoint, beside the slot it destroys, and it
//! trips inside that call whether the caller is `P2-K5`'s suite or this crate's
//! product flow.
//!
//! # Why the index is a directory walk and not a store query
//!
//! This crate has no `academic-store` edge, so there is no descriptor table to
//! ask. What a deletion needs from the object tree is the set of artifacts of
//! each derivative class, and the vault's own layout is what answers it:
//! `objects/<domain>/<retention>/<lineage>/<xx>/<yy>/<locator>.aobj`, with the
//! artifact id and the locator both cleartext in the 208-byte header. Reading
//! the header is what makes two registrations of the same bytes two entries
//! rather than one, which is the whole point of `P3-G10`.

use std::collections::BTreeMap;

use academic_domain::ArtifactDescriptor;
use academic_retention::{
    AppendOnlyJournal, BackupTombstone, DerivativeClass, ExecutionFailure, UnresolvedReason, engine,
};
use academic_vault::EncryptedVault;

use crate::{
    dry_run::{ClassTargets, DerivativeIndex},
    executors::KeySlotShredder,
    target::DeletionTarget,
};

/// Destroys a real key slot through `P2-K5`'s own shred.
#[derive(Debug)]
pub struct VaultShredder<'a> {
    vault: &'a EncryptedVault,
    descriptors: BTreeMap<DeletionTarget, ArtifactDescriptor>,
}

impl<'a> VaultShredder<'a> {
    /// Binds a vault to the descriptors a deletion may destroy.
    ///
    /// The descriptor map is the caller's, keyed by artifact **and** locator.
    /// A map keyed by locator alone would hand the shred whichever of two
    /// registrations it happened to hold, which is `P1-G1`. `P2-K5`'s own
    /// `ShreddingExecutor` fixture finds a descriptor by
    /// `vault_locator == action.locator`, which is that defect one layer out.
    #[must_use]
    pub const fn over(
        vault: &'a EncryptedVault,
        descriptors: BTreeMap<DeletionTarget, ArtifactDescriptor>,
    ) -> Self {
        Self { vault, descriptors }
    }
}

impl KeySlotShredder for VaultShredder<'_> {
    fn shred(
        &mut self,
        journal: &mut AppendOnlyJournal,
        target: &DeletionTarget,
        tombstone: &BackupTombstone,
    ) -> Result<(), ExecutionFailure> {
        let descriptor = self
            .descriptors
            .get(target)
            .ok_or_else(|| ExecutionFailure {
                reason: UnresolvedReason::ShredFailed,
                detail: format!("no descriptor is recorded for {}", target.to_row()),
            })?;
        engine::shred_with_tombstone(journal, self.vault, descriptor, tombstone).map_err(|source| {
            ExecutionFailure {
                reason: UnresolvedReason::ShredFailed,
                detail: format!("{} could not be shredded: {source}", target.to_row()),
            }
        })
    }
}

/// A derivative index over a real object tree.
///
/// The caller states which artifacts belong to which class — that is the
/// subsystem knowledge this crate does not have — and this type is what turns
/// each answer into the artifact-and-locator pairs a plan runs on, refusing a
/// class it was told nothing about rather than reporting it empty.
#[derive(Debug, Clone, Default)]
pub struct ObjectTreeIndex {
    classes: BTreeMap<DerivativeClass, Vec<DeletionTarget>>,
    empty_reasons: BTreeMap<DerivativeClass, String>,
}

impl ObjectTreeIndex {
    /// An index that can answer for no class yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            classes: BTreeMap::new(),
            empty_reasons: BTreeMap::new(),
        }
    }

    /// Records the artifacts one class holds.
    pub fn holding(&mut self, class: DerivativeClass, targets: Vec<DeletionTarget>) {
        self.classes.insert(class, targets);
    }

    /// Records that one class holds nothing, and why.
    ///
    /// A reason, not a blank. `P2-K5` refuses an empty class that cannot say
    /// why it is empty, because such a node cannot be told apart from a class
    /// the walk never reached.
    pub fn empty(&mut self, class: DerivativeClass, reason: String) {
        self.empty_reasons.insert(class, reason);
    }
}

impl DerivativeIndex for ObjectTreeIndex {
    fn resolve(&self, class: DerivativeClass, _subject: &DeletionTarget) -> ClassTargets {
        if let Some(targets) = self.classes.get(&class) {
            return ClassTargets::Targets(targets.clone());
        }
        if let Some(reason) = self.empty_reasons.get(&class) {
            return ClassTargets::NothingToDelete {
                reason: reason.clone(),
            };
        }
        // `RB03`: nothing was said about this class, so the deletion refuses to
        // complete and the node is named. Reporting it empty would be a
        // deletion reporting on a subset of itself.
        ClassTargets::Unresolved {
            reason: format!(
                "no subsystem answered for {} over this subject",
                class.as_str()
            ),
        }
    }
}
