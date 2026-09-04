//! The two acceptance claims that are absences, and the inventories behind
//! them.
//!
//! `no_login_bypass_or_evasion_module_exists` and
//! `raw_review_text_is_excluded_from_export_and_share` are claims about what is
//! *not* here. A behavioural test cannot observe an absence, so they are here.
//!
//! `docs/contracts/policy-source-scans.md` records the three shapes that make a
//! scan of this repository empty, and this file is written against all three.
//!
//! **The walk does not stop short.** [`crate_sources`] descends into every
//! subdirectory of the package, and `the_walk_reads_every_module_in_this_crate`
//! carries the floor and a tripwire: every `mod name;` and every `#[path]`
//! target in the crate has to be a file the walk read.
//!
//! **The checks are not token lists.** There is no list of forbidden spellings
//! anywhere in this file, on purpose: this run measured five times over that a
//! forbidden-name list refuses the edits somebody thought of and admits every
//! edit spelled differently, and `P2-RF13` found six real leaks the moment a
//! name list became a whole-set classification. So every load-bearing check
//! here is a **whole set compared in both directions**:
//!
//! * every import rooted outside this crate, per file;
//! * every function declaration in this crate's product source, as a
//!   visibility and a signature;
//! * every field of every type in this crate, each with a class;
//! * every `impl` block whose self type is one of this crate's, with its
//!   derive list;
//! * every file in this crate that reads the retained text;
//! * every file in the **workspace** that names a value an outbound request is
//!   composed from;
//! * every file in the **workspace** outside this crate that names one of this
//!   crate's types.
//!
//! A function, a field, a type or a file nobody predicted fails as an entry
//! nobody wrote down, whatever it is called.
//!
//! **Nothing is unbounded.** Every whole-set comparison is an `assert_eq!`
//! against a pinned list, so an empty walk fails as a set of missing keys, and
//! `the_walk_reads_every_module_in_this_crate` fails first if the walk stops
//! reading.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------
// Reading the tree
// ---------------------------------------------------------------------------

/// This crate's root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The repository root.
///
/// Taken by climbing rather than by joining `..`, because a path with `..`
/// components in it never strips as a prefix and every relative path this file
/// prints would silently become an absolute one.
fn repository_root() -> PathBuf {
    let root = crate_root();
    root.parent()
        .and_then(Path::parent)
        .map_or(root.clone(), Path::to_path_buf)
}

/// A repository-relative path with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(repository_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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

/// Every `.rs` file under this package, recursively.
fn crate_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file this crate ships, which is every one outside `tests`.
///
/// The whole package rather than `src`: `S-12` in
/// `docs/contracts/policy-source-scans.md` is the row about a walk that reads
/// `<crate>/src` and stops seeing product-shaped code beside it, and
/// `examples/`, `probes/` and `benches/` are all compiled by
/// `cargo clippy --workspace --all-targets`. This crate has none of them today;
/// widening the walk now is what stops the first one from being a tree no scan
/// reads.
fn product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    Ok(crate_sources()?
        .into_iter()
        .filter(|path| {
            !path
                .strip_prefix(&root)
                .unwrap_or(path)
                .starts_with("tests")
        })
        .collect())
}

