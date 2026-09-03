-- Migration 0015: `P2-U2`'s typed columns for the `REQUIREMENT_SET_PUBLISHED`
-- aggregate -- section 11.2's rule bodies, section 11.1's versioned publication,
-- and the two-reviewer attestation that section 11.2 requires before a
-- model-extracted candidate becomes executable.
--
-- Migration 0004 states the rule this file follows: "The columns below are the
-- whole of the v3 registration frame ... Typed aggregate attributes are
-- deliberately absent. Each aggregate owner adds its own typed columns in a
-- later migration." `P2-U2` owns the `requirement_set` arm, and this is that
-- migration. It adds no event kind, no Proto tag and no CBOR arm: t068 section
-- 3.8 fixes the v3 arm list at eighteen and this file does not touch it.
--
-- It is `0015`. `0014` is `P2-U1`'s. `0006`, `0007`, `0009` and `0012` are in
-- use; `0008`, `0010` and `0011` were left unclaimed by tasks that landed with
-- no migration and stay that way; `0013` is reserved for `P2-L3`. A migration
-- number decides the order and nothing else rests on it: what the admission
-- fingerprint fixes is the resulting object set, read out of `sqlite_schema`
-- sorted by type and name.
--
-- # Three tables, and what each refuses
--
-- ## `requirement_rule`
--
-- One row per rule in one published version. `rule_type` admits exactly the
-- fourteen identifiers section 11.2 gives -- five from its yaml block and nine
-- derived from its prose sentence -- so a fifteenth kind is refused by the
-- database as well as by a total `match` in `academic-requirement`.
--
-- There is **no `body_text` column and no `quoted_source` column**. The
-- sentence a model read lives on `RuleCandidate`, which is not published; a
-- column for it here would be the free-text interpretation section 11.2
-- forbids, sitting one layer below the Rust boundary that refuses it. The rule
-- body is reconstructed from the typed operand tables an audit reads, and what
-- this row carries is the rule's identity, its kind, and the digest of the
-- official snapshot it was read out of.
--
-- ## `requirement_rule_review`
--
-- One row per attestation. The primary key is `(requirement_rule_id,
-- reviewer_entity_id)`, so **one reviewer cannot attest twice to one rule**:
-- two attestations by one person is one review recorded twice, and here that is
-- a constraint violation rather than a duplicate row. `ReviewGate::admit`
-- refuses the same shape in Rust; this is the second layer, and it does not
-- depend on the Rust boundary having been used.
--
-- The reviewer is an entity, never an actor kind, because section 11.2 says
-- *사람이 검토한* -- a person. A deterministic engine and a model run have no
-- entity identity to put here.
--
-- ## `requirement_set_version`
--
-- One row per published version. `supersedes_version` is `UNIQUE`, so **two
-- versions cannot supersede the same predecessor**: a fork in the version chain
-- is what would make section 11.4's *과거 audit은 당시 입력과 rule hash로
-- 재현한다* ambiguous, because a replay would have two successors to walk from.
-- `NULL` is admitted more than once by SQLite's `UNIQUE`, which is correct here:
-- a first version supersedes nothing, and a set may be re-founded.
--
-- `rule_set_hash` is the digest a historical audit replays against, and it is
-- `UNIQUE`: two versions with the same content are the same version, and a
-- version whose content changed would need a different row.
--
-- # Every table is INSERT-only
--
-- Each carries the `guard_<table>_update` / `guard_<table>_delete` pair
-- migration `0004` sets as the terms for every canonical table, and each is
-- listed in `academic_store::authorizer::CANONICAL_TABLES` so the SQLite
-- authorizer is the second layer. An UPDATE here would be the edit to a
-- published rule set that section 11.4 forbids.

