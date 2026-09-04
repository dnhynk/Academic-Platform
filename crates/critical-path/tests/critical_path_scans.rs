//! What `academic-critical-path` may reach, hold and hand out.
//!
//! ## The claim this file exists for
//!
//! `slider_changes_order_not_facts` drives the behaviour: two preferences
//! produce two orders over vectors that compare equal afterwards. That is a
//! statement about two runs. **The statement this task actually makes is
//! stronger** -- that a preference *cannot* rewrite a fact and that a vector
//! *cannot* be folded into a number, because neither operation exists -- and a
//! behavioural test cannot say that. [`the_vectors_cannot_be_folded`] and
//! [`the_preference_layer_cannot_reach_a_vector`] say it, as whole-set
//! comparisons over the crate's own source plus whole-text pins on the
//! decisions.
//!
//! The pins are the decisions a later edit could move without any behavioural
//! test noticing: how two candidates compare under a preference
//! ([`WHOLE_COMPARE_UNDER`]), what Pareto domination means on intervals
//! ([`WHOLE_DOMINANCE`]), which band section 16.3 calls stale
//! ([`WHOLE_IS_STALE`]), where the checkpoint threshold sits and which side of
//! it is strict ([`WHOLE_FOR_RATIO`], [`WHOLE_THRESHOLD`]), that an unmeasured
//! estimate cannot be a point ([`WHOLE_ESTIMATE_OF`]), that elimination is the
//! only route onto a front ([`WHOLE_ELIMINATE`], [`WHOLE_RANK_SIGNATURE`]),
//! that a slider is a complete permutation ([`WHOLE_SLIDER_OF`]), that all
//! eight constraints are answered in order ([`WHOLE_EVALUATE`]), that the
//! stricter verdict wins ([`WHOLE_WORSE`]), and that all five disclosure groups
//! are taken by one constructor ([`WHOLE_DISCLOSURE_OF`]).
//!
//! The extractors are `crates/gap/tests/gap_scans.rs`'s, restated the way that
//! file restates `academic-freshness`'s and that file restates `P2-N2`'s: a test
//! module is not a library target. [`the_helpers_are_not_vacuous`] re-exercises
//! each of them here against a sample it must match, because an extractor that
//! always answered the empty set would satisfy every comparison below -- and it
//! carries a **control**: the same reader is required to find most of a set of
//! names in a file that does spell them and none in one that does not, so a
//! zero reported here is a measurement rather than a reader that always answers
//! zero.
//!
//! ## It reads no clock
//!
//! Every instant this engine holds arrived as a caller-supplied day count or
//! inside a `P2-N3` value. [`REACHED_PATHS`] holds no `std::time`,
//! [`USE_ITEMS`] imports no clock, and nothing here opens a file or a socket.

#![allow(clippy::items_after_statements)]

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

/// The directories under this package that are not product code.
///
/// Named rather than implied. `the_package_has_no_unscanned_directory` compares
/// this against what is actually on disk in both directions, so a `benches/` or
/// a `build.rs` added later is a failure here rather than a directory the
/// product scans quietly stopped covering.
const NON_PRODUCT_DIRECTORIES: [&str; 2] = ["tests", "examples"];

fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root().join("src"), &mut found)?;
    walk(&crate_root().join("tests"), &mut found)?;
    walk(&crate_root().join("examples"), &mut found)?;
    found.sort();
    Ok(found)
}

fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    let mut found = Vec::new();
    walk(&root, &mut found)?;
    found.retain(|path| {
        let relative = path.strip_prefix(&root).unwrap_or(path);
        !NON_PRODUCT_DIRECTORIES
            .iter()
            .any(|directory| relative.starts_with(directory))
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

/// One free function's text, from its signature to the `}` at column zero.
fn free_function(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{signature} has no closing brace at column zero"))?;
    Ok(collapse(&source[start..start + end + 2]))
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

/// This crate's product files, as code with comments and literals removed.
fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

/// Joins a path or a macro name that whitespace was inserted into.
///
/// `std :: env :: var(k)` and `std::env::var(k)` are one reach, and an earlier
/// guard `P2-R2` shipped saw only the second.
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
    let roots = ["std", "core", "alloc", "crate", "thiserror"];
    let code = &tighten(code);
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
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
        // crate root. A **leading** `::` is not a middle segment:
        // `::std::path::Path::new(p)` is the absolute form of the same reach,
        // and `P2-R2` measured an earlier version of this function skipping it.
        let after_segment = start >= 3
            && &code[start - 2..start] == "::"
            && (bytes[start - 3].is_ascii_alphanumeric() || bytes[start - 3] == b'_');
        if after_segment {
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

/// Every `use` item, flattened to one leaf per line, `pub use` excluded.
fn use_items(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(at) = rest.find("use ") {
        let before = rest[..at].trim_end();
        let re_export = before.ends_with("pub");
        let boundary = rest[..at].chars().next_back().unwrap_or('\n');
        let joined = boundary.is_alphanumeric() || boundary == '_';
        let Some(end) = rest[at..].find(';') else {
            break;
        };
        if !re_export && !joined {
            let item: String = rest[at + 4..at + end].split_whitespace().collect();
            flatten(&item, String::new(), &mut found);
        }
        rest = &rest[at + end + 1..];
    }
    found.sort();
    found.dedup();
    found
}

/// The modules a `pub use` hands names out of.
fn re_export_modules(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(at) = rest.find("pub use ") {
        let Some(end) = rest[at..].find(';') else {
            break;
        };
        let item: String = rest[at + 8..at + end].split_whitespace().collect();
        let module = item.split("::").next().unwrap_or("").to_owned();
        if !module.is_empty() {
            found.push(module);
        }
        rest = &rest[at + end + 1..];
    }
    found.sort();
    found.dedup();
    found
}

fn flatten(item: &str, prefix: String, found: &mut Vec<String>) {
    if let Some(open) = item.find('{') {
        let head = format!("{prefix}{}", &item[..open]);
        let Some(inner) = balanced(&item[open..]) else {
            return;
        };
        for part in top_level_items(inner) {
            flatten(part.trim(), head.clone(), found);
        }
        return;
    }
    if item.is_empty() {
        return;
    }
    found.push(format!("{prefix}{item}"));
}

// ---------------------------------------------------------------------------
// The inventories.
// ---------------------------------------------------------------------------

/// Every `use` item this crate's product code spells, in both directions.
///
/// Filled by `the_helpers_are_not_vacuous`'s sibling: an item the crate reaches
/// and this list does not hold is an **extra key** rather than a token nobody
/// listed, which is what makes the comparison a whole set rather than a
/// denylist.
const USE_ITEMS: [&str; 76] = [
    "academic_curriculum::Credits",
    "academic_curriculum::Meeting",
    "academic_curriculum::OfferingStatus",
    "academic_curriculum::Weekday",
    "academic_domain::ContentDigest",
    "academic_domain::EntityId",
    "academic_domain::EvidenceId",
    "academic_domain::FreshnessBand",
    "academic_domain::OfferingId",
    "academic_domain::engines::EngineError",
    "academic_domain::engines::EngineOutcome",
    "academic_domain::engines::EngineResult",
    "academic_domain::engines::FrozenInputs",
    "academic_domain::engines::InputKey",
    "academic_domain::engines::InputValue",
    "academic_domain::engines::NodeId",
    "academic_domain::engines::ProofNode",
    "academic_domain::engines::ProofStatus",
    "academic_domain::engines::RuleId",
    "academic_gap::GapCase",
    "academic_gap::PrerequisiteEdge",
    "crate::CriticalPathError",
    "crate::checkpoint::CheckpointDecision",
    "crate::checkpoint::uncertain_edge_ratio_permille",
    "crate::constraint::CONSTRAINTS",
    "crate::constraint::Constraint",
    "crate::constraint::ConstraintFinding",
    "crate::constraint::ConstraintInputs",
    "crate::constraint::ConstraintVerdict",
    "crate::constraint::OfficialPrerequisiteStanding",
    "crate::constraint::RequiredInsertion",
    "crate::constraint::evaluate",
    "crate::counterfactual::sensitivity_of",
    "crate::counterfactual::without",
    "crate::disclosure::AlternativeRoute",
    "crate::disclosure::Alternatives",
    "crate::disclosure::ComputationSnapshot",
    "crate::disclosure::CostAssumption",
    "crate::disclosure::CostAssumptions",
    "crate::disclosure::Disclosure",
    "crate::disclosure::ExcludedRoute",
    "crate::disclosure::ExclusionReason",
    "crate::disclosure::Exclusions",
    "crate::disclosure::UncertainEdge",
    "crate::disclosure::UncertainEdges",
    "crate::engine::PlanRequest",
    "crate::hypergraph::EdgeMember",
    "crate::hypergraph::EdgeStanding",
    "crate::hypergraph::Hyperedge",
    "crate::hypergraph::PrerequisiteHypergraph",
    "crate::hypergraph::SatisfyingSet",
    "crate::hypergraph::satisfying_sets",
    "crate::option::AcquisitionOption",
    "crate::pareto::ParetoFront",
    "crate::plan::Candidate",
    "crate::plan::CriticalPathResult",
    "crate::plan::PathRole",
    "crate::plan::PlanStep",
    "crate::preference::NAMED_STRATEGIES",
    "crate::preference::NamedStrategy",
    "crate::preference::PreferenceSlider",
    "crate::preference::rank",
    "crate::vector::BENEFIT_COMPONENTS",
    "crate::vector::BasisFamily",
    "crate::vector::BenefitComponent",
    "crate::vector::BenefitVector",
    "crate::vector::COST_COMPONENTS",
    "crate::vector::CostComponent",
    "crate::vector::CostEstimate",
    "crate::vector::CostVector",
    "crate::vector::VectorAxis",
    "crate::vector::all_axes",
    "serde::Deserialize",
    "serde::Serialize",
    "std::collections::BTreeMap",
    "std::collections::BTreeSet",
];

/// The modules `lib.rs` re-exports from, in both directions.
const RE_EXPORT_MODULES: [&str; 13] = [
    "checkpoint",
    "constraint",
    "counterfactual",
    "disclosure",
    "edit",
    "engine",
    "hypergraph",
    "option",
    "pareto",
    "plan",
    "preference",
    "proof",
    "vector",
];

/// Every two-segment path reached through a crate root, in both directions.
const REACHED_PATHS: [&str; 17] = [
    "academic_domain::Decimal",
    "academic_domain::DomainError",
    "academic_domain::EntityId",
    "academic_domain::EvidenceId",
    "academic_domain::engines",
    "academic_freshness::band_token",
    "academic_gap::GapError",
    "crate::constraint",
    "crate::counterfactual",
    "crate::hypergraph",
    "crate::option",
    "crate::preference",
    "crate::proof",
    "crate::vector",
    "std::cmp",
    "std::collections",
    "thiserror::Error",
];

/// Every macro the product code invokes, in both directions.
const MACROS_SPELLED: [&str; 3] = ["format", "matches", "vec"];

/// Names the control reader must find in this crate and not in a neighbour's.
const CRITICAL_PATH_NAMES: [&str; 8] = [
    "CostVector",
    "BenefitVector",
    "ParetoFront",
    "PreferenceSlider",
    "SatisfyingSet",
    "Disclosure",
    "CheckpointDecision",
    "AcquisitionOption",
];

/// Filesystem, clock, process and transport spellings, as a weakest third
/// layer behind the three whole-set comparisons above.
const FORBIDDEN_CONSTRUCTS: [&str; 15] = [
    "fs",
    "net",
    "process",
    "env",
    "time",
    "Instant",
    "SystemTime",
    "now",
    "File",
    "Path",
    "PathBuf",
    "TcpStream",
    "Command",
    "include_str",
    "include_bytes",
];

/// Every way of folding a vector into one number.
///
/// The rule those three whole sets exist to hold: **no product file may name
/// an operation that combines two axes.** `slider_changes_order_not_facts` and
/// `cost_vector_has_seven_separate_components` can only observe the axes they
/// exercised; this is the whole-crate statement.
///
/// `plus` is deliberately absent: interval addition combines the *same* axis of
/// two steps and is what a path's cost is. What is forbidden is combining two
/// *different* axes, and every spelling below is a way of doing that.
const FOLDING_OPERATIONS: [&str; 12] = [
    "sum", "product", "fold", "reduce", "total", "score", "weight", "weighted", "midpoint",
    "average", "mean", "scalar",
];

/// Every way of mutating through a shared reference.
///
/// `rank` takes `&ParetoFront` and returns a `Ranking` that borrows it, so the
/// slider cannot write. That guarantee is the borrow checker's only while there
/// is no interior mutability anywhere in the crate.
const INTERIOR_MUTABILITY: [&str; 8] = [
    "Cell",
    "RefCell",
    "UnsafeCell",
    "Mutex",
    "RwLock",
    "AtomicUsize",
    "AtomicU32",
    "unsafe",
];

/// The files of this package permitted to read anything at all.
const READERS: [&str; 3] = [
    "crates/critical-path/tests/critical_path.rs",
    "crates/critical-path/tests/critical_path_harness.rs",
    "crates/critical-path/tests/critical_path_scans.rs",
];

/// The modules allowed to name an ordering.
///
/// `preference.rs` computes it and `lib.rs` re-exports the type. Every other
/// module may hold a `PreferenceSlider` as a field or an argument -- `plan.rs`
/// stores the one a result was produced under and `engine.rs` hands it to
/// `rank` -- but none of them may *produce* an order, and naming [`Ranking`] or
/// the comparison is what producing one looks like. A second module that
/// started ordering candidates appears here rather than in a diff nobody read.
const ORDERING_MODULES: [&str; 2] = [
    "crates/critical-path/src/preference.rs",
    "crates/critical-path/src/lib.rs",
];

/// The names that mean *an order is being produced here*.
const ORDERING_NAMES: [&str; 2] = ["Ranking", "compare_under"];

// ---------------------------------------------------------------------------
// The scans.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    // A scan over a file list that quietly lost a module would report nothing
    // about that module, so the list is compared against `lib.rs`'s own
    // declarations in both directions before anything else runs.
    let lib = fs::read_to_string(crate_root().join("src/lib.rs"))?;
    let declared: BTreeSet<String> = lib
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .map(|rest| rest.trim_end_matches(';').to_owned())
        .collect();
    let walked: BTreeSet<String> = crate_product_sources()?
        .iter()
        .filter_map(|path| path.file_stem().and_then(|stem| stem.to_str()))
        .filter(|stem| *stem != "lib")
        .map(str::to_owned)
        .collect();
    assert_eq!(
        declared, walked,
        "lib.rs declares {declared:?} and the walk found {walked:?}"
    );
    assert!(
        declared.len() >= 13,
        "only {} modules were declared",
        declared.len()
    );

    // And the re-export modules `lib.rs` names are the same set.
    let re_exported: BTreeSet<String> = re_export_modules(&lib).into_iter().collect();
    assert_eq!(
        re_exported,
        RE_EXPORT_MODULES.into_iter().map(str::to_owned).collect(),
        "lib.rs re-exports from a different set of modules than the pin"
    );
    Ok(())
}

/// The product walk skips exactly `tests/` and `examples/`, and nothing else.
///
/// Excluding a directory from the product scan is how a scan goes quiet without
/// anybody noticing, so what is excluded is compared against what is on disk in
/// both directions. A `build.rs` or a `benches/` added later fails here rather
/// than becoming source that no rule below reads.
#[test]
fn the_package_has_no_unscanned_directory() -> TestResult {
    let root = crate_root();
    let mut directories: BTreeSet<String> = BTreeSet::new();
    let mut files: BTreeSet<String> = BTreeSet::new();
    for entry in fs::read_dir(&root)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("a package entry is not UTF-8")?
            .to_owned();
        if path.is_dir() {
            directories.insert(name);
        } else {
            files.insert(name);
        }
    }
    assert_eq!(
        directories,
        ["examples", "src", "tests"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "the package holds a directory no scan accounts for"
    );
    assert_eq!(
        files,
        ["Cargo.toml"].into_iter().map(str::to_owned).collect(),
        "the package holds a file no scan accounts for, a build script among the \
         shapes that would matter"
    );
    for excluded in NON_PRODUCT_DIRECTORIES {
        assert!(
            directories.contains(excluded),
            "{excluded} is excluded from the product walk and does not exist"
        );
    }

    // The example writes the corpus and reads nothing, which is why it is not a
    // named reader and why excluding it from the product walk costs nothing.
    let example = strip_non_code(&fs::read_to_string(root.join("examples/emit_corpus.rs"))?);
    assert_eq!(calls_of(&example, "read_to_string"), 0);
    assert_eq!(calls_of(&example, "read_dir"), 0);
    assert_eq!(
        calls_of(&example, "write"),
        1,
        "the corpus example writes something other than the corpus"
    );
    Ok(())
}

/// Every public function of this crate that hands out a bare number.
///
/// Compared in both directions, which is the closure the twelve folding
/// spellings above cannot give. `P2-N7` measured the gap: a method summing all
/// seven axes of a `CostVector` and named `as_one_number` spells none of those
/// twelve, none of `impl CostEstimate`'s four, adds no `use` item and reaches no
/// new path, and it passed this whole suite and `clippy` alike. A scalar the API
/// hands out now appears here as an **extra key** whatever it is called, which
/// is the shape `docs/contracts/policy-source-scans.md` records seven spellings
/// defeating a list.
///
/// The ten below are each an ordinal, a count or one end of a declared interval.
/// None of them combines two axes.
const NUMERIC_RETURNS: [&str; 10] = [
    "checkpoint.rs uncertain_edge_ratio_permille -> u16",
    "counterfactual.rs routes_after -> usize",
    "counterfactual.rs routes_before -> usize",
    "hypergraph.rs uncertain_member_count -> usize",
    "option.rs credits -> u8",
    "pareto.rs dominated_by -> usize",
    "pareto.rs len -> usize",
    "plan.rs rank -> usize",
    "vector.rs high -> u32",
    "vector.rs low -> u32",
];

#[test]
fn the_vectors_cannot_be_folded() -> TestResult {
    // The whole `use` inventory, in both directions.
    let mut found: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        found.extend(use_items(&code));
    }
    let pinned: BTreeSet<String> = USE_ITEMS.into_iter().map(str::to_owned).collect();
    assert_eq!(found, pinned, "the crate's use items moved");

    // The whole set of two-segment paths reached through a crate root, in both
    // directions, over code with the `use` items removed so a re-export is not
    // counted as a reach.
    let mut reached: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        reached.extend(absolute_paths(&without_use_items(&code)));
    }
    let pinned: BTreeSet<String> = REACHED_PATHS.into_iter().map(str::to_owned).collect();
    assert_eq!(reached, pinned, "the crate's reached paths moved");

    // Every macro, in both directions.
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        macros.extend(macros_spelled(&code));
    }
    let pinned: BTreeSet<String> = MACROS_SPELLED.into_iter().map(str::to_owned).collect();
    assert_eq!(macros, pinned, "the crate's macros moved");

    // And the rule those three exist to hold: nothing combines two axes.
    for (path, code) in product_code()? {
        let tightened = tighten(&code);
        for operation in FOLDING_OPERATIONS {
            assert_eq!(
                uses_of(&tightened, operation),
                0,
                "{path} names {operation}, so a vector can be folded into a number"
            );
        }
    }

    // The two vector types derive no order, so there is no comparison to fold
    // into either.
    let vector = strip_non_code(&fs::read_to_string(crate_root().join("src/vector.rs"))?);
    for declaration in ["pub struct CostVector", "pub struct BenefitVector"] {
        let start = vector
            .find(declaration)
            .ok_or_else(|| format!("{declaration} is not in vector.rs"))?;
        let derive = vector[..start]
            .rfind("#[derive(")
            .ok_or_else(|| format!("{declaration} has no derive"))?;
        let attribute = &vector[derive..start];
        for order in ["PartialOrd", "Ord"] {
            assert!(
                !attribute.contains(order),
                "{declaration} derives {order}, which is an order over a whole vector"
            );
        }
    }

    // And the interval type hands out its two ends and no third number.
    let estimate = whole_block(&vector, "impl CostEstimate")?;
    for narrowing in ["midpoint", "point", "expected", "value"] {
        assert_eq!(
            uses_of(&estimate, narrowing),
            0,
            "CostEstimate names {narrowing}, so a range can collapse to a point"
        );
    }

    // The whole set of public functions that hand out a bare number, in both
    // directions. The twelve spellings above are a list and a list refuses the
    // edits somebody predicted; this refuses every scalar the API could hand
    // out, whatever it is called. See NUMERIC_RETURNS for how the gap was found.
    let mut numeric: Vec<String> = Vec::new();
    for (path, code) in product_code()? {
        let module = path.rsplit('/').next().unwrap_or(&path).to_owned();
        for (name, signature) in public_signatures(&code) {
            let tail = signature
                .split_once("->")
                .map_or("()", |(_, rest)| rest)
                .trim();
            let tail = tail.split_whitespace().collect::<Vec<_>>().join(" ");
            if matches!(
                tail.as_str(),
                "u8" | "u16"
                    | "u32"
                    | "u64"
                    | "usize"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "isize"
                    | "f32"
                    | "f64"
            ) {
                numeric.push(format!("{module} {name} -> {tail}"));
            }
        }
    }
    numeric.sort();
    assert_eq!(
        numeric,
        NUMERIC_RETURNS
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "this crate's public numeric returns and NUMERIC_RETURNS disagree"
    );
    // The control: the reader is required to see the two ends of an interval, so
    // an extractor that always answered the empty set would not pass here.
    assert!(
        numeric
            .iter()
            .any(|entry| entry.starts_with("vector.rs high"))
    );
    assert!(
        numeric
            .iter()
            .any(|entry| entry.starts_with("vector.rs low"))
    );
    Ok(())
}

