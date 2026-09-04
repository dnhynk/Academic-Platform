# Offering status and the calibrated forecast behind it

`academic-offering` is `P2-U5`. It holds section 8.3's four offering statuses as
four types, the seven feature families that decide which one an unconfirmed
offering carries, the calibrated probability and sample window every forecast
records, and the per-term Brier/coverage/abstention evaluation. It persists
nothing, opens nothing, and runs no connector: migration `0014` already holds
`offering_detail.official_status` with the four-value `CHECK`, migration `0001`
already holds `prediction_metadata_version`, and `crates/store` owns both.

This engine answers *will this course be offered next term*, and a user plans
their degree on the answer. One sentence is behind every contract below: **a
prediction never becomes an official fact.**

## Current outcome

No real academic record can reach this engine and none has. Admission is closed,
ADR-002 is unaccepted, the default lane reports `storage_encryption=NONE` and
`production_data_allowed=false`. Every fixture here is synthetic and built by
`academic_offering::corpus`: ten named histories over course codes shaped like
the catalogue's that name nothing in it, no instructor who is a person, and no
term that was read from a registration system.

`GATE-38-017` stays open, every term. See
[What is deliberately undecided](#what-is-deliberately-undecided).

## Four statuses, four types

Section 8.3's table:

| 상태 | 요건 | UI 문구 | Planner 취급 |
|---|---|---|---|
| `CONFIRMED` | 해당 학기 공식 수강편람/수강신청 시스템에 존재하고 최근 확인 | 공식 개설 확인 · 확인일 | 실제 시간표·정원 사용 |
| `HISTORICALLY_LIKELY` | 여러 과거 학기의 재현 가능한 패턴, 미래 공식 공지 없음 | 과거 패턴상 가능성 | placeholder만, 졸업계획 확정에 사용 금지 |
| `UNCERTAIN` | 표본 부족·불규칙·교수 변동 | 근거 부족 | 경고와 대체 경로 요구 |
| `CANCELLED/WITHDRAWN` | 공식 폐강·변경 공지 | 공식 취소 | 선택 불가, 과거 scenario 보존 |

**The table has four rows and the fourth row's name has a slash in it.** `t068`
§2.3-4 writes the fourth status as `CANCELLED`, migration `0014`'s `CHECK`
admits `CANCELLED`, and `academic_curriculum::OfferingStatus` declares four
variants — so `CANCELLED/WITHDRAWN` is one status under two spellings and not a
fifth. `the_four_standings_are_section_8_3s_own` reads the table out of the
design document and asserts that divergence exactly rather than papering over
it: the document cell must *start with* the enumeration's spelling, and the one
row where they differ must be that one.

Each row is its own struct with private fields and its own `UI_COPY` and
`PLANNER_TREATMENT` constants, compared cell for cell against the document in
both directions. What that buys is not tidiness:

- `ConfirmedStanding` holds a `ConfirmationEvidence`, whose single constructor
  takes a `SourceCategory::RegistrationSystem` reading inside a recorded
  verification bound. A forecast holds no such reading, so **there is no
  expression that turns a prediction into a confirmation** — not a check that
  could be skipped, an argument that cannot be supplied.
- `HistoricallyLikelyStanding` holds a `ScoredForecast` **by value**, which
  holds a `CalibratedConfidence` — issued only by `P2-M1`'s registry — and a
  `PredictionMetadata`, whose constructor refuses a zero positive-sample count.
  A likely standing with an uncalibrated number or an undisclosed window is not
  a value that exists.
- `ConfirmedStanding::seat` is the only producer of a `ConfirmedSeat` in this
  workspace, and `DeterminatePlan::commit` takes seats by value. So
  `HISTORICALLY_LIKELY` cannot enter a determinate plan because there is no seat
  for it to enter as.

That is `P2-N2`'s *`AutomaticLevel` has no `Fluent`*, `P2-R5`'s
*`AuthorshipMode` has no review value*, `P2-L4`'s *a `PdfArtifact` without a
witness is `INCOMPLETE`* and `P2-U2`'s *two-attestation gate is a type*, applied
to a fourth thing.

## Seven feature families, and each one measured

Section 8.3: *역사 기반 예측은 최근 N개 학기의 단순 다수결이 아니다. 계절성(1/2학기),
교과목 신설·폐지·대체, 교수자 변화, 최근 공지, 미개설 gap, 불규칙 특강 여부를 feature로
사용하고, Course별 calibrated probability와 표본 window를 남긴다.*

### Six named features, seven named families

**That sentence names six things as features.** `t068` §5's `P2-U5` entry and
`t001`'s `REQ-08-029` row both say **seven feature families** and both resolve
the seventh as the history window, which the same sentence requires be recorded
as 표본 window. This crate implements seven, and the seventh is the window
depth: two courses with the same seasonal rate and the same gap are not equally
predictable when one was read four times and the other twice.

`the_feature_families_are_section_8_3s_own` splits the sentence at *를 feature로
사용하고*, compares the six units against the first six families in order and in
both directions, and requires the seventh's phrase to be in the sentence after
the split and not before it. So the divergence between six and seven is executed
rather than described, and a seventh unit appearing in the sentence fails here.

This is the seventh count divergence found in this plan, after §28's twelve
engines called thirteen, §31.3's fifteen dimensions called thirteen, §14.2's six
states called seven, `P2-U1`'s "five names, four relations", §11.2's fourteen
rule types called thirteen, and §11.3's five leaf tokens that are not the
harness's five.

### The rule set

`FORECAST_RULE_SET` is the published text and its SHA-256 is the `rule_set_hash`
every outcome is bound to, so changing a weight changes the canonical bytes of
every evaluation. It is written out rather than rendered from the code, because
a rule set generated from the implementation would agree with it by
construction.

| family | value | contribution |
|---|---|---|
| `seasonality` | `positive * 1000 / terms`, over same-semester terms | `(value − 500) * 2 / 5` |
| `lifecycle_status` | 0 unknown, 1 established, 2 new-and-started, 3 new-not-yet, 4 sunset-after-target, 5 sunset-at-or-before-target | `+0, +0, +60, −500, −40, −500` |
| `instructor_change` | distinct instructor sets over offered terms | `0 → +0`, `1 → +60`, `2 → −60`, `3+ → −120` |
| `recent_notices` | `80·announced − 200·suspended − 60·curriculum_change` | `clamp(value, −300, 300)` |
| `offering_gap` | same-semester terms since the last offered one | `0 → +60`, `1 → −60`, `2 → −160`, `3+ → −260` |
| `irregular_special` | irregular offered terms | none `+0`, all `−200`, some `−100` |
| `history_window` | same-semester terms read | `0..1 → −150`, `2 → −40`, `3 → +30`, `4+ → +80` |

The raw score is `clamp(500 + Σ contributions, 0, 1000)`. Five hundred is *no
information*, not *even odds measured*.

### Seasonal by construction, which is what refuses the majority vote

Section 8.3's first feature is 계절성(1/2학기), so the window a spring forecast
reads is the **spring** terms of the history and not the last N terms. That one
restriction is what makes the seasonal rate, the gap and the window depth all
answers about the same semester, and it is what separates this from the vote the
specification refuses: a course that runs every spring and never in autumn reads
1000 permille for spring and 0 for autumn, where a vote over the last N terms
reads 500 for both. The corpus's `every_spring` and
`spring_only_asked_for_autumn` are that pair, and they land on different
statuses.

### Every family moves the score, and that is measured

Declaring a family and not using it is the failure this crate is written
against. `offering_feature_contract` gives **each family its own pair** — a
control and a variant that differ in that family alone — and requires the raw
score to move and every *other* family's contribution to stay equal. A family
whose arm collapsed to a constant fails there rather than passing as
documentation.

The pair is the unit rather than one shared baseline because two families cannot
always be varied against the same control: a course offered in every term of its
window has no gap to lengthen without also changing its seasonal rate. The first
attempt at this test used one baseline and failed on exactly that — it is
recorded here because the failure is the reason for the shape.

The row that separates a seasonal forecaster from a majority vote is the
corpus's `gap_two` / `every_other_spring` pair: the same seasonal rate (500
permille), the same window depth (4), the same single instructor, no notices,
and the two offered terms at opposite ends of the window. They land on 330 and
780 permille, and on `UNCERTAIN` and `HISTORICALLY_LIKELY`.

