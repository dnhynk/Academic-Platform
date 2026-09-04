# Career readiness matrix

`P2-Y3` implements section 24.3's career readiness view and section 24.4's
bidirectional navigation in `academic-readiness`. This page is the contract:
what the view guarantees, what it refuses, and what it is **not** evidence for.

## Section 24.3 states two different sixes, and this crate keeps them apart

Section 24.3 contains two enumerations of six things, in two different
sentences, and they are **not the same set**.

| Reading | Where | The six |
|---|---|---|
| the matrix's columns | section 24.3's table header row, after `Competency` | `학문적으로 배움`, `문제/과제`, `Project 적용`, `장애/Debug`, `설계 선택`, `Freshness` |
| the evidence stages | section 24.3's prose, in back-quotes | `사용해봄`, `구조 이해`, `문제 해결`, `장애 debugging`, `설계 선택`, `새 상황 전이` |

The first six are corroborated by a second, independent place: section 36.9's
per-competency career-view block writes `academic`, `practice`, `project`,
`debugging`, `design`, `freshness` — six keys, in the same order, one per
column. The second six are `academic_competency::EvidenceStage`, which `P2-Y1`
already owns and already reads out of that sentence.

`six_axes_are_separate_columns` parses the table header and the section 36.9
block and compares each against `ReadinessAxis::ALL` position by position and in
both directions. `an_axis_and_a_stage_are_two_vocabularies` parses the prose
sentence, compares it against `EvidenceStage::ALL`, and then requires the two
sets to be **unequal** and to overlap on exactly one spelling, `설계 선택`. So
this reading is executed, not assumed: a design document that renames a column,
adds one, drops one, lets its two places disagree, or merges the two sixes into
one fails this crate rather than being folded into it.

**The reading is open where the document is.** Section 24.3 does not say which
of its two sixes the phrase *six axes* in the execution plan refers to. This
build takes the columns, because the acceptance test is named
`six_axes_are_separate_**columns**`, because the columns are the six that appear
twice, and because section 34.5 asks for `missing/unknown과 freshness를 별도
표시` — a requirement that is vacuous unless freshness is one of the columns.
Both sixes are represented and neither is folded into the other.

## No function maps a depth to a column

`P2-Y1` recorded that section 13.2's eight evidence rows and section 24.3's six
stages are a coincidence of counts and not a correspondence, because a total map
between them would have to invent three of its six answers. One layer up the
same map would be worse: section 24.3's `설계 선택` **column** has no section
13.2 row at all.

So `AxisEvidence::place` takes the column as an argument. The column is where
the user put the evidence; the record's own stage and origin travel beside it so
a reader sees both, and a reader can see a lecture placed in the design column
and judge it. `no_function_maps_a_stage_or_a_kind_to_an_axis` compares the whole
set of public signatures naming both a `ReadinessAxis` and an `EvidenceStage` or
an `EvidenceKind` **and returning something** against the empty set, and shows
both halves of that conjunction to be separately non-empty so the result is an
absence rather than nothing to look at.

## What a cell says

| Reading | Meaning | Produced when |
|---|---|---|
| `EVIDENCED` | at least one placement settles this column | a placement names a criterion the competency states and the rubric admits a row at that placement's own stage |
| `MISSING` | nothing was recorded in this column | no placement reached the column at all — section 24.3's `—` |
| `UNKNOWN` | something was recorded and none of it settles the column | every placement that reached it named no stated criterion, or its stage has no rubric row for the criterion it named |

Both `UNKNOWN` bases are `P2-Y1`'s own readings one layer up: the first is a
placement its `RubricSheet` would leave in `unmatched`, and the second is its
`CellState::NotInRubric`.

**Freshness is a different type.** `FreshnessCell` wraps
`academic_domain::FreshnessBand` and there is no conversion between it and
`AxisCell` in either direction, neither is a field of the other, and no declared
type of the crate holds both. `FreshnessBand::Unknown` and `AxisCell::Unknown`
are spelled the same and mean different things — a band for a concept about
which nothing datable was ever admitted, against a column that received
something it cannot read — and the shared spelling is exactly why they are two
types.

