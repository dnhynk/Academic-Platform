#![allow(dead_code)]
//! Synthetic fixtures and source-reading helpers shared by the two integration
//! suites.
//!
//! Every repository, token, keystore, webhook body, ledger batch and calendar
//! event here is built in-process from fixed values. Nothing reads a network,
//! and nothing can: this crate ships no transport and every seam it names is a
//! trait implemented in this file.
//!
//! The extractors below are `crates/blind-spot/tests/blind_spot_scans.rs`'s,
//! restated because a test module is not a library target.
//! `the_helpers_are_not_vacuous` re-exercises each of them here against a
//! sample it must match, because an extractor that always answered the empty
//! set would satisfy every whole-set comparison in this suite.

use std::{
    cell::Cell,
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use academic_contracts::{DeviceAuthorization, sign_batch, verify_signed_batch};
use academic_crypto::{DeviceKeystore, KeystoreFailure};
use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, ArtifactRepresentation, AuthorityClass, BatchId, Claim,
    ClaimObject, ConfidencePermille, ContentDigest, DeviceId, DomainError, DomainId,
    EVENT_SCHEMA_VERSION, EntityId, EpistemicStatus, EventPayload, EvidenceId, EvidenceItem,
    EvidenceLocator, EvidenceRole, EvidenceStrength, MasteryLevel, MediaType, PermissionLineageId,
    PredicateId, RetentionClass, ScopeDescriptor, ScopeId, TimestampMillis, UnsignedBatch,
    ValidInterval, VaultLocator,
};
use academic_integrations::{
    ConnectorFleet, ConnectorHealth, ConnectorKind, CoreGraph, CoreView, IdeWorkspace, SymbolRef,
    WorkspacePath,
};
use academic_ledger::{LedgerState, event};
use academic_policy::{
    BrokerError, CapabilityToken, ContentDigest as PolicyDigest, DecisionOutcome, EgressRule,
    PermissionBroker, PermissionRequest, PolicySnapshot, ProcessClass, ProviderIdentity,
    ProviderPolicyDraft, ProviderPolicySnapshot, ProviderSurface,
};
use ed25519_dalek::SigningKey;
use zeroize::Zeroizing;

pub type TestResult = Result<(), Box<dyn Error>>;

/// The one process class `P2-G7` admits for an outbound socket capability.
pub const EGRESS_ACTOR: &str = "synthetic-egress-proxy";
pub const EGRESS_CLASS: ProcessClass = ProcessClass::EgressProxy;
/// The purpose the ordinary transfer is granted for.
pub const TRANSFER_PURPOSE: &str = "assistant-context-handoff";
/// The purpose a private blob's second grant is granted for.
pub const DISCLOSURE_PURPOSE: &str = "private-blob-disclosure";

// ---------------------------------------------------------------------------
// Source-reading helpers
// ---------------------------------------------------------------------------

pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

pub fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    let mut found = Vec::new();
    walk(&root, &mut found)?;
    found.retain(|path| {
        !path
            .strip_prefix(&root)
            .unwrap_or(path)
            .starts_with("tests")
    });
    found.sort();
    Ok(found)
}

