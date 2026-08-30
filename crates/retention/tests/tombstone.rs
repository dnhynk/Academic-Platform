//! What a tombstone file is *named*, and why two deletions are two files.
//!
//! A locator is not an identity. It derives from the domain KEK over the media
//! type and the content digest, with no permission lineage and no retention
//! class in it, so inside one domain the same bytes registered twice get one
//! locator and two artifacts. `T119` put the artifact inside the record; these
//! rows are about the file the record is written to. A backup directory is a
//! flat namespace, so a name carrying only the locator makes the second of two
//! such deletions *replace* the first record instead of joining it — and a
//! restore of any backup taken before them then republishes the artifact
//! deleted first as readable while the receipt calls it a copy the deletion
//! deliberately spared.
//!
//! Nothing here needs an encrypted object, so these run in the default
//! workspace lane on every platform. What the same two deletions do to a real
//! object tree is `rotation.rs`, and what they do through the product backup
//! and the product restore is the encrypted portability lane.

use std::{error::Error, fs, path::PathBuf};

use academic_retention::{
    BackupTombstone,
    tombstone::{self, TOMBSTONE_EXTENSION, TOMBSTONE_VERSION, TombstoneError},
};

type TestResult = Result<(), Box<dyn Error>>;

/// The one locator two registrations of the same bytes share.
const SHARED_LOCATOR: &str = "77";
/// The artifact deleted first.
const FIRST_ARTIFACT: &str = "11";
/// And the one deleted after it.
const SECOND_ARTIFACT: &str = "22";

/// A record with the fields the product deletion path fills in.
///
/// `academic-retention` cannot build an `ArtifactId` — it is UUIDv7-backed and
/// this crate has no uuid edge — so these rows name the identity in the hex
/// spelling the record itself carries, which is what a file name derives from.
fn record(artifact: &str, locator: &str, action: u8) -> BackupTombstone {
    BackupTombstone {
        tombstone_version: TOMBSTONE_VERSION,
        action_id: artifact.repeat(16),
        artifact_id: artifact.repeat(16),
        locator: locator.repeat(32),
        superseded_locators: Vec::new(),
        shredded_at_ms: 1_700_000_000_000 + u64::from(action),
    }
}

/// A unique, self-deleting directory below the host temp directory.
struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "academic-retention-tombstone-{label}-{}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Every file a backup's tombstone directory holds, sorted.
fn files_in(backup: &std::path::Path) -> Result<Vec<String>, Box<dyn Error>> {
    let directory = tombstone::tombstone_dir(backup);
    let mut names: Vec<String> = fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names)
}

/// Deleting both registrations of one document leaves both records.
///
/// This is the shape the fifth `P2-A1` audit reproduced on both platforms: two
/// deletions, one locator, and a backup that came out of it holding **one**
/// record. The second write replaced the first, so restoring any backup taken
/// before the deletions resurrected whichever artifact was deleted first.
#[test]
fn two_tombstones_that_share_a_locator_are_two_files_and_two_records() -> TestResult {
    let root = TestRoot::new("two-deletions")?;
    let backup = root.0.join("backup");

    let first = record(FIRST_ARTIFACT, SHARED_LOCATOR, 1);
    let second = record(SECOND_ARTIFACT, SHARED_LOCATOR, 2);
    assert_eq!(first.locator, second.locator, "the row needs one locator");

    let first_path = tombstone::write_into_backup(&backup, &first)?;
    let second_path = tombstone::write_into_backup(&backup, &second)?;
    assert_ne!(
        first_path, second_path,
        "the second deletion was written over the first"
    );
    assert!(first_path.is_file() && second_path.is_file());
    assert_eq!(files_in(&backup)?.len(), 2, "a deletion lost its record");

    // Read back in the order the re-deletion and the three reports see them.
    assert_eq!(
        tombstone::read_from_backup(&backup)?,
        vec![first, second],
        "a record the backup holds was not read back"
    );
    Ok(())
}

