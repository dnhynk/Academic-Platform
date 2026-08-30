//! Adding and revoking the recipients that hold a wrapped Vault Master Key.
//!
//! `keys/recipients.cbor` is `P2-K1`'s frozen document and nothing here
//! changes its shape. A rotation adds records for the new generation beside the
//! old ones ([`rewrap_for_generation`]) and a *completed* rotation writes a set
//! holding only the new generation ([`retire_generation`]); a revocation writes
//! a set with the revoked records absent.
//!
//! The order is the whole point. Between the two calls, both generations are on
//! disk, so an object that has not migrated yet is still openable by a key the
//! profile actually holds. Retiring the old generation before every unit moved
//! would leave objects that no key on disk opens, which is why
//! [`retire_generation`] refuses until the journal records the rotation as
//! complete.
//!
//! # What revocation is, exactly
//!
//! Revoking a recipient removes its wrapped copy of the key and stops any
//! future generation from being wrapped for it. That is the whole of it.
//!
//! It does **not** erase plaintext that recipient already read, it does not
//! reach a copy taken while the recipient was live, and it does not make an
//! object that is still under the revoked generation unreadable to a holder of
//! that generation's key. [`REVOCATION_SCOPE_STATEMENT`] says so in the words
//! every surface repeats, and
//! `revocation_does_not_claim_prior_plaintext_erasure` fails if any surface
//! stops carrying it or starts claiming more.
//!
//! The thing that *does* make ciphertext unreadable is [`crate::shred`], and it
//! works by destroying key material rather than by revoking a recipient.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Write as _,
    path::{Path, PathBuf},
};

use academic_crypto::{
    ProfileId, RecipientKind, RecipientRecord, RecipientSet, RecordError, UnlockError,
};

use crate::{
    entry::JournalEntry,
    fault::{self, FaultPoint},
    journal::{AppendOnlyJournal, JournalError, sync_directory},
    rotation::{KeyGeneration, RotationError, RotationState},
};

/// Relative path of the recipient set inside a profile.
pub const RECIPIENTS_RELATIVE_PATH: &str = "keys/recipients.cbor";

/// The exact scope of a revocation, stated in the words every surface repeats.
///
/// A shorter or friendlier paraphrase is not permitted: the sentence exists so
/// no reader can take a revocation for an erasure.
pub const REVOCATION_SCOPE_STATEMENT: &str = "revocation stops this recipient from receiving any future key; \
     it does not erase plaintext that was already read, and it does not reach \
     a copy taken while the recipient was live";

