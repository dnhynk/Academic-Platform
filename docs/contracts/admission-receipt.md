# Admission receipt and two-posture contract

## Current outcome

`P2-K6` installs the verifier and posture switch but does not open admission.
`ACCEPTANCE_PUBLIC_KEY` is compiled as `Unprovisioned`, and the committed
candidate receipt contains only `windows-x86_64` and `linux-x86_64` rows out of
the five required rows. Product verification therefore returns a denial,
`production_data_allowed` remains `false`, and every current surface emits the
unchanged synthetic posture. ADR-002 remains unaccepted; its default store lane
still reports `storage_encryption: NONE` and `adr_002_accepted: false`.

The signer is the user's own offline Ed25519 key (user decision, 2026-08-30).
The build system and CI are not signers: placing the private half in CI would
turn a public-repository pipeline secret into admission authority. The private
half is not stored in this repository. When the user supplies the public half,
provisioning changes the value of the single typed constant
`ACCEPTANCE_PUBLIC_KEY` from `Unprovisioned` to `Provisioned([u8; 32])`; receipt
verification has no runtime key argument.

## Signed deterministic-CBOR shape

The receipt at `<profile>/admission/receipt.cbor` uses the existing Ed25519
envelope profile:

```text
[
  envelope_version = 1,
  deterministic_payload_bytes,
  signer_public_key[32],
  ed25519_signature[64]
]
```

The signature covers the exact payload bytes. Both envelope and payload are
definite-length arrays, are decoded as one CBOR value with no trailing bytes,
and must round-trip to identical bytes through the pinned encoder. The payload
is:

```text
[
  receipt_version = 1,
  spec_digest[32],
  store_schema_version,
  [platform_row...]
]
```

Each signed platform row is:

```text
[
  platform_triple,
  row_spec_digest[32],
  row_store_schema_version,
  build_digest[32],
  sqlcipher_version,
  sqlite_version,
  crypto_provider_version,
  canary_file_count,
  canary_byte_count,
  canary_hit_count,
  fault_matrix_result_digest[32],
  independent_restore_digest[32]
]
```

The verifier requires nonzero digests, nonempty bounded ASCII version strings,
positive canary file/byte counts, and exactly zero canary hits. Row-level spec
and schema fields make stale rows reject independently of the outer receipt.

## Verification order

1. Read the bounded receipt from the fixed relative path and enforce canonical
   envelope bytes.
2. Require the envelope key to equal the sole compiled acceptance public key,
   then verify Ed25519 over the original payload bytes. An unprovisioned,
   malformed, wrong, or all-zero key denies.
3. Require exactly one valid signed row for every compiled platform:
   `windows-x86_64`, `windows-aarch64`, `linux-x86_64`, `linux-aarch64`,
   `macos-aarch64`. Rows may appear in the receipt in any order; the verifier
   matches them by name and emits the platform list in the compiled order.
4. Require spec digest
   `4830DEBD1A9EE8BE13B10D1E72BA3D2A3943F9D63417051CC123EF51743B2E45`,
   store schema version `2`, the exact encrypted-profile-v2 marker, the absence
   of the plaintext synthetic marker, and a profile store whose first sixteen
   bytes are not the SQLite format-3 header. The marker is a text file that can
   be copied into a plaintext profile, so it is a claim; the store header is
   what the verifier checks about the store itself.
5. Return an opaque `VerifiedAdmission`; every error maps the emitted posture
   to synthetic.

The first Windows and second Linux rows are committed as the signed synthetic
fixture `testdata/admission/incomplete-receipt.cbor.hex`. Its `cfg(test)` key is
not acceptance authority. It records the intended two-row denied state; the
three remaining platform rows belong to `P2-H1`.

## Posture surfaces

`Posture` has private state. Its admitted constructor consumes the opaque
`VerifiedAdmission`, and the compile-fail case in the existing
`academic-scenario` trybuild harness proves external code cannot fabricate that
capability with a struct literal. The remaining routes are closed by the type
system, and nothing executes them. One compact canonical JSON byte sequence is
emitted through:

- CLI JSON as the `policy` object and CLI human output as the `posture:` line;
- local IPC as `DataPosture.canonical_json`, alongside typed fields;
- exports as `posture.json`.

There is no desktop surface in this repository, so this contract does not claim
one. `posture_object_is_byte_exact_on_every_surface` compares the three present
surfaces.

`no_environment_or_flag_override_exists` reads every `*.rs` under every crate's
`src`, above that file's test module and refusing any file that declares an item
at file scope below it. In `crates/admission/src` it forbids a fixed list of
key and override seams — `std::env`, `env!(`, `env::var`, `debug_assertions`,
`include_bytes!`, `include_str!`, and four setter spellings. It pins the two
places the key is obtained as whole text rather than by token: the
`ACCEPTANCE_PUBLIC_KEY` declaration and the whole body of
`verify_with_compiled_acceptance_key`, both whitespace-collapsed against a
constant. Provisioning changes the declaration, so provisioning updates that
constant in the same commit. Six admission-authority tokens are counted against
an explicit allowance — the admission crate's exact count, every other crate
zero — which is what pins the sole verified-capability and admitted-posture
construction sites. It also recursively scans the Clap command tree.

The scan does not read the other crates for key seams; what it requires of them
is that they spell none of the six authority tokens.

`P2-RF7` put the five substitutions the `P2-K6` audit passed back through it,
one at a time, on Windows and Linux: a build environment variable through
`option_env!`, the same substitution spelling no forbidden token, a runtime key
file read inside the key check, a second module file beside `lib.rs`, product
code below `lib.rs`'s test module, and a `debug_assertions` bypass. Each failed
the scan and passed again after it was reverted. The runtime tests additionally
cover absent receipts, missing rows, stale spec bytes, forged signatures,
unprovisioned/empty/one-zero/all-zero acceptance keys, and a plaintext profile
carrying a copied format marker.

## What the receipt is not bound to

The signed payload names a spec digest, a store schema version, and one evidence
row per platform. It carries no profile identifier, store identity, nonce, or
expiry, so one valid receipt admits any profile on the machine that passes the
profile-format check, for as long as the compiled key stays provisioned. The
posture's `storage_mode` and `storage_encryption` are therefore not claims the
signature covers: what stands behind them is the format marker plus the store
header check in step 4, and both are properties of the profile the verifier was
pointed at, read at verification time.

Binding a receipt to a profile would change the signed payload shape, which is
frozen here and reproduced byte-for-byte by the committed fixture. `P2-H1` owns
the signing round that could change it.

`storage_schema` on the local IPC handshake is chosen by the posture through one
function, `academic_rpc::handshake::storage_schema_for`, which both the emitter
and the client validator call. The vault object formats are not chosen by the
posture: this build's vault reads and writes `PLAINTEXT_SYNTHETIC_V1` whatever
the posture says, because the vault that writes `AEAD_CHUNKED_V2` is the
non-default `aead-objects` feature and is not what a default-lane daemon links.
The admitted posture's `object_format` therefore describes the format admission
would require, not the format the running daemon uses.
