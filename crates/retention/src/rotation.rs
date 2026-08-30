//! Resumable key rotation and the one invariant that makes it safe.
//!
//! # What rotates
//!
//! `KEK_d` is `HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/kek/v1" || domain_id)`
//! and `SKEY_p` is the same function of the Vault Master Key. Neither takes an
//! epoch, so **rotating a domain key means rotating the Vault Master Key.** One
//! rotation therefore moves every object and the store database together, which
//! is exactly why it needs a journal: it cannot be atomic.
//!
//! # The invariant
//!
//! > After an interruption at any point, exactly one of the old and new keys
//! > opens any object or database.
//!
//! "Both open" and "neither opens" are both violations. The property holds
//! because of two ordering rules and one refusal:
//!
//! 1. **A generation is refused if it is not new.** [`RotationPlan::new`]
//!    rejects a rotation whose target generation name equals its source, so the
//!    degenerate "rotation" that leaves both keys working cannot be started.
//!    Without this rule every unit would open under both keys and the invariant
//!    would be violated everywhere.
//! 2. **Reachability moves only after a verified read-back.** The engine writes
//!    the target object, lets the vault read it back and authenticate it, and
//!    only then appends [`JournalEntry::UnitResealed`]. `UnitMigrated` — the
//!    entry that moves reachability — comes after that. A kill anywhere before
//!    `UnitMigrated` leaves the source generation in force over an untouched
//!    source object; a kill after it leaves the target generation in force over
//!    an object that has already been proved readable.
//! 3. **The source object is never edited or removed by the rotation.** It
//!    becomes unreferenced when reachability moves, and the vault's own
//!    reconciliation quarantines it after the grace window. So "neither opens"
//!    cannot arise from the rotation deleting something too early.
//!
//! [`OpeningGeneration`] is a pure function of the journal, so a resumed
//! process decides which key is in force without holding either.

use academic_crypto::{KeyScheduleError, ProfileId, VaultMasterKey};
use academic_domain::ArtifactId;
use sha2::{Digest as _, Sha256};

use crate::{
    entry::{JournalEntry, PlannedUnit, UnitKind},
    journal::{AppendOnlyJournal, JournalError},
};

/// Domain separator for a rotation unit identity.
pub const UNIT_ID_DOMAIN: &[u8] = b"academic-os/rotation-unit/v1";

/// Width of a rotation identity.
pub const ROTATION_ID_BYTES: usize = 16;

/// The public, non-secret name of one key generation.
///
/// It is [`VaultMasterKey::generation_id`], which is the SHA-256 of an HKDF
/// output and therefore reveals nothing about the key and is not usable as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyGeneration([u8; 32]);

impl KeyGeneration {
    /// Names the generation a Vault Master Key belongs to.
    pub fn of(master: &VaultMasterKey, profile: ProfileId) -> Result<Self, KeyScheduleError> {
        Ok(Self(master.generation_id(profile)?))
    }

    /// Rebuilds a generation name from its recorded hex spelling.
    pub fn parse(value: &str) -> Result<Self, RotationError> {
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(value, &mut bytes).map_err(|_| RotationError::MalformedGeneration)?;
        Ok(Self(bytes))
    }

    /// Returns the lowercase hex spelling written into the journal.
    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

/// Identity of one rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationId([u8; ROTATION_ID_BYTES]);

impl RotationId {
    /// Wraps the caller's rotation identity bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; ROTATION_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the lowercase hex spelling written into the journal.
    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

/// One thing a rotation must move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationUnit {
    unit_id: [u8; 32],
    kind: UnitKind,
    source_locator: Option<[u8; 32]>,
}

impl RotationUnit {
    /// Builds the unit that rewraps one object, named by its source locator.
    #[must_use]
    pub fn object(source_locator: [u8; 32]) -> Self {
        Self {
            unit_id: derive_unit_id(UnitKind::Object, Some(&source_locator)),
            kind: UnitKind::Object,
            source_locator: Some(source_locator),
        }
    }

    /// Builds the unit that rekeys the SQLCipher profile database.
    #[must_use]
    pub fn store_database(profile: ProfileId) -> Self {
        Self {
            unit_id: derive_unit_id(UnitKind::StoreDatabase, Some(profile.as_bytes())),
            kind: UnitKind::StoreDatabase,
            source_locator: None,
        }
    }

    /// Returns the unit identity.
    #[must_use]
    pub const fn unit_id(&self) -> &[u8; 32] {
        &self.unit_id
    }

    /// Returns the unit identity's hex spelling.
    #[must_use]
    pub fn unit_id_hex(&self) -> String {
        hex::encode(self.unit_id)
    }

