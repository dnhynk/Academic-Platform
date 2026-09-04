//! What `academic-cs-map` may reach, hold and hand out.
//!
//! ## Why a forbidden-name list is not enough, measured in this run
//!
//! Three bypass classes have been measured in this repository, each of which
//! defeats a list of names and none of which defeats a whole-set comparison:
//!
//! * `P2-R2` measured seven spellings of a filesystem or environment reach that
//!   compile, spell none of the listed tokens and add no `use` item. The repair
//!   was three whole-set comparisons: every `use` item, every two-segment path
//!   reached through a crate root, and every macro invoked.
//! * `P2-Y3` measured a **`From`-impl conversion escaping every `pub fn`
//!   sweep**, because a trait implementation declares no `pub fn` at all. The
//!   repair was pinning every `impl` header in the package.
//! * `P2-N7` measured **five public functions that a five-name list could not
//!   see**, because none of them spelled one of the five names. The repair was a
//!   whole-set inventory of every public signature.
//!
//! All three repairs are here, because all three shapes exist in this crate:
//! [`IMPL_HEADERS`] pins every `impl` header, [`PUBLIC_SIGNATURES`] pins every
//! public signature, and [`USE_ITEMS`], [`REACHED_PATHS`] and [`MACROS_SPELLED`]
//! close the reach.
//!
//! ## The four rules of this task that are pinned rather than described
//!
//! `the_producers_of_a_relevance_are_pinned` compares the whole set of public
//! functions returning a [`academic_cs_map::LensRelevance`] **by value** against
//! two, and pins both signatures: neither names a mastery type, and one is a
//! delegation to the other over the base lens.
//!
//! `no_signature_names_both_a_relevance_and_a_mastery` compares the whole set of
//! public signatures naming both against the empty set, with both halves shown
//! to be separately non-empty and the predicate shown to bite on a fragment that
//! *does* map one to the other.
//!
//! `no_function_returns_a_bare_change_origin` compares the whole set of public
//! signatures returning `ChangeOrigin` outside an `Option` against the empty
//! set, which is what would catch a total conversion from
//! [`academic_cs_map::MapTransition`] added later. `P2-C6` recorded that a scope
//! change has no canonical origin; a total conversion would have to invent one.
//!
//! `every_query_producer_returns_a_reveal` compares the whole set of public
//! functions taking a query string against one, and pins that it returns a
//! [`academic_cs_map::SearchReveal`] rather than a node.

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

/// Every `.rs` of this package, tests included.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` of this package outside `tests/`.
fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    let mut found = crate_all_sources()?;
    found.retain(|path| {
        !path
            .strip_prefix(&root)
            .unwrap_or(path)
            .starts_with("tests")
    });
    Ok(found)
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

/// Collapses whitespace so a rewrapped signature still matches its pin.
fn tighten(code: &str) -> String {
    code.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// Every `use` item of the product tree, one per imported leaf.
fn use_items(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = code;
    while let Some(at) = rest.find("use ") {
        let before_ok = at == 0
            || !(rest.as_bytes()[at - 1].is_ascii_alphanumeric()
                || rest.as_bytes()[at - 1] == b'_');
        let tail = &rest[at + 4..];
        let Some(end) = tail.find(';') else {
            break;
        };
        if before_ok {
            found.insert(tighten(&tail[..end]));
        }
        rest = &tail[end + 1..];
    }
    found
}

/// Drops every `use` item, so an import is not counted as a reach.
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

/// Every two-segment path `code` spells through a crate root.
fn absolute_paths(code: &str) -> BTreeSet<String> {
    let roots = ["std", "core", "alloc", "serde", "thiserror"];
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

/// Every `impl` header of `code`, up to its opening brace.
///
/// This is the sweep `P2-Y3` measured a `From` implementation escaping: a trait
/// impl declares no `pub fn`, so a public-function inventory cannot see it.
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
        found.insert(tighten(&header[..end]));
    }
    found
}

/// Every `pub fn` of `code`, as its name and its signature up to the body.
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
            found.push((name, tighten(&code[at..end])));
            cursor = after;
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Every named field of every `struct` and `enum` `code` declares.
fn declared_fields(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut depth = 0_usize;
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim();
        if !inside {
            let opens = trimmed.starts_with("struct ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub enum ");
            if opens && trimmed.contains('{') {
                inside = true;
                depth = 0;
            } else {
                continue;
            }
        }
        depth += trimmed.matches('{').count();
        let closes = trimmed.matches('}').count();
        if let Some((name, kind)) = trimmed.trim_start_matches("pub ").split_once(':') {
            let named = !name.is_empty()
                && name.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                });
            let kind = kind.trim().trim_end_matches(',');
            if named && !kind.is_empty() && !kind.contains('{') {
                found.insert(format!("{name}: {kind}"));
            }
        }
        depth = depth.saturating_sub(closes);
        if depth == 0 {
            inside = false;
        }
    }
    found
}

