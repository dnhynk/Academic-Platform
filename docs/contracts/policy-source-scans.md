# Policy source scans

A *policy source scan* is a test that reads this repository's own source text
and refuses a shape. It exists for the class of change that alters nothing
observable at runtime — a second key source, a widened allowlist, a suppressed
banner behind a marker file — where there is no behaviour to assert against and
the source is the only evidence.

This page enumerates every one of them, because the same defect has been found
in a scan one step outside the one just repaired in each of the last four
rounds, and each time the next person started the survey from nothing.

That sentence was false twice — `P2-G2` found the page missing its own egress
rows, and `T141` found it missing three more scans, one of them the weaker half
of a contract whose other half had already been repaired. So it is executed
rather than asserted. `tools/policy-source-scan-inventory.test.mjs` finds every
file in `crates/`, `tools/` and `packages/` that names a Rust source path in a
position where it is read — an `include_str!`, a literal argument to a read, a
`join`, an extension filter, a `const` or table entry holding a path, or a
`#[path]` include of a module that does one of those — and fails if this page
does not name that file. What it cannot decide is whether a file that reads
source is a *policy* scan, so it does not try: a file it finds that scans
nothing is listed below anyway, in the rows saying what it does instead. That
is the intended outcome for a false positive — a row, not a hole.

## The three shapes that make a scan empty

**The walk stops short.** A `read_dir` that does not descend reads a flat tree
correctly and reads a subdirectory module not at all. It keeps passing, so
nothing says the day a subdirectory appears.

**The check is a token list.** A list of forbidden spellings refuses the edits
somebody thought of in advance and admits every edit spelled differently. The
`P2-K6` audit put five key substitutions past a token list; `P2-RF8` put six
lane widenings past another one. A whole-text pin — the item's own text,
whitespace-collapsed, compared to a constant — refuses every edit to that item
instead, including edits nobody predicted.

**Nothing bounds the coverage.** A walk that silently returns an empty list
passes every assertion in the loop below it.

A pin fixes the decision sites that exist. It does not forbid a *new* one, so a
pin needs a companion: an allowance table that counts the authority tokens the
tree spells today and fails on an addition anywhere.

## Two more the `T141` audit found, in the repair for the first three

The three above are about what a scan reads. These two are about what it
concludes from what it read, and both were found in the guard that had just
been repaired against the first three.

**The pin fixes the item and not its caller.** A whole-text pin refuses every
edit to the text it names, and says nothing about whether that text runs. The
`T141` audit left `WHOLE_KEY_CHECK` byte-identical and wrapped the *call* to it
in `if !profile_root.join("admission/field-trust").is_file() { … }`; signature
verification was then skipped whenever a marker file existed, and a probe
observed a receipt signed by a key the build had never heard of receiving an
admitted posture. A pin on a decision needs a second pin on the sequence that
reaches it — the first statement of each entry point, as `rotation_gate.rs`
does, or the whole calling function, as `WHOLE_VERIFY` now does.

**The allowance token is a spelling and not a structure.** An allowance entry
counts characters, so it counts the *context* those characters appear in.
`kind: PostureKind::Admitted {` is a field initialiser; the same value bound to
a local first — `let admitted = PostureKind::Admitted { .. }; Posture { kind:
admitted }` — spells it zero times and passed. Counting the type path
`PostureKind::Admitted` instead survives the rewrite, because a value of that
type cannot be built without naming it.

Both were then looked for in every other pin and every other allowance table.
`WHOLE_KEY_CHECK` was the only pin whose call was unconstrained. Every other one
already fixes its callers: `WHOLE_GATE` pins the first statement of all seven
gated entry points; `deny_on_findings`'s two call sites sit inside `WHOLE_STAGE`
and `posture_for_profile`'s inside `WHOLE_DISPATCH_SPINE`, so those calls are
already pinned text; `write_authorized_bytes`, `.execute(`,
`staged.preview().bytes()` and the rounding site are each held to an exact
occurrence count; `cloud_egress_default` and `ACCEPTANCE_PUBLIC_KEY` are not
functions with a decision to skip. Of the allowance tables, only
`ADMISSION_AUTHORITY_TOKENS` held a field-initialiser spelling —
`LANE_AUTHORITY_TOKENS`, `SOCKET_ALLOWANCE` and the two word-level lists are all
type paths or identifiers, which a rewrite of the surrounding expression cannot
remove.

## Every scan in this repository

"Walk" is how the scan reaches files. "Check" is what it does with the text.
"Floor" is what fails if the walk returns less than it should.

