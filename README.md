# Academic Platform — synthetic Phase 1 local core

This repository is the runnable foundation for a local-first Personal Academic · CS · Project OS. It implements a synthetic, throwaway Phase 1 local core: the daemon owns a plaintext SQLite profile, accepts one allowlisted signed fixture over current-user local IPC, persists canonical and vault state, builds disposable projections, and supports doctor, deterministic export, consistent plaintext backup, and verified restore into a new empty profile. It has no desktop UI, recorder, cloud connector, arbitrary input, or network-capable product behavior. Real or production data remains forbidden because ADR-002 is unaccepted, the default lane reports `storage_encryption=NONE`, and five-platform SQLCipher evidence is incomplete.

## What is executable

- `academic-domain`: RFC-variant UUIDv7 IDs enforced by every constructor, canonical decimal coefficient strings, portable logical paths, registered evidence representations, required scopes, typed claims, actor/authority/status rules, half-open valid intervals, independent mastery/freshness types, user decisions, and the canonical entity registry — multilingual and abbreviation aliases, `ConceptSense` separation, abstaining mention resolution, non-destructive merge with redirect, queue-only split, and the `IDENTICAL`/`REFINED`/`SPLIT_AMBIGUOUS`/`INCOMPARABLE` migration-equivalence contract. See [the entity registry contract](docs/contracts/entity-registry.md). On that identity layer it adds distinct `Field`/`Concept`/`Operation` imports bound to exact taxonomy versions, single-occurrence promotion abstention, `GRANULARITY_UNDER_REVIEW`, content-free ontology quality metrics, and a versioned-impact approval token that only ADR-003's user actor can obtain; the ACM/curriculum/user-derived base mix remains unselected. See [the ontology core contract](docs/contracts/ontology-core.md). It also fixes the deterministic engine contract: the twelve-engine §28 registry, the pure `(frozen_inputs, rule_set_hash, engine_version)` signature, the fixed proof-tree node shape, normalized explanation snapshots, and byte-equal replay. See [the engine harness contract](docs/contracts/engine-harness.md).
- `academic-ledger`: verified-capability-only append, device origin-chain gap/fork checks, replica-local `accept_seq`, cross-domain evidence closure, scope-isolated authority resolution, and bitemporal queries. Its product extension supplies the six claim-type precedence tables, fail-closed upstream-source corroboration, and conflict-card vocabulary without forking Phase 1 decision replay. See [the product authority contract](docs/contracts/product-authority-resolution.md).
- Bitemporal query surface: every read takes `known_at_accept_seq` and `valid_at` as one value, the eighteen Phase 2 aggregate closure tables and the resolved claim lane are projected at those coordinates, materialized snapshots live in a separate disposable sidecar that records the projector that built them, and a transition is labelled `EVIDENCE_CHANGE`, `ONTOLOGY_CHANGE`, `ANALYZER_UPGRADE`, or `OFFICIAL_SOURCE_CORRECTION` by splitting the known-time interval rather than ranking a mixed one. See [the bitemporal time-travel contract](docs/contracts/bitemporal-time-travel.md).
- `academic-contracts`: deterministic CBOR v3 encode/sign plus v1/v2/v3 decode/verify, semantic v3 validation of returned writer bytes, Ed25519 verification over original bytes, source-aware typed byte equality, device/key/user identity binding, pure v1-to-v3 and v2-to-v3 upcasters that rewrite no historical byte, and executable Protobuf actor/relation round trips with the same RFC-variant UUIDv7 boundary.
- `academic-core`: the signed-envelope acceptance boundary; fixture verification and replay use an independent trust anchor rather than wrapper-supplied keys.
- `academic-model-run`: the twelve section 27.3 fields every model execution records, the per-model calibration dataset registry with its refresh metadata, and the reconciliation of a recorded transmission against the broker's `egress_audit`. A raw provider score is unorderable and undisplayable by type; only an interpreted one reaches a reader. See [model-run provenance](docs/contracts/model-run-provenance.md).
- `academic-proposal`: the `Proposed<T>` boundary, the four section 27.4 risk tiers with the workflow each requires, the review queue with section 29.7's confidence/impact batching under a versioned configuration, and the append-only disposition history an undo extends rather than edits. See [the proposal review queue](docs/contracts/proposal-review-queue.md).
- `academic-policy`: a socket-free, default-deny permission broker that hashes immutable policy snapshots, minimizes configured object ranges, records the fixed grant/audit shapes, and releases an exact runtime payload only after atomically consuming a one-use expiring capability. See [the permission broker contract](docs/contracts/permission-broker.md).
- `academic-consent`: the section 3.7 `capture_permission` aggregate and the append-only consent ledger under it. A new offering has no record, `UNKNOWN` is what a missing record resolves to, and nothing is mintable from it. A user attestation and a written authority are unrelated types with no conversion between them, so a self-assessment cannot reach a permitting status; audio and transcript retention are two independent bounds and a derivative inherits the stricter of each; and an expiry cannot be applied without the deletion-impact preview it describes. `GATE-38-009` and `GATE-38-019` stay open per offering and per term. See [the consent contract](docs/contracts/consent-and-capture-permission.md).
- Process boundaries: six separate executables bind capture client, indexer, repository analyzer, connector, egress proxy, and export job to distinct broker-owned capability sets. The egress executable has no transport yet; P2-G2 owns the sole future outbound socket.
- `academic` CLI: `admission verify|show`, `daemon serve|status`, `doctor` (with `--profile`/`--deep`), `ingest`, `export`, `backup`, `restore`, `crash-replay`, and `fixture emit|verify|replay`. Every path prints its receipt-derived posture before human results and repeats it as the JSON `policy` object; the present unprovisioned key keeps that posture synthetic. Exit codes distinguish policy denial, conflict, repair-required, incompatible, unavailable, and internal failure. See [the CLI contract](docs/contracts/phase1-cli.md) and [admission receipt contract](docs/contracts/admission-receipt.md).
- `academic-record`: the section 10 attempt ledger and the first two §28 engines. Every attempt is preserved — `AttemptHistory` has one mutator, no removal path, and a correction is a new entry carrying ADR-003's `SUPERSEDES` — and a `CourseAttempt` exists only where a confirmed registration or `academic-transcript`'s user-confirmed row does. Requirement classification is a versioned rule-engine output with no public constructor, asserted under `DeterministicEngine` authority that ADR-003's actor matrix refuses to a user. The 2015-spring repeat ceiling and the post-2004 external-grade exclusion are effective-dated policy rows rather than constants, and the repeat-recognition rule no official source states is `UNKNOWN` rather than a default. `engine.gpa` and `engine.credit.accounting` are the two registry entries this brings to `IMPLEMENTED`; both are pure functions of `(frozen_inputs, rule_set_hash, engine_version)`, both publish a value only when it is fully determined, and their harness corpora under `testdata/engines/` are executed and byte-compared rather than merely counted. All arithmetic is exact over `academic_domain::Decimal`: no `f32`, no `f64`, and no floating-point literal appears in the crate, and the shipped corpus averages `33.9 / 12` — exactly `2.825`, a tie `f64` cannot represent — so a float would fail the fixture rather than pass it silently. Expected averages come from `tools/gpa-oracle.mjs`, an independent transcription in another language. See [the GPA and attempt contract](docs/contracts/gpa-and-attempts.md).
- `academic-repository`: `P2-R1`'s repository snapshot boundary. Section 17.1's eight inputs — local directory, GitHub public, GitHub private, archive, branch, commit, dirty working tree, spec-only project — each produce a read-only snapshot, and a total mapping carries them onto section 17.2's four `sourceType` values, so **a dirty working tree resolves to `DIRTY_WORKTREE` however it was named**. Section 17.3's permission and secret gate runs before inventory and before indexing, held three ways at once: `AdmittedPaths` and `Inventory` have crate-private constructors so no outside stage implementation can skip one, an admission carries the digest of the request it was decided for so it cannot be reused, and `secret_gate_precedes_indexer` wraps the real stages in a spy whose index count on a blocked source is zero. A secret file's digest is never computed without a recorded `DisclosureDecision`, and migration `0012` refuses the same shape in a row a process this repository did not write inserted. GitHub access is repo-scoped, read-only and short-lived, held by `P2-K1`'s `DeviceKeystore`; **no implementation of the reader trait ships and no network call is made**. Repository bytes are `P2-G5`'s `Untrusted<IngestedDocument>`. See [the repository snapshot contract](docs/contracts/repository-snapshot.md).
- `academic-repository-analysis`: `P2-R2`'s static analysis over a frozen snapshot. Section 17.3's third stage — AST, symbol, call and data flow, schema, config and IaC indexing — and section 17.3's own tier table over the result. **Section 17.3's five observations fold onto three tier values and which row becomes which is the contract**: manifest presence is `PRESENT_ONLY`, an import with no reachable use is `POSSIBLE`, and the last three rows are `OBSERVED` — a reachable call plus configuration, a use that is *nowhere but* tests, and a runtime trace agreeing with production configuration, which differ by the scope and the strength they carry rather than by tier. An `OBSERVED` finding holds a `DisplayedConfidence`, which only `P2-M1`'s calibration registry issues, so a rung that would be observed without a fresh dataset is refused rather than shown with an uncalibrated number. **A finding cannot be repository-wide**: `FindingScope` has no such variant, `ComponentId` refuses every spelling of the root, `Finding` has one crate-private constructor counted at one call site, and evidence spanning three components produces three findings. Vendored, generated and example paths never promote and a site in another package of a monorepo corroborates nothing; both are kept on the finding as labelled exclusions rather than dropped. Coverage is total by construction — one outcome per index kind per manifest path — so a file this analyzer has no reader for reports a typed gap instead of silence. It opens no file and no socket, admits **no parser dependency**, and holds no text lifted out of a repository: a declaration is a symbol fingerprint, and a canary corpus observes that no analyzed byte reaches an accessor or a `Debug`. See [the repository static-analysis contract](docs/contracts/repository-static-analysis.md).
- `academic-repository-correlation`: `P2-R3`'s cross-artifact correlation and drift lanes, built directly on `P2-R2`. Section 17.5's typed evidence relations are **compared against the design document rather than counted**: the acceptance suite reads section 17.5's own bullet list and requires the enumeration to equal it in both directions, and requires each relation to be produced by a corpus rather than only declared. Section 30.3's rows four and five are two questions and this crate does not mix them — `academic-ledger` already holds those rows' rank tables, so what is added here is the qualifier a rank table cannot carry: direct evidence about **another** snapshot, and a draft, deprecated or superseded specification, are each admitted at rank zero rather than at their apparent authority, and neither lane lends the other any. **A conflict overwrites nothing**: `INTENDED_NOT_IMPLEMENTED` and `IMPLEMENTED_NOT_DOCUMENTED` are records beside both lanes' edges, each lane still answers from its own evidence, and correlating again with more evidence appends. Section 17.5's four drift scopes — deprecated spec, feature flag, undeployed code, branch difference — are four independent qualifiers with four different payloads, because they can hold at once. Section 19's dependency and semantic channels are separate types with no accessor returning their union, and a difference is attributed to the one axis that moved: same bytes under two analyzer builds is `ANALYSIS_CHANGED`, a moved snapshot under one analyzer is a code change, and a comparison that moved both is **refused** rather than displayed as either. It opens no file and no socket, holds no analyzed byte, and inventories **every field of every type it declares** in both directions. See [the repository correlation contract](docs/contracts/repository-correlation.md).
- `academic-repository-classification`: `P2-R4`'s section 18 classification, built directly on `P2-R2` and `P2-R3`. **`REQUIRED` is five types, not five checks**: section 18.2's `current code/goal → concrete responsibility or failure scenario → mechanism that controls it → required concept → user's insufficient/uncertain evidence` is a chain in which each step is the next constructor's argument taken by value, so a chain with a step left out is a program that does not compile — seven committed diagnostics say so — and the untyped door a model proposal arrives through names the first missing step with its own code. The fifth step **has no `SUFFICIENT` value**: a user whose evidence is applied, fresh and confirmed produces no gap, so their chain cannot be closed at all. A **whole field cannot be required**: section 7.4's `FIELD` and `ALIAS` tiers are refused by the one constructor, a chain's first step is a `P2-R2` finding or an *approved* `P2-R3` specification rather than a project label, and one chain yields one concept. `WOULD_BENEFIT_FROM` takes at least one trigger, the current trigger state, a benefit dimension and at least one trade-off in one constructor, so a generic list of technology names produces **zero** findings rather than findings a later layer filters. `OBSERVED` and `REQUIRED` coexist because the observation is its own field; `REQUIRED` and `WOULD_BENEFIT_FROM` cannot, because the forward-looking half is one slot, and offering both in one goal scope is refused rather than resolved. A classification is bound to a snapshot **and** a goal version, and the materialized `ProjectConceptRequirement`'s identity is a digest of those facts rather than a truncation of them. A user override opens a `ClassificationConflict` beside both sides and rewrites neither, and survives the next capture because it is keyed on the goal rather than the snapshot. Locator migration keeps the original finding whole and produces **one record per original locator, positionally** — two byte-identical locators are two records, which is the identity-from-content collapse `P2-A1`'s fifth audit found one step away. It opens no file and no socket, holds no analyzed byte, and inventories **all 105 fields of every type it declares** in both directions. See [the repository classification contract](docs/contracts/repository-classification.md).
- `academic-knowledge-state`: `P2-N2`'s section 13 knowledge state, built on `P2-N1`, `P2-M3`, `P2-L4` and `P2-R4`. **The six levels are not declared twice**: `academic_domain::MasteryLevel` already holds them, and what this crate adds is section 13.1's own row order plus an ordinal whose `match` has no wildcard arm, so a seventh level is a compile error. The count is a **measurement**: `mastery_enum_is_exactly_six_ordered` reads section 13.1's table out of the specification and compares it in both directions, and the same test does it for the five facet keys, `evidence_ceilings_are_never_exceeded` for section 13.2's eight rows with both of their text cells, and `eligibility_four_checks_block_with_reason_codes` for section 13.4's four questions. **An automatic projection cannot reach `FLUENT`** — `AutomaticLevel` has five variants and no such value, so the level section 13.1 reserves for a user confirmation cannot be named on that path; `FLUENT` is reachable only through a `FluentAuthorization` that takes repeated independent cross-context evidence **and** a verified `UserConfirmation` by value, and a `UserConfirmation`'s one constructor runs ADR-003's actor matrix, which every automatic actor fails. **A course grade cannot promote a concept** because `CourseGradeSignal` has no concept field and no `ConceptEvidence` variant: there is nowhere to write the concept down. **An installed dependency promotes nothing** but is retained and shown, and the difference between it and authored project code is `P2-R4`'s `ObservedProof` rather than a heuristic here. **`UNSEEN` is not a failed test**: nothing-recorded and tried-and-did-not-succeed are two different projections with two bases, and the copy shown for either is the specification's own sentence. `estimateConfidence` is evidence sufficiency and **not a skill score** — the type is not orderable, converts to no mastery level, and always carries what is missing. An assertion has no setter, no public field and no `&mut self` method; a revision is a new version whose identity is a length-prefixed hash chain binding its predecessor, and a retraction appends a row and recomputes only the projection. A user-confirmed state is immune to an AI adjustment **in both directions**, which opens a review card carrying both sides and `P2-M3`'s own `NEW_EVIDENCE_CONFLICT`. It opens no file, opens no socket and **reads no clock**, which is what makes "time never demotes mastery" a property of the whole package; it persists nothing and adds no migration, and inventories all 134 fields of every type it declares in both directions. `GATE-38-023` and `GATE-38-025` are left open. See [the knowledge state contract](docs/contracts/knowledge-state.md).
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
cargo test -p academic-desktop --test compile_fail --locked --offline
cargo test -p academic-repository-classification --test compile_fail --locked --offline
cargo test -p academic-knowledge-state --test compile_fail --locked --offline
cargo test -p academic-repository-competency --test compile_fail --locked --offline
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
cargo clippy -p academic-capture-gate --all-targets --locked --offline --features native-capture -- -D warnings
cargo test -p academic-capture-gate --all-targets --locked --offline --features native-capture
cargo clippy -p academic-capture --all-targets --locked --offline --features phase2-fault-injection -- -D warnings
cargo test -p academic-capture --all-targets --locked --offline --features phase2-fault-injection
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

