//! The readers. One per [`FileKind`], each producing the facts section 17.3's
//! third stage names.
//!
//! ## Why these are hand-written
//!
//! `CONTRIBUTING.md` admits a dependency through owner, licence, feature and
//! advisory review and an argument for why it belongs inside its trust
//! boundary. A general parser generator would be several crates inside the
//! boundary that reads untrusted repository bytes, and its grammars would still
//! not cover section 17.1's manifest, lock, schema, container and pipeline
//! formats. So the readers here are the same shape as this repository's own
//! `.gitignore` parser and its data-record escaper: small, total over their
//! input, and honest about what they do not model — which is what
//! [`crate::index::support`] and the coverage gaps exist to say.
//!
//! ## Two views of a file, and why there are two
//!
//! [`blank_comments`] replaces every comment byte with a space.
//! [`blank_comments_and_strings`] replaces string literal bytes too. Both
//! preserve every byte offset, so a span computed on either view is a span into
//! the original file.
//!
//! Calls and declarations are read from the second view, because a `//` comment
//! that mentions a function name is not a call. Imports and configuration are
//! read from the first, because in TypeScript, JSON, YAML and TOML the thing
//! being named *is* a string literal — `import x from "redis"` says nothing on
//! the second view at all.
//!
//! ## Nothing here is public
//!
//! Every item in this module is `pub(crate)`. What comes out of a reader is a
//! token lifted out of untrusted repository bytes, and the only thing this
//! crate does with such a token is compare it against a needle its caller
//! supplied. `no_analyzed_byte_reaches_a_text_accessor` is the executed half of
//! that: it analyzes a corpus whose every identifier is a canary and requires
//! the canary to appear in no public accessor's output.

use crate::index::{FileKind, SourceSpan, SymbolFingerprint, SymbolKind};

/// One declaration, with the extent of its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Declaration {
    pub(crate) fingerprint: SymbolFingerprint,
    pub(crate) kind: SymbolKind,
    pub(crate) span: SourceSpan,
    /// The declared name, normalized. Used to resolve a call and never exposed.
    pub(crate) name: String,
    /// Whether the declaration is reachable without being called: an entry
    /// point, a test, or a name the file exports.
    pub(crate) is_root: bool,
}

/// One call, with the qualified callee and where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallSite {
    /// The full qualified callee, normalized: `redis::client::open` or
    /// `redis.open`, lowercased with `::` folded to `.`.
    pub(crate) callee: String,
    /// The last segment, which is what resolves against a declaration.
    pub(crate) leaf: String,
    pub(crate) span: SourceSpan,
}

/// One token lifted out of a file for matching against a caller's needle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenSite {
    pub(crate) token: String,
    pub(crate) span: SourceSpan,
}

/// One dependency named by a manifest or a lock file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencySite {
    pub(crate) token: String,
    pub(crate) span: SourceSpan,
    /// Whether the manifest names it as a development or build dependency.
    /// A `[dev-dependencies]` entry is evidence about tests, not about what
    /// ships, and section 18.1's scope is where that has to show up.
    pub(crate) development_only: bool,
}

/// One definition-to-use edge inside a file.
///
/// This is the whole of what this analyzer calls data flow: a module-level
/// binding, and a later mention of its name in the same file. It is a def-use
/// chain and not an interprocedural analysis, and the contract page says so in
/// those words rather than in the phrase section 17.3 uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataFlowEdge {
    pub(crate) definition: SymbolFingerprint,
    pub(crate) use_span: SourceSpan,
}

/// Everything one file yielded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FileFacts {
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) calls: Vec<CallSite>,
    pub(crate) imports: Vec<TokenSite>,
    pub(crate) config_tokens: Vec<TokenSite>,
    pub(crate) dependencies: Vec<DependencySite>,
    pub(crate) schema_objects: Vec<Declaration>,
    pub(crate) iac_tokens: Vec<TokenSite>,
    pub(crate) data_flow: Vec<DataFlowEdge>,
}

