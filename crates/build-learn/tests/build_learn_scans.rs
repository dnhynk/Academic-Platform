//! What `academic-build-learn` may reach, hold and hand out.
//!
//! ## Why a forbidden-name list is not enough, measured in this run
//!
//! Three bypass classes have been measured in this repository, each of which
//! defeats a list of names and none of which defeats a whole-set comparison:
//!
//! * `P2-R2` measured seven spellings of a filesystem or environment reach that
//!   compile, spell none of the listed tokens and add no `use` item. The repair
//!   was three whole-set comparisons: every `use` item, every two-segment path
//!   reached through a crate root, and every macro invoked.
//! * `P2-Y3` measured a **`From`-impl conversion escaping every `pub fn`
//!   sweep**, because a trait implementation declares no `pub fn` at all.
//!   `P2-X5` then measured the same shape as a still-open instance:
//!   `impl From<FieldCoverage> for u32` in `academic-blind-spot`, summing every
//!   source's evidence count, passes that crate's whole suite and its
//!   114-signature inventory. The repair is pinning every `impl` header.
//! * `P2-N7` measured **five public functions that a five-name list could not
//!   see**, because none of them spelled one of the five names. The repair was a
//!   whole-set inventory of every public signature.
//!
//! All three repairs are here, because all three shapes exist in this crate:
//! [`IMPL_HEADERS`] pins every `impl` header, [`PUBLIC_SIGNATURES`] pins every
//! public signature, and [`USE_ITEMS`], [`REACHED_PATHS`] and [`MACROS_SPELLED`]
//! close the reach.
//!
//! ## The four rules of this task that are pinned rather than described
//!
//! `no_signature_folds_the_motivation_edges` compares the whole set of public
//! signatures that name a motivation type **and** return a number against the
//! empty set, with both halves shown separately non-empty and the predicate
//! shown to bite on a fragment that *does* fold. That is section 20.3's
//! `합산 점수로 숨기지 않고`, stated over every signature rather than over a
//! list of names — and `every_impl_header_in_this_crate_is_in_the_inventory`
//! closes the half no signature sweep can see, by refusing every conversion
//! trait over the whole inventory for any type pair at all.
//!
//! `the_only_producer_of_a_technology_slate_takes_a_goal` compares the whole set
//! of public functions returning a `TechnologySlate` against one, and pins that
//! its parameter is a `&ProjectGoal`. That is
//! `criteria_and_choices_precede_technology`'s other half: the acceptance suite
//! shows the one producer behaves, and this shows there is no second.
//!
//! `the_only_producer_of_a_learning_item_takes_both` does the same for
//! `LearningItem`, and additionally requires the producer's parameter list to
//! name both `EvidenceTask` and `ReturnCheckpoint`.
//!
//! `the_build_learn_crate_holds_no_phrase_list` observes that the product
//! sources hold no string literal long enough to be a phrase to match against,
//! outside the design document's own quoted cells. That is `P2-N5`'s
//! `the_gap_crate_holds_no_phrase_list` for this crate's validator: a plan is
//! refused for what it structurally lacks, not for what it says.

use std::{
    collections::BTreeSet,
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

/// Every `.rs` of this package, tests included.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` of this package outside `tests/`.
fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    let mut found = crate_all_sources()?;
    found.retain(|path| {
        !path
            .strip_prefix(&root)
            .unwrap_or(path)
            .starts_with("tests")
    });
    Ok(found)
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

/// Collapses whitespace so a rewrapped signature still matches its pin.
fn tighten(code: &str) -> String {
    code.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// This crate's product files, as code with comments and literals removed.
fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

/// Every `use` item of the product tree, one per imported leaf.
fn use_items(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = code;
    while let Some(at) = rest.find("use ") {
        let before_ok = at == 0
            || !(rest.as_bytes()[at - 1].is_ascii_alphanumeric()
                || rest.as_bytes()[at - 1] == b'_');
        let tail = &rest[at + 4..];
        let Some(end) = tail.find(';') else {
            break;
        };
        if before_ok {
            found.insert(tighten(&tail[..end]));
        }
        rest = &tail[end + 1..];
    }
    found
}

/// Drops every `use` item, so an import is not counted as a reach.
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

/// Every two-segment path `code` spells through a crate root.
fn absolute_paths(code: &str) -> BTreeSet<String> {
    let roots = ["std", "core", "alloc", "serde", "thiserror"];
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

/// Every `impl` header of `code`, up to its opening brace.
///
/// This is the sweep `P2-Y3` measured a `From` implementation escaping: a trait
/// impl declares no `pub fn`, so a public-function inventory cannot see it.
fn impl_headers(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut lines = code.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if !(trimmed == "impl" || trimmed.starts_with("impl ") || trimmed.starts_with("impl<")) {
            continue;
        }
        // A header may be wrapped, so keep reading until the block opens. An
        // `impl Trait` in argument position is not a header and is skipped by
        // the line anchor above: it can never begin a line, because a parameter
        // list always puts a name and a colon in front of it.
        let mut header = trimmed.to_owned();
        while !header.contains('{') {
            let Some(next) = lines.next() else {
                break;
            };
            header.push(' ');
            header.push_str(next.trim());
        }
        let end = header.find('{').unwrap_or(header.len());
        found.insert(tighten(&header[..end]));
    }
    found
}

/// Every `pub fn` of `code`, as its name and its signature up to the body.
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
            found.push((name, tighten(&code[at..end])));
            cursor = after;
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Every named field of every `struct` and `enum` `code` declares.
fn declared_fields(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut depth = 0_usize;
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim();
        if !inside {
            let opens = trimmed.starts_with("struct ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub enum ");
            if opens && trimmed.contains('{') {
                inside = true;
                depth = 0;
            } else {
                continue;
            }
        }
        depth += trimmed.matches('{').count();
        let closes = trimmed.matches('}').count();
        if let Some((name, kind)) = trimmed.trim_start_matches("pub ").split_once(':') {
            let named = !name.is_empty()
                && name.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                });
            let kind = kind.trim().trim_end_matches(',');
            if named && !kind.is_empty() && !kind.contains('{') {
                found.insert(format!("{name}: {kind}"));
            }
        }
        depth = depth.saturating_sub(closes);
        if depth == 0 {
            inside = false;
        }
    }
    found
}