Hosted CI materializes **22 required jobs**: one source preflight, five
`rust-default-*` jobs, five `rust-store-*` jobs, five `rust-features-*` jobs,
two Phase 1 exit jobs, three Linux-only encrypted/rotation jobs, and one pnpm
contracts job. A green run is therefore reported as **22/22**, and the measured
duration-to-timeout ratios and refresh rule live in
[the CI budget record](docs/development/ci-budget.md).

The block above runs the whole workspace in one `cargo test`; hosted CI runs the
same default-feature coverage as two jobs, `--workspace --exclude academic-store`
beside `-p academic-store`. That is a timeout split and not a coverage
difference — `academic-store`'s file-backed tests are the largest single share
of that lane's runtime on Windows — and `pnpm verify:contracts` fails if the two
package sets stop being complementary or if the store job stops running on all
five labels.

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
platform set, and one exact posture emitter for CLI, IPC, and export. The user's offline acceptance public key has not been
provided, so the typed compiled key state is unprovisioned; the committed
candidate receipt also has only its Windows x86-64 and Linux x86-64 rows. Both
conditions fail closed, `production_data_allowed` remains `false`, and ADR-002
remains unaccepted. The exact receipt shape and provisioning boundary are in
[the admission receipt contract](docs/contracts/admission-receipt.md).

