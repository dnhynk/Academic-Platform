//! Source scans for the `P2-G5` trust boundary.
//!
//! What this crate claims is a shape of the source, not a behaviour: the label
//! is a type because the type implements nothing that would strip it, and the
//! instruction channel is trusted because its constructor takes a compile-time
//! constant. Neither claim has a run-time observation that would notice the day
//! it stops being true, which is what
//! `docs/contracts/policy-source-scans.md` says a policy source scan is for.
//!
//! The page names three shapes that make a scan empty and two more about what a
//! scan concludes. This file is written against all five.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends into every
//! subdirectory of the whole package rather than into `src` by name -- `S-12`
//! on that page is the row about a walk that reads `<crate>/src` and stops
//! seeing a `[[bin]]` whose `path` is outside it. There is a floor under it, a
//! tripwire requiring every `mod name;` and every `#[path = "…"]` target in the
//! crate to be a file the walk read, and a rule that this crate's product
//! source is under `src` and nowhere else.
//!
//! **The checks are not token lists.** The two that could have been are not:
//! the trait-implementation rule compares the crate's *whole set* of `impl`
//! blocks naming `Untrusted` against a pinned list, so an implementation nobody
//! predicted fails as an extra key; and the exposure rule compares the whole
//! inventory of call sites of the one crate-private accessor, with a written
//! reason for each, rather than searching for spellings.
//!
//! **The pins fix their callers too.** `T141` found a pinned signature check
//! skipped by a condition wrapped around it, so [`WHOLE_ADJUDICATE`] is
//! accompanied by [`WHOLE_ADMIT`] and an occurrence count, and
//! [`WHOLE_ENVELOPE`] by [`WHOLE_ENVELOPE_FOR`] and another.
//!
//! **Every inventory counts a name, not a spelling.** The `Untrusted` and
//! `AcceptedResponse` inventories count the type name, which a value of that
//! type cannot be built or held without naming. The exposure inventory counts
//! the accessor's name for the same reason: `T146` reached a fourth site by
//! writing `Untrusted::expose(d)` instead of `d.expose()`, which is the same
//! call and passed a count of the second spelling.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

/// The crate root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The workspace root.
fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Every `.rs` file anywhere under this crate's package directory.
///
/// The package directory rather than `src`, because `S-12` in
/// `docs/contracts/policy-source-scans.md` is exactly the walk that reads
/// `<crate>/src` and nothing else: `P2-G4` put a `[[bin]]` with an explicit
/// `path` outside `src`, and two scans stopped seeing that crate's product
/// code. Injection `G-I13` is the observation that this walk does not: a
/// `#[path = "../extra/leak.rs"]` module with an exposure site in it is
/// refused, and it passed an earlier version of this file that walked `src`.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships, which is every one outside `tests`.
///
/// `benches` was excluded beside `tests` until `T149` observed that a bench
/// target has no feature gate and is compiled by
/// `cargo clippy --workspace --all-targets`, which is the test `T146` applied
/// to `examples/`. No `benches` tree exists in this repository today.
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

/// Extracts one item's text, comment lines dropped and whitespace collapsed.
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
/// `occurrences` above counts a spelling, which is right for a fixed phrase and
/// wrong for a function. `T146` wrote a fourth exposure site as
/// `Untrusted::expose(document)` -- UFCS through the type path, the same call,
/// containing no `.expose()` -- and the inventory that counted the spelling saw
/// nothing, while the whole workspace suite, every JS scan, and
/// `clippy -D warnings` all passed. Writing the identical function with the
/// receiver spelling failed immediately, so what separated pass from fail was
/// the spelling and not the behaviour.
///
/// A name has no such freedom: the call has to spell it, whether it is written
/// as a method, through the type path, or taken as a function value. The
/// boundary test is the one `names_unsafe` in `crates/worker/tests/capability.rs`
/// and `crates/record/tests/record_scans.rs` already use.
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
/// Each count below subtracts a declaration from a use count, and reading the
/// declaration as a *spelling* is what `T149` walked through: `occurrences(code,
/// "fn expose")` counts `pub fn expose_rendered(`, which `uses_of` does not
/// count as a use of `expose` -- so one function whose name merely starts with
/// the guarded one cancels its own call. With that injection applied, an
/// integration test outside this crate put an ingested payload verbatim into a
/// `[SYSTEM]` segment while this file, the workspace suite and both JS scans
/// passed. The same hole was open on `quote` and on `adjudicate`.
///
/// What follows the name has to open a parameter list or a generic list and
/// nothing else, so `fn expose_rendered(` is not `expose` and
/// `fn quote<'a>(` still is.
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
///
/// `saturating_sub` was here, and it folded an underflow to zero silently: a
/// count that read more declarations than uses would have reported "no call
/// sites" rather than reporting that the two halves disagree.
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
/// A re-export names a function and calls nothing. It cannot be dropped by a
/// line filter alone: rustfmt wraps a long list over several lines and the name
/// lands on whichever one it fits.
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

