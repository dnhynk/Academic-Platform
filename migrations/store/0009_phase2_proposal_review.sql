-- Migration 0009: `P2-M2`'s typed columns for the `PROPOSAL_DISPOSED`
-- aggregate -- the risk tier a proposal was classified into, the append-only
-- history of what a user did with it, and the versioned batching configuration
-- the review queue banded it under.
--
-- It is `0009` rather than `0008`. This branch was written while `P2-L1` was in
-- flight on the same `main` and might have taken `0008`; it landed without a
-- migration, so `0008` is simply unclaimed. The number is not repaired to close
-- the gap, because a migration number decides the order and nothing else rests
-- on it: what the admission fingerprint fixes is the resulting object *set*,
-- read out of `sqlite_schema` sorted by type and name, and renumbering a file
-- that has been applied anywhere would be the more expensive kind of change.
--
-- Migration 0004 states the rule this file follows: "The columns below are the
-- whole of the v3 registration frame ... Typed aggregate attributes are
-- deliberately absent. Each aggregate owner adds its own typed columns in a
-- later migration." `P2-M2` owns `PROPOSAL_DISPOSED`, and this is that later
-- migration. It adds no event kind, no Proto tag, and no CBOR arm: t068
-- section 3.8 fixes the v3 arm list at eighteen and this file does not touch
-- it.
--
-- # The vocabularies here are not new ones
--
-- `disposition` holds `CONFIRM`, `REJECT` and `REPLACE`, which are the three
-- arms of `academic-domain`'s `DecisionAction` as its own serde attribute
-- spells them. Section 3 of the authoritative spec names exactly three things a
-- user does with an AI proposal -- approve, modify, reject -- and ADR-003 froze
-- those three. There is no fourth and this file does not invent one.
--
-- A proposal nobody has decided on has no row in
-- `proposal_disposition_record`. Pending is the absence of a decision, not a
-- decision that means "not yet": a fourth token would take a place in ADR-003's
-- authority computation, where "undecided" would read as "the user judged".
--
-- `risk_tier` holds the four rows of section 27.4 under the execution plan's
-- spellings. The spec states those rows in prose and names no identifier, so
-- the tokens are the plan's and section 27.4 is the authority for what each one
-- means.
--
-- # What each guard here does that the Rust half does not
--
-- `academic-proposal` refuses the same shapes in memory. These triggers are the
-- second layer, on the terms migration 0004 sets for every canonical table: the
-- trigger pair is the first enforcement layer and the product connection's
-- SQLite authorizer is the second. What is new here is that the guards run
-- against rows a process this repository did not write could have inserted.
--
--   * `guard_proposal_review_authorized` binds a review row to the accepted
--     `PROPOSAL_DISPOSED` event, the way `guard_model_run_provenance_authorized`
--     binds a provenance row to its own.
--   * `guard_proposal_disposition_actor` refuses a disposition row that is not
--     a user's. Section 27.4's fourth row is user-only, and this is the half of
--     it that survives a writer outside the Rust boundary.
--   * `guard_proposal_high_approval_is_explicit` requires a `HIGH_APPROVAL`
--     confirmation to carry the explicit-approval flag, and refuses that flag
--     on every other tier.
--   * `guard_proposal_disposition_supersession` requires an undo to name an
--     earlier record of the same proposal that nothing has superseded yet.
--   * `guard_proposal_outcome_matches_tier` is the tier-to-workflow mapping:
--     `LOW_AUTOSAVE` settles with `AI_INFERRED` and no disposition, and every
--     other tier settles with `USER_CONFIRMED` and a `CONFIRM` record that is
--     still open.
--
-- # Rejection is retained
--
-- Nothing removes a row. A rejection is a `REJECT` record beside the review row
-- it addresses; the review row stays, the record stays, and a later undo is a
-- further record naming it. That is ADR-003's rule and this file adds no second
-- mechanism for it.

CREATE TABLE proposal_batching_policy (
    thresholds_version INTEGER PRIMARY KEY CHECK (thresholds_version >= 1),
    policy_digest BLOB NOT NULL UNIQUE CHECK (
        typeof(policy_digest) = 'blob' AND length(policy_digest) = 32
    ),
    adopted_at INTEGER NOT NULL CHECK (adopted_at >= 0)
) STRICT;

