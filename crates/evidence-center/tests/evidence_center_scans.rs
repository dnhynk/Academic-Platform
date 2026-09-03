//! Source scans for the `P2-X7` evidence and correction centre.
//!
//! Four of this task's claims are shapes of the source rather than behaviours,
//! so nothing at run time would notice the day they stopped being true: that no
//! type in this crate can hold a payload byte, that the class of an inbox entry
//! is its payload's type rather than a string, that nothing but a user
//! settles a conflict, and that nothing extends an expiry.
//! `docs/contracts/policy-source-scans.md` is the page those scans are
//! enumerated on, and this file is written against all five of the empty-scan
//! shapes it names.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends the whole
//! package, not `src` by name, with a floor, a `mod`/`#[path]` tripwire, and a
//! rule that this crate's product source is under `src` and nowhere else.
//!
//! **The primary checks are whole sets, not token lists.** The field inventory
//! is compared in both directions, and — this is the part that matters — so is
//! the set of *declared types* those fields have. `T166` measured that
//! `tools/secret-debug-policy.test.mjs` passes a `Vec<u8>` field named
//! `excerpt`, because that tool matches field **names** against a fixed
//! alternation. A name list cannot be made complete. A whole-set comparison of
//! declared types fails on `Vec<u8>` under any name, including under a type
//! alias, because the alias itself is then the unreviewed type.
//!
//! **The forbidden-token layer is explicitly the weakest.** It is kept because
//! it names the exact shapes `P2-G7` leaked through, and it is listed last for
//! the reason `P2-R2` records: a list is broken by the spelling nobody thought
//! of.
//!
//! **The floors bound the coverage.** A walk that returned nothing would pass
//! every loop below it, so each loop has a floor and each whole-set comparison
//! fails on a missing key as well as an extra one.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_evidence_center::{
    CenterSection, ConflictClass, CorrectionChoice, ProposalClass, ProviderSurface, SpanKind,
};

type TestResult = Result<(), Box<dyn Error>>;

/// The closing brace of an `impl` member, at its own indentation.
///
/// A member's own terminator, so `declared_member` stops at the end of the
/// function rather than at the end of a `match` arm inside it.
const TAIL_BRACE: &str = "
    }
";

// ---------------------------------------------------------------------------
// the walk
// ---------------------------------------------------------------------------

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

fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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

/// Every `.rs` file anywhere under this crate's package directory.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships: everything outside `tests`.
///
/// The package rather than its `src`, for the reason `S-12` records:
/// `crates/record` ships an `examples/` tree and `crates/worker` a `probes/`
/// tree, and both are product-shaped code a walk rooted at `src` never reads.
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

/// Comments, string literals and character literals removed.
///
/// The same lexer `P2-U2`'s scans use: raw strings, nested block comments, line
/// comments, escaped quotes and lifetimes-versus-character-literals.
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
                let end = rest.find(&terminator).map_or(bytes.len(), |at| {
                    probe + 1 + rest[..at].chars().count() + terminator.chars().count()
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
            let closes_two_on = bytes.get(index + 2) == Some(&'\'');
            let closes_three_on = bytes.get(index + 3) == Some(&'\'');
            if closes_two_on || (bytes.get(index + 1) == Some(&'\\') && closes_three_on) {
                index += if closes_two_on { 3 } else { 4 };
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

/// One item's text, ending at `terminator`, comments dropped and whitespace
/// collapsed.
fn declared_member(
    source: &str,
    signature: &str,
    terminator: &str,
) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find(terminator)
        .ok_or_else(|| format!("{signature} has no closing brace at {terminator:?}"))?;
    let body = &source[start..start + end + terminator.len()];
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

/// The authoritative specification.
fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(
        workspace_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?)
}

/// The declared closure of every edge kind, read out of the manifests.
fn workspace_closure(package: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let crates = workspace_root().join("crates");
    let mut by_name: BTreeMap<String, PathBuf> = BTreeMap::new();
    for entry in fs::read_dir(&crates)? {
        let directory = entry?.path();
        let manifest = directory.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let text = fs::read_to_string(&manifest)?;
        if let Some(name) = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("name = \""))
            .and_then(|rest| rest.split('"').next())
        {
            by_name.insert(name.to_owned(), manifest);
        }
    }
    assert!(
        by_name.len() >= 25,
        "the manifest inventory found only {} packages",
        by_name.len()
    );

    fn direct(manifest: &Path) -> Result<Vec<String>, Box<dyn Error>> {
        let text = fs::read_to_string(manifest)?;
        let mut found = Vec::new();
        let mut inside = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                inside = trimmed == "[dependencies]";
                continue;
            }
            if !inside || trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let name: String = trimmed
                .chars()
                .take_while(|character| {
                    character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
                })
                .collect();
            if !name.is_empty() {
                found.push(name);
            }
        }
        Ok(found)
    }

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut pending = vec![package.to_owned()];
    while let Some(name) = pending.pop() {
        if let Some(manifest) = by_name.get(&name) {
            for dependency in direct(manifest)? {
                if seen.insert(dependency.clone()) {
                    pending.push(dependency);
                }
            }
        }
    }
    seen.remove(package);
    Ok(seen)
}

// ---------------------------------------------------------------------------
// the field inventory
// ---------------------------------------------------------------------------

/// One field position found in the product source.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FieldPosition {
    /// `Type` for a struct field, `Enum::Variant` for an enum struct-variant
    /// field, `Enum::Variant#n` for an enum tuple-variant position.
    owner: String,
    /// The field's name, or the tuple position's index as text.
    name: String,
    /// The declared type, whitespace-collapsed.
    declared: String,
}