/// Reads one file, dispatching on what it was recognised as.
///
/// Total over [`FileKind`] with no default arm: an added file kind is a compile
/// error here, so a reader is written or the kind is explicitly given none.
pub(crate) fn read(path: &str, file: FileKind, bytes: &[u8]) -> FileFacts {
    let Ok(source) = core::str::from_utf8(bytes) else {
        return FileFacts::default();
    };
    match file {
        FileKind::RustSource => braced_source(path, source, Dialect::Rust),
        FileKind::TypeScriptSource => braced_source(path, source, Dialect::TypeScript),
        FileKind::PythonSource => python_source(path, source),
        FileKind::SqlScript => sql_script(path, source),
        FileKind::CargoManifest | FileKind::PythonManifest => toml_document(source, true),
        FileKind::NodeManifest => json_document(source, true),
        FileKind::LockFile => lock_document(source),
        FileKind::ConfigDocument => config_document(path, source),
        FileKind::ContainerFile => container_file(source),
        FileKind::ComposeFile | FileKind::CiWorkflow => yaml_document(source, false, true),
        FileKind::Prose | FileKind::Unsupported => FileFacts::default(),
    }
}

/// Which comment and literal shapes a file uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Rust,
    TypeScript,
}

/// Replaces every comment byte with a space, preserving every offset.
fn blank_comments(source: &str, line_marker: &str, block: bool) -> Vec<u8> {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let marker = line_marker.as_bytes();
    let mut index = 0;
    let mut depth = 0_usize;
    while index < bytes.len() {
        if depth > 0 {
            if block && bytes[index..].starts_with(b"/*") {
                depth += 1;
                blank(&mut out, index, 2);
                index += 2;
                continue;
            }
            if block && bytes[index..].starts_with(b"*/") {
                depth -= 1;
                blank(&mut out, index, 2);
                index += 2;
                continue;
            }
            if bytes[index] != b'\n' {
                out[index] = b' ';
            }
            index += 1;
            continue;
        }
        if block && bytes[index..].starts_with(b"/*") {
            depth = 1;
            blank(&mut out, index, 2);
            index += 2;
            continue;
        }
        if !marker.is_empty() && bytes[index..].starts_with(marker) {
            while index < bytes.len() && bytes[index] != b'\n' {
                out[index] = b' ';
                index += 1;
            }
            continue;
        }
        index += 1;
    }
    out
}

/// Blanks `length` bytes from `at`.
fn blank(out: &mut [u8], at: usize, length: usize) {
    for byte in out.iter_mut().skip(at).take(length) {
        *byte = b' ';
    }
}

/// Blanks comments and every string and character literal.
///
/// Raw strings are modelled, for the reason `P2-G4` recorded about
/// `crates/record/tests/record_scans.rs`: a lexer without them desynchronizes
/// at the first one and reads every literal after it as code.
fn blank_comments_and_strings(source: &str, dialect: Dialect) -> Vec<u8> {
    let bytes = source.as_bytes();
    let mut out = blank_comments(source, "//", true);
    let mut index = 0;
    while index < bytes.len() {
        if out[index] == b' ' && bytes[index] != b' ' {
            index += 1;
            continue;
        }
        let quotes: &[u8] = match dialect {
            Dialect::Rust => b"\"",
            Dialect::TypeScript => b"\"'`",
        };
        if dialect == Dialect::Rust && bytes[index] == b'r' {
            let mut hashes = 0;
            let mut cursor = index + 1;
            while bytes.get(cursor) == Some(&b'#') {
                hashes += 1;
                cursor += 1;
            }
            if bytes.get(cursor) == Some(&b'"') {
                let closing = format!("\"{}", "#".repeat(hashes));
                let body = cursor + 1;
                let end = find(bytes, body, closing.as_bytes())
                    .map_or(bytes.len(), |at| at + closing.len());
                blank_keep_newlines(&mut out, bytes, index, end);
                index = end;
                continue;
            }
        }
        if quotes.contains(&bytes[index]) {
            let quote = bytes[index];
            let mut cursor = index + 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor += 2;
                    continue;
                }
                if bytes[cursor] == quote {
                    cursor += 1;
                    break;
                }
                cursor += 1;
            }
            let end = cursor.min(bytes.len());
            blank_keep_newlines(&mut out, bytes, index, end);
            index = end;
            continue;
        }
        index += 1;
    }
    out
}

