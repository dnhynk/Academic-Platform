-- P2-G1 permission-broker operational store. The canonical store's migration
-- number 0005 is already occupied by P2-K5, so this schema is owned and applied
-- by academic-policy rather than silently taking a second store migration 0005.

CREATE TABLE egress_grant (
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
    consent_event_id TEXT NOT NULL
) STRICT;

CREATE TRIGGER guard_egress_grant_delete
BEFORE DELETE ON egress_grant
BEGIN
    SELECT RAISE(ABORT, 'egress_grant is append-only');
END;

CREATE TRIGGER guard_egress_grant_update
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

CREATE TABLE egress_audit (
    audit_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    grant_id TEXT REFERENCES egress_grant(grant_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
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
    actor_process_class TEXT NOT NULL,
    payload_digest TEXT CHECK (payload_digest IS NULL OR length(payload_digest) = 64),
    byte_count INTEGER NOT NULL CHECK (byte_count >= 0),
    destination_id TEXT NOT NULL,
    started_at INTEGER NOT NULL CHECK (started_at >= 0),
    finished_at INTEGER NOT NULL CHECK (finished_at >= started_at),
    provider_response_digest TEXT
        CHECK (provider_response_digest IS NULL OR length(provider_response_digest) = 64),
    deletion_receipt_id TEXT
) STRICT;

CREATE TRIGGER guard_egress_audit_update
BEFORE UPDATE ON egress_audit
BEGIN
    SELECT RAISE(ABORT, 'egress_audit is append-only');
END;

CREATE TRIGGER guard_egress_audit_delete
BEFORE DELETE ON egress_audit
BEGIN
    SELECT RAISE(ABORT, 'egress_audit is append-only');
END;
