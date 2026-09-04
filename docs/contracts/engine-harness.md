# Deterministic engine harness

## Purpose

Every §28 engine is a pure function

```text
(frozen_inputs, rule_set_hash, engine_version) -> (result, proof_tree, explanation_snapshot)
```

with no clock, no RNG, no network, no model, and no ambient state. Twelve
engines decide credit, graduation, deletion, and egress, so the answer to "did
this run produce the same result as the last one" has to be a byte comparison
rather than an absence of errors. This harness fixes the signature, the
proof-tree node shape, the explanation normalization, and the CI enforcement
that a registered engine ships evidence before it ships behaviour.

## The two files

| File | Role |
|---|---|
| `schemas/registry/engine-registry-v1.json` | Single source of truth |
| `crates/domain/src/engines/generated.rs` | Rust constants rendered from it |

Regenerate after any registry change:

```powershell
node tools/engine-registry.mjs --write
```

`pnpm verify:contracts` renders again and compares byte-for-byte, so neither
file can be edited alone. The same check pins the registry to the canonical
design document, whose digest is already asserted there: the registered engines
must be exactly the §28 table rows, in table order, each with `spec_row` equal
to its four table cells verbatim. The comparison is an enumeration, not a count,
so a specification edit that renames or drops an engine and a registry edit that
adds one the table does not name fail the same assertion.

`registry_version` is `1`; every entry records the `since_registry_version` that
introduced it.

## The twelve engines

The registry is the §28 table and nothing else.

| Engine | Requirement | High-impact path |
|---|---|---|
| `GPA` | `REQ-28-001` | `GPA` |
| `CREDIT_ACCOUNTING` | `REQ-28-002` | — |
| `GRADUATION_AUDIT` | `REQ-28-003` | `GRADUATION` |
| `TIMETABLE` | `REQ-28-004` | — |
| `OFFICIAL_PREREQUISITE` | `REQ-28-005` | — |
| `EQUIVALENCY` | `REQ-28-006` | — |
| `TRANSCRIPT_COVERAGE` | `REQ-28-007` | — |
| `ARTIFACT_INTEGRITY` | `REQ-28-008` | — |
| `REPOSITORY_DIFF` | `REQ-28-009` | — |
| `OVERRIDE_RESOLVER` | `REQ-28-010` | — |
| `PERMISSION_BROKER` | `REQ-28-011` | `EGRESS` |
| `RETENTION_DELETION` | `REQ-28-012` | `DELETION` |

The high-impact four come from the §28 closing paragraph, which names the paths
that must be tested beyond a successful computation as money, graduation,
deletion, and external transmission. `GPA` carries money because it is the
engine a grade, a repeat decision, and every downstream credit consequence rest
on. `EGRESS` belongs to the permission broker because it is the only registered
engine whose output governs whether data may leave the device: its inputs are
the data class, the purpose, the destination, and the consent, and its output is
the allow/deny decision plus the audit row.

Which Phase 2 task implements each engine is fixed by the task catalog, not by
this registry.

### The count is twelve, not thirteen

t068 §3.9 and its `P2-C5` entry call this a "thirteen-engine registry". **That
number is wrong and this registry does not follow it.** §28 tabulates twelve
engines; the thirteenth t068 implies is the property sentence printed under the
table — that a published rule executes deterministically even when a model
extracted the candidate. That sentence names no input, no output, and no
invariant of its own, and the twelve engines below the table are the subjects it
is about, so counting it as a thirteenth engine counts them twice.

`REQ-28-013` states that same property. It is therefore an obligation on rule
publication (`P2-U2`) and on every engine here, not an engine of its own, even
though `t001`'s `REQ-28-014` row resolves "every deterministic engine" to
`REQ-28-001`…`REQ-28-013`. The registry closes `REQ-28-001`…`REQ-28-012`.

The same class of error appears independently in t068 §31.3, which says
"thirteen named dimensions" where the specification names fifteen. **Treat every
count in t068 as derived and unverified; the specification is the source of
truth.** `engine_registry_is_complete` compares against the §28 table itself for
exactly that reason, and its name is kept as t068 spells it because the name
carries no count.

## Registration is not implementation

Four entries are `IMPLEMENTED` and the rest are `PLANNED`. The four are
enumerated rather than counted, because a count would let one flip forward while
another flipped back and stay silent -- which is the shape this page warns about
for the registry itself:

