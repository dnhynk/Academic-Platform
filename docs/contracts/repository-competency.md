# Personal competency promotion

`academic-repository-competency` is the `P2-R5` boundary. It is section 17.6:
the separation of `ProjectSnapshot OBSERVES Concept` from `User APPLIED
Concept`, and what has to be true before the second may be written.

It sits directly on `P2-R4`. A project claim is read out of a
`academic_repository_classification::ConceptStance`'s observed half and from
nothing else; a personal claim needs, beside that observation, a sealed
`AuthoredWork` whose own changed sites meet the observation's locators. It opens
no file and no socket, holds no analyzed byte, and persists nothing: it adds no
migration and has no edge to `academic-store`.

The whole of this task is that the second claim is not the first with a
different word on it. Section 24.3 says why: `dependency를 사용했다는 이유만으로
competency를 채우지 않는다`. If the separation fails, the product tells a user
they have a competency they do not have.

## Section 17.6's five checks, and what holds each

Section 17.6 lists five things to confirm separately. `PromotionCheck` is that
list, in the section's own order, and the count is not written here or in the
code: `the_promotion_checks_are_section_17_6_s` reads the bullets back out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares their number
against `PromotionCheck::ALL`.

| Bullet | Check | What holds it |
|---|---|---|
| `사용자 authorship 또는 실질적 기여` | `AUTHORSHIP` | `AuthorshipMap::resolve`, whole-pair set membership |
| `단순 scaffold가 아닌 이해가 필요한 선택·수정` | `MEANINGFUL_CHANGE` | `ScaffoldRubric`, versioned configuration |
| `test, explanation, debugging, review 등 결과 evidence` | `OUTCOME_EVIDENCE` | `CandidateSupport`, which grades rather than blocks |
| `읽은 것인지 직접 구현한 것인지` | `READ_VERSUS_AUTHORED` | `AuthorshipMode` has no review value |
| `생성형 AI ... 검증·수정·설명했는지` | `GENERATED_CODE_WARRANT` | `CodeOrigin::Generated` holds the warrant |

Four block a promotion and the third grades it.
`each_of_section_17_6_s_checks_changes_the_outcome` fails one check at a time
over the whole enumeration and requires each to change what a run produces, so
no entry can be registered without biting — which is the `S-18` shape this
repository has already measured once.

## Using a repository is not a personal claim

`promote`'s personal loop iterates the `AuthoredWork`s it was given, so a run
with none publishes every project claim and no personal one. That is arithmetic
rather than a rule.

`repo_use_alone_creates_no_personal_claim` asserts it and anchors the ceiling
externally: section 13.2's own row for `dependency/install/import만 존재` is read
back out of the design document and required to say `mastery 승격 없음`, so the
claim the test makes is the specification's rather than the test's.

A work also has to have touched **the place the observation names**.
`AuthoredWork::touches` requires the path to be equal and then the two to be at
the same **granularity**: a site inside a declaration meets a locator inside the
same declaration, by `P2-R2` `SymbolFingerprint`, and a site outside every
declaration — a manifest row, a configuration key, a module-level import — meets
a locator outside every declaration. It is span-independent for `P2-R4`'s
locator-migration reason: an edit above a declaration moves its span and leaves
its fingerprint alone.

The mixed pair is refused rather than falling back to the path, and that is a
correction the injection pass produced. The first version compared fingerprints
when both sides had one and the path otherwise, which meant a symbol-bearing
edit met the symbol-less locator recorded at a file's import line — so editing
*any* declaration in a file that happens to import a library would have credited
the user with that library's use, which is the failure this whole task is
against. `a_work_meets_an_observation_by_fingerprint_before_by_path` measured it
and is what holds the repair: it asserts the corpus really carries both a
symbol-bearing and a symbol-less locator at one path, admits an edit inside the
declaration the observation names, and refuses an edit at the same path inside a
differently named one.

Authoring anything at all in a repository that observes a concept is not
authoring that concept's use, and `a_change_elsewhere_promotes_no_concept`
measures the coarser half in both directions.

