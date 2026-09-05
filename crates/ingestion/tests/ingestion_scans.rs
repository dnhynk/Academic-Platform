//! Source scans for `P2-U6`.
//!
//! Three of this task's named acceptance cases are statements about what the
//! source does *not* contain — `no_numeric_source_winner`,
//! `credentials_never_reach_a_general_crawler`, and
//! `no_captcha_or_access_control_bypass_module_exists`. A behavioural test
//! cannot observe an absence, so they are here.
//!
//! `docs/contracts/policy-source-scans.md` records the three shapes that make a
//! scan of this repository empty, and this file is written against all three.
//!
//! **The walk does not stop short.** [`crate_sources`] descends into every
//! subdirectory of the package. `the_walk_reads_every_module_in_this_crate`
//! carries the floor and a tripwire: every `mod name;` and every `#[path]`
//! target in the crate has to be a file the walk read.
//!
//! **The checks are not token lists.** The load-bearing halves are whole-set
//! comparisons — the item set of `conflict.rs`, the signature set of every
//! function anywhere in the crate that takes a conflict value or a credential
//! binding, the external import set, and the workspace-wide inventory of files
//! that name this crate's request, target and credential types. A name that
//! nobody predicted fails as an extra key rather than passing a list. The one
//! genuine token list — the counting and positioning vocabulary in
//! [`numeric_findings`] — sits *beside* the numeric-type and numeric-literal
//! rules, which have no vocabulary, and beside the whole-text pin on the two
//! functions that could hold such a call.
//!
//! **Nothing is unbounded.** Every whole-set comparison is an `assert_eq!`
//! against a pinned list, so an empty walk fails as a set of missing keys.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

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

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/record/tests/record_scans.rs`, which is where this
/// repository's Rust-side stripper lives, raw strings and nested block comments
/// included. `P2-G4` found that a lexer without raw strings desynchronizes and
/// reads every literal after one as code, so the copy is deliberate.
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
    let bytes = code.as_bytes();
    let before_ok = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
    let after = bytes.get(at + name.len()).copied().unwrap_or(b' ');
    before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
}

/// Counts whole-identifier occurrences of `name` in already-stripped code.
fn identifier_uses(code: &str, name: &str) -> usize {
    code.match_indices(name)
        .filter(|(at, _)| is_whole_identifier(code, *at, name))
        .count()
}

/// Every function signature in `code`, whitespace-collapsed.
///
/// A signature runs from `fn` to the `{` or `;` that follows its return type at
/// nesting depth zero. Reading signatures rather than names is what makes the
/// pinned sets below statements about what a function *takes and returns*: a
/// second function with the same name and a different type fails, and so does
/// the same function with a widened parameter.
fn signatures(code: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let bytes = code.as_bytes();
    for (at, _) in code.match_indices("fn ") {
        if !(at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_')) {
            continue;
        }
        let mut depth = 0_i32;
        let mut end = None;
        for (offset, character) in code[at..].char_indices() {
            match character {
                '(' | '<' | '[' => depth += 1,
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
                at,
                code[at..end]
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            ));
        }
    }
    found
}

/// Every `impl` block in `code`, as the byte range of its body and the type it
/// is written on.
///
/// The subject is what follows `for` in a trait implementation and what follows
/// `impl` otherwise, generic arguments and a `where` clause dropped, reduced to
/// its last path segment. `impl core::fmt::Debug for FetchOutcome` is
/// `FetchOutcome`; `impl ConditionalRequest` is `ConditionalRequest`.
fn impl_blocks(code: &str) -> Vec<(usize, usize, String)> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("impl") {
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        if code[at + 4..]
            .starts_with(|character: char| character.is_alphanumeric() || character == '_')
        {
            continue;
        }
        let mut depth = 0_i32;
        let mut open = None;
        for (offset, character) in code[at..].char_indices() {
            match character {
                '<' | '(' | '[' => depth += 1,
                '>' | ')' | ']' => depth -= 1,
                ';' if depth <= 0 => break,
                '{' if depth <= 0 => {
                    open = Some(at + offset);
                    break;
                }
                _ => {}
            }
        }
        let Some(open) = open else {
            continue;
        };
        let header = &code[at..open];
        let mut close = None;
        let mut braces = 0_i32;
        for (offset, character) in code[open..].char_indices() {
            match character {
                '{' => braces += 1,
                '}' => {
                    braces -= 1;
                    if braces == 0 {
                        close = Some(open + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            continue;
        };
        let after_impl = header.get(4..).unwrap_or_default();
        // `impl<'a, T: Bound>` -- the generic list belongs to the `impl`, not
        // to the subject, so it is stepped over before anything is read.
        let trimmed = after_impl.trim_start();
        let subject_region = if trimmed.starts_with('<') {
            let mut depth = 0_i32;
            let mut end = trimmed.len();
            for (offset, character) in trimmed.char_indices() {
                match character {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            end = offset + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            trimmed.get(end..).unwrap_or_default()
        } else {
            trimmed
        };
        let mut subject = subject_region;
        // `for` at depth zero separates the trait from the type it is on.
        let mut depth = 0_i32;
        for (offset, character) in subject_region.char_indices() {
            match character {
                '<' | '(' | '[' => depth += 1,
                '>' | ')' | ']' => depth -= 1,
                _ => {}
            }
            if depth == 0 && subject_region[offset..].starts_with(" for ") {
                subject = subject_region.get(offset + 5..).unwrap_or_default();
            }
        }
        let subject = subject
            .split(" where ")
            .next()
            .unwrap_or_default()
            .split('<')
            .next()
            .unwrap_or_default()
            .trim()
            .rsplit("::")
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        found.push((open, close, subject));
    }
    found
}

/// The type an item at `at` is written on, if it is inside an `impl` block.
///
/// The innermost enclosing block, so a nested `impl` inside a function body
/// resolves to itself rather than to whatever encloses it.
fn owner_at(blocks: &[(usize, usize, String)], at: usize) -> Option<&str> {
    blocks
        .iter()
        .filter(|(open, close, _)| at > *open && at < *close)
        .min_by_key(|(open, close, _)| close - open)
        .map(|(_, _, subject)| subject.as_str())
}

/// A signature with `Self` and the receiver resolved against `owner`.
///
/// `P2-R6` measured what this is for: a signature's text cannot see what
/// `&self` is, and `pub fn emphasis(&self) -> u32` passed a scan about which
/// types may be folded to a number because the type it was written on was
/// nowhere in the string. `P2-A3` measured the same hole from the other side —
/// the idiomatic `Self` inside `impl ConditionalRequest` put a
/// challenge-to-request function past a set that was supposed to hold every
/// signature producing or consuming a request. Both are the same defect: a
/// reader that is not owner-aware sees a method as if it belonged to nobody.
fn owner_resolved(signature: &str, owner: &str) -> String {
    let mut out = String::with_capacity(signature.len());
    let mut skip_to = 0_usize;
    for (index, character) in signature.char_indices() {
        if index < skip_to {
            continue;
        }
        let matched = ["Self", "self"].into_iter().find(|name| {
            signature[index..].starts_with(name) && is_whole_identifier(signature, index, name)
        });
        match matched {
            Some(name) => {
                out.push_str(owner);
                skip_to = index + name.len();
            }
            None => out.push(character),
        }
    }
    out
}

/// Every function declaration in `code`, as a public flag and a signature.
///
/// Visibility is read off the text before `fn` on the same line: `pub(` is
/// crate-private however it continues, a bare `pub` is public, and anything
/// else is private. That is what lets the pinned surfaces below be about what a
/// caller outside this crate can reach.
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
        for (offset, character) in code[at..].char_indices() {
            match character {
                '(' | '<' | '[' => depth += 1,
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

/// The public surface of one `impl` block, sorted.
///
/// A whole-set pin on what a type lets a caller do. The block is taken from the
/// unstripped source so `declared_item`'s comment filter runs, and the reader
/// above is what finds the multi-line declarations a line filter would only see
/// the first line of.
fn public_surface(source: &str, header: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let block = declared_item(source, header, "\n}")?;
    let mut surface: Vec<String> = declarations(&block)
        .into_iter()
        .filter_map(|(public, signature)| public.then_some(signature))
        .collect();
    surface.sort();
    Ok(surface)
}

/// Every signature anywhere in this crate's product source that names one of
/// `types`, paired with the file it is in and the type it is written on.
///
/// Both the membership test and the recorded entry use the **owner-resolved**
/// text, so a method inside `impl ConditionalRequest` is in the set whether it
/// writes `Self`, `self` or the type's own name. Before `P2-A3`, the set was
/// keyed on the spelling: `fn from_challenge(previous: Self, answered:
/// &FetchOutcome) -> Self` — a request derived from a response, carrying the
/// previous request's credential forward — sat in `fetch.rs` with the whole
/// suite green, and the identical method with the two `Self`s written out
/// failed. The owner is part of the key as well as of the text, so two types
/// with an identically spelled method are two entries rather than one.
fn signatures_naming(types: &[&str]) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut found = BTreeSet::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        let blocks = impl_blocks(&code);
        for (at, signature) in signatures(&code) {
            // A free function and a trait method have no owner to resolve
            // against: the first has no receiver and the second's is whichever
            // type implements it, so both keep the text they were written with.
            let owner = owner_at(&blocks, at);
            let resolved = owner.map_or_else(
                || signature.clone(),
                |subject| owner_resolved(&signature, subject),
            );
            if types
                .iter()
                .any(|name| identifier_uses(&resolved, name) > 0)
            {
                found.insert(format!(
                    "{}: [{}] {resolved}",
                    relative(&path),
                    owner.unwrap_or("-")
                ));
            }
        }
    }
    Ok(found)
}

/// Extracts one item's text, comment lines dropped and whitespace collapsed.
fn declared_item(source: &str, signature: &str, closing: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find(closing)
        .ok_or_else(|| format!("{signature} has no closing brace"))?;
    let body = &source[start..start + end + closing.len()];
    Ok(body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// The walk reaches every module, and every module is where the scans expect.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = crate_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 17,
        "the walk found only {} files under the package",
        sources.len()
    );

    // Product source lives under `src` and nowhere else. That is the condition
    // `S-12` says a crate has to keep if it does not want to widen every scan
    // that reads it.
    let root = crate_root();
    let outside: Vec<String> = product_sources()?
        .iter()
        .filter(|path| !path.strip_prefix(&root).unwrap_or(path).starts_with("src"))
        .map(|path| relative(path))
        .collect();
    assert_eq!(
        outside,
        Vec::<String>::new(),
        "this crate has product source outside src; every scan that reads it has to widen"
    );

    let mut read: BTreeSet<String> = BTreeSet::new();
    for path in &sources {
        if let Some(stem) = path.file_stem() {
            let stem = stem.to_string_lossy().into_owned();
            if stem == "mod" {
                if let Some(parent) = path.parent().and_then(Path::file_name) {
                    read.insert(parent.to_string_lossy().into_owned());
                }
            } else {
                read.insert(stem);
            }
        }
    }

    // The tripwire. Every `mod name;` and every `#[path = "…"]` in the crate
    // has to name a file the walk read. It fails the day the walk is narrowed,
    // and the day a module is added somewhere the walk does not descend into.
    let mut declared = 0_usize;
    for path in &sources {
        let source = fs::read_to_string(path)?;
        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))
                .and_then(|rest| rest.strip_suffix(';'))
            {
                declared += 1;
                assert!(
                    read.contains(name),
                    "`{name}` is declared in {} and the walk never read it",
                    relative(path)
                );
            }
            if let Some(rest) = trimmed.strip_prefix("#[path = \"") {
                let target = rest.split('"').next().unwrap_or_default();
                let resolved = path
                    .parent()
                    .map_or_else(|| PathBuf::from(target), |parent| parent.join(target));
                assert!(
                    sources.iter().any(|read_path| read_path == &resolved),
                    "{} includes {target}, which the walk never read",
                    relative(path)
                );
            }
        }
    }
    assert!(declared >= 13, "the crate declares only {declared} modules");
    Ok(())
}

