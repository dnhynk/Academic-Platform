# ADR-011: Monorepo, toolchain, CI, and release

- Status: Accepted baseline; release acceptance remains open

## Decision

Use one Cargo workspace and one pnpm workspace without Nx, Bazel, Docker, or a remote cache. Rust 1.98.0 with rustfmt/clippy, Node 24.19.0, and pnpm 11.22.0 are exact pins. `Cargo.lock` and `pnpm-lock.yaml` are committed. Cross-platform workflows are Cargo, pnpm, and small Node scripts rather than a PowerShell/bash-only build graph.

Windows native and Linux are required CI environments for core tests and fixture replay. Frontend/contract CI runs frozen install, lint, typecheck, test, build, schema/semantic parity checks, and the offline dependency-source baseline. The pnpm gate parses lock YAML structurally and traverses importers, packages, snapshots, and every top-level default/named catalog. Scalar entries and object `specifier`/`version` forms reject Git source types/repositories/schemes/hosts/hosted shorthands plus insecure HTTP tarballs independent of quoting or flow/block serialization; malformed catalog mappings and entries fail closed. Its allow/reject fixtures retain registry, workspace, link, and ordinary local-file cases. GitHub Actions use full commit SHAs, `contents: read`, bounded timeouts, and no credential persistence.

## Implemented evidence

Workspace manifests, locks, line-ending/format rules, doctor/bootstrap scripts, full locked commands, and Windows/Linux CI are present. CI hash-pins and verifies/replays the byte-immutable v1 fixture, v1 JSON Schema, and v1 Proto through the deterministic reader, deterministically emits/diffs/verifies/replays the current v2 fixture, validates committed JSON through raw lexical checks, Draft 2020-12 schemas, and semantic parity, cross-checks Rust/Protobuf.js wire tags and oneof order for both Proto versions, executes structural pnpm source negatives including catalogs, and hashes the exact LF canonical-spec bytes. Docker is not used. The repository has no remote, and Phase 0 does not publish an artifact.

`yaml` 2.8.1 (ISC) is pinned and owned by root security tooling solely to parse `pnpm-lock.yaml`; it is development-only and adds no product runtime networking. npm/GitHub advisories and upstream releases are its advisory path, with lockfile update or a recorded exploitability decision required for applicable findings.

## Acceptance gates

Fresh Windows one-command bootstrap rehearsal; architecture-specific native dependency matrix when storage/UI arrive; SBOM and license/advisory policy; signed installer and OS code signing; updater signature negative test; protected release environment; provenance; and secret/capability/plaintext-canary CI.
