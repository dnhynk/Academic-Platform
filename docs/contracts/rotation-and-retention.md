# Rotation, revocation, crypto-shredding, and retention (`P2-K5`)

## Posture

Rotating a key or destroying a key slot is not ADR-002, ADR-004, ADR-005, or
ADR-012 acceptance.

```text
adr_002_accepted=false
production_data_allowed=false
default storage_encryption=NONE
```

`academic-retention` is a workspace crate no product binary links. One crate
declares a product edge to it — `academic-portability`, optional and selected
only by the non-default `encrypted-portability` lane, so the encrypted restore
can re-apply the tombstones a backup carries — and
`rotation_engine_lane_is_not_default` proves no default graph resolves it.
`P2-P2` is the task that wires the real transcript, embedding, claim, document,
cache, and replica subsystems to it.

**What is not here.** No daemon or CLI command runs a rotation: the sequence
below is a library sequence, executed end to end by
`crates/portability/tests/encrypted_rotation.rs` and by nothing a user can
invoke. The `STORE_DATABASE` unit still has no bound executor, so a rotation
covers objects and leaves `SKEY_p` where it was. Both are stated again where
they bite rather than only here.

## What rotates, and why one rotation moves everything

`KEK_d` and `SKEY_p` are deterministic functions of the Vault Master Key with no
epoch input:

```text
KEK_d  = HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/kek/v1" || domain_id)
SKEY_p = HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/store/v1")
```

So **rotating a domain key means rotating the Vault Master Key**, and one
rotation moves every object and the store database together. That cannot be
atomic, which is what the journal is for.

A key generation has a public name:

```text
generation_id = SHA-256(HKDF-SHA-512(VMK, salt = profile_id, info = "academic-os/key-generation/v1"))
```

It is readable on a locked profile, it reveals nothing about the key, and the
extra hash means it is structurally not usable as one. It is what lets a resumed
rotation prove it was handed the same pair of keys it started with.

## Rotation journal

`<profile>/keys/rotation-journal.jsonl`, one JSON object per line, UTF-8,
newline-terminated:

```json
{"journal_version":1,"sequence":0,"previous_digest":"00…00","entry":{"kind":"RotationStarted",…},"entry_digest":"<64 hex>"}
```

`entry_digest = SHA-256("academic-os/journal/v1" | LE64(sequence) | previous_digest | entry_json)`,
where `entry_json` is the serialized `entry` of that same line. Record zero links
to 64 zero characters; every later record links to its predecessor's
`entry_digest`.

That chain is what makes append-only *checkable* for every edit inside the file.
A rewritten line breaks its own digest, a reordered pair breaks the link, and a
line removed from the middle breaks the sequence of everything after it;
`AppendOnlyJournal::open` verifies the whole chain before it will append. The
handle is held in append mode for the life of the value.

A backward chain cannot see its own tail being cut off — drop the last *k*
records and the remaining prefix still verifies — so the head is recorded beside
the journal:

```json
<journal>.head   {"journal_version":1,"record_count":3,"head_digest":"<64 hex>"}
```

It is written after the record it names is already durable. A journal holding
*fewer* records than its head declares, or a different digest at the position
the head names, is refused and cannot be extended. A journal holding *more* is a
kill between an append and the head write; the next open repairs the head.

**This is a consistency anchor, not a MAC.** A profile's journal is cleartext by
design and nothing here is keyed, so an adversary who can write both files can
rewrite both consistently and is not detected. What the head closes is the case
the chain alone could not see at all.

A torn final line — a fragment with no newline — is a crash, not tampering:
`append` writes one whole line and syncs it before returning, so a fragment was
never a durable record. It is dropped and truncated away when the journal is
opened, which is what makes an interrupted rotation resumable. A *complete* line
that does not parse is still `Malformed`.

Entries are a closed set: `RotationStarted`, `UnitResealed`, `UnitMigrated`,
`UnitSourceRetired`, `RotationCompleted`, `RecipientAdded`, `RecipientRevoked`,
`RetentionPlanned`, `ArtifactShredded`, `BackupTombstoneWritten`,
`RetentionSettled`.

**A journal is not encrypted, so nothing private may be in one.** Entries carry
locators — which are already the on-disk filenames — digests of them, generation
names, recipient identifiers, and reason codes. They carry no media type, no
content digest, no plaintext length, no artifact identity, and no key byte.

`<profile>/retention/deletion-journal.jsonl` is the same contract for the
deletion half.

## The invariant

> After an interruption at any point, exactly one of the old and new keys opens
> any object or database.

Both "both open" and "neither opens" are violations. Three things make it hold.

