//! What `academic-role-profile` may reach, hold and hand out.
//!
//! ## Why a token list is not enough, measured five tasks ago
//!
//! `P2-R2` shipped this crate's ancestor with a forbidden-token list as its
//! only net, and `docs/contracts/policy-source-scans.md` records what that
//! measured: seven spellings of a filesystem or environment reach compile,
//! spell none of the listed tokens, add no `use` item, and passed. The repair
//! was not a longer list. It was three **whole-set** comparisons, in both
//! directions:
//!
//! * every `use` item ([`USE_ITEMS`], [`CRATE_IMPORTS`], [`RE_EXPORTS`]);
//! * every two-segment path reached through a crate root ([`REACHED_PATHS`]);
//! * every macro invoked ([`MACROS_SPELLED`]).
//!
//! [`FORBIDDEN_CONSTRUCTS`] is kept as the third and weakest layer, because it
//! names the shapes a reader expects to see refused.
//!
//! ## What this crate is allowed to hold
//!
//! Six things, and the last column of [`FIELDS`] is which one:
//!
//! * a **caller-supplied identifier** --- the text inside a lineage identity, a
//!   scope or a direction name, admitted by `identity::validated`;
//! * an **identifier a refusal echoes** --- the copy a `RoleError` carries so
//!   its message can name what it refused;
//! * a **closed vocabulary value** --- one of this crate's enumerations;
//! * a **value of a reviewed crate** --- `P2-Y1`'s competency identity,
//!   `P2-U6`'s calendar date;
//! * a **value of this crate**; and
//! * **the user's own words** --- a label, a source citation, the reason for an
//!   adjustment. Section 24.2 asks for all three by name and a bundle a reader
//!   cannot read is not inspectable, so they are inventoried rather than
//!   refused.
//!
//! There is no seventh, and in particular there is no byte buffer and no
//! timestamp: no field of this crate is declared `u8` or `u64` under any name,
//! and nothing here records when anything happened. A date arrives as a
//! `RecordedOn`, which is `P2-U6`'s `Date` — valid time, with no constructor
//! that takes an instant — so *freshness stays `P2-N3`'s* is a property of the
//! whole crate.
//!
//! ## Four rules of this task that are pinned rather than described
//!
//! `the_only_producers_of_a_bundle_are_the_three_doors` compares the whole set
//! of public functions that return a `RoleProfile` **by value** against
//! `declare`, `revise` and `fork`. That is `GATE-38-029` in the source: a
//! shipped default bundle would be a fourth producer, and there is none.
//!
//! `an_interest_is_not_an_input_to_anything` compares the whole set of public
//! signatures naming `RoleInterest` against its own four functions, so a
//! favourite cannot become an argument to anything without failing here.
//!
//! `a_label_reaches_no_direction_and_no_single_bundle` requires `direction.rs`
//! to name `RoleLabel` zero times, and pins `by_label`'s return type: it is a
//! reading over a list, and no public function keyed on a label returns one
//! bundle.
//!
//! `the_identity_is_a_pair_and_the_rendering_has_no_inverse` pins
//! `RoleProfileRef`'s two fields and requires the rendered spelling to be
//! produced in exactly one place and consumed nowhere as an identity.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root().join("src"), &mut found)?;
    walk(&crate_root().join("tests"), &mut found)?;
    found.sort();
    Ok(found)
}

fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    let mut found = Vec::new();
    walk(&root, &mut found)?;
    found.retain(|path| {
        !path
            .strip_prefix(&root)
            .unwrap_or(path)
            .starts_with("tests")
    });
    found.sort();
    Ok(found)
}

fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Removes comments, string literals and character literals.
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
                let terminator: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
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

/// One brace-balanced block's text, from `header` to its matching `}`.
///
/// The slice, with its line breaks intact, so a reader that needs them — like
/// [`public_signatures`] — can be run over one block rather than a whole file.
fn raw_block<'a>(source: &'a str, header: &str) -> Result<&'a str, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("{header} opens no block"))?;
    let mut depth = 0_usize;
    let bytes = source.as_bytes();
    let mut at = open;
    while at < bytes.len() {
        match bytes[at] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        at += 1;
    }
    Ok(&source[start..=at.min(bytes.len() - 1)])
}

/// The same block, collapsed to one line so a rewrapped pin still matches.
fn whole_block(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    Ok(collapse(raw_block(source, header)?))
}

