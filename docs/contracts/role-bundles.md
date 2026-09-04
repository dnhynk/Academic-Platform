# Role bundles, versioning and fork

`academic-role-profile` is the `P2-Y2` boundary. It is section 24.2, and its
premise is one sentence of that section:

> `Backend, Systems, Database, Distributed Systems, Infrastructure/Platform,
> SRE, Cloud, Security, ML/AI, Data, Compiler/PL, Research 등을 지원하되 role
> 이름을 시장의 단일 진리로 두지 않는다. 사용자가 목표 조직·연구실·project에
> 맞춰 bundle을 fork할 수 있다.`

A bundle is the user's own versioned claim about which competencies a role asks
for. It is not a fact about the labour market, there is no market feed here,
and this build ships no bundle of its own.

## The identity is a pair

Section 24.2's example writes `id: backend_engineer_profile_v4`. That spelling
folds two different things — which lineage, and which version of it — into one
string.

**The stored identity is `RoleProfileRef`: the ordered pair of a
`RoleProfileId` and a `RoleProfileVersion`, compared field by field.**
`RoleProfileRef::rendered` produces section 24.2's spelling for a reader and
has **no inverse**: no `FromStr`, no `TryFrom<String>`, no parser. Two different
pairs may render the same text — `backend_engineer_profile` at version four and
`backend_engineer_profile_v4` at version one both render
`backend_engineer_profile_v4` — and `an_identity_is_a_pair_and_not_a_rendered_name`
exhibits exactly that and shows the two are not equal and both fit on one shelf.

`P2-R4` measured why this matters one stage over: a classification key built by
joining several values and truncating the join collided two findings whose
*goal version alone* differed, and `P2-A1` had already caught the same shape as
a P1 defect. A versioned bundle is the same risk in the same place.

The version's shape is **not** this crate's choice. Section 7.2's
`RELEVANT_TO_ROLE` row carries exactly one qualifier, `role_profile_version`,
of kind `PositiveInteger` and required, so `RoleProfileVersion` is a non-zero
`u32` with zero refused at every door including deserialization, and
`the_version_qualifier_is_the_registry_s` compares both the qualifier set and
that kind against the registry in both directions.

That comparison also carries a second fact. Section 24.2's `importance` —
`CORE`, `COMMON`, `CONTEXT_DEPENDENT` — is **not** a registry qualifier, unlike
`ENABLES_COMPETENCY`'s `contribution_importance`. So `BundleImportance` is read
out of section 24.2's own YAML block, in both directions, and the qualifier-set
comparison is what fails if a later registry grows an importance qualifier for
this edge. There is one vocabulary for one thing, and the test says which
document owns it.

## A role name settles nothing

Three separations, and each one is measured rather than described.

| The thing that must not happen | What stops it |
|---|---|
| a label read as a direction | there is no function from `RoleLabel` to `RoleDirection` and none back; `direction.rs` names `RoleLabel` **zero** times and `identity.rs` names `RoleDirection` **zero** times. A bundle's direction is a field the user set. |
| a label resolved to one bundle | `BundleShelf::by_label` returns a `LabelReading` — a list plus an optional diagnostic — and its whole signature is pinned. No public function keyed on a label returns `Option<&RoleProfile>`. |
| one organisation's bundle overwriting another's | the shelf is keyed on `RoleProfileRef` and `shelve` returns `RoleError::VersionAlreadyShelved` for an occupied key. Two organisations occupy two keys. |

When a label reaches more than one bundle, the reading carries a
`LabelAmbiguity` naming the distinct lineages and the distinct scopes it
reached. **A diagnostic, not a resolution** — `P2-R4`'s `ClassificationConflict`
and `P2-N5`'s tied roots, one stage over. A label that reaches one bundle
carries no diagnostic, which is what makes the diagnostic a reading rather than
a constant.

## Twelve directions, and the `등`

`RoleDirection` has twelve named arms in section 24.2's own order, and
`twelve_role_directions_are_representable_or_explicitly_absent` reads that
sentence out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, splits
it, and compares the names against `RoleDirection::NAMED` in both directions. A
specification that renames, adds or drops one fails this crate.

The sentence ends `등을 지원하되` — *and so on*. That openness is
`RoleDirection::UserNamed`, which is **not** one of `NAMED` and **not** a
thirteenth name: it is where a user curating a bundle for something the sentence
does not spell puts it, without anybody widening the closed part.

