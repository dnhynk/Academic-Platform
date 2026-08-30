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

No daemon command and no CLI command runs a rotation, and none may: the entry
points refuse. What follows describes machinery that is built and executed under
a lane, not behaviour a user or a product path can reach.

## Phase 2 does not accept a rotation

**Running a rotation is refused.** Seven entry points would drive one, and each
one refuses on its first line — before a journal is read, before a file is
opened, before a page is rewritten:

| refused | error |
|---|---|
| `RotationEngine::begin` | `EngineError::NotAccepted` |
| `RotationEngine::rotate_object` | `EngineError::NotAccepted` |
| `RotationEngine::rotate_store_database` | `EngineError::NotAccepted` |
| `RotationEngine::complete` | `EngineError::NotAccepted` |
| `engine::retire_superseded_object` | `EngineError::NotAccepted` |
| `recipients::rewrap_for_generation` | `RecipientError::NotAccepted` |
| `recipients::retire_generation` | `RecipientError::NotAccepted` |

**Deletion is not refused, and neither is anything a deletion needs.** These keep
working exactly as the rest of this document describes: `shred_with_tombstone`
and `EncryptedVault::shred_key_slot`; `BackupTombstone`,
`encrypted::rotation::deletion_tombstone`, `tombstone::write_into_backup` and
`read_from_backup`, and `apply_tombstones`; `backup_encrypted_profile`,
`restore_encrypted_profile`, and `verify_shredded_object`; the deletion plan,
`settle`, and the retention result vocabulary; `recipients::add_recipient`,
`revoke_recipient`, and `revoked_recipient_ids`; and every reader —
`RotationPlan::new`, `RotationState::replay`, `resume`, `probe_header`,
`observe_reachable_opening`. A rotation *plan* is data and still builds. What is
refused is running one.

`rotation::require_rotation_accepted` is the whole gate and the only place it is
decided. It takes no argument, reads no environment variable, and has no
debug-build branch; the non-default `rotation-orchestration` feature is the only
thing that turns it off, no product graph selects it
(`phase1-scaffold-policy.test.mjs`), and the rows that execute a rotation and the
rows that refuse one never link into one binary.
`the_rotation_gate_is_one_decision_with_no_flag_variable_or_debug_path` checks
the gate's whole text rather than a list of forbidden tokens, so an environment
read moved into a helper it calls, or a process-wide flag with a public setter,
is caught as well — both were built against the previous guard and neither
tripped it. `rotation_gate.rs` and
`encrypted_rotation_gate.rs` are the refusal, run everywhere; the
`rotation-orchestration-lane` CI job is where the machinery — the `KY03`-`KY05`
fault rows, the `T114` and `T116` seam closures — still runs.

### What the gate covers, and what stands beside it

**The gate wraps the journalled orchestration.** The seven entry points above
are the calls that read or append the rotation journal, and each refuses before
it does. It does not wrap the primitives a rotation composes, and it cannot:

| beside the gate | what one call does | why it is not behind the gate |
|---|---|---|
| `EncryptedVault::reseal` (`vault/src/encrypted.rs:452`) | re-seals one object into a destination vault's keyring — the object half of a rotation, without the journal entry that records it | `academic-vault` is below `academic-retention` in the crate graph, so calling the gate from it is a cycle in a graph that is machine-checked acyclic. It is also the vault's own primitive with its own `OB09` crash contract, exercised by `vault/tests/encrypted_crash.rs` with no rotation in sight. |
| `StoreDatabaseExecutor::rekey_store_database` (`portability/src/encrypted/rotation.rs:83`), and `cipher::rekey_encrypted_profile` (`store/src/cipher.rs:329`) behind it | rewrites every page of the profile database under the target generation's store key. The profile then opens under neither the generation the rest of it is under nor the one its backups were taken with, and the journal is empty | The irreversible write is `academic-store`'s. Every product binary links `academic-store` and none links `academic-retention`, so gating it would put this crate into the default product graph — the posture the first paragraph of this document rests on and `rotation_engine_lane_is_not_default` proves. Gating only the portability implementation would leave `rekey_encrypted_profile` reachable one step out, which is the same thing one layer down. |
| `AcceptanceStore::record_descriptor_migration` (`store/src/accept.rs:181`) | appends the chain row that moves an artifact's canonical reference | the same crate edge as above |
| `recipients::add_recipient`, `recipients::revoke_recipient` | key management, which is a fact whether or not a rotation follows | deliberate, and stated above |

