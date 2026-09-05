# Academic dashboard, semester planner, course detail, audit view

`academic-dashboard` is `P2-X3`. It fills the four `Academic` routes of the
section 25.1 tree — `Dashboard`, `Semester Planner`,
`Course Catalog & Course Detail` and `Graduation Audit` — plus the `Academic`
index above them. `packages/ui`'s `academic.ts` is the shell side that says
which sections each of those routes shows and in what order.

It persists nothing. It claims no migration number, and the section below says
why with the evidence.

## What this is not evidence for

**No window opens.** `P2-X1` merged with no Tauri runtime linked, and that
decision is still open under the user gate. Nothing here depends on a window,
and nothing here is evidence that one exists: the crate is a set of typed
records and the rules between them, checked by compiling it, running its tests,
or reading its source. The shell half adds that opening `/academic/dashboard`
yields sections naming section 25.4's own six lines instead of a promise — that
is a structure, not a rendering, and `P2-X1`'s, `P2-X2`'s, `P2-X5`'s and
`P2-X7`'s pages say the same thing about their own.

**No average is computed here.** `GpaFigure::publish` takes a
`academic_record::views::GpaValue` and the attempts behind it. This crate has no
grading scheme, no repeat policy and no arithmetic over grades.
`dashboard_shows_three_gpas_with_proof` drives `P2-U4`'s own engine over `P2-U4`'s
own synthetic corpus and compares what this surface publishes against what that
engine returned, so the oracle is not the code under test.

**No verdict is computed here either.** `AuditStateReading::of` takes a section
3.9 `ProofStatus` a caller supplies. There is no `academic-audit` and no
`academic-requirement` **product** edge, so no rule, no requirement set and no
proof tree is nameable from a product file. This surface displays a verdict
`P2-U3` computed.

**The absence claims are about this crate's declared surface.** They are
whole-set statements over the items this crate compiles and over the items the
workspace compiles, not proofs that no such path could ever be written.

**Every number is over a synthetic corpus.** Admission is closed, ADR-002 is
unaccepted, and `academic-transcript`'s gated entry points refuse on every
profile in this repository. No real academic record can reach these surfaces,
and none has.

## Section 25.4's four audit states are not the engine's five

This is the discrepancy this task recorded rather than resolved, and it is the
**seventh** planning-versus-specification mismatch measured in this run.

`academic_domain::engines::ProofStatus` is the section 3.9 proof-tree node
status and has **five** arms: `SATISFIED`, `NEEDS`, `NOT_SATISFIED`, `UNKNOWN`,
`CONFLICT`. `P2-U3`'s engine publishes those five, and section 11.3's own
rendered tree shows `NEEDS 12` on one line and `NOT_SATISFIED` on another.

Section 25.4 names **four**: `SATISFIED`, `REMAINING`, `UNKNOWN`, `CONFLICT`.

Three of the four are spelled identically in both. `REMAINING` is therefore the
word both `NEEDS` and `NOT_SATISFIED` are shown as, and that is a **reading**
rather than an invention: the dashboard has to show some state for every rule the
audit evaluated, section 25.4 offers exactly these four words, and the other
three are fixed by their own spelling, so nothing else is available to either of
the remaining statuses.

**What the collapse costs.** `NEEDS` is a quantified shortfall an admitted path
closes; `NOT_SATISFIED` is a rule no admitted path closes. A reader of the word
`REMAINING` alone cannot tell *twelve more credits* from *this course was only
ever planned*, and section 11.3 puts both on one screen.

**What pays it back.** The collapse never happens at the value level.
`AuditStateReading::of` takes the engine's status and derives the word;
`engine_status()` is always available; there is no constructor taking an
`AuditState`, so a reading whose word is not the image of its status is
unrepresentable, and `an_audit_state_is_not_built_from_a_word.rs` is the
compiled half. `audit_states_are_exactly_four` requires the mapping to be total
over `ProofStatus::ALL`, requires exactly one word to be the image of more than
one status, requires that word to be `REMAINING` and those statuses to be
`NEEDS` and `NOT_SATISFIED`, and requires two readings that show one word to be
different values.

**Nothing was invented and nothing was dropped.** The specification's four are
the display vocabulary; the engine's five are the verdict vocabulary; the
mapping between them is written down here and executed there.

## The six enumerations, and where each order comes from

No count is asserted anywhere. Every enumeration is parsed out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compared with this
crate's in both directions **and** in order, so a paraphrase fails, a reordering
fails, and an added line fails as a missing key.

