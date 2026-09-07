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

Versioned Proto schemas, versioned JSON Schemas, the deterministic CBOR profile, v1/v2/v3/v4 golden fixtures, Rust v4 encode/sign plus v1/v2/v3/v4 verify/replay, and exact TypeScript fixture validation are present. The original `signed-batch-v1.json`, v1 fixture JSON Schema, and v1 Proto bytes are frozen respectively at SHA-256 `287F7DEA8FD24C3C6EB205C3F1E2873F6AFDF7D6532FE7BE4FCCFB44A0B7E163`, `9588EE9B439C9DBCF864A8F07BD64BD6353ECC8F1D46151348C9B3283B36E6BD`, and `8BC58C574E0BEC84F6BC3D6BB3A7E006E45DE69B1793C407BBCAC57FD29C507A`. A v1 payload is signature- and canonicality-checked in its original representation, then deterministically upcast from the immutable same-batch target claim into v2 resolution-slot, object, and valid-time semantics. A v1 decision without that immutable same-batch target or with a mismatched legacy scope fails closed. Before the v4 addition below, new writes and fixture emission were v3-only; the writer decodes and checks the schema version of its exact returned bytes, and no public contracts/core API or CLI option can sign or emit v1 or v2.

Event schema v3 adds eighteen `OriginEvent.payload` arms at Proto tags 16..=33 over the frozen 10..=15, with 6..=9 still reserved: `CURRICULUM_VERSION_PUBLISHED`, `COURSE_REVISION_PUBLISHED`, `OFFERING_OBSERVED`, `ATTEMPT_RECORDED`, `REQUIREMENT_SET_PUBLISHED`, `AUDIT_COMPUTED`, `CAPTURE_PERMISSION_RECORDED`, `LECTURE_SESSION_RECORDED`, `TRANSCRIPT_VERSION_ADDED`, `LECTURE_DOCUMENT_PUBLISHED`, `SNAPSHOT_REGISTERED`, `FINDING_PUBLISHED`, `MODEL_RUN_RECORDED`, `PROPOSAL_DISPOSED`, `EGRESS_DECIDED`, `CONSENT_RECORDED`, `ENTITY_IDENTITY_CHANGED`, `RETENTION_ACTION_RECORDED`. Every arm carries the identical registration frame — aggregate identity, the domain equal to its event's domain, its scope, the parent aggregate where one exists, an optional `source_digest`, and `valid_time` — and carries no aggregate attribute. Disputable facts remain `CLAIM_ASSERTED` claims; everything else becomes typed closure-table columns owned by the task that owns that aggregate. `signed-batch-v2.json` joined `signed-batch-v1.json` as a read-only compatibility golden, frozen at SHA-256 `F94DFCF7E3E376E54B5514CEB3016B0B7D97D17366562F7AC4A16286D3AA367D`, and each legacy source version owns a private projection that only authenticated read verification can construct. The v2-to-v3 upcast rewrites nothing: v3 is purely additive, so the transform is the guard that rejects a legacy payload carrying a v3 arm. Upcasting is proved pure and deterministic — repeated reads of the same bytes yield one batch, the source bytes are never mutated, and the upcast batch reprojects to its exact historical bytes. Store acceptance refuses a v3 arm with a typed error before any SQL runs, because no acceptance path writes their closure rows yet and, on a schema-1 profile, `ledger_event.event_kind` is a closed CHECK over the v1/v2 arms that only migration `0004` widens; the persisted `event_schema_version` column is the version a batch was authenticated as, not the version reading it upcasts to.

All three Proto versions carry lossless `ClaimRelation` scope plus structured user/engine/model/importer provenance. The hand-written Prost `OriginEvent` recognizes every payload tag any declared version emits, 10–33, so the final known oneof arm wins; Rust returns `UnsupportedPayload` when it is not `ClaimRelated`, and Rust/Protobuf.js execute every competing field-order permutation for every declared Proto version. Contract verification reflects all three declared Proto versions and cross-checks every represented hand-written Rust field's exact scalar/message/enum type, Rust payload type, cardinality/presence, oneof membership, and tag; each version's own oneof tag list must be exactly the prefix of the emitted superset it declares, so no emitted tag is dropped, reordered, or reused across versions. Rust and protobuf.js independently reproduce a fixed byte golden for each of the eighteen v3 arms, including the arm-selection rule in both directions against a competing legacy arm and the difference between an absent and a present `source_digest`. Scalar, cardinality, payload-type, oneof-type, and tag mutations fail the official gate on either side of the v3 boundary. Rust and protobuf.js independently encode and decode fixed bytes plus selected oneof arms and fields for User, DeterministicEngine, ModelRun, and Importer under both Proto versions; symmetric actor-mapping swaps fail the official gate. Every Rust UUID constructor and the signed-CBOR and Protobuf decode boundaries require both the RFC variant and version seven; NCS, Microsoft, and future-variant negatives are also rejected by both fixture JSON Schemas and TypeScript. The relation event retains a separate fixed cross-runtime byte golden.