| Scan | Walk | Check | Floor |
|---|---|---|---|
| `no_environment_or_flag_override_exists` — `crates/cli/src/main.rs` | recursive, every crate's `src`, and every `*.rs` under `crates/admission` except its `tests`, `benches` and `examples` | 12 forbidden key/override seams anywhere in the admission crate; a 10-token allowance table counting the tokens the admission tree spells and forbidding nine of the ten to every other crate; whole-text pins on the `ACCEPTANCE_PUBLIC_KEY` declaration, on `verify_with_compiled_acceptance_key`, and on the whole of `AdmissionVerifier::verify`; refuses a file that declares an item at file scope below its test module, at any indentation; recursive Clap command-tree scan | no count; fails if the walk never reached `crates/admission`, plus a tripwire: every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the admission tree must be a file the walk read |
| `cli_has_no_real_data_override` — `crates/cli/tests/cli.rs` | recursive, `cli/src` + `core/src` | 10 forbidden tokens; a 5-token lane-authority allowance table; whole-text pins on `posture_for_profile`, `ALLOWLISTED_FIXTURE_IDS`, `is_allowlisted`, the daemon-side allowlist `match`, and the whole `fn main` dispatch spine; file-scope-below-test-module rule; a 12-pair environment battery, a 14-argument flag battery and a 3-path ingest battery against the built binary | `scanned >= 18` (19 today) |
| `read_crate_sources` — `crates/recovery/tests/recovery_admission.rs` | recursive, `recovery/src`, through `crates/test-support/src/word_level_entry_points.rs` | three token lists across three named tests: 8 device-key-source tokens plus a per-line `DeviceKeystore` allowlist, the shared 16 word-level spellings, 3 profile-default tokens | `>= 5`, plus a tripwire: a `mod name;` or `pub mod name;` on its own line must be a file the walk read |
| `no_public_api_accepts_or_reports_a_single_recovery_word` — `crates/crypto/tests/key_hierarchy.rs` | recursive, `crypto/src`, through the same shared module | `KY06`'s other structural half: the shared 16 word-level spellings, plus the assertion that `RecoverySecret` takes a whole 256-bit value | `>= 7`, plus the same module tripwire |
| `read_crate_sources`, `WORD_LEVEL_ENTRY_POINTS` — `crates/test-support/src/word_level_entry_points.rs` | the recursive walk the two rows above share, `#[path]`-included by both | not a scan itself: the walk, the floor, the module tripwire, and the one spelling list `KY06`'s two halves both read | the floor its caller passes |
| `keystore_leaf_public_facade_exposes_no_raw_handle` — `crates/keystore-platform/tests/facade.rs` | none — one fixed path, `../src/lib.rs` | no public signature in the FFI leaf names a raw handle, pointer, descriptor, key serial or D-Bus object; the three entry points are present; the two platform modules are declared private | `surface.len() >= 8` public signatures |
| `lifecycle_transition_table_rejects_every_non_edge`, `growth_descriptors_contain_no_scalar_score` — `crates/domain/tests/question_graph.rs` | none — one fixed path, `../src/question.rs`, read twice between named markers | the `QuestionStatus` variants read out of the enum must equal the expected six; no identifier in the descriptor schema is a scalar difficulty, score, rating, rank or percentile — with the mutation applied to the schema inside the test and required to be caught | none — a missing marker fails |
| `planned_engine_that_gains_an_implementation_fails_ci` — `crates/domain/tests/engine_harness.rs` | recursive, every `*.rs` under `crates/`, this crate's own test tree excluded | a planned §28 engine must have no implementation site: no workspace source outside `generated.rs` may name its engine id | none on the walk; the registry is compared whole, and an injected site is required to be reported |
| `read_crate_sources` — `crates/retention/tests/retention.rs` | recursive, `retention/src` | token lists at three call sites (revocation claims, `GATE-38-026` decision seams, journal truncation) | `>= 8` |
| `the_rotation_gate_is_one_decision_with_no_flag_variable_or_debug_path` — `crates/retention/tests/rotation_gate.rs` | none — three fixed paths | `WHOLE_GATE` whole-text pin on `require_rotation_accepted`; a 6-token list over its body; the first statement of each of 7 gated entry points | none |
| `default_feature_tree_has_no_conversion_entry_point` — `crates/store/tests/encrypted_profile.rs` | recursive, `store/src` | 3 forbidden conversion entry points; plus fixed-path reads of `src/lib.rs` for the compile-time guard, and byte scans of the built probe binary | none |
| `phase1_exit_has_no_product_network` — `crates/daemon/tests/phase1_exit.rs` | recursive, every crate's `src` except `test-support` | 10 networking tokens, paired with an independent link scan of the built default-feature `academicd` image for 8 symbol byte sequences | `scanned > 0` |
| `no_float_reaches_the_gpa_path` — `crates/record/tests/record_scans.rs` | recursive, `crates/record/src` | not a token list: a float *type* under any spelling, a decimal-point literal, and an exponent literal, over code with comments and string literals removed; five evasion samples are run through the check inside the test and each must be caught | `>= 11` files, plus a tripwire that every `pub mod name;` in `lib.rs` is a file the walk read |
| `the_published_average_is_rounded_in_one_pinned_place` — same file | the recursive walk above, for the rounding-site count; one fixed path for the pin | `WHOLE_DIVISION` whole-text pin on `div_round_half_up`; exactly one rounding site in the crate; the published scale still an argument; no type declared in the arithmetic module | the walk's floor above |
| `tools/secret-debug-policy.test.mjs` | recursive, every `crates/*/src` | regex over derive attributes against a registry of secret-carrying types | none on the file walk; a `>= 11` floor on the macro-generated key-type registry |
| `tools/phase1-scaffold-policy.test.mjs` | recursive, from eight roots: every workspace package's `src` except `academic-test-support`, a named crate set, `store-platform/src`, each of the six process crates' `src`, `transcript/src` twice, `record/src` (the two implemented §28 engines, with a `>= 12` floor), and — for `only_egress_crate_has_a_socket` — every `.rs` anywhere under every workspace package; fixed paths elsewhere | `cargo metadata` dependency graph, acceptance-receipt comparison, and regex/substring assertions on named files — including a second, independent copy of the rotation-gate decision-site count | none |
| `only_egress_crate_has_a_socket` — `tools/phase1-scaffold-policy.test.mjs` | recursive, **every `.rs` under every workspace package**, comments and every literal — raw strings included — stripped before matching | a per-file allowance of exact socket spellings (eight IPC files, two `P2-G4` files, every other allowance empty); a rule that a crate root or a socket module segment may be renamed only to `_`; zero foreign-function declarations anywhere; every `#[path]` target resolved and required to be one of the files this scan read; the one `include!` pinned whole; a pinned build-script inventory; a per-crate link closure intersected with the socket-capable crates; and, for the sandbox's Linux backend, a **counted** requirement that every `SYS_` spelling in the file sits inside its `denied_syscalls` function | `scanned.length >= 10` on the capability scan it sits beside; the allowance map is compared whole, so a file that stops being read fails as a missing key |
| `the_byte_path_has_one_derivation`, `no_exception_path_fails_open`, `a_denial_has_no_payload_field` — `crates/egress-boundary/tests/byte_path_pin.rs` | none — six fixed paths under this crate's own `src` | seven whole-text pins (below); occurrence counts for the single construction site, the single emit helper and the two `execute` call sites; a per-file fallback inventory with a written reason for each site; six shapes that may not appear at all (`catch_unwind`, `let _ =`, `if let Ok(`, `.is_ok()`, `unwrap()`, `.expect(`); the `EgressDenial` field list read out of the struct | none on the walk — the six paths are named; a file gaining a `#[cfg(test)]` module fails, because the product half would then be smaller than the file |
| `deny_reason_codes_are_exhaustive` — `crates/egress-boundary/tests/egress_boundary.rs` | none — one fixed path, `crates/policy/src/schema.sql` | a compiler-checked witness `match` over `ReasonCode` (a new variant stops the suite compiling), an index set over the enumerated list, a transcription of the execution plan's section 3.5 sentence, and the quoted codes in the `egress_audit` `CHECK` | n/a — the enum is read through the type system, not a walk |
| `the_tombstone_row_calls_the_product_restore_and_lives_only_here` — `crates/portability/tests/encrypted_rotation.rs` | none — two fixed *test* source paths | substring: the acceptance row is in this file, calls the product restore, and has no second definition in `academic-retention` | none |
| `unsafe_is_confined_to_the_sandbox_backends`, `probe_targets_are_not_in_any_default_build`, `the_probe_enters_the_sandbox_before_it_reads_a_job` — `crates/worker/tests/capability.rs` | recursive, this crate's `src`, `probes` and `tests` | the set of files holding an `unsafe` item compared whole against a two-entry list; the manifest's `[[bin]]` inventory read for `required-features` and a `path` under `probes/`; a whole-text pin on the probe's `run` function plus a call-site count of one on `sandbox::enter` and an ordering check against the job read | `scanned >= 8` |
| `tools/verify-contracts.mjs` | recursive, `crates/contracts/src`; the two generated modules through `tools/{engine,predicate}-registry.mjs` | digest pins and byte-for-byte re-render; refuses any tree entry that is not a `.rs` file | n/a — an unreviewed entry fails |
| `tools/engine-registry.mjs`, `tools/predicate-registry.mjs` | none — one fixed generated path each, named as `GENERATED_PATH` | not a scan: they render the generated module from `schemas/registry/`, and are the halves `verify-contracts.mjs` re-renders and compares against the committed file | n/a |
| `tools/policy-source-scan-inventory.test.mjs` | recursive, `crates/`, `tools/`, `packages/` | this page names every file that reads Rust source text: six read-position markers plus one hop through a `#[path]` include, each marker checked against a sample inside the test | `>= 20` files found |
| `tools/{source-preflight,cargo-lock-source-policy,dependency-source-policy,restricted-yaml}.mjs`, `tools/{dependency-source-policy,pnpm-source-policy-consumption}.test.mjs` | n/a | lockfile and registry parsing; not a source-text scan | n/a |
| `tools/{phase1-exit,security-baseline}.mjs` | n/a | execution observation and committed fixture bytes | n/a |
| `crates/store/tests/api_boundary.rs`, `crates/store/tests/sqlcipher_spike.rs` | n/a | manifest text and scratch-crate compile-fail; not a source-text scan — the `.rs` paths they name are the scratch crate's own `src/main.rs`, which they write | n/a |

