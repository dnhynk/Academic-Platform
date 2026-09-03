//! Synthetic fixtures for the `P2-L4` acceptance suite.
//!
//! **Nothing here records, transcribes or renders anything.** Every audio
//! "chunk" is a committed byte string, every image is another one, the provider
//! is a table lookup over literals in this file, and every render measurement
//! is a number written here. No microphone, no camera, no speech engine, no
//! font, no socket, no clock.
//!
//! The `P2-L2` and `P2-L3` halves are `crates/transcription/tests/common/mod.rs`
//! restated rather than imported — a test module is not a library target — and
//! they are built from the same public APIs, so the journals these fixtures
//! read are written by the real `academic_capture::begin` and the transcripts
//! come out of the real `academic_transcription::run`.

// Several suites share this module and each uses part of it.
#![allow(dead_code)]

use std::{error::Error, path::PathBuf, str::FromStr};

use academic_capture::{
    CapturePolicyBook, JournalRecovery, MicrophoneState, Orientation, PreflightReading, RecordBody,
};
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
use academic_lecture_document::{
    DocumentAnnotation, DocumentBuilder, DocumentId, LectureDocument, NodeDraft, NodeId, NodeKind,
    PreservationTransform,
};
use academic_model_run::{
    ArtifactId, CalibrationBin, CalibrationDataset, CalibrationDatasetId, CalibrationRegistry,
    Cost, Digest32, ModelRunId, ModelVersion, ProviderId, Purpose,
};
use academic_transcription::{
    AudioFormat, AuthorizationBinding, ChunkBoundary, ConfidenceSemantics, ContractDraft,
    ContractRegistry, InputManifest, ProviderContract, ProviderPlacement, ProviderResponse,
    ProviderSelection, RESPONSE_BANNER, RawResponseArchive, RunIdentity, RunOutcome, SttPolicy,
    SttProvider, TranscriptLineage, TranscriptionRequest,
};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The instant the fixture term opens.
pub const TERM_FROM: u64 = 1_000_000;
/// The instant the fixture term closes, exclusive.
pub const TERM_TO: u64 = 2_000_000;
/// An instant inside the fixture term.
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

/// The person every declaration is attributed to.
pub fn user() -> Result<Actor, Box<dyn Error>> {
    Ok(Actor::User {
        user_id: EntityId::from_str("01900000-0000-7000-8000-0000000000f1")?,
    })
}

/// A model run, which is the automatic actor every refusal row uses.
pub fn model_actor() -> Result<Actor, Box<dyn Error>> {
    Ok(Actor::ModelRun {
        run_id: EntityId::from_str("01900000-0000-7000-8000-0000000000f2")?,
    })
}

/// A deterministic engine, the second automatic actor.
pub fn engine_actor() -> Actor {
    Actor::DeterministicEngine {
        name: "coverage".to_owned(),
        version: "1".to_owned(),
    }
}

