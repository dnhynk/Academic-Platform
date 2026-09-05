# The typed requirement rule DSL and RuleSet publication

`academic-requirement` is `P2-U2`. It holds section 11.2's rule types, section
11.1's `DegreeRequirementSet`, and the reviewer gate that stops a
model-extracted rule candidate from ever executing. It persists nothing, opens
nothing, and runs no connector: migration `0015` holds the typed rows and
`crates/store` owns them.

## The rule types are fourteen, and the count is not what decides it

`t068` section 5's `P2-U2` entry says *all thirteen §11.2 rule types* and then
lists **fourteen** in its own parenthesis. Its own acceptance evidence names
**fourteen** `dsl_*` tests. The word is wrong. It is the fifth count error found
in this plan — after `§28`'s twelve engines called thirteen, `§31.3`'s fifteen
dimensions called thirteen, `§14.2`'s six states called seven, and `P2-U1`'s
"five names, four relations".

The specification says it in two places, and neither is a list of fourteen on
its own.

| Reading | Where | What it gives |
|---|---|---|
| The yaml block | lines 597–629 | five `type:` values, and they are the only rule-type identifiers anywhere in the document |
| The prose sentence | line 632 | twelve categories, opening with *rule type에는 … 를 포함한다* — "includes" |

The two overlap. The prose's **course set** is one category and the yaml spells
three distinct types under it, because a set every operand must satisfy, a
choice of `n`, and a count under constraints have three different operand
shapes. Twelve prose categories with *course set* opened into the yaml's three
is fourteen.

The independent reading agrees. `t001`'s requirement matrix, derived from the
specification line by line without reference to `t068`, gives each rule type its
own row: `REQ-11-004` through `REQ-11-017`, fourteen consecutive requirements,
each naming one of the fourteen `dsl_*` tests.

**Nothing in this crate asserts a count.** `SPEC_YAML_TYPES` and
`SPEC_PROSE_CATEGORIES` hold both readings and
`the_rule_types_are_the_specifications_own` parses each back out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares in both
directions. Injection `U2-I4` dropped a rule type from `RuleType::ALL` *and*
from the prose table *and* adjusted both declared lengths, so no count moved,
and the test failed because the specification still writes the category.

### Where each identifier comes from

| Rule type | Spelling from | Requirement | Test |
|---|---|---|---|
| `CREDIT_MINIMUM` | §11.2 yaml | `REQ-11-004` | `dsl_credit_minimum` |
| `ALL_OF` | §11.2 yaml | `REQ-11-005` | `dsl_required_course_set` |
| `AT_LEAST_N_OF` | §11.2 yaml | `REQ-11-006` | `dsl_at_least_n` |
| `COUNT_WITH_CONSTRAINTS` | §11.2 yaml | `REQ-11-007` | `dsl_count_constraints` |
| `GPA_MINIMUM` | §11.2 yaml | `REQ-11-008` | `dsl_gpa_minimum` |
| `AREA_DISTRIBUTION` | §11.2 prose | `REQ-11-009` | `dsl_area_distribution` |
| `CO_REQUISITE` | §11.2 prose | `REQ-11-010` | `dsl_corequisite` |
| `MUTUALLY_EXCLUSIVE` | §11.2 prose | `REQ-11-011` | `dsl_mutually_exclusive` |
| `EQUIVALENCY` | §11.2 prose | `REQ-11-012` | `dsl_equivalency` |
| `MAXIMUM_RECOGNITION` | §11.2 prose | `REQ-11-013` | `dsl_maximum_recognition` |
| `NON_CREDIT_TRAINING` | §11.2 prose | `REQ-11-014` | `dsl_noncredit_training` |
| `LANGUAGE_OF_INSTRUCTION` | §11.2 prose | `REQ-11-015` | `dsl_language_instruction` |
| `THESIS_RESEARCH` | §11.2 prose | `REQ-11-016` | `dsl_thesis_research` |
| `EXCEPTION_APPROVAL` | §11.2 prose | `REQ-11-017` | `dsl_exception_approval` |

