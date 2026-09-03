# Project concept classification

`academic-repository-classification` is the `P2-R4` boundary. It is section 18:
`OBSERVED`, `REQUIRED` and `WOULD_BENEFIT_FROM`, and — for each of the three —
what a publication has to carry before the word may be used.

It sits directly on `P2-R2` and `P2-R3`. `OBSERVED` is read out of a
`academic_repository_correlation::RelationEdge` whose evidence is a
`academic_repository_analysis::Finding` at `EvidenceTier::Observed`, and from
nothing else; there is no second ladder here, no second tier vocabulary, and no
path from repository bytes to a classification that skips either crate. It opens
no file and no socket, holds no analyzed byte, and persists nothing: it adds no
migration and has no edge to `academic-store`.

## The proofs are types

Section 34.4's row for this task states the prevention as *enforces REQUIRED
proof schema and mandatory BENEFIT trigger*. Every rule below is a value that
does not exist rather than a check somebody has to remember to run.

| Section 18 rule | What holds it |
|---|---|
| `REQUIRED` needs all five chain steps | each step is the next constructor's argument, taken by value |
| a sufficient user is not a requirement | `UserEvidenceGap` has no `SUFFICIENT` variant and its one constructor answers `None` |
| a whole field cannot be required | `RequiredConcept::realizing` refuses `FIELD` and `ALIAS`; the fields are private |
| a broad category cannot found a requirement | `CurrentBasis` is built from a `P2-R2` finding or an **approved** `P2-R3` document, never from a label |
| `REQUIRED` and `WOULD_BENEFIT_FROM` cannot coexist | `ConceptStance`'s outlook is **one slot** holding one `Outlook` |
| `OBSERVED` and `REQUIRED` may coexist | the observation is a **different field** |
| a benefit needs a trigger and a trade-off | `BenefitContract::new` takes both, and neither list may be empty |
| a classification carries no bare label | `Outlook` has no payload-free variant |

`crates/repository-classification/tests/compile_fail/` holds the compiled half:
seven cases, each a program that fails with a committed diagnostic.

## Section 18.2's chain, step by step

```text
current code/goal
  → concrete responsibility or failure scenario
  → mechanism that controls it
  → required concept
  → user's insufficient/uncertain evidence
```

| Step | Type | What it must carry |
|---|---|---|
| 1 | `CurrentBasis` | a `P2-R2` finding above `PRESENT_ONLY`, **or** an `APPROVED` `P2-R3` intent document |
| 2 | `ConcreteNeed` | step 1 by value, a `RESPONSIBILITY`/`FAILURE_SCENARIO` kind, a name, and **at least one locator** |
| 3 | `ControllingMechanism` | step 2 by value and a name |
| 4 | `RequiredConcept` | step 3 by value, a concept, and a section 7.4 tier that is not `FIELD` or `ALIAS` |
| 5 | `ProofChain` | step 4 by value and a `UserEvidenceGap` |

Each arrow is a **by-value argument**, so the derivation is carried rather than
asserted: a mechanism cannot be recorded as controlling one need and read as
controlling another, and a chain cannot be assembled out of order.

The number of steps is not asserted as a number here or in the code.
`required_failure_chain` reads section 18.2's arrow diagram back out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares its length
against `ChainStep::ALL`, so the step count is a measurement of the design
document. Dropping the fifth variant from the enumeration was injected and fails
on `section 18.2 draws 5 steps and this crate enumerates 4`.

### The per-step missing codes, and where they apply

Section 18.4's fourth bullet is `AI는 제안하고 사용자는 확인·수정한다`, and what a
model proposes is not yet any of those five types. `ChainDraft` is the one door
from an untyped proposal into a chain, and `ChainDraft::seal` names the **first**
missing step:

| Step absent | Code |
|---|---|
| `current code/goal` | `MISSING_CURRENT_BASIS` |
| `concrete responsibility or failure scenario` | `MISSING_CONCRETE_NEED` |
| `mechanism that controls it` | `MISSING_CONTROLLING_MECHANISM` |
| `required concept` | `MISSING_REQUIRED_CONCEPT` |
| `user's insufficient/uncertain evidence` | `MISSING_USER_EVIDENCE_GAP` |

