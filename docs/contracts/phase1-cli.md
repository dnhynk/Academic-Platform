# Phase 1 CLI: daemon, doctor, ingest, export, backup, restore, crash-replay

## Posture

`academic` operates a synthetic, plaintext, throwaway Phase 1 profile. Real or
production data is forbidden until ADR-002 is accepted, and no flag, environment
variable, configuration key, or debug path in this binary changes that.

Every command emits the frozen policy object:

```json
{
  "data_policy": "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
  "storage_mode": "PLAINTEXT_TEMPORARY_SQLITE",
  "storage_encryption": "NONE",
  "production_data_allowed": false,
  "product_network": "NONE"
}
```

In `--format human` the banner
`PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN` is the
first line of standard output, before any result, on success and on failure. In
`--format json` standard output carries exactly one JSON document so a caller can
parse it without stripping a preamble; the banner goes to standard error and is
repeated inside the document alongside the policy object.

The policy values are compile-time constants copied from the frozen protocol
contract. `policy_banner::data_policy` takes no argument, so no runtime input can
reach it, and a `const` assertion fails the build if `production_data_allowed`
ever becomes true.

## Command surface

| Command | Purpose |
|---|---|
| `academic daemon serve --profile <p> [--runtime <r>]` | Hosts one foreground daemon until the terminal interrupts it. Creates the throwaway profile when the root is absent. |
| `academic daemon status --profile <p> [--runtime <r>]` | Negotiated protocol facts plus canonical and projection watermarks. |
| `academic doctor [--profile <p>] [--deep]` | Pinned toolchain checks; with a profile, store identity and canonical watermarks; with `--deep`, integrity, foreign keys, vault residue, and projection lag. |
| `academic ingest --profile <p> --fixture <id> [--runtime <r>] [--expected-revision <n>]` | Accepts one allowlisted synthetic fixture through the daemon. |
| `academic export --profile <p> --destination <d> [--runtime <r>]` | One deterministic open export directory. |
| `academic backup --profile <p> --destination <d> [--runtime <r>]` | One plaintext synthetic backup directory. |
| `academic restore --backup <b> --new-profile <n> [--runtime <r>]` | Rebuilds a verified backup into a new empty profile. |
| `academic crash-replay (--fault <id> \| --all)` | Reports the enumerated fault matrix. |
| `academic fixture emit\|verify\|replay` | Deterministic committed-fixture workflows. |

`--format human\|json` is accepted by every command except `fixture`, which
already renders JSON.

`--runtime` defaults to `%LOCALAPPDATA%` on Windows and `$XDG_RUNTIME_DIR` on
Unix. The lookup fails closed: there is no world-writable fallback, because a
shared temporary directory would let another account present a socket or a
session file to this one.

Caller-supplied paths are normalized to the host's native absolute form at the
argument boundary. On Windows this is what makes a relative or dot-bearing
spelling usable below the vault: the durability layer prefixes only a rooted
spelling and refuses a non-absolute one at the handle rename, and it rejects a
`.` or `..` component with a typed error instead of collapsing it, because
collapsing is only correct when no earlier component is a link. Resolving both
needs the process working directory, which the composition root owns. Separator
spelling is not part of this: the vault normalizes separators itself before it
applies any prefix, for every caller, so a forward-slash argument is addressed
correctly either way.

## Exit codes

Exit codes distinguish *why* a command failed, so a caller can branch without
parsing prose. `crates/cli/tests/cli.rs` produces each class from a real command
rather than asserting against this table.

| Code | Class | Meaning |
|---:|---|---|
| 0 | `OK` | The command completed. |
| 2 | usage | Clap rejected the invocation. No outcome class may claim this code. |
| 10 | `POLICY_DENIED` | The synthetic-only data policy refused the request. |
| 11 | `CONFLICT` | A destination was occupied, or an expected revision or watermark conflicted. |
| 12 | `REPAIR_REQUIRED` | The profile or artefact must be repaired before it is served or published. |
| 13 | `INCOMPATIBLE` | A protocol version, capability, artefact format, or fault identifier could not be negotiated. |
| 14 | `UNAVAILABLE` | No daemon owns the profile, so an IPC-only command cannot proceed. |
| 20 | `INTERNAL` | None of the above describes the failure. |

A failing command still emits its structured result where it produced one, so a
`REPAIR_REQUIRED` doctor shows the findings that demanded the repair.

## What travels over IPC, and what cannot

`ingest` is the only canonical mutation and always travels over local IPC to the
daemon, which is the sole canonical writer. There is no offline ingest path.

`daemon status` completes a read-only handshake.

`export` and `backup` are reads. When a daemon owns the profile they complete a
read-only handshake first, so the owning daemon remains the authority over its
own profile: a `LOCKED` or `REPAIR_REQUIRED` lock state stops the command before
anything is written. When no daemon owns the profile they run offline against the
same read-only boundary, which opens the database with SQLite read-only flags
through the guarded store reader. The CLI never holds a writer handle.

Two limits come from the frozen P1 contract rather than from this task:

- `MutableRequest.command` is a **closed** oneof of synthetic ingest, backup, and
  restore, with reserved tag bands and golden/compatibility tests. There is no
  export command, so export bytes cannot travel over IPC in this phase.
- `SyntheticBackupCommand` carries no destination field and
  `SyntheticRestoreCommand` carries only a 16-byte receipt identifier, so a
  destination-bearing backup or restore cannot be expressed over IPC either.

Changing either would require a Proto change, which is the P1/F0 frozen surface.
Until then the artefact bytes are produced locally and the handshake supplies the
policy object, lock state, and negotiated capability.

`restore` is offline by contract. It targets a destination no daemon owns, and it
refuses a destination a daemon does own.

### Recorded deviation from the T009 C1 line

T009 specifies that the CLI's data commands travel over IPC except restore. The
frozen Phase 1 protocol cannot express two of them: `MutableRequest.command` has
no export arm at all, and neither `SyntheticBackupCommand` nor
`SyntheticRestoreCommand` carries a destination, so "export to `<path>`" and
"back up to `<path>`" have no wire representation. This build therefore routes
ingest — the only canonical mutation — strictly over IPC with no local fallback,
answers `daemon status` from the IPC handshake, and runs export and backup
locally read-only after first handshaking with any daemon that owns the profile
and refusing when that daemon reports `LOCKED` or `REPAIR_REQUIRED`. The
single-writer invariant of ADR-001 is intact because the deviation moves only
reads: export and backup admit the source profile through the guarded store
reader, then read it through a `SQLITE_OPEN_READ_ONLY` handle constrained to
`query_only`, hold no writer handle on it, and append no event, receipt, or
revision — so the owning daemon remains the sole canonical writer and keeps its
veto over both commands through the handshake. This is a deliberate recorded
deviation, not an omission; it ends when the protocol gains an export arm and a
destination-bearing backup, which is a Proto change against the F0 frozen
surface.

### After an abrupt daemon termination

A daemon killed abruptly cannot remove its own session metadata, so stale
metadata is the normal state after a crash. The CLI treats an endpoint nothing
answers as an unowned profile:

- `export` and `backup` fall back to the offline read and report
  `ownership.stale_session_metadata: true`. This is what makes them usable after
  a crash, which is when they are needed most.
- `daemon status` exits `UNAVAILABLE` with reason `DAEMON_NOT_RUNNING`, which is
  distinct from `NO_DAEMON_OWNS_PROFILE` for a profile no daemon ever served.

## Ingest is idempotent by construction

Every identifier in the ingest request is derived deterministically from the
fixture name under fixed domain separators, so a repeated `academic ingest`
presents the same `(client_instance_id, idempotency_key)` pair and the daemon
returns the original stored receipt with status `DUPLICATE` instead of accepting
the batch twice. That is the same guarantee the DB07 and IPC02 fault points
require after a lost acknowledgement.

`--fixture` names one entry in a compile-time allowlist. It is never a file path
and cannot select arbitrary bytes. The name is refused by the CLI before a
connection is opened and refused again by the daemon, so neither side is the only
guard.

## `crash-replay` is a report, not a switch

`academic crash-replay` **cannot terminate anything**. Faults compile only under
the non-default `phase1-fault-injection` feature of the crate that owns each
failpoint, and even there a fault fires solely when a test harness has set the
selection variable in a child process it owns. A production build contains no
user-accessible crash switch, and `injectable_by_this_build` is always `false`.

The command reports, for each of the 26 enumerated faults, the owning subsystem,
the termination point, and the outcome a restart must produce (`N` no reference,
`C` complete, `Q` quarantine, `R` idempotent retry). A harness terminates a real
daemon by fault identifier and then checks the resulting profile against these
rows with `academic doctor --deep`.

## Deep doctor findings

| Code | Severity | Meaning |
|---|---|---|
| `SYNTHETIC_MARKER_MISSING` | repair required | The synthetic-only marker file is absent from the profile root. |
| `INTEGRITY_CHECK_FAILED` | repair required | `PRAGMA integrity_check` did not return `ok`. |
| `FOREIGN_KEY_CHECK_FAILED` | repair required | `PRAGMA foreign_key_check` reported violations. |
| `ORPHAN_TEMP_PRESENT` | repair required | Unpublished `*.partial` ingest temps remain in the vault. |
| `QUARANTINED_OBJECTS_PRESENT` | warning | Quarantined `*.orphan` entries await disposition. |
| `PROJECTION_LAG` | warning | An active generation is behind the canonical outbox head. |

Only files carrying the documented extension are counted. The vault's own
directory barrier marker is structure, not residue, and is never reported as an
orphan.

Projection lag is expected after an ingest, because the daemon does not run the
projection consumer: the generations are rebuilt from empty by `restore`, and a
lagging generation is reportable rather than a failure.

## Non-goals

No background service manager, no interactive UI, no arbitrary import, and no
external connector. The CLI takes no database writer dependency and offers no
SQLCipher key option.
