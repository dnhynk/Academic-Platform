-- P2-G1/P2-G3/P2-G7 permission, provider-policy, and process-capability operational store. The canonical store's migration
-- number 0005 is already occupied by P2-K5, so this schema is owned and applied
-- by academic-policy rather than silently taking a second store migration 0005.

CREATE TABLE IF NOT EXISTS policy_schema_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version = 2),
    audit_retention_policy_id TEXT NOT NULL
        CHECK (audit_retention_policy_id = 'SECURITY_AUDIT_APPEND_ONLY')
) STRICT;

INSERT OR IGNORE INTO policy_schema_meta (
    singleton, schema_version, audit_retention_policy_id
) VALUES (1, 2, 'SECURITY_AUDIT_APPEND_ONLY');

CREATE TRIGGER IF NOT EXISTS guard_policy_schema_meta_update
BEFORE UPDATE ON policy_schema_meta
BEGIN
    SELECT RAISE(ABORT, 'policy schema identity is immutable');
END;

CREATE TRIGGER IF NOT EXISTS guard_policy_schema_meta_delete
BEFORE DELETE ON policy_schema_meta
BEGIN
    SELECT RAISE(ABORT, 'policy schema identity is immutable');
END;

CREATE TABLE IF NOT EXISTS provider_policy_snapshot (
    snapshot_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_digest TEXT NOT NULL UNIQUE CHECK (length(snapshot_digest) = 64),
    destination_id TEXT NOT NULL,
    vendor_id TEXT NOT NULL CHECK (length(vendor_id) > 0),
    surface TEXT NOT NULL CHECK (surface IN ('ENTERPRISE_API', 'CONSUMER_UI')),
    training_use_enabled INTEGER NOT NULL CHECK (training_use_enabled IN (0, 1)),
    training_opt_out_applied INTEGER NOT NULL CHECK (training_opt_out_applied IN (0, 1)),
    server_retention_millis INTEGER NOT NULL CHECK (server_retention_millis >= 0),
    abuse_logging_enabled INTEGER NOT NULL CHECK (abuse_logging_enabled IN (0, 1)),
    transit_encryption_declared INTEGER NOT NULL
        CHECK (transit_encryption_declared IN (0, 1)),
    at_rest_encryption_declared INTEGER NOT NULL
        CHECK (at_rest_encryption_declared IN (0, 1)),
    deletion_api_available INTEGER NOT NULL CHECK (deletion_api_available IN (0, 1)),
    deletion_receipt_capable INTEGER NOT NULL CHECK (
        deletion_receipt_capable IN (0, 1)
        AND (deletion_receipt_capable = 0 OR deletion_api_available = 1)
    ),
    maximum_input_bytes INTEGER NOT NULL CHECK (maximum_input_bytes > 0),
    logging_configuration TEXT NOT NULL CHECK (length(logging_configuration) > 0),
    policy_source_digest TEXT NOT NULL CHECK (length(policy_source_digest) = 64),
    last_verified_at INTEGER NOT NULL CHECK (last_verified_at >= 0),
    ttl_millis INTEGER NOT NULL CHECK (ttl_millis > 0),
    registered_at INTEGER NOT NULL CHECK (registered_at >= last_verified_at),
    UNIQUE (snapshot_digest, destination_id)
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_provider_policy_snapshot_update
BEFORE UPDATE ON provider_policy_snapshot
BEGIN
    SELECT RAISE(ABORT, 'provider_policy_snapshot is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_provider_policy_snapshot_delete
BEFORE DELETE ON provider_policy_snapshot
BEGIN
    SELECT RAISE(ABORT, 'provider_policy_snapshot is append-only');
END;

CREATE TABLE IF NOT EXISTS provider_policy_residency (
    snapshot_digest TEXT NOT NULL
        REFERENCES provider_policy_snapshot(snapshot_digest) ON UPDATE RESTRICT ON DELETE RESTRICT,
    region TEXT NOT NULL CHECK (length(region) > 0),
    PRIMARY KEY (snapshot_digest, region)
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_provider_policy_residency_update
BEFORE UPDATE ON provider_policy_residency
BEGIN
    SELECT RAISE(ABORT, 'provider_policy_residency is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_provider_policy_residency_delete
BEFORE DELETE ON provider_policy_residency
BEGIN
    SELECT RAISE(ABORT, 'provider_policy_residency is append-only');
END;

CREATE TABLE IF NOT EXISTS provider_policy_subprocessor (
    snapshot_digest TEXT NOT NULL
        REFERENCES provider_policy_snapshot(snapshot_digest) ON UPDATE RESTRICT ON DELETE RESTRICT,
    subprocessor TEXT NOT NULL CHECK (length(subprocessor) > 0),
    PRIMARY KEY (snapshot_digest, subprocessor)
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_provider_policy_subprocessor_update
BEFORE UPDATE ON provider_policy_subprocessor
BEGIN
    SELECT RAISE(ABORT, 'provider_policy_subprocessor is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_provider_policy_subprocessor_delete
BEFORE DELETE ON provider_policy_subprocessor
BEGIN
    SELECT RAISE(ABORT, 'provider_policy_subprocessor is append-only');
END;

CREATE TABLE IF NOT EXISTS provider_user_policy (
    user_policy_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    policy_id TEXT NOT NULL UNIQUE CHECK (length(policy_id) > 0),
    destination_id TEXT NOT NULL,
    provider_policy_snapshot_digest TEXT NOT NULL,
    allow_without_deletion_api INTEGER NOT NULL CHECK (allow_without_deletion_api IN (0, 1)),
    require_transit_encryption INTEGER NOT NULL CHECK (require_transit_encryption IN (0, 1)),
    require_at_rest_encryption INTEGER NOT NULL CHECK (require_at_rest_encryption IN (0, 1)),
    decision_evidence_id TEXT NOT NULL CHECK (length(decision_evidence_id) > 0),
    recorded_at INTEGER NOT NULL CHECK (recorded_at >= 0),
    FOREIGN KEY (provider_policy_snapshot_digest, destination_id)
        REFERENCES provider_policy_snapshot(snapshot_digest, destination_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_provider_user_policy_update
BEFORE UPDATE ON provider_user_policy
BEGIN
    SELECT RAISE(ABORT, 'provider_user_policy is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_provider_user_policy_delete
BEFORE DELETE ON provider_user_policy
BEGIN
    SELECT RAISE(ABORT, 'provider_user_policy is append-only');
END;

CREATE TABLE IF NOT EXISTS provider_user_policy_residency (
    policy_id TEXT NOT NULL
        REFERENCES provider_user_policy(policy_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    region TEXT NOT NULL CHECK (length(region) > 0),
    PRIMARY KEY (policy_id, region)
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_provider_user_policy_residency_update
BEFORE UPDATE ON provider_user_policy_residency
BEGIN
    SELECT RAISE(ABORT, 'provider_user_policy_residency is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_provider_user_policy_residency_delete
BEFORE DELETE ON provider_user_policy_residency
BEGIN
    SELECT RAISE(ABORT, 'provider_user_policy_residency is append-only');
END;

CREATE TABLE IF NOT EXISTS egress_grant (
    grant_id TEXT PRIMARY KEY,
    request_digest TEXT NOT NULL CHECK (length(request_digest) = 64),
    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 64),
    byte_ranges_canonical TEXT NOT NULL CHECK (length(byte_ranges_canonical) > 0),
    purpose_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    provider_policy_snapshot_digest TEXT NOT NULL
        CHECK (length(provider_policy_snapshot_digest) = 64),
    retention_terms_hash TEXT NOT NULL CHECK (length(retention_terms_hash) = 64),
    training_use_allowed INTEGER NOT NULL CHECK (training_use_allowed IN (0, 1)),
    redaction_policy_hash TEXT NOT NULL CHECK (length(redaction_policy_hash) = 64),
    issued_at INTEGER NOT NULL CHECK (issued_at >= 0),
    expires_at INTEGER NOT NULL CHECK (expires_at > issued_at),
    max_uses INTEGER NOT NULL DEFAULT 1 CHECK (max_uses = 1),
    consumed_at INTEGER CHECK (
        consumed_at IS NULL OR (consumed_at >= issued_at AND consumed_at < expires_at)
    ),
    consent_event_id TEXT NOT NULL,
    UNIQUE (grant_id, provider_policy_snapshot_digest),
    FOREIGN KEY (provider_policy_snapshot_digest, provider_id)
        REFERENCES provider_policy_snapshot(snapshot_digest, destination_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_egress_grant_delete
BEFORE DELETE ON egress_grant
BEGIN
    SELECT RAISE(ABORT, 'egress_grant is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_egress_grant_update
BEFORE UPDATE ON egress_grant
WHEN NOT (
    OLD.consumed_at IS NULL AND NEW.consumed_at IS NOT NULL
    AND NEW.grant_id = OLD.grant_id
    AND NEW.request_digest = OLD.request_digest
    AND NEW.payload_digest = OLD.payload_digest
    AND NEW.byte_ranges_canonical = OLD.byte_ranges_canonical
    AND NEW.purpose_id = OLD.purpose_id
    AND NEW.provider_id = OLD.provider_id
    AND NEW.provider_policy_snapshot_digest = OLD.provider_policy_snapshot_digest
    AND NEW.retention_terms_hash = OLD.retention_terms_hash
    AND NEW.training_use_allowed = OLD.training_use_allowed
    AND NEW.redaction_policy_hash = OLD.redaction_policy_hash
    AND NEW.issued_at = OLD.issued_at
    AND NEW.expires_at = OLD.expires_at
    AND NEW.max_uses = OLD.max_uses
    AND NEW.consent_event_id = OLD.consent_event_id
)
BEGIN
    SELECT RAISE(ABORT, 'only first capability consumption may update a grant');
END;

CREATE TABLE IF NOT EXISTS egress_audit (
    audit_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    grant_id TEXT CHECK (grant_id IS NULL OR length(grant_id) = 64),
    decision TEXT NOT NULL CHECK (decision IN ('ALLOW', 'DENY')),
    reason_code TEXT CHECK (
        (decision = 'ALLOW' AND reason_code IS NULL)
        OR (decision = 'DENY' AND reason_code IN (
            'NO_GRANT', 'GRANT_EXPIRED', 'GRANT_CONSUMED', 'SCOPE_MISMATCH',
            'POLICY_STALE', 'PROVIDER_POLICY_INCOMPATIBLE', 'SCANNER_ERROR',
            'SECRET_PATTERN', 'SECRET_ENTROPY', 'PII_DETECTED', 'UNKNOWN_BINARY',
            'OVERSIZE', 'REDACTION_DESTROYS_MEANING', 'CANARY_IN_RESPONSE',
            'NO_DELETION_RECEIPT'
        ))
    ),
    actor_id TEXT NOT NULL CHECK (length(actor_id) > 0),
    actor_process_class TEXT NOT NULL CHECK (actor_process_class IN (
        'CAPTURE_CLIENT', 'INDEXER', 'REPOSITORY_ANALYZER', 'CONNECTOR',
        'EGRESS_PROXY', 'EXPORT_JOB'
    )),
    capability TEXT NOT NULL CHECK (capability IN (
        'CAPTURE_DEVICE', 'WRITE_STAGED_ARTIFACT', 'READ_ARTIFACT_RANGE',
        'WRITE_SEARCH_INDEX', 'ANALYZE_REPOSITORY', 'BORROW_CONNECTOR_CREDENTIAL',
        'STAGE_EXTERNAL_PAYLOAD', 'OPEN_OUTBOUND_SOCKET', 'CREATE_CLAIM',
        'ASSEMBLE_EXPORT', 'READ_KEY_MATERIAL'
    )),
    payload_digest TEXT CHECK (payload_digest IS NULL OR length(payload_digest) = 64),
    byte_count INTEGER NOT NULL CHECK (byte_count >= 0),
    destination_id TEXT NOT NULL,
    started_at INTEGER NOT NULL CHECK (started_at >= 0),
    finished_at INTEGER NOT NULL CHECK (finished_at >= started_at),
    provider_response_digest TEXT
        CHECK (provider_response_digest IS NULL OR length(provider_response_digest) = 64),
    deletion_receipt_id TEXT,
    external_transmission_digest TEXT CHECK (
        external_transmission_digest IS NULL OR length(external_transmission_digest) = 64
    ),
    retention_policy_id TEXT NOT NULL
        CHECK (retention_policy_id = 'SECURITY_AUDIT_APPEND_ONLY'),
    UNIQUE (audit_seq, grant_id)
) STRICT;

CREATE TABLE IF NOT EXISTS audit_artifact_range (
    audit_seq INTEGER NOT NULL REFERENCES egress_audit(audit_seq)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    range_ordinal INTEGER NOT NULL CHECK (range_ordinal >= 0),
    object_id TEXT NOT NULL CHECK (length(object_id) > 0),
    byte_start INTEGER NOT NULL CHECK (byte_start >= 0),
    byte_end INTEGER NOT NULL CHECK (byte_end > byte_start),
    content_digest TEXT NOT NULL CHECK (length(content_digest) = 64),
    PRIMARY KEY (audit_seq, range_ordinal)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER IF NOT EXISTS guard_audit_artifact_range_update
BEFORE UPDATE ON audit_artifact_range
BEGIN
    SELECT RAISE(ABORT, 'audit_artifact_range is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_audit_artifact_range_delete
BEFORE DELETE ON audit_artifact_range
BEGIN
    SELECT RAISE(ABORT, 'audit_artifact_range is append-only');
END;

CREATE TABLE IF NOT EXISTS audit_created_claim (
    audit_seq INTEGER NOT NULL REFERENCES egress_audit(audit_seq)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    claim_ordinal INTEGER NOT NULL CHECK (claim_ordinal >= 0),
    claim_id TEXT NOT NULL CHECK (length(claim_id) > 0),
    PRIMARY KEY (audit_seq, claim_ordinal)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER IF NOT EXISTS guard_audit_created_claim_update
BEFORE UPDATE ON audit_created_claim
BEGIN
    SELECT RAISE(ABORT, 'audit_created_claim is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_audit_created_claim_delete
BEFORE DELETE ON audit_created_claim
BEGIN
    SELECT RAISE(ABORT, 'audit_created_claim is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_egress_audit_update
BEFORE UPDATE ON egress_audit
BEGIN
    SELECT RAISE(ABORT, 'egress_audit is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_egress_audit_delete
BEFORE DELETE ON egress_audit
BEGIN
    SELECT RAISE(ABORT, 'egress_audit is append-only');
END;

CREATE TABLE IF NOT EXISTS egress_consumption (
    grant_id TEXT PRIMARY KEY
        REFERENCES egress_grant(grant_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    egress_audit_seq INTEGER NOT NULL UNIQUE
        REFERENCES egress_audit(audit_seq) ON UPDATE RESTRICT ON DELETE RESTRICT,
    consumed_at INTEGER NOT NULL CHECK (consumed_at >= 0),
    UNIQUE (egress_audit_seq, grant_id),
    FOREIGN KEY (egress_audit_seq, grant_id)
        REFERENCES egress_audit(audit_seq, grant_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_egress_consumption_update
BEFORE UPDATE ON egress_consumption
BEGIN
    SELECT RAISE(ABORT, 'egress_consumption is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_egress_consumption_delete
BEFORE DELETE ON egress_consumption
BEGIN
    SELECT RAISE(ABORT, 'egress_consumption is append-only');
END;

CREATE TABLE IF NOT EXISTS provider_deletion_receipt (
    receipt_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    receipt_id TEXT NOT NULL UNIQUE CHECK (length(receipt_id) > 0),
    grant_id TEXT NOT NULL
        REFERENCES egress_grant(grant_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    egress_audit_seq INTEGER NOT NULL
        REFERENCES egress_audit(audit_seq) ON UPDATE RESTRICT ON DELETE RESTRICT,
    provider_policy_snapshot_digest TEXT NOT NULL
        REFERENCES provider_policy_snapshot(snapshot_digest) ON UPDATE RESTRICT ON DELETE RESTRICT,
    provider_receipt_digest TEXT NOT NULL CHECK (length(provider_receipt_digest) = 64),
    requested_at INTEGER NOT NULL CHECK (requested_at >= 0),
    received_at INTEGER NOT NULL CHECK (received_at >= requested_at),
    FOREIGN KEY (grant_id, provider_policy_snapshot_digest)
        REFERENCES egress_grant(grant_id, provider_policy_snapshot_digest)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (egress_audit_seq, grant_id)
        REFERENCES egress_consumption(egress_audit_seq, grant_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_provider_deletion_receipt_update
BEFORE UPDATE ON provider_deletion_receipt
BEGIN
    SELECT RAISE(ABORT, 'provider_deletion_receipt is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_provider_deletion_receipt_delete
BEFORE DELETE ON provider_deletion_receipt
BEGIN
    SELECT RAISE(ABORT, 'provider_deletion_receipt is append-only');
END;

CREATE TABLE IF NOT EXISTS process_capability_grant (
    token_id TEXT PRIMARY KEY CHECK (length(token_id) = 64),
    actor_id TEXT NOT NULL CHECK (length(actor_id) > 0),
    process_class TEXT NOT NULL CHECK (process_class IN (
        'CAPTURE_CLIENT', 'INDEXER', 'REPOSITORY_ANALYZER', 'CONNECTOR',
        'EGRESS_PROXY', 'EXPORT_JOB'
    )),
    capability TEXT NOT NULL CHECK (capability IN (
        'CAPTURE_DEVICE', 'WRITE_STAGED_ARTIFACT', 'READ_ARTIFACT_RANGE',
        'WRITE_SEARCH_INDEX', 'ANALYZE_REPOSITORY', 'BORROW_CONNECTOR_CREDENTIAL',
        'STAGE_EXTERNAL_PAYLOAD', 'OPEN_OUTBOUND_SOCKET', 'CREATE_CLAIM',
        'ASSEMBLE_EXPORT', 'READ_KEY_MATERIAL'
    )),
    issued_at INTEGER NOT NULL CHECK (issued_at >= 0),
    expires_at INTEGER NOT NULL CHECK (expires_at > issued_at),
    max_uses INTEGER NOT NULL DEFAULT 1 CHECK (max_uses = 1),
    consumed_at INTEGER CHECK (
        consumed_at IS NULL OR (consumed_at >= issued_at AND consumed_at < expires_at)
    )
) STRICT;

CREATE TRIGGER IF NOT EXISTS guard_process_capability_grant_delete
BEFORE DELETE ON process_capability_grant
BEGIN
    SELECT RAISE(ABORT, 'process_capability_grant is append-only');
END;

CREATE TRIGGER IF NOT EXISTS guard_process_capability_grant_update
BEFORE UPDATE ON process_capability_grant
WHEN NOT (
    OLD.consumed_at IS NULL AND NEW.consumed_at IS NOT NULL
    AND NEW.token_id = OLD.token_id
    AND NEW.actor_id = OLD.actor_id
    AND NEW.process_class = OLD.process_class
    AND NEW.capability = OLD.capability
    AND NEW.issued_at = OLD.issued_at
    AND NEW.expires_at = OLD.expires_at
    AND NEW.max_uses = OLD.max_uses
)
BEGIN
    SELECT RAISE(ABORT, 'only first capability consumption may update a process grant');
END;
