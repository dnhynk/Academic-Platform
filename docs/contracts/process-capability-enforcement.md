# Process capability enforcement contract

`P2-G7` gives every product process a class and a capability set. `P2-RF21` is
what makes that set true of the running process, or stops the process.

## The contract

> A process runs only while the capabilities it holds are the capabilities its
> class declares. Where that cannot be enforced, the process refuses to start.

`academic_process_sandbox::enter` is the whole of it. A process-class binary
calls it at the top of `main`, before any work, and there are exactly two
outcomes:

* an `Enforcement`, which means the refusals were installed **and** re-observed
  from the kernel, and the process may continue; or
* an `EnforcementError`, in which case the process writes one line to standard
  error and exits non-zero, having written nothing to standard output.

There is no third outcome. A partial application is an error, and an
installation the kernel does not confirm is an error.

`Enforcement` has no public constructor other than `enter`, so a caller cannot
produce one without having entered.

## What was wrong, and what the fix is not

Before this, six process binaries were four lines: compute the class's
capability set, drop it. `P2-A5` measured a `REPOSITORY_ANALYZER` process
declaring `OpenOutboundSocket = false` and `WriteStagedArtifact = false`, and
connecting to `1.1.1.1:53`, binding a listener, resolving a name, creating a
file and opening a source file for append — on both native hosts, with Linux
reporting `Seccomp: 0`. `P2-A4` measured the same shape on the capture and
egress binaries.

The defect was not "there is no containment". It was that the declaration and
the process disagreed while the process ran. Refusing to start closes it as
completely as confining does, and on Windows refusing is the only honest option.

## The mechanism is per platform; the contract is not

### Linux

The whole backend is self-applied and unprivileged, in this order:

1. `prctl(PR_SET_NO_NEW_PRIVS, 1)`, which both later steps require.
2. When `WriteStagedArtifact` is refused: a Landlock ruleset that *handles*
   every write-shaped access class the running ABI knows and grants **no rule at
   all**, applied with `landlock_restrict_self`. The result is a read-only
   filesystem for this process. `EXECUTE`, `READ_FILE` and `READ_DIR` are
   deliberately outside `handled_access_fs`: the capability being refused is the
   write, and refusing reads would enforce something the class never declared.
3. When `OpenOutboundSocket` is refused: a seccomp filter returning `EPERM` for
   the socket family — `socket`, `socketpair`, `connect`, `bind`, `listen`,
   `accept4`, `sendto`, `recvfrom`, `sendmsg`, `recvmsg` — plus `io_uring_setup`,
   `io_uring_enter` and `io_uring_register`, because a submission queue performs
   socket operations the filter would not see.

Landlock before seccomp, because a filter installed first would deny the
`landlock_*` syscalls the step needs.

#### The filter gates an ABI, and the arch word is only half of one

`seccomp_data` carries an `arch` token and a syscall number, and on x86 the token
does not identify the ABI: `AUDIT_ARCH_X86_64` is the token for the 64-bit ABI
**and** for x32, which the kernel tells apart by `__X32_SYSCALL_BIT` — bit 30 of
the number — and by nothing else. A filter that checks the token and then
compares native numbers therefore lets every x32 number fall past the comparisons
to `SECCOMP_RET_ALLOW`.

`P2-A5` measured that here. A `REPOSITORY_ANALYZER` process entered enforcement,
read `Seccomp: 2` back from `/proc/self/status`, printed the receipt below — and
opened a socket and completed a three-way handshake to a listener in a separate
process, through x32 `socket` and x32 `connect`. All five socket-refusing classes
did it. The sentence "a class that does not declare `OpenOutboundSocket` gets no
socket at all" was false on any kernel built with `CONFIG_X86_X32`.

So the gate is two instructions rather than one: the token has to be
`AUDIT_ARCH` **and** the number has to be below the bit, which is an unsigned
floor because every x32 number is `bit | n` and every native number is far below
the bit. A number that is neither ABI's — a negative `nr` read as `u32` — is
refused with them, which is the safe direction.