/// The file names the artifact as well as the locator, in that order.
///
/// The artifact comes first because that is what makes two records under one
/// locator two names. The locator stays in it because one artifact has one file
/// per locator its reference chain moved through.
#[test]
fn a_tombstone_file_names_the_artifact_it_was_written_for() -> TestResult {
    let root = TestRoot::new("file-name")?;
    let backup = root.0.join("backup");
    let stone = record(FIRST_ARTIFACT, SHARED_LOCATOR, 1);

    let written = tombstone::write_into_backup(&backup, &stone)?;
    assert_eq!(
        written.file_name().and_then(|name| name.to_str()),
        Some(
            format!(
                "{}-{}.{TOMBSTONE_EXTENSION}",
                stone.artifact_id, stone.locator
            )
            .as_str()
        )
    );
    Ok(())
}

/// One destroyed key slot stays one file however often it is re-written.
///
/// `RB02`'s repair re-applies a deletion, and a re-application that added a
/// file each time would make a backup grow while no fact changed.
#[test]
fn re_writing_one_record_leaves_one_file() -> TestResult {
    let root = TestRoot::new("idempotent")?;
    let backup = root.0.join("backup");
    let stone = record(FIRST_ARTIFACT, SHARED_LOCATOR, 1);

    let once = tombstone::write_into_backup(&backup, &stone)?;
    let twice = tombstone::write_into_backup(&backup, &stone)?;
    assert_eq!(once, twice);
    assert_eq!(files_in(&backup)?.len(), 1);
    assert_eq!(tombstone::read_from_backup(&backup)?, vec![stone]);
    Ok(())
}

/// A stored record declaring version 1 is refused by version.
///
/// Version 1 named a locator and no artifact, so it cannot be applied to the
/// artifact it was written for — it would reach whichever object the directory
/// walk saw first. `read_from_backup` refuses one rather than guessing, which
/// `TOMBSTONE_VERSION` and the module header both state and nothing observed.
#[test]
fn a_stored_record_declaring_version_one_is_refused() -> TestResult {
    let root = TestRoot::new("version-one")?;
    let backup = root.0.join("backup");
    let path = tombstone::write_into_backup(&backup, &record(FIRST_ARTIFACT, SHARED_LOCATOR, 1))?;

    let stored = fs::read_to_string(&path)?;
    let downgraded = stored.replace(r#""tombstone_version":2"#, r#""tombstone_version":1"#);
    assert_ne!(downgraded, stored, "the stored record did not declare 2");
    fs::write(&path, downgraded)?;

    let read = tombstone::read_from_backup(&backup);
    assert!(
        matches!(
            read,
            Err(TombstoneError::UnsupportedVersion { version: 1, .. })
        ),
        "a version 1 record was not refused by version: {read:?}"
    );
    Ok(())
}

/// A record whose identity is not hex is refused instead of becoming a path.
///
/// The record's fields are public, so the name a file gets is caller-supplied
/// text unless something re-derives it. `write_into_backup` encodes the
/// record's own decoded bytes, so a record that is not 16 and 32 bytes of hex
/// has no file name at all: a deletion cannot be spelled into writing outside
/// the backup directory it was handed.
#[test]
fn a_record_whose_identity_is_not_hex_never_becomes_a_path() -> TestResult {
    let root = TestRoot::new("not-hex")?;
    let backup = root.0.join("backup");

    for mangle in [
        |stone: &mut BackupTombstone| stone.artifact_id = "../../escaped".to_owned(),
        |stone: &mut BackupTombstone| stone.locator = "../../escaped".to_owned(),
        |stone: &mut BackupTombstone| stone.artifact_id = "zz".repeat(16),
    ] {
        let mut stone = record(FIRST_ARTIFACT, SHARED_LOCATOR, 1);
        mangle(&mut stone);
        let written = tombstone::write_into_backup(&backup, &stone);
        assert!(
            matches!(written, Err(TombstoneError::Malformed(_))),
            "a record that is not an identity was given a file name: {written:?}"
        );
    }

    assert!(
        !tombstone::tombstone_dir(&backup).exists(),
        "a refused record still created the directory it would have been written to"
    );
    assert!(
        !root.0.join("escaped").exists(),
        "a refused record reached a path"
    );
    Ok(())
}
