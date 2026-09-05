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
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use common::TestResult;

/// Constructions of `Name { .. }`, less every position that is not one.
///
/// `constructions_of` carries the whole rule now. It used to subtract only
/// `struct`, `impl` and `for`, and this function subtracted the fourth form,
/// `-> Name {`, on top. `P2-A4`'s second audit found the fifth: a function
/// returning a **reference**, written `-> &Name {`, which neither subtraction
/// saw, so `RestrictedOriginal` read as having two producers. Each round of
/// that was one spelling further out, so the rule moved to what introduces
/// the name rather than to a list of the ways it can be introduced.
fn built_count(code: &str, name: &str) -> usize {
    constructions_of(code, name)
}

/// The counter is not vacuous and subtracts every position that is not a
/// construction, including the two that were counted before.
#[test]
fn the_construction_counter_reads_a_literal_and_not_a_return_type() {
    let sample = "pub struct Thing { a: u8, } impl Thing { } fn make() -> Thing { Thing { a: 1 } }";
    assert_eq!(built_count(sample, "Thing"), 1);
    let returns_only = "fn make() -> Thing { other() }";
    assert_eq!(built_count(returns_only, "Thing"), 0);
    // The form that made `RestrictedOriginal` read as two.
    let borrowed = "const fn held(&self) -> &Thing { &self.thing }";
    assert_eq!(built_count(borrowed, "Thing"), 0);
    let exclusive = "fn held(&mut self) -> &mut Thing { &mut self.thing }";
    assert_eq!(built_count(exclusive, "Thing"), 0);
    let lifetime = "fn held(&self) -> &'a Thing { &self.thing }";
    assert_eq!(built_count(lifetime, "Thing"), 0);
    // A trait impl for the type is still not a construction of it, and a
    // literal written after one still is.
    let trait_impl = "impl Debug for Thing { fn fmt(&self) { let _ = Thing { a: 1 }; } }";
    assert_eq!(built_count(trait_impl, "Thing"), 1);
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

/// Every `impl` header of `code`, up to its opening brace.
///
/// A trait impl's methods carry no visibility modifier, so an inventory keyed
/// on `pub fn` cannot see one at all. `impl From<&RestrictedOriginal> for
/// Vec<String>` is a public route to removed student speech that declares no
/// `pub fn`, and it passed this whole file before this header sweep existed.
/// The precedent is `P2-Y3`'s and `P2-X5`'s: pin the complete set of headers,
/// so a conversion nobody predicted fails as an extra entry rather than having
/// to be named on a forbidden list.
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

/// The type an `impl` header targets, and whether the header is a trait impl.
///
/// `impl From<&RestrictedOriginal> for Vec<String>` targets `Vec<String>`;
/// `impl RestrictedOriginal` targets itself and is not a trait impl. The
/// distinction matters because only the first kind holds methods a `pub fn`
/// sweep is blind to.
fn impl_target(header: &str) -> Option<(String, bool)> {
    let rest = header
        .strip_prefix("impl")
        .map(str::trim_start)
        .filter(|rest| !rest.is_empty())?;
    // Drop the generic parameter list, which may itself contain ` for `
    // nowhere but can contain angle brackets that the split below must not see.
    let rest = if rest.starts_with('<') {
        let mut depth = 0_usize;
        let mut cut = None;
        for (offset, character) in rest.char_indices() {
            match character {
                '<' => depth = depth.saturating_add(1),
                '>' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        cut = Some(offset.saturating_add(1));
                        break;
                    }
                }
                _ => {}
            }
        }
        rest.get(cut?..)?.trim_start()
    } else {
        rest
    };
    rest.split_once(" for ").map_or_else(
        || Some((rest.trim().to_owned(), false)),
        |(_, target)| Some((target.trim().to_owned(), true)),
    )
}

/// Every `pub fn`, `pub const fn`, `pub async fn` and `pub unsafe fn`
/// signature in `code`, plus every `fn` declared inside a trait `impl` block,
/// whitespace-collapsed.
///
/// The trait half is `P2-A4`'s F2: a trait impl's methods have no visibility
/// modifier, so a sweep that keyed on `pub fn ` walked past
/// `impl From<&RestrictedOriginal> for Vec<String>` and every rule below was
/// blind to it. Inside such a block `Self` is the block's target type, so the
/// signature is rewritten with the target substituted before it is returned --
/// otherwise every conversion reads as returning `Self` and no rule about
/// return types can see through it.
fn public_signatures(code: &str) -> Vec<String> {
    signatures_in_blocks(code)
        .into_iter()
        .map(|(_, signature)| signature)
        .collect()
}

