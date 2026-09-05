//! What `academic-repository-classification` may reach, hold and hand out.
//!
//! ## Why a token list is not enough, measured two tasks ago
//!
//! `P2-R2` shipped this crate's grandparent with a forbidden-token list as its
//! only net, and `docs/contracts/policy-source-scans.md` records what that
//! measured: seven spellings of a filesystem or environment reach --- including
//! `std::path::Path::new(p).metadata()`, its leading-`::` form, its
//! whitespace-inside-the-path form, and `include_str!` --- compile, spell none
//! of the listed tokens, add no `use` item, and passed. The repair was not a
//! longer list. It was three **whole-set** comparisons, in both directions:
//!
//! * every `use` item ([`USE_ITEMS`]);
//! * every two-segment path reached through a crate root ([`REACHED_PATHS`]);
//! * every macro invoked ([`MACROS_SPELLED`]).
//!
//! Those three cover the three ways a capability is reached --- through an
//! import, through an absolute path, through a macro --- and a reach nobody
//! predicted appears as an **extra key** rather than as a token nobody listed.
//! [`FORBIDDEN_CONSTRUCTS`] is kept as the third and weakest layer, because it
//! names the shapes a reader expects to see refused.
//!
//! ## The same defect class, one step out, and what this file does about it
//!
//! `tools/secret-debug-policy.test.mjs` decides whether a field holds something
//! a `Debug` must not print by matching the **field's name** against a fixed
//! alternation --- its `SECRET_FIELD_NAMES` regular expression, which lists
//! `source_bytes`, `payload`, `plaintext` and about forty more. A field holding
//! the same bytes under a name outside that alternation is invisible to it,
//! exactly the way a filesystem call under a spelling nobody listed was
//! invisible to the token pass. `P2-R3` measured that one step out and recorded
//! it in `docs/contracts/repository-correlation.md`; repairing the tool is
//! `T167`'s.
//!
//! **That tool passing this crate is not evidence about this crate.** What is
//! evidence is [`FIELDS`] and the three arrays beside it: every field of every
//! type this crate declares, compared in both directions, each entry carrying
//! what it holds. A field that held a symbol name, an import specifier, a
//! configuration key or a buffer of analyzed bytes appears here as an extra key
//! whatever it is called.
//!
//! ## What this crate is allowed to hold
//!
//! Seven things, and the last column of the inventory is which one:
//!
//! * a **caller-supplied identifier** --- a concept, a goal, a trigger, a
//!   trade-off. `academic-repository-analysis`'s `SubjectId` shape, admitted by
//!   `scope::validated`;
//! * a **system-derived identifier** --- a snapshot identifier, which
//!   `academic-repository` minted;
//! * **a path the gate classified** and the frozen manifest already hands out;
//! * a **closed vocabulary value** --- one of this crate's or another reviewed
//!   crate's enumerations;
//! * a **value of a reviewed crate** --- a `Locator`, a `Finding`, a
//!   `FindingScope`, a `SourceSpan`, a `SymbolFingerprint`, a `DocumentId`;
//! * a **value of this crate**; and
//! * a **count, revision or timestamp**.
//!
//! There is no eighth, and in particular there is no byte buffer: no field of
//! this crate is declared `Vec<u8>` or `[u8; N]` under any name, which is what
//! the inventory says without consulting a list of names.

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
const USE_ITEMS: [(&str, &str); 15] = [
    (
        "crates/repository-classification/src/benefit.rs",
        "use academic_repository_analysis::SubjectId;",
    ),
    (
        "crates/repository-classification/src/benefit.rs",
        "use crate::{ClassificationError, scope::validated};",
    ),
    (
        "crates/repository-classification/src/chain.rs",
        "use academic_domain::{EpistemicStatus, FreshnessBand, MasteryLevel, entity_registry::EntityKind};",
    ),
    (
        "crates/repository-classification/src/chain.rs",
        "use academic_repository_analysis::{EvidenceTier, Finding, FindingScope, Locator, SubjectId};",
    ),
    (
        "crates/repository-classification/src/chain.rs",
        "use academic_repository_correlation::{ApprovalStatus, DocumentId, IntentDocument};",
    ),
    (
        "crates/repository-classification/src/chain.rs",
        "use crate::{ClassificationError, scope::GoalScope};",
    ),
    (
        "crates/repository-classification/src/conflict.rs",
        "use crate::{ ClassificationError, scope::{ClassificationKey, GoalScope, validated}, stance::{ClassificationLabel, Outlook}, };",
    ),
    (
        "crates/repository-classification/src/migrate.rs",
        "use academic_repository_analysis::{ Finding, Locator, RepositoryAnalysis, SourceSpan, SymbolFingerprint, SymbolKind, SymbolRecord, };",
    ),
    (
        "crates/repository-classification/src/requirement.rs",
        "use academic_repository_analysis::Locator;",
    ),
    (
        "crates/repository-classification/src/requirement.rs",
        "use crate::{ ClassificationError, chain::{ConcreteNeed, ProofChain, UserEvidenceGap}, scope::{ClassificationKey, GoalScope, validated}, };",
    ),
    (
        "crates/repository-classification/src/scope.rs",
        "use crate::ClassificationError;",
    ),
    (
        "crates/repository-classification/src/stance.rs",
        "use academic_repository_analysis::{ArtifactScope, EvidenceTier, LadderRung, Locator};",
    ),
    (
        "crates/repository-classification/src/stance.rs",
        "use academic_repository_correlation::{EdgeEvidence, EvidenceRelation, RelationEdge};",
    ),
    (
        "crates/repository-classification/src/stance.rs",
        "use crate::{benefit::BenefitContract, chain::ProofChain, scope::ClassificationKey};",
    ),
    (
        "crates/repository-classification/src/lib.rs",
        "use std::collections::{BTreeMap, BTreeSet};",
    ),
];

