# ADR-005: Key hierarchy and recovery

- Status: Proposed decision register. The hierarchy below is implemented by `academic-crypto` and `academic-keystore-platform` (`P2-K1`), its rotation and revocation by `academic-retention` (`P2-K5`); recovery-profile selection is not decided.

## Registered direction

Use a Vault Master Key wrapped by one or more recovery/device recipients, domain KEKs below it, and random artifact DEKs wrapped by the appropriate domain KEK. OS adapters use DPAPI/Keychain/Secret Service or an approved hardware-backed mechanism; general logs, models, UI code, plugins, and provider SDKs never receive root or provider credentials.

Password recovery, device recovery, and account recovery are distinct. Password-based recovery, if offered, uses a reviewed Argon2id profile and an explicit offline recovery recipient. Rotation normally rewraps keys; full data re-encryption is a versioned migration, not a silent overwrite. Revoking a device prevents future keys and objects but cannot pretend to erase plaintext already obtained by that device.

## Fixed key schedule

```text
VMK      : 32 random bytes from OS randomness, never persisted unwrapped
KEK_d    = HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/kek/v1" || domain_id)
SKEY_p   = HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/store/v1")
AUDKEY   = HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/audit/v1")
RMACKEY  = HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/recipient-mac/v1")
GENID    = SHA-256(HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/key-generation/v1"))
```

All four key outputs are 32 bytes. `profile_id` and `domain_id` are the caller's canonical 16-byte identities; `academic-crypto` does not parse UUIDs, so the schedule cannot drift from the identities the rest of the profile uses.

`RMACKEY` is the fourth info string. The three named in the design document do not cover the separate requirement that each recipient record carry a MAC *under the VMK*; that MAC needs its own key rather than borrowing one of the other three.

`GENID` is not a key and no constructor accepts it. It is the public *name* of one key generation, which `P2-K5`'s rotation journal and recipient records need in order to say which generation a record belongs to while the profile is still locked. That name must therefore be readable without a key and must reveal nothing about one: HKDF-SHA-512 is one-way, and hashing its output again means the published value is structurally not usable as key material. Two Vault Master Keys share a generation name only if they are the same key.

`SKEY_p` is supplied to SQLCipher as a raw 32-byte key rendered as 64 lowercase hex characters, never as a passphrase.

## Recipient structure

Both recipient kinds are structurally identical: something produces a 32-byte wrapping key, and the VMK is sealed under it with XChaCha20-Poly1305.

- **Device recipient.** The operating system holds a 32-byte device wrapping key. On Windows that is CNG DPAPI (`NCryptProtectSecret` under a `LOCAL=user` protection descriptor), which seals statelessly and stores nothing itself. On Linux that is Secret Service (`org.freedesktop.secrets`), which stores the key in the default collection. The raw wrapping key never leaves the broker except for the length of one wrap or unwrap call.
- **Recovery recipient.** Argon2id derives the wrapping key from a 256-bit recovery secret under a versioned, pinned parameter profile.

`keys/recipients.cbor` therefore holds a real AEAD ciphertext of the VMK for every recipient on every platform, never the VMK.

### Recipient record

Deterministic CBOR, integer keys in ascending order, no unknown key accepted:

```text
0 record_version u8 = 1      6 wrap_algorithm_id tstr = "XCHACHA20-POLY1305"
1 profile_id     bstr(16)    7 wrap_nonce        bstr(24)
2 recipient_id   bstr(16)    8 wrapped_vmk       bstr(48)
3 kind           u8          9 keystore_blob     bstr
4 kdf_algorithm_id tstr     10 record_mac        bstr(64)
5 kdf_parameters map
```

`kind` is `1` for a device recipient and `2` for a recovery recipient. `kdf_algorithm_id` is `OS-KEYSTORE-V1` or `ARGON2ID` accordingly.

Two independent checks stand between a wrong key and a plaintext VMK:

1. the AEAD tag, whose associated data is the canonical encoding of fields `0..=6`, so a tampered identity, algorithm, or parameter fails before any plaintext exists; and
2. `record_mac = HMAC-SHA-512(RMACKEY, canonical encoding of fields 0..=9)`, compared in constant time after unwrapping, which is what catches a record whose MAC was replaced or lifted from elsewhere.

A broker that returns a wrong key and a record whose MAC does not verify are both reported as integrity incidents, distinct from a wrong recovery secret, which is an ordinary rate-limited refusal.

### Pinned Argon2id profile

| Identifier | Memory | Passes | Lanes | Output |
|---|---:|---:|---:|---:|
| `RECOVERY_ARGON2ID_V1` | 64 MiB | 3 | 1 | 32 bytes |