| enumeration | the document's own text | how it is parsed |
|---|---|---|
| `GpaScope::ALL` | `누적·학기·전공 GPA와 각 계산 proof.` | split on the line's own middle dot |
| `DashboardLine::ALL` | section 25.4's six bullets | the bullet run, in order |
| `AuditState::ALL` | ``졸업 audit의 `SATISFIED`, `REMAINING`, `UNKNOWN`, `CONFLICT`.`` | the back-quoted runs, then the line's residue must be punctuation |
| `LifecycleFacet::ALL` | `수강 시도 timeline: 예정/수강/취소/재수강/S-U/인정.` | split on the line's own slashes |
| `PlannerDimension::ALL` | section 25.5's bullets after `다음을 즉시 재평가한다` | the bullet run, in order |
| `CourseSection::ALL` | section 25.6's fenced block headings | the fenced block's own heading lines |
| `CoverageTab::ALL` | `DESIGNED / TAUGHT / PRACTICED / ASSESSED (겹치지 않는 탭)` | split on the line's own slashes, before the parenthesis |
| `OpenGate::ALL` | section 38.1's first six lines and section 38.2's seventh bullet | position in each list, and the identifier derived from it |

The audit-state parse is the strictest of them: after the four back-quoted words
are removed, what is left of the line must be punctuation. A fifth word in the
document leaves text behind and fails rather than being folded into the nearest
arm. `P2-X2`'s `permission_status_is_exactly_four_values` is the same shape one
surface over.

## Three averages, each with the attempts behind it

`GpaFigure::publish` takes the proof **by value** and is the only producer.
There is no `Default`, no setter, no `&mut` accessor and no constructor taking
only a value, so there is no state in which a figure exists and its proof does
not. `a_gpa_figure_has_no_route_without_its_proof.rs` is the compiled half.

Two refusals are driven rather than described:

- a `Known` average with no attempt behind it is an average over nothing;
- an `Unknown` average whose proof does not name every attempt the value itself
  names is a surface that says *unknown* and cannot say *because of which
  attempt*. `academic_record::views::GpaValue::Unknown` carries those
  identifiers and this refusal is what stops them being lost on the way here.

A `NoGradedAttempts` value is the one case where an empty inclusion list is the
honest answer, and it is admitted for exactly that value.

**The three proofs are three proofs.** The test requires the term proof and the
major proof to be proper subsets of the cumulative one and to differ from each
other. Three figures carrying one proof would satisfy "each has a proof" and say
nothing.

## No composite

Section 10's last paragraph: *Academic Dashboard에서 GPA chart와 Knowledge Map을
같은 카드의 한 score로 합치지 않는다.* Section 36.9 closes with *한 학기의 결과는
"Database 83%"가 아니라*, and section 35's anti-goal table forbids the 단순
GPA/졸업 계산기 from the other end.

Three things hold it, and `dashboard_no_composite` measures all three.

- **There is no second half to add.** No `academic-knowledge-state` edge and no
  `academic-freshness` edge, so no mastery level, no knowledge state and no
  concept reading is nameable from a product file.
  `the_dashboard_surface_cannot_name_a_mastery` measures that over the whole set
  of capitalized identifiers of every product file, with a control that requires
  the same reader to find `MasteryLevel` and `FreshnessBand` in
  `academic-domain`'s own `lib.rs` — which *is* a product edge, so those two are
  one `use` away and are not written. `P2-X2` holds the same line by the same
  means.
- **No section holds two lines' values.** `DashboardSection` carries the line it
  answers for and that line's values only. The section-to-line map is checked to
  be injective and total over `DashboardLine::ALL`, so a card showing a grade
  average beside a knowledge score would have to be a seventh arm.
- **Nothing folds the three averages.** `every_item_that_reaches_a_closed_type_is_pinned`
  keys a whole item set on `GpaFigure`, so an item anywhere in the workspace that
  reaches one is pinned and a new route fails by name.

**What this is not.** It is not a claim that no number this screen shows was
computed from a grade average: a caller who put one into a
`CreditsByCategory` entry would pass every check here.

## A percentage is secondary and never travels alone

Section 25.4: *"졸업 72%"는 보조 시각화일 수 있으나 서로 대체 불가능한
requirement를 한 막대로 오해시키지 않도록 상세 breakdown이 항상 붙는다.*

