-- Migration 0006: `P2-G6`'s typed columns for the `CAPTURE_PERMISSION_RECORDED`
-- and `CONSENT_RECORDED` aggregates.
--
-- Migration 0004 states the rule this file follows: "The columns below are the
-- whole of the v3 registration frame ... Typed aggregate attributes are
-- deliberately absent. Each aggregate owner adds its own typed columns in a
-- later migration." `P2-G6` owns both of those arms, and this is that later
-- migration. It adds no event kind, no Proto tag, and no CBOR arm: t068 section
-- 3.8 fixes the v3 arm list at eighteen and this file does not touch it.
--
-- # The section 3.7 key
--
-- Section 3.7 keys `capture_permission` on `(offering_id, permission_seq)`.
-- Migration 0004's closure row is keyed on the aggregate identifier, so that
-- pair is a `UNIQUE` constraint here rather than a second primary key. A
-- correction is a new row at the next `permission_seq`, never an edit: the
-- table is append-only twice over, by the trigger pair below and by the
-- product connection's SQLite authorizer.
--
-- # Why retention is four columns and not one
--
-- The contract this task fixes makes audio retention and transcript retention
-- independent values. An instructor may permit a transcript for the term while
-- refusing to let the recording outlive the lecture, and one `retention_until`
-- column can hold neither of those without silently widening or narrowing the
-- other. Each axis therefore carries its own kind and its own instant, and the
-- `CHECK` pairs them so `PROHIBITED` cannot carry an instant and `UNTIL` cannot
-- be missing one.
--
-- # Why the two sets are child tables
--
-- `allowed_media` and `allowed_processing` are sets, and a set encoded as a
-- delimited string is a set nothing can constrain. Each member is a row with
-- its own closed `CHECK`, so a medium or a processing step outside the fixed
-- vocabulary is refused at INSERT rather than parsed later.
--
-- # `GATE-38-009` and `GATE-38-019`
--
-- Both stay open, and both are open here as the absence of a row rather than as
-- a permissive default. An offering with no `capture_permission_terms` row is
-- `UNKNOWN` because nothing answered for it, and one whose grant listed no
-- medium has no `capture_permission_medium` rows, which matches no request.
-- There is no default row, no template, and no seeded vocabulary of "usual"
-- permissions.
--
-- # What binds one row to a canonical event
--
-- The same two things migration 0005 uses. `capture_permission_id` is a foreign
-- key, so a typed row cannot exist without the accepted
-- `CAPTURE_PERMISSION_RECORDED` event that registered the aggregate; and
-- `guard_capture_permission_terms_authorized` refuses an insert whose
-- `record_digest` is not the `source_digest` that event carries, so a row
-- nobody signed for cannot state a permission.

