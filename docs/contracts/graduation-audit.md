# The graduation audit engine and its proof tree

`academic-audit` is `P2-U3`. It holds section 11.1's fail-closed `RuleSet`
selector, section 11.3's explainable proof tree, and section 11.4's three-gate
`DETERMINATE` rule. It persists nothing, opens nothing, and runs no connector:
migration `0004`'s `audit` aggregate row is `crates/store`'s.

This is the engine that tells a user whether they can graduate. Every contract
below is fail-closed, and one sentence is behind all of them: **an absent fact
is never a verdict**.

## Current outcome

No real academic record can reach this engine and none has. Admission is closed
(`P2-K6` built a verifier and did not open admission), ADR-002 is unaccepted,
the default lane reports `storage_encryption=NONE` and
`production_data_allowed=false`, and `academic-transcript`'s two gated entry
points refuse on every profile in this repository. Every fixture here is
synthetic: the transcript is `academic_record::corpus`'s own, reused rather than
transcribed, and the official documents are `academic_ingestion`'s own.

`GATE-38-001`–`GATE-38-004`, `GATE-38-006`, `GATE-38-011` and `GATE-38-012` stay
open. See [What is deliberately undecided](#what-is-deliberately-undecided).

## `DETERMINATE` is three values, not three checks

Section 11.4: *고위험 결과(졸업 가능/불가)는 rule coverage 100%, unresolved
conflict 0, source freshness 기준 충족 시에만 `DETERMINATE`가 된다.*

```text
CoverageWitness::establish(leaves, unevaluated) ──┐
ConflictFreeWitness::establish(leaves, cases)  ───┼─▶ DeterminateVerdict::new
FreshnessWitness::establish(policy, at, as_of) ───┘        three by value
```

`DeterminateVerdict` has private fields and one constructor, and that
constructor takes the three witnesses **by value**. Each witness has private
fields, no public constructor, and one `establish` that is crate-private and
returns `Option<Self>` from the evidence its gate is about. A caller holding two
of them has no expression that produces a determination, and a caller outside
this crate cannot produce a witness at all.

That is `P2-R4`'s five-stage-by-value chain applied to three stages, and it is
what lets `determinate_three_gate` vary each condition independently: the
missing witness is not a branch somebody could forget to write, it is an
argument that cannot be supplied. `a_determinate_verdict_has_no_public_constructor`
and `a_determinate_verdict_has_no_struct_literal` are the two compiler
diagnostics.

**The default is not `DETERMINATE`, and it is not reachable.** No source states
a source-freshness number — `t001`'s `REQ-11-029` row records the criterion as
an open gate candidate — so `SourceFreshnessPolicy` has no `Default`, no
constant and no constructor that omits the bound. An audit with no recorded
criterion has no third witness and is `INDETERMINATE`, naming
`SOURCE_FRESHNESS_POLICY_ABSENT`. The fixtures record a **synthetic,
user-confirmed** criterion, labelled as such, so a determinate case exists to
check.

`the_three_witnesses_have_one_construction_site_each` holds the rest: three
witness declarations, three crate-private `establish` sites, no `pub fn
establish` anywhere, the constructor pinned to take all three by name, and
exactly one `DegreeVerdict::Determinate` expression in the engine.

## `INDETERMINATE` always says what is outstanding

`IndeterminateVerdict::new` takes its first `MissingCheck` as a **parameter**,
so an indeterminate verdict with an empty list is not a call that can be
written. Every arm of `MissingCheck` names the exact field, rule, attempt or
source that is outstanding, and `MissingCheck::action` says what settles it.
There is no arm that reports only that something is missing: section 11.1 asks
for *필요한 확인 항목*, and a list with an unspecific entry in it would satisfy
the letter and lose the point.

The selector reports **every** unrecorded profile field rather than the first,
because a user who fixes one gap and meets the next has been told the truth
twice instead of once.

## Section 11.1's eight selector inputs

*selector는 대학·단과대·학부·입학년도·사용자가 적법하게 선택한 졸업기준·
주전공/복수/부/연합/연계·교환/편입·예외 승인을 함께 사용한다.*

`SelectorDimension` holds each `·`-delimited unit of that sentence, and
`the_selector_dimensions_are_the_specifications_own` splits the sentence out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares the two lists
in both directions, in order. **Nothing asserts how many there are.** Injection
`U3-I1` dropped a dimension *and* the declared array length together, so no
count moved, and the test failed because the document still writes the unit.

`t001`'s `REQ-11-002`, derived from the specification without reference to
`t068`, lists the same units in the same order in English. That independent
agreement is what the comparison rests on.

### What a published set declares a scope for, and what it does not

Section 11.1's yaml gives a `DegreeRequirementSet` four scope fields —
`institutionPath`, `admissionYear`, `selectedGraduationStandardRange` and
`majorMode` — which between them cover the first six of the sentence's eight
inputs. It declares no field for 교환/편입 and none for 예외 승인.

Those two are therefore **required inputs that narrow nothing**: an unrecorded
one is `INDETERMINATE`, and a recorded one removes no candidate. Inventing two
scope fields the specification does not write would have made
`selector_dimension_matrix` look stronger than the document it comes from.
`SelectorDimension::narrows_the_catalogue` is that split, the matrix asserts
both halves of it, and the scan requires the yaml to declare exactly the four
fields the split rests on.

### Nine fields under eight dimensions

The sixth unit, 주전공/복수/부/연합/연계, is two recorded facts: which mode, and
which additional programmes. Section 38.1 asks for them as two lines — `Degree
Mode` and `Additional Major / Minor` — and gives each its own cell, so splitting
them is what lets a missing check name the exact one.

`the_profile_fields_are_the_specifications_own` reads section 3's
`StudentProfile` block and requires every claimed key to be one section 3
writes, no key to be claimed twice, and every key section 3 writes to be either
claimed or on the three-name list of things that select no rule
(`gradingContext`, `interests`, `privacyPolicy`). One field — the exception
approvals — has no section 3 key at all, because it is section 11.1's sentence
that names it, and the test states that difference rather than carrying an
exception for it.

`degreeMode`'s five values are section 11.1's sixth unit's `/`-separated
alternatives. `SINGLE_MAJOR` is the yaml's own identifier and is compared
against it; the other four have no identifier anywhere in the document, so the
`SCREAMING_SNAKE_CASE` spelling is this crate's — derived, not invented, by the
same mechanical rule `academic-requirement` uses for section 11.2's prose rule
types.

## A leaf that cannot cite itself is not a leaf

Section 11.3: *모든 leaf에는 적용 rule ID, source page/paragraph, 사용한
CourseAttempt, equivalency decision이 붙는다.*

All four are `ProofLeaf::new` parameters. The type has private fields, no
`Default`, no setter and no builder, so there is no expression that produces a
leaf with three of the four. The two that could plausibly be empty are not
`Option`s and not bare vectors:

| Part | Type | Why not a `Vec` |
|---|---|---|
| the attempts used | `AttemptUsage` | an empty list reads as "no attempt" and as "nobody said" at once, and only the first is publishable; the other arm states the reason |
| the equivalency decision | `EquivalencyDecision` | *no substitution was used* is a decision, and `NoneApplied` says it |

`a_proof_leaf_has_no_shorter_form` and `a_proof_leaf_has_no_struct_literal` are
the compiler diagnostics. `proof_leaf_completeness` walks the whole tree, checks
each of the four resolves — the rule against the published set, the span against
a real page and a non-empty paragraph, the attempts against the bound
transcript, the equivalencies against the published set — and additionally
requires the tree to contain at least one leaf that named an attempt and at
least one that applied a substitution. Without those two halves the walk would
pass over a tree in which nothing ever did either. Injection `U3-I6` replaced an
undischarged operand's `NoneUsed` with an empty `Used` and was refused.

### Why a rule with no recorded page is not evaluated

`academic_requirement::ExecutableRule` carries the digest of the official
snapshot a rule was read out of and no position inside it. That is the right
boundary there — a rule's meaning does not depend on which page printed it — so
the position is the audit's obligation, and `RuleSourceIndex` is where it is
recorded.

A rule the index does not place **cannot become a leaf**, because
`RuleSourceSpan` is a constructor parameter with no absent arm. Rather than
invent a citation, the engine leaves the rule unevaluated — which is exactly
`EngineResult::is_partial_failure` — reports
`MissingCheck::RuleSourceSpanAbsent`, and refuses the coverage witness. A
verdict without a citation and a verdict withheld are different things, and only
the second is publishable. That is also what makes `adverse/partial_failure` an
input file rather than a differently configured engine.

## The mixed tree, and where section 11.3 and the harness disagree

`ProofStatus` is the deterministic engine harness's fixed five-value set:
`SATISFIED`, `NEEDS`, `NOT_SATISFIED`, `UNKNOWN`, `CONFLICT`. Section 11.3's own
tree prints five leaf tokens and **they are not the same five**:

| Section 11.3 prints | Harness value | Note |
|---|---|---|
| `PASS` | `SATISFIED` | a spelling difference |
| `PASS_PARTIAL` | `NEEDS` | see below |
| `NEEDS` | `NEEDS` | section 11.3 writes the quantity beside it: *NEEDS 12* |
| `NOT_SATISFIED` | `NOT_SATISFIED` | |
| `UNKNOWN` | `UNKNOWN` | |
| — | `CONFLICT` | section 11.3's example prints none |
| `INDETERMINATE` | — | the root's verdict, not a leaf status |

**Section 11.3 labels two structurally identical credit rows differently.**
*Total credits: 93 / 130* reads `PASS_PARTIAL` and *CSE major total: 51 / 63*
reads `NEEDS 12`; both are a floor short of its threshold with the shortfall
quantified. They are one reading, the harness spells it `NEEDS`, and `Measure`
carries the quantity — so both render as `NEEDS` here.

`CONFLICT` is required anyway: section 11.4's *unresolved conflict 0* and
section 8.4's *dangerous determination stays INDETERMINATE* both presuppose one.
`the_proof_statuses_cover_section_11_3s_own_tree` parses section 11.3's block
out of the document, requires every token it prints to be in the mapping above
and every mapped token to be printed, and asserts both of the two directions the
tables disagree in — so a specification edit that introduces a sixth reading
fails here rather than being folded into the nearest status.

The two-credit-row reading is executed too, since it is what the `PASS_PARTIAL`
row of the mapping rests on. The test finds every `X / Y` line in the tree,
requires there to be exactly two, requires each to be short of its threshold,
requires their two labels to **differ**, and requires the mapping to send both to
`NEEDS`. If the document ever makes the two rows agree, or adds a third, the
justification above is stale rather than wrong and fails here instead of being
carried forward unread. Three injections, each its own build: relabelling the
first row `NEEDS 37`, making it `130 / 130`, and adding a third credit row. All
three fail.

`mixed_proof_tree` produces every one of the harness's five statuses in one
tree, over one transcript and one published set.

### The root's fold

`CONFLICT` if any leaf conflicts, else `UNKNOWN` if any is unknown, else
`NOT_SATISFIED`, else `NEEDS`, else `SATISFIED`. There is no arm in which an
`UNKNOWN` child yields a `SATISFIED` or a `NOT_SATISFIED` root, which is §28's
*unknown을 pass/fail로 강제하지 않음* as a total function rather than as a
comment. The harness imposes no fold — §11.2 has rule types where a parent is
legitimately satisfied over unsatisfied children — and
[the engine harness](engine-harness.md) says that invariant is this task's to
express.

The root status is **not** the verdict. A tree can read `NOT_SATISFIED` and the
audit still be `DETERMINATE`: that is 졸업 불가 with the exact shortfalls, which
is what `golden/baseline` is.

## Nothing derived is published under `CONFLICT`

`EngineResult::values` is empty when the root conflicts. That is the rule
`academic-record` settled one engine over: a reader who sees a number beside a
`CONFLICT` has been handed a figure computed over a record that disagrees with
itself, and the number is what survives into a screenshot.

What is published otherwise is a count per status and the number of outstanding
checks — a description of the tree the reader is already looking at. There is no
aggregate remaining-credit total, because a total over rules with different
categories would be a number no rule states.

## Opening a credit number

Section 11.3: *사용자는 숫자뿐 아니라 "왜 이 학점이 포함/제외되었는가"를 열 수
있다.*

`CreditExplanation` is **total over the transcript**, not over the attempts that
counted. A user asking why a number is lower than expected is asking about the
attempts that are *not* in it, and a list of inclusions does not contain them.
`credit_explanation_drilldown` asserts the partition rather than the sum: one
line per transcript entry, in transcript order, none missing and none repeated,
and the included lines adding up to the rule's own numerator. Injection `U3-I12`
listed only the inclusions and was refused.

Every exclusion names the record engine's own `DispositionReason`. This crate
adds exactly **one** reason of its own — the attempt earned credit and this
rule's category is not one it counts under — because that is the one decision
the requirement rule makes and the record engine does not.

## Whether a credit counts is `P2-U4`'s decision

`RecordViews` publishes one `AttemptDisposition` per attempt with a
`CreditContribution` of `Earned`, `NotEarned` or `Unknown`. `EntryAdmission` is
a total function of that three-arm enum:

| `CreditContribution` | admission |
|---|---|
| `Earned(credits)` | `Counted` with those credits |
| `NotEarned` | `Excluded`, carrying the record's reason |
| `Unknown` | `Pending`, carrying the record's reason |

Totality is by arithmetic over that enum rather than by a list of cases somebody
thought of, and the reason a reader sees is the record engine's own word rather
than a second vocabulary that could disagree with it.

An attempt whose contribution is `Unknown` — an undecided external recognition
(`GATE-38-006`), a repeat group whose recognition rule no confirmed source
states, or an external term no dated policy row reaches — is **known to be
undecided**. It counts toward nothing, it is not silently dropped, and it is
reported as an exact missing check that blocks `DETERMINATE`.

The audit computes no average either. `TranscriptSnapshot::reading` hands a
`GPA_MINIMUM` rule the reading `P2-U4` published, summed from the exact
contributions its dispositions carry. Nothing here rounds: the rule compares by
cross-multiplication, so no ratio is formed, and the only divisions in this
crate reduce an exact decimal to whole units and check the remainder is zero
first — a fractional credit is a typed refusal, not a rounded one. The
workspace's one rounding site stays
`academic_record::decimal::div_round_half_up`.

## Planned work never satisfies anything, in four layers

1. **`P2-C7`.** `academic_scenario::Proposed<T>` has no exit, and this crate has
   no `academic-scenario` product edge, so a projection is not nameable from a
   product file here. `a_projected_value_cannot_enter_an_audit` is the compiler
   diagnostic and
   `no_product_file_names_a_projection_and_only_one_names_a_plan` is the sweep.
2. **`P2-U4`.** `PlanScenarioChoice` has no route to a `CourseAttempt` and
   `AttemptStatus::Planned` has no producer.
3. **`P2-U4` again.** Its credit engine reports a not-settled attempt as
   `NotEarned`, so a plan contributes no credit even if one reached the ledger.
   The corpus's `REGISTERED` attempt is exactly that case and
   `plan_excluded_from_actual_audit` asserts it.
4. **This crate.** `DegreeAudit::evaluate` has **no plan parameter**, and the
   plan is not a frozen input: section 6 binds an audit to a profile, a
   requirement set and a transcript snapshot, and putting a proposal in the
   digest would make an audit's identity move when a proposal did.

`PlanAnnotatedView` borrows a finished audit and a plan and produces
`PlanNote`s. It has no method returning a `DegreeAudit`, no `&mut` borrow of
one, and no method returning a `ProofStatus`. Section 11.3's *Algorithms:
planned only NOT_SATISFIED* is that view's rendering over a leaf whose status the
plan did not touch.

`plan_excluded_from_actual_audit` runs the same audit twice, requires the whole
`EngineOutcome` and the whole binding to be equal, and **then** requires the
annotation to have found the planned course. Without that second half the first
would pass on an annotation that found nothing; injection `U3-I8` inverted the
note and was refused. A course that is both planned and already earned is not
planned-only, which the same test asserts.

## What an audit is bound to

Section 6: a `DegreeAuditAggregate` is a reproducible proof tree over a
`StudentProfile`, a `RequirementSet` and a transcript snapshot.
`AuditInputBinding` carries one digest per input plus the rule-set hash and the
frozen-input digest, so `degree_audit_input_binding` can move each live source
in turn and observe exactly its own digest move while the others hold.

**Everything the engine reads is a frozen input, and that is load-bearing.** The
source placements, the open conflict cases and the freshness criterion are
inputs rather than engine configuration, because an engine that read a value the
digest does not cover would not be the pure function of `(frozen_inputs,
rule_set_hash, engine_version)` the harness fixes — two evaluations could then
agree on the declared signature and disagree on the answer. The published rule
set is the exception, and it is covered by `rule_set_hash`, which is the other
half of the signature and the half a historical replay walks.

`historic_audit_replay` records an audit, publishes a second version beside the
first, looks the recorded hash up in the ledger, re-runs, and requires the
canonical bytes to be identical. The second half is that the latest audit under
the new version reaches a *different conclusion* on the same transcript —
without it the first would pass on two audits that merely differ.

## A source conflict is `P2-U6`'s finding and this engine's refusal

Section 8.4 compares five dimensions and opens a `ConflictCase`; deciding
whether two documents really disagree is that crate's work and is not redone
here. `ConflictReference::of` reads the case's rule, both connectors and its
`AuditDisposition`, and the engine refuses the conflict-free witness while any
applicable case is unresolved, reporting the rule and both connectors as the
actionable reference.

`graduation_conflict_fail_closed` blocks a determination with a real detected
case, checks the reference names the rule and two different connectors, then
**resolves the case and requires the determination through** — which is what
says the refusal was the conflict rather than anything else. Injection `U3-I10`
made the witness ignore an unresolved case and was refused.

A rule that itself concludes `CONFLICT` blocks a determination too, publishes no
derived value, and is `adverse/conflict`.

## The harness corpus, and the oracle that is not this engine

`GRADUATION_AUDIT` flips to `IMPLEMENTED` with this task, so its directory under
`testdata/engines/graduation_audit/` carries golden fixtures, property bounds,
version-compat fixtures, an explanation snapshot, and — because it is one of the
four high-impact paths — all three adverse sets. Each adverse state is reached
under the **baseline** rule set, from the inputs:

| path | how |
|---|---|
| `unknown` | the exchange attempt in 2003, which no dated policy row reaches, so `P2-U4` withholds the average and the `GPA_MINIMUM` rule reads `UNKNOWN` |
| `conflict` | two settled attempts at one course in one term, which breaks the `MUTUALLY_EXCLUSIVE` ceiling |
| `partial_failure` | a published rule the source index does not place |

`crates/audit/tests/audit_harness.rs` is the executing half the audit in
`academic-domain` cannot be: it runs every committed fixture against the real
engine, byte-compares, requires each adverse fixture to land on the outcome its
directory names, re-renders the whole corpus from the deterministic builder, and
requires the directory to hold nothing the builder does not render.

**The corpus carries both sides of the three-gate rule.**
`the_corpus_shows_both_sides_of_the_three_gate_rule` requires
`golden/baseline` to be determinate with an empty outstanding list and
`golden/no_freshness_criterion` to be indeterminate with a non-empty one. A
corpus in which nothing is ever determinate would make the adverse directories
index a distinction the golden set no longer makes.

### The oracle

A proof tree checked against a tree the same engine produced proves only that
the engine is deterministic, and a proof tree is large enough that comparing two
of them *looks* like thorough evidence. So the expected statuses and measures
come from `tools/graduation-audit-oracle.mjs`: a second transcription of the
transcript, the grade table, the repeat ceiling and the rules, in another
language, with fixed-point `BigInt` units and no rescaling step.

Four things are independent of the Rust side: the transcript rows, the credit
admission, the numeric representation, and the rule evaluation. The row that
separates implementations is the 2015 repeat ceiling — without it the repeated
`A+` contributes `4.3` and the weighted total is `34.8` rather than `33.9` — and
`graduation_audit_oracle_is_committed_and_re_derivable` asserts the oracle
itself applied it. Earned credits and the grade-point denominator differ on this
corpus, `14` against `12`, and both sides say so separately.

Injections `U3-I9` (a category removed from one course) and `U3-I13` (a
threshold changed) each moved the Rust side and not the oracle, and each was
refused.

## What is deliberately undecided

`t068` section 5's `P2-U3` entry leaves seven section 38 cells open. None has a
default, no cohort is assumed from a term, an attempt or a published set, and
each appears as its own exact missing check.

**`GATE-38-001`** — the admission year. Without it no requirement set is
selected, the audit is `INDETERMINATE`, and there is no personal figure of any
kind.

**`GATE-38-002`** — the graduation standard the user lawfully selected. It is a
different fact from the admission year and is not read off it: section 11.1's
sentence names both, and section 11.1's yaml scopes on both.

**`GATE-38-003`** — the degree mode. Single major is not assumed.

**`GATE-38-004`** — any additional major or minor. An unanswered question is not
an empty list, so the audit stays `INDETERMINATE` until the user says which,
including that there are none.

**`GATE-38-006`** — the transferred and exchange credits and the recognition
decision on each. An attempt whose recognition is undecided is *known* to be
undecided and is never counted as recognized or as refused.

**`GATE-38-011`** and **`GATE-38-012`** are `academic-requirement`'s and are
forwarded rather than restated: a rule's cohort applicability and the 2027-1
thesis-research scope are official facts a rule verdict already carries.
`thesis_determinate_gate` uses a thesis rule pointed at a course the transcript
holds a **completed, credited** attempt at, and the verdict is still `UNKNOWN` —
which is the claim the cell makes, rather than the weaker one that an absent
attempt reads unknown.

`GATE-38-015` and `GATE-38-016` stay `academic-requirement`'s entirely. A
`MUTUALLY_EXCLUSIVE` or `MAXIMUM_RECOGNITION` verdict that reads `UNKNOWN` is
still an `UNKNOWN` leaf here and still blocks `DETERMINATE`, but this crate does
not restate what they leave open: `OpenGate::from_rule_gate` maps them to
`None`, and the leaf carries the rule crate's own value through
`ProofLeaf::rule_gate`.

### Where the specification's example and its selector rule meet

Section 11.3's tree prints *General education area: cannot evaluate └─
admissionYear missing UNKNOWN* under a root that reads `INDETERMINATE`, while
section 11.1 requires the admission year for selection. Read together those
would be an audit that ran without an input its own selector demands.

The two are about different facts and both hold. Section 11.1's admission year
is the year the **user** entered under; `GATE-38-011` is which cohort an
**official rule** applies to and what the transitional arrangement between two
standards is. Section 38.2's first bullet asks for the second, and
`academic-requirement` already separates them. So an audit exists only when the
user's year is recorded, and a rule inside it can still read `UNKNOWN` because
nobody has confirmed who that rule was written for. `mixed_proof_tree` is that
tree.

### The public example, and why it holds no personal number

Section 11.4 closes: *현재 사용자의 입학년도가 없으므로 이 문서는 130학점 등
공개된 공통 사실을 예시로 사용할 뿐, 개인의 "남은 학점"을 산출하지 않는다.*

`CommonRuleExamples` is that sentence. It reads the published credit floors out
of a `RuleSet` — not a `SelectedRuleSet`, because a public common fact is
readable whether or not a set was selected for this user — takes no transcript,
and carries the threshold and nothing else. There is no attained figure, no
remaining figure and no accessor for either, so a screen that rendered one
cannot have got it from here.
`a_common_rule_example_has_no_remaining_credits` is the compiler diagnostic, and
`missing_admission_no_remaining` additionally requires no outstanding check to
quote a threshold as though it were a personal figure.

## What this contract does not claim

- **It does not claim no code anywhere can produce a determination.** What is
  executed is narrower and is a composite: the verdict and all three witnesses
  have private fields, the witnesses' `establish` and the verdict's constructor
  are crate-private and counted at one site each, the constructor is pinned to
  take all three by name, and the engine holds exactly one
  `DegreeVerdict::Determinate` expression. A module that reimplements the
  semantics from scratch is refused by nothing here — it also reaches none of
  this crate's values.

- **It does not claim the source scan catches every clock.** The scan matches
  API spellings, and a clock reached through a spelling nobody listed is not
  caught by it. What carries the claim is the closure comparison — no clock
  crate is in the product graph at all — plus the frozen-input signature, in
  which the instant an audit is anchored to is an argument.

- **It does not claim a rule's page number is correct.** `RuleSourceIndex` is
  supplied by the caller and this crate verifies that a span exists, is
  well-formed, and travels with every leaf. Whether page seven really prints
  that rule is a fact about the official document, and no code here can check
  it.

- **The curriculum facts are supplied, not inferred.** Which durable `Course` a
  transcript code names is `P2-U1`'s identity decision, which categories and
  area a revision places it in is `P2-U1`'s catalogue, and what language an
  offering was taught in is the offering's evidence. `GATE-38-013` stays open
  there: an unconfirmed revision holds `CurriculumCategory::Unknown` and there
  is no conversion from that value to a `CreditCategory` anywhere, so an
  unconfirmed category cannot arrive here as a confirmed one. What this crate
  refuses is a transcript the index does not cover at all.

- **`product_network` remains `NONE` and `production_data_allowed` remains
  `false`.** Nothing in this task moves either.

- **No migration is added.** Migration `0004` already creates the `audit`
  aggregate row for the `AUDIT_COMPUTED` arm, with its guard triggers and its
  `requirement_set_id`, `scope_id` and `source_digest` columns, and this task
  adds no typed attribute to it: an audit's inputs are bound by hash and its
  tree is rendered from those inputs, so there is no per-audit column this crate
  needs that `0004` does not already carry. `0024` stays unclaimed. Nothing here
  writes to a store in any case — `academic-store` is in no edge of this crate.

## Named acceptance evidence

| Test | Where | Proves |
|---|---|---|
| `selector_dimension_matrix` | `tests/audit.rs` | each of the six scope dimensions varied alone selects the matching set and no other; the two that narrow nothing are still required |
| `selector_fail_closed` | same | every omitted field named with its section 38 cell and its action; two competing sets both named and neither chosen; nothing covering reported as such |
| `mixed_proof_tree` | same | all five harness statuses in one tree; `CONFLICT` root publishes no value; the unknown leaf names its cell; the planned course reads `NOT_SATISFIED` |
| `proof_leaf_completeness` | same | every node below the root carries all four parts and each resolves; at least one leaf names an attempt and one applies a substitution; an unplaced rule produces no leaf |
| `credit_explanation_drilldown` | same | one line per transcript entry per credit rule, adding up to the rule's own numerator, with both sides of the partition present |
| `unknown_profile_audit` | same | section 3's unrecorded profile selects nothing and reports every field, each with its cell |
| `missing_admission_no_remaining` | same, and `tests/compile_fail/` | no audit without the year; the public floor is readable and carries no personal number, which is a type fact |
| `plan_excluded_from_actual_audit` | same, and `tests/compile_fail/` | the audit function has no plan parameter; the outcome and the binding are unchanged; the annotation finds the planned course anyway |
| `historic_audit_replay` | same | the recorded audit replays byte-identically under its own rule hash after a second version is published, and the latest audit concludes differently |
| `determinate_three_gate` | same, and `tests/compile_fail/` | all three gates determinate; each falsified alone is indeterminate and names why; an `UNKNOWN` leaf defeats coverage |
| `graduation_conflict_fail_closed` | same | an unresolved case blocks and names the rule and both connectors; resolving it lets the determination through; a `CONFLICT` leaf blocks too |
| `degree_audit_input_binding` | same | each of the four bound inputs moved alone moves its own digest and no other |
| `thesis_determinate_gate` | same | a completed, credited thesis attempt still reads `UNKNOWN` under an unresolved scope; `GATE-38-015`/`016` stay the rule crate's |

Beside them:

- `crates/audit/tests/audit_harness.rs` executes every committed harness
  fixture against the real engine, byte-compares, requires each adverse fixture
  to land on the outcome its directory names, re-renders the whole corpus from
  the deterministic builder, compares the baseline tree against the independent
  oracle, and requires both sides of the three-gate rule to be present.
- `crates/audit/tests/compile_fail/` holds the eight absences.
- `crates/audit/tests/audit_scans.rs` holds the nine source scans, enumerated in
  [policy source scans](policy-source-scans.md) with the thirteen injections
  each was measured against.

## Enforcement

- `cargo test -p academic-audit` — the named acceptance evidence, the harness,
  the scans and the compile-fail suite, on every supported platform, inside
  `cargo test --workspace`.
- `cargo run -p academic-audit --example emit_harness` — the deterministic
  corpus builder.
- `node --test tools/graduation-audit-oracle.test.mjs` — the oracle is committed
  and re-derivable; runs inside `pnpm test`.
- `pnpm test` — `engine_source_contains_no_clock_rng_network_or_model` covers
  this crate's sources and pins its product closure whole.
- `pnpm verify:contracts` — the registry render and the §28 cross-check.
