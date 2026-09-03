//! The `CP02`, `CP03` and `CP04` rows of the t068 section 7 fault matrix.
//!
//! Required outcomes, verbatim from section 7:
//!
//! | ID | injection point | outcome |
//! |---|---|---|
//! | `CP02` | storage full during capture | capture stops; chunk journal intact; gap recorded with cause |
//! | `CP03` | microphone lost | as `CP02` |
//! | `CP04` | clock drift beyond tolerance | `ALIGNMENT_LOW_CONFIDENCE` with ±seconds; no silent re-timestamping |
//!
//! All three are error-induced. A preflight reading and an anchor are both
//! values the public seams already take, so each is injected as a committed
//! literal rather than through a failpoint. `CP05` is kill-induced and is in
//! `capture_crash.rs` behind the `phase2-fault-injection` feature.
//!
//! Each row here asserts the on-disk state as well as the return value. A
//! capture that stopped in memory and left a broken file would satisfy the
//! first half of `CP02` and fail the second, which is the half the row exists
//! for.

mod common;

use academic_capture::{
    AlignmentConfidence, Anchor, CaptureFault, CapturePolicyBook, CapturePolicyRow, ChunkJournal,
    FailureKind, GapCause, MicrophoneState, PreflightReading, RecordBody, begin, estimate_drift,
};

use common::{
    INSIDE, SECOND, TestResult, chunk, healthy_reading, journal_path, ledger_permitting, request,
};

fn shipped_row() -> Result<CapturePolicyRow, Box<dyn std::error::Error>> {
    CapturePolicyBook::published()
        .effective_at(INSIDE)
        .ok_or_else(|| "the shipped book reaches no row at the fixture instant".into())
}

/// Drives a capture to three chunks, then hands it `reading`, and asserts the
/// three things `CP02` requires of the outcome.
fn resource_failure_row(tag: &str, reading: PreflightReading, expected: FailureKind) -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, tag);
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    let written: Vec<Vec<u8>> = (0..3)
        .map(|index| chunk(&format!("{tag}-{index}")))
        .collect();
    for (index, bytes) in written.iter().enumerate() {
        recorder.record_audio_chunk(
            &mut ledger,
            bytes.clone(),
            u64::try_from(index)?.saturating_mul(SECOND),
            INSIDE,
        )?;
    }
    let before = recorder.verify_on_disk()?;

    let raised = recorder.observe(reading, 3 * SECOND, 3 * SECOND + 100_000_000)?;
    assert_eq!(
        raised
            .iter()
            .map(|signal| signal.kind())
            .collect::<Vec<_>>(),
        vec![expected]
    );

    // Capture stops.
    assert_eq!(recorder.stopped(), Some(GapCause::ResourceFailure));
    let refused = recorder.record_audio_chunk(&mut ledger, chunk("after"), 10 * SECOND, INSIDE);
    assert!(
        matches!(
            refused,
            Err(CaptureFault::Stopped(GapCause::ResourceFailure))
        ),
        "a stopped capture took another chunk: {refused:?}"
    );

    // The chunk journal is intact: every frame written before the failure is
    // still there, byte-identical, in order, and the chain still verifies. Read
    // by something that never saw the writer.
    let after = ChunkJournal::recover(&path)?;
    assert_eq!(
        after.partial_tail_bytes(),
        0,
        "the failure left a partial frame"
    );
    for (index, earlier) in before.records().iter().enumerate() {
        assert_eq!(
            after.records().get(index),
            Some(earlier),
            "frame {index} changed when the capture stopped"
        );
    }
    for (index, bytes) in written.iter().enumerate() {
        let held = after
            .records()
            .get(index)
            .and_then(|record| record.body().bytes())
            .ok_or("a chunk left the file")?;
        assert_eq!(held.as_slice(), bytes.as_slice());
    }

    // A gap is recorded with its cause, and the signal frame that explains it
    // sits immediately before it.
    let tail: Vec<&'static str> = after
        .records()
        .iter()
        .skip(written.len())
        .map(|record| record.body().kind_str())
        .collect();
    assert_eq!(tail, vec!["FAILURE_SIGNAL", "GAP"]);
    let gap = after.records().last().ok_or("no gap frame")?;
    assert_eq!(
        gap.body(),
        &RecordBody::Gap {
            cause: GapCause::ResourceFailure,
            resumed_domain: None
        }
    );
    let signal = after
        .records()
        .get(written.len())
        .ok_or("no signal frame")?;
    assert_eq!(
        signal.body(),
        &RecordBody::FailureSignal {
            kind: expected,
            delivery: expected.delivery(),
            observed_at_nanos: 3 * SECOND,
        }
    );
    Ok(())
}