| Engine | Task | Crate | Contract |
|---|---|---|---|
| `GPA` | `P2-U4` | `academic-record` | [GPA and attempts](gpa-and-attempts.md) |
| `CREDIT_ACCOUNTING` | `P2-U4` | `academic-record` | [GPA and attempts](gpa-and-attempts.md) |
| `GRADUATION_AUDIT` | `P2-U3` | `academic-audit` | [the graduation audit](graduation-audit.md) |
| `TRANSCRIPT_COVERAGE` | `P2-L4` | `academic-lecture-document` | [the lecture document](lecture-document.md) |

Each carries the artifacts under `testdata/engines/` that flipping requires, and
`GPA` and `GRADUATION_AUDIT` -- the two that decide a high-impact path -- carry
all three adverse fixture sets beside them.

The audit enforces both directions:

- **`PLANNED`** — the engine has no harness artifacts under `testdata/engines/`
  and no workspace source names its `engine_id`. Either one appearing is a
  violation, because an engine's proof tree, explanation snapshot, and audit
  rows are all keyed by that identifier, so an implementation cannot exist
  without naming it.
- **`IMPLEMENTED`** — all four artifact classes are present and non-empty, and
  a high-impact engine additionally carries all three adverse fixture sets.

Flipping an entry is therefore the moment its harness obligations become due.
A task that ships an engine and forgets its fixtures fails
`planned_engine_that_gains_an_implementation_fails_ci`; a task that flips the
entry without the fixtures fails the four `engine_without_*_fails_ci` tests.

### What the audit cannot do, and who does it instead

The audit counts artifacts. It cannot *run* a real engine's fixtures, because
every engine crate depends on `academic-domain` and this audit lives in it, so
a fixture that only exists would satisfy the audit and prove nothing. The
executing half belongs to the implementing crate.

Each live engine's implementing crate carries that half:
`crates/record/tests/record_harness.rs`,
`crates/audit/tests/audit_harness.rs`, and
`crates/lecture-document/tests/lecture_document_harness.rs`. Each evaluates
every committed `.input` against the real engine and byte-compares the
`.expected`, requires each adverse fixture to land on the outcome its directory
names, and re-renders the whole corpus from the deterministic builder so a
fixture cannot be hand-edited into agreement with a broken engine. **An engine
that flips to `IMPLEMENTED` without that second half has satisfied the audit and
demonstrated nothing.**

`GPA` and `GRADUATION_AUDIT` each carry a third half as well: an independent
oracle in another language, rendering the expected values from a second
transcription of the corpus, so that what a golden fixture is compared against
did not come from the engine it checks. `tools/gpa-oracle.mjs` and
`tools/graduation-audit-oracle.mjs` are those, and each is committed and
re-derivable.

## Harness layout

Every engine's artifacts live under `testdata/engines/<harness_dir>/`, where
`harness_dir` is the registry name in lower case. Nothing else may sit under
that root.

| Class | Path | Content |
|---|---|---|
| `GOLDEN_FIXTURES` | `golden/` | `<case>.input` plus `<case>.expected` canonical bytes |
| `PROPERTY_TESTS` | `property/` | the generator bounds the property test is driven from |
| `VERSION_COMPAT_FIXTURES` | `version-compat/` | `<case>.input` plus the `<case>.explanation` every admitted version must still produce |
| `EXPLANATION_SNAPSHOT` | `explanation.snapshot` | the normalized explanation of one representative case |

A high-impact engine adds `adverse/unknown/`, `adverse/conflict/`, and
`adverse/partial_failure/`, each with executable `<case>.input` and
`<case>.expected` pairs. `high_impact_engines_cover_unknown_conflict_partial`
runs them and asserts each one lands on the outcome its directory names, so an
adverse fixture cannot be a file that merely exists.

`ruleset.txt` in the harness directory is the published rule set; its SHA-256 is
the `rule_set_hash` every case is evaluated under.

## Proof tree

```text
{node_id, rule_id, status, inputs[], source_locators[], children[]}
```

`status` is the fixed five-value set `SATISFIED`, `NEEDS`, `NOT_SATISFIED`,
`UNKNOWN`, `CONFLICT`. `ProofNode::validate` rejects a duplicate `node_id`
anywhere in the tree, an `inputs` entry the frozen input set does not declare,
an unordered or duplicated `inputs`/`source_locators`/`children` list, and an
invalid span. Ordering is not cosmetic: it is what makes the canonical bytes a
function of the tree rather than of the walk that produced it.

