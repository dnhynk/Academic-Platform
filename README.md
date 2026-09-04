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
- `academic-freshness`: `P2-N3`'s section 13.3 freshness, built on `P2-N2`. **The six bands are not declared twice**: `academic_domain::FreshnessBand` already holds them, and what this crate adds is the order section 13.3's own sentence names them in plus a rank, a token and a step-down whose `match` has no wildcard arm, so a seventh band is a compile error. Both counts are **measurements**: `freshness_bands_are_exactly_six` reads section 13.3's band sentence and its seven-bullet input list out of the specification and compares each in both directions. **Time takes nothing away.** Section 1's fifth invariant is held by a missing vocabulary rather than by a rule: this crate has no name for a `MasteryLevel`, so `decay` takes an elapsed span and a persistence window because those are the only things it can take, and the one value it hands `P2-N2` is a `FreshnessInput` carrying a band and a confidence. `the_freshness_crate_cannot_name_a_mastery` compares its 39 `use` items, 4 reached paths, 2 macros and 8 re-exported modules against pinned inventories in both directions and refuses eight workspace spellings of a mastery level, **with a control**: the same reader must find five of those eight in `P2-N2`'s own ladder. **Ineligible evidence cannot freshen a concept** — a `DatedEvidence` wraps an `EligibleEvidence` and has no other constructor, so section 13.4's four checks bind this axis too. **Spillover is one hop, cited, and weaker than direct use**: a `NeighborUse` is built from a neighbour's own dated evidence and from no band, projection or contribution, it refuses a dated item linked to any other concept — which is the two-hop route that survives every other limit — the edge must be one of four section 7.2 predicates carrying the evidence section 7.3 requires, and the contribution is a band strictly below the neighbour's own and at or under a ceiling, combined with the higher of two rather than summed. **A recall failure caps rather than votes**: while it is at least as recent as every raiser, no raiser lifts the band, and relearning after it does. **An AI cannot say the user still remembers** — `RecallStatement`'s one constructor runs ADR-003's actor matrix. The prior is versioned, named `UNCALIBRATED_PRIOR_V1`, carries `NO_EVIDENCE_BASIS_ESTABLISHED`, stays identifiable after calibration, and its personalization speed has **no default at all**, so `GATE-38-024` stays open on both halves. It opens no file, opens no socket and reads no clock; it persists nothing and adds no migration. See [the freshness contract](docs/contracts/freshness.md).
- `academic-student-voice`: `P2-L5`'s section 32.5 student-voice boundary, built on `P2-L4`, `P2-L3`, `P2-L2` and `P2-G6`. It is about the people in the room who never agreed to this product, so every rule in it is fail-closed and the one that governs the rest is that **an automatic editing claim needs a measured number**. The diarization figure is a run over a named, versioned, digested corpus — `student-voice-diarization` v1, six synthetic cases under `testdata/diarization/`, executed and byte-compared and re-rendered from one builder — and `DiarizationMeasurement` has private fields, no `Default` and one producer, so a figure quoted from anywhere else has no value of that type. The shipped corpus measures **967 permille attribution accuracy and 33 permille of student speech labelled instructor**, and the recorded default threshold is 990 and 0, so **this build clears neither axis and makes no automatic redaction claim at all**: the only plan a profile can build is a manual one whose every exclusion a person decided. That is the type rather than a check — `RedactionMode::Automatic` carries an `AccuracyWitness` **by value** and a witness's one producer compares a measured permille against a configured one — and configuration cannot empty it, because `DiarizationThreshold::new` refuses a required accuracy below 900 permille or an allowed missed-student fraction above 50. **`RedactionScope` has one variant and it is the derivative**, so a policy authorising an edit to an original recording has no spelling here at all; `academic-retention` holds that mechanism behind an `OriginalVoiceAuthority` this crate never produces and never names. One redaction produces **two** values: a derivative holding the speaker and span of every removed utterance and **no text**, and a `RestrictedOriginal` holding the text with **no accessor**, reachable only against a `RawAccessGrant` that is spent by being moved and that writes its audit row inside the call. A capture holding a student's face, a roster or a personal screen has **no byte accessor at all**, the one type that has one has no public constructor, and the acceptance row drives the real dispatch against a real stage and observes zero calls. Every derivative's retention comes from `P2-G6`'s one inheritance function, called at one site and walked over all 256 pairs of a bound grid and a three-link chain; the deletion preview is `P2-G6`'s own with section 32.5's concept and evidence projections listed on top, and every deleted object is either cited by a listed projection or listed as unreferenced. Every ratio is permille in `u64` — no `f32`, no `f64`, no decimal literal anywhere in the package. It persists nothing, adds no migration, opens no socket, opens no file and reads no clock. **`GATE-38-026` is partially discharged**: the accuracy question is answered with a measurement and the default is fail-closed derivative-only redaction, while whether student voices may be removed from the **originals** stays an open decision for the user and their institution. See [the student voice contract](docs/contracts/student-voice.md).
- `academic-export`: `P2-P1`'s section 37 graduation export and the vendor-free restore that reads it, built on `P2-K4`, `P2-U7` and `P2-L4`. `INV-C-015` is the claim that a user can read their own record when this product and their school account are both gone, so the **dependency list is the first half of the evidence**: this crate links no store, vault, crypto, keystore, recovery, retention, projection engine or transport, `read_bundle` takes a path and no key, token, host, account or provider, and there is no argument to pass one as. The bundle carries section 37's **six named parts**, and nothing asserts the number six: `graduation_bundle_contains_all_six_named_parts` parses that list out of the specification and compares it with the enumeration in both directions. The sixth part is not a selection — its subject is the graph, so it carries the canonical state whole and the five topical parts are views over it, which is what makes the assignment total without inventing a seventh part the specification does not write. Every file carries section 32.10's **three attributes**: the sharing restriction is a total function of the sensitivity label so the two cannot disagree, the label is recorded per security domain and refused if it is weaker than that domain's own artifacts, and the source copyright notice is recorded with **no fallback string**, so an export over a domain nobody stated terms for fails closed. **Originals are a user choice with no `Default`**, and a withheld original keeps its identity and digest and names no path, so nothing in a published bundle points at a file the bundle does not carry; an artifact is addressed by its own identifier everywhere, because two artifacts with identical bytes share one vault locator. Two bundles of one watermark are **byte-identical whole-file**, manifest included: the generation instant is a parameter and the one clock the crate reads names a staging directory the publish rename removes. The restore **re-runs** `P2-U3`'s audit rather than re-reading it — it re-performs section 11.1's selection from the recorded scope and the profile decoded out of the frozen inputs, then byte-compares the engine outcome — and the published rule set comes from the caller, never minted from the bundle, so `P2-U2`'s review gate has no way around it. Section 32.10 names one format this build cannot write: **there is no PDF**, because nothing here produces PDF bytes, and the bundle states that absence rather than shipping an empty file. See [the graduation export contract](docs/contracts/graduation-export.md).
- `academic-gap`: `P2-N5`'s section 15 gap engine, built on `P2-N2`, `P2-N3` and `P2-N4`. **A gap is a refusal before it is a report**: section 15.1 says a gap is not a low knowledge state but an evidence-backed prerequisite deficit blocking an active goal, and no function here produces a `GapCase` without an `ActiveGoal`, which cannot be declared without success criteria — `GoalCriteria` has no `Default` and its one constructor refuses an empty list, so step 2's expansion has nothing to run from until step 1 has happened. Three counts are **measurements** read out of the specification in both directions: section 15.2's five gap kinds with both of their text cells, its third step's four overlay dimensions, and section 15.3's eight explanation fields. Section 15.2's **sixth step names four informal kinds where its table has five rows**; the table is normative, the four are kept beside it, and the discrepancy is recorded rather than reconciled. **Traversal is `P2-C4`'s registry, not a list here** — `prerequisite_descriptor` refuses eighteen of section 7.2's twenty predicates including `RELATED_TO`, and the descriptor is what says `REQUIRES` never carries `HELPFUL` and `BUILDS_ON` never carries `HARD`. A **weak `BUILDS_ON` has no rung at all**, so the descent has nothing to cross; it becomes section 15.2's `CONTEXT_GAP` only when two or more of them leave one node and no criterion chooses. **Exactly one of the five kinds is a `강한 부족`**, read off the table's own meaning column: the other four say the person may know it, used to know it, that the graph is wrong, or that the goal has not decided. **A tie is retained, never resolved** — every root at the shallowest depth is returned, a case with tied roots and no diagnostic is refused, and there is no `primary_root`. **The specificity validator holds no words**: section 15.3's `데이터베이스를 더 공부하세요` is refused because `Database` is a `FIELD`, which `P2-C3` says carries no independent prerequisite of its own, and because the advice cites no evidence, states no duration, names nothing to read and links neither a lecture nor a project — eight structural facts and no text comparison anywhere in the crate, which a whole-text pin and a whole-set comparison of every non-ASCII literal against the specification's own cells both observe. **One concept's evidence never becomes another's**: the overlay takes the inputs to section 13.4's four checks rather than a ready-made projection, guards the admitted and blocked halves separately, and the descent refuses a freshness band a neighbour **on the blocking path** raised — the two-hop route `P2-N3` reported, arriving on the very edges section 15.2 step 2 descends. It opens no file, opens no socket and reads no clock; it persists nothing and adds no migration. See [the gap engine contract](docs/contracts/gap-engine.md).
- `academic-critical-path`: `P2-N6`'s section 16 critical path engine, built on `P2-N5` and `P2-C5`. **The answer is a set of undominated routes, not a score** — section 16.1 says the critical path is not the shortest path, and folding the answer into one number would give the product authority it has not earned. So the **seven-component cost vector** and the **five-component benefit vector** have private fields, one accessor per named axis, and **no `total`, `sum`, `score`, `weighted`, `Ord` or numeric conversion anywhere in the crate**, which a whole-crate scan for twelve folding spellings and a derive-attribute read both observe. Seven, five, **eight** constraints, **five** disclosure groups and **four** path roles are each measured against the specification in both directions. **An unknown cost is a range structurally**: `CostEstimate` hands out `low` and `high` and nothing between them, and an unmeasured estimate with `low == high` is refused at the constructor, so no route can fold to a point. **Elimination is a type, not a comment** — `ParetoFront::eliminate` is the only constructor of a front and `rank` takes nothing else — and **a preference is a permutation of all twelve axes, never a weight vector**, so no arithmetic combines two axes; `PreferenceSlider` has no `Default` and `Ranking` borrows its front, so a slider cannot reach a fact. **Satisfaction is not a walk**: every member of a `REQUIRES ALL` is required and a `REQUIRES ONE OF` branch is taken whole, and the naive node-count answer is implemented **once**, unreachable from the engine, so the acceptance row compares the two and shows they differ. **A course is an acquisition option** with no function returning a mastery, a state or a satisfied concept. Section 16.3's **eighth constraint is the checkpoint rule**, and section 16.2's four strategy names are introduced with `같은`; both readings are recorded with tests that fail when the document stops saying so. Section 16.5's five groups are five private fields taken by one constructor with no `Default`, and the three that may be empty **name their reason** instead of showing an empty list. A user's relation edit recomputes and keeps the **original** base across a chain of edits. It **registers no thirteenth deterministic engine** — section 28 tabulates twelve and none is a critical path engine — and proves determinism in `P2-C5`'s own vocabulary over its own corpus at `testdata/critical-path/`. It opens no socket, opens no file and reads no clock; it persists nothing and adds no migration. See [the critical path contract](docs/contracts/critical-path.md).
- `academic-competency`: `P2-Y1`'s section 24.1 competency model and section 24.3 evidence rubric, built on `P2-N2` and `P2-R5`. **`knows X` has no constructor**: there is no `statement` argument anywhere in the crate, `Competency::statement` renders section 24.1's sentence from the context, the criteria and the rubric, and `Deserialize` is `try_from` and refuses a document whose `statement` is not the one its own parts render — so a hand-written sentence cannot ride in through JSON either, whatever it says. A competency with no context, no criterion, or a criterion no rubric row witnesses is a value that cannot be built. **A concept is not a competency**: `ConceptRef` and `CompetencyId` have no conversion in either direction, and `ConceptRef` carries the namespace that named it as part of the value, so `P2-R4`'s classification token spelled out as `P2-N1`'s identifier — byte-identical text — is still not that identifier; this crate resolves neither namespace into the other. The enabling relation is **one stored forward list with two query views** and no reverse row, which is section 7.2's own `반대 edge를 중복 저장하지 않는다`, and its two qualifiers are **measured** against `PredicateName::EnablesCompetency`'s descriptor in both directions. The six evidence stages are a **measurement** too: `six_evidence_stages_are_distinct` reads section 24.3's own sentence out of the specification and compares the backticked names against the enumeration in both directions. They are **not** section 13.2's six promoting rows — that is a coincidence of counts, and a total map would have to invent three of its six answers. **Using a dependency settles no cell**: `PromotingEvidence` refuses the section 13.2 rows that license no promotion, `EvidenceSource` has no arm for `P2-R5`'s `ProjectObservationClaim`, and the join has one key — the criterion's own concepts — with no arm that falls back to the competency's enabling set, because a criterion that names no concept has no representation. It opens no file, opens no socket, **reads no clock**, persists nothing and adds no migration, and inventories all 55 fields of every type it declares in both directions. It leaves no `§38` gate open. See [the competency contract](docs/contracts/competency-model.md).
- `academic-role-profile`: `P2-Y2`'s section 24.2 role bundle, versioning and fork, built on `P2-Y1`. **A role name is not a market truth**: there is no function from `RoleLabel` to `RoleDirection` and none back, so `Backend Engineer` is never read as a direction — the direction is a field the user set; `BundleShelf::by_label` returns a `LabelReading` over a list and **never one bundle**, carrying a `LabelAmbiguity` that names the distinct lineages and scopes when a label reached several; and the shelf is keyed on the lineage-and-version pair, so `shelve` **refuses** an occupied key and one organisation's bundle cannot displace another's. **The identity is a pair, not a name**: section 24.2 writes `id: backend_engineer_profile_v4`, which folds a lineage and a version into one string, and `P2-R4` measured what a folded identity collides — so `RoleProfileRef` is the pair, `rendered` writes that spelling for a reader, and there is no parser back, no `FromStr` and no `TryFrom<String>`. The version itself is **read from the registry**: `RELEVANT_TO_ROLE`'s one required qualifier is `role_profile_version`, of kind `PositiveInteger`, compared in both directions — which is also what fails if a later registry grows the importance qualifier section 24.2 has and section 7.2 does not. **An edit is a new version and a fork is a new lineage**: nothing in the crate takes `&mut self`, `BundleShelf` included; `revise` and `fork` borrow their base and return a value at a version it did not hold; a fork records its base by the exact pair and states its own scope, label and citations rather than copying them. **User adjustments are a second document**: `RoleProfile` has no adjustment field and its wire denies unknown fields, and an `AdjustmentLayer` names the exact version it was written over, so a layer over version three is not applied to version four. **Favouriting is not a career decision**: `RoleInterest` holds a lineage — not a version, so it does not even select which bundle is in force — and a standing from a three-word vocabulary with **no** arm meaning *chosen* and **no** arm meaning *failed*, both refusals read back out of sections 25.11 and 37; no public signature outside its own four functions names one. Section 24.2's **twelve** direction names are parsed out of its own sentence and compared with `RoleDirection::NAMED` in both directions, and the `등` that ends that sentence is a separate arm rather than a thirteenth name. **This build ships no bundle for any of them**: `GATE-38-029` is open, so `BundleShelf::directions_covered` reports every named direction including the ones nothing covers, by name, and the whole set of public functions returning a bundle by value is exactly `declare`, `revise` and `fork`. It opens no file, opens no socket, **reads no clock** — a `validAt` is `P2-U6`'s valid-time `Date` — persists nothing and adds no migration, and inventories all 57 fields of every type it declares in both directions. It leaves `GATE-38-029` open and closes none. See [the role bundle contract](docs/contracts/role-bundles.md).
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
cargo test -p academic-freshness --test compile_fail --locked --offline
cargo test -p academic-gap --test compile_fail --locked --offline
cargo test -p academic-critical-path --test compile_fail --locked --offline
cargo test -p academic-repository-competency --test compile_fail --locked --offline
cargo test -p academic-competency --test compile_fail --locked --offline
cargo test -p academic-role-profile --test compile_fail --locked --offline
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