Past that door there is no incomplete chain to guard, because there is no
incomplete chain: `ProofChain` has private fields, no `Default`, and one
constructor. `removing_any_chain_step_blocks_publish` removes each of the five
in turn and observes its own code, and also observes that a corpus with no
complete chain publishes **no** `REQUIRED` and materializes no entity.
Defaulting the fifth step instead of refusing it was injected and fails on
`removing MISSING_USER_EVIDENCE_GAP did not block the publish with its own code`.

### The fifth step has no `SUFFICIENT` value

`UserEvidenceGap::of(mastery, freshness, status)` is total and answers:

| Observed state | Answer |
|---|---|
| mastery below `APPLIED` | `INSUFFICIENT` |
| `APPLIED` or above, and freshness below `MODERATE` **or** status not `USER_CONFIRMED` | `UNCERTAIN` |
| `APPLIED` or above, freshness `MODERATE` or above, status `USER_CONFIRMED` | `None` |

`None` is not an error. A concept the user demonstrably knows is a concept this
project does not require them to learn, and the caller simply has nothing to
pass to `ProofChain::closed_by`. That is the refusal, and there is no variant to
name instead — `user_evidence_gap_has_no_sufficient_value` is the compiled half,
and adding a `Sufficient` variant was injected and fails it.

The `UNCERTAIN` arm is why `OBSERVED` and `REQUIRED` coexist in practice as well
as in principle: section 4's `2년 전 배운 Virtual Memory는 mastery가 유지된 채
freshness가 STALE로 보일 수 있고` is a concept the project uses and the user has
applied, whose evidence is still not current.

## A broad category cannot require a whole field

Section 18.2: `단지 backend라는 이유로 Distributed Systems 전체를 요구하지 않는다`.
Three independent mechanisms, none of them a keyword list.

1. **The tier.** Section 7.4 calls a `FIELD` *a broad area that carries no
   independent prerequisite of its own* and an `ALIAS` a surface form that
   *never carries evidence itself*. `RequiredConcept::realizing` refuses both,
   the fields are private, and it is the only constructor — so no
   `RequiredConcept` anywhere in a program is at either tier.
2. **The basis.** `CurrentBasis` has two constructors and neither takes a label.
   One takes a `P2-R2` `Finding` — scoped to a symbol or a component and never
   to the repository, so the requirement inherits that refusal — and refuses one
   at `PRESENT_ONLY`, because section 18.1's own example is a manifest naming
   `redis` with no import, call or configuration, and section 36.5 has the user
   correct exactly such an entry as `template 잔재`. The other takes an intent
   document at `APPROVED`.
3. **The arity.** One chain yields one required concept. Requiring twelve
   concepts needs twelve chains, each with its own concrete need and its own
   sites in the snapshot.

The tier is section 7.4's fact, supplied by the caller the way a `SubjectId` is:
this crate holds no entity registry and reads no ontology. What it holds is that
a `FIELD` has no route to `REQUIRED` however it arrives.

`broad_category_cannot_require_a_whole_field` runs `REQ-18-007`'s own sentence
in both halves: a backend-label-only corpus produces no basis and no
requirement, and the lost-update chain over real evidence produces exactly one,
`isolation`, with no field beside it. Admitting `EntityKind::Field` was injected
and fails on `tier FIELD reached REQUIRED`.

## Section 18.3's contract, and why a generic list produces nothing

`BenefitContract` has private fields, no `Default`, and one constructor taking
all four parts:

| Section 18.3 field | Type | Rule |
|---|---|---|
| `trigger` | `Vec<Trigger>` | at least one |
| `currentTriggerState` | `TriggerState` | `NOT_MET`, `MET` or `UNKNOWN` |
| `benefit` | `BenefitDimension` | `SCALE`, `RESILIENCE`, `PERFORMANCE`, `MAINTAINABILITY` — section 18.3's own four |
| `tradeoffs` | `Vec<TradeOff>` | at least one |

A benefit dimension is an enumeration because section 18.3 names the four a
benefit may be. A trade-off is not: `consistency, failover complexity, cost` are
three examples of an open set, so a `TradeOff` is a caller-chosen identifier.

`MET` exists because a trigger that has fired is a fact a reader has to see, and
because seeing it is **not** a reclassification: a met trigger does not make a
concept `REQUIRED`, which needs section 18.2's chain and the user's own evidence
gap, neither of which a trigger supplies. `beneficial_trigger_contract` observes
that a `MET` contract still publishes as `WOULD_BENEFIT_FROM` and produces no
requirement.

A list of concept names has none of the four, so it does not become a contract
that fails validation — it never becomes a contract.
`generic_nice_to_have_list_produces_zero_findings` feeds five bare names through
`BenefitDraft::seal`, observes five `MISSING_TRIGGER` refusals and zero
contracts, and observes that the publication over them carries **zero**
findings. Synthesizing a trigger and a trade-off for a bare name was injected
and fails it.

The constructor's own two refusals are asserted directly rather than only
through the draft. They were not, at first: removing `BenefitContract::new`'s
empty-trade-off check left `beneficial_trigger_contract` green, because
`BenefitDraft::seal` raises its own refusal first. The injection pass found it
and the test now covers both.

## Coexistence is shape, not a check

Section 18.4's first two bullets say different things, so they are held by two
different shapes:

* `OBSERVED와 REQUIRED는 동시에 가능하다` — `ConceptStance::observed` is **its own
  field**, present or absent independently of anything else.
* `REQUIRED와 WOULD_BENEFIT_FROM은 같은 goal/scope에서는 동시에 둘 수 없다` —
  `ConceptStance::outlook` is **one slot** holding one `Outlook`, whose two
  variants are `Required(ProofChain)` and `Beneficial(BenefitContract)`.

`P2-R3`'s `ImplementationDrift` argued the same distinction in the other
direction: its four scopes *can* hold at once, so they are four fields and not
one enumeration. Here exactly one may hold, so it is one enumeration and not two
fields.

`서로 다른 goal에는 가능하다` needs no second mechanism: `ClassificationKey` carries
the goal version, so two goals are two keys and two stances.

Offering both for one concept in one goal scope is **refused** —
`RequiredAndBenefitInOneScope` — rather than resolved by a precedence rule.
Choosing between them would be the automatic reclassification section 18.4's
fifth bullet forbids. Silently dropping the benefit instead was injected and
fails `required_and_benefit_conflict_in_one_scope`.

## A classification is bound to a snapshot and a goal version

`ClassificationKey` holds the snapshot identifier, the `GoalScope` — a goal
identifier **and** its version — and the concept, and has no constructor that
omits one. A chain read over one snapshot cannot be published against another:
`classify` refuses it with `ChainIsAboutAnotherSnapshot`.

`classification_is_snapshot_and_goal_scoped` classifies one concept under two
snapshots and two goal versions and requires three distinct keys and three
distinct requirement identities.

### The requirement identity is a digest, and why

A materialized `ProjectConceptRequirement`'s identity is a domain-separated
digest of the four facts section 18.4 binds — snapshot, goal, version, concept.
It was, at first, those four joined with `.` and truncated to `RequirementId`'s
64 bytes. A snapshot identifier is most of 64 bytes, so the goal version and the
concept were truncated away and two requirements differing only there shared one
identity. `classification_is_snapshot_and_goal_scoped` measured that failing
before the identity became a digest, and reverting it to the joined form is an
injection that fails it again.

That is this Run's `P2-A1` fifth-audit P1 defect in a second place: content
standing in for identity, with the collision silent.

## A user override opens a conflict and rewrites nothing

Section 18.4's fifth bullet: `새 evidence가 사용자 override와 충돌하면 자동
재분류하지 않고 ClassificationConflict를 연다`.

A `ClassificationConflict` is a record **beside** both sides, built by cloning
each — the shape `P2-R3` fixed for `ImplementationDrift` and the same reason,
`CONTRIBUTING.md` rule 2. Two things follow in `classify`'s output:

* the published stance keeps the **user's** answer, because a correction is the
  later decision about the same subject; and
* the proposal is not discarded — it is the conflict's second side, so a reader
  can see what the analysis said and why it did not take effect.

`OverrideDecision` has three values and a total `contradicts` table over the
three classification labels. There is no override of `OBSERVED`: that is a
statement about what the snapshot contains, and a correction to it is a
correction to `P2-R2`'s evidence rather than to this classification.

An override is keyed on the **goal and the concept**, and records the snapshot
it was made *from* rather than being keyed on it. Section 36.5's `이 override는
다음 분석에서도 유지된다` is why: keying the decision on the snapshot would
silently expire it at the next capture, which is the failure `REQ-36-029` is
written against. `user_override_creates_conflict_not_reclassification` reruns
the classification over a second snapshot and observes the override still
standing, and observes that an override under another goal governs nothing.
Publishing the proposal anyway was injected and fails on `the analysis
overwrote the user's decision`.