**Why the x32 numbers are not added to the deny list instead.** An x32 number is
not always the native number with the bit set. x32 has its own entry points from
512 up for calls whose argument layout differs, and this deny list carries three
of them: `recvfrom` is 517 and not 45, `sendmsg` 518 and not 46, `recvmsg` 519
and not 47. The rest keep their native numbers — `socket` 41, `socketpair` 53,
`connect` 42, `bind` 49, `listen` 50, `accept4` 288, `sendto` 44, and
`io_uring_setup`/`_enter`/`_register` 425/426/427. So a deny list built by
setting the bit on each native number would refuse seven of the ten socket calls
and leave three reachable. The ABI gate has no such table to get wrong.

**aarch64 needs no floor.** Its 32-bit compat ABI carries `AUDIT_ARCH_ARM`, a
different token, so the arch check already refuses it. The i386 ABI reachable
from an x86-64 process through `int 0x80` carries `AUDIT_ARCH_I386` and is
likewise already refused — with `SECCOMP_RET_KILL_PROCESS`, measured as a child
killed by `SIGSYS` against a control process that survives the same instruction.

### Windows

There is no mechanism, and the reason is not a gap in this crate. A process
cannot replace its own primary token, and no user-mode call refuses the creation
of a socket handle to the process that asks for it — `docs/contracts/worker-sandbox.md`
records the same measurement for `P2-G4`, whose answer is an AppContainer applied
by the **parent** that calls `CreateProcessW`.

**No launcher in this repository launches a process class.** So on Windows a
process class has no enforcing parent, `enter` returns
`EnforcementError::Unavailable`, and all three enforced binaries exit `1`
without doing work. That is the contract holding: the declaration and the
process agree, because the process does not run.

**What would change that, exactly.** A parent that creates the process with
`CreateProcessW`, a `SECURITY_CAPABILITIES` attribute naming an AppContainer
profile with no capability SIDs, and ACEs for that SID on whatever the class is
allowed to reach; and, in this crate, a Windows arm of `enter` that reads its own
token back with `GetTokenInformation(TokenIsAppContainer)` and refuses when the
answer is no. Neither exists today, and neither is claimed. `academic-worker`'s
launcher is not that parent: `LaunchSpec` carries a `JobPlan` and a
`CapabilityDescriptor` and grants the container read-write on a staged output
directory, which is the capability `RepositoryAnalyzer` and `EgressProxy`
declare they do not have.

### Any other platform, and any build without the backend

`enter` returns `EnforcementError::Unavailable` with the reason. The
`native-enforcement` feature is off by default, so `cargo build --workspace`
produces binaries that refuse to start on every platform. That is the
default-deny posture and it is observable: the acceptance suite in each of the
three crates launches its own binary and requires exactly that.

## Which capabilities are enforced, and where the others live

`academic_process_sandbox::basis` answers this for **every** member of
`ProcessCapability::ALL`, through an exhaustive `match`, so a capability added to
that vocabulary is a compile error here rather than a silent addition to the
unenforced remainder.

| Capability | Basis | Why |
|---|---|---|
| `OpenOutboundSocket` | `PROCESS_BOUNDARY` | the seccomp filter above |
| `WriteStagedArtifact` | `PROCESS_BOUNDARY` | the Landlock ruleset above |
| `CaptureDevice` | `ELSEWHERE` | `academic-capture-gate`'s device ruleset, measured on both hosts by its own probe |
| `ReadArtifactRange` | `BROKER_ONLY` | no operating-system mechanism distinguishes a named artifact range from any other read |
| `WriteSearchIndex` | `BROKER_ONLY` | a search projection is a staged write; separating it needs the staged path this process is not handed |
| `AnalyzeRepository` | `BROKER_ONLY` | computation over bytes already read; no syscall names it |
| `AssembleExport` | `BROKER_ONLY` | the same |
| `CreateClaim` | `BROKER_ONLY` | a claim reaches the core over the local transport; the broker's decision table refuses it on the receiving side |
| `BorrowConnectorCredential` | `BROKER_ONLY` | a credential handle is minted by the broker; a process never handed one holds nothing to refuse |
| `StageExternalPayload` | `BROKER_ONLY` | a write into the core-owned boundary, governed by `WriteStagedArtifact` plus the core's acceptance |
| `ReadKeyMaterial` | `BROKER_ONLY` | no class declares it, and the key hierarchy is in no process-class crate's closure |

