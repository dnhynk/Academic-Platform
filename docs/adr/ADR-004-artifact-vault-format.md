# ADR-004: Artifact vault format

- Status: Proposed decision register

## Registered direction

Artifact identity is SHA-256 over exact plaintext bytes plus descriptor metadata (`media_type`, length, encryption domain, retention class, permission lineage, format version). The logical digest is stored only inside encrypted metadata. A physical locator is `HMAC-SHA-256(domain_locator_key, format_version || media_type || 0x00 || plaintext_digest)` in a domain namespace; a locked directory listing must not expose global plaintext equality.

Deduplication is permitted only when encryption domain, retention class, and permission lineage all match. Global/convergent deduplication is rejected. Evidence locators include the source digest and exact byte/time/page/repository coordinate so a changed source cannot silently satisfy an old claim.

The object format is `AEAD_CHUNKED_V2`: random per-artifact DEKs, domain KEK wrapping, a versioned header, independently authenticated chunks, and AAD binding object identity and chunk position. The AEAD is XChaCha20-Poly1305 (`aead_id = 1`) and the default chunk size is `1048576`, seekable by chunk index. The complete byte layout is [the encrypted object format](../contracts/encrypted-object-format.md); it is the contract, and it differs from t068 section 3.4 in three places that section could not have executed.

Those three differences are forced, not chosen, and a later implementer reading the plan rather than this register would reintroduce all three:

1. `base_nonce` is **24 bytes**, not the 12 the plan writes, because `aead_id = 1` is XChaCha20-Poly1305.
2. The header is stored in cleartext and the `KEK_d` operation seals `DEK || plaintext_sha256` with the whole cleartext header as AAD. The plan's `plaintext = header` cannot run: a reader must read `base_nonce` before any `KEK_d` operation and `base_nonce` is inside that header, an already-wrapped DEK cannot also be the plaintext that wraps it, and a second `KEK_d` operation under the same nonce would be nonce reuse. `header_tag` is consequently not a separate field; it is the trailing 16 bytes of `wrapped_dek`.
3. The chunk AAD hashes the header's **streaming prefix** `P0` — everything through `base_nonce` — rather than the finished header. The fault matrix requires a kill point after the last chunk and before the header tag, so the header is written last, and the header carries `locator` and `plaintext_len`, neither of which exists until the stream ends. `P0` is a prefix of the wrap's AAD and the wrap is verified before any chunk is read, so the header tag already authenticates it.

`header_len` is `200` while the header occupies 208 bytes: the field counts the header after the eight-byte `"ACOB" | format_version | header_len` prefix. The header is written in two stages, the cleartext prefix `P` and then `wrapped_dek`; a single-call header writer is not conforming, because it collapses the fault matrix's `OB01` and `OB03` into one instant and makes `OB03` unreachable. The chunk nonce XORs `LE64(i + 1)` into `base_nonce[16..24]`, the trailing eight bytes, little-endian. A zero-length object is exactly one chunk with `len_0 = 0` and `is_final = 1`. All four are fixed by committed test vectors under `testdata/aead-chunked-v2/`.

The domain locator key is `HKDF-SHA-512(KEK_d, salt = profile_id, info = "academic-os/vault-locator/v1")`, a sub-derivation of the domain KEK rather than a fifth key off the Vault Master Key, so no single key is both an XChaCha20-Poly1305 key and an HMAC key. The locator derivation itself is unchanged, so descriptor uniqueness and cross-domain incomparability are unchanged.

## Implemented now

Algorithm-prefixed digest and keyed locator newtypes, domain-key separation tests, byte-length/media metadata, and exact evidence locator validation are implemented. A complete `TEXT_BYTES 0..artifact.byte_length` representation must use the artifact content digest for both source and representation, cryptographically closing the excerpt to the registered bytes. Partial text, page, transcript-time, and repository representations remain valid descriptor vocabulary, but Phase 0 event/evidence acceptance fails closed because no byte-resolving verifier capability exists; an actor label alone is not trusted proof of resolved bytes. Evidence acceptance also enforces artifact/event domain closure and never compares two caller-controlled digests as proof. The encrypted object writer exists behind the non-default `academic-vault` feature `aead-objects`. It keeps the Phase 1 crash-safe publish sequence, the domain-keyed locator namespace, and exact-policy deduplication unchanged, and adds the header and chunk layout above, a seekable reader, and re-sealing that writes a new object rather than editing one in place. `academic-store` reaches it only through the `SealedObjectVerifier`/`SealedObjectReceipt` traits, which both vaults implement, so the encrypted lane grants no byte, hash, or path bypass. The two object namespaces are separate physical trees, `vault/v1/*.obj` and `vault/v2/*.aobj`, and a layout is bound to one of them for its whole life, so a synthetic vault has no spelling for an encrypted object path.

The artifact JSON boundary first parses the raw text with unique decoded property names, Unicode-scalar-only strings, and canonical unsigned integer lexemes. It then rejects unsafe integers and nonportable paths at schema level and executes a semantic post-validator for ranges, artifact bounds, span lengths, source/full-range digest binding, and locator-identity uniqueness. Rust and Ajv/TypeScript run the same committed structured and exact-raw mutation corpus, including duplicate names, lone surrogates, positive text/page/time/repository Unicode descriptors, and unknown properties at the descriptor, representation, and locator levels. Mutation checks independently require Rust's descriptor and representation structs to retain closed-object deserialization.

