//! Shared synthetic fixtures for the `P2-P2` deletion suites.
//!
//! Every value here is synthetic and built in process, as `CONTRIBUTING.md`
//! requires. Nothing reads a clock and nothing reaches a network.
//!
//! The two artifacts [`SHARED_A`] and [`SHARED_B`] deliberately hold the **same
//! locator** and differ only in artifact id: that is the shape the fifth
//! `P2-A1` audit found `P1-G1` in, and a suite that never built it could not
//! tell a deletion that names an artifact from one that takes whichever
//! registration it reaches first.

#![allow(dead_code)]

use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

use academic_deletion::{
    ClassTargets, DeletionPaths, DeletionTarget, DerivativeIndex, KeySlotShredder,
    ProtectionDecision, ProtectionReason, ProtectionRegistry,
};
use academic_domain::{ArtifactId, ContentDigest, EntityId};
use academic_retention::{
    AppendOnlyJournal, BackupTombstone, DerivativeClass, ExecutionFailure, UnresolvedReason,
};

/// Result alias every case in these suites returns.
pub type TestResult = Result<(), Box<dyn Error>>;

/// The subject artifact every fixture deletes.
pub const SUBJECT_ARTIFACT: &str = "01900000-0000-7000-8000-0000000002a1";
/// One of two registrations of identical bytes in one domain.
pub const SHARED_A: &str = "01900000-0000-7000-8000-0000000002b1";
/// The other one. Same locator, different artifact.
pub const SHARED_B: &str = "01900000-0000-7000-8000-0000000002b2";
/// The user who confirms every deletion in these suites.
pub const DECIDING_USER: &str = "01900000-0000-7000-8000-0000000002c1";
/// A model run, for the actor rows that must be refused.
pub const MODEL_RUN: &str = "01900000-0000-7000-8000-0000000002c2";

/// Parses one synthetic artifact identifier.
pub fn artifact(value: &str) -> Result<ArtifactId, Box<dyn Error>> {
    Ok(value.parse::<ArtifactId>()?)
}

/// Parses one synthetic entity identifier.
pub fn entity(value: &str) -> Result<EntityId, Box<dyn Error>> {
    Ok(value.parse::<EntityId>()?)
}

/// A deterministic 32-byte locator.
#[must_use]
pub const fn locator(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// A deterministic digest over a label.
#[must_use]
pub fn digest(label: &str) -> ContentDigest {
    ContentDigest::sha256(label.as_bytes())
}

/// One target, by artifact spelling and locator seed.
pub fn target(id: &str, seed: u8) -> Result<DeletionTarget, Box<dyn Error>> {
    Ok(DeletionTarget::new(artifact(id)?, locator(seed)))
}

/// A derivative index a case states class by class.
#[derive(Debug, Clone, Default)]
pub struct StatedIndex {
    answers: BTreeMap<DerivativeClass, ClassTargets>,
}

impl StatedIndex {
    /// An index that answers `Unresolved` for every class it was not told about.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An index that says every class is empty, for one stated reason.
    #[must_use]
    pub fn all_empty(reason: &str) -> Self {
        let mut index = Self::default();
        for class in academic_retention::DERIVATIVE_CLASSES {
            index.answers.insert(
                *class,
                ClassTargets::NothingToDelete {
                    reason: reason.to_owned(),
                },
            );
        }
        index
    }

    /// States one class's answer.
    pub fn state(&mut self, class: DerivativeClass, answer: ClassTargets) -> &mut Self {
        self.answers.insert(class, answer);
        self
    }
}

impl DerivativeIndex for StatedIndex {
    fn resolve(&self, class: DerivativeClass, _subject: &DeletionTarget) -> ClassTargets {
        self.answers
            .get(&class)
            .cloned()
            .unwrap_or_else(|| ClassTargets::Unresolved {
                reason: format!("no subsystem answered for {}", class.as_str()),
            })
    }
}

/// A protection registry that protects nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NothingProtected;

impl ProtectionRegistry for NothingProtected {
    fn decide(&self, _target: &DeletionTarget) -> ProtectionDecision {
        ProtectionDecision::NotProtected
    }
}

/// A protection registry that protects exactly the targets it was given.
#[derive(Debug, Clone, Default)]
pub struct StatedProtection {
    protected: BTreeMap<DeletionTarget, ProtectionReason>,
}

impl StatedProtection {
    /// A registry protecting nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Protects one target under one reason.
    pub fn protect(&mut self, target: DeletionTarget, reason: ProtectionReason) -> &mut Self {
        self.protected.insert(target, reason);
        self
    }
}

impl ProtectionRegistry for StatedProtection {
    fn decide(&self, target: &DeletionTarget) -> ProtectionDecision {
        self.protected
            .get(target)
            .map_or(ProtectionDecision::NotProtected, |reason| {
                ProtectionDecision::Protected(reason.clone())
            })
    }
}

/// A shredder that records what it was asked to destroy.
///
/// The default lane has no vault, so the crypto-shred is recorded rather than
/// performed here; `deletion_faults.rs` runs the real one under
/// `deletion-engine`. What this fixture does prove is that the shred is asked
/// for once per artifact and with that artifact's own tombstone.
#[derive(Debug, Clone, Default)]
pub struct RecordingShredder {
    pub shredded: Vec<(DeletionTarget, String)>,
    pub refuse: Vec<DeletionTarget>,
}

impl KeySlotShredder for RecordingShredder {
    fn shred(
        &mut self,
        _journal: &mut AppendOnlyJournal,
        target: &DeletionTarget,
        tombstone: &BackupTombstone,
    ) -> Result<(), ExecutionFailure> {
        if self.refuse.contains(target) {
            return Err(ExecutionFailure {
                reason: UnresolvedReason::ShredFailed,
                detail: format!("{} was held open", target.to_row()),
            });
        }
        self.shredded.push((*target, tombstone.artifact_id.clone()));
        Ok(())
    }
}

/// One disposable directory, removed on drop.
///
/// The name carries a discriminator and the base is canonicalised, because a
/// shared temporary base resolves through a symbolic link on macOS runners and
/// a name without a discriminator collides with another worker's.
#[derive(Debug)]
pub struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    /// Creates a unique, owner-only directory below the canonicalised
    /// temporary base.
    ///
    /// The owner-only mode is not decoration: the vault's path policy refuses a
    /// profile root any group or other bit can reach, so a root created with
    /// the default mode fails `RB01` on Linux with `UnsafeEntry` while passing
    /// on Windows. This suite measured exactly that in its WSL2 run.
    pub fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let base = fs::canonicalize(std::env::temp_dir())?;
        let path = base.join(format!(
            "academic-deletion-{label}-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { path })
    }

    /// The root path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let counter = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| u64::from(elapsed.subsec_nanos()));
    counter.wrapping_mul(1_000_000_007).wrapping_add(nanos)
}

/// Writes one file with one byte in it and returns its path.
pub fn touch(root: &std::path::Path, name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = root.join(name);
    fs::write(&path, b"synthetic")?;
    Ok(path)
}

/// Records a purge path and a backup root for one target.
pub fn paths_for(
    purge: &[(DeletionTarget, PathBuf)],
    backups: &[(DeletionTarget, PathBuf)],
) -> DeletionPaths {
    let mut paths = DeletionPaths::new();
    for (target, path) in purge {
        paths.purge_at(*target, path.clone());
    }
    for (target, path) in backups {
        paths.backup_at(*target, path.clone());
    }
    paths
}