## The entity and its lifecycle

Section 18.4 materializes each `REQUIRED` finding as a
`ProjectConceptRequirement` binding six facts, and the reason for binding them
is the seventh: the history. All six are fields with no `Option` among them, and
every one but the identity and the timestamp comes out of the chain, so the
entity cannot disagree with the finding it materializes.

| Status | Payload | Meaning |
|---|---|---|
| `OPEN` | — | the requirement stands |
| `SATISFIED` | snapshot **and at least one locator** | `충족`: the need is now controlled, and here is where |
| `RETIRED` | snapshot and a `RetirementReason` | `소멸`: it stopped applying without being met |
| `REPLACED` | snapshot **and the successor's identity** | `대체`: another requirement took its place |

A satisfaction with no site in the new snapshot and a replacement with no
successor have no representation. Section 36.6's `Path A`/`Path B` is the second
one: a mechanism gives way to another and the reader follows the arrow.

No transition takes `&mut self`. Each **consumes** the requirement and returns a
new one whose history is the old one plus a row, which is what makes
`REQ-18-018`'s *without deleting A* structural rather than a convention. A
second transition out of a terminal status is refused and the refusal names the
status it is already in. Replacing the history instead of appending to it was
injected and fails
`requirement_entity_lifecycle_tracks_satisfied_retired_replaced`.

