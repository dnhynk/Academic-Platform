//! The one-action Record/Capture/Mark surface, and everything it owns.
//!
//! # One action
//!
//! [`begin`] is the whole of starting a capture: it evaluates the section 3.7
//! permission, selects the effective policy row, runs preflight, starts the one
//! session clock and creates the journal. There is no public constructor for
//! [`CaptureRecorder`], so a value of it is proof that all five happened, and a
//! refusal at any of them leaves nothing on disk — `capture_one_action_authorization`
//! observes that the journal file does not exist after a refused begin.
//!
//! # This surface adds no comparison of its own
//!
//! `academic-consent`'s `bind_permission` is where every section 3.7 comparison
//! happens, and `P2-RF10` is why there is exactly one of it. [`begin`] calls
//! `mint_capture_capability` and every recording call re-runs the same binding
//! through `continue_capture`, exactly as `academic-capture-gate`'s session
//! does. Neither crate compares a status, a lifetime or a medium itself.
//!
//! **This crate has no dependency edge to `academic-capture-gate`, and that is
//! not an oversight.** That package carries a platform backend and a probe
//! binary, and `only_egress_crate_has_a_socket` fails the day any workspace
//! crate depends on it, because the probe would then be reachable from a
//! default build. `P2-G2`'s precedent is to split rather than to weaken a
//! guard, so the two crates are siblings over one shared binding. What that
//! costs is written down as open item `C-9` in
//! [the capture subsystem contract](../../../docs/contracts/capture-subsystem.md).
//!
//! # Every instant comes from the one clock
//!
//! [`CaptureRecorder`] holds one [`SessionClock`], the crate has exactly one
//! call to [`SessionClock::start`], and every record this module writes takes
//! its instant from `self.clock.tick`. An anchor offered from outside is
//! admitted through the same clock and refused if it came from another, which
//! is the reachable second-clock injection.

use std::path::Path;

use academic_consent::{
    CaptureCapabilityToken, CaptureDenial, CaptureRequest, ConsentLedger, RetentionTerms,
    continue_capture, mint_capture_capability,
};
use academic_domain::{ContentDigest, LectureSessionId, OfferingId};

use crate::{
    align::{AlignmentFault, Anchor, MappingLedger, MappingVersion},
    capture::{CaptureBytes, Orientation},
    clock::{ClockFault, SessionClock, SessionClockDomain, SessionTick},
    journal::{
        ChunkJournal, GapCause, JournalFault, JournalHeader, JournalRecord, JournalRecovery,
        RecordBody, mapping_version_body,
    },
    mark::{LabelledMark, Mark, MarkFault, MarkLabelKind, MarkLedger},
    policy::{CapturePolicyBook, CapturePolicyRow},
    preflight::{FailureKind, FailureSignal, PreflightReading},
};

