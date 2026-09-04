# Gaps, root candidates and the explanation contract

`academic-gap` is the `P2-N5` boundary. It is section 15: the five gap kinds, the
four-dimension overlay, the descent to the first strong deficit with its ancestor
impact, the candidates it refuses to choose between, and the eight-field
explanation contract with the validator that rejects broad advice.

It sits on three boundaries and restates none. `academic-domain` already declares
section 7.2's twenty predicates, the `prerequisite` column that says which two a
path engine may traverse, `EntityKind`'s five ontology tiers and section 14's
question statuses. `academic-knowledge-state` already decided what evidence is
admissible and what mastery it supports. `academic-freshness` already decided
what a band is. This crate reads all three and adds one thing: the judgement that
a person is missing something — which it makes as rarely as section 15.1 says to.

It opens no file, opens no socket, reads no clock, persists nothing, adds no
migration, and has no edge to `academic-store`.

## A gap is a refusal before it is a report

Section 15.1: `Gap은 낮은 Knowledge State 자체가 아니라 **활성 목표의 성공을
가로막는, 근거가 있는 prerequisite 부족**이다.`

Three restrictions, and each is a value that does not exist rather than a check.

| Section 15 rule | What holds it |
|---|---|
| a low state with no goal is not a gap | no function produces a `GapCase` without an `&ActiveGoal`; there is no `GapCase::for_concept` |
| success criteria come before expansion | `GoalCriteria::of` returns `None` for an empty list, has no `Default`, and `ActiveGoal::declare` takes one by value |
| traversal is `REQUIRES` and strong `BUILDS_ON` | `PrerequisiteEdge::admit` calls `P2-C4`'s `prerequisite_descriptor`; this crate holds no allowlist of predicates |
| a weak `BUILDS_ON` never blocks | `blocking_floor` answers `None` for `HELPFUL`, so `PrerequisiteEdge::blocks` is false and the descent has nothing to cross |
| four dimensions are overlaid onto **one** concept | `ConceptState::overlay` refuses evidence, a projection or a contribution naming another |
| a tie is retained | `roots_of` returns every tied root and `GapCase::of` refuses a tie with no diagnostic; there is no `primary_root` |
| broad advice is not an explanation | `GapExplanation::of` refuses one on structure, and the crate holds no phrase to match against |
| an explanation is never edited | no `&mut self` method, no setter, no public field on any of the eight |

`crates/gap/tests/compile_fail/` holds the compiled half: eight programs that
each fail to compile with a committed diagnostic.

## The three counts are measurements

| Count | Where it is read from | Test |
|---|---|---|
| five gap kinds | section 15.2's table, all three columns | `five_gap_types_route_correctly` |
| four state dimensions | section 15.2 step 3's sentence | `four_state_dimensions_are_overlaid` |
| eight explanation fields | section 15.3's sentence | `eight_field_explanation_is_complete` |

Each is compared in both directions, so a row added to the design document and a
variant added here are equally visible.

### Step 6 names four where the table has five

