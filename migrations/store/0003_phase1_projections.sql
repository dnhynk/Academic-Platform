PRAGMA application_id = 1094930514;
PRAGMA user_version = 3;

CREATE TABLE projection_generation (
    generation_seq INTEGER PRIMARY KEY CHECK (generation_seq >= 1),
    generation_id BLOB NOT NULL UNIQUE CHECK (
        typeof(generation_id) = 'blob' AND length(generation_id) = 16
    ),
    projection_kind TEXT NOT NULL CHECK (
        projection_kind IN (
            'relational-graph-v1',
            'fts5-unicode61-v1',
            'fts5-trigram-v1'
        )
    ),
    schema_version INTEGER NOT NULL CHECK (schema_version = 2),
    builder_binary_digest BLOB NOT NULL CHECK (
        typeof(builder_binary_digest) = 'blob' AND length(builder_binary_digest) = 32
    ),
    algorithm_version TEXT NOT NULL CHECK (length(trim(algorithm_version)) > 0),
    tokenizer_version TEXT NOT NULL CHECK (length(trim(tokenizer_version)) > 0),
    effective_config_hash BLOB NOT NULL CHECK (
        typeof(effective_config_hash) = 'blob' AND length(effective_config_hash) = 32
    ),
    known_at_accept_seq INTEGER NOT NULL CHECK (known_at_accept_seq >= 0),
    valid_at_unix_ms INTEGER NOT NULL,
    source_outbox_seq INTEGER NOT NULL CHECK (source_outbox_seq >= 0),
    source_ledger_digest BLOB NOT NULL CHECK (
        typeof(source_ledger_digest) = 'blob' AND length(source_ledger_digest) = 32
    ),
    resolver_version TEXT NOT NULL CHECK (length(trim(resolver_version)) > 0),
    policy_registry_version TEXT NOT NULL CHECK (length(trim(policy_registry_version)) > 0),
    policy_registry_hash BLOB NOT NULL CHECK (
        typeof(policy_registry_hash) = 'blob' AND length(policy_registry_hash) = 32
    ),
    security_domain BLOB NOT NULL CHECK (
        typeof(security_domain) = 'blob' AND length(security_domain) = 16
    ),
    built_at_unix_ms INTEGER NOT NULL CHECK (built_at_unix_ms >= 0),
    state TEXT NOT NULL CHECK (state IN ('BUILDING', 'VERIFIED', 'FAILED')),
    record_count INTEGER CHECK (record_count IS NULL OR record_count >= 0),
    canonical_checksum BLOB CHECK (
        canonical_checksum IS NULL
        OR (typeof(canonical_checksum) = 'blob' AND length(canonical_checksum) = 32)
    ),
    failure_reason TEXT,
    CHECK (
        (state = 'BUILDING' AND record_count IS NULL AND canonical_checksum IS NULL)
        OR (state = 'VERIFIED' AND record_count IS NOT NULL AND canonical_checksum IS NOT NULL
            AND failure_reason IS NULL)
        OR (state = 'FAILED' AND failure_reason IS NOT NULL)
    ),
    UNIQUE (generation_id, projection_kind, security_domain)
) STRICT;

