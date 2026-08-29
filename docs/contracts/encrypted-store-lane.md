# Encrypted store lane (`sqlcipher-store`)

## Posture

Building this lane is not acceptance of ADR-002 and not permission to ingest a
real byte. The encrypted profile is a *format*; whether a running profile may
hold real data is decided by the `P2-K6` admission verifier at runtime, and
until that verifier passes the daemon serves the synthetic posture.

```text
adr_002_accepted=false
production_data_allowed=false
default storage_encryption=NONE
```

## The two lanes are mutually exclusive

`academic-store` keeps `default = ["bundled-sqlite"]`. The encrypted lane is the
non-default `sqlcipher-store` feature, and enabling both is a compile error:

```
error: `bundled-sqlite` and `sqlcipher-store` are mutually exclusive store lanes
  --> crates/store/src/lib.rs
```

So the plaintext synthetic lane and the encrypted lane cannot link into one
binary. Everything that creates, opens, or declares a plaintext synthetic
profile — `create_synthetic_profile`, `open_synthetic_profile`,
`write_policy_banner`, `SyntheticIngestManifest`, the plaintext posture strings
— is compiled out under `sqlcipher-store`, and `cipher` is compiled out without
it. Build the encrypted lane as:

```sh
cargo test -p academic-store --no-default-features --features sqlcipher-store --locked --offline
cargo clippy -p academic-store --no-default-features --features sqlcipher-store --all-targets --locked -- -D warnings
```

## Profile

`P2-K2` owns these entries of the section 3.2 layout; the rest belong to
`P2-K3`–`P2-K6`.

```text
<profile>/
  PROFILE_FORMAT_V2              # format UUID + schema version, exact bytes
  academic-platform.sqlite3      # SQLCipher, key applied before first page access
  .academic-profile-incomplete   # bootstrap marker, removed last
```

`PROFILE_FORMAT_V2` and `SYNTHETIC_ONLY_PLAINTEXT_DO_NOT_USE_REAL_DATA.txt` are
mutually exclusive. Each lane recognises the other's marker and refuses a
profile carrying both; the plaintext side raises `InvalidProfileState`, the
encrypted side `ConflictingProfileFormat`.

The Phase 1 path policy applies unchanged: empty root, local, owner-only,
outside any Git worktree, no UNC/device/remote/reparse path, no consumer sync
root. Creation and opening both validate it.

## Frozen schema-2 identity

Migration `migrations/store/0003_phase2_encrypted_identity.sql` runs
immediately after `0001_phase1_core.sql`, inside the same exclusive creation
transaction, against a database proven empty. It replaces the Phase 1 identity
singleton; every canonical table, index, and append-only trigger from `0001` is
carried forward unchanged.

| Field | Value | Pinned by |
| --- | --- | --- |
| format UUID | `67cb6d3ea27e4b53b1e727d46920e4f9` | `0003` column `CHECK` **and** `STORE_FORMAT_UUID` |
| `schema_version` / `user_version` | `2` | `0003` `CHECK`, `PRAGMA user_version` |
| `schema_semver` | `2.0.0` | `0003` `CHECK` |
| minimum reader protocol | `2.0` | `0003` `CHECK` |
| minimum writer protocol | `2.0` | `0003` `CHECK` |
| `data_policy` | `REAL_PERSONAL_DATA_PERMITTED` | `0003` `CHECK` |
| `storage_mode` | `SQLCIPHER_ENCRYPTED_PROFILE_V2` | `0003` `CHECK` |
| `storage_encryption` | `SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000` | `0003` `CHECK` |
| `application_id` | `1094926660` (`ACAD`) | unchanged from schema 1 |

`production_data_allowed` and `product_network` are **absent** from the
schema-2 singleton. They are the admission verifier's runtime output, not a
stored column: an encrypted profile without a receipt still serves the synthetic
posture, so the posture is never read from this singleton. Freezing either value
in a `CHECK` here would state an admission decision `P2-K6` has not made.

### No conversion from schema 1

There is no code path that turns a Phase 1 profile into a schema-2 one, and the
absence is executable rather than asserted. The only entry point that can write
a schema-2 identity admits an empty database or an exactly-current one; a
schema-1 database is neither, so it is refused with
`UnsupportedMigrationState { application_id, user_version: 1 }` and its bytes
are left untouched. Separately, the Phase 1 singleton's own `CHECK`s make a
schema-2 row physically unstorable, and the schema-2 singleton's `CHECK`s make a
schema-1 row unstorable.

## Key

`SKEY_p` comes from `academic-crypto` (`P2-K1`) and is never derived a second
time here:

```text
SKEY_p = HKDF-SHA-512(VMK, salt=profile_id, info="academic-os/store/v1")
```

It is supplied as a **raw 32-byte key**, `PRAGMA key = "x'<64 hex>'"`, never as
a passphrase, and it is the first SQLite statement issued on every handle —
creation, writer, and reader alike. The rendered hex lives in a zeroizing buffer
for the length of the call. `open_reader` (unkeyed) is compiled out of this
lane, so no call site can reach an encrypted database without a key.

A key that does not authenticate page one produces `EncryptedStoreLocked` and
nothing else. The reason string is identical for a wrong key and for a destroyed
page one, because distinguishing them would tell a caller something about the
key.

## Cipher settings, read back at every open

Asserted on every create and every open, not once at creation:

| Setting | Required |
| --- | --- |
| `cipher_version` | major version `4` |
| `cipher_page_size` | `4096` |
| `kdf_iter` | `256000` |
| `cipher_hmac_algorithm` | `HMAC_SHA512` |
| `cipher_kdf_algorithm` | `PBKDF2_HMAC_SHA512` |

The library pins the cryptography that `storage_encryption` names, not the patch
level: a patch pin would refuse an identical SQLCipher 4 build for no security
reason. The acceptance tests pin the exact observed build instead, so a
toolchain move is caught and has to be recorded deliberately.

Observed on the Linux evidence host: SQLCipher `4.14.0 community` over SQLite
`3.51.3`.

## Fault coverage

`EN01`–`EN06`, and `DB01`–`DB07` re-run under encryption. Failpoints live only
in `crates/store/src/bin/sqlcipher_store_probe.rs`, which cannot compile without
the non-default feature; the library contains no environment lookup and no crash
switch.

| Fault | Outcome proved |
| --- | --- |
| `EN01` kill mid store rekey | exactly one of the old and new keys opens the database |
| `EN02` wrong store key | locked, `schema_meta` unreadable, no weaker key tried |
| `EN03` corrupt cipher header | `cipher_integrity_check` names the exact page; a destroyed page one leaves the profile locked |
| `EN04` write-ahead log truncated mid-frame | complete old state or locked, never a partial canonical row set |
| `EN05` cipher downgrade or older identity | a SQLCipher 3 compatibility handle cannot read the database; a schema identity pushed back to version 1 is refused with a version reason |
| `EN06` storage exhausted during commit and checkpoint | transaction aborts, no partial commit, actionable error, every page still authenticates |
| `DB01`–`DB07` | a kill before commit leaves nothing visible; `DB07` (after commit) is durable; the database still authenticates in every case |

`EN06` bounds the database with `PRAGMA max_page_count` at its current size, so
every further page allocation fails at SQLite's own storage boundary with
`SQLITE_FULL`. That is deterministic, needs no privileged filesystem, and runs
identically on both hosts. It does not exercise an operating-system `ENOSPC`
return, which remains a host-level check.

## Canary scan

`sqlcipher_store_probe run <workdir>` writes the committed
`testdata/sqlcipher-canary/store-v2-canaries.txt` corpus into the canonical
store and a memory temp table, produces an encrypted backup under an independent
key, kills a child with committed frames still in the write-ahead log, and
streams every resulting artifact. It prints a receipt with the read-back cipher
settings, the schema-2 identity, file and byte counts, and the hit count, which
must be zero. The scanner's own sensitivity is proved in the test by planting a
plaintext canary and observing it found.

## Native Windows

The encrypted lane has not been built natively on Windows. `openssl-src` runs
`perl Configure VC-WIN64A`, and the only Perl on the evidence machine is
Git-for-Windows' Cygwin Perl 5.42.2, which omits `Locale::Maketext::Simple`
(reached through `Params/Check.pm` → `IPC/Cmd.pm` → `OpenSSL/config.pm` →
`Configure`). `libsqlite3-sys` offers exactly two Windows routes — vendored
OpenSSL, or a prebuilt OpenSSL named by `OPENSSL_DIR` — and both need software
the machine does not have; `SQLCIPHER_CRYPTO_CC` is Apple-only, so there is no
CNG path.

This is **awaiting a user decision on installing a pinned Windows Perl**, not a
closed `NOT_RUN`: a working route exists and only approval is missing. Vendoring
the missing pure-Perl modules and pointing `PERL5LIB` at them is the ad-hoc
workaround t068 section 8.2 rejects, and a Cygwin Perl still emits `/d/…` paths
that `nmake` cannot consume.

## Still open

ADR-002 is not accepted. `production_data_allowed` stays false, the admission
receipt does not exist, five-platform SQLCipher evidence does not exist, and no
recovery profile has been selected (`GATE-38-031`).
