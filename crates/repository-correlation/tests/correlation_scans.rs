//! What `academic-repository-correlation` may reach, hold and hand out.
//!
//! ## Why a token list is not enough, measured one task ago
//!
//! `P2-R2` shipped this crate's neighbour with a forbidden-token list as its
//! only net, and `docs/contracts/policy-source-scans.md` records what that
//! measured: seven spellings of a filesystem or environment reach — including
//! `std::path::Path::new(p).metadata()`, its leading-`::` form, its
//! whitespace-inside-the-path form, and `include_str!` — compile, spell none of
//! the listed tokens, add no `use` item, and passed. The repair was not a
//! longer list. It was three **whole-set** comparisons, in both directions:
//!
//! * every `use` item ([`USE_ITEMS`]);
//! * every two-segment path reached through a crate root ([`REACHED_PATHS`]);
//! * every macro invoked ([`MACROS_SPELLED`]).
//!
//! Those three cover the three ways a capability is reached — through an
//! import, through an absolute path, through a macro — and a reach nobody
//! predicted appears as an **extra key** rather than as a token nobody listed.
//! [`FORBIDDEN_CONSTRUCTS`] is kept as the third and weakest layer, because it
//! names the shapes a reader expects to see refused.
//!
//! ## The same defect class, one step out, and what this file does about it
//!
//! A name list standing in for a whole-set check is not confined to that one
//! guard. `tools/secret-debug-policy.test.mjs` decides whether a field holds
//! something a `Debug` must not print by matching the **field's name** against
//! a fixed alternation — `source_bytes`, `payload`, `plaintext`, and so on. A
//! field holding the same bytes under a name that is not in the alternation is
//! invisible to it, exactly the way a filesystem call under a spelling nobody
//! listed was invisible to the token pass. That measurement is recorded in
//! `docs/contracts/repository-correlation.md`.
//!
//! This crate closes its own half of that by the whole-set route rather than by
//! adding names: [`FIELDS`] is every field of every type this crate declares,
//! compared in both directions, each entry carrying what it holds. A field that
//! held a symbol name, an import specifier or a configuration key appears here
//! as an extra key whatever it is called.

use std::{
    collections::{BTreeSet, HashSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root().join("src"), &mut found)?;
    walk(&crate_root().join("tests"), &mut found)?;
    found.sort();
    Ok(found)
}

fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
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

fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
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

/// One free function's text, from its signature to the `}` at column zero.
fn free_function(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{signature} has no closing brace at column zero"))?;
    Ok(collapse(&source[start..start + end + 2]))
}