// ---------------------------------------------------------------------------
// no_numeric_source_winner
// ---------------------------------------------------------------------------

/// Numeric type names, as whole identifiers.
const NUMERIC_TYPES: [&str; 14] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32",
    "f64",
];

/// Operations that turn a collection into a position, a count, or an order.
///
/// This one *is* a vocabulary, and it is the weakest of the three rules in
/// [`numeric_findings`] for exactly that reason. It sits beside two rules with
/// no vocabulary at all — a numeric type under any spelling, and a numeric
/// literal under any spelling — and beside the whole-text pins on the two
/// functions in `conflict.rs` that could hold such a call. An operation not on
/// this list still has to put its result somewhere, and a comparison of two
/// counts is a comparison of two values of a numeric type.
const ORDERING_OPERATIONS: [&str; 18] = [
    "len",
    "count",
    "position",
    "rposition",
    "enumerate",
    "nth",
    "sort",
    "sort_by",
    "sort_by_key",
    "sort_unstable",
    "min",
    "max",
    "min_by_key",
    "max_by_key",
    "cmp",
    "partial_cmp",
    "sum",
    "product",
];

/// Every way `code` could reach a number, under any spelling.
///
/// Three shapes, and only the third is a vocabulary:
///
/// - a numeric type, spelled anywhere;
/// - a numeric literal, decimal, hexadecimal or binary, which is a number
///   whatever it is assigned to;
/// - a counting, positioning or ordering operation.
fn numeric_findings(code: &str) -> Vec<String> {
    let mut findings = Vec::new();

    for name in NUMERIC_TYPES {
        let uses = identifier_uses(code, name);
        if uses > 0 {
            findings.push(format!("numeric type `{name}`"));
        }
    }
    for name in ORDERING_OPERATIONS {
        if identifier_uses(code, name) > 0 {
            findings.push(format!("ordering operation `{name}`"));
        }
    }

    let characters: Vec<char> = code.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        if !characters[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        // Do not start inside an identifier such as `art_12` or `u8`.
        if index > 0 {
            let previous = characters[index - 1];
            if previous.is_alphanumeric() || previous == '_' {
                index += 1;
                continue;
            }
        }
        let start = index;
        while index < characters.len()
            && (characters[index].is_ascii_alphanumeric() || characters[index] == '_')
        {
            index += 1;
        }
        let literal: String = characters[start..index].iter().collect();
        findings.push(format!("numeric literal `{literal}`"));
    }
    findings
}

/// The whole set of item headers `conflict.rs` declares.
///
/// A pin on the module's shape. A function added there — under any name, doing
/// anything — fails as an extra key, which is what makes the numeric rules
/// below a statement about the module rather than about the spellings they
/// know.
const CONFLICT_ITEMS: [&str; 24] = [
    "pub enum ConflictDimension",
    "impl ConflictDimension",
    "pub enum Side",
    "impl Side",
    "pub enum DateComparison",
    "impl DateComparison",
    "pub enum DimensionOutcome",
    "impl DimensionOutcome",
    "pub struct DimensionFinding",
    "impl DimensionFinding",
    "pub struct ContendingSource",
    "impl ContendingSource",
    "pub struct UserResolution",
    "impl UserResolution",
    "pub enum Resolution",
    "pub enum AuditDisposition",
    "impl AuditDisposition",
    "pub struct ConflictCase",
    "impl ConflictCase",
    "pub trait HasDate",
    "impl HasDate for IssuanceDate",
    "impl HasDate for crate::dating::EffectiveDate",
    "pub fn detect(left: ContendingSource, right: ContendingSource) -> Option<ConflictCase>",
    "fn compare_optional_dates<T: HasDate>(left: Option<T>, right: Option<T>) -> DateComparison",
];

/// The whole set of signatures anywhere in this crate that touch a conflict
/// value.
///
/// This is the half that refuses a winner written somewhere other than
/// `conflict.rs`. Every one of these returns a finding, a relation, a
/// disposition, or a case — none of them returns a source, a connector, a
/// target, or a claim, which is what "there is no winner" means.
const CONFLICT_SIGNATURES: [&str; 27] = [
    "crates/ingestion/src/conflict.rs: [-] fn detect(left: ContendingSource, right: ContendingSource) -> Option<ConflictCase>",
    "crates/ingestion/src/conflict.rs: [ConflictCase] fn disposition(&ConflictCase) -> AuditDisposition",
    "crates/ingestion/src/conflict.rs: [ConflictCase] fn finding(&ConflictCase, dimension: ConflictDimension) -> Option<&DimensionFinding>",
    "crates/ingestion/src/conflict.rs: [ConflictCase] fn findings(&ConflictCase) -> &[DimensionFinding]",
    "crates/ingestion/src/conflict.rs: [ConflictCase] fn left(&ConflictCase) -> &ContendingSource",
    "crates/ingestion/src/conflict.rs: [ConflictCase] fn open(left: ContendingSource, right: ContendingSource) -> ConflictCase",
    "crates/ingestion/src/conflict.rs: [ConflictCase] fn resolution(&ConflictCase) -> &Resolution",
    "crates/ingestion/src/conflict.rs: [ConflictCase] fn resolve(&mut ConflictCase, resolution: UserResolution)",
    "crates/ingestion/src/conflict.rs: [ConflictCase] fn right(&ConflictCase) -> &ContendingSource",
    "crates/ingestion/src/conflict.rs: [ConflictDimension] fn as_str(ConflictDimension) -> &'static str",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn authority(&ContendingSource) -> LegalAuthority",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn connector(&ContendingSource) -> &ConnectorId",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn dating(&ContendingSource) -> Dating",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn from_document( connector: ConnectorId, target: DeclaredTarget, document: &OfficialDocument, rule: &RuleId, ) -> Option<ContendingSource>",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn issued(&ContendingSource) -> Option<IssuanceDate>",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn rule(&ContendingSource) -> &RuleId",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn scope(&ContendingSource) -> &TargetScope",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn target(&ContendingSource) -> DeclaredTarget",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn text_digest(&ContendingSource) -> &ContentDigest",
    "crates/ingestion/src/conflict.rs: [ContendingSource] fn transitional_measures(&ContendingSource) -> TransitionalMeasures",
    "crates/ingestion/src/conflict.rs: [DimensionFinding] fn dimension(&DimensionFinding) -> ConflictDimension",
    "crates/ingestion/src/conflict.rs: [DimensionFinding] fn outcome(&DimensionFinding) -> DimensionOutcome",
    "crates/ingestion/src/conflict.rs: [DimensionOutcome] fn as_str(DimensionOutcome) -> &'static str",
    "crates/ingestion/src/publish.rs: [ReviewQueued] fn conflicts(&ReviewQueued) -> &[ConflictCase]",
    "crates/ingestion/src/publish.rs: [ReviewQueued] fn new( connector: ConnectorId, reason: QueueReason, rules: Vec<RuleId>, conflicts: Vec<ConflictCase>, ) -> ReviewQueued",
    "crates/ingestion/src/stage.rs: [Corpus] fn with_contender(mut Corpus, contender: ContendingSource) -> Corpus",
    "crates/ingestion/src/stage.rs: [Reconciled] fn conflicts(&Reconciled) -> &[ConflictCase]",
];

/// The whole public surface of `ConflictCase`.
///
/// Eight. Two name the contending documents, two read the findings, one opens a
/// case, one records a person's decision, one reads it back, and one reports the
/// disposition. There is no ninth that reduces the findings to a source, and the
/// signature sweep above cannot see these because each spells `Self` or a borrow
/// of it.
const CONFLICT_CASE_SURFACE: [&str; 8] = [
    "fn disposition(&self) -> AuditDisposition",
    "fn finding(&self, dimension: ConflictDimension) -> Option<&DimensionFinding>",
    "fn findings(&self) -> &[DimensionFinding]",
    "fn left(&self) -> &ContendingSource",
    "fn open(left: ContendingSource, right: ContendingSource) -> Self",
    "fn resolution(&self) -> &Resolution",
    "fn resolve(&mut self, resolution: UserResolution)",
    "fn right(&self) -> &ContendingSource",
];

/// The whole public surface of `ContendingSource`.
const CONTENDING_SOURCE_SURFACE: [&str; 10] = [
    "fn authority(&self) -> LegalAuthority",
    "fn connector(&self) -> &ConnectorId",
    "fn dating(&self) -> Dating",
    "fn from_document( connector: ConnectorId, target: DeclaredTarget, document: &OfficialDocument, rule: &RuleId, ) -> Option<Self>",
    "fn issued(&self) -> Option<IssuanceDate>",
    "fn rule(&self) -> &RuleId",
    "fn scope(&self) -> &TargetScope",
    "fn target(&self) -> DeclaredTarget",
    "fn text_digest(&self) -> &ContentDigest",
    "fn transitional_measures(&self) -> TransitionalMeasures",
];

/// The whole set of `impl` block headers naming `SourceCategory`.
///
/// Section 8.4's six collection targets are a numbered list in the
/// specification and the sentence beside them says the number decides nothing.
/// One inherent block, and no `Ord`, no `PartialOrd`, no `From<…> for usize`.
const SOURCE_CATEGORY_IMPLS: [&str; 1] = ["impl SourceCategory"];

/// The derive list on `SourceCategory`, whole.
const SOURCE_CATEGORY_DERIVE: &str = "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]";