The synthetic Phase 1 vault also binds object liveness to Store transaction lifetime. A sealed
capability owns a live no-follow object handle, its exact host file identity, and a shared lease in
the policy-namespaced `vault/leases/v1` tree. Ingest and verification acquire that shared lease;
product-controlled quarantine, removal, and replacement must acquire the corresponding exclusive
lease. Store re-hashes the retained handle and reopens/re-hashes the canonical path under that same
lease immediately before every successful new, duplicate, or idempotent commit, rejecting missing,
truncated, replaced, or identity-changed objects without a new durable receipt.

One `Vault` value is safe to share by reference across threads. Ingest, sealed-object
verification, and pre-commit revalidation may run concurrently on `&Vault`, for the same artifact
or for different artifacts, without a caller-supplied lock; the daemon must not serialize ingest to
work around them. Concurrent ingest of identical bytes into one policy namespace publishes exactly
one object and adopts it for every other caller, so no caller receives a spurious failure, no
object is published without a receipt, and no partial is leaked into `vault/tmp`. Reconciliation is
excluded from that contract: it is a startup pass that removes expired partials and quarantines
orphans, and it must complete before the daemon accepts clients.

Windows makes that contract concrete. The explicit durable directory handle is opened with
`GENERIC_WRITE` because `FlushFileBuffers` refuses a read-only directory handle, so the flush the
design names actually executes instead of being swallowed as unsupported. Every synchronization
still writes the write-through directory barrier, which is the ordering mechanism only on a host
that rejects the flush; the barrier permits read and write sharing and its write is covered by the
same bounded sharing-violation retry as publication, because concurrent ingests synchronize the
same `vault/tmp`, object fan-out, and lease directories. Lease-file creation requests read access
only, so it cannot collide with a shared lease another thread took inside the existence-check
window.

This lease is a product coordination boundary, not an OS sandbox claim. Windows file sharing and
Unix advisory `flock` provide a portable cross-process protocol for every Academic Platform owner,
but an unrelated same-user process, malware, administrator, or storage failure can ignore or bypass
the Unix advisory lease. Immediate pre-commit revalidation detects mutations visible at that gate;
SQLite and a separate filesystem cannot be made atomic against a hostile mutation in the final
instruction window. The single-owner daemon, protected local profile, and out-of-process trust
boundary therefore remain required, and this Phase 1 mechanism does not accept or close ADR-004's
encrypted production format gate.

## Crypto-shredding an object

`P2-K5` adds the one operation in this format that writes into an object that is
already published, and it is deliberate: a crypto-shred that wrote a new file and
left the old one would have destroyed nothing.

An object's DEK exists in exactly one place — the 80-byte wrapped-DEK key slot at
offset 128 of its header, sealed under `KEK_d`, holding the DEK and the plaintext
digest. Destroying that slot is the shred. It is one positioned write plus a
sync, covering exactly `[KEY_SLOT_OFFSET, HEADER_BYTES)`; every other byte, the
file itself, and its length are left as they are.

```text
slot := "ACOB-KEYSLOT-SHREDDED-V1" (24) | tombstone_digest (32) | zero (24)
```

It claims the ciphertext is unreadable: no key opens the object afterwards, not
the domain KEK it was sealed under, not a rotated generation's, and not one
recovered from a backup. It does **not** claim the file was deleted, that its
bytes were overwritten, or that a copy taken earlier was reached — a copy inside
a backup is reached by `P2-K5`'s backup tombstone instead.
`ObjectFormatError::Shredded` states that in those words and is a distinct
variant from `Aead`, so a deliberate shred and a bit-rotted object have different
operator reports. The marker is that label and not a security boundary: whoever
can write it can equally overwrite the slot with noise.

The details, and the `RB01` "shredded or intact" argument, are in
[rotation and retention](../contracts/rotation-and-retention.md).

## Acceptance gate

Zero-byte/small/multi-GB/seekable-audio vectors; a trusted byte-resolving verifier capability for partial/page/time/repository evidence; wrong key, truncation, reorder, splice, and wrong-domain detection; every crash-point closure outcome; cross-policy dedupe rejection; quarantine/GC dry run; and format N/N-1 read/migration.

Discharged by `P2-K3`: wrong key, truncation, reorder, splice, and wrong-domain detection; the `OB01`-`OB09` crash-point outcomes; cross-policy dedupe rejection; quarantine of an unreferenced re-sealed object; zero-byte, one-byte, sub-chunk, exact-chunk, and exact-multiple vectors; and a committed format N and N-1 corpus. Seeking is exact over a real multi-chunk object; the multi-gigabyte half of that row is the chunk arithmetic, checked at a 6 GiB header rather than against a 6 GiB file, and is recorded as such rather than as an executed multi-gigabyte write. Still open: the byte-resolving verifier capability for partial, page, time, and repository evidence; seekable-audio vectors, which need `P2-L2`'s capture format; and collection of the vault's `quarantine/` directory, which stays open and is the daemon lane's. Collecting the *superseded object a rotation leaves* is a different thing and is not open: `P2-K5`'s `retire_superseded_object` destroys its key slot, gated on the rotation being complete, the unit migrated, the superseded object being the one that unit supersedes, and the store resolving to the locator the journal recorded. [Rotation and retention](../contracts/rotation-and-retention.md) states those gates; a quarantined object is one reconciliation moved aside, and nothing here collects it yet.
