# ADR-008: Projection and retrieval

- Status: Proposed decision register

## Registered direction

Graph, lexical search, vector metadata, and generated documents are disposable projections. Each generation records kind, schema version, builder binary digest, algorithm/model/tokenizer version, effective configuration hash, source `accept_seq` watermark, security domain, and build time. Builders write a new generation and atomically activate it only after coverage/checksum verification.

Initial graph storage is a relational adjacency projection; initial lexical search evaluates FTS5 tokenizers against Korean and code corpora; vector search begins exact/in-process until scale proves another backend necessary. No projection is exported or backed up as canonical truth.

## Implemented now

The pure replay summary is rebuilt from accepted events and produces a canonical semantic digest. No persistent search, graph, vector, or document projection exists.

## Acceptance gate

Drop and rebuild from ledger only; as-known watermark equivalence; generation switch/rollback; Korean and code relevance metrics; security-domain isolation; and model/tokenizer changes creating a new generation.
