# Bitemporal query surface and time travel

## Purpose

A personal record is read twice: as it was known at some point, and as it is
understood now about some point. Collapsing those into one "current" answer
mixes a past audit with a future plan. This contract fixes the query surface
that keeps them apart, the disposable snapshots that make it fast, and the
vocabulary that says whether a value moved because the user changed or because
the way the record is observed changed.

## The query API takes both coordinates

`academic_domain::temporal::TimeCoordinates` carries `known_at_accept_seq` and
`valid_at` together. It has no `Default`, no single-coordinate constructor, and
no "now" constructor, so a caller that has not decided both cannot build one.
Every entry point below takes that value:

| Entry point | Reads |
|---|---|
| `academic_store::timeline::aggregate_timeline_snapshot` | the eighteen migration 0004 aggregate closure tables |
| `academic_store::queries::projection_source_snapshot` | the resolved schema-1 claim lane |
| `academic_projections::bitemporal::TimelineStore::materialize` | both, into one snapshot |
| `academic_projections::query::ProjectionReader` | the Phase 1 graph and search generations |

`academic_projections::generation::ProjectionCoordinates` is a re-export of the
same type, not a second copy, so the canonical read and the sidecar read cannot
drift into differently shaped coordinates.

The two axes are separate SQL predicates on separate columns:

```sql
WHERE a.domain_id = ?1
  AND e.accept_seq <= ?2                                     -- as-known-at
  AND a.valid_from <= ?3 AND (a.valid_to IS NULL OR a.valid_to > ?3)  -- valid-at
```

Neither can stand in for the other. `aggregate_timeline_excludes_knowledge_accepted_later`
fails if the first predicate is defeated and
`aggregate_timeline_valid_at_reinterprets_history` fails if the second is.

A known-at coordinate past the canonical head is refused
(`QueryError::KnownAtBeyondHead`), never clamped to the head. Clamping would
answer a question the caller did not ask.

## Absence is not emptiness

Three refusals exist so that "there is nothing here" is never confused with
"this cannot be recorded here":

| Refusal | Means |
|---|---|
| `QueryError::AggregatesAbsent` | the profile carries no aggregate closure tables at all — a schema-1 profile |
| `TemporalError::AggregateLaneAbsent` | the dimension has a carrier, but this profile holds no aggregate lane |
| `TemporalError::DimensionNotCarried` | no canonical carrier for this dimension is landed yet |

A snapshot also records `aggregate_lane` as `PRESENT` or `ABSENT` rather than
implying it from a zero row count.

## Snapshots are disposable and are not the ledger

The materialized snapshot lives in a third SQLite sidecar with its own
`application_id` (`ACTL`, `0x4143544C`), beside the Phase 1 graph and search
sidecar under `projections/`. The canonical store is opened read-only.

The sidecar tables carry **no** `guard_<table>_update` / `guard_<table>_delete`
trigger pair, and the product connection's canonical authorizer does not cover
them. That is deliberate and is the opposite of the canonical rule: those two
layers exist to make history append-only, and a snapshot that could not be
deleted would have become a second ledger. `TimelineStore::discard` removes the
whole file, and reopening recreates an empty one.

`snapshot_deletion_and_rebuild_preserves_ledger` runs that end to end against a
real profile: it hashes the canonical database file, materializes two
snapshots, discards the sidecar, rebuilds both, and asserts the canonical bytes
and the canonical row counts are unchanged and the rebuilt snapshots are equal
to the originals. Snapshot identity is derived from
`(security_domain, coordinates, projector_version)` rather than drawn at random,
which is what makes that comparison possible.

Nothing in this sidecar is exported or backed up as truth.

## Every snapshot records what produced it

| Column | Identifies |
|---|---|
| `projector_version`, `projector_binary_digest`, `projector_config_hash` | the code and configuration that ran |
| `source_ledger_digest` | the canonical source-ledger authority at the known-at coordinate |
| `source_row_digest` | the canonical input the reading was bound to: the coordinates, the aggregate rows visible at them, and the ledger digest |

`source_row_digest` commits to no projector output. `MaterializedSnapshot::content_digest`
commits to output only — including the applied predicate policy, which is a
projector decision rather than a canonical fact. So two readings that agree on
`source_row_digest` and disagree on `content_digest` disagree because the
projector changed, and `explain_recomputation` reports
`ChangeOrigin::AnalyzerUpgrade` on that basis rather than on an assumption. Two
readings that agree on both have nothing to explain and say
`TemporalError::UnexplainedTransition` instead of inventing a cause.

## Change origin

```text
EVIDENCE_CHANGE              the record moved because evidence about the subject moved
ONTOLOGY_CHANGE              an identity merge or split moved what the value attaches to
ANALYZER_UPGRADE             the projector that computed the value changed version
OFFICIAL_SOURCE_CORRECTION   an official source superseded an earlier official statement
```

