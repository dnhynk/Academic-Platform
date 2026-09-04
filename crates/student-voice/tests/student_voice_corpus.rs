//! The committed corpus is executed and byte-compared, not counted.
//!
//! `docs/contracts/engine-harness.md` says a set of fixtures that only exists
//! "has satisfied the audit and demonstrated nothing", and `P2-L4`'s
//! `harness_corpus_matches_a_fresh_render` is the shape that answers it. This
//! file is that shape for the diarization corpus: every committed `.input` is
//! read off disk, parsed back into a case, scored by the **real** scorer, and
//! byte-compared against the committed `.expected`; and the whole directory is
//! re-rendered from the builder and compared, so a fixture edited by hand into
//! agreement with a broken scorer fails rather than passes.
//!
//! It also walks the directory, so a file nobody rendered is a failure rather
//! than a file nothing reads.

mod common;

use std::{collections::BTreeSet, fs, path::PathBuf};

use academic_student_voice::{
    CORPUS_ID, CORPUS_VERSION, DiarizationCase, DiarizationCorpus, VoiceSpan, corpus_v1,
    harness::{corpus_dir, corpus_files},
    measure, measure_case,
};
use academic_transcription::Speaker;

use common::TestResult;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The committed corpus is exactly what the builder renders.
///
/// `CONTRIBUTING.md` rule 5 in executable form. Without this the `.expected`
/// files could be regenerated from a broken scorer, or edited by hand, and
/// every row below would still pass.
#[test]
fn corpus_matches_a_fresh_render() -> TestResult {
    let root = repository_root();
    let rendered = corpus_files()?;
    let corpus = corpus_v1()?;
    // Two files per case plus the digest and the fold.
    assert_eq!(rendered.len(), corpus.cases().len() * 2 + 2);
    for file in &rendered {
        let path = root.join(&file.path);
        let committed = fs::read(&path).map_err(|error| format!("{}: {error}", file.path))?;
        assert_eq!(
            committed, file.bytes,
            "{} differs from a fresh render; re-run `cargo run -p academic-student-voice \
             --example emit_corpus` and explain the change",
            file.path
        );
    }

    // And nothing extra hides under the corpus directory.
    let rendered_paths: BTreeSet<String> = rendered.iter().map(|file| file.path.clone()).collect();
    let directory = root.join(corpus_dir(&corpus));
    let mut walked = BTreeSet::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        assert!(
            entry.file_type()?.is_file(),
            "the corpus directory holds a subdirectory nothing renders"
        );
        walked.insert(format!(
            "{}/{}",
            corpus_dir(&corpus),
            entry.file_name().to_string_lossy()
        ));
    }
    assert_eq!(walked, rendered_paths);
    Ok(())
}

/// Every committed case is parsed back off disk and scored by the real scorer.
///
/// This is the half that makes the corpus evidence rather than decoration: the
/// `.input` is re-read and re-parsed rather than taken from the builder that
/// wrote it, so a case whose committed bytes do not describe the case the
/// builder holds fails here.
#[test]
fn every_committed_case_is_executed_and_byte_compared() -> TestResult {
    let root = repository_root();
    let corpus = corpus_v1()?;
    let directory = root.join(corpus_dir(&corpus));
    let mut executed = 0_usize;
    for case in corpus.cases() {
        let input = fs::read_to_string(directory.join(format!("{}.input", case.name())))?;
        let parsed = parse_case(&input)?;
        assert_eq!(
            &parsed,
            case,
            "{} parsed back to a different case",
            case.name()
        );
        let expected = fs::read(directory.join(format!("{}.expected", case.name())))?;
        assert_eq!(
            measure_case(&parsed).canonical_bytes(),
            expected,
            "{} scored differently from its committed expectation",
            case.name()
        );
        executed += 1;
    }
    assert_eq!(executed, corpus.cases().len());
    assert!(executed >= 6, "the shipped corpus lost cases");

    // The digest and the fold, off disk.
    let committed_digest = fs::read_to_string(directory.join("corpus.digest"))?;
    assert_eq!(committed_digest.trim(), corpus.digest().to_string());
    let committed_fold = fs::read(directory.join("measurement.expected"))?;
    assert_eq!(measure(&corpus).canonical_bytes(), committed_fold);

    // The committed corpus is the one the identity names.
    assert_eq!(corpus.id(), CORPUS_ID);
    assert_eq!(corpus.version(), CORPUS_VERSION);
    Ok(())
}

/// Parses one committed `.input` back into a case.
///
/// Written here rather than in the crate on purpose: a parser the crate owned
/// would be the crate agreeing with itself, which is the `P2-L3` oracle defect.
/// This one is an independent transcription of the grammar.
fn parse_case(text: &str) -> Result<DiarizationCase, Box<dyn std::error::Error>> {
    let mut lines = text.lines();
    let banner = lines.next().ok_or("the case file is empty")?;
    let name = banner
        .strip_prefix("diarization-case/1 ")
        .ok_or("the case file has no banner")?;
    let mut reference = Vec::new();
    let mut hypothesis = Vec::new();
    for line in lines {
        let mut fields = line.split(' ');
        let timeline = fields.next().ok_or("a span line has no timeline")?;
        let start: u64 = fields.next().ok_or("a span line has no start")?.parse()?;
        let end: u64 = fields.next().ok_or("a span line has no end")?.parse()?;
        let speaker = fields.next().ok_or("a span line has no speaker")?;
        assert!(fields.next().is_none(), "a span line has an extra field");
        let speaker = Speaker::parse(speaker).ok_or("a span line has an unknown speaker")?;
        let span = VoiceSpan::new(start, end, speaker);
        match timeline {
            "reference" => reference.push(span),
            "hypothesis" => hypothesis.push(span),
            other => return Err(format!("unknown timeline {other}").into()),
        }
    }
    Ok(DiarizationCase::new(name, reference, hypothesis)?)
}

/// The corpus grammar round-trips, so the parser above is reading the same
/// thing the renderer wrote.
#[test]
fn the_corpus_grammar_round_trips() -> TestResult {
    let corpus = corpus_v1()?;
    let mut cases = Vec::new();
    for case in corpus.cases() {
        let text = String::from_utf8(case.canonical_bytes())?;
        cases.push(parse_case(&text)?);
    }
    let round_tripped = DiarizationCorpus::new(corpus.id(), corpus.version(), cases)?;
    assert_eq!(round_tripped.canonical_bytes(), corpus.canonical_bytes());
    assert_eq!(round_tripped.digest(), corpus.digest());
    Ok(())
}
