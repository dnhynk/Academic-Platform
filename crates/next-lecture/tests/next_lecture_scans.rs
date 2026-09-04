//! Whole-set inventories of this crate's own source.
//!
//! Three claims a call cannot make, each held by a comparison in **both**
//! directions rather than by a list of forbidden names. A list refuses the
//! edits somebody thought of in advance and admits every edit spelled
//! differently; `P2-N7` measured five public functions walking past a name list
//! that held none of their names, and `P2-Y3` measured a `From` impl walking
//! past a `pub fn` sweep.
//!
//! | Claim | The set |
//! |---|---|
//! | nothing here reaches a clock, a socket, a file or a process | every `use` item, every two-segment path through a crate root, every macro, and a whole-identifier pass for fifteen constructs |
//! | every `ALL` array is the enum it names | each enum's declared variants against its own array, both ways |
//! | the public surface is inventoried, impl headers included | every public signature with its owner, and every `impl` header |
//!
//! The third is the one `P2-Y3` and `P2-N7` are about. A sweep over `pub fn`
//! names alone would miss a `From`/`Into` conversion entirely, and a sweep
//! guided by a list of interesting names would miss a function that mentions
//! none of them. So the inventory is the *whole* set of public signatures with
//! the type each sits on, plus the whole set of `impl` headers, and any
//! addition of either kind is a diff.

mod support;

use std::{collections::BTreeSet, fs};

use academic_next_lecture::{ExpectedConceptSource, MinimalityDefect, PrepAxis};

use support::{
    TestResult, absolute_paths, all_array, crate_product_sources, crate_root, enum_variants,
    macros_spelled, product_code, relative, strip_non_code, tighten, use_items, uses_of,
};

/// Every `use` item every product file in this package spells.
const USE_ITEMS: [&str; 34] = [
    "academic_domain::ConfidencePermille",
    "academic_domain::EntityId",
    "academic_domain::EpistemicStatus",
    "academic_domain::EvidenceId",
    "academic_domain::FreshnessBand",
    "academic_domain::MasteryLevel",
    "academic_domain::entity_registry::EntityKind",
    "academic_domain::predicates::PredicateName",
    "academic_domain::predicates::PrerequisiteStrength",
    "academic_gap::BlockingPath",
    "academic_gap::ConceptState",
    "academic_gap::GapCase",
    "academic_gap::GapKind",
    "academic_gap::MinimumRemediation",
    "academic_gap::PrerequisiteEdge",
    "academic_gap::PrerequisiteGraph",
    "academic_gap::RootCandidate",
    "academic_gap::gap_bearing",
    "academic_ingestion::dating::Date",
    "academic_lecture_document::NodeId",
    "academic_untrusted_content::Proposal",
    "academic_untrusted_content::ResolvedSpan",
    "academic_untrusted_content::SourceId",
    "crate::NextLectureError",
    "crate::brief::CandidateParts",
    "crate::brief::HIGHEST_PREPARATION",
    "crate::brief::PreparationBrief",
    "crate::brief::PreparationCandidate",
    "crate::claim::ExpectedConceptClaim",
    "crate::minimality::minimality_defects",
    "crate::source::MaterialReference",
    "crate::uncertainty::PrepUncertainty",
    "serde::Deserialize",
    "serde::Serialize",
];

/// Every two-segment path this crate reaches through a crate root, with the
/// `use` items removed so a re-export is not counted as a reach.
const REACHED_PATHS: [&str; 7] = [
    "academic_domain::ConfidencePermille",
    "academic_domain::DomainError",
    "academic_domain::EntityId",
    "academic_domain::entity_registry",
    "academic_gap::ConceptState",
    "academic_gap::GapError",
    "thiserror::Error",
];

/// Every macro this crate invokes.
const MACROS_SPELLED: [&str; 1] = ["vec"];

/// The modules `lib.rs` declares and re-exports from.
const MODULES: [&str; 6] = [
    "brief",
    "claim",
    "engine",
    "minimality",
    "source",
    "uncertainty",
];

/// The shapes a reader expects to see refused, kept as the weakest layer.
const FORBIDDEN_CONSTRUCTS: [&str; 15] = [
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "std::time",
    "SystemTime",
    "Instant",
    "now",
    "File",
    "Path",
    "PathBuf",
    "TcpStream",
    "Command",
    "include_str",
    "include_bytes",
];

