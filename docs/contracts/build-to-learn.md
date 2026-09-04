# Build → Learn, and course ↔ project mapping

`academic-build-learn` implements section 20's Build → Learn mode and section
21's SNU course ↔ project mapping. This page states what the crate refuses, what
it deliberately does not decide, and the one place the design document says two
different things.

## The order is the contract, and it is types

Section 20.1's whole sentence is that the system does not turn an input into a
technology list:

> 시스템은 이를 바로 기술 목록으로 바꾸지 않고 성공 조건과 선택 지점을 추출한다.

That order is four by-value arguments, not four checks:

```text
GoalInput --normalize--> NormalizedIntent
                              |  ProjectGoal::state(&intent, SuccessCriteria, Constraints, UnresolvedDecisions)
                              v      ^ SuccessCriteria::of returns None for []
                        ProjectGoal
                     /                \
   TechnologySlate::under(&goal)   ResponsibilityDecomposition::decompose(goal, ..)   <- goal by value
              |                                    |
      TechnologySlate                  ArchitectureBranch::of(decomposition, ..)      <- decomposition by value
                                                   |
                                          ArchitectureBranch
```

Each arrow is the ownership of a value. A technology list stated before the
criteria, or an architecture branch derived before the capability is decomposed,
is a program that does not compile — `crates/build-learn/tests/compile_fail/`
holds the eight compiled refusals.

`TechnologySlate::under` is the **only** producer of a technology list, and
`the_only_producer_of_a_technology_slate_takes_a_goal` compares the whole set of
public functions returning one against exactly it. A goal with no unresolved
decision yields an **empty** slate: the system does not hand back a technology
list it was not asked to choose.

## The four groups are four types

A `ProjectGoal` holds a `NonEmptyText`, a `SuccessCriteria`, a `Constraints` and
an `UnresolvedDecisions`. An unresolved decision cannot be serialized as a
constraint, because `UnresolvedDecision` has an alternative list and `Constraint`
has no field one could go in, and there is no conversion between them.
`goal_schema_separates_four_groups` compares the four serialized key sets in both
directions and reads section 20.1's own YAML block back out of the design
document for the key names.

An `ObservableCriterion` takes the statement **and** what would be watched to
decide it, both non-blank. `실시간 협업 편집기를 만들고 싶다` — the goal's own
`text` — names nothing that could be watched, and that is why it is the `text`
field and not a criterion.

## AND/OR is section 16.1's hypergraph, and the answer is a set

`ArchitectureBranch::hypergraph` builds `academic_critical_path::Hyperedge`
values over `academic_gap::PrerequisiteEdge` members, and
`ArchitectureBranch::satisfying_sets` delegates to
`academic_critical_path::satisfying_sets`. **There is no solver in this crate and
no path length anywhere in it.** Section 20.2 asks which whole set of concepts
satisfies the capability; a satisfying set is that answer.

An `OR` member is conditional **by construction**. `BranchGroup::of` stamps the
decision and the alternative onto every member it is given, no public constructor
of a `ConceptRequirement` takes a `RequirementCondition` as an argument, and the
field is private. `ConceptRequirement::always` is the one producer of an
unconditional requirement and it cannot be called from inside a group.

The branch also refuses four shapes that would silently resolve a choice the user
left open: a group naming a decision the goal does not hold, a group naming an
alternative that decision does not offer, a decision offered fewer than two
distinct groups, and an open decision with no group at all.

## Five readiness names, six readiness rows

**The design document states the count two different ways, and both readings are
kept.**

* The reverse-path drawing's fifth line is `ready / refresh / direct need /
  conditional / later-scale`, which names **five**. `t001` derives `REQ-20-006`
  from it.
* The table under `결과는 다음 범주로 제시한다` has **six** rows: `이미 준비됨`,
  `refresh 필요`, `현재 약함`, `구현에 직접 필요`, `선택에 따라 필요`,
  `규모/조건이 바뀌면`. `t001` derives six consecutive requirements from it,
  `REQ-20-008`–`REQ-20-013`.

`t068` names the acceptance test `five_readiness_categories_map_exactly`.

**The resolution:** `ReadinessCategory` has six variants, one per table row,
because the table is what a result is presented as and six requirements enumerate
it. `SHORT_NAMES` is the drawing's five paired with the row each names, and
`ROW_WITHOUT_A_SHORT_NAME` is the one row the drawing does not name — `현재
약함`. `five_readiness_categories_map_exactly` parses **both** out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` at run time and requires the
five to be an **order-preserving injection** into the six, with the residue
identified. It fails if the document stops saying either thing. Five is not a
number written in the crate and neither is six.