// ---------------------------------------------------------------------------
// The pinned inventories
// ---------------------------------------------------------------------------

/// Every `use` item of the product tree.
const USE_ITEMS: &[&str] = &[
    "academic_domain::EntityId",
    "academic_domain::{ ConfidencePermille, EntityId, EpistemicStatus, FreshnessBand, MasteryLevel, predicates::PredicateName, temporal::TimeCoordinates, }",
    "academic_domain::{ EntityId, predicates::{NodeType, PredicateName}, }",
    "academic_domain::{ EntityId, temporal::{ChangeOrigin, TimeCoordinates}, }",
    "academic_domain::{ConfidencePermille, EntityId, EpistemicStatus, predicates::PredicateName}",
    "academic_domain::{EntityId, predicates::NodeType}",
    "anchor::{YOU_REFERENCE_LABEL, YouAnchor}",
    "atlas::{ Atlas, Coordinate, InitialView, LAYOUT_TOLERANCE_MILLI, Landmark, LandmarkDrift, LevelView, MAX_INITIAL_CLUSTERS, MIN_INITIAL_CLUSTERS, Placement, SEMANTIC_ZOOMS, SemanticZoom, lay_out, }",
    "budget::{ATLAS_BUDGET, BUDGET_MEASURES, BudgetMeasure, BudgetReading, RenderBudget}",
    "crate::CsMapError",
    "crate::{ CsMapError, atlas::{Atlas, Coordinate}, }",
    "crate::{ CsMapError, encoding::NodeReading, graph::{MapEdge, MapGraph}, }",
    "crate::{ CsMapError, encoding::{LensRelevance, VISUAL_CHANNELS, VisualChannel}, }",
    "crate::{ CsMapError, graph::{ClusterId, MapGraph}, }",
    "encoding::{ AsOfBadge, BorderPattern, ChannelFrame, ChannelSubject, ChannelValue, DashPattern, EdgeStroke, EncodedEdge, EncodedNode, FreshnessRing, GLYPH_MARKS, GlyphMark, HaloState, LENS_RELEVANCES, LensRelevance, MasteryFill, NodeReading, VISUAL_CHANNELS, VisualChannel, encode_edge, encode_node, }",
    "focus::{FOCUS_KINDS, FocusKind, FocusMode, HopCount, MAX_HOPS, MIN_HOPS, Subgraph, focus}",
    "graph::{ClusterId, MapEdge, MapGraph, MapNode}",
    "lens::{ LayerCollision, Legend, LegendEntry, LensComposition, LensSubject, MAP_LENSES, MAX_OVERLAYS, MapLens, relevance_of, }",
    "scrubber::{ Appearance, MAP_TRANSITIONS, MapDelta, MapEvent, MapProjection, MapTransition, SplitComparison, Timeline, TransitionPattern, }",
    "search::{REVEAL_STAGES, RevealStage, SearchReveal, reveal}",
    "serde::Serialize",
    "serde::{Serialize, Serializer}",
    "std::collections::BTreeSet",
    "std::collections::{BTreeMap, BTreeSet, VecDeque}",
    "std::collections::{BTreeMap, BTreeSet}",
    "std::{ collections::{BTreeMap, BTreeSet}, fmt, }",
    "thiserror::Error",
];

/// Every two-segment path the product tree reaches through a crate root.
const REACHED_PATHS: &[&str] = &["serde::Serializer"];

/// Every macro the product tree invokes.
const MACROS_SPELLED: &[&str] = &["matches", "vec"];

/// Constructs no file of this package may spell, tests included.
///
/// The weakest of the three layers and kept anyway, because it names the shapes
/// a reader expects to see refused. The whole-set comparisons above are what
/// actually close the reach.
const FORBIDDEN_CONSTRUCTS: &[&str] = &[
    "File",
    "OpenOptions",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "UnixStream",
    "Command",
    "SystemTime",
    "Instant",
    "now",
    "var",
    "var_os",
    "set_var",
    "current_dir",
    "extern",
    "unsafe",
    "libc",
    "rand",
    "thread_rng",
];

// ---------------------------------------------------------------------------
// The scans
// ---------------------------------------------------------------------------

