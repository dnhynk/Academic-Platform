CREATE TABLE schema_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    format_uuid BLOB NOT NULL CHECK (typeof(format_uuid) = 'blob' AND length(format_uuid) = 16),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    schema_semver TEXT NOT NULL CHECK (schema_semver = '1.0.0'),
    minimum_reader_protocol_major INTEGER NOT NULL CHECK (minimum_reader_protocol_major = 1),
    minimum_reader_protocol_minor INTEGER NOT NULL CHECK (minimum_reader_protocol_minor = 0),
    minimum_writer_protocol_major INTEGER NOT NULL CHECK (minimum_writer_protocol_major = 1),
    minimum_writer_protocol_minor INTEGER NOT NULL CHECK (minimum_writer_protocol_minor = 0),
    data_policy TEXT NOT NULL CHECK (data_policy = 'SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED'),
    storage_mode TEXT NOT NULL CHECK (storage_mode = 'PLAINTEXT_TEMPORARY_SQLITE'),
    storage_encryption TEXT NOT NULL CHECK (storage_encryption = 'NONE'),
    production_data_allowed INTEGER NOT NULL CHECK (production_data_allowed = 0),
    product_network TEXT NOT NULL CHECK (product_network = 'NONE'),
    creating_build_digest BLOB NOT NULL CHECK (
        typeof(creating_build_digest) = 'blob' AND length(creating_build_digest) = 32
    ),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
) STRICT;

CREATE TABLE ledger_batch (
    batch_id BLOB PRIMARY KEY CHECK (typeof(batch_id) = 'blob' AND length(batch_id) = 16),
    signed_envelope BLOB NOT NULL CHECK (typeof(signed_envelope) = 'blob' AND length(signed_envelope) > 0),
    envelope_hash BLOB NOT NULL UNIQUE CHECK (typeof(envelope_hash) = 'blob' AND length(envelope_hash) = 32),
    deterministic_payload BLOB NOT NULL CHECK (
        typeof(deterministic_payload) = 'blob' AND length(deterministic_payload) > 0
    ),
    deterministic_payload_hash BLOB NOT NULL CHECK (
        typeof(deterministic_payload_hash) = 'blob' AND length(deterministic_payload_hash) = 32
    ),
    signing_public_key BLOB NOT NULL CHECK (
        typeof(signing_public_key) = 'blob' AND length(signing_public_key) = 32
    ),
    signature BLOB NOT NULL CHECK (typeof(signature) = 'blob' AND length(signature) = 64),
    device_id BLOB NOT NULL CHECK (typeof(device_id) = 'blob' AND length(device_id) = 16),
    origin_seq_start INTEGER NOT NULL CHECK (origin_seq_start >= 1),
    origin_seq_end INTEGER NOT NULL CHECK (origin_seq_end >= origin_seq_start),
    previous_batch_hash BLOB CHECK (
        previous_batch_hash IS NULL
        OR (typeof(previous_batch_hash) = 'blob' AND length(previous_batch_hash) = 32)
    ),
    origin_created_at INTEGER NOT NULL,
    event_schema_version INTEGER NOT NULL CHECK (event_schema_version >= 1),
    accept_seq_start INTEGER NOT NULL CHECK (accept_seq_start >= 1),
    accept_seq_end INTEGER NOT NULL CHECK (accept_seq_end >= accept_seq_start),
    accepted_at INTEGER NOT NULL,
    CHECK (
        (origin_seq_start = 1 AND previous_batch_hash IS NULL)
        OR (origin_seq_start > 1 AND previous_batch_hash IS NOT NULL)
    ),
    CHECK ((origin_seq_end - origin_seq_start) = (accept_seq_end - accept_seq_start)),
    UNIQUE (device_id, origin_seq_start),
    UNIQUE (device_id, origin_seq_end),
    UNIQUE (device_id, deterministic_payload_hash)
) STRICT;