/// Drops comment lines and squeezes runs of whitespace to one space.
fn collapse(body: &str) -> String {
    let kept: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    kept.join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Counts whole-identifier occurrences of `name` in already-stripped code.
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

/// Drops every `use` item, so a re-export is not counted as a reach.
fn without_use_items(code: &str) -> String {
    let mut kept = String::with_capacity(code.len());
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim_start();
        let opens = trimmed.starts_with("use ")
            || (trimmed.starts_with("pub") && trimmed.contains(" use "));
        if inside || opens {
            inside = !line.trim_end().ends_with(';');
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    kept
}

/// The relative path of `path` under the workspace, with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn source_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?)
}

/// This crate's product files, as code with comments and literals removed.
fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

/// Collapses whitespace so a rewrapped signature still matches its pin.
fn tighten(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find(char::is_whitespace) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let stop = tail
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(tail.len());
        let after = &tail[stop..];
        let joins = out.ends_with("::")
            || out.ends_with('!')
            || after.starts_with("::")
            || after.starts_with('!');
        if !joins {
            out.push(' ');
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Every two-segment path `code` spells through a crate root.
fn absolute_paths(code: &str) -> BTreeSet<String> {
    let roots = ["std", "core", "alloc", "thiserror"];
    let code = &tighten(code);
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    let mut taken = 0_usize;
    for (at, _) in code.match_indices("::") {
        let mut start = at;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        if start == at {
            continue;
        }
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        // A middle segment of a longer path -- the `b` of `a::b::c` -- is not a
        // crate root, and skipping it is what stops one path yielding two keys.
        // What decides it is whether this segment already sits inside a key
        // this pass took, not the byte three positions back. `tighten` glues
        // `as ::std` shut, so that byte is the `s` of a keyword and the leading
        // `::` of a qualified path read as a middle one: `P2-A5` measured
        // `<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)` resolving
        // a name from a live function while this pass reported nothing. Every
        // segment outside a key already taken is a root, and a root nobody
        // admits fails as an extra key rather than passing.
        if start < taken {
            continue;
        }
        let root = &code[start..at];
        if !roots.contains(&root) && !root.starts_with("academic_") {
            continue;
        }
        let mut end = at + 2;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if end > at + 2 {
            found.insert(code[start..end].to_owned());
            taken = end;
        }
    }
    found
}

/// Every macro `code` invokes, by name.
fn macros_spelled(code: &str) -> BTreeSet<String> {
    let code = &tighten(code);
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    for (at, _) in code.match_indices('!') {
        let opens = bytes
            .get(at + 1)
            .is_some_and(|byte| matches!(byte, b'(' | b'[' | b'{'));
        if !opens {
            continue;
        }
        let mut start = at;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        if start == at {
            continue;
        }
        let name = &code[start..at];
        let keyword = matches!(
            name,
            "if" | "while" | "for" | "match" | "return" | "else" | "let" | "in"
        );
        if !keyword && (bytes[start].is_ascii_lowercase() || bytes[start] == b'_') {
            found.insert(name.to_owned());
        }
    }
    found
}

/// Every `pub fn` in `code`, as its name and its signature up to the body.
fn public_signatures(code: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for marker in ["pub fn ", "pub const fn "] {
        let mut cursor = 0;
        while let Some(at) = code[cursor..].find(marker).map(|at| at + cursor) {
            let after = at + marker.len();
            let name: String = code[after..]
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            let end = code[at..]
                .find(" {\n")
                .or_else(|| code[at..].find(";\n"))
                .map_or(code.len(), |offset| at + offset);
            found.push((name, code[at..end].to_owned()));
            cursor = after;
        }
    }
    found
}

/// Counts declarations of a function whose name is exactly `name`.
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

/// Every named field of every `struct` and `enum` `code` declares.
///
/// A struct-variant of an enumeration is reported as `Enum::Variant`, so a
/// field added inside one is a key here rather than a shape this extractor has
/// no name for. A tuple struct has no named field at all, and
/// [`the_unnamed_fields_are_pinned`] closes those separately.
fn type_fields(code: &str) -> Vec<(String, String, String)> {
    let mut found = Vec::new();
    for keyword in ["struct ", "enum "] {
        let mut cursor = 0;
        while let Some(at) = code[cursor..].find(keyword).map(|at| at + cursor) {
            cursor = at + keyword.len();
            let before = code[..at].chars().next_back().unwrap_or(' ');
            if before.is_alphanumeric() || before == '_' {
                continue;
            }
            let name: String = code[cursor..]
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let rest = &code[cursor + name.len()..];
            let opens = rest.find(['{', ';', '(']);
            let Some(offset) = opens else {
                continue;
            };
            if rest.as_bytes()[offset] != b'{' {
                continue;
            }
            let body_start = cursor + name.len() + offset;
            let Some(body) = balanced(&code[body_start..]) else {
                continue;
            };
            collect_fields(&name, body, &mut found);
        }
    }
    found.sort();
    found
}

/// The text between `code`'s first `{` and its matching `}`, exclusive.
fn balanced(code: &str) -> Option<&str> {
    let bytes = code.as_bytes();
    let mut depth = 0_usize;
    for (at, byte) in bytes.iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&code[1..at]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits one declaration body into its top-level items and reads each.
fn collect_fields(owner: &str, body: &str, found: &mut Vec<(String, String, String)>) {
    for item in top_level_items(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if let Some(open) = item.find('{') {
            let variant = item[..open]
                .split_whitespace()
                .next_back()
                .unwrap_or("")
                .to_owned();
            if let Some(inner) = balanced(&item[open..]) {
                collect_fields(&format!("{owner}::{variant}"), inner, found);
            }
            continue;
        }
        let Some(colon) = item.find(':') else {
            continue;
        };
        if item.as_bytes().get(colon + 1) == Some(&b':') {
            continue;
        }
        let name = item[..colon]
            .split_whitespace()
            .next_back()
            .unwrap_or("")
            .to_owned();
        if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') {
            continue;
        }
        let declared: String = item[colon + 1..].split_whitespace().collect();
        found.push((owner.to_owned(), name, declared));
    }
}

/// Splits `body` on the commas that sit at nesting depth zero.
fn top_level_items(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut braces = 0_usize;
    let mut parens = 0_usize;
    let mut angles = 0_usize;
    for character in body.chars() {
        match character {
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '(' | '[' => parens += 1,
            ')' | ']' => parens = parens.saturating_sub(1),
            '<' => angles += 1,
            '>' => angles = angles.saturating_sub(1),
            ',' if braces == 0 && parens == 0 && angles == 0 => {
                items.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    items.push(current);
    items
}

// ---------------------------------------------------------------------------
// The inventories.
// ---------------------------------------------------------------------------

/// Every `use` item of a product file that is not a `crate::` item.
///
/// Fifteen, and the three crate roots among them are the whole of this crate's
/// reach outside itself: `P2-Y1`'s competency identity, `P2-C1`'s predicate
/// registry, and `P2-U6`'s calendar date. There is no `std::fs`, no `std::net`,
/// no `std::env`, no `std::process` and no `std::time`; the only `std` item is
/// a collection.
const USE_ITEMS: [(&str, &str); 15] = [
    (
        "crates/role-profile/src/adjustment.rs",
        "use academic_competency::CompetencyId;",
    ),
    (
        "crates/role-profile/src/adjustment.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/role-profile/src/bundle.rs",
        "use academic_competency::CompetencyId;",
    ),
    (
        "crates/role-profile/src/bundle.rs",
        "use academic_ingestion::Date;",
    ),
    (
        "crates/role-profile/src/bundle.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/role-profile/src/direction.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/role-profile/src/identity.rs",
        "use academic_domain::predicates::{PredicateName, QualifierKind};",
    ),
    (
        "crates/role-profile/src/identity.rs",
        "use core::{fmt, num::NonZeroU32};",
    ),
    (
        "crates/role-profile/src/identity.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/role-profile/src/interest.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/role-profile/src/lib.rs",
        "use academic_competency::CompetencyId;",
    ),
    (
        "crates/role-profile/src/lib.rs",
        "use academic_domain::predicates::{NodeType, PredicateName};",
    ),
    (
        "crates/role-profile/src/lib.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/role-profile/src/lib.rs",
        "use std::collections::BTreeSet;",
    ),
    (
        "crates/role-profile/src/shelf.rs",
        "use std::collections::{BTreeMap, BTreeSet};",
    ),
];

/// Every `use crate::` item of a product file.
const CRATE_IMPORTS: [(&str, &str); 6] = [
    (
        "crates/role-profile/src/adjustment.rs",
        "use crate::{ RoleError, bundle::BundleImportance, identity::{RoleProfileRef, non_empty}, };",
    ),
    (
        "crates/role-profile/src/bundle.rs",
        "use crate::{ RoleError, identity::{RoleProfileRef, non_empty, validated}, };",
    ),
    (
        "crates/role-profile/src/direction.rs",
        "use crate::{RoleError, identity::validated};",
    ),
    (
        "crates/role-profile/src/identity.rs",
        "use crate::RoleError;",
    ),
    (
        "crates/role-profile/src/interest.rs",
        "use crate::identity::RoleProfileId;",
    ),
    (
        "crates/role-profile/src/shelf.rs",
        "use crate::{ RoleError, RoleProfile, direction::{NO_SHIPPED_BUNDLES, RoleDirection}, identity::{RoleLabel, RoleProfileId, RoleProfileRef}, };",
    ),
];

/// Every `pub use` re-export, all of which are in `lib.rs`.
const RE_EXPORTS: [&str; 6] = [
    "pub use adjustment::{Adjustment, AdjustmentLayer, UserAdjustment};",
    "pub use bundle::{ BundleEntry, BundleImportance, BundleOrigin, BundleScope, BundleSource, RecordedOn, };",
    "pub use direction::{DirectionName, NO_SHIPPED_BUNDLES, RoleDirection};",
    "pub use identity::{RoleLabel, RoleProfileId, RoleProfileRef, RoleProfileVersion};",
    "pub use interest::{InterestStanding, REFUSED_STANDINGS, RoleInterest};",
    "pub use shelf::{BundleShelf, DirectionCoverage, LabelAmbiguity, LabelReading};",
];

/// Every two-segment path reached through a crate root, outside `use` items.
///
/// One. Everything else this crate names arrives through a `use` item that is
/// itself in the inventory above, so there is no back door where a filesystem
/// or transport module is reached by its absolute path without an import.
const REACHED_PATHS: [(&str, &str); 1] = [(
    "thiserror::Error",
    "the derive on RoleError, which is the only attribute path in this crate",
)];

/// Every macro this crate invokes.
///
/// Two. Neither reads anything: an `include_str!` or an `env!` would be a third
/// key here, and a `println!` would be a fourth.
const MACROS_SPELLED: [(&str, &str); 2] = [
    (
        "format",
        "the rendered section 24.2 `id` spelling, and nothing else",
    ),
    (
        "matches",
        "the byte class in `validated`, and the origin arm in `BundleOrigin::base`",
    ),
];

/// The files of this package permitted to read a file at all.
const READERS: [&str; 2] = ["role_scans.rs", "role_bundles.rs"];

/// The third and weakest layer.
const FORBIDDEN_CONSTRUCTS: [&str; 17] = [
    "File",
    "OpenOptions",
    "read_to_string",
    "fs::",
    "Path",
    "PathBuf",
    "env",
    "var",
    "Command",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "net",
    "reqwest",
    "Instant",
    "SystemTime",
    "now",
];

/// The six things a field of this crate may hold.
const REASONS: [&str; 6] = [
    "a caller-supplied identifier",
    "an identifier a refusal echoes",
    "a closed vocabulary value",
    "a value of a reviewed crate",
    "a value of this crate",
    "the user's own words",
];

/// Every named field of every type this crate declares.
const FIELDS: [(&str, &str, &str, &str); 57] = [
    (
        "Adjustment::Add",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "Adjustment::Add",
        "importance",
        "BundleImportance",
        "a closed vocabulary value",
    ),
    (
        "Adjustment::Remove",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "Adjustment::Reweight",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "Adjustment::Reweight",
        "importance",
        "BundleImportance",
        "a closed vocabulary value",
    ),
    (
        "AdjustmentLayer",
        "adjustments",
        "Vec<UserAdjustment>",
        "a value of this crate",
    ),
    (
        "AdjustmentLayer",
        "base",
        "RoleProfileRef",
        "a value of this crate",
    ),
    (
        "AdjustmentWire",
        "adjustment",
        "Adjustment",
        "a value of this crate",
    ),
    (
        "AdjustmentWire",
        "because",
        "String",
        "the user's own words",
    ),
    (
        "BundleEntry",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "BundleEntry",
        "importance",
        "BundleImportance",
        "a closed vocabulary value",
    ),
    (
        "BundleShelf",
        "profiles",
        "BTreeMap<RoleProfileRef,RoleProfile>",
        "a value of this crate",
    ),
    ("BundleSource", "cited_as", "String", "the user's own words"),
    (
        "BundleSource",
        "consulted_on",
        "RecordedOn",
        "a value of this crate",
    ),
    (
        "DirectionCoverage",
        "direction",
        "RoleDirection",
        "a closed vocabulary value",
    ),
    (
        "DirectionCoverage",
        "held",
        "Vec<RoleProfileRef>",
        "a value of this crate",
    ),
    (
        "LabelAmbiguity",
        "lineages",
        "Vec<RoleProfileId>",
        "a value of this crate",
    ),
    (
        "LabelAmbiguity",
        "scopes",
        "Vec<String>",
        "a caller-supplied identifier",
    ),
    (
        "LabelReading",
        "ambiguity",
        "Option<LabelAmbiguity>",
        "a value of this crate",
    ),
    (
        "LabelReading",
        "label",
        "RoleLabel",
        "a value of this crate",
    ),
    (
        "LabelReading",
        "reached",
        "Vec<RoleProfileRef>",
        "a value of this crate",
    ),
    (
        "LayerWire",
        "base",
        "RoleProfileRef",
        "a value of this crate",
    ),
    (
        "LayerWire",
        "user_adjustments",
        "Vec<UserAdjustment>",
        "a value of this crate",
    ),
    (
        "RoleError::AddedCompetencyAlreadyPresent",
        "competency",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleError::AddedCompetencyAlreadyPresent",
        "profile",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleError::AdjustedCompetencyIsNotInTheBundle",
        "competency",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleError::AdjustedCompetencyIsNotInTheBundle",
        "profile",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleError::DuplicateCompetency",
        "competency",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleError::DuplicateCompetency",
        "profile",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleError::LayerIsForAnotherVersion",
        "layer_base",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleError::LayerIsForAnotherVersion",
        "profile",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleError::OriginDoesNotMatchTheVersion",
        "origin",
        "&'staticstr",
        "a closed vocabulary value",
    ),
    (
        "RoleError::OriginDoesNotMatchTheVersion",
        "profile",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "RoleInterest",
        "profile",
        "RoleProfileId",
        "a value of this crate",
    ),
    (
        "RoleInterest",
        "standing",
        "InterestStanding",
        "a closed vocabulary value",
    ),
    (
        "RoleProfile",
        "competencies",
        "Vec<BundleEntry>",
        "a value of this crate",
    ),
    (
        "RoleProfile",
        "direction",
        "RoleDirection",
        "a closed vocabulary value",
    ),
    (
        "RoleProfile",
        "id",
        "RoleProfileId",
        "a value of this crate",
    ),
    ("RoleProfile", "label", "RoleLabel", "a value of this crate"),
    (
        "RoleProfile",
        "origin",
        "BundleOrigin",
        "a value of this crate",
    ),
    (
        "RoleProfile",
        "scope",
        "BundleScope",
        "a value of this crate",
    ),
    (
        "RoleProfile",
        "sources",
        "Vec<BundleSource>",
        "a value of this crate",
    ),
    (
        "RoleProfile",
        "valid_at",
        "RecordedOn",
        "a value of this crate",
    ),
    (
        "RoleProfile",
        "version",
        "RoleProfileVersion",
        "a value of this crate",
    ),
    (
        "RoleProfileRef",
        "profile",
        "RoleProfileId",
        "a value of this crate",
    ),
    (
        "RoleProfileRef",
        "version",
        "RoleProfileVersion",
        "a value of this crate",
    ),
    (
        "RoleProfileWire",
        "competencies",
        "Vec<BundleEntry>",
        "a value of this crate",
    ),
    (
        "RoleProfileWire",
        "direction",
        "RoleDirection",
        "a closed vocabulary value",
    ),
    (
        "RoleProfileWire",
        "id",
        "RoleProfileId",
        "a value of this crate",
    ),
    (
        "RoleProfileWire",
        "label",
        "RoleLabel",
        "a value of this crate",
    ),
    (
        "RoleProfileWire",
        "origin",
        "BundleOrigin",
        "a value of this crate",
    ),
    (
        "RoleProfileWire",
        "scope",
        "BundleScope",
        "a value of this crate",
    ),
    (
        "RoleProfileWire",
        "sources",
        "Vec<BundleSource>",
        "a value of this crate",
    ),
    (
        "RoleProfileWire",
        "valid_at",
        "RecordedOn",
        "a value of this crate",
    ),
    (
        "RoleProfileWire",
        "version",
        "RoleProfileVersion",
        "a value of this crate",
    ),
    (
        "UserAdjustment",
        "adjustment",
        "Adjustment",
        "a value of this crate",
    ),
    (
        "UserAdjustment",
        "because",
        "String",
        "the user's own words",
    ),
];

/// The tuple structs, which a named-field inventory has no key for.
const TUPLE_STRUCTS: [(&str, &str, &str); 6] = [
    (
        "BundleScope",
        "pub struct BundleScope(String);",
        "a caller-supplied identifier",
    ),
    (
        "DirectionName",
        "pub struct DirectionName(String);",
        "a caller-supplied identifier",
    ),
    (
        "RecordedOn",
        "pub struct RecordedOn(Date);",
        "a value of a reviewed crate, which is valid time rather than a clock reading",
    ),
    (
        "RoleLabel",
        "pub struct RoleLabel(String);",
        "the user's own words",
    ),
    (
        "RoleProfileId",
        "pub struct RoleProfileId(String);",
        "a caller-supplied identifier",
    ),
    (
        "RoleProfileVersion",
        "pub struct RoleProfileVersion(NonZeroU32);",
        "the registry's positive integer",
    ),
];

/// The enumerations with unnamed variants, for the same reason.
const TUPLE_ENUMS: [(&str, &str); 3] = [
    ("src/bundle.rs", "pub enum BundleOrigin {"),
    ("src/direction.rs", "pub enum RoleDirection {"),
    ("src/lib.rs", "pub enum RoleError {"),
];

/// One guarded name: what it is called, where it is called from and how often,
/// and why those sites are the ones it may have.
type GuardedName = (&'static str, &'static [(&'static str, usize)], &'static str);

/// The guarded names, each with the file it is called from and how often.
///
/// A name whose measured sites differ from this list — in count or in file —
/// fails, so a fourth door that skipped a check is visible here rather than in
/// a diff nobody read.
const CALL_SITE_COUNTS: [GuardedName; 4] = [
    (
        "checked",
        &[("crates/role-profile/src/lib.rs", 4)],
        "every door that makes a bundle runs the same entry-and-source check. Four, not three: \
         `declare`, `revise` and `fork` are the public ones, and `TryFrom<RoleProfileWire>` is the \
         fourth, because a document read back from JSON is not a way past the check",
    ),
    (
        "validated",
        &[
            ("crates/role-profile/src/bundle.rs", 1),
            ("crates/role-profile/src/direction.rs", 1),
            ("crates/role-profile/src/identity.rs", 1),
        ],
        "the identifier rule has one implementation and three callers, one per identifier type; a \
         fourth identifier type that checked itself would not appear here",
    ),
    (
        "non_empty",
        &[
            ("crates/role-profile/src/adjustment.rs", 1),
            ("crates/role-profile/src/bundle.rs", 1),
            ("crates/role-profile/src/identity.rs", 1),
        ],
        "the three places this crate holds the user's own prose, each refusing the empty one",
    ),
    (
        "rendered",
        &[
            ("crates/role-profile/src/adjustment.rs", 1),
            ("crates/role-profile/src/identity.rs", 1),
            ("crates/role-profile/src/lib.rs", 2),
            ("crates/role-profile/src/shelf.rs", 1),
        ],
        "section 24.2's `_v4` spelling is written for a reader and never compared: one call in \
         `Display` and four inside refusal messages, and none of them is a key, a lookup or an \
         equality",
    ),
];

// ---------------------------------------------------------------------------
// The scans.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let lib = source_of(&crate_root().join("src/lib.rs"))?;
    let declared: BTreeSet<String> = strip_non_code(&lib)
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod ").map(str::to_owned))
        .map(|name| name.trim_end_matches(';').to_owned())
        .collect();
    let read: BTreeSet<String> = crate_product_sources()?
        .iter()
        .filter_map(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
        .filter(|stem| stem != "lib")
        .collect();
    assert_eq!(
        read, declared,
        "the walk and lib.rs disagree about this crate's modules"
    );
    assert!(
        declared.len() >= 6,
        "this crate declares {} modules; the walk would be reading almost nothing",
        declared.len()
    );
    assert!(
        crate_product_sources()?.len() >= 7,
        "the product walk found too few files to be reading this package"
    );
    Ok(())
}

/// The three whole-set comparisons, and then the token pass.
#[test]
fn the_role_profile_crate_touches_no_file_and_no_socket() -> TestResult {
    let mut found: Vec<(String, String)> = Vec::new();
    for (file, _) in product_code()? {
        let source = fs::read_to_string(workspace_root().join(&file))?;
        let mut inside = false;
        let mut buffer = String::new();
        for line in source.lines() {
            let trimmed = line.trim();
            let opens = trimmed.starts_with("use ")
                || (trimmed.starts_with("pub") && trimmed.contains(" use "));
            if inside || opens {
                if inside {
                    buffer.push(' ');
                } else {
                    buffer.clear();
                }
                buffer.push_str(trimmed);
                inside = !trimmed.ends_with(';');
                if !inside {
                    found.push((file.clone(), collapse(&buffer)));
                }
            }
        }
    }
    let lib = "crates/role-profile/src/lib.rs";
    let expected: BTreeSet<(String, String)> = USE_ITEMS
        .iter()
        .chain(CRATE_IMPORTS.iter())
        .map(|(file, item)| ((*file).to_owned(), collapse(item)))
        .chain(
            RE_EXPORTS
                .iter()
                .map(|item| (lib.to_owned(), collapse(item))),
        )
        .collect();
    let found_set: BTreeSet<(String, String)> = found.into_iter().collect();
    assert_eq!(
        found_set, expected,
        "this crate's `use` set changed; a filesystem, transport or feed import is an extra key \
         here"
    );

    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        let body = without_use_items(&code);
        reached.extend(absolute_paths(&body));
        macros.extend(macros_spelled(&body));
    }
    assert_eq!(
        reached,
        REACHED_PATHS
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate reaches a path outside its inventory; every entry needs a reason"
    );
    assert_eq!(
        macros,
        MACROS_SPELLED
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate invokes a macro outside its inventory; an include_ macro reads a file"
    );

    for path in crate_all_sources()? {
        let code = strip_non_code(&source_of(&path)?);
        for forbidden in FORBIDDEN_CONSTRUCTS {
            let named = if forbidden.ends_with("::") {
                code.contains(forbidden)
            } else {
                uses_of(&code, forbidden) > 0
            };
            let permitted = READERS
                .iter()
                .any(|reader| relative(&path).ends_with(reader));
            assert!(
                permitted || !named,
                "{} spells {forbidden}",
                relative(&path)
            );
        }
    }
    Ok(())
}

/// Every declared field, in both directions, each saying what it holds.
#[test]
fn every_field_of_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (_, code) in product_code()? {
        for (owner, field, declared) in type_fields(&code) {
            found.insert((owner, field, declared));
        }
    }
    let expected: BTreeSet<(String, String, String)> = FIELDS
        .iter()
        .map(|(owner, field, declared, _)| {
            (
                (*owner).to_owned(),
                (*field).to_owned(),
                (*declared).to_owned(),
            )
        })
        .collect();
    assert_eq!(
        found, expected,
        "a field of this crate is not in the inventory; every field needs a line saying what it \
         holds, and a field holding a clock reading, a byte buffer or a feed handle does not get \
         one"
    );

    let admitted: BTreeSet<&str> = REASONS.into_iter().collect();
    for (owner, field, declared, reason) in FIELDS {
        assert!(
            admitted.contains(reason),
            "{owner}.{field} is described as {reason:?}, which is not one of the six"
        );
        assert!(
            !declared.contains("u8"),
            "{owner}.{field} is declared {declared}, which is a byte buffer"
        );
        assert!(
            !declared.contains("u64") && !declared.contains("Instant"),
            "{owner}.{field} is declared {declared}, which could be a clock reading"
        );
    }
    Ok(())
}

/// The unnamed fields, which [`type_fields`] has no name for, pinned whole.
#[test]
fn the_unnamed_fields_are_pinned() -> TestResult {
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        let mut cursor = 0;
        while let Some(at) = code[cursor..].find("struct ").map(|at| at + cursor) {
            cursor = at + "struct ".len();
            let name: String = code[cursor..]
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            if code[cursor + name.len()..].starts_with('(') {
                declared.insert(name);
            }
        }
    }
    assert_eq!(
        declared,
        TUPLE_STRUCTS
            .iter()
            .map(|(name, _, _)| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate declares a tuple struct the inventory does not name"
    );

    let mut all = String::new();
    for (_, code) in product_code()? {
        all.push_str(&collapse(&code));
        all.push(' ');
    }
    for (name, pin, reason) in TUPLE_STRUCTS {
        assert!(
            all.contains(pin),
            "{name} is not declared as {pin:?}; it holds {reason}"
        );
    }

    for (file, header) in TUPLE_ENUMS {
        let source = strip_non_code(&source_of(&crate_root().join(file))?);
        let block = whole_block(&source, header)?;
        assert!(
            block.len() > 40,
            "the pin on {header} extracted {} characters, which is not a declaration",
            block.len()
        );
        assert!(
            !block.contains("u8"),
            "{header} carries a byte buffer in an unnamed field"
        );
    }
    Ok(())
}

/// No public function edits a value in place.
#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    let mut mutating: Vec<String> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            if signature.contains("&mut self") {
                mutating.push(format!("{file}::{name}"));
            }
        }
    }
    assert_eq!(
        mutating,
        Vec::<String>::new(),
        "`CONTRIBUTING.md` rule 2 is append-only and a correction is a new event; a public \
         function that takes `&mut self` is an edit in place, and an edit to a bundle is a new \
         version"
    );
    Ok(())
}

/// `GATE-38-029` in the source: three doors make a bundle and none of them
/// ships one.
#[test]
fn the_only_producers_of_a_bundle_are_the_three_doors() -> TestResult {
    let mut producers: Vec<String> = Vec::new();
    for (_, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            let Some((_, returns)) = signature.split_once("->") else {
                continue;
            };
            let returns = collapse(returns);
            if uses_of(&returns, "RoleProfile") > 0 && !returns.contains('&') {
                producers.push(name);
            }
        }
    }
    producers.sort();
    assert_eq!(
        producers,
        vec!["declare".to_owned(), "fork".to_owned(), "revise".to_owned()],
        "a fourth public function produces a bundle. `GATE-38-029` is open: this build ships no \
         bundle for any direction, so the only bundles that exist are the ones a user declared, \
         revised or forked"
    );

    // The one producer that is not a public function is the deserializer, and
    // it runs the same check the three doors do —
    // `each_guarded_name_has_exactly_its_call_sites` counts the four calls.
    let lib = strip_non_code(&source_of(&crate_root().join("src/lib.rs"))?);
    assert!(
        lib.contains("impl TryFrom<RoleProfileWire> for RoleProfile {"),
        "the deserializer is no longer the fourth door"
    );

    // And the absence is stated rather than left silent.
    let direction = strip_non_code(&source_of(&crate_root().join("src/direction.rs"))?);
    assert!(
        direction.contains("pub const NO_SHIPPED_BUNDLES: &str ="),
        "the sentence naming what this build ships is gone"
    );
    assert!(
        source_of(&crate_root().join("src/direction.rs"))?.contains("GATE-38-029"),
        "the absence sentence no longer names the gate it is open on"
    );
    Ok(())
}

/// A favourite is not an argument to anything.
#[test]
fn an_interest_is_not_an_input_to_anything() -> TestResult {
    // `RoleInterest`'s own surface: four functions, and every one of them is
    // about the interest itself.
    let interest = strip_non_code(&source_of(&crate_root().join("src/interest.rs"))?);
    let own = raw_block(&interest, "impl RoleInterest {")?;
    let mut surface: Vec<String> = public_signatures(own)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    surface.sort();
    assert_eq!(
        surface,
        vec![
            "in_role".to_owned(),
            "profile".to_owned(),
            "standing".to_owned(),
            "standing_now".to_owned(),
        ],
        "`RoleInterest`'s own surface changed"
    );

    // And nothing else in the package names one, in a parameter or a return.
    // `impl RoleInterest`'s own functions say `Self`, so they are not in this
    // set and it is not narrowed by them.
    let mut naming: Vec<String> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            if uses_of(&collapse(&signature), "RoleInterest") > 0 {
                naming.push(format!("{file}::{name}"));
            }
        }
    }
    naming.sort();
    assert_eq!(
        naming,
        Vec::<String>::new(),
        "a public function takes or returns a favourite. Section 25.11: \
         `role을 즐겨찾기해도 진로 확정으로 간주하지 않는다`"
    );

    // The module reaches nothing but the lineage identity, so an interest has
    // no way to carry a competency, a weight or a plan.
    for absent in [
        "RoleProfile",
        "RoleProfileRef",
        "BundleEntry",
        "CompetencyId",
        "BundleShelf",
        "BundleImportance",
        "RecordedOn",
    ] {
        assert_eq!(
            uses_of(&interest, absent),
            0,
            "interest.rs names {absent}; a favourite would then carry something"
        );
    }
    assert!(
        uses_of(&interest, "RoleProfileId") > 0,
        "interest.rs no longer names the lineage it is about"
    );
    Ok(())
}