/// Every module the compiler pulls in is a file the walk read.
///
/// Without it, a reach could be moved into a file the walk never visits and
/// every scan below would pass over a package it had not read.
#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let read: BTreeSet<String> = crate_all_sources()?
        .iter()
        .map(|path| relative(path))
        .collect();
    assert!(read.len() >= 12, "the walk found only {} files", read.len());

    let mut declared: BTreeSet<String> = BTreeSet::new();
    for path in crate_all_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let directory = path.parent().unwrap_or(&path).to_path_buf();
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        for line in code.lines() {
            let trimmed = line.trim();
            let body = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
            let Some(rest) = body.strip_prefix("mod ") else {
                continue;
            };
            let Some(name) = rest.strip_suffix(';') else {
                continue;
            };
            let name = name.trim();
            // `lib.rs`, `mod.rs` and every integration-test root resolve a
            // child module beside themselves; a plain module file resolves it
            // in a directory of its own name.
            let is_root = stem == "lib"
                || stem == "mod"
                || directory.file_name().is_some_and(|last| last == "tests");
            let sibling = if is_root {
                directory.join(format!("{name}.rs"))
            } else {
                directory.join(&stem).join(format!("{name}.rs"))
            };
            let nested = if is_root {
                directory.join(name).join("mod.rs")
            } else {
                directory.join(&stem).join(name).join("mod.rs")
            };
            let target = if sibling.exists() { sibling } else { nested };
            declared.insert(relative(&target));
        }
    }
    assert!(
        !declared.is_empty(),
        "no module declaration was found at all"
    );
    for module in &declared {
        assert!(read.contains(module), "{module} is compiled and not read");
    }

    // No `#[path]` attribute anywhere: this package includes nothing from
    // outside its own tree, so the walk is the whole compilation unit.
    for (path, code) in product_code()? {
        assert!(!code.contains("#[path"), "{path} includes a file by path");
    }
    Ok(())
}

/// The crate opens nothing, reads no clock and reaches nothing it does not
/// declare.
#[test]
fn the_cs_map_crate_touches_no_file_and_no_socket() -> TestResult {
    let files = product_code()?;
    assert!(
        files.len() >= 10,
        "only {} product files were read",
        files.len()
    );
    let whole: String = files
        .iter()
        .map(|(_, code)| code.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let items = use_items(&whole);
    assert_eq!(
        items,
        USE_ITEMS.iter().map(|item| (*item).to_owned()).collect(),
        "the use-item inventory and the source disagree"
    );

    let reached = absolute_paths(&without_use_items(&whole));
    assert_eq!(
        reached,
        REACHED_PATHS
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the reached-path inventory and the source disagree"
    );

    let macros = macros_spelled(&whole);
    assert_eq!(
        macros,
        MACROS_SPELLED
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the macro inventory and the source disagree"
    );

    // The whole package, tests included: nothing here opens a file, a socket, a
    // process or a clock.
    let mut read = 0_usize;
    for path in crate_all_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        read += 1;
        for construct in FORBIDDEN_CONSTRUCTS {
            assert_eq!(
                uses_of(&code, construct),
                0,
                "{} spells {construct}",
                relative(&path)
            );
        }
    }
    assert!(
        read >= 12,
        "the forbidden-construct pass read only {read} files"
    );

    // **The control.** A reader that always answers zero would satisfy every
    // assertion above. Each of the nineteen constructs is required to be found
    // by the same reader, through the same stripper, in a sample that does spell
    // it -- and to be found in *code* rather than only in a literal, because
    // `strip_non_code` removes literals and a construct that only ever appeared
    // inside one would have been invisible either way.
    for construct in FORBIDDEN_CONSTRUCTS {
        let sample = format!(
            "let value = {construct}(path);
"
        );
        assert_eq!(
            uses_of(&strip_non_code(&sample), construct),
            1,
            "the reader cannot find {construct} in a sample that spells it"
        );
        let quoted = format!(
            "let value = \"{construct}\";
"
        );
        assert_eq!(
            uses_of(&strip_non_code(&quoted), construct),
            0,
            "the stripper does not remove {construct} from a string literal"
        );
    }
    Ok(())
}