CREATE TABLE requirement_set_version (
    requirement_set_id BLOB NOT NULL
        REFERENCES requirement_set(requirement_set_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    version INTEGER NOT NULL CHECK (version >= 1),
    -- The predecessor this version replaces. NULL for a first version.
    -- UNIQUE, so the version chain cannot fork.
    supersedes_version INTEGER
        CHECK (supersedes_version IS NULL OR supersedes_version < version),
    -- The digest section 11.4's historical replay addresses this version by.
    rule_set_hash BLOB NOT NULL UNIQUE
        CHECK (typeof(rule_set_hash) = 'blob' AND length(rule_set_hash) = 32),
    curriculum_version_id BLOB NOT NULL
        REFERENCES curriculum_version(curriculum_version_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    effective_from INTEGER NOT NULL,
    PRIMARY KEY (requirement_set_id, version),
    UNIQUE (requirement_set_id, supersedes_version)
) WITHOUT ROWID, STRICT;

CREATE TABLE requirement_rule (
    requirement_rule_id BLOB PRIMARY KEY
        CHECK (typeof(requirement_rule_id) = 'blob' AND length(requirement_rule_id) = 16),
    requirement_set_id BLOB NOT NULL
        REFERENCES requirement_set(requirement_set_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    version INTEGER NOT NULL CHECK (version >= 1),
    -- Section 11.2's `id:`. The same narrow identifier alphabet
    -- `academic_requirement::dsl::is_identifier` admits, which is what keeps a
    -- sentence out of a column that is compared as text.
    rule_id TEXT NOT NULL
        CHECK (length(rule_id) BETWEEN 1 AND 128 AND rule_id NOT GLOB '*[^A-Za-z0-9._-]*'),
    -- The fourteen section 11.2 rule types, enumerated. Five are the
    -- specification's own yaml; nine are derived from its prose sentence by
    -- upper-casing and replacing each space, hyphen or slash with an
    -- underscore. `docs/contracts/requirement-rule-dsl.md` records both halves.
    rule_type TEXT NOT NULL CHECK (rule_type IN (
        'CREDIT_MINIMUM',
        'ALL_OF',
        'AT_LEAST_N_OF',
        'COUNT_WITH_CONSTRAINTS',
        'GPA_MINIMUM',
        'AREA_DISTRIBUTION',
        'CO_REQUISITE',
        'MUTUALLY_EXCLUSIVE',
        'EQUIVALENCY',
        'MAXIMUM_RECOGNITION',
        'NON_CREDIT_TRAINING',
        'LANGUAGE_OF_INSTRUCTION',
        'THESIS_RESEARCH',
        'EXCEPTION_APPROVAL'
    )),
    -- The official snapshot the rule was read out of. Section 11.3 requires
    -- every proof leaf to name its source.
    source_digest BLOB NOT NULL
        CHECK (typeof(source_digest) = 'blob' AND length(source_digest) = 32),
    FOREIGN KEY (requirement_set_id, version)
        REFERENCES requirement_set_version(requirement_set_id, version)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE (requirement_set_id, version, rule_id)
) STRICT;

CREATE TABLE requirement_rule_review (
    requirement_rule_id BLOB NOT NULL
        REFERENCES requirement_rule(requirement_rule_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    -- A person. There is no actor-kind column, because a model run and a
    -- deterministic engine have no entity identity to put here.
    reviewer_entity_id BLOB NOT NULL
        CHECK (typeof(reviewer_entity_id) = 'blob' AND length(reviewer_entity_id) = 16),
    attested_at INTEGER NOT NULL,
    -- One reviewer, one attestation, per rule. Two rows by one person is what
    -- the key refuses.
    PRIMARY KEY (requirement_rule_id, reviewer_entity_id)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER guard_requirement_set_version_update
BEFORE UPDATE ON requirement_set_version
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_requirement_set_version_delete
BEFORE DELETE ON requirement_set_version
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_requirement_rule_update
BEFORE UPDATE ON requirement_rule
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_requirement_rule_delete
BEFORE DELETE ON requirement_rule
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_requirement_rule_review_update
BEFORE UPDATE ON requirement_rule_review
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_requirement_rule_review_delete
BEFORE DELETE ON requirement_rule_review
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