/// Why a recipient set could not be read, written, or changed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RecipientError {
    /// The recipient file could not be read or written.
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
    /// The stored document is unusable.
    #[error("the recipient set is unusable: {0}")]
    Record(#[from] RecordError),
    /// The set holds no record for the named recipient.
    #[error("the recipient set holds no record for recipient {0}")]
    UnknownRecipient(String),
    /// Revoking would leave the profile with no recipient at all.
    ///
    /// A profile whose last recipient is revoked is a profile nobody can
    /// unlock, which is data destruction rather than access control. It is
    /// refused, and the caller is told to add the replacement first.
    #[error(
        "recipient {0} is the only remaining recipient; revoking it would leave \
         the profile permanently locked. Add the replacement recipient first"
    )]
    LastRecipient(String),
    /// A revoked identity is still present in the stored recipient set.
    #[error(
        "recipient {0} is recorded as revoked but is still present in the recipient set; no key is rewrapped for it"
    )]
    RevokedRecipientInSet(String),
    /// A rewrap produced a record for a revoked identity.
    #[error(
        "a rewrap produced a record for recipient {0}, which is revoked; the revoked recipient receives no new key"
    )]
    RevokedRecipientRewrapped(String),
    /// A revoked identity was offered to [`add_recipient`].
    ///
    /// Revocation is a journal fact about an identity, not about one stored
    /// record. Re-adding the same identity with a freshly wrapped record would
    /// hand the current key back to exactly the holder the revocation removed
    /// it from, so the identity is refused rather than the record.
    #[error(
        "recipient {0} is recorded as revoked and cannot be added again; a revoked identity receives no key"
    )]
    RevokedRecipientAdded(String),
    /// The old generation was asked to retire while a rotation is unfinished.
    #[error(
        "the rotation is not complete: {remaining} of {planned} units are still under the \
         old generation, so retiring it would leave objects no key on disk opens"
    )]
    RotationIncomplete {
        /// Units the journal still lists as not migrated.
        remaining: usize,
        /// Units the plan named.
        planned: usize,
    },
    /// The set holds no record wrapping the generation that was to be kept.
    #[error(
        "the recipient set holds no record for generation {0}; retiring the other generation \
         would leave the profile permanently locked"
    )]
    GenerationAbsent(String),
    /// A rotation left no record of itself for a generation retirement to check.
    ///
    /// Retiring a generation removes every wrapped copy of the other one from
    /// the profile. Without a completed rotation in the journal there is
    /// nothing that says the surviving generation opens anything, so the only
    /// safe answer is to refuse: the reproduction is a profile whose every
    /// object is under the generation the call would have deleted.
    #[error(
        "the journal records no rotation, so nothing says generation {0} opens this \
         profile's objects; retiring the other generation would leave it permanently locked"
    )]
    NoRotationRecorded(String),
    /// A generation retirement was asked to keep the superseded generation.
    ///
    /// A completed rotation left every reachable object under its target
    /// generation. Keeping the source generation would write a set holding only
    /// a key that opens nothing while deleting the one that opens everything.
    #[error(
        "the completed rotation left this profile under generation {target}, so the set \
         cannot be reduced to generation {kept}: no record on disk would open a reachable object"
    )]
    KeptGenerationIsNotTheRotationTarget {
        /// Generation the caller asked to keep.
        kept: String,
        /// Generation the completed rotation left in force.
        target: String,
    },
    /// A rewrap produced a record under a different identity than it wrapped.
    ///
    /// A rewrap re-wraps *this* recipient's copy of the key. A produced record
    /// with a different identity is a different recipient, and appending it
    /// beside the survivors silently adds an unrevoked reader.
    #[error(
        "a rewrap of recipient {survivor} produced a record for recipient {produced}; \
         a rewrap re-wraps the same recipient's copy and mints no new identity"
    )]
    RewrappedIdentityChanged {
        /// Identity that was to be rewrapped.
        survivor: String,
        /// Identity the produced record carries.
        produced: String,
    },
    /// The stored set already holds both generations for one recipient.
    ///
    /// A rotation's rewrap puts the new generation's record beside the old
    /// one, so between the rewrap and [`retire_generation`] each identity has
    /// exactly two. Running the rewrap again would give it three, and no key is
    /// needed to see that: `recipients.cbor` is `P2-K1`'s frozen document and
    /// carries no generation, so the identity count is the only thing this
    /// crate can check. The resume after a kill between the set rename and the
    /// journal record is [`retire_generation`], not a second rewrap.
    #[error(
        "the recipient set already holds {count} records for recipient {recipient}; the \
         rewrap for this rotation has already been written and re-running it would add another"
    )]
    GenerationAlreadyRewrapped {
        /// Identity that already has more than one record.
        recipient: String,
        /// How many records it has.
        count: usize,
    },

    /// Producing a record for one recipient failed.
    ///
    /// The wrapping itself is `academic-crypto`'s: this crate hands it the
    /// surviving recipient and reports whatever the key hierarchy said.
    #[error("a recipient record could not be produced: {0}")]
    Wrap(#[from] UnlockError),
    /// The journal could not be extended.
    #[error("the rotation journal is unusable: {0}")]
    Journal(#[from] JournalError),
    /// A key could not be derived.
    #[error("the key schedule failed")]
    KeySchedule,
}

impl From<RotationError> for RecipientError {
    fn from(_: RotationError) -> Self {
        Self::KeySchedule
    }
}

fn io(operation: &'static str, path: &Path, source: std::io::Error) -> RecipientError {
    RecipientError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

fn kind_name(kind: RecipientKind) -> &'static str {
    match kind {
        RecipientKind::DeviceKeystore => "DEVICE_KEYSTORE",
        RecipientKind::RecoverySecret => "RECOVERY_SECRET",
        // `RecipientKind` is `#[non_exhaustive]`; an unnamed kind must not be
        // silently spelled as one of the two above.
        _ => "UNKNOWN",
    }
}

/// What a revocation did, and — just as importantly — what it did not do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationOutcome {
    recipient_id: String,
    revoked_generation: String,
    remaining_recipients: usize,
    still_under_revoked_generation: Vec<String>,
}

impl RevocationOutcome {
    /// Returns the revoked recipient's hex identity.
    #[must_use]
    pub fn recipient_id(&self) -> &str {
        &self.recipient_id
    }