/// Every signature `public_signatures` returns, paired with the `impl` header
/// it was declared in, or the empty string for a free function.
///
/// The header is what makes a rule about a *type* possible: a method of
/// `RestrictedOriginal` need not name the type anywhere in its own signature,
/// so a sweep that read signatures alone would miss an inherent
/// `pub fn all_verbatim(&self) -> Vec<String>` as surely as the `pub fn` sweep
/// missed the trait impl.
fn signatures_in_blocks(code: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = code.lines().collect();
    let mut found = Vec::new();
    // The block an `impl` opened, which runs to the first `}` at column zero --
    // the same convention `declared_item` reads an item by.
    let mut block: Option<(String, String, bool)> = None;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with('}') {
            block = None;
        }
        let trimmed = line.trim_start();
        if trimmed == "impl" || trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
            let mut header = trimmed.to_owned();
            for follow in lines.iter().skip(index.saturating_add(1)) {
                if header.contains('{') {
                    break;
                }
                header.push(' ');
                header.push_str(follow.trim());
            }
            let end = header.find('{').unwrap_or(header.len());
            let header = header[..end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            block =
                impl_target(&header).map(|(target, is_trait)| (header.clone(), target, is_trait));
            continue;
        }
        let public = [
            "pub fn ",
            "pub const fn ",
            "pub async fn ",
            "pub unsafe fn ",
        ]
        .iter()
        .any(|start| trimmed.starts_with(start));
        let in_trait_impl = block.as_ref().is_some_and(|(_, _, is_trait)| *is_trait)
            && ["fn ", "const fn ", "async fn ", "unsafe fn "]
                .iter()
                .any(|start| trimmed.starts_with(start));
        if !public && !in_trait_impl {
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
        let signature = signature.split_whitespace().collect::<Vec<_>>().join(" ");
        let (header, signature) = match block.as_ref() {
            Some((header, target, true)) => (header.clone(), substitute_self(&signature, target)),
            Some((header, _, false)) => (header.clone(), signature),
            None => (String::new(), signature),
        };
        found.push((header, signature));
    }
    found
}

/// Every signature in `code` that touches `name`: declared in an `impl` block
/// for it, or naming it in its own parameters or return.
fn signatures_touching(code: &str, name: &str) -> BTreeSet<String> {
    signatures_in_blocks(code)
        .into_iter()
        .filter(|(header, signature)| uses_of(header, name) > 0 || uses_of(signature, name) > 0)
        .map(|(_, signature)| signature)
        .collect()
}

/// `signature` with every whole-word `Self` replaced by `target`.
fn substitute_self(signature: &str, target: &str) -> String {
    signature
        .split_inclusive(|character: char| !character.is_alphanumeric() && character != '_')
        .map(|piece| {
            let (word, tail) = piece.split_at(
                piece
                    .find(|character: char| !character.is_alphanumeric() && character != '_')
                    .unwrap_or(piece.len()),
            );
            if word == "Self" {
                format!("{target}{tail}")
            } else {
                piece.to_owned()
            }
        })
        .collect()
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

/// Whether a `Name {` written after `before` is a declaration or a type
/// rather than a construction.
///
/// Four tokens introduce the name in a position that builds nothing:
/// `struct Name {`, `impl Name {`, `impl Trait for Name {` and a return type
/// `-> Name {`. Between the token and the name a type may carry `&`, `&mut`
/// and a lifetime, and those are stripped rather than enumerated as further
/// prefixes — which is the repair, because a list of prefixes is what failed:
/// `-> Name {` was subtracted and `-> &RestrictedOriginal {` was not, so the
/// type that holds the removed speech read as having two producers when it has
/// one, and `P2-A4`'s second audit found it had no producer count at all.
fn introduced_as_a_type(before: &str) -> bool {
    let mut head = before.trim_end();
    loop {
        let shorter = if let Some(rest) = strip_token(head, "mut") {
            rest
        } else if let Some(rest) = head.strip_suffix('&') {
            rest
        } else if let Some(rest) = strip_lifetime(head) {
            rest
        } else {
            break;
        };
        head = shorter.trim_end();
    }
    head.ends_with("->")
        || ["struct", "impl", "for"]
            .iter()
            .any(|token| strip_token(head, token).is_some())
}

/// `text` without a trailing whole-word `token`, or `None`.
fn strip_token<'a>(text: &'a str, token: &str) -> Option<&'a str> {
    let rest = text.strip_suffix(token)?;
    let boundary = rest
        .chars()
        .next_back()
        .is_none_or(|character| !(character.is_alphanumeric() || character == '_'));
    boundary.then_some(rest)
}

