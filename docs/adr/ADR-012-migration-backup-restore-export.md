# ADR-012: Migration, backup, restore, and export

- Status: Proposed decision register

## Registered direction

Canonical schema migration is forward-only and transactional with preflight space/version checks, backup, integrity verification, and resume/repair state for large transformations. Historical signed event bytes and signatures are never rewritten; pure upcasters decode old payloads into current in-memory representations, while semantic corrections append new events.

Projection migrations build generations side-by-side and atomically switch an active pointer. Vault format readers support N and N-1 while writers emit N; re-encryption seals and verifies a new object before a descriptor migration event switches reachability. That event is the existing v3 arm `RETENTION_ACTION_RECORDED`, and the move it authorizes is an appended `artifact_descriptor_migration` row rather than an edit — see below.

Backup fixes a commit watermark and ledger heads, creates a consistent encrypted DB snapshot, closes all reachable object descriptors, verifies ciphertext/digests, and signs/encrypts a manifest with a recovery recipient independent of the live OS wrapper. Restore always targets a new empty profile and verifies keys, DB, ledger chains, object closure, replay, and counts before an atomic profile switch.

Full export is documented, versioned, and vendor-neutral: original artifacts or a documented encrypted bundle, entity/claim/event/evidence records, original signed envelopes, schemas/predicate registry, and a human-readable inventory. Projection export is optional.

## Implemented for the synthetic Phase 1 local core

The original deterministic signed envelope is committed and verification rejects byte drift. The synthetic Phase 1 schema is built by forward-only migrations 0001–0003 and admitted before the daemon listens: `0001` is the canonical store at schema version 1, and `0002`/`0003` are the projection sidecar, which is a separate database with its own version sequence. A Phase 1 profile is not upgradable — `schema_meta` pins `schema_version = 1` and every mutation of it aborts — so a later schema version is a new profile format created only by the encrypted lane, never a conversion.

Migration `0004` is committed but is not part of that lane. It adds the typed, INSERT-only closure tables for the eighteen event schema v3 aggregate registration arms and widens the `ledger_event.event_kind` CHECK to admit them, on top of store schema version 2. Migration `0005` follows it with `P2-K5`'s typed columns for the `RETENTION_ACTION_RECORDED` aggregate — the `artifact_descriptor_migration` table below — which is the shape `0004` reserves for each aggregate owner. Migration `0003_phase2_encrypted_identity.sql` establishes that version together with the encrypted lane, and that lane's `STORE_MIGRATION_SQL` is `0001`, `0003`, `0004`, `0005`: an encrypted profile carries the closure tables and those typed columns out of its creation transaction and admission fingerprints all four steps, because the fingerprint is exact structural equality against exactly that set. `academic_store::migration::apply_aggregate_migration_pre_listen` remains the forward-only, pre-listen boundary for a schema-2 base assembled outside that runner, and it fails against a profile that already carries `0004`. Widening a CHECK is the one place SQLite forces a table rebuild, so `0004` rebuilds `ledger_event` by SQLite's documented copy-and-rename procedure: every canonical row is preserved byte-for-byte, the rebuild runs only on the maintenance connection, which installs no authorizer, and `foreign_key_check` and `integrity_check` both pass before the profile is admitted. `0004` reads and writes no part of the schema identity — not `schema_meta`, not `application_id`, not `user_version` — so applying it can never convert one profile format into another. The portability boundary produces deterministic open exports of the synthetic subset, takes a consistent plaintext SQLite backup with reachable vault objects and a verified manifest, and restores a verified backup only into a new empty profile before rebuilding projections from empty. Export and backup open the canonical source read-only and exclude projections; restore never activates over an occupied or daemon-owned destination.

