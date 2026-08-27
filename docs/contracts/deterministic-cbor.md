# Deterministic signed batch envelope v1

## Purpose

Signed historical bytes must never be rewritten during schema migration. Phase 0 therefore defines one deterministic envelope representation before introducing storage or sync. Event payload schemas v1 and v2 share that envelope; schema version is authenticated inside the signed payload.

## Envelope

The envelope is a definite-length CBOR array:

```text
[envelope_version, deterministic_payload_bytes, ed25519_public_key, ed25519_signature]
```

Version 1 signs `deterministic_payload_bytes` directly with Ed25519. Verification requires a valid signature and an independent authorization binding the expected public key, `device_id`, and user identity; trusting envelope or fixture-wrapper metadata is forbidden. The envelope hash is SHA-256 over the complete canonical envelope bytes and is the device-chain link used by the ledger.

## Payload profile

The Rust domain object is first represented as JSON-shaped values without floating-point numbers. That value is encoded into CBOR using tagged, definite arrays:

| Tag | Meaning | Array shape |
|---:|---|---|
| 0 | null | `[0]` |
| 1 | boolean | `[1, value]` |
| 2 | JSON integer number | `[2, canonical decimal text]` |
| 3 | text | `[3, text]` |
| 4 | array | `[4, [values...]]` |
| 5 | object | `[5, [[utf8-sorted-key, value]...]]` |

Objects are arrays of pairs, never CBOR maps, so runtime map iteration cannot affect bytes. Keys are strictly increasing by UTF-8 bytes. Integers, booleans, text, arrays, and byte strings use the shortest encoding produced by the pinned encoder. Indefinite lengths, floats, duplicate keys, trailing bytes, unknown tags, and a decode/re-encode byte mismatch fail closed.

Domain `Decimal` values are JSON-shaped objects whose `coefficient` is the canonical base-10 i128 string and whose `scale` is an integer from 0 through 18. This keeps i128 minimum/maximum values lossless without relying on JSON number range and matches the Protobuf `DecimalValue` representation. Leading zeroes, a plus sign, `-0`, whitespace, overflow, number-valued coefficients, and excess scale fail closed.

## Verification order

1. Parse exactly one envelope value and reject trailing bytes.
2. Enforce version and key/signature lengths.
3. Re-encode and require byte equality.
4. Decode the generic payload shape, reject trailing/noncanonical payload bytes, and bind the embedded key to independent device authorization.
5. Verify Ed25519 over exact original payload bytes before compatibility or typed semantic interpretation.
6. Select the authenticated source schema, perform the constrained v1 compatibility transform when required, and validate domain and batch invariants.
7. Re-encode the validated typed result through the exact authenticated source version and require byte equality, rejecting every signed field or representation that typed decoding would discard or normalize.
8. Match the decoded `device_id` and every user actor identity to the independent device authorization, then issue an opaque verified capability; only that capability may enter ledger acceptance.
9. Check origin gap/fork, registered scope, domain, and exact evidence-representation closure, then assign local `accept_seq` atomically.

The committed JSON wrapper stores lowercase hex only for source-control portability. JSON is not signed and must not be used as an alternate signing representation.

## Event-schema compatibility

`schemas/fixtures/signed-batch-v1.json`, its JSON Schema, and its Proto are immutable at SHA-256 `287F7DEA8FD24C3C6EB205C3F1E2873F6AFDF7D6532FE7BE4FCCFB44A0B7E163`, `9588EE9B439C9DBCF864A8F07BD64BD6353ECC8F1D46151348C9B3283B36E6BD`, and `8BC58C574E0BEC84F6BC3D6BB3A7E006E45DE69B1793C407BBCAC57FD29C507A`. Their signed bytes, declared shapes, and v1 Proto `UserDecision` wire shape are never regenerated into the v2 shape. The reader first completes envelope/payload canonicality, independent key binding, and signature checks over the original bytes, then deterministically upcasts a v1 decision by looking up its target claim in the same authenticated batch. It requires the legacy decision scope to equal the target scope and derives the v2 subject/predicate/scope slot, target object, and valid interval from that claim; missing targets and mismatches fail closed.

Event schema v2 is the only writer profile and is represented by `signed-batch-v2.json`, the v2 fixture JSON Schema, and `schemas/proto/academic/v2/ledger.proto`. The contracts crate validates the schema version decoded from the exact returned writer bytes, inventories every Rust source module, and pins every public function and inherent-method signature. Production-facing contracts/core APIs and the CLI expose no v1 signing or emission path. The reader's private v1 projection exists only for final source-aware byte equality, and core accepts the committed v1 fixture as exact frozen-golden verification rather than regenerating it. Every identifier decoded from signed CBOR or Protobuf must be an RFC-variant UUIDv7, matching JSON Schema and TypeScript.