The five yaml identifiers are the specification's own bytes and are compared
against it. The nine prose ones have no identifier in the document, so the
`SCREAMING_SNAKE_CASE` spelling is this crate's — **derived**, not invented, by
one mechanical rule: upper-case, with each space, hyphen or slash becoming an
underscore. The scan applies that rule to the prose name and requires the result
to be the identifier, so a respelling fails; injection `U2-I5` is
`NONCREDIT_TRAINING`.

`academic-proposal` met the same shape and resolved it the same way: section
27.4 states four risk tiers in prose and names no identifier, so the plan
supplied the spelling and the section stayed the authority for the meaning.

## The review gate is a type, not a check

Section 11.2: *LLM은 원문에서 rule 후보를 추출할 수 있으나 사람이 검토한
executable rule만 production audit에 사용한다.*

```text
RuleCandidate  ── ReviewGate::admit(candidate, first, second) ──▶ ReviewedRule
                       two attestations, two people                    │
                                                                       │
        RuleSetDraft::include(reviewed, official, synthetic) ───────────┘
                       both fixture classes, carried with the rule
                                     │
                     RuleSetDraft::publish() ── every case, against
                                     │          the whole published set
                                     ▼
                             ExecutableRule ──▶ RuleSet::evaluate
```

`ReviewedRule` and `ExecutableRule` both have private fields and **no public
constructor**. Each is built in exactly one expression in this crate: the tail
of `ReviewGate::admit` and the body of `RuleSetDraft::include`. Neither
implements `From`, `Into`, `Deref` or `AsRef`. So a caller holding a
`RuleCandidate` cannot reach either — not because a check refuses, but because
there is no call to make.

Five compile-fail cases are that absence, and `tests/compile_fail/` is where a
running test cannot go:

| Case | Diagnostic |
|---|---|
| `a_candidate_cannot_be_published` | `E0308` on `include`, `E0599` on `ReviewedRule::new`, `E0277` on `From<RuleCandidate>` |
| `a_reviewed_rule_has_no_struct_literal` | `E0451` — five private fields |
| `a_candidate_cannot_be_evaluated` | `E0599` on `evaluate`, `E0425` on a text interpreter that does not exist |
| `an_executable_rule_has_no_public_constructor` | `E0599` on `new`, `E0616` on `RuleSet::rules` |
| `an_executable_rule_has_no_struct_literal` | `E0451` — three private fields |

**The two literal cases are separate files on purpose.** `E0451` comes from the
privacy pass, which rustc does not reach once type checking has already failed.
The first version of this suite put each literal beside three other refused
routes, and the committed `.stderr` recorded three errors, none of them about a
literal: the half meant to prove the fields are private proved nothing. One
route per file is the rule that follows, and it is the same "one injection, one
build" rule [policy source scans](policy-source-scans.md) states.

### Why two attestations, and why two people

`ReviewGate::admit` takes the two attestations as **two parameters**, so the
arity is the requirement: there is no call that supplies one. What the body adds
is that the two must name different reviewers and both must be `Actor::User`.
Two attestations by one person is one review recorded twice; a model attesting
to its own candidate is the gate reviewing itself.

Both refusals are typed errors and both are driven by
`rule_candidate_review_gate`. Injection `U2-I9` removed the reviewer comparison
and `U2-I20` admitted an `Actor::ModelRun`; each was refused.

Migration `0015` is the second layer, and it does not depend on the Rust
boundary having been used: `requirement_rule_review`'s primary key is
`(requirement_rule_id, reviewer_entity_id)`, so one person cannot attest twice
to one rule however the row was written.

## No free-text interpretation on the audit path

`production_audit_no_llm` is three halves, in the two shapes §2.3-14 already
establishes for a capability plus one specific to this task.

**Available.** This crate's transitive product closure is computed from the
manifests and compared *whole* against a twelve-entry list. `academic-model-run`
— §27.3's provenance aggregate, which is where a model execution is recorded —
is absent, as is every HTTP client. An addition of any kind fails as an extra
key rather than having to be predicted: injection `U2-I12` added an
`academic-record` edge, which is on no forbidden list, and was refused.

