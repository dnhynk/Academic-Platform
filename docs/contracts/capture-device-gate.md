# Capture device gate

`academic-capture-gate` is the `P2-L1` boundary: the daemon-side evaluation that
turns a live section 3.7 permission into a `CaptureCapabilityToken` and a device
ruleset, the enforcement under it, and the `PERMISSION_VIOLATION_RISK` state a
capture that did not stay inside its permission ends in.

It opens no socket and reads no clock: every instant it compares arrives as an
argument, which is why the acceptance rows can name the instants they assert
against. It adds no comparison beside `academic-consent`'s — `bind_permission`
is where every section 3.7 comparison happens and `P2-RF10` is why there is
exactly one of it.

## What is not here

**The `capture_permission` row is `PermissionRecord`.** The
[consent contract](consent-and-capture-permission.md) leaves `C-2` open:
nothing writes migration `0006`'s `capture_permission_terms`. **This task did
not close it.** The evaluation below reads the aggregate out of a
`ConsentLedger` held in memory, so `0006` is still checked as a schema and not
as a round trip, and the durable form is still asserted structurally.

**No workspace crate depends on this one.** It carries a platform backend and a
probe binary, and a product crate that linked it would put both in a default
build's dependency graph. That is `academic-worker`'s arrangement and it is
checked the same way, from `cargo metadata` rather than on trust. The
`academic-capture-client` process crate is unchanged and still holds exactly its
one `P2-G7` process-class binding.

**No recording happens anywhere in this repository.** Every chunk in every
fixture is a committed literal. The `native-capture` probe opens an
operating-system device handle and drops it: it starts no stream and reads no
sample, `the_probe_opens_a_handle_and_reads_no_sample` pins the whole of the
function that opens one, and five read shapes are forbidden in that file.

## A ruleset cannot exist without a token

`DeviceRuleset::for_token` is the only constructor. There is no `Default`, no
`new`, and no way to name a device class and get a ruleset holding it, so a
value of this type is proof that `mint_capture_capability` returned — which is
proof that `bind_permission` ran. Section 3.7's "the recorder holds no
microphone capability by default" is that absence rather than a check somebody
has to remember to write.

The media it reads are the token's *bound* media — the ones the binding compared
against the grant — not the request's, so a request field the binding refused
cannot reach a rule.

`CaptureSession` has no public constructor either, and
`tests/compile_fail/capture_session_has_no_public_constructor.rs` is the program
that shows it, compared against its committed diagnostic.

## Allowed media is enforced at the device layer

`DeviceClass::of` maps section 3.7's four media onto the three device kinds an
operating system hands out:

| Medium | Device class |
|---|---|
| `AUDIO` | `MICROPHONE` |
| `PHOTO_OF_BOARD` | `CAMERA` |
| `VIDEO` | `CAMERA` |
| `SCREEN_CAPTURE` | `SCREEN` |

`CaptureMedium` is `#[non_exhaustive]` and belongs to another crate, so the map
cannot be exhaustive at the compiler. Its wildcard is `None` rather than a
device: a medium this crate has not classified opens nothing, which is the
fail-closed direction. `every_capture_medium_is_classified` reads
`CaptureMedium`'s variants out of `crates/consent/src/permission.rs` and fails
the day a fifth is declared without a row here, and it refuses a wildcard that
is not `None`.

A grant listing `AUDIO` therefore derives a ruleset holding only `MICROPHONE`,
and `open_device` refuses `CAMERA` at the layer that would have opened one. With
the `native-capture` feature the same ruleset is what the platform backend
installs — on Linux the kernel enforces the split, and on Windows it does not.
The two rows below say which.

## What each platform actually refuses

Every row was produced by launching a process, attempting the open inside it,
and reading the operating system's answer. None of it is a source scan. Each
contained run is paired with an uncontained run of the same binary against the
same paths, and a row whose *baseline* is refused is not evidence: a path that
does not exist is unopenable to everybody.