// ---------------------------------------------------------------------------
// The pinned inventories
// ---------------------------------------------------------------------------

/// Every `use` item of the product tree.
const USE_ITEMS: &[&str] = &[
    "academic_critical_path::is_stale",
    "academic_critical_path::{EdgeMember, EdgeStanding, Hyperedge, SatisfyingSet, satisfying_sets}",
    "academic_curriculum::{CourseOffering, CourseRevision, OfferingStatus}",
    "academic_domain::EntityId",
    "academic_domain::{CourseId, EntityId, EvidenceId, OfferingId}",
    "academic_domain::{EntityId, entity_registry::EntityKind}",
    "academic_domain::{FreshnessBand, MasteryLevel}",
    "academic_gap::ConceptState",
    "academic_gap::{PrerequisiteEdge, gap_bearing}",
    "academic_repository_classification::BenefitContract",
    "branch::{ArchitectureBranch, BranchGroup, ConceptRequirement, RequirementCondition}",
    "crate::BuildLearnError",
    "crate::text::{NonEmptyText, PartId}",
    "crate::{ BuildLearnError, goal::ProjectGoal, responsibility::ResponsibilityDecomposition, text::PartId, }",
    "crate::{ BuildLearnError, goal::ProjectGoal, text::{NonEmptyText, PartId}, }",
    "crate::{ BuildLearnError, input::{InputKind, NormalizedIntent}, text::{NonEmptyText, PartId}, }",
    "crate::{ branch::ArchitectureBranch, learning::LearningItem, motivation::MotivationDisplay, readiness::ReadinessFinding, text::{NonEmptyText, PartId}, }",
    "crate::{ branch::{ConceptRequirement, RequirementCondition}, text::PartId, }",
    "crate::{ goal::ProjectGoal, text::{NonEmptyText, PartId}, }",
    "crate::{ plan::{PlanDraft, PlanStep}, readiness::ReadinessCategory, text::PartId, }",
    "crate::{BuildLearnError, text::NonEmptyText}",
    "goal::{ Alternative, Constraint, Constraints, ObservableCriterion, ProjectGoal, SuccessCriteria, UnresolvedDecision, UnresolvedDecisions, }",
    "input::{GoalInput, INPUT_KINDS, InputKind, NormalizedIntent, normalize}",
    "learning::{ CHECKPOINT_STAGES, CheckpointStage, EvidenceTask, ExplainedByHand, LearningItem, ReadingDone, ReturnCheckpoint, SelectionApproved, SimulationPassed, }",
    "mapping::{ ActualCoverage, COVERAGE_EVIDENCE_KINDS, ChannelComparison, CourseProjectMapping, CoverageEvidenceKind, DesignedCoverage, EnrolmentStanding, MAPPING_STATUSES, MappingEvidence, MappingStatus, PersonalEvidenceStanding, }",
    "motivation::{MOTIVATIONS, Motivation, MotivationDisplay, MotivationEdge, MotivationRow}",
    "plan::{PlanDraft, PlanStep, STEP_KINDS}",
    "readiness::{ READINESS_CATEGORIES, RESOLUTION_ORDER, ROW_WITHOUT_A_SHORT_NAME, ReadinessCategory, ReadinessFinding, RequirementOrigin, SHORT_NAMES, categorize, }",
    "responsibility::{ObservableResponsibility, ResponsibilityDecomposition}",
    "serde::{Deserialize, Serialize}",
    "std::collections::BTreeSet",
    "std::collections::{BTreeMap, BTreeSet}",
    "technology::{TechnologyEntry, TechnologySlate}",
    "text::{NonEmptyText, PartId}",
    "validate::{PLAN_DEFECT_KINDS, PlanDefect, PlanVerdict, ValidatedPlan, validate}",
];

/// Every two-segment path the product tree reaches through a crate root.
const REACHED_PATHS: &[&str] = &[
    "academic_critical_path::CostEstimate",
    "academic_critical_path::CriticalPathError",
    "academic_critical_path::PrerequisiteHypergraph",
    "academic_gap::GapError",
    "thiserror::Error",
];

/// Every macro the product tree invokes.
const MACROS_SPELLED: &[&str] = &["matches"];

