-- Migration 0004: typed, INSERT-only closure tables for the eighteen event
-- schema v3 aggregate registration arms.
--
-- Applies on top of store schema version 2. Migration 0003 establishes that
-- version, its identity triplet, and the encrypted lane; this file changes no
-- part of that identity and touches neither `schema_meta` nor `application_id`
-- nor `user_version`.
--
-- Every table here is canonical history: INSERT-only, guarded by a
-- `guard_<table>_update`/`guard_<table>_delete` trigger pair and by the
-- product connection's SQLite authorizer, exactly as the schema-1 canonical
-- set is. A correction appends a new event; it never edits a row.
--
-- Each row is written inside the same acceptance transaction that inserts its
-- event, and `registered_event_id` is UNIQUE so one event registers at most
-- one aggregate. The column is named `registered_event_id` rather than
-- `assertion_event_id` because all eighteen v3 arms are registration depth:
-- they place an aggregate in a domain and a scope. `claim.assertion_event_id`
-- keeps the assertion name because a claim asserts a typed object about a
-- subject. The two are contractually different events.
--
-- The columns below are the whole of the v3 registration frame: identity,
-- parent binding, domain, scope, optional provenance digest, and the half-open
-- valid interval. Typed aggregate attributes are deliberately absent. Each
-- aggregate owner adds its own typed columns in a later migration; none of them
-- may be smuggled into `claim.object_text`, whose `object_kind` stays the
-- closed nine-value enum that this migration does not touch.

-- SQLite cannot alter a CHECK constraint in place, so widening
-- `ledger_event.event_kind` from the six v1/v2 kinds to those plus the
-- eighteen v3 kinds uses SQLite's documented table-rebuild procedure. The copy
-- preserves every canonical row byte-for-byte, and the rebuild runs only on the
-- pre-listen migration connection, which installs no authorizer.
--
-- Profile creation runs this file as the last step of `STORE_MIGRATION_SQL`,
-- inside the one exclusive creation transaction, against a database admission
-- has proved empty. The rebuild therefore copies no rows, and the
-- `integrity_check` and `foreign_key_check` that run on the migrated database
-- before the profile is admitted are what prove nothing was left dangling.
-- `apply_aggregate_migration_pre_listen`, which layers this file onto a
-- schema-2 base assembled by hand, additionally disables foreign keys around
-- its transaction and runs both checks before committing, because such a base
-- may already hold rows.

DROP TRIGGER guard_ledger_event_update;
DROP TRIGGER guard_ledger_event_delete;