    /// Returns what this unit rewraps.
    #[must_use]
    pub const fn kind(&self) -> UnitKind {
        self.kind
    }

    /// Returns the source locator of an object unit.
    #[must_use]
    pub const fn source_locator(&self) -> Option<&[u8; 32]> {
        self.source_locator.as_ref()
    }

    fn to_planned(&self) -> PlannedUnit {
        PlannedUnit {
            unit_id: self.unit_id_hex(),
            unit_kind: self.kind,
            source_locator: self.source_locator.as_ref().map(hex::encode),
        }
    }
}

/// Domain separator for the identity a rekeyed store database records.
pub const STORE_DATABASE_TARGET_DOMAIN: &[u8] = b"academic-os/rotation-store-database/v1";

/// Names the profile database as it stands once `generation` opens it.
///
/// A rekey rewrites pages in place, so a `STORE_DATABASE` unit has no second
/// file the way an object unit does and no locator to record. What
/// `UnitResealed` carries for it is this digest: a pure function of the profile
/// identity and the generation whose `SKEY_p` now opens the database. It is 64
/// hex like every other locator field, and it discloses nothing the journal's
/// generation records do not already say.
#[must_use]
pub fn store_database_target_id(profile: ProfileId, generation: KeyGeneration) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(STORE_DATABASE_TARGET_DOMAIN);
    hasher.update([0]);
    hasher.update(profile.as_bytes());
    hasher.update([0]);
    hasher.update(generation.0);
    hasher.finalize().into()
}

/// What one `STORE_DATABASE` executor did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreDatabaseRekey {
    /// The database opened under the source generation's key and was rekeyed.
    Rekeyed,
    /// The database already opened under the target generation's key.
    ///
    /// This is the resume answer. `PRAGMA rekey` leaves exactly one working key
    /// at every point — that is fault `EN01` — so a kill during it leaves a
    /// database the target key opens and a journal that does not say so yet.
    /// Re-running reaches this arm and the journal records catch up.
    AlreadyAtTarget,
}

/// The executor a `STORE_DATABASE` rotation unit needs.
///
/// `SKEY_p` and `KEK_d` are both functions of the Vault Master Key, so a
/// rotation that moved every object and left the database under the superseded
/// key produces a profile whose two halves are under two generations. A backup
/// of one is not restorable, because a restore re-derives both halves from the
/// single master it recovered. The unit exists so that does not happen, and
/// this trait is how it is executed.
///
/// It is a trait rather than a function because `academic-retention` cannot
/// depend on the store: the encrypted store lane and the default lane are
/// mutually exclusive builds. The implementation ships in the encrypted
/// portability lane, which is the one place the store, the vault, and this
/// crate link into one process.
pub trait StoreDatabaseExecutor {
    /// Names the pair of generations this executor rekeys between.
    ///
    /// The engine checks it against the plan before it calls
    /// [`Self::rekey_store_database`], because nothing else can: the journal
    /// record this unit writes is `store_database_target_id(profile, plan
    /// target)`, a pure function of the plan, so an executor holding some other
    /// pair would move the database somewhere the journal then names as the
    /// target — a database neither generation the journal mentions can open,
    /// with `retire_generation` cleared to remove the records that still could.
    /// A rekey cannot be undone by reading the journal afterwards, so the check
    /// has to be in front of it.
    ///
    /// It is derived from the same key schedule the plan's generations come
    /// from, not stated: an implementation returns
    /// `KeyGeneration::of(master, profile)` for each of the two masters it
    /// holds.
    fn generations(&self) -> Result<(KeyGeneration, KeyGeneration), StoreDatabaseError>;

    /// Rekeys the profile database from the source generation to the target.
    ///
    /// It must be idempotent: a resume calls it again, and a database already
    /// under the target key is [`StoreDatabaseRekey::AlreadyAtTarget`], not a
    /// failure. It must fail closed when neither generation opens the database.
    fn rekey_store_database(&self) -> Result<StoreDatabaseRekey, StoreDatabaseError>;
}

/// Why an executor could not rekey the profile database.
///
/// The string is the store lane's own message. This crate cannot name the
/// store's error type, and inventing a taxonomy for a failure it cannot
/// classify would be a claim it has no evidence for.
#[derive(Debug, thiserror::Error)]
#[error("the store database could not be rekeyed: {0}")]
pub struct StoreDatabaseError(pub String);