/// Section 24.2's `role 이름을 시장의 단일 진리로 두지 않는다`, in the source.
#[test]
fn a_label_reaches_no_direction_and_no_single_bundle() -> TestResult {
    let direction = strip_non_code(&source_of(&crate_root().join("src/direction.rs"))?);
    assert_eq!(
        uses_of(&direction, "RoleLabel"),
        0,
        "direction.rs names a label; reading `Backend Engineer` as a direction is the market \
         truth section 24.2 refuses"
    );

    let identity = strip_non_code(&source_of(&crate_root().join("src/identity.rs"))?);
    assert_eq!(
        uses_of(&identity, "RoleDirection"),
        0,
        "identity.rs names a direction; a label and a direction stay unconnected"
    );

    // A lookup by label returns a reading over a list, never one bundle.
    let shelf = strip_non_code(&source_of(&crate_root().join("src/shelf.rs"))?);
    let signature = public_signatures(&shelf)
        .into_iter()
        .find(|(name, _)| name == "by_label")
        .map(|(_, signature)| collapse(&signature))
        .ok_or("shelf.rs declares no `by_label`")?;
    assert_eq!(
        signature, "pub fn by_label(&self, label: &RoleLabel) -> LabelReading",
        "`by_label`'s signature changed; it must not be able to return one bundle"
    );
    for (name, signature) in public_signatures(&shelf) {
        let collapsed = collapse(&signature);
        if uses_of(&collapsed, "RoleLabel") > 0 {
            assert!(
                !collapsed.contains("Option<&RoleProfile>"),
                "{name} resolves a label to one bundle"
            );
        }
    }
    Ok(())
}