/// Every `pub` function signature in `code`, from `pub` to the `{` or `;` that
/// ends it, whitespace-collapsed.
///
/// The lines are joined because a signature long enough to matter is the one
/// rustfmt wraps. The shape is `public_surface` in
/// `crates/keystore-platform/tests/facade.rs`, which reads that leaf's public
/// surface for the same reason: what a caller outside the crate can reach is a
/// property of the signatures, not of the bodies.
fn public_signatures(code: &str) -> Vec<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if ![
            "pub fn ",
            "pub const fn ",
            "pub async fn ",
            "pub unsafe fn ",
        ]
        .iter()
        .any(|start| trimmed.starts_with(start))
        {
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
        found.push(signature.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    found
}

/// Splits a signature into its parameter list and its return type.
///
/// The split is the `->` after the parameter list closes, not the first one in
/// the text: a parameter that is itself a function type (`now: &dyn Fn() -> u64`)
/// spells one inside the parentheses. A signature with no return type yields an
/// empty second half.
fn parameters_and_return(signature: &str) -> Option<(&str, &str)> {
    let open = signature.find('(')?;
    let mut depth = 0_usize;
    // Sliced from `open` rather than skipping `open` items: `char_indices`
    // yields byte offsets, and `skip` counts characters, so the two agree only
    // while everything before the parenthesis is ASCII.
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

/// Whether a return type hands back the bytes rather than a label over them.
///
/// The names are matched as whole identifiers so a lifetime cannot hide one:
/// `&'static str` does not contain the substring `&str`, and `u8` covers
/// `&[u8]`, `Vec<u8>`, `Box<[u8]>`, and `Cow<'_, [u8]>` alike.
fn returns_raw_text(returns: &str) -> Option<&'static str> {
    ["str", "String", "u8"]
        .into_iter()
        .find(|name| uses_of(returns, name) > 0)
}

/// One file of this crate, as code with comments and literals removed.
fn code_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(path)?))
}

/// The relative path of `path` under the workspace, with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The whole of `impl<T> Untrusted<T>`. Every accessor the wrapper has, in one
/// constant: an added method that returns the value edits this.
const WHOLE_UNTRUSTED: &str = "impl<T> Untrusted<T> { pub(crate) fn seal(value: T, provenance: Provenance, bytes: &[u8]) -> Self { Self { value, provenance, digest: digest_of(bytes), byte_len: bytes.len(), sealed: PhantomData, } } pub(crate) const fn expose(&self) -> &T { &self.value } #[must_use] pub const fn provenance(&self) -> &Provenance { &self.provenance } #[must_use] pub fn digest(&self) -> &str { &self.digest } #[must_use] pub const fn byte_len(&self) -> usize { self.byte_len } }";

/// The whole of `impl SystemDirective`. What the instruction channel accepts.
const WHOLE_SYSTEM_DIRECTIVE: &str = "impl SystemDirective { #[must_use] pub const fn new(text: &'static str) -> Self { Self(text) } #[must_use] pub const fn as_str(self) -> &'static str { self.0 } }";

/// The whole of `impl ToolDirective`. The same claim, the other channel.
const WHOLE_TOOL_DIRECTIVE: &str = "impl ToolDirective { #[must_use] pub const fn new(text: &'static str) -> Self { Self(text) } #[must_use] pub const fn as_str(self) -> &'static str { self.0 } }";

