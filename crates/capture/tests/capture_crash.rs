//! The `CP05` row of the t068 section 7 fault matrix.
//!
//! | ID | injection point | outcome |
//! |---|---|---|
//! | `CP05` | kill mid chunk | journal recovers to the last synced chunk; explicit gap |
//!
//! Kill-induced. Each of the three failpoints is reached by a real process
//! abort, and each leaves a distinguishable state on disk — nothing written, a
//! frame whose trailing digest never arrived, and a frame that is whole — so a
//! child that aborted early cannot pass as a child that aborted late.
//!
//! The child is this test binary re-invoked at a named entry point, which is
//! `academic-transcript`'s `IN04` arrangement. It runs the real
//! [`academic_capture::begin`] and [`academic_capture::CaptureRecorder::record_audio_chunk`]
//! against the real synthetic ledger, so the injection is in the product path
//! rather than in an imitation of it.

#![cfg(feature = "phase2-fault-injection")]

mod common;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use academic_capture::{
    CapturePolicyBook, ChunkJournal, FAULT_FRAME_VARIABLE, FAULT_READY_MARKER_VARIABLE,
    FAULT_SELECTION_VARIABLE, FAULT_SELECTORS, GapCause, RecordBody, begin, resume,
};

use common::{INSIDE, SECOND, TestResult, chunk, healthy_reading, ledger_permitting, request};

const CHILD_ENV: &str = "ACADEMIC_CAPTURE_TEST_CHILD";
const JOURNAL_ENV: &str = "ACADEMIC_CAPTURE_TEST_JOURNAL";

/// The three chunks the child writes. Committed literals; nothing is recorded.
const CHUNK_TAGS: [&str; 3] = ["cp05-0", "cp05-1", "cp05-2"];

/// The entry point the child runs. It is a `#[test]` so the harness can select
/// it by name, and it does nothing at all unless the parent set `CHILD_ENV`.
#[test]
fn cp05_child_entrypoint() -> TestResult {
    if env::var(CHILD_ENV).is_err() {
        return Ok(());
    }
    let path = PathBuf::from(env::var(JOURNAL_ENV)?);
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    for (index, tag) in CHUNK_TAGS.iter().enumerate() {
        recorder.record_audio_chunk(
            &mut ledger,
            chunk(tag),
            u64::try_from(index)?.saturating_mul(SECOND),
            INSIDE,
        )?;
    }
    Err("the child was not killed at its failpoint".into())
}