## The authorship identity mapping

A version-control system records an author string. Section 33's rule for every
such value is `외부 ID는 canonical ID가 아니라 ExternalIdentity mapping으로
저장한다`, so this crate never treats one as the user.

`ExternalAuthorId` is a **pair** — an `IdentitySource` and a value — because the
same characters mean different things in different namespaces.
`AuthorshipMap::resolve` is whole-pair set membership: no case folding, no
trimming, no display-name comparison, and no fallback to the value when the
source does not match. An identity the user has not recorded is not the user,
which is the direction that fails closed.

`other_author_commit_is_ineligible` runs four shapes past it — a colleague's
address as the control, then three that spell nothing forbidden and are each a
way an identity could be *nearly* the user's: the user's own address under
another `IdentitySource`, the same address differing only in ASCII case, and a
display name that reads like the user while the address is somebody else's.
Each was also injected into `resolve` as an accepted shape and observed failing.

The mapping carries a **version**, and a personal claim records which version
admitted it, so a user who later adds a work address does not silently change
what an earlier claim rested on.

## The scaffold rubric is versioned configuration

`ScaffoldRubric` is a value the caller supplies. It has no `Default`, no
constructor that fills any part in, and `judge` compares against no numeric
literal at all — every threshold it uses is a field of the value it was handed.
`the_rubric_is_configuration_and_not_a_constant` holds all three over the source
and additionally requires every part to be a `ScaffoldRubric::of` argument and
to be named on this page.

| Part | Question it answers |
|---|---|
| `id` | which rubric decided |
| `version` | which version of it |
| `scaffold_change_kinds` | is this **kind** of edit one that needed a choice? |
| `scaffold_path_classes` | is the file one this repository is the author of? |
| `minimum_bearing_sites` | is there enough of it to be a contribution? |

A site bears understanding when **both** hold: its `ChangeKind` is not one the
rubric calls scaffold, and its `P2-R2` `PathClass` is not one either. A
`CONTROL_FLOW` edit inside vendored source is somebody else's control flow, and a
`FORMATTING` edit inside first-party source is still formatting.

`scaffold_path_classes` reuses `P2-R2`'s `PathClass` rather than introducing a
second vocabulary for the same fact: `VENDORED` and `GENERATED` are already that
crate's names for source this repository did not write.

A verdict carries the rubric and the version that produced it, both arms. Two
rubric versions may disagree about one change without either being a bug, and
`scaffold_only_change_is_ineligible` runs exactly that: the strict rubric
refuses a pin bump and a compose edit, a version that does not call a
configuration edit scaffold admits the same change, and the claim the second
produces says which version admitted it.

A rubric that requires no site is refused — `RubricAdmitsNothing`. That is not a
rubric, it is the absence of one.

## Review is never serialized as authored

Section 17.6's fourth bullet, held by a value that does not exist.

A connector may report four things a person did. A personal claim may serialize
two.

| `ContributionKind` | `AuthorshipMode` |
|---|---|
| `AUTHORED` | `AUTHORED` |
| `MODIFIED` | `SUBSTANTIVE_CONTRIBUTION` |
| `REVIEWED` | — |
| `READ` | — |

`ContributionKind::authorship_mode` is that table. It is the only function
anywhere that produces an `AuthorshipMode` from anything, its call sites are
counted at one, and it is total over its own enumeration with no default arm, so
a fifth kind added later has no arm and the crate stops compiling rather than
defaulting to authorship.

`AuthorshipMode` has no `Reviewed` and no `Read` variant. A review therefore has
no spelling in the field a claim puts its authorship into — not one that is
rejected at runtime, one that does not exist. `REVIEW` is an `OutcomeKind`, it
lives in the outcome list, and the two vocabularies are compared and required to
be disjoint.

`review_is_never_serialized_as_authored` walks the whole of
`ContributionKind::ALL`, partitions it into what sealed and what was refused,
requires the two to cover the enumeration and not to overlap, and requires the
set of authorship spellings a claim actually serialized to equal
`AuthorshipMode::ALL`'s. Mapping `REVIEWED` to `AUTHORED` inside the one door
was injected and fails it.

