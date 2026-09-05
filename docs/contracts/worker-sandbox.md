# Worker sandbox contract

`academic-worker` is the `P2-G4` containment boundary. It runs a pipeline job or
a provider SDK call in a separate process, under an operating-system sandbox,
against two staged directories, with four bounds and a receipt.

It sits below `P2-G7`'s process classes rather than beside them. A worker holds
no `ProcessClass` and no cell of that matrix: it is the unprivileged compute box
those processes hand work to, and the six process entry points are unchanged.

## What is measured, and on which platform

Every row below was produced by launching a process, attempting the operation
inside it, and reading the operating system's answer. None of it is a source
scan. The error numbers are what the probe reported on the two platforms this
task ran on.

| Claim | Linux — seccomp, Landlock, rlimits | Windows — AppContainer, job object |
|---|---|---|
| home read | `EACCES` (13): the path is under no Landlock rule | `ERROR_ACCESS_DENIED` (5): no ACE for the container SID |
| vault read | `EACCES` (13), same rule | `ERROR_ACCESS_DENIED` (5), same reason |
| write outside the staged output | `EACCES` (13) | `ERROR_ACCESS_DENIED` (5) |
| child process | `EPERM` (1) at `clone`/`fork`/`vfork`/`execve` | `ERROR_NOT_ENOUGH_QUOTA` (1816), `ActiveProcessLimit = 1` |
| socket | `EPERM` (1) at `socket(2)`; no handle is created | the handle **is** created; every off-host connect is `WSAEACCES` (10013) |
| the same on the x32 ABI | `EPERM` (1); the filter gates the ABI, not a second number table | not applicable: the Windows backend is not a syscall filter |
| CPU bound | `RLIMIT_CPU`, `SIGXCPU`, job killed | `PerProcessUserTimeLimit`, job killed |
| memory bound | `RLIMIT_AS`; the allocation is refused | `ProcessMemoryLimit`; the allocation is refused |
| wall bound | parent deadline, `SIGKILL` | parent deadline, `TerminateJobObject` |
| output bound | `RLIMIT_FSIZE`, plus the parent's own measurement | the parent's measurement |

Two rows are asymmetric and are written that way on purpose.

**The socket row.** On Linux the unqualified reading of
`worker_cannot_open_a_socket` is true: the filter refuses `socket(2)`, so the
loopback listener the probe builds as its control is never created either. On
Windows it is not. `\Device\Afd` grants `ALL APPLICATION PACKAGES` and no
user-mode mechanism removes that without a filter driver or an administrator, so
the handle exists; the platform's loopback exemption then lets an AppContainer
connect to an endpoint it owns itself, which the probe's own listener is. What
Windows refuses is every address off the host, with a permission code no routing
failure produces. The acceptance test asserts exactly that per platform, and
fails if the Windows backend ever starts refusing loopback as well, so the day
that changes this page changes with it.

**The x32 row.** `seccomp_data` carries an `arch` token and a syscall number, and
on x86 the token does not identify the ABI: `AUDIT_ARCH_X86_64` is the token for
the 64-bit ABI **and** for x32, which the kernel tells apart by
`__X32_SYSCALL_BIT` — bit 30 of the number — and by nothing else. This filter
checked the token and then compared native numbers, so every x32 number fell past
the comparisons to `SECCOMP_RET_ALLOW`. `P2-A5` measured the identical gap in
`academic-process-sandbox`'s filter, which is this one instruction for
instruction: a process reported `Seccomp: 2` and completed a TCP handshake.

The exposure here is wider than a socket, because six of the numbers this filter
refuses have their own x32 entry points and are therefore *not* the native number
with the bit set: `execve` 520, `execveat` 545, `ptrace` 521, `recvfrom` 517,
`sendmsg` 518 and `recvmsg` 519. A repair that added x32 spellings to the deny
list by setting the bit on each native number would have left all six reachable —
including the two that create a process. So the refusal is of the ABI: one
unsigned floor after the arch check, since every x32 number is at or above the
bit and every native number is far below it. There is no table to get wrong.