/// Blanks `[from, to)` but keeps newlines, so line numbers stay computable.
fn blank_keep_newlines(out: &mut [u8], bytes: &[u8], from: usize, to: usize) {
    for offset in from..to.min(bytes.len()) {
        if bytes[offset] != b'\n' {
            out[offset] = b' ';
        }
    }
}

/// The first occurrence of `needle` in `haystack` at or after `from`.
fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| at + from)
}

/// One-based line numbers for a byte range.
fn lines_of(source: &str, start: usize, end: usize) -> (u32, u32) {
    let bytes = source.as_bytes();
    let count = |upto: usize| -> u32 {
        u32::try_from(
            bytes[..upto.min(bytes.len())]
                .iter()
                .filter(|&&b| b == b'\n')
                .count(),
        )
        .unwrap_or(u32::MAX)
        .saturating_add(1)
    };
    (count(start), count(end.saturating_sub(1)))
}

/// Builds a span from a byte range.
fn span_of(source: &str, start: usize, end: usize) -> SourceSpan {
    let (first, last) = lines_of(source, start, end);
    SourceSpan::new(
        u32::try_from(start).unwrap_or(u32::MAX),
        u32::try_from(end).unwrap_or(u32::MAX),
        first,
        last,
    )
}

/// Whether a byte can appear inside an identifier.
const fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Reads the identifier starting at `at`, or `None`.
fn identifier_at(bytes: &[u8], at: usize) -> Option<(String, usize)> {
    if at >= bytes.len() || !(bytes[at].is_ascii_alphabetic() || bytes[at] == b'_') {
        return None;
    }
    let mut end = at;
    while end < bytes.len() && is_identifier_byte(bytes[end]) {
        end += 1;
    }
    Some((String::from_utf8_lossy(&bytes[at..end]).into_owned(), end))
}

/// Skips spaces and tabs forward.
fn skip_blanks(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && (bytes[at] == b' ' || bytes[at] == b'\t' || bytes[at] == b'\n') {
        at += 1;
    }
    at
}

/// Keywords that are followed by `(` and are not calls.
const NOT_A_CALL: [&str; 16] = [
    "if", "while", "for", "match", "switch", "return", "catch", "fn", "def", "function", "class",
    "let", "const", "with", "assert", "yield",
];

/// Reads a brace-delimited language: Rust or TypeScript.
fn braced_source(path: &str, source: &str, dialect: Dialect) -> FileFacts {
    let code = blank_comments_and_strings(source, dialect);
    let no_comments = blank_comments(source, "//", true);
    let mut facts = FileFacts::default();

    let declaration_keywords: &[(&str, SymbolKind)] = match dialect {
        Dialect::Rust => &[
            ("fn", SymbolKind::Function),
            ("struct", SymbolKind::Type),
            ("enum", SymbolKind::Type),
            ("trait", SymbolKind::Type),
            ("type", SymbolKind::Type),
            ("const", SymbolKind::Constant),
            ("static", SymbolKind::Constant),
        ],
        Dialect::TypeScript => &[
            ("function", SymbolKind::Function),
            ("class", SymbolKind::Type),
            ("interface", SymbolKind::Type),
            ("type", SymbolKind::Type),
            ("const", SymbolKind::Constant),
            ("let", SymbolKind::Constant),
        ],
    };

    let mut index = 0;
    while index < code.len() {
        let Some((word, after)) = identifier_at(&code, index) else {
            index += 1;
            continue;
        };
        if index > 0 && is_identifier_byte(code[index - 1]) {
            index = after;
            continue;
        }
        if let Some((_, kind)) = declaration_keywords
            .iter()
            .find(|(keyword, _)| *keyword == word)
        {
            let name_at = skip_blanks(&code, after);
            if let Some((name, name_end)) = identifier_at(&code, name_at) {
                let end = declaration_end(&code, name_end);
                let is_test = *kind == SymbolKind::Function
                    && preceded_by_test_attribute(source, &no_comments, index);
                let symbol_kind = if is_test {
                    SymbolKind::TestFunction
                } else {
                    *kind
                };
                let normalized = name.to_ascii_lowercase();
                facts.declarations.push(Declaration {
                    fingerprint: SymbolFingerprint::of(path, symbol_kind, &name),
                    kind: symbol_kind,
                    span: span_of(source, index, end),
                    is_root: is_test
                        || normalized == "main"
                        || exported_at(&no_comments, index, dialect),
                    name: normalized,
                });
                index = name_end;
                continue;
            }
        }
        // A call: an identifier, possibly qualified, immediately before `(`.
        let open = skip_blanks(&code, after);
        if code.get(open) == Some(&b'(') && !NOT_A_CALL.contains(&word.as_str()) {
            let (callee, start) = qualified_before(&code, index, dialect);
            facts.calls.push(CallSite {
                leaf: word.to_ascii_lowercase(),
                callee,
                span: span_of(source, start, open + 1),
            });
        }
        index = after;
    }

    facts.imports = match dialect {
        Dialect::Rust => rust_imports(source, &code),
        Dialect::TypeScript => typescript_imports(source, &no_comments),
    };
    facts.data_flow = definition_uses(&code, source, &facts.declarations);
    facts
}