## The calibrated probability, and the rung below it

`P2-M1`'s `CalibrationRegistry::interpret` is the only producer of a
`CalibratedConfidence` in this workspace and `DisplayedConfidence::of` takes
one, so a raw score reaching a reader is a type error. This crate is a consumer
of that boundary rather than an exception to it: the forecaster registers as its
own provider (`snu.offering.history`), its `model_version` is the engine
version, and its purpose is `offering.forecast.next_term`. With no dataset
registered for that exact key, or with the registered one stale, the forecast
**abstains** — `AbstentionReason::NoFreshCalibrationDataset` — rather than
showing an uninterpreted number. `Forecast::raw_units` exists and is documented
as not displayable: nothing in the crate formats it for a reader.

The curve in the corpus is deliberately **not** the identity. The raw scale is
compressed at both ends, so a fixture that read the raw number where it should
read the calibrated one produces a different answer.

## The sample window is `§2.3-15`'s existing shape

`t068` §2.3-15 pins `prediction_metadata_version = 1` by `CHECK` and requires an
active `PREDICTION` claim to carry confidence plus a bounded observation window
and a positive sample count. This crate reuses that shape and mints nothing:
`ScoredForecast::metadata` is an `academic_domain::PredictionMetadata` at
version 1, its `positive_sample_count` is how many same-semester terms held a
section, and its `PredictionObservationWindow` is the span of instants the
readings actually happened over.