/// Every `impl` header this crate declares is in the inventory, both ways.
///
/// This is `P2-Y3`'s bypass class closed for this crate's types. A `From`,
/// `Into`, `Deref`, `AsRef`, `TryFrom` or `Borrow` between an opacity and a
/// mastery, or between a map transition and a change origin, would be an entry
/// here and nowhere else — no `pub fn` sweep can see one.
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

    // No conversion trait at all is implemented in this crate, for any type.
    // Stated as a property of the whole inventory rather than of a list of type
    // pairs, so a conversion between two types nobody thought of is refused too.
    for header in &found {
        for conversion in [
            "From<",
            "Into<",
            "Deref",
            "DerefMut",
            "AsRef<",
            "AsMut<",
            "Borrow<",
            "BorrowMut<",
            "TryFrom<",
            "TryInto<",
            "FromStr",
            "FromIterator<",
        ] {
            assert!(
                !header.contains(conversion),
                "{header} implements {conversion}, which is a conversion this crate does not admit"
            );
        }
    }

    // The scanner is not vacuous: it finds a conversion in a fragment that has
    // one, and it finds the trait impls this crate really does declare.
    let fragment = "impl From<MasteryFill> for LensRelevance {
    fn from(_: MasteryFill) -> Self { LensRelevance::Central }
}";
    assert_eq!(
        impl_headers(fragment),
        ["impl From<MasteryFill> for LensRelevance"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(found.iter().any(|header| header.contains("fmt::Display")));
    Ok(())
}

/// Every public signature of the crate is in the inventory, both ways.
///
/// This is `P2-N7`'s bypass class closed: a name list cannot see a function that
/// spells none of its names, and a whole-set inventory sees every function
/// whatever it is called.
#[test]
fn every_public_signature_is_in_the_inventory() -> TestResult {
    let found = all_public_signatures()?;
    let rendered: Vec<String> = found
        .iter()
        .map(|(name, signature)| format!("{name} | {signature}"))
        .collect();
    assert_eq!(
        rendered,
        PUBLIC_SIGNATURES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "the public-signature inventory and the source disagree"
    );
    assert!(
        found.len() >= 70,
        "only {} public functions were found",
        found.len()
    );
    Ok(())
}

fn all_public_signatures() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for (_, code) in product_code()? {
        found.extend(public_signatures(&code));
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// The whole set of public functions producing a relevance, and what they name.
///
/// Two, and neither parameter list names a mastery type. `relevance` delegates
/// to `relevance_of` over the base lens, which is why the composition has
/// exactly one base.
#[test]
fn the_producers_of_a_relevance_are_pinned() -> TestResult {
    let producers: Vec<(String, String)> = all_public_signatures()?
        .into_iter()
        .filter(|(_, signature)| signature.contains("-> LensRelevance"))
        .collect();
    assert_eq!(
        producers,
        vec![
            (
                "relevance".to_owned(),
                "pub fn relevance(&self, subject: &LensSubject) -> LensRelevance".to_owned()
            ),
            (
                "relevance_of".to_owned(),
                "pub fn relevance_of(lens: MapLens, subject: &LensSubject) -> LensRelevance"
                    .to_owned()
            ),
        ],
        "the set of relevance producers changed"
    );

    for (name, signature) in &producers {
        for mastery in ["MasteryLevel", "MasteryFill", "mastery"] {
            assert_eq!(
                uses_of(signature, mastery),
                0,
                "{name} names {mastery} in its signature"
            );
        }
    }
    Ok(())
}

/// No public signature names both an opacity and a mastery.
///
/// Both halves of the conjunction are shown to be separately non-empty, so the
/// empty intersection is a fact rather than a predicate that matches nothing,
/// and the predicate is shown to bite on a fragment that does map one to the
/// other.
#[test]
fn no_signature_names_both_a_relevance_and_a_mastery() -> TestResult {
    let signatures = all_public_signatures()?;
    let names_relevance = |signature: &str| {
        uses_of(signature, "LensRelevance") > 0 || uses_of(signature, "relevance") > 0
    };
    let names_mastery = |signature: &str| {
        uses_of(signature, "MasteryLevel") > 0
            || uses_of(signature, "MasteryFill") > 0
            || uses_of(signature, "mastery") > 0
    };

    let relevance: Vec<&String> = signatures
        .iter()
        .filter(|(_, signature)| names_relevance(signature))
        .map(|(name, _)| name)
        .collect();
    let mastery: Vec<&String> = signatures
        .iter()
        .filter(|(_, signature)| names_mastery(signature))
        .map(|(name, _)| name)
        .collect();
    assert!(
        !relevance.is_empty(),
        "no signature names an opacity at all"
    );
    assert!(!mastery.is_empty(), "no signature names a mastery at all");

    let both: Vec<&String> = signatures
        .iter()
        .filter(|(_, signature)| names_relevance(signature) && names_mastery(signature))
        .map(|(name, _)| name)
        .collect();
    assert!(
        both.is_empty(),
        "{both:?} name both an opacity and a mastery"
    );

    // The predicate bites.
    let offender = "pub fn opacity_for(mastery: MasteryLevel) -> LensRelevance";
    assert!(names_relevance(offender) && names_mastery(offender));
    Ok(())
}

/// No public function hands back a bare canonical change origin.
///
/// `MapTransition::change_origin` returns an `Option`, and the `None` arm is the
/// scope change. A function returning a bare `ChangeOrigin` would be the total
/// conversion `P2-C6` refused, and it would have to invent an origin for a
/// display setting.
#[test]
fn no_function_returns_a_bare_change_origin() -> TestResult {
    let signatures = all_public_signatures()?;
    let bare: Vec<&String> = signatures
        .iter()
        .filter(|(_, signature)| {
            signature.contains("-> ChangeOrigin") || signature.contains("-> Result<ChangeOrigin")
        })
        .map(|(name, _)| name)
        .collect();
    assert!(bare.is_empty(), "{bare:?} return a bare change origin");

    // The optional form exists, so the filter is looking at something.
    let optional: Vec<&String> = signatures
        .iter()
        .filter(|(_, signature)| signature.contains("-> Option<ChangeOrigin>"))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(optional, vec!["change_origin"]);

    // The filter bites on a fragment that has the shape it refuses.
    let offender = "pub const fn origin(self) -> ChangeOrigin";
    assert!(offender.contains("-> ChangeOrigin"));
    Ok(())
}

/// Every public function taking a query returns a guided reveal.
#[test]
fn every_query_producer_returns_a_reveal() -> TestResult {
    let signatures = all_public_signatures()?;
    let takers: Vec<(String, String)> = signatures
        .into_iter()
        .filter(|(_, signature)| signature.contains("query: &str"))
        .collect();
    assert_eq!(
        takers,
        vec![(
            "reveal".to_owned(),
            "pub fn reveal( graph: &MapGraph, standing_at: EntityId, query: &str, ) -> Result<SearchReveal, CsMapError>".to_owned()
        )],
        "the set of query-taking functions changed"
    );
    Ok(())
}

/// Nothing in this crate takes `&mut self`.
///
/// A map is a derivation over values somebody else froze. A method that edited
/// one in place would let a view diverge from the graph it was drawn from with
/// nothing recording that it had.
#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    for (path, code) in product_code()? {
        assert_eq!(
            uses_of(&code, "mut self"),
            0,
            "{path} takes a mutable receiver"
        );
    }
    Ok(())
}

