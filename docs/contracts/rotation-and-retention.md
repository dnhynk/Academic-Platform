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
`crates/portability/tests/encrypted_rotation.rs` and
`crates/portability/tests/encrypted_rotation_seam.rs`, and by nothing a user can
invoke. So there is no orchestrator to run the preflight the re-rotation section
below requires. `P2-P2` is the task that writes it.

**What the first orchestrator is bound by.** Four rules, and the difference
between them matters, because only one is left for a reader to remember:

| rule | where it is enforced |
|---|---|
| the `STORE_DATABASE` unit is planned last | `RotationPlan::new`, `RotationError::StoreDatabaseNotLast` |
| the executor's two generations are the plan's two | `rotate_store_database`, `EngineError::StoreDatabaseExecutorGeneration` |
| only a unit the plan holds is moved under it | `rotate_object` / `rotate_store_database`, `EngineError::UnitNotInPlan` |
| **a superseded object is retired before the next rotation begins** | **nothing — this one is an obligation** |

The last one is the only one a type does not hold to.
`retire_superseded_object` finds its unit in the *latest* rotation
`RotationState::replay` returns, so after a second rotation the first rotation's
superseded objects can no longer be retired and each stays openable under the
first generation's key, in the live tree, for as long as the profile exists. An
orchestrator that rotates twice without retiring in between has left them there
and nothing will refuse it.

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

Entries are a closed set of eleven: `RotationStarted`, `UnitResealed`,
`UnitMigrated`, `UnitSourceRetired`, `RotationCompleted`, `RecipientAdded`,
`RecipientRevoked`, `RetentionPlanned`, `ArtifactShredded`,
`BackupTombstoneWritten`, `RetentionSettled`.

`UnitResealed` carries a `target_locator`, which for an object unit is the
locator the re-sealed object landed on. A `STORE_DATABASE` unit is rekeyed in
place and has no locator, so what it records there is
`store_database_target_id(profile, target generation)` — same width, same
cleartext class, and a pure function of two values the journal already carries.

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
chain.

**Every reader that decides which object is reachable resolves this chain.**
There are four, and all four go through
`descriptor_migration::resolve_with_stored_migrations`: `read_artifact_descriptors`,
which backup and restore use; `resolved_artifact_descriptor`, which is what a
retirement's `CanonicalReference` reads; the store's pre-transaction sealing
closure (`preflight_artifact_closure`); and the acceptance transaction's own
closure writer, whose sealed receipts are checked against the descriptor it
loads. A reader that stopped at the signed row would refuse every batch whose
closure reaches a rotated artifact, under the new key because the row names the
superseded object and — once that object is retired — under both.

The vault's reconciliation is fed a referenced set by its caller and resolves
nothing itself. **The encrypted lane has no product reconciliation caller**:
`core::local_service` reconciles the plaintext `Vault`. What decides which
object a rotated profile reaches is the chain above and the journal, not a
reconciliation pass.

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

**The window closes only before the next rotation.** Gate 1 below reads the
rotation `RotationState::replay` returns, which is the latest one in the journal,
so once a second rotation is recorded the first one's units are no longer the
ones a retirement can find and it refuses with "the journal does not record the
unit as migrated". The first generation's copies then stay openable under the
first generation's key indefinitely. This is the one ordering rule in this
contract that no type holds the caller to.

Every argument is bound to something already recorded, because the write cannot
be undone and — until an orchestrator exists — this is the only garbage
collection there is. A retirement refuses unless all four hold:

1. the journal records the rotation complete and this unit migrated;
2. `superseded` is the object **this unit** supersedes. A unit identity is
   derived from its source locator, so the two are comparable without reading
   anything, and without this the gates inspect one object while the positioned
   write destroys another;
3. the canonical reference the store resolves that artifact to is the
   `target_locator` this unit's `UnitResealed` record named. The reference is
   *read*, through `CanonicalReference` — `academic-retention` cannot link the
   store, so the encrypted portability lane binds
   `StoreCanonicalReference` over the store's own resolution. A caller whose
   store row is not written yet is refused rather than believed;
4. the artifact is in the store at all. "Nothing says it moved" is not "it
   moved".