/// Every `impl` header the product tree declares.
const IMPL_HEADERS: &[&str] = &[
    "impl ActualCoverage",
    "impl Alternative",
    "impl ArchitectureBranch",
    "impl BranchGroup",
    "impl ChannelComparison",
    "impl CheckpointStage",
    "impl ConceptRequirement",
    "impl Constraint",
    "impl Constraints",
    "impl CourseProjectMapping",
    "impl CoverageEvidenceKind",
    "impl DesignedCoverage",
    "impl EvidenceTask",
    "impl ExplainedByHand",
    "impl From<NonEmptyText> for String",
    "impl From<PartId> for String",
    "impl From<SuccessCriteria> for Vec<ObservableCriterion>",
    "impl From<academic_critical_path::CriticalPathError> for BuildLearnError",
    "impl From<academic_gap::GapError> for BuildLearnError",
    "impl GoalInput",
    "impl InputKind",
    "impl LearningItem",
    "impl MappingStatus",
    "impl Motivation",
    "impl MotivationDisplay",
    "impl MotivationEdge",
    "impl MotivationRow",
    "impl NonEmptyText",
    "impl NormalizedIntent",
    "impl ObservableCriterion",
    "impl ObservableResponsibility",
    "impl PartId",
    "impl PlanDefect",
    "impl PlanDraft<'_>",
    "impl PlanStep",
    "impl PlanVerdict",
    "impl ProjectGoal",
    "impl ReadinessCategory",
    "impl ReadinessFinding",
    "impl ReadingDone",
    "impl RequirementCondition",
    "impl RequirementOrigin",
    "impl ResponsibilityDecomposition",
    "impl ReturnCheckpoint",
    "impl SelectionApproved",
    "impl SimulationPassed",
    "impl SuccessCriteria",
    "impl TechnologyEntry",
    "impl TechnologySlate",
    "impl TryFrom<String> for NonEmptyText",
    "impl TryFrom<String> for PartId",
    "impl TryFrom<Vec<ObservableCriterion>> for SuccessCriteria",
    "impl UnresolvedDecision",
    "impl UnresolvedDecisions",
    "impl ValidatedPlan",
];

/// The conversions this crate implements, in inventory order.
const CONVERSION_IMPLS: &[&str] = &[
    "impl From<NonEmptyText> for String",
    "impl From<PartId> for String",
    "impl From<SuccessCriteria> for Vec<ObservableCriterion>",
    "impl From<academic_critical_path::CriticalPathError> for BuildLearnError",
    "impl From<academic_gap::GapError> for BuildLearnError",
    "impl TryFrom<String> for NonEmptyText",
    "impl TryFrom<String> for PartId",
    "impl TryFrom<Vec<ObservableCriterion>> for SuccessCriteria",
];

