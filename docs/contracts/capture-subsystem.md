# Capture subsystem

`academic-capture` is the `P2-L2` surface: the desktop host's one-action
Record/Capture/Mark, the single monotonic session clock both audio and image
capture derive every instant from, the append-only chunk journal underneath
them, the preflight that stops a capture before it loses one, and the Mark
Moment whose instant no later label can move.

It reads no clock and opens no device. Every elapsed reading and every
permission instant arrives as an argument, which is why the acceptance rows can
name the instants they assert against, and there is no code path in it that
could open a microphone, a camera or a screen.

## Which device is an authorized recorder is still open

Section 12 lists it as an open product question and **`P2-L2` does not answer
it. Phase 2 ships the desktop host only.** This task opens no section 38 gate
and closes none. `GATE-38-009` and `GATE-38-019` stay exactly where `P2-L1` left
them: a per-offering, per-term evidence-backed permission, unfilled, which keeps
the recorder inactive because `mint_capture_capability` refuses and there is no
recorder without one.

## What is not here

**No recording happens anywhere in this repository.** Every chunk and every
image in every fixture is a committed literal. This crate holds no device
handle, makes no syscall, and contains no `unsafe`.
`academic-capture-gate` is where a device would be opened, and
[its contract](capture-device-gate.md) says what each platform there actually
refuses.

**No vault object is written.** The journal frames are plaintext on disk. The
profile's `storage_encryption` is `NONE`, `production_data_allowed` is `false`,
and ADR-002 is unaccepted, so nothing this crate writes today is a user's
recording — but sealing the journal under `AEAD_CHUNKED_V2` is not done and is
open item `C-8` below. The plan lists `P2-K3` as a dependency of `P2-L2`; what
that dependency is, as built, is *sequencing* — the encrypted object format
exists before a capture surface does — and not an edge. None of `P2-L2`'s four
fixed contracts is an object format.

**No dependency edge to `academic-capture-gate`, in either direction.** That
package carries a platform backend and a probe binary, and
`only_egress_crate_has_a_socket` reads `cargo metadata` and fails the day any
workspace crate depends on it, because the probe would then be reachable from a
default build. `P2-G2`'s precedent is to split rather than to weaken a guard, so
the two crates are siblings over `academic-consent`'s one section 3.7 binding.
What that costs is `C-9`.

**No store, no socket, no migration.** The journal is one local file.
`STORE_MIGRATION_SQL` still pins seven migrations; this task adds none, and the
`0008` gap `P2-M2` left is still empty.

## One action

`academic_capture::begin` is the whole of starting a capture and does five
things: it evaluates the section 3.7 permission through
`mint_capture_capability`, selects the effective policy row, runs preflight,
starts the one session clock, and creates the journal. `CaptureRecorder` has no
public constructor, so holding one is proof that all five happened.

A refusal at any of them leaves **nothing on disk**.
`capture_one_action_authorization` observes the journal file's absence after a
written refusal, after a preflight below the floor, and after an instant no
policy row reaches — the last of which is refused rather than defaulted,
because a default would be a claim about a period no decision covers.

This surface adds no section 3.7 comparison of its own. `begin` calls
`mint_capture_capability` once and every recording seam re-runs the same binding
through `continue_capture`, for the reason `academic-capture-gate` gives: a
token minted at one instant says nothing about a later one, and only the binding
sees an expired grant, a scope interval that ended, and a superseding record.

## The session clock

### Domain

`SessionClockDomain` is an opaque digest over the lecture identifier, the
capability token identifier, a per-process clock ordinal, and — for a resumed
session — the tail digest of the journal it continues. There is no constructor
that takes a digest.

`SessionTick` is the only instant type in the crate and has **no public
constructor**. Its three fields are private, so only `clock.rs` can build one,
and there are exactly two sites: `SessionClock::tick`, which mints, and a
crate-private `SessionTick::recorded`, which reads one back out of a journal
frame under the domain that frame's file names.

**"Shared" is the contract, so it is a type rather than an assertion.** A suite
that read an audio instant and an image instant and compared them would pass
whether there was one clock or two that agreed. Here `CaptureRecorder` holds one
`SessionClock`, every recording seam takes its instant from it, and a seam that
accepts an instant from outside — a realignment anchor — admits it through that
same clock and refuses a foreign domain.
`shared_session_clock_for_audio_and_capture` builds a second clock over the same
lecture and the same token, observes that its domain differs, and observes both
a wholly foreign anchor pair and a mixed pair refused.

