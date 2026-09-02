//! Synthetic fixtures for the `P2-G6` acceptance suite.
//!
//! Everything here is built from committed literals. No clock, no randomness,
//! and no file is read: `CONTRIBUTING.md` requires synthetic fixtures only, and
//! a consent record built from the wall clock would make the expiry rows
//! non-deterministic.

use std::{error::Error, str::FromStr};

use academic_consent::{
    AttestationKind, AttestationRecord, AuthorityGrant, CaptureMedium, CaptureProcessing,
    Checklist, ChecklistDimension, ChecklistEntry, Condition, ConsentLedger, Disposition,
    EvidenceArtifact, GrantAuthority, NotApplicableReason, PermissionRecord, PermissionScope,
    RefusalRecord, RetentionBound, RetentionTerms, ScopeGrain, Season, TermKey, WrittenAuthority,
    WrittenEvidenceKind, permission::PermittedUse,
};
use academic_domain::{
    ArtifactId, CapturePermissionId, ContentDigest, LectureSessionId, OfferingId,
};

/// The instant the fixture term opens.
pub const TERM_FROM: u64 = 1_000_000;
/// The instant the fixture term closes, exclusive.
pub const TERM_TO: u64 = 2_000_000;
/// An instant inside the fixture term.
pub const INSIDE: u64 = 1_500_000;

pub type TestResult = Result<(), Box<dyn Error>>;

/// The offering the fixtures grant against.
pub fn offering_a() -> Result<OfferingId, Box<dyn Error>> {
    Ok(OfferingId::from_str(
        "01900000-0000-7000-8000-0000000000a1",
    )?)
}

/// A second offering, for the scope row.
pub fn offering_b() -> Result<OfferingId, Box<dyn Error>> {
    Ok(OfferingId::from_str(
        "01900000-0000-7000-8000-0000000000b2",
    )?)
}

/// The session the fixtures capture.
pub fn lecture() -> Result<LectureSessionId, Box<dyn Error>> {
    Ok(LectureSessionId::from_str(
        "01900000-0000-7000-8000-0000000000c3",
    )?)
}

/// A second session, for the single-lecture grain.
pub fn other_lecture() -> Result<LectureSessionId, Box<dyn Error>> {
    Ok(LectureSessionId::from_str(
        "01900000-0000-7000-8000-0000000000c4",
    )?)
}

/// The aggregate identifier the fixtures record under.
pub fn permission_id() -> Result<CapturePermissionId, Box<dyn Error>> {
    Ok(CapturePermissionId::from_str(
        "01900000-0000-7000-8000-0000000000d5",
    )?)
}

/// A second aggregate identifier.
pub fn other_permission_id() -> Result<CapturePermissionId, Box<dyn Error>> {
    Ok(CapturePermissionId::from_str(
        "01900000-0000-7000-8000-0000000000d6",
    )?)
}

/// The term the fixtures are recorded in.
pub fn term() -> Result<TermKey, Box<dyn Error>> {
    Ok(TermKey::new(2026, Season::First)?)
}

/// The following term.
pub fn next_term() -> Result<TermKey, Box<dyn Error>> {
    Ok(TermKey::new(2026, Season::Second)?)
}

/// A synthetic evidence artifact.
pub fn artifact(tag: &str) -> Result<EvidenceArtifact, Box<dyn Error>> {
    Ok(EvidenceArtifact::new(
        ArtifactId::from_str("01900000-0000-7000-8000-0000000000e7")?,
        ContentDigest::sha256(tag.as_bytes()),
        u64::try_from(tag.len())?,
    ))
}

/// A written syllabus act by the instructor.
pub fn written_syllabus() -> Result<WrittenAuthority, Box<dyn Error>> {
    Ok(WrittenAuthority::new(
        GrantAuthority::Instructor,
        WrittenEvidenceKind::Syllabus,
        artifact("syllabus")?,
    ))
}

