-- Migration 0007: `P2-M1`'s typed columns for the `MODEL_RUN_RECORDED`
-- aggregate -- the twelve fields section 27.3 of the authoritative spec gives a
-- model execution, and the reanalysis candidates one execution produces.
--
-- It is `0007` rather than `0006` because `P2-G6` took that number first. The
-- two are independent DDL -- neither references an object the other creates,
-- and applying them in either order produces the same schema -- so the number
-- decides the order and nothing else rests on it. What the admission
-- fingerprint fixes is the object *set*: it reads `sqlite_schema` sorted by
-- type and name, so a profile created before this migration landed is refused
-- because it lacks these tables, not because the sequence changed.
--
-- Migration 0004 states the rule this file follows: "The columns below are the
-- whole of the v3 registration frame ... Typed aggregate attributes are
-- deliberately absent. Each aggregate owner adds its own typed columns in a
-- later migration." `P2-M1` owns `MODEL_RUN_RECORDED`, and this is that later
-- migration. It adds no event kind, no Proto tag, and no CBOR arm: t068
-- section 3.8 fixes the v3 arm list at eighteen and this file does not touch
-- it. The arm stays registration depth; the twelve fields live here.
--
-- # Why three tables and not one
--
-- Two of the twelve fields are lists -- `inputArtifactRefs` and
-- `transmittedByteRanges` -- and a list in a column is a list nothing can
-- constrain. They get child tables on the shape `audit_artifact_range` already
-- uses in the permission broker, so a transmitted range here and an audited
-- range there are the same four values and can be compared without decoding a
-- packed string.
--
-- # `transmittedByteRanges` is never absent
--
-- A local model transmits nothing, and "nothing" is a recorded value rather
-- than a missing one: `transmission_kind` is `NOT NULL` and `LOCAL_ONLY` says
-- the empty list explicitly. That matters because the reconciliation against
-- `egress_audit` runs in both directions -- a `LOCAL_ONLY` run that turns out
-- to have an audited transmission fails it just as an `EGRESSED` run whose
-- ranges do not match does.
--
-- `transmitted_grant_id` is the `egress_grant.grant_id` the transfer spent. It
-- is not a foreign key: `egress_grant` lives in `academic-policy`'s operational
-- store, which is a different database. What checks the reference is the
-- reconciliation, and what makes that key exact is `egress_consumption`'s own
-- two foreign keys rather than anything declared here.
--
-- # What binds one row to a canonical event
--
-- Two things, both enforced here rather than asserted:
--
--   * `model_run_id` is a foreign key, so a provenance row cannot exist without
--     the accepted `MODEL_RUN_RECORDED` event that registered that aggregate;
--     and
--   * `guard_model_run_provenance_authorized` refuses an insert whose
--     `record_digest` is not the `source_digest` that event carries.
--
-- The digest covers the child rows as well, and a `BEFORE INSERT` trigger on
-- the parent cannot see rows that do not exist yet. So the trigger is the write
-- half and recomputation on read is the other: a reader rebuilds the digest
-- from the persisted parent and children and compares it with the event's
-- `source_digest`, which is what makes an edited child row detectable.
--
-- # Reanalysis appends
--
-- ADR-003's rule is the one that applies: a correction appends a new assertion
-- and never edits a row. A second model run over the same source therefore
-- writes a new `model_run_candidate` whose `supersedes_candidate_id` names the
-- first. `guard_model_run_candidate_supersession` refuses a supersession that
-- addresses another subject, or that comes from the same model run -- an
-- execution cannot revise its own output, only a later one can -- and the
-- `UNIQUE` on `supersedes_candidate_id` refuses a fork. The prior row is never
-- touched, which is what makes the diff between the two readable.
--
-- The tables are canonical history on the same terms as every other: the
-- update/delete trigger pairs here are the first enforcement layer and the
-- product connection's SQLite authorizer is the second.