`enter` then asks the kernel. After the filter installs, it makes one x32 `getpid`
— a number the deny list does **not** carry, so the answer separates a filter
that refuses the x32 ABI from one that merely carries x32 spellings — and refuses
to return a backend unless the answer is `EPERM`. That is the filter's own answer
rather than the `ENOSYS` a kernel built without x32 would give. Every containment
test is therefore also this test: a filter that stopped covering x32 does not let
a contained run start.

aarch64 needs no floor: its 32-bit compat ABI carries `AUDIT_ARCH_ARM`, a
different token the arch check already refuses. `socketcall`, the multiplexed
socket entry point, does not exist on this target — `libc::SYS_socketcall` is
absent on `x86_64-unknown-linux-gnu` and the build fails on it.

**The output row.** Two of the four bounds kill the job and two refuse the
operation. `RLIMIT_FSIZE` bounds one file rather than a directory, and a job
object bounds no file at all, so the directory total is measured by the parent
after the run and turned into `KilledByLimit(OutputBytes)` there. The job is not
killed for it; what the bound buys is that the bytes are never accepted.

**The wall row measures on a different clock than it decides on, on Windows
only.** Linux compares `started.elapsed()` to the deadline and then records
`started.elapsed()` again, so a receipt's `wall_millis` cannot be below the
bound it hit. Windows waits with `WaitForSingleObject`, whose timeout is counted
in system timer ticks, and records `Instant`, which is the performance counter;
the wait can return before the counter reaches the bound. A Windows receipt's
`wall_millis` is therefore at the bound **within one system timer tick**
(15.625 ms at the default resolution), not at or past it, and
`cpu_memory_time_output_limits_are_enforced` asserts each platform's own claim
rather than one sentence for both. The outcome is unaffected: the kill happened,
and `KilledByLimit(WallTime)` is what the wait returned.

### Every refusal is paired with a permission

A refusal on its own is not evidence. A connect to an address nothing routes
fails with or without a sandbox, and a file that does not exist is unreadable to
everybody. So each containment test runs the same probe binary twice — once
through its `baseline` mode with no sandbox, once inside one — and requires the
uncontained run to have been *permitted* what the contained run is refused. The
socket control is a loopback listener plus a connect to the port it just chose:
a complete TCP round trip that needs no service and no network, which any
uncontained process completes.

The error number is checked too, not just the fact of a refusal. That is not
pedantry: with the probe redirecting its streams to the null device,
`worker_cannot_spawn_a_child` passed on Linux with `EACCES`, because Landlock
refused the `/dev/null` open before `clone` was ever reached. The test was green
and the claim was not measured. Inheriting the streams instead opens nothing,
and the answer moved to `EPERM` from the filter.

## The capability descriptor

One job, one descriptor, one use.

A descriptor names the job, the capabilities it holds, its two staged
directories, its four bounds, and the instant after which it is worthless. It
carries no secret and is written into the staged input directory in plain text,
because what makes it unforgeable is the registry rather than its contents:
`DescriptorRegistry::consume` compares the presented descriptor's SHA-256 against
the issued one, so a re-encoded descriptor with a later expiry is a mismatch and
not a fresh grant.

Refusals are ordered. An expired descriptor is refused as expired whether or not
it was also consumed, so a replay after expiry cannot be reported as a fresh
descriptor that merely ran twice.

`JobCapability` has two variants: read the staged input, write the staged
output. There is no variant for creating a claim, opening a socket, or reading
key material, and `worker_cannot_publish_a_canonical_claim` reads the enum
through a compiler-checked witness `match`, so a third variant stops that suite
compiling rather than widening what a job may do.

## Staged input, staged output, and who may accept

Three directories cross the boundary, and the sandbox grants exactly three:

- **staged input**, read-only — the descriptor and the job script;
- **staged output**, writable — where a result goes;
- **report**, writable — the control channel the parent reads the run's
  per-operation answers from. It is not counted against the output bound,
  because it is not the result.

