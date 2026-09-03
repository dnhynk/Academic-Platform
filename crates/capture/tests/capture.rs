//! The eight named `P2-L2` acceptance rows.
//!
//! Every fixture is a committed literal and nothing here records anything. The
//! journal files are written into a `tempfile` directory, which is the only
//! filesystem this suite touches.

mod common;

use std::path::Path;

use academic_capture::{
    ALIGNMENT_LOW_CONFIDENCE, AlignmentConfidence, AlignmentFault, Anchor, CaptureBytes,
    CaptureFault, CapturePolicyBook, CapturePolicyRow, ChunkJournal, FailureKind, GapCause,
    JournalRecord, MarkLabelKind, MicrophoneState, Orientation, PreflightReading, RecordBody,
    SessionClock, begin, estimate_drift,
};
use academic_domain::ContentDigest;

use common::{
    INSIDE, SECOND, TestResult, append_refusal, chunk, healthy_reading, image, journal_path,
    ledger_permitting, ledger_refusing, request,
};

/// The shipped book, and the row that is effective at the fixture instant.
fn shipped_row() -> Result<CapturePolicyRow, Box<dyn std::error::Error>> {
    CapturePolicyBook::published()
        .effective_at(INSIDE)
        .ok_or_else(|| "the shipped book reaches no row at the fixture instant".into())
}

fn audio_records(records: &[JournalRecord]) -> Vec<&JournalRecord> {
    records
        .iter()
        .filter(|record| matches!(record.body(), RecordBody::AudioChunk { .. }))
        .collect()
}

#[test]
fn capture_one_action_authorization() -> TestResult {
    // Section 12.2 draws one button. `begin` is that button: it evaluates the
    // section 3.7 permission, selects the effective policy row, runs preflight,
    // starts the one clock and creates the journal. There is no public
    // constructor for a recorder, so holding one is proof all five happened.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, "one-action");

    let recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    assert!(path.is_file(), "the one action created no journal");
    assert_eq!(recorder.journal_header().domain(), recorder.clock_domain());
    assert_eq!(recorder.journal_header().token_id(), recorder.token_id());
    assert_eq!(
        recorder.journal_header().policy_digest(),
        &shipped_row()?.digest(),
        "the journal does not cite the policy row the capture began under"
    );
    drop(recorder);

    // The refusing half. A written refusal binds to nothing, so the same one
    // action refuses -- and leaves nothing on disk, which is what makes the
    // refusal fail-closed rather than a flag on a started capture.
    let mut refusing = ledger_refusing()?;
    let refused_path = journal_path(&directory, "refused");
    let refusal = begin(
        &mut refusing,
        &request()?,
        &refused_path,
        &book,
        healthy_reading(),
        INSIDE,
    );
    assert!(
        matches!(refusal, Err(CaptureFault::Permission(_))),
        "a written refusal did not stop the one action: {refusal:?}"
    );
    assert!(
        !refused_path.exists(),
        "a refused capture left a journal on disk"
    );

    // The preflight half, at the same seam. A reading below the effective floor
    // refuses before anything is created, so a capture that could not have been
    // written is never begun.
    let starved_path = journal_path(&directory, "starved");
    let starved = begin(
        &mut ledger,
        &request()?,
        &starved_path,
        &book,
        PreflightReading::observed(1_024, 80, false, MicrophoneState::Held),
        INSIDE,
    );
    assert!(
        matches!(starved, Err(CaptureFault::Preflight(ref found)) if found == &[FailureKind::StorageExhausted]),
        "a storage-starved device began a capture: {starved:?}"
    );
    assert!(!starved_path.exists(), "a refused preflight left a journal");

    // And an instant no policy row reaches is refused rather than defaulted.
    // The permission still binds -- the book is the only thing that does not
    // reach -- so this is the policy lookup failing and not the section 3.7
    // comparison failing ahead of it.
    let undated_book = CapturePolicyBook::of(vec![CapturePolicyRow::declare(
        "capture.thresholds.fixture-not-yet",
        INSIDE.saturating_add(1),
        shipped_row()?.drift_tolerance_nanos(),
        shipped_row()?.storage_floor_bytes(),
        shipped_row()?.battery_floor_percent(),
        shipped_row()?.notification_within_nanos(),
    )]);
    let undated_path = journal_path(&directory, "undated");
    let undated = begin(
        &mut ledger,
        &request()?,
        &undated_path,
        &undated_book,
        healthy_reading(),
        INSIDE,
    );
    assert!(
        matches!(undated, Err(CaptureFault::NoEffectivePolicy { at: INSIDE })),
        "an instant no policy row reaches was given a default: {undated:?}"
    );
    assert!(!undated_path.exists(), "an undated capture left a journal");
    Ok(())
}

