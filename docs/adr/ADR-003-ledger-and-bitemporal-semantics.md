# ADR-003: Append-only ledger and bitemporal semantics

- Status: Accepted semantics; physical-store enforcement remains open

## Context

A UUID or wall-clock timestamp cannot simultaneously express device causality, replica knowledge order, and real-world applicability. Corrections must retain prediction and decision history instead of rewriting rows, and authority cannot be arrival-time last-write-wins.

## Decision

Every batch and claim keeps three independent coordinates:

1. `device_id + origin_seq`: authoring-device order and hash-chain continuity.
2. replica-local `accept_seq`: total order assigned only after verification and atomic acceptance.
3. half-open `valid_time [from,to)`: when a claim says it applies in the domain.

Claims, claim relations, user decisions, and resolution queries also carry one required registered `scope_id`. A user decision addresses an explicit semantic resolution slot (`subject_entity_id`, `predicate_id`, `scope_id`), the exact target object, and its own half-open user-controlled valid interval. Acceptance proves that target and replacement claims belong to that slot and that replacement changes the object. Acceptance rejects cross-scope relations/decisions, and resolution filters claims, relations, and decisions before applying policy, so two curricula, offerings, repositories, or project contexts cannot contaminate each other.

Event schema v2 is the current write contract for those decision semantics. The original signed event-schema-v1 fixture and Proto field layout remain byte/wire immutable; after authenticating original v1 bytes, the compatibility reader derives the missing slot, object, and valid interval only from the decision's immutable target claim in the same batch and rejects any missing-target or scope-mismatch case.

Canonical events, claims, evidence links, claim relations, and user decisions are INSERT-only. Corrections append a new assertion plus `SUPERSEDES`, `RETRACTS`, `CONTRADICTS`, or an explicit user decision. Current state is a resolver projection.

Authority and epistemic status are independent enums. Predicate policy—not arrival time—ranks official facts, user-owned state, direct implementation observation, curated relations, deterministic results, model inference, and prediction. Applicable decisions replay in acceptance order: rejects persist for the addressed object, confirmation reverses rejection only for that object, and replacement rejects A while selecting B. A decision contributes user-owned authority only for its exact semantic slot/object; it is not an early-return override of the predicate. Unaffected claims continue through `UserOwned`, `OfficialFact`, `ImplementationObservation`, or `CuratedRelation` ranking against the applicable decision's policy-specific user-authority rank, so an applicable official fact or direct implementation observation can remain active when a decision addresses a weaker unrelated object while lower automated alternatives do not silently reactivate. Their semantics survive regenerated claim IDs and adjacent valid-time handoffs.

State-removing `SUPERSEDES` and `RETRACTS` relations preserve their actor provenance and use a fail-closed source/target matrix. Both claims must have the same nonterminal authority/status pair owned by that actor class: user/user-confirmed, deterministic-engine/deterministic-derived, model-inference/AI-inferred, prediction/prediction, importer/official-confirmed, or importer/code-observed. Non-state-removing relations retain evidence semantics but cannot remove state. A relation can never remove a user-selected object, and automated actors cannot remove `USER_EXPLICIT`/`USER_CONFIRMED` state.

`SUPERSEDED` is lifecycle-terminal and always projects as rejected. `DISPUTED` is conflict-only and never becomes a sole active truth; corroborating and contrary nonterminal evidence retain the ordinary equal-rank rules.

## Executable evidence

- The ledger rejects origin gaps, parent-hash forks, batch-ID collisions, duplicate immutable IDs, and missing artifact/evidence/claim closure.
- Actor provenance is bound to signed device/user authorization and a fail-closed actor/authority/status matrix; model, importer, and deterministic-engine events cannot assert `USER_EXPLICIT`/`USER_CONFIRMED`.
- Equal-rank claims with different objects produce a conflict set and no false winner; same-object equal-rank claims may corroborate one active value.
- Narrow T007/T010 regressions cover regenerated IDs, every Confirm/Reject/Replace action under all four predicate policies, known/valid interval boundaries, stronger unrelated authority, adjacent A→B handoff, decision composition, replacement-slot mismatch, relation provenance/authorization, override protection, and sole/corroborating/conflicting lifecycle cases.
- Duplicate identical batches are idempotent and return the original acceptance range.
- Fourteen named bitemporal cases cover before-known, before-valid, user rejection, user confirmation, independent freshness windows, competing official claims, and later supersession.
- Final fixture replay keeps mastery `PRACTICED`, projects freshness `STALE`, rejects the earlier AI claim, exposes the later AI `FLUENT` claim as conflict, and selects the corrected official deadline.

## Acceptance gates still open

- SQL authorizer/trigger enforcement of append-only tables.
- batch tail anchoring in backup/paired-device receipts and explicit unanchored-tail status.
- a versioned predicate registry corpus with subject/object/cardinality and minimum-evidence validation.
- fuzz, resource limits, and at least twelve production-representative examples retained per supported schema version.

## Consequences

Queries require both `valid_at` and `known_at_accept_seq`; an ambiguous mutable “current claim” API is forbidden. UUIDv7 aids locality only and never establishes causality or canonical order.
