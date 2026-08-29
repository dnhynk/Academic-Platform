# Phase 1 SQLCipher acceptance spike (E1)

## Decision posture

This spike provides positive local Linux/WSL evidence for the already admitted
SQLCipher variant. It does **not** approve production encryption, production
data, a shipping key provider, a commercial-license choice, or ADR-002.

```text
adr_002_accepted=false
production_data_allowed=false
default storage_encryption=NONE
```

The SQLCipher executable is an evidence-only Cargo target with
`required-features = ["sqlcipher-spike"]`. The ordinary workspace and daemon
retain `bundled-sqlite`; asking Cargo to build `--bin sqlcipher_spike` without
the feature fails because the required feature is absent.

## Implementation boundary

Every create/open path constructs a raw `rusqlite::Connection` and applies
`PRAGMA key` as its first SQLite statement. New encrypted databases then call
`academic_store::migration::migrate_open_connection_pre_listen`, the same S1
migration implementation used by plaintext `migrate_pre_listen`. The shared
runner owns the exact migration bytes, FTS5 executable probe, connection
PRAGMAs, schema identity, shape, SQLite integrity and foreign-key checks. The
spike adds only cipher-setting/cipher-integrity checks and synthetic evidence
operations; it contains no copied S1 migration or schema-shape assumptions.

The shared runner and explicit `[[bin]]` target are narrow, coordinator-approved
exceptions to E1's initial file list. They do not change dependency features,
default features, the lockfiles, migration SQL, storage declarations, or daemon
linkage.

The successful read-back was:

| Property | Value |
| --- | --- |
| SQLCipher | `4.14.0 community` |
| SQLite | `3.51.3` |
| `cipher_page_size` | `4096` |
| `kdf_iter` | `256000` |
| HMAC | `HMAC_SHA512` |
| KDF | `PBKDF2_HMAC_SHA512` |
| `application_id` | `1094926660` (`ACAD`) |
| `user_version` | `1` |
| journal / durability | `WAL` / `FULL` (`2`) |
| foreign keys / trusted schema | `ON` / `OFF` |
| busy timeout / temp store | `250 ms` / `MEMORY` (`2`) |

## Evidence run

The accepted run used WSL2 Linux x86-64 with the pinned cache environment,
Rust/Cargo 1.98.0, Node 24.19.0 and pnpm 11.22.0. Cargo was always invoked with
`--locked --offline` and `CARGO_NET_OFFLINE=true`; no install, fetch, browser,
or advisory command contributed to the evidence receipt.

The seven required tests passed:

| Test | Evidence covered |
| --- | --- |
| `sqlcipher_feature_is_explicit` | Non-default feature, shared exact-S1 migration, cipher/SQLite version and crypto/operational PRAGMA read-back. |
| `sqlcipher_wrong_key_fails_closed` | Wrong synthetic key cannot read `schema_meta`; a copied database with 32 encrypted-header bytes corrupted also cannot read it. |
| `sqlcipher_plaintext_canary_absent_from_all_artifacts` | Five unique high-entropy synthetic canaries survive keyed reads and an independent restore; streaming raw-byte scans cover live/final DB, WAL, SHM, memory-temp exercise, online backup and crash copies. |
| `sqlcipher_wal_crash_recovers_or_fails_closed` | DB01-DB06 exit without commit and reopen with no rows or sequence consumption; DB07 exits after commit/before response and replays the exact stored receipt; a WAL cut 17 bytes mid-frame returns complete/old state or fails closed, never a partial canary set. |
| `sqlcipher_rekey_fault_has_one_documented_recovery_key` | A child is stopped after the pre-rewrite marker (or allowed to finish if already complete); exactly one of the old/new synthetic keys opens the resulting database. |
| `sqlcipher_online_backup_restores_empty_profile` | Online backup uses a distinct key, restore uses another distinct key and a new empty destination, restored canaries match, and reuse of a non-empty destination is rejected. |
| `plaintext_default_binary_has_no_cipher_claim` | Default feature remains `bundled-sqlite`, the SQLCipher target requires its explicit feature, the default daemon feature tree excludes SQLCipher/OpenSSL, and default RPC posture remains `NONE`/false. |