`P2-X1` adds the desktop shell's contracts: `packages/ui` holds the route
manifest, the command palette, backlink traversal, the persistent right-hand
evidence drawer and the optimistic-update typing; `crates/desktop` holds the
typed local-core command allowlist and the sealed `Optimistic<T>`; and
`crates/desktop/tauri.conf.json` with `crates/desktop/capabilities/` is the
committed Tauri capability and CSP snapshot, validated against Tauri's own
published config schema and against the schema generated from
`tauri_utils::acl::capability::Capability`, both vendored under `schemas/tauri/`.
`route_manifest_matches_ia_exactly` parses section 25.1's tree out of the
specification and compares it with the manifest in both directions.
**No Tauri runtime is linked and no window opens**: `cargo metadata` measures 388
new packages in the default product closure for `tauri 2.11.5`, six of them on
the list `phase1_default_features_have_no_product_network` forbids, at every
feature setting. That measurement, what the snapshot is and is not evidence for,
and the three-way graph/link/source boundary that keeps this surface away from
the database and the keys are in
[the desktop shell contract](docs/contracts/desktop-shell.md).

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

`P2-U2` adds `academic-requirement`: section 11.2's typed rule DSL, section
11.1's immutable versioned `RuleSet`, and the reviewer gate a model-extracted
candidate has to pass before anything executes. The rule types are **fourteen**,
not the thirteen `t068` says — its own parenthesis lists fourteen, its own
acceptance evidence names fourteen `dsl_*` tests, and `t001` derives fourteen
consecutive requirements, `REQ-11-004`–`REQ-11-017`. Nothing in the crate
asserts a count: both of the specification's readings are parsed back out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compared in both
directions.