`academic-untrusted-content` **is** in the closure, transitively through
`academic-ingestion`. That crate's whole purpose is that a provider response
cannot be unwrapped into a string without naming the exposure site; it runs no
model and calls none. It is named in the list with that reason rather than
quietly excluded.

**Used.** The product source is scanned for the API spellings of a model call, a
clock, an RNG and a socket, with the samples run through the check inside the
test so a rule that matches nothing fails.

**Interpreted.** This is the half a token list cannot do. The *whole set* of
`String` and `&str` fields in the product source is compared against a table of
owning-type/field pairs — one on `RuleCandidate`, thirteen on
`RequirementError`. The pair rather than the name is load-bearing for the reason
`U-I5` made load-bearing for `P2-U1`: a name compared alone is satisfied by the
same name one type over.

A second rule then forbids any of eight audit-path types from owning such a
field at all. **That rule cannot be satisfied by editing the table**, which is
what injection `U2-I24` measured: it added `provenance: String` to
`ExecutableRule` *and* added the matching row to the allowance table, and was
still refused.

The identifier newtypes are a `String` behind a validator that admits no space.
They are enumerated rather than exempted — but they are generated by a macro, so
no source sweep can see their names: the first version of this scan reported one
newtype called the empty string and none of the six real ones. The macro body is
pinned whole and its invocation list is read instead.

Migration `0015` carries the same rule one layer down: `requirement_rule` has no
`body_text` and no `quoted_source` column, and each table's whole column set is
compared, so a column nobody predicted appears as a difference. Injection
`U2-I16` added `interpretation TEXT`, which is on no forbidden list, and was
refused.

## What immutable publication is executed as

Section 11.4: *변경은 기존 RuleSet을 수정하지 않고 새 버전을 publish한다* and
*과거 audit은 당시 입력과 rule hash로 재현한다*.

`RuleSet` has private fields, no `&mut self` method and no setter — that makes
one value immutable. What makes the *history* immutable is `RuleSetLedger`:
`publish` refuses a version number the ledger already holds and refuses a
supersession that names anything but the head. Both versions stay addressable by
their own `rule_set_hash`, which is what a historical replay walks.

**`rule_set_hash` is taken over every field of the set**, not over the fields
that look like identity: `RuleSet`'s six, all four of `OfficialSourceBinding`'s,
and all three of `ExecutableRule`'s — including the compiled body, which
`RuleBody::canonical_text` renders totally over the fourteen rule types. That
sentence is the repair for a hash that carried a rule's identifier, its type and
its source digest and nothing the rule *said*: two sets differing only in
`CREDIT_MINIMUM`'s threshold hashed alike, produced a byte-identical
`AuditInputBinding`, and answered 졸업 불가 and 졸업 가능, with the stricter
audit's recorded hash replaying against the laxer bodies and being accepted.
The same rendering dropped three of the four fields of the source binding, one
of which — `retrieved_at` — is what `academic-audit`'s freshness gate reads, so
a fresh set and a stale one hashed alike too.

Three tests hold it, and none of them is a count.
`the_canonical_renderings_bind_every_field` compares the field set each type
**declares** against the field set each renderer **destructures**, in both
directions, and refuses a `..` in either renderer.
`every_rule_body_field_moves_the_canonical_text` walks the fourteen arms moving
one field at a time and requires every rendering to be distinct.
`every_rule_set_field_moves_the_hash` does the same for the twelve movable
positions of a published set. The first says the field is bound; the other two
say the binding reaches the bytes.

`ruleset_immutable_publish` asserts that publishing a successor leaves the
predecessor byte-identical, that two published versions hash differently, *and*
— separately, because the first pair differs in its version number and its
supersession too — that two sets differing **only** in a credit threshold hash
differently. The audit that found the hole found the old assertion passing on
its version number while its message named the threshold.

Migration `0015` is the second layer: `requirement_set_version` makes
`supersedes_version` and `rule_set_hash` `UNIQUE`, so the chain cannot fork and
two versions cannot share content.

**The version primary key cannot be widened in place.** `requirement_rule`'s
composite foreign key needs a unique index on exactly
`(requirement_set_id, version)`, so injection `U2-I19` produced *foreign key
mismatch* before any row was written — refused, but not by the property. What
isolates the primary key is the test's own case, which supersedes nothing and
carries a fresh hash so neither `UNIQUE` can be what refuses it.