/// Why the surface refused.
#[derive(Debug, thiserror::Error)]
pub enum CaptureFault {
    /// The section 3.7 binding refused. The reason is the binding's own.
    #[error("the capture permission does not cover this: {0}")]
    Permission(#[from] CaptureDenial),
    /// No policy row reaches this instant, so no threshold is known.
    ///
    /// Refused rather than defaulted: a default would be a claim about a period
    /// no decision in [`CapturePolicyBook`] covers.
    #[error("no capture policy row is effective at {at}")]
    NoEffectivePolicy {
        /// The instant that reached no row.
        at: u64,
    },
    /// Preflight found one or more resources below the effective floor.
    #[error("preflight refused: {0:?}")]
    Preflight(Vec<FailureKind>),
    /// The session clock refused a reading or a foreign tick.
    #[error("the session clock refused: {0}")]
    Clock(#[from] ClockFault),
    /// The journal refused.
    #[error("the journal refused: {0}")]
    Journal(#[from] JournalFault),
    /// A realignment refused.
    #[error("the realignment refused: {0}")]
    Alignment(#[from] AlignmentFault),
    /// A label named no mark.
    #[error("the label refused: {0}")]
    Mark(#[from] MarkFault),
    /// The capture has stopped, and a stopped capture does not resume in place.
    #[error("the capture stopped: {0:?}")]
    Stopped(GapCause),
}

/// A running capture.
///
/// Private fields, and [`begin`] and [`resume`] are the only places a value is
/// built.
#[derive(Debug)]
pub struct CaptureRecorder {
    token: CaptureCapabilityToken,
    clock: SessionClock,
    journal: ChunkJournal,
    policy: CapturePolicyRow,
    origin: SessionTick,
    first_audio: Option<SessionTick>,
    marks: MarkLedger,
    mapping: MappingLedger,
    signals: Vec<FailureSignal>,
    stopped: Option<GapCause>,
}

/// Starts a capture: authorize, select the policy, preflight, clock, journal.
///
/// `now` is on the permission axis — the instant `academic-consent` compares a
/// grant's lifetime against, and the instant [`CapturePolicyBook::effective_at`]
/// selects a row with. It is deliberately not a [`SessionTick`], which measures
/// elapsed time inside one session and cannot order two.
pub fn begin(
    ledger: &mut ConsentLedger,
    request: &CaptureRequest,
    journal_path: &Path,
    book: &CapturePolicyBook,
    reading: PreflightReading,
    now: u64,
) -> Result<CaptureRecorder, CaptureFault> {
    let token = mint_capture_capability(ledger, request, now)?;
    let policy = book
        .effective_at(now)
        .ok_or(CaptureFault::NoEffectivePolicy { at: now })?;
    let failures = reading.failures(policy);
    if !failures.is_empty() {
        return Err(CaptureFault::Preflight(failures));
    }
    start_session(token, policy, journal_path, None)
}

/// Recovers a killed capture and continues it under a new clock.
///
/// Fault `CP05`. The recovered journal keeps every frame that was whole, its
/// partial tail is dropped, and the first frame this appends is an explicit
/// [`GapCause::SessionResumed`] carrying the new clock's domain — so no frame
/// written before the kill is re-timestamped onto the clock written after it.
pub fn resume(
    ledger: &mut ConsentLedger,
    request: &CaptureRequest,
    journal_path: &Path,
    book: &CapturePolicyBook,
    reading: PreflightReading,
    now: u64,
) -> Result<(CaptureRecorder, JournalRecovery), CaptureFault> {
    let token = mint_capture_capability(ledger, request, now)?;
    let policy = book
        .effective_at(now)
        .ok_or(CaptureFault::NoEffectivePolicy { at: now })?;
    let failures = reading.failures(policy);
    if !failures.is_empty() {
        return Err(CaptureFault::Preflight(failures));
    }
    let (journal, recovery) = ChunkJournal::reopen(journal_path)?;
    let recorder = start_session(token, policy, journal_path, Some(journal))?;
    Ok((recorder, recovery))
}

/// The one place a session clock is started and a recorder is built.
///
/// Both entry points funnel through it, so `SessionClock::start` has exactly
/// one call site in this crate's product source and
/// `the_only_instant_type_comes_from_one_clock` can pin it at one.
fn start_session(
    token: CaptureCapabilityToken,
    policy: CapturePolicyRow,
    journal_path: &Path,
    recovered: Option<ChunkJournal>,
) -> Result<CaptureRecorder, CaptureFault> {
    let lecture_id = token.bound().lecture_id();
    let token_id = *token.token_id();
    // A resumed clock is derived from the tail of the journal it continues, so
    // it cannot be the clock that wrote those frames: they were written before
    // that digest existed. A fresh session has no predecessor.
    let predecessor = recovered
        .as_ref()
        .filter(|journal| !journal.records().is_empty())
        .map(|journal| *journal.tail());
    let mut clock = SessionClock::start(lecture_id, &token_id, predecessor.as_ref());
    let origin = clock.tick(0)?;
    let mut journal = match recovered {
        Some(journal) => journal,
        None => ChunkJournal::create(journal_path, clock.domain(), policy.digest(), token_id)?,
    };
    if !journal.records().is_empty() {
        journal.append(
            origin,
            RecordBody::Gap {
                cause: GapCause::SessionResumed,
                resumed_domain: Some(clock.domain()),
            },
        )?;
    }
    Ok(CaptureRecorder {
        token,
        clock,
        journal,
        policy,
        origin,
        first_audio: None,
        marks: MarkLedger::new(),
        mapping: MappingLedger::new(),
        signals: Vec::new(),
        stopped: None,
    })
}

impl CaptureRecorder {
    /// The identity of the one clock every instant here comes from.
    #[must_use]
    pub const fn clock_domain(&self) -> SessionClockDomain {
        self.clock.domain()
    }

    /// The origin tick the session opened at.
    #[must_use]
    pub const fn origin(&self) -> SessionTick {
        self.origin
    }

    /// The instant audio offsets are measured from: the first audio chunk, or
    /// the session origin while there is none.
    #[must_use]
    pub const fn audio_epoch(&self) -> SessionTick {
        match self.first_audio {
            Some(tick) => tick,
            None => self.origin,
        }
    }

    /// The policy row this capture is running under.
    #[must_use]
    pub const fn policy(&self) -> CapturePolicyRow {
        self.policy
    }

    /// The opaque identifier of the capability behind it.
    #[must_use]
    pub const fn token_id(&self) -> &ContentDigest {
        self.token.token_id()
    }

    /// Which offering.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.token.bound().offering_id()
    }

    /// Which lecture.
    #[must_use]
    pub const fn lecture_id(&self) -> LectureSessionId {
        self.token.bound().lecture_id()
    }

    /// The two retention bounds the grant attached.
    #[must_use]
    pub const fn retention(&self) -> RetentionTerms {
        self.token.bound().retention()
    }

    /// The journal file's header.
    #[must_use]
    pub const fn journal_header(&self) -> JournalHeader {
        self.journal.header()
    }

    /// Where the journal is.
    #[must_use]
    pub fn journal_path(&self) -> &Path {
        self.journal.path()
    }

    /// Every frame written so far.
    #[must_use]
    pub fn records(&self) -> &[JournalRecord] {
        self.journal.records()
    }

    /// Every mark and label.
    #[must_use]
    pub const fn marks(&self) -> &MarkLedger {
        &self.marks
    }

    /// Every mapping version.
    #[must_use]
    pub const fn mapping(&self) -> &MappingLedger {
        &self.mapping
    }

    /// Every failure signal raised.
    #[must_use]
    pub fn signals(&self) -> &[FailureSignal] {
        &self.signals
    }

    /// Why the capture stopped, once it has.
    #[must_use]
    pub const fn stopped(&self) -> Option<GapCause> {
        self.stopped
    }

    /// Records one audio chunk.
    ///
    /// The first statement re-runs the whole section 3.7 binding, for
    /// `academic-capture-gate`'s reason: a token minted at one instant says
    /// nothing about a later one. The instant comes from `self.clock`, which is
    /// the only clock this recorder has.
    pub fn record_audio_chunk(
        &mut self,
        ledger: &mut ConsentLedger,
        bytes: Vec<u8>,
        elapsed_nanos: u64,
        now: u64,
    ) -> Result<&JournalRecord, CaptureFault> {
        self.still_running()?;
        self.rebind(ledger, now)?;
        let at = self.clock.tick(elapsed_nanos)?;
        if self.first_audio.is_none() {
            self.first_audio = Some(at);
        }
        let record = self.journal.append(
            at,
            RecordBody::AudioChunk {
                bytes: CaptureBytes::of(bytes),
            },
        )?;
        Ok(record)
    }

    /// Records one image with its orientation and its audio-clock offset.
    ///
    /// The offset is [`SessionTick::offset_from`] between this capture's tick
    /// and [`CaptureRecorder::audio_epoch`], both minted by `self.clock`. In a
    /// two-clock design that number is an estimate the image device makes
    /// against the audio device; here it is a subtraction inside one domain, so
    /// `image.at().elapsed_nanos()` equals
    /// `audio_epoch().elapsed_nanos() + audio_clock_offset_nanos` exactly, and
    /// `capture_metadata_integrity` asserts that identity.
    ///
    /// The bytes are stored as they arrived. Nothing here rotates, re-encodes
    /// or strips them, and the orientation travels as a field beside them.
    pub fn capture_image(
        &mut self,
        ledger: &mut ConsentLedger,
        bytes: Vec<u8>,
        orientation: Orientation,
        elapsed_nanos: u64,
        now: u64,
    ) -> Result<&JournalRecord, CaptureFault> {
        self.still_running()?;
        self.rebind(ledger, now)?;
        let epoch = self.audio_epoch();
        let at = self.clock.tick(elapsed_nanos)?;
        let audio_clock_offset_nanos = at.offset_from(epoch).ok_or(ClockFault::ForeignDomain)?;
        let record = self.journal.append(
            at,
            RecordBody::ImageCapture {
                bytes: CaptureBytes::of(bytes),
                orientation,
                audio_clock_offset_nanos,
            },
        )?;
        Ok(record)
    }

    /// Marks the moment. No label, and none is asked for.
    ///
    /// Section 12.2: "먼저 한 번의 표시만 저장하고". The frame this writes
    /// carries the mark's sequence number and its instant and nothing else.
    pub fn mark(
        &mut self,
        ledger: &mut ConsentLedger,
        elapsed_nanos: u64,
        now: u64,
    ) -> Result<Mark, CaptureFault> {
        self.still_running()?;
        self.rebind(ledger, now)?;
        let at = self.clock.tick(elapsed_nanos)?;
        let mark = self.marks.append_mark(at);
        self.journal.append(
            at,
            RecordBody::Mark {
                mark_seq: mark.seq(),
            },
        )?;
        Ok(mark)
    }

    /// Labels a mark that is already recorded.
    ///
    /// It takes the mark's sequence number and its own instant, and it writes a
    /// new frame. There is no path from here to the mark's frame: the mark is
    /// already chain-digested, so editing it would break every digest after it,
    /// and there is no method that would try.
    ///
    /// It re-binds like every other recording call, so a label applied after
    /// the permission stopped is refused rather than quietly appended.
    pub fn label_mark(
        &mut self,
        ledger: &mut ConsentLedger,
        mark_seq: u32,
        kind: MarkLabelKind,
        elapsed_nanos: u64,
        now: u64,
    ) -> Result<LabelledMark, CaptureFault> {
        self.still_running()?;
        self.rebind(ledger, now)?;
        let at = self.clock.tick(elapsed_nanos)?;
        self.marks.append_label(mark_seq, kind, at)?;
        self.journal
            .append(at, RecordBody::MarkLabel { mark_seq, kind })?;
        self.marks
            .resolve(mark_seq)
            .ok_or(CaptureFault::Mark(MarkFault::UnknownMark { seq: mark_seq }))
    }

    /// Takes one preflight reading and stops the capture if it holds a failure.
    ///
    /// Faults `CP02` and `CP03`. Every failure the reading holds raises its own
    /// signal frame, then one explicit gap closes the timeline with
    /// [`GapCause::ResourceFailure`]. The journal is left whole — the frames
    /// already written are already synced and chain-digested, and this appends
    /// rather than rewrites.
    ///
    /// `observed_at_nanos` is when the host took the reading and
    /// `elapsed_nanos` is when it reached this surface, both on the session
    /// clock, so the latency is in the record rather than asserted by the code
    /// that wrote it.
    pub fn observe(
        &mut self,
        reading: PreflightReading,
        observed_at_nanos: u64,
        elapsed_nanos: u64,
    ) -> Result<Vec<FailureSignal>, CaptureFault> {
        self.still_running()?;
        let failures = reading.failures(self.policy);
        if failures.is_empty() {
            return Ok(Vec::new());
        }
        let mut raised = Vec::with_capacity(failures.len());
        for kind in failures {
            let at = self.clock.tick(elapsed_nanos)?;
            let signal = FailureSignal::raised(kind, at, observed_at_nanos);
            self.journal.append(
                at,
                RecordBody::FailureSignal {
                    kind,
                    delivery: signal.delivery(),
                    observed_at_nanos,
                },
            )?;
            self.signals.push(signal);
            raised.push(signal);
        }
        self.open_gap(GapCause::ResourceFailure, elapsed_nanos)?;
        Ok(raised)
    }

    /// Appends a mapping version from two manual anchors.
    ///
    /// Section 34.1's recovery cell. Both anchors are admitted through this
    /// recorder's own clock, so an anchor minted by a second clock is refused;
    /// and the append never touches a version already in the ledger.
    pub fn realign(
        &mut self,
        first: Anchor,
        second: Anchor,
        elapsed_nanos: u64,
    ) -> Result<MappingVersion, CaptureFault> {
        let version = self
            .mapping
            .append_realignment(&self.clock, first, second, self.policy)?;
        let at = self.clock.tick(elapsed_nanos)?;
        self.journal.append(at, mapping_version_body(version))?;
        Ok(version)
    }

    /// Re-reads the journal from disk and checks every frame.
    ///
    /// The in-memory records are not consulted, so this is an independent
    /// reading of what is durable.
    pub fn verify_on_disk(&self) -> Result<JournalRecovery, CaptureFault> {
        Ok(self.journal.verify_on_disk()?)
    }

    /// Stops the capture with an explicit gap and returns what is durable.
    pub fn seal(
        mut self,
        cause: GapCause,
        elapsed_nanos: u64,
    ) -> Result<SealedCapture, CaptureFault> {
        if self.stopped.is_none() {
            self.open_gap(cause, elapsed_nanos)?;
        }
        let recovery = self.journal.verify_on_disk()?;
        Ok(SealedCapture {
            recovery,
            marks: self.marks,
            mapping: self.mapping,
            signals: self.signals,
            retention: self.token.bound().retention(),
        })
    }

    fn still_running(&self) -> Result<(), CaptureFault> {
        match self.stopped {
            Some(cause) => Err(CaptureFault::Stopped(cause)),
            None => Ok(()),
        }
    }

    /// Re-runs the whole section 3.7 binding, and opens a gap if it refuses.
    fn rebind(&mut self, ledger: &mut ConsentLedger, now: u64) -> Result<(), CaptureFault> {
        match continue_capture(ledger, &self.token, now) {
            Ok(()) => Ok(()),
            Err(denial) => {
                self.stopped = Some(GapCause::PermissionRefused);
                Err(CaptureFault::Permission(denial))
            }
        }
    }

    fn open_gap(&mut self, cause: GapCause, elapsed_nanos: u64) -> Result<(), CaptureFault> {
        let at = self.clock.tick(elapsed_nanos)?;
        self.journal.append(
            at,
            RecordBody::Gap {
                cause,
                resumed_domain: None,
            },
        )?;
        self.stopped = Some(cause);
        Ok(())
    }
}

/// What a sealed capture leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCapture {
    recovery: JournalRecovery,
    marks: MarkLedger,
    mapping: MappingLedger,
    signals: Vec<FailureSignal>,
    retention: RetentionTerms,
}

impl SealedCapture {
    /// What the journal file holds, read back off disk.
    #[must_use]
    pub const fn journal(&self) -> &JournalRecovery {
        &self.recovery
    }

    /// Every mark and label.
    #[must_use]
    pub const fn marks(&self) -> &MarkLedger {
        &self.marks
    }

    /// Every mapping version, oldest first.
    #[must_use]
    pub const fn mapping(&self) -> &MappingLedger {
        &self.mapping
    }

    /// Every failure signal raised.
    #[must_use]
    pub fn signals(&self) -> &[FailureSignal] {
        &self.signals
    }

    /// The two retention bounds the grant attached, carried on so the deletion
    /// the grant asked for reaches this capture without a second lookup.
    #[must_use]
    pub const fn retention(&self) -> RetentionTerms {
        self.retention
    }

    /// Every gap in the timeline, in file order.
    #[must_use]
    pub fn gaps(&self) -> Vec<(u32, GapCause)> {
        self.recovery
            .records()
            .iter()
            .filter_map(|record| match record.body() {
                RecordBody::Gap { cause, .. } => Some((record.seq(), *cause)),
                _ => None,
            })
            .collect()
    }
}
