# ADR-003: Append-only ledger and bitemporal semantics

- Status: Accepted semantics; physical-store enforcement remains open

## Context

A UUID or wall-clock timestamp cannot simultaneously express device causality, replica knowledge order, and real-world applicability. Corrections must retain prediction and decision history instead of rewriting rows, and authority cannot be arrival-time last-write-wins.

## Decision

Every batch and claim keeps three independent coordinates:

1. `device_id + origin_seq`: authoring-device order and hash-chain continuity.
2. replica-local `accept_seq`: total order assigned only after verification and atomic acceptance.
3. half-open `valid_time [from,to)`: when a claim says it applies in the domain.

Canonical events, claims, evidence links, claim relations, and user decisions are INSERT-only. Corrections append a new assertion plus `SUPERSEDES`, `RETRACTS`, `CONTRADICTS`, or an explicit user decision. Current state is a resolver projection.

Authority and epistemic status are independent enums. Predicate policy—not arrival time—ranks official facts, user-owned state, direct implementation observation, curated relations, deterministic results, model inference, and prediction. Explicit user reject/confirm/replace decisions remain effective when later automated output arrives; contradictory evidence remains visible.

## Executable evidence

- The ledger rejects origin gaps, parent-hash forks, batch-ID collisions, duplicate immutable IDs, and missing artifact/evidence/claim closure.
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