#[test]
fn the_preference_layer_cannot_reach_a_vector() -> TestResult {
    // A slider physically cannot build or replace a vector, because the module
    // that holds one cannot name the types. That is the structural half of
    // `slider_changes_order_not_facts`.
    let preference = strip_non_code(&fs::read_to_string(crate_root().join("src/preference.rs"))?);
    for name in ["CostVector", "BenefitVector", "CostEstimate", "CostBasis"] {
        assert_eq!(
            uses_of(&preference, name),
            0,
            "preference.rs names {name}, so an ordering can build a fact"
        );
    }

    // Nothing anywhere mutates through a shared reference.
    for (path, code) in product_code()? {
        let tightened = tighten(&code);
        for construct in INTERIOR_MUTABILITY {
            assert_eq!(
                uses_of(&tightened, construct),
                0,
                "{path} names {construct}, so a shared borrow is not a guarantee"
            );
        }
    }

    // Only the named modules produce an order at all.
    let mut producers: BTreeSet<String> = BTreeSet::new();
    for (path, code) in product_code()? {
        let tightened = tighten(&code);
        if ORDERING_NAMES
            .into_iter()
            .any(|name| uses_of(&tightened, name) > 0)
        {
            producers.insert(path);
        }
    }
    assert_eq!(
        producers,
        ORDERING_MODULES.into_iter().map(str::to_owned).collect(),
        "an ordering is produced somewhere other than the preference module"
    );
    // Both directions: a module that stopped naming one would shrink the set,
    // and the comparison above catches that too.
    assert_eq!(producers.len(), ORDERING_MODULES.len());
    Ok(())
}

