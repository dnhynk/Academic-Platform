# Egress boundary contract

`academic-egress-boundary` is the `P2-G2` staging, preview, and outbound seam.
It sits above the `P2-G1` broker and the `P2-G3` provider registry and reuses
their grant, token, audit, and provider records rather than defining its own.

## Why it is not inside `academic-egress`

`P2-G7` created `academic-egress` as the egress-proxy process identity and pins
its whole manifest and its whole product source as one fixed process-class
binding. A library target inside that package would have made
`six_process_entrypoints_are_exact_and_distinct` weaker rather than exact, so
the boundary is a second package with the same identity in the topology. The
socket, when one exists, belongs to the process; what this crate owns is
everything that must be right before the process may open one.

`P2-G7`'s `ProcessClass` matrix is what makes that division enforceable:
`OpenOutboundSocket` is allowed for `EgressProxy` and no other class, and
`RuntimeToolCall::new` refuses a call whose class does not hold it. So a
transmission is refused from the wrong process before this crate's guards run,
and before any byte reaches a transport.
`a_transmission_from_another_process_class_is_refused` walks all five other
classes with everything else in the plan held equal, observes `SCOPE_MISMATCH`
and zero bytes written for each, and then transmits successfully as
`EgressProxy` so the refusals are attributable to the class.

## What ships, and what does not

The crate ships the versioned DLP rulepack, structural minimization, the
byte-accurate preview, the staged grant journal, the provider-response canary
scan, and one outbound transport **trait**.

It ships **no socket**. ADR-002 is unaccepted, the admission receipt is
incomplete, and the emitted posture is still `product_network: "NONE"`, so there
is no destination a socket could legitimately reach. `OutboundTransport` is
supplied by the caller; this repository contains no implementation of it and no
crate in the workspace names an outbound socket construct.

`only_egress_crate_has_a_socket` in `tools/phase1-scaffold-policy.test.mjs` is
what keeps that exception scoped by crate rather than granted globally, which is
what execution-plan section 2.3-14 requires. What it enforces is below.

## Section 3.6 topology

```text
core (no socket) ──> broker decision ──> staged payload (in memory, TTL = grant expiry)
                                                   │
                                          academic-egress ──> OutboundTransport
```

`EgressProxy::stage` produces a `StagedPayload`; `EgressProxy::transmit` is the
only function that reaches a transport, and it does so inside
`PermissionBroker::execute`, which refuses a `RuntimeToolCall` whose process
class does not hold `OpenOutboundSocket`. A caller that never calls `transmit` cannot reach a
transport, and `transmit` cannot be called without a `StagedPayload` and a live
`CapabilityToken`.

## The rulepack identifier in every grant

The shipped pack is `academic-dlp-rulepack/1`. Its identity is the lowercase
SHA-256 of `academic-dlp-rulepack-v1\0`, the pack name, the version, the rule
count, and each rule's identifier, reason code, and kind — so an edit to a rule
moves the digest even if the version is not incremented.

That digest is a grant's `redaction_policy_hash`. It is the existing `P2-G1`
column, not a new one. `transmit` reads the grant row before it does anything
else and refuses with `SCOPE_MISMATCH` when the recorded digest is not the
digest of the pack that produced the staged bytes: a grant reviewed under one
redaction policy may not carry a payload produced by another.

## The staging pipeline

Seven steps, in this order, each refusal final:

1. **Source size** against the destination's `maximum_input_bytes` from the
   provider registry. Refuses `OVERSIZE`. It runs before classification so an
   oversize archive is reported as oversize, which tells the caller to send
   less, rather than as unreadable, which would send them looking for a decoder.
2. **Classification.** A known container or executable magic, invalid UTF-8, or
   a control byte no source text uses refuses `UNKNOWN_BINARY`. A scanner cannot
   report what it cannot read.
3. **Minimization.** A whole-document request is reduced to the brace-balanced
   declarations of the requested symbols. A symbol the document does not declare
   refuses `SCOPE_MISMATCH`; it is not a licence to send the whole document.
4. **The scan** over the selected slice. Any finding refuses with that finding's
   reason code — `SECRET_PATTERN`, `SECRET_ENTROPY`, or `PII_DETECTED`. A scan
   that cannot finish refuses `SCANNER_ERROR`; there is no partial result,
   because the caller could not tell one from a clean payload.
5. **Redaction**, which substitutes the identifiers the caller's
   `IdentifierPolicy` names with stable `IDENT_n` placeholders. Refuses
   `REDACTION_DESTROYS_MEANING` when the policy renames a requested symbol, or
   when the substituted share of the slice's non-whitespace bytes passes the
   policy's own bound.
6. **A second scan** of the redacted bytes, so a substitution can neither
   introduce nor preserve a finding.
