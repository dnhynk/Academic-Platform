# Phase 1 crash, replay, and restore exit

The Phase 1 exit is one sequence run once per enumerated fault. It proves the
local synthetic core's ordering and recovery semantics and nothing else. Passing
it is not ADR-002 acceptance, not permission to ingest real data, not a
hardware-atomicity claim, and not a production-security claim.

## The sequence

For each fault identifier the harness runs exactly this, and stops at the first
step that disagrees with the contract:

1. a fresh profile root begins empty;
2. a harness-owned child ingests only the one repository-allowlisted
   deterministic synthetic fixture;
3. the child is killed at that fault point;
4. a real daemon restarts on the same profile;
5. deep doctor and startup reconciliation run;
6. the same command is retried idempotently, twice;
7. the canonical ledger is replayed from its stored signed envelopes;
8. the profile is exported twice;
9. it is backed up;
10. it is restored into another empty profile;
11. that profile's projections are rebuilt from empty;
12. canonical heads, counts, and semantic checksums are compared.

## Running it

Build output and profiles stay outside the worktree. Set `CARGO_TARGET_DIR` and
the temp lane before either command.

```powershell
cargo test -p academic-daemon --test phase1_exit --locked --offline `
  --features phase1-fault-injection
node tools/phase1-exit.mjs --all-faults --format json
```

The first command is the harness and makes every assertion. The second runs a
disposable profile through every data-bearing CLI surface, re-runs the harness,
and assembles the normalized result document the exit receipt is written from.
`tools/phase1-exit.mjs` asserts nothing of its own about a fault: it reads the
harness's rows out of the run that produced them.

## Surfaces

| Path | Role |
|---|---|
| `crates/test-support/src/fault_driver.rs` | the executable matrix: owner, stage, subject, activation, expected letters, reachability |
| `crates/test-support/src/process.rs` | bounded child control; records each child's PID, profile path, end state, and duration |
| `crates/test-support/src/oracle.rs` | observation and verdict, kept apart from the code that produces the observation |
| `crates/daemon/tests/phase1_exit.rs` | the six named tests and the sequence itself |
| `tools/phase1-exit.mjs` | normalized result schema and exact command receipt |
| `.github/workflows/ci.yml`, job `phase1-exit` | the same commands on native Windows and Linux |

The three `crates/test-support/src` modules are included with `#[path]`, the way
`synthetic_artifacts.rs` already is, so `academic-test-support` keeps no
dependency edge and no product crate can reach the harness.

## Why the child is the test binary

`academicd` has no crash switch and must not gain one. The harness child is this
same test binary re-entered at `phase1_exit_fault_child`, which is the pattern
`crates/vault/tests/crash.rs` and `crates/portability/tests/crash.rs` already
use. The child links exactly the crates and features the harness was built with
and drives the same `LocalService`, `ProjectionRunner`, and portability entry
points the product does — the production entry points reach the identical body
with the no-fault value, so the harness kills the real path rather than a copy.

## How a fault is activated

There are three shapes, because the repository has three.

- **Environment selection.** `academic-vault` (`V01`–`V06`) and
  `academic-portability` (`BK01`–`BK04`, `RS01`–`RS04`) compile a failpoint that
  reads one selection variable and aborts the process itself. The harness sets
  the variable in the child it owns; a process that did not set it never
  evaluates anything.
- **Injected callback.** `academic-store` (`DB01`–`DB07`, `IPC02`) and
  `academic-projections` (`PR01`–`PR03`) expose a trait at fixed ordering
  boundaries. The harness supplies an implementation that writes a ready marker
  and aborts. Production always passes the no-op value.
- **External seam.** `IPC01` only. See below.

Every shape compiles only under the non-default `phase1-fault-injection`
feature. `tools/phase1-scaffold-policy.test.mjs` asserts that no default feature
resolution enables it anywhere, and carries negative fixtures so that assertion
fails if its own predicate is wrong.

## `IPC01` is realized differently, and that difference is not cosmetic

Twenty-five rows are injected failpoints inside the crate that owns the ordering
they protect. `IPC01` is not. The Phase 1 daemon carries no failpoint between
reading a complete request and admitting it to the writer queue, and adding one
would put a crash switch into the product's own serve path for a single matrix
row. The harness instead composes the same two public steps the daemon composes
— `academic_rpc::read_envelope` to completion, then `WriterQueue::try_admit` —
and aborts the child between them, after asserting that the decoded frame is
byte-identical to the request that was framed and that the writer lane has
committed nothing.

**What that proves.** Reading a complete, authorized request frame consumes no
acceptance sequence, writes nothing canonical, and leaves the profile in a state
where the client's retry is a fresh admission carrying the same idempotency key.

**What it does not prove.** That the product's own `serve_connection` body has
no additional side effect between those two calls. That body is covered by the
daemon's connection tests, not by this fault. An exit receipt must not read as
though all twenty-six rows were exercised the same way.

## The oracle

Two invariants are checked for every fault before any letter is assigned,
because they outrank the letters:

- **No normal-looking partial state.** Canonical counts are either exactly
  absent or exactly the fault-free reference. Anything between the two fails the
  run whatever the matrix says the letter should be.
- **No normal-looking reference to a missing or corrupt object.** Every
  canonical artifact reference must resolve through the vault. The deterministic
  export is that proof — it reads each referenced object back through the vault
  and refuses to publish if one is missing — so an export whose object count
  equals the canonical artifact count is a closure receipt.

The letter itself is the disposition of the artifact the row's termination point
protects, named by `FaultSubject`. This matters because the physical end state
of two adjacent rows can be identical: an interrupted `V06` and an interrupted
`DB01` both leave one sealed object with no canonical reference, and only the
subject distinguishes the vault's orphan-disposition contract from the store's
rollback contract.