## What is pinned as whole text, and what changing it costs

Each row below is compared against a constant rather than searched for tokens.
Editing one of them is intended to require editing its constant in the same
commit; that is the cost the pin buys, and it is why a pin is spent only where
a silent edit is the whole risk.

| Pinned item | Constant | Edited by |
|---|---|---|
| `ACCEPTANCE_PUBLIC_KEY` declaration | `WHOLE_ACCEPTANCE_KEY` | acceptance-key provisioning (`P2-H1`) |
| `verify_with_compiled_acceptance_key` body | `WHOLE_KEY_CHECK` | a change to how the compiled key is checked |
| `AdmissionVerifier::verify` | `WHOLE_VERIFY` | a change to which steps a receipt goes through, their order, or whether any of them is conditional |
| `require_rotation_accepted` | `WHOLE_GATE` | a change to what decides rotation acceptance — not turning the feature on, which the gate already reads |
| `posture_for_profile` | `WHOLE_POSTURE_SOURCE` | a change to where the CLI's posture comes from |
| `ALLOWLISTED_FIXTURE_IDS`, `is_allowlisted`, the daemon-side allowlist `match`, `fn main` | `WHOLE_FIXTURE_ALLOWLIST`, `WHOLE_ALLOWLIST_PREDICATE`, `WHOLE_DAEMON_FIXTURE_GATE`, `WHOLE_DISPATCH_SPINE` | ADR-002 acceptance, or a new command |
| `staged_runtime_call`, `write_authorized_bytes`, `Preview::bytes`, `StagedPayload::preview` | `WHOLE_RUNTIME_CALL`, `WHOLE_EMIT`, `WHOLE_PREVIEW_BYTES`, `WHOLE_STAGED_PREVIEW` | a change to where the transmitted bytes come from — these four are the whole path from the preview's buffer to the transport |
| `stage`, `deny_on_findings` | `WHOLE_STAGE`, `WHOLE_DENY_ON_FINDINGS` | a change to the staging pipeline's step order, the reason code a step denies with, or any default it takes; the fallback inventory counts sites and cannot see a default that changed direction |
| `cloud_egress_default` | `WHOLE_CLOUD_DEFAULT` | the user closing `GATE-38-028`; it takes no argument, so no quality heuristic can reach it |
| `div_round_half_up` | `WHOLE_DIVISION` | a change to how a published average is rounded — not a change to the scale, which is an argument the versioned grading scheme supplies |