/// Every declared field is in the inventory, and none of them is a float.
///
/// A coordinate is thousandths of a unit in an `i32` and a confidence is a
/// permille. There is no `f32` and no `f64` anywhere, so an opacity expressed as
/// a fraction has no type to arrive in whatever the field is called.
#[test]
fn every_field_of_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        found.extend(declared_fields(&code));
    }
    assert_eq!(
        found,
        DECLARED_FIELDS
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the field inventory and the source disagree"
    );

    for (path, code) in product_code()? {
        for float in ["f32", "f64"] {
            assert_eq!(uses_of(&code, float), 0, "{path} declares an {float}");
        }
    }
    Ok(())
}

/// This scan is named in the source-scan inventory.
#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(
        workspace_root()
            .join("docs")
            .join("contracts")
            .join("policy-source-scans.md"),
    )?;
    assert!(
        page.contains("crates/cs-map/tests/cs_map_scans.rs"),
        "the source-scan inventory does not name this file"
    );
    for named in [
        "the_walk_reads_every_module_in_this_package",
        "the_cs_map_crate_touches_no_file_and_no_socket",
        "every_impl_header_in_this_crate_is_in_the_inventory",
        "every_public_signature_is_in_the_inventory",
    ] {
        assert!(page.contains(named), "the inventory does not name {named}");
    }
    Ok(())
}

