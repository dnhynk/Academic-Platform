# Encrypted object format `AEAD_CHUNKED_V2`

The object format an encrypted profile writes. It is a non-default
`academic-vault` feature, `aead-objects`, built and tested separately from the
plaintext synthetic lane:

```powershell
cargo clippy -p academic-vault --all-targets --locked --offline --features aead-objects,phase2-fault-injection -- -D warnings
cargo test -p academic-vault --all-targets --locked --offline --features aead-objects,phase2-fault-injection
```

Enabling it is not acceptance of ADR-002 or ADR-004 and not permission to seal
a real byte. It changes what an object *is*; it does not change who may store
one.

## Byte layout

Every multi-byte integer is little-endian.

| offset | size | field |
|---:|---:|---|
| 0 | 4 | `"ACOB"` |
| 4 | 2 | `format_version = 2` |
| 6 | 2 | `header_len = 200` |
| 8 | 1 | `aead_id = 1` (XChaCha20-Poly1305) |
| 9 | 4 | `chunk_size` (default `1048576`) |
| 13 | 16 | `artifact_id` |
| 29 | 16 | `domain_id` |
| 45 | 1 | `retention_class` |
| 46 | 16 | `permission_lineage_id` |
| 62 | 24 | `base_nonce` — **end of the streaming prefix `P0`, 86 bytes** |
| 86 | 32 | `locator` (domain-keyed HMAC) |
| 118 | 8 | `plaintext_len` |
| 126 | 2 | `wrapped_dek_len = 80` — **end of the wrap AAD `P`, 128 bytes** |
| 128 | 80 | `wrapped_dek` — **end of the header, 208 bytes** |
| 208 | … | `chunk 0`, `chunk 1`, … |

```text
wrapped_dek := XChaCha20Poly1305(KEK_d, nonce = base_nonce, aad = P,
                                 plaintext = DEK(32) || plaintext_sha256(32))
             = 64 bytes ciphertext || 16 bytes Poly1305 tag
chunk_i     := XChaCha20Poly1305(DEK, nonce = base_nonce XOR LE64(i + 1),
                                 aad = SHA-256(P0) | LE64(i) | LE32(len_i) | u8 is_final,
                                 plaintext = plaintext[i * chunk_size ..])
```

`retention_class` is `EPHEMERAL = 1`, `COURSE_TERM = 2`, `USER_MANAGED = 3`,
`LEGAL_HOLD = 4`.

`header_len = 200` while the header is 208 bytes on disk: the field counts the
header **after** the eight-byte `"ACOB" | format_version | header_len` prefix.
Both numbers are committed as test vectors.

Four points a reader must not have to infer:

- **`header_tag` is not a field.** It is the trailing 16 bytes of
  `wrapped_dek`, which is the Poly1305 tag of the wrap above. Nothing else in
  the file is a tag over the header.
- **The chunk nonce XORs `LE64(i + 1)` into `base_nonce[16..24]`**, the
  trailing eight bytes, little-endian. Bytes `0..16` are never modified.
- **A zero-length object has exactly one chunk**, with `len_0 = 0` and
  `is_final = 1`, so every object has a final chunk whose finality is
  authenticated. An object whose length is an exact multiple of `chunk_size`
  has *no* trailing empty chunk: its last full chunk is the final one.
  `testdata/aead-chunked-v2/format-2-empty.aobj` is that object, frozen.
- **The header is written in two stages**, `P` then `wrapped_dek`. A writer
  that emits all 208 bytes in one call is not conforming: it collapses t068
  §7's `OB01` ("kill before header write") and `OB03` ("kill before header
  tag") into a single instant and makes `OB03` an unreachable execution path.
  With the split, `OB01` leaves an empty temp and `OB03` leaves a temp whose
  first 128 bytes are `P` and whose next 80 are still the reservation zeros.

## What each property is enforced by

