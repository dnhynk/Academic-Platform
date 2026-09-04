# Non-delegable actions

`academic-non-delegable` is the `P2-M4` boundary: the compiled set of actions no
automatic actor may perform, and the command layer that refuses one. It closes
`INV-C-010` — *AI alone never resolves a question*.

It persists nothing. There is no `academic-store` edge, this task claims no
migration number, and a refusal is a value the caller records rather than a table
this task invents.

## The set, and where each entry comes from

Six actions. The execution plan's outcome sentence enumerates them; section 27 of
the design document does not enumerate the same six, and the difference is
carried in the type rather than erased.

| action | token | section 27.4 row | the door that already existed |
|---|---|---|---|
| resolve a question | `RESOLVE_QUESTION` | `non-delegable` | `academic_domain::question::VerifiedQuestionResolution` verifies `Actor::User` |
| confirm mastery | `CONFIRM_MASTERY` | `high risk` | `P2-N2`'s `UserConfirmation`, and an `AutomaticLevel` with no `Fluent` variant |
| decide enrollment or career | `DECIDE_ENROLLMENT_OR_CAREER` | `non-delegable` | **none** |
| attest a permission | `ATTEST_PERMISSION` | `non-delegable` | **none** |
| approve an egress | `APPROVE_EGRESS` | `high risk` | **none** |
| confirm a deletion | `CONFIRM_DELETION` | **none** | `P2-P2`'s `DeletionConfirmation` |

`NonDelegableAction::declared_tier` is that third column, answered in
`academic_proposal::RiskTier`, and `NonDelegableAction::declared_phrase` is the
document's own words for the entry. `the_spec_tables_are_this_action_set` parses
section 27.4's four rows and requires each phrase to occur in the row named here
**and in no other row**, so a document that renames or moves one fails this crate
rather than drifting past it.

### Three lists, and why they are not the same list

The plan and the specification are read together and neither is quietly made to
say the other's thing.