**Instants, not terms.** The window is built from each `TermObservation`'s
`read_at` — §8.2's `observedAt`, the instant somebody read the registration
system for that term — rather than by converting a term range through a
term-to-date table no confirmed source supplies. Everything else in this crate
is ordered on `academic_record::TermKey`, for the reason `P2-U4` recorded: an
effective date in this domain is written as *2015학년도 1학기 이수 교과목부터*.
The two axes never cross, and this crate holds no conversion between them.

### Zero observations is an abstention, twice over

Section 8.3: *과거에 한 번도 관찰하지 못한 것은 `UNCERTAIN`이며 미개설 확정이
아니다.* `AbstentionReason::NeverObserved` is the explicit arm.
`PredictionMetadata::new` refusing a zero positive-sample count is the
structural one: a never-observed course has no metadata to disclose, and
`ScoredForecast` takes the metadata by value, so there is no scored forecast for
it to become. The check and the type agree, and the type is the one that cannot
be forgotten.

**An unobserved term is not a term with no offering.** A term nobody read is
absent from the history; a term somebody read and found no section in is present
with `Offered::No`. Only the second enters the seasonal rate, and neither
produces a claim that the course will not run: `OfferingAssertion::DoesNotRun`
is reachable only from an official cancellation notice, and a forecast has no
producer for it.

## The two claims, and `SUPERSEDED_FOR_DECISION`

`t068` §2.3-4: offering status is **not** a claim status; it is an aggregate
field backed by `OFFICIAL_CONFIRMED` and `PREDICTION` claims. §30.1 writes what
happens when both exist: *When A arrives, B is not rewritten as official. B
becomes `SUPERSEDED_FOR_DECISION` while its prediction history remains.*

`forecast_claim` takes a `ScoredForecast`; `confirmation_claim` takes a
`ConfirmationEvidence`. Neither takes the other's argument, and there is no
conversion between them. The claim layer refuses the same thing a second time on
its own terms: `Claim::validate` pairs `EpistemicStatus::Prediction` with
`AuthorityClass::Prediction` alone, requires a confidence on it, and requires
prediction metadata on it and on nothing else — so a prediction relabelled
`OFFICIAL_CONFIRMED` fails validation twice over.

**`SUPERSEDED_FOR_DECISION` is not an edit.** The canonical claim table is
append-only twice over (`t068` §2.3-2) and `EpistemicStatus::Superseded` is
lifecycle-terminal and derived, so §30.1's phrase is not a value written onto
the prediction row: the prediction's bytes never change. It is a property of the
*set* — which claim a decision reads — and `OfferingClaimSet::prediction_standing`
is where it is answered. `t001`'s `REQ-30-002` row records the exact supersession
status naming as open; this crate does not settle it by inventing an
`EpistemicStatus` variant.

`prediction_official_parallel` observes both halves: two claim identifiers, two
statuses, the prediction claim byte-identical before and after, and the whole
`Forecast` — result, tree, explanation, canonical bytes — identical with and
without the official reading beside it. The forecast runs **first and
unconditionally** in `standing::resolve` for exactly that reason.

