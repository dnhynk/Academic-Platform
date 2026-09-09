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

**"Zeroized on drop" is a property of the named key types, not of every buffer a key byte passes through.** A key is derived, wrapped, or applied by copying bytes somewhere, and the copies outside a named type are cleared one by one where the code owns them:

| Buffer | Cleared |
| --- | --- |
| the eleven `secret_key!` types, `BackupMasterKey`, `RecoveredSecret` | yes — `Zeroize`/`ZeroizeOnDrop` |
| `OpenedHeader`'s DEK and plaintext digest, `DomainKeyring`'s domain keys | yes — hand-written `Drop` |
| the unwrapped `DEK ‖ digest` inside `open_header`, `EncryptedObjectReader`'s decrypted chunk | yes — cleared on every path out |
| the NCrypt output buffer holding `label ‖ device wrapping key` | yes — overwritten before `NCryptFreeBuffer` |
| the `PRAGMA key=…` statement text | yes — overwritten in `apply_store_key` |
| SQLite's copy of that statement inside the prepared statement | **no** — not reachable from this codebase |
| the D-Bus message buffers `zbus` serializes a Secret Service call into | **no** — not reachable from this codebase |

The two "no" rows are in-process memory only. No route puts a key byte on disk: the byte-level canary scan of the vault tree, the store database with its WAL and shared-memory file, and the backup and restore trees reports zero hits for any key in this schedule.

A type carrying key material or decrypted plaintext must not derive `Debug`; `missing_debug_implementations = "deny"` demands an implementation and the derive is the leaking way to supply one. That has regressed three times, so it is checked mechanically by `tools/secret-debug-policy.test.mjs` rather than by review.

## `unsafe` confinement

`academic-keystore-platform` is the second reviewed native FFI boundary after `academic-store-platform` and follows the same pattern: the crate overrides the workspace's `unsafe_code = "forbid"` to `deny`, each `unsafe` block sits in a small private function carrying `#[allow(unsafe_code)]` and a concrete safety argument, and the public facade exposes no raw handle, pointer, descriptor, or D-Bus object. The Linux half contains no `unsafe`. `academic-crypto` inherits `unsafe_code = "forbid"` unchanged.

## Still open

- **Recovery-profile selection is a user choice with no default** (`GATE-38-031`). `P2-K1` builds the hierarchy and selects nothing, and `P2-K4` ships and drills all three profiles without selecting one either: `academic_recovery::RecoveryProfile` implements no `Default` and no constant names a selection. What is still open is the selection itself, and the first real ingest stays blocked until it is made.

  **The 24-word codec and its wordlist belong to that same decision and are also still open.** `P2-K4` shipped no codec, deliberately. t068 section 5 fixes no wordlist for `P2-K4`, none of its eight named acceptance tests needs one, and a wordlist is permanently frozen the moment a phrase is printed under it — a phrase written from one list cannot be read back under another — so adopting a language and a list is a user decision, not an implementation detail a task may guess at. The next implementer must not assume `P2-K4` did it.

  What `P2-K4` did do is keep the cryptographic contract independent of that decision. `academic-crypto` and `academic-recovery` both accept only a whole 256-bit `RecoverySecret` and expose no word-level entry point, so a codec can be added later without changing a single derivation, and no API in either crate can report *which* word of a phrase was wrong — which is how `KY06`'s "no oracle" requirement is met structurally rather than by care. `recovery_secret_api_has_no_word_level_entry_point` fails if that regresses. Every test whose name says "phrase" is exercising a 256-bit secret and says so; none of them is evidence that a codec works.
- **A stateless sealing broker cannot revoke a blob it already issued.** That asymmetry is carried in `PurgeOutcome` rather than hidden, and `native_roundtrip` asserts each broker's half of it unconditionally: on Windows that a purge reports nothing stored and the blob still opens, on Linux that a purge removes the item and the next unlock fails closed.
- **The Linux Secret Service session is `plain`, so the device wrapping key crosses the session bus in the clear.** The specification's alternative, `dh-ietf1024-sha256-aes128-cbc-pkcs7`, is not adopted. What that costs and why it is the choice are below.
- **ADR-002 is not accepted.** The default lane is still plaintext SQLite with `storage_encryption = NONE` and `adr_002_accepted = false`. A key hierarchy existing does not admit real data; `GATE-P2-ADMISSION` governs that and is closed.

