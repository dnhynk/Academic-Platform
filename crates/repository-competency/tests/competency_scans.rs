//! What `academic-repository-competency` may reach, hold and hand out.
//!
//! ## Why a token list is not enough, measured three tasks ago
//!
//! `P2-R2` shipped this crate's great-grandparent with a forbidden-token list
//! as its only net, and `docs/contracts/policy-source-scans.md` records what
//! that measured: seven spellings of a filesystem or environment reach compile,
//! spell none of the listed tokens, add no `use` item, and passed. The repair
//! was not a longer list. It was three **whole-set** comparisons, in both
//! directions:
//!
//! * every `use` item ([`USE_ITEMS`]);
//! * every two-segment path reached through a crate root ([`REACHED_PATHS`]);
//! * every macro invoked ([`MACROS_SPELLED`]).
//!
//! [`FORBIDDEN_CONSTRUCTS`] is kept as the third and weakest layer, because it
//! names the shapes a reader expects to see refused.
//!
//! ## The same defect class, one step out
//!
//! `tools/secret-debug-policy.test.mjs` decides whether a field holds something
//! a `Debug` must not print by matching the **field's name** against a fixed
//! alternation. A field holding the same bytes under a name outside that
//! alternation is invisible to it. **That tool passing this crate is not
//! evidence about this crate.** What is evidence is [`FIELDS`] and the arrays
//! beside it: every field of every type this crate declares, compared in both
//! directions, each entry carrying what it holds.
//!
//! ## What this crate is allowed to hold
//!
//! Seven things, and the last column of the inventory is which one:
//!
//! * a **caller-supplied identifier** --- a user, a change, a rubric, a
//!   concept, admitted by `identity::validated`;
//! * a **system-derived identifier** --- a snapshot identifier, which
//!   `academic-repository` minted, and a claim identity, which is a digest;
//! * an **external identity value** --- what a version-control system or a
//!   forge calls a person. Bounded in length and **not** put through
//!   `validated`, because an address holds `@` and a display name holds spaces;
//! * a **closed vocabulary value** --- one of this crate's or another reviewed
//!   crate's enumerations;
//! * a **value of a reviewed crate** --- a `Locator`, a `ClassificationKey`;
//! * a **value of this crate**; and
//! * a **count, revision or timestamp**.
//!
//! There is an eighth thing this crate holds that `P2-R4` does not, and it is
//! called out rather than folded in: **the user's own words**. A warrant's note
//! and its explanation are prose the user wrote about generated code, and
//! section 17.6 asks for them by name. They are inventoried as
//! `the user's own words` and no `Debug` here reduces them to a length, because
//! a warrant a reader cannot read is not evidence of anything.
//!
//! There is no ninth, and in particular there is no byte buffer: no field of
//! this crate is declared `Vec<u8>` or `[u8; N]` under any name, which is what
//! the inventory says without consulting a list of names.

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
fn whole_block(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
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
    Ok(collapse(&source[start..=at]))
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

/// Collapses whitespace so a rewrapped signature still matches the pin that
/// it closes.
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
/// [`the_helpers_are_not_vacuous`] requires this crate to declare none, so the
/// whole-set claim is not quietly narrowed by one.
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
            // Skip a lifetime or generic parameter list, then require a body.
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
            // An enum struct-variant. Its name is what comes before the brace,
            // less any attribute or doc line the stripper left behind.
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

/// Every `use` item of every product file, file by file.
const USE_ITEMS: [(&str, &str); 12] = [
    (
        "crates/repository-competency/src/claim.rs",
        "use academic_policy::ContentDigest;",
    ),
    (
        "crates/repository-competency/src/claim.rs",
        "use academic_repository_analysis::{ArtifactScope, EvidenceTier, LadderRung, Locator};",
    ),
    (
        "crates/repository-competency/src/claim.rs",
        "use academic_repository_classification::{ClassificationKey, ObservedProof};",
    ),
    (
        "crates/repository-competency/src/claim.rs",
        "use academic_repository_correlation::EvidenceRelation;",
    ),
    (
        "crates/repository-competency/src/claim.rs",
        "use academic_domain::MasteryLevel;",
    ),
    (
        "crates/repository-competency/src/claim.rs",
        "use crate::{ CompetencyError, contribution::{AuthoredWork, AuthorshipMode, ChangeId}, \
         generated::CodeOrigin, identity::{ExternalAuthorId, UserId}, \
         outcome::{CandidateSupport, OutcomeArtifact, OutcomeKind}, \
         rubric::{ChangedSite, RubricId}, };",
    ),
    (
        "crates/repository-competency/src/contribution.rs",
        "use academic_repository_analysis::Locator;",
    ),
    (
        "crates/repository-competency/src/contribution.rs",
        "use crate::{ CompetencyError, generated::{CodeOrigin, GeneratedCodeWarrant, \
         OriginReport, WarrantStep}, identity::{AuthorshipMap, ExternalAuthorId, UserId, \
         validated}, rubric::{ChangeVerdict, ChangedSite, ScaffoldRubric}, };",
    ),
    (
        "crates/repository-competency/src/generated.rs",
        "use academic_repository_analysis::Locator;",
    ),
    (
        "crates/repository-competency/src/generated.rs",
        "use crate::{CompetencyError, rubric::ChangedSite};",
    ),
    (
        "crates/repository-competency/src/outcome.rs",
        "use academic_repository_analysis::Locator;",
    ),
    (
        "crates/repository-competency/src/outcome.rs",
        "use crate::{CompetencyError, contribution::ChangeId};",
    ),
];

/// The two files that reach `std`, and for a `BTreeSet` and nothing else.
const COLLECTION_IMPORTS: [(&str, &str); 4] = [
    (
        "crates/repository-competency/src/identity.rs",
        "use std::collections::BTreeSet;",
    ),
    (
        "crates/repository-competency/src/identity.rs",
        "use crate::CompetencyError;",
    ),
    (
        "crates/repository-competency/src/rubric.rs",
        "use std::collections::BTreeSet;",
    ),
    (
        "crates/repository-competency/src/rubric.rs",
        "use academic_repository_analysis::{Locator, PathClass};",
    ),
];

/// `rubric.rs`'s remaining import, kept separate so the two above read as a pair.
const RUBRIC_IMPORT: &str = "use crate::{CompetencyError, identity::validated};";

/// The crate root's own imports.
const LIB_IMPORTS: [&str; 1] =
    ["use academic_repository_classification::{ClassificationSet, ConceptStance};"];

/// The crate root's re-exports.
const RE_EXPORTS: [&str; 6] = [
    "pub use claim::{ ClaimId, ClaimStanding, PersonalApplicationClaim, PersonalProvenance, \
     ProjectObservationClaim, ProjectProvenance, RejectionReason, };",
    "pub use contribution::{ AuthoredWork, AuthorshipMode, ChangeId, ContributionDraft, \
     ContributionKind, ContributionRecord, };",
    "pub use generated::{ CodeOrigin, ExplainedByUser, GeneratedCodeWarrant, ModifiedByUser, \
     OriginReport, VerifiedByUser, WarrantStep, };",
    "pub use identity::{AuthorshipMap, ExternalAuthorId, IdentitySource, UserId};",
    "pub use outcome::{CandidateSupport, OutcomeArtifact, OutcomeKind};",
    "pub use rubric::{ChangeKind, ChangeVerdict, ChangedSite, RubricId, ScaffoldRubric};",
];

/// Every two-segment path this crate reaches through a crate root.
///
/// One, and it is a derive. A filesystem or transport reach written as an
/// absolute path — the shape a token list missed in `P2-R2` — would be an extra
/// key here whatever it were called.
const REACHED_PATHS: [(&str, &str); 1] = [(
    "thiserror::Error",
    "the derive on CompetencyError, which is this crate's one macro dependency",
)];

/// Every macro this crate invokes.
///
/// One, and it is `matches!`. An `include_str!` reads a file at compile time and
/// spells no `fs` name at all, which is why the macro set is compared rather
/// than searched.
const MACROS_SPELLED: [(&str, &str); 1] =
    [("matches", "the byte-class test in identity::validated")];

/// The files of this package that read a file.
///
/// Two, and neither is product source: this scan, which walks the package, and
/// the acceptance suite, which reads the design document so that section 17.6's
/// bullets and section 13.2's ceilings are measured rather than restated. Both
/// are named on `docs/contracts/policy-source-scans.md`.
const READERS: [&str; 2] = ["competency_scans.rs", "competency_lanes.rs"];

/// The shapes a reader expects to see refused, as the third and weakest layer.
const FORBIDDEN_CONSTRUCTS: [&str; 11] = [
    "fs::",
    "File",
    "OpenOptions",
    "Command",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "UnixStream",
    "socket",
    "env",
    "include_str",
];

/// The seven reasons a field may exist, plus the eighth this crate adds.
const REASONS: [&str; 8] = [
    "a caller-supplied identifier",
    "a system-derived identifier",
    "an external identity value",
    "a closed vocabulary value",
    "a value of a reviewed crate",
    "a value of this crate",
    "a count, revision or timestamp",
    "the user's own words",
];

/// The names whose call sites are counted, with the count and the one file.
const CALL_SITE_COUNTS: [(&str, usize, &str, &str); 4] = [
    (
        "authorship_mode",
        1,
        "crates/repository-competency/src/contribution.rs",
        "the one door from a ContributionKind to an AuthorshipMode; a second caller would be a \
         second table",
    ),
    (
        "resolve",
        1,
        "crates/repository-competency/src/contribution.rs",
        "the one place the authorship mapping is consulted",
    ),
    (
        "judge",
        1,
        "crates/repository-competency/src/contribution.rs",
        "the one place the scaffold rubric decides",
    ),
    (
        "touches",
        1,
        "crates/repository-competency/src/lib.rs",
        "the one place a work is joined to an observation",
    ),
];

const FIELDS: [(&str, &str, &str, &str); 34] = [
    (
        "ClaimId",
        "identifier",
        "String",
        "a system-derived identifier",
    ),
    (
        "ClaimStanding::Rejected",
        "at",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "ClaimStanding::Rejected",
        "reason",
        "RejectionReason",
        "a closed vocabulary value",
    ),
    (
        "PersonalApplicationClaim",
        "id",
        "ClaimId",
        "a value of this crate",
    ),
    (
        "PersonalApplicationClaim",
        "key",
        "ClassificationKey",
        "a value of a reviewed crate",
    ),
    (
        "PersonalApplicationClaim",
        "provenance",
        "PersonalProvenance",
        "a value of this crate",
    ),
    (
        "PersonalApplicationClaim",
        "standing",
        "ClaimStanding",
        "a value of this crate",
    ),
    (
        "PersonalApplicationClaim",
        "support",
        "CandidateSupport",
        "a closed vocabulary value",
    ),
    (
        "PersonalProvenance",
        "author",
        "ExternalAuthorId",
        "a value of this crate",
    ),
    (
        "PersonalProvenance",
        "bearing_sites",
        "Vec<ChangedSite>",
        "a value of this crate",
    ),
    (
        "PersonalProvenance",
        "change",
        "ChangeId",
        "a value of this crate",
    ),
    (
        "PersonalProvenance",
        "mapping_version",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "PersonalProvenance",
        "mode",
        "AuthorshipMode",
        "a closed vocabulary value",
    ),
    (
        "PersonalProvenance",
        "observed_by",
        "ClaimId",
        "a value of this crate",
    ),
    (
        "PersonalProvenance",
        "origin",
        "CodeOrigin",
        "a value of this crate",
    ),
    (
        "PersonalProvenance",
        "outcomes",
        "Vec<OutcomeKind>",
        "a closed vocabulary value",
    ),
    (
        "PersonalProvenance",
        "rubric",
        "RubricId",
        "a value of this crate",
    ),
    (
        "PersonalProvenance",
        "rubric_version",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "PersonalProvenance",
        "user",
        "UserId",
        "a value of this crate",
    ),
    (
        "ProjectObservationClaim",
        "id",
        "ClaimId",
        "a value of this crate",
    ),
    (
        "ProjectObservationClaim",
        "key",
        "ClassificationKey",
        "a value of a reviewed crate",
    ),
    (
        "ProjectObservationClaim",
        "provenance",
        "ProjectProvenance",
        "a value of this crate",
    ),
    (
        "ProjectProvenance",
        "artifact_scope",
        "ArtifactScope",
        "a closed vocabulary value",
    ),
    (
        "ProjectProvenance",
        "locators",
        "Vec<Locator>",
        "a value of a reviewed crate",
    ),
    (
        "ProjectProvenance",
        "relation",
        "EvidenceRelation",
        "a closed vocabulary value",
    ),
    (
        "ProjectProvenance",
        "rung",
        "LadderRung",
        "a closed vocabulary value",
    ),
    (
        "ProjectProvenance",
        "tier",
        "EvidenceTier",
        "a closed vocabulary value",
    ),
    (
        "AuthoredWork",
        "author",
        "ExternalAuthorId",
        "a value of this crate",
    ),
    (
        "AuthoredWork",
        "change",
        "ChangeId",
        "a value of this crate",
    ),
    (
        "AuthoredWork",
        "mapping_version",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "AuthoredWork",
        "mode",
        "AuthorshipMode",
        "a closed vocabulary value",
    ),
    (
        "AuthoredWork",
        "origin",
        "CodeOrigin",
        "a value of this crate",
    ),
    (
        "AuthoredWork",
        "recorded_at",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "AuthoredWork",
        "snapshot_id",
        "String",
        "a system-derived identifier",
    ),
];

const MORE_FIELDS: [(&str, &str, &str, &str); 34] = [
    ("AuthoredWork", "user", "UserId", "a value of this crate"),
    (
        "AuthoredWork",
        "verdict",
        "ChangeVerdict",
        "a value of this crate",
    ),
    (
        "ChangeId",
        "identifier",
        "String",
        "a caller-supplied identifier",
    ),
    (
        "ContributionDraft",
        "map",
        "&'aAuthorshipMap",
        "a value of this crate",
    ),
    (
        "ContributionDraft",
        "record",
        "&'aContributionRecord",
        "a value of this crate",
    ),
    (
        "ContributionDraft",
        "rubric",
        "&'aScaffoldRubric",
        "a value of this crate",
    ),
    (
        "ContributionDraft",
        "warrant",
        "Option<GeneratedCodeWarrant>",
        "a value of this crate",
    ),
    (
        "ContributionRecord",
        "author",
        "ExternalAuthorId",
        "a value of this crate",
    ),
    (
        "ContributionRecord",
        "change",
        "ChangeId",
        "a value of this crate",
    ),
    (
        "ContributionRecord",
        "kind",
        "ContributionKind",
        "a closed vocabulary value",
    ),
    (
        "ContributionRecord",
        "origin",
        "OriginReport",
        "a closed vocabulary value",
    ),
    (
        "ContributionRecord",
        "recorded_at",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "ContributionRecord",
        "sites",
        "Vec<ChangedSite>",
        "a value of this crate",
    ),
    (
        "ContributionRecord",
        "snapshot_id",
        "String",
        "a system-derived identifier",
    ),
    (
        "ExplainedByUser",
        "explanation",
        "String",
        "the user's own words",
    ),
    (
        "ExplainedByUser",
        "modified",
        "ModifiedByUser",
        "a value of this crate",
    ),
    (
        "GeneratedCodeWarrant",
        "explained",
        "ExplainedByUser",
        "a value of this crate",
    ),
    (
        "ModifiedByUser",
        "edits",
        "Vec<ChangedSite>",
        "a value of this crate",
    ),
    (
        "ModifiedByUser",
        "verified",
        "VerifiedByUser",
        "a value of this crate",
    ),
    (
        "VerifiedByUser",
        "at",
        "Vec<Locator>",
        "a value of a reviewed crate",
    ),
    ("VerifiedByUser", "note", "String", "the user's own words"),
    (
        "AuthorshipMap",
        "identities",
        "BTreeSet<ExternalAuthorId>",
        "a value of this crate",
    ),
    ("AuthorshipMap", "user", "UserId", "a value of this crate"),
    (
        "AuthorshipMap",
        "version",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "ExternalAuthorId",
        "source",
        "IdentitySource",
        "a closed vocabulary value",
    ),
    (
        "ExternalAuthorId",
        "value",
        "String",
        "an external identity value",
    ),
    (
        "UserId",
        "identifier",
        "String",
        "a caller-supplied identifier",
    ),
    (
        "CompetencyError::AuthorIsNotTheUser",
        "change",
        "String",
        "a caller-supplied identifier",
    ),
    (
        "CompetencyError::AuthorIsNotTheUser",
        "mapping_version",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "CompetencyError::AuthorIsNotTheUser",
        "namespace",
        "&'staticstr",
        "a closed vocabulary value",
    ),
    (
        "CompetencyError::ChangeIsScaffoldOnly",
        "bearing_sites",
        "u32",
        "a count, revision or timestamp",
    ),
    (
        "CompetencyError::ChangeIsScaffoldOnly",
        "change",
        "String",
        "a caller-supplied identifier",
    ),
    (
        "CompetencyError::ChangeIsScaffoldOnly",
        "required",
        "u32",
        "a count, revision or timestamp",
    ),
    (
        "CompetencyError::ChangeIsScaffoldOnly",
        "rubric",
        "String",
        "a caller-supplied identifier",
    ),
];

const LAST_FIELDS: [(&str, &str, &str, &str); 32] = [
    (
        "CompetencyError::ChangeIsScaffoldOnly",
        "version",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "CompetencyError::ContributionIsNotAuthorship",
        "change",
        "String",
        "a caller-supplied identifier",
    ),
    (
        "CompetencyError::ContributionIsNotAuthorship",
        "kind",
        "ContributionKind",
        "a closed vocabulary value",
    ),
    (
        "CompetencyError::GeneratedCodeHasNoWarrant",
        "change",
        "String",
        "a caller-supplied identifier",
    ),
    (
        "CompetencyError::GeneratedCodeHasNoWarrant",
        "first_missing",
        "WarrantStep",
        "a closed vocabulary value",
    ),
    (
        "PromotionInput",
        "classification",
        "&'aClassificationSet",
        "a value of a reviewed crate",
    ),
    (
        "PromotionInput",
        "outcomes",
        "&'a[OutcomeArtifact]",
        "a value of this crate",
    ),
    (
        "PromotionInput",
        "user",
        "&'aUserId",
        "a value of this crate",
    ),
    (
        "PromotionInput",
        "works",
        "&'a[AuthoredWork]",
        "a value of this crate",
    ),
    (
        "PromotionSet",
        "personal",
        "Vec<PersonalApplicationClaim>",
        "a value of this crate",
    ),
    (
        "PromotionSet",
        "project",
        "Vec<ProjectObservationClaim>",
        "a value of this crate",
    ),
    (
        "PromotionSet",
        "snapshot_id",
        "String",
        "a system-derived identifier",
    ),
    (
        "OutcomeArtifact",
        "at",
        "Locator",
        "a value of a reviewed crate",
    ),
    (
        "OutcomeArtifact",
        "change",
        "ChangeId",
        "a value of this crate",
    ),
    (
        "OutcomeArtifact",
        "concept",
        "String",
        "a caller-supplied identifier",
    ),
    (
        "OutcomeArtifact",
        "kind",
        "OutcomeKind",
        "a closed vocabulary value",
    ),
    (
        "OutcomeArtifact",
        "recorded_at",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "ChangeVerdict::Meaningful",
        "bearing_sites",
        "Vec<ChangedSite>",
        "a value of this crate",
    ),
    (
        "ChangeVerdict::Meaningful",
        "rubric",
        "RubricId",
        "a value of this crate",
    ),
    (
        "ChangeVerdict::Meaningful",
        "version",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "ChangeVerdict::ScaffoldOnly",
        "bearing_sites",
        "u32",
        "a count, revision or timestamp",
    ),
    (
        "ChangeVerdict::ScaffoldOnly",
        "required",
        "u32",
        "a count, revision or timestamp",
    ),
    (
        "ChangeVerdict::ScaffoldOnly",
        "rubric",
        "RubricId",
        "a value of this crate",
    ),
    (
        "ChangeVerdict::ScaffoldOnly",
        "version",
        "u64",
        "a count, revision or timestamp",
    ),
    (
        "ChangedSite",
        "kind",
        "ChangeKind",
        "a closed vocabulary value",
    ),
    (
        "ChangedSite",
        "locator",
        "Locator",
        "a value of a reviewed crate",
    ),
    (
        "RubricId",
        "identifier",
        "String",
        "a caller-supplied identifier",
    ),
    ("ScaffoldRubric", "id", "RubricId", "a value of this crate"),
    (
        "ScaffoldRubric",
        "minimum_bearing_sites",
        "u32",
        "a count, revision or timestamp",
    ),
    (
        "ScaffoldRubric",
        "scaffold_change_kinds",
        "BTreeSet<ChangeKind>",
        "a closed vocabulary value",
    ),
    (
        "ScaffoldRubric",
        "scaffold_path_classes",
        "BTreeSet<PathClass>",
        "a closed vocabulary value",
    ),
    (
        "ScaffoldRubric",
        "version",
        "u64",
        "a count, revision or timestamp",
    ),
];

// ---------------------------------------------------------------------------
// The scans.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 9,
        "the walk found only {} files under the package",
        sources.len()
    );

    let root = crate_root();
    let outside: Vec<String> = crate_product_sources()?
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
            read.insert(stem.to_string_lossy().into_owned());
        }
    }

    let mut declared = 0_usize;
    for path in &sources {
        let source = source_of(path)?;
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
            assert!(
                !trimmed.starts_with("#[path"),
                "{} pulls in a file by path; the walk does not follow one",
                relative(path)
            );
        }
    }
    assert!(
        declared >= 6,
        "the tripwire read only {declared} module declarations"
    );
    Ok(())
}