/// Every public signature of the product tree.
const PUBLIC_SIGNATURES: &[&str] = &[
    "actual | pub const fn actual(&self) -> Option<&ActualCoverage>",
    "after | pub const fn after( simulation: SimulationPassed, decision: PartId, alternative: PartId, ) -> Self",
    "after | pub const fn after(explanation: ExplainedByHand, artifact: NonEmptyText) -> Self",
    "after | pub const fn after(reading: ReadingDone, property: NonEmptyText) -> Self",
    "alternative | pub const fn alternative(&self) -> &PartId",
    "alternative | pub const fn alternative(&self) -> Option<&PartId>",
    "alternative | pub fn alternative(&self, id: &PartId) -> Option<&Alternative>",
    "alternatives | pub fn alternatives(&self) -> &[Alternative]",
    "always | pub fn always( concept: EntityId, kind: EntityKind, serves: PartId, ) -> Result<Self, BuildLearnError>",
    "answers | pub const fn answers(&self) -> Option<&PartId>",
    "approved | pub const fn approved(&self) -> &SelectionApproved",
    "artifact | pub const fn artifact(&self) -> &NonEmptyText",
    "as_str | pub const fn as_str(&self) -> &'static str",
    "as_str | pub const fn as_str(self) -> &'static str",
    "as_str | pub fn as_str(&self) -> &str",
    "breadth | pub const fn breadth(&self) -> &academic_critical_path::CostEstimate",
    "capability | pub const fn capability(&self) -> &NonEmptyText",
    "carries | pub fn carries(&self, motivation: Motivation) -> bool",
    "categorize | pub fn categorize( requirement: &ConceptRequirement, origin: RequirementOrigin, state: &ConceptState, ) -> ReadinessFinding",
    "category | pub const fn category(&self) -> ReadinessCategory",
    "checkpoint | pub const fn checkpoint(&self) -> &ReturnCheckpoint",
    "competencies | pub fn competencies(&self) -> &[EntityId]",
    "concept | pub const fn concept(&self) -> EntityId",
    "concepts | pub fn concepts(&self) -> &[EntityId]",
    "condition | pub const fn condition(&self) -> &RequirementCondition",
    "conjunction | pub fn conjunction(&self) -> &[ConceptRequirement]",
    "constraints | pub const fn constraints(&self) -> &Constraints",
    "constraints | pub fn constraints(&self) -> &[Constraint]",
    "course | pub const fn course(&self) -> CourseId",
    "criteria | pub fn criteria(&self) -> &[ObservableCriterion]",
    "criterion | pub fn criterion(&self, id: &PartId) -> Option<&ObservableCriterion>",
    "decision | pub const fn decision(&self) -> &PartId",
    "decision | pub const fn decision(&self) -> Option<&PartId>",
    "decision | pub fn decision(&self, id: &PartId) -> Option<&UnresolvedDecision>",
    "decisions | pub fn decisions(&self) -> &[UnresolvedDecision]",
    "decompose | pub fn decompose( goal: ProjectGoal, responsibilities: Vec<ObservableResponsibility>, ) -> Result<Self, BuildLearnError>",
    "decomposition | pub const fn decomposition(&self) -> &ResponsibilityDecomposition",
    "defects | pub fn defects(&self) -> &[PlanDefect]",
    "designed | pub const fn designed(&self) -> Option<&DesignedCoverage>",
    "designs | pub fn designs(&self, subject: EntityId) -> bool",
    "disjunctions | pub fn disjunctions(&self) -> &[Vec<BranchGroup>]",
    "entries | pub fn entries(&self) -> &[TechnologyEntry]",
    "evidence_task | pub const fn evidence_task(&self) -> &EvidenceTask",
    "explanation | pub const fn explanation(&self) -> &ExplainedByHand",
    "failure_if_absent | pub const fn failure_if_absent(&self) -> &NonEmptyText",
    "fixed | pub const fn fixed(id: PartId, statement: NonEmptyText) -> Self",
    "for_evidence | pub fn for_evidence(evidence: &MappingEvidence<'_>) -> Self",
    "freshness | pub const fn freshness(&self) -> FreshnessBand",
    "goal | pub const fn goal(&self) -> &ProjectGoal",
    "hypergraph | pub fn hypergraph( &self, edges: &BTreeMap<EntityId, PrerequisiteEdge>, standing: EdgeStanding, ) -> Result<Vec<Hyperedge>, BuildLearnError>",
    "id | pub const fn id(&self) -> &PartId",
    "immediate_gap | pub const fn immediate_gap(&self) -> &academic_critical_path::CostEstimate",
    "implementation_steps | pub fn implementation_steps(&self) -> Vec<&PlanStep>",
    "is_accepted | pub const fn is_accepted(&self) -> bool",
    "is_empty | pub const fn is_empty(&self) -> bool",
    "is_upcoming | pub const fn is_upcoming(&self) -> bool",
    "kind | pub const fn kind(&self) -> &'static str",
    "kind | pub const fn kind(&self) -> EntityKind",
    "kind | pub const fn kind(&self) -> InputKind",
    "learning | pub const fn learning(&self) -> Option<&LearningItem>",
    "learning_items | pub fn learning_items(&self) -> Vec<&LearningItem>",
    "mastery | pub const fn mastery(&self) -> MasteryLevel",
    "meaning_token | pub const fn meaning_token(self) -> &'static str",
    "members | pub fn members(&self) -> &[ConceptRequirement]",
    "motivation | pub const fn motivation(&self) -> Motivation",
    "motivation | pub fn motivation(&self, concept: EntityId) -> Option<&MotivationDisplay>",
    "name | pub const fn name(&self) -> &NonEmptyText",
    "named | pub const fn named(id: PartId, name: NonEmptyText) -> Self",
    "new | pub fn new(value: impl Into<String>) -> Result<Self, BuildLearnError>",
    "normalize | pub fn normalize(input: &GoalInput) -> Result<NormalizedIntent, BuildLearnError>",
    "observed | pub fn observed( offering: &CourseOffering, subject: EntityId, sightings: Vec<(CoverageEvidenceKind, EvidenceId)>, upcoming: bool, ) -> Result<Self, BuildLearnError>",
    "observed_by | pub const fn observed_by(&self) -> &NonEmptyText",
    "of | pub const fn of( id: PartId, serves: PartId, statement: NonEmptyText, failure_if_absent: NonEmptyText, ) -> Self",
    "of | pub const fn of( subject: EntityId, immediate_gap: academic_critical_path::CostEstimate, breadth: academic_critical_path::CostEstimate, ) -> Self",
    "of | pub const fn of(approved: SelectionApproved, returns_to: PartId) -> Self",
    "of | pub const fn of(constraints: Vec<Constraint>) -> Self",
    "of | pub const fn of(decisions: Vec<UnresolvedDecision>) -> Self",
    "of | pub const fn of(motivation: Motivation, concept: EntityId, reason: NonEmptyText) -> Self",
    "of | pub const fn of(runs: NonEmptyText, shows: NonEmptyText) -> Self",
    "of | pub const fn of(source: NonEmptyText) -> Self",
    "of | pub fn of( decision: PartId, alternative: PartId, members: Vec<(EntityId, EntityKind, PartId)>, ) -> Result<Self, BuildLearnError>",
    "of | pub fn of( decomposition: ResponsibilityDecomposition, target: EntityId, conjunction: Vec<ConceptRequirement>, groups: Vec<BranchGroup>, ) -> Result<Self, BuildLearnError>",
    "of | pub fn of(concept: EntityId, edges: &[MotivationEdge]) -> Result<Self, BuildLearnError>",
    "of | pub fn of(criteria: Vec<ObservableCriterion>) -> Option<Self>",
    "of | pub fn of(revision: &CourseRevision) -> Self",
    "offering | pub const fn offering(&self) -> OfferingId",
    "open | pub fn open( id: PartId, question: NonEmptyText, alternatives: Vec<Alternative>, ) -> Result<Self, BuildLearnError>",
    "origin | pub const fn origin(&self) -> &RequirementOrigin",
    "plan | pub const fn plan( id: PartId, concept: EntityId, evidence_task: EvidenceTask, checkpoint: ReturnCheckpoint, ) -> Self",
    "plan | pub const fn plan(&self) -> Option<&ValidatedPlan>",
    "property | pub const fn property(&self) -> &NonEmptyText",
    "publish | pub fn publish( subject: EntityId, designed: Option<DesignedCoverage>, actual: Option<ActualCoverage>, status: MappingStatus, reason: NonEmptyText, ) -> Result<Self, BuildLearnError>",
    "question | pub const fn question(&self) -> &NonEmptyText",
    "reading | pub const fn reading(&self) -> &ReadingDone",
    "reason | pub const fn reason(&self) -> &NonEmptyText",
    "requirement | pub const fn requirement(&self) -> &ConceptRequirement",
    "requirements | pub fn requirements(&self) -> Vec<&ConceptRequirement>",
    "requires_actual_coverage | pub const fn requires_actual_coverage(self) -> bool",
    "responsibilities | pub fn responsibilities(&self) -> &[ObservableResponsibility]",
    "responsibility | pub fn responsibility(&self, id: &PartId) -> Option<&ObservableResponsibility>",
    "returns_to | pub const fn returns_to(&self) -> &PartId",
    "rows | pub fn rows(&self) -> &[MotivationRow]",
    "runs | pub const fn runs(&self) -> &NonEmptyText",
    "satisfies | pub const fn satisfies(&self) -> Option<&PartId>",
    "satisfying_sets | pub fn satisfying_sets( &self, edges: &BTreeMap<EntityId, PrerequisiteEdge>, standing: EdgeStanding, ) -> Result<Vec<SatisfyingSet>, BuildLearnError>",
    "serves | pub const fn serves(&self) -> &PartId",
    "serving | pub fn serving(&self, criterion: &PartId) -> Vec<&ObservableResponsibility>",
    "short_name | pub fn short_name(self) -> Option<&'static str>",
    "shows | pub const fn shows(&self) -> &NonEmptyText",
    "sightings | pub fn sightings(&self) -> &[(CoverageEvidenceKind, EvidenceId)]",
    "simulation | pub const fn simulation(&self) -> &SimulationPassed",
    "snapshot_id | pub const fn snapshot_id(&self) -> Option<&NonEmptyText>",
    "source | pub const fn source(&self) -> &NonEmptyText",
    "source | pub const fn source(&self) -> InputKind",
    "spec_token | pub const fn spec_token(self) -> &'static str",
    "stages | pub const fn stages(&self) -> [CheckpointStage; 4]",
    "stages | pub fn stages(&self) -> Vec<CoverageEvidenceKind>",
    "state | pub const fn state(id: PartId, statement: NonEmptyText, observed_by: NonEmptyText) -> Self",
    "state | pub fn state( intent: &NormalizedIntent, success_criteria: SuccessCriteria, constraints: Constraints, unresolved_decisions: UnresolvedDecisions, ) -> Result<Self, BuildLearnError>",
    "statement | pub const fn statement(&self) -> &NonEmptyText",
    "status | pub const fn status(&self) -> MappingStatus",
    "steps | pub fn steps(&self) -> &[PlanStep]",
    "subject | pub const fn subject(&self) -> EntityId",
    "success_criteria | pub const fn success_criteria(&self) -> &SuccessCriteria",
    "sufficiency_gap_count | pub const fn sufficiency_gap_count(&self) -> usize",
    "target | pub const fn target(&self) -> EntityId",
    "text | pub const fn text(&self) -> &NonEmptyText",
    "under | pub fn under(goal: &ProjectGoal) -> Self",
    "unresolved_decisions | pub const fn unresolved_decisions(&self) -> &UnresolvedDecisions",
    "validate | pub fn validate(draft: &PlanDraft<'_>) -> PlanVerdict",
];