Comment-only lines are dropped before a pin is compared, so a pin fixes code and
not prose. Whitespace is collapsed, so `cargo fmt` decides layout and the pin
decides content.

## What the two `P2-RF8` repairs hold

`crates/recovery/src` is flat today, so the flat `read_dir` it replaced missed
nothing that exists. What it missed is anything that would exist after one
subdirectory module is added, and all three of that crate's scanning tests read
through the same helper, so all three were blind at once.

`cli_has_no_real_data_override` scanned each file only as far as its first
`#[cfg(test)]` of any kind. `policy_banner.rs` carries a `#[cfg(test)]` helper
above its test module, so that file's scanned half ended at byte 210 of 1253 and
`posture_for_profile` — the one place in the CLI where a profile becomes a
posture — was never read by the test that claims to scan it. The split is now on
the test module, and anything at file scope below the test module is refused
rather than hidden.

Ten injections were applied one at a time, each reverted with its file's
SHA-256 checked back to its recorded value, on Windows native and WSL2 Linux
with the same result on both. Nine of them passed both guards before the repair
and fail after it; the tenth is the `union ` hole below, which passed with a
16-entry list and fails with the 17-entry one.

| # | Injection | Refused by |
|---|---|---|
| R-I1 | `src/platform/mod.rs` reaches `unlock_with_device` | recursive walk |
| R-I2 | `src/phrase/codec.rs` adds `from_words` | recursive walk |
| R-I3 | `src/lane/mod.rs` gives `RecoveryProfile` a `Default` | recursive walk |
| C-I1 | a second identifier joins `ALLOWLISTED_FIXTURE_IDS` | `WHOLE_FIXTURE_ALLOWLIST` |
| C-I2 | `is_allowlisted` becomes a prefix match | `WHOLE_ALLOWLIST_PREDICATE` |
| C-I3 | `posture_for_profile` takes its profile root from a marker file | `WHOLE_POSTURE_SOURCE` |
| C-I4 | `main` skips the mandatory banner when a marker file exists | `WHOLE_DISPATCH_SPINE` |
| C-I5 | the daemon-side allowlist arm becomes a prefix match | `WHOLE_DAEMON_FIXTURE_GATE` |
| C-I6 | a second posture source hides in the dead zone a `#[cfg(test)]` helper opens | test-module split + allowance table |
| C-I7 | a product item is declared below the test module as a `union` | `FILE_SCOPE_ITEM_STARTS` with `union ` |

