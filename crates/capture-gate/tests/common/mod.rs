//! Synthetic fixtures for the `P2-L1` acceptance suite.
//!
//! Everything here is built from committed literals. No clock, no randomness,
//! no device and no file: `CONTRIBUTING.md` requires synthetic fixtures only,
//! and a capture built from the wall clock would make the boundary rows
//! non-deterministic.
//!
//! No test in this tree records anything. A "chunk" is a committed byte string.

// Two suites share this module and each uses part of it; an unused fixture here
// is a fixture the other suite needs.
#![allow(dead_code)]

use std::{error::Error, str::FromStr};

use academic_consent::{
    AuthorityGrant, CaptureMedium, CaptureProcessing, CaptureRequest, Checklist, ConsentLedger,
    Disposition, EvidenceArtifact, GrantAuthority, PermissionRecord, PermissionScope,
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
/// An instant inside the fixture term, and the one every row binds at.
pub const INSIDE: u64 = 1_500_000;
/// The instant the fixture token stops at.
pub const TOKEN_UNTIL: u64 = 1_600_000;

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

/// The aggregate identifier the fixtures record under.
pub fn permission_id() -> Result<CapturePermissionId, Box<dyn Error>> {
    Ok(CapturePermissionId::from_str(
        "01900000-0000-7000-8000-0000000000d5",
    )?)
}

/// A second aggregate identifier, for a superseding record.
pub fn other_permission_id() -> Result<CapturePermissionId, Box<dyn Error>> {
    Ok(CapturePermissionId::from_str(
        "01900000-0000-7000-8000-0000000000d6",
    )?)
}

/// The term the fixtures are recorded in.
pub fn term() -> Result<TermKey, Box<dyn Error>> {
    Ok(TermKey::new(2026, Season::First)?)
}

/// The following term, for the semester-recheck row.
pub fn next_term() -> Result<TermKey, Box<dyn Error>> {
    Ok(TermKey::new(2026, Season::Second)?)
}

fn artifact(tag: &str) -> Result<EvidenceArtifact, Box<dyn Error>> {
    Ok(EvidenceArtifact::new(
        ArtifactId::from_str("01900000-0000-7000-8000-0000000000e7")?,
        ContentDigest::sha256(tag.as_bytes()),
        u64::try_from(tag.len())?,
    ))
}

/// A written syllabus act by the instructor.
pub fn written_authority() -> Result<WrittenAuthority, Box<dyn Error>> {
    Ok(WrittenAuthority::new(
        GrantAuthority::Instructor,
        WrittenEvidenceKind::Syllabus,
        artifact("syllabus")?,
    ))
}

fn whole_term_scope(
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

/// A checklist with every dimension answered.
pub fn checklist() -> Result<Checklist, Box<dyn Error>> {
    let mut checklist = Checklist::new();
    for dimension in academic_consent::CHECKLIST_DIMENSIONS {
        checklist.answer(
            dimension,
            academic_consent::ChecklistEntry::Evidenced(artifact(dimension.as_str())?),
        )?;
    }
    Ok(checklist)
}

fn split_retention() -> RetentionTerms {
    RetentionTerms::new(
        RetentionBound::Until(1_600_000),
        RetentionBound::Until(1_900_000),
    )
}

/// A grant over `media`, expiring at `not_after`.
pub fn grant_over(
    media: Vec<CaptureMedium>,
    not_after: u64,
) -> Result<AuthorityGrant, Box<dyn Error>> {
    Ok(AuthorityGrant::record(
        written_authority()?,
        PermittedUse::new(media, vec![CaptureProcessing::LocalStt], false, false),
        split_retention(),
        Vec::new(),
        not_after,
    ))
}

/// A ledger holding one whole-term grant over `media` expiring at `not_after`.
pub fn ledger_granting(
    media: Vec<CaptureMedium>,
    not_after: u64,
) -> Result<ConsentLedger, Box<dyn Error>> {
    let mut ledger = ConsentLedger::new();
    ledger.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Granted(grant_over(media, not_after)?),
            checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    Ok(ledger)
}

/// The ledger every permitting row uses: audio only, live to the end of term.
pub fn ledger_audio_only() -> Result<ConsentLedger, Box<dyn Error>> {
    ledger_granting(vec![CaptureMedium::Audio], TERM_TO)
}

/// A ledger holding one written refusal.
pub fn ledger_refusing() -> Result<ConsentLedger, Box<dyn Error>> {
    let mut ledger = ConsentLedger::new();
    ledger.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Prohibited(RefusalRecord::record(written_authority()?, TERM_FROM)),
            checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    Ok(ledger)
}

/// Appends a written refusal at the next sequence, superseding the grant.
///
/// This is the authority answering during the lecture, which is what makes a
/// quarantine reachable: the chunks already recorded no longer re-bind.
pub fn append_refusal(ledger: &mut ConsentLedger, at: u64) -> TestResult {
    ledger.record_permission(
        PermissionRecord::record(
            other_permission_id()?,
            2,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Prohibited(RefusalRecord::record(written_authority()?, at)),
            checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"refusal verification"),
        )?,
        at,
    )?;
    Ok(())
}

/// A whole request over `media`, stopping at `not_after`.
pub fn request_for(
    media: Vec<CaptureMedium>,
    not_after: u64,
) -> Result<CaptureRequest, Box<dyn Error>> {
    Ok(CaptureRequest {
        offering_id: Some(offering_a()?),
        lecture_id: Some(lecture()?),
        term: Some(term()?),
        media: Some(media),
        processing: Some(vec![CaptureProcessing::LocalStt]),
        requested_at: Some(INSIDE),
        not_after: Some(not_after),
    })
}

/// The request every permitting row uses.
pub fn audio_request() -> Result<CaptureRequest, Box<dyn Error>> {
    request_for(vec![CaptureMedium::Audio], TOKEN_UNTIL)
}

/// One synthetic chunk. A committed literal; nothing was recorded to make it.
pub fn chunk(tag: &str) -> Vec<u8> {
    format!("synthetic-capture-chunk:{tag}").into_bytes()
}