Those refuse the three orderings that destroyed live data before this: retiring
before the store row exists, retiring another artifact's live object under a
rotated unit, and retiring the object the rotation moved *to*.

A retirement is not a deletion of the artifact. It leaves backup copies taken
before the rotation untouched and writes no tombstone: the artifact still exists,
at the locator the migration chain resolves to.

## The store database unit

The rotation plan carries a `STORE_DATABASE` unit, and it is the reason a
rotation is not "a rotation of the objects". `SKEY_p` and `KEK_d` are both
functions of the Vault Master Key, so a rotation that moved every object and
left the database behind produces a profile whose two halves are under two
generations. That profile still works on the machine holding both keys, and no
backup of it can be restored: a restore recovers one master and derives both
halves from it.

Its executor is `P2-K2`'s `PRAGMA rekey`, which cannot link into
`academic-retention` — the encrypted store lane and the default lane are
mutually exclusive builds. `RotationEngine::rotate_store_database` therefore
takes a `StoreDatabaseExecutor`, and the encrypted portability lane binds it:
`encrypted::rotation::StoreDatabaseRekey` derives both generations' store keys
and calls `academic_store::cipher::rekey_encrypted_profile`. A plan that reaches
this unit with no executor run still refuses to complete, by name.

```text
open under the source key  ->  PRAGMA rekey  ->  reopen under the target key
UnitResealed(target = store_database_target_id(profile, target generation))
UnitMigrated
```

The order differs from an object unit's for one reason: a rekey rewrites pages
in place, so there is no second file that is durable and verified while the
first is still reachable. What takes the place of the read-back is the
executor's own reopen under the target key, which re-asserts the format marker,
the frozen SQLCipher settings, and the schema-2 identity before it returns.
`UnitResealed` is appended only after that. Because a rekeyed database does not
move, the record's `target_locator` is
`SHA-256("academic-os/rotation-store-database/v1" | 0 | profile_id | 0 | target
generation)` — 64 hex like every other locator field, and a pure function of two
values the journal already carries.

**The database moves last.** While objects are still moving, the store key that
opens the database has to stay in force for both the migrated and the unmigrated
ones — `record_descriptor_migration` opens the store to write each move — so the
unit is planned last and run after the last object. `RotationPlan::new` refuses a
plan that orders it anywhere else (`the store database unit is planned at
position … and a rotation moves it last`), so this is a property of the plan
rather than a rule an orchestrator has to remember.

**The executor is checked against the plan before it runs.** The records this
unit appends are pure functions of the plan — the target is
`store_database_target_id(profile, plan target)` — so an executor holding some
other pair of masters would move the database out from under both generations the
journal then names, and no later read of the journal could tell. A rekey is not
undone by reading anything afterwards, so `StoreDatabaseExecutor::generations`
reports the pair the executor holds, `rotate_store_database` compares it with the
plan's, and a mismatch refuses before a page is rewritten.

**A unit outside the plan is refused.** `RotationState::replay` resolves every
journalled unit against the plan, so a record written for a unit the plan does
not hold makes the whole journal unreplayable and the rotation impossible to
complete — permanently, because the records are append-only, and with no kill
anywhere in it. `rotate_object` and `rotate_store_database` both refuse such a
unit before the first record.

A kill during the rekey leaves exactly one working key, which is fault `EN01`
(`store_rekey_kill_leaves_exactly_one_working_key`); since `P2-K5` the
`encrypted-store-lane` CI job runs that test on `ubuntu-latest`, so it is
executed evidence rather than a pointer. Native Windows is not in that job:
`openssl-src` needs a Perl the hosted Windows image does not carry, which t068
section 2.3-17 records; Windows stays the README-documented local lane with its
pinned interpreter. The resume is the same call: a database `current` no longer
opens but `next` does is reported as already at the target, and the journal
records catch up.

A kill between the two records is repaired the same way an object unit's is, but
the state in that window does not read the same. `RotationState` reports
`opening_generation = source` for a unit that is resealed and not yet migrated,
which is true of an object — both copies are on disk — and false of the database,
which the rekey has already moved to the target. No product code consumes that
value, and the resume is the executor's own `AlreadyAtTarget` rather than
anything read from it.