/// Every re-export of this crate's `lib.rs`, kept apart from [`USE_ITEMS`].
///
/// A `pub use` is a surface decision rather than a reach, so it is listed here
/// and compared here; mixing the two lists would let a new `use` be excused as
/// a re-export.
const RE_EXPORTS: [&str; 7] = [
    "pub use benefit::{ BenefitContract, BenefitDimension, BenefitDraft, BenefitPart, TradeOff, Trigger, TriggerState, };",
    "pub use chain::{ ChainDraft, ChainStep, ConcreteNeed, ControllingMechanism, CurrentBasis, NeedKind, ProofChain, RequiredConcept, UserEvidenceGap, };",
    "pub use conflict::{ClassificationConflict, OverrideDecision, UserOverride};",
    "pub use migrate::{ LocatorMigration, MigratedFinding, MigratedSite, MigrationOutcome, UnmatchedReason, migrate_locators, };",
    "pub use requirement::{ LifecycleRow, ProjectConceptRequirement, RequirementId, ResolutionStatus, RetirementReason, };",
    "pub use scope::{ClassificationKey, GoalId, GoalScope};",
    "pub use stance::{ClassificationLabel, ConceptStance, ObservedProof, Outlook};",
];

/// The non-`use` imports of `lib.rs`, which the two lists above do not hold.
const LIB_IMPORTS: [&str; 4] = [
    "use academic_domain::entity_registry::EntityKind;",
    "use academic_policy::ContentDigest;",
    "use academic_repository_analysis::{EvidenceTier, Finding, SubjectId};",
    "use academic_repository_correlation::{Correlation, IntentDocument, RelationEdge};",
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
        "academic_repository_analysis::AnalysisError",
        "the P2-R2 refusal a SubjectId constructor raises, carried into this \
         crate's own error rather than re-validated here",
    ),
    (
        "academic_repository_correlation::ApprovalStatus",
        "P2-R3's approval vocabulary, read to decide whether a goal is approved",
    ),
    (
        "std::collections",
        "the BTreeMap entry API the observation reducer uses, and the BTreeSet \
         the concept union is collected into",
    ),
    ("thiserror::Error", "the error enumeration's derive"),
];

/// Every macro this crate's product code invokes.
///
/// A macro is not a path, so [`REACHED_PATHS`] cannot see one. `include_str!`
/// and `include_bytes!` read a file at compile time while spelling no `std`
/// path and needing no `use`.
const MACROS_SPELLED: [(&str, &str); 2] = [
    (
        "matches",
        "the identifier validator's byte class, and ResolutionStatus::is_open, \
         both over closed sets",
    ),
    (
        "vec",
        "the one-row history a materialized requirement opens with",
    ),
];

/// The two files of this package that read a file, as a whole set.
///
/// Both are test files and both are named on
/// `docs/contracts/policy-source-scans.md`. No product file is here, which is
/// what makes *this crate opens nothing* a statement about the crate.
const READERS: [&str; 2] = [
    "tests/classification_scans.rs",
    "tests/classification_lanes.rs",
];

/// The constructs the forbidden-token pass refuses anywhere in the package.
///
/// Assembled from halves for the reason `P2-R2`'s and `P2-R3`'s equivalent
/// lists are: two other scans in this repository read raw source for these
/// exact spellings, and a file spelling one whole would have to be added to
/// somebody else's *reviewed sites* list as a file that does what it does not
/// do. The `concat!` is evaluated at compile time, so the value compared
/// against the source is the whole spelling either way.
///
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

/// Names whose call-site count is pinned, with where they may be called from.
///
/// A second producer of any of these is a second place a section 18 decision
/// could be made differently.
const CALL_SITE_COUNTS: [(&str, usize, &str, &str); 5] = [
    (
        // Three: the classification key, the stance and the conflict. All in
        // `lib.rs`, because a second producer could seal a stance whose outlook
        // disagreed with the chains it was derived from, or a conflict that
        // carried one side and not the other.
        "seal",
        3,
        "crates/repository-classification/src/lib.rs",
        "a key, a stance or a conflict is constructed outside the classifier",
    ),
    (
        // The one route from a P2-R3 edge to section 18.1's OBSERVED. A second
        // one could admit an edge this one refuses -- a manifest presence, or a
        // document -- as an observation of use.
        "of_edge",
        2,
        "crates/repository-classification/src/lib.rs",
        "an OBSERVED is derived from an edge outside the one reducer and the \
         one reader that expose it",
    ),
    (
        // The one place section 18.4's entity is created.
        "materialize",
        1,
        "crates/repository-classification/src/lib.rs",
        "a ProjectConceptRequirement is materialized from more than one place",
    ),
    (
        // The one lifecycle transition, called by the three terminal verbs. A
        // fourth verb that did not go through it could skip the
        // already-settled refusal or forget to append the history row.
        "settle",
        3,
        "crates/repository-classification/src/requirement.rs",
        "a lifecycle transition happens outside the one function that refuses \
         a second one and appends the history row",
    ),
    (
        // The one place a user decision is weighed against a proposal.
        "contradicts",
        1,
        "crates/repository-classification/src/lib.rs",
        "an override is weighed against a proposal from more than one place",
    ),
];