/// `text` without a trailing lifetime such as `'a` or `'static`, or `None`.
fn strip_lifetime(text: &str) -> Option<&str> {
    let name: String = text
        .chars()
        .rev()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    let cut = text.len().checked_sub(name.len())?;
    let rest = text.get(..cut)?;
    rest.strip_suffix('\'')
}

/// Counts constructions of `Name { .. }`, less the declarations and the type
/// positions that spell the same characters.
///
/// A bare `Name {` count reads a struct declaration, an inherent `impl`, a
/// trait `impl` and a return type as constructions. Subtracting them is the
/// same repair `declarations_of` is to `uses_of` one level up: the count has
/// to be of the thing, not of the spelling.
fn constructions_of(code: &str, name: &str) -> usize {
    let literal = format!("{name} {{");
    code.match_indices(&literal)
        .filter(|(at, _)| {
            code.get(..*at)
                .is_some_and(|before| !introduced_as_a_type(before))
        })
        .count()
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
// A restricted original has one producer
// ---------------------------------------------------------------------------

/// The record of what a redaction took out is built in one place.
///
/// `P2-A4`'s second audit enumerated this crate's construction counts rather
/// than asserting them and found exactly three — `AccuracyWitness`,
/// `DiarizationMeasurement` and `ReviewedCapture`. The type that *holds the
/// removed speech* had none, and that is why the forgery half of its F1
/// worked: a second `RestrictedOriginal { … }` written beside the real one,
/// copying the real digest and leaving `removed` empty, is a record of a
/// redaction that says nothing was taken out of somebody's lecture, and
/// `RawAccessGrant::issued(real, …)` **opens it**, because the refusal
/// compares the digest and the digest was carried over. The audit measured it
/// disclosing 0 utterances while the real original held 3, and writing an
/// audit row asserting `utterances_disclosed = 0`.
///
/// A count over constructions is the shape that catches that, and it is the
/// shape the other three already had. It is independent of
/// `every_item_that_reaches_a_closed_type_is_pinned` in
/// `crates/contracts/tests/item_inventory_scans.rs`, which catches the same
/// injection as an item nobody wrote down: this one catches it as a second
/// producer even if somebody adds it to that pin.
#[test]
fn a_restricted_original_has_one_producer() -> TestResult {
    let mut built = 0;
    for path in crate_product_sources()? {
        built += built_count(&code_of(&path)?, "RestrictedOriginal");
    }
    assert_eq!(built, 1, "a RestrictedOriginal is built somewhere else");

    // And the one place is the redaction, not merely somewhere in the file.
    let derivative = code_of(&crate_root().join("src/derivative.rs"))?;
    let redact = declared_item(&derivative, "pub fn redact(")?;
    assert_eq!(
        built_count(&redact, "RestrictedOriginal"),
        1,
        "the one construction of an original is outside `redact`"
    );
    assert_eq!(
        declarations_of(&derivative, "redact"),
        1,
        "a second function named redact exists"
    );

    // The surface is seven reads and no write. `open` is the only one that
    // returns the speech, and it takes the grant by value.
    assert_eq!(
        public_methods(&derivative, "impl RestrictedOriginal {")?,
        vec![
            "classification".to_owned(),
            "digest".to_owned(),
            "lecture".to_owned(),
            "open".to_owned(),
            "removed_count".to_owned(),
            "source_version".to_owned(),
            "terms".to_owned(),
        ],
        "the restricted original's public surface changed"
    );
    // No method takes the value by exclusive reference, so nothing writes
    // through one. This is the half claim 5 is literally about, stated over
    // the whole signature set rather than over the three the reader expects.
    for signature in signatures_touching(&derivative, "RestrictedOriginal") {
        assert!(
            !signature.contains("&mut self"),
            "a restricted original has a method that can write through it: {signature}"
        );
    }

    // No second route into one: no `Default`, and the derive list is what it
    // is. A `Serialize` added here hands out `removed` with no grant at all.
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        assert_eq!(
            occurrences(&code, "impl Default for RestrictedOriginal"),
            0,
            "{} gives an original a second route",
            relative(&path)
        );
    }
    assert_eq!(
        declared_item(&derivative, "pub struct RestrictedOriginal {")?,
        WHOLE_RESTRICTED_ORIGINAL,
        "the original's fields changed"
    );
    assert_eq!(
        occurrences(&derivative, RESTRICTED_ORIGINAL_DERIVES),
        1,
        "the derives on a restricted original changed"
    );
    Ok(())
}

