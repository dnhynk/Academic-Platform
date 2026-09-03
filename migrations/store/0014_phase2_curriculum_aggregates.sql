-- Migration 0014: `P2-U1`'s typed columns for the `CURRICULUM_VERSION_PUBLISHED`,
-- `COURSE_REVISION_PUBLISHED` and `OFFERING_OBSERVED` aggregates -- section
-- 8.2's three blocks, the durable course identity they hang from, and section
-- 11.4's four independent relations.
--
-- Migration 0004 states the rule this file follows: "The columns below are the
-- whole of the v3 registration frame ... Typed aggregate attributes are
-- deliberately absent. Each aggregate owner adds its own typed columns in a
-- later migration." `P2-U1` owns those three arms, and this is that migration.
-- It adds no event kind, no Proto tag and no CBOR arm: t068 section 3.8 fixes
-- the v3 arm list at eighteen and this file does not touch it.
--
-- It is `0014`. `0006`, `0007`, `0009` and `0012` are in use; `0008`, `0010`
-- and `0011` were left unclaimed by tasks that landed with no migration and
-- stay that way; `0013` is reserved for `P2-L3`, which was in flight on the
-- same `main` when this branch was written. A migration number decides the
-- order and nothing else rests on it: what the admission fingerprint fixes is
-- the resulting object set, read out of `sqlite_schema` sorted by type and
-- name.
--
-- # `course` has no registration arm, and that is deliberate
--
-- Section 8.2's `Course` is durable identity: it outlives every
-- `CourseRevision` that names it. The eighteen v3 arms register no course, so
-- `course_id` is a parent reference with no arm of its own -- exactly the
-- position `repository_id` holds in migration 0004's `snapshot` table, for the
-- same reason. A course row is therefore authorized through the curriculum
-- version whose publication introduced it: `guard_course_authorized` requires
-- `registered_event_id` to be the event that registered
-- `introduced_by_version_id`, so a course cannot appear beside an unsigned
-- publication. Later revisions under later versions reference the same
-- `course_id`; the introducing version records where the identity entered the
-- record, not how long it lasts.
--
-- # The four relations are four tables
--
-- Section 11.4: *동일·대체·폐지·경과조치는 독립 rule이며 양방향 동일성으로
-- 단순화하지 않는다*. One table with a `relation_kind` discriminator would have
-- given every relation the same columns, and a column that exists is a column
-- something can be inferred from. Four tables give each relation only the
-- columns its own question needs:
--
--   * `course_identity_decision` carries a verdict and the user decision it was
--     recorded against. There is no row shape that records a verdict without
--     naming the decision, which is section 8.2's rule that course-code reuse
--     is an explicit decision rather than an inference.
--
--   * `course_equivalence` carries an ordered pair and nothing else. It is
--     directional because the primary key is ordered; the reverse direction is
--     a second row, not a property of this one.
--
--   * `course_replacement` carries the retired course and the one named in its
--     place. It carries no verdict, so a replacement cannot say anything about
--     identity, and there is no join here that would let it.
--
--   * `course_retirement` carries one course and an interval. **There is no
--     replacement column.** Section 8.1's *IT창업개론 폐지·대체 미지정* is not a
--     nullable column here; it is the only shape this table has.
--
-- `UNKNOWN` appears in no `CHECK` list on any of the four. Every `UNKNOWN` this
-- task defines is the absence of a row, and a row recording "no record exists"
-- would be a record saying nothing was recorded.
--
-- # The two prerequisite lists are two tables
--
-- `GATE-38-018` asks how a course's official prerequisite differs from the
-- instructor's recommended prior knowledge. That difference needs a reviewed
-- source. Until one exists the two are recorded separately and nothing joins
-- them: `course_revision_official_prerequisite` and
-- `course_revision_recommended_prerequisite` share no key and no view.
--
-- # What the guards here hold that the Rust half cannot
--
-- `academic-curriculum` refuses the same shapes in memory, and it has no
-- `academic-store` edge at all, so nothing in it can write a row. These
-- triggers are the layer that runs against rows a process this repository did
-- not write could have inserted, on the terms migration 0004 sets for every
-- canonical table: the trigger pair is the first enforcement layer and the
-- product connection's SQLite authorizer is the second.