#[test]
fn the_competency_crate_touches_no_file_and_no_socket() -> TestResult {
    // The whole set of `use` items, both directions.
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
    let mut expected: Vec<(String, String)> = USE_ITEMS
        .iter()
        .chain(COLLECTION_IMPORTS.iter())
        .map(|(file, item)| ((*file).to_owned(), collapse(item)))
        .collect();
    expected.push((
        "crates/repository-competency/src/rubric.rs".to_owned(),
        RUBRIC_IMPORT.to_owned(),
    ));
    let lib = "crates/repository-competency/src/lib.rs";
    for item in LIB_IMPORTS {
        expected.push((lib.to_owned(), item.to_owned()));
    }
    for item in RE_EXPORTS {
        expected.push((lib.to_owned(), collapse(item)));
    }
    let found_set: BTreeSet<(String, String)> = found.iter().cloned().collect();
    let expected_set: BTreeSet<(String, String)> = expected.into_iter().collect();
    assert_eq!(
        found_set, expected_set,
        "this crate's `use` set changed; a filesystem or transport import is an extra key here"
    );

    // The whole set of paths reached through a crate root, both directions.
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

    // And no `fs::`, no `Command`, no socket construct, anywhere in the package
    // -- tests included, because a test that opened a file would make the crate
    // documentation's claim about the whole package false.
    for path in crate_all_sources()? {
        let code = strip_non_code(&source_of(&path)?);
        for forbidden in FORBIDDEN_CONSTRUCTS {
            // A needle ending in `::` is a path prefix and is matched as a
            // substring; every other one is an identifier and is matched whole,
            // so this file's own test name — which ends in `no_socket` — is not
            // read as a socket.
            let named = if forbidden.ends_with("::") {
                code.contains(forbidden)
            } else {
                uses_of(&code, forbidden) > 0
            };
            let permitted = (forbidden == "fs::" || forbidden == "env")
                && READERS
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

#[test]
fn every_field_of_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (_, code) in product_code()? {
        for (owner, field, declared) in type_fields(&code) {
            found.insert((owner, field, declared));
        }
    }
    let expected: BTreeSet<(String, String, String)> = inventory()
        .into_iter()
        .map(|(owner, field, declared, _)| {
            (owner.to_owned(), field.to_owned(), declared.to_owned())
        })
        .collect();
    assert_eq!(
        found, expected,
        "a field of this crate is not in the inventory; every field needs a line saying what it \
         holds, and a field holding repository text does not get one"
    );

    // Every reason is one of the eight the module documentation names, so the
    // column is a classification rather than free prose.
    let admitted: BTreeSet<&str> = REASONS.into_iter().collect();
    for (owner, field, _, reason) in inventory() {
        assert!(
            admitted.contains(reason),
            "{owner}.{field} is described as {reason:?}, which is not one of the eight"
        );
    }

    // And no field of this crate is a byte buffer under any name. That is the
    // claim `tools/secret-debug-policy.test.mjs` answers with a name
    // alternation; here it is answered over the declared type of every field.
    for (owner, field, declared, _) in inventory() {
        assert!(
            !declared.contains("u8"),
            "{owner}.{field} is declared {declared}, which is a byte buffer"
        );
    }

    // The user's own words are exactly two fields and both are a warrant's.
    // Section 17.6 asks for them by name, so they are inventoried rather than
    // refused — and the set is pinned so a third place for prose is visible.
    let words: Vec<String> = inventory()
        .into_iter()
        .filter(|(_, _, _, reason)| *reason == "the user's own words")
        .map(|(owner, field, _, _)| format!("{owner}.{field}"))
        .collect();
    assert_eq!(
        words,
        vec![
            "ExplainedByUser.explanation".to_owned(),
            "VerifiedByUser.note".to_owned(),
        ],
        "the places this crate holds the user's own prose have changed"
    );
    Ok(())
}

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
         function that takes `&mut self` is an edit in place"
    );
    Ok(())
}

