# Curriculum, course, revision and offering aggregates

`academic-curriculum` is `P2-U1`. It holds section 8.2's three aggregates, the
durable course identity they hang from, and section 11.4's independent
relations. It persists nothing, opens nothing, and runs no connector: migration
`0014` holds the typed rows and `crates/store` owns them.

## The boundaries are absences

Section 9's table gives each aggregate one row saying what it does **not**
contain.

| Aggregate | Excludes, in section 9's words |
|---|---|
| `Course` | 특정 교수·학기·시간표·실제 설명 |
| `CourseRevision` | 특정 분반의 현실 |
| `CourseOffering` | 매 수업시간의 실제 발화 |

Those are fields that do not exist and setters that were never written. Each
`*Draft` is the only route to its aggregate, has private fields, and declares no
method that takes an excluded value, so the three cases in
`crates/curriculum/tests/compile_fail/` fail with `E0599` — *no method named …
found* — rather than with a run-time refusal. There is nothing to reject.

`the_forbidden_fields_are_the_specifications_own` is the half that says the list
is the specification's own rather than a list written twice. It reads section
8.2's four yaml blocks out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, requires every key to map
to exactly one Rust accessor **in that aggregate's own `impl` block**, and then
sweeps every mapped accessor against every other aggregate's whole module. The
per-`impl` reading is load-bearing: injection `U-I5` renamed
`CourseRevision::source_snapshot` and a per-file name set still passed, because
`CourseRevisionDraft::source_snapshot` spelled the same name one type over.

Five accessor names appear on more than one aggregate and each carries its
written reason in `SHARED_ACCESSORS`: `id`, `code`, `valid_time`, `credits`, and
`source_snapshot` — the last two because section 8.2 writes `courseCode` on a
course and prints it on a revision, and writes `sourceSnapshot` on both
`CurriculumVersion` and `CourseRevision`.

The offering's excluded list is section 12.4's `TranscriptSegment` block, read
out of the specification the same way. That is where the specification writes
down what 매 수업시간의 실제 발화 is; a vocabulary invented here would have been
a token list.

**Nothing is decided by a count of the forbidden list.** The mapping is a Rust
array and so has a length, but the comparison is against section 8.2's own key
list in order, in both directions: injection `U-I4` dropped one entry *and* its
declared length together, and the test failed because the specification still
writes the key. Adding an accessor that belongs to another aggregate fails
because the sweep finds it. The sweep itself has a floor of sixty compared
pairs, so a mapping that shrank to nothing would fail rather than pass over an
empty set.

## Four independent relations, and where the fourth lives

Section 11.4: *동일·대체·폐지·경과조치는 독립 rule이며 양방향 동일성으로
단순화하지 않는다.*

| Relation | Type | Ends | Answers |
|---|---|---|---|
| 동일, as identity | `IdentityDecision` | ordered pair of courses, plus the `DecisionId` it was recorded against | `CourseRelations::same_course` |
| 동일, as substitutability | `EquivalenceRelation` | ordered pair, `source` may be presented for `target` | `CourseRelations::equivalent` |
| 대체 | `ReplacementRelation` | retired course, replacement course | `CourseRelations::replacements_for` |
| 폐지 | `RetirementRelation` | one course | `CourseRelations::retired` |

`동일` is two types because the plan names identity and equivalency separately
and the acceptance evidence distinguishes them:
`replacement_does_not_imply_identity` is about the first,
`equivalence_is_directional_and_effective_dated` about the second.

**경과조치 is not a course relation.** Section 8.1 says where it applies —
*2026학번 전공 표준형태는 2026학번부터 적용하며 2025학번 이전은 종전 형태 적용*,
tabulated as *version applicability + transition rule*. The unit that moves is
an admission cohort and the thing it moves between is two curriculum versions,
so it has no course end to hang from. `TransitionArrangement` and
`CurriculumVersion::transition_for` are in `version.rs`, and
`no_relation_derives_another` requires `relation.rs` to answer no transition and
`version.rs` to answer one.