CREATE TABLE projection_active (
    projection_kind TEXT NOT NULL,
    security_domain BLOB NOT NULL CHECK (
        typeof(security_domain) = 'blob' AND length(security_domain) = 16
    ),
    generation_id BLOB NOT NULL CHECK (
        typeof(generation_id) = 'blob' AND length(generation_id) = 16
    ),
    known_at_accept_seq INTEGER NOT NULL CHECK (known_at_accept_seq >= 0),
    valid_at_unix_ms INTEGER NOT NULL,
    source_outbox_seq INTEGER NOT NULL CHECK (source_outbox_seq >= 0),
    source_ledger_digest BLOB NOT NULL CHECK (
        typeof(source_ledger_digest) = 'blob' AND length(source_ledger_digest) = 32
    ),
    resolver_version TEXT NOT NULL CHECK (length(trim(resolver_version)) > 0),
    policy_registry_version TEXT NOT NULL CHECK (length(trim(policy_registry_version)) > 0),
    policy_registry_hash BLOB NOT NULL CHECK (
        typeof(policy_registry_hash) = 'blob' AND length(policy_registry_hash) = 32
    ),
    activated_at_unix_ms INTEGER NOT NULL CHECK (activated_at_unix_ms >= 0),
    PRIMARY KEY (projection_kind, security_domain),
    FOREIGN KEY (generation_id, projection_kind, security_domain)
        REFERENCES projection_generation(generation_id, projection_kind, security_domain)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TABLE projection_cursor (
    projection_kind TEXT NOT NULL,
    security_domain BLOB NOT NULL CHECK (
        typeof(security_domain) = 'blob' AND length(security_domain) = 16
    ),
    last_outbox_seq INTEGER NOT NULL CHECK (last_outbox_seq >= 0),
    source_ledger_digest BLOB NOT NULL CHECK (
        typeof(source_ledger_digest) = 'blob' AND length(source_ledger_digest) = 32
    ),
    known_at_accept_seq INTEGER NOT NULL CHECK (known_at_accept_seq >= 0),
    valid_at_unix_ms INTEGER NOT NULL,
    resolver_version TEXT NOT NULL CHECK (length(trim(resolver_version)) > 0),
    policy_registry_version TEXT NOT NULL CHECK (length(trim(policy_registry_version)) > 0),
    policy_registry_hash BLOB NOT NULL CHECK (
        typeof(policy_registry_hash) = 'blob' AND length(policy_registry_hash) = 32
    ),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0),
    PRIMARY KEY (projection_kind, security_domain)
) STRICT;

CREATE TABLE projection_graph_edge (
    generation_id BLOB NOT NULL REFERENCES projection_generation(generation_id)
        ON UPDATE RESTRICT ON DELETE CASCADE,
    claim_id BLOB NOT NULL CHECK (typeof(claim_id) = 'blob' AND length(claim_id) = 16),
    source_entity_id BLOB NOT NULL CHECK (
        typeof(source_entity_id) = 'blob' AND length(source_entity_id) = 16
    ),
    predicate_id TEXT NOT NULL CHECK (length(trim(predicate_id)) > 0),
    target_entity_id BLOB NOT NULL CHECK (
        typeof(target_entity_id) = 'blob' AND length(target_entity_id) = 16
    ),
    scope_id BLOB NOT NULL CHECK (typeof(scope_id) = 'blob' AND length(scope_id) = 16),
    security_domain BLOB NOT NULL CHECK (
        typeof(security_domain) = 'blob' AND length(security_domain) = 16
    ),
    authority_class TEXT NOT NULL,
    epistemic_status TEXT NOT NULL,
    authority_policy TEXT NOT NULL CHECK (
        authority_policy IN ('USER_OWNED', 'OFFICIAL_FACT', 'IMPLEMENTATION_OBSERVATION', 'CURATED_RELATION')
    ),
    valid_from_unix_ms INTEGER NOT NULL,
    valid_to_unix_ms INTEGER,
    source_accept_seq INTEGER NOT NULL CHECK (source_accept_seq >= 1),
    stable_tiebreaker BLOB NOT NULL CHECK (
        typeof(stable_tiebreaker) = 'blob' AND length(stable_tiebreaker) = 32
    ),
    PRIMARY KEY (generation_id, claim_id),
    CHECK (valid_to_unix_ms IS NULL OR valid_from_unix_ms < valid_to_unix_ms)
) STRICT;

CREATE TABLE projection_graph_edge_evidence (
    generation_id BLOB NOT NULL,
    claim_id BLOB NOT NULL,
    evidence_ordinal INTEGER NOT NULL CHECK (evidence_ordinal >= 0),
    evidence_id BLOB NOT NULL CHECK (
        typeof(evidence_id) = 'blob' AND length(evidence_id) = 16
    ),
    PRIMARY KEY (generation_id, claim_id, evidence_ordinal),
    UNIQUE (generation_id, claim_id, evidence_id),
    FOREIGN KEY (generation_id, claim_id)
        REFERENCES projection_graph_edge(generation_id, claim_id)
        ON UPDATE RESTRICT ON DELETE CASCADE
) STRICT;