#[test]
fn offline_capture_continuity() -> TestResult {
    // `t001`'s `REQ-12-017` row: a contiguous local chunk timeline with hashes,
    // and later resumable processing. There is no network to sever because
    // there is no upload path -- the journal is what a capture writes, always,
    // and `phase1_exit_has_no_product_network` is what keeps that true.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, "continuity");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;

    let offered: Vec<Vec<u8>> = (0..6)
        .map(|index| chunk(&format!("offline-{index}")))
        .collect();
    for (index, bytes) in offered.iter().enumerate() {
        let elapsed = u64::try_from(index)?.saturating_mul(SECOND);
        recorder.record_audio_chunk(&mut ledger, bytes.clone(), elapsed, INSIDE)?;
    }

    // The timeline is contiguous and the hashes are the frames' own. Read back
    // off disk rather than out of the writer, so this is what is durable.
    let sealed = recorder.seal(GapCause::ResourceFailure, 6 * SECOND)?;
    let journal = sealed.journal();
    assert_eq!(journal.partial_tail_bytes(), 0, "a clean seal left a tail");
    let audio = audio_records(journal.records());
    assert_eq!(audio.len(), offered.len(), "a chunk did not reach the file");
    for (index, record) in audio.iter().enumerate() {
        let expected = offered.get(index).ok_or("missing fixture chunk")?;
        let held = record
            .body()
            .bytes()
            .ok_or("an audio frame holds no bytes")?;
        assert_eq!(
            held.as_slice(),
            expected.as_slice(),
            "chunk {index} changed"
        );
        assert_eq!(
            record.at().elapsed_nanos(),
            u64::try_from(index)?.saturating_mul(SECOND),
            "chunk {index} is at the wrong instant"
        );
    }
    // Contiguity is the chain, not a count: every frame names the one before it.
    for pair in journal.records().windows(2) {
        let (earlier, later) = (
            pair.first().ok_or("empty window")?,
            pair.get(1).ok_or("short window")?,
        );
        assert_eq!(
            later.parent(),
            earlier.digest(),
            "frame {} does not chain to {}",
            later.seq(),
            earlier.seq()
        );
        assert!(
            later.at().elapsed_nanos() >= earlier.at().elapsed_nanos(),
            "the timeline went backwards at frame {}",
            later.seq()
        );
    }
    // And the timeline ends with the explicit gap the seal opened.
    assert_eq!(
        sealed
            .gaps()
            .iter()
            .map(|(_, cause)| *cause)
            .collect::<Vec<_>>(),
        vec![GapCause::ResourceFailure]
    );

    // Later resumable processing: a second reader that never saw the writer
    // reaches the same frames from the file alone.
    let reread = ChunkJournal::recover(&path)?;
    assert_eq!(reread.records(), journal.records());
    Ok(())
}

#[test]
fn capture_failure_notifications() -> TestResult {
    // Section 12.2: storage, battery and microphone failures are signalled
    // immediately and non-intrusively. `t001`'s `REQ-12-018` row asks for the
    // alert inside a configured latency, no loud interruption, and a failure
    // marker in the timeline. All three are checked per failure.
    let directory = tempfile::tempdir()?;
    let book = CapturePolicyBook::published();
    let row = shipped_row()?;

    let cases: [(FailureKind, PreflightReading); 3] = [
        (
            FailureKind::StorageExhausted,
            PreflightReading::observed(4_096, 80, false, MicrophoneState::Held),
        ),
        (
            FailureKind::BatteryCritical,
            PreflightReading::observed(4 * 1024 * 1024 * 1024, 2, false, MicrophoneState::Held),
        ),
        (
            FailureKind::MicrophoneLost,
            PreflightReading::observed(4 * 1024 * 1024 * 1024, 80, false, MicrophoneState::Lost),
        ),
    ];

    for (expected, reading) in cases {
        let mut ledger = ledger_permitting()?;
        let path = journal_path(&directory, expected.as_str());
        let mut recorder = begin(
            &mut ledger,
            &request()?,
            &path,
            &book,
            healthy_reading(),
            INSIDE,
        )?;
        recorder.record_audio_chunk(&mut ledger, chunk("before-failure"), 0, INSIDE)?;

        let raised = recorder.observe(reading, SECOND, SECOND + 500_000_000)?;
        let signal = *raised.first().ok_or("the failure raised no signal")?;
        assert_eq!(raised.len(), 1, "one reading raised more than its failure");
        assert_eq!(signal.kind(), expected);

        // Immediately: inside the row's own bound, not a constant's.
        assert!(
            signal.within(row),
            "{} took {}ns, past the effective {}ns",
            expected.as_str(),
            signal.latency_nanos(),
            row.notification_within_nanos()
        );
        // Non-intrusively: the delivery is one of two silent forms, and there
        // is no third form to be.
        assert!(
            academic_capture::SignalDelivery::ALL.contains(&signal.delivery()),
            "the delivery is outside the silent vocabulary"
        );
        // A marker in the timeline, durable, followed by the explicit gap.
        let recovered = recorder.verify_on_disk()?;
        let bodies: Vec<&'static str> = recovered
            .records()
            .iter()
            .map(|record| record.body().kind_str())
            .collect();
        assert_eq!(bodies, vec!["AUDIO_CHUNK", "FAILURE_SIGNAL", "GAP"]);
        assert_eq!(recorder.stopped(), Some(GapCause::ResourceFailure));

        // A stopped capture does not resume in place.
        let after = recorder.record_audio_chunk(&mut ledger, chunk("after"), 4 * SECOND, INSIDE);
        assert!(
            matches!(after, Err(CaptureFault::Stopped(GapCause::ResourceFailure))),
            "a stopped capture accepted another chunk: {after:?}"
        );
    }
    Ok(())
}