`supersedes` and a transition are separate facts. Section 8.2 gives
`CurriculumVersion` a `supersedes` field; it says which version this one
follows and nothing about which cohorts move. A version that supersedes another
and records no arrangement answers `CohortTransition::Unknown` for every cohort,
which is section 8.1's case exactly.

### What "independent" is executed as

- Four types with no `From`, `TryFrom`, `Into`, `AsRef` or `Deref` between them.
  `no_relation_derives_another` compares the **whole `impl` set** naming any of
  the four against a pinned list, so a conversion nobody predicted fails as an
  extra key.
- No signature anywhere in this crate takes one relation type and returns
  another. The whole public signature set is swept, so a method named nothing
  like a conversion fails too.
- Both sweeps read **every product file in the crate**, not `relation.rs`. The
  type and the trait are both local, so the orphan rule refuses a conversion
  written in another crate and refuses nothing written in a sibling module;
  injections `U-I24` and `U-I25` put one in `publish.rs`.
- Each of the four lookups is pinned as whole text, and each is required to read
  exactly one of the four vectors — counted over the pinned text, so a second
  read is a changed pin as well as a changed count.
- Migration `0014` writes the four as **four tables**, not one table with a
  `relation_kind` discriminator. `course_retirement` has no replacement column
  and `course_replacement` has no verdict column, and
  `no_relation_table_carries_another_relations_column` compares each table's
  whole column set.

## Course-code reuse is a decision, never an inference

Section 8.2's `courseCode` is what the catalogue prints. Whether two occurrences
of one code name one durable course is
`CourseRelations::same_course`, which reads recorded `IdentityDecision`s and
nothing else and returns `CourseCodeReuse::Unknown` when none addresses the
ordered pair.

`IdentityDecision::record` refuses `Unknown` as a recorded verdict, and
migration `0014`'s `course_identity_decision.verdict` `CHECK` admits only `SAME`
and `DISTINCT`: the absence of a decision is the absence of a row, and a row
saying nothing was decided would be a record of nothing.

`nothing_infers_a_course_identity` pins the **whole set** of functions in this
crate that produce a `CourseCodeReuse` — three today — and requires no signature
anywhere to take two `CourseCode` values, because comparing two codes is the
strongest available inference and this crate performs none.

## What a publication is, and what atomic means

`CurriculumPublication::from_official_source` takes an
`academic_ingestion::PublishedRules`. That type has private fields and its only
producer is `P2-U6`'s stage nine, which takes a `PublishableRules` that
`Reconciled::publishable` returns `None` for on an undated document. A
curriculum version founded on an `UNSCOPED_OFFICIAL_SOURCE` is therefore not a
value that exists — not because a check here refuses it, but because there is no
argument to call the constructor with.
`crates/curriculum/tests/curriculum.rs` obtains its one `PublishedRules` by
running that crate's pipeline over its own synthetic fixtures, so the reuse is
executed rather than asserted.

`CurriculumPublisher::publish` appends into a live `CurriculumLedger`, one
aggregate at a time, so a partial publication is a state the code can physically
reach. What makes it unreachable is a mark taken before the first append and a
single rewind on every error. Both are pinned whole, `append` and `rewind_to`
are each counted to one call site, and every vector the appending body pushes to
is required to be one the rewind truncates — enumerated out of the appending
body rather than written down, so a fifth vector fails until the rewind reaches
it.

`curriculum_publish_is_atomic_under_injected_failure` is two halves.

- **In memory**: `PublishCheckpoint::ALL` is walked, each checkpoint is failed in
  turn against a ledger that already holds one published version, and the
  assertion is whole-value equality — the ledger after the failed publication is
  the value it was, not a subset of it. The loop checkpoints are failed on their
  *second* arrival, because failing the first is indistinguishable from a
  publication that never started. The same publication with nothing injected is
  then run and required to change the ledger.
