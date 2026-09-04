//! Synthetic fixtures for the `P2-L5` acceptance suite.
//!
//! **Nothing here records, transcribes or diarizes anything.** Every audio
//! "chunk" is a committed byte string, every image is another one, the provider
//! is a table lookup over literals in this file, and every diarization timeline
//! is written down. No microphone, no camera, no speech engine, no socket, no
//! clock.
//!
//! The `P2-L2` and `P2-L3` halves are `crates/lecture-document/tests/common/mod.rs`
//! restated rather than imported -- a test module is not a library target -- and
//! they are built from the same public APIs, so the transcript this suite
//! redacts is the one the real `academic_transcription::run` produced over a
//! journal the real `academic_capture::begin` wrote.
//!
//! # The canary
//!
//! Every student utterance in [`SEGMENTS`] carries [`STUDENT_CANARY`], and no
//! instructor utterance does. That is what makes "the derivative excludes the
//! targeted speakers" a byte search over everything the derivative can be
//! turned into rather than a walk over the field the code decided to fill.

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
use academic_lecture_document::{RedactionBasis, RedactionPolicyRef};
use academic_model_run::{
    ArtifactId, Cost, Digest32, ModelRunId, ModelVersion, ProviderId, Purpose,
};
use academic_student_voice::{
    DiarizationCase, DiarizationCorpus, RedactionPolicy, RedactionScope, SpeakerTargeting,
    VoiceSpan,
};
use academic_transcription::{
    AudioFormat, AuthorizationBinding, ChunkBoundary, ConfidenceSemantics, ContractDraft,
    ContractRegistry, InputManifest, ProviderContract, ProviderPlacement, ProviderResponse,
    ProviderSelection, RESPONSE_BANNER, RawResponseArchive, RunIdentity, RunOutcome, Speaker,
    SttPolicy, SttProvider, TranscriptLineage, TranscriptionRequest,
};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The instant the fixture term opens.
pub const TERM_FROM: u64 = 1_000_000;
/// The instant the fixture term closes, exclusive.
pub const TERM_TO: u64 = 2_000_000;
/// An instant inside the fixture term.
pub const INSIDE: u64 = 1_500_000;
/// One second, in nanoseconds.
pub const SECOND: u64 = 1_000_000_000;

/// The token every student utterance carries and no instructor utterance does.
pub const STUDENT_CANARY: &str = "zqxjcanary";

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

