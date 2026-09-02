//! The `IN03` and `IN04` rows of the t068 section 7 fault matrix.
//!
//! Required outcomes, verbatim from section 7:
//!
//! | ID | injection point | outcome |
//! |---|---|---|
//! | `IN03` | transcript row checksum mismatch | import halts at the exact row; nothing confirmed |
//! | `IN04` | kill mid import | no partial attempt set; lease released; resumable |
//!
//! `IN03` is error-induced and needs no failpoint: it is driven through
//! `reconcile`'s public seam and then through the session, so "nothing
//! confirmed" is observed on disk and not only in a return value.
//!
//! `IN04` is kill-induced. Three distinguishable on-disk states are each
//! reached by a real process abort, and each row asserts what the child
//! actually got to before it died — a session with nothing staged is a
//! different state from one with a complete staged set, so a child that
//! aborted early cannot pass as a child that aborted late.

#![cfg(feature = "phase2-fault-injection")]

mod support;

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use academic_domain::TranscriptVersionId;
use academic_transcript::{
    FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE, FAULT_SELECTORS,
    admission::AdmittedImport,
    reconcile::{TranscriptChecksums, reconcile},
    record::TranscriptField,
    session::{
        CONFIRMED_FILE_NAME, ImportSession, LEASE_FILE_NAME, STAGING_FILE_NAME, SessionState,
        encode_confirmed_set, inspect, session_directory,
    },
};
use support::{TestRoot, refusal, synthetic_transcript};

type TestResult = Result<(), Box<dyn Error>>;

const CHILD_ENV: &str = "ACADEMIC_TRANSCRIPT_TEST_CHILD";
const PROFILE_ENV: &str = "ACADEMIC_TRANSCRIPT_TEST_PROFILE";
const VERSION: &str = "01900000-0000-7000-8000-0000000007e1";

fn version() -> Result<TranscriptVersionId, Box<dyn Error>> {
    Ok(VERSION.parse()?)
}

// ---------------------------------------------------------------------------
// IN03 — transcript row checksum mismatch
// ---------------------------------------------------------------------------