`EngineOutcome::new` additionally rejects a `SATISFIED` result over a tree
containing a `CONFLICT`. It does **not** impose a fold from children to parent.
§11.2 has rule types — `AT_LEAST_N_OF`, `COUNT_WITH_CONSTRAINTS` — where a
parent is legitimately satisfied over unsatisfied children, so how a node
derives its own status belongs to the engine that owns the rule. The graduation
audit's "unknown is never forced into a pass or a fail" is `P2-U3`'s invariant
over its own rules, expressed in this vocabulary.

`InputValue::Unknown` is a value, not a missing key. An official fact nobody has
confirmed and a §38 gate the user has not answered are both *known to be
unknown*, and folding either into a default would manufacture a verdict.

## Frozen inputs

The canonical encoding is one `key=value` line per input, LF-terminated, with
keys in strictly ascending order:

```text
reference.source.a=ref:registrar.v1
reference.threshold=int:60
reference.value=unknown
```

Values are `int:<i64>`, `dec:<coefficient>/<scale>`, `ref:<identifier>`, or
`unknown`. Identifiers are ASCII alphanumerics, `.`, `_`, and `-`, at most 128
bytes: the encoding separates fields with `=`, `:`, and newline, so a value that
could contain one would make the byte comparison meaningless. There is no free
text, which is why no structured value can be smuggled through one (§2.3-3).

`FrozenInputs::parse` returns a typed `EngineError` for every malformed input —
a missing separator, a non-canonical integer spelling, an out-of-range decimal
scale, a duplicate or unordered key, an unknown type tag. It never panics, which
is `§2.3-11` for the input boundary.

## Explanation snapshot

`ExplanationSnapshot::render` is total and locale-free: LF endings, two spaces
of indentation per proof depth, statuses spelled as the contract spells them, no
trailing whitespace, and nothing time- or host-dependent. `EngineOutcome::new`
renders it from the result and the tree rather than accepting one, so an
explanation cannot disagree with the outcome it explains.

## Byte equality

`EngineOutcome::canonical_bytes` binds the engine id, the engine version, the
rule-set hash, and the frozen-input digest to the normalized explanation. Two
evaluations agree when those bytes agree.
`same_inputs_and_rule_hash_yield_byte_equal_results` asserts that, and asserts
that the same inputs under a *different* rule-set hash produce different bytes
with an identical result — without that second half the first would pass on an
encoding that ignored the hash.

## No clock, RNG, network, or model

Enforced in the two halves §2.3-14 already establishes for a capability.

**Available.** `engine_source_contains_no_clock_rng_network_or_model` pins the
`academic-domain` product closure to its exact reviewed crate set. No clock,
network, or model crate is in it. `getrandom` *is*, because §2.3-18 admits it
for a synthetic nonce and locator seed and `uuid`'s `v7` feature reaches it; the
test therefore asserts `uuid` is its only owner rather than pretending the
capability is absent. The closure is resolved with `--target all`, not for the
host: `getrandom` reaches `libc` on Linux and not on Windows, so a host-resolved
list would be one platform's claim asserted on every runner.

**Used.** The same test scans the engine sources the registry accounts for —
today the harness module, its generated registry, and the reference engine — for
the API spellings of a clock, an RNG, a socket, and a model call, including
`Uuid::now_v7` and `Uuid::new_v4`, which are how `getrandom` would be reached.
An entry that flips to `IMPLEMENTED` without adding its source to that scan
fails the test rather than leaving its implementation unscanned.

Test dependencies are outside both halves on purpose. A property test generates
inputs; the engine still consumes only the frozen ones it is handed.

## The reference engine

`same_inputs_and_rule_hash_yield_byte_equal_results` needs an engine to run
twice, and the audit's `IMPLEMENTED` branch needs a complete artifact set or the
guard would never be observed to bite. `Reference` in
`crates/domain/tests/engine_harness.rs` is that engine, with its corpus in
`testdata/engine-harness-reference/`. It is test-only, ships in no product
build, is deliberately not one of the twelve, and
`reference_engine_is_not_registered` proves it. Its corpus is the worked example
a real engine's harness directory copies.

## Enforcement

- `cargo test -p academic-domain --test engine_harness` — the named acceptance
  evidence, on every supported platform.
- `cargo test -p academic-record --test record_harness`,
  `cargo test -p academic-audit --test audit_harness` — the executing half for
  the live engines, inside `cargo test --workspace`.
- `pnpm test` — `engine_source_contains_no_clock_rng_network_or_model`, the
  source and dependency scan.
- `pnpm verify:contracts` — the byte comparison against a fresh render and the
  §28 specification cross-check.