The gate is a type. `ReviewedRule` and `ExecutableRule` have private fields and
one construction site each — `ReviewGate::admit`, which takes its two
attestations as two parameters and requires two different people, and
`RuleSetDraft::include`, which takes a `ReviewedRule` by value and *evaluates*
both fixture classes against the rule before admitting it. A candidate reaching
an audit is five compiler diagnostics, in `crates/requirement/tests/compile_fail/`.
The crate's dependency closure contains no crate that runs, wraps or transports a
model call, and its one free-text value sits on the one type that never reaches
an evaluation, so `production_audit_no_llm` is a fact about the graph and the
field inventory rather than a check inside a function. `GATE-38-011`,
`GATE-38-012`, `GATE-38-015` and `GATE-38-016` stay open and each reads
`UNKNOWN`. Migration `0015` holds the typed rows, with the review key and the
supersession `UNIQUE` as the second layer. It runs inside `cargo test
--workspace` and adds no external package to `Cargo.lock`. What it does and does
not claim is in
[the requirement rule DSL](docs/contracts/requirement-rule-dsl.md).

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

`P2-G6` adds `academic-consent`, the consent ledger and the section 3.7
capture-permission model. The default is the absence of a record rather than a
permissive base: a new ledger holds nothing, `CaptureStatus::Unknown` is what a
scope with no record resolves to, and `mint_capture_capability` refuses on it.
A user attestation and a written authority are unrelated types — there is no
conversion between them, a `trybuild` case is the program that tries, and a
workspace-wide signature rule refuses one written anywhere else. Audio and
transcript retention are two bounds with no accessor returning one for the
other, and a derivative inherits the stricter of each independently. A legal
exception leaves the system as an `ExternalReviewTask` with no resolution API
and comes back as nothing. An expiry is applied only through the preview it
describes, at the instant that preview was taken.