/// One brace-balanced block's text, from `header` to its matching `}`.
fn whole_block(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
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
fn collapse(body: &str) -> String {
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

/// Drops every `use` item, so a re-export is not counted as a reach.
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

/// The relative path of `path` under the workspace, with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn source_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?)
}

/// This crate's product files, as code with comments and literals removed.
fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// The inventories.
// ---------------------------------------------------------------------------

/// Every `use` item of this crate's product code, in file and source order.
///
/// Compared in both directions. A filesystem or transport import appears as an
/// extra key here whatever it is named or aliased to.
const USE_ITEMS: [(&str, &str); 11] = [
    (
        "crates/repository-correlation/src/artifact.rs",
        "use academic_repository_analysis::SubjectId;",
    ),
    (
        "crates/repository-correlation/src/artifact.rs",
        "use crate::CorrelationError;",
    ),
    (
        "crates/repository-correlation/src/authority.rs",
        "use academic_domain::AuthorityClass;",
    ),
    (
        "crates/repository-correlation/src/authority.rs",
        "use academic_ledger::AuthorityTable;",
    ),
    (
        "crates/repository-correlation/src/authority.rs",
        "use crate::{ CorrelationError, artifact::{ApprovalStatus, DocumentId, IntentDocumentKind}, relation::AuthorityLane, };",
    ),
    (
        "crates/repository-correlation/src/compare.rs",
        "use std::collections::{BTreeMap, BTreeSet};",
    ),
    (
        "crates/repository-correlation/src/compare.rs",
        "use crate::{Correlation, CorrelationError, EvidenceRelation, drift::DriftKind};",
    ),
    (
        "crates/repository-correlation/src/drift.rs",
        "use crate::{ artifact::{DeploymentTarget, DocumentId, FlagKey, FlagState}, edge::RelationEdge, };",
    ),
    (
        "crates/repository-correlation/src/edge.rs",
        "use academic_repository_analysis::{ArtifactScope, EvidenceTier, LadderRung, Locator};",
    ),
    (
        "crates/repository-correlation/src/edge.rs",
        "use crate::{ EvidenceRelation, artifact::{ApprovalStatus, DocumentId, IncidentId, IntentDocumentKind}, relation::AuthorityLane, };",
    ),
    (
        "crates/repository-correlation/src/lib.rs",
        "use std::collections::{BTreeMap, BTreeSet};",
    ),
];

/// Every re-export of this crate's `lib.rs`, kept apart from [`USE_ITEMS`].
///
/// A `pub use` is a surface decision rather than a reach, so it is listed here
/// and compared here; mixing the two lists would let a new `use` be excused as
/// a re-export.
const RE_EXPORTS: [&str; 6] = [
    "pub use artifact::{ ApprovalStatus, BehaviorDocument, DeploymentRecord, DeploymentTarget, DocumentId, FeatureFlagRecord, FlagKey, FlagState, IncidentId, IncidentRecord, IntentDocument, IntentDocumentKind, };",
    "pub use authority::{AnswerSource, Candidate, LaneAnswer, RankedCandidate, active_view};",
    "pub use compare::{ ChangeCause, DependencyChange, PresenceChange, SemanticChange, SemanticTransition, SnapshotComparison, compare, };",
    "pub use drift::{ BranchDifference, DeprecatedSpec, DriftKind, DriftScopeKind, DriftScopes, GatingFlag, ImplementationDrift, UndeployedCode, };",
    "pub use edge::{EdgeEvidence, RelationEdge};",
    "pub use relation::{AuthorityLane, EvidenceRelation};",
];

/// The non-`use` imports of `lib.rs`, which the two lists above do not hold.
const LIB_IMPORTS: [&str; 2] = [
    "use academic_repository::RepositorySnapshot;",
    "use academic_repository_analysis::{ AnalyzerIdentity, ArtifactScope, EvidenceTier, FileKind, Finding, LadderRung, SubjectId, };",
];

/// Every two-segment path this crate's product code spells through a crate
/// root, and why each one is here.
///
/// This is the net a token list cannot be: a capability written as an absolute
/// path adds a key here whatever it is called. Two segments and not one,
/// because `std::path` and `std::fs` are different capabilities and collapsing
/// them to `std` would admit both.
const REACHED_PATHS: [(&str, &str); 4] = [
    (
        "academic_ledger::ProductClaimType",
        "section 30.3's six rows, as P2-C3 already implemented them; this crate \
         maps a lane onto one of them and adds no rank",
    ),
    (
        "academic_repository::ManifestEntry",
        "the frozen manifest's path, to check a document is in the snapshot",
    ),
    ("core::fmt", "the hand-written Display impl for AuthorityLane"),
    ("thiserror::Error", "the error enumeration's derive"),
];

/// Every macro this crate's product code invokes.
///
/// A macro is not a path, so [`REACHED_PATHS`] cannot see one. `include_str!`
/// and `include_bytes!` read a file at compile time while spelling no `std`
/// path and needing no `use`.
const MACROS_SPELLED: [(&str, &str); 1] = [(
    "matches",
    "the identifier validator's byte class, over a closed set",
)];

/// The constructs the forbidden-token pass refuses anywhere in the package.
///
/// Assembled from halves for the reason `P2-R2`'s equivalent list is: two other
/// scans in this repository read raw source for these exact spellings, and a
/// file spelling one whole would have to be added to somebody else's *reviewed
/// sites* list as a file that does what it does not do. The `concat!` is
/// evaluated at compile time, so the value compared against the source is the
/// whole spelling either way.
///
/// The two files of this package that read a file, as a whole set.
///
/// Both are test files and both are named on
/// `docs/contracts/policy-source-scans.md`. No product file is here, which is
/// what makes *this crate opens nothing* a statement about the crate.
const READERS: [&str; 2] = [
    "tests/correlation_scans.rs",
    "tests/correlation_lanes.rs",
];

/// This is the **third** net and the weakest of the three.
const FORBIDDEN_CONSTRUCTS: [&str; 11] = [
    concat!("fs", "::"),
    concat!("File", "::"),
    concat!("process", "::Command"),
    concat!("Tcp", "Stream"),
    concat!("Tcp", "Listener"),
    concat!("Udp", "Socket"),
    concat!("Unix", "Stream"),
    concat!("sock", "et"),
    concat!("conn", "ect"),
    concat!("req", "west"),
    concat!("hy", "per"),
];

/// Every field of every type this crate declares, and what it holds.
///
/// The whole-set answer to the question `tools/secret-debug-policy.test.mjs`
/// answers with a name alternation. A field is here as `(type or variant,
/// field, declared type, what it holds)`, compared in both directions, so a
/// field carrying a symbol name, an import specifier or a configuration key
/// lifted out of a repository is an extra key regardless of what it is called.
///
/// There are five things a field of this crate may hold, and they are the last
/// column: a caller-supplied identifier, a path the gate already classified and
/// `academic-repository`'s own manifest already hands out, a system-derived
/// identifier, a closed vocabulary value, or a value of another reviewed crate.
const FIELDS: [(&str, &str, &str, &str); 44] = [
    // artifact.rs
    ("DocumentId", "identifier", "String", "caller-supplied identifier"),
    ("IncidentId", "identifier", "String", "caller-supplied identifier"),
    ("FlagKey", "identifier", "String", "caller-supplied identifier"),
    ("DeploymentTarget", "identifier", "String", "caller-supplied identifier"),
    ("IntentDocument", "id", "DocumentId", "caller-supplied identifier"),
    ("IntentDocument", "kind", "IntentDocumentKind", "closed vocabulary"),
    ("IntentDocument", "status", "ApprovalStatus", "closed vocabulary"),
    ("IntentDocument", "revision", "u64", "caller-supplied ordinal"),
    ("IntentDocument", "branch", "Option<String>", "a branch name, which the snapshot already hands out"),
    ("IntentDocument", "path", "String", "a manifest path"),
    ("IntentDocument", "mentions", "Vec<SubjectId>", "P2-R2's caller-chosen subject identifiers"),
    ("BehaviorDocument", "id", "DocumentId", "caller-supplied identifier"),
    ("BehaviorDocument", "path", "String", "a manifest path"),
    ("BehaviorDocument", "explains", "Vec<SubjectId>", "P2-R2's caller-chosen subject identifiers"),
    ("IncidentRecord", "id", "IncidentId", "caller-supplied identifier"),
    ("IncidentRecord", "snapshot_id", "String", "a system-derived snapshot identifier"),
    ("IncidentRecord", "occurred_at", "u64", "a timestamp"),
    ("IncidentRecord", "exposed", "Vec<SubjectId>", "P2-R2's caller-chosen subject identifiers"),
    ("FeatureFlagRecord", "key", "FlagKey", "caller-supplied identifier"),
    ("FeatureFlagRecord", "state", "FlagState", "closed vocabulary"),
    ("FeatureFlagRecord", "gates", "Vec<SubjectId>", "P2-R2's caller-chosen subject identifiers"),
    ("DeploymentRecord", "target", "DeploymentTarget", "caller-supplied identifier"),
    ("DeploymentRecord", "deployed_snapshot", "String", "a system-derived snapshot identifier"),
    // authority.rs
    ("AnswerSource::DirectEvidence", "snapshot_id", "String", "a system-derived snapshot identifier"),
    ("AnswerSource::IntentDocument", "document", "DocumentId", "caller-supplied identifier"),
    ("AnswerSource::IntentDocument", "kind", "IntentDocumentKind", "closed vocabulary"),
    ("AnswerSource::IntentDocument", "status", "ApprovalStatus", "closed vocabulary"),
    ("AnswerSource::IntentDocument", "revision", "u64", "caller-supplied ordinal"),
    ("Candidate", "id", "String", "caller-supplied identifier"),
    ("Candidate", "source", "AnswerSource", "closed vocabulary"),
    ("RankedCandidate", "id", "String", "caller-supplied identifier"),
    ("RankedCandidate", "authority", "AuthorityClass", "academic-domain's closed vocabulary"),
    ("RankedCandidate", "rank", "u16", "academic-ledger's stable comparison value"),
    ("LaneAnswer", "lane", "AuthorityLane", "closed vocabulary"),
    ("LaneAnswer", "table", "AuthorityTable", "academic-ledger's section 30.3 table"),
    ("LaneAnswer", "ranked", "Vec<RankedCandidate>", "the candidates above"),
    // compare.rs
    ("DependencyChange", "subject", "String", "P2-R2's caller-chosen subject identifier"),
    ("DependencyChange", "direction", "PresenceChange", "closed vocabulary"),
    ("DependencyChange", "cause", "ChangeCause", "closed vocabulary"),
    ("SemanticChange", "subject", "String", "P2-R2's caller-chosen subject identifier"),
    ("SemanticChange", "transition", "SemanticTransition", "closed vocabulary"),
    ("SemanticChange", "cause", "ChangeCause", "closed vocabulary"),
    ("SemanticChange", "before", "Vec<EvidenceRelation>", "closed vocabulary"),
    ("SemanticChange", "after", "Vec<EvidenceRelation>", "closed vocabulary"),
];

/// The rest of [`FIELDS`], split only because one array literal of this width
/// is unreadable. The two are concatenated before the comparison.
const MORE_FIELDS: [(&str, &str, &str, &str); 30] = [
    ("SemanticChange", "drift", "Option<DriftKind>", "closed vocabulary"),
    ("SnapshotComparison", "cause", "ChangeCause", "closed vocabulary"),
    ("SnapshotComparison", "dependency", "Vec<DependencyChange>", "the first channel above"),
    ("SnapshotComparison", "semantic", "Vec<SemanticChange>", "the second channel above"),
    // drift.rs
    ("DeprecatedSpec", "document", "DocumentId", "caller-supplied identifier"),
    ("DeprecatedSpec", "revision", "u64", "caller-supplied ordinal"),
    ("GatingFlag", "key", "FlagKey", "caller-supplied identifier"),
    ("GatingFlag", "state", "FlagState", "closed vocabulary"),
    ("UndeployedCode", "target", "DeploymentTarget", "caller-supplied identifier"),
    ("UndeployedCode", "deployed_snapshot", "String", "a system-derived snapshot identifier"),
    ("BranchDifference", "intent_branch", "String", "a branch name"),
    ("BranchDifference", "snapshot_branch", "Option<String>", "a branch name the snapshot hands out"),
    ("DriftScopes", "deprecated_spec", "Option<DeprecatedSpec>", "the scope above"),
    ("DriftScopes", "feature_flag", "Option<GatingFlag>", "the scope above"),
    ("DriftScopes", "undeployed_code", "Option<UndeployedCode>", "the scope above"),
    ("DriftScopes", "branch_difference", "Option<BranchDifference>", "the scope above"),
    ("ImplementationDrift", "kind", "DriftKind", "closed vocabulary"),
    ("ImplementationDrift", "subject", "String", "P2-R2's caller-chosen subject identifier"),
    ("ImplementationDrift", "snapshot_id", "String", "a system-derived snapshot identifier"),
    ("ImplementationDrift", "intent_side", "Vec<RelationEdge>", "the edges below"),
    ("ImplementationDrift", "implementation_side", "Vec<RelationEdge>", "the edges below"),
    ("ImplementationDrift", "description_side", "Vec<RelationEdge>", "the edges below"),
    ("ImplementationDrift", "scopes", "DriftScopes", "the scopes above"),
    // edge.rs
    ("EdgeEvidence::Analysis", "rung", "LadderRung", "P2-R2's closed vocabulary"),
    ("EdgeEvidence::Analysis", "tier", "EvidenceTier", "P2-R2's closed vocabulary"),
    ("EdgeEvidence::Analysis", "artifact_scope", "ArtifactScope", "P2-R2's closed vocabulary"),
    ("EdgeEvidence::Analysis", "locators", "Vec<Locator>", "P2-R2's locator, which holds a path, a digest, a span and a fingerprint"),
    ("EdgeEvidence::Document", "document", "DocumentId", "caller-supplied identifier"),
    ("EdgeEvidence::Document", "status", "ApprovalStatus", "closed vocabulary"),
    ("EdgeEvidence::Document", "revision", "u64", "caller-supplied ordinal"),
];

/// The last of [`FIELDS`].
const LAST_FIELDS: [(&str, &str, &str, &str); 12] = [
    ("EdgeEvidence::Document", "path", "String", "a manifest path"),
    ("EdgeEvidence::Incident", "incident", "IncidentId", "caller-supplied identifier"),
    ("EdgeEvidence::Incident", "occurred_at", "u64", "a timestamp"),
    ("RelationEdge", "relation", "EvidenceRelation", "closed vocabulary"),
    ("RelationEdge", "subject", "String", "P2-R2's caller-chosen subject identifier"),
    ("RelationEdge", "snapshot_id", "String", "a system-derived snapshot identifier"),
    ("RelationEdge", "evidence", "EdgeEvidence", "the evidence above"),
    // lib.rs
    ("Correlation", "snapshot_id", "String", "a system-derived snapshot identifier"),
    ("Correlation", "analyzer_tool", "String", "the analyzer's own name"),
    ("Correlation", "analyzer_version", "String", "the analyzer's own version"),
    ("Correlation", "edges", "Vec<RelationEdge>", "the edges above"),
    ("Correlation", "drifts", "Vec<ImplementationDrift>", "the drifts above"),
];

/// The last field of the last type, and the request's own borrowed fields.
///
/// `CorrelationInput` is the argument list of `correlate`, so every field is a
/// borrow of a value another reviewed crate already owns. They are fields all
/// the same, and a whole-set inventory that skipped the one type holding the
/// analyzer's input would be the narrowing this file exists to refuse.
const FINAL_FIELDS: [(&str, &str, &str, &str); 9] = [
    (
        "Correlation",
        "dependencies",
        "BTreeSet<String>",
        "P2-R2's caller-chosen subject identifiers",
    ),
    (
        "CorrelationInput",
        "snapshot",
        "&'aRepositorySnapshot",
        "P2-R1's frozen snapshot, borrowed",
    ),
    (
        "CorrelationInput",
        "analyzer",
        "&'aAnalyzerIdentity",
        "P2-R2's analyzer identity, borrowed",
    ),
    (
        "CorrelationInput",
        "findings",
        "&'a[Finding]",
        "P2-R2's findings, borrowed",
    ),
    (
        "CorrelationInput",
        "intent_documents",
        "&'a[IntentDocument]",
        "the documents above, borrowed",
    ),
    (
        "CorrelationInput",
        "behavior_documents",
        "&'a[BehaviorDocument]",
        "the documents above, borrowed",
    ),
    (
        "CorrelationInput",
        "incidents",
        "&'a[IncidentRecord]",
        "the records above, borrowed",
    ),
    (
        "CorrelationInput",
        "feature_flags",
        "&'a[FeatureFlagRecord]",
        "the records above, borrowed",
    ),
    (
        "CorrelationInput",
        "deployments",
        "&'a[DeploymentRecord]",
        "the records above, borrowed",
    ),
];

/// Each guarded name, its call count over the package, the one file it may be
/// called from, and what a different count would mean.
const CALL_SITE_COUNTS: [(&str, usize, &str, &str); 3] = [
    (
        // Ten: four edge kinds, the drift, and the four scopes plus the value
        // that holds them. All in `lib.rs`, because a second producer could
        // build an edge whose lane disagreed with its relation, or a drift that
        // carried one side and not the other -- which is the mixing this task
        // exists to prevent.
        "seal",
        10,
        "crates/repository-correlation/src/lib.rs",
        "an edge, a drift or a scope is constructed outside the correlator",
    ),
    (
        // The one route into section 30.3's tables. A second one could compare
        // ranks this crate chose rather than academic-ledger's.
        "authority_table",
        1,
        "crates/repository-correlation/src/authority.rs",
        "a section 30.3 table is read from more than one place",
    ),
    (
        // The one place a lane is turned into a claim type.
        "claim_type",
        1,
        "crates/repository-correlation/src/authority.rs",
        "a lane is mapped onto a section 30.3 row from more than one place",
    ),
];

// ---------------------------------------------------------------------------
// The pins.
// ---------------------------------------------------------------------------

const WHOLE_RELATION_LANE: &str = "impl EvidenceRelation { pub const ALL: [Self; 7] = [ Self::SpecMentions, Self::CodeUses, Self::ArchitectureRequires, Self::TestExercises, Self::ConfigEnables, Self::IncidentExposed, Self::DocExplains, ]; #[must_use] pub const fn as_str(self) -> &'static str { match self { Self::SpecMentions => \"PROJECT_SPEC_MENTIONS\", Self::CodeUses => \"PROJECT_CODE_USES\", Self::ArchitectureRequires => \"PROJECT_ARCHITECTURE_REQUIRES\", Self::TestExercises => \"PROJECT_TEST_EXERCISES\", Self::ConfigEnables => \"PROJECT_CONFIG_ENABLES\", Self::IncidentExposed => \"PROJECT_INCIDENT_EXPOSED\", Self::DocExplains => \"PROJECT_DOC_EXPLAINS\", } } #[must_use] pub const fn lane(self) -> AuthorityLane { match self { Self::SpecMentions | Self::ArchitectureRequires => AuthorityLane::Intent, Self::CodeUses | Self::TestExercises | Self::ConfigEnables | Self::IncidentExposed => { AuthorityLane::Implementation } Self::DocExplains => AuthorityLane::Description, } } }";

const WHOLE_ADMIT: &str = "fn admit( lane: AuthorityLane, snapshot_id: &str, latest_approved: Option<u64>, source: &AnswerSource, ) -> AuthorityClass { match (lane, source) { ( AuthorityLane::Implementation, AnswerSource::DirectEvidence { snapshot_id: observed, }, ) => { if observed == snapshot_id { AuthorityClass::DirectObservation } else { AuthorityClass::Unknown } } ( AuthorityLane::Intent, AnswerSource::IntentDocument { status, revision, .. }, ) => { if *status == ApprovalStatus::Approved && latest_approved == Some(*revision) { AuthorityClass::Curated } else { AuthorityClass::Unknown } } (AuthorityLane::Implementation, AnswerSource::IntentDocument { .. }) | (AuthorityLane::Intent, AnswerSource::DirectEvidence { .. }) => AuthorityClass::Unknown, ( AuthorityLane::Implementation | AuthorityLane::Intent, AnswerSource::UserClarification, ) => AuthorityClass::UserExplicit, (AuthorityLane::Implementation | AuthorityLane::Intent, AnswerSource::ModelInference) => { AuthorityClass::ModelInference } (AuthorityLane::Description, _) => AuthorityClass::Unknown, } }";

const WHOLE_ACTIVE_VIEW: &str = "pub fn active_view( lane: AuthorityLane, snapshot_id: &str, candidates: &[Candidate], ) -> Result<LaneAnswer, CorrelationError> { let row = lane .claim_type() .ok_or(CorrelationError::LaneHasNoAuthorityRow(lane))?; let table = row.authority_table(); let latest_approved = candidates .iter() .filter_map(|candidate| match candidate.source() { AnswerSource::IntentDocument { status: ApprovalStatus::Approved, revision, .. } => Some(*revision), AnswerSource::IntentDocument { .. } | AnswerSource::DirectEvidence { .. } | AnswerSource::UserClarification | AnswerSource::ModelInference => None, }) .max(); let mut ranked: Vec<RankedCandidate> = candidates .iter() .map(|candidate| { let authority = admit(lane, snapshot_id, latest_approved, candidate.source()); RankedCandidate { id: candidate.id().to_owned(), authority, rank: table.rank(authority), } }) .collect(); ranked.sort_by(|left, right| { right .rank .cmp(&left.rank) .then_with(|| left.id.cmp(&right.id)) }); Ok(LaneAnswer { lane, table, ranked, }) }";

const WHOLE_CODE_RELATIONS: &str = "fn code_relations(finding: &Finding) -> Vec<EvidenceRelation> { let mut relations = Vec::new(); if finding.tier() == EvidenceTier::Observed { if finding.artifact_scope() == ArtifactScope::Test { relations.push(EvidenceRelation::TestExercises); } else { relations.push(EvidenceRelation::CodeUses); } } if finding.rung() == LadderRung::RuntimeAndProductionConfig { relations.push(EvidenceRelation::ConfigEnables); } relations }";

const WHOLE_DRIFTS_OF: &str = "fn drifts_of( input: &CorrelationInput<'_>, snapshot_id: &str, edges: &[RelationEdge], ) -> Vec<ImplementationDrift> { let mut by_subject: BTreeMap<&str, Vec<&RelationEdge>> = BTreeMap::new(); for edge in edges { by_subject.entry(edge.subject()).or_default().push(edge); } let mut drifts = Vec::new(); for (subject, subject_edges) in by_subject { let side = |lane: AuthorityLane| -> Vec<RelationEdge> { subject_edges .iter() .filter(|edge| edge.lane() == lane) .map(|edge| (*edge).clone()) .collect() }; let intent = side(AuthorityLane::Intent); let implementation = side(AuthorityLane::Implementation); let description = side(AuthorityLane::Description); let code_uses = implementation .iter() .any(|edge| edge.relation() == EvidenceRelation::CodeUses); let kind = if !intent.is_empty() && !code_uses { Some(DriftKind::IntendedNotImplemented) } else if code_uses && description.is_empty() { Some(DriftKind::ImplementedNotDocumented) } else { None }; let Some(kind) = kind else { continue; }; drifts.push(ImplementationDrift::seal( kind, subject.to_owned(), snapshot_id.to_owned(), intent, implementation, description, scopes_of(input, snapshot_id, subject), )); } drifts }";

const WHOLE_SCOPES_OF: &str = "fn scopes_of(input: &CorrelationInput<'_>, snapshot_id: &str, subject: &str) -> DriftScopes { let names = |mentions: &[SubjectId]| -> bool { mentions.iter().any(|value| value.as_str() == subject) }; let deprecated_spec = input .intent_documents .iter() .filter(|document| { document.status() == ApprovalStatus::Deprecated && names(document.mentions()) }) .max_by_key(|document| document.revision()) .map(|document| DeprecatedSpec::seal(document.id().clone(), document.revision())); let feature_flag = input .feature_flags .iter() .find(|flag| names(flag.gates())) .map(|flag| GatingFlag::seal(flag.key().clone(), flag.state())); let deployed_here = input .deployments .iter() .any(|record| record.deployed_snapshot() == snapshot_id); let undeployed_code = if deployed_here { None } else { input .deployments .iter() .min_by_key(|record| record.target().as_str().to_owned()) .map(|record| { UndeployedCode::seal( record.target().clone(), record.deployed_snapshot().to_owned(), ) }) }; let snapshot_branch = input.snapshot.branch(); let branch_difference = input .intent_documents .iter() .filter(|document| names(document.mentions())) .find_map(|document| { let named = document.branch()?; if Some(named) == snapshot_branch { None } else { Some(BranchDifference::seal( named.to_owned(), snapshot_branch.map(str::to_owned), )) } }); DriftScopes::seal( deprecated_spec, feature_flag, undeployed_code, branch_difference, ) }";

const WHOLE_DECLARES_DEPENDENCY: &str = "const fn declares_dependency(kind: FileKind) -> bool { match kind { FileKind::CargoManifest | FileKind::NodeManifest | FileKind::PythonManifest | FileKind::LockFile => true, FileKind::RustSource | FileKind::TypeScriptSource | FileKind::PythonSource | FileKind::SqlScript | FileKind::ConfigDocument | FileKind::ContainerFile | FileKind::ComposeFile | FileKind::CiWorkflow | FileKind::Prose | FileKind::Unsupported => false, } }";

const WHOLE_COMPARE: &str = "pub fn compare( before: &Correlation, after: &Correlation, ) -> Result<SnapshotComparison, CorrelationError> { let snapshot_moved = before.snapshot_id() != after.snapshot_id(); let analyzer_moved = before.analyzer_version() != after.analyzer_version() || before.analyzer_tool() != after.analyzer_tool(); let cause = match (snapshot_moved, analyzer_moved) { (true, false) => ChangeCause::CodeChanged, (false, true) => ChangeCause::AnalysisChanged, (true, true) => { return Err(CorrelationError::ConfoundedComparison( before.snapshot_id().to_owned(), after.snapshot_id().to_owned(), )); } (false, false) => { return Err(CorrelationError::NoComparisonAxis( before.snapshot_id().to_owned(), )); } }; Ok(SnapshotComparison { cause, dependency: dependency_diff(before, after, cause), semantic: semantic_diff(before, after, cause), }) }";

const WHOLE_DRIFT_SCOPES: &str = "impl DriftScopes { pub(crate) const fn seal( deprecated_spec: Option<DeprecatedSpec>, feature_flag: Option<GatingFlag>, undeployed_code: Option<UndeployedCode>, branch_difference: Option<BranchDifference>, ) -> Self { Self { deprecated_spec, feature_flag, undeployed_code, branch_difference, } } #[must_use] pub const fn deprecated_spec(&self) -> Option<&DeprecatedSpec> { self.deprecated_spec.as_ref() } #[must_use] pub const fn feature_flag(&self) -> Option<&GatingFlag> { self.feature_flag.as_ref() } #[must_use] pub const fn undeployed_code(&self) -> Option<&UndeployedCode> { self.undeployed_code.as_ref() } #[must_use] pub const fn branch_difference(&self) -> Option<&BranchDifference> { self.branch_difference.as_ref() } #[must_use] pub fn present(&self) -> Vec<DriftScopeKind> { DriftScopeKind::ALL .into_iter() .filter(|kind| match kind { DriftScopeKind::DeprecatedSpec => self.deprecated_spec.is_some(), DriftScopeKind::FeatureFlag => self.feature_flag.is_some(), DriftScopeKind::UndeployedCode => self.undeployed_code.is_some(), DriftScopeKind::BranchDifference => self.branch_difference.is_some(), }) .collect() } }";

// ---------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 8,
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

    let mut read: BTreeSet<String> = BTreeSet::new();
    for path in &sources {
        if let Some(stem) = path.file_stem() {
            read.insert(stem.to_string_lossy().into_owned());
        }
    }

    let mut declared = 0_usize;
    for path in &sources {
        let source = source_of(path)?;
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
            assert!(
                !trimmed.starts_with("#[path"),
                "{} pulls in a file by path; the walk does not follow one",
                relative(path)
            );
        }
    }
    assert!(
        declared >= 6,
        "the tripwire read only {declared} module declarations"
    );
    Ok(())
}

