# The what-if semester simulator

`P2-N8`. Section 22's `PlanScenario`: the screen a user asks *what happens if I
take this*, and the line between what it may state and what it may only guess.

The whole crate is one sentence. **An assumption must not leak into the record.**
A product that let it would be editing the user's own history to match a plan
they were only considering, which is what `INV-C-009` — *projections never write
actual state* — is about.

`academic-what-if` is the package. It persists nothing, opens no file, opens no
socket, reads no clock, adds no migration and registers no deterministic engine.

## What holds section 22, and where

| Section 22 rule | What holds it |
|---|---|
| facts and assumptions are frozen apart | `ScenarioBasis` holds section 22.1's four `basedOn` references; `PlanAssumptions` holds its three assumptions; `PlanInputs` holds everything else the engine reads |
| the two lanes are separate data types | `DeterministicResults` and `ProjectedResults` share no field, no constructor and no conversion |
| the two lanes are separate UI sections | `SectionView`'s two arms borrow **different types**, so a projection has no position under the deterministic heading |
| a plan never writes actual state | no edge of any kind to `academic-store` or `academic-store-platform`, walked over the workspace manifests; every output has private fields and one crate-private producer; nine compile-fail cases |
| `ProjectedEvidenceOpportunity` is the only future-knowledge output | section 22.3's first three bullets are `P2-C7`'s `ScenarioProjection`, produced by that crate's own `project`; this crate declares no second future-knowledge type |
| no mastery delta in the output | this crate names no part of `P2-N2`'s ladder, measured over its whole identifier inventory against a vocabulary derived from that crate's own source |
| workload is a range with bias | `ProjectedWorkload` takes `P2-U8`'s `BiasDisclosure` **by value**, and its whole method inventory is four entries with no point accessor among them |
| `STALE_INPUT` freezes and asks | `FrozenPlan` returns its plan unchanged; `FrozenPlan::recompute` takes a `RecomputeConsent` built from `P2-M2`'s `UserDecision` |
| the graduation modes are distinct | `HypotheticalGraduation` is the only one here, and `academic-audit` is unreachable from this package through an edge of any kind |
| no default aggregate recommendation score | `DimensionPriority` is a **permutation**, not a weight vector; no arithmetic combines two dimensions; there is no `Default` anywhere in the package |
| a reordering explains itself | `ReorderingExplanation::between` names every moved weight and the dimension that decided the new leader, and refuses an unchanged priority |
| calibration evaluates the model | `ModelCalibrationReport` carries the engine version, the input digest and the model run, and no field about a person |

## The deterministic lane needs an official timetable to be deterministic

`ScheduleConflicts::of` reads `P2-U5`'s `ConfirmedSeat` values. That type's one
producer in this workspace is `ConfirmedStanding::seat`, and section 8.3's
`HISTORICALLY_LIKELY` row — *placeholder만, 졸업계획 확정에 사용 금지* — has no
seat to hand it. An official schedule conflict therefore cannot be computed over
a predicted offering: not because a check refuses it, but because the argument
does not exist.

Two of section 22.2's seven bullets carry a condition, and both are parameters
rather than flags. `RuleContribution::under` takes `HypotheticalCompletion` by
value, so *이수한다고 가정했을 때의* is an argument. `GpaScenario::under` takes
the stated grades and refuses a set that leaves any of the plan's choices
unstated, so *사용자가 명시한 grade 가정에 한해서만* is a refusal rather than an
average over the part the user typed. A plan with no stated grades has `None`
where the GPA would be — which is not zero, and is not the record's GPA either:
`HypotheticalTermAverage` is a different type from `academic_record::GpaValue`
with no conversion between them, because a cumulative average needs the attempt
ledger and this crate cannot reach one.

`EnrolmentLimitStanding::Unknown` is never a pass. Section 28's
`OFFICIAL_PREREQUISITE` engine is `PLANNED`, so nothing here concludes that a
registrar would admit a registration; `PrerequisiteStanding::verdict` maps an
unrecorded restriction to `Unknown` and a known failure to `NotMet`, and
`NotMet` outranks `Unknown` because a known failure is a conclusion.

