# Critical path engine

## Purpose

§16 answers *what should I study first*, and the whole design of the answer is
about refusing to say it too confidently. A single recommendation score would
make the engine's judgement unauditable and its precision false, so the answer
is a **set of undominated routes**, each carrying two vectors that are never
folded, eight constraint verdicts, and five disclosure groups that are always
present.

`academic-critical-path` is that engine. It plans around a `P2-N5` `GapCase`
rather than around a concept and a state, so a plan with no evidence-backed
prerequisite deficit behind it is not a value the entry point can be called
with.

## The counts, and where each comes from

Every count below is **measured against the design document in both
directions** by a named test, not restated here. §16's own text is the source.

| Count | What | Measured by |
|---|---|---|
| 7 | §16.2's `Cost(P)` axes | `cost_vector_has_seven_separate_components` |
| 5 | §16.2's `Benefit(P)` axes | `benefit_vector_has_five_separate_components` |
| 8 | §16.3's constraint bullets | `eight_constraints_are_enforced` |
| 5 | §16.5's disclosure groups | `five_disclosure_groups_are_always_present` |
| 4 | §16.4's path roles | `five_disclosure_groups_are_always_present` |
| 4 | §16.2's strategy names | `the_four_strategy_names_are_introduced_as_examples` |
| 4 | §16.2's cost-estimation input families | `unknown_cost_is_a_range` |

### Two readings the design document leaves open

Both are **recorded rather than reconciled**, because `t068`'s own instruction
is that every count in it is derived and unverified.

**§16.2's four strategy names are examples.** The sentence is
`“빠른 project unblock”, “학교 강의 활용”, “기초 견고성”, “낮은 불확실성” **같은**
이름으로 보여준다`, and `같은` is *such as*. `REQ-16-006`'s acceptance evidence
is `four archetype fixtures`, which fixes four as the measured number.
`NAMED_STRATEGIES` therefore has four and `STRATEGY_NAMES_ARE_EXAMPLES` records
the hedge, so a later reader adding a fifth knows the list was open and the
count came from the requirement rather than from the prose.
`the_four_strategy_names_are_introduced_as_examples` fails if the `같은`
disappears, which is when this paragraph would become stale.

**§16.3's eighth constraint and the checkpoint rule are one rule.** `t068` names
`eight_constraints_are_enforced` and
`uncertain_edge_ratio_inserts_diagnostic_checkpoint` as two acceptance rows.
§16.3 has eight bullets and the last one **is** the checkpoint:
`불확실 edge가 일정 비율을 넘을 때 diagnostic checkpoint 삽입`. A reader who
counted the two rows as two constraints would look for a ninth bullet that does
not exist. `the_eighth_constraint_is_the_checkpoint_rule` holds both halves.

## No thirteenth deterministic engine

`t068`'s `P2-N6` entry names `P2-C5` as a dependency. It does **not** say to add
a registry row, and adding one would be wrong:

- `docs/contracts/engine-harness.md` pins the registry to §28's table as an
  enumeration — *exactly the §28 table rows, in table order* — and §28
  tabulates **twelve** engines, none of them a critical path engine.
- `engine_registry_is_complete` additionally refuses anything unregistered under
  `testdata/engines/`.

So this crate claims no `engine_id`, adds no directory under
`testdata/engines/`, and proves its determinism in `P2-C5`'s own vocabulary over
its own corpus at `testdata/critical-path/` — the way the reference engine's
lives at `testdata/engine-harness-reference/`.

`the_registry_does_not_hold_a_critical_path_engine` asserts every half of that
reading against the specification and the registry themselves: §28 still has
twelve rows, the table still names no critical path engine, the registry still
holds twelve entries, `SECTION_16_CRITICAL_PATH` is not a registry name, and
nothing of this crate's sits under the harness root. **A later edit that adds
section 16 to §28's table fails there**, and this section is what it then
points at.

## The two vectors

```text
Cost(P)    = <learning_effort, refresh_effort, prerequisite_risk, uncertainty,
              calendar_delay, context_switching, opportunity_cost>
Benefit(P) = <goal_coverage, immediate_project_value, curriculum_value,
              reuse_across_goals, evidence_opportunity>
```