**Absence is named, not silent.** `BundleShelf::directions_covered` returns a
row for every one of the twelve — including the ones the shelf holds nothing for
— plus a row for every `등` direction a held bundle points at. A map that
omitted its empty rows would leave ten of the twelve silently missing on a shelf
holding two, and that is the normal case here, because:

## `GATE-38-029` stays open

Section 38.3's eighth question asks what governance keeps a role bundle current
without over-weighting labour-market fashion. **Phase 2 does not answer it.**
It ships user-owned bundles with recorded sources and no market feed.

In the source that is three facts, each measurable:

* **No bundle is shipped.** `the_only_producers_of_a_bundle_are_the_three_doors`
  compares the whole set of public functions returning a `RoleProfile` by value
  against `declare`, `revise` and `fork`. A shipped default bundle would be a
  fourth producer. `DirectionCoverage::absent_because` is the one sentence that
  says so, and it names the gate.
* **Every bundle records its sources.** `declare` refuses an empty `sources`
  list, and a `BundleSource` carries the user's own citation and the day they
  consulted it — two fields and no third. A source with no date cannot answer
  *is this bundle current?*, which is the gate's own question.
* **Nothing fetches.** The crate's whole `use` set, its reached paths and its
  macros are compared against pinned inventories in both directions, and its
  dependency map holds no HTTP, feed or fetch edge.

The fourth producer of a `RoleProfile` is the deserializer, and it is not a
public function: `TryFrom<RoleProfileWire>` runs the same entry-and-source check
the three doors run, and `each_guarded_name_has_exactly_its_call_sites` counts
`checked`'s four call sites so a fifth door that skipped it is visible.

## An edit is a new version; a fork is a new lineage

`RoleProfile` has no public field, no setter and no `&mut self` method — and
**nothing in this crate takes `&mut self`**, `BundleShelf` included, which
`no_public_function_mutates_in_place` measures over every public signature in
the package. The shelf consumes and returns.

| Operation | What it takes | What comes back | `BundleOrigin` |
|---|---|---|---|
| `declare` | the parts | version 1 of a new lineage | `Authored` |
| `revise` | `&RoleProfile` and an `AdjustmentLayer` | the base's lineage at the next version | `Revised(base pair)` |
| `fork` | `&RoleProfile` and a new lineage identity | version 1 of the new lineage | `Forked(base pair)` |

Neither `revise` nor `fork` can touch its base, and `shelve` refuses to replace
an occupied pair, so a change that wants to be stored has to take a version it
did not hold. That is `P2-N2`'s `assertion 은 제자리에서 변경되지 않는다` one
stage over, and `P2-R5`'s replaced claims one stage over that.

A fork carries the base's entries and its direction, and states its **own**
label, scope, date and sources. It cites its base once, by the exact pair, and
does not copy the base's citations and claim them. Forking version three and
forking version four therefore record different things, which
`fork_preserves_base_and_records_lineage` shows by comparing the two origins.

Deserialization is not a way around any of this: an `AUTHORED` bundle at a
version nothing authored, a `REVISED` bundle that does not name its own
predecessor, and a `FORKED` bundle that names its own lineage are each refused
with `RoleError::OriginDoesNotMatchTheVersion`.

## User adjustments are a second document

Section 24.2's YAML block shows `userAdjustments` as a key of the role profile.
That is what a *rendered* profile looks like. What is stored is two documents:

* `RoleProfile`, which has **no adjustment field at all** and whose wire denies
  unknown fields, so `userAdjustments` cannot ride in through JSON either; and
* `AdjustmentLayer`, which names the base it adjusts by its **exact**
  `RoleProfileRef`.

The separation buys two things a merged document cannot have. An organisation's
bundle stays byte-identical whatever the user did to it — which is what
`two_org_bundles_coexist_with_scope_and_source` compares. And a layer written
over version three is not silently applied to version four: `revise` refuses the
mismatch by identity rather than by lineage, with
`RoleError::LayerIsForAnotherVersion`.

An adjustment is one of three changes to the entry list — `ADD`, `REMOVE`,
`REWEIGHT` — and carries the user's required, non-empty reason. Two adjustments
naming one competency are refused, because the outcome would otherwise depend on
the order they were written in.

## Favouriting is not a career decision

Section 25.11: `role을 즐겨찾기해도 "진로 확정"으로 간주하지 않는다.`
Section 37: `과거 Backend path를 실패로 표시하지 않는다` and
`관심이 없으면 다시 neutral 상태로 둘 수 있다`.