#[test]
fn no_clock_socket_or_file_reaches_this_crate() -> TestResult {
    for (path, code) in product_code()? {
        let tightened = tighten(&code);
        for construct in FORBIDDEN_CONSTRUCTS {
            assert_eq!(
                uses_of(&tightened, construct),
                0,
                "{path} names {construct}"
            );
        }
    }
    Ok(())
}

#[test]
fn only_the_named_test_files_read_anything() -> TestResult {
    for path in crate_all_sources()? {
        let relative = relative(&path);
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let reads = calls_of(&code, "read_to_string") + calls_of(&code, "read_dir");
        if reads == 0 {
            continue;
        }
        assert!(
            READERS.contains(&relative.as_str()),
            "{relative} reads from the filesystem and is not a named reader"
        );
    }
    Ok(())
}

#[test]
fn the_critical_path_decisions_are_pinned() -> TestResult {
    let checkpoint = strip_non_code(&fs::read_to_string(crate_root().join("src/checkpoint.rs"))?);
    assert!(
        checkpoint.contains(WHOLE_THRESHOLD),
        "the uncertain-edge threshold moved"
    );
    assert_eq!(
        whole_block(
            &checkpoint,
            "pub const fn for_ratio(ratio_permille: u16) -> Self"
        )?,
        WHOLE_FOR_RATIO,
        "which side of the threshold inserts a checkpoint moved"
    );

    let constraint = strip_non_code(&fs::read_to_string(crate_root().join("src/constraint.rs"))?);
    assert_eq!(
        free_function(&constraint, "pub const fn is_stale(band: FreshnessBand)")?,
        WHOLE_IS_STALE,
        "which band section 16.3 calls stale moved"
    );
    assert_eq!(
        free_function(&constraint, "const fn worse(")?,
        WHOLE_WORSE,
        "the stricter-verdict rule moved"
    );
    assert_eq!(
        free_function(&constraint, "pub fn evaluate(")?,
        WHOLE_EVALUATE,
        "the eight constraint answers or their order moved"
    );

    let pareto = strip_non_code(&fs::read_to_string(crate_root().join("src/pareto.rs"))?);
    assert_eq!(
        free_function(&pareto, "pub fn dominance(")?,
        WHOLE_DOMINANCE,
        "what Pareto domination means on intervals moved"
    );
    assert_eq!(
        whole_block(
            &pareto,
            "pub fn eliminate(candidates: Vec<Candidate>) -> Self"
        )?,
        WHOLE_ELIMINATE,
        "the elimination moved"
    );

    let preference = strip_non_code(&fs::read_to_string(crate_root().join("src/preference.rs"))?);
    assert!(
        collapse(&preference).contains(WHOLE_RANK_SIGNATURE),
        "the ranker's signature moved, so elimination may no longer come first"
    );
    assert_eq!(
        free_function(&preference, "fn compare_under(")?,
        WHOLE_COMPARE_UNDER,
        "how a preference compares two candidates moved"
    );
    assert_eq!(
        whole_block(&preference, "pub fn of(order: Vec<VectorAxis>)")?,
        WHOLE_SLIDER_OF,
        "the rule that a slider is a complete permutation moved"
    );

    let vector = strip_non_code(&fs::read_to_string(crate_root().join("src/vector.rs"))?);
    assert!(
        collapse(&vector).contains(WHOLE_ESTIMATE_OF),
        "the rule that an unmeasured estimate is not a point moved"
    );

    let disclosure = strip_non_code(&fs::read_to_string(crate_root().join("src/disclosure.rs"))?);
    assert!(
        collapse(&disclosure).contains(WHOLE_DISCLOSURE_OF),
        "the constructor that takes all five disclosure groups moved"
    );

    let engine = strip_non_code(&fs::read_to_string(crate_root().join("src/engine.rs"))?);
    assert_eq!(
        free_function(&engine, "pub fn plan(request: &PlanRequest<'_>)")?,
        WHOLE_PLAN,
        "the order of section 16's stages moved"
    );
    Ok(())
}