/// Constructs no file of this package may spell.
const FORBIDDEN_CONSTRUCTS: &[&str] = &[
    "File",
    "OpenOptions",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "UnixStream",
    "Command",
    "SystemTime",
    "Instant",
    "now",
    "var",
    "var_os",
    "set_var",
    "current_dir",
    "extern",
    "unsafe",
    "libc",
    "rand",
    "thread_rng",
];

/// The longest string literal in `source`, and how long it is in characters.
///
/// Reads the **unstripped** text, because a phrase list would be string
/// literals and `strip_non_code` removes exactly those.
fn longest_literal(source: &str) -> (usize, String) {
    let bytes: Vec<char> = source.chars().collect();
    let mut index = 0;
    let mut longest = String::new();
    while index < bytes.len() {
        if bytes[index] == '/' && bytes.get(index + 1) == Some(&'/') {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index] == '"' {
            let mut held = String::new();
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
                held.push(bytes[index]);
                index += 1;
            }
            if held.chars().count() > longest.chars().count() {
                longest = held;
            }
            continue;
        }
        index += 1;
    }
    (longest.chars().count(), longest)
}

fn all_public_signatures() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for (_, code) in product_code()? {
        found.extend(public_signatures(&code));
    }
    found.sort();
    found.dedup();
    Ok(found)
}

// ---------------------------------------------------------------------------
// The scans
// ---------------------------------------------------------------------------

/// Every module the compiler pulls in is a file the walk read.
///
/// Without it, a reach could be moved into a file the walk never visits and
/// every scan below would pass over a package it had not read.
#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let root = crate_root().join("src");
    let lib = fs::read_to_string(root.join("lib.rs"))?;
    let declared: BTreeSet<String> = lib
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .filter_map(|rest| rest.split(';').next())
        .map(|name| format!("{name}.rs"))
        .collect();
    assert!(!declared.is_empty(), "lib.rs declares no module");

    let walked: BTreeSet<String> = crate_product_sources()?
        .iter()
        .filter_map(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    for module in &declared {
        assert!(walked.contains(module), "the walk did not read {module}");
    }
    // And the other direction: every file the walk read is either `lib.rs` or a
    // declared module, so a file nothing declares cannot be sitting in `src/`
    // holding a reach the compiler never sees but a reader would trust.
    for file in &walked {
        assert!(
            file == "lib.rs" || declared.contains(file),
            "{file} is in src/ and lib.rs declares no module for it"
        );
    }
    Ok(())
}

