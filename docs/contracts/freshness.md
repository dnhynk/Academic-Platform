# Freshness bands, decay, spillover and the priors

`academic-freshness` is the `P2-N3` boundary. It is section 13.3: the six bands,
the seven inputs that produce one, the single-hop cited spillover, the versioned
prior, and — for the promotion section 1's fifth invariant forbids — the value
that does not exist rather than the check somebody has to remember to run.

It sits on two boundaries and restates neither. `academic-domain` already
declares `FreshnessBand` and already has `ClaimObject::Freshness` as the wire
shape of a claim about one, so this crate declares no second enumeration and no
second scale. `academic-knowledge-state` already decided what evidence is
admissible, so this crate dates an `EligibleEvidence` and has no constructor that
dates anything else. It opens no file, opens no socket, reads no clock, persists
nothing, adds no migration, and has no edge to `academic-store`.

## Time does not take anything away

Section 13.3: `시간 decay는 freshness projection에만 적용한다. mastery를 자동
내리지 않는다`. Section 34.2 says how to hold it — `별도 field·색/문구·API type,
mastery 자동 강등 금지` — and *API type* is where it is held here.

**This crate has no name for a mastery level.** `academic-knowledge-state` is a
product edge and hands out `LADDER`, `rung`, `level_token`, `AutomaticLevel` and
`MasteryProjection`; `academic_domain::MasteryLevel` is one `use` away. Nothing
here reaches for any of them, so `decay` takes an elapsed span and a persistence
window because those are the only things it *can* take.

`time_decay_touches_freshness_only` observes the behaviour — a clock swept
forward across six instants moves the band from `VERY_HIGH` to `STALE` and leaves
the level identical at every step. That is a statement about the paths the test
drove. `the_freshness_crate_cannot_name_a_mastery` makes the stronger one, as
four whole-set comparisons in both directions plus an eight-name refusal over
every product file and every public signature, with a control: the same reader
is required to find at least five of those eight names in `P2-N2`'s own
`ladder.rs`, so the zero it reports here is a measurement.

## The proofs are types

| Section 13.3 rule | What holds it |
|---|---|
| decay cannot reach a mastery | the crate has no name for one; `decay` takes a span and a window |
| freshness reaches `P2-N2` through one value | `FreshnessProjection::input` returns a `FreshnessInput`, which has two fields |
| ineligible evidence cannot freshen a concept | `DatedEvidence` wraps an `EligibleEvidence` and has no other constructor |
| another concept's evidence cannot freshen this one | `project` refuses it, and `NeighborUse::direct` refuses it one hop out |
| spillover is one hop | `NeighborUse` is built from dated evidence, never from a band, a projection or a contribution |
| spillover weighs less than direct use | `Spillover::band` is a band strictly below the neighbour's own |
| an AI cannot say the user still remembers | `RecallStatement`'s one constructor runs ADR-003's actor matrix |
| the shipped prior cannot pass for a measured one | `PriorBasis::NoEvidenceBasisEstablished` is a value, and `PersonalizationSpeed` has no `Default` |
| a band is recomputed, never edited | no `&mut self` method, no setter, no public field |

`crates/freshness/tests/compile_fail/` holds the compiled half: nine cases, each
a program that fails with a committed diagnostic.

## Neither count is a number in this crate

`freshness_bands_are_exactly_six` reads section 13.3's own sentence —
`Freshness는 … band로 표시한다` — out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, splits it on its own
back-quoted spellings and compares them against `BANDS` in both directions. It
reads the seven `- ` bullets under `계산 입력은 다음과 같다` the same way and
compares them against `FreshnessSignal::ALL`. Six and seven are measurements of
the design document.

`BANDS` is the sentence's order, **best first**, and
`academic_domain::FreshnessBand` derives `Ord` the other way with `Unknown`
lowest. Both are kept and the test requires `BANDS` to be strictly decreasing
under `rank`, so the two cannot drift apart.

`UNKNOWN` is not "very stale". It is the band for a concept nothing datable was
recorded about; `STALE` means something *was* recorded and only its immediate
retrieval is unverified. No contrary event reaches `UNKNOWN` for that reason: a
recall failure is a record.

## What raises a band, what caps it, and which wins

Three inputs raise — this concept's own dated evidence, a bounded spillover, and
the user saying `지금도 바로 사용할 수 있음`. Two cap — the user saying
`복습 필요`, and section 13.3's three contrary events.

