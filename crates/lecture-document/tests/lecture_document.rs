//! The fifteen `P2-L4` acceptance rows the plan names.
//!
//! Every oracle in this file is written from the fixture table in
//! `common/mod.rs` — five segments carrying 4, 5, 5, 6 and 1 tokens — and never
//! from a report. `P2-L3` shipped an `AuthorizationBinding` whose expected
//! value came out of the journal it was checking, so the comparison agreed with
//! itself; a coverage oracle that reads its numerator off the report it is
//! checking has the same shape, and every count below is a literal.

mod common;

use std::error::Error;

use academic_domain::{ContentDigest, engines::EngineVersion};
use academic_lecture_document::{
    COVERAGE_CONFIG_V1, CaptureExclusion, CaptureExclusionLedger, CaptureExclusionReason,
    CoverageFault, CoverageInputs, CoverageValidator, DispositionLedger, DocumentAnnotation,
    DocumentBuilder, DocumentCompleteness, DocumentFault, NodeDraft, NodeKind, NonSpeechEvidence,
    NonSpeechReason, PdfArtifact, PreservationTransform, RedactionBasis, RedactionPolicyRef,
    RenderDefect, RenderQa, RenderedImage, RenderedNode, RenderedPage, ReviewQueue, RiskClass,
    Salience, SegmentDisposition, SegmentStatus, StudyIndexBuilder, StudyIndexId,
    TRANSCRIPT_COVERAGE_ENGINE_ID, TRANSCRIPT_COVERAGE_ENGINE_VERSION, TranscriptCoverageEngine,
    TranscriptionFailure, freeze, ruleset_hash,
};

use common::{
    INSIDE, SEGMENT_UNITS, SEGMENTS, TOTAL_TOKENS, TestResult, calibration, capture_frame_seq,
    capture_with_explained_gap, capture_with_hole, clean_capture, clean_render, cross_reference,
    document_id, engine_actor, full_manifest, importer_actor, model_actor, no_calibration, node_id,
    purpose, response_body, transcribe, transcribe_body, user, validate, validate_with,
    whole_document, whole_segment_node,
};

/// The specification, read for the phrases the closed sets are compared to.
fn specification() -> Result<String, Box<dyn Error>> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md");
    Ok(std::fs::read_to_string(path)?)
}

// ---------------------------------------------------------------------------
// coverage_determinism
// ---------------------------------------------------------------------------

/// `REQ-12-038`. The same inputs produce the same bytes, twice, and the answer
/// travels through `P2-C5`'s engine so "deterministic" is a committed byte
/// comparison rather than an absence of errors.
#[test]
fn coverage_determinism() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "determinism")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let document = whole_document(lineage, &manifest, seq)?;

    let first = validate(lineage, &document, &manifest, &capture.recovery)?;
    let second = validate(lineage, &document, &manifest, &capture.recovery)?;
    assert_eq!(
        first.canonical_bytes(),
        second.canonical_bytes(),
        "two runs over the same inputs produced different report bytes"
    );
    assert_eq!(first.digest(), second.digest());

    // A second document built from the same drafts is a second value, and it
    // has to produce the same report. Without this the first assertion would
    // pass on a report that memoized itself.
    let rebuilt = whole_document(lineage, &manifest, seq)?;
    assert_eq!(document.digest(), rebuilt.digest());
    let third = validate(lineage, &rebuilt, &manifest, &capture.recovery)?;
    assert_eq!(first.canonical_bytes(), third.canonical_bytes());

    // The engine half.
    let qa = clean_render(&document)?;
    let inputs = freeze(&first, &qa)?;
    let hash = ruleset_hash();
    let version = EngineVersion::new(TRANSCRIPT_COVERAGE_ENGINE_VERSION)?;
    let left = TranscriptCoverageEngine::evaluate_coverage(&inputs, hash)?;
    let right = TranscriptCoverageEngine::evaluate_coverage(&inputs, hash)?;
    assert_eq!(
        left.canonical_bytes(TRANSCRIPT_COVERAGE_ENGINE_ID, hash, version, &inputs),
        right.canonical_bytes(TRANSCRIPT_COVERAGE_ENGINE_ID, hash, version, &inputs),
    );

    // The control: a different configuration is a different answer.
    //
    // It varies **only** the confidence permille, which no check in the report
    // reads -- it belongs to the review queue. The first version of this control
    // varied the gap threshold too, and an injection that removed the whole
    // configuration from the encoding still passed it: the two reports differed
    // by their *gap findings*, so the assertion was true for a reason its own
    // comment did not claim. Varying a field that changes no measurement is what
    // makes this an assertion about the encoding.
    let other_config = academic_lecture_document::CoverageConfig::new(
        COVERAGE_CONFIG_V1.version(),
        COVERAGE_CONFIG_V1.gap_threshold_nanos(),
        500,
    )?;
    let dispositions = DispositionLedger::new();
    let exclusions = CaptureExclusionLedger::new();
    let under_other = academic_lecture_document::CoverageValidator::validate(
        &academic_lecture_document::CoverageInputs {
            lineage,
            version: 1,
            document: &document,
            manifest: &manifest,
            journal: &capture.recovery,
            dispositions: &dispositions,
            capture_exclusions: &exclusions,
            config: other_config,
        },
    )?;
    assert_ne!(
        first.canonical_bytes(),
        under_other.canonical_bytes(),
        "the report bytes do not depend on the configuration they were measured under"
    );
    assert_eq!(
        under_other.segment_coverage(),
        first.segment_coverage(),
        "the control's configuration changed a measurement, so it is not a control          over the encoding"
    );
    assert_eq!(under_other.gaps(), first.gaps());
    assert_eq!(under_other.unmapped_count(), first.unmapped_count());

    // And a rule-set hash that is not this engine's published one is refused
    // rather than evaluated under.
    let foreign = academic_domain::engines::RuleSetHash::new(ContentDigest::sha256(b"not-ours"));
    assert!(TranscriptCoverageEngine::evaluate_coverage(&inputs, foreign).is_err());
    Ok(())
}

// ---------------------------------------------------------------------------
// segment_coverage_oracle
// ---------------------------------------------------------------------------

/// `REQ-12-039`. Mapped non-silence segments over all eligible segments, with
/// the denominator's one exclusion observed.
///
/// The fixture has five segments. The document below maps three of them,
/// segment four is declared non-speech, and segment three has no status at all.
/// The expected value is therefore **3/4**, written here.
#[test]
fn segment_coverage_oracle() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "segment-oracle")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();

    let mut builder = DocumentBuilder::over(document_id("seg-oracle")?, lineage, 1, &manifest)?;
    for (index, id) in [(0, "s-0"), (1, "s-1"), (2, "s-2")] {
        builder.push(whole_segment_node(
            id,
            NodeKind::Paragraph,
            index,
            PreservationTransform::Punctuation,
        )?)?;
    }
    let document = builder.finish()?;

    let mut dispositions = DispositionLedger::new();
    dispositions.record(SegmentDisposition::excluded_non_speech(
        4,
        NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
    ))?;
    let exclusions = CaptureExclusionLedger::new();
    let report = validate_with(
        lineage,
        &document,
        &manifest,
        &capture.recovery,
        &dispositions,
        &exclusions,
    )?;

    assert_eq!(report.segment_coverage().numerator(), 3);
    assert_eq!(report.segment_coverage().denominator(), 4);
    assert!(!report.segment_coverage().is_whole());
    assert_eq!(report.unmapped_count(), 1);
    assert_eq!(report.unmapped()[0].segment_index(), 3);

    // The positive control: mapping every segment makes it whole, so the
    // assertion above is not passing because nothing is ever whole.
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let whole = whole_document(lineage, &manifest, seq)?;
    let full = validate(lineage, &whole, &manifest, &capture.recovery)?;
    assert_eq!(full.segment_coverage().numerator(), 5);
    assert_eq!(full.segment_coverage().denominator(), 5);
    assert!(full.segment_coverage().is_whole());
    Ok(())
}

// ---------------------------------------------------------------------------
// token_coverage_oracle
// ---------------------------------------------------------------------------