/// The escaper. What makes a data record one line of ASCII.
const WHOLE_ESCAPE: &str = "fn escape(text: &str) -> String { let mut escaped = String::with_capacity(text.len()); for character in text.chars() { match character { '\"' => escaped.push_str(\"\\\\\\\"\"), '\\\\' => escaped.push_str(\"\\\\\\\\\"), ' '..='~' => escaped.push(character), other => { let mut units = [0_u16; 2]; for unit in other.encode_utf16(&mut units) { escaped.push_str(\"\\\\u\"); for shift in [12_u32, 8, 4, 0] { const DIGITS: &[u8; 16] = b\"0123456789abcdef\"; let nibble = usize::from((*unit >> shift) & 0x000f); escaped.push(char::from(DIGITS[nibble])); } } } } } escaped }";

/// The whole of `impl PromptEnvelope`: what goes into each channel, and how the
/// rendered bytes and the untrusted span map are produced.
const WHOLE_ENVELOPE: &str = "impl PromptEnvelope { #[must_use] pub const fn new() -> Self { Self { system: Vec::new(), tools: Vec::new(), data: Vec::new(), } } pub fn push_system(&mut self, directive: SystemDirective) { self.system.push(directive); } pub fn push_tool(&mut self, directive: ToolDirective) { self.tools.push(directive); } pub fn quote(&mut self, document: &Untrusted<IngestedDocument>) { let inner = document.expose(); self.data.push(QuotedDocument { provenance: document.provenance().clone(), digest: document.digest().to_owned(), byte_len: inner.byte_len(), escaped: escape(inner.text()), }); } #[must_use] pub fn quoted_len(&self) -> usize { self.data.len() } #[must_use] pub fn render(&self) -> RenderedPrompt { let mut text = String::new(); let mut segments = Vec::new(); let mut untrusted = Vec::new(); let push = |text: &mut String, segments: &mut Vec<Segment>, kind, line: &str| { let start = text.len(); text.push_str(line); text.push('\\n'); segments.push(Segment { kind, start, end: text.len(), provenance: None, }); }; push( &mut text, &mut segments, ChannelKind::Structure, PROMPT_FORMAT, ); push(&mut text, &mut segments, ChannelKind::Structure, \"[SYSTEM]\"); for directive in &self.system { push( &mut text, &mut segments, ChannelKind::System, directive.as_str(), ); } push(&mut text, &mut segments, ChannelKind::Structure, \"[TOOLS]\"); for directive in &self.tools { push( &mut text, &mut segments, ChannelKind::ToolInstruction, directive.as_str(), ); } push(&mut text, &mut segments, ChannelKind::Structure, \"[DATA]\"); let instruction_end = text.len(); for document in &self.data { let start = text.len(); text.push_str(\"{\\\"id\\\":\\\"\"); text.push_str(document.provenance.source_id().as_str()); text.push_str(\"\\\",\\\"kind\\\":\\\"\"); text.push_str(document.provenance.kind().as_str()); text.push_str(\"\\\",\\\"seq\\\":\"); text.push_str(&document.provenance.ingest_seq().to_string()); text.push_str(\",\\\"sha256\\\":\\\"\"); text.push_str(&document.digest); text.push_str(\"\\\",\\\"bytes\\\":\"); text.push_str(&document.byte_len.to_string()); text.push_str(\",\\\"content\\\":\\\"\"); let content_start = text.len(); text.push_str(&document.escaped); let content_end = text.len(); text.push_str(\"\\\"}\"); text.push('\\n'); segments.push(Segment { kind: ChannelKind::Data, start, end: text.len(), provenance: Some(document.provenance.clone()), }); untrusted.push(UntrustedSpan { provenance: document.provenance.clone(), start: content_start, end: content_end, }); } RenderedPrompt { text, segments, untrusted, instruction_end, } } }";

/// Provenance resolution, exposure site two.
const WHOLE_RESOLVE_SPAN: &str = "fn resolve_span(index: &SourceIndex, span: &ParsedSpan) -> Result<ResolvedSpan, SpanError> { let Some(document) = index.get(&span.source_id) else { return Err(SpanError::UnknownSource); }; if span.start >= span.end { return Err(SpanError::EmptySpan); } let text = document.expose().text(); if span.end > text.len() { return Err(SpanError::OutOfRange); } if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) { return Err(SpanError::NotACharBoundary); } let Some(slice) = text.get(span.start..span.end) else { return Err(SpanError::OutOfRange); }; let expected = digest_of(slice.as_bytes()); if expected.get(..SPAN_DIGEST_HEX_LEN) != Some(span.digest.as_str()) { return Err(SpanError::DigestMismatch); } Ok(ResolvedSpan { source_id: span.source_id.clone(), kind: document.provenance().kind(), start: span.start, end: span.end, digest: span.digest.clone(), }) }";