CREATE TABLE curriculum_version_detail (
    curriculum_version_id BLOB PRIMARY KEY
        REFERENCES curriculum_version(curriculum_version_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    institution_path TEXT NOT NULL CHECK (length(institution_path) > 0),
    admission_year_from TEXT NOT NULL CHECK (length(admission_year_from) > 0),
    admission_year_to TEXT NOT NULL CHECK (length(admission_year_to) > 0),
    publication_status TEXT NOT NULL CHECK (publication_status IN (
        'OFFICIAL_CONFIRMED',
        'UNKNOWN'
    )),
    -- Section 8.2's `supersedes`. Which version this one follows, and nothing
    -- about which cohorts move: that is
    -- `curriculum_transition_arrangement`'s row, and its absence is UNKNOWN.
    supersedes_curriculum_version_id BLOB
        REFERENCES curriculum_version(curriculum_version_id) ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TABLE curriculum_transition_arrangement (
    curriculum_version_id BLOB NOT NULL
        REFERENCES curriculum_version_detail(curriculum_version_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    admission_cohort TEXT NOT NULL CHECK (length(admission_cohort) > 0),
    -- No `UNKNOWN`: a cohort with no arrangement has no row here.
    disposition TEXT NOT NULL CHECK (disposition IN ('MOVES', 'STAYS')),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from),
    PRIMARY KEY (curriculum_version_id, admission_cohort, valid_from)
) WITHOUT ROWID, STRICT;

CREATE TABLE course (
    course_id BLOB PRIMARY KEY CHECK (typeof(course_id) = 'blob' AND length(course_id) = 16),
    course_code TEXT NOT NULL CHECK (length(course_code) > 0 AND length(course_code) <= 128),
    canonical_identity BLOB NOT NULL CHECK (
        typeof(canonical_identity) = 'blob' AND length(canonical_identity) = 16
    ),
    introduced_by_version_id BLOB NOT NULL
        REFERENCES curriculum_version(curriculum_version_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    registered_event_id BLOB NOT NULL
        REFERENCES ledger_event(event_id) ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TABLE course_revision_detail (
    course_revision_id BLOB PRIMARY KEY
        REFERENCES course_revision(course_revision_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    course_id BLOB NOT NULL REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    course_code TEXT NOT NULL CHECK (length(course_code) > 0 AND length(course_code) <= 128),
    title TEXT NOT NULL CHECK (length(title) > 0 AND length(title) <= 128),
    credits INTEGER NOT NULL CHECK (credits >= 0 AND credits <= 30),
    curriculum_category TEXT NOT NULL CHECK (curriculum_category IN (
        'UNKNOWN',
        'MAJOR_REQUIRED',
        'MAJOR_ELECTIVE',
        'GENERAL_STUDIES',
        'GENERAL_ELECTIVE',
        'NON_CREDIT'
    ))
) STRICT;

CREATE TABLE course_revision_official_prerequisite (
    course_revision_id BLOB NOT NULL
        REFERENCES course_revision_detail(course_revision_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    prerequisite_course_id BLOB NOT NULL
        REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    PRIMARY KEY (course_revision_id, ordinal)
) WITHOUT ROWID, STRICT;

CREATE TABLE course_revision_recommended_prerequisite (
    course_revision_id BLOB NOT NULL
        REFERENCES course_revision_detail(course_revision_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    prerequisite_course_id BLOB NOT NULL
        REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    PRIMARY KEY (course_revision_id, ordinal)
) WITHOUT ROWID, STRICT;

CREATE TABLE course_revision_designed_coverage (
    course_revision_id BLOB NOT NULL
        REFERENCES course_revision_detail(course_revision_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    coverage_kind TEXT NOT NULL CHECK (coverage_kind IN ('CONCEPT', 'COMPETENCY')),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    entity_id BLOB NOT NULL CHECK (typeof(entity_id) = 'blob' AND length(entity_id) = 16),
    PRIMARY KEY (course_revision_id, coverage_kind, ordinal)
) WITHOUT ROWID, STRICT;

CREATE TABLE offering_detail (
    offering_id BLOB PRIMARY KEY
        REFERENCES offering(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    term TEXT NOT NULL CHECK (length(term) > 0 AND length(term) <= 128),
    section TEXT NOT NULL CHECK (length(section) > 0 AND length(section) <= 128),
    capacity INTEGER CHECK (capacity IS NULL OR (capacity >= 0 AND capacity <= 65535)),
    grading_mode TEXT NOT NULL CHECK (grading_mode IN (
        'UNKNOWN',
        'LETTER',
        'SATISFACTORY_UNSATISFACTORY'
    )),
    syllabus_artifact_id BLOB
        REFERENCES artifact_descriptor(artifact_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    official_status TEXT NOT NULL CHECK (official_status IN (
        'CONFIRMED',
        'HISTORICALLY_LIKELY',
        'UNCERTAIN',
        'CANCELLED'
    )),
    observed_at INTEGER NOT NULL
) STRICT;

CREATE TABLE offering_instructor (
    offering_id BLOB NOT NULL
        REFERENCES offering_detail(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    instructor_name TEXT NOT NULL CHECK (
        length(instructor_name) > 0 AND length(instructor_name) <= 128
    ),
    PRIMARY KEY (offering_id, ordinal)
) WITHOUT ROWID, STRICT;

CREATE TABLE offering_meeting (
    offering_id BLOB NOT NULL
        REFERENCES offering_detail(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    weekday TEXT NOT NULL CHECK (weekday IN (
        'MONDAY',
        'TUESDAY',
        'WEDNESDAY',
        'THURSDAY',
        'FRIDAY',
        'SATURDAY',
        'SUNDAY'
    )),
    from_minute INTEGER NOT NULL CHECK (from_minute >= 0 AND from_minute < 1440),
    to_minute INTEGER NOT NULL CHECK (to_minute > 0 AND to_minute <= 1440),
    PRIMARY KEY (offering_id, ordinal)
) WITHOUT ROWID, STRICT;

-- Section 8.2's four reference lists. Identifiers only: an offering references
-- a lecture, and section 9 keeps that lecture's per-session utterance out of
-- this aggregate. There is no text column on this table and no other table
-- here carries one for an offering.
CREATE TABLE offering_reference (
    offering_id BLOB NOT NULL
        REFERENCES offering_detail(offering_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    reference_kind TEXT NOT NULL CHECK (reference_kind IN (
        'MATERIAL',
        'LECTURE',
        'ASSESSMENT',
        'REVIEW'
    )),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    entity_id BLOB NOT NULL CHECK (typeof(entity_id) = 'blob' AND length(entity_id) = 16),
    PRIMARY KEY (offering_id, reference_kind, ordinal)
) WITHOUT ROWID, STRICT;

CREATE TABLE course_identity_decision (
    earlier_course_id BLOB NOT NULL
        REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    later_course_id BLOB NOT NULL
        REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    -- No `UNKNOWN`: an undecided pair has no row.
    verdict TEXT NOT NULL CHECK (verdict IN ('SAME', 'DISTINCT')),
    decision_id BLOB NOT NULL CHECK (typeof(decision_id) = 'blob' AND length(decision_id) = 16),
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from),
    PRIMARY KEY (earlier_course_id, later_course_id, valid_from)
) WITHOUT ROWID, STRICT;

CREATE TABLE course_equivalence (
    source_course_id BLOB NOT NULL
        REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    target_course_id BLOB NOT NULL
        REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from),
    PRIMARY KEY (source_course_id, target_course_id, valid_from)
) WITHOUT ROWID, STRICT;

CREATE TABLE course_replacement (
    retired_course_id BLOB NOT NULL
        REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    replacement_course_id BLOB NOT NULL
        REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from),
    PRIMARY KEY (retired_course_id, replacement_course_id, valid_from)
) WITHOUT ROWID, STRICT;

-- One course and an interval. No replacement column: section 8.1's
-- *폐지·대체 미지정* is the only shape a retirement has here, and a replacement
-- is `course_replacement`'s own row.
CREATE TABLE course_retirement (
    course_id BLOB NOT NULL REFERENCES course(course_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    valid_from INTEGER NOT NULL,
    valid_to INTEGER CHECK (valid_to IS NULL OR valid_to > valid_from),
    PRIMARY KEY (course_id, valid_from)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER guard_curriculum_version_detail_authorized
BEFORE INSERT ON curriculum_version_detail
BEGIN
    SELECT RAISE(ABORT, 'curriculum version detail has no registered version')
    WHERE NOT EXISTS (
        SELECT 1 FROM curriculum_version
        WHERE curriculum_version.curriculum_version_id = NEW.curriculum_version_id
    );
    SELECT RAISE(ABORT, 'a curriculum version supersedes itself')
    WHERE NEW.supersedes_curriculum_version_id = NEW.curriculum_version_id;
END;

CREATE TRIGGER guard_course_authorized
BEFORE INSERT ON course
BEGIN
    SELECT RAISE(ABORT, 'course is not authorized by its introducing version')
    WHERE NOT EXISTS (
        SELECT 1 FROM curriculum_version
        WHERE curriculum_version.curriculum_version_id = NEW.introduced_by_version_id
          AND curriculum_version.registered_event_id = NEW.registered_event_id
    );
END;

CREATE TRIGGER guard_course_revision_detail_authorized
BEFORE INSERT ON course_revision_detail
BEGIN
    SELECT RAISE(ABORT, 'course revision detail has no registered revision')
    WHERE NOT EXISTS (
        SELECT 1 FROM course_revision
        WHERE course_revision.course_revision_id = NEW.course_revision_id
    );
END;

CREATE TRIGGER guard_offering_detail_authorized
BEFORE INSERT ON offering_detail
BEGIN
    SELECT RAISE(ABORT, 'offering detail has no registered offering')
    WHERE NOT EXISTS (
        SELECT 1 FROM offering WHERE offering.offering_id = NEW.offering_id
    );
END;

CREATE TRIGGER guard_offering_meeting_range
BEFORE INSERT ON offering_meeting
BEGIN
    SELECT RAISE(ABORT, 'a meeting is a half-open minute range inside one day')
    WHERE NEW.to_minute <= NEW.from_minute;
END;

CREATE TRIGGER guard_course_identity_decision_pair
BEFORE INSERT ON course_identity_decision
BEGIN
    SELECT RAISE(ABORT, 'an identity decision names one course on both ends')
    WHERE NEW.earlier_course_id = NEW.later_course_id;
END;

CREATE TRIGGER guard_course_equivalence_pair
BEFORE INSERT ON course_equivalence
BEGIN
    SELECT RAISE(ABORT, 'an equivalence names one course on both ends')
    WHERE NEW.source_course_id = NEW.target_course_id;
END;

CREATE TRIGGER guard_course_replacement_pair
BEFORE INSERT ON course_replacement
BEGIN
    SELECT RAISE(ABORT, 'a replacement names one course on both ends')
    WHERE NEW.retired_course_id = NEW.replacement_course_id;
END;

CREATE TRIGGER guard_curriculum_version_detail_update
BEFORE UPDATE ON curriculum_version_detail
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_curriculum_version_detail_delete
BEFORE DELETE ON curriculum_version_detail
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_curriculum_transition_arrangement_update
BEFORE UPDATE ON curriculum_transition_arrangement
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_curriculum_transition_arrangement_delete
BEFORE DELETE ON curriculum_transition_arrangement
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_update
BEFORE UPDATE ON course
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_delete
BEFORE DELETE ON course
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_detail_update
BEFORE UPDATE ON course_revision_detail
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_detail_delete
BEFORE DELETE ON course_revision_detail
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_official_prerequisite_update
BEFORE UPDATE ON course_revision_official_prerequisite
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_official_prerequisite_delete
BEFORE DELETE ON course_revision_official_prerequisite
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_recommended_prerequisite_update
BEFORE UPDATE ON course_revision_recommended_prerequisite
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_recommended_prerequisite_delete
BEFORE DELETE ON course_revision_recommended_prerequisite
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_designed_coverage_update
BEFORE UPDATE ON course_revision_designed_coverage
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_revision_designed_coverage_delete
BEFORE DELETE ON course_revision_designed_coverage
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_detail_update
BEFORE UPDATE ON offering_detail
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_detail_delete
BEFORE DELETE ON offering_detail
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_instructor_update
BEFORE UPDATE ON offering_instructor
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_instructor_delete
BEFORE DELETE ON offering_instructor
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_meeting_update
BEFORE UPDATE ON offering_meeting
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_meeting_delete
BEFORE DELETE ON offering_meeting
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_reference_update
BEFORE UPDATE ON offering_reference
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_offering_reference_delete
BEFORE DELETE ON offering_reference
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_identity_decision_update
BEFORE UPDATE ON course_identity_decision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_identity_decision_delete
BEFORE DELETE ON course_identity_decision
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_equivalence_update
BEFORE UPDATE ON course_equivalence
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_equivalence_delete
BEFORE DELETE ON course_equivalence
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_replacement_update
BEFORE UPDATE ON course_replacement
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_replacement_delete
BEFORE DELETE ON course_replacement
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_retirement_update
BEFORE UPDATE ON course_retirement
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_course_retirement_delete
BEFORE DELETE ON course_retirement
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
