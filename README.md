# Academic Platform — Phase 0 executable invariants

This repository is the runnable foundation for a local-first Personal Academic · CS · Project OS. Phase 0 deliberately ships no database, daemon, desktop UI, recorder, cloud connector, or network-capable product path. It proves the canonical vocabulary and signed-ledger semantics with synthetic fixtures before any real personal data is allowed.

## What is executable

- `academic-domain`: RFC-variant UUIDv7 IDs enforced by every constructor, canonical decimal coefficient strings, portable logical paths, registered evidence representations, required scopes, typed claims, actor/authority/status rules, half-open valid intervals, independent mastery/freshness types, and user decisions.
- `academic-ledger`: verified-capability-only append, device origin-chain gap/fork checks, replica-local `accept_seq`, cross-domain evidence closure, scope-isolated authority resolution, and bitemporal queries.
- `academic-contracts`: deterministic CBOR v2 encode/sign plus v1/v2 decode/verify, semantic v2 validation of returned writer bytes, Ed25519 verification over original bytes, source-aware typed byte equality, device/key/user identity binding, an immutable v1-to-v2 decision upcaster, and executable Protobuf actor/relation round trips with the same RFC-variant UUIDv7 boundary.
- `academic-core`: the signed-envelope acceptance boundary; fixture verification and replay use an independent trust anchor rather than wrapper-supplied keys.
- `academic` CLI: privacy-safe doctor plus fixture emit/verify/replay commands.
- `@academic-os/web-contracts`: exact TypeScript fixture validation kept in positive/negative parity with JSON Schema and Rust.

Both committed fixtures are synthetic and declare `network_egress: NONE`. `signed-batch-v1.json` is a byte-immutable read-only compatibility golden; v1 signed payloads are verified before a deterministic, fail-closed semantic upcast, and no public API or CLI can mint v1. `signed-batch-v2.json` is the only writer fixture and demonstrates that a later AI inference cannot override an explicit user decision, that freshness can become `STALE` while mastery remains `PRACTICED`, and that a Prediction retains confidence plus versioned evidence-window/sample metadata distinct from its applicability interval. Fixture-wrapper ingress consumes original bytes with fatal UTF-8 decoding, so malformed sequences never become U+FFFD before JSON, Ajv, or semantic parsing. The raw boundary also rejects duplicate decoded property names, non-Unicode-scalar strings, and mathematically fractional designated integer lexemes at arbitrary precision while preserving integral spellings such as `2.0` and `2e0`. Artifact JSON applies the same ambiguity checks plus canonical unsigned number tokens, and Ajv, TypeScript, and Rust run the shared byte, exact-integer, prediction-metadata, and raw corpora.

## Bootstrap

Prerequisites are pinned to Rust 1.98.0, Node 24.19.0, and pnpm 11.22.0. See [the full bootstrap guide](docs/development/bootstrap.md).

```powershell
nvm use 24.19.0
rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy
npm install --global pnpm@11.22.0
node tools/source-preflight.mjs
pnpm install --frozen-lockfile
pnpm run doctor
```

No Docker or cloud account is required.

## Required verification

```powershell
node tools/source-preflight.mjs
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
pnpm security:source-probes
pnpm fixture:emit
pnpm fixture:verify
pnpm fixture:replay
pnpm fixture:verify:v1
pnpm fixture:replay:v1
```

## Safety boundary

Do not ingest personal, academic, lecture, repository, credential, or recording data in Phase 0. ADR-002 remains an acceptance gate: an encrypted transactional store must pass packaging, leakage, power-loss, backup/restore, and unsafe-location tests before real data is permitted. See [the security baseline](docs/security/baseline.md) and [the ADR register](docs/adr/README.md).