`CostVector` and `BenefitVector` have private fields, one constructor each
taking every axis, and one accessor each answering for one **named** axis. There
is no `total`, `sum`, `score`, `weighted`, `Ord`, `PartialOrd` or numeric
conversion anywhere. `the_vectors_cannot_be_folded` is a whole-crate statement:
no product file may name any of twelve folding spellings, and neither vector
type may derive an order.

### Units are declared, because §16.2 declares none

§16.2 fixes the axes and no unit for any of them. An interval with no unit
compares to nothing, so this crate declares one `Unit` per axis — minutes, days,
permille, occurrences — and **comparison is only ever between the same axis of
two routes**. Nothing compares two different axes, which is why no common unit
is needed and none is invented.

### An unknown cost is a range, structurally

`CostEstimate` hands out `low()` and `high()` and nothing between them. There is
no `midpoint`, no `expected` and no `value`, so *no path can fold to a point
estimate* — the narrowing operation does not exist. Aggregation along a route is
interval addition, which widens.

A `CostBasis::Unmeasured` estimate is additionally required to be **wide**:
`low < high` strictly, refused at the constructor. That is §16.2's
`근거가 없으면 범위로 표시한다` as a type rule rather than a rendering
convention. Adding a measured estimate to an unmeasured one yields an
**unmeasured** sum: the basis is not laundered.

`CostBasis::Measured` names which of §16.2's four input families the estimate
actually read — `사용자 state/freshness`, `concept granularity`,
`이용 가능한 resource`, `과거 실제 학습 속도` — and a basis that read none of
them is not `Measured`.

## The pipeline, and why its order is a type

```text
satisfy → cost → constrain → eliminate → order
```

- `satisfying_sets` returns **sets**, not walks.
- `evaluate` returns `[ConstraintFinding; 8]`, one per §16.3 bullet, in order.
- A route any constraint refuses is partitioned out **before** elimination, so a
  refused route cannot dominate a feasible one. It is visible in §16.5's third
  disclosure group, which is what that group is for.
- `ParetoFront::eliminate` is the **only** constructor of a `ParetoFront`.
- `rank` takes `&ParetoFront` and takes nothing else.

So §16.2's `engine은 먼저 Pareto-dominated path를 제거하고` is a type rather than
a comment: a caller cannot assemble a front that skipped elimination and hand it
to the ranker. `crates/critical-path/tests/compile_fail/` holds the compiled
half.

### Domination on intervals never forms a midpoint

A cost axis of `A` weakly beats `B`'s when **both** ends are no worse. Two
intervals whose ends cross — `[10, 40]` against `[20, 30]` — are
**incomparable**, and neither route is dominated on that axis in either
direction. That is what keeps the relation genuinely partial: collapsing each
interval to a midpoint would make every pair comparable and would delete an
alternative on the strength of a number nobody measured. `REQ-16-005` leaves
`incomparable/uncertain vector dominance` open; this is the reading, and it errs
toward keeping a route.

### A preference is a permutation, not a weight vector

`PreferenceSlider` is a **complete priority order over all twelve axes**.
Comparison walks them in that order and takes the first that separates two
routes; on a cost axis lower wins and on a benefit axis higher wins, each on
both interval ends. **No arithmetic combines two axes anywhere.** Weights would
multiply and add, and the product would be a single number whose value depended
on the preference — exactly the collapse §16.2 forbids.

The permutation must be complete: `PreferenceSlider::of` refuses an order that
omits or repeats an axis, because an omitted axis is a silent decision that it
does not matter, which is a change to the answer and not to its order.

There is **no `Default`**. A shipped neutral preference would be the engine
answering the ordering question on the user's behalf, and §16.5's closing
paragraph is that the engine recommends and the user chooses. That is `P2-N3`'s
`PersonalizationSpeed has no Default` applied to §16.2.