/// The identity is a pair, and its rendering has no inverse.
#[test]
fn the_identity_is_a_pair_and_the_rendering_has_no_inverse() -> TestResult {
    let identity = strip_non_code(&source_of(&crate_root().join("src/identity.rs"))?);

    let declaration = whole_block(&identity, "pub struct RoleProfileRef {")?;
    assert_eq!(
        declaration,
        "pub struct RoleProfileRef { profile: RoleProfileId, version: RoleProfileVersion, }",
        "the identity is no longer the lineage-and-version pair"
    );

    let rendered = whole_block(&identity, "pub fn rendered(&self) -> String {")?;
    assert_eq!(
        rendered,
        "pub fn rendered(&self) -> String { format!( , self.profile.as_str(), self.version.get()) }",
        "`RoleProfileRef::rendered` changed. The pin is over stripped code, so the format string \
         is not in it; the spelling itself is pinned by \
         `an_identity_is_a_pair_and_not_a_rendered_name`"
    );

    // Nothing reads a rendered name back. There is no parser, and no `TryFrom`
    // or `FromStr` on the pair.
    assert_eq!(
        uses_of(&identity, "FromStr"),
        0,
        "the rendered spelling has grown a parser"
    );
    for (_, code) in product_code()? {
        assert!(
            !code.contains("impl TryFrom<String> for RoleProfileRef"),
            "the rendered spelling can be read back as an identity"
        );
    }

    // A version is a positive integer with one door, and zero is refused at it.
    assert!(
        identity.contains("pub struct RoleProfileVersion(NonZeroU32);"),
        "the version is no longer a non-zero integer"
    );
    assert!(
        identity.contains("None => Err(RoleError::VersionIsNotPositive),"),
        "the zero arm no longer refuses"
    );
    Ok(())
}