Every `C-*` injection spells **none** of the ten forbidden tokens the guard
already held; the harness refuses to apply one that does. An injection that
names a token already on the list demonstrates only that the list works.

### And one found in this repair's own shape

`FILE_SCOPE_ITEM_STARTS` — the list of line starts that mean "an item is
declared here", used by the two guards that refuse product code below a test
module — is itself a token list, and it has to be complete or the rule it
enforces has a hole. It omitted `union `, which is a stable item keyword. No
`union` exists in any product source, and a `union` alone widens nothing, but
the hole is the same shape as the two defects this page is about, so it is
closed rather than recorded. `C-I7` is the observation: a `union` declared below
`output.rs`'s test module passes both guards with the 16-entry list and fails
both with the 17-entry one. The remaining item keywords not on the list —
`macro`, `default`, `auto trait` — are all unstable and cannot appear in this
repository's product source.

## What the `P2-RF9` repair holds

The `T141` audit put three shapes past `no_environment_or_flag_override_exists`
that spelled no forbidden token, edited neither whole-text pin, and were observed
by nothing in the README's verification block on either platform: a `#[path]`
module beside `src`, a conditional call to the pinned key check, and an admitted
posture assembled through a local binding. It put two more past `KY06`'s crypto
half. Each is now refused by the scan that claims the property.

Twenty-three injections were applied one at a time, each reverted with its file's
SHA-256 checked back to its recorded value, on Windows native and WSL2 Linux
with the same result on both. Six are `P2-RF7`'s, re-run as a regression: the
widened walk must not have cost the guard anything it already had.

| # | Injection | Refused by |
|---|---|---|
| A1–A6 | `P2-RF7`'s six, re-run | unchanged |
| B1 | `#[path = "../authority.rs"]` pulls in a product module beside `src` | the widened walk, and the `#[path]` tripwire |
| B1b | the same module placed outside `crates/admission` | the `#[path]` tripwire |
| B1c | the same module placed in the crate's own skipped `tests/` | the `#[path]` tripwire |
| B1d | the same module, observed by `only_egress_crate_has_a_socket` | **nothing** — `P2-G4`'s widened walk reads `crates/admission/authority.rs`, so that scan's membership rule is satisfied truthfully |
| B1e | the same module, naming an outbound socket in it | the widened walk, which reads the module's text |
| B2 | the call to the pinned key check is wrapped in a marker-file condition | `WHOLE_VERIFY` |
| B2b | the five steps all still called, unconditionally, reordered | `WHOLE_VERIFY` |
| B3 | `Unprovisioned` becomes an associated const holding a provisioned key | **not this scan** — `admitted_posture_requires_verified_receipt` and `unprovisioned_empty_zero_byte_and_all_zero_keys_fail_closed`, which observe provisioning unconditionally |
| B4 | an admitted posture built through `let admitted = PostureKind::Admitted { .. }` | the `PostureKind::Admitted` count |
| B4b | an admitted posture that never names `PostureKind::Admitted` at all | the `VerifiedAdmission` and `Posture {` counts |
| B5 | the same product item as A5, indented two spaces below the test module | the file-scope rule — previously `cargo fmt --all -- --check` alone |
| B5b | the same shape in `cli_has_no_real_data_override`'s tree | the same rule in that guard |
| C1 | a `word_index` in `crates/crypto/src/keystore.rs`, which the `KY06` half never read | the recursive walk |
| C1b | a `mnemonic_at` and a `WORD_COUNT` in a crypto file the half did read | the shared spelling list |
| C1c | a `phrase_word` in `crates/recovery/src`, a spelling only the crypto list held | the shared spelling list, from the other side |
| D1 | a new source-text scan this page does not name | `policy-source-scan-inventory` |
| D1b | an existing scan's path altered on this page | `policy-source-scan-inventory` |

