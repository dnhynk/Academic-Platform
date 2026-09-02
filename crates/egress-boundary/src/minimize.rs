//! Content classification and structural minimization.
//!
//! Two questions are answered here, in this order, and both before any scan.
//!
//! 1. Is this text at all? A payload whose bytes cannot be read as UTF-8, or
//!    that carries a known container magic, is refused rather than guessed at.
//!    A scanner cannot report what it cannot read, so an unreadable payload is
//!    a denial and not a clean result.
//! 2. What is the smallest part of it the request actually needs? A whole-file
//!    request is reduced to the brace-balanced items that declare the requested
//!    symbols. What is not needed never reaches the scanner and therefore never
//!    reaches a staged payload.
//!
//! The item scanner is a brace-balancing reader with string, character, and
//! comment awareness. It is not a Rust parser: it does not resolve paths,
//! macros, or generics, and a symbol produced by a macro is not found by it.
//! What it does is exact for the shape it handles, and a symbol it cannot find
//! is a denial rather than a fallback to the whole file.

/// A half-open byte range of the source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceRange {
    start: usize,
    end: usize,
}

impl SourceRange {
    /// A half-open range over the source document.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Inclusive start offset in the source document.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Exclusive end offset in the source document.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// Byte length of the range.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range covers no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// Why a payload could not be read as scannable text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassificationError {
    /// The bytes are not valid UTF-8.
    NotUtf8,
    /// The bytes carry a known container or executable magic number.
    ContainerMagic(&'static str),
    /// The bytes hold a control character no source text uses.
    ControlByte,
}

/// Container and executable magic numbers refused before any scan.
const CONTAINER_MAGICS: &[(&str, &[u8])] = &[
    ("zip-archive", b"PK\x03\x04"),
    ("zip-archive-empty", b"PK\x05\x06"),
    ("gzip-archive", b"\x1f\x8b"),
    ("elf-executable", b"\x7fELF"),
    ("portable-executable", b"MZ"),
    ("png-image", b"\x89PNG"),
    ("pdf-document", b"%PDF"),
    ("java-class", b"\xca\xfe\xba\xbe"),
];

/// Reads `bytes` as scannable source text, or says why it is not.
///
/// The magic check runs before the UTF-8 check so an archive is reported as an
/// archive rather than as a UTF-8 failure; `MZ` and `%PDF` are readable ASCII
/// and would otherwise pass.
pub fn classify(bytes: &[u8]) -> Result<&str, ClassificationError> {
    for (name, magic) in CONTAINER_MAGICS {
        if bytes.starts_with(magic) {
            return Err(ClassificationError::ContainerMagic(name));
        }
    }
    let text = core::str::from_utf8(bytes).map_err(|_| ClassificationError::NotUtf8)?;
    if text
        .bytes()
        .any(|byte| byte.is_ascii_control() && !matches!(byte, b'\t' | b'\n' | b'\r'))
    {
        return Err(ClassificationError::ControlByte);
    }
    Ok(text)
}

/// One brace-balanced declaration and the name it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    name: String,
    keyword: String,
    range: SourceRange,
    body: Option<SourceRange>,
}

impl Item {
    /// The declared name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The keyword that introduced the declaration.
    #[must_use]
    pub fn keyword(&self) -> &str {
        &self.keyword
    }

    /// The declaration's whole range, attributes and doc comments included.
    #[must_use]
    pub const fn range(&self) -> SourceRange {
        self.range
    }
}

/// Keywords the item reader recognizes, longest first so `pub fn` is not read as `pub`.
const ITEM_KEYWORDS: &[&str] = &[
    "fn", "struct", "enum", "trait", "impl", "mod", "const", "static", "type", "union", "macro",
];