A backup refuses a key set and a master that name two generations
(`backup profile key generation`). That guard compares the two *arguments* a
caller assembled, `keys.generation()` against `master.generation_id()`, so what
it catches is a caller pairing a rotated key set with a superseded master. A
profile whose halves are actually split on disk is refused too, but by the halves
themselves: under the superseded generation the objects no longer derive
(`LocatorMismatch`), and under the target one the database does not open
(`EncryptedStoreLocked`). Both are fail-closed and neither publishes a directory;
the named guard is not what fires.

## Re-rotation, and the locator a chain records once

`artifact_descriptor_migration` carries `UNIQUE (artifact_id, vault_locator)`
and `UNIQUE (artifact_id, superseded_locator)`. A locator is a deterministic
function of the generation, so **an artifact cannot be rotated back to a
generation its chain has already recorded.** `G1 -> G2 -> G1` is refused at the
second rotation's store row.

That constraint is what stops a chain from forking or looping, and migration
`0005` is frozen, so the sequence is refused rather than the schema widened.
What the refusal must not do is arrive late: the journal records `UnitResealed`
and `UnitMigrated` before the store row is written, so a rotation discovering
this at the insert leaves a journal that says the unit migrated and a store that
still resolves to the superseded object — a divergence with no kill in it.

**An orchestrator must ask before it journals anything.**
`AcceptanceStore::locator_is_already_in_chain` is that question, and
`record_descriptor_migration` refuses by name — "the artifact's reference chain
has already recorded this locator" — rather than surfacing a raw constraint
violation. A rollback that has to be re-advanced rotates to a *new* generation,
which is what the key schedule makes cheap: a generation is a fresh Vault Master
Key, not a slot.

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
new generation.

Both are narrower than "write what the caller asked for".

A rewrap re-wraps *one recipient's own copy* of the key, so a produced record
must carry the identity it was produced for; a record under another identity is
another recipient, and appending it beside the survivors would add a reader
nothing authorized. And a rewrap runs once: between it and `retire_generation`
each identity has exactly two records, so a third is a rewrap of a rewrap and is
refused. `recipients.cbor` is `P2-K1`'s frozen document and a record does not say
which generation it wraps, so that count is what can be checked without a key.
After a kill between the set's rename and the journal record the set on disk is
already the rewrapped one, and the resume is `retire_generation`, not a second
rewrap.

`retire_generation` is the last point at which a profile can be made permanently
unopenable. **Which generation it keeps is decided by the journal**: it refuses
unless the journal records a rotation, that rotation is complete with no unit
remaining, and `kept_generation` is the generation the rotation moved **to**.
Without the first, nothing says the generation being kept opens anything and
every object may be under the one being removed; without the third, the call is
that same destruction stated backwards.

**Which records survive is still the caller's `keeps` predicate**, and that is
not the same statement. `recipients.cbor` is `P2-K1`'s frozen document and a
record does not say which generation it wraps, so no key-free check in this crate
can read a record and answer the question; the caller — which has just finished
rotating to that generation — answers it by opening one. A caller that names the
right generation and then keeps the superseded records is accepted, and the
profile it leaves opens nothing that is reachable. Recording each produced
record's digest in `RecipientAdded` would move the choice into the journal, and
it is not done here because every path that puts a record on `recipients.cbor` —
profile creation included — would have to journal one first, or the selection
would silently drop records and become the destruction it is meant to prevent.
Until then this is an obligation on the caller, and `rewrap_for_generation`'s
identity and count gates are the whole of what bounds it.

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

**A tombstone names every locator its artifact has been reachable under**: the
one the live shred destroyed, and every locator the store's
`artifact_descriptor_migration` chain moved through before it, oldest first.
A locator is a function of `KEK_d`, so a rotation gives an artifact a new one and
a backup taken before that rotation holds the object under an older name; a
record naming only the current locator would leave that copy readable while
reporting nothing. `AcceptanceStore::superseded_locators` is where the chain
comes from and `encrypted::rotation::deletion_tombstone` is the product path
that builds the record. An artifact that never moved produces a record with an
empty list, which serializes to exactly the bytes this format wrote before the
field existed.

