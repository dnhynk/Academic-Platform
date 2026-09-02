# Permission broker contract

`academic-policy` is the P2-G1 decision and runtime-capability boundary and the
P2-G7 process-capability/audit boundary. It has no network API. Its embedded
SQLite schema can be opened at an explicit local path for retention; isolated
tests may use the in-memory constructor.

## Request and policy pin

The canonical design §32.3 names eight semantic fields:

1. actor;
2. data object/range;
3. operation;
4. purpose;
5. destination/provider;
6. retention;
7. time; and
8. consent evidence.

Execution-plan §3.5 expands data object/range into `data_class` plus
`object_range_digest_set` and adds `policy_version`, producing ten concrete
entries while still calling the tuple “eight-field.” P2-G7 separates the actor
identifier from the trusted, non-optional `ProcessClass` execution context.
P2-G1 checks all ten optional request entries before selecting a rule, so
neither reading leaves a missing entry permissive.
`missing_tuple_field_denies_and_audits` varies all ten and observes one
`NO_GRANT` audit for each; every resulting row also carries the typed process
class. Policy and request hashes use v2 domain separators because this split
changes their canonical input.

A `PolicySnapshot` is encoded with a fixed domain separator, big-endian counts
and lengths, and rules sorted by their canonical bytes. `PolicyVersion` is the
lowercase SHA-256 of that encoding. Installed snapshots are immutable by hash;
replay looks up the request's pinned hash rather than a newly installed policy.

## Default and minimization

The new-profile snapshot reports local processing preferred and contains no
egress rules. A fully populated request using that snapshot is denied with
`NO_GRANT` and audited. `GATE-38-010` therefore remains open: only an explicit
per-tuple rule supplies an egress preference.

An explicit rule carries its smallest required half-open byte ranges. A broad
request is reduced to the smallest satisfying configured rule, with total byte
count and canonical rule bytes as deterministic tie-breakers. A request that
does not contain one complete configured minimum is denied with
`SCOPE_MISMATCH`.

## Grant and runtime boundary

An allow inserts the fifteen §3.5 grant columns. `max_uses` has a database
`CHECK (max_uses = 1)`. All columns are append-only except the first transition
of `consumed_at` from null to a timestamp.

`PermissionBroker::execute` receives the exact payload bytes and recomputes
their SHA-256 at the boundary. Actor, process class, operation, purpose, destination, byte
ranges, payload length, and digest must equal the opaque token and its stored
grant. The atomic SQL update requires `consumed_at IS NULL` and
`expires_at > now`; the tool closure is invoked after that update and its allow
audit commit. The acceptance test races two threads with the same token and
observes exactly one closure invocation. Another test supplies a wider runtime
range and observes `SCOPE_MISMATCH`, a deny audit, and zero closure calls.

## Process classes and cross-capability matrix

The closed `ProcessClass` enum is `CAPTURE_CLIENT`, `INDEXER`,
`REPOSITORY_ANALYZER`, `CONNECTOR`, `EGRESS_PROXY`, and `EXPORT_JOB`. Each has
one exact, distinct capability set:

| Process class | Allowed capabilities |
|---|---|
| capture client | capture device; write staged artifact |
| indexer | read artifact range; write search index |
| repository analyzer | read artifact range; analyze repository; create claim |
| connector | borrow scoped connector credential; stage external payload |
| egress proxy | open outbound socket |
| export job | read artifact range; assemble export |

`READ_KEY_MATERIAL` is a closed-enum capability with no allowed cell. A
`ProcessCapabilityToken` is opaque, expiring, single-use, and bound to actor,
process class, and one capability. The P2-G1 `CapabilityToken` is likewise
bound to actor and the typed `EGRESS_PROXY` class. Runtime use supplies actor,
class, and capability independently, so injecting either the wrong class or
the wrong capability is denied without consuming the token.
`cross_capability_matrix_denies_every_disallowed_cell` walks the full Cartesian
product and then injects every other class and every other capability into each
allowed token.

The six classes also have six separate executable packages. Their complete
manifests, Cargo targets, product source, and resolved dependency closure are
checked as wholes. `indexer_cannot_open_a_socket` and
`export_job_cannot_read_keys` are those whole checks on two of them: each
package's shipping closure equals one reviewed list that contains no network or
key-material crate, and its entire product source equals its one fixed
process-class binding.

Those two names are stronger than what the checks establish, so read them as
scoped. The standard library puts `std::net` and a file read within reach of
any process that links it — `std::net::TcpStream::connect` compiles inside
`academic-indexer` today. What is executable here is availability through
dependencies and use in source, not what the operating system permits the
process. P2-G4 owns the sandbox that would make the unqualified reading true.
The `academic-egress` entrypoint has the logical socket capability but still
contains no transport; P2-G2 owns adding and testing the sole product socket.

## Audit contents and retention

Every evaluation, replay, and runtime use appends one audit row. Denials use
the closed §3.5 reason-code enum. Allows use a null `reason_code`, because that
closed enum contains no allow code; the execution plan lists the column but
does not define its allow-row nullability.