The standalone artifact run scanned 8 files / 1,365,968 bytes and found zero
plaintext canary occurrences. It restored all 5 canaries. The compiled evidence
binary was 36,392,344 bytes with SHA-256
`83de5591daec782f42f16d4a781ae6148c2725a4d0b6e5af2b534c34890cdc58`.
Local symbol inspection found `sqlcipher_version`, `sqlite3_key`,
`sqlite3_key_v2` and `sqlite3_rekey_v2` in that binary.
The run-specific external `t034-e1-evidence-final.json` receipt SHA-256 is
`0be00ecbad799de972655522a5be543e20bb3558f5ebf7eda02b5f9d32382561`.

## Admitted native sources

No new source resolution occurred. The run used only the frozen F0 admission:

| Component | Version/checksum or notice evidence |
| --- | --- |
| SQLCipher Community | `4.14.0`; BSD-style notice SHA-256 `ea4fcb309f14a22065e1ea45362d494d320012249ed865fe9c7c0946db754131` |
| Embedded SQLite | `3.51.3`; source id `2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6alt1` |
| `libsqlite3-sys` | `0.38.2`; crate checksum `f1d20bef17f513b9b3004532233187769cd072d790971f4e4da0e346eb6401e8` |
| OpenSSL | `3.6.3`; Apache-2.0 notice SHA-256 `7d5450cb2d142651b8afa315b5f238efc805dad827d91ba367d8516bc9d49e7a` |
| `openssl-src` | `300.6.1+3.6.3`; crate checksum `46eb8fb9fb3b61ce1c0f8a026c4c1a0714d3a9e138e7fbde78753ce2babc3846` |
| `openssl-sys` | `0.9.117`; crate checksum `b47e7e6bb2c38cd930d25a23b40fa52e068c10e85f3e03a7f5ba5aaca5713695` |

No `RUSTFLAGS`, `CFLAGS`, `CPPFLAGS`, or `LDFLAGS` were set for the accepted
run. The dependency-update owner remains the `academic-store` security/admission
lane. Legal/distribution acceptance and final notice packaging remain H1/ADR
work; the presence of admitted sources is not license approval.

## Limits and unresolved gates

- Native Windows evidence is unresolved. An exact Rust 1.98.0 locked/offline
  feature build was retried with the local Git-for-Windows Perl 5.42.2 path
  prepended. Cached `openssl-sys 0.9.117` / `openssl-src 300.6.1+3.6.3`
  reached `perl ./Configure ... VC-WIN64A`, then stopped because that Cygwin Perl
  lacks `Locale::Maketext::Simple` (loaded through `Params/Check.pm`). Nothing
  was fetched or installed. This is a concrete native-host toolchain limit, not
  a SQLCipher pass or an implementation blocker. `P2-K2` reproduced the same
  stop and enumerated the routes out of it; see
  [the encrypted store lane](../contracts/encrypted-store-lane.md).
- macOS, Android and iOS compile/runtime, packaging, symbol, crash-dump and
  artifact-leak evidence remain H1.
- Disk-full injection was not practical in this file-limited spike and no
  simulated pass is recorded here. `P2-K2` closed it for the encrypted store
  lane with bounded storage exhaustion at SQLite's own allocation boundary; see
  [the encrypted store lane](../contracts/encrypted-store-lane.md).
- Hardware-backed key brokering, production key lifecycle, migration from a
  future plaintext product store, update cadence, legal review and distribution
  notices remain out of scope.

## Excluded process event

After the accepted receipt was produced, a version-only diagnostic accidentally
ran without the required pinned `RUSTUP_HOME`/`CARGO_HOME` exports:

```text
wsl.exe -e bash -lc 'export PATH=<pinned paths>:$PATH; uname -a; rustc --version; cargo --version; node --version; pnpm --version'
info: syncing channel updates for 1.98.0-x86_64-unknown-linux-gnu
info: downloading 5 components
```

This violated the run's no-fetch process rule and is excluded from acceptance
evidence. It did not change repository files, manifests, lockfiles, migrations,
the already generated evidence receipt, or its artifacts. All subsequent
commands use explicit pinned `RUSTUP_HOME`, `CARGO_HOME`, target/cache paths and
locked/offline execution; the external completion report repeats this event.