Migration `0006_phase2_consent_and_capture.sql` carries the aggregate's typed
columns — section 3.7's `(offering_id, permission_seq)` key, its status and
authority vocabularies, four retention columns for two independent axes, and the
seven-dimension checklist — under the same append-only triggers and authorizer
coverage as every other canonical table. Every `CHECK` list in it is compared
against the Rust `as_str` spellings it mirrors. Nothing writes those rows yet:
`P2-L1` owns the daemon evaluation and the device layer. `GATE-38-009` and
`GATE-38-019` stay open per offering and per term, and an unfilled cell keeps
the recorder disabled. What the boundary does and does not claim is in
[the consent contract](docs/contracts/consent-and-capture-permission.md). It
runs inside `cargo test --workspace` and adds no package to `Cargo.lock`.

`P2-L2` adds `academic-capture`, the desktop host's one-action
Record/Capture/Mark surface. `begin` is the whole of starting a capture —
permission, effective policy row, preflight, clock, journal — and a refusal at
any of them leaves nothing on disk. **One monotonic session clock is shared by
audio and image capture, and the sharing is a type**: `SessionTick` has no
public constructor, carries the domain of the clock that minted it, and an
anchor offered from outside is admitted through that clock and refused if it
came from another. An image's audio-clock offset is therefore a subtraction
inside one domain rather than an estimate between two, and the instant and the
offset agree exactly. The chunk journal is one append-only file of chain-
digested frames that survives a lost connection and a killed process; a Mark
Moment stores a bare instant and a label appended later never moves it; a drift
past the effective tolerance is `ALIGNMENT_LOW_CONFIDENCE` with ±seconds rather
than a refusal; and a two-anchor realignment appends a mapping version and edits
none. The four thresholds are fields of an effective-dated policy row, not
constants. **Nothing here records anything**: no device is opened, no clock is
read, every chunk is a committed literal, and the journal frames are plaintext
under the current posture. Which device is an authorized recorder is section
12's open product question and Phase 2 ships the desktop host only. The frame
layout, the fault-matrix rows and the four open items are in
[the capture subsystem contract](docs/contracts/capture-subsystem.md). It adds
one workspace member and no package to `Cargo.lock`; its `CP05` failpoints are
behind the non-default `phase2-fault-injection` feature in the block above.