/// The canonical reference a retirement is checked against.
///
/// Retiring a superseded object destroys its key slot and cannot be undone, so
/// the check that the reference has already moved cannot be the caller's word
/// for it. This crate cannot read the store, so it asks one: the implementation
/// resolves the artifact through the store's own `artifact_descriptor` row and
/// its appended migration chain, which is the same resolution a backup and a
/// restore use.
pub trait CanonicalReference {
    /// Returns the locator the store now resolves `artifact` to.
    ///
    /// `None` means the store holds no descriptor for that artifact at all,
    /// which is a refusal rather than a permission.
    fn resolved_locator(
        &self,
        artifact: ArtifactId,
    ) -> Result<Option<[u8; 32]>, CanonicalReferenceError>;
}

/// Why the canonical reference could not be read.
#[derive(Debug, thiserror::Error)]
#[error("the canonical reference could not be read: {0}")]
pub struct CanonicalReferenceError(pub String);

fn derive_unit_id(kind: UnitKind, seed: Option<&[u8]>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(UNIT_ID_DOMAIN);
    hasher.update([0]);
    hasher.update(kind.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(seed.unwrap_or_default());
    hasher.finalize().into()
}

/// Why a rotation could not be planned, executed, or resumed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RotationError {
    /// The target key is the same generation as the source.
    ///
    /// This is refused before anything is written. A rotation that does not
    /// change the key would leave both "generations" opening every object,
    /// which is one of the two halves of an invariant violation.
    #[error(
        "the target key is the same generation as the source, so this is not a \
         rotation: both keys would open every object"
    )]
    GenerationUnchanged,
    /// The plan holds no unit.
    #[error("a rotation plan must name at least one unit")]
    EmptyPlan,
    /// The plan names the same unit twice.
    #[error("the rotation plan names unit {0} more than once")]
    DuplicateUnit(String),
    /// A journal entry names a unit the plan does not hold.
    #[error("the journal names unit {0}, which the rotation plan does not hold")]
    UnknownUnit(String),
    /// The database unit is planned somewhere other than last.
    ///
    /// While objects are still moving, the store key that opens the database
    /// has to stay in force for the migrated ones and the unmigrated ones
    /// alike, and `record_descriptor_migration` opens the store to write each
    /// move. A plan that rekeys the database first leaves those writes with no
    /// key the plan names. The rule was a documented obligation on an
    /// orchestrator that does not exist yet; it is checked here instead.
    #[error(
        "the store database unit is planned at position {at} of {of}, and a \
         rotation moves it last"
    )]
    StoreDatabaseNotLast {
        /// Zero-based position the database unit was planned at.
        at: usize,
        /// How many units the plan holds.
        of: usize,
    },
    /// The journal holds a second, different rotation.
    #[error("the journal already holds rotation {0}, which is not this one")]
    ConcurrentRotation(String),
    /// A resumed rotation was handed a different pair of keys.
    #[error(
        "this journal records a rotation from generation {expected_source} to \
         {expected_target}, but the supplied keys name {actual_source} and {actual_target}"
    )]
    GenerationMismatch {
        /// Source generation the journal recorded.
        expected_source: String,
        /// Target generation the journal recorded.
        expected_target: String,
        /// Source generation the caller supplied.
        actual_source: String,
        /// Target generation the caller supplied.
        actual_target: String,
    },
    /// A recorded generation name is not 64 hex characters.
    #[error("a recorded key generation name is malformed")]
    MalformedGeneration,
    /// A unit was migrated without ever being resealed.
    ///
    /// The ordering rule that makes the invariant hold was violated, so the
    /// journal itself is refused rather than replayed into a wrong answer.
    #[error(
        "the journal migrates unit {0} without a preceding UnitResealed record, \
         so reachability moved before the target object was verified"
    )]
    MigratedBeforeReseal(String),
    /// The journal could not be read or extended.
    #[error("the rotation journal is unusable: {0}")]
    Journal(#[from] JournalError),
    /// A key could not be derived.
    #[error("the key schedule failed")]
    KeySchedule,
}

impl From<KeyScheduleError> for RotationError {
    fn from(_: KeyScheduleError) -> Self {
        Self::KeySchedule
    }
}

/// A planned rotation: two generation names and the units to move.
#[derive(Debug, Clone)]
pub struct RotationPlan {
    rotation_id: RotationId,
    profile_id: ProfileId,
    source: KeyGeneration,
    target: KeyGeneration,
    units: Vec<RotationUnit>,
}