`SecondaryPercentage::over` is the **only** producer and takes a
`RequirementBreakdown` **by value**. There is no `Default`, no `From<u32>`, no
`new(permille)`, no setter and no `&mut` accessor, so a percentage with no
breakdown is unrepresentable rather than discouraged; and the number is
*computed from* the breakdown rather than supplied beside it, so it cannot
disagree with the parts it claims to summarise. `P2-Y3` fixed the same shape
with one producer taking four disclosures by value, and `P2-N6` with a result
that does not exist without five public groups.
`a_percentage_is_not_built_from_a_number.rs` is the compiled half, and the
whole-set half — that no *other* item in the workspace produces one — is
`every_item_that_reaches_a_closed_type_is_pinned` keyed on the type rather than
on a spelling.

The breakdown refuses four things, each driven:

| refusal | why |
|---|---|
| an empty breakdown | *항상 붙는다* is the whole point |
| one requirement twice | the merge the sentence warns about, arriving inside the breakdown that was supposed to prevent it |
| a part reading `UNKNOWN` or `CONFLICT` | a ratio over either invents a denominator. `academic_record::views::GpaValue` refuses the same fold one surface over by answering `Unknown(attempts)`. The breakdown is still shown; the bar is not drawn |
| a requirement that asks for nothing, or a part that counts more than it asks for | no ratio, and a bar past its own end |

**Secondary is a position.** The percentage is not one of the six sections; it
is reached through `AcademicDashboard::secondary_percentage`, and the breakdown
through the percentage, so there is no path to the number that does not pass the
parts. The shell half is that `DASHBOARD_SECTIONS` holds six entries and none of
them is a percentage.

## The attempt timeline preserves six facets, and reads two sources

Section 25.4: `수강 시도 timeline: 예정/수강/취소/재수강/S-U/인정.`

**`예정` is not an attempt.** `academic_record::attempt::AttemptStatus::Planned`
exists in the schema and **no constructor in `academic-record` produces it** —
section 10 says why: *`PlannedCourse`는 CourseAttempt와도 분리한다.* A timeline
built from the ledger alone would read `예정` absent on every row forever, and
that facet would be a constant no test could fail on.
`attempt_timeline_preserves_six_lifecycle_facets` **builds that ledger-only
timeline and observes it** rather than describing the problem, then reads the
two-source one.

Three properties a timeline of six constants would fail:

1. **Every facet varies** — each reads `Present` on at least one row and
   not-`Present` on another.
2. **They are independent** — no two facets read the same on every row, checked
   over all fifteen pairs.
3. **A repeat preserves what came before it** — appending a repeat leaves the
   earlier row's six readings identical and the timeline longer, which is
   section 10's *재수강과 취소를 덮어쓰지 않고 매 시도를 보존한다*.

**Three readings, not two.** A facet whose input the record does not carry reads
`Unknown` rather than `Absent`. An ungraded attempt read as *not S/U*, or an
undecided recognition read as *not recognized*, would be this surface answering
a question the record has not. Section 30's own line is *`UNKNOWN`: 필요한 정보가
없음. 낮은 confidence의 동의어가 아님.*

**What the timeline shows and what the ledger keeps.** It reads
`AttemptHistory::current`, so a **correction** — which supersedes — is in the
ledger and not on the timeline, and `attempt_history_append_only` in
`academic-record` is where that is checked. A **repeat** supersedes nothing and
both entries are current.

## The planner re-reads six axes on every drag

Section 25.5 lists six things a drag re-evaluates, and they are
`PlannerDimension::ALL`.

`PlannerBoard::place` and `PlannerBoard::remove` each return a **new** board and
a `DragOutcome` computed from the whole placed set. There is no cache, no
incremental update and no `&mut self`, so a reading cannot survive the placement
that should have changed it.

Four properties, and the last three are what a board returning six constants
would fail:

1. every drag answers on all six;
2. **every axis moves** — placing a second candidate changes the reading on each
   of the six;
3. **it is re-evaluation and not accumulation** — removing what was placed
   returns the earlier outcome exactly;
4. **a conflict is a property of the pair** — two candidates whose meetings
   overlap produce a conflict entry that neither produces alone, which is the
   one axis whose value is not a union of per-candidate facts.

**There is no score.** A number on an axis would rank one axis's evidence
against another's, and section 25.5 asks for a consequence rather than a
ranking.

## A saved plan is immutable, and staleness is identified rather than applied

Section 25.5: *안 A/B/C를 고정 snapshot으로 저장하고, 공식 정보가 바뀌면 무엇이
stale해졌는지만 표시한다.*

`PlanSnapshot` has private fields, no setter, no `&mut` accessor and no method
taking `&mut self`. `restate` takes `&self` and returns a `StaleMarking` and
nothing else: *무엇이 stale해졌는지만* is the whole of what the sentence
licenses, and no `StaleInput` arm carries a replacement value, so applying the
change is not something a caller could do from what it is given.

