//! Fixtures, and the design-document reader every enumeration is compared with.
//!
//! Nothing here invents a count. [`spec`] returns the authoritative
//! specification's own text and the helpers below cut the exact block or line a
//! test names out of it, so a renumbered section, a reordered bullet or a
//! paraphrase fails as a missing key rather than being absorbed.

use std::{error::Error, fs, path::PathBuf};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The workspace root, from this crate's own manifest directory.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// The authoritative specification, verbatim.
pub fn spec() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// Any repository file, verbatim.
pub fn repository_file(relative: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(relative))?)
}

/// The body of one `### <heading>` section, up to the next heading of any level.
pub fn section(text: &str, heading: &str) -> Result<String, Box<dyn Error>> {
    let marker = format!("### {heading}\n");
    let at = text
        .find(&marker)
        .ok_or_else(|| format!("the specification has no section {heading}"))?;
    let body = &text[at + marker.len()..];
    let end = body
        .find("\n## ")
        .into_iter()
        .chain(body.find("\n### "))
        .min()
        .unwrap_or(body.len());
    Ok(body[..end].to_owned())
}

/// The `- ` bullets of a block, in order, with the marker removed.
///
/// A line that is not a bullet ends the run rather than being skipped: a
/// skipped line is an item that silently stops being required, which is the
/// failure `P2-X2`'s numbered-list parser refuses for the same reason.
pub fn bullets(block: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut started = false;
    for line in block.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            started = true;
            found.push(rest.trim().to_owned());
            continue;
        }
        if started && !trimmed.is_empty() {
            break;
        }
    }
    found
}

/// The one bullet of `block` that starts with `prefix`.
pub fn bullet_starting(block: &str, prefix: &str) -> Result<String, Box<dyn Error>> {
    bullets(block)
        .into_iter()
        .find(|line| line.starts_with(prefix))
        .ok_or_else(|| format!("no bullet starts with {prefix}").into())
}

/// Every back-quoted run in `line`, in order.
pub fn back_quoted(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let tail = &rest[open + 1..];
        let Some(close) = tail.find('`') else {
            break;
        };
        found.push(tail[..close].to_owned());
        rest = &tail[close + 1..];
    }
    found
}

/// The fenced ```text block of a section, without its fences.
pub fn fenced_text(block: &str) -> Result<String, Box<dyn Error>> {
    let open = "```text\n";
    let at = block
        .find(open)
        .ok_or("the section holds no fenced text block")?;
    let body = &block[at + open.len()..];
    let end = body.find("```").ok_or("the fenced block is unterminated")?;
    Ok(body[..end].to_owned())
}