impl RotationPlan {
    /// Builds a plan, refusing a rotation that does not change the key, names a
    /// unit twice, or moves the store database before an object.
    pub fn new(
        rotation_id: RotationId,
        profile_id: ProfileId,
        source: KeyGeneration,
        target: KeyGeneration,
        units: Vec<RotationUnit>,
    ) -> Result<Self, RotationError> {
        if source == target {
            return Err(RotationError::GenerationUnchanged);
        }
        if units.is_empty() {
            return Err(RotationError::EmptyPlan);
        }
        for (index, unit) in units.iter().enumerate() {
            if units
                .iter()
                .skip(index + 1)
                .any(|other| other.unit_id == unit.unit_id)
            {
                return Err(RotationError::DuplicateUnit(unit.unit_id_hex()));
            }
            if unit.kind == UnitKind::StoreDatabase && index + 1 != units.len() {
                return Err(RotationError::StoreDatabaseNotLast {
                    at: index,
                    of: units.len(),
                });
            }
        }
        Ok(Self {
            rotation_id,
            profile_id,
            source,
            target,
            units,
        })
    }

    /// Returns the rotation identity.
    #[must_use]
    pub const fn rotation_id(&self) -> RotationId {
        self.rotation_id
    }

    /// Returns the generation being rotated away from.
    #[must_use]
    pub const fn source(&self) -> KeyGeneration {
        self.source
    }

    /// Returns the generation being rotated to.
    #[must_use]
    pub const fn target(&self) -> KeyGeneration {
        self.target
    }

    /// Returns the profile every generation in this plan is salted with.
    #[must_use]
    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    /// Returns every unit in plan order.
    #[must_use]
    pub fn units(&self) -> &[RotationUnit] {
        &self.units
    }

    /// Returns the `RotationStarted` entry this plan opens its journal with.
    #[must_use]
    pub fn started_entry(&self) -> JournalEntry {
        JournalEntry::RotationStarted {
            rotation_id: self.rotation_id.to_hex(),
            profile_id: hex::encode(self.profile_id.as_bytes()),
            source_generation: self.source.to_hex(),
            target_generation: self.target.to_hex(),
            units: self.units.iter().map(RotationUnit::to_planned).collect(),
        }
    }
}

/// Which generation's key opens one unit's reachable object right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningGeneration {
    /// The generation the rotation is moving away from.
    Source,
    /// The generation the rotation is moving to.
    Target,
}

/// How far one unit has progressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitProgress {
    /// Nothing has been written for this unit.
    Planned,
    /// The target object exists, is durable, and was read back and verified.
    Resealed,
    /// Reachability has moved to the target generation.
    Migrated,
}

impl UnitProgress {
    /// Returns the generation whose key opens this unit's reachable object.
    ///
    /// Reachability moves only at `Migrated`, so everything before it is still
    /// served by the source object under the source key.
    #[must_use]
    pub const fn opening_generation(self) -> OpeningGeneration {
        match self {
            Self::Planned | Self::Resealed => OpeningGeneration::Source,
            Self::Migrated => OpeningGeneration::Target,
        }
    }
}

/// One unit's replayed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitState {
    /// The planned unit.
    pub unit: RotationUnit,
    /// How far it got.
    pub progress: UnitProgress,
    /// 64 hex locator the target object landed on, once resealed.
    pub target_locator: Option<String>,
}

/// The state a journal replays to.
///
/// This is a pure function of the journal: no filesystem, no key, no clock.
#[derive(Debug, Clone)]
pub struct RotationState {
    rotation_id: String,
    source: KeyGeneration,
    target: KeyGeneration,
    units: Vec<UnitState>,
    completed: bool,
}