const FIELDS: [(&str, &str, &str, &str); 27] = [
    (
        "BenefitContract",
        "benefit",
        "BenefitDimension",
        "closed vocabulary value",
    ),
    (
        "BenefitContract",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "BenefitContract",
        "state",
        "TriggerState",
        "closed vocabulary value",
    ),
    (
        "BenefitContract",
        "tradeoffs",
        "Vec<TradeOff>",
        "value of this crate",
    ),
    (
        "BenefitContract",
        "triggers",
        "Vec<Trigger>",
        "value of this crate",
    ),
    (
        "BenefitDraft",
        "benefit",
        "Option<BenefitDimension>",
        "closed vocabulary value",
    ),
    (
        "BenefitDraft",
        "concept",
        "Option<String>",
        "caller-supplied identifier",
    ),
    (
        "BenefitDraft",
        "state",
        "Option<TriggerState>",
        "closed vocabulary value",
    ),
    (
        "BenefitDraft",
        "tradeoffs",
        "Vec<TradeOff>",
        "value of this crate",
    ),
    (
        "BenefitDraft",
        "triggers",
        "Vec<Trigger>",
        "value of this crate",
    ),
    (
        "ChainDraft",
        "basis",
        "Option<CurrentBasis>",
        "value of this crate",
    ),
    (
        "ChainDraft",
        "concept",
        "Option<DraftConcept>",
        "value of this crate",
    ),
    (
        "ChainDraft",
        "gap",
        "Option<UserEvidenceGap>",
        "value of this crate",
    ),
    (
        "ChainDraft",
        "mechanism",
        "Option<String>",
        "caller-supplied identifier",
    ),
    (
        "ChainDraft",
        "need",
        "Option<DraftNeed>",
        "value of this crate",
    ),
    (
        "ClassificationConflict",
        "key",
        "ClassificationKey",
        "value of this crate",
    ),
    (
        "ClassificationConflict",
        "proposed",
        "Outlook",
        "value of this crate",
    ),
    (
        "ClassificationConflict",
        "standing",
        "UserOverride",
        "value of this crate",
    ),
    (
        "ClassificationError::BenefitPartMissing",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "ClassificationError::BenefitPartMissing",
        "part",
        "BenefitPart",
        "closed vocabulary value",
    ),
    (
        "ClassificationError::TierCannotBeRequired",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "ClassificationError::TierCannotBeRequired",
        "tier",
        "EntityKind",
        "closed vocabulary value",
    ),
    (
        "ClassificationInput",
        "beneficial",
        "&'a[BenefitContract]",
        "value of this crate",
    ),
    (
        "ClassificationInput",
        "correlation",
        "&'aCorrelation",
        "value of a reviewed crate",
    ),
    (
        "ClassificationInput",
        "goal",
        "&'aGoalScope",
        "value of this crate",
    ),
    (
        "ClassificationInput",
        "overrides",
        "&'a[UserOverride]",
        "value of this crate",
    ),
    (
        "ClassificationInput",
        "required",
        "&'a[ProofChain]",
        "value of this crate",
    ),
];

const MORE_FIELDS: [(&str, &str, &str, &str); 27] = [
    (
        "ClassificationKey",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "ClassificationKey",
        "goal",
        "GoalScope",
        "value of this crate",
    ),
    (
        "ClassificationKey",
        "snapshot_id",
        "String",
        "system-derived identifier",
    ),
    (
        "ClassificationSet",
        "conflicts",
        "Vec<ClassificationConflict>",
        "value of this crate",
    ),
    (
        "ClassificationSet",
        "goal",
        "GoalScope",
        "value of this crate",
    ),
    (
        "ClassificationSet",
        "requirements",
        "Vec<ProjectConceptRequirement>",
        "value of this crate",
    ),
    (
        "ClassificationSet",
        "snapshot_id",
        "String",
        "system-derived identifier",
    ),
    (
        "ClassificationSet",
        "stances",
        "Vec<ConceptStance>",
        "value of this crate",
    ),
    (
        "ConceptStance",
        "key",
        "ClassificationKey",
        "value of this crate",
    ),
    (
        "ConceptStance",
        "observed",
        "Option<ObservedProof>",
        "value of this crate",
    ),
    (
        "ConceptStance",
        "outlook",
        "Option<Outlook>",
        "value of this crate",
    ),
    (
        "ConcreteNeed",
        "basis",
        "CurrentBasis",
        "value of this crate",
    ),
    (
        "ConcreteNeed",
        "kind",
        "NeedKind",
        "closed vocabulary value",
    ),
    (
        "ConcreteNeed",
        "name",
        "String",
        "caller-supplied identifier",
    ),
    (
        "ConcreteNeed",
        "sites",
        "Vec<Locator>",
        "value of a reviewed crate",
    ),
    (
        "ControllingMechanism",
        "name",
        "String",
        "caller-supplied identifier",
    ),
    (
        "ControllingMechanism",
        "need",
        "ConcreteNeed",
        "value of this crate",
    ),
    (
        "CurrentBasis::ApprovedGoal",
        "document",
        "DocumentId",
        "value of a reviewed crate",
    ),
    (
        "CurrentBasis::ApprovedGoal",
        "goal",
        "GoalScope",
        "value of this crate",
    ),
    (
        "CurrentBasis::ApprovedGoal",
        "revision",
        "u64",
        "count, revision or timestamp",
    ),
    (
        "CurrentBasis::ApprovedGoal",
        "snapshot_id",
        "String",
        "system-derived identifier",
    ),
    (
        "CurrentBasis::CurrentCode",
        "scope",
        "FindingScope",
        "value of a reviewed crate",
    ),
    (
        "CurrentBasis::CurrentCode",
        "sites",
        "Vec<Locator>",
        "value of a reviewed crate",
    ),
    (
        "CurrentBasis::CurrentCode",
        "snapshot_id",
        "String",
        "system-derived identifier",
    ),
    (
        "CurrentBasis::CurrentCode",
        "subject",
        "String",
        "caller-supplied identifier",
    ),
    (
        "DraftConcept",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "DraftConcept",
        "tier",
        "EntityKind",
        "closed vocabulary value",
    ),
];

