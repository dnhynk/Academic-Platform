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
| `phase1_exit_has_no_product_network` — `crates/daemon/tests/phase1_exit.rs` | recursive, **every `.rs` under every crate's package** except `test-support`, less each package's `tests` and `benches`, less the sandbox probe | 10 networking tokens, paired with an independent link scan of the built default-feature `academicd` image for 8 symbol byte sequences | `scanned >= 200` |
| `no_float_reaches_the_gpa_path` — `crates/record/tests/record_scans.rs` | recursive, the whole `crates/record` package less `tests` and `benches`, so `examples/` is read | not a token list: a float *type* under any spelling, a decimal-point literal, and an exponent literal, over code with comments and string literals removed; five evasion samples are run through the check inside the test and each must be caught | `>= 11` files, plus a tripwire that every `pub mod name;` in `lib.rs` is a file the walk read |
| `the_published_average_is_rounded_in_one_pinned_place` — same file | the recursive walk above, for the rounding-site count; one fixed path for the pin | `WHOLE_DIVISION` whole-text pin on `div_round_half_up`; exactly one rounding site in the crate; the published scale still an argument; no type declared in the arithmetic module | the walk's floor above |
| `tools/secret-debug-policy.test.mjs` | recursive, **every `.rs` under every `crates/*` package** less its `tests` and `benches`, so `examples/` and `probes/` are read | regex over derive attributes against a registry of secret-carrying types | none on the file walk; a `>= 11` floor on the macro-generated key-type registry |
| `tools/phase1-scaffold-policy.test.mjs` | recursive, from eight roots: every workspace package except `academic-test-support` (the whole package, not its `src`), a named crate set, `store-platform/src`, each of the six process crates' `src`, `transcript/src` twice, `record/src` (the two implemented §28 engines, with a `>= 12` floor), and — for `only_egress_crate_has_a_socket` — every `.rs` anywhere under every workspace package; fixed paths elsewhere | `cargo metadata` dependency graph, acceptance-receipt comparison, and regex/substring assertions on named files — including a second, independent copy of the rotation-gate decision-site count | none |
| `only_egress_crate_has_a_socket` — `tools/phase1-scaffold-policy.test.mjs` | recursive, **every `.rs` under every workspace package**, comments and every literal — raw strings included — stripped before matching | a per-file allowance of exact socket spellings (eight IPC files, two `P2-G4` files, every other allowance empty); a rule that a crate root or a socket module segment may be renamed only to `_`; zero foreign-function declarations anywhere; every `#[path]` target resolved and required to be one of the files this scan read; the one `include!` pinned whole; a pinned build-script inventory; a per-crate link closure intersected with the socket-capable crates; and, for the sandbox's Linux backend, two rules over syscalls: every `libc::syscall(` call in the file must name a `libc::SYS_` constant as its first argument and that constant must be one of the four reviewed syscalls the file installs the sandbox with, and every other `SYS_` spelling in the file must sit inside `denied_syscalls`, **counted** | `scanned.length >= 10` on the capability scan it sits beside; the allowance map is compared whole, so a file that stops being read fails as a missing key |
| `the_byte_path_has_one_derivation`, `no_exception_path_fails_open`, `a_denial_has_no_payload_field` — `crates/egress-boundary/tests/byte_path_pin.rs` | none — six fixed paths under this crate's own `src` | eight whole-text pins (below); occurrence counts for the single construction site, the single emit helper, the two `execute` call sites, and the two `bind_grant` call sites — the last counted by identifier rather than by spelling, so a call written `EgressProxy::bind_grant(self, ..)` counts; a per-file fallback inventory with a written reason for each site; six shapes that may not appear at all (`catch_unwind`, `let _ =`, `if let Ok(`, `.is_ok()`, `unwrap()`, `.expect(`); the `EgressDenial` field list read out of the struct | none on the walk — the six paths are named; a file gaining a `#[cfg(test)]` module fails, because the product half would then be smaller than the file |
| `deny_reason_codes_are_exhaustive` — `crates/egress-boundary/tests/egress_boundary.rs` | none — one fixed path, `crates/policy/src/schema.sql` | a compiler-checked witness `match` over `ReasonCode` (a new variant stops the suite compiling), an index set over the enumerated list, a transcription of the execution plan's section 3.5 sentence, and the quoted codes in the `egress_audit` `CHECK` | n/a — the enum is read through the type system, not a walk |
| `the_tombstone_row_calls_the_product_restore_and_lives_only_here` — `crates/portability/tests/encrypted_rotation.rs` | none — two fixed *test* source paths | substring: the acceptance row is in this file, calls the product restore, and has no second definition in `academic-retention` | none |
| `unsafe_is_confined_to_the_sandbox_backends`, `probe_targets_are_not_in_any_default_build`, `the_probe_enters_the_sandbox_before_it_reads_a_job` — `crates/worker/tests/capability.rs` | recursive, this crate's `src`, `probes` and `tests` | the set of files holding an `unsafe` item compared whole against a two-entry list; the manifest's `[[bin]]` inventory read for `required-features` and a `path` under `probes/`; a whole-text pin on the probe's `run` function plus a call-site count of one on `sandbox::enter` and an ordering check against the job read | `scanned >= 8` |
| `the_walk_reads_every_module_in_this_crate`, `untrusted_has_no_unwrapping_trait_impl`, `every_exposure_site_is_named_and_justified`, `the_instruction_channel_takes_only_static_text`, `the_adjudicator_receives_no_capability`, `only_reviewed_files_hold_an_unlabelled_provider_response` — `crates/untrusted-content/tests/trust_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests` and `benches`) and all source; plus a second recursive walk over every `.rs` under every package in `crates/` for the `AcceptedResponse` inventory | the whole set of `impl` blocks whose header names `Untrusted<` compared against a two-entry list; the whole inventory of the crate-private accessor's call sites with a written reason for each, counted by identifier rather than by the spelling `.expose()`; a rule that no `pub` signature in the crate's product source takes an `Untrusted<…>` and returns a type naming `str`, `String` or `u8`; nine whole-text pins (below); occurrence counts on the two directive constructions, the one `quote` caller, the one `adjudicate` caller — that one also by identifier, with `use` items dropped so a re-export is not read as a call — and `leak`; the manifest read for `academic-policy` as a dev edge and `academic-worker` as no edge, and four broker type names forbidden in product source; the whole set of files naming `AcceptedResponse` | `>= 8` files, a rule that no product source sits outside `src`, and a tripwire: every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the crate must be a file the walk read |
| `no_public_signature_hands_out_ingested_text` — `crates/untrusted-content/tests/untrusted_boundary.rs` | recursive, **every `.rs` under every `crates/*` package** less each package's `tests` and `benches`, comment lines dropped | no `pub fn` signature anywhere in the workspace takes an `Untrusted<…>` parameter and returns a type naming `str`, `String` or `u8` as a whole identifier — so a lifetime cannot hide one, and `&[u8]`, `Vec<u8>`, `Box<[u8]>` and `Cow<'_, [u8]>` are all reached | `>= 25` packages and `>= 1_200` public signatures |
| `the_walk_reads_every_module_in_this_crate`, `the_capture_decision_is_one_binding_that_every_path_runs`, `a_status_comes_from_one_derivation_and_absence_is_unknown`, `an_attestation_has_no_route_into_an_authority`, `no_legal_conclusion_reaches_a_permission`, `retention_holds_two_independent_bounds_and_narrows_only`, `the_checklist_is_the_seven_dimensions_the_contract_names`, `an_expiry_cannot_be_applied_without_its_preview`, `the_two_derivative_vocabularies_are_the_same_list`, `the_migration_vocabularies_are_the_rust_ones`, `every_instant_this_crate_compares_is_an_argument` — `crates/consent/tests/consent_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests` and `benches`) and all source; plus a second recursive walk over **every `.rs` under every package in `crates/`**, less each package's `tests` and `benches`, for the two workspace-wide signature rules; plus fixed reads of `migrations/store/0006_phase2_consent_and_capture.sql`, `crates/store/src/authorizer.rs`, `crates/retention/src/plan.rs` and this crate's own `Cargo.toml` | fourteen whole-text pins (below); whole-set comparisons of the `impl` blocks naming `AuthorityGrant` and those naming `AttestationRecord`, and of the files naming `CaptureCapabilityToken` with a written reason for each; call-site counts by identifier on `bind_permission` (2), `record_capture_denial` (3), `record_capture_mint` (1), `status_of` (3), `inherit` (1) and `apply_expiry` (0), each with `fn <name>(` subtracted so `inherited` is not read as `inherit`; struct-literal counts on `CaptureCapabilityToken` (1), `BoundPermission` (1) and `ExpiryPlan` (0); a rule that no `pub` signature anywhere in the workspace takes an `AttestationRecord` and returns a type naming `AuthorityGrant`, `WrittenAuthority`, `BoundPermission`, `CaptureCapabilityToken` or `CaptureStatus`, and the same rule for a signature taking a `LegalQuestion` or an `ExternalReviewTask`; the `CaptureStatus`, `ChecklistDimension`, `ChecklistEntry` and `DerivativeClass` variant lists read out of the enums and compared whole; each of the eight migration `CHECK` lists compared against the Rust `as_str` spellings **and** against its enum's variant count; two mutations applied to the pinned retention text inside the test and each required to be caught; `#[path]` refused outright; a five-spelling clock list — the whole of `std::time`'s surface plus `chrono`, which this crate's one product edge cannot reach past | `>= 12` files in the crate walk and `>= 10` product files, plus a tripwire requiring every `mod name;` and `pub mod name;` to be a file the walk read and every product file to sit under `src`; `>= 25` packages and `>= 1_200` public signatures in the workspace walk; `>= 1` signature taking a legal question, so the legal rule cannot pass by finding nothing |
| `crates/consent/tests/compile_fail.rs` and `tests/compile_fail/*.rs` | n/a — `trybuild` compiles two committed programs | not a source-text scan: passing an `AttestationRecord` where a `WrittenAuthority` belongs, and constructing a `CaptureCapabilityToken` with a struct literal. Each must fail to compile *and* fail with the committed diagnostic | n/a |
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
| `bind_grant` | `WHOLE_BIND_GRANT` | a change to what a transmission has to agree with before a byte is built — the plan naming the grant the token spends, and the grant recording the rulepack that produced the payload; the two call sites are counted beside it, because a pin on a check says nothing about whether every path runs it |
| `stage`, `deny_on_findings` | `WHOLE_STAGE`, `WHOLE_DENY_ON_FINDINGS` | a change to the staging pipeline's step order, the reason code a step denies with, or any default it takes; the fallback inventory counts sites and cannot see a default that changed direction |
| `cloud_egress_default` | `WHOLE_CLOUD_DEFAULT` | the user closing `GATE-38-028`; it takes no argument, so no quality heuristic can reach it |
| `impl<T> Untrusted<T>` | `WHOLE_UNTRUSTED` | a change to what the trust wrapper hands back — the whole-set `impl` rule refuses a new trait, and this refuses a new inherent method |
| `impl SystemDirective`, `impl ToolDirective` | `WHOLE_SYSTEM_DIRECTIVE`, `WHOLE_TOOL_DIRECTIVE` | a change to what the instruction channels accept |
| `escape`, `impl PromptEnvelope` | `WHOLE_ESCAPE`, `WHOLE_ENVELOPE` | a change to how ingested bytes are quoted, which channel they land in, or what the untrusted span map records |
| `resolve_span`, `adjudicate` | `WHOLE_RESOLVE_SPAN`, `WHOLE_ADJUDICATE` | a change to provenance resolution, to schema validation, or to what the adjudicator is handed — its parameter list is the claim that it holds no capability |
| `envelope_for`, `admit` | `WHOLE_ENVELOPE_FOR`, `WHOLE_ADMIT` | the callers of the two pinned decisions, pinned for the `T141` reason: a pin on a decision says nothing about whether it runs |
| `div_round_half_up` | `WHOLE_DIVISION` | a change to how a published average is rounded — not a change to the scale, which is an argument the versioned grading scheme supplies |
| `status_of`, `impl CaptureStatus` | `WHOLE_STATUS_OF`, `WHOLE_CAPTURE_STATUS` | a change to what decides a section 3.7 status, the order of the five tests, or which statuses permit at all |
| `bind_permission`, `impl<'a> ResolvedRequest<'a>` | `WHOLE_BIND_PERMISSION`, `WHOLE_RESOLVE_REQUEST` | a change to what a capture has to agree with before a device opens, or to which absent request field denies — the two callers are pinned beside it and counted at two, for the `T141` and `P2-RF10` reasons |
| `mint_capture_capability`, `continue_capture` | `WHOLE_MINT`, `WHOLE_CONTINUE` | a change to which path reaches the binding, or to whether a refusal leaves its audit row |
| `impl RetentionTerms`, `impl RetentionBound`, `pub struct RetentionTerms` | `WHOLE_RETENTION_TERMS`, `WHOLE_RETENTION_BOUND`, `WHOLE_RETENTION_TERMS_STRUCT` | a change to the two independent axes, to the direction a derivative inherits, or to the order that makes `PROHIBITED` the strictest bound |
| `impl AuthorityGrant`, `impl AttestationRecord` | `WHOLE_AUTHORITY_GRANT`, `WHOLE_ATTESTATION` | a change to what a written authority has to supply, or to what a user's own account of events hands back |
| `preview_expiry`, `apply_expiry`, `impl ExpiryPlan` | `WHOLE_PREVIEW`, `WHOLE_APPLY`, `WHOLE_EXPIRY_PLAN` | a change to what a deletion preview enumerates, to the instant an expiry is compared against, or to whether a plan can exist without a preview |

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
socket guard's seven. `B1` itself is unaffected: what refuses it is
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
carries a structural rule rather than a promise: a `SYS_` spelling in the file
must appear inside `denied_syscalls`, **counted**, because a spelling that is in
the deny list *and also* somewhere else passes a presence check while naming a
syscall the file does not refuse. `I6` is that observation. `P2-G4` applied that
rule to the ten socket names on the allowance and skipped `libc::syscall`
itself; both gaps are closed by `P2-RF10` below.

A bare numeric `libc::syscall(41, …)` passed every one of these when `P2-G4`
wrote them, and was recorded as `S-11`. `P2-RF10` closed that row; what the rule
is now, and why the sandbox is not the answer to it, is in the `P2-RF10` section
below.

### The injection matrix

Fifteen injections, applied one at a time, each reverted with its file's SHA-256
checked back to its recorded value, on Windows native and WSL2 Linux. The full
table with per-platform verdicts is in the task report. Six of them —
`I1`, `I2`, `I3`, `I4`, `I5`, `I6` — spell **none** of the socket patterns the
guard held before this task, and `I7` through `I15` are not about spellings at
all: they widen a manifest, reorder a function, drop a record write, or turn one
kernel bound off.

## What the `P2-G5` scans hold

`P2-G5`'s claim is a shape of the source: the trust label is a type because the
type implements nothing that would strip it, and the instruction channel is
trusted because its constructor takes a compile-time constant. Neither has a
run-time observation that would notice the day it stops being true.

Two of the checks could have been token lists and are not. The trait rule
compares the crate's **whole set** of `impl` blocks whose header names
`Untrusted<` against a two-entry list, so an implementation of a trait nobody
predicted fails as an extra key; the orphan rule refuses the same implementation
written in another crate, which is the half nothing here needs to check. The
exposure rule compares the **whole inventory** of the accessor's call sites,
each with a written reason, rather than searching for a spelling. That last
clause was not true when `P2-G5` wrote it — the inventory counted the spelling
`.expose()` until `P2-RF10`; see below.

### The walk that stopped short, found in this task's own scan

The first version of `trust_scans.rs` walked `crates/untrusted-content/src`.
`G-I13` — a `#[path = "../extra/leak.rs"]` module with an exposure site in it —
was refused by the module tripwire and **passed the exposure inventory**, which
is `S-12`'s shape reproduced one round later inside the scan written to avoid it.
The walk now reads every `.rs` anywhere under the package, split into product
source and all source, and a rule requires this crate's product source to sit
under `src` and nowhere else. The workspace-wide `AcceptedResponse` walk was
widened the same way in the same commit: it named `src`, `tests` and `benches`,
which is three directory names rather than a package.

`S-12` was left open here and is closed by `P2-RF10` below.
`academic-untrusted-content` adds no file outside `src`, so this task did not
widen that row; what widened it was `crates/record/examples/`, which had been
there a commit longer than the tree `S-12` named.

### The injection matrix

Fifteen injections, applied one at a time, each reverted with its file's SHA-256
checked back to its recorded value, on Windows native and WSL2 Linux. Every one
was refused on both, and the refusing test is the same on both except where the
table says otherwise. `G-I1` is the only one that spells a name any list in this
file holds; every other one is refused by a whole-set comparison, a whole-text
pin, a type, or a behavioural assertion.

| # | Injection | Refused by |
|---|---|---|
| G-I1 | `impl Deref for Untrusted<IngestedDocument>` in a new module | the whole `impl` set, and the exposure inventory |
| G-I2 | a **local** trait `Reveal` handing the text back, spelling none of the nine listed trait names | the whole `impl` set, and the exposure inventory |
| G-I3a | `SystemDirective::new` becomes non-`const` and leaks a `String` | the compiler: `E0015`, a non-`const` call in the four `const` initialisers |
| G-I3b | a second constructor `from_owned` beside it, which compiles | `WHOLE_SYSTEM_DIRECTIVE` |
| G-I4 | the call to `adjudicate` wrapped in a marker-file condition — `T141`'s shape | `WHOLE_ADMIT` |
| G-I5 | the escaper stops escaping the line terminator | `WHOLE_ESCAPE`, and `taint_flow_test_keeps_untrusted_spans_in_data_channel` |
| G-I6 | `quote` stores the raw text instead of the escaped text | `WHOLE_ENVELOPE`, and the same taint test |
| G-I7 | `expose` becomes `pub` | `WHOLE_UNTRUSTED`, and the crate-private assertion |
| G-I8 | a fourth exposure site in an existing file | the exposure inventory |
| G-I9 | `academic-policy` moves to `[dependencies]` | the manifest half of `the_adjudicator_receives_no_capability`, and `workspace_dependency_direction_is_acyclic` |
| G-I10 | a new `src` file handing out `AcceptedResponse::bytes` unlabelled | `only_reviewed_files_hold_an_unlabelled_provider_response` |
| G-I11 | eight corpus records removed | the corpus floor, which fails all five acceptance rows |
| G-I12 | `academic-indexer` gains an edge to this crate with no receipt row | `workspace_dependency_direction_is_acyclic`, `six_process_entrypoints_are_exact_and_distinct`, `indexer_cannot_open_a_socket` on Windows; on Linux `cargo metadata --locked --offline` refuses first, because the new edge changes `Cargo.lock` and nothing had regenerated it there |
| G-I13 | product code outside `src`, reached by `#[path]` | the `#[path]` tripwire, the product-source-under-`src` rule, and — only after the walk was widened — the exposure inventory |
| G-I14 | the schema stops refusing an unknown key | `model_output_failing_schema_is_quarantined`, whose eleven cases are each required to be produced |

`G-I3a` is recorded as refused by the compiler rather than by the pin because
that is what it is: `BOUNDARY_SYSTEM_DIRECTIVES` is a `const` array, so a
non-`const` constructor cannot be reached at all, and the pin never runs.
`G-I3b` is the edit of the same shape that does compile, and the pin is what
refuses it.

## What the `P2-RF10` repair holds

`P2-A2`'s independent audit measured four things this page and two contract
pages claimed and the code did not do. Two were reachable without spelling a
single token any list here holds, which is the shape this page exists to record.

**An inventory that counts a spelling is not an inventory.**
`every_exposure_site_is_named_and_justified` counted the substring `.expose()`.
`Untrusted::expose(document)` is the same call written through the type path, it
contains no such substring, and a fourth exposure site written that way passed
`trust_scans`, `cargo test --workspace --all-targets`, every JS scan, and
`cargo clippy --workspace --all-targets -- -D warnings`. Written
`document.expose()` it failed at once — so what separated pass from fail was the
spelling, not the reach. With that one function present, an integration test
*outside* the crate put an ingested payload verbatim into a `[SYSTEM]` segment,
unescaped, on its own line, in no untrusted span. The inventory now counts the
accessor's **name**, with a non-identifier byte required on each side, which is
the shape `names_unsafe` in `crates/worker/tests/capability.rs` and
`crates/record/tests/record_scans.rs` already used. The same change was made to
the `adjudicate` caller count, which had counted the argument spelling
`adjudicate(index, output)` and could not see a second caller that renamed its
two locals.

Counting the name is necessary and not sufficient: it says how many sites there
are, not what they hand back. So a second rule was added beside it — no `pub`
signature may take an `Untrusted<…>` and return a type naming `str`, `String` or
`u8` — and a workspace-wide copy of it,
`no_public_signature_hands_out_ingested_text`, because `Untrusted<T>` is a
public type any crate can name and the harm measured was one crate out.

**A second path that skipped the check nothing counted.**
`EgressProxy::transmit_without_completion` is `pub`, not feature-gated, and read
no grant row and compared no rulepack. Given the same staged payload, the same
token, and a grant reviewed under another rulepack, `transmit` refused with zero
bytes and it wrote 180 to the transport. `transmit`'s own comparison was not
observed by anything either: deleting it outright left the whole workspace suite,
`pnpm test` and `pnpm security` green. Both comparisons now live in one
`bind_grant`, called as the first statement of both paths, pinned as whole text
by `WHOLE_BIND_GRANT`, and its call sites counted at two. `bind_grant` also
refuses a plan that names a grant other than the token's, which `T146` measured
transmitting 180 bytes while journalling a grant nobody spent.

**A walk rooted at `src` does not read the package.** `S-12` recorded this for
`crates/worker/probes/`. It named that as the first tree of its kind; it was not.
`crates/record/examples/emit_harness.rs` arrived one commit earlier in `P2-U4`,
has no feature gate, is compiled by `cargo clippy --workspace --all-targets`, and
is run by the documented `pnpm harness:emit` script. Four scans could not see it,
including `no_float_reaches_the_gpa_path`, which keeps `academic-record`'s own
README sentence. All four now walk the package.

**`libc::syscall` was on an allowance and its argument was not read.** `S-11`
said a bare-number syscall was open, that nothing in the repository had one, and
that `P2-G4`'s sandbox rather than a better scan was the answer. All three had
weakened. `libc::syscall` is on this file's socket allowance and the counted rule
skipped it, so adding `libc::syscall(41, 2, 1, 0)` changed no allowance and
passed every scan and `clippy -D warnings`. And the sandbox is not the answer,
because `crates/worker/src/sandbox/linux.rs` holds the parent-side `launch` as
well as the child-side `enter`, and the parent runs outside the sandbox it
installs. Every `libc::syscall(` in that file must now name a `libc::SYS_`
constant, and that constant must be one of the four the file installs the sandbox
with; every other `SYS_` name in the file must sit inside `denied_syscalls`,
counted, which closes the non-socket `SYS_` gap the previous rule left.

### The injection matrix

Nineteen injections, applied one at a time, each reverted with its file's
SHA-256 checked back to its recorded value, on Windows native and WSL2 Linux.
Every one is refused on both. Six of them are `T146`'s own probes replayed
verbatim, marked below; each of those passed every check before this repair.

| # | Injection | Refused by |
|---|---|---|
| E-I2 | `bind_grant` loses the plan/token equality half | `a_plan_naming_another_grant_is_refused`, and `WHOLE_BIND_GRANT` |
| E-I3 | `bind_grant` loses the rulepack comparison (`T146`'s `E-I1`) | `a_grant_reviewed_under_another_rulepack_is_refused_on_every_transmit_path`, and `WHOLE_BIND_GRANT` |
| E-I3-full | the same, against `cargo test --workspace --all-targets` | the same two — this is the command that passed before the repair |
| E-I4 | `transmit_without_completion` stops calling `bind_grant` | the call-site count, and both named tests |
| E-I5 | `transmit` stops calling `bind_grant` | the same |
| E-I6 | the journal names a grant the transfer did not spend | `eg04_grant_expiring_mid_transfer_aborts_and_audits_the_partial_count`, whose `grant_id` half was discarded by a `..` pattern until this repair |
| G-I1 | a fourth exposure site written `Untrusted::expose(d)` (`T146`'s `G-I1`) | the exposure inventory, and both public-signature rules |
| G-I1c | the same function, receiver spelling — the control | the same |
| G-I1-owned | the same reach returning `String` | the same |
| G-I1-static | the same reach returning `&'static str`, which does not contain the substring `&str` | the same — the return type is read as identifiers |
| G-I1-bytes | the same reach returning `Vec<u8>` | the same |
| G-I2 | a second `adjudicate` caller with renamed arguments (`T146`'s `G-I2`) | the caller count, now by identifier |
| S-I1 | an `f64` and a decimal literal in `crates/record/examples/` (`T146`'s `S-I1`) | `no_float_reaches_the_gpa_path`, now walking the package |
| S-I1c | the same under `crates/record/src/` — the control | the same |
| S-I2 | `#[derive(Debug)]` over `key_bytes` in `crates/record/examples/` (`T146`'s `S-I2`) | `tools/secret-debug-policy.test.mjs`, now walking the package |
| S-I3 | `process::Command` in `crates/record/examples/` (`T146`'s `S-I3`) | `os_keystore_capabilities_are_available_but_unused`, now walking the package |
| S-I4 | `#[derive(Debug)]` over `key_bytes` in `crates/worker/probes/` (`T146`'s `S-I4`) | the same widened `secret-debug` walk |
| S-I5 | `std::net::TcpStream::connect` in `crates/record/examples/` (`T146`'s `S-I5`) | `phase1_exit_has_no_product_network`, now walking the package |
| S-I5b | the same file, against the socket scan | `only_egress_crate_has_a_socket`, which already read that tree |
| K-I1 | `libc::syscall(41, 2, 1, 0)` in the sandbox backend (`T146`'s `K-I1`) | the first-argument rule |
| K-I2 | `libc::SYS_socket` spelled outside `denied_syscalls` | the counted rule, as before |
| K-I3 | `libc::SYS_memfd_create` outside `denied_syscalls` (`T146`'s `K-I3`) | the whole-`SYS_` counted rule, which is new |

The six existing `libc::syscall(libc::SYS_…)` calls in that file are the control
for `K-I1`: they are all first-argument-named, all four names are on the reviewed
list, and the unmodified tree passes.

`E-I3-full` is worth its own row because it is the exact command `T146` ran. The
repair is not that the comparison exists — it existed before — but that removing
it now fails a named test rather than nothing.

## What the `P2-G6` scans hold

`P2-G6`'s claims are of three kinds, and only one of them has a run-time
observation that would notice the day it stopped being true.

**A type separation.** A user attestation and a written authority are unrelated
types. The compiler refuses a caller who passes one where the other belongs, and
`tests/compile_fail/attestation_is_not_a_written_authority.rs` is that program
with its diagnostic committed. What the compiler does not refuse is the *commit*
that adds the conversion, so the whole set of `impl` blocks naming each type is
compared against a one-entry list, and a workspace-wide signature rule refuses a
free function anywhere in `crates/` that takes an attestation and returns a
permission-shaped value. That last rule is `P2-RF10`'s
`no_public_signature_hands_out_ingested_text` applied to the other direction of
the same mistake, and it is workspace-wide for the same reason: the type is
public and the harm measured in that round was one crate out.

**A decision that every path has to run.** `bind_permission` is pinned as whole
text, its two callers are pinned beside it, and its call sites are counted —
`P2-RF10`'s shape, because that round found a second public path that skipped a
check nothing counted. The refusing paths are counted separately, at three,
because a refusal that returns without its audit row is the same defect in the
audit rather than in the decision.

**A restatement.** `academic-consent` declares its own copy of
`academic-retention`'s derivative-class list, because
`rotation_engine_lane_is_not_default` holds that exactly one crate declares a
product edge to that crate and a consent ledger has no business inside that
boundary. The copy is compared against the original — spellings, order, and both
enums' variant names — through a dev edge that reaches no product binary.

### The migration is compared against the Rust it mirrors

Migration `0006` restates eight closed vocabularies as SQL `CHECK` lists.
`the_migration_vocabularies_are_the_rust_ones` compares each list against the
`as_str` spellings of the enum it mirrors **and** against that enum's variant
count, so a spelling that drifts fails, and so does a variant added with no
`as_str` arm to compare. It also reads every `CREATE TABLE` out of the file and
requires each one to be in the authorizer's canonical set and to carry the
append-only trigger pair — which is the same claim
`authorizer_covers_every_canonical_table` makes about the two enforcement
layers, checked here against the file that creates them.

### The injection matrix

Twenty-eight injections, applied one at a time, each reverted with its file's
SHA-256 checked back to its recorded value, on Windows native and WSL2 Linux
with the same result on both. The harness is
`t148-injections.py` in the task report directory; it restores the original
bytes rather than rewriting the file, because this repository's working tree is
CRLF on Windows and a text-mode round trip would change every byte of a file the
harness never meant to touch.

`I26` and `I26b` are the same edit run under two commands. `cargo test` stops
at the first test binary that fails, so the whole-crate command never reaches
the scan binary and the row records what actually refused it; `I26b` runs the
scan binary alone and is where the pin and the clock list are observed. Any row
above naming a scan was refused by that scan under a command that reached it.

`I13` is recorded as proving nothing, because that is what it is: adding an
eighth `ChecklistDimension` variant stops the crate compiling, since `as_str` is
an exhaustive match. `I13b` is the edit of the same shape that *does* compile —
the variant with its `as_str` arm, absent from the registry array — and it is
the one the scan refuses.

| # | Injection | Refused by |
|---|---|---|
| I1 | a new module with an inherent method on `AttestationRecord` returning a `WrittenAuthority` | the whole `impl` set naming `AttestationRecord` |
| I2 | the same reach as a free `pub fn`, in no `impl` block | the workspace-wide signature rule |
| I3 | a `pub fn` taking an attestation and returning `CaptureStatus::Permitted` | the same rule, and the count of permitting statuses named outside `status.rs` |
| I4 | a `pub fn` turning a `LegalQuestion` into a permitting status | `no_legal_conclusion_reaches_a_permission`, and the same count |
| I5 | `RetentionTerms::transcript` returns the audio bound | `WHOLE_RETENTION_TERMS`, and `audio_and_transcript_retention_are_independent` |
| I6 | `inherit` widens the audio axis instead of narrowing it | `WHOLE_RETENTION_TERMS`, and `derivative_expiry_is_equal_or_stricter` |
| I7 | the `RetentionBound` variants are reordered, so `Prohibited` stops being strictest | `derivative_expiry_is_equal_or_stricter` — the enum declaration is not pinned, and the behavioural grid is what catches it |
| I8 | a third minting path that never binds | the `bind_permission` call-site count, and the token construction count |
| I9 | the continuation skips the binding when a marker file exists — `T141`'s shape | `WHOLE_CONTINUE`, and the call-site count |
| I10 | one refusing path stops appending its audit row | the `record_capture_denial` count of three |
| I11 | the expiry comparison is off by one, so a grant survives its own `not_after` | `WHOLE_STATUS_OF`, and `expired_permission_denies_and_queues_recheck` |
| I12 | an absent media set resolves to every medium instead of denying | `WHOLE_RESOLVE_REQUEST`, and `new_offering_permission_defaults_unknown` |
| I13 | an eighth checklist dimension with no `as_str` arm | **the compiler** — proves nothing about the scan |
| I13b | the same variant with its `as_str` arm, absent from the registry array | the variant-list comparison, and the migration vocabulary comparison |
| I14 | a third `ChecklistEntry` arm meaning nobody looked | the arm comparison |
| I15 | `apply_expiry` stops comparing the previewed instant | `WHOLE_APPLY`, and `expiry_requires_the_preview_it_was_shown_for` |
| I16 | a second `ExpiryPlan` constructor that takes no preview | `WHOLE_EXPIRY_PLAN` |
| I17 | a `#[path]` module holding product code outside `src` | the `#[path]` rule and the product-source-under-`src` rule |
| I18 | a subdirectory module with an extra `impl AuthorityGrant` | the recursive walk, and the whole `impl` set |
| I19 | the migration collapses the two retention axes into one | the four-column rule |
| I20 | a migration `CHECK` spelling drifts from the Rust one | the vocabulary comparison |
| I21 | a new canonical table in `0006` with no guard triggers | the table/trigger/authorizer rule |
| I22 | the derivative-class list is reordered on the consent side | `the_two_derivative_vocabularies_are_the_same_list` |
| I23 | `academic-retention` becomes a product edge of `academic-consent` | `workspace_dependency_direction_is_acyclic` and `rotation_engine_lane_is_not_default` |
| I24 | the authorizer loses one of the new canonical tables | `authorizer_covers_every_canonical_table` |
| I25 | an eighth dimension spelling added to the migration only | the vocabulary comparison |
| I26 | `status_of` reads the wall clock instead of its argument | the acceptance rows, which stop the whole crate suite before the scan binary runs |
| I26b | the same edit, run against the scan binary alone | `WHOLE_STATUS_OF`, and `every_instant_this_crate_compares_is_an_argument` |

`I2`, `I3`, `I4` and `I18` spell **none** of the names any list in this file
holds, and none of them is an `impl` block of a shape anybody predicted: what
refuses them is a whole-set comparison, a signature rule, or a count.

## Open

Each row says what makes it start mattering: "it cannot happen today" is not a
reason to leave one open. A row that has been closed keeps its identifier and
says so, because other pages and other audits cite these by number, and because
the reasoning that left it open is worth reading beside the reasoning that
closed it.

| # | Scan | What is open | When it starts mattering |
|---|---|---|---|
| S-1 | `crates/retention/tests/retention.rs` | token lists at all three call sites; no whole-text pin on any decision site | A revocation, `GATE-38-026`, or journal-truncation seam spelled differently from the listed tokens passes. The rotation *gate* is separately pinned in `rotation_gate.rs`, so the exposure is the three claims those lists carry, not the gate. |
| S-2 | `crates/store/tests/encrypted_profile.rs` | 3-token list, no floor | A profile-conversion entry point named anything other than `upgrade_profile`, `convert_profile`, or `migrate_schema_1_to_2` passes. With no floor, a walk that returns nothing also passes. This list is the source half of the execution plan's "no … profile-conversion command"; the behavioural half is `academic profile convert` exiting `USAGE` in the `cli_has_no_real_data_override` flag battery. Matters as soon as ADR-002 acceptance work adds a real migration path. |
| S-3 | `crates/daemon/tests/phase1_exit.rs` | 10-token list; `scanned > 0` is the whole floor | A socket reached through a re-export or a dependency's wrapper type names none of the ten. The paired link scan of the built binary is what actually carries this claim; the source half is the weaker of the two and should be read that way. |
| S-4 | `tools/secret-debug-policy.test.mjs` | no floor on the file walk; `T123 P3-G6` (`I37`, `I46`) still silent | The walk returning an empty list passes every assertion. The second half of this row — "matters if the walk root moves" — **came true and has been repaired**: `T146` observed the root standing at `crates/*/src` while product-shaped code sat in `examples/` and `probes/`, and `P2-RF10` widened it to the package. The floor half is still open and is now the whole row: a `readdir` that throws is caught and returns an empty list, so a scan that reads nothing still passes. Severity **P3** — it needs a filesystem fault or a moved `crates/` root to fire, and a `>= 200` file floor beside the macro-registry floor would close it. |
| S-5 | `tools/phase1-scaffold-policy.test.mjs` | fixed paths outside `store-platform` | A file renamed or split leaves its assertions reading a path that no longer holds the code they describe. `readFile` throws on a missing path, so a rename fails loudly; a *split* does not — the assertions keep passing against the half that stayed. |
| S-7 | `crates/record/tests/record_scans.rs` | the comment/string stripper distinguishes a character literal from a lifetime by looking for a closing quote two characters on | A character literal wider than one `char` — `'\u{1F600}'` — is not stripped, so its digits would be read as code. No such literal exists in the crate and the scan errs toward reporting rather than hiding, so the failure mode is a false positive, not a miss. Matters if the crate ever needs a wide character literal. |
| S-6 | `crates/portability/tests/encrypted_rotation.rs` | two fixed test-source paths, substring only | It checks that one acceptance row lives in one file. A third file could hold a third copy and nothing would see it. |
| S-8 | `crates/crypto/tests/key_hierarchy.rs`, `crates/keystore-platform/tests/facade.rs`, `crates/domain/tests/question_graph.rs` | the last two still read one fixed path each, and `facade.rs` has a floor while `question_graph.rs` has none | A public item moved out of `keystore-platform/src/lib.rs` or `domain/src/question.rs` into a sibling module is not read. `P2-RF9` repaired the first of the three because `RecoverySecret` lives in that crate and its contract already had a repaired half to match; the other two were left as they are and are recorded here rather than fixed. |
| S-9 | `tools/policy-source-scan-inventory.test.mjs` | six read-position markers plus one `#[path]` hop — a mechanical proxy for "reads Rust source text" | A scan that reaches source some other way — a path assembled from fragments, a walk in a language this does not search — is not found, so the page could miss it and pass. The proxy is stated in the page's own opening sentence, so what the page claims is exactly what the test checks; widening the claim means widening the markers. |
| S-10 | `tools/secret-debug-policy.test.mjs` | `SECRET_FIELD_NAMES` holds `payload` and `payload_bytes` and not the generic names a raw buffer actually hides behind. `T146` measured four more that pass today: `text`, `escaped`, `bytes`, and `staged_text`, against the control `payload`, which fails. Adding `bytes` alone reaches four pre-existing sites — `WireField.bytes` (`crates/rpc/src/convert.rs`), `FingerprintEncoder.bytes` (`crates/store/src/schema_fingerprint.rs`), `SyntheticTranscriptPdf.bytes` (`crates/transcript/src/source.rs`), `StreamingPrefix.bytes` (`crates/vault/src/object.rs`); `text` and `escaped` reach `QuotedDocument` and `RenderedPrompt` in `crates/untrusted-content`, and `staged_text` reaches `crates/egress-boundary/src/stage.rs`. | Now, for any site that holds something private. Nothing leaks today: all four `P2-G4`/`P2-G5`/`P2-G2` types — `QuotedDocument`, `RenderedPrompt`, `StagedOutput`, `AcceptedOutput` — have hand-written `Debug` impls, and the four `bytes` sites are public buffers. What is open is the **net**, not any site. Severity **P2**, raised from the earlier reading: the vocabulary now trails the code by six names rather than one, and each new crate has added to the gap. **`P2-RF10` measured the cost of closing it rather than estimating it.** Adding `bytes`, `text`, `escaped` and `staged_text` to `SECRET_FIELD_NAMES` fires 13 sites in 8 crates: `Alias.text`, `PartialAlias.text`, `RegistryFact.text` and the tuple variant `ClaimObject::Text(String)` in `academic-domain`; `SearchHit.text` and `ExactSymbolHit.text` in `academic-projections`; `AliasSpec.text` in `academic-store`; `JsonValue::Text` in `academic-test-support`; and `CorpusFile.bytes`, `WireField.bytes`, `FingerprintEncoder.bytes`, `SyntheticTranscriptPdf.bytes`, `StreamingPrefix.bytes`. The four `untrusted-content` and `egress-boundary` types the vocabulary was widened *for* fire nothing, because all four already hand-write `Debug`. So the work is not the vocabulary line: it is a redaction decision about the eight `text` sites, which hold entity surface forms, indexed content and claim values — user content, not public buffers — spread over four crates whose contracts this row's task did not read. A `PUBLIC_BYTES` entry silences a field permanently, and writing eight of them to close one row would trade this row for a worse one. Closing it means one commit per crate from its owner, redacting rather than declaring. `P2-G6` added a crate and did not widen this row: `academic-consent` declares no `text`, `bytes`, `escaped` or `staged_text` field at all, because every evidence item it holds is a locator plus a digest plus a byte count and its one place for prose is a closed `NotApplicableReason` enum. |
| S-11 | `only_egress_crate_has_a_socket` — the spelling half | **Closed by `P2-RF10`.** Every `libc::syscall(` call in the sandbox backend must now name a reviewed `libc::SYS_` constant as its first argument, so a bare number is refused. Why the three reasons this row gave for leaving it open were all wrong by the time it was read is in the `P2-RF10` section above. | n/a — closed. The step out from it that stays open is `S-13`. |
| S-12 | `os_keystore_capabilities_are_available_but_unused` — `tools/phase1-scaffold-policy.test.mjs` — `tools/secret-debug-policy.test.mjs`, `no_float_reaches_the_gpa_path`, and `phase1_exit_has_no_product_network` | **Closed by `P2-RF10`.** All four walked `<crate>/src`; all four now walk the package, less `tests` and `benches`, and the eight files outside `src` that spell `process::Command` each carry the reason they are allowed. The tree this row named was not the first of its kind — `crates/record/examples/` arrived a commit earlier and has no feature gate — which is why widening only the two walks this row listed would have closed half of it. | n/a — closed. |
| S-13 | `only_egress_crate_has_a_socket` — the syscall rule `P2-RF10` added | It reads `crates/worker/src/sandbox/linux.rs` and only that file, because that is the one file whose allowance lists `libc::syscall`. A `libc::syscall(` call in any other file is refused by the allowance itself — the spelling is not on that file's list — so the rule and the allowance together do cover the workspace. What they do not cover is a **future second allowance entry**: the day another file is allowed `libc::syscall`, the first-argument rule will not run on it unless the branch is widened with it. | The next commit that adds a second `libc::syscall` allowance entry. Closing it means keying the rule on the spelling rather than on the file. It is written this way today because the reviewed-name list is that file's, not a workspace list, and inventing a workspace one before there is a second caller would be a guess about what the second caller needs. Severity **P3**. |

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