Section 15.2's sixth step reads `hard gap, refresh gap, evidence gap, terminology
mismatch를 구분한다` — four informal names. The table immediately below it has
five rows, and the fifth, `CONTEXT_GAP`, appears in no prose sentence of section
15.

**The table is normative** because it is the half that fixes the identifiers, and
`t068`'s acceptance evidence is named `five_gap_types_route_correctly`. So
`GapKind` has five variants, and `STEP_SIX_INFORMAL_NAMES` keeps step 6's four so
the discrepancy is a measured value with
`the_step_six_prose_names_one_fewer_than_the_table` on it rather than something a
later reader rediscovers. Nothing was invented to reconcile them.

`P2-RF17` widened that test from step 6 to the whole of section 15, because *no
prose sentence of section 15* is what this page claims and step 6 alone cannot
say it. Section 15's lines are split into prose and table rows, and both halves
are checked in both directions: no gap identifier appears in the prose, every one
appears in the table, each of step 6's four informal names appears in the prose
and in no table row, and the prose names neither `context` nor `맥락`. Four
injections, each its own build: a `CONTEXT_GAP` sentence added to 15.3, a context
sentence added to 15.1, one informal name moved into a table row, and one dropped
from step 6. All four fail.

## The four dimensions, and what each one decides

Section 15.2 step 3: `사용자 mastery, freshness, confidence와 contradicting
evidence를 overlay한다`. A dimension nothing branches on is a field in a struct,
not an overlay, so each of the four is the sole difference between two outcomes:

| Dimension | Source | Where it decides |
|---|---|---|
| mastery | `P2-N2`'s `MasteryProjection::level` | at or below the edge's floor |
| freshness | `P2-N3`'s `FreshnessProjection::band` | at the floor, but below `RETRIEVAL_FLOOR` |
| confidence | `P2-N2`'s `EvidenceSufficiency` | below the floor *because the records are thin* rather than *because the performance is* |
| contradicting evidence | `P2-N2`'s `MasteryProjection::contradicting` | a recorded failure is a deficit at any rung |

`four_state_dimensions_are_overlaid` holds three fixed, moves the fourth, and
watches the routed kind change.

## Routing, and the one kind that is a `강한 부족`

`route` is total and ordered. The identity is read first, because a mastery
reading on a node `P2-C3` says cannot be attributed to one identity is not a
reading at all.

1. `ONTOLOGY_GAP` — the `P2-C3` identity standing is not `Settled`.
2. `CONTEXT_GAP` — two or more helpful `BUILDS_ON` branches leave the node and no
   success criterion names any of them.
3. `MASTERY_GAP` — contradicting evidence is on record, at any rung.
4. at or above the edge's floor — `FRESHNESS_GAP` below `RETRIEVAL_FLOOR`, and no
   gap otherwise.
5. below the floor — `EVIDENCE_GAP` when nothing was admitted at all or an
   admission check did not resolve, and `MASTERY_GAP` otherwise.

Section 15.2 step 4 looks for `최초의 강한 부족`, so some deficits are not strong.
Which ones is read off the table's own `뜻` column: `EVIDENCE_GAP` is `실제로 알 수
있으나 시스템에 근거가 없음` — the design document says the person may know it;
`FRESHNESS_GAP` is `즉시 사용 불확실` and `P2-N3` says the mastery stands;
`ONTOLOGY_GAP` is a statement about the graph; `CONTEXT_GAP` is a statement about
the goal. Only `MASTERY_GAP` is a claim that the person is missing something, so
`GapKind::is_strong_deficit` is true for it and false for the other four. The
other four are reported and never become a root.

### The two thresholds this crate chose

Everything else is read off a boundary. These two are judgements, and they are
pinned by `the_gap_decisions_are_pinned` so they cannot drift silently.

* `blocking_floor`. A `HARD` edge is section 7.2's `없으면 목표 수행이 신뢰성 있게
  막히는`, which is about *performing*, and section 13.1's rung for `Used in a
  problem, an assignment or an experiment` is `PRACTICED`. A `STRONG` edge is the
  registry's `Near-hard: the goal is unreliable without it`, and section 13.1's
  rung for `Explained in the user's own words` is `UNDERSTOOD`. `HELPFUL` answers
  `None` rather than a floor, so a helpful edge has no rung to compare against.
* `RETRIEVAL_FLOOR` is `MODERATE`. `P2-N3` already named the band at which a use
  stops counting as `최근 사용`: its `SPILLOVER_SOURCE_FLOOR` is `MODERATE`, with
  `a neighbour at LOW was not recently used`. Section 15.2's `즉시 사용 불확실` is
  the same reading of the same scale.

## `weak_builds_on_is_excluded_or_conditional`

`P2-C4`'s registry admits `HARD` and `STRONG` on `REQUIRES` and `STRONG` and
`HELPFUL` on `BUILDS_ON`, so `강한 BUILDS_ON` is `BUILDS_ON` at `STRONG` because
that is the only strength above `HELPFUL` the registry lets that edge carry.

