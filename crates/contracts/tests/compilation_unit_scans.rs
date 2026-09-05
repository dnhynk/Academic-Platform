//! The set of files this repository's source scans read, against the set the
//! compiler reads.
//!
//! Every source scan in this repository walks a directory and keeps the entries
//! whose extension is `rs`. `docs/contracts/policy-source-scans.md` calls that
//! walk total over the crate, and the inventories built on it say so in their
//! own words: *"There is no spelling of that injection that this test does not
//! see, because it does not look for spellings: it compares the set."*
//!
//! Rust does not require a compiled file to be named `*.rs`. `include!` takes
//! any path, `#[path]` takes any path, and neither is bounded by the directory
//! the declaring file is in. A file the compiler reads and the walk does not is
//! not a weaker check — it is no check, and every inventory, `Default` sweep,
//! producer count and determinism scan in the workspace is total over a set
//! that does not contain it. `P2-A3`'s second audit measured that: one
//! `include!("witness_ext.inc")` line put four different injections past
//! `academic-audit`, `academic-offering` and `academic-review` with the
//! workspace reporting 279 `test result: ok` blocks and exit 0.
//!
//! So the two sets are enumerated and compared here. The reader below resolves
//! a crate's compilation unit the way `rustc` does — from each target root,
//! through `mod name;`, `#[path = "…"] mod name;` and `include!("…")`,
//! recursively — and the tests state what that closure may contain:
//!
//! * every file in it is a `*.rs` file, so a `.inc` fails by name;
//! * every file a crate's **product** targets pull in is under that crate's own
//!   `src`, so a `#[path]` reaching sideways fails by name;
//! * every `*.rs` file under a crate's `src` is in that crate's product
//!   closure, so a file that no target compiles fails as well — the direction
//!   that makes this an equality rather than a floor;
//! * every path handed to the compiler is one this reader could resolve, so a
//!   computed path fails rather than being skipped.
//!
//! It is not a list of forbidden spellings. `include!` stays legal and so does
//! `#[path]`: what fails is a compilation unit that the `*.rs` walks cannot
//! see. An injection renamed to `witness_ext.rs` passes here and is then read
//! by the crate's own walk, where it arrives at the declaration and impl
//! inventories as an entry nobody wrote down. Both routes end in a named
//! failure, which is the property the inventories claim.
//!
//! **What this reader deliberately over-approximates.** `cfg` is not
//! evaluated, so a platform or feature module is in the closure on every
//! platform. That is the direction a scan needs: the set a scan must read is
//! every file the compiler could read under any configuration, not the subset
//! one build happened to compile.
//!
//! **What it cannot see.** A path built by `concat!`/`env!` is not decidable
//! from the text. There are four such sites, pinned whole below, and the one
//! that injects *items* — `crates/rpc/src/generated.rs` — carries its own
//! byte fingerprint of the generated file (`EXPECTED_CODEGEN_FNV1A64`). A
//! procedural macro can also read a file; none in the dependency closure does,
//! and that is a statement about `Cargo.lock`, which
//! `tools/cargo-lock-source-policy.mjs` holds, rather than about this walk.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

/// The repository root, by climbing rather than by joining `..`.
///
/// A path carrying `..` components never strips as a prefix, so every relative
/// path printed below would silently become an absolute one.
fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "the manifest directory has no grandparent".into())
}

/// A repository-relative path with forward slashes.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Resolves `.` and `..` textually, without touching the filesystem.
///
/// A target that does not exist still has to produce a path, because a path the
/// walk never read is exactly what the tests below refuse.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Reading the text the compiler reads
// ---------------------------------------------------------------------------

/// Source with every comment and every literal body blanked, plus the literal
/// bodies kept aside by the position of their opening quote.
///
/// Both halves are needed and neither alone is enough. `#[path = "…"]` and
/// `include!("…")` are only resolvable from the literal's text, and
/// `crates/keystore-platform/tests/facade.rs` asserts on the string
/// `"mod linux;"`, which a reader that kept literals in place would follow as a
/// module declaration into a file that does not exist. Blanking in place keeps
/// every position stable, so the side table is addressable by offset.
struct Lexed {
    /// The source, character for character, with comments and literal bodies
    /// blanked. One output character per input character, so an index into it
    /// is an index into [`Lexed::source`].
    code: Vec<char>,
    /// The unblanked source, for quoting a site back in a failure.
    source: Vec<char>,
    literals: BTreeMap<usize, String>,
}