### What this task found one step out

ADR-003's actor matrix in `Claim::validate_for_actor` gives
`AuthorityClass::Prediction` to `Actor::ModelRun` alone.
`Actor::DeterministicEngine` carries `AuthorityClass::DeterministicEngine` and
nothing else, so **a deterministic historical forecaster cannot sign its own
prediction claim as a deterministic engine** — while §30.1's own example of a
`PREDICTION` claim is *status PREDICTION · historical pattern · confidence .72*,
which is a pattern and not a model. This crate does not widen the matrix. It
records the divergence, and
`a_forecast_claim_is_not_signable_by_a_deterministic_engine` executes it, so a
later widening is a deliberate change rather than a silent one. `S-21` in
[policy source scans](policy-source-scans.md) carries it as open.

## Source authority

Section 8.3: *공식 개설 확인은 [서울대학교 수강신청 시스템]의 최신 강좌 상세를
기준으로 하고, CSE 홈페이지·수강편람은 교차 출처로 사용한다.*

That sentence names a basis, and it is **not** §8.4's mechanical winner: §8.4
forbids deciding a *regulation* conflict by a source's number in a list, and
this is not a regulation conflict — it is the one reading that says whether a
section exists in a term, stated as such by the specification.

`ConfirmationEvidence::from_registration_system` compares against
`SourceCategory::RegistrationSystem` and refuses every other level, and
`offering_source_authority` runs **every value of `SourceCategory::ALL`** through
it, so a level added to that enumeration arrives refused rather than
unconsidered. It also refuses a reading older than the recorded
`VerificationRecency`, and a reading that found no section — which is an
observation about the term, not a confirmation of one.

A cross source that disagrees is **disclosed, not dropped and not promoted**.
There is no method that makes a cross source the basis: not by being newer, not
by being more numerous, and not by being a higher-numbered §8.4 level. §8.3 says
the 수강편람 can change after its own compilation date, so a stale cross source
disagreeing is the expected case and hiding it would lose the reason to
re-check.

### A retirement is not a cancellation until somebody records the notice

`CourseLifecycle::RetiredFrom` and `ReplacedFrom` are the 교과목 폐지·대체 half
of the second feature family, and a retirement effective at or before the
forecast term contributes −500. It does **not** by itself produce a `CANCELLED`
standing: §8.3's fourth row requires 공식 폐강·변경 공지 — an official notice —
and this crate does not infer one from a lifecycle field. A `CancellationNotice`
carries its source and its issue instant, and only that reaches the fourth row.
The corpus's `retired` case is therefore `UNCERTAIN` at 330 permille rather than
`CANCELLED`, which yields no seat either way.

## `UNCERTAIN` has no negative twin, and that is deliberate

§8.3 offers no status for *this course will confidently not run*. A calibrated
probability below the recorded floor therefore reads `UNCERTAIN` with the
probability disclosed — `UncertainStanding::scored` carries it — and never a
claim that the course will not be offered. That is the same sentence as
*미개설 확정이 아니다*, applied to a confident negative rather than to an absence.

## The three recorded criteria, and why none has a default

§8.3 puts a bound in three places and states no number: how recent 최근 확인 is,
how many terms 여러 과거 학기 is, and how likely 재현 가능한 패턴 is. `t001`'s
`REQ-08-024`, `REQ-08-025` and `REQ-08-026` rows each record the missing number
as an open gate candidate.

So `VerificationRecency` and `ForecastPolicy` have private fields, one
constructor each that takes every bound, **no `Default`** and no associated
constant — the shape `P2-U3` used for `SourceFreshnessPolicy` for the same
reason. `ConfirmationEvidence::from_registration_system` cannot be called
without a recency bound, and `standing::resolve` takes `Option<ForecastPolicy>`
and abstains with `AbstentionReason::ForecastPolicyAbsent` when it is `None`.
The corpus records a **synthetic, user-confirmed** pair, labelled as such, so a
case that reaches each side exists to check.

## The harness, and why nothing flips

