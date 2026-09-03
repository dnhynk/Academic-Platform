//! Synthetic fixtures for the `P2-L3` acceptance suite.
//!
//! **Nothing here records or transcribes anything.** Every audio "chunk" is a
//! committed byte string, every image is another one, and every provider is a
//! table lookup over literals in this file. No microphone, no camera, no
//! speech engine, no socket, and no clock: `CONTRIBUTING.md` requires synthetic
//! fixtures only, and a run built from the wall clock would make every instant
//! these rows assert against depend on when the suite ran.
//!
//! The section 3.7 half is `crates/capture/tests/common/mod.rs` restated rather
//! than imported -- a test module is not a library target -- and it is built
//! from the same public `academic-consent` API, so the capture journals these
//! fixtures read are written by the real `academic_capture::begin`.

// Three suites share this module and each uses part of it.
#![allow(dead_code)]

use std::{error::Error, path::PathBuf, str::FromStr};

use academic_capture::{CapturePolicyBook, JournalRecovery, MicrophoneState, PreflightReading};
use academic_consent::{
    AuthorityGrant, CaptureMedium, CaptureProcessing, CaptureRequest, Checklist, ConsentLedger,
    Disposition, EvidenceArtifact, GrantAuthority, PermissionRecord, PermissionScope,
    RetentionBound, RetentionTerms, ScopeGrain, Season, TermKey, WrittenAuthority,
    WrittenEvidenceKind, permission::PermittedUse,
};
use academic_domain::{
    Actor, ArtifactId as DomainArtifactId, CapturePermissionId, ContentDigest, EntityId,
    LectureSessionId, OfferingId,
};
use academic_model_run::{
    ArtifactId, Cost, Digest32, ModelRunId, ModelVersion, ProviderId, Purpose,
};
use academic_transcription::{
    AudioFormat, AuthorizationBinding, ChunkBoundary, ConfidenceSemantics, ContractDraft,
    ContractRegistry, InputManifest, ProviderContract, ProviderPlacement, ProviderResponse,
    ProviderSelection, RESPONSE_BANNER, RunIdentity, SttProvider, Support, TimestampSemantics,
    TranscriptionRequest,
};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The instant the fixture term opens.
pub const TERM_FROM: u64 = 1_000_000;
/// The instant the fixture term closes, exclusive.
pub const TERM_TO: u64 = 2_000_000;
/// An instant inside the fixture term: every permitted row binds here.
pub const INSIDE: u64 = 1_500_000;
/// A second instant inside the term, which mints a different capability token.
pub const INSIDE_LATER: u64 = 1_500_001;
/// One second, in nanoseconds.
pub const SECOND: u64 = 1_000_000_000;

/// The offering the fixtures grant against.
pub fn offering() -> Result<OfferingId, Box<dyn Error>> {
    Ok(OfferingId::from_str(
        "01900000-0000-7000-8000-0000000000a1",
    )?)
}

/// The lecture session the fixtures capture.
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

/// The term the fixtures record in.
pub fn term() -> Result<TermKey, Box<dyn Error>> {
    Ok(TermKey::new(2026, Season::First)?)
}

/// The user every settled correction is attributed to.
pub fn user() -> Result<Actor, Box<dyn Error>> {
    Ok(Actor::User {
        user_id: EntityId::from_str("01900000-0000-7000-8000-0000000000f1")?,
    })
}

/// A model run, which is the actor a proposal comes from.
pub fn model_actor() -> Result<Actor, Box<dyn Error>> {
    Ok(Actor::ModelRun {
        run_id: EntityId::from_str("01900000-0000-7000-8000-0000000000f2")?,
    })
}

