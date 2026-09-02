# Academic Platform — synthetic Phase 1 local core

This repository is the runnable foundation for a local-first Personal Academic · CS · Project OS. It implements a synthetic, throwaway Phase 1 local core: the daemon owns a plaintext SQLite profile, accepts one allowlisted signed fixture over current-user local IPC, persists canonical and vault state, builds disposable projections, and supports doctor, deterministic export, consistent plaintext backup, and verified restore into a new empty profile. It has no desktop UI, recorder, cloud connector, arbitrary input, or network-capable product behavior. Real or production data remains forbidden because ADR-002 is unaccepted, the default lane reports `storage_encryption=NONE`, and five-platform SQLCipher evidence is incomplete.

## What is executable

- `academic-domain`: RFC-variant UUIDv7 IDs enforced by every constructor, canonical decimal coefficient strings, portable logical paths, registered evidence representations, required scopes, typed claims, actor/authority/status rules, half-open valid intervals, independent mastery/freshness types, user decisions, and the canonical entity registry — multilingual and abbreviation aliases, `ConceptSense` separation, abstaining mention resolution, non-destructive merge with redirect, queue-only split, and the `IDENTICAL`/`REFINED`/`SPLIT_AMBIGUOUS`/`INCOMPARABLE` migration-equivalence contract. See [the entity registry contract](docs/contracts/entity-registry.md). On that identity layer it adds distinct `Field`/`Concept`/`Operation` imports bound to exact taxonomy versions, single-occurrence promotion abstention, `GRANULARITY_UNDER_REVIEW`, content-free ontology quality metrics, and a versioned-impact approval token that only ADR-003's user actor can obtain; the ACM/curriculum/user-derived base mix remains unselected. See [the ontology core contract](docs/contracts/ontology-core.md). It also fixes the deterministic engine contract: the twelve-engine §28 registry, the pure `(frozen_inputs, rule_set_hash, engine_version)` signature, the fixed proof-tree node shape, normalized explanation snapshots, and byte-equal replay. See [the engine harness contract](docs/contracts/engine-harness.md).
- `academic-ledger`: verified-capability-only append, device origin-chain gap/fork checks, replica-local `accept_seq`, cross-domain evidence closure, scope-isolated authority resolution, and bitemporal queries. Its product extension supplies the six claim-type precedence tables, fail-closed upstream-source corroboration, and conflict-card vocabulary without forking Phase 1 decision replay. See [the product authority contract](docs/contracts/product-authority-resolution.md).
- Bitemporal query surface: every read takes `known_at_accept_seq` and `valid_at` as one value, the eighteen Phase 2 aggregate closure tables and the resolved claim lane are projected at those coordinates, materialized snapshots live in a separate disposable sidecar that records the projector that built them, and a transition is labelled `EVIDENCE_CHANGE`, `ONTOLOGY_CHANGE`, `ANALYZER_UPGRADE`, or `OFFICIAL_SOURCE_CORRECTION` by splitting the known-time interval rather than ranking a mixed one. See [the bitemporal time-travel contract](docs/contracts/bitemporal-time-travel.md).
- `academic-contracts`: deterministic CBOR v3 encode/sign plus v1/v2/v3 decode/verify, semantic v3 validation of returned writer bytes, Ed25519 verification over original bytes, source-aware typed byte equality, device/key/user identity binding, pure v1-to-v3 and v2-to-v3 upcasters that rewrite no historical byte, and executable Protobuf actor/relation round trips with the same RFC-variant UUIDv7 boundary.
- `academic-core`: the signed-envelope acceptance boundary; fixture verification and replay use an independent trust anchor rather than wrapper-supplied keys.
- `academic-policy`: a socket-free, default-deny permission broker that hashes immutable policy snapshots, minimizes configured object ranges, records the fixed grant/audit shapes, and releases an exact runtime payload only after atomically consuming a one-use expiring capability. See [the permission broker contract](docs/contracts/permission-broker.md).
- Process boundaries: six separate executables bind capture client, indexer, repository analyzer, connector, egress proxy, and export job to distinct broker-owned capability sets. The egress executable has no transport yet; P2-G2 owns the sole future outbound socket.
- `academic` CLI: `admission verify|show`, `daemon serve|status`, `doctor` (with `--profile`/`--deep`), `ingest`, `export`, `backup`, `restore`, `crash-replay`, and `fixture emit|verify|replay`. Every path prints its receipt-derived posture before human results and repeats it as the JSON `policy` object; the present unprovisioned key keeps that posture synthetic. Exit codes distinguish policy denial, conflict, repair-required, incompatible, unavailable, and internal failure. See [the CLI contract](docs/contracts/phase1-cli.md) and [admission receipt contract](docs/contracts/admission-receipt.md).
- `academic-record`: the section 10 attempt ledger and the first two §28 engines. Every attempt is preserved — `AttemptHistory` has one mutator, no removal path, and a correction is a new entry carrying ADR-003's `SUPERSEDES` — and a `CourseAttempt` exists only where a confirmed registration or `academic-transcript`'s user-confirmed row does. Requirement classification is a versioned rule-engine output with no public constructor, asserted under `DeterministicEngine` authority that ADR-003's actor matrix refuses to a user. The 2015-spring repeat ceiling and the post-2004 external-grade exclusion are effective-dated policy rows rather than constants, and the repeat-recognition rule no official source states is `UNKNOWN` rather than a default. `engine.gpa` and `engine.credit.accounting` are the two registry entries this brings to `IMPLEMENTED`; both are pure functions of `(frozen_inputs, rule_set_hash, engine_version)`, both publish a value only when it is fully determined, and their harness corpora under `testdata/engines/` are executed and byte-compared rather than merely counted. All arithmetic is exact over `academic_domain::Decimal`: no `f32`, no `f64`, and no floating-point literal appears in the crate, and the shipped corpus averages `33.9 / 12` — exactly `2.825`, a tie `f64` cannot represent — so a float would fail the fixture rather than pass it silently. Expected averages come from `tools/gpa-oracle.mjs`, an independent transcription in another language. See [the GPA and attempt contract](docs/contracts/gpa-and-attempts.md).
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
cargo clippy -p academic-retention --all-targets --locked --offline --features rotation-engine,phase2-fault-injection -- -D warnings
cargo test -p academic-retention --all-targets --locked --offline --features rotation-engine,phase2-fault-injection
cargo clippy -p academic-retention --all-targets --locked --offline --features rotation-engine,rotation-orchestration,phase2-fault-injection -- -D warnings
cargo test -p academic-retention --all-targets --locked --offline --features rotation-engine,rotation-orchestration,phase2-fault-injection
cargo clippy -p academic-transcript --all-targets --locked --offline --features encrypted-vault,phase2-fault-injection -- -D warnings
cargo test -p academic-transcript --all-targets --locked --offline --features encrypted-vault,phase2-fault-injection
cargo clippy -p academic-worker --all-targets --locked --offline --features native-sandbox -- -D warnings
cargo test -p academic-worker --all-targets --locked --offline --features native-sandbox
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

