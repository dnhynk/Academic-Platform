# Phase 1 synthetic-only local-core contract

Phase 1 implements a synthetic, throwaway local core behind the frozen package graph and safety vocabulary. It creates a plaintext profile with migrations, canonical tables and vault objects; serves a current-user local listener; accepts only the allowlisted deterministic signed fixture; and provides disposable projections, deterministic export, consistent plaintext backup, verified empty-target restore, and a report-only fault surface. There is no arbitrary import path or user-accessible fault switch.

## Unavoidable posture

Every data-bearing human surface prints this line before results:

```text
PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN
```

Every machine surface and local handshake carries this exact object:

```json
{
  "data_policy": "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
  "storage_mode": "PLAINTEXT_TEMPORARY_SQLITE",
  "storage_encryption": "NONE",
  "production_data_allowed": false,
  "product_network": "NONE"
}
```

There is no quiet mode, environment bypass, real-data override, arbitrary importer, production profile, or conversion command. Every created profile also contains `SYNTHETIC_ONLY_PLAINTEXT_DO_NOT_USE_REAL_DATA.txt`.

## Input admission

The only Phase 1 manifest contract is `schemas/jsonschema/synthetic-ingest-manifest-v1.schema.json`. Its current allowlist binds the tracked `signed-batch-v2.json` bytes by ID, schema version, relative path, byte length, SHA-256, synthetic data class, and `network_egress=NONE`. Passing JSON Schema is only a boundary check: the runtime independently compares the exact builder identity and digest allowlist before it issues any verified capability.

Personal or production grades, transcripts, lecture media, repositories, questions, credentials, documents, recordings, exports, and network payloads are forbidden. Phase 1 has no arbitrary input path that can accept them; ingest accepts only the named compile-time allowlist entry.

## Frozen names and values

| Contract | Frozen value |
|---|---|
| SQLite application ID | `0x41434144` (`ACAD`) |
| store schema | integer `1`, semantic `1.0.0` |
| SQLite busy timeout | `250 ms` |
| daemon binary | `academicd` |
| writer queue capacity | `64` |
| Protobuf package | `academic.v1` |
| local protocol | `learning-platform.local-core` major `1`, minor `0` |
| frame prefix | four-byte big-endian unsigned length |
| handshake/command caps | `64 KiB` / `8 MiB` |
| vault format | `PLAINTEXT_SYNTHETIC_V1` |
| default storage feature | `bundled-sqlite` |
| SQLCipher spike feature | `sqlcipher-spike` |
| test-only fault feature | `phase1-fault-injection` |

Capability IDs use product-neutral learning-platform wording and are fixed in `academic-rpc`. Fault IDs are exactly `V01`–`V06`, `DB01`–`DB07`, `PR01`–`PR03`, `BK01`–`BK04`, `RS01`–`RS04`, and `IPC01`–`IPC02` in `academic-test-support`.

## Storage and networking boundary

The `academic-store` default feature compiles rusqlite with its bundled plaintext SQLite source. `sqlcipher-spike` is non-default and is valid only with an explicit feature selection for later compile/evidence work. Its existence and successful compilation do not mean SQLCipher packaging, leakage, crash, recovery, licensing, or ADR-002 acceptance passed.

Tokio's admitted `net` feature serves current-user named pipes and protected Unix-domain sockets. Product source has no HTTP, TCP, UDP, DNS, TLS, cloud SDK, connector, or provider behavior; policy tests admit only the named-pipe/UDS local-IPC exception.

## Crate direction

```text
academic-domain
  -> academic-contracts
  -> academic-ledger
  -> academic-store
       -> academic-vault
       -> academic-projections
            -> academic-portability

academic-domain + academic-contracts -> academic-rpc
academic-domain -> academic-scenario

store + vault + rpc + projections + portability -> academic-core
academic-core -> academic-daemon / academic-cli

academic-test-support is test-only and no product crate depends on it.
```

The arrows mean “may be depended on by the item to the right.” The checked policy uses the actual Cargo metadata graph, rejects cycles and upward edges, and prevents the daemon from becoming a dependency of the store or any canonical layer.

`academic-scenario` is placed off `academic-domain` and nowhere else on purpose. It holds every projected what-if value, and the absence of an edge to `academic-store` is what makes a projected mastery, opportunity, or workload value unable to reach an actual-state write: the writer is not in the dependency closure a projection is compiled against, so nothing there can name it. `scenario_crate_has_no_writer_dependency` asserts that from the Cargo metadata graph across normal, build, and dev edges, and `academic-core` links the crate by a dev edge only, so that `tests/scenario_isolation.rs` can hold the projection engine and the canonical writer in one process and prove the canonical state digest is unchanged after a fuzz.

## Current boundary and open gates

The synthetic throwaway Phase 1 local core passed at commit `9347a2a99cab0be2729349ecd2ff5ad50afd13b7`; encrypted-at-rest and production-data gates remain open. Functional daemon IPC, plaintext persistence, projections, portability, path-safety checks, current-user transport protections, and Windows and Linux crash/exit evidence exist for that boundary. ADR-002 remains unaccepted with `adr_002_accepted=false`, `storage_encryption=NONE`, and `production_data_allowed=false`; ADR-004 encrypted objects, ADR-005 production key hierarchy, the complete five-platform SQLCipher matrix, and macOS/Android/iOS `phase1-exit` evidence remain open. The SQLCipher spike is non-default evidence and does not authorize real data.