| Claim | Linux — Landlock ruleset | Windows — AppContainer |
|---|---|---|
| a process holding no token opens a device | `EACCES` (13): the path is under no rule | `ERROR_ACCESS_DENIED` (5): the device object's DACL grants no AppContainer |
| the same open, uncontained | permitted | permitted |
| a token naming the class opens it | permitted: the rule is added for that tree | **still `ERROR_ACCESS_DENIED` (5)** |
| a token not naming the class | `EACCES` (13) | `ERROR_ACCESS_DENIED` (5) |
| where the restriction is applied | the child, between `fork` and `exec` | the parent, by `CreateProcessW` |

**The Windows row cannot widen, and that is the honest half.** A kernel
streaming capture filter's security descriptor is the driver's, not the
caller's. On the host this was measured on it is
`D:P(A;;FA;;;SY)(A;;0x1201bf;;;BA)(A;;0x1201bf;;;WD)(A;;0x1201bf;;;RC)` — four
entries and no `ALL APPLICATION PACKAGES`, so an AppContainer's access check
fails. Adding the container SID would need `WRITE_DAC`, which the driver grants
to `SYSTEM` and administrators. So the Windows container refuses **every** class
including the granted one, and the media split there is this crate's own
comparison rather than the kernel's.

That is the exact inverse of `academic-worker`'s socket row, where `\Device\Afd`
*does* grant `ALL APPLICATION PACKAGES` and the handle is therefore created
inside the container. Same mechanism, opposite answer, because the two device
objects are ACLed differently — and both are written down as measured rather
than as expected.

**The Linux ruleset grants three things that are not devices**, and each is
there because without it the run measures something else: the program image, or
`execve` is refused before the capture binary starts; the directories a
dynamically linked program's loader and libraries live in, for the same reason;
and the report directory, or the run cannot say what the kernel answered. `/dev`
is under none of them. There is no rule for the home directory, the vault, or
the working directory.

**The paths each row was measured on are the host's, not the repository's.**
`native::device_paths` enumerates them — the configuration manager's present
device interfaces on Windows, the conventional device tree roots that exist on
Linux — and `the_measured_device_nodes_are_reported` prints what the baseline
got for each.

| Host | Class | Path | Baseline |
|---|---|---|---|
| Windows 11 26200 | microphone | `\\?\USB#VID_0DB0&PID_7696&MI_00#…#{65e8773d-…}\WaveIn2` and four more kernel-streaming capture filters | `OPENED` |
| Windows 11 26200 | camera | none — `KSCATEGORY_VIDEO_CAMERA` is empty on this host | `NOT_RUN` |
| WSL2 `6.18.33.2-microsoft-standard-WSL2` | microphone | `/dev/snd` | `OPENED` |
| WSL2 | camera | none — no `/dev/video*` exists, so `/dev/null` stands in and the row says so | stand-in |

`/dev/snd/timer` is the only node under `/dev/snd` on that host and it is
`EACCES` to a user outside the `audio` group with or without a ruleset, so no
row is measured on it. The camera row is measured on a real character device
node that is not a camera; what it establishes is that the ruleset refuses a
device node by path and admits one the token names, not that a camera was
present. A host that has one produces a different row rather than a silent skip.

**A host that installs no device layer records `NOT_RUN` with the reason**, per
section 8.4 of the execution plan, and is never coerced to a pass. A hosted CI
runner has no capture device, so the Windows device rows are `NOT_RUN` there and
`PASS` on the measuring host.

## Termination at the boundary

Four checks, and each asks a different question.

`open_device` asks whether this token opens this class, and the instant it is
asked at becomes the session's first accepted one. `record_chunk` asks two.

The first is order: `now` against `accepted_at`, the highest instant this
session has accepted, and a lower one is refused as `CHUNK_OUT_OF_ORDER`.
`accepted_at` starts at the instant the device opened, so the first chunk is
compared against something rather than exempted — a rule whose first case is
skipped is a rule with a hole in it. Equal instants are accepted and take their
own sequence number, for the reason `academic-capture`'s `SessionClock::tick`
accepts equal readings: two events can share a nanosecond and still need an
order.