The two enforced ones are refused at a **coarser** granularity than their names,
in one direction only:

* a class that does not declare `OpenOutboundSocket` gets no socket at all;
* a class that does not declare `WriteStagedArtifact` gets no filesystem write
  at all.

Both refusals are therefore at least as strong as the declaration. The converse
is the remaining gap and it is stated rather than hidden: a class that *does*
declare `WriteStagedArtifact` is left unrestricted, because scoping the write to
the staged directory needs that directory's path and no process class is handed
one. Closing it means handing each class a descriptor at startup, which is a
different task and a different contract.

## What each class gets

| Class | Declares | Refused |
|---|---|---|
| `CAPTURE_CLIENT` | `CaptureDevice`, `WriteStagedArtifact` | `OPEN_OUTBOUND_SOCKET` |
| `INDEXER` | `ReadArtifactRange`, `WriteSearchIndex` | `WRITE_STAGED_ARTIFACT`, `OPEN_OUTBOUND_SOCKET` |
| `REPOSITORY_ANALYZER` | `ReadArtifactRange`, `AnalyzeRepository`, `CreateClaim` | `WRITE_STAGED_ARTIFACT`, `OPEN_OUTBOUND_SOCKET` |
| `CONNECTOR` | `BorrowConnectorCredential`, `StageExternalPayload` | `WRITE_STAGED_ARTIFACT`, `OPEN_OUTBOUND_SOCKET` |
| `EGRESS_PROXY` | `OpenOutboundSocket` | `WRITE_STAGED_ARTIFACT` |
| `EXPORT_JOB` | `ReadArtifactRange`, `AssembleExport` | `WRITE_STAGED_ARTIFACT`, `OPEN_OUTBOUND_SOCKET` |

The refusal column is computed, not written: it is the complement of the
declaration inside the enforced subset, so there is no per-class exception list
to fall out of date. `each_class_refuses_exactly_what_it_does_not_declare`
compares it against a table spelled from `P2-G7`'s matrix, in both directions.

## Verification is the kernel's answer, not this crate's

A syscall that returned zero is not evidence that a restriction is in force.
After installing, `enter` reads `/proc/self/status` back and requires
`NoNewPrivs: 1`; when a socket was refused, `Seccomp: 2` and a non-zero
`Seccomp_filters`; and when a write was refused, it opens `/dev/null` for
writing and requires the open to fail. Opening `/dev/null` creates nothing, and
a success there means the ruleset did not take, which is
`EnforcementError::NotVerified` and therefore a refusal to start. The answers go
into the receipt line the binary prints.

**`Seccomp: 2` is the weakest of those answers**, and the x32 bypass above is why
that matters: it says a filter is attached and nothing about what the filter
covers, so a process with a filter that refused nothing would read the same `2`.
So when a socket was refused, `enter` also makes one syscall on the x32 ABI and
requires `EPERM` — the filter's own answer, not the `ENOSYS` a kernel without
x32 would give. The syscall is `getpid`, which is **not** in the deny list on
purpose: its answer separates a filter that refuses the x32 ABI from one that
merely carries x32 spellings of the denied numbers, because under the second a
`getpid` on either ABI still returns the pid.