const LAST_FIELDS: [(&str, &str, &str, &str); 27] = [
    ("DraftNeed", "kind", "NeedKind", "closed vocabulary value"),
    ("DraftNeed", "name", "String", "caller-supplied identifier"),
    (
        "DraftNeed",
        "sites",
        "Vec<Locator>",
        "value of a reviewed crate",
    ),
    (
        "GoalId",
        "identifier",
        "String",
        "caller-supplied identifier",
    ),
    ("GoalScope", "goal", "GoalId", "value of this crate"),
    (
        "GoalScope",
        "version",
        "u64",
        "count, revision or timestamp",
    ),
    ("LifecycleRow", "at", "u64", "count, revision or timestamp"),
    (
        "LifecycleRow",
        "status",
        "ResolutionStatus",
        "value of this crate",
    ),
    (
        "LocatorMigration",
        "ordinal",
        "usize",
        "count, revision or timestamp",
    ),
    (
        "LocatorMigration",
        "original",
        "Locator",
        "value of a reviewed crate",
    ),
    (
        "LocatorMigration",
        "outcome",
        "MigrationOutcome",
        "value of this crate",
    ),
    (
        "MigratedFinding",
        "migrations",
        "Vec<LocatorMigration>",
        "value of this crate",
    ),
    (
        "MigratedFinding",
        "original",
        "Finding",
        "value of a reviewed crate",
    ),
    (
        "MigratedFinding",
        "to_snapshot",
        "String",
        "system-derived identifier",
    ),
    (
        "MigratedSite",
        "path",
        "String",
        "a path the gate classified and the frozen manifest hands out",
    ),
    (
        "MigratedSite",
        "span",
        "SourceSpan",
        "value of a reviewed crate",
    ),
    (
        "MigratedSite",
        "symbol",
        "SymbolFingerprint",
        "value of a reviewed crate",
    ),
    (
        "MigratedSite",
        "symbol_kind",
        "SymbolKind",
        "closed vocabulary value",
    ),
    (
        "ObservedProof",
        "artifact_scope",
        "ArtifactScope",
        "closed vocabulary value",
    ),
    (
        "ObservedProof",
        "locators",
        "Vec<Locator>",
        "value of a reviewed crate",
    ),
    (
        "ObservedProof",
        "relation",
        "EvidenceRelation",
        "closed vocabulary value",
    ),
    (
        "ObservedProof",
        "rung",
        "LadderRung",
        "closed vocabulary value",
    ),
    (
        "ProjectConceptRequirement",
        "history",
        "Vec<LifecycleRow>",
        "value of this crate",
    ),
    (
        "ProjectConceptRequirement",
        "id",
        "RequirementId",
        "value of this crate",
    ),
    (
        "ProjectConceptRequirement",
        "key",
        "ClassificationKey",
        "value of this crate",
    ),
    (
        "ProjectConceptRequirement",
        "need",
        "ConcreteNeed",
        "value of this crate",
    ),
    (
        "ProjectConceptRequirement",
        "status",
        "ResolutionStatus",
        "value of this crate",
    ),
];

const FINAL_FIELDS: [(&str, &str, &str, &str); 24] = [
    (
        "ProjectConceptRequirement",
        "user_state",
        "UserEvidenceGap",
        "value of this crate",
    ),
    (
        "ProofChain",
        "concept",
        "RequiredConcept",
        "value of this crate",
    ),
    (
        "ProofChain",
        "gap",
        "UserEvidenceGap",
        "value of this crate",
    ),
    (
        "RequiredConcept",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "RequiredConcept",
        "mechanism",
        "ControllingMechanism",
        "value of this crate",
    ),
    (
        "RequiredConcept",
        "tier",
        "EntityKind",
        "closed vocabulary value",
    ),
    (
        "RequirementId",
        "identifier",
        "String",
        "caller-supplied identifier",
    ),
    (
        "ResolutionStatus::Replaced",
        "by",
        "RequirementId",
        "value of this crate",
    ),
    (
        "ResolutionStatus::Replaced",
        "snapshot_id",
        "String",
        "system-derived identifier",
    ),
    (
        "ResolutionStatus::Retired",
        "reason",
        "RetirementReason",
        "closed vocabulary value",
    ),
    (
        "ResolutionStatus::Retired",
        "snapshot_id",
        "String",
        "system-derived identifier",
    ),
    (
        "ResolutionStatus::Satisfied",
        "evidence",
        "Vec<Locator>",
        "value of a reviewed crate",
    ),
    (
        "ResolutionStatus::Satisfied",
        "snapshot_id",
        "String",
        "system-derived identifier",
    ),
    (
        "TradeOff",
        "identifier",
        "String",
        "caller-supplied identifier",
    ),
    (
        "Trigger",
        "identifier",
        "String",
        "caller-supplied identifier",
    ),
    (
        "UserEvidenceGap::Insufficient",
        "mastery",
        "MasteryLevel",
        "closed vocabulary value",
    ),
    (
        "UserEvidenceGap::Uncertain",
        "freshness",
        "FreshnessBand",
        "closed vocabulary value",
    ),
    (
        "UserEvidenceGap::Uncertain",
        "mastery",
        "MasteryLevel",
        "closed vocabulary value",
    ),
    (
        "UserEvidenceGap::Uncertain",
        "status",
        "EpistemicStatus",
        "closed vocabulary value",
    ),
    (
        "UserOverride",
        "asserted_at",
        "u64",
        "count, revision or timestamp",
    ),
    (
        "UserOverride",
        "asserted_from_snapshot",
        "String",
        "system-derived identifier",
    ),
    (
        "UserOverride",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "UserOverride",
        "decision",
        "OverrideDecision",
        "closed vocabulary value",
    ),
    ("UserOverride", "goal", "GoalScope", "value of this crate"),
];

// ---------------------------------------------------------------------------
// The pins.
// ---------------------------------------------------------------------------

const WHOLE_CHAIN_STEP: &str = "impl ChainStep { pub const ALL: [Self; 5] = [ Self::CurrentBasis, Self::ConcreteNeed, Self::ControllingMechanism, Self::RequiredConcept, Self::UserEvidenceGap, ]; #[must_use] pub const fn as_str(self) -> &'static str { match self { Self::CurrentBasis => \"MISSING_CURRENT_BASIS\", Self::ConcreteNeed => \"MISSING_CONCRETE_NEED\", Self::ControllingMechanism => \"MISSING_CONTROLLING_MECHANISM\", Self::RequiredConcept => \"MISSING_REQUIRED_CONCEPT\", Self::UserEvidenceGap => \"MISSING_USER_EVIDENCE_GAP\", } } }";

