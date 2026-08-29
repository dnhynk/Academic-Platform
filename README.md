# Academic Platform — synthetic Phase 1 local core

This repository is the runnable foundation for a local-first Personal Academic · CS · Project OS. It implements a synthetic, throwaway Phase 1 local core: the daemon owns a plaintext SQLite profile, accepts one allowlisted signed fixture over current-user local IPC, persists canonical and vault state, builds disposable projections, and supports doctor, deterministic export, consistent plaintext backup, and verified restore into a new empty profile. It has no desktop UI, recorder, cloud connector, arbitrary input, or network-capable product behavior. Real or production data remains forbidden because ADR-002 is unaccepted, the default lane reports `storage_encryption=NONE`, and five-platform SQLCipher evidence is incomplete.

## What is executable

- `academic-domain`: RFC-variant UUIDv7 IDs enforced by every constructor, canonical decimal coefficient strings, portable logical paths, registered evidence representations, required scopes, typed claims, actor/authority/status rules, half-open valid intervals, independent mastery/freshness types, user decisions, and the canonical entity registry — multilingual and abbreviation aliases, `ConceptSense` separation, abstaining mention resolution, non-destructive merge with redirect, queue-only split, and the `IDENTICAL`/`REFINED`/`SPLIT_AMBIGUOUS`/`INCOMPARABLE` migration-equivalence contract. See [the entity registry contract](docs/contracts/entity-registry.md). It also fixes the deterministic engine contract: the thirteen-engine registry, the pure `(frozen_inputs, rule_set_hash, engine_version)` signature, the fixed proof-tree node shape, normalized explanation snapshots, and byte-equal replay. See [the engine harness contract](docs/contracts/engine-harness.md).
- `academic-ledger`: verified-capability-only append, device origin-chain gap/fork checks, replica-local `accept_seq`, cross-domain evidence closure, scope-isolated authority resolution, and bitemporal queries.
- Bitemporal query surface: every read takes `known_at_accept_seq` and `valid_at` as one value, the eighteen Phase 2 aggregate closure tables and the resolved claim lane are projected at those coordinates, materialized snapshots live in a separate disposable sidecar that records the projector that built them, and a transition is labelled `EVIDENCE_CHANGE`, `ONTOLOGY_CHANGE`, `ANALYZER_UPGRADE`, or `OFFICIAL_SOURCE_CORRECTION` by splitting the known-time interval rather than ranking a mixed one. See [the bitemporal time-travel contract](docs/contracts/bitemporal-time-travel.md).
- `academic-contracts`: deterministic CBOR v3 encode/sign plus v1/v2/v3 decode/verify, semantic v3 validation of returned writer bytes, Ed25519 verification over original bytes, source-aware typed byte equality, device/key/user identity binding, pure v1-to-v3 and v2-to-v3 upcasters that rewrite no historical byte, and executable Protobuf actor/relation round trips with the same RFC-variant UUIDv7 boundary.
- `academic-core`: the signed-envelope acceptance boundary; fixture verification and replay use an independent trust anchor rather than wrapper-supplied keys.
- `academic` CLI: `daemon serve|status`, `doctor` (with `--profile`/`--deep`), `ingest`, `export`, `backup`, `restore`, `crash-replay`, and `fixture emit|verify|replay`. Every path prints the synthetic-only banner before its results and repeats the policy object in JSON, and exit codes distinguish policy denial, conflict, repair-required, incompatible, unavailable, and internal failure. See [the CLI contract](docs/contracts/phase1-cli.md).
- `@academic-os/web-contracts`: exact TypeScript fixture validation kept in positive/negative parity with JSON Schema and Rust.
- Phase 1 local-core crates: functional store, vault, RPC, daemon, projection, portability, and test-support boundaries; bundled plaintext SQLite remains the default lane. The explicit non-default SQLCipher spike supplies limited evidence only and carries no ADR-002 or production-data acceptance claim.