`plan_snapshot_is_immutable_and_stale_marked` compares the whole snapshot value
before and after rather than trusting the signature, drives each kind of change
one at a time so a marking that reported everything or nothing fails, and drives
an unchanged reading first as the control.

**`안 A/B/C` is an example and not a closed set.** The label is caller text that
has to be non-empty. There is no three-armed enumeration here, because the
specification names no third thing a fourth scenario would violate.

## No registration endpoint

Section 25.5: *사용자의 실제 수강신청을 자동 수행하지 않는다.*

`P2-M4` already made confirming a registration **non-delegable**:
`RegistrationConfirmation::new` takes no actor, so no agent may be asked to
stand in for the user. That is a different claim from this one. `P2-M4`'s is
about *who* may do it; this one is that the planner has no route to it at all,
delegated or otherwise.

This crate links `academic-record`, so `RegistrationConfirmation`,
`CourseAttempt` and `AttemptHistory` are all nameable from here. **That edge is
what makes the absence a measurement**: an absence over a type a crate cannot
name is not an absence. `planner_has_no_registration_endpoint` states it in four
parts:

1. the edge is declared and every name is really spelled in `academic-record`'s
   own product text, read through the same whole-set reader;
2. **no product file here spells `RegistrationConfirmation` or `SettledStatus`**
   — the confirmation `CourseAttempt`'s first constructor takes, and the
   argument type its second one takes. Without either, neither constructor is
   callable from here;
3. **the planner surface reads no ledger vocabulary at all** — `planner.rs` and
   `screen.rs` spell none of the six. `timeline.rs` is not on that list because
   section 25.4's fifth line is a reading *of* the ledger;
4. **what `timeline.rs` does read, it reads by borrow** — every occurrence of
   `AttemptHistory` and `CourseAttempt`, over all occurrences rather than over
   the ones an inventory listed, is preceded by an ampersand.

The workspace-wide half is `every_item_that_reaches_a_closed_type_is_pinned`
keyed on `RegistrationConfirmation`: the whole set of items anywhere that reach
it is pinned, so a route added from any crate fails **by name**.

## Section 25.6's four coverage tabs partition the evidence

Each tab is one section 7.2 predicate — `DESIGNED_TO_TEACH`, `TAUGHT_IN`,
`PRACTICED_IN`, `ASSESSED_IN` — read from `academic_domain`'s own
`PredicateName`, and `CoverageTab::of` is the inverse. One entry is on exactly
one tab by construction rather than by a rule somebody remembered to apply, and
an entry whose predicate is not one of the four is **refused** by
`CoverageEntry::of` rather than landing on a default tab.

`coverage_tabs_are_non_overlapping` measures the partition itself:

- the union of the four tabs is the whole report, and every entry is on exactly
  one;
- every pairwise intersection is empty, over all six pairs;
- the same concept under three predicates appears once on each of three tabs and
  not at all on the fourth, so the tabs partition the **evidence** rather than
  the concepts;
- `APPLIED_IN` is driven as the control and is refused.

## A catalogue fact and an offering review are two blocks

Section 25.6: *Course catalog 정보와 특정 Offering review를 같은 속성처럼 보이지
않게 한다.*

`academic-curriculum` and `academic-review` are **both** product edges, so
keeping the two apart is a choice this surface makes rather than something the
compiler made for it.

- **`CatalogIdentity` declares no review type.** Its whole field list is read out
  of this crate's own source and compared, in both directions, with the six
  things section 25.6's `Official identity` line names, and every review
  dimension spelling and every aggregate type name is required to be absent from
  it.
- **`ReviewSection` names no course.** `ReviewScope` has no `CourseId` field, no
  constructor taking one and no accessor returning one — read out of `P2-U8`'s
  own source, the way that crate's `scalar_is_not_a_course_property` reads
  `academic-curriculum`'s `Course`. Section 34's own failure row is *Course와
  Offering 혼동 — catalog row에 교수·학기 속성을 덮어씀*.
- **Nothing here reduces a distribution.** `P2-U8` drew that line with
  `scalar_is_not_a_course_property`: a course reading is a distribution and
  there is no value it reduces to. This crate declares no conversion out of a
  band and no accessor returning one, and the test requires the aggregate a
  review section carries to keep every dimension's distribution and requires two
  reviews a mean would collapse to stay distinguishable.

## Section 38: six cells block, one stays open