P2-G1 uses individual codes in individual tests and enumerates none of them, so
"closed" was a claim about the enum rather than a checked property of it.
`deny_reason_codes_are_exhaustive` in
`crates/egress-boundary/tests/egress_boundary.rs`
is where that became executable: a compiler-checked witness `match` over
`ReasonCode`, an index set that fails on an omission, a transcription of the
section 3.5 sentence, and the `egress_audit` `CHECK` read out of `schema.sql`.
Adding a variant stops that suite compiling; removing one from any of the four
lists fails it.

Audit rows contain actor identifier, typed process class and capability,
artifact identifiers with half-open ranges and content digests, external
destination/digest/count, created claim identifiers, and times. Range and
claim children are append-only rows keyed to the parent audit sequence. They
contain no raw payload, prompt, source artifact, or provider response fields. The existing
`tools/secret-debug-policy.test.mjs` source discovery net covers raw fields
named `payload`, `prompt`, `provider_response`, and — from P2-G7 — the
transmission byte-buffer names; P2-G1 does not add a second
audit-only scanner. P2-G7 registers `ProcessActivity` in that net, and because
the generic vocabulary now reaches `transmitted_bytes`, the net holds whether
or not that registration stays. During
P2-G1 acceptance, a temporary `AuditRow.payload_bytes:
Vec<u8>` injection first passed the old vocabulary, then failed the extended
generic discovery net, and passed again after the injection was removed. That
investigation also found signed `DecodedEnvelope.payload` buffers one layer
outside the broker; their derived `Debug` implementations were removed.

Every parent row carries the fixed `SECURITY_AUDIT_APPEND_ONLY` retention
identity. The schema identity, grants, parent audits, ranges, and created-claim
links reject update/delete (grant consumption is the sole one-way exception).
`PermissionBroker::open` persists the audit database at a caller-selected local
path, and the retention test reopens it and observes the same rows after direct
update/delete attempts fail. Profile-path placement remains an integration
responsibility.

`audit_contains_no_raw_canary` reuses the committed SQLCipher canary corpus and
passes exact, prefixed, case-changed, and reversed variants through the real
transmission-audit path. Only a SHA-256, range, and byte count remain. It reads
two surfaces, because either alone has a hole. The rows the crate projects back
are rendered and searched; then the same variants are written through an
on-disk broker and the whole retained database file is scanned for their bytes,
since a row the read API does not project back is still a copy. Beside those,
the applied schema is enumerated whole — every table and every column compared
against an exact expected set, so a new column or side table fails whatever it
is named. A blocklist of forbidden column names was the earlier shape here and
did not hold: a side table carrying the raw bytes passed it, the projection
render, and the generic debug guard together.

## `egress_audit.grant_id` is polymorphic, and nothing discriminates it

`egress_audit.grant_id` carries no foreign key to `egress_grant`, and that is
deliberate: of the seven `insert_audit` call sites, four write a **process
capability token id** into that column and three write an **egress grant id**.
Restoring the foreign key would make `P2-G7`'s process-activity rows fail at
INSERT. The two identifiers are SHA-256 values under different domain separators
(`academic-process-capability-v1 ` and `academic-egress-grant-v1 `), so a
collision is not the risk.

The risk is that a reader cannot tell which namespace a row's `grant_id` belongs
to. `T146` measured whether the typed `(process_class, capability)` pair
discriminates them and found it does not: `EGRESS_PROXY` x
`OPEN_OUTBOUND_SOCKET` is the cell where the two namespaces overlap exactly, and
it is the cell egress auditing cares most about. Three consecutive allow rows
with identical decision, class and capability carried `grant_id` values from both
namespaces in the same 64-hex shape, and `PermissionBroker::grant_row` returns
`None` for the process-token ones — so to a reader treating the column as an
`egress_grant` reference, those rows look dangling.

No dangling row exists today: all seven call sites write an identifier that does
exist in one of the two tables. What is unresolved is the discrimination, and
the fix is a discriminator column (or two columns), **not** the foreign key.
It starts mattering at `P2-M1`, whose acceptance evidence includes
`transmitted_ranges_reconcile_with_egress_audit`: that reconciliation has to key
on `egress_audit.grant_id`, and the key is polymorphic with nothing to resolve
the polymorphism. Severity P3 until then.

## Schema allocation discrepancy

The execution plan reserves canonical-store migration `0005` for P2-G1. On the
P2-K6 baseline, `migrations/store/0005_phase2_descriptor_migration.sql` is
already applied by P2-K5 and included in the encrypted store fingerprint.
P2-G1 does not reuse that number or silently renumber another owner's canonical
migration. The operational grant/audit schema is instead embedded and applied
by `academic-policy`; integration can assign a canonical-store migration only
after resolving the allocation conflict.

## Provider-policy registry integration

P2-G3 adds versioned provider facts, explicit provider user-policy rows, and
deletion-receipt rows to the same operational store without changing the
fifteen grant columns, audit reason enum, opaque token, or atomic consumption
path fixed here. Grant issuance now requires the rule's provider snapshot and
retention pins to resolve through that registry. Provider TTL caps the stored
grant expiry; runtime use also rejects a provider snapshot that has changed
since issuance.

Historical replay uses the original evaluation time and registry revision
ceilings. The record schema, digest encoding, `(vendor_id, surface)` identity,
TTL behavior, decision-storage boundary, and receipt links are fixed in
[the provider registry contract](provider-registry.md).