/// `REQ-12-040`. Mapped tokens over all tokens, counted from the fixture table
/// rather than from the report.
///
/// The fixture's five segments carry 4, 5, 5, 6 and 1 tokens: twenty-one in
/// all. A document mapping the whole of segments zero, one and two covers
/// 4 + 5 + 5 = **14** of **21**. Declaring segment four non-speech takes its
/// token out of the *numerator* and leaves it in the denominator, because
/// section 12.6 writes the token ratio as "mapped normalized tokens / all
/// normalized tokens" with no qualifier — the qualifier is on the segment line
/// only. Until `P2-RF20` this denominator read 20, and `P2-A4` measured what
/// that bought: a document holding one of twenty-one tokens reading `COMPLETE`.
#[test]
fn token_coverage_oracle() -> TestResult {
    assert_eq!(
        SEGMENTS
            .iter()
            .map(|(_, _, words)| words.len() as u64)
            .sum::<u64>(),
        TOTAL_TOKENS,
        "the fixture table and its stated total disagree"
    );

    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "token-oracle")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();

    let mut builder = DocumentBuilder::over(document_id("tok-oracle")?, lineage, 1, &manifest)?;
    for (index, id) in [(0, "t-0"), (1, "t-1"), (2, "t-2")] {
        builder.push(whole_segment_node(
            id,
            NodeKind::Paragraph,
            index,
            PreservationTransform::Punctuation,
        )?)?;
    }
    let document = builder.finish()?;

    let mut dispositions = DispositionLedger::new();
    dispositions.record(SegmentDisposition::excluded_non_speech(
        4,
        NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
    ))?;
    let exclusions = CaptureExclusionLedger::new();
    let report = validate_with(
        lineage,
        &document,
        &manifest,
        &capture.recovery,
        &dispositions,
        &exclusions,
    )?;
    assert_eq!(report.token_coverage().numerator(), 14);
    assert_eq!(report.token_coverage().denominator(), TOTAL_TOKENS);

    // A segment can be mapped and still be missing tokens. "serializability is"
    // is the first eighteen characters of segment zero and covers two of its
    // four tokens, so a document that maps only that span is `MAPPED` with a
    // token coverage of two.
    let mut partial = DocumentBuilder::over(document_id("tok-partial")?, lineage, 1, &manifest)?;
    partial.push(NodeDraft {
        id: node_id("p-0")?,
        kind: NodeKind::Paragraph,
        rendered_text: "Instructor: serializability is".to_owned(),
        mappings: vec![(0, 0, 18, PreservationTransform::SpeakerLabel)],
        nearby_captures: Vec::new(),
        annotations: Vec::new(),
        cross_reference: None,
    })?;
    let partial = partial.finish()?;
    let partial_report = validate(lineage, &partial, &manifest, &capture.recovery)?;
    assert_eq!(partial_report.token_coverage().numerator(), 2);
    assert_eq!(partial_report.token_coverage().denominator(), TOTAL_TOKENS);
    assert_eq!(
        partial_report.accounts()[0].status().as_str(),
        "MAPPED",
        "a partially covered segment is still mapped; the loss shows in the token count"
    );

    // A ratio nobody reads is not a check. `P2-A4`'s F4: deleting
    // `!self.token_coverage.is_whole()` from `completeness_witness` left all
    // thirty-one rows of this crate green, because the 2101-shape sweep builds
    // every node with `whole_segment_node` and this oracle stopped at the two
    // numbers. The condition is the *only* one that catches a segment that is
    // mapped and whose tokens are not all rendered, so it is driven here: a
    // document whose segments are all mapped, whose partition reconciles and
    // whose unmapped list is empty is still `INCOMPLETE` on the token count
    // alone.
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let mut whole_but_one =
        DocumentBuilder::over(document_id("tok-drives")?, lineage, 1, &manifest)?;
    whole_but_one.push(NodeDraft {
        id: node_id("p-00")?,
        kind: NodeKind::Paragraph,
        rendered_text: "Instructor: serializability is".to_owned(),
        mappings: vec![(0, 0, 18, PreservationTransform::SpeakerLabel)],
        nearby_captures: Vec::new(),
        annotations: Vec::new(),
        cross_reference: None,
    })?;
    for index in 1..SEGMENTS.len() {
        whole_but_one.push(whole_segment_node(
            &format!("p-{index}"),
            NodeKind::Paragraph,
            index,
            PreservationTransform::Punctuation,
        )?)?;
    }
    let mut placement = whole_segment_node(
        "p-cap",
        NodeKind::CapturePlacement,
        SEGMENTS.len().saturating_sub(1),
        PreservationTransform::CapturePlacement,
    )?;
    placement.nearby_captures = vec![seq];
    whole_but_one.push(placement)?;
    let whole_but_one = whole_but_one.finish()?;
    let drives = validate(lineage, &whole_but_one, &manifest, &capture.recovery)?;
    assert_eq!(drives.unmapped_count(), 0, "the driver has no unmapped row");
    assert!(drives.reconciles(), "the driver's partition reconciles");
    assert!(
        drives.segment_coverage().is_whole(),
        "the driver's segment coverage is whole, so only the token ratio can refuse"
    );
    assert!(drives.ordering_findings().is_empty());
    assert!(drives.unaccounted_captures().is_empty());
    assert!(drives.unexplained_gaps().is_empty());
    assert_eq!(drives.token_coverage().numerator(), TOTAL_TOKENS - 2);
    assert_eq!(drives.token_coverage().denominator(), TOTAL_TOKENS);
    assert!(
        drives.completeness_witness().is_none(),
        "two tokens are missing from the document and it minted a witness"
    );
    let qa = clean_render(&whole_but_one)?;
    let pdf = PdfArtifact::render(&whole_but_one, &drives, &qa, ContentDigest::sha256(b"pdf"));
    assert_eq!(pdf.completeness().as_str(), "INCOMPLETE");
    Ok(())
}

// ---------------------------------------------------------------------------
// a_non_speech_declaration_cannot_delete_the_lecture
// ---------------------------------------------------------------------------

/// `REQ-12-045`. Declaring a transcribed segment non-speech does not buy a
/// completeness badge.
///
/// `P2-A4`'s F1, driven. `NonSpeechEvidence::declared` takes the caller's word
/// that a segment holds no speech and nothing checks it against the segment's
/// own token list — the `P2-L3` shape `disposition.rs`'s own module doc warns
/// against. That is not closed here, because `EXCLUDED_NON_SPEECH` is one of
/// section 12.6's four required statuses and `RawSegment::close` refuses a
/// zero-token segment, so *every* legitimate declaration sits on at least one
/// transcribed word. What is closed is the arithmetic: those words stay in the
/// token denominator, so the declaration cannot make them disappear from the
/// measurement as well as from the page.
///
/// At `c81b74b` this document — four of the five fixture segments declared
/// `SILENCE`, one token of twenty-one rendered — measured `seg=1/1 tok=1/1`,
/// minted a `CompletenessWitness` and rendered `COMPLETE` on Windows native and
/// on WSL2.
#[test]
fn a_non_speech_declaration_cannot_delete_the_lecture() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "non-speech-loss")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let last = SEGMENTS.len().saturating_sub(1);
    let exclusions = CaptureExclusionLedger::new();

    let mut builder = DocumentBuilder::over(document_id("silenced")?, lineage, 1, &manifest)?;
    builder.push(whole_segment_node(
        "s-last",
        NodeKind::Paragraph,
        last,
        PreservationTransform::Punctuation,
    )?)?;
    let mut placement = whole_segment_node(
        "s-cap",
        NodeKind::CapturePlacement,
        last,
        PreservationTransform::CapturePlacement,
    )?;
    placement.nearby_captures = vec![seq];
    builder.push(placement)?;
    let document = builder.finish()?;

    let mut dispositions = DispositionLedger::new();
    for index in 0..last {
        dispositions.record(SegmentDisposition::excluded_non_speech(
            index,
            NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
        ))?;
    }
    let report = validate_with(
        lineage,
        &document,
        &manifest,
        &capture.recovery,
        &dispositions,
        &exclusions,
    )?;

    // Every other condition the witness needs is satisfied, so the token ratio
    // is what refuses. Without this the assertion below would pass for a reason
    // it does not claim.
    assert_eq!(report.unmapped_count(), 0);
    assert!(report.reconciles());
    assert!(report.segment_coverage().is_whole());
    assert!(report.ordering_findings().is_empty());
    assert!(report.unaccounted_captures().is_empty());
    assert!(report.unexplained_gaps().is_empty());

    let rendered = SEGMENTS[last].2.len() as u64;
    assert_eq!(report.token_coverage().numerator(), rendered);
    assert_eq!(
        report.token_coverage().denominator(),
        TOTAL_TOKENS,
        "a declaration took the lecture's own words out of the denominator"
    );
    assert!(
        !report.token_coverage().is_whole(),
        "{} of {TOTAL_TOKENS} tokens are in the document and the ratio is whole",
        rendered
    );
    assert!(report.completeness_witness().is_none());

    let qa = clean_render(&document)?;
    let pdf = PdfArtifact::render(&document, &report, &qa, ContentDigest::sha256(b"pdf"));
    assert!(!pdf.completeness().is_complete());
    assert_eq!(pdf.completeness().as_str(), "INCOMPLETE");

    // The control, so this is not a test that everything is incomplete: the
    // same document with nothing declared and every segment mapped is
    // `COMPLETE`.
    let whole = whole_document(lineage, &manifest, seq)?;
    let clean = validate_with(
        lineage,
        &whole,
        &manifest,
        &capture.recovery,
        &DispositionLedger::new(),
        &exclusions,
    )?;
    assert!(clean.token_coverage().is_whole());
    assert!(clean.completeness_witness().is_some());

    // And one declaration on its own is enough to refuse, whichever segment it
    // is: the loss does not need to be large to stop the badge.
    for index in 0..SEGMENTS.len() {
        let mut one = DispositionLedger::new();
        one.record(SegmentDisposition::excluded_non_speech(
            index,
            NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
        ))?;
        let mut short = DocumentBuilder::over(document_id("one-off")?, lineage, 1, &manifest)?;
        for other in (0..SEGMENTS.len()).filter(|other| *other != index) {
            short.push(whole_segment_node(
                &format!("o-{other}"),
                NodeKind::Paragraph,
                other,
                PreservationTransform::Punctuation,
            )?)?;
        }
        let mut placement = whole_segment_node(
            "o-cap",
            NodeKind::CapturePlacement,
            if index == last { 0 } else { last },
            PreservationTransform::CapturePlacement,
        )?;
        placement.nearby_captures = vec![seq];
        short.push(placement)?;
        let short = short.finish()?;
        let one_report = validate_with(
            lineage,
            &short,
            &manifest,
            &capture.recovery,
            &one,
            &exclusions,
        )?;
        assert!(
            one_report.segment_coverage().is_whole(),
            "segment {index}: the segment ratio is what refused, not the token ratio"
        );
        assert_eq!(
            one_report.token_coverage().denominator(),
            TOTAL_TOKENS,
            "segment {index} left the token denominator"
        );
        assert!(
            one_report.completeness_witness().is_none(),
            "segment {index} declared non-speech still minted a witness"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// a_segment_whose_verbatim_omits_its_tokens_is_refused
// ---------------------------------------------------------------------------

/// `REQ-12-039`. The token alignment section 12.6 asks for is a refusal, and
/// the refusal is reachable from the real pipeline.
///
/// `P2-A4`'s F8 recorded this guard as undriven and left its severity
/// contingent on whether a segment whose verbatim text does not contain its own
/// tokens can be built by `academic_transcription::run` at all. It can:
/// `RawSegment::close` (`crates/transcription/src/transcript.rs`) checks that a
/// verbatim line is present and that the token list is non-empty, and **nothing
/// there compares the two**. A provider answering with a verbatim line and a
/// word that is not in it produces exactly that segment, and the decoder
/// accepts it. So the contingency resolves to "reachable", and the guard is
/// driven here rather than recorded.
///
/// The two arms are the two ways `token_spans` is called: `DocumentBuilder::push`
/// and `CoverageValidator::validate`. Replacing the refusal with
/// `unwrap_or(cursor)` left the whole crate suite green before this row.
#[test]
fn a_segment_whose_verbatim_omits_its_tokens_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "misaligned")?;
    let manifest = full_manifest(&capture)?;

    // The fixture body with one word replaced by a token the verbatim line does
    // not carry. Everything else — the segment header, the timings, the
    // speaker, the chunk list — is the fixture's own, so what the decoder is
    // being asked to accept differs in one field.
    let (_, verbatim, words) = SEGMENTS[0];
    let intruder = "zqxjabsentword";
    assert!(
        !verbatim.contains(intruder),
        "the intruder token is in the fixture's own verbatim text"
    );
    let body = response_body().replace(
        &format!("word: 0 {} {}\n", SEGMENT_UNITS[0], words[0]),
        &format!("word: 0 {} {intruder}\n", SEGMENT_UNITS[0]),
    );
    assert!(
        body.contains(intruder),
        "the response body was not rewritten"
    );

    // The pipeline accepts it, which is the half that makes this reachable
    // rather than dead.
    let transcribed = transcribe_body(&manifest, &body)?;
    let lineage = transcribed.lineage();
    let segment = lineage
        .segment_at(1, 0)
        .ok_or("the misaligned transcript has no first segment")?;
    assert!(
        !segment.verbatim_text().contains(intruder),
        "the decoder rewrote the verbatim text"
    );
    assert!(
        segment
            .tokens()
            .iter()
            .any(|token| token.text() == intruder),
        "the decoder dropped the intruding token"
    );

    // The builder's arm.
    let mut builder = DocumentBuilder::over(document_id("misaligned")?, lineage, 1, &manifest)?;
    let refusal = builder.push(whole_segment_node(
        "m-0",
        NodeKind::Paragraph,
        0,
        PreservationTransform::Punctuation,
    )?);
    assert!(
        matches!(
            refusal,
            Err(DocumentFault::VerbatimDoesNotContainTokens {
                token_position: 0,
                ..
            })
        ),
        "the builder mapped a segment whose verbatim text omits its tokens: {refusal:?}"
    );

    // The validator's arm. `CoverageValidator::validate` walks the whole
    // eligible set and aligns every segment, mapped or not, so a document that
    // maps only the four aligned segments still reaches segment zero. The
    // builder never saw it, so this refusal is the validator's own.
    let mut partial = DocumentBuilder::over(document_id("m-partial")?, lineage, 1, &manifest)?;
    for index in 1..SEGMENTS.len() {
        partial.push(whole_segment_node(
            &format!("m-{index}"),
            NodeKind::Paragraph,
            index,
            PreservationTransform::Punctuation,
        )?)?;
    }
    let partial = partial.finish()?;
    let dispositions = DispositionLedger::new();
    let exclusions = CaptureExclusionLedger::new();
    let validated = CoverageValidator::validate(&CoverageInputs {
        lineage,
        version: 1,
        document: &partial,
        manifest: &manifest,
        journal: &capture.recovery,
        dispositions: &dispositions,
        capture_exclusions: &exclusions,
        config: COVERAGE_CONFIG_V1,
    });
    assert!(
        matches!(
            validated,
            Err(CoverageFault::Document(
                DocumentFault::VerbatimDoesNotContainTokens { .. }
            ))
        ),
        "the validator measured a segment whose verbatim text omits its tokens: {validated:?}"
    );

    // The control: the unmodified body maps and validates, so neither arm above
    // refuses everything.
    let clean = transcribe(&manifest)?;
    let clean_lineage = clean.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let document = whole_document(clean_lineage, &manifest, seq)?;
    assert!(validate(clean_lineage, &document, &manifest, &capture.recovery).is_ok());
    Ok(())
}

