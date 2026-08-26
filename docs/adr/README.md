# Architecture decision register

The tracked LF bytes of the canonical design document have SHA-256 `4830DEBD1A9EE8BE13B10D1E72BA3D2A3943F9D63417051CC123EF51743B2E45`; `.gitattributes` fixes text checkout to LF and `pnpm verify:contracts` hashes the exact file bytes. The approved T002 architecture report has SHA-256 `A74768B811524FDCA8FEB0E1C63B87440BE626B182C04B22D00C7E39B5C6C469`. Those sources outrank this register if an accidental wording drift is found; a corrective ADR must then make the reconciliation explicit.

| ADR | Status | Phase 0 evidence | Remaining acceptance gate |
|---|---|---|---|
| [001](ADR-001-process-and-surface-authority.md) | Accepted architecture | Core admits only verified batches; CLI is headless | daemon singleton, authenticated IPC, client compatibility, Tauri capability/CSP |
| [002](ADR-002-encrypted-transactional-store-gate.md) | Proposed / blocked for real data | synthetic-only warning | SQLCipher five-OS packaging/license/leakage, fault, backup/restore, unsafe-location tests |
| [003](ADR-003-ledger-and-bitemporal-semantics.md) | Accepted semantics | append-only ledger, gap/fork checks, 14 time-travel examples | physical INSERT-only enforcement, anchoring, full predicate corpus |
| [004](ADR-004-artifact-vault-format.md) | Proposed | digest, keyed locator, exact evidence span | chunk AEAD, crash matrix, large/seek vectors, GC/migration |
| [005](ADR-005-key-hierarchy-and-recovery.md) | Proposed | no production key path | OS keystore, recovery, rewrap, revoke, irrecoverability UX |
| [006](ADR-006-policy-and-egress-broker.md) | Proposed | runtime egress absent | scoped token, preview, default-deny network, policy replay/audit |
| [007](ADR-007-pipeline-and-provider-sandbox.md) | Proposed | no worker runtime | OS containment, limits, malicious corpus, core-only output acceptance |
| [008](ADR-008-projection-and-retrieval.md) | Proposed | pure replay summary | full rebuild, generations, Korean/code corpus, domain isolation |
| [009](ADR-009-ipc-and-external-contracts.md) | Accepted Phase 0 profile | Proto, JSON Schema, deterministic CBOR, Rust/TS fixture | N-1 clients, Kotlin/Swift, fuzz/limits, IPC framing |
| [010](ADR-010-sync-and-device-conflict.md) | Proposed | device hash-chain semantics | offline merge, pairing/revoke, relay privacy, lost-device exercise |
| [011](ADR-011-monorepo-toolchain-ci-release.md) | Accepted baseline | pinned Cargo/pnpm, Windows/Linux CI | installer/signing/SBOM/updater negative tests |
| [012](ADR-012-migration-backup-restore-export.md) | Proposed | original signed bytes preserved | version fixtures, resumable migration, empty-target restore, export round-trip |

“Proposed” is not permission to ingest real data or ship a security claim. Acceptance evidence belongs in tests and reproducible reports, not only in prose.