CREATE TABLE model_run_provenance (
    model_run_id BLOB PRIMARY KEY
        REFERENCES model_run(model_run_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    purpose_id TEXT NOT NULL CHECK (length(purpose_id) > 0),
    provider_id TEXT NOT NULL CHECK (length(provider_id) > 0),
    model_version TEXT NOT NULL CHECK (length(model_version) > 0),
    prompt_template_hash BLOB NOT NULL CHECK (
        typeof(prompt_template_hash) = 'blob' AND length(prompt_template_hash) = 32
    ),
    transmission_kind TEXT NOT NULL
        CHECK (transmission_kind IN ('LOCAL_ONLY', 'EGRESSED')),
    transmitted_grant_id TEXT CHECK (
        (transmission_kind = 'LOCAL_ONLY' AND transmitted_grant_id IS NULL)
        OR (transmission_kind = 'EGRESSED' AND length(transmitted_grant_id) = 64)
    ),
    redaction_policy_hash BLOB NOT NULL CHECK (
        typeof(redaction_policy_hash) = 'blob' AND length(redaction_policy_hash) = 32
    ),
    output_artifact_id BLOB NOT NULL CHECK (
        typeof(output_artifact_id) = 'blob' AND length(output_artifact_id) = 16
    ),
    started_at INTEGER NOT NULL CHECK (started_at >= 0),
    cost_micros INTEGER NOT NULL CHECK (cost_micros >= 0),
    cost_currency TEXT NOT NULL CHECK (length(cost_currency) = 3),
    retention_declaration_id TEXT NOT NULL CHECK (length(retention_declaration_id) > 0),
    record_digest BLOB NOT NULL CHECK (
        typeof(record_digest) = 'blob' AND length(record_digest) = 32
    )
) STRICT;

CREATE TABLE model_run_input_artifact (
    model_run_id BLOB NOT NULL
        REFERENCES model_run_provenance(model_run_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    input_ordinal INTEGER NOT NULL CHECK (input_ordinal >= 0),
    artifact_id BLOB NOT NULL CHECK (
        typeof(artifact_id) = 'blob' AND length(artifact_id) = 16
    ),
    content_digest BLOB NOT NULL CHECK (
        typeof(content_digest) = 'blob' AND length(content_digest) = 32
    ),
    PRIMARY KEY (model_run_id, input_ordinal),
    UNIQUE (model_run_id, artifact_id)
) WITHOUT ROWID, STRICT;

CREATE TABLE model_run_transmitted_range (
    model_run_id BLOB NOT NULL
        REFERENCES model_run_provenance(model_run_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    range_ordinal INTEGER NOT NULL CHECK (range_ordinal >= 0),
    object_id TEXT NOT NULL CHECK (length(object_id) > 0),
    byte_start INTEGER NOT NULL CHECK (byte_start >= 0),
    byte_end INTEGER NOT NULL CHECK (byte_end > byte_start),
    content_digest BLOB NOT NULL CHECK (
        typeof(content_digest) = 'blob' AND length(content_digest) = 32
    ),
    PRIMARY KEY (model_run_id, range_ordinal)
) WITHOUT ROWID, STRICT;

CREATE TABLE model_run_candidate (
    candidate_id BLOB PRIMARY KEY CHECK (
        typeof(candidate_id) = 'blob' AND length(candidate_id) = 16
    ),
    model_run_id BLOB NOT NULL
        REFERENCES model_run_provenance(model_run_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    subject_digest BLOB NOT NULL CHECK (
        typeof(subject_digest) = 'blob' AND length(subject_digest) = 32
    ),
    candidate_digest BLOB NOT NULL CHECK (
        typeof(candidate_digest) = 'blob' AND length(candidate_digest) = 32
    ),
    supersedes_candidate_id BLOB
        REFERENCES model_run_candidate(candidate_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE (model_run_id, subject_digest),
    UNIQUE (supersedes_candidate_id),
    CHECK (supersedes_candidate_id IS NULL OR supersedes_candidate_id <> candidate_id)
) STRICT;

CREATE TRIGGER guard_model_run_provenance_authorized
BEFORE INSERT ON model_run_provenance
BEGIN
    SELECT RAISE(
        ABORT,
        'model run provenance is not the record its event authorized'
    )
    WHERE NOT EXISTS (
        SELECT 1 FROM model_run
        WHERE model_run.model_run_id = NEW.model_run_id
          AND model_run.source_digest IS NOT NULL
          AND model_run.source_digest = NEW.record_digest
    );
END;

CREATE TRIGGER guard_model_run_transmitted_range_local_only
BEFORE INSERT ON model_run_transmitted_range
BEGIN
    SELECT RAISE(ABORT, 'a local-only model run transmitted no range')
    WHERE NOT EXISTS (
        SELECT 1 FROM model_run_provenance
        WHERE model_run_provenance.model_run_id = NEW.model_run_id
          AND model_run_provenance.transmission_kind = 'EGRESSED'
    );
END;

CREATE TRIGGER guard_model_run_candidate_supersession
BEFORE INSERT ON model_run_candidate
BEGIN
    SELECT RAISE(ABORT, 'a reanalysis candidate does not supersede the same subject from another run')
    WHERE NEW.supersedes_candidate_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM model_run_candidate AS prior
        WHERE prior.candidate_id = NEW.supersedes_candidate_id
          AND prior.subject_digest = NEW.subject_digest
          AND prior.model_run_id <> NEW.model_run_id
      );
END;

CREATE TRIGGER guard_model_run_provenance_update
BEFORE UPDATE ON model_run_provenance
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_provenance_delete
BEFORE DELETE ON model_run_provenance
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_input_artifact_update
BEFORE UPDATE ON model_run_input_artifact
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_input_artifact_delete
BEFORE DELETE ON model_run_input_artifact
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_transmitted_range_update
BEFORE UPDATE ON model_run_transmitted_range
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_transmitted_range_delete
BEFORE DELETE ON model_run_transmitted_range
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_candidate_update
BEFORE UPDATE ON model_run_candidate
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_candidate_delete
BEFORE DELETE ON model_run_candidate
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
