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

The Proto schema, two JSON Schemas, deterministic CBOR profile, committed golden fixture, Rust encode/sign/verify/replay, and exact TypeScript fixture validation are present. Proto carries lossless `ClaimRelation` scope plus structured user/engine/model/importer provenance; a Prost wire round-trip and schema drift assertions are executable. Ajv 8.17.1 validates the committed fixture and artifact example against Draft 2020-12, while positive/negative parity cases cover consts, minima, nonempty strings, uniqueness, and additional properties in Schema, TypeScript, and Rust. Canonical signed decode requires byte-for-byte re-encoding, reconstructs constrained values through checked deserializers, and rejects trailing/noncanonical data.

`prost` 0.14.1 (Apache-2.0) is owned by `academic-contracts` for local Protobuf wire conformance; default features are disabled and only `derive` plus `std` are enabled. `ajv` 8.17.1 (MIT) and `protobufjs` 8.7.2 (BSD-3-Clause) are owned by root contract tooling and are development-only: Ajv validates JSON Schema and protobuf.js parses/resolves the committed Proto contract. All use pinned direct versions and lockfile-resolved transitive graphs; RustSec/crates.io and npm/GitHub advisory channels apply, and applicable advisories require a prompt lockfile update or recorded exploitability decision. None adds product runtime networking or a feature outside contract verification.

## Acceptance gates

Kotlin and Swift golden decoding; unknown-field preservation rules; N-1 client compatibility; length-prefixed IPC framing; decompression/size/depth limits; malformed and fuzz corpus; and generated bindings for all payload types. Phase 0 already executes Rust actor/relation wire round trips plus declarative schema drift checks.