/// The derive list on a restricted original, with the line it sits on.
///
/// Load-bearing, and invisible to every sweep in this file that reads a
/// signature or an `impl` header: the items a derive writes are generated, so
/// a serializing derive added here would hand out `removed` with no grant and
/// no log row and no source line for a collector to keep. `Clone` is what
/// lets a holder keep a copy; it is not what let `P2-A4` forge one, which
/// needed the fields.
const RESTRICTED_ORIGINAL_DERIVES: &str = "#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedOriginal {";

/// The fields a restricted original holds, whole.
const WHOLE_RESTRICTED_ORIGINAL: &str = "pub struct RestrictedOriginal { lecture: LectureSessionId, source_version: u32, terms: RetentionTerms, removed: Vec<RemovedUtterance>, digest: ContentDigest, }";

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
// Every impl header is in the inventory
// ---------------------------------------------------------------------------

/// Every `impl` header this crate declares, pinned as a complete set.
///
/// Three entries, and all three are the redacting `Debug` this crate writes by
/// hand instead of deriving. That is the whole trait surface: this crate
/// implements no conversion, no dereference, no iteration and no arithmetic for
/// any type it owns.
const IMPL_HEADERS: &[&str] = &[
    "impl AccuracyWitness",
    "impl AffectedProjection",
    "impl AffectedProjectionKind",
    "impl CaptureUnderReview",
    "impl CaseMeasurement",
    "impl DeletionOutcome",
    "impl DerivedArtifact",
    "impl DiarizationCase",
    "impl DiarizationCorpus",
    "impl DiarizationMeasurement",
    "impl DiarizationThreshold",
    "impl DisclosedOriginal<'_>",
    "impl EvidenceIndex",
    "impl ExclusionRecord",
    "impl HoldState",
    "impl IngestionJobKind",
    "impl IngestionReceipt",
    "impl KeptUtterance",
    "impl LectureDeletionPlan",
    "impl LectureDeletionPreview",
    "impl ManualExclusion",
    "impl PiiClass",
    "impl PiiFinding",
    "impl ProjectionEffect",
    "impl ProjectionRecord",
    "impl RawAccessGrant",
    "impl RawAccessLog",
    "impl RawAccessRecord",
    "impl RedactedDerivative",
    "impl Redaction",
    "impl RedactionMode",
    "impl RedactionPlan",
    "impl RedactionPolicy",
    "impl RedactionScope",
    "impl RestrictedOriginal",
    "impl ReviewDecision",
    "impl ReviewOutcome",
    "impl ReviewedCapture<'_>",
    "impl SpeakerTargeting",
    "impl VoiceClass",
    "impl VoiceSpan",
    "impl fmt::Debug for KeptUtterance",
    "impl fmt::Debug for RemovedUtterance",
    "impl fmt::Debug for SourceUtterance<'_>",
    "impl<'a> LectureSource<'a>",
    "impl<'a> SourceUtterance<'a>",
];