fn artifact(tag: &str) -> Result<EvidenceArtifact, Box<dyn Error>> {
    Ok(EvidenceArtifact::new(
        DomainArtifactId::from_str("01900000-0000-7000-8000-0000000000e7")?,
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
        offering()?,
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

/// A ledger holding one whole-term grant over audio and board photographs.
pub fn ledger_permitting() -> Result<ConsentLedger, Box<dyn Error>> {
    let mut ledger = ConsentLedger::new();
    ledger.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope()?,
            Disposition::Granted(AuthorityGrant::record(
                written_authority()?,
                PermittedUse::new(
                    vec![CaptureMedium::Audio, CaptureMedium::PhotoOfBoard],
                    vec![CaptureProcessing::LocalStt],
                    false,
                    false,
                ),
                split_retention(),
                Vec::new(),
                TERM_TO,
            )),
            checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    Ok(ledger)
}

/// The capture request the fixtures bind.
pub fn request() -> Result<CaptureRequest, Box<dyn Error>> {
    Ok(CaptureRequest {
        offering_id: Some(offering()?),
        lecture_id: Some(lecture()?),
        term: Some(term()?),
        media: Some(vec![CaptureMedium::Audio, CaptureMedium::PhotoOfBoard]),
        processing: Some(vec![CaptureProcessing::LocalStt]),
        requested_at: Some(INSIDE),
        not_after: Some(TERM_TO),
    })
}

/// One synthetic audio chunk. A committed literal; nothing was recorded.
pub fn chunk(tag: &str) -> Vec<u8> {
    format!("synthetic-lecture-audio:{tag}").into_bytes()
}

/// One synthetic image. A committed literal; no camera was opened.
pub fn image(tag: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF_u8, 0xD8];
    bytes.extend_from_slice(format!("synthetic-board-photo:{tag}").as_bytes());
    bytes
}

/// A reading comfortably above the shipped floors.
pub fn healthy_reading() -> PreflightReading {
    PreflightReading::observed(4 * 1024 * 1024 * 1024, 80, false, MicrophoneState::Held)
}

/// A journal path inside `directory` that nothing has created yet.
pub fn journal_path(directory: &tempfile::TempDir, tag: &str) -> PathBuf {
    directory.path().join(format!("{tag}.acjrnl"))
}

/// Writes a real capture journal: two audio chunks and one board photograph.
///
/// It goes through `academic_capture::begin`, so the journal's header carries
/// the capability token `mint_capture_capability` returned and the policy row
/// the capture began under. `now` decides the token identifier, so two calls at
/// two instants produce two journals under two authorizations.
pub fn write_journal(
    directory: &tempfile::TempDir,
    tag: &str,
    now: u64,
) -> Result<Capture, Box<dyn Error>> {
    let mut ledger = ledger_permitting()?;
    let path = journal_path(directory, tag);
    let book = CapturePolicyBook::published();
    let mut recorder = academic_capture::begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        now,
    )?;
    recorder.record_audio_chunk(&mut ledger, chunk("first"), 0, now)?;
    recorder.record_audio_chunk(&mut ledger, chunk("second"), 5 * SECOND, now)?;
    recorder.capture_image(
        &mut ledger,
        image("board"),
        academic_capture::Orientation::TopLeft,
        6 * SECOND,
        now,
    )?;
    recorder.mark(&mut ledger, 7 * SECOND, now)?;
    let recovery = recorder.verify_on_disk()?;
    Ok(Capture { recorder, recovery })
}

/// One finished capture: the recorder that made it, and what is on disk.
///
/// The pair travels together because `AuthorizationBinding::of` takes both. The
/// recorder is what carries the authorization -- it has no public constructor,
/// so holding one is proof that `academic_capture::begin` minted a capability
/// token -- and the journal is what is compared against it.
#[derive(Debug)]
pub struct Capture {
    /// The recorder `begin` returned.
    pub recorder: academic_capture::CaptureRecorder,
    /// What it wrote, read back off disk.
    pub recovery: JournalRecovery,
}

/// A journal nothing captured, built by this file and replayed from bytes.
///
/// `ChunkJournal::replay` is public and takes bytes, so this is a
/// `JournalRecovery` whose header names a capability token no
/// `mint_capture_capability` ever returned. It is what makes the binding rule a
/// comparison against the capture rather than a comparison of the journal with
/// itself. Every byte is a literal computed here; nothing was recorded.
pub fn forged_journal_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"ACJRNL01");
    // The three header digests: a clock domain, a policy row and a capability
    // token, all invented.
    for tag in [
        b"forged-domain".as_slice(),
        b"forged-policy",
        b"forged-token",
    ] {
        bytes.extend_from_slice(ContentDigest::sha256(tag).as_bytes());
    }
    bytes
}

