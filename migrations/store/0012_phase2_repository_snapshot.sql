-- Migration 0012: `P2-R1`'s typed columns for the `SNAPSHOT_REGISTERED`
-- aggregate -- section 17.2's `RepositorySnapshot` fields, its explicit
-- tracked/untracked dirty manifest, and the recorded decision without which a
-- secret file's digest is not stored at all.
--
-- Migration 0004 states the rule this file follows: "The columns below are the
-- whole of the v3 registration frame ... Typed aggregate attributes are
-- deliberately absent. Each aggregate owner adds its own typed columns in a
-- later migration." `P2-R1` owns `SNAPSHOT_REGISTERED`, and this is that
-- migration. It adds no event kind, no Proto tag and no CBOR arm: t068 section
-- 3.8 fixes the v3 arm list at eighteen and this file does not touch it.
--
-- It is `0012` rather than `0010` or `0011`. Those two numbers were reserved
-- for `P2-L2` and `P2-U6`, which were in flight on the same `main` when this
-- branch was written. `0008` was left unclaimed by `P2-M2` for the same kind of
-- reason and stays that way. A migration number decides the order and nothing
-- else rests on it: what the admission fingerprint fixes is the resulting
-- object set, read out of `sqlite_schema` sorted by type and name.
--
-- # The two vocabularies here are section 17's, not new ones
--
-- `source` holds the eight inputs section 17.1 names -- local directory, a
-- public or private GitHub repository, an archive, a branch, a commit, a dirty
-- working tree, and a specification-only project. `source_type` holds the four
-- values section 17.2's own `sourceType` field writes. They are different
-- questions: one is how the repository was named, the other is what the frozen
-- snapshot is of, and `guard_repository_snapshot_dirty_shape` below is what
-- keeps the second from being derived incorrectly from the first.
--
-- `secret_scan_result` holds `PASS` and `BLOCKED`, which are the two values
-- section 17.2 writes. `reason_code` on a finding holds section 3.5's closed
-- reason codes as `academic-policy` spells them; the four this scanner can
-- raise are named in the `CHECK`, and a fifth is a change to the scanner and to
-- this line together.
--
-- # What the guards here hold that the Rust half cannot
--
-- `academic-repository` refuses the same shapes in memory. These triggers are
-- the second layer, on the terms migration 0004 sets for every canonical table:
-- the trigger pair is the first enforcement layer and the product connection's
-- SQLite authorizer is the second. What is new here is that they run against
-- rows a process this repository did not write could have inserted.
--
--   * `guard_repository_snapshot_authorized` binds a typed row to the accepted
--     `SNAPSHOT_REGISTERED` event, the way `guard_model_run_provenance_authorized`
--     binds a provenance row to its own.
--
--   * `guard_repository_snapshot_dirty_shape` refuses a `DIRTY_WORKTREE` row
--     with no dirty patch digest and any other row that carries one. That is
--     section 17.2's rule that a dirty working tree is not implicitly
--     identified with HEAD, written where a row that arrived from somewhere
--     else is checked too.
--
--   * `guard_repository_manifest_dirty_parent` refuses a manifest row labelled
--     `TRACKED` or `UNTRACKED` under a snapshot that is not a dirty working
--     tree, so the two halves of the dirty manifest cannot exist without the
--     snapshot that says they should.
--
--   * `guard_repository_secret_finding_disclosure` is the default-deny half.
--     A finding's `blob_digest` and its `disclosure_decision_id` are present
--     together or absent together; there is no row shape holding a secret
--     file's digest with no decision naming who permitted it and why. The
--     foreign key is what makes the decision a record rather than a flag.
--
-- A blocked capture produces no snapshot, so a finding is keyed on the request
-- digest rather than on a `snapshot_id`. That is the whole reason the finding
-- table is not a child of `repository_snapshot`: the rows that matter most are
-- the ones from captures that never produced a snapshot at all.

