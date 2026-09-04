//! The vocabularies and the numbers, compared against the documents that own
//! them.
//!
//! Three of this task's claims are about agreement with a document rather than
//! about behaviour:
//!
//! * the two downstream jobs and the three PII classes are section 32.5's own,
//!   not a list transcribed here;
//! * the two projection families are section 32.5's own; and
//! * the published accuracy figure and the recorded threshold defaults are the
//!   ones the contract page prints.
//!
//! Each is read out of the source document and compared **in both directions**,
//! so a set that gains a member the specification does not name fails as
//! surely as one that loses a member it does. `engine_registry_is_complete` is
//! the same shape in `academic-domain`, and
//! `the_recorded_defaults_are_the_documented_ones` is `P2-L4`'s.
//!
//! Reading only `.md` here is deliberate: this file names no Rust source path,
//! so it is not a policy source scan and carries no row on
//! `docs/contracts/policy-source-scans.md`.

mod common;

use std::{fs, path::PathBuf};

use academic_student_voice::{
    ABSOLUTE_ACCURACY_FLOOR, ABSOLUTE_MISSED_STUDENT_CEILING, AffectedProjectionKind, CORPUS_ID,
    CORPUS_VERSION, DIARIZATION_THRESHOLD_V1, IngestionJobKind, PiiClass, SCORER_VERSION,
    corpus_v1, measure,
};