const WHOLE_REQUIRED_CONCEPT: &str = "impl RequiredConcept { pub fn realizing( mechanism: ControllingMechanism, concept: &SubjectId, tier: EntityKind, ) -> Result<Self, ClassificationError> { match tier { EntityKind::Field | EntityKind::Alias => { Err(ClassificationError::TierCannotBeRequired { concept: concept.as_str().to_owned(), tier, }) } EntityKind::Concept | EntityKind::ConceptSense | EntityKind::Operation => Ok(Self { mechanism, concept: concept.as_str().to_owned(), tier, }), } } #[must_use] pub const fn mechanism(&self) -> &ControllingMechanism { &self.mechanism } #[must_use] pub fn concept(&self) -> &str { &self.concept } #[must_use] pub const fn tier(&self) -> EntityKind { self.tier } }";

const WHOLE_USER_EVIDENCE_GAP: &str = "impl UserEvidenceGap { #[must_use] pub fn of( mastery: MasteryLevel, freshness: FreshnessBand, status: EpistemicStatus, ) -> Option<Self> { if mastery < MasteryLevel::Applied { return Some(Self::Insufficient { mastery }); } let fresh = freshness >= FreshnessBand::Moderate; let confirmed = status == EpistemicStatus::UserConfirmed; if fresh && confirmed { None } else { Some(Self::Uncertain { mastery, freshness, status, }) } } #[must_use] pub const fn as_str(&self) -> &'static str { match self { Self::Insufficient { .. } => \"INSUFFICIENT\", Self::Uncertain { .. } => \"UNCERTAIN\", } } #[must_use] pub const fn mastery(&self) -> MasteryLevel { match self { Self::Insufficient { mastery } | Self::Uncertain { mastery, .. } => *mastery, } } }";

const WHOLE_CHAIN_DRAFT: &str = "impl ChainDraft { #[must_use] pub fn new() -> Self { Self::default() } #[must_use] pub fn with_basis(mut self, basis: CurrentBasis) -> Self { self.basis = Some(basis); self } #[must_use] pub fn with_need(mut self, kind: NeedKind, name: &SubjectId, sites: Vec<Locator>) -> Self { self.need = Some(DraftNeed { kind, name: name.as_str().to_owned(), sites, }); self } #[must_use] pub fn with_mechanism(mut self, name: &SubjectId) -> Self { self.mechanism = Some(name.as_str().to_owned()); self } #[must_use] pub fn with_concept(mut self, concept: &SubjectId, tier: EntityKind) -> Self { self.concept = Some(DraftConcept { concept: concept.as_str().to_owned(), tier, }); self } #[must_use] pub const fn with_gap(mut self, gap: UserEvidenceGap) -> Self { self.gap = Some(gap); self } pub fn seal(self) -> Result<ProofChain, ClassificationError> { let missing = ClassificationError::ProofChainStepMissing; let basis = self.basis.ok_or(missing(ChainStep::CurrentBasis))?; let need = self.need.ok_or(missing(ChainStep::ConcreteNeed))?; let mechanism = self .mechanism .ok_or(missing(ChainStep::ControllingMechanism))?; let concept = self.concept.ok_or(missing(ChainStep::RequiredConcept))?; let gap = self.gap.ok_or(missing(ChainStep::UserEvidenceGap))?; let linked_need = ConcreteNeed::shown_by(basis, need.kind, &SubjectId::new(need.name)?, need.sites)?; let linked_mechanism = ControllingMechanism::controlling(linked_need, &SubjectId::new(mechanism)?); let linked_concept = RequiredConcept::realizing( linked_mechanism, &SubjectId::new(concept.concept)?, concept.tier, )?; Ok(ProofChain::closed_by(linked_concept, gap)) } }";

const WHOLE_OUTLOOK: &str = "impl Outlook { #[must_use] pub const fn label(&self) -> ClassificationLabel { match self { Self::Required(_) => ClassificationLabel::Required, Self::Beneficial(_) => ClassificationLabel::WouldBenefitFrom, } } #[must_use] pub const fn chain(&self) -> Option<&ProofChain> { match self { Self::Required(chain) => Some(chain), Self::Beneficial(_) => None, } } #[must_use] pub const fn contract(&self) -> Option<&BenefitContract> { match self { Self::Beneficial(contract) => Some(contract), Self::Required(_) => None, } } }";

const WHOLE_OVERRIDE_DECISION: &str = "impl OverrideDecision { pub const ALL: [Self; 3] = [Self::NotRequired, Self::NotBeneficial, Self::Required]; #[must_use] pub const fn as_str(self) -> &'static str { match self { Self::NotRequired => \"NOT_REQUIRED\", Self::NotBeneficial => \"NOT_BENEFICIAL\", Self::Required => \"REQUIRED\", } } #[must_use] pub const fn contradicts(self, proposed: ClassificationLabel) -> bool { match (self, proposed) { (Self::NotRequired, ClassificationLabel::Required) | (Self::NotBeneficial, ClassificationLabel::WouldBenefitFrom) | (Self::Required, ClassificationLabel::WouldBenefitFrom) => true, (Self::NotRequired, ClassificationLabel::WouldBenefitFrom) | (Self::NotBeneficial, ClassificationLabel::Required) | (Self::Required, ClassificationLabel::Required) | ( Self::NotRequired | Self::NotBeneficial | Self::Required, ClassificationLabel::Observed, ) => false, } } }";