    /// Returns the generation the revoked record wrapped.
    #[must_use]
    pub fn revoked_generation(&self) -> &str {
        &self.revoked_generation
    }

    /// Returns how many recipients the profile still has.
    #[must_use]
    pub const fn remaining_recipients(&self) -> usize {
        self.remaining_recipients
    }

    /// Returns the locators still readable under the revoked generation.
    ///
    /// `KY05` requires these to be enumerated rather than described: a
    /// revocation that is followed by an interrupted rotation leaves objects
    /// the revoked key still opens, and the operator is owed the exact list.
    #[must_use]
    pub fn still_under_revoked_generation(&self) -> &[String] {
        &self.still_under_revoked_generation
    }

    /// Returns the exact scope of what was done.
    ///
    /// Every surface that reports a revocation reports this sentence unchanged.
    #[must_use]
    pub const fn scope_statement(&self) -> &'static str {
        REVOCATION_SCOPE_STATEMENT
    }
}

/// Reads the recipient set from a profile, or an empty set if none exists.
pub fn read_set(profile_root: &Path, profile: ProfileId) -> Result<RecipientSet, RecipientError> {
    let path = profile_root.join(RECIPIENTS_RELATIVE_PATH);
    match fs::read(&path) {
        Ok(bytes) => Ok(RecipientSet::from_canonical_cbor(&bytes)?),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(RecipientSet::new(profile))
        }
        Err(source) => Err(io("read recipient set", &path, source)),
    }
}

/// Replaces the recipient set with one atomic write.
///
/// `KY04` and `KY05` both require the set on disk to be the old one or the new
/// one and never a partial one. The bytes are written to a temporary file in
/// the same directory, synced, and renamed over the target; a rename within a
/// directory either happens or does not, so no reader can observe a half set.
pub fn write_set(profile_root: &Path, set: &RecipientSet) -> Result<(), RecipientError> {
    let path = profile_root.join(RECIPIENTS_RELATIVE_PATH);
    let Some(parent) = path.parent() else {
        return Err(io(
            "resolve recipient set directory",
            &path,
            std::io::Error::from(std::io::ErrorKind::InvalidInput),
        ));
    };
    fs::create_dir_all(parent).map_err(|source| io("create key directory", parent, source))?;

    let bytes = set.to_canonical_cbor()?;
    let temp = parent.join("recipients.cbor.partial");
    // A leftover partial from an earlier kill is replaced rather than appended
    // to: it is never the authoritative file and is never read back.
    let mut file =
        File::create(&temp).map_err(|source| io("create recipient set temp", &temp, source))?;
    file.write_all(&bytes)
        .map_err(|source| io("write recipient set temp", &temp, source))?;
    file.sync_all()
        .map_err(|source| io("synchronize recipient set temp", &temp, source))?;
    drop(file);

    // KY04/KY05: a kill here leaves the previous set intact and a partial file
    // beside it that nothing reads.
    fault::trip(FaultPoint::RecipientSetRename);

    fs::rename(&temp, &path).map_err(|source| io("publish recipient set", &path, source))?;
    sync_directory(parent)?;
    Ok(())
}

/// Adds one recipient record and journals the addition.
///
/// The record is produced by `academic-crypto`; nothing here wraps a key.
///
/// A revoked identity is refused here, not only in [`rewrap_for_generation`]:
/// `revoked_recipient_gets_no_new_key` is a statement about the identity, and
/// an addition path that did not read the revocation history would let a
/// caller undo a revocation by minting a fresh record under the same
/// `recipient_id`.
pub fn add_recipient(
    profile_root: &Path,
    profile: ProfileId,
    journal: &mut AppendOnlyJournal,
    record: RecipientRecord,
    generation: KeyGeneration,
) -> Result<(), RecipientError> {
    let recipient_id = hex::encode(record.recipient_id());
    if revoked_recipient_ids(journal).contains(&recipient_id) {
        return Err(RecipientError::RevokedRecipientAdded(recipient_id));
    }
    let mut set = read_set(profile_root, profile)?;
    let kind = kind_name(record.kind());
    set.push(record);
    write_set(profile_root, &set)?;
    journal.append(JournalEntry::RecipientAdded {
        recipient_id,
        recipient_kind: kind.to_owned(),
        generation: generation.to_hex(),
    })?;
    Ok(())
}

