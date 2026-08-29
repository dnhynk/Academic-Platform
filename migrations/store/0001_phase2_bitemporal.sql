-- Migration 0001 for the disposable bitemporal time-travel sidecar.
--
-- This is not the canonical store and not the Phase 1 projection sidecar. It is
-- a third database with its own `application_id` (`ACTL`), created beside them
-- under `projections/`, and it holds materialized time-travel snapshots.
--
-- A snapshot here is a cache of what the canonical store already says at one
-- exact `(known_at_accept_seq, valid_at)` coordinate pair. It is never the
-- truth, it is never backed up or exported as truth, and deleting the whole
-- file loses nothing: every row can be recomputed from the ledger.
--
-- That is why no table below carries a `guard_<table>_update` /
-- `guard_<table>_delete` trigger pair and why the product connection's
-- canonical authorizer does not cover them. Those two layers exist to make
-- canonical history append-only. Applying them here would state the opposite of
-- what this file is for: a snapshot that could not be deleted would have become
-- a second ledger.
--
-- What a snapshot may not do is lose track of which projector produced it.
-- `projector_version`, `projector_binary_digest`, and `projector_config_hash`
-- identify the code; `source_ledger_digest` and `source_row_digest` identify the
-- canonical bytes it read. Two snapshots that agree on the source digests and
-- disagree on their rows disagree because the projector changed, and the
-- change-origin labelling has that as a fact rather than as an assumption.

PRAGMA application_id = 1094931532;
PRAGMA user_version = 1;

CREATE TABLE timeline_snapshot (
    snapshot_seq INTEGER PRIMARY KEY CHECK (snapshot_seq >= 1),
    snapshot_id BLOB NOT NULL UNIQUE CHECK (
        typeof(snapshot_id) = 'blob' AND length(snapshot_id) = 16
    ),
    security_domain BLOB NOT NULL CHECK (
        typeof(security_domain) = 'blob' AND length(security_domain) = 16
    ),
    known_at_accept_seq INTEGER NOT NULL CHECK (known_at_accept_seq >= 0),
    valid_at_unix_ms INTEGER NOT NULL,
    projector_version TEXT NOT NULL CHECK (length(trim(projector_version)) > 0),
    projector_binary_digest BLOB NOT NULL CHECK (
        typeof(projector_binary_digest) = 'blob' AND length(projector_binary_digest) = 32
    ),
    projector_config_hash BLOB NOT NULL CHECK (
        typeof(projector_config_hash) = 'blob' AND length(projector_config_hash) = 32
    ),
    source_ledger_digest BLOB NOT NULL CHECK (
        typeof(source_ledger_digest) = 'blob' AND length(source_ledger_digest) = 32
    ),
    source_row_digest BLOB NOT NULL CHECK (
        typeof(source_row_digest) = 'blob' AND length(source_row_digest) = 32
    ),
    -- Whether the canonical profile carries the eighteen aggregate closure
    -- tables at all. A snapshot of a profile that cannot hold them says so,
    -- instead of recording zero aggregate rows and reading as "none were
    -- registered".
    aggregate_lane TEXT NOT NULL CHECK (aggregate_lane IN ('PRESENT', 'ABSENT')),
    latest_accept_seq INTEGER NOT NULL CHECK (latest_accept_seq >= 0),
    built_at_unix_ms INTEGER NOT NULL CHECK (built_at_unix_ms >= 0),
    aggregate_row_count INTEGER NOT NULL CHECK (aggregate_row_count >= 0),
    claim_row_count INTEGER NOT NULL CHECK (claim_row_count >= 0),
    CHECK (aggregate_lane = 'PRESENT' OR aggregate_row_count = 0),
    UNIQUE (security_domain, known_at_accept_seq, valid_at_unix_ms, projector_version)
) STRICT;

CREATE TABLE timeline_snapshot_aggregate (
    snapshot_id BLOB NOT NULL REFERENCES timeline_snapshot(snapshot_id)
        ON UPDATE RESTRICT ON DELETE CASCADE,
    aggregate_kind TEXT NOT NULL CHECK (length(trim(aggregate_kind)) > 0),
    aggregate_id BLOB NOT NULL CHECK (
        typeof(aggregate_id) = 'blob' AND length(aggregate_id) = 16
    ),
    registered_event_id BLOB NOT NULL CHECK (
        typeof(registered_event_id) = 'blob' AND length(registered_event_id) = 16
    ),
    accept_seq INTEGER NOT NULL CHECK (accept_seq >= 1),
    scope_id BLOB NOT NULL CHECK (typeof(scope_id) = 'blob' AND length(scope_id) = 16),
    source_digest BLOB CHECK (
        source_digest IS NULL
        OR (typeof(source_digest) = 'blob' AND length(source_digest) = 32)
    ),
    valid_from_unix_ms INTEGER NOT NULL,
    valid_to_unix_ms INTEGER CHECK (
        valid_to_unix_ms IS NULL OR valid_to_unix_ms > valid_from_unix_ms
    ),
    PRIMARY KEY (snapshot_id, aggregate_kind, aggregate_id)
) STRICT;

CREATE TABLE timeline_snapshot_claim (
    snapshot_id BLOB NOT NULL REFERENCES timeline_snapshot(snapshot_id)
        ON UPDATE RESTRICT ON DELETE CASCADE,
    claim_id BLOB NOT NULL CHECK (typeof(claim_id) = 'blob' AND length(claim_id) = 16),
    subject_entity_id BLOB NOT NULL CHECK (
        typeof(subject_entity_id) = 'blob' AND length(subject_entity_id) = 16
    ),
    predicate_id TEXT NOT NULL CHECK (length(trim(predicate_id)) > 0),
    scope_id BLOB NOT NULL CHECK (typeof(scope_id) = 'blob' AND length(scope_id) = 16),
    accept_seq INTEGER NOT NULL CHECK (accept_seq >= 1),
    authority_class TEXT NOT NULL CHECK (length(trim(authority_class)) > 0),
    epistemic_status TEXT NOT NULL CHECK (length(trim(epistemic_status)) > 0),
    applied_policy TEXT NOT NULL CHECK (
        applied_policy IN (
            'USER_OWNED', 'OFFICIAL_FACT', 'IMPLEMENTATION_OBSERVATION', 'CURATED_RELATION'
        )
    ),
    object_kind TEXT NOT NULL CHECK (
        object_kind IN (
            'ENTITY', 'TEXT', 'INTEGER', 'BOOLEAN', 'DECIMAL',
            'INSTANT', 'INTERVAL', 'MASTERY', 'FRESHNESS'
        )
    ),
    valid_from_unix_ms INTEGER NOT NULL,
    valid_to_unix_ms INTEGER CHECK (
        valid_to_unix_ms IS NULL OR valid_to_unix_ms > valid_from_unix_ms
    ),
    PRIMARY KEY (snapshot_id, claim_id)
) STRICT;