Only `EVIDENCE_CHANGE` means the user changed. The other three are changes in
the observation system, which is the distinction the specification's temporal
model requires and which `ChangeOrigin::is_observation_system_change` exposes.

The visualization section names a third transition kind, `user scope change`,
alongside ontology and evidence change. It is **not** an arm here, and its
absence is not an oversight: changing which scope is displayed changes what a
viewer is shown, not what the record says. It belongs to the view that owns the
scope filter, and putting it here would let a display setting be recorded as a
change in canonical history.

### One origin per transition, by splitting rather than by ranking

`TransitionCause` has one flag per origin-bearing input. `TransitionCause::label`
returns an origin only when exactly one flag is set; a cause with two returns
`TemporalError::AmbiguousOrigin` and one with none returns
`TemporalError::UnexplainedTransition`. There is no precedence order, because a
precedence order would silently attribute a mixed interval to one cause.

What makes labelling total anyway is that the interval is split instead.
`academic_store::timeline::origin_marks` reads the ledger for the
origin-bearing acceptances in `(after, through]`:

| Mark | Read from |
|---|---|
| ontology change | `entity_identity_change` joined to its `ledger_event` |
| official-source correction | a `SUPERSEDES` `claim_relation` authored by an `IMPORTER` where both claims are `OFFICIAL_CONFIRMED` |
| other evidence | every other acceptance in the interval |

One event carries one payload arm, so those three sets are disjoint by
construction. `origin_pure_steps` turns each mark into one step with one cause
and appends the analyzer step for the one axis the ledger cannot record — the
projector version. `explain_transition` then labels every step.

`OriginMarks::identity_lane_present` is `false` on a schema-1 profile: an empty
identity-change list there means the profile cannot record one, not that none
happened.

## Comparison across an ontology change

A comparison that crosses an identity change is governed by the four-class
equivalence contract in [the entity registry](entity-registry.md), not by this
surface. `ONTOLOGY_CHANGE` labels the transition; it does not license a delta.
`INCOMPARABLE` and `SPLIT_AMBIGUOUS` nodes still carry no delta, and a growth
narrative still reports what it refused to count.

## The named dimensions

The design document's temporal model names fifteen time-travel targets across
seven bullets. `NAMED_TIME_TRAVEL_DIMENSIONS` is that list, in that order, with
`TimeTravelDimension::spec_name` holding each target's exact words and
`spec_bullet` recording which bullet it came from.

`time_travel_covers_all_fifteen_named_dimensions` reads the design document
itself, removes each `spec_name` from its bullet, and requires what remains to
be punctuation. Dropping an arm, or paraphrasing one, leaves text behind and
fails.

Four of the fifteen have a landed canonical carrier among the eighteen event
schema v3 registration arms:

| Dimension | Carrier |
|---|---|
| `COURSE_ATTEMPT` | `ATTEMPT_RECORDED` |
| `DEGREE_AUDIT_VERSION` | `AUDIT_COMPUTED` |
| `PROJECT_SNAPSHOT` | `SNAPSHOT_REGISTERED` |
| `FINDING_CLASSIFICATION` | `FINDING_PUBLISHED` |

The other eleven are `DimensionCarrier::NotYetCarried` and are refused rather
than answered with an empty page. Landing a carrier is a one-line change to
`TimeTravelDimension::carrier`, and
`every_declared_carrier_is_a_real_v3_registration_arm` rejects a carrier that is
not one of the eighteen arms.

### Renamed acceptance test

The T068 Phase 2 plan names this task's coverage test
`time_travel_covers_all_thirteen_named_dimensions`. It is committed here as
`time_travel_covers_all_fifteen_named_dimensions`. The reason is that the design
document's temporal model enumerates fifteen named targets, not thirteen, and
the plan's count differs from it. Keeping the plan's name would have meant
either a name that asserts a false count or a list that leaves two of the
document's targets uncovered. An auditor searching for the plan's name will find
it in this paragraph.

## Which lane proves what

Store schema version 2 — and therefore the aggregate closure tables — is created
only by the encrypted lane, and the plaintext lane's reader admission refuses a
database carrying objects its migration set does not create. The evidence is
therefore split, and both halves run in the default `cargo test --workspace`:

| Suite | Base | Proves |
|---|---|---|
| `academic-store` `aggregate_timeline_tests` | schema-2 core plus migration 0004, read through a plain read-only connection | the coordinate SQL over real closure tables, coverage of all eighteen arms, the two refusals, and the ontology mark |
| `academic-core` `bitemporal_time_travel` | a real synthetic profile with the real acceptance path and the real sidecar | the caller-facing surface: both coordinates, snapshot disposability, recomputation attribution, one origin per transition, and every named dimension answering |

The profile-admission path for a schema-2 profile is the encrypted lane's and is
verified there; see [the encrypted store lane](encrypted-store-lane.md).
