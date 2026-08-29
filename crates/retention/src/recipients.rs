//! Adding and revoking the recipients that hold a wrapped Vault Master Key.
//!
//! `keys/recipients.cbor` is `P2-K1`'s frozen document and nothing here
//! changes its shape. A rotation adds records for the new generation beside the
//! old ones and a completed rotation writes a set holding only the new
//! generation; a revocation writes a set with the revoked records absent.
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
    rotation::{KeyGeneration, RotationError},
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
pub fn add_recipient(
    profile_root: &Path,
    profile: ProfileId,
    journal: &mut AppendOnlyJournal,
    record: RecipientRecord,
    generation: KeyGeneration,
) -> Result<(), RecipientError> {
    let mut set = read_set(profile_root, profile)?;
    let recipient_id = hex::encode(record.recipient_id());
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

/// Rewraps the profile's key for the recipients that survive, and only those.
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
        rewrapped.push(produced);
    }

    let mut replacement = RecipientSet::new(profile);
    for record in &rewrapped {
        replacement.push(record.clone());
    }
    write_set(profile_root, &replacement)?;
    for record in &rewrapped {
        journal.append(JournalEntry::RecipientAdded {
            recipient_id: hex::encode(record.recipient_id()),
            recipient_kind: kind_name(record.kind()).to_owned(),
            generation: generation.to_hex(),
        })?;
    }
    Ok(rewrapped)
}