#[test]
fn capture_metadata_integrity() -> TestResult {
    // `t001`'s `REQ-12-015` row: a rotated fixture at a known audio time, with
    // the original bytes and hash plus an exact EXIF-independent orientation,
    // instant and offset.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, "metadata");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;

    // Audio starts at three seconds, so the audio epoch is not the origin and
    // the offset below is a real subtraction rather than the tick restated.
    recorder.record_audio_chunk(&mut ledger, chunk("audio-epoch"), 3 * SECOND, INSIDE)?;
    let epoch = recorder.audio_epoch();
    assert_eq!(epoch.elapsed_nanos(), 3 * SECOND);

    for (index, orientation) in Orientation::ALL.into_iter().enumerate() {
        let original = image(&format!("board-{}", orientation.code()));
        let elapsed = (4 + u64::try_from(index)?).saturating_mul(SECOND);
        let record =
            recorder.capture_image(&mut ledger, original.clone(), orientation, elapsed, INSIDE)?;
        let seq = record.seq();
        let RecordBody::ImageCapture {
            bytes,
            orientation: stored,
            audio_clock_offset_nanos,
        } = record.body()
        else {
            return Err("an image capture is not an image frame".into());
        };
        // The original bytes, unchanged, and their own digest.
        assert_eq!(bytes.as_slice(), original.as_slice(), "the bytes changed");
        assert_eq!(
            bytes.digest(),
            academic_domain::ContentDigest::sha256(&original),
            "the stored digest is not the original's"
        );
        // The orientation is the caller's, exactly, and the bytes carry no EXIF
        // block for a reader to prefer.
        assert_eq!(*stored, orientation);
        assert!(
            !original.windows(4).any(|window| window == b"Exif"),
            "the fixture grew an EXIF block, so the orientation is no longer EXIF-independent"
        );
        // The offset is the distance from the audio epoch on the one clock, so
        // the instant and the offset agree exactly. A second clock for the
        // image path is what would break this identity.
        assert_eq!(
            *audio_clock_offset_nanos,
            i64::try_from(elapsed)? - i64::try_from(3 * SECOND)?
        );
        assert_eq!(
            record.at().elapsed_nanos(),
            epoch
                .elapsed_nanos()
                .saturating_add(u64::try_from(*audio_clock_offset_nanos)?),
            "frame {seq}: the instant and the audio-clock offset disagree"
        );
        assert_eq!(record.at().domain(), epoch.domain());
    }

    // Durable, and identical when read by something that never saw the writer.
    let recovered = recorder.verify_on_disk()?;
    assert_eq!(recovered.records(), recorder.records());
    Ok(())
}