The second is permission: whether section 3.7 still holds, by re-running the
whole binding through `continue_capture`, not by comparing the token's own
`not_after`, because a token minted at one instant says nothing about a later
one — the grant can expire, the scope interval can end, and a superseding record
can arrive, and only the binding sees all three.

**Order is compared first, and that ordering is load-bearing.** A binding
refusal opens a `TimelineGap` at `now`; a backwards `now` allowed through to it
would put the gap itself earlier than a chunk already recorded, which is the
same backwards timeline one layer over. Refusing first means no instant below
the mark reaches the chunk list or the gap. It does reach the **audit row**,
whose `recorded_at` is the instant that was offered — that is the evidence, and
it is the one place a backwards instant belongs. It is not a way past the
boundary either: a chunk the order check refuses is not recorded at all, so
nothing crosses, and `seal` re-binds whatever was recorded regardless.

**An ordering refusal is not a stop and not a quarantine.** What a backwards
reading says is that the caller's clock moved, not that the permission ended, so
no gap opens, the mark stays where it was, and a later chunk at or above it is
accepted. It is not `PERMISSION_VIOLATION_RISK` either: that state is section
34.1's *unpermitted recording*, and an out-of-order chunk is no evidence about
permission — a row spelling it would tell a reviewer an authority was involved
when none was. Quarantine is also a seal-time verdict, so reaching for it here
would mean letting the backwards instant into the manifest first and reporting
it afterwards. The defect is prevented rather than recorded.

A **binding** refusal stops the capture. The `TimelineGap` opens at that instant
and every later chunk is refused as `SESSION_ALREADY_STOPPED`, so a caller that
ignores the error does not resume across the boundary. The gap is open-ended:
the system knows when it stopped and does not know when the lecture ended, and
writing an end it inferred would be the silent re-timestamping section 34.1
forbids one row above.

`seal` asks the question none of the others can: whether every chunk that *was*
recorded re-binds at its own instant. That is what makes the binding check
falsifiable — delete the `continue_capture` call from `record_chunk` and chunks
keep being appended past the boundary, and the seal then finds the first one
that does not re-bind. The ordering check has no such second observer, because a
chunk it refuses is never recorded and there is nothing left to reconcile; what
stands in for one is `the_capture_gate_appends_a_chunk_from_one_place`, which
holds that `record_chunk` is the only path that appends a chunk at all. Injection `L-I1` is that observation, and it is caught by
two independent mechanisms rather than by the check it deleted.

Fault `CP01` is the clean case: a permission that expires mid-lecture stops the
capture at the boundary, the chunks recorded before it were recorded while the
permission was live, so they re-bind, and the artefact is **releasable** with an
explicit gap. Quarantine is the other case.

## `PERMISSION_VIOLATION_RISK` is a state, not a log line

Section 34.1's unpermitted-recording row asks for `PERMISSION_VIOLATION_RISK`
with sharing and AI processing blocked. A boolean beside the bytes would be a
flag every reader has to remember to consult. So the two outcomes are two types:
`ReleasableArtifact` has a byte accessor and `QuarantinedArtifact` has none.

There is nothing to remember. A caller holding a `QuarantinedArtifact` has no
method that yields a `&[u8]`, a `String` or a `&str`, so there is no
`SourceDocument` to hand `academic-egress-boundary` and no `IngestedDocument` to
hand a `PromptEnvelope`. Four things hold it:

* the compiler, for a caller who reads its bytes —
  `tests/compile_fail/quarantined_artifact_hands_out_no_bytes.rs` is that
  program, and its committed diagnostic is compared;
* the whole set of `impl` blocks whose header names `QuarantinedArtifact`,
  compared against a one-entry list, so a `Deref<Target = [u8]>`, an
  `AsRef<[u8]>` or any other trait that hands the bytes back fails as an extra
  key. An `impl` written in another crate is refused by the orphan rule instead;
