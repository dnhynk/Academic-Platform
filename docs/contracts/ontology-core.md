# Ontology core, taxonomy import, and curator workflow

## Scope and ownership

This contract implements the §7.4 granularity policy on top of the canonical
[entity registry](entity-registry.md). It does not create a second identity
system. Multilingual and abbreviation aliases, `ConceptSense`, abstaining
mention resolution, redirect-based non-destructive merge, queue-only split,
and the four migration-equivalence classes remain owned by that registry.

The ontology import layer owns only three primary node types:

| Type | Meaning | Required parent |
|---|---|---|
| `Field` | broad cluster such as Database Systems | none |
| `Concept` | independently explainable or linkable knowledge unit | `Field` |
| `Operation` | named procedure such as B+ Tree node split | `Concept` |

They are distinct Rust types and `TaxonomyNode` variants. Import validation
rejects duplicate identities, a concept without an imported field parent, and
an operation without an imported concept parent. Each variant maps to the
existing `EntityKind` vocabulary rather than extending or copying it.

## Taxonomy version identity and the open mix

`TaxonomyVersionIdentity` is the tuple of stable taxonomy-family ID, source
class, non-empty release identifier, and SHA-256 digest of canonical imported
nodes. `VersionedTaxonomyImport::with_identity` recomputes the digest and
rejects changed nodes under an old identity. Node order does not affect the
digest; node kind, identity, parent identity, and exact label do.

The three source classes—ACM, curriculum, and user-derived—are choices, not a
precedence order. `TaxonomyMixSelection` requires an explicit non-zero
configuration version and one or more exact taxonomy version identities. It
has no `Default` implementation. The product state may remain
`BaseTaxonomyMix::Unselected`; this task selects no source and no combination.

Therefore `GATE-38-022` is only partially discharged. User-only curator
authority is a technical fact below, while the base taxonomy mix remains open.

## Promotion and granularity state

A raw term is a `Mention`, not an entity-registry concept. Promotion has two
independent gates:

1. At least two distinct evidence occurrences must name the term. A single
   occurrence remains `MENTION` even if every attachment below is present.
2. At least one independent explanation, question, evidence item, or
   prerequisite attachment must exist.

Passing both gates produces a `ConceptCandidate` in
`GRANULARITY_UNDER_REVIEW`; it does not produce a curated concept. The exact
candidate bytes include its proposed identity, field, label, occurrence IDs,
and typed criteria. A user approval must cite their digest before the candidate
can become `CURATED`.

The normative examples are executable constants:

| Label | Type | Parent |
|---|---|---|
| Database Systems | `Field` | — |
| Serializability | `Concept` | Database Systems |
| B+ Tree | `Concept` | Database Systems |
| B+ Tree node split | `Operation` | B+ Tree |

## Impact preview and user-only approval

`OntologyChangeReview` delegates merge/split validation and impact counting to
the entity registry's `OntologyChangeProposal`, `OntologyImpactSnapshot`, and
`ImpactPreview`. It wraps those counts with the exact taxonomy version and
resolution scope before hashing the bytes shown for approval. Consequently an
unversioned registry preview, stale counts, a different scope, or the same
counts under another taxonomy release cannot authorize the change.

The action methods accept `VerifiedCuratorApproval`, not an `Actor` or a raw
`Claim`. Its fields are private and it is neither clonable nor serializable.
The only constructors call ADR-003's existing `Claim::validate_for_actor` and
then require all of the following:

- actor is `Actor::User`;
- authority/status is `USER_EXPLICIT` / `USER_CONFIRMED`;
- predicate/object is `ontology.curator.approved` / `APPROVE`;
- subject is the exact candidate or change source;
- claim scope is the exact review scope;
- the claim cites the evidence item whose excerpt digest is the exact review
  digest.

Every automatic actor variant fails the existing actor/authority/status matrix
when it attempts the user pairing. Each automatic actor's own valid pairing is
also rejected as a curator action. The clean user pairing is then admitted and
is the only path that can produce the typed token consumed by `approve`.

## Content-free quality metrics

The public metrics result contains and serializes only `orphan_count` and
`near_duplicate_pair_count`. The observation boundary accepts only count
values. A producer that supplies a label, term, identity list, or duplicate
pair as `Content` is rejected, and both the observation's `Debug` output and
the rejection error redact the attempted content so diagnostics are not a
second leak path.

## Named acceptance evidence

`cargo test -p academic-domain --test ontology_core` executes:

| Test | Evidence |
|---|---|
| `field_concept_separation` | the three types remain distinct and wrong-tier parents are rejected |
| `concept_granularity_gate` | each one-of-four attachment admits review, while an unsupported repeated term remains a mention |
| `single_mention_abstention` | one occurrence stays a mention even after all four attachments are injected |
| `granularity_examples_contract` | the four normative labels, tiers, and parents remain exact |
| `ontology_change_preview_gate` | unversioned, stale, and cross-version preview evidence fails before the current versioned digest passes |
| `orphan_and_near_duplicate_metrics_do_not_expose_content` | content-bearing variants for both metrics fail, diagnostics remain redacted, and restored count-only observations pass |
| `curator_approval_is_a_non_delegable_user_action` | forged user claims and native automatic-actor pairings fail for model, importer, and deterministic engine before a user approval passes |

`taxonomy_import_is_versioned_and_base_mix_remains_unselected` additionally
proves that changed content cannot reuse a version identity and that this task
does not select the open base taxonomy mix.

All fixtures are synthetic and every operation is local and deterministic.