All three committed fixtures are synthetic and declare `network_egress: NONE`. `signed-batch-v1.json` and `signed-batch-v2.json` are byte-immutable read-only compatibility goldens; their signed payloads are verified before a deterministic, fail-closed semantic upcast to v3, and no public API or CLI can mint v1 or v2. `signed-batch-v3.json` is the only writer fixture. It exercises all eighteen event schema v3 registration arms at Proto tags 16..=33 — `CURRICULUM_VERSION_PUBLISHED`, `COURSE_REVISION_PUBLISHED`, `OFFERING_OBSERVED`, `ATTEMPT_RECORDED`, `REQUIREMENT_SET_PUBLISHED`, `AUDIT_COMPUTED`, `CAPTURE_PERMISSION_RECORDED`, `LECTURE_SESSION_RECORDED`, `TRANSCRIPT_VERSION_ADDED`, `LECTURE_DOCUMENT_PUBLISHED`, `SNAPSHOT_REGISTERED`, `FINDING_PUBLISHED`, `MODEL_RUN_RECORDED`, `PROPOSAL_DISPOSED`, `EGRESS_DECIDED`, `CONSENT_RECORDED`, `ENTITY_IDENTITY_CHANGED`, and `RETENTION_ACTION_RECORDED` — with the optional provenance digest present on some arms and absent on others, and it carries forward the v2 claims that demonstrate that a later AI inference cannot override an explicit user decision, that freshness can become `STALE` while mastery remains `PRACTICED`, and that a Prediction retains confidence plus versioned evidence-window/sample metadata distinct from its applicability interval. Fixture-wrapper ingress consumes original bytes with fatal UTF-8 decoding, so malformed sequences never become U+FFFD before JSON, Ajv, or semantic parsing. The raw boundary also rejects duplicate decoded property names, non-Unicode-scalar strings, and mathematically fractional designated integer lexemes at arbitrary precision while preserving integral spellings such as `2.0` and `2e0`. Prediction disclosure integers follow that same lexical rule while enforcing their typed bounds; every disclosed timestamp is a non-negative JavaScript-safe integer, and an applicability `to` key is required with explicit `null` as the sole open-ended representation. Artifact JSON applies the same ambiguity checks plus canonical unsigned number tokens, and Ajv, TypeScript, and Rust run the shared byte, exact-integer, prediction-metadata, and raw corpora.

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
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
cargo test -p academic-scenario --test compile_fail --locked --offline
cargo clippy -p academic-vault --all-targets --locked --offline --features aead-objects,phase2-fault-injection -- -D warnings
cargo test -p academic-vault --all-targets --locked --offline --features aead-objects,phase2-fault-injection
pnpm install --frozen-lockfile --offline
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
pnpm fixture:verify:v2
pnpm fixture:replay:v2
git diff --exit-code -- schemas/fixtures/
```

The encrypted object lane above is a non-default `academic-vault` feature and
needs no native toolchain: it is pure-Rust XChaCha20-Poly1305 over the Phase 1
publish sequence. What it is and is not evidence for is in
[the encrypted object format](docs/contracts/encrypted-object-format.md).

The encrypted store lane is non-default and is verified separately, because it
cannot be linked into the same binary as the plaintext synthetic lane:

```powershell
pnpm verify:windows-toolchain
cargo clippy -p academic-store --no-default-features --features sqlcipher-store --all-targets --locked --offline -- -D warnings
cargo test -p academic-store --no-default-features --features sqlcipher-store --locked --offline
```

Building it needs a native SQLCipher and OpenSSL. On Windows that needs a native
Perl, pinned and recorded in
[the Windows toolchain](tools/sqlcipher/windows-toolchain.md); set
`OPENSSL_SRC_PERL` to the pinned interpreter before the commands above.
`verify:windows-toolchain` enforces the pin whenever that variable is set and
reports and exits zero when it is not. What the lane is and is not evidence for
is in [the encrypted store lane](docs/contracts/encrypted-store-lane.md).

## Operating a throwaway Phase 1 profile

Every path below is synthetic-only and disposable. Do not point any of them at
real data.

```powershell
cargo run --locked --offline -p academic-cli -- daemon serve --profile <profile> --runtime <runtime>
cargo run --locked --offline -p academic-cli -- daemon status --profile <profile> --runtime <runtime> --format json
cargo run --locked --offline -p academic-cli -- ingest --profile <profile> --runtime <runtime> --fixture phase0-synthetic-bitemporal-ledger-v2
cargo run --locked --offline -p academic-cli -- doctor --profile <profile> --deep --format json
cargo run --locked --offline -p academic-cli -- export --profile <profile> --destination <export>
cargo run --locked --offline -p academic-cli -- backup --profile <profile> --destination <backup>
cargo run --locked --offline -p academic-cli -- restore --backup <backup> --new-profile <fresh>
cargo run --locked --offline -p academic-cli -- crash-replay --all --format json
```

`daemon serve` runs in the foreground and creates the profile when its root is
absent. `crash-replay` only reports the enumerated fault matrix; it cannot
terminate a process, and a production build carries no crash switch.

## Safety boundary

Do not ingest personal, academic, lecture, repository, credential, or recording data in Phase 0. ADR-002 remains an acceptance gate: an encrypted transactional store must pass packaging, leakage, power-loss, backup/restore, and unsafe-location tests before real data is permitted. See [the security baseline](docs/security/baseline.md) and [the ADR register](docs/adr/README.md).
