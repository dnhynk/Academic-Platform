//! The closed set of facts a journal may record.
//!
//! Every variant is a fact that already happened and is durable. There is no
//! "in progress" entry and no entry that can be superseded in place: a
//! rotation that is abandoned is an incomplete chain, not a rewritten one.

use serde::{Deserialize, Serialize};

/// Which physical thing one rotation unit rewraps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum UnitKind {
    /// One reachable `AEAD_CHUNKED_V2` object.
    Object,
    /// The SQLCipher profile database.
    ///
    /// Its executor is `P2-K2`'s `PRAGMA rekey`, which cannot link into this
    /// crate: the encrypted store lane and the default lane are mutually
    /// exclusive builds. So the unit is planned, journalled, and
    /// invariant-checked here and executed through
    /// [`crate::rotation::StoreDatabaseExecutor`], which the encrypted
    /// portability lane binds. Its kill evidence is fault `EN01`.
    StoreDatabase,
}

impl UnitKind {
    /// Returns the stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Object => "OBJECT",
            Self::StoreDatabase => "STORE_DATABASE",
        }
    }
}

/// One unit named by `RotationStarted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedUnit {
    /// 64 lowercase hex; a domain-separated digest of the unit's source identity.
    pub unit_id: String,
    /// What this unit rewraps.
    pub unit_kind: UnitKind,
    /// 64 lowercase hex source locator, for an object unit.
    ///
    /// The locator is already the object's filename, so recording it discloses
    /// nothing the directory listing does not.
    pub source_locator: Option<String>,
}

/// One recorded fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
#[non_exhaustive]
pub enum JournalEntry {
    /// A rotation from one key generation to another was planned and begun.
    RotationStarted {
        /// 32 lowercase hex rotation identity.
        rotation_id: String,
        /// 32 lowercase hex profile identity.
        profile_id: String,
        /// 64 lowercase hex name of the generation being rotated away from.
        source_generation: String,
        /// 64 lowercase hex name of the generation being rotated to.
        target_generation: String,
        /// Every unit this rotation must move, in plan order.
        units: Vec<PlannedUnit>,
    },
    /// A unit's target object exists, is durable, and was read back and verified.
    ///
    /// Reachability has **not** moved. Appending this before the read-back
    /// verification would be the bug `interrupted_rewrap_has_exactly_one_opening_key`
    /// exists to catch.
    UnitResealed {
        /// Owning rotation.
        rotation_id: String,
        /// Unit identity from the plan.
        unit_id: String,
        /// 64 lowercase hex naming what this unit was resealed onto.
        ///
        /// The locator the object landed on, for an object unit. A
        /// `STORE_DATABASE` unit is rekeyed in place and has no locator, so it
        /// records [`crate::rotation::store_database_target_id`]: the same
        /// width, the same cleartext class, and a pure function of the profile
        /// and the generation the journal already names.
        target_locator: String,
    },
    /// A unit's reachability moved to the target generation.
    UnitMigrated {
        /// Owning rotation.
        rotation_id: String,
        /// Unit identity from the plan.
        unit_id: String,
    },
    /// A migrated unit's superseded object was retired.
    ///
    /// Retirement is the garbage collection `ADR-004` leaves open: the object
    /// the rotation moved away from is unreferenced, its grace window has
    /// passed, and its key slot is destroyed. After it, no key opens the
    /// superseded copy — which is what makes "exactly one of the old and the
    /// new key opens this artifact" true of the files on disk and not only of
    /// the journal-reachable one.
    UnitSourceRetired {
        /// Owning rotation.
        rotation_id: String,
        /// Unit identity from the plan.
        unit_id: String,
        /// 64 lowercase hex locator of the retired object.
        source_locator: String,
        /// 64 lowercase hex digest the destroyed key slot names.
        retirement_digest: String,
    },
    /// Every planned unit migrated.
    RotationCompleted {
        /// Owning rotation.
        rotation_id: String,
        /// How many units the plan held.
        unit_count: u64,
    },
    /// A recipient was added and can open the named generation.
    RecipientAdded {
        /// 32 lowercase hex recipient identity.
        recipient_id: String,
        /// `DEVICE_KEYSTORE` or `RECOVERY_SECRET`.
        recipient_kind: String,
        /// 64 lowercase hex generation this recipient wraps.
        generation: String,
    },
    /// A recipient was revoked and receives no further generation.
    RecipientRevoked {
        /// 32 lowercase hex recipient identity.
        recipient_id: String,
        /// 64 lowercase hex generation the revoked record wrapped.
        revoked_generation: String,
        /// The exact scope statement, so the journal itself cannot be read as a
        /// stronger claim than revocation makes.
        scope_statement: String,
    },
    /// A retention action was planned across every derivative class.
    RetentionPlanned {
        /// 32 lowercase hex retention action identity.
        action_id: String,
        /// 64 lowercase hex locator of the subject object.
        subject_locator: String,
        /// Every derivative class the plan enumerated, in registry order.
        classes: Vec<String>,
        /// Locators the plan could not resolve, if any.
        unresolved: Vec<String>,
    },
    /// One object's key slot was destroyed.
    ArtifactShredded {
        /// Owning retention action.
        action_id: String,
        /// 64 lowercase hex locator of the shredded object.
        locator: String,
        /// 64 lowercase hex digest of the tombstone that authorized the shred.
        tombstone_digest: String,
    },
    /// A backup tombstone was written into a named backup.
    BackupTombstoneWritten {
        /// Owning retention action.
        action_id: String,
        /// 64 lowercase hex digest identifying the backup directory.
        backup_id: String,
        /// 64 lowercase hex digest of the tombstone written.
        tombstone_digest: String,
    },
    /// A retention action reached a terminal result.
    RetentionSettled {
        /// Owning retention action.
        action_id: String,
        /// One of `PLANNED`, `COMPLETE`, `PARTIAL`, `REPAIR_REQUIRED`.
        outcome: String,
        /// The exact unresolved locators. Empty only when `outcome` is `COMPLETE`.
        unresolved: Vec<String>,
    },
}
