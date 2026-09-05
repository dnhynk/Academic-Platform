//! The reader every source scan in this repository is measured against.
//!
//! One implementation, two test targets. `compilation_unit_scans.rs` compares
//! the set of files this resolves against the set the `*.rs` walks read;
//! `item_inventory_scans.rs` reads the **items** inside those files. They were
//! one file until `P2-RF25`, and a second copy of a lexer is a second place
//! for a lexer bug to hide: a scan built on a lexer that desynchronizes is not
//! a weaker scan but no scan.

#![allow(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The repository root, by climbing rather than by joining `..`.
///
/// A path carrying `..` components never strips as a prefix, so every relative
/// path printed below would silently become an absolute one.
pub fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "the manifest directory has no grandparent".into())
}

/// A repository-relative path with forward slashes.
pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Resolves `.` and `..` textually, without touching the filesystem.
///
/// A target that does not exist still has to produce a path, because a path the
/// walk never read is exactly what the tests below refuse.
pub fn normalize(path: &Path) -> PathBuf {
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
pub struct Lexed {
    /// The source, character for character, with comments and literal bodies
    /// blanked. One output character per input character, so an index into it
    /// is an index into [`Lexed::source`].
    pub code: Vec<char>,
    /// The unblanked source, for quoting a site back in a failure.
    pub source: Vec<char>,
    pub literals: BTreeMap<usize, String>,
}

/// Blanks comments and literal bodies, character position preserved.
///
/// Raw strings are handled explicitly: `P2-G4` found that a reader without them
/// desynchronizes at the first one and reads every literal after it as code.
pub fn lex(source: &str) -> Lexed {
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
pub fn token_at(code: &[char], at: usize, word: &str) -> bool {
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
pub fn module_directory(file: &Path) -> PathBuf {
    let parent = file.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    match file.file_stem().and_then(|stem| stem.to_str()) {
        Some("lib" | "main" | "mod") => parent,
        Some(stem) => parent.join(stem),
        None => parent,
    }
}

/// What one target root pulls into the compilation unit.
#[derive(Default)]
pub struct Closure {
    /// Every file the compiler reads as Rust, the root included.
    pub files: BTreeSet<PathBuf>,
    /// Every literal `include_str!`/`include_bytes!` target, with its site.
    pub embedded: BTreeSet<(String, PathBuf)>,
    /// Every `include!`, `include_str!` or `include_bytes!` whose argument is
    /// not a single string literal, as `file: whole invocation`.
    pub computed: BTreeSet<String>,
}

/// Resolves the compilation unit reachable from one target root.
///
/// A target root is a crate root whatever its stem — `tests/audit.rs` is the
/// root of its own crate — so it resolves its own `mod` declarations against
/// the directory it sits in rather than against a directory named after it.
pub fn resolve(root: &Path, repository: &Path) -> Result<Closure, Box<dyn Error>> {
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
pub fn auto_targets(directory: &Path, roots: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
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
pub fn product_roots(crate_directory: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
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
pub fn all_roots(crate_directory: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut roots = product_roots(crate_directory)?;
    for directory in ["tests", "benches", "examples"] {
        auto_targets(&crate_directory.join(directory), &mut roots)?;
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

/// Every crate directory in the workspace.
pub fn crate_directories(repository: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
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
pub fn rust_files(directory: &Path) -> Result<BTreeSet<PathBuf>, Box<dyn Error>> {
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
// Reading the items the compiler compiles
// ---------------------------------------------------------------------------

/// The item kinds Rust has, as this reader names them.
///
/// **Why the list is closed.** Rust's grammar admits an item only after
/// optional outer attributes, an optional visibility and a fixed set of
/// modifier keywords, and then exactly one of the keywords below or a macro
/// invocation. The Reference's *Items* chapter enumerates them; this is that
/// enumeration, transcribed, with what each can do to a type it names:
///
/// * `mod` — holds items that can;
/// * `extern` — binds a crate name, or declares foreign functions over one;
/// * `use` — re-exports an item that can, or renames the type;
/// * `fn` — takes one, returns one, builds one;
/// * `type` — **is** one, under another name;
/// * `struct`, `enum`, `union` — declare one, and their derives write impls;
/// * `const`, `static` — **hold** one, or hold a function pointer to one;
/// * `trait` — declares methods over one;
/// * `impl` — writes methods and conversions for one;
/// * `macro_rules`, `macro` — expand to any of the above.
///
/// `P2-A4`'s second audit walked past two whole-set inventories with the
/// `const` row of that list, which is neither a `pub fn` nor an `impl` header.
/// The repair is not a third spelling; it is to read the row set.
///
/// **What makes the transcription checkable rather than asserted.** Two
/// properties, both tested in `item_inventory_scans.rs`:
///
/// * an item-position construct whose head is none of these and is not a macro
///   invocation makes [`items_of`] return `Err`, so a form nobody predicted
///   fails by name rather than being skipped; and
/// * the extents this reader returns **tile** the file — every character
///   outside whitespace belongs to exactly one item — so an item it did not
///   see cannot exist without leaving a hole a test reads.
pub const ITEM_KEYWORDS: [&str; 14] = [
    "const",
    "enum",
    "extern",
    "fn",
    "impl",
    "macro",
    "macro_rules",
    "mod",
    "static",
    "struct",
    "trait",
    "type",
    "union",
    "use",
];

/// The keywords that may stand in front of an item's own keyword.
///
/// `const` and `extern` are on both lists: `const fn` and `extern "C" fn` are
/// modifiers while `const NAME:` and `extern crate` are items, and the two are
/// told apart by what follows.
pub const ITEM_MODIFIERS: [&str; 6] = ["async", "auto", "const", "default", "extern", "unsafe"];

/// One item of a compilation unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The repository-relative file it was read from.
    pub file: String,
    /// The `impl`, `trait` or `mod` chain it sits in, or the empty string.
    pub owner: String,
    /// `pub`, `pub(crate)`, `pub(super)`, `pub(in …)` or `priv`.
    pub visibility: String,
    /// One of [`ITEM_KEYWORDS`], or `macro-call`.
    pub kind: String,
    /// The name it declares, or the empty string for `impl` and `use`.
    pub name: String,
    /// The declaration, whitespace-collapsed and without its body.
    pub declaration: String,
    /// Its attributes, whitespace-collapsed, in source order.
    pub attributes: Vec<String>,
    /// The whole item, attributes and body included, whitespace-collapsed.
    ///
    /// Comments and string bodies are blank here, which is what makes a name
    /// found in it a name the compiler sees rather than one a doc comment
    /// mentions.
    pub text: String,
    /// Where the item starts in the lexed source, attributes included.
    pub start: usize,
    /// One past its last character.
    pub end: usize,
}

impl Item {
    /// The line a pin holds.
    #[must_use]
    pub fn key(&self) -> String {
        let owner = if self.owner.is_empty() {
            String::new()
        } else {
            format!("{} :: ", self.owner)
        };
        let attributes = if self.attributes.is_empty() {
            String::new()
        } else {
            format!("{} ", self.attributes.join(" "))
        };
        format!(
            "{} [{}] {owner}{attributes}{}",
            self.file, self.visibility, self.declaration
        )
    }

    /// Whether the item's own text names `word` as a whole token.
    #[must_use]
    pub fn names(&self, word: &str) -> bool {
        spells(&self.text, word)
    }

    /// Whether the item names `word`, or sits inside something that does.
    ///
    /// The owner half is what makes this a rule about a *type* rather than
    /// about a signature. `pub fn bytes(&self) -> &[u8]` inside
    /// `impl ReleasableArtifact` names no type in its own text, and it is the
    /// accessor that crate's note calls the only one there is; a second
    /// written beside it is invisible to any sweep that reads signatures
    /// alone, and is an extra key here.
    #[must_use]
    pub fn reaches(&self, word: &str) -> bool {
        self.names(word) || spells(&self.owner, word)
    }

    /// The names this item introduces that another item could write a type
    /// with: a type alias, a data type, a trait, a module, a macro, and every
    /// capitalized identifier a `use` tree binds.
    ///
    /// A rule keyed on one type name is a rule about a spelling until this
    /// closes over it. `type Removed = RestrictedOriginal;` is an item that
    /// names the closed type, so it is in the set; because it is, `Removed`
    /// becomes a name that puts the next item in the set too.
    #[must_use]
    pub fn introduced_type_names(&self) -> Vec<String> {
        match self.kind.as_str() {
            "type" | "struct" | "enum" | "union" | "trait" | "mod" | "macro" | "macro_rules" => {
                if self.name.is_empty() {
                    Vec::new()
                } else {
                    vec![self.name.clone()]
                }
            }
            "use" => self
                .declaration
                .split(|character: char| !(character.is_alphanumeric() || character == '_'))
                .filter(|word| word.starts_with(|first: char| first.is_uppercase()))
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// The source with comments blanked and string bodies left in place.
///
/// Two views of one set of positions, and both are needed. A name is read
/// from the fully blanked view, so a type named in a doc comment is not a
/// route; a pinned key is read from this one, so a `cfg` for one operating
/// system and a `cfg` for another are two keys rather than one blanked key
/// that reads the same. Blanking preserved every position, so writing the
/// bodies back is an in-place edit.
#[must_use]
pub fn restored_literals(source: &str) -> Vec<char> {
    let Lexed { code, literals, .. } = lex(source);
    let mut restored = code;
    for (opening, body) in &literals {
        for (offset, character) in body.chars().enumerate() {
            if let Some(slot) = restored.get_mut(opening + 1 + offset) {
                *slot = character;
            }
        }
        // The closing quote is blanked too, and a key that ends mid-literal
        // reads as an unterminated string to anyone who has to edit the pin.
        if let Some(slot) = restored.get_mut(opening + 1 + body.chars().count()) {
            *slot = 0x22 as char;
        }
    }
    restored
}

/// Whether `text` holds `word` as a whole token.
fn spells(text: &str, word: &str) -> bool {
    let characters: Vec<char> = text.chars().collect();
    (0..characters.len()).any(|at| token_at(&characters, at, word))
}

/// Reads `source` into the items the compiler reads, recursively.
///
/// # Errors
///
/// When an item-position construct is none of [`ITEM_KEYWORDS`] and is not a
/// macro invocation. That is the default-deny: a form this reader has no rule
/// for stops the scan rather than passing through it.
pub fn items_of(file: &str, source: &str) -> Result<Vec<Item>, Box<dyn Error>> {
    let Lexed { code, .. } = lex(source);
    let restored = restored_literals(source);
    let mut found = Vec::new();
    read_items(file, &code, &restored, 0, code.len(), "", &mut found)?;
    Ok(found)
}

/// Whether `character` may appear in a path.
fn path_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == ':'
}

/// The word starting at `at`, or the empty string.
fn word_at(code: &[char], at: usize) -> String {
    code.get(at..)
        .unwrap_or_default()
        .iter()
        .take_while(|character| character.is_alphanumeric() || **character == '_')
        .collect()
}

/// The next index at or after `from` that is not whitespace.
fn skip_space(code: &[char], from: usize) -> usize {
    let mut cursor = from;
    while code.get(cursor).is_some_and(|c| c.is_whitespace()) {
        cursor += 1;
    }
    cursor
}

/// One past the delimiter matching the one at `open`.
fn matching(code: &[char], open: usize) -> usize {
    let (opening, closing) = match code.get(open) {
        Some('(') => ('(', ')'),
        Some('[') => ('[', ']'),
        Some('{') => ('{', '}'),
        _ => return open,
    };
    let mut cursor = open + 1;
    let mut depth = 1_i32;
    while cursor < code.len() && depth > 0 {
        if code[cursor] == opening {
            depth += 1;
        } else if code[cursor] == closing {
            depth -= 1;
        }
        cursor += 1;
    }
    cursor
}

/// One past the `;` that ends a declaration, ignoring every nested delimiter.
fn to_semicolon(code: &[char], from: usize) -> usize {
    let mut cursor = from;
    while cursor < code.len() {
        match code[cursor] {
            '(' | '[' | '{' => cursor = matching(code, cursor),
            ';' => return cursor + 1,
            _ => cursor += 1,
        }
    }
    cursor
}

/// One past the closing quote of the string literal opening at `at`.
///
/// Read from the restored view. `lex` keeps a literal's opening quote and
/// blanks its closing one, so a scan for `"` over the blanked view runs to
/// the end of the file: `unsafe extern "C" { … }` was read as an extern
/// block with no body until this took the other view.
fn to_string_end(restored: &[char], at: usize) -> usize {
    let mut cursor = at + 1;
    while cursor < restored.len() && restored[cursor] != '"' {
        cursor += 1;
    }
    cursor + 1
}

/// The characters between `from` and `to`, whitespace-collapsed.
fn collapse(code: &[char], from: usize, to: usize) -> String {
    code.get(from..to.min(code.len()))
        .unwrap_or_default()
        .iter()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reads the items between `from` and `to` into `found`.
fn read_items(
    file: &str,
    code: &[char],
    restored: &[char],
    from: usize,
    to: usize,
    owner: &str,
    found: &mut Vec<Item>,
) -> Result<(), Box<dyn Error>> {
    let mut index = from;
    loop {
        index = skip_space(code, index);
        if index >= to {
            return Ok(());
        }
        let start = index;

        // Attributes first, and they are kept rather than skipped. `P2-A4`'s
        // F3 was an attribute written on the same line as an `impl`, which
        // moved the keyword off the line start and blinded two inventories
        // anchored on it. Reading the item rather than the line is what makes
        // that a non-event; keeping the attribute in the key is what makes a
        // `#[derive(Serialize)]` added to a closed type an extra entry.
        let mut attributes = Vec::new();
        let mut inner = false;
        loop {
            let at = skip_space(code, index);
            if code.get(at) != Some(&'#') {
                index = at;
                break;
            }
            // `#![…]` belongs to the item that *encloses* it -- the file or
            // the `mod` block -- so it is an item of its own here rather
            // than an attribute of whatever is written next. Attaching it
            // forward would put a file's `#![allow]` into the key of
            // whichever declaration happened to come first.
            if code.get(at + 1) == Some(&'!') {
                if !attributes.is_empty() {
                    return Err(format!(
                        "{file}: an inner attribute after an outer one, at `{}`",
                        collapse(restored, at, (at + 60).min(to))
                    )
                    .into());
                }
                if code.get(at + 2) != Some(&'[') {
                    index = at;
                    break;
                }
                inner = true;
                index = matching(code, at + 2);
                attributes.push(collapse(restored, at, index));
                break;
            }
            if code.get(at + 1) != Some(&'[') {
                index = at;
                break;
            }
            let end = matching(code, at + 1);
            attributes.push(collapse(restored, at, end));
            index = end;
        }
        if inner {
            found.push(Item {
                file: file.to_owned(),
                owner: owner.to_owned(),
                visibility: "priv".to_owned(),
                kind: "attribute".to_owned(),
                name: String::new(),
                declaration: attributes.join(" "),
                attributes: Vec::new(),
                text: collapse(code, start, index),
                start,
                end: index,
            });
            continue;
        }
        index = skip_space(code, index);
        if index >= to {
            if attributes.is_empty() {
                return Ok(());
            }
            // An outer attribute with no item after it is not Rust.
            return Err(format!(
                "{file}: an outer attribute with nothing to attach to, at `{}`",
                collapse(restored, start, (start + 60).min(to))
            )
            .into());
        }

        let mut visibility = "priv".to_owned();
        if word_at(code, index) == "pub" {
            let after = skip_space(code, index + 3);
            if code.get(after) == Some(&'(') {
                let end = matching(code, after);
                visibility = collapse(code, index, end);
                index = end;
            } else {
                visibility = "pub".to_owned();
                index = after;
            }
        }

        let declaration_start = skip_space(code, index);
        let mut keyword = String::new();
        loop {
            index = skip_space(code, index);
            let word = word_at(code, index);
            if word.is_empty() {
                break;
            }
            if word == "const" {
                // `const fn`, `const unsafe fn`, `const async fn` and
                // `const extern "…" fn` are modifiers; `const NAME:` and
                // `const _:` are the item.
                let after = skip_space(code, index + 5);
                let next = word_at(code, after);
                if ["fn", "unsafe", "async", "extern"].contains(&next.as_str()) {
                    index = after;
                    continue;
                }
                keyword = word;
                break;
            }
            if word == "extern" {
                // `extern crate NAME;` and `extern "…" { … }` are items;
                // `extern "…" fn` is a modifier.
                let mut after = skip_space(code, index + 6);
                if code.get(after) == Some(&'"') {
                    after = skip_space(code, to_string_end(restored, after));
                }
                if word_at(code, after) == "fn" {
                    index = after;
                    continue;
                }
                keyword = word;
                break;
            }
            if ITEM_KEYWORDS.contains(&word.as_str()) {
                keyword = word;
                break;
            }
            if ITEM_MODIFIERS.contains(&word.as_str()) {
                index = skip_space(code, index + word.chars().count());
                continue;
            }
            break;
        }

        if keyword.is_empty() {
            // The only remaining legal item is a macro invocation:
            // `path` `!` `DelimTokenTree`, three tokens with whitespace
            // allowed between them, which is how `T217`'s reader learned to
            // see `include !(…)` and `include!{…}`.
            let path_end = index
                + code
                    .get(index..)
                    .unwrap_or_default()
                    .iter()
                    .take_while(|character| path_character(**character))
                    .count();
            let bang = skip_space(code, path_end);
            if path_end == index || code.get(bang) != Some(&'!') {
                return Err(format!(
                    "{file}: an item form this reader has no rule for, at `{}`",
                    collapse(code, start, (start + 60).min(to))
                )
                .into());
            }
            let open = skip_space(code, bang + 1);
            if !matches!(code.get(open), Some('(' | '[' | '{')) {
                return Err(format!(
                    "{file}: a macro invocation with no delimited tree, at `{}`",
                    collapse(code, start, (start + 60).min(to))
                )
                .into());
            }
            let tree = matching(code, open);
            let end = if code.get(open) == Some(&'{') {
                let after = skip_space(code, tree);
                if code.get(after) == Some(&';') {
                    after + 1
                } else {
                    tree
                }
            } else {
                to_semicolon(code, tree)
            };
            found.push(Item {
                file: file.to_owned(),
                owner: owner.to_owned(),
                visibility,
                kind: "macro-call".to_owned(),
                name: collapse(code, index, path_end),
                declaration: collapse(restored, declaration_start, tree),
                attributes,
                text: collapse(code, start, end),
                start,
                end,
            });
            index = end;
            continue;
        }

        let keyword_end = index + keyword.chars().count();
        let shape = item_shape(file, code, restored, &keyword, keyword_end)?;
        let item = Item {
            file: file.to_owned(),
            owner: owner.to_owned(),
            visibility,
            kind: keyword,
            name: shape.name,
            declaration: collapse(restored, declaration_start, shape.declaration_end),
            attributes,
            text: collapse(code, start, shape.end),
            start,
            end: shape.end,
        };
        // `mod`, `impl`, `trait` and an `extern` block hold items of their own,
        // and those are what a rule about a *type* has to see: a method added
        // inside `impl RestrictedOriginal` names no type in its own signature.
        // A function body is **not** descended into: an item declared there is
        // reachable from nowhere, and its text is inside this item's own text,
        // so a name it mentions still counts as this item naming it.
        if let Some((open, close)) = shape.body {
            let nested_owner = if owner.is_empty() {
                item.declaration.clone()
            } else {
                format!("{owner} :: {}", item.declaration)
            };
            read_items(file, code, restored, open, close, &nested_owner, found)?;
        }
        index = shape.end;
        found.push(item);
    }
}

/// What one item keyword's own grammar says about where the item ends.
struct Shape {
    name: String,
    declaration_end: usize,
    body: Option<(usize, usize)>,
    end: usize,
}

/// Reads one item's extent, given the keyword that opened it.
fn item_shape(
    file: &str,
    code: &[char],
    restored: &[char],
    keyword: &str,
    keyword_end: usize,
) -> Result<Shape, Box<dyn Error>> {
    let after = skip_space(code, keyword_end);
    match keyword {
        // `impl` has no name; its header runs to the opening brace.
        "impl" => {
            let open = scan_to(code, after, &['{']);
            if open >= code.len() {
                return Err(format!("{file}: an impl header with no block").into());
            }
            let end = matching(code, open);
            Ok(Shape {
                name: String::new(),
                declaration_end: open,
                body: Some((open + 1, end - 1)),
                end,
            })
        }
        // A named block, or a named `;`.
        "mod" | "trait" | "macro" => {
            let name = word_at(code, after);
            let cursor = scan_to(code, after + name.chars().count(), &['{', ';']);
            if code.get(cursor) == Some(&';') {
                return Ok(Shape {
                    name,
                    declaration_end: cursor,
                    body: None,
                    end: cursor + 1,
                });
            }
            let end = matching(code, cursor);
            Ok(Shape {
                name,
                declaration_end: cursor,
                // A `macro NAME { … }` body is a transcriber, not items.
                body: (keyword != "macro").then_some((cursor + 1, end - 1)),
                end,
            })
        }
        "macro_rules" => {
            let bang = skip_space(code, keyword_end);
            if code.get(bang) != Some(&'!') {
                return Err(format!("{file}: `macro_rules` with no `!`").into());
            }
            let at = skip_space(code, bang + 1);
            let name = word_at(code, at);
            let open = skip_space(code, at + name.chars().count());
            let end = matching(code, open);
            Ok(Shape {
                name,
                declaration_end: open,
                body: None,
                end,
            })
        }
        // `extern crate NAME;`, or an `extern "…" { … }` block.
        "extern" => {
            if word_at(code, after) == "crate" {
                let at = skip_space(code, after + 5);
                let end = to_semicolon(code, at);
                return Ok(Shape {
                    name: word_at(code, at),
                    declaration_end: end - 1,
                    body: None,
                    end,
                });
            }
            let open = if code.get(after) == Some(&'"') {
                skip_space(code, to_string_end(restored, after))
            } else {
                after
            };
            if code.get(open) != Some(&'{') {
                return Err(format!("{file}: an extern block with no body").into());
            }
            let end = matching(code, open);
            Ok(Shape {
                name: String::new(),
                declaration_end: open,
                body: Some((open + 1, end - 1)),
                end,
            })
        }
        // A signature, then a body or a `;`.
        "fn" => {
            let name = word_at(code, after);
            let cursor = scan_to(code, after + name.chars().count(), &['{', ';']);
            if code.get(cursor) == Some(&';') {
                return Ok(Shape {
                    name,
                    declaration_end: cursor,
                    body: None,
                    end: cursor + 1,
                });
            }
            if cursor >= code.len() {
                return Err(format!("{file}: a fn with no body and no `;`").into());
            }
            Ok(Shape {
                name,
                declaration_end: cursor,
                body: None,
                end: matching(code, cursor),
            })
        }
        // `struct NAME;`, `struct NAME(…);` or a braced body. Fields are not
        // items, so none of the three is descended into.
        "struct" | "enum" | "union" => {
            let name = word_at(code, after);
            let cursor = scan_to(code, after + name.chars().count(), &['{', ';']);
            if code.get(cursor) == Some(&';') {
                return Ok(Shape {
                    name,
                    declaration_end: cursor,
                    body: None,
                    end: cursor + 1,
                });
            }
            if cursor >= code.len() {
                return Err(format!("{file}: a {keyword} with no body and no `;`").into());
            }
            Ok(Shape {
                name,
                declaration_end: cursor,
                body: None,
                end: matching(code, cursor),
            })
        }
        // `const`, `static` and `type` all run to a `;`, and each may carry a
        // whole expression, a block included, on the way there.
        // `pub const NAME: fn(&T) -> U = |value| …;` is that shape, and it is
        // the route `P2-A4`'s second audit walked out of a restricted original
        // while an `impl` sweep and a `pub fn` sweep both reported whole sets.
        "const" | "static" => {
            let at = skip_space(
                code,
                if word_at(code, after) == "mut" {
                    after + 3
                } else {
                    after
                },
            );
            let name = if code.get(at) == Some(&'_') {
                "_".to_owned()
            } else {
                word_at(code, at)
            };
            let end = to_semicolon(code, at);
            Ok(Shape {
                name,
                declaration_end: find_initializer(code, at, end),
                body: None,
                end,
            })
        }
        "type" | "use" => {
            let end = to_semicolon(code, after);
            Ok(Shape {
                name: word_at(code, after),
                declaration_end: end.saturating_sub(1),
                body: None,
                end,
            })
        }
        other => Err(format!("{file}: no rule for the item keyword `{other}`").into()),
    }
}

/// The first index at or after `from` holding one of `stops`, skipping every
/// delimited tree on the way.
fn scan_to(code: &[char], from: usize, stops: &[char]) -> usize {
    let mut cursor = from;
    while cursor < code.len() {
        let current = code[cursor];
        if stops.contains(&current) {
            return cursor;
        }
        if current == '(' || current == '[' {
            cursor = matching(code, cursor);
            continue;
        }
        cursor += 1;
    }
    cursor
}

/// Where a `const` or `static` declaration stops and its value begins.
///
/// The declared type is the half a rule about routes reads; the value is in
/// [`Item::text`] either way, so a closure body naming a private field is
/// still a name this item carries.
fn find_initializer(code: &[char], from: usize, end: usize) -> usize {
    let mut cursor = from;
    let mut angle = 0_i32;
    while cursor < end {
        match code[cursor] {
            '(' | '[' | '{' => {
                cursor = matching(code, cursor);
                continue;
            }
            '<' => angle += 1,
            '>' => angle -= 1,
            '=' if angle <= 0 && code.get(cursor + 1) != Some(&'=') => return cursor,
            _ => {}
        }
        cursor += 1;
    }
    end.saturating_sub(1)
}