## The release gate runs its fixtures

Section 11.4: *새 rule은 공식 예시와 synthetic transcript fixture로 회귀
검증한다*. `RuleSetDraft::include` takes the two classes as **two parameters**,
so a release with one of them is not a call that can be written, and each class
refuses construction when empty.

What makes it more than two files existing is that each case declares the
verdict the rule must reach on it and **is evaluated**, so a fixture that
disagrees stops the publication. That is the lesson [the engine
harness](engine-harness.md) records about adverse fixture directories, applied
where the fixture is a value rather than a file. Injection `U2-I11` disabled the
comparison and was refused.

**Against the published set, not against a prefix of it.** `include` can only
reach the rules admitted before the one it is admitting, and `RuleSet::evaluate`
resolves a `COURSE_OR_EQUIVALENT` operand through the `EQUIVALENCY` rules of the
set it is handed. Running the cases at `include` was therefore not a weaker
check but a wrong one, in both directions and depending only on admission order:
a rule admitted before its equivalency was released on a fixture declaring
`NOT_SATISFIED` for facts the published set answers `Satisfied` for, and the
same rule's release was **refused** when the reviewer declared the status the
published set actually reaches. Nothing enforced an order, and
`crates/audit/tests/support/mod.rs` carried the workaround as a comment on its
own fixture list.

So `include` keeps the classes beside the rule and `RuleSetDraft::publish` runs
every case of every admitted rule against the whole set.
`the_release_fixtures_run_against_the_published_set` drives both directions
under both admission orders, with a control that the same rule alone really does
answer `NOT_SATISFIED`, so the pair is a measurement rather than a fixture that
cannot fail.

Every rule in every `dsl_*` test goes through the gate and through this
admission, so both run fourteen more times than their own named tests run them.

## Absence is `UNKNOWN`

`GATE-38-011`, `GATE-38-012`, `GATE-38-015` and `GATE-38-016` are open, and
`OpenGate` states each where it bites.

**`GATE-38-011`** — which admission cohort a rule applies to, and the
transitional arrangement between two standards. A rule scoped by admission year
against an unrecorded year evaluates to `UNKNOWN`, and no cohort is assumed from
a term, an attempt, or a sibling rule.

**`GATE-38-012`** — the exact scope and transitional arrangement of the 2027-1
thesis-research requirement. Section 8.1 says it needs a departmental notice and
an administrative confirmation. An unresolved applicability evaluates to
`UNKNOWN` whatever the record holds, *including a completed thesis*.

**`GATE-38-015`** — whether one attempt may count toward two majors at once. A
`MUTUALLY_EXCLUSIVE` rule with no confirmed ceiling evaluates to `UNKNOWN`, and
nothing infers a ceiling from the member count.

**`GATE-38-016`** — how much external, transferred or exchange credit is
recognized. A `MAXIMUM_RECOGNITION` rule with no confirmed cap evaluates to
`UNKNOWN`, and nothing infers a cap from the credits presented.

The verdict carries the cell that produced it, so an audit above can display the
exact missing check rather than a number. `the_open_gates_have_no_default`
compares the whole set of `Default` implementations in the crate against a
one-entry list; the one that exists is the empty ledger, which is emptiness
rather than a value. Injection `U2-I6` gave `Applicability` a `Default` and was
refused.

## What this contract does not claim

- **It does not claim no code anywhere can construct an executable rule.** What
  is executed is narrower and is a composite: both gated types have private
  fields and one construction site each, counted over every product file in the
  crate; the whole `impl` set naming either is compared against a two-entry
  list; the whole set of public signatures taking a `RuleCandidate` to a gated
  value is compared against one, which is the gate itself; and no file outside
  this crate names any of them. A module that spells none of those and
  reimplements the semantics from scratch is refused by nothing here — it also
  reaches none of this crate's values.