`P2-U3` adds `academic-audit`: section 11.1's fail-closed `RuleSet` selector,
section 11.3's explainable proof tree, and section 11.4's three-gate
`DETERMINATE` rule. It is the engine that tells a user whether they can
graduate, so every contract in it is fail-closed.

`DETERMINATE` is three values rather than three checks. `DeterminateVerdict` has
private fields and one constructor, and that constructor takes a
`CoverageWitness`, a `ConflictFreeWitness` and a `FreshnessWitness` **by
value**; each has private fields and a crate-private `establish` that returns
`Option<Self>` from the evidence its gate is about. A caller holding two of them
has no expression that produces a determination. No source states a
source-freshness number, so `SourceFreshnessPolicy` has no `Default` and no
constant, and an audit with none recorded is `INDETERMINATE` naming that. An
`INDETERMINATE` verdict takes its first missing check as a **parameter**, so an
empty outstanding list is not a value that can be written, and every arm names
the exact field, rule, attempt or source that is outstanding.

Section 11.3's four leaf parts — the rule identifier, the source page and
paragraph, the attempts used, and the equivalency decision — are four
constructor parameters on a type with private fields, no `Default` and no
setter, and the two that could be empty are enums whose other arm states the
reason. A published rule the source index does not place therefore cannot become
a leaf: it is left unevaluated, which is a partial failure, rather than
evaluated into a verdict with no citation. A plan reaches none of this —
`DegreeAudit::evaluate` has no plan parameter, and the annotated view produces
labels and never a verdict. Eight compile-fail cases are those absences.

