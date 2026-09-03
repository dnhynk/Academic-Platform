//! Synthetic fixtures for the `P2-L2` acceptance suite.
//!
//! Everything here is built from committed literals. No clock, no randomness,
//! no device: `CONTRIBUTING.md` requires synthetic fixtures only, and a capture
//! built from the wall clock would make every instant these rows assert against
//! depend on when the suite ran.
//!
//! **No test in this tree records anything.** A "chunk" is a byte string in
//! this file and an "image" is another one. Nothing opens a microphone, a
//! camera or a screen, and there is no code path in `academic-capture` that
//! could.
//!
//! The section 3.7 half is `crates/capture-gate/tests/common/mod.rs` restated
//! rather than imported: no workspace crate may depend on `academic-capture-gate`,
//! for the reason `only_egress_crate_has_a_socket` gives, so the two suites
//! build the same synthetic aggregate from the same public `academic-consent`
//! API side by side.

// Three suites share this module and each uses part of it; an unused fixture
// here is a fixture another suite needs.
#![allow(dead_code)]

use std::{error::Error, path::PathBuf, str::FromStr};

use academic_consent::{
    AuthorityGrant, CaptureMedium, CaptureProcessing, CaptureRequest, Checklist, ConsentLedger,
    Disposition, EvidenceArtifact, GrantAuthority, PermissionRecord, PermissionScope,
    RefusalRecord, RetentionBound, RetentionTerms, ScopeGrain, Season, TermKey, WrittenAuthority,
    WrittenEvidenceKind, permission::PermittedUse,
};
use academic_domain::{
    ArtifactId, CapturePermissionId, ContentDigest, LectureSessionId, OfferingId,
};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The instant the fixture term opens.
pub const TERM_FROM: u64 = 1_000_000;
/// The instant the fixture term closes, exclusive.
pub const TERM_TO: u64 = 2_000_000;
/// An instant inside the fixture term, and the one every row binds at.
pub const INSIDE: u64 = 1_500_000;

/// One second, in nanoseconds. Every elapsed reading below is a multiple.
pub const SECOND: u64 = 1_000_000_000;

/// The offering the fixtures grant against.
pub fn offering_a() -> Result<OfferingId, Box<dyn Error>> {
    Ok(OfferingId::from_str(
        "01900000-0000-7000-8000-0000000000a1",
    )?)
}

/// The session the fixtures capture.
pub fn lecture() -> Result<LectureSessionId, Box<dyn Error>> {
    Ok(LectureSessionId::from_str(
        "01900000-0000-7000-8000-0000000000c3",
    )?)
}

fn permission_id() -> Result<CapturePermissionId, Box<dyn Error>> {
    Ok(CapturePermissionId::from_str(
        "01900000-0000-7000-8000-0000000000d5",
    )?)
}

fn other_permission_id() -> Result<CapturePermissionId, Box<dyn Error>> {
    Ok(CapturePermissionId::from_str(
        "01900000-0000-7000-8000-0000000000d6",
    )?)
}

/// The term the fixtures are recorded in.
pub fn term() -> Result<TermKey, Box<dyn Error>> {
    Ok(TermKey::new(2026, Season::First)?)
}

fn artifact(tag: &str) -> Result<EvidenceArtifact, Box<dyn Error>> {
    Ok(EvidenceArtifact::new(
        ArtifactId::from_str("01900000-0000-7000-8000-0000000000e7")?,
        ContentDigest::sha256(tag.as_bytes()),
        u64::try_from(tag.len())?,
    ))
}

fn written_authority() -> Result<WrittenAuthority, Box<dyn Error>> {
    Ok(WrittenAuthority::new(
        GrantAuthority::Instructor,
        WrittenEvidenceKind::Syllabus,
        artifact("syllabus")?,
    ))
}

fn whole_term_scope() -> Result<PermissionScope, Box<dyn Error>> {
    Ok(PermissionScope::new(
        offering_a()?,
        term()?,
        ScopeGrain::WholeTerm,
        TERM_FROM,
        TERM_TO,
    )?)
}

fn checklist() -> Result<Checklist, Box<dyn Error>> {
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

/// A ledger holding one whole-term grant over `media`, expiring at `not_after`.
pub fn ledger_granting(
    media: Vec<CaptureMedium>,
    not_after: u64,
) -> Result<ConsentLedger, Box<dyn Error>> {
    let mut ledger = ConsentLedger::new();
    ledger.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope()?,
            Disposition::Granted(AuthorityGrant::record(
                written_authority()?,
                PermittedUse::new(media, vec![CaptureProcessing::LocalStt], false, false),
                split_retention(),
                Vec::new(),
                not_after,
            )),
            checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    Ok(ledger)
}

/// The ledger every permitting row uses: audio and board photos, live to the
/// end of term.
pub fn ledger_permitting() -> Result<ConsentLedger, Box<dyn Error>> {
    ledger_granting(
        vec![CaptureMedium::Audio, CaptureMedium::PhotoOfBoard],
        TERM_TO,
    )
}

/// A ledger holding one written refusal, so nothing binds.
pub fn ledger_refusing() -> Result<ConsentLedger, Box<dyn Error>> {
    let mut ledger = ConsentLedger::new();
    ledger.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope()?,
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
pub fn append_refusal(ledger: &mut ConsentLedger, at: u64) -> TestResult {
    ledger.record_permission(
        PermissionRecord::record(
            other_permission_id()?,
            2,
            whole_term_scope()?,
            Disposition::Prohibited(RefusalRecord::record(written_authority()?, at)),
            checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"refusal verification"),
        )?,
        at,
    )?;
    Ok(())
}

/// A whole request over both media, stopping at `not_after`.
pub fn request_until(not_after: u64) -> Result<CaptureRequest, Box<dyn Error>> {
    Ok(CaptureRequest {
        offering_id: Some(offering_a()?),
        lecture_id: Some(lecture()?),
        term: Some(term()?),
        media: Some(vec![CaptureMedium::Audio, CaptureMedium::PhotoOfBoard]),
        processing: Some(vec![CaptureProcessing::LocalStt]),
        requested_at: Some(INSIDE),
        not_after: Some(not_after),
    })
}

/// The request every permitting row uses.
pub fn request() -> Result<CaptureRequest, Box<dyn Error>> {
    request_until(TERM_TO)
}

/// One synthetic audio chunk. A committed literal; nothing was recorded.
pub fn chunk(tag: &str) -> Vec<u8> {
    format!("synthetic-capture-chunk:{tag}").into_bytes()
}

/// One synthetic image. A committed literal; no camera was opened.
///
/// It deliberately opens with the two bytes a JPEG does and carries no EXIF
/// block, so a reader that went looking for an orientation inside the bytes
/// would find none — which is what `capture_metadata_integrity` means by an
/// EXIF-independent orientation.
pub fn image(tag: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF_u8, 0xD8];
    bytes.extend_from_slice(format!("synthetic-board-photo:{tag}").as_bytes());
    bytes
}

/// A reading with every resource comfortably above the shipped floors.
pub fn healthy_reading() -> academic_capture::PreflightReading {
    academic_capture::PreflightReading::observed(
        4 * 1024 * 1024 * 1024,
        80,
        false,
        academic_capture::MicrophoneState::Held,
    )
}

/// A journal path inside `directory` that nothing has created yet.
pub fn journal_path(directory: &tempfile::TempDir, tag: &str) -> PathBuf {
    directory.path().join(format!("{tag}.acjrnl"))
}