CREATE TABLE capture_permission_terms (
    capture_permission_id BLOB PRIMARY KEY
        REFERENCES capture_permission(capture_permission_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    offering_id BLOB NOT NULL
        REFERENCES offering(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    permission_seq INTEGER NOT NULL CHECK (permission_seq >= 1),
    status TEXT NOT NULL CHECK (
        status IN ('UNKNOWN', 'PROHIBITED', 'PERMITTED', 'PERMITTED_WITH_CONDITIONS', 'EXPIRED')
    ),
    grant_authority TEXT NOT NULL CHECK (
        grant_authority IN ('INSTRUCTOR', 'INSTITUTION', 'ACCESSIBILITY_ACCOMMODATION')
    ),
    evidence_kind TEXT NOT NULL CHECK (
        evidence_kind IN (
            'SYLLABUS', 'LMS_POLICY', 'CORRESPONDENCE', 'ANNOUNCEMENT',
            'INSTITUTIONAL_RULE', 'ACCESSIBILITY_DETERMINATION'
        )
    ),
    evidence_artifact_id BLOB NOT NULL
        REFERENCES artifact_descriptor(artifact_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    external_processing_allowed INTEGER NOT NULL DEFAULT 0 CHECK (
        external_processing_allowed IN (0, 1)
    ),
    sharing_allowed INTEGER NOT NULL DEFAULT 0 CHECK (sharing_allowed IN (0, 1)),
    audio_retention_kind TEXT NOT NULL CHECK (audio_retention_kind IN ('PROHIBITED', 'UNTIL')),
    audio_retention_until INTEGER,
    transcript_retention_kind TEXT NOT NULL CHECK (
        transcript_retention_kind IN ('PROHIBITED', 'UNTIL')
    ),
    transcript_retention_until INTEGER,
    conditions_hash BLOB NOT NULL CHECK (
        typeof(conditions_hash) = 'blob' AND length(conditions_hash) = 32
    ),
    verified_at INTEGER NOT NULL,
    verification_source_digest BLOB NOT NULL CHECK (
        typeof(verification_source_digest) = 'blob' AND length(verification_source_digest) = 32
    ),
    scope_term_year INTEGER NOT NULL CHECK (scope_term_year BETWEEN 1900 AND 2999),
    scope_term_season TEXT NOT NULL CHECK (scope_term_season IN ('1', 'S', '2', 'W')),
    scope_grain TEXT NOT NULL CHECK (scope_grain IN ('WHOLE_TERM', 'SINGLE_LECTURE')),
    scope_lecture_session_id BLOB
        REFERENCES lecture_session(lecture_session_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    scope_valid_from INTEGER NOT NULL,
    scope_valid_to INTEGER NOT NULL,
    record_digest BLOB NOT NULL CHECK (
        typeof(record_digest) = 'blob' AND length(record_digest) = 32
    ),
    UNIQUE (offering_id, permission_seq),
    CHECK (scope_valid_to > scope_valid_from),
    CHECK (
        (audio_retention_kind = 'UNTIL' AND audio_retention_until IS NOT NULL)
        OR (audio_retention_kind = 'PROHIBITED' AND audio_retention_until IS NULL)
    ),
    CHECK (
        (transcript_retention_kind = 'UNTIL' AND transcript_retention_until IS NOT NULL)
        OR (transcript_retention_kind = 'PROHIBITED' AND transcript_retention_until IS NULL)
    ),
    CHECK (
        (scope_grain = 'SINGLE_LECTURE' AND scope_lecture_session_id IS NOT NULL)
        OR (scope_grain = 'WHOLE_TERM' AND scope_lecture_session_id IS NULL)
    )
) STRICT;

CREATE TABLE capture_permission_medium (
    capture_permission_id BLOB NOT NULL
        REFERENCES capture_permission_terms(capture_permission_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    medium TEXT NOT NULL CHECK (
        medium IN ('AUDIO', 'PHOTO_OF_BOARD', 'SCREEN_CAPTURE', 'VIDEO')
    ),
    PRIMARY KEY (capture_permission_id, medium)
) STRICT;

CREATE TABLE capture_permission_processing (
    capture_permission_id BLOB NOT NULL
        REFERENCES capture_permission_terms(capture_permission_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    processing TEXT NOT NULL CHECK (
        processing IN ('LOCAL_STT', 'LOCAL_OCR', 'EXTERNAL_STT', 'EXTERNAL_SUMMARISATION')
    ),
    PRIMARY KEY (capture_permission_id, processing)
) STRICT;

-- One row per dimension the user answered. A dimension with no row is
-- unanswered, which is a different thing from one recorded as not applicable:
-- the `CHECK` below requires exactly one of the two answers, so "nobody looked"
-- has no spelling here at all and is the absence of the row.
CREATE TABLE capture_permission_checklist (
    capture_permission_id BLOB NOT NULL
        REFERENCES capture_permission_terms(capture_permission_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    dimension TEXT NOT NULL CHECK (
        dimension IN (
            'SYLLABUS_OR_LMS_POLICY', 'STUDENT_SPEECH', 'FILMING_SCOPE',
            'ACCESSIBILITY_PROCEDURE', 'COPYRIGHT', 'PRIVACY', 'INSTITUTIONAL_RULES'
        )
    ),
    evidence_artifact_id BLOB
        REFERENCES artifact_descriptor(artifact_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    not_applicable_reason TEXT CHECK (
        not_applicable_reason IS NULL
        OR not_applicable_reason IN (
            'NO_STUDENT_PARTICIPATION_IS_CAPTURED', 'NO_VISUAL_CAPTURE_REQUESTED',
            'NO_ACCOMMODATION_IN_EFFECT', 'MATERIAL_IS_THE_USERS_OWN',
            'NO_THIRD_PARTY_PERSONAL_DATA', 'INSTITUTION_PUBLISHES_NO_APPLICABLE_RULE'
        )
    ),
    PRIMARY KEY (capture_permission_id, dimension),
    CHECK (
        (evidence_artifact_id IS NOT NULL AND not_applicable_reason IS NULL)
        OR (evidence_artifact_id IS NULL AND not_applicable_reason IS NOT NULL)
    )
) STRICT;

-- The general consent ledger's typed columns. `event_kind` is the closed
-- vocabulary `academic-consent` records with, and the two evidence arms are on
-- it deliberately: filing evidence is a recorded act, and it is a different act
-- from a written authority granting.
CREATE TABLE consent_record (
    consent_id BLOB PRIMARY KEY
        REFERENCES consent(consent_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    event_kind TEXT NOT NULL CHECK (
        event_kind IN (
            'EVIDENCE_RECORDED', 'ATTESTATION_RECORDED', 'PERMISSION_GRANTED',
            'PERMISSION_PROHIBITED', 'EXTERNAL_REVIEW_OPENED', 'RECHECK_QUEUED',
            'CAPTURE_CAPABILITY_MINTED', 'CAPTURE_CAPABILITY_DENIED',
            'EXPIRY_PREVIEWED', 'EXPIRY_APPLIED'
        )
    ),
    offering_id BLOB
        REFERENCES offering(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    term_year INTEGER CHECK (term_year IS NULL OR term_year BETWEEN 1900 AND 2999),
    term_season TEXT CHECK (term_season IS NULL OR term_season IN ('1', 'S', '2', 'W')),
    subject_digest BLOB CHECK (
        subject_digest IS NULL
        OR (typeof(subject_digest) = 'blob' AND length(subject_digest) = 32)
    ),
    status TEXT NOT NULL CHECK (
        status IN ('UNKNOWN', 'PROHIBITED', 'PERMITTED', 'PERMITTED_WITH_CONDITIONS', 'EXPIRED')
    ),
    recorded_at INTEGER NOT NULL,
    record_digest BLOB NOT NULL CHECK (
        typeof(record_digest) = 'blob' AND length(record_digest) = 32
    ),
    CHECK ((term_year IS NULL) = (term_season IS NULL))
) STRICT;

CREATE TRIGGER guard_capture_permission_terms_authorized
BEFORE INSERT ON capture_permission_terms
BEGIN
    SELECT RAISE(
        ABORT,
        'capture permission terms are not the ones their event authorized'
    )
    WHERE NOT EXISTS (
        SELECT 1 FROM capture_permission
        WHERE capture_permission.capture_permission_id = NEW.capture_permission_id
          AND capture_permission.offering_id = NEW.offering_id
          AND capture_permission.source_digest IS NOT NULL
          AND capture_permission.source_digest = NEW.record_digest
    );
END;

CREATE TRIGGER guard_consent_record_authorized
BEFORE INSERT ON consent_record
BEGIN
    SELECT RAISE(ABORT, 'consent record is not the one its event authorized')
    WHERE NOT EXISTS (
        SELECT 1 FROM consent
        WHERE consent.consent_id = NEW.consent_id
          AND consent.source_digest IS NOT NULL
          AND consent.source_digest = NEW.record_digest
    );
END;

CREATE TRIGGER guard_capture_permission_terms_update
BEFORE UPDATE ON capture_permission_terms
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_capture_permission_terms_delete
BEFORE DELETE ON capture_permission_terms
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;

CREATE TRIGGER guard_capture_permission_medium_update
BEFORE UPDATE ON capture_permission_medium
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_capture_permission_medium_delete
BEFORE DELETE ON capture_permission_medium
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;

CREATE TRIGGER guard_capture_permission_processing_update
BEFORE UPDATE ON capture_permission_processing
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_capture_permission_processing_delete
BEFORE DELETE ON capture_permission_processing
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;

CREATE TRIGGER guard_capture_permission_checklist_update
BEFORE UPDATE ON capture_permission_checklist
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_capture_permission_checklist_delete
BEFORE DELETE ON capture_permission_checklist
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;

CREATE TRIGGER guard_consent_record_update
BEFORE UPDATE ON consent_record
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_consent_record_delete
BEFORE DELETE ON consent_record
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