The parameters are written into the record verbatim and read back on every unlock. A reader accepts only a profile from the pinned set: an unknown identifier, or a pinned identifier carrying weakened costs, is refused rather than honoured, so a record edited on disk cannot downgrade the KDF.

The input is a 256-bit secret, so the KDF is defence in depth rather than the security boundary. The cost is chosen so a *replacement* machine can always run it: a recovery that fails for want of memory defeats the purpose of a recovery recipient.

## Zeroization and exposure boundary

Every key type owns exactly 32 bytes, implements `Zeroize` and `ZeroizeOnDrop`, prints a redacted `Debug`, and hands its bytes out only through an explicitly named `expose_secret`. There is no `Deref`, no `AsRef<[u8]>`, no `Clone`, and no `Serialize`, so a key cannot reach a writer, a log line, an audit row, or an export by accident.

## `unsafe` confinement

`academic-keystore-platform` is the second reviewed native FFI boundary after `academic-store-platform` and follows the same pattern: the crate overrides the workspace's `unsafe_code = "forbid"` to `deny`, each `unsafe` block sits in a small private function carrying `#[allow(unsafe_code)]` and a concrete safety argument, and the public facade exposes no raw handle, pointer, descriptor, or D-Bus object. The Linux half contains no `unsafe`. `academic-crypto` inherits `unsafe_code = "forbid"` unchanged.

## Still open

- **Recovery-profile selection is a user choice with no default** (`GATE-38-031`). `P2-K1` builds the hierarchy and selects nothing, and `P2-K4` ships and drills all three profiles without selecting one either: `academic_recovery::RecoveryProfile` implements no `Default` and no constant names a selection. What is still open is the selection itself, and the first real ingest stays blocked until it is made.

  **The 24-word codec and its wordlist belong to that same decision and are also still open.** `P2-K4` shipped no codec, deliberately. t068 section 5 fixes no wordlist for `P2-K4`, none of its eight named acceptance tests needs one, and a wordlist is permanently frozen the moment a phrase is printed under it — a phrase written from one list cannot be read back under another — so adopting a language and a list is a user decision, not an implementation detail a task may guess at. The next implementer must not assume `P2-K4` did it.

  What `P2-K4` did do is keep the cryptographic contract independent of that decision. `academic-crypto` and `academic-recovery` both accept only a whole 256-bit `RecoverySecret` and expose no word-level entry point, so a codec can be added later without changing a single derivation, and no API in either crate can report *which* word of a phrase was wrong — which is how `KY06`'s "no oracle" requirement is met structurally rather than by care. `recovery_secret_api_has_no_word_level_entry_point` fails if that regresses. Every test whose name says "phrase" is exercising a 256-bit secret and says so; none of them is evidence that a codec works.
- **A stateless sealing broker cannot revoke a blob it already issued.** That asymmetry is carried in `PurgeOutcome` rather than hidden.
- **ADR-002 is not accepted.** The default lane is still plaintext SQLite with `storage_encryption = NONE` and `adr_002_accepted = false`. A key hierarchy existing does not admit real data; `GATE-P2-ADMISSION` governs that and is closed.

## Rotation and revocation

Rotating a domain KEK means rotating the Vault Master Key: `KEK_d` and `SKEY_p`
take no epoch, so they change only when the VMK does. One rotation therefore
moves every object and the store database, cannot be atomic, and is driven by an
append-only journal at `<profile>/keys/rotation-journal.jsonl`.

The invariant is that after an interruption at any point, exactly one of the old
and new keys opens any object or database. "Both open" and "neither opens" are
both violations, and the three rules that make it hold — refusing a rotation that
does not change the key, moving reachability only after a verified read-back, and
never editing or removing the source object — are in
[rotation and retention](../contracts/rotation-and-retention.md) with the fault
rows that prove each one.

Revocation removes a recipient's wrapped key and stops any future generation from
being wrapped for it. **That is the whole of it.** It does not erase plaintext
that recipient already read and it does not reach a copy taken while the
recipient was live; `academic_retention::REVOCATION_SCOPE_STATEMENT` says so in
the words every surface repeats, and
`revocation_does_not_claim_prior_plaintext_erasure` fails if any surface stops
carrying them or starts claiming more. The operation that *can* make one
artifact's ciphertext unreadable is the crypto-shred of ADR-004, and it works by
destroying key material rather than by revoking a recipient.

## Acceptance gate

OS reimage/lost device/lost password decision table; fresh-machine recovery; interrupted KEK rewrap; revoked-device future-access test; key-memory/lifecycle review; backup key independence; and UX that accurately states irrecoverability. No production key material exists in Phase 0.