`P2-M1` adds `academic-model-run`, the boundary every model execution is
recorded and every displayed confidence is interpreted at. A `ModelRun` carries
the twelve section 27.3 fields — including the transmitted byte ranges and the
redaction-policy hash — as twelve distinct constructor arguments, so a run that
omits one does not compile, and `model_run_requires_every_field` parses the
spec's own YAML block rather than transcribing it. Migration `0007` fills the
place migration `0004` left for this aggregate's typed columns; the two list
fields get child tables, and a reanalysis appends a candidate beside the earlier
one rather than editing it, on ADR-003's existing mechanism. A provider's raw
score implements no ordering trait, hands back no number, and prints none, so
ranking two providers' numbers is a compile error rather than a convention;
`CalibrationRegistry::interpret` is the only producer of the calibrated value
the display surface accepts. The reconciliation against `egress_audit` keys on
`egress_consumption` rather than on that table's polymorphic `grant_id` column,
so a process-capability token cannot be mistaken for the egress grant a run
spent; what that establishes is in
[model-run provenance](docs/contracts/model-run-provenance.md). It runs inside
`cargo test --workspace` and adds no external package to `Cargo.lock`.

`P2-M2` adds `academic-proposal`, the boundary between what a model proposed and
what this system will record. A `Proposed<T>` implements no unwrapping trait and
has one crate-private accessor, whose three call sites are inventoried by name
with a written reason for each; the crate declares no edge to `academic-store`,
so naming the canonical writer from a case compiled against it is a compile
error. Section 27.4's four risk tiers each map to one workflow and one queue
door, and a proposal handed to another tier's door is refused by the single
comparison all four doors call — the sixteen-cell permutation is run and exactly
the four on the diagonal are accepted. An autosaved record's epistemic status is
a constant equal to `AI_INFERRED` rather than a field, a high-risk approval takes
a receipt carrying the proposal's identity, and a non-delegable proposal is
reachable only through a receipt that `academic-domain`'s closed `Actor` enum
issues for a user and nobody else. Dispositions are that crate's frozen
`DecisionAction` and not a second vocabulary: **three, not the four the execution
plan's test name claims** — section 3 of the spec names approve, modify and
reject and no fourth, and pending is a queue state with no conversion into a
decision. An undo appends a record naming the one it reverses, so a rejection and
its reversal both stay. Migration `0009` holds the same rules in SQL for a writer
that skips the Rust boundary. What the boundary does and does not claim, and
where the execution plan's section references do not resolve, is in
[the proposal review queue](docs/contracts/proposal-review-queue.md). It runs
inside `cargo test --workspace` and adds no external package to `Cargo.lock`.

