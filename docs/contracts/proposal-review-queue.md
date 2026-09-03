# The proposal boundary, risk tiers, and the review queue

`academic-proposal` is the `P2-M2` boundary between what a model proposed and
what this system will record. It holds `Proposed<T>`, the four risk tiers of
section 27.4 of the authoritative spec, the review queue with section 29.7's
confidence/impact batching, and the append-only disposition history that makes a
decision reversible.

It persists nothing. The typed rows are `academic-store`'s, written by migration
`0009`, and the deliberate absence of a Cargo edge to that crate is what makes
`proposed_type_cannot_reach_canonical_writer` a compile error rather than a
source scan.

## Where the spec says this

The execution plan cites `Proposed<T>` as section 3.10. **There is no section
3.10.** Section 3 is `User Model` and has no numbered subsections at all — its
three headings are the user's roles, the minimum profile, and the decisions the
user owns. `Proposed<`, `LOW_AUTOSAVE`, `MEDIUM_REVIEW`, `HIGH_APPROVAL` and
`NON_DELEGABLE` appear nowhere in the spec.

The sections that do say this are:

| What | Section |
|---|---|
| what a proposal is, and what a model may produce a candidate for | 27.1, `AI가 담당하는 후보 생성` |
| what a model may not decide | 27.2 |
| the four risk tiers and the workflow each requires | 27.4, `Human-in-the-loop 강도` |
| confidence/impact review batching, so approval is not spam | 29.7, `사용자 입력` |
| ingestion ends at claim publication *or* a review queue | 29.1 |
| the user owns approve, modify and reject over an AI proposal | 3, `사용자가 소유하는 결정` |

The four tier tokens are the execution plan's spellings. Section 27.4 states its
four rows in prose and names no identifier, so the plan supplies the spelling
and section 27.4 is the authority for what each row means.

### The plan references that do not resolve

Collected here because the same references keep being met. Each is a case where
the plan and the spec disagree and **the spec is authoritative**; in every case
the resolution was to take what the spec says and not to invent the missing
part.

| Plan says | Spec says | Resolution |
|---|---|---|
| `Proposed<T>` is section 3.10 | section 3 has no subsections | cite 27.4, 27.1, 29.7, 29.1 and section 3 (this task) |
| the twelve model-run fields are section 3.10 | they are section 27.3 | `P2-M1` cites 27.3 |
| `four_dispositions_are_durable_and_audited` | section 3 names three | three, enumerated (below) |
| a thirteen-engine registry | section 28's table has twelve rows | twelve |
| a `..._thirteen_...` blind-spot test name | section 31.3 has fifteen dimensions | fifteen, renamed |
| a seven-state lifecycle | section 14.2 has six states | six |

## The label is a type

`Proposed<T>` wraps a candidate at the moment its risk tier is decided. It
implements no `Deref`, `DerefMut`, `AsRef`, `AsMut`, `Borrow`, `Display`,
`ToString`, `From` or `Into`, and it has one accessor for the payload,
`pub(crate) fn release`, which no caller outside the crate can name.

That is a statement about what a caller can *call*. It is not, by itself, a
statement about what a caller can *get*: this crate is free to call `release` on
a caller's behalf, which is exactly what the three doors below do. So the claim
this page makes is the narrower one, and it is the one that is checked: **every
call to `release` in this crate's product source is counted and inventoried
below, each behind a disposition that is already recorded, and no public
signature in the crate takes a `Proposed<…>` and returns its payload.**

The count reads whole identifiers on both sides. `P2-RF10` repaired an inventory
that read a spelling on the use side, and `P2-RF11` one that read a spelling on
the declaration side; both shapes are injected here (`M2-I1`, `M2-I2`) rather
than assumed, because the repaired helpers were copied into a new file.

`Proposed<T>`'s `Debug` is hand-written, prints identity, tier, confidence and
impact, and is implemented for every `T` with no `T: Debug` bound — so there is
no instantiation whose payload a format string reaches.

### The three places the payload comes out, and why each is allowed

`every_release_site_is_named_and_justified` compares the whole inventory of the
accessor's call sites against this list, counted by name. A fourth fails as an
extra key however it is spelled; a removed one fails as a missing key.

| Site | Why |
|---|---|
| `ReviewQueue::autosave` | Section 27.4's low-risk row saves without a human. What leaves is an `Autosaved<T>`, whose epistemic status is a constant equal to `AI_INFERRED`. |
| `ReviewQueue::approve` | The high-risk row needs an explicit approval. The approval that named this exact proposal is already in the history when the value leaves. |
| `ReviewQueue::commit` | The two queued rows release only after a user `CONFIRM` for this exact proposal is in the history, which the call checks immediately above. |

## The four tiers, and the four doors

`RiskTier::workflow` is a total `match`, so a fifth tier stops the crate
compiling until it names its workflow.