So a caller that links the encrypted portability lane and drives the executor
directly performs **a rotation with no journal**: `rekey_store_database` returns
`Ok`, `open_store` then fails, and the rotation journal holds nothing to replay
or resume from. That is not a bypass of the gate; it is what
the gate does not claim, and until `P2-P2` there is no non-test caller of either
primitive. `the_primitives_a_rotation_composes_are_not_refused_beside_the_gate`
in `encrypted_rotation_gate.rs` executes it rather than leaving it as a sentence.

**Why.** The machinery is built and tested. What does not exist is the
orchestrator that would hold its obligations, and the fourth `P2-A1` audit
(`C:\Users\dongh\.claude\orchestration\run_98ccc873ba4b\t118-a1-crypto-admission-audit4.md`)
reached four states through the shipped API with no kill and no tampering. Phase
2 narrows the contract to what it can hold rather than repairing each state
under an orchestrator nobody has written; `P2-P2` selects the feature and closes
the list below.

### Known unresolved, for whoever reopens this

Each was reproduced over the shipped API, except **P3-G10**, which is a gap in
what a record states rather than a state to reach. **P1-F2**, **P2-F3**,
**P2-G3**, **P3-F5** and **P3-F6** need the gate to open first. **P2-F4** is a
vault-level state and **P3-G10** is on the deletion path, so both are reachable
today, and their rows say so.

| item | what happens | where |
|---|---|---|
| **P1-F2** a deletion lands inside an open rotation | `rotate_object` refuses the shredded source and writes nothing, `complete` refuses the remaining unit, the engine has no abandon, and the deletion path never reads the journal. The rotation cannot finish; `retire_generation` refuses forever; `rewrap_for_generation` leaves two generations of records and `recover_profile_keys` takes the first that opens, so **no backup of that profile restores**. | `engine.rs` `rotate_object`/`complete`, `recipients.rs` |
| **P2-F3** `rotate_object` does not bind its descriptor to its unit | `rotate_object(unit of A, descriptor of B)` is accepted; the journal records unit A migrated to B's target locator while A never moved. After `retire_generation`, no recipient on disk opens A. The one-line comparison `retire_superseded_object` already makes (`UnitDoesNotNameSupersededObject`) is the fix. | `engine.rs:399-424` |
| **P2-F4** `EncryptedVault::reconcile` is a fifth reader | Reachable **at the vault level**, with no engine, no plan and no journal, so unlike the rest of this table it is not behind the gate. It resolves each referenced descriptor with `validate_descriptor_locator`, so a shredded row under a rotated keyring fails the whole pass with `LocatorMismatch`. Giving it the `may_be_shredded` → `verify_shredded_object` branch backup and restore have closes it. Without a rotation the pass completes and reports the shred as `ReferencedCorruptRepairRequired` — recorded by `reconciliation_completes_over_a_profile_that_deleted_an_artifact`, and itself a state a shred and a bit-rotted object share. | `reconcile.rs:181-184`, `encrypted.rs` |
| **P2-G3** a rotation unit is identified by its locator, so a profile holding the same bytes in two lineages cannot be planned | `RotationUnit::object` derives `unit_id = SHA-256(domain ‖ kind ‖ **source_locator**)` and `RotationPlan::new` refuses a plan that names one unit twice. A locator carries no permission lineage, so registering one document twice in a domain gives two artifacts one locator and therefore one unit id: `RotationPlan::new` returns `DuplicateUnit` and the rotation never starts. Nothing is lost — the refusal is before any write — but the first orchestrator meets it on the shipped API, over exactly the profile shape the tombstone rows above are built on. The fix is to identify a unit by artifact (or by the four-tuple its path is), or to state that one unit moves every artifact sharing its locator, in order, and to say which here. | `rotation.rs:110-118` `RotationUnit::object`, `rotation.rs:271-279` `derive_unit_id`, `rotation.rs:429-436` `RotationPlan::new`; fifth `P2-A1` audit §4 (`C:\Users\dongh\.claude\orchestration\run_98ccc873ba4b\t121-a1-crypto-admission-audit5.md`) |
| **P3-F5** the database unit's *execution* order is not enforced | `RotationPlan::new` refuses a plan that orders it anywhere but last; the engine holds no state, so running it first is accepted and the store then records a chain row under a key it has moved away from. | `rotation.rs:400-405`, `engine.rs` |
| **P3-F6** a second `begin` over an open rotation | `AppendOnlyJournal::append` does not replay before appending, so a second `RotationStarted` makes every later replay `ConcurrentRotation` — permanently, because records are append-only. | `journal.rs:413-445`, `rotation.rs:533-540` |
| **P3-G10** the deletion path names its subject by locator everywhere except the tombstone record | `PlannedAction.locator`, `JournalEntry::RetentionPlanned.subject_locator`, and `JournalEntry::ArtifactShredded.locator` all identify one deleted object by its locator. Two registrations of the same bytes in one domain share it, so a profile that deleted both leaves two journal entries that differ only in `action_id` and `tombstone_digest` — and the obligation table in the next section tells an orchestrator to learn which artifacts are shredded by replaying `ArtifactShredded`. Nothing replays it today, and the digest does bind the artifact, so this is a gap in what the journal *states* rather than a collision: the tombstone record is the only place in the deletion path that names an artifact. Adding the artifact to these records is a journal format change and is left for whoever writes the executor. Found in `T122` while closing P1-G1, not by the fifth audit. | `plan.rs:180-216`, `entry.rs:142-160`, `engine.rs` `shred_with_tombstone` |
| **P3-F7** the obligation table below is what a caller is still held to | Two obligations are typed refusals and the rest are prose. The table states all of them. | this document |