* the whole set of signatures in this crate whose return type names `u8`,
  compared against a two-entry list;
* a rule over every `pub` signature in every package in `crates/`: none takes a
  `QuarantinedArtifact` and returns a type naming `u8`, `str` or `String`. That
  rule is workspace-wide because the type is public and any crate could
  otherwise declare the accessor this one does not. It is
  `P2-RF10`'s `no_public_signature_hands_out_ingested_text` applied to the other
  quarantine, and injection `L-I7` — a `String`-returning renderer, which the
  byte-set rule cannot see — is what it catches.

`violation_risk_blocks_share_and_ai_processing` is the behavioural half, and it
is not one-sided: it stages a **releasable** artefact through the real
`SourceDocument` and quotes it into a real `PromptEnvelope` first, so a gate
tight enough to refuse everything fails it as loudly as one that permits
everything.

`releasable_bytes` is the one function a caller holding the sum type calls. It
returns the bytes for one arm and an audited `ARTIFACT_QUARANTINED` refusal for
the other, so the reachable arm of that refusal is not a branch nobody can
produce.

### How an artefact becomes quarantined

`seal` re-binds every recorded chunk at its own instant. A chunk that does not
re-bind is one recorded outside the permission that was supposed to cover it,
and there are two ways to get one: a chunk past the boundary, which is the
injection above; and a written refusal recorded during the lecture, which makes
every chunk already recorded stop re-binding. The second is the reachable
product shape and it is what `violation_risk_blocks_share_and_ai_processing`
builds its quarantined artefact from.

## Every denial leaves exactly one row

`CaptureAudit::record_refusal` returns the refusal it was handed. That is
`academic-consent`'s `record_capture_denial` shape, taken deliberately: a
function that returns the value the caller is about to return leaves no early
exit that skips the row on its way out.

That is checked rather than asserted. The number of `CaptureRefusal`
constructions in the crate's product source equals the number of
`record_refusal` calls, so a path that builds a refusal and returns it without a
row makes the two unequal. Injection `L-I5` is that observation.

`capture_audit_records_every_denial` walks two vocabularies rather than counting
them:

| This layer's reason | Reached by |
|---|---|
| `PERMISSION_REFUSED` | the section 3.7 binding refusing; the row carries which comparison |
| `MEDIUM_NOT_ON_TOKEN` | asking for a class the token's media set does not name |
| `SESSION_ALREADY_STOPPED` | a chunk offered after the boundary |
| `ARTIFACT_QUARANTINED` | `releasable_bytes` on a quarantined artefact |
| `DEVICE_LAYER_UNAVAILABLE` | a backend that was asked to install and could not |
| `CHUNK_OUT_OF_ORDER` | a chunk whose instant is below the session's highest accepted one |

and, under the first of those, every arm of `CaptureDenialReason` with the
scenario that produces it. Eight of the nine are reachable.
`SCOPE_MISMATCH` is not, and the suite says so rather than leaving a case nobody
can produce: `ConsentLedger::permission_for` filters on
`PermissionScope::answers`, so a request naming another offering, term or
session finds no record at all and is refused as `PERMISSION_UNKNOWN`; and
`status_of` returns `EXPIRED` for the only other way in, an instant outside the
scope interval. Both readings fail closed, so this is a defect of reporting
precision rather than of safety, and it is `C-4` below.

A row carries identifiers, a digest, a length and a time. It carries no chunk,
no sample and no frame — there is no byte-carrying field on it to reach — for
the reason `audit_contains_no_raw_canary` gives in `academic-egress-boundary`.

## The five records, enumerated

`record_fail_closed` walks section 3.7's five and requires each to produce its
own outcome:

| Case | Outcome |
|---|---|
| `UNKNOWN` — no record answers the scope | no device; `PERMISSION_UNKNOWN` |
| `PROHIBITED` — a written authority refused | no device; `PERMISSION_PROHIBITED` |
| `EXPIRED` — the grant no longer covers this instant | no device; `PERMISSION_EXPIRED` |
| scope mismatch — the request names another offering, term or session | no device; `PERMISSION_UNKNOWN`, for the reason above |
| valid | a device opens, and no audit row is appended |

The count is not asserted beside the list. The index each case must sit at comes
from a `match` over the enum, so a case dropped from the array, duplicated in
it, or reordered fails. **That check was written wrong first**: the original
walked the array and then asserted every element of the array had been walked,
which is true of any array whatever it holds. Injection `L-I15b` — replacing one
case with a duplicate, so the length is unchanged and the file compiles — passed
it. The same shape was then found and closed in two more enumerations in this
task, `REFUSAL_REASONS` and `DEVICE_CLASSES`, which is `L-I16` and `L-I17`.

`INV-C-013` is the first row: a new offering has no record, and no path in this
crate turns that into a device.

## The default lane installs nothing

With `native-capture` off there is no device handle anywhere in this crate, no
syscall, no probe target, and `DeviceLayer::Bookkeeping` is what a session
records. Every type in it is bookkeeping, exactly as `academic-worker` reports
with `native-sandbox` off, and `the_default_lane_reports_bookkeeping` refuses a
default lane that claims otherwise. The containment claims belong to the
feature, the platform, and the kernel or Windows build they were measured on.

`DeviceLayer` is an argument rather than a query inside the session, so a caller
cannot reach a device by asking at a moment when the answer is convenient, and
so the default lane's honest state is a value a test can pass rather than a
condition a test cannot reach.

## `unsafe`, and the three syscalls

This crate sets `unsafe_code = "deny"` rather than the workspace's `forbid`,
because a Landlock ruleset and an AppContainer token are syscalls. Every
`unsafe` block carries `#[allow(unsafe_code)]` on its function, and
`unsafe_is_confined_to_the_device_backends` compares the whole set of files
holding an `unsafe` item against exactly
`["src/native/linux.rs", "src/native/windows.rs"]`.

The Linux backend spells `libc::syscall` and it is the third file in this
repository allowed to. It names three syscalls —
`landlock_create_ruleset`, `landlock_add_rule`, `landlock_restrict_self` — and
no socket syscall at all. Two rules read that, in
`only_egress_crate_has_a_socket` and again in this crate's own scan: every
mention of `libc::syscall` is a call, so its arguments stay in sight; and every
call's first argument is a `libc::SYS_` path from that three-name list, so a
number is refused. Injections `L-I9` (a bare `libc::syscall(319, …)`, which
spells no forbidden name) and `L-I10` (`SYS_memfd_create`) are those two
observations, and both compile clean under
`cargo clippy -p academic-capture-gate --features native-capture -- -D warnings`
before their rules exist.

`P2-G4` wrote the first-argument rule for one file and keyed it on that file's
name. This task is the second file that needs it, and a second allowance entry
with no rule behind it is exactly the hole
[policy source scans](policy-source-scans.md) is about, so the rule is now keyed
on `RAW_SYSCALL_FILES`: a file on the socket allowance for `libc::syscall` that
is not a key there fails, and a call whose first argument is not one of *that
file's own* reviewed names fails. The worker's file keeps its extra rule — every
other `SYS_` name must sit inside `denied_syscalls` — because that file also
builds a seccomp deny list and this one does not.

## Why the parent installs the ruleset

The child installs nothing and is handed nothing it could widen. On Linux the
Landlock ruleset goes in between `fork` and `exec`, from a `pre_exec` closure;
on Windows the AppContainer is applied by `CreateProcessW`. So there is no wire
form of a ruleset for a contained process to misparse in its own favour, and
`DeviceRuleset` keeps the one constructor that takes a token.

That has a cost the code pays explicitly: a `pre_exec` closure must make
syscalls and nothing else, because the child of a multi-threaded process can
deadlock on an allocator lock another thread held at `fork`. Every path is
therefore resolved to an `O_PATH` descriptor in the parent, with its access mask
decided there, and the closure iterates a list of descriptors and allocates
nothing.