/// Blanks comments and literal bodies, character position preserved.
///
/// Raw strings are handled explicitly: `P2-G4` found that a reader without them
/// desynchronizes at the first one and reads every literal after it as code.
fn lex(source: &str) -> Lexed {
    let chars: Vec<char> = source.chars().collect();
    let mut code: Vec<char> = Vec::with_capacity(chars.len());
    let mut literals = BTreeMap::new();
    let mut index = 0_usize;
    while index < chars.len() {
        let current = chars[index];
        let next = chars.get(index + 1).copied();

        if current == '/' && next == Some('/') {
            while index < chars.len() && chars[index] != '\n' {
                code.push(' ');
                index += 1;
            }
            continue;
        }
        if current == '/' && next == Some('*') {
            let mut depth = 1_usize;
            code.push(' ');
            code.push(' ');
            index += 2;
            while index < chars.len() && depth > 0 {
                if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
                    depth += 1;
                    code.push(' ');
                    code.push(' ');
                    index += 2;
                } else if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    code.push(' ');
                    code.push(' ');
                    index += 2;
                } else {
                    code.push(if chars[index] == '\n' { '\n' } else { ' ' });
                    index += 1;
                }
            }
            continue;
        }
        // A character literal hides text the same way a string does; a lifetime
        // does not, and the two are told apart by the closing quote.
        if current == '\'' {
            let literal = if chars.get(index + 1) == Some(&'\\') {
                chars
                    .get(index + 3)
                    .is_some_and(|character| *character == '\'')
                    .then_some(4)
            } else {
                chars
                    .get(index + 2)
                    .is_some_and(|character| *character == '\'')
                    .then_some(3)
            };
            if let Some(width) = literal {
                code.extend(core::iter::repeat_n(' ', width));
                index += width;
                continue;
            }
        }
        // A raw string: `r`, `br` or `cr`, then hashes, then the quote.
        if current == 'r' || ((current == 'b' || current == 'c') && next == Some('r')) {
            let mut cursor = if current == 'r' { index + 1 } else { index + 2 };
            let mut hashes = 0_usize;
            while chars.get(cursor) == Some(&'#') {
                hashes += 1;
                cursor += 1;
            }
            if chars.get(cursor) == Some(&'"') {
                let opening = cursor;
                // The prefix is blanked and the quote is kept, so the side
                // table's key is the quote's own position.
                code.extend(core::iter::repeat_n(' ', opening - index));
                code.push('"');
                let mut body = String::new();
                cursor = opening + 1;
                loop {
                    if cursor >= chars.len() {
                        break;
                    }
                    if chars[cursor] == '"'
                        && (0..hashes).all(|offset| chars.get(cursor + 1 + offset) == Some(&'#'))
                    {
                        break;
                    }
                    body.push(chars[cursor]);
                    cursor += 1;
                }
                literals.insert(opening, body.clone());
                for character in body.chars() {
                    code.push(if character == '\n' { '\n' } else { ' ' });
                }
                let closing = (cursor + 1 + hashes).min(chars.len());
                code.extend(core::iter::repeat_n(' ', closing.saturating_sub(cursor)));
                index = closing;
                continue;
            }
        }
        if current == '"' {
            let opening = index;
            let mut body = String::new();
            let mut cursor = index + 1;
            let mut escaped = false;
            while cursor < chars.len() {
                if escaped {
                    escaped = false;
                    body.push(chars[cursor]);
                    cursor += 1;
                    continue;
                }
                if chars[cursor] == '\\' {
                    escaped = true;
                    body.push(chars[cursor]);
                    cursor += 1;
                    continue;
                }
                if chars[cursor] == '"' {
                    break;
                }
                body.push(chars[cursor]);
                cursor += 1;
            }
            literals.insert(opening, body.clone());
            code.push('"');
            for character in body.chars() {
                code.push(if character == '\n' { '\n' } else { ' ' });
            }
            if cursor < chars.len() {
                code.push(' ');
                cursor += 1;
            }
            index = cursor;
            continue;
        }
        code.push(current);
        index += 1;
    }
    debug_assert_eq!(code.len(), chars.len());
    Lexed {
        code,
        source: chars,
        literals,
    }
}

