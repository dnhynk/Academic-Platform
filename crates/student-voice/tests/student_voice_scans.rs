//! Source scans for the `P2-L5` boundary.
//!
//! Six of this task's claims are statements about what the source does not
//! contain, and a behavioural test cannot observe an absence:
//!
//! * an accuracy witness has one producer and it is a run over a corpus;
//! * an admitted capture has one producer and it is inside the hold's door;
//! * a derivative's retention has one producer and it is `P2-G6`'s function;
//! * no product file here produces or names an `OriginalVoiceAuthority`, which
//!   is how `GATE-38-026` stays open;
//! * no floating-point type or literal reaches a number that decides whether
//!   somebody's voice may be processed automatically; and
//! * nothing here opens a clock, a socket, a file or a process.
//!
//! Every shape here is one `docs/contracts/policy-source-scans.md` already
//! records, and the two failure modes this Run has measured repeatedly are
//! avoided by construction:
//!
//! * **Whole-set, never a token list.** `P2-R2` measured five guards in a row
//!   failing because each asked whether a name was on a list of forbidden
//!   spellings; a bypass that spells nothing on the list walks past all five.
//!   Every rule below compares a **complete set** against a pinned list, so an
//!   unforeseen spelling fails as an extra key.
//! * **A pin fixes its callers too.** `T141` left a pinned check byte-identical
//!   and wrapped the call to it in a condition. Each pin below is accompanied
//!   by a count of the sites that reach it.
//! * **A sweep over signatures is not a claim about constructions.** `U-G3`: a
//!   second entry point can build its argument in its body and name the type
//!   nowhere in its signature, so the closed types below are counted where they
//!   are *built*.
//!
//! The helper block is `crates/lecture-document/tests/lecture_document_scans.rs`
//! copied verbatim, which is that file's own note about
//! `crates/transcription/tests/transcription_scans.rs`: a test module is not a
//! library target, and a stripper without raw strings desynchronizes.

mod common;

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use common::TestResult;

/// Constructions of `Name { .. }`, less every declaration form.
///
/// `constructions_of` subtracts `struct`, `impl` and `for`. A **fourth** form
/// spells the same three characters and it is not a construction: a function
/// whose return type is the struct, written `-> Name {`. Counting it made "one
/// producer" read as two the first time this rule ran, which is exactly the
/// class of miscount `declarations_of` exists to fix one level up.
fn built_count(code: &str, name: &str) -> usize {
    constructions_of(code, name).saturating_sub(occurrences(code, &format!("-> {name} {{")))
}

/// The counter is not vacuous and does not over-subtract.
#[test]
fn the_construction_counter_reads_a_literal_and_not_a_return_type() {
    let sample = "pub struct Thing { a: u8, } impl Thing { } fn make() -> Thing { Thing { a: 1 } }";
    assert_eq!(built_count(sample, "Thing"), 1);
    let returns_only = "fn make() -> Thing { other() }";
    assert_eq!(built_count(returns_only, "Thing"), 0);
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Every `.rs` file anywhere under this crate's package directory.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships, which is every one outside `tests`.
///
/// `benches` is deliberately **not** excluded beside `tests`: `S-14` records
/// that a bench target has no feature gate and is compiled by
/// `cargo clippy --workspace --all-targets`, which is the README's third
/// verification command.
fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    Ok(crate_all_sources()?
        .into_iter()
        .filter(|path| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            !relative.starts_with("tests")
        })
        .collect())
}

fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Every `.rs` file under every package in `crates/`.
///
/// The workspace half of the raw-token rules. These types are public and any
/// crate could declare the accessor this one does not, which is the shape
/// `a_label_has_no_path_that_moves_a_mark` in `academic-capture` already uses.
fn workspace_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    let crates = workspace_root().join("crates");
    for entry in fs::read_dir(&crates)? {
        let package = entry?.path();
        if package.is_dir() {
            walk(&package, &mut found)?;
        }
    }
    found.sort();
    Ok(found)
}

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/record/tests/record_scans.rs`, which is where this
/// repository's Rust-side stripper lives, raw strings and nested block comments
/// included. `P2-G4` found that a lexer without raw strings desynchronizes and
/// reads every literal after one as code, so the copy is deliberate rather than
/// a simplification.
fn strip_non_code(source: &str) -> String {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();

        if current == '/' && next == Some('/') {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if current == '/' && next == Some('*') {
            let mut depth = 1_usize;
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index] == '/' && bytes.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if bytes[index] == '*' && bytes.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            continue;
        }
        if current == 'r' && matches!(next, Some('"') | Some('#')) {
            let mut hashes = 0_usize;
            let mut probe = index + 1;
            while bytes.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if bytes.get(probe) == Some(&'"') {
                index = probe + 1;
                loop {
                    if index >= bytes.len() {
                        break;
                    }
                    if bytes[index] == '"' {
                        let mut closing = 0_usize;
                        while bytes.get(index + 1 + closing) == Some(&'#') {
                            closing += 1;
                        }
                        if closing >= hashes {
                            index += 1 + hashes;
                            break;
                        }
                    }
                    index += 1;
                }
                out.push(' ');
                continue;
            }
        }
        if current == '"' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == '\\' {
                    index += 2;
                    continue;
                }
                if bytes[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            out.push(' ');
            continue;
        }
        if current == '\'' {
            // A lifetime, not a character literal, when no quote closes it
            // within two characters. `S-7` records the width of this rule.
            let closes = bytes.get(index + 2) == Some(&'\'')
                || (bytes.get(index + 1) == Some(&'\\') && bytes.get(index + 3) == Some(&'\''));
            if closes {
                while index < bytes.len() && bytes[index] != '\'' {
                    index += 1;
                }
                index += 1;
                while index < bytes.len() && bytes[index] != '\'' {
                    index += 1;
                }
                index += 1;
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

fn code_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(path)?))
}

fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The whole text of one item, whitespace-collapsed, comments dropped.
fn declared_item(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{signature} has no closing brace at column zero"))?;
    let body = &source[start..start + end + 2];
    let kept: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    Ok(kept
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

/// How many times `needle` appears in `code`.
fn occurrences(code: &str, needle: &str) -> usize {
    code.split(needle).count().saturating_sub(1)
}

/// Counts whole-identifier occurrences of `name` in already-stripped code.
///
/// `occurrences` counts a spelling, which is right for a fixed phrase and wrong
/// for a name. `P2-RF10` reached a fourth exposure site by writing
/// `Untrusted::expose(d)` past a count of `.expose()`; injection `L3-I2` below
/// is the same shape against this crate's accessor.
fn uses_of(code: &str, name: &str) -> usize {
    let bytes = code.as_bytes();
    code.match_indices(name)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + name.len()).copied().unwrap_or(b' ');
            before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
        })
        .count()
}

/// Counts declarations of a function whose name is exactly `name`.
///
/// What follows the name has to open a parameter list or a generic list and
/// nothing else, so `fn response_bytes_rendered(` is not `response_bytes` and
/// `fn quote<'a>(` still is. `P2-RF11` found that reading the declaration as a
/// *spelling* lets one function cancel its own call; injection `L3-I3` is that
/// shape.
fn declarations_of(code: &str, name: &str) -> usize {
    let needle = format!("fn {name}");
    let bytes = code.as_bytes();
    code.match_indices(&needle)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + needle.len()).copied().unwrap_or(b' ');
            before_ok && (after == b'(' || after == b'<')
        })
        .count()
}

/// The use count of `name` less its declarations, which cannot go negative.
fn calls_of(code: &str, name: &str) -> usize {
    let uses = uses_of(code, name);
    let declarations = declarations_of(code, name);
    assert!(
        uses >= declarations,
        "{name} is declared {declarations} times and named {uses}; the two counts disagree"
    );
    uses - declarations
}

/// Drops every `use` item, so a re-export is not counted as a caller.
///
/// Whole items, not first lines. A `use crate::{ ... }` block spans several
/// lines and a filter that dropped only the line beginning `use ` left the
/// names inside it in the text -- which is how the first version of the
/// decoder's call count read three callers where there is one. A `pub use`
/// re-export is dropped for the same reason: it names a function and calls
/// nothing.
fn drop_use_items(code: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim_start();
        if !inside && (trimmed.starts_with("use ") || trimmed.starts_with("pub use ")) {
            inside = !trimmed.trim_end().ends_with(';');
            continue;
        }
        if inside {
            inside = !line.trim_end().ends_with(';');
            continue;
        }
        kept.push(line);
    }
    kept.join(
        "
",
    )
}