#[test]
fn mark_now_label_later() -> TestResult {
    // Section 12.2: one action during recording stores a bare mark; the label
    // comes after class. The rule the row exists for is that the later label
    // never moves the original mark time.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, "mark");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;

    // 42:18 into the lecture, which is section 36.2's own instant.
    let mark_at = (42 * 60 + 18) * SECOND;
    recorder.record_audio_chunk(&mut ledger, chunk("lecture"), 0, INSIDE)?;
    let mark = recorder.mark(&mut ledger, mark_at, INSIDE)?;
    assert_eq!(mark.at().elapsed_nanos(), mark_at);
    assert_eq!(
        recorder
            .marks()
            .resolve(mark.seq())
            .and_then(|held| held.label()),
        None,
        "the bare mark arrived with a label"
    );
    let mark_frame = recorder
        .records()
        .iter()
        .find(|record| matches!(record.body(), RecordBody::Mark { .. }))
        .cloned()
        .ok_or("the mark reached no frame")?;

    // The label arrives after class, at a much later instant, twice.
    let after_class = 90 * 60 * SECOND;
    for (offset, kind) in [
        (0, MarkLabelKind::Question),
        (SECOND, MarkLabelKind::Review),
    ] {
        let labelled = recorder.label_mark(
            &mut ledger,
            mark.seq(),
            kind,
            after_class.saturating_add(offset),
            INSIDE,
        )?;
        assert_eq!(labelled.label(), Some(kind), "the label did not apply");
        assert_eq!(
            labelled.at().elapsed_nanos(),
            mark_at,
            "the label moved the mark to {}",
            labelled.at().elapsed_nanos()
        );
        assert_eq!(labelled.mark(), mark, "the mark itself changed");
    }
    // Append-only: two labels, both kept, the last one current.
    assert_eq!(recorder.marks().labels().len(), 2);
    assert_eq!(recorder.marks().marks(), &[mark]);

    // The durable half. The mark's frame is chain-digested, so a label appended
    // after it cannot edit it without breaking every digest that follows.
    let recovered = recorder.verify_on_disk()?;
    let durable_mark = recovered
        .records()
        .iter()
        .find(|record| matches!(record.body(), RecordBody::Mark { .. }))
        .ok_or("the mark left no durable frame")?;
    assert_eq!(durable_mark, &mark_frame, "the mark's frame was rewritten");
    assert_eq!(durable_mark.at().elapsed_nanos(), mark_at);

    // And the frame really is load-bearing: flipping one byte of the mark's
    // recorded instant on disk is refused rather than read back as a new time.
    let mut bytes = std::fs::read(&path)?;
    let elapsed_at = find_mark_elapsed_offset(&bytes).ok_or("the mark frame is not in the file")?;
    let victim = bytes.get_mut(elapsed_at).ok_or("offset outside the file")?;
    *victim ^= 0x01;
    let tampered = ChunkJournal::replay(&bytes);
    assert!(
        matches!(
            tampered,
            Err(academic_capture::JournalFault::FrameCorrupt { .. })
        ),
        "a mark's instant was edited on disk and read back: {tampered:?}"
    );
    Ok(())
}

/// The byte offset of the mark frame's elapsed field, found by replaying.
fn find_mark_elapsed_offset(bytes: &[u8]) -> Option<usize> {
    // The header is 104 bytes and a frame header is 53: 4 sequence, 1 kind, 4
    // tick sequence, then 8 elapsed. Walking the frames is the only way to find
    // the mark's, because the bodies before it are not fixed width.
    let mut at = 104_usize;
    while at < bytes.len() {
        let kind = *bytes.get(at.checked_add(4)?)?;
        if kind == 3 {
            return at.checked_add(9);
        }
        let len_bytes: [u8; 4] = bytes
            .get(at.checked_add(17)?..at.checked_add(21)?)?
            .try_into()
            .ok()?;
        let body_len = usize::try_from(u32::from_be_bytes(len_bytes)).ok()?;
        at = at.checked_add(53)?.checked_add(body_len)?.checked_add(32)?;
    }
    None
}

#[test]
fn shared_session_clock_for_audio_and_capture() -> TestResult {
    // Section 34.1's prevention cell: one common session clock for the capture
    // and the audio process. "Shared" is the contract, so this row is not two
    // instants that happen to agree -- it is that there is one clock, that both
    // paths derive from it, and that a tick from a second clock is refused.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, "shared-clock");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    let domain = recorder.clock_domain();

    for index in 0..3_u64 {
        recorder.record_audio_chunk(
            &mut ledger,
            chunk(&format!("a{index}")),
            index.saturating_mul(SECOND),
            INSIDE,
        )?;
        recorder.capture_image(
            &mut ledger,
            image(&format!("i{index}")),
            Orientation::RightTop,
            index.saturating_mul(SECOND).saturating_add(SECOND / 2),
            INSIDE,
        )?;
    }
    recorder.mark(&mut ledger, 4 * SECOND, INSIDE)?;

    // Every frame either path wrote carries the same domain, and the file's
    // header names it, so a reader that never saw the recorder reaches the same
    // conclusion.
    let recovered = recorder.verify_on_disk()?;
    assert_eq!(recovered.header().domain(), domain);
    assert!(recovered.records().len() >= 7);
    for record in recovered.records() {
        assert_eq!(
            record.at().domain(),
            domain,
            "frame {} came from another clock",
            record.seq()
        );
    }
    // And the sequence is one sequence: the two paths interleave rather than
    // each counting from zero, which two clocks could not produce.
    let ticks: Vec<u32> = recovered.records().iter().map(|r| r.at().seq()).collect();
    let expected: Vec<u32> = (1..=u32::try_from(ticks.len())?).collect();
    assert_eq!(ticks, expected, "the tick sequence is not one clock's");

    // The injection. A second clock over the same lecture and the same token is
    // a value this test can build, and the anchors it mints are refused.
    let second = SessionClock::start(common::lecture()?, recorder.token_id(), None);
    assert_ne!(second.domain(), domain, "two clocks share a domain");
    let mut second = second;
    let foreign_first = Anchor::at(second.tick(SECOND)?, SECOND);
    let foreign_second = Anchor::at(second.tick(2 * SECOND)?, 2 * SECOND);
    let refused = recorder.realign(foreign_first, foreign_second, 5 * SECOND);
    assert!(
        matches!(
            refused,
            Err(CaptureFault::Alignment(AlignmentFault::ForeignClock(_)))
        ),
        "an anchor from a second clock was accepted: {refused:?}"
    );
    // A mixed pair is refused too, so one good anchor does not carry a bad one.
    let own = Anchor::at(recorder.records().first().ok_or("no frames")?.at(), SECOND);
    let mixed = recorder.realign(own, foreign_second, 5 * SECOND);
    assert!(
        matches!(
            mixed,
            Err(CaptureFault::Alignment(AlignmentFault::ForeignClock(_)))
        ),
        "a mixed anchor pair was accepted: {mixed:?}"
    );
    Ok(())
}