/// The scanners find what they are supposed to find.
///
/// Each is run over a fragment whose answer is known, including the shapes a
/// naive scanner misses: a `use` inside a word, a middle path segment, a macro
/// name that is a keyword, and a `-> Result<Self, _>` signature.
#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(
        use_items("use std::fmt; fn refuse() { let refuse_item = 1; }"),
        ["std::fmt"].into_iter().map(str::to_owned).collect()
    );
    assert_eq!(
        absolute_paths("std::fs::read(a::b::c)"),
        ["std::fs"].into_iter().map(str::to_owned).collect()
    );
    assert_eq!(
        without_use_items("use std::fmt;\nstd::fs::read(p);\n"),
        "std::fs::read(p);\n"
    );
    assert!(absolute_paths(&without_use_items("use std::fmt;\n")).is_empty());
    assert_eq!(
        macros_spelled("format!(\"x\") if (true) {} vec![1]"),
        ["format", "vec"].into_iter().map(str::to_owned).collect()
    );
    assert_eq!(
        public_signatures("    pub fn new(hops: u8) -> Result<Self, CsMapError> {\n")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        vec!["new".to_owned()]
    );
    assert_eq!(
        declared_fields("pub struct A {\n    pub x_milli: i32,\n    inner: BTreeSet<EntityId>,\n}"),
        ["x_milli: i32", "inner: BTreeSet<EntityId>"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(uses_of("a_now now nowhere", "now"), 1);
    assert_eq!(
        strip_non_code("let a = \"File\"; // Command\n"),
        "let a =  ; \n\n"
    );
    Ok(())
}

/// Every `impl` header the product tree declares.
///
/// The whole set, in both directions. `P2-Y3` measured a `From`
/// implementation escaping every public-function sweep, because a trait
/// impl declares no `pub fn`. Adding one here is an edit to this list.
const IMPL_HEADERS: &[&str] = &[
    "impl AsOfBadge",
    "impl Atlas",
    "impl BorderPattern",
    "impl BudgetMeasure",
    "impl BudgetReading",
    "impl ChannelFrame",
    "impl ChannelValue",
    "impl ClusterId",
    "impl Coordinate",
    "impl EdgeStroke",
    "impl FocusKind",
    "impl FocusMode",
    "impl FreshnessRing",
    "impl GlyphMark",
    "impl HopCount",
    "impl InitialView",
    "impl LandmarkDrift",
    "impl Legend",
    "impl LensComposition",
    "impl MapGraph",
    "impl MapLens",
    "impl MapNode",
    "impl MapTransition",
    "impl MasteryFill",
    "impl SearchReveal",
    "impl SemanticZoom",
    "impl Timeline",
    "impl VisualChannel",
    "impl YouAnchor",
    "impl fmt::Display for ClusterId",
];

/// Every public signature the product tree declares, as `name | signature`.
///
/// The whole set, in both directions. `P2-N7` measured five public
/// functions a five-name list could not see; an inventory sees a function
/// whatever it is called.
const PUBLIC_SIGNATURES: &[&str] = &[
    "anchor | pub fn anchor(&self, cluster: ClusterId) -> Option<Coordinate>",
    "as_str | pub const fn as_str(self) -> &'static str",
    "at | pub const fn at(&self) -> Coordinate",
    "at | pub const fn at(coordinates: TimeCoordinates) -> Self",
    "badge | pub const fn badge(self) -> &'static str",
    "band | pub const fn band(self) -> FreshnessBand",
    "base | pub const fn base(base: MapLens) -> Self",
    "base_lens | pub const fn base_lens(&self) -> MapLens",
    "change_origin | pub const fn change_origin(self) -> Option<ChangeOrigin>",
    "channel | pub const fn channel(&self) -> VisualChannel",
    "channel_values | pub fn channel_values(&self) -> [ChannelValue; VISUAL_CHANNELS.len()]",
    "channels | pub fn channels(&self) -> Vec<VisualChannel>",
    "claimed_channel | pub const fn claimed_channel(self) -> Option<VisualChannel>",
    "cluster | pub const fn cluster(&self) -> ClusterId",
    "clusters | pub fn clusters(&self) -> &[ClusterId]",
    "collision | pub fn collision(&self) -> Option<LayerCollision>",
    "compare | pub fn compare( &self, left: TimeCoordinates, right: TimeCoordinates, ) -> Result<SplitComparison, CsMapError>",
    "composed | pub fn composed(&self) -> Vec<MapLens>",
    "declare | pub fn declare( id: EntityId, node_type: NodeType, label: impl Into<String>, cluster: ClusterId, ) -> Result<Self, CsMapError>",
    "declare | pub fn declare(mut events: Vec<MapEvent>) -> Result<Self, CsMapError>",
    "declare | pub fn declare(nodes: Vec<MapNode>, edges: Vec<MapEdge>) -> Result<Self, CsMapError>",
    "displacement | pub const fn displacement(self, other: Self) -> i64",
    "draw | pub fn draw( reading: &NodeReading, relevance: LensRelevance, as_of: AsOfBadge, edge: EncodedEdge, ) -> Result<Self, CsMapError>",
    "edges | pub const fn edges(&self) -> &BTreeSet<MapEdge>",
    "encode_edge | pub const fn encode_edge( from: EntityId, to: EntityId, predicate: PredicateName, status: EpistemicStatus, ) -> EncodedEdge",
    "encode_node | pub fn encode_node( reading: &NodeReading, relevance: LensRelevance, as_of: AsOfBadge, ) -> EncodedNode",
    "entity | pub const fn entity(self) -> EntityId",
    "entries | pub fn entries(&self) -> &[LegendEntry]",
    "events | pub fn events(&self) -> &[MapEvent]",
    "focus | pub fn focus( graph: &MapGraph, readings: &BTreeMap<EntityId, NodeReading>, mode: &FocusMode, ) -> Result<Subgraph, CsMapError>",
    "furthest | pub const fn furthest(&self) -> i64",
    "growth_band | pub const fn growth_band(&self) -> usize",
    "growth_band | pub fn growth_band(node_count: usize) -> usize",
    "horizon | pub const fn horizon(self) -> Option<u8>",
    "id | pub const fn id(&self) -> EntityId",
    "initial_view | pub fn initial_view( &self, graph: &MapGraph, goal: EntityId, ) -> Result<InitialView, CsMapError>",
    "is_pinned | pub const fn is_pinned(&self) -> bool",
    "key | pub const fn key(self) -> &'static str",
    "kind | pub const fn kind(&self) -> FocusKind",
    "label | pub const fn label(&self) -> &'static str",
    "label | pub const fn label(self) -> &'static str",
    "label | pub fn label(&self) -> &str",
    "landmark_drift | pub fn landmark_drift(&self, other: &Self) -> LandmarkDrift",
    "landmarks | pub fn landmarks(&self) -> &[Landmark]",
    "lay_out | pub fn lay_out(graph: &MapGraph) -> Result<Atlas, CsMapError>",
    "legend | pub fn legend(&self) -> Legend",
    "level | pub const fn level(self) -> MasteryLevel",
    "level | pub fn level( &self, graph: &MapGraph, zoom: SemanticZoom, goal: EntityId, ) -> Result<LevelView, CsMapError>",
    "materialised | pub fn materialised(&self) -> BTreeSet<EntityId>",
    "members | pub fn members(&self, cluster: ClusterId) -> BTreeSet<EntityId>",
    "new | pub fn new(hops: u8) -> Result<Self, CsMapError>",
    "node | pub const fn node(&self) -> EntityId",
    "node | pub fn node(&self, id: EntityId) -> Option<&MapNode>",
    "node_count | pub fn node_count(&self) -> usize",
    "node_type | pub const fn node_type(&self) -> NodeType",
    "nodes | pub fn nodes(&self) -> impl Iterator<Item = &MapNode>",
    "of | pub const fn of(band: FreshnessBand) -> Self",
    "of | pub const fn of(level: MasteryLevel) -> Self",
    "of | pub const fn of(predicate: PredicateName, status: EpistemicStatus) -> Self",
    "of | pub fn of(status: EpistemicStatus, confidence: Option<ConfidencePermille>) -> Self",
    "of_field | pub const fn of_field(field: EntityId) -> Self",
    "over | pub fn over(references: BTreeSet<EntityId>, atlas: &Atlas) -> Result<Self, CsMapError>",
    "overlay | pub fn overlay(self, lens: MapLens) -> Result<Self, CsMapError>",
    "overlays | pub fn overlays(&self) -> &[MapLens]",
    "path | pub fn path(&self) -> &[EntityId]",
    "pattern | pub const fn pattern(self) -> TransitionPattern",
    "placement | pub fn placement(&self, node: EntityId) -> Option<Coordinate>",
    "placements | pub fn placements(&self) -> impl Iterator<Item = Placement> + '_",
    "project | pub fn project(&self, at: TimeCoordinates) -> MapProjection",
    "record_moved | pub const fn record_moved(self) -> bool",
    "references | pub const fn references(&self) -> &BTreeSet<EntityId>",
    "relevance | pub fn relevance(&self, subject: &LensSubject) -> LensRelevance",
    "relevance_of | pub fn relevance_of(lens: MapLens, subject: &LensSubject) -> LensRelevance",
    "reveal | pub fn reveal( graph: &MapGraph, standing_at: EntityId, query: &str, ) -> Result<SearchReveal, CsMapError>",
    "screen_reader_name | pub const fn screen_reader_name(self) -> &'static str",
    "shown_types | pub fn shown_types(self) -> BTreeSet<NodeType>",
    "spec_bullet_head | pub const fn spec_bullet_head(self) -> &'static str",
    "spec_label | pub const fn spec_label(self) -> &'static str",
    "spec_name | pub const fn spec_name(self) -> &'static str",
    "stages | pub fn stages(&self) -> [RevealStage; REVEAL_STAGES]",
    "subject | pub const fn subject(self) -> ChannelSubject",
    "symbol | pub const fn symbol(self) -> &'static str",
    "value | pub const fn value(self) -> u8",
    "vanished | pub const fn vanished(&self) -> &BTreeSet<ClusterId>",
    "within | pub fn within(&self, budget: &RenderBudget) -> Result<(), CsMapError>",
    "within_tolerance | pub fn within_tolerance(&self) -> Result<(), CsMapError>",
    "work_units | pub const fn work_units(&self) -> usize",
];

/// Every named field the product tree declares, as `name: type`.
///
/// The whole set, in both directions. No entry is an `f32` or an `f64`,
/// so an opacity expressed as a fraction has no type to arrive in.
const DECLARED_FIELDS: &[&str] = &[
    "anchors: BTreeMap<ClusterId, Coordinate>",
    "appearance: Appearance",
    "at: Coordinate",
    "at: TimeCoordinates",
    "band: usize",
    "base: &'static str",
    "base: MapLens",
    "below: ConfidencePermille",
    "border_pattern: BorderPattern",
    "ceiling: usize",
    "centre: EntityId",
    "channel: VisualChannel",
    "claimants: Vec<MapLens>",
    "claimed_by: Vec<MapLens>",
    "cluster: ClusterId",
    "clusters: Vec<ClusterId>",
    "confidence: Option<ConfidencePermille>",
    "count: usize",
    "coverage: BTreeMap<EntityId, CoverageSide>",
    "dash: DashPattern",
    "deltas: Vec<MapDelta>",
    "edge: EncodedEdge",
    "edge_stroke: EdgeStroke",
    "edge_types: BTreeSet<PredicateName>",
    "edges: BTreeSet<MapEdge>",
    "entered: BTreeMap<EntityId, MapTransition>",
    "entries: Vec<LegendEntry>",
    "events: Vec<MapEvent>",
    "evidence_nodes: usize",
    "first: &'static str",
    "freshness: FreshnessBand",
    "from: EntityId",
    "furthest: i64",
    "glyph: Vec<GlyphMark>",
    "goal: EntityId",
    "goal_near_nodes: usize",
    "goal_neighbourhood: BTreeSet<EntityId>",
    "halo: HaloState",
    "hops: HopCount",
    "hops: u8",
    "id: EntityId",
    "initial_view_nodes: usize",
    "kind: FocusKind",
    "known_at_accept_seq: u64",
    "label: String",
    "landmarks: Vec<Landmark>",
    "layout_work_units: usize",
    "layout_work_units_per_node: usize",
    "left: MapProjection",
    "lens: &'static str",
    "marks: BTreeSet<GlyphMark>",
    "mastery: MasteryLevel",
    "matches: usize",
    "measure: &'static str",
    "measured: usize",
    "moved: i64",
    "named_by: BTreeSet<MapLens>",
    "node: EncodedNode",
    "node: EntityId",
    "node_count: usize",
    "node_fill: MasteryFill",
    "node_type: &'static str",
    "node_type: NodeType",
    "nodes: BTreeMap<EntityId, MapNode>",
    "nodes: BTreeSet<EntityId>",
    "offering_lectures: BTreeSet<EntityId>",
    "on_active_critical_path: bool",
    "opacity: LensRelevance",
    "outer_ring: FreshnessRing",
    "overlays: Vec<MapLens>",
    "path: Vec<EntityId>",
    "pinned: bool",
    "placements: BTreeMap<EntityId, Coordinate>",
    "predicate: PredicateName",
    "query: String",
    "reached_by: BTreeSet<MapLens>",
    "references: BTreeSet<EntityId>",
    "refused: &'static str",
    "revision: EntityId",
    "right: MapProjection",
    "search_path_hops: usize",
    "second: &'static str",
    "status: EpistemicStatus",
    "subject: EntityId",
    "timestamp_badge: AsOfBadge",
    "to: EntityId",
    "tolerance: i64",
    "transition: MapTransition",
    "types: BTreeSet<NodeType>",
    "valid_at_millis: i64",
    "vanished: BTreeSet<ClusterId>",
    "visible: BTreeSet<EntityId>",
    "work_units: usize",
    "x_milli: i32",
    "y_milli: i32",
    "zoom: SemanticZoom",
];