/// The qualified path ending at the identifier that starts at `at`.
fn qualified_before(code: &[u8], at: usize, dialect: Dialect) -> (String, usize) {
    let separator: &[u8] = match dialect {
        Dialect::Rust => b"::",
        Dialect::TypeScript => b".",
    };
    let mut start = at;
    loop {
        if start < separator.len() || &code[start - separator.len()..start] != separator {
            break;
        }
        let mut cursor = start - separator.len();
        while cursor > 0 && is_identifier_byte(code[cursor - 1]) {
            cursor -= 1;
        }
        if cursor == start - separator.len() {
            break;
        }
        start = cursor;
    }
    let mut end = at;
    while end < code.len() && is_identifier_byte(code[end]) {
        end += 1;
    }
    let raw = String::from_utf8_lossy(&code[start..end]).into_owned();
    (raw.replace("::", ".").to_ascii_lowercase(), start)
}

/// The end of a declaration: its matching `}`, or the `;` that ends it.
fn declaration_end(code: &[u8], from: usize) -> usize {
    let mut index = from;
    while index < code.len() {
        match code[index] {
            b'{' => {
                let mut depth = 1_usize;
                let mut cursor = index + 1;
                while cursor < code.len() && depth > 0 {
                    match code[cursor] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        _ => (),
                    }
                    cursor += 1;
                }
                return cursor;
            }
            b';' => return index + 1,
            _ => index += 1,
        }
    }
    code.len()
}

/// Whether a `#[test]` attribute sits on one of the three preceding lines.
fn preceded_by_test_attribute(source: &str, no_comments: &[u8], at: usize) -> bool {
    let head = &no_comments[..at.min(no_comments.len())];
    let text = String::from_utf8_lossy(head);
    text.lines()
        .rev()
        .take(4)
        .any(|line| line.trim_start().starts_with("#[test]"))
        && source.len() >= at
}

/// Whether the declaration at `at` is exported.
fn exported_at(no_comments: &[u8], at: usize, dialect: Dialect) -> bool {
    let head = &no_comments[..at.min(no_comments.len())];
    let text = String::from_utf8_lossy(head);
    let Some(line) = text.lines().next_back() else {
        return false;
    };
    match dialect {
        Dialect::Rust => line.trim_start().starts_with("pub"),
        Dialect::TypeScript => line.trim_start().starts_with("export"),
    }
}