impl RotationState {
    /// Replays the newest rotation recorded in `entries`.
    ///
    /// Returns `Ok(None)` when the journal records no rotation at all.
    pub fn replay<'a>(
        entries: impl IntoIterator<Item = &'a JournalEntry>,
    ) -> Result<Option<Self>, RotationError> {
        let mut state: Option<Self> = None;
        for entry in entries {
            match entry {
                JournalEntry::RotationStarted {
                    rotation_id,
                    source_generation,
                    target_generation,
                    units,
                    ..
                } => {
                    if let Some(existing) = &state
                        && !existing.completed
                        && existing.rotation_id != *rotation_id
                    {
                        return Err(RotationError::ConcurrentRotation(
                            existing.rotation_id.clone(),
                        ));
                    }
                    state = Some(Self {
                        rotation_id: rotation_id.clone(),
                        source: KeyGeneration::parse(source_generation)?,
                        target: KeyGeneration::parse(target_generation)?,
                        units: units.iter().map(rebuild_unit).collect::<Result<_, _>>()?,
                        completed: false,
                    });
                }
                JournalEntry::UnitResealed {
                    rotation_id,
                    unit_id,
                    target_locator,
                } => {
                    let unit = locate(&mut state, rotation_id, unit_id)?;
                    unit.progress = UnitProgress::Resealed;
                    unit.target_locator = Some(target_locator.clone());
                }
                JournalEntry::UnitMigrated {
                    rotation_id,
                    unit_id,
                } => {
                    let unit = locate(&mut state, rotation_id, unit_id)?;
                    if unit.progress == UnitProgress::Planned {
                        return Err(RotationError::MigratedBeforeReseal(unit_id.clone()));
                    }
                    unit.progress = UnitProgress::Migrated;
                }
                JournalEntry::RotationCompleted { rotation_id, .. } => {
                    if let Some(current) = &mut state
                        && current.rotation_id == *rotation_id
                    {
                        current.completed = true;
                    }
                }
                _ => {}
            }
        }
        Ok(state)
    }

    /// Returns the rotation identity's hex spelling.
    #[must_use]
    pub fn rotation_id(&self) -> &str {
        &self.rotation_id
    }

    /// Returns the generation being rotated away from.
    #[must_use]
    pub const fn source(&self) -> KeyGeneration {
        self.source
    }

    /// Returns the generation being rotated to.
    #[must_use]
    pub const fn target(&self) -> KeyGeneration {
        self.target
    }

    /// Returns every unit's replayed state, in plan order.
    #[must_use]
    pub fn units(&self) -> &[UnitState] {
        &self.units
    }

    /// Reports whether the rotation recorded its completion.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.completed
    }

    /// Returns the units a resume still has to move, in plan order.
    ///
    /// A unit is remaining until its reachability has moved. A unit whose
    /// target object was written but not yet migrated is still remaining: the
    /// source generation is still in force over it.
    #[must_use]
    pub fn remaining(&self) -> Vec<&UnitState> {
        self.units
            .iter()
            .filter(|unit| unit.progress != UnitProgress::Migrated)
            .collect()
    }

    /// Returns the units whose reachability has moved, in plan order.
    #[must_use]
    pub fn migrated(&self) -> Vec<&UnitState> {
        self.units
            .iter()
            .filter(|unit| unit.progress == UnitProgress::Migrated)
            .collect()
    }

    /// Returns the generation whose key opens one unit's reachable object.
    #[must_use]
    pub fn opening_generation(&self, unit_id: &[u8; 32]) -> Option<OpeningGeneration> {
        self.units
            .iter()
            .find(|state| state.unit.unit_id == *unit_id)
            .map(|state| state.progress.opening_generation())
    }

    /// Refuses a resume that was handed a different pair of keys.
    pub fn require_generations(
        &self,
        source: KeyGeneration,
        target: KeyGeneration,
    ) -> Result<(), RotationError> {
        if self.source == source && self.target == target {
            return Ok(());
        }
        Err(RotationError::GenerationMismatch {
            expected_source: self.source.to_hex(),
            expected_target: self.target.to_hex(),
            actual_source: source.to_hex(),
            actual_target: target.to_hex(),
        })
    }
}

fn rebuild_unit(planned: &PlannedUnit) -> Result<UnitState, RotationError> {
    let mut unit_id = [0_u8; 32];
    hex::decode_to_slice(&planned.unit_id, &mut unit_id)
        .map_err(|_| RotationError::UnknownUnit(planned.unit_id.clone()))?;
    let source_locator = match &planned.source_locator {
        Some(text) => {
            let mut bytes = [0_u8; 32];
            hex::decode_to_slice(text, &mut bytes)
                .map_err(|_| RotationError::UnknownUnit(planned.unit_id.clone()))?;
            Some(bytes)
        }
        None => None,
    };
    Ok(UnitState {
        unit: RotationUnit {
            unit_id,
            kind: planned.unit_kind,
            source_locator,
        },
        progress: UnitProgress::Planned,
        target_locator: None,
    })
}

fn locate<'a>(
    state: &'a mut Option<RotationState>,
    rotation_id: &str,
    unit_id: &str,
) -> Result<&'a mut UnitState, RotationError> {
    let current = state
        .as_mut()
        .ok_or_else(|| RotationError::UnknownUnit(unit_id.to_owned()))?;
    if current.rotation_id != rotation_id {
        return Err(RotationError::ConcurrentRotation(
            current.rotation_id.clone(),
        ));
    }
    current
        .units
        .iter_mut()
        .find(|unit| unit.unit.unit_id_hex() == unit_id)
        .ok_or_else(|| RotationError::UnknownUnit(unit_id.to_owned()))
}

/// Opens the rotation journal and replays whatever rotation it holds.
pub fn resume(journal: &AppendOnlyJournal) -> Result<Option<RotationState>, RotationError> {
    RotationState::replay(journal.entries())
}