#[test]
fn cp02_storage_full_during_capture() -> TestResult {
    resource_failure_row(
        "cp02",
        PreflightReading::observed(8_192, 80, false, MicrophoneState::Held),
        FailureKind::StorageExhausted,
    )
}

#[test]
fn cp03_microphone_lost() -> TestResult {
    resource_failure_row(
        "cp03",
        PreflightReading::observed(4 * 1024 * 1024 * 1024, 80, false, MicrophoneState::Lost),
        FailureKind::MicrophoneLost,
    )
}

#[test]
fn cp04_clock_drift_beyond_tolerance() -> TestResult {
    // `ALIGNMENT_LOW_CONFIDENCE` with ±seconds, and no silent re-timestamping.
    // The second half is the one a named acceptance row cannot show on its own:
    // it is not about what the estimate says, it is about what the estimate
    // does *not* do to the frames already written.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let row = shipped_row()?;
    let path = journal_path(&directory, "cp04");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    for index in 0..4_u64 {
        recorder.record_audio_chunk(
            &mut ledger,
            chunk(&format!("cp04-{index}")),
            index.saturating_mul(10).saturating_mul(SECOND),
            INSIDE,
        )?;
    }
    let before = recorder.verify_on_disk()?;
    let base = before.records().first().ok_or("no frames")?.at();
    let later = before.records().last().ok_or("no frames")?.at();

    // Nine seconds of drift over a thirty-second interval, against a two-second
    // tolerance.
    let first = Anchor::at(base, base.elapsed_nanos());
    let second = Anchor::at(later, later.elapsed_nanos().saturating_sub(9 * SECOND));
    let estimate = estimate_drift(first, second, row)?;
    let AlignmentConfidence::Low { plus_minus_nanos } = estimate.confidence() else {
        return Err("nine seconds of drift is not low confidence".into());
    };
    assert_eq!(plus_minus_nanos, 9 * SECOND);
    assert_eq!(estimate.confidence().plus_minus_seconds(), 9);
    assert_eq!(
        estimate.confidence().badge(),
        Some(academic_capture::ALIGNMENT_LOW_CONFIDENCE)
    );

    // The realignment appends a version and the low confidence travels on it.
    let version = recorder.realign(first, second, 40 * SECOND)?;
    assert!(version.estimate().confidence().is_low());

    // No silent re-timestamping. Every frame written before the estimate still
    // carries the instant it was written at, on the same clock domain, and the
    // chain still verifies -- so the correction is a new frame beside them
    // rather than an edit of them.
    let after = ChunkJournal::recover(&path)?;
    for (index, earlier) in before.records().iter().enumerate() {
        let now = after.records().get(index).ok_or("a frame left the file")?;
        assert_eq!(now, earlier, "frame {index} was re-timestamped");
    }
    assert_eq!(
        after.records().len(),
        before.records().len().saturating_add(1),
        "the realignment did more than append"
    );
    let appended = after.records().last().ok_or("no appended frame")?;
    assert!(matches!(
        appended.body(),
        RecordBody::MappingVersion { version: 1, .. }
    ));
    Ok(())
}