**What the ordinal separates, and what it does not.** Two clocks started in one
process differ by it. A clock that continues a journal another process wrote
differs by that journal's tail digest. Two independent processes that each start
a *first* clock for the same lecture under the same token derive the same
domain; that is `C-10`, and nothing here needs to tell those apart because they
write different journals and a journal's header names its own.

### Monotonicity

`SessionClock::tick` accepts a reading equal to or above the last one it
accepted and **refuses one below it**. Equal readings get their own sequence
number, because two events can share a nanosecond and still need an order.

The refusal is the whole of the guarantee, and it is written to that width: this
crate reads no clock, so it cannot promise the host's source is monotonic. What
it promises is that no tick it minted is below one it already minted, so no
frame in a journal carries an instant earlier than the frame before it. A
reading that went backwards is refused rather than clamped — clamping would put
two different real instants on one tick, which is the silent re-timestamping
section 34.1 forbids one row above.

`no_wall_clock_reaches_the_session_clock` is the source half: `SystemTime`,
`Instant`, `UNIX_EPOCH`, `std::time`, `chrono`, `Uuid::now_v7` and a bare
`.elapsed()` are all refused anywhere in the crate's product source, and each of
those four shapes is run through the check inside the test.

### Drift estimation

Two `Anchor`s — a session instant the user says lines up with a known instant on
a reference timeline — produce a `DriftEstimate`: the offset the first anchor
fixes, how far it moved by the second, and the confidence that magnitude
produces against the effective tolerance.

A drift **greater than** the tolerance is `AlignmentConfidence::Low` carrying
the ± range; a drift exactly at it is `Normal`.
`drift_beyond_tolerance_is_alignment_low_confidence` tests both sides with the
same anchors moved by one nanosecond. Past the tolerance is **low confidence,
not a refusal and not silence**: the estimate is still an estimate, the offset it
carries is unchanged, and the badge is the section 34.1 spelling
`ALIGNMENT_LOW_CONFIDENCE`.

## The chunk journal record

One append-only file. All integers are big-endian.

| Region | Bytes | Field |
|---|---|---|
| header | 8 | `ACJRNL01` |
| header | 32 | session clock domain digest |
| header | 32 | policy row digest |
| header | 32 | capability token identifier |
| frame | 4 | frame sequence, from zero |
| frame | 1 | body kind |
| frame | 4 | tick sequence |
| frame | 8 | elapsed nanoseconds |
| frame | 4 | body length |
| frame | 32 | previous frame's digest (zeros for the first) |
| frame | *n* | body |
| frame | 32 | SHA-256 over this frame's 53 header bytes and its body |

`t001`'s `REQ-12-017` row asks for a "contiguous local chunk timeline/hashes and
later resumable processing". The contiguity and the hashes are the same
structure: every frame names the one before it, so a missing frame is a broken
chain rather than a gap somebody has to notice.

Seven body kinds, each a closed vocabulary read out of its own enum by
`every_closed_vocabulary_is_the_list_its_enum_declares`, with frame bytes
required to be distinct and non-zero:

| Kind | Body |
|---|---|
| `AUDIO_CHUNK` | the bytes |
| `IMAGE_CAPTURE` | orientation, audio-clock offset, then the original bytes |
| `MARK` | the mark's sequence number, and nothing else |
| `MARK_LABEL` | the mark's sequence number and the label |
| `FAILURE_SIGNAL` | the failure, its delivery, and the reading's own instant |
| `GAP` | the cause, and the new clock's domain for a resume |
| `MAPPING_VERSION` | the version number, both anchors, the offset, the drift and the ± range |

**Append-only, and what recovery removes.** `ChunkJournal::append` is the one
public `&mut self` method and it only extends. Recovery removes exactly one
thing: a trailing partial frame — bytes no frame digest ever covered, left by a
process that died between `write` and `sync`. `ChunkJournal::reopen` is the only
place in the crate that shortens a file and it is pinned whole beside `append`.

**What the chain is and is not.** It detects truncation, reordering and
corruption: `mark_now_label_later` flips one bit of a recorded instant on disk
and the replay refuses the frame rather than reading back a new time. It is not
a signature. A writer who can already edit files inside the profile can rebuild
the chain, and nothing here claims otherwise.

## Capture metadata

`REQ-12-015` asks a capture to store the original image, its orientation, its
timestamp and its audio-clock offset. All four are separate fields and the bytes
are never touched.

- **Original bytes.** No function in this crate rotates, re-encodes, strips or
  re-compresses a capture. `capture_metadata_integrity` compares the stored
  digest with the digest of what the caller handed in.