/// The adjudicator, exposure site three. Its parameter list is the claim that
/// the pipeline holds no capability, no broker, no transport, and no path.
const WHOLE_ADJUDICATE: &str = "pub fn adjudicate( index: &SourceIndex, output: &Untrusted<ModelOutput>, ) -> Result<Proposal, QuarantinedOutput> { let quarantine = |reason: QuarantineReason| QuarantinedOutput { output_id: output.provenance().source_id().clone(), digest: output.digest().to_owned(), byte_len: output.byte_len(), reason, }; let parsed = match parse_schema(output.expose().source_bytes.as_str()) { Ok(parsed) => parsed, Err(error) => return Err(quarantine(QuarantineReason::Schema(error))), }; let mut support = Vec::with_capacity(parsed.support.len()); for span in &parsed.support { match resolve_span(index, span) { Ok(resolved) => support.push(resolved), Err(error) => return Err(quarantine(QuarantineReason::Provenance(error))), } } let summary_bytes = parsed.summary.clone().into_bytes(); Ok(Proposal { kind: parsed.kind, summary: Untrusted::seal(parsed.summary, output.provenance().clone(), &summary_bytes), support, }) }";

/// The one caller of `PromptEnvelope::quote` in this crate.
const WHOLE_ENVELOPE_FOR: &str = "pub fn envelope_for(index: &SourceIndex) -> PromptEnvelope { let mut envelope = PromptEnvelope::new(); for directive in BOUNDARY_SYSTEM_DIRECTIVES { envelope.push_system(directive); } for directive in BOUNDARY_TOOL_DIRECTIVES { envelope.push_tool(directive); } for document in index.documents() { envelope.quote(document); } envelope }";

/// The one caller of `adjudicate` in this crate. A pin on a decision says
/// nothing about whether the decision runs; `T141` found exactly that hole.
const WHOLE_ADMIT: &str = "pub fn admit(queue: &mut ReviewQueue, index: &SourceIndex, output: &Untrusted<ModelOutput>) { queue.admit(adjudicate(index, output)); }";

/// Every `impl` block in this crate whose header names `Untrusted`.
///
/// Compared as a whole set, not searched. The two entries are the inherent
/// block and the hand-written `Debug`; an implementation of anything else --
/// `Deref`, `AsRef`, `Borrow`, `Display`, `From`, or a trait nobody has thought
/// of -- appears here as an extra key and fails.
///
/// An `impl` in another crate is refused by the orphan rule rather than by
/// this: both the trait and the type would be foreign there.
const UNTRUSTED_IMPL_BLOCKS: [&str; 2] = [
    "impl<T> Untrusted<T> {",
    "impl<T> fmt::Debug for Untrusted<T> {",
];

/// Every call site of the crate-private accessor, and why each is allowed.
///
/// The whole inventory is compared against what the walk finds, so a fourth
/// site fails as an extra key and a removed one fails as a missing key.
const EXPOSURE_SITES: [(&str, &str, &str); 3] = [
    (
        "crates/untrusted-content/src/channel.rs",
        "PromptEnvelope::quote",
        "The quoted data channel is the one place ingested bytes may appear, and \
         quoting has to read the bytes it escapes. What leaves is escaped, one \
         line, pure ASCII, and recorded as an untrusted span.",
    ),
    (
        "crates/untrusted-content/src/proposal.rs",
        "resolve_span",
        "Provenance resolution has to compare a cited range against the source \
         bytes. What leaves is a ResolvedSpan: offsets and a digest, no text.",
    ),
    (
        "crates/untrusted-content/src/proposal.rs",
        "adjudicate",
        "Schema validation has to read the output it validates. What leaves is a \
         closed ProposalKind, resolved spans, and a summary sealed again.",
    ),
];

/// Every file in the workspace that names `academic-egress-boundary`'s
/// `AcceptedResponse`.
///
/// This is the one-step-out check. `AcceptedResponse::bytes` is public and
/// returns `&[u8]` with no label on it, so the value one layer outside this
/// crate's boundary is exactly the shape this crate exists to stop being
/// spendable. What keeps that scoped is that the set of files that can hold one
/// is small and reviewed; a fourth file naming the type fails here.
const ACCEPTED_RESPONSE_FILES: [&str; 4] = [
    "crates/egress-boundary/src/lib.rs",
    "crates/egress-boundary/src/response.rs",
    "crates/untrusted-content/src/ingest.rs",
    "crates/untrusted-content/src/proposal.rs",
];