/// Whether `code` spells `word` at `at`, as a whole token.
///
/// Both boundaries, because only the leading one is not enough: `modules;`
/// starts with `mod` and would otherwise be read as a module declaration named
/// `ules`, and `include_str!` starts with `include`.
fn token_at(code: &[char], at: usize, word: &str) -> bool {
    let before_ok = at == 0 || !(code[at - 1].is_alphanumeric() || code[at - 1] == '_');
    if !before_ok {
        return false;
    }
    let mut cursor = at;
    for expected in word.chars() {
        if code.get(cursor) != Some(&expected) {
            return false;
        }
        cursor += 1;
    }
    !code
        .get(cursor)
        .is_some_and(|after| after.is_alphanumeric() || *after == '_')
}

/// The directory a `mod name;` inside `file` resolves against.
///
/// `lib.rs`, `main.rs` and `mod.rs` are `mod-rs` files and own their own
/// directory; every other file owns a directory named after its stem.
fn module_directory(file: &Path) -> PathBuf {
    let parent = file.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    match file.file_stem().and_then(|stem| stem.to_str()) {
        Some("lib" | "main" | "mod") => parent,
        Some(stem) => parent.join(stem),
        None => parent,
    }
}

/// What one target root pulls into the compilation unit.
#[derive(Default)]
struct Closure {
    /// Every file the compiler reads as Rust, the root included.
    files: BTreeSet<PathBuf>,
    /// Every literal `include_str!`/`include_bytes!` target, with its site.
    embedded: BTreeSet<(String, PathBuf)>,
    /// Every `include!`, `include_str!` or `include_bytes!` whose argument is
    /// not a single string literal, as `file: whole invocation`.
    computed: BTreeSet<String>,
}