CREATE TABLE repository_snapshot (
    snapshot_id BLOB PRIMARY KEY
        REFERENCES snapshot(snapshot_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source TEXT NOT NULL CHECK (source IN (
        'LOCAL_DIRECTORY',
        'GITHUB_PUBLIC',
        'GITHUB_PRIVATE',
        'ARCHIVE',
        'BRANCH',
        'COMMIT',
        'DIRTY_WORKTREE',
        'SPEC_ONLY'
    )),
    source_type TEXT NOT NULL CHECK (source_type IN (
        'GIT_COMMIT',
        'DIRTY_WORKTREE',
        'ARCHIVE',
        'SPEC_ONLY'
    )),
    branch TEXT CHECK (branch IS NULL OR length(branch) > 0),
    commit_id TEXT CHECK (
        commit_id IS NULL
        OR (length(commit_id) >= 7 AND length(commit_id) <= 64)
    ),
    captured_at INTEGER NOT NULL CHECK (captured_at >= 0),
    manifest_digest BLOB NOT NULL CHECK (
        typeof(manifest_digest) = 'blob' AND length(manifest_digest) = 32
    ),
    dirty_patch_digest BLOB CHECK (
        dirty_patch_digest IS NULL
        OR (typeof(dirty_patch_digest) = 'blob' AND length(dirty_patch_digest) = 32)
    ),
    analysis_policy_hash BLOB NOT NULL CHECK (
        typeof(analysis_policy_hash) = 'blob' AND length(analysis_policy_hash) = 32
    ),
    -- A `BLOCKED` capture produces no snapshot at all, so a row here can only
    -- record `PASS`. The column exists because section 17.2 names it and a
    -- reader of the persisted snapshot should not have to infer it; the `CHECK`
    -- is what says the inference would have been correct.
    secret_scan_result TEXT NOT NULL CHECK (secret_scan_result = 'PASS'),
    record_digest BLOB NOT NULL CHECK (
        typeof(record_digest) = 'blob' AND length(record_digest) = 32
    )
) STRICT;

CREATE TABLE repository_snapshot_manifest_entry (
    snapshot_id BLOB NOT NULL
        REFERENCES repository_snapshot(snapshot_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    path_ordinal INTEGER NOT NULL CHECK (path_ordinal >= 0),
    path TEXT NOT NULL CHECK (
        length(path) > 0
        AND path NOT LIKE '/%'
        AND path NOT LIKE '%\%' ESCAPE '\'
    ),
    blob_digest BLOB NOT NULL CHECK (
        typeof(blob_digest) = 'blob' AND length(blob_digest) = 32
    ),
    language TEXT NOT NULL CHECK (language IN (
        'RUST',
        'TYPESCRIPT',
        'PYTHON',
        'SQL',
        'MARKDOWN',
        'CONFIGURATION',
        'UNKNOWN'
    )),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    -- `NULL` for a path that is simply in the tree; `TRACKED` and `UNTRACKED`
    -- are the two halves of section 17.2's explicit dirty manifest. There is no
    -- third value and no default: a path is one of the three by an insert that
    -- says which.
    dirty_kind TEXT CHECK (dirty_kind IS NULL OR dirty_kind IN ('TRACKED', 'UNTRACKED')),
    PRIMARY KEY (snapshot_id, path_ordinal),
    UNIQUE (snapshot_id, path)
) WITHOUT ROWID, STRICT;

CREATE TABLE repository_snapshot_tool_version (
    snapshot_id BLOB NOT NULL
        REFERENCES repository_snapshot(snapshot_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    tool TEXT NOT NULL CHECK (length(tool) > 0),
    version TEXT NOT NULL CHECK (length(version) > 0),
    PRIMARY KEY (snapshot_id, tool)
) WITHOUT ROWID, STRICT;

CREATE TABLE repository_snapshot_excluded_path (
    snapshot_id BLOB NOT NULL
        REFERENCES repository_snapshot(snapshot_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    path TEXT NOT NULL CHECK (length(path) > 0),
    exclusion_reason TEXT NOT NULL CHECK (exclusion_reason IN (
        'GITIGNORE',
        'DENY_RULE',
        'USER_EXCLUSION',
        'SECRET_FILE_POLICY'
    )),
    PRIMARY KEY (snapshot_id, path)
) WITHOUT ROWID, STRICT;

CREATE TABLE repository_hash_disclosure_decision (
    decision_id TEXT PRIMARY KEY CHECK (length(decision_id) > 0),
    actor_id TEXT NOT NULL CHECK (length(actor_id) > 0),
    reason TEXT NOT NULL CHECK (length(reason) > 0),
    recorded_at INTEGER NOT NULL CHECK (recorded_at >= 0)
) WITHOUT ROWID, STRICT;

CREATE TABLE repository_secret_finding (
    request_digest BLOB NOT NULL CHECK (
        typeof(request_digest) = 'blob' AND length(request_digest) = 32
    ),
    path TEXT NOT NULL CHECK (length(path) > 0),
    reason_code TEXT NOT NULL CHECK (reason_code IN (
        'SECRET_PATTERN',
        'SECRET_ENTROPY',
        'SCANNER_ERROR',
        'UNKNOWN_BINARY'
    )),
    disclosure_decision_id TEXT
        REFERENCES repository_hash_disclosure_decision(decision_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    blob_digest BLOB CHECK (
        blob_digest IS NULL
        OR (typeof(blob_digest) = 'blob' AND length(blob_digest) = 32)
    ),
    PRIMARY KEY (request_digest, path)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER guard_repository_snapshot_authorized
BEFORE INSERT ON repository_snapshot
BEGIN
    SELECT RAISE(
        ABORT,
        'repository snapshot is not the record its event authorized'
    )
    WHERE NOT EXISTS (
        SELECT 1 FROM snapshot
        WHERE snapshot.snapshot_id = NEW.snapshot_id
          AND snapshot.source_digest IS NOT NULL
          AND snapshot.source_digest = NEW.record_digest
    );
END;

CREATE TRIGGER guard_repository_snapshot_dirty_shape
BEFORE INSERT ON repository_snapshot
BEGIN
    SELECT RAISE(ABORT, 'a dirty working tree is not its HEAD commit')
    WHERE (NEW.source_type = 'DIRTY_WORKTREE' AND NEW.dirty_patch_digest IS NULL)
       OR (NEW.source_type <> 'DIRTY_WORKTREE' AND NEW.dirty_patch_digest IS NOT NULL);
END;

CREATE TRIGGER guard_repository_snapshot_commit_shape
BEFORE INSERT ON repository_snapshot
BEGIN
    SELECT RAISE(ABORT, 'a git-commit snapshot names no commit')
    WHERE NEW.source_type = 'GIT_COMMIT' AND NEW.commit_id IS NULL;
END;

CREATE TRIGGER guard_repository_manifest_dirty_parent
BEFORE INSERT ON repository_snapshot_manifest_entry
BEGIN
    SELECT RAISE(ABORT, 'a dirty manifest row belongs to a snapshot that is not one')
    WHERE NEW.dirty_kind IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM repository_snapshot
        WHERE repository_snapshot.snapshot_id = NEW.snapshot_id
          AND repository_snapshot.source_type = 'DIRTY_WORKTREE'
      );
END;

CREATE TRIGGER guard_repository_secret_finding_disclosure
BEFORE INSERT ON repository_secret_finding
BEGIN
    SELECT RAISE(ABORT, 'a secret file digest needs a recorded disclosure decision')
    WHERE (NEW.blob_digest IS NOT NULL AND NEW.disclosure_decision_id IS NULL)
       OR (NEW.blob_digest IS NULL AND NEW.disclosure_decision_id IS NOT NULL);
END;

CREATE TRIGGER guard_repository_snapshot_update
BEFORE UPDATE ON repository_snapshot
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_snapshot_delete
BEFORE DELETE ON repository_snapshot
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_snapshot_manifest_entry_update
BEFORE UPDATE ON repository_snapshot_manifest_entry
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_snapshot_manifest_entry_delete
BEFORE DELETE ON repository_snapshot_manifest_entry
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_snapshot_tool_version_update
BEFORE UPDATE ON repository_snapshot_tool_version
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_snapshot_tool_version_delete
BEFORE DELETE ON repository_snapshot_tool_version
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_snapshot_excluded_path_update
BEFORE UPDATE ON repository_snapshot_excluded_path
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_snapshot_excluded_path_delete
BEFORE DELETE ON repository_snapshot_excluded_path
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_hash_disclosure_decision_update
BEFORE UPDATE ON repository_hash_disclosure_decision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_hash_disclosure_decision_delete
BEFORE DELETE ON repository_hash_disclosure_decision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_secret_finding_update
BEFORE UPDATE ON repository_secret_finding
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_repository_secret_finding_delete
BEFORE DELETE ON repository_secret_finding
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