/// An importer, the third.
pub fn importer_actor() -> Actor {
    Actor::Importer {
        name: "lms".to_owned(),
        version: "1".to_owned(),
    }
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

/// A reading below the shipped floors, which opens a resource-failure gap.
pub fn starved_reading() -> PreflightReading {
    PreflightReading::observed(1024, 2, false, MicrophoneState::Held)
}

/// A journal path inside `directory` that nothing has created yet.
pub fn journal_path(directory: &tempfile::TempDir, tag: &str) -> PathBuf {
    directory.path().join(format!("{tag}.acjrnl"))
}

/// One finished capture: the recorder that made it, and what is on disk.
#[derive(Debug)]
pub struct Capture {
    /// The recorder `begin` returned.
    pub recorder: academic_capture::CaptureRecorder,
    /// What it wrote, read back off disk.
    pub recovery: JournalRecovery,
}

/// A capture with no hole above the threshold.
///
/// Two audio frames one second apart, one board photograph, one mark. Frame
/// sequences are 0, 1, 2, 3 in that order.
pub fn clean_capture(directory: &tempfile::TempDir, tag: &str) -> Result<Capture, Box<dyn Error>> {
    let mut ledger = ledger_permitting()?;
    let path = journal_path(directory, tag);
    let book = CapturePolicyBook::published();
    let mut recorder = academic_capture::begin(
        &mut ledger,
        &request()?,
        &path,
        &book,
        healthy_reading(),
        INSIDE,
    )?;
    recorder.record_audio_chunk(&mut ledger, chunk("first"), 0, INSIDE)?;
    recorder.record_audio_chunk(&mut ledger, chunk("second"), SECOND, INSIDE)?;
    recorder.capture_image(
        &mut ledger,
        image("board"),
        Orientation::TopLeft,
        2 * SECOND,
        INSIDE,
    )?;
    recorder.mark(&mut ledger, 3 * SECOND, INSIDE)?;
    let recovery = recorder.verify_on_disk()?;
    Ok(Capture { recorder, recovery })
}

/// A manifest holding every audio frame, the capture, and one supplied
/// material.
pub fn full_manifest(capture: &Capture) -> Result<InputManifest, Box<dyn Error>> {
    let recovery = &capture.recovery;
    let mut manifest =
        InputManifest::for_binding(AuthorizationBinding::of(&capture.recorder, recovery)?);
    for record in recovery.records() {
        if matches!(record.body(), RecordBody::AudioChunk { .. }) {
            manifest.admit_audio_chunk(recovery, record.seq())?;
        }
        if matches!(record.body(), RecordBody::ImageCapture { .. }) {
            manifest.admit_capture(recovery, record.seq())?;
        }
    }
    manifest.admit_supplied_material(&user()?, "week-07-slides", b"synthetic-slide-deck")?;
    Ok(manifest)
}

/// The frame sequence of the one board photograph in a capture.
pub fn capture_frame_seq(capture: &Capture) -> Option<u32> {
    capture
        .recovery
        .records()
        .iter()
        .find(|record| matches!(record.body(), RecordBody::ImageCapture { .. }))
        .map(|record| record.seq())
}

/// The provider identifier every fixture uses.
pub fn local_provider() -> Result<ProviderId, Box<dyn Error>> {
    Ok(ProviderId::new("mock-local-stt")?)
}

/// The model version every fixture declares.
pub fn version_one() -> Result<ModelVersion, Box<dyn Error>> {
    Ok(ModelVersion::new("v1.0.0")?)
}

/// The purpose every run and every calibration dataset is keyed by.
pub fn purpose() -> Result<Purpose, Box<dyn Error>> {
    Ok(Purpose::new("LECTURE_TRANSCRIPTION")?)
}

/// A whole contract: every declaration made, every capability offered.
pub fn whole_contract() -> Result<ProviderContract, Box<dyn Error>> {
    Ok(
        ContractDraft::for_provider(local_provider()?, version_one()?, ProviderPlacement::Local)
            .audio_format(AudioFormat::new("wav/pcm_s16le", 16_000, 1))
            .chunk_boundary(ChunkBoundary::new(30 * SECOND, 2 * SECOND)?)
            .language_hints(academic_transcription::Support::Offered)
            .vocabulary_hints(academic_transcription::Support::Offered)
            .timestamp_semantics(academic_transcription::TimestampSemantics::WordAndSegment)
            .confidence_semantics(ConfidenceSemantics::PerToken)
            .diarization(academic_transcription::Support::Offered)
            .math_and_code(academic_transcription::Support::Offered)
            .declare()?,
    )
}

/// The five fixture segments, each with its verbatim text and its tokens.
///
/// These are the numbers every coverage oracle is written against, and they are
/// written **here** rather than read back off a report: an oracle whose expected
/// value comes out of the thing it checks agrees with itself.
///
/// | index | id | tokens |
/// |---|---|---|
/// | 0 | `raw_segment_0001` | 4 |
/// | 1 | `raw_segment_0002` | 5 |
/// | 2 | `raw_segment_0003` | 5 |
/// | 3 | `raw_segment_0004` | 6 |
/// | 4 | `raw_segment_0005` | 1 |
pub const SEGMENTS: [(&str, &str, &[&str]); 5] = [
    (
        "raw_segment_0001",
        "serializability is the goal",
        &["serializability", "is", "the", "goal"],
    ),
    (
        "raw_segment_0002",
        "is that two phase locking",
        &["is", "that", "two", "phase", "locking"],
    ),
    (
        "raw_segment_0003",
        "again serializability is the goal",
        &["again", "serializability", "is", "the", "goal"],
    ),
    (
        "raw_segment_0004",
        "let x equal alpha over beta",
        &["let", "x", "equal", "alpha", "over", "beta"],
    ),
    ("raw_segment_0005", "hmm", &["hmm"]),
];

/// How many tokens the whole fixture transcript has: 4 + 5 + 5 + 6 + 1.
pub const TOTAL_TOKENS: u64 = 21;

/// The raw confidence units each segment's tokens carry.
///
/// Segment 3 is the equation and segment 4 is the aside; both are given low
/// raw numbers so the calibration curve reads them below the configured
/// permille, and the first three are given high ones.
pub const SEGMENT_UNITS: [u32; 5] = [900, 880, 860, 400, 300];

/// One synthetic provider response over the five fixture segments.
pub fn response_body() -> String {
    let mut body = String::new();
    body.push_str(RESPONSE_BANNER);
    body.push('\n');
    for (index, (id, verbatim, words)) in SEGMENTS.iter().enumerate() {
        let start = index as u64 * 3 * SECOND;
        let end = start + 3 * SECOND;
        let chunks = if index == 0 { "0,1" } else { "1" };
        let speaker = if index == 1 {
            "student_unknown_2"
        } else {
            "instructor"
        };
        body.push_str(&format!("segment: {id} {start} {end} {speaker} {chunks}\n"));
        body.push_str(&format!("verbatim: {verbatim}\n"));
        for (position, word) in words.iter().enumerate() {
            let at = start + position as u64 * 100_000_000;
            body.push_str(&format!(
                "word: {at} {} {word}\n",
                SEGMENT_UNITS[index] - position as u32
            ));
        }
    }
    body
}

/// A provider that answers with a fixed body on the local route.
#[derive(Debug)]
pub struct MockLocalProvider {
    provider: ProviderId,
    model_version: ModelVersion,
    body: String,
}

impl MockLocalProvider {
    /// A provider answering `body`.
    pub fn answering(body: impl Into<String>) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            provider: local_provider()?,
            model_version: version_one()?,
            body: body.into(),
        })
    }
}

