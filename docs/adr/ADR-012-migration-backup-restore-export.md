# ADR-012: Migration, backup, restore, and export

- Status: Proposed decision register

## Registered direction

Canonical schema migration is forward-only and transactional with preflight space/version checks, backup, integrity verification, and resume/repair state for large transformations. Historical signed event bytes and signatures are never rewritten; pure upcasters decode old payloads into current in-memory representations, while semantic corrections append new events.

Projection migrations build generations side-by-side and atomically switch an active pointer. Vault format readers support N and N-1 while writers emit N; re-encryption seals and verifies a new object before a descriptor migration event switches reachability.

Backup fixes a commit watermark and ledger heads, creates a consistent encrypted DB snapshot, closes all reachable object descriptors, verifies ciphertext/digests, and signs/encrypts a manifest with a recovery recipient independent of the live OS wrapper. Restore always targets a new empty profile and verifies keys, DB, ledger chains, object closure, replay, and counts before an atomic profile switch.

Full export is documented, versioned, and vendor-neutral: original artifacts or a documented encrypted bundle, entity/claim/event/evidence records, original signed envelopes, schemas/predicate registry, and a human-readable inventory. Projection export is optional.

## Implemented now

The original deterministic signed envelope is committed and verification rejects byte drift. Forward-only migration 0001 and its pre-listen admission exist; there is no backup, restore, or export implementation.

Admission is read-only and rejects before mutation. Identity, integrity, foreign keys, complete user-object emptiness at version zero, and an exact reference-derived structural fingerprint are all checked before the FTS5 probe, journal-mode configuration, exclusive locking, or a migration transaction. That fingerprint excludes only the tables and indexes SQLite itself creates — `sqlite_sequence`, `sqlite_stat1` through `sqlite_stat4`, and `sqlite_autoindex_*` — and covers every trigger and view unconditionally, because only `CREATE` applies SQLite's reserved-prefix rejection: a `sqlite_`-prefixed trigger or view written directly into `sqlite_schema` is loaded and fires like any other.

Rejection preserves durable content, not the whole file family. The main database keeps its exact bytes and an existing WAL keeps its committed frames, because read-write admission handles disable checkpoint-on-close; otherwise closing a rejected handle would checkpoint a tampered WAL into the main database. SQLite's own read and recovery path may still create an empty `-wal` and create or rewrite the rebuildable `-shm`; neither carries committed content, so both are outside the claim. Rollback-journal-mode inputs keep the whole family byte-identical.

## Acceptance gate

Fixture for each supported schema version; interrupted large migration resume; restore only to empty destination; object/ledger/key closure; independent fresh-profile restore; and vendor-neutral export/import round trip.