CREATE TABLE proposal_batching_cut (
    thresholds_version INTEGER NOT NULL
        REFERENCES proposal_batching_policy(thresholds_version)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    axis TEXT NOT NULL CHECK (axis IN ('CONFIDENCE', 'IMPACT')),
    cut_ordinal INTEGER NOT NULL CHECK (cut_ordinal >= 0),
    cut_permille INTEGER NOT NULL CHECK (cut_permille BETWEEN 1 AND 1000),
    PRIMARY KEY (thresholds_version, axis, cut_ordinal),
    UNIQUE (thresholds_version, axis, cut_permille)
) WITHOUT ROWID, STRICT;

CREATE TABLE proposal_review (
    proposal_id BLOB PRIMARY KEY
        REFERENCES proposal_disposition(proposal_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    risk_tier TEXT NOT NULL CHECK (
        risk_tier IN ('LOW_AUTOSAVE', 'MEDIUM_REVIEW', 'HIGH_APPROVAL', 'NON_DELEGABLE')
    ),
    confidence_permille INTEGER NOT NULL CHECK (confidence_permille BETWEEN 0 AND 1000),
    impact_permille INTEGER NOT NULL CHECK (impact_permille BETWEEN 0 AND 1000),
    subject_digest BLOB NOT NULL CHECK (
        typeof(subject_digest) = 'blob' AND length(subject_digest) = 32
    ),
    thresholds_version INTEGER NOT NULL
        REFERENCES proposal_batching_policy(thresholds_version)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    record_digest BLOB NOT NULL CHECK (
        typeof(record_digest) = 'blob' AND length(record_digest) = 32
    )
) STRICT;

CREATE TABLE proposal_disposition_record (
    proposal_id BLOB NOT NULL
        REFERENCES proposal_review(proposal_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    disposition_seq INTEGER NOT NULL CHECK (disposition_seq >= 1),
    disposition TEXT NOT NULL CHECK (disposition IN ('CONFIRM', 'REJECT', 'REPLACE')),
    replacement_claim_id BLOB CHECK (
        (disposition = 'REPLACE'
            AND typeof(replacement_claim_id) = 'blob'
            AND length(replacement_claim_id) = 16)
        OR (disposition <> 'REPLACE' AND replacement_claim_id IS NULL)
    ),
    actor_class TEXT NOT NULL CHECK (actor_class = 'USER'),
    user_id BLOB NOT NULL CHECK (typeof(user_id) = 'blob' AND length(user_id) = 16),
    explicit_approval INTEGER NOT NULL CHECK (explicit_approval IN (0, 1)),
    decided_at INTEGER NOT NULL CHECK (decided_at >= 0),
    supersedes_seq INTEGER CHECK (
        supersedes_seq IS NULL OR (supersedes_seq >= 1 AND supersedes_seq < disposition_seq)
    ),
    record_digest BLOB NOT NULL CHECK (
        typeof(record_digest) = 'blob' AND length(record_digest) = 32
    ),
    PRIMARY KEY (proposal_id, disposition_seq),
    UNIQUE (proposal_id, supersedes_seq)
) WITHOUT ROWID, STRICT;

CREATE TABLE proposal_outcome (
    proposal_id BLOB PRIMARY KEY
        REFERENCES proposal_review(proposal_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    epistemic_status TEXT NOT NULL CHECK (
        epistemic_status IN ('AI_INFERRED', 'USER_CONFIRMED')
    ),
    disposition_seq INTEGER,
    settled_at INTEGER NOT NULL CHECK (settled_at >= 0),
    FOREIGN KEY (proposal_id, disposition_seq)
        REFERENCES proposal_disposition_record(proposal_id, disposition_seq)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER guard_proposal_review_authorized
BEFORE INSERT ON proposal_review
BEGIN
    SELECT RAISE(ABORT, 'proposal review is not the record its event authorized')
    WHERE NOT EXISTS (
        SELECT 1 FROM proposal_disposition
        WHERE proposal_disposition.proposal_id = NEW.proposal_id
          AND proposal_disposition.source_digest IS NOT NULL
          AND proposal_disposition.source_digest = NEW.record_digest
    );
END;

CREATE TRIGGER guard_proposal_disposition_actor
BEFORE INSERT ON proposal_disposition_record
BEGIN
    SELECT RAISE(ABORT, 'a proposal disposition is a user action')
    WHERE NEW.actor_class <> 'USER';
END;

CREATE TRIGGER guard_proposal_low_risk_is_not_disposed
BEFORE INSERT ON proposal_disposition_record
BEGIN
    SELECT RAISE(ABORT, 'a low-risk autosave records no user decision')
    WHERE EXISTS (
        SELECT 1 FROM proposal_review
        WHERE proposal_review.proposal_id = NEW.proposal_id
          AND proposal_review.risk_tier = 'LOW_AUTOSAVE'
    );
END;

CREATE TRIGGER guard_proposal_high_approval_is_explicit
BEFORE INSERT ON proposal_disposition_record
BEGIN
    SELECT RAISE(ABORT, 'a high-approval confirmation needs an explicit approval')
    WHERE NEW.disposition = 'CONFIRM'
      AND NEW.supersedes_seq IS NULL
      AND NEW.explicit_approval <> (
        SELECT CASE WHEN proposal_review.risk_tier = 'HIGH_APPROVAL' THEN 1 ELSE 0 END
        FROM proposal_review
        WHERE proposal_review.proposal_id = NEW.proposal_id
      );
END;

CREATE TRIGGER guard_proposal_disposition_supersession
BEFORE INSERT ON proposal_disposition_record
BEGIN
    SELECT RAISE(ABORT, 'an undo names an open earlier record of the same proposal')
    WHERE NEW.supersedes_seq IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM proposal_disposition_record AS prior
        WHERE prior.proposal_id = NEW.proposal_id
          AND prior.disposition_seq = NEW.supersedes_seq
          AND prior.supersedes_seq IS NULL
      );
END;

CREATE TRIGGER guard_proposal_outcome_matches_tier
BEFORE INSERT ON proposal_outcome
BEGIN
    SELECT RAISE(ABORT, 'a low-risk autosave settles as AI_INFERRED with no disposition')
    WHERE EXISTS (
        SELECT 1 FROM proposal_review
        WHERE proposal_review.proposal_id = NEW.proposal_id
          AND proposal_review.risk_tier = 'LOW_AUTOSAVE'
    )
      AND (NEW.epistemic_status <> 'AI_INFERRED' OR NEW.disposition_seq IS NOT NULL);

    SELECT RAISE(ABORT, 'a reviewed proposal settles as USER_CONFIRMED on an open CONFIRM')
    WHERE EXISTS (
        SELECT 1 FROM proposal_review
        WHERE proposal_review.proposal_id = NEW.proposal_id
          AND proposal_review.risk_tier <> 'LOW_AUTOSAVE'
    )
      AND (
        NEW.epistemic_status <> 'USER_CONFIRMED'
        OR NEW.disposition_seq IS NULL
        OR NOT EXISTS (
            SELECT 1 FROM proposal_disposition_record AS chosen
            WHERE chosen.proposal_id = NEW.proposal_id
              AND chosen.disposition_seq = NEW.disposition_seq
              AND chosen.disposition = 'CONFIRM'
              AND NOT EXISTS (
                SELECT 1 FROM proposal_disposition_record AS undo
                WHERE undo.proposal_id = chosen.proposal_id
                  AND undo.supersedes_seq = chosen.disposition_seq
              )
        )
      );
END;

CREATE TRIGGER guard_proposal_batching_policy_update
BEFORE UPDATE ON proposal_batching_policy
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_batching_policy_delete
BEFORE DELETE ON proposal_batching_policy
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_batching_cut_update
BEFORE UPDATE ON proposal_batching_cut
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_batching_cut_delete
BEFORE DELETE ON proposal_batching_cut
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_review_update
BEFORE UPDATE ON proposal_review
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_review_delete
BEFORE DELETE ON proposal_review
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_disposition_record_update
BEFORE UPDATE ON proposal_disposition_record
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_disposition_record_delete
BEFORE DELETE ON proposal_disposition_record
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_outcome_update
BEFORE UPDATE ON proposal_outcome
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
CREATE TRIGGER guard_proposal_outcome_delete
BEFORE DELETE ON proposal_outcome
BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;
