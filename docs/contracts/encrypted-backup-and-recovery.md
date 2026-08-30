# Encrypted backup, independent restore, recovery profiles, rehearsal

## Posture

Producing an encrypted backup is not ADR-002, ADR-005, or ADR-012 acceptance
and is not permission to back up a real byte.

```text
adr_002_accepted=false
production_data_allowed=false
default storage_encryption=NONE
```

Every manifest this build writes records `production_data_allowed=false` and
`adr_002_accepted=false`, and a manifest claiming otherwise is refused on read.

## The recovery profile is a user choice

`GATE-38-031` is open. This build ships all three profiles of section 3.3,
drills all three, and states what each one loses. It selects none:
`RecoveryProfile` implements no `Default`, no constant names a selected profile,
and `backup_key_is_independent_of_device_wrapper` fails if one appears.

| Profile | Recipients | Loss behaviour | Can hold a backup key |
| --- | --- | --- | --- |
| `DEVICE_ONLY` | device keystore | `OS reimage or device loss is unrecoverable` | **no** |
| `DEVICE_PLUS_PHRASE` | device keystore + printed recovery phrase | recoverable on a fresh machine with the phrase | yes |
| `DEVICE_PLUS_PHRASE_PLUS_OFFLINE_FILE` | above + an offline key file | recoverable with either secondary recipient; largest exposure surface | yes |

The `DEVICE_ONLY` cell is a constant, not a sentence someone retyped. Every
surface that shows the profile shows
`academic_recovery::DEVICE_ONLY_IRRECOVERABILITY_STATEMENT`, and the refusal a
user sees when they attempt a backup under `DEVICE_ONLY` quotes it verbatim.

`DEVICE_ONLY` cannot hold a backup key because a backup key must be independent
of the device wrapper and that profile has no recipient which survives the loss
of the device. The table's last column is therefore mechanical rather than
advisory: `create_backup_key_set` refuses, and so does `backup_encrypted_profile`.

## Backup key independence, and its exact boundary

The backup root is 32 random bytes generated for one backup key set and wrapped
**only** by recovery-class recipients. Independence fails in two ways and both
are closed:

- **Directly**, if a device recipient could wrap the root.
  `BackupRecipientKind` has no device variant, and
  `BackupRecipientKind::from_requirement(RecipientRequirement::DeviceKeystore)`
  is `None`. There is nowhere in the type system to put one.
- **Transitively**, if the root were derived from the Vault Master Key. The
  device wrapper unwraps the VMK, so every key derived from the VMK is a key
  the device wrapper produces. The root is a *root*: no derivation edge from
  the VMK exists, and nothing in `academic-recovery` accepts a `VaultMasterKey`.

```text
recovery phrase ──Argon2id(pinned)──> wrap key ──XChaCha20-Poly1305──> BMK
                                                                        │
                            MKEY   = HKDF-SHA-512(BMK, salt = backup_set_id, info = "academic-os/backup-manifest/v1")
                            SIGSEED= HKDF-SHA-512(BMK, salt = backup_set_id, info = "academic-os/backup-signature/v1")
                            MACKEY = HKDF-SHA-512(BMK, salt = backup_set_id, info = "academic-os/backup-recipient-mac/v1")
```

**What this does not claim.** The snapshot inside the backup is still the
profile's own SQLCipher database under `SKEY_p`, and the objects are still
`AEAD_CHUNKED_V2` under `KEK_d`. Someone holding the live device already holds
those keys and already holds the live profile, so re-encrypting the payload
under the backup root would move no threat boundary. What the backup root
guarantees is the property recovery actually depends on: the manifest — and with
it the file inventory, the object closure, and the profile's own recovery
recipient records — cannot be opened by the device, so a backup whose only
reader was the lost machine cannot exist.

## Backup layout

```text
<backup>/
  BACKUP_FORMAT_V2                   # plaintext marker: format name + manifest version
  keys/backup-recipients.cbor        # BMK wrapped per recovery-class recipient
  manifest.cbor                      # sealed and signed under the BMK
  store/academic-platform.sqlite3    # SQLCipher snapshot at a fixed watermark
  objects/<artifact-id>.aobj         # AEAD_CHUNKED_V2 objects, byte-for-byte
```

The copy into `store/` is keyed **before** the first page is written. A SQLite
Online Backup into an unkeyed destination writes plaintext pages, so this is not
a convenience: `require_unreadable_without_key` re-opens the copy with no key
and fails the backup if the schema is readable.

## The manifest, and the two digests