/// A whole-term scope over the fixture interval.
pub fn whole_term_scope(
    offering_id: OfferingId,
    term: TermKey,
) -> Result<PermissionScope, Box<dyn Error>> {
    Ok(PermissionScope::new(
        offering_id,
        term,
        ScopeGrain::WholeTerm,
        TERM_FROM,
        TERM_TO,
    )?)
}

/// The audio-only local-processing use the fixtures grant.
pub fn audio_local_use() -> PermittedUse {
    PermittedUse::new(
        vec![CaptureMedium::Audio],
        vec![CaptureProcessing::LocalStt],
        false,
        false,
    )
}

/// The retention pair the fixtures grant: two different instants.
pub fn split_retention() -> RetentionTerms {
    RetentionTerms::new(
        RetentionBound::Until(1_600_000),
        RetentionBound::Until(1_900_000),
    )
}

/// A grant expiring at the end of the fixture term.
pub fn grant(conditions: Vec<Condition>) -> Result<AuthorityGrant, Box<dyn Error>> {
    Ok(AuthorityGrant::record(
        written_syllabus()?,
        audio_local_use(),
        split_retention(),
        conditions,
        TERM_TO,
    ))
}

/// A checklist with every dimension answered.
pub fn complete_checklist() -> Result<Checklist, Box<dyn Error>> {
    let mut checklist = Checklist::new();
    for dimension in academic_consent::CHECKLIST_DIMENSIONS {
        checklist.answer(dimension, answer_for(dimension)?)?;
    }
    Ok(checklist)
}

/// A checklist missing exactly one dimension.
pub fn checklist_missing(omitted: ChecklistDimension) -> Result<Checklist, Box<dyn Error>> {
    let mut checklist = Checklist::new();
    for dimension in academic_consent::CHECKLIST_DIMENSIONS {
        if dimension == omitted {
            continue;
        }
        checklist.answer(dimension, answer_for(dimension)?)?;
    }
    Ok(checklist)
}

/// One explicit answer per dimension: some evidenced, some not applicable.
fn answer_for(dimension: ChecklistDimension) -> Result<ChecklistEntry, Box<dyn Error>> {
    Ok(match dimension {
        ChecklistDimension::StudentSpeech => {
            ChecklistEntry::NotApplicable(NotApplicableReason::NoStudentParticipationIsCaptured)
        }
        ChecklistDimension::FilmingScope => {
            ChecklistEntry::NotApplicable(NotApplicableReason::NoVisualCaptureRequested)
        }
        ChecklistDimension::AccessibilityProcedure => {
            ChecklistEntry::NotApplicable(NotApplicableReason::NoAccommodationInEffect)
        }
        other => ChecklistEntry::Evidenced(artifact(other.as_str())?),
    })
}

/// A ledger holding one whole-term grant with a complete checklist.
pub fn ledger_with_grant() -> Result<ConsentLedger, Box<dyn Error>> {
    let mut ledger = ConsentLedger::new();
    ledger.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Granted(grant(Vec::new())?),
            complete_checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    Ok(ledger)
}

/// A ledger holding one written refusal.
pub fn ledger_with_refusal() -> Result<ConsentLedger, Box<dyn Error>> {
    let mut ledger = ConsentLedger::new();
    ledger.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Prohibited(RefusalRecord::record(written_syllabus()?, TERM_FROM)),
            complete_checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    Ok(ledger)
}

/// An attestation of the kind section 12.1 describes.
pub fn oral_attestation() -> AttestationRecord {
    AttestationRecord::file(
        AttestationKind::OralInstructorPermission,
        TERM_FROM,
        ContentDigest::sha256(b"heard the instructor say it was fine"),
    )
}

/// The self-judgement section 12.1 names as insufficient.
pub fn personal_use_attestation() -> AttestationRecord {
    AttestationRecord::file(
        AttestationKind::PersonalUseBelief,
        TERM_FROM,
        ContentDigest::sha256(b"it is only for my own study"),
    )
}