- **A pin can be edited by whoever changes the thing it pins.** `WHOLE_ADMIT`,
  `WHOLE_INCLUDE`, `WHOLE_PUBLISH`, `WHOLE_USER_ID`, `GATE_SIGNATURE`,
  `WHOLE_IDENTIFIER_MACRO` and `PRODUCT_CLOSURE` all have that property, as
  every pin in this repository does. What they buy is that the change is visible
  in the diff and has to be argued for, not that it is impossible.

- **This crate is not the audit engine.** The proof tree, the `INDETERMINATE`
  selector and the three-gate `DETERMINATE` rule are `P2-U3`'s. What is here is
  one rule's verdict with everything a leaf needs — status, exact measure, used
  attempts, equivalency decisions, and the open gate when there is one.

- **The `EQUIVALENCY` rule type is not `academic-curriculum`'s
  `EquivalenceRelation`.** That is a catalogue fact about two courses; this is a
  substitution one published requirement set admits. A requirement set that
  silently inherited the catalogue's equivalences would change meaning when the
  catalogue changed, and section 11.4 forbids exactly that by making a change a
  new version rather than an edit. `Operand` resolves against the rules
  published beside it and nothing else, and this crate has no
  `academic-curriculum` edge, so `no_file_outside_this_crate_names_a_curriculum_relation`
  stays empty.

- **`REQ-28-002` and `REQ-28-006` are not closed here.** `t068` lists both under
  `P2-U2`. They are §28 registry engines — `CREDIT_ACCOUNTING`, already
  `IMPLEMENTED` by `P2-U4`, and `EQUIVALENCY`, still `PLANNED` — and
  [the engine harness](engine-harness.md) is where they are accounted for. What
  `P2-U2` closes from §28 is `REQ-28-013`, which that page already records as an
  obligation on rule publication rather than an engine of its own. No registry
  entry changes lifecycle in this task and no source here names a registered
  `engine_id`.

- **`product_network` remains `NONE` and `production_data_allowed` remains
  `false`.** Nothing in this task moves either.

- **Every fixture is synthetic.** No byte here derives from a personal record, a
  transcript, or an external fetch. The one `PublishedRules` each test set is
  founded on comes from running `P2-U6`'s own pipeline over `P2-U6`'s own
  fixtures.

## Named acceptance evidence

| Test | Where | Proves |
|---|---|---|
| `dsl_credit_minimum` … `dsl_exception_approval` (fourteen) | `tests/requirement.rs` | each rule type compiles, evaluates, and reports its exact measure; the four open cells read `UNKNOWN` |
| `rule_candidate_review_gate` | `tests/requirement.rs` and `tests/compile_fail/` | five type-level routes are absent; one reviewer twice, a non-user reviewer, a mismatched attestation and a malformed body are each refused; two people admit |
| `ruleset_immutable_publish` | `tests/requirement.rs` and `crates/store/src/requirement_tests.rs` | the predecessor is byte-identical after a successor; different content hashes differently; republishing and forking are refused in Rust and in SQL |
| `new_rule_release_gate_requires_official_and_synthetic_fixtures` | `tests/requirement.rs` | an empty class is refused at construction; a case that disagrees with the rule stops the release; both classes agreeing admits |
| `the_release_fixtures_run_against_the_published_set` | `tests/requirement.rs` | under both admission orders the true declared status is released and the false one is refused, with a control that the rule alone answers the false one |
| `the_two_reviewers_attest_the_document_rule_as_well` | `tests/requirement.rs` | either reviewer naming another document rule is refused and the refusal names both; both naming the candidate's own is admitted and carried onto the reviewed rule |
| `production_audit_no_llm` | `tests/requirement_scans.rs` and `crates/store/src/requirement_tests.rs` | the closure, the API spellings, the whole free-text inventory, the audit-path rule, and the absence of a text column |

The five source scans that hold what a behavioural test cannot observe —
`the_rule_types_are_the_specifications_own`, `production_audit_no_llm`,
`the_only_route_to_an_executable_rule_is_the_gate`,
`the_open_gates_have_no_default` and `no_float_reaches_a_requirement_verdict` —
plus the walk and the one-step-out inventory are in
`crates/requirement/tests/requirement_scans.rs` and are enumerated in
[policy source scans](policy-source-scans.md), with the twenty-four injections
each was measured against.
