# ADR-009: IPC and external contracts

- Status: Accepted Phase 0 profile; multi-client gates remain

## Decision

Use one source of truth per boundary:

- Rust newtypes/enums and invariant tests for core domain validity.
- Protobuf for versioned multi-language IPC/sync DTOs; field numbers are never reused and removed fields are reserved.
- deterministic CBOR arrays for signed historical bytes; signed bytes are never JSON-round-tripped or rewritten.
- JSON Schema 2020-12 for human/tool manifests and fixture wrappers.
- ordered SQL migrations for physical storage once ADR-002 is accepted.
- versioned manifest plus referenced original bytes for vendor-independent export.

Protocol handshake reports major/minor, minimum client, storage schema, capabilities, vault read/write formats, projection builders, and lock state. Unknown write versions fail closed; an older binary may expose bounded diagnostics/export only.

## Implemented evidence

The Proto schema, two JSON Schemas, deterministic CBOR profile, committed golden fixture, Rust encode/sign/verify/replay, and TypeScript structural fixture validation are present. Canonical decode requires byte-for-byte re-encoding and rejects trailing/noncanonical data. Signature validation also requires an expected key outside the envelope.

## Acceptance gates

Kotlin and Swift golden decoding; unknown-field preservation rules; N-1 client compatibility; length-prefixed IPC framing; decompression/size/depth limits; malformed and fuzz corpus; DST/leap/Unicode/path fixtures; and generated-code drift checks.