`t068`: *Leaves `GATE-38-017` open (per-term offering facts) and surfaces
`GATE-38-001`–`GATE-38-006` as blocking dashboard inputs.*

Every identifier is derived from section 38's own numbering — section 38.1's ten
lines are `GATE-38-001` to `GATE-38-010`, and section 38.2's eleven bullets
continue from `GATE-38-011` — rather than read from a table, so a renumbered
section or a reordered line fails. `academic-audit` and `academic-offering`
derive their own the same way.

| cell | section 38 line | what stands while it is empty |
|---|---|---|
| `GATE-38-001` | `Admission Year` | no requirement set is selected, so no credit category, no audit state and no percentage is shown |
| `GATE-38-002` | `Selected Curriculum/Graduation Standard` | the audit block shows the missing check; the admission year is not read as the standard |
| `GATE-38-003` | `Degree Mode` | the major average has no scope; single major is not assumed |
| `GATE-38-004` | `Additional Major / Minor` | section 10's 다전공별 GPA is one figure per programme, and with no programme named there is no figure rather than a figure over none |
| `GATE-38-005` | `Current Official Transcript` | there is no attempt set, so the three averages, the credit totals and the timeline are empty rather than zero |
| `GATE-38-006` | `Transferred/Exchange Credits` | an undecided recognition reads `UNKNOWN` on the timeline's 인정 facet and keeps the affected average out of the numerator rather than counting it as zero |
| `GATE-38-017` | 해당 학기의 최신 CourseOffering, 교수자, 정원, 시간표, syllabus, 평가 방식 | **stays open every term**; every planner candidate is an official reading the caller supplies and nothing is carried over from a term that has passed |

**`GATE-38-005` is not in `academic_audit::OpenGate`, and that is correct
there.** The graduation engine takes an attempt set as a frozen input and never
reads a transcript, so the import is not a blocking input for it. Every average
on *this* screen is over the imported record, so it is one here. That is why
this enumeration is derived from section 38.1 rather than forwarded from that
crate: forwarding would have carried the hole with it.
`the_open_gates_are_section_38s_own` measures the overlap — five shared cells,
one that is only here — rather than asserting it, and `academic-audit` is a
**dev** edge so the comparison is a comparison.

`an_open_cell_blocks_the_line_it_reaches` drives each of the six and requires it
to block exactly the lines that name it and at least one line, requires
`GATE-38-017` to block none, and requires an empty open set to block none.

## What holds each named test

| `t068` acceptance evidence | where |
|---|---|
| `dashboard_shows_three_gpas_with_proof` | `crates/dashboard/tests/dashboard.rs` |
| `dashboard_no_composite` | `crates/dashboard/tests/dashboard.rs`, with `the_dashboard_surface_cannot_name_a_mastery` in `tests/dashboard_scans.rs` |
| `audit_states_are_exactly_four` | `crates/dashboard/tests/dashboard.rs` |
| `attempt_timeline_preserves_six_lifecycle_facets` | `crates/dashboard/tests/dashboard.rs` |
| `planner_reevaluates_six_dimensions_on_drag` | `crates/dashboard/tests/dashboard.rs` |
| `plan_snapshot_is_immutable_and_stale_marked` | `crates/dashboard/tests/dashboard.rs` |
| `planner_has_no_registration_endpoint` | `crates/dashboard/tests/dashboard_scans.rs` |
| `coverage_tabs_are_non_overlapping` | `crates/dashboard/tests/dashboard.rs` |
| `catalog_and_review_are_separate_sections` | `crates/dashboard/tests/dashboard.rs` |
| `percentage_is_secondary_with_breakdown` | `crates/dashboard/tests/dashboard.rs` |

## What is deliberately undecided

**Whether the four `Academic` routes render.** No Tauri runtime is linked and
the link decision is open under the user gate. Everything above is a structure.

**Which of section 25.4's second line's categories exist.** The line is
`총 취득학점과 category별 학점`, and which categories a user's applied rule set
produces is `GATE-38-002`'s answer. `DashboardSection::CreditsByCategory`
carries the pairs a caller supplies and this crate names no category vocabulary
of its own; `academic_record::classify::RequirementCategory` is `P2-U4`'s five
and is not restated here.

**What `안 A/B/C` is a closed set of.** It is not one. See above.

**Whether a percentage should be shown at all.** Section 25.4 says *일 수
있으나* — it *may* be a secondary visualisation. This crate makes one
constructible and never required: `AcademicDashboard::assemble` takes
`Option<SecondaryPercentage>` and a screen with `None` has the same six
sections.
