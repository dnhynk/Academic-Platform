# Competency model and evidence rubric

`academic-competency` is the `P2-Y1` boundary. It is section 24.1 and section
24.3: a competency as an observable performance statement with a context,
performance criteria, enabling concepts and an evidence rubric; the six evidence
stages; and the rule that using a dependency settles no cell.

It sits on three boundaries and restates none of them. `academic-domain` already
places `Competency` in section 7.1's node hierarchy and already fixes
`ENABLES_COMPETENCY`'s direction, cardinality and closed qualifier schema, so
this crate reads that registry rather than declaring a second one.
`academic-knowledge-state` already decided which section 13.2 evidence rows
license a promotion, so section 24.3's dependency sentence is that table's own
answer. `academic-repository-competency` already separated
`ProjectSnapshot OBSERVES Concept` from `User APPLIED Concept`, so this crate
takes the second and has no arm for the first. It opens no file, opens no
socket, reads no clock, persists nothing, adds no migration, and has no edge to
`academic-store`.

## The proofs are types

| Section 24 rule | What holds it |
|---|---|
| a competency is a performance, never `knows X` | there is no `statement` argument anywhere; `Competency::statement` renders one from the parts |
| a deserialized competency cannot carry a hand-written sentence | `Deserialize` is `try_from`, re-renders, and refuses a document whose `statement` disagrees |
| a competency has a context | `Situation` is a required argument of `declare` and refuses the empty one |
| a competency has criteria | `declare` refuses an empty list |
| every criterion is checkable | `declare` requires a rubric row for each criterion, and every row to name a criterion, in both directions |
| a concept is not a competency | `ConceptRef` and `CompetencyId` have no conversion in either direction |
| one spelling in two namespaces is two concepts | `ConceptRef` carries the namespace as part of the value |
| a dependency declaration settles no cell | `PromotingEvidence::of` refuses `EvidenceCeiling::NoPromotion` |
| a repository observation settles no cell | `EvidenceSource` has two origins and neither is a `ProjectObservationClaim` |
| a withdrawn claim settles no cell | `StageEvidence::of_personal_claim` refuses `ClaimStanding::Rejected` |
| a filled cell is a derivation | `RubricSheet` and `StageEvidence` are `Serialize` and not `Deserialize` |
| a competency is never edited in place | no `&mut self` method, no setter, no public field |

`crates/competency/tests/compile_fail/` holds the compiled half: eight cases,
each a program that fails with a committed diagnostic.

## Section 24.1's statement is rendered, not stored

Section 7.1 states the rule this crate exists for:

> `Competency는 "개념을 안다"가 아니라 관찰 가능한 상황에서 수행할 수 있다는
> 문장으로 모델링한다.`

`declare` takes five arguments — an identity, a `Situation`, the criteria, the
enabling concepts, and the rubric — and none of them is a sentence.
`Competency::statement` composes a `CompetencyStatement` from the situation, the
criteria's own text and the rubric's own rows, and `CompetencyStatement` has no
constructor of its own. There is therefore nowhere in this crate to write
`knows X`.

The serialized form carries section 24.1's `statement` key, because section 24.1
does. It is written by `From<Competency>` and **compared** by
`TryFrom<CompetencyWire>`: a document whose `statement` is not the sentence its
own parts render is refused. That refusal is over the whole rendered string, so
it does not depend on what the substituted sentence says —
`competency_observability` shows a `안다` sentence, section 24.1's own example
sentence, and the correct sentence with one trailing space all refused, and the
unedited document accepted.

**What makes the statement observable is that every part of it names either an
occasion somebody could watch or an artifact somebody could open.** A situation
is required; at least one criterion is required; every criterion must be named
by at least one rubric row, and a rubric row says what a reader has to be able
to open. A competency whose criteria nothing witnesses is a value that cannot be
built.

## A concept and a competency are two types, and two namespaces

`CompetencyId` and `ConceptRef` have no `From`, no `Into`, no shared
`AsRef<str>`, and no constructor of one that takes the other.

