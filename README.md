# Academic Platform — Phase 0 executable invariants

This repository is the runnable foundation for a local-first Personal Academic · CS · Project OS. Phase 0 deliberately ships no database, daemon, desktop UI, recorder, cloud connector, or network-capable product path. It proves the canonical vocabulary and signed-ledger semantics with synthetic fixtures before any real personal data is allowed.

## What is executable

- `academic-domain`: checked UUIDv7 IDs, canonical decimal coefficient strings, portable logical paths, registered evidence representations, required scopes, typed claims, actor/authority/status rules, half-open valid intervals, independent mastery/freshness types, and user decisions.
- `academic-ledger`: verified-capability-only append, device origin-chain gap/fork checks, replica-local `accept_seq`, cross-domain evidence closure, scope-isolated authority resolution, and bitemporal queries.
- `academic-contracts`: deterministic CBOR encode/decode, Ed25519 sign/verify, canonical-byte rejection, device/key/user identity binding, and executable Protobuf actor/relation round trips.
- `academic-core`: the signed-envelope acceptance boundary; fixture verification and replay use an independent trust anchor rather than wrapper-supplied keys.
- `academic` CLI: privacy-safe doctor plus fixture emit/verify/replay commands.
- `@academic-os/web-contracts`: exact TypeScript fixture validation kept in positive/negative parity with JSON Schema and Rust.

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
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm verify:contracts
pnpm security
pnpm fixture:emit
pnpm fixture:verify
pnpm fixture:replay
```

## Safety boundary

Do not ingest personal, academic, lecture, repository, credential, or recording data in Phase 0. ADR-002 remains an acceptance gate: an encrypted transactional store must pass packaging, leakage, power-loss, backup/restore, and unsafe-location tests before real data is permitted. See [the security baseline](docs/security/baseline.md) and [the ADR register](docs/adr/README.md).