- **In the database**: `crates/store/src/curriculum_tests.rs` drives the same
  publication as SQL against a migrated store, refuses each insert in turn, rolls
  back, and requires every one of migration `0014`'s fifteen tables to be empty.
  The insert count is driven rather than written down. The uninjected sequence is
  then committed and required to leave no table empty.

The fault injector is `academic_store::fault`'s shape reused: a callback trait,
a production `NoFault` whose every checkpoint is a no-op, and no
environment-variable, command-line or process-exit switch anywhere.
`academic-retention`'s environment-selected abort is the other shape this
repository has and it is the right one for a fault that must kill a process
mid-write; a publication is a single in-process append sequence, so the failure
that matters is a returned error part-way through it.

## Retirement with no replacement

`RetirementRelation` has one course and an interval. It has no replacement field
and no constructor that takes one, and `course_retirement` has no replacement
column. Section 8.1's *IT창업개론 폐지·대체 미지정* is therefore not a special
case — it is the only shape a retirement has. A replacement is
`ReplacementRelation`'s own value and `course_replacement`'s own row.

## Migration 0014

Fifteen tables, all INSERT-only, each with the `guard_<table>_update` /
`guard_<table>_delete` pair migration `0004` sets as the terms for every
canonical table, and each listed in `academic_store::authorizer::CANONICAL_TABLES`
so the SQLite authorizer is the second layer.

`course` has no registration arm. Section 3.8 fixes the event schema v3 arm list
at eighteen and none of them registers a course, so `course_id` is a parent
reference with no arm — the position `repository_id` already holds in migration
`0004`'s `snapshot` table. A course row is authorized through the curriculum
version whose publication introduced it: `guard_course_authorized` requires
`registered_event_id` to be the event that registered
`introduced_by_version_id`.

Section 8.2's `sourceSnapshot` is the registration frame's `source_digest`.
Migration `0014` adds no column for it because migration `0004` already carries
it on `curriculum_version`, `course_revision` and `offering`.

`0006`, `0007`, `0009` and `0012` are in use. `0008`, `0010` and `0011` are
unclaimed and stay that way. `0013` is reserved for `P2-L3`, which was in flight
on the same `main` when this branch was written.

## Section 38

Three cells stay open, and `OpenGate` states each where it bites.

**`GATE-38-013`** — the engineering-common recognition list and its
required/elective distribution — is an official fact the user must confirm. An
unconfirmed revision holds `CurriculumCategory::Unknown`, and nothing infers a
category from a course code, a credit count, or a sibling revision.

**`GATE-38-014`** — which courses substitute for which, for whom, and from when
— is the same kind of fact. An equivalence exists only where one was recorded,
holds in the asserted direction only, and is derived from no replacement, no
retirement and no shared course code.

**`GATE-38-018`** — how a course's official prerequisite differs from the
instructor's recommended prior knowledge — needs a reviewed source. The two are
captured as separate typed lists on a revision, with separate types
(`OfficialPrerequisite`, `RecommendedPrerequisite`) and separate migration
tables, and this crate contains no function that compares them.

`the_open_gates_have_no_default` compares the whole set of `Default`
implementations in this crate against a four-entry list. The two that exist are
the empty ledger and the empty relation set; both are emptiness rather than a
value.

## What this contract does not claim

- **It does not claim that no code can derive one relation from another.** What
  is executed is narrower and is a composite: `relation.rs` has no conversion
  and no cross-returning signature, both compared as whole sets; the four
  lookups are pinned whole and each reads one vector; migration `0014` gives
  each relation only its own columns; and no file outside
  `crates/curriculum/` names any of the four types, which
  `no_file_outside_this_crate_names_a_curriculum_relation` compares as a whole
  inventory over every workspace product file. A module that spells none of
  those and reimplements the semantics from scratch is refused by nothing here —
  it also reaches none of this crate's values.
- **It does not claim the offering carries no session content in every sense.**
  `lectureRefs` is a list of identifiers, and what those identifiers point at is
  `P2-U7`'s and `P2-L2`'s aggregate. What is executed is that no accessor here
  returns a `TranscriptSegment` field and that no offering table in migration
  `0014` carries a text column beyond the four section 8.2 names.