## The frozen inputs are the whole of what the engine reads

`P2-N6` found the opposite in its own engine: frozen inputs that omitted the
hypergraph, the constraint inputs and the acquisition options, so two corpus
cases with byte-identical frozen inputs had different expected outputs. The
guard here is structural rather than declared.

`PlanInputField` and `PlanChoiceField` are the fields `plan_inputs_digest`
hashes, each in a total `match`.
`frozen_inputs_are_the_whole_of_what_the_engine_reads` parses the field
declarations of both input types out of the crate's own source and compares them
against those enumerations **in both directions**, then varies each of the
twenty-two fields in turn and requires the digest to move. A field added to an
input struct without a digest arm fails as an undigested declaration; an arm
added without a field fails as a digest of something that does not exist.

Every field is keyed by its own name inside the digest. The key is **not** what
separates a swapped pair of same-shaped fields — the field order does that, and
an injection that removed the key from every field left every behavioural
assertion in the suite unchanged. What the key buys is that the digest is bound
to the field *names* rather than only to their order.

That is invisible to any comparison the engine can make against itself, so both
digests are **pinned**: `scenario_basis_round_trip` and
`frozen_inputs_are_the_whole_of_what_the_engine_reads` each hold one committed
value, and the cost of changing an encoding is changing it there. Before those
pins existed, an injection that dropped the keying passed every test in this
suite.

## Two readings the design document leaves open

**Section 22.4's `후속 경로` row is `Mixed` because section 22 puts its two
halves in different lanes.** The official prerequisite unlock is section 22.2's
sixth bullet and the informal readiness is section 22.3's seventh, and the
table's own `확실성` cell for that row reads `Mixed`. `DimensionLane::Mixed` is
that cell. Folding the row onto one side would have made one half claim the
other's certainty. `reordering_explains_the_changed_weight` derives each
dimension's lane from its certainty cell and fails if the document stops saying
`Mixed` there.

**`PlanScenario` carries a seventh field section 22.1 does not name.** The block
in section 22.1 has six keys — `id`, `basedOn`, `choices`, `assumptions`,
`deterministicResults`, `projections` — and this crate holds all six plus
`inputs_digest`. It is there because a plan that cannot say which frozen inputs
produced it cannot be shown to be stale, cannot be recomputed reproducibly, and
cannot be calibrated against the term that followed it, all three of which
section 22.5 requires and none of which section 22.1's block provides a key for.
`SCENARIO_KEYS` is compared against the document's six in both directions, so the
six stay measured and the seventh stays recorded here.

## What the acceptance suite is, and is not, evidence for

The ten named rows are in `crates/what-if/tests/what_if.rs` and
`crates/what-if/tests/what_if_scans.rs`. The three absence claims —
`plan_scenario_never_writes_actual_state`, `no_mastery_delta_in_plan_output`
and `no_default_recommendation_score` — are proved by exhaustion: the whole set
of 153 declared field positions, the whole set of 49 external import leaves, the
whole set of 24 functions section 22.4's module declares, each compared in both
directions. There is no forbidden-name list of this crate's own invention
anywhere in the suite, because `P2-X2`'s injections showed that such a list
passes for every spelling nobody predicted.

Three of the vocabularies the suite refuses are **derived from the crate that
owns them** rather than typed into the test: the mastery ladder from
`crates/knowledge-state/src/ladder.rs` and `academic-domain`'s own
`MasteryLevel`, `P2-U3`'s verdict vocabulary from `crates/audit/src/verdict.rs`,
and the reachability of the canonical writer from the workspace manifests. A name
added to any of them extends the guard without anybody editing the suite. Each
has a control that requires the same reader to find those names where they do
live.

It is **not** evidence that a real semester behaves this way. Every offering,
seat, review aggregate, concept signal and path role in the suite is synthetic
and built in process, and the one real composition it exercises is with the other
engines in this repository rather than with a registrar.

It is **not** an offering forecast, a critical path, a graduation verdict, a
review aggregate or a knowledge state. Each of those is another task's, read here
and decided nowhere in this crate.

## `§38`

`P2-N8` opens and closes no section 38 gate.