A worker writes bytes. Turning bytes into something the rest of the system will
read is a separate act in a different process. `AcceptedOutput` has private
fields and one producer, `StagingAuthority::accept`; a `StagingAuthority` holds a
secret the parent generates and never writes into a descriptor or either staged
directory, so the sandboxed process has nothing to construct one from. A
`compile_fail` case closes the other half: an `AcceptedOutput` cannot be
assembled field by field from outside the crate either.

`accept` refuses six ways, and `pj02_output_that_fails_validation_is_quarantined_not_accepted`
walks all six and then accepts a clean one, so the boundary is not simply closed:
a path that escapes the staged output, a staged file that is not there, a run
that did not complete, a run past its output bound, bytes whose job is not the
descriptor's, and a descriptor that never held the write capability.

## The receipt

`WorkerRun` is a `ModelRunId` and a `ResourceReceipt` in one value with one
constructor that takes both, no `Default`, and no setter. That is the whole
mechanism for "a resource receipt is attached to every `ModelRun`": there is no
order of calls that produces the identity without the measurement, and the
`compile_fail` case refuses assembling one field by field.

`P2-M1` owns the twelve section 27.3 `ModelRun` fields and the event arm that
records them. This task adds no thirteenth field and edits no signed envelope;
what it owns is that nothing leaves this crate's seam without a receipt.

A receipt records the backend, the four bounds, the CPU milliseconds and peak
memory the operating system attributed to the run, the wall time, the staged
output byte count, and how the run ended. A killed run has one too — `PJ01`'s row
says a killed job records a receipt, and the receipt is the return value rather
than something a caller may skip.

## `PJ01` and `PJ02`

`PJ01` — a worker over its CPU, memory, time, or output bound — is this task's
in full: the job is killed or the operation refused, `RunOutcome::is_acceptable`
is false for every outcome but `Completed`, and no staged byte reaches the
acceptance boundary.

`PJ02` is split, and the execution plan is inconsistent about it. Its section 7
fault table assigns `PJ02` to `P2-G5`; its section 5 `P2-G4` row lists `PJ01` and
`PJ02` as this task's. The reconciliation is that `P2-G5` owns schema validation
and span provenance, which are about what the bytes say, and this task owns the
acceptance boundary those bytes cross first. The six refusals above are that
half. Nothing here validates a schema.

## The malicious-plugin corpus

The corpus is a set of *scripts*, not binaries. A job is a list of
`JobOperation`s, the corpus is composed at run time from that enum, and
`JobOperation::must_be_refused` says which entries are adversarial — so an
operation added later has to be classified before the test says anything about
it. The corpus is synthetic by construction and cannot drift from the code.

`malicious_plugin_corpus_is_contained` runs all nine entries in one job and
requires the seven adversarial ones to be refused *and* the two legitimate ones
to be permitted, so a sandbox tight enough to refuse everything fails it as
loudly as one that permits everything. Both canaries are re-read afterwards and
compared to their original bytes.

## Where the socket spellings are, and why

Two files in this crate spell an outbound socket construct, and they are the
first in this repository allowed to.

`crates/worker/probes/worker_probe.rs` is the process being contained. A test
that proves the operating system refuses a socket has to ask for one. It is a
`[[bin]]` with `required-features = ["native-sandbox"]` and a `path` outside
`src`, no workspace crate depends on `academic-worker`, and
`only_egress_crate_has_a_socket` reads both facts out of `cargo metadata` rather
than taking them on trust.

`crates/worker/src/sandbox/linux.rs` names the socket *syscalls*, and names them
to put them in a seccomp deny list. That is structural rather than a promise, in
two halves the same scan checks.

Every `SYS_` spelling in that file must appear inside its `denied_syscalls`
function, **counted** rather than merely present, so a spelling that is in the
deny list and also somewhere else fails. The exception is the five syscalls the
file *makes* -- `SYS_landlock_create_ruleset`, `SYS_landlock_add_rule`,
`SYS_landlock_restrict_self`, `SYS_seccomp`, and the `SYS_getpid` of the x32
control above -- which are enumerated in the scan with that reason. Until
`P2-RF10` the counted rule read only the ten socket names on the file's
allowance, so a non-socket `SYS_` name outside `denied_syscalls` passed; `T146`
observed that with `libc::SYS_memfd_create`.