7. **Staged size**, because a substitution can make a payload longer.

Secrets are refused, never redacted. A substituted secret still leaks its
position and length, and the fault matrix's `EG01`–`EG03` outcomes are denials.

The whole pipeline is pinned as whitespace-collapsed whole text by
`the_byte_path_has_one_derivation`, so its step order, its reason codes, and
every default it takes change only in a commit that changes that pin.

## Preview and transmission are one buffer

`Preview` owns the only copy of the staged bytes. `StagedPayload` holds the
preview; `Preview::bytes` returns the field and computes nothing. The runtime
call is built from `staged.preview().bytes()`, and the transport is written from
`authorized.payload()` — the buffer `PermissionBroker::execute` has just
re-hashed against the grant's `payload_digest`.

There are therefore two independent reasons the transmitted bytes are the
previewed bytes, and one of them is enforced by code this crate does not own:
the grant is minted over `StagedPayload::object_range`, whose digest is the
preview's, and the broker refuses any other payload at the capability boundary.

`preview_bytes_equal_transmitted_bytes` observes a run; the whole-text pins on
`staged_runtime_call`, `write_authorized_bytes`, `Preview::bytes`, and
`StagedPayload::preview` are what stop a second derivation being introduced
later.

## Zero bytes on a refusal

`stage` returns either a `StagedPayload` or an `EgressDenial`. The denial type
has exactly four fields — reason, detail, findings, and a transmitted-byte
count — and no payload; `a_denial_has_no_payload_field` reads them. The staged
bytes on a refusal are not withheld from the caller, they are never built.

The one non-zero count is a transfer already past the capability boundary:
`EG04`. See below.

## Reason codes

The closed enum is `P2-G1`'s and is unchanged. `deny_reason_codes_are_exhaustive`
enumerates it four ways rather than counting it: a compiler-checked witness
`match` that stops the suite compiling when a variant is added, an index set that
fails when the list omits one, a transcription of the execution plan's section
3.5 sentence, and the `egress_audit` `CHECK` read out of
`crates/policy/src/schema.sql`. Each of the fifteen names a producer, and every
code this crate claims is produced by actually running the pipeline in that test.

`GRANT_EXPIRED` is produced here as well as by the broker: a transfer that
outlives its grant mid-stream aborts with it.

## Faults

| Fault | Outcome |
| --- | --- |
| `EG01` scanner error | `SCANNER_ERROR`, zero bytes |
| `EG02` over the size threshold | `OVERSIZE`, zero bytes |
| `EG03` unknown binary in the slice | `UNKNOWN_BINARY`, zero bytes |
| `EG04` grant expires mid-transfer | aborted; the partial count is journalled |
| `EG05` kill after send, before the outcome record | reconstructed from the journal; grant already consumed; second send refused |
| `EG06` canary in a provider response | quarantined; high-severity incident; response bytes dropped |
| `EG07` provider offers no deletion receipt | `P2-G3`'s decision; this crate only carries the code |
| `EG08` redaction destroys meaning | `REDACTION_DESTROYS_MEANING`; `LOCAL_ONLY_OR_STOP` |

### Where `EG04`'s partial count is recorded

`PermissionBroker::execute` commits its allow audit row *before* it calls the
tool closure, so `egress_audit.byte_count` is the count the grant authorized —
the whole staged payload. The count actually handed to the transport is in the
staged grant journal's `SendOutcome`, and `EgressDenial::bytes_transmitted`
reports it to the caller. The two records name the same `grant_id` and the same
payload digest, and `eg04_grant_expiring_mid_transfer_aborts_and_audits_the_partial_count`
asserts both. This crate does not write a second row into `P2-G1`'s append-only
table to reconcile them; doing so would need a new broker API, which is a
`P2-A2` integration decision rather than this task's.

### Where `EG05`'s reconstruction comes from

The journal records a `SendIntent` before the capability boundary and a
`SendOutcome` after the transport returns. A kill between them leaves an intent
with no outcome, and `StagedGrantJournal::reconstruct` returns exactly those.
The grant's `consumed_at` is already set — the broker consumes before it calls
the closure — so a second send of the same token is refused `GRANT_CONSUMED`
with zero bytes written. The journal holds identifiers, digests, and counts and
no payload byte, for the same reason `egress_audit` does not.

## Provider responses

`accept_response` scans a response with the registered canary corpus and the
rulepack. Any hit quarantines: the bytes are dropped inside the call and the
caller receives an `Incident` carrying digests, ranges, and rule names. A
response that could not be scanned is quarantined too, with `SCANNER_ERROR`. An
accepted response is returned as `AcceptedResponse`; nothing here persists a
claim, which is `P2-M1`'s boundary.

## `GATE-38-028` stays open