## Generated code needs a warrant, and the warrant is three steps

Section 17.6's fifth bullet is three verbs in an order — `검증·수정·설명` — so
there are three types, each taking the previous by value:

```text
VerifiedByUser  →  ModifiedByUser  →  ExplainedByUser  →  GeneratedCodeWarrant
```

None of the four has a public field, a `Default`, or a second constructor, so a
warrant over code the user verified and explained but never modified is not a
value that validates badly — it is a program that does not compile.

`CodeOrigin::Generated` **holds** the warrant, so generated code with no warrant
has no representation at all: the question *is there a warrant for this* has no
place to be asked and no place to be forgotten. The runtime half is one layer
out, at `ContributionDraft::seal`, which refuses a report that says the code was
generated and offers no warrant.

`unmodified_generated_code_creates_no_applied_claim` measures the refusal, the
admission with all three steps present, and — as the control — that the *same
report* with a hand-written origin seals, so the refusal is about the origin and
not about the change.

## Outcome evidence strengthens and never creates

`CandidateSupport` is ordered, and its top two levels are read out of section
13.2's table rather than invented: `CODE_AND_OUTCOME` is the row whose ceiling
is `Applied candidate`, and `DIAGNOSED_FAILURE` is the row whose ceiling is
`Applied, transfer facet 강화`. `AUTHORSHIP_ONLY` is what is left when the user's
own change carries nothing beside it — and it is **already a candidate**, which
is the whole of *strengthens rather than creates*.

`outcome_artifact_strengthens_candidate` runs both directions: all four outcome
kinds raise the level, and all four with no authorship — each alone and then all
together — produce zero personal claims. An outcome naming another change does
not count toward this one.

## The two claims, and the identity trap

`ClaimId` admits 64 bytes, which is exactly one hexadecimal SHA-256 digest. A
snapshot identifier is most of 64 bytes on its own, so an identity built by
joining the facts a claim binds and truncating to fit would drop the last of
them, and two claims differing only there would share one identity, silently.

`P2-R4` shipped exactly that defect and measured it: its materialized
requirement's identity was four facts joined with `.` and cut to 64 bytes, so two
requirements differing only in goal version collided. It is the same shape this
Run's `P2-A1` fifth audit raised as a P1 — content standing in for identity.

Both identities here are domain-separated digests and the two domain strings
differ:

| Claim | Preimage |
|---|---|
| `ProjectSnapshot OBSERVES Concept` | domain, snapshot, goal, version, concept |
| `User APPLIED Concept` | domain, user, snapshot, goal, version, concept, change |

`two_claims_have_independent_ids_and_provenance` measures three things: the two
claims over one subject have different identifiers; two personal claims
differing only in the **last** bound fact have different identifiers, over a
corpus whose snapshot identifier is asserted long enough for a truncated join to
have dropped it; and neither identifier begins with the snapshot identifier,
which a joined identity would. Reverting either function to the joined form is an
injection that fails it.

The provenance records are two, and no field of one is a field of the other. A
personal claim names the project claim it was promoted from **by identifier**
and does not hold it: a personal claim that embedded the project one could be
read as speaking for it.

## Rejecting the personal claim leaves the project claim

`PersonalApplicationClaim::rejected` **consumes** the claim and returns a new
one, the way `P2-R4`'s requirement lifecycle does, and it touches nothing else.
Nothing in this crate takes `&mut self`; `no_public_function_mutates_in_place`
holds that over the whole package.

A `ProjectObservationClaim` has no rejection at all. What the snapshot contains
is not a thing a judgement about the user can retract, and a correction to it is
a correction to `P2-R2`'s evidence.

A second rejection is refused and names the claim.

## What this crate may hold

Eight things, and the last column of the field inventory is which one: a
caller-supplied identifier, a system-derived identifier, an external identity
value, a closed vocabulary value, a value of a reviewed crate, a value of this
crate, a count, revision or timestamp, or **the user's own words**.