* **Section 27.4's `non-delegable` row names three things** — `question
  resolved, career/course decision, permission attestation는 사용자만`.
* **Its `high risk` row names three more** and requires `명시적 승인` of them —
  `Knowledge State 승격, private data 외부 반출, official rule publish`. Two of
  those three are in the plan's set; `official rule publish` is not, and this
  crate does not add it.
* **Section 3's `사용자가 소유하는 결정` names six**, and they are a different
  six again: it includes exploring or excluding a blind spot and approving an AI
  proposal, and it names neither permission attestation nor deletion
  confirmation.
* **Deletion confirmation is in none of them.** Section 27 does not contain the
  word `삭제`, `deletion` or `delete` at all, and
  `the_spec_tables_are_this_action_set` measures that over the whole section
  rather than over 27.4. Its basis is the execution plan, and `P2-P2` built it
  before this task merged.

What unifies the six is the narrower claim the plan actually states and this
crate actually enforces: **every entry needs an authenticated user actor and an
explicit decision event.** That is true of the high-risk row too, because
`명시적 승인` is `P2-M2`'s `ExplicitApproval`, which is built from a
`UserDecision` that only `Actor::User` can mint. It is *not* the same as section
27.4's `non-delegable` row, and this page does not claim it is.

## What this crate adds, and what it only agrees with

Three of the six were already refused by a type. For those, this task's
contribution is that its compiled constant **agrees** with them, measured by
driving the real door:

* `ai_cannot_resolve_a_question` offers `academic-domain` the user-explicit,
  user-confirmed claim a forgery would have to carry, with each automatic actor,
  and observes the refusal; then opens both doors for the user, so the refusals
  are attributable to the actor and not to the fixture.
* `ai_cannot_confirm_mastery` does the same against `P2-N2`'s
  `UserConfirmation::verify`, and then observes that no `AutomaticLevel` maps to
  `MasteryLevel::Fluent`.
* `ai_cannot_confirm_deletion` builds `P2-P2`'s real dry run, real preview and
  real confirmation and drives `DeletionConfirmation::given` with each automatic
  actor. **The deletion confirmation is not implemented twice**: both that type
  and this crate's `DecisionEvent` go through `academic_proposal::UserDecision`,
  so they are one fact rather than two checks that could drift.

Three were refused nowhere, and those are the ones this layer closes. Each
absence is measured rather than assumed:

* `academic_record::RegistrationConfirmation::new` takes a course code, a term, a
  credit count and evidence identifiers — and **no actor**.
  `ai_cannot_decide_enrollment_or_career` builds one with values a model run
  could supply and observes it succeed.
* `academic_consent::AuthorityGrant::record` takes the written authority, the
  permitted use, the retention terms, the conditions and an expiry — and **no
  actor**. `ai_cannot_attest_permission` builds a real grant with no user
  identity anywhere in it.
* `academic_policy::PermissionRequest::actor_id` is an `Option<String>` and
  `EgressRule::actor_id` is a `String` compared for equality, so the broker has
  no notion of actor *kind*. `ai_cannot_approve_egress` installs two rules
  identical in all fifteen other fields and different only in that one, one
  naming a user and one naming a model run, and observes the real broker **allow
  both**. The allow is the load-bearing half; a pair of denials would have agreed
  for any reason at all.

If any of those three crates later grows an actor parameter, this suite stops
compiling. That is the intended signal: the two layers changed and have to be
reconciled.

## Where this is enforced, and where it is not

**The daemon has no product command surface yet.**
`academic_rpc::ValidatedWriteCommand` has three synthetic Phase 1 arms —
ingest, backup, restore — and **no arm carries an actor at all**. There is no
wire command for any of the six actions and therefore no arm for a refusal to
sit inside. `authorise` is the door a command layer calls before it dispatches;
wiring it to a proto command arm belongs to the task that adds that arm.

**A caller that skips this door skips the refusal.**
`RegistrationConfirmation::new` and `AuthorityGrant::record` are public and take
no actor, so a surface that reaches them directly is not refused by anything.
That is stated here rather than implied, and it is the reason the plan's outcome
says *enforced in the daemon command layer, not only in the UI* — the UI is not
the boundary, and neither is any single owning crate.

## Why graduation is a different axis

Section 27.2's ninth bullet is `graduation pass/fail을 자유 텍스트 generation으로
결정`. It is **not** a member of the set above, and putting it there would have
been wrong in a way worth stating: the six actions refuse
`DETERMINISTIC_ENGINE`, and a deterministic engine is exactly the correct author
of a graduation result — section 28's `Graduation Audit` row is an engine, and
`P2-U3` is that engine. What the bullet forbids is a *generation* deciding it,
which is a claim about where the input came from and not about which actor
pressed the button.

`graduation_result_cannot_come_from_generation` holds it as three facts:

1. no row of section 27.1 is a graduation row, so a model may not even produce a
   candidate for one, and no `Action` in this layer yields one;
2. `academic_domain::InputValue::Reference` is `identifier-shaped … never a
   sentence`, so free text cannot enter the frozen inputs the engine is a
   function of — driven with a sentence, which is refused, and with an
   identifier, which is not; and
3. `academic_audit::DeterminateVerdict::new` is `pub(crate)`, so no crate outside
   `academic-audit` can assemble a verdict at all.
   `tests/compile_fail/a_verdict_cannot_be_assembled_outside_its_crate.rs` names
   it from here and observes the error.

## The classification is total, not a list

`Action` is closed over section 27.1's ten candidate-generation rows and the six
above, and `Action::delegability` names all sixteen individually. A seventeenth
variant added to either half makes that `match` non-exhaustive and the crate
stops compiling until it says which side it is on. `Delegability` carries the
action in each arm, so `authorise` dispatches on the classification and has no
arm it must call impossible.

Section 27.1's ten rows are compared against `CandidateGeneration::spec_row` in
both directions, by parsing the table out of the design document.

## What the guards catch, measured

Three injections were written, each compiling and reachable, each spelling none
of the six action tokens, each built and run on its own. Every one of the twelve
acceptance tests passed for all three; exactly one guard caught each, and with
that guard removed the whole suite passed:

| injection | caught by | acceptance tests that saw it |
|---|---|---|
| `impl From<ActionCommand> for AuthorizedCommand` returning a proposal for anything | `the_impl_blocks_naming_the_gate_types_are_these` | none |
| `pub fn relay(ActionCommand) -> Result<AuthorizedCommand, _>` returning a proposal for anything | `the_public_signature_set_is_this` | none |
| a `note: String` field on `ActionCommand` | `every_field_type_in_this_crate_is_reviewed` | none |

The first is the `P2-X5` class — a `trait impl` declares no `pub fn`, so it
escapes every public-signature sweep. The second is the `P2-N7` class — a public
function that spells none of the names on a list. The third is the `P2-U8` class
— a field holding text that a name-matching tool does not classify.

A sixth scan was added after this crate's own documentation was found citing
**three tests that were never written**. They were working names from a design
the tests moved away from, and nothing in the crate noticed. A sentence that
cites evidence which does not exist is the same defect as a guard that checks
nothing: it reads as proof and is not.
`every_test_this_crates_docs_cite_exists` compares every backticked lower-snake
name of three or more words in a product doc comment against the `fn` set of
this crate's three test targets. Reintroducing one of the three phantom names
makes it fail with that name in the message, which was observed before the fix
was kept.

Two semantic guards were removed and the suite observed to fail:

* removing the actor check in `DecisionEvent::recorded` fails eight tests,
  including all six named acceptance tests. The first attempt at removing it did
  not compile at all, because `DecisionEvent` holds a `UserDecision` whose field
  is private and whose only producer refuses automatic actors;
* removing the `delegability` dispatch in `authorise`, so every command comes
  back as a proposal, fails nine.

Neither masks the other: the first leaves a user's non-delegable command coming
back as an AI candidate, and the second leaves an automatic actor's command
coming back as a decision.

## What this is not evidence for

**The `PREDICTION` gate is untouched.** Whether a deterministic engine may assert
its own forecast under `AuthorityClass::Prediction` is a separate open user
decision about *claims*. This crate is about *decisions*, names no authority
class, and has no `academic-ledger` edge.

**Posture is unchanged.** `adr_002_accepted` stays `false` and
`production_data_allowed` stays `false`.