/// Every declaration in `text`, at every nesting depth, outermost first.
///
/// Offsets are absolute: `base` is added to every range, so a recursive call
/// over an item body reports ranges in the original document.
#[must_use]
pub fn items(text: &str, base: usize) -> Vec<Item> {
    let mut found = Vec::new();
    for item in top_level_items(text, base) {
        if let Some(body) = item.body {
            let inner_start = body.start.saturating_add(1);
            let inner_end = body.end.saturating_sub(1);
            if inner_start < inner_end {
                let relative_start = inner_start.saturating_sub(base);
                let relative_end = inner_end.saturating_sub(base);
                if let Some(inner) = text.get(relative_start..relative_end) {
                    let children = items(inner, inner_start);
                    found.push(item);
                    found.extend(children);
                    continue;
                }
            }
        }
        found.push(item);
    }
    found
}

fn top_level_items(text: &str, base: usize) -> Vec<Item> {
    let bytes = text.as_bytes();
    let mut items = Vec::new();
    let mut cursor = 0_usize;
    let mut header_start: Option<usize> = None;
    let mut depth = 0_usize;
    let mut body_start = 0_usize;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        // A declaration's header is the run of attributes and doc comments
        // directly above it. A blank line ends that run, which is what keeps a
        // module-level `//!` block out of the first item's range.
        if depth == 0 && byte == b'\n' && blank_line_follows(bytes, cursor) {
            header_start = None;
        }
        if !byte.is_ascii_whitespace() && header_start.is_none() {
            header_start = Some(cursor);
        }
        if let Some(skipped) = skip_trivia(text, cursor) {
            cursor = skipped;
            continue;
        }
        match byte {
            b'{' => {
                if depth == 0 {
                    body_start = cursor;
                }
                depth = depth.saturating_add(1);
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = cursor.saturating_add(1);
                    if let Some(start) = header_start {
                        push_item(
                            &mut items,
                            text,
                            base,
                            start,
                            end,
                            Some((body_start, end)),
                            body_start,
                        );
                    }
                    header_start = None;
                }
            }
            b';' if depth == 0 => {
                let end = cursor.saturating_add(1);
                if let Some(start) = header_start {
                    push_item(&mut items, text, base, start, end, None, end);
                }
                header_start = None;
            }
            _ => {}
        }
        cursor = cursor.saturating_add(1);
    }
    items
}

/// Whether the line after the newline at `cursor` holds only whitespace.
fn blank_line_follows(bytes: &[u8], cursor: usize) -> bool {
    let mut ahead = cursor.saturating_add(1);
    while let Some(byte) = bytes.get(ahead) {
        match byte {
            b' ' | b'\t' | b'\r' => ahead = ahead.saturating_add(1),
            b'\n' => return true,
            _ => return false,
        }
    }
    true
}

fn push_item(
    items: &mut Vec<Item>,
    text: &str,
    base: usize,
    start: usize,
    end: usize,
    body: Option<(usize, usize)>,
    header_end: usize,
) {
    let Some(header) = text.get(start..header_end) else {
        return;
    };
    let Some((keyword, name)) = declared_name(header) else {
        return;
    };
    items.push(Item {
        name,
        keyword,
        range: SourceRange {
            start: base.saturating_add(start),
            end: base.saturating_add(end),
        },
        body: body.map(|(body_start, body_end)| SourceRange {
            start: base.saturating_add(body_start),
            end: base.saturating_add(body_end),
        }),
    });
}

/// Reads `keyword name` out of a declaration header.
///
/// Attributes and doc comments precede the keyword and are part of the item, so
/// the search is for the first recognized keyword at a word boundary rather
/// than for the first word.
fn declared_name(header: &str) -> Option<(String, String)> {
    let bytes = header.as_bytes();
    let mut cursor = 0_usize;
    while cursor < bytes.len() {
        if let Some(skipped) = skip_trivia(header, cursor) {
            cursor = skipped;
            continue;
        }
        if !is_word_start(bytes, cursor) {
            cursor = cursor.saturating_add(1);
            continue;
        }
        for keyword in ITEM_KEYWORDS {
            let end = cursor.saturating_add(keyword.len());
            if bytes.get(cursor..end) != Some(keyword.as_bytes()) {
                continue;
            }
            if bytes.get(end).is_some_and(|byte| is_identifier_byte(*byte)) {
                continue;
            }
            if let Some(name) = identifier_after(header, end) {
                return Some(((*keyword).to_owned(), name));
            }
        }
        while cursor < bytes.len() && is_identifier_byte(bytes[cursor]) {
            cursor = cursor.saturating_add(1);
        }
        cursor = cursor.saturating_add(1);
    }
    None
}