const WHOLE_CLASSIFY: &str = "pub fn classify(input: &ClassificationInput<'_>) -> Result<ClassificationSet, ClassificationError> { let snapshot_id = input.correlation.snapshot_id(); let goal_name = input.goal.goal().as_str().to_owned(); let version = input.goal.version(); let mut required: BTreeMap<String, &ProofChain> = BTreeMap::new(); for chain in input.required { if chain.snapshot_id() != snapshot_id { return Err(ClassificationError::ChainIsAboutAnotherSnapshot( chain.concept().concept().to_owned(), chain.snapshot_id().to_owned(), )); } let concept = chain.concept().concept().to_owned(); if required.insert(concept.clone(), chain).is_some() { return Err(ClassificationError::DuplicateRequirement( concept, goal_name, version, )); } } let mut beneficial: BTreeMap<String, &BenefitContract> = BTreeMap::new(); for contract in input.beneficial { let concept = contract.concept().to_owned(); if beneficial.insert(concept.clone(), contract).is_some() { return Err(ClassificationError::DuplicateBenefit( concept, goal_name, version, )); } } if let Some(concept) = required.keys().find(|name| beneficial.contains_key(*name)) { return Err(ClassificationError::RequiredAndBenefitInOneScope( concept.clone(), goal_name, version, )); } let observed = observed_by_concept(input.correlation); let mut concepts: BTreeSet<&str> = BTreeSet::new(); concepts.extend(observed.keys().map(String::as_str)); concepts.extend(required.keys().map(String::as_str)); concepts.extend(beneficial.keys().map(String::as_str)); let mut stances = Vec::new(); let mut conflicts = Vec::new(); let mut requirements = Vec::new(); for concept in concepts { let key = ClassificationKey::seal( snapshot_id.to_owned(), input.goal.clone(), concept.to_owned(), ); let proposed = required .get(concept) .map(|chain| Outlook::Required((*chain).clone())) .or_else(|| { beneficial .get(concept) .map(|contract| Outlook::Beneficial((*contract).clone())) }); let standing = input .overrides .iter() .filter(|item| item.governs(&key)) .max_by_key(|item| item.asserted_at()); let published = match (standing, proposed) { (Some(user), Some(proposal)) if user.decision().contradicts(proposal.label()) => { conflicts.push(ClassificationConflict::seal( key.clone(), (*user).clone(), proposal, )); None } (_, proposal) => proposal, }; if let Some(Outlook::Required(chain)) = &published { requirements.push(ProjectConceptRequirement::materialize( RequirementId::new(requirement_identity( snapshot_id, &goal_name, version, concept, ))?, key.clone(), chain, 0, )); } stances.push(ConceptStance::seal( key, observed.get(concept).cloned(), published, )); } Ok(ClassificationSet { snapshot_id: snapshot_id.to_owned(), goal: input.goal.clone(), stances, conflicts, requirements, }) }";

const WHOLE_REQUIREMENT_IDENTITY: &str = "fn requirement_identity(snapshot_id: &str, goal: &str, version: u64, concept: &str) -> String { let mut preimage = b\"academic-repository-classification-requirement-v1\\0\".to_vec(); for part in [snapshot_id, goal, &version.to_string(), concept] { preimage.extend_from_slice(part.as_bytes()); preimage.push(0); } ContentDigest::of(&preimage).as_str().to_owned() }";

const WHOLE_MIGRATE_LOCATORS: &str = "pub fn migrate_locators(finding: &Finding, into: &RepositoryAnalysis) -> MigratedFinding { let symbols = into.symbols(); let migrations = finding .locators() .iter() .enumerate() .map(|(ordinal, original)| LocatorMigration { ordinal, original: original.clone(), outcome: follow(original, &symbols, into), }) .collect(); MigratedFinding { original: finding.clone(), to_snapshot: into.snapshot_id().to_owned(), migrations, } }";

const WHOLE_FOLLOW: &str = "fn follow( original: &Locator, symbols: &[SymbolRecord], into: &RepositoryAnalysis, ) -> MigrationOutcome { let Some(fingerprint) = original.symbol() else { return MigrationOutcome::Unmatched(UnmatchedReason::NoSymbolAnchor); }; let found = symbols .iter() .find(|record| record.fingerprint() == fingerprint); match found { Some(record) => MigrationOutcome::Migrated(MigratedSite { path: record.path().to_owned(), symbol: record.fingerprint().clone(), symbol_kind: record.kind(), span: record.span(), }), None => { let path_present = into .coverage() .iter() .any(|row| row.path() == original.path()); if path_present { MigrationOutcome::Unmatched(UnmatchedReason::SymbolGone) } else { MigrationOutcome::Unmatched(UnmatchedReason::PathRemoved) } } } }";

// ---------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 9,
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
        declared >= 7,
        "the tripwire read only {declared} module declarations"
    );
    Ok(())
}