## Locator migration preserves the original evidence

Section 17.4: `새 snapshot에서는 locator migration을 시도하되 원래 evidence를
보존한다`. `migrate_locators` does both verbs. A `MigratedFinding` holds the whole
original `Finding` by value and never edits it; what migration produces is a
list beside it.

The match is on `P2-R2`'s `SymbolFingerprint`, a digest of path, symbol kind and
name that holds no span — so inserting lines before a declaration moves its span
and leaves its fingerprint alone, which is what makes migration possible at all.
A locator that does not land is a record, not an error:

| Outcome | When |
|---|---|
| `MIGRATED` | the fingerprint is in the new snapshot's symbol table |
| `NO_SYMBOL_ANCHOR` | the original locator names no symbol — a manifest row or a configuration key |
| `PATH_REMOVED` | the path is not in the new snapshot's coverage |
| `SYMBOL_GONE` | the path is there and the symbol is not |

### One record per original locator, positionally

`MigratedFinding::migrations` is a `Vec` with one entry per original locator, in
the original's order, each carrying its `ordinal`. Not a map, and the reason is
specific: this Run's `P2-A1` fifth audit found a P1 defect where an artifact's
**content** was its identity, so deleting two byte-identical artifacts wrote two
tombstones under one key and the second silently replaced the first. Two things
here would reproduce it.

* Keying on the **original** locator. Two locators of one finding can be equal
  in every field, and `P2-R2`'s own extractor produces such a pair: a scalar in
  an infrastructure document is pushed to both the configuration index and the
  IaC index at one span, and the ladder reads
  `config_tokens().chain(iac_tokens())`.
* Keying on the **migrated** symbol. Two sites inside one declaration migrate to
  one fingerprint.

`finding_locator_migration_preserves_original_evidence` asserts that the corpus
actually contains both collapsing shapes before asserting anything about them,
so neither half can pass vacuously, and deduplicating on the original locator is
an injection that fails it. `the_migration_result_is_positional_and_never_keyed_
on_content` holds the shape as well as the behaviour: `src/migrate.rs` names no
map or set type at all, so a content-keyed collapse cannot be introduced without
that scan failing.

## What this crate may hold