The portability boundary carries a second, non-default lane, whose complete layout, digests, and refusal order are [encrypted backup and recovery](../contracts/encrypted-backup-and-recovery.md). `encrypted-portability` snapshots the SQLCipher profile with the Online Backup API into a destination that is keyed before its first page is written, closes over every reachable `AEAD_CHUNKED_V2` object, and seals a manifest v2 under a backup root that is generated for the backup and wrapped only by recovery-class recipients. That root has no derivation edge from the Vault Master Key, so a device wrapper that unwraps the VMK still cannot open a manifest; `DEVICE_ONLY` therefore has no backup key and no backup at all, which is what its loss statement says. The manifest carries two digests because only one of them can be stable: SQLCipher re-encrypts every page it writes, so two snapshots of one unchanged database differ byte-for-byte, and the digest two backups at one watermark must share covers the schema identity, the watermark, the counts, the device heads, the canonical semantic digest, and each object's logical identity and plaintext digest — not the file inventory. Restore targets only a new empty destination, recovers the Vault Master Key from the profile's own recovery-class recipient records inside the sealed manifest rather than from any device broker, and re-derives the schema identity, watermark, counts, device heads, canonical semantic digest, signed-batch replay, and object closure from the restored bytes before publishing; a manifest re-sealed with the real backup key after its counts were altered still fails. Projections are not restored: they are disposable and the encrypted lane links no projection engine. Before the first real ingest of any kind, `<profile>/admission/rehearsal.cbor` must record a completed drill authenticated under the profile's own key and naming both the monotonic key-material generation and a digest of the recipient set; any key-material change makes it stale and ingest is refused until a new drill runs. `BK01`-`BK04` and `RS01`-`RS04` are re-run under encryption, and `BK03` — which Phase 1 could not reach with a one-artifact corpus — is reached and asserted.

The Phase 1 formats are synthetic, and the encrypted lane is a *format*, not acceptance. ADR-012 remains Proposed: production key closure, RPO/RTO evidence, interrupted large-migration recovery, and the full vendor-neutral export/import round trip remain acceptance gates, and the encrypted bundle and its independent recovery recipient are implemented and drilled but not yet accepted, because acceptance needs the ADR-002 platform evidence `P2-H1` has not produced. Which recovery profile is in force is `GATE-38-031` and stays a blocking user choice with no default. The default lane remains `storage_encryption=NONE` and cannot accept production data.

## Phase 0 admission evidence (historical, before the Phase 1 local core)

Admission is read-only and rejects before mutation. Identity, integrity, foreign keys, complete user-object emptiness at version zero, and an exact reference-derived structural fingerprint are all checked before the FTS5 probe, journal-mode configuration, exclusive locking, or a migration transaction. That fingerprint excludes an exact enumeration of the objects SQLite creates itself — the tables `sqlite_sequence` and `sqlite_stat1` through `sqlite_stat4`, and the `sqlite_autoindex_*` indexes — and treats every other object as a user object regardless of its type or its name. The reserved `sqlite_` prefix is not evidence of ownership: only `CREATE` applies SQLite's reserved-prefix rejection, so a `sqlite_`-prefixed table, index, trigger, or view written directly into `sqlite_schema` is loaded and used like any other object, and excluding by name prefix would hide it. The projection sidecar applies the same enumeration.

The structural fingerprint is a startup check, and it is not the only schema check. A writer records SQLite's schema cookie when it is admitted and compares it again inside every acceptance transaction, after `BEGIN IMMEDIATE` has taken the write lock. The cookie is the same signal SQLite uses to reload `sqlite_schema` on an already-open connection, so an acceptance cannot commit against a schema that changed under the running writer; such an acceptance fails closed and consumes no batch, receipt, acceptance sequence, or revision.

Rejection preserves durable content, not the whole file family. The main database keeps its exact bytes and an existing WAL keeps its committed frames, because read-write admission handles disable checkpoint-on-close for the admission window; otherwise closing a rejected handle would checkpoint a tampered WAL into the main database. SQLite's own read and recovery path may still create an empty `-wal` and create or rewrite the rebuildable `-shm`; neither carries committed content, so both are outside the claim.

The window ends where admission does. A handle that is admitted restores checkpoint-on-close, so the WAL steady state is unchanged from before that protection existed: when the last connection to a profile closes cleanly, SQLite checkpoints and removes `-wal` and `-shm`, and the main database alone is a complete database at rest. Only an unclean exit leaves committed content in `-wal`.

A rollback-journal-mode input keeps the whole family byte-identical only when it carries no hot journal. A hot `-journal` is SQLite's own recovery record, and SQLite rolls it back on the first read of a read-write handle, before any admission statement can run. On such an input the read-only reader path refuses the rollback and leaves every family member byte-identical, while the read-write maintenance path restores the main database to its last committed bytes and deletes the `-journal`. Nothing else in the family changes on either path and no committed content is lost.