/// Section 16.3's `일정 비율`, and the fact that it is a `const` rather than a
/// literal at the comparison.
const WHOLE_THRESHOLD: &str = "pub const UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE: u16 = 300;";

/// `넘을 때` is strict. A `>=` here would insert a checkpoint at the threshold.
const WHOLE_FOR_RATIO: &str = "pub const fn for_ratio(ratio_permille: u16) -> Self { if ratio_permille > UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE { Self::Insert } else { Self::BelowThreshold } }";

/// Section 16.3's seventh bullet names one band, and `UNKNOWN` is not it.
const WHOLE_IS_STALE: &str = "pub const fn is_stale(band: FreshnessBand) -> bool { match band { FreshnessBand::Stale => true, FreshnessBand::Unknown | FreshnessBand::Low | FreshnessBand::Moderate | FreshnessBand::High | FreshnessBand::VeryHigh => false, } }";

/// `Violated` beats `Unknown` beats `SatisfiedWithInsertion` beats `Satisfied`.
const WHOLE_WORSE: &str = "const fn worse(left: ConstraintVerdict, right: ConstraintVerdict) -> ConstraintVerdict { match (left, right) { (ConstraintVerdict::Violated, _) | (_, ConstraintVerdict::Violated) => { ConstraintVerdict::Violated } (ConstraintVerdict::Unknown, _) | (_, ConstraintVerdict::Unknown) => { ConstraintVerdict::Unknown } (ConstraintVerdict::SatisfiedWithInsertion, _) | (_, ConstraintVerdict::SatisfiedWithInsertion) => { ConstraintVerdict::SatisfiedWithInsertion } (ConstraintVerdict::Satisfied, ConstraintVerdict::Satisfied) => { ConstraintVerdict::Satisfied } } }";