- **Orientation.** One of the eight EXIF values, stated by the caller and stored
  beside the bytes rather than read out of them. The fixture carries no EXIF
  block at all, which is what "EXIF-independent" means: a capture whose bytes
  hold no orientation still has an exact one.
- **Timestamp.** The frame's `SessionTick`.
- **Audio-clock offset.** The distance from the session's audio epoch — the
  first audio chunk's tick, or the session origin while there is none — measured
  between two ticks on **one** clock.

In a two-clock design that offset is an estimate the image device makes against
the audio device. Here it is a subtraction inside one domain, so
`image.at().elapsed_nanos()` equals
`audio_epoch().elapsed_nanos() + audio_clock_offset_nanos` **exactly**, and
`capture_metadata_integrity` asserts that identity for all eight orientations.
It is the identity a second clock for the image path would break.

**The bytes are secret-bearing.** A lecture recording and a photograph of a
board are the user's private content, so `CaptureBytes` hand-writes a redacting
`Debug` that reaches the buffer only through a length, and it is registered in
`tools/secret-debug-policy.test.mjs`'s `SECRET_BEARING_TYPES`. This is the
decision `S-10` leaves to each new crate that declares a byte field from that
vocabulary, and it is made here in the strengthening direction: a new
`PUBLIC_BYTES` exemption would have been the other one.

## Mark now, label later

`Mark` carries a sequence number and one instant. It has **no label field and no
`&mut self` method at all**, so a value of it cannot change after it is built.
A label is a separate `MarkLabel` with its own instant and the mark's sequence
number; `LabelledMark::at` returns the mark's.

Labelling twice appends twice and the current label is the last one, which is
ADR-003's "corrections append a new assertion" rather than a second correction
mechanism invented here. The durable half is the same mechanism: both are frames
in the chain-digested journal, and the mark's frame is already digested when the
label arrives.

`a_label_has_no_path_that_moves_a_mark` holds four rules — the whole set of
`impl` blocks whose header names `Mark` against a one-entry list, no `&mut self`
on any of them, no signature in the crate that takes a `MarkLabel` and returns a
`SessionTick`, and **the same signature rule across every package in
`crates/`**, because the types are public and any crate could otherwise declare
the accessor this one does not.

## Preflight and non-intrusive failure

A `PreflightReading` — free storage, battery percentage, whether it is charging,
and whether the microphone is still held — is handed in at `begin` and again
whenever the host observes a change. Nothing here queries a device.

The battery row is not raised while charging: a capture on mains power at three
percent is not about to stop, and stopping it would be the intrusive failure the
row exists to avoid.

`SignalDelivery` has exactly two variants, `SILENT_BANNER` and `SILENT_HAPTIC`.
**There is no audible, modal or blocking form to select.** A boolean named
`intrusive` would be a flag somebody could flip; the whole variant set is
compared instead, so a third form fails whatever it is called, and every
spelling is required to begin `SILENT_`.

"즉시" is measured against the effective row's `notification_within_nanos`, and
the signal carries both the instant the reading was taken and the instant it
reached the timeline, so the latency is in the record rather than asserted by
the code that wrote it.

## The four thresholds are dated rows

The specification fixes none of them, and `t001` lists a threshold as an open
gate candidate under `REQ-12-017`, `REQ-12-018` and `REQ-34-021`. A number
spelled in an `if` cannot be superseded, cannot be dated against a capture that
predates it, and cannot say which decision it came from, so all four are fields
of a `CapturePolicyRow` selected by the capture's own instant — the
`repeat_ceiling_effective_date` shape from `P2-U4`.

| Field | Shipped value | Row |
|---|---|---|
| `drift_tolerance_nanos` | 2 s | `capture.thresholds.2026_first` |
| `storage_floor_bytes` | 64 MiB | same |
| `battery_floor_percent` | 5 | same |
| `notification_within_nanos` | 2 s | same |

**Those four numbers are this repository's decision, not the specification's.**
An instant no row reaches resolves to `None` and is reported as a refusal to
begin — never as "no threshold applies".

`the_thresholds_are_versioned_rows_and_not_constants` refuses each of the four
names anywhere outside `policy.rs` except as a row's accessor, so a constant, a
field read or a shadowing local all fail; and
`drift_beyond_tolerance_is_alignment_low_confidence` moves the effective date
and observes the verdict move, so the dating is exercised and not only declared.

## A realignment appends