/// Every `pub fn`, `pub const fn`, `pub async fn` and `pub unsafe fn`
/// signature in `code`, whitespace-collapsed.
fn public_signatures(code: &str) -> Vec<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if ![
            "pub fn ",
            "pub const fn ",
            "pub async fn ",
            "pub unsafe fn ",
        ]
        .iter()
        .any(|start| trimmed.starts_with(start))
        {
            continue;
        }
        let mut signature = String::new();
        for follow in lines.iter().skip(index) {
            signature.push(' ');
            signature.push_str(follow.trim());
            if follow.contains('{') || follow.trim_end().ends_with(';') {
                break;
            }
        }
        found.push(signature.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    found
}

/// Splits a signature into its parameter list and its return type.
fn parameters_and_return(signature: &str) -> Option<(&str, &str)> {
    let open = signature.find('(')?;
    let mut depth = 0_usize;
    for (offset, character) in signature.get(open..)?.char_indices() {
        let at = open.saturating_add(offset);
        match character {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let (parameters, rest) = signature.split_at(at.saturating_add(1));
                    let returns = rest.split_once("->").map_or("", |(_, tail)| tail);
                    return Some((parameters, returns));
                }
            }
            _ => (),
        }
    }
    None
}

/// The public method names of one `impl` block, in declaration order.
///
/// `declared_item` collapses a whole block to one line, which is what a text
/// pin compares. This reads the same block as a **set of names**, which is what
/// an API-surface rule needs: a method added anywhere in the block fails as an
/// extra key whatever it is called, and a text pin over a 120-line block would
/// fail on a reflowed doc comment as well.
fn public_methods(source: &str, header: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let block = declared_item(source, header)?;
    let mut found = Vec::new();
    let mut rest = block.as_str();
    while let Some(at) = rest.find("fn ") {
        let before = &rest[..at];
        let public = before.ends_with("pub ") || before.ends_with("pub const ");
        rest = &rest[at + 3..];
        if !public {
            continue;
        }
        let name: String = rest
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            found.push(name);
        }
    }
    found.sort();
    Ok(found)
}

/// Counts constructions of `Name { .. }`, less the declarations that spell the
/// same three characters.
///
/// A bare `Name {` count reads a struct declaration, an inherent `impl` and a
/// trait `impl` as constructions. Subtracting them is the same repair
/// `declarations_of` is to `uses_of` one level up: the count has to be of the
/// thing, not of the spelling.
fn constructions_of(code: &str, name: &str) -> usize {
    let literal = format!("{name} {{");
    let total = occurrences(code, &literal);
    let declarations = occurrences(code, &format!("struct {literal}"))
        .saturating_add(occurrences(code, &format!("impl {literal}")))
        .saturating_add(occurrences(code, &format!("for {literal}")));
    total.saturating_sub(declarations)
}

/// Whether `text` names any of `names` as a whole identifier.
fn names_any(text: &str, names: &[&str]) -> bool {
    names.iter().any(|name| uses_of(text, name) > 0)
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// Every module of this crate is read by the rules below.
///
/// `docs/contracts/policy-source-scans.md`'s first empty shape is a walk that
/// stops short. This pins the whole file set in both directions, so a module
/// added and not covered fails here rather than being silently unscanned.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let files: Vec<String> = crate_all_sources()?
        .iter()
        .map(|path| relative(path))
        .collect();
    assert_eq!(
        files,
        vec![
            "crates/student-voice/examples/emit_corpus.rs".to_owned(),
            "crates/student-voice/src/corpus.rs".to_owned(),
            "crates/student-voice/src/derivative.rs".to_owned(),
            "crates/student-voice/src/fault.rs".to_owned(),
            "crates/student-voice/src/harness.rs".to_owned(),
            "crates/student-voice/src/hold.rs".to_owned(),
            "crates/student-voice/src/lib.rs".to_owned(),
            "crates/student-voice/src/measure.rs".to_owned(),
            "crates/student-voice/src/policy.rs".to_owned(),
            "crates/student-voice/src/preview.rs".to_owned(),
            "crates/student-voice/tests/common/mod.rs".to_owned(),
            "crates/student-voice/tests/compile_fail/a_derivative_cannot_be_given_wider_terms.rs"
                .to_owned(),
            "crates/student-voice/tests/compile_fail/a_held_capture_has_no_bytes.rs".to_owned(),
            "crates/student-voice/tests/compile_fail/a_redaction_cannot_reach_an_original.rs"
                .to_owned(),
            "crates/student-voice/tests/compile_fail/a_reviewed_capture_cannot_be_assembled.rs"
                .to_owned(),
            "crates/student-voice/tests/compile_fail/an_access_grant_is_spent_by_being_used.rs"
                .to_owned(),
            "crates/student-voice/tests/compile_fail/an_accuracy_witness_cannot_be_forged.rs"
                .to_owned(),
            "crates/student-voice/tests/compile_fail/an_automatic_redaction_needs_a_witness.rs"
                .to_owned(),
            "crates/student-voice/tests/compile_fail.rs".to_owned(),
            "crates/student-voice/tests/student_voice.rs".to_owned(),
            "crates/student-voice/tests/student_voice_corpus.rs".to_owned(),
            "crates/student-voice/tests/student_voice_scans.rs".to_owned(),
            "crates/student-voice/tests/student_voice_spec.rs".to_owned(),
        ],
        "the module set changed; a rule below may no longer cover the crate"
    );
    // Product sources are every file outside `tests`, which is what the
    // capability rules scope to.
    assert_eq!(crate_product_sources()?.len(), 10);
    Ok(())
}

