# Phase 0 security and dependency baseline

## Enforced now

- Product crates contain no HTTP client, socket, recorder, database, cloud SDK, shell execution, or telemetry dependency.
- The only fixture is visibly synthetic and declares no network egress.
- Canonical acceptance requires canonical CBOR, an independent expected key/device/user authorization, a valid signature, a contiguous device origin chain, registered scope/domain closure, and exact evidence-representation closure. Ledger append accepts only the opaque verifier capability.
- UUID ordering is never treated as causality; local acceptance sequence is explicit.
- Artifact identity is `sha256:<plaintext digest>` inside the logical contract, while the physical locator is an HMAC-derived, domain-keyed value.
- Normalized repository evidence paths reject POSIX roots, Windows drive/UNC/device forms, URI-like prefixes, backslashes, controls, empty components, and parent traversal on every host platform.
- `Cargo.lock` and `pnpm-lock.yaml` are committed. Git Cargo sources, pnpm SSH dependencies, and insecure tarballs fail `pnpm security`.
- Rust forbids unsafe code and denies clippy warnings. CI actions use full commit SHAs and minimum read-only repository permissions.

## Dependency admission

Every dependency needs a named owning crate/package, a reviewed feature set, an SPDX-compatible license, an upstream advisory path, and a patch expectation. Cryptography is delegated to maintained libraries; the repository does not implement Ed25519, SHA-256, or HMAC primitives. Runtime networking remains forbidden until ADR-006 defines and proves the broker boundary.

Automated advisory services can change independently of source. CI's deterministic baseline therefore checks locked sources on every change; scheduled dependency/advisory review remains a repository-owner operation and must record exploitability decisions rather than suppressing findings silently.

## Not yet provided

Encrypted transactional storage, SQLCipher packaging, WAL/temp plaintext leakage tests, OS keystore integration, key recovery/rotation, daemon single-writer IPC, process sandboxing, native capability enforcement, backup/restore, secure deletion, and egress audit are not implemented. Their ADRs explicitly prevent production use from being inferred from Phase 0 tests.