/// Resolves the compilation unit reachable from one target root.
///
/// A target root is a crate root whatever its stem — `tests/audit.rs` is the
/// root of its own crate — so it resolves its own `mod` declarations against
/// the directory it sits in rather than against a directory named after it.
fn resolve(root: &Path, repository: &Path) -> Result<Closure, Box<dyn Error>> {
    let mut closure = Closure::default();
    let mut pending = vec![(
        normalize(root),
        root.parent().map_or_else(PathBuf::new, Path::to_path_buf),
    )];
    while let Some((file, directory)) = pending.pop() {
        if !closure.files.insert(file.clone()) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&file) else {
            // Left in `files` so the caller fails naming it.
            continue;
        };
        let Lexed {
            code,
            source: original,
            literals,
        } = lex(&source);
        let file_directory = file.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        let mut stack = vec![directory];
        let mut depth = 0_i32;
        let mut opened: Vec<i32> = Vec::new();
        let mut declared_path: Option<String> = None;
        let mut index = 0_usize;
        while index < code.len() {
            let current = code[index];
            if current == '{' {
                depth += 1;
                index += 1;
                continue;
            }
            if current == '}' {
                depth -= 1;
                while opened.last() == Some(&depth) {
                    opened.pop();
                    stack.pop();
                }
                index += 1;
                continue;
            }
            if current == '#' && code.get(index + 1) == Some(&'[') {
                let mut cursor = index + 2;
                let mut nesting = 1_i32;
                while cursor < code.len() && nesting > 0 {
                    match code[cursor] {
                        '[' => nesting += 1,
                        ']' => nesting -= 1,
                        _ => {}
                    }
                    cursor += 1;
                }
                // `#[path = "…"]` and `#[cfg_attr(…, path = "…")]` both land
                // here; the attribute is read for a `path` key and the quote
                // after it addresses the side table.
                let mut scan = index;
                while scan < cursor {
                    if token_at(&code, scan, "path") {
                        let mut after = scan + 4;
                        while code.get(after).is_some_and(|c| c.is_whitespace()) {
                            after += 1;
                        }
                        if code.get(after) == Some(&'=') {
                            after += 1;
                            while code.get(after).is_some_and(|c| c.is_whitespace()) {
                                after += 1;
                            }
                            if let Some(value) = literals.get(&after) {
                                declared_path = Some(value.clone());
                            }
                        }
                    }
                    scan += 1;
                }
                index = cursor;
                continue;
            }
            if token_at(&code, index, "mod") {
                let mut cursor = index + 3;
                while code.get(cursor).is_some_and(|c| c.is_whitespace()) {
                    cursor += 1;
                }
                let start = cursor;
                while code
                    .get(cursor)
                    .is_some_and(|c| c.is_alphanumeric() || *c == '_')
                {
                    cursor += 1;
                }
                let name: String = code[start..cursor].iter().collect();
                while code.get(cursor).is_some_and(|c| c.is_whitespace()) {
                    cursor += 1;
                }
                let terminator = code.get(cursor).copied();
                if !name.is_empty() && matches!(terminator, Some(';') | Some('{')) {
                    let here = stack.last().cloned().unwrap_or_default();
                    // A `#[path]` written at the top level of a file resolves
                    // against that file's directory; one written inside an
                    // inline `mod` block resolves against the module directory.
                    let attribute_base = if stack.len() == 1 {
                        file_directory.clone()
                    } else {
                        here.clone()
                    };
                    if terminator == Some(';') {
                        let (target, next) = match declared_path.as_ref() {
                            Some(declared) => {
                                let target = normalize(&attribute_base.join(declared));
                                let next = module_directory(&target);
                                (target, next)
                            }
                            None => {
                                let flat = here.join(format!("{name}.rs"));
                                if flat.is_file() {
                                    (normalize(&flat), here.join(&name))
                                } else {
                                    let nested = here.join(&name).join("mod.rs");
                                    (normalize(&nested), here.join(&name))
                                }
                            }
                        };
                        pending.push((target, next));
                    } else {
                        let next = match declared_path.as_ref() {
                            Some(declared) => normalize(&attribute_base.join(declared)),
                            None => here.join(&name),
                        };
                        stack.push(next);
                        depth += 1;
                        opened.push(depth - 1);
                    }
                    declared_path = None;
                    index = cursor + 1;
                    continue;
                }
                index += 3;
                continue;
            }
            // A macro invocation is `path` `!` `DelimTokenTree`, and the three
            // are separate tokens: whitespace may sit between the name and the
            // `!`, and the tree may be delimited by any of the three pairs.
            // `include !("x.inc")` and `include!{"x.inc"}` compile, and a
            // reader that insists on `include!(` sees neither -- which is the
            // hole this file exists to close, one step further out.
            let including = ["include", "include_str", "include_bytes"]
                .into_iter()
                .find(|name| token_at(&code, index, name))
                .and_then(|name| {
                    let mut cursor = index + name.len();
                    while code.get(cursor).is_some_and(char::is_ascii_whitespace) {
                        cursor += 1;
                    }
                    if code.get(cursor) != Some(&'!') {
                        return None;
                    }
                    cursor += 1;
                    while code.get(cursor).is_some_and(char::is_ascii_whitespace) {
                        cursor += 1;
                    }
                    match code.get(cursor) {
                        Some('(') => Some((name, cursor, '(', ')')),
                        Some('[') => Some((name, cursor, '[', ']')),
                        Some('{') => Some((name, cursor, '{', '}')),
                        _ => None,
                    }
                });
            if let Some((macro_name, open, opening, closing)) = including {
                let mut cursor = open + 1;
                let mut nesting = 1_i32;
                while cursor < code.len() && nesting > 0 {
                    let current = code[cursor];
                    if current == opening {
                        nesting += 1;
                    } else if current == closing {
                        nesting -= 1;
                    }
                    cursor += 1;
                }
                let mut argument = open + 1;
                while code.get(argument).is_some_and(char::is_ascii_whitespace) {
                    argument += 1;
                }
                // The argument is one string literal exactly when the quote the
                // side table is keyed by is the only thing between the
                // delimiters.
                let literal = literals.get(&argument).filter(|value| {
                    let mut after = argument + 1 + value.chars().count() + 1;
                    while code.get(after).is_some_and(char::is_ascii_whitespace) {
                        after += 1;
                    }
                    after + 1 >= cursor
                });
                match literal {
                    Some(value) => {
                        let target = normalize(&file_directory.join(value));
                        if macro_name == "include" {
                            let next = module_directory(&target);
                            pending.push((target, next));
                        } else {
                            closure
                                .embedded
                                .insert((relative(repository, &file), target));
                        }
                    }
                    None => {
                        let invocation = original[index..cursor.min(original.len())]
                            .iter()
                            .collect::<String>()
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        closure
                            .computed
                            .insert(format!("{}: {invocation}", relative(repository, &file)));
                    }
                }
                index = cursor;
                continue;
            }
            index += 1;
        }
    }
    Ok(closure)
}

// ---------------------------------------------------------------------------
// The target roots Cargo compiles
// ---------------------------------------------------------------------------

