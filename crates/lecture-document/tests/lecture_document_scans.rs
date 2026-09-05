//! Source scans for the `P2-L4` boundary.
//!
//! Four of this task's claims are statements about what the source does not
//! contain, and a behavioural test cannot observe an absence:
//!
//! * no path in this crate reduces a document because a span was ranked low;
//! * the rendering is a sink, so nothing reads a `PdfArtifact` back as a record;
//! * `MAPPED` has one producer and it is the validator; and
//! * the completeness witness has one producer and it is the report.
//!
//! Every shape here is one `docs/contracts/policy-source-scans.md` already
//! records, and the two the previous tasks in this run measured failing are
//! avoided by construction:
//!
//! * **Whole-set, never a token list.** `P2-R2` measured five guards in a row
//!   failing because each asked whether a name was on a list of forbidden
//!   spellings; a bypass that spells nothing on the list walks past all five.
//!   Every rule below compares a **complete set** — the producers of a type,
//!   the files that may name one, the public signatures over a pair of types —
//!   against a pinned list, so an unforeseen spelling fails as an extra key.
//! * **A pin fixes its callers too.** `T141` left a pinned check byte-identical
//!   and wrapped the call to it in a condition. Each pin below is accompanied by
//!   a count of the sites that reach it.
//! * **A sweep over signatures is not a claim about constructions.** `U-G3`: a
//!   second entry point can build its argument in its body and name the type
//!   nowhere in its signature, so the closed types below are counted where they
//!   are *built*.
//!
//! The helper block is `crates/transcription/tests/transcription_scans.rs`
//! copied verbatim, which is that file's own note about
//! `crates/record/tests/record_scans.rs`: a test module is not a library
//! target, and a stripper without raw strings desynchronizes.

mod common;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use common::TestResult;

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
// PINS
// ---------------------------------------------------------------------------

/// The account, whole. One field of status type, every field private.
///
/// A pin rather than a field count: a count would let `status` become a `Vec`
/// and stay at four fields.
const WHOLE_SEGMENT_ACCOUNT: &str = "pub struct SegmentAccount { segment_index: usize, segment_id: String, token_count: usize, status: SegmentStatus, }";

/// What a coverage run reads, whole.
///
/// This is where `no_low_importance_deletion`'s absence lives: there is no
/// field naming a salience, a ranking, a threshold on importance, or a list of
/// segments to consider, and the eligible set is walked off the lineage inside
/// the validator. A pin over the whole struct fails on an added field however
/// it is spelled, which a list of forbidden field names would not.
const WHOLE_COVERAGE_INPUTS: &str = "pub struct CoverageInputs<'a> { pub lineage: &'a TranscriptLineage, pub version: u32, pub document: &'a LectureDocument, pub manifest: &'a InputManifest, pub journal: &'a JournalRecovery, pub dispositions: &'a DispositionLedger, pub capture_exclusions: &'a CaptureExclusionLedger, pub config: CoverageConfig, }";

/// The witness, whole. One private field, no public constructor.
const WHOLE_WITNESS: &str = "pub struct CompletenessWitness { report_digest: ContentDigest, }";

/// The three declaring constructors, whole.
///
/// There is no fourth and there is no `mapped`. A pin over the whole `impl`
/// fails on an added constructor however it is named, which a list of forbidden
/// names would not.
const WHOLE_DISPOSITION_IMPL: &str = "impl SegmentDisposition { #[must_use] pub const fn excluded_non_speech(segment_index: usize, evidence: NonSpeechEvidence) -> Self { Self { segment_index, status: SegmentStatus::ExcludedNonSpeech { evidence }, } } #[must_use] pub const fn redacted_with_policy(segment_index: usize, policy: RedactionPolicyRef) -> Self { Self { segment_index, status: SegmentStatus::RedactedWithPolicy { policy }, } } #[must_use] pub const fn untranscribed_failure( segment_index: usize, failure: TranscriptionFailure, ) -> Self { Self { segment_index, status: SegmentStatus::UntranscribedFailure { failure }, } } #[must_use] pub const fn segment_index(&self) -> usize { self.segment_index } #[must_use] pub const fn status(&self) -> &SegmentStatus { &self.status } }";