**Section 11.3's five rendered leaf tokens are not the harness's five.** The
tree in the specification writes `PASS_PARTIAL`, which the harness has no value
for, prints no `CONFLICT`, which the harness has, and labels two structurally
identical credit rows differently — `93 / 130 PASS_PARTIAL` beside
`51 / 63 NEEDS 12`. The mapping is written down and compared against the
document in both directions rather than assumed.

`GRADUATION_AUDIT` flips to `IMPLEMENTED` with this task and its harness
directory carries all three adverse fixture sets, each reached under the
baseline rule set from the inputs. What a golden fixture is compared against did
not come from the engine it checks: `tools/graduation-audit-oracle.mjs` is a
second transcription of the transcript, the grade table, the repeat ceiling and
the rules, in another language with fixed-point `BigInt` units.
`GATE-38-001`–`GATE-38-004`, `GATE-38-006`, `GATE-38-011` and `GATE-38-012` stay
open, and each identifier is derived from its line's position in section 38
rather than typed. The crate runs inside `cargo test --workspace`, adds no
external package to `Cargo.lock`, and adds no migration. What it does and does
not claim is in
[the graduation audit](docs/contracts/graduation-audit.md).

`P2-U5` adds `academic-offering`: section 8.3's four offering statuses and the
calibrated forecast that decides which one an unconfirmed offering carries.
**The four statuses are four types, and the prohibition on promoting a
prediction is an absence rather than a check** — `ConfirmedStanding` holds a
registration-system reading inside a recorded verification bound, which a
forecast does not hold and cannot produce, so there is no expression anywhere
that turns a prediction into a confirmation. The same absence carries the plan
rule: `ConfirmedStanding::seat` is the only producer of a `ConfirmedSeat` and a
determinate plan takes seats by value, so a `HISTORICALLY_LIKELY` offering has
nothing to enter as. Seven compile-fail cases are those absences.

