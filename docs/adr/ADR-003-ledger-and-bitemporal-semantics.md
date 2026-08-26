# ADR-003: Append-only ledger and bitemporal semantics

- Status: Accepted semantics; physical-store enforcement remains open

## Context

A UUID or wall-clock timestamp cannot simultaneously express device causality, replica knowledge order, and real-world applicability. Corrections must retain prediction and decision history instead of rewriting rows, and authority cannot be arrival-time last-write-wins.

## Decision

Every batch and claim keeps three independent coordinates:

1. `device_id + origin_seq`: authoring-device order and hash-chain continuity.
2. replica-local `accept_seq`: total order assigned only after verification and atomic acceptance.
3. half-open `valid_time [from,to)`: when a claim says it applies in the domain.

Claims, claim relations, user decisions, and resolution queries also carry one required registered `scope_id`. Acceptance rejects cross-scope relations/decisions, and resolution filters claims, relations, and decisions before applying policy, so two curricula, offerings, repositories, or project contexts cannot contaminate each other.

Canonical events, claims, evidence links, claim relations, and user decisions are INSERT-only. Corrections append a new assertion plus `SUPERSEDES`, `RETRACTS`, `CONTRADICTS`, or an explicit user decision. Current state is a resolver projection.

Authority and epistemic status are independent enums. Predicate policy—not arrival time—ranks official facts, user-owned state, direct implementation observation, curated relations, deterministic results, model inference, and prediction. Explicit user reject/confirm/replace decisions remain effective when later automated output arrives; contradictory evidence remains visible.

## Executable evidence

- The ledger rejects origin gaps, parent-hash forks, batch-ID collisions, duplicate immutable IDs, and missing artifact/evidence/claim closure.
- Actor provenance is bound to signed device/user authorization and a fail-closed actor/authority/status matrix; model, importer, and deterministic-engine events cannot assert `USER_EXPLICIT`/`USER_CONFIRMED`.
- Equal-rank claims with different objects produce a conflict set and no false winner; same-object equal-rank claims may corroborate one active value.
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