#[test]
fn drift_beyond_tolerance_is_alignment_low_confidence() -> TestResult {
    // Section 34.1's uncertainty cell: `±초 오차 범위와 ALIGNMENT_LOW_CONFIDENCE`.
    // Past the tolerance is low confidence -- not a refusal and not silence.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let row = shipped_row()?;
    let tolerance = row.drift_tolerance_nanos();
    let path = journal_path(&directory, "drift");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    recorder.record_audio_chunk(&mut ledger, chunk("drift"), 0, INSIDE)?;
    let base = recorder.records().first().ok_or("no frames")?.at();
    let later = recorder.mark(&mut ledger, 600 * SECOND, INSIDE)?.at();

    // Both sides of the boundary, with the same anchors moved by one nanosecond.
    let first = Anchor::at(base, base.elapsed_nanos());
    let at_tolerance = Anchor::at(later, later.elapsed_nanos().saturating_sub(tolerance));
    let past_tolerance = Anchor::at(
        later,
        later
            .elapsed_nanos()
            .saturating_sub(tolerance)
            .saturating_sub(1),
    );

    let inside = estimate_drift(first, at_tolerance, row)?;
    assert_eq!(inside.drift_nanos(), i64::try_from(tolerance)?);
    assert_eq!(inside.confidence(), AlignmentConfidence::Normal);
    assert_eq!(inside.confidence().badge(), None);

    let outside = estimate_drift(first, past_tolerance, row)?;
    assert_eq!(outside.drift_nanos(), i64::try_from(tolerance)? + 1);
    assert!(
        outside.confidence().is_low(),
        "one nanosecond past tolerance is not low"
    );
    assert_eq!(
        outside.confidence().badge(),
        Some(ALIGNMENT_LOW_CONFIDENCE),
        "the low-confidence arm carries the wrong badge"
    );
    assert_eq!(
        outside.confidence().plus_minus_seconds(),
        tolerance.saturating_add(1).div_ceil(SECOND),
        "the ± range is not the drift"
    );
    // Beyond tolerance is not a refusal: the estimate is still an estimate and
    // the offset it carries is still the first anchor's.
    assert_eq!(outside.offset_nanos(), inside.offset_nanos());

    // The tolerance is a row, not a constant. The same anchors under a book
    // whose effective row is looser are normal, and under a stricter one are
    // low -- which a hard-coded comparison could not produce.
    let looser = CapturePolicyBook::of(vec![CapturePolicyRow::declare(
        "capture.thresholds.fixture-looser",
        INSIDE,
        tolerance.saturating_add(2),
        row.storage_floor_bytes(),
        row.battery_floor_percent(),
        row.notification_within_nanos(),
    )]);
    let looser_row = looser
        .effective_at(INSIDE)
        .ok_or("the looser book reaches no row")?;
    assert_eq!(
        estimate_drift(first, past_tolerance, looser_row)?.confidence(),
        AlignmentConfidence::Normal,
        "a looser row did not move the boundary"
    );
    // And the date is load-bearing: the same looser row dated after the capture
    // is not selected, so the shipped row still decides.
    let not_yet = CapturePolicyBook::of(vec![
        row,
        CapturePolicyRow::declare(
            "capture.thresholds.fixture-later",
            INSIDE.saturating_add(1),
            tolerance.saturating_add(2),
            row.storage_floor_bytes(),
            row.battery_floor_percent(),
            row.notification_within_nanos(),
        ),
    ]);
    let selected = not_yet
        .effective_at(INSIDE)
        .ok_or("no row at the instant")?;
    assert_eq!(selected.id(), row.id(), "a row dated later was selected");
    assert!(
        estimate_drift(first, past_tolerance, selected)?
            .confidence()
            .is_low(),
        "the effective date did not decide the tolerance"
    );
    Ok(())
}