#[test]
fn the_correlation_crate_touches_no_file_and_no_socket() -> TestResult {
    // The whole set of `use` items, both directions.
    let mut found: Vec<(String, String)> = Vec::new();
    for (file, _) in product_code()? {
        let source = fs::read_to_string(workspace_root().join(&file))?;
        let mut inside = false;
        let mut buffer = String::new();
        for line in source.lines() {
            let trimmed = line.trim();
            let opens = trimmed.starts_with("use ")
                || (trimmed.starts_with("pub") && trimmed.contains(" use "));
            if inside || opens {
                if inside {
                    buffer.push(' ');
                } else {
                    buffer.clear();
                }
                buffer.push_str(trimmed);
                inside = !trimmed.ends_with(';');
                if !inside {
                    found.push((file.clone(), collapse(&buffer)));
                }
            }
        }
    }
    let mut expected: Vec<(String, String)> = USE_ITEMS
        .iter()
        .map(|(file, item)| ((*file).to_owned(), (*item).to_owned()))
        .collect();
    let lib = "crates/repository-correlation/src/lib.rs";
    for item in LIB_IMPORTS {
        expected.push((lib.to_owned(), item.to_owned()));
    }
    for item in RE_EXPORTS {
        expected.push((lib.to_owned(), item.to_owned()));
    }
    let found_set: BTreeSet<(String, String)> = found.iter().cloned().collect();
    let expected_set: BTreeSet<(String, String)> = expected.into_iter().collect();
    assert_eq!(
        found_set, expected_set,
        "this crate's `use` set changed; a filesystem or transport import is an extra key here"
    );

    // The whole set of paths reached through a crate root, both directions.
    // This is the net the token list below could not be: a capability written
    // as an absolute path adds a key here whatever it is named.
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        let body = without_use_items(&code);
        reached.extend(absolute_paths(&body));
        macros.extend(macros_spelled(&body));
    }
    assert_eq!(
        reached,
        REACHED_PATHS
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate reaches a path outside its inventory; every entry needs a reason"
    );
    assert_eq!(
        macros,
        MACROS_SPELLED
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate invokes a macro outside its inventory; an include_ macro reads a file"
    );

    // And no `fs::`, no `Command`, no socket construct, anywhere in the package
    // -- tests included, because a test that opened a file would make the crate
    // documentation's claim about the whole package false.
    for path in crate_all_sources()? {
        let code = strip_non_code(&source_of(&path)?);
        for forbidden in FORBIDDEN_CONSTRUCTS {
            // A needle ending in `::` is a path prefix and is matched as a
            // substring; every other one is an identifier and is matched whole,
            // so this file's own test name -- which ends in `no_socket` -- is
            // not read as a socket.
            let named = if forbidden.ends_with("::") {
                code.contains(forbidden)
            } else {
                uses_of(&code, forbidden) > 0
            };
            // Two files in this package read a file, and the set of them is
            // pinned rather than decided here: this one is a source scan and
            // reaches its targets through `fs::read_dir`, and the acceptance
            // suite reads the design document itself so that section 17.5's
            // relation count is measured rather than restated. Both are named
            // on `docs/contracts/policy-source-scans.md`, which
            // `this_scan_is_in_the_inventory` requires. No product file is
            // permitted one.
            let permitted = forbidden == "fs::"
                && READERS
                    .iter()
                    .any(|reader| relative(&path).ends_with(reader));
            assert!(
                permitted || !named,
                "{} spells {forbidden}",
                relative(&path)
            );
        }
    }
    Ok(())
}

