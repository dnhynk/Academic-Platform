-- Phase 2 encrypted-profile schema identity (store schema version 2).
--
-- This migration exists only in the `sqlcipher-store` lane. It runs
-- immediately after `0001_phase1_core.sql`, inside the same exclusive
-- creation transaction, before any listener exists, and only against a
-- database that was proven empty. It replaces the Phase 1 identity singleton
-- with the schema-2 one; every canonical table, index, and append-only
-- trigger created by 0001 is carried forward unchanged.
--
-- There is no conversion path from a schema-1 profile. A Phase 1 profile is
-- plaintext, so the encrypted lane cannot open it at all, and the encrypted
-- lane applies this migration only to an empty database. The Phase 1 CHECKs
-- pinned `schema_version = 1`, `data_policy` and `storage_mode` to their
-- plaintext synthetic values; the CHECKs below pin the schema-2 values, so
-- neither singleton can hold the other's row even if a caller tried.
--
-- `production_data_allowed` and `product_network` are deliberately absent.
-- They are the admission verifier's runtime output (t068 sections 3.1 and 6):
-- an encrypted profile with no receipt still serves the synthetic posture, so
-- the posture is not read from this singleton. Freezing either value in a
-- CHECK here would state an admission decision that `P2-K6` has not made.

DROP TABLE schema_meta;

CREATE TABLE schema_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    format_uuid BLOB NOT NULL CHECK (
        typeof(format_uuid) = 'blob'
        AND length(format_uuid) = 16
        AND format_uuid = x'67cb6d3ea27e4b53b1e727d46920e4f9'
    ),
    schema_version INTEGER NOT NULL CHECK (schema_version = 2),
    schema_semver TEXT NOT NULL CHECK (schema_semver = '2.0.0'),
    minimum_reader_protocol_major INTEGER NOT NULL CHECK (minimum_reader_protocol_major = 2),
    minimum_reader_protocol_minor INTEGER NOT NULL CHECK (minimum_reader_protocol_minor = 0),
    minimum_writer_protocol_major INTEGER NOT NULL CHECK (minimum_writer_protocol_major = 2),
    minimum_writer_protocol_minor INTEGER NOT NULL CHECK (minimum_writer_protocol_minor = 0),
    data_policy TEXT NOT NULL CHECK (data_policy = 'REAL_PERSONAL_DATA_PERMITTED'),
    storage_mode TEXT NOT NULL CHECK (storage_mode = 'SQLCIPHER_ENCRYPTED_PROFILE_V2'),
    storage_encryption TEXT NOT NULL CHECK (
        storage_encryption = 'SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000'
    ),
    creating_build_digest BLOB NOT NULL CHECK (
        typeof(creating_build_digest) = 'blob' AND length(creating_build_digest) = 32
    ),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
) STRICT;

CREATE TRIGGER guard_schema_meta_update BEFORE UPDATE ON schema_meta
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_schema_meta_delete BEFORE DELETE ON schema_meta
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