#[test]
fn manual_two_anchor_realignment_appends_a_mapping_version() -> TestResult {
    // Section 34.1's recovery cell: 수동 anchor 2개로 재정렬, mapping version 추가.
    // "Appends" is the contract: the prior mapping is still there and still says
    // what it said.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, "realign");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    recorder.record_audio_chunk(&mut ledger, chunk("realign"), 0, INSIDE)?;
    let base = recorder.records().first().ok_or("no frames")?.at();
    let later = recorder.mark(&mut ledger, 300 * SECOND, INSIDE)?.at();

    let first = recorder.realign(
        Anchor::at(base, 0),
        Anchor::at(later, later.elapsed_nanos().saturating_sub(SECOND)),
        301 * SECOND,
    )?;
    assert_eq!(first.version(), 1);
    let snapshot = first;

    let second = recorder.realign(
        Anchor::at(base, 0),
        Anchor::at(later, later.elapsed_nanos().saturating_sub(5 * SECOND)),
        302 * SECOND,
    )?;
    assert_eq!(second.version(), 2);
    assert_ne!(
        second.estimate().drift_nanos(),
        first.estimate().drift_nanos()
    );

    // Version one is still there and byte-for-byte what it was.
    let versions = recorder.mapping().versions();
    assert_eq!(versions.len(), 2, "the second realignment did not append");
    assert_eq!(
        versions.first().copied(),
        Some(snapshot),
        "version one changed"
    );
    assert_eq!(recorder.mapping().current(), Some(second));

    // Both versions reached the journal, in order, and neither frame was
    // rewritten when the next one arrived.
    let recovered = recorder.verify_on_disk()?;
    let mapped: Vec<u32> = recovered
        .records()
        .iter()
        .filter_map(|record| match record.body() {
            RecordBody::MappingVersion { version, .. } => Some(*version),
            _ => None,
        })
        .collect();
    assert_eq!(mapped, vec![1, 2]);
    // Each version cites the row whose tolerance decided its confidence.
    for version in versions {
        assert_eq!(version.policy_id(), shipped_row()?.id());
    }
    // Two anchors at the same instant measure nothing and are refused rather
    // than dividing by an interval of zero.
    let coincident = recorder.realign(Anchor::at(base, 0), Anchor::at(base, SECOND), 303 * SECOND);
    assert!(
        matches!(
            coincident,
            Err(CaptureFault::Alignment(AlignmentFault::AnchorsCoincide))
        ),
        "two anchors at one instant produced a mapping: {coincident:?}"
    );
    assert_eq!(recorder.mapping().versions().len(), 2);
    Ok(())
}

