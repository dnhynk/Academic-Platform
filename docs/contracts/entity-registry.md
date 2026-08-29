# Entity registry and ontology-migration equivalence

## Purpose

An ontology changes; a personal history must not. This contract fixes how a
stable identity carries aliases and senses, how merge and split are recorded
without touching evidence, and how a comparison that crosses an ontology change
reports what it is allowed to conclude.

It closes `GATE-38-030` with a technical fact: the required migration
equivalence is the four-class contract below, proved by replaying a golden
multi-year history across a major ontology change and showing the pre-change
canonical bytes unchanged.

## Where the registry is stored

The registry has no storage of its own. It is folded from two canonical
sources, both of which are signed.

| Source | Supplies |
|---|---|
| `entity_identity_change` (migration 0004) | one anchor per `ENTITY_IDENTITY_CHANGED` event: the entity whose identity changed, its domain, its scope, and the interval the change is effective over |
| `claim` with a registry predicate | the typed detail: tier, label, sense, alias metadata, redirect, split successor, reclassification queue entry |

The eighteen event schema v3 arms are registration depth. Their payload is
exactly `id / parent / domain_id / scope_id / source_digest / valid_time`, so a
registration arm has nowhere to carry a redirect target or an alias language.
Typed detail therefore arrives as `CLAIM_ASSERTED`, which is the mechanism
migration 0004 already names for disputable facts. Nothing about an identity
change is unsigned.

`EntityRegistry::build` rejects a registry claim whose subject has no anchor.
The 0004 closure row is the registry's admission control, not decoration.

## Registry predicates

Twelve predicates, all typed, none carrying a structure inside `object_text`.

| Predicate | Object kind | Meaning |
|---|---|---|
| `identity.entity.kind` | `TEXT` | ontology tier: `FIELD`, `CONCEPT`, `CONCEPT_SENSE`, `OPERATION`, `ALIAS` |
| `identity.entity.label` | `TEXT` | canonical label |
| `identity.entity.label.language` | `TEXT` | language of the canonical label |
| `identity.sense.of` | `ENTITY` | the ambiguous concept a `CONCEPT_SENSE` disambiguates |
| `identity.alias.of` | `ENTITY` | the entity an alias names |
| `identity.alias.text` | `TEXT` | surface form |
| `identity.alias.language` | `TEXT` | language tag of the surface form |
| `identity.alias.kind` | `TEXT` | `PREFERRED`, `ABBREVIATION`, `TRANSLATION`, `VERSIONED` |
| `identity.alias.version` | `TEXT` | the release a versioned name applies to |
| `identity.merged.into` | `ENTITY` | redirect from a merged-away identity to its survivor |
| `identity.split.into` | `ENTITY` | one successor a split produced |
| `identity.reclassification.pending` | `TEXT` | one evidence item queued after a split; it has not moved |

**An alias is an entity.** Language, kind, and version have to stay bound to one
surface form, and a claim carries one object. Giving the alias its own
identifier keeps the four facts joined by subject instead of by a blob, and
leaves the closed nine-value `object_kind` enum untouched.

## Identity actions

**Merge is non-destructive.** `identity.merged.into` adds a redirect. The
merged-away identifier is not deleted, still resolves, and every evidence link
that named it still names it. `resolve_identity` follows the redirect chain to
the surviving identity.

**Split moves nothing.** `identity.split.into` names successors and
`identity.reclassification.pending` enqueues each affected evidence item. No
evidence is reattached, no claim subject is rewritten, and each queue entry
carries the successors as candidates for a later decision. Redistributing
evidence automatically is what silently rewrites a personal history, so the
registry has no code path that does it.

**A mention that cannot be decided stays a mention.** `resolve_mention` matches
surface form and language exactly. One match resolves. More than one match
resolves only if context names exactly one candidate; anything else returns the
candidate set unresolved. There is no ranking, no normalisation heuristic, and
no default sense.

**Both actions are non-delegable.** A merge or split claim must be
`USER_CONFIRMED`, which pairs only with `USER_EXPLICIT` authority, which the
canonical writer accepts only from a user actor. A model run, an importer, or a
deterministic engine cannot reach a merge at all.

