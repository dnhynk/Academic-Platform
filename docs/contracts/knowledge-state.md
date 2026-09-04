# Knowledge state, facets, evidence ceilings

`academic-knowledge-state` is the `P2-N2` boundary. It is section 13: the six
mastery levels with their five facets, the eight evidence-to-ceiling rows, the
four deterministic eligibility checks, and — for each promotion section 13
forbids — the value that does not exist rather than the check somebody has to
remember to run.

It sits on four boundaries and restates none of them. `academic-domain` already
declares `MasteryLevel` and `FreshnessBand`, so this crate declares neither
again. `academic-ledger` already fixed section 30.3's conflict vocabulary, so
this crate emits `ConflictReason::NewEvidenceConflict` rather than a second
token. `academic-lecture-document` decides what a lecture document is, and
`academic-repository-classification` decides what a project snapshot observes.
It opens no file, opens no socket, reads no clock, persists nothing, adds no
migration, and has no edge to `academic-store`.

## The proofs are types

| Section 13 rule | What holds it |
|---|---|
| an automatic projection never reaches `FLUENT` | `AutomaticLevel` has five variants and no `Fluent` |
| `FLUENT` needs repetition **and** user confirmation | `FluentAuthorization` takes both by value and is `with_fluency`'s only argument |
| an AI cannot mint a user confirmation | `UserConfirmation`'s one constructor runs ADR-003's actor matrix |
| a grade cannot promote a concept | `CourseGradeSignal` has no concept field and no `ConceptEvidence` variant |
| ineligible evidence cannot be projected | `EligibleEvidence` has one producer and `project` takes only those |
| an assertion is never mutated | no `&mut self` method, no setter, no public field; `revise` returns a new value |
| a deserialized `FLUENT` cannot skip the gate | `Deserialize` is `try_from` and refuses `FLUENT` without its record |
| a history is not edited | every method that changes it consumes it and returns a new one |

`crates/knowledge-state/tests/compile_fail/` holds the compiled half: eight
cases, each a program that fails with a committed diagnostic.

## Section 13.1's ladder, measured rather than restated

The six are `academic_domain::MasteryLevel`'s. What this crate adds is `LADDER`
— the same six in section 13.1's own row order — and `rung`, a `match` with no
wildcard arm, so a seventh level is a compile error here rather than a value some
list quietly fails to mention.

**The count is not asserted as a number.**
`mastery_enum_is_exactly_six_ordered` reads section 13.1's table out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares its `Level`
and `이름` columns against `LADDER` and `rung` in both directions, so six is a
measurement of the design document. The same test reads the schema example's
`facets:` keys and compares them against `MasteryFacet::ALL`, so five is too.

`FacetStrength` is closed at the three values section 13.1's example exhibits —
`STRONG`, `MODERATE`, `LIMITED_EVIDENCE` — and `ks_applied_mixed_facets`
compares the distinct values in that example against the enumeration in both
directions. A fourth is a change to the design document, not one this crate may
make on its own.

A `FacetProfile` has one slot per facet, no `Default` and no public field, so a
profile with `transferToNovelSituation` left out is a value that cannot be built.

## Section 13.2's eight rows