/// Whether a line opens a type declaration, and what it is called.
fn opened_type(trimmed: &str) -> Option<String> {
    let rest = trimmed
        .strip_prefix("pub struct ")
        .or_else(|| trimmed.strip_prefix("struct "))
        .or_else(|| trimmed.strip_prefix("pub enum "))
        .or_else(|| trimmed.strip_prefix("enum "))
        .or_else(|| trimmed.strip_prefix("pub union "))
        .or_else(|| trimmed.strip_prefix("union "))?;
    let name: String = rest
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Every field position of every type declared in this crate's product source.
///
/// The reader is deliberately simple and deliberately conservative: it tracks
/// the type a brace-delimited body belongs to by line, and it reports a
/// position for a named field, for an enum struct-variant field, and for each
/// position of an enum tuple variant or a tuple struct. A position it cannot
/// classify is reported rather than skipped, so the whole-set comparison fails
/// on it instead of it disappearing.
fn field_positions() -> Result<Vec<FieldPosition>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let mut current_type: Option<String> = None;
        let mut current_variant: Option<String> = None;
        let mut depth = 0_usize;
        let mut type_depth = 0_usize;
        for line in code.lines() {
            let trimmed = line.trim();
            let opens = trimmed.matches('{').count();
            let closes = trimmed.matches('}').count();

            if let Some(name) = opened_type(trimmed) {
                // A tuple struct closes on the same line: `pub struct X(T);`
                if let Some(open) = trimmed.find('(')
                    && trimmed.ends_with(");")
                {
                    let inner = &trimmed[open + 1..trimmed.len() - 2];
                    for (index, part) in split_top_level(inner).into_iter().enumerate() {
                        found.push(FieldPosition {
                            owner: name.clone(),
                            name: index.to_string(),
                            declared: collapse(&part),
                        });
                    }
                    continue;
                }
                if opens > 0 {
                    current_type = Some(name);
                    current_variant = None;
                    type_depth = depth;
                    depth += opens - closes;
                    continue;
                }
                continue;
            }

            if let Some(type_name) = current_type.clone() {
                // An enum tuple variant: `Variant(T, U),`
                if let Some(open) = trimmed.find('(')
                    && trimmed.ends_with("),")
                    && depth == type_depth + 1
                {
                    let variant: String = trimmed[..open]
                        .chars()
                        .take_while(|character| {
                            character.is_ascii_alphanumeric() || *character == '_'
                        })
                        .collect();
                    if !variant.is_empty() {
                        let inner = &trimmed[open + 1..trimmed.len() - 2];
                        for (index, part) in split_top_level(inner).into_iter().enumerate() {
                            found.push(FieldPosition {
                                owner: format!("{type_name}::{variant}"),
                                name: format!("#{index}"),
                                declared: collapse(&part),
                            });
                        }
                    }
                } else if trimmed.ends_with('{') && depth == type_depth + 1 {
                    // An enum struct variant: `Variant {`
                    let variant: String = trimmed
                        .chars()
                        .take_while(|character| {
                            character.is_ascii_alphanumeric() || *character == '_'
                        })
                        .collect();
                    if !variant.is_empty() {
                        current_variant = Some(variant);
                    }
                } else if let Some((name, declared)) =
                    trimmed.trim_end_matches(',').split_once(": ")
                {
                    let name = name.trim_start_matches("pub ").trim();
                    if !name.is_empty()
                        && name
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '_')
                    {
                        let owner = current_variant.as_ref().map_or_else(
                            || type_name.clone(),
                            |variant| format!("{type_name}::{variant}"),
                        );
                        found.push(FieldPosition {
                            owner,
                            name: name.to_owned(),
                            declared: collapse(declared),
                        });
                    }
                }
            }

            depth += opens;
            depth = depth.saturating_sub(closes);
            if current_type.is_some() && depth <= type_depth {
                current_type = None;
                current_variant = None;
            } else if current_variant.is_some() && depth <= type_depth + 1 {
                current_variant = None;
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Splits `a, Vec<b, c>, d` on top-level commas.
fn split_top_level(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut angle = 0_i32;
    let mut round = 0_i32;
    for character in text.chars() {
        match character {
            '<' => angle += 1,
            '>' => angle -= 1,
            '(' => round += 1,
            ')' => round -= 1,
            ',' if angle == 0 && round == 0 => {
                parts.push(current.trim().to_owned());
                current = String::new();
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    let last = current.trim();
    if !last.is_empty() {
        parts.push(last.to_owned());
    }
    parts
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A view of `code` with the whitespace Rust allows *inside* a path removed.
///
/// `academic_domain :: ClaimId` and `academic_domain::ClaimId` are the same
/// path, and `P2-R2` measured the first one passing a guard that read the
/// second. Only the whitespace around a `::` is removed; deleting all of it
/// joins unrelated tokens and makes keys disappear, which is worse than the
/// hole it closes.
fn paths_normalised(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let characters: Vec<char> = code.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        if characters[index] == ':' && characters.get(index + 1) == Some(&':') {
            while out.ends_with(char::is_whitespace) {
                out.pop();
            }
            out.push_str("::");
            index += 2;
            while index < characters.len() && characters[index].is_whitespace() {
                index += 1;
            }
            continue;
        }
        out.push(characters[index]);
        index += 1;
    }
    out
}

/// Every identifier this code writes a `::` after, as a path root.
///
/// A middle segment is skipped -- the `b` of `a::b::c` yields one key, not two
/// -- and a **leading** `::` is not a middle segment: in `::std::path::Path`
/// the root is `std`, and what tells the two apart is the character before the
/// `::`. `P2-R2` found a guard that got that wrong and let
/// `::std::path::Path::new(p).metadata()` through.
fn path_roots(code: &str) -> BTreeSet<String> {
    let normalised = paths_normalised(code);
    let bytes = normalised.as_bytes();
    let mut found = BTreeSet::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b':' && bytes.get(index + 1) == Some(&b':') {
            let mut start = index;
            while start > 0
                && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_')
            {
                start -= 1;
            }
            if start < index {
                // A middle segment is one whose identifier is itself preceded
                // by a `::` that is preceded by an identifier character. A
                // leading `::` fails that second condition, which is what keeps
                // `::std::path` reporting `std`.
                let middle = start >= 3
                    && bytes[start - 1] == b':'
                    && bytes[start - 2] == b':'
                    && (bytes[start - 3].is_ascii_alphanumeric() || bytes[start - 3] == b'_');
                if !middle {
                    found.insert(normalised[start..index].to_owned());
                }
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    found
}

/// Every macro this code invokes, as a whole name.
///
/// A keyword is not a name a macro may have, which is what stops `if !(x)`
/// being read as a macro called `if`.
fn macros_invoked(code: &str) -> BTreeSet<String> {
    const KEYWORDS: [&str; 8] = ["if", "while", "match", "return", "let", "else", "for", "loop"];
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'!' {
            let mut cursor = index;
            while cursor > 0 && bytes[cursor - 1].is_ascii_whitespace() {
                cursor -= 1;
            }
            let mut start = cursor;
            while start > 0
                && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_')
            {
                start -= 1;
            }
            if start < cursor {
                let name = &code[start..cursor];
                let opens = bytes[index + 1..]
                    .iter()
                    .find(|byte| !byte.is_ascii_whitespace())
                    .is_some_and(|byte| matches!(byte, b'(' | b'[' | b'{'));
                if opens && !KEYWORDS.contains(&name) {
                    found.insert(name.to_owned());
                }
            }
        }
        index += 1;
    }
    found
}

/// Every type constructor named inside a declared type, as whole identifiers.
///
/// `Vec<ObjectRange>` yields `Vec` and `ObjectRange`; `Option<SnapshotId>`
/// yields `Option` and `SnapshotId`; `&'static str` yields `str`; `[u8; 32]`
/// yields `u8`. Lifetimes are dropped because a lifetime is not a type. What
/// remains is the whole set a reviewer has to have looked at.
fn type_constructors(declared: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let characters: Vec<char> = declared.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        if characters[index] == '\'' {
            index += 1;
            while index < characters.len()
                && (characters[index].is_ascii_alphanumeric() || characters[index] == '_')
            {
                index += 1;
            }
            continue;
        }
        if characters[index].is_ascii_alphabetic() || characters[index] == '_' {
            let start = index;
            while index < characters.len()
                && (characters[index].is_ascii_alphanumeric() || characters[index] == '_')
            {
                index += 1;
            }
            let word: String = characters[start..index].iter().collect();
            if !matches!(word.as_str(), "mut" | "dyn" | "impl" | "as" | "where") {
                found.insert(word);
            }
            continue;
        }
        index += 1;
    }
    found
}

/// Every field position in this crate, and the type each one declares.
///
/// The list is compared in **both** directions: a field added anywhere fails as
/// an extra key, and a field removed fails as a missing one. Each entry is a
/// `(owner, name, declared type)` triple, so moving a field between two types,
/// renaming it, or changing its type each fails as a different mismatch.
const FIELD_INVENTORY: [(&str, &str, &str); 88] = [
    // --- lib.rs: the centre and its index ---------------------------------
    ("CenterItem::Conflict", "#0", "ConflictClass"),
    ("CenterItem::Conflict", "#1", "academic_domain::ClaimId"),
    (
        "CenterItem::DeletionReceipt",
        "#0",
        "academic_domain::EgressDecisionId",
    ),
    (
        "CenterItem::LowConfidenceSpan",
        "#0",
        "SpanKind",
    ),
    (
        "CenterItem::LowConfidenceSpan",
        "#1",
        "academic_domain::LectureSessionId",
    ),
    ("CenterItem::Permission", "#0", "PermissionRef"),
    ("CenterItem::Proposal", "#0", "academic_proposal::ProposalId"),
    ("CenterItem::Proposal", "#1", "ProposalClass"),
    (
        "CenterItem::SourceChange",
        "#0",
        "academic_domain::ContentDigest",
    ),
    (
        "CenterItem::Transmission",
        "#0",
        "academic_domain::EgressDecisionId",
    ),
    ("EvidenceCenter", "conflicts", "ConflictBoard"),
    ("EvidenceCenter", "corrections", "CorrectionLedger"),
    ("EvidenceCenter", "inbox", "ProposalInbox"),
    ("EvidenceCenter", "low_confidence", "LowConfidenceQueue"),
    ("EvidenceCenter", "permissions", "PermissionQueue"),
    ("EvidenceCenter", "source_changes", "SourceChangeLog"),
    ("EvidenceCenter", "transmissions", "TransmissionLog"),
    ("SectionIndex", "items", "Vec<CenterItem>"),
    ("SectionIndex", "section", "CenterSection"),
    // --- error.rs ----------------------------------------------------------
    ("CenterError::NoEntryOfClass", "class", "ProposalClass"),
    ("CenterError::NoSuchConflict", "claim", "ClaimId"),
    ("CenterError::NoSuchConflict", "class", "ConflictClass"),
    ("CenterError::NotTheUser", "refusal", "WorkflowError"),
    ("CenterError::PermissionAbsent", "permission", "PermissionRef"),
    ("CenterError::PermissionExpired", "expires_at", "TimestampMillis"),
    ("CenterError::PermissionExpired", "permission", "PermissionRef"),
    ("CenterError::ProposalAlreadyAdmitted", "proposal", "ProposalId"),
    // --- inbox.rs ----------------------------------------------------------
    ("ConceptMergeProposal", "absorbed", "EntityId"),
    ("ConceptMergeProposal", "evidence_before_merge", "u32"),
    ("ConceptMergeProposal", "header", "ProposalHeader"),
    ("ConceptMergeProposal", "retained", "EntityId"),
    ("InboxEntry::ConceptMerge", "#0", "ConceptMergeProposal"),
    (
        "InboxEntry::ProjectClassification",
        "#0",
        "ProjectClassificationProposal",
    ),
    ("InboxEntry::Relation", "#0", "RelationProposal"),
    ("InboxEntry::StateUpdate", "#0", "StateUpdateProposal"),
    ("ProjectClassificationProposal", "classification", "FindingClassification"),
    ("ProjectClassificationProposal", "finding", "FindingId"),
    ("ProjectClassificationProposal", "header", "ProposalHeader"),
    ("ProjectClassificationProposal", "project", "EntityId"),
    ("ProjectClassificationProposal", "snapshot", "SnapshotId"),
    ("ProposalHeader", "confidence", "ConfidencePermille"),
    ("ProposalHeader", "id", "ProposalId"),
    ("ProposalHeader", "impact", "ImpactPermille"),
    ("ProposalHeader", "model_run", "ModelRunId"),
    ("ProposalHeader", "proposed_at", "TimestampMillis"),
    ("ProposalHeader", "tier", "RiskTier"),
    ("ProposalInbox", "entries", "Vec<InboxEntry>"),
    ("RelationProposal", "corroborating_sources", "u32"),
    ("RelationProposal", "header", "ProposalHeader"),
    ("RelationProposal", "object", "EntityId"),
    ("RelationProposal", "predicate", "PredicateId"),
    ("RelationProposal", "subject", "EntityId"),
    ("StateUpdateProposal", "concept", "EntityId"),
    ("StateUpdateProposal", "from_level", "MasteryLevel"),
    ("StateUpdateProposal", "header", "ProposalHeader"),
    ("StateUpdateProposal", "to_level", "MasteryLevel"),
    // --- conflict.rs -------------------------------------------------------
    ("ConflictBoard", "cases", "Vec<ConflictCase>"),
    ("ConflictCase", "class", "ConflictClass"),
    ("ConflictCase", "held", "ConflictSide"),
    ("ConflictCase", "history", "Vec<CorrectionRecord>"),
    ("ConflictCase", "incoming", "ConflictSide"),
    ("ConflictCase", "opened_at", "TimestampMillis"),
    ("ConflictSide", "applies", "ValidInterval"),
    ("ConflictSide", "authority", "AuthorityClass"),
    ("ConflictSide", "claim", "ClaimId"),
    ("ConflictSide", "lane", "ConflictLane"),
    ("ConflictSide", "observed_in", "Option<SnapshotId>"),
    ("ConflictSide", "recorded_at", "TimestampMillis"),
    ("ConflictSide", "status", "EpistemicStatus"),
    ("CorrectionOutcome::EndScope", "ends_at", "TimestampMillis"),
    ("CorrectionOutcome::Modify", "replacement", "ClaimId"),
    ("CorrectionRecord", "decided_at", "TimestampMillis"),
    ("CorrectionRecord", "decided_by", "UserDecision"),
    ("CorrectionRecord", "outcome", "CorrectionOutcome"),
    ("Resolution::Settled", "#0", "CorrectionChoice"),
    // --- correction.rs -----------------------------------------------------
    ("CorrectionLedger", "markers", "Vec<CorrectionMarker>"),
    ("CorrectionLedger", "used", "Vec<UsedClaim>"),
    ("CorrectionMarker", "corrected", "ClaimId"),
    ("CorrectionMarker", "origin", "CorrectionOrigin"),
    ("CorrectionMarker", "recorded_at", "TimestampMillis"),
    ("CorrectionMarker", "recorded_at_seq", "u64"),
    ("CorrectionMarker", "superseding", "ClaimId"),
    ("HistoricalView", "coordinates", "TimeCoordinates"),
    ("HistoricalView", "markers", "Vec<CorrectionMarker>"),
    ("HistoricalView", "shown", "Vec<UsedClaim>"),
    ("UsedClaim", "accepted_at_seq", "u64"),
    ("UsedClaim", "applies_from", "TimestampMillis"),
    ("UsedClaim", "claim", "ClaimId"),
];

/// The rest of the inventory, split only because a Rust array literal of this
/// size is unreadable as one.
const FIELD_INVENTORY_TAIL: [(&str, &str, &str); 40] = [
    // --- low_confidence.rs -------------------------------------------------
    ("DocumentRegionLocator", "document", "LectureDocumentId"),
    ("DocumentRegionLocator", "page", "u32"),
    ("DocumentRegionLocator", "session", "LectureSessionId"),
    ("DocumentRegionLocator", "source_image", "ContentDigest"),
    ("LowConfidenceQueue", "spans", "Vec<LowConfidenceSpan>"),
    ("LowConfidenceSpan::Code", "confidence", "ConfidencePermille"),
    ("LowConfidenceSpan::Code", "locator", "DocumentRegionLocator"),
    ("LowConfidenceSpan::Math", "confidence", "ConfidencePermille"),
    ("LowConfidenceSpan::Math", "locator", "DocumentRegionLocator"),
    (
        "LowConfidenceSpan::Transcript",
        "confidence",
        "ConfidencePermille",
    ),
    (
        "LowConfidenceSpan::Transcript",
        "locator",
        "TranscriptLocator",
    ),
    ("TranscriptLocator", "ends_at", "TimestampMillis"),
    ("TranscriptLocator", "session", "LectureSessionId"),
    ("TranscriptLocator", "starts_at", "TimestampMillis"),
    ("TranscriptLocator", "version", "TranscriptVersionId"),
    // --- permission.rs -----------------------------------------------------
    ("DependentAction", "kind", "DependentActionKind"),
    ("DependentAction", "requires", "PermissionRef"),
    ("DependentAction", "subject", "EntityId"),
    ("ExpiringPermission", "expires_at", "TimestampMillis"),
    ("ExpiringPermission", "granted_at", "TimestampMillis"),
    ("ExpiringPermission", "lineage", "PermissionLineageId"),
    ("ExpiringPermission", "reference", "PermissionRef"),
    ("LivePermission", "proved_at", "TimestampMillis"),
    ("LivePermission", "reference", "PermissionRef"),
    ("PermissionQueue", "dependents", "Vec<DependentAction>"),
    ("PermissionQueue", "permissions", "Vec<ExpiringPermission>"),
    ("PermissionRef::Capture", "#0", "CapturePermissionId"),
    ("PermissionRef::Consent", "#0", "ConsentId"),
    // --- source_change.rs --------------------------------------------------
    ("SourceChangeEntry", "connector", "ConnectorId"),
    ("SourceChangeEntry", "current_content", "ContentDigest"),
    ("SourceChangeEntry", "document_changes", "Vec<DocumentChange>"),
    ("SourceChangeEntry", "impacted_plans", "Vec<DependentNode>"),
    ("SourceChangeEntry", "impacted_rules", "Vec<RuleId>"),
    ("SourceChangeEntry", "observed_at", "TimestampMillis"),
    ("SourceChangeEntry", "previous_content", "ContentDigest"),
    ("SourceChangeLog", "entries", "Vec<SourceChangeEntry>"),
    // --- transmission.rs ---------------------------------------------------
    ("DeletionReceiptRef", "provider_policy_snapshot", "ContentDigest"),
    ("DeletionReceiptRef", "receipt_digest", "ContentDigest"),
    ("DeletionReceiptRef", "received_at", "TimestampMillis"),
    ("DeletionReceiptRef", "requested_at", "TimestampMillis"),
];

/// The remainder.
const FIELD_INVENTORY_TAIL_TWO: [(&str, &str, &str); 14] = [
    ("ObjectRange", "length", "u64"),
    ("ObjectRange", "offset", "u64"),
    ("ProviderRef", "destination", "ContentDigest"),
    ("ProviderRef", "surface", "ProviderSurface"),
    ("ReceiptState::Received", "#0", "DeletionReceiptRef"),
    ("ReceiptState::Requested", "requested_at", "TimestampMillis"),
    ("TransmissionLog", "records", "Vec<TransmissionRecord>"),
    ("TransmissionRecord", "decision", "EgressDecisionId"),
    ("TransmissionRecord", "payload_digest", "ContentDigest"),
    ("TransmissionRecord", "provider", "ProviderRef"),
    ("TransmissionRecord", "purpose", "TransmissionPurpose"),
    ("TransmissionRecord", "ranges", "Vec<ObjectRange>"),
    ("TransmissionRecord", "receipt", "ReceiptState"),
    ("TransmissionRecord", "transmitted_at", "TimestampMillis"),
];

/// Every type constructor a field in this crate may declare, with the reason it
/// cannot hold a payload byte.
///
/// This is the guard the whole "no payload byte" claim rests on, and it is a
/// **whole set compared in both directions**: an unreviewed type fails as an
/// extra key whatever the field is called, and a reviewed type that no field
/// uses any more fails as a dead entry. `String`, `str`, `u8`, `Box` and
/// `Untrusted` are absent, and their absence is the claim.
///
/// | Group | Why it holds no payload byte |
/// |---|---|
/// | `u32`, `u64` | counts, offsets and sequence numbers |
/// | the UUID identifiers | opaque 128-bit values with no text |
/// | `ContentDigest` | a one-way SHA-256 of bytes this crate never holds |
/// | `TimestampMillis`, `ValidInterval`, `TimeCoordinates` | instants |
/// | `ConfidencePermille`, `ImpactPermille` | bounded integers |
/// | the closed enums | no data but their own arms, all of which are here |
/// | `PredicateId`, `RuleId`, `ConnectorId` | validated identifiers, charset-restricted by their own crates to `[A-Za-z0-9._-]` or narrower, so none can carry prose, a separator or a directive |
/// | `UserDecision`, `WorkflowError` | `P2-M2`'s receipt (a `u128`) and its refusal (a `&'static str` actor-kind name from that crate's own source) |
/// | `DependentNode`, `DocumentChange` | `P2-U6`'s graph node and header-change arm |
/// | `Vec`, `Option` | containers, judged by what is inside them, which is also here |
const DECLARED_TYPE_ALLOWLIST: [&str; 46] = [
    "AuthorityClass",
    "CapturePermissionId",
    "CenterSection",
    "CenterItem",
    "ClaimId",
    "ConceptMergeProposal",
    "ConfidencePermille",
    "ConflictBoard",
    "ConflictCase",
    "ConflictClass",
    "ConflictLane",
    "ConflictSide",
    "ConnectorId",
    "ConsentId",
    "ContentDigest",
    "CorrectionChoice",
    "CorrectionLedger",
    "CorrectionMarker",
    "CorrectionOrigin",
    "CorrectionOutcome",
    "CorrectionRecord",
    "DeletionReceiptRef",
    "DependentAction",
    "DependentActionKind",
    "DependentNode",
    "DocumentChange",
    "DocumentRegionLocator",
    "EgressDecisionId",
    "EntityId",
    "EpistemicStatus",
    "ExpiringPermission",
    "FindingClassification",
    "FindingId",
    "ImpactPermille",
    "InboxEntry",
    "LectureDocumentId",
    "LectureSessionId",
    "LowConfidenceQueue",
    "LowConfidenceSpan",
    "MasteryLevel",
    "ModelRunId",
    "ObjectRange",
    "Option",
    "PermissionLineageId",
    "PermissionQueue",
    "PermissionRef",
];

/// The rest of the allowlist.
const DECLARED_TYPE_ALLOWLIST_TAIL: [&str; 29] = [
    "PredicateId",
    "ProjectClassificationProposal",
    "ProposalClass",
    "ProposalHeader",
    "ProposalId",
    "ProposalInbox",
    "ProviderRef",
    "ProviderSurface",
    "ReceiptState",
    "RelationProposal",
    "RiskTier",
    "RuleId",
    "SnapshotId",
    "SourceChangeEntry",
    "SourceChangeLog",
    "SpanKind",
    "StateUpdateProposal",
    "TimeCoordinates",
    "TimestampMillis",
    "TranscriptLocator",
    "TranscriptVersionId",
    "TransmissionLog",
    "TransmissionPurpose",
    "TransmissionRecord",
    "UsedClaim",
    "UserDecision",
    "ValidInterval",
    "Vec",
    "WorkflowError",
];

/// The scalar type names a field may declare.
///
/// Held apart from the type allowlist so that the day a numeric width is added
/// it is reviewed as a numeric width. `u8` is deliberately absent: a `[u8; N]`
/// or a `Vec<u8>` is exactly the shape `P2-G7` leaked a payload through.
const SCALAR_ALLOWLIST: [&str; 2] = ["u32", "u64"];

/// The spellings the weakest layer refuses.
///
/// Kept because each names a shape this repository has actually leaked through
/// or has had to keep out, and listed last because a list of spellings is
/// broken by the spelling nobody predicted. The whole-set comparisons above are
/// what actually decide.
const WEAKEST_LAYER_SPELLINGS: [&str; 10] = [
    "Vec<u8>",
    "&[u8]",
    "Box<[u8]>",
    "transmitted_bytes",
    "source_bytes",
    "payload_bytes",
    "Untrusted",
    "StagedPayload",
    "Preview",
    "Box::leak",
];

/// This crate's whole declared dependency closure, every edge kind.
///
/// **`academic-egress-boundary` and `academic-policy` are in it, and that is
/// not an oversight.** `academic-untrusted-content` declares a product edge to
/// `academic-egress-boundary`, so every crate that links `P2-G5`'s trust label
/// transitively links the crate that owns `StagedPayload` and `Preview` and,
/// through it, the broker and its bundled SQLite. `academic-ingestion`,
/// `academic-curriculum`, `academic-requirement` and `academic-repository` all
/// carry the same closure, and `P2-U2`'s admission receipt records it.
///
/// So the "no payload byte" claim here is **not** an edge claim about
/// `StagedPayload`. What refuses it is [`PATH_ROOTS`] — a whole-set allowlist
/// of the crate roots this crate's product source spells a `::` after — and the
/// field-type inventory below. The closure comparison still carries the
/// narrower claim in [`FORBIDDEN_IN_CLOSURE`]: no writer, no key, no model and
/// no process launcher is reachable at all.
const PRODUCT_CLOSURE: [&str; 13] = [
    "academic-domain",
    "academic-egress-boundary",
    "academic-ingestion",
    "academic-policy",
    "academic-proposal",
    "academic-untrusted-content",
    "hex",
    "hmac",
    "rusqlite",
    "serde",
    "sha2",
    "thiserror",
    "uuid",
];

/// The crates that own a canonical write, a key, a model run, or a process.
///
/// None of them may be in the closure at any feature setting.
const FORBIDDEN_IN_CLOSURE: [&str; 12] = [
    "academic-store",
    "academic-store-platform",
    "academic-vault",
    "academic-crypto",
    "academic-keystore-platform",
    "academic-projections",
    "academic-transcript",
    "academic-record",
    "academic-model-run",
    "academic-worker",
    "academic-core",
    "academic-rpc",
];

/// Every crate root, module root and type this crate's product source writes a
/// `::` after.
///
/// This is the guard that replaces the edge claim the closure cannot make. It
/// is a **closed world compared in both directions**, read on paths rather than
/// on `use` items, so a fully qualified `academic_untrusted_content::Untrusted`
/// or `academic_egress_boundary::Preview` is refused even though it spells no
/// `use` and even though both crates are reachable. `P2-R2`'s repair is the
/// shape: a leading `::`, whitespace inside a path, and a middle segment are
/// each handled, because each of those defeated the first version of that
/// guard.
const PATH_ROOTS: [&str; 34] = [
    "CenterError",
    "CenterItem",
    "CenterSection",
    "ConflictBoard",
    "CorrectionChoice",
    "CorrectionLedger",
    "LowConfidenceQueue",
    "PermissionKind",
    "PermissionQueue",
    "ProposalClass",
    "ProposalInbox",
    "ReceiptState",
    "Resolution",
    "Self",
    "SourceChangeLog",
    "SpanKind",
    "TransmissionLog",
    "UserDecision",
    "Vec",
    "academic_domain",
    "academic_ingestion",
    "academic_proposal",
    "conflict",
    "correction",
    "crate",
    "engines",
    "error",
    "inbox",
    "low_confidence",
    "permission",
    "source_change",
    "temporal",
    "thiserror",
    "transmission",
];

/// Every macro this crate invokes.
///
/// A macro is not a path, so the closed world above is blind to one.
/// `include_str!` reads a file at compile time and spells no path; `P2-R2`
/// measured it passing a guard that had just been repaired against three other
/// bypasses.
const MACROS_INVOKED: [&str; 0] = [];

/// Every function in this crate that returns a `&'static str`.
///
/// Each is a total `match` over a closed enum with no argument. They are the
/// only route by which text leaves this crate, and the text is a literal in
/// this file's own source rather than anything a caller supplied.
const STATIC_STR_RETURNS: [&str; 8] = [
    "CenterSection::spec_words",
    "ConflictClass::marker_token",
    "ConflictClass::spec_words",
    "CorrectionChoice::spec_words",
    "ProposalClass::spec_words",
    "ProviderSurface::as_str",
    "SpanKind::marker_token",
    "SpanKind::spec_words",
];

// ---------------------------------------------------------------------------
// the_walk_reads_every_module_in_this_crate
// ---------------------------------------------------------------------------

/// The walk every scan below reads through, with a floor and a tripwire.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = crate_all_sources()?;
    assert!(
        sources.len() >= 10,
        "the walk found only {} files under the package",
        sources.len()
    );

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

    // The walk read this file, which is in `tests` rather than in `src`. That
    // is what says it descended the package rather than `src` by name.
    assert!(
        sources
            .iter()
            .any(|path| path.ends_with("evidence_center_scans.rs")),
        "the walk did not read this file, so it is not reading the package"
    );

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

    let mut declared = 0_usize;
    for path in &sources {
        let source = fs::read_to_string(path)?;
        let mut pending: Option<PathBuf> = None;
        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("#[path = \"") {
                let target = rest.split('"').next().unwrap_or_default();
                pending = Some(
                    path.parent()
                        .map_or_else(|| PathBuf::from(target), |parent| parent.join(target)),
                );
                continue;
            }
            if let Some(name) = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))
                .and_then(|rest| rest.strip_suffix(';'))
            {
                declared += 1;
                if let Some(target) = pending.take() {
                    assert!(
                        target.exists(),
                        "{} declares a #[path] target the walk cannot read: {}",
                        relative(path),
                        target.display()
                    );
                    continue;
                }
                assert!(
                    read.contains(name),
                    "{} declares module {name}, which the walk did not read",
                    relative(path)
                );
            }
        }
    }
    assert!(
        declared >= 8,
        "the tripwire checked only {declared} module declarations"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// the_six_sections_are_section_25_13s_own
// ---------------------------------------------------------------------------

/// The centre's own enumeration is the specification's, read out of it.
///
/// The comparison removes each arm's words from its bullet and requires what
/// remains to be punctuation and connective text this test names, so a section
/// renamed or paraphrased here leaves text behind and fails. The four proposal
/// classes, the two conflict classes and the three span kinds are each read out
/// of their own bullet the same way.
#[test]
fn the_six_sections_are_section_25_13s_own() -> TestResult {
    let specification = specification()?;
    let start = specification
        .find("### 25.13 Evidence & Correction Center")
        .ok_or("section 25.13 is not in the specification")?;
    let rest = &specification[start..];
    let end = rest.find("\n---").ok_or("section 25.13 does not end")?;
    let block = &rest[..end];

    let bullets: Vec<&str> = block
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .collect();
    assert_eq!(
        bullets.len(),
        CenterSection::ALL.len(),
        "section 25.13 does not have one bullet per section: {bullets:?}"
    );

    for (section, bullet) in CenterSection::ALL.into_iter().zip(&bullets) {
        assert!(
            bullet.contains(section.spec_words()),
            "{section:?} does not name its own bullet: {bullet}"
        );
    }

    // The four proposal classes are the first bullet's own words.
    let inbox_bullet = bullets[0];
    let mut remaining = inbox_bullet.to_owned();
    remaining = remaining.replace(CenterSection::ProposalInbox.spec_words(), "");
    for class in ProposalClass::ALL {
        let words = class.spec_words();
        assert!(
            remaining.contains(words),
            "the specification's inbox bullet does not name {words}"
        );
        remaining = remaining.replacen(words, "", 1);
    }
    assert!(
        remaining
            .chars()
            .all(|character| character.is_whitespace() || ":,.".contains(character)),
        "the inbox bullet holds a class the enumeration does not: {remaining:?}"
    );

    // The two conflict classes are the third bullet's own words.
    let mut conflict_remaining = bullets[2].to_owned();
    conflict_remaining =
        conflict_remaining.replace(CenterSection::UnresolvedConflict.spec_words(), "");
    for class in ConflictClass::ALL {
        let words = class.spec_words();
        assert!(
            conflict_remaining.contains(words),
            "the specification's conflict bullet does not name {words}"
        );
        conflict_remaining = conflict_remaining.replacen(words, "", 1);
    }
    assert!(
        conflict_remaining
            .chars()
            .all(|character| character.is_whitespace() || ":,.".contains(character)),
        "the conflict bullet holds a class the enumeration does not: {conflict_remaining:?}"
    );

    // The three span kinds are the fourth bullet's own words.
    let span_bullet = bullets[3];
    for kind in SpanKind::ALL {
        assert!(
            span_bullet.contains(kind.spec_words()),
            "the specification's low-confidence bullet does not name {:?}",
            kind.spec_words()
        );
    }

    // Section 30.4's three choices, read out of section 30.4.
    let override_paragraph = specification
        .find("사용자가 유지·수정·scope 종료를 선택한다")
        .ok_or("section 30.4's three choices are not in the specification")?;
    let sentence = &specification[override_paragraph..override_paragraph + 80];
    for choice in CorrectionChoice::ALL {
        assert!(
            sentence.contains(choice.spec_words()),
            "section 30.4 does not name {:?}",
            choice.spec_words()
        );
    }

    // The scan is not vacuous: a word no section names is not found.
    assert!(
        !block.contains("graduation audit"),
        "the block read is not section 25.13"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// the_center_cannot_name_a_payload_byte
// ---------------------------------------------------------------------------

/// No type, no signature and no dependency of this crate can hold a payload
/// byte.
///
/// Four layers, strongest first. Each is blind to a bypass the next one sees.
#[test]
fn the_center_cannot_name_a_payload_byte() -> TestResult {
    // ---- Layer one: the closure -------------------------------------------
    let closure = workspace_closure("academic-evidence-center")?;
    assert_eq!(
        closure,
        PRODUCT_CLOSURE
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate's product dependency closure changed; a payload, a writer or \
         a key may now be in reach"
    );
    // The closure walk is not vacuous: it reached past the direct edges.
    assert!(
        closure.contains("academic-untrusted-content"),
        "the closure walk stopped at the direct edges"
    );
    for forbidden in FORBIDDEN_IN_CLOSURE {
        assert!(
            !closure.contains(forbidden),
            "{forbidden} is in the centre's dependency closure"
        );
    }

    // ---- Layer one and a half: the closed world over path roots ------------
    //
    // The closure holds `academic-egress-boundary` and `academic-policy`, so
    // the edge cannot say `StagedPayload` is unreachable. This does: a whole
    // set of the roots this crate spells a `::` after, compared in both
    // directions, read on paths rather than on imports.
    let mut roots: BTreeSet<String> = BTreeSet::new();
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        roots.extend(path_roots(&code));
        macros.extend(macros_invoked(&code));
    }
    assert_eq!(
        roots,
        PATH_ROOTS.into_iter().map(str::to_owned).collect(),
        "the set of path roots this crate spells changed"
    );
    assert_eq!(
        macros,
        MACROS_INVOKED.into_iter().map(str::to_owned).collect(),
        "the set of macros this crate invokes changed"
    );
    // The closed world is not vacuous: the three crates that are reachable and
    // must not be named are absent from it, and so is every filesystem and
    // process root.
    for absent in [
        "academic_untrusted_content",
        "academic_egress_boundary",
        "academic_policy",
        "std",
        "alloc",
        "libc",
        "rusqlite",
    ] {
        assert!(
            !roots.contains(absent),
            "{absent} is on this crate's path-root allowlist"
        );
    }
    // And the extractor really reads a path: the three shapes that defeated
    // `P2-R2`'s first repair each yield the root they name, and a middle
    // segment still yields none.
    for sample in [
        "let x = academic_domain::ClaimId::from(y);",
        "let x = academic_domain :: ClaimId :: from(y);",
        "let x = ::academic_domain::ClaimId::from(y);",
    ] {
        assert!(
            path_roots(sample).contains("academic_domain"),
            "the path reader misses {sample}"
        );
    }
    assert!(
        !path_roots("let x = a::academic_domain::b;").contains("academic_domain"),
        "the path reader reports a middle segment as a root"
    );
    assert!(
        macros_invoked("include_str! (SOME_PATH)").contains("include_str"),
        "the macro reader misses a macro with whitespace before its bang"
    );
    assert!(
        !macros_invoked("if !(value) { }").contains("if"),
        "the macro reader reads a keyword as a macro"
    );

    // ---- Layer two: every field position, and every declared type ----------
    let positions = field_positions()?;
    assert!(
        positions.len() >= 100,
        "the field reader found only {} positions",
        positions.len()
    );

    let mut inventory: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (owner, name, declared) in FIELD_INVENTORY
        .into_iter()
        .chain(FIELD_INVENTORY_TAIL)
        .chain(FIELD_INVENTORY_TAIL_TWO)
    {
        assert!(
            inventory.insert((owner.to_owned(), name.to_owned(), declared.to_owned())),
            "{owner}.{name} is in the inventory twice"
        );
    }
    let found: BTreeSet<(String, String, String)> = positions
        .iter()
        .map(|position| {
            (
                position.owner.clone(),
                position.name.clone(),
                position.declared.clone(),
            )
        })
        .collect();
    let extra: Vec<_> = found.difference(&inventory).collect();
    assert!(
        extra.is_empty(),
        "a field of this crate is not in the inventory: {extra:?}"
    );
    let missing: Vec<_> = inventory.difference(&found).collect();
    assert!(
        missing.is_empty(),
        "the inventory names a field this crate does not declare: {missing:?}"
    );

    // The declared types, as a whole set, both directions. This is the layer
    // that fails on `Vec<u8>` under any field name, and on a type alias whose
    // own name nobody reviewed.
    let mut used_types: BTreeSet<String> = BTreeSet::new();
    for position in &positions {
        used_types.extend(type_constructors(&position.declared));
    }
    // A path-qualified type contributes its module segments; those are dropped
    // here because a module is not a type, and each is named explicitly so a
    // new one is a review rather than a silent pass.
    for module in ["academic_domain", "academic_proposal", "academic_ingestion"] {
        used_types.remove(module);
    }
    let allowed: BTreeSet<String> = DECLARED_TYPE_ALLOWLIST
        .into_iter()
        .chain(DECLARED_TYPE_ALLOWLIST_TAIL)
        .chain(SCALAR_ALLOWLIST)
        .map(str::to_owned)
        .collect();
    let unreviewed: Vec<_> = used_types.difference(&allowed).collect();
    assert!(
        unreviewed.is_empty(),
        "a field of this crate declares a type nobody reviewed: {unreviewed:?}"
    );
    let dead: Vec<_> = allowed.difference(&used_types).collect();
    assert!(
        dead.is_empty(),
        "the type allowlist names a type no field declares: {dead:?}"
    );
    // The allowlist is not vacuous: the four shapes a payload arrives in are
    // absent from it, so a field declaring one fails above.
    for absent in ["u8", "i8", "String", "str", "Box", "Cow", "Untrusted"] {
        assert!(
            !allowed.contains(absent),
            "{absent} is on the declared-type allowlist"
        );
    }

    // ---- Layer three: the public surface ------------------------------------
    let mut signature_types: BTreeSet<String> = BTreeSet::new();
    let mut signatures = 0_usize;
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for signature in public_signatures(&code) {
            signatures += 1;
            signature_types.extend(signature_type_constructors(&signature));
        }
    }
    assert!(
        signatures >= 60,
        "the signature reader found only {signatures} public functions"
    );
    for module in ["academic_domain", "academic_proposal", "academic_ingestion"] {
        signature_types.remove(module);
    }
    // Nine types appear only in a signature and never in a field, plus the
    // shapes every signature carries: `Self`, `Result`, `bool`, this crate's
    // error, and `str`, which arrives through the eight `&'static str` returns
    // enumerated below. Each is named here rather than exempted by a pattern,
    // so a tenth is a review.
    let signature_extras: BTreeSet<String> = [
        "Actor",
        "CenterError",
        "DependencyGraph",
        "DependentKind",
        "HistoricalView",
        "LivePermission",
        "PermissionKind",
        "Resolution",
        "Result",
        "SectionIndex",
        "Self",
        "SourceDiff",
        "bool",
        "str",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let unreviewed_signature: Vec<_> = signature_types
        .difference(&allowed)
        .filter(|name| !signature_extras.contains(*name))
        .collect();
    assert!(
        unreviewed_signature.is_empty(),
        "a public signature names a type nobody reviewed: {unreviewed_signature:?}"
    );

    // The eight `&'static str` returns are enumerated, and each is a total
    // match over a closed enum. Any ninth fails as an extra key.
    let mut static_returns: BTreeSet<String> = BTreeSet::new();
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let mut current_impl: Option<String> = None;
        for line in code.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("impl ") {
                current_impl = Some(
                    rest.split(|character: char| {
                        !character.is_ascii_alphanumeric() && character != '_'
                    })
                    .find(|segment| !segment.is_empty())
                    .unwrap_or_default()
                    .to_owned(),
                );
            }
            if trimmed.contains("-> &'static str")
                && let Some(rest) = trimmed.split("fn ").nth(1)
            {
                let name: String = rest
                    .chars()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || *character == '_'
                    })
                    .collect();
                let owner = current_impl.clone().unwrap_or_default();
                static_returns.insert(format!("{owner}::{name}"));
            }
        }
    }
    assert_eq!(
        static_returns,
        STATIC_STR_RETURNS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        "the set of functions returning a &'static str changed"
    );

    // ---- Layer four, the weakest: the spellings ------------------------------
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for spelling in WEAKEST_LAYER_SPELLINGS {
            assert!(
                !code.contains(spelling),
                "{} spells {spelling}",
                relative(&path)
            );
        }
        scanned += 1;
    }
    assert!(
        scanned >= 9,
        "the spelling scan read only {scanned} product files"
    );
    // The layer is not vacuous: each rule matches the text it forbids.
    for spelling in WEAKEST_LAYER_SPELLINGS {
        let sample = format!("let value: {spelling} = todo!();");
        assert!(
            sample.contains(spelling),
            "the {spelling} rule matches nothing"
        );
    }

    // ---- No tuple struct, because a tuple field has no name to inventory ----
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for line in code.lines() {
            let trimmed = line.trim();
            if let Some(name) = opened_type(trimmed) {
                assert!(
                    !trimmed.contains('('),
                    "{} declares tuple struct {name}",
                    relative(&path)
                );
            }
        }
    }
    Ok(())
}