And every `libc::syscall(` call in that file must name a `libc::SYS_` constant
from that five-name list as its first argument, optionally OR-ed with
`X32_SYSCALL_BIT`, whose definition the same rule pins as whole text. A number
fails. This is the half
that had been missing entirely: `libc::syscall` sits on the file's allowance, so
`libc::syscall(41, 2, 1, 0)` -- which opens an AF_INET stream socket -- changed no
allowance, spelled no listed pattern, passed every scan, and compiled clean under
`cargo clippy -p academic-worker --features native-sandbox -- -D warnings`.

That rule reads a *call* written as a path, so it holds only while a call has to
be written that way. `P2-RF11` added the half that makes that true: no file in
the workspace may import `libc::syscall` -- not `use libc::syscall;`, not
`use libc::syscall as raw;`, not a braced list naming it, not `use libc::*;`,
not `extern crate libc as raw;`, and not `use libc::{self as l};` -- so a call
to it spells `libc::syscall(` and reaches the first-argument rule. Beside that,
every mention of `libc::syscall` in this file must *be* a call, because taking
the function as a value moves its arguments out of the rule's sight. `T149`
reached the same socket by number through three of those imports and `P2-RF11`
reached it through the other two and through a function value; every one of them
compiled clean and passed every scan before its rule existed.

The sandbox does not cover that call, and it is worth saying why, because
`policy-source-scans.md` used to say it did. This file holds the parent-side
`launch` as well as the child-side `enter`; the parent installs the sandbox and
runs outside it. What bounds a raw syscall here are the three source rules
together, not the filter: the import ban and the rename ban make the call spell
the path, the every-mention-is-a-call rule keeps its arguments in view, and the
first-argument rule reads them.

Adding those two entries widened `only_egress_crate_has_a_socket`'s allowance.
Three things were tightened in the same commit and are described in
[policy source scans](policy-source-scans.md): the walk now reads every `.rs`
under a crate rather than three directory names, the lexer models raw strings,
and `libc::syscall` and the `SYS_*` socket spellings joined the pattern list.
`product_network` stays `NONE`.

## `unsafe`

This crate sets `unsafe_code = "deny"` rather than the workspace's `forbid`,
because a filter, a ruleset, a token and a job object are syscalls. Every
`unsafe` block carries `#[allow(unsafe_code)]` on its function, as the four
existing platform leaves do, and `unsafe_is_confined_to_the_sandbox_backends`
walks `src`, `probes` and `tests`, counts what it read, and compares the set of
files holding an `unsafe` item against exactly
`["src/sandbox/linux.rs", "src/sandbox/windows.rs"]` as a whole.

## What this contract does not claim

- **Nothing here is ADR-002 acceptance.** The default lane remains
  `storage_encryption=NONE`, `production_data_allowed=false`,
  `adr_002_accepted=false`, the acceptance public key is unprovisioned, and the
  committed candidate receipt carries two of five platform rows.
- **The default feature set installs no sandbox.** With `native-sandbox` off,
  every type in this crate is bookkeeping and the operating system permits the
  worker everything it permits any process. `BackendId::compiled()` reports
  `NONE` and `sandbox::launch` returns `SandboxError::Unavailable`. The
  containment claims belong to the feature, the platform, and the kernel version
  they were measured on.
- **The worker binary sandboxes itself on Linux.** The untrusted unit is the job
  payload, not the probe: `sandbox::enter` is called once, at the top of the
  contained path, before a single job byte is read, and
  `the_probe_enters_the_sandbox_before_it_reads_a_job` pins that whole function
  as text and checks the call site count is one. A worker binary that was itself
  hostile is outside this boundary; that is the process-image integrity question
  `P2-H1`'s signing work owns.