#[test]
fn the_rubric_is_configuration_and_not_a_constant() -> TestResult {
    let rubric = source_of(&crate_root().join("src/rubric.rs"))?;
    let code = strip_non_code(&rubric);

    // No `Default`, so there is no rubric a caller can get without writing one.
    assert_eq!(
        uses_of(&code, "Default"),
        0,
        "ScaffoldRubric has a Default; a rubric nobody chose is a threshold compiled in"
    );

    // The judgement compares against fields and against no literal. A numeric
    // literal in `judge` would be exactly the shape section 17.6's rubric may
    // not have: a decision made where nobody can read it.
    let judging = whole_block(
        &rubric,
        "    pub fn judge(&self, sites: &[ChangedSite]) -> ChangeVerdict {",
    )?;
    // A digit inside an identifier — the `32` of `u32` — is a type and not a
    // threshold, so what is looked for is a digit that starts a token.
    let literals: Vec<char> = judging
        .char_indices()
        .filter(|(at, character)| {
            character.is_ascii_digit()
                && !judging[..*at]
                    .chars()
                    .next_back()
                    .is_some_and(|previous| previous.is_alphanumeric() || previous == '_')
        })
        .map(|(_, character)| character)
        .collect();
    assert!(
        literals.is_empty(),
        "`judge` holds the numeric literals {literals:?}; a rubric's thresholds are its fields"
    );
    assert!(judging.contains("self.minimum_bearing_sites"));
    assert!(judging.contains("bears_understanding"));

    // Every part of the rubric is a constructor argument. A part added to the
    // type and not to `of` would be a part no caller can set — a constant with
    // extra steps.
    let parts: Vec<String> = inventory()
        .into_iter()
        .filter(|(owner, _, _, _)| *owner == "ScaffoldRubric")
        .map(|(_, field, _, _)| field.to_owned())
        .collect();
    assert_eq!(
        parts,
        vec![
            "id".to_owned(),
            "minimum_bearing_sites".to_owned(),
            "scaffold_change_kinds".to_owned(),
            "scaffold_path_classes".to_owned(),
            "version".to_owned(),
        ],
        "a rubric part is not in the inventory"
    );
    let constructor = whole_block(&rubric, "    pub fn of(")?;
    for part in &parts {
        assert!(
            constructor.contains(part.as_str()),
            "`ScaffoldRubric::of` does not take {part}; that part cannot be configured"
        );
    }

    // And the contract page states the parts, so the rubric is not a decision
    // made only in code. `S-17` is the shape this avoids one page over: a list
    // written from an authoritative source with nothing comparing the two.
    let page =
        fs::read_to_string(workspace_root().join("docs/contracts/repository-competency.md"))?;
    for part in &parts {
        assert!(
            page.contains(part.as_str()),
            "`docs/contracts/repository-competency.md` does not name the rubric part {part}"
        );
    }
    Ok(())
}