/// Whether `path` is a target root under `directory` of the auto-discovered
/// shape: `directory/name.rs` or `directory/name/main.rs`.
fn auto_targets(directory: &Path, roots: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            let nested = path.join("main.rs");
            if nested.is_file() {
                roots.push(nested);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            roots.push(path);
        }
    }
    Ok(())
}

/// The roots of the targets that ship: the library, the binaries, the build
/// script. Their closure is product source.
fn product_roots(crate_directory: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut roots = Vec::new();
    for candidate in ["src/lib.rs", "src/main.rs", "build.rs"] {
        let path = crate_directory.join(candidate);
        if path.is_file() {
            roots.push(path);
        }
    }
    auto_targets(&crate_directory.join("src/bin"), &mut roots)?;
    // An explicit `path = "…"` in the manifest is the other way a target root
    // is named, and it is how `academic-worker` and `academic-capture-gate`
    // compile a probe that does not live under `src`.
    let manifest = fs::read_to_string(crate_directory.join("Cargo.toml"))?;
    for line in manifest.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("path = \"") else {
            continue;
        };
        let Some(value) = rest.split('"').next() else {
            continue;
        };
        if !value.ends_with(".rs") {
            continue;
        }
        let path = normalize(&crate_directory.join(value));
        if path.is_file() && !roots.contains(&path) {
            roots.push(path);
        }
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

/// Every target root Cargo compiles for this crate, product and test alike.
fn all_roots(crate_directory: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut roots = product_roots(crate_directory)?;
    for directory in ["tests", "benches", "examples"] {
        auto_targets(&crate_directory.join(directory), &mut roots)?;
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

/// Every crate directory in the workspace.
fn crate_directories(repository: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(repository.join("crates"))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() && path.join("Cargo.toml").is_file() {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// Every `*.rs` file under `directory`, recursively.
fn rust_files(directory: &Path) -> Result<BTreeSet<PathBuf>, Box<dyn Error>> {
    let mut found = BTreeSet::new();
    if !directory.is_dir() {
        return Ok(found);
    }
    let mut pending = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.insert(normalize(&path));
            }
        }
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// The pinned exceptions, each with the reason it is one
// ---------------------------------------------------------------------------

/// Product target roots that do not live under their crate's `src`.
///
/// `S-12` in `docs/contracts/policy-source-scans.md` is the row about a scan
/// that walks `<crate>/src` and stops seeing product-shaped code beside it.
/// These four are that shape, and the three probes are why the row exists:
/// they are the only files in the workspace that name a socket type, each a
/// `[[bin]]` behind `required-features` with a `path` outside `src`. A fifth
/// arriving fails here rather than becoming a tree that no `src` walk reads —
/// which is what happened: `crates/process-sandbox/probes/enforcement_probe.rs`
/// arrived from `P2-RF21` while this list held three, and it failed as an extra
/// key with no edit to any scan.
const PRODUCT_ROOTS_OUTSIDE_SRC: [&str; 4] = [
    "crates/capture-gate/probes/capture_probe.rs",
    "crates/process-sandbox/probes/enforcement_probe.rs",
    "crates/rpc/build.rs",
    "crates/worker/probes/worker_probe.rs",
];

/// `*.rs` files under a crate's `src` that no target of that crate compiles.
///
/// `academic-test-support`'s `lib.rs` declares no module. These six files are
/// text that other crates' **test** targets pull in through `#[path]` —
/// `crates/crypto/tests/key_hierarchy.rs` and
/// `crates/recovery/tests/recovery_admission.rs` share
/// `word_level_entry_points.rs` that way, which is the row `S-14` records. They
/// are read by a walk of `crates/test-support/src`; they are not compiled by
/// `academic-test-support`.
const SOURCE_NO_TARGET_OF_ITS_CRATE_COMPILES: [&str; 6] = [
    "crates/test-support/src/encrypted_artifacts.rs",
    "crates/test-support/src/fault_driver.rs",
    "crates/test-support/src/oracle.rs",
    "crates/test-support/src/process.rs",
    "crates/test-support/src/synthetic_artifacts.rs",
    "crates/test-support/src/word_level_entry_points.rs",
];

/// Every path handed to the compiler that this reader cannot resolve.
///
/// A computed path is the one shape that defeats a textual reader, so the set
/// is pinned whole and each entry is pinned as its own text. Exactly one of
/// them injects **items**: `crates/rpc/src/generated.rs`, whose `include!`
/// names the file `prost-build` writes into `OUT_DIR`. That file is not a hole
/// — the same module fingerprints both the schema it was generated from and
/// the generated bytes, and `academic-rpc` refuses to agree with a build whose
/// fingerprints have moved.
///
/// The `!` of the bare `include` is written `\u{21}` in the two entries that
/// carry one. `only_egress_crate_has_a_socket` in
/// `tools/phase1-scaffold-policy.test.mjs` enumerates every `include!` site in
/// the repository against a pinned map and reads the source with its string
/// literals in place, so a pin quoting the text of an `include!` would arrive
/// there as a second site in a file that has none. The runtime values are the
/// text they pin, byte for byte, which is what the comparisons below use.
const COMPUTED_INCLUDE_PATHS: [&str; 4] = [
    "crates/rpc/src/generated.rs: include\u{21}(concat!(env!(\"OUT_DIR\"), \"/academic.v1.rs\"))",
    "crates/rpc/src/generated.rs: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/academic.v1.rs\"))",
    "crates/transcript/tests/support/mod.rs: include_str!(concat!( env!(\"CARGO_MANIFEST_DIR\"), \
     \"/../../testdata/transcript-canary/canaries.txt\" ))",
    "crates/transcript/tests/transcript_ingestion.rs: include_str!(concat!( \
     env!(\"CARGO_MANIFEST_DIR\"), \"/../../testdata/admission/incomplete-receipt.cbor.hex\" ))",
];

/// The one `include!` of a computed path, pinned as whole text.
const GENERATED_MODULE_INCLUDE: &str =
    "include\u{21}(concat!(env!(\"OUT_DIR\"), \"/academic.v1.rs\"));";

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// The crates this file reads are the crates the workspace compiles.
///
/// The floor every test below rests on. A walk that returned nothing would
/// satisfy every "no closure contains X" assertion in this file, so the crate
/// set is compared against the workspace member list in both directions: a
/// member with no directory fails, and a directory no member names fails.
#[test]
fn the_walk_reads_every_crate_the_workspace_compiles() -> TestResult {
    let repository = repository_root()?;
    let manifest = fs::read_to_string(repository.join("Cargo.toml"))?;
    let mut declared: BTreeSet<String> = BTreeSet::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "members = [" {
            inside = true;
            continue;
        }
        if inside {
            if trimmed == "]" {
                break;
            }
            let member = trimmed.trim_end_matches(',').trim_matches('"');
            declared.insert(member.to_owned());
        }
    }
    let walked: BTreeSet<String> = crate_directories(&repository)?
        .iter()
        .map(|path| relative(&repository, path))
        .collect();
    assert_eq!(
        walked, declared,
        "the crate directories and the workspace members are not the same set"
    );
    assert!(
        declared.len() >= 60,
        "the member list holds only {} crates",
        declared.len()
    );
    Ok(())
}

/// Every file the compiler pulls into a crate is a `*.rs` file a walk reads.
///
/// The statement `P2-A3` falsified. Two conditions, and the second is the one
/// the audit's `include!("witness_ext.inc")` walked past:
///
/// * the closure of a crate's **product** targets stays under that crate's own
///   `src`, so a `#[path]` reaching into `tests/`, into another crate, or out
///   of the repository fails by name;
/// * every file in every closure, product and test alike, is a `*.rs` file, so
///   a compiled file that no extension filter admits fails by name.
#[test]
fn every_file_the_compiler_compiles_is_a_rust_file_a_walk_reads() -> TestResult {
    let repository = repository_root()?;
    let crates = crate_directories(&repository)?;
    let mut product_total = 0_usize;
    let mut every_total = 0_usize;
    let mut outside_src: BTreeSet<String> = BTreeSet::new();

    for crate_directory in &crates {
        let source_root = crate_directory.join("src");
        let mut product: BTreeSet<PathBuf> = BTreeSet::new();
        for root in product_roots(crate_directory)? {
            product.extend(resolve(&root, &repository)?.files);
        }
        for file in &product {
            assert!(
                file.is_file(),
                "{} is compiled into {} and is not a file",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
            assert!(
                file.extension().is_some_and(|extension| extension == "rs"),
                "{} is compiled into {} and no `*.rs` walk reads it",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
            if !file.starts_with(&source_root) {
                outside_src.insert(relative(&repository, file));
            }
        }
        product_total += product.len();

        let mut every: BTreeSet<PathBuf> = BTreeSet::new();
        for root in all_roots(crate_directory)? {
            every.extend(resolve(&root, &repository)?.files);
        }
        for file in &every {
            assert!(
                file.is_file(),
                "{} is compiled into a target of {} and is not a file",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
            assert!(
                file.extension().is_some_and(|extension| extension == "rs"),
                "{} is compiled into a target of {} and no `*.rs` walk reads it",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
            assert!(
                file.starts_with(repository.join("crates")),
                "{} is compiled into a target of {} and lives outside `crates/`",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
        }
        every_total += every.len();
    }

    assert_eq!(
        outside_src,
        PRODUCT_ROOTS_OUTSIDE_SRC
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "a crate compiles product source that a walk of its `src` does not read"
    );
    // The floors. Both counts are sums over crates, so an empty walk in one
    // crate is not enough to trip them on its own; the crate-set equality above
    // is what carries that half.
    assert!(
        product_total >= 500,
        "the product closures hold only {product_total} files"
    );
    assert!(
        every_total >= 750,
        "the target closures hold only {every_total} files"
    );
    Ok(())
}

/// Every `*.rs` file under a crate's `src` is in that crate's product closure.
///
/// The other direction, and what makes the pair an equality rather than a
/// floor. Without it a file could sit in `src` compiled by nothing, be read by
/// every scan, and mean nothing — or, the shape that matters, a module could be
/// moved out of the tree the walks read while still compiling.
#[test]
fn every_product_file_is_compiled_by_its_own_crate() -> TestResult {
    let repository = repository_root()?;
    let mut unreached: BTreeSet<String> = BTreeSet::new();
    let mut examined = 0_usize;
    for crate_directory in crate_directories(&repository)? {
        let mut product: BTreeSet<PathBuf> = BTreeSet::new();
        for root in product_roots(&crate_directory)? {
            product.extend(resolve(&root, &repository)?.files);
        }
        for file in rust_files(&crate_directory.join("src"))? {
            examined += 1;
            if !product.contains(&file) {
                unreached.insert(relative(&repository, &file));
            }
        }
    }
    assert_eq!(
        unreached,
        SOURCE_NO_TARGET_OF_ITS_CRATE_COMPILES
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "a file under a crate's `src` is compiled by no target of that crate"
    );
    assert!(
        examined >= 500,
        "the `src` walk found only {examined} files"
    );
    Ok(())
}

/// Every path the compiler is given is one this reader resolved.
///
/// A computed path is where a textual reader stops being able to decide, so the
/// set of them is pinned whole rather than skipped, and the literal targets are
/// required to stay inside this repository — the escape an embedded file has
/// that a module does not.
#[test]
fn every_path_handed_to_the_compiler_is_one_this_reader_resolved() -> TestResult {
    let repository = repository_root()?;
    let mut computed: BTreeSet<String> = BTreeSet::new();
    let mut embedded: BTreeSet<(String, PathBuf)> = BTreeSet::new();
    for crate_directory in crate_directories(&repository)? {
        for root in all_roots(&crate_directory)? {
            let closure = resolve(&root, &repository)?;
            computed.extend(closure.computed);
            embedded.extend(closure.embedded);
        }
    }
    assert_eq!(
        computed,
        COMPUTED_INCLUDE_PATHS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "a file is included under a path this reader cannot resolve"
    );
    for (site, target) in &embedded {
        assert!(
            target.starts_with(&repository),
            "{site} embeds {}, which is outside this repository",
            target.display()
        );
        assert!(
            target.is_file(),
            "{site} embeds {}, which is not a file",
            target.display()
        );
    }
    assert!(
        embedded.len() >= 25,
        "the walk found only {} embedded files",
        embedded.len()
    );

    // The one computed path that injects items, pinned as its own text.
    let generated = fs::read_to_string(repository.join("crates/rpc/src/generated.rs"))?;
    let line = generated
        .lines()
        .find(|line| line.trim_start().starts_with("include!"))
        .ok_or("crates/rpc/src/generated.rs no longer includes a generated module")?;
    assert_eq!(
        line.trim(),
        GENERATED_MODULE_INCLUDE,
        "the generated module's include changed; the pin must change with it"
    );
    Ok(())
}