`restore_encrypted_profile` applies every tombstone the backup carries to the
objects it materialises, in the staging tree, after every object has been
authenticated and before the rename that publishes the restore. So no published
restore holds a key slot the profile it came from had destroyed, and a tombstone
that cannot be applied fails the restore instead of silently resurrecting an
artifact. **The re-deletion needs no key**: the locator lives in the clear at a
fixed header offset, only the 208-byte header is read, and destroying a key slot
is a positioned write.

A tombstone that matched no object in the tree — under any of its artifact's
names — is reported, not ignored. `EncryptedRestoreReceipt` carries two sorted
lists: `re_deleted_locators`, the locators actually re-deleted, and
`absent_locators`, the tombstones that reached nothing. Absence is not an error:
the artifact may have been registered after the backup was taken, or shredded
before it. It is a fact the caller is told rather than one the receipt drops.

`tombstones/` is the one path in a published backup the sealed manifest does not
cover, because a tombstone is written into a backup that was published and
sealed long before, and no re-seal is possible without the backup root.
`verify_encrypted_backup_directory` therefore excludes it from the inventory
comparison. That weakens nothing the manifest proved: every listed file is still
required, present, and digest-checked. What an added tombstone can do is destroy
a key slot on restore, and anyone who can write into the backup directory could
delete the object outright.

A crypto-shredded object does not stop a backup, and a rotation after the shred
does not either. Its `artifact_descriptor` row is append-only and stays, so
refusing the profile would make one that had ever deleted an artifact permanently
un-backupable. The object is copied as the destroyed thing it is — the shred
marker is inside the bytes and the ciphertext digest covers it — and the restore
digest-checks it without asking it to authenticate.

**A shredded object cannot be rotated, and its reference cannot move.** A reseal
opens the source object and a destroyed key slot opens for nobody, so a plan that
names one cannot complete; `record_descriptor_migration` will not write a chain
row for an object that does not exist either. The orchestrator therefore leaves
shredded artifacts out of the plan and their rows keep the locator of the
generation they were destroyed under while every other row moves.

That is the state a backup then meets. A locator is a function of `KEK_d`, so the
rotated keyring derives a different one for that row and `validate_descriptor_locator`
refuses it as a `LocatorMismatch` before reading a byte — which is right for a
live object and, left alone, would refuse every later backup of that profile.
`EncryptedVault::verify_shredded_object` is what tells the two apart: it reads the
header at the descriptor's own name and returns its path only if the shred marker
is there and every cleartext identity field matches the descriptor. That is more
than the keyed path checks for a shredded object, since a destroyed slot stops
that read before `require_matches` runs, and none of it is authenticated — the
wrap that authenticated those bytes is what the shred destroyed, so it is the same
operator-facing label the marker is. Anything else stays the locator mismatch it
was. `a_deletion_before_a_rotation_still_backs_up_and_restores` is the whole
chain: delete, rotate what is left, back up, restore, and restore the
pre-deletion backup the tombstone was written into.

There is no encrypted export. `export_profile` is the Phase 1 plaintext lane over
`Vault`, which has no key generations and no crypto-shred, so nothing in this
section reaches it.


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

The half that needs the store is in the encrypted portability lane, because that
is the only build where `academic-store`, `academic-vault`, and
`academic-retention` link into one process:

```powershell
cargo test -p academic-portability --no-default-features --features encrypted-portability --locked --offline
```

`encrypted_rotation.rs` states what a rotation does; `encrypted_rotation_seam.rs`
states what everything after one does — the acceptance, the backup, the restore,
the deletion before a rotation and after one, the retirement, the executor's
generations, and the re-rotation. Both run in the hosted
`encrypted-portability-lane` job.

Two of this contract's statements are held by tests outside those two files,
because the crate that owns each gate is where a reverted gate has to bite:
`retention/tests/rotation_seam.rs` refuses an executor outside the plan's
generations without a store, and `retention/tests/rotation.rs` holds the plan's
ordering rule and the unit-in-plan gate. `vault/tests/encrypted_objects.rs` holds
what `verify_shredded_object` requires of the file it hands back.