pub fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
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
pub fn strip_non_code(source: &str) -> String {
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

/// One brace-balanced block's text, from `header` to its matching `}`.
pub fn whole_block(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
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
pub fn collapse(body: &str) -> String {
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
pub fn uses_of(code: &str, name: &str) -> usize {
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

/// The relative path of `path` under the workspace, with forward slashes.
pub fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// This crate's product files, as code with comments and literals removed.
pub fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

/// Joins a path or a macro name that whitespace was inserted into.
pub fn tighten(code: &str) -> String {
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
pub fn absolute_paths(code: &str) -> BTreeSet<String> {
    let roots = ["std", "core", "alloc", "crate", "thiserror", "sha2"];
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
pub fn macros_spelled(code: &str) -> BTreeSet<String> {
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

/// Every `pub fn` in `code`, as its name, its signature up to the body, and the
/// byte offset it starts at.
pub fn public_signature_sites(code: &str) -> Vec<(String, String, usize)> {
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
            found.push((name, collapse(&code[at..end]), at));
            cursor = after;
        }
    }
    found.sort_by_key(|(_, _, at)| *at);
    found
}

/// Every `pub fn` in `code`, as its name and its signature up to the body.
pub fn public_signatures(code: &str) -> Vec<(String, String)> {
    public_signature_sites(code)
        .into_iter()
        .map(|(name, signature, _)| (name, signature))
        .collect()
}

/// Every `pub fn` in `code`, with the type whose `impl` block it sits in.
///
/// The owner matters because an accessor is a way *out* of a value that
/// already holds something, and a constructor is a way to make one. A
/// classification that could not tell them apart would have to admit both or
/// refuse both.
pub fn public_signatures_with_owner(code: &str) -> Vec<(String, String, String)> {
    public_signature_sites(code)
        .into_iter()
        .map(|(name, signature, at)| (owner_before(code, at), name, signature))
        .collect()
}

/// The type named by the last `impl` header before `at`.
fn owner_before(code: &str, at: usize) -> String {
    // `impl<'a, T> Name<'a, T>` has no space after `impl`, so the anchor is the
    // keyword alone. An earlier version anchored on `impl ` and silently
    // reported an empty owner for every generic block.
    let Some(start) = code[..at].rfind("\nimpl") else {
        return String::new();
    };
    let header: String = code[start + 5..]
        .chars()
        .take_while(|character| *character != '{')
        .collect();
    let mut rest = header.trim();
    if rest.starts_with('<') {
        let mut depth = 0_usize;
        let mut end = 0;
        for (offset, character) in rest.char_indices() {
            match character {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth == 0 {
                        end = offset + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = rest[end..].trim();
    }
    if let Some((_, after)) = rest.split_once(" for ") {
        rest = after.trim();
    }
    rest.split(['<', ' ', ':'])
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

/// The text between `code`'s first `{` and its matching `}`, exclusive.
pub fn balanced(code: &str) -> Option<&str> {
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
pub fn top_level_items(body: &str) -> Vec<String> {
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
pub fn use_items(code: &str) -> Vec<String> {
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
            // Whitespace is squeezed rather than removed, so `Digest as _`
            // stays two words. Removing it outright rendered that import as
            // `Digestas_`, which is a spelling nothing could be compared with.
            let mut item = rest[at + 4..at + end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            for (spaced, tight) in [
                (" ::", "::"),
                (":: ", "::"),
                (" {", "{"),
                ("{ ", "{"),
                (" }", "}"),
                ("} ", "}"),
                (" ,", ","),
                (", ", ","),
            ] {
                item = item.replace(spaced, tight);
            }
            flatten(&item, String::new(), &mut found);
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

pub fn module_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// One type declaration's text, from its header to the matching `}` at column
/// zero.
pub fn type_declaration(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{header} has no closing brace at column zero"))?;
    Ok(collapse(&source[start..start + end + 2]))
}

/// The `(name, type)` pairs of one `struct` declaration, in declaration order.
pub fn struct_fields(source: &str, header: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("{header} declares no fields"))?;
    let body = balanced(&source[open..]).ok_or_else(|| format!("{header} is unbalanced"))?;
    let mut found = Vec::new();
    for item in top_level_items(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (name, ty) = item
            .split_once(':')
            .ok_or_else(|| format!("{header} has a field with no type: {item}"))?;
        found.push((
            name.split_whitespace().collect::<Vec<_>>().join(" "),
            ty.split_whitespace().collect::<Vec<_>>().join(" "),
        ));
    }
    Ok(found)
}

/// The variant names of one `enum` declaration, in declaration order.
pub fn enum_variants(source: &str, header: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("{header} declares no variants"))?;
    let body = balanced(&source[open..]).ok_or_else(|| format!("{header} is unbalanced"))?;
    let mut found = Vec::new();
    for item in top_level_items(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let name: String = item
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            found.push(name);
        }
    }
    Ok(found)
}

/// The method names one `trait` block declares, in declaration order.
pub fn trait_methods(source: &str, header: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let block = whole_block(source, header)?;
    let mut found = Vec::new();
    let mut rest = block.as_str();
    while let Some(at) = rest.find("fn ") {
        let after = &rest[at + 3..];
        let name: String = after
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            let end = after
                .find(';')
                .or_else(|| after.find('{'))
                .unwrap_or(after.len());
            found.push(format!("{name} {}", collapse(&after[..end])));
        }
        rest = after;
    }
    Ok(found)
}

pub fn read_module(name: &str) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(
        crate_root().join("src").join(name),
    )?))
}

// ---------------------------------------------------------------------------
// Fleet and core-graph doubles
// ---------------------------------------------------------------------------

/// A fleet that counts every time a health question is asked.
#[derive(Debug)]
pub struct CountingFleet {
    inner: academic_integrations::ConnectorRegistry,
    asked: Cell<usize>,
}

impl CountingFleet {
    #[must_use]
    pub const fn new(inner: academic_integrations::ConnectorRegistry) -> Self {
        Self {
            inner,
            asked: Cell::new(0),
        }
    }

    /// How many times the fleet has been consulted.
    #[must_use]
    pub fn asked(&self) -> usize {
        self.asked.get()
    }
}

impl ConnectorFleet for CountingFleet {
    fn health(&self, kind: ConnectorKind) -> ConnectorHealth {
        self.asked.set(self.asked.get() + 1);
        self.inner.health(kind)
    }
}

/// The real `academic-ledger` state, read through this crate's core seam.
///
/// The three identifiers are resolved once, at construction, so a read is a
/// lookup rather than a parse and the reader has no failure arm of its own.
#[derive(Debug)]
pub struct LedgerCore {
    ledger: LedgerState,
    claim_id: academic_domain::ClaimId,
    evidence_id: EvidenceId,
    artifact_id: ArtifactId,
    reads: Cell<usize>,
}

impl LedgerCore {
    /// Binds a ledger and the three identifiers its views read.
    #[must_use]
    pub const fn new(
        ledger: LedgerState,
        claim_id: academic_domain::ClaimId,
        evidence_id: EvidenceId,
        artifact_id: ArtifactId,
    ) -> Self {
        Self {
            ledger,
            claim_id,
            evidence_id,
            artifact_id,
            reads: Cell::new(0),
        }
    }

    /// How many views have been read.
    #[must_use]
    pub fn reads(&self) -> usize {
        self.reads.get()
    }
}

impl CoreGraph for LedgerCore {
    fn read_view(&self, view: CoreView) -> Vec<u8> {
        self.reads.set(self.reads.get() + 1);
        let mut out = Vec::new();
        out.extend_from_slice(view.as_str().as_bytes());
        out.push(b'\n');
        match view {
            CoreView::LedgerHead => {
                out.extend_from_slice(self.ledger.accept_seq_head().to_string().as_bytes());
            }
            CoreView::AcceptedEvents => {
                for accepted in self.ledger.accepted_events() {
                    out.extend_from_slice(accepted.accept_seq.to_string().as_bytes());
                    out.push(b'\n');
                }
            }
            CoreView::Claims => {
                if let Some(claim) = self.ledger.claim(self.claim_id) {
                    out.extend_from_slice(claim.predicate_id.as_str().as_bytes());
                }
            }
            CoreView::Evidence => {
                if let Some(item) = self.ledger.evidence(self.evidence_id) {
                    out.extend_from_slice(item.extraction_method.as_bytes());
                }
            }
            CoreView::Artifacts => {
                if let Some(descriptor) = self.ledger.artifact(self.artifact_id) {
                    out.extend_from_slice(&descriptor.byte_length.to_be_bytes());
                }
            }
        }
        out
    }
}

/// A workspace that records every call the adapter makes.
#[derive(Debug, Default)]
pub struct RecordingWorkspace {
    open: Vec<WorkspacePath>,
    symbols: Vec<SymbolRef>,
    changed: Cell<usize>,
    changed_paths: Vec<WorkspacePath>,
    calls: Cell<usize>,
}

impl RecordingWorkspace {
    pub fn new(
        open: Vec<WorkspacePath>,
        symbols: Vec<SymbolRef>,
        changed_paths: Vec<WorkspacePath>,
    ) -> Self {
        Self {
            open,
            symbols,
            changed: Cell::new(0),
            changed_paths,
            calls: Cell::new(0),
        }
    }

    /// How many trait methods have been entered.
    #[must_use]
    pub fn calls(&self) -> usize {
        self.calls.get()
    }

    /// Replaces the changed set, which is what a file changing looks like.
    pub fn set_changed(&mut self, changed_paths: Vec<WorkspacePath>) {
        self.changed_paths = changed_paths;
    }
}

impl IdeWorkspace for RecordingWorkspace {
    fn open_paths(&self) -> Vec<WorkspacePath> {
        self.calls.set(self.calls.get() + 1);
        self.open.clone()
    }

    fn symbols(&self, path: &WorkspacePath) -> Vec<SymbolRef> {
        self.calls.set(self.calls.get() + 1);
        self.symbols
            .iter()
            .filter(|symbol| symbol.path() == path)
            .cloned()
            .collect()
    }

    fn changed_paths(&self, _since: TimestampMillis) -> Vec<WorkspacePath> {
        self.calls.set(self.calls.get() + 1);
        self.changed.set(self.changed.get() + 1);
        self.changed_paths.clone()
    }
}

/// An in-memory `P2-K1` keystore double. It is not a broker and holds no key.
#[derive(Debug, Default)]
pub struct MemoryKeystore;

impl DeviceKeystore for MemoryKeystore {
    fn provider(&self) -> &str {
        "synthetic-memory-keystore"
    }

    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        let mut blob = Vec::with_capacity(label.len() + secret.len() + 1);
        blob.extend_from_slice(label.as_bytes());
        blob.push(0);
        blob.extend_from_slice(secret);
        Ok(blob)
    }

    fn open(&self, label: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreFailure> {
        let prefix = label.len() + 1;
        blob.get(prefix..)
            .map(|secret| Zeroizing::new(secret.to_vec()))
            .ok_or(KeystoreFailure::Unavailable)
    }
}

// ---------------------------------------------------------------------------
// The synthetic ledger
// ---------------------------------------------------------------------------

fn id<T: FromStr<Err = DomainError>>(suffix: u32) -> Result<T, DomainError> {
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
}

/// The claim, evidence and artifact identifiers the fixture ledger holds.
///
/// # Errors
///
/// The domain identifier parser, when a literal here stops being a UUIDv7.
pub fn fixture_ids() -> Result<(academic_domain::ClaimId, EvidenceId, ArtifactId), Box<dyn Error>> {
    Ok((id(6)?, id(3)?, id(2)?))
}

/// The subject entity the fixture claim is about.
///
/// # Errors
///
/// As [`fixture_ids`].
pub fn fixture_entity_id() -> Result<EntityId, Box<dyn Error>> {
    Ok(id(4)?)
}

/// A synthetic offering identifier, for the calendar payload.
///
/// # Errors
///
/// As [`fixture_ids`].
pub fn fixture_offering_id() -> Result<academic_domain::OfferingId, Box<dyn Error>> {
    Ok(id(21)?)
}

/// A synthetic course identifier.
///
/// # Errors
///
/// As [`fixture_ids`].
pub fn fixture_course_id() -> Result<academic_domain::CourseId, Box<dyn Error>> {
    Ok(id(22)?)
}

/// A synthetic repository identifier, which a calendar refuses as a subject.
///
/// # Errors
///
/// As [`fixture_ids`].
pub fn fixture_repository_id() -> Result<academic_domain::RepositoryId, Box<dyn Error>> {
    Ok(id(23)?)
}

/// The grade the fixture's attempt carries, and the mastery the fixture's claim
/// carries. `calendar_payload_contains_no_grade_or_state` needs a subject that
/// really does have both, so that a payload that leaked one would have
/// something to leak.
pub const FIXTURE_GRADE: &str = "A+";
pub const FIXTURE_MASTERY: MasteryLevel = MasteryLevel::Practiced;

/// A synthetic ledger holding one scope, one artifact, one evidence item and
/// one mastery claim, accepted through the real signature and batch path.
pub fn fixture_ledger() -> Result<LedgerState, Box<dyn Error>> {
    let domain_id: DomainId = id(1)?;
    let artifact_id: ArtifactId = id(2)?;
    let evidence_id: EvidenceId = id(3)?;
    let subject_id: EntityId = id(4)?;
    let scope_id: ScopeId = id(12)?;
    let user_id: EntityId = id(13)?;
    let device_id: DeviceId = id(8)?;
    let media_type = MediaType::parse("text/plain")?;
    let digest = ContentDigest::sha256(b"synthetic");
    let locator = VaultLocator::derive(b"fixture-domain-key", 1, &media_type, digest)?;
    let evidence_locator = EvidenceLocator::TextBytes {
        source_digest: digest,
        start: 0,
        end: 9,
    };
    let artifact = ArtifactDescriptor {
        id: artifact_id,
        content_digest: digest,
        media_type,
        byte_length: 9,
        domain_id,
        confidentiality: academic_domain::Confidentiality::Personal,
        retention_class: RetentionClass::UserManaged,
        permission_lineage_id: PermissionLineageId::from_str(
            "01900000-0000-7000-8000-000000000005",
        )?,
        format_version: 1,
        vault_locator: locator,
        evidence_representations: vec![ArtifactRepresentation {
            locator: evidence_locator.clone(),
            content_digest: digest,
            byte_length: 9,
        }],
    };
    let evidence = EvidenceItem {
        id: evidence_id,
        artifact_id,
        locator: evidence_locator,
        excerpt_digest: digest,
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "fixture".to_owned(),
        extractor_version: "1".to_owned(),
    };
    let claim = Claim {
        id: id(6)?,
        subject_entity_id: subject_id,
        predicate_id: PredicateId::parse("knowledge.mastery")?,
        object: ClaimObject::Mastery(FIXTURE_MASTERY),
        scope_id,
        authority_class: AuthorityClass::UserExplicit,
        epistemic_status: EpistemicStatus::UserConfirmed,
        confidence: Some(ConfidencePermille::new(900)?),
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(10)),
        evidence_ids: vec![evidence_id],
    };
    let importer = Actor::Importer {
        name: "fixture".to_owned(),
        version: "1".to_owned(),
    };
    let batch = UnsignedBatch {
        schema_version: EVENT_SCHEMA_VERSION,
        batch_id: id::<BatchId>(7)?,
        device_id,
        origin_seq_start: 1,
        origin_seq_end: 4,
        previous_batch_hash: None,
        origin_created_at: TimestampMillis::new(20),
        events: vec![
            event(
                id(9)?,
                1,
                TimestampMillis::new(10),
                importer.clone(),
                domain_id,
                EventPayload::ScopeRegistered(ScopeDescriptor {
                    id: scope_id,
                    domain_id,
                    label: "fixture.scope".to_owned(),
                }),
            ),
            event(
                id(10)?,
                2,
                TimestampMillis::new(11),
                importer.clone(),
                domain_id,
                EventPayload::ArtifactRegistered(artifact),
            ),
            event(
                id(11)?,
                3,
                TimestampMillis::new(12),
                importer,
                domain_id,
                EventPayload::EvidenceRegistered(evidence),
            ),
            event(
                id(14)?,
                4,
                TimestampMillis::new(13),
                Actor::User { user_id },
                domain_id,
                EventPayload::ClaimAsserted(claim),
            ),
        ],
    };
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let authorization = DeviceAuthorization::new(device_id, user_id, signing_key.verifying_key());
    let envelope = sign_batch(&batch, &signing_key)?;
    let verified = verify_signed_batch(&envelope, &authorization)?;
    let mut ledger = LedgerState::new();
    ledger.accept_verified_batch(&verified)?;
    Ok(ledger)
}

// ---------------------------------------------------------------------------
// Broker fixtures
// ---------------------------------------------------------------------------

fn policy_digest(label: &str) -> PolicyDigest {
    PolicyDigest::of(label.as_bytes())
}

pub fn provider_draft(maximum_input_bytes: u64) -> Result<ProviderPolicyDraft, BrokerError> {
    Ok(ProviderPolicyDraft {
        identity: Some(ProviderIdentity::new(
            "synthetic-assistant",
            ProviderSurface::EnterpriseApi,
        )?),
        training_use_enabled: Some(false),
        training_opt_out_applied: Some(false),
        server_retention_millis: Some(0),
        abuse_logging_enabled: Some(false),
        residency_regions: Some(vec!["kr".to_owned()]),
        subprocessors: Some(Vec::new()),
        transit_encryption_declared: Some(true),
        at_rest_encryption_declared: Some(true),
        deletion_api_available: Some(true),
        deletion_receipt_capable: Some(true),
        maximum_input_bytes: Some(maximum_input_bytes),
        logging_configuration: Some("content-logging-disabled".to_owned()),
        policy_source_digest: Some(policy_digest("synthetic-assistant-policy-source")),
        last_verified_at: Some(0),
        ttl_millis: Some(1_000_000),
    })
}

pub fn broker_with_provider(
    maximum_input_bytes: u64,
) -> Result<(PermissionBroker, ProviderPolicySnapshot), BrokerError> {
    let broker = PermissionBroker::new_profile_with_ttl(600_000)?;
    let provider = broker.register_provider_policy(provider_draft(maximum_input_bytes)?, 0)?;
    Ok((broker, provider))
}

fn rule_for(
    staged: &academic_egress_boundary::StagedPayload,
    provider: &ProviderPolicySnapshot,
    rulepack_hash: PolicyDigest,
    purpose_id: &str,
) -> Result<EgressRule, BrokerError> {
    Ok(EgressRule {
        actor_id: EGRESS_ACTOR.to_owned(),
        process_class: EGRESS_CLASS,
        data_class: "synthetic-private-code".to_owned(),
        operation: "assist".to_owned(),
        purpose_id: purpose_id.to_owned(),
        destination_id: provider.destination_id().to_owned(),
        retention_terms_hash: provider.retention_terms_hash(),
        consent_evidence_id: "synthetic-consent-event".to_owned(),
        valid_from: 0,
        valid_until: 1_000_000,
        minimal_ranges: vec![staged.object_range()?],
        payload_digest: staged.preview().digest(),
        provider_policy_snapshot_digest: provider.snapshot_digest().clone(),
        training_use_allowed: false,
        redaction_policy_hash: rulepack_hash,
    })
}

fn request_for(
    staged: &academic_egress_boundary::StagedPayload,
    provider: &ProviderPolicySnapshot,
    policy_version: academic_policy::PolicyVersion,
    purpose_id: &str,
    requested_at: u64,
) -> Result<PermissionRequest, BrokerError> {
    Ok(PermissionRequest {
        actor_id: Some(EGRESS_ACTOR.to_owned()),
        process_class: EGRESS_CLASS,
        data_class: Some("synthetic-private-code".to_owned()),
        object_range_digest_set: Some(vec![staged.object_range()?]),
        operation: Some("assist".to_owned()),
        purpose_id: Some(purpose_id.to_owned()),
        destination_id: Some(provider.destination_id().to_owned()),
        retention_terms_hash: Some(provider.retention_terms_hash()),
        requested_at: Some(requested_at),
        consent_evidence_id: Some("synthetic-consent-event".to_owned()),
        policy_version: Some(policy_version),
    })
}

/// Installs a rule for each named purpose and mints one capability per purpose.
///
/// The two grants are two rows in `P2-G1`'s append-only store, minted from two
/// complete request tuples over the same staged bytes and differing only in
/// purpose. That is what "a second grant" means here.
pub fn capabilities_for(
    broker: &PermissionBroker,
    staged: &academic_egress_boundary::StagedPayload,
    provider: &ProviderPolicySnapshot,
    rulepack_hash: PolicyDigest,
    purposes: &[&str],
    issued_at: u64,
) -> Result<Vec<DecisionOutcome>, BrokerError> {
    let mut rules = Vec::new();
    for purpose in purposes {
        rules.push(rule_for(staged, provider, rulepack_hash.clone(), purpose)?);
    }
    let version = broker.install_policy(PolicySnapshot::from_rules(rules)?)?;
    let mut outcomes = Vec::new();
    for purpose in purposes {
        let request = request_for(staged, provider, version.clone(), purpose, issued_at)?;
        outcomes.push(broker.evaluate(request, issued_at)?);
    }
    Ok(outcomes)
}

/// The capability token and grant identifier out of a decision.
pub fn token(outcome: DecisionOutcome) -> Result<(CapabilityToken, String), Box<dyn Error>> {
    let grant_id = outcome
        .receipt
        .grant_id()
        .ok_or("the broker allowed without minting a grant")?
        .to_owned();
    let capability = outcome
        .capability
        .ok_or("the broker allowed without a capability")?;
    Ok((capability, grant_id))
}

/// A transport that records every chunk it is handed.
#[derive(Debug, Default)]
pub struct RecordingTransport {
    pub written: Vec<u8>,
    pub chunks: usize,
}

impl academic_egress_boundary::OutboundTransport for RecordingTransport {
    fn send_chunk(&mut self, chunk: &[u8]) -> Result<(), academic_egress_boundary::TransportError> {
        self.written.extend_from_slice(chunk);
        self.chunks += 1;
        Ok(())
    }
}

/// A synthetic source document with markers inside and outside the selection.
#[must_use]
pub fn selection_document() -> String {
    [
        "//! A synthetic module used by the integrations acceptance suite.",
        "",
        "/// Sums the credits of a plan.",
        "pub fn selected_total(plan: &[u32]) -> u32 {",
        "    let mut total = 0;",
        "    for credit in plan {",
        "        total += credit;",
        "    }",
        "    total",
        "}",
        "",
        "/// A declaration nobody selected.",
        "pub fn unselected_neighbour(rows: &[u32]) -> u32 {",
        "    let marker_outside_the_selection = 41;",
        "    rows.len() as u32 + marker_outside_the_selection",
        "}",
        "",
        "/// A second declaration nobody selected.",
        "pub fn another_unselected(rows: &[u32]) -> usize {",
        "    let second_marker_outside = 17;",
        "    rows.len() + second_marker_outside",
        "}",
        "",
    ]
    .join("\n")
}