/// Removes one recipient's wrapped key and records what that does and does not do.
///
/// `still_under_revoked_generation` is supplied by the caller from the rotation
/// state, because only the rotation knows which objects have not moved yet.
pub fn revoke_recipient(
    profile_root: &Path,
    profile: ProfileId,
    journal: &mut AppendOnlyJournal,
    recipient_id: &[u8; 16],
    revoked_generation: KeyGeneration,
    still_under_revoked_generation: Vec<String>,
) -> Result<RevocationOutcome, RecipientError> {
    let set = read_set(profile_root, profile)?;
    let target = hex::encode(recipient_id);
    if !set
        .records()
        .iter()
        .any(|record| record.recipient_id() == recipient_id)
    {
        return Err(RecipientError::UnknownRecipient(target));
    }
    let kept: Vec<RecipientRecord> = set
        .records()
        .iter()
        .filter(|record| record.recipient_id() != recipient_id)
        .cloned()
        .collect();
    if kept.is_empty() {
        return Err(RecipientError::LastRecipient(target));
    }

    let mut replacement = RecipientSet::new(profile);
    for record in &kept {
        replacement.push(record.clone());
    }
    write_set(profile_root, &replacement)?;

    let outcome = RevocationOutcome {
        recipient_id: target,
        revoked_generation: revoked_generation.to_hex(),
        remaining_recipients: kept.len(),
        still_under_revoked_generation,
    };
    journal.append(JournalEntry::RecipientRevoked {
        recipient_id: outcome.recipient_id.clone(),
        revoked_generation: outcome.revoked_generation.clone(),
        scope_statement: REVOCATION_SCOPE_STATEMENT.to_owned(),
    })?;
    Ok(outcome)
}

/// Returns every recipient identity the journal records as revoked.
#[must_use]
pub fn revoked_recipient_ids(journal: &AppendOnlyJournal) -> BTreeSet<String> {
    journal
        .entries()
        .filter_map(|entry| match entry {
            JournalEntry::RecipientRevoked { recipient_id, .. } => Some(recipient_id.clone()),
            _ => None,
        })
        .collect()
}

/// Wraps the profile's key for the recipients that survive, and only those.
///
/// This is the operation `revoked_recipient_gets_no_new_key` is about. Two
/// independent things stop a revoked recipient from receiving the new key:
///
/// 1. the iteration source is the *current* recipient set, which a revocation
///    has already removed the record from; and
/// 2. every produced record is checked against the journal's revocation
///    history, so a caller that reintroduces a revoked identity — by holding a
///    stale record, or by minting a fresh record under the same identity — is
///    refused rather than silently honoured.
///
/// A produced record must also carry the identity it was asked to rewrap. A
/// rewrap re-wraps one recipient's own copy of the key; a record under another
/// identity is another recipient, and appending it beside the survivors would
/// add a reader nothing authorized.
///
/// The produced records are written **beside** the ones already stored, not
/// over them. That is what the rotation depends on: while units are still
/// moving, some reachable objects are under the old generation and some are
/// under the new one, so both generations have to be openable from the one
/// document the profile holds. [`retire_generation`] is the other half, and it
/// refuses until every unit has moved.
///
/// Running it twice is refused rather than obeyed. Between this call and
/// `retire_generation` each identity has exactly two records; a third would be
/// a rewrap of a rewrap. `recipients.cbor` is `P2-K1`'s frozen document and a
/// record does not say which generation it wraps, so the count is what this
/// crate can check without a key — and after a kill between the set rename and
/// the journal record the set on disk is already the rewrapped one, so the
/// resume is `retire_generation`, not a second rewrap.
///
/// `wrap` is the caller's `academic-crypto` call: this function never holds a
/// wrapping key, never reaches a broker, and never invents a recipient.
pub fn rewrap_for_generation<F>(
    profile_root: &Path,
    profile: ProfileId,
    journal: &mut AppendOnlyJournal,
    generation: KeyGeneration,
    mut wrap: F,
) -> Result<Vec<RecipientRecord>, RecipientError>
where
    F: FnMut(&RecipientRecord) -> Result<RecipientRecord, RecipientError>,
{
    let revoked = revoked_recipient_ids(journal);
    let survivors = read_set(profile_root, profile)?;
    // The set is `P2-K1`'s frozen document and a record does not say which
    // generation it wraps, so "has this rewrap already run?" is answered by the
    // one public fact a record does carry. A rotation's rewrap leaves exactly
    // two records per identity; a third is a re-run, and appending it is the
    // duplication `a_rewrap_re_run_is_refused_rather_than_duplicated` names.
    for record in survivors.records() {
        let identity = hex::encode(record.recipient_id());
        let count = survivors
            .records()
            .iter()
            .filter(|other| other.recipient_id() == record.recipient_id())
            .count();
        if count > 1 {
            return Err(RecipientError::GenerationAlreadyRewrapped {
                recipient: identity,
                count,
            });
        }
    }
    let mut rewrapped = Vec::with_capacity(survivors.records().len());
    for record in survivors.records() {
        let identity = hex::encode(record.recipient_id());
        if revoked.contains(&identity) {
            return Err(RecipientError::RevokedRecipientInSet(identity));
        }
        let produced = wrap(record)?;
        let produced_identity = hex::encode(produced.recipient_id());
        if revoked.contains(&produced_identity) {
            return Err(RecipientError::RevokedRecipientRewrapped(produced_identity));
        }
        if produced.recipient_id() != record.recipient_id() {
            return Err(RecipientError::RewrappedIdentityChanged {
                survivor: identity,
                produced: produced_identity,
            });
        }
        rewrapped.push(produced);
    }

    let mut extended = RecipientSet::new(profile);
    for record in survivors.records() {
        extended.push(record.clone());
    }
    for record in &rewrapped {
        extended.push(record.clone());
    }
    write_set(profile_root, &extended)?;
    for record in &rewrapped {
        journal.append(JournalEntry::RecipientAdded {
            recipient_id: hex::encode(record.recipient_id()),
            recipient_kind: kind_name(record.kind()).to_owned(),
            generation: generation.to_hex(),
        })?;
    }
    Ok(rewrapped)
}