| Tier (27.4) | Workflow | Door | What settles it |
|---|---|---|---|
| `LOW_AUTOSAVE` | `AUTOSAVE_AS_AI_INFERRED` | `autosave` | nothing human; the record is `AI_INFERRED` and can be nothing else |
| `MEDIUM_REVIEW` | `QUEUE_AND_UNDO` | `review`, then `commit`; `undo` reverses | a user disposition, reversible afterwards |
| `HIGH_APPROVAL` | `EXPLICIT_APPROVAL` | `approve` | an `ExplicitApproval` carrying this proposal's identity |
| `NON_DELEGABLE` | `USER_ONLY` | `decide`, then `commit` | a `UserDecision`, which only `Actor::User` mints |

`ReviewQueue::require` is the one place a tier is compared against the workflow
a caller reached for. Each door calls it with the workflow it serves, so the
mapping is executed once rather than by four conditions that could drift apart.
`every_tier_reaches_only_its_own_workflow` drives all sixteen cells and requires
exactly the four on the diagonal to be accepted and the other twelve to be
refused with `WrongWorkflow` — so swapping any two rows of the mapping moves two
cells and fails.

`WHOLE_REQUIRE` pins that comparison as text, and `DOOR_GUARDS` pins the first
statement of each of the four doors beside it. `T141` found a pinned check
skipped by a condition wrapped around the *call*, so the pin alone would carry
that hole.

### `LOW_AUTOSAVE` persists only as `AI_INFERRED`

`Autosaved<T>` has no status field. `Autosaved::EPISTEMIC_STATUS` is a constant
and `epistemic_status` returns it, so there is no argument a caller could pass
that would make an autosaved record anything else. `Approved<T>` is the same
shape around `USER_CONFIRMED`. Migration `0009`'s
`guard_proposal_outcome_matches_tier` refuses the other direction too: a
`LOW_AUTOSAVE` outcome that is not `AI_INFERRED`, and a reviewed proposal's
outcome that is.

### `NON_DELEGABLE` is user-only

Every public method of `ReviewQueue` that can move a proposal is named in
`SETTLEMENT_DOORS` with the reason an automatic actor cannot use it, and the
list is compared against the queue's whole public `&mut self` surface, so a
fifth door fails as an extra key. `non_delegable_has_no_automatic_actor_path`
runs each one:

* `autosave`, `review` and `approve` are other tiers' doors and are refused by
  the workflow comparison;
* `decide` takes a `UserDecision`, which `UserDecision::by` issues only for
  `Actor::User` — an exhaustive `match` over `academic-domain`'s closed `Actor`
  enum, so a fifth actor variant stops this crate compiling until it is
  classified;
* `commit` needs a recorded `CONFIRM` that only `decide` can have put there; and
* `undo` needs an open record that only `decide` can have opened.

`admit` is in the list and takes no actor: admission is not a disposition and
records nothing. Section 27.1 lets a model produce a candidate; section 27.2 is
about deciding, and nothing here lets a model do that.

A `NON_DELEGABLE` proposal is also never grouped with another in a batch. Bulk
approval is the shortcut that turns a user-only act into a rubber stamp, and a
singleton batch is how such a proposal stays in the partition without being
grouped.

## Three dispositions, not four

**The execution plan names its acceptance test
`four_dispositions_are_durable_and_audited`. The test here is
`three_dispositions_are_durable_and_audited`.** An audit looking for the plan's
name should read this section.

There are three. Section 3 of the spec names exactly three things a user does
with an AI proposal — approve, modify, reject — and no section names a fourth.
ADR-003 froze those three as `academic-domain`'s `DecisionAction`, whose arms
are `Confirm`, `Reject` and `Replace`, and whose semantics the ledger's resolver
already replays in acceptance order. This crate therefore adds no vocabulary:
the disposition a record carries **is** a `DecisionAction`.

| Section 3 | `DecisionAction` | Token |
|---|---|---|
| 승인 (approve) | `Confirm` | `CONFIRM` |
| 수정 (modify) | `Replace { replacement_claim_id }` | `REPLACE` |
| 거절 (reject) | `Reject` | `REJECT` |

`disposition_token` is compared against what serde emits for each arm, so the
spelling is the frozen wire contract's rather than a second list written beside
it.

`Replace` does not release the proposal's payload. ADR-003 has a replacement
reject the target and select a different object, so the model's own candidate is
not what becomes the record; `commit` refuses it by name with `NotConfirmed`.

### Pending is a state, not a decision

A proposal nobody has decided on is `DispositionState::Undisposed`. That is not
a `DecisionAction`, has no conversion into one, and cannot be handed to anything
that ranks user authority. `pending_is_not_a_disposition` in
`tests/compile_fail` is that as a compile error.

The reason is ADR-003's authority computation. A fourth token meaning "seen, not
now" would take a place in it, and "undecided" would then read as "the user
judged". Section 29.7's batching is about **what a reviewer is shown and when**,
which is a property of the queue; it is not a claim about what the user decided.

## Undo appends