/// The identifier rule is a whole-set byte classification.
#[test]
fn the_identifier_rule_is_executed_over_every_byte() -> TestResult {
    let identity = strip_non_code(&source_of(&crate_root().join("src/identity.rs"))?);
    let validated = whole_block(
        &identity,
        "pub(crate) fn validated(value: String, what: &'static str) -> Result<String, RoleError> {",
    )?;
    assert!(
        validated.contains(".bytes() .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, "),
        "`validated` no longer classifies every byte: {validated}"
    );
    assert!(
        validated.contains("value.len() <= MAX_IDENTIFIER"),
        "`validated` no longer bounds the length"
    );
    assert!(
        validated.contains("!value.is_empty()"),
        "`validated` no longer refuses the empty identifier"
    );
    assert!(
        !validated.contains("contains"),
        "`validated` searches for listed characters instead of classifying every byte"
    );
    Ok(())
}

/// The guarded names, each with its counted call sites in each file.
#[test]
fn each_guarded_name_has_exactly_its_call_sites() -> TestResult {
    for (name, expected, reason) in CALL_SITE_COUNTS {
        let mut measured: Vec<(String, usize)> = Vec::new();
        for (path, code) in product_code()? {
            // `use` items are dropped first, so what is counted is a call site
            // rather than an import that names the same thing.
            let calls = calls_of(&without_use_items(&code), name);
            if calls > 0 {
                measured.push((path, calls));
            }
        }
        measured.sort();
        let pinned: Vec<(String, usize)> = expected
            .iter()
            .map(|(file, calls)| ((*file).to_owned(), *calls))
            .collect();
        assert_eq!(measured, pinned, "{name}'s call sites moved: {reason}");
    }
    Ok(())
}