**When the latest cap is at least as recent as every raiser, the cap applies.**
That is `REQ-13-030`'s own case, and it is what makes
`recall_failure_prevents_freshness_increase` a property rather than one
comparison: the test piles six further dated items, a spillover and a
`CanUseNow` statement onto a standing failure and the band does not move, then
removes the failure and observes that every one of those raisers does move it.
Relearning still works, because a raiser dated *after* the failure is more recent
than it.

The three contrary ceilings differ because the three phrases say different
things. `설명 실패` is a failure to produce, which leaves recognition intact, so
it caps at `LOW`. `기억 안 남음 표시` and `재학습 필요` are the user reporting
that nothing is retrievable, which is `STALE`.

## Section 13.3's third input, read for its own word order

The bullet is `노출·복습보다 실제 적용·debugging·설계에 더 긴 지속성`.
`PersistenceClass` has one variant per side carrying the document's own phrase,
and `debugging_evidence_persists_longer_than_exposure` splits the bullet on its
own `보다`, checks the phrase before it against `ExposureOrReview`, the phrase
after it against `ApplicationOrDesign`, and reads the relation word `더 긴` out
of the same line. **The direction is measured from the design document, not
asserted beside the table.**

`persistence_class` is total over `EvidenceKind` with no wildcard arm. The grade
row answers `None`, and that is not an oversight: section 13.2's eighth row has
no `ConceptEvidence` variant at all, so no dated evidence can carry that kind and
a window for it would be a value nothing reaches.
`no_dated_evidence_can_carry_a_grade` observes the branch is empty over every
constructible value.

## The three limits on spillover, and the one that survives the other two

Section 13.3: `관련 concept 사용의 전파는 한 단계, 낮은 weight, 명시적 근거로
제한해 연쇄적으로 전체 분야가 신선해지는 오류를 막는다`.

* **`명시적 근거`.** `CitedEdge` has one constructor, which refuses an edge with
  no evidence item — section 7.3 already says an edge is a claim with evidence —
  a self-edge, and any predicate outside `SPILLOVER_EDGES`. That list is the four
  section 7.2 rows whose two endpoints are both concepts, and the test compares
  it against `PredicateName::ALL` in both directions, so a twenty-first predicate
  is an extra key rather than a silent admission.
* **`낮은 weight`, as a band comparison rather than a coefficient.** A weight is a
  number somebody has to check is smaller. `Spillover::toward` steps the
  neighbour's own band down once and takes the lower of that and
  `SPILLOVER_CEILING`, and the test checks `spilled < source` over the bands that
  contribute at all. The two rules are separate and both are observed: the step
  alone would let a neighbour at the top contribute `HIGH`, which is a band this
  concept's own evidence reaches. A neighbour below `MODERATE` contributes
  nothing, because the bullet says `최근 사용`. Contributions are combined with
  the higher of the two rather than summed, so ten neighbours give a concept
  exactly what one gives it — accumulation is how
  `연쇄적으로 전체 분야가 신선해지는 오류` arrives without any single hop being
  wrong.
* **`한 단계`.** `NeighborUse` is built from the neighbour's **own** dated
  evidence and from nothing else. `A → B → C` gives `C` nothing because `B` has
  no evidence of its own to date, which is `REQ-13-034`'s case.

**The route that survives all three is the one where the evidence handed to a
neighbour is not the neighbour's.** Cite the real `B — C` edge, then offer `A`'s
evidence as `B`'s recent use, and `A` reaches `C` across two hops through one
well-formed edge with nothing malformed anywhere. That is the misattribution
`P2-N2` found one layer up, where a history accepted another concept's admitted
evidence. `NeighborUse::direct` refuses any dated item linked to a concept other
than the neighbour; `project` refuses the zero-hop form, where the same evidence
is offered with no edge at all; and a `Spillover` carries the concept it was
computed toward, so one aimed at `B` is refused under `C`'s projection.

## The prior is versioned, and half of it is `GATE-38-024`

Two numbers live in `UNCALIBRATED_PRIOR_V1` — 90 days and 360 days — and
**neither is derived from evidence**. `GATE-38-024` is exactly that: the evidence
basis for the priors and the speed of personalization are configuration
decisions, and this task makes neither.

What is fixed is everything around them.

* The prior is versioned and its name is `UNCALIBRATED_PRIOR_V1`. `PriorName` is
  a closed two-value vocabulary rather than a string, so a prior cannot be
  shipped under a name saying it was calibrated when it was not.