| Property | Mechanism |
|---|---|
| wrong key, wrong domain | the wrap AAD is the whole cleartext header, which carries `domain_id`; it fails before any chunk is read |
| truncation | `plaintext_len` is inside the wrap AAD, the sealed length is checked against it, and `is_final` is chunk AAD |
| reorder | `LE64(i)` is chunk AAD |
| splice from another object | `SHA-256(P0)` is chunk AAD and `P0` carries a 24-byte per-object random nonce |
| header tampering | every identity field is inside the wrap AAD |
| plaintext substitution | the logical SHA-256 is sealed inside `wrapped_dek` and re-checked on read-back |

## Where this differs from t068 §3.4, and why

t068 §3.4 is a planning document and cannot be edited. Three of its lines are
not executable as written; each difference below is forced, not chosen.

1. **`base_nonce` is 24 bytes, not 12.** §3.4 fixes `aead_id = 1` to
   XChaCha20-Poly1305, whose nonce is 24 bytes. Widening the nonce rather than
   changing the AEAD also keeps the random per-object `base_nonce` safe under
   `KEK_d`, which is a long-lived per-domain key.

2. **The header is cleartext and the wrap seals `DEK || digest`, not "the
   header".** §3.4 writes `header_tag := AEAD(KEK_d, nonce=base_nonce,
   aad="ACOB"|version|header_len, plaintext = header)` with `base_nonce` and
   `wrapped_dek` inside that header. A reader must read `base_nonce` before it
   can run any `KEK_d` operation, an already-wrapped DEK cannot also be the
   plaintext that wraps it, and a second `KEK_d` operation under the same
   `base_nonce` would be nonce reuse. Sealing `DEK || plaintext_sha256` with
   the cleartext header as AAD is the one reading that keeps every property
   §3.4 asserts, including "the logical SHA-256 plaintext digest lives only
   inside the encrypted metadata".

3. **The chunk AAD hashes `P0`, not the whole header.** t068 §7 requires
   `OB01` ("kill after DEK generation, before header write") and `OB03` ("kill
   after final chunk, before header tag") to be reachable, so the header is
   written *after* the last chunk. The header carries `locator` and
   `plaintext_len`, neither of which exists until the stream ends, so a chunk
   AAD that hashed the finished header could not be computed while streaming.
   Nothing is lost: `P0` is a prefix of `P`, `P` is the wrap's AAD, and the
   wrap is verified before any chunk is read, so the header tag already
   authenticates `P0`.

`base_nonce` sits before `locator` for the same reason: `P0` has to be a
contiguous prefix.

## Keys

Every key comes from the ADR-005 schedule in `academic-crypto`; this lane
generates only the per-object DEK and base nonce.

```text
KEK_d = HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/kek/v1" || domain_id)
LOC_d = HKDF-SHA-512(KEK_d, salt = profile_id, info = "academic-os/vault-locator/v1")
```

An opened header holds a live DEK and the logical plaintext digest. Both are
private, both are reached only through a named `expose_dek` / `plaintext_digest`
borrow, both zeroize on drop, and `Debug` prints neither: a derived `Debug`
would put a live key into any log line, panic message, or audit row that
formatted a reader.

`LOC_d` is the HMAC key the domain-keyed locator is derived under. It is a
sub-derivation of `KEK_d`, not a fifth key off the Vault Master Key: a domain's
locator namespace cannot be computed by anyone who cannot already open that
domain's objects, and one key never serves as both an XChaCha20-Poly1305 key
and an HMAC key.

The locator derivation itself is unchanged from Phase 1 —
`HMAC-SHA-256(LOC_d, format_version || media_type || 0x00 || plaintext_digest)`
— so `artifact_descriptor` uniqueness stays
`(domain_id, retention_class, permission_lineage_id, vault_locator)` and
locators stay incomparable across domains. Global and convergent deduplication
remain rejected.

Changing the locator key source changes the locator, and therefore the path, of
every object in a domain. That is not a migration hazard here because the two
formats never meet inside one profile: format `1` is read only in a synthetic
profile, format `2` is written only in an encrypted one, and there is no
conversion between the two profile formats. A future change that let both
appear in one profile would have to answer this question again.

## Namespaces

```text
<profile>/vault/v1/<domain>/<retention>/<lineage>/<hh>/<hh>/<locator>.obj    PLAINTEXT_SYNTHETIC_V1
<profile>/vault/v2/<domain>/<retention>/<lineage>/<hh>/<hh>/<locator>.aobj   AEAD_CHUNKED_V2
```

A `VaultLayout` is bound to exactly one format for its whole life, so the
synthetic vault has no spelling for an encrypted object path and the encrypted
vault has none for a plaintext one. "Readers accept format 1 only inside a
synthetic profile" is therefore structural rather than a runtime check.
`vault/tmp`, `vault/leases`, and `vault/quarantine` are shared, and
reconciliation is one pass parameterised by namespace rather than two
implementations with two orphan policies.

## The store seam

`academic-store` depends on the `SealedObjectVerifier` and
`SealedObjectReceipt` traits, never on a concrete vault. Both vaults implement
them, so the encrypted lane gives the store no byte, hash, or path bypass and
no second acceptance path. Neither trait can mint evidence: the only
implementors are receipt types whose constructors are crate-private to
`academic-vault` and are reached only after an object has been read back from
its canonical policy-namespaced path.

## Re-sealing

`EncryptedVault::reseal` writes a **new** object, verifies its read-back, and
returns both the new receipt and the superseded descriptor. It never edits an
object in place and it does not move reachability: the caller appends a
descriptor-migration event, and only after that event commits may it quarantine
the superseded object. A termination between the two leaves the old object
reachable and the new one an unreferenced orphan, which reconciliation
quarantines after the grace window.

That event is the existing event schema v3 arm `RETENTION_ACTION_RECORDED`, and
what it authorizes is one appended `artifact_descriptor_migration` row naming
the superseded and the new locator — `artifact_descriptor` is INSERT-only and
its locator is inside the signed payload, so the reference moves by appending.
`academic_store::descriptor_migration` holds the resolution, and
[rotation and retention](rotation-and-retention.md) holds the ordering and the
retirement that ends the superseded object's readable life.

## Committed corpus

`testdata/aead-chunked-v2/` holds one object of each format, the zero-length
object, and the frozen byte vectors of the current format — including
`header_len_field`, `header_total_bytes`, both chunk nonces, and the
zero-length chunk AAD. `format_n_minus_1_reader_corpus` reads it and
never regenerates it, so a format change that moves a byte fails that test.
Regenerate only for a deliberate format change:

```powershell
cargo run -p academic-vault --features aead-objects --bin emit-object-corpus -- testdata/aead-chunked-v2
```

Every value in the corpus, including the domain KEK it is opened with, is
synthetic and disposable, and is committed on purpose: a corpus nobody can open
proves nothing.

## Faults

`OB01`–`OB09` of the t068 §7 matrix live in
`crates/vault/tests/encrypted_crash.rs`, behind the `phase2-fault-injection`
feature. `OB01`–`OB05` and `OB09` are process kills at a named failpoint;
`OB06`–`OB08` are injected corruption. A production build contains no
environment lookup and no crash switch, and the Phase 1 `V01`–`V06` family
cannot be fired by the Phase 2 feature or the reverse.

## What this is not evidence for

`ENCRYPTED_VAULT_FORMAT` reports `encrypted: true` and
`production_data_allowed: false`, and that pairing is the point: encrypting an
object is not permission to store a real one. That permission comes from
`P2-K6`'s verified admission receipt, which this crate neither reads nor can
construct.

It is not evidence that real data may be stored: `adr_002_accepted` is still
`false`, no admission receipt exists, and `P2-K6` has not shipped. It is not
five-platform evidence: the suite runs on every hosted Rust label, but macOS,
Windows ARM64, and Linux ARM64 remain `P2-H1`'s gate. It says nothing about
key custody, recovery-profile selection, or backup, which are `P2-K4`'s and
`P2-K5`'s.