**`HISTORICALLY_LIKELY` is a conjunction and both halves are implemented.** The
row requires *여러 과거 학기의 재현 가능한 패턴, 미래 공식 공지 없음*: a
reproducible pattern **and** no future official notice. An official notice that
the course will run, from a source that is not the registration system, reaches
no confirmation -- that row requires a listing that was recently verified -- and
lands the offering on `UNCERTAIN` naming the notice, with the probability it
overrode kept on the record. §8.3's *별도 official Claim을 활성화한다* is
`announcement_claim`, and the notice bounds it: a claim backdated past its own
source is refused.

**Section 8.3's sentence names six features and the plan names seven families.**
The seventh is the sample window the same sentence requires be recorded, and the
divergence is executed rather than described: the six units are split out of the
document and compared in order and in both directions, and the seventh's phrase
is required to be after the split. Every family is measured rather than
declared — each gets a control and a variant differing in that family alone, and
the raw score has to move while every other family's contribution stays equal.
The window a spring forecast reads is the **spring** terms of the history, which
is what refuses the majority vote §8.3 forbids: two histories with the same
seasonal rate, window depth and instructor set, differing only in where in the
window the offerings sit, land on different statuses.

A never-observed course abstains twice over — an explicit reason, and a
`PredictionMetadata` that refuses a zero sample count, so there is no scored
forecast for it to become. A probability with no fresh calibration dataset is
refused rather than shown, through `P2-M1`'s registry. An official reading
arriving activates a second claim and leaves the prediction byte-identical;
`SUPERSEDED_FOR_DECISION` is a property of the claim set, not a value written
onto an append-only row. Per-term Brier score, coverage and abstention rate are
exact integers with no `f32`, `f64` or floating-point literal anywhere in the
crate, and what the expected values are compared against did not come from the
engine that produced them: `tools/offering-forecast-oracle.mjs` is a second
transcription of the corpus, the rule set, the calibration curve and the
arithmetic, in another language. **Nothing flips in the §28 registry**, because
§28's table names twelve engines and none of them is an offering forecast.
`GATE-38-017` stays open every term, and its identifier is derived from the
bullet's position in section 38 rather than typed. The crate runs inside
`cargo test --workspace`, adds no external package to `Cargo.lock`, and adds no
migration. What it does and does not claim is in
[offering status and the calibrated forecast](docs/contracts/offering-forecast.md).

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