`MappingLedger` has one mutating operation and it pushes. There is no `&mut`
accessor into an existing `MappingVersion`, no removal and no replacement.
`manual_two_anchor_realignment_appends_a_mapping_version` compares version one
before and after appending version two, checks both reached the journal in
order, and checks each cites the policy row whose tolerance decided its
confidence. Two anchors at the same session instant measure nothing over no
interval and are refused rather than divided.

## The fault matrix rows

| ID | Injection | Observed |
|---|---|---|
| `CP02` | a reading with free storage below the effective floor | the capture stops, every frame already written is byte-identical on disk and the chain still verifies, and a `FAILURE_SIGNAL` frame is followed by a `GAP` naming `RESOURCE_FAILURE` |
| `CP03` | a reading whose microphone is `Lost` | the same three, with `MICROPHONE_LOST` |
| `CP04` | nine seconds of drift over a thirty-second interval | `ALIGNMENT_LOW_CONFIDENCE` with ±9 s, a mapping version appended, and **every earlier frame unchanged** — the correction is a frame beside them, not an edit of them |
| `CP05` | a real process abort at each of three failpoints | the journal recovers to the last synced chunk, the partial tail is dropped, and the resume opens a `GAP` naming `SESSION_RESUMED` and the new clock's domain |

`CP02`, `CP03` and `CP04` are error-induced: a reading and an anchor are values
the public seams already take, so each is a committed literal rather than a
failpoint. `CP05` is kill-induced and reuses `P2-K5`'s convention — a
`phase2-fault-injection` feature that compiles no environment lookup and no
crash switch into a product build.

Its three failpoints leave **distinguishable** states, so a child that aborted
early cannot pass as one that aborted late: nothing of the interrupted frame on
disk, its header and body but not its trailing digest, and the whole frame
durable. The kill child is the fault suite's own test binary re-invoked at a
named entry point, which is `academic-transcript`'s `IN04` arrangement, so this
package declares no binary target and the injection runs through the real
`begin` and `record_audio_chunk`.

`CP01` belongs to `P2-L1` and is restated at this surface by
`a_permission_that_stops_mid_lecture_stops_the_capture`: a superseding written
refusal and an expired grant each stop the capture at the recording seam, and
the frames already written stay whole.

## Open

| # | What is open | When it starts mattering |
|---|---|---|
| C-7 | Nothing writes migration `0006`'s rows. This is the consent contract's `C-2`, restated a second time because `P2-L1`'s `C-5` named "`P2-L2`'s chunk journal" as when it starts mattering. The journal does survive a restart — but the permission behind it does not: `begin` and `resume` both read an in-memory `ConsentLedger`. **This task did not close it either.** | A capture that has to re-bind after the daemon restarts, which is a resume that outlives the process that held the ledger. |
| C-8 | The journal frames are plaintext on disk. The chain detects truncation and corruption; it is not a signature and it is not encryption. Sealing them under `AEAD_CHUNKED_V2` would put `academic-crypto` and the `aead-objects` lane in this crate's graph, which is a dependency admission this task does not make. | The first capture of a real recording, which admission has not opened: `production_data_allowed` is `false`. |
| C-9 | The per-chunk re-binding is written twice — here and in `academic-capture-gate` — because no workspace crate may depend on that package. Both call `academic-consent`'s one binding and neither adds a comparison, so what is duplicated is the *call sequence* rather than the decision; but a change to the sequence has to be made in two places, and only each crate's own pins would notice. | A third capture surface, or a change to when a capture re-binds. Closing it means a fourth crate holding the sequence that both depend on. |
| C-11 (`academic-capture-gate`'s) | The same class of defect, one step outside this crate: `CaptureSession::record_chunk` compares a chunk's instant against the section 3.7 binding and against nothing else, so a backwards reading appends a chunk earlier than the one before it and the artefact is still releasable. It was measured, not reasoned — the numbers are on [the capture device gate contract](capture-device-gate.md), where the row lives, because closing it edits that crate's pinned text. | A capture whose host clock steps back. `academic-capture` refuses the same reading at its own clock; the two crates share no code, for the reason `C-9` gives. |
| C-10 | Two independent processes that each start a *first* session clock for the same lecture under the same capability token derive the same `SessionClockDomain`. Within a process the ordinal separates them and across a resume the predecessor digest does; this is the case neither covers. | Two capture processes writing one journal, which no code here does — a journal is created by one `begin` and refuses to overwrite an existing file. |

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`, the acceptance public key is unprovisioned, and every
fixture in this crate's test tree is synthetic and built from committed
literals. No recording is made, no device is opened, no sample is read, and no
permission in this repository refers to a real offering.