### The resolution order

Two of the six rows speak about evidence the user has, three about why the
concept is on the list at all, and one about a benefit whose trigger has not
fired, so without a stated order more than one row would be true of the same
requirement. `RESOLUTION_ORDER` pins it as data:

| Order | Condition | Row |
|---|---|---|
| 1 | a `P2-R4` benefit contract, whatever the overlay says | `규모/조건이 바뀌면` |
| 2 | no sufficiency gap and not stale | `이미 준비됨` |
| 3 | no sufficiency gap and stale | `refresh 필요` |
| 4 | a gap, and conditional on an open decision | `선택에 따라 필요` |
| 5 | a gap, and the criterion is about this concept | `구현에 직접 필요` |
| 6 | a gap, reached as a prerequisite neighbour | `현재 약함` |

Row 1 is unconditional on the overlay because `WOULD_BENEFIT_FROM` is not a
requirement: section 18.4 forbids automatic re-classification, so a benefit
contract stays a benefit contract however much evidence the user has. Rows 2 and
3 win over 4–6 because a concept the user already has sufficiently and recently
is ready for whichever branch is chosen.

The evidence half is **read**, never recomputed. `충분하고 최근인 evidence` is
`P2-N2`'s `SufficiencyGap` list being empty, reached through `P2-N5`'s overlay;
`stale` is `P2-N3`'s band read through `academic_critical_path::is_stale`, which
already refuses `UNKNOWN` as stale. This crate has no ladder, no decay, no
threshold and no clock.

## A learning item cannot exist without both halves

`LearningItem::plan` takes an `EvidenceTask` and a `ReturnCheckpoint` by value.
The checkpoint holds a `SelectionApproved`, and the four stages behind it each
take the one before by value:

```text
ReadingDone --> ExplainedByHand --> SimulationPassed --> SelectionApproved
개념 읽기        손으로 설명           최소 simulation test    선택 승인
```

So `선택 승인` before `최소 simulation test` has no value of the right type to
pass. `learning_item_requires_evidence_task_and_checkpoint` parses the four stages
out of section 20.2's own example and requires them in the example's own order.

## A study-only plan is refused structurally, not by its words

Section 20.2's `OS가 긴 강의 목록만 제시해 build 동기를 끊지 않는다` is enforced
by four absences and no phrase list:

| `PlanDefect` | What is structurally absent |
|---|---|
| `NO_IMPLEMENTATION_STEP` | no step of the plan builds anything |
| `CRITERION_REACHED_BY_NO_IMPLEMENTATION` | a success criterion no implementation step moves toward |
| `CHECKPOINT_RETURNS_TO_NO_STEP` | a return checkpoint naming a step the plan does not have |
| `CHECKPOINT_RETURNS_TO_NON_IMPLEMENTATION` | a return checkpoint that returns to more studying |

Four more hold the joins: `LEARNING_ITEM_IS_ABOUT_NO_REQUIREMENT`,
`REQUIREMENT_HAS_NO_FINDING`, `ACQUISITION_NEEDED_BUT_NO_STEP` and
`EXPERIMENT_ANSWERS_NO_DECISION`.

**No threshold is invented.** `t001` records `REQ-20-016`'s gate candidate as
`최대 연속 학습 단계` and leaves it open; this crate states no maximum run of
learning steps. What it requires instead is that every success criterion is
reached by an implementation step and every learning item returns to one, which
refuses a study-only plan without deciding how many study steps in a row are too
many.

`a_fluent_lecture_list_plan_fails_validation` drives a plan whose every step is a
well-formed course with a real evidence task and a real four-stage checkpoint,
spelled with none of the design document's words and none of the crate's
identifiers, and observes exactly the same three defects.
`the_build_learn_crate_holds_no_phrase_list` observes the other half: the
validator's own file holds no string literal that is not a stable spelling.