*Excluded* is the whole of the blocking descent: `blocks()` is false for a
helpful edge, so `expand` never crosses one and it can never produce a candidate.

*Conditional* is section 15.2's `CONTEXT_GAP`: `목표나 구현 선택이 불명확해
prerequisite가 갈림`. A node with **two or more** distinct helpful objects that no
success criterion names has a prerequisite set that branches and a goal that has
not chosen — section 36.6's two paths, where the design document's own answer is
that the user picks. One helpful edge is not a branch and stays excluded.

## The root, its ancestors, and the tie

`expand` is breadth-first, so the first time a concept is reached is at its
shallowest depth and step 4's `최초의` needs no tie-break. The roots are every
strong-deficit candidate at the shallowest depth one was found, and every one of
them that ties on confidence there.

`AncestorImpact` carries the ancestor, its distance above the root and the
weakest hop between them. It carries **no evidence and no state**: the ancestors
are affected because the root is short, not because anything was observed about
them, and an impact that carried the root's evidence would be the misattribution
this crate spends its guards on, with the direction reversed.

When more than one root survives, `GapCase::of` **refuses** a case with no
diagnostic. It does not choose. That line is drawn here for the reason `P2-R4`'s
`ClassificationConflict`, `P2-R3`'s `ImplementationDrift` and `P2-N2`'s conflict
card draw it: a tie is information about the evidence, and resolving it by rule
discards that information silently. The `TieDiagnostic` names every tied
candidate, is shaped like section 15.2's own `사용자 확인 또는 diagnostic`, and may
reference a `P2-N4` question — which it may only reference while that question is
`OPEN` or `REOPENED`, and which nothing in this crate can resolve.

## The specificity validator holds no words

Section 15.3's second sentence is `“데이터베이스를 더 공부하세요”는 너무 넓어
유효한 Gap 설명이 아니다`. A validator that refused that sentence by matching its
words would pass the next paraphrase, so **this crate contains no list of broad
phrases and no text comparison at all.** Every rule is a structural fact:

| Defect | The fact |
|---|---|
| `SUBJECT_CARRIES_NO_PREREQUISITE` | `Database` is a `FIELD`, and `P2-C3` says a field `carries no independent prerequisite of its own`; an `ALIAS` `never carries evidence itself` |
| `BLOCKING_PATH_DOES_NOT_REACH_SUBJECT` | the path is empty, or ends somewhere other than the subject |
| `NO_EVIDENCE_CITED` | no evidence identity |
| `REMEDIATION_UNBOUNDED` | no stated duration — section 36.4's is `25분짜리` |
| `REMEDIATION_UNCITED` | nothing to read, run or answer |
| `REMEDIATION_DOES_NOT_MATCH_KIND` | the activity is not the shape section 15.2's `예시 대응` column gives this kind |
| `ALTERNATIVE_IS_EMPTY` | an empty route list where a closed reason belongs |
| `NO_LINKED_CONTEXT` | neither a lecture nor a project |

`defects` returns **all** of them rather than the first, which is `P2-N2`'s
`blocking_reasons` shape.

`generic_advice_fails_validation` drives a fluent, plausible recommendation that
uses none of section 15.3's words, and two further rewordings with no shared
vocabulary, and observes the same seven defects each time. The stronger claim —
that the validator *cannot* be lexical — is
`the_gap_crate_holds_no_phrase_list`: every non-ASCII string literal in the
package is one of the design document's own cells, and `defects`'s whole text is
pinned and names no text operation.

`NO_LINKED_CONTEXT` is the one that constrains a caller. A concept with no
lecture and no project attachment cannot carry a gap explanation, because section
15.3 requires the field unconditionally and a gap nobody can situate in the
user's own coursework or project is the broad advice section 15.3 rejects.

## One concept's evidence never becomes another's

`P2-N2` closed this at the history: `KnowledgeStateHistory` refuses admitted
evidence linked to another concept. `P2-N3` closed the one-hop form:
`NeighborUse::direct` refuses a dated item linked to any concept but the
neighbour — and reported that the route surviving every other limit is one
concept's evidence crossing a real edge into a neighbour's reading.

**This engine descends exactly those edges.** Three routes arrive here.

**One.** `academic_knowledge_state::project` takes a slice of `EligibleEvidence`
and does not require the slice to be about one concept; the `MasteryProjection`
it returns carries no concept at all, so nothing downstream can recover which one
it was about. A gap engine that accepted a ready-made projection would have no
way to check. `ConceptState::overlay` therefore does not accept one: it takes the
*inputs* to section 13.4's four checks, runs `EligibilityOutcome::admit` itself,
and refuses any item whose resolved link names another concept — including a
blocked item, whose dossier still holds the link `BlockedEvidence` does not
retain. `one_concepts_evidence_cannot_reach_another_concepts_deficit` observes
the mixed slice being accepted one layer down before observing the refusal here.

**Two.** A `FreshnessProjection` carries its concept and every `Spillover`
carries the concept it was computed toward, so both are checked. `overlay` also
compares the declared contributions against the projection's own trace — the
count of `RelatedConceptSpillover` entries and the multiset of their bands — so a
caller cannot hand over a projection built from three contributions while
declaring one.

**Three, and it is the one this task had to find.** Section 13.3's spillover is
licensed on `REQUIRES`, `BUILDS_ON`, `RELATED_TO` and `SPECIAL_CASE_OF`, and
**two of those four are the edges this engine descends.** Section 36.4's own
worked example is the case: `Buffer Pool` is the surface concept of an active
goal, so it is the concept the user is using *now*; `Disk Page` is one `REQUIRES`
hop below it; a spillover from `Buffer Pool` across that very edge puts
`Disk Page` at `MODERATE` with no evidence of its own; and section 36.4's answer
is that `Disk Page` **is** the root gap. Reading that band as `Disk Page`'s
retrieval readiness is the surface concept's evidence deciding its own
prerequisite's deficit.

It cannot be refused at the overlay, because whether the neighbour lies on the
blocking path is not known until the descent knows the path. `SpilloverSource`
carries the neighbour and the edge, and `search` refuses with
`FreshnessRestsOnPathSpillover` naming both. It does **not** silently lower the
band and does not silently keep it: the caller re-projects that concept with
`P2-N3`'s own function and without that contribution, which keeps the concept's
own evidence rather than discarding it.
`a_band_raised_by_a_concept_on_the_blocking_path_is_refused` observes the
contamination first — the band clears `RETRIEVAL_FLOOR`, and the same concept
reads `UNKNOWN` without the contribution — and then the refusal, and then that a
contribution from a concept **off** the path is untouched.

## Two places the descent stops

An **unsettled identity** is routed to `ONTOLOGY_GAP` and not descended through:
its outgoing edges were asserted about an identity a split may have divided, so
crossing one would attribute a deeper concept's deficit to this goal through an
edge whose subject is ambiguous. An unsettled *surface* concept is refused
outright with `SurfaceIdentityUnsettled`.

A **missing reading** is refused with `NoReadingForConcept`. This engine never
guesses a state for a concept it reaches.

## What this task does not decide

* **Paths and costs.** `P2-N6` owns the AND/OR hypergraph, the cost and benefit
  vectors and the choice between routes. This crate computes no alternative
  route and ranks nothing by preference: section 15.3's `대체 경로` is filled with
  a closed graph reason, or with routes the caller supplies. Nothing here treats
  sibling prerequisites as alternatives, because siblings are conjunctive.
* **Remediation content.** Which lecture segment, which exercise and how many
  minutes are facts the system holds about a concept. They arrive on a
  `ConceptReading` and this engine only checks that they are bounded, cited and
  shaped like the response section 15.2's table gives the routed kind.
* **Blind spots.** `P2-N7`. Nothing here reports a concept the goal does not
  reach.
* **`§38`.** `P2-N5` opens and closes no gate.