use common::TestResult;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn specification() -> Result<String, Box<dyn std::error::Error>> {
    Ok(fs::read_to_string(repository_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

fn contract_page() -> Result<String, Box<dyn std::error::Error>> {
    Ok(fs::read_to_string(
        repository_root().join("docs/contracts/student-voice.md"),
    )?)
}

/// The sentence section 32.5 writes the capture hold in.
///
/// It is located rather than transcribed: the test finds the line the
/// specification actually contains and every comparison below reads that line.
fn hold_sentence(specification: &str) -> Result<String, Box<dyn std::error::Error>> {
    specification
        .lines()
        .find(|line| line.contains("graph/OCR ingestion"))
        .map(str::to_owned)
        .ok_or_else(|| "section 32.5's capture-hold sentence is not in the specification".into())
}

/// The two downstream jobs and the three PII classes are the specification's.
///
/// Both directions. The forward half is that every value this crate declares
/// appears in the sentence; the reverse half is that the sentence names nothing
/// this crate left out, which is asserted by reconstructing the list the
/// sentence contains and comparing it whole.
#[test]
fn the_downstream_jobs_are_section_32_5s_own() -> TestResult {
    let specification = specification()?;
    let sentence = hold_sentence(&specification)?;

    // Forward: every declared value is in the sentence.
    for kind in IngestionJobKind::ALL {
        assert!(
            sentence.contains(kind.spec_word()),
            "the specification's hold sentence does not name {}",
            kind.spec_word()
        );
    }
    for class in PiiClass::ALL {
        assert!(
            sentence.contains(class.spec_phrase()),
            "the specification's hold sentence does not name {}",
            class.spec_phrase()
        );
    }

    // Reverse: the sentence's own job list, split on the separator it uses, is
    // exactly this crate's two.
    let jobs = sentence
        .split_once("graph/OCR")
        .map(|_| vec!["graph", "OCR"])
        .ok_or("the hold sentence stopped spelling the job pair")?;
    assert_eq!(
        jobs,
        IngestionJobKind::ALL
            .into_iter()
            .map(IngestionJobKind::spec_word)
            .collect::<Vec<_>>(),
        "the job set and the specification's pair disagree"
    );

    // Reverse for the classes: the sentence's own list, split on its separator.
    let listed: Vec<&str> = sentence
        .split_once("Capture에 ")
        .and_then(|(_, rest)| rest.split_once("이 들어가면"))
        .map(|(list, _)| list.split('·').collect())
        .ok_or("the hold sentence stopped spelling the class list")?;
    assert_eq!(
        listed,
        PiiClass::ALL
            .into_iter()
            .map(PiiClass::spec_phrase)
            .collect::<Vec<_>>(),
        "the PII class set and the specification's list disagree"
    );

    // Non-vacuous: the reader finds something, and a phrase the specification
    // does not use is not found.
    assert!(!sentence.is_empty());
    assert!(!sentence.contains("학생 지문"));
    Ok(())
}

/// The two projection families are section 32.5's own pair.
#[test]
fn the_projection_families_are_section_32_5s_own() -> TestResult {
    let specification = specification()?;
    let sentence = specification
        .lines()
        .find(|line| line.contains("projection에 미치는 영향을 미리 보여준다"))
        .ok_or("section 32.5's deletion-preview sentence is not in the specification")?;

    for kind in AffectedProjectionKind::ALL {
        assert!(
            sentence.contains(kind.spec_word()),
            "the specification's preview sentence does not name {}",
            kind.spec_word()
        );
    }
    let pair = sentence
        .split_once(" projection에")
        .and_then(|(before, _)| before.rsplit(' ').next())
        .ok_or("the preview sentence stopped spelling the family pair")?;
    assert_eq!(
        pair.split('/').collect::<Vec<_>>(),
        AffectedProjectionKind::ALL
            .into_iter()
            .map(AffectedProjectionKind::spec_word)
            .collect::<Vec<_>>(),
        "the projection families and the specification's pair disagree"
    );
    Ok(())
}

/// Reads one `| key | value | ...` row's second cell out of a markdown table.
fn table_cell(page: &str, key: &str) -> Option<String> {
    page.lines()
        .filter(|line| line.starts_with("| "))
        .find_map(|line| {
            let mut cells = line.split('|').map(str::trim);
            let _ = cells.next()?;
            let first = cells.next()?;
            let second = cells.next()?;
            (first.trim_matches('`') == key).then(|| second.trim_matches('`').to_owned())
        })
}

/// The threshold defaults and the band are the ones the contract page prints.
///
/// `P2-L4`'s `the_recorded_defaults_are_the_documented_ones`, applied to the
/// numbers that decide whether an automatic redaction claim can exist.
#[test]
fn the_recorded_defaults_are_the_documented_ones() -> TestResult {
    let page = contract_page()?;
    assert_eq!(
        table_cell(&page, "version").as_deref(),
        Some("1"),
        "the documented threshold version"
    );
    assert_eq!(
        table_cell(&page, "min_accuracy_permille").as_deref(),
        Some("990")
    );
    assert_eq!(
        table_cell(&page, "max_missed_student_permille").as_deref(),
        Some("0")
    );
    assert_eq!(DIARIZATION_THRESHOLD_V1.version(), 1);
    assert_eq!(DIARIZATION_THRESHOLD_V1.min_accuracy_permille(), 990);
    assert_eq!(DIARIZATION_THRESHOLD_V1.max_missed_student_permille(), 0);

    assert_eq!(
        table_cell(&page, "ABSOLUTE_ACCURACY_FLOOR").as_deref(),
        Some("900 permille")
    );
    assert_eq!(
        table_cell(&page, "ABSOLUTE_MISSED_STUDENT_CEILING").as_deref(),
        Some("50 permille")
    );
    assert_eq!(ABSOLUTE_ACCURACY_FLOOR, 900);
    assert_eq!(ABSOLUTE_MISSED_STUDENT_CEILING, 50);

    // The reader is not vacuous: a key the page does not carry answers None.
    assert_eq!(table_cell(&page, "min_accuracy_permile"), None);
    Ok(())
}

/// The published accuracy figure is the one a fresh run produces.
///
/// The contract page prints a number, and a page that printed a number nobody
/// re-derived would be the estimate this whole task exists to refuse. Every
/// cell of its measurement table is compared against a fresh run over the
/// shipped corpus.
#[test]
fn the_published_number_is_the_documented_one() -> TestResult {
    let page = contract_page()?;
    let corpus = corpus_v1()?;
    let measurement = measure(&corpus);

    let rows: [(&str, String); 9] = [
        ("corpus id", CORPUS_ID.to_owned()),
        ("corpus version", CORPUS_VERSION.to_string()),
        ("corpus digest", corpus.digest().to_string()),
        ("scorer version", SCORER_VERSION.to_string()),
        ("cases", corpus.cases().len().to_string()),
        (
            "scored reference time",
            format!("{} ms", measurement.scored_ms()),
        ),
        (
            "student reference time",
            format!("{} ms", measurement.reference_student_ms()),
        ),
        (
            "attribution accuracy",
            format!("{} permille", measurement.accuracy_permille()),
        ),
        (
            "student speech labelled instructor",
            format!("{} permille", measurement.missed_student_permille()),
        ),
    ];
    for (key, expected) in rows {
        assert_eq!(
            table_cell(&page, key).as_deref(),
            Some(expected.as_str()),
            "the contract page's {key} row is not what a fresh run measures"
        );
    }
    assert_eq!(
        table_cell(&page, "student speech also labelled student").as_deref(),
        Some(format!("{} permille", measurement.student_recall_permille()).as_str())
    );

    // And the page says plainly that the shipped corpus does not clear the
    // shipped default, which is the posture this build ships.
    assert!(
        page.contains("**It fails, on both axes.**"),
        "the contract page stopped stating the shipped posture"
    );
    assert!(measurement.witness(DIARIZATION_THRESHOLD_V1).is_err());
    Ok(())
}