The sealed body carries the store schema identity, the watermark, the counts,
the device heads, the canonical semantic digest, the file inventory, one entry
per object with **both** its ciphertext and its plaintext digest, and the
profile's recovery-class recipient records. Only the format marker and the
wrapped key set are readable without a secret.

Two digests, and they answer different questions.

| Digest | Covers | Equal for two backups of one watermark |
| --- | --- | --- |
| `semantic_digest` | the whole semantic block, file inventory included | **no** |
| `semantic_identity_digest` | format, posture, schema identity, watermark, counts, device heads, canonical semantic digest, and each object's logical identity and plaintext digest | **yes** |

`semantic_digest` cannot be stable, and pretending otherwise would be the
mistake. SQLCipher re-encrypts every page it writes with a fresh initialisation
vector, so two Online Backup copies of one unchanged database are different byte
strings. `two_backups_at_same_watermark_have_equal_semantic_digest` asserts the
identity digests are equal **and** that the database digests differ, so the
equality is a claim about committed state rather than an artifact of copying.

## Restore

Restore targets only a new empty destination and publishes with one rename, so
every failure leaves the backup and the current profile untouched and the
destination either absent or completely verified. The order is fixed:

 1. refuse a destination that is not new and empty;
 2. refuse a destination inside the backup being restored. A restore into the
    backup's own tree publishes a directory the backup does not list, so the
    backup stops verifying against its own manifest — a silent, permanent
    deletion behind a destination that looked like a perfectly good new empty
    directory. The comparison is over canonical paths, and because the
    destination does not exist yet its nearest existing ancestor is what is
    compared;
 3. read the format marker and the wrapped key set;
 4. verify the manifest signature, then open it with the backup root;
 5. check the recorded digest and length of every file in the directory;
 6. stage an encrypted profile and copy the database, then prove the copy is
    unreadable without its key;
 7. `cipher_integrity_check`, `integrity_check`, `foreign_key_check`;
 8. compare schema identity, watermark, counts, device heads, and the canonical
    semantic digest against the manifest;
 9. replay every signed batch against **caller-supplied** trust anchors — never
    anchors read out of the backup, which would authenticate nothing;
10. copy every object, check its ciphertext and plaintext digests, then
    authenticate each one through the vault;
11. re-apply every tombstone the backup carries to the staged object tree, and
    record on the receipt what was re-deleted, what was deliberately left
    readable because it only shares a locator, and what reached nothing.
    This is the restore half of a `P2-K5` deletion and it happens here, after
    every object has been authenticated and before the rename, so no published
    restore holds a key slot the profile it came from had destroyed;
12. remove the incomplete markers, synchronize, publish.

A backup is taken of **one** generation. `SKEY_p` and `KEK_d` both derive from
the Vault Master Key, so a caller that rotated the objects and kept a key set
derived from the superseded master would write a database under one generation
beside objects under another. That backup verifies — every file is present and
digest-checked — and nothing restores it, because a restore recovers a single
master and derives both halves from it. `backup_encrypted_profile` refuses that
pairing when it is taken (`backup profile key generation`).

That guard reads the two *arguments*: `keys.generation()` against
`master.generation_id()`. A profile whose halves are genuinely split on disk —
objects rotated, `STORE_DATABASE` unit not run — is refused as well, but by the
halves rather than by that name: under the superseded generation the rotated
objects no longer derive their locators, and under the target one the database
does not open. Both refuse before anything is published.

A crypto-shredded object does not stop a backup. Its descriptor keeps the locator
of the generation it was destroyed under, because nothing can re-seal an object
whose key slot is gone; `EncryptedVault::verify_shredded_object` is what lets the
copy still be taken, and what it requires of the file it copies is in
[rotation and retention](rotation-and-retention.md).

A rotation after the shred does not stop one either, and that half is not
reachable in Phase 2: running a rotation is refused, and what an orchestrator
would have to close first is listed in the same document. A deletion needs no
rotation, so everything above and below this paragraph is.

A restore does not trust its manifest. A manifest re-sealed with the real backup
key after its counts were altered verifies and decrypts, and the restore still
refuses, because step 8 re-derives the counts from the restored database.

Projections are not restored. They are disposable, are never backed up as truth,
and are rebuilt by the projection engine, which the encrypted lane does not link.

## Fresh-machine recovery

A fresh machine holds the backup directory and the printed phrase, and nothing
else. That is the whole chain:

```text
phrase → BMK → sealed manifest → the profile's recovery recipients → VMK → SKEY_p and KEK_d → restored profile
```

