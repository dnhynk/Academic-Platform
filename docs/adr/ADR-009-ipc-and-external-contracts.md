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

Versioned Proto schemas, three JSON Schemas, the deterministic CBOR profile, v1/v2 golden fixtures, Rust v2 encode/sign plus v1/v2 verify/replay, and exact TypeScript fixture validation are present. The original `signed-batch-v1.json`, v1 fixture JSON Schema, and v1 Proto bytes are frozen respectively at SHA-256 `287F7DEA8FD24C3C6EB205C3F1E2873F6AFDF7D6532FE7BE4FCCFB44A0B7E163`, `9588EE9B439C9DBCF864A8F07BD64BD6353ECC8F1D46151348C9B3283B36E6BD`, and `8BC58C574E0BEC84F6BC3D6BB3A7E006E45DE69B1793C407BBCAC57FD29C507A`. A v1 payload is signature- and canonicality-checked in its original representation, then deterministically upcast from the immutable same-batch target claim into v2 resolution-slot, object, and valid-time semantics. A v1 decision without that immutable same-batch target or with a mismatched legacy scope fails closed. New writes and fixture emission are v2-only; no public contracts/core API or CLI option can sign or emit v1.

Both Proto versions carry lossless `ClaimRelation` scope plus structured user/engine/model/importer provenance. The hand-written Prost `OriginEvent` recognizes every current payload tag 10–15, so the final known oneof arm wins; Rust returns `UnsupportedPayload` when it is not `ClaimRelated`, and Rust/Protobuf.js execute every competing field-order permutation for both declared Proto versions. Contract verification reflects both declared Proto versions and cross-checks every represented hand-written Rust field's exact scalar/message/enum type, Rust payload type, cardinality/presence, oneof membership, and tag. Scalar, cardinality, payload-type, oneof-type, and tag mutations fail the official gate, and Rust plus Protobuf.js v1/v2 emit one fixed relation-event byte golden.

The current v2 `ClaimRelationKind` set is exactly `UNSPECIFIED`, `SUPPORTS`, `CONTRADICTS`, `SUPERSEDES`, `RETRACTS`, and `DUPLICATES` at discriminants 0–5. The official gate checks exact Proto and Rust membership, both hand-written Rust conversion directions, and in-memory mapping and added-discriminant mutations; Rust execution round-trips all five domain kinds.

Ajv 8.17.1 validates Draft 2020-12 syntax, while an exported TypeScript artifact semantic validator enforces cross-field invariants JSON Schema cannot express. Artifact JSON has one recursive raw policy before parsing: decoded object names are unique, every string is a Unicode scalar sequence, and every number token matches `0|[1-9][0-9]*`. Duplicate/escaped-equivalent names, lone surrogates, decimal, exponent, negative, unsafe, out-of-range, and fractional cases execute as exact raw texts through the gate plus Ajv, TypeScript, and Rust. Typed range and cross-field checks still follow lexical validation, so ambiguous or nonportable raw input is never silently normalized.

Fixture wrapper version fields follow JSON Schema's semantic `integer` boundary. Rust uses bounded integral-number deserializers for `fixture_version` and `event_schema_version`, accepting equivalent integral decimal/exponent lexemes such as `2.0` and `2e0` while retaining exact supported-version validation and rejecting fractions, negatives, and values above `u16`.

Every `EvidenceLocator` variant is an exact closed object in JSON Schema, TypeScript raw validation, and Rust Serde. Undeclared page, text-byte, transcript-time, or repository-byte coordinates fail before an `ArtifactDescriptor` can cross the common contract boundary.

Canonical signed verification first enforces exact envelope and generic payload canonicality plus independent key binding, then verifies Ed25519 over the original payload bytes, performs source-aware v1 compatibility, validates the typed batch, and finally re-encodes through the authenticated source version before issuing `VerifiedBatch`. Unknown authenticated fields at event, actor, payload, claim/object, decision/action, and version-specific nested levels fail closed for both v1 and v2 instead of disappearing during Serde conversion.

`prost` 0.14.1 (Apache-2.0) is owned by `academic-contracts` for local Protobuf wire conformance; default features are disabled and only `derive` plus `std` are enabled. `ajv` 8.17.1 (MIT) and `protobufjs` 8.7.2 (BSD-3-Clause) are owned by root contract tooling and are development-only: Ajv validates JSON Schema and protobuf.js parses/resolves the committed Proto contract. All use pinned direct versions and lockfile-resolved transitive graphs; RustSec/crates.io and npm/GitHub advisory channels apply, and applicable advisories require a prompt lockfile update or recorded exploitability decision. None adds product runtime networking or a feature outside contract verification.

Event/Proto v2 carries explicit semantic resolution-slot, target-object, and decision-validity fields rather than inferring them from a transient claim ID. The v1 reader performs the one deterministic compatibility inference described above. Its private source-version projection exists only to compare authenticated input with the typed result; it is not a signer or caller-facing encoder. The frozen v1 fixture is checked against its committed exact document rather than regenerated.

## Acceptance gates

Kotlin and Swift golden decoding; general Protobuf unknown-field preservation rules; N-1 client compatibility; length-prefixed IPC framing; decompression/size/depth limits; malformed and fuzz corpus; and generated bindings for all payload types. Signed deterministic-CBOR authenticated-field discard is already fail-closed. Phase 0 executes Rust actor/relation wire round trips plus declarative schema drift checks.
