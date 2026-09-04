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
rather than asserted, in both directions.
`tools/policy-source-scan-inventory.test.mjs` finds every file in `crates/`,
`tools/` and `packages/` that names a Rust source path in a position where it is
read — an `include_str!`, a literal argument to a read, a `join`, an extension
filter, a `const` or table entry holding a path, or a `#[path]` include of a
module that does one of those — and fails unless the table below holds a **row**
for that file. It then reads the table back: every repository path a row names
must be a file this repository holds. What it cannot decide is whether a file
that reads source is a *policy* scan, so it does not try: a file it finds that
scans nothing is listed below anyway, in the rows saying what it does instead.
That is the intended outcome for a false positive — a row, not a hole.

A row rather than a mention, because a mention is what the check used to accept
and two scans had only that. `crates/offering/tests/offering_scans.rs` and
`tools/shared-name-isolation.test.mjs` each had a section of their own further
down this page and no line in the table, so the survey the table exists for
skipped both. The reverse pass reads the registration table only: the injection
matrices and the open ledger name files on purpose that do not exist —
`crates/admission/authority.rs` is an edit `P2-G4` considered and did not make,
and `crates/record/benches/` is a tree this page says outright is absent — and
requiring those to exist would turn a record of what was rejected into a demand
that it be built.

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

`P2-R2` reproduced the second shape inside a guard written against it.
`the_analysis_crate_touches_no_file_and_no_socket` paired a whole-set comparison
of `use` items with a list of eleven forbidden filesystem and transport
spellings, and the pair looked complete: an import appears in the first, and an
absolute-path call in the second. Three injections that spell none of the eleven
and add no `use` item each passed it — `std::path::Path::new(p).metadata()`,
which opens the filesystem; `include_str!`, which reads a file at compile time
and is a macro rather than a path; and `std::env::var`. The repair is not a
longer list: the primary nets are now two more whole sets, every two-segment
path spelled through a crate root and every macro invoked, each compared in both
directions, and the token list is kept as an explicitly weakest third layer. The
same three injections now fail.

The repair then had the same defect one layer in. The path extractor skips a
middle segment — the `b` of `a::b::c` — so that one path yields one key, and it
skipped a **leading** `::` for the same reason: `::std::path::Path::new(p)
.metadata()` opens the filesystem, spells none of the eleven, adds no `use`
item, and passed the repaired guard. A leading `::` is not a middle segment, and
what tells them apart is the byte before the `::`.

And then twice more, both about whitespace, because Rust allows it inside a path
and around a macro's `!`: `std :: path :: Path::new(p).metadata()` and
`include_str! ("x")` each compiled and each passed. The extractors now read a
view with exactly that whitespace removed. Deleting **all** whitespace was tried
first and is wrong in the one direction that matters — it joins unrelated
tokens, so `… Formatter and core::str …` becomes `…Formatterandcore::str…`,
`core` stops being a whole identifier, and the key **disappears**; a
normalisation that can hide a key is worse than the hole it closes. Tightening
the left side of a `!` then read `if !(x)` as a macro named `if`, which a
keyword filter answers: a keyword is not a name a macro may have.