/// `use a::b::c;` and `extern crate a;`, reduced to the first segment.
fn rust_imports(source: &str, code: &[u8]) -> Vec<TokenSite> {
    let text = String::from_utf8_lossy(code).into_owned();
    let mut found = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let lead = line.len() - trimmed.len();
        let rest = trimmed
            .strip_prefix("use ")
            .or_else(|| trimmed.strip_prefix("pub use "))
            .or_else(|| trimmed.strip_prefix("extern crate "));
        if let Some(rest) = rest {
            let head = rest
                .split([':', ';', ' ', '{', ','])
                .find(|segment| !segment.is_empty())
                .unwrap_or("");
            if !head.is_empty() && head != "crate" && head != "self" && head != "super" {
                found.push(TokenSite {
                    token: head.to_ascii_lowercase(),
                    span: span_of(source, offset + lead, offset + line.len()),
                });
            }
        }
        offset += line.len();
    }
    found
}

/// `import … from "pkg"`, `require("pkg")` and `import("pkg")`.
fn typescript_imports(source: &str, no_comments: &[u8]) -> Vec<TokenSite> {
    let text = String::from_utf8_lossy(no_comments).into_owned();
    let mut found = Vec::new();
    for marker in ["from ", "require(", "import("] {
        let mut cursor = 0;
        while let Some(at) = text[cursor..].find(marker) {
            let start = cursor + at + marker.len();
            let bytes = text.as_bytes();
            let quote_at = skip_blanks(bytes, start);
            cursor = start;
            let Some(&quote) = bytes.get(quote_at) else {
                continue;
            };
            if quote != b'"' && quote != b'\'' {
                continue;
            }
            let body = quote_at + 1;
            let Some(end) = bytes[body..].iter().position(|&byte| byte == quote) else {
                continue;
            };
            let specifier = &text[body..body + end];
            let package = package_of_specifier(specifier);
            if !package.is_empty() {
                found.push(TokenSite {
                    token: package,
                    span: span_of(source, quote_at, body + end + 1),
                });
            }
            cursor = body + end;
        }
    }
    found
}

/// The package a module specifier names: `@scope/name` or its first segment.
///
/// A relative specifier names a file in this repository rather than a package,
/// and produces no import token at all.
fn package_of_specifier(specifier: &str) -> String {
    if specifier.starts_with('.') || specifier.starts_with('/') {
        return String::new();
    }
    let mut segments = specifier.split('/');
    let first = segments.next().unwrap_or("");
    if first.starts_with('@') {
        let second = segments.next().unwrap_or("");
        return format!("{first}/{second}").to_ascii_lowercase();
    }
    first.to_ascii_lowercase()
}

/// A module-level binding and every later mention of its name.
fn definition_uses(code: &[u8], source: &str, declarations: &[Declaration]) -> Vec<DataFlowEdge> {
    let mut edges = Vec::new();
    let text = String::from_utf8_lossy(code).into_owned();
    for declaration in declarations
        .iter()
        .filter(|declaration| declaration.kind == SymbolKind::Constant)
    {
        let needle = &declaration.name;
        let mut cursor = usize::try_from(declaration.span.end()).unwrap_or(0);
        while let Some(at) = text.get(cursor..).and_then(|tail| {
            tail.to_ascii_lowercase()
                .find(needle)
                .map(|position| position + cursor)
        }) {
            let before = at.checked_sub(1).map(|index| code[index]);
            let after = code.get(at + needle.len()).copied();
            let bounded = before.is_none_or(|byte| !is_identifier_byte(byte))
                && after.is_none_or(|byte| !is_identifier_byte(byte));
            if bounded {
                edges.push(DataFlowEdge {
                    definition: declaration.fingerprint.clone(),
                    use_span: span_of(source, at, at + needle.len()),
                });
            }
            cursor = at + needle.len();
        }
    }
    edges
}

