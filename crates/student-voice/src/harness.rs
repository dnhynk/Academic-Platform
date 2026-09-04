//! The committed corpus files, rendered from one builder.
//!
//! `docs/contracts/engine-harness.md` says an entry that gains committed
//! fixtures without a second half that *executes* them "has satisfied the audit
//! and demonstrated nothing", and `CONTRIBUTING.md` rule 5 says a golden file
//! is updated only through the deterministic builder. Diarization is not one of
//! §28's twelve engines -- the registry is pinned to that table and a
//! thirteenth entry would fail `engine_registry_is_complete` -- so what this
//! module borrows is the discipline rather than the registry: a corpus rendered
//! from a builder, committed, and re-rendered and byte-compared by the suite,
//! with the real scorer run over every case.
//!
//! It is product code rather than a test so that `cargo run --example
//! emit_corpus` writes the same bytes the suite compares, exactly as
//! `academic_lecture_document::harness` does for `TRANSCRIPT_COVERAGE`.
//!
//! **Nothing here reads a file.** It returns bytes; the example writes them and
//! the suite compares them.

use crate::{
    corpus::{CORPUS_ROOT, DiarizationCorpus, corpus_v1},
    fault::CorpusFault,
    measure::{measure, measure_case},
};

/// One rendered file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusFile {
    /// Its repository-relative path.
    pub path: String,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

/// The directory the shipped corpus version writes into.
#[must_use]
pub fn corpus_dir(corpus: &DiarizationCorpus) -> String {
    format!("{CORPUS_ROOT}/v{}", corpus.version())
}

/// Renders every committed file of the shipped corpus.
///
/// Per case, an `.input` holding the case as the corpus grammar writes it and
/// an `.expected` holding what the real scorer produced over it. Then one
/// `corpus.digest` naming the whole corpus, and one `measurement.expected`
/// holding the fold and both ratios -- which is the number the contract page
/// publishes.
///
/// # Errors
///
/// [`CorpusFault`] if the shipped corpus literals have been edited into an
/// invalid timeline.
pub fn corpus_files() -> Result<Vec<CorpusFile>, CorpusFault> {
    let corpus = corpus_v1()?;
    let directory = corpus_dir(&corpus);
    let mut files = Vec::new();
    for case in corpus.cases() {
        files.push(CorpusFile {
            path: format!("{directory}/{}.input", case.name()),
            bytes: case.canonical_bytes(),
        });
        files.push(CorpusFile {
            path: format!("{directory}/{}.expected", case.name()),
            bytes: measure_case(case).canonical_bytes(),
        });
    }
    files.push(CorpusFile {
        path: format!("{directory}/corpus.digest"),
        bytes: format!("{}\n", corpus.digest()).into_bytes(),
    });
    files.push(CorpusFile {
        path: format!("{directory}/measurement.expected"),
        bytes: measure(&corpus).canonical_bytes(),
    });
    Ok(files)
}
