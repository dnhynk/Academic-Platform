# Deterministic signed batch profile v1

## Purpose

Signed historical bytes must never be rewritten during schema migration. Phase 0 therefore defines one deterministic representation before introducing storage or sync.

## Envelope

The envelope is a definite-length CBOR array:

```text
[envelope_version, deterministic_payload_bytes, ed25519_public_key, ed25519_signature]
```

Version 1 signs `deterministic_payload_bytes` directly with Ed25519. Verification requires both a valid signature and an independently expected device public key; trusting only the embedded key is forbidden. The envelope hash is SHA-256 over the complete canonical envelope bytes and is the device-chain link used by the ledger.

## Payload profile

The Rust domain object is first represented as JSON-shaped values without floating-point numbers. That value is encoded into CBOR using tagged, definite arrays:

| Tag | Meaning | Array shape |
|---:|---|---|
| 0 | null | `[0]` |
| 1 | boolean | `[1, value]` |
| 2 | integer number | `[2, canonical decimal text]` |
| 3 | text | `[3, text]` |
| 4 | array | `[4, [values...]]` |
| 5 | object | `[5, [[utf8-sorted-key, value]...]]` |

Objects are arrays of pairs, never CBOR maps, so runtime map iteration cannot affect bytes. Keys are strictly increasing by UTF-8 bytes. Integers, booleans, text, arrays, and byte strings use the shortest encoding produced by the pinned encoder. Indefinite lengths, floats, duplicate keys, trailing bytes, unknown tags, and a decode/re-encode byte mismatch fail closed.

## Verification order

1. Parse exactly one envelope value and reject trailing bytes.
2. Enforce version and key/signature lengths.
3. Re-encode and require byte equality.
4. Match the embedded key to the independently expected device key.
5. Verify Ed25519 over exact payload bytes.
6. Decode the payload, validate domain and batch invariants, then re-encode and require byte equality.
7. Check origin gap/fork/evidence closure and assign local `accept_seq` atomically.

The committed JSON wrapper stores lowercase hex only for source-control portability. JSON is not signed and must not be used as an alternate signing representation.