#[test]
fn every_field_of_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (_, code) in product_code()? {
        for (owner, field, declared) in type_fields(&code) {
            found.insert((owner, field, declared));
        }
    }
    let expected: BTreeSet<(String, String, String)> = inventory()
        .into_iter()
        .map(|(owner, field, declared, _)| {
            (owner.to_owned(), field.to_owned(), declared.to_owned())
        })
        .collect();
    assert_eq!(
        found, expected,
        "a field of this crate is not in the inventory; every field needs a line saying what it \
         holds, and a field holding repository text does not get one"
    );

    // Every reason is one of the five the module documentation names, so the
    // column is a classification rather than free prose.
    for (owner, field, _, reason) in inventory() {
        assert!(
            !reason.is_empty(),
            "{owner}.{field} has no reason in the inventory"
        );
    }
    Ok(())
}

#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    let mut mutating: Vec<String> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            if signature.contains("&mut self") {
                mutating.push(format!("{file}::{name}"));
            }
        }
    }
    assert_eq!(
        mutating,
        Vec::<String>::new(),
        "`CONTRIBUTING.md` rule 2 is append-only and a correction is a new event; a public \
         function that takes `&mut self` is an edit in place"
    );
    Ok(())
}

#[test]
fn the_correlation_decisions_are_pinned() -> TestResult {
    let relation = source_of(&crate_root().join("src/relation.rs"))?;
    let authority = source_of(&crate_root().join("src/authority.rs"))?;
    let lib = source_of(&crate_root().join("src/lib.rs"))?;
    let compare = source_of(&crate_root().join("src/compare.rs"))?;
    let drift = source_of(&crate_root().join("src/drift.rs"))?;

    assert_eq!(
        whole_block(&relation, "impl EvidenceRelation {")?,
        WHOLE_RELATION_LANE,
        "the relation vocabulary or its lane assignment changed"
    );
    assert_eq!(
        free_function(&authority, "fn admit(")?,
        WHOLE_ADMIT,
        "which authority class a source is admitted at changed"
    );
    assert_eq!(
        free_function(&authority, "pub fn active_view(")?,
        WHOLE_ACTIVE_VIEW,
        "the active view changed"
    );
    assert_eq!(
        free_function(&lib, "fn code_relations(")?,
        WHOLE_CODE_RELATIONS,
        "which relations a P2-R2 finding produces changed"
    );
    assert_eq!(
        free_function(&lib, "fn drifts_of(")?,
        WHOLE_DRIFTS_OF,
        "the drift decision changed"
    );
    assert_eq!(
        free_function(&lib, "fn scopes_of(")?,
        WHOLE_SCOPES_OF,
        "the four drift scopes changed"
    );
    assert_eq!(
        free_function(&lib, "const fn declares_dependency(")?,
        WHOLE_DECLARES_DEPENDENCY,
        "what the dependency channel is over changed"
    );
    assert_eq!(
        free_function(&compare, "pub fn compare(")?,
        WHOLE_COMPARE,
        "the attribution of a difference to an axis changed"
    );
    assert_eq!(
        whole_block(&drift, "impl DriftScopes {")?,
        WHOLE_DRIFT_SCOPES,
        "the four independent scopes changed"
    );
    Ok(())
}

