//! What a behavioural test cannot observe about a restore that must work when
//! the vendor is gone.
//!
//! `restore_without_vendor_or_school_account_succeeds` runs the reader with no
//! credential and no profile, which is the behaviour. It cannot see a
//! dependency that would reach a network inside a call that test happens not to
//! make, and it cannot see a byte buffer added to a type next year. Both halves
//! are needed and each catches what the other cannot; `P2-L5` measured a task
//! where two of thirteen injections were caught by exactly one layer.
//!
//! Every sweep below is a **whole set** compared in both directions rather than
//! a list of forbidden spellings. `docs/contracts/policy-source-scans.md`
//! records why: a token list refuses the edits somebody thought of in advance
//! and admits every edit spelled differently. The token list at the end is kept
//! as an explicitly weakest last layer and nothing is closed by it alone.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use support::TestResult;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Every product file of this crate, recursively.
fn product_sources() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let base = repository_root().join("crates").join("export").join("src");
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let name = path
                    .strip_prefix(&base)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((name, fs::read_to_string(&path)?));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Strips `//` comments and string literals, so a scan does not match prose.
fn stripped(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or_default();
        let mut in_string = false;
        let mut escaped = false;
        for character in code.chars() {
            match (in_string, escaped, character) {
                (_, true, _) => escaped = false,
                (true, false, '\\') => escaped = true,
                (_, false, '"') => in_string = !in_string,
                (false, false, _) => out.push(character),
                (true, false, _) => {}
            }
        }
        out.push('\n');
    }
    out
}

/// Removes the whitespace Rust allows inside a path and around a macro's `!`.
///
/// `std :: net :: TcpStream` and `include_str! ("x")` both compile and both
/// spell nothing a naive extractor sees. Deleting **all** whitespace is wrong
/// in the one direction that matters -- it joins unrelated tokens, so a key
/// disappears rather than appearing -- so only the whitespace adjacent to `::`
/// and to `!` is removed. `policy-source-scans.md`'s `P2-R2` section records
/// the six vacuous passes that produced this rule.
fn normalized(code: &str) -> String {
    let bytes: Vec<char> = code.chars().collect();
    let mut out = String::with_capacity(code.len());
    let mut index = 0;
    while index < bytes.len() {
        let character = bytes[index];
        if character.is_whitespace() {
            let mut lookahead = index;
            while lookahead < bytes.len() && bytes[lookahead].is_whitespace() {
                lookahead += 1;
            }
            let joins_path = lookahead + 1 < bytes.len()
                && bytes[lookahead] == ':'
                && bytes[lookahead + 1] == ':';
            let follows_path = out.ends_with("::");
            let precedes_call =
                lookahead < bytes.len() && bytes[lookahead] == '(' && out.ends_with('!');
            if joins_path || follows_path || precedes_call {
                index = lookahead;
                continue;
            }
            out.push(' ');
            index = lookahead;
            continue;
        }
        out.push(character);
        index += 1;
    }
    out
}

fn is_identifier_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