Hosted CI materializes **17 required jobs**: one source preflight, five
`rust-default-*` jobs, five `rust-features-*` jobs, two Phase 1 exit jobs,
three Linux-only encrypted/rotation jobs, and one pnpm contracts job. A green
run is therefore reported as **17/17**, and the measured duration-to-timeout
ratios and refresh rule live in [the CI budget record](docs/development/ci-budget.md).

The encrypted object lane above is a non-default `academic-vault` feature and
needs no native toolchain: it is pure-Rust XChaCha20-Poly1305 over the Phase 1
publish sequence. What it is and is not evidence for is in
[the encrypted object format](docs/contracts/encrypted-object-format.md).

The recovery contract of `P2-K4` — the section 3.3 profile registry, the backup
key that is independent of the operating-system device wrapper, and the restore
rehearsal receipt — is pure Rust in `academic-recovery` and runs in the block
above, on every platform. What it is and is not evidence for is in
[encrypted backup and recovery](docs/contracts/encrypted-backup-and-recovery.md).

`P2-K6` adds the deterministic-CBOR `AdmissionVerifier`, its compiled five-row
platform set, and one exact posture emitter for CLI, IPC, and export (there is no
desktop surface yet). The user's offline acceptance public key has not been
provided, so the typed compiled key state is unprovisioned; the committed
candidate receipt also has only its Windows x86-64 and Linux x86-64 rows. Both
conditions fail closed, `production_data_allowed` remains `false`, and ADR-002
remains unaccepted. The exact receipt shape and provisioning boundary are in
[the admission receipt contract](docs/contracts/admission-receipt.md).

Several tests keep that posture by reading this repository's own source text,
because the changes they refuse — a second key source, a widened fixture
allowlist, a banner suppressed behind a marker file — alter nothing observable
without their trigger. Every such scan is enumerated, with what it reads and
what it still leaves open, in
[policy source scans](docs/contracts/policy-source-scans.md).