// ---------------------------------------------------------------------------
// section_12_6_states_both_ratios
// ---------------------------------------------------------------------------

/// Section 12.6's two ratio lines, parsed out of the specification both ways.
///
/// The reason this test exists rather than a sentence in a contract page: the
/// implementation treats the two denominators differently, and the *only*
/// warrant for the difference is that section 12.6 writes `non-silence` on one
/// line and not the other. If the document stops saying that, the code's
/// justification is gone and this fails — which is the shape `P2-N6` used, and
/// the alternative is a divergence nobody notices, which is what `P2-A4` found.
///
/// Both readings of the segment line are recorded in
/// `docs/contracts/lecture-document.md`. This test does not choose between
/// them; it pins the text they are readings *of*.
#[test]
fn section_12_6_states_both_ratios() -> TestResult {
    let specification = specification()?;
    let segment_line = specification
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("segment coverage"))
        .ok_or("section 12.6's segment-coverage line moved")?;
    let token_line = specification
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("token coverage"))
        .ok_or("section 12.6's token-coverage line moved")?;

    // Forward: the document says what the code assumes.
    assert_eq!(
        segment_line,
        "segment coverage = mapped non-silence transcript segments / all eligible segments",
        "section 12.6's segment ratio changed"
    );
    assert_eq!(
        token_line, "token coverage   = mapped normalized tokens / all normalized tokens",
        "section 12.6's token ratio changed"
    );

    // Backward: the qualifier is on one line and not on the other, stated as a
    // property rather than as two string literals, so a rewording that keeps
    // the sentences but moves the qualifier still fails.
    assert!(
        segment_line.contains("non-silence"),
        "the segment ratio lost its non-silence qualifier; the segment denominator's subtraction has no warrant now"
    );
    assert!(
        !token_line.contains("non-silence") && !token_line.contains("silence"),
        "the token ratio gained a silence qualifier; the token denominator would have to change with it"
    );
    assert_eq!(
        token_line
            .split_once('/')
            .map(|(_, denominator)| denominator.trim()),
        Some("all normalized tokens"),
        "the token denominator is no longer every normalized token"
    );

    // And the four statuses the document requires are the four the code has, so
    // "declared non-speech" is a real account of a segment rather than a way
    // out of the measurement.
    let statuses = specification
        .lines()
        .find(|line| line.contains("`EXCLUDED_NON_SPEECH`"))
        .ok_or("section 12.6's status list moved")?;
    for status in [
        "`MAPPED`",
        "`EXCLUDED_NON_SPEECH`",
        "`REDACTED_WITH_POLICY`",
        "`UNTRANSCRIBED_FAILURE`",
    ] {
        assert!(
            statuses.contains(status),
            "section 12.6 no longer lists {status}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ordering_check
// ---------------------------------------------------------------------------

/// `REQ-12-041`. Source order is monotonic unless a cross-reference says
/// otherwise, and the exception is recorded rather than swallowed.
#[test]
fn ordering_check() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "ordering")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;

    let whole = whole_document(lineage, &manifest, seq)?;
    let ordered = validate(lineage, &whole, &manifest, &capture.recovery)?;
    assert!(ordered.ordering_findings().is_empty());
    assert!(ordered.ordering_exceptions().is_empty());

    // Two mapped segments swapped in document order.
    let mut swapped = DocumentBuilder::over(document_id("swapped")?, lineage, 1, &manifest)?;
    swapped.push(whole_segment_node(
        "o-1",
        NodeKind::Paragraph,
        2,
        PreservationTransform::OrderPreservation,
    )?)?;
    swapped.push(whole_segment_node(
        "o-0",
        NodeKind::Paragraph,
        0,
        PreservationTransform::OrderPreservation,
    )?)?;
    let swapped = swapped.finish()?;
    let report = validate(lineage, &swapped, &manifest, &capture.recovery)?;
    assert_eq!(report.ordering_findings().len(), 1);
    assert_eq!(report.ordering_findings()[0].segment_index(), 0);
    assert_eq!(report.ordering_findings()[0].previous_segment_index(), 2);
    assert!(report.completeness_witness().is_none());

    // The same order with an explicit cross-reference is an exception, not a
    // finding — and the transcript's own order is untouched by it.
    let before: Vec<String> = (0..5)
        .filter_map(|index| lineage.segment_at(1, index))
        .map(|segment| segment.id().to_owned())
        .collect();
    let mut referenced = DocumentBuilder::over(document_id("referenced")?, lineage, 1, &manifest)?;
    referenced.push(whole_segment_node(
        "x-1",
        NodeKind::Paragraph,
        2,
        PreservationTransform::OrderPreservation,
    )?)?;
    let mut back = whole_segment_node(
        "x-0",
        NodeKind::Paragraph,
        0,
        PreservationTransform::OrderPreservation,
    )?;
    back.cross_reference = Some(cross_reference(0));
    referenced.push(back)?;
    let referenced = referenced.finish()?;
    let report = validate(lineage, &referenced, &manifest, &capture.recovery)?;
    assert!(report.ordering_findings().is_empty());
    assert_eq!(report.ordering_exceptions().len(), 1);
    assert_eq!(report.ordering_exceptions()[0].segment_index(), 0);
    let after: Vec<String> = (0..5)
        .filter_map(|index| lineage.segment_at(1, index))
        .map(|segment| segment.id().to_owned())
        .collect();
    assert_eq!(
        before, after,
        "declaring a cross-reference reordered the source"
    );

    // A cross-reference that names a different segment than the node maps is
    // not an exception. Without this the check would be a boolean that turns
    // itself off.
    let mut mismatched = DocumentBuilder::over(document_id("mismatched")?, lineage, 1, &manifest)?;
    mismatched.push(whole_segment_node(
        "m-1",
        NodeKind::Paragraph,
        2,
        PreservationTransform::OrderPreservation,
    )?)?;
    let mut back = whole_segment_node(
        "m-0",
        NodeKind::Paragraph,
        0,
        PreservationTransform::OrderPreservation,
    )?;
    back.cross_reference = Some(cross_reference(1));
    mismatched.push(back)?;
    let mismatched = mismatched.finish()?;
    let report = validate(lineage, &mismatched, &manifest, &capture.recovery)?;
    assert_eq!(report.ordering_findings().len(), 1);
    assert!(report.ordering_exceptions().is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// capture_coverage
// ---------------------------------------------------------------------------

/// `REQ-12-042`. Every authorized capture is placed or excluded with a reason,
/// and neither is a state the report can be complete in.
#[test]
fn capture_coverage() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "captures")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    assert_eq!(manifest.captures().len(), 1);

    // Placed.
    let placed = whole_document(lineage, &manifest, seq)?;
    let report = validate(lineage, &placed, &manifest, &capture.recovery)?;
    assert_eq!(report.placed_captures(), &[seq]);
    assert!(report.unaccounted_captures().is_empty());
    assert!(report.completeness_witness().is_some());

    // Neither placed nor excluded.
    let mut orphan = DocumentBuilder::over(document_id("orphan")?, lineage, 1, &manifest)?;
    for index in 0..5 {
        orphan.push(whole_segment_node(
            &format!("c-{index}"),
            NodeKind::Paragraph,
            index,
            PreservationTransform::Punctuation,
        )?)?;
    }
    let orphan = orphan.finish()?;
    let report = validate(lineage, &orphan, &manifest, &capture.recovery)?;
    assert_eq!(report.unaccounted_captures().len(), 1);
    assert_eq!(report.unaccounted_captures()[0].frame_seq(), seq);
    assert!(
        report.completeness_witness().is_none(),
        "a capture nothing accounts for did not deny the witness"
    );

    // Excluded with a reason.
    let mut exclusions = CaptureExclusionLedger::new();
    exclusions.record(CaptureExclusion::declared(
        seq,
        CaptureExclusionReason::UnreadableImage,
        user()?,
    )?)?;
    let dispositions = DispositionLedger::new();
    let report = validate_with(
        lineage,
        &orphan,
        &manifest,
        &capture.recovery,
        &dispositions,
        &exclusions,
    )?;
    assert_eq!(
        report.excluded_captures(),
        &[(seq, CaptureExclusionReason::UnreadableImage)]
    );
    assert!(report.unaccounted_captures().is_empty());
    assert!(report.completeness_witness().is_some());

    // Placed *and* excluded is a refusal rather than a choice between the two.
    let report = validate_with(
        lineage,
        &placed,
        &manifest,
        &capture.recovery,
        &dispositions,
        &exclusions,
    );
    assert_eq!(report, Err(CoverageFault::CaptureIsPlacedAndExcluded(seq)));

    // Only a person excludes a capture, and only an authorized one can be
    // excluded at all.
    for actor in [model_actor()?, engine_actor(), importer_actor()] {
        assert_eq!(
            CaptureExclusion::declared(seq, CaptureExclusionReason::UnreadableImage, actor),
            Err(CoverageFault::AutomaticActorCannotExclude)
        );
    }
    let mut stray = CaptureExclusionLedger::new();
    stray.record(CaptureExclusion::declared(
        seq + 900,
        CaptureExclusionReason::UnreadableImage,
        user()?,
    )?)?;
    assert_eq!(
        validate_with(
            lineage,
            &orphan,
            &manifest,
            &capture.recovery,
            &dispositions,
            &stray,
        ),
        Err(CoverageFault::ExclusionForNoSuchCapture(seq + 900))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// audio_gap_threshold
// ---------------------------------------------------------------------------

/// `REQ-12-043` and `REQ-34-013`. Only an unexplained hole above the threshold
/// is a finding, and every finding carries its length and whether the journal
/// explains it.
#[test]
fn audio_gap_threshold() -> TestResult {
    let directory = tempfile::tempdir()?;

    // Below the threshold: two frames one second apart, threshold two seconds.
    let clean = clean_capture(&directory, "gap-clean")?;
    let manifest = full_manifest(&clean)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&clean).ok_or("the fixture capture has no image")?;
    let document = whole_document(lineage, &manifest, seq)?;
    let report = validate(lineage, &document, &manifest, &clean.recovery)?;
    assert!(
        report.gaps().is_empty(),
        "a one-second hole under a two-second threshold was reported"
    );

    // Above it, unexplained.
    let holed = capture_with_hole(&directory, "gap-holed")?;
    let holed_manifest = full_manifest(&holed)?;
    let holed_transcribed = transcribe(&holed_manifest)?;
    let holed_lineage = holed_transcribed.lineage();
    let holed_seq = capture_frame_seq(&holed).ok_or("the fixture capture has no image")?;
    let holed_document = whole_document(holed_lineage, &holed_manifest, holed_seq)?;
    let report = validate(
        holed_lineage,
        &holed_document,
        &holed_manifest,
        &holed.recovery,
    )?;
    assert_eq!(report.gaps().len(), 1);
    assert_eq!(report.gaps()[0].length_nanos(), Some(5_000_000_000));
    assert!(!report.gaps()[0].explained());
    assert_eq!(report.unexplained_gaps().len(), 1);
    assert!(
        report.completeness_witness().is_none(),
        "an unexplained hole did not deny the witness"
    );

    // At the threshold exactly: a configuration whose threshold is the hole's
    // own length reports nothing, which is what "above threshold" means.
    let at_threshold = academic_lecture_document::CoverageConfig::new(1, 5_000_000_000, 700)?;
    let dispositions = DispositionLedger::new();
    let exclusions = CaptureExclusionLedger::new();
    let report = academic_lecture_document::CoverageValidator::validate(
        &academic_lecture_document::CoverageInputs {
            lineage: holed_lineage,
            version: 1,
            document: &holed_document,
            manifest: &holed_manifest,
            journal: &holed.recovery,
            dispositions: &dispositions,
            capture_exclusions: &exclusions,
            config: at_threshold,
        },
    )?;
    assert!(report.gaps().is_empty());

    // Explained: the journal carries the gap frames the capture wrote, and the
    // two audio frames on either side are on different clocks, so the hole has
    // no measurable length and is a finding whatever the threshold is.
    let resumed = capture_with_explained_gap(&directory, "gap-resumed")?;
    let resumed_manifest = full_manifest(&resumed)?;
    let resumed_transcribed = transcribe(&resumed_manifest)?;
    let resumed_lineage = resumed_transcribed.lineage();
    let mut nothing = DocumentBuilder::over(
        document_id("resumed-doc")?,
        resumed_lineage,
        1,
        &resumed_manifest,
    )?;
    for index in 0..5 {
        nothing.push(whole_segment_node(
            &format!("g-{index}"),
            NodeKind::Paragraph,
            index,
            PreservationTransform::Punctuation,
        )?)?;
    }
    let nothing = nothing.finish()?;
    let report = validate(
        resumed_lineage,
        &nothing,
        &resumed_manifest,
        &resumed.recovery,
    )?;
    assert_eq!(report.gaps().len(), 1);
    assert_eq!(report.gaps()[0].length_nanos(), None);
    assert!(
        report.gaps()[0].explained(),
        "the journal's own gap frames did not explain the hole they opened"
    );
    assert!(report.unexplained_gaps().is_empty());

    // The failure status a caller may declare comes out of one of those frames,
    // and a frame that is not a gap cannot be cited.
    let gap_seq = resumed
        .recovery
        .records()
        .iter()
        .find(|record| matches!(record.body(), academic_capture::RecordBody::Gap { .. }))
        .map(|record| record.seq())
        .ok_or("the resumed journal has no gap frame")?;
    let failure = TranscriptionFailure::citing_journal_gap(&resumed.recovery, gap_seq)?;
    assert_eq!(failure.frame_seq(), gap_seq);
    assert_eq!(
        TranscriptionFailure::citing_journal_gap(&resumed.recovery, 0),
        Err(CoverageFault::NoSuchGapFrame(0)),
        "an audio frame was accepted as evidence of a recording failure"
    );
    assert_eq!(
        TranscriptionFailure::citing_journal_gap(&resumed.recovery, 9_999),
        Err(CoverageFault::NoSuchGapFrame(9_999))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// segment_status_exhaustive
// ---------------------------------------------------------------------------

/// `REQ-12-044`, `REQ-25-059`, `REQ-28-007`, `INV-C-003`. Every segment lands in
/// exactly one of the four statuses or in the unmapped list, over every shape a
/// five-segment transcript can take.
#[test]
fn segment_status_exhaustive() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "exhaustive")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;

    // The four statuses at once, on one report.
    let mut builder = DocumentBuilder::over(document_id("four")?, lineage, 1, &manifest)?;
    builder.push(whole_segment_node(
        "f-0",
        NodeKind::Paragraph,
        0,
        PreservationTransform::Punctuation,
    )?)?;
    let mut placement = whole_segment_node(
        "f-cap",
        NodeKind::CapturePlacement,
        0,
        PreservationTransform::CapturePlacement,
    )?;
    placement.nearby_captures = vec![seq];
    builder.push(placement)?;
    let document = builder.finish()?;

    let gap_directory = tempfile::tempdir()?;
    let resumed = capture_with_explained_gap(&gap_directory, "exhaustive-gap")?;
    let gap_seq = resumed
        .recovery
        .records()
        .iter()
        .find(|record| matches!(record.body(), academic_capture::RecordBody::Gap { .. }))
        .map(|record| record.seq())
        .ok_or("the resumed journal has no gap frame")?;

    let mut dispositions = DispositionLedger::new();
    dispositions.record(SegmentDisposition::excluded_non_speech(
        1,
        NonSpeechEvidence::declared(NonSpeechReason::MusicOrApplause, user()?)?,
    ))?;
    dispositions.record(SegmentDisposition::redacted_with_policy(
        2,
        RedactionPolicyRef::citing(
            ContentDigest::sha256(b"redaction-policy"),
            RedactionBasis::RightsRequest,
            user()?,
        )?,
    ))?;
    dispositions.record(SegmentDisposition::untranscribed_failure(
        3,
        TranscriptionFailure::citing_journal_gap(&resumed.recovery, gap_seq)?,
    ))?;
    let exclusions = CaptureExclusionLedger::new();
    let report = validate_with(
        lineage,
        &document,
        &manifest,
        &capture.recovery,
        &dispositions,
        &exclusions,
    )?;
    let mut seen: Vec<&str> = report
        .accounts()
        .iter()
        .map(|account| account.status().as_str())
        .collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen,
        vec![
            "EXCLUDED_NON_SPEECH",
            "MAPPED",
            "REDACTED_WITH_POLICY",
            "UNTRANSCRIBED_FAILURE"
        ],
        "the four statuses were not all reachable on one report"
    );
    assert_eq!(report.unmapped_count(), 1);
    assert!(report.reconciles());

    // A segment that is both mapped and declared is refused, not resolved.
    let mut both = DispositionLedger::new();
    both.record(SegmentDisposition::excluded_non_speech(
        0,
        NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
    ))?;
    assert_eq!(
        validate_with(
            lineage,
            &document,
            &manifest,
            &capture.recovery,
            &both,
            &exclusions,
        ),
        Err(CoverageFault::SegmentHasTwoStatuses { segment_index: 0 })
    );

    // A second declaration for one segment is refused by the ledger.
    let mut twice = DispositionLedger::new();
    twice.record(SegmentDisposition::excluded_non_speech(
        4,
        NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
    ))?;
    assert_eq!(
        twice.record(SegmentDisposition::redacted_with_policy(
            4,
            RedactionPolicyRef::citing(
                ContentDigest::sha256(b"other"),
                RedactionBasis::InstitutionalPolicy,
                user()?,
            )?,
        )),
        Err(CoverageFault::DuplicateDisposition(4))
    );

    // A declaration for a segment that does not exist is refused.
    let mut absent = DispositionLedger::new();
    absent.record(SegmentDisposition::excluded_non_speech(
        99,
        NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
    ))?;
    assert_eq!(
        validate_with(
            lineage,
            &document,
            &manifest,
            &capture.recovery,
            &absent,
            &exclusions,
        ),
        Err(CoverageFault::DispositionForNoSuchSegment(99))
    );

    // The partition, over every shape five segments can take. Five outcomes per
    // segment is 3125 combinations; the 1024 with nothing mapped cannot build a
    // document at all, and the rest are all evaluated.
    let outcomes = 5_usize;
    let mut evaluated = 0_usize;
    let mut witnesses = 0_usize;
    // The sweep's documents place no capture, so the one authorized capture is
    // excluded with a reason. Without that every shape would fail the capture
    // check and the witness arm below would never be reached.
    let mut sweep_exclusions = CaptureExclusionLedger::new();
    sweep_exclusions.record(CaptureExclusion::declared(
        seq,
        CaptureExclusionReason::DuplicateOfPlacedCapture,
        user()?,
    )?)?;
    for pattern in 0..outcomes.pow(5) {
        let mut choice = [0_usize; 5];
        let mut rest = pattern;
        for slot in &mut choice {
            *slot = rest % outcomes;
            rest /= outcomes;
        }
        if !choice.contains(&0) {
            continue;
        }
        let mut builder = DocumentBuilder::over(document_id("sweep")?, lineage, 1, &manifest)?;
        let mut ledger = DispositionLedger::new();
        for (index, slot) in choice.iter().enumerate() {
            match slot {
                0 => builder.push(whole_segment_node(
                    &format!("w-{index}"),
                    NodeKind::Paragraph,
                    index,
                    PreservationTransform::Punctuation,
                )?)?,
                1 => ledger.record(SegmentDisposition::excluded_non_speech(
                    index,
                    NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
                ))?,
                2 => ledger.record(SegmentDisposition::redacted_with_policy(
                    index,
                    RedactionPolicyRef::citing(
                        ContentDigest::sha256(b"sweep"),
                        RedactionBasis::PermissionCondition,
                        user()?,
                    )?,
                ))?,
                3 => ledger.record(SegmentDisposition::untranscribed_failure(
                    index,
                    TranscriptionFailure::citing_journal_gap(&resumed.recovery, gap_seq)?,
                ))?,
                _ => {}
            }
        }
        let built = builder.finish()?;
        let report = validate_with(
            lineage,
            &built,
            &manifest,
            &capture.recovery,
            &ledger,
            &sweep_exclusions,
        )?;
        assert!(
            report.reconciles(),
            "the partition did not reconcile at {pattern}"
        );
        assert_eq!(
            report.accounts().len() + report.unmapped().len(),
            5,
            "a segment fell out of the accounting at {pattern}"
        );
        for account in report.accounts() {
            assert!(SegmentStatus::SPELLINGS.contains(&account.status().as_str()));
        }
        if report.completeness_witness().is_some() {
            assert_eq!(report.unmapped_count(), 0);
            witnesses += 1;
        }
        // The two halves of section 12.6's completeness sentence, and how they
        // are related. An injection that deleted the unmapped condition from
        // `completeness_witness` passed every row of this suite unchanged, and
        // the measurement explains why: an unmapped segment is in the coverage
        // denominator and not in its numerator, so whole coverage already
        // implies an empty unmapped list. The implication is asserted here
        // rather than assumed, because it is a property of the *denominator
        // rule* -- which is configuration-shaped -- and not of the two
        // sentences. The unmapped condition stays in the witness because it is
        // the specification's own, and the contract page records that it is not
        // independently observable today.
        if report.unmapped_count() > 0 {
            assert!(
                !report.segment_coverage().is_whole(),
                "an unmapped segment left the coverage ratio whole at {pattern}"
            );
        }
        evaluated += 1;
    }
    assert_eq!(
        evaluated, 2101,
        "the sweep did not cover the shapes it claims"
    );
    assert!(
        witnesses > 0,
        "no shape in the sweep produced a witness, so the witness assertion is vacuous"
    );

    // Only a person may declare a span absent.
    for actor in [model_actor()?, engine_actor(), importer_actor()] {
        assert_eq!(
            NonSpeechEvidence::declared(NonSpeechReason::Silence, actor.clone()),
            Err(CoverageFault::AutomaticActorCannotExclude)
        );
        assert_eq!(
            RedactionPolicyRef::citing(
                ContentDigest::sha256(b"p"),
                RedactionBasis::RightsRequest,
                actor,
            ),
            Err(CoverageFault::AutomaticActorCannotExclude)
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// unmapped_forces_incomplete
// ---------------------------------------------------------------------------

/// `REQ-12-045` and `REQ-34-017`. One eligible segment with no status makes the
/// rendering `INCOMPLETE`, and the banner carries the count.
///
/// **What this row does not cover, and why that mattered.** It drops a segment
/// from the document and declares nothing, so the only path it drives is
/// `UNMAPPED`. `P2-A4` found that the *declared* path — the same segment left
/// out of the document and named `EXCLUDED_NON_SPEECH` — produced a `COMPLETE`
/// badge over a document holding one of twenty-one tokens, and this row saw
/// none of it. The declared path is driven below and again, on its own, in
/// `a_non_speech_declaration_cannot_delete_the_lecture`.
#[test]
fn unmapped_forces_incomplete() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "incomplete")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;

    let whole = whole_document(lineage, &manifest, seq)?;
    let report = validate(lineage, &whole, &manifest, &capture.recovery)?;
    let qa = clean_render(&whole)?;
    let complete = PdfArtifact::render(&whole, &report, &qa, ContentDigest::sha256(b"pdf"));
    assert_eq!(complete.completeness(), DocumentCompleteness::Complete);
    assert!(complete.completeness().is_complete());

    // Drop one segment from the document and nothing else changes.
    let mut short = DocumentBuilder::over(document_id("short")?, lineage, 1, &manifest)?;
    for index in 0..4 {
        short.push(whole_segment_node(
            &format!("u-{index}"),
            NodeKind::Paragraph,
            index,
            PreservationTransform::Punctuation,
        )?)?;
    }
    let mut placement = whole_segment_node(
        "u-cap",
        NodeKind::CapturePlacement,
        3,
        PreservationTransform::CapturePlacement,
    )?;
    placement.nearby_captures = vec![seq];
    short.push(placement)?;
    let short = short.finish()?;
    let report = validate(lineage, &short, &manifest, &capture.recovery)?;
    assert_eq!(report.unmapped_count(), 1);
    assert!(report.completeness_witness().is_none());
    let qa = clean_render(&short)?;
    let pdf = PdfArtifact::render(&short, &report, &qa, ContentDigest::sha256(b"pdf"));
    assert_eq!(
        pdf.completeness(),
        DocumentCompleteness::Incomplete {
            unmapped_segments: 1,
            render_defects: 0,
        }
    );
    assert!(!pdf.completeness().is_complete());
    assert_eq!(pdf.completeness().as_str(), "INCOMPLETE");

    // A witness minted for another document does not travel. A report of the
    // whole document handed to the short one's render is refused, so the
    // upgrade cannot be borrowed from a different measurement.
    let whole_report = validate(lineage, &whole, &manifest, &capture.recovery)?;
    let borrowed = PdfArtifact::render(&short, &whole_report, &qa, ContentDigest::sha256(b"pdf"));
    assert!(!borrowed.completeness().is_complete());

    // The same missing segment, now *declared* rather than left unmapped. The
    // unmapped count goes to zero and the segment ratio goes whole — the two
    // things this row measured — and the rendering is still `INCOMPLETE`,
    // because the segment's six tokens are in the token denominator and not in
    // the document. At `c81b74b` this branch read `COMPLETE`.
    let mut declared = DispositionLedger::new();
    declared.record(SegmentDisposition::excluded_non_speech(
        4,
        NonSpeechEvidence::declared(NonSpeechReason::Silence, user()?)?,
    ))?;
    let declared_report = validate_with(
        lineage,
        &short,
        &manifest,
        &capture.recovery,
        &declared,
        &CaptureExclusionLedger::new(),
    )?;
    assert_eq!(
        declared_report.unmapped_count(),
        0,
        "the declaration accounts for the segment this row dropped"
    );
    assert!(
        declared_report.segment_coverage().is_whole(),
        "the declared segment left the segment denominator, as section 12.6's segment line says"
    );
    assert_eq!(
        declared_report.token_coverage().denominator(),
        TOTAL_TOKENS,
        "the declared segment's tokens left the token denominator too"
    );
    assert!(declared_report.completeness_witness().is_none());
    let declared_pdf =
        PdfArtifact::render(&short, &declared_report, &qa, ContentDigest::sha256(b"pdf"));
    assert_eq!(declared_pdf.completeness().as_str(), "INCOMPLETE");
    assert!(!declared_pdf.completeness().is_complete());
    Ok(())
}

// ---------------------------------------------------------------------------
// lossless_transform_allowlist
// ---------------------------------------------------------------------------

/// `REQ-12-035`. The nine transforms are the specification's own, and a
/// rendering that drops or replaces a token is refused under every one of them.
#[test]
fn lossless_transform_allowlist() -> TestResult {
    // The whole set, read out of the specification rather than transcribed.
    let specification = specification()?;
    let sentence = specification
        .lines()
        .find(|line| line.starts_with("허용되는 것은 "))
        .ok_or("section 12.5's allow-list sentence moved")?;
    let (members, _rest) = sentence
        .trim_start_matches("허용되는 것은 ")
        .split_once("다. ")
        .ok_or("section 12.5's allow-list sentence does not end where it did")?;
    let listed: Vec<&str> = members.split(", ").map(str::trim).collect();
    let declared: Vec<&str> = PreservationTransform::ALL
        .iter()
        .map(|transform| transform.spec_phrase())
        .collect();
    assert_eq!(
        listed, declared,
        "the transform allow-list and section 12.5's sentence disagree"
    );

    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "allowlist")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();

    // Every transform accepts a rendering that preserves its tokens.
    let (_, verbatim, _) = SEGMENTS[0];
    let chars = verbatim.chars().count();
    for transform in PreservationTransform::ALL {
        let mut builder = DocumentBuilder::over(document_id("allow")?, lineage, 1, &manifest)?;
        builder.push(NodeDraft {
            id: node_id("a-0")?,
            kind: NodeKind::Paragraph,
            rendered_text: format!("[00:00] Instructor: {verbatim}."),
            mappings: vec![(0, 0, chars, transform)],
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        })?;
        assert!(
            builder.finish().is_ok(),
            "{} refused a rendering that preserves every token",
            transform.as_str()
        );
    }

    // And every transform refuses a deletion and a paraphrase. The rule does
    // not read the transform's name, which is why the loop is over all nine.
    for transform in PreservationTransform::ALL {
        let mut deleting = DocumentBuilder::over(document_id("delete")?, lineage, 1, &manifest)?;
        let refusal = deleting.push(NodeDraft {
            id: node_id("d-0")?,
            kind: NodeKind::Paragraph,
            // "the" is gone.
            rendered_text: "Instructor: serializability is goal.".to_owned(),
            mappings: vec![(0, 0, chars, transform)],
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        });
        assert_eq!(
            refusal,
            Err(DocumentFault::TokenNotPreserved {
                node: "d-0".to_owned(),
                segment_index: 0,
                // "the" is token two of "serializability is the goal".
                token_position: 2,
            }),
            "{} admitted a rendering with a word deleted",
            transform.as_str()
        );

        let mut paraphrasing =
            DocumentBuilder::over(document_id("paraphrase")?, lineage, 1, &manifest)?;
        let refusal = paraphrasing.push(NodeDraft {
            id: node_id("p-0")?,
            kind: NodeKind::Paragraph,
            // "serializability" became "correctness".
            rendered_text: "Instructor: correctness is the goal.".to_owned(),
            mappings: vec![(0, 0, chars, transform)],
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        });
        assert!(
            matches!(refusal, Err(DocumentFault::TokenNotPreserved { .. })),
            "{} admitted a paraphrase",
            transform.as_str()
        );

        // Reordering is a loss too: the tokens have to survive *in order*.
        let mut reordering = DocumentBuilder::over(document_id("reorder")?, lineage, 1, &manifest)?;
        let refusal = reordering.push(NodeDraft {
            id: node_id("r-0")?,
            kind: NodeKind::Paragraph,
            rendered_text: "Instructor: goal the is serializability.".to_owned(),
            mappings: vec![(0, 0, chars, transform)],
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        });
        assert!(
            matches!(refusal, Err(DocumentFault::TokenNotPreserved { .. })),
            "{} admitted a reordering",
            transform.as_str()
        );
    }

    // A spelling that is not one of the nine has no value at all.
    assert!(PreservationTransform::parse("SUMMARIZE").is_none());
    assert!(PreservationTransform::parse("DELETE_LOW_VALUE").is_none());
    for transform in PreservationTransform::ALL {
        assert_eq!(
            PreservationTransform::parse(transform.as_str()),
            Some(transform)
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// no_low_importance_deletion
// ---------------------------------------------------------------------------

/// `REQ-12-036` and `REQ-35-003`. Repetition, examples and digressions stay
/// mapped and rendered, and a summary that ranks them out changes nothing about
/// the document.
#[test]
fn no_low_importance_deletion() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "no-deletion")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;

    // Segment two is annotated `REPETITION` and `DIGRESSION`, and segment four
    // `EXAMPLE`. Those are exactly the three section 12.5 names.
    let document = whole_document(lineage, &manifest, seq)?;
    let annotated: Vec<&academic_lecture_document::DocumentNode> = document
        .nodes()
        .iter()
        .filter(|node| {
            node.annotations().iter().any(|annotation| {
                matches!(
                    annotation,
                    DocumentAnnotation::Repetition
                        | DocumentAnnotation::Example
                        | DocumentAnnotation::Digression
                )
            })
        })
        .collect();
    assert_eq!(
        annotated.len(),
        2,
        "the fixture stopped annotating low-value spans"
    );
    for node in &annotated {
        assert!(!node.rendered_text().is_empty());
        assert!(!node.mappings().is_empty());
    }

    let before = validate(lineage, &document, &manifest, &capture.recovery)?;
    assert!(before.segment_coverage().is_whole());
    assert!(before.token_coverage().is_whole());
    let before_bytes = before.canonical_bytes();
    let document_digest = document.digest();

    // A study index that keeps only the high-salience entries: a legitimate
    // summary that leaves things out.
    let mut index = StudyIndexBuilder::over(StudyIndexId::new("study-06")?, &document);
    for (position, node) in document.nodes().iter().enumerate() {
        let salience = if node.annotations().iter().any(|annotation| {
            matches!(
                annotation,
                DocumentAnnotation::Repetition
                    | DocumentAnnotation::Example
                    | DocumentAnnotation::Digression
            )
        }) {
            Salience::Low
        } else {
            Salience::High
        };
        if salience == Salience::Low {
            continue;
        }
        index.add(
            &format!("e-{position}"),
            "a heading",
            node.id().clone(),
            salience,
        )?;
    }
    let index = index.finish()?;
    assert!(
        index.entries().len() < document.nodes().len(),
        "the summary did not actually leave anything out, so the assertion below is vacuous"
    );

    // The document and its coverage are the same value afterwards.
    assert_eq!(document.digest(), document_digest);
    let after = validate(lineage, &document, &manifest, &capture.recovery)?;
    assert_eq!(after.canonical_bytes(), before_bytes);
    for node in &annotated {
        let account = after
            .accounts()
            .iter()
            .find(|account| {
                matches!(account.status(), SegmentStatus::Mapped { nodes } if nodes.contains(node.id()))
            })
            .ok_or("an annotated span left the accounting")?;
        assert_eq!(account.status().as_str(), "MAPPED");
    }
    assert_eq!(after.unmapped_count(), 0);
    Ok(())
}

// ---------------------------------------------------------------------------
// lecture_render_qa
// ---------------------------------------------------------------------------

/// `REQ-12-048` and `REQ-34-014`. Each of the four defects is caught by name
/// and each one denies the completeness the rendering would otherwise claim.
#[test]
fn lecture_render_qa() -> TestResult {
    let specification = specification()?;
    let sentence = specification
        .lines()
        .find(|line| line.starts_with("- 문서 render 후 "))
        .ok_or("section 12.6's render QA sentence moved")?;
    let listed: Vec<&str> = sentence
        .trim_start_matches("- 문서 render 후 ")
        .trim_end_matches("를 검사한다.")
        .split(", ")
        .map(str::trim)
        .collect();
    let declared: Vec<&str> = RenderDefect::ALL
        .iter()
        .map(|defect| defect.spec_phrase())
        .collect();
    assert_eq!(
        listed, declared,
        "the render QA defect set and section 12.6's sentence disagree"
    );

    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "render")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let document = whole_document(lineage, &manifest, seq)?;
    let report = validate(lineage, &document, &manifest, &capture.recovery)?;

    let clean = clean_render(&document)?;
    assert!(clean.is_clean());
    let pdf = PdfArtifact::render(&document, &report, &clean, ContentDigest::sha256(b"pdf"));
    assert!(pdf.completeness().is_complete());

    let nodes: Vec<RenderedNode> = document
        .nodes()
        .iter()
        .map(|node| RenderedNode {
            node: node.id().clone(),
            page: 1,
            clipped: false,
            missing_glyphs: 0,
        })
        .collect();
    let images = vec![RenderedImage {
        capture_frame_seq: seq,
        resolved: true,
    }];
    let good_page = RenderedPage {
        number: 1,
        content_height_units: 900,
        frame_height_units: 1_000,
    };

    // One injection per defect, each on its own.
    let overflow = RenderQa::inspect(
        &document,
        &[RenderedPage {
            number: 1,
            content_height_units: 1_400,
            frame_height_units: 1_000,
        }],
        &nodes,
        &images,
    )?;
    assert_eq!(overflow.of(RenderDefect::PageOverflow).len(), 1);
    assert_eq!(overflow.of(RenderDefect::PageOverflow)[0].page(), Some(1));

    let mut clipped = nodes.clone();
    clipped[3].clipped = true;
    let clipped = RenderQa::inspect(&document, &[good_page], &clipped, &images)?;
    assert_eq!(clipped.of(RenderDefect::ClippedCode).len(), 1);

    let missing = RenderQa::inspect(
        &document,
        &[good_page],
        &nodes,
        &[RenderedImage {
            capture_frame_seq: seq,
            resolved: false,
        }],
    )?;
    assert_eq!(missing.of(RenderDefect::MissingImage).len(), 1);
    assert_eq!(
        missing.of(RenderDefect::MissingImage)[0].capture_frame_seq(),
        Some(seq)
    );

    // A placed capture the render did not mention at all is the same defect.
    let unmentioned = RenderQa::inspect(&document, &[good_page], &nodes, &[])?;
    assert_eq!(unmentioned.of(RenderDefect::MissingImage).len(), 1);

    let mut broken = nodes.clone();
    broken[0].missing_glyphs = 4;
    let broken = RenderQa::inspect(&document, &[good_page], &broken, &images)?;
    assert_eq!(broken.of(RenderDefect::BrokenGlyph).len(), 1);

    // Every one of them denies completeness.
    for defective in [&overflow, &clipped, &missing, &unmentioned, &broken] {
        assert!(!defective.is_clean());
        let pdf = PdfArtifact::render(&document, &report, defective, ContentDigest::sha256(b"pdf"));
        assert!(
            !pdf.completeness().is_complete(),
            "a render defect did not deny completeness"
        );
    }

    // A measurement that does not describe this document is a refusal, not a
    // clean report: a partial render would otherwise report "clean" for the
    // half nobody looked at.
    assert!(RenderQa::inspect(&document, &[], &nodes, &images).is_err());
    assert!(RenderQa::inspect(&document, &[good_page], &nodes[..2], &images).is_err());
    assert!(
        RenderQa::inspect(
            &document,
            &[good_page],
            &nodes,
            &[RenderedImage {
                capture_frame_seq: seq + 500,
                resolved: true,
            }],
        )
        .is_err()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// study_index_disclosure
// ---------------------------------------------------------------------------

/// `REQ-12-037`, `REQ-02-003`, `REQ-02-006` and `REQ-35-004`. A summary is a
/// separate artifact, carries a disclosure it cannot drop, links back to its
/// source, and round-trips into it.
#[test]
fn study_index_disclosure() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "study-index")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let document = whole_document(lineage, &manifest, seq)?;

    let mut builder = StudyIndexBuilder::over(StudyIndexId::new("study-06")?, &document);
    for (position, node) in document.nodes().iter().enumerate() {
        builder.add(
            &format!("e-{position}"),
            "a heading",
            node.id().clone(),
            Salience::Medium,
        )?;
    }
    let index = builder.finish()?;

    // A distinct identifier type, a required source link, and the disclosure.
    assert_eq!(index.document(), document.id());
    assert_eq!(index.document_digest(), &document.digest());
    assert_eq!(
        index.disclosure(),
        academic_lecture_document::STUDY_INDEX_DISCLOSURE
    );
    assert!(index.disclosure().contains("does not replace"));
    assert!(!index.disclosure().is_empty());
    assert!(index.round_trips(&document));

    // Every entry links to a node that exists, and one that does not is
    // refused: an index that points nowhere has stopped being navigation.
    let mut dangling = StudyIndexBuilder::over(StudyIndexId::new("study-bad")?, &document);
    assert!(
        dangling
            .add("e-0", "a heading", node_id("nowhere")?, Salience::High)
            .is_err()
    );
    assert!(
        StudyIndexBuilder::over(StudyIndexId::new("study-empty")?, &document)
            .finish()
            .is_err()
    );

    // Two indexes over the same document carry the same disclosure, because it
    // is a constant rather than a value either of them was given.
    let mut second = StudyIndexBuilder::over(StudyIndexId::new("study-07")?, &document);
    second.add(
        "only",
        "a heading",
        document.nodes()[0].id().clone(),
        Salience::Low,
    )?;
    let second = second.finish()?;
    assert_eq!(second.disclosure(), index.disclosure());
    assert_ne!(second.id(), index.id());

    // And the index has no completeness of any kind: the report over the
    // document is what carries one, and the index cannot reach it.
    let report = validate(lineage, &document, &manifest, &capture.recovery)?;
    assert!(report.completeness_witness().is_some());
    assert_ne!(index.canonical_bytes(), report.canonical_bytes());
    Ok(())
}

