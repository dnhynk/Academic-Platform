# ADR-011: Monorepo, toolchain, CI, and release

- Status: Accepted baseline; release acceptance remains open

## Decision

Use one Cargo workspace and one pnpm workspace without Nx, Bazel, Docker, or a remote cache. Rust 1.98.0 with rustfmt/clippy, Node 24.19.0, and pnpm 11.22.0 are exact pins. `Cargo.lock` and `pnpm-lock.yaml` are committed. Cross-platform workflows are Cargo, pnpm, and small Node scripts rather than a PowerShell/bash-only build graph.

Windows native and Linux are required CI environments for core tests and fixture replay. Frontend/contract CI runs frozen install, lint, typecheck, test, build, schema checks, and the offline dependency-source baseline. GitHub Actions use full commit SHAs, `contents: read`, bounded timeouts, and no credential persistence.

## Implemented evidence

Workspace manifests, locks, line-ending/format rules, doctor/bootstrap scripts, full required commands, and Windows/Linux CI are present. Docker is not used. The repository has no remote, and Phase 0 does not publish an artifact.

## Acceptance gates

Fresh Windows one-command bootstrap rehearsal; architecture-specific native dependency matrix when storage/UI arrive; SBOM and license/advisory policy; signed installer and OS code signing; updater signature negative test; protected release environment; provenance; and secret/capability/plaintext-canary CI.