#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 8,
        "the walk found only {} files under the package",
        sources.len()
    );

    // Product source lives under `src` and nowhere else. That is what makes the
    // three per-file rules below -- the exposure inventory, the directive
    // construction counts, and the forbidden broker names -- cover everything
    // that ships, and it is the condition `S-12` says a crate has to keep if it
    // does not want to widen every scan that reads it.
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
    // A module is either `<name>.rs` or `<name>/mod.rs`, so both spellings are
    // collected: a tripwire that only knew the first would fire on every
    // directory module and be turned off rather than fixed.
    let mut read: BTreeSet<String> = BTreeSet::new();
    for path in &sources {
        if let Some(stem) = path.file_stem() {
            let stem = stem.to_string_lossy().into_owned();
            if stem == "mod" {
                if let Some(parent) = path.parent().and_then(Path::file_name) {
                    read.insert(parent.to_string_lossy().into_owned());
                }
            } else {
                read.insert(stem);
            }
        }
    }

    // The tripwire. Every `mod name;` and every `#[path = "…"]` in the crate
    // has to name a file the walk read. It fails the day the walk is narrowed,
    // and the day a module is added somewhere the walk does not descend into.
    let mut declared = 0_usize;
    for path in &sources {
        let source = fs::read_to_string(path)?;
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
            if let Some(rest) = trimmed.strip_prefix("#[path = \"") {
                let target = rest.split('"').next().unwrap_or_default();
                let resolved = path
                    .parent()
                    .map_or_else(|| PathBuf::from(target), |parent| parent.join(target));
                assert!(
                    sources.iter().any(|read_path| read_path == &resolved),
                    "{} includes {target}, which the walk never read",
                    relative(path)
                );
            }
        }
    }
    assert!(declared >= 6, "the crate declares only {declared} modules");
    Ok(())
}

#[test]
fn untrusted_has_no_unwrapping_trait_impl() -> TestResult {
    let mut found: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        let code = code_of(&path)?;
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl") && trimmed.contains("Untrusted<") {
                found.push(trimmed.to_owned());
            }
        }
    }
    found.sort();
    let mut expected: Vec<String> = UNTRUSTED_IMPL_BLOCKS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        found, expected,
        "the set of impl blocks naming Untrusted changed; an unwrapping trait is \
         how the label stops propagating"
    );

    // The pin. It fixes what the inherent block contains, which the set above
    // does not: an inherent `pub fn into_inner` names no trait.
    let label = fs::read_to_string(crate_root().join("src/label.rs"))?;
    assert_eq!(
        declared_item(&label, "impl<T> Untrusted<T> {")?,
        WHOLE_UNTRUSTED,
        "impl<T> Untrusted<T> changed"
    );

    // The trait names that would strip the label, none of which may appear in
    // an impl header here. This list is the weaker half and is written as such:
    // the whole-set comparison above is what refuses one nobody predicted.
    for forbidden in [
        "Deref", "DerefMut", "AsRef", "AsMut", "Borrow", "Display", "ToString", "From", "Into",
    ] {
        assert!(
            !found.iter().any(|header| header.contains(forbidden)),
            "an impl of {forbidden} for Untrusted exists"
        );
    }
    Ok(())
}

