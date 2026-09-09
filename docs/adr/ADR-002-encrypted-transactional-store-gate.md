# ADR-002: Encrypted transactional store acceptance gate

- Status: Proposed; blocks real personal data
- Preferred candidate: SQLite semantics with SQLCipher, daemon-only writer

## Context

The canonical store needs transactions, foreign keys, bitemporal queries, append-only enforcement, backup, and local packaging. Plain SQLite is useful for throwaway semantic tests but is insufficient for grades, lecture metadata, private-code provenance, and decision history. Saying “encrypted at rest” is not enough if plaintext appears in WAL, SHM, temp, crash, or backup files.

## Proposed decision

Use a single local SQLite-family database per profile/domain model with one daemon writer, WAL, `synchronous=FULL`, foreign keys, and STRICT tables. SQLCipher is the preferred implementation only after the gate below passes. Canonical event/claim/evidence/decision tables must reject UPDATE and DELETE outside a signed maintenance migration. The database must reject network shares and consumer sync folders.

Until acceptance, the repository permits only in-memory or disposable plaintext state populated with synthetic fixtures. The product must display a production-data prohibition; the Phase 0 doctor exposes that prohibition in machine-readable output.

## Acceptance gate

1. Reproducible build, license, update, and binary-size matrix for supported Windows, macOS, and Linux architectures (five target combinations minimum).
2. Zero plaintext canary hits in DB, WAL, SHM, temp, backup, crash dump/artifacts, and migration intermediates.
3. Kill/power-loss/fault matrix across transaction, checkpoint, rekey, ingest reference, and migration boundaries.
4. Online backup followed by independent restore into a new empty profile.
5. Network/sync-folder detection and fail-closed behavior.
6. Wrong key, corrupt header, downgrade, old binary, locked key broker, and disk-full diagnostics.

## Rejected shortcuts

- treating OS full-disk encryption as the database encryption acceptance proof;
- letting desktop, CLI, or plugins open the file;
- storing real data temporarily in plaintext for a later migration;
- claiming acceptance from `PRAGMA cipher_version` without leakage and restore evidence.

## Consequences

Phase 1 may test schema/query semantics in a throwaway plaintext database, but no actual personal data may enter it. If SQLCipher fails the gate, a replacement must preserve ADR-003 semantics and ADR-012 export/restore independence.

### Bounded encrypted synthetic amendment (D3)

The [D3 contract](../contracts/encrypted-synthetic-domain-v1.md) permits an
explicit non-admitted encrypted description alongside the two byte-preserved
legacy postures. It names the existing SQLCipher schema 2 and AEAD format while
retaining synthetic-only policy, production permission false and network `NONE`.
This amends the old representation, not this real-data acceptance gate. The
opt-in RPC scaffold is locked and unavailable; absent D4 material means startup
still refuses before I/O even for an open keyed D1 session. Encryption alone
does not authenticate recognized synthetic sources or complete X4/H1.