/// Every module-like `root` of a `root::rest` path, however it is spelled.
///
/// A leading `::` is not a middle segment, and what tells them apart is the
/// byte before it, so `::std::net::TcpStream` yields `std` here.
///
/// Roots that are not `snake_case` are left out, because a `CamelCase` root is
/// a type and a type is reachable only through a module root this set already
/// holds. Including them would make the set churn on every rename, and a guard
/// that fails on unrelated edits is a guard somebody weakens.
fn path_roots(code: &str) -> BTreeSet<String> {
    let text: Vec<char> = normalized(code).chars().collect();
    let mut roots = BTreeSet::new();
    let mut index = 0;
    while index + 1 < text.len() {
        if text[index] == ':' && text[index + 1] == ':' {
            let mut start = index;
            while start > 0 && is_identifier_char(text[start - 1]) {
                start -= 1;
            }
            // A middle segment of `a::b::c` has `::` immediately before it.
            let is_middle = start >= 2 && text[start - 1] == ':' && text[start - 2] == ':';
            if start < index && !is_middle {
                let root: String = text[start..index].iter().collect();
                if root.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                }) {
                    roots.insert(root);
                }
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    roots
}

/// The first segment of every `use` declaration.
///
/// A narrower set than [`path_roots`] and a different question: what this crate
/// brings into scope at all. A dependency reached only through an inline path
/// shows in the first set, one reached only through an import shows in this
/// one, and neither alone sees both.
fn use_roots(code: &str) -> BTreeSet<String> {
    let mut roots = BTreeSet::new();
    // Line by line, and each line normalized on its own: `normalized` collapses
    // every whitespace run, newlines included, so normalizing the whole file
    // first would leave one line and this sweep would read one import.
    for line in code.lines() {
        let normalized_line = normalized(line);
        let trimmed = normalized_line.trim();
        let Some(rest) = trimmed
            .strip_prefix("pub use ")
            .or_else(|| trimmed.strip_prefix("use "))
        else {
            continue;
        };
        let rest = rest.trim().trim_start_matches("::");
        let root: String = rest
            .chars()
            .take_while(|c| is_identifier_char(*c))
            .collect();
        if !root.is_empty() {
            roots.insert(root);
        }
    }
    roots
}

/// Every `use` item flattened to one full path per line.
///
/// This closes a hole this task found in its own guard. `use std::{fs, io,
/// sync::atomic::Ordering}` reaches `std::sync` and spells `std::` **once**, so
/// a sweep that searched for `std::` saw `fs` and nothing else: `sync` and
/// `time` were invisible, and so would `net` and `process` have been if they
/// were reached the same way. The expansion below rewrites that statement into
/// `std::fs`, `std::io` and `std::sync::atomic::Ordering`, one per line, and
/// the sweeps read the expansion beside the source.
fn expanded_uses(code: &str) -> String {
    let mut expanded = String::new();
    let text: Vec<char> = code.chars().collect();
    let mut index = 0;
    while index < text.len() {
        let rest: String = text[index..].iter().collect();
        let Some(at) = rest.find("use ") else {
            break;
        };
        let start = index + rest[..at].chars().count();
        let before = if start == 0 { ' ' } else { text[start - 1] };
        if is_identifier_char(before) {
            index = start + 4;
            continue;
        }
        let mut cursor = start + 4;
        let mut depth = 0_i32;
        let mut statement = String::new();
        while cursor < text.len() {
            let character = text[cursor];
            if character == '{' {
                depth += 1;
            } else if character == '}' {
                depth -= 1;
            } else if character == ';' && depth <= 0 {
                break;
            }
            statement.push(character);
            cursor += 1;
        }
        for path in flatten_use(&normalized(&statement)) {
            expanded.push_str(&path);
            expanded.push('\n');
        }
        index = cursor.max(start + 4);
    }
    expanded
}

/// Expands one `use` body into its full paths.
fn flatten_use(body: &str) -> Vec<String> {
    let body = body.trim();
    let Some(open) = body.find('{') else {
        return vec![body.to_owned()];
    };
    let prefix = body[..open].trim().to_owned();
    let mut depth = 0_i32;
    let mut close = None;
    for (offset, character) in body.char_indices().skip(open) {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        return vec![body.to_owned()];
    };
    let inner = &body[open + 1..close];
    let mut paths = Vec::new();
    for part in split_top_level(inner) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        for tail in flatten_use(part) {
            paths.push(format!("{prefix}{tail}"));
        }
    }
    paths
}

/// Every second segment of a `<root>::<segment>` path, for one root.
fn second_segments(code: &str, root: &str) -> BTreeSet<String> {
    let text = normalized(code);
    let needle = format!("{root}::");
    let mut segments = BTreeSet::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut search = 0;
    while let Some(offset) = text[search..].find(&needle) {
        let position = search + offset;
        let character_index = text[..position].chars().count();
        let preceded = character_index > 0 && is_identifier_char(bytes[character_index - 1]);
        if !preceded {
            let mut end = character_index + needle.chars().count();
            let start = end;
            while end < bytes.len() && is_identifier_char(bytes[end]) {
                end += 1;
            }
            if end > start {
                segments.insert(bytes[start..end].iter().collect::<String>());
            }
        }
        search = position + needle.len();
    }
    segments
}