impl SttProvider for MockLocalProvider {
    fn transcribe(&self, request: &TranscriptionRequest<'_>) -> Option<ProviderResponse> {
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

/// The seven section 27.3 fields a caller supplies.
pub fn run_identity() -> Result<RunIdentity, Box<dyn Error>> {
    Ok(RunIdentity {
        id: ModelRunId::from_bytes([7_u8; 16]),
        purpose: purpose()?,
        prompt_template_hash: Digest32::of(b"academic-transcription-prompt/1"),
        redaction_policy_hash: Digest32::of(b"academic-dlp-rulepack/1"),
        output_artifact: ArtifactId::from_bytes([9_u8; 16]),
        started_at: INSIDE,
        cost: Cost::new(0, "KRW")?,
        transmission: None,
    })
}

/// One completed `P2-L3` run, held so its lineage can be borrowed.
///
/// `CompletedRun` owns its lineage and hands it out by shared reference, which
/// is `P2-L3`'s rule and not something to work around here.
#[derive(Debug)]
pub struct Transcribed {
    record: academic_transcription::RunRecord,
}

impl Transcribed {
    /// The transcript the run produced.
    pub fn lineage(&self) -> &TranscriptLineage {
        self.record
            .completed()
            .map(academic_transcription::CompletedRun::lineage)
            .unwrap_or_else(|| unreachable!("the fixture run completed"))
    }
}

/// Runs the real `P2-L3` pipeline over one manifest.
pub fn transcribe(manifest: &InputManifest) -> Result<Transcribed, Box<dyn Error>> {
    transcribe_body(manifest, &response_body())
}

/// The same, over an arbitrary response body.
pub fn transcribe_body(
    manifest: &InputManifest,
    body: &str,
) -> Result<Transcribed, Box<dyn Error>> {
    let mut registry = ContractRegistry::new();
    registry.declare(whole_contract()?)?;
    let policy = SttPolicy::new();
    let selection = ProviderSelection::of(local_provider()?, version_one()?, vec![]);
    let provider = MockLocalProvider::answering(body)?;
    let mut archive = RawResponseArchive::new();
    let record = academic_transcription::run(
        manifest,
        &registry,
        &policy,
        &selection,
        &provider,
        &mut archive,
        &run_identity()?,
    );
    match record.outcome() {
        RunOutcome::Completed(_) => Ok(Transcribed { record }),
        RunOutcome::Halted { stage, fault } => {
            Err(format!("the fixture pipeline halted at {stage:?}: {fault}").into())
        }
    }
}

/// A calibration registry that reads the fixture's raw units.
///
/// Two bins: raw units at or below 500 read as 400 permille, everything above
/// reads as 900. The configured threshold is 700, so the first three fixture
/// segments are above it and the last two are below.
pub fn calibration() -> Result<CalibrationRegistry, Box<dyn Error>> {
    let mut registry = CalibrationRegistry::new();
    registry.register(CalibrationDataset::new(
        CalibrationDatasetId::new("fixture-curve")?,
        local_provider()?,
        version_one()?,
        purpose()?,
        Digest32::of(b"fixture-calibration"),
        1_000,
        INSIDE,
        1_000_000,
        vec![
            CalibrationBin::new(500, 400)?,
            CalibrationBin::new(1_000, 900)?,
        ],
    )?)?;
    Ok(registry)
}

/// An empty calibration registry: nothing can be read, so everything with a
/// provider number is fail-closed into the queue.
pub fn no_calibration() -> CalibrationRegistry {
    CalibrationRegistry::new()
}

/// A document identifier.
pub fn document_id(tag: &str) -> Result<DocumentId, Box<dyn Error>> {
    Ok(DocumentId::new(tag)?)
}

/// A node identifier.
pub fn node_id(tag: &str) -> Result<NodeId, Box<dyn Error>> {
    Ok(NodeId::new(tag)?)
}

/// A paragraph draft mapping the whole of one segment under one transform.
pub fn whole_segment_node(
    id: &str,
    kind: NodeKind,
    segment_index: usize,
    transform: PreservationTransform,
) -> Result<NodeDraft, Box<dyn Error>> {
    let (_, verbatim, _) = SEGMENTS[segment_index];
    let chars = verbatim.chars().count();
    Ok(NodeDraft {
        id: node_id(id)?,
        kind,
        // A rendered text that adds punctuation and a speaker label: two of the
        // nine transforms, and both are insertions rather than deletions.
        rendered_text: format!("Instructor: {verbatim}."),
        mappings: vec![(segment_index, 0, chars, transform)],
        nearby_captures: Vec::new(),
        annotations: Vec::new(),
        cross_reference: None,
    })
}

/// The document every positive control uses: five nodes, one per segment, plus
/// a capture placement.
pub fn whole_document(
    lineage: &TranscriptLineage,
    manifest: &InputManifest,
    capture_seq: u32,
) -> Result<LectureDocument, Box<dyn Error>> {
    let mut builder = DocumentBuilder::over(document_id("lecture-06-doc")?, lineage, 1, manifest)?;
    builder.push(whole_segment_node(
        "n-01",
        NodeKind::Section,
        0,
        PreservationTransform::SectionHeading,
    )?)?;
    builder.push(whole_segment_node(
        "n-02",
        NodeKind::Paragraph,
        1,
        PreservationTransform::SpeakerLabel,
    )?)?;
    let mut repetition = whole_segment_node(
        "n-03",
        NodeKind::Paragraph,
        2,
        PreservationTransform::RepetitionAndEmphasisAnnotation,
    )?;
    repetition.annotations = vec![
        DocumentAnnotation::Repetition,
        DocumentAnnotation::Digression,
    ];
    builder.push(repetition)?;
    let mut equation = whole_segment_node(
        "n-04",
        NodeKind::Equation,
        3,
        PreservationTransform::MathAndCodeFormatting,
    )?;
    equation.annotations = vec![DocumentAnnotation::UnverifiedEquation];
    builder.push(equation)?;
    let mut aside = whole_segment_node(
        "n-05",
        NodeKind::Paragraph,
        4,
        PreservationTransform::Punctuation,
    )?;
    aside.annotations = vec![DocumentAnnotation::Example];
    builder.push(aside)?;
    let mut placement = whole_segment_node(
        "n-06",
        NodeKind::CapturePlacement,
        4,
        PreservationTransform::CapturePlacement,
    )?;
    placement.nearby_captures = vec![capture_seq];
    builder.push(placement)?;
    Ok(builder.finish()?)
}