// ---------------------------------------------------------------------------
// The witness has one producer
// ---------------------------------------------------------------------------

/// The whole `witness` function, pinned.
///
/// It is the only place an `AccuracyWitness` is built, and both refusals are in
/// it. A pin rather than a pair of assertions: a reordering that built the
/// witness before checking the privacy axis would pass two assertions and fail
/// this. The pinned text runs to the enclosing `impl` block's closing brace,
/// because `declared_item` reads to the first `}` at column zero, so a method
/// added after this one also fails the pin.
const WHOLE_WITNESS_FN: &str = "pub fn witness( &self, threshold: DiarizationThreshold, ) -> Result<AccuracyWitness, AccuracyRefusal> { let accuracy = self.accuracy_permille(); if accuracy < threshold.min_accuracy_permille { return Err(AccuracyRefusal::AccuracyBelowThreshold { measured: accuracy, required: threshold.min_accuracy_permille, }); } let missed = self.missed_student_permille(); if missed > threshold.max_missed_student_permille { return Err(AccuracyRefusal::MissedStudentSpeechAboveThreshold { measured: missed, allowed: threshold.max_missed_student_permille, }); } Ok(AccuracyWitness { corpus_id: self.corpus_id.clone(), corpus_version: self.corpus_version, corpus_digest: self.corpus_digest, scorer_version: self.scorer_version, threshold, accuracy_permille: accuracy, missed_student_permille: missed, }) } }";

/// An automatic redaction claim can only come from a measured number.
///
/// Three halves. The construction of a witness is counted across the whole
/// crate, so a second producer written anywhere fails; the one producer is
/// pinned whole, so a reordered or relaxed comparison fails; and its public
/// surface is compared as a whole set, so a setter nobody predicted fails as an
/// extra key.
#[test]
fn an_accuracy_witness_has_one_producer() -> TestResult {
    let mut built = 0;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        built += built_count(&code, "AccuracyWitness");
    }
    assert_eq!(built, 1, "an AccuracyWitness is built somewhere else");

    let measure = code_of(&crate_root().join("src/measure.rs"))?;
    assert_eq!(
        declared_item(&measure, "    pub fn witness(")?,
        WHOLE_WITNESS_FN,
        "the one producer of a witness changed"
    );
    assert_eq!(
        declarations_of(&measure, "witness"),
        1,
        "a second function named witness exists"
    );

    // The witness has no setter and no field a caller can write.
    assert_eq!(
        public_methods(&measure, "impl AccuracyWitness {")?,
        vec![
            "accuracy_permille".to_owned(),
            "corpus_digest".to_owned(),
            "corpus_id".to_owned(),
            "corpus_version".to_owned(),
            "missed_student_permille".to_owned(),
            "scorer_version".to_owned(),
            "threshold".to_owned(),
        ],
        "the witness's public surface changed"
    );
    for path in crate_product_sources()? {
        assert_eq!(
            occurrences(&code_of(&path)?, "impl Default for AccuracyWitness"),
            0,
            "{} gives a witness a second route",
            relative(&path)
        );
    }

    // A measurement has one producer too, and it walks a corpus.
    let mut measurements = 0;
    for path in crate_product_sources()? {
        measurements += built_count(&code_of(&path)?, "DiarizationMeasurement");
    }
    assert_eq!(
        measurements, 1,
        "a DiarizationMeasurement is built elsewhere"
    );
    assert_eq!(calls_of(&measure, "measure_case"), 1);
    Ok(())
}