`P2-U7` adds `academic-transcript`: PDF, CSV and manual-entry import of an
official transcript, import rows kept as claims distinct from the rows the user
confirmed, field-level checksum reconciliation that halts at the exact row, and
redaction as a projection that never edits a source. Its default half is pure
Rust and runs inside `cargo test --workspace`; the half that seals an original
as an `AEAD_CHUNKED_V2` object and the half that takes the `IN04` kill matrix
need the two non-default features in the commands above, which are also hosted
CI steps on every Rust matrix label. The crate adds no PDF or OCR dependency and
defines no object format: the corpus is written by a deterministic builder and
the seal is ADR-004's. Nothing durable happens today, because `P2-K6` did not
open admission and both gated entry points take a verified receipt by type. What
that refusal covers, what the text-layer parser does and does not read, and why
the identity header is not a reconciliation halt condition are in
[transcript ingestion](docs/contracts/transcript-ingestion.md).

`P2-G1` adds `academic-policy` without adding a product socket or a new
external dependency. A new profile exposes `local_processing_preferred=true`
and zero configured egress rules; a complete tuple against that empty snapshot
is denied and audited. Its policy store is separate from the canonical store
because the execution plan's `0005` allocation was already occupied on the
P2-K6 baseline; the discrepancy and the concrete tuple interpretation are in
[the permission broker contract](docs/contracts/permission-broker.md).

`P2-G3` extends that same socket-free boundary with an append-only provider
policy registry. Provider identity includes the enterprise/API versus consumer
surface, required privacy facts produce a deterministic snapshot digest, and
provider freshness directly caps grant expiry. Changed facts invalidate old
snapshot pins; providers without a deletion API need an evidence-linked user
policy rather than a synthesized default. Immutable deletion-receipt metadata
links back to the exact grant and allow audit. `GATE-38-010` and
`GATE-38-028` remain open; see
[the provider registry contract](docs/contracts/provider-registry.md).

`P2-G7` replaces the broker's free-form process label with a closed enum,
binds it into both egress and process capability tokens, and enumerates every
allowed and denied matrix cell. Actor, artifact ranges, external transmission
digest/count, and created claim identifiers are retained in append-only audit
rows without source bytes. The indexer and export-job process packages have
complete dependency-closure and whole-entrypoint guards: neither closure
contains a network or key-material crate, and neither entrypoint reaches for
one. The Rust standard library still puts a raw socket within reach of any
process, so that is a bound on what these packages depend on and use, not an
operating-system sandbox — `P2-G4` owns that. See
[the permission broker contract](docs/contracts/permission-broker.md).

`P2-G2` adds `academic-egress-boundary`, which stages for the egress-proxy
process at the edge of the section 3.6 topology. It ships the versioned DLP
rulepack whose identity every grant records as its `redaction_policy_hash`,
structural minimization of a whole-file request to the declarations it named, a
byte-accurate preview that is the same buffer the transport writes, and one
outbound transport trait. Two functions reach that trait, and both bind the
grant first: the plan has to name the grant the capability will consume, and
that grant's recorded rulepack has to be the pack that produced the staged
bytes. The two call sites are counted, so a third path cannot skip it. It is a separate package from `P2-G7`'s
`academic-egress` process entry point because that task pins the whole manifest
and the whole product source of the process package as one fixed process-class
binding, and a library target inside it would have made that pin weaker rather
than exact.

It ships no socket: ADR-002 is unaccepted, the admission receipt is incomplete,
and the emitted posture is still `product_network: "NONE"`, so no crate in this
workspace names an outbound socket construct.
`only_egress_crate_has_a_socket` in `tools/phase1-scaffold-policy.test.mjs` is
what keeps that exception scoped by crate, and it narrows rather than replaces
the process-closure guards above: a per-file spelling allowance read with
comments and literals stripped, an alias rule, a foreign-function rule, a
`#[path]` and `include!` rule, a build-script inventory, and a per-crate link
closure. `GATE-38-028` stays open and `cloud_egress_default()` takes no
argument, so no quality heuristic can move it off `LOCAL_ONLY_OR_STOP`. What the
boundary does and does not claim is in
[the egress boundary contract](docs/contracts/egress-boundary.md).

`P2-G4` adds `academic-worker`, which runs a pipeline job out of process under a
measured operating-system sandbox. The two commands above are its non-default
lane and are the only place the backends are built: seccomp, Landlock and
`setrlimit` on Linux, an AppContainer with no capabilities plus a job object on
Windows. Both were measured by launching a process, attempting the operation
inside it, and reading what the kernel answered — a home read, a vault read, a
socket, a child process, and each of the four bounds — and each refusal is
paired with the same probe run uncontained, so a refusal the machine would have
given anyway fails the test rather than passing it.

Two of those claims are narrower than their names. On Windows the socket
*handle* is still created; what is refused is every address off the host, with
`WSAEACCES`. And two of the four bounds kill the job while the other two refuse
the operation. The per-platform table, and the six ways the acceptance boundary
refuses a staged output, are in
[the worker sandbox contract](docs/contracts/worker-sandbox.md). The crate adds
no package to `Cargo.lock`; it takes two direct edges to crates already in it,
receipted in
[dependency-admission-phase2-g4.json](docs/security/dependency-admission-phase2-g4.json).
`product_network` stays `NONE` and nothing here moves ADR-002.