/// All eight answers, in `CONSTRAINTS` order, with no filter and no `Option`.
const WHOLE_EVALUATE: &str = "pub fn evaluate( set: &SatisfyingSet, options: &[AcquisitionOption], inputs: &ConstraintInputs, calendar_delay_days_high: u32, ) -> Result<[ConstraintFinding; 8], CriticalPathError> { let findings = vec![ hard_prerequisites(set, inputs), offering_standing(options, inputs), timetable_and_credits(options, inputs), deadline_or_horizon(inputs, calendar_delay_days_high), privacy_excluded(options, inputs), user_exclusions(set, options, inputs), stale_refresh(set, inputs), uncertain_checkpoint(set), ]; findings .try_into() .map_err(|_| CriticalPathError::ConstraintCountChanged) }";

/// Domination on intervals compares both ends and forms no midpoint, so two
/// crossing intervals stay incomparable.
const WHOLE_DOMINANCE: &str = "pub fn dominance(left: &Candidate, right: &Candidate) -> Dominance { let mut left_better_somewhere = false; let mut right_better_somewhere = false; for component in COST_COMPONENTS { let a = left.cost().component(component); let b = right.cost().component(component); if a.low() <= b.low() && a.high() <= b.high() && (a.low() < b.low() || a.high() < b.high()) { left_better_somewhere = true; } else if b.low() <= a.low() && b.high() <= a.high() && (b.low() < a.low() || b.high() < a.high()) { right_better_somewhere = true; } else if a != b { return Dominance::Incomparable; } } for component in BENEFIT_COMPONENTS { let a = left.benefit().component(component); let b = right.benefit().component(component); if a.low() >= b.low() && a.high() >= b.high() && (a.low() > b.low() || a.high() > b.high()) { left_better_somewhere = true; } else if b.low() >= a.low() && b.high() >= a.high() && (b.low() > a.low() || b.high() > a.high()) { right_better_somewhere = true; } else if a != b { return Dominance::Incomparable; } } match (left_better_somewhere, right_better_somewhere) { (true, false) => Dominance::LeftDominates, (false, true) => Dominance::RightDominates, (true, true) | (false, false) => Dominance::Incomparable, } }";

/// The only route onto a Pareto front.
const WHOLE_ELIMINATE: &str = "pub fn eliminate(candidates: Vec<Candidate>) -> Self { let mut kept: Vec<Candidate> = Vec::new(); let mut removed: Vec<(Candidate, usize)> = Vec::new(); for (index, candidate) in candidates.iter().enumerate() { let mut dominator: Option<usize> = None; for (other_index, other) in candidates.iter().enumerate() { if other_index == index { continue; } if dominance(other, candidate) == Dominance::LeftDominates { dominator = Some(other_index); break; } } match dominator { Some(by) => removed.push((candidate.clone(), by)), None => kept.push(candidate.clone()), } } let dominated = removed .into_iter() .map(|(candidate, original)| { let dominator = candidates .get(original) .and_then(|winner| kept.iter().position(|survivor| survivor == winner)) .unwrap_or(0); Dominated { candidate, dominated_by: dominator, } }) .collect(); Self { candidates: kept, dominated, } }";

/// The ranker takes a `&ParetoFront` and nothing else, which is what makes
/// section 16.2's `먼저` a type rather than a comment.
const WHOLE_RANK_SIGNATURE: &str =
    "pub fn rank<'a>(front: &'a ParetoFront, slider: &PreferenceSlider) -> Ranking<'a> {";

/// A preference walks the axes in priority order and combines none of them.
const WHOLE_COMPARE_UNDER: &str = "fn compare_under( left: &Candidate, right: &Candidate, slider: &PreferenceSlider, ) -> std::cmp::Ordering { for axis in slider.order() { let ordering = match axis { VectorAxis::Cost { component } => { let a = left.cost().component(*component); let b = right.cost().component(*component); (a.high(), a.low()).cmp(&(b.high(), b.low())) } VectorAxis::Benefit { component } => { let a = left.benefit().component(*component); let b = right.benefit().component(*component); (b.low(), b.high()).cmp(&(a.low(), a.high())) } }; if ordering != std::cmp::Ordering::Equal { return ordering; } } std::cmp::Ordering::Equal }";

/// A slider is a complete permutation: an omitted axis is a silent decision
/// that it does not matter.
const WHOLE_SLIDER_OF: &str = "pub fn of(order: Vec<VectorAxis>) -> Result<Self, CriticalPathError> { let offered: BTreeSet<VectorAxis> = order.iter().copied().collect(); let expected: BTreeSet<VectorAxis> = all_axes().into_iter().collect(); if offered.len() != order.len() || offered != expected { return Err(CriticalPathError::SliderIsNotAPermutation); } Ok(Self { order }) }";