## What the first orchestrator is bound by

Five rules were listed here as "only two left for a reader to remember", and
that was wrong: three more obligations were unlisted and the audit reached each
of them. The complete list, with what enforces it:

| rule | where it is enforced |
|---|---|
| the `STORE_DATABASE` unit is planned last | `RotationPlan::new`, `RotationError::StoreDatabaseNotLast` |
| the executor's two generations are the plan's two | `rotate_store_database`, `EngineError::StoreDatabaseExecutorGeneration` |
| only a unit the plan holds is moved under it | `rotate_object` / `rotate_store_database`, `EngineError::UnitNotInPlan` |
| **the two vaults an engine is built on are the plan's two generations** | **nothing — an obligation** |
| **a superseded object is retired before the next rotation begins** | **nothing — an obligation** |
| **the descriptor passed to `rotate_object` is the unit's own** | **nothing — an obligation (P2-F3)** |
| **the `STORE_DATABASE` unit is *run* after the last object** | **nothing — an obligation (P3-F5)** |
| **no `begin` while a rotation is open** | **nothing — an obligation (P3-F6)** |
| **no deletion settles while a rotation is open** | **nothing — an obligation (P1-F2)** |
| **shredded artifacts are left out of the plan** | **nothing — an obligation, and the means is not stated: the store holds no "shredded" row, so an orchestrator has to replay `ArtifactShredded` from the journal or read each header** |