/// Writes a recipient set holding only the generation a completed rotation left.
///
/// This is the second half of [`rewrap_for_generation`] and the point at which
/// the old generation's wrapped copies leave the profile. It is also the last
/// point at which the profile can be made permanently unopenable, so what it
/// keeps is decided by the journal rather than by the caller:
///
/// 1. the journal must record a rotation at all — without one, nothing says the
///    generation being kept opens anything, and every object may be under the
///    generation this call would delete;
/// 2. that rotation must be complete with no unit remaining, because a set
///    holding only the new generation would otherwise name a key that opens
///    nothing for the units still under the old one; and
/// 3. `kept_generation` must be the generation the rotation moved **to**. Being
///    asked to keep the superseded one is the same destruction stated backwards.
///
/// `keeps` is the caller's test of whether one stored record wraps the
/// generation being kept. A record's generation is not readable without the
/// key it wraps, and this crate holds no key, so the caller — which has just
/// finished rotating to that generation — answers it. What the caller does not
/// get to choose is *which* generation that is.
///
/// Returns the records that were kept.
pub fn retire_generation<F>(
    profile_root: &Path,
    profile: ProfileId,
    journal: &AppendOnlyJournal,
    kept_generation: KeyGeneration,
    mut keeps: F,
) -> Result<Vec<RecipientRecord>, RecipientError>
where
    F: FnMut(&RecipientRecord) -> bool,
{
    let Some(state) = RotationState::replay(journal.entries())? else {
        return Err(RecipientError::NoRotationRecorded(kept_generation.to_hex()));
    };
    let remaining = state.remaining().len();
    if !state.is_complete() || remaining > 0 {
        return Err(RecipientError::RotationIncomplete {
            remaining,
            planned: state.units().len(),
        });
    }
    if kept_generation != state.target() {
        return Err(RecipientError::KeptGenerationIsNotTheRotationTarget {
            kept: kept_generation.to_hex(),
            target: state.target().to_hex(),
        });
    }

    let revoked = revoked_recipient_ids(journal);
    let stored = read_set(profile_root, profile)?;
    let mut kept = Vec::new();
    for record in stored.records() {
        if !keeps(record) {
            continue;
        }
        let identity = hex::encode(record.recipient_id());
        if revoked.contains(&identity) {
            return Err(RecipientError::RevokedRecipientInSet(identity));
        }
        kept.push(record.clone());
    }
    if kept.is_empty() {
        return Err(RecipientError::GenerationAbsent(kept_generation.to_hex()));
    }

    let mut replacement = RecipientSet::new(profile);
    for record in &kept {
        replacement.push(record.clone());
    }
    write_set(profile_root, &replacement)?;
    Ok(kept)
}