#[test]
fn the_classification_crate_touches_no_file_and_no_socket() -> TestResult {
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
    let lib = "crates/repository-classification/src/lib.rs";
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
            // suite reads the design document itself so that section 18's
            // classification names and section 18.2's chain steps are measured
            // rather than restated. Both are named on
            // `docs/contracts/policy-source-scans.md`, which
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

    // Every reason is one of the seven the module documentation names, so the
    // column is a classification rather than free prose.
    let admitted: BTreeSet<&str> = REASONS.into_iter().collect();
    for (owner, field, _, reason) in inventory() {
        assert!(
            admitted.contains(reason),
            "{owner}.{field} is described as {reason:?}, which is not one of the seven"
        );
    }

    // And no field of this crate is a byte buffer under any name. That is the
    // claim `tools/secret-debug-policy.test.mjs` answers with a name
    // alternation; here it is answered over the declared type of every field.
    for (owner, field, declared, _) in inventory() {
        assert!(
            !declared.contains("u8"),
            "{owner}.{field} is declared {declared}, which is a byte buffer"
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
fn the_classification_decisions_are_pinned() -> TestResult {
    let chain = source_of(&crate_root().join("src/chain.rs"))?;
    let stance = source_of(&crate_root().join("src/stance.rs"))?;
    let conflict = source_of(&crate_root().join("src/conflict.rs"))?;
    let lib = source_of(&crate_root().join("src/lib.rs"))?;
    let migrate = source_of(&crate_root().join("src/migrate.rs"))?;

    assert_eq!(
        whole_block(&chain, "impl ChainStep {")?,
        WHOLE_CHAIN_STEP,
        "section 18.2's five steps or their missing-step codes changed"
    );
    assert_eq!(
        whole_block(&chain, "impl RequiredConcept {")?,
        WHOLE_REQUIRED_CONCEPT,
        "which ontology tiers a project may require changed"
    );
    assert_eq!(
        whole_block(&chain, "impl UserEvidenceGap {")?,
        WHOLE_USER_EVIDENCE_GAP,
        "what counts as the user's insufficient or uncertain evidence changed"
    );
    assert_eq!(
        whole_block(&chain, "impl ChainDraft {")?,
        WHOLE_CHAIN_DRAFT,
        "the one door from an untyped proposal into a proof chain changed"
    );
    assert_eq!(
        whole_block(&stance, "impl Outlook {")?,
        WHOLE_OUTLOOK,
        "the single forward-looking slot changed"
    );
    assert_eq!(
        whole_block(&conflict, "impl OverrideDecision {")?,
        WHOLE_OVERRIDE_DECISION,
        "which user decision contradicts which proposed label changed"
    );
    assert_eq!(
        free_function(&lib, "pub fn classify(")?,
        WHOLE_CLASSIFY,
        "the classifier changed"
    );
    assert_eq!(
        free_function(&lib, "fn requirement_identity(")?,
        WHOLE_REQUIREMENT_IDENTITY,
        "the requirement identity changed; a joined and truncated one collides"
    );
    assert_eq!(
        free_function(&migrate, "pub fn migrate_locators(")?,
        WHOLE_MIGRATE_LOCATORS,
        "locator migration changed; one record per original locator is the guard"
    );
    assert_eq!(
        free_function(&migrate, "fn follow(")?,
        WHOLE_FOLLOW,
        "which symbol a locator follows into a new snapshot changed"
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
fn the_migration_result_is_positional_and_never_keyed_on_content() -> TestResult {
    // `P2-A1`'s fifth audit found a P1 defect where an artifact's content was
    // its identity, so two byte-identical artifacts collapsed into one key.
    // `finding_locator_migration_preserves_original_evidence` measures the
    // behaviour; this measures the shape, because a map keyed on a locator or
    // on a migrated symbol would reintroduce it whatever the behaviour test
    // happened to cover.
    let migrate = strip_non_code(&source_of(&crate_root().join("src/migrate.rs"))?);
    for keyed in ["BTreeMap", "HashMap", "BTreeSet", "HashSet"] {
        assert_eq!(
            uses_of(&migrate, keyed),
            0,
            "src/migrate.rs names {keyed}; a migration record set keyed on anything but the \
             original locator's position collapses two equal originals into one"
        );
    }
    // And the field that holds them is a `Vec` carrying an ordinal.
    assert!(
        inventory().iter().any(|(owner, field, declared, _)| {
            *owner == "MigratedFinding"
                && *field == "migrations"
                && *declared == "Vec<LocatorMigration>"
        }),
        "the migration records are no longer an ordered list"
    );
    assert!(
        inventory()
            .iter()
            .any(|(owner, field, declared, _)| *owner == "LocatorMigration"
                && *field == "ordinal"
                && *declared == "usize"),
        "a migration record no longer carries its position"
    );
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for named in [
        "crates/repository-classification/tests/classification_scans.rs",
        "crates/repository-classification/tests/classification_lanes.rs",
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
    let mut taken = 0_usize;
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
        // crate root, and skipping it is what stops one path yielding two keys.
        // What decides it is whether this segment already sits inside a key
        // this pass took, not the byte three positions back. `tighten` glues
        // `as ::std` shut, so that byte is the `s` of a keyword and the leading
        // `::` of a qualified path read as a middle one: `P2-A5` measured
        // `<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)` resolving
        // a name from a live function while this pass reported nothing. Every
        // segment outside a key already taken is a root, and a root nobody
        // admits fails as an extra key rather than passing.
        if start < taken {
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
            taken = end;
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

/// The seven things a field of this crate may hold.
const REASONS: [&str; 7] = [
    "caller-supplied identifier",
    "system-derived identifier",
    "a path the gate classified and the frozen manifest hands out",
    "closed vocabulary value",
    "value of a reviewed crate",
    "value of this crate",
    "count, revision or timestamp",
];

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
    assert_eq!(calls_of("fn seal(){} Key::seal(a);", "seal"), 1);
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
        vec![("A".to_owned(), "innocuous".to_owned(), "Vec<u8>".to_owned())]
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

    // The pins are text, not empty strings: an empty pin would compare equal to
    // nothing this file extracts and would still pass every `assert_eq!` if the
    // extractor silently started returning one.
    for (name, pin) in [
        ("WHOLE_CHAIN_STEP", WHOLE_CHAIN_STEP),
        ("WHOLE_REQUIRED_CONCEPT", WHOLE_REQUIRED_CONCEPT),
        ("WHOLE_USER_EVIDENCE_GAP", WHOLE_USER_EVIDENCE_GAP),
        ("WHOLE_CHAIN_DRAFT", WHOLE_CHAIN_DRAFT),
        ("WHOLE_OUTLOOK", WHOLE_OUTLOOK),
        ("WHOLE_OVERRIDE_DECISION", WHOLE_OVERRIDE_DECISION),
        ("WHOLE_CLASSIFY", WHOLE_CLASSIFY),
        ("WHOLE_REQUIREMENT_IDENTITY", WHOLE_REQUIREMENT_IDENTITY),
        ("WHOLE_MIGRATE_LOCATORS", WHOLE_MIGRATE_LOCATORS),
        ("WHOLE_FOLLOW", WHOLE_FOLLOW),
    ] {
        assert!(pin.len() > 80, "{name} is too short to be a pin");
    }

    assert_eq!(REACHED_PATHS.len(), 4);
    assert_eq!(MACROS_SPELLED.len(), 2);
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
    assert_eq!(inventory().len(), 105);
    // A qualified path is a leading `::` however it is spelled. `tighten` glues
    // the space in `<T as ::std::net::X>` shut, and deciding on the byte before
    // the `::` then read the crate root as a middle segment: `P2-A5` measured a
    // name resolved from a live function with this pass reporting nothing.
    assert!(
        absolute_paths("let _ = <str as ::std::net::ToSocketAddrs>::to_socket_addrs(h);")
            .contains("std::net")
    );
    assert!(absolute_paths("let _: &dyn ::core::fmt::Debug = &v;").contains("core::fmt"));
    // The other direction, so the repair is not "every segment is a root": a
    // real middle segment still yields no second key.
    assert!(!absolute_paths("std::alloc::Layout::new::<u8>()").contains("alloc::Layout"));
    Ok(())
}

/// Every `impl` header this crate declares, as a whole set.
///
/// Read out of this crate's own source by the reader below and compared
/// whole in both directions, so an `impl` added anywhere is an entry here or
/// a failure. `P2-A5` measured that nothing else in the repository can see one.
const IMPL_HEADERS: [&str; 38] = [
    "impl BenefitContract",
    "impl BenefitDimension",
    "impl BenefitDraft",
    "impl BenefitPart",
    "impl ChainDraft",
    "impl ChainStep",
    "impl ClassificationConflict",
    "impl ClassificationKey",
    "impl ClassificationLabel",
    "impl ClassificationSet",
    "impl ConceptStance",
    "impl ConcreteNeed",
    "impl ControllingMechanism",
    "impl CurrentBasis",
    "impl From<academic_repository_analysis::AnalysisError> for ClassificationError",
    "impl GoalId",
    "impl GoalScope",
    "impl LifecycleRow",
    "impl LocatorMigration",
    "impl MigratedFinding",
    "impl MigratedSite",
    "impl MigrationOutcome",
    "impl NeedKind",
    "impl ObservedProof",
    "impl Outlook",
    "impl OverrideDecision",
    "impl ProjectConceptRequirement",
    "impl ProofChain",
    "impl RequiredConcept",
    "impl RequirementId",
    "impl ResolutionStatus",
    "impl RetirementReason",
    "impl TradeOff",
    "impl Trigger",
    "impl TriggerState",
    "impl UnmatchedReason",
    "impl UserEvidenceGap",
    "impl UserOverride",
];

// ---------------------------------------------------------------------------
// The `impl` header inventory.
// ---------------------------------------------------------------------------

/// Every `impl` header of `code`, from `impl` to the brace that opens it.
///
/// A header may be wrapped across lines, so reading continues until the block
/// opens. An `impl Trait` in argument position never begins a line — a
/// parameter list always puts a name and a colon in front of it — so the line
/// anchor is what separates the two.
fn impl_headers(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut lines = code.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if !(trimmed == "impl" || trimmed.starts_with("impl ") || trimmed.starts_with("impl<")) {
            continue;
        }
        let mut header = trimmed.to_owned();
        while !header.contains('{') {
            let Some(next) = lines.next() else {
                break;
            };
            header.push(' ');
            header.push_str(next.trim());
        }
        let end = header.find('{').unwrap_or(header.len());
        found.insert(tighten(&header[..end]).trim().to_owned());
    }
    found
}

/// Traits whose whole purpose is to fold one value into another.
///
/// A conversion, an addition or a dereference from one of this crate's types
/// hands a caller a second reading of the same value, and nothing in a `pub fn`
/// inventory can see one. The list is refused as a property of the whole
/// header inventory rather than of named type pairs, so a fold between two
/// types nobody thought of is refused too.
const FOLDING_TRAITS: [&str; 15] = [
    "Add",
    "AddAssign",
    "Sum",
    "Product",
    "Mul",
    "MulAssign",
    "Deref",
    "DerefMut",
    "AsRef<",
    "AsMut<",
    "Borrow<",
    "BorrowMut<",
    "FromIterator<",
    "IntoIterator",
    "Index",
];

/// Scalar types a conversion out of one of this crate's types must not reach.
const SCALAR_TARGETS: [&str; 14] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32",
    "f64",
];

/// Every `impl` header this crate declares is in the inventory, both ways.
///
/// `P2-A5` measured this bypass class open across R1 to R5. It injected
///
/// ```text
/// impl From<&PromotionSet> for u32 {
///     fn from(set: &PromotionSet) -> Self { … }
/// }
/// ```
///
/// into `academic-repository-competency` — a conversion that folds section
/// 17.6's project half and personal half into one number, which is exactly the
/// separation the crate exists to keep — and it passed 1543 tests over 265
/// binaries with nothing in the repository seeing it. A trait `impl` declares
/// no `pub fn`, so a signature inventory that looks for `pub fn ` and
/// `pub const fn ` is blind to one by construction.
///
/// This is `P2-R6`'s `every_impl_header_in_this_crate_is_in_the_inventory`
/// ported here, which is where the class was first closed.
#[test]
fn every_impl_header_in_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        found.extend(impl_headers(&code));
    }
    assert_eq!(
        found,
        IMPL_HEADERS.iter().map(|item| (*item).to_owned()).collect(),
        "the impl-header inventory and the source disagree"
    );

    for header in &found {
        for folding in FOLDING_TRAITS {
            assert!(
                uses_of(header, folding) == 0,
                "{header} implements {folding}, which this crate does not admit"
            );
        }
        if !(header.contains("From<") || header.contains("Into<")) {
            continue;
        }
        for scalar in SCALAR_TARGETS {
            assert!(
                uses_of(header, scalar) == 0,
                "{header} converts to or from {scalar}"
            );
        }
    }

    // The reader is not vacuous, in both directions: it finds a header in a
    // fragment that has one — the exact shape `P2-A5` injected — and this
    // crate really declares some, so the property above is a statement about
    // something rather than about an empty set.
    let fragment = "impl From<&PromotionSet> for u32 {\n    fn from(_: &PromotionSet) -> Self {\n        0\n    }\n}\n";
    assert_eq!(
        impl_headers(fragment),
        ["impl From<&PromotionSet> for u32"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(
        !found.is_empty(),
        "this crate declares no impl header, so the refusals above say nothing"
    );
    Ok(())
}
