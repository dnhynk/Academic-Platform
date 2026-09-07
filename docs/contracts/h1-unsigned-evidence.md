# Unsigned H1 preparation evidence, version 1

`tools/h1-evidence.mjs` collects and validates **synthetic component evidence**.
It cannot create, sign or accept an admission receipt. `acceptedH1` and
`productionDataAllowed` are always false. The signed receipt v1 schema and
compiled acceptance key are unchanged. A successful five-job workflow is not
A5/A6/A7, final Phase 2 exit, independent security/licence approval or ADR-002
acceptance. No native keystore or physical hardware claim follows from a build.

The separate `h1-unsigned-evidence` workflow is manually dispatchable after it
exists on the default branch, and PR-triggered for its owned paths. It uses the
existing Windows x64/ARM64, Linux x64/ARM64 and Apple-silicon hosted runners.
It selects real `sqlcipher-store`, `encrypted-portability` and encrypted BK/RS
fault tests with default features disabled. Raw named-test output must contain
the required successful rows; ignored or zero-selected rows cannot satisfy them.
All ignored test observations are retained in each command's `tests` array.

## Bundle and validation

The fresh output directory contains `manifest.json`, a complete SHA-256/byte
inventory, `source.json` with every tracked source file and exact commit/tree,
the committed synthetic canary corpus, admitted dependency reference, copied
native licence bytes, command records and separate raw stdout/stderr logs.
The schema-2 probe binary is retained with its file size/hash; its synthetic
profile, wrapped recipient and DB/WAL/SHM/backup/crash scan files are retained.
Cargo's JSON compiler-artifact output binds the retained encrypted profile,
backup and crash test executables to their command records in `binaries.json`;
all four retained executable types have native architecture and byte hashes.
These are evidence executables, not release packages.

Command records include argv, executable, timestamps, exit code, signal/error,
derived status and all named test outcomes. Failed setup/builds retain diagnostics;
unreached commands are enumerated in `notRun`. Cancellation before collection or
upload can still prevent a bundle: the hosted job log/conclusion must then be
recorded as missing artifact evidence, never silently accepted.

Validation requires externally supplied expected repository/run/attempt, commit
and platform, plus the exact Git commit object in the validating checkout:

```text
node tools/h1-evidence.mjs validate BUNDLE PLATFORM COMMIT RUN_ID ATTEMPT REPOSITORY
node --test tools/h1-evidence.test.mjs
```

Retrieve using the existing authenticated GitHub CLI, preserve the artifact API
record and downloaded archive, and compare its SHA-256 with the API `digest`.
Then extract into a fresh directory and run the validator. An unsigned manifest
has no authenticity of its own: trusted hosted run identity, archive metadata,
source commit and command logs remain necessary provenance. The validator checks
the complete artifact inventory, hashes and sizes, source bytes against Git
objects, host/platform correspondence, actual binary architecture, command/plan
correspondence, required test results and category derivation. It independently
rescans the retained probe artifact bytes and reconciles file/byte/hit counts.
Limits are 10,000 files, 512 MiB per file and 1 GiB total, with 16 MiB per JSON
metadata file and 32 directory levels. The tree bounds and absence of symlinks
are checked before parsing metadata. References must resolve to the fixed source
record or an inventoried artifact. Symlinks, traversal,
unlisted files, substitutions and duplicates are rejected. A verified failed
bundle means its failure evidence is internally consistent; it does not pass
component checks. The CLI prints `integrity` separately from `status`.

SQLCipher and SQLite version values come from the running probe. OpenSSL's
version in `licenses.json` is `locked-source-not-runtime-provider`; an exact
runtime crypto-provider version remains unavailable. There are no dummy counts
or substitute digests. Canary observations apply only to the actual probe scan
files, not arbitrary process memory, crash dumps, every object format or package.

## Required H1 categories

Every manifest carries all eight category keys. Category status describes scope,
not an admission verdict, and links to command/artifact records including their
failures. `observed` records a bounded measurement, `partial` records incomplete
category coverage, `missing` identifies absent evidence, and `not_run` identifies
work this preparation does not execute.

| H1 category | Real artifact link and explicit boundary |
| --- | --- |
| `hosted_windows_phase2_exit` | `not_run`; encrypted components do not execute final Phase 2 exit |
| `hosted_linux_phase2_exit` | `not_run`; same boundary |
| `platform_build_and_license_receipt` | `partial`; source, native binary/build logs and native licence hashes; installers, signatures, updater, size budget and independent audit missing |
| `platform_zero_canary` | `observed` only when real probe output and independent retained-byte scan agree; otherwise `missing` |
| `platform_fault_and_restore` | `partial`; store rekey/DB tests, encrypted BK01–BK04/RS01–RS04 logs, named fresh-machine recovery and closure tests; full fault suite and physical power loss missing |
| `platform_keystore_native` | `missing`; Windows/Linux positive broker tests not run; macOS backend absent; TPM/Secure Enclave binding unverified |
| `five_platform_receipt_is_complete` | `missing`; no accepted signed five-row receipt, signer or public-key provisioning |
| `missing_platform_keeps_admission_denied` | `not_run`; no admission command executes in this preparation |

Future signing review may use actual build hashes, SQLCipher/SQLite observations,
canary counts and the raw fault/restore evidence digests as inputs only after all
required categories and missing provider observations are independently resolved.
This manifest is not a schema-v1 row conversion tool. Offline user-owned signing
custody, macOS native keystore implementation, Windows/Linux broker/hardware
observations, final packaged licence/size/update evidence and formal A7/H1 review
remain separate work. See [hosted dependency admission](../security/h1-hosted-dependency-admission.md).

The existing `fresh_machine_restore_with_phrase_only` test removes its synthetic
device-key fixture, recovers keys from a 256-bit recovery secret and verifies a
new empty destination's real encrypted database/object closure. It simulates
fresh-machine conditions on the same runner; it is neither a second physical
machine nor a 24-word phrase-codec test.
