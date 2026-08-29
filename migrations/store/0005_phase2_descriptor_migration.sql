-- Migration 0005: `P2-K5`'s typed columns for the `RETENTION_ACTION_RECORDED`
-- aggregate, which are how a re-seal moves a canonical object reference.
--
-- Migration 0004 states the rule this file follows: "The columns below are the
-- whole of the v3 registration frame ... Typed aggregate attributes are
-- deliberately absent. Each aggregate owner adds its own typed columns in a
-- later migration." `P2-K5` owns `RETENTION_ACTION_RECORDED`, and this is that
-- later migration. It adds no event kind, no Proto tag, and no CBOR arm: t068
-- section 3.8 fixes the v3 arm list at eighteen and this file does not touch it.
--
-- # Why a reference needs a migration row at all
--
-- `artifact_descriptor.vault_locator` is inside the signed `ARTIFACT_REGISTERED`
-- payload and the table is INSERT-only twice over, so the locator a rotation
-- moves an object to cannot be written over the one the signature covers. The
-- reference moves the way every other correction in this store moves: a new
-- appended row supersedes an older one, and readers resolve the current value
-- by walking the chain. `artifact_descriptor` is never updated.
--
-- # What binds one row to a canonical event
--
-- Two things, both enforced here rather than asserted:
--
--   * `retention_action_id` is a foreign key, so a migration row cannot exist
--     without the accepted `RETENTION_ACTION_RECORDED` event that registered
--     that aggregate; and
--   * `guard_artifact_descriptor_migration_authorized` refuses an insert whose
--     `record_digest` is not the `source_digest` that event carries. The v3
--     registration frame's optional provenance digest is what authorizes the
--     exact locator pair below, so a row nobody signed for cannot move a
--     reference.
--
-- `guard_artifact_descriptor_migration_chain` is the third: sequence 1 must
-- supersede the descriptor's own signed locator and sequence n must supersede
-- sequence n-1's, so the chain cannot fork, skip, or start in the middle.
--
-- The table is canonical history on the same terms as every other: the
-- update/delete trigger pair here is the first enforcement layer and the
-- product connection's SQLite authorizer is the second.

CREATE TABLE artifact_descriptor_migration (
    retention_action_id BLOB PRIMARY KEY
        REFERENCES retention_action(retention_action_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    artifact_id BLOB NOT NULL
        REFERENCES artifact_descriptor(artifact_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    migration_seq INTEGER NOT NULL CHECK (migration_seq >= 1),
    superseded_locator BLOB NOT NULL CHECK (
        typeof(superseded_locator) = 'blob' AND length(superseded_locator) = 32
    ),
    vault_locator BLOB NOT NULL CHECK (
        typeof(vault_locator) = 'blob' AND length(vault_locator) = 32
    ),
    format_version INTEGER NOT NULL CHECK (format_version BETWEEN 1 AND 65535),
    record_digest BLOB NOT NULL CHECK (
        typeof(record_digest) = 'blob' AND length(record_digest) = 32
    ),
    UNIQUE (artifact_id, migration_seq),
    UNIQUE (artifact_id, superseded_locator),
    UNIQUE (artifact_id, vault_locator),
    CHECK (vault_locator <> superseded_locator)
) STRICT;

CREATE TRIGGER guard_artifact_descriptor_migration_authorized
BEFORE INSERT ON artifact_descriptor_migration
BEGIN
    SELECT RAISE(
        ABORT,
        'descriptor migration is not the one its retention action authorized'
    )
    WHERE NOT EXISTS (
        SELECT 1 FROM retention_action
        WHERE retention_action.retention_action_id = NEW.retention_action_id
          AND retention_action.source_digest IS NOT NULL
          AND retention_action.source_digest = NEW.record_digest
    );
END;

CREATE TRIGGER guard_artifact_descriptor_migration_chain
BEFORE INSERT ON artifact_descriptor_migration
BEGIN
    SELECT RAISE(ABORT, 'descriptor migration does not continue the reference chain')
    WHERE NOT EXISTS (
        SELECT 1 FROM artifact_descriptor
        WHERE artifact_descriptor.artifact_id = NEW.artifact_id
          AND (
              NEW.migration_seq > 1
              OR artifact_descriptor.vault_locator = NEW.superseded_locator
          )
    )
    OR (
        NEW.migration_seq > 1
        AND NOT EXISTS (
            SELECT 1 FROM artifact_descriptor_migration AS previous
            WHERE previous.artifact_id = NEW.artifact_id
              AND previous.migration_seq = NEW.migration_seq - 1
              AND previous.vault_locator = NEW.superseded_locator
        )
    );
END;

CREATE TRIGGER guard_artifact_descriptor_migration_update
BEFORE UPDATE ON artifact_descriptor_migration
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_artifact_descriptor_migration_delete
BEFORE DELETE ON artifact_descriptor_migration
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