#[test]
fn every_exposure_site_is_named_and_justified() -> TestResult {
    let mut sites: Vec<(String, usize)> = Vec::new();
    let mut total = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        let count = calls_of(&code, "expose");
        if count > 0 {
            sites.push((relative(&path), count));
            total += count;
        }
    }
    sites.sort();

    let mut expected: Vec<(String, usize)> = Vec::new();
    for (file, _, _) in EXPOSURE_SITES {
        match expected.iter_mut().find(|(name, _)| name == file) {
            Some(entry) => entry.1 += 1,
            None => expected.push((file.to_owned(), 1)),
        }
    }
    expected.sort();
    assert_eq!(
        sites, expected,
        "the exposure inventory and the source disagree"
    );
    assert_eq!(total, EXPOSURE_SITES.len(), "an exposure site is unnamed");

    // Each site carries a reason, and each reason says something.
    for (file, function, reason) in EXPOSURE_SITES {
        assert!(
            reason.len() >= 80,
            "{file}:{function} has no written reason"
        );
        let source = fs::read_to_string(workspace_root().join(file))?;
        assert!(
            source.contains(function.rsplit("::").next().unwrap_or(function)),
            "{file} no longer declares {function}"
        );
    }

    // The accessor is crate-private, so no caller outside this crate can spell
    // one at all. A `pub` here would make the inventory above meaningless.
    let label = fs::read_to_string(crate_root().join("src/label.rs"))?;
    assert!(
        label.contains("pub(crate) const fn expose(&self) -> &T {"),
        "the accessor is no longer crate-private"
    );
    assert_eq!(
        occurrences(&strip_non_code(&label), "fn expose"),
        1,
        "there is more than one accessor"
    );

    // Crate-private stops a caller from calling it. It does not stop this crate
    // from calling it on a caller's behalf. `T146`'s fourth site was one `pub fn`
    // taking an `&Untrusted<IngestedDocument>` and returning `&str`, and with it
    // an outside caller put ingested bytes verbatim into a System segment --
    // unescaped, on their own line, recorded in no untrusted span. The inventory
    // above now counts it, and this refuses the shape whatever it is named.
    let mut surface = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            surface = surface.saturating_add(1);
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if !parameters.contains("Untrusted<") {
                continue;
            }
            assert_eq!(
                returns_raw_text(returns),
                None,
                "{}: a public signature takes an Untrusted and returns the bytes inside it: \
                 {signature}",
                relative(&path)
            );
        }
    }
    assert!(
        surface >= 60,
        "the public-signature scan found only {surface} signatures, so it proved nothing"
    );
    Ok(())
}

#[test]
fn the_instruction_channel_takes_only_static_text() -> TestResult {
    let channel = fs::read_to_string(crate_root().join("src/channel.rs"))?;
    assert_eq!(
        declared_item(&channel, "impl SystemDirective {")?,
        WHOLE_SYSTEM_DIRECTIVE,
        "impl SystemDirective changed"
    );
    assert_eq!(
        declared_item(&channel, "impl ToolDirective {")?,
        WHOLE_TOOL_DIRECTIVE,
        "impl ToolDirective changed"
    );
    assert_eq!(
        declared_item(&channel, "fn escape(text: &str) -> String {")?,
        WHOLE_ESCAPE,
        "the escaper changed"
    );
    assert_eq!(
        declared_item(&channel, "impl PromptEnvelope {")?,
        WHOLE_ENVELOPE,
        "impl PromptEnvelope changed"
    );
    let lib = fs::read_to_string(crate_root().join("src/lib.rs"))?;
    assert_eq!(
        declared_item(&lib, "pub fn envelope_for(")?,
        WHOLE_ENVELOPE_FOR,
        "the one caller of quote changed"
    );

    // The tuple structs are constructed nowhere but their own constructors, so
    // the `&'static str` bound cannot be walked around by writing the struct
    // literal directly.
    let mut system_constructions = 0_usize;
    let mut tool_constructions = 0_usize;
    let mut quote_calls = 0_usize;
    let mut leaks = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        system_constructions += occurrences(&code, "SystemDirective(");
        tool_constructions += occurrences(&code, "ToolDirective(");
        quote_calls += calls_of(&code, "quote");
        leaks += occurrences(&code, "leak");
    }
    assert_eq!(
        system_constructions, 1,
        "SystemDirective is constructed somewhere other than its constructor"
    );
    assert_eq!(
        tool_constructions, 1,
        "ToolDirective is constructed somewhere other than its constructor"
    );
    assert_eq!(
        quote_calls, 1,
        "quote has more than one caller in this crate"
    );

    // The remaining route to a `&'static str` at run time is leaking an
    // allocation. Nothing in this crate does, and the wrapper hands out no
    // owned value to leak in the first place. This is a substring count over
    // code with comments and literals removed, so it errs toward reporting: an
    // identifier that merely contains the four letters fails here and has to be
    // renamed or this rule widened. That is the safe direction for a
    // supplementary check whose primary is the accessor being crate-private.
    assert_eq!(
        leaks, 0,
        "this crate spells `leak` in code; a leaked allocation is the one          remaining way an owned value becomes a `&'static str`"
    );
    Ok(())
}