#[test]
fn each_guarded_name_has_exactly_its_call_sites() -> TestResult {
    let code: Vec<(String, String)> = product_code()?;
    let joined = code
        .iter()
        .map(|(_, body)| body.clone())
        .collect::<Vec<_>>()
        .join(" ");
    for (name, expected, file, reason) in CALL_SITE_COUNTS {
        let mut total = 0_usize;
        let mut sites: Vec<String> = Vec::new();
        for (path, body) in &code {
            let calls = calls_of(body, name);
            if calls > 0 {
                sites.push(path.clone());
            }
            total += calls;
        }
        assert_eq!(
            total, expected,
            "{name} has {total} call sites, not {expected}: {reason}"
        );
        assert_eq!(
            sites,
            vec![file.to_owned()],
            "{name} is called outside {file}"
        );
        assert_eq!(
            declarations_of(&joined, name),
            1,
            "{name} is declared more than once"
        );
    }
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for name in [
        "crates/repository-competency/tests/competency_scans.rs",
        "crates/repository-competency/tests/competency_lanes.rs",
    ] {
        assert!(
            page.contains(name),
            "{name} reads Rust source or the design document and is not on the inventory page"
        );
    }
    Ok(())
}

/// The helpers this file's claims rest on, exercised against inputs whose
/// answers are known independently.
///
/// `P2-L3`'s finding is why: an oracle that reads its expectation out of the
/// thing it is checking agrees with itself. Every case below is written here and
/// none of it is read from the crate.
#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(collapse("  a   b \n c "), "a b c");
    assert_eq!(uses_of("a_fs_b fs fs::x", "fs"), 2);
    assert_eq!(uses_of("no match here", "fs"), 0);
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
    assert_eq!(calls_of("x.judge(&a); judge(b); judged(c);", "judge"), 2);
    assert_eq!(declarations_of("fn judge(x: u32) {}", "judge"), 1);
    assert!(whole_block("impl A { fn b() {} }", "impl A {").is_ok());
    assert!(whole_block("impl A { fn b() {} }", "impl Z {").is_err());

    // And the walk really reads this package rather than an empty directory.
    assert!(crate_product_sources()?.len() >= 6);
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

