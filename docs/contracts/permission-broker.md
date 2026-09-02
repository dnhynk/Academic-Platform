# Permission broker contract

`academic-policy` is the P2-G1 decision and runtime-capability boundary. It has
no network API. It owns an in-memory SQLite operational store containing the
fixed `egress_grant` and `egress_audit` rows from execution-plan §3.5.

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
entries while still calling the tuple “eight-field.” P2-G1 checks all ten
concrete entries before selecting a rule, so neither reading leaves a missing
entry permissive. `missing_tuple_field_denies_and_audits` varies all ten and
observes one `NO_GRANT` audit for each.

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
their SHA-256 at the boundary. Actor, operation, purpose, destination, byte
ranges, payload length, and digest must equal the opaque token and its stored
grant. The atomic SQL update requires `consumed_at IS NULL` and
`expires_at > now`; the tool closure is invoked after that update and its allow
audit commit. The acceptance test races two threads with the same token and
observes exactly one closure invocation. Another test supplies a wider runtime
range and observes `SCOPE_MISMATCH`, a deny audit, and zero closure calls.

## Audit contents

Every evaluation, replay, and runtime use appends one audit row. Denials use
the closed §3.5 reason-code enum. Allows use a null `reason_code`, because that
closed enum contains no allow code; the execution plan lists the column but
does not define its allow-row nullability.

Audit rows contain identifiers, lowercase digests, byte counts, and times.
They contain no raw payload, prompt, or provider response fields. The existing
`tools/secret-debug-policy.test.mjs` source discovery net covers raw fields
named `payload`, `prompt`, and `provider_response`; P2-G1 does not add a second
audit-only scanner. During acceptance, a temporary `AuditRow.payload_bytes:
Vec<u8>` injection first passed the old vocabulary, then failed the extended
generic discovery net, and passed again after the injection was removed. That
investigation also found signed `DecodedEnvelope.payload` buffers one layer
outside the broker; their derived `Debug` implementations were removed.

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
deletion-receipt rows to the same in-memory operational store without changing
the fifteen grant columns, audit reason enum, opaque token, or atomic
consumption path fixed here. Grant issuance now requires the rule's provider
snapshot and retention pins to resolve through that registry. Provider TTL caps
the stored grant expiry; runtime use also rejects a provider snapshot that has
changed since issuance.

Historical replay uses the original evaluation time and registry revision
ceilings. The record schema, digest encoding, `(vendor_id, surface)` identity,
TTL behavior, decision-storage boundary, and receipt links are fixed in
[the provider registry contract](provider-registry.md).