/// Section 12.5's nine transforms, whole.
const WHOLE_TRANSFORM_ENUM: &str = "pub enum PreservationTransform { OrderPreservation, Punctuation, SectionHeading, Timestamp, SpeakerLabel, MathAndCodeFormatting, TerminologyMarking, RepetitionAndEmphasisAnnotation, CapturePlacement, }";

/// The rendering, whole.
///
/// `render` writes `Incomplete` first and `upgrade` is the only path off it,
/// and `upgrade` takes a `CompletenessWitness` by value.
const WHOLE_PDF_IMPL: &str = "impl PdfArtifact { #[must_use] pub fn render( document: &LectureDocument, report: &CoverageReport, qa: &RenderQaReport, rendered_bytes_digest: ContentDigest, ) -> Self { let document_digest = document.digest(); let mut completeness = DocumentCompleteness::Incomplete { unmapped_segments: report.unmapped_count(), render_defects: qa.findings().len(), }; if report.document_digest() == &document_digest && qa.document_digest() == &document_digest && qa.is_clean() && let Some(witness) = report.completeness_witness() { completeness = Self::upgrade(&document_digest, report, witness); } Self { document: document.id().clone(), document_digest, rendered_bytes_digest, completeness, } } fn upgrade( document_digest: &ContentDigest, report: &CoverageReport, witness: CompletenessWitness, ) -> DocumentCompleteness { if witness.report_digest() == &report.digest() && report.document_digest() == document_digest { DocumentCompleteness::Complete } else { DocumentCompleteness::Incomplete { unmapped_segments: report.unmapped_count(), render_defects: 0, } } } #[must_use] pub const fn document(&self) -> &DocumentId { &self.document } #[must_use] pub const fn document_digest(&self) -> &ContentDigest { &self.document_digest } #[must_use] pub const fn rendered_bytes_digest(&self) -> &ContentDigest { &self.rendered_bytes_digest } #[must_use] pub const fn completeness(&self) -> DocumentCompleteness { self.completeness } #[must_use] pub fn canonical_bytes(&self) -> Vec<u8> { let mut material = Vec::new(); material.extend_from_slice(b ); push_str(&mut material, self.document.as_str()); material.extend_from_slice(self.document_digest.to_string().as_bytes()); material.extend_from_slice(self.rendered_bytes_digest.to_string().as_bytes()); push_str(&mut material, self.completeness.as_str()); match self.completeness { DocumentCompleteness::Incomplete { unmapped_segments, render_defects, } => { material.extend_from_slice(&be_len(unmapped_segments)); material.extend_from_slice(&be_len(render_defects)); } DocumentCompleteness::Complete => {} } material } }";

/// The one file that may name `Salience`.
///
/// The whole-set half of `no_low_importance_deletion`: a ranking is what a
/// summary is for, and it may not reach the document or the coverage report.
const SALIENCE_FILES: [&str; 1] = ["crates/lecture-document/src/study_index.rs"];