#[test]
fn the_adjudicator_receives_no_capability() -> TestResult {
    let proposal = fs::read_to_string(crate_root().join("src/proposal.rs"))?;
    assert_eq!(
        declared_item(&proposal, "pub fn adjudicate(")?,
        WHOLE_ADJUDICATE,
        "adjudicate changed"
    );
    assert_eq!(
        declared_item(&proposal, "fn resolve_span(")?,
        WHOLE_RESOLVE_SPAN,
        "resolve_span changed"
    );
    let lib = fs::read_to_string(crate_root().join("src/lib.rs"))?;
    assert_eq!(
        declared_item(&lib, "pub fn admit(")?,
        WHOLE_ADMIT,
        "the one caller of adjudicate changed"
    );

    // The pin fixes the text; this fixes that nothing else calls it, so a
    // second, unpinned entry point cannot appear beside `admit`.
    //
    // The name is counted, not the argument spelling. `T146` added a second
    // caller written `adjudicate(idx, out)` and this count, which read
    // `adjudicate(index, output)`, saw one caller; renaming the two locals back
    // failed it at once. Declarations are subtracted so `pub fn adjudicate(`
    // itself is not read as a call.
    let mut calls = 0_usize;
    for path in crate_product_sources()? {
        let code = without_use_items(&code_of(&path)?);
        calls += calls_of(&code, "adjudicate");
    }
    assert_eq!(
        calls, 1,
        "adjudicate has more than one caller in this crate"
    );

    // The manifest half. `academic-policy` is a dev edge, so a file under `src`
    // cannot name `PermissionBroker`, `CapabilityToken`, `RuntimeToolCall`, or
    // `ProcessCapabilityToken`: an undeclared crate is a compile error. The
    // whole edge map is pinned in `tools/phase1-scaffold-policy.test.mjs`; this
    // reads the manifest so the two halves fail together.
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml"))?;
    // Comment lines are dropped first: this file explains in prose why
    // `academic-policy` is a dev edge, and a check that read the prose would
    // report the explanation as the violation.
    let declarations: Vec<&str> = manifest
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    let section = |name: &str| -> String {
        declarations
            .iter()
            .skip_while(|line| line.trim() != name)
            .skip(1)
            .take_while(|line| !line.trim_start().starts_with('['))
            .copied()
            .collect::<Vec<_>>()
            .join(
                "
",
            )
    };
    let dependencies = section("[dependencies]");
    let dev = section("[dev-dependencies]");
    assert!(
        dependencies.contains("academic-egress-boundary"),
        "academic-egress-boundary is no longer a product edge, so the provider          response scan is no longer reused by construction"
    );
    assert!(
        !dependencies.contains("academic-policy"),
        "academic-policy became a product edge of this crate"
    );
    assert!(
        dev.contains("academic-policy"),
        "academic-policy is no longer a dev edge"
    );
    assert!(
        !declarations
            .iter()
            .any(|line| line.contains("academic-worker")),
        "this crate depends on academic-worker, which only_egress_crate_has_a_socket refuses"
    );
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for forbidden in [
            "PermissionBroker",
            "CapabilityToken",
            "RuntimeToolCall",
            "ProcessCapabilityToken",
        ] {
            assert!(
                !code.contains(forbidden),
                "{} names {forbidden}",
                relative(&path)
            );
        }
    }
    Ok(())
}

#[test]
fn only_reviewed_files_hold_an_unlabelled_provider_response() -> TestResult {
    let mut found: Vec<String> = Vec::new();
    let crates = workspace_root().join("crates");
    for entry in fs::read_dir(&crates)? {
        let package = entry?.path();
        if !package.is_dir() {
            continue;
        }
        // The whole package, not three directory names. `S-12` is the row this
        // avoids: a walk that names `src` stops reading a crate the day one of
        // its `[[bin]]` targets gets an explicit `path` outside it.
        let mut files = Vec::new();
        walk(&package, &mut files)?;
        for path in files {
            if code_of(&path)?.contains("AcceptedResponse") {
                found.push(relative(&path));
            }
        }
    }
    found.sort();
    let mut expected: Vec<String> = ACCEPTED_RESPONSE_FILES
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        found, expected,
        "a file holds an AcceptedResponse, whose bytes carry no trust label"
    );
    Ok(())
}