`cloud_egress_default()` returns `Route::LocalOnlyOrStop` and takes no argument.
No quality score, benchmark, or confidence estimate can be passed to it, so none
can change it. What closes the gate is the user configuring a per-tuple egress
rule through the broker; until one exists the broker denies `NO_GRANT` and this
is the route. Every `EgressDenial::route` is `LOCAL_ONLY_OR_STOP` as well: there
is no reason code that routes to a retry, a downgrade, or another provider.

Both `cloud_egress_default` and the routing constant are pinned as whole text.

## What `only_egress_crate_has_a_socket` enforces

Six halves, none of which is a forbidden-token list.

1. **Per-file spelling allowance.** Every `.rs` file under every crate's `src`,
   `tests`, and `benches`, plus every build script, is read with comments and
   string literals removed, and the exact set of socket spellings it uses must
   equal a pinned allowance. Eight files run the local IPC seam — named pipes on
   Windows, Unix-domain sockets elsewhere — and their allowances list only local
   transports. Every other file's allowance is empty, both egress crates
   included. A file allowed `tokio::net` for a named pipe is not allowed
   `TcpStream`, and reaching one through the other spells it.
2. **No usable alias of a socket path.** Inside a `use` rooted in `std`,
   `core`, `alloc`, `tokio`, `rustix`, `libc`, `windows_sys`, `socket2`, `mio`,
   or `nix`, two renames may only ever go to `_`, which cannot be written as a
   path: the crate root itself (`use tokio as t;` leaves `t::net::TcpStream`
   spelling nothing on the list) and a socket module segment — `net`, `socket`,
   `sys`, `WinSock`, `named_pipe` — including inside a braced group
   (`use tokio::{net as n};`, which the `tokio::net` anchor does not match).
   Renaming anything else, such as `process::Command as ProcessCommand`, is not
   on a socket path and is left alone: forbidding it would be a rule about
   imports rather than about sockets.
3. **No foreign function declaration.** `extern "…"`, `#[link(…)]`, and
   `no_mangle` appear nowhere in a workspace crate. `unsafe_code = "forbid"`
   already refuses these outside the four reviewed leaves; this refuses them
   inside those four too, which is where a winsock import would otherwise fit
   without spelling a Rust socket name.
4. **No source from outside the scanned trees.** Every `#[path]` target resolves
   to an existing `.rs` file under `crates/`, and the single `include!` is
   pinned as whole text with its build script.
5. **A pinned build-script inventory.** `academic-rpc` is the only crate with
   one; a new build script can generate source this scan never sees.
6. **The link half.** Each workspace crate's normal-and-build dependency closure
   is intersected with the crates that can open a socket, and the resulting map
   is pinned. This is the one bypass that spells no name anywhere: adding
   `tokio` to a manifest. Both egress crates intersect to `["libc"]`, which
   reaches them through `academic-policy`'s bundled SQLite.

`P2-G7`'s `indexer_cannot_open_a_socket` and `export_job_cannot_read_keys` prove
two process packages' whole dependency closures and whole entry points. This
scan is the workspace-wide narrowing of the same claim: those two read one
package each, and this reads every `.rs` in every crate with comments and string
literals stripped, so an aliased path or a foreign function declaration in any
of them is refused as well.

Each of these was verified by injecting the bypass it names into the shipped
source and observing the guard refuse it. All three shapes
[policy source scans](policy-source-scans.md) names as making a scan empty are
answered here: the walk covers `src`, `tests`, `benches`, and build scripts and
compares the whole allowance map rather than iterating it, so a file that stops
being read fails as a missing key; the checks that matter are whole-text pins
rather than token lists; and the capability scan it sits beside carries a
`>= 10` crate floor.

The day a socket is written, the allowance in half 1 and the closure in half 6
change in the same commit. That is the review.

## What this contract does not claim

- No socket exists, so nothing here is evidence that a socket would be confined
  at run time — that is `P2-G4`'s operating-system sandbox. It is evidence that
  one cannot appear outside the two egress crates without a reviewed change to
  the guard, and that neither of them has one today.
- The item reader is a brace-balancing scanner with string, character, and
  comment awareness. It is not a Rust parser: it resolves no paths, macros, or
  generics, and a symbol produced by a macro is not found by it. A symbol it
  cannot find is a `SCOPE_MISMATCH` denial rather than a fallback to the whole
  document.
- The span lexer recognizes `//`, `/* */`, and double-quoted strings. Raw
  strings and nested block comments are not modelled; a finding inside one is
  still reported, with the span kind the lexer reached.
- The DLP corpora are synthetic and composed at run time from fixed
  sixty-four-bit seeds. Passing them is evidence about these rules against these
  shapes, not a claim of completeness against real secrets.
- `product_network` remains `NONE` and `production_data_allowed` remains
  `false`. Nothing in this task moves either.