`B3` is recorded as refused by something other than the scan because that is
what it is. A pin fixes an item's text and not what the names in it mean, and
`B3` leaves the pinned declaration byte-identical while redefining
`Unprovisioned` as a provisioned key. What refuses it is that the provisioning
is unconditional, so two behavioural tests see it. An edit of that shape that
*was* conditional would be `B2`, which `WHOLE_VERIFY` now refuses.

`B1d` is recorded as refused by nothing because it now is. `P2-G4` widened
`only_egress_crate_has_a_socket`'s walk to every `.rs` under a workspace
package, and `crates/admission/authority.rs` sits at a crate root, so the
membership rule that `B1d` was written against is satisfied — not evaded. `B1e`
is the observation that separates the two readings: the same module, this time
naming `TcpStream::connect`, is refused, so its text is genuinely read. The rule
still has force for a target under no package at all —
`#[path = "../../../target/hidden.rs"]` is still refused, which is half 4 of the
socket guard's six. `B1` itself is unaffected: what refuses it is
`no_environment_or_flag_override_exists`, whose own walk and `#[path]` tripwire
this widening did not touch.

The `mod name;` half of the admission tripwire has no injection, and cannot have
one while the walk reads the whole crate: any `mod name;` without its own
`#[path]` resolves to a file beside its declarer, which is a file the walk read.
It is a tripwire on the walk, in the same sense as `academic-recovery`'s and
`academic-record`'s — it fails the day the walk is narrowed, not today.

## Why the float scan is not a token list

`P2-U4` needed a scan saying no floating-point value reaches a grade-point
average. A list of forbidden spellings — `f32`, `f64` — is the obvious shape and
is the second empty-scan shape above: **three of the five ways a float arrives
in Rust name neither token.**

```rust
let ratio = 33.9 / 12.0;   // f64 by inference
let epsilon = 1e-9;        // f64, and not even a decimal point
let one = 1.;              // f64
```

The check is therefore over *literals* rather than over names: any
decimal-point or exponent literal in Rust code is a floating-point value,
whatever it is called. That needs comments and string literals removed first, or
the check would fire on this repository's own prose — the number `2.825` is
written in four documentation comments deliberately, because it is the tie the
corpus is built to land on.

All five evasions are applied to the check inside the test itself, and each must
be caught; six integer expressions the crate really uses are applied and must
not be. Four of them were also injected into the crate's own sources one at a
time, each reverted with its file's SHA-256 checked back to its recorded value,
on Windows native and WSL2 Linux with the same result on both:

| # | Injection | Names a forbidden token? |
|---|---|---|
| F-I1 | `views.rs` gains `let _ratio = 33.9 / 12.0;` | no |
| F-I2 | `decimal.rs` gains `let _epsilon = 1e-9;` | no |
| F-I3 | `grade.rs` gains `let _one = 1.;` | no |
| F-I4 | `engine.rs` gains `const SLIP: core::primitive::f64` | yes — the token half |
| P-I1 | `div_round_half_up` truncates instead of rounding | n/a — the pin |

`P-I1` is what the pin buys and a token list could not have: the truncation
spells nothing forbidden, leaves the function's shape intact, and changes the
published average from `2.83` to `2.82`. It fails both the pin and
`gpa_formula_fixture`.

## What the three `P2-G4` repairs hold

`P2-G4` had to widen `only_egress_crate_has_a_socket`: proving that an operating
system refuses a socket means asking it for one, so the sandbox probe spells
`TcpStream` and the Linux backend spells the socket syscall numbers. Three
holes were found while doing it, all three in the shapes this page already
names, and all three closed in the same commit as the widening.

**The walk stopped short.** It read `src`, `tests`, `benches` and `build.rs` by
name. `crates/worker/probes/` is a `[[bin]]` with an explicit `path`, so nothing
in it was scanned at all — not the file that was being allowed, and not any
other file that might appear there later. The walk is now every `.rs` anywhere
under a workspace package, which also reaches `build.rs` without naming it, so
the pinned build-script inventory is what bounds build scripts rather than the
walk. `I2` is the observation: a second `probes/` file with its own
`TcpStream::connect` and its own `[[bin]]` entry.