CREATE TABLE ledger_event (
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
            'CLAIM_ASSERTED', 'CLAIM_RELATED', 'DECISION_RECORDED'
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

CREATE TABLE scope (
    scope_id BLOB PRIMARY KEY CHECK (typeof(scope_id) = 'blob' AND length(scope_id) = 16),
    created_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    label TEXT NOT NULL CHECK (length(trim(label)) > 0)
) STRICT;

CREATE TABLE artifact_descriptor (
    artifact_id BLOB PRIMARY KEY CHECK (typeof(artifact_id) = 'blob' AND length(artifact_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    content_digest BLOB NOT NULL CHECK (
        typeof(content_digest) = 'blob' AND length(content_digest) = 32
    ),
    media_type TEXT NOT NULL CHECK (length(trim(media_type)) > 0),
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    domain_id BLOB NOT NULL CHECK (typeof(domain_id) = 'blob' AND length(domain_id) = 16),
    confidentiality TEXT NOT NULL CHECK (
        confidentiality IN ('PUBLIC', 'PERSONAL', 'RESTRICTED', 'SECRET')
    ),
    retention_class TEXT NOT NULL CHECK (
        retention_class IN ('EPHEMERAL', 'COURSE_TERM', 'USER_MANAGED', 'LEGAL_HOLD')
    ),
    permission_lineage_id BLOB NOT NULL CHECK (
        typeof(permission_lineage_id) = 'blob' AND length(permission_lineage_id) = 16
    ),
    format_version INTEGER NOT NULL CHECK (format_version BETWEEN 1 AND 65535),
    vault_locator BLOB NOT NULL CHECK (typeof(vault_locator) = 'blob' AND length(vault_locator) = 32),
    UNIQUE (domain_id, retention_class, permission_lineage_id, vault_locator)
) STRICT;

CREATE TABLE artifact_representation (
    artifact_id BLOB NOT NULL
        REFERENCES artifact_descriptor(artifact_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    representation_index INTEGER NOT NULL CHECK (representation_index >= 0),
    locator_kind TEXT NOT NULL CHECK (
        locator_kind IN ('TEXT_BYTES', 'PAGE', 'TRANSCRIPT_TIME', 'REPOSITORY_BYTES')
    ),
    locator_payload BLOB NOT NULL CHECK (typeof(locator_payload) = 'blob' AND length(locator_payload) > 0),
    content_digest BLOB NOT NULL CHECK (
        typeof(content_digest) = 'blob' AND length(content_digest) = 32
    ),
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    PRIMARY KEY (artifact_id, representation_index),
    UNIQUE (artifact_id, locator_kind, locator_payload)
) STRICT;

CREATE TABLE evidence_item (
    evidence_id BLOB PRIMARY KEY CHECK (typeof(evidence_id) = 'blob' AND length(evidence_id) = 16),
    registered_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    artifact_id BLOB NOT NULL CHECK (typeof(artifact_id) = 'blob' AND length(artifact_id) = 16),
    representation_index INTEGER NOT NULL CHECK (representation_index >= 0),
    excerpt_digest BLOB NOT NULL CHECK (
        typeof(excerpt_digest) = 'blob' AND length(excerpt_digest) = 32
    ),
    evidence_role TEXT NOT NULL CHECK (
        evidence_role IN ('SUPPORTS', 'CONTRADICTS', 'CONTEXT_ONLY')
    ),
    evidence_strength TEXT NOT NULL CHECK (
        evidence_strength IN ('DIRECT', 'CORROBORATING', 'WEAK')
    ),
    extraction_method TEXT NOT NULL CHECK (length(trim(extraction_method)) > 0),
    extractor_version TEXT NOT NULL CHECK (length(trim(extractor_version)) > 0),
    FOREIGN KEY (artifact_id, representation_index)
        REFERENCES artifact_representation(artifact_id, representation_index)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TABLE claim (
    claim_id BLOB PRIMARY KEY CHECK (typeof(claim_id) = 'blob' AND length(claim_id) = 16),
    assertion_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    subject_entity_id BLOB NOT NULL CHECK (
        typeof(subject_entity_id) = 'blob' AND length(subject_entity_id) = 16
    ),
    predicate_id TEXT NOT NULL CHECK (
        length(predicate_id) >= 3
        AND predicate_id NOT GLOB '*[^a-z0-9.]*'
        AND predicate_id NOT LIKE '.%'
        AND predicate_id NOT LIKE '%.'
        AND instr(predicate_id, '.') > 1
        AND instr(predicate_id, '..') = 0
    ),
    scope_id BLOB NOT NULL REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    object_kind TEXT NOT NULL CHECK (
        object_kind IN (
            'ENTITY', 'TEXT', 'INTEGER', 'BOOLEAN', 'DECIMAL',
            'INSTANT', 'INTERVAL', 'MASTERY', 'FRESHNESS'
        )
    ),
    object_entity_id BLOB CHECK (
        object_entity_id IS NULL
        OR (typeof(object_entity_id) = 'blob' AND length(object_entity_id) = 16)
    ),
    object_text TEXT,
    object_integer INTEGER,
    object_decimal_coefficient TEXT,
    object_decimal_scale INTEGER CHECK (
        object_decimal_scale IS NULL OR object_decimal_scale BETWEEN 0 AND 18
    ),
    object_interval_from INTEGER,
    object_interval_to INTEGER,
    authority_class TEXT NOT NULL CHECK (
        authority_class IN (
            'OFFICIAL', 'USER_EXPLICIT', 'DIRECT_OBSERVATION', 'CURATED',
            'DETERMINISTIC_ENGINE', 'MODEL_INFERENCE', 'PREDICTION', 'UNKNOWN'
        )
    ),
    epistemic_status TEXT NOT NULL CHECK (
        epistemic_status IN (
            'OFFICIAL_CONFIRMED', 'USER_CONFIRMED', 'CODE_OBSERVED',
            'DETERMINISTIC_DERIVED', 'AI_INFERRED', 'PREDICTION',
            'DISPUTED', 'SUPERSEDED', 'UNKNOWN'
        )
    ),
    confidence_permille INTEGER CHECK (
        confidence_permille IS NULL OR confidence_permille BETWEEN 0 AND 1000
    ),
    prediction_metadata_version INTEGER,
    prediction_observation_from INTEGER,
    prediction_observation_to INTEGER,
    prediction_sample_count INTEGER,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from),
    CHECK (
        (object_kind = 'ENTITY'
            AND object_entity_id IS NOT NULL
            AND object_text IS NULL AND object_integer IS NULL
            AND object_decimal_coefficient IS NULL AND object_decimal_scale IS NULL
            AND object_interval_from IS NULL AND object_interval_to IS NULL)
        OR (object_kind = 'TEXT'
            AND object_entity_id IS NULL AND object_text IS NOT NULL
            AND object_integer IS NULL
            AND object_decimal_coefficient IS NULL AND object_decimal_scale IS NULL
            AND object_interval_from IS NULL AND object_interval_to IS NULL)
        OR (object_kind IN ('INTEGER', 'INSTANT')
            AND object_entity_id IS NULL AND object_text IS NULL
            AND object_integer IS NOT NULL
            AND object_decimal_coefficient IS NULL AND object_decimal_scale IS NULL
            AND object_interval_from IS NULL AND object_interval_to IS NULL)
        OR (object_kind = 'BOOLEAN'
            AND object_entity_id IS NULL AND object_text IS NULL
            AND object_integer IN (0, 1)
            AND object_decimal_coefficient IS NULL AND object_decimal_scale IS NULL
            AND object_interval_from IS NULL AND object_interval_to IS NULL)
        OR (object_kind = 'DECIMAL'
            AND object_entity_id IS NULL AND object_text IS NULL AND object_integer IS NULL
            AND object_decimal_coefficient IS NOT NULL
            AND length(object_decimal_coefficient) BETWEEN 1 AND 40
            AND (
                object_decimal_coefficient = '0'
                OR (
                    object_decimal_coefficient GLOB '[1-9]*'
                    AND object_decimal_coefficient NOT GLOB '*[^0-9]*'
                )
                OR (
                    object_decimal_coefficient GLOB '-[1-9]*'
                    AND substr(object_decimal_coefficient, 2) NOT GLOB '*[^0-9]*'
                )
            )
            AND object_decimal_scale IS NOT NULL
            AND object_interval_from IS NULL AND object_interval_to IS NULL)
        OR (object_kind = 'INTERVAL'
            AND object_entity_id IS NULL AND object_text IS NULL AND object_integer IS NULL
            AND object_decimal_coefficient IS NULL AND object_decimal_scale IS NULL
            AND object_interval_from IS NOT NULL
            AND (object_interval_to IS NULL OR object_interval_to > object_interval_from))
        OR (object_kind = 'MASTERY'
            AND object_entity_id IS NULL
            AND object_text IN ('UNSEEN', 'EXPOSED', 'UNDERSTOOD', 'PRACTICED', 'APPLIED', 'FLUENT')
            AND object_integer IS NULL
            AND object_decimal_coefficient IS NULL AND object_decimal_scale IS NULL
            AND object_interval_from IS NULL AND object_interval_to IS NULL)
        OR (object_kind = 'FRESHNESS'
            AND object_entity_id IS NULL
            AND object_text IN ('UNKNOWN', 'STALE', 'LOW', 'MODERATE', 'HIGH', 'VERY_HIGH')
            AND object_integer IS NULL
            AND object_decimal_coefficient IS NULL AND object_decimal_scale IS NULL
            AND object_interval_from IS NULL AND object_interval_to IS NULL)
    ),
    CHECK (
        (epistemic_status = 'OFFICIAL_CONFIRMED' AND authority_class = 'OFFICIAL')
        OR (epistemic_status = 'USER_CONFIRMED' AND authority_class = 'USER_EXPLICIT')
        OR (epistemic_status = 'CODE_OBSERVED' AND authority_class = 'DIRECT_OBSERVATION')
        OR (epistemic_status = 'DETERMINISTIC_DERIVED'
            AND authority_class IN ('CURATED', 'DETERMINISTIC_ENGINE'))
        OR (epistemic_status = 'AI_INFERRED' AND authority_class = 'MODEL_INFERENCE')
        OR (epistemic_status = 'PREDICTION' AND authority_class = 'PREDICTION')
        OR epistemic_status IN ('DISPUTED', 'SUPERSEDED')
        OR (epistemic_status = 'UNKNOWN' AND authority_class = 'UNKNOWN')
    ),
    CHECK (
        epistemic_status NOT IN ('OFFICIAL_CONFIRMED', 'UNKNOWN')
        OR confidence_permille IS NULL
    ),
    CHECK (
        (epistemic_status = 'PREDICTION'
            AND confidence_permille IS NOT NULL
            AND prediction_metadata_version = 1
            AND prediction_observation_from IS NOT NULL
            AND prediction_observation_to > prediction_observation_from
            AND prediction_sample_count > 0)
        OR (epistemic_status <> 'PREDICTION'
            AND prediction_metadata_version IS NULL
            AND prediction_observation_from IS NULL
            AND prediction_observation_to IS NULL
            AND prediction_sample_count IS NULL)
    )
) STRICT;

CREATE TABLE claim_evidence (
    claim_id BLOB NOT NULL REFERENCES claim(claim_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    evidence_id BLOB NOT NULL
        REFERENCES evidence_item(evidence_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    evidence_ordinal INTEGER NOT NULL CHECK (evidence_ordinal >= 0),
    PRIMARY KEY (claim_id, evidence_id),
    UNIQUE (claim_id, evidence_ordinal)
) STRICT;

CREATE TABLE claim_relation (
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
        actor_kind IN ('USER', 'DETERMINISTIC_ENGINE', 'MODEL_RUN', 'IMPORTER')
    ),
    CHECK (source_claim_id <> target_claim_id),
    UNIQUE (source_claim_id, target_claim_id, relation_kind, scope_id)
) STRICT;

CREATE TABLE user_decision (
    decision_id BLOB PRIMARY KEY CHECK (typeof(decision_id) = 'blob' AND length(decision_id) = 16),
    decision_event_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    target_claim_id BLOB NOT NULL
        REFERENCES claim(claim_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    target_object_canonical BLOB NOT NULL CHECK (
        typeof(target_object_canonical) = 'blob' AND length(target_object_canonical) > 0
    ),
    resolution_subject_entity_id BLOB NOT NULL CHECK (
        typeof(resolution_subject_entity_id) = 'blob' AND length(resolution_subject_entity_id) = 16
    ),
    resolution_predicate_id TEXT NOT NULL CHECK (
        length(resolution_predicate_id) >= 3
        AND resolution_predicate_id NOT GLOB '*[^a-z0-9.]*'
        AND resolution_predicate_id NOT LIKE '.%'
        AND resolution_predicate_id NOT LIKE '%.'
        AND instr(resolution_predicate_id, '.') > 1
        AND instr(resolution_predicate_id, '..') = 0
    ),
    resolution_scope_id BLOB NOT NULL
        REFERENCES scope(scope_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    action TEXT NOT NULL CHECK (action IN ('CONFIRM', 'REJECT', 'REPLACE')),
    replacement_claim_id BLOB
        REFERENCES claim(claim_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from),
    rationale_evidence_ids_canonical BLOB NOT NULL CHECK (
        typeof(rationale_evidence_ids_canonical) = 'blob'
        AND length(rationale_evidence_ids_canonical) > 0
    ),
    decided_at INTEGER NOT NULL,
    reversible_until INTEGER CHECK (reversible_until IS NULL OR reversible_until > decided_at),
    CHECK (
        (action = 'REPLACE' AND replacement_claim_id IS NOT NULL
            AND replacement_claim_id <> target_claim_id)
        OR (action IN ('CONFIRM', 'REJECT') AND replacement_claim_id IS NULL)
    )
) STRICT;

CREATE TABLE projection_outbox (
    outbox_seq INTEGER PRIMARY KEY CHECK (outbox_seq >= 1),
    accepted_batch_id BLOB NOT NULL
        REFERENCES ledger_batch(batch_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    accept_seq_start INTEGER NOT NULL CHECK (accept_seq_start >= 1),
    accept_seq_end INTEGER NOT NULL CHECK (accept_seq_end >= accept_seq_start),
    canonical_revision INTEGER NOT NULL UNIQUE CHECK (canonical_revision >= 1),
    event_kind_mask BLOB NOT NULL CHECK (
        typeof(event_kind_mask) = 'blob' AND length(event_kind_mask) = 8
    ),
    payload_digest BLOB NOT NULL CHECK (
        typeof(payload_digest) = 'blob' AND length(payload_digest) = 32
    ),
    created_at INTEGER NOT NULL,
    UNIQUE (accepted_batch_id, accept_seq_start, accept_seq_end)
) STRICT;

CREATE TABLE command_receipt (
    client_instance_id BLOB NOT NULL CHECK (
        typeof(client_instance_id) = 'blob' AND length(client_instance_id) = 16
    ),
    idempotency_key BLOB NOT NULL CHECK (
        typeof(idempotency_key) = 'blob' AND length(idempotency_key) = 32
    ),
    request_hash BLOB NOT NULL CHECK (typeof(request_hash) = 'blob' AND length(request_hash) = 32),
    expected_revision INTEGER CHECK (expected_revision IS NULL OR expected_revision >= 0),
    committed_revision INTEGER NOT NULL CHECK (committed_revision >= 1),
    response_bytes BLOB NOT NULL CHECK (typeof(response_bytes) = 'blob' AND length(response_bytes) > 0),
    response_hash BLOB NOT NULL CHECK (
        typeof(response_hash) = 'blob' AND length(response_hash) = 32
    ),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (client_instance_id, idempotency_key),
    UNIQUE (client_instance_id, idempotency_key, request_hash)
) STRICT;

CREATE TABLE replica_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    next_accept_seq INTEGER NOT NULL CHECK (next_accept_seq >= 1),
    profile_revision INTEGER NOT NULL CHECK (profile_revision >= 0)
) STRICT;

INSERT INTO replica_state (singleton, next_accept_seq, profile_revision) VALUES (1, 1, 0);

CREATE TABLE device_head (
    device_id BLOB PRIMARY KEY CHECK (typeof(device_id) = 'blob' AND length(device_id) = 16),
    next_origin_seq INTEGER NOT NULL CHECK (next_origin_seq >= 1),
    head_batch_id BLOB NOT NULL UNIQUE
        REFERENCES ledger_batch(batch_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    head_envelope_hash BLOB NOT NULL CHECK (
        typeof(head_envelope_hash) = 'blob' AND length(head_envelope_hash) = 32
    ),
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE projection_cursor (
    projection_kind TEXT PRIMARY KEY CHECK (length(trim(projection_kind)) > 0),
    last_outbox_seq INTEGER NOT NULL CHECK (last_outbox_seq >= 0),
    source_accept_seq INTEGER NOT NULL CHECK (source_accept_seq >= 0),
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE projection_active (
    projection_kind TEXT PRIMARY KEY CHECK (length(trim(projection_kind)) > 0),
    generation_id BLOB NOT NULL CHECK (typeof(generation_id) = 'blob' AND length(generation_id) = 16),
    source_accept_seq INTEGER NOT NULL CHECK (source_accept_seq >= 0),
    activated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE ingest_lease (
    lease_id BLOB PRIMARY KEY CHECK (typeof(lease_id) = 'blob' AND length(lease_id) = 16),
    owner_instance_id BLOB NOT NULL CHECK (
        typeof(owner_instance_id) = 'blob' AND length(owner_instance_id) = 16
    ),
    acquired_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL CHECK (expires_at > acquired_at),
    lease_state TEXT NOT NULL CHECK (lease_state IN ('ACTIVE', 'RELEASING')),
    UNIQUE (owner_instance_id)
) STRICT;

CREATE INDEX idx_ledger_batch_device_range
    ON ledger_batch(device_id, origin_seq_start, origin_seq_end);
CREATE INDEX idx_ledger_event_batch_origin ON ledger_event(batch_id, origin_seq);
CREATE INDEX idx_ledger_event_domain_accept ON ledger_event(domain_id, accept_seq);
CREATE INDEX idx_scope_domain ON scope(domain_id, scope_id);
CREATE INDEX idx_artifact_domain ON artifact_descriptor(domain_id, artifact_id);
CREATE INDEX idx_evidence_artifact ON evidence_item(artifact_id, representation_index);
CREATE INDEX idx_claim_resolution
    ON claim(subject_entity_id, predicate_id, scope_id, valid_from, valid_to);
CREATE INDEX idx_claim_assertion_event ON claim(assertion_event_id);
CREATE INDEX idx_claim_evidence_evidence ON claim_evidence(evidence_id, claim_id);
CREATE INDEX idx_claim_relation_target ON claim_relation(target_claim_id, scope_id);
CREATE INDEX idx_user_decision_slot
    ON user_decision(
        resolution_subject_entity_id,
        resolution_predicate_id,
        resolution_scope_id,
        valid_from,
        valid_to
    );
CREATE INDEX idx_projection_outbox_accept ON projection_outbox(accept_seq_end, outbox_seq);
CREATE INDEX idx_ingest_lease_expiry ON ingest_lease(expires_at);

CREATE TRIGGER guard_schema_meta_update BEFORE UPDATE ON schema_meta
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_schema_meta_delete BEFORE DELETE ON schema_meta
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_ledger_batch_update BEFORE UPDATE ON ledger_batch
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_ledger_batch_delete BEFORE DELETE ON ledger_batch
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_ledger_event_update BEFORE UPDATE ON ledger_event
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_ledger_event_delete BEFORE DELETE ON ledger_event
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_scope_update BEFORE UPDATE ON scope
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_scope_delete BEFORE DELETE ON scope
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_artifact_descriptor_update BEFORE UPDATE ON artifact_descriptor
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_artifact_descriptor_delete BEFORE DELETE ON artifact_descriptor
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_artifact_representation_update BEFORE UPDATE ON artifact_representation
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_artifact_representation_delete BEFORE DELETE ON artifact_representation
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_evidence_item_update BEFORE UPDATE ON evidence_item
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_evidence_item_delete BEFORE DELETE ON evidence_item
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_claim_update BEFORE UPDATE ON claim
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_claim_delete BEFORE DELETE ON claim
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_claim_evidence_update BEFORE UPDATE ON claim_evidence
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_claim_evidence_delete BEFORE DELETE ON claim_evidence
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_claim_relation_update BEFORE UPDATE ON claim_relation
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_claim_relation_delete BEFORE DELETE ON claim_relation
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_user_decision_update BEFORE UPDATE ON user_decision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_user_decision_delete BEFORE DELETE ON user_decision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_projection_outbox_update BEFORE UPDATE ON projection_outbox
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_projection_outbox_delete BEFORE DELETE ON projection_outbox
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_command_receipt_update BEFORE UPDATE ON command_receipt
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_command_receipt_delete BEFORE DELETE ON command_receipt
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