/// Every inventory row, as one list.
fn inventory() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    FIELDS
        .into_iter()
        .chain(MORE_FIELDS)
        .chain(LAST_FIELDS)
        .collect()
}

/// Every `impl` header this crate declares, as a whole set.
///
/// Read out of this crate's own source by the reader below and compared
/// whole in both directions, so an `impl` added anywhere is an entry here or
/// a failure. `P2-A5` measured that nothing else in the repository can see one.
const IMPL_HEADERS: [&str; 33] = [
    "impl AuthoredWork",
    "impl AuthorshipMap",
    "impl AuthorshipMode",
    "impl CandidateSupport",
    "impl ChangeId",
    "impl ChangeKind",
    "impl ChangeVerdict",
    "impl ChangedSite",
    "impl ClaimId",
    "impl ClaimStanding",
    "impl CodeOrigin",
    "impl ContributionKind",
    "impl ExplainedByUser",
    "impl ExternalAuthorId",
    "impl GeneratedCodeWarrant",
    "impl IdentitySource",
    "impl ModifiedByUser",
    "impl OriginReport",
    "impl OutcomeArtifact",
    "impl OutcomeKind",
    "impl PersonalApplicationClaim",
    "impl PersonalProvenance",
    "impl ProjectObservationClaim",
    "impl ProjectProvenance",
    "impl PromotionCheck",
    "impl PromotionSet",
    "impl RejectionReason",
    "impl RubricId",
    "impl ScaffoldRubric",
    "impl UserId",
    "impl VerifiedByUser",
    "impl WarrantStep",
    "impl<'a> ContributionDraft<'a>",
];