`P2-G5` adds `academic-untrusted-content`, the boundary between bytes that came
from outside and anything this system acts on. Every ingested byte — syllabus,
README, issue, code comment, review text, provider response — is wrapped in
`Untrusted<T>` at parse time, and the wrapper implements no `Deref`, no
`Into<String>`, and no `Display`, so no conversion off the label exists for a
caller to call. What the compiler cannot refuse is a function inside the crate
calling the crate-private accessor on a caller's behalf, so two source rules
carry that half: the whole inventory of the accessor's call sites, counted by
name, and a rule that no public signature in the workspace takes an
`Untrusted<…>` and returns the bytes. A rendered prompt's instruction channels take
`&'static str`; its data channel escapes what it quotes into one line of ASCII,
so a payload cannot open a line, close the field it sits in, or spell a
bidirectional override. A model output becomes a proposal only after schema
validation and provenance resolution, and either refusal is a quarantine state
with no conversion back. The 54-record synthetic injection corpus is in
`testdata/injection-corpus/`, and the provider-response scan is `P2-G2`'s,
reused by taking its `AcceptedResponse` as an argument rather than
reimplemented. What the boundary does and does not claim — including that its
link to `P2-G4`'s acceptance boundary is by composition and not by type — is in
[the untrusted-content contract](docs/contracts/untrusted-content.md). It runs
inside `cargo test --workspace` and adds no package to `Cargo.lock`.

`P2-K5`'s rotation journal, recipient revocation, crypto-shred, and retention
vocabulary live in `academic-retention`. Where a rotation moves the canonical
object reference and where a deletion reaches a backup are in the encrypted
portability lane below, because only that lane links the store and the backup
boundary in one process. Its default-lane half is pure Rust and
runs inside `cargo test --workspace`; the half that rewraps and shreds real
`AEAD_CHUNKED_V2` objects needs the non-default `rotation-engine` feature and the
commands above, which are also hosted CI steps on every Rust matrix label.
What a rotation and a crypto-shred do and do not claim is in
[rotation and retention](docs/contracts/rotation-and-retention.md).

**Phase 2 does not accept running a rotation.** The seven entry points that would
drive one refuse on their first line, and the third and fourth commands above are
the lane where the machinery under those refusals still executes — including the
`KY03` to `KY05` fault rows and the `T114`/`T116` seam closures. No product graph
selects that lane, which `phase1-scaffold-policy.test.mjs` checks, and the hosted
`rotation-orchestration-lane` job is what runs it. Crypto-shredding, backup
tombstones, and their re-application on restore are outside the gate and keep
working. So are the primitives a rotation composes — re-sealing an object and
rekeying the profile database live in crates below `academic-retention` and
cannot reach the refusal — so what the gate covers is the journalled
orchestration, not every write a rotation performs. That, and what an
orchestrator has to close before the gate opens, is listed in the same
contract.

The encrypted store lane and the encrypted backup lane are non-default and are
verified separately, because neither can be linked into the same binary as the
plaintext synthetic lane. Both halves also run in hosted CI on `ubuntu-latest`,
as the `encrypted-store-lane` and `encrypted-portability-lane` jobs. The first
is what makes `EN01` — the store-rekey kill the `P2-K5` rotation journal's
database unit depends on — executed evidence rather than a citation. The second
covers the seam where that rotation and its deletions meet the canonical store
and the backup boundary — including the refusal, in `encrypted_rotation_gate.rs`,
and including
`backup_tombstone_is_present_and_re_deletes_on_restore`, which calls the product
backup and the product restore rather than imitating them. Native Windows is not in that job
and stays local, because `openssl-src` needs a Perl the hosted Windows image does
not carry:

```powershell
pnpm verify:windows-toolchain
cargo clippy -p academic-store --no-default-features --features sqlcipher-store --all-targets --locked --offline -- -D warnings
cargo test -p academic-store --no-default-features --features sqlcipher-store --locked --offline
cargo clippy -p academic-portability --no-default-features --features encrypted-portability,phase2-fault-injection --all-targets --locked --offline -- -D warnings
cargo test -p academic-portability --no-default-features --features encrypted-portability --locked --offline
cargo test -p academic-portability --no-default-features --features encrypted-portability,phase2-fault-injection --locked --offline --test encrypted_crash
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
cargo run --locked --offline -p academic-cli -- admission show --profile <profile> --format json
cargo run --locked --offline -p academic-cli -- admission verify --profile <profile> --format json
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