The current v2 `ClaimRelationKind` set is exactly `UNSPECIFIED`, `SUPPORTS`, `CONTRADICTS`, `SUPERSEDES`, `RETRACTS`, and `DUPLICATES` at discriminants 0–5. The official gate checks exact Proto and Rust membership, both hand-written Rust conversion directions, and in-memory mapping and added-discriminant mutations; Rust execution round-trips all five domain kinds.

Ajv 8.17.1 validates Draft 2020-12 syntax, while an exported TypeScript artifact semantic validator enforces cross-field invariants JSON Schema cannot express. Artifact JSON has one recursive raw policy before parsing: decoded object names are unique, every string is a Unicode scalar sequence, and every number token matches `0|[1-9][0-9]*`. Duplicate/escaped-equivalent names, lone surrogates, decimal, exponent, negative, unsafe, out-of-range, and fractional cases execute as exact raw texts through the gate plus Ajv, TypeScript, and Rust. Typed range and cross-field checks still follow lexical validation, so ambiguous or nonportable raw input is never silently normalized.

Fixture wrapper ingress consumes original bytes and performs fatal UTF-8 decoding before JSON parsing, Ajv, or TypeScript semantics; malformed standalone, truncated, overlong, and surrogate encodings never normalize to U+FFFD. The recursive raw boundary then rejects duplicate decoded property names and non-Unicode-scalar strings without imposing Artifact JSON's canonical-number spelling profile. Before IEEE-754/f64 conversion, both JavaScript and Rust decide mathematical integrality directly from the arbitrary-precision decimal/exponent lexeme for `fixture_version`, `event_schema_version`, `accepted_events`, and `accept_seq_head`. Equivalent integral spellings such as `2.0` and `2e0` remain valid, while nonzero fractional tails that would round to integers fail. Shared v1/v2 byte, exact-integer, and raw corpora run through Rust, fatal decode plus Ajv, and TypeScript semantics.

`ArtifactDescriptor`, `ArtifactRepresentation`, and every `EvidenceLocator` variant are exact closed objects in JSON Schema, TypeScript raw validation, and Rust Serde. Undeclared descriptor, representation, page, text-byte, transcript-time, or repository-byte properties fail before an `ArtifactDescriptor` can cross the common contract boundary.

Canonical signed verification first enforces exact envelope and generic payload canonicality plus independent key binding, then verifies Ed25519 over the original payload bytes, performs source-aware v1 compatibility, validates the typed batch, and finally re-encodes through the authenticated source version before issuing `VerifiedBatch`. Unknown authenticated fields at batch root, event, actor, payload wrapper, claim/object, decision/action, v3 registration record, and version-specific nested levels fail closed for v1, v2, and v3 instead of disappearing during Serde conversion; a dropped authenticated field fails the same way, because the typed re-encode no longer reproduces the source bytes.

`prost` 0.14.1 (Apache-2.0) is owned by `academic-contracts` for local Protobuf wire conformance; default features are disabled and only `derive` plus `std` are enabled. `ajv` 8.17.1 (MIT) and `protobufjs` 8.7.2 (BSD-3-Clause) are owned by root contract tooling and are development-only: Ajv validates JSON Schema and protobuf.js parses/resolves the committed Proto contract. All use pinned direct versions and lockfile-resolved transitive graphs; RustSec/crates.io and npm/GitHub advisory channels apply, and applicable advisories require a prompt lockfile update or recorded exploitability decision. None adds product runtime networking or a feature outside contract verification.

Event/Proto v2 carries explicit semantic resolution-slot, target-object, and decision-validity fields rather than inferring them from a transient claim ID. The v1 reader performs the one deterministic compatibility inference described above. Its private source-version projection requires a private source-equality capability constructed only by authenticated read verification. A lexical/structural Rust scanner masks comments and literals, inventories every contracts-crate source module, reviews public items in both the root and child module, and pins every public function and inherent-method signature token-for-token. It rejects unsupported identifiers, macros/includes, unreviewed source modules, public items/re-exports, and crate-wide impl drift. The exact public API, function-reference-aware writer graph, and semantic v2 check over the returned bytes are enforced together. Downstream compile-fail imports plus executable child-module impl, signature-constness, neutral-transform, rename, direct/function-value/generic writer-data-flow, raw/Unicode name, macro/include, root-export, impl-method, and commented-`cfg(test)` mutations prevent the projection from becoming a signer or caller-facing encoder. The frozen v1 fixture is checked against its committed exact document rather than regenerated.