`ReviewQueue::undo` pushes a record whose `supersedes` names the record it
reverses and removes nothing, which is ADR-003's rule for every canonical
correction. The reversed disposition is carried on the undo record, so the pair
reads as "this reverses the `REJECT` at sequence four" without needing a fourth
disposition that means nothing on its own. `state_of` walks the history and
returns `Undisposed` after an undo, so the entry is pending again.

`rejected_proposal_is_retained` is the same rule read from the other side. A
rejection is a decision recorded against a proposal, not the removal of one: the
queue has no method that removes an entry at all, the payload stays, and the
record survives every decision that follows it. Migration `0009` holds the same
thing in SQL through the append-only trigger pairs and the SQLite authorizer.

Each `DispositionRecord` carries a SHA-256 over every field, including the
`replacement_claim_id` a `Replace` names — hashing only the token would give a
replacement of one claim and a replacement of another the same digest.
`the_disposition_digest_covers_every_field` moves each field in turn and
requires each move to change the digest.

## Batching, and what "without loss" means

`ReviewQueue::batches` partitions the pending set by
`(thresholds_version, tier, confidence_band, impact_band)`. Bands are half-open
upward, so a value exactly on a cut belongs to the band above it and every value
lands in exactly one band.

`high_volume_proposals_are_batched_without_loss` runs four hundred proposals
across every tier and both axes, including values exactly on each cut, and
asserts **set equality in both directions** plus no duplication — not a count,
which would pass an implementation that dropped one item and emitted another
twice. Beside it runs a deliberately lossy batcher, and the test requires *that*
one to fail the same equality: without the control, the assertion would pass
over an implementation that could not lose anything.

`BatchingThresholds` carries its own version and a digest over its contents, so
a configuration edited without renumbering produces a different digest and the
version claim is checkable rather than decorative. A batch key carries the
version it was computed under, so batches from two configurations are
distinguishable rather than silently merged. A configuration that does not
divide — an axis with no cut, a cut outside 1..=1000, cuts that do not increase
strictly — is refused.

Migration `0009` makes that durable: `proposal_batching_policy` and
`proposal_batching_cut` are append-only, and `proposal_review.thresholds_version`
is a foreign key to the policy, so the edges a proposal was banded under cannot
move after the fact and a review row cannot name a version nobody adopted.

## What binds a typed row to a signed event

Two things, the same two migration `0007` uses:

* `guard_proposal_review_authorized` refuses an insert whose `record_digest` is
  not the `source_digest` the `PROPOSAL_DISPOSED` event carries; and
* `proposal_review.proposal_id` is a foreign key to `proposal_disposition`, so a
  review row cannot exist without that event either.

The order matters for what a reader sees. A `BEFORE INSERT` trigger runs before
foreign keys are evaluated, so an unregistered proposal is refused by the
trigger — there is no row to match a digest against — and the foreign key is the
layer behind it rather than the one that speaks.
`the_batching_configuration_is_versioned_and_immutable` is where a foreign key
on this table is observed refusing on its own, against a proposal whose
registration and digest are both right.

## Migration numbering

`main` carries `0006` (consent) and `0007` (model-run). This is `0009`, not
`0008`. The branch was written while `P2-L1` was in flight on the same `main`
and might have taken `0008`; it landed with no migration, so **`0008` is
unclaimed and stays that way.** The gap is not closed, because a migration
number decides the order and nothing else rests on it: what the admission
fingerprint fixes is the resulting object set, read out of `sqlite_schema`
sorted by type and name. The next migration to land should be `0010` rather than
backfilling `0008`.

`0009` is in the encrypted lane's `STORE_MIGRATION_SQL`, so an encrypted profile
carries these tables from creation and admission fingerprints them. That set is
pinned as a whole — length and each element — by
`encrypted_profile_v2_is_created_only_by_cipher_lane`, which compiles only under
`sqlcipher-store`.

## What this is not

It is not an inference pipeline and it parses nothing. `P2-M1`
(`academic-model-run`) records what a model execution did and interprets its
confidence; `P2-G5` (`academic-untrusted-content`) validates a model output
against a schema, resolves its provenance, and produces the `Proposal` record a
caller wraps in a `Proposed` here. **Both are upstream by composition, not by
type.** This crate has an edge to neither, and nothing in this repository
observes that a caller performed the sequence. `Proposed<T>` is generic for that
reason: the rules here are about the tier, the disposition and the actor, none
of which depend on what the proposal says.

It is not `P2-M4`. Service-level refusal of automatic actors for question
resolution, mastery promotion, course and career decisions, permission
attestation, egress approval and deletion confirmation is that task's, in the
daemon command layer. What is here is the queue's own user-only door and the
receipt it takes.

It is not an authority resolver. Which claim wins when two disagree is
`P2-M3`'s, and this crate emits no claim.

It opens and closes no section 38 gate.

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`. Every proposal, decision, threshold and row in this
crate's tests is synthetic and built in process; the crate calls no provider and
no model, and its link closure holds nothing that can open a socket.
