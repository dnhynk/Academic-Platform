//! `P2-L5`'s seven acceptance rows.
//!
//! Every fixture is synthetic. The transcript each redaction runs over comes
//! out of the real `academic_transcription::run` over a journal the real
//! `academic_capture::begin` wrote, so the utterances being excluded are ones
//! the pipeline actually produced rather than records written here.

mod common;

use std::collections::BTreeSet;

use academic_consent::{
    ConsentEventKind, DERIVATIVE_CLASSES, DerivativeClass, RetentionBound, RetentionTerms,
    SubjectInventory,
};
use academic_domain::ContentDigest;
use academic_student_voice::{
    ABSOLUTE_ACCURACY_FLOOR, ABSOLUTE_MISSED_STUDENT_CEILING, AccessRefusal, AccuracyRefusal,
    AffectedProjectionKind, CORPUS_ID, CORPUS_VERSION, CaptureUnderReview, CorpusFault,
    DIARIZATION_THRESHOLD_V1, DeletionFault, DerivedArtifact, DiarizationCase, DiarizationCorpus,
    DiarizationThreshold, EvidenceIndex, HoldRefusal, HoldState, IngestionJobKind, IngestionStage,
    LectureDeletionPlan, LectureSource, ManualExclusion, ORIGINAL_CLASSIFICATION, PiiClass,
    PiiFinding, ProjectionEffect, ProjectionRecord, RawAccessGrant, RawAccessLog, RedactionFault,
    RedactionMode, RedactionPlan, RedactionScope, ReviewDecision, ReviewOutcome, ReviewedCapture,
    SpeakerTargeting, ThresholdFault, VoiceSpan, apply_deletion, corpus_v1, dispatch,
    inherit_terms, measure, preview_deletion, redact,
};
use academic_transcription::Speaker;

use common::{
    INSIDE, INSTRUCTOR_INDEXES, NON_INSTRUCTOR_INDEXES, PARENT_AUDIO_UNTIL,
    PARENT_TRANSCRIPT_UNTIL, SEGMENTS, STUDENT_CANARY, TestResult, automatic_actors, clean_capture,
    full_manifest, named_speaker_policy, non_instructor_policy, parent_terms, perfect_corpus,
    permission_id, poor_corpus, reference_to, term, transcribe, user,
};

/// The number the contract page publishes, measured rather than quoted.
///
/// It is written here as a literal and compared against a fresh run, so a
/// change to the corpus or to the scorer fails this row rather than moving the
/// published figure silently. The oracle is **not** read out of the
/// measurement: `P2-L3` shipped one that was, and it agreed with itself.
const SHIPPED_ACCURACY_PERMILLE: u64 = 967;
/// The fraction of student speech the shipped corpus's diarizer left in.
const SHIPPED_MISSED_STUDENT_PERMILLE: u64 = 33;
/// The fraction of student speech it also called student.
const SHIPPED_STUDENT_RECALL_PERMILLE: u64 = 766;
/// Milliseconds of reference time in the shipped corpus.
const SHIPPED_SCORED_MS: u64 = 550_000;
/// Student milliseconds in it.
const SHIPPED_STUDENT_MS: u64 = 60_000;

// ---------------------------------------------------------------------------
// 1. diarization_accuracy_is_measured_and_versioned
// ---------------------------------------------------------------------------