`Ranking<'a>` **borrows** its front. A shared reference plus no interior
mutability anywhere in the crate is the whole guarantee that a slider cannot
rewrite a fact, and `the_preference_layer_cannot_reach_a_vector` adds the
structural half: `preference.rs` names none of `CostVector`, `BenefitVector`,
`CostEstimate` or `CostBasis`, so it cannot build one.

### A named strategy is a slider and nothing more

`NamedStrategy::slider` is its entire output. A route earns a name when ordering
the whole surviving set under that strategy's own slider puts it first, so a
name is a **report of an ordering** and never an input to one.

## Satisfaction, not shortest path

§16.1's own shape:

```text
Goal
  REQUIRES ALL [a, b]
  REQUIRES ONE OF
    ├─ [c]
    └─ [d, e]
```

Two hyperedge shapes and no third. A `REQUIRES ONE OF` with fewer than two
branches is refused: it is a conjunction wearing the other shape's name, and
§16.4's point is that the user can see there is a choice.

A shortest-path algorithm minimises a length over a sequence of arcs, and two
things go wrong:

- **A conjunction is not a choice.** The first arc out of the goal reaches one
  member of `REQUIRES ALL` and a path algorithm stops because it arrived. The
  answer is a set, and every member is required.
- **A disjunction is not the cheapest member.** A branch is taken **whole**, so
  the branch with the single cheapest node can be strictly worse.

`shortest_by_node_count` implements the naive answer **once**, in a function the
product path cannot reach, so `and_or_hypergraph_is_satisfied_not_shortest`
compares the two answers on one graph and shows they differ rather than
describing the difference. On the fixture graph the naive walk omits a mandatory
member and is a strict subset of a satisfying set.

Hyperedge members are `P2-N5`'s admitted `PrerequisiteEdge` values, so eighteen
of §7.2's twenty predicates have no value of that type at all and this crate
holds no allowlist. `RELATED_TO` is refused in `P2-C4`'s registry.

## The eight constraints

| # | §16.3's bullet | What decides it |
|---|---|---|
| 1 | `hard prerequisite satisfaction` | the satisfying set exists |
| 2 | `현재/미래 CourseOffering의 확인 상태와 선수과목` | `P2-U1`'s `OfferingStatus`, plus `OfficialPrerequisiteStanding` |
| 3 | `학기 시간표·학점 한도` | `P2-U1`'s `Meeting` overlap and `Credits` against a ceiling |
| 4 | `project deadline 또는 목표 horizon` | the calendar-delay interval's **high** end |
| 5 | `privacy상 사용할 수 없는 provider/resource` | forbidden evidence sources on the route's options |
| 6 | `사용자가 제외한 분야·과목·학습 방식` | excluded concepts and offerings |
| 7 | `stale concept의 최소 refresh requirement` | `P2-N3`'s band |
| 8 | `불확실 edge가 일정 비율을 넘을 때 diagnostic checkpoint 삽입` | the uncertain-edge ratio |

**Bullets one and two are different prerequisites.** One is a *concept*
prerequisite — `P2-C4`'s `REQUIRES` at `HARD`. Two ends in `선수과목`, which is a
*course* prerequisite out of the official catalogue, and §28's
`OFFICIAL_PREREQUISITE` engine's own invariant is `AI-inferred 선수지식과 분리`.
They are two constraints with two inputs, and `OfficialPrerequisiteStanding` has
an `Unknown` value because that engine is `PLANNED`: folding its silence into a
pass is how a plan recommends a course the registrar refuses.

`ConstraintVerdict::Unknown` is likewise a value and not a missing answer. §28's
graduation-audit invariant is `unknown을 pass/fail로 강제하지 않음`, and this is
the same refusal in this engine: an `Unknown` route is disclosed and is neither
silently admitted nor silently dropped. `HistoricallyLikely` and `Uncertain`
offerings reach `Unknown`, which is §36.7's *do not confuse an official course
with an offering's actual coverage*.

`evaluate` returns a fixed-size array with no filter, no `Option` and no early
return, so a plan cannot be produced with a constraint unanswered.

### Bullet seven names exactly one band

