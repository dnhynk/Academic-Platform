# GPA, credit accounting, and the attempt model

The `P2-U4` boundary in `academic-record`: the section 10 attempt model, the
versioned `GradingScheme` and `RepeatPolicy`, and the two §28 engines
`engine.gpa` and `engine.credit.accounting`.

## Current outcome

Every average this crate publishes is over a synthetic corpus. Admission is
closed (`P2-K6` built a verifier and did not open admission), ADR-002 is
unaccepted, the default lane reports `storage_encryption=NONE` and
`production_data_allowed=false`, and `academic-transcript`'s two gated entry
points refuse on every profile in this repository. No real academic record can
reach these engines, and none has.

`GATE-38-005` (the official transcript), `GATE-38-006` (transferred and exchange
credit recognition) and `GATE-38-016` (external recognition rules) stay open.
See [What is deliberately undecided](#what-is-deliberately-undecided).

## The attempt ledger

`AttemptHistory` has one mutator, `append`, and its correction form
`append_correction`. There is no removal, no `&mut` borrow of a stored attempt,
and no public field on `CourseAttempt`. A repeat, a cancellation, and a
correction are all new entries.

- `all()` is the ledger. It only grows.
- `current()` is the resolver projection: the entries no later entry superseded.
  It is derived on every call, so there is no second copy to fall out of step.
- A correction carries ADR-003's `SUPERSEDES` `ClaimRelation`, built by
  `append_correction` rather than accepted from the caller, so its kind cannot
  be something weaker. Both claims are user-confirmed attempt assertions, which
  is the nonterminal authority/status pair ADR-003's fail-closed matrix admits
  for a state-removing relation authored by the user.

`attempt_history_append_only` appends a correction, observes `current` drop the
corrected attempt and `all` not shrink, and observes the superseded attempt
still readable and unchanged by its own identity.

The status set is section 10's eight and the repeat set is its four, both
closed. `AttemptStatus::Planned` is present because the schema names it and
**no constructor in this crate produces it** — see the next section.

## Where an attempt comes from

Two constructors, and neither takes a status the caller chose:

| constructor | produces | gate |
|---|---|---|
| `CourseAttempt::from_confirmed_registration` | `REGISTERED` | a `RegistrationConfirmation`, which requires at least one evidence identifier |
| `CourseAttempt::from_confirmed_row` | one of four settled statuses | `SettledStatus`, an argument type that cannot spell `PLANNED`, `REGISTERED`, `IN_PROGRESS`, or `CANCELLED` |

`ingest::attempt_from_confirmed_row` is the `P2-U7` seam and adds a third gate
on top of the second: it takes the `ConfirmedRowClaim` that crate minted and
refuses

- a claim whose ordinal is not the row's,
- a claim whose object text is not the row's four fields,
- a claim that is not `UserExplicit`/`UserConfirmed` — which is what an *import*
  row is.

None of that re-decides what a confirmation is. `confirm_reconciled_rows` mints
one only from a `ReconciledTranscript`, which `reconcile` returns only when every
row agreed on all four fields, and `Claim::validate_for_actor` permits
`UserExplicit` to `Actor::User` and to nobody else.

`crate::plan` declares no method returning a `CourseAttempt` and none returning
a `RegistrationConfirmation`, and neither `CourseAttempt` constructor accepts a
`PlanScenarioChoice` — so a plan choice has nothing to hand either of them.

That is a statement about this crate's surface as it stands, not a proof that no
such path could be added; there is no compile-fail case here.
`registered_attempt_gate` executes what *is* checkable: the constructors and the
statuses each produces, the refusal of a registration with no evidence, and the
three refusals on the `P2-U7` seam.

### Term ordering, and what is refused

An effective-dated policy row is compared against a term, so a term needs an
order. A `TranscriptRow`'s term is whatever the document wrote. The declared
mapping is:

| spelling | term |
|---|---|
| `2026_FALL` | the canonical form, section 10's own |
| `2024-1`, `2024-2` | 1학기, 2학기 |

Everything else is refused. **A 계절학기 is not guessed**: no source in this
repository states how a summer or winter term is written on a transcript, and a
term placed in the wrong session by a guess would move an attempt to the wrong
side of an effective date.

## Classification is a rule-engine output

Two independent mechanisms, and neither is a comment.

**Construction.** `RequirementClassification`'s fields are private to
`classify.rs` and it has no public constructor, no `Default`, no builder, and no
setter. The only values that exist anywhere are the ones
`ClassificationRuleSet::classify` returned, and each names the rule and the rule
set version that produced it. A course no rule mentions gets **no**
classification rather than a defaulted 일선: "the rule set does not mention this
course" and "the rule set says this is a free elective" are different facts and
only the second is published.

**Assertion.** A classification travels as a claim under
`AuthorityClass::DeterministicEngine`, and ADR-003's actor matrix permits
`Actor::User` exactly one authority class — `UserExplicit`.
`classification_by_ruleset` executes both directions: the claim handed a user
actor is refused, and the same claim rebuilt as `UserExplicit` is refused for the
engine actor. There is no pairing that lets a hand-written category through.

A classification is scoped to a programme, which is what makes multi-major
representable: one attempt is 전선 under `cse` and 일선 under `stat` in the
shipped corpus, and each programme's average reads its own category.

## The two dated policies

Section 10 states two dated facts and this crate encodes both as rows in a
`PolicyBook`, selected by the attempt's own term:

| row | what it says | source |
|---|---|---|
| `repeat.ceiling.2015_spring` | an undergraduate repeat grade is capped at `A0` from 2015 spring onward | the 2026-2 수강신청 안내, as section 10 quotes it |
| `external.excluded.2004_spring` | a grade earned at another university from 2004 onward is not in the 본교 평점평균 | the 평점환산기준표 유의사항, as section 10 quotes it |

A lookup returns the last row whose `effective_from` is at or before the term.
**A term no row reaches resolves to `None`, and the engine reports that as
`UNKNOWN`** — never as "the rule does not apply", which would be a policy claim
about a period no source here covers.

`repeat_ceiling_effective_date` evaluates one attempt set under three effective
terms: the published `2015_SPRING` (average `2.83`), `2016_SPRING` (average
`2.9`, because the repeat now falls under the no-ceiling row), and
`2014_SPRING` (still `2.83`, because the repeat is on the same side of the
boundary). A ceiling hard-coded on a `2015` comparison passes the first and
third and fails the second; that injection was run and observed.

## Arithmetic

Every value is an `academic_domain::Decimal` — a coefficient and a scale.
`crates/record/src/decimal.rs` supplies the operations and **introduces no
second numeric type**. No `f32`, no `f64`, and no floating-point literal appears
anywhere in the crate.

Rounding happens in exactly one function, `div_round_half_up`, and both the
scale and the rule are arguments: the scale comes from the versioned
`GradingScheme`, so `gpa_policy_version_matrix` changes a scheme's published
scale and observes the average move.

The shipped corpus averages `33.9 / 12`, which is exactly `2.825` — a tie at the
second digit. Half away from zero publishes `2.83`. The nearest `f64` to `2.825`
is below it, so a floating-point implementation publishes `2.82` and the fixture
fails. That row is the float detector, and it is why the corpus is built to land
on a tie.

### How the arithmetic is checked

`gpa_formula_fixture` does **not** compare the implementation to itself. Its
expected values come from `tools/gpa-oracle.mjs`: a second transcription of the
corpus, a second transcription of the grade table, and a second arithmetic —
fixed-point `BigInt` units in another language, with no rescaling step — written
against the specification rather than against `crates/record`. The oracle renders
`testdata/engines/gpa/oracle.expected`; `node tools/gpa-oracle.mjs --check`
re-renders and byte-compares, so the file cannot be hand-edited either.

Changing a grade in the Rust corpus moves one side of that comparison and not the
other. That injection was run and observed.

## The views section 10 requires

`RecordViews` is built once and every view is a projection of the *same*
dispositions, so two views cannot disagree about one attempt.

| view | accessor |
|---|---|
| cumulative average, and the attempts it used | `cumulative_gpa`, `cumulative_included` |
| per term | `terms`, `term_gpa` |
| per programme, 전공 only | `programs`, `major_gpa` |
| earned credits **versus** the GPA denominator | `earned_credits`, `gpa_denominator` |
| the repeat group, before and after | `repeat_proofs` |
| why each attempt contributes what it does | `dispositions` |

### Earned credits and the GPA denominator are different quantities

This is the distinction the whole task turns on, and the corpus is built so the
two differ by construction: `14` earned against a denominator of `12`. Four
attempts each move exactly one side.

| attempt | earned | denominator |
|---|---|---|
| `S` | yes | no |
| `F` | no | **yes** |
| `W` | no | no |
| a recognized exchange grade after 2004 | yes | no |

An `F` that left the denominator would raise the average of a student who
failed, which is the wrong direction. `credits_vs_denominator` asserts the two
totals differ, re-derives the gap attempt by attempt from the per-attempt
reasons, and a property test asserts an `S` moves one side and never the other
over generated attempt sets.

Both totals come back as a `CreditTotal`, whose number a caller has to name:
`complete()` returns `None` while any attempt's contribution is pending, and
`partial()` is the sum of what is known. A public `total` field beside a list of
pending attempts would be read as a total — the same defect the engines avoid by
publishing no derived value under `CONFLICT`, one step outside them.

### Every attempt has exactly one reason

`DispositionReason` is a closed set of thirteen.
`special_attempt_reason_matrix` walks the whole product of settled status,
origin, grade symbol, and recognition decision — every triple the constructors
admit — and requires each to produce exactly one disposition inside the set.
Totality is by arithmetic over the product, not by a list of cases somebody
thought of.

## The two engines

Both are pure functions of `(frozen_inputs, rule_set_hash, engine_version)`.
Neither reads a clock, an RNG, a socket, or a model, and neither holds mutable
state. `ruleset.txt` in each harness directory is the `RuleBook`'s canonical
text and its SHA-256 is the hash every fixture is evaluated under; an engine
refuses a hash that is not its own book's, so an average can never be attributed
to a rule set that did not produce it.

The attempt set reaches the engine entirely through the frozen inputs, and the
product views compute from the same decoded facts — so a golden fixture and a
product call run the same arithmetic over the same values rather than over two
paths that agree by inspection.

### A value is published only when it is fully determined

The temptation is to publish the arithmetic that ran and let the status carry
the caveat. A reader who sees `gpa=2.65` beside a `CONFLICT` has been handed an
average over a record that disagrees with itself, and the number is what
survives into a screenshot. So:

- a `CONFLICT` evaluation publishes no derived value at all;
- an `UNKNOWN` one publishes only the totals no unknown attempt touched;
- `attempts.pending.disposition` always says how many are outstanding.

### The three adverse paths

`GPA` is one of the four high-impact paths, so its harness carries all three.
Each is a state the *shipped* rules reach under the *baseline* book — not a
state contrived for the directory:

| path | how it is reached |
|---|---|
| `unknown` | an exchange attempt in 2003, which no dated external row reaches |
| `conflict` | two settled attempts at one course in one term, neither a repeat of the other |
| `partial_failure` | a term scope the attempt set has nothing in; `rule.gpa.average` is left unevaluated rather than answered with a zero |

### Credit accounting traces double recognition rather than resolving it

§28's invariant for `CREDIT_ACCOUNTING` is "한 학점의 중복 인정 근거 추적" —
trace the basis on which one credit is recognized twice. `GATE-38-015`
(multi-major double-counting) is open, so the engine names every credit that
reached two programmes' totals, with both categories, and reports `UNKNOWN`.
Choosing a rule here would close that gate by invention.

## Deleting a plan

`delete_scenario(&mut PlanStore, &AttemptHistory, EntityId)`. The history
argument is an **immutable** borrow and is read only to report how many attempts
survived: a deletion that could reach the ledger would need `&mut`, and
`AttemptHistory` has no removal mutator to reach in any case.
`delete_plan_preserves_attempts` deletes a scenario whose every choice names a
course the ledger has attempts for, and compares the ledger and every
disposition before and after.

**This is not `P2-K5`'s deletion.** `academic-retention` plans over a
`RetentionSubject` — a vault object or a span inside one — and settles with a
shredded key slot and a tombstone a restore re-applies. A plan scenario is
canonical record state with no vault object, and deleting one writes no
tombstone. The two paths share no type and no function, so there is nothing here
for a rotation or a restore to reach.

## What is deliberately undecided

Section 10 fixes the repeat **ceiling** and says the eligibility rule, the
경과조치 for old courses, and the 동일·대체 mapping are "별도 versioned policy로
관리하고 최신 원문을 확인한다". It does not say which attempt of a repeat group
is the recognized one. So:

- `PolicyBook::published_v1` carries `RepeatRecognition::Unknown`, and an engine
  that meets a repeat group under it reports `UNKNOWN` naming the exact attempts
  rather than choosing. The fixtures use a **synthetic user-confirmed** book,
  labelled as such in `corpus.rs`, so a definite average exists to check.
- External credits carry `RecognitionDecision::Undecided` until the user records
  one. Undecided is not a synonym for "no": the credits are *known to be
  undecided* and the total that would contain them is withheld.
- The published book has no row before 2015 spring at all, so a repeat that early
  resolves `UNKNOWN`. "Nothing is stated about that period" and "no ceiling
  applied in that period" are different claims and only the first is true here.

`GATE-38-005`, `GATE-38-006` and `GATE-38-016` are open, and this crate keeps
them open.

## Named acceptance evidence

| row | where |
|---|---|
| `attempt_history_append_only` | `crates/record/tests/record.rs` |
| `attempt_grade_repeat_contract` | same |
| `classification_by_ruleset` | same |
| `gpa_formula_fixture` | same, against `testdata/engines/gpa/oracle.expected` |
| `gpa_policy_version_matrix` | same |
| `snu_grade_mapping_gpa` | same |
| `external_credit_vs_gpa` | same |
| `cumulative_gpa_proof` | same |
| `term_gpa_partition` | same |
| `major_gpa_classification` | same |
| `multi_major_gpa` | same |
| `credits_vs_denominator` | same |
| `repeat_proof_view` | same |
| `special_attempt_reason_matrix` | same |
| `repeat_ceiling_effective_date` | same |
| `registered_attempt_gate` | same |
| `delete_plan_preserves_attempts` | same |

All seventeen run in the default lane, on every platform, inside
`cargo test --workspace`.

Beside them:

- `crates/record/tests/record_harness.rs` executes every committed harness
  fixture against the real engines, byte-compares, requires each adverse fixture
  to land on the outcome its directory names, and re-renders the whole corpus
  from the deterministic builder.
- `crates/record/tests/record_scans.rs` holds the two source scans below.

## Structural guards

`crates/record/tests/record_scans.rs`:

- **`no_float_reaches_the_gpa_path`** — a recursive walk of `crates/record/src`
  with a file-count floor and a `pub mod` tripwire, refusing floating point under
  three spellings: the type (`f32`, `f64`, qualified or not), a decimal-point
  literal, and an exponent literal. The last two name neither token and are how a
  float actually arrives in Rust — `let ratio = 33.9 / 12.0;` is `f64` by
  inference. Comments and string literals are stripped first, so this document's
  own `2.825` does not trip it and does not have to be avoided. The check is run
  against five evasions inside the test and each must be caught.
- **`the_published_average_is_rounded_in_one_pinned_place`** — `WHOLE_DIVISION`
  is a whole-text pin on `div_round_half_up`, comment lines dropped and
  whitespace collapsed, in the shape `rotation_gate.rs`'s `WHOLE_GATE` uses. It
  also asserts the crate has exactly one rounding site, that the published scale
  is still an argument, and that the arithmetic module declares no type.

`tools/phase1-scaffold-policy.test.mjs`:

- `engine_source_contains_no_clock_rng_network_or_model` scans every
  `crates/record/src` file for clock, RNG, socket, and model APIs, and pins the
  crate's product closure with `getrandom` reaching it through `uuid` alone.
- `dependency_license_and_source_receipt_is_complete` subtracts
  `docs/security/dependency-admission-phase2-u4.json`, which admits no external
  crate. A decimal or big-rational library would have been the obvious way to
  build an average and would have arrived as an unreceipted package.