1. **A rotation that does not change the key is refused before anything is
   written.** `RotationPlan::new` rejects a plan whose target generation name
   equals its source. Without that rule every object would open under both keys.
2. **Reachability moves only after a verified read-back.** For each unit the
   engine (a) re-seals into the target namespace, where the vault publishes,
   reads back, and authenticates the object before returning; (b) appends
   `UnitResealed`, which says the target object is durable and verified and
   **not** reachable; (c) appends `UnitMigrated`, which moves reachability. A
   kill anywhere before (c) leaves the source generation in force over an
   untouched source object.
3. **The rotation never edits or removes the source object.** It becomes
   unreferenced when reachability moves, and the vault's own reconciliation
   quarantines it after the grace window. So "neither opens" cannot arise from
   the rotation deleting something too early.

Because the locator derives from `KEK_d`, a re-sealed object lands on a *new
path*. The two generations are two files, never one file rewritten in place.

Which generation is in force is a pure function of the journal
(`UnitProgress::opening_generation`), so a resumed process decides without
holding either key.

**The invariant is over the object the profile resolves to, not over every file
on disk.** Until a superseded object is retired (below) it is still a readable
file under the superseded key. What the sentence rules out is an artifact with
two live references or none.

## Moving the canonical reference

Reachability inside the journal is not reachability for the store. The store's
`artifact_descriptor.vault_locator` is inside the signed `ARTIFACT_REGISTERED`
payload and the table is INSERT-only twice over, so a rotation cannot write the
new locator over the old one. The reference moves the way every other correction
in this store moves: an appended row supersedes an older one.

```text
artifact_descriptor_migration
  retention_action_id  -> retention_action(retention_action_id)
  artifact_id          -> artifact_descriptor(artifact_id)
  migration_seq, superseded_locator, vault_locator, format_version, record_digest
```

Migration `0005` adds it, as the typed columns migration `0004` reserves for each
aggregate owner. **No new event kind and no nineteenth v3 arm**: the canonical
authorization is the existing `RETENTION_ACTION_RECORDED` arm, whose registration
frame carries the optional provenance digest that binds this exact move.
`record_digest` is `SHA-256` over the domain separator, the retention action
identity, the artifact identity, the sequence, both locators, and the format
version, and migration `0005`'s triggers refuse a row whose digest is not the
`source_digest` its retention action carries, or that does not continue the
chain. `read_artifact_descriptors` resolves the chain, so backup, restore, the
store's own pre-commit revalidation, and reconciliation all name the object the
current key opens.

The order is event first, row second. A kill between them leaves the reference
where it was, the superseded object still reachable, and the migration
re-runnable. The opposite order would point the store at an object no accepted
event had authorized.

## Retiring a superseded object

`ADR-004` quarantines the superseded object and leaves the collection point
open. `retire_superseded_object` is that point: it destroys the superseded
object's key slot, using a digest that names the rotation rather than a
deletion, and records `UnitSourceRetired`.

It is what makes "exactly one of the old and the new key opens this artifact"
true of the files on disk. **Before retirement it is not.** A holder of the
superseded generation's key — including a recipient the rotation revoked — opens
the superseded copy in the live tree for as long as it is merely unreferenced.
`revocation stops this recipient from receiving any future key` is exactly that
narrow, and retirement is the operation that closes the window.

Retirement refuses unless the journal records the rotation complete and the unit
migrated, and unless the caller passes the descriptor the *store* now resolves
to and it differs from the superseded one. The retention crate cannot read the
store, so that argument is how a caller states that the canonical reference has
already moved; retiring on the journal alone would destroy the only object the
store could still name.

A retirement is not a deletion of the artifact. It leaves backup copies taken
before the rotation untouched and writes no tombstone: the artifact still exists,
at the locator the migration chain resolves to.

## The store database unit

The rotation plan carries a `STORE_DATABASE` unit. Its executor is `P2-K2`'s
`PRAGMA rekey`, which cannot link into `academic-retention`: the encrypted store
lane and the default lane are mutually exclusive builds. The engine therefore
refuses to record that unit as migrated rather than pretending to have moved it.

Its byte-level kill evidence is fault `EN01`
(`store_rekey_kill_leaves_exactly_one_working_key`), and since `P2-K5` the
`encrypted-store-lane` CI job runs that test on `ubuntu-latest` — so it is
executed evidence rather than a pointer. Native Windows is not in that job:
`openssl-src` needs a Perl the hosted Windows image does not carry, which t068
section 2.3-17 records; Windows stays the README-documented local lane with its
pinned interpreter.

## Recipient add and revoke