`STALE`, and not `UNKNOWN`. §13.3's gloss is
`STALE: 과거 evidence는 있으나 최근성 낮음` and §15.2's table row is
`mastery evidence는 있으나 stale`, so a refresh has something to refresh.
`P2-N3`'s own module note says `UNKNOWN` *is not "very stale": it is the band
for a concept about which nothing datable was ever admitted*, and inserting a
refresh requirement for a concept with no record is a step the user cannot
perform. That concept's answer is `P2-N5`'s evidence gap.

### Bullet eight: the ratio, its denominator and its threshold

`REQ-16-017` leaves `비율 임계치·계산 분모` open. Both are closed here and both
are pinned:

- The **denominator** is the satisfying set's own members, not every edge in the
  graph. An edge on a branch the route did not take is not an assumption the
  route rests on, so counting it would make a route's uncertainty depend on how
  many alternatives the graph happens to hold.
- The **threshold** is `300` permille and the comparison is `넘을 때` —
  strictly above. A route exactly at the threshold gets no checkpoint.

The checkpoint is attached to the plan's **first** step, which is
`REQ-16-017`'s `before branch commitment`.

## A course is an acquisition option

`AcquisitionOption` has three variants — a course, self-study, and project work —
and hands out `Opportunity` values, each naming a concept and an occasion. It has
**no function returning a mastery level, a knowledge state, or a satisfied
concept**, and nothing in this crate can produce an `EligibleEvidence` or a
`KnowledgeStateAssertion`. So a plan that takes a course still reports the
concept as one the goal needs; the course changes the route's
`evidence_opportunity` benefit and its `calendar_delay` cost, and there is no
code path by which it could change what the user knows.

A course that bundles no exposure or no practice is refused: §16.2 calls a course
`여러 exposure/practice 기회를 묶은` option, and one that bundles neither is being
modelled as an acquisition.

§36.7 is the design document's own worked case — `isolation` is supported by the
current offering, `idempotent API design` is not, and external reading plus a
project experiment is the better option — so all three live in one enumeration
and a plan chooses between them by vector rather than by kind.

## Counterfactual and edit

`sensitivity` removes one hyperedge member and runs the **same solver** again. It
does not estimate what would change; it computes it, which is the only reading
under which `counterfactual_shows_edge_sensitivity` can fail when the solver is
wrong. §34.5's detection column names `sensitivity analysis` for that reason.

`EdgeOutcome` separates *the goal becomes unsatisfiable*, *fewer routes*, *routes
lose a concept* and *no change*. Collapsing a branch is `FewerRoutes`: the
surviving branch requires more, not less, so nothing that every route needed
stops being needed.

`EditedPlan` keeps the base. A second edit still answers `base()` with the plan
before the **first**, not with the previous recomputation, and the edits are a
list. That is `CONTRIBUTING.md` rule 2 — canonical values are append-only and a
correction is a new entry — and §34.5's `old path 보존`, and the discipline
`P2-R4`'s `ClassificationConflict`, `P2-R3`'s `ImplementationDrift` and `P2-N5`'s
retained tied roots already set. Nothing here takes `&mut`.

## The five disclosure groups

`Disclosure` has five private fields, one constructor taking all five, no
`Default`, and `CriticalPathResult` holds one **by value**. A result with a
missing group is not a value this crate can produce.

| Group | §16.5's words | Can it be empty |
|---|---|---|
| 1 | `계산 snapshot` | no |
| 2 | `비용 가정` | no — one entry per cost axis |
| 3 | `제외된 목표` | yes, as `NoneExcluded` |
| 4 | `불확실 edge` | yes, as `AllSettled` |
| 5 | `대안` | yes, as `SoleSurvivingRoute` or `NoFeasibleRoute` |

`REQ-16-022` allows `explicit empty lists ... with reason`, so the three that can
be empty are enumerations whose empty case **names its reason** rather than lists
that are sometimes empty. That is `P2-N5`'s `AlternativePath::None { reason }`
applied to §16.5, and it is what stops `제외된 목표: []` from being ambiguous
between *nothing was excluded* and *nobody checked*.