// ---------------------------------------------------------------------------
// The hold's door has one producer
// ---------------------------------------------------------------------------

/// The whole `dispatch` function, pinned.
///
/// The hold check is the first statement and the stage call is after it. A pin
/// rather than an assertion that the check exists: `T141`'s finding is that a
/// check can stay byte-identical and stop running.
const WHOLE_DISPATCH_FN: &str = "pub fn dispatch<S: IngestionStage + ?Sized>( stage: &mut S, kind: IngestionJobKind, capture: &CaptureUnderReview, ) -> Result<IngestionReceipt, HoldRefusal> { if let HoldState::Held(classes) = capture.hold_state() { match capture.review.as_ref().map(|review| review.outcome) { None => { return Err(HoldRefusal::HeldPendingReview { classes: classes.iter().map(|class| class.as_str()).collect(), }); } Some(ReviewOutcome::Withhold) => return Err(HoldRefusal::ReviewWithheld), Some(ReviewOutcome::Release) => {} } } let admitted = ReviewedCapture { digest: capture.digest, kind, bytes: &capture.bytes, }; stage.ingest(&admitted); Ok(IngestionReceipt { digest: capture.digest, kind, }) }";

/// A capture reaches a downstream job through one door and no other.
#[test]
fn a_reviewed_capture_has_one_producer() -> TestResult {
    let mut built = 0;
    for path in crate_product_sources()? {
        built += built_count(&code_of(&path)?, "ReviewedCapture");
    }
    assert_eq!(built, 1, "a ReviewedCapture is built somewhere else");

    let hold = code_of(&crate_root().join("src/hold.rs"))?;
    assert_eq!(
        declared_item(&hold, "pub fn dispatch<")?,
        WHOLE_DISPATCH_FN,
        "the one door changed"
    );
    assert_eq!(declarations_of(&hold, "dispatch"), 1);

    // The held capture offers no bytes, and the admitted one offers them once.
    assert_eq!(
        public_methods(&hold, "impl CaptureUnderReview {")?,
        vec![
            "byte_len".to_owned(),
            "digest".to_owned(),
            "findings".to_owned(),
            "hold_state".to_owned(),
            "record_review".to_owned(),
            "review".to_owned(),
            "screened".to_owned(),
        ],
        "the held capture's public surface changed"
    );
    assert_eq!(
        public_methods(&hold, "impl ReviewedCapture<'_> {")?,
        vec!["bytes".to_owned(), "digest".to_owned(), "kind".to_owned()],
        "the admitted capture's public surface changed"
    );

    // Across every package in `crates/`, no `pub` signature takes a held
    // capture and returns anything its content could travel in.
    for path in workspace_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if !names_any(parameters, &["CaptureUnderReview"]) {
                continue;
            }
            assert!(
                !names_any(returns, &["u8", "String", "str", "CaptureBytes"]),
                "{} hands out the content of a held capture: {signature}",
                relative(&path)
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The retention rule has one producer
// ---------------------------------------------------------------------------

/// A derivative's terms come from `P2-G6`'s inheritance and from nowhere else.
///
/// The direction of the comparison is one character, so what is measured here
/// is that there is one call to it: a second `inherit` written with the
/// arguments the other way round is an extra call rather than a case the grid
/// missed.
#[test]
fn derivative_terms_have_one_producer() -> TestResult {
    let mut inherit_calls = 0;
    let mut inherit_terms_calls = 0;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        inherit_calls += occurrences(&code, ".inherit(");
        inherit_terms_calls += calls_of(&drop_use_items(&code), "inherit_terms");
    }
    assert_eq!(
        inherit_calls, 1,
        "RetentionTerms::inherit is called somewhere besides inherit_terms"
    );
    assert_eq!(
        inherit_terms_calls, 3,
        "the callers of inherit_terms changed; each one is a derivative link"
    );

    let derivative = code_of(&crate_root().join("src/derivative.rs"))?;
    assert_eq!(
        declared_item(&derivative, "pub fn inherit_terms(")?,
        "pub fn inherit_terms(parent: RetentionTerms, requested: RetentionTerms) -> RetentionTerms { parent.inherit(requested) }",
        "the one inheritance path changed"
    );

    // No widening helper, and no second retention constructor.
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for forbidden in [".max(", "RetentionTerms::new("] {
            assert_eq!(
                occurrences(&code, forbidden),
                0,
                "{} builds retention terms outside the inheritance path with {forbidden}",
                relative(&path)
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// GATE-38-026 stays open
// ---------------------------------------------------------------------------

/// Nothing here produces or names an `OriginalVoiceAuthority`.
///
/// `academic-retention` holds the mechanism for removing voices from an
/// original behind an authority a caller has to state. `GATE-38-026` is the
/// question of whether that should ever happen, and this crate answers it by
/// having no route to it: no product file names the type, no scope variant
/// reaches an original, and no `pub` signature in any package takes something
/// of this crate's and returns one.
#[test]
fn no_original_voice_authority_is_produced_here() -> TestResult {
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        assert_eq!(
            uses_of(&code, "OriginalVoiceAuthority"),
            0,
            "{} names the authority this crate must not produce",
            relative(&path)
        );
        assert_eq!(
            uses_of(&code, "VoiceSpansInOriginal"),
            0,
            "{} names the retention scope that edits an original",
            relative(&path)
        );
    }

    // The scope enum has one variant, and it is the derivative.
    let policy = code_of(&crate_root().join("src/policy.rs"))?;
    assert_eq!(
        declared_item(&policy, "pub enum RedactionScope {")?,
        "pub enum RedactionScope { DerivativeOnly, }",
        "a redaction scope that reaches an original was added"
    );

    // Across every package, nothing turns one of this crate's values into an
    // authority to edit an original.
    let owned = [
        "RedactionPolicy",
        "RedactionPlan",
        "RedactedDerivative",
        "RestrictedOriginal",
        "DisclosedOriginal",
        "AccuracyWitness",
    ];
    for path in workspace_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if !names_any(parameters, &owned) {
                continue;
            }
            assert!(
                !names_any(returns, &["OriginalVoiceAuthority", "SubjectScope"]),
                "{} routes a P2-L5 value into an original-editing authority: {signature}",
                relative(&path)
            );
        }
    }

    // The open statement names the gate and says it is open.
    assert!(academic_student_voice::GATE_38_026_OPEN.contains("GATE-38-026"));
    assert!(academic_student_voice::GATE_38_026_OPEN.contains("open"));
    assert!(
        academic_student_voice::GATE_38_026_OPEN.contains("selects no policy"),
        "the open statement stopped saying that no policy was chosen"
    );
    // `P2-K5` says the same thing where the mechanism lives.
    assert!(academic_retention::GATE_38_026_STATEMENT.contains("GATE-38-026"));
    assert!(academic_retention::GATE_38_026_STATEMENT.contains("open"));
    Ok(())
}

// ---------------------------------------------------------------------------
// The disclosure does not travel
// ---------------------------------------------------------------------------

/// Nothing turns an authorized read back into a derivative.
///
/// Authorized access means a person may read what was removed. What it must not
/// mean is a route by which the removed speech re-enters the artifact the
/// redaction produced, so this is a rule over a **pair of types** rather than a
/// list of function names: a route from a disclosure to a derivative fails
/// however it is spelled.
#[test]
fn no_disclosure_reaches_a_derivative() -> TestResult {
    let mut checked = 0;
    for path in workspace_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if !names_any(parameters, &["DisclosedOriginal", "RestrictedOriginal"]) {
                continue;
            }
            checked += 1;
            assert!(
                !names_any(
                    returns,
                    &[
                        "RedactedDerivative",
                        "KeptUtterance",
                        "LectureSource",
                        "DerivedArtifact",
                    ]
                ),
                "{} routes a restricted original into a derivative: {signature}",
                relative(&path)
            );
        }
    }
    assert!(
        checked > 0,
        "the pair rule read no signature, so it asserts nothing"
    );

    // The disclosure is borrowed and has no owned form.
    let derivative = code_of(&crate_root().join("src/derivative.rs"))?;
    for forbidden in [
        "impl Clone for DisclosedOriginal",
        "impl ToOwned for DisclosedOriginal",
    ] {
        assert_eq!(occurrences(&derivative, forbidden), 0);
    }
    assert_eq!(
        declared_item(&derivative, "pub struct DisclosedOriginal<'a> {")?,
        "pub struct DisclosedOriginal<'a> { removed: &'a [RemovedUtterance], }",
        "the disclosure gained a field"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// No floating point
// ---------------------------------------------------------------------------

/// Every ratio here is permille in `u64`.
///
/// `academic-record` fixed this rule for money. It holds for the same reason
/// here: a number that decides whether somebody's voice may be processed
/// automatically must not depend on a rounding mode, and a threshold comparison
/// under `f64` is one.
#[test]
fn no_floating_point_reaches_this_crate() -> TestResult {
    for path in crate_all_sources()? {
        let code = code_of(&path)?;
        for spelling in ["f32", "f64"] {
            assert_eq!(
                uses_of(&code, spelling),
                0,
                "{} names {spelling}",
                relative(&path)
            );
        }
        for line in code.lines() {
            assert!(
                !holds_decimal_literal(line),
                "{} holds a decimal literal: {line}",
                relative(&path)
            );
        }
    }

    // Non-vacuous: the reader finds a float in a sample that has one, and does
    // not find one in a path expression or a range.
    assert!(holds_decimal_literal("let ratio = 0.967;"));
    assert!(!holds_decimal_literal("let range = 0..967;"));
    assert!(!holds_decimal_literal("let value = self.scored_ms;"));
    Ok(())
}

/// Whether `line` holds a digit-dot-digit sequence.
fn holds_decimal_literal(line: &str) -> bool {
    let characters: Vec<char> = line.chars().collect();
    characters.iter().enumerate().any(|(at, character)| {
        *character == '.'
            && at > 0
            && characters[at - 1].is_ascii_digit()
            && characters.get(at + 1).is_some_and(char::is_ascii_digit)
    })
}

// ---------------------------------------------------------------------------
// No clock, socket, file or process
// ---------------------------------------------------------------------------

/// Nothing in this crate's library reaches an ambient capability.
#[test]
fn no_wall_clock_socket_or_file_reaches_this_crate() -> TestResult {
    let forbidden: [(&str, &str); 14] = [
        ("clock", "SystemTime"),
        ("clock", "Instant::now"),
        ("clock", "std::time"),
        ("clock", "chrono"),
        ("socket", "TcpStream"),
        ("socket", "TcpListener"),
        ("socket", "UdpSocket"),
        ("socket", "std::net"),
        ("file", "File::open"),
        ("file", "read_to_string"),
        ("file", "fs::write"),
        ("process", "Command::new"),
        ("environment", "env::var"),
        ("environment", "var_os"),
    ];
    let src = crate_root().join("src");
    for path in crate_product_sources()? {
        if !path.starts_with(&src) {
            continue;
        }
        let code = code_of(&path)?;
        for (capability, spelling) in forbidden {
            assert_eq!(
                occurrences(&code, spelling),
                0,
                "{} reaches for a {capability} capability with {spelling}",
                relative(&path)
            );
        }
    }

    // Non-vacuous: each spelling matches the call it forbids.
    let sample = "SystemTime::now(); Instant::now(); std::time::Duration; chrono::Utc; \
                  TcpStream::connect(); TcpListener::bind(); UdpSocket::bind(); std::net::Ipv4Addr; \
                  File::open(p); read_to_string(p); fs::write(p, b); Command::new(x); \
                  env::var(k); var_os(k);";
    for (_, spelling) in forbidden {
        assert!(
            occurrences(sample, spelling) > 0,
            "the {spelling} rule matches nothing"
        );
    }

    // The example writes files -- it is the corpus emitter and it is not
    // compiled into the library -- so the rule scopes to `src`, and this
    // records the exception rather than leaving it silent.
    let emitter = code_of(&crate_root().join("examples/emit_corpus.rs"))?;
    assert!(occurrences(&emitter, "fs::write") > 0);
    Ok(())
}