`CEILINGS` carries all eight with the design's own `허용되는 기본 해석` and `자동
상한` cells verbatim, and `evidence_ceilings_are_never_exceeded` compares the
table against the document in both directions.

**The typed ceiling is read out of the cell, not out of the value it checks.**
This was a defect in this task's own first test, found by injection: raising
`MeaningfulTeaching`'s ceiling in `CEILINGS` *and* in `EvidenceKind::ceiling`
together, leaving the cell text alone, passed. `ceiling_of_cell` now derives the
expected ceiling from the cell by the document's own rule — a cell names one of
section 13.1's six level names or it names none, and naming none is
`NoPromotion` — and every projection is compared against that.

The word `candidate` appears in three cells and not in the other three. Both
spellings are kept verbatim in `CeilingRow::ceiling_cell`, and in this task the
word **carries no rule beyond the ceiling**: for all eight rows alike, the
ceiling is the highest level the row's evidence alone may support. `FLUENT`'s
extra requirement comes from section 13.1's `AI 단독 판정 금지, 반복된 강한
evidence와 사용자 확인 필요`, which is a different sentence, and it is held by
`FluentAuthorization`.

### The last two rows are refused by two different mechanisms

Because the two cells say different things.

* `dependency/install/import만 존재 → mastery 승격 없음`. That evidence *does*
  name a concept, so it is a `ConceptEvidence` variant whose ceiling licenses
  nothing. It is admitted, retained and shown, and it raises nothing.
  `DependencyOnly::of_stance` answers only for a `P2-R4` stance whose
  `observed()` is absent, and `ProjectUse::of_stance` only for one where it is
  present — the two are complements over the same input. An `ObservedProof`
  exists only for a `P2-R2` finding at `EvidenceTier::Observed`, so the
  difference between the fourth row and the seventh is `P2-R4`'s decision and not
  a second ladder here.
* `과목 grade → concept별 직접 승격 없음`. A grade is not evidence about a concept
  at all, so `CourseGradeSignal` **has no concept field** and no
  `ConceptEvidence` variant. There is nowhere to write the concept down. It is
  retained on the assertion as a `BroadSignal`, which is `REQ-13-019`'s *grade
  remains linked as broad signal*.

### The automatic contribution caps at `APPLIED`

`automatic_contribution` is total over `EvidenceKind` with no wildcard arm. Row
six's ceiling is `Fluent candidate` and `AutomaticLevel` has no `Fluent`, so its
automatic contribution is `Applied` — the highest the automatic type can express,
which is section 13.2's own `자동 상한은 안전한 기본값이다`.

## `UNSEEN` is not a failed test

Section 13.1's first row: `evidence 없음이지 "모른다"는 시험 결과가 아님`. Two
states project as `UNSEEN` and they are **not the same value**:

* `UnseenBasis::NoEvidenceRecorded` — nothing was recorded; and
* `UnseenBasis::EvidenceRecordedWithoutPromotion` — something was recorded and
  none of it licensed a promotion. An installed dependency is here, and so is an
  exercise attempt that did not succeed, which reaches the assertion's
  contradicting-evidence list rather than a verdict.

`UNSEEN_MEANING` is the design document's own sentence, and
`unseen_is_not_a_failed_test` asserts the document contains it, so the copy a
user is shown cannot drift into a claim about the person.

## `estimateConfidence` is evidence sufficiency

Section 13.1 says so in as many words. The schema keeps the design's field name
and `EvidenceSufficiency` keeps the meaning, three ways: it is not `PartialOrd`
and not `Ord`, so two users' or two concepts' values cannot be ranked by the
type; there is no conversion in either direction with `MasteryLevel`; and it
always carries `SufficiencyGap`s, so a low value says *what is missing* rather
than *how good the user is*.

The value is deterministic: 1000 permille less each gap's deduction, clamped at
zero. A blocked candidate contributes one gap per distinct failing check, not one
per blocked item. The per-check deduction is 275 because section 13.1's own
illustration — `"mastery 4, confidence 0.45"는 applied evidence 후보가 있지만
authorship이나 수행 결과가 불명확함을 뜻한다` — names two unresolved checks, and
two of them on otherwise sufficient evidence lands on 450.
`estimate_confidence_is_evidence_sufficiency_and_not_a_score` reproduces exactly
that case.

## Section 13.4's four checks

```text
├─ exact concept linked?
├─ user authorship/participation known?
├─ outcome known?
└─ source integrity valid?
```

`eligibility_four_checks_block_with_reason_codes` reads those four lines out of
the design document and compares them against `EligibilityCheck::ALL` in both
directions, so *four* is a measurement.

Three of the four lines end in `known?` and the fourth in `valid?`, so an absent
answer is the answer *no*: each of the four answer types has an explicit
`Unknown` variant that blocks with its own reason code. That is section 3's
`알 수 없는 필드는 빈 문자열이 아니라 UNKNOWN으로 저장한다`, applied to evidence.

A blocked item carries **every** failing check's code, not the first. A known
*failure* is a known outcome and passes the third check; whether it promotes
anything is section 13.2's question and not this one's. The first check refuses
section 7.4's `FIELD` and `ALIAS` tiers — the same two `P2-R4` refuses, for the
same reason: a field carries no independent prerequisite of its own and an alias
never carries evidence itself.

`EligibleEvidence` has private fields and one producer, and `project` takes a
slice of those, so evidence that failed a check is not evidence a later layer
must remember to filter.

## Assertions, retraction, and the review card

An assertion's identity is a SHA-256 over a **length-prefixed** preimage that
includes the version number and the predecessor's identity. A value that spells a
separator cannot collide with two that do not — the identity-from-content
collapse `P2-A1` found in `P2-R4` came from joining four fields and truncating
them — and a version binds its predecessor, so a history cannot be reordered and
a middle version cannot be dropped without every later identity changing. The
`try_from` deserializer recomputes it, so a renumbered version does not come back.

A retraction says which of section 13.4's four checks the evidence turned out to
fail; `타인의 풀이를 복사한 것` is the authorship check answered again and
differently. There is no second reason vocabulary, because a retraction is the
admission decision revisited. The retraction row and every earlier assertion stay
in the history and only the projection is recomputed, which is section 13.2's own
`철회 event도 역사에 남고 projection만 다시 계산한다`.

A user-confirmed state is immune in both directions. A proposal that would raise
or lower it opens a `KnowledgeStateConflict` holding both sides — the standing
assertion and the model's proposal, cloned and neither rewritten — and appends
no version. That is `P2-R3`'s `ImplementationDrift` and `P2-R4`'s
`ClassificationConflict` for a third pair. A proposal naming the level the state
already holds is not an adjustment, so it opens no card and changes nothing.

## Where the section 38 gates stand

Two are left open and this task fills in neither.

* **`GATE-38-023`** — whether mastery facets are always visible or progressively
  disclosed below the level. That is a user-tested interface decision, and both
  modes must remain reachable and accessible. This crate exposes all five facets
  through one accessor and selects no disclosure behaviour; nothing here hides a
  facet, orders them for display, or ties one to a level.
* **`GATE-38-025`** — the conditions under which reconfirmation of a
  user-confirmed state should be recommended. `P2-M3` left it open and this task
  leaves it open: nothing here expires a confirmation, downgrades one, or
  schedules a prompt.

`GATE-38-024` — the freshness priors — belongs to `P2-N3` and is untouched.

## Named acceptance evidence

`cargo test -p academic-knowledge-state` executes:

| Test | Evidence |
|---|---|
| `mastery_enum_is_exactly_six_ordered` | section 13.1's six rows and five facet keys, measured against the design document in both directions, and the ladder strictly increasing under both orders |
| `ks_applied_mixed_facets` | the schema example's five facet values, an `APPLIED` assertion carrying them, and a wire round-trip that preserves every one |
| `evidence_ceilings_are_never_exceeded` | section 13.2's eight rows with both text cells, the typed ceiling derived from the cell rather than from itself, and no single row's projection above the ceiling its cell names |
| `course_attendance_only_ceiling_is_exposed` | teaching-only evidence reaching `EXPOSED`, the ceiling disclosed with the row that fixed it and that row's own cell, and a control that one further row rises |
| `dependency_only_creates_no_promotion` | the two stance constructors complementary over one input, an installed dependency admitted and promoting nothing, and the same concept observed reaching `APPLIED` |
| `grade_creates_no_concept_promotion` | no `ConceptEvidence` variant for the row, the row contributing `UNSEEN` and licensing no ceiling, and an assertion carrying the grade while projecting `UNSEEN` |
| `unseen_is_not_a_failed_test` | two `UNSEEN` projections that are not equal, their two bases, the retained contradiction, and the copy that is the design document's own sentence |
| `eligibility_four_checks_block_with_reason_codes` | the four questions measured against the design document, one check false at a time with its own code, all four false reporting all four, the `FIELD` tier refused, and a known failure passing |
| `fluent_requires_repetition_and_user_confirmation` | no automatic level for `FLUENT`, a repetition refused for one context and for dependent work, a model actor refused at ADR-003's matrix in both pairings, and a wire `FLUENT` refused without its record and with a record on another level |
| `assertion_is_never_mutated_in_place` | the old version byte-identical after a revision, the chain binding its predecessor, and a renumbered version failing to deserialize |
| `retraction_is_append_only_and_recomputes_projection` | the retraction row and both versions in the history, the earlier projection still readable at its own identity, the current one recomputed, and an unknown retraction refused |
| `confirmed_state_rejects_ai_adjustment` | a raise and a lower each refused with the assertion unchanged, the direction recorded, and the same proposal superseding before confirmation |
| `conflict_card_instead_of_auto_change` | both sides preserved, `P2-M3`'s token, no version appended, an unchanged proposal opening no card, and another concept's proposal refused |

Beside them: `estimate_confidence_is_evidence_sufficiency_and_not_a_score`, the
eight source scans, and the eight `compile_fail` cases.

All fixtures are synthetic and built in process. The lecture evidence is a node
of a document `P2-L4`'s builder produced over a real `P2-L2` capture and a real
`P2-L3` run; the project evidence is a `P2-R4` stance over a real `P2-R1`
capture and `P2-R2` ladder. Every operation is local and deterministic.

## What this task does not decide

* **Freshness.** `P2-N3` owns the bands, their inputs, the priors, the decay and
  the bounded spillover. This crate carries `P2-N3`'s band and its confidence in
  their own fields and computes neither. It reads no clock at all, which is what
  makes section 1's fifth invariant — `Mastery와 Freshness를 합치지 않는다` — a
  property of the whole package rather than a rule inside one function.
* **Persistence.** Nothing here is written. There is no migration and no edge to
  `academic-store`.
* **The ontology.** The tier a concept sits at is section 7.4's fact and arrives
  as an argument; this crate holds no registry and resolves no alias.
* **The question graph, gaps and paths.** `P2-N4` through `P2-N6`.