/// Reads Python: indentation decides a definition's extent.
fn python_source(path: &str, source: &str) -> FileFacts {
    let code = blank_comments(source, "#", false);
    let text = String::from_utf8_lossy(&code).into_owned();
    let mut facts = FileFacts::default();
    let lines: Vec<(usize, &str)> = {
        let mut collected = Vec::new();
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            collected.push((offset, line));
            offset += line.len();
        }
        collected
    };
    for (position, (offset, line)) in lines.iter().enumerate() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();
        for (keyword, kind) in [("def ", SymbolKind::Function), ("class ", SymbolKind::Type)] {
            let Some(rest) = trimmed.strip_prefix(keyword) else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let mut end = offset + line.len();
            for (later_offset, later) in lines.iter().skip(position + 1) {
                let later_indent = later.len() - later.trim_start().len();
                if !later.trim().is_empty() && later_indent <= indent {
                    break;
                }
                end = later_offset + later.len();
            }
            let is_test = name.starts_with("test_");
            let symbol_kind = if is_test && kind == SymbolKind::Function {
                SymbolKind::TestFunction
            } else {
                kind
            };
            facts.declarations.push(Declaration {
                fingerprint: SymbolFingerprint::of(path, symbol_kind, &name),
                kind: symbol_kind,
                span: span_of(source, *offset + indent, end),
                is_root: is_test || indent == 0 && kind == SymbolKind::Type,
                name: name.to_ascii_lowercase(),
            });
        }
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            let rest = trimmed
                .strip_prefix("import ")
                .or_else(|| trimmed.strip_prefix("from "))
                .unwrap_or("");
            let head = rest
                .split([' ', '.', ',', '\n', '\r'])
                .find(|segment| !segment.is_empty())
                .unwrap_or("");
            if !head.is_empty() {
                facts.imports.push(TokenSite {
                    token: head.to_ascii_lowercase(),
                    span: span_of(source, *offset + indent, offset + line.len()),
                });
            }
        }
    }
    let bytes = code.clone();
    let mut index = 0;
    while index < bytes.len() {
        let Some((word, after)) = identifier_at(&bytes, index) else {
            index += 1;
            continue;
        };
        if index > 0 && is_identifier_byte(bytes[index - 1]) {
            index = after;
            continue;
        }
        let open = skip_blanks(&bytes, after);
        if bytes.get(open) == Some(&b'(') && !NOT_A_CALL.contains(&word.as_str()) {
            let mut start = index;
            while start > 0
                && (is_identifier_byte(bytes[start - 1]) || bytes[start - 1] == b'.')
                && !(bytes[start - 1] == b'.' && start >= 2 && bytes[start - 2] == b'.')
            {
                start -= 1;
            }
            let callee = String::from_utf8_lossy(&bytes[start..after])
                .to_ascii_lowercase()
                .trim_matches('.')
                .to_owned();
            facts.calls.push(CallSite {
                leaf: word.to_ascii_lowercase(),
                callee,
                span: span_of(source, start, open + 1),
            });
        }
        index = after;
    }
    facts
}

/// Reads SQL: schema objects and nothing else.
fn sql_script(path: &str, source: &str) -> FileFacts {
    let code = blank_comments(source, "--", true);
    let text = String::from_utf8_lossy(&code).into_owned();
    let lowered = text.to_ascii_lowercase();
    let mut facts = FileFacts::default();
    for keyword in [
        "create table",
        "create view",
        "create index",
        "create trigger",
    ] {
        let mut cursor = 0;
        while let Some(at) = lowered[cursor..].find(keyword).map(|at| at + cursor) {
            let bytes = text.as_bytes();
            let mut name_at = skip_blanks(bytes, at + keyword.len());
            for prefix in ["if not exists "] {
                if lowered[name_at..].starts_with(prefix) {
                    name_at = skip_blanks(bytes, name_at + prefix.len());
                }
            }
            if let Some((name, end)) = identifier_at(bytes, name_at) {
                facts.schema_objects.push(Declaration {
                    fingerprint: SymbolFingerprint::of(path, SymbolKind::SchemaObject, &name),
                    kind: SymbolKind::SchemaObject,
                    span: span_of(source, at, end),
                    name: name.to_ascii_lowercase(),
                    is_root: true,
                });
            }
            cursor = at + keyword.len();
        }
    }
    facts
}