**The lexer desynchronized on a raw string.** `rustCodeOnly` modelled `//`,
`/* */`, `"…"` and `'c'`, but not `r#"…"#`. A raw string containing one quote
left the quote count odd, and from there every literal in the file was read as
code and every stretch of code as a literal — so a socket spelling *after* one
was invisible to a scan that already knew the spelling. Three files in this
repository contain raw strings and two predate this task. `I3` is the
observation, injected into `crates/retention/tests/tombstone.rs`.

**The pattern list missed the syscall.** `libc::syscall(libc::SYS_socket, …)`
opens a socket and spells none of the fifteen patterns the list held. `I1` is
the observation. `libc::syscall` and the `SYS_*` socket names are now on the
list, which is why the Linux backend needs an allowance — and why that allowance
carries a structural rule rather than a promise: every `SYS_` spelling in the
file must appear inside `denied_syscalls`, **counted**, because a spelling that
is in the deny list *and also* somewhere else passes a presence check while
naming a syscall the file does not refuse. `I6` is that observation.

A bare numeric `libc::syscall(41, …)` still passes every one of these, and is
recorded as `S-11` below. It is also the reason this task exists: a source scan
cannot see a syscall number, and an operating-system sandbox does not need to.

### The injection matrix

Fifteen injections, applied one at a time, each reverted with its file's SHA-256
checked back to its recorded value, on Windows native and WSL2 Linux. The full
table with per-platform verdicts is in the task report. Six of them —
`I1`, `I2`, `I3`, `I4`, `I5`, `I6` — spell **none** of the socket patterns the
guard held before this task, and `I7` through `I15` are not about spellings at
all: they widen a manifest, reorder a function, drop a record write, or turn one
kernel bound off.

## Open

These are not closed. "It cannot happen today" is not a reason to leave one
open, so each row says what makes it start mattering.