// ---------------------------------------------------------------------------
// pdf_non_authority
// ---------------------------------------------------------------------------

/// `REQ-12-032`. The rendering is derived from the record and never the other
/// way round: discarding it changes nothing, and rebuilding it from the same
/// record produces the same value.
#[test]
fn pdf_non_authority() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "pdf")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let document = whole_document(lineage, &manifest, seq)?;
    let report = validate(lineage, &document, &manifest, &capture.recovery)?;
    let qa = clean_render(&document)?;

    let document_digest = document.digest();
    let report_bytes = report.canonical_bytes();
    let token_digest = *document.transcript_token_digest();

    let pdf = PdfArtifact::render(&document, &report, &qa, ContentDigest::sha256(b"rendered"));
    assert_eq!(pdf.document(), document.id());
    assert_eq!(pdf.document_digest(), &document_digest);

    // Discard the rendering. Nothing about the record moves.
    let bytes = pdf.canonical_bytes();
    drop(pdf);
    assert_eq!(document.digest(), document_digest);
    assert_eq!(report.canonical_bytes(), report_bytes);
    assert_eq!(document.transcript_token_digest(), &token_digest);
    assert_eq!(&lineage.raw().token_sequence_digest(), &token_digest);

    // Rebuild it. The same record renders the same artifact.
    let again = PdfArtifact::render(&document, &report, &qa, ContentDigest::sha256(b"rendered"));
    assert_eq!(again.canonical_bytes(), bytes);

    // A rendering of *other* bytes is a different artifact and still changes
    // nothing about the record.
    let altered = PdfArtifact::render(&document, &report, &qa, ContentDigest::sha256(b"tampered"));
    assert_ne!(
        altered.rendered_bytes_digest(),
        again.rendered_bytes_digest()
    );
    assert_ne!(altered.canonical_bytes(), bytes);
    assert_eq!(document.digest(), document_digest);
    assert_eq!(report.canonical_bytes(), report_bytes);
    assert_eq!(
        altered.completeness(),
        again.completeness(),
        "the bytes a renderer produced decided the completeness"
    );

    // And a rendering of a document the report is not about cannot be complete.
    let mut other = DocumentBuilder::over(document_id("other")?, lineage, 1, &manifest)?;
    other.push(whole_segment_node(
        "z-0",
        NodeKind::Paragraph,
        0,
        PreservationTransform::Punctuation,
    )?)?;
    let other = other.finish()?;
    let mismatched = PdfArtifact::render(&other, &report, &qa, ContentDigest::sha256(b"rendered"));
    assert!(!mismatched.completeness().is_complete());
    Ok(())
}