`ConceptRef` also carries **which namespace named the concept**, because two
boundaries below this one name concepts and they do not share a spelling.
`P2-N1`'s ontology issues an `EntityId`, which is what `P2-N2`'s admitted
evidence carries; `P2-R4`'s classification key carries a validated token, which
is what `P2-R5`'s personal claim carries. Equality is over the pair, the way
`P2-R5`'s `ExternalAuthorId` carries its `IdentitySource`, so a classification
token spelled out as the ontology identifier itself — byte-identical text — is
still not the ontology reference.

**This crate resolves no namespace into the other.** That is the entity
registry's job, and doing it here would be exactly the silent conversion section
24.1 refuses, one type over. A criterion may name concepts in either namespace,
or in both.

## The enabling relation is stored once and queried both ways

Section 7.2 fixes `ENABLES_COMPETENCY` as `concept → competency`, `ManyToMany`,
with the inverse label `is enabled by`, and ends with
`역방향 탐색은 query view로 제공하며 반대 edge를 중복 저장하지 않는다`.

`EnablingGraph::of` reads the edges out of the competencies themselves — the one
place `enabledByConcepts` is written — and holds a single forward list.
`competencies_enabled_by` and `concepts_enabling` are two filters over it. There
is no reverse index and no `competency → concept` row.

The two qualifiers section 7.2 requires are `ContributionImportance` and
`Necessity`, and their values are **measured** rather than restated:
`enabling_qualifiers_are_the_registry_s` compares both enumerations against
`PredicateName::EnablesCompetency`'s own descriptor in both directions, so a
fourth importance is a change to the predicate registry rather than one this
crate may make on its own.

## Section 24.3's six stages, measured

Section 24.3 names them in one sentence, in backticks, in this order:

> `사용해봄`, `구조 이해`, `문제 해결`, `장애 debugging`, `설계 선택`,
> `새 상황 전이` evidence를 구분한다.

**The count is not asserted as a number.**
`six_evidence_stages_are_distinct` reads that sentence out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, takes the backticked
spans whole, and compares them against `EvidenceStage::ALL` and
`EvidenceStage::spec_name` in both directions. Six is therefore a measurement of
the design document.

**They are not section 13.2's rows.** Section 13.2 has eight evidence rows and
two of them license no promotion, which leaves six that do. That coincidence is
not a correspondence and this crate builds no map between them: section 13.2's
first row is `transcript에서 meaningful teaching`, which is exposure rather than
`사용해봄`, and section 24.3's `설계 선택` has no row of its own at all, so a
total map would have to invent three of its six answers.

## What settles a cell, in full

A `StageEvidence` settles the cell at (`criterion`, `stage`) when **both** of the
following hold, and there is no third case and no weaker one:

1. the competency's rubric declares a row at that criterion and that stage; and
2. the criterion **names** the concept the record is about, whole-pair,
   namespace included.

There is deliberately **no** arm that reads the competency's
`enabledByConcepts` when a criterion names no concept, because
`PerformanceCriterion::of` refuses a criterion that names none. That arm is
section 24.3's own counter-example one level up: a competency six concepts enable
would otherwise have every cell settled by evidence about any one of them.
`P2-R5` measured the same defect one layer down — `AuthoredWork::touches` fell
back to comparing paths when either side carried no symbol, and credited a user
with a library they had not used — and the repair in both places is that the
weaker key does not exist. `the_join_has_no_second_key` pins `fill`'s matching
condition and `PerformanceCriterion::is_about` whole, and requires `sheet.rs` to
name `enabled_by` and `about` zero times.

Section 24.1 lists a competency's concepts and its criteria and does not bind one
to the other. **This crate requires the binding**, and that is a decision this
task makes rather than a sentence it read: without it there is no key to join
evidence on that is narrower than the competency, and section 24.3's first
sentence is exactly the refusal of the wider one. `P2-Y3`'s
`four_navigation_directions_terminate_at_criterion_and_evidence` needs the same
binding for a different reason: navigation has to end at a performance criterion
plus an evidence locator, and a cell that is not criterion-bound cannot.

## What may found a record

Two doors, and there is no third.

**`P2-N2`'s admitted evidence, if its section 13.2 row promotes.**
`EligibleEvidence` has one producer, `EligibilityOutcome::admit`, which runs
section 13.4's four checks. `PromotingEvidence::of` then asks
`EvidenceKind::ceiling` and refuses `EvidenceCeiling::NoPromotion`, which is that
table's own answer for `dependency/install/import만 존재` and for `과목 grade`.