/// Both files of this package that read anything are on the inventory page.
#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for name in [
        "crates/role-profile/tests/role_scans.rs",
        "crates/role-profile/tests/role_bundles.rs",
    ] {
        assert!(
            page.contains(name),
            "{name} reads Rust source or the design document and is not on the inventory page"
        );
    }
    Ok(())
}

/// The helpers this file's claims rest on, exercised against known answers.
#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(collapse("  a   b \n c "), "a b c");
    assert_eq!(uses_of("a_fs_b fs fs::x", "fs"), 2);
    assert_eq!(uses_of("no match here", "fs"), 0);
    assert_eq!(uses_of("RoleProfileId RoleProfile", "RoleProfile"), 1);
    assert_eq!(
        absolute_paths("thiserror::Error and std::mem::swap"),
        ["std::mem".to_owned(), "thiserror::Error".to_owned()]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        macros_spelled("matches!(a, b) and include_str!(\"x\")"),
        ["include_str".to_owned(), "matches".to_owned()]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        type_fields("pub struct A { pub b: Vec<u8>, c: u64, }"),
        vec![
            ("A".to_owned(), "b".to_owned(), "Vec<u8>".to_owned()),
            ("A".to_owned(), "c".to_owned(), "u64".to_owned()),
        ]
    );
    assert_eq!(
        calls_of("x.shelve(&a); shelve(b); shelved(c);", "shelve"),
        2
    );
    assert_eq!(declarations_of("fn shelve(x: u32) {}", "shelve"), 1);
    assert!(whole_block("impl A { fn b() {} }", "impl A {").is_ok());
    assert!(whole_block("impl A { fn b() {} }", "impl Z {").is_err());
    assert_eq!(
        public_signatures("pub fn a(x: u8) -> u8 {\n").len(),
        1,
        "the signature extractor finds a signature it is given"
    );

    // The forbidden-token controls: the pass really does find a name when one
    // is there, so the zeros it reports over this crate are measurements.
    assert!(uses_of("let file = File::open(p);", "File") > 0);
    assert!(uses_of("let t = SystemTime::now();", "now") > 0);
    assert!(uses_of("let s = TcpStream::connect(a);", "TcpStream") > 0);

    // And the walk really reads this package rather than an empty directory.
    assert!(crate_product_sources()?.len() >= 7);
    assert!(crate_all_sources()?.len() >= 9);
    assert!(!product_code()?.is_empty());
    // A qualified path is a leading `::` however it is spelled. `tighten` glues
    // the space in `<T as ::std::net::X>` shut, and deciding on the byte before
    // the `::` then read the crate root as a middle segment: `P2-A5` measured a
    // name resolved from a live function with this pass reporting nothing.
    assert!(
        absolute_paths("let _ = <str as ::std::net::ToSocketAddrs>::to_socket_addrs(h);")
            .contains("std::net")
    );
    assert!(absolute_paths("let _: &dyn ::core::fmt::Debug = &v;").contains("core::fmt"));
    // The other direction, so the repair is not "every segment is a root": a
    // real middle segment still yields no second key.
    assert!(!absolute_paths("std::alloc::Layout::new::<u8>()").contains("alloc::Layout"));
    Ok(())
}