/// Every `.rs` file in every workspace package, product and test alike.
fn workspace_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&repository_root().join("crates"), &mut found)?;
    found.sort();
    Ok(found)
}

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/ingestion/tests/ingestion_scans.rs`, which copied it
/// from `crates/record/tests/record_scans.rs`, where this repository's
/// Rust-side stripper lives -- raw strings and nested block comments included.
/// `P2-G4` found that a lexer without raw strings desynchronizes and reads
/// every literal after one as code, so the copy is deliberate.
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
            out.push('\n');
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
            out.push(' ');
            continue;
        }
        if current == 'r' && matches!(next, Some('"') | Some('#')) {
            let mut probe = index + 1;
            let mut hashes = 0_usize;
            while bytes.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if bytes.get(probe) == Some(&'"') {
                let terminator: String = core::iter::once('"')
                    .chain(core::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = bytes[probe + 1..].iter().collect();
                let end = rest.find(&terminator).map_or(bytes.len(), |offset| {
                    probe + 1 + rest[..offset].chars().count() + terminator.chars().count()
                });
                index = end;
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
            let closes = if next == Some('\\') {
                bytes
                    .iter()
                    .skip(index + 2)
                    .position(|character| *character == '\'')
                    .map(|offset| index + 2 + offset)
            } else {
                (bytes.get(index + 2) == Some(&'\'')).then_some(index + 2)
            };
            if let Some(end) = closes {
                index = end + 1;
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

/// One file's code, with comments and literals removed.
fn code_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(path)?))
}

/// Whether the byte at `at` starts a whole identifier occurrence of `name`.
fn is_whole_identifier(code: &str, at: usize, name: &str) -> bool {
    let before = code[..at].chars().next_back();
    let after = code[at + name.len()..].chars().next();
    let boundary = |character: Option<char>| {
        character.is_none_or(|value| !(value.is_alphanumeric() || value == '_'))
    };
    boundary(before) && boundary(after)
}

/// Counts whole-identifier occurrences of `name` in already-stripped code.
fn identifier_uses(code: &str, name: &str) -> usize {
    code.match_indices(name)
        .filter(|(at, _)| is_whole_identifier(code, *at, name))
        .count()
}

/// Every function declaration in `code`, as a public flag and a signature.
///
/// Visibility is read off the text before `fn` on the same line: `pub(` is
/// crate-private however it continues, a bare `pub` is public, and anything
/// else is private. Reading signatures rather than names is what makes the
/// pinned set below a statement about what a function *takes and returns*: a
/// second function with the same name and a different type fails, and so does
/// the same function with a widened parameter.
///
/// The `>` of a `->` is skipped. `crates/ingestion`'s copy of this reader
/// treats it as a closing bracket, which is harmless there because no signature
/// in that crate returns an array; here `fn counts(self) -> [u32; 5]` came back
/// as `fn counts(self) -> [u32`, because the arrow drove the depth below zero
/// and the `;` inside the array type then read as the end of the declaration.
/// A pin on a truncated signature is a pin two different signatures satisfy,
/// which is the shape of an empty guard, so the arrow is skipped.
fn declarations(code: &str) -> Vec<(bool, String)> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("fn ") {
        if !(at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_')) {
            continue;
        }
        let line_start = code[..at].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &code[line_start..at];
        let public = prefix.contains("pub") && !prefix.contains("pub(");
        let mut depth = 0_i32;
        let mut end = None;
        let region = &code[at..];
        let region_bytes = region.as_bytes();
        for (offset, character) in region.char_indices() {
            match character {
                '(' | '<' | '[' => depth += 1,
                // The `>` of a `->` is not a closing bracket.
                '>' if offset > 0 && region_bytes[offset - 1] == b'-' => {}
                ')' | '>' | ']' => depth -= 1,
                '{' | ';' if depth <= 0 => {
                    end = Some(at + offset);
                    break;
                }
                _ => {}
            }
        }
        if let Some(end) = end {
            found.push((
                public,
                code[at..end]
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            ));
        }
    }
    found
}

/// Every `struct` and `enum` in `code`, with the named fields each declares.
///
/// A named field is `name: Type` at brace depth one inside the item. Enum
/// variants with named fields contribute theirs under the enum's name, which is
/// what makes `CourseReading`'s two arms visible to the classification below.
/// Tuple positions are reported as `.0`, `.1`, ... so a tuple newtype cannot
/// hide a payload by having no name -- that is the hole
/// `tools/secret-debug-policy.test.mjs` documents for a position with no name.
fn fields_of_types(code: &str) -> BTreeMap<String, Vec<(String, String)>> {
    let mut found: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for keyword in ["struct ", "enum "] {
        for (at, _) in code.match_indices(keyword) {
            let bytes = code.as_bytes();
            if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
                continue;
            }
            let rest = &code[at + keyword.len()..];
            let name: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let Some(open) = rest.find(['{', '(', ';']) else {
                continue;
            };
            let opener = rest.as_bytes()[open];
            if opener == b';' {
                found.entry(name).or_default();
                continue;
            }
            let (depth_open, depth_close) = if opener == b'{' {
                ('{', '}')
            } else {
                ('(', ')')
            };
            let mut depth = 0_i32;
            let mut end = rest.len();
            for (offset, character) in rest[open..].char_indices() {
                if character == depth_open {
                    depth += 1;
                } else if character == depth_close {
                    depth -= 1;
                    if depth == 0 {
                        end = open + offset;
                        break;
                    }
                }
            }
            let body = &rest[open + 1..end];
            let entry = found.entry(name).or_default();
            if opener == b'(' {
                for (position, part) in split_top_level(body).into_iter().enumerate() {
                    let part = part.trim().trim_start_matches("pub").trim();
                    if !part.is_empty() {
                        entry.push((format!(".{position}"), collapse(part)));
                    }
                }
                continue;
            }
            for part in split_top_level(body) {
                let part = part.trim();
                // An enum variant with named fields: recurse into its braces.
                if let Some(brace) = part.find('{')
                    && !part[..brace].contains(':')
                {
                    let inner_end = part.rfind('}').unwrap_or(part.len());
                    for inner in split_top_level(&part[brace + 1..inner_end]) {
                        push_named_field(entry, inner.trim());
                    }
                    continue;
                }
                push_named_field(entry, part);
            }
        }
    }
    found
}

/// Records `part` as `name: Type` when it is one.
///
/// A leading attribute is removed by finding its own closing bracket rather
/// than by taking the text after the last `]` in the fragment. The second
/// reading is what this file was written with first, and it dropped
/// `counts: [u32; 5]` entirely -- a field whose *type* ends in a bracket looked
/// like an attribute with nothing after it. That is a hole in an inventory
/// whose whole job is to have none, and it was found by dumping the inventory
/// and reading it rather than by the assertion, which was comparing an
/// incomplete discovery against an empty pin.
fn push_named_field(entry: &mut Vec<(String, String)>, part: &str) {
    let mut part = part.trim();
    while part.starts_with("#[") {
        let Some(close) = part.find(']') else { break };
        part = part[close + 1..].trim();
    }
    let cleaned = part
        .trim_start_matches("pub(crate)")
        .trim_start_matches("pub")
        .trim();
    let Some((name, kind)) = cleaned.split_once(':') else {
        return;
    };
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return;
    }
    entry.push((name.to_owned(), collapse(kind)));
}

/// Splits on commas that are not inside brackets.
fn split_top_level(body: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for character in body.chars() {
        match character {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(core::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    parts.push(current);
    parts
}

/// Whitespace-collapses a fragment.
fn collapse(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// The walk reaches every module, and every module is where the scans expect.
///
/// The floor is what fails if the walk stops finding files: an empty read
/// would otherwise make every set below pass as an empty comparison against an
/// empty pin, and it would not, because the pins are non-empty -- but this
/// fails first and says why.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = crate_sources()?;
    let read: BTreeSet<String> = sources.iter().map(|path| relative(path)).collect();
    assert!(
        read.len() >= 14,
        "the walk read {} files, which is fewer than this crate has",
        read.len()
    );
    assert!(
        read.contains("crates/review/src/lib.rs"),
        "the walk did not reach the crate root"
    );

    // Every module the crate declares is a file the walk read.
    for path in &sources {
        let code = code_of(path)?;
        let directory = path.parent().ok_or("a source file has no directory")?;
        for (at, _) in code.match_indices("mod ") {
            let bytes = code.as_bytes();
            if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
                continue;
            }
            let rest = &code[at + 4..];
            let name: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            let terminated = rest[name.len()..].trim_start().starts_with(';');
            if name.is_empty() || !terminated {
                continue;
            }
            let flat = directory.join(format!("{name}.rs"));
            let nested = directory.join(&name).join("mod.rs");
            assert!(
                read.contains(&relative(&flat)) || read.contains(&relative(&nested)),
                "the walk did not read the module `{name}` declared in {}",
                relative(path)
            );
        }
        assert!(
            !code.contains("#[path"),
            "{} uses a #[path] attribute, which this walk does not follow",
            relative(path)
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// no_login_bypass_or_evasion_module_exists
// ---------------------------------------------------------------------------

/// Every `use` statement in this crate, whole.
///
/// Every one, not only the ones rooted outside: a re-export is how a crate
/// widens what a caller can reach, and a set that skipped them would be a set
/// somebody could add to silently. An HTTP client, a TLS stack, a browser
/// driver, an image decoder, a headless runtime or a cookie store cannot be
/// reached without a line here, and a new one fails as an extra key whatever
/// the module using it is called.
///
/// A statement is read to its `;` rather than to the end of its line. The line
/// reading is what this file was written with first, and it recorded
/// `crates/review/src/text.rs: use academic_untrusted_content::{` -- a braced
/// group whose contents were outside the pin entirely, which is exactly the
/// hole `T114` found in a different scan.
const USE_STATEMENTS: &[&str] = &[
    "crates/review/src/access.rs: use academic_ingestion::{ConnectorId, Denial, DenialReason, TermsStatus, terms::deny};",
    "crates/review/src/aggregate.rs: use academic_curriculum::{InstructorName, TermCode};",
    "crates/review/src/aggregate.rs: use academic_domain::{CourseId, OfferingId};",
    "crates/review/src/aggregate.rs: use crate::{ bias::BiasDisclosure, dimension::{DimensionBand, ReviewDimension}, error::ReviewError, record::ReviewRecord, scope::{ReviewScope, ScopeDimension}, };",
    "crates/review/src/bias.rs: use crate::error::ReviewError;",
    "crates/review/src/dimension.rs: use crate::error::ReviewError;",
    "crates/review/src/duplicate.rs: use crate::{error::ReviewError, record::ReviewRecord};",
    "crates/review/src/duplicate.rs: use std::collections::BTreeSet;",
    "crates/review/src/error.rs: use crate::{bias::BiasDimension, dimension::ReviewDimension, scope::ScopeDimension};",
    "crates/review/src/gate.rs: use core::fmt;",
    "crates/review/src/lib.rs: pub use access::{PermittedCollection, SourceAccessMode, SourceTermsLedger, permit};",
    "crates/review/src/lib.rs: pub use aggregate::{ AggregationClaim, AggregationMethod, BandDistribution, CourseAggregate, CourseReading, OfferingAggregate, OfferingReading, };",
    "crates/review/src/lib.rs: pub use bias::{BiasDimension, BiasDisclosure, BiasDisclosureDraft, BiasFinding, BiasStrength};",
    "crates/review/src/lib.rs: pub use dimension::{DimensionBand, DimensionReading, ReviewDimension, ReviewExtraction};",
    "crates/review/src/lib.rs: pub use duplicate::{ DuplicateFinding, SimilarityPermille, duplicate_findings, duplicated_record_count, similarity, };",
    "crates/review/src/lib.rs: pub use error::ReviewError;",
    "crates/review/src/lib.rs: pub use gate::OpenGate;",
    "crates/review/src/lib.rs: pub use record::{ReviewRecord, SampleBias};",
    "crates/review/src/lib.rs: pub use scope::{ReviewScope, ScopeDimension};",
    "crates/review/src/lib.rs: pub use text::{MAX_REVIEW_BYTES, ProvenanceSpan, RawReviewText};",
    "crates/review/src/record.rs: use academic_domain::EpistemicStatus;",
    "crates/review/src/record.rs: use academic_ingestion::RetrievalInstant;",
    "crates/review/src/record.rs: use academic_proposal::Autosaved;",
    "crates/review/src/record.rs: use crate::{ access::{PermittedCollection, SourceAccessMode}, bias::BiasDimension, dimension::{DimensionBand, ReviewDimension, ReviewExtraction}, scope::ReviewScope, text::{ProvenanceSpan, RawReviewText}, };",
    "crates/review/src/scope.rs: use academic_curriculum::{InstructorName, TermCode};",
    "crates/review/src/scope.rs: use academic_domain::OfferingId;",
    "crates/review/src/scope.rs: use academic_ingestion::ConnectorId;",
    "crates/review/src/text.rs: use academic_untrusted_content::{ IngestError, IngestedDocument, SourceId, Untrusted, ingest_review_text, };",
    "crates/review/src/text.rs: use core::fmt;",
    "crates/review/src/text.rs: use crate::error::ReviewError;",
];

/// The whole set of function declarations in this crate's product source.
///
/// This is the exhaustive net. It is not a list of names to refuse; it is the
/// list of functions that exist. A function added anywhere in this crate --
/// spelling nothing anybody thought to forbid, in a module nobody predicted --
/// fails here as an extra entry, and one deleted fails as a missing one.
///
/// Read it as the answer to "what can this crate do": every entry takes values
/// this crate already holds and returns values this crate already holds. There
/// is no signature from a response to a request, none that takes a credential,
/// a session, a header or a cookie, and none that returns anything an outbound
/// request could be built from.
const DECLARATIONS: &[&str] = &[
    "crates/review/src/access.rs [pub] fn as_str(self) -> &'static str",
    "crates/review/src/access.rs [pub] fn empty() -> Self",
    "crates/review/src/access.rs [pub] fn is_a_person_act(self) -> bool",
    "crates/review/src/access.rs [pub] fn mode(&self) -> SourceAccessMode",
    "crates/review/src/access.rs [pub] fn parse(value: &str) -> Option<Self>",
    "crates/review/src/access.rs [pub] fn permit( ledger: &SourceTermsLedger, source: &ConnectorId, mode: SourceAccessMode, ) -> Result<PermittedCollection, Denial>",
    "crates/review/src/access.rs [pub] fn presents_a_credential(self) -> bool",
    "crates/review/src/access.rs [pub] fn recording( mut self, source: ConnectorId, mode: SourceAccessMode, status: TermsStatus, ) -> Self",
    "crates/review/src/access.rs [pub] fn source(&self) -> &ConnectorId",
    "crates/review/src/access.rs [pub] fn status_of(&self, source: &ConnectorId, mode: SourceAccessMode) -> TermsStatus",
    "crates/review/src/aggregate.rs [priv] fn differing_dimension(left: &ReviewScope, right: &ReviewScope) -> Option<ScopeDimension>",
    "crates/review/src/aggregate.rs [priv] fn pooled(aggregates: &[OfferingAggregate]) -> Vec<BandDistribution>",
    "crates/review/src/aggregate.rs [pub] fn as_str(self) -> &'static str",
    "crates/review/src/aggregate.rs [pub] fn asserted_over(&self) -> &[ReviewScope]",
    "crates/review/src/aggregate.rs [pub] fn asserting( method: AggregationMethod, course: CourseId, aggregates: &[OfferingAggregate], ) -> Self",
    "crates/review/src/aggregate.rs [pub] fn count(self, band: DimensionBand) -> u32",
    "crates/review/src/aggregate.rs [pub] fn counts(self) -> [u32; 5]",
    "crates/review/src/aggregate.rs [pub] fn course(&self) -> CourseId",
    "crates/review/src/aggregate.rs [pub] fn course(&self) -> CourseId",
    "crates/review/src/aggregate.rs [pub] fn dimension(self) -> ReviewDimension",
    "crates/review/src/aggregate.rs [pub] fn disclosure(&self) -> &BiasDisclosure",
    "crates/review/src/aggregate.rs [pub] fn disclosure(&self) -> &BiasDisclosure",
    "crates/review/src/aggregate.rs [pub] fn distribution(&self, dimension: ReviewDimension) -> BandDistribution",
    "crates/review/src/aggregate.rs [pub] fn distributions(&self) -> &[BandDistribution]",
    "crates/review/src/aggregate.rs [pub] fn distributions(&self) -> &[BandDistribution]",
    "crates/review/src/aggregate.rs [pub] fn instructor(&self) -> Option<&InstructorName>",
    "crates/review/src/aggregate.rs [pub] fn method(&self) -> AggregationMethod",
    "crates/review/src/aggregate.rs [pub] fn method(&self) -> AggregationMethod",
    "crates/review/src/aggregate.rs [pub] fn method(&self) -> AggregationMethod",
    "crates/review/src/aggregate.rs [pub] fn offering(&self) -> Option<OfferingId>",
    "crates/review/src/aggregate.rs [pub] fn over(&self) -> &[ReviewScope]",
    "crates/review/src/aggregate.rs [pub] fn over(records: &[ReviewRecord], disclosure: BiasDisclosure) -> Result<Self, ReviewError>",
    "crates/review/src/aggregate.rs [pub] fn promote( claim: AggregationClaim, aggregates: &[OfferingAggregate], disclosure: BiasDisclosure, ) -> Result<Self, ReviewError>",
    "crates/review/src/aggregate.rs [pub] fn reading(&self) -> &CourseReading",
    "crates/review/src/aggregate.rs [pub] fn sample_size(&self) -> u32",
    "crates/review/src/aggregate.rs [pub] fn sample_size(&self) -> u32",
    "crates/review/src/aggregate.rs [pub] fn sample_size(&self) -> u32",
    "crates/review/src/aggregate.rs [pub] fn scope(&self) -> &ReviewScope",
    "crates/review/src/aggregate.rs [pub] fn scope(&self) -> &ReviewScope",
    "crates/review/src/aggregate.rs [pub] fn term(&self) -> Option<&TermCode>",
    "crates/review/src/aggregate.rs [pub] fn total(self) -> u32",
    "crates/review/src/bias.rs [pub] fn as_str(self) -> &'static str",
    "crates/review/src/bias.rs [pub] fn as_str(self) -> &'static str",
    "crates/review/src/bias.rs [pub] fn build(self) -> Result<BiasDisclosure, ReviewError>",
    "crates/review/src/bias.rs [pub] fn dimension(self) -> BiasDimension",
    "crates/review/src/bias.rs [pub] fn disclosed(&self) -> Vec<BiasDimension>",
    "crates/review/src/bias.rs [pub] fn disclosing(mut self, finding: BiasFinding) -> Self",
    "crates/review/src/bias.rs [pub] fn finding(&self, dimension: BiasDimension) -> BiasFinding",
    "crates/review/src/bias.rs [pub] fn findings(&self) -> &[BiasFinding]",
    "crates/review/src/bias.rs [pub] fn index(self) -> usize",
    "crates/review/src/bias.rs [pub] fn measured(self) -> u32",
    "crates/review/src/bias.rs [pub] fn new() -> Self",
    "crates/review/src/bias.rs [pub] fn new(dimension: BiasDimension, measured: u32, strength: BiasStrength) -> Self",
    "crates/review/src/bias.rs [pub] fn spec_phrase(self) -> &'static str",
    "crates/review/src/bias.rs [pub] fn strength(self) -> BiasStrength",
    "crates/review/src/dimension.rs [pub] fn as_str(self) -> &'static str",
    "crates/review/src/dimension.rs [pub] fn as_str(self) -> &'static str",
    "crates/review/src/dimension.rs [pub] fn band(&self, dimension: ReviewDimension) -> DimensionBand",
    "crates/review/src/dimension.rs [pub] fn band(self) -> DimensionBand",
    "crates/review/src/dimension.rs [pub] fn dimension(self) -> ReviewDimension",
    "crates/review/src/dimension.rs [pub] fn index(self) -> usize",
    "crates/review/src/dimension.rs [pub] fn index(self) -> usize",
    "crates/review/src/dimension.rs [pub] fn new(dimension: ReviewDimension, band: DimensionBand, span_index: usize) -> Self",
    "crates/review/src/dimension.rs [pub] fn read(readings: &[DimensionReading]) -> Result<Self, ReviewError>",
    "crates/review/src/dimension.rs [pub] fn reading(&self, dimension: ReviewDimension) -> DimensionReading",
    "crates/review/src/dimension.rs [pub] fn readings(&self) -> &[DimensionReading]",
    "crates/review/src/dimension.rs [pub] fn span_index(self) -> usize",
    "crates/review/src/dimension.rs [pub] fn spec_key(self) -> &'static str",
    "crates/review/src/duplicate.rs [priv] fn shingles(text: &str) -> BTreeSet<String>",
    "crates/review/src/duplicate.rs [priv] fn words(text: &str) -> Vec<String>",
    "crates/review/src/duplicate.rs [pub] fn duplicate_findings( records: &[ReviewRecord], threshold: SimilarityPermille, ) -> Vec<DuplicateFinding>",
    "crates/review/src/duplicate.rs [pub] fn duplicated_record_count(records: &[ReviewRecord], threshold: SimilarityPermille) -> u32",
    "crates/review/src/duplicate.rs [pub] fn left(self) -> usize",
    "crates/review/src/duplicate.rs [pub] fn new(value: u16) -> Result<Self, ReviewError>",
    "crates/review/src/duplicate.rs [pub] fn right(self) -> usize",
    "crates/review/src/duplicate.rs [pub] fn similarity(left: &ReviewRecord, right: &ReviewRecord) -> SimilarityPermille",
    "crates/review/src/duplicate.rs [pub] fn similarity(self) -> SimilarityPermille",
    "crates/review/src/duplicate.rs [pub] fn value(self) -> u16",
    "crates/review/src/gate.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/review/src/gate.rs [pub] fn identifier(self) -> &'static str",
    "crates/review/src/gate.rs [pub] fn question(self) -> &'static str",
    "crates/review/src/gate.rs [pub] fn while_open(self) -> &'static str",
    "crates/review/src/record.rs [pub] fn band(&self, dimension: ReviewDimension) -> DimensionBand",
    "crates/review/src/record.rs [pub] fn collected( collection: &PermittedCollection, scope: ReviewScope, raw_artifact: RawReviewText, collected_at: RetrievalInstant, extraction: Autosaved<ReviewExtraction>, sample_bias: SampleBias, ) -> Self",
    "crates/review/src/record.rs [pub] fn collected_at(&self) -> RetrievalInstant",
    "crates/review/src/record.rs [pub] fn dimensions(&self) -> &ReviewExtraction",
    "crates/review/src/record.rs [pub] fn extraction_status(&self) -> EpistemicStatus",
    "crates/review/src/record.rs [pub] fn flagging(mut self, dimension: BiasDimension) -> Self",
    "crates/review/src/record.rs [pub] fn flags(&self, dimension: BiasDimension) -> bool",
    "crates/review/src/record.rs [pub] fn none() -> Self",
    "crates/review/src/record.rs [pub] fn provenance_spans(&self) -> &[ProvenanceSpan]",
    "crates/review/src/record.rs [pub] fn raw_artifact(&self) -> &RawReviewText",
    "crates/review/src/record.rs [pub] fn sample_bias(&self) -> &SampleBias",
    "crates/review/src/record.rs [pub] fn scope(&self) -> &ReviewScope",
    "crates/review/src/record.rs [pub] fn signals(&self) -> &[BiasDimension]",
    "crates/review/src/record.rs [pub] fn source_access_mode(&self) -> SourceAccessMode",
    "crates/review/src/scope.rs [pub] fn as_str(self) -> &'static str",
    "crates/review/src/scope.rs [pub] fn carries(&self, dimension: ScopeDimension) -> bool",
    "crates/review/src/scope.rs [pub] fn instructor(&self) -> Option<&InstructorName>",
    "crates/review/src/scope.rs [pub] fn is_nullable(self) -> bool",
    "crates/review/src/scope.rs [pub] fn new( source: ConnectorId, offering: Option<OfferingId>, instructor: Option<InstructorName>, term: Option<TermCode>, ) -> Self",
    "crates/review/src/scope.rs [pub] fn offering(&self) -> Option<OfferingId>",
    "crates/review/src/scope.rs [pub] fn same_scope_as(&self, other: &Self) -> bool",
    "crates/review/src/scope.rs [pub] fn source(&self) -> &ConnectorId",
    "crates/review/src/scope.rs [pub] fn spec_name(self) -> &'static str",
    "crates/review/src/scope.rs [pub] fn term(&self) -> Option<&TermCode>",
    "crates/review/src/text.rs [priv] fn content(&self) -> &str",
    "crates/review/src/text.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/review/src/text.rs [priv] fn hex_lower(bytes: &[u8]) -> String",
    "crates/review/src/text.rs [priv] fn span_digest(bytes: &[u8]) -> String",
    "crates/review/src/text.rs [pub] fn byte_len(&self) -> usize",
    "crates/review/src/text.rs [pub] fn digest(&self) -> &str",
    "crates/review/src/text.rs [pub] fn digest(&self) -> &str",
    "crates/review/src/text.rs [pub] fn digest_of(bytes: &[u8]) -> String",
    "crates/review/src/text.rs [pub] fn end(&self) -> usize",
    "crates/review/src/text.rs [pub] fn is_empty(&self) -> bool",
    "crates/review/src/text.rs [pub] fn len(&self) -> usize",
    "crates/review/src/text.rs [pub] fn retain(text: &str, spans: &[(usize, usize, &str)]) -> Result<Self, ReviewError>",
    "crates/review/src/text.rs [pub] fn seal( &self, source_id: SourceId, ingest_seq: u64, ) -> Result<Untrusted<IngestedDocument>, IngestError>",
    "crates/review/src/text.rs [pub] fn spans(&self) -> &[ProvenanceSpan]",
    "crates/review/src/text.rs [pub] fn start(&self) -> usize",
];

/// The whole set of files in the **workspace** whose code names a value an
/// outbound request is composed from.
///
/// `academic-egress-boundary`'s `OutboundTransport` is, by its own module
/// documentation, the only trait in this workspace whose method hands bytes to
/// something outside; `academic-ingestion`'s `ConditionalFetch`,
/// `ConditionalRequest`, `CredentialBinding` and `DeclaredTarget` are the only
/// values a fetch is composed from; `StagingRequest` is what the egress proxy
/// stages. So a module that reaches a source has to name one of the six, or
/// open a socket -- and `only_egress_crate_has_a_socket` in
/// `tools/phase1-scaffold-policy.test.mjs` compares the whole per-file
/// allowance map of socket spellings across every workspace package.
///
/// This crate is deliberately absent from this list. A new file anywhere in the
/// workspace that composes a request fails here as an extra key, whatever it is
/// named and whatever words it avoids.
const OUTBOUND_COMPOSITION_FILES: &[&str] = &[
    "crates/egress-boundary/src/lib.rs",
    "crates/egress-boundary/src/stage.rs",
    "crates/egress-boundary/src/transport.rs",
    "crates/egress-boundary/tests/common/mod.rs",
    "crates/egress-boundary/tests/egress_boundary.rs",
    "crates/egress-boundary/tests/egress_faults.rs",
    "crates/evidence-center/tests/evidence_center.rs",
    "crates/ingestion/src/conflict.rs",
    "crates/ingestion/src/fetch.rs",
    "crates/ingestion/src/lib.rs",
    "crates/ingestion/src/manifest.rs",
    "crates/ingestion/src/snapshot.rs",
    "crates/ingestion/src/stage.rs",
    "crates/ingestion/tests/compile_fail/a_credential_binding_cannot_be_assembled.rs",
    "crates/ingestion/tests/compile_fail/a_fetch_target_cannot_be_built_at_run_time.rs",
    "crates/ingestion/tests/ingestion.rs",
    "crates/ingestion/tests/support/mod.rs",
];

/// How this crate is spelled when another package reaches it.
const CRATE_PATH_NAME: &str = "academic_review";

/// Every file outside `crates/review/` whose code names one of this crate's
/// public types.
///
/// `P2-N8` is the first package to depend on `academic-review`, and it names
/// exactly one of these types plus the error: section 22.4 requires a projected
/// workload to be displayed with its sample count, its recency, its selection
/// bias and its instructor and term mix, and `BiasDisclosure` is the value that
/// carries all six of section 29.5's dimensions. A file elsewhere that wired
/// any of these types into a fetcher still fails here as an extra key before it
/// fails anywhere else.
const FOREIGN_USERS: &[&str] = &[
    "crates/what-if/src/error.rs",
    "crates/what-if/src/inputs.rs",
    "crates/what-if/src/projected.rs",
    "crates/what-if/tests/support/mod.rs",
    "crates/what-if/tests/what_if.rs",
];

/// This crate's public type names, for the reverse search.
///
/// `OpenGate` is deliberately absent. Six crates declare their own section 38
/// gate enum under that name -- `academic-audit`, `academic-consent`,
/// `academic-curriculum`, `academic-ingestion`, `academic-offering` and
/// `academic-requirement` -- so the name identifies a shape rather than this
/// crate, and searching for it would pin twenty-seven files that have nothing
/// to do with reviews. What covers it instead is [`CRATE_PATH_NAME`]: reaching
/// *this* crate's `OpenGate` from another package needs the crate's own path,
/// and both searches below look for that too.
const PUBLIC_TYPES: &[&str] = &[
    "AggregationClaim",
    "AggregationMethod",
    "BandDistribution",
    "BiasDimension",
    "BiasDisclosure",
    "BiasDisclosureDraft",
    "BiasFinding",
    "BiasStrength",
    "CourseAggregate",
    "CourseReading",
    "DimensionBand",
    "DimensionReading",
    "DuplicateFinding",
    "OfferingAggregate",
    "OfferingReading",
    "PermittedCollection",
    "ProvenanceSpan",
    "RawReviewText",
    "ReviewDimension",
    "ReviewError",
    "ReviewExtraction",
    "ReviewRecord",
    "ReviewScope",
    "SampleBias",
    "SourceTermsLedger",
];

/// No module here logs in for somebody, shares an account, or evades a control.
///
/// Five whole sets, each compared in both directions. None of them is a list of
/// forbidden spellings; each is a list of what exists.
#[test]
fn no_login_bypass_or_evasion_module_exists() -> TestResult {
    // 1. What this crate can reach at all.
    let mut imports = BTreeSet::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        let mut pending: Option<String> = None;
        for line in code.lines() {
            let trimmed = line.trim();
            let mut statement = match pending.take() {
                Some(started) => format!("{started} {trimmed}"),
                None if trimmed.starts_with("use ") || trimmed.starts_with("pub use ") => {
                    trimmed.to_owned()
                }
                None => continue,
            };
            if !statement.contains(';') {
                pending = Some(statement);
                continue;
            }
            statement = collapse(&statement);
            imports.insert(format!("{}: {statement}", relative(&path)));
        }
    }
    assert_eq!(
        imports,
        USE_STATEMENTS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate's use-statement set changed"
    );

    // 2. Everything this crate can do.
    let mut declared = Vec::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        for (public, signature) in declarations(&code) {
            let visibility = if public { "pub" } else { "priv" };
            declared.push(format!("{} [{visibility}] {signature}", relative(&path)));
        }
    }
    declared.sort();
    assert_eq!(
        declared,
        DECLARATIONS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's function set changed"
    );

    // 3. Where an outbound request can be composed, workspace-wide.
    let outbound = [
        "OutboundTransport",
        "ConditionalFetch",
        "ConditionalRequest",
        "CredentialBinding",
        "DeclaredTarget",
        "StagingRequest",
    ];
    let mut composing = BTreeSet::new();
    for path in workspace_sources()? {
        let code = code_of(&path)?;
        if outbound.iter().any(|name| identifier_uses(&code, name) > 0) {
            composing.insert(relative(&path));
        }
    }
    assert_eq!(
        composing,
        OUTBOUND_COMPOSITION_FILES
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "the workspace-wide set of files that compose an outbound request changed"
    );
    assert!(
        !composing
            .iter()
            .any(|file| file.starts_with("crates/review/")),
        "this crate now composes an outbound request"
    );

    // 4. Who uses this crate's types, workspace-wide.
    let root = crate_root();
    let mut foreign = BTreeSet::new();
    for path in workspace_sources()? {
        if path.starts_with(&root) {
            continue;
        }
        let code = code_of(&path)?;
        if identifier_uses(&code, CRATE_PATH_NAME) > 0
            || PUBLIC_TYPES
                .iter()
                .any(|name| identifier_uses(&code, name) > 0)
        {
            foreign.insert(relative(&path));
        }
    }
    assert_eq!(
        foreign,
        FOREIGN_USERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "a file outside this crate now names one of its types"
    );

    // 5. The manifest edges, both sections.
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml"))?;
    let edges: Vec<&str> = manifest
        .lines()
        .filter(|line| line.contains("path = \"../") || line.contains(".workspace = true"))
        .map(str::trim)
        .filter(|line| {
            !line.starts_with('#')
                && !line.starts_with("version")
                && !line.starts_with("edition")
                && !line.starts_with("rust-version")
                && !line.starts_with("license")
                && !line.starts_with("publish")
        })
        .collect();
    assert_eq!(
        edges,
        vec![
            "academic-curriculum = { path = \"../curriculum\" }",
            "academic-domain = { path = \"../domain\" }",
            "academic-ingestion = { path = \"../ingestion\" }",
            "academic-proposal = { path = \"../proposal\" }",
            "academic-untrusted-content = { path = \"../untrusted-content\" }",
            "thiserror.workspace = true",
            "academic-untrusted-content = { path = \"../untrusted-content\" }",
            "trybuild.workspace = true",
        ],
        "this crate's declared edges changed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// raw_review_text_is_excluded_from_export_and_share
// ---------------------------------------------------------------------------

/// The whole set of `impl` block headers in this crate.
///
/// A `Display`, a `ToString`, a `Serialize`, an `AsRef<str>` or a
/// `From<RawReviewText> for String` added anywhere fails here as an extra key.
/// The orphan rule closes the same thing from outside: both the trait and the
/// type would be foreign in another crate.
const IMPL_HEADERS: &[&str] = &[
    "crates/review/src/access.rs: impl PermittedCollection",
    "crates/review/src/access.rs: impl SourceAccessMode",
    "crates/review/src/access.rs: impl SourceTermsLedger",
    "crates/review/src/aggregate.rs: impl AggregationClaim",
    "crates/review/src/aggregate.rs: impl AggregationMethod",
    "crates/review/src/aggregate.rs: impl BandDistribution",
    "crates/review/src/aggregate.rs: impl CourseAggregate",
    "crates/review/src/aggregate.rs: impl CourseReading",
    "crates/review/src/aggregate.rs: impl OfferingAggregate",
    "crates/review/src/aggregate.rs: impl OfferingReading",
    "crates/review/src/bias.rs: impl BiasDimension",
    "crates/review/src/bias.rs: impl BiasDisclosure",
    "crates/review/src/bias.rs: impl BiasDisclosureDraft",
    "crates/review/src/bias.rs: impl BiasFinding",
    "crates/review/src/bias.rs: impl BiasStrength",
    "crates/review/src/dimension.rs: impl DimensionBand",
    "crates/review/src/dimension.rs: impl DimensionReading",
    "crates/review/src/dimension.rs: impl ReviewDimension",
    "crates/review/src/dimension.rs: impl ReviewExtraction",
    "crates/review/src/duplicate.rs: impl DuplicateFinding",
    "crates/review/src/duplicate.rs: impl SimilarityPermille",
    "crates/review/src/gate.rs: impl OpenGate",
    "crates/review/src/gate.rs: impl fmt::Display for OpenGate",
    "crates/review/src/record.rs: impl ReviewRecord",
    "crates/review/src/record.rs: impl SampleBias",
    "crates/review/src/scope.rs: impl ReviewScope",
    "crates/review/src/scope.rs: impl ScopeDimension",
    "crates/review/src/text.rs: impl ProvenanceSpan",
    "crates/review/src/text.rs: impl RawReviewText",
    "crates/review/src/text.rs: impl fmt::Debug for RawReviewText",
];

/// Every file in this crate whose code calls the retained text's accessor.
const TEXT_READERS: &[&str] = &["crates/review/src/duplicate.rs"];

/// Every public function in this crate that returns text or bytes.
///
/// Two, and both return a hexadecimal digest this crate computed. Neither
/// returns a byte of what somebody wrote.
const PUBLIC_TEXT_RETURNS: &[&str] = &[
    "crates/review/src/text.rs: fn digest(&self) -> &str",
    "crates/review/src/text.rs: fn digest(&self) -> &str",
    "crates/review/src/text.rs: fn digest_of(bytes: &[u8]) -> String",
];

/// Somebody else's writing is retained and never redistributed.
///
/// The claim is executed in four parts, and the fourth is about
/// `crates/export` specifically rather than about this crate alone.
#[test]
fn raw_review_text_is_excluded_from_export_and_share() -> TestResult {
    // 1. What this crate's types implement, whole.
    let mut headers = BTreeSet::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        for (at, _) in code.match_indices("impl") {
            let bytes = code.as_bytes();
            if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
                continue;
            }
            if code[at + 4..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let Some(end) = code[at..].find(['{', ';']) else {
                continue;
            };
            headers.insert(format!(
                "{}: {}",
                relative(&path),
                collapse(&code[at..at + end])
            ));
        }
    }
    assert_eq!(
        headers,
        IMPL_HEADERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate's impl inventory changed"
    );

    // 2. Who reads the retained text.
    let mut readers = BTreeSet::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        if code.contains(".content()") {
            readers.insert(relative(&path));
        }
    }
    assert_eq!(
        readers,
        TEXT_READERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "the set of files that read a retained review changed"
    );

    // 3. What a caller outside this crate can get as text or bytes.
    let text_returns = [
        "-> String",
        "-> &str",
        "-> &String",
        "-> Vec<u8>",
        "-> &[u8]",
    ];
    let mut returning = Vec::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        for (public, signature) in declarations(&code) {
            let collapsed = collapse(&signature);
            if public
                && text_returns
                    .iter()
                    .any(|shape| collapsed.replace(" ", "").contains(&shape.replace(" ", "")))
            {
                returning.push(format!("{}: {collapsed}", relative(&path)));
            }
        }
    }
    returning.sort();
    assert_eq!(
        returning,
        PUBLIC_TEXT_RETURNS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's public text-returning surface changed"
    );

    // 4. `P2-P1`'s bundle, directly.
    let export_root = repository_root().join("crates").join("export");
    let mut export_files = Vec::new();
    walk(&export_root, &mut export_files)?;
    assert!(
        export_files.len() >= 10,
        "the export crate walk read {} files",
        export_files.len()
    );
    for path in &export_files {
        let code = code_of(path)?;
        for name in PUBLIC_TYPES.iter().copied() {
            assert_eq!(
                identifier_uses(&code, name),
                0,
                "{} names this crate's {name}",
                relative(path)
            );
        }
        assert_eq!(
            identifier_uses(&code, "academic_review"),
            0,
            "{} names this crate",
            relative(path)
        );
    }
    let export_manifest = fs::read_to_string(export_root.join("Cargo.toml"))?;
    assert!(
        !export_manifest.contains("academic-review"),
        "the export crate declares an edge to this one"
    );
    assert!(
        !fs::read_to_string(crate_root().join("Cargo.toml"))?.contains("academic-export"),
        "this crate declares an edge to the export crate"
    );

    // A bundle row is filled from a `String` the caller already holds, so the
    // question is whether a `String` of a review can exist. Part 3 says the
    // whole public text-returning surface is two digests, and part 1 says no
    // conversion trait produces one. This is the remaining half: nothing in
    // this crate serialises.
    for path in product_sources()? {
        let source = fs::read_to_string(&path)?;
        for forbidden in ["Serialize", "Deserialize", "serde"] {
            assert_eq!(
                identifier_uses(&strip_non_code(&source), forbidden),
                0,
                "{} reaches {forbidden}",
                relative(&path)
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The field inventory
// ---------------------------------------------------------------------------

/// Every field of every type in this crate: its type, and what it holds.
///
/// `S-18` on `docs/contracts/policy-source-scans.md` records that
/// `tools/secret-debug-policy.test.mjs`'s text half is open: ten registrations
/// there are inert because a `String` field under a name outside
/// `SECRET_FIELD_NAMES` is judged by nothing. So this crate does not rely on
/// that tool for its own text. It classifies **every** field it declares, the
/// way `P2-RF13` classified every byte buffer in the workspace, and a field
/// nobody classified fails whatever it is called.
///
/// Each row is `(type, field, field type, class)`. The first three are compared
/// against what the walk discovers, in both directions, so a field added, a
/// field removed, or a field whose *type* changed all fail. The class is the
/// judgement, from a closed vocabulary:
///
/// * `review-content` -- a byte somebody else wrote. Its holder has to
///   hand-write `Debug`, and no field of any other class may have a bare text
///   type.
/// * `digest` -- a hexadecimal digest this crate computed.
/// * `identifier` -- a name or opaque identifier naming a thing.
/// * `count` -- a number this crate counted, an offset it recorded, or a clock
///   reading it was handed.
/// * `enum` -- one of this crate's closed vocabularies.
/// * `composite` -- a value of another classified type, or a collection of
///   them.
///
/// Two fields of one type that share a name are one row: `ReviewError`'s two
/// struct-shaped variants both carry `start` and `end`, and both are the same
/// offset under the same class. A pair that differed would need two rows and
/// this reader would not give them; that is written down here rather than left
/// for somebody to find.
const FIELD_CLASSES: &[(&str, &str, &str, &str)] = &[
    (
        "AggregationClaim",
        "asserted_over",
        "Vec<ReviewScope>",
        "composite",
    ),
    ("AggregationClaim", "course", "CourseId", "identifier"),
    ("AggregationClaim", "method", "AggregationMethod", "enum"),
    ("BandDistribution", "counts", "[u32; 5]", "count"),
    ("BandDistribution", "dimension", "ReviewDimension", "enum"),
    (
        "BiasDisclosure",
        "findings",
        "Vec<BiasFinding>",
        "composite",
    ),
    (
        "BiasDisclosureDraft",
        "findings",
        "Vec<BiasFinding>",
        "composite",
    ),
    ("BiasFinding", "dimension", "BiasDimension", "enum"),
    ("BiasFinding", "measured", "u32", "count"),
    ("BiasFinding", "strength", "BiasStrength", "enum"),
    ("CourseAggregate", "course", "CourseId", "identifier"),
    (
        "CourseAggregate",
        "disclosure",
        "BiasDisclosure",
        "composite",
    ),
    ("CourseAggregate", "method", "AggregationMethod", "enum"),
    ("CourseAggregate", "over", "Vec<ReviewScope>", "composite"),
    ("CourseAggregate", "reading", "CourseReading", "composite"),
    ("CourseAggregate", "sample_size", "u32", "count"),
    (
        "CourseReading",
        "distributions",
        "Vec<BandDistribution>",
        "composite",
    ),
    (
        "CourseReading",
        "offerings",
        "Vec<OfferingReading>",
        "composite",
    ),
    ("DimensionReading", "band", "DimensionBand", "enum"),
    ("DimensionReading", "dimension", "ReviewDimension", "enum"),
    ("DimensionReading", "span_index", "usize", "count"),
    ("DuplicateFinding", "left", "usize", "count"),
    ("DuplicateFinding", "right", "usize", "count"),
    (
        "DuplicateFinding",
        "similarity",
        "SimilarityPermille",
        "composite",
    ),
    (
        "OfferingAggregate",
        "disclosure",
        "BiasDisclosure",
        "composite",
    ),
    (
        "OfferingAggregate",
        "distributions",
        "Vec<BandDistribution>",
        "composite",
    ),
    ("OfferingAggregate", "sample_size", "u32", "count"),
    ("OfferingAggregate", "scope", "ReviewScope", "composite"),
    (
        "OfferingReading",
        "distributions",
        "Vec<BandDistribution>",
        "composite",
    ),
    ("OfferingReading", "sample_size", "u32", "count"),
    ("OfferingReading", "scope", "ReviewScope", "composite"),
    ("PermittedCollection", "mode", "SourceAccessMode", "enum"),
    ("PermittedCollection", "source", "ConnectorId", "identifier"),
    ("ProvenanceSpan", "digest", "String", "digest"),
    ("ProvenanceSpan", "end", "usize", "count"),
    ("ProvenanceSpan", "start", "usize", "count"),
    ("RawReviewText", "digest", "String", "digest"),
    ("RawReviewText", "source_bytes", "String", "review-content"),
    ("RawReviewText", "spans", "Vec<ProvenanceSpan>", "composite"),
    ("ReviewError", "end", "usize", "count"),
    ("ReviewError", "start", "usize", "count"),
    (
        "ReviewExtraction",
        "readings",
        "Vec<DimensionReading>",
        "composite",
    ),
    ("ReviewRecord", "collected_at", "RetrievalInstant", "count"),
    (
        "ReviewRecord",
        "dimensions",
        "ReviewExtraction",
        "composite",
    ),
    ("ReviewRecord", "raw_artifact", "RawReviewText", "composite"),
    ("ReviewRecord", "sample_bias", "SampleBias", "composite"),
    ("ReviewRecord", "scope", "ReviewScope", "composite"),
    (
        "ReviewRecord",
        "source_access_mode",
        "SourceAccessMode",
        "enum",
    ),
    (
        "ReviewScope",
        "instructor",
        "Option<InstructorName>",
        "identifier",
    ),
    (
        "ReviewScope",
        "offering",
        "Option<OfferingId>",
        "identifier",
    ),
    ("ReviewScope", "source", "ConnectorId", "identifier"),
    ("ReviewScope", "term", "Option<TermCode>", "identifier"),
    ("SampleBias", "signals", "Vec<BiasDimension>", "composite"),
    ("SimilarityPermille", ".0", "u16", "count"),
    (
        "SourceTermsLedger",
        "recorded",
        "Vec<(ConnectorId, SourceAccessMode, TermsStatus)>",
        "composite",
    ),
];

/// Types in this crate that hand-write `Debug` because they hold content.
const HAND_WRITTEN_DEBUG: &[&str] = &["RawReviewText"];

/// The whole vocabulary a class may be drawn from.
const FIELD_VOCABULARY: &[&str] = &[
    "review-content",
    "digest",
    "identifier",
    "count",
    "enum",
    "composite",
];

/// Every field is classified, and everything holding content redacts.
#[test]
fn every_field_of_every_type_is_classified() -> TestResult {
    let mut discovered: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut derives: BTreeMap<String, String> = BTreeMap::new();
    for path in product_sources()? {
        let source = fs::read_to_string(&path)?;
        let code = strip_non_code(&source);
        for (name, fields) in fields_of_types(&code) {
            for (field, kind) in fields {
                discovered.insert((name.clone(), field, kind));
            }
            // The derive list immediately above the item, from the unstripped
            // source, so a `#[derive(Debug)]` on a content-holding type fails.
            if let Some(at) = source
                .find(&format!("struct {name} "))
                .or_else(|| source.find(&format!("struct {name}(")))
                .or_else(|| source.find(&format!("enum {name} ")))
            {
                let head = &source[..at];
                let derive = head
                    .rfind("#[derive(")
                    .filter(|start| head[*start..].lines().count() <= 3)
                    .map(|start| {
                        let rest = &head[start..];
                        let end = rest.find(")]").map_or(rest.len(), |offset| offset + 2);
                        collapse(&rest[..end])
                    })
                    .unwrap_or_default();
                derives.insert(name.clone(), derive);
            }
        }
    }
    let expected: BTreeSet<(String, String, String)> = FIELD_CLASSES
        .iter()
        .map(|(type_name, field, kind, _)| {
            (
                (*type_name).to_owned(),
                (*field).to_owned(),
                (*kind).to_owned(),
            )
        })
        .collect();
    assert_eq!(
        discovered, expected,
        "this crate's field inventory changed; classify the new field"
    );

    // Every class is one of the six, and every field whose *type* is text is
    // classified as content or as a digest. The type decides where it can, the
    // way `P2-RF13` made a byte buffer's type decide: a `String` field under an
    // innocent name cannot be classified `count` and pass.
    for (type_name, field, kind, class) in FIELD_CLASSES.iter().copied() {
        assert!(
            FIELD_VOCABULARY.contains(&class),
            "{type_name}.{field} is classified `{class}`, which is not one of the six"
        );
        let bare = kind
            .trim_start_matches('&')
            .trim_start_matches("'static ")
            .trim();
        let text = matches!(bare, "String" | "str" | "Vec<u8>" | "[u8]");
        assert!(
            !text || matches!(class, "review-content" | "digest"),
            "{type_name}.{field} is `{kind}` and classified `{class}`"
        );
    }

    // Every field classified `review-content` lives in a type that hand-writes
    // `Debug`, and every type that hand-writes one holds such a field.
    let content_holders: BTreeSet<&str> = FIELD_CLASSES
        .iter()
        .filter(|(_, _, _, class)| *class == "review-content")
        .map(|(type_name, _, _, _)| *type_name)
        .collect();
    assert_eq!(
        content_holders,
        HAND_WRITTEN_DEBUG.iter().copied().collect::<BTreeSet<_>>(),
        "the set of content-holding types and the set that redact have diverged"
    );
    for holder in HAND_WRITTEN_DEBUG.iter().copied() {
        let derive = derives
            .get(holder)
            .ok_or_else(|| format!("{holder} was not discovered"))?;
        assert!(
            !derive.contains("Debug"),
            "{holder} derives Debug while holding review content"
        );
        let text_module = fs::read_to_string(crate_root().join("src").join("text.rs"))?;
        assert!(
            text_module.contains(&format!("impl fmt::Debug for {holder}")),
            "{holder} does not hand-write Debug"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// One producer, and every refusal driven
// ---------------------------------------------------------------------------

/// The whole set of signatures in this crate that can return a course-level
/// value.
///
/// One, and it takes the claim by value. `T186` measured the failure this
/// closes in `P2-U5`'s crate: the same guard written at two sites, with a test
/// driving one of them, so the other could be relaxed and every test still
/// passed. A second producer here fails as an extra key before anybody has to
/// notice that its refusals are undriven.
const COURSE_PRODUCERS: &[&str] = &[
    "crates/review/src/aggregate.rs: fn promote( claim: AggregationClaim, aggregates: &[OfferingAggregate], disclosure: BiasDisclosure, ) -> Result<Self, ReviewError>",
];

/// The whole set of signatures that can return an offering-level value.
const OFFERING_PRODUCERS: &[&str] = &[
    "crates/review/src/aggregate.rs: fn over(records: &[ReviewRecord], disclosure: BiasDisclosure) -> Result<Self, ReviewError>",
];

/// The whole set of signatures that can return a review record.
const RECORD_PRODUCERS: &[&str] = &[
    "crates/review/src/record.rs: fn collected( collection: &PermittedCollection, scope: ReviewScope, raw_artifact: RawReviewText, collected_at: RetrievalInstant, extraction: Autosaved<ReviewExtraction>, sample_bias: SampleBias, ) -> Self",
];

/// Each aggregate and the record have exactly one producer.
///
/// A producer is a function inside that type's `impl` block whose return type
/// is `Self` or a `Result` over it. Reading the `impl` block rather than
/// searching for the type name is what makes this about *producing* the value
/// rather than about mentioning it.
#[test]
fn each_value_has_one_producer() -> TestResult {
    for (type_name, expected) in [
        ("CourseAggregate", COURSE_PRODUCERS),
        ("OfferingAggregate", OFFERING_PRODUCERS),
        ("ReviewRecord", RECORD_PRODUCERS),
    ] {
        let mut producers = Vec::new();
        for path in product_sources()? {
            let code = code_of(&path)?;
            let header = format!("impl {type_name} {{");
            let Some(start) = code.find(&header) else {
                continue;
            };
            // The block runs to the closing brace at depth zero.
            let mut depth = 0_i32;
            let mut end = code.len();
            for (offset, character) in code[start..].char_indices() {
                if character == '{' {
                    depth += 1;
                } else if character == '}' {
                    depth -= 1;
                    if depth == 0 {
                        end = start + offset;
                        break;
                    }
                }
            }
            for (_, signature) in declarations(&code[start..end]) {
                let collapsed = collapse(&signature);
                let returns_self = collapsed.contains("-> Self")
                    || collapsed.contains("-> Result<Self,")
                    || collapsed.contains("-> Result<Self ,");
                if returns_self {
                    producers.push(format!("{}: {collapsed}", relative(&path)));
                }
            }
        }
        producers.sort();
        assert_eq!(
            producers,
            expected
                .iter()
                .map(|entry| (*entry).to_owned())
                .collect::<Vec<_>>(),
            "the set of functions that produce a {type_name} changed"
        );
    }
    Ok(())
}