fn identifier_after(header: &str, from: usize) -> Option<String> {
    let bytes = header.as_bytes();
    let mut cursor = from;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor = cursor.saturating_add(1);
    }
    let start = cursor;
    while cursor < bytes.len() && is_identifier_byte(bytes[cursor]) {
        cursor = cursor.saturating_add(1);
    }
    if cursor == start {
        return None;
    }
    header.get(start..cursor).map(str::to_owned)
}

const fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_word_start(bytes: &[u8], cursor: usize) -> bool {
    let previous_is_word = cursor
        .checked_sub(1)
        .and_then(|index| bytes.get(index))
        .is_some_and(|byte| is_identifier_byte(*byte));
    !previous_is_word
}

/// Advances past a comment, string, or character literal starting at `cursor`.
///
/// Returns `None` when `cursor` is not the start of one, so the caller can read
/// the byte itself. This is what keeps a brace inside a string or a comment
/// from moving the depth counter.
fn skip_trivia(text: &str, cursor: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let byte = *bytes.get(cursor)?;
    let two = bytes.get(cursor..cursor.saturating_add(2));
    match (byte, two) {
        (b'/', Some(b"//")) => Some(
            text.get(cursor..)?
                .find('\n')
                .map_or(bytes.len(), |offset| cursor.saturating_add(offset)),
        ),
        (b'/', Some(b"/*")) => Some(
            text.get(cursor.saturating_add(2)..)?
                .find("*/")
                .map_or(bytes.len(), |offset| {
                    cursor.saturating_add(4).saturating_add(offset)
                }),
        ),
        (b'"', _) => Some(quoted_end(bytes, cursor, b'"')),
        (b'\'', _) => {
            let end = quoted_end(bytes, cursor, b'\'');
            // A lifetime (`'a`) has no closing quote; the scan must not swallow
            // the rest of the file looking for one.
            if end.saturating_sub(cursor) <= 4 && bytes.get(end.saturating_sub(1)) == Some(&b'\'') {
                Some(end)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn quoted_end(bytes: &[u8], open: usize, quote: u8) -> usize {
    let mut cursor = open.saturating_add(1);
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if byte == b'\\' {
            cursor = cursor.saturating_add(2);
            continue;
        }
        if byte == quote {
            return cursor.saturating_add(1);
        }
        if byte == b'\n' {
            return cursor;
        }
        cursor = cursor.saturating_add(1);
    }
    bytes.len()
}

/// The innermost declaration of each requested symbol, merged and ordered.
///
/// Returns `None` when any requested symbol has no declaration, because a
/// request naming a symbol the document does not declare is out of scope, not a
/// licence to send the whole document.
#[must_use]
pub fn minimal_ranges(text: &str, focus: &[String]) -> Option<Vec<SourceRange>> {
    if focus.is_empty() {
        return None;
    }
    let declarations = items(text, 0);
    let mut selected = Vec::new();
    for symbol in focus {
        let innermost = declarations
            .iter()
            .filter(|item| item.name == *symbol)
            .min_by_key(|item| item.range.len())?;
        selected.push(innermost.range);
    }
    selected.sort_unstable();
    let mut merged: Vec<SourceRange> = Vec::new();
    for range in selected {
        match merged.last_mut() {
            Some(previous) if range.start <= previous.end => {
                previous.end = previous.end.max(range.end);
            }
            _ => merged.push(range),
        }
    }
    Some(merged)
}
