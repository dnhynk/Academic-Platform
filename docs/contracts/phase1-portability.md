# Phase 1 portability: export, backup, restore

## Posture

Every artefact described here is plaintext and synthetic-only. The backup format
protects nothing: it is not encrypted, it is not confidential, and it is not
evidence for ADR-002, ADR-004, ADR-005, or ADR-012. Real or production data is
forbidden until ADR-002 is accepted, and nothing in this contract changes that.

Every manifest repeats the frozen policy object and fails closed if it differs:

```json
{
  "data_policy": "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
  "storage_mode": "PLAINTEXT_TEMPORARY_SQLITE",
  "storage_encryption": "NONE",
  "production_data_allowed": false,
  "product_network": "NONE"
}
```

Projections are disposable generations rebuilt from the ledger. They are never
export content, never backup content, and never restore authority.

`academic-portability` carries two mutually exclusive lanes, for the same
reason `academic-store` does: the plaintext synthetic lane and the encrypted
lane cannot link into one binary. This contract is the default
`plaintext-portability` lane. The non-default `encrypted-portability` lane is a
different format with a different manifest, a different posture block, and no
projection rebuild; it is described in
[encrypted backup and recovery](encrypted-backup-and-recovery.md).

## Deterministic open export

`academic_portability::export::export_profile` writes a directory, never an
archive, because archive containers record filesystem metadata and entry
ordering that differ between hosts.

```text
manifest.json
inventory.md
schemas/phase1-export-v1.schema.json
schemas/store-schema-v1.json
ledger/batches/<batch-id>.cbor
ledger/events.jsonl
canonical/artifacts.jsonl
canonical/claims.jsonl
canonical/decisions.jsonl
canonical/evidence.jsonl
canonical/relations.jsonl
canonical/scopes.jsonl
objects/<domain-id>/<artifact-id>.bin
```

Fixed rules:

- Rows and files sort by canonical identifier. `ledger/batches/<batch-id>.cbor`
  and `objects/<domain-id>/<artifact-id>.bin` are named by canonical UUID;
  `manifest.semantic.files` is strictly sorted by relative path.
- Relative paths use forward slashes on every host, stay within
  `MAX_PORTABLE_RELATIVE_PATH_BYTES` (160), and never contain a Windows reserved
  device name, a trailing dot or space, or a character Windows refuses.
- Records are UTF-8 with LF line endings and a final newline. JSONL records are
  compact JSON, one per line; `manifest.json` is two-space pretty JSON with a
  final newline. Key order is fixed by declaration order and no value is a
  floating-point number.
- `ledger/batches/<batch-id>.cbor` is the original signed envelope copied
  byte-for-byte. The exported bytes are re-hashed and compared with the stored
  envelope digest before publication.
- Filesystem metadata (mode, owner, timestamps, ordering) is not part of the
  contract and is never recorded.

Two exports of the same committed watermark produce identical per-file hashes
and an identical `semantic_digest` on Windows and Linux.

### Manifest shape

Both manifests separate a hashed block from an unhashed one:

```json
{ "semantic": { ... }, "semantic_digest": "<hex>", "volatile": { "generated_at_unix_ms": 0 } }
```

`semantic_digest` is SHA-256 over the domain separator
`learning-platform.phase1.export-manifest.v1` and the compact canonical JSON of
`semantic`, each field length-delimited with an unsigned big-endian 64-bit
prefix. `volatile` carries the generation instant and is deliberately outside
that digest, so two runs at the same watermark agree even though their manifest
bytes differ by one integer. `schemas/jsonschema/phase1-export-v1.schema.json`
is the machine-readable contract and is copied into every export directory.

`semantic` carries the frozen format name and manifest version, the generator
identity, the policy block, `encrypted: false`, `projections_included: false`,
the physical store identity, the watermark, canonical counts, device heads, the
ordered accepted-batch records, the reachable-object records, the canonical
semantic digest, and the file inventory with exact SHA-256 digests and lengths.

### Canonical semantic digest

`CanonicalRows::semantic_digest` hashes the complete canonical state under the
domain separator `learning-platform.phase1.canonical-semantic.v1`: the physical
schema identity, the watermark, the counts, and every device head, batch, event,
scope, artifact, evidence item, claim, relation, decision, outbox row, and
command receipt, each as compact canonical JSON in canonical-identifier order.
It never observes wall-clock generation time, filesystem metadata, or projection
state. An export and a backup of the same watermark report the same value.

