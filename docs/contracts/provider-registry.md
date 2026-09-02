# Provider registry contract

`academic-policy` owns the P2-G3 provider registry beside the P2-G1 broker.
It records reviewed facts and explicit user policy; it performs no provider call,
opens no socket, and does not choose a provider or cloud fallback.

## Identity and versioning

A provider identity key is the ordered pair `(vendor_id, surface)`, where
`surface` is exactly `ENTERPRISE_API` or `CONSUMER_UI`. The broker destination is
`provider:` followed by lowercase SHA-256 of this canonical encoding:

1. `academic-provider-identity-v1\0`;
2. the vendor ID as an eight-byte big-endian byte length followed by UTF-8; and
3. the surface spelling encoded the same way.

Every query and foreign-key link uses the resulting destination plus the
snapshot digest. There is no vendor-only lookup in grant evaluation. Thus two
surfaces from one vendor occupy separate identities and policy histories;
`same_vendor_two_surfaces_are_distinct_records` crosses their destination and
snapshot deliberately and observes `PROVIDER_POLICY_INCOMPATIBLE`.

`provider_policy_snapshot` carries these scalar facts, with the two set-valued
facts in append-only child tables:

- training use enabled and effective opt-out;
- maximum server retention and abuse logging;
- a non-empty region/residency set and an explicitly present subprocessor set
  (which may be empty);
- transit- and at-rest-encryption declarations;
- deletion API availability and deletion-receipt capability;
- the enterprise/API or consumer-UI surface through the identity key;
- maximum input bytes and the exact logging-configuration identifier;
- reviewed source-policy digest, last verification time, and an explicit TTL.

`ProviderPolicyDraft` uses `Option` for every fact solely to distinguish an
omitted fact from a declared false, zero, or empty value. Registration rejects
each omission. It also rejects an empty residency set, zero maximum input,
zero TTL, an overflowing freshness boundary, or receipt capability without a
deletion API.

## Snapshot digest

The snapshot digest is lowercase SHA-256 of the following canonical bytes in
order:

1. `academic-provider-policy-snapshot-v1\0`;
2. vendor ID and surface, each encoded as an eight-byte big-endian byte length
   plus UTF-8;
3. training-enabled byte, opt-out-applied byte, server-retention `u64`, and
   abuse-logging byte;
4. sorted/deduplicated residency strings, then sorted/deduplicated subprocessor
   strings; each set starts with an eight-byte big-endian count and each string
   uses the length-plus-UTF-8 encoding;
5. transit encryption, at-rest encryption, deletion API, and receipt-capable
   bytes;
6. maximum-input `u64`, logging configuration, and source-policy digest;
7. last-verified `u64` and TTL `u64`.

All integers are big-endian and each Boolean is one byte (`0` or `1`).
`registered_at` is transaction history rather than a provider fact and is not
hashed. Re-registering the exact digest is idempotent and does not append a new
version or move its verification boundary. A later verification necessarily
changes `last_verified_at`, hence the digest.

The retention terms pinned by the existing P2-G1 rule remain a separate digest:
`academic-provider-retention-terms-v1\0`, server-retention `u64`, abuse-logging
byte, and the length-prefixed logging configuration. This uses the existing
grant column rather than adding a parallel grant format.

## Grant interaction and TTL

Grant issuance validates the P2-G1 rule against the provider version in force
at `issued_at`: exact destination and current snapshot digest, retention digest,
effective training use, and maximum input size. Explicit user constraints, when
present, also check the full processing-region set and requested encryption
declarations. Runtime consumption repeats provider validation before the
existing atomic single-use update.

The provider freshness boundary is exclusive:

```text
provider_verified_until = last_verified_at + ttl_millis
grant.expires_at = min(issued_at + broker_grant_ttl, provider_verified_until)
```

An identical registration does not change either operand. A re-verification is
a new snapshot digest, so an old rule cannot mint another grant and an
unconsumed old capability is rejected if the provider policy changes before its
stored expiry. `stale_policy_does_not_auto_renew_grant` exercises the tempting
identical-registration refresh path and observes the original `expires_at` plus
`GRANT_EXPIRED`. `changed_policy_hash_invalidates_future_grants` changes the
subprocessor set and observes both issuance denial and zero runtime-tool calls.

Decision replay retains the original evaluation time plus the then-visible
provider-snapshot and user-policy sequence ceilings. A later user decision,
even one labelled with the same time, therefore does not rewrite an older
decision receipt.

## Facts versus user decisions

The provider snapshot records facts; it does not decide which residency or
encryption posture the user accepts. `provider_user_policy` stores a separately
identified, evidence-linked user decision for one exact provider snapshot. No
such row is synthesized on a new profile.

A provider without a deletion API receives `NO_DELETION_RECEIPT` unless a user
policy for that exact snapshot explicitly sets
`allow_without_deletion_api=true`. The allowed residency set and encryption
requirements in that row are also explicit inputs rather than registry
defaults. User-policy rows and their residency rows are append-only.

`GATE-38-010` stays open: only an explicit P2-G1 per-tuple rule supplies an
external-AI egress preference. `GATE-38-028` also stays open: this registry has
no quality heuristic and supplies no cloud-fallback default.

## Deletion receipts

`provider_deletion_receipt` stores only receipt identifiers, digests, and times;
it does not store receipt or provider-response bytes. Each row has composite
foreign-key links to the exact grant/provider-policy snapshot and to the exact
runtime-consumption allow audit row for that grant. The broker appends a narrow
`egress_consumption` link in the same transaction as the existing grant
single-use update and runtime audit; this distinguishes that audit from an
issuance or replay audit even when their timestamps are equal. The stored
consumption time must also equal the grant's `consumed_at` and the audit start.
A deletion request timestamp before that audit finishes is rejected. The
schema installs update and delete guards on receipts, consumption links,
grants, audits, and provider-policy records.

`deletion_receipt_is_immutable_and_linked` first injects the issuance audit as a
wrong parent, then stores and reads the receipt, injects a conflicting duplicate,
and injects SQL `UPDATE` and `DELETE` attempts. Those mutations fail while the
original runtime-audit and grant links remain readable.

## Scope boundary and specification discrepancy

Execution-plan P2-G3 says it closes all of `REQ-32-048` through `REQ-32-057`.
The canonical specification's `REQ-32-056` requires a complete provider input,
output, retention, and disclosure data flow, whose acceptance evidence executes
a model call. P2-G3 is explicitly socket-free and records facts rather than
calling a provider, so this implementation supplies the registry facts needed
by that flow but does not claim the model-call/data-flow requirement complete.
That integration remains with the later egress/model-run work. For the same
reason, this registry supplies the policy boundary for `REQ-33-022` but does not
claim a cloud-drive connector exists.