`AxisCell::Evidenced` and `AxisCell::Unknown` are `#[non_exhaustive]`, so an
*empty* filled cell — one saying a column is settled by nothing — is not a value
another crate can write. `AxisCell::read` is the one producer.

## The matrix is of a bundle at an exact version

`take` reads its row set out of a `P2-Y2` `RoleProfile`'s own `competencies`, in
that bundle's order, and records the bundle by its `RoleProfileRef` — the
lineage-and-version pair, not section 24.2's folded `_v4` spelling. `P2-R4`
measured what a concatenated-and-truncated key costs, and a matrix keyed on a
rendered name would be that shape one stage up.

A bundle entry the caller supplied no input for is **not** dropped and not
filled with a zero. It becomes a row whose five evidence columns all read
`MISSING` and whose band is `Unknown`, which is section 24.3's own last table
row written as a value rather than as a rendering convention. An input naming a
competency the bundle does not list reaches no row.

## The auxiliary score does not exist without its four disclosures

Section 24.3: *보조 score가 필요하면 각 cell의 rubric, source, 누락 데이터와
가중치를 공개하고 비교·채용 가능성을 보장하는 수치가 아님을 표시한다.*

`disclose` is the one producer of an `AuxiliaryScore`. It takes all four
disclosures **by value** and it has **no score parameter**: the number is
computed from the disclosed weights over the disclosed matrix, so there is no
expression anywhere that produces a score somebody chose.

Three of the four are re-derived and refused if they disagree.
`RubricDisclosure`, `SourceDisclosure` and `MissingDataDisclosure` each have one
producer that takes the matrix, and `disclose` re-derives all three and refuses
any that is not equal to its own derivation — so a disclosure taken of a
different matrix, or of the same matrix before a cell changed, is refused rather
than published beside a number it does not describe.

The fourth, `WeightDisclosure`, is the user's own judgement and is derivable from
nothing. What is checked is that it is *total*: one weight for every evidence
column, each with the user's own stated reason. A column left out is refused
rather than weighted at zero by omission, and the freshness column is not
weightable at all because it is not evidence.

`ScoreValue` is two `u32` counts of weighted units. **Nothing in the crate
declares an `f32` or an `f64`**, so section 34.5's *허위 정밀도* has no type to
arrive in — whatever a field is called.

## No aggregate percentage is the primary output

This is an absence, so it is stated as whole-set comparisons rather than as a
list of forbidden names. A name list refuses the spellings somebody thought of
and admits every other one.

1. **Position.** `ReadinessView::render` emits `ViewBlock::Matrix` first in every
   view the crate can produce, and the score never precedes it. A hero block
   placed before the matrix fails whatever it is called.
2. **The block vocabulary is closed.** Every kind emitted is compared against
   `ViewBlock::KINDS` in both directions, so a new block kind fails until
   somebody writes it down — and then still has to come after the matrix.
3. **The scalar vocabulary is closed.** Every declared field of every type is
   compared against a 69-entry inventory in both directions, each entry says
   which of seven things it holds, and `u8`, `u64`, `f32` and `f64` are refused
   outright. `ScoreValue` is required to appear as a field in exactly four
   places — the score itself and the three history entries that record what a
   score said — and the whole set of public functions producing one is compared
   against `AuxiliaryScore::value`.
4. **There is one view.** `ReadinessView::of` takes no mode, no preference and
   no flag, and the whole set of public functions returning a view is compared
   against the four this crate documents.

## Every navigation direction terminates

Section 24.4's four directions are `NavigationDirection::ALL`, parsed back out of
that section's own bullets and compared in both directions.

The direction is **not** a second argument. `traverse` takes one
`StartingPoint`, whose four arms are the four directions and each of which
carries the typed identity its direction is about — a `P2-Y1` `ConceptRef`
(namespace included), a `P2-Y2` `RoleProfileRef`, or this crate's
`StartingPointId` for the two that name an outside record. Nothing compares an
ontology identifier against a classification token.