// ---------------------------------------------------------------------------
// paragraph_mapping_integrity
// ---------------------------------------------------------------------------

/// `REQ-12-034`. Every node records its identifier, its rendered text, its
/// segment/range/transform mappings, its nearby captures and its annotations,
/// and an out-of-range or dangling reference is refused.
#[test]
fn paragraph_mapping_integrity() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "mapping")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let document = whole_document(lineage, &manifest, seq)?;

    // Every one of section 12.5's five node kinds appears, and each carries an
    // ordered mapping that reconstructs the text it claims.
    let mut kinds: Vec<&str> = document
        .nodes()
        .iter()
        .map(|node| node.kind().as_str())
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(
        kinds,
        vec!["CAPTURE_PLACEMENT", "EQUATION", "PARAGRAPH", "SECTION"]
    );
    for node in document.nodes() {
        for mapping in node.mappings() {
            let segment = lineage
                .segment_at(1, mapping.segment_index())
                .ok_or("a mapping names a segment that is gone")?;
            assert_eq!(mapping.segment_id(), segment.id());
            let (start, end) = mapping.char_range();
            assert!(start < end);
            assert!(end <= segment.verbatim_text().chars().count());
            assert!(!mapping.covered_tokens().is_empty());
            let mut ascending = mapping.covered_tokens().to_vec();
            ascending.sort_unstable();
            assert_eq!(ascending, mapping.covered_tokens());
            for position in mapping.covered_tokens() {
                let token = &segment.tokens()[*position];
                assert!(
                    node.rendered_text().contains(token.text()),
                    "a covered token is not in the rendered text"
                );
            }
        }
    }

    let (_, verbatim, _) = SEGMENTS[0];
    let chars = verbatim.chars().count();

    // A dangling segment.
    let mut builder = DocumentBuilder::over(document_id("dangling")?, lineage, 1, &manifest)?;
    assert_eq!(
        builder.push(NodeDraft {
            id: node_id("g-0")?,
            kind: NodeKind::Paragraph,
            rendered_text: "anything".to_owned(),
            mappings: vec![(99, 0, 3, PreservationTransform::Punctuation)],
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        }),
        Err(DocumentFault::DanglingSegment {
            node: "g-0".to_owned(),
            segment_index: 99,
        })
    );

    // A range past the end of the verbatim text, and an empty range.
    for (start, end) in [(0, chars + 1), (3, 3), (5, 2)] {
        let refusal = builder.push(NodeDraft {
            id: node_id("g-1")?,
            kind: NodeKind::Paragraph,
            rendered_text: format!("Instructor: {verbatim}."),
            mappings: vec![(0, start, end, PreservationTransform::Punctuation)],
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        });
        assert!(matches!(
            refusal,
            Err(DocumentFault::CharRangeOutOfBounds { .. })
        ));
    }

    // A range inside the text that covers no token at all.
    let refusal = builder.push(NodeDraft {
        id: node_id("g-2")?,
        kind: NodeKind::Paragraph,
        rendered_text: format!("Instructor: {verbatim}."),
        // The single space between "serializability" and "is".
        mappings: vec![(0, 15, 16, PreservationTransform::Punctuation)],
        nearby_captures: Vec::new(),
        annotations: Vec::new(),
        cross_reference: None,
    });
    assert_eq!(
        refusal,
        Err(DocumentFault::MappingCoversNoToken {
            node: "g-2".to_owned(),
            segment_index: 0,
        })
    );

    // A dangling capture.
    let refusal = builder.push(NodeDraft {
        id: node_id("g-3")?,
        kind: NodeKind::Paragraph,
        rendered_text: format!("Instructor: {verbatim}."),
        mappings: vec![(0, 0, chars, PreservationTransform::Punctuation)],
        nearby_captures: vec![seq + 700],
        annotations: Vec::new(),
        cross_reference: None,
    });
    assert_eq!(
        refusal,
        Err(DocumentFault::DanglingCapture {
            node: "g-3".to_owned(),
            frame_seq: seq + 700,
        })
    );

    // A node that maps nothing, a duplicate identifier, unreadable text, and a
    // capture placement naming no capture.
    assert!(matches!(
        builder.push(NodeDraft {
            id: node_id("g-4")?,
            kind: NodeKind::Paragraph,
            rendered_text: "text".to_owned(),
            mappings: Vec::new(),
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        }),
        Err(DocumentFault::NodeMapsNothing(_))
    ));
    assert!(matches!(
        builder.push(NodeDraft {
            id: node_id("g-5")?,
            kind: NodeKind::Paragraph,
            rendered_text: "bad\ntext".to_owned(),
            mappings: vec![(0, 0, chars, PreservationTransform::Punctuation)],
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        }),
        Err(DocumentFault::MalformedRenderedText(_))
    ));
    assert!(matches!(
        builder.push(NodeDraft {
            id: node_id("g-6")?,
            kind: NodeKind::CapturePlacement,
            rendered_text: format!("Instructor: {verbatim}."),
            mappings: vec![(0, 0, chars, PreservationTransform::CapturePlacement)],
            nearby_captures: Vec::new(),
            annotations: Vec::new(),
            cross_reference: None,
        }),
        Err(DocumentFault::CapturePlacementNamesNoCapture(_))
    ));
    builder.push(whole_segment_node(
        "g-7",
        NodeKind::Paragraph,
        0,
        PreservationTransform::Punctuation,
    )?)?;
    assert!(matches!(
        builder.push(whole_segment_node(
            "g-7",
            NodeKind::Paragraph,
            1,
            PreservationTransform::Punctuation,
        )?),
        Err(DocumentFault::DuplicateNodeId(_))
    ));

    // A document built over another version, and one built over another
    // lecture's manifest, cannot be validated against this transcript.
    assert!(matches!(
        DocumentBuilder::over(document_id("v9")?, lineage, 9, &manifest),
        Err(DocumentFault::NoSuchVersion(9))
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// risky_span_review_context
// ---------------------------------------------------------------------------

/// `REQ-12-047` and `REQ-04-005`. Exactly three risk classes enter the queue,
/// every item names the audio it came from, and a calibrated confidence — never
/// a raw provider number — is what decides the third class.
#[test]
fn risky_span_review_context() -> TestResult {
    let directory = tempfile::tempdir()?;
    let capture = clean_capture(&directory, "review")?;
    let manifest = full_manifest(&capture)?;
    let transcribed = transcribe(&manifest)?;
    let lineage = transcribed.lineage();
    let seq = capture_frame_seq(&capture).ok_or("the fixture capture has no image")?;
    let document = whole_document(lineage, &manifest, seq)?;

    let queue = ReviewQueue::build(
        &document,
        lineage,
        &calibration()?,
        &purpose()?,
        INSIDE,
        COVERAGE_CONFIG_V1,
    )?;

    // The fixture's first three segments carry raw units above the curve's
    // break, and the last two below it; segment three is the equation.
    assert_eq!(queue.of(RiskClass::Equation).len(), 1);
    assert!(queue.of(RiskClass::Code).is_empty());
    assert_eq!(
        queue.of(RiskClass::LowConfidence).len(),
        2,
        "the low-confidence class did not pick out exactly the low-confidence spans"
    );
    for item in queue.items() {
        assert!(
            RiskClass::ALL.contains(&item.class()),
            "a fourth risk class reached the queue"
        );
        assert!(
            !item.audio().chunk_frame_seqs().is_empty(),
            "a review item is orphaned text with no audio"
        );
        assert!(item.audio().end_nanos() > item.audio().start_nanos());
    }

    // The two high-confidence paragraphs are not in the queue, which is the
    // half `REQ-04-005` calls excessive review.
    let queued: Vec<&str> = queue
        .items()
        .iter()
        .map(|item| item.node().as_str())
        .collect();
    assert!(!queued.contains(&"n-01"));
    assert!(!queued.contains(&"n-02"));

    // The capture placement beside the equation carries the image, so the
    // reviewer has the slide as well as the audio.
    let placement = queue
        .items()
        .iter()
        .find(|item| item.node().as_str() == "n-06")
        .ok_or("the capture placement did not reach the queue")?;
    assert_eq!(placement.nearby_captures(), &[seq]);

    // Fail closed: with no calibration dataset a provider number cannot be
    // read, and a span with an unreadable number is looked at rather than
    // trusted.
    let unreadable = ReviewQueue::build(
        &document,
        lineage,
        &no_calibration(),
        &purpose()?,
        INSIDE,
        COVERAGE_CONFIG_V1,
    )?;
    assert!(unreadable.items().len() > queue.items().len());
    for item in unreadable.items() {
        if item.class() == RiskClass::LowConfidence {
            assert_eq!(
                item.calibrated_permille(),
                None,
                "a permille was reported without a dataset to read it through"
            );
        }
    }

    // And a raw provider number is never what decides: raising the configured
    // permille above the curve's high bin puts every span in the queue, and
    // lowering it below the low bin empties the low-confidence class.
    let permissive = academic_lecture_document::CoverageConfig::new(1, 2_000_000_000, 0)?;
    let none_low = ReviewQueue::build(
        &document,
        lineage,
        &calibration()?,
        &purpose()?,
        INSIDE,
        permissive,
    )?;
    assert!(none_low.of(RiskClass::LowConfidence).is_empty());
    assert_eq!(none_low.of(RiskClass::Equation).len(), 1);
    Ok(())
}

// ---------------------------------------------------------------------------
// The recorded defaults
// ---------------------------------------------------------------------------

/// The plan's `P2-L4` row: the confidence and gap thresholds are versioned
/// configuration **with recorded defaults**.
///
/// A constant in code and nothing in the contract is the shape the row exists
/// to prevent, so the table is read out of the contract page and compared
/// against the constants rather than transcribed here.
#[test]
fn the_recorded_defaults_are_the_documented_ones() -> TestResult {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("contracts")
        .join("lecture-document.md");
    let page = std::fs::read_to_string(path)?;
    let mut documented: Vec<(String, String)> = Vec::new();
    for line in page.lines() {
        let mut cells = line.split('|').map(str::trim);
        if cells.next() != Some("") {
            continue;
        }
        let (Some(field), Some(value)) = (cells.next(), cells.next()) else {
            continue;
        };
        let field = field.trim_matches('`');
        if [
            "version",
            "gap_threshold_nanos",
            "low_confidence_at_or_below_permille",
        ]
        .contains(&field)
        {
            documented.push((field.to_owned(), value.to_owned()));
        }
    }
    assert_eq!(
        documented,
        vec![
            (
                "version".to_owned(),
                COVERAGE_CONFIG_V1.version().to_string()
            ),
            (
                "gap_threshold_nanos".to_owned(),
                COVERAGE_CONFIG_V1.gap_threshold_nanos().to_string()
            ),
            (
                "low_confidence_at_or_below_permille".to_owned(),
                COVERAGE_CONFIG_V1
                    .low_confidence_at_or_below_permille()
                    .to_string()
            ),
        ],
        "the contract page's defaults table and COVERAGE_CONFIG_V1 disagree"
    );

    // The reader is not vacuous: it found three rows, and it finds none in a
    // page that has no such table.
    assert_eq!(documented.len(), 3);

    // And the configuration refuses what it says it refuses.
    assert!(academic_lecture_document::CoverageConfig::new(0, 1, 1).is_err());
    assert!(academic_lecture_document::CoverageConfig::new(1, 1, 1001).is_err());
    assert!(academic_lecture_document::CoverageConfig::new(1, 0, 1000).is_ok());
    Ok(())
}