| Subject | Rows | What must be true |
|---|---|---|
| `VAULT_TEMP` | `V01`–`V05` | canonical absent, and no unpublished temp survives the expiry pass |
| `SEALED_OBJECT` | `V06` | canonical absent, and reconciliation recorded an explicit disposition for the object |
| `CANONICAL_TRANSACTION` | `DB01`–`DB07`, `IPC02` | absent, or complete with object closure |
| `PROJECTION_GENERATION` | `PR01`–`PR03` | no generation is queryable while disagreeing with the canonical head, and a clean rebuild activates |
| `BACKUP_DIRECTORY` | `BK01`–`BK04` | destination unpublished, staging discoverable and removable, source unchanged |
| `RESTORE_DESTINATION` | `RS01`–`RS04` | destination not publishable, staging removable, source and backup untouched |
| `QUEUED_REQUEST` | `IPC01` | nothing written, and the retry is a fresh admission with the same key |

`R` is added only when the subject is the canonical transaction and that
transaction had already committed. Every fault after the ingest stage retries
against a profile that already holds the fixture, so its retry always replays a
stored receipt; the harness asserts that for all twenty-six, but letting it add
an `R` would hand later rows a letter their termination point never earned.

### The temp expiry pass

The vault deliberately keeps an unexpired `*.partial`, because a live one may
belong to a concurrent ingest, and the daemon always reconciles at the current
clock. A leftover from a kill seconds ago is therefore `TempLive` and stays,
which is correct. The `V01` row says *expired* temp removed, so before the
restart the harness asks the product's own reconciliation the question the row
asks, with the expiry threshold set to zero and no descriptors supplied. Only
the temp lane is in scope; a sealed object is left for the daemon's own startup
pass to dispose of, and the clock is not moved.

## `BK03` is `NOT_RUN` here

`academic_portability::backup` trips `BK03` at index 1 of the reachable-object
copy, which is the only honest position for "midway through" that copy. The one
allowlisted synthetic fixture registers a single artifact, so the exit corpus
never reaches index 1 and there is no midpoint to interrupt.

Reaching it would need a second allowlisted fixture, which the synthetic-only
data policy does not admit, or a moved failpoint, which would change product
ordering to make a test pass. Neither is permitted, so the row carries a typed
`Reachability::NotRunInExitCorpus` with its reason and the suite that does cover
it: `crates/portability/tests/crash.rs::bk01_bk04_leave_no_partially_published_backup`,
against a two-artifact corpus that crate builds for itself.
`phase1_exit_at_every_fault_point` asserts the unreachable set is exactly
`["BK03"]`, so a second unreachable row cannot appear silently. A receipt says
25 PASS and 1 NOT_RUN; it never says 26 passed.

A pointer is not evidence, so the exit lane runs the suite it points at. Both
`tools/phase1-exit.mjs` and the `phase1-exit` CI job execute
`cargo test -p academic-portability -p academic-vault --test crash --locked --offline --features phase1-fault-injection`
— those tests are `#![cfg(feature = "phase1-fault-injection")]` and compile to
nothing in every default-feature build, so nothing else in the lane executes
them — and record its argv and exit status in every `not_run[].covered_by_result`.
A `NOT_RUN` row whose covering suite did not pass fails the run.

## The normalized result

`tools/phase1-exit.mjs --format json` emits one
`learning-platform.phase1-exit-result.v1` document containing the commit and
tree hash, worktree cleanliness, the pinned tool versions, the resolved default
Cargo feature graph, the banner and policy object read back from every data-bearing
surface, the accepted fixture's acceptance range, receipt identity and stored
signed-envelope SHA-256, the deep
doctor before and after an abrupt kill, the two exports' agreement, the backup,
the restore into a new empty profile, every fault row with its expected and
observed letters, the six named-test results, the open gates, and the exact
command receipt.

The tool versions are read from `doctor --format json`, not from a direct
spawn. On Windows a bare-name spawn appends only `.exe` and never consults
`PATHEXT`, so the `.cmd` shim `npm install --global pnpm@11.22.0` writes is
unreachable by name, and Node refuses to spawn a resolved `.cmd` without a
shell. `doctor` already resolves and runs it over the same four pinned tools,
so the receipt and the doctor cannot disagree about a tool the host has.

The command receipt records argv, working directory, exit status, and duration
for every command the run executed. It never records command output: a receipt
is evidence about what ran, not a transcript of what a synthetic profile held.

## Bounds and recorded identity

Every child wait is bounded; a child that does not reach its checkpoint is
terminated and reported as `TimedOut` rather than parking the suite. Each child
record carries its operating-system process identifier, its disposable profile
path, how it ended, and how long it took. Child streams are attached to the null
device, so no ingested bytes and no diagnostic text can reach a receipt.

## Platform evidence is per platform

Windows named-pipe evidence and Unix domain-socket evidence are separate claims.
Every emitted row states its `host_family`, and neither lane's receipt may be
read as the other's. A definition existing in `.github/workflows/ci.yml` is not
a hosted run; hosted proof is H1.

## Not claimed

- No ADR-002, ADR-004, or ADR-005 acceptance. Storage stays
  `PLAINTEXT_TEMPORARY_SQLITE` with `storage_encryption: NONE`.
- No production or personal data admission. Only the one allowlisted synthetic
  fixture is accepted, and `phase1_exit_rejects_real_data` proves the refusal at
  the request builder, at the daemon, and in the resulting profile.
- No hardware-atomicity or physical power-loss claim. The kills are process
  aborts; the matrix is a deterministic software fixture.
- No product networking. `phase1_exit_has_no_product_network` scans every
  product crate's sources and separately link-scans a default-feature
  `academicd` built in its own target directory.