`P2-U6`'s official-source ingestion lives in `academic-ingestion`. Section
29.1's nine stages are nine types whose argument chain makes the order a compile
error to break, and each stage's failure is exercised on its own so that a run
that failed publishes nothing. A connector declares section 29.1's nine fields
and `build` refuses each one dropped in turn; a fetch target is `&'static` and a
credential binding has no public constructor, so a link found inside a fetched
page can become neither. A document whose effective date is missing is
`UNSCOPED_OFFICIAL_SOURCE` and cannot be published, because the publisher's
argument type has no value for it. Competing sources are compared on section
8.4's five dimensions — legal hierarchy, issuance date, effective date, target
scope, transitional measures — and **no function anywhere reduces those five to a
winner**: a `ConflictCase` stays `INDETERMINATE` until a person decides. Every
denial offers the same four fallbacks and routes nowhere else. The crate holds no
transport, persists nothing, adds no migration, and reuses `P2-G5`'s
`Untrusted<T>` as the one public route to a snapshot's bytes. `GATE-38-020` and
`GATE-38-027` stay open, and Phase 2 ships manual import and user-provided export
with no browser automation module. What it does and does not claim is in
[official source ingestion](docs/contracts/official-source-ingestion.md). It runs
inside `cargo test --workspace` and adds no external package to `Cargo.lock`.