* Its `PriorBasis` is `NO_EVIDENCE_BASIS_ESTABLISHED`, which a caller reads
  rather than a comment somebody has to find.
* It is **visibly uncalibrated**: `RetentionPrior::is_uncalibrated` is true,
  `FreshnessProjection::prior_is_uncalibrated` carries it to every rendering, the
  trace line says `UNCALIBRATED`, and the projection carries a
  `ConfidenceGap::PriorUncalibrated` while it does.
* It stays **identifiable after calibration**: a calibrated prior keeps the
  identity it came from in `RetentionPrior::origin`, and the projection carries
  it as `prior_origin`. `REQ-13-033`'s two halves —
  *stored versioned posterior differs from initial prior* and *prior remains
  identifiable* — are one assertion each.
* The *speed* has **no shipped value at all**. `PersonalizationSpeed` implements
  no `Default`, no constant of the type exists in this crate, and `calibrate`
  takes one by value, so nothing personalizes until somebody decides a minimum
  sample count and a step. Below that minimum, `calibrate` returns the prior
  unchanged and still `Uncalibrated`, which is section 13.3's cold start.

The one thing the shipped numbers are checked against is the design document's
own worked case: section 4's `2년 전 배운 Virtual Memory는 mastery가 유지된 채
freshness가 STALE로 보일 수 있고`, and the same sentence's
`최근 성능 debugging으로 다시 높아진`.
`the_shipped_prior_does_not_contradict_the_document` drives both. Passing that is
not evidence that the numbers are right; it is evidence that they are not already
wrong.

Five bands are computed from elapsed time and split at 0.5, 1, 2 and 4 windows,
one boundary per doubling. There is no finer structure because section 13.3
licenses none.

## `freshnessConfidence` is how well-founded the band is

`P2-N2` fixed that `estimateConfidence` is evidence sufficiency and not a skill
score. This is the same kind of value for the other axis, and its scale is the
design document's own: section 13.1's schema example reads
`freshnessBand: VERY_HIGH` with `freshnessConfidence: 0.92` over four evidence
items, an empty `contradictingEvidence` list and no recall history — a state with
exactly one thing missing, the calibration section 13.3 says the prior is waiting
for. So one gap costs 80 permille, and
`the_confidence_scale_is_the_schema_examples` reproduces that case rather than
asserting the constant.

## What a `STALE` band says to the person it is about

Section 34.2's row for `Knowledge Freshness를 실력 저하로 오인` gives the copy in
its `불확실성 표시` cell: `“과거 mastery 유지, 최근 사용 근거 없음” 문구`.
Section 13.3's example block gives it at length. `StaleDisclosure` carries both,
verbatim, and `stale_copy_says_past_mastery_remains` reads both back out of the
design document.

Three things it cannot say. It cannot appear on a band that is not `STALE`, so
there is no value of the type on a fresh concept. It cannot say the user does not
know something — the design document's own `Action` line ends
`“모름”으로 표시하지 않음`, and the copy is checked against eight spellings of
that with a control, since a token list that matches nothing is the empty guard
`docs/contracts/policy-source-scans.md` records this repository finding again and
again. And it cannot say anything about
mastery at all: the `Mastery:` line of section 13.3's block is the assertion's
own and is rendered by whoever holds it.

## Where the section 38 gate stands

**`GATE-38-024` is left open and this task fills in neither half.** The evidence
basis for the freshness priors and the speed of personalization stay
configuration decisions. The shipped default is labelled uncalibrated in its
name, in a typed basis value, in an accessor, in the trace and in a confidence
gap; the personalization speed has no shipped value at all and is unreachable
without a decision. Nothing here invents a calibration.

`GATE-38-023` and `GATE-38-025` belong to `P2-N2` and are untouched: nothing here
hides a facet, expires a confirmation or schedules a prompt.

## Named acceptance evidence

`cargo test -p academic-freshness` executes:

| Test | Evidence |
|---|---|
| `freshness_bands_are_exactly_six` | section 13.3's band sentence and its seven-bullet input list, measured against the design document in both directions, the array strictly decreasing under both orders, and a control on each reader |
| `stale_does_not_demote` | a `VERY_HIGH` history two years later reading `STALE` with the level byte-identical, the earlier version still readable at its own identity, and `P2-N2`'s own entry point doing the recomputation |
| `time_decay_touches_freshness_only` | a six-step clock sweep moving the band from `VERY_HIGH` to `STALE`, seven assertion versions carrying one level and more than one band, and the decay function's own signature |
| `spillover_is_one_hop_and_cited` | the four admitted predicates against section 7.2 and the sixteen refused, an uncited edge and a self-edge refused, the contribution strictly below its source and at or under the ceiling, `A → B → C` giving `C` nothing, `A`'s evidence refused as `B`'s use across a real edge, an edge that does not join its neighbour refused, ten neighbours giving what one gives, and both the zero-hop and wrong-subject misattributions refused |
| `debugging_evidence_persists_longer_than_exposure` | the direction read out of the bullet's own `보다` and `더 긴`, every section 13.2 row classified or windowless, and a seven-step sweep where debugging never falls below exposure and is strictly above it somewhere |
| `user_recall_confirmation_is_reflected` | both phrases against the document, opposite confirmations over one evidence history giving different bands, a `복습 필요` capping a top band, a statement that decays, ADR-003's matrix in both pairings by exact error, and the wrong predicate, band, concept and actor each refused |
| `recall_failure_prevents_freshness_increase` | all three phrases against the document, a failure capping a `VERY_HIGH`, eight further raisers dated at or before it moving nothing, the same raisers moving it when the failure is removed, relearning after it lifting the band, and no ceiling reaching `UNKNOWN` |
| `prior_is_versioned_and_identifiable_after_calibration` | the shipped name, basis and generation, the uncalibrated label in the accessor, the trace and the gaps, a cold start below the minimum, a calibration that moves the windows and the identity, the origin still naming the shipped default afterwards and in the projection, a second generation, failures moving it the other way, and both degenerate speeds refused |
| `stale_copy_says_past_mastery_remains` | section 13.3's four block lines and section 34.2's cell, both verbatim from the design document, the copy absent on a fresh band, both halves of `REQ-34-045`, and eight judgement spellings absent with a control that finds two of them where they are |

Beside them: `the_confidence_scale_is_the_schema_examples`,
`no_dated_evidence_can_carry_a_grade`,
`the_shipped_prior_does_not_contradict_the_document`,
`repetition_counts_occasions_and_not_items`,
`unknown_is_the_absence_of_a_record`,
`an_input_from_the_future_is_refused`, the eight source scans, and the nine
`compile_fail` cases.

**Every one of the nine was shown non-empty by injection.** Thirty-eight
injections were applied one at a time, each in its own build, and all thirty-eight
were caught. Two were remade because their first form changed an array's length
and did not compile, and an injection that does not build is not evidence. Three
of the campaign's findings changed this crate:

* deleting `claim.validate_for_actor` from `RecallStatement::verify` **passed**.
  The `Actor::User` destructure at the end of `verify` refuses a model run on its
  own, so an `is_err` assertion could not tell the two apart and ADR-003's matrix
  could have gone away with nothing observing it. The test now drives the pairing
  only the matrix refuses — the user's own actor on a claim carrying a model's
  authority — and asserts the exact error.
* deleting the `!edge.joins(neighbor)` guard from `NeighborUse::direct` **passed**,
  because `Spillover::toward` re-checks the same shape one step later. The guard
  is kept, because a `NeighborUse` whose edge does not name its own neighbour is
  a value a caller can hold, and it is now observed directly.
* the spillover **ceiling** was unobserved. `spillover_is_one_hop_and_cited`
  compared the contribution against its source, which the one-step demotion
  satisfies on its own, so removing `SPILLOVER_CEILING` entirely would have
  passed. The ceiling is now a separate assertion.

All fixtures are synthetic and built in process. The lecture evidence is a node
of a document `P2-L4`'s builder produced over a real `P2-L2` capture and a real
`P2-L3` run, reached through `P2-N2`'s own fixture module rather than restated.
Every operation is local and deterministic, and no clock is read.

## What this task does not decide

* **Mastery.** `P2-N2` owns the ladder, the facets, the ceilings and the
  eligibility gate. This crate reads its `EligibleEvidence` and hands back a
  `FreshnessInput`.
* **Persistence.** Nothing here is written. There is no migration and no edge to
  `academic-store`; `as_of` is an argument and no clock is read.
* **The graph.** Whether a cited edge is a stated one or a transitively derived
  one is the graph layer's fact. This crate requires the edge to name this
  concept and the neighbour directly, to carry the evidence section 7.3 requires,
  and to be one of four predicates; it holds no registry and traverses nothing.
* **Gaps and paths.** `P2-N5` and `P2-N6`.