`P2-P1` adds `academic-export`, export schema v2 and the clean-room reader that
opens it. What it does **not** link is the claim: `INV-C-015` says a user keeps
their own record when this product and their school account are both gone, and a
bundle writer that needed the database engine or a reader that needed the key
hierarchy would be making that claim about software the user no longer has. Its
product closure is `academic-audit`, `academic-domain`, `academic-requirement`
and three encoding crates, compared with the manifest in both directions; the
writer's input is a value the caller already holds and the reader's input is a
directory of bytes.

The restore re-runs the graduation audit rather than re-reading a verdict. It
parses the frozen inputs out of the bundle, hashes the rule set's canonical text
and requires it to equal the recorded `rule_set_hash` — section 37's *과거
audit은 당시 rule hash로 재현된다* — rebuilds the catalogue scope, re-runs
section 11.1's selector over the profile decoded out of those inputs, evaluates
the engine and byte-compares the outcome. `SelectedRuleSet` has one producer, so
the decision about which published rules apply is genuinely taken again. The
`RuleSet` itself is supplied by the caller: a bundle that could mint one would be
a way around `P2-U2`'s two-attestation gate.

`clean_offline_restore_reruns_deterministic_audit` deletes the profile and the
Phase 1 export the bundle was built from before it reads the bundle, and then
requires the re-run to refuse when one integer of the frozen inputs is moved.
What that pair of observations closes, what the six parts are, and what section
32.10 names that this build does not write are in
[the graduation export contract](docs/contracts/graduation-export.md).


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