The profile's *device* recipient records are deliberately not carried into a
backup: they name a broker that does not exist on the machine the backup is
being restored onto, and carrying them would invite a restore that depends on
the device that was lost. `a_backup_carries_no_device_recipient` asserts it.

## The rehearsal receipt

```text
<profile>/admission/rehearsal.cbor
```

`GATE-P2-RECOVERY` blocks the first real ingest of any kind until a rehearsal
receipt exists which belongs to this profile, authenticates under this profile's
key, exercised the recovery profile that is in force, and names the key material
the profile holds **now**.

"Newer than the last key change" is not a clock comparison. A wall clock can
move backwards and two key changes inside one millisecond are
indistinguishable, so the receipt records a monotonic `key_material_generation`
**and** a digest over the recipient set, and the gate requires both to match. A
rotation, a recipient added, or a recipient revoked changes the digest, and the
rehearsal stops admitting until a new drill runs.

The receipt is authenticated under `HKDF-SHA-512(VMK, salt = profile_id,
info = "academic-os/rehearsal/v1")` rather than under the backup key, because the
gate runs at ingest time on an unlocked profile, where the VMK is in hand and
the recovery phrase is not.

Refusal reasons are a closed set: `RehearsalAbsent`, `ProfileMismatch`,
`ReceiptUnverified`, `RecoveryProfileMismatch`, `StaleKeyMaterial`,
`KeyMaterialMismatch`. There is no seventh outcome that admits.

## Faults

`BK01`–`BK04` and `RS01`–`RS04` are the Phase 1 identifiers at the Phase 1
positions, re-run against an encrypted profile under the
`phase2-fault-injection` feature. A production build compiles every checkpoint
away: there is no environment lookup and no crash switch.

**`BK03` is reachable here.** It fires on the second object copy, so it needs a
corpus with two registered artifacts. Phase 1's daemon *exit* corpus registers
one and records `BK03` as `NOT_RUN` pointing at `crates/portability/tests/crash.rs`,
which does register two and does assert the outcome; `tools/phase1-exit.mjs`
gates the run on that suite passing. The encrypted corpus registers two for the
same reason, so the checkpoint is reached rather than pointed at: the child
process aborts there — the harness asserts the ready marker names it — and the
parent observes exactly one copied object and no manifest.

## Building and verifying

The encrypted lane is non-default and cannot link into the same binary as the
plaintext synthetic lane, because `academic-store`'s two lanes cannot.

```sh
cargo test -p academic-recovery --locked --offline
cargo clippy -p academic-portability --no-default-features --features encrypted-portability,phase2-fault-injection --all-targets --locked -- -D warnings
cargo test -p academic-portability --no-default-features --features encrypted-portability --locked --offline
cargo test -p academic-portability --no-default-features --features encrypted-portability,phase2-fault-injection --locked --offline --test encrypted_crash
```

`academic-recovery` is pure Rust and runs in the default workspace lane on every
CI platform. The `academic-portability` encrypted lane needs a native SQLCipher
and OpenSSL, the same requirement and the same pinned Windows toolchain as
[the encrypted store lane](encrypted-store-lane.md). Since `P2-RF1` it runs in
hosted CI on `ubuntu-latest` as the `encrypted-portability-lane` job; native
Windows stays the README-documented local lane, for the `openssl-src` reason
t068 section 2.3-17 records.

`tests/encrypted_rotation.rs` is in that job. It is where `P2-K5`'s rotation and
deletion meet this boundary: a rotation moving the canonical object reference, a
crypto-shredded object that no longer blocks a backup, a tombstoned backup that
still verifies and restores, and a restore that refuses a destination inside the
backup it is reading.

## Which named test proves what

| Test | Crate | Lane |
| --- | --- | --- |
| `backup_key_is_independent_of_device_wrapper` | `academic-recovery` | default, in CI |
| `device_only_profile_states_irrecoverability_verbatim` | `academic-recovery` | default, in CI |
| `rehearsal_receipt_is_required_before_first_ingest` | `academic-recovery` | default, in CI |
| `rehearsal_is_invalidated_by_key_change` | `academic-recovery` | default, in CI |
| `restore_rejects_nonempty_target` | `academic-portability` | `encrypted-portability` |
| `restore_verifies_ledger_object_and_count_closure` | `academic-portability` | `encrypted-portability` |
| `fresh_machine_restore_with_phrase_only` | `academic-portability` | `encrypted-portability` |
| `two_backups_at_same_watermark_have_equal_semantic_digest` | `academic-portability` | `encrypted-portability` |