/// Reads a TOML document into dotted key paths, and optionally dependencies.
fn toml_document(source: &str, manifest: bool) -> FileFacts {
    let code = blank_comments(source, "#", false);
    let text = String::from_utf8_lossy(&code).into_owned();
    let mut facts = FileFacts::default();
    let mut section = String::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        let lead = line.len() - line.trim_start().len();
        if let Some(header) = trimmed.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            section = header
                .trim_matches('[')
                .trim_matches(']')
                .to_ascii_lowercase();
            if manifest
                && let Some(name) = section.rsplit('.').next()
                && is_dependency_section(&section, 1)
            {
                facts.dependencies.push(DependencySite {
                    token: name.to_owned(),
                    span: span_of(source, offset + lead, offset + line.len()),
                    development_only: section.contains("dev-") || section.contains("build-"),
                });
            }
        } else if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim().trim_matches('"').to_ascii_lowercase();
            if !key.is_empty() {
                let span = span_of(source, offset + lead, offset + line.len());
                // A dependency entry is manifest presence and not
                // configuration. Emitting it as both would make section 17.3's
                // first row corroborate itself: the same line would be counted
                // once as `manifest에 dependency만 있음` and once as the
                // `config 존재` half of the third row, and a manifest-only
                // fixture would classify as a use of what it merely installs.
                if manifest && is_dependency_section(&section, 0) {
                    facts.dependencies.push(DependencySite {
                        token: key,
                        span,
                        development_only: section.contains("dev-") || section.contains("build-"),
                    });
                } else {
                    let path = if section.is_empty() {
                        key.clone()
                    } else {
                        format!("{section}.{key}")
                    };
                    facts.config_tokens.push(TokenSite { token: path, span });
                    for token in scalar_tokens(value) {
                        facts.config_tokens.push(TokenSite { token, span });
                    }
                }
            }
        }
        offset += line.len();
    }
    facts
}

/// Whether a TOML section names dependencies, `depth` segments above the key.
fn is_dependency_section(section: &str, depth: usize) -> bool {
    let segments: Vec<&str> = section.split('.').collect();
    let Some(at) = segments.len().checked_sub(depth + 1) else {
        return false;
    };
    matches!(
        segments[at],
        "dependencies" | "dev-dependencies" | "build-dependencies"
    )
}

/// Reads a JSON document into dotted key paths, and optionally dependencies.
fn json_document(source: &str, manifest: bool) -> FileFacts {
    let bytes = source.as_bytes();
    let mut facts = FileFacts::default();
    let mut stack: Vec<String> = Vec::new();
    let mut index = 0;
    let mut pending: Option<String> = None;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'[' => {
                stack.push(pending.take().unwrap_or_default());
                index += 1;
            }
            b'}' | b']' => {
                stack.pop();
                index += 1;
            }
            b'"' => {
                let start = index;
                let mut cursor = index + 1;
                while cursor < bytes.len() && bytes[cursor] != b'"' {
                    cursor += if bytes[cursor] == b'\\' { 2 } else { 1 };
                }
                let literal = source
                    .get(start + 1..cursor.min(bytes.len()))
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let after = skip_blanks(bytes, cursor + 1);
                let span = span_of(source, start, cursor + 1);
                if bytes.get(after) == Some(&b':') {
                    let owner: Vec<&str> = stack
                        .iter()
                        .filter(|segment| !segment.is_empty())
                        .map(String::as_str)
                        .collect();
                    let path = if owner.is_empty() {
                        literal.clone()
                    } else {
                        format!("{}.{literal}", owner.join("."))
                    };
                    // See the TOML reader: a dependency entry is manifest
                    // presence and never configuration, so it is pushed to one
                    // list and not to both.
                    if manifest
                        && matches!(
                            owner.last().copied(),
                            Some("dependencies" | "devdependencies" | "peerdependencies")
                        )
                    {
                        facts.dependencies.push(DependencySite {
                            token: literal.clone(),
                            span,
                            development_only: owner.last().copied() == Some("devdependencies"),
                        });
                    } else {
                        facts.config_tokens.push(TokenSite { token: path, span });
                    }
                    pending = Some(literal);
                } else {
                    for token in scalar_tokens(&literal) {
                        facts.config_tokens.push(TokenSite { token, span });
                    }
                }
                index = cursor + 1;
            }
            _ => index += 1,
        }
    }
    facts
}

