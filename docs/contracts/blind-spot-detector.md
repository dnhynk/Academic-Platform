# Blind spots, coverage, dispositions and neutral copy

`academic-blind-spot` is the `P2-N7` boundary. It is section 23: the five states
with one refusing precondition each, the field coverage that counts admitted
evidence and never becomes a mastery score, the granularity, window and exposure
minimum the user selects, the four dispositions that outlive a rerun, and — for
each pressure section 34.5 names — the value that does not exist rather than the
check somebody has to remember to run.

It sits on two boundaries and restates neither. `academic-domain` already
declares `FreshnessBand`, `EntityKind` and `P2-N1`'s three primary node types, so
this crate declares no second tier vocabulary and computes no band.
`academic-knowledge-state` already decided what evidence is admissible, so an
`ExposureItem` wraps an `EligibleEvidence` and has no other constructor. It opens
no file, opens no socket, reads no clock, persists nothing, adds no migration,
and has no edge to `academic-store`.

## The screen this is

Section 23 is the feature that tells a user what their record cannot be read
for. Section 34.5's failure mode for it is `Blind Spot을 공부 압박으로 변환` —
turning a blind spot into study pressure — whose cause is
`모든 taxonomy 영역의 균등 coverage 목표` and whose impact is `불안·목표 이탈`.
Everything below is that row's prevention column made structural.

## The proofs are types

| Section 23 rule | What holds it |
|---|---|
| the five states mean five different things | `StateBasis` has one payload per state, each with private fields and a refusing constructor, and `state_of` is a bijection onto `BLIND_SPOT_STATES` |
| coverage may reach only three of the five | `ExposureClass` has three variants and no `OutOfScope`, no `Gap` |
| `UNOBSERVED` says ability cannot be inferred | `headline` answers section 23's own replacement phrase and no string this crate emits is the claim it replaces |
| coverage never becomes a mastery score | the crate has no name for a mastery level; `FieldCoverage` and `EvidenceDiversity` derive neither `PartialOrd` nor `Ord` |
| granularity, window and threshold are the user's | `BlindSpotScope` has no `Default`, no constant of its type exists, and `select` takes all four by value |
| a disposition is the user's and cannot be cleared | `UserDispositionChoice::verify` runs ADR-003's actor matrix; `DispositionLedger` has no removal method and no `&mut self` method |
| `NOT_RELEVANT` survives an AI rerun | `detect` is this crate's only producer of a finding and reads the ledger first |
| no goal to equalise coverage is generated | there is no `academic-gap` edge, so a goal this engine emitted would first have to be a goal it could name |
| a blind spot is never a warning | the crate has no name for a warning presentation; `EMPHASIS` is one token |
| low relevance demands no action | `NeutralPresentation` has no field an action could occupy |
| `EXPLORE` opens one bounded path | `TastePath` holds one `TasteStep` and not a list |

`crates/blind-spot/tests/compile_fail/` holds the compiled half: eleven cases,
each a program that fails with a committed diagnostic.

## Section 23's five states, measured rather than restated

`BLIND_SPOT_STATES` is the five in the order section 23's own `text` block writes
them, and `meaning` carries each line's right-hand cell verbatim.
`five_states_are_semantically_distinct` reads that block back out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares name and cell
against the enumeration in both directions, so **five is a measurement of the
design document**. The same test reads the schema block's own `key:` lines and
compares them against `FINDING_FIELDS`, so eight is too.

Five names in one enumeration are five names. What makes them *semantically*
distinct is `StateBasis`: one payload per state, each refusing the facts its
state is not.

| Basis | What its constructor refuses |
|---|---|
| `BelowMinimum` | an observed count at or above the user's minimum |
| `ObservedDifficulty` | an empty list of failed attempts |
| `LowRecency` | any band outside `LOW_RECENCY_BANDS` |
| `ScopeExclusion` | a disposition that is not `NOT_RELEVANT` |
| `GoalBlock` | a goal that is its own blocking concept |

`state_of` is a total match with no wildcard arm and the map is a bijection,
which the acceptance case measures in both directions.

### Two of the five are not the evidence's to decide

`ExposureClass` is the three states an evidence reading may yield. It has no
`OutOfScope` and no `Gap`, and `EXPOSURE_CLASSES` is compared against
`BLIND_SPOT_STATES` as a set so the complement is measured to be exactly those
two.

* `OUT_OF_SCOPE` is `사용자가 현재 탐색하지 않기로 함` and is reachable only
  through `ScopeExclusion::of`, which takes a `UserDispositionChoice` — a value
  ADR-003's matrix refuses every automatic actor.