/// The permission the fixtures record under.
pub fn permission_id() -> Result<CapturePermissionId, Box<dyn Error>> {
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

/// A model run, the first automatic actor every refusal row uses.
pub fn model_actor() -> Result<Actor, Box<dyn Error>> {
    Ok(Actor::ModelRun {
        run_id: EntityId::from_str("01900000-0000-7000-8000-0000000000f2")?,
    })
}

/// A deterministic engine, the second automatic actor.
pub fn engine_actor() -> Actor {
    Actor::DeterministicEngine {
        name: "diarizer".to_owned(),
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

/// Every automatic actor, which is every arm of `Actor` but `User`.
pub fn automatic_actors() -> Result<Vec<Actor>, Box<dyn Error>> {
    Ok(vec![model_actor()?, engine_actor(), importer_actor()])
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

/// The permission's own two bounds: audio expires before the transcript does.
pub const PARENT_AUDIO_UNTIL: u64 = 1_600_000;
/// The transcript bound, which is later than the audio one.
pub const PARENT_TRANSCRIPT_UNTIL: u64 = 1_900_000;

/// The parent terms every derivative in this suite inherits from.
pub fn parent_terms() -> RetentionTerms {
    RetentionTerms::new(
        RetentionBound::Until(PARENT_AUDIO_UNTIL),
        RetentionBound::Until(PARENT_TRANSCRIPT_UNTIL),
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
                parent_terms(),
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

/// One finished capture: the recorder that made it, and what is on disk.
#[derive(Debug)]
pub struct Capture {
    /// The recorder `begin` returned.
    pub recorder: academic_capture::CaptureRecorder,
    /// What it wrote, read back off disk.
    pub recovery: JournalRecovery,
}

/// A capture with no hole above the threshold.
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
    let recovery = recorder.verify_on_disk()?;
    Ok(Capture { recorder, recovery })
}

/// A manifest holding every audio frame and the capture.
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
    Ok(manifest)
}

/// The provider identifier every fixture uses.
pub fn local_provider() -> Result<ProviderId, Box<dyn Error>> {
    Ok(ProviderId::new("mock-local-stt")?)
}

/// The model version every fixture declares.
pub fn version_one() -> Result<ModelVersion, Box<dyn Error>> {
    Ok(ModelVersion::new("v1.0.0")?)
}

/// The purpose every run is keyed by.
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

/// The six fixture segments: who spoke, the verbatim text, and the tokens.
///
/// Indexes 1 and 3 are students and index 4 is unattributed; each of those
/// three carries [`STUDENT_CANARY`] and none of the instructor's does. The
/// speaker spellings are section 12.4's own.
///
/// | index | speaker | canary |
/// |---|---|---|
/// | 0 | `instructor` | no |
/// | 1 | `student_unknown_1` | yes |
/// | 2 | `instructor` | no |
/// | 3 | `student_unknown_2` | yes |
/// | 4 | `unresolved` | yes |
/// | 5 | `instructor` | no |
pub const SEGMENTS: [(&str, &str, &str, &[&str]); 6] = [
    (
        "raw_segment_0001",
        "instructor",
        "serializability is the goal",
        &["serializability", "is", "the", "goal"],
    ),
    (
        "raw_segment_0002",
        "student_unknown_1",
        "zqxjcanary is two phase locking enough",
        &["zqxjcanary", "is", "two", "phase", "locking", "enough"],
    ),
    (
        "raw_segment_0003",
        "instructor",
        "not by itself it is not",
        &["not", "by", "itself", "it", "is", "not"],
    ),
    (
        "raw_segment_0004",
        "student_unknown_2",
        "zqxjcanary what about deadlock",
        &["zqxjcanary", "what", "about", "deadlock"],
    ),
    (
        "raw_segment_0005",
        "unresolved",
        "zqxjcanary inaudible from the back",
        &["zqxjcanary", "inaudible", "from", "the", "back"],
    ),
    (
        "raw_segment_0006",
        "instructor",
        "we will return to that next week",
        &["we", "will", "return", "to", "that", "next", "week"],
    ),
];

/// The indexes whose speaker is not the instructor.
pub const NON_INSTRUCTOR_INDEXES: [usize; 3] = [1, 3, 4];

/// The indexes whose speaker is the instructor.
pub const INSTRUCTOR_INDEXES: [usize; 3] = [0, 2, 5];

/// The raw confidence units each segment's tokens carry.
pub const SEGMENT_UNITS: [u32; 6] = [900, 880, 860, 840, 820, 800];

/// One synthetic provider response over the six fixture segments.
pub fn response_body() -> String {
    let mut body = String::new();
    body.push_str(RESPONSE_BANNER);
    body.push('\n');
    for (index, (id, speaker, verbatim, words)) in SEGMENTS.iter().enumerate() {
        let start = index as u64 * 3 * SECOND;
        let end = start + 3 * SECOND;
        let chunks = if index == 0 { "0,1" } else { "1" };
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

/// A second synthetic response over the same journal, with different words.
///
/// It exists so that "this grant is for another original" has two originals to
/// be about. Every student utterance still carries the canary.
pub fn other_response_body() -> String {
    let mut body = String::new();
    body.push_str(RESPONSE_BANNER);
    body.push('\n');
    for (index, (id, speaker, verbatim, words)) in SEGMENTS.iter().enumerate() {
        let start = index as u64 * 3 * SECOND;
        let end = start + 3 * SECOND;
        let chunks = if index == 0 { "0,1" } else { "1" };
        body.push_str(&format!("segment: {id} {start} {end} {speaker} {chunks}\n"));
        body.push_str(&format!("verbatim: {verbatim} again\n"));
        for (position, word) in words.iter().enumerate() {
            let at = start + position as u64 * 100_000_000;
            body.push_str(&format!(
                "word: {at} {} {word}\n",
                SEGMENT_UNITS[index] - position as u32
            ));
        }
        let at = start + words.len() as u64 * 100_000_000;
        body.push_str(&format!("word: {at} {} again\n", SEGMENT_UNITS[index]));
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

/// The shipped policy: every non-instructor voice, into a derivative only.
pub fn non_instructor_policy() -> Result<RedactionPolicy, Box<dyn Error>> {
    Ok(RedactionPolicy::published(
        1,
        RedactionBasis::PermissionCondition,
        SpeakerTargeting::NonInstructorVoices,
        RedactionScope::DerivativeOnly,
        user()?,
    )?)
}

/// A policy naming one student, which is what a rights request produces.
pub fn named_speaker_policy(speakers: Vec<Speaker>) -> Result<RedactionPolicy, Box<dyn Error>> {
    Ok(RedactionPolicy::published(
        1,
        RedactionBasis::RightsRequest,
        SpeakerTargeting::NamedSpeakers(speakers),
        RedactionScope::DerivativeOnly,
        user()?,
    )?)
}

/// The `P2-L4` reference that cites `policy`.
pub fn reference_to(policy: &RedactionPolicy) -> Result<RedactionPolicyRef, Box<dyn Error>> {
    Ok(RedactionPolicyRef::citing(
        policy.digest(),
        policy.basis(),
        user()?,
    )?)
}

/// A corpus every case of which the diarizer got exactly right.
///
/// Its measured accuracy is a thousand permille and its missed-student figure
/// is zero, so it clears the recorded default. It is a *measurement* rather
/// than a fabricated witness: the suite runs the same scorer over it.
pub fn perfect_corpus() -> Result<DiarizationCorpus, Box<dyn Error>> {
    let timeline = vec![
        VoiceSpan::new(0, 30_000, Speaker::Instructor),
        VoiceSpan::new(30_000, 40_000, Speaker::StudentUnknown(1)),
        VoiceSpan::new(40_000, 90_000, Speaker::Instructor),
    ];
    Ok(DiarizationCorpus::new(
        "student-voice-diarization-perfect",
        1,
        vec![DiarizationCase::new(
            "everything_agrees",
            timeline.clone(),
            timeline,
        )?],
    )?)
}

/// A corpus whose diarizer mislabels a third of the student speech.
///
/// It clears no legal threshold on the missed-student axis, and its accuracy is
/// below the floor as well, so it is what the two-axis refusal is driven with.
pub fn poor_corpus() -> Result<DiarizationCorpus, Box<dyn Error>> {
    Ok(DiarizationCorpus::new(
        "student-voice-diarization-poor",
        1,
        vec![DiarizationCase::new(
            "half_the_question_is_missed",
            vec![
                VoiceSpan::new(0, 50_000, Speaker::Instructor),
                VoiceSpan::new(50_000, 100_000, Speaker::StudentUnknown(1)),
            ],
            vec![
                VoiceSpan::new(0, 75_000, Speaker::Instructor),
                VoiceSpan::new(75_000, 100_000, Speaker::StudentUnknown(1)),
            ],
        )?],
    )?)
}