**`P2-R5`'s `User APPLIED Concept`.** `StageEvidence::of_personal_claim` takes a
`PersonalApplicationClaim` and refuses one that has been taken back. There is no
overload, trait or conversion that takes a `ProjectObservationClaim`, and
`no_product_file_names_a_project_observation_claim` sweeps every product file for
the name and requires it to be absent.

The concept is **read out of the foundation** in both cases. `StageEvidence` has
no concept argument, so there is no way to record evidence about one concept and
file it under another.

## Three readings of an unsettled cell

`CellState` separates `Filled`, `Empty` — the rubric admits this stage here and
nothing settles it — and `NotInRubric` — the rubric declares no row here.
Section 24.3's example table writes the last two the same way, as `—`.
Separating them is what lets `P2-Y3` display them apart; this crate selects no
display and computes no aggregate.

A record that settles no cell is in `RubricSheet::unmatched` rather than
discarded, so a caller can see that evidence arrived and settled nothing.

## It persists nothing

No migration and no edge to `academic-store`, for `P2-N2`'s and `P2-R5`'s
reason: a sheet is a derivation over evidence two crates below already froze, and
a second copy of it in a database would be a second place for it to be wrong. A
`Competency` is authored rather than derived, and its durable form is the
section 24.1 schema this crate round-trips; **where** it is written is `P2-Y2`'s
question, because that task owns the versioned bundle a competency is published
inside.

## What the injection campaign measured

Twenty-eight injections, each applied on its own, built on its own, and reverted
before the next. Every one compiled. Twenty-seven were bitten, and each one's
first bite is recorded in the task report.

**One was not.** Reducing `identity::validated` to a non-empty check — dropping
the `[A-Za-z0-9._-]` rule and the 64-byte bound — passed the whole suite: the
shape a concept token arriving from `P2-R4` has to be was declared and
unmeasured. `every_identifier_is_the_shape_p2_r4_issues` closes it as a
**whole-set classification** rather than a list of rejected spellings: every
ASCII byte is offered inside an otherwise legal identifier and required to be
admitted exactly when the test's own independent predicate says it belongs, in
both directions, across all four constructors, beside the length boundary on
both sides and the four refusals naming themselves apart. The same injection now
fails, as do a one-byte-wider bound and a widened byte class.

**The same hole was one step out.** `P2-R5`'s `identity::validated` carries the
same rule, and the same weakening — reshaped to keep the `matches!` invocation,
so that crate's macro inventory is unchanged and only the shape rule is gone —
passed *its* whole suite too. It is repaired there rather than recorded: see
`every_identifier_is_the_shape_this_crate_admits` in
`crates/repository-competency/tests/competency_lanes.rs`.

## What this task does not decide

* **Role bundles.** `P2-Y2` owns `RoleProfile`, versioning, importance inside a
  bundle, and fork lineage. Nothing here bundles competencies.
* **The readiness view.** `P2-Y3` owns the matrix as a view, the separation of
  missing from unknown from freshness, auxiliary scores and their disclosure, and
  the non-guarantee notice.
* **Freshness.** `P2-N3` owns the bands. There is no time input to any function
  here, and no field of this crate is declared `u64`.
* **Concept identity resolution.** Named above: the entity registry owns it.
* **§38.** This task leaves no gate open and closes none.

## Where the design document and the plan diverge

`t068` §5's `P2-Y1` entry and section 24.3's prose agree on six evidence stages
and on their names. **Section 24.3's own example matrix does not**: its columns
are `학문적으로 배움`, `문제/과제`, `Project 적용`, `장애/Debug`, `설계 선택` and
`Freshness` — five evidence columns, two of the six stages (`구조 이해` and
`새 상황 전이`) absent, and one column (`학문적으로 배움`) that is not one of the
six. The prose sentence is the enumeration this crate implements, because it is
the one the plan's `six_evidence_stages_are_distinct` names and the one that
lists six. The table is an illustration of a view, and `P2-Y3` owns the view.