### What the export is not

The export carries the synthetic canonical subset, the original envelopes, the
sealed objects, the schemas, and the inventory. It does not carry command
receipts or operational tables, so it is a documented interchange format, not a
restore source. Restore consumes a backup.

## Backup

`academic_portability::backup::backup_profile` copies the canonical database
with the SQLite Online Backup API into a sibling staging directory, copies every
reachable sealed object, writes a versioned manifest, synchronizes the tree, and
publishes it with one rename.

```text
manifest.json
schemas/phase1-backup-v1.schema.json
store/academic-platform.sqlite3
objects/<artifact-id>.bin
```

- The watermark is fixed: the source watermark, counts, device heads, and
  canonical semantic digest are read before the copy and re-read from the copy.
  Any drift fails closed rather than producing a smeared snapshot.
- The copy passes `integrity_check` and `foreign_key_check` before any object is
  copied.
- Object closure is complete: every `artifact_descriptor` row in the snapshot is
  read back through the vault's sealed-object verifier and copied with its exact
  plaintext digest and length.
- The backup does not mirror the vault's deep policy-namespaced fan-out. Restore
  re-derives the canonical vault path from the signed descriptor, which is both
  stronger and far shorter than trusting a path recorded in a manifest.
- The SQLite file need not be byte-identical across runs. Its integrity and its
  canonical semantic digest must be.

## Restore

`academic_portability::restore::restore_profile` accepts only a new empty
destination: an absent path, or an existing empty directory that is removed
immediately before the publish rename. A non-empty destination is refused.

The order is fixed:

1. Validate policy, format, manifest version, and every file hash and length in
   the backup directory; reject any unlisted file.
2. Create a sibling staging profile through the store's protected profile
   creation, carrying the store's incomplete marker and a restore marker naming
   the intended destination.
3. Copy the database and verify its digest and length.
4. Open the database and run `integrity_check` and `foreign_key_check`.
5. Compare the physical schema identity, watermark, counts, device heads, and
   canonical semantic digest with the manifest.
6. Replay every stored signed envelope against a caller-supplied
   `DeviceAuthorization`. The signing key carried inside a stored envelope is
   never trusted on its own. The replay re-derives every batch identity, event
   coordinate, canonical payload and actor digest, acceptance range, and device
   origin chain and compares them with the stored rows.
7. Copy each object to the canonical vault path re-derived from its signed
   descriptor, verify its plaintext digest and length, and then read every object
   back through the vault's sealed-object verifier.
8. Rebuild the requested projection generations from empty and require each to
   be VERIFIED and activated.
9. Remove the incomplete markers, synchronize the tree, and publish with one
   rename.

A failure at any step leaves the backup source and the current profile
untouched, and the destination absent. The restored profile opens through the
store's ordinary profile boundary.

## Fault matrix

Failpoints compile only under the non-default `phase1-fault-injection` feature.
Production builds contain no environment lookup and no crash switch.

| ID | Termination point | Required outcome |
|---|---|---|
| BK01 | midway through the Online Backup copy | staging removable; destination absent; source unchanged |
| BK02 | snapshot complete, before the first object copy | staging rejected by verification; destination absent |
| BK03 | midway through the object copy | manifest absent; staging rejected; destination absent |
| BK04 | manifest synced, before the publish rename | staging is a complete verifiable backup; destination still absent |
| RS01 | staging profile and markers created | staging recognizable and removable; no database copied |
| RS02 | database copied, before the integrity and ledger checks | staging not openable as a profile; destination absent |
| RS03 | objects copied, before closure checks and projection rebuild | staging not openable as a profile; backup and current profile unchanged |
| RS04 | all checks passed, before the publish rename | unpublished verified staging; destination absent |

Unpublished staging directories are recognizable by name beside their
destination (`find_unpublished_backups`, `find_unpublished_restores`) and are
removed only through `remove_unpublished_backup` / `remove_unpublished_restore`,
which refuse any path this crate did not name.

## Non-goals

Encrypted backup keys, compression, incremental production backup, in-place
restore, and any cloud destination are out of scope for Phase 1. Full mandatory
export formats, long-term format governance, cross-implementation import,
recovery recipients, and RPO/RTO objectives remain open under ADR-012.