Six vacuous passes in one guard, each found by injecting the shape the previous
repair did not cover. `P2-RF11`'s sentence held every time: assume there is one
more.

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
| `no_numeric_source_winner`, `credentials_never_reach_a_general_crawler`, `no_captcha_or_access_control_bypass_module_exists`, `the_only_public_route_to_snapshot_bytes_is_the_untrusted_seal`, `the_walk_reads_every_module_in_this_crate`, `this_crate_declares_three_product_edges` — `crates/ingestion/tests/ingestion_scans.rs` | recursive, **every `.rs` under the whole `crates/ingestion` package** for the per-crate rules, less `tests` for the ones about shipped code; recursive over **every `.rs` under `crates/`** for the workspace-wide inventory; fixed paths for the pins | whole-set comparisons rather than token lists: the item set of `conflict.rs` at file scope, the signature set of every function anywhere in the crate that touches a conflict value, a credential binding or a request, the public surfaces of `ConflictCase`, `ContendingSource`, `CredentialBinding`, `ConditionalRequest` and `RawSnapshot`, the whole external import set, the whole `Cargo.toml` edge set, four access vocabularies compared variant by variant, whole-text pins on `credential_binding`, `DeclaredTarget::declared` and `deny`, call-site counts for `credentialed`, `declared`, `source_bytes()` and the two `Denial` initialiser fields, and a three-shape numeric rule over `conflict.rs` — a numeric type under any spelling, a numeric literal under any spelling, and a counting/positioning vocabulary — with five evasions and four benign shapes run through it inside the test | `sources.len() >= 17` on the package walk, `declared >= 13` on the module tripwire, `naming.len() >= 5` on the workspace walk; every whole-set comparison is an `assert_eq!` against a pinned list, so an empty walk fails as missing keys |
| `tools/secret-debug-policy.test.mjs` | recursive, **every `.rs` under every `crates/*` package** less its `tests`, so `examples/`, `probes/` and any `benches/` are read | regex over derive attributes against a registry of secret-carrying types; **a whole-set classification of every named byte-buffer field in workspace product source** -- 137 of them, `(type.field, class)` compared against the source in both directions, the class drawn from a closed 13-entry vocabulary of which `key-material` and `content` forbid a derived `Debug`; a whole-set declaration of every classification key whose type name is declared in more than one crate (4 today), also in both directions, because the classification is keyed by name and a second type reusing a name would otherwise inherit a judgement nobody made; the same whole set for tuple structs and tuple enum variants; and, as a third and explicitly weakest layer, the `SECRET_FIELD_NAMES` alternation, which reaches `String` and `str` only | none on the file walk; a `>= 11` floor on the macro-generated key-type registry; the classification is its own floor, since a byte buffer missing from it fails |
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
| `the_walk_reads_every_module_in_this_crate`, `the_capture_gate_records_every_refusal_it_returns`, `the_capture_gate_re_runs_the_binding_on_every_path`, `the_capture_gate_appends_a_chunk_from_one_place`, `no_public_signature_hands_out_a_quarantined_capture`, `every_capture_medium_is_classified`, `unsafe_is_confined_to_the_device_backends`, `the_linux_backend_names_only_the_three_syscalls_it_installs`, `the_probe_opens_a_handle_and_reads_no_sample` — `crates/capture-gate/tests/capture_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source — `probes/` is product source here, because it is the one file that opens a device and it is exactly what a walk rooted at `src` would miss; plus a second recursive walk over **every `.rs` under every package in `crates/`**, less each package's `tests`, for the workspace-wide signature rule; plus fixed reads of `crates/consent/src/permission.rs` and this crate's own `Cargo.toml` | ten whole-text pins (below); a whole-set comparison of the `impl` blocks naming `QuarantinedArtifact` against a one-entry list, so a `Deref`, an `AsRef<[u8]>` or any other trait that hands the bytes back fails as an extra key; a whole-set comparison of every signature in the crate whose return type names `u8` against a two-entry list; a rule that no `pub` signature anywhere in the workspace takes a `QuarantinedArtifact` and returns a type naming `u8`, `str` or `String`; an equality between the number of `CaptureRefusal` constructions and the number of `record_refusal` calls, which is how "every refusing path appends its row" is checked rather than asserted; call-site counts by identifier on `mint_capture_capability` (1), `continue_capture` (2) and `bind_permission` (1), each with `fn <name>(` subtracted; the whole set of functions `src/session.rs` declares **at any visibility**, because a pin runs from its own signature to the end of its `impl` block and a second appender written above it is outside every pin in the file, and a function in that file can reach a session's private fields whether or not it is `pub`; `CaptureSession`'s field list, with none of them `pub`; call-site counts of one each on `ChunkRecord::build`, `CaptureArtifact::manifest_of`, `::releasable` and `::quarantined`, **counting the `Self::` spelling beside the type-qualified one** — the first version counted only the latter and `T-I5` walked past it — with the one file that may hold them named, and no `use` or `type` alias on those types; the `CaptureMedium` variant list read out of `academic-consent`'s source and compared against the four `DeviceClass::of` classifies, plus a rule that its wildcard arm is `None` and not a device; the set of files holding an `unsafe` item compared whole against a two-entry list; for the Linux backend, the same two syscall rules `only_egress_crate_has_a_socket` applies to the worker's, read against this file's own three-name list; five read shapes forbidden in the probe; and the manifest read for `default = []`, `required-features` and a `path` outside `src` | `>= 8` files in the crate walk and `>= 8` product files, plus a tripwire requiring every `mod name;` and `pub mod name;` to be a file the walk read, `#[path]` refused outright, `>= 6` declared modules, and every product file under `src/` or `probes/`; `>= 25` packages and `>= 1_200` public signatures in the workspace walk |
| `the_walk_reads_every_module_in_this_crate`, `the_only_instant_type_comes_from_one_clock`, `no_wall_clock_reaches_the_session_clock`, `a_label_has_no_path_that_moves_a_mark`, `the_journal_appends_and_never_rewrites`, `a_mapping_version_is_built_from_an_ordered_pair`, `the_thresholds_are_versioned_rows_and_not_constants`, `every_closed_vocabulary_is_the_list_its_enum_declares`, `the_default_lane_compiles_no_failpoint` — `crates/capture/tests/capture_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus a second recursive walk over **every `.rs` under every package in `crates/`**, less each package's `tests`, for the workspace-wide signature rule | fifteen whole-text pins (below); a whole-set comparison of the signatures in this crate whose return type names `SessionTick` against a nine-key set plus a ten-entry reviewed inventory, so an accessor and a producer nobody predicted both fail; **the mirror of it** — the whole set of `pub` signatures whose *parameter list* names a `SessionTick`, against a three-entry inventory naming what orders the instant each one accepts, because the clock orders what it mints and says nothing about the order a seam handed a minted tick receives them in; the `MappingVersion` struct literal at one site and `estimate_drift` at one call site with `estimate_drift` and `append_realignment` pinned whole beside them; struct-literal counts inside `impl SessionTick` and `impl SessionClock`, counted as literals rather than as spellings — a return type, an `impl` header and a type declaration all spell `Name {` and each is subtracted; the tick's field list read out of the struct, so a `pub` field that would open the type to a literal written anywhere fails; `SessionClock::start` at exactly one call site with the file named, plus a rule that the type is never aliased on a `use`; a per-seam `self.clock.tick` count whose **sum is compared with the whole recorder's**, so a ninth seam fails even if nobody adds it to a pin; the whole set of `impl` blocks naming `Mark` against a one-entry list and a rule that no `pub` signature **anywhere in the workspace** takes a `MarkLabel` and returns a `SessionTick`; the whole set of the journal's public `&mut self` methods against a one-row table with a written reason, `set_len` at one call site, `write_all` at three, and no absolute seek; the four threshold names refused outside `policy.rs` except as a row's accessor; five closed vocabularies read out of their own enums and compared with the `ALL` array, the frame bytes required distinct and non-zero; and a rule that every environment read and the abort live inside the one pinned feature-gated failpoint. **Each of those rules is run against three or four evasion samples inside the test and each sample must be caught.** | `>= 13` files in the crate walk and `>= 9` product files, `>= 40` public signatures in the crate; plus a tripwire requiring every `mod name;` and `pub mod name;` to be a file the walk read, `#[path]` refused outright, `>= 9` declared modules, and every product file under `src/`; `>= 25` packages and `>= 1_200` public signatures in the workspace walk |
| `crates/capture-gate/tests/compile_fail.rs` and `tests/compile_fail/*.rs` | n/a — `trybuild` compiles two committed programs | not a source-text scan: constructing a `CaptureSession` with a struct literal, and reading a `QuarantinedArtifact`'s bytes. Each must fail to compile *and* fail with the committed diagnostic | n/a |
| `tools/verify-contracts.mjs` | recursive, `crates/contracts/src`; the two generated modules through `tools/{engine,predicate}-registry.mjs` | digest pins and byte-for-byte re-render; refuses any tree entry that is not a `.rs` file | n/a — an unreviewed entry fails |
| `tools/engine-registry.mjs`, `tools/predicate-registry.mjs` | none — one fixed generated path each, named as `GENERATED_PATH` | not a scan: they render the generated module from `schemas/registry/`, and are the halves `verify-contracts.mjs` re-renders and compares against the committed file | n/a |
| `desktop_cannot_open_the_database_or_read_keys` — `tools/phase1-scaffold-policy.test.mjs` | recursive, **every `.rs` anywhere under `crates/desktop`**, comments and literals stripped before matching | three halves, following `only_egress_crate_has_a_socket`. Graph: the declared workspace closure of every edge kind compared whole against a four-entry list, and the resolved closure checked against ten workspace crates that own the database or a key. Link: the resolved shipping closure pinned entire, plus intersections against thirteen database-capable and seventeen key-custody crates. Source: a closed world over path roots — every identifier the crate writes a `::` after must be one of twenty-five reviewed roots, read on paths rather than on `use`, so a fully qualified `rusqlite::Connection::open` is refused; plus no foreign function, no `unsafe`, no environment read and no embedded file. The root allowlist is compared in both directions, so a dead entry fails | `>= 9` files, plus a tripwire: every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the package must be a file the walk read |
| `desktop_names_only_the_core_fixture_allowlist` — same file | none — two fixed paths, `crates/core/src/local_service.rs` and `crates/desktop/src/command.rs` | the one fixture identifier `academic-core` defines, compared against the `as_str` arms of the desktop's `SyntheticFixtureId`, sliced to that `impl` block so the capability arms are not read as fixtures. A source scan because the desktop must have no dependency edge to `academic-core`, which opens the store: the two constants can only be compared as text | none — a missing `impl` block or an unclosed one fails |
| `capability_snapshot_has_no_wildcard` — `packages/ui/src/capability-snapshot.test.ts`, over `packages/ui/src/capability-snapshot.ts` | none — four fixed paths: the two committed snapshot documents and the two vendored Tauri schemas | not a source-text scan; the text it reads is JSON. Three layers: SHA-256 whole-file pins on all four; validation of both snapshot documents against Tauri's own schemas, with negative controls including one showing the schema accepts `$HOME/**` and so is not the guard; and `scanSnapshot`, whose deciding rule is a closed world over reviewed strings, keys and values separately, with the named wildcard forms used only for the failure message | none — a missing file fails the read; the enumeration of forms is compared against its own sample table in both directions so a form that stopped matching fails |
| `route_manifest_matches_ia_exactly` — `packages/ui/src/route-manifest.test.ts`, over `packages/ui/src/ia.ts` | none — one fixed path, `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` | not a source-text scan; the text it reads is the specification. Section 25.1's drawn tree is parsed into labelled nodes with parents and compared with the route manifest as sets in both directions, plus a parent map and the reading order. A line the parser cannot account for raises rather than being skipped | none — an empty parse raises, and a second root or an unaccountable line raises |
| `the_walk_reads_every_module_in_this_package`, `the_stage_order_and_the_gate_are_pinned`, `a_secret_digest_has_exactly_two_writers_and_one_needs_a_decision`, `the_snapshot_hands_back_owned_data_and_nothing_else`, `the_credential_is_repo_scoped_read_only_and_expiring_in_source`, `the_crate_touches_the_filesystem_only_to_read_it`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory`, `every_stage_of_the_seam_is_counted`, `the_vocabularies_match_the_specification` — `crates/repository/tests/repository_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus two fixed reads, `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and this page | eight whole-text pins (below); call-site counts by identifier on `permission_and_secret_gate` (1), `inventory` (1), `freeze` (2), `index` (1), `scan_secrets` (1), `admit` (1), `run_gate` (1) and `resolve_snapshot_type` (1), each with `fn <name>(` subtracted and each with the one file it may be called from; a whole-set comparison of every method signature in `impl RepositorySnapshot` against an 18-entry list, plus no `&mut self` and no public field; whole-set comparisons of every `fs::` name the product code spells against a three-entry read-only list and of every `use` item against a 14-entry list, both in both directions; counts of the two `blob_digest` assignment sites and of the functions taking a `DisclosureDecision` by value (1); a rule that the four `SnapshotStages` methods are exactly the four counted names; and section 17.2's `sourceType` values parsed out of the specification and compared with this crate's `SnapshotType` in both directions | `>= 6` files in the package walk, `>= 4` declared modules, every product file under `src/`, and a tripwire requiring every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the package to be a file the walk read |
| `the_walk_reads_every_module_in_this_package`, `the_analysis_crate_touches_no_file_and_no_socket`, `the_ladder_and_the_path_classification_are_pinned`, `each_guarded_name_has_exactly_its_call_sites`, `no_public_accessor_hands_out_analyzed_text`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory` — `crates/repository-analysis/tests/analysis_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus one fixed read of this page | eight whole-text pins (below); call-site counts by identifier on `seal` (1), `interpret` (1), `promotes` (1) and `build` (2), each with `fn <name>(` subtracted and each with the one file it may be called from; **three whole-set comparisons of what the product code can reach, all in both directions** — every `use` item against a 20-entry list, every two-segment path spelled through a crate root against a 4-entry list, and every macro invoked against a 4-entry list; a whole-set comparison of every `pub fn` whose return type names `str`, `String` or `u8` against a 14-entry justified inventory, in both directions, with the justification drawn from a closed four-value list; and, as a third and weakest layer, a forbidden-token pass over **all** source, `tests` included, for eleven filesystem and transport spellings | `>= 7` files in the package walk, `>= 5` declared modules, every product file under `src/`, and a tripwire requiring every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the package to be a file the walk read |
| `crates/repository-analysis/tests/evidence_tiers.rs` | n/a — the one `.rs` path it names is `target/debug/build.rs` in a synthetic corpus, which the fixture uses to show that `P2-R1`'s point-1 file policy removes the whole `target` subtree before this analyzer sees it | not a source-text scan: `P2-R2`'s eight named acceptance tests plus the per-rung promotion injections and the canary run of `no_analyzed_byte_reaches_a_text_accessor`, all over in-process synthetic corpora captured through `P2-R1`'s own `capture_local` | n/a |
| `the_walk_reads_every_module_in_this_package`, `the_correlation_crate_touches_no_file_and_no_socket`, `every_field_of_this_crate_is_in_the_inventory`, `no_public_function_mutates_in_place`, `the_correlation_decisions_are_pinned`, `each_guarded_name_has_exactly_its_call_sites`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory` — `crates/repository-correlation/tests/correlation_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus one fixed read of this page | nine whole-text pins (`impl EvidenceRelation`, `admit`, `active_view`, `code_relations`, `drifts_of`, `scopes_of`, `declares_dependency`, `compare`, `impl DriftScopes`); call-site counts by identifier on `seal` (10), `authority_table` (1) and `claim_type` (1), each with `fn <name>(` subtracted and each with the one file it may be called from; **three whole-set comparisons of what the product code can reach, all in both directions** — every `use` item, every two-segment path spelled through a crate root against a 4-entry list, and every macro invoked against a 1-entry list; **a whole-set comparison of every field of every type this crate declares** against a justified inventory, in both directions, which is the answer `tools/secret-debug-policy.test.mjs` gave with a field-name alternation until `P2-RF13` made it a whole set there too; a refusal of any `pub fn` taking `&mut self`; and, as a third and weakest layer, a forbidden-token pass over **all** source, `tests` included, for eleven filesystem and transport spellings | `>= 8` files in the package walk, `>= 6` declared modules, every product file under `src/`, a tripwire requiring every `mod name;` and `pub mod name;` in the package to be a file the walk read and refusing a `#[path]` attribute outright, and a two-entry pinned set of the files permitted to read one |
| `crates/repository-correlation/tests/correlation_lanes.rs` | one fixed read of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, for section 17.5's relation bullets | not a source-text scan: `P2-R3`'s nine named acceptance tests over in-process synthetic corpora captured through `P2-R1`'s own `capture_local` and classified through `P2-R2`'s own ladder. It reads the design document so that the relation vocabulary is **compared** against section 17.5 in both directions rather than restated as a number | n/a |
| `the_walk_reads_every_module_in_this_package`, `the_classification_crate_touches_no_file_and_no_socket`, `every_field_of_this_crate_is_in_the_inventory`, `no_public_function_mutates_in_place`, `the_classification_decisions_are_pinned`, `each_guarded_name_has_exactly_its_call_sites`, `the_migration_result_is_positional_and_never_keyed_on_content`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory` — `crates/repository-classification/tests/classification_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus one fixed read of this page | ten whole-text pins (`impl ChainStep`, `impl RequiredConcept`, `impl UserEvidenceGap`, `impl ChainDraft`, `impl Outlook`, `impl OverrideDecision`, `classify`, `requirement_identity`, `migrate_locators`, `follow`); call-site counts by identifier on `seal` (3), `of_edge` (2), `materialize` (1), `settle` (3) and `contradicts` (1), each with `fn <name>(` subtracted and each with the one file it may be called from; **three whole-set comparisons of what the product code can reach, all in both directions** — every `use` item, every two-segment path spelled through a crate root against a 4-entry list, and every macro invoked against a 2-entry list; **a whole-set comparison of all 105 fields of every type this crate declares** against a justified inventory, in both directions, each entry classified into one of seven admitted things a field may hold and each checked for a `u8` in its declared type — which is the answer `tools/secret-debug-policy.test.mjs` gives with a field-name alternation, given as a whole set instead; a refusal of any `pub fn` taking `&mut self`; a refusal of any map or set type in `src/migrate.rs`, because a migration record keyed on anything but the original locator's position collapses two byte-identical originals into one; and, as a third and weakest layer, a forbidden-token pass over **all** source, `tests` included, for eleven filesystem and transport spellings | `>= 9` files in the package walk, `>= 7` declared modules, every product file under `src/`, a tripwire requiring every `mod name;` and `pub mod name;` in the package to be a file the walk read and refusing a `#[path]` attribute outright, a two-entry pinned set of the files permitted to read one, an exact `105` on the field inventory, and a minimum length on every one of the ten pins so an extractor that started returning an empty string is a failure rather than a silent pass |
| `crates/repository-classification/tests/classification_lanes.rs` | one fixed read of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, for section 18's classification names, section 19's legend glyphs and section 18.2's chain diagram | not a source-text scan: `P2-R4`'s eleven named acceptance tests over in-process synthetic corpora captured through `P2-R1`'s own `capture_local`, analyzed through `P2-R2`'s own ladder and correlated through `P2-R3`'s own `correlate`. It reads the design document so that the number of proof-chain steps is **measured** against section 18.2's diagram rather than restated as a number here | n/a |
| `crates/repository/tests/snapshot.rs` | n/a — the `.rs` paths it names are the synthetic repository fixtures its own deterministic builder writes into a `TempDir` | not a source-text scan: `P2-R1`'s eight named acceptance tests, driven over in-process fixtures, an in-memory `DeviceKeystore` and an in-memory `GitHubRepositoryReader` | n/a |
| `crates/store/src/repository_snapshot_tests.rs` | n/a — the `.rs` path it names is a manifest row in a synthetic snapshot | not a source-text scan: migration `0012`'s five guards fired against a migrated database | n/a |
| `the_forbidden_fields_are_the_specifications_own`, `no_relation_derives_another`, `nothing_infers_a_course_identity`, `the_publish_path_has_one_rewind_and_every_failure_takes_it`, `the_walk_reads_every_module_in_this_crate`, `no_file_outside_this_crate_names_a_curriculum_relation`, `the_migration_vocabularies_are_the_rust_ones`, `the_open_gates_have_no_default` — `crates/curriculum/tests/curriculum_scans.rs` | recursive, **every `.rs` anywhere under this crate's package** for the per-crate rules, less `tests` for the ones about shipped code; recursive over **every `.rs` under `crates/`**, less each package's `tests`, for the one-step-out inventory; plus two fixed reads, `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and `migrations/store/0014_phase2_curriculum_aggregates.sql` | seven whole-text pins (below); section 8.2's four yaml blocks and section 12.4's `TranscriptSegment` block parsed out of the specification and compared with the accessor mapping in order and in both directions, with the existence half read from each aggregate's own `impl` block rather than from its module; a forbidden sweep of every mapped accessor against every other aggregate's whole module; the whole `impl` set naming any of the four relation types; the whole public signature set of `relation.rs`, swept for a signature taking one relation and returning another; the whole set of signatures anywhere in the crate producing a `CourseCodeReuse`; call-site counts by identifier on `append` (1), `rewind_to` (1) and `ledger.mark()` (1), each with `fn <name>(` subtracted; the ledger vectors the appending body pushes to, enumerated from that body and each required to be one the rewind truncates; every `PublishCheckpoint` variant required to be reached through an injector call and the injector calls counted against the checkpoint names; six migration `CHECK` vocabularies compared with this crate's enums, two of them less `UNKNOWN`; and the whole set of `Default` implementations in the crate | `sources.len() >= 10` on the package walk plus a rule that the walk read this very file, which is in `tests`; `declared >= 10` on the module tripwire, with a `#[path]` target inside the package required to be a file the walk read; every product file under `src/`; `>= 60` compared pairs in the forbidden sweep; `>= 25` signatures in `relation.rs`; `>= 150` files outside the package and `>= 10` inside it on the workspace walk; every whole-set comparison is an `assert_eq!` against a pinned list, so an empty walk fails as missing keys |
| `the_rule_types_are_the_specifications_own`, `production_audit_no_llm`, `the_only_route_to_an_executable_rule_is_the_gate`, `the_open_gates_have_no_default`, `no_float_reaches_a_requirement_verdict`, `the_walk_reads_every_module_in_this_crate`, `no_file_outside_this_crate_names_a_requirement_rule` — `crates/requirement/tests/requirement_scans.rs` | recursive, **every `.rs` anywhere under this crate's package** for the per-crate rules, less `tests` for the ones about shipped code; recursive over **every `.rs` under `crates/`**, less each package's `tests`, for the one-step-out inventory; plus fixed reads of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, this crate's `src/*.rs` for the pins, and every workspace `Cargo.toml` for the dependency closure | six whole-text pins (below); section 11.2's yaml `type:` lines and its prose sentence parsed out of the specification and compared with the rule-type tables in order and in both directions, with the nine prose spellings required to be what the derivation rule produces; the transitive product dependency closure computed from the manifests and compared whole at twelve; the whole set of `String`/`&str` fields as **owning-type/field pairs**, plus a rule that none of eight audit-path types may own one — the half that cannot be satisfied by editing the table; the six identifier newtypes read from the macro's invocation list, because the names exist only after expansion; struct-literal counts of one each on `ReviewedRule` and `ExecutableRule`; the whole `impl` set naming either; the whole set of public signatures taking a `RuleCandidate` to a gated value, compared against the gate's own signature; the whole `Default` set; and a three-shape float rule (type, decimal-point literal, exponent literal scanned by index) with five evasions and four benign shapes run through it inside the test | `sources.len() >= 10` on the package walk plus a rule that the walk read this very file, which is in `tests`; `declared >= 8` on the module tripwire; every product file under `src/`; `scanned >= 8` on the capability, float and literal sweeps; `signatures >= 40` on the signature sweep; `>= 25` packages in the manifest inventory; `>= 150` files outside the package **and `>= 8` inside it** on the workspace walk, so the exclusion cannot be what makes it empty; every whole-set comparison is an `assert_eq!` against a pinned list |
| `the_center_cannot_name_a_payload_byte`, `the_class_of_an_entry_is_its_payloads_type`, `nothing_but_a_user_settles_a_conflict_or_extends_an_expiry`, `the_provider_and_receipt_vocabularies_are_the_brokers_own`, `the_six_sections_are_section_25_13s_own`, `the_walk_reads_every_module_in_this_crate` — `crates/evidence-center/tests/evidence_center_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, less `tests` for the rules about shipped code; plus fixed reads of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, of `crates/policy/src/provider.rs` for the broker vocabulary, and of every workspace `Cargo.toml` for the dependency closure | the whole set of **field positions** as `(owner, name, declared type)` triples, compared in both directions, with enum struct-variants reported as `Enum::Variant` and enum tuple positions as `Enum::Variant#n`, so a tuple position has a key too; the whole set of **declared type constructors** those fields use, compared in both directions against a reviewed allowlist from which `String`, `str`, `u8`, `Box`, `Cow` and `Untrusted` are absent — this is the half a field name cannot evade; the same extraction over every public signature, argument names excluded; a closed world over the **path roots** the crate spells a `::` after, both directions, with `P2-R2`'s leading-`::`, whitespace and middle-segment repairs each exercised against the reader inside the test; a closed world over **macros invoked**, which is empty; whole-text pins on `InboxEntry::class`, `user_receipt`, `ConflictCase::settle`, `ConflictCase::offered` and `ExpiringPermission::has_lapsed`; the whole `impl` set naming `ProposalClass` and the one naming `LivePermission`; a construction count of one on `LivePermission`; a rule that the crate declares no tuple struct; and a ten-spelling forbidden-token list kept as the **explicitly weakest** layer | `sources.len() >= 10` on the package walk plus a rule that the walk read this very file, which is in `tests`; `declared >= 8` on the module tripwire; `positions.len() >= 100` on the field reader; `signatures >= 60` on the signature reader; `scanned >= 9` on the spelling sweep; `>= 25` packages in the manifest inventory; a `broker.len() > 10_000` floor on the cross-crate read; every whole-set comparison is an `assert_eq!` against a pinned list |
| `the_shell_sections_are_the_crates_own` — `packages/ui/src/evidence-center.test.ts` | none — one fixed path, `crates/evidence-center/src/lib.rs` | the arms of `CenterSection::spec_words` are parsed out of that function's body — the match rather than the enum, so an arm renamed with a stale specification string fails on the string — and compared with `EVIDENCE_CENTER_SECTIONS` as maps in both directions, plus an equality on each section's words. It is the one place the TypeScript shell and the Rust model are held to the same six names, and there is no runtime across which they could otherwise be compared | `fromCrate.size > 0`, a raise if the function or its closing brace is not found, and a negative control: an arm the crate does not have reads as absent |
| `crates/store/src/requirement_tests.rs` | n/a — it reads migration `0015`'s own SQL text, not Rust source | not a source-text scan: migration `0015`'s guards, each of its three tables' whole column set, the rule-type `CHECK` vocabulary compared against section 11.2's fourteen in order, and the two keys that carry the review and supersession properties, driven as SQL against a migrated store with real parent rows written by the real closure writer | n/a |
| `crates/store/src/curriculum_tests.rs` | n/a — it reads migration `0014`'s own SQL text, not Rust source | not a source-text scan: migration `0014`'s guards, its four relation tables' whole column sets, its table list against the migration's own `CREATE TABLE` lines, and the SQL half of `curriculum_publish_is_atomic_under_injected_failure`, all fired against a migrated database | n/a |
| `crates/curriculum/tests/curriculum.rs` | n/a — the `#[path]` it names pulls in `P2-U6`'s fixture module, which the inventory follows rather than treating as a read | not a source-text scan: `P2-U1`'s five behavioural acceptance cases, driven over an in-process ledger and one real `academic-ingestion` run | n/a |
| `the_walk_reads_every_module_in_this_crate`, `every_raw_byte_site_is_named_and_justified`, `the_accepted_response_is_sealed_immediately`, `raw_token_write_protection`, `the_lineage_has_no_raw_mutation`, `no_default_reaches_the_remote_arm`, `the_archive_appends_and_nothing_removes`, `no_fourth_disposition_is_declared`, `the_transmission_is_decided_by_the_route`, `the_binding_is_compared_against_the_journal_header`, `two_runs_carry_no_order`, `no_wall_clock_socket_or_file_reaches_this_crate` — `crates/transcription/tests/transcription_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus a second recursive walk over **every package in `crates/`** for the four workspace-wide rules; plus five fixed reads of this crate's own modules for the whole-text pins and one of its `Cargo.toml` | ten whole-text pins (below); a whole-inventory comparison of `response_bytes`'s call sites counted by identifier with `fn response_bytes(` subtracted, each with a written reason; call-site counts by identifier on `route_for` (1), `covers` (2), `decode` (1), `record_model_run` (1) and `ModelRun::record(` (1), each with the one file it may be called from; whole-set comparisons of every `impl` header naming a raw type (6 entries) and every one naming a comparison type (4), each with a forbidden-trait list beside it as the weaker half; three pinned derive lists on the comparison types; a rule that each raw declaration carries exactly one `pub`, which is the condition the language rule about struct literals rests on; a whole-set comparison of the lineage's `&mut self` surface (3 entries); workspace-wide public-signature sweeps refusing a `ProviderResponse`/`ArchivedResponse` that returns `str`, `String` or `u8` and a raw value that is mutable, both reading whole identifiers; a workspace-wide rule that no file outside `crates/transcription/` names a raw type; a rule that the binding comparison is the **first statement** of both admitting methods; a 15-token absence list over the product source for clocks, sockets, processes and files; and a manifest rule that this crate declares no `academic-worker` and no `academic-store` edge, with comment lines stripped first | `>= 13` files in the package walk, `>= 400` in the workspace walk, `>= 10` declared modules, `>= 1000` signatures in each of the two workspace sweeps, every product file under `src/`, and a tripwire requiring every `mod name;`, `pub mod name;` and `#[path = "…"]` target in the package to be a file the walk read |
| `the_walk_reads_every_module_in_this_crate`, `the_mapped_status_has_one_producer`, `incomplete_is_the_only_value_with_no_measurement_behind_it`, `no_signature_reads_a_rendering_back_into_a_record`, `a_ranking_cannot_reach_the_preservation_path`, `the_preservation_types_offer_no_reducing_method`, `the_transform_set_is_closed_and_the_mapping_has_one_producer`, `the_document_names_no_raw_type`, `no_wall_clock_socket_or_file_reaches_this_crate` — `crates/lecture-document/tests/lecture_document_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus a second recursive walk over **every package in `crates/`** for the two workspace-wide rules; plus six fixed reads of this crate's own modules for the whole-text pins | six whole-text pins — the account, the coverage inputs, the witness, the three declaring constructors, the transform set and the whole rendering `impl`; file-to-count maps over the producers of an account (1 file, 2 sites), a witness (1, 1), a `COMPLETE` (1, 2 — one construction and one match arm, which a count cannot separate) and a source mapping (1, 1), each counted with declarations subtracted; whole-set comparisons of the public method names of `LectureDocument` (6) and `CoverageReport` (21), of every `impl` header naming a document type (5), of the files that may name `Salience` (1) and of the files that may name `PdfArtifact` (5); a workspace-wide public-signature sweep refusing a `PdfArtifact` parameter beside any of six record return types, with a control signature that the rule matches; a workspace-wide rule that no package outside this one names an account or a witness; a rule that neither preservation module names a study-index type; and a 14-spelling absence list over `src/` for clocks, sockets, files, processes and environment reads, each spelling checked against a sample that contains it | `>= 16` files in the package walk, `>= 12` in the product walk, and a `#[path]` tripwire over every product file |
| `tools/policy-source-scan-inventory.test.mjs` | recursive, `crates/`, `tools/`, `packages/` | this page names every file that reads Rust source text: six read-position markers plus one hop through a `#[path]` include, each marker checked against a sample inside the test | `>= 20` files found |
| `tools/shared-name-isolation.test.mjs` | recursive, every `.rs` under `crates/`, lexed so a string literal is told from code | five whole sets compared with a committed table in **both** directions and no forbidden-spelling list anywhere: every `env::<item>` path, every variable name a `var`/`var_os` reads with its argument resolved through crate-scoped `const`s and literal `for` arrays, every function that hands out a machine-owned root iterated to a fixed point, every shared name built on one, and every `CreateAppContainerProfile` call site with the whole text of the two guard statements its function must open with | `> 200` files scanned and `> 200` holding a string literal, so a lexer that stopped reading Rust fails rather than emptying every set below it |
| `tools/{source-preflight,cargo-lock-source-policy,dependency-source-policy,restricted-yaml}.mjs`, `tools/{dependency-source-policy,pnpm-source-policy-consumption}.test.mjs` | n/a | lockfile and registry parsing; not a source-text scan | n/a |
| `tools/{phase1-exit,security-baseline}.mjs` | n/a | execution observation and committed fixture bytes | n/a |
| `crates/store/tests/api_boundary.rs`, `crates/store/tests/sqlcipher_spike.rs` | n/a | manifest text and scratch-crate compile-fail; not a source-text scan — the `.rs` paths they name are the scratch crate's own `src/main.rs`, which they write | n/a |
| `the_walk_reads_every_module_in_this_package`, `the_state_crate_reads_no_clock_and_opens_nothing`, `only_the_named_test_files_read_anything`, `every_field_of_this_crate_is_in_the_inventory`, `no_public_function_mutates_in_place`, `the_state_decisions_are_pinned`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory` — `crates/knowledge-state/tests/knowledge_state_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus three fixed reads of this crate's own modules for the whole-text pins | **three whole-set comparisons of what the product code can reach, all in both directions** — every `use` item against a 94-entry list, every two-segment path spelled through a crate root against a 9-entry list, and every macro invoked against a 3-entry list — plus a fourth over the eight modules `lib.rs` re-exports from, because `pub use` hands a name out rather than reaching for one; **a whole-set comparison of all 134 fields of every type this crate declares** against a justified inventory, in both directions, each entry classified into one of seven admitted things a field may hold and each checked for a `u8` in its declared type, beside a separate rule requiring the crate's one tuple struct to be `AssertionId`, because a whole-set claim over *named* fields would otherwise miss an unnamed one; two whole-text pins (`EvidenceKind::ceiling`, `automatic_contribution`) and one over `AutomaticLevel`'s variant list, which is the whole of "an automatic projection cannot reach `FLUENT`"; a refusal of any `pub fn` taking `&mut self`, with a control signature the extractor does match; and, as a third and weakest layer, a forbidden-token pass over every product file for fourteen filesystem, clock, process and transport spellings — `std::time` among them, which is what makes "time never demotes mastery" a property of the whole crate rather than a rule inside one function | `>= 8` files in the package walk against `lib.rs`'s own `pub mod` list in both directions, a three-entry pinned set of the files of this package permitted to read anything, an exact `1` on the tuple-struct count, and a control on the `Fluent` absence pin — the same extractor is required to find `Fluent` once in `rung`, so the zero above is a measurement rather than an extractor that always answers zero |
| `the_walk_reads_every_module_in_this_package`, `the_competency_crate_touches_no_file_and_no_socket`, `every_field_of_this_crate_is_in_the_inventory`, `no_public_function_mutates_in_place`, `the_rubric_is_configuration_and_not_a_constant`, `each_guarded_name_has_exactly_its_call_sites`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory` — `crates/repository-competency/tests/competency_scans.rs` | recursive, **every `.rs` anywhere under the package**; the `use`/path/macro comparison reads `src` only, the forbidden-construct pass reads `tests` too | three whole-set comparisons in both directions — every `use` item, every two-segment path reached through a crate root, every macro invoked — plus an 11-token forbidden-construct pass as the third and weakest layer; a whole-set field inventory over all 100 declared fields in both directions, each carrying which of eight things it holds; a `&mut self` scan; the rubric-is-configuration scan, which reads `docs/contracts/repository-competency.md` as well as the source; and four counted call sites each pinned to its one file | `>= 9` files, `>= 6` module declarations each read by the walk, `>= 6` product files |
| `crates/repository-competency/tests/competency_lanes.rs` | one fixed read of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, for section 17.6's five bullets and section 13.2's ceiling table | not a source-text scan: `P2-R5`'s seven named acceptance tests over in-process synthetic corpora captured through `P2-R1`'s own `capture_local`, analyzed through `P2-R2`'s own ladder, correlated through `P2-R3`'s own `correlate` and classified through `P2-R4`'s own `classify`. It reads the design document so that section 17.6's bullet count and section 13.2's `mastery 승격 없음` ceiling are measured rather than restated | n/a |
| `the_walk_reads_every_module_in_this_package`, `the_gap_crate_holds_no_phrase_list`, `no_clock_socket_or_file_reaches_this_crate`, `only_the_named_test_files_read_anything`, `the_gap_decisions_are_pinned`, `no_public_function_mutates_in_place`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory` — `crates/gap/tests/gap_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus five fixed reads of this crate's own modules for the whole-text pins, one of its `lib.rs` as the control, and one of `crates/freshness/src/decay.rs` as the counter-control | **four whole-set comparisons of what the product code can reach, all in both directions** — every `use` item against a 59-entry list, every two-segment path spelled through a crate root against a 7-entry list, every macro invoked against a 2-entry list, and the ten modules `lib.rs` re-exports from against its own `pub mod` declarations; then the rule those four exist to hold, which is that **the specificity validator cannot be lexical**: `GapExplanation::defects`'s whole text is pinned and is required to name none of ten ways of reading text — `contains`, `starts_with`, `ends_with`, `find`, `split`, `chars`, `trim`, `to_lowercase`, `eq_ignore_ascii_case`, `description` — because `generic_advice_fails_validation` can only observe the three sentences it drove; five more whole-text pins on the decisions a later edit could move without any behavioural test noticing (`blocking_floor`, `RETRIEVAL_FLOOR`, `GapKind::is_strong_deficit`, the whole `route` order, and `require_band_is_not_from_the_path`) plus two substring pins inside `ConceptState::overlay` and `require_trace_declares`, which are the misattribution `P2-N2` found two layers up; a refusal of any `pub fn` taking `&mut self`; and, as a third and weakest layer, a forbidden-token pass over every product file for fifteen filesystem, clock, process and transport spellings | `>= 9` modules in the package walk against `lib.rs`'s own `pub mod` list in both directions, `>= 60` public signatures, a three-entry pinned set of the files of this package permitted to read anything, and **a two-sided control on the name reader**: the same reader is required to find at least six of eight gap names in this crate's own `lib.rs` and none of them in `P2-N3`'s `decay.rs`, so what it reports elsewhere is a measurement rather than a reader that always answers nothing; beside it every extractor is re-exercised against a sample it must match, each whole-text pin is required to extract more than 100 characters, and a signature the extractor cannot find is required to be an error |
| `crates/gap/tests/gap.rs` | one fixed read of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, for section 15.2's five-row table, its numbered steps 1, 3 and 6, and section 15.3's eight-field sentence; plus a read of this crate's own `src` for the three rules that are statements about the public surface rather than about one call | not a source-text scan: `P2-N5`'s ten named acceptance tests plus seven more, over in-process synthetic state whose lecture evidence is a node of a document `P2-L4` produced and whose bands are produced by `P2-N3`'s own `project`. It reads the design document so that five gap kinds, four state dimensions and eight explanation fields are **measured** in both directions rather than restated as numbers, and so that section 15.2 step 6's four informal names are recorded as one fewer than the table's five rows. It reads this crate's own source for three claims a call cannot make: that every producer of a `GapCase` names an `ActiveGoal`, that every entry point takes one, and that every non-ASCII string literal in the package is a cell the design document writes | `>= 18` design-document cells found by the literal reader, `>= 2` entry points, and exactly one public constructor on `ActiveGoal` |
| `the_walk_reads_every_module_in_this_crate`, `the_selector_dimensions_are_the_specifications_own`, `the_profile_fields_are_the_specifications_own`, `the_open_gates_are_section_38s_own`, `the_proof_statuses_cover_section_11_3s_own_tree`, `no_product_file_reaches_a_clock_rng_socket_or_model`, `no_product_file_names_a_projection_and_only_one_names_a_plan`, `the_three_witnesses_have_one_construction_site_each`, `the_only_defaults_are_empty_collections` — `crates/audit/tests/audit_scans.rs` | recursive, every `.rs` under `crates/audit/src` | section 11.1's selector sentence, section 3's profile block, section 38.1's ten lines and section 38.2's eleven bullets and section 11.3's tree, each parsed out of the design document and compared in both directions; a fourteen-spelling clock/RNG/socket/model sweep with its evasions run through the same predicate inside the test; a whole-set sweep for a projection name and a two-file allowance for a plan name; three witness declarations, three crate-private `establish` sites, a pinned constructor and one determination expression; and the whole `#[derive(Default)]` set against a four-entry list | a `pub mod` tripwire — every module `lib.rs` declares must be a file the walk read, and the walk must read no file no module declares — plus `declared >= 12` and `sources.len() >= 12` |
| `the_walk_reads_every_module_in_this_package`, `the_freshness_crate_cannot_name_a_mastery`, `no_clock_socket_or_file_reaches_this_crate`, `only_the_named_test_files_read_anything`, `the_freshness_decisions_are_pinned`, `no_public_function_takes_a_mastery_or_mutates_in_place`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory` — `crates/freshness/tests/freshness_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus four fixed reads of this crate's own modules for the whole-text pins, one of its `Cargo.toml`, and one of `crates/knowledge-state/src/ladder.rs` as the control | **four whole-set comparisons of what the product code can reach, all in both directions** — every `use` item against a 39-entry list, every two-segment path spelled through a crate root against a 4-entry list, every macro invoked against a 2-entry list, and the eight modules `lib.rs` re-exports from; then the rule those four exist to hold, an **eight-name refusal of every workspace spelling that means a mastery level** — `MasteryLevel`, `AutomaticLevel`, `MasteryProjection`, `MasteryFacet`, `LADDER`, `rung`, `level_token`, `automatic_contribution` — over every product file and again over every public signature, because `academic-knowledge-state` is a product edge that hands all of them out and `time_decay_touches_freshness_only` can only observe the paths it exercised; four whole-text pins (`decay`, `Spillover::toward`, the shipped `UNCALIBRATED_PRIOR_V1`, and the statement that makes a recall failure a cap — read through `collapse`, because it spans lines and a line-spanning pin is a pin on the platform's newline as much as on the rule) plus three substring guards inside `NeighborUse::direct` and four inside `require_about`, which is the misattribution `P2-N2` found one layer up; a refusal of any `pub fn` taking `&mut self`; a rule that no product file names `Default`, which is the half of `GATE-38-024` a caller cannot skip; a manifest rule with comment lines stripped first, refusing an `academic-store`, `academic-worker`, `academic-egress-boundary` or `academic-vault` edge; and, as a third and weakest layer, a forbidden-token pass over every product file for fifteen filesystem, clock, process and transport spellings — `std::time` and a bare `now` among them, which is what makes *this engine cannot ask what time it is* a property of the whole crate | `>= 8` files in the package walk against `lib.rs`'s own `pub mod` list in both directions, `>= 40` public signatures, a two-entry pinned set of the files of this package permitted to read anything, and **a control on the mastery rule**: the same reader is required to find at least five of the eight names in `P2-N2`'s own `ladder.rs`, so the zero it reports here is a measurement rather than a reader that always answers zero; beside it every extractor is re-exercised against a sample it must match, each whole-text pin is required to extract more than 100 characters, and a signature the extractor cannot find is required to be an error |
| `the_walk_reads_every_module_in_this_crate`, `the_construction_counter_reads_a_literal_and_not_a_return_type`, `an_accuracy_witness_has_one_producer`, `a_reviewed_capture_has_one_producer`, `derivative_terms_have_one_producer`, `no_original_voice_authority_is_produced_here`, `no_disclosure_reaches_a_derivative`, `no_floating_point_reaches_this_crate`, `no_wall_clock_socket_or_file_reaches_this_crate` — `crates/student-voice/tests/student_voice_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests`) and all source; plus a second recursive walk over **every package in `crates/`** for the three workspace-wide pair rules; plus four fixed reads of this crate's own modules for the whole-text pins | a whole-set comparison of the package's 23 files in both directions; four whole-text pins — the `witness` function, the `dispatch` function, `inherit_terms`, and the `RedactionScope` enum's variant list, which is the whole of "a redaction cannot reach an original"; construction counts over the whole package for an `AccuracyWitness` (1), a `DiarizationMeasurement` (1) and a `ReviewedCapture` (1), each with the `-> Name {` return-type form subtracted and the subtraction itself tested in both directions; a call count of `RetentionTerms::inherit` (1) and of `inherit_terms` (3) with `use` items dropped so a re-export is not a caller; whole-set comparisons of the public method names of `AccuracyWitness` (7), `CaptureUnderReview` (7) and `ReviewedCapture` (3); three workspace-wide public-signature pair rules — no signature takes a `CaptureUnderReview` and returns bytes, none takes a `P2-L5` value and returns an `OriginalVoiceAuthority`, and none takes a `DisclosedOriginal` or a `RestrictedOriginal` and returns a derivative type — the last with a count that the rule read at least one signature; a whole-package refusal of `f32`, `f64` and every digit-dot-digit literal, with the reader checked against a sample that has one and two that do not; and a 14-spelling absence list over `src/` for clocks, sockets, files, processes and environment reads, each spelling checked against a sample that contains it | 23 files in the package walk compared as a whole set, 10 in the product walk, a non-vacuity assertion on each workspace pair rule, and the emitter exception recorded as an assertion that `examples/emit_corpus.rs` *does* write files |
| `the_walk_reads_every_module_in_this_crate`, `the_four_standings_are_section_8_3s_own`, `the_feature_families_are_section_8_3s_own`, `the_abstention_reasons_are_section_8_3s_own`, `the_open_gate_is_section_38s_own`, `no_product_file_reaches_a_clock_rng_socket_or_model`, `no_floating_point_reaches_a_forecast`, `no_product_file_promotes_a_prediction`, `nothing_in_this_crate_has_a_default`, `this_crate_persists_nothing_and_registers_no_engine` — `crates/offering/tests/offering_scans.rs` | recursive, every `.rs` under this crate's package, split into product source and all source; plus four fixed reads of the design document for the vocabulary comparisons | four whole-set comparisons against section 8.3 and section 38 in both directions — the four standings cell for cell, the six feature families by their position in the document's own sentence, the abstention grounds as two whole sets, and the open gate's identifier derived from its bullet's position rather than typed; fourteen API spellings and six floating-point spellings over every product file with comments and literals stripped, each rule run against evasions inside the test; every `fn` signature and every `impl` header as two whole sets, so a promotion called anything at all fails; the whole `#[derive(...)]` set and every `impl Default` header | `>= 12` modules declared, `>= 12` files read by the walk, and `>= 12` impls read by the signature sweep |
| `the_walk_reads_every_module_in_this_crate`, `the_product_closure_is_exactly_the_declared_edges`, `the_product_source_reaches_only_the_declared_vocabulary`, `no_type_in_this_crate_holds_an_unclassified_byte_buffer`, `the_only_clock_read_names_a_staging_directory`, `the_portable_path_rules_match_the_phase_1_export` — `crates/export/tests/export_scans.rs` | recursive, `crates/export/src`, plus `lib.rs`'s `pub mod` lines, `crates/export/Cargo.toml`'s `[dependencies]` and `crates/portability/src/lib.rs`'s reserved-name list | the six product edges as a whole set, and by name a store, vault, crypto, keystore, recovery, retention, projection, transport, connector or model edge; four whole sets and two per-item sets over every product file, so a new path root, crate, `std` module, `std::process`/`std::os` item or macro fails; every `struct` and `enum` body against the byte classification table in both directions; a second `SystemTime::now`, or the one call leaving the staging-path reservation; and the repeated portable path rules against the ones they repeat | `>= 9` modules declared, `>= 9` files in each of the two walks, and `>= 22` reserved names read |
| `the_walk_reads_every_module_in_this_package`, `the_package_has_no_unscanned_directory`, `the_vectors_cannot_be_folded`, `the_preference_layer_cannot_reach_a_vector`, `no_clock_socket_or_file_reaches_this_crate`, `only_the_named_test_files_read_anything`, `the_critical_path_decisions_are_pinned`, `no_public_function_mutates_in_place`, `the_helpers_are_not_vacuous`, `this_scan_is_in_the_inventory` — `crates/critical-path/tests/critical_path_scans.rs` | recursive, **every `.rs` anywhere under this crate's package**, split into product source (everything outside `tests` and `examples`) and all source; plus seven fixed reads of this crate's own modules for the whole-text pins, one of its `lib.rs` as the control, one of `crates/gap/src/kind.rs` as the counter-control, and one non-recursive listing of the package root | **three whole-set comparisons of what the product code can reach, all in both directions** — every `use` item against a 75-entry list, every two-segment path spelled through a crate root against a 16-entry list, and every macro invoked against a 3-entry list, plus the thirteen modules `lib.rs` re-exports from against its own `pub mod` declarations; then the two rules those exist to hold. First, **a vector cannot be folded into a number**: twelve folding spellings — `sum`, `product`, `fold`, `reduce`, `total`, `score`, `weight`, `weighted`, `midpoint`, `average`, `mean`, `scalar` — are refused over every product file as whole identifiers, `CostVector` and `BenefitVector` are required to derive neither `PartialOrd` nor `Ord` (read out of the `#[derive(..)]` attribute immediately above each declaration), and `impl CostEstimate` is required to name none of `midpoint`, `point`, `expected`, `value`, because `cost_vector_has_seven_separate_components` can only observe the axes it moved. Second, **a preference cannot rewrite a fact**: `preference.rs` is required to name none of `CostVector`, `BenefitVector`, `CostEstimate`, `CostBasis`, eight interior-mutability spellings — `Cell`, `RefCell`, `UnsafeCell`, `Mutex`, `RwLock`, `AtomicUsize`, `AtomicU32`, `unsafe` — are refused everywhere so the shared borrow `rank` takes is a real guarantee, and the set of modules naming `Ranking` or `compare_under` is compared in both directions against a two-entry pin, so an order produced anywhere but the preference module is an extra key; then eleven whole-text pins and one substring pin on the decisions a later edit could move without any behavioural test noticing (`CheckpointDecision::for_ratio` and the threshold constant beside it, `is_stale`, `worse`, `evaluate`, `dominance`, `ParetoFront::eliminate`, `rank`'s signature, `compare_under`, `PreferenceSlider::of`, `CostEstimate::of`, `Disclosure::of`, and the whole `plan` stage order); a refusal of any `pub fn` taking `&mut self`; a package-root listing compared in both directions against the two directories the product walk excludes, so a `build.rs` or a `benches/` added later is a failure rather than unscanned source; and, as a third and weakest layer, a forbidden-token pass over every product file for fifteen filesystem, clock, process and transport spellings | `>= 13` modules in the package walk against `lib.rs`'s own `pub mod` list in both directions, `>= 90` public signatures, a three-entry pinned set of the files of this package permitted to read anything, and **a two-sided control on the name reader**: the same reader is required to find at least six of eight critical-path type names in this crate's own `lib.rs` and none of them in `P2-N5`'s `kind.rs`, so what it reports elsewhere is a measurement rather than a reader that always answers nothing; beside it every extractor is re-exercised against a sample it must match, each of the twelve folding spellings and eight mutability spellings is required to be found in a sample that does spell it, each whole-text pin is required to extract more than 100 characters, and a signature the extractor cannot find is required to be an error |
| `crates/critical-path/tests/critical_path.rs` | one fixed read of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, for section 16.2's two `NAME(P) = <` blocks and its four strategy names, section 16.3's bullet list, section 16.4's four path roles, and section 16.5's `항상 노출된다` sentence | not a source-text scan: `P2-N6`'s thirteen named acceptance tests plus five more, over in-process synthetic state whose `GapCase` is produced by driving `P2-N5`'s real `search` over a `P2-L4` document that a real `P2-L2` capture and a real `P2-L3` run produced. It reads the design document so that seven cost axes, five benefit axes, eight constraints, five disclosure groups and four path roles are **measured** in both directions rather than restated as numbers, and so that section 16.2's `같은` hedge on the four strategy names and the identity of section 16.3's eighth bullet with the checkpoint rule are each recorded with a test that fails when the document stops saying so | `>= 5` disclosure-group tokens found in section 16.5's own sentence, exactly 8 bullets in section 16.3, and the naive node-count walk required to be a strict subset of exactly one satisfying set, so the comparison against it is not vacuous |
| `crates/critical-path/tests/critical_path_harness.rs` | one fixed read of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` for section 28's engine table, one recursive listing of `testdata/critical-path/`, one non-recursive listing of `testdata/engines/`, and one read per committed corpus file | not a source-text scan: `P2-C5`'s determinism contract over this crate's own corpus. It reads section 28 because this crate's premise is that section 16 is **not** one of the twelve registered engines, so an edit that adds a critical path row to that table fails here rather than leaving the premise silently false; it lists the registry's harness root because nothing unregistered may hide there; and it byte-compares every committed corpus file against a fresh render from the single deterministic builder `examples/emit_corpus.rs` also uses | exactly 12 rows in section 28's table and 12 entries in `ENGINE_REGISTRY`, `>= 11` files rendered by the builder, 4 distinct canonical byte strings across the 4 corpus cases and `>= 3` distinct route counts among them |

## What is pinned as whole text, and what changing it costs

Each row below is compared against a constant rather than searched for tokens.
Editing one of them is intended to require editing its constant in the same
commit; that is the cost the pin buys, and it is why a pin is spent only where
a silent edit is the whole risk.

| Pinned item | Constant | Edited by |
|---|---|---|
| `ACCEPTANCE_PUBLIC_KEY` declaration | `WHOLE_ACCEPTANCE_KEY` | acceptance-key provisioning (`P2-H1`) |
| `P2-L2` `start_session` | `WHOLE_START_SESSION` | a change to how a capture session opens its clock or its journal |
| `P2-L2` `record_audio_chunk` | `WHOLE_RECORD_AUDIO_CHUNK` | a change to the audio recording seam |
| `P2-L2` `capture_image` | `WHOLE_CAPTURE_IMAGE` | a change to the image seam or to how the audio-clock offset is derived |
| `P2-L2` `mark` | `WHOLE_MARK` | a change to Mark Moment |
| `P2-L2` `label_mark` | `WHOLE_LABEL_MARK` | a change to how a label is appended |
| `P2-L2` `observe` | `WHOLE_OBSERVE` | a change to preflight signalling |
| `P2-L2` `realign` | `WHOLE_REALIGN` | a change to two-anchor realignment |
| `P2-L2` `open_gap` | `WHOLE_OPEN_GAP` | a change to how a timeline gap opens |
| `P2-L2` `ChunkJournal::append` | `WHOLE_APPEND` | a change to the journal write sequence, or to the comparison that keeps a file's frames in the order their instants run — it reads the instant and not the clock's sequence number, and it is scoped to one clock |
| `P2-L2` `estimate_drift` | `WHOLE_ESTIMATE_DRIFT` | a change to what an anchor pair has to be before it is measured — the ordering refusal is in this body, so a caller that sorts the pair first fails the `estimate_drift` call-site count instead |
| `P2-L2` `MappingLedger::append_realignment` | `WHOLE_APPEND_REALIGNMENT` | a change to the one place a `MappingVersion` is built; the struct-literal count is pinned at one beside it |
| `P2-L2` `ChunkJournal::reopen` | `WHOLE_REOPEN` | a change to what recovery truncates |
| `P2-L2` `CapturePolicyBook::published` | `WHOLE_PUBLISHED` | a change to a shipped capture threshold |
| `P2-L2` `CapturePolicyBook::effective_at` | `WHOLE_EFFECTIVE_AT` | a change to how a dated row is selected |
| `P2-L2` `fault::trip` | `WHOLE_TRIP` | a change to the `CP05` failpoint |
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
| `capture` | `WHOLE_CAPTURE` | a change to section 17.3's stage order, to the early return a blocked gate takes, or to how many times any stage runs |
| `run_gate`, `scan_secrets` | `WHOLE_RUN_GATE`, `WHOLE_SCAN_SECRETS` | a change to what the gate applies before a byte is read, or to the five detectors and the fail-closed arm above them |
| `resolve_snapshot_type` | `WHOLE_RESOLVE_SNAPSHOT_TYPE` | an input moved out of the group that reads the tree's dirtiness — which is how a dirty working tree would come to be recorded as its HEAD |
| `PathPolicy::classify` | `WHOLE_PATH_POLICY_CLASSIFY` | a change to the order the allow/deny rules, `.gitignore`, user exclusions and section 32.4's file defaults are applied in |
| `impl SecretFinding` | `WHOLE_SECRET_FINDING` | a change to what a secret finding can be asked for — the two `blob_digest` assignment counts are beside it, because a pin on a block says nothing about a second block writing the same field |
| `impl TokenPermission` | `WHOLE_TOKEN_PERMISSION` | a fourth permission, or a permission that stops being a read |
| `impl FineGrainedToken`, `impl CredentialStore` | `WHOLE_FINE_GRAINED_TOKEN`, `WHOLE_CREDENTIAL_STORE` | a change to the three-property check or its order, to what can reach the token material, or to the expiry running before the broker is asked |
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
| `open_device`, `record_chunk`, `seal`, `first_unbound_chunk` | `WHOLE_OPEN_DEVICE`, `WHOLE_RECORD_CHUNK`, `WHOLE_SEAL`, `WHOLE_FIRST_UNBOUND` | a change to what a device open compares, to whether a running capture re-runs the whole binding per chunk, to whether it compares the chunk's instant against the highest it has accepted, or to what the seal reconciles — the three consent call sites are counted beside them, and the set of functions the file declares is compared whole, because these pins run to the end of their `impl` block and a function written above `record_chunk` is outside all of them |
| `releasable_bytes` | `WHOLE_RELEASABLE_BYTES` | a change to the one place a sealed capture is asked for its bytes; the type-level half is that `QuarantinedArtifact` has no accessor at all |
| `DeviceClass::of`, `DeviceRuleset::for_token` | `WHOLE_DEVICE_CLASS_OF`, `WHOLE_FOR_TOKEN` | a change to which device a medium opens, or to the one constructor that turns a token into a ruleset |
| `CaptureAudit::record_refusal` | `WHOLE_RECORD_REFUSAL` | a change to what an audit row carries, or to whether appending one is what returns the refusal |
| the capture probe's `attempt` | `WHOLE_PROBE_ATTEMPT` | a change to what the probe does to a device — it opens a handle and drops it, and a read added here has to be added to the pin in the same commit |

Comment-only lines are dropped before a pin is compared, so a pin fixes code and
not prose. Whitespace is collapsed, so `cargo fmt` decides layout and the pin
decides content.

### What `P2-U2` pins

| Pin | What it holds |
|---|---|
| `WHOLE_ADMIT` | the whole review gate: both attestations checked against the candidate, both required to be a user, the two users required to differ, the body compiled, and only then the one `ReviewedRule` this crate builds |
| `GATE_SIGNATURE` | the gate's signature alone, as the public-signature sweep renders it, because an inventory of doors has to be able to say *which* door and a body is not what a caller sees |
| `WHOLE_USER_ID` | the reviewer check: only `Actor::User` files an attestation |
| `WHOLE_INCLUDE` | the one place an `ExecutableRule` is built, with the fixtures evaluated rather than counted |
| `WHOLE_PUBLISH` | the ledger publication: a new version number and a supersession that names the head |
| `WHOLE_IDENTIFIER_MACRO` | the identifier newtype template, because the six types it generates have no names in the source at all |

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
system refuses an open, or it does not — and five that are shapes of the source:
that a quarantined artefact has no byte accessor anywhere in the workspace, that
every refusal this crate returns appended a row, that `unsafe` is confined to
the two platform backends, that the Linux backend names only the three syscalls
it installs a ruleset with, and — `T161`'s — that a chunk reaches a manifest from
one place, which is the place that compares its instant. Those five are
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

## What `T161` added to both capture crates

`P2-L2` measured a defect in `P2-L1` and left it as `C-11`: `record_chunk`
compared a chunk's instant against the section 3.7 binding and against nothing
else, so a caller whose clock stepped back appended a chunk earlier than the one
before it and the artefact was still releasable. `T161` closed it, and closing
it turned up the same defect one crate over.

**The distinction the repair is built on.** `SessionClock::tick` refuses a
reading below one it accepted, which orders the instants a clock *mints*. A seam
that is handed an already-minted instant decides for itself what order it
accepts them in, and nothing about the first claim implies the second. Two
public seams in `academic-capture` were relying on it: `ChunkJournal::append`
took two ticks from one clock in reverse and wrote `[9000, 1000]` with the chain
still verifying, and `estimate_drift` took a reversed anchor pair and returned
the same badge and ± range off a different base offset. `C-12` on
[the capture subsystem contract](capture-subsystem.md) is where both are
recorded as closed, and the whole set of `pub` signatures that accept a
`SessionTick` is now compared against a reviewed inventory so a third seam has to
answer the question rather than inherit the guarantee.

**Sixteen injections, applied one at a time**, each reverted with its file's
SHA-256 checked back to its recorded value. **All sixteen in the table compile
clean.** `T-J3`'s first form did not — it borrowed `self.records` mutably twice
— and it was rewritten until it did, because a refusal that is a compile error
proves nothing about a guard.

| # | Injection | Compiles | Refused by |
|---|---|---|---|
| T-I1 | the ordering comparison is deleted from `record_chunk` | yes | `out_of_order_chunk_is_refused`, `capture_audit_records_every_denial`, and `WHOLE_RECORD_CHUNK` |
| T-I2 | the same comparison, made conditional on the chunk count | yes | the same three |
| T-I3 | a second appender in `session.rs`, above the pinned one in the same `impl` block | yes | the function set of that file |
| T-I4 | `accepted_at` becomes a `pub` field, so a caller lowers the mark | yes | the field set, and that none of them is `pub` |
| T-I5 | a second assembly path in `artifact.rs` spelling `Self::releasable` and `Self::manifest_of` | yes | the constructor counts — **and nothing, before the `Self::` spelling was counted beside the type-qualified one** |
| T-I6 | `open_device` starts the session's mark at zero instead of the instant it opened | yes | `out_of_order_chunk_is_refused` and `WHOLE_OPEN_DEVICE` |
| T-I7 | `ChunkRecord` is renamed on its `use`, so the counted path is spelled nowhere | yes | the alias rule, and `WHOLE_RECORD_CHUNK` |
| T-I8 | the same second assembly path, spelling the type instead of `Self` | yes | the constructor counts |
| T-J1 | the frame comparison is deleted from `ChunkJournal::append` | yes | `out_of_order_frame_is_refused` and `WHOLE_APPEND` |
| T-J2 | the same comparison, made conditional on the frame count | yes | the same two |
| T-J3 | a second `pub &mut self` append that empties `records` so the comparison sees no predecessor | yes | `JOURNAL_MUTATORS`, the whole set of the journal's mutating surface |
| T-J4 | the domain test is inverted, so every same-clock frame is excused instead of a resume's | yes | `out_of_order_frame_is_refused` and `WHOLE_APPEND` |
| T-A1 | the anchor comparison is deleted from `estimate_drift` | yes | `anchors_out_of_order_are_refused` and `a_mapping_version_is_built_from_an_ordered_pair` |
| T-A2 | the same comparison, made conditional on the anchor's sequence number | yes | the same two |
| T-A3 | a second builder that pushes a `MappingVersion` without measuring | yes | the struct-literal count |
| T-A4 | `append_realignment` sorts the pair before measuring, so the refusal never fires | yes | `WHOLE_APPEND_REALIGNMENT` |

`T-I5` is the row worth keeping. The first version of the constructor count read
`CaptureArtifact::releasable` and `CaptureArtifact::manifest_of`, and the
injection reached both from inside the type's own `impl` block, where they are
spelled `Self::`. It compiled, it assembled a releasable artefact from chunks no
session had ordered, and every rule passed. This is the counting-a-spelling
failure `P2-RF10` closed in seven guards, reappearing in a guard written after
it.

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

## What the `P2-R1` scans hold

`P2-R1`'s claim is an *order* — section 17.3 puts the permission and secret gate
above inventory and above indexing — and an order has no run-time observation
that notices the day it stops holding, because every stage still runs and every
stage still returns. A gate whose scan moved into the indexer produces the same
snapshot for every clean repository in the corpus.

**The order is held three ways, and each is blind to something the others see.**
`AdmittedPaths` and `Inventory` have crate-private constructors, so an
implementation of `SnapshotStages` written in another crate cannot return either
without calling this crate's stage that produces it — the type half. `capture`
is pinned whole and every stage's call sites are counted over the package with
the one file each may be called from — the count half. And
`secret_gate_precedes_indexer` wraps the real `LocalStages` in a spy and reads
the recorded sequence, for all eight of section 17.1's inputs — the observation
half.

**The observation the plan asked for is a count, not a sequence.** On a clean
source every stage runs once in order, and a variant that moved the scan behind
the indexer produces exactly that sequence too. What separates them is the
blocked source: the pipeline stops at the gate, so **the indexer's count is
zero**, and a variant that scanned later has already indexed by the time it
refuses. `R-I1` is that variant, and it compiles.

**An admission is bound to its request.** The type stops a stage from being
skipped; it does not stop an earlier capture's answer from standing in for this
one's. `AdmittedPaths` carries the digest of the request it was decided for and
`LocalStages::inventory` refuses one that names another, which is the shape
`bind_grant` uses one crate over.

**The two inventories that could have been token lists are whole sets.** The
crate's read-only claim is checked as *every `fs::` name the product code
spells* against a three-entry list and *every `use` item* against a 14-entry
list, both compared in both directions. A write reached without spelling `fs::`
needs an import, and an import appears in the second set; `R-I4` and `R-I5` are
the two halves, and neither spells a forbidden token because there is no list of
forbidden tokens to spell.

**The digest a secret file must not have is counted, not searched.** Two
assignment sites, one `impl` block pinned whole, and one function taking a
`DisclosureDecision` by value. `R-I6` adds a second door with a name nobody
predicted and fails on the count rather than on a spelling.

### What the specification and the plan disagree about, and which won

Section 17.1 names **eight** inputs and section 17.2's `sourceType` has **four**
values. The execution plan's `P2-R1` row names the eight and calls the result a
snapshot; it does not mention the four. The specification is authoritative, so
this crate carries both vocabularies and one total mapping between them, and
`the_vocabularies_match_the_specification` parses section 17.2's own
`sourceType` line out of the specification and compares it with `SnapshotType`
in both directions. The plan is not contradicted — it is less specific — and
`eight_source_kinds_snapshot_read_only` enumerates the plan's eight while that
test enumerates the specification's four.

### The `S-10` decision this crate had to make

`SealedCredential.blob` is a `Vec<u8>` under a field name
`tools/secret-debug-policy.test.mjs` did not hold: what an operating-system key
broker returns is half of recovering the secret it holds, and a derived `Debug`
would have printed it. That is `S-10`'s shape arriving in a new crate, and the
row says the crate that creates such a field owns the decision.

Both halves were done. The `Debug` is hand-written and prints a length, so
nothing leaks whatever the net says; and `blob` was added to `SECRET_FIELD_NAMES`
in the same commit, because **the measured cost of that one name is one site and
that site is already redacted.** No `PUBLIC_BYTES` entry was written, no other
crate's contract was touched, and the five names `S-10` still trails the code by
are not added here — their cost is on that row and it is not this one's.

`R-I10` is the observation that the widened net is not an empty guard: with the
hand-written `Debug` replaced by a derive, it names the field, the file and the
line.

Three other new field names were read and left alone. `RepositoryId.identifier`
and `CommitId.identifier` are caller-chosen metadata under a validated charset,
the reasoning `SourceId` already carries one crate over.
`DisclosureDecision.reason` is the user's own sentence about why a digest may be
stored, and it is written to be read: redacting the reason would make the record
unreadable to the audit it exists for.

### The injection matrix

Every row was applied to the working tree, run, and reverted with the file's
SHA-256 compared back to its recorded value. Each was measured on Windows
native; the WSL2 Linux lane re-ran the reverted suite rather than the
injections, because these are source-text and type rules with no platform
component.

| # | Injection | Names a forbidden token? | Observed |
|---|---|---|---|
| `R-I1` | `run_gate` stops calling `scan_secrets`, as a gate whose scan moved behind the indexer would | no — it removes a call | `secret_gate_precedes_indexer` fails: the indexer's count on a blocked source is 1 and not 0. `secret_hash_disclosure_requires_a_recorded_decision` fails too, and `the_stage_order_and_the_gate_are_pinned` fails on the `scan_secrets` count |
| `R-I2` | a second `admit_all` beside the gate, building an `AdmittedPaths` with no scan | no — it spells only names the crate already uses | `the_stage_order_and_the_gate_are_pinned` fails: `admit` has two call sites |
| `R-I3` | `resolve_snapshot_type` moves `Commit` out of the group that reads the tree's dirtiness | no | `dirty_worktree_is_not_head` fails on the eight-input enumeration, and the whole-text pin fails |
| `R-I4` | `SourceTree::read` writes a marker file through `fs::write` | no | `the_crate_touches_the_filesystem_only_to_read_it` fails: `write` is an extra key in the `fs::` set |
| `R-I5` | `use std::fs::OpenOptions` and `use std::io::Write as _`, spelling no `fs::` call | no | the same test fails on the `use` inventory instead |
| `R-I6` | `SecretFinding::with_digest`, a second way to attach a digest with no decision | no | `a_secret_digest_has_exactly_two_writers_and_one_needs_a_decision` fails on the assignment count |
| `R-I7a` | a `ContentsWrite` variant with no arm anywhere | n/a | **does not compile** — recorded because a refusal that is a compile error proves nothing about a scan, which is why `R-I7` exists |
| `R-I7` | the same variant with every arm it needs, mapping to `Access::Read` | no | `the_credential_is_repo_scoped_read_only_and_expiring_in_source` fails on the pinned `impl`, and `github_token_is_repo_scoped_read_only_and_expiring` fails on the `:read` suffix rule |
| `R-I8` | `TokenScope::covers` compares owners instead of repositories | no | `github_token_is_repo_scoped_read_only_and_expiring` fails: the token covers a second repository |
| `R-I9` | the walk narrowed to `<crate>/src`, which is `S-12`'s shape | n/a — it is the scan's own walk | `the_walk_reads_every_module_in_this_package` fails on the floor and the product-source rule |
| `R-I10` | `SealedCredential` derives `Debug` instead of hand-writing it | no | `tools/secret-debug-policy.test.mjs` names the field, the file and the line |

`R-I2`, `R-I4`, `R-I5` and `R-I6` are the four that spell nothing any list
holds. Each is caught by a whole-set or a count rather than by a search, which
is the property this page asks a new guard to have.

## What the `P2-U6` scans hold

Three of `P2-U6`'s named acceptance cases are statements about what the source
does not contain, and a behavioural test cannot observe an absence.

**`no_numeric_source_winner`** is five halves. The item set of `conflict.rs` at
file scope is pinned whole, so a function added there fails whatever it is
called. The module is then required to reach no number under any spelling: a
numeric type, a numeric literal, or an operation that turns a collection into a
position, a count or an order. The first two rules have no vocabulary and are
the same shape as `no_float_reaches_the_gpa_path`'s; the third is a vocabulary
and is the weakest of the three, which is why it does not stand alone. Third,
the whole set of signatures *anywhere in the crate* that touches a conflict
value is pinned, so a winner written in another module fails as an extra key.
Fourth, `ConflictCase`'s and `ContendingSource`'s public surfaces are pinned,
because a method spelling `Self` is invisible to a signature sweep. Fifth,
`SourceCategory` — section 8.4's numbered list of collection targets — has one
`impl` block, a pinned derive list with no ordering in it, and a compile-fail
case observing that two values cannot be compared.

Dates are compared, because two of the five dimensions *are* dates, and each
comparison is a named relation computed by the module that owns the value. What
the rule refuses is the step after that: a number saying how many dimensions
favoured a side, a rank read out of a list, or a source picked because it came
first.

**`credentials_never_reach_a_general_crawler`** is five halves. The whole set of
signatures that take or return a `CredentialBinding` is pinned, and so is that
type's own surface. The producer is pinned as whole text, so a binding minted
for an authentication method that holds no credential is an edit to a constant.
The one constructor that consumes a binding is counted, and the binding's whole
`impl` set is compared with the rule that its declaration carries no attribute
at all — a derived `Clone` is how one binding becomes two requests, and it spells
nothing on any token list. The target constructor
is pinned as whole text and takes `&'static str`, so a link read out of a
fetched page is a value no target can be built from, and the identifier is
counted so a second construction site fails. And the crate is required to
implement `ConditionalFetch` nowhere: it holds no transport, which is what makes
"not a crawler" a fact about the package rather than about its intentions.

**`no_captcha_or_access_control_bypass_module_exists`** is six halves. The whole
set of signatures that produce or consume a request is pinned and each is
required to name no response type, because a bypass of an access control is a
function from a challenge to an answer. Four access vocabularies —
`AuthenticationMethod`, `TermsStatus`, `Fallback`, `DenialRoute` — are compared
variant by variant, so a variant meaning "obtained some other way" fails as an
extra key. `deny` is pinned whole and the two fields no other expression sets
are counted, so a denial that routes to a retry or offers a shorter list fails.
`Denial`'s fields are private to `terms.rs`, so a second construction site
outside that module is a compile error rather than a scan finding. The whole
external import set is pinned, so a decoder, a driver or an HTTP client cannot
be reached without a line that is not on the list. And a workspace-wide walk
requires that no file outside `crates/ingestion/` names this crate's request,
target or credential types.

What that composite does **not** say is that no bypass can be written. It says
three things together: a module elsewhere cannot transmit, because
`only_egress_crate_has_a_socket` refuses a socket outside the two egress crates;
it cannot obtain a target or a credential, because the producers are here and
pinned; and it cannot be built on these types unnoticed, because the inventory
is a whole map. `docs/contracts/official-source-ingestion.md` states that width
rather than the wider sentence.

### The injection matrix

Thirty-three injections, applied one at a time to shipped source and reverted.
Each is compiled first: a refusal that is a compile error proves nothing about
the scan that was supposed to refuse it, and one row below is recorded as
exactly that outcome rather than dropped. The unmodified tree is run before and
after the matrix and passes both times.

| # | Injection | Refused by |
|---|---|---|
| U-N1 | `ConflictCase` gains `winner(&self) -> Side`, reading the hierarchy relation and spelling no number | `no_numeric_source_winner`, the `ConflictCase` surface pin |
| U-N2 | `SourceCategory` gains a hand-written `PartialOrd` that orders by its stable spelling | the `SourceCategory` `impl`-set pin |
| U-N3 | `ConflictCase::open` counts its findings with `fold`, which is on no vocabulary list | the numeric-literal rule |
| U-N4 | `conflict.rs` gains a private `fn weight(..) -> u8` | the numeric-type and numeric-literal rules, and the item-set pin |
| U-N5 | `publish.rs` gains `preferred(&ConflictCase) -> &ContendingSource` | the crate-wide conflict-signature sweep |
| U-C1 | `manifest.rs` gains `lend(from, to) -> Option<CredentialBinding>`, minting one connector's binding for another | the credential-signature sweep |
| U-C2 | `credential_binding` mints for every authentication method | the whole-text pin |
| U-C3 | `DeclaredTarget` gains `discovered(String)`, leaking the allocation for a `'static` borrow | the `declared` call-site count, and the `leak` count |
| U-C4 | the crate implements `ConditionalFetch` for a type of its own | the "implements the transport it exists not to have" rule |
| U-C5 | `CredentialBinding` gains a derived `Clone`, so one binding can be spent on two requests | the `impl`-set comparison and the no-attribute rule on its declaration |
| U-B1 | `fetch.rs` gains `answer(&FetchOutcome, ..) -> Result<ConditionalRequest, Denial>` | the request-signature pin, and the response-derivation rule beside it |
| U-B2 | `AuthenticationMethod` gains `NegotiatedSession` | the access-vocabulary comparison |
| U-B3 | `DenialRoute` gains `TryAnotherPath` | the same |
| U-B4 | `document.rs` gains `use std::process::Command;` | the external-import set |
| U-B5 | `terms.rs` gains a second `Denial` initialiser offering one fallback four times | the `route:` initialiser count |
| U-B6 | `crates/connector/src/main.rs` declares a type named `ConditionalRequest` | the workspace-wide inventory |
| U-B7 | the crate gains an `examples/` tree pulled in by `#[path]` | `the_walk_reads_every_module_in_this_crate`, the product-source-outside-`src` rule |
| U-S1 | stage nine stops re-reading the terms ledger | `ingestion_stage_order_is_strict`, the stage-nine case |
| U-S2 | stage six discards the schema error | the same, the stage-six case |
| U-S3 | the snapshot stops comparing the observed digest | the same, the stage-three case |
| U-S4 | stage one stops comparing the declared cadence against the clock | `the_declared_cadence_limits_a_fetch_and_not_an_import` |
| U-U1 | `Reconciled::publishable` fabricates an effective date for the undated arm | `unscoped_official_source_cannot_publish` |
| U-V1 | invalidation reaches every node with an edge, ignoring which rule changed | `source_change_invalidates_exact_dependents`, over-invalidation |
| U-V2 | invalidation stops walking transitively | the same, under-invalidation |
| U-R1 | the diff reports only rule-level changes, so a header change moves nothing | `rule_change_impact_identifies_exact_rules`, under-reporting |
| U-R2 | every rule present in both readings is reported as text-changed | the same, over-reporting |
| U-M1 | `build` defaults `completeness` instead of refusing | `connector_manifest_requires_every_field`, that field's case |
| U-F1 | `Fallback::ALL` repeats one entry instead of listing four | `manual_and_export_fallbacks_are_offered_when_denied` |
| U-D1 | `ConflictDimension::ALL` drops the transitional-measures dimension | `conflict_case_dimensions` |
| U-P1 | the snapshot stores empty HTTP metadata | `rule_source_snapshot_metadata` |
| U-G1 | `RawSnapshot` gains `pub fn retained(&self) -> &[u8]` | `the_only_public_route_to_snapshot_bytes_is_the_untrusted_seal` |
| U-G2 | `ParsedRule` gains `pub fn text(&self) -> String` | `no_document_text_leaves_the_parser`, the surface pin |
| U-G3 | `publish.rs` gains `publish_anyway(document, connector, effective, retrieved_at) -> PublishedRules` | `the_publisher_has_one_argument_type_and_one_producer`, the construction count — **which it did not, before this round** |

One injection is recorded as proving nothing about a scan, because the record is
more useful than the omission. Adding a `precedence: u8` field to
`ContendingSource` does not compile: the field is required by the one
initialiser and the initialiser does not set it. That is the type refusing it,
not the scan, so the injection was rewritten as `U-N4` — a private function
carrying the same number — which does compile and is refused by the numeric
rules.

### The empty guard this task found in its own suite

`U-G3` passed the first time it was run, and the guard it was aimed at was one
of this task's own.

`the_publisher_has_one_argument_type_and_one_producer` pinned the whole set of
*signatures* naming `PublishableRules`. A second public entry point into
publication does not have to name it: `publish_anyway(document, connector,
effective, retrieved_at) -> PublishedRules` builds the argument **in its body**,
names the type nowhere in its signature, and passed. That is
[the pin fixes the item and not its caller](#two-more-the-t141-audit-found-in-the-repair-for-the-first-three)
one layer in — a sweep over signatures is a claim about what functions *declare*,
and what makes publication reachable is a *construction*.

The repair is the construction count beside the sweep: `PublishableRules::new(`
appears exactly once and in `stage.rs`, the named struct literal appears nowhere,
and the one assembly — inside the constructor — is pinned as whole text. Anything
routed through `Reconciled::publishable` inherits that function's `None` for the
undated arm; anything not routed through it has to build the value, and building
it is what is counted.

The same question was then asked of every other signature sweep in this file.
`CONFLICT_SIGNATURES` and `CREDENTIAL_SIGNATURES` are each backed by a surface
pin on the type itself, which is what sees a constructor spelling `Self`;
`REQUEST_SIGNATURES` is backed by `REQUEST_SURFACE` for the same reason; and
`SNAPSHOT_PUBLIC_METHODS` is a surface pin rather than a sweep to begin with. The
publisher was the one that had a sweep and no construction count.

## What the `P2-U1` scans hold

Four of `P2-U1`'s claims are statements about what the source does not contain,
and a behavioural test cannot observe an absence.

**`the_forbidden_fields_are_the_specifications_own`** is five halves. Section
8.2's four yaml blocks are parsed out of the specification and compared with the
accessor mapping *in order and in both directions*, so a key the specification
writes and the mapping does not fails as a missing entry and the reverse fails
as an extra one. Every mapped accessor is then required to exist in its own
aggregate's own `impl` block — not merely somewhere in its module, which is the
distinction injection `U-I5` made load-bearing: renaming
`CourseRevision::source_snapshot` left `CourseRevisionDraft::source_snapshot`
spelling the same name one type over, and a per-file name set passed. Third, the
forbidden sweep runs every mapped accessor against every other aggregate's whole
module, with five shared names each carrying a written reason. Fourth, section
12.4's `TranscriptSegment` block supplies the offering's excluded list, because
that is where the specification writes down what 매 수업시간의 실제 발화 is; a
vocabulary invented in the test would have been a token list. Fifth, section 9's
three boundary rows are pinned whole and each module is required to quote its
own exclusion cell, so a module cannot state a narrower boundary than the
specification does.

**`no_relation_derives_another`** is five halves. Section 11.4's sentence is
pinned whole and walked forwards word by word, so a relation dropped from
`CourseRelationKind` fails against the specification rather than against a
number. The whole `impl` set naming any of the four relation types is compared
with a four-entry list, so a `From`, a `TryFrom`, a `Deref` or an `AsRef`
appears as an extra key rather than being searched for by spelling. Both that
sweep and the signature sweep read **every product file in the crate**, not
`relation.rs`: the type and the trait are both local, so the orphan rule refuses
a conversion written outside the crate and refuses nothing written in a sibling
module — `U-I24` and `U-I25` put one in `publish.rs`, and both were written
after this task noticed its own sweeps were file-scoped. The signature sweep is
what catches a conversion whose name spells no trait — injection `U-I7`'s
`implied_identity`. Each of the four
lookups is pinned as whole text and each is required to read exactly one of the
four vectors, counted over the pinned text. And 경과조치 is required to be
answered in `version.rs` and nowhere in `relation.rs`, because it names a cohort
and a curriculum version and has no course end.

**`nothing_infers_a_course_identity`** is three halves. The whole set of
signatures anywhere in the crate that produce a `CourseCodeReuse` is pinned at
three, so a heuristic added under any name fails as an extra key —
injection `U-I13` is a `same_course_by_code` that reads two catalogue codes. The
`Unknown` reading is counted at its two sites, the `map_or` fallback and the
constructor's refusal, and both are pinned. And no signature anywhere in the
crate may take two `CourseCode` values, because comparing two codes is the
strongest available inference and section 8.2's contract is that course-code
reuse is an explicit decision.

**`the_publish_path_has_one_rewind_and_every_failure_takes_it`** is four halves.
`publish`, `rewind_to` and `truncate_to` are pinned whole. `append`, `rewind_to`
and `ledger.mark()` are each counted to one site, with `fn <name>(` subtracted,
so a second public entry point that writes without taking a mark fails —
injection `U-I16`. The ledger vectors the appending body pushes to are
enumerated *out of that body* rather than written down, and each is required to
be one the rewind truncates, so a fifth vector fails until the rewind reaches it.
And every `PublishCheckpoint` variant is required to be reached through an
injector call, with the injector calls counted against the checkpoint names, so a
checkpoint named in a condition written inline fails.

**The one-step-out inventory.** Every rule above is about
`crates/curriculum`. `no_file_outside_this_crate_names_a_curriculum_relation`
walks every product file in every other workspace package and requires none of
them to name any of the four relation types, the identity verdict, the
publisher, the ledger or the transition arrangement. It is empty today and a
file added to it is a review rather than a silent second implementation.

### The injection matrix

Twenty-six injections, applied one at a time to shipped source and reverted.
Each is compiled first, with `cargo build` rather than `cargo test`, so a
trybuild case's *expected* diagnostic cannot be mistaken for a real build
failure: **all twenty-four compile.** The unmodified tree is run before and after
the matrix and passes both times.

Four rows below are refusals this task found in its own guards rather than
confirmations of them. `U-I5` passed unrefused on the first run and the
existence check was moved from the module to the aggregate's own `impl` block.
`U-I20` was originally a `#[path]` naming a file that does not exist, which is a
compile error and therefore no evidence at all; it was replaced by the walk
narrowing it was supposed to stand for, and the walk grew the rule that it must
have read its own file — which is in `tests` — before the replacement was
refused. `U-I24` and `U-I25` are the same question asked one module out: the
relation sweeps read `relation.rs` only, and a conversion in `publish.rs`
compiles exactly as well, so both sweeps were widened to every product file in
the crate before either injection was written.

| # | Injection | Refused by |
|---|---|---|
| U-I1 | `Course` grows an `instructors` accessor with no field behind it | `aggregate_boundaries_are_compile_errors` |
| U-I2 | `CourseRevision` grows a `term` accessor | `aggregate_boundaries_are_compile_errors` |
| U-I3 | `CourseOffering` grows a `verbatim_text` accessor — section 12.4's own key, on no list written in the test | `the_forbidden_fields_are_the_specifications_own` |
| U-I4 | one key is dropped from the accessor mapping, together with its declared length | `the_forbidden_fields_are_the_specifications_own` |
| U-I5 | `CourseRevision::source_snapshot` is renamed, leaving the draft's setter spelling it | `the_forbidden_fields_are_the_specifications_own` — **after** the check moved to the aggregate's own `impl` block |
| U-I6 | `impl From<ReplacementRelation> for EquivalenceRelation` | `aggregate_boundaries_are_compile_errors` |
| U-I7 | `ReplacementRelation::implied_identity` returning an `IdentityDecision`, spelling no conversion trait | `aggregate_boundaries_are_compile_errors` |
| U-I8 | `same_course` also reads the replacement set | `replacement_does_not_imply_identity` |
| U-I9 | `equivalent` answers both directions | `equivalence_is_directional_and_effective_dated` |
| U-I10 | `RetirementRelation` grows a `replacement` accessor | `aggregate_boundaries_are_compile_errors` |
| U-I11 | migration `0014` gives `course_retirement` a `replacement_course_id` column | `no_relation_table_carries_another_relations_column` |
| U-I12 | migration `0014` admits `'UNKNOWN'` as a recorded identity verdict | `no_relation_table_carries_another_relations_column` |
| U-I13 | the ledger grows a second identity producer that compares catalogue codes | `nothing_infers_a_course_identity` |
| U-I14 | `IdentityDecision::record` stops refusing `UNKNOWN` | `replacement_does_not_imply_identity` |
| U-I15 | the rewind stops truncating one vector the publication appends to | `curriculum_publish_is_atomic_under_injected_failure` |
| U-I16 | a second public entry point calls the appending body without taking a mark | `the_publish_path_has_one_rewind_and_every_failure_takes_it` |
| U-I17 | `publish` rewinds only for an injected fault, not for every failure | `the_publish_path_has_one_rewind_and_every_failure_takes_it` |
| U-I18 | one checkpoint is never consulted in the appending body | `curriculum_publish_is_atomic_under_injected_failure` |
| U-I19 | a product file outside `src`, carrying a forbidden accessor | `the_walk_reads_every_module_in_this_crate`, `the_forbidden_fields_are_the_specifications_own` |
| U-I20 | the walk is narrowed from the package to `src` | `the_walk_reads_every_module_in_this_crate` — **after** the rule that the walk read its own file |
| U-I20b | a `#[path]` module inside the package that the narrowed walk would miss | `the_walk_reads_every_module_in_this_crate` (tripwire) |
| U-I21 | another crate's product file declares a type named `IdentityDecision` | `no_file_outside_this_crate_names_a_curriculum_relation` |
| U-I22 | `relation.rs` answers the transitional measure | `no_relation_derives_another` |
| U-I23 | `CurriculumCategory` gains a `Default` | `the_open_gates_have_no_default` |
| U-I24 | `impl From<RetirementRelation> for ReplacementRelation` written in `publish.rs` rather than `relation.rs` | `no_relation_derives_another` — **after** the `impl` sweep widened to every product file |
| U-I25 | a cross-returning `widen(ReplacementRelation) -> EquivalenceRelation` written in `publish.rs` | `no_relation_derives_another` — **after** the signature sweep widened to every product file |

`U-I1`, `U-I2`, `U-I6`, `U-I7`, `U-I10` and `U-I24` are recorded against the
compile-fail suite when the whole crate is tested, because that target fails
first and Cargo stops there. Each also violates the source scan named beside it:
`U-I24` was re-run against `--test curriculum_scans` alone and
`no_relation_derives_another` refused it there.
## What the `P2-L3` scans hold

Three of `P2-L3`'s named acceptance cases are statements about what the source
does not contain: nothing writes a raw token, the raw provider response leaves
the crate only under `P2-G5`'s label, and no configuration reaches the remote
arm. A behavioural test cannot observe an absence.

**The raw-token rules are four things, and the first is the compiler's.** Every
field of `RawToken`, `RawSegment` and `RawTranscript` is private, so a struct
literal for any of them is an error outside `transcript.rs` — three
`compile_fail` cases observe that with their diagnostics committed. What the
scan adds is the condition that rests on (each declaration carries exactly one
`pub`), the three assemblies pinned as whole text, a whole-set comparison of
every `impl` header naming a raw type, a workspace-wide signature sweep, and a
workspace-wide rule that no file outside this package names a raw type at all.
That last one is `P2-U6`'s `credentials_never_reach_a_general_crawler` shape and
it is a tripwire for `P2-L4`, the first task that will.

**The raw-byte inventory is `Untrusted::expose`'s, copied rather than
reinvented.** `ProviderResponse::response_bytes` is `pub(crate)` and its two call
sites are compared as a whole inventory, counted by **identifier** with `fn
response_bytes(` subtracted — both halves of the repair `P2-RF10` and `P2-RF11`
made to that inventory, because the repaired helpers were copied into a new file
and a copy inherits nothing automatically. `L3-I2` and `L3-I4` are the two shapes
injected here rather than assumed.

**The pins fix their callers.** `T141` left a pinned check byte-identical and
wrapped the *call* to it in a marker-file condition, so each pin has a count
beside it: `route_for` at one call site, `covers` at two, `decode` at one,
`record_model_run` at one, `ModelRun::record(` at one. `covers` gets a second
rule of its own — it must be the **first statement** of both admitting methods —
because a count says nothing about ordering, and `L3-I18` moves it below the
frame lookup while keeping the count at two.

**A binding read out of the thing it validates is not a binding.** The first
version of `AuthorizationBinding::of` took the capability token out of the very
`JournalRecovery` it was about to admit inputs from, so `covers` could only catch
a caller *mixing* two journals — and `academic_capture::ChunkJournal::replay` is
public and takes bytes, so a synthesized recovery naming any token agreed with
itself. It now takes a `CaptureRecorder`, which has no public constructor.
Three rules hold that: the whole text of `impl AuthorizationBinding`, a one-entry
comparison of the functions that produce a binding from a journal, and a count of
the construction — the last because `U-G3` records that a signature sweep says
nothing about a body that builds its own argument. `L3-I25` is the observation.

**The `use`-item filter reads whole items.** The first version of the decoder's
call count dropped only lines beginning `use ` and read three callers where there
is one: a `use crate::{ … }` block spans several lines and a `pub use`
re-export begins with neither. It now drops whole items, which is the same class
of defect as reading a name as a spelling — a filter that matches the first line
of a construct rather than the construct.

### What this task found in a neighbouring guard

`academic-untrusted-content`'s `ACCEPTED_RESPONSE_FILES` is a four-entry
whole-set inventory of the files that may hold an unlabelled provider response.
This task is the first to add a fifth, and adding one is what that inventory is
for. What it does **not** carry is what the new file does with the value, so the
scoped half is written here:
`the_accepted_response_is_sealed_immediately` holds that
`ProviderResponse::from_remote` is the only function in this crate taking an
`AcceptedResponse`, and that no product file here names `EgressProxy` — so a
second unlabelled response cannot be produced locally either. Without the second
rule the first is a claim about signatures, which is exactly the hole `U-G3`
recorded one round earlier.

### One stage has no failure, and that is recorded rather than papered over

`lecture_pipeline_dag` drives every stage but one. `FanOutDownstreamJobs` derives
three handles from values every earlier stage has already validated and there is
no input that makes it fail; an invented failure would be a case that tests the
invention. `INFALLIBLE_STAGES` names it with a written reason and is compared
against `Stage::ALL`, so a stage added without an arranged failure has to be
classified, and the positive control asserts the run reached it and then asserts
every property of what it produced.

### The injection matrix

Twenty-five injections, applied one at a time to shipped source and reverted
with each file's SHA-256 checked back to its recorded value. Each is compiled
first: a refusal that is a compile error proves nothing about the scan that was
supposed to refuse it, and two rows below are recorded as exactly that outcome
rather than dropped — each with the variant that does compile beside it. The
unmodified tree is run before and after the matrix and passes both times. The
numbering skips `L3-I14` and `L3-I20`: both were drafted and both turned out to
be the same observation as a row that is here — a fourth disposition token is
what `L3-I13` already moves, and a narrowed walk is what `L3-I7`'s tripwire
already fires on. They are left out rather than padded in.

| # | Injection | Refused by |
|---|---|---|
| L3-I1 | a third `response_bytes` call in an existing file | the raw-byte inventory |
| L3-I2 | a fourth site written `ProviderResponse::response_bytes(r)`, spelling no `.response_bytes()` | the same, counted by identifier |
| L3-I3 | `pub fn rendered(&ProviderResponse) -> &[u8]`, naming nothing on any list | the same, and the workspace public-signature rule |
| L3-I4 | `fn response_bytes_rendered(` — a declaration read as a spelling would cancel its own call | the same; `declarations_of` requires `(` or `<` after the exact name |
| L3-I5 | a `tokens_mut` accessor in a new module | **the compiler**: `tokens` is private to `transcript.rs`, so the scan never runs |
| L3-I5b | the same accessor written inside the declaring module, which compiles | `raw_token_write_protection`'s workspace signature sweep |
| L3-I6 | a local trait `Rewrite` with an `overwrite` method, spelling none of the six forbidden trait names, in a new module | **the compiler**, for the same reason |
| L3-I6b | the same trait written inside the declaring module, which compiles | the whole `impl`-set comparison |
| L3-I7 | product code outside `src`, reached by `#[path = "../probes/leak.rs"]`, with a raw-byte site in it | the `#[path]` tripwire, the product-source-under-`src` rule, and the raw-byte inventory |
| L3-I8 | another package naming `RawToken` | the workspace-scope rule |
| L3-I9 | `route_for` loses the external-processing half | `WHOLE_POLICY`, and `stt_provider_policy` |
| L3-I10 | a second caller of `route_for` in a new module | the call-site count |
| L3-I11 | a second `SttPolicy` constructor beside `new` | `WHOLE_POLICY` |
| L3-I12 | the archive gains a `forget` that retains around one entry | `WHOLE_ARCHIVE`, the one-`&mut self` rule, and the forbidden-call rule |
| L3-I13 | `LineageEffect::of` maps `Reject` to `AppendsVersion` | `WHOLE_LINEAGE_EFFECT`, and `user_correction_lineage` |
| L3-I15 | `impl PartialOrd for ProviderRun` in a new module | the comparison `impl`-set |
| L3-I16 | a local trait `Ahead` ranking two runs, spelling none of the three forbidden trait names | the same |
| L3-I17 | a local run records the caller's transmission | `WHOLE_RECORD_MODEL_RUN`, and `a_remote_response_comes_through_the_egress_boundary` |
| L3-I18 | the binding comparison moves below the frame lookup, keeping the call-site count at two | the first-statement rule |
| L3-I19 | the binding compares only the capability token and not the policy row | `WHOLE_BINDING` |
| L3-I21 | `use std::time::SystemTime` in product source | the absence list |
| L3-I22 | a second function taking an `&AcceptedResponse` | `the_accepted_response_is_sealed_immediately` |
| L3-I23 | a second caller of the decoder | the `decode` call-site count |
| L3-I24 | `parse_token` stops refusing a response that contradicts its contract's timestamp declaration | `WHOLE_PARSE_TOKEN`, and `a_malformed_provider_response_is_refused` |
| L3-I25 | `AuthorizationBinding::of` goes back to reading the token out of the journal it is about to admit from | `WHOLE_BINDING`, the producer comparison, and `pipeline_input_authorization`'s forged-journal row |

`L3-I5` and `L3-I6` are the two rows that are refused by the compiler rather than
by a scan. They are kept because what they measure is real — the private-field
rule is the first of the four things holding the raw layer — and each is paired
with the variant written where the field is reachable, which compiles and is
refused by the scan the pair was aimed at.

## What the `P2-L4` scans hold

Four of `P2-L4`'s claims are statements about what the source does not contain:
no path reduces a document because a span was ranked low, the rendering is a
sink, `MAPPED` has one producer, and the completeness witness has one producer.

**Every rule is a whole set, and that is a response to a measurement rather
than a preference.** `P2-R2` recorded five guards in this run failing in a row
because each asked whether a name was on a list of forbidden spellings, and a
bypass that spells nothing on the list walks past all five. So: the producers of
an account, of a witness, of a `COMPLETE` and of a source mapping are compared
as file-to-count maps; the fields a coverage run reads and the public method
names of the two preservation types are compared as complete sets, so a
parameter or a method added under any name at all fails as an extra key; and the
rendering rule is over a **pair of types** — a public signature anywhere in
`crates/` that takes a `PdfArtifact` and returns a record type — rather than
over function names nobody may write.

**Two counting shapes had to be repaired while writing them.** A bare `Name {`
count reads a struct declaration and two `impl` headers as constructions, which
is `declarations_of`'s relationship to `uses_of` one level up; `constructions_of`
subtracts them. And a fieldless enum variant's construction and its match arm
are the same three tokens, so the `DocumentCompleteness::Complete` count in
`pdf.rs` is two and cannot be one — the whole-`impl` pin is what fixes which of
the two is the upgrade, and the map's job is that no other file spells it at all.
That is recorded rather than filed as a passing count.

**`P2-L3`'s tripwire did not fire, and this task asserts that from its own
side.** That task's workspace rule — no file outside `crates/transcription/`
names `RawToken`, `RawSegment` or `RawTranscript` — was recorded as "a tripwire
for `P2-L4`, the first task that will". It is not: the document is built over
`TranscriptSegment` and `EffectiveToken` at one version, which is what "the
document is layered over the transcript and does not write raw tokens" means
when it is a graph fact. `the_document_names_no_raw_type` holds that here too,
with a control that this crate does read a transcript, so the assertion is not
passing because it reads none.

### What this task found in a neighbouring guard

`engine_source_contains_no_clock_rng_network_or_model` forbade the bare name
`ModelRun` in every engine source. This crate refuses a model-authored coverage
exclusion with an exhaustive `match` over `academic-domain`'s closed `Actor`, so
the rule read three **refusals** of a model as three model calls. The rule is now
`/(?<!Actor::)\bModelRun\b/u`, which is what that scan's own comment already
says it checks — "API spellings, not prose" — and a control pins both directions
so the narrowing cannot widen into a hole: `ModelRun::record()` and
`let run: ModelRun` still trip, `Actor::ModelRun { .. } => refuse()` does not.

`record_harness`'s "nothing extra hides under either directory" walked the whole
of `testdata/engines`, which is wider than the sentence it carries and wider
than the two directories that builder can render. Flipping `TRANSCRIPT_COVERAGE`
to `IMPLEMENTED` put a third directory under that root and the walk failed on
files `academic-record` cannot render. It is now scoped to its own two, and the
root-wide rule stays where it belongs — `engine_registry_is_complete` in
`academic-domain`, which knows every registered engine's directory.

### The two guards this task measured empty in its own suite

`coverage_determinism`'s control varied the gap threshold as well as the
confidence permille. An injection that removed the **whole** configuration from
the report's canonical encoding still passed it, because the two reports
differed by their *gap findings* — the assertion was true for a reason its own
comment did not claim. The control now varies only the permille, which no check
in the report reads, and asserts that the varied configuration changed no
measurement.

`completeness_witness`'s unmapped condition is implied by its coverage
condition. An unmapped segment is in the coverage denominator and not in its
numerator, so whole segment coverage already implies an empty unmapped list, and
deleting the condition changes no row of the suite. The condition **stays** — it
is section 12.6's own sentence, and the implication is a property of the
denominator rule, which is configuration-shaped rather than fixed — and the
implication is now asserted over all 2101 shapes of the partition sweep instead
of being assumed. It is recorded here as a condition that is not independently
observable today rather than as a guard that bites.

### The injection matrix

Twenty injections, applied one at a time to shipped source and reverted after
each. Each is compiled first: a refusal that is a compile error proves nothing
about the scan it was aimed at, and two rows below are recorded as exactly that
outcome with the variant that does compile beside them. The unmodified tree is
run before and after the matrix and passes both times.

Five rows spell **no** forbidden name at all — `L4-I4`, `L4-I5`, `L4-I6r`,
`L4-I7` and `L4-I8` — which is the shape `P2-R2` measured walking past a token
list five times.

| # | Injection | Refused by |
|---|---|---|
| L4-I1 | the classification picks the disposition instead of refusing a double status | `segment_status_exhaustive` |
| L4-I2 | the disposition ledger accepts a second declaration for one segment | the same |
| L4-I3 | the witness stops reading the unmapped list | **nothing**: implied by the coverage condition, recorded above |
| L4-I4 | a second witness producer called `attest`, naming nothing on any list | the witness producer map |
| L4-I5 | `pub fn restore(&PdfArtifact, &LectureDocument) -> LectureDocument`, naming nothing on any list | the workspace pair-of-types sweep |
| L4-I6 | a `floor` field on `CoverageInputs`, naming nothing on any list | **the compiler**: every construction site loses a field |
| L4-I6r | the same field with its construction sites updated, which compiles | `WHOLE_COVERAGE_INPUTS` |
| L4-I7 | a second source-mapping producer called `echo`, naming nothing on any list | the mapping producer map |
| L4-I8 | `LectureDocument::above(floor)` returning a subset of the nodes, naming nothing on any list | `the_preservation_types_offer_no_reducing_method` |
| L4-I9 | a tenth preservation transform | **the compiler**: `ALL` is `[Self; 9]` and the `match`es are total |
| L4-I9r | the same arm with `ALL` and both `match`es widened, which compiles | `WHOLE_TRANSFORM_ENUM` |
| L4-I10 | the crate names `RawSegment` | `the_document_names_no_raw_type` |
| L4-I11 | the token-preservation rule stops reading the rendered text | `lossless_transform_allowlist` |
| L4-I12 | render QA reports clean over a partial measurement | `lecture_render_qa` |
| L4-I13 | the study index carries an empty disclosure | `study_index_disclosure` |
| L4-I14 | two of the three configuration fields leave the report's encoding | **nothing**: a partial injection, remade below |
| L4-I14r | all three leave it | `coverage_determinism`, after its control was repaired |
| L4-I15 | a cross-reference excuses any node whatever segment it names | `ordering_check` |
| L4-I16 | the gap threshold becomes exclusive at the boundary | `audio_gap_threshold` |
| L4-I17 | a committed golden expectation is edited by hand | `harness_corpus_matches_a_fresh_render` |

`L4-I6` and `L4-I9` are the two rows refused by the compiler rather than by a
scan. Both are kept, because what they measure is real — a coverage input is
constructed at five sites and a transform set is `[Self; 9]` with total
`match`es — and each is paired with the variant that compiles.

### The `S-10` decision this crate had to make

Five types hold the lecture in words on the document side: what a caller offers
the builder, what the builder admits, the document itself, and the two
study-index types whose headings are written over the lecture and can quote it.
The decision is `P2-L3`'s, made in the same strengthening direction — every one
is registered in `SECRET_BEARING_TYPES` and hand-writes a redacting `Debug`.

The cost is stated rather than discovered later: a registered type's
hand-written `Debug` may reach a field only through a length, and the field set
includes the identifier newtypes, so `DocumentId`, `NodeId` and `StudyIndexId`
print as byte lengths in every `Debug` output of a type that holds one. They
gained a `len` accessor for that reason, because the length reduction the policy
accepts sits directly on the field and `self.id.as_str().len()` is not it. The
alternative — widening `LENGTH_REDUCTIONS` with `.as_str().len()` — would have
loosened a guard every crate shares to save one crate three accessors, so it was
not taken.

There is no `PUBLIC_BYTES` entry.

## What the `P2-U2` scans hold

Four of `P2-U2`'s claims are statements about what the source does not contain,
and a behavioural test cannot observe an absence.

**`the_rule_types_are_the_specifications_own`** is five halves. Section 11.2's
prose sentence is pinned whole. Its yaml block's `type:` lines are parsed out of
the document, deduplicated in first-appearance order, and compared with
`SPEC_YAML_TYPES`; its prose categories are split on the comma and compared with
`SPEC_PROSE_CATEGORIES`, in order. The union of the two readings is then
required to be exactly `RuleType::ALL`, so a type this crate invented fails as
an extra key and one the specification writes and this crate dropped fails as a
missing one — **and no count is compared anywhere**, which is what injection
`U2-I4` made load-bearing: it dropped a rule type from three places at once,
adjusting both declared lengths, and the test failed against the document.
Fourth, the nine prose-only spellings are required to be what the mechanical
derivation produces, so a respelling fails (`U2-I5`). Fifth, each rule type is
required to have its own `t001` requirement and its own `dsl_*` test, both
injective, and every one of those tests is required to exist in the suite.

**`production_audit_no_llm`** is three halves. The *available* half computes
this crate's transitive product closure from the manifests and compares it
whole, so a dependency of any kind is an extra key — injection `U2-I12` added
`academic-record`, which is on no forbidden list. The *used* half is an API
spelling scan with its samples run through the check inside the test. The
*interpreted* half is the one a token list cannot do: the whole set of `String`
and `&str` fields as owning-type/field pairs, plus a rule that none of eight
audit-path types may own one. That second rule is what survives the strongest
attack — `U2-I24` added a free-text field to `ExecutableRule` **and** added its
row to the allowance table, and was still refused.

**`the_only_route_to_an_executable_rule_is_the_gate`** is four halves. Four
whole-text pins. Struct-literal counts of one each on the two private-field
types, over every product file in the crate, because the orphan rule refuses a
conversion written outside the crate and refuses nothing written in a sibling
module — `U2-I3` put a second door in `publish.rs`. The whole `impl` set naming
either type. And the whole set of public signatures taking a `RuleCandidate` to
a gated value, compared against the gate's own signature: a blanket prohibition
would have been wrong, because the gate *is* that signature and it is the route
that must exist. What must not exist is a second one, and an inventory says that
where a prohibition cannot.

**`the_open_gates_have_no_default`** compares the whole set of `Default`
implementations — `impl` headers and `#[derive]` lists alike — against a
one-entry list. The one that exists is the empty ledger.

**The one-step-out inventory.** `no_file_outside_this_crate_names_a_requirement_rule`
walks every product file in every other workspace package and requires none of
them to name the gated types, the gate, the ledger or the fixture classes. It is
empty today. Its floor counts files **inside** the crate as well as outside, so
the exclusion that makes it an outside-only walk cannot be what makes it empty.

### The two empty guards this task found in its own suite

Both were found by the injection matrix rather than by review, and both were
this task's own tests.

**`U2-I17` — the duplicate that was a duplicate of the wrong thing.** The
injection widened `requirement_rule_review`'s key to
`(rule, reviewer, attested_at)`, so one person can attest twice. The test still
saw a refusal, because it attested twice at the *same* instant: what was refused
was a duplicate row, not a duplicate reviewer. The instant is now a parameter
and the second attestation by the same person uses a different one.

**`U2-I22` — the guard that refused something that was never on offer.** The
injection removed the `equivalent_admitted` check, so an operand that refuses
substitution is discharged by one. The test still passed, because its strict
operand lived in its own one-rule set that held no `EQUIVALENCY` rule at all.
The strict rule now sits in the set beside the equivalency, and a second
assertion requires that equivalency to be live in that set, so the case cannot
pass on an empty one again.

The same shape was then looked for one step out and found once more, in the
store half: the "republished version number" case would also have been refused
by the supersession `UNIQUE`, so it could not say which constraint it measured.
It now supersedes nothing and carries a fresh hash, leaving the primary key as
the only thing that can refuse it.

### The injection matrix

Twenty-four injections, applied one at a time to shipped source and reverted.
Each is compiled first, with `cargo build --all-targets` rather than
`cargo test`, so a trybuild case's *expected* diagnostic cannot be mistaken for
a real build failure. The driver is
`~/.claude/orchestration/run_98ccc873ba4b/t165-injections.py` and the measured
results are `t165-injections.json`. The unmodified tree is run before and after
the matrix and passes both times.

**Twenty-three of the twenty-four compile.** `U2-I2` did not, twice, and is
recorded below as what that cost rather than as evidence.

| # | Injection | Refused by |
|---|---|---|
| U2-I1 | `ExecutableRule` grows a free-text field named nothing on any list | `production_audit_no_llm`, `the_only_route_to_an_executable_rule_is_the_gate` |
| U2-I2 | `RuleOutcome` grows a free-text field — an audit-path type one step from the two named in the check | `production_audit_no_llm` — **after** two non-compiling attempts (below) |
| U2-I3 | a second door from a candidate to a reviewed rule, in `publish.rs`, spelling no conversion trait | `the_only_route_to_an_executable_rule_is_the_gate` |
| U2-I4 | one rule type dropped from `RuleType::ALL` *and* from the prose table *and* from both declared lengths, so no count moves | `the_rule_types_are_the_specifications_own` |
| U2-I5 | a prose-spelled rule type respelled to `NONCREDIT_TRAINING` | `the_rule_types_are_the_specifications_own` |
| U2-I6 | `Applicability` gains a `Default` | `the_open_gates_have_no_default` |
| U2-I7 | an unresolved thesis scope answers `SATISFIED` | `dsl_thesis_research` |
| U2-I8 | an unconfirmed double-counting ceiling is treated as one | `dsl_mutually_exclusive` |
| U2-I9 | the gate stops comparing the two reviewers | `rule_candidate_review_gate` |
| U2-I10 | the ledger stops refusing a republished version number | `ruleset_immutable_publish` |
| U2-I11 | the release gate stops evaluating its fixtures and only checks that both classes exist | `new_rule_release_gate_requires_official_and_synthetic_fixtures` |
| U2-I12 | a product dependency edge on `academic-record`, a crate on no forbidden list | `production_audit_no_llm` |
| U2-I13 | a decimal-point literal on the verdict path | `no_float_reaches_a_requirement_verdict` |
| U2-I14 | the walk is narrowed from the package to `src` | `the_walk_reads_every_module_in_this_crate` |
| U2-I15 | `crates/curriculum` declares a type named `ReviewGate` | `no_file_outside_this_crate_names_a_requirement_rule` |
| U2-I16 | migration `0015` grows an `interpretation TEXT` column, on no forbidden list | `no_requirement_table_carries_a_free_text_column` |
| U2-I17 | the review key admits one reviewer twice | `one_reviewer_cannot_attest_twice_to_one_rule` — **after** the test stopped re-attesting at the same instant |
| U2-I18 | the version chain is allowed to fork | `a_published_version_is_append_only_and_the_chain_does_not_fork` |
| U2-I19 | the version primary key is widened | refused, but by *foreign key mismatch* rather than by the property — see below |
| U2-I20 | the gate admits an attestation filed by an `Actor::ModelRun` | `rule_candidate_review_gate` |
| U2-I21 | an `EQUIVALENCY` substitution is admitted outside its interval | `dsl_equivalency` |
| U2-I22 | an operand that does not admit an equivalent is discharged by one | `dsl_required_course_set` — **after** the strict rule moved into the set that holds the equivalency |
| U2-I23 | a withdrawn attempt is treated as a recognized one | `dsl_corequisite` |
| U2-I24 | `U2-I1` again, with the allowance table updated to match | `production_audit_no_llm`, `the_only_route_to_an_executable_rule_is_the_gate` |

**`U2-I2` did not compile, twice.** The first form put a `String` on a
`RuleBody` variant and failed `E0027` — `evaluate`'s `match` is total and does
not mention the new field. The second widened that one pattern with `..` and
failed `E0063`, because every fixture in the suite constructs the variant. Both
are facts about the crate rather than about the guard, and neither is evidence
for anything: a build failure and a refusal look the same from outside. The
shape it was meant to stand for — a sentence on an audit-path type one step from
the two the check names by hand — was rebuilt on `RuleOutcome`, which has one
construction site, and refused there.

**`U2-I19` is refused for the wrong reason and is recorded as such.**
`requirement_rule`'s composite foreign key needs a unique index on exactly
`(requirement_set_id, version)`, so widening that primary key produces
*foreign key mismatch — "requirement_rule" referencing "requirement_set_version"*
before any row is written, and all five store tests fail on the schema rather
than on the property. The primary key therefore cannot be widened in place at
all. What isolates it is the test's own case, which supersedes nothing and
carries a fresh hash so neither `UNIQUE` can be what refuses it.

## What the `P2-X7` scans hold

Four of `P2-X7`'s claims are statements about what the source does not contain,
and a behavioural test cannot observe an absence.

**`the_center_cannot_name_a_payload_byte`** is four layers, and writing them
found that the strongest one as first drafted was **false**.

The first draft asserted that `academic-egress-boundary` — which owns
`StagedPayload` and `Preview`, the only values in this workspace that hold bytes
on their way to a provider — is absent from this crate's closure. It is not.
`academic-untrusted-content` declares a product edge to it, so every crate that
links `P2-G5`'s trust label carries the same closure: `academic-ingestion`,
`academic-curriculum`, `academic-requirement`, `academic-repository`, and this
one. `P2-U2`'s admission receipt already records it. **The edge therefore cannot
say that type is unreachable, and this page does not say so either.**

What the closure comparison still carries is narrower and is checked: the whole
declared closure is compared at thirteen, and twelve crates that own a canonical
write, a key, a model run or a process — `academic-store`, `academic-vault`,
`academic-crypto`, `academic-keystore-platform`, `academic-projections`,
`academic-transcript`, `academic-record`, `academic-model-run`,
`academic-worker`, `academic-core`, `academic-rpc`, `academic-store-platform` —
are each required to be absent.

What replaces the edge claim is a **closed world over path roots**: every
identifier the crate's product source writes a `::` after, compared in both
directions against a thirty-four-entry allowlist in which `academic_untrusted_content`,
`academic_egress_boundary`, `academic_policy`, `std`, `alloc`, `libc` and
`rusqlite` do not appear. It is read on paths rather than on `use` items, so a
fully qualified `academic_egress_boundary::Preview` is refused even though the
crate is reachable. `P2-R2`'s three repairs are each exercised against the reader
inside the test — a leading `::`, whitespace inside a path, and a middle segment
— and `X7-I5` and `X7-I6` are the injections that made them load-bearing.

The layer that answers `T166`'s finding is the **declared-type set**. Every field
position in the crate is read as an `(owner, name, declared type)` triple; the
type constructors those declarations use are collected across the whole crate and
compared with a reviewed allowlist in both directions. `String`, `str`, `u8`,
`Box`, `Cow` and `Untrusted` are absent from it, and their absence is the claim.
`tools/secret-debug-policy.test.mjs` decided the same question by matching a
field's **name** against a fixed alternation, and `T166` measured a `Vec<u8>`
called `excerpt` passing it; `P2-RF13` replaced that with a whole-set
classification of every byte buffer in workspace product source, so the two
scans now answer it the same way and this crate's triple set is the finer of
the two rather than the only one. Here `excerpt: [u8; 64]` fails as an unreviewed
type (`X7-I1`), a private struct holding `[u8; 32]` fails as an unreviewed type
(`X7-I2`), and a `String` field on a type whose constructor is edited to match
fails the same way (`X7-I3`). None of the three spells anything on the
forbidden-token list. **That list is kept as the explicitly weakest layer** and
is listed last, for the reason `P2-R2` records.

The **public-signature** layer is the same extraction over every `pub fn`, with
argument names excluded. `X7-I4` adds a public accessor returning `[u8; 32]`
backed by no field at all, which the field inventory is blind to by construction.
Beside it, the eight functions that return a `&'static str` are enumerated as a
whole set, because a `&'static str` is the one shape by which text leaves this
crate at all.

**`the_class_of_an_entry_is_its_payloads_type`** compares the whole `impl` set
naming `ProposalClass` against a one-entry list, requires `inbox.rs` to name no
route from text into a class, pins `InboxEntry::class` whole, and compares the
whole set of fields whose declared type is `ProposalClass` against the two that
may hold one — the index reference and the refusal. The compile-fail cases are
the other half.

**`nothing_but_a_user_settles_a_conflict_or_extends_an_expiry`** pins
`user_receipt`, `ConflictCase::settle`, `ConflictCase::offered` and
`ExpiringPermission::has_lapsed` as whole text; counts `UserDecision::by` at one
call site and `self.history.push` at one; compares the whole `impl` set naming
`LivePermission`; counts its construction sites at one, subtracting only the
declaration and the `impl` header, both of which the header comparison already
pins; and requires no permission signature to mutate anything but the two
recording doors.

**`the_provider_and_receipt_vocabularies_are_the_brokers_own`** reads
`crates/policy/src/provider.rs` as text, because this crate deliberately does not
link `academic-policy` as a declared edge. Both surface tokens must appear there,
the broker's own enum must have exactly two arms, the four receipt columns this
crate mirrors must exist, and the two columns it deliberately does not carry —
`receipt_id` and `grant_id`, both `String` — must still exist there and appear
nowhere here.

### The empty guard this task found in its own suite

**`X7-I15` — the assertion that ran on one shape.** Section 30.4's three choices
were asserted once, against a two-conflict corpus whose sides both carry
`CODE_OBSERVED`. An injection that narrowed the offer when the incoming side is
`OFFICIAL_CONFIRMED` never reached that branch, passed
`both_conflict_classes_are_unresolved_until_user_action`, and was caught only by
`the_center_cannot_name_a_payload_byte` — which noticed the unrelated
`EpistemicStatus` path root the injection introduced. **A guard that refuses
something is not evidence that it refuses what you meant.**

The repair drives the whole nine-value status vocabulary on both sides of both
classes, with a compiler-checked witness so a tenth arm in `academic-domain`
stops the suite compiling.

**And the same shape is one step out, found by looking for it.** A sweep is
bounded by what it varies, and the repaired sweep holds the authority class
fixed. `X7-I27` narrows the offer on `AuthorityClass::Curated` and the sweep
cannot see it. `ConflictCase::offered` is therefore pinned as whole text beside
the sweep: the pin refuses a narrowing keyed on anything at all, and the sweep is
what says the constant is actually returned.

### The `P2-U6` guard this task had to widen

`no_captcha_or_access_control_bypass_module_exists` required that **no file**
outside `crates/ingestion/` names `CredentialBinding`, `DeclaredTarget` or
`ConditionalRequest`. `P2-X7`'s acceptance suite names `DeclaredTarget`, because
`source_change_links_impacted_rules_and_plans` drives that crate's stages one to
five to build the two official documents it diffs, and there is no other producer
of an `OfficialDocument`. A locally imitated diff would have made that test
evidence about the imitation.

The prefix test is now a **whole-set comparison** against a one-entry list, in
both directions, and every entry is separately required to be a test rather than
product source. `X7-I28` and `X7-I29` are what say the repair is not a hole: a
product file naming one of the three still fails whatever list it is added to,
and a *second* test file naming one fails as an extra key. The
[official source ingestion](official-source-ingestion.md) contract carries the
corrected sentence.

### The injection matrix

Twenty-nine injections, applied one at a time to shipped source and reverted
with `git checkout --`. Each is compiled first, with
`cargo build -p academic-evidence-center --all-targets`, so a trybuild case's
*expected* diagnostic cannot be mistaken for a real build failure. The driver is
`~/.claude/orchestration/run_98ccc873ba4b/t168-injections.py` and the measured
results are `t168-injection-results.json`. The unmodified tree is run before and
after the matrix and passes both times.

**All twenty-nine compile, all twenty-nine are caught, and after the `X7-I15`
repair every one is caught by the guard it was aimed at.** Two — `X7-I1` and
`X7-I3` — did not compile in their first form, because a field added to a struct
has to be initialised by the constructor that builds it; each is a two-edit
injection for that reason, and neither non-compiling attempt is recorded as
evidence.

**Seven of the twenty-nine spell nothing on any forbidden list** — `X7-I1`
through `X7-I7` — which is what makes the whole-set layers load-bearing rather
than decorative.

| # | Injection | Refused by |
|---|---|---|
| X7-I1 | `TransmissionRecord` grows `excerpt: [u8; 64]`, initialised in its constructor | `the_center_cannot_name_a_payload_byte` |
| X7-I2 | a private struct holding `[u8; 32]`, reachable from no field yet | same |
| X7-I3 | `HistoricalView` grows a `String`, initialised in `view_at` | same |
| X7-I4 | a public accessor returning `[u8; 32]`, backed by no field | same |
| X7-I5 | `::std::mem::size_of` — a leading `::`, spelling no forbidden token and adding no `use` | same, and `nothing_but_a_user_settles_a_conflict_or_extends_an_expiry` |
| X7-I6 | the same path with whitespace inside it | same |
| X7-I7 | `include_str!` reading a sibling module at compile time | `the_center_cannot_name_a_payload_byte` |
| X7-I8 | the walk is narrowed from the package to `src` | `the_walk_reads_every_module_in_this_crate` |
| X7-I9 | two arms of `InboxEntry::class` are rewired | `proposal_inbox_holds_four_typed_classes` |
| X7-I10 | `of_class` stops narrowing and returns every entry | same |
| X7-I11 | the impacted rules are dropped | `source_change_links_impacted_rules_and_plans` |
| X7-I12 | every node in the graph is reported as an impacted plan | same |
| X7-I13 | an undecided conflict reads as settled | `both_conflict_classes_are_unresolved_until_user_action` |
| X7-I14 | settling replaces the history instead of appending | same |
| X7-I15 | the offer is narrowed when the incoming side is `OFFICIAL_CONFIRMED` | same — **after** the sweep replaced a one-shape assertion |
| X7-I16 | the user-only door promotes a model run to a user | same |
| X7-I17 | the code arm reports the math kind | `low_confidence_queue_has_three_span_kinds_with_context` |
| X7-I18 | two kinds share one uncertainty marker | same |
| X7-I19 | the expiry comparison fails open at the expiry instant | `expiring_permission_is_queued_and_blocks_dependents` |
| X7-I20 | an unrecorded permission is treated as unrestricted | same |
| X7-I21 | markers are filtered by the view's own known-at coordinate | `correction_marker_appears_in_historical_views` |
| X7-I22 | the corrected claim is dropped from the past view | same |
| X7-I23 | a requested deletion reads as a receipt | `transmission_log_and_deletion_receipts_are_discoverable` |
| X7-I24 | an empty section is dropped from the index | same |
| X7-I25 | a section is respelled away from the specification | `the_six_sections_are_section_25_13s_own` |
| X7-I26 | a provider surface token drifts from the broker's | `the_provider_and_receipt_vocabularies_are_the_brokers_own` |
| X7-I27 | the offer is narrowed on the authority class, which the sweep holds fixed | `nothing_but_a_user_settles_a_conflict_or_extends_an_expiry` |
| X7-I28 | a **product** file outside `crates/ingestion/` names `DeclaredTarget` | `P2-U6`'s `no_captcha_or_access_control_bypass_module_exists` |
| X7-I29 | a **second test** file outside `crates/ingestion/` names `DeclaredTarget` | same, as an extra key |

## What the `P2-RF13` repair holds

`tools/secret-debug-policy.test.mjs` decided whether a byte buffer leaked by
matching the **field name** against `SECRET_FIELD_NAMES`. `T166` measured the
consequence: a `Vec<u8>` field called `excerpt` under `#[derive(Debug)]` passed
the whole file, 10 pass 0 fail. The list of names a buffer can hide behind has
no end; the list of fields that exist does, so the question is asked the other
way round.

**The whole set is every named field in workspace product source whose declared
type carries bytes** -- `Vec<u8>`, `[u8; N]`, `[u8]`, and any of those inside a
container. There are 137. Each is one entry in `BYTE_FIELD_CLASSES` as
`(type.field, class)`, compared against the source **in both directions**: a
buffer that is not classified fails, and a classification naming no live field
fails. The field's name is not read.

**The class column is a closed vocabulary, not prose.** Thirteen classes, each
declared in `BYTE_CLASSES` with what it means, and a class outside the thirteen
fails. Eleven say where the bytes already are in the clear -- `identifier`,
`digest`, `nonce`, `salt`, `signature`, `mac`, `locator`, `ciphertext`,
`canonical-encoding`, `mask`, `public-fixture`. Two -- `key-material` and
`content` -- forbid a derived `Debug`, and a type holding one must hand-write a
redacting `Debug` and be registered. This is the difference that matters
against `S-10`: widening the net is visible as a new class rather than as one
more plausible sentence in an exception table.

**The name alternation is kept as the explicitly weakest of three layers** and
now reaches `String` and `str` only, where the type genuinely cannot tell
`Qualifier.key` from a key. Adding a name to it closes nothing on its own, and
the file says so where the list is declared. This is the shape `P2-R2` settled
on for the reach guard and `P2-X7` for the declared-type set.

### The six leaks the new net found, and what each one holds

Every one derives `Debug` over bytes under a name `SECRET_FIELD_NAMES` does not
contain, in a crate the alternation had never reached. All six are sealed in
this repair with a hand-written redacting `Debug` and a registration in
`SECRET_BEARING_TYPES`; none was a false positive, and no product code was
excused to make the guard pass.

| Site | What the bytes are |
|---|---|
| `CaptureSession.bytes` (`crates/capture-gate/src/session.rs`) | every chunk the session has accepted -- the lecture audio or the board photograph. `P2-L2` sealed exactly these bytes as `CaptureBytes` in `academic-capture`; this is the same content one crate away, and `S-10` lists `bytes` as a name the vocabulary trails the code by |
| `ReleasableArtifact.bytes` (`crates/capture-gate/src/artifact.rs`) | the same capture after every chunk re-bound against the permission that covered it, which is the arm with the crate's only byte accessor |
| `RecipientRecord.wrapped_vmk` (`crates/crypto/src/recipient.rs`) | one wrapped copy of the Vault Master Key |
| `RecipientRecord.keystore_blob` (same type) | what an operating-system key broker returns. This is the shape `P2-R1` added `blob` to the vocabulary for, and the alternation missed it because the field is called `keystore_blob` and the pattern is anchored |
| `BackupRecipientRecord.wrapped_root` (`crates/recovery/src/backup_key.rs`) | one wrapped copy of the backup root |
| `BackupPlan.profile_recovery_recipients` (`crates/portability/src/encrypted/backup.rs`) | the canonical CBOR of this profile's recovery-class recipient records, which is what a restore on a fresh machine recovers the Vault Master Key from |

Two of the six formatters print **fewer** fields than the type has, and the
reason is this file's own older rule rather than caution: `ReleasableArtifact`
leaves out its `CaptureManifest` and `BackupPlan` leaves out its
`&BackupMasterKey`, because a formatter over secret bytes may reach a
secret-bearing neighbour only through a length. Both say so where they are
written.

### What this task found one step out

The audit loop has found the same mechanism one step outside the repair in
every round, so it was looked for and it is recorded whether or not it bit.

* **A hole in this repair's own first shape, closed here.** The classification
  matched the declared type exactly, so `Vec<[u8; 32]>` and
  `BTreeMap<DomainId, Vec<u8>>` reached it as neither a byte buffer nor
  anything else -- and both print their contents through a derived `Debug`.
  Five such fields exist in workspace product source; two are under a derived
  `Debug`. `LedgerState.registrations` holds `(name, [u8; 16])` identifier
  pairs and `TranscriptChecksums.rows` holds four SHA-256 values per row, so
  **neither leaks** -- what was wrong is that the guard could not see them.
  All five are classified, and the whole set is 137 rather than 132 because of
  it.
* **A stale cross-reference, corrected.** `ReleasableArtifact::bytes` told the
  next reader that `the_only_byte_accessor_is_on_the_releasable_arm` compares
  the whole set of byte-returning signatures against it. No test of that name
  exists anywhere in the repository. The coverage it claims is real and is
  `no_public_signature_hands_out_a_quarantined_capture`, which compares against
  the two-entry `BYTE_RETURNING_SIGNATURES`; the comment now names it.
* **`DESKTOP_FORBIDDEN_WORKSPACE_CRATES` in `tools/phase1-scaffold-policy.test.mjs`
  was read and is not this defect.** It is a ten-name forbidden list, which is
  the shape that fails, but it sits beside a `deepEqual` over the desktop's
  entire declared workspace closure and a second one over its entire resolved
  shipping closure. The whole set is already the load-bearing half and the list
  is the backup, which is the layering this repair adopts. No change.

### What the rebase onto `P2-L4` found

This branch was cut before `P2-L4` merged and rebased onto it afterwards. The
conflict was one hunk -- both tasks appended to the tail of
`SECRET_BEARING_TYPES` -- and stacking the two blocks is the right resolution
there, because they are independent entries of one `Map` literal rather than
two halves of one expression. That was checked by reading the merged text and
then **breaking each side of the merge one at a time**, which is what the two
`fix(workspace-guards)` commits at the head of `main` exist for: `node --check`
passing says nothing about whether a stacked resolution still means what both
sides meant.

Breaking each side measured something the merged text does not show.
**`P2-RF13`'s five registrations are load-bearing and `P2-L4`'s five are not.**
Deleting `CaptureSession` or `BackupPlan` fails `a type whose Debug is
hand-written over secret bytes is registered`; deleting `LectureDocument` or
`StudyIndex` fails nothing. That is not a rebase artefact and not caused by
this repair -- it reproduces on unmodified `main`, where removing
`LectureDocument`'s registration leaves the suite at 10 pass, 0 fail.

`P2-RF15` deleted all thirty-eight one at a time rather than five, and the
count is **21 inert, not ten**: the sampled five were the text half, and sixteen
more came from a second mechanism this section did not reach. Both are on the
`S-18` row and the repair is in the section below.

### The hole this rebase found in `P2-RF13`'s own shape

`BYTE_FIELD_CLASSES` is keyed by `Type.field` and not by path, so two types
sharing a name share one classification. `P2-L4` added a second `CorpusFile`
with a second `bytes`, and it arrived already classified by a line written for
`academic-record`'s type -- silently, and correctly by luck, since both are
committed corpus files. A key that spans crates is now declared in
`SHARED_BYTE_FIELD_KEYS` with the reason it holds for each crate that shares
it, compared in both directions. Four keys span two crates today:
`CorpusFile.bytes`, and the three `DecodedEnvelope` fields that
`academic-admission` and `academic-contracts` each decode.

### The injection matrix

Each was applied to the working tree, the suite was observed failing, the
change was reverted, and the suite was observed passing again. `RF13-I1` is
`T166`'s measured bypass reproduced exactly. `RF13-I2` through `RF13-I4` spell
**no name from `SECRET_FIELD_NAMES` at all**, because injecting a name the list
already contains would measure nothing about the layer that replaced it.
`RF13-I10` is the one case that deliberately does spell such a name, and it is
there to measure the demoted layer rather than the new one.

| Injection | What it is | What caught it |
|---|---|---|
| `RF13-I1` | `T166`'s bypass: `#[derive(Debug)] struct { excerpt: Vec<u8> }` in product source | `every_named_byte_buffer_in_the_workspace_is_classified` (unclassified), then the leak net |
| `RF13-I2` | the same under `watermark: [u8; 32]` -- bytes, harmless name, fixed size | the same two |
| `RF13-I3` | `caption: Option<zeroize::Zeroizing<Vec<u8>>>` -- bytes behind two wrappers, harmless name | the same two, through `normalizeFieldType` |
| `RF13-I4` | `struct T167OneLine { pub annotation: Vec<u8>, pub seq: u64 }` written on one line | the same two; the single-line body shape `T114` found silent stays closed |
| `RF13-I5` | `struct T167Opaque([u8; 32]);` -- no field name at all | `no unregistered tuple type derives Debug over a secret payload` (control: the tuple net was already whole-set) |
| `RF13-I6` | `CaptureBytes`'s hand-written `Debug` rewritten to `let Self { chunk_bytes: note, .. } = self;` and print `note` -- name and type both harmless at the use site | `no hand-written Debug prints a secret field it was written to hide` (control: `T118`'s destructuring layer) |
| `RF13-I7` | `ObjectHeader.locator` deleted from `BYTE_FIELD_CLASSES` while the field still exists | the classification's **missing** half, then the leak net |
| `RF13-I8` | `GoneAway.bytes` added to `BYTE_FIELD_CLASSES` naming no live field | the classification's **stale** half |
| `RF13-I9` | `RecipientRecord.wrapped_vmk` reclassified as the prose `"probably fine, it is wrapped"` | the closed-vocabulary half, which is what stops a widening from being written as a sentence |
| `RF13-I10` | `#[derive(Debug)] struct { plaintext: String }` -- a listed name on a type the classification does not cover | the demoted **name** layer, which this repair's own risk is that it silently stopped reading |
| `RF13-I11` | a second `WireField` with a second `bytes` added to another crate | the shared-key half: a name already classified elsewhere is refused until somebody says the class holds for both |
| `RF13-I12` | `CorpusFile.bytes` removed from `SHARED_BYTE_FIELD_KEYS` while two crates still declare it | the same half's **undeclared** direction |
| `RF13-I13` | `NeverShared.bytes` added to `SHARED_BYTE_FIELD_KEYS`, spanning nothing | the same half's **stale** direction |

`RF13-I5` and `RF13-I6` are controls rather than new coverage: they were
already caught before this repair, and they are here because a matrix that only
exercises the layer just added does not show that the layers beside it still
hold. `RF13-I10` is the specific risk this repair introduces rather than one it
removes: demoting `SECRET_FIELD_NAMES` to `String` and `str` could have
disconnected it instead of narrowing it, and nothing else in the matrix would
have shown that, because every other case is a byte buffer the classification
reaches.

## What the `P2-RF15` repair holds

Deleting each of the thirty-eight registrations in turn and running the suite is
the whole-set form of the sample the rebase section above took. The count is on
the `S-18` row; what the repair found is here.

The widened guard demanded **eight registrations that did not exist**, and seven
are types whose own crate wrote the redaction deliberately -- every one carries a
doc comment naming the rule of this file it was written for -- and then nothing
recorded that the type was covered: `SourceEntry`, `SourceUnit`,
`IngestedDocument`, `ModelOutput`, `FineGrainedToken`, `SealedCredential` and
`Acquisition`. `Digest32` was the one false positive the marker had been
absorbing; it is a public tuple newtype, so it is declared in
`PUBLIC_TUPLE_BYTES` beside the other fifteen rather than registered.

The eighth is a leak. **`AppliedCorrection` derived `Debug` over
`previous_text`**, which is what one token of the lecture read before a
correction replaced it, in the same file as the `CorrectionCandidate` whose
`replacement_text` `P2-L3` had already sealed --
`crates/transcription/src/version.rs`. It is `P2-RF13`'s finding repeated: the
same content class, sealed on one side of a seam and open on the other, missed
because the field name is not in the vocabulary. It now hand-writes the same
redacting `Debug` its sibling does.

| Injection | What it is | Pre-repair guard | Repaired guard |
|---|---|---|---|
| `RF15-I1` | `Ledger { tally: Vec<u8> }`, classified `content`, hand-written `Debug` reducing to a length, spelling no marker, unregistered | 12 pass, 0 fail | `a type whose Debug is hand-written over secret bytes is registered` |
| `RF15-I2` | `struct Tally([u8; 32]);` whose hand-written `Debug` hex-encodes it, in neither `PUBLIC_TUPLE_BYTES` nor the registry | 12 pass, 0 fail | the same |
| `RF15-I3` | `Roster { entry: CaptureSession }` -- one hop from a registered type, hand-written `Debug`, unregistered | 12 pass, 0 fail | the same, and the leak net beside it |
| `RF15-I4` | `ProviderResponse`'s registration deleted -- one of the 21 measured inert | 12 pass, 0 fail | the same |

None of the four spells a name from `SECRET_FIELD_NAMES`. Each was applied to a
clean tree on its own, run against both guards, reverted, and the repaired guard
observed back at 12 pass, 0 fail. The pre-repair column is the point: a
registration that is merely present is what `S-18` was about, so the evidence has
to be a bypass the old guard admitted and the new one does not.

## What the `P2-RF16` repair holds

Six rebase races in this run collided on the same two files, and every one of
them collided on a *registration*: a line written once per crate, at the same
place in the same list, by two tasks at once. The cost is not the merge tool. It
is that a list with one entry per crate has exactly one insertion point, and
`node --check` returns zero on the broken join of two such lists — measured four
times, and the four shapes are in the `P2-U5` rebase section below.

So the registrations that had a source elsewhere are read from that source.
`T177` did this once already, for the receipt **pairs**; this does it for the
receipt bindings themselves.

### What was derived, and from what

`dependency_license_and_source_receipt_is_complete` used to name every admission
receipt nine times over. Enumerated in the file before the change:

| Written once per crate | Count | Now derived from |
|---|---|---|
| a destructured `…ReceiptText` name | 37 | `readdir("docs/security")` |
| a `readFile("docs/security/…json")` | 38 | the same walk |
| a `const …Receipt = JSON.parse(…)` | 37 | the same walk |
| a `…Admitted` and a `…PathPackages` set | 74 | each receipt's own `admissions` and `added_workspace_path_packages` |
| a `…Tuples = lockTuples.filter(…)` and its length assertion | 37 + 37 | one loop over the receipts on disk |
| two clauses of the `incomingTuples` conjunction | 73 | the whole-set difference of the lock and every receipt's claims |
| a summand of the lock-tuple sum | 37 | a `reduce` over the same map |
| an `assert.equal(<receipt>.task, "P2-XX")` | 28 | the key each block is looked up by |
| a cascading duplicate-claim loop | 28 loops, 577 lines, 407 clauses | `no two admission receipts claim the same package`, which `T177` added and which reaches every one of the 1406 ordered pairs 38 receipts make, against the 462 those 407 clauses reach |

**The derived values are the same values.** The incoming set is the same 173
tuples in the same order, `sha256 4f370a5d…`, which is the digest
`dependency-admission-phase1.json` already pins; the receipted tuple count is 94;
and `268 = 173 + 1 + 94` holds as before. Nothing in the receipts, the lock or the
digest moved.

**One clause was missing and nobody could have seen it.** The conjunction had 73
of the 74 clauses the thirty-seven receipts call for: `processAdmitted` was
absent. It is inert only because `P2-G7` admits no external crate, so the
omission has no effect today and would have had one the day that changed. A
whole-set difference cannot omit a term.

**Two assertions are stronger than what they replace.** `P2-G7`'s tuple check
compared against its path packages alone and now compares against its admitted
set as well; and the file name of every receipt must agree with the task it
names, which is the binding the identity lines made for 28 of the 37 and which
`T186` measured as carrying no load in any of them.

### What was not derived, and why

**The receipts' dependency maps.** `direct_workspace_dependencies` and
`dev_workspace_dependencies` look like they mirror `cargo metadata`, and they do
not. Measured against the workspace edges of each receipt's owner crate: **12 of
the 37** direct maps disagree — `P2-K1`'s names six external crates (`argon2`,
`chacha20poly1305`, `hkdf`, `subtle`, `zbus`, `zeroize`), `P2-P1`'s names `serde`,
`serde_json` and `sha2`, `P2-C7`'s names `trybuild`, and eight name nothing where
the crate has edges — and **4 of the 11** dev maps omit a workspace edge the
manifest declares: `P2-N2`, `P2-N3` and `P2-N5` omit `academic-domain`, and
`P2-N6` omits `academic-domain` and `academic-gap`. `P2-G7` is not counted in
either column, because it adds six path packages and has no single owner crate to
measure against. They are a reviewer's account of which edges a task is
answering for, and the account is the only copy. A list that is the only source
of its information is a specification, not a registration, and it stays written
down.

**The frozen graphs.** `workspace_dependency_direction_is_acyclic`'s product and
dev expectations, `SOCKET_CAPABLE_CLOSURES`, `DESKTOP_SHIPPING_CLOSURE` and
`PROCESS_POLICY_CLOSURE` each hold one entry per crate and each is compared
against `cargo metadata`. Deriving them from metadata would compare metadata with
itself. They stay, and they are still the merge points `T186` numbered `MP1`
through `MP3`.

**The `Walk`, `Check` and `Floor` columns of the table above.** They say what a
scan does, and this page is the only place that is written. What is derived from
the first column is the file half, in both directions.

### The injection matrix

Thirteen injections, one at a time, each its own edit and its own run of the
guard it is aimed at, reverted and re-observed passing before the next. **None
spells a forbidden token, because neither guard holds a token list**: one
compares the receipts on disk against the receipts the test binds, the other
compares the files that read Rust source against the rows of one table.
`node --check` returned **0 on every one of them**.

| # | Injection | Guard | Observed |
|---|---|---|---|
| `RF16-I1` | a receipt arrives for a crate nobody binds and claims a package the lock does not hold | scaffold | fails: `a P2-Z9 admitted package is missing from Cargo.lock`, `0 !== 1` |
| `RF16-I2` | `P2-U5`'s receipt is deleted from `docs/security` | scaffold | fails: `docs/security holds no admission receipt naming P2-U5` |
| `RF16-I3` | `P2-N5`'s receipt also claims `academic-offering`, which `P2-U5` claims | scaffold | two tests fail; the pair test names `academic-offering@0.1.0` with both `P2-N5` and `P2-U5` |
| `RF16-I4` | `P2-U5`'s receipt is re-tasked in place | scaffold | fails: `dependency-admission-phase2-u5.json is filed under a name that is not P2-Z9's` |
| `RF16-I5` | `P2-U5` stops claiming `academic-offering` | scaffold | fails on that task's own path-package pin, before the arithmetic |
| `RF16-I6` | a new scan reads Rust source text and has no row in the table | inventory | fails naming `crates/gap/tests/rf16_probe.rs (include_str)` |
| `RF16-I7` | a row names a scan file this repository does not hold | inventory | both directions fail; the reverse names `crates/gap/tests/gap_scan.rs` |
| `RF16-I8` | `crates/offering/tests/offering_scans.rs` keeps its prose section and loses its row | inventory | fails naming the file — this is the case `page.includes` accepted |
| `RF16-I9` | the receipt keeps its task and is filed under another task's file name | scaffold | fails: `dependency-admission-phase2-z9.json is filed under a name that is not P2-U5's` |
| `RF16-I10` | a row for a file the walk never demands names a path this repository does not hold | inventory | fails naming `tools/security-baselines.mjs` |
| `RF16-I11` | `P2-U5`'s receipt is re-tasked, file name and `task` field together | scaffold | fails: `docs/security holds no admission receipt naming P2-U5` |
| `RF16-I12` | one crate `P2-K1` admitted is receipted at a version the lock does not hold | scaffold | fails: `a P2-K1 admitted package is missing from Cargo.lock`, `44 !== 45` |
| `RF16-I13` | a merge drops the last two per-receipt blocks while their receipts stay on disk | scaffold | fails: `an admission receipt in docs/security is read but named by no block here`, naming `P2-P1` and `P2-U5` |

`RF16-I13` is the injection that changed the shape of the repair. Deriving the
sum takes away the coupling that used to make a dropped block noisy: each block
held its own tuple filter, so deleting one left the sum referring to a name that
no longer existed. Measured on the first derivation, dropping the whole `P2-U5`
block was **silent** — `node --check` 0, twenty-seven of twenty-seven passing.
The repair is not to put the coupling back but to say the thing it stood for:
every receipt read off disk must be named by a block, and one that is not is
reported by task.

### What deleting each new check measures

A guard hidden behind another guard can be deleted without any test failing,
which is the shape `P2-N5` found in its own suite. Each check added here was
deleted, and the injection it is aimed at re-run.

| # | Deleted | Injection re-run | Observed |
|---|---|---|---|
| `RF16-S1` | the file-name/task rule | `RF16-I9` | **passes** — nothing else catches it |
| `RF16-S2` | `no two admission receipts claim the same package` | `RF16-I3` | still fails, on `P2-N5`'s own `must admit no external crate` pin, `1 !== 0`; with that pin gone too, on the lock-tuple arithmetic, `268 !== 269`. Neither names the package or either task — the backstop `T177` described, reached through whichever per-receipt pin the claim trips first |
| `RF16-S3` | the reverse table pass | `RF16-I10` | **passes** — nothing else catches it |
| `RF16-S4` | the row rule, relaxed back to `page.includes` | `RF16-I8` | **passes** — which is what that check accepted before |
| `RF16-S5` | the bound-receipt completeness rule | `RF16-I13` | **passes** — nothing else catches it |

`RF16-S2` is the one that is not sole custody, and it is recorded rather than
repaired: a per-receipt pin, the arithmetic and the pair test each refuse a
double claim, and only the pair test says which package and which two tasks.

### How this repair is rebased

A derived registration cannot collide, but the derivation is itself a patch on
two files and `main` moves under it. Twice now: `P2-P1` landed `academic-export`
while the branch was open, and `P2-N6` landed `academic-critical-path` after
that. `P2-N6` conflicted on both files and on nothing else. Neither rebase was a
hunk merge, and neither was merged by hand.

**The branch is reset onto the new `main` and the transformation re-applied to
`main`'s own text**, by a script that recognises the nine per-receipt shapes by
their syntax rather than by line position — so a receipt written in the old
hand-registered shape is derived by the same rules that derived the others.
The script is checked twice before its output is used, because `node --check`
decides neither and returned 0 on a re-derivation that had silently dropped
three lines:

- re-emitting every segment it parsed reproduces its input **byte for byte**; and
- applying the transformation to the commit this repair was first written
  against reproduces this repair's own committed bytes.

The second check is what makes the output the same file the derivation would
have produced had the new receipt been there from the start. Its counts are the
audit: 36 set constructions, 27 identity lines, 28 cascade loops and 36 tuple
filters on that commit, one more of each but the loops on `main`.

What is **not** automatic is the row. `P2-P1` gave
`crates/export/tests/export_scans.rs` a prose section and no line in the
registration table, so the strengthened inventory check failed until the row was
written — the third scan on this page with a section and no row, which is the
case this repair exists to make visible. `P2-N6` wrote rows for all three of its
scans, so its rebase needed no edit there; both sides had appended to the end of
the same table, and the resolution keeps every row from both.

One anchor had to be widened for `P2-P1`: it put its `readFile` at one position
in the destructured list and its `JSON.parse` at another, so a parse run
reconstructed from the destructured names no longer matched the file; the run is
now taken as the literal text that follows the destructuring. That is the same
defect in miniature as the one being repaired — two lists that must agree, kept
in step by hand.

### What is left at a merge point

Three registrations still take one edit per crate in
`tools/phase1-scaffold-policy.test.mjs`, and all three are frozen expectations
rather than lists: the product edge and the dev edge in
`workspace_dependency_direction_is_acyclic`, and the entry in
`SOCKET_CAPABLE_CLOSURES`. `T186` measured all three as caught, and they still
are. A fourth, the per-receipt block, is now caught by name rather than by
arithmetic. In this page a new crate adds a row to the table above, and the row
is now required in both directions; its prose section is not, and dropping one is
still silent — which is `MP11` in `T186`'s measurement and is not closed here.

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
| S-4 | `tools/secret-debug-policy.test.mjs` | no floor on the file walk; `T123 P3-G6` (`I37`, `I46`) still silent. `P2-RF13` added a second floor beside the macro registry that this row does not name: the byte-field classification has 137 entries and a walk reading nothing makes every one of them stale, so an empty walk now fails the stale half even though the leak half still passes vacuously | The walk returning an empty list passes every assertion. The second half of this row — "matters if the walk root moves" — **came true and has been repaired**: `T146` observed the root standing at `crates/*/src` while product-shaped code sat in `examples/` and `probes/`, and `P2-RF10` widened it to the package. The floor half is still open and is now the whole row: a `readdir` that throws is caught and returns an empty list, so a scan that reads nothing still passes. Severity **P3** — it needs a filesystem fault or a moved `crates/` root to fire, and a `>= 200` file floor beside the macro-registry floor would close it. |
| S-5 | `tools/phase1-scaffold-policy.test.mjs` | fixed paths outside `store-platform` | A file renamed or split leaves its assertions reading a path that no longer holds the code they describe. `readFile` throws on a missing path, so a rename fails loudly; a *split* does not — the assertions keep passing against the half that stayed. |
| S-7 | `crates/record/tests/record_scans.rs` | the comment/string stripper distinguishes a character literal from a lifetime by looking for a closing quote two characters on | A character literal wider than one `char` — `'\u{1F600}'` — is not stripped, so its digits would be read as code. No such literal exists in the crate and the scan errs toward reporting rather than hiding, so the failure mode is a false positive, not a miss. Matters if the crate ever needs a wide character literal. |
| S-6 | `crates/portability/tests/encrypted_rotation.rs` | two fixed test-source paths, substring only | It checks that one acceptance row lives in one file. A third file could hold a third copy and nothing would see it. |
| S-8 | `crates/crypto/tests/key_hierarchy.rs`, `crates/keystore-platform/tests/facade.rs`, `crates/domain/tests/question_graph.rs` | the last two still read one fixed path each, and `facade.rs` has a floor while `question_graph.rs` has none | A public item moved out of `keystore-platform/src/lib.rs` or `domain/src/question.rs` into a sibling module is not read. `P2-RF9` repaired the first of the three because `RecoverySecret` lives in that crate and its contract already had a repaired half to match; the other two were left as they are and are recorded here rather than fixed. |
| S-9 | `tools/policy-source-scan-inventory.test.mjs` | six read-position markers plus one `#[path]` hop — a mechanical proxy for "reads Rust source text" | A scan that reaches source some other way — a path assembled from fragments, a walk in a language this does not search — is not found, so the page could miss it and pass. The proxy is stated in the page's own opening sentence, so what the page claims is exactly what the test checks; widening the claim means widening the markers. |
| S-10 | `tools/secret-debug-policy.test.mjs` | **The byte half is closed by `P2-RF13`; the text half is what is left, and it is smaller than this row was written for.** A `Vec<u8>`, `[u8; N]` or `[u8]` field is no longer judged by its name at all: `BYTE_FIELD_CLASSES` enumerates all 137 of them and says what each holds, in both directions, so `bytes`, `escaped`, `staged_text`, `value`, `output` and `text` cost nothing to *reach* any more and the 17-to-19-sites-in-10-to-11-crates measurement below is no longer the price of covering a buffer under one of those names. What the vocabulary still decides is `String` and `str`, where the type cannot tell `Qualifier.key` from a key -- so the eight `text: String` sites the paragraph below calls the real work are still the real work, and they are still a redaction decision in each owning crate rather than a vocabulary line. Everything from here to the end of this row is the original measurement, kept because the `String` half is unchanged by `P2-RF13` and because it is the record of five rounds of trying to close a whole-set problem by adding names. `SECRET_FIELD_NAMES` holds `payload` and `payload_bytes` and not the generic names a raw buffer actually hides behind. `T146` measured four more that pass today: `text`, `escaped`, `bytes`, and `staged_text`, against the control `payload`, which fails. Adding `bytes` alone reaches four pre-existing sites — `WireField.bytes` (`crates/rpc/src/convert.rs`), `FingerprintEncoder.bytes` (`crates/store/src/schema_fingerprint.rs`), `SyntheticTranscriptPdf.bytes` (`crates/transcript/src/source.rs`), `StreamingPrefix.bytes` (`crates/vault/src/object.rs`); `text` and `escaped` reach `QuotedDocument` and `RenderedPrompt` in `crates/untrusted-content`, and `staged_text` reaches `crates/egress-boundary/src/stage.rs`. | Now, for any site that holds something private. Nothing leaks today: all four `P2-G4`/`P2-G5`/`P2-G2` types — `QuotedDocument`, `RenderedPrompt`, `StagedOutput`, `AcceptedOutput` — have hand-written `Debug` impls, and the four `bytes` sites are public buffers. What is open is the **net**, not any site. Severity **P2**, raised from the earlier reading: the vocabulary trails the code by at least the six generic names `bytes`, `text`, `escaped`, `staged_text`, `value` and `output`, and each new crate has added to the gap. `P2-RF10` recorded four of the six; `T149` found `value` and `output`, and `P2-RF11` re-measured all six. **The cost of closing it is measured rather than estimated.** Adding `bytes`, `text`, `escaped` and `staged_text` to `SECRET_FIELD_NAMES` fires 13 sites in 8 crates: `Alias.text`, `PartialAlias.text`, `RegistryFact.text` and the tuple variant `ClaimObject::Text(String)` in `academic-domain`; `SearchHit.text` and `ExactSymbolHit.text` in `academic-projections`; `AliasSpec.text` in `academic-store`; `JsonValue::Text` in `academic-test-support`; and `CorpusFile.bytes`, `WireField.bytes`, `FingerprintEncoder.bytes`, `SyntheticTranscriptPdf.bytes`, `StreamingPrefix.bytes`. Adding `value` and `output` beside them fires four more, in two crates the first four do not reach: `ToolVersionCase.output` (`crates/cli/src/commands/doctor.rs`), `ScenarioAssumption.value` (`crates/scenario/src/simulate.rs`), and `EngineError.value` and `RegistryError.value` in `academic-domain` — 17 sites in 10 crates for all six. The four `untrusted-content` and `egress-boundary` types the vocabulary was widened *for* fire nothing, because all four already hand-write `Debug`. Six is a floor and not a total: widening further to `detail` and `message` fires around twenty more, and those are error-report fields on types that carry no user content, so a wider list is a different decision from this one and not a larger version of it. So the work is not the vocabulary line: it is a redaction decision about the eight `text` sites, which hold entity surface forms, indexed content and claim values — user content, not public buffers — spread over four crates whose contracts this row's task did not read. A `PUBLIC_BYTES` entry silences a field permanently, and writing eight of them to close one row would trade this row for a worse one. Closing it means one commit per crate from its owner, redacting rather than declaring. `P2-G6` added a crate and did not widen this row: `academic-consent` declares no `text`, `bytes`, `escaped` or `staged_text` field at all, because every evidence item it holds is a locator plus a digest plus a byte count and its one place for prose is a closed `NotApplicableReason` enum. `P2-X1` added a crate that declares none of those four either, and **two** `value` fields, so the `value` half of the six-name count is 19 sites in 11 crates rather than 17 in 10. Its owner's decision, made rather than deferred: `Optimistic<T>.value` in `crates/desktop/src/optimistic.rs` already hand-writes a redacting `Debug` and would need a registration line and no redaction work, because it holds an edit the core has not accepted; `Canonical<T>.value` beside it derives `Debug` and **should keep printing**, because it holds a value the core returned a receipt for and the surface is required to display. The seal is on the pending state and the accepted state is deliberately printable, which `debug_does_not_print_the_unaccepted_value` asserts in both directions. So this crate adds one registration line to the cost of widening the vocabulary and no redaction decision, and it writes no `PUBLIC_BYTES` entry. `P2-R1` added a seventh name to the gap and then **closed that one name rather than recording it**: `SealedCredential.blob` in `crates/repository/src/github.rs` is what an operating-system key broker returns, which is half of recovering the secret it holds, and no struct field in this workspace was named `blob` before it. `blob` is now in `SECRET_FIELD_NAMES`, because its measured cost is one site -- that one -- and that site hand-writes a redacting `Debug`, so widening by it needed no redaction work in another crate's contract and no `PUBLIC_BYTES` entry. `R-I10` is the observation that the widened name bites. The six names above it are unchanged and their cost is unchanged; what this shows is that the row is not one decision but one per name, and that the cheap ones can be taken separately. `academic-repository`'s three other new field names were read and left alone: `RepositoryId.identifier` and `CommitId.identifier` are caller-chosen metadata under a validated charset, and `DisclosureDecision.reason` is the user's own sentence about why a digest may be stored, which the audit it exists for has to be able to read. `P2-L2` records its own the same way: `CaptureBytes.chunk_bytes` in `crates/capture/src/capture.rs` holds a lecture recording or a photograph of a board, and it is named from the vocabulary the net **already** reads rather than from the six generic names above, hand-writes a redacting `Debug`, and is registered in `SECRET_BEARING_TYPES`. So it adds nothing to the measured cost of widening the vocabulary and writes no `PUBLIC_BYTES` entry either. `P2-L3` adds seven types to the registry and no `PUBLIC_BYTES` entry: `AuthorizedChunk.chunk_bytes`, `AuthorizedCapture.chunk_bytes` and `ProviderResponse.provider_response_bytes` are named from the vocabulary the net **already** reads, and `RawToken`, `RawSegment`, `CorrectionCandidate` and `EffectiveToken` hold the lecture's words under names it does not -- `text`, `verbatim_text`, `replacement_text`, `effective_text` -- and are registered anyway, so widening the vocabulary by `text` later costs that crate nothing. What the decision cost is stated rather than hidden: a registered type's hand-written `Debug` may reach a raw field only through a length, and `ContentDigest`, `ProviderId` and `ModelVersion` are all in the propagated bearing set, so those types print byte lengths and scalars and **not** their own digests, providers or model versions. That is a real loss of diagnostic detail and it is the direction this row asks for. |
| S-11 | `only_egress_crate_has_a_socket` — the spelling half | **Closed by `P2-RF11`.** `P2-RF10` recorded this row closed and it was not: its rule reads the *call* spelling `libc::syscall(`, and `T149` reached the same socket by number through `use libc::syscall;`, `use libc::syscall as raw;` and `use libc::*;`, each of which compiles, passes `clippy -D warnings`, and carries the spelling only in the `use` item the allowance reads. The scan's own comment beside the rule said the gap was open while this row said closed. Three rules now hold it together: no file may import `libc::syscall` under any of those shapes, and no file may rename `libc` through `extern crate libc as …` or `use libc::{self as …}`, so a call spells the path; every mention of the name in the sandbox backend is itself a call, so the function cannot be bound to a value and called through that; and every `libc::syscall(` call there must name a reviewed `libc::SYS_` constant as its first argument. `P2-RF11` found the last two of those by walking around its own first fix. Why the three reasons this row originally gave for leaving it open were all wrong is in the `P2-RF10` section above. | n/a — closed. |
| S-12 | `os_keystore_capabilities_are_available_but_unused` — `tools/phase1-scaffold-policy.test.mjs` — `tools/secret-debug-policy.test.mjs`, `no_float_reaches_the_gpa_path`, and `phase1_exit_has_no_product_network` | **Closed by `P2-RF10`, and widened again by `P2-RF11`.** All four walked `<crate>/src`; all four now walk the package, less `tests` only, and the eight files outside `src` that spell `process::Command` each carry the reason they are allowed. `P2-RF10` left `benches` out beside `tests`; the reasons it wrote for that were reasons about `tests` — this repository's own suites name `f64` and open the local IPC seam on purpose — and said nothing about benches. See `S-14`. The tree this row named was not the first of its kind — `crates/record/examples/` arrived a commit earlier and has no feature gate — which is why widening only the two walks this row listed would have closed half of it. | n/a — closed. |
| S-13 | `only_egress_crate_has_a_socket` — the syscall rule's file scope | **Closed by `P2-L1`.** The first-argument rule read `crates/worker/src/sandbox/linux.rs` and only that file, because that was the one file whose allowance listed `libc::syscall`; the half about a call reaching its allowance at all was closed by `P2-RF11`'s import ban. What stayed open was the **future second allowance entry**, and `P2-L1` is it: `crates/capture-gate/src/native/linux.rs` reaches Landlock the same way. The rule is now keyed on `RAW_SYSCALL_FILES`, a map from file to the syscalls that file may make — a file on the allowance for `libc::syscall` that is not a key there fails, a call whose first argument is not one of that file's own reviewed names fails, and a reviewed name the file no longer calls fails as a stale exception. The worker's file keeps its extra `denied_syscalls` rule, which is about a seccomp list the capture gate does not build. `L-I9` and `L-I10` are the observations, on both platforms. | n/a — closed. |
| S-14 | `no_float_reaches_the_gpa_path`, `tools/secret-debug-policy.test.mjs`, `phase1_exit_has_no_product_network`, the two `academic-untrusted-content` walks, and the two in `crates/consent/tests/consent_scans.rs` — the `benches` tree | **Closed by `P2-RF11`.** Seven walks excluded `benches` beside `tests`; the last two arrived with `P2-G6` while this repair was in flight and are widened here for the same reason. No `benches` tree exists in this repository, but a bench target has no feature gate and `cargo clippy --workspace --all-targets` — the README verification block's third command — compiles it, which is the two-part test `T146` applied to `examples/`. `T149` measured all three halves: a `f64`, a `#[derive(Debug)]` over `key_bytes`, and a `TcpStream::connect` in a new `crates/record/benches/` file each passed its scan, and a bench that does not compile fails the clippy lane. All seven now exclude `tests` only. | n/a — closed. `tests` stays out on the reasons those walks give for it. |
| S-15 | `the_transport_is_reached_from_no_module_but_the_proxy` — `crates/egress-boundary/tests/byte_path_pin.rs` | **Closed by `P2-RF11`.** This crate's counts read three fixed file names and its fallback inventory read six, in exactly the shape `S-5` and `S-8` record elsewhere, and no row named it. `T149` added `mod relay;` and one new file: the module reached the transport through the broker without binding a grant, wrote 178 bytes under a grant reviewed by another rulepack for a payload `transmit` refused with zero, left no journal row, and passed this crate's suite, `cargo test --workspace --all-targets` and both JS scans. The counts are now sums over a package walk, the inventory is keyed on the walk with a floor, and a module tripwire fails the day the walk is narrowed. | n/a — closed. |
| S-16 | `egress_audit.grant_id`, for rows that are not a consumed grant | The column is polymorphic and only `egress_consumption` resolves it. Deny rows and process-capability activity rows are joined to nothing that says which namespace their identifier came from, so a reader treating the column as an `egress_grant` reference finds them dangling. `P2-M1` does not need them: its reconciliation reads only consumed grants, which `T149` measured is exactly what the join resolves. | The first reader that has to attribute a *denial* or a process activity to a namespace. Closing it means a discriminator column or a second join table; severity **P3**, because no dangling row exists today: all seven `insert_audit` call sites write an identifier that is in one of the two tables. |
| S-18 | `tools/secret-debug-policy.test.mjs` | **Closed by `P2-RF17`.** What was open was measured by deleting each registration in turn and running the suite: **21 of the 38 were inert** -- deleting them left 12 pass, 0 fail -- from two mechanisms, and only one of them is the `S-10` text gap this row was first written for. The other sixteen came from the guard's own shape: it read a hand-written `Debug` only when the impl spelled `<redacted>` or `finish_non_exhaustive`, and ten registered types redact by reducing a buffer to a length and spell neither; and it read a tuple position through `RAW_BYTE_TYPES`, which contains `String`, rather than through the `RAW_BYTE_PAYLOAD_TYPES` this file documents for a position with no name. `P2-RF15` applied `PUBLIC_BYTES` and `PUBLIC_TUPLE_BYTES` in the registration guard, dropped the marker, judged a tuple position as bytes only, and added one hop -- not the fixed point, which reaches `AcceptanceService`. **21 inert became 10**, all of them text under a field name outside `SECRET_FIELD_NAMES`. `P2-RF17` closed those ten with the whole-set classification `P2-RF13` gave byte buffers: `TEXT_CLASSIFIED_CRATES` names the crates whose every `String`/`str` field carries a class from `TEXT_CLASSES`, compared against the source in both directions, and `SECRET_TEXT_CLASSES` says which class a derived `Debug` may not reach. Widening `SECRET_FIELD_NAMES` was not it, for the reason `S-10` records five times over, and the guard now says so in its own failure message. **Re-measured after: none of the 47 registrations there now is inert**, by deleting each in turn. | The classification is a *set of crates* and not the workspace, and that is what is left. 805 named `String`/`str` fields in workspace product source against the 137 byte fields `P2-RF13` classified, and what a text field holds is a judgement its crate's owner makes; `P2-RF17` entered the two that owned the ten inert registrations -- `transcription` (11 fields) and `lecture-document` (18) -- and every other crate stays on the name alternation, which is the weakest layer and is documented as such. Enumerating that gap rather than asserting it: `P2-RF15`'s name-shaped sweep found ten content-shaped `String` fields under a derived `Debug` outside these two crates -- `Alias.text`, `PartialAlias.text`, `RegistryFact.text`, `Question.canonical_text` and `QuestionRevision.{previous,replacement}_text` in `crates/domain`; `SearchHit.text` and `ExactSymbolHit.text` in `crates/projections`; `RuleCandidate.quoted_source` in `crates/requirement`; `ClaimRow.object_text` in `crates/portability`. `crates/projections` and `crates/requirement` hand-write no redaction anywhere, so whether an indexed claim is content is those crates' contract question and not this guard's; `P2-RF17` did not answer it for them. The other limit is inherent to both classifications and not new here: a *wrong* class weakens silently, because the class is the judgement. Severity **P3** -- the ten enumerated fields are the measured remainder, and each is one crate-owner commit of the shape this row now carries. |
| S-17 | `packages/web-contracts/src/index.ts` — the four closed vocabulary sets | `masteryLevels`, `freshnessBands`, `confidentialityValues` and `retentionClassValues` restate `academic_domain`'s `MasteryLevel`, `FreshnessBand`, `Confidentiality` and `RetentionClass`, and **nothing compares the two sides**. This is the defect class `route_manifest_matches_ia_exactly` closes one step away: a list written from an authoritative enumeration with no bidirectional check. `P2-X1` found it while looking for its own kind one step out and did not fix it: the file is `P2-C7`'s contract surface, and a cross-language parity scan is its own reviewed piece of work. All four sets agree with the Rust enums today, measured at this commit, so the row is latent rather than broken. | The first commit that adds a variant to one of the four Rust enums. The TypeScript validator would then reject a fixture the Rust side accepts, and would do so silently until a fixture happened to carry the new variant — the fixture suites pin specific bytes and would not notice a set that had merely stopped being complete. Severity **P3**. Closing it means reading the four variant lists out of `crates/domain/src/lib.rs` and comparing them with the four sets in both directions, the way `model_run_requires_every_field` compares a struct against the specification's own YAML. |
| S-19 | `crates/daemon/tests/phase1_exit.rs::default_build_lane` — `%TEMP%/academic-x1-default-features` | The nested default-feature build writes to one `CARGO_TARGET_DIR` shared by every process on the machine, on purpose, so the build is cached across runs instead of repeated. Cargo takes its own exclusive lock on a target directory, so two processes serialise rather than corrupt each other, and `T175` left it as it is with a `SHARED_NAME_SITES` row saying so rather than making each process build its own copy. | When two processes on one machine run this test with the same `TEMP` and one is killed mid-build. That is not hypothetical: `T169`'s `pkill -9 -f cargo` killed `T162`'s build in this Run. Severity **P3** — the surviving process sees a stale lock or a half-written fingerprint and rebuilds, so the cost is time rather than a wrong answer. Closing it means a per-process lane and a full rebuild per run, which is the trade this row records. |
| S-20 | The eleven `OpenGate::identifier` arms outside `crates/audit` and `crates/offering` — `crates/consent/src/gate.rs` (2), `crates/curriculum/src/gate.rs` (3), `crates/ingestion/src/gate.rs` (2), `crates/requirement/src/gate.rs` (4) | **Closed by `P2-RF17`.** Eleven `GATE-38-xxx` identifiers were hand-written strings whose only check was a **hand-written list in the same test** — `crates/requirement/tests/requirement_scans.rs` and `crates/curriculum/tests/curriculum_scans.rs` were both a `BTreeSet::from([...])` of the identifiers the crate declares, and nothing compared any of them against section 38's own list. This is the class `S-17` records one step away: a list written twice with no bidirectional check against the authority. `P2-U3` found it, closed it for the seven cells `academic-audit` declares, and left the other four crates measured. Each of those four now carries its own `the_open_gates_are_section_38s_own`, which concatenates section 38.1's ten lines, section 38.2's eleven bullets and section 38.3's ten numbered questions **in document order**, finds each cell by the line it quotes, and rebuilds `GATE-38-{:03}` from that cell's one-based position. `GATE-38-027` is the one arm section 38.3 numbers, so the concatenation reaches further than `P2-U3`'s two blocks did. Enumerated on the tree this row now describes rather than asserted: 27 distinct `GATE-38-xxx` identifiers appear in workspace product source; 19 of them are `identifier()` arms across six crates -- `audit` 7, `requirement` 4, `curriculum` 3, `consent` 2, `ingestion` 2, `offering` 1 -- covering 17 distinct cells, because `academic-audit` forwards `academic-requirement`'s `GATE-38-011` and `GATE-38-012` rather than restating them; the remaining ten are cited in prose or in a comment rather than declared. **All 19 arms are now position-derived; none is compared against a typed list.** | The failure this row was opened for is now observed rather than argued. Nine injections, each its own build: four wrong identifiers, one per crate, and five section 38 reorders — swapping section 38.1's ninth and tenth lines, section 38.2's first-and-second, third-and-fourth and seventh-and-eighth bullets, and section 38.3's sixth and seventh questions. Every one failed naming the identifier and the cell it had moved to (`GATE-38-009 is section 38's cell 10`), and the reorders reached `academic-audit` and `academic-offering` as well, so the derivation binds all six crates and not only the four this task edited. What stays open is the ten identifiers cited in prose and comments: those are text, no `identifier()` arm declares them, and nothing derives them. Severity **P4** — a stale citation misleads a reader; it decides nothing. |
| S-21 | `academic_domain::Claim::validate_for_actor` — the actor matrix, for a `PREDICTION` claim | **Still open, and misattributed until `P2-RF17` read both documents.** `AuthorityClass::Prediction` is permitted to `Actor::ModelRun` alone, and `Actor::DeterministicEngine` carries `AuthorityClass::DeterministicEngine` and nothing else, so a **deterministic** historical forecaster cannot sign its own prediction claim as the engine it is. `P2-U5` found that and recorded it as *ADR-003's actor matrix*. **ADR-003 does not write that rule.** It constrains actors in three places, enumerated: a deterministic-engine actor may not self-assign curated authority; a state-removing relation's two claims must share an authority/status pair, listed as six of which one is *prediction/prediction* with no actor named; and the one actor-to-authority rule it states is negative — model, importer and deterministic-engine events cannot assert `USER_EXPLICIT`/`USER_CONFIRMED`, which does not reach `Prediction`. §30.2 defines `PREDICTION` as *미래에 대한 확률적 claim* with no actor in it and reserves the model for `AI_INFERRED` — *model이 source로부터 제안한 해석* — and §30.1's example sources its `PREDICTION` claim from a historical pattern. So the specification permits it, ADR-003 is silent, and the restriction lives only in `academic-domain`'s own `match`. | Widening the matrix is a decision about who may assert what and `P2-RF17` did not make it: the alternative is not free either — ADR-003 requires every active `Prediction` to carry confidence and versioned metadata with a bounded observation window and a positive sample count, and a deterministic engine signing one has to be held to that by something other than the `ModelRun` record's existence. What `P2-RF17` did is make the decision untakeable by drift: `no_document_gives_a_prediction_to_one_actor` in `academic-offering` pins ADR-003's three actor sentences **as a whole set** and §30.1's and §30.2's own words, beside `a_forecast_claim_is_not_signable_by_a_deterministic_engine`, which pins the code. A fourth actor sentence in ADR-003, a `PREDICTION` definition that names a model, or a widened matrix each fail — measured, six injections. What is still open is the product question, unchanged: the first `PREDICTION` claim this repository actually signs. `academic-offering` builds one and validates it; nothing writes one to the ledger yet, because the crate has no store edge. Severity **P3** — today the cost is that a forecast recorded through ADR-003 would be attributed to a model run that did not happen. |

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

## What the `T175` scan holds

`tools/shared-name-isolation.test.mjs` is a source scan, and it is on this page
for the ordinary reason: it reads every `.rs` file under `crates/` and refuses a
shape. What it refuses is not a policy shape but a *naming* one — a name this
repository puts in a namespace it does not own, without a row saying somebody
looked at it.

The defect is the one the three shapes above describe, in a different material.
It has occurred three times. `crates/worker/tests/containment.rs` wrote its home
canary to `<home>/.academic-worker-g4-<label>` and removed it on `Drop`, so two
lanes running that suite at once deleted each other's canary: the survivor
reported 1 passed and 7 failed — exactly the tests that build a `Harness` — with
`ERROR_PATH_NOT_FOUND` standing where the backend owed `ERROR_ACCESS_DENIED`.
`crates/worker/src/sandbox/windows.rs` and
`crates/capture-gate/src/native/windows.rs` ask Windows for one fixed
AppContainer profile name on every launch, and two concurrent asks for an absent
profile tear its directory down; every `CreateProcessW` into the container while
it was absent failed `ERROR_FILE_NOT_FOUND`, at about one run in ten.
`crates/retention/tests/rotation_gate.rs` built
`%TEMP%/academic-rotation-gate-recipients` and removed it at both ends of a test.

Five whole sets, each compared with a committed table in **both** directions, and
no forbidden-spelling list anywhere:

| Set | What is extracted | What an addition costs |
|---|---|---|
| `ENV_VOCABULARY` | Every `env::<item>` two-segment path, whitespace-collapsed and leading-`::` tolerant | A new way to reach outside the process fails until it is classified |
| `ENVIRONMENT_NAMES` | Every variable name a `var`/`var_os` reads, resolved through crate-scoped `const`s and literal `for` arrays, minus every name this repository sets on a child | A read of a name the machine set fails until it has a row; an argument that cannot be resolved fails rather than passing quietly |
| `ROOT_PRODUCERS` | Every function that reads a machine-owned root, or calls one that does, and returns a path — iterated to a fixed point so the set is closed under wrapping | A new wrapper fails |
| `SHARED_NAME_SITES` | Every name built on a producer call, on a local holding the root, or on a parameter one was handed | A new name fails; a name recorded `UNIQUE` that spells none of the eight discriminators fails |
| `PROFILE_GATE_SITES` | Every `CreateAppContainerProfile` call site, and the whole text of the two guard statements its function must open with | A new call site fails; removing the serialisation fails |

The width is stated in the file and repeated here. The scan does not decide
whether a name is *safe*; it decides that no name reaches a shared namespace
without a row. It does not follow a root through a constructor either: the two
`PathBuf::from(value).join(PROFILE_LOCK_FILE)` sites are enumerated as
`profile_lock_path` in `ROOT_PRODUCERS` and written down in
[the worker sandbox contract](worker-sandbox.md) instead.

### The injection matrix

Six injections, one at a time, each its own edit and its own build, and **none of
them spells a name any table forbids** -- because no table forbids a name. Each
was compiled before it was scanned: an injection that does not build is not
evidence. The build is
`cargo clippy -p <crate> --all-targets --locked --offline [--features ...] -- -D warnings`
and the scan is `node --test tools/shared-name-isolation.test.mjs`.

| # | Injection | Compiles | Scan |
|---|---|---|---|
| N-I1 | `crates/store/tests/migration.rs`: `::std :: env :: vars_os().count()` -- a reach spelled with a leading `::` and interior whitespace | yes | fails: `env::vars_os` is not in `ENV_VOCABULARY` |
| N-I2 | `crates/store/tests/migration.rs`: `std::env::var("APPDATA")` | yes | fails: `APPDATA` is read, never set, and not in `ENVIRONMENT_NAMES` |
| N-I3 | `crates/store/tests/migration.rs`: `fn scratch_root() -> PathBuf { std::env::temp_dir() }`, called once | yes | fails: a new function hands out an ambient root and is not in `ROOT_PRODUCERS` |
| N-I4 | `crates/store/tests/migration.rs`: `std::env::temp_dir().join("academic-store-scratch")` | yes | fails: an unlisted `SHARED_NAME_SITES` entry |
| N-I5 | `crates/worker/tests/containment.rs`: the canary name reduced to `.academic-worker-g4-{label}` | yes | fails: an unlisted site, and it spells none of the eight discriminators |
| N-I6 | `crates/capture-gate/src/native/windows.rs`: the two serialisation guards **swapped** | yes | fails: `container_sid` no longer opens with `PROFILE_GATE` |

`N-I5` and `N-I6` are the two regressions this task exists to prevent, injected
back. `N-I1` through `N-I4` are the same defect one step out, in a crate the
repair did not touch.

**`N-I1` passed on its first run, and that is why it is first in the table.**
The whitespace normalisation collapsed `::` and padded the removed characters
back with spaces so that every offset kept pointing at the same character. That
is what defeated it: `::std :: env :: vars_os` became `::std::  env::  vars_os`,
with the gap moved rather than removed, so the path read as two paths and matched
neither. The repair is a real collapse plus a map from each collapsed offset back
to the original, which is what the string-literal check needs and the only thing
the padding was there for. This is the third time on this page that a
normalisation, not a rule, was where a guard was empty.

**`N-I5` and `N-I6` did not compile in their first form**, and the rule on this
page is that an injection which does not build is not evidence. Deleting the
discriminators from the canary name left `sequence` and `nanos` unused, and
deleting the two guards from `container_sid` left `PROFILE_CREATION`,
`ProfileLock`, `profile_lock_path` and three constants unused; `-D warnings`
refuses both. The compiling forms are the ones in the table, and the second is
the stronger injection anyway: swapping the two guards uses every item, compiles
cleanly, and looks like a harmless reordering. It is not -- the pin is on the
*sequence that reaches* the creation call, and a whole-text pin that admitted a
reordering would be the `S-5` shape one layer in.

## What the `P2-U3` scans hold

`crates/audit/tests/audit_scans.rs` holds nine scans. Four read the design
document and compare a vocabulary against it in both directions; three sweep the
crate's own product source as a whole set; one is the tripwire the other eight
rest on; and one compares the whole `Default` set.

| Scan | What it compares | What an addition costs |
|---|---|---|
| `the_walk_reads_every_module_in_this_crate` | every `pub mod` in `lib.rs` against the files the walk read, in both directions | a module with no file fails; a file no module declares fails; a walk that shrank below twelve fails |
| `the_selector_dimensions_are_the_specifications_own` | section 11.1's `·`-delimited sentence against `SelectorDimension::ALL`, in order; the sixth unit's `/`-alternatives against `DegreeMode::ALL` | a dimension dropped from either side fails, whatever happens to the declared length |
| `the_profile_fields_are_the_specifications_own` | section 3's `StudentProfile` keys against `ProfileField::ALL`, in both directions, with the three keys that select no rule named and the one field that has no key named | a key claimed twice fails; a key claimed by nothing and not excluded fails |
| `the_open_gates_are_section_38s_own` | each cell's quoted line against the document, then each identifier against its line's **position** in section 38.1's ten-line block and section 38.2's eleven bullets | a paraphrase fails; a swapped identifier fails; a renumbered section fails |
| `the_proof_statuses_cover_section_11_3s_own_tree` | every `SCREAMING_SNAKE` token section 11.3's tree prints against a mapping, in both directions | a sixth reading in the document fails rather than being folded into the nearest status; a mapped token the tree stops printing fails |
| `no_product_file_reaches_a_clock_rng_socket_or_model` | fourteen API spellings against every product file, with comments and string literals stripped | the rule is run against five evasions inside the test, so a rule that matches nothing fails |
| `no_product_file_names_a_projection_and_only_one_names_a_plan` | three projection names against every product file as a whole set; the plan names against a two-file allowance; `DegreeAudit::evaluate`'s parameter list read off the source | a projection name anywhere fails; a plan name in a third file fails; a plan parameter on the audit function fails |
| `the_three_witnesses_have_one_construction_site_each` | three witness declarations, three crate-private `establish` sites, no public one, the constructor's three parameters by name, and one `DegreeVerdict::Determinate` expression | a second determination route fails; a public `establish` fails; a witness dropped from the constructor fails |
| `the_only_defaults_are_empty_collections` | the whole `#[derive(Default)]` set against a four-entry list, and each of the four required to answer nothing | a type that gains a `Default` fails as an extra key; a hand-written `impl Default` fails outright |

### What this task found one step out

`S-20`. Eleven of the `OpenGate::identifier` arms in this workspace were
hand-written `GATE-38-xxx` strings compared only against a hand-written list in
the same test. This task closed it for its own seven by deriving each identifier
from its line's position in section 38 and left the other four crates recorded
rather than repaired, because each is another task's contract surface.
`P2-RF17` closed those four the same way. The row carries the measurement, what
each derivation reads, and the injections that drive it.

### The count this task had to check before asserting one

`t068`'s `P2-U3` entry names the mixed-status tree as
`SATISFIED`/`NEEDS`/`NOT_SATISFIED`/`UNKNOWN`/`CONFLICT` — five, and the
deterministic engine harness fixes exactly those five. Section 11.3's own tree
prints five leaf tokens too, and **they are not the same five**: it writes
`PASS_PARTIAL`, which the harness has no value for, and prints no `CONFLICT`,
which the harness has. It also labels two structurally identical credit rows
differently — `93 / 130 PASS_PARTIAL` beside `51 / 63 NEEDS 12`.

Enumerating rather than counting is what surfaced that.
[The graduation audit contract](graduation-audit.md) records the mapping and why
both rows render as `NEEDS`, and
`the_proof_statuses_cover_section_11_3s_own_tree` compares the document's tokens
against that mapping in both directions, so the divergence is executed rather
than described. `P2-RF17` added the two-credit-row reading itself: exactly two
`X / Y` rows, each short of its threshold, carrying **different** labels that the
mapping sends to one status. The sentence *labels two structurally identical
credit rows differently* now fails when the document stops being true rather than
staying on the page after it. This is the sixth count divergence found in this plan, after
`§28`'s twelve engines called thirteen, `§31.3`'s fifteen dimensions called
thirteen, `§14.2`'s six states called seven, `P2-U1`'s "five names, four
relations", and `§11.2`'s fourteen rule types called thirteen.

### The injection matrix

Thirteen injections, one at a time, each its own edit and its own build. **None
of them spells a name any table forbids** — the tables here forbid no name; they
compare whole sets and derive identifiers from positions. Each was compiled
before it was scanned: an injection that does not build is not evidence. The
build is
`cargo clippy -p academic-audit --all-targets --offline -- -D warnings` and the
observation is the named test.

| # | Injection | Compiles | Observation |
|---|---|---|---|
| U3-I1 | `profile.rs`: drop `SelectorDimension::ExceptionApproval` from `ALL` **and** shrink the declared length to `[Self; 7]`, so no count moves | yes | fails: section 11.1 still writes 예외 승인, so the two lists differ |
| U3-I2 | `gate.rs`: `GATE-38-003`'s quoted line becomes `Degree mode` — a paraphrase, one character | yes | fails: section 38.1 does not write that line |
| U3-I3 | `gate.rs`: swap the identifiers of `GATE-38-004` and `GATE-38-006`, so both are still real cells | yes | fails: `GATE-38-006` is section 38.1's fourth line's identifier and the position says `GATE-38-004` |
| U3-I4 | `explain.rs`: a `pub fn is_proposed` taking a `&PlannedCoursework` | yes | fails: a plan type is named in a third product file |
| U3-I5 | `engine.rs`: a second `DegreeVerdict::Determinate` arm reached with two witnesses and a third established on the spot | yes, on the **second** form | fails: two determination routes where the pin says one |
| U3-I6 | `engine.rs`: an undischarged operand's `NoneUsed` becomes an empty `Used` | yes | fails: `proof_leaf_completeness` finds a leaf that is not complete |
| U3-I7 | `verdict.rs`: `FreshnessWitness::establish` returns `Some` when no criterion is recorded | yes | fails: `determinate_three_gate`'s no-criterion case reaches a determination |
| U3-I8 | `plan.rs`: `note_for` labels an **earned** course planned-only instead of an unearned one | yes | fails: the annotation finds nothing, which is the half that stops the byte comparison being vacuous |
| U3-I9 | `tests/support/mod.rs`: drop `CSE_MAJOR` from one course's categories | yes | fails: `measure.cse_major_total` reads `3/63` and the oracle says `6/63` |
| U3-I10 | `verdict.rs`: `ConflictFreeWitness::establish` admits a set of unresolved cases | yes | fails: `graduation_conflict_fail_closed`'s blocked audit reaches a determination |
| U3-I11 | `leaf.rs`: `#[derive(Default)]` on `NoAttemptReason`, with a `#[default]` variant | yes | fails: an extra key in the whole `Default` set |
| U3-I12 | `explain.rs`: the drilldown lists only the entries that earned credit | yes | fails: the drilldown is not total over the transcript |
| U3-I13 | `tests/support/mod.rs`: the credit floor becomes `120` | yes | fails: `measure.total_credits` reads `14/120` and the oracle says `14/130` |

**`U3-I5` did not compile in its first form.** Establishing the missing witness
inline with `?` inside a function that returns `Result<_, AuditError>` needs a
`From<Option<…>>` that does not exist, so the first form was a type error rather
than a hole. The compiling form is a `match` on the establish, and it is the
stronger injection anyway: it is what a well-meaning edit that wanted to "handle
the None case" would actually look like, and it produces a determination from
two witnesses and one minted on the spot.

**`U3-I9` and `U3-I13` are the oracle's own evidence.** Neither touches the
engine: one moves a course's category and one moves a threshold, both on the
fixture side, and each moves the Rust answer while the JavaScript transcription
still says the old one. Without a second transcription somewhere else, both
edits would have re-rendered the expected bytes and passed.

## What the `P2-N5` scans hold

`academic-gap` is section 15's gap engine. Three of its ten acceptance rows are
counts — five gap kinds, four state dimensions, eight explanation fields — and
all three are read back out of the design document and compared in both
directions, so none of them is a number a test restates.

The rule the scan exists for is the fourth: `generic_advice_fails_validation`
drives three fluent, plausible recommendations and observes the same seven
structural defects each time. That is a statement about three sentences. The
statement the crate actually makes is that the validator **cannot** be lexical,
and `the_gap_crate_holds_no_phrase_list` makes it: `GapExplanation::defects`'s
whole text is pinned and required to name none of ten ways of reading text
(`contains`, `starts_with`, `ends_with`, `find`, `split`, `chars`, `trim`,
`to_lowercase`, `eq_ignore_ascii_case`, `description`), and every non-ASCII
string literal in the package is compared against the design document's own
cells — section 15.2's five `뜻` and five `예시 대응` cells and section 15.3's
eight field names. A phrase list would appear there as an extra entry.

### The section 15.2 discrepancy this task recorded rather than resolved

Section 15.2's sixth step reads `hard gap, refresh gap, evidence gap, terminology
mismatch를 구분한다` — **four** informal names. The table immediately below it has
**five** rows, and the fifth, `CONTEXT_GAP`, appears in no prose sentence of
section 15.

The table is normative because it is the half that fixes the identifiers, and
`t068`'s acceptance evidence is named `five_gap_types_route_correctly`. So
`GapKind` has five variants and `STEP_SIX_INFORMAL_NAMES` keeps step 6's four.
`the_step_six_prose_names_one_fewer_than_the_table` compares both against the
document and asserts that step 6 still names neither `context` nor `선택`, so a
later edit that adds a fifth informal name is visible. `P2-RF17` widened it from
step 6 to the whole of section 15, because *no prose sentence of section 15* is
the claim and step 6 alone cannot carry it: section 15's lines are split into
prose and table rows and both halves are checked in both directions. Nothing was
invented to reconcile them; `docs/contracts/gap-engine.md` records the reading.

### The misattribution route this task found

`P2-N2` closed one concept's evidence reaching another's history. `P2-N3` closed
the one-hop form and reported that the route surviving every other limit is one
concept's evidence crossing a real edge into a neighbour's reading.
**This engine descends exactly those edges**, so both arrive here and a third
with them.

Section 13.3 licenses a spillover on `REQUIRES`, `BUILDS_ON`, `RELATED_TO` and
`SPECIAL_CASE_OF`, and **two of those four are the edges section 15.2 step 2
descends**. Section 36.4's own worked example is the case: `Buffer Pool` is the
surface concept of an active goal, so it is the concept the user is using now;
`Disk Page` is one `REQUIRES` hop below it; a spillover from `Buffer Pool` across
that very edge puts `Disk Page` at `MODERATE` with no evidence of its own; and
section 36.4's answer is that `Disk Page` **is** the root gap.
`a_band_raised_by_a_concept_on_the_blocking_path_is_refused` observes the
contamination before the refusal — the contaminated band clears
`RETRIEVAL_FLOOR`, and the same concept reads `UNKNOWN` without the contribution
— then observes `search` refusing it by name, then observes that a contribution
from a concept **off** the path is untouched.

It is a traversal-time refusal rather than a type, because whether the neighbour
lies on the blocking path is not known until the descent knows the path. It does
not silently lower the band: the caller re-projects with `P2-N3`'s own function
and without that contribution, which keeps the concept's own evidence.

### The shadowed guard this task found in its own crate

`ConceptState::overlay`'s first shape ran one concept check over every offered
item before admission and a second over each admitted one. **`N5-I2` deleted the
second and every test still passed**, because the first already refused
everything the second would have: `EligibilityOutcome::admit` derives an admitted
item's concept from the same dossier link the first check reads.

The repair is not to delete the redundant half but to make the two halves
disjoint. An **admitted** item carries `EligibleEvidence::concept`, which is the
resolved answer; a **blocked** one does not — `BlockedEvidence` keeps the failing
codes and drops the link, so only the dossier still holds it. The two checks now
sit in the two match arms, cover different items, and neither can stand in for
the other. `N5-I1` and `N5-I2` remove one each and each fails.

### The shadowed fixture this task found in its own suite

`four_state_dimensions_are_overlaid`'s third case blocked **every** offered item
to move the confidence dimension. That empties the admitted set, so `P2-N2`
reports `NO_EVIDENCE_RECORDED` and the first clause of the evidence-gap rule
fires — dimension one answering for dimension three. `N5-I16` deleted the
confidence clause outright and only the whole-text pin noticed.

The repair keeps the mastery-gap case's own admitted item and adds one item that
could not be admitted, so the level, the band and the contradicting set are
identical across the two readings and the only difference is a sufficiency gap.
The fixture now asserts `unseen_basis() == None` before it routes, which is the
assertion that stops the shadow returning. `N5-I16` is caught behaviourally after
the repair.

### The injection matrix

Twenty injections, one at a time, each its own edit and its own build, each
applied to a clean tree and reverted before the next. **None of them spells a
name any table forbids** — the tables here forbid no name; they compare whole
sets, pin whole texts, and derive identifiers from positions. Each was compiled
before it was scanned: an injection that does not build is not evidence. The
build is `cargo clippy -p academic-gap --all-targets --offline -- -D warnings`
and the observation is `cargo test -p academic-gap --offline`.

| # | Injection | Compiles | Observation |
|---|---|---|---|
| N5-I1 | `state.rs`: drop the **blocked**-half concept guard, keeping the accessor call so nothing is unused | yes | fails `one_concepts_evidence_cannot_reach_another_concepts_deficit`: a blocked item linked to another concept enters this concept's sufficiency |
| N5-I2 | `state.rs`: drop the **admitted**-half concept guard, leaving the blocked-half one | yes | fails the same test: an admitted item about another concept enters this concept's mastery. Before the repair above, this injection passed |
| N5-I3 | `engine.rs`: neutralise the path-spillover guard with `&& false` rather than deleting it | yes | fails `a_band_raised_by_a_concept_on_the_blocking_path_is_refused`: section 36.4's own shape reaches a determination |
| N5-I4 | `state.rs`: drop the trace/declaration comparison | yes | fails `a_projection_cannot_hide_a_contribution_it_used`: a projection built from a contribution the caller did not declare is accepted |
| N5-I5 | `graph.rs`: give `HELPFUL` a rung, so the descent crosses a weak `BUILDS_ON` | yes | fails `weak_builds_on_is_excluded_or_conditional` and `five_gap_types_route_correctly` |
| N5-I6 | `routing.rs`: make `EVIDENCE_GAP` a strong deficit too | yes | fails `five_gap_types_route_correctly` and `first_strong_deficit_is_root_with_ancestor_impact`: a root rests on absent evidence |
| N5-I7 | `routing.rs`: lower `RETRIEVAL_FLOOR` one band | yes | fails `four_state_dimensions_are_overlaid`: a `LOW` band stops reporting a freshness gap |
| N5-I8 | `case.rs`: `.take(1)` on the tied roots, which is the auto-selection this repository refuses | yes | fails `equal_candidates_are_both_retained_with_diagnostic` |
| N5-I9 | `explanation.rs`: let a described-but-undated remediation through, which is what broad advice looks like | yes | fails `the_gap_crate_holds_no_phrase_list`: `defects` names `description`, so the validator reads text |
| N5-I10 | `explanation.rs`: admit a `FIELD` subject while still refusing an `ALIAS`, keeping `gap_bearing` in the expression | yes | fails `generic_advice_fails_validation`: `데이터베이스` passes the validator |
| N5-I11 | `node.rs`: the same tier admitted at its source rather than at the validator | yes | fails three tests, including `goal_criteria_required_before_expansion`: a `FIELD` becomes a declarable goal surface |
| N5-I12 | `graph.rs`: keep calling `prerequisite_descriptor` and discard its answer, reading the descriptor directly | yes | fails `weak_builds_on_is_excluded_or_conditional`: `RELATED_TO` becomes traversable |
| N5-I13 | `engine.rs`: `!options.is_empty()` rather than `>= 2`, so one helpful edge is a branch | yes | fails `weak_builds_on_is_excluded_or_conditional`: a useful ordering becomes a `CONTEXT_GAP` |
| N5-I14 | `engine.rs`: keep the set mutated with a value no path can name, so an unsettled identity stops blocking the descent | yes | fails `an_unsettled_identity_stops_the_descent`: a concept below a split identity is judged |
| N5-I15 | `routing.rs`: `len() > 1`, so one recorded failure is ignored while the dimension stays named | yes | fails `four_state_dimensions_are_overlaid` |
| N5-I16 | `routing.rs`: drop the confidence half of the evidence-gap rule, keeping `is_admission_gap` reached under a count no state can hit | yes | fails `four_state_dimensions_are_overlaid`. Before the fixture repair above, only the whole-text pin caught it |
| N5-I17 | `case.rs`: admit a blank `reason` cell while keeping a length rule | yes | fails `gap_case_round_trip`. Before this campaign, nothing observed section 15.1's `reason` at all |
| N5-I18 | `case.rs`: admit any activity shape as a tie diagnostic, under a count no tie can hit | yes | fails `equal_candidates_are_both_retained_with_diagnostic`. Before this campaign, nothing observed the shape |
| N5-I19 | `explanation.rs`: admit a zero-hop path, so the surface concept can explain itself | yes | fails `the_gap_crate_holds_no_phrase_list`: the pinned `defects` text moved |
| N5-I20 | `engine.rs`: report the **strongest** hop as an ancestor's link rather than the weakest | yes | fails `first_strong_deficit_is_root_with_ancestor_impact`: a `STRONG` chain is reported as `HARD` |

**Six injections did not compile in their first form**, and each first form was a
lint rather than a hole: deleting a branch left an import, a helper or a
`mut` unused, and `-D warnings` refused it. Each was reshaped to keep the deleted
guard's names reached — a discarded call, an `is_empty` in place of a length
comparison, a count no input can reach — which is also the stronger injection,
because it is what an edit that meant to keep the code compiling would look like.

**`N5-I17` and `N5-I18` were not caught in the first campaign.** Neither is a
scan finding: both are rules `GapCase` enforces that no test drove, because the
engine always supplies a non-blank reason and always supplies the diagnostic
shape itself. Both are now observed directly through the public constructors,
which is where a caller other than `search` would reach them.

## What the `P2-U5` scans hold

`crates/offering/tests/offering_scans.rs` holds ten scans. Four read the design
document and compare a vocabulary against it in both directions; four sweep the
crate's own product source as a whole set; one is the tripwire the other nine
rest on; and one compares the whole `Default` set.

| Scan | What it compares | What an addition costs |
|---|---|---|
| `the_walk_reads_every_module_in_this_crate` | every `pub mod` in `lib.rs` against the files the walk read, in both directions | a module with no file fails; a file no module declares fails; a walk that shrank below twelve fails |
| `the_four_standings_are_section_8_3s_own` | section 8.3's four table rows -- status, UI 문구 and Planner 취급 cells -- against the four standing types, cell for cell, in both directions | a fifth row in the document fails rather than being folded into the nearest type; a paraphrased UI cell fails; the one row whose document name is longer than the enumeration's must be exactly `CANCELLED/WITHDRAWN` |
| `the_feature_families_are_section_8_3s_own` | section 8.3's feature sentence split at *를 feature로 사용하고*, its six units against the first six families in order, and the seventh's phrase required to be after the split and not before | a seventh unit appearing in the sentence fails; a paraphrase on either side fails; two families sharing one frozen-input key fails |
| `the_abstention_reasons_are_section_8_3s_own` | the `UNCERTAIN` row's `·`-delimited grounds against the reasons that quote one, as two whole sets | a reason that quotes the wrong one of the row's own three fails; a reason that invents a ground fails; the three that quote nothing are enumerated by name |
| `the_open_gate_is_section_38s_own` | the quoted line against the document, then the identifier against the bullet's **position** in section 38.2's eleven bullets after section 38.1's ten lines | a paraphrase fails; a different real `GATE-38-xxx` fails; a renumbered section fails |
| `no_product_file_reaches_a_clock_rng_socket_or_model` | fourteen API spellings against every product file, with comments and string literals stripped | the rule is run against five evasions inside the test, so a rule that matches nothing fails |
| `no_floating_point_reaches_a_forecast` | six floating-point spellings against every product file | run against four evasions inside the test; a Brier score in binary floating point would depend on the machine that computed it |
| `no_product_file_promotes_a_prediction` | every `fn` signature and every `impl` header in the crate as two whole sets: none may name a prediction-side type and a confirmation-side type together, and the one that legitimately does is named | a promotion called anything at all fails; a `From` between the two sides fails; the single `ConfirmedSeat` construction site is counted with a type-name-boundary rule that tells it apart from `NoConfirmedSeat` |
| `nothing_in_this_crate_has_a_default` | the whole `#[derive(...)]` set and every `impl Default` header | a `Default` on a recorded criterion fails -- which the behavioural test beside it would not have caught |
| `this_crate_persists_nothing_and_registers_no_engine` | three store spellings against every product file; the engine identifier against the §28 registry and against its namespace; the registry's twelve entries; the harness root's directories | an identifier claiming `engine.` fails; a directory under `testdata/engines/` fails; a registry that stopped holding twelve fails |

### What this task found in its own suite

**Section 8.3's `HISTORICALLY_LIKELY` cell is a conjunction and only one half
was implemented.** The row requires *여러 과거 학기의 재현 가능한 패턴, 미래 공식
공지 없음*. The pattern half was structural from the start; the second half was
not representable at all — `OfficialTermReading` had `Confirmed` and `Cancelled`
and no value for an official notice that the course *will* run without a
verified listing, so such a notice left the standing at `HISTORICALLY_LIKELY`,
which the row forbids. Found by reading the requirement cell against the
resolver rather than by any test failing. `U5-I24` is the observation.

**`announcement_claim`'s first draft took the announcement and did nothing with
it** — the exact defect `offering_feature_contract` refuses one level down, in
the function written to close the gap above. The notice now bounds the claim's
validity, and `U5-I25` is the observation.

**Two guards would not have bitten, and both were repaired before the matrix was
run.**

`the_recorded_criteria_have_no_default` checked that `ForecastPolicy::new`
refuses an out-of-range floor and a zero window. It says *no default* in its
name and an `impl Default for ForecastPolicy` returning the corpus's own numbers
passes it unchanged — the constructor's refusals are still true. The absence of
a `Default` is a source property, so it is now
`nothing_in_this_crate_has_a_default`, a sweep over the whole derive set and
every `impl Default` header with a floor. `U5-I20` is the observation.

`same_inputs_and_rule_hash_yield_byte_equal_results` compared two evaluations
and one hand-built encoding under another rule-set hash. Neither half reads the
frozen inputs, so **dropping a recorded criterion from them passes both**: two
forecasts answering different questions would then have equal canonical bytes.
The test now also evaluates the same history under a second recorded floor and
requires the bytes to differ. `U5-I21` is the observation.

### What this task found one step out

`S-21`. The actor matrix in `academic_domain::Claim::validate_for_actor` gives
`AuthorityClass::Prediction` to `Actor::ModelRun` alone, while §30.1's own
example of a `PREDICTION` claim is *status PREDICTION · historical pattern ·
confidence .72* — a pattern, not a model. A deterministic historical forecaster
therefore cannot sign its own prediction claim as a deterministic engine. This
task did not widen the matrix; it recorded the divergence and executed it.
`P2-RF17` read ADR-003 and found the matrix is not ADR-003's: the ADR states
three actor constraints and none reaches `Prediction`. The row carries the
enumeration.

### The privacy pass runs after type checking

Two of this task's first five compile-fail cases proved less than they claimed.
Rust checks privacy **after** type checking, so a case that bundled a private
struct literal with a wrong-arity call never reached the privacy pass: it failed
to compile, passed the suite, and its committed `.stderr` carried no `E0451` at
all. The two literal cases now hold nothing but the literal, and the seven cases
are:

| Case | Diagnostic | What it proves |
|---|---|---|
| `a_likely_standing_has_no_seat` | `E0599` × 3 | the three standings that are not `CONFIRMED` have no `seat` method |
| `a_confirmed_seat_cannot_be_assembled` | `E0451` | all five fields of `ConfirmedSeat` are private |
| `a_confirmed_seat_has_no_default_and_no_setter` | `E0599`, `E0616` | no `Default`, and no field write on a seat obtained legitimately |
| `a_scored_forecast_cannot_be_assembled` | `E0451` × 2 | a calibrated probability and a disclosed window cannot be assembled by hand, and neither can the standing that holds them |
| `a_determinate_plan_cannot_be_assembled` | `E0451` | the plan's seat list is private |
| `an_indeterminate_plan_cannot_be_empty` | `E0061` | the first refusal is a parameter |
| `a_forecast_does_not_become_confirmation_evidence` | `E0308`, `E0277`, `E0615` | a forecast is not a listing, there is no `From` between the two sides, and the standing has no status setter |

### What the rebase onto `P2-N3` and `P2-L5` found

`main` gained `academic-freshness` and `academic-student-voice` while this task
was in flight, and both added their own rows to
`dependency_license_and_source_receipt_is_complete`. Six merge points in
`tools/phase1-scaffold-policy.test.mjs` conflicted, and **`node --check` is not
what settled any of them**: it passed on the concatenation of three of the six —
the destructured names, the `readFile` list and the `JSON.parse` lines, whose
items are already terminated — and refused the other three, where `main`'s side
ends mid-expression. Those three needed a real edit rather than a paste: an
assertion left open by a `);` that had moved, a filter term ending in `,` where
`&&` was now needed, and a summand ending the sum.

Then each merge point was **broken one at a time and the guard was run**, in
both directions — this task's contribution deleted, and `main`'s. Fourteen
breaks, and `node --check` returned zero on **every one of them**:

| Break | `node --check` | `node --test` | Observed |
|---|---|---|---|
| point 1, either side: a receipt text is not destructured | 0 | fails | `dependency_license_and_source_receipt_is_complete` |
| point 2, either side: a receipt is not read, so every later position shifts | 0 | fails | same |
| point 3, either side: a receipt is never parsed | 0 | fails | same |
| point 4, either side: `assert.equal(<receipt>.task, …)` alone is deleted | 0 | **passes** | nothing |
| point 4, either side: the whole per-task block is deleted | 0 | fails | same |
| point 5, either side: a package is not excluded from the incoming set | 0 | fails | same |
| point 6, either side: a tuple count is not in the sum | 0 | fails | same |

**The `task` identity line carried no load on its own.** Deleting
`assert.equal(offeringReceipt.task, "P2-U5")` — or `main`'s equivalent — was
caught by nothing: the rest of the block still ran, still built its two sets and
still computed its tuple filter, so the sum in point 6 was unchanged. It was a
second layer over a positional one, and the positional one is what points 1
through 3 measure. All six points are gone — `P2-RF16` derives them from
`docs/security` — and the task is now the key each block is looked up by, which
is what this measurement said closing it would take.

### The injection matrix

Twenty-five injections, one at a time, each its own edit and its own build.
**None of them spells a name any table forbids** — the tables here forbid no
name; they compare whole sets and derive identifiers from positions. Every
paraphrase substitutes another phrase the specification itself writes, and every
identifier substitution is another real §38 cell. Each was compiled before it
was scanned: an injection that does not build is not evidence. The build is
`cargo clippy -p academic-offering --all-targets --offline -- -D warnings` and
the observation is `cargo test -p academic-offering --offline --no-fail-fast`.

| # | Injection | Compiles | Observation |
|---|---|---|---|
| U5-I1 | `source.rs`: the recorded verification bound is read as four times itself | yes | fails `offering_confirmed_contract`: a two-day-old registration reading confirms under a one-day bound |
| U5-I2 | `standing.rs`: the `HISTORICALLY_LIKELY` UI cell gains one syllable — a paraphrase, one character | yes | fails `historical_likely_limits` and `the_four_standings_are_section_8_3s_own`: section 8.3 does not write that cell |
| U5-I3 | `forecast.rs`: the 불규칙 abstention is unreachable — the guard requires no offered term | yes | fails `uncertain_offering_flow`, the oracle comparison and `term_forecast_metrics` |
| U5-I4 | `source.rs`: a history-derived reading may issue an official cancellation notice | yes | fails `cancelled_offering_contract`: a prediction cancels an offering |
| U5-I5 | `source.rs`: a cross source is a disagreement when it is *newer* rather than when it disagrees | yes | fails `offering_source_authority`: an agreeing cross source is reported as a disagreement |
| U5-I6 | `feature.rs`: 미개설 gap is still measured and its contribution is a constant | yes | fails `offering_feature_contract`, the oracle comparison, `offering_epistemic_split` and `term_forecast_metrics` |
| U5-I7 | `forecast.rs`: the disclosed window ends one millisecond past the last reading | yes | fails `course_forecast_metadata`: the window is pinned to the readings that happened |
| U5-I8 | `claims.rs`: a prediction stays active for decision after an official claim arrives | yes | fails `prediction_official_parallel` |
| U5-I9 | `feature.rs`: a term that was *read* counts as a term the course ran in | yes | fails `zero_observation_semantics`, `offering_feature_contract`, the oracle comparison and `term_forecast_metrics` |
| U5-I10 | `metrics.rs`: coverage is the complement of abstention | yes | fails `term_forecast_metrics`: 500 where the oracle says 400 |
| U5-I11 | `standing.rs`: the `UNCERTAIN` UI cell becomes the `CANCELLED` one — both are section 8.3's own | yes | fails `offering_epistemic_split`, `uncertain_offering_flow` and `the_four_standings_are_section_8_3s_own` |
| U5-I12 | `plan.rs`: a choice with no confirmed seat is skipped instead of refused | yes, on the **second** form | fails `historically_likely_cannot_enter_determinate_plan`, `historical_likely_limits`, `uncertain_offering_flow` and `cancelled_offering_contract` |
| U5-I13 | `corpus.rs`: the irregular case's empty spring becomes a fourth special run | yes | fails the oracle comparison: the Rust answer moves and the JavaScript transcription still says the old one |
| U5-I14 | `corpus.rs`: one calibration bin reads a higher permille | yes | fails the oracle comparison, for the same reason |
| U5-I15 | `gate.rs`: the identifier becomes another real section 38 cell | yes | fails `the_open_gate_is_section_38s_own` and `the_open_gate_holds_every_term`: the bullet's position says `017` |
| U5-I16 | `feature.rs`: two families swap the phrases they quote, and both phrases are still section 8.3's | yes | fails `the_feature_families_are_section_8_3s_own`: the order is the sentence's |
| U5-I17 | `standing.rs`: a signature naming a forecast and a confirmation, called neither `promote` nor `upgrade` | yes | fails `no_product_file_promotes_a_prediction` |
| U5-I18 | `feature.rs`: the seasonal rate is computed in binary floating point | yes | fails `no_floating_point_reaches_a_forecast` |
| U5-I19 | `forecast.rs`: the 불규칙 ground quotes 표본 부족 instead, which is still one of the row's own three | yes | fails `the_abstention_reasons_are_section_8_3s_own`: the two sets differ |
| U5-I20 | `policy.rs`: the forecast policy gains a `Default` with the corpus's own numbers | yes | fails `nothing_in_this_crate_has_a_default` — and passes `the_recorded_criteria_have_no_default`, which is why that guard was repaired |
| U5-I21 | `forecast.rs`: the recorded likely floor is not a frozen input | yes | fails `same_inputs_and_rule_hash_yield_byte_equal_results` — and passed both of that test's original halves, which is why it gained a third |
| U5-I22 | `corpus.rs`: a helper that reads the machine's clock | yes | fails `no_product_file_reaches_a_clock_rng_socket_or_model` |
| U5-I23 | `forecast.rs`: the engine identifier claims the §28 registry's namespace | yes | fails `this_crate_persists_nothing_and_registers_no_engine` |
| U5-I24 | `standing.rs`: an official notice that the course will run does not defeat the pattern, so the row keeps its standing without its second conjunct | yes | fails `historical_likely_limits` |
| U5-I25 | `claims.rs`: the announcement argument reaches no part of the output, so an official claim may be backdated past the notice that made it | yes | fails `prediction_official_parallel` |

**`U5-I12` did not compile in its first form.** Replacing the `NoConfirmedSeat`
arm with an empty one leaves a `match` whose second arm does nothing, which
`clippy::single_match` refuses under `-D warnings`, so the first form was a lint
error rather than a hole. The compiling form is the `if let` a well-meaning edit
that wanted to "only report the interesting case" would actually write, and it
is the stronger injection: it drops the refusal a plan is built on while leaving
the other one in place.

**`U5-I13` and `U5-I14` are the oracle's own evidence.** Neither touches the
engine: one moves a term's reading and one moves a calibration bin, both on the
fixture side, and each moves the Rust answer while the JavaScript transcription
still says the old one. Without a second transcription somewhere else, both
edits would have re-rendered the expected values and passed.

## What the `P2-P1` scans hold

`crates/export/tests/export_scans.rs`. `INV-C-015` is the claim that a user can
read their own record when this product and their school account are both gone,
and `restore_without_vendor_or_school_account_succeeds` runs the reader with no
credential and no profile. That is the behaviour. It cannot see a dependency
that would reach a network inside a call that test happens not to make, and it
cannot see a byte buffer added to a type next year, so both halves are here and
each catches what the other cannot.

| Scan | What it reads | What it refuses |
|---|---|---|
| `the_walk_reads_every_module_in_this_crate` | `crates/export/src` recursively, and `lib.rs`'s `pub mod` lines | a module declared without a file the walk reaches, and a file no module declares |
| `the_product_closure_is_exactly_the_declared_edges` | `crates/export/Cargo.toml`'s `[dependencies]` | any addition to the six product edges, and by name a store, vault, crypto, keystore, recovery, retention, projection, transport, connector or model edge |
| `the_product_source_reaches_only_the_declared_vocabulary` | every product file, as four whole sets and two per-item sets | a new `snake_case` path root, a new crate brought into scope, a new `std` module, a new item under `std::process` or `std::os`, or a new macro |
| `no_type_in_this_crate_holds_an_unclassified_byte_buffer` | every `struct` and `enum` body in the crate | a byte-typed field or tuple position that the classification table does not name, in both directions |
| `the_only_clock_read_names_a_staging_directory` | every product file | a second `SystemTime::now`, or the one call moving out of the staging-path reservation |
| `the_portable_path_rules_match_the_phase_1_export` | `crates/portability/src/lib.rs`'s reserved-name list | the repeated path rules forking from the ones they repeat |

### What this task found in its own guard

The `std`-module sweep was written as "every second segment of a `std::` path"
and it was blind to the shape this crate actually uses. `use std::{fs, io,
sync::atomic::Ordering}` reaches `std::sync` and spells `std::` **once**, so the
sweep saw `fs` and nothing else: `sync` and `time` were invisible, and
`std::net::TcpStream` written the same way would have been invisible for exactly
the same reason. The repair is `expanded_uses`, which rewrites every `use`
statement into one full path per line before the sweep reads it. `P1-I15` is
the injection that reaches the sweep **only** through that expansion — a
transport module inside a braced group, spelling `std::` once — and `P1-I16` is
the direct form the group form was hiding behind.

Then the same guard had the defect one layer in. Admitting a module is not
admitting everything in it: `std::process::id` names the staging directory and
`std::process`'s process launcher starts a program, and once the first was
needed the module-level set admitted the second. The two admitted-but-sharp
modules now carry their own whole item sets, so `std::process::exit` and
`std::process::abort` each fail as an addition. `P2-RF11`'s sentence held again:
assume there is one more.

### The byte-buffer table is empty, and that is the point

`P2-RF13` and `P2-RF15` found seven `Debug` leaks, four of them in `crypto`,
`recovery` and `portability`. That the workspace-wide net in
`tools/secret-debug-policy.test.mjs` passes is not evidence for a crate it has
not been re-measured against, so the same question is asked at this crate's own
boundary. The answer today is that no type here holds bytes: they stream through
a fixed buffer inside `directory::copy_new_file` and are never held in a value.
The table is therefore empty, and a byte-typed field added to any type of this
crate fails until somebody records what it holds.

### The injection matrix

Twenty injections, one at a time, each its own edit and its own build, each
reverted before the next with `git status --porcelain` read to prove the tree
came back clean. **None of them spells a name any table forbids** — the tables
here compare whole sets and derive identifiers from positions. Every one was
compiled before it was observed: an injection that does not build is not
evidence, and four had to be reshaped after their first form failed to compile.
The builds are
`cargo clippy -p academic-export --all-targets --offline -- -D warnings` and
`cargo test -p academic-export --offline`.

| # | Injection | Compiles | Observation |
|---|---|---|---|
| P1-I1 | `write.rs`: an original is written at a path derived from its **vault locator** rather than its artifact identifier | yes | fails: two artifacts with identical bytes share one locator, so the second copy hits an existing file |
| P1-I2 | `write.rs`: a withheld original records the path its bytes would have had | yes, on the **second** form | fails: `original_inclusion_is_user_selected_with_no_dangling_locator`, and `restore_without_vendor_or_school_account_succeeds` |
| P1-I3 | `write.rs`: a topical part's claims are labelled `PUBLIC` instead of their domain's label | yes, on the **second** form | fails: `export_carries_labels_restrictions_and_notices` |
| P1-I4 | `write.rs`: every file carries the bundle notice instead of its domain's | yes | fails: the same test, on the notice rather than the label |
| P1-I5 | `source.rs`: the security-domain list is a hash set rather than a sorted vector | yes | fails: `domains_are_sorted_and_deduplicated` — **and nothing else**; see below |
| P1-I6 | `part.rs`: one section 37 bullet is paraphrased by one character | yes | fails: the specification's own list disagrees |
| P1-I7 | `part.rs`: one part is dropped from `ALL` **and** the declared length shrinks with it, so no count moves | yes | fails: the same comparison, in the other direction |
| P1-I8 | `audit.rs`: the re-run compares outcome **lengths** instead of outcome bytes | yes | fails: `clean_offline_restore_reruns_deterministic_audit` — after two repairs; see below |
| P1-I9 | `read.rs`: a bundle holding a file the manifest does not list is accepted | yes | fails: the stray-file case |
| P1-I10 | `read.rs`: one of the audit's four referenced paths is left unchecked | yes | fails: the dangling-locator case — after a repair; see below |
| P1-I11 | `write.rs`: a recorded domain label weaker than the ledger is admitted | yes | fails: the understated-register case |
| P1-I12 | `graph.rs`: the JSON-LD graph addresses an artifact node by its vault locator | yes | fails: the graph holds three artifact nodes where the ledger holds four |
| P1-I13 | `bundle.rs`: every file record claims its content may be redistributed | yes | fails: the restriction no longer follows the label |
| P1-I14 | `bundle.rs`: the semantic digest drops its length prefix | yes | **passes**; see below |
| P1-I15 | `directory.rs`: a transport module reached through a braced `use` group | yes, on the **second** form | fails: `std::net` is not in the module set, and the expansion is what sees it |
| P1-I16 | `write.rs`: the writer reads an ambient variable | yes | fails: `std::env` is not in the module set |
| P1-I17 | `directory.rs`: a sharp item under an **admitted** `std` module | yes | fails: `std::process::exit` is not in that module's item set |
| P1-I18 | `bundle.rs`: a byte buffer on a new type of this crate | yes, on the **second** form | fails: nothing classifies it |
| P1-I19 | `directory.rs`: a second clock read | yes | fails: the crate reads a clock twice |
| P1-I20 | `directory.rs`: one reserved Windows device name dropped | yes | fails: the repeated rule set forked from the one it repeats |

**Four did not compile in their first form.** `P1-I2` and `P1-I3` left an import
and a binding unused; `P1-I15` imported a trait it never called; `P1-I18` added
a `Vec<u8>` to a `Copy` type and broke every initializer. Each was reshaped
until it built, because the discipline is that an injection which does not
compile has measured nothing.

**Three passed, and each was a different empty guard.** They were found by the
campaign rather than by review, and two are repaired:

- `P1-I5` replaced the sorted domain list with a hash set and every acceptance
  test still passed. The obvious explanation — one domain in the corpus, and one
  element is in order whatever holds it — was wrong: a second security domain
  was added to the fixture and the injection still passed. The domain order
  reaches **no byte** of a bundle, because every file list is sorted by path
  before it is written and every label is a maximum. What holds it is the
  function's own contract test, and the fixture's second domain stayed anyway,
  because it is what makes the label, restriction and notice checks compare two
  different values instead of one repeated one.
- `P1-I10` deleted one of the four audit-path checks and the dangling-locator
  case still passed. That case edited `manifest.json` in place, and the reader
  refuses on the recorded semantic digest long before it reaches the locator
  rule — so the case had been measuring the digest and nothing else, for its
  whole life. It now **re-seals** the manifest after breaking it, over four
  sites: the audit's first path, its last, a part record's file list, and an
  included original's path.
- `P1-I8` weakened the re-run's byte comparison to a length comparison and
  passed **twice more** after the first repair. Matching the refusal's arm did
  not help, because the mismatched-audit case's two outcomes differ in length as
  well as in content and the weakened check still reached the right arm. What
  closes it is a bundle whose recorded audit is correct, whose outcome file is
  edited by one byte and no more, whose file digest is corrected and whose
  manifest is re-sealed: every other check in the re-run then agrees, and the
  byte comparison is the only thing that can refuse.

**`P1-I14` still passes, and that is recorded rather than repaired.** It removes
the semantic digest's length prefix, and no test can see it because the sentence
beside it was false. The digest is one hash over one JSON document; every field
is bound because the structure is in those bytes, and the length only keeps the
domain separator from running into the body. Making it observable would need a
`semantic` block whose JSON begins with the separator's tail. The claim was
corrected instead — a sentence nothing executes is the defect, not the missing
test.