CREATE TABLE ledger_event_rebuilt_0004 (
    event_id BLOB PRIMARY KEY CHECK (typeof(event_id) = 'blob' AND length(event_id) = 16),
    batch_id BLOB NOT NULL REFERENCES ledger_batch(batch_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    origin_seq INTEGER NOT NULL CHECK (origin_seq >= 1),
    origin_observed_at INTEGER NOT NULL,
    accept_seq INTEGER NOT NULL UNIQUE CHECK (accept_seq >= 1),
    actor_kind TEXT NOT NULL CHECK (
        actor_kind IN ('USER', 'DETERMINISTIC_ENGINE', 'MODEL_RUN', 'IMPORTER')
    ),
    actor_canonical BLOB NOT NULL CHECK (typeof(actor_canonical) = 'blob' AND length(actor_canonical) > 0),
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    event_kind TEXT NOT NULL CHECK (
        event_kind IN (
            'SCOPE_REGISTERED', 'ARTIFACT_REGISTERED', 'EVIDENCE_REGISTERED',
            'CLAIM_ASSERTED', 'CLAIM_RELATED', 'DECISION_RECORDED',
            'CURRICULUM_VERSION_PUBLISHED', 'COURSE_REVISION_PUBLISHED',
            'OFFERING_OBSERVED', 'ATTEMPT_RECORDED',
            'REQUIREMENT_SET_PUBLISHED', 'AUDIT_COMPUTED',
            'CAPTURE_PERMISSION_RECORDED', 'LECTURE_SESSION_RECORDED',
            'TRANSCRIPT_VERSION_ADDED', 'LECTURE_DOCUMENT_PUBLISHED',
            'SNAPSHOT_REGISTERED', 'FINDING_PUBLISHED',
            'MODEL_RUN_RECORDED', 'PROPOSAL_DISPOSED',
            'EGRESS_DECIDED', 'CONSENT_RECORDED',
            'ENTITY_IDENTITY_CHANGED', 'RETENTION_ACTION_RECORDED'
        )
    ),
    canonical_payload BLOB NOT NULL CHECK (
        typeof(canonical_payload) = 'blob' AND length(canonical_payload) > 0
    ),
    payload_hash BLOB NOT NULL CHECK (typeof(payload_hash) = 'blob' AND length(payload_hash) = 32),
    UNIQUE (batch_id, origin_seq),
    UNIQUE (batch_id, accept_seq),
    UNIQUE (event_id, payload_hash)
) STRICT;

INSERT INTO ledger_event_rebuilt_0004 (
    event_id, batch_id, origin_seq, origin_observed_at, accept_seq,
    actor_kind, actor_canonical, domain_id, event_kind,
    canonical_payload, payload_hash
)
SELECT
    event_id, batch_id, origin_seq, origin_observed_at, accept_seq,
    actor_kind, actor_canonical, domain_id, event_kind,
    canonical_payload, payload_hash
FROM ledger_event;

DROP TABLE ledger_event;
ALTER TABLE ledger_event_rebuilt_0004 RENAME TO ledger_event;

CREATE TRIGGER guard_ledger_event_update BEFORE UPDATE ON ledger_event
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_ledger_event_delete BEFORE DELETE ON ledger_event
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;

-- CURRICULUM_VERSION_PUBLISHED
CREATE TABLE curriculum_version (
    curriculum_version_id BLOB PRIMARY KEY CHECK (typeof(curriculum_version_id) = 'blob' AND length(curriculum_version_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- COURSE_REVISION_PUBLISHED
CREATE TABLE course_revision (
    course_revision_id BLOB PRIMARY KEY CHECK (typeof(course_revision_id) = 'blob' AND length(course_revision_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    curriculum_version_id BLOB NOT NULL
        REFERENCES curriculum_version(curriculum_version_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- OFFERING_OBSERVED
CREATE TABLE offering (
    offering_id BLOB PRIMARY KEY CHECK (typeof(offering_id) = 'blob' AND length(offering_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    course_revision_id BLOB NOT NULL
        REFERENCES course_revision(course_revision_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- ATTEMPT_RECORDED
CREATE TABLE attempt (
    attempt_id BLOB PRIMARY KEY CHECK (typeof(attempt_id) = 'blob' AND length(attempt_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    offering_id BLOB NOT NULL
        REFERENCES offering(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- REQUIREMENT_SET_PUBLISHED
CREATE TABLE requirement_set (
    requirement_set_id BLOB PRIMARY KEY CHECK (typeof(requirement_set_id) = 'blob' AND length(requirement_set_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    curriculum_version_id BLOB NOT NULL
        REFERENCES curriculum_version(curriculum_version_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- AUDIT_COMPUTED
CREATE TABLE audit (
    audit_id BLOB PRIMARY KEY CHECK (typeof(audit_id) = 'blob' AND length(audit_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    requirement_set_id BLOB NOT NULL
        REFERENCES requirement_set(requirement_set_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- CAPTURE_PERMISSION_RECORDED
CREATE TABLE capture_permission (
    capture_permission_id BLOB PRIMARY KEY CHECK (typeof(capture_permission_id) = 'blob' AND length(capture_permission_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    offering_id BLOB NOT NULL
        REFERENCES offering(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- LECTURE_SESSION_RECORDED
CREATE TABLE lecture_session (
    lecture_session_id BLOB PRIMARY KEY CHECK (typeof(lecture_session_id) = 'blob' AND length(lecture_session_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    offering_id BLOB NOT NULL
        REFERENCES offering(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- TRANSCRIPT_VERSION_ADDED
CREATE TABLE transcript_version (
    transcript_version_id BLOB PRIMARY KEY CHECK (typeof(transcript_version_id) = 'blob' AND length(transcript_version_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    lecture_session_id BLOB NOT NULL
        REFERENCES lecture_session(lecture_session_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- LECTURE_DOCUMENT_PUBLISHED
CREATE TABLE lecture_document (
    lecture_document_id BLOB PRIMARY KEY CHECK (typeof(lecture_document_id) = 'blob' AND length(lecture_document_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    lecture_session_id BLOB NOT NULL
        REFERENCES lecture_session(lecture_session_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- SNAPSHOT_REGISTERED
-- `repository_id` carries no foreign key because no `repository` registration arm exists: section 3.8 fixes the arm list at
-- eighteen, and P2-R1 owns the open question.
CREATE TABLE snapshot (
    snapshot_id BLOB PRIMARY KEY CHECK (typeof(snapshot_id) = 'blob' AND length(snapshot_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    repository_id BLOB NOT NULL CHECK (
        typeof(repository_id) = 'blob' AND length(repository_id) = 16
    ),
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- FINDING_PUBLISHED
CREATE TABLE finding (
    finding_id BLOB PRIMARY KEY CHECK (typeof(finding_id) = 'blob' AND length(finding_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    snapshot_id BLOB NOT NULL
        REFERENCES snapshot(snapshot_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- MODEL_RUN_RECORDED
CREATE TABLE model_run (
    model_run_id BLOB PRIMARY KEY CHECK (typeof(model_run_id) = 'blob' AND length(model_run_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- PROPOSAL_DISPOSED
CREATE TABLE proposal_disposition (
    proposal_id BLOB PRIMARY KEY CHECK (typeof(proposal_id) = 'blob' AND length(proposal_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    model_run_id BLOB NOT NULL
        REFERENCES model_run(model_run_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- EGRESS_DECIDED
CREATE TABLE egress_decision (
    egress_decision_id BLOB PRIMARY KEY CHECK (typeof(egress_decision_id) = 'blob' AND length(egress_decision_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- CONSENT_RECORDED
CREATE TABLE consent (
    consent_id BLOB PRIMARY KEY CHECK (typeof(consent_id) = 'blob' AND length(consent_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- ENTITY_IDENTITY_CHANGED
-- `entity_id` carries no foreign key because the entity registry is P2-C3's aggregate.
CREATE TABLE entity_identity_change (
    entity_identity_change_id BLOB PRIMARY KEY CHECK (typeof(entity_identity_change_id) = 'blob' AND length(entity_identity_change_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    entity_id BLOB NOT NULL CHECK (
        typeof(entity_id) = 'blob' AND length(entity_id) = 16
    ),
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- RETENTION_ACTION_RECORDED
CREATE TABLE retention_action (
    retention_action_id BLOB PRIMARY KEY CHECK (typeof(retention_action_id) = 'blob' AND length(retention_action_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

-- Append-only enforcement, the first of the two layers every canonical table
-- carries. The second is the product connection's SQLite authorizer, which
-- denies UPDATE, DELETE, DROP, and ALTER over the same set.
CREATE TRIGGER guard_curriculum_version_update BEFORE UPDATE ON curriculum_version
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_curriculum_version_delete BEFORE DELETE ON curriculum_version
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_update BEFORE UPDATE ON course_revision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_delete BEFORE DELETE ON course_revision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_update BEFORE UPDATE ON offering
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_delete BEFORE DELETE ON offering
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_attempt_update BEFORE UPDATE ON attempt
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_attempt_delete BEFORE DELETE ON attempt
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_requirement_set_update BEFORE UPDATE ON requirement_set
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_requirement_set_delete BEFORE DELETE ON requirement_set
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_audit_update BEFORE UPDATE ON audit
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_audit_delete BEFORE DELETE ON audit
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_capture_permission_update BEFORE UPDATE ON capture_permission
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_capture_permission_delete BEFORE DELETE ON capture_permission
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_lecture_session_update BEFORE UPDATE ON lecture_session
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_lecture_session_delete BEFORE DELETE ON lecture_session
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_transcript_version_update BEFORE UPDATE ON transcript_version
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_transcript_version_delete BEFORE DELETE ON transcript_version
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_lecture_document_update BEFORE UPDATE ON lecture_document
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_lecture_document_delete BEFORE DELETE ON lecture_document
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_snapshot_update BEFORE UPDATE ON snapshot
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_snapshot_delete BEFORE DELETE ON snapshot
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_finding_update BEFORE UPDATE ON finding
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_finding_delete BEFORE DELETE ON finding
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_update BEFORE UPDATE ON model_run
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_model_run_delete BEFORE DELETE ON model_run
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_disposition_update BEFORE UPDATE ON proposal_disposition
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_disposition_delete BEFORE DELETE ON proposal_disposition
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_egress_decision_update BEFORE UPDATE ON egress_decision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_egress_decision_delete BEFORE DELETE ON egress_decision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_consent_update BEFORE UPDATE ON consent
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_consent_delete BEFORE DELETE ON consent
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_entity_identity_change_update BEFORE UPDATE ON entity_identity_change
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_entity_identity_change_delete BEFORE DELETE ON entity_identity_change
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_retention_action_update BEFORE UPDATE ON retention_action
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_retention_action_delete BEFORE DELETE ON retention_action
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