/// This crate opens no file, no socket, no process and no clock.
#[test]
fn the_build_learn_crate_touches_no_file_and_no_socket() -> TestResult {
    let mut items = BTreeSet::new();
    let mut reached = BTreeSet::new();
    let mut macros = BTreeSet::new();
    for (_, code) in product_code()? {
        items.extend(use_items(&code));
        reached.extend(absolute_paths(&without_use_items(&code)));
        macros.extend(macros_spelled(&code));
    }
    assert_eq!(
        items,
        USE_ITEMS.iter().map(|item| (*item).to_owned()).collect(),
        "the use-item inventory and the source disagree"
    );
    assert_eq!(
        reached,
        REACHED_PATHS
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the reached-path inventory and the source disagree"
    );
    assert_eq!(
        macros,
        MACROS_SPELLED
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the macro inventory and the source disagree"
    );

    // The whole package, tests included: nothing here opens a file, a socket, a
    // process or a clock. The test tree is swept too, because a fixture that
    // read a real repository would be as much a reach as a product file that did.
    let mut read = 0_usize;
    for path in crate_all_sources()? {
        if relative(&path).contains("/tests/support/") {
            // The one exception, and it is `academic-gap`'s fixture module
            // included by `#[path]`: that module opens a `tempfile` to drive a
            // real `P2-L2` capture. It is that crate's file and is swept by that
            // crate's own suite. Nothing this task wrote is skipped.
            continue;
        }
        let code = strip_non_code(&fs::read_to_string(&path)?);
        read += 1;
        for construct in FORBIDDEN_CONSTRUCTS {
            assert_eq!(
                uses_of(&code, construct),
                0,
                "{} spells {construct}",
                relative(&path)
            );
        }
    }
    assert!(
        read >= 14,
        "the forbidden-construct pass read only {read} files"
    );

    // **The control.** A reader that always answers zero would satisfy every
    // assertion above. Each construct is required to be found by the same
    // reader, through the same stripper, in a sample that does spell it — and to
    // be found in *code* rather than only in a literal, because
    // `strip_non_code` removes literals and a construct that only ever appeared
    // inside one would have been invisible either way.
    for construct in FORBIDDEN_CONSTRUCTS {
        let sample = format!("let value = {construct}(path);\n");
        assert_eq!(
            uses_of(&strip_non_code(&sample), construct),
            1,
            "the reader cannot find {construct} in a sample that spells it"
        );
        let quoted = format!("let value = \"{construct}\";\n");
        assert_eq!(
            uses_of(&strip_non_code(&quoted), construct),
            0,
            "the stripper does not remove {construct} from a string literal"
        );
    }
    Ok(())
}

/// Every `impl` header this crate declares is in the inventory, both ways.
///
/// This is `P2-Y3`'s and `P2-X5`'s bypass class closed for this crate's types. A
/// `From`, `Into`, `Sum`, `Add`, `Deref`, `AsRef`, `TryFrom` or `Borrow` from a
/// motivation value to a number would be an entry here and nowhere else — no
/// `pub fn` sweep can see one, which is exactly how
/// `impl From<FieldCoverage> for u32` still passes `academic-blind-spot`'s
/// whole suite.
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

    // No conversion or arithmetic trait is implemented for any type this crate
    // declares that section 20.3 is about. Stated as a property of the whole
    // inventory rather than of a list of type pairs, so a fold between two types
    // nobody thought of is refused too.
    let folding = [
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
    for header in &found {
        for trait_name in folding {
            assert!(
                !header.contains(trait_name),
                "{header} implements {trait_name}, which this crate does not admit"
            );
        }
    }

    // The four conversion traits this crate *does* implement are exactly the
    // `serde` round trips of its two text wrappers and of the two non-empty
    // collections, compared as a whole set. A conversion added for any other
    // type is an extra entry here.
    let conversions: BTreeSet<&String> = found
        .iter()
        .filter(|header| {
            header.contains("From<") || header.contains("TryFrom<") || header.contains("Into<")
        })
        .collect();
    assert_eq!(
        conversions
            .iter()
            .map(|header| (*header).clone())
            .collect::<Vec<String>>(),
        CONVERSION_IMPLS
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<String>>(),
        "the set of conversions this crate implements changed"
    );
    for header in &conversions {
        for forbidden in [
            "u8", "u16", "u32", "u64", "usize", "i32", "i64", "f32", "f64",
        ] {
            assert!(
                !header.contains(forbidden),
                "{header} converts to or from {forbidden}"
            );
        }
    }

    // The scanner is not vacuous: it finds a conversion in a fragment that has
    // one, and it finds the trait impls this crate really does declare.
    let fragment = "impl From<MotivationDisplay> for u32 {
    fn from(_: MotivationDisplay) -> Self { 0 }
}";
    assert_eq!(
        impl_headers(fragment),
        ["impl From<MotivationDisplay> for u32"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(
        found
            .iter()
            .any(|header| header.contains("TryFrom<String>"))
    );
    Ok(())
}

/// Every public signature of the crate is in the inventory, both ways.
///
/// This is `P2-N7`'s bypass class closed: a name list cannot see a function that
/// spells none of its names, and a whole-set inventory sees every function
/// whatever it is called.
#[test]
fn every_public_signature_is_in_the_inventory() -> TestResult {
    let found = all_public_signatures()?;
    let rendered: Vec<String> = found
        .iter()
        .map(|(name, signature)| format!("{name} | {signature}"))
        .collect();
    assert_eq!(
        rendered,
        PUBLIC_SIGNATURES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "the public-signature inventory and the source disagree"
    );
    assert!(
        found.len() >= 90,
        "only {} public functions were found",
        found.len()
    );
    Ok(())
}

/// Nothing in this crate turns the three motivation edges into a number.
///
/// Section 20.3's `UI는 이를 합산 점수로 숨기지 않고`. Compared as the whole set
/// of public signatures naming a motivation type **and** returning a numeric
/// type, with each half shown separately non-empty.
#[test]
fn no_signature_folds_the_motivation_edges() -> TestResult {
    let found = all_public_signatures()?;
    let numeric = [
        "-> u8",
        "-> u16",
        "-> u32",
        "-> u64",
        "-> usize",
        "-> i8",
        "-> i16",
        "-> i32",
        "-> i64",
        "-> isize",
        "-> f32",
        "-> f64",
        "-> Ordering",
    ];
    let names_motivation = |signature: &str| {
        [
            "Motivation",
            "MotivationEdge",
            "MotivationRow",
            "MotivationDisplay",
        ]
        .iter()
        .any(|name| signature.contains(name))
    };
    let returns_number = |signature: &str| numeric.iter().any(|suffix| signature.contains(suffix));

    let folding: Vec<&(String, String)> = found
        .iter()
        .filter(|(_, signature)| names_motivation(signature) && returns_number(signature))
        .collect();
    assert!(
        folding.is_empty(),
        "these signatures fold the motivation edges into a number: {folding:?}"
    );

    // Each half is separately non-empty, so the emptiness above is an
    // intersection and not a predicate that matches nothing.
    assert!(
        found
            .iter()
            .any(|(_, signature)| names_motivation(signature)),
        "no public signature names a motivation type at all"
    );
    assert!(
        found.iter().any(|(_, signature)| returns_number(signature)),
        "no public signature returns a number at all"
    );

    // And the predicate bites on a fragment that does fold, read by the same
    // signature reader.
    let fragment = "impl MotivationDisplay {
    pub fn score(&self) -> u32 {
        self.rows.len() as u32
    }
}";
    let folded = public_signatures(fragment);
    assert_eq!(folded.len(), 1);
    assert!(names_motivation(&folded[0].1) || folded[0].1.contains("score"));
    assert!(returns_number(&folded[0].1));

    // The type carries no numeric payload either: no field of any declared type
    // in this crate is a floating-point number, and the one integral field is a
    // count the validator reads off `P2-N2`'s own list.
    let mut fields = BTreeSet::new();
    for (_, code) in product_code()? {
        fields.extend(declared_fields(&code));
    }
    assert!(!fields.is_empty(), "no declared field was found");
    for field in &fields {
        for forbidden in ["f32", "f64"] {
            assert!(
                !field.ends_with(forbidden),
                "{field} is a floating-point field"
            );
        }
    }
    let numeric_fields: BTreeSet<&String> = fields
        .iter()
        .filter(|field| {
            ["u8", "u16", "u32", "u64", "usize", "i32", "i64"]
                .iter()
                .any(|kind| field.ends_with(kind))
        })
        .collect();
    assert_eq!(
        numeric_fields.into_iter().cloned().collect::<Vec<String>>(),
        vec!["sufficiency_gap_count: usize".to_owned()],
        "a numeric field was added to this crate"
    );
    Ok(())
}

