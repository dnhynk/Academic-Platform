# Academic Platform — Phase 0 executable invariants

This repository is the runnable foundation for a local-first Personal Academic · CS · Project OS. Phase 0 deliberately ships no database, daemon, desktop UI, recorder, cloud connector, or network-capable product path. It proves the canonical vocabulary and signed-ledger semantics with synthetic fixtures before any real personal data is allowed.

## What is executable

- `academic-domain`: UUIDv7 IDs, algorithm-prefixed digests, keyed vault locators, exact evidence spans, typed claims, epistemic and authority vocabularies, half-open valid intervals, independent mastery/freshness types, and user decisions.
- `academic-ledger`: append-only batch acceptance, device origin-chain gap/fork checks, replica-local `accept_seq`, evidence closure, predicate-specific authority resolution, and bitemporal queries.
- `academic-contracts`: deterministic CBOR encode/decode, Ed25519 sign/verify, canonical-byte rejection, and expected device-key anchoring.
- `academic-core`: the acceptance boundary; unsigned or unverified events never reach the ledger.
- `academic` CLI: privacy-safe doctor plus fixture emit/verify/replay commands.
- `@academic-os/web-contracts`: TypeScript-side fixture contract validation.

The committed fixture is synthetic and declares `network_egress: NONE`. Its replay demonstrates that a later AI inference cannot override an explicit user decision and that freshness can become `STALE` while mastery remains `PRACTICED`.

## Bootstrap

Prerequisites are pinned to Rust 1.98.0, Node 24.19.0, and pnpm 11.22.0. See [the full bootstrap guide](docs/development/bootstrap.md).

```powershell
nvm use 24.19.0
rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy
npm install --global pnpm@11.22.0
pnpm install --frozen-lockfile
pnpm run doctor
```

No Docker or cloud account is required.

## Required verification

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm verify:contracts
pnpm security
pnpm fixture:replay
```

## Safety boundary

Do not ingest personal, academic, lecture, repository, credential, or recording data in Phase 0. ADR-002 remains an acceptance gate: an encrypted transactional store must pass packaging, leakage, power-loss, backup/restore, and unsafe-location tests before real data is permitted. See [the security baseline](docs/security/baseline.md) and [the ADR register](docs/adr/README.md).