## Impact preview and approval

Before an approval exists, `ImpactPreview::compute` counts the knowledge states,
graph edges, open questions, and evidence items attached to **every** identity
the proposal touches — both sides of a merge, the source and all successors of a
split. The registry owns none of those, so it reads their per-entity counts from
an `OntologyImpactSnapshot` supplied by the projections that do.

`ImpactPreview::canonical_bytes` is a fixed-order rendering with split
successors sorted, and `ImpactPreview::digest` is SHA-256 over it. **The
approving claim cites an evidence item whose excerpt digest is that value.**
A claim with no evidence is already rejected by `Claim::validate`, so an
approval recorded against counts nobody was shown fails to verify rather than
passing quietly.

## Equivalence classes

Comparing a pre-change node to a post-change node produces exactly one class.

| Class | When | Comparison |
|---|---|---|
| `IDENTICAL` | same identity, untouched by any identity change | permitted |
| `REFINED` | a redirect chain leads from the earlier identity to the later one, which covers at least as much | permitted |
| `SPLIT_AMBIGUOUS` | the earlier identity was split; no single successor inherits its state until the reclassification queue is decided | withheld |
| `INCOMPARABLE` | no justified correspondence in either direction, or a correspondence that would be permitted but whose other side was never observed | refused |

A split dominates a merge. If the earlier identity was split, the class is
`SPLIT_AMBIGUOUS` even when a redirect also exists, and it stays
`SPLIT_AMBIGUOUS` when nothing was observed on the other side, because the
split is the more informative reason the comparison is refused. Attributing an
earlier state to one successor is the distortion this contract prevents.

`StateComparison.delta` is `Some` exactly when
`equivalence.permits_comparison()`. A caller cannot obtain a number across an
ontology change without holding the class that licenses it, so "silently
compares `INCOMPARABLE` nodes" is not a thing the API can express.

`GrowthNarrative` counts only `IDENTICAL` and `REFINED` rows and returns the
rest in `excluded_incomparable` and `withheld_split_ambiguous`. Both lists are
part of the result rather than a log line, because a narrative that quietly
dropped nodes would read as complete.

## Named acceptance evidence

`crates/store/src/entity_registry_tests.rs` runs every one of these against a
real store — migration `0001`, `0003`, and `0004`, written through the
acceptance closure writer and sealed against a real vault.

| Test | Proves |
|---|---|
| `merge_preserves_ids_and_redirects` | both identifiers resolve, the redirect exists, no evidence link is rewritten |
| `split_creates_reclassification_queue_and_moves_nothing` | every affected evidence item is queued and none is reattached to a successor |
| `ambiguous_mention_abstains` | three senses share a surface form; without deciding context the resolver returns candidates, and a tie is still ambiguous |
| `homonym_split_keeps_evidence_separate` | each sense is a `CONCEPT_SENSE` of the ambiguous concept and holds evidence disjoint from it and from the others |
| `ontology_change_preview_shows_state_edge_question_counts` | the four counts are computed before approval and the approval cites their digest |
| `historical_state_is_not_silently_distorted` | the change appends; every pre-change ledger row is byte-identical afterwards, the pre-change states replay identically, and the canonical tables refuse the mutation that would be needed otherwise |
| `equivalence_class_is_reported_for_every_cross_change_comparison` | every row on either side carries a class, and delta presence and class agree |
| `incomparable_nodes_are_excluded_from_growth_narratives` | the narrative counts three rows, withholds the split source, and reports four excluded nodes |

The golden history is four synthetic years of mastery observations over four
concepts in one batch, followed by a second batch that merges a duplicate
concept into its survivor and splits an ambiguous concept into three senses. The
change is a separate batch so the pre-change bytes belong to a batch that is
already closed when it is accepted.

Every byte is synthetic. No fixture here derives from a personal record, a
lecture, a private repository, or an external fetch.

## What this does not decide

The registry supplies the comparison frame, not the knowledge model.
`ObservedState` is a mastery value attached to an identity at a valid-time
instant; facets, freshness, and evidence ceilings belong to the knowledge-state
task. `OntologyImpactSnapshot` is read, not owned: the projections that hold
states, edges, and questions supply the counts.