`RotationEngine::new` takes two `EncryptedVault`s and cannot check either
against the plan *as it stands today*: a vault holds `KEK_d` and the locator key
derived from it, not the Vault Master Key, and a generation name is a function of
that master. An engine built on a target vault under some third generation moves
every object there, and the journal is *truthful* about it — an object unit
records the locator the reseal actually produced, and the store row is verified
against the object it names — so nothing downstream contradicts it. What is then
false is the plan: `RotationStarted` names a target generation no object is
under. `retire_superseded_object` still passes all four of its gates, because
every one of them compares the journal with the store rather than with a key, and
destroys the superseded copy; `retire_generation(kept = state.target())` keeps
the records for the generation the plan named. The profile that leaves opens
nothing. A backup refuses it (`LocatorMismatch` under the plan's target,
`EncryptedStoreLocked` under the objects' actual one), so it is caught before it
is carried anywhere — but only after the copies that would have opened it are
gone.

A derived binding is possible and simply is not built: `ProfileKeys` already
carries its `generation` and `ProfileKeys::keyring(master)` builds a keyring with
the master in hand, so a vault could be given the same kind of derived proof
`StoreDatabaseExecutor::generations` gives. What is true is that nothing carries
it today, not that nothing could.
`an_engine_outside_the_plans_generations_leaves_a_profile_no_backup_can_take`
executes the two backup refusals at the end of that sequence; the retirement and
`retire_generation` steps in the middle of it are argued here, not run.

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
which backup, restore, and export use; `resolved_artifact_descriptor`, which is
what a retirement's `CanonicalReference` reads; the store's pre-transaction sealing
closure (`preflight_artifact_closure`); and the acceptance transaction's own
closure writer, whose sealed receipts are checked against the descriptor it
loads. A reader that stopped at the signed row would refuse every batch whose
closure reaches a rotated artifact, under the new key because the row names the
superseded object and — once that object is retired — under both.

`EncryptedVault::reconcile` is a fifth reader, and it does not go through that
resolution: it is fed a referenced set by its caller and validates each
descriptor's locator against its own keyring. That is why it is in the open list
above — under a rotated keyring the shredded row's locator no longer derives and
the whole pass fails, where a backup and a restore fall back to
`verify_shredded_object`. **The encrypted lane has no product reconciliation
caller**: `core::local_service` reconciles the plaintext `Vault`. What decides
which object a rotated profile reaches is the chain above and the journal, not a
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
unit is planned last and must be *run* after the last object.

Those are two rules and only the first is enforced. `RotationPlan::new` refuses a
plan that orders the unit anywhere else (`the store database unit is planned at
position … and a rotation moves it last`), so the plan's order is a property of
the plan. The engine holds no execution state, so a caller that runs the database
unit first is accepted, and the store then records a chain row under a key the
database has moved away from — `page one did not authenticate`. Running it last
is an obligation (P3-F5), not a property.

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
<backup>/tombstones/<artifact-id>-<locator>.tombstone   # one JSON object, one write
```

**A tombstone names its artifact, and every locator that artifact has been
reachable under**: the one the live shred destroyed, and every locator the
store's `artifact_descriptor_migration` chain moved through before it, oldest
first. A locator is a function of `KEK_d`, so a rotation gives an artifact a new
one and a backup taken before that rotation holds the object under an older name;
a record naming only the current locator would leave that copy readable while
reporting nothing. `AcceptanceStore::superseded_locators` is where the chain
comes from and `encrypted::rotation::deletion_tombstone` is the product path
that builds the record. An artifact that never moved produces a record with an
empty list.

**A locator is not an identity.** It is
`HMAC(LOC_d, format || media_type || 0 || content_digest)` — no permission
lineage, no retention class — so one domain gives the same bytes the same locator
in every lineage, while the path
`objects/<domain>/<retention>/<lineage>/<xx>/<yy>/<locator>.aobj` keeps them
apart. Registering one document in two lineages is two artifacts, two paths, one
name. A re-deletion matching the locator alone therefore reaches whichever the
directory walk sees first — NTFS gives lexical order and ext4 gives hash order —
and it destroys a key slot the profile never deleted, or leaves the deleted
artifact readable, and reports the ordinary success either way. So the record
carries the 16-byte artifact id, which is cleartext at a fixed header offset like
the locator; `apply_tombstones` matches on both; and a match does not consume the
record, so one tombstone still reaches every name its own artifact has.

**The file is named for both, for the same reason.** A backup directory is a
flat namespace, so a name carrying only the locator makes the second of two such
deletions replace the first record instead of joining it — and a restore of
every backup taken before them republishes the artifact deleted first as
readable, while the receipt lists it as a copy the deletion deliberately spared.
Deleting two registrations of one document destroys two key slots and leaves two
files. A second record for the *same* artifact at the *same* locator does
replace the first, which is what re-writing a tombstone means and what `RB02`'s
repair relies on. Both halves of the name are re-encoded from the record's own
decoded bytes, so a record that is not 16 and 32 bytes of hex has no file name
at all rather than a caller-spelled one.
`two_tombstones_that_share_a_locator_are_two_files_and_two_records` is the file
layout on its own, in the default lane on every platform;
`two_tombstones_reach_both_deleted_artifacts_when_the_lower_lineage_goes_first`
and its two siblings are the same two deletions over a real object tree; and
`a_restore_keeps_both_deleted_artifacts_deleted_when_the_lower_lineage_goes_first`
and `…_when_the_higher_lineage_goes_first` are the product backup and the
product restore, for both deletion orders.

That is why `TOMBSTONE_VERSION` is `2`. A version 1 record named a locator and no
artifact and cannot be applied to the artifact it was written for;
`read_from_backup` refuses one by version rather than guessing.
`a_tombstone_reaches_its_own_artifact_when_the_deleted_lineage_sorts_first` and
`…_last` are the engine half over three lineages of one domain, and
`a_restore_re_deletes_only_the_named_artifact_when_its_lineage_sorts_first` and
`…_last` are the same three through the product backup and the product restore,
for both a pre-deletion and a post-deletion backup. Two orders, because one order
cannot be unfavourable on both filesystems.

`restore_encrypted_profile` applies every tombstone the backup carries to the
objects it materialises, in the staging tree, after every object has been
authenticated and before the rename that publishes the restore. So no published
restore holds a key slot the profile it came from had destroyed, and a tombstone
that cannot be applied fails the restore instead of silently resurrecting an
artifact. **The re-deletion needs no key**: the locator lives in the clear at a
fixed header offset, only the 208-byte header is read, and destroying a key slot
is a positioned write.

A tombstone that matched no object in the tree — under any of its artifact's
names — is reported, not ignored. `EncryptedRestoreReceipt` carries three sorted
lists, and **each names an artifact as well as a locator**, because a list keyed
by locator cannot tell two registrations of the same bytes apart: it would
report two re-deletions as one, and would let the record that found its object
answer for the record that found nothing. They are `re_deleted_objects`, the
artifacts actually re-deleted and the locator each was reached under;
`spared_objects`, the objects a record's locator reached whose artifact it does
not name, which the restore left readable on purpose; and `absent_tombstones`,
one entry per record that reached nothing. Absence is not an error: the artifact may have been registered
after the backup was taken, or shredded before it. `spared_objects` is not an
error either, and it is empty for every profile that never registered the same
bytes twice in one domain — what it says when it is not is that deleting one
registration of a document left an identical copy readable under another lineage,
which is a fact about the deletion and not about the restore. All three are facts
the caller is told rather than ones the receipt drops.

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
row for an object that does not exist either. An orchestrator therefore has to
leave shredded artifacts out of the plan, and their rows then keep the locator of
the generation they were destroyed under while every other row moves.

That is a rule an orchestrator can only keep **before it calls `begin`**. A
deletion that settles after it has no way to reach a plan already fixed in
`RotationStarted`, and the deletion path does not read the journal, so the
rotation is stranded (P1-F2 above). Nothing states how an orchestrator is
supposed to know which artifacts are shredded either: the store holds no such
row, and the fact lives in the journal's `ArtifactShredded` records and in each
object's header. Both are why running a rotation is refused in Phase 2. The
paragraphs below describe what a backup and a restore then meet, and those *are*
reachable: a deletion needs no rotation.

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
operator-facing label the marker is. A header that is not a destroyed key slot is
refused as the same `LocatorMismatch` it was, and a name with no file behind it as
the read that failed.
`a_deletion_before_a_rotation_still_backs_up_and_restores` is the whole chain:
delete, rotate what is left, back up, restore, and restore the pre-deletion
backup the tombstone was written into.

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

`KY03` and the `revoked_recipient_gets_no_new_key` half of `KY05` drive
`rotate_object` and `rewrap_for_generation`, which Phase 2 refuses, so those two
run in the `rotation-orchestration-lane` job rather than in the default graph.
`KY04`, the rest of `KY05`, and `RB01`-`RB04` are outside the gate and run
everywhere they did.

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
also runs inside `cargo test --workspace`. What those two commands execute of a
rotation is the **refusal**: `rotation_gate.rs` calls all seven entry points and
asserts each one refuses, that nothing is journalled, and that the sequences the
fourth audit used stop at their first call.

The machinery itself runs with the lane selected, which is the hosted
`rotation-orchestration-lane` job and nothing else:

```powershell
cargo clippy -p academic-retention --all-targets --locked --offline --features rotation-engine,rotation-orchestration,phase2-fault-injection -- -D warnings
cargo test -p academic-retention --all-targets --locked --offline --features rotation-engine,rotation-orchestration,phase2-fault-injection
cargo clippy -p academic-portability --no-default-features --features encrypted-portability-rotation,phase2-fault-injection --all-targets --locked --offline -- -D warnings
cargo test -p academic-portability --no-default-features --features encrypted-portability-rotation --locked --offline
```

The half that needs the store is in the encrypted portability lane, because that
is the only build where `academic-store`, `academic-vault`, and
`academic-retention` link into one process:

```powershell
cargo test -p academic-portability --no-default-features --features encrypted-portability --locked --offline
```

`encrypted_rotation.rs` states what a rotation does; `encrypted_rotation_seam.rs`
states what everything after one does — the acceptance, the backup, the restore,
the deletion before a rotation and after one, the retirement, the executor's
generations, and the re-rotation. The rows in them that run a rotation are behind
`encrypted-portability-rotation` and run in the `rotation-orchestration-lane`
job; the rest — the deletion, the tombstone, the backup, the restore — stay in
`encrypted-portability-lane`, and `encrypted_rotation_gate.rs` runs there too:
the whole product rotation sequence refused against a real profile, and a
deletion reaching a real backup while it is.

Two of this contract's statements are held by tests outside those two files,
because the crate that owns each gate is where a reverted gate has to bite:
`retention/tests/rotation_seam.rs` refuses an executor outside the plan's
generations without a store, and `retention/tests/rotation.rs` holds the plan's
ordering rule and the unit-in-plan gate. `vault/tests/encrypted_objects.rs` holds
what `verify_shredded_object` requires of the file it hands back, and what a
reconciliation pass says about a profile that deleted an artifact.