The mask is per path because `landlock_add_rule` refuses `EINVAL` when a rule
over a regular file or a device node carries a directory-only right such as
`READ_DIR` or `MAKE_REG`. That was measured, not predicted: the first version
used one constant for every rule and every contained run failed to start.

## `GATE-38-009` and `GATE-38-019` stay open

Both are per-offering, per-term inputs and this task fills neither. What they
leave is stated in the [consent contract](consent-and-capture-permission.md);
what it means here is that an unfilled cell keeps the recorder inactive:

| Gate | What stands while it is empty |
|---|---|
| `GATE-38-009` | no record answers the scope, so `mint_capture_capability` refuses, so there is no token, so there is no `DeviceRuleset` and no `CaptureSession` |
| `GATE-38-019` | the grant's media set is empty, so every request naming a medium is refused with `MEDIUM_NOT_GRANTED` — and so is a request naming none, which is why asking for nothing is not a way past an unconfirmed offering |

There is no constant holding a "usual" device set, no `Default` on
`DeviceRuleset`, and no fallback that reads one offering's answer for another.

## Open

| # | What is open | When it starts mattering |
|---|---|---|
| C-4 | `CaptureDenialReason::ScopeMismatch` is unreachable through `bind_permission`, so a scope that does not answer is audited as `PERMISSION_UNKNOWN`. Both readings fail closed; what is lost is which comparison a reviewer sees in the row. Closing it means editing `bind_permission`, which is pinned as whole text by `P2-G6`'s scans, or removing the arm. | A review of capture audit rows that needs to tell "nobody answered" from "somebody answered for another term". `scope_mismatch_is_refused_as_unknown_and_the_scope_arm_is_unreachable` fails the day either becomes reachable, so this row cannot go stale silently. |
| C-5 | Nothing writes migration `0006`'s rows. This is `C-2` from the consent contract, restated because that page names `P2-L1` as when it starts mattering and this task did not close it: the evaluation reads an in-memory ledger. | **`P2-L2` has landed and did not close it either.** Its chunk journal does survive a daemon restart, but the permission behind it does not: `begin` and `resume` both read an in-memory `ConsentLedger`. The row moves on to a resume that outlives the process that held one, and is `C-7` on [the capture subsystem contract](capture-subsystem.md). |
| C-6 | The Windows backend cannot widen by class, so a Windows capture reaches its device through the unrestricted parent rather than through a container the token opened. The media split on Windows is this crate's comparison, not the kernel's. | A Windows capture surface. **`P2-L2` is not one**: it opens no device, holds no handle, and takes every byte it journals as an argument, so it decided no process shape. Closing this means handle inheritance from the parent, or a packaged application with device capability declarations, and the task that first opens a device on Windows is the one that has to choose. |

### Closed

| # | How it was closed |
|---|---|
| C-11 | **`record_chunk` compares the chunk's instant against the session's highest accepted one and refuses a lower one as `CHUNK_OUT_OF_ORDER`.** The mark starts at the instant `open_device` was called, so the first chunk is compared too. `out_of_order_chunk_is_refused` runs `P2-L2`'s exact measurement — `INSIDE + 100` then `INSIDE + 1`, which gave `Ok(())`, `is_quarantined() == false` and instants `[1500100, 1500001]` — and now observes the refusal, the chunk count unchanged at one, no gap opened, and a manifest that runs forwards. Its three boundaries are exercised: one tick below is refused, equal is accepted, one tick above is accepted. It is a refusal rather than a quarantine for the reason in *Termination at the boundary*, and `the_capture_gate_appends_a_chunk_from_one_place` is what keeps a second appender from existing beside the compared one. |

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`, the acceptance public key is unprovisioned, and every
fixture in this crate's test tree is synthetic and built from committed
literals. No recording is made, no sample is read, and no permission in this
repository refers to a real offering.