**Group two is about the answer's own route.** `CostAssumptions` carries one
entry per §16.2 cost axis of the **ranked-first** route, not of every surviving
route: §16.5 asks for the assumptions the answer rests on, and the answer's
route is the one it is offering. Group five names the alternatives by their
concepts, rank, strategy and sources but **not** by their vectors, so a surface
that wants to show a user two routes' costs side by side reads them off
`CriticalPathResult::front()` rather than off the disclosure. That is a scope
line rather than an omission, and it is stated here so a later reader does not
take an alternative's absent cost for a missing measurement.

## Determinism

`frozen_inputs` renders a run's identity into `P2-C5`'s canonical `key=value`
encoding — every key built from a position, every value a count, an interval end
or a hex identity, and an unmeasured basis as `unknown` rather than a zero.
`outcome` renders the answer into a `ProofNode` tree with one node per §16 stage
and one child per §16.3 constraint, and an `ExplanationSnapshot` rendered from
the result rather than accepted alongside it.

**The frozen inputs carry everything that can change the answer**: the goal, the
whole hypergraph's shape, every axis interval and its basis, the slider's whole
order, every acquisition option with its credits, its offering standing and its
occasions, and every one of §16.3's constraint inputs with its own count so an
empty list is distinguishable from an absent one. That completeness *is* the
contract, not a convenience: an input the encoding omits makes the engine **not
a function of its frozen inputs**, and two runs that differ only in it then share
a digest while their canonical bytes differ.

`the_frozen_inputs_are_the_runs_identity` asserts exactly that, in two forms — no
two corpus cases may share a digest, and changing any one constraint input, any
edge standing or any acquisition option must move it. The first version of this
bridge failed the first form: it rendered the goal, the intervals and the slider
and nothing else, so `two_routes` and `sole_route` had byte-identical `.input`
files and different `.expected` files, and the determinism suite passed anyway
because it only ever compared a case with itself.

`same_inputs_and_rule_hash_yield_byte_equal_results` asserts both halves: equal
bytes under one rule-set hash, and **different** bytes under another and under a
different engine version. Without the second half the first would pass on an
encoding that ignored either.

`the_committed_corpus_matches_a_fresh_render` re-renders the whole corpus from
the single builder in `crates/critical-path/tests/corpus/mod.rs` — which
`examples/emit_corpus.rs` also uses, so the two halves cannot drift — and
compares the directory listing as well, so a stale case left behind is visible.
`the_corpus_cases_are_not_all_the_same_shape` requires the four cases to produce
four different byte strings and at least three different route counts, so the
byte comparison is not passing on four copies of one answer.

The corpus is synthetic end to end: every identifier is a SHA-256 of its own
name with the UUIDv7 nibbles set, every instant is an offset from `P2-N5`'s
`ORIGIN`, and the `GapCase` the cases plan around is produced by driving
`P2-N5`'s real `search` over a `P2-L4` document that a real `P2-L2` capture and a
real `P2-L3` run produced.

## What this task does not decide

- **What a concept costs.** §16.2's four estimation input families arrive as a
  `CostBasis` on an estimate the caller supplies. This crate compares intervals;
  it measures none.
- **Whether an offering runs.** `P2-U1` owns §8.3's four standings.
- **Whether the registrar's prerequisites are met.** §28's
  `OFFICIAL_PREREQUISITE` engine is `PLANNED`.
- **A semester.** `P2-N8` owns `PlanScenario` and the deterministic/projected
  split.
- **Persistence.** Nothing is written. There is no migration and no
  `academic-store` edge. The crate opens no file, opens no socket and reads no
  clock.
- **`§38`.** `P2-N6` opens and closes no gate.

## Enforcement

- `cargo test -p academic-critical-path --test critical_path` — the thirteen
  named acceptance rows plus five more.
- `cargo test -p academic-critical-path --test critical_path_harness` — the
  determinism half.
- `cargo test -p academic-critical-path --test critical_path_scans` — the source
  scans, enumerated in [the policy source scans](policy-source-scans.md).
- `cargo test -p academic-critical-path --test compile_fail` — the eight limits
  that are types.
- `cargo run -p academic-critical-path --example emit_corpus` — regenerates
  `testdata/critical-path/` from the deterministic builder.