/// A checksum mismatch halts at the exact row and confirms nothing.
///
/// The second half is what makes this a fault row rather than a repeat of the
/// named acceptance row: the session is driven to the point where a
/// confirmation would be published, and the profile is inspected afterwards.
#[test]
fn in03_row_checksum_mismatch_halts_at_the_row_and_confirms_nothing() -> TestResult {
    let root = TestRoot::new("in03")?;
    let version = version()?;
    let transcript = synthetic_transcript()?;
    let reference = TranscriptChecksums::of(&transcript);

    // The candidate disagrees in exactly one field of the middle row.
    let mut rows = transcript.rows().to_vec();
    let target = &rows[1];
    rows[1] = academic_transcript::record::TranscriptRow::new(
        target.ordinal(),
        target.course_code(),
        target.term(),
        target.credits(),
        "F",
    )?;
    let candidate = academic_transcript::record::NormalizedTranscript::new(
        academic_transcript::record::TranscriptIdentity::new(
            transcript.identity().student_number(),
            transcript.identity().student_name(),
            transcript.identity().institution(),
            transcript.identity().issued_on(),
        )?,
        rows,
    )?;

    let admitted = AdmittedImport::for_fault_injection_only();
    let session = ImportSession::begin(&admitted, root.path(), version)?;

    let outcome = reconcile(&candidate, &reference);
    let halt = outcome.halt().ok_or("the mismatch did not halt")?;
    assert_eq!(halt.ordinal(), 1);
    assert_eq!(halt.disagreeing_fields(), &[TranscriptField::Grade]);
    assert_eq!(halt.rows_reconciled_before_halt(), 1);

    // There is no reconciled transcript to stage, so the session stays at
    // `Started` and publishing refuses.
    assert!(outcome.reconciled().is_none());
    assert_eq!(
        inspect(root.path(), version)?,
        SessionState::Started { lease_held: true }
    );
    let error = refusal(session.publish(), "a halted import published")?;
    assert_eq!(error.code(), "NOTHING_STAGED");

    let directory = session_directory(root.path(), version);
    assert!(!directory.join(STAGING_FILE_NAME).exists());
    assert!(!directory.join(CONFIRMED_FILE_NAME).exists());

    // Non-vacuity: the same session with an agreeing candidate publishes, so
    // the refusals above are caused by the mismatch and not by the harness.
    let session = ImportSession::resume(&admitted, root.path(), version)?;
    let clean = reconcile(&transcript, &reference);
    let reconciled = clean
        .reconciled()
        .ok_or("the clean corpus did not reconcile")?;
    session.stage(reconciled)?;
    let published = session.publish()?;
    assert_eq!(
        fs::read(&published)?,
        encode_confirmed_set(version, reconciled)
    );
    assert_eq!(
        inspect(root.path(), version)?,
        SessionState::Published { lease_held: false }
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// IN04 — kill mid import
// ---------------------------------------------------------------------------

/// Every failpoint this crate implements is taken by a real kill below.
#[test]
fn in04_selectors_are_exhaustive() -> TestResult {
    assert_eq!(
        FAULT_SELECTORS,
        [
            "IN04:before-staging-rename",
            "IN04:after-staging-rename",
            "IN04:after-publish-rename",
        ]
    );
    Ok(())
}

/// A kill before the staging rename leaves nothing staged and nothing confirmed.
#[test]
fn in04_kill_before_staging_rename_leaves_no_partial_set() -> TestResult {
    let root = TestRoot::new("in04-before-staging")?;
    let version = version()?;
    run_child(root.path(), "IN04:before-staging-rename")?;

    let directory = session_directory(root.path(), version);
    assert!(
        !directory.join(STAGING_FILE_NAME).exists(),
        "a staged set survived"
    );
    assert!(
        !directory.join(CONFIRMED_FILE_NAME).exists(),
        "a confirmed set survived"
    );
    assert_eq!(
        inspect(root.path(), version)?,
        SessionState::Started { lease_held: true },
        "the killed child did not leave a started, leased session"
    );
    resume_and_finish(root.path(), version)
}

/// A kill after the staging rename leaves a complete staged set and no
/// confirmed set.
#[test]
fn in04_kill_after_staging_rename_leaves_a_complete_staged_set() -> TestResult {
    let root = TestRoot::new("in04-after-staging")?;
    let version = version()?;
    run_child(root.path(), "IN04:after-staging-rename")?;

    let directory = session_directory(root.path(), version);
    assert!(
        !directory.join(CONFIRMED_FILE_NAME).exists(),
        "a confirmed set survived a kill before publication"
    );
    assert_eq!(
        inspect(root.path(), version)?,
        SessionState::Staged { lease_held: true }
    );

    // The staged file is complete, not truncated: it arrived by rename over a
    // fully written, fsynced temporary.
    let transcript = synthetic_transcript()?;
    let reference = TranscriptChecksums::of(&transcript);
    let outcome = reconcile(&transcript, &reference);
    let reconciled = outcome.reconciled().ok_or("the corpus did not reconcile")?;
    assert_eq!(
        fs::read(directory.join(STAGING_FILE_NAME))?,
        encode_confirmed_set(version, reconciled),
        "the staged set is not the complete encoding"
    );
    resume_and_finish(root.path(), version)
}

/// A kill after the publish rename leaves a complete confirmed set and the
/// lease still held.
///
/// This is the row that says the lease is removed *after* publication: a kill
/// between the two leaves a complete set that a resumption must not republish.
#[test]
fn in04_kill_after_publish_rename_leaves_a_complete_confirmed_set() -> TestResult {
    let root = TestRoot::new("in04-after-publish")?;
    let version = version()?;
    run_child(root.path(), "IN04:after-publish-rename")?;

    let directory = session_directory(root.path(), version);
    assert!(!directory.join(STAGING_FILE_NAME).exists());
    assert_eq!(
        inspect(root.path(), version)?,
        SessionState::Published { lease_held: true },
        "the lease was released before the confirmed set was durable"
    );

    let transcript = synthetic_transcript()?;
    let reference = TranscriptChecksums::of(&transcript);
    let outcome = reconcile(&transcript, &reference);
    let reconciled = outcome.reconciled().ok_or("the corpus did not reconcile")?;
    assert_eq!(
        fs::read(directory.join(CONFIRMED_FILE_NAME))?,
        encode_confirmed_set(version, reconciled)
    );

    // A resumption of a published session is refused: resuming would be a
    // second publication rather than a recovery.
    let admitted = AdmittedImport::for_fault_injection_only();
    let error = refusal(
        ImportSession::resume(&admitted, root.path(), version),
        "a published session was resumed",
    )?;
    assert_eq!(error.code(), "SESSION_ALREADY_PUBLISHED");
    Ok(())
}

/// Two live sessions cannot both hold one lease.
#[test]
fn in04_lease_excludes_a_second_live_session() -> TestResult {
    let root = TestRoot::new("in04-lease")?;
    let version = version()?;
    let admitted = AdmittedImport::for_fault_injection_only();
    let first = ImportSession::begin(&admitted, root.path(), version)?;
    let error = refusal(
        ImportSession::begin(&admitted, root.path(), version),
        "two live sessions took one lease",
    )?;
    assert_eq!(error.code(), "SESSION_LEASE_HELD");

    // Releasing it makes the next `begin` succeed, so the refusal above is the
    // lease and not the directory.
    first.release()?;
    assert!(
        !session_directory(root.path(), version)
            .join(LEASE_FILE_NAME)
            .exists()
    );
    ImportSession::begin(&admitted, root.path(), version)?;
    Ok(())
}

/// A confirmed set names the transcript version it belongs to.
///
/// Not only the directory it sits in: a file identified solely by its location
/// could be moved into another session's directory and read there as that
/// session's confirmed set. Two versions of the same transcript therefore
/// encode differently, and the version's bytes are in the file.
#[test]
fn confirmed_set_is_bound_to_its_transcript_version() -> TestResult {
    let transcript = synthetic_transcript()?;
    let reference = TranscriptChecksums::of(&transcript);
    let outcome = reconcile(&transcript, &reference);
    let reconciled = outcome.reconciled().ok_or("the corpus did not reconcile")?;

    let first = version()?;
    let second: TranscriptVersionId = "01900000-0000-7000-8000-0000000007e2".parse()?;
    assert_ne!(first, second);

    let left = encode_confirmed_set(first, reconciled);
    let right = encode_confirmed_set(second, reconciled);
    assert_ne!(left, right, "the confirmed set does not name its version");
    assert!(
        left.windows(16).any(|window| window == first.as_bytes()),
        "the confirmed set does not carry its version bytes"
    );

    // Still identity-free: the durable file beside the vault is one more place
    // the student number must not be.
    for value in [
        transcript.identity().student_number(),
        transcript.identity().student_name(),
    ] {
        assert!(
            !left
                .windows(value.len())
                .any(|window| window == value.as_bytes()),
            "the confirmed set carries an identity value"
        );
    }
    Ok(())
}

/// Re-enters the session the killed child left and drives it to publication.
fn resume_and_finish(profile_root: &Path, version: TranscriptVersionId) -> TestResult {
    let admitted = AdmittedImport::for_fault_injection_only();
    // The lease the dead process left is still on disk: nothing about a killed
    // process removes a file. `resume` is what releases it, and that is the
    // only sense in which this crate claims a lease is released.
    assert!(inspect(profile_root, version)?.lease_held());
    let session = ImportSession::resume(&admitted, profile_root, version)?;
    assert!(inspect(profile_root, version)?.lease_held());

    let transcript = synthetic_transcript()?;
    let reference = TranscriptChecksums::of(&transcript);
    let outcome = reconcile(&transcript, &reference);
    let reconciled = outcome.reconciled().ok_or("the corpus did not reconcile")?;
    session.stage(reconciled)?;
    let published = session.publish()?;
    assert_eq!(
        fs::read(&published)?,
        encode_confirmed_set(version, reconciled)
    );
    assert_eq!(
        inspect(profile_root, version)?,
        SessionState::Published { lease_held: false }
    );
    Ok(())
}

/// Runs this test binary again as a child that takes one failpoint and aborts.
fn run_child(profile_root: &Path, selector: &str) -> TestResult {
    let marker = profile_root.join("fault-ready.marker");
    let status = Command::new(env::current_exe()?)
        .arg("in04_child_entrypoint")
        .arg("--exact")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env(PROFILE_ENV, profile_root)
        .env(FAULT_SELECTION_VARIABLE, selector)
        .env(FAULT_READY_MARKER_VARIABLE, &marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    assert!(
        !status.success(),
        "the child exited cleanly instead of aborting at {selector}"
    );
    let reached = fs::read_to_string(&marker)
        .map_err(|error| format!("the child never reached {selector}: {error}"))?;
    assert_eq!(reached, selector, "the child took the wrong failpoint");
    fs::remove_file(&marker)?;
    Ok(())
}

/// Re-entry point for the killed child. Never runs in a normal test pass.
///
/// The parent selects it by exact name, so the child runs this body and nothing
/// else in the binary.
#[test]
fn in04_child_entrypoint() -> TestResult {
    if env::var(CHILD_ENV).ok().as_deref() != Some("1") {
        return Ok(());
    }
    let profile_root = PathBuf::from(env::var(PROFILE_ENV)?);
    let version = version()?;
    let admitted = AdmittedImport::for_fault_injection_only();
    let session = match inspect(&profile_root, version)? {
        SessionState::Absent => ImportSession::begin(&admitted, &profile_root, version)?,
        _ => ImportSession::resume(&admitted, &profile_root, version)?,
    };
    let transcript = synthetic_transcript()?;
    let reference = TranscriptChecksums::of(&transcript);
    let outcome = reconcile(&transcript, &reference);
    let reconciled = outcome.reconciled().ok_or("the corpus did not reconcile")?;
    session.stage(reconciled)?;
    session.publish()?;
    Err("the child ran to completion without taking its failpoint".into())
}