/// Every trait `impl` in this crate is one of the three redacting `Debug`s.
///
/// `P2-A4`'s F2 and F3 are one defect measured twice: `public_signatures` read
/// only lines beginning `pub fn `, so
/// `impl From<&RestrictedOriginal> for Vec<String>` — five lines handing out
/// removed student speech with no grant and no audit row — passed all
/// twenty-four tests of this crate on both hosts, and so did
/// `impl From<&DiarizationMeasurement> for AccuracyWitness`, which minted an
/// automatic-editing claim out of a measurement the threshold had refused.
///
/// The close is a **whole-set** comparison of the header inventory rather than
/// a list of forbidden trait names: `From` is the spelling that was measured,
/// but `Into`, `Deref`, `AsRef`, `Borrow`, `Index`, `IntoIterator` and a trait
/// this crate's author has not thought of all reach the same private fields.
/// An entry nobody predicted fails as an extra key.
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

    // Stated again as a property of the whole set, so the reason survives an
    // edit to the list: the only trait this crate implements for any of its
    // types is `fmt::Debug`, and each of those three is written by hand to
    // redact. There is no conversion, no dereference, no iteration and no
    // arithmetic anywhere in the inventory, which is a stronger statement than
    // a list of forbidden trait names because it is closed on the other side.
    let traits: Vec<&str> = found
        .iter()
        .filter(|header| impl_target(header).is_some_and(|(_, is_trait)| is_trait))
        .map(String::as_str)
        .collect();
    assert_eq!(
        traits,
        vec![
            "impl fmt::Debug for KeptUtterance",
            "impl fmt::Debug for RemovedUtterance",
            "impl fmt::Debug for SourceUtterance<'_>",
        ],
        "this crate implements a trait that is not the redacting Debug"
    );

    // The scanner is not vacuous: it finds the header `P2-A4` injected, it
    // reads the target out of a trait impl and out of an inherent one, and it
    // does not read an `impl Trait` in argument position as a header.
    assert_eq!(
        impl_headers("impl From<&RestrictedOriginal> for Vec<String> {\n}\n"),
        ["impl From<&RestrictedOriginal> for Vec<String>"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(
        impl_target("impl From<&RestrictedOriginal> for Vec<String>"),
        Some(("Vec<String>".to_owned(), true))
    );
    assert_eq!(
        impl_target("impl<'a> LectureSource<'a>"),
        Some(("LectureSource<'a>".to_owned(), false))
    );
    assert!(impl_headers("fn takes(value: impl Display) {}\n").is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// Every route out of a restricted original is in the inventory
// ---------------------------------------------------------------------------

/// Every signature that touches a [`RestrictedOriginal`], pinned.
///
/// Nine entries: the six read-only accessors declared in its own `impl` block,
/// the grant constructor that binds a grant to it, the accessor that reads that
/// binding back, and `open`. Exactly one of the nine returns the removed
/// speech, and it is `open`, which takes the grant **by value** and appends to
/// the log before it returns.
const RESTRICTED_ORIGINAL_SIGNATURES: &[&str] = &[
    "pub const fn classification(&self) -> &'static str {",
    "pub const fn digest(&self) -> &ContentDigest {",
    "pub const fn lecture(&self) -> LectureSessionId {",
    "pub const fn original(&self) -> &RestrictedOriginal {",
    "pub const fn source_version(&self) -> u32 {",
    "pub const fn terms(&self) -> RetentionTerms {",
    "pub fn issued( original: &RestrictedOriginal, requested_by: Actor, purpose: &str, at: u64, ) -> Result<Self, AccessRefusal> {",
    "pub fn open( &self, grant: RawAccessGrant, log: &mut RawAccessLog, ) -> Result<DisclosedOriginal<'_>, AccessRefusal> {",
    "pub fn removed_count(&self) -> usize {",
];

/// Every signature that touches an [`AccuracyWitness`], pinned.
///
/// Ten entries. One produces a witness -- `witness`, which compares both axes
/// against the threshold -- one consumes it by value into an automatic
/// redaction mode, one hands back a reference to the one a plan already holds,
/// and the rest are the witness's own read-only accessors.
const ACCURACY_WITNESS_SIGNATURES: &[&str] = &[
    "pub const fn accuracy_permille(&self) -> u64 {",
    "pub const fn corpus_digest(&self) -> &ContentDigest {",
    "pub const fn corpus_version(&self) -> u32 {",
    "pub const fn missed_student_permille(&self) -> u64 {",
    "pub const fn scorer_version(&self) -> u32 {",
    "pub const fn threshold(&self) -> DiarizationThreshold {",
    "pub const fn witness(&self) -> Option<&AccuracyWitness> {",
    "pub fn automatic(policy: RedactionPolicy, witness: AccuracyWitness) -> Self {",
    "pub fn corpus_id(&self) -> &str {",
    "pub fn witness( &self, threshold: DiarizationThreshold, ) -> Result<AccuracyWitness, AccuracyRefusal> {",
];

/// Every signature that touches a [`DisclosedOriginal`], pinned.
///
/// Six entries. `open` is where one comes from; the other five are the reads a
/// holder may make, and each is by position rather than in bulk. `P2-A4` noted
/// that an inherent `pub fn all_verbatim(&self) -> Vec<String>` added here
/// survives `no_disclosure_reaches_a_derivative`, because that rule is about
/// routes into a *derivative* type and a `Vec<String>` is not one. It grants no
/// capability a holder lacks, so it was recorded as an observation about that
/// rule's scope rather than a hole — but a bulk read is a different shape from
/// five positional ones, and it is a seventh entry here.
const DISCLOSED_ORIGINAL_SIGNATURES: &[&str] = &[
    "pub const fn is_empty(&self) -> bool {",
    "pub const fn len(&self) -> usize {",
    "pub fn open( &self, grant: RawAccessGrant, log: &mut RawAccessLog, ) -> Result<DisclosedOriginal<'_>, AccessRefusal> {",
    "pub fn source_index(&self, position: usize) -> Option<usize> {",
    "pub fn speaker(&self, position: usize) -> Option<Speaker> {",
    "pub fn verbatim(&self, position: usize) -> Option<&str> {",
];

/// The three closed types have a complete signature inventory, both ways.
///
/// A rule about *return* types cannot close `P2-A4`'s F2, because
/// `impl From<&RestrictedOriginal> for Vec<String>` returns a standard-library
/// collection that no forbidden list would name. What closes it is counting
/// every signature that touches the type at all and comparing the set: the
/// injected `fn from(original: &RestrictedOriginal) -> Vec<String>` is an
/// eighth entry here whatever it returns and however it is spelled.
///
/// The same rule holds for `AccuracyWitness`, where F3's injection is a tenth
/// entry, and it is a stronger statement than
/// `an_accuracy_witness_has_one_producer`'s construction count: that count is
/// over `AccuracyWitness {` literals, and a conversion is free to build one.
#[test]
fn every_signature_naming_a_closed_type_is_in_the_inventory() -> TestResult {
    let mut originals: BTreeSet<String> = BTreeSet::new();
    let mut witnesses: BTreeSet<String> = BTreeSet::new();
    let mut disclosures: BTreeSet<String> = BTreeSet::new();
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        originals.extend(signatures_touching(&code, "RestrictedOriginal"));
        witnesses.extend(signatures_touching(&code, "AccuracyWitness"));
        disclosures.extend(signatures_touching(&code, "DisclosedOriginal"));
    }
    assert_eq!(
        originals,
        RESTRICTED_ORIGINAL_SIGNATURES
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the routes to a restricted original changed"
    );
    assert_eq!(
        witnesses,
        ACCURACY_WITNESS_SIGNATURES
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the routes to an accuracy witness changed"
    );
    assert_eq!(
        disclosures,
        DISCLOSED_ORIGINAL_SIGNATURES
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the reads a disclosure offers changed"
    );

    // The sweep sees a trait impl's method, which is the whole point: the same
    // fragment run through the old `pub fn` inventory produced nothing.
    let injected = "impl From<&RestrictedOriginal> for Vec<String> {
    fn from(original: &RestrictedOriginal) -> Self {
        original.removed.iter().map(|u| u.verbatim.clone()).collect()
    }
}
";
    assert_eq!(
        public_signatures(injected),
        vec!["fn from(original: &RestrictedOriginal) -> Vec<String> {".to_owned()],
        "the signature sweep is blind to a trait impl again"
    );
    assert!(
        !RESTRICTED_ORIGINAL_SIGNATURES.contains(&public_signatures(injected)[0].as_str()),
        "the injected route is on the inventory"
    );

    // `Self` resolves to the block's target and not to the parameter's type,
    // so a conversion cannot hide its return behind the keyword.
    assert_eq!(
        public_signatures(
            "impl From<&DiarizationMeasurement> for AccuracyWitness {\n    fn from(m: &DiarizationMeasurement) -> Self {\n"
        ),
        vec!["fn from(m: &DiarizationMeasurement) -> AccuracyWitness {".to_owned()]
    );
    // An inherent block's `pub fn` is still collected and its `Self` is left
    // alone, so the inventories above are what the source says and not what the
    // substitution rewrote.
    assert_eq!(
        public_signatures("impl RawAccessGrant {\n    pub fn issued() -> Self {\n"),
        vec!["pub fn issued() -> Self {".to_owned()]
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