/// Types the rendering must not be readable back into.
///
/// `pdf_non_authority`'s workspace half: no public signature anywhere in
/// `crates/` may take a `PdfArtifact` and return one of these. It is a rule
/// over a **pair of types**, not a list of function names nobody may write.
const RECORD_TYPES: [&str; 6] = [
    "LectureDocument",
    "CoverageReport",
    "CompletenessWitness",
    "TranscriptLineage",
    "SegmentAccount",
    "StudyIndex",
];

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// The walk reads the package, not `src`.
///
/// `S-12`'s finding, applied here in advance: a `[[bin]]` with an explicit
/// `path`, an `examples/` tree, or a `#[path]` module outside `src` is product
/// code every scan below would otherwise miss. This crate has an `examples/`
/// tree -- the harness emitter -- so the rule has something to catch.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let all = crate_all_sources()?;
    assert!(
        all.len() >= 16,
        "the walk found only {} files under this package; it stopped short",
        all.len()
    );
    let product = crate_product_sources()?;
    assert!(
        product.len() >= 12,
        "the product walk found only {} files; it stopped short",
        product.len()
    );

    let root = crate_root();
    let mut outside: Vec<String> = Vec::new();
    for path in &product {
        let inside_src = path.starts_with(root.join("src"));
        let is_example = path.starts_with(root.join("examples"));
        if !inside_src && !is_example {
            outside.push(relative(path));
        }
    }
    assert_eq!(
        outside,
        Vec::<String>::new(),
        "this crate has product source outside src and examples; every scan has to widen"
    );
    for path in &product {
        assert_eq!(
            occurrences(&code_of(path)?, "#[path"),
            0,
            "{} moves a module with #[path]",
            relative(path)
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// segment_status_exhaustive, the source half
// ---------------------------------------------------------------------------

/// `MAPPED` has one producer and it is the validator.
#[test]
fn the_mapped_status_has_one_producer() -> TestResult {
    let coverage = code_of(&crate_root().join("src/coverage.rs"))?;
    assert_eq!(
        declared_item(&coverage, "pub struct SegmentAccount {")?,
        WHOLE_SEGMENT_ACCOUNT,
        "the account's shape changed"
    );
    assert_eq!(
        declared_item(&coverage, "impl SegmentDisposition {")?,
        WHOLE_DISPOSITION_IMPL,
        "a declaring constructor was added, removed or renamed"
    );

    // The whole set of sites that construct an account, counted across every
    // product file. A map rather than a total: a second site in another file
    // fails as an extra key. Two constructions in one function is the whole of
    // it -- the mapped arm and the declared arm of the classification -- and a
    // third anywhere is a second way for a segment to acquire a status.
    let mut account_sites: BTreeMap<String, usize> = BTreeMap::new();
    for path in crate_product_sources()? {
        let code = drop_use_items(&code_of(&path)?);
        let accounts = constructions_of(&code, "SegmentAccount");
        if accounts > 0 {
            account_sites.insert(relative(&path), accounts);
        }
    }
    assert_eq!(
        account_sites,
        BTreeMap::from([("crates/lecture-document/src/coverage.rs".to_owned(), 2)]),
        "an account is built somewhere other than the validator"
    );

    // And no other package in `crates/` names the type at all, which is
    // `P2-U6`'s `credentials_never_reach_a_general_crawler` shape: a type
    // nothing else can name is a type nothing else can build on unnoticed.
    let mut elsewhere: Vec<String> = Vec::new();
    for path in workspace_sources()? {
        let relative = relative(&path);
        if relative.starts_with("crates/lecture-document/") {
            continue;
        }
        if names_any(&code_of(&path)?, &["SegmentAccount", "CompletenessWitness"]) {
            elsewhere.push(relative);
        }
    }
    assert_eq!(
        elsewhere,
        Vec::<String>::new(),
        "a package outside academic-lecture-document names an account or a witness"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// unmapped_forces_incomplete, the source half
// ---------------------------------------------------------------------------

/// The witness has one producer and `COMPLETE` has one site.
#[test]
fn incomplete_is_the_only_value_with_no_measurement_behind_it() -> TestResult {
    let coverage = code_of(&crate_root().join("src/coverage.rs"))?;
    let pdf = code_of(&crate_root().join("src/pdf.rs"))?;
    assert_eq!(
        declared_item(&coverage, "pub struct CompletenessWitness {")?,
        WHOLE_WITNESS,
        "the witness gained a field or lost its privacy"
    );
    assert_eq!(
        declared_item(&pdf, "impl PdfArtifact {")?,
        WHOLE_PDF_IMPL,
        "the rendering's construction changed"
    );

    let mut witness_sites: BTreeMap<String, usize> = BTreeMap::new();
    let mut complete_sites: BTreeMap<String, usize> = BTreeMap::new();
    for path in crate_product_sources()? {
        let code = drop_use_items(&code_of(&path)?);
        let witnesses = constructions_of(&code, "CompletenessWitness");
        if witnesses > 0 {
            witness_sites.insert(relative(&path), witnesses);
        }
        let completes = occurrences(&code, "DocumentCompleteness::Complete");
        if completes > 0 {
            complete_sites.insert(relative(&path), completes);
        }
    }
    assert_eq!(
        witness_sites,
        BTreeMap::from([("crates/lecture-document/src/coverage.rs".to_owned(), 1)]),
        "a completeness witness is minted somewhere other than the report"
    );
    // Two spellings in one file: the construction in `upgrade` and the match
    // arm in `canonical_bytes` that renders it. A fieldless variant's
    // construction and its pattern are the same three tokens, so the count
    // cannot tell them apart -- the pin above is what fixes which is which, and
    // this map's job is that **no other file spells it at all**.
    assert_eq!(
        complete_sites,
        BTreeMap::from([("crates/lecture-document/src/pdf.rs".to_owned(), 2)]),
        "COMPLETE is written outside the one module that may upgrade to it"
    );

    // The pin fixes its caller too: `upgrade` is reached from exactly one site,
    // which is `render`. `T141` left a pinned check byte-identical and wrapped
    // its call in a condition.
    let calls = calls_of(&drop_use_items(&pdf), "upgrade");
    assert_eq!(calls, 1, "the one upgrade path has {calls} callers");
    Ok(())
}

// ---------------------------------------------------------------------------
// pdf_non_authority, the source half
// ---------------------------------------------------------------------------

/// The rendering is a sink: nothing in `crates/` reads one back as a record.
#[test]
fn no_signature_reads_a_rendering_back_into_a_record() -> TestResult {
    let mut offenders: Vec<String> = Vec::new();
    let mut naming: Vec<String> = Vec::new();
    for path in workspace_sources()? {
        let code = code_of(&path)?;
        if !names_any(&code, &["PdfArtifact"]) {
            continue;
        }
        naming.push(relative(&path));
        for signature in public_signatures(&code) {
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if names_any(parameters, &["PdfArtifact"]) && names_any(returns, &RECORD_TYPES) {
                offenders.push(format!("{}: {signature}", relative(&path)));
            }
        }
    }
    assert_eq!(
        offenders,
        Vec::<String>::new(),
        "a public signature reads a rendering back into a record"
    );

    // The whole set of files that may name the rendering at all. `P2-R2`'s
    // repair shape: a list of forbidden spellings answers "is this one of the
    // things we thought of"; a whole set fails on anything new.
    assert_eq!(
        naming,
        vec![
            "crates/lecture-document/src/lib.rs".to_owned(),
            "crates/lecture-document/src/pdf.rs".to_owned(),
            "crates/lecture-document/tests/compile_fail/a_pdf_cannot_be_marked_complete.rs"
                .to_owned(),
            "crates/lecture-document/tests/compile_fail/a_study_index_cannot_be_rendered_as_the_document.rs"
                .to_owned(),
            "crates/lecture-document/tests/lecture_document.rs".to_owned(),
        ],
        "a file outside the rendering's own module and its suite names it"
    );

    // The non-vacuity control: the rule matches the signature it forbids.
    let injected = "pub fn recover(pdf: &PdfArtifact) -> LectureDocument {";
    let (parameters, returns) =
        parameters_and_return(injected).ok_or("the control signature does not parse")?;
    assert!(names_any(parameters, &["PdfArtifact"]) && names_any(returns, &RECORD_TYPES));
    Ok(())
}

// ---------------------------------------------------------------------------
// no_low_importance_deletion, the source half
// ---------------------------------------------------------------------------

/// A ranking cannot reach the document or the coverage report.
#[test]
fn a_ranking_cannot_reach_the_preservation_path() -> TestResult {
    let coverage = code_of(&crate_root().join("src/coverage.rs"))?;
    assert_eq!(
        declared_item(&coverage, "pub struct CoverageInputs<")?,
        WHOLE_COVERAGE_INPUTS,
        "what a coverage run reads changed; a new input could shrink the denominator"
    );

    // The whole set of product files that name `Salience`, less the crate root,
    // which re-exports every public name and says nothing about reach.
    let mut salience: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        if names_any(&drop_use_items(&code_of(&path)?), &["Salience"]) {
            salience.push(relative(&path));
        }
    }
    assert_eq!(
        salience,
        SALIENCE_FILES.map(str::to_owned).to_vec(),
        "a ranking reached a file outside the study index"
    );

    // And the two preservation modules name no study-index type at all, so the
    // separation is a graph fact in one direction as well as a rule.
    for module in ["src/document.rs", "src/coverage.rs"] {
        let code = code_of(&crate_root().join(module))?;
        assert!(
            !names_any(
                &code,
                &[
                    "StudyIndex",
                    "StudyIndexEntry",
                    "StudyIndexBuilder",
                    "Salience"
                ]
            ),
            "{module} names a study index type"
        );
    }

    // The eligible set is walked off the lineage, not taken as an argument.
    assert!(
        occurrences(&coverage, "lineage.segment_at(version, index)") >= 1,
        "the eligible set is no longer walked off the lineage"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// lossless_transform_allowlist, the source half
// ---------------------------------------------------------------------------

/// The nine transforms are a closed set with one producer of a mapping.
#[test]
fn the_transform_set_is_closed_and_the_mapping_has_one_producer() -> TestResult {
    let transform = code_of(&crate_root().join("src/transform.rs"))?;
    assert_eq!(
        declared_item(&transform, "pub enum PreservationTransform {")?,
        WHOLE_TRANSFORM_ENUM,
        "the transform set changed"
    );

    let mut mapping_sites: BTreeMap<String, usize> = BTreeMap::new();
    for path in crate_product_sources()? {
        let code = drop_use_items(&code_of(&path)?);
        let count = constructions_of(&code, "SourceMapping");
        if count > 0 {
            mapping_sites.insert(relative(&path), count);
        }
    }
    assert_eq!(
        mapping_sites,
        BTreeMap::from([("crates/lecture-document/src/document.rs".to_owned(), 1)]),
        "a source mapping is built somewhere other than the builder"
    );

    // The whole set of `impl` blocks whose header names a document type. An
    // implementation nobody predicted -- a `From<&str>` for a transform, a
    // `Deref` for a mapping -- fails as an extra key.
    let document = code_of(&crate_root().join("src/document.rs"))?;
    let mut blocks: Vec<String> = Vec::new();
    for source in [&document, &transform] {
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl ")
                && names_any(
                    trimmed,
                    &["SourceMapping", "PreservationTransform", "LectureDocument"],
                )
            {
                blocks.push(trimmed.trim_end_matches(" {").to_owned());
            }
        }
    }
    blocks.sort();
    assert_eq!(
        blocks,
        vec![
            "impl LectureDocument".to_owned(),
            "impl PreservationTransform".to_owned(),
            "impl SourceMapping".to_owned(),
            "impl fmt::Debug for LectureDocument".to_owned(),
            "impl fmt::Debug for SourceMapping".to_owned(),
        ],
        "an implementation on a document type appeared or disappeared"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The tripwire P2-L3 left for this task
// ---------------------------------------------------------------------------

/// This crate names no raw type, so `P2-L3`'s workspace scope rule still holds.
///
/// The document is built over `TranscriptSegment` and `EffectiveToken` at one
/// version. `P2-L3` recorded its rule as "a tripwire for `P2-L4`, which is the
/// first task that will"; it is not, and this asserts that from this side too
/// rather than leaving the claim in one crate's test.
#[test]
fn the_document_names_no_raw_type() -> TestResult {
    let mut naming: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        if names_any(
            &code_of(&path)?,
            &["RawToken", "RawSegment", "RawTranscript"],
        ) {
            naming.push(relative(&path));
        }
    }
    assert_eq!(
        naming,
        Vec::<String>::new(),
        "this crate names a raw type; P2-L3's scope rule has to widen or this has to stop"
    );

    // The versioned view is what it names instead, so the assertion above is
    // not passing because the crate reads no transcript at all.
    let document = code_of(&crate_root().join("src/document.rs"))?;
    assert!(names_any(&document, &["TranscriptSegment"]));
    let review = code_of(&crate_root().join("src/review.rs"))?;
    assert!(names_any(&review, &["TranscriptLineage"]));
    Ok(())
}

// ---------------------------------------------------------------------------
// The absences
// ---------------------------------------------------------------------------

/// No clock, socket, file, process or environment read reaches this crate.
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

    // The example writes files -- it is the harness emitter and it is not
    // compiled into the library -- so the rule scopes to `src`, and this
    // records the exception rather than leaving it silent.
    let emitter = code_of(&crate_root().join("examples/emit_harness.rs"))?;
    assert!(occurrences(&emitter, "fs::write") > 0);
    Ok(())
}

// ---------------------------------------------------------------------------
// no_low_importance_deletion, the API-surface half
// ---------------------------------------------------------------------------

/// Neither the document nor the report offers a method that returns less than
/// it holds.
///
/// The pins above fix what a coverage run *reads*; this fixes what the two
/// preservation types *offer*. Without it a method taking a floor, a limit or a
/// weight and returning a subset of the nodes would be a reduction path in the
/// product that no other rule here sees — measured as a real hole while writing
/// the injection matrix, and closed rather than recorded as open.
///
/// A whole set of names, not a list of forbidden ones: a method called anything
/// at all fails as an extra key.
#[test]
fn the_preservation_types_offer_no_reducing_method() -> TestResult {
    let document = code_of(&crate_root().join("src/document.rs"))?;
    assert_eq!(
        public_methods(&document, "impl LectureDocument {")?,
        vec![
            "digest".to_owned(),
            "id".to_owned(),
            "lecture".to_owned(),
            "nodes".to_owned(),
            "transcript_token_digest".to_owned(),
            "version".to_owned(),
        ],
        "the document's public surface changed"
    );

    let coverage = code_of(&crate_root().join("src/coverage.rs"))?;
    assert_eq!(
        public_methods(&coverage, "impl CoverageReport {")?,
        vec![
            "accounts".to_owned(),
            "canonical_bytes".to_owned(),
            "completeness_witness".to_owned(),
            "config".to_owned(),
            "digest".to_owned(),
            "document_digest".to_owned(),
            "excluded_captures".to_owned(),
            "gaps".to_owned(),
            "lecture".to_owned(),
            "ordering_exceptions".to_owned(),
            "ordering_findings".to_owned(),
            "placed_captures".to_owned(),
            "reconciles".to_owned(),
            "segment_coverage".to_owned(),
            "token_coverage".to_owned(),
            "transcript_token_digest".to_owned(),
            "unaccounted_captures".to_owned(),
            "unexplained_gaps".to_owned(),
            "unmapped".to_owned(),
            "unmapped_count".to_owned(),
            "version".to_owned(),
        ],
        "the report's public surface changed"
    );

    // The reader is not vacuous: it finds names, and it finds none in a block
    // whose methods are all private.
    assert!(!public_methods(&coverage, "impl CoverageValidator {")?.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// Every impl header is in the inventory
// ---------------------------------------------------------------------------

/// Every `impl` header of `code`, up to its opening brace.
///
/// A trait impl's methods carry no visibility modifier, so an inventory keyed
/// on `pub fn` cannot see one at all. `P2-A4` measured that gap here with
/// `impl From<&CoverageReport> for CompletenessWitness`, which passed this crate's whole suite. The precedent for closing
/// it is `P2-Y3`'s and `P2-X5`'s: pin the complete set of headers, so a
/// conversion nobody predicted fails as an extra entry rather than having to be
/// named on a forbidden list.
fn impl_headers(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut lines = code.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if !(trimmed == "impl" || trimmed.starts_with("impl ") || trimmed.starts_with("impl<")) {
            continue;
        }
        // A header may be wrapped, so keep reading until the block opens. An
        // `impl Trait` in argument position is not a header and is skipped by
        // the line anchor above: it can never begin a line, because a parameter
        // list always puts a name and a colon in front of it.
        let mut header = trimmed.to_owned();
        while !header.contains('{') {
            let Some(next) = lines.next() else {
                break;
            };
            header.push(' ');
            header.push_str(next.trim());
        }
        let end = header.find('{').unwrap_or(header.len());
        found.insert(
            header[..end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    found
}

/// Every `impl` header this crate declares, pinned as a complete set.
const IMPL_HEADERS: &[&str] = &[
    "impl $name",
    "impl AudioLocator",
    "impl CaptureExclusion",
    "impl CaptureExclusionLedger",
    "impl CaptureExclusionReason",
    "impl CompletenessWitness",
    "impl CoverageConfig",
    "impl CoverageReport",
    "impl CoverageValidator",
    "impl CrossReference",
    "impl CrossReferenceReason",
    "impl DeterministicEngine for TranscriptCoverageEngine",
    "impl DispositionLedger",
    "impl DocumentAnnotation",
    "impl DocumentCompleteness",
    "impl DocumentNode",
    "impl GapFinding",
    "impl LectureDocument",
    "impl NodeKind",
    "impl NonSpeechEvidence",
    "impl NonSpeechReason",
    "impl OrderingException",
    "impl OrderingFinding",
    "impl PdfArtifact",
    "impl PreservationTransform",
    "impl Ratio",
    "impl RedactionBasis",
    "impl RedactionPolicyRef",
    "impl RenderDefect",
    "impl RenderFinding",
    "impl RenderQa",
    "impl RenderQaReport",
    "impl ReviewItem",
    "impl ReviewQueue",
    "impl RiskClass",
    "impl Salience",
    "impl SegmentAccount",
    "impl SegmentDisposition",
    "impl SegmentStatus",
    "impl SourceMapping",
    "impl StudyIndex",
    "impl StudyIndexEntry",
    "impl StudyIndexId",
    "impl TranscriptCoverageEngine",
    "impl TranscriptionFailure",
    "impl UnaccountedCapture",
    "impl UnmappedSegment",
    "impl core::fmt::Debug for StudyIndex",
    "impl core::fmt::Debug for StudyIndexEntry",
    "impl fmt::Debug for $name",
    "impl fmt::Debug for DocumentNode",
    "impl fmt::Debug for LectureDocument",
    "impl fmt::Debug for NodeDraft",
    "impl fmt::Debug for SourceMapping",
    "impl fmt::Display for $name",
    "impl<'a> DocumentBuilder<'a>",
    "impl<'a> StudyIndexBuilder<'a>",
];

/// The traits this crate implements for its own types, pinned as a set.
///
/// Nine. Eight are a `Debug` or a `Display`, two of those from the identifier
/// macro, and the ninth is `P2-C5`'s engine trait. No conversion, no
/// dereference, no iteration -- in particular nothing that mints a
/// `CompletenessWitness` outside `completeness_witness`.
const TRAIT_IMPLS: &[&str] = &[
    "impl DeterministicEngine for TranscriptCoverageEngine",
    "impl core::fmt::Debug for StudyIndex",
    "impl core::fmt::Debug for StudyIndexEntry",
    "impl fmt::Debug for $name",
    "impl fmt::Debug for DocumentNode",
    "impl fmt::Debug for LectureDocument",
    "impl fmt::Debug for NodeDraft",
    "impl fmt::Debug for SourceMapping",
    "impl fmt::Display for $name",
];

/// Every `impl` header this crate declares is in the inventory, both ways.
///
/// `P2-A4`'s F12: the blindness that let a trait impl hand out removed student
/// speech in `academic-student-voice` is a property of the scan's definition of
/// "signature", not of that crate, and the same injection compiled and passed
/// here. The close is the same whole-set comparison: `From` is the spelling
/// that was measured, but `Into`, `TryFrom`, `Deref`, `AsRef`, `Borrow`,
/// `Index`, `IntoIterator` and a trait nobody has thought of all reach the same
/// private fields, so the rule is stated over the complete set rather than over
/// a list of trait names.
#[test]
fn every_impl_header_in_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for path in crate_product_sources()? {
        found.extend(impl_headers(&code_of(&path)?));
    }
    assert_eq!(
        found,
        IMPL_HEADERS.iter().map(|item| (*item).to_owned()).collect(),
        "the impl-header inventory and the source disagree"
    );

    // The trait half stated on its own, so the reason survives an edit to the
    // list above: every header that names a trait is one of these, and none of
    // them is a conversion, a dereference, an iteration or an arithmetic fold.
    let traits: Vec<&str> = found
        .iter()
        .filter(|header| header.contains(" for "))
        .map(String::as_str)
        .collect();
    assert_eq!(
        traits,
        TRAIT_IMPLS.to_vec(),
        "this crate implements a trait the inventory does not carry"
    );

    // The scanner is not vacuous: it finds the shape `P2-A4` injected, and it
    // does not read an `impl Trait` in argument position as a header.
    assert_eq!(
        impl_headers("impl From<&CoverageReport> for CompletenessWitness {\n}\n"),
        ["impl From<&CoverageReport> for CompletenessWitness"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(impl_headers("fn takes(value: impl Display) {}\n").is_empty());
    Ok(())
}
