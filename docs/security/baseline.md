# Phase 0 security and dependency baseline

## Enforced now

- Product crates contain no HTTP client, socket, recorder, database, cloud SDK, shell execution, or telemetry dependency.
- Both v1 compatibility and current v2 fixtures are visibly synthetic and declare no network egress; the v1 fixture, v1 JSON Schema, and v1 Proto have pinned immutable byte hashes, and only v2 has a signing/emission path. The legacy projection requires a private verification capability, is absent from the exact public-API allowlist, and is unreachable from current writers.
- Canonical acceptance requires canonical envelope and payload CBOR, an independent expected key/device/user authorization, a valid signature over original bytes, source-aware typed byte equality, a contiguous device origin chain, registered scope/domain closure, and exact evidence-representation closure. Ledger append accepts only the opaque verifier capability.
- UUID ordering is never treated as causality; local acceptance sequence is explicit.
- Artifact identity is `sha256:<plaintext digest>` inside the logical contract, while the physical locator is an HMAC-derived, domain-keyed value.
- Normalized repository evidence paths reject POSIX roots, Windows drive/UNC/device forms, URI-like prefixes, backslashes, controls, empty components, and parent traversal on every host platform.
- `Cargo.lock` and `pnpm-lock.yaml` are committed. Before any fetch/install/build/test, dependency-free checked-in TOML/YAML parsers structurally inspect Cargo package sources and pnpm importers, packages, snapshots, and default/named catalogs. Cargo Git sources, malformed/duplicate lock structures, YAML node properties on block/flow keys, malformed dependency objects, pnpm Git source types/repository fields/SSH/HTTPS/Git/file forms/hosted shorthands, and insecure HTTP tarballs are rejected independent of legal quote, spacing, multiline, block, or flow serialization. Plain YAML null/boolean, decimal/binary/octal/hex integer, float, date, and timestamp kinds remain typed and therefore reject in direct dependency, `specifier`, and `version` string positions; the shared block/flow matrix proves explicitly quoted lookalikes remain valid strings. Registry, workspace/link, pnpm v11 object forms, and ordinary local-file dependencies remain allowed. Committed Cargo and pnpm allow/reject corpora plus a real frozen/offline pnpm consumption regression probe the accepted and denied encodings.
- Rust forbids unsafe code and denies clippy warnings. CI actions use full commit SHAs and minimum read-only repository permissions.

## Dependency admission

Every dependency needs a named owning crate/package, a reviewed feature set, an SPDX-compatible license, an upstream advisory path, and a patch expectation. Cryptography is delegated to maintained libraries; the repository does not implement Ed25519, SHA-256, or HMAC primitives. The source preflight intentionally has no installed dependency, so it remains available before dependency materialization. npm/GitHub and RustSec/crates.io advisories and upstream releases are monitored, and applicable findings require a prompt pinned update or recorded exploitability decision. Runtime networking remains forbidden until ADR-006 defines and proves the broker boundary.

Automated advisory services can change independently of source. CI's deterministic baseline therefore checks locked sources on every change; scheduled dependency/advisory review remains a repository-owner operation and must record exploitability decisions rather than suppressing findings silently.

## Not yet provided

Encrypted transactional storage, SQLCipher packaging, WAL/temp plaintext leakage tests, OS keystore integration, key recovery/rotation, daemon single-writer IPC, process sandboxing, native capability enforcement, backup/restore, secure deletion, and egress audit are not implemented. Their ADRs explicitly prevent production use from being inferred from Phase 0 tests.