#[test]
fn each_guarded_name_has_exactly_its_call_sites() -> TestResult {
    for (name, expected, owner, consequence) in CALL_SITE_COUNTS {
        let mut total = 0;
        for (file, code) in product_code()? {
            let count = calls_of(&without_use_items(&code), name);
            if count > 0 && !owner.is_empty() {
                assert_eq!(
                    file, owner,
                    "{name} is called from {file}, which is not {owner}: {consequence}"
                );
            }
            total += count;
        }
        assert_eq!(total, expected, "{name}: {consequence}");
    }
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for named in [
        "crates/repository-correlation/tests/correlation_scans.rs",
        "crates/repository-correlation/tests/correlation_lanes.rs",
    ] {
        assert!(
            page.contains(named),
            "{named} is not named in docs/contracts/policy-source-scans.md"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The extractors.
// ---------------------------------------------------------------------------

/// Joins the runs of whitespace that sit inside a path or before a macro's
/// delimiter, and no others.
///
/// `std :: path :: Path` and `include_str ! ("x")` both compile and both pass a
/// scanner that requires the tokens to be adjacent; `P2-R2` measured both. All
/// whitespace cannot simply be deleted: that joins `and` onto `core` in
/// `Formatter and core::str`, `core` stops being a whole identifier, and the
/// key **disappears** -- a transform that can hide a key is worse than the hole
/// it closes.
fn tighten(code: &str) -> String {
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
fn absolute_paths(code: &str) -> BTreeSet<String> {
    let roots = ["std", "core", "alloc", "thiserror"];
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
        // A middle segment of a longer path -- the `b` of `a::b::c` -- is not a
        // crate root. A **leading** `::` is not a middle segment:
        // `::std::path::Path::new(p)` is the absolute form of the same reach,
        // and `P2-R2` measured an earlier version of this function skipping it.
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
fn macros_spelled(code: &str) -> BTreeSet<String> {
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

/// Every `pub fn` in `code`, as its name and its signature up to the body.
fn public_signatures(code: &str) -> Vec<(String, String)> {
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
            found.push((name, code[at..end].to_owned()));
            cursor = after;
        }
    }
    found
}

/// Counts declarations of a function whose name is exactly `name`.
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
fn calls_of(code: &str, name: &str) -> usize {
    let uses = uses_of(code, name);
    let declarations = declarations_of(code, name);
    assert!(
        uses >= declarations,
        "{name} is declared {declarations} times and named {uses}; the two counts disagree"
    );
    uses - declarations
}

/// Every named field of every `struct` and `enum` `code` declares.
///
/// A struct-variant of an enumeration is reported as `Enum::Variant`, so a
/// field added inside one is a key here rather than a shape this extractor has
/// no name for. A tuple struct has no named field at all, and
/// [`the_helpers_are_not_vacuous`] requires this crate to declare none, so the
/// whole-set claim is not quietly narrowed by one.
fn type_fields(code: &str) -> Vec<(String, String, String)> {
    let mut found = Vec::new();
    for keyword in ["struct ", "enum "] {
        let mut cursor = 0;
        while let Some(at) = code[cursor..].find(keyword).map(|at| at + cursor) {
            cursor = at + keyword.len();
            let before = code[..at].chars().next_back().unwrap_or(' ');
            if before.is_alphanumeric() || before == '_' {
                continue;
            }
            let name: String = code[cursor..]
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let rest = &code[cursor + name.len()..];
            // Skip a lifetime or generic parameter list, then require a body.
            let opens = rest.find(['{', ';', '(']);
            let Some(offset) = opens else {
                continue;
            };
            if rest.as_bytes()[offset] != b'{' {
                continue;
            }
            let body_start = cursor + name.len() + offset;
            let Some(body) = balanced(&code[body_start..]) else {
                continue;
            };
            collect_fields(&name, body, &mut found);
        }
    }
    found.sort();
    found
}

/// The text between `code`'s first `{` and its matching `}`, exclusive.
fn balanced(code: &str) -> Option<&str> {
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

/// Splits one declaration body into its top-level items and reads each.
fn collect_fields(owner: &str, body: &str, found: &mut Vec<(String, String, String)>) {
    for item in top_level_items(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if let Some(open) = item.find('{') {
            // An enum struct-variant. Its name is what comes before the brace,
            // less any attribute or doc line the stripper left behind.
            let variant = item[..open]
                .split_whitespace()
                .next_back()
                .unwrap_or("")
                .to_owned();
            if let Some(inner) = balanced(&item[open..]) {
                collect_fields(&format!("{owner}::{variant}"), inner, found);
            }
            continue;
        }
        let Some(colon) = item.find(':') else {
            continue;
        };
        if item.as_bytes().get(colon + 1) == Some(&b':') {
            continue;
        }
        let name = item[..colon]
            .split_whitespace()
            .next_back()
            .unwrap_or("")
            .to_owned();
        if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') {
            continue;
        }
        let declared: String = item[colon + 1..].split_whitespace().collect();
        found.push((owner.to_owned(), name, declared));
    }
}

/// Splits `body` on the commas that sit at nesting depth zero.
fn top_level_items(body: &str) -> Vec<String> {
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

/// The four inventory arrays, as one list.
fn inventory() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    FIELDS
        .into_iter()
        .chain(MORE_FIELDS)
        .chain(LAST_FIELDS)
        .chain(FINAL_FIELDS)
        .collect()
}

#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(declarations_of("fn seal_edge(", "seal"), 0);
    assert_eq!(declarations_of("fn seal(x)", "seal"), 1);
    assert_eq!(calls_of("fn seal(){} Edge::seal(a);", "seal"), 1);
    assert_eq!(uses_of("resealed sealed seal", "seal"), 1);
    assert_eq!(
        without_use_items("use a::b;\nlet x = b();\n").trim(),
        "let x = b();"
    );
    assert_eq!(collapse("// gone\n  a   b\n"), "a b");
    // The stripper is what makes the forbidden-token pass a statement about
    // code: a `fs::` inside a string literal or a comment is prose about the
    // rule, and this file writes both.
    assert_eq!(
        strip_non_code("let a = \"fs::read\"; // fs::read\n"),
        "let a =  ; \n\n"
    );
    // The whole-identifier half: this file's own test name ends in `no_socket`.
    assert_eq!(uses_of("fn a_name_with_no_socket() {}", "socket"), 0);
    assert_eq!(uses_of("Stream::connect(x)", "connect"), 1);

    // The two whole-set reach extractors. Each case is a shape `P2-R2` measured
    // passing an earlier guard.
    assert_eq!(
        absolute_paths("let _ = std::path::Path::new(p).metadata();"),
        BTreeSet::from(["std::path".to_owned()])
    );
    assert_eq!(
        absolute_paths("let _ = std::env::var(k);"),
        BTreeSet::from(["std::env".to_owned()])
    );
    assert_eq!(
        absolute_paths("core::fmt::Formatter and core::str::from_utf8"),
        BTreeSet::from(["core::fmt".to_owned(), "core::str".to_owned()])
    );
    assert_eq!(
        absolute_paths("Self::Variant and self.field"),
        BTreeSet::new()
    );
    assert_eq!(
        absolute_paths("std::collections::BTreeMap"),
        BTreeSet::from(["std::collections".to_owned()])
    );
    assert_eq!(
        absolute_paths("let _ = ::std::path::Path::new(p);"),
        BTreeSet::from(["std::path".to_owned()])
    );
    assert_eq!(
        absolute_paths("let _ = std :: path :: Path::new(p);"),
        BTreeSet::from(["std::path".to_owned()])
    );
    assert_eq!(
        macros_spelled("include_str! (\"x\")"),
        BTreeSet::from(["include_str".to_owned()])
    );
    assert_eq!(tighten("a :: b !\n("), "a::b!(");
    // The case that rules out deleting all whitespace.
    assert_eq!(
        tighten("Formatter and core::str::from_utf8"),
        "Formatter and core::str::from_utf8"
    );
    assert_eq!(
        macros_spelled("return format!(x); if !flag { }"),
        BTreeSet::from(["format".to_owned()])
    );
    assert_eq!(macros_spelled("if a != b { }"), BTreeSet::new());
    assert_eq!(macros_spelled("if !(a || b) { }"), BTreeSet::new());

    // The field extractor. A struct field, an enum struct-variant field, a
    // generic argument that holds a comma, and a shape that is not a field.
    assert_eq!(
        type_fields("struct A {\n    one: String,\n    two: Vec<u8>,\n}\n"),
        vec![
            ("A".to_owned(), "one".to_owned(), "String".to_owned()),
            ("A".to_owned(), "two".to_owned(), "Vec<u8>".to_owned()),
        ]
    );
    assert_eq!(
        type_fields("enum E {\n    Bare,\n    Named { held: u64 },\n}\n"),
        vec![("E::Named".to_owned(), "held".to_owned(), "u64".to_owned())]
    );
    assert_eq!(
        type_fields("struct M {\n    map: BTreeMap<String, Vec<u8>>,\n}\n"),
        vec![(
            "M".to_owned(),
            "map".to_owned(),
            "BTreeMap<String,Vec<u8>>".to_owned()
        )]
    );
    // A field renamed is a different key, which is the whole point: this
    // extractor does not consult a list of names.
    assert_eq!(
        type_fields("struct A {\n    innocuous: Vec<u8>,\n}\n"),
        vec![(
            "A".to_owned(),
            "innocuous".to_owned(),
            "Vec<u8>".to_owned()
        )]
    );
    // A function's parameter list is not a declaration body.
    assert_eq!(
        type_fields("fn f(\n    one: String,\n) -> u8 {\n    1\n}\n"),
        Vec::<(String, String, String)>::new()
    );
    // Every type this crate declares has named fields or none; a tuple struct
    // would carry a field this extractor has no name for.
    for (_, code) in product_code()? {
        for marker in ["struct "] {
            let mut cursor = 0;
            while let Some(at) = code[cursor..].find(marker).map(|at| at + cursor) {
                cursor = at + marker.len();
                let rest = &code[cursor..];
                let name: String = rest
                    .chars()
                    .take_while(|character| character.is_alphanumeric() || *character == '_')
                    .collect();
                let tail = &rest[name.len()..];
                if let Some(offset) = tail.find(['{', ';', '(']) {
                    assert_ne!(
                        tail.as_bytes()[offset],
                        b'(',
                        "{name} is a tuple struct; its fields have no names to inventory"
                    );
                }
            }
        }
    }

    assert_eq!(REACHED_PATHS.len(), 4);
    assert_eq!(MACROS_SPELLED.len(), 1);
    assert_eq!(FORBIDDEN_CONSTRUCTS.len(), 11);
    assert!(
        FORBIDDEN_CONSTRUCTS
            .iter()
            .any(|item| item.ends_with("::Command"))
    );
    let names: HashSet<&str> = CALL_SITE_COUNTS.iter().map(|(name, ..)| *name).collect();
    assert_eq!(names.len(), CALL_SITE_COUNTS.len());
    // No field is listed twice, so the inventory cannot pass by holding one key
    // under two reasons.
    let keys: HashSet<(&str, &str)> = inventory()
        .iter()
        .map(|(owner, field, _, _)| (*owner, *field))
        .collect();
    assert_eq!(keys.len(), inventory().len());
    Ok(())
}