/// A manifest holding both audio frames, the capture, and one supplied
/// material.
pub fn full_manifest(capture: &Capture) -> Result<InputManifest, Box<dyn Error>> {
    let recovery = &capture.recovery;
    let mut manifest =
        InputManifest::for_binding(AuthorizationBinding::of(&capture.recorder, recovery)?);
    for record in recovery.records() {
        if matches!(
            record.body(),
            academic_capture::RecordBody::AudioChunk { .. }
        ) {
            manifest.admit_audio_chunk(recovery, record.seq())?;
        }
        if matches!(
            record.body(),
            academic_capture::RecordBody::ImageCapture { .. }
        ) {
            manifest.admit_capture(recovery, record.seq())?;
        }
    }
    manifest.admit_supplied_material(&user()?, "week-07-slides", b"synthetic-slide-deck")?;
    Ok(manifest)
}

/// The local provider identifier every default-route fixture uses.
pub fn local_provider() -> Result<ProviderId, Box<dyn Error>> {
    Ok(ProviderId::new("mock-local-stt")?)
}

/// The remote provider identifier every scoped-remote fixture uses.
pub fn remote_provider() -> Result<ProviderId, Box<dyn Error>> {
    Ok(ProviderId::new("mock-remote-stt")?)
}

/// The model version every fixture declares.
pub fn version_one() -> Result<ModelVersion, Box<dyn Error>> {
    Ok(ModelVersion::new("v1.0.0")?)
}

/// A second model version, for the re-transcription comparison.
pub fn version_two() -> Result<ModelVersion, Box<dyn Error>> {
    Ok(ModelVersion::new("v2.0.0")?)
}

/// A whole contract: every declaration made, every capability offered.
pub fn whole_contract(
    provider: ProviderId,
    model_version: ModelVersion,
    placement: ProviderPlacement,
) -> Result<ProviderContract, Box<dyn Error>> {
    Ok(whole_draft(provider, model_version, placement)?.declare()?)
}

/// The draft `whole_contract` closes, so a row can drop one declaration.
pub fn whole_draft(
    provider: ProviderId,
    model_version: ModelVersion,
    placement: ProviderPlacement,
) -> Result<ContractDraft, Box<dyn Error>> {
    Ok(
        ContractDraft::for_provider(provider, model_version, placement)
            .audio_format(AudioFormat::new("wav/pcm_s16le", 16_000, 1))
            .chunk_boundary(ChunkBoundary::new(30 * SECOND, 2 * SECOND)?)
            .language_hints(Support::Offered)
            .vocabulary_hints(Support::Offered)
            .timestamp_semantics(TimestampSemantics::WordAndSegment)
            .confidence_semantics(ConfidenceSemantics::PerToken)
            .diarization(Support::Offered)
            .math_and_code(Support::Offered),
    )
}

/// A registry holding one whole local contract.
pub fn registry_with_local() -> Result<ContractRegistry, Box<dyn Error>> {
    let mut registry = ContractRegistry::new();
    registry.declare(whole_contract(
        local_provider()?,
        version_one()?,
        ProviderPlacement::Local,
    )?)?;
    Ok(registry)
}

/// The selection every default-route fixture makes.
pub fn local_selection() -> Result<ProviderSelection, Box<dyn Error>> {
    Ok(ProviderSelection::of(
        local_provider()?,
        version_one()?,
        vec![],
    ))
}