CREATE TABLE projection_search_content (
    content_id INTEGER PRIMARY KEY,
    generation_id BLOB NOT NULL REFERENCES projection_generation(generation_id)
        ON UPDATE RESTRICT ON DELETE CASCADE,
    record_key BLOB NOT NULL CHECK (typeof(record_key) = 'blob' AND length(record_key) = 32),
    claim_id BLOB NOT NULL CHECK (typeof(claim_id) = 'blob' AND length(claim_id) = 16),
    evidence_id BLOB NOT NULL CHECK (typeof(evidence_id) = 'blob' AND length(evidence_id) = 16),
    subject_entity_id BLOB NOT NULL CHECK (
        typeof(subject_entity_id) = 'blob' AND length(subject_entity_id) = 16
    ),
    predicate_id TEXT NOT NULL CHECK (length(trim(predicate_id)) > 0),
    body TEXT NOT NULL CHECK (length(body) > 0),
    artifact_id BLOB NOT NULL CHECK (typeof(artifact_id) = 'blob' AND length(artifact_id) = 16),
    representation_index INTEGER NOT NULL CHECK (representation_index >= 0),
    locator_kind TEXT NOT NULL CHECK (
        locator_kind IN ('TEXT_BYTES', 'PAGE', 'TRANSCRIPT_TIME', 'REPOSITORY_BYTES')
    ),
    locator_payload BLOB NOT NULL CHECK (
        typeof(locator_payload) = 'blob' AND length(locator_payload) > 0
    ),
    security_domain BLOB NOT NULL CHECK (
        typeof(security_domain) = 'blob' AND length(security_domain) = 16
    ),
    authority_class TEXT NOT NULL,
    epistemic_status TEXT NOT NULL,
    authority_policy TEXT NOT NULL CHECK (
        authority_policy IN ('USER_OWNED', 'OFFICIAL_FACT', 'IMPLEMENTATION_OBSERVATION', 'CURATED_RELATION')
    ),
    valid_from_unix_ms INTEGER NOT NULL,
    valid_to_unix_ms INTEGER,
    source_accept_seq INTEGER NOT NULL CHECK (source_accept_seq >= 1),
    stable_tiebreaker BLOB NOT NULL CHECK (
        typeof(stable_tiebreaker) = 'blob' AND length(stable_tiebreaker) = 32
    ),
    UNIQUE (generation_id, record_key),
    UNIQUE (generation_id, content_id),
    CHECK (valid_to_unix_ms IS NULL OR valid_from_unix_ms < valid_to_unix_ms)
) STRICT;

CREATE TABLE projection_exact_symbol (
    generation_id BLOB NOT NULL REFERENCES projection_generation(generation_id)
        ON UPDATE RESTRICT ON DELETE CASCADE,
    symbol TEXT NOT NULL CHECK (length(symbol) > 0),
    content_id INTEGER NOT NULL,
    stable_tiebreaker BLOB NOT NULL CHECK (
        typeof(stable_tiebreaker) = 'blob' AND length(stable_tiebreaker) = 32
    ),
    PRIMARY KEY (generation_id, symbol, content_id),
    FOREIGN KEY (generation_id, content_id)
        REFERENCES projection_search_content(generation_id, content_id)
        ON UPDATE RESTRICT ON DELETE CASCADE
) STRICT;

CREATE VIRTUAL TABLE projection_search_unicode USING fts5(
    body,
    content_id UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE VIRTUAL TABLE projection_search_trigram USING fts5(
    body,
    content_id UNINDEXED,
    tokenize = 'trigram'
);

CREATE INDEX idx_projection_generation_lookup
    ON projection_generation(projection_kind, security_domain, generation_seq);
CREATE INDEX idx_projection_generation_authority
    ON projection_generation(
        projection_kind,
        security_domain,
        known_at_accept_seq,
        valid_at_unix_ms,
        resolver_version,
        policy_registry_version,
        policy_registry_hash,
        source_outbox_seq,
        source_ledger_digest,
        generation_seq
    ) WHERE state = 'VERIFIED';
CREATE INDEX idx_projection_graph_source
    ON projection_graph_edge(generation_id, source_entity_id, stable_tiebreaker);
CREATE INDEX idx_projection_graph_target
    ON projection_graph_edge(generation_id, target_entity_id, stable_tiebreaker);
CREATE INDEX idx_projection_search_generation
    ON projection_search_content(generation_id, stable_tiebreaker);
CREATE INDEX idx_projection_exact_symbol_lookup
    ON projection_exact_symbol(generation_id, symbol, stable_tiebreaker);