// ---------------------------------------------------------------------------
// The `impl` header inventory.
// ---------------------------------------------------------------------------

/// Every `impl` header of `code`, from `impl` to the brace that opens it.
///
/// A header may be wrapped across lines, so reading continues until the block
/// opens. An `impl Trait` in argument position never begins a line — a
/// parameter list always puts a name and a colon in front of it — so the line
/// anchor is what separates the two.
fn impl_headers(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut lines = code.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if !(trimmed == "impl" || trimmed.starts_with("impl ") || trimmed.starts_with("impl<")) {
            continue;
        }
        let mut header = trimmed.to_owned();
        while !header.contains('{') {
            let Some(next) = lines.next() else {
                break;
            };
            header.push(' ');
            header.push_str(next.trim());
        }
        let end = header.find('{').unwrap_or(header.len());
        found.insert(tighten(&header[..end]).trim().to_owned());
    }
    found
}

/// Traits whose whole purpose is to fold one value into another.
///
/// A conversion, an addition or a dereference from one of this crate's types
/// hands a caller a second reading of the same value, and nothing in a `pub fn`
/// inventory can see one. The list is refused as a property of the whole
/// header inventory rather than of named type pairs, so a fold between two
/// types nobody thought of is refused too.
const FOLDING_TRAITS: [&str; 15] = [
    "Add",
    "AddAssign",
    "Sum",
    "Product",
    "Mul",
    "MulAssign",
    "Deref",
    "DerefMut",
    "AsRef<",
    "AsMut<",
    "Borrow<",
    "BorrowMut<",
    "FromIterator<",
    "IntoIterator",
    "Index",
];

