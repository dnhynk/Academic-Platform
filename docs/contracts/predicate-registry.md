# Predicate and edge registry v1

## Purpose

The twenty §7.2 edges are the whole graph vocabulary. Their direction, admitted
node types, evidence floor, and qualifier schema are one versioned registry
rather than a rule per caller, so a path engine, an importer, and a projection
cannot disagree about what an edge means.

## The two files

| File | Role |
|---|---|
| `schemas/registry/predicate-registry-v1.json` | Single source of truth |
| `crates/domain/src/predicates/generated.rs` | Rust constants rendered from it |

Regenerate after any registry change:

```powershell
node tools/predicate-registry.mjs --write
```

`pnpm verify:contracts` renders again and compares byte-for-byte, so neither
file can be edited alone. The same check pins the registry to the canonical
design document, whose digest is already asserted there: `node_types` must equal
the §7.1 hierarchy leaves in order, and each entry's `spec_direction` and
`spec_meaning` must equal its §7.2 table cells verbatim. A specification edit
that moves an edge fails the build before it can reach a caller.

`registry_version` is `1`; every entry records the `since_registry_version` that
introduced it. Narrowing an edge is a registry change with a version bump, never
an undeclared rule inside a caller.

## Direction and cardinality

Each entry declares the §7.1 node types admitted on each end. An assertion whose
ends do not match is rejected, so the reverse of a type-asymmetric edge — the
majority of the twenty — is not constructible at all.

Every edge is `MANY_TO_MANY`. §7.2 constrains no edge to a functional arity, and
§7.3 makes every edge a scoped, valid-timed claim, so a functional constraint
would forbid legitimate re-assertion under a second scope or interval.

## Inverse is a view

Each entry carries an `inverse_label`, a reading for the user interface. There
is no inverse predicate and no reverse row: storing one would put the same fact
in the append-only ledger twice. `predicates::inverse_neighbours` derives the
inverse direction from the stored forward rows and is the only inverse read
path. `verify:contracts` rejects an `inverse_label` that is itself a registered
predicate name.

## `RELATED_TO`

`RELATED_TO` is the only `UNDIRECTED_CANONICAL` edge. `EdgeKey::new` stores its
ends smaller identifier first, so a pair asserted either way is one row. The
ordering is a storage canonicalisation over the opaque identifier bytes and
carries no semantic ranking.

`RELATED_TO` is not a prerequisite. `predicates::prerequisite_descriptor`
refuses it, which is the API a path engine must go through; it declares no
strength and no qualifier, so it cannot be dressed as one.

## Strength

`HARD`/`STRONG`/`HELPFUL` is carried only by the two prerequisite edges, and the
admitted sets differ:

| Edge | Admitted strengths |
|---|---|
| `REQUIRES` | `HARD`, `STRONG` |
| `BUILDS_ON` | `STRONG`, `HELPFUL` |

`REQUIRES` refuses `HELPFUL` because §7.2 forbids a preference from being stated
as a requirement. `BUILDS_ON` refuses `HARD` because that is what distinguishes
it from `REQUIRES`. `prerequisite` is true for exactly the edges with a
non-empty strength set.

## Minimum evidence

Every entry carries a base rule and, where strength changes it, a per-strength
override. A supporting item qualifies when its role is `SUPPORTS`, its strength
is at least `min_strength`, and its locator kind is in `locator_kinds` when that
list is non-empty. `independent_sources` counts distinct artifacts among the
qualifying items. `authority` constrains the assertion's own authority class,
not any single item.

The rules that are not the default `1` item from `1` source:

| Edge | Rule |
|---|---|
| `REQUIRES` at `HARD` | 2 `DIRECT` items from 2 distinct artifacts |
| `TAUGHT_IN` | 1 `DIRECT` item located in a transcript, text range, or page |
| `APPLIED_IN` | 1 `DIRECT` item; authority `USER_EXPLICIT` or `DIRECT_OBSERVATION` |
| `OBSERVED_IN_PROJECT` | 1 `DIRECT` repository-bytes item; authority `DIRECT_OBSERVATION` or `DETERMINISTIC_ENGINE` |
| `DESIGNED_TO_TEACH` | authority `OFFICIAL` or `CURATED` |
| `RESOLVES_QUESTION`, `EVIDENCED_BY` | no separate supporting item |

**A single-source `REQUIRES` edge at `HARD` strength is rejected.** The last two
rows carry no separate evidence requirement because the edge's own subject or
object *is* the evidence; demanding more would be circular.

## Qualifiers

Every qualifier schema is closed. An unknown key, a missing required key, a
duplicate key, and a value outside the declared domain are each rejected. Every
value is typed — a strength, a closed enumeration, an entity reference, or a
positive integer — so no structured value reaches free text (§2.3-3).

| Edge | Required qualifiers |
|---|---|
| `REQUIRES`, `BUILDS_ON` | `prerequisite_strength` |
| `ENABLES_COMPETENCY` | `contribution_importance` (`CRITICAL`/`SUBSTANTIAL`/`MINOR`), `necessity` (`NECESSARY`/`OPTIONAL`) |
| `RELEVANT_TO_ROLE` | `role_profile_version` |
| `REQUIRED_BY_PROJECT` | `failure_chain_ref` |
| `BENEFICIAL_TO_PROJECT` | `trigger_ref` |

`failure_chain_ref` and `trigger_ref` are entity references. Which aggregate
records a failure chain or a trigger belongs to the project-graph task; the
registry fixes only that it is a typed reference and never a sentence.

Every other edge declares an empty schema. `ASSESSED_IN` has no grade or score
qualifier on purpose: §7.2 keeps grade and mastery separate.

## Personal state

`personal_state_ceiling` is the highest mastery an edge alone may support.
`predicates::personal_mastery_ceiling` returns an error rather than a floor for
an edge that licenses no personal claim, so no caller can read a personal state
out of an edge that only describes the world.

| Edge | Ceiling |
|---|---|
| `MENTIONED_IN`, `TAUGHT_IN`, `ASSESSED_IN` | `EXPOSED` |
| `PRACTICED_IN` | `PRACTICED` |
| `APPLIED_IN`, `OBSERVED_IN_PROJECT` | `APPLIED` |
| every other edge | none |

A mention therefore cannot reach `UNDERSTOOD`, and it cannot be restated as
`TAUGHT_IN`: its object is a source segment, a teaching claim's object is a
lecture, and a weak mention does not satisfy `TAUGHT_IN`'s evidence rule.
`USED_IN` has no ceiling at all — where a concept is generally used is not
evidence that the user applied it.

## What stays open

`GATE-38-022`'s base taxonomy mix is unselected and is listed in the registry's
`open_gates` and in `predicates::OPEN_GATES`. This registry names predicates and
seeds no concept, field, or competency; which combination of ACM, curriculum,
and personal taxonomy forms the curated core is a user decision.

## Enforcement

- `cargo test -p academic-domain --test predicate_registry` — the named
  acceptance evidence, plus a field-by-field comparison of the generated
  constants against the registry file on every supported platform.
- `pnpm verify:contracts` — the byte comparison against a fresh render and the
  §7.1/§7.2 specification cross-check.
