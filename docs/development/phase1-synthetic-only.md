# Phase 1 F0 synthetic-only contract

Phase 1 F0 freezes a compileable local-core package graph and its safety vocabulary. It does not create a profile, database, table, migration, object, listener, import path, export, backup, restore, or fault switch. The existing deterministic signed fixtures remain the only data-bearing executable inputs in the repository.

## Unavoidable posture

Every later data-bearing human surface must print this line before results:

```text
PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN
```

Every later machine surface and local handshake must carry this exact object:

```json
{
  "data_policy": "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
  "storage_mode": "PLAINTEXT_TEMPORARY_SQLITE",
  "storage_encryption": "NONE",
  "production_data_allowed": false,
  "product_network": "NONE"
}
```

There is no quiet mode, environment bypass, real-data override, arbitrary importer, production profile, or conversion command. A future profile must also contain `SYNTHETIC_ONLY_PLAINTEXT_DO_NOT_USE_REAL_DATA.txt`.

## Input admission

The only Phase 1 manifest contract is `schemas/jsonschema/synthetic-ingest-manifest-v1.schema.json`. Its current allowlist binds the tracked `signed-batch-v2.json` bytes by ID, schema version, relative path, byte length, SHA-256, synthetic data class, and `network_egress=NONE`. Passing JSON Schema is only a boundary check: the later runtime must independently compare the exact builder identity and digest allowlist before it issues any verified capability.

Personal or production grades, transcripts, lecture media, repositories, questions, credentials, documents, recordings, exports, and network payloads are forbidden. F0 contains no path that could accept them.

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

Tokio's admitted `net` feature is reserved for current-user named pipes and protected Unix-domain sockets. F0 creates neither. Product source has no HTTP, TCP, UDP, DNS, TLS, cloud SDK, listener, connector, or provider behavior; the policy test treats only the future named-pipe/UDS boundary as an allowed local-IPC exception.

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

store + vault + rpc + projections + portability -> academic-core
academic-core -> academic-daemon / academic-cli

academic-test-support is test-only and no product crate depends on it.
```

The arrows mean “may be depended on by the item to the right.” The checked policy uses the actual Cargo metadata graph, rejects cycles and upward edges, and prevents the daemon from becoming a dependency of the store or any canonical layer.

## Still open

ADR-002 encrypted storage, ADR-004 encrypted object format, ADR-005 key hierarchy, functional daemon/IPC, persistence, projections, portability, path safety, process security, crash evidence, Linux native evidence, and every production-data gate remain open. The only accurate completion claim for F0 is that contracts, feature names, dependencies, and empty crate boundaries compile under the pinned tools.