`keys/recipients.cbor` is `P2-K1`'s frozen document and nothing here changes its
shape. A change is one atomic write: bytes to a temporary file in the same
directory, synced, renamed over the target. `KY04` and `KY05` both require the
set on disk to be the old one or the new one and never a partial one, and a
rename within a directory either happens or does not.

**What revocation is:**

> revocation stops this recipient from receiving any future key; it does not
> erase plaintext that was already read, and it does not reach a copy taken
> while the recipient was live

That sentence is a constant, every surface repeats it unchanged, and it is
written into the journal so an audit of the file alone cannot read a revocation
as an erasure. `revocation_does_not_claim_prior_plaintext_erasure` fails if a
surface stops carrying it or if any library source starts claiming more.

Three independent things stop a revoked recipient from receiving the new key:
the rewrap iterates the *current* set, which the revocation already removed the
record from; every produced record is checked against the journal's revocation
history; and `add_recipient` reads the same history, so a caller minting a fresh
record under a revoked `recipient_id` is refused as an identity rather than as a
record. Revocation is a fact about an identity, not about one stored record.

A rotation touches the set twice, and the order is the point.
`rewrap_for_generation` adds the new generation's records **beside** the old
ones, so while units are still moving both generations are openable from the one
document the profile holds. `retire_generation` writes the set holding only the
new generation, and refuses while any planned unit is still under the old one —
a set holding only the new generation before then would name a key that opens
nothing for those units while the key that does open them is no longer on disk.

Revoking the last remaining recipient is refused. That is not access control, it
is data destruction.

`RevocationOutcome::still_under_revoked_generation()` names the exact objects the
revoked key still opens, which is `KY05`'s enumeration requirement.

## Crypto-shredding

An object's DEK exists in exactly one place: the 80-byte wrapped-DEK key slot at
offset `KEY_SLOT_OFFSET` (128) of its header, sealed under `KEK_d`. Destroying
that slot is the shred.

```text
slot := "ACOB-KEYSLOT-SHREDDED-V1" (24) | tombstone_digest (32) | zero (24)
```

It is one positioned write plus a sync. Every other byte, the file itself, and
its length are untouched.

**What a shred claims:** the ciphertext is unreadable. No key opens the object
afterwards — not the domain KEK it was sealed under, not a rotated generation's,
not one recovered from a backup — and the plaintext digest goes with it, because
the slot held both.

**What a shred does not claim:** that the file was deleted, that its bytes were
overwritten, or that a copy taken earlier was reached. `ObjectFormatError::Shredded`
says so in those words, and it is a distinct variant from `Aead` so an operator
report can tell "we destroyed this" from "this failed to authenticate".

A shredded object is not a corrupt object. An attacker who can write the marker
can equally overwrite the slot with noise, so the marker is an operator-facing
label rather than a security boundary; what it buys is that a deliberate shred
and a bit-rotted object have different reports.

`RB01` requires "shredded or intact". A kill before the write leaves the object
intact; a kill after it leaves the object shredded; a kill *during* the 80-byte
write has already destroyed the key even if the marker is incomplete, and
re-applying repairs the label. Re-application is idempotent, which is what makes
a resumed retention action safe.

## Backup tombstones

A backup holds `AEAD_CHUNKED_V2` objects byte for byte, so shredding the live
object does not reach the copy inside one. A tombstone closes that gap:

```text
<backup>/tombstones/<locator>.tombstone     # one JSON object, one atomic write
```

`restore_encrypted_profile` applies every tombstone the backup carries to the
objects it materialises, in the staging tree, after every object has been
authenticated and before the rename that publishes the restore. So no published
restore holds a key slot the profile it came from had destroyed, and a tombstone
that cannot be applied fails the restore instead of silently resurrecting an
artifact. **The re-deletion needs no key**: the locator lives in the clear at a
fixed header offset and destroying a key slot is a positioned write. A tombstone
whose object is not in the tree is reported as absent, not ignored — the
artifact may have been registered after the backup was taken, or shredded before
it.

`tombstones/` is the one path in a published backup the sealed manifest does not
cover, because a tombstone is written into a backup that was published and
sealed long before, and no re-seal is possible without the backup root.
`verify_encrypted_backup_directory` therefore excludes it from the inventory
comparison. That weakens nothing the manifest proved: every listed file is still
required, present, and digest-checked. What an added tombstone can do is destroy
a key slot on restore, and anyone who can write into the backup directory could
delete the object outright.

A crypto-shredded object does not stop a backup. Its `artifact_descriptor` row
is append-only and stays, so refusing the profile would make one that had ever
deleted an artifact permanently un-backupable. The object is copied as the
destroyed thing it is — the shred marker is inside the bytes and the ciphertext
digest covers it — and the restore digest-checks it without asking it to
authenticate.