/// Every type constructor a signature names, argument names excluded.
///
/// A signature is `name: Type` pairs and a return type. Reading the whole text
/// would report every argument *name* as a type, which is the shape that made
/// the first version of this scan report ninety-odd unreviewed "types" and
/// would have made a real one invisible in the noise.
fn signature_type_constructors(signature: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let Some(open) = signature.find('(') else {
        return found;
    };
    // The argument list, up to the matching close paren.
    let bytes: Vec<char> = signature.chars().collect();
    let mut depth = 0_i32;
    let mut close = open;
    for (index, character) in bytes.iter().enumerate().skip(open) {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = index;
                    break;
                }
            }
            _ => {}
        }
    }
    let arguments: String = bytes[open + 1..close].iter().collect();
    for argument in split_top_level(&arguments) {
        if let Some((_, declared)) = argument.split_once(": ") {
            found.extend(type_constructors(declared));
        }
    }
    let tail: String = bytes[close..].iter().collect();
    if let Some((_, returned)) = tail.split_once("-> ") {
        found.extend(type_constructors(returned));
    }
    found
}

/// Every `pub fn` signature in already-stripped code, whitespace-collapsed.
fn public_signatures(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let flat: Vec<&str> = code.lines().collect();
    for (index, line) in flat.iter().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("pub fn ") || trimmed.starts_with("pub const fn ")) {
            continue;
        }
        let mut signature = String::from(trimmed);
        let mut cursor = index;
        while !signature.contains('{') && !signature.contains(';') && cursor + 1 < flat.len() {
            cursor += 1;
            signature.push(' ');
            signature.push_str(flat[cursor].trim());
        }
        let head = signature
            .split_once('{')
            .map_or(signature.as_str(), |(head, _)| head);
        found.push(collapse(head));
    }
    found
}