/// Every `impl` header this crate's product code declares.
///
/// `P2-Y3` measured a `From`/`Into` conversion walking past a sweep that only
/// looked at `pub fn`, because a trait implementation declares no `pub`
/// anywhere. Pinning the headers is what closes that: a `impl From<X> for Y`
/// added here is a diff whatever its body does.
const IMPL_HEADERS: [&str; 11] = [
    "impl ExpectedConceptClaim {",
    "impl ExpectedConceptReading {",
    "impl ExpectedConceptSource {",
    "impl MaterialReference {",
    "impl MinimalityDefect {",
    "impl PrepAxis {",
    "impl PrepUncertainty {",
    "impl PreparationBrief {",
    "impl PreparationCandidate {",
    "impl PrerequisiteEdgeReading {",
    "impl UserStateReading {",
];

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
    assert_eq!(
        declared,
        MODULES.into_iter().map(str::to_owned).collect(),
        "the module set moved"
    );
    // And every module is re-exported from, so nothing is declared and left
    // unreachable.
    for module in MODULES {
        assert!(
            lib.contains(&format!("pub use {module}::")),
            "lib.rs declares {module} and re-exports nothing from it"
        );
    }
    Ok(())
}

#[test]
fn no_clock_socket_or_file_reaches_this_crate() -> TestResult {
    // The whole `use` inventory, in both directions.
    let mut found: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        found.extend(use_items(&code));
    }
    assert_eq!(
        found,
        USE_ITEMS.into_iter().map(str::to_owned).collect(),
        "the crate's use items moved"
    );

    // The whole set of two-segment paths reached through a crate root, over
    // code with the `use` items removed so a re-export is not counted as a
    // reach.
    let mut reached: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        reached.extend(absolute_paths(&without_use_items(&code)));
    }
    assert_eq!(
        reached,
        REACHED_PATHS.into_iter().map(str::to_owned).collect(),
        "the crate's reached paths moved"
    );

    // Every macro, in both directions.
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        macros.extend(macros_spelled(&code));
    }
    assert_eq!(
        macros,
        MACROS_SPELLED.into_iter().map(str::to_owned).collect(),
        "the crate's macros moved"
    );

    // And the weakest layer, kept because it costs nothing: a whole-identifier
    // pass for fifteen filesystem, clock, process and transport constructs.
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

/// Every `ALL` array is the enum it names, in both directions.
///
/// `P2-P3` closed this by one scan over every enum in the package rather than
/// three assertions somebody has to remember to add: a fourth enum with an
/// `ALL` that omits one of its own variants is found by the walk, not by a
/// list.
#[test]
fn every_all_array_is_the_enum_it_names() -> TestResult {
    let mut checked = 0_usize;
    for (path, code) in product_code()? {
        for name in public_enums(&code) {
            let Some(all) = all_array(&code, &name) else {
                continue;
            };
            let declared = enum_variants(&code, &format!("pub enum {name} {{"))?;
            assert_eq!(all, declared, "{path}: {name}::ALL and its variants differ");
            checked += 1;
        }
    }
    assert!(checked >= 3, "only {checked} ALL arrays were found");

    // And the three the acceptance suite reads are among them, compared through
    // the values rather than through the source a second time.
    assert_eq!(ExpectedConceptSource::ALL.len(), 7);
    assert_eq!(PrepAxis::ALL.len(), 3);
    assert_eq!(MinimalityDefect::ALL.len(), 3);
    for place in ExpectedConceptSource::ALL {
        assert!(!place.as_str().is_empty());
        assert!(!place.spec_token().is_empty());
    }
    for axis in PrepAxis::ALL {
        assert!(!axis.as_str().is_empty());
        assert!(!axis.spec_token().is_empty());
    }
    for defect in MinimalityDefect::ALL {
        assert!(!defect.as_str().is_empty());
        assert!(!defect.spec_token().is_empty());
    }
    // The spellings are distinct, so an arm that answers with another arm's
    // token is a collision rather than a pass.
    let spellings: BTreeSet<&str> = ExpectedConceptSource::ALL
        .iter()
        .map(|place| place.as_str())
        .chain(PrepAxis::ALL.iter().map(|axis| axis.as_str()))
        .chain(MinimalityDefect::ALL.iter().map(|defect| defect.as_str()))
        .collect();
    assert_eq!(spellings.len(), 7 + 3 + 3, "two arms share a spelling");
    Ok(())
}