| # | Scan | What is open | When it starts mattering |
|---|---|---|---|
| S-1 | `crates/retention/tests/retention.rs` | token lists at all three call sites; no whole-text pin on any decision site | A revocation, `GATE-38-026`, or journal-truncation seam spelled differently from the listed tokens passes. The rotation *gate* is separately pinned in `rotation_gate.rs`, so the exposure is the three claims those lists carry, not the gate. |
| S-2 | `crates/store/tests/encrypted_profile.rs` | 3-token list, no floor | A profile-conversion entry point named anything other than `upgrade_profile`, `convert_profile`, or `migrate_schema_1_to_2` passes. With no floor, a walk that returns nothing also passes. This list is the source half of the execution plan's "no … profile-conversion command"; the behavioural half is `academic profile convert` exiting `USAGE` in the `cli_has_no_real_data_override` flag battery. Matters as soon as ADR-002 acceptance work adds a real migration path. |
| S-3 | `crates/daemon/tests/phase1_exit.rs` | 10-token list; `scanned > 0` is the whole floor | A socket reached through a re-export or a dependency's wrapper type names none of the ten. The paired link scan of the built binary is what actually carries this claim; the source half is the weaker of the two and should be read that way. |
| S-4 | `tools/secret-debug-policy.test.mjs` | no floor on the file walk; `T123 P3-G6` (`I37`, `I46`) still silent | `crates/*/src` returning an empty list passes every assertion. Matters if a crate is renamed or the walk root moves. |
| S-5 | `tools/phase1-scaffold-policy.test.mjs` | fixed paths outside `store-platform` | A file renamed or split leaves its assertions reading a path that no longer holds the code they describe. `readFile` throws on a missing path, so a rename fails loudly; a *split* does not — the assertions keep passing against the half that stayed. |
| S-7 | `crates/record/tests/record_scans.rs` | the comment/string stripper distinguishes a character literal from a lifetime by looking for a closing quote two characters on | A character literal wider than one `char` — `'\u{1F600}'` — is not stripped, so its digits would be read as code. No such literal exists in the crate and the scan errs toward reporting rather than hiding, so the failure mode is a false positive, not a miss. Matters if the crate ever needs a wide character literal. |
| S-6 | `crates/portability/tests/encrypted_rotation.rs` | two fixed test-source paths, substring only | It checks that one acceptance row lives in one file. A third file could hold a third copy and nothing would see it. |
| S-8 | `crates/crypto/tests/key_hierarchy.rs`, `crates/keystore-platform/tests/facade.rs`, `crates/domain/tests/question_graph.rs` | the last two still read one fixed path each, and `facade.rs` has a floor while `question_graph.rs` has none | A public item moved out of `keystore-platform/src/lib.rs` or `domain/src/question.rs` into a sibling module is not read. `P2-RF9` repaired the first of the three because `RecoverySecret` lives in that crate and its contract already had a repaired half to match; the other two were left as they are and are recorded here rather than fixed. |
| S-9 | `tools/policy-source-scan-inventory.test.mjs` | six read-position markers plus one `#[path]` hop — a mechanical proxy for "reads Rust source text" | A scan that reaches source some other way — a path assembled from fragments, a walk in a language this does not search — is not found, so the page could miss it and pass. The proxy is stated in the page's own opening sentence, so what the page claims is exactly what the test checks; widening the claim means widening the markers. |
| S-10 | `tools/secret-debug-policy.test.mjs` | `SECRET_FIELD_NAMES` holds `payload` and `payload_bytes` but not `bytes`, which is the other generic name a raw buffer hides behind. Adding one alternation catches four pre-existing sites: `WireField.bytes` (`crates/rpc/src/convert.rs`), `FingerprintEncoder.bytes` (`crates/store/src/schema_fingerprint.rs`), `SyntheticTranscriptPdf.bytes` (`crates/transcript/src/source.rs`), and `StreamingPrefix.bytes` (`crates/vault/src/object.rs`). | Now, for any of those four that holds something private. `P2-G4` found it by naming its own staged-output field `bytes` and watching the net miss it; it hand-wrote redacting `Debug` impls rather than renaming the field, so its own buffers leak nothing whatever the vocabulary reaches. Closing the row means one `PUBLIC_BYTES` sentence or one hand-written impl per site, from each crate's owner. |
| S-11 | `only_egress_crate_has_a_socket` — the spelling half | a syscall reached by a bare number. `libc::syscall(41, 2, 1, 0)` opens an AF_INET stream socket and spells nothing any pattern can match, because there is no name to match. | Any commit that adds a numeric syscall call. Nothing in this repository has one today; `unsafe_code = "forbid"` outside the five reviewed leaves is what keeps the reach small, and the link half bounds who can name `libc` at all. The answer to it is not a better scan — it is `P2-G4`'s sandbox, which refuses the syscall whatever spelled it. |
| S-12 | `os_keystore_capabilities_are_available_but_unused` — `tools/phase1-scaffold-policy.test.mjs` — and `tools/secret-debug-policy.test.mjs` | both walk `<crate>/src` and nothing else, so neither reads `crates/worker/probes/` | Now. `P2-G4` added the first tree here that holds product-shaped code outside `src` — a `[[bin]]` with an explicit `path` — and put a `process::Command` allowance in that same crate. Measured: a `process::Command` in `crates/worker/probes/worker_probe.rs` passes the first scan and a `#[derive(Debug)]` over a `key_bytes` field passes the second, while both edits placed under `src` are refused. This is the shape the `P2-G4` section above calls a walk that stopped short, one scan outside the one that was repaired. The socket and `unsafe` claims are unaffected: `only_egress_crate_has_a_socket` reads that directory, and so does `crates/worker/tests/capability.rs`. Closing it means widening both walks and writing one reviewed allowance line for each of the eight files outside `src` that already spell `process::Command`. |

## Intended, not a defect

`crates/rpc/src/convert.rs` restates the admitted arm of
`academic_admission::Posture`'s canonical JSON instead of asking `Posture` for
it. An admitted `Posture` is issued only by `AdmissionVerifier::verify`, the
acceptance key is `Unprovisioned`, and the `admitted_posture_requires_verified_receipt`
compile-fail case keeps external construction closed — so no test in this
repository can build one to compare against. The two spellings can therefore
drift, and if they do it is in one direction: a client holding the old spelling
refuses every admitted handshake its own daemon emits. Nothing observes this.
The literals are collected in one place and the comment there says so. It closes
when acceptance-key provisioning (`P2-H1`) makes an admitted `Posture`
constructible in a test.

## Where the requirement comes from

The authoritative spec (`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`)
does not mention source scans, the fixture allowlist, or `--allow-real-data`, so
nothing on this page can conflict with it. The normative source is the execution
plan's §3.1 — "There is no quiet flag, environment variable, debug build
shortcut, `--allow-real-data`, or profile-conversion command", and "a source scan
proving no second construction site exists" — and its §5 `P2-K6` acceptance row,
which names `no_environment_or_flag_override_exists` as "source scan + CLI
surface scan". The table above is what those two sentences actually rest on.

## Posture

Nothing on this page is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`, the acceptance public key is unprovisioned, and the
committed candidate receipt carries two of five platform rows.