`RoleInterest` holds a `RoleProfileId` and an `InterestStanding`. **Not a
version** — so it does not even select which bundle is in force — and not a
competency, a weight, a plan, a goal or a date.

`InterestStanding` has exactly the three standings those two sections spell:
`FAVORITED`, `EXPLORING`, `NEUTRAL`. `REFUSED_STANDINGS` carries the two that
are absent — `CHOSEN` and `FAILED` — each with the sentence that refuses it, and
`favoriting_a_role_is_not_a_career_decision` reads both sentences back out of
the specification and requires neither name to be an arm. That is `P2-N2`'s
shape one stage over: `AutomaticLevel` has no `Fluent`, because the value
nothing may promote to is held by the absence of the value.

Nothing takes one as an argument. `an_interest_is_not_an_input_to_anything`
compares `impl RoleInterest`'s own four functions against the whole set of
public signatures in the package naming `RoleInterest`, which must be empty; and
`interest.rs` is required to name none of `RoleProfile`, `RoleProfileRef`,
`BundleEntry`, `CompetencyId`, `BundleShelf`, `BundleImportance` or `RecordedOn`.
`fork` — the one act section 24.2 says a user performs on a bundle — takes a
`&RoleProfile`, and `an_interest_cannot_be_forked` is the compiled half.

`RoleInterest::standing_now` consumes and returns rather than taking
`&mut self`, so moving from favourited to neutral does not rewrite the standing
that came before it. There is no failure to record, because there is no arm for
one.

## What this task does not decide

* **The readiness matrix.** `P2-Y3` owns the competency × evidence view, the six
  axes, auxiliary scores and their disclosure, and the non-guarantee notice.
  Nothing here scores anything or reads an evidence rubric.
* **The Career Explorer.** Section 25.11's graph, comparison view and
  acquisition options are a `P2-X`-stage surface. This crate has no compare
  function.
* **Freshness.** `P2-N3` owns the bands. A bundle's `validAt` and a source's
  consultation date are recorded and read by nothing here.
* **Persistence.** No `academic-store` edge, no migration. A bundle is a value;
  where it is written is not this task's question.

## Refusals

| Error | When |
|---|---|
| `InvalidIdentifier` | a lineage identifier, scope or direction name that is not `[A-Za-z0-9._-]` within 64 bytes, classified byte by byte over the whole value |
| `EmptyText` | a label, source citation or adjustment reason that carries nothing |
| `NotACalendarDate` | a `validAt` that is not `YYYY-MM-DD`, or a day `academic_ingestion::Date` refuses |
| `VersionIsNotPositive` | zero, at every door including deserialization |
| `VersionWouldOverflow` | the next version at the top of the range; wrapping to one would claim to be the first |
| `BundleNamesNoCompetency` | a bundle with no entries, including one the adjustments emptied |
| `DuplicateCompetency` | one competency named twice in one bundle |
| `BundleRecordsNoSource` | a bundle citing nothing; `GATE-38-029` stays open on the strength of the recording |
| `LayerAdjustsNothing` | an empty adjustment layer, which would still take a version |
| `CompetencyAdjustedTwice` | two adjustments on one competency, whose outcome would depend on their order |
| `LayerIsForAnotherVersion` | a layer offered against a pair it was not written over |
| `AddedCompetencyAlreadyPresent`, `AdjustedCompetencyIsNotInTheBundle` | an adjustment that disagrees with the base about what is in it |
| `ForkIntoTheSameLineage` | a fork into the base's own lineage, which is a revision |
| `VersionAlreadyShelved` | a shelf key that is already occupied |
| `OriginDoesNotMatchTheVersion` | a deserialized bundle whose origin disagrees with its own version |

## Acceptance

`crates/role-profile/tests/role_bundles.rs` holds the six tests the execution
plan names — `role_profile_schema_round_trip`, `role_edit_creates_a_new_version`,
`twelve_role_directions_are_representable_or_explicitly_absent`,
`two_org_bundles_coexist_with_scope_and_source`,
`fork_preserves_base_and_records_lineage`,
`favoriting_a_role_is_not_a_career_decision` — plus seven measurements those
rest on. `crates/role-profile/tests/role_scans.rs` is the policy source scan and
is registered in [policy source scans](policy-source-scans.md).
`crates/role-profile/tests/compile_fail/` holds nine cases, one shape each.