`P2-L3` adds `academic-transcription`, section 12.3's provider-neutral pipeline.
A job's inputs are only authorized chunks, captures and explicitly supplied
materials, and **the binding comes from the capture rather than from the
journal**: `AuthorizationBinding::of` takes the `CaptureRecorder` that
`academic_capture::begin` returned — a value with no public constructor — and
refuses a journal whose header names another capability token or policy row.
Reading the token out of the journal instead would have compared a synthesized
recovery with itself, because `ChunkJournal::replay` is public. A
provider declares eight technical facts before it may be used — the four privacy
ones stay in `P2-G3`'s registry — and an omitted declaration is refused while a
declared *absence* travels with the contract and blocks the feature that depends
on it. The route has three arms and no fourth: **local is the default for raw
audio, remote needs all three of `REQ-32-040`'s facets for that exact provider
and model version, and everything else is blocked**; a new profile approves
nothing, so an unconfigured request never falls through to remote. Every raw
provider response is retained under `P2-G5`'s `Untrusted<T>` and leaves the
archive in no other form, and its bytes have one crate-private accessor whose two
call sites are inventoried. **Nothing writes a raw token**: every field of the
three raw types is private, so a struct literal for one is a compile error
outside the decoder's module, and a correction is a new version over an
annotation layer that leaves the raw token digest identical. A correction is one
of `P2-M2`'s three dispositions — a rejection has no constructor at all — and two
providers' results are diffed with a digest that does not depend on which was
passed first, because `P2-M1` forbids ordering them. Every run records `P2-M1`'s
twelve section 27.3 fields and this crate adds no provenance of its own.
**Nothing here records or transcribes anything**: no `SttProvider` implementation
ships, every fixture is a committed literal, and the crate opens no socket, reads
no clock and touches no file. `GATE-38-019` stays open and this task invents no
default for it. What it does and does not claim is in
[the transcription pipeline contract](docs/contracts/transcription-pipeline.md).
It adds one workspace member, no external package to `Cargo.lock`, and no
migration, and it runs inside `cargo test --workspace`.

`P2-L4` adds `academic-lecture-document`, section 12.5's lossless document and
section 12.6's deterministic coverage validator. **Exactly one status per
segment is held by the type**: `SegmentStatus` has four variants, a
`SegmentAccount` has one field of it, each non-mapped variant carries its
evidence by value, and there is no `mapped` constructor at all — `MAPPED` is
derived from the document, so a coverage number cannot be asserted rather than
measured. The one property that is genuinely about two inputs is a total `match`
whose fourth arm refuses, so a report that exists partitions its segments.
**`INCOMPLETE` is the only value with no measurement behind it**: the rendering
starts there and is replaced only by a `CompletenessWitness` whose one producer
is the report, and there is no completeness parameter and no setter. The nine
preservation transforms are the specification's own sentence read out of it, and
the rule that catches a deletion or a paraphrase does not read the transform's
name at all — **every token a mapping covers has to still occur, in order, in
the rendered text**, under all nine. Nothing here can shrink the coverage
denominator: the eligible set is walked off the transcript rather than taken as
an argument, neither preservation type offers a method that returns less than it
holds, and `Salience` lives only in the `StudyIndex`, which is a distinct type
carrying a disclosure it has no setter for. `TRANSCRIPT_COVERAGE` flips from
`PLANNED` to `IMPLEMENTED` with a corpus that is executed and byte-compared, not
merely counted. **This crate names no raw type**, so `P2-L3`'s workspace scope
rule holds unweakened. What it does and does not claim is in
[the lecture document contract](docs/contracts/lecture-document.md). It adds one
workspace member, no external package to `Cargo.lock`, and no migration, and it
runs inside `cargo test --workspace`.

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