The current v2 Claim contract assigns tag 11 to versioned Prediction metadata, with a distinct bounded observation-window message and positive sample count. The v2 fixture JSON/TypeScript replay surface exposes the same signed claim's confidence, Prediction metadata, and `valid_time` as separate typed fields. At fixture ingress, Rust, JSON Schema, and TypeScript preserve mathematically integral decimal and exponent spellings while enforcing each typed range, require every Prediction timestamp to be a non-negative JavaScript-safe integer, and require the applicability `to` key while reserving explicit `null` for an open-ended interval. Frozen v1 Proto, JSON Schema, fixture bytes, and their read-only upcast remain unchanged: the optional v1 disclosure key is absent rather than null, and authenticated source-v1 claims carrying v2-only Prediction metadata are rejected before a verified capability can be issued.

## Additive forecast actor in v4

Resolved gate_7e4af1a2ad17 introduces only Actor oneof tag 5,
`DeterministicPredictionActor`, in `academic.v4`. Its four required semantic
fields are engine name (1), engine version (2), frozen-input SHA-256 bytes (3),
and rule-set SHA-256 bytes (4). Existing Actor tags 1–4 and all event payload
and claim tags keep their original meanings. The v4 Proto namespace is compared
against every v3 declaration; only the actor addition may differ.

Current signing and fixture emission are v4-only. The new private v3 projection
has the same read-only source-equality capability as v1/v2; authenticated
historical bytes are upcast without changing a byte or adding actor provenance.
All v1/v2/v3 signed readers explicitly reject the new actor in a historical
source. Existing unversioned Proto relation APIs retain their v1–v3 profile and
reject tag 5 even if a later competing old actor would hide it. Explicit `_v4`
APIs admit it, with required name/version and exact 32-byte digests.

The v4 fixture wrapper, standalone actor schema and TypeScript raw actor parser
are versioned together. Fatal UTF-8, duplicate names, unknown fields, missing
provenance and invalid identities fail closed. Cross-runtime fixed actor bytes,
360 actor/authority/status cells, correctly signed malformed payloads, signed
replay, and the original v1/v2/v3 semantic corpora exercise the boundary. The
frozen v3 fixture SHA-256 is
`d61381ed855768a5a9399a4e7a3b5ea3ce917e25cca192f1d35e4d33eda256b8`.

## Acceptance gates

The durable detail workspace adds local envelope tags 16/17 with one bounded,
canonical, closed typed JSON field and independently negotiated read/decide/audio
capabilities. Versioned `ClaimAsserted` detail predicates bind imported snapshots
to canonical relation claims and append explicit user rejection/undo overlays;
they do not extend the canonical `UserDecision` action set. Existing signed
v1-v4 and old local RPC fixture bytes are unchanged. The source/authority boundary,
host profile incarnation, receipt correlation, deterministic source inventory
update and incomplete ordinary-domain producer/media/preview coverage are defined
in [the detail workspace contract](../contracts/detail-workspace.md). The ordinary
item inventory is regenerated from actual source changes without changing scanner
policy, accepting unreviewed closures or asserting restricted A5 acceptance.
The binary-reference inventory adds only `daemon academic_rpc::details`,
`desktop academic_rpc::details`, and `desktop academic_rpc::generated`, with
reasons for the bounded DTO and frame calls. These references use the existing
RPC dependency and local session; they grant no store, key or network authority.
The exact source inventories also record the closed desktop DTO paths, the
daemon's test-only core fixture feature and process/counter-isolated test roots.
New correlation byte fields are identifiers or digests; audio and signed history
buffers are content and their Debug implementations redact payloads. The
transcript DTO uses the specific Rust name `DetailSegment` and redacts raw and
corrected text. Existing source scanners and discovery floors remain unchanged.

Kotlin and Swift golden decoding; general Protobuf unknown-field preservation rules; N-1 client compatibility; length-prefixed IPC framing; decompression/size/depth limits; malformed and fuzz corpus; and generated bindings for all payload types. Signed deterministic-CBOR authenticated-field discard is already fail-closed. Phase 0 executes Rust actor/relation wire round trips plus declarative schema drift checks.
