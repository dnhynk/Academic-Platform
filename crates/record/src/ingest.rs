//! Turning `P2-U7`'s user-confirmed rows into `P2-U4` attempts.
//!
//! This is the seam between the two tasks, and it is deliberately narrow. The
//! transcript crate already decided what a confirmed row is:
//! `confirm_reconciled_rows` mints one only from a `ReconciledTranscript`,
//! which `reconcile` returns only when every row agreed on all four fields, and
//! `Claim::validate_for_actor` permits `AuthorityClass::UserExplicit` to
//! `Actor::User` and to nobody else. **None of that is re-decided here.** This
//! module pairs each row with the confirmed claim that was minted for it and
//! refuses every way the pair could be wrong:
//!
//! - a claim whose ordinal is not the row's, which would attach a confirmation
//!   to a different line of the document;
//! - a claim whose object text is not the row's four fields, which would let a
//!   confirmation of one row stand for another with the same ordinal;
//! - a claim that is not `UserExplicit`/`UserConfirmed`, which is what an
//!   *import* row is — the whole point of the two-claim split is that an
//!   importer's reading is not a confirmation.
//!
//! What that buys: a `CourseAttempt` built through this path exists only where
//! a user-confirmed row does. `registered_attempt_gate` executes the three
//! refusals rather than describing them.

use academic_domain::{AttemptId, AuthorityClass, EpistemicStatus, EvidenceId};
use academic_transcript::{
    claims::ConfirmedRowClaim,
    record::{TranscriptField, TranscriptRow},
};

use crate::{
    RecordError,
    attempt::{CourseAttempt, SettledStatus},
    grade::GradeSymbol,
    policy::AttemptOrigin,
    term::TermKey,
};

/// Builds one attempt from a transcript row and the confirmation minted for it.
///
/// `status` and `origin` are the caller's because a transcript row does not
/// carry them: whether a row is 편입, 교환, or an ordinary completion is a fact
/// about the record as a whole, and `GATE-38-006` leaves the recognition
/// decision to the user in any case. Everything the row *does* carry —
/// course code, term, credits, grade — is read from the row and never from the
/// caller.
pub fn attempt_from_confirmed_row(
    id: AttemptId,
    row: &TranscriptRow,
    confirmed: &ConfirmedRowClaim,
    status: SettledStatus,
    origin: AttemptOrigin,
    grading_scheme_id: impl Into<String>,
    evidence_ids: Vec<EvidenceId>,
) -> Result<CourseAttempt, RecordError> {
    if confirmed.ordinal() != row.ordinal() {
        return Err(RecordError::ConfirmationOrdinalMismatch {
            row: row.ordinal(),
            confirmation: confirmed.ordinal(),
        });
    }
    let claim = confirmed.claim();
    if claim.authority_class != AuthorityClass::UserExplicit
        || claim.epistemic_status != EpistemicStatus::UserConfirmed
    {
        return Err(RecordError::AttemptNeedsAConfirmedRow);
    }
    if claim_object_text(claim) != Some(row_object_text(row)) {
        return Err(RecordError::ConfirmationIsForAnotherRow);
    }

    let grade = GradeSymbol::parse(row.grade())
        .ok_or_else(|| RecordError::UnknownGradeSymbol(row.grade().to_owned()))?;
    let earns = crate::grade::GradingScheme::snu_4_3_v1()?
        .treatment(grade)
        .earns_credit();
    CourseAttempt::from_confirmed_row(
        id,
        row.course_code(),
        TermKey::parse_transcript_term(row.term())?,
        status,
        origin,
        row.credits(),
        if earns {
            row.credits()
        } else {
            crate::decimal::rescale(crate::decimal::zero()?, row.credits().scale())?
        },
        Some(grade),
        grading_scheme_id,
        evidence_ids,
    )
}

/// Renders the four reconciled fields the way the transcript crate does.
///
/// The spelling is `academic_transcript::claims`'s, reproduced here because
/// that function is private to it. Nothing forces the two to agree, so the
/// agreement is executed rather than assumed: `registered_attempt_gate` builds
/// a claim pair through `confirm_reconciled_rows` — the transcript crate's own
/// public API — and hands it to [`attempt_from_confirmed_row`], which succeeds
/// only if the two spellings match. A change on either side turns that row's
/// success into a `ConfirmationIsForAnotherRow` refusal.
fn row_object_text(row: &TranscriptRow) -> String {
    TranscriptField::ALL
        .map(|field| format!("{}={}", field.as_str(), row.field(field)))
        .join(";")
}

fn claim_object_text(claim: &academic_domain::Claim) -> Option<String> {
    match &claim.object {
        academic_domain::ClaimObject::Text(text) => Some(text.clone()),
        _ => None,
    }
}