#[test]
fn a_permission_that_stops_mid_lecture_stops_the_capture() -> TestResult {
    // `CP01` restated at this surface. `academic-capture-gate` owns the device
    // half; what is checked here is that the journal surface re-runs the same
    // one binding per record rather than trusting the token it began with, and
    // that the frames already written stay whole.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, "boundary");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    recorder.record_audio_chunk(&mut ledger, chunk("live"), 0, INSIDE)?;

    append_refusal(&mut ledger, INSIDE.saturating_add(1))?;
    let refused = recorder.record_audio_chunk(
        &mut ledger,
        chunk("after-refusal"),
        SECOND,
        INSIDE.saturating_add(2),
    );
    assert!(
        matches!(refused, Err(CaptureFault::Permission(_))),
        "a superseding refusal did not stop the capture: {refused:?}"
    );
    assert_eq!(recorder.stopped(), Some(GapCause::PermissionRefused));
    let recovered = recorder.verify_on_disk()?;
    assert_eq!(
        recovered.records().len(),
        1,
        "a refused chunk reached the file"
    );
    assert_eq!(recovered.partial_tail_bytes(), 0);

    // An expired grant is the other way in, and it stops the capture at the
    // same seam rather than at a comparison of the token's own `not_after`.
    let mut expiring = common::ledger_granting(
        vec![
            academic_consent::CaptureMedium::Audio,
            academic_consent::CaptureMedium::PhotoOfBoard,
        ],
        INSIDE.saturating_add(10),
    )?;
    let expiring_path = journal_path(&directory, "expiring");
    let expiring_request = common::request_until(INSIDE.saturating_add(10))?;
    let mut expiring_recorder = begin(
        &mut expiring,
        &expiring_request,
        &expiring_path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    expiring_recorder.record_audio_chunk(&mut expiring, chunk("inside"), 0, INSIDE)?;
    let past = expiring_recorder.record_audio_chunk(
        &mut expiring,
        chunk("past"),
        SECOND,
        INSIDE.saturating_add(11),
    );
    // The grant stops at `INSIDE + 10`, so the instant above is one past it and
    // the binding is what notices -- not a comparison of the token's own
    // `not_after`, which this surface never makes.
    assert!(
        matches!(past, Err(CaptureFault::Permission(_))),
        "an expired grant did not stop the capture: {past:?}"
    );
    Ok(())
}

#[test]
fn the_clock_refuses_a_reading_that_went_backwards() -> TestResult {
    // There is no path by which a wall-clock rollback becomes elapsed time. The
    // reading is refused, not clamped: clamping would put two different real
    // instants on one tick, which is the silent re-timestamping section 34.1
    // forbids one row above.
    let directory = tempfile::tempdir()?;
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let path = journal_path(&directory, "backwards");
    let mut recorder = begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    recorder.record_audio_chunk(&mut ledger, chunk("forward"), 10 * SECOND, INSIDE)?;
    let backwards =
        recorder.record_audio_chunk(&mut ledger, chunk("backwards"), 9 * SECOND, INSIDE);
    assert!(
        matches!(
            backwards,
            Err(CaptureFault::Clock(
                academic_capture::ClockFault::WentBackwards {
                    offered: 9_000_000_000,
                    accepted: 10_000_000_000
                }
            ))
        ),
        "a reading that went backwards was accepted: {backwards:?}"
    );
    // Equal readings are fine and still get their own place in the sequence.
    recorder.record_audio_chunk(&mut ledger, chunk("same"), 10 * SECOND, INSIDE)?;
    let ticks: Vec<u32> = recorder.records().iter().map(|r| r.at().seq()).collect();
    assert_eq!(ticks, vec![1, 2]);
    assert!(check_no_frame_went_backwards(&path)?);
    Ok(())
}

fn check_no_frame_went_backwards(path: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let recovered = ChunkJournal::recover(path)?;
    Ok(recovered
        .records()
        .windows(2)
        .all(|pair| match (pair.first(), pair.get(1)) {
            (Some(earlier), Some(later)) => {
                later.at().elapsed_nanos() >= earlier.at().elapsed_nanos()
            }
            _ => true,
        }))
}

/// A frame whose instant is below the frame before it, on the same clock, is
/// refused -- and a resumed clock's first frame is not.
///
/// `SessionClock::tick` orders the instants a clock *mints*.
/// `the_clock_refuses_a_reading_that_went_backwards` is that half, and it is
/// the whole of what the clock can promise. `ChunkJournal::append` is public,
/// takes a tick rather than a reading, and orders the instants the *file*
/// holds: a caller holding two ticks from one clock can offer them in either
/// order, and before this row it could. Measured before the repair: the second
/// append returned `Ok`, the frame instants were `[9000, 1000]`, and the chain
/// still verified over both frames.
///
/// | Offered against the last frame, same clock | Outcome |
/// |---|---|
/// | one tick below | refused, `FrameOutOfOrder` |
/// | equal | accepted |
/// | one tick above | accepted |
/// | below, but minted by another clock | accepted -- see below |
#[test]
fn out_of_order_frame_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = journal_path(&directory, "ordered");
    let token = ContentDigest::sha256(b"t161-token");
    let mut clock = SessionClock::start(common::lecture()?, &token, None);
    let early = clock.tick(1_000)?;
    let same = clock.tick(9_000)?;
    let late = clock.tick(9_000)?;
    let later = clock.tick(9_001)?;
    let mut journal = ChunkJournal::create(
        &path,
        clock.domain(),
        ContentDigest::sha256(b"t161-policy"),
        token,
    )?;
    journal.append(
        late,
        RecordBody::AudioChunk {
            bytes: CaptureBytes::of(chunk("late")),
        },
    )?;

    let backwards = journal.append(
        early,
        RecordBody::AudioChunk {
            bytes: CaptureBytes::of(chunk("early")),
        },
    );
    assert!(
        matches!(
            backwards,
            Err(academic_capture::JournalFault::FrameOutOfOrder {
                offered: 1_000,
                recorded: 9_000
            })
        ),
        "a frame earlier than the one before it was appended: {backwards:?}"
    );
    assert_eq!(journal.records().len(), 1, "the refused frame was kept");

    // Equal, then one nanosecond above. Both are accepted, for the reason equal
    // readings are: two events can share a nanosecond and still need an order.
    journal.append(
        same,
        RecordBody::AudioChunk {
            bytes: CaptureBytes::of(chunk("same")),
        },
    )?;
    journal.append(
        later,
        RecordBody::AudioChunk {
            bytes: CaptureBytes::of(chunk("later")),
        },
    )?;
    assert_eq!(journal.records().len(), 3);

    // Nothing about the refused frame is on disk, and the chain over what is
    // there still verifies -- the refusal happens before the first write.
    let recovered = ChunkJournal::recover(&path)?;
    assert_eq!(recovered.records().len(), 3);
    assert_eq!(recovered.partial_tail_bytes(), 0);
    let instants: Vec<u64> = recovered
        .records()
        .iter()
        .map(|record| record.at().elapsed_nanos())
        .collect();
    assert_eq!(instants, vec![9_000, 9_000, 9_001]);
    assert!(check_no_frame_went_backwards(&path)?);

    // A resume is the case the comparison must not catch. The new clock starts
    // at its own origin, so its first frame's instant is below every frame the
    // previous clock wrote -- and that discontinuity is what the `GAP` frame
    // records rather than something to refuse. Across domains there is no
    // defined distance, which is the same reading `SessionTick::offset_from`
    // takes.
    let mut ledger = ledger_permitting()?;
    let book = CapturePolicyBook::published();
    let resumed_path = journal_path(&directory, "resumed");
    let mut first = begin(
        &mut ledger,
        &request()?,
        &resumed_path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    first.record_audio_chunk(&mut ledger, chunk("before"), 30 * SECOND, INSIDE)?;
    drop(first);
    let (mut second, _) = academic_capture::resume(
        &mut ledger,
        &request()?,
        &resumed_path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    second.record_audio_chunk(&mut ledger, chunk("after"), SECOND, INSIDE)?;
    let after = ChunkJournal::recover(&resumed_path)?;
    let kinds: Vec<&'static str> = after
        .records()
        .iter()
        .map(|record| record.body().kind_str())
        .collect();
    assert_eq!(kinds, vec!["AUDIO_CHUNK", "GAP", "AUDIO_CHUNK"]);
    let across: Vec<u64> = after
        .records()
        .iter()
        .map(|record| record.at().elapsed_nanos())
        .collect();
    assert_eq!(
        across,
        vec![30 * SECOND, 0, SECOND],
        "the resumed frames were forced onto the dead clock's numbering"
    );
    let domains: Vec<_> = after
        .records()
        .iter()
        .map(|record| record.at().domain())
        .collect();
    assert_ne!(
        domains.first(),
        domains.get(1),
        "the resumed frame claims the domain of the clock that died"
    );
    assert_eq!(domains.get(1), domains.get(2));
    Ok(())
}

/// Two anchors whose second sits earlier than their first are refused, not
/// reordered.
///
/// The confidence badge cannot see the difference -- it reads the magnitude, so
/// a swapped pair produces the same `ALIGNMENT_LOW_CONFIDENCE` and the same ±
/// range. What changes is `offset_nanos`, which becomes the offset the *other*
/// anchor fixes, and the sign of `drift_nanos`. Measured before the repair:
/// `forwards` gave `offset_nanos: 0, drift_nanos: 9000000000` and `backwards`
/// gave `offset_nanos: 9000000000, drift_nanos: -9000000000`, both
/// `Low { plus_minus_nanos: 9000000000 }`.
///
/// It is refused rather than swapped because a `MappingVersion` is the
/// append-only record of what the user asserted, and reordering a user's two
/// inputs records a pair they did not give.
#[test]
fn anchors_out_of_order_are_refused() -> TestResult {
    let token = ContentDigest::sha256(b"t161-anchor-token");
    let mut clock = SessionClock::start(common::lecture()?, &token, None);
    let earlier = clock.tick(10 * SECOND)?;
    let just_after = clock.tick(10 * SECOND + 1)?;
    let later = clock.tick(40 * SECOND)?;
    let policy = shipped_row()?;
    let first = Anchor::at(earlier, 10 * SECOND);
    let second = Anchor::at(later, 31 * SECOND);

    let forwards = estimate_drift(first, second, policy)?;
    assert_eq!(forwards.offset_nanos(), 0);
    assert_eq!(forwards.drift_nanos(), 9 * SECOND as i64);

    let backwards = estimate_drift(second, first, policy);
    assert_eq!(
        backwards,
        Err(AlignmentFault::AnchorsOutOfOrder),
        "a pair whose second anchor sits earlier was accepted: {backwards:?}"
    );

    // The boundary on the other side is the one that was already there: equal
    // instants measure nothing over no interval, and that is its own refusal.
    let coincide = estimate_drift(first, Anchor::at(earlier, 20 * SECOND), policy);
    assert_eq!(coincide, Err(AlignmentFault::AnchorsCoincide));

    // One nanosecond apart is an interval, and it is accepted in the forward
    // direction and refused in the other.
    let narrow = Anchor::at(just_after, 10 * SECOND + 1);
    assert!(estimate_drift(first, narrow, policy).is_ok());
    assert_eq!(
        estimate_drift(narrow, first, policy),
        Err(AlignmentFault::AnchorsOutOfOrder)
    );
    Ok(())
}
