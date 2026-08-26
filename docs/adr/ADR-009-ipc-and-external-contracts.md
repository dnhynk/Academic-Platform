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

Versioned Proto schemas, three JSON Schemas, the deterministic CBOR profile, v1/v2 golden fixtures, Rust encode/sign/verify/replay, and exact TypeScript fixture validation are present. The original `signed-batch-v1.json` bytes are frozen at SHA-256 `287F7DEA8FD24C3C6EB205C3F1E2873F6AFDF7D6532FE7BE4FCCFB44A0B7E163`; the v1 Proto `UserDecision` field layout is unchanged. A v1 payload is signature- and canonicality-checked in its original representation, then deterministically upcast from the immutable same-batch target claim into v2 resolution-slot, object, and valid-time semantics. A v1 decision without that immutable same-batch target or with a mismatched legacy scope fails closed. New writes and the repaired fixture use event schema v2 and its matching Proto/JSON contracts.

Both Proto versions carry lossless `ClaimRelation` scope plus structured user/engine/model/importer provenance. The hand-written Prost `OriginEvent` recognizes every current payload tag 10–15, so the final known oneof arm wins; Rust returns `UnsupportedPayload` when it is not `ClaimRelated`, and Rust/Protobuf.js execute every competing field-order permutation for both declared Proto versions. Ajv 8.17.1 validates Draft 2020-12 syntax, while an exported TypeScript artifact semantic validator enforces cross-field invariants JSON Schema cannot express. Ajv, TypeScript, and Rust execute one committed mutation corpus covering safe integers, ranges, bounds, digest closure, locator identity, path grammar, and positive text/page/time/repository descriptors. Canonical signed decode requires byte-for-byte re-encoding, reconstructs constrained values through checked deserializers, and rejects trailing/noncanonical data.

`prost` 0.14.1 (Apache-2.0) is owned by `academic-contracts` for local Protobuf wire conformance; default features are disabled and only `derive` plus `std` are enabled. `ajv` 8.17.1 (MIT) and `protobufjs` 8.7.2 (BSD-3-Clause) are owned by root contract tooling and are development-only: Ajv validates JSON Schema and protobuf.js parses/resolves the committed Proto contract. All use pinned direct versions and lockfile-resolved transitive graphs; RustSec/crates.io and npm/GitHub advisory channels apply, and applicable advisories require a prompt lockfile update or recorded exploitability decision. None adds product runtime networking or a feature outside contract verification.

Event/Proto v2 carries explicit semantic resolution-slot, target-object, and decision-validity fields rather than inferring them from a transient claim ID. The v1 reader performs the one deterministic compatibility inference described above; the v1 writer exists only as a lossless golden-byte test helper and rejects non-representable v2 decisions.

## Acceptance gates

Kotlin and Swift golden decoding; unknown-field preservation rules; N-1 client compatibility; length-prefixed IPC framing; decompression/size/depth limits; malformed and fuzz corpus; and generated bindings for all payload types. Phase 0 already executes Rust actor/relation wire round trips plus declarative schema drift checks.