- **The offering status is a field, not a prediction.** Section 8.3's four
  statuses are typed here; the calibrated probability, the seven feature
  families, the observation window and the per-term evaluation are `P2-U5`'s.
  Nothing here computes one or promotes one into `CONFIRMED`.
- **The rule engine is not here.** A `DegreeRequirementSet`, its `rules` and its
  `transitionRules` (section 11.1) are `P2-U2`'s aggregate.
- **`product_network` remains `NONE` and `production_data_allowed` remains
  `false`.** Nothing in this task moves either. ADR-002 is unaccepted and the
  default lane is `storage_encryption=NONE`.
- **Every fixture is synthetic.** No byte here derives from a personal record, a
  lecture, a private repository, or an external fetch.

## Where the plan and the specification disagree

`t068` section 5's `P2-U1` entry says *identity/equivalency/replacement/
retirement/transition are four independent effective-dated relations*. That is
five names and a count of four. The specification names four
(동일·대체·폐지·경과조치, section 11.4) and puts the fourth at the curriculum
version rather than at the course: section 8.1 tabulates 경과조치 as *version
applicability + transition rule*, and section 8.2's `CurriculumVersion` block is
where an applicability range and a `supersedes` live. The plan's list diverges
in two places — it splits 동일 into identity and equivalency, and it raises
경과조치 to the course level.

The specification wins, as `CONTRIBUTING` and the plan itself require. The
implementation keeps both halves of 동일, because the plan names both and the
acceptance evidence distinguishes them, and places 경과조치 at the curriculum
version, because that is the only level at which it has ends.

A second, smaller divergence: `t068`'s `P2-U1` entry cites
`DegreeRequirementSet.transitionRules` as a `CurriculumVersion` field. That
field is section 11.1's, on `P2-U2`'s aggregate. Nothing here is that field.

## Named acceptance evidence

| Test | Where | Proves |
|---|---|---|
| `course_boundary_rejects_offering_fields` | `tests/compile_fail/` | seven `CourseOffering` fields have no setter on `CourseDraft` and four have no accessor on `Course` |
| `revision_boundary_rejects_section_fields` | `tests/compile_fail/` | twelve section-reality names have no setter on `CourseRevisionDraft` and five no accessor on `CourseRevision` |
| `offering_boundary_rejects_session_transcript` | `tests/compile_fail/` | six per-session names have no setter on `CourseOfferingDraft` and four no accessor on `CourseOffering` |
| `one_course_two_revisions_three_offerings_do_not_leak` | `tests/curriculum.rs` | four named directions plus an exhaustive sweep of all seventy-two ordered pairs of the nine fixture aggregates |
| `equivalence_is_directional_and_effective_dated` | `tests/curriculum.rs` | `A → B` does not give `B → A`; both interval edges; recording the reverse leaves the forward interval alone |
| `replacement_does_not_imply_identity` | `tests/curriculum.rs` | a recorded replacement leaves both identity questions `UNKNOWN`, produces no equivalence in either direction, and retires nothing |
| `retired_course_with_no_replacement_is_representable` | `tests/curriculum.rs` | a retirement publishes with no replacement and produces none |
| `curriculum_publish_is_atomic_under_injected_failure` | `tests/curriculum.rs` and `crates/store/src/curriculum_tests.rs` | every checkpoint failed in turn leaves the ledger the value it was, and every SQL insert refused in turn leaves all fifteen tables empty |

The four source scans that hold what a behavioural test cannot observe —
`the_forbidden_fields_are_the_specifications_own`, `no_relation_derives_another`,
`nothing_infers_a_course_identity`, and
`the_publish_path_has_one_rewind_and_every_failure_takes_it` — are in
`crates/curriculum/tests/curriculum_scans.rs` and are enumerated in
[policy source scans](policy-source-scans.md), with the twenty-six injections
each was measured against.