## Secret Service transport: the session is `plain`

`open_session` opens the Secret Service session with the `plain` algorithm, so the 32-byte device wrapping key travels the user's session bus as cleartext inside the D-Bus message body — on `CreateItem` when it is sealed, and on `GetSecret` every time the profile is unlocked. This is a Linux-only exposure; the Windows broker seals in-process and puts nothing on a bus.

**What is exposed.** Anything that can observe that bus traffic or dump those buffers sees the key: a bus monitor running as the same user, a captured message dump, a core dump of the broker process or of this one. The `zbus` message buffers are not reachable from this codebase and are not cleared, which is the second "no" row above.

**What is not exposed.** Nothing crosses a machine boundary — the session bus is a Unix socket owned by the user — and nothing reaches disk. The blob persisted beside the recipient record carries only the label binding, never a key byte.

**Why `dh-ietf1024-sha256-aes128-cbc-pkcs7` is not used instead.**

- It does not gate access to the key. The session algorithm encrypts the secret in transit between this process and the broker; it authenticates neither end. Any process running as the same user can call `SearchItems` and `GetSecret` itself and be handed the key by the broker, because Secret Service authorizes on the bus connection and the item lives in the default collection. A negotiated session raises the bar against passive capture of bus traffic and against nothing else.
- It cannot be built in this lane. AES-128-CBC and the 1024-bit modular exponentiation the group needs are absent from the locked dependency graph — there is no `aes`, no `cbc`, and no big-integer crate in `Cargo.lock`. Adopting it means three new dependencies through the source policy and a second reviewed cipher path inside the broker crate, whose stated design is that it carries exactly one.
- 1024-bit MODP is below current guidance, so the second cipher path would be added at a strength the rest of this schedule does not use anywhere.

The decision is therefore to keep `plain` and state the exposure here rather than in a source comment only. It is revisited if the threat model ever admits an attacker who can observe the session bus but cannot talk to the broker; that attacker does not exist in the current model, where both capabilities come with being the same user.

## macOS data-protection Keychain (T251)

The macOS implementation adds `MACOS_KEYCHAIN_DATA_PROTECTION_V1`, provider tag
3, through the same private FFI leaf. Existing provider tags and blob payloads
stay unchanged. It explicitly selects the data-protection Keychain, disables
synchronization, uses `WhenUnlockedThisDeviceOnly`, and refuses interaction,
duplicate labels and main-thread blocking calls. A random item generation binds
open and purge to one creation, so an old blob cannot delete a later key under
the same label. It stores generic passwords and makes no Secure Enclave or
hardware-backed claim.

The [macOS device keystore contract](../contracts/macos-device-keystore.md)
records the required signed-in process identity, provisioning and access-group
continuity, no-fallback behavior, persistence and purge limits, and exact native
acceptance prerequisites. A system launchd daemon cannot use this provider.
The outgoing mutable CF buffer and recovered Rust buffer are zeroized; Apple's
immutable returned CFData and internal copies are additional buffers we cannot
safely clear. Earlier platform canary observations do not cover those copies.
Hosted refusal/build evidence does not establish native positive acceptance,
packaging, hardware protection or H1 admission.

### Add-only recipient publication recovery (T268 / PR119 R1)

Enabling macOS under the existing one-step recipient helper exposed a lifetime
gap: a fallible wrap after successful persistent seal, or interruption before
the record was returned/published, lost the only random generation blob. A
same-label retry correctly refused the duplicate, but could not recover or
purge the orphan. Local object destruction and the in-memory KY08 fixture do
not establish persistent recovery, and no native RNG failure is claimed.

The correction prepares a fresh, private, single-use seal identity first and
finishes all recipient cryptography before native add. A required caller
callback durably stages the complete encrypted recipient as an incomplete
publication; callback error prevents add. The caller retains that record across
ambiguous add errors, KY08 and failed final publication, resolving it only after
durable publication or exact cleanup. Restart can open the staged recipient or
purge only its exact generation; cleanup refusal retains the identity and a
fresh attempt uses a fresh token. No API rebuilds that token from old bytes.