/// One producer of a technology slate, and it takes a goal.
#[test]
fn the_only_producer_of_a_technology_slate_takes_a_goal() -> TestResult {
    let producers: Vec<(String, String)> = all_public_signatures()?
        .into_iter()
        .filter(|(_, signature)| {
            signature.contains("-> Self") || signature.contains("TechnologySlate")
        })
        .filter(|(_, signature)| signature.contains("under"))
        .collect();
    assert_eq!(
        producers,
        vec![(
            "under".to_owned(),
            "pub fn under(goal: &ProjectGoal) -> Self".to_owned()
        )],
        "the set of technology-slate producers changed"
    );

    // And nothing else in the crate returns one. Stated over the whole set of
    // signatures rather than over the `impl TechnologySlate` block, so a free
    // function returning one is caught too.
    let returning: Vec<(String, String)> = all_public_signatures()?
        .into_iter()
        .filter(|(_, signature)| signature.contains("-> TechnologySlate"))
        .collect();
    assert!(
        returning.is_empty(),
        "a function outside the impl block returns a TechnologySlate: {returning:?}"
    );
    Ok(())
}

/// One producer of a learning item, and it takes both required parts.
///
/// The whole set of public signatures naming an `EvidenceTask` — which is every
/// function that could be handed one or hand one back, whatever it is called —
/// compared against two: the producer, which takes one **by value** together
/// with a `ReturnCheckpoint`, and the accessor, which returns a reference.
#[test]
fn the_only_producer_of_a_learning_item_takes_both() -> TestResult {
    let naming: Vec<(String, String)> = all_public_signatures()?
        .into_iter()
        .filter(|(_, signature)| signature.contains("EvidenceTask"))
        .collect();
    assert_eq!(
        naming,
        vec![
            (
                "evidence_task".to_owned(),
                "pub const fn evidence_task(&self) -> &EvidenceTask".to_owned()
            ),
            (
                "plan".to_owned(),
                "pub const fn plan( id: PartId, concept: EntityId, evidence_task: EvidenceTask, \
                 checkpoint: ReturnCheckpoint, ) -> Self"
                    .to_owned()
            ),
        ],
        "the set of signatures naming an EvidenceTask changed"
    );

    // Exactly one of the two takes an `EvidenceTask` by value, and it names a
    // `ReturnCheckpoint` in the same parameter list. So there is one way to
    // build a learning item and it needs both.
    let taking: Vec<&(String, String)> = naming
        .iter()
        .filter(|(_, signature)| signature.contains(": EvidenceTask"))
        .collect();
    assert_eq!(
        taking.len(),
        1,
        "more than one function takes an EvidenceTask"
    );
    assert!(
        taking[0].1.contains(": ReturnCheckpoint"),
        "the producer does not take a ReturnCheckpoint: {}",
        taking[0].1
    );

    // And nothing outside the `impl LearningItem` block returns one, so the
    // producer above is the only route to the type.
    let returning: Vec<(String, String)> = all_public_signatures()?
        .into_iter()
        .filter(|(_, signature)| signature.contains("-> LearningItem"))
        .collect();
    assert!(
        returning.is_empty(),
        "a function outside the impl block returns a LearningItem: {returning:?}"
    );
    Ok(())
}

