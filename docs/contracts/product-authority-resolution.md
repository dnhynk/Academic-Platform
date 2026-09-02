# Product authority resolution

## Scope

This contract extends the Phase 1 ledger resolver with the six authority
questions in design section 30.3. It does not add a second implementation of
decision replay. `academic_ledger::resolve_product_snapshot` delegates scope
filtering, acceptance-order decision replay, `Confirm`/`Replace` rank floors,
state-removing relation authorization, terminal statuses, and equal-rank
conflicts to the same resolver body as `resolve_snapshot`.

The relative rank numbers are stable comparison values, not confidence scores:

| Authority class | Official academic fact | Personal intent | Mastery / question | Current implementation | Project intent | Relation / prerequisite |
|---|---:|---:|---:|---:|---:|---:|
| `OFFICIAL` | 800 | 600 | 350 | 400 | 750 | 700 |
| `USER_EXPLICIT` | 400 | 800 | 800 | 600 | 600 | 800 |
| `DIRECT_OBSERVATION` | 600 | 500 | 700 | 800 | 400 | 600 |
| `DETERMINISTIC_ENGINE` | 350 | 400 | 600 | 350 | 350 | 500 |
| `CURATED` | 500 | 600 | 400 | 400 | 800 | 800 |
| `MODEL_INFERENCE` | 200 | 200 | 200 | 200 | 200 | 200 |
| `PREDICTION` | 100 | 100 | 100 | 100 | 100 | 100 |
| `UNKNOWN` | 0 | 0 | 0 | 0 | 0 | 0 |

For relation/prerequisite claims, a corroborated model inference receives rank
500 after the source check below. A single-source or unestablished inference
stays at rank 200. Curated and user-confirmed relations remain above both.

“Latest” in the official-fact and project-intent rows is expressed by valid
intervals and explicit lifecycle relations. Replica arrival order does not
break equal-rank ties: different objects remain a conflict.

## Conflict vocabulary

`NEW_EVIDENCE_CONFLICT` is the sole emitted machine token. The older, longer
`NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE` spelling is accepted by
`ConflictReason::from_token` as a UI alias and normalizes to the canonical
value. A `ConflictCard` is returned when the Phase 1 result has conflicting
claims; no competing claim is overwritten.

An official academic fact and a personal applicability dispute therefore
coexist: the official claim may remain active while the `DISPUTED` claim is
conflict-only. Conversely, personal intent ranks a user decision above an
official or imported description. There is no global “user always wins” or
“official always wins” rule.

## Upstream-source identity and corroboration

Design section 30.3 ranks `corroborated inference` over `single-source
inference`; section 30.5 says copies of the same upstream source do not count as
independent corroboration. The design does not choose an identity key. This
implementation reuses the existing canonical upstream `source_digest` supplied
with provenance; it does not use an evidence artifact's `content_digest` and
does not introduce a second source digest.

The resolver applies these fail-closed rules:

1. Equal `source_digest` values are one upstream source, even when they arrive
   through two claims or paths.
2. A missing digest establishes no independence.
3. Different digest bytes alone establish no independence. Reformatting or
   re-collecting one upstream source can change bytes.
4. Promotion requires two present, unequal digests plus an explicit pairwise
   `SourceIndependenceAttestation`. The accepted basis labels are
   `DISTINCT_SIGNED_ORIGINS` and `SEPARATE_DIRECT_OBSERVATIONS`.
5. Duplicate or ambiguous provenance rows fail closed and produce a reason
   code instead of a promotion.

The attestation is an input fact from the provenance layer; this resolver checks
the pair and digest conditions but does not itself authenticate a signature or
reproduce a direct observation. In particular, it does not detect differently
formatted near-duplicates. It prevents those unknown cases from being promoted
and reports `INDEPENDENCE_UNESTABLISHED`. Content-safe near-duplicate metrics
belong to `P2-N1`, not this resolver.

Relation support is computed only from claims applicable to the query's exact
subject, predicate, scope, valid time, and known-at sequence. Claims in another
scope can coexist but cannot corroborate or promote one another.

## Lifecycle and open configuration

`SUPERSEDED` always projects as rejected and `DISPUTED` always projects as a
conflict. Neither can be active. Applicable decisions still replay by
acceptance sequence, and `Confirm`/`Replace` still apply
`max(original rank, user-decision rank)` to every claim supporting the selected
object. Automated state-removing relations still cannot remove that selected
object.

`GATE-38-025` remains open. A future configuration may decide whether and when
to recommend reconfirmation, but no setting in this milestone automatically
downgrades a user-confirmed state.

## Executable evidence

`crates/ledger/tests/product_authority.rs` contains the named acceptance cases:

- `authority_differs_by_claim_type`
- `ai_rerun_never_removes_an_override`
- `contrary_evidence_creates_a_conflict_card`
- `official_fact_is_not_mutated_by_user_dispute`
- `scoped_relations_coexist_without_promotion`
- `duplicated_upstream_sources_count_as_one_corroboration`
- `terminal_status_is_never_active`

The duplicate-source case covers equal digests, unequal digests without an
independence attestation, an absent digest, and the positive explicitly
attested path.