/// Section 16.2's `근거가 없으면 범위로 표시한다`, at the constructor.
const WHOLE_ESTIMATE_OF: &str = "pub fn of( low: u32, high: u32, unit: Unit, basis: CostBasis, ) -> Result<Self, CriticalPathError> { if high < low { return Err(CriticalPathError::InvertedEstimate); } if !basis.is_measured() && low == high { return Err(CriticalPathError::UnmeasuredEstimateIsAPoint); } Ok(Self { low, high, unit, basis, }) }";

/// Section 16.5's five groups, taken by one constructor with no `Option`.
const WHOLE_DISCLOSURE_OF: &str = "pub const fn of( snapshot: ComputationSnapshot, cost_assumptions: CostAssumptions, exclusions: Exclusions, uncertain_edges: UncertainEdges, alternatives: Alternatives, ) -> Self { Self { snapshot, cost_assumptions, exclusions, uncertain_edges, alternatives, } }";

/// Section 16.2's stage order: satisfy, cost, constrain, **eliminate**, order.
const WHOLE_PLAN: &str = "pub fn plan(request: &PlanRequest<'_>) -> Result<CriticalPathResult, CriticalPathError> { let goal_concept = request.gap_case.surface_concept(); let sets = satisfying_sets(request.graph, goal_concept)?; let by_concept: BTreeMap<[u8; 16], &ConceptEstimate> = request .estimates .iter() .map(|estimate| (*estimate.concept.as_bytes(), estimate)) .collect(); let mut candidates = Vec::new(); for set in &sets { candidates.push(candidate_for(set, &by_concept, request.constraints)?); } let (feasible, refused): (Vec<Candidate>, Vec<Candidate>) = candidates .into_iter() .partition(academic_partition_is_feasible); let front = ParetoFront::eliminate(feasible); let ranking = rank(&front, request.slider); let ranked_candidates: Vec<Candidate> = ranking .candidates() .into_iter() .cloned() .collect::<Vec<_>>(); let roles = roles_of(request.graph, &ranked_candidates); let ranked = ranked_candidates .iter() .enumerate() .map(|(position, candidate)| { CriticalPathResult::ranked_path( candidate.clone(), position, if position == 0 { PathRole::SharedSpine } else { PathRole::AlternativePath }, strategy_for(candidate, &ranked_candidates), ) }) .collect(); let disclosure = disclose(request, &sets, &front, &refused, &ranked_candidates)?; CriticalPathResult::of( request.gap_case.goal(), front, ranked, roles, request.slider.clone(), disclosure, ) }";

#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    let mut signatures = 0_usize;
    for (path, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            signatures += 1;
            assert!(
                !signature.contains("&mut self"),
                "{path}::{name} takes &mut self"
            );
        }
    }
    assert!(
        signatures >= 90,
        "only {signatures} public signatures were read"
    );
    Ok(())
}

#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    // Every extractor is exercised against a sample it must match. An extractor
    // that always answered the empty set would satisfy every comparison above.
    assert_eq!(
        use_items("use academic_domain::{EntityId, EvidenceId};"),
        vec![
            "academic_domain::EntityId".to_owned(),
            "academic_domain::EvidenceId".to_owned()
        ]
    );
    assert!(use_items("pub use crate::vector::COST_COMPONENTS;").is_empty());
    assert_eq!(
        re_export_modules("pub use vector::COST_COMPONENTS;"),
        vec!["vector"]
    );
    assert!(absolute_paths("std::time::SystemTime::now()").contains("std::time"));
    assert!(absolute_paths("::std :: env :: vars_os()").contains("std::env"));
    assert!(macros_spelled("let s = include_str!(\"x\");").contains("include_str"));
    assert_eq!(uses_of("CostComponent::Uncertainty", "CostComponent"), 1);
    assert_eq!(uses_of("NotCostComponentHere", "CostComponent"), 0);
    assert_eq!(
        public_signatures("pub fn f(&mut self) {\n}\n")
            .first()
            .map(|(name, _)| name.clone()),
        Some("f".to_owned())
    );
    assert_eq!(calls_of("fn g() {} let x = g();", "g"), 1);
    assert_eq!(
        strip_non_code("let s = \"std::fs\"; // std::net\n").trim(),
        "let s =  ;"
    );

    // The whole-text pins are compared against a constant, so a pin whose
    // extractor found nothing would compare two empty strings. Each extractor is
    // required to find something in this crate's own source first.
    let pareto = strip_non_code(&fs::read_to_string(crate_root().join("src/pareto.rs"))?);
    assert!(
        free_function(&pareto, "pub fn dominance(")?.len() > 100,
        "the free-function reader found nothing to pin"
    );
    let preference = strip_non_code(&fs::read_to_string(crate_root().join("src/preference.rs"))?);
    assert!(
        whole_block(&preference, "pub fn of(order: Vec<VectorAxis>)")?.len() > 100,
        "the block reader found nothing to pin"
    );
    assert!(free_function(&pareto, "pub fn no_such_function(").is_err());

    // And the control on the folding rule. The same readers that report this
    // crate's use items, reached paths and type names are required to find most
    // of those names in a file that does spell them, so the sets reported above
    // are measurements rather than a reader that always answers nothing.
    let own = strip_non_code(&fs::read_to_string(crate_root().join("src/lib.rs"))?);
    let found: Vec<&str> = CRITICAL_PATH_NAMES
        .into_iter()
        .filter(|name| uses_of(&own, name) > 0)
        .collect();
    assert!(
        found.len() >= 6,
        "the reader found only {found:?} in this crate's own lib.rs, so what it \
         reports elsewhere proves nothing"
    );
    // The same reader over a file that spells none of them answers zero, which
    // is the other half of the control.
    let unrelated = strip_non_code(&fs::read_to_string(
        workspace_root().join("crates/gap/src/kind.rs"),
    )?);
    let leaked: Vec<&str> = CRITICAL_PATH_NAMES
        .into_iter()
        .filter(|name| uses_of(&unrelated, name) > 0)
        .collect();
    assert!(
        leaked.is_empty(),
        "the reader found {leaked:?} in P2-N5's kind module"
    );

    // The forbidden-token readers are exercised the same way: each is required
    // to find its own token in a sample that spells it.
    for token in FOLDING_OPERATIONS {
        assert_eq!(
            uses_of(&format!("let x = a.{token}(b);"), token),
            1,
            "the folding reader does not see {token}"
        );
    }
    for token in INTERIOR_MUTABILITY {
        assert_eq!(
            uses_of(&format!("let x: {token} = y;"), token),
            1,
            "the mutability reader does not see {token}"
        );
    }
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    assert!(
        page.contains("crates/critical-path/tests/critical_path_scans.rs"),
        "this scan is not named in the inventory page"
    );
    for name in [
        "the_walk_reads_every_module_in_this_package",
        "the_vectors_cannot_be_folded",
        "the_preference_layer_cannot_reach_a_vector",
        "no_clock_socket_or_file_reaches_this_crate",
        "only_the_named_test_files_read_anything",
        "the_critical_path_decisions_are_pinned",
        "no_public_function_mutates_in_place",
        "the_helpers_are_not_vacuous",
        "this_scan_is_in_the_inventory",
    ] {
        assert!(
            page.contains(name),
            "the inventory page does not name {name}"
        );
    }
    Ok(())
}

