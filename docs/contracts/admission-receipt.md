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
3. Require exactly one valid signed row for every compiled platform, in this
   order: `windows-x86_64`, `windows-aarch64`, `linux-x86_64`,
   `linux-aarch64`, `macos-aarch64`.
4. Require spec digest
   `4830DEBD1A9EE8BE13B10D1E72BA3D2A3943F9D63417051CC123EF51743B2E45`,
   store schema version `2`, and the exact encrypted-profile-v2 marker.
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
capability. One compact canonical JSON byte sequence is emitted through:

- CLI JSON as the `policy` object and CLI human output as the `posture:` line;
- local IPC as `DataPosture.canonical_json`, alongside typed fields;
- exports as `posture.json`.

There is no desktop surface in this repository, so this contract does not claim
one. `posture_object_is_byte_exact_on_every_surface` compares the three present
surfaces. `no_environment_or_flag_override_exists` scans the product source for
key/override seams, pins the sole verified-capability and admitted-posture
construction sites, scans every other crate's product source for a second
admission-authority site, and recursively scans the Clap command tree. During
acceptance, an actual public arbitrary-key verifier was injected into the
product `AdmissionVerifier`; the named scan failed on that method, and passed
again after it was removed. The runtime tests additionally cover absent
receipts, missing rows, stale spec bytes, forged signatures, and
unprovisioned/empty/one-zero/all-zero acceptance keys.