/// Runs this test binary again as a child that takes one failpoint and aborts.
///
/// The failpoint is armed for frame one rather than for every frame, so the
/// child writes frame zero whole before it is interrupted and every row below
/// has a synced chunk to recover to.
fn run_child(journal: &Path, selector: &str) -> TestResult {
    let marker = journal.with_extension("marker");
    let status = Command::new(env::current_exe()?)
        .arg("cp05_child_entrypoint")
        .arg("--exact")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env(JOURNAL_ENV, journal)
        .env(FAULT_SELECTION_VARIABLE, selector)
        .env(FAULT_FRAME_VARIABLE, "1")
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

#[test]
fn cp05_kill_mid_chunk_recovers_to_the_last_synced_chunk() -> TestResult {
    // The first chunk is written whole before any failpoint can trip on the
    // second, so every row below has at least one synced chunk to recover to.
    // What differs between them is what the *interrupted* chunk left behind.
    let cases: [(&str, usize, bool); 3] = [
        // nothing of the interrupted frame reached the file
        ("CP05:before-frame-write", 0, false),
        // its header and body did; its trailing digest did not
        ("CP05:after-body-before-trailer", 0, true),
        // the whole frame is durable, and the writer died after `sync` returned
        ("CP05:after-frame-synced", 1, false),
    ];
    assert_eq!(
        FAULT_SELECTORS,
        cases.map(|(selector, _, _)| selector).as_slice(),
        "the fault selector inventory and this matrix disagree"
    );

    let directory = tempfile::tempdir()?;
    for (selector, extra_synced, expect_tail) in cases {
        let journal = directory
            .path()
            .join(format!("{}.acjrnl", selector.replace(':', "-")));
        run_child(&journal, selector)?;

        // The parent recovers. Nothing but a file is shared with the child.
        let recovered = ChunkJournal::recover(&journal)?;
        let synced: Vec<&RecordBody> = recovered
            .records()
            .iter()
            .map(academic_capture::JournalRecord::body)
            .collect();
        let expected_chunks = 1 + extra_synced;
        assert_eq!(
            synced.len(),
            expected_chunks,
            "{selector}: recovered {} frames, expected {expected_chunks}",
            synced.len()
        );
        for (index, body) in synced.iter().enumerate() {
            let tag = CHUNK_TAGS.get(index).ok_or("more frames than chunks")?;
            let RecordBody::AudioChunk { bytes } = body else {
                return Err(format!("{selector}: frame {index} is not a chunk").into());
            };
            assert_eq!(bytes.as_slice(), chunk(tag).as_slice());
        }
        assert_eq!(
            recovered.partial_tail_bytes() > 0,
            expect_tail,
            "{selector}: the partial tail is {} bytes",
            recovered.partial_tail_bytes()
        );
        let last = recovered.last_synced().ok_or("no synced frame")?;
        assert_eq!(last.seq(), u32::try_from(expected_chunks)? - 1);

        // Resuming drops the partial tail, keeps every synced frame, and opens
        // an explicit gap that carries the new clock's domain — so nothing the
        // dead process wrote is re-timestamped onto the live one.
        let mut ledger = ledger_permitting()?;
        let book = CapturePolicyBook::published();
        let (mut recorder, at_resume) = resume(
            &mut ledger,
            &request()?,
            &journal,
            &book,
            healthy_reading(),
            INSIDE,
        )?;
        assert_eq!(at_resume.records(), recovered.records());
        // The partial tail is gone. It is checked before anything else is
        // appended, so a `reopen` that truncated nothing fails here rather than
        // being masked by the frames written after it.
        let truncated = ChunkJournal::recover(&journal)?;
        assert_eq!(
            truncated.partial_tail_bytes(),
            0,
            "{selector}: the partial tail was not dropped"
        );
        for (index, earlier) in recovered.records().iter().enumerate() {
            assert_eq!(truncated.records().get(index), Some(earlier));
        }

        recorder.record_audio_chunk(&mut ledger, chunk("after-resume"), 9 * SECOND, INSIDE)?;
        let after = ChunkJournal::recover(&journal)?;
        assert_eq!(after.partial_tail_bytes(), 0);
        let kinds: Vec<&'static str> = after
            .records()
            .iter()
            .map(|record| record.body().kind_str())
            .collect();
        let mut expected_kinds = vec!["AUDIO_CHUNK"; expected_chunks];
        expected_kinds.push("GAP");
        expected_kinds.push("AUDIO_CHUNK");
        assert_eq!(
            kinds, expected_kinds,
            "{selector}: the resumed shape is wrong"
        );

        let gap = after.records().get(expected_chunks).ok_or("no gap frame")?;
        let RecordBody::Gap {
            cause,
            resumed_domain,
        } = gap.body()
        else {
            return Err("the resume frame is not a gap".into());
        };
        assert_eq!(*cause, GapCause::SessionResumed);
        assert_eq!(
            *resumed_domain,
            Some(recorder.clock_domain()),
            "{selector}: the gap does not name the clock that follows it"
        );

        // The two halves of the file are on two clocks and the file says which.
        let before_domain = after.records().first().ok_or("no frames")?.at().domain();
        assert_eq!(before_domain, at_resume.header().domain());
        assert_ne!(
            before_domain,
            recorder.clock_domain(),
            "{selector}: the resumed session claims the dead process's clock"
        );
        let last_domain = after.records().last().ok_or("no frames")?.at().domain();
        assert_eq!(last_domain, recorder.clock_domain());

        // And nothing the dead process wrote moved.
        for (index, earlier) in recovered.records().iter().enumerate() {
            assert_eq!(
                after.records().get(index),
                Some(earlier),
                "{selector}: frame {index} changed across the resume"
            );
        }
    }
    Ok(())
}