A walk ends at one of two things and there is no third:

* `CriterionAndEvidence` — a performance criterion and a locator a reader opens;
* `ExplicitAbsence` — `CellIsMissing`, `CellIsUnknown`, or
  `NoRowReachesTheStartingPoint`, each naming exactly what is absent.

Neither arm carries free text, so section 24.4's `직무에 중요` has no field to
arrive in. `Termination` holds its first terminus as a field taken by value, so
an empty walk is not a value that exists.

**The non-emptiness rests on two other crates' refusals, measured rather than
assumed.** `P2-Y2`'s `declare` refuses a bundle that names no competency and
`P2-Y1`'s refuses a competency that states no criterion, so a matrix has a row
and a row has a criterion. This crate adds no third check: a branch guarding a
case its own inputs cannot produce is a branch that never runs.

## Hiding a score and resetting a weighting preserve history

Section 34.5's recovery for `career readiness 과도한 점수화` is `score 숨김/가중치
초기화`, and section 34.6's first principle is that the original is preserved.

Neither recovery is a mutation. `hide_score` and `reset_weights` take `&self` and
return a **new** view; nothing in the crate takes `&mut self` at all. The prior
number and the prior weighting travel into the new view's history, so what was
once displayed stays openable. `score_hide_and_weight_reset_preserve_history`
compares the two histories as sequences and requires the older to be a prefix of
the newer, which a deletion could not satisfy.

## The non-guarantee notice survives export

`NonGuaranteeNotice::rendered` takes no argument and the type has no public
field, no `Default`, no setter and no `Deserialize`, so there is no expression
that produces a different notice. Every serialized view carries it under
`nonGuaranteeNotice`, and `published_notice` refuses a document that lost it or
altered it.

`non_guarantee_disclaimer_survives_export` measures this against `P2-P1` itself
rather than simulating it. It builds a real `academic_export::BundleRequest`,
writes a real bundle with `write_bundle`, and reads it back with `read_bundle`,
which takes a path and **no key, no token, no host and no account**. The
readiness view travels as the canonical JSON of a claim under a `career.`
predicate, which is the namespace section 37's `role 관심 변화와 alternative
paths` part selects, so it is routed and written by that crate's own code.

Three things are then measured, and the third is `P2-P1`'s:

1. every published file carrying the document carries the notice, and the line
   is byte-identical to what the writer was handed;
2. `published_notice` refuses the restored bytes with the notice removed, and
   with the notice altered by one character — both built out of the restored
   bytes rather than typed;
3. deleting the notice from the published bundle makes `read_bundle` refuse the
   bundle, because that crate digests every file and seals the manifest.

### What this is not evidence for

**It does not claim that no program could parse the matrix rows while ignoring
the notice key.** Nothing in an open interchange format can claim that. What is
claimed is that the notice is in the bytes a recipient receives, that this
repository's own reader refuses a document that has lost it, and that removing
it from a published bundle breaks that bundle.

## What this crate does not decide

* **The Career Explorer surface.** Section 25.11's graph, comparison view and
  acquisition options are a `P2-X`-stage screen. This crate renders an ordered
  list of blocks and draws nothing. **No Tauri runtime is linked and no window
  opens for any claim on this page**; every test here runs in process against
  values, exactly as `P2-X1`, `P2-X7` and `P2-X2` record for their own surfaces.
* **Freshness.** `P2-N3` owns the bands and the decay. There is no time input to
  any function here: a band arrives as an argument.
* **What evidence is admissible.** `P2-N2` owns eligibility and `P2-Y1` owns
  which of section 13.2's rows may found a stage record. This crate takes
  `StageEvidence` values that already passed both.
* **Persistence.** No `academic-store` edge, no migration, no file and no
  socket. A readiness view is a derivation over values three crates below
  already froze, and a correction is a new call over new inputs.
* **§38.** This task leaves no gate open and closes none.