`P2-C5` fixes every §28 engine as a pure
`(frozen_inputs, rule_set_hash, engine_version)` function, and `forecast` is
written to that signature: no clock, no RNG, no socket, no model, and byte-equal
`EngineOutcome::canonical_bytes` over equal inputs under equal hashes.
`same_inputs_and_rule_hash_yield_byte_equal_results` asserts both halves — equal
bytes for two evaluations, and *different* bytes under a different rule-set
hash, without which the first would pass on an encoding that ignored the hash.

**§28's table names twelve engines and none of them is an offering forecast.**
`schemas/registry/engine-registry-v1.json` is that table and nothing else, and
[the engine harness contract](engine-harness.md) says the comparison against §28
is an enumeration — so an entry this task added would fail
`engine_registry_is_complete` against the design document. Nothing here flips a
registry entry, nothing here sits under `testdata/engines/`, and
`OFFERING_FORECAST_ENGINE_ID` is deliberately outside the `engine.` namespace.
`this_crate_persists_nothing_and_registers_no_engine` executes all four of
those: the registry does not name this engine, this engine does not claim the
registry's namespace, the registry still holds exactly twelve entries, and
`testdata/engines/` holds no directory of this crate's.

The corpus lives at `testdata/offering-forecast/` instead, and the independent
oracle is `tools/offering-forecast-oracle.mjs`.

## The independent oracle

A forecast checked against numbers the forecaster produced proves only that the
forecaster is deterministic — and it is a particularly easy mistake here,
because a probability looks like a measurement whichever side it came from. So
every expected value is derived in another language, from a second transcription
of the corpus, the rule set, the calibration curve and the arithmetic.

`the_corpus_agrees_with_an_independent_oracle` compares, for every one of the
ten cases: the course, the raw score, the window depth, the positive sample
count, **all seven families' values and contributions**, the calibrated
probability or the abstention, and the standing.
`offering_forecast_oracle_is_committed_and_re_derivable` asserts the committed
file re-renders identically, so it cannot be hand-edited into agreement with a
broken forecaster.

## Per-term metrics

Section 8.3: *예측 성능은 학기마다 Brier score/coverage와 abstention rate로
검증한다.*

**Exact integers, no floating point.** The Brier score is carried as a rational:
`brier_numerator` is the sum of squared permille errors and `brier_denominator`
is how many forecasts it is a sum over, so comparing one term against another is
cross-multiplication — exactly as `P2-U4` compares a grade-point average without
dividing. `no_floating_point_reaches_a_forecast` sweeps every product file for
`f32`, `f64`, `as f` and three float literals, and runs the rule against four
evasions inside the test so a rule that matched nothing fails.

**Coverage and abstention are different questions.** Abstention is
`abstained / total`: how often the forecaster declined. Coverage is
`resolved / total`, where a forecast is resolved when it was scored *and* the
term's realized outcome was recorded. They are not complements, and the gap
between them is `missing_outcomes` — courses the forecaster spoke about and
nobody afterwards checked. On the shipped corpus they read 500 and 400 permille,
which is what says the two were kept apart rather than one computed and printed
twice.

**An empty denominator is not a perfect score.** `brier_numerator` and
`brier_denominator` are `None` when nothing resolved. A Brier of zero over zero
forecasts would read as a flawless term, which is how a silently-degrading model
stays invisible. `TermEvaluation::new` refuses an empty entry list for the same
reason.

## Executable evidence

`crates/offering/tests/offering.rs` holds `t068`'s twelve named tests plus the
three halves they rest on:

| Test | Requirement | What it holds |
|---|---|---|
| `offering_confirmed_contract` | `REQ-08-024` | only a fresh registration reading listing a section confirms; the seat carries the real timetable and capacity; a stale reading and an empty reading are each refused by name |
| `historical_likely_limits` | `REQ-08-025` | a six-term seasonal pattern is `HISTORICALLY_LIKELY` in §8.3's words, the number shown names its dataset, there is no seat, the plan refuses it, and an official reading takes the standing away |
| `uncertain_offering_flow` | `REQ-08-026` | §8.3's three grounds each reach `UNCERTAIN` and name themselves; the refusal carries the alternative path; an absent policy and a stale dataset each abstain by name |
| `cancelled_offering_contract` | `REQ-08-027` | a cancellation makes a new plan impossible and leaves the already-committed one byte-identical; a notice from a level that publishes no offering changes is refused |
| `offering_source_authority` | `REQ-08-028` | every `SourceCategory` through the one constructor; a stale *and* a newer cross source each disclosed and neither promoted |
| `offering_feature_contract` | `REQ-08-029` | seven families, one pair each, each moving the score and only its own contribution; the majority-vote refutation |
| `course_forecast_metadata` | `REQ-08-030` | course, calibrated probability with its dataset, and `prediction_metadata` v1 with the exact disclosed instants |
| `prediction_official_parallel` | `REQ-08-031`, `REQ-30-002` | two claims, two statuses, prediction bytes unchanged, forecast bytes unchanged, `SUPERSEDED_FOR_DECISION` |
| `zero_observation_semantics` | `REQ-08-032` | two never-observed cases abstain; the metadata cannot be built at all; an unread term and an empty term are different values |
| `term_forecast_metrics` | `REQ-08-033` | three metrics against the oracle, missing outcomes reported, no Brier over an empty denominator, empty evaluation refused |
| `offering_epistemic_split` | `REQ-04-011`, `REQ-APA-011` | four distinct statuses, four distinct UI strings, four distinct planner cells, one seat among them |
| `historically_likely_cannot_enter_determinate_plan` | §8.3 planner cell | no seat on three standings; the confirmed control commits; a seat for another term is refused by name |
| `the_corpus_agrees_with_an_independent_oracle` | — | every case against a second transcription in another language |
| `same_inputs_and_rule_hash_yield_byte_equal_results` | `P2-C5` | byte equality, and inequality under a different rule-set hash |
| `a_forecast_claim_is_not_signable_by_a_deterministic_engine` | — | the ADR-003 actor-matrix divergence, executed |

`crates/offering/tests/offering_scans.rs` holds nine scans and
`crates/offering/tests/compile_fail/` holds seven cases; both are enumerated in
[policy source scans](policy-source-scans.md).

### One compile-fail case per privacy error

Rust runs the privacy pass **after** type checking, so a file whose type
checking already failed never reaches it and the `E0451` a case exists for is
never emitted. A case that bundled a struct literal with a wrong-arity call
would still fail to compile, still pass the suite, and prove only the arity.
This task found that in its own first draft: two of five cases were bundled that
way and their committed `.stderr` carried no `E0451` at all. The two literal
cases now hold nothing but the literal, and the committed diagnostic beside each
is what says so.

## What is deliberately undecided

`GATE-38-017` — §38.2's seventh bullet, *해당 학기의 최신 CourseOffering, 교수자,
정원, 시간표, syllabus, 평가 방식* — stays open **every term**. Those are facts a
user or a connector retrieves once per term and they go stale by the next one.

So there is no table of them here, no default capacity, no assumed timetable and
no cached instructor. What stands while the cell is empty is that
`ConfirmationEvidence` cannot be built at all, so no offering is `CONFIRMED` and
every standing falls to the forecast or to `UNCERTAIN`.
`the_open_gate_holds_every_term` runs all ten corpus cases and requires each one
to reach neither `CONFIRMED` nor a seat.

There is also no *fill* function and no notion of the cell being closed. §38 asks
for the reading, not for a decision, and a reading recorded last term is not the
reading this term needs.

`the_open_gate_is_section_38s_own` derives `GATE-38-017` from the bullet's
position: it reads §38.1's ten-line block and §38.2's eleven bullets out of the
design document and requires the identifier to be `GATE-38-{position + lines +
1}`. `P2-U3` found that eleven of the eighteen `OpenGate::identifier` arms in
this workspace were hand-written strings checked only against a hand-written
list in the same test and left the rest as `S-20`; this crate's one arm is
derived from the start and is not among them.

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`. Every history, listing, notice, dataset and outcome in
this crate's tests is synthetic and built in process; the crate calls no
connector and no model, and its link closure holds nothing that can open a
socket.

## Enforcement

- `cargo test -p academic-offering` — the twelve named tests, the nine scans and
  the seven compile-fail cases, on every supported platform.
- `pnpm test` — `offering_forecast_oracle_is_committed_and_re_derivable`, and
  the crate-graph, socket-closure and admission-receipt rows in
  `tools/phase1-scaffold-policy.test.mjs`.
- `pnpm offering:oracle` — the oracle re-render on its own.