## The three motivation edges are never summed

`MotivationDisplay::rows` returns one row per edge with its own reason, in
`MOTIVATIONS`' order whatever order the edges arrived in. Two edges under one
label are **refused** rather than joined, because joining them is the only way one
row could come to stand for two reasons.

The absence of a sum is stated three ways, none of them a list of names:

* `every_impl_header_in_this_crate_is_in_the_inventory` pins all 55 `impl`
  headers in both directions and refuses fifteen folding traits — `Add`, `Sum`,
  `Mul`, `Deref`, `AsRef`, `Borrow`, `FromIterator`, `Index` and the rest — over
  the **whole** inventory for any type pair at all, and pins the eight
  conversions this crate does implement, requiring none of them to name a numeric
  type.
* `every_public_signature_is_in_the_inventory` pins all 130 public signatures.
* `no_signature_folds_the_motivation_edges` compares the whole set of signatures
  naming a motivation type **and** returning a number against the empty set, with
  each half shown separately non-empty and the predicate shown to bite on a
  fragment that does fold. It also compares the whole set of declared fields
  against having any floating-point member and against having more than one
  integral member.

**Why the impl-header pin exists.** `P2-Y3` measured a `From` implementation
escaping every `pub fn` sweep, because a trait impl declares no `pub fn`. `P2-X5`
then measured the same shape as a **still-open instance elsewhere**:
`impl From<FieldCoverage> for u32` in `academic-blind-spot`, summing every
source's evidence count, passes that crate's whole suite and its 114-signature
inventory. That instance is not this task's to repair; it is recorded here so the
next reader knows the class is still live one crate over.

## Section 21: a course is not its title

`DesignedCoverage` is what `P2-U1`'s `CourseRevision` says a revision is
*designed* to teach; `ActualCoverage` is what a particular `CourseOffering` was
*observed* to cover, and it carries the syllabus, lecture, assignment or
assessment sighting that observed it. They are two types with **no conversion
between them in either direction**.

A title reaches neither. `DesignedCoverage::of` reads
`CourseRevision::designed_concept_coverage`, a list of identities, and there is no
constructor taking a `CourseTitle`; `ActualCoverage::observed` refuses an empty
sighting list. So `“데이터베이스” 과목 이름만 보고 모든 isolation·replication
competency를 채운다` is a claim with no representation.

`MappingStatus::requires_actual_coverage` is a property of the enumeration rather
than a check at one call site, and `CourseProjectMapping::publish` refuses
`CAN_BE_SUPPORTED_BY_CURRENT_COURSE` and `CONFIRMED_NEXT_TERM` without an
`ActualCoverage`. That is REQ-21-014 and REQ-36-038 — the existence of a course is
not a guarantee that a term's offering covers anything.

Section 21.3's `양쪽 효과를 구분한다` is `ChannelComparison`, which holds two
`academic_critical_path::CostEstimate` values on two named axes and has no
function returning one number.

## What this crate does not decide

* **Whether a concept is understood.** `P2-N2` owns the ladder, `P2-N3` the band.
* **What a concept costs.** `P2-N6` owns the vectors and the Pareto elimination.
* **Whether an offering runs.** `P2-U1` owns section 8.3's four standings.
* **Whether the repository observes a concept.** `P2-R4` owns section 18's three
  classifications.
* **Persistence.** Nothing here is written. There is no migration and no edge to
  `academic-store`: a plan is recomputed from a goal, an overlay and a snapshot
  rather than stored beside them.
* **A registry row.** Section 28's table has twelve rows and none of them is a
  build-to-learn planner, so `P2-C5`'s registry gains nothing and nothing appears
  under `testdata/engines/`.
* **`§38`.** `P2-R6` opens and closes no gate.

## What is not serialized, and why

`RequirementOrigin` and `ReadinessFinding` implement neither `Serialize` nor
`Deserialize`. `P2-R4`'s `BenefitContract` implements neither, and giving one a
wire form here would be a second serialization of a value that crate chose not to
publish. The wire forms in this crate are the goal, its four groups, the plan
steps, the motivation display and the channel comparison, and
`goal_schema_separates_four_groups` and
`motivation_edges_are_shown_in_parallel` compare their key sets as whole sets in
both directions.
