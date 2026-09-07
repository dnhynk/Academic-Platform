-- Migration 0016: additive deterministic prediction actor, gate_7e4af1a2ad17.
-- Encrypted schema-2 maintenance only. No event payload tag or signed byte changes.
-- Rebuilds preserve all columns; foreign keys and integrity are checked before commit.

CREATE TEMP TABLE prediction_actor_preflight_0016 (applicable INTEGER CHECK (applicable = 1));
INSERT INTO prediction_actor_preflight_0016 VALUES ((
    SELECT count(*) = 2 FROM sqlite_schema
    WHERE type = 'table' AND name IN ('ledger_event', 'claim_relation')
      AND instr(sql, 'DETERMINISTIC_PREDICTION') = 0
));
DROP TABLE prediction_actor_preflight_0016;

DROP TRIGGER guard_ledger_event_update;
DROP TRIGGER guard_ledger_event_delete;

CREATE TABLE ledger_event_rebuilt_0016 (
    event_id BLOB PRIMARY KEY CHECK (typeof(event_id) = 'blob' AND length(event_id) = 16),
    batch_id BLOB NOT NULL REFERENCES ledger_batch(batch_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    origin_seq INTEGER NOT NULL CHECK (origin_seq >= 1),
    origin_observed_at INTEGER NOT NULL,
    accept_seq INTEGER NOT NULL UNIQUE CHECK (accept_seq >= 1),
    actor_kind TEXT NOT NULL CHECK (
        actor_kind IN ('USER', 'DETERMINISTIC_ENGINE', 'MODEL_RUN', 'IMPORTER', 'DETERMINISTIC_PREDICTION')
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

INSERT INTO ledger_event_rebuilt_0016 (
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
ALTER TABLE ledger_event_rebuilt_0016 RENAME TO ledger_event;

CREATE TRIGGER guard_ledger_event_update BEFORE UPDATE ON ledger_event
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_ledger_event_delete BEFORE DELETE ON ledger_event
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;

DROP TRIGGER guard_claim_relation_update;
DROP TRIGGER guard_claim_relation_delete;

CREATE TABLE claim_relation_rebuilt_0016 (
    relation_event_id BLOB PRIMARY KEY
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT
        CHECK (typeof(relation_event_id) = 'blob' AND length(relation_event_id) = 16),
    source_claim_id BLOB NOT NULL
        REFERENCES claim(claim_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    target_claim_id BLOB NOT NULL
        REFERENCES claim(claim_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    relation_kind TEXT NOT NULL CHECK (
        relation_kind IN ('SUPPORTS', 'CONTRADICTS', 'SUPERSEDES', 'RETRACTS', 'DUPLICATES')
    ),
    actor_kind TEXT NOT NULL CHECK (
        actor_kind IN ('USER', 'DETERMINISTIC_ENGINE', 'MODEL_RUN', 'IMPORTER', 'DETERMINISTIC_PREDICTION')
    ),
    CHECK (source_claim_id <> target_claim_id),
    UNIQUE (source_claim_id, target_claim_id, relation_kind, scope_id)
) STRICT;

INSERT INTO claim_relation_rebuilt_0016
SELECT relation_event_id, source_claim_id, target_claim_id, scope_id, relation_kind, actor_kind
FROM claim_relation;
DROP TABLE claim_relation;
ALTER TABLE claim_relation_rebuilt_0016 RENAME TO claim_relation;

CREATE TRIGGER guard_claim_relation_update BEFORE UPDATE ON claim_relation
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_claim_relation_delete BEFORE DELETE ON claim_relation
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