* `GAP` is `활성 목표를 실제로 막음`, which is `P2-N5`'s question. This crate has
  no `academic-gap` edge, so a `GAP` minted out of a coverage reading is not an
  omission somebody has to check for: it is a value `ExposureClass` cannot
  express.

`REQ-23-007` is closed on both sides — `P2-N5` holds *only a blocked active goal
is a gap*, and this crate holds *a coverage reading is never one*.

## `UNOBSERVED` is not a claim about the person

Section 23's UX bullet: `"약하다" 대신 "판단할 exposure가 없다"고 쓴다`.
`unobserved_says_cannot_infer_ability` reads both halves of that sentence out of
the bullet's own typographic quotes and compares them against
`CLAIM_ABOUT_THE_PERSON` and `CANNOT_INFER_ABILITY` in order, so the pair is one
measurement of one sentence rather than two constants that could drift apart.
`headline(Unobserved)` is the replacement phrase, and every string a finding can
render is required not to contain the claim it replaces.

This is `P2-N2`'s `UNSEEN` discipline on a second axis: there, `evidence 없음이지
"모른다"는 시험 결과가 아님`; here, an absent record is not a verdict either.

## Coverage, and why it cannot become a mastery

Not because a function declines to convert it. **This crate has no name for a
mastery level.** `academic-knowledge-state` is a product edge and hands out
`LADDER`, `rung`, `level_token`, `AutomaticLevel` and `MasteryProjection`;
`academic_domain::MasteryLevel` is one `use` away. Nothing here reaches for any
of them, and `the_blind_spot_crate_cannot_name_a_mastery` says so as four
whole-set comparisons in both directions plus an eight-name refusal over every
product file and again over every public signature, with a control: the same
reader is required to find at least five of those eight in `P2-N2`'s own
`ladder.rs`, so the zero it reports here is a measurement. `P2-N3` holds section
1's fifth invariant with the identical shape on the freshness axis.

Beside it, a twelve-spelling refusal of every way to fold a reading into one
number, and a derive-attribute read that requires `FieldCoverage` and
`EvidenceDiversity` to implement neither `PartialOrd` nor `Ord`.

### `존재` is admitted evidence and the source is the other axis

`ExposureItem` wraps an `EligibleEvidence`, so the items counted are the items
that passed section 13.4's four checks. A second admissibility rule here would be
a second ladder, which is `P2-N3`'s reason for dating the same value.

`EXPOSURE_SOURCES` is section 23's own `강의·과제·project·질문·사용자 확인`, read
back out of the coverage sentence by splitting the run before `evidence의` on its
own `·` separators and compared in both directions. The source is **provenance**
and does not come from section 13.2's row: section 13.2 says what a piece of
evidence licenses, section 23's five say where it came from, and one exercise can
arrive from a lecture or from an assignment. It arrives as an argument, the way
section 7.4's tier arrives as an argument in `P2-N2`.

`WEAK` is read off `P2-N2`'s own `Outcome::Failed`. That crate keeps a failed
attempt as evidence and refuses it a promotion; this reads the same fact on the
other axis, so `시도·평가 evidence에서 어려움이 관찰됨` is not a second reading of
difficulty.

### An item about another key is refused rather than absorbed

`FieldCoverage::of` refuses an item whose entity resolves to a different
aggregation key, and refuses one the selected taxonomy release does not hold at
all. `P2-N2` found this defect one layer up — an `APPLIED` state for one concept
projected out of another concept's admitted evidence — and `P2-N3` found the
one-hop form. A count is the third place the same mistake fits, because a count
absorbs a wrong item silently.

### The diversity scale is two names and one of them is not the document's

Section 23 exhibits exactly one token, `LOW`, beside
`exposureEvidenceCount: 1`. One item carries one source, so `LOW` holds at one
distinct source; `EvidenceDiversity::Mixed` is **this crate's name** for its
complement. The acceptance case reads the example's own count and token out of
the schema block and requires the reading built from one item to reproduce both,
so the split point is measured even though the second name is not the document's.

## The scope has no shipped value

`BlindSpotScope::select` takes the taxonomy version, the granularity, the window
and the exposure minimum by value. There is no `Default` and no constant of the
type in this crate — `P2-N3` holds `GATE-38-024`'s personalization half the same
way, and `P2-N1` holds the base taxonomy mix the same way again — and
`no_public_function_mutates_in_place` compares the whole set of product files
naming `Default` against a one-entry pin, so a `Default` for the scope is an
extra key rather than an addition nobody looked at.

**The fourth choice is the one section 23 leaves least specified and it decides
the most.** `evidence가 거의 없어` is a threshold; `t001` flagged it as a gate
candidate and section 23 fixes no number. It is therefore the user's, in the same
value that carries the granularity and the window, and
`granularity_and_window_are_user_selected` drives the same field to `UNOBSERVED`
under one minimum and to no finding at all under another. A minimum of zero is
refused, because under it no field can ever be `UNOBSERVED` and the detector says
nothing.

`BlindSpotScope::label` assembles section 23's own `scope:` line from the release
identifier and the window token, and the acceptance case requires it to reproduce
the example's `undergraduate CS breadth v2, all-time` exactly.

## The four dispositions, and the fifth spelling that is not one

`DISPOSITIONS` is `EXPLORE`, `LATER`, `NOT_RELEVANT`, `HIDE_UNTIL`, read back out
of section 23's own bullet by splitting it on its back-quoted spellings and
compared in both directions, so **four is a measurement**.

**Section 23's schema example writes a fifth spelling and it is not a fifth
disposition.** The example ends with
`userDisposition: ACKNOWLEDGED_NOT_CURRENTLY_RELEVANT`, which the UX bullet does
not list and which `REQ-23-014` does not name. The bullet is the normative
enumeration, because it is the half that fixes the identifiers the user picks
from. `SCHEMA_EXAMPLE_DISPOSITION` keeps the example's spelling and
`four_dispositions_are_durable` reads it back out of the document and requires it
**not** to be one of the four, so the discrepancy is a measured value with a test
on it rather than one a later reader rediscovers. Nothing routes on it.

`UserDispositionChoice::verify` is the only constructor and runs ADR-003's actor
matrix. The acceptance case drives every one of the four against a model run
holding the user pairing, a model run holding its own valid pairing, a
deterministic engine and an importer, and only the clean user pairing produces
the value. Exactly one of the four takes a deadline, and the rule runs in both
directions.

`DispositionLedger` has no removal method — the scan refuses `remove`, `clear`,
`retain`, `drain`, `take` and `delete` in that module — and every operation that
changes it consumes it and returns a new one. `record` refuses a choice that is
not newer than the standing one, so a rerun replaying an older claim cannot undo
a later decision.

## `NOT_RELEVANT` survives a rerun, and there is only one rerun

Section 25.12: `NOT_RELEVANT는 존중되며 새로운 AI run이 경고를 되살리지 않는다`.

The behavioural half drives five reruns against a standing `NOT_RELEVANT` —
no evidence, new evidence clearing the minimum, a failed attempt, a stale band,
and a blocked active goal — and each keeps the classification, the disposition
and the suppressed warning. **The control is the half that makes it evidence**:
the same five inputs with an empty ledger classify as `UNOBSERVED`, no finding,
`WEAK`, `STALE` and `GAP` respectively, so the sweep is measuring the ledger.

The structural half is `the_finding_has_exactly_one_producer`: every public
signature in the package whose return type names `BlindSpotFinding` is extracted
and compared against a one-entry pin in both directions, and the private
constructor's call sites are compared against a two-file pin. A second
recomputation path added later is a failure until it is listed and driven.

So the three ways a rerun could resurrect a warning are respectively a claim no
model actor can make, a ledger operation that does not exist, and the first step
of the classification order.

### The order, and the two steps in it that are decisions

```text
NOT_RELEVANT standing        -> OUT_OF_SCOPE
P2-N5 carried a goal block   -> GAP
count below the user minimum -> UNOBSERVED
an admitted attempt failed   -> WEAK
P2-N3's band is low          -> STALE
otherwise                    -> no finding at all
```

**`NOT_RELEVANT` outranks everything, including `GAP`.** Section 3 lists
`특정 Blind Spot을 탐색할지 의도적으로 제외할지` among the decisions the user
owns, and section 23 says the exclusion is `user scope에서 제외된다` rather than a
state the detector overrides. `P2-N5` still reports the gap in its own lane; this
view does not reopen a scope the user closed.

**A key that is adequately covered, undamaged and fresh produces no finding.**
A detector that emitted a row per key would be exactly the endless deficit list
section 34.5's failure mode describes.

The whole function is pinned as a whitespace-collapsed constant, so every step of
the order is refused against a later edit rather than only the steps somebody
thought to assert.

## `HIDE_UNTIL` expires and `NOT_RELEVANT` does not

`suppresses_warning_at` is a total match with no wildcard arm and is pinned
whole. `NOT_RELEVANT` never stops suppressing — section 39's
`경고와 추천에서 제외한다` has no expiry — and `HIDE_UNTIL` stops the moment the
clock reaches its deadline.

The clock arrives as an argument; this crate reads none.
`hide_until_reappears_after_clock_advance` sweeps five instants across the
deadline and observes the warning return, observes the finding and the
disposition surviving at every one of them — section 39's
`evidence 부족 분류와 user disposition을 별도 저장한다` — and runs the same sweep
twice more: once with `NOT_RELEVANT`, which never warns, and once with no
disposition at all, which warns at every instant, so the middle sweep measures
the deadline rather than a detector that never warns.

## The skew explanation is a distribution, not a sentence

Section 23's schema example writes
`likelyCause: "course/project choices concentrated in backend"`, and the
paragraph below it says what that sentence summarises: `backend repo 세 개와
Database/Networks 강의 때문에 Application/Backend evidence가 많고
Graphics/Formal Methods가 비어 있음`. That is counts, crowded keys and empty ones.

**This crate carries the distribution and not the sentence, and that is a
deliberate deviation from the example's field type.** A free-text cause is the
one slot in a finding through which an action demand could arrive, and section
23's last sentence is that when a blind spot is unrelated to the user's goals the
product must not make one. A word list cannot hold that — every list admits the
sentence spelled differently — so the slot does not exist. The wire keeps section
23's own `likelyCause` field name.

Neither bound in it is a threshold this crate chose: `concentrated` is the keys
holding the **maximum** count with every tie retained, which is `P2-N5`'s
`equal candidates are both retained` on a different axis, and `sparse` is the
keys below the minimum **the user selected**.

## Neutral copy, checked against the design document rather than a word list

Section 23: `관련성이 낮은 Blind Spot은 warning red가 아니라 중립 outline으로
표시한다`, and section 34.5's prevention column for the whole failure mode is
`neutral UI`. So **this crate has no name for a warning presentation**: there is
no `WarningRed`, no severity and no alert level, `EMPHASIS` is one token, and the
scan refuses eight warning spellings over every product file.

`low_relevance_uses_neutral_tokens` compares the **whole set** of strings a
`NeutralPresentation` can render — five headlines and section 34.5's
`실력 판단 불가` — against the design document in both directions: each is
required to occur verbatim in the document, and the document's own five state
cells plus that phrase are required to be exactly the six. A copy edit that
demands an action is then refused for a reason that does not depend on its
wording — it is a sentence the design document does not contain — which is what a
forbidden-word list cannot do. The case carries three demanding sentences spelled
with none of the words any list here holds and requires each to be neither in the
document nor renderable, so the reader is not one that always answers yes.

The structural half is `the_presentation_carries_no_free_text`: both presentation
types are pinned whole, so a `String` field added to either is a failure rather
than a slot nobody looked at.

**And low relevance demands nothing because there is no field it could demand
through.** `FindingPresentation::Neutral` has no action slot at all; only
`Explore` carries one, and its `TastePath` needs the user's own verified choice.

`GoalRelevance` carries the active goals that reach a key and `LOW` is the empty
case — the only reading under which the example's `LOW` and its `likelyCause`
are the same fact. **`RELATED` is this crate's name for the complement**; the
design document spells only `LOW`.

## `EXPLORE` opens one step, and only the user opens it

`TASTE_STEPS` is section 23's `한 강의, 한 chapter, 한 toy experiment`, read back
out of the run between the bullet's em dashes and compared in both directions.

A `TastePath` holds **one** `TasteStep` and not a list, so a path of two lectures
is not a value refused at the end of a constructor — it is a value that cannot be
written. `TastePath::for_explore` refuses the other three dispositions and a key
the choice does not name. Through the engine, `EXPLORE` with no step offered is
refused rather than silently neutral, and section 37's
`다시 neutral 상태로 둘 수 있다` is driven: moving off `EXPLORE` takes the path
with it.

## A name list is not a closure, and this crate measured it

Five of the rules above are held in the scan file by lists of forbidden
spellings: eight mastery names, twelve score-folding names, eight warning names,
eight goal names, six ledger-removal names. **Five injections spelled none of
them and passed every one.** Each was a public function:

* `FieldCoverage::as_permille`, folding the count into one number;
* `NeutralPresentation::nudge`, returning a demanding sentence;
* `DispositionLedger::without`, dropping a standing choice through `filter`
  rather than `remove`;
* `BlindSpotScope::recommended`, shipping a scope with a minimum chosen here;
* `GoalRelevance::as_recommendation`, whose lowercase name the capitalised goal
  list does not match.

`every_public_signature_is_in_the_inventory` closes all five at once: the whole
set of this crate's public functions, as module, name and return type, compared
against `PUBLIC_SIGNATURES` in both directions. A function that hands out a
number, a sentence, a scope or a finding is then an **extra key** whatever it is
called, which is what a list of names cannot be. `P2-RF13` and `P2-RF15` found
seven real leaks by making the same substitution on a different axis.

The same shape was probed one step out. `academic-critical-path` holds *a vector
is never collapsed to a scalar in the API* with twelve folding spellings, and a
public `CostVector` method summing all seven axes and named `as_one_number`
passed that crate's whole suite and `clippy` alike. It is closed there the same
way, and `docs/contracts/critical-path.md` records it.

## Where the section 38 gates stand

`P2-N7` opens and closes none. `GATE-38-022`'s base taxonomy mix stays `P2-N1`'s
open question — this crate selects no taxonomy and takes the version identity as
an argument — and `GATE-38-024`'s freshness priors stay `P2-N3`'s.

## Named acceptance evidence

`cargo test -p academic-blind-spot` executes:

| Test | Evidence |
|---|---|
| `five_states_are_semantically_distinct` | section 23's five block lines with both cells measured in both directions, the five bases mapping bijectively onto them, each payload's own refusals, the complement `ExposureClass` cannot reach, and the schema block's eight keys |
| `unobserved_says_cannot_infer_ability` | both halves of section 23's replacement sentence read out of its own quotes, no renderable string containing the claim it replaces, a real `UNOBSERVED` finding presenting the replacement, and section 34.5's uncertainty cell |
| `coverage_never_becomes_mastery` | section 23's five sources measured from its own sentence in both directions, a field carrying all five, the example's own count and diversity token reproduced, and both misattribution refusals |
| `granularity_and_window_are_user_selected` | one item aggregating under three different keys at the three granularities, a node above the tier resolving to nothing, the window excluding rather than reweighting, the example's scope string reproduced, both refusals, and the same field classified differently under two minima |
| `four_dispositions_are_durable` | section 23's four measured from its bullet, the example's fifth spelling measured and refused as a disposition, four automatic actor pairings refused for each of the four, the deadline rule in both directions, three recomputations preserving each, and a replayed older claim refused |
| `hide_until_reappears_after_clock_advance` | five instants swept across the deadline, the finding and the disposition surviving every one, `NOT_RELEVANT` never warning over the same sweep, and a no-disposition control warning at every instant |
| `not_relevant_survives_ai_rerun` | five reruns that would each classify differently, the classification, disposition and suppression surviving all five, the five-state control that makes it a measurement, a replayed choice refused, and a wire round-trip carrying section 23's eight keys |
| `no_equalize_all_goal_is_generated` | section 23's and section 34.5's own sentences, the most skewed distribution this taxonomy holds, the crowded key producing no finding, the finding's whole wire key set compared against the eight in both directions, and the skew as counts rather than prose |
| `low_relevance_uses_neutral_tokens` | the whole set of renderable copy compared against the design document in both directions, three demanding sentences spelled with no listed word required to be neither in the document nor renderable, the neutral emphasis and the uncertainty disclosure, and the related case discloses the other token and still demands nothing |
| `explore_creates_one_bounded_taste_path` | section 23's three steps measured from its bullet, the other three dispositions refused, another key refused, one step through the engine, `EXPLORE` with no step refused, and the return to neutral taking the path with it |

Beside them: the twelve source scans in
`crates/blind-spot/tests/blind_spot_scans.rs` and the eleven `compile_fail`
cases.

All fixtures are synthetic and built in process. The lecture evidence is a node
of a document `P2-L4`'s builder produced over a real `P2-L2` capture and a real
`P2-L3` run. Every operation is local and deterministic.

## What this task does not decide

* **Gaps.** `P2-N5` owns whether an active goal is actually blocked. This crate
  carries its finding and computes nothing about goals.
* **Freshness.** `P2-N3` owns the six bands and their inputs. This crate carries
  one and computes none.
* **Mastery.** `P2-N2` owns the ladder, and this crate cannot name it.
* **The ontology.** `P2-N1` owns the tiers and the taxonomy version identity;
  both arrive as arguments and no registry is held here.
* **Persistence.** Nothing here is written. There is no migration and no edge to
  `academic-store`.
