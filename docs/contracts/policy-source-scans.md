# Policy source scans

A *policy source scan* is a test that reads this repository's own source text
and refuses a shape. It exists for the class of change that alters nothing
observable at runtime — a second key source, a widened allowlist, a suppressed
banner behind a marker file — where there is no behaviour to assert against and
the source is the only evidence.

This page enumerates every one of them, because the same defect has been found
in a scan one step outside the one just repaired in each of the last three
rounds, and each time the next person started the survey from nothing.

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

## Every scan in this repository

"Walk" is how the scan reaches files. "Check" is what it does with the text.
"Floor" is what fails if the walk returns less than it should.

| Scan | Walk | Check | Floor |
|---|---|---|---|
| `no_environment_or_flag_override_exists` — `crates/cli/src/main.rs` | recursive, every crate's `src` | 12 forbidden key/override seams in `crates/admission/src`; a 7-token per-crate allowance table; whole-text pins on the `ACCEPTANCE_PUBLIC_KEY` declaration and on `verify_with_compiled_acceptance_key`; refuses any file declaring an item at file scope below its test module; recursive Clap command-tree scan | no count; fails if the walk never reached `crates/admission` |
| `cli_has_no_real_data_override` — `crates/cli/tests/cli.rs` | recursive, `cli/src` + `core/src` | 10 forbidden tokens; a 5-token lane-authority allowance table; whole-text pins on `posture_for_profile`, `ALLOWLISTED_FIXTURE_IDS`, `is_allowlisted`, the daemon-side allowlist `match`, and the whole `fn main` dispatch spine; file-scope-below-test-module rule; a 12-pair environment battery, a 14-argument flag battery and a 3-path ingest battery against the built binary | `scanned >= 18` (19 today) |
| `read_crate_sources` — `crates/recovery/tests/recovery_admission.rs` | recursive, `recovery/src` | three token lists across three named tests: 8 device-key-source tokens plus a per-line `DeviceKeystore` allowlist, 13 word-level tokens, 3 profile-default tokens | `>= 5`, plus a tripwire: a `mod name;` or `pub mod name;` on its own line must be a file the walk read |
| `read_crate_sources` — `crates/retention/tests/retention.rs` | recursive, `retention/src` | token lists at three call sites (revocation claims, `GATE-38-026` decision seams, journal truncation) | `>= 8` |
| `the_rotation_gate_is_one_decision_with_no_flag_variable_or_debug_path` — `crates/retention/tests/rotation_gate.rs` | none — three fixed paths | `WHOLE_GATE` whole-text pin on `require_rotation_accepted`; a 6-token list over its body; the first statement of each of 7 gated entry points | none |
| `default_feature_tree_has_no_conversion_entry_point` — `crates/store/tests/encrypted_profile.rs` | recursive, `store/src` | 3 forbidden conversion entry points; plus fixed-path reads of `src/lib.rs` for the compile-time guard, and byte scans of the built probe binary | none |
| `phase1_exit_has_no_product_network` — `crates/daemon/tests/phase1_exit.rs` | recursive, every crate's `src` except `test-support` | 10 networking tokens, paired with an independent link scan of the built default-feature `academicd` image for 8 symbol byte sequences | `scanned > 0` |
| `tools/secret-debug-policy.test.mjs` | recursive, every `crates/*/src` | regex over derive attributes against a registry of secret-carrying types | none on the file walk; a `>= 11` floor on the macro-generated key-type registry |
| `tools/phase1-scaffold-policy.test.mjs` | recursive for `store-platform/src`; fixed paths elsewhere | `cargo metadata` dependency graph, acceptance-receipt comparison, and regex/substring assertions on named files — including a second, independent copy of the rotation-gate decision-site count | none |
| `the_tombstone_row_calls_the_product_restore_and_lives_only_here` — `crates/portability/tests/encrypted_rotation.rs` | none — two fixed *test* source paths | substring: the acceptance row is in this file, calls the product restore, and has no second definition in `academic-retention` | none |
| `tools/verify-contracts.mjs` | recursive, `crates/contracts/src` | digest pins and byte-for-byte re-render; refuses any tree entry that is not a `.rs` file | n/a — an unreviewed entry fails |
| `tools/{source-preflight,cargo-lock-source-policy,dependency-source-policy,restricted-yaml}.mjs`, `tools/{dependency-source-policy,pnpm-source-policy-consumption}.test.mjs` | n/a | lockfile and registry parsing; not a source-text scan | n/a |
| `tools/{phase1-exit,security-baseline}.mjs` | n/a | execution observation and committed fixture bytes | n/a |
| `crates/store/tests/{api_boundary,sqlcipher_spike}.rs` | n/a | manifest text and scratch-crate compile-fail; not a source-text scan | n/a |

## What is pinned as whole text, and what changing it costs

Five items are compared against a constant rather than searched for tokens.
Editing one of them is intended to require editing its constant in the same
commit; that is the cost the pin buys, and it is why a pin is spent only where
a silent edit is the whole risk.

| Pinned item | Constant | Edited by |
|---|---|---|
| `ACCEPTANCE_PUBLIC_KEY` declaration | `WHOLE_ACCEPTANCE_KEY` | acceptance-key provisioning (`P2-H1`) |
| `verify_with_compiled_acceptance_key` body | `WHOLE_KEY_CHECK` | a change to how the compiled key is checked |
| `require_rotation_accepted` | `WHOLE_GATE` | a change to what decides rotation acceptance — not turning the feature on, which the gate already reads |
| `posture_for_profile` | `WHOLE_POSTURE_SOURCE` | a change to where the CLI's posture comes from |
| `ALLOWLISTED_FIXTURE_IDS`, `is_allowlisted`, the daemon-side allowlist `match`, `fn main` | `WHOLE_FIXTURE_ALLOWLIST`, `WHOLE_ALLOWLIST_PREDICATE`, `WHOLE_DAEMON_FIXTURE_GATE`, `WHOLE_DISPATCH_SPINE` | ADR-002 acceptance, or a new command |

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
| S-6 | `crates/portability/tests/encrypted_rotation.rs` | two fixed test-source paths, substring only | It checks that one acceptance row lives in one file. A third file could hold a third copy and nothing would see it. |

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