- **The Windows environment is inherited, not minimal.** A hand-built minimal
  block is refused `ERROR_ENVVAR_NOT_FOUND` (203) by `CreateProcessW` into an
  AppContainer — measured, with `SystemRoot`/`windir`/`SystemDrive` alone and
  again with `PATH`/`PATHEXT`/`COMSPEC` added. The environment is not part of the
  boundary: the container's rights come from its token and the ACEs granted to
  its SID.
- **WSL2 is where the Linux rows were measured**, on kernel
  `6.18.33.2-microsoft-standard-WSL2` with Landlock ABI 7. Section 8.4 of the
  execution plan says WSL may exercise Linux code paths and never substitutes for
  a Windows claim; the Windows rows above were measured on Windows natively, and
  the two sets are recorded separately for that reason.
- **No provider SDK is called.** `academic-egress-boundary` owns the staging and
  the transport trait, and no implementation of it ships. What this task
  contributes to a provider call is the process it would run in.
- **This is not the `P2-G7` process-class boundary.** `sandbox::enter` here
  contains a *job* inside a worker process: it takes a `CapabilityDescriptor`
  with two staged directories and resource limits, and it grants the contained
  process read-write on the staged output. A `ProcessClass` capability set is one
  layer up, has no directories, and includes classes that declare no write at
  all, so this launcher cannot serve it. That boundary is
  [process capability enforcement](process-capability-enforcement.md), and the
  rule that nothing may depend on `academic-worker` is what keeps the two apart.

## The two names this backend shares with the machine

Two of the names this boundary uses are not private to a process. Both are
written down here because a process that assumes it owns them produces failures
that look like the sandbox and are not.

**The AppContainer profile.** `CONTAINER_NAME` is one fixed name,
`academic-worker-p2-g4`, and `container_sid` asks for it on every
`availability()` and every `launch()`. One name is deliberate: a name per
process would leave a profile directory under `%LOCALAPPDATA%\Packages` behind
on every run. What one name costs is that every process on the machine is a
candidate concurrent creator of it, and two creators of an *absent* profile tear
each other's directory down. Measured: with eight test threads,
`%LOCALAPPDATA%\Packages\academic-worker-p2-g4` went absent and was recreated
every few seconds; with one thread it was unchanged for seventy seconds across
two full runs. Every `CreateProcessW` issued into the container while the
directory was absent failed with `ERROR_FILE_NOT_FOUND` (2), which is how this
suite came to fail about one run in ten with a launch error that named the probe
binary and had nothing to do with the probe binary.

So creation is serialised, twice: a process-wide `Mutex` because eight test
threads supply two creators without a second process being involved, and an
exclusive open — share mode zero — of a lock file beside the profile under
`%LOCALAPPDATA%`, because two lanes on one machine supply them as well. The lock
is a held handle rather than a marker file, so a process that dies releases it.
A caller that cannot take it within five seconds proceeds anyway: creation
happens once per machine and every later call answers `ERROR_ALREADY_EXISTS`
without touching the directory, so the lock removes a race and is no part of what
the backend refuses. It adds no `unsafe`. `academic-capture-gate`'s Windows
device layer carries the same fixed name, the same measured churn, and the same
repair.

**The home canary.** The acceptance suite writes a canary inside the real home
directory, because "the worker cannot read *this user's home*" is the claim; the
vault canary, which is about an arbitrary path, is the one under a temporary
root. A canary under the home directory is therefore a name in a directory
shared by every process of that user, and the harness removes it on `Drop`. Its
name carries the process id, the wall clock and a counter, and is reserved with
`create_dir` rather than `create_dir_all` so a name that already exists is
refused instead of joined. Before that, the name was `.academic-worker-g4-`
plus the test's label and nothing else: two lanes running this suite at once
wrote the same path, the first to finish deleted the other's canary, and the
survivor reported 1 passed and 7 failed — exactly the tests that build a
`Harness` — with `ERROR_PATH_NOT_FOUND` standing where the backend owed
`ERROR_ACCESS_DENIED`. `two_harnesses_with_one_label_do_not_share_a_canary` is
what holds the name apart now.