The eighth is what `P2-R4`'s seven did not need. A warrant's note and its
explanation are prose the user wrote about generated code, and section 17.6 asks
for them by name, so they are inventoried rather than refused — and no `Debug`
here reduces them to a length, because a warrant a reader cannot read is not
evidence of anything. The inventory pins that there are exactly two such fields
and which, so a third place for prose is visible.

There is no ninth, and in particular no byte buffer: no field of this crate is
declared `Vec<u8>` or `[u8; N]` under any name. That claim is a whole-set
comparison over **all 100 fields of every type this crate declares**, in both
directions, and it is deliberately not the check
`tools/secret-debug-policy.test.mjs` performs — that tool matches a field's
**name** against a fixed alternation, which is `S-10`. That tool passing this
crate is therefore not evidence about this crate; the inventory is.

## It opens nothing

Three whole-set comparisons in both directions — every `use` item, every
two-segment path reached through a crate root, every macro invoked — plus a
forbidden-token pass over every file of the package, tests included, as the
third and weakest layer. The reached-path set has one entry, `thiserror::Error`,
and the macro set has one, `matches!`.

Two files of the package read a file and the set is pinned: the source scan
itself, and the acceptance suite, which reads the design document so that
section 17.6's five bullets and section 13.2's ceiling rows are measured rather
than restated. Both are named in `docs/contracts/policy-source-scans.md`.

## Named acceptance evidence

`cargo test -p academic-repository-competency` executes:

| Test | Evidence |
|---|---|
| `repo_use_alone_creates_no_personal_claim` | section 13.2's own ceiling row read back, an observation that really is one, every project claim published and no personal one |
| `other_author_commit_is_ineligible` | four author shapes, three of which spell nothing forbidden, each refused; the recorded identity as the control |
| `scaffold_only_change_is_ineligible` | a pin bump and a compose edit refused with the rubric, version and counts named; a second rubric version admitting the same change; the claim recording which |
| `outcome_artifact_strengthens_candidate` | all four kinds raising the level, `DEBUGGING` reaching its own, all four creating nothing without authorship, and an outcome about another change counting for nothing |
| `review_is_never_serialized_as_authored` | the whole of `ContributionKind::ALL` partitioned, the two vocabularies disjoint, the serialized set equal to `AuthorshipMode::ALL`, and a review beside authorship staying an outcome |
| `unmodified_generated_code_creates_no_applied_claim` | the refusal, the same report hand-written as the control, the three-step admission, and each step refusing an empty offering |
| `two_claims_have_independent_ids_and_provenance` | two identities over one subject, the truncation collision measured, two provenance records, and the project claim unchanged by the personal one's rejection |

Beside them: `the_promotion_checks_are_section_17_6_s`,
`each_of_section_17_6_s_checks_changes_the_outcome`,
`a_change_elsewhere_promotes_no_concept`,
`a_work_meets_an_observation_by_fingerprint_before_by_path`,
`a_work_is_bound_to_its_snapshot_and_its_user`,
`a_rubric_that_requires_nothing_is_not_a_rubric`, the eight source scans, and the
seven `compile_fail` cases.

All fixtures are synthetic, built in process, and captured through `P2-R1`'s own
`capture_local`; every operation is local and deterministic. A `ChangedSite` can
only be built over a `P2-R2` `Locator`, whose constructor is crate-private to
that crate, so no contribution anywhere in this suite names a place the analyzer
did not see.

## What this task does not decide

* **Whether the candidate is true.** This crate produces a `CANDIDATE`, which is
  section 13.2's own ceiling for `직접 작성한 production/personal project code와
  test`. Section 13.4's `user accept / edit / leave unconfirmed / reject` is the
  user's decision and not an analyzer's.
* **The competency model.** Section 24.1's `Competency`, its performance
  criteria and its evidence rubric are `P2-Y1`'s. What is here is one
  `User APPLIED Concept` claim, which is a concept and not a competency.
* **Freshness.** Section 13.3's bands are a separate axis and nothing here
  computes one.
* **Persistence.** Nothing here is written. There is no migration and no edge to
  `academic-store`.