/// No path in this crate picks a winner by position, count, or score.
#[test]
fn no_numeric_source_winner() -> TestResult {
    let conflict = crate_root().join("src/conflict.rs");
    let code = code_of(&conflict)?;

    // Half one. The module's whole item set is pinned, so a function added
    // there fails whatever it is called.
    let mut items: Vec<String> = code
        .lines()
        // File scope only: a line that starts at column zero. An accessor
        // inside an `impl` is indented, and the `impl` header is not, so this
        // reads the module's shape rather than every method in it.
        .filter(|line| {
            !line.is_empty() && !line.starts_with(char::is_whitespace) && line.contains(' ')
        })
        .filter(|line| {
            ["pub ", "impl ", "enum ", "struct ", "trait ", "const ", "static ", "fn "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .map(|line| line.split('{').next().unwrap_or(line).trim().to_owned())
        .collect();
    items.sort();
    let mut expected: Vec<String> = CONFLICT_ITEMS
        .iter()
        .map(|item| (*item).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        items, expected,
        "the item set of conflict.rs changed; a winner is an item that is not in this list"
    );

    // Half two. The module reaches no number, under any spelling.
    let findings = numeric_findings(&code);
    assert!(
        findings.is_empty(),
        "conflict.rs reaches a number: {findings:?}"
    );

    // The check is not vacuous. Each evasion it exists to refuse is run
    // through it here, and each must be caught. None of the first three names
    // an operation on the vocabulary list.
    let evasions = [
        (
            "a count compared against a count",
            "if left.findings.len() > right.findings.len() { Side::Left } else { Side::Right }",
        ),
        (
            "a score accumulated in an integer",
            "let mut score: u32 = 0; for finding in findings { score += 1; }",
        ),
        (
            "a rank read out of a list position",
            "ConflictDimension::ALL.iter().position(|d| *d == dimension)",
        ),
        ("a bare numeric literal", "let weight = 3; weight"),
        (
            "an ordering derived on the fly",
            "findings.sort_by_key(|finding| finding.dimension())",
        ),
    ];
    for (label, sample) in evasions {
        assert!(
            !numeric_findings(sample).is_empty(),
            "the scan does not catch {label}"
        );
    }
    // And it does not fire on the shapes the module really uses.
    for benign in [
        "match self { Self::Left => Side::Left, Self::Right => Side::Right }",
        "findings.iter().find(|finding| finding.dimension() == dimension)",
        "SUPERIOR_PAIRS.iter().any(|(superior, inferior)| *superior == self)",
        "let mut findings = Vec::new(); findings.push(finding);",
    ] {
        assert!(
            numeric_findings(benign).is_empty(),
            "the scan fires on a shape the module uses: {benign}"
        );
    }
    // The stripper is what makes the literal rule usable.
    assert!(numeric_findings(&strip_non_code("// section 8.4 lists six\n")).is_empty());
    assert!(numeric_findings(&strip_non_code("let s = \"GATE-38-020\";")).is_empty());
    assert!(!numeric_findings(&strip_non_code("let rank = 6;")).is_empty());

    // Half three. Every signature in the crate that touches a conflict value.
    // A winner written in another module fails here as an extra key.
    let touching = signatures_naming(&[
        "ConflictCase",
        "ContendingSource",
        "DimensionFinding",
        "DimensionOutcome",
        "ConflictDimension",
    ])?;
    let expected: BTreeSet<String> = CONFLICT_SIGNATURES
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    assert_eq!(
        touching, expected,
        "a function that takes a conflict value appeared, changed, or vanished"
    );
    // And the two types' own blocks, because a method spelling `Self` is
    // invisible to the sweep above.
    let conflict_source = fs::read_to_string(&conflict)?;
    assert_eq!(
        public_surface(&conflict_source, "impl ConflictCase")?,
        CONFLICT_CASE_SURFACE.to_vec(),
        "ConflictCase gained, lost or changed a public method; a winner is one of these"
    );
    assert_eq!(
        public_surface(&conflict_source, "impl ContendingSource")?,
        CONTENDING_SOURCE_SURFACE.to_vec(),
        "ContendingSource gained, lost or changed a public method"
    );

    // Half four. `SourceCategory` has one inherent block and derives no order,
    // so section 8.4's numbering cannot be read back off it.
    let manifest_source = fs::read_to_string(crate_root().join("src/manifest.rs"))?;
    let manifest_code = strip_non_code(&manifest_source);
    let mut category_impls: Vec<String> = manifest_code
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("impl ") && line.contains("SourceCategory"))
        .map(|line| line.trim_end_matches(" {").trim().to_owned())
        .collect();
    category_impls.sort();
    assert_eq!(
        category_impls,
        SOURCE_CATEGORY_IMPLS.to_vec(),
        "the set of impl blocks on SourceCategory changed; an ordering is one of them"
    );
    let derive_at = manifest_source
        .find("pub enum SourceCategory")
        .ok_or("SourceCategory is gone")?;
    let derive_line = manifest_source[..derive_at]
        .lines()
        .next_back()
        .ok_or("SourceCategory has no derive line")?;
    assert_eq!(
        derive_line.trim(),
        SOURCE_CATEGORY_DERIVE,
        "SourceCategory's derive list changed"
    );

    // Half five. The dimension list is a slice, so it declares no length, and
    // the specification's five are what it holds.
    assert!(
        manifest_code.contains("pub const ALL: [Self; 6]"),
        "SourceCategory::ALL stopped being the exhaustive listing"
    );
    assert!(
        code.contains("pub const ALL: &'static [Self]"),
        "ConflictDimension::ALL gained a length"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// credentials_never_reach_a_general_crawler
// ---------------------------------------------------------------------------

/// The whole set of signatures anywhere in this crate that touch a credential.
///
/// Three: the producer, the accessor, and the one request constructor that
/// consumes one. A fourth fails as an extra key however it is named, which is
/// what makes this a statement about the credential rather than about a
/// spelling.
const CREDENTIAL_SIGNATURES: [&str; 4] = [
    "crates/ingestion/src/fetch.rs: [ConditionalRequest] fn credentialed( manifest: &ConnectorManifest, binding: CredentialBinding, target: DeclaredTarget, validators: Validators, ) -> Result<ConditionalRequest, Denial>",
    "crates/ingestion/src/manifest.rs: [ConnectorManifest] fn credential_binding(&ConnectorManifest) -> Option<CredentialBinding>",
    "crates/ingestion/src/manifest.rs: [CredentialBinding] fn connector(&CredentialBinding) -> &ConnectorId",
    "crates/ingestion/src/manifest.rs: [CredentialBinding] fn fmt(&CredentialBinding, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
];

/// The whole set of `impl` block headers naming `CredentialBinding`.
///
/// Two: the inherent block and the hand-written `Debug`. The type derives
/// nothing, which is what makes "a binding cannot be spent twice" a fact rather
/// than a habit — `ConditionalRequest::credentialed` takes it by value, and
/// without `Clone` or `Copy` there is no second one to give a second request.
const CREDENTIAL_IMPLS: [&str; 2] = [
    "impl CredentialBinding",
    "impl fmt::Debug for CredentialBinding",
];

/// The whole public surface of `CredentialBinding`.
///
/// One accessor, which returns the connector the borrow belongs to. The
/// signature sweep above cannot see an inherent method whose signature spells
/// `Self`, so the type's own block is pinned beside it.
const CREDENTIAL_SURFACE: [&str; 1] = ["fn connector(&self) -> &ConnectorId"];

/// The one producer of a credential binding, whole.
const WHOLE_CREDENTIAL_BINDING: &str = "pub fn credential_binding(&self) -> Option<CredentialBinding> { self.authentication .holds_a_credential() .then(|| CredentialBinding { connector: self.connector.clone(), }) }";

/// The one constructor of a fetch target, whole. It takes `&'static str`.
const WHOLE_LOCATOR: &str =
    "pub const fn declared(value: &'static str) -> Self { Self { declared: value } }";

/// A credential is bound to one connector's declared documents, and a fetch
/// target cannot be built from anything read at run time.
#[test]
fn credentials_never_reach_a_general_crawler() -> TestResult {
    // Half one. Every signature in the crate that touches a credential.
    let touching = signatures_naming(&["CredentialBinding"])?;
    let expected: BTreeSet<String> = CREDENTIAL_SIGNATURES
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    assert_eq!(
        touching, expected,
        "a function that takes or returns a credential binding appeared, changed, or vanished"
    );
    let manifest_source = fs::read_to_string(crate_root().join("src/manifest.rs"))?;
    assert_eq!(
        public_surface(&manifest_source, "impl CredentialBinding")?,
        CREDENTIAL_SURFACE.to_vec(),
        "CredentialBinding's public surface changed"
    );
    // The binding is moved into the one constructor that takes it, and there is
    // no `Clone` and no `Copy` to make a second. That is a fact about the impl
    // set, so the impl set is compared whole and the declaration is required to
    // carry no derive at all.
    let manifest_code = strip_non_code(&manifest_source);
    let mut binding_impls: Vec<String> = manifest_code
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("impl ") && line.contains("CredentialBinding"))
        .map(|line| line.trim_end_matches(" {").trim().to_owned())
        .collect();
    binding_impls.sort();
    assert_eq!(
        binding_impls,
        CREDENTIAL_IMPLS.to_vec(),
        "the set of impl blocks on CredentialBinding changed; Clone is one of them"
    );
    let declaration_at = manifest_source
        .find("pub struct CredentialBinding")
        .ok_or("CredentialBinding is gone")?;
    let preceding = manifest_source[..declaration_at]
        .lines()
        .next_back()
        .ok_or("CredentialBinding has no preceding line")?;
    assert!(
        !preceding.trim_start().starts_with("#["),
        "CredentialBinding gained an attribute; a derived Clone is how it is spent twice"
    );

    // Half two. The producer, pinned whole. A binding minted for an
    // authentication method that holds no credential is an edit to this text.
    let manifest = fs::read_to_string(crate_root().join("src/manifest.rs"))?;
    assert_eq!(
        declared_item(&manifest, "pub fn credential_binding", "\n    }")?,
        WHOLE_CREDENTIAL_BINDING,
        "the credential producer changed; the pin must change with it in the same commit"
    );

    // Half three. The consumer's call site count. One constructor takes a
    // binding, and it is called from nowhere in this crate: a caller supplies
    // one, and the tests are what exercise it.
    let mut consumers = 0_usize;
    for path in product_sources()? {
        consumers += identifier_uses(&code_of(&path)?, "credentialed");
    }
    assert_eq!(
        consumers, 1,
        "the credentialed request constructor is named {consumers} times in product source; \
         one is its declaration and there should be no other"
    );

    // Half four. A fetch target is `&'static`, pinned whole. Bytes that arrive
    // at run time are owned, and `Untrusted<IngestedDocument>` hands out
    // neither a `String` nor a `&str` outside `academic-untrusted-content`, so
    // a link inside a fetched page is a value no target can be built from.
    assert_eq!(
        declared_item(&manifest, "pub const fn declared", "\n    }")?,
        WHOLE_LOCATOR,
        "the target constructor changed; a run-time target is what this pin refuses"
    );
    let mut target_constructions = 0_usize;
    for path in product_sources()? {
        target_constructions += identifier_uses(&code_of(&path)?, "declared");
    }
    assert_eq!(
        target_constructions, 5,
        "`declared` is named {target_constructions} times in product source: the field, the \
         constructor, the field initialiser inside it, and the two readers. A sixth is a second \
         way to build or read a target"
    );

    // Half five. This crate has no transport of its own, so a credential it
    // binds cannot be presented by anything here. The trait is the caller's.
    let fetch = code_of(&crate_root().join("src/fetch.rs"))?;
    assert!(
        fetch.contains("pub trait ConditionalFetch"),
        "the transport stopped being the caller's"
    );
    let implementations: Vec<String> = product_sources()?
        .iter()
        .filter_map(|path| code_of(path).ok().map(|code| (path.clone(), code)))
        .flat_map(|(path, code)| {
            code.lines()
                .map(str::trim)
                .filter(|line| line.starts_with("impl") && line.contains("ConditionalFetch"))
                .map(|line| format!("{}: {line}", relative(&path)))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        implementations,
        Vec::<String>::new(),
        "this crate implements the transport it exists not to have"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// no_captcha_or_access_control_bypass_module_exists
// ---------------------------------------------------------------------------

/// The whole set of signatures anywhere in this crate that produce a request.
///
/// A bypass of an access control is a function from a challenge to an answer —
/// that is, from a response to a request. Every entry here takes a manifest, a
/// `&'static` target and a set of validators, and nothing else.
const REQUEST_SIGNATURES: [&str; 7] = [
    "crates/ingestion/src/fetch.rs: [-] fn fetch(&self, request: &ConditionalRequest) -> Result<FetchOutcome, String>",
    "crates/ingestion/src/fetch.rs: [ConditionalRequest] fn anonymous( manifest: &ConnectorManifest, target: DeclaredTarget, validators: Validators, ) -> Result<ConditionalRequest, Denial>",
    "crates/ingestion/src/fetch.rs: [ConditionalRequest] fn connector(&ConditionalRequest) -> &ConnectorId",
    "crates/ingestion/src/fetch.rs: [ConditionalRequest] fn credentialed( manifest: &ConnectorManifest, binding: CredentialBinding, target: DeclaredTarget, validators: Validators, ) -> Result<ConditionalRequest, Denial>",
    "crates/ingestion/src/fetch.rs: [ConditionalRequest] fn presents_a_credential(&ConditionalRequest) -> bool",
    "crates/ingestion/src/fetch.rs: [ConditionalRequest] fn target(&ConditionalRequest) -> DeclaredTarget",
    "crates/ingestion/src/fetch.rs: [ConditionalRequest] fn validators(&ConditionalRequest) -> &Validators",
];

/// The whole public surface of `ConditionalRequest`.
///
/// Two constructors and four accessors. Both constructors take a manifest, a
/// `&'static` target and a set of validators; neither takes a response, a
/// body, or a snapshot. A bypass of an access control is a function from a
/// challenge to an answer, and there is no signature here that could be one.
const REQUEST_SURFACE: [&str; 6] = [
    "fn anonymous( manifest: &ConnectorManifest, target: DeclaredTarget, validators: Validators, ) -> Result<Self, Denial>",
    "fn connector(&self) -> &ConnectorId",
    "fn credentialed( manifest: &ConnectorManifest, binding: CredentialBinding, target: DeclaredTarget, validators: Validators, ) -> Result<Self, Denial>",
    "fn presents_a_credential(&self) -> bool",
    "fn target(&self) -> DeclaredTarget",
    "fn validators(&self) -> &Validators",
];

/// The one denial constructor, whole.
const WHOLE_DENY: &str = "pub fn deny(connector: ConnectorId, reason: DenialReason) -> Denial { Denial { connector, reason, route: DenialRoute::ManualOrStop, fallbacks: Fallback::ALL, connector_disabled: matches!( reason, DenialReason::TermsRefuse | DenialReason::TermsRevoked | DenialReason::TermsUnreviewed ), } }";

/// Every import in this crate that is rooted outside it.
///
/// The link half of this claim, at source level. An image decoder, an audio
/// decoder, a browser driver, or an HTTP client cannot be reached without one
/// of these lines, and a new one fails as an extra key. The manifest half is
/// `this_crate_declares_three_product_edges` below, and the resolved-closure
/// half is `only_egress_crate_has_a_socket` in
/// `tools/phase1-scaffold-policy.test.mjs`.
const EXTERNAL_IMPORTS: [&str; 14] = [
    "crates/ingestion/src/conflict.rs: use academic_domain::{ContentDigest, engines::RuleId};",
    "crates/ingestion/src/dating.rs: use core::fmt;",
    "crates/ingestion/src/diff.rs: use academic_domain::{ContentDigest, engines::RuleId};",
    "crates/ingestion/src/document.rs: use academic_domain::{ContentDigest, engines::RuleId};",
    "crates/ingestion/src/fetch.rs: use academic_domain::ContentDigest;",
    "crates/ingestion/src/graph.rs: use academic_domain::engines::RuleId;",
    "crates/ingestion/src/identifier.rs: use core::fmt;",
    "crates/ingestion/src/manifest.rs: use core::fmt;",
    "crates/ingestion/src/publish.rs: use academic_domain::engines::RuleId;",
    "crates/ingestion/src/snapshot.rs: use academic_domain::ContentDigest;",
    "crates/ingestion/src/snapshot.rs: use academic_untrusted_content::{ IngestError, IngestedDocument, SourceId, SourceKind, Untrusted, ingest, };",
    "crates/ingestion/src/snapshot.rs: use core::fmt;",
    "crates/ingestion/src/stage.rs: use academic_domain::engines::RuleId;",
    "crates/ingestion/src/stage.rs: use academic_untrusted_content::{IngestError, IngestedDocument, SourceId, SourceKind, Untrusted};",
];

/// The vocabularies that decide how a source may be reached, each whole.
///
/// Every variant of each is something the publisher permits or the user does.
/// A variant meaning "obtained some other way" fails as an extra key.
const ACCESS_VOCABULARIES: [(&str, &str, &[&str]); 4] = [
    (
        "src/manifest.rs",
        "pub enum AuthenticationMethod",
        &[
            "PublicNoCredential",
            "ScopedOfficialApiToken",
            "UserSuppliedExport",
        ],
    ),
    (
        "src/terms.rs",
        "pub enum TermsStatus",
        &[
            "PermittedForDeclaredMethod",
            "Unreviewed",
            "Refused",
            "Revoked",
        ],
    ),
    (
        "src/terms.rs",
        "pub enum Fallback",
        &[
            "ManualPaste",
            "UserProvidedExport",
            "SaveFromYourOwnBrowser",
            "LowFrequencyManualSync",
        ],
    ),
    ("src/terms.rs", "pub enum DenialRoute", &["ManualOrStop"]),
];

/// Nothing here answers a challenge, and nothing anywhere is built on the
/// types that would let it.
#[test]
fn no_captcha_or_access_control_bypass_module_exists() -> TestResult {
    // Half one. Every signature in the crate that produces or consumes a
    // request. None takes a response, a body, or a snapshot.
    let touching = signatures_naming(&["ConditionalRequest"])?;
    let expected: BTreeSet<String> = REQUEST_SIGNATURES
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    assert_eq!(
        touching, expected,
        "a function that produces or consumes a request appeared, changed, or vanished"
    );
    let fetch_source = fs::read_to_string(crate_root().join("src/fetch.rs"))?;
    let surface = public_surface(&fetch_source, "impl ConditionalRequest")?;
    assert_eq!(
        surface,
        REQUEST_SURFACE.to_vec(),
        "ConditionalRequest's public surface changed; a constructor is one of these"
    );
    for signature in surface.iter().chain(touching.iter()) {
        for response_type in ["FetchOutcome", "RawSnapshot", "Untrusted", "HttpMetadata"] {
            // The transport trait names `FetchOutcome` as what it returns; that
            // is the response leaving, not a request being derived from one.
            if signature.contains("fn fetch(") && response_type == "FetchOutcome" {
                continue;
            }
            assert_eq!(
                identifier_uses(signature, response_type),
                0,
                "a request is derived from a response: {signature}"
            );
        }
    }

    // Half two. The access vocabularies, each compared whole.
    for (file, header, variants) in ACCESS_VOCABULARIES {
        let source = fs::read_to_string(crate_root().join(file))?;
        let block = declared_item(&source, header, "\n}")?;
        let found: Vec<String> = block
            .split(' ')
            .filter_map(|token| token.strip_suffix(','))
            .filter(|token| {
                token
                    .chars()
                    .next()
                    .is_some_and(|first| first.is_ascii_uppercase())
            })
            .map(str::to_owned)
            .collect();
        assert_eq!(
            found,
            variants
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>(),
            "{header} gained, lost, or reordered a variant"
        );
    }

    // Half three. The one denial constructor, pinned whole, and counted. A
    // denial that routed to a retry, a second credential, or another way in
    // would be an edit to this text; a denial built elsewhere would be a
    // different count.
    let terms = fs::read_to_string(crate_root().join("src/terms.rs"))?;
    assert_eq!(
        declared_item(&terms, "pub fn deny(", "\n}")?,
        WHOLE_DENY,
        "the denial constructor changed; the pin must change with it in the same commit"
    );
    // The initialiser is counted by the two fields no other expression sets,
    // rather than by the spelling `Denial {` -- which the struct declaration,
    // its `impl` header and the constructor's return type all spell too.
    let mut routes = 0_usize;
    let mut fallback_sets = 0_usize;
    for path in product_sources()? {
        let code = code_of(&path)?;
        routes += code.matches("route: DenialRoute::ManualOrStop").count();
        fallback_sets += code.matches("fallbacks: Fallback::ALL").count();
    }
    assert_eq!(routes, 1, "a Denial routes somewhere other than in `deny`");
    assert_eq!(
        fallback_sets, 1,
        "a Denial's fallback list is set somewhere other than in `deny`"
    );

    // Half four. The whole external import set. A decoder, a driver, or an
    // HTTP client cannot be reached without a line that is not on this list.
    let mut imports = BTreeSet::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        // A `use` runs to its `;`, which is what makes a braced import that
        // wraps over three lines one entry rather than a truncated first line.
        for (at, _) in code.match_indices("use ") {
            let line_start = code[..at].rfind('\n').map_or(0, |index| index + 1);
            if !code[line_start..at].trim().is_empty() {
                continue;
            }
            let Some(end) = code[at..].find(';') else {
                continue;
            };
            let statement = code[at..at + end + 1]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let root = statement
                .trim_start_matches("use ")
                .split([':', '{', ';', ' '])
                .next()
                .unwrap_or_default();
            if matches!(root, "crate" | "self" | "super") {
                continue;
            }
            imports.insert(format!("{}: {statement}", relative(&path)));
        }
    }
    let expected: BTreeSet<String> = EXTERNAL_IMPORTS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    assert_eq!(
        imports, expected,
        "this crate reaches outside itself somewhere new"
    );

    // Half five. The `&'static` target constructor is what stops a challenge
    // from redirecting a fetch, and it is pinned in
    // `credentials_never_reach_a_general_crawler`. Here it is the statement
    // that nothing in the crate builds a target from a `String`.
    for path in product_sources()? {
        let code = code_of(&path)?;
        assert_eq!(
            identifier_uses(&code, "leak"),
            0,
            "{} leaks an allocation, which is how an owned value becomes 'static",
            relative(&path)
        );
    }

    // Half six. The workspace-wide inventory. A module built on this crate's
    // request, target or credential types fails here as a new key, wherever
    // it lives and whatever it is called. Nothing outside this crate names one
    // today.
    let mut naming: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in workspace_sources()? {
        let code = code_of(&path)?;
        let mut found: Vec<String> = ["CredentialBinding", "DeclaredTarget", "ConditionalRequest"]
            .into_iter()
            .filter(|name| identifier_uses(&code, name) > 0)
            .map(str::to_owned)
            .collect();
        found.sort();
        if !found.is_empty() {
            naming.insert(relative(&path), found);
        }
    }
    let outside: BTreeSet<&str> = naming
        .keys()
        .filter(|path| !path.starts_with("crates/ingestion/"))
        .map(String::as_str)
        .collect();
    assert_eq!(
        outside,
        NAMED_OUTSIDE.into_iter().collect::<BTreeSet<_>>(),
        "a file outside academic-ingestion names its request, target or credential types"
    );
    // The half that stays absolute. A *module* built on these types is what
    // this guard is for, and the allowance above admits a caller's test and
    // nothing else: a product file naming one fails here whatever list it is
    // added to, because the condition is the path shape rather than the entry.
    for path in &outside {
        assert!(
            path.contains("/tests/"),
            "{path} is product source outside academic-ingestion and names its target, \
             request or credential types"
        );
    }
    assert!(
        naming.len() >= 5,
        "the workspace walk found only {} files naming these types; it stopped short",
        naming.len()
    );
    Ok(())
}

/// Every file outside this crate that names one of the three types.
///
/// The list is compared as a whole set in both directions, so a new one fails
/// as an extra key and a removed one fails as a missing key, and every entry is
/// separately required to be a test rather than product source.
///
/// `P2-X7`'s acceptance suite drives this crate's stages one to five to build
/// the two official documents its source-change test diffs. It does that
/// because there is no other producer of an `OfficialDocument`, and a locally
/// imitated diff would make `source_change_links_impacted_rules_and_plans`
/// evidence about the imitation rather than about this pipeline. It names
/// `DeclaredTarget` and nothing else of the three: it builds no request and
/// holds no credential.
const NAMED_OUTSIDE: [&str; 1] = ["crates/evidence-center/tests/evidence_center.rs"];

/// Every `.rs` file under `crates/`, recursively.
fn workspace_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&repository_root().join("crates"), &mut found)?;
    found.sort();
    Ok(found)
}

/// This crate's manifest declares three product edges and one dev edge.
#[test]
fn this_crate_declares_three_product_edges() -> TestResult {
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml"))?;
    let section = |name: &str| -> Vec<String> {
        let Some(start) = manifest.find(name) else {
            return Vec::new();
        };
        manifest[start + name.len()..]
            .lines()
            .take_while(|line| !line.trim_start().starts_with('['))
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                line.split(['=', '.', ' '])
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned()
            })
            .collect()
    };
    assert_eq!(
        section("[dependencies]"),
        ["academic-domain", "academic-untrusted-content", "thiserror"],
        "this crate's product edges changed; review the whole new closure for a transport or a decoder"
    );
    assert_eq!(
        section("[dev-dependencies]"),
        ["trybuild"],
        "this crate's dev edges changed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The snapshot's bytes
// ---------------------------------------------------------------------------

/// The whole public surface of `RawSnapshot`.
///
/// Nine accessors and one seal. The seal returns `Untrusted<IngestedDocument>`
/// and nothing else returns the bytes; a tenth public method fails as an extra
/// key, which is what makes this a statement about the type rather than about
/// the accessors somebody thought of.
const SNAPSHOT_PUBLIC_METHODS: [&str; 10] = [
    "fn byte_len(&self) -> usize",
    "fn connector(&self) -> &ConnectorId",
    "fn digest(&self) -> &ContentDigest",
    "fn has_same_content_as(&self, other: &Self) -> bool",
    "fn http(&self) -> &HttpMetadata",
    "fn target(&self) -> DeclaredTarget",
    "fn next_validators(&self) -> Validators",
    "fn parser_version(&self) -> ParserVersion",
    "fn retrieved_at(&self) -> RetrievalInstant",
    "fn seal( &self, source_id: SourceId, kind: SourceKind, ingest_seq: u64, ) -> Result<Untrusted<IngestedDocument>, IngestError>",
];

/// Nothing hands out a snapshot's bytes but the `P2-G5` seal.
#[test]
fn the_only_public_route_to_snapshot_bytes_is_the_untrusted_seal() -> TestResult {
    let source = fs::read_to_string(crate_root().join("src/snapshot.rs"))?;
    let code = strip_non_code(&source);
    let mut expected: Vec<String> = SNAPSHOT_PUBLIC_METHODS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        public_surface(&source, "impl RawSnapshot")?,
        expected,
        "RawSnapshot's public surface changed; a second route to the bytes is one of these"
    );

    // The bytes accessor is crate-private and named once, by the parser.
    assert!(
        code.contains("pub(crate) fn source_bytes(&self) -> &[u8]"),
        "the crate-private byte accessor changed shape"
    );
    let mut call_sites = Vec::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        for _ in 0..code.matches("source_bytes()").count() {
            call_sites.push(relative(&path));
        }
    }
    assert_eq!(
        call_sites,
        ["crates/ingestion/src/document.rs"],
        "the crate-private byte accessor is called from somewhere other than the parser"
    );

    // And no public signature anywhere in this crate returns bytes or text.
    // The workspace-wide form of this rule is `P2-G5`'s
    // `no_public_signature_hands_out_ingested_text`, which reads signatures
    // naming `Untrusted<...>`; this is the same rule over the crate that holds
    // the bytes before they are sealed, and it reads *every* public signature
    // rather than the ones that spell a snapshot.
    //
    // Two shapes are allowed, and both are arguments rather than exemptions.
    //
    // A return of `&'static str` is a compiled constant. Ingested bytes arrive
    // at run time and are owned, so a `&'static str` cannot be one -- the same
    // argument `P2-G5` makes for `SystemDirective::new`'s parameter, read in
    // the other direction. It is recognised structurally, not by name.
    //
    // A return of `&str` from an `as_str` accessor is this crate's own
    // restricted name: `[A-Za-z0-9._-]` for an identifier, printable ASCII for
    // a header value, both bounded in length. That one is a named exception,
    // because nothing structural distinguishes it.
    const RESTRICTED_NAME_ACCESSOR: &str = "fn as_str(&self) -> &str";
    for path in product_sources()? {
        let code = code_of(&path)?;
        for (public, signature) in declarations(&code) {
            if !public || signature == RESTRICTED_NAME_ACCESSOR {
                continue;
            }
            let Some((_, returns)) = signature.split_once("->") else {
                continue;
            };
            let collapsed: String = returns.chars().filter(|c| !c.is_whitespace()).collect();
            if collapsed == "&'staticstr" {
                continue;
            }
            for text_type in ["str", "String"] {
                assert_eq!(
                    identifier_uses(returns, text_type),
                    0,
                    "{}: a public signature hands out text: {signature}",
                    relative(&path)
                );
            }
            // `u8` only inside a buffer. A scalar `u8` is a month, a version or
            // a status code; `[u8]`, `&[u8]` and `Vec<u8>` are the document.
            assert!(
                !collapsed.contains("[u8") && !collapsed.contains("Vec<u8"),
                "{}: a public signature hands out bytes: {signature}",
                relative(&path)
            );
        }
    }
    Ok(())
}

/// The whole public surface of `OfficialDocument`.
///
/// Metadata and rules. Nothing returns text, which is what makes the diff, the
/// invalidation and the conflict case carry identifiers and digests instead of
/// document bytes.
const DOCUMENT_SURFACE: [&str; 8] = [
    "fn authority(&self) -> LegalAuthority",
    "fn dating(&self) -> Dating",
    "fn issued(&self) -> Option<IssuanceDate>",
    "fn parser_version(&self) -> ParserVersion",
    "fn rule(&self, id: &RuleId) -> Option<&ParsedRule>",
    "fn rules(&self) -> &[ParsedRule]",
    "fn scope(&self) -> &TargetScope",
    "fn transitional_measures(&self) -> TransitionalMeasures",
];

/// The whole public surface of `ParsedRule`. An identifier, a section, a digest.
const PARSED_RULE_SURFACE: [&str; 3] = [
    "fn id(&self) -> &RuleId",
    "fn section(&self) -> &SectionPath",
    "fn text_digest(&self) -> &ContentDigest",
];

/// The parse hands out metadata, and no rule text.
#[test]
fn no_document_text_leaves_the_parser() -> TestResult {
    let source = fs::read_to_string(crate_root().join("src/document.rs"))?;
    assert_eq!(
        public_surface(&source, "impl OfficialDocument")?,
        DOCUMENT_SURFACE.to_vec(),
        "OfficialDocument's public surface changed; a text accessor is one of these"
    );
    assert_eq!(
        public_surface(&source, "impl ParsedRule")?,
        PARSED_RULE_SURFACE.to_vec(),
        "ParsedRule's public surface changed; the textual half is a digest, not the text"
    );
    Ok(())
}

/// The whole set of signatures naming the publishable value.
///
/// Two: the producer, and the one function that consumes it. The constructor
/// they both go through is `pub(crate)` and spells `Self`, so it is invisible to
/// this sweep and is covered instead by
/// `tests/compile_fail/publishable_rules_cannot_be_assembled.rs`. A second
/// entry point into publication fails here as an extra key.
const PUBLISHABLE_SIGNATURES: [&str; 5] = [
    "crates/ingestion/src/publish.rs: [-] fn publish(publishable: PublishableRules<'_>) -> PublishedRules",
    "crates/ingestion/src/publish.rs: [PublishableRules] fn effective(&PublishableRules) -> EffectiveDate",
    "crates/ingestion/src/publish.rs: [PublishableRules] fn new( document: &'run OfficialDocument, connector: &'run ConnectorId, effective: EffectiveDate, retrieved_at: RetrievalInstant, ) -> PublishableRules",
    "crates/ingestion/src/publish.rs: [PublishableRules] fn scope(&PublishableRules) -> &TargetScope",
    "crates/ingestion/src/stage.rs: [Reconciled] fn publishable(&Reconciled) -> Option<PublishableRules<'_>>",
];

/// The one assembly of the publishable value, whole.
const WHOLE_PUBLISHABLE_NEW: &str = "pub(crate) const fn new( document: &'run OfficialDocument, connector: &'run ConnectorId, effective: EffectiveDate, retrieved_at: RetrievalInstant, ) -> Self { Self { document, connector, effective, retrieved_at, } }";

/// Publication has one argument type, one producer, and one consumer.
///
/// The signature sweep alone was empty against the shape it exists to refuse.
/// A second public entry point — `publish_anyway(document, connector, effective,
/// retrieved_at) -> PublishedRules`, which builds the argument in its body —
/// names `PublishableRules` nowhere in its signature and passed. That is the
/// "the pin fixes the item and not its caller" shape
/// `docs/contracts/policy-source-scans.md` records, one layer in: the sweep
/// fixes the *signatures*, and what makes publication reachable is a
/// *construction*. So the constructor is counted beside it.
#[test]
fn the_publisher_has_one_argument_type_and_one_producer() -> TestResult {
    let touching = signatures_naming(&["PublishableRules"])?;
    let expected: BTreeSet<String> = PUBLISHABLE_SIGNATURES
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    assert_eq!(
        touching, expected,
        "a second way into publication appeared, or the one that exists changed"
    );

    // The construction sites, which is what a second entry point really needs.
    // `Reconciled::publishable` is the one caller of the constructor and it is
    // the function that returns `None` for `Dating::Unscoped`; anything routed
    // through it inherits that refusal, and anything not routed through it has
    // to build the value itself.
    let mut constructor_calls = Vec::new();
    let mut named_literals = Vec::new();
    for path in product_sources()? {
        let code = code_of(&path)?;
        for _ in 0..code.matches("PublishableRules::new(").count() {
            constructor_calls.push(relative(&path));
        }
        for _ in 0..code.matches("PublishableRules {").count() {
            named_literals.push(relative(&path));
        }
    }
    assert_eq!(
        constructor_calls,
        ["crates/ingestion/src/stage.rs"],
        "the publishable value is built somewhere other than Reconciled::publishable"
    );
    // The type's fields are private, so it can only be assembled inside its own
    // module -- `tests/compile_fail/publishable_rules_cannot_be_assembled.rs`
    // observes that from outside. Inside, the assembly is written as `Self {`
    // in exactly one place, the constructor, and a named literal anywhere is a
    // second assembly whatever module it sits in.
    assert_eq!(
        named_literals,
        Vec::<String>::new(),
        "the publishable value is assembled by name instead of through its constructor"
    );
    let publish_source = fs::read_to_string(crate_root().join("src/publish.rs"))?;
    assert_eq!(
        declared_item(
            &publish_source,
            "pub(crate) const fn new",
            "
    }"
        )?,
        WHOLE_PUBLISHABLE_NEW,
        "the one assembly of the publishable value changed; the pin must change with it"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The whole-set inventory
// ---------------------------------------------------------------------------

/// Every `impl` block header this crate ships, whole.
///
/// The header runs from `impl` to the opening brace, so
/// `impl From<&FetchOutcome> for ConditionalRequest` and `impl
/// ConditionalRequest` are different entries and a trait implementation cannot
/// arrive as an edit to an inherent one.
fn impl_headers(code: &str) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("impl") {
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        if code[at + 4..]
            .starts_with(|character: char| character.is_alphanumeric() || character == '_')
        {
            continue;
        }
        let Some(end) = code[at..].find(['{', ';']) else {
            continue;
        };
        found.push(
            code[at..at + end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    found
}

/// The floor under the inventory walk.
const INVENTORY_FILE_FLOOR: usize = 14;

/// Nothing this crate declares is outside the two pinned sets.
///
/// `P2-A3` recorded that `ingestion_scans.rs` held eight tests and none of them
/// was a whole-declaration or `impl` inventory, and that the repair which gave
/// six other crates one skipped this crate on the ground that it already had
/// a set comparison. It did not: `signatures_naming` is a set of the signatures
/// whose **text** names a type, which the idiomatic `Self` walks past. That
/// half is now owner-aware; this is the half that does not depend on naming a
/// type at all.
///
/// Two whole sets, each compared in both directions:
///
/// 1. every function declaration this package ships, as a file, a visibility
///    and a full signature;
/// 2. every `impl` block header this package ships, as a file and a header.
///
/// A new function, a new method, a new inherent `impl` and a new trait `impl`
/// each fail as an entry nobody wrote down, whatever they are called. That is
/// what makes the sentence in `src/fetch.rs` true: a challenge-response loop
/// fails as an extra key rather than as a missing token.
#[test]
fn every_declaration_and_impl_in_this_crate_is_pinned() -> TestResult {
    let sources = product_sources()?;
    assert!(
        sources.len() >= INVENTORY_FILE_FLOOR,
        "the inventory walk read only {} files",
        sources.len()
    );

    let mut declared = Vec::new();
    let mut headers = Vec::new();
    for path in &sources {
        let name = relative(path);
        let code = code_of(path)?;
        for (public, signature) in declarations(&code) {
            let visibility = if public { "pub" } else { "priv" };
            declared.push(format!("{name} [{visibility}] {signature}"));
        }
        for header in impl_headers(&code) {
            headers.push(format!("{name}: {header}"));
        }
    }
    declared.sort();
    headers.sort();

    assert_eq!(
        declared,
        DECLARATIONS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's declaration set changed"
    );
    assert_eq!(
        headers,
        IMPL_HEADERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's impl inventory changed"
    );
    Ok(())
}

/// Every function this package declares, sorted.
const DECLARATIONS: [&str; 261] = [
    "crates/ingestion/src/conflict.rs [priv] fn compare_optional_dates<T: HasDate>(left: Option<T>, right: Option<T>) -> DateComparison",
    "crates/ingestion/src/conflict.rs [priv] fn date(&self) -> crate::dating::Date",
    "crates/ingestion/src/conflict.rs [priv] fn date(&self) -> crate::dating::Date",
    "crates/ingestion/src/conflict.rs [priv] fn date(&self) -> crate::dating::Date",
    "crates/ingestion/src/conflict.rs [pub] fn actor(&self) -> &DependentId",
    "crates/ingestion/src/conflict.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/conflict.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/conflict.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/conflict.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/conflict.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/conflict.rs [pub] fn authority(&self) -> LegalAuthority",
    "crates/ingestion/src/conflict.rs [pub] fn chose(&self) -> Side",
    "crates/ingestion/src/conflict.rs [pub] fn connector(&self) -> &ConnectorId",
    "crates/ingestion/src/conflict.rs [pub] fn dating(&self) -> Dating",
    "crates/ingestion/src/conflict.rs [pub] fn detect(left: ContendingSource, right: ContendingSource) -> Option<ConflictCase>",
    "crates/ingestion/src/conflict.rs [pub] fn dimension(&self) -> ConflictDimension",
    "crates/ingestion/src/conflict.rs [pub] fn disposition(&self) -> AuditDisposition",
    "crates/ingestion/src/conflict.rs [pub] fn finding(&self, dimension: ConflictDimension) -> Option<&DimensionFinding>",
    "crates/ingestion/src/conflict.rs [pub] fn findings(&self) -> &[DimensionFinding]",
    "crates/ingestion/src/conflict.rs [pub] fn from_document( connector: ConnectorId, target: DeclaredTarget, document: &OfficialDocument, rule: &RuleId, ) -> Option<Self>",
    "crates/ingestion/src/conflict.rs [pub] fn issued(&self) -> Option<IssuanceDate>",
    "crates/ingestion/src/conflict.rs [pub] fn left(&self) -> &ContendingSource",
    "crates/ingestion/src/conflict.rs [pub] fn open(left: ContendingSource, right: ContendingSource) -> Self",
    "crates/ingestion/src/conflict.rs [pub] fn outcome(&self) -> DimensionOutcome",
    "crates/ingestion/src/conflict.rs [pub] fn recorded(chose: Side, actor: DependentId) -> Self",
    "crates/ingestion/src/conflict.rs [pub] fn resolution(&self) -> &Resolution",
    "crates/ingestion/src/conflict.rs [pub] fn resolve(&mut self, resolution: UserResolution)",
    "crates/ingestion/src/conflict.rs [pub] fn right(&self) -> &ContendingSource",
    "crates/ingestion/src/conflict.rs [pub] fn rule(&self) -> &RuleId",
    "crates/ingestion/src/conflict.rs [pub] fn scope(&self) -> &TargetScope",
    "crates/ingestion/src/conflict.rs [pub] fn target(&self) -> DeclaredTarget",
    "crates/ingestion/src/conflict.rs [pub] fn text_digest(&self) -> &ContentDigest",
    "crates/ingestion/src/conflict.rs [pub] fn transitional_measures(&self) -> TransitionalMeasures",
    "crates/ingestion/src/dating.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/ingestion/src/dating.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/ingestion/src/dating.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/ingestion/src/dating.rs [priv] fn is_leap_year(year: u16) -> bool",
    "crates/ingestion/src/dating.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/dating.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/dating.rs [pub] fn date(self) -> Date",
    "crates/ingestion/src/dating.rs [pub] fn date(self) -> Date",
    "crates/ingestion/src/dating.rs [pub] fn day(self) -> u8",
    "crates/ingestion/src/dating.rs [pub] fn effective_date(self) -> Option<EffectiveDate>",
    "crates/ingestion/src/dating.rs [pub] fn is_publishable(self) -> bool",
    "crates/ingestion/src/dating.rs [pub] fn month(self) -> u8",
    "crates/ingestion/src/dating.rs [pub] fn new(year: u16, month: u8, day: u8) -> Result<Self, DateError>",
    "crates/ingestion/src/dating.rs [pub] fn on(date: Date) -> Self",
    "crates/ingestion/src/dating.rs [pub] fn on(date: Date) -> Self",
    "crates/ingestion/src/dating.rs [pub] fn relation_to(self, other: Self) -> DateRelation",
    "crates/ingestion/src/dating.rs [pub] fn year(self) -> u16",
    "crates/ingestion/src/diff.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/diff.rs [pub] fn between(previous: &OfficialDocument, current: &OfficialDocument) -> Self",
    "crates/ingestion/src/diff.rs [pub] fn document_changes(&self) -> &[DocumentChange]",
    "crates/ingestion/src/diff.rs [pub] fn impacted_rules(&self) -> Vec<RuleId>",
    "crates/ingestion/src/diff.rs [pub] fn is_empty(&self) -> bool",
    "crates/ingestion/src/diff.rs [pub] fn rule(&self) -> &RuleId",
    "crates/ingestion/src/diff.rs [pub] fn rule_changes(&self) -> &[RuleChange]",
    "crates/ingestion/src/document.rs [priv] fn parse_cohorts(value: &str) -> Option<CohortRange>",
    "crates/ingestion/src/document.rs [priv] fn parse_date(value: &str) -> Option<Date>",
    "crates/ingestion/src/document.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/document.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/document.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/document.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/document.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/document.rs [pub] fn authority(&self) -> LegalAuthority",
    "crates/ingestion/src/document.rs [pub] fn between(first: AdmissionYear, last: AdmissionYear) -> Self",
    "crates/ingestion/src/document.rs [pub] fn cohorts(&self) -> CohortRange",
    "crates/ingestion/src/document.rs [pub] fn covers(self, year: AdmissionYear) -> bool",
    "crates/ingestion/src/document.rs [pub] fn dating(&self) -> Dating",
    "crates/ingestion/src/document.rs [pub] fn every() -> Self",
    "crates/ingestion/src/document.rs [pub] fn from(year: AdmissionYear) -> Self",
    "crates/ingestion/src/document.rs [pub] fn get(self) -> u16",
    "crates/ingestion/src/document.rs [pub] fn hierarchy_relation(self, other: Self) -> HierarchyRelation",
    "crates/ingestion/src/document.rs [pub] fn id(&self) -> &RuleId",
    "crates/ingestion/src/document.rs [pub] fn intersects(self, other: Self) -> bool",
    "crates/ingestion/src/document.rs [pub] fn is_within(self, other: Self) -> bool",
    "crates/ingestion/src/document.rs [pub] fn issued(&self) -> Option<IssuanceDate>",
    "crates/ingestion/src/document.rs [pub] fn new(program: ProgramKey, cohorts: CohortRange) -> Self",
    "crates/ingestion/src/document.rs [pub] fn new(year: u16) -> Self",
    "crates/ingestion/src/document.rs [pub] fn parse(snapshot: &RawSnapshot) -> Result<OfficialDocument, ParseError>",
    "crates/ingestion/src/document.rs [pub] fn parse(value: &str) -> Option<Self>",
    "crates/ingestion/src/document.rs [pub] fn parse(value: &str) -> Option<Self>",
    "crates/ingestion/src/document.rs [pub] fn parser_version(&self) -> ParserVersion",
    "crates/ingestion/src/document.rs [pub] fn program(&self) -> &ProgramKey",
    "crates/ingestion/src/document.rs [pub] fn provides_for_a_transition(self) -> bool",
    "crates/ingestion/src/document.rs [pub] fn relation_to(&self, other: &Self) -> ScopeRelation",
    "crates/ingestion/src/document.rs [pub] fn relation_to(self, other: Self) -> TransitionRelation",
    "crates/ingestion/src/document.rs [pub] fn rule(&self, id: &RuleId) -> Option<&ParsedRule>",
    "crates/ingestion/src/document.rs [pub] fn rules(&self) -> &[ParsedRule]",
    "crates/ingestion/src/document.rs [pub] fn scope(&self) -> &TargetScope",
    "crates/ingestion/src/document.rs [pub] fn section(&self) -> &SectionPath",
    "crates/ingestion/src/document.rs [pub] fn text_digest(&self) -> &ContentDigest",
    "crates/ingestion/src/document.rs [pub] fn transitional_measures(&self) -> TransitionalMeasures",
    "crates/ingestion/src/document.rs [pub] fn validate(document: &OfficialDocument) -> Result<(), SchemaError>",
    "crates/ingestion/src/fetch.rs [priv] fn fetch(&self, request: &ConditionalRequest) -> Result<FetchOutcome, String>",
    "crates/ingestion/src/fetch.rs [priv] fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result",
    "crates/ingestion/src/fetch.rs [pub] fn anonymous( manifest: &ConnectorManifest, target: DeclaredTarget, validators: Validators, ) -> Result<Self, Denial>",
    "crates/ingestion/src/fetch.rs [pub] fn as_str(&self) -> &str",
    "crates/ingestion/src/fetch.rs [pub] fn connector(&self) -> &ConnectorId",
    "crates/ingestion/src/fetch.rs [pub] fn content_type(&self) -> Option<&HeaderValue>",
    "crates/ingestion/src/fetch.rs [pub] fn credentialed( manifest: &ConnectorManifest, binding: CredentialBinding, target: DeclaredTarget, validators: Validators, ) -> Result<Self, Denial>",
    "crates/ingestion/src/fetch.rs [pub] fn entity_tag(&self) -> Option<&HeaderValue>",
    "crates/ingestion/src/fetch.rs [pub] fn entity_tag(&self) -> Option<&HeaderValue>",
    "crates/ingestion/src/fetch.rs [pub] fn is_conditional(&self) -> bool",
    "crates/ingestion/src/fetch.rs [pub] fn last_modified(&self) -> Option<&HeaderValue>",
    "crates/ingestion/src/fetch.rs [pub] fn last_modified(&self) -> Option<&HeaderValue>",
    "crates/ingestion/src/fetch.rs [pub] fn new( status: Option<u16>, entity_tag: Option<HeaderValue>, last_modified: Option<HeaderValue>, content_type: Option<HeaderValue>, ) -> Self",
    "crates/ingestion/src/fetch.rs [pub] fn new(value: impl Into<String>) -> Result<Self, HeaderError>",
    "crates/ingestion/src/fetch.rs [pub] fn next_validators(&self) -> Validators",
    "crates/ingestion/src/fetch.rs [pub] fn none() -> Self",
    "crates/ingestion/src/fetch.rs [pub] fn presents_a_credential(&self) -> bool",
    "crates/ingestion/src/fetch.rs [pub] fn status(&self) -> Option<u16>",
    "crates/ingestion/src/fetch.rs [pub] fn target(&self) -> DeclaredTarget",
    "crates/ingestion/src/fetch.rs [pub] fn validators(&self) -> &Validators",
    "crates/ingestion/src/fetch.rs [pub] fn with_entity_tag(mut self, value: HeaderValue) -> Self",
    "crates/ingestion/src/fetch.rs [pub] fn with_last_modified(mut self, value: HeaderValue) -> Self",
    "crates/ingestion/src/gate.rs [pub] fn identifier(self) -> &'static str",
    "crates/ingestion/src/gate.rs [pub] fn phase2_shipped_fallbacks() -> [Fallback",
    "crates/ingestion/src/gate.rs [pub] fn statement(self) -> &'static str",
    "crates/ingestion/src/gate.rs [pub] fn unreviewed_status() -> TermsStatus",
    "crates/ingestion/src/graph.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/graph.rs [pub] fn edges(&self) -> &[(DependentNode, Dependency)]",
    "crates/ingestion/src/graph.rs [pub] fn id(&self) -> &DependentId",
    "crates/ingestion/src/graph.rs [pub] fn invalidate(&self, impacted: &[RuleId]) -> Invalidation",
    "crates/ingestion/src/graph.rs [pub] fn is_empty(&self) -> bool",
    "crates/ingestion/src/graph.rs [pub] fn kind(&self) -> DependentKind",
    "crates/ingestion/src/graph.rs [pub] fn new() -> Self",
    "crates/ingestion/src/graph.rs [pub] fn new(kind: DependentKind, id: DependentId) -> Self",
    "crates/ingestion/src/graph.rs [pub] fn nodes(&self) -> &[DependentNode]",
    "crates/ingestion/src/graph.rs [pub] fn of_kind(&self, kind: DependentKind) -> Vec<&DependentNode>",
    "crates/ingestion/src/graph.rs [pub] fn record(&mut self, dependent: DependentNode, dependency: Dependency)",
    "crates/ingestion/src/identifier.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/ingestion/src/identifier.rs [priv] fn is_name(value: &str) -> bool",
    "crates/ingestion/src/identifier.rs [pub] fn as_str(&self) -> &str",
    "crates/ingestion/src/identifier.rs [pub] fn new(value: impl Into<String>) -> Result<Self, NameError>",
    "crates/ingestion/src/manifest.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/ingestion/src/manifest.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/ingestion/src/manifest.rs [pub] fn allowed_frequency(&self) -> AllowedFrequency",
    "crates/ingestion/src/manifest.rs [pub] fn allowed_frequency(mut self, value: AllowedFrequency) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/manifest.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/manifest.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/manifest.rs [pub] fn at(seconds_since_epoch: u64) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn authentication_method(&self) -> AuthenticationMethod",
    "crates/ingestion/src/manifest.rs [pub] fn authentication_method(mut self, value: AuthenticationMethod) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn build(self) -> Result<ConnectorManifest, ManifestError>",
    "crates/ingestion/src/manifest.rs [pub] fn category(&self) -> SourceCategory",
    "crates/ingestion/src/manifest.rs [pub] fn completeness(&self) -> Completeness",
    "crates/ingestion/src/manifest.rs [pub] fn completeness(mut self, value: Completeness) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn connector(&self) -> &ConnectorId",
    "crates/ingestion/src/manifest.rs [pub] fn connector(&self) -> &ConnectorId",
    "crates/ingestion/src/manifest.rs [pub] fn connector_id(value: &str) -> Result<ConnectorId, NameError>",
    "crates/ingestion/src/manifest.rs [pub] fn credential_binding(&self) -> Option<CredentialBinding>",
    "crates/ingestion/src/manifest.rs [pub] fn declared(value: &'static str) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn declared_targets(&self) -> &[DeclaredTarget]",
    "crates/ingestion/src/manifest.rs [pub] fn declares(&self, target: DeclaredTarget) -> bool",
    "crates/ingestion/src/manifest.rs [pub] fn declaring(mut self, target: DeclaredTarget) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn due_at(instant: RetrievalInstant) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn earliest_next(self, last: RetrievalInstant) -> Option<RetrievalInstant>",
    "crates/ingestion/src/manifest.rs [pub] fn for_connector(connector: ConnectorId, category: SourceCategory) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn get(self) -> u16",
    "crates/ingestion/src/manifest.rs [pub] fn holds_a_credential(self) -> bool",
    "crates/ingestion/src/manifest.rs [pub] fn instant(self) -> RetrievalInstant",
    "crates/ingestion/src/manifest.rs [pub] fn is_overdue(self, now: RetrievalInstant) -> bool",
    "crates/ingestion/src/manifest.rs [pub] fn last_success(&self) -> LastSuccess",
    "crates/ingestion/src/manifest.rs [pub] fn last_success(mut self, value: LastSuccess) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn new(version: u16) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn next_verification(&self) -> NextVerification",
    "crates/ingestion/src/manifest.rs [pub] fn next_verification(mut self, value: NextVerification) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn parser_version(&self) -> ParserVersion",
    "crates/ingestion/src/manifest.rs [pub] fn parser_version(mut self, value: ParserVersion) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn personal_data_class(&self) -> PersonalDataClass",
    "crates/ingestion/src/manifest.rs [pub] fn personal_data_class(mut self, value: PersonalDataClass) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn seconds(self) -> u64",
    "crates/ingestion/src/manifest.rs [pub] fn source_ownership(&self) -> SourceOwnership",
    "crates/ingestion/src/manifest.rs [pub] fn source_ownership(mut self, value: SourceOwnership) -> Self",
    "crates/ingestion/src/manifest.rs [pub] fn terms_status(&self) -> TermsStatus",
    "crates/ingestion/src/manifest.rs [pub] fn terms_status(mut self, value: TermsStatus) -> Self",
    "crates/ingestion/src/publish.rs [priv] fn new( connector: ConnectorId, reason: QueueReason, rules: Vec<RuleId>, conflicts: Vec<ConflictCase>, ) -> Self",
    "crates/ingestion/src/publish.rs [priv] fn new( document: &'run OfficialDocument, connector: &'run ConnectorId, effective: EffectiveDate, retrieved_at: RetrievalInstant, ) -> Self",
    "crates/ingestion/src/publish.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/publish.rs [pub] fn conflicts(&self) -> &[ConflictCase]",
    "crates/ingestion/src/publish.rs [pub] fn connector(&self) -> &ConnectorId",
    "crates/ingestion/src/publish.rs [pub] fn connector(&self) -> &ConnectorId",
    "crates/ingestion/src/publish.rs [pub] fn effective(&self) -> EffectiveDate",
    "crates/ingestion/src/publish.rs [pub] fn effective(&self) -> EffectiveDate",
    "crates/ingestion/src/publish.rs [pub] fn parser_version(&self) -> ParserVersion",
    "crates/ingestion/src/publish.rs [pub] fn publish(publishable: PublishableRules<'_>) -> PublishedRules",
    "crates/ingestion/src/publish.rs [pub] fn published(&self) -> Option<&PublishedRules>",
    "crates/ingestion/src/publish.rs [pub] fn queued(&self) -> Option<&ReviewQueued>",
    "crates/ingestion/src/publish.rs [pub] fn reason(&self) -> QueueReason",
    "crates/ingestion/src/publish.rs [pub] fn retrieved_at(&self) -> RetrievalInstant",
    "crates/ingestion/src/publish.rs [pub] fn rules(&self) -> &[RuleId]",
    "crates/ingestion/src/publish.rs [pub] fn rules(&self) -> &[RuleId]",
    "crates/ingestion/src/publish.rs [pub] fn scope(&self) -> &TargetScope",
    "crates/ingestion/src/publish.rs [pub] fn scope(&self) -> &TargetScope",
    "crates/ingestion/src/snapshot.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/ingestion/src/snapshot.rs [priv] fn source_bytes(&self) -> &[u8]",
    "crates/ingestion/src/snapshot.rs [pub] fn byte_len(&self) -> usize",
    "crates/ingestion/src/snapshot.rs [pub] fn connector(&self) -> &ConnectorId",
    "crates/ingestion/src/snapshot.rs [pub] fn digest(&self) -> &ContentDigest",
    "crates/ingestion/src/snapshot.rs [pub] fn has_same_content_as(&self, other: &Self) -> bool",
    "crates/ingestion/src/snapshot.rs [pub] fn http(&self) -> &HttpMetadata",
    "crates/ingestion/src/snapshot.rs [pub] fn next_validators(&self) -> Validators",
    "crates/ingestion/src/snapshot.rs [pub] fn parser_version(&self) -> ParserVersion",
    "crates/ingestion/src/snapshot.rs [pub] fn retrieved_at(&self) -> RetrievalInstant",
    "crates/ingestion/src/snapshot.rs [pub] fn seal( &self, source_id: SourceId, kind: SourceKind, ingest_seq: u64, ) -> Result<Untrusted<IngestedDocument>, IngestError>",
    "crates/ingestion/src/snapshot.rs [pub] fn store( connector: ConnectorId, target: DeclaredTarget, parser_version: ParserVersion, outcome: FetchOutcome, ) -> Result<RawSnapshot, SnapshotError>",
    "crates/ingestion/src/snapshot.rs [pub] fn target(&self) -> DeclaredTarget",
    "crates/ingestion/src/stage.rs [priv] fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result",
    "crates/ingestion/src/stage.rs [priv] fn reason_for(status: crate::terms::TermsStatus) -> DenialReason",
    "crates/ingestion/src/stage.rs [pub] fn ai_proposal_where_appropriate( validated: Validated, appropriateness: Appropriateness, ) -> Result<Proposed, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/stage.rs [pub] fn at(seq: u64) -> Self",
    "crates/ingestion/src/stage.rs [pub] fn claim_publication_or_review_queue( reconciled: Reconciled, ledger: &TermsLedger, ) -> Result<Publication, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn conflicts(&self) -> &[ConflictCase]",
    "crates/ingestion/src/stage.rs [pub] fn denial(&self) -> Option<&Denial>",
    "crates/ingestion/src/stage.rs [pub] fn deterministic_parse(described: Described) -> Result<Parsed, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn discover_fetch_import( manifest: &ConnectorManifest, ledger: &TermsLedger, now: RetrievalInstant, acquisition: Acquisition<'_>, ) -> Result<Fetched, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn document(&self) -> &OfficialDocument",
    "crates/ingestion/src/stage.rs [pub] fn document(&self) -> &OfficialDocument",
    "crates/ingestion/src/stage.rs [pub] fn document(&self) -> &OfficialDocument",
    "crates/ingestion/src/stage.rs [pub] fn document(&self) -> &OfficialDocument",
    "crates/ingestion/src/stage.rs [pub] fn failure(&self) -> Option<&StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn get(self) -> u64",
    "crates/ingestion/src/stage.rs [pub] fn immutable_raw_snapshot( cleared: TermsCleared, manifest: &ConnectorManifest, ) -> Result<Snapshotted, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn ingest_seq(&self) -> IngestSeq",
    "crates/ingestion/src/stage.rs [pub] fn into_snapshot(self) -> RawSnapshot",
    "crates/ingestion/src/stage.rs [pub] fn knowing(mut self, program: ProgramKey) -> Self",
    "crates/ingestion/src/stage.rs [pub] fn new() -> Self",
    "crates/ingestion/src/stage.rs [pub] fn outcome(&self) -> &FetchOutcome",
    "crates/ingestion/src/stage.rs [pub] fn outcome(&self) -> &RunOutcome",
    "crates/ingestion/src/stage.rs [pub] fn policy_and_terms_check( fetched: Fetched, manifest: &ConnectorManifest, ledger: &TermsLedger, ) -> Result<TermsCleared, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn publishable(&self) -> Option<PublishableRules<'_>>",
    "crates/ingestion/src/stage.rs [pub] fn published(&self) -> Option<&crate::publish::PublishedRules>",
    "crates/ingestion/src/stage.rs [pub] fn reached(&self) -> &[Stage]",
    "crates/ingestion/src/stage.rs [pub] fn reason(&self) -> &FailureReason",
    "crates/ingestion/src/stage.rs [pub] fn reconciliation_and_entity_resolution( proposed: Proposed, corpus: &Corpus, ) -> Result<Reconciled, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn run( manifest: &ConnectorManifest, ledger: &TermsLedger, corpus: &Corpus, now: RetrievalInstant, acquisition: Acquisition<'_>, ingest_seq: IngestSeq, appropriateness: Appropriateness, ) -> RunRecord",
    "crates/ingestion/src/stage.rs [pub] fn schema_validation(parsed: Parsed) -> Result<Validated, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn sealed(&self) -> Option<&Untrusted<IngestedDocument>>",
    "crates/ingestion/src/stage.rs [pub] fn snapshot(&self) -> &RawSnapshot",
    "crates/ingestion/src/stage.rs [pub] fn snapshot(&self) -> &RawSnapshot",
    "crates/ingestion/src/stage.rs [pub] fn snapshot(&self) -> &RawSnapshot",
    "crates/ingestion/src/stage.rs [pub] fn source_metadata_and_retrieval_time( snapshotted: Snapshotted, manifest: &ConnectorManifest, ingest_seq: IngestSeq, ) -> Result<Described, StageFailure>",
    "crates/ingestion/src/stage.rs [pub] fn spec_line(self) -> &'static str",
    "crates/ingestion/src/stage.rs [pub] fn stage(&self) -> Stage",
    "crates/ingestion/src/stage.rs [pub] fn with_contender(mut self, contender: ContendingSource) -> Self",
    "crates/ingestion/src/terms.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/terms.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/terms.rs [pub] fn as_str(self) -> &'static str",
    "crates/ingestion/src/terms.rs [pub] fn connector(&self) -> &ConnectorId",
    "crates/ingestion/src/terms.rs [pub] fn connector_disabled(&self) -> bool",
    "crates/ingestion/src/terms.rs [pub] fn deny(connector: ConnectorId, reason: DenialReason) -> Denial",
    "crates/ingestion/src/terms.rs [pub] fn fallbacks(&self) -> &[Fallback]",
    "crates/ingestion/src/terms.rs [pub] fn new() -> Self",
    "crates/ingestion/src/terms.rs [pub] fn permits_a_fetch(self) -> bool",
    "crates/ingestion/src/terms.rs [pub] fn reason(&self) -> DenialReason",
    "crates/ingestion/src/terms.rs [pub] fn record(&mut self, connector: ConnectorId, status: TermsStatus)",
    "crates/ingestion/src/terms.rs [pub] fn route(&self) -> DenialRoute",
    "crates/ingestion/src/terms.rs [pub] fn status(&self, connector: &ConnectorId) -> TermsStatus",
];

/// Every `impl` block header this package declares, sorted.
const IMPL_HEADERS: [&str; 84] = [
    "crates/ingestion/src/conflict.rs: impl AuditDisposition",
    "crates/ingestion/src/conflict.rs: impl ConflictCase",
    "crates/ingestion/src/conflict.rs: impl ConflictDimension",
    "crates/ingestion/src/conflict.rs: impl ContendingSource",
    "crates/ingestion/src/conflict.rs: impl DateComparison",
    "crates/ingestion/src/conflict.rs: impl DimensionFinding",
    "crates/ingestion/src/conflict.rs: impl DimensionOutcome",
    "crates/ingestion/src/conflict.rs: impl HasDate for IssuanceDate",
    "crates/ingestion/src/conflict.rs: impl HasDate for crate::dating::EffectiveDate",
    "crates/ingestion/src/conflict.rs: impl Side",
    "crates/ingestion/src/conflict.rs: impl UserResolution",
    "crates/ingestion/src/dating.rs: impl Date",
    "crates/ingestion/src/dating.rs: impl DateRelation",
    "crates/ingestion/src/dating.rs: impl Dating",
    "crates/ingestion/src/dating.rs: impl EffectiveDate",
    "crates/ingestion/src/dating.rs: impl IssuanceDate",
    "crates/ingestion/src/dating.rs: impl fmt::Display for Date",
    "crates/ingestion/src/dating.rs: impl fmt::Display for EffectiveDate",
    "crates/ingestion/src/dating.rs: impl fmt::Display for IssuanceDate",
    "crates/ingestion/src/diff.rs: impl DocumentChange",
    "crates/ingestion/src/diff.rs: impl RuleChange",
    "crates/ingestion/src/diff.rs: impl SourceDiff",
    "crates/ingestion/src/document.rs: impl AdmissionYear",
    "crates/ingestion/src/document.rs: impl CohortRange",
    "crates/ingestion/src/document.rs: impl HierarchyRelation",
    "crates/ingestion/src/document.rs: impl LegalAuthority",
    "crates/ingestion/src/document.rs: impl OfficialDocument",
    "crates/ingestion/src/document.rs: impl ParsedRule",
    "crates/ingestion/src/document.rs: impl ScopeRelation",
    "crates/ingestion/src/document.rs: impl TargetScope",
    "crates/ingestion/src/document.rs: impl TransitionRelation",
    "crates/ingestion/src/document.rs: impl TransitionalMeasures",
    "crates/ingestion/src/fetch.rs: impl ConditionalRequest",
    "crates/ingestion/src/fetch.rs: impl HeaderValue",
    "crates/ingestion/src/fetch.rs: impl HttpMetadata",
    "crates/ingestion/src/fetch.rs: impl Into<String>) -> Result<Self, HeaderError>",
    "crates/ingestion/src/fetch.rs: impl Validators",
    "crates/ingestion/src/fetch.rs: impl core::fmt::Debug for FetchOutcome",
    "crates/ingestion/src/gate.rs: impl OpenGate",
    "crates/ingestion/src/graph.rs: impl DependencyGraph",
    "crates/ingestion/src/graph.rs: impl DependentKind",
    "crates/ingestion/src/graph.rs: impl DependentNode",
    "crates/ingestion/src/graph.rs: impl Invalidation",
    "crates/ingestion/src/identifier.rs: impl $name",
    "crates/ingestion/src/identifier.rs: impl Into<String>) -> Result<Self, NameError>",
    "crates/ingestion/src/identifier.rs: impl fmt::Display for $name",
    "crates/ingestion/src/manifest.rs: impl AllowedFrequency",
    "crates/ingestion/src/manifest.rs: impl AuthenticationMethod",
    "crates/ingestion/src/manifest.rs: impl ConnectorManifest",
    "crates/ingestion/src/manifest.rs: impl CredentialBinding",
    "crates/ingestion/src/manifest.rs: impl DeclaredTarget",
    "crates/ingestion/src/manifest.rs: impl ManifestDraft",
    "crates/ingestion/src/manifest.rs: impl ManifestField",
    "crates/ingestion/src/manifest.rs: impl NextVerification",
    "crates/ingestion/src/manifest.rs: impl ParserVersion",
    "crates/ingestion/src/manifest.rs: impl RetrievalInstant",
    "crates/ingestion/src/manifest.rs: impl SourceCategory",
    "crates/ingestion/src/manifest.rs: impl fmt::Debug for CredentialBinding",
    "crates/ingestion/src/manifest.rs: impl fmt::Display for DeclaredTarget",
    "crates/ingestion/src/publish.rs: impl Publication",
    "crates/ingestion/src/publish.rs: impl PublishedRules",
    "crates/ingestion/src/publish.rs: impl QueueReason",
    "crates/ingestion/src/publish.rs: impl ReviewQueued",
    "crates/ingestion/src/publish.rs: impl<'run> PublishableRules<'run>",
    "crates/ingestion/src/snapshot.rs: impl RawSnapshot",
    "crates/ingestion/src/snapshot.rs: impl fmt::Debug for RawSnapshot",
    "crates/ingestion/src/stage.rs: impl Corpus",
    "crates/ingestion/src/stage.rs: impl Described",
    "crates/ingestion/src/stage.rs: impl Fetched",
    "crates/ingestion/src/stage.rs: impl IngestSeq",
    "crates/ingestion/src/stage.rs: impl Parsed",
    "crates/ingestion/src/stage.rs: impl Proposed",
    "crates/ingestion/src/stage.rs: impl Reconciled",
    "crates/ingestion/src/stage.rs: impl RunRecord",
    "crates/ingestion/src/stage.rs: impl Snapshotted",
    "crates/ingestion/src/stage.rs: impl Stage",
    "crates/ingestion/src/stage.rs: impl StageFailure",
    "crates/ingestion/src/stage.rs: impl Validated",
    "crates/ingestion/src/stage.rs: impl core::fmt::Debug for Acquisition<'_>",
    "crates/ingestion/src/terms.rs: impl Denial",
    "crates/ingestion/src/terms.rs: impl DenialReason",
    "crates/ingestion/src/terms.rs: impl Fallback",
    "crates/ingestion/src/terms.rs: impl TermsLedger",
    "crates/ingestion/src/terms.rs: impl TermsStatus",
];