// ---------------------------------------------------------------------------
// the_class_of_an_entry_is_its_payloads_type
// ---------------------------------------------------------------------------

/// A class is read off a payload's type and is never parsed from a string.
///
/// The behavioural half is `proposal_inbox_holds_four_typed_classes`. This is
/// the half that says no second route exists: the whole `impl` set naming
/// `ProposalClass` is compared against a pinned list, so a `FromStr`, a
/// `TryFrom` or a `From<&str>` nobody predicted fails as an extra key, and
/// `InboxEntry::class` is pinned as whole text so its four arms cannot be
/// rewired without a review.
#[test]
fn the_class_of_an_entry_is_its_payloads_type() -> TestResult {
    let inbox = fs::read_to_string(crate_root().join("src").join("inbox.rs"))?;
    let code = strip_non_code(&inbox);

    // The whole `impl` set naming the class type.
    let mut headers: BTreeSet<String> = BTreeSet::new();
    for line in code.lines() {
        let trimmed = collapse(line.trim());
        if trimmed.starts_with("impl ") && uses_of(&trimmed, "ProposalClass") > 0 {
            headers.insert(trimmed.trim_end_matches('{').trim().to_owned());
        }
    }
    assert_eq!(
        headers,
        ["impl ProposalClass"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        "an impl naming ProposalClass appeared or disappeared"
    );

    // The derive is the whole of what the type implements besides that block.
    let derive = declared_member(&code, "pub enum ProposalClass {", "}")?;
    assert!(
        derive.starts_with("pub enum ProposalClass { "),
        "the class enum moved"
    );
    for route in [
        "FromStr",
        "TryFrom",
        "From<&str>",
        "from_str",
        "parse",
        "as_class",
    ] {
        assert_eq!(
            uses_of(&code, route),
            0,
            "inbox.rs names {route}, which is a route from text to a class"
        );
    }

    // `class` is pinned whole, so the four arms cannot be rewired silently.
    const WHOLE_CLASS: &str = "pub const fn class(&self) -> ProposalClass { match self { Self::Relation(_) => ProposalClass::Relation, Self::ConceptMerge(_) => ProposalClass::ConceptMerge, Self::ProjectClassification(_) => ProposalClass::ProjectClassification, Self::StateUpdate(_) => ProposalClass::StateUpdate, } }";
    assert_eq!(
        declared_member(&code, "pub const fn class(&self)", "\n    }\n")?,
        WHOLE_CLASS,
        "InboxEntry::class changed"
    );

    // No entry carries a class. The field inventory already says so as a whole
    // set; this says it in the shape a reader of this file is looking for, and
    // it is the assertion that fails first if a discriminant field is added.
    let holders: BTreeSet<String> = field_positions()?
        .into_iter()
        .filter(|position| position.declared == "ProposalClass")
        .map(|position| format!("{}.{}", position.owner, position.name))
        .collect();
    assert_eq!(
        holders,
        [
            "CenterItem::Proposal.#1",
            "CenterError::NoEntryOfClass.class",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        "a class is carried somewhere other than the index reference and the refusal"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// nothing_but_a_user_settles_a_conflict_or_extends_an_expiry
// ---------------------------------------------------------------------------

/// The source half of the two "only a user" claims.
///
/// The behavioural halves are `both_conflict_classes_are_unresolved_until_user_action`
/// and `expiring_permission_is_queued_and_blocks_dependents`. These are the
/// halves that say there is no second route: the one function that appends a
/// correction record is pinned whole, the one function that mints a user
/// receipt is pinned whole, the whole `impl` set naming `LivePermission` is
/// compared against a pinned list, and no signature anywhere takes a permission
/// and moves its expiry.
#[test]
fn nothing_but_a_user_settles_a_conflict_or_extends_an_expiry() -> TestResult {
    let conflict = strip_non_code(&fs::read_to_string(
        crate_root().join("src").join("conflict.rs"),
    )?);
    let permission = strip_non_code(&fs::read_to_string(
        crate_root().join("src").join("permission.rs"),
    )?);

    // ---- The one place a receipt is minted ---------------------------------
    const WHOLE_RECEIPT: &str = "pub fn user_receipt(actor: &Actor) -> Result<UserDecision, CenterError> { UserDecision::by(actor).map_err(|refusal| CenterError::NotTheUser { refusal }) }";
    assert_eq!(
        declared_member(&conflict, "pub fn user_receipt(", "\n}\n")?,
        WHOLE_RECEIPT,
        "the receipt door changed"
    );
    assert_eq!(
        uses_of(&conflict, "UserDecision::by"),
        1,
        "P2-M2's actor door is reached from more than one place"
    );

    // ---- The one place a correction record is appended ----------------------
    assert_eq!(
        uses_of(&conflict, "CorrectionRecord"),
        // the struct declaration, its `impl`, `ConflictCase::history`'s element
        // type, the `history` accessor's return type, and the one push site.
        5,
        "the number of places a correction record is named changed"
    );
    assert_eq!(
        uses_of(&conflict, "history"),
        // the field, its initialiser in `open`, the accessor, the accessor's
        // body, the walk in `resolution`, and the one push site.
        6,
        "the number of places a history is named changed"
    );
    assert_eq!(
        conflict.matches("self.history.push").count(),
        1,
        "a correction record is appended in more than one place"
    );
    assert_eq!(
        uses_of(&conflict, "push"),
        // the history append above, and `ConflictBoard::open` recording a case.
        2,
        "conflict.rs grew a third append"
    );
    const WHOLE_SETTLE: &str = "pub fn settle( &mut self, outcome: CorrectionOutcome, decided_by: UserDecision, decided_at: TimestampMillis, ) { self.history.push(CorrectionRecord { outcome, decided_by, decided_at, }); }";
    assert_eq!(
        declared_member(&conflict, "pub fn settle(", "\n    }\n")?,
        WHOLE_SETTLE,
        "ConflictCase::settle changed"
    );

    // The three choices are a constant, pinned whole.
    //
    // The behavioural half drives the whole status vocabulary on both sides,
    // and `X7-I15` is why: the assertion that used to be there ran on one
    // shape, and a narrowing keyed on another passed it. A sweep is bounded by
    // what it varies, though -- `X7-I27` narrows on the authority class, which
    // the sweep holds fixed -- so the pin is what refuses a narrowing keyed on
    // anything at all.
    const WHOLE_OFFERED: &str =
        "pub const fn offered(&self) -> [CorrectionChoice; 3] { CorrectionChoice::ALL }";
    assert_eq!(
        declared_member(&conflict, "pub const fn offered(", TAIL_BRACE)?,
        WHOLE_OFFERED,
        "ConflictCase::offered changed"
    );

    // Nothing removes, truncates or clears a history, and nothing resolves one
    // without a record.
    for absent in [
        "auto_resolve",
        "fn resolve",
        "truncate",
        "clear",
        "retain",
        "remove",
        "pop",
        "drain",
        "expire",
    ] {
        assert_eq!(
            uses_of(&conflict, absent),
            0,
            "conflict.rs names {absent}"
        );
    }
    // `Resolution` is computed rather than stored, so there is no field to set.
    for position in field_positions()? {
        assert!(
            position.declared != "Resolution",
            "{}.{} stores a resolution",
            position.owner,
            position.name
        );
    }

    // ---- A live permission has exactly one producer ------------------------
    let mut headers: BTreeSet<String> = BTreeSet::new();
    for line in permission.lines() {
        let trimmed = collapse(line.trim());
        if trimmed.starts_with("impl ") && uses_of(&trimmed, "LivePermission") > 0 {
            headers.insert(trimmed.trim_end_matches('{').trim().to_owned());
        }
    }
    assert_eq!(
        headers,
        ["impl LivePermission"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        "an impl naming LivePermission appeared or disappeared"
    );
    let constructions = permission
        .lines()
        .filter(|line| line.contains("LivePermission {"))
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("pub struct ") && !trimmed.starts_with("impl ")
        })
        .count();
    assert_eq!(
        constructions, 1,
        "a live permission is built in more than one place"
    );
    // The subtraction is not a hole: the declaration and the impl are exactly
    // the two sites removed, and both are pinned by the header comparison
    // above, so a third site written as either shape fails there instead.
    assert_eq!(
        permission.matches("LivePermission {").count(),
        3,
        "the number of places LivePermission is named with a brace changed"
    );
    const WHOLE_HAS_LAPSED: &str =
        "pub fn has_lapsed(&self, at: TimestampMillis) -> bool { self.expires_at <= at }";
    assert_eq!(
        declared_member(&permission, "pub fn has_lapsed(", "\n    }\n")?,
        WHOLE_HAS_LAPSED,
        "the expiry comparison changed; fail-closed at the expiry instant is what it holds"
    );

    // ---- Nothing extends an expiry -----------------------------------------
    for absent in ["renew", "extend", "refresh", "prolong", "set_expires"] {
        assert_eq!(
            uses_of(&permission, absent),
            0,
            "permission.rs names {absent}"
        );
    }
    // And no public signature takes a `&mut self` on a permission at all.
    for signature in public_signatures(&permission) {
        if signature.contains("&mut self") {
            assert!(
                signature.contains("fn record(")
                    || signature.contains("fn register_dependent("),
                "a permission has a mutating door beside recording: {signature}"
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// the_provider_and_receipt_vocabularies_are_the_brokers_own
// ---------------------------------------------------------------------------

/// The two surfaces and the four receipt fields are `P2-G1`/`P2-G3`'s own.
///
/// This crate does not link `academic-policy` — that is the point of layer one
/// above — so the comparison is against that crate's source text rather than
/// against its types. What it buys is that a token renamed there fails here,
/// which is the drift a restatement would otherwise hide.
#[test]
fn the_provider_and_receipt_vocabularies_are_the_brokers_own() -> TestResult {
    let broker = fs::read_to_string(
        workspace_root()
            .join("crates")
            .join("policy")
            .join("src")
            .join("provider.rs"),
    )?;
    assert!(
        broker.len() > 10_000,
        "the broker source read is too short to be the provider registry"
    );

    // The two contract surfaces.
    for surface in ProviderSurface::ALL {
        assert!(
            broker.contains(surface.as_str()),
            "the broker does not name the surface {}",
            surface.as_str()
        );
    }
    // And it names no third, which is what makes the two a whole set.
    let declared = broker
        .find("pub enum ProviderSurface {")
        .ok_or("the broker declares no ProviderSurface")?;
    let body_end = broker[declared..]
        .find('}')
        .ok_or("the broker's ProviderSurface has no body")?;
    let body = &broker[declared..declared + body_end];
    let arms = body
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("pub enum")
                && !trimmed.starts_with('#')
        })
        .count();
    assert_eq!(
        arms,
        ProviderSurface::ALL.len(),
        "the broker's ProviderSurface has {arms} arms and this crate names {}",
        ProviderSurface::ALL.len()
    );

    // The receipt reference carries the broker's own columns, less the two
    // identifier strings this crate deliberately does not hold.
    for column in [
        "provider_policy_snapshot_digest",
        "provider_receipt_digest",
        "requested_at",
        "received_at",
    ] {
        assert!(
            broker.contains(column),
            "the broker's DeletionReceiptRow has no {column} column"
        );
    }
    // The two it does not carry, and the reason: both are `String`, and a
    // `String` field is what the type allowlist above refuses.
    for absent in ["receipt_id", "grant_id"] {
        assert!(
            broker.contains(absent),
            "the broker's DeletionReceiptRow has no {absent} column any more"
        );
        let ours = fs::read_to_string(crate_root().join("src").join("transmission.rs"))?;
        assert_eq!(
            uses_of(&strip_non_code(&ours), absent),
            0,
            "the centre carries the broker's {absent} column, which is a String"
        );
    }
    Ok(())
}
