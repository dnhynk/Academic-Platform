# Policy source scans

A *policy source scan* is a test that reads this repository's own source text
and refuses a shape. It exists for the class of change that alters nothing
observable at runtime — a second key source, a widened allowlist, a suppressed
banner behind a marker file — where there is no behaviour to assert against and
the source is the only evidence.

This page enumerates every one of them, because the same defect has been found
in a scan one step outside the one just repaired in every round so far, and each
time the next person started the survey from nothing. `P2-RF11` found three
inside its own repair: closing the `use libc::syscall` shapes left
`extern crate libc as raw`, closing that left `use libc::{self as l}`, and
closing that left the function taken as a value. Assume there is one more.

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
| `phase1_exit_has_no_product_network` — `crates/daemon/tests/phase1_exit.rs` | recursive, **every `.rs` under every crate's package** except `test-support`, less each package's `tests`, less the sandbox probe | 10 networking tokens, paired with an independent link scan of the built default-feature `academicd` image for 8 symbol byte sequences | `scanned >= 200` |
| `no_float_reaches_the_gpa_path` — `crates/record/tests/record_scans.rs` | recursive, the whole `crates/record` package less `tests`, so `examples/` and any `benches/` are read | not a token list: a float *type* under any spelling, a decimal-point literal, and an exponent literal, over code with comments and string literals removed; five evasion samples are run through the check inside the test and each must be caught | `>= 11` files, plus a tripwire that every `pub mod name;` in `lib.rs` is a file the walk read |
| `the_published_average_is_rounded_in_one_pinned_place` — same file | the recursive walk above, for the rounding-site count; one fixed path for the pin | `WHOLE_DIVISION` whole-text pin on `div_round_half_up`; exactly one rounding site in the crate; the published scale still an argument; no type declared in the arithmetic module | the walk's floor above |
| `tools/secret-debug-policy.test.mjs` | recursive, **every `.rs` under every `crates/*` package** less its `tests`, so `examples/`, `probes/` and any `benches/` are read | regex over derive attributes against a registry of secret-carrying types | none on the file walk; a `>= 11` floor on the macro-generated key-type registry |
| `tools/phase1-scaffold-policy.test.mjs` | recursive, from nine roots: every workspace package except `academic-test-support` (the whole package, not its `src`), a named crate set, `store-platform/src`, each of the six process crates' `src`, `transcript/src` twice, `record/src` (the two implemented §28 engines, with a `>= 12` floor), the whole `crates/desktop` package (not its `src`, with a `>= 9` floor), and — for `only_egress_crate_has_a_socket` — every `.rs` anywhere under every workspace package; fixed paths elsewhere | `cargo metadata` dependency graph, acceptance-receipt comparison, and regex/substring assertions on named files — including a second, independent copy of the rotation-gate decision-site count | none |
| `only_egress_crate_has_a_socket` — `tools/phase1-scaffold-policy.test.mjs` | recursive, **every `.rs` under every workspace package**, comments and every literal — raw strings included — stripped before matching | a per-file allowance of exact socket spellings (eight IPC files, two `P2-G4` files, every other allowance empty); a rule that a crate root or a socket module segment may be renamed only to `_`, read on `use` and on `extern crate` alike; a rule that no file in the workspace may import `libc::syscall` — by name, under an alias, inside a braced list, or through `use libc::*` — so a call to it has to spell `libc::syscall(`; zero foreign-function declarations anywhere; every `#[path]` target resolved and required to be one of the files this scan read; the one `include!` pinned whole; a pinned build-script inventory; a per-crate link closure intersected with the socket-capable crates; and, for the sandbox's Linux backend, two rules over syscalls: every `libc::syscall(` call in the file must name a `libc::SYS_` constant as its first argument and that constant must be one of the four reviewed syscalls the file installs the sandbox with, and every other `SYS_` spelling in the file must sit inside `denied_syscalls`, **counted** | `scanned.length >= 10` on the capability scan it sits beside; the allowance map is compared whole, so a file that stops being read fails as a missing key |
| `the_byte_path_has_one_derivation`, `no_exception_path_fails_open`, `the_transport_is_reached_from_no_module_but_the_proxy`, `a_denial_has_no_payload_field` — `crates/egress-boundary/tests/byte_path_pin.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; the whole-text pins still read their own file by name, so a rename fails the read | nine whole-text pins (below), including the whole `OutboundTransport` trait, so a second way to write bytes cannot be added without editing the pin; call-site counts summed over the product walk — `execute`, `bind_grant` and `write_authorized_bytes` at two each and `send_chunk` at one — each counted by identifier, less declarations of a function named exactly that, so `EgressProxy::bind_grant(self, ..)` counts and `fn bind_grant_later(` is not subtracted; a per-file rule that only `lib.rs` may call the first three and only `transport.rs` may call `send_chunk`; the single construction site and the single redaction pass; a per-file fallback inventory with a written reason for each site; six shapes that may not appear at all (`catch_unwind`, `let _ =`, `if let Ok(`, `.is_ok()`, `unwrap()`, `.expect(`); the `EgressDenial` field list read out of the struct | `>= 6` files on both walks, a rule that no product source sits outside `src`, and a tripwire: every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the crate must be a file the walk read, with a floor of five declared modules; a file gaining a `#[cfg(test)]` module fails, because the product half would then be smaller than the file |
| `deny_reason_codes_are_exhaustive` — `crates/egress-boundary/tests/egress_boundary.rs` | none — one fixed path, `crates/policy/src/schema.sql` | a compiler-checked witness `match` over `ReasonCode` (a new variant stops the suite compiling), an index set over the enumerated list, a transcription of the execution plan's section 3.5 sentence, and the quoted codes in the `egress_audit` `CHECK` | n/a — the enum is read through the type system, not a walk |
| `the_tombstone_row_calls_the_product_restore_and_lives_only_here` — `crates/portability/tests/encrypted_rotation.rs` | none — two fixed *test* source paths | substring: the acceptance row is in this file, calls the product restore, and has no second definition in `academic-retention` | none |
| `unsafe_is_confined_to_the_sandbox_backends`, `probe_targets_are_not_in_any_default_build`, `the_probe_enters_the_sandbox_before_it_reads_a_job` — `crates/worker/tests/capability.rs` | recursive, this crate's `src`, `probes` and `tests` | the set of files holding an `unsafe` item compared whole against a two-entry list; the manifest's `[[bin]]` inventory read for `required-features` and a `path` under `probes/`; a whole-text pin on the probe's `run` function plus a call-site count of one on `sandbox::enter` and an ordering check against the job read | `scanned >= 8` |
| `the_walk_reads_every_module_in_this_crate`, `untrusted_has_no_unwrapping_trait_impl`, `every_exposure_site_is_named_and_justified`, `the_instruction_channel_takes_only_static_text`, `the_adjudicator_receives_no_capability`, `only_reviewed_files_hold_an_unlabelled_provider_response` — `crates/untrusted-content/tests/trust_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus a second recursive walk over every `.rs` under every package in `crates/` for the `AcceptedResponse` inventory | the whole set of `impl` blocks whose header names `Untrusted<` compared against a two-entry list; the whole inventory of the crate-private accessor's call sites with a written reason for each, counted by identifier rather than by the spelling `.expose()`, and less declarations of a function named exactly `expose` — not less the spelling `fn expose`, which subtracted `pub fn expose_rendered(` too; a rule that no `pub` signature in the crate's product source takes an `Untrusted<…>` and returns a type naming `str`, `String` or `u8`; nine whole-text pins (below); occurrence counts on the two directive constructions, the one `quote` caller, the one `adjudicate` caller — all three by identifier on both halves, with `use` items dropped from the `adjudicate` count so a re-export is not read as a call — and `leak`; the manifest read for `academic-policy` as a dev edge and `academic-worker` as no edge, and four broker type names forbidden in product source; the whole set of files naming `AcceptedResponse` | `>= 8` files, a rule that no product source sits outside `src`, and a tripwire: every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the crate must be a file the walk read |
| `no_public_signature_hands_out_ingested_text` — `crates/untrusted-content/tests/untrusted_boundary.rs` | recursive, **every `.rs` under every `crates/*` package** less each package's `tests`, comment lines dropped | no `pub fn` signature anywhere in the workspace takes an `Untrusted<…>` parameter and returns a type naming `str`, `String` or `u8` as a whole identifier — so a lifetime cannot hide one, and `&[u8]`, `Vec<u8>`, `Box<[u8]>` and `Cow<'_, [u8]>` are all reached | `>= 25` packages and `>= 1_200` public signatures |
| `the_walk_reads_every_module_in_this_crate`, `the_capture_decision_is_one_binding_that_every_path_runs`, `a_status_comes_from_one_derivation_and_absence_is_unknown`, `an_attestation_has_no_route_into_an_authority`, `no_legal_conclusion_reaches_a_permission`, `retention_holds_two_independent_bounds_and_narrows_only`, `the_checklist_is_the_seven_dimensions_the_contract_names`, `an_expiry_cannot_be_applied_without_its_preview`, `the_two_derivative_vocabularies_are_the_same_list`, `the_migration_vocabularies_are_the_rust_ones`, `every_instant_this_crate_compares_is_an_argument` — `crates/consent/tests/consent_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus a second recursive walk over **every `.rs` under every package in `crates/`**, less each package's `tests`, for the two workspace-wide signature rules; plus fixed reads of `migrations/store/0006_phase2_consent_and_capture.sql`, `crates/store/src/authorizer.rs`, `crates/retention/src/plan.rs` and this crate's own `Cargo.toml` | fourteen whole-text pins (below); whole-set comparisons of the `impl` blocks naming `AuthorityGrant` and those naming `AttestationRecord`, and of the files naming `CaptureCapabilityToken` with a written reason for each; call-site counts by identifier on `bind_permission` (2), `record_capture_denial` (3), `record_capture_mint` (1), `status_of` (3), `inherit` (1) and `apply_expiry` (0), each with `fn <name>(` subtracted so `inherited` is not read as `inherit`; struct-literal counts on `CaptureCapabilityToken` (1), `BoundPermission` (1) and `ExpiryPlan` (0); a rule that no `pub` signature anywhere in the workspace takes an `AttestationRecord` and returns a type naming `AuthorityGrant`, `WrittenAuthority`, `BoundPermission`, `CaptureCapabilityToken` or `CaptureStatus`, and the same rule for a signature taking a `LegalQuestion` or an `ExternalReviewTask`; the `CaptureStatus`, `ChecklistDimension`, `ChecklistEntry` and `DerivativeClass` variant lists read out of the enums and compared whole; each of the eight migration `CHECK` lists compared against the Rust `as_str` spellings **and** against its enum's variant count; two mutations applied to the pinned retention text inside the test and each required to be caught; `#[path]` refused outright; a five-spelling clock list — the whole of `std::time`'s surface plus `chrono`, which this crate's one product edge cannot reach past | `>= 12` files in the crate walk and `>= 10` product files, plus a tripwire requiring every `mod name;` and `pub mod name;` to be a file the walk read and every product file to sit under `src`; `>= 25` packages and `>= 1_200` public signatures in the workspace walk; `>= 1` signature taking a legal question, so the legal rule cannot pass by finding nothing |
| `the_walk_reads_every_module_in_this_crate`, `raw_score_has_no_ordering_implementation_anywhere`, `raw_score_hands_back_no_number`, `every_calibrated_value_comes_from_the_registry`, `the_calibration_and_reconciliation_decisions_are_pinned`, `the_record_constructor_takes_every_field`, `the_consumption_join_is_the_only_key_into_the_audit`, `the_migration_is_applied_and_guarded`, `the_checks_catch_the_evasions_they_are_written_against` — `crates/model-run/tests/model_run_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus a second recursive walk over every `.rs` under every package in `crates/` for the two workspace-wide rules; fixed paths for the four pins and for `crates/policy/src/schema.sql` | the whole set of `impl` headers naming `RawScore` compared against a two-entry list, and the same set required empty in every other crate; a rule that no `pub fn` signature in the workspace turns a `RawScore` into a bare number type, read as identifiers so `&'static str`-shaped evasions do not slip; the whole inventory of public signatures naming `CalibratedConfidence`, each with a written reason; four whole-text pins (below); a whole-text pin on `egress_consumption`, a rule that its one write site takes its sequence from the allow audit, and a rule that the reconciliation names no `row.grant_id`; `reconcile_egressed` declared and called exactly once; the `ModelRun` struct's fields compared with `ModelRun::record`'s parameters and with `record_digest`'s coverage; migration `0007`'s tables compared against their trigger pairs and `CANONICAL_TABLES`; and five ordering evasions plus a raw-string sample run through the checks inside the test and each required to be caught | `>= 7` files under the package and `>= 5` product files, a rule that this crate's product source sits under `src` and nowhere else, and a tripwire: every `mod name;` and `pub mod name;` must be a file the walk read, and `#[path]` may not appear at all |
| `the_walk_reads_every_module_in_this_crate`, `every_release_site_is_named_and_justified`, `no_public_signature_hands_out_a_proposed_payload`, `the_boundary_has_no_unwrapping_trait_impl`, `every_door_reaches_the_workflow_comparison`, `every_settlement_door_is_named`, `the_user_receipt_has_one_producer`, `the_crate_has_no_writer_dependency` — `crates/proposal/tests/proposal_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus fixed paths for `src/queue.rs`, `src/disposition.rs`, `src/proposed.rs` and the crate manifest | the whole set of `impl` headers naming `Proposed<` compared against a two-entry list; the whole inventory of the crate-private payload accessor's call sites with a written reason for each, counted by identifier rather than by the spelling `.release()` and less declarations of a function named exactly `release` — not less the spelling `fn release`, which would subtract `fn release_now(` too; a rule that no `pub fn` in the crate's product source takes a `Proposed<…>` and returns its payload; two whole-text pins (below), each with the callers that reach it pinned beside it; a call-site count of four on the workflow comparison and a pinned first statement for each of the four doors; the whole set of the queue's public `&mut self` methods compared against a seven-row table naming why an automatic actor cannot use each; and the manifest read for four forbidden writer edges with comment lines dropped first, plus the whole product dependency set pinned at three | `>= 8` files under the package, a rule that this crate's product source sits under `src` and nowhere else, and a tripwire: every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the crate must be a file the walk read; `>= 5` modules declared |
| `model_run_requires_every_field` — `crates/model-run/tests/model_run.rs` | none — three fixed paths: the authoritative spec, `crates/model-run/src/record.rs`, and `migrations/store/0007_phase2_model_run_provenance.sql` | not a transcription: the section 27.3 `ModelRun` YAML keys are parsed out of the spec, mapped to field names by camel-to-snake case, and compared with the `ModelRun` struct's own field list as whole sets in both directions; the storage map is compared with the same key set; every named storage site must exist in migration `0007`; and each key is dropped in turn with each comparison required to notice | the parsed key set must hold more than one key, so a parser that stopped reading fails rather than passing with an empty expectation |
| `crates/consent/tests/compile_fail.rs` and `tests/compile_fail/*.rs` | n/a — `trybuild` compiles two committed programs | not a source-text scan: passing an `AttestationRecord` where a `WrittenAuthority` belongs, and constructing a `CaptureCapabilityToken` with a struct literal. Each must fail to compile *and* fail with the committed diagnostic | n/a |
| `the_walk_reads_every_module_in_this_crate`, `the_capture_gate_records_every_refusal_it_returns`, `the_capture_gate_re_runs_the_binding_on_every_path`, `no_public_signature_hands_out_a_quarantined_capture`, `every_capture_medium_is_classified`, `unsafe_is_confined_to_the_device_backends`, `the_linux_backend_names_only_the_three_syscalls_it_installs`, `the_probe_opens_a_handle_and_reads_no_sample` — `crates/capture-gate/tests/capture_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source — `probes/` is product source here, because it is the one file that opens a device and it is exactly what a walk rooted at `src` would miss; plus a second recursive walk over **every `.rs` under every package in `crates/`**, less each package's `tests`, for the workspace-wide signature rule; plus fixed reads of `crates/consent/src/permission.rs` and this crate's own `Cargo.toml` | ten whole-text pins (below); a whole-set comparison of the `impl` blocks naming `QuarantinedArtifact` against a one-entry list, so a `Deref`, an `AsRef<[u8]>` or any other trait that hands the bytes back fails as an extra key; a whole-set comparison of every signature in the crate whose return type names `u8` against a two-entry list; a rule that no `pub` signature anywhere in the workspace takes a `QuarantinedArtifact` and returns a type naming `u8`, `str` or `String`; an equality between the number of `CaptureRefusal` constructions and the number of `record_refusal` calls, which is how "every refusing path appends its row" is checked rather than asserted; call-site counts by identifier on `mint_capture_capability` (1), `continue_capture` (2) and `bind_permission` (1), each with `fn <name>(` subtracted; the `CaptureMedium` variant list read out of `academic-consent`'s source and compared against the four `DeviceClass::of` classifies, plus a rule that its wildcard arm is `None` and not a device; the set of files holding an `unsafe` item compared whole against a two-entry list; for the Linux backend, the same two syscall rules `only_egress_crate_has_a_socket` applies to the worker's, read against this file's own three-name list; five read shapes forbidden in the probe; and the manifest read for `default = []`, `required-features` and a `path` outside `src` | `>= 8` files in the crate walk and `>= 8` product files, plus a tripwire requiring every `mod name;` and `pub mod name;` to be a file the walk read, `#[path]` refused outright, `>= 6` declared modules, and every product file under `src/` or `probes/`; `>= 25` packages and `>= 1_200` public signatures in the workspace walk |
| `crates/capture-gate/tests/compile_fail.rs` and `tests/compile_fail/*.rs` | n/a — `trybuild` compiles two committed programs | not a source-text scan: constructing a `CaptureSession` with a struct literal, and reading a `QuarantinedArtifact`'s bytes. Each must fail to compile *and* fail with the committed diagnostic | n/a |
| `tools/verify-contracts.mjs` | recursive, `crates/contracts/src`; the two generated modules through `tools/{engine,predicate}-registry.mjs` | digest pins and byte-for-byte re-render; refuses any tree entry that is not a `.rs` file | n/a — an unreviewed entry fails |
| `tools/engine-registry.mjs`, `tools/predicate-registry.mjs` | none — one fixed generated path each, named as `GENERATED_PATH` | not a scan: they render the generated module from `schemas/registry/`, and are the halves `verify-contracts.mjs` re-renders and compares against the committed file | n/a |
| `desktop_cannot_open_the_database_or_read_keys` — `tools/phase1-scaffold-policy.test.mjs` | recursive, **every `.rs` anywhere under `crates/desktop`**, comments and literals stripped before matching | three halves, following `only_egress_crate_has_a_socket`. Graph: the declared workspace closure of every edge kind compared whole against a four-entry list, and the resolved closure checked against ten workspace crates that own the database or a key. Link: the resolved shipping closure pinned entire, plus intersections against thirteen database-capable and seventeen key-custody crates. Source: a closed world over path roots — every identifier the crate writes a `::` after must be one of twenty-five reviewed roots, read on paths rather than on `use`, so a fully qualified `rusqlite::Connection::open` is refused; plus no foreign function, no `unsafe`, no environment read and no embedded file. The root allowlist is compared in both directions, so a dead entry fails | `>= 9` files, plus a tripwire: every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the package must be a file the walk read |
| `desktop_names_only_the_core_fixture_allowlist` — same file | none — two fixed paths, `crates/core/src/local_service.rs` and `crates/desktop/src/command.rs` | the one fixture identifier `academic-core` defines, compared against the `as_str` arms of the desktop's `SyntheticFixtureId`, sliced to that `impl` block so the capability arms are not read as fixtures. A source scan because the desktop must have no dependency edge to `academic-core`, which opens the store: the two constants can only be compared as text | none — a missing `impl` block or an unclosed one fails |
| `capability_snapshot_has_no_wildcard` — `packages/ui/src/capability-snapshot.test.ts`, over `packages/ui/src/capability-snapshot.ts` | none — four fixed paths: the two committed snapshot documents and the two vendored Tauri schemas | not a source-text scan; the text it reads is JSON. Three layers: SHA-256 whole-file pins on all four; validation of both snapshot documents against Tauri's own schemas, with negative controls including one showing the schema accepts `$HOME/**` and so is not the guard; and `scanSnapshot`, whose deciding rule is a closed world over reviewed strings, keys and values separately, with the named wildcard forms used only for the failure message | none — a missing file fails the read; the enumeration of forms is compared against its own sample table in both directions so a form that stopped matching fails |
| `route_manifest_matches_ia_exactly` — `packages/ui/src/route-manifest.test.ts`, over `packages/ui/src/ia.ts` | none — one fixed path, `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` | not a source-text scan; the text it reads is the specification. Section 25.1's drawn tree is parsed into labelled nodes with parents and compared with the route manifest as sets in both directions, plus a parent map and the reading order. A line the parser cannot account for raises rather than being skipped | none — an empty parse raises, and a second root or an unaccountable line raises |
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
| `OutboundTransport` | `WHOLE_TRANSPORT_TRAIT` | a second method that writes bytes — the `send_chunk` call-site count reaches a defaulted one through its body and a required one not at all |
| `bind_grant` | `WHOLE_BIND_GRANT` | a change to what a transmission has to agree with before a byte is built — the plan naming the grant the token spends, and the grant recording the rulepack that produced the payload; the two call sites are counted beside it, because a pin on a check says nothing about whether every path runs it |
| `stage`, `deny_on_findings` | `WHOLE_STAGE`, `WHOLE_DENY_ON_FINDINGS` | a change to the staging pipeline's step order, the reason code a step denies with, or any default it takes; the fallback inventory counts sites and cannot see a default that changed direction |
| `cloud_egress_default` | `WHOLE_CLOUD_DEFAULT` | the user closing `GATE-38-028`; it takes no argument, so no quality heuristic can reach it |
| `impl<T> Untrusted<T>` | `WHOLE_UNTRUSTED` | a change to what the trust wrapper hands back — the whole-set `impl` rule refuses a new trait, and this refuses a new inherent method |
| `impl SystemDirective`, `impl ToolDirective` | `WHOLE_SYSTEM_DIRECTIVE`, `WHOLE_TOOL_DIRECTIVE` | a change to what the instruction channels accept |
| `escape`, `impl PromptEnvelope` | `WHOLE_ESCAPE`, `WHOLE_ENVELOPE` | a change to how ingested bytes are quoted, which channel they land in, or what the untrusted span map records |
| `resolve_span`, `adjudicate` | `WHOLE_RESOLVE_SPAN`, `WHOLE_ADJUDICATE` | a change to provenance resolution, to schema validation, or to what the adjudicator is handed — its parameter list is the claim that it holds no capability |
| `envelope_for`, `admit` | `WHOLE_ENVELOPE_FOR`, `WHOLE_ADMIT` | the callers of the two pinned decisions, pinned for the `T141` reason: a pin on a decision says nothing about whether it runs |
| `impl fmt::Debug for RawScore` | `WHOLE_RAW_DEBUG` | a change to what a raw score prints — the last formatting trait that could put an uninterpreted number in front of a reader |
| `DisplayedConfidence::of` | `WHOLE_DISPLAY_OF` | a change to what the display surface accepts; its parameter list is the claim that a displayed confidence has been interpreted |
| `reconcile_transmitted_ranges` | `WHOLE_RECONCILE_DISPATCH` | a change to which reconciliation a recorded transmission reaches; pinned beside the single call site count on `reconcile_egressed`, for the `T141` reason |
| `egress_consumption` | `WHOLE_CONSUMPTION_TABLE` | a change to either foreign key that makes `egress_audit.grant_id` unambiguous for a consumed grant — a key edited to reference something weaker keeps its name, so a name search would miss it |
| `div_round_half_up` | `WHOLE_DIVISION` | a change to how a published average is rounded — not a change to the scale, which is an argument the versioned grading scheme supplies |
| `status_of`, `impl CaptureStatus` | `WHOLE_STATUS_OF`, `WHOLE_CAPTURE_STATUS` | a change to what decides a section 3.7 status, the order of the five tests, or which statuses permit at all |
| `bind_permission`, `impl<'a> ResolvedRequest<'a>` | `WHOLE_BIND_PERMISSION`, `WHOLE_RESOLVE_REQUEST` | a change to what a capture has to agree with before a device opens, or to which absent request field denies — the two callers are pinned beside it and counted at two, for the `T141` and `P2-RF10` reasons |
| `mint_capture_capability`, `continue_capture` | `WHOLE_MINT`, `WHOLE_CONTINUE` | a change to which path reaches the binding, or to whether a refusal leaves its audit row |
| `impl RetentionTerms`, `impl RetentionBound`, `pub struct RetentionTerms` | `WHOLE_RETENTION_TERMS`, `WHOLE_RETENTION_BOUND`, `WHOLE_RETENTION_TERMS_STRUCT` | a change to the two independent axes, to the direction a derivative inherits, or to the order that makes `PROHIBITED` the strictest bound |
| `impl AuthorityGrant`, `impl AttestationRecord` | `WHOLE_AUTHORITY_GRANT`, `WHOLE_ATTESTATION` | a change to what a written authority has to supply, or to what a user's own account of events hands back |
| `preview_expiry`, `apply_expiry`, `impl ExpiryPlan` | `WHOLE_PREVIEW`, `WHOLE_APPLY`, `WHOLE_EXPIRY_PLAN` | a change to what a deletion preview enumerates, to the instant an expiry is compared against, or to whether a plan can exist without a preview |
| `authorize` | `WHOLE_AUTHORIZE` | a change to what the daemon-side evaluation does between a request and a token — it adds no comparison of its own, and the pin is what keeps a second one from appearing beside the binding |
| `open_device`, `record_chunk`, `seal`, `first_unbound_chunk` | `WHOLE_OPEN_DEVICE`, `WHOLE_RECORD_CHUNK`, `WHOLE_SEAL`, `WHOLE_FIRST_UNBOUND` | a change to what a device open compares, to whether a running capture re-runs the whole binding per chunk, or to what the seal reconciles — the three consent call sites are counted beside them |
| `releasable_bytes` | `WHOLE_RELEASABLE_BYTES` | a change to the one place a sealed capture is asked for its bytes; the type-level half is that `QuarantinedArtifact` has no accessor at all |
| `DeviceClass::of`, `DeviceRuleset::for_token` | `WHOLE_DEVICE_CLASS_OF`, `WHOLE_FOR_TOKEN` | a change to which device a medium opens, or to the one constructor that turns a token into a ruleset |
| `CaptureAudit::record_refusal` | `WHOLE_RECORD_REFUSAL` | a change to what an audit row carries, or to whether appending one is what returns the refusal |
| the capture probe's `attempt` | `WHOLE_PROBE_ATTEMPT` | a change to what the probe does to a device — it opens a handle and drops it, and a read added here has to be added to the pin in the same commit |

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
wrote them, and was recorded as `S-11`. `P2-RF10` added the first-argument rule
and recorded the row closed; `T149` then reached the same socket by number
through four imports the rule could not read, so the row was closed twice and
is closed by `P2-RF11`. What the two rules are now, and why the sandbox is not
the answer to either, is in the `P2-RF10` and `P2-RF11` sections below.

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
`.expose()` until `P2-RF10`, and then subtracted the spelling `fn expose` until
`P2-RF11`, which is a second way to read a name as a spelling; see below.

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

Migration `0007` restates eight closed vocabularies as SQL `CHECK` lists.
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
## What the `P2-RF11` repair holds

`P2-A2`'s re-audit measured `P2-RF10`'s four repairs and found every one of them
still reachable, each a layer out from where the repair had looked: the exposure
inventory through a longer function name, the transport count through a new
module, the widened walk through `benches`, and the syscall rule through an
import. No bypass spells a token any list here holds, and all of them compile.

**A count that reads one file is a count about one file.**
`byte_path_pin.rs` counted `execute`, `write_authorized_bytes` and `bind_grant`
in `crates/egress-boundary/src/lib.rs`, and read its fallback inventory from six
file names. The crate had no walk, no module inventory and no public-API
inventory. `T149` added `mod relay;` to `lib.rs` and one new file beside it:
because `EgressProxy` is declared at the crate root, a child module reaches its
private `broker` field, so the new path called `execute` and
`write_authorized_bytes` directly and never called `bind_grant`. Under the same
staged payload, the same token, and a grant reviewed under another rulepack,
`transmit` refused with `ScopeMismatch` and zero bytes and the third path wrote
178 to the transport — with no `egress_grant` row at all, it still wrote 178, and
the journal recorded nothing. It passed `byte_path_pin`, the egress suite,
`cargo test --workspace --all-targets`, and both JS scans.

The counts are now sums over a walk of the package, and
`the_transport_is_reached_from_no_module_but_the_proxy` is what makes the walk
worth counting over: a floor on the files found, a rule that no product source
sits outside `src`, and a tripwire that every `mod name;`, `pub mod name;` and
`#[path]` target is a file the walk read. Beside the sums it holds a per-file
rule — only `lib.rs` may call `execute`, `bind_grant` or
`write_authorized_bytes`, and only `transport.rs` may call `send_chunk` — which
is what a sum cannot say: three call sites are still three if one of them moves
into a new module. `send_chunk` was counted per file in `transport.rs` and is now
counted the same way, because a module that calls the trait method directly
reaches the transport without the broker at all.

Counting `send_chunk` is worth something only while it is the only writer, and
nothing said it was. A second method on `OutboundTransport` with a default body
is caught by that count, because the body calls `send_chunk`; a second
*required* method is caught by nothing — the new name is not one of the counted
ones and there is no body to call anything from. So the trait itself is pinned
as `WHOLE_TRANSPORT_TRAIT`, one method wide. This is `X-T13`, and it was silent
against every other rule in this repair.

`T149` also measured three shapes that keep the call-site count at two while
disabling the binding — swallowing the refusal in an `Err(_)` arm, moving the
call into a branch no caller reaches, and deleting the call and adding a
dead-code decoy. The pin is silent on all three and the named behavioural tests
refuse all three. That layering is what it was built to do and is left alone.

**A subtraction that reads a spelling undoes a count that reads a name.**
`P2-RF10` changed three counts in `trust_scans.rs` to read the identifier on the
use side and left `occurrences(code, "fn expose")` on the declaration side.
`uses_of` does not count `expose_rendered` as a use of `expose`; `occurrences`
does count `pub fn expose_rendered(` as a declaration of it. So one function
whose name merely starts with the guarded one cancels its own call, and with
`pub fn expose_rendered(&Untrusted<IngestedDocument>) -> Box<dyn fmt::Display>`
present — a return type that names no `str`, `String` or `u8`, so neither
public-signature rule sees it either — an integration test outside the crate put
an ingested payload verbatim into a `[SYSTEM]` segment. Renaming the same
function `t149_rendered` failed at once, so what separated pass from fail was
again the prefix and not the reach. The `quote` and `adjudicate` caller counts
carried the identical hole. All three now subtract declarations of a function
named *exactly* the counted name, with `(` or `<` required after it, and the
`saturating_sub` that folded an underflow to zero is an assertion that the two
counts agree. `byte_path_pin.rs` had already required the `(`; it now requires
the same and accepts a generic list, because `write_authorized_bytes` has one.

**A rule that reads a call spelling holds only while the call must spell it.**
The `libc::syscall(` first-argument rule reads a path-qualified call. `T149`
wrote the same numeric socket three other ways — `use libc::syscall;`,
`use libc::syscall as raw;`, and `use libc::*;` in a file whose allowance lists
nothing — and all three passed every scan and `clippy -D warnings`. The `use`
item itself carries the spelling `libc::syscall`, which is what satisfied the
allowance while the call matched nothing; the glob import carries no spelling at
all, so it reached no allowance to fail. Importing `libc::syscall` is now refused
in every file, under every shape.

That closure was then walked around three more times inside this repair, each
time one layer further out, and each hole is closed with the injection that
found it recorded beside it. `extern crate libc as raw;` needs no `use` item and
the alias rule was keyed on `use`; it now reads both spellings.
`use libc::{self as l};` renames the crate root while naming it only in the
statement head, where nothing is renamed -- `self` is now resolved to the path
its brace hangs off and judged as that path, which refuses this and leaves the
repository's three existing `use rustix::fs::{self as rfs, …}` statements alone,
because `fs` is neither a crate root nor a socket module. And
`let raw = libc::syscall; raw(41, 2, 1, 0)` takes the function as a value: it
spells `libc::syscall`, so the allowance is satisfied, and it never writes
`libc::syscall(`, so the first-argument rule reads nothing. Every mention of the
name in that file must now be a call.

**A walk that skips a tree for a reason about another tree.** Five walks
excluded `benches` beside `tests`. The reasons written for the exclusion are
reasons about `tests`. A bench target has no feature gate and
`cargo clippy --workspace --all-targets` compiles it, which is the two-part test
`T146` applied to `examples/`. All five now exclude `tests` only. Nothing exists
under `benches` today; this is what stops the first one from being a tree no
scan reads.

### The injection matrix

Forty injections, applied one at a time, each reverted with its file's
SHA-256 checked back to its recorded value and its first check re-run, on
Windows native and WSL2 Linux. Every injection that touches product source was
compiled alone first, so no refusal below is a compile error wearing a guard's
name. Fourteen are `T149`'s own probes replayed verbatim, marked below; every
one of those passed before this repair. Five of the rest are this repair's own
step-outs: `X-T5`, `X-T9`, `X-T11` and `X-T13` each passed a version of this
repair before the rule beside them existed, and `X-T10` passed for a reason that
turned out to be the wrong one.

| # | Injection | Refused by |
|---|---|---|
| X-E1 | `mod relay;` and a third transport path in a new file (`T149`'s `X-E1`) | `the_transport_is_reached_from_no_module_but_the_proxy`, and the walked call-site sums |
| X-E1-suite | the same, against the whole egress suite | the same |
| X-E1-full | the same, against `cargo test --workspace --all-targets` | the same — this is the command that passed before this repair |
| X-T1 | a new module that calls `send_chunk` directly, with no broker at all | the `send_chunk` sum and the per-file caller rule |
| X-T12 | a second `OutboundTransport` method with a default body, reached from a new module | the `send_chunk` sum — the default body calls it |
| X-T13 | the same as a *required* method, so nothing calls `send_chunk` | `WHOLE_TRANSPORT_TRAIT` — nothing else saw it, because the new name is not one of the counted ones |
| X-T2 | control: a new module that reaches nothing | nothing — it is allowed, which is what keeps the tripwire from being a rule against adding modules |
| X-T4 | `fn bind_grant_later` calling `bind_grant`, in `lib.rs` | the `bind_grant` sum — the longer name is not subtracted as a declaration |
| X-U1 | `pub fn expose_rendered(…)` cancelling its own call (`T149`'s `X-U1`) | `every_exposure_site_is_named_and_justified` |
| X-U1-suite | the same, against the whole `academic-untrusted-content` suite | the same |
| X-U1-full | the same, against `cargo test --workspace --all-targets` | the same |
| X-U1c | control: the same reach named `t149_rendered` | the same, as before this repair |
| X-U2 | `pub fn quote_into(…)` cancelling its own call (`T149`'s `X-U2`) | `the_instruction_channel_takes_only_static_text` |
| X-U2c | control: the same caller named `t149_quote_into` | the same, as before |
| X-U3 | `pub fn adjudicate_into(…)` cancelling its own call (`T149`'s `X-U3`) | `the_adjudicator_receives_no_capability` |
| X-U3c | control: the same caller named `t149_adjudicate_into` | the same, as before |
| X-T3 | the same declaration-subtracting exposure site, in a module file that did not exist | the exposure inventory, over the package walk |
| X-K1 | `use libc::syscall as raw;` then `raw(41, 2, 1, 0)` (`T149`'s `X-K1`) | the import ban |
| X-K2 | `use libc::syscall;` then `syscall(41, 2, 1, 0)` (`T149`'s `X-K2`) | the import ban |
| X-K3 | `use libc::*;` then `syscall(41, 2, 1, 0)`, in a file with an empty allowance (`T149`'s `X-K3`) | the import ban, which is checked in every file |
| X-T6 | `use libc::{c_long, syscall};` then `syscall(41, 2, 1, 0)` | the import ban |
| X-T5 | `extern crate libc as raw;` then `raw::syscall(41, 2, 1, 0)` | the alias rule, now read on `extern crate` as well as `use` |
| X-T9 | `use libc::{self as l};` then `l::syscall(41, 2, 1, 0)` | the alias rule, with `self` resolved to the path its brace hangs off |
| X-T10 | `libc::syscall` bound to a typed function value, then called | the rule that every mention of the name is a call; the `extern "C"` in its type is refused separately |
| X-T11 | the same with the type inferred, so no `extern "` is spelled | the rule that every mention of the name is a call — this is the one `X-T10` would have hidden |
| X-K2c | control: `libc::syscall(41, 2, 1, 0)`, the spelling the argument rule reads | the first-argument rule, as before |
| X-K3c | control: the same call in the file with an empty allowance | the allowance comparison, as before |
| X-T7 | control: `use libc::{c_int, c_long};`, a libc import naming no syscall | nothing — the ban reads a whole identifier, so `syscall_thing` and `c_long` are not it |
| X-W1 | an `f64` in a new `crates/record/benches/` file (`T149`'s `X-W1`) | `no_float_reaches_the_gpa_path`, now walking `benches` |
| X-W2 | `#[derive(Debug)]` over `key_bytes` in the same tree (`T149`'s `X-W2`) | `tools/secret-debug-policy.test.mjs`, now walking `benches` |
| X-W3 | `std::net::TcpStream::connect` in the same tree (`T149`'s `X-W3`) | `phase1_exit_has_no_product_network`, now walking `benches` |
| X-W3b | the same file, against the socket scan (`T149`'s `X-W3b`) | `only_egress_crate_has_a_socket`, which already read that tree |
| X-W4 | control: an `f64` in a new `build.rs` (`T149`'s `X-W4`) | `no_float_reaches_the_gpa_path`, as before |
| X-T8 | the same bench file, against `cargo clippy --workspace --all-targets -- -D warnings` | the clippy lane, which is what makes `benches` product-shaped |

The six existing `libc::syscall(libc::SYS_…)` calls in the sandbox backend are
the control for the import ban: the unmodified tree passes, and none of them
needs an import.

`X-E2`, `X-E3` and `X-E4` — the three shapes that keep the call-site count at
two while disabling the binding — are recorded above rather than here: the pin
does not see them and the named behavioural tests do, which is the layering
working, not a gap.

## What the `P2-M1` scans hold

`P2-M1`'s claim is a shape of the source in two places. A provider's raw score
is unrankable because the type implements no ordering and hands back no number,
and a displayed confidence has been interpreted because the display constructor
takes a type only the registry issues. Neither has a run-time observation that
would notice the day somebody adds the trait or the accessor: a new
`impl PartialOrd for RawScore` makes the compile-fail case compile and leaves
every behavioural test passing.

Two more things are held here that are not about this crate's source at all, and
are listed because the same injection matrix measures them: the two foreign keys
`P2-M1`'s reconciliation keys on, and migration `0007`'s supersession guard.

**None of the rules is a token list.** The `impl` set naming `RawScore` is
compared whole against a two-entry list, so a trait nobody predicted fails as an
extra key -- `M-I6` is a *local* trait that spells none of the nine trait names
any list in this file holds. The number rule reads the return type as
identifiers rather than searching for `u32`. The calibrated-value inventory is a
whole set of public signatures with a written reason for each. And
`egress_consumption` is pinned as whole text rather than searched for by name,
because a foreign key edited to reference something weaker keeps its name.

### What the reconciliation does not read

`P2-M1` needs `egress_audit.grant_id` disambiguated. A discriminator column on
that table would do it and is not there: `T149` measured that
`egress_consumption` already answers the question, because `grant_id` references
`egress_grant` and `(egress_audit_seq, grant_id)` references
`egress_audit(audit_seq, grant_id)`. The reconciliation joins through that table
instead, and the scan holds two things: the pin on the table, and a rule that
the reconciliation names no `row.grant_id` at all.

The control is inside the named test rather than in the matrix, and runs on
every build: `an_audit_row_from_the_other_namespace_is_not_the_grant` executes a
`grant_id`-only reconciliation beside the product one and **requires it to
accept** the forged grant. If the join ever stopped being what refuses that
grant, that assertion is what fails.

### What `M-I2` found in this task's own test

`crates/policy/tests/consumption_join.rs` first tried the mismatched pair with
an unminted second grant. `grant_id`'s own foreign key refuses that case, so the
composite key was never the thing under test: dropping it changed nothing and
the test passed. `M-I2` is the observation. Both grants are now real
`egress_grant` rows, the composite key is the only constraint left to refuse the
pair, and `M-I2` fails.

### The injection matrix

Nineteen injections, applied one at a time, each reverted with its file's
SHA-256 checked back to its recorded value, on Windows native and WSL2 Linux
with the same result on both. Every one builds -- `cargo build --workspace` is
run on the injected tree before the refusing command, because a refusal that is
a compile error proves nothing about the guard. Every one is refused.

| # | Injection | Refused by |
|---|---|---|
| M-I1 | `reconcile_egressed` stops filtering consumptions by the grant the run named | `transmitted_ranges_reconcile_with_egress_audit` |
| M-I1b | the reconciliation reads `egress_audit.grant_id` directly | `the_consumption_join_is_the_only_key_into_the_audit` |
| M-I2 | the composite foreign key on `egress_consumption` is dropped | `a_consumption_row_cannot_name_a_grant_the_audit_row_does_not` |
| M-I2b | the same edit, against the pin | `WHOLE_CONSUMPTION_TABLE` |
| M-I3 | the `egress_grant` foreign key on `egress_consumption` is dropped | `a_consumption_row_cannot_name_a_grant_the_audit_row_does_not` |
| M-I3b | the same edit, against the pin | `WHOLE_CONSUMPTION_TABLE` |
| M-I4 | the consumption names the decision row instead of the transmission | the ordering rule beside the pin |
| M-I5 | a new module implements `PartialOrd for RawScore` | the whole `impl` set, and the `compile_fail` case that stops failing |
| M-I6 | a local trait ranks two `RawScore`s, spelling no listed trait name | the whole `impl` set |
| M-I7 | `RawScore` gains an accessor returning its number | `raw_score_hands_back_no_number`, and the `compile_fail` case |
| M-I8 | `RawScore`'s hand-written `Debug` prints the number | `WHOLE_RAW_DEBUG`, and `uncalibrated_score_cannot_be_displayed` |
| M-I9 | a second producer of `CalibratedConfidence`, one crate out | `every_calibrated_value_comes_from_the_registry` |
| M-I10 | the pinned display constructor is edited | `WHOLE_DISPLAY_OF` |
| M-I11 | one of the twelve fields is renamed away from its section 27.3 key | `model_run_requires_every_field`, against the spec's own YAML block |
| M-I12 | migration `0007` drops one of the twelve storage sites | the same row's storage half |
| M-I13 | `record_digest` stops covering the cost | `the_record_constructor_takes_every_field` |
| M-I14 | migration `0007`'s supersession guard is deleted | `a_reanalysis_addresses_the_subject_it_supersedes` |
| M-I15 | the candidate table's append-only `UPDATE` trigger is removed | `reanalysis_creates_new_candidate_not_mutation` |
| M-I16 | product code outside `src`, reached by `#[path]` | the `#[path]` rule and the product-source-under-`src` rule |

`M-I5` and `M-I7` are recorded as refused by the `compile_fail` suite as well as
by a scan, because that is what happens: the injected accessor and the injected
ordering both make a committed case compile, and `trybuild` fails on the
diagnostic mismatch rather than on the case passing silently.

`M-I11` renames the field rather than removing it, deliberately. Removing it
breaks every caller and the refusal would be a compile error, which proves
nothing about the guard; renaming keeps the arity and the positional call sites
intact, so what fails is the comparison against the spec.
## What the `P2-L1` scans hold

`P2-L1` adds a device gate whose claims are mostly behaviours — an operating
system refuses an open, or it does not — and four that are shapes of the source:
that a quarantined artefact has no byte accessor anywhere in the workspace, that
every refusal this crate returns appended a row, that `unsafe` is confined to
the two platform backends, and that the Linux backend names only the three
syscalls it installs a ruleset with. Those four are
`crates/capture-gate/tests/capture_scans.rs`.

### The allowance that needed a rule, and the rule that was keyed on one file

`only_egress_crate_has_a_socket` had to be widened again: reaching Landlock
means calling `libc::syscall`, because there is no libc wrapper for it, and that
spelling is on the socket pattern list. `P2-G4` widened it once for
`crates/worker/src/sandbox/linux.rs` and wrote the first-argument rule that
makes such an allowance non-empty — and keyed that rule on that one file's name.

A second allowance entry with no rule behind it is the hole this page is about,
so the rule is now keyed on `RAW_SYSCALL_FILES`, a map from file to the syscalls
that file may make. A file on the allowance for `libc::syscall` that is not a
key there fails; a call whose first argument is not one of *that file's own*
reviewed names fails; and a reviewed name the file no longer calls fails as a
stale exception. The worker's file keeps its extra rule — every other `SYS_`
name must sit inside `denied_syscalls` — because that file also builds a seccomp
deny list and the capture gate does not.

### The empty guard this task found in its own suite

`record_fail_closed` walks five record cases and then checked that every case in
the array had been walked. That is true of any array whatever it holds.
Injection `L-I15b` — replacing `Expired` with a second `Valid`, so the length is
unchanged and the file compiles — passed it and dropped a fail-closed case from
the suite silently.

The check is now an index from a `match` over the enum, which a duplicate, a
removal and a reorder all fail. The same shape was then looked for in every
other enumeration this task wrote and found in two more: `REFUSAL_REASONS`
(`L-I16`) and `DEVICE_CLASSES` (`L-I17`). Both are closed the same way, and
`DEVICE_CLASSES` is additionally compared against the variant list read out of
its own enum.

### The injection matrix

Eighteen injections, applied one at a time, each reverted with its file's
SHA-256 checked back to its recorded value, on **Windows native and WSL2 Linux
with the same result on both**. Seventeen of the eighteen compile clean — a
rejection that is a compile error proves nothing about a guard, because the
guard never ran — and the compile is checked in the same pass with
`cargo build -p academic-capture-gate --all-targets`.

Eight of them spell **none** of the tokens the guards they defeat hold:
`L-I1`, `L-I2`, `L-I3`, `L-I5`, `L-I7`, `L-I9`, `L-I15b`, `L-I16`.

| # | Injection | Compiles | Refused by |
|---|---|---|---|
| L-I1 | `record_chunk` reaches `continue_capture` only at `u64::MAX`, so a chunk past the boundary is accepted | yes | `token_expiry_stops_capture_at_the_boundary`, the `seal` reconciliation, and the `continue_capture` call-site count |
| L-I2 | `open_device` stops comparing the class against the token's ruleset | yes | `audio_only_permission_denies_camera` and `WHOLE_OPEN_DEVICE` |
| L-I3 | `seal` always returns the releasable arm | yes | `a_chunk_recorded_past_the_boundary_quarantines_the_artefact` and `WHOLE_SEAL` |
| L-I4 | the unclassified-medium arm opens a microphone instead of nothing | yes | `every_capture_medium_is_classified` and `WHOLE_DEVICE_CLASS_OF` |
| L-I5 | a refusing path returns without appending its audit row | yes | the construction/append equality, and `capture_audit_records_every_denial` |
| L-I6 | `QuarantinedArtifact` gains a byte accessor | yes | the byte-returning signature set, and the `compile_fail` case stops failing |
| L-I7 | a signature elsewhere in the crate turns a quarantined artefact into a `String`, which the byte-set rule cannot see | yes | the workspace-wide signature rule |
| L-I8 | an `unsafe` item appears outside the two platform backends | yes | `unsafe_is_confined_to_the_device_backends` |
| L-I9 | the Linux backend makes a raw syscall by number | yes | the first-argument rule, in both scans |
| L-I10 | the Linux backend names `SYS_memfd_create` | yes | the reviewed-name rule, in both scans |
| L-I11 | a `#[path]` module beside `src` holds a second device open | yes | the walk's `#[path]` tripwire |
| L-I12 | the probe reads from the handle it opened | yes | `the_probe_opens_a_handle_and_reads_no_sample` |
| L-I13 | the Linux ruleset adds a rule for every tree, ignoring the token | yes | `the_kernel_splits_by_the_tokens_media_set` — Linux only |
| L-I14 | `for_token` derives every device class regardless of the media set | yes | `audio_only_permission_denies_camera` and `WHOLE_FOR_TOKEN` |
| L-I15 | a record case is dropped from the enumeration | **no** | the compiler: the array length is part of its type |
| L-I15b | a record case is replaced by a duplicate, so the length is unchanged | yes | the witness index — **and nothing, before this task's own repair** |
| L-I16 | a refusal reason is replaced by a duplicate in `REFUSAL_REASONS` | yes | the witness index — the same shape one step out |
| L-I17 | a device class is replaced by a duplicate in `DEVICE_CLASSES` | yes | the enum's variant list, read out of the source |

`L-I15` is recorded as not compiling because that is what it is: dropping an
entry from a `[RecordCase; 5]` is a type error, which is a stronger refusal than
any test. It is listed anyway, with `L-I15b` beside it as the shape that does
compile, because a matrix that quietly dropped the case that the compiler
catches would be claiming the test caught it.

## What the `P2-M2` scans hold

`P2-M2`'s claim is a shape of the source in three places. The payload of a
`Proposed<T>` comes out at three named sites and nowhere else; the tier of a
queued proposal is compared against the workflow a caller reached for at every
door; and the receipt that separates a user from an automatic actor has one
producer. None has a run-time observation that would notice the day it stops
being true: a fourth release site, a door that stopped comparing, or a second
way to mint a user receipt each leaves every behavioural test passing.

**None of the rules is a token list.** The `impl` set naming `Proposed<` is
compared whole against a two-entry list, so a trait nobody predicted fails as an
extra key. The release inventory counts the accessor's *name*, so
`Proposed::release(taken)` is the same call as `taken.release()`; it subtracts
declarations of a function named exactly `release`, so `fn release_now(` cannot
cancel its own call. The door table is the whole set of the queue's public
`&mut self` methods, so a fifth door fails as an extra key however it is named.

### The pins fix their callers

`WHOLE_REQUIRE` pins the one place a tier is compared against a workflow.
`T141`'s lesson is that a pin on a decision says nothing about whether the
decision runs, so `DOOR_GUARDS` pins the **first statement** of each of the four
doors beside it, and a call-site count holds the comparison at four reachers.
A door that stopped calling it, or that called it behind an `if`, fails on the
first-statement pin rather than on the count alone.

`WHOLE_USER_DECISION` pins the whole inherent surface of the user receipt, for
the reason `WHOLE_UNTRUSTED` pins the untrusted wrapper's: an inherent
`pub fn forge` would name no trait and would pass a rule that only read trait
implementations. A companion rule requires the set of `impl` blocks naming
`UserDecision` to be that one block.

### The twelve injections, and what each one measured

Each was applied to the working tree, the suite was observed failing, the change
was reverted, and the suite was observed passing again.

| Injection | What it is | What caught it |
|---|---|---|
| `M2-I1` | a fourth release site written `Proposed::release(taken)` through the type path | `every_release_site_is_named_and_justified` (extra key), `every_settlement_door_is_named` |
| `M2-I2` | a fourth release site named `release_now`, whose declaration a spelling-subtracting count would let cancel its own call | the same two |
| `M2-I3` | a release site in a `#[path = "../extra/side.rs"]` module outside `src` | `the_walk_reads_every_module_in_this_crate` (product source outside `src`) and the release inventory |
| `M2-I4` | `impl Deref for Proposed<T>` | the `proposed_has_no_unwrapping_trait` compile-fail case, and the pinned `impl` set |
| `M2-I5` | two rows of the tier-to-workflow mapping swapped | `every_tier_reaches_only_its_own_workflow` |
| `M2-I6` | `Autosaved::EPISTEMIC_STATUS` changed to `USER_CONFIRMED` | `low_risk_autosave_is_marked_ai_inferred` |
| `M2-I7` | a batcher that drops the last member of every group | `high_volume_proposals_are_batched_without_loss` (set equality) |
| `M2-I8` | an undo that removes the record it reverses instead of appending | `medium_risk_requires_queue_and_undo`, `rejected_proposal_is_retained` |
| `M2-I9` | a rejection that removes the queue entry | the same two, plus the batching partition |
| `M2-I10` | `Actor::Importer` admitted as a user | `non_delegable_has_no_automatic_actor_path`, `high_risk_requires_explicit_approval`, `the_user_receipt_has_one_producer` |
| `M2-I11` | the explicit approval's identity check removed | `high_risk_requires_explicit_approval` |
| `M2-I12` | `decided_at` dropped from the record digest | `the_disposition_digest_covers_every_field` |
| `M2-I13` | a door whose `&mut self` sits five lines into a wrapped signature | `every_settlement_door_is_named`, after the window it read was replaced by the whole signature |

`M2-I1` and `M2-I2` are the two shapes `P2-RF10` and `P2-RF11` repaired in the
untrusted-content inventory. They are injected here rather than assumed,
because the repaired helpers were copied into a new file and a copy that
silently lost a clause would pass every other rule.

`M2-I13` is this task's own instance of the same class, found by asking the
question one layer out about a rule this file had just written. The door rule
originally decided "is this a mutating method" by looking four lines ahead for
`&mut self`, which is a *window* and not a structure: a door whose signature
rustfmt wrapped one line further would not have been inserted into the observed
set at all, and a door nothing observes is not a missing key -- it is a hole no
comparison covers. The rule now reads the whole signature, from `pub fn` to the
`{` or `;` that ends it, and asserts it found that terminator.

### What the migration holds that the crate cannot

The Rust boundary refuses these shapes in memory. Migration `0009`'s triggers
refuse them again against rows a process outside this repository could insert:
`guard_proposal_disposition_actor` refuses a non-user actor,
`guard_proposal_low_risk_is_not_disposed` refuses a user decision on an
autosaved proposal, `guard_proposal_high_approval_is_explicit` requires the
explicit-approval flag on that tier and refuses it on the others, and
`guard_proposal_outcome_matches_tier` refuses an outcome whose epistemic status
or disposition does not match the tier. Each is observed refusing in
`crates/store/src/proposal_closure_tests.rs` against a control that is accepted.
## What the `P2-X1` scans hold

`P2-X1`'s claim is a boundary and a snapshot, and neither has a run-time
observation that would notice the day it stops holding. A desktop crate that
gained a dependency on the store would keep every behavioural test passing until
somebody wrote the call; a capability file that gained a filesystem scope would
keep validating against Tauri's own schema, which accepts one.

**No Tauri runtime is linked.** `crates/desktop/tauri.conf.json` and
`crates/desktop/capabilities/desktop.json` are committed configuration, checked
against Tauri's own published config schema and against the schema generated
from `tauri_utils::acl::capability::Capability`. No window opens, and no
assertion here says one does. What the snapshot is evidence for is its own
content: `P2-A2` left the desktop capability diff `NOT_RUN` because there was
nothing to diff, and there is now.

**The deciding rule for the snapshot is not a list of wildcard shapes.**
`WILDCARD_FORMS` enumerates ten and is used for the failure message. What
decides is `closedValueWorld`: every string in either document, keys and values
in separate closed sets, must be one that was reviewed. That is why `X-I38`
through `X-I41` are refused — a fullwidth asterisk, a bare drive root, a
protocol-relative source, a `data:` scheme, a plugin permission namespace and a
plugin declaration with an empty body carry no form the enumeration names.

**The boundary is judged three ways, because each is blind to a different
bypass.** The declared-edge closure misses an optional dependency a feature
turns on; the resolved closure misses a crate that links a capability and has
not used it yet; the source scan misses everything that spells no forbidden name,
which is the whole of "add a dependency". `X-I3` is the optional edge behind a
feature nobody enables, and the resolved closure is what refuses it.

**The source half is a closed world over path roots, not a token list.** Every
identifier the crate writes a `::` after must be one of twenty-five reviewed
roots. A `use`-root allowlist would not see `rusqlite::Connection::open` written
in full, which is `X-I5`.

### What `X-I28` found in this task's own tests

`backlinks_resolve_for_four_entity_types` first compared the rendered view's
backlink list against `backlinksOf`, which is the function that produced it.
`X-I28` — `backlinksOf` filtering the wrong end of each edge — passed, because
both sides of the comparison moved together. The expectation is now derived in
the test from the relation table, and `X-I28` fails. The same round also found
the test reading one representative entity per kind rather than every entity;
it now walks the whole corpus.

### The injection matrix

Forty-four injections, applied one at a time and reverted, plus two more
recorded below as not violations. Each dependency
injection refreshes `Cargo.lock` before the refusing command is run, because a
refusal that is `--locked` complaining about a stale lockfile proves nothing
about the guard; each source injection is compiled first, for the same reason.
Every one is refused, except the two rows marked as not violations.

| # | Injection | Refused by |
|---|---|---|
| X-I1 | the desktop takes a product edge to `academic-store` | `desktop_cannot_open_the_database_or_read_keys`, declared and resolved halves |
| X-I2 | the desktop takes a dev edge to `academic-vault` | the same, declared half — a dev edge is still a compiled edge |
| X-I3 | the desktop takes an optional edge to `academic-crypto` behind a feature nobody enables | the same, resolved half |
| X-I4 | the desktop takes a direct `rusqlite` edge | the same, link half and the closure pin |
| X-I5 | a fully qualified `rusqlite::Connection` path with no `use` | the same, source half — the closed world over path roots |
| X-I6 | product code outside the package's `src`, reached by `#[path = "../extra/leak.rs"]` | the `#[path]` tripwire |
| X-I7 | an `option_env!` read of a database path | the environment rule |
| X-I8 | the desktop's fixture identifier drifts from `academic-core`'s | `desktop_names_only_the_core_fixture_allowlist` |
| X-I9 | the desktop names a capability the daemon does not negotiate | `desktop_command_allowlist_equals_the_negotiated_capabilities` |
| X-I10 | a read-only command grows a write arm | `every_write_command_binds_the_capability_the_daemon_expects` |
| X-I11 | a variant is dropped from `DesktopCommand::ALL` | `desktop_command_allowlist_equals_the_negotiated_capabilities` |
| X-I12 | `Optimistic<T>` gains an accessor | `optimistic_update_has_no_exit_but_a_receipt`, whose committed diagnostic stops matching |
| X-I13 | `confirm` stops comparing one of the four bound fields | `a_receipt_that_differs_in_any_bound_field_promotes_nothing` |
| X-I14 | `Debug` stops redacting the unaccepted value | `debug_does_not_print_the_unaccepted_value` |
| X-I15 | the TypeScript optimistic wrapper carries its value on the object | `optimistic_update_is_not_canonical_before_receipt` |
| X-I16 | a route is removed from the manifest | `route_manifest_matches_ia_exactly`, specification-to-manifest direction |
| X-I17 | a route is added to the manifest | the same, manifest-to-specification direction |
| X-I18 | a line is removed from section 25.1 | the same |
| X-I19 | a line is added to section 25.1 | the same |
| X-I20 | a line in section 25.1 is renamed | the same |
| X-I21 | a route loses its registered view | `every_destination_opens`, registry-to-manifest equality |
| X-I22 | one view renders no sections | the same |
| X-I23 | navigation drops the drawer for index destinations | `evidence_drawer_persists_across_views` |
| X-I24 | navigation rebuilds the drawer instead of carrying it | the same |
| X-I25 | the drawer moves to the left | the same, and `every_destination_opens` |
| X-I26 | the palette stops offering one entity kind | `palette_reaches_four_entity_types_from_every_route` |
| X-I27 | the palette targets the wrong route for one entity kind | the same, and `backlinks_resolve_for_four_entity_types` |
| X-I28 | `backlinksOf` filters the wrong end of each edge | `backlinks_resolve_for_four_entity_types` — see above; it did not, before this round |
| X-I29 | every relation into `Project` is removed | the same, four-kind coverage half |
| X-I30 | a relation names an entity the corpus does not hold | the same, corpus-integrity half |
| X-I31 | a backlink opens the index form instead of the entity | the same, and the palette matrix |
| X-I32 | a `$HOME/**` asset-protocol scope | the scope rule, the closed value world, and the named form |
| X-I33 | a wildcard CSP source | the CSP directive rule and the closed value world |
| X-I34 | an `http://**` CSP source | the same |
| X-I35 | a scheme-less host in a remote capability origin | the `remote` rule and the closed value world |
| X-I36 | a `fs:` plugin permission in the capability | the permission allowlist and the closed value world |
| X-I37 | a `shell` plugin declared with an empty configuration | the empty-`plugins` rule and the closed key world |
| X-I38 | a filesystem scope written as a fullwidth asterisk | the closed value world |
| X-I39 | a filesystem scope written as a bare drive root, no metacharacter | the closed value world |
| X-I40 | a protocol-relative CSP source | the closed value world |
| X-I41 | a `data:` CSP source, which names no host | the closed value world |
| X-I42 | the CSP drops a directive instead of widening one | the CSP directive rule |
| X-I43 | the committed snapshot file is edited at all | the SHA-256 whole-file pin |
| X-I44 | a vendored Tauri schema is swapped for a permissive one | the schema pin, and the validation losing its negative controls |

`X-I38` through `X-I41` are the point of the closed value world: each is a
breadth grant written in a shape `WILDCARD_FORMS` does not name, and each is
refused anyway. `X-I32` through `X-I37` are refused twice over, once by a named
rule and once by the closed world, which is what a layered guard should do.

Two injections are recorded as **not** violations, because the record is more
useful than the omission. Removing one relation edge from the synthetic corpus
leaves every one of the four entity kinds with a resolvable backlink, so nothing
should refuse it and nothing does; and `resolveBacklinks` filtering a dangling
edge instead of raising changes nothing on a corpus that has none. The dangling
case itself is `X-I30`, and the corpus-integrity loop is what refuses it.

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
| S-10 | `tools/secret-debug-policy.test.mjs` | `SECRET_FIELD_NAMES` holds `payload` and `payload_bytes` and not the generic names a raw buffer actually hides behind. `T146` measured four more that pass today: `text`, `escaped`, `bytes`, and `staged_text`, against the control `payload`, which fails. Adding `bytes` alone reaches four pre-existing sites — `WireField.bytes` (`crates/rpc/src/convert.rs`), `FingerprintEncoder.bytes` (`crates/store/src/schema_fingerprint.rs`), `SyntheticTranscriptPdf.bytes` (`crates/transcript/src/source.rs`), `StreamingPrefix.bytes` (`crates/vault/src/object.rs`); `text` and `escaped` reach `QuotedDocument` and `RenderedPrompt` in `crates/untrusted-content`, and `staged_text` reaches `crates/egress-boundary/src/stage.rs`. | Now, for any site that holds something private. Nothing leaks today: all four `P2-G4`/`P2-G5`/`P2-G2` types — `QuotedDocument`, `RenderedPrompt`, `StagedOutput`, `AcceptedOutput` — have hand-written `Debug` impls, and the four `bytes` sites are public buffers. What is open is the **net**, not any site. Severity **P2**, raised from the earlier reading: the vocabulary trails the code by at least the six generic names `bytes`, `text`, `escaped`, `staged_text`, `value` and `output`, and each new crate has added to the gap. `P2-RF10` recorded four of the six; `T149` found `value` and `output`, and `P2-RF11` re-measured all six. **The cost of closing it is measured rather than estimated.** Adding `bytes`, `text`, `escaped` and `staged_text` to `SECRET_FIELD_NAMES` fires 13 sites in 8 crates: `Alias.text`, `PartialAlias.text`, `RegistryFact.text` and the tuple variant `ClaimObject::Text(String)` in `academic-domain`; `SearchHit.text` and `ExactSymbolHit.text` in `academic-projections`; `AliasSpec.text` in `academic-store`; `JsonValue::Text` in `academic-test-support`; and `CorpusFile.bytes`, `WireField.bytes`, `FingerprintEncoder.bytes`, `SyntheticTranscriptPdf.bytes`, `StreamingPrefix.bytes`. Adding `value` and `output` beside them fires four more, in two crates the first four do not reach: `ToolVersionCase.output` (`crates/cli/src/commands/doctor.rs`), `ScenarioAssumption.value` (`crates/scenario/src/simulate.rs`), and `EngineError.value` and `RegistryError.value` in `academic-domain` — 17 sites in 10 crates for all six. The four `untrusted-content` and `egress-boundary` types the vocabulary was widened *for* fire nothing, because all four already hand-write `Debug`. Six is a floor and not a total: widening further to `detail` and `message` fires around twenty more, and those are error-report fields on types that carry no user content, so a wider list is a different decision from this one and not a larger version of it. So the work is not the vocabulary line: it is a redaction decision about the eight `text` sites, which hold entity surface forms, indexed content and claim values — user content, not public buffers — spread over four crates whose contracts this row's task did not read. A `PUBLIC_BYTES` entry silences a field permanently, and writing eight of them to close one row would trade this row for a worse one. Closing it means one commit per crate from its owner, redacting rather than declaring. `P2-G6` added a crate and did not widen this row: `academic-consent` declares no `text`, `bytes`, `escaped` or `staged_text` field at all, because every evidence item it holds is a locator plus a digest plus a byte count and its one place for prose is a closed `NotApplicableReason` enum. `P2-X1` added a crate that declares none of those four either, and **two** `value` fields, so the `value` half of the six-name count is 19 sites in 11 crates rather than 17 in 10. Its owner's decision, made rather than deferred: `Optimistic<T>.value` in `crates/desktop/src/optimistic.rs` already hand-writes a redacting `Debug` and would need a registration line and no redaction work, because it holds an edit the core has not accepted; `Canonical<T>.value` beside it derives `Debug` and **should keep printing**, because it holds a value the core returned a receipt for and the surface is required to display. The seal is on the pending state and the accepted state is deliberately printable, which `debug_does_not_print_the_unaccepted_value` asserts in both directions. So this crate adds one registration line to the cost of widening the vocabulary and no redaction decision, and it writes no `PUBLIC_BYTES` entry. |
| S-11 | `only_egress_crate_has_a_socket` — the spelling half | **Closed by `P2-RF11`.** `P2-RF10` recorded this row closed and it was not: its rule reads the *call* spelling `libc::syscall(`, and `T149` reached the same socket by number through `use libc::syscall;`, `use libc::syscall as raw;` and `use libc::*;`, each of which compiles, passes `clippy -D warnings`, and carries the spelling only in the `use` item the allowance reads. The scan's own comment beside the rule said the gap was open while this row said closed. Three rules now hold it together: no file may import `libc::syscall` under any of those shapes, and no file may rename `libc` through `extern crate libc as …` or `use libc::{self as …}`, so a call spells the path; every mention of the name in the sandbox backend is itself a call, so the function cannot be bound to a value and called through that; and every `libc::syscall(` call there must name a reviewed `libc::SYS_` constant as its first argument. `P2-RF11` found the last two of those by walking around its own first fix. Why the three reasons this row originally gave for leaving it open were all wrong is in the `P2-RF10` section above. | n/a — closed. |
| S-12 | `os_keystore_capabilities_are_available_but_unused` — `tools/phase1-scaffold-policy.test.mjs` — `tools/secret-debug-policy.test.mjs`, `no_float_reaches_the_gpa_path`, and `phase1_exit_has_no_product_network` | **Closed by `P2-RF10`, and widened again by `P2-RF11`.** All four walked `<crate>/src`; all four now walk the package, less `tests` only, and the eight files outside `src` that spell `process::Command` each carry the reason they are allowed. `P2-RF10` left `benches` out beside `tests`; the reasons it wrote for that were reasons about `tests` — this repository's own suites name `f64` and open the local IPC seam on purpose — and said nothing about benches. See `S-14`. The tree this row named was not the first of its kind — `crates/record/examples/` arrived a commit earlier and has no feature gate — which is why widening only the two walks this row listed would have closed half of it. | n/a — closed. |
| S-13 | `only_egress_crate_has_a_socket` — the syscall rule's file scope | **Closed by `P2-L1`.** The first-argument rule read `crates/worker/src/sandbox/linux.rs` and only that file, because that was the one file whose allowance listed `libc::syscall`; the half about a call reaching its allowance at all was closed by `P2-RF11`'s import ban. What stayed open was the **future second allowance entry**, and `P2-L1` is it: `crates/capture-gate/src/native/linux.rs` reaches Landlock the same way. The rule is now keyed on `RAW_SYSCALL_FILES`, a map from file to the syscalls that file may make — a file on the allowance for `libc::syscall` that is not a key there fails, a call whose first argument is not one of that file's own reviewed names fails, and a reviewed name the file no longer calls fails as a stale exception. The worker's file keeps its extra `denied_syscalls` rule, which is about a seccomp list the capture gate does not build. `L-I9` and `L-I10` are the observations, on both platforms. | n/a — closed. |
| S-14 | `no_float_reaches_the_gpa_path`, `tools/secret-debug-policy.test.mjs`, `phase1_exit_has_no_product_network`, the two `academic-untrusted-content` walks, and the two in `crates/consent/tests/consent_scans.rs` — the `benches` tree | **Closed by `P2-RF11`.** Seven walks excluded `benches` beside `tests`; the last two arrived with `P2-G6` while this repair was in flight and are widened here for the same reason. No `benches` tree exists in this repository, but a bench target has no feature gate and `cargo clippy --workspace --all-targets` — the README verification block's third command — compiles it, which is the two-part test `T146` applied to `examples/`. `T149` measured all three halves: a `f64`, a `#[derive(Debug)]` over `key_bytes`, and a `TcpStream::connect` in a new `crates/record/benches/` file each passed its scan, and a bench that does not compile fails the clippy lane. All seven now exclude `tests` only. | n/a — closed. `tests` stays out on the reasons those walks give for it. |
| S-15 | `the_transport_is_reached_from_no_module_but_the_proxy` — `crates/egress-boundary/tests/byte_path_pin.rs` | **Closed by `P2-RF11`.** This crate's counts read three fixed file names and its fallback inventory read six, in exactly the shape `S-5` and `S-8` record elsewhere, and no row named it. `T149` added `mod relay;` and one new file: the module reached the transport through the broker without binding a grant, wrote 178 bytes under a grant reviewed by another rulepack for a payload `transmit` refused with zero, left no journal row, and passed this crate's suite, `cargo test --workspace --all-targets` and both JS scans. The counts are now sums over a package walk, the inventory is keyed on the walk with a floor, and a module tripwire fails the day the walk is narrowed. | n/a — closed. |
| S-16 | `egress_audit.grant_id`, for rows that are not a consumed grant | The column is polymorphic and only `egress_consumption` resolves it. Deny rows and process-capability activity rows are joined to nothing that says which namespace their identifier came from, so a reader treating the column as an `egress_grant` reference finds them dangling. `P2-M1` does not need them: its reconciliation reads only consumed grants, which `T149` measured is exactly what the join resolves. | The first reader that has to attribute a *denial* or a process activity to a namespace. Closing it means a discriminator column or a second join table; severity **P3**, because no dangling row exists today: all seven `insert_audit` call sites write an identifier that is in one of the two tables. |
| S-17 | `packages/web-contracts/src/index.ts` — the four closed vocabulary sets | `masteryLevels`, `freshnessBands`, `confidentialityValues` and `retentionClassValues` restate `academic_domain`'s `MasteryLevel`, `FreshnessBand`, `Confidentiality` and `RetentionClass`, and **nothing compares the two sides**. This is the defect class `route_manifest_matches_ia_exactly` closes one step away: a list written from an authoritative enumeration with no bidirectional check. `P2-X1` found it while looking for its own kind one step out and did not fix it: the file is `P2-C7`'s contract surface, and a cross-language parity scan is its own reviewed piece of work. All four sets agree with the Rust enums today, measured at this commit, so the row is latent rather than broken. | The first commit that adds a variant to one of the four Rust enums. The TypeScript validator would then reject a fixture the Rust side accepts, and would do so silently until a fixture happened to carry the new variant — the fixture suites pin specific bytes and would not notice a set that had merely stopped being complete. Severity **P3**. Closing it means reading the four variant lists out of `crates/domain/src/lib.rs` and comparing them with the four sets in both directions, the way `model_run_requires_every_field` compares a struct against the specification's own YAML. |

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