Seven things, and the field inventory's last column is which one: a
caller-supplied identifier, a system-derived identifier, a path the gate
classified, a closed vocabulary value, a value of a reviewed crate, a value of
this crate, or a count, revision or timestamp. There is no eighth, and in
particular no byte buffer: no field of this crate is declared `Vec<u8>` or
`[u8; N]` under any name.

That claim is a whole-set comparison over **all 105 fields of every type this
crate declares**, in both directions, and it is deliberately not the check
`tools/secret-debug-policy.test.mjs` performs. That tool matches a field's
**name** against a fixed alternation, so a field holding the same bytes under a
name outside its vocabulary is invisible to it — the defect `P2-R3` measured one
step out and recorded in `docs/contracts/repository-correlation.md`. That tool
passing this crate is therefore not evidence about this crate; the inventory is.
A field holding analyzed bytes under the name `annotation` was injected and
fails `every_field_of_this_crate_is_in_the_inventory` as an extra key.

## It opens nothing

Three whole-set comparisons in both directions — every `use` item, every
two-segment path reached through a crate root, every macro invoked — plus a
forbidden-token pass over every file of the package, tests included, as the
third and weakest layer. Three injections spelling none of the listed tokens and
adding no listed name were each observed failing it: a whitespace-separated
absolute filesystem path with no `use` item, a compile-time `include_str!` file
read, and a filesystem type imported under a harmless alias.

Two files of the package read a file and the set is pinned: the source scan
itself, and the acceptance suite, which reads the design document so that
section 18's classification names, section 19's legend glyphs and section 18.2's
step count are measured rather than restated. Both are named in
`docs/contracts/policy-source-scans.md`.

## Named acceptance evidence

`cargo test -p academic-repository-classification` executes:

| Test | Evidence |
|---|---|
| `required_failure_chain` | section 18.2's five steps, measured against the design document, and one complete chain publishing one `REQUIRED` and one entity |
| `removing_any_chain_step_blocks_publish` | each of the five removed in turn, each blocked with its own code, and no `REQUIRED` published without one |
| `broad_category_cannot_require_a_whole_field` | a backend-label-only corpus founds nothing; `FIELD` and `ALIAS` are refused; the lost-update chain requires exactly `isolation` |
| `beneficial_trigger_contract` | the four parts required by the constructor **and** by the draft, each refusal naming its part, and a met trigger that is still a benefit |
| `generic_nice_to_have_list_produces_zero_findings` | five bare names produce five refusals, zero contracts and an empty publication |
| `observed_and_required_coexist` | one concept carrying both labels, and an applied-fresh-confirmed state closing no chain |
| `required_and_benefit_conflict_in_one_scope` | one goal scope refused, two goal scopes both publishing, and no stance anywhere showing both |
| `classification_is_snapshot_and_goal_scoped` | three distinct keys and three distinct requirement identities across both axes, and a chain refused against another snapshot |
| `user_override_creates_conflict_not_reclassification` | the user's answer published, the proposal preserved as the conflict's second side, the override surviving the next capture, and another goal's override governing nothing |
| `requirement_entity_lifecycle_tracks_satisfied_retired_replaced` | the six bound facts, all three terminal statuses, the earlier value unchanged, and a second transition refused |
| `finding_locator_migration_preserves_original_evidence` | both collapsing shapes asserted present, one record per original locator with its ordinal, spans moved, the original whole, and a removed symbol reported rather than dropped |

Beside them: `only_an_approved_goal_founds_a_requirement`,
`a_need_without_a_site_is_not_concrete`,
`the_three_classifications_are_the_design_document_s`, the nine source scans,
and the seven `compile_fail` cases.

All fixtures are synthetic, built in process, and captured through `P2-R1`'s own
`capture_local`; every operation is local and deterministic.

## What this task does not decide

* **Personal competency.** Section 17.6's `ProjectSnapshot OBSERVES Concept` and
  `User APPLIED Concept` are different claims, and this crate produces only the
  first. `P2-R5` owns the second.
* **The goal schema.** `GoalScope` is an identifier and a version. `P2-R6` owns
  `ProjectGoal`'s text, criteria, constraints and unresolved decisions.
* **Persistence.** Nothing here is written. There is no migration and no edge to
  `academic-store`.
* **The ontology.** The tier a concept sits at is section 7.4's fact and arrives
  as an argument; this crate holds no registry and resolves no alias.