The optional `RecoverableDeviceKeystore` seam preserves existing trait
implementations. macOS requires it and refuses the legacy crypto helper before
mutation; Windows/Linux formats and legacy behavior are unchanged. Native query
selection and atomic duplicate refusal remain unchanged. Storage durability,
exclusive incomplete-attempt ownership and final-publication reconciliation
are caller obligations, detailed in the macOS contract. Bounded persistent
fixture tests exercise restart and cleanup refusal; they do not grant native
crash, provisioning or H1 acceptance. Exact crypto/keystore item inventories are
regenerated through the unchanged deterministic reader for this API change.

The exact item inventory is regenerated only for `keystore-platform`, using
the unchanged `contracts/tests/support` compilation-unit and item reader. Its
diff records the new private module, versioned provider and error variant;
neither old provider payloads nor the inventory's enforcement rules change.
The dependency receipt admits the two target-only framework bindings and the
new use of existing `getrandom` for generation IDs. The link inventory records
its `libc` edge while the source-level prohibition on socket calls remains.

## Rotation and revocation

Rotating a domain KEK means rotating the Vault Master Key: `KEK_d` and `SKEY_p`
take no epoch, so they change only when the VMK does. One rotation therefore
moves every object and the store database, cannot be atomic, and is driven by an
append-only journal at `<profile>/keys/rotation-journal.jsonl`.

The invariant is that after an interruption at any point, exactly one of the old
and new keys opens **the object the profile resolves to**. "Both open" and
"neither opens" are both violations, and the rules that make it hold — refusing a
rotation that does not change the key, moving reachability only after a verified
read-back, never editing or removing the source object, and moving the canonical
reference by an appended `artifact_descriptor_migration` row that a
`RETENTION_ACTION_RECORDED` event authorizes — are in
[rotation and retention](../contracts/rotation-and-retention.md) with the fault
rows that prove each one.

It is an invariant over the resolved object and not over every file on disk. A
superseded object stays a readable file under the superseded key until
`retire_superseded_object` destroys its key slot, which is the collection point
ADR-004 leaves open. A crypto-shredded object is the other exception: its key
slot is already gone, so nothing can re-seal it and its `artifact_descriptor` row
keeps the locator of the generation it was destroyed under. Neither key opens it,
which is the point of destroying it.

The store database is covered by its own rotation unit. `academic-retention`
plans and journals it and `RotationEngine::rotate_store_database` runs it through
a `StoreDatabaseExecutor`, which the encrypted portability lane binds to
`P2-K2`'s `PRAGMA rekey`; the executor reports the pair of generations it holds
and the engine refuses one that is not the plan's pair. A rotation that reaches
the unit with no executor run still refuses to complete, by name.

**Phase 2 does not accept running a rotation.** Everything in this section is
built and tested, and the seven entry points that would drive one — begin, move a
unit, complete, retire a superseded object, rewrap a recipient set for a new
generation, retire a generation — refuse on their first line unless the
non-default `rotation-orchestration` lane is selected, which no product graph
does. Crypto-shredding, backup tombstones, and their re-application on restore
are outside that gate and keep working. What is not yet closed, and what an
orchestrator has to close first, is listed in
[rotation and retention](../contracts/rotation-and-retention.md).

Revocation removes a recipient's wrapped key and stops any future generation from
being wrapped for it. **That is the whole of it.** It does not erase plaintext
that recipient already read, it does not reach a copy taken while the recipient
was live, and — until the rotation's superseded objects are retired — it does not
stop that recipient's key from opening the superseded copies still in the live
tree. `academic_retention::REVOCATION_SCOPE_STATEMENT` says the first two in the
words every surface repeats, `revocation_does_not_claim_prior_plaintext_erasure`
fails if any surface stops carrying them or starts claiming more, and
`a_retired_source_object_is_opened_by_neither_generation` is the third. The
operation that *can* make one artifact's ciphertext unreadable is the
crypto-shred of ADR-004, and it works by destroying key material rather than by
revoking a recipient.

## Acceptance gate

OS reimage/lost device/lost password decision table; fresh-machine recovery; interrupted KEK rewrap; revoked-device future-access test; key-memory/lifecycle review; backup key independence; and UX that accurately states irrecoverability. No production key material exists in Phase 0.