/// Reads a YAML document into dotted key paths by indentation.
fn yaml_document(source: &str, manifest: bool, iac: bool) -> FileFacts {
    let code = blank_comments(source, "#", false);
    let text = String::from_utf8_lossy(&code).into_owned();
    let mut facts = FileFacts::default();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start().trim_start_matches("- ").trim_end();
        let indent = line.len() - line.trim_start().len();
        if trimmed.is_empty() {
            offset += line.len();
            continue;
        }
        while stack.last().is_some_and(|(depth, _)| *depth >= indent) {
            stack.pop();
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().trim_matches('"').to_ascii_lowercase();
            let owner: Vec<&str> = stack.iter().map(|(_, name)| name.as_str()).collect();
            let path = if owner.is_empty() {
                key.clone()
            } else {
                format!("{}.{key}", owner.join("."))
            };
            let span = span_of(source, offset + indent, offset + line.len());
            facts.config_tokens.push(TokenSite {
                token: path.clone(),
                span,
            });
            for token in scalar_tokens(value) {
                facts.config_tokens.push(TokenSite {
                    token: token.clone(),
                    span,
                });
                if iac {
                    facts.iac_tokens.push(TokenSite { token, span });
                }
            }
            if iac {
                facts.iac_tokens.push(TokenSite { token: path, span });
            }
            if manifest && owner.last().copied() == Some("dependencies") {
                facts.dependencies.push(DependencySite {
                    token: key.clone(),
                    span,
                    development_only: false,
                });
            }
            stack.push((indent, key));
        }
        offset += line.len();
    }
    facts
}

/// Reads a config document, dispatching on its extension.
fn config_document(path: &str, source: &str) -> FileFacts {
    let extension = path.rsplit_once('.').map_or("", |(_, tail)| tail);
    match extension {
        "toml" => toml_document(source, false),
        "json" => json_document(source, false),
        _ => yaml_document(source, false, false),
    }
}

/// Reads a lock file for the package names it pins.
fn lock_document(source: &str) -> FileFacts {
    let mut facts = toml_document(source, false);
    let bare = json_document(source, false);
    facts.config_tokens.extend(bare.config_tokens);
    let mut dependencies = Vec::new();
    for site in &facts.config_tokens {
        if let Some(name) = site.token.strip_prefix("name.") {
            dependencies.push(DependencySite {
                token: name.to_owned(),
                span: site.span,
                development_only: false,
            });
        }
    }
    for line in source.lines() {
        if let Some(rest) = line.trim().strip_prefix("name = ") {
            let name = rest.trim().trim_matches('"').to_ascii_lowercase();
            if !name.is_empty() {
                dependencies.push(DependencySite {
                    token: name,
                    span: SourceSpan::new(0, 0, 1, 1),
                    development_only: false,
                });
            }
        }
    }
    facts.dependencies = dependencies;
    facts
}

/// Reads a container file's directives.
fn container_file(source: &str) -> FileFacts {
    let code = blank_comments(source, "#", false);
    let text = String::from_utf8_lossy(&code).into_owned();
    let mut facts = FileFacts::default();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some((directive, rest)) = trimmed.split_once(' ') {
            let directive = directive.to_ascii_uppercase();
            if [
                "FROM",
                "RUN",
                "ENV",
                "EXPOSE",
                "CMD",
                "ENTRYPOINT",
                "COPY",
                "ARG",
                "WORKDIR",
            ]
            .contains(&directive.as_str())
            {
                let span = span_of(source, offset, offset + line.len());
                facts.iac_tokens.push(TokenSite {
                    token: directive.to_ascii_lowercase(),
                    span,
                });
                for token in scalar_tokens(rest) {
                    facts.iac_tokens.push(TokenSite {
                        token: token.clone(),
                        span,
                    });
                    facts.config_tokens.push(TokenSite { token, span });
                }
            }
        }
        offset += line.len();
    }
    facts
}

/// The identifier-like tokens a scalar value holds, lowercased.
///
/// A value such as `redis://cache:6379` yields `redis`, `cache` and `6379`, so
/// a caller's needle matches the scheme without this crate holding the string.
fn scalar_tokens(value: &str) -> Vec<String> {
    let cleaned = value.trim().trim_matches(['"', '\'', ',']);
    cleaned
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}