/// The plan validator matches no phrase, because there is no phrase to match.
///
/// `P2-N5`'s `the_gap_crate_holds_no_phrase_list` for this crate. Every string
/// literal in the product tree is either an identifier, a stable spelling, an
/// error message, or one of the design document's own quoted cells; none is long
/// enough to be a sentence somebody could key a plan judgement on.
#[test]
fn the_build_learn_crate_holds_no_phrase_list() -> TestResult {
    // The validator's own file first, alone: it is the file that decides whether
    // a plan is refused, and it holds no literal at all beyond its stable
    // spellings.
    let validator = fs::read_to_string(crate_root().join("src").join("validate.rs"))?;
    let (length, longest) = longest_literal(&validator);
    assert!(
        length <= 40,
        "the validator holds a {length}-character literal: {longest:?}"
    );
    assert!(
        longest
            .chars()
            .all(|character| character.is_ascii_uppercase() || character == '_'),
        "the validator's longest literal is not a stable spelling: {longest:?}"
    );

    // And across the product tree the only literals longer than a stable
    // spelling are the design document's own cells and the error messages, none
    // of which the validator reads. Stated as: no product file outside the two
    // that hold spec tokens has a literal a plan judgement could key on.
    for path in crate_product_sources()? {
        let name = relative(&path);
        let source = fs::read_to_string(&path)?;
        let (length, longest) = longest_literal(&source);
        let holds_spec_tokens = name.ends_with("/input.rs")
            || name.ends_with("/readiness.rs")
            || name.ends_with("/learning.rs")
            || name.ends_with("/mapping.rs")
            || name.ends_with("/motivation.rs")
            || name.ends_with("/lib.rs")
            || name.ends_with("/goal.rs")
            || name.ends_with("/branch.rs")
            || name.ends_with("/responsibility.rs")
            || name.ends_with("/text.rs");
        assert!(
            holds_spec_tokens || length <= 40,
            "{name} holds a {length}-character literal: {longest:?}"
        );
    }

    // The reader is not vacuous: it finds a long literal in a sample that has
    // one, and it does not report a comment as a literal.
    let sample = "let phrase = \"a plan that only lists lectures is not a build to learn plan\";";
    assert!(longest_literal(sample).0 > 40);
    let commented = "// \"a plan that only lists lectures is not a build to learn plan\"\n";
    assert_eq!(longest_literal(commented).0, 0);
    Ok(())
}

/// No public function of this crate takes `&mut self`.
///
/// `P2-R4`'s `no_public_function_mutates_in_place`, for the reason that crate
/// gives: a correction is a new run over new evidence, not an edit of a
/// published value.
#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    let found = all_public_signatures()?;
    let mutating: Vec<&(String, String)> = found
        .iter()
        .filter(|(_, signature)| signature.contains("&mut self"))
        .collect();
    assert!(
        mutating.is_empty(),
        "these public functions mutate in place: {mutating:?}"
    );
    assert!(
        found
            .iter()
            .any(|(_, signature)| signature.contains("&self")),
        "no public function takes &self at all, so the sweep is vacuous"
    );
    Ok(())
}

/// This scan file is registered in the source-scan inventory.
///
/// `docs/contracts/policy-source-scans.md`'s table is the survey the next
/// person starts from, and two scans have been found missing from it. This
/// checks the row exists from the Rust side as well as from
/// `tools/policy-source-scan-inventory.test.mjs`.
#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(
        workspace_root()
            .join("docs")
            .join("contracts")
            .join("policy-source-scans.md"),
    )?;
    assert!(
        page.contains("crates/build-learn/tests/build_learn_scans.rs"),
        "this scan file has no row in the policy source scan table"
    );
    Ok(())
}

/// The helpers this file is built on are not vacuous.
///
/// Each reader is run over a fragment that has the shape it looks for and over
/// one that does not, so a reader that silently answered nothing would fail here
/// rather than making every comparison above pass over an empty set.
#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(
        use_items("use std::fmt;\nfn main() {}\n"),
        ["std::fmt"].into_iter().map(str::to_owned).collect()
    );
    assert!(use_items("fn main() {}\n").is_empty());

    assert_eq!(
        absolute_paths("let value = std::fs::read(path);"),
        ["std::fs"].into_iter().map(str::to_owned).collect()
    );
    assert!(absolute_paths("let value = read(path);").is_empty());
    assert!(
        absolute_paths(&without_use_items("use std::fs;\n")).is_empty(),
        "a use item was counted as a reach"
    );

    assert_eq!(
        macros_spelled("let held = vec![1];"),
        ["vec"].into_iter().map(str::to_owned).collect()
    );
    assert!(macros_spelled("let held = read(1);").is_empty());

    assert_eq!(
        impl_headers("impl Display for Motivation {\n}\n"),
        ["impl Display for Motivation"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(impl_headers("fn takes(value: impl Display) {}\n").is_empty());

    assert_eq!(
        public_signatures("pub fn one(value: u8) -> u8 {\n    value\n}\n"),
        vec![("one".to_owned(), "pub fn one(value: u8) -> u8".to_owned())]
    );
    assert!(public_signatures("fn one(value: u8) -> u8 {\n    value\n}\n").is_empty());

    assert_eq!(
        declared_fields("pub struct Held {\n    name: String,\n}\n"),
        ["name: String"].into_iter().map(str::to_owned).collect()
    );
    assert!(declared_fields("pub fn held(name: String) {}\n").is_empty());

    assert_eq!(strip_non_code("let a = \"x\"; // y\n").trim(), "let a =  ;");
    assert_eq!(tighten("a\n  b"), "a b");
    assert_eq!(uses_of("let now = 1;", "now"), 1);
    assert_eq!(uses_of("let known = 1;", "now"), 0);
    Ok(())
}