What the receipt carries is that answer — `x32(getpid)=-1`, `-1` being `-EPERM` —
and not a word saying the ABI was asked. The two differ under one edit: a check
that makes the call and drops its result leaves a receipt claiming a refusal
beside a process that got its own pid, and a relayed number cannot say that.
`no_class_reaches_the_second_abi_under_the_same_arch_token` reads it for every
class, in both directions from the declaration: the five whose socket is refused
carry it, and `EGRESS_PROXY`, which declares a socket and installs no filter,
does not.

**It has to be the main thread, and that is checked rather than asked for.**
Both mechanisms apply to the *calling thread* and are inherited by threads
created after it, while the verification reads the thread group leader's status.
A call from anywhere but the main thread, or after a thread has been spawned,
therefore cannot be confirmed and fails closed.
`entering_off_the_main_thread_is_not_confirmed_by_the_kernel` is that as an
observation, and it is also the proof that the verification can fail at all.

## Where the socket spellings are, and why

`crates/process-sandbox/src/linux.rs` names the socket syscalls because it is the
file that refuses them. `only_egress_crate_has_a_socket` reads it against
`SOCKET_ALLOWANCE`, compared as a whole map, and `SECCOMP_DENY_LISTS` applies the
rule `P2-G4` wrote for one file to both: every `SYS_` name in each backend is
either inside its `denied_syscalls` function or one of the three it installs
with. `the_backend_names_only_the_syscalls_it_installs_with_outside_its_deny_list`
in `crates/process-sandbox/tests/scans.rs` repeats that inside the crate, because
a rule that lives in one file only is a rule one merge can drop.

`crates/process-sandbox/probes/enforcement_probe.rs` asks for a socket, because
proving the operating system refuses one means asking for one. It is a `[[bin]]`
with `required-features = ["native-enforcement"]` and a path outside `src`.

`P2-G4`'s rule that **nothing may depend on `academic-worker`** is unchanged and
still enforced; it is not available here, because three product crates do depend
on `academic-process-sandbox` — a process class that enforces its declaration has
to link the thing that enforces it. What keeps the probe out of a product build
instead is that a dependent links a package's *library* target and never its
binaries, so a `[[bin]]` with `required-features` is unreachable from a dependent
whatever edges exist. Both halves are read out of `cargo metadata`.

## What this contract does not claim

- **Three of the six process classes are not enforced yet.** `academic-connector`,
  `academic-export-job` and `academic-indexer` still compute their capability set
  and drop it. `P2-RF21` was scoped to the three binaries `P2-A5` and `P2-A4`
  measured. `the_unenforced_process_classes_are_named` writes the split out and
  re-derives the enforced half from `cargo metadata`, so the remainder is visible
  rather than passing as done, and closing it needs its own process-level
  evidence on both hosts.
- **Nothing is enforced on Windows.** All three binaries refuse to start there.
  This is not a claim that a Windows process is contained.
- **The default build enforces nothing and runs nothing.** With
  `native-enforcement` off, `enter` refuses on every platform.
- **This is not `P2-G4`.** `academic-worker`'s `sandbox::enter` contains a *job*
  running inside a worker process: it takes a `CapabilityDescriptor` with staged
  directories and resource limits, and it grants a writable output directory.
  This crate contains a *process class*, takes no directory, grants nothing, and
  has no resource limits. The two boundaries are one layer apart and neither
  substitutes for the other.
- **The Linux rows were measured on WSL2**, kernel
  `6.18.33.2-microsoft-standard-WSL2`. Section 8.4 of the execution plan says WSL
  may exercise Linux code paths and never substitutes for a Windows claim; the
  Windows rows were measured on Windows natively and the two sets are recorded
  separately for that reason.
- **`ReadKeyMaterial` is not enforced here and does not need to be.** No class
  declares it, and the key hierarchy is in no process-class crate's dependency
  closure — which is a graph fact, checked elsewhere, not a syscall.