/// Every macro invoked, by name.
///
/// A keyword is not a name a macro may have, so `if !(x)` is not a macro named
/// `if`.
fn macros(code: &str) -> BTreeSet<String> {
    const KEYWORDS: [&str; 12] = [
        "if", "while", "return", "let", "match", "else", "and", "or", "in", "for", "loop", "move",
    ];
    let text: Vec<char> = normalized(code).chars().collect();
    let mut names = BTreeSet::new();
    for index in 0..text.len() {
        if text[index] != '!' {
            continue;
        }
        if index + 1 < text.len() && text[index + 1] == '=' {
            continue;
        }
        let mut start = index;
        while start > 0 && is_identifier_char(text[start - 1]) {
            start -= 1;
        }
        if start == index {
            continue;
        }
        let name: String = text[start..index].iter().collect();
        if KEYWORDS.contains(&name.as_str()) {
            continue;
        }
        names.insert(name);
    }
    names
}

// ---------------------------------------------------------------------------
// The walk reaches every module
// ---------------------------------------------------------------------------

/// Every `pub mod` this crate declares is a file the sweeps below read.
///
/// The tripwire the rest of this file rests on. A module declared without a
/// file the walk reaches would make every sweep below silently narrower, which
/// is the first of the three shapes `policy-source-scans.md` names.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = product_sources()?;
    let names: BTreeSet<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
    let lib = sources
        .iter()
        .find(|(name, _)| name == "lib.rs")
        .map(|(_, text)| text.clone())
        .ok_or("the walk did not reach lib.rs")?;

    let mut declared = 0_usize;
    for line in lib.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("pub mod ") else {
            continue;
        };
        let module = rest.trim_end_matches(';');
        assert!(
            names.contains(format!("{module}.rs").as_str()),
            "{module} is declared and the walk does not read it"
        );
        declared += 1;
    }
    assert!(declared >= 9, "only {declared} modules were declared");
    assert_eq!(
        declared + 1,
        sources.len(),
        "the walk read a file no module declares, or missed one"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The product closure
// ---------------------------------------------------------------------------

/// This crate's declared product dependencies are exactly the four it needs.
///
/// A whole set, compared in both directions against the manifest's own text.
/// `INV-C-015` is a claim about what a reader still needs, so a store, a vault,
/// a key hierarchy or a transport reaching this closure is the claim failing,
/// and it fails here rather than in a review.
#[test]
fn the_product_closure_is_exactly_the_declared_edges() -> TestResult {
    const DECLARED: [&str; 6] = [
        "academic-audit",
        "academic-domain",
        "academic-requirement",
        "serde",
        "serde_json",
        "sha2",
    ];

    let manifest = fs::read_to_string(
        repository_root()
            .join("crates")
            .join("export")
            .join("Cargo.toml"),
    )?;
    let section = manifest
        .split_once("\n[dependencies]\n")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("\n[").map(|(block, _)| block))
        .ok_or("the manifest has no [dependencies] section")?;

    let mut observed: BTreeSet<&str> = BTreeSet::new();
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let name = trimmed
            .split_once('=')
            .map(|(name, _)| name.trim())
            .ok_or("a dependency line has no assignment")?;
        // `serde.workspace = true` names the crate before the dot; a crate
        // name cannot contain one, so the first segment is the name.
        observed.insert(name.split('.').next().unwrap_or(name));
    }

    let expected: BTreeSet<&str> = DECLARED.into_iter().collect();
    assert_eq!(
        observed, expected,
        "the declared product closure moved; every addition to it is a review"
    );
    for forbidden in [
        "academic-store",
        "academic-vault",
        "academic-crypto",
        "academic-keystore-platform",
        "academic-recovery",
        "academic-retention",
        "academic-projections",
        "academic-rpc",
        "academic-connector",
        "academic-model-run",
    ] {
        assert!(
            !observed.contains(forbidden),
            "{forbidden} is a product edge of the crate that must read a bundle without it"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// No network, no vendor, no account
// ---------------------------------------------------------------------------

/// Every path root, every `std` module and every macro this crate's product
/// source uses, as three whole sets.
///
/// Compared in both directions. An addition of any kind is a review rather than
/// something the author had to predict: `std::net`, `std::process` and
/// `std::env` fail as new second segments, a new crate fails as a new root, and
/// `include_str!` of something else fails as an existing macro used on a new
/// file only if the closure below moves — which is why the byte-buffer and
/// clock sweeps exist beside this one rather than inside it.
#[test]
fn the_product_source_reaches_only_the_declared_vocabulary() -> TestResult {
    // Every `snake_case` path root the product source spells, whether it names
    // a crate, a `std` module brought into scope, a module of this crate, or a
    // primitive an associated function is called on.
    const PATH_ROOTS: [&str; 33] = [
        "academic_audit",
        "academic_domain",
        "academic_requirement",
        "audit",
        "bundle",
        "char",
        "clippy",
        "collect",
        "collections",
        "crate",
        "directory",
        "engines",
        "error",
        "fmt",
        "fs",
        "graph",
        "io",
        "label",
        "part",
        "path",
        "profile",
        "read",
        "serde",
        "serde_json",
        "sha2",
        "source",
        "std",
        "super",
        "sync",
        "time",
        "u32",
        "u64",
        "write",
    ];
    /// Every root this product source brings into scope, `lib.rs`'s
    /// re-exports and a test module's `super` included.
    const USE_ROOTS: [&str; 16] = [
        "academic_audit",
        "academic_domain",
        "academic_requirement",
        "audit",
        "bundle",
        "crate",
        "error",
        "label",
        "part",
        "read",
        "serde",
        "sha2",
        "source",
        "std",
        "super",
        "write",
    ];
    // `os` is the Unix directory mode, `process` is the staging directory's
    // name, and both are inside `directory.rs`. `net` is what this set exists
    // to refuse, and it is refused by not being here.
    //
    // Admitting a module is not admitting everything in it. `std::process::id`
    // reads a number and the same module's process launcher starts a program,
    // and a set that stopped at the module would have admitted both once the
    // first was needed -- the shape this task found and closed in its own
    // guard. So the two admitted modules carry their own item sets below.
    const STD_MODULES: [&str; 11] = [
        "collections",
        "error",
        "fmt",
        "fs",
        "io",
        "iter",
        "os",
        "path",
        "process",
        "sync",
        "time",
    ];
    /// Every item reached under `std::process`.
    const PROCESS_ITEMS: [&str; 1] = ["id"];
    /// Every item reached under `std::os`.
    const OS_ITEMS: [&str; 1] = ["unix"];
    const MACROS: [&str; 8] = [
        "assert",
        "assert_eq",
        "assert_ne",
        "format",
        "include_str",
        "matches",
        "vec",
        "write",
    ];

    let sources = product_sources()?;
    assert!(sources.len() >= 9, "the walk read {} files", sources.len());

    let mut roots: BTreeSet<String> = BTreeSet::new();
    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut std_modules: BTreeSet<String> = BTreeSet::new();
    let mut process_items: BTreeSet<String> = BTreeSet::new();
    let mut os_items: BTreeSet<String> = BTreeSet::new();
    let mut invoked: BTreeSet<String> = BTreeSet::new();
    for (_, text) in &sources {
        let code = stripped(text);
        let expanded = expanded_uses(&code);
        roots.extend(path_roots(&code));
        imports.extend(use_roots(&code));
        for text in [&code, &expanded] {
            std_modules.extend(second_segments(text, "std"));
            process_items.extend(second_segments(text, "std::process"));
            os_items.extend(second_segments(text, "std::os"));
        }
        invoked.extend(macros(&code));
    }
    assert!(!roots.is_empty(), "the root extractor found nothing");
    assert!(!imports.is_empty(), "the import extractor found nothing");
    assert!(!invoked.is_empty(), "the macro extractor found nothing");

    assert_eq!(
        imports,
        USE_ROOTS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        "the set of imported crates moved"
    );

    assert_eq!(
        roots,
        PATH_ROOTS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        "the set of path roots moved"
    );
    assert_eq!(
        std_modules,
        STD_MODULES
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        "the set of std modules moved"
    );
    assert_eq!(
        process_items,
        PROCESS_ITEMS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        "the set of std::process items moved"
    );
    assert_eq!(
        os_items,
        OS_ITEMS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        "the set of std::os items moved"
    );
    assert_eq!(
        invoked,
        MACROS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        "the set of invoked macros moved"
    );

    // The extractors bite on the shapes a token list would miss. Each sample
    // goes through the same functions the sweep uses.
    let cases: [(&str, &str, bool); 10] = [
        (
            "a plain socket",
            "let stream = std::net::TcpStream::connect(host)?;",
            true,
        ),
        (
            "whitespace inside the path",
            "let stream = std :: net :: TcpStream::connect(host)?;",
            true,
        ),
        (
            "a leading ::",
            "let stream = ::std::net::TcpStream::connect(host)?;",
            true,
        ),
        (
            "an environment read",
            "let endpoint = std::env::var(\"VENDOR\")?;",
            true,
        ),
        (
            "an item under an admitted module",
            "std::process::exit(1);",
            true,
        ),
        (
            "a socket hidden inside a use group",
            "use std::{fs, io, net::TcpStream};",
            true,
        ),
        (
            "an environment read hidden inside a nested use group",
            "use std::{path::Path, env::{var, vars}};",
            true,
        ),
        (
            "a second item under the same admitted module",
            "std::process::abort();",
            true,
        ),
        (
            "something already allowed",
            "let path = std::path::Path::new(x);",
            false,
        ),
        (
            "the admitted process item",
            "let pid = std::process::id();",
            false,
        ),
    ];
    let allowed_modules: BTreeSet<String> = STD_MODULES.into_iter().map(str::to_owned).collect();
    let allowed_process: BTreeSet<String> = PROCESS_ITEMS.into_iter().map(str::to_owned).collect();
    let allowed_os: BTreeSet<String> = OS_ITEMS.into_iter().map(str::to_owned).collect();
    for (label, sample, must_be_caught) in cases {
        let code = stripped(sample);
        let expanded = expanded_uses(&code);
        let mut modules = BTreeSet::new();
        let mut process = BTreeSet::new();
        let mut os = BTreeSet::new();
        for text in [&code, &expanded] {
            modules.extend(second_segments(text, "std"));
            process.extend(second_segments(text, "std::process"));
            os.extend(second_segments(text, "std::os"));
        }
        assert!(
            !modules.is_empty(),
            "{label}: the extractor found no std module in {sample}"
        );
        let escapes = modules
            .iter()
            .any(|module| !allowed_modules.contains(module))
            || process.iter().any(|item| !allowed_process.contains(item))
            || os.iter().any(|item| !allowed_os.contains(item));
        assert_eq!(
            escapes,
            must_be_caught,
            "{label}: the sweep would have {} {sample}",
            if must_be_caught {
                "admitted"
            } else {
                "refused"
            }
        );
    }

    // A macro spelled with whitespace before its parenthesis is still a macro.
    assert!(macros(&stripped("let text = include_str! (\"x\");")).contains("include_str"));
    assert!(!macros(&stripped("if !(condition) { }")).contains("if"));

    // The weakest layer, kept as a third net and never as the only one.
    const FORBIDDEN: [&str; 12] = [
        "TcpStream",
        "UdpSocket",
        "reqwest",
        "hyper",
        "ureq",
        "OAuth",
        "oauth",
        "access_token",
        "session_cookie",
        "school_account",
        "vendor_endpoint",
        "api_key",
    ];
    for (name, text) in &sources {
        let code = stripped(text);
        for spelling in FORBIDDEN {
            assert!(
                !code.contains(spelling),
                "{name} reaches {spelling} on the vendor-free restore path"
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The byte-buffer inventory, closed at this crate's own boundary
// ---------------------------------------------------------------------------

/// Every named field and every tuple position in this crate, classified.
///
/// `P2-RF13` and `P2-RF15` found seven `Debug` leaks, four of them in
/// `crypto`, `recovery` and `portability`, and the net that found them
/// classifies byte buffers across the workspace. That the workspace net passes
/// is **not** evidence for a crate it has not been re-measured against, so this
/// is the same question asked at this crate's own boundary and answered in both
/// directions: every buffer-typed field is listed here, and every entry here
/// must still exist.
///
/// The list is empty on purpose. Bytes stream through a fixed buffer inside
/// `directory::copy_new_file` and are never held in a value, so a type of this
/// crate holding one is a new decision that fails here until somebody records
/// what it holds.
#[test]
fn no_type_in_this_crate_holds_an_unclassified_byte_buffer() -> TestResult {
    /// Field or tuple position to what it holds. Empty: this crate holds none.
    const CLASSIFIED: [(&str, &str); 0] = [];

    let sources = product_sources()?;
    assert!(sources.len() >= 9, "the walk read {} files", sources.len());

    let mut observed: BTreeMap<String, String> = BTreeMap::new();
    let mut bodies = 0_usize;
    for (name, text) in &sources {
        let code = stripped(text);
        for (type_name, body, is_tuple) in type_bodies(&code) {
            bodies += 1;
            for (position, (field, kind)) in fields(&body, is_tuple).into_iter().enumerate() {
                if holds_bytes(&kind) {
                    let key = if field.is_empty() {
                        format!("{type_name}.{position}")
                    } else {
                        format!("{type_name}.{field}")
                    };
                    observed.insert(key, format!("{name} :: {kind}"));
                }
            }
        }
    }
    assert!(
        bodies >= 20,
        "only {bodies} struct and enum bodies were read; the extractor is not seeing this crate"
    );

    let classified: BTreeSet<&str> = CLASSIFIED.iter().map(|(field, _)| *field).collect();
    for (field, where_found) in &observed {
        assert!(
            classified.contains(field.as_str()),
            "{field} at {where_found} holds bytes and nothing classifies it"
        );
    }
    for (field, _) in CLASSIFIED {
        assert!(
            observed.contains_key(field),
            "{field} is classified and no longer exists"
        );
    }

    // The classifier bites: each shape below is one this net must see, and each
    // shape after it is one it must not.
    for shape in [
        "Vec<u8>",
        "[u8; 32]",
        "&'a [u8]",
        "Option<Vec<u8>>",
        "zeroize::Zeroizing<Vec<u8>>",
        "Box<[u8]>",
        "BTreeMap<String, Vec<u8>>",
    ] {
        assert!(holds_bytes(shape), "{shape} was not read as a byte buffer");
    }
    for shape in [
        "String",
        "u64",
        "PathBuf",
        "Vec<String>",
        "SensitivityLabel",
    ] {
        assert!(!holds_bytes(shape), "{shape} was read as a byte buffer");
    }

    // And the body extractor bites: a byte field added to a type of this crate
    // is seen wherever it sits, including on one line, in a tuple position and
    // inside an enum arm. `T114` found a variant payload invisible to a guard
    // that read only named fields.
    for sample in [
        "pub struct Leaky { pub dek: Vec<u8> }",
        "struct Leaky { plaintext: [u8; 32], name: String }",
        "enum Leaky { Dek([u8; 32]), Empty }",
        "pub struct Leaky(pub Vec<u8>);",
    ] {
        let found: Vec<(String, String)> = type_bodies(sample)
            .into_iter()
            .flat_map(|(_, body, is_tuple)| fields(&body, is_tuple))
            .filter(|(_, kind)| holds_bytes(kind))
            .collect();
        assert!(
            !found.is_empty(),
            "the field extractor did not see a byte buffer in {sample}"
        );
    }

    // A function parameter is not a field. Reading every `name: type` line
    // instead would have reported `copy_new_file`'s streaming buffer as a
    // stored one, and a guard that reports what is not there is one somebody
    // silences.
    assert!(
        type_bodies("fn copy(bytes: &[u8], path: &Path) -> Result<(), Error> { }").is_empty(),
        "a function signature was read as a type body"
    );
    Ok(())
}

/// Every `struct` and `enum` body in one file, with whether it is a tuple form.
///
/// A body is taken by balancing its own delimiter, so a nested type, a generic
/// bound and an inner block are inside the body they belong to rather than
/// ending it.
fn type_bodies(code: &str) -> Vec<(String, String, bool)> {
    let text: Vec<char> = code.chars().collect();
    let mut bodies = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let rest: String = text[index..].iter().collect();
        let Some((at, keyword)) = ["struct ", "enum "]
            .into_iter()
            .filter_map(|keyword| rest.find(keyword).map(|at| (at, keyword)))
            .min_by_key(|(at, _)| *at)
        else {
            break;
        };
        let start = index + rest[..at].chars().count();
        let before = if start == 0 { ' ' } else { text[start - 1] };
        if is_identifier_char(before) {
            index = start + keyword.chars().count();
            continue;
        }
        let mut cursor = start + keyword.chars().count();
        while cursor < text.len() && text[cursor].is_whitespace() {
            cursor += 1;
        }
        let name_start = cursor;
        while cursor < text.len() && is_identifier_char(text[cursor]) {
            cursor += 1;
        }
        let name: String = text[name_start..cursor].iter().collect();
        // Step over generics and where-clauses to the body's own delimiter.
        let mut depth = 0_i32;
        while cursor < text.len() {
            match text[cursor] {
                '<' => depth += 1,
                '>' => depth -= 1,
                '{' | '(' if depth <= 0 => break,
                ';' if depth <= 0 => break,
                _ => {}
            }
            cursor += 1;
        }
        if cursor >= text.len() || text[cursor] == ';' {
            index = cursor.max(start + 1);
            continue;
        }
        let (open, close) = if text[cursor] == '{' {
            ('{', '}')
        } else {
            ('(', ')')
        };
        let is_tuple = open == '(';
        let body_start = cursor + 1;
        let mut balance = 1_i32;
        cursor = body_start;
        while cursor < text.len() && balance > 0 {
            if text[cursor] == open {
                balance += 1;
            } else if text[cursor] == close {
                balance -= 1;
            }
            cursor += 1;
        }
        let body: String = text[body_start..cursor.saturating_sub(1)].iter().collect();
        if !name.is_empty() {
            bodies.push((name, body, is_tuple));
        }
        index = cursor;
    }
    bodies
}

/// Every field of one type body, as a name and a type.
fn fields(body: &str, is_tuple: bool) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for entry in split_top_level(body) {
        let entry = entry.trim();
        if entry.is_empty() || entry.starts_with('#') {
            continue;
        }
        let entry = entry
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub ")
            .trim();
        if is_tuple {
            found.push((String::new(), entry.to_owned()));
            continue;
        }
        if let Some((name, kind)) = entry.split_once(':')
            && !name.trim().is_empty()
            && name.trim().chars().all(is_identifier_char)
        {
            found.push((name.trim().to_owned(), kind.trim().to_owned()));
            continue;
        }
        // An enum arm: `Name(payload, payload)` or `Name { field: type }`.
        if let Some((_, payload)) = entry.split_once('(') {
            for part in split_top_level(payload.trim_end_matches(')')) {
                found.push((String::new(), part.trim().to_owned()));
            }
        } else if let Some((_, payload)) = entry.split_once('{') {
            found.extend(fields(payload.trim_end_matches('}'), false));
        }
    }
    found
}

/// Splits on commas that are not inside brackets.
fn split_top_level(body: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for character in body.chars() {
        match character {
            '<' | '(' | '[' | '{' => {
                depth += 1;
                current.push(character);
            }
            '>' | ')' | ']' | '}' => {
                depth -= 1;
                current.push(character);
            }
            ',' if depth == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// Whether a field type holds raw bytes, decided by the type and never by the
/// field's name.
///
/// The name is the weakest of the three layers `secret-debug-policy.test.mjs`
/// describes, and `S-10` records five attempts to close a hole by adding one,
/// so it is not consulted here at all: a `Vec<u8>` called `excerpt` is bytes.
fn holds_bytes(kind: &str) -> bool {
    let compact: String = kind.chars().filter(|c| !c.is_whitespace()).collect();
    compact.contains("[u8]") || compact.contains("<u8>") || compact.contains("[u8;")
}

// ---------------------------------------------------------------------------
// The clock
// ---------------------------------------------------------------------------

/// The one clock read in this crate is the staging directory's name.
///
/// `export_is_deterministic_at_a_fixed_watermark` observes that two bundles are
/// byte-identical, which would also be true of a writer that read a clock and
/// happened not to record it. This is the other half: the clock is read in
/// exactly one function, that function names a directory removed by the publish
/// rename, and every other file is written from the recorded instant.
#[test]
fn the_only_clock_read_names_a_staging_directory() -> TestResult {
    let sources = product_sources()?;
    let mut readers: BTreeSet<&str> = BTreeSet::new();
    for (name, text) in &sources {
        let code = normalized(&stripped(text));
        if code.contains("SystemTime") || code.contains("Instant::now") {
            readers.insert(name.as_str());
        }
    }
    assert_eq!(
        readers,
        ["directory.rs"].into_iter().collect::<BTreeSet<&str>>(),
        "a clock is read outside the staging-path reservation"
    );

    let directory = sources
        .iter()
        .find(|(name, _)| name == "directory.rs")
        .map(|(_, text)| stripped(text))
        .ok_or("the walk did not reach directory.rs")?;
    // An import is not a read. What is counted is the call, across every product
    // file, and it must be one call inside one function.
    let reads: usize = sources
        .iter()
        .map(|(_, text)| {
            normalized(&stripped(text))
                .matches("SystemTime::now")
                .count()
        })
        .sum();
    assert_eq!(reads, 1, "the crate reads a clock {reads} times");

    let reservation = directory
        .split_once("fn reserve_staging_path")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("\nfn ").map(|(block, _)| block))
        .ok_or("reserve_staging_path is not in directory.rs")?;
    assert!(
        normalized(reservation).contains("SystemTime::now"),
        "the clock moved out of the staging-path reservation"
    );
    let before = directory
        .split_once("fn reserve_staging_path")
        .map(|(head, _)| head)
        .ok_or("reserve_staging_path is not in directory.rs")?;
    assert!(
        !normalized(before).contains("SystemTime::now"),
        "a clock is read before the staging-path reservation"
    );

    // What that one read produces names a staging directory, and the publish
    // rename removes it, so no clock value reaches a file a bundle keeps. The
    // literal is read from the unstripped source, because `stripped` removes
    // string literals and this is a claim about one.
    let raw = sources
        .iter()
        .find(|(name, _)| name == "directory.rs")
        .map(|(_, text)| text.clone())
        .ok_or("the walk did not reach directory.rs")?;
    let raw_reservation = raw
        .split_once("fn reserve_staging_path")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("\n#[cfg(unix)]").map(|(block, _)| block))
        .ok_or("reserve_staging_path is not in directory.rs")?;
    assert!(
        raw_reservation.contains("bundle-staging-"),
        "the clock no longer names the staging directory"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The portable path rules
// ---------------------------------------------------------------------------

/// This crate's path rules are the Phase 1 export's, and the two are compared.
///
/// `directory.rs` repeats them because a reader that had to link
/// `academic-portability` to open a directory would contradict the sentence
/// this crate exists for. A repetition nobody compares is a fork, so the
/// reserved-name list and the length budget are read out of the other crate's
/// source and required to agree.
#[test]
fn the_portable_path_rules_match_the_phase_1_export() -> TestResult {
    assert_eq!(
        academic_export::MAX_BUNDLE_RELATIVE_PATH_BYTES,
        academic_portability::MAX_PORTABLE_RELATIVE_PATH_BYTES,
        "the portable path budget forked"
    );

    let phase1 = fs::read_to_string(
        repository_root()
            .join("crates")
            .join("portability")
            .join("src")
            .join("lib.rs"),
    )?;
    let theirs = reserved_names(&phase1, "const WINDOWS_RESERVED_NAMES")?;
    let ours: BTreeSet<String> = academic_export::directory::WINDOWS_RESERVED_NAMES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert!(theirs.len() >= 22, "only {} names were read", theirs.len());
    assert_eq!(ours, theirs, "the reserved Windows device names forked");
    Ok(())
}

fn reserved_names(
    source: &str,
    marker: &str,
) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let block = source
        .split_once(marker)
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("= &["))
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once(']').map(|(block, _)| block))
        .ok_or("the reserved-name list is not where it was")?;
    Ok(block
        .split(',')
        .filter_map(|entry| {
            let trimmed = entry.trim().trim_matches('"');
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        })
        .collect())
}