/// Prints every value the pins above compare against.
///
/// Ignored by default: it asserts nothing and exists so that a pin can be
/// re-derived from the source after a deliberate change, rather than retyped.
/// `cargo test -p academic-critical-path --test critical_path_scans -- --ignored --nocapture dump_pins`
#[test]
#[ignore = "prints the pin values; run explicitly after a deliberate change"]
fn dump_pins() -> TestResult {
    let checkpoint = strip_non_code(&fs::read_to_string(crate_root().join("src/checkpoint.rs"))?);
    let constraint = strip_non_code(&fs::read_to_string(crate_root().join("src/constraint.rs"))?);
    let pareto = strip_non_code(&fs::read_to_string(crate_root().join("src/pareto.rs"))?);
    let preference = strip_non_code(&fs::read_to_string(crate_root().join("src/preference.rs"))?);
    let vector = strip_non_code(&fs::read_to_string(crate_root().join("src/vector.rs"))?);
    let disclosure = strip_non_code(&fs::read_to_string(crate_root().join("src/disclosure.rs"))?);
    let engine = strip_non_code(&fs::read_to_string(crate_root().join("src/engine.rs"))?);

    println!(
        "WHOLE_FOR_RATIO<<{}>>",
        whole_block(
            &checkpoint,
            "pub const fn for_ratio(ratio_permille: u16) -> Self"
        )?
    );
    println!(
        "WHOLE_IS_STALE<<{}>>",
        free_function(&constraint, "pub const fn is_stale(band: FreshnessBand)")?
    );
    println!(
        "WHOLE_WORSE<<{}>>",
        free_function(&constraint, "const fn worse(")?
    );
    println!(
        "WHOLE_EVALUATE<<{}>>",
        free_function(&constraint, "pub fn evaluate(")?
    );
    println!(
        "WHOLE_DOMINANCE<<{}>>",
        free_function(&pareto, "pub fn dominance(")?
    );
    println!(
        "WHOLE_ELIMINATE<<{}>>",
        whole_block(
            &pareto,
            "pub fn eliminate(candidates: Vec<Candidate>) -> Self"
        )?
    );
    println!(
        "WHOLE_COMPARE_UNDER<<{}>>",
        free_function(&preference, "fn compare_under(")?
    );
    println!(
        "WHOLE_SLIDER_OF<<{}>>",
        whole_block(&preference, "pub fn of(order: Vec<VectorAxis>)")?
    );
    println!(
        "WHOLE_ESTIMATE_OF<<{}>>",
        whole_block(&vector, "pub fn of(\n        low: u32,")
            .or_else(|_| whole_block(&vector, "pub fn of(low: u32,"))?
    );
    println!(
        "WHOLE_DISCLOSURE_OF<<{}>>",
        whole_block(
            &disclosure,
            "pub const fn of(\n        snapshot: ComputationSnapshot,"
        )
        .or_else(|_| whole_block(&disclosure, "pub const fn of("))?
    );
    println!(
        "WHOLE_PLAN<<{}>>",
        free_function(&engine, "pub fn plan(request: &PlanRequest<'_>)")?
    );
    println!("USE_ITEMS<<{:?}>>", {
        let mut found: BTreeSet<String> = BTreeSet::new();
        for (_, code) in product_code()? {
            found.extend(use_items(&code));
        }
        found
    });
    println!("RE_EXPORT_MODULES<<{:?}>>", {
        let lib = fs::read_to_string(crate_root().join("src/lib.rs"))?;
        re_export_modules(&lib)
            .into_iter()
            .collect::<BTreeSet<String>>()
    });
    println!("REACHED_PATHS<<{:?}>>", {
        let mut reached: BTreeSet<String> = BTreeSet::new();
        for (_, code) in product_code()? {
            reached.extend(absolute_paths(&without_use_items(&code)));
        }
        reached
    });
    println!("MACROS_SPELLED<<{:?}>>", {
        let mut macros: BTreeSet<String> = BTreeSet::new();
        for (_, code) in product_code()? {
            macros.extend(macros_spelled(&code));
        }
        macros
    });
    Ok(())
}