`RB02` — a tombstone write that fails — makes the deletion `REPAIR_REQUIRED`
rather than `PARTIAL`. A deletion whose tombstone did not land is not "mostly
done"; it is one that will not re-apply on restore.

## Deletion plan and the four-word result

Seven derivative classes, closed and in this order:

`TRANSCRIPT`, `EMBEDDING`, `GRAPH_CLAIM`, `DOCUMENT`, `CACHE`, `REPLICA`,
`BACKUP_EXPIRY`.

Every plan carries one node per class, always. A class with nothing to delete is
a node saying so *with a reason*; a class the resolver could not answer for is a
node saying that, and it makes the action `REPAIR_REQUIRED` before any action
runs — a deletion that skipped a class it could not resolve would be reporting on
a subset of itself (`RB03`).

Results are exactly four words:

| Result | Meaning |
| --- | --- |
| `PLANNED` | a plan exists; nothing has been executed |
| `COMPLETE` | every planned action succeeded and nothing is left |
| `PARTIAL` | some succeeded; these exact locators are still there (`RB04`) |
| `REPAIR_REQUIRED` | an operator is needed; these are why (`RB02`, `RB03`) |

**"Mostly deleted" is not a result.** `PARTIAL` and `REPAIR_REQUIRED` carry an
`UnresolvedSet`, whose constructor refuses an empty list, and `COMPLETE` is
returned only when nothing is left. The exact locators reach the report and the
journal from the same list, so a preview cannot drift from what ran.

## `GATE-38-026` stays open

Whether non-instructor voices may be removed from an **original** recording, and
under whose authority, is a user decision. This build implements the mechanism
and selects no policy:

- `RetentionSubject::voice_spans_in_original` requires an
  `OriginalVoiceAuthority` the caller states. There is no constructor that omits
  it.
- `OriginalVoiceAuthority` implements no `Default` and no constant names one.
- A voice-scoped subject carries `GATE_38_026_STATEMENT`, so no surface can
  render such a plan as settled policy.
- `gate_38_026_ships_the_mechanism_and_selects_no_policy` and
  `rotation_engine_lane_is_not_default` both scan for the forbidden default
  spellings, the same shape `P2-K4` used for `GATE-38-031`.

## Fault coverage

| Fault | Outcome proved | Where |
| --- | --- | --- |
| `KY03` kill mid domain-KEK rewrap | exactly one of old/new KEK opens every object at each of four distinguishable on-disk states; the journal lists the remainder; resumable | `interrupted_rewrap_has_exactly_one_opening_key` |
| `KY04` kill during recipient add | the set on disk is byte-identical to the old one | `recipient_set_is_old_or_new_and_never_partial` |
| `KY05` kill during recipient revoke | as above, and the revoked recipient receives no new key while the objects still under the old key are named | same, plus `revoked_recipient_gets_no_new_key` |
| `RB01` kill during crypto-shred | intact before the write, shredded after; a torn write is repaired by re-application | `crypto_shred_kill_leaves_shredded_or_intact` |
| `RB02` backup tombstone write fails | no partial tombstone; the deletion is `REPAIR_REQUIRED` | `interrupted_backup_tombstone_leaves_no_partial_tombstone` |
| `RB03` derivative not found while planning | the deletion does not run and the node is named | `deletion_plan_enumerates_every_derivative_class` |
| `RB04` replica or cache purge partial | `PARTIAL` with the exact remaining locators | `partial_purge_reports_exact_remaining_locators` |

`RB01`'s failpoint lives in `academic-vault`, beside the key slot it destroys.
The `KY03` selector is `KY03:<stage>` — one fault-matrix row with four
distinguishable states, and no invented identifier.

t068 section 7 lists `RB02`–`RB04` under `P2-P2`; section 5 requires `P2-K5`'s
acceptance to cover `RB01`–`RB04`. Both are true: the mechanism and its outcomes
are proved here, and `P2-P2` replaces the synthetic resolver and executor with
real ones.

## Running it

```powershell
cargo clippy -p academic-retention --all-targets --locked --offline --features rotation-engine,phase2-fault-injection -- -D warnings
cargo test -p academic-retention --all-targets --locked --offline --features rotation-engine,phase2-fault-injection
```

Both are hosted CI steps on every Rust matrix label, because the key-slot write
and the recipient-set rename are per-platform. The default-lane half — the
journal, the plan, the vocabulary, and the revocation contract — is pure Rust and
also runs inside `cargo test --workspace`.