/// The accuracy figure is a run over a named, versioned, digested corpus.
///
/// Four things, and the fourth is the one that makes the first three mean
/// something:
///
/// 1. the number is what the scorer produced, compared against literals written
///    in this file rather than read back off the measurement;
/// 2. the measurement carries the corpus identity and the corpus digest, so it
///    names what it was taken on;
/// 3. a corpus edited by one millisecond has a different digest and a different
///    number, so the binding is not decorative; and
/// 4. every millisecond of every case lands in exactly one bucket, so a scorer
///    that double-counted an overlap or dropped a hole fails rather than
///    reporting a slightly wrong ratio.
#[test]
fn diarization_accuracy_is_measured_and_versioned() -> TestResult {
    let corpus = corpus_v1()?;
    let measurement = measure(&corpus);

    // 1. the measured numbers.
    assert_eq!(measurement.accuracy_permille(), SHIPPED_ACCURACY_PERMILLE);
    assert_eq!(
        measurement.missed_student_permille(),
        SHIPPED_MISSED_STUDENT_PERMILLE
    );
    assert_eq!(
        measurement.student_recall_permille(),
        SHIPPED_STUDENT_RECALL_PERMILLE
    );
    assert_eq!(measurement.scored_ms(), SHIPPED_SCORED_MS);
    assert_eq!(measurement.reference_student_ms(), SHIPPED_STUDENT_MS);
    assert_eq!(measurement.cases().len(), corpus.cases().len());

    // 2. the identity it carries.
    assert_eq!(measurement.corpus_id(), CORPUS_ID);
    assert_eq!(measurement.corpus_version(), CORPUS_VERSION);
    assert_eq!(*measurement.corpus_digest(), corpus.digest());
    assert_eq!(
        measurement.scorer_version(),
        academic_student_voice::SCORER_VERSION
    );

    // 3. one millisecond moves both the digest and the number.
    let mut cases = Vec::new();
    for (index, case) in corpus.cases().iter().enumerate() {
        let mut hypothesis = case.hypothesis().to_vec();
        if index == 0 {
            let first = hypothesis[0];
            hypothesis[0] = VoiceSpan::new(first.start_ms(), first.end_ms() - 1, first.speaker());
        }
        cases.push(DiarizationCase::new(
            case.name(),
            case.reference().to_vec(),
            hypothesis,
        )?);
    }
    let nudged = DiarizationCorpus::new(CORPUS_ID, CORPUS_VERSION, cases)?;
    assert_ne!(nudged.digest(), corpus.digest());
    let nudged_measurement = measure(&nudged);
    assert_ne!(nudged_measurement.agreed_ms(), measurement.agreed_ms());
    assert_eq!(*nudged_measurement.corpus_digest(), nudged.digest());

    // 4. the partition, per case and over the fold.
    assert!(measurement.partition_reconciles());
    for case in measurement.cases() {
        assert!(
            case.partition_reconciles(),
            "{} does not partition its scored time",
            case.case()
        );
    }
    let folded: u64 = measurement
        .cases()
        .iter()
        .map(academic_student_voice::CaseMeasurement::scored_ms)
        .sum();
    assert_eq!(folded, measurement.scored_ms());

    // A corpus that cannot measure the privacy axis is not a corpus.
    let no_students = DiarizationCorpus::new(
        "instructor-only",
        1,
        vec![DiarizationCase::new(
            "only_the_instructor",
            vec![VoiceSpan::new(0, 10_000, Speaker::Instructor)],
            vec![VoiceSpan::new(0, 10_000, Speaker::Instructor)],
        )?],
    );
    assert_eq!(no_students, Err(CorpusFault::NoStudentSpeech));

    // A ground truth that declines to attribute would agree with everything.
    let unresolved_reference = DiarizationCase::new(
        "unlabelled_truth",
        vec![VoiceSpan::new(0, 10_000, Speaker::Unresolved)],
        vec![VoiceSpan::new(0, 10_000, Speaker::Instructor)],
    );
    assert_eq!(
        unresolved_reference,
        Err(CorpusFault::UnresolvedInReference {
            case: "unlabelled_truth".to_owned()
        })
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. below_threshold_diarization_blocks_automatic_redaction
// ---------------------------------------------------------------------------

/// A measurement below the configured threshold produces no witness, and
/// without a witness there is no automatic redaction claim to make.
///
/// The type-level half is a `compile_fail` case: `RedactionMode::Automatic`
/// takes an `AccuracyWitness` by value and a witness has no public
/// constructor. This row is the behavioural half, and it drives **both** axes
/// independently, the pass arm, the floor a configuration may not go below, and
/// what a below-threshold profile is left with.
#[test]
fn below_threshold_diarization_blocks_automatic_redaction() -> TestResult {
    let corpus = corpus_v1()?;
    let measurement = measure(&corpus);

    // The recorded default refuses the shipped corpus on the accuracy axis.
    assert_eq!(
        measurement.witness(DIARIZATION_THRESHOLD_V1),
        Err(AccuracyRefusal::AccuracyBelowThreshold {
            measured: SHIPPED_ACCURACY_PERMILLE,
            required: DIARIZATION_THRESHOLD_V1.min_accuracy_permille(),
        })
    );

    // A configuration that clears the accuracy axis still fails the privacy
    // one. The two are separate failures and this is the one that matters.
    let accuracy_only = DiarizationThreshold::new(2, 960, 0)?;
    assert_eq!(
        measurement.witness(accuracy_only),
        Err(AccuracyRefusal::MissedStudentSpeechAboveThreshold {
            measured: SHIPPED_MISSED_STUDENT_PERMILLE,
            allowed: 0,
        })
    );

    // A configuration inside the legal band that both axes clear produces a
    // witness, and the witness carries the numbers and the configuration it
    // cleared so a weak profile is visible on the claim.
    let permissive = DiarizationThreshold::new(3, 960, 40)?;
    let witness = measurement.witness(permissive)?;
    assert_eq!(witness.accuracy_permille(), SHIPPED_ACCURACY_PERMILLE);
    assert_eq!(
        witness.missed_student_permille(),
        SHIPPED_MISSED_STUDENT_PERMILLE
    );
    assert_eq!(witness.threshold(), permissive);
    assert_eq!(witness.corpus_id(), CORPUS_ID);
    assert_eq!(*witness.corpus_digest(), corpus.digest());

    // A corpus a diarizer got right clears the recorded default.
    let perfect = measure(&perfect_corpus()?);
    assert_eq!(perfect.accuracy_permille(), 1000);
    assert_eq!(perfect.missed_student_permille(), 0);
    let default_witness = perfect.witness(DIARIZATION_THRESHOLD_V1)?;
    assert_eq!(default_witness.threshold(), DIARIZATION_THRESHOLD_V1);

    // A bad diarizer clears nothing a configuration is allowed to state.
    let poor = measure(&poor_corpus()?);
    let weakest =
        DiarizationThreshold::new(4, ABSOLUTE_ACCURACY_FLOOR, ABSOLUTE_MISSED_STUDENT_CEILING)?;
    assert!(poor.witness(weakest).is_err());

    // Configuration cannot empty the guard. Both bounds bind, and a permille
    // above one thousand is not a permille.
    assert_eq!(
        DiarizationThreshold::new(5, ABSOLUTE_ACCURACY_FLOOR - 1, 0),
        Err(ThresholdFault::AccuracyFloorIsBinding {
            stated: ABSOLUTE_ACCURACY_FLOOR - 1,
            floor: ABSOLUTE_ACCURACY_FLOOR,
        })
    );
    assert_eq!(
        DiarizationThreshold::new(6, 990, ABSOLUTE_MISSED_STUDENT_CEILING + 1),
        Err(ThresholdFault::MissedStudentCeilingIsBinding {
            stated: ABSOLUTE_MISSED_STUDENT_CEILING + 1,
            ceiling: ABSOLUTE_MISSED_STUDENT_CEILING,
        })
    );
    assert_eq!(
        DiarizationThreshold::new(7, 1001, 0),
        Err(ThresholdFault::AccuracyIsNotAPermille { stated: 1001 })
    );

    // What a below-threshold profile is left with: a manual plan, whose every
    // exclusion a person decided, and which is not an automatic claim.
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "below-threshold")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let policy = non_instructor_policy()?;
    let reference = reference_to(&policy)?;
    let source = LectureSource::of(lineage, 1, parent_terms())?;

    let mut decisions = Vec::new();
    for index in NON_INSTRUCTOR_INDEXES {
        decisions.push(ManualExclusion::decided(index, user()?)?);
    }
    let manual = RedactionPlan::manual(policy.clone(), decisions)?;
    assert_eq!(manual.mode(), &RedactionMode::Manual);
    assert!(manual.mode().witness().is_none());
    let redaction = redact(&manual, &reference, &source, parent_terms())?;
    assert_eq!(
        redaction.derivative().excluded().len(),
        NON_INSTRUCTOR_INDEXES.len()
    );
    assert_eq!(redaction.derivative().mode().as_str(), "MANUAL");

    // A manual plan that excludes nothing is refused rather than being a
    // redaction that redacts nothing.
    assert_eq!(
        RedactionPlan::manual(policy.clone(), Vec::new()),
        Err(RedactionFault::NothingExcluded)
    );

    // And an automatic actor cannot decide one.
    for actor in automatic_actors()? {
        assert_eq!(
            ManualExclusion::decided(1, actor),
            Err(RedactionFault::AutomaticActorCannotRedact)
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. redacted_derivative_excludes_targeted_speakers
// ---------------------------------------------------------------------------

/// Nothing a targeted speaker said survives into the derivative, in any form.
///
/// The walk over the kept list is the weak half: it reads the field the code
/// decided to fill. The canary is the strong half -- every non-instructor
/// utterance in the fixture carries a token no instructor utterance does, and
/// the assertion is that the token appears in **nothing** the derivative can be
/// turned into: not its canonical bytes, not its `Debug`, not any accessor.
///
/// The control is the named-speaker policy, under which one student is targeted
/// and the others are not: the canary is then present, which is what says the
/// exclusion follows the targeting rather than the token.
#[test]
fn redacted_derivative_excludes_targeted_speakers() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "excludes")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let source = LectureSource::of(lineage, 1, parent_terms())?;
    assert_eq!(source.utterances().len(), SEGMENTS.len());

    let policy = non_instructor_policy()?;
    let reference = reference_to(&policy)?;
    let witness = measure(&perfect_corpus()?).witness(DIARIZATION_THRESHOLD_V1)?;
    let plan = RedactionPlan::automatic(policy.clone(), witness);
    let redaction = redact(&plan, &reference, &source, parent_terms())?;
    let derivative = redaction.derivative();

    // Every kept utterance is the instructor's, and every targeted one is gone.
    let kept: Vec<usize> = derivative.kept().iter().map(|kept| kept.index()).collect();
    let excluded: Vec<usize> = derivative
        .excluded()
        .iter()
        .map(|record| record.index())
        .collect();
    assert_eq!(kept, INSTRUCTOR_INDEXES.to_vec());
    assert_eq!(excluded, NON_INSTRUCTOR_INDEXES.to_vec());
    assert!(!derivative.keeps_a_targeted_speaker(&policy));
    for utterance in derivative.kept() {
        assert_eq!(utterance.speaker(), Speaker::Instructor);
    }

    // The canary appears in nothing the derivative can be turned into.
    let bytes = derivative.canonical_bytes();
    let rendered = String::from_utf8(bytes.clone())?;
    assert!(
        !rendered.contains(STUDENT_CANARY),
        "the canary reached the derivative's canonical bytes"
    );
    assert!(
        !format!("{derivative:?}").contains(STUDENT_CANARY),
        "the canary reached the derivative's Debug"
    );
    for utterance in derivative.kept() {
        assert!(!utterance.text().contains(STUDENT_CANARY));
    }
    // And it is in the source, so the search is looking for something that is
    // there to find.
    assert!(
        source
            .utterances()
            .iter()
            .any(|utterance| utterance.verbatim().contains(STUDENT_CANARY)),
        "the fixture has no canary to exclude"
    );

    // What the exclusion records carry is the span and the speaker; the
    // duration is non-zero, so a reader can see how much was removed.
    for record in derivative.excluded() {
        assert!(record.duration_nanos() > 0);
        assert!(policy.targets(record.speaker()));
    }

    // The control: a policy naming one student leaves the others in, canary
    // and all.
    let named = named_speaker_policy(vec![Speaker::StudentUnknown(1)])?;
    let named_reference = reference_to(&named)?;
    let named_witness = measure(&perfect_corpus()?).witness(DIARIZATION_THRESHOLD_V1)?;
    let named_plan = RedactionPlan::automatic(named.clone(), named_witness);
    let named_redaction = redact(&named_plan, &named_reference, &source, parent_terms())?;
    let named_derivative = named_redaction.derivative();
    assert_eq!(
        named_derivative
            .excluded()
            .iter()
            .map(|record| record.index())
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert!(
        String::from_utf8(named_derivative.canonical_bytes())?.contains(STUDENT_CANARY),
        "the control did not keep the untargeted students"
    );
    assert!(!named_derivative.keeps_a_targeted_speaker(&named));

    // A manual exclusion naming an utterance the policy does not target is
    // refused, so the manual arm cannot remove the instructor either.
    let manual = RedactionPlan::manual(
        policy.clone(),
        vec![ManualExclusion::decided(INSTRUCTOR_INDEXES[0], user()?)?],
    )?;
    assert_eq!(
        redact(&manual, &reference, &source, parent_terms()),
        Err(RedactionFault::SegmentIsNotTargeted {
            index: INSTRUCTOR_INDEXES[0]
        })
    );

    // A reference citing another policy does not resolve, which is `P2-L4`'s
    // `D-3` closed: the digest that crate holds is checked here.
    let other = named_speaker_policy(vec![Speaker::StudentUnknown(2)])?;
    let other_reference = reference_to(&other)?;
    assert!(matches!(
        redact(&plan, &other_reference, &source, parent_terms()),
        Err(RedactionFault::PolicyReferenceDoesNotResolve { .. })
    ));
    assert!(policy.resolves(&reference));
    assert!(!policy.resolves(&other_reference));

    // `GATE-38-026`: there is one scope and it is the derivative.
    assert_eq!(RedactionScope::ALL.len(), 1);
    assert_eq!(RedactionScope::ALL[0], RedactionScope::DerivativeOnly);
    assert_eq!(policy.scope(), RedactionScope::DerivativeOnly);

    // An empty target list is refused rather than being a policy that targets
    // nobody.
    assert_eq!(
        academic_student_voice::RedactionPolicy::published(
            1,
            academic_lecture_document::RedactionBasis::RightsRequest,
            SpeakerTargeting::NamedSpeakers(Vec::new()),
            RedactionScope::DerivativeOnly,
            user()?,
        ),
        Err(RedactionFault::NoTargets)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. raw_remains_restricted_under_authorized_access
// ---------------------------------------------------------------------------

/// The original keeps what the derivative removed, and an authorized read does
/// not lift the restriction.
///
/// `REQ-12-031` is two sentences and both are here: the tagged utterance is
/// absent from the derivative and present under an authorized raw access. What
/// is added is the four things that make "restricted" more than a label -- the
/// grant is a person's, it is bound to one original, it is spent by being used,
/// and using it writes an audit row -- and the observation that afterwards the
/// original, the derivative and the classification are exactly as they were.
#[test]
fn raw_remains_restricted_under_authorized_access() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "restricted")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let source = LectureSource::of(lineage, 1, parent_terms())?;
    let policy = non_instructor_policy()?;
    let reference = reference_to(&policy)?;
    let witness = measure(&perfect_corpus()?).witness(DIARIZATION_THRESHOLD_V1)?;
    let plan = RedactionPlan::automatic(policy, witness);
    let redaction = redact(&plan, &reference, &source, parent_terms())?;
    let original = redaction.original();
    let derivative_before = redaction.derivative().canonical_bytes();

    // What a reader with no grant sees: that something was removed, and how
    // much. Not what.
    assert_eq!(original.removed_count(), NON_INSTRUCTOR_INDEXES.len());
    assert_eq!(original.classification(), ORIGINAL_CLASSIFICATION);
    assert_eq!(original.classification(), "RESTRICTED");
    assert!(
        !format!("{original:?}").contains(STUDENT_CANARY),
        "the canary reached the restricted original's Debug"
    );

    // The original's retention is the permission's, not the derivative's.
    assert_eq!(original.terms(), parent_terms());

    // No automatic actor can be issued a grant.
    for actor in automatic_actors()? {
        assert_eq!(
            RawAccessGrant::issued(original, actor, "audit", INSIDE).err(),
            Some(AccessRefusal::AutomaticActorCannotOpen)
        );
    }

    // A grant issued for another original does not open this one. The other
    // original is a different lecture in words -- a second run of the real
    // pipeline over a different response body -- so its digest differs, which
    // is what the refusal reads.
    let other_transcribed = common::transcribe_body(&manifest, &common::other_response_body())?;
    let other_source = LectureSource::of(other_transcribed.lineage(), 1, parent_terms())?;
    let other = redact(&plan, &reference, &other_source, parent_terms())?;
    assert_ne!(other.original().digest(), original.digest());
    let mut log = RawAccessLog::new();
    let foreign = RawAccessGrant::issued(other.original(), user()?, "audit", INSIDE)?;
    assert_eq!(
        original.open(foreign, &mut log).err(),
        Some(AccessRefusal::GrantIsForAnotherOriginal)
    );
    assert!(
        log.entries().is_empty(),
        "a refused access wrote an audit row"
    );

    // An authorized read reaches the removed speech, and writes one row.
    let grant = RawAccessGrant::issued(original, user()?, "rights-request-audit", INSIDE)?;
    assert_eq!(grant.purpose(), "rights-request-audit");
    let disclosure = original.open(grant, &mut log)?;
    assert_eq!(disclosure.len(), NON_INSTRUCTOR_INDEXES.len());
    assert!(!disclosure.is_empty());
    let mut disclosed_canaries = 0;
    for position in 0..disclosure.len() {
        let text = disclosure
            .verbatim(position)
            .ok_or("a disclosed utterance has no text")?;
        if text.contains(STUDENT_CANARY) {
            disclosed_canaries += 1;
        }
        let index = disclosure
            .source_index(position)
            .ok_or("a disclosed utterance has no index")?;
        assert!(NON_INSTRUCTOR_INDEXES.contains(&index));
        assert!(disclosure.speaker(position).is_some());
    }
    assert_eq!(disclosed_canaries, NON_INSTRUCTOR_INDEXES.len());

    assert_eq!(log.entries().len(), 1);
    let row = &log.entries()[0];
    assert_eq!(row.opened_by(), &user()?);
    assert_eq!(row.purpose(), "rights-request-audit");
    assert_eq!(row.at(), INSIDE);
    assert_eq!(row.utterances_disclosed(), NON_INSTRUCTOR_INDEXES.len());
    assert_eq!(row.original_digest(), original.digest());

    // The restriction did not move. The classification, the removed count, the
    // terms and the derivative are what they were, and the canary is still
    // absent from the derivative.
    assert_eq!(original.classification(), ORIGINAL_CLASSIFICATION);
    assert_eq!(original.removed_count(), NON_INSTRUCTOR_INDEXES.len());
    assert_eq!(original.terms(), parent_terms());
    assert_eq!(redaction.derivative().canonical_bytes(), derivative_before);
    assert!(!String::from_utf8(derivative_before)?.contains(STUDENT_CANARY));

    // A second read is a second grant and a second row. The first grant is
    // gone: `open` took it by value, which the `compile_fail` case observes.
    let again = RawAccessGrant::issued(original, user()?, "second-look", INSIDE + 1)?;
    let second = original.open(again, &mut log)?;
    assert_eq!(second.len(), NON_INSTRUCTOR_INDEXES.len());
    assert_eq!(log.entries().len(), 2);
    assert_eq!(log.entries()[1].purpose(), "second-look");
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. capture_pii_hold_blocks_downstream_jobs
// ---------------------------------------------------------------------------

/// A counting stage. It is the whole point of this row: the assertion is that
/// it is **not called**.
#[derive(Debug, Default)]
struct CountingStage {
    calls: usize,
    bytes_seen: usize,
    kinds: Vec<IngestionJobKind>,
}

impl IngestionStage for CountingStage {
    fn ingest(&mut self, capture: &ReviewedCapture<'_>) {
        self.calls += 1;
        self.bytes_seen += capture.bytes().len();
        self.kinds.push(capture.kind());
    }
}

/// A capture with a student's face, a roster or a personal screen in it does
/// not reach graph or OCR ingestion until a person has reviewed it.
///
/// The spy is what makes this more than a flag: on every held arm the real
/// dispatch runs against a real stage and the stage's call count stays zero.
#[test]
fn capture_pii_hold_blocks_downstream_jobs() -> TestResult {
    let bytes = academic_capture::CaptureBytes::of(common::image("lecture-room"));
    let digest = bytes.digest();

    // Every class, alone and together, against both jobs.
    for class in PiiClass::ALL {
        let capture =
            CaptureUnderReview::screened(bytes.clone(), vec![PiiFinding::found(class, user()?)]);
        assert_eq!(capture.hold_state(), HoldState::Held(vec![class]));
        assert!(capture.hold_state().is_held());
        for kind in IngestionJobKind::ALL {
            let mut stage = CountingStage::default();
            let refusal = dispatch(&mut stage, kind, &capture);
            assert_eq!(
                refusal,
                Err(HoldRefusal::HeldPendingReview {
                    classes: vec![class.as_str()],
                })
            );
            assert_eq!(stage.calls, 0, "{kind:?} ran over a held capture");
            assert_eq!(stage.bytes_seen, 0);
        }
    }

    let mut all_three = Vec::new();
    for class in PiiClass::ALL {
        all_three.push(PiiFinding::found(class, user()?));
    }
    let mut capture = CaptureUnderReview::screened(bytes.clone(), all_three.clone());
    assert_eq!(
        capture.hold_state(),
        HoldState::Held(PiiClass::ALL.to_vec())
    );
    assert_eq!(capture.byte_len(), bytes.len());
    assert_eq!(capture.digest(), &digest);
    assert!(capture.review().is_none());

    let mut stage = CountingStage::default();
    assert!(dispatch(&mut stage, IngestionJobKind::GraphIngestion, &capture).is_err());
    assert_eq!(stage.calls, 0);

    // A review that leaves findings unaddressed is not a review.
    assert_eq!(
        capture.record_review(ReviewDecision::recorded(
            digest,
            vec![PiiClass::StudentFace],
            ReviewOutcome::Release,
            user()?,
            INSIDE,
        )?),
        Err(HoldRefusal::ReviewIsIncomplete { count: 2 })
    );

    // A review of another capture is not this capture's.
    assert_eq!(
        capture.record_review(ReviewDecision::recorded(
            ContentDigest::sha256(b"another-capture"),
            PiiClass::ALL.to_vec(),
            ReviewOutcome::Release,
            user()?,
            INSIDE,
        )?),
        Err(HoldRefusal::ReviewIsForAnotherCapture)
    );

    // No automatic actor can review one.
    for actor in automatic_actors()? {
        assert_eq!(
            ReviewDecision::recorded(
                digest,
                PiiClass::ALL.to_vec(),
                ReviewOutcome::Release,
                actor,
                INSIDE,
            ),
            Err(HoldRefusal::AutomaticActorCannotReview)
        );
    }

    // A complete review that withholds still blocks, and the stage still does
    // not run.
    capture.record_review(ReviewDecision::recorded(
        digest,
        PiiClass::ALL.to_vec(),
        ReviewOutcome::Withhold,
        user()?,
        INSIDE,
    )?)?;
    for kind in IngestionJobKind::ALL {
        let mut stage = CountingStage::default();
        assert_eq!(
            dispatch(&mut stage, kind, &capture),
            Err(HoldRefusal::ReviewWithheld)
        );
        assert_eq!(stage.calls, 0);
    }

    // A complete review that releases admits, and the stage sees the bytes.
    let mut released = CaptureUnderReview::screened(bytes.clone(), all_three);
    released.record_review(ReviewDecision::recorded(
        digest,
        PiiClass::ALL.to_vec(),
        ReviewOutcome::Release,
        user()?,
        INSIDE,
    )?)?;
    let mut stage = CountingStage::default();
    for kind in IngestionJobKind::ALL {
        let receipt = dispatch(&mut stage, kind, &released)?;
        assert_eq!(receipt.kind(), kind);
        assert_eq!(receipt.digest(), &digest);
    }
    assert_eq!(stage.calls, 2);
    assert_eq!(stage.kinds, IngestionJobKind::ALL.to_vec());
    assert_eq!(stage.bytes_seen, bytes.len() * 2);

    // A capture nothing was found in needs no review.
    let clear = CaptureUnderReview::screened(bytes.clone(), Vec::new());
    assert_eq!(clear.hold_state(), HoldState::Clear);
    assert!(!clear.hold_state().is_held());
    let mut stage = CountingStage::default();
    dispatch(&mut stage, IngestionJobKind::OcrIngestion, &clear)?;
    assert_eq!(stage.calls, 1);

    // Two findings of one class are one reason, not two.
    let doubled = CaptureUnderReview::screened(
        bytes,
        vec![
            PiiFinding::found(PiiClass::Roster, user()?),
            PiiFinding::found(PiiClass::Roster, user()?),
        ],
    );
    assert_eq!(
        doubled.hold_state(),
        HoldState::Held(vec![PiiClass::Roster])
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. derivative_expiry_is_equal_or_stricter
// ---------------------------------------------------------------------------

/// The bound grid every retention assertion walks.
fn bound_grid() -> [RetentionBound; 4] {
    [
        RetentionBound::Prohibited,
        RetentionBound::Until(1),
        RetentionBound::Until(PARENT_AUDIO_UNTIL),
        RetentionBound::Until(u64::MAX),
    ]
}

/// No derivative of a lecture is wider than what it came from, on either axis,
/// at any depth.
///
/// This does not check a case. It walks the whole cross product of a bound grid
/// -- 256 `(parent, requested)` pairs -- through the real redaction, so a `max`
/// in place of the `min` fails on the first pair where the two differ. Then it
/// walks a three-link chain requesting the widest possible terms at every link
/// and requires the result to be no wider than the root.
#[test]
fn derivative_expiry_is_equal_or_stricter() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "retention")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let policy = non_instructor_policy()?;
    let reference = reference_to(&policy)?;
    let witness = measure(&perfect_corpus()?).witness(DIARIZATION_THRESHOLD_V1)?;
    let plan = RedactionPlan::automatic(policy, witness);

    let mut pairs = 0_usize;
    let mut strictly_narrower = 0_usize;
    for parent_audio in bound_grid() {
        for parent_transcript in bound_grid() {
            let parent = RetentionTerms::new(parent_audio, parent_transcript);
            let source = LectureSource::of(lineage, 1, parent)?;
            for requested_audio in bound_grid() {
                for requested_transcript in bound_grid() {
                    let requested = RetentionTerms::new(requested_audio, requested_transcript);
                    let redaction = redact(&plan, &reference, &source, requested)?;
                    let derived = redaction.derivative().terms();
                    assert!(
                        derived.is_no_wider_than(parent),
                        "a derivative of {parent:?} asking for {requested:?} got {derived:?}"
                    );
                    assert!(derived.is_no_wider_than(requested));
                    assert_eq!(derived, inherit_terms(parent, requested));
                    // The original keeps the permission's own terms; redacting
                    // a copy is not a reason to move them.
                    assert_eq!(redaction.original().terms(), parent);
                    if derived != parent {
                        strictly_narrower += 1;
                    }
                    pairs += 1;
                }
            }
        }
    }
    assert_eq!(pairs, 256);
    assert!(
        strictly_narrower > 0,
        "no pair in the grid narrowed, so the comparison was never exercised"
    );

    // A chain: derivative, a transcript of it, an embedding of that. Every link
    // asks for the widest terms there are, and none of them gets them.
    let widest = RetentionTerms::new(
        RetentionBound::Until(u64::MAX),
        RetentionBound::Until(u64::MAX),
    );
    for parent_audio in bound_grid() {
        for parent_transcript in bound_grid() {
            let parent = RetentionTerms::new(parent_audio, parent_transcript);
            let source = LectureSource::of(lineage, 1, parent)?;
            let redaction = redact(&plan, &reference, &source, widest)?;
            let derivative = redaction.derivative();
            let transcript =
                DerivedArtifact::of_derivative(derivative, DerivativeClass::Transcript, widest);
            let embedding =
                DerivedArtifact::of_artifact(&transcript, DerivativeClass::Embedding, widest);
            assert!(derivative.terms().is_no_wider_than(parent));
            assert!(transcript.terms().is_no_wider_than(derivative.terms()));
            assert!(embedding.terms().is_no_wider_than(transcript.terms()));
            assert!(embedding.terms().is_no_wider_than(parent));
            assert_eq!(transcript.parent_digest(), &derivative.digest());
            assert_eq!(embedding.parent_digest(), &transcript.digest());
        }
    }

    // The two axes are independent: a parent whose audio is prohibited and
    // whose transcript runs to the end of term produces a derivative that says
    // exactly that.
    let split = RetentionTerms::new(
        RetentionBound::Prohibited,
        RetentionBound::Until(PARENT_TRANSCRIPT_UNTIL),
    );
    let source = LectureSource::of(lineage, 1, split)?;
    let redaction = redact(&plan, &reference, &source, widest)?;
    assert_eq!(
        redaction.derivative().terms().audio(),
        RetentionBound::Prohibited
    );
    assert_eq!(
        redaction.derivative().terms().transcript(),
        RetentionBound::Until(PARENT_TRANSCRIPT_UNTIL)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. deletion_impact_preview_lists_affected_projections
// ---------------------------------------------------------------------------

fn object(tag: &str) -> ContentDigest {
    ContentDigest::sha256(tag.as_bytes())
}

/// Before anything is deleted, the preview names every concept and evidence
/// projection the deletion reaches, and accounts for every object it does not.
///
/// The class-level half is `P2-G6`'s, called rather than forked, so the
/// derivative walk and the ledger row are the same ones that crate writes. What
/// is added is the projection layer, and its guard is the partition: every
/// deleted object is cited by a listed projection or is listed as unreferenced,
/// never both and never neither.
#[test]
fn deletion_impact_preview_lists_affected_projections() -> TestResult {
    let mut ledger = common::ledger_permitting()?;
    let audio = object("audio-object");
    let transcript = object("transcript-object");
    let embedding = object("embedding-object");
    let orphan = object("nothing-cites-this");
    let survivor = object("not-deleted");

    let index = EvidenceIndex::of(vec![
        ProjectionRecord::citing(
            AffectedProjectionKind::Concept,
            "concept:two-phase-locking",
            vec![transcript, survivor],
        )?,
        ProjectionRecord::citing(
            AffectedProjectionKind::Evidence,
            "evidence:lecture-07-audio",
            vec![audio],
        )?,
        ProjectionRecord::citing(
            AffectedProjectionKind::Evidence,
            "evidence:embedding-07",
            vec![embedding, transcript],
        )?,
        ProjectionRecord::citing(
            AffectedProjectionKind::Concept,
            "concept:untouched",
            vec![survivor],
        )?,
    ])?;

    let subject = SubjectInventory::new(
        common::offering()?,
        term()?,
        permission_id()?,
        parent_terms(),
        2,
        1,
        vec![
            (DerivativeClass::Transcript, 1, parent_terms()),
            (DerivativeClass::Embedding, 3, parent_terms()),
        ],
    );

    // An instant after the audio bound and before the transcript one: the two
    // axes disagree, which a preview with one retention value could not say.
    let at = PARENT_AUDIO_UNTIL + 1;
    let deleted = vec![audio, transcript, embedding, orphan];
    let preview = preview_deletion(&mut ledger, &subject, &index, &deleted, at);

    // `P2-G6`'s half, whole.
    assert_eq!(preview.previewed_at(), at);
    assert_eq!(
        preview.impact().derivatives().len(),
        DERIVATIVE_CLASSES.len()
    );
    assert!(preview.impact().audio().expires_now());
    assert!(!preview.impact().transcript().expires_now());
    assert_eq!(preview.impact().audio().object_count(), 2);
    assert_eq!(preview.impact().transcript().object_count(), 1);

    // The projection layer: every affected projection, with what it loses.
    let listed: Vec<(&str, &str, usize, usize, &str)> = preview
        .projections()
        .iter()
        .map(|projection| {
            (
                projection.kind().as_str(),
                projection.id(),
                projection.cited_deleted(),
                projection.cited_total(),
                projection.effect().as_str(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![
            (
                "CONCEPT",
                "concept:two-phase-locking",
                1,
                2,
                "LOSES_SOME_EVIDENCE"
            ),
            (
                "EVIDENCE",
                "evidence:lecture-07-audio",
                1,
                1,
                "LOSES_ALL_EVIDENCE"
            ),
            (
                "EVIDENCE",
                "evidence:embedding-07",
                2,
                2,
                "LOSES_ALL_EVIDENCE"
            ),
        ]
    );

    // A projection citing nothing this deletion reaches is not listed.
    assert!(
        !preview
            .projections()
            .iter()
            .any(|projection| projection.id() == "concept:untouched"),
        "an unaffected projection was listed"
    );

    // The object nothing cites is a row rather than a hole, and the partition
    // reconciles over the whole deletion set.
    assert_eq!(preview.unreferenced(), &[orphan]);
    assert!(preview.partition_reconciles(&index));
    assert_eq!(preview.deleted().len(), deleted.len());

    // A projection that cites nothing cannot enter the index, and an index that
    // names one projection twice is refused.
    assert_eq!(
        ProjectionRecord::citing(AffectedProjectionKind::Concept, "empty", Vec::new()),
        Err(DeletionFault::ProjectionCitesNoEvidence)
    );
    assert_eq!(
        EvidenceIndex::of(vec![
            ProjectionRecord::citing(AffectedProjectionKind::Concept, "same", vec![audio])?,
            ProjectionRecord::citing(AffectedProjectionKind::Concept, "same", vec![transcript])?,
        ]),
        Err(DeletionFault::ProjectionIsNamedTwice)
    );

    // Nothing is deleted without the preview: the plan has one constructor and
    // it takes one.
    let plan = LectureDeletionPlan::from_preview(preview.clone());
    assert_eq!(
        apply_deletion(&mut ledger, &plan, &object("some-other-preview"), at),
        Err(DeletionFault::PreviewDigestDoesNotMatch)
    );
    assert_eq!(
        apply_deletion(&mut ledger, &plan, preview.digest(), at + 1),
        Err(DeletionFault::PreviewIsForAnotherInstant)
    );
    let outcome = apply_deletion(&mut ledger, &plan, preview.digest(), at)?;
    assert_eq!(outcome.projections_affected(), 3);
    assert!(outcome.objects_reached() > 0);

    // The ledger carries `P2-G6`'s own two rows.
    let kinds: Vec<ConsentEventKind> = ledger
        .entries()
        .iter()
        .map(academic_consent::LedgerEntry::kind)
        .filter(|kind| {
            matches!(
                kind,
                ConsentEventKind::ExpiryPreviewed | ConsentEventKind::ExpiryApplied
            )
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            ConsentEventKind::ExpiryPreviewed,
            ConsentEventKind::ExpiryApplied
        ]
    );

    // Nothing has expired at an instant before either bound.
    let mut early_ledger = common::ledger_permitting()?;
    let early = preview_deletion(&mut early_ledger, &subject, &index, &deleted, 1);
    let early_plan = LectureDeletionPlan::from_preview(early.clone());
    assert_eq!(
        apply_deletion(&mut early_ledger, &early_plan, early.digest(), 1),
        Err(DeletionFault::NothingHasExpired)
    );

    // Every projection family is exercised, so neither is a value nothing
    // produces.
    let families: BTreeSet<&str> = preview
        .projections()
        .iter()
        .map(|projection| projection.kind().as_str())
        .collect();
    assert_eq!(
        families,
        AffectedProjectionKind::ALL
            .into_iter()
            .map(AffectedProjectionKind::as_str)
            .collect::<BTreeSet<_>>()
    );
    let effects: BTreeSet<&str> = preview
        .projections()
        .iter()
        .map(|projection| projection.effect().as_str())
        .collect();
    assert_eq!(
        effects,
        ProjectionEffect::ALL
            .into_iter()
            .map(ProjectionEffect::as_str)
            .collect::<BTreeSet<_>>()
    );
    Ok(())
}