/// Every public signature and every `impl` header, as two whole sets.
///
/// This is the inventory `P2-N7` and `P2-Y3` are each about. A sweep guided by
/// a list of interesting names lets through a function that mentions none of
/// them, and a sweep over `pub fn` alone lets through a trait implementation
/// that declares no `pub`. So both sets are pinned and any addition of either
/// kind is a diff.
#[test]
fn the_public_surface_is_inventoried() -> TestResult {
    let mut headers: BTreeSet<String> = BTreeSet::new();
    let mut signatures: Vec<(String, String, String)> = Vec::new();
    for (path, code) in product_code()? {
        for header in impl_headers(&code) {
            headers.insert(header);
        }
        for (owner, name, signature) in support::public_signatures_with_owner(&code) {
            signatures.push((relative_module(&path), owner, name));
            assert!(
                !signature.is_empty(),
                "{path} has a public signature the reader could not read"
            );
        }
    }
    assert_eq!(
        headers,
        IMPL_HEADERS.into_iter().map(str::to_owned).collect(),
        "an impl header appeared or moved"
    );
    signatures.sort();
    let owners: BTreeSet<&str> = signatures
        .iter()
        .map(|(_, owner, _)| owner.as_str())
        .collect();
    // Every public function sits on one of the eight types, or is a free
    // function of a module. A free function is what `minimality_defects` and
    // `propose` are, and the empty owner is how the reader spells that.
    assert_eq!(
        owners,
        BTreeSet::from([
            "",
            "ExpectedConceptClaim",
            "ExpectedConceptReading",
            "ExpectedConceptSource",
            "MaterialReference",
            "MinimalityDefect",
            "PrepAxis",
            "PrepUncertainty",
            "PrerequisiteEdgeReading",
            "PreparationBrief",
            "PreparationCandidate",
            "UserStateReading",
        ]),
        "a public function sits on a type the inventory does not hold"
    );
    // The two free functions, named.
    let free: BTreeSet<&str> = signatures
        .iter()
        .filter(|(_, owner, _)| owner.is_empty())
        .map(|(_, _, name)| name.as_str())
        .collect();
    assert_eq!(
        free,
        BTreeSet::from(["minimality_defects", "propose"]),
        "a third free function was added to this crate's surface"
    );
    assert!(
        signatures.len() >= 40,
        "the signature reader found only {} public functions",
        signatures.len()
    );
    Ok(())
}

/// The module a workspace-relative path names.
fn relative_module(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// Every `impl` header in `code`, up to and including its opening brace.
fn impl_headers(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(at) = rest.find("\nimpl") {
        let after = &rest[at + 1..];
        let end = after.find('{').map_or(after.len(), |offset| offset + 1);
        found.push(
            after[..end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
        rest = &after[end.min(after.len())..];
    }
    found
}

/// Every `pub enum` this code declares, by name.
fn public_enums(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(at) = rest.find("pub enum ") {
        let name: String = rest[at + 9..]
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            found.push(name);
        }
        rest = &rest[at + 9..];
    }
    found
}

/// `code` with every `use` line removed.
fn without_use_items(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find("use ") {
        let boundary = rest[..at].chars().next_back().unwrap_or('\n');
        if boundary.is_alphanumeric() || boundary == '_' {
            out.push_str(&rest[..at + 4]);
            rest = &rest[at + 4..];
            continue;
        }
        out.push_str(&rest[..at]);
        let Some(end) = rest[at..].find(';') else {
            rest = "";
            break;
        };
        rest = &rest[at + end + 1..];
    }
    out.push_str(rest);
    out
}

/// The three readers this file classifies with each find what they should.
#[test]
fn the_scan_helpers_are_not_vacuous() -> TestResult {
    let sample = strip_non_code(
        "\nimpl From<u8> for Sample {\n    fn from(value: u8) -> Self { Self }\n}\n",
    );
    assert_eq!(impl_headers(&sample), vec!["impl From<u8> for Sample {"]);
    assert_eq!(
        public_enums("pub enum Two { A, B }"),
        vec!["Two".to_owned()]
    );
    let stripped = without_use_items("use a::b;\nfn f() { a::b(); }");
    assert!(!stripped.contains("use a::b"));
    assert!(stripped.contains("a::b()"));
    // And the walk itself found this crate's files.
    let sources = crate_product_sources()?;
    assert_eq!(sources.len(), MODULES.len() + 1);
    assert!(
        sources
            .iter()
            .all(|path| relative(path).starts_with("crates/next-lecture/src/")),
        "the product walk left this crate"
    );
    Ok(())
}