/// One synthetic provider response over two segments and five tokens.
///
/// Written to the wire grammar in `RESPONSE_BANNER`'s version. Every value is a
/// literal in this function.
pub fn response_body(words: &[&str]) -> String {
    let mut body = String::new();
    body.push_str(RESPONSE_BANNER);
    body.push('\n');
    body.push_str("segment: raw_segment_0001 0 5000000000 instructor 0,1\n");
    body.push_str("verbatim: serializability is the goal\n");
    for (index, word) in words.iter().take(4).enumerate() {
        let start = u64::try_from(index).unwrap_or(0) * SECOND;
        body.push_str(&format!("word: {start} {} {word}\n", 900 - index * 10));
    }
    body.push_str("segment: raw_segment_0002 5000000000 9000000000 student_unknown_2 1\n");
    body.push_str("verbatim: is that two phase locking\n");
    body.push_str("word: 5000000000 630 locking\n");
    body
}

/// The four words the default response returns.
pub const DEFAULT_WORDS: [&str; 4] = ["serializability", "is", "the", "goal"];

/// A provider that answers with a fixed body on the local route.
#[derive(Debug)]
pub struct MockLocalProvider {
    provider: ProviderId,
    model_version: ModelVersion,
    body: String,
}

impl MockLocalProvider {
    /// A provider answering `body`.
    pub fn answering(
        provider: ProviderId,
        model_version: ModelVersion,
        body: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            model_version,
            body: body.into(),
        }
    }
}

impl SttProvider for MockLocalProvider {
    fn transcribe(&self, request: &TranscriptionRequest<'_>) -> Option<ProviderResponse> {
        // A provider reads the audio it was handed. The fixture reads it and
        // discards it: what it answers with is the literal above, so no row in
        // this suite depends on a decode of a byte string nothing produced.
        let _audio: usize = request
            .manifest
            .chunks()
            .iter()
            .map(|chunk| chunk.audio().len())
            .sum();
        Some(ProviderResponse::from_local(
            self.provider.clone(),
            self.model_version.clone(),
            self.body.as_bytes(),
        ))
    }
}

/// A provider whose answer is arbitrary bytes rather than text.
///
/// `academic-untrusted-content` refuses bytes that are not UTF-8, so this is
/// what makes the retention stage's own failure reachable.
#[derive(Debug)]
pub struct RawBytesProvider {
    pub provider: ProviderId,
    pub model_version: ModelVersion,
    pub bytes: Vec<u8>,
}

impl SttProvider for RawBytesProvider {
    fn transcribe(&self, _request: &TranscriptionRequest<'_>) -> Option<ProviderResponse> {
        Some(ProviderResponse::from_local(
            self.provider.clone(),
            self.model_version.clone(),
            &self.bytes,
        ))
    }
}

/// A provider that fails.
#[derive(Debug)]
pub struct FailingProvider;

impl SttProvider for FailingProvider {
    fn transcribe(&self, _request: &TranscriptionRequest<'_>) -> Option<ProviderResponse> {
        None
    }
}

/// A provider that answers a local route with a locally-built response for
/// another provider, so `RouteMismatch` has a case.
#[derive(Debug)]
pub struct ImpersonatingProvider {
    pub provider: ProviderId,
    pub model_version: ModelVersion,
    pub body: String,
}

impl SttProvider for ImpersonatingProvider {
    fn transcribe(&self, _request: &TranscriptionRequest<'_>) -> Option<ProviderResponse> {
        Some(ProviderResponse::from_local(
            self.provider.clone(),
            self.model_version.clone(),
            self.body.as_bytes(),
        ))
    }
}

/// The seven section 27.3 fields a caller supplies.
pub fn run_identity() -> Result<RunIdentity, Box<dyn Error>> {
    Ok(RunIdentity {
        id: ModelRunId::from_bytes([7_u8; 16]),
        purpose: Purpose::new("LECTURE_TRANSCRIPTION")?,
        prompt_template_hash: Digest32::of(b"academic-transcription-prompt/1"),
        redaction_policy_hash: Digest32::of(b"academic-dlp-rulepack/1"),
        output_artifact: ArtifactId::from_bytes([9_u8; 16]),
        started_at: INSIDE,
        cost: Cost::new(0, "KRW")?,
        transmission: None,
    })
}