## A re-sealed object's reference moves by appending, not by editing

`artifact_descriptor.vault_locator` is inside the signed `ARTIFACT_REGISTERED` payload and the table is INSERT-only twice over, so a `P2-K5` rotation cannot write the new locator over the one the signature covers. Migration `0005` adds `artifact_descriptor_migration`: one appended row per move, keyed by the `retention_action_id` of the `RETENTION_ACTION_RECORDED` event that authorizes it, carrying the superseded locator, the new locator, the format version, the chain position, and a record digest. Readers resolve an artifact's current locator by walking that chain from the signed one.

No new event kind and no nineteenth v3 arm: t068 section 3.8 fixes the arm list at eighteen, and the registration frame's optional provenance digest is what binds one event to one exact move. Two triggers enforce that binding — one refuses a row whose `record_digest` is not the `source_digest` its retention action carries, one refuses a row that does not continue the chain — and the authorizer denies `UPDATE`, `DELETE`, `DROP`, and `ALTER` over the table exactly as it does over every other canonical one. The event is accepted first and the row is written second, so a kill between them leaves the reference where it was and the migration re-runnable.

`RETENTION_ACTION_RECORDED` is consequently the one v3 arm the encrypted lane's acceptance admits. The other seventeen have no writer yet and are still refused with `UnstorableEventKind` before any SQL runs, and the plaintext lane refuses all eighteen because it applies neither migration.

Retiring the superseded object is the collection point for what a rotation supersedes; `ADR-004`'s open item is the vault's `quarantine/` directory, which is a different thing and still the daemon lane's. `academic-retention`'s `retire_superseded_object` destroys the superseded object's key slot once the rotation is complete, the unit is migrated, the superseded object is the one that unit supersedes, and the store — read through `CanonicalReference`, not stated by the caller — resolves that artifact to the locator the journal recorded as the unit's target. Until then the superseded copy stays readable under the superseded key, which is stated in [rotation and retention](../contracts/rotation-and-retention.md) rather than left implied.

## Deletion reaches the copies a backup holds

A backup holds `AEAD_CHUNKED_V2` objects byte for byte, so crypto-shredding the
live object does not reach the copy inside one. `P2-K5` closes that with a
backup tombstone: one JSON record per deleted artifact, written into
`<backup>/tombstones/<locator>.tombstone` with a single atomic write, and applied
to the object tree a restore materialises.

A record names every locator its artifact has been reachable under — the one the
live shred destroyed and every locator the store's migration chain moved through
before it. A locator is a function of `KEK_d`, so a rotation renames an
artifact, and a backup taken before that rotation holds the object under the
older name; a record naming only the current locator would leave that copy
readable.

`restore_encrypted_profile` is what applies them: in the staging tree, after
every object has been authenticated and before the rename that publishes the
restore, so no published restore holds a key slot the profile it came from had
destroyed. The re-deletion needs **no key** — the locator is cleartext at a
fixed header offset and destroying a key slot is a positioned write. A tombstone
that reached no object under any of its artifact's names is reported rather than
ignored: `EncryptedRestoreReceipt` carries `re_deleted_locators` and
`absent_locators` as two sorted lists, so a deletion the backup could not carry
out is distinguishable from one it did. Absence is not an error — the artifact
may have been registered after the backup was taken, or shredded before it. A
tombstone write that fails makes the deletion `REPAIR_REQUIRED` rather than
`PARTIAL` — a deletion whose tombstone did not land is one that will not
re-apply on restore.

Two consequences follow for the backup boundary itself. `tombstones/` is the one
path a published backup carries that its sealed manifest cannot list, because
the tombstone is written long after the manifest was sealed and no re-seal is
possible without the backup root; `verify_encrypted_backup_directory` excludes it
from the inventory comparison and still requires and digest-checks every listed
file. And a crypto-shredded object does not stop a backup: its append-only
descriptor row stays, so the object is copied as the destroyed thing it is and
the restore digest-checks it without asking it to authenticate.

The record shape, the retention result vocabulary, and the fault rows are in
[rotation and retention](../contracts/rotation-and-retention.md).

## Acceptance gate

Fixture for each supported schema version; interrupted large migration resume; restore only to empty destination; object/ledger/key closure; independent fresh-profile restore; and vendor-neutral export/import round trip.