/// Scalar types a conversion out of one of this crate's types must not reach.
const SCALAR_TARGETS: [&str; 14] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32",
    "f64",
];

/// Every `impl` header this crate declares is in the inventory, both ways.
///
/// `P2-A5` measured this bypass class open across R1 to R5. It injected
///
/// ```text
/// impl From<&PromotionSet> for u32 {
///     fn from(set: &PromotionSet) -> Self { … }
/// }
/// ```
///
/// into `academic-repository-competency` — a conversion that folds section
/// 17.6's project half and personal half into one number, which is exactly the
/// separation the crate exists to keep — and it passed 1543 tests over 265
/// binaries with nothing in the repository seeing it. A trait `impl` declares
/// no `pub fn`, so a signature inventory that looks for `pub fn ` and
/// `pub const fn ` is blind to one by construction.
///
/// This is `P2-R6`'s `every_impl_header_in_this_crate_is_in_the_inventory`
/// ported here, which is where the class was first closed.
#[test]
fn every_impl_header_in_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        found.extend(impl_headers(&code));
    }
    assert_eq!(
        found,
        IMPL_HEADERS.iter().map(|item| (*item).to_owned()).collect(),
        "the impl-header inventory and the source disagree"
    );

    for header in &found {
        for folding in FOLDING_TRAITS {
            assert!(
                uses_of(header, folding) == 0,
                "{header} implements {folding}, which this crate does not admit"
            );
        }
        if !(header.contains("From<") || header.contains("Into<")) {
            continue;
        }
        for scalar in SCALAR_TARGETS {
            assert!(
                uses_of(header, scalar) == 0,
                "{header} converts to or from {scalar}"
            );
        }
    }

    // The reader is not vacuous, in both directions: it finds a header in a
    // fragment that has one — the exact shape `P2-A5` injected — and this
    // crate really declares some, so the property above is a statement about
    // something rather than about an empty set.
    let fragment = "impl From<&PromotionSet> for u32 {\n    fn from(_: &PromotionSet) -> Self {\n        0\n    }\n}\n";
    assert_eq!(
        impl_headers(fragment),
        ["impl From<&PromotionSet> for u32"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(
        !found.is_empty(),
        "this crate declares no impl header, so the refusals above say nothing"
    );
    Ok(())
}
