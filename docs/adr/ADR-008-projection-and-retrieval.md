# ADR-008: Projection and retrieval

- Status: Proposed decision register

## Registered direction

Graph, lexical search, vector metadata, and generated documents are disposable projections. Each generation records kind, schema version, builder binary digest, algorithm/model/tokenizer version, effective configuration hash, source `accept_seq` watermark, security domain, and build time. Builders write a new generation and atomically activate it only after coverage/checksum verification.

Initial graph storage is a relational adjacency projection; initial lexical search evaluates FTS5 tokenizers against Korean and code corpora; vector search begins exact/in-process until scale proves another backend necessary. No projection is exported or backed up as canonical truth.

Materialized time-travel snapshots are a projection under this same rule and live in their own sidecar (`application_id` `ACTL`), separate from the graph and lexical generations. Their tables carry no append-only trigger pair and are outside the canonical authorizer, because a snapshot that could not be deleted would have become a second ledger. Each snapshot records its projector version, binary digest, and configuration hash alongside the canonical input digest it was bound to, so a recomputation whose result differs while that input digest is equal is attributable to the projector and to nothing else.

## Implemented now

The pure replay summary is rebuilt from accepted events and produces a canonical semantic digest. Persistent disposable generations exist for the relational graph and for the two FTS5 lexical baselines, and a separate disposable sidecar holds materialized bitemporal snapshots. No vector or generated-document projection exists.

## Acceptance gate

Drop and rebuild from ledger only; as-known watermark equivalence; generation switch/rollback; Korean and code relevance metrics; security-domain isolation; and model/tokenizer changes creating a new generation.
