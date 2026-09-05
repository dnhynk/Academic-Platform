//! What this crate's own source says, read as whole sets rather than as tokens.
//!
//! Every net here is a set compared in **both** directions against a pin. None
//! of them is a list of forbidden spellings, because a list of forbidden
//! spellings refuses the edits somebody thought of in advance and admits every
//! edit spelled differently — `P2-K6` put five key substitutions past one and
//! `P2-RF8` put six lane widenings past another.
//!
//! # What is here, and what is deliberately not
//!
//! **Not here: an inventory keyed on a line prefix.** `P2-A4`'s second audit
//! measured three routes out of `student-voice` that two whole-set inventories
//! over *spellings* could not see, and `P2-A5`'s fourth measured two more
//! through `pub const NAME: fn(&T) -> u32`. `P2-RF23`, `P2-RF25` and `P2-RF28`
//! moved the workspace onto an **item** reader for that reason, and
//! `crates/contracts/tests/item_inventory_scans.rs` is where it lives. This
//! crate is on that reader: its whole item set is pinned in
//! `crates/contracts/tests/pinned-items/dashboard.items`, and
//! `GpaFigure`, `SecondaryPercentage` and `PlanSnapshot` are closed types
//! there, so an item that reaches one of them is pinned workspace-wide and
//! fails **by name**. Nothing in this file collects on `pub fn `, `pub const
//! fn ` or `impl `, so nothing here is the shape that reader replaced.
//!
//! **Here: four whole-set nets over this crate's product text.**
//!
//! * every `use` item, in both directions — the edge claim;
//! * every two-segment path spelled through a crate root — the reach claim,
//!   which is the net `P2-R2` repaired from a token list and `P2-A5` repaired
//!   again after `<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)`
//!   walked past the repair. The reader is the canonical one, copied, and
//!   `the_reach_readers_are_one_reader` holds this copy to being that text;
//! * every macro invoked — the form `include_str!` used to arrive as;
//! * every capitalized identifier the product text holds. That last one is
//!   this crate's own backstop under the item pin: a `pub static`, a `pub
//!   const NAME: fn(..) -> ..`, a trait `impl`, an item written inside a
//!   function body and an item a macro expands to each introduce a capitalized
//!   name, and none of them has to be spelled in any particular way to be
//!   caught by a set that holds all of them.
//!
//! # The control
//!
//! `the_dashboard_crate_touches_no_file_and_no_socket` drives the form that
//! bypassed the repaired reader through this crate's own copy, so the copy is
//! measured rather than assumed to work.

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
/// Every capitalized identifier `code` spells, whatever it is.
///
/// Not keyed on a keyword, a visibility or a position. A type, a trait, an
/// enum variant, a `const`, a `static`, a macro's name and an item a macro
/// expands to all introduce one, so a set that holds every one of them refuses
/// an addition without having to predict its shape. `P2-A5` measured
/// `pub const STANDING_TOTAL: fn(&PromotionSet) -> u32` passing a whole
/// workspace; `STANDING_TOTAL` is a capitalized identifier and would be an
/// extra key here.
fn capitalized_identifiers(code: &str) -> BTreeSet<String> {
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    let mut index = 0_usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if !(byte.is_ascii_alphanumeric() || byte == b'_') {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
        {
            index += 1;
        }
        let word = &code[start..index];
        if word.starts_with(|first: char| first.is_ascii_uppercase()) {
            found.insert(word.to_owned());
        }
    }
    found
}

/// This crate's product names, as one `file name` key per occurrence.
fn product_names() -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut found = BTreeSet::new();
    for (path, code) in product_code()? {
        let file = path
            .rsplit('/')
            .next()
            .ok_or_else(|| format!("{path} has no file name"))?
            .to_owned();
        for name in capitalized_identifiers(&code) {
            found.insert(format!("{file} {name}"));
        }
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// The pinned sets
// ---------------------------------------------------------------------------

/// Every `use` item of the product tree.
const USE_ITEMS: &[&str] = &[
    "academic_curriculum::{ Course, CourseCode, CourseOffering, CourseRevision, Credits, CurriculumCategory, InstructorName, SectionCode, TermCode, }",
    "academic_curriculum::{CourseCode, TermCode}",
    "academic_domain::AttemptId",
    "academic_domain::OfferingId",
    "academic_domain::engines::ProofStatus",
    "academic_domain::{ ContentDigest, CourseId, CourseRevisionId, EntityId, OfferingId, ValidInterval, predicates::PredicateName, }",
    "academic_record::views::{DispositionReason, GpaValue, RepeatProof}",
    "academic_record::{ attempt::{AttemptHistory, AttemptStatus, CourseAttempt, RepeatStatus}, grade::GradeSymbol, plan::PlanScenarioChoice, policy::RecognitionDecision, term::TermKey, }",
    "academic_review::{BiasDisclosure, OfferingAggregate, ReviewScope}",
    "audit_state::{AuditState, AuditStateReading}",
    "course::{ CatalogIdentity, Connections, CourseDetail, CourseSection, CoverageEntry, CoverageReport, CoverageTab, OfferingRow, ReviewSection, }",
    "crate::DashboardError",
    "crate::{ AttemptTimeline, AuditStateReading, DashboardError, GpaFigure, GpaScope, OpenGate, SecondaryPercentage, }",
    "crate::{AuditStateReading, DashboardError}",
    "crate::{DashboardError, TimelineEntry}",
    "gate::OpenGate",
    "gpa::{GpaFigure, GpaProof, GpaScope}",
    "percentage::{BreakdownPart, RequirementBreakdown, SecondaryPercentage}",
    "planner::{ AxisReading, CandidateOffering, DragOutcome, MeetingSlot, PlanSnapshot, PlannerBoard, PlannerDimension, RequirementContribution, StaleInput, StaleMarking, WorkloadRange, }",
    "screen::{AcademicDashboard, DashboardLine, DashboardSection}",
    "timeline::{AttemptTimeline, FacetReading, LifecycleFacet, TimelineEntry}",
];

/// Every two-segment path the product tree reaches through a crate root.
const REACHED_PATHS: &[&str] = &["core::fmt", "thiserror::Error"];

/// Every macro the product tree invokes.
const MACROS_SPELLED: &[&str] = &["format"];

/// Every capitalized identifier of the product tree, by file.
const PRODUCT_NAMES: &[&str] = &[
    "audit_state.rs ALL",
    "audit_state.rs AuditState",
    "audit_state.rs AuditStateReading",
    "audit_state.rs Clone",
    "audit_state.rs Conflict",
    "audit_state.rs Copy",
    "audit_state.rs Debug",
    "audit_state.rs Display",
    "audit_state.rs Eq",
    "audit_state.rs Formatter",
    "audit_state.rs Hash",
    "audit_state.rs Needs",
    "audit_state.rs NotSatisfied",
    "audit_state.rs Ord",
    "audit_state.rs PartialEq",
    "audit_state.rs PartialOrd",
    "audit_state.rs ProofStatus",
    "audit_state.rs Remaining",
    "audit_state.rs Result",
    "audit_state.rs Satisfied",
    "audit_state.rs Self",
    "audit_state.rs Unknown",
    "course.rs ALL",
    "course.rs Assessed",
    "course.rs AssessedIn",
    "course.rs BiasDisclosure",
    "course.rs CatalogIdentity",
    "course.rs Clone",
    "course.rs Connections",
    "course.rs ContentDigest",
    "course.rs Copy",
    "course.rs Course",
    "course.rs CourseCode",
    "course.rs CourseDetail",
    "course.rs CourseDetailWithoutAnOffering",
    "course.rs CourseId",
    "course.rs CourseOffering",
    "course.rs CourseRevision",
    "course.rs CourseRevisionId",
    "course.rs CourseSection",
    "course.rs Coverage",
    "course.rs CoverageEntry",
    "course.rs CoverageReport",
    "course.rs CoverageTab",
    "course.rs Credits",
    "course.rs CurriculumCategory",
    "course.rs DashboardError",
    "course.rs Debug",
    "course.rs Default",
    "course.rs Designed",
    "course.rs DesignedToTeach",
    "course.rs EmptyField",
    "course.rs EntityId",
    "course.rs Eq",
    "course.rs Err",
    "course.rs Hash",
    "course.rs InstructorName",
    "course.rs Into",
    "course.rs MyRecord",
    "course.rs OfferingAggregate",
    "course.rs OfferingId",
    "course.rs OfferingRow",
    "course.rs Offerings",
    "course.rs OfficialIdentity",
    "course.rs Ok",
    "course.rs Option",
    "course.rs Ord",
    "course.rs PartialEq",
    "course.rs PartialOrd",
    "course.rs Practiced",
    "course.rs PracticedIn",
    "course.rs PredicateIsNotACoverageTab",
    "course.rs PredicateName",
    "course.rs Result",
    "course.rs ReviewScope",
    "course.rs ReviewSection",
    "course.rs Reviews",
    "course.rs SectionCode",
    "course.rs Self",
    "course.rs String",
    "course.rs Taught",
    "course.rs TaughtIn",
    "course.rs TermCode",
    "course.rs TimelineEntry",
    "course.rs ValidInterval",
    "course.rs Vec",
    "gate.rs ALL",
    "gate.rs BLOCKING",
    "gate.rs Clone",
    "gate.rs Copy",
    "gate.rs CurrentTermOfferingFacts",
    "gate.rs Debug",
    "gate.rs Eq",
    "gate.rs Hash",
    "gate.rs OpenGate",
    "gate.rs Ord",
    "gate.rs PartialEq",
    "gate.rs PartialOrd",
    "gate.rs ProfileAdditionalMajor",
    "gate.rs ProfileAdmissionYear",
    "gate.rs ProfileDegreeMode",
    "gate.rs ProfileExchangeOrTransfer",
    "gate.rs ProfileGraduationStandard",
    "gate.rs ProfileOfficialTranscript",
    "gate.rs Self",
    "gpa.rs ALL",
    "gpa.rs AttemptId",
    "gpa.rs AverageWithoutProof",
    "gpa.rs Clone",
    "gpa.rs Copy",
    "gpa.rs Cumulative",
    "gpa.rs DashboardError",
    "gpa.rs Debug",
    "gpa.rs Display",
    "gpa.rs DispositionReason",
    "gpa.rs Eq",
    "gpa.rs Err",
    "gpa.rs Formatter",
    "gpa.rs GpaFigure",
    "gpa.rs GpaProof",
    "gpa.rs GpaScope",
    "gpa.rs GpaValue",
    "gpa.rs Hash",
    "gpa.rs Known",
    "gpa.rs Major",
    "gpa.rs NoGradedAttempts",
    "gpa.rs Ok",
    "gpa.rs Ord",
    "gpa.rs PartialEq",
    "gpa.rs PartialOrd",
    "gpa.rs ProofOmitsUnknownAttempts",
    "gpa.rs RepeatProof",
    "gpa.rs Result",
    "gpa.rs Self",
    "gpa.rs Term",
    "gpa.rs Unknown",
    "gpa.rs Vec",
    "lib.rs AcademicDashboard",
    "lib.rs AttemptTimeline",
    "lib.rs AuditState",
    "lib.rs AuditStateReading",
    "lib.rs AverageWithoutProof",
    "lib.rs BreakdownPart",
    "lib.rs BreakdownPartOverflows",
    "lib.rs BreakdownPartRequiresNothing",
    "lib.rs BreakdownRepeatsARequirement",
    "lib.rs CandidateOffering",
    "lib.rs CatalogIdentity",
    "lib.rs Clone",
    "lib.rs Connections",
    "lib.rs CourseDetail",
    "lib.rs CourseDetailWithoutAnOffering",
    "lib.rs CourseSection",
    "lib.rs CoverageEntry",
    "lib.rs CoverageReport",
    "lib.rs CoverageTab",
    "lib.rs DashboardError",
    "lib.rs DashboardLine",
    "lib.rs DashboardSection",
    "lib.rs Debug",
    "lib.rs AxisReading",
    "lib.rs DragOutcome",
    "lib.rs EmptyField",
    "lib.rs Eq",
    "lib.rs Error",
    "lib.rs FacetReading",
    "lib.rs GpaFigure",
    "lib.rs GpaProof",
    "lib.rs GpaScope",
    "lib.rs LifecycleFacet",
    "lib.rs MeetingEndsBeforeItStarts",
    "lib.rs MeetingSlot",
    "lib.rs OfferingIsAlreadyPlaced",
    "lib.rs OfferingRow",
    "lib.rs OpenGate",
    "lib.rs PartialEq",
    "lib.rs PercentageOverAnUnsettledPart",
    "lib.rs PercentageWithoutBreakdown",
    "lib.rs PlanSnapshot",
    "lib.rs PlannerBoard",
    "lib.rs PlannerDimension",
    "lib.rs PredicateIsNotACoverageTab",
    "lib.rs ProofOmitsUnknownAttempts",
    "lib.rs RequirementBreakdown",
    "lib.rs RequirementContribution",
    "lib.rs ReviewSection",
    "lib.rs SecondaryPercentage",
    "lib.rs SnapshotOfAnEmptyBoard",
    "lib.rs SnapshotWithoutLabel",
    "lib.rs StaleInput",
    "lib.rs StaleMarking",
    "lib.rs String",
    "lib.rs TimelineEntry",
    "lib.rs WorkloadRange",
    "lib.rs WorkloadRangeIsInverted",
    "lib.rs WorkloadWithoutBasis",
    "percentage.rs AuditStateReading",
    "percentage.rs BreakdownPart",
    "percentage.rs BreakdownPartOverflows",
    "percentage.rs BreakdownPartRequiresNothing",
    "percentage.rs BreakdownRepeatsARequirement",
    "percentage.rs Clone",
    "percentage.rs DashboardError",
    "percentage.rs Debug",
    "percentage.rs EmptyField",
    "percentage.rs Eq",
    "percentage.rs Err",
    "percentage.rs Into",
    "percentage.rs Ok",
    "percentage.rs PartialEq",
    "percentage.rs PercentageOverAnUnsettledPart",
    "percentage.rs PercentageWithoutBreakdown",
    "percentage.rs RequirementBreakdown",
    "percentage.rs Result",
    "percentage.rs SecondaryPercentage",
    "percentage.rs Self",
    "percentage.rs String",
    "percentage.rs Vec",
    "planner.rs ALL",
    "planner.rs CandidateOffering",
    "planner.rs Clone",
    "planner.rs ConceptCompetencyExposure",
    "planner.rs Copy",
    "planner.rs CourseCode",
    "planner.rs CreditsConflictsAndPrerequisites",
    "planner.rs CreditsMoved",
    "planner.rs DashboardError",
    "planner.rs Debug",
    "planner.rs Default",
    "planner.rs AxisReading",
    "planner.rs DragOutcome",
    "planner.rs EmptyField",
    "planner.rs Eq",
    "planner.rs Err",
    "planner.rs FollowOnUnlock",
    "planner.rs GraduationRuleContribution",
    "planner.rs Hash",
    "planner.rs Into",
    "planner.rs MeetingEndsBeforeItStarts",
    "planner.rs MeetingMoved",
    "planner.rs MeetingSlot",
    "planner.rs OfferingId",
    "planner.rs OfferingIsAlreadyPlaced",
    "planner.rs OfferingIsGone",
    "planner.rs Ok",
    "planner.rs Ord",
    "planner.rs PartialEq",
    "planner.rs PartialOrd",
    "planner.rs PlanSnapshot",
    "planner.rs PlannerBoard",
    "planner.rs PlannerDimension",
    "planner.rs PrerequisitesMoved",
    "planner.rs ProjectAndRoleRelevance",
    "planner.rs RequirementContribution",
    "planner.rs Result",
    "planner.rs Self",
    "planner.rs SnapshotOfAnEmptyBoard",
    "planner.rs SnapshotWithoutLabel",
    "planner.rs Some",
    "planner.rs StaleInput",
    "planner.rs StaleMarking",
    "planner.rs String",
    "planner.rs TermCode",
    "planner.rs Vec",
    "planner.rs WorkloadRange",
    "planner.rs WorkloadRangeBasisAndBias",
    "planner.rs WorkloadRangeIsInverted",
    "planner.rs WorkloadWithoutBasis",
    "screen.rs ALL",
    "screen.rs AcademicDashboard",
    "screen.rs AppliedProfile",
    "screen.rs AttemptTimeline",
    "screen.rs AuditStateReading",
    "screen.rs AuditStates",
    "screen.rs Averages",
    "screen.rs Blocked",
    "screen.rs Clone",
    "screen.rs Copy",
    "screen.rs CreditsByCategory",
    "screen.rs DashboardError",
    "screen.rs DashboardLine",
    "screen.rs DashboardSection",
    "screen.rs Debug",
    "screen.rs Eq",
    "screen.rs GpaFigure",
    "screen.rs GpaScope",
    "screen.rs Hash",
    "screen.rs None",
    "screen.rs Ok",
    "screen.rs OpenGate",
    "screen.rs Option",
    "screen.rs Ord",
    "screen.rs PartialEq",
    "screen.rs PartialOrd",
    "screen.rs ProfileAdditionalMajor",
    "screen.rs ProfileAdmissionYear",
    "screen.rs ProfileDegreeMode",
    "screen.rs ProfileExchangeOrTransfer",
    "screen.rs ProfileGraduationStandard",
    "screen.rs ProfileOfficialTranscript",
    "screen.rs Result",
    "screen.rs SecondaryPercentage",
    "screen.rs Self",
    "screen.rs Some",
    "screen.rs SourceFreshness",
    "screen.rs String",
    "screen.rs Vec",
    "timeline.rs ALL",
    "timeline.rs Absent",
    "timeline.rs AttemptHistory",
    "timeline.rs AttemptId",
    "timeline.rs AttemptStatus",
    "timeline.rs AttemptTimeline",
    "timeline.rs Attempted",
    "timeline.rs Cancelled",
    "timeline.rs Clone",
    "timeline.rs Completed",
    "timeline.rs Copy",
    "timeline.rs CourseAttempt",
    "timeline.rs Debug",
    "timeline.rs EntryKind",
    "timeline.rs Eq",
    "timeline.rs FacetReading",
    "timeline.rs GradeSymbol",
    "timeline.rs Hash",
    "timeline.rs InProgress",
    "timeline.rs LifecycleFacet",
    "timeline.rs None",
    "timeline.rs NotApplicable",
    "timeline.rs NotRecognized",
    "timeline.rs Option",
    "timeline.rs Ord",
    "timeline.rs Original",
    "timeline.rs PartialEq",
    "timeline.rs PartialOrd",
    "timeline.rs PlanScenarioChoice",
    "timeline.rs Planned",
    "timeline.rs Present",
    "timeline.rs RecognitionDecision",
    "timeline.rs Recognized",
    "timeline.rs Registered",
    "timeline.rs Repeat",
    "timeline.rs RepeatStatus",
    "timeline.rs Repeated",
    "timeline.rs Replaced",
    "timeline.rs S",
    "timeline.rs SatisfactoryUnsatisfactory",
    "timeline.rs Self",
    "timeline.rs Some",
    "timeline.rs String",
    "timeline.rs Taken",
    "timeline.rs TermKey",
    "timeline.rs TimelineEntry",
    "timeline.rs Transferred",
    "timeline.rs U",
    "timeline.rs Undecided",
    "timeline.rs Unknown",
    "timeline.rs Vec",
    "timeline.rs Withdrawn",
];

/// The record vocabulary that *makes* an attempt of record.
///
/// Not a forbidden-token list: `planner_has_no_registration_endpoint` requires
/// each of these to be spelled in `academic-record`'s own product text before
/// it asserts anything about this crate, and drives the reader over a sample
/// that spells each. The whole-set name net above is what refuses an addition
/// nobody predicted; this names the one the specification's sentence is about.
const CONSTRUCTION_VOCABULARY: &[&str] = &["RegistrationConfirmation", "SettledStatus"];

/// The attempt-ledger vocabulary, which the planner surface does not read either.
///
/// `timeline.rs` reads two of these and section 25.4's fifth line is why: the
/// timeline is *of* the ledger. It reads them by borrow and returns none of
/// them, which is the second half of the test below.
const LEDGER_VOCABULARY: &[&str] = &[
    "AttemptHistory",
    "AttemptId",
    "AttemptStatus",
    "CourseAttempt",
    "RegistrationConfirmation",
    "SettledStatus",
];

/// The files that make up the planner surface.
const PLANNER_FILES: &[&str] = &[
    "crates/dashboard/src/planner.rs",
    "crates/dashboard/src/screen.rs",
];

/// Every ledger type this crate reads, and the file that reads it.
///
/// Each is required to appear **only** behind an ampersand, so every mention is
/// a borrow and none is a production. Written down so that a mention in a
/// second file fails here as well as in the whole-set name inventory.
const BORROWED_LEDGER_TYPES: &[(&str, &str)] = &[
    ("timeline.rs", "AttemptHistory"),
    ("timeline.rs", "CourseAttempt"),
];

/// Constructs no file of this package may spell, tests included.
///
/// The weakest of the layers and kept anyway, because it names the shapes a
/// reader expects to see refused. The whole-set comparisons above are what
/// actually close the reach.
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

// ---------------------------------------------------------------------------
// The scans
// ---------------------------------------------------------------------------

/// Every module the compiler pulls in is a file the walk read.
///
/// Without it a reach could be moved into a file the walk never visits and
/// every scan below would pass over a package it had not read. `P2-G5`
/// measured exactly that: a walk that did not descend read a flat tree
/// correctly and a subdirectory module not at all.
#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let read: BTreeSet<String> = crate_product_sources()?
        .iter()
        .map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        for line in code.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed
                .strip_prefix("mod ")
                .or_else(|| trimmed.strip_prefix("pub mod "))
            else {
                continue;
            };
            let Some(name) = rest.strip_suffix(';') else {
                continue;
            };
            declared.insert(name.trim().to_owned());
        }
    }
    assert!(
        declared.len() >= 8,
        "only {} module declarations were found",
        declared.len()
    );
    for module in &declared {
        assert!(read.contains(module), "{module} is compiled and not read");
    }
    for (path, code) in product_code()? {
        assert!(!code.contains("#[path"), "{path} includes a file by path");
    }
    Ok(())
}

/// The crate opens nothing, reads no clock and reaches nothing it does not
/// declare.
#[test]
fn the_dashboard_crate_touches_no_file_and_no_socket() -> TestResult {
    let files = product_code()?;
    assert!(
        files.len() >= 8,
        "only {} product files were read",
        files.len()
    );
    let whole: String = files
        .iter()
        .map(|(_, code)| code.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let items = use_items(&whole);
    assert_eq!(
        items,
        USE_ITEMS.iter().map(|item| (*item).to_owned()).collect(),
        "the use-item inventory and the source disagree"
    );

    let reached = absolute_paths(&without_use_items(&whole));
    assert_eq!(
        reached,
        REACHED_PATHS
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the reached-path inventory and the source disagree"
    );

    let macros = macros_spelled(&whole);
    assert_eq!(
        macros,
        MACROS_SPELLED
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "the macro inventory and the source disagree"
    );

    let mut read = 0_usize;
    for path in crate_all_sources()? {
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
        read >= 10,
        "the forbidden-construct pass read only {read} files"
    );

    // **The control.** A reader that always answers zero satisfies every
    // assertion above. Each construct is required to be found by the same
    // reader through the same stripper in a sample that does spell it, and to
    // be found in *code* rather than only in a literal.
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

    // **The second control**, and it is the form that walked past the repaired
    // reader. `P2-A5` measured
    // `<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)` resolving a
    // name from a live classification path while the guard reported nothing.
    // `the_reach_readers_are_one_reader` requires every crate that copies this
    // reader to drive that form through its own copy, and this is where this
    // crate does.
    let bypass = "<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)";
    let seen = absolute_paths(&without_use_items(bypass));
    assert!(
        seen.contains("std::net"),
        "this copy of the reader does not see the form that bypassed the repair: {seen:?}"
    );
    Ok(())
}

/// Every capitalized identifier this crate compiles is one somebody wrote down.
///
/// The backstop under this crate's own contract sentences, and it is keyed on
/// nothing: not a visibility, not a keyword, not a type name. A `pub static`, a
/// `pub const NAME: fn(&T) -> u32`, a trait `impl`, an `impl` block written
/// inside a function body and an item a macro expands to each introduce a
/// capitalized identifier, and each is an extra key here whatever it is called.
///
/// `crates/contracts/tests/item_inventory_scans.rs` holds the same statement
/// over **items** and is the primary; this is the copy that fails inside the
/// crate it is about, which is what `P2-A5` found missing when a crate suite
/// stayed green while the workspace pin was the only thing that saw the
/// injection.
#[test]
fn every_capitalized_identifier_in_this_crate_is_in_the_inventory() -> TestResult {
    let found = product_names()?;
    assert!(
        found.len() >= 100,
        "the name reader found only {} names",
        found.len()
    );
    assert_eq!(
        found,
        PRODUCT_NAMES
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        "this crate compiles names nobody wrote down, or no longer compiles ones that are pinned"
    );

    // **The control.** The reader must find each of the four shapes this net
    // exists for, in a sample that spells it and nothing else.
    for (shape, expected) in [
        (
            "pub const STANDING_TOTAL: fn(&PlanSnapshot) -> u32 = |_| 0;",
            "STANDING_TOTAL",
        ),
        (
            "pub static ESCAPE: fn(&GpaFigure) -> u32 = |_| 0;",
            "ESCAPE",
        ),
        (
            "impl From<&PlanSnapshot> for Vec<String> { }",
            "PlanSnapshot",
        ),
        ("macro_rules! whisper { () => { struct Held; }; }", "Held"),
    ] {
        assert!(
            capitalized_identifiers(shape).contains(expected),
            "the name reader cannot see {expected} in {shape}"
        );
    }
    assert!(
        capitalized_identifiers("let placed = board.placed();").is_empty(),
        "the name reader reports a local binding as a name"
    );
    Ok(())
}

/// Section 25.5's last sentence, as an absence over the whole surface.
///
/// > 사용자의 실제 수강신청을 자동 수행하지 않는다.
///
/// Four statements, and the first is what makes the rest worth making.
///
/// 1. **The vocabulary exists and is in this crate's closure.**
///    `academic-record` is a product dependency of this crate, and every name
///    in [`LEDGER_VOCABULARY`] is spelled in that crate's own product text. So
///    the absences below are about types this crate *can* name.
/// 2. **Nothing here makes an attempt of record.** No product file spells
///    [`CONSTRUCTION_VOCABULARY`] — the confirmation `CourseAttempt`'s first
///    constructor takes, and the argument type its second one takes. Without
///    either, neither constructor is callable from here.
/// 3. **The planner surface reads no ledger vocabulary at all.**
///    [`PLANNER_FILES`] spell none of [`LEDGER_VOCABULARY`]. Section 25.4's
///    fifth line is why `timeline.rs` is not on that list: the attempt timeline
///    *is* a reading of the ledger, and it lives on the dashboard rather than
///    in the planner.
/// 4. **What `timeline.rs` does read, it reads by borrow.** Every occurrence of
///    each type in [`BORROWED_LEDGER_TYPES`] is preceded by an ampersand, over
///    *all* occurrences rather than over the ones an inventory listed, so a
///    signature returning one is a failure here.
///
/// `P2-M4` made confirming a registration non-delegable:
/// `RegistrationConfirmation::new` takes no actor, so no agent may be asked to
/// stand in for the user. That is a different claim from this one. `P2-M4`'s is
/// about *who* may do it; this one is that the planner has no route to it at
/// all, delegated or otherwise. The workspace-wide half — every item anywhere
/// that reaches `RegistrationConfirmation`, pinned by name — is
/// `every_item_that_reaches_a_closed_type_is_pinned` in
/// `crates/contracts/tests/item_inventory_scans.rs`.
#[test]
fn planner_has_no_registration_endpoint() -> TestResult {
    // 1. The edge is declared, so the vocabulary is nameable from here.
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml"))?;
    let (dependencies, _) = manifest
        .split_once("[dev-dependencies]")
        .ok_or("the manifest has no dev-dependency section")?;
    assert!(
        dependencies.contains("academic-record = { path = \"../record\" }"),
        "academic-record is not a product edge, so the absences below are vacuous"
    );

    // 1b. And every name is really spelled there.
    let mut record_sources = Vec::new();
    walk(
        &workspace_root().join("crates/record/src"),
        &mut record_sources,
    )?;
    assert!(
        record_sources.len() >= 5,
        "only {} record sources were read",
        record_sources.len()
    );
    let mut record_names: BTreeSet<String> = BTreeSet::new();
    for path in &record_sources {
        record_names.extend(capitalized_identifiers(&strip_non_code(
            &fs::read_to_string(path)?,
        )));
    }
    for name in LEDGER_VOCABULARY {
        assert!(
            record_names.contains(*name),
            "{name} is not spelled in academic-record, so this check names nothing"
        );
    }

    // 2. Nothing here makes an attempt of record.
    let files = product_code()?;
    assert!(
        files.len() >= 8,
        "only {} product files were read",
        files.len()
    );
    for (path, code) in &files {
        let names = capitalized_identifiers(code);
        for name in CONSTRUCTION_VOCABULARY {
            assert!(
                !names.contains(*name),
                "{path} names {name}, which is what a CourseAttempt is built from"
            );
        }
    }

    // 3. The planner surface reads no ledger vocabulary at all.
    let mut planner_read = 0_usize;
    for (path, code) in &files {
        if !PLANNER_FILES.contains(&path.as_str()) {
            continue;
        }
        planner_read += 1;
        let names = capitalized_identifiers(code);
        for name in LEDGER_VOCABULARY {
            assert!(
                !names.contains(*name),
                "{path} names {name}, which is a route from the planner into the attempt ledger"
            );
        }
    }
    assert_eq!(
        planner_read,
        PLANNER_FILES.len(),
        "the planner surface named in PLANNER_FILES is not the set of files that were read"
    );

    // 4. What is read is read by borrow, over every occurrence.
    for (file, name) in BORROWED_LEDGER_TYPES {
        let mut occurrences = 0_usize;
        for (path, code) in &files {
            // The import that brings the type in names it without a borrow and
            // is not a use of it; every other occurrence is.
            let tight = tighten(&without_use_items(code));
            let bytes = tight.as_bytes();
            let holder = path.rsplit('/').next().unwrap_or(path);
            for (at, _) in tight.match_indices(name) {
                let after = bytes.get(at + name.len()).copied().unwrap_or(b' ');
                if after.is_ascii_alphanumeric() || after == b'_' {
                    continue;
                }
                assert_eq!(
                    holder, *file,
                    "{path} names {name}, and only {file} may read it"
                );
                occurrences += 1;
                let before = at.checked_sub(1).map(|index| bytes[index]);
                assert_eq!(
                    before,
                    Some(b'&'),
                    "{path} spells {name} at byte {at} without a borrow, so it may be produced \
                     rather than read"
                );
            }
        }
        assert!(
            occurrences > 0,
            "{name} is pinned as borrowed in {file} and appears nowhere, so this check is empty"
        );
    }

    // 5. The controls. The reader finds each name when it is there, and the
    // borrow rule fails on a production rather than passing it.
    for name in LEDGER_VOCABULARY {
        let sample = format!("pub fn open(value: &{name}) -> u32 {{ 0 }}\n");
        assert!(
            capitalized_identifiers(&strip_non_code(&sample)).contains(*name),
            "the name reader cannot see {name} in a sample that spells it"
        );
    }
    for (name, spelled, borrowed) in [
        ("CourseAttempt", "fn mint() -> CourseAttempt { }", false),
        ("CourseAttempt", "fn read(value: &CourseAttempt) { }", true),
    ] {
        let tight = tighten(&strip_non_code(spelled));
        let bytes = tight.as_bytes();
        let at = tight
            .find(name)
            .ok_or_else(|| format!("the control sample does not spell {name}"))?;
        let before = at.checked_sub(1).map(|index| bytes[index]);
        assert_eq!(
            before == Some(b'&'),
            borrowed,
            "the borrow rule reads {spelled} the wrong way round"
        );
    }
    Ok(())
}

/// The dashboard surface has no name for the other half of a composite.
///
/// Section 10: *Academic Dashboard에서 GPA chart와 Knowledge Map을 같은 카드의
/// 한 score로 합치지 않는다.* `P2-X2` holds the same line the same way, and the
/// reason it is a *source* claim rather than a behavioural one is that a sum of
/// two numbers is not observable as a shape — what is observable is that one of
/// the two operands cannot be spelled here at all.
///
/// The check is over the whole set of `use` items and the whole set of
/// capitalized identifiers, both pinned above, so this test states which names
/// the pinned sets must not hold rather than sweeping for spellings of its own.
#[test]
fn the_dashboard_surface_cannot_name_a_mastery() -> TestResult {
    let absent = [
        "MasteryLevel",
        "KnowledgeState",
        "ConceptReading",
        "FreshnessBand",
        "FreshnessProjection",
    ];
    let mut names: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        names.extend(capitalized_identifiers(&code));
    }
    assert!(!names.is_empty(), "the name reader read nothing");
    for name in absent {
        assert!(
            !names.contains(name),
            "a product file names {name}, so a card here could hold a mastery beside an average"
        );
    }
    // The control: `MasteryLevel` and `FreshnessBand` are `academic-domain`'s
    // own, and `academic-domain` *is* a product edge — so these two are one
    // `use` away and are not written, rather than being unspellable for a
    // reason nobody chose.
    let domain = fs::read_to_string(workspace_root().join("crates/domain/src/lib.rs"))?;
    let domain_names = capitalized_identifiers(&strip_non_code(&domain));
    for name in ["MasteryLevel", "FreshnessBand"] {
        assert!(
            domain_names.contains(name),
            "{name} is not academic-domain's, so this check names nothing"
        );
    }
    Ok(())
}

/// This scan file is registered in the policy-source-scan inventory.
#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for name in [
        "crates/dashboard/tests/dashboard_scans.rs",
        "crates/dashboard/tests/dashboard.rs",
    ] {
        assert!(
            page.contains(name),
            "{name} has no row in docs/contracts/policy-source-scans.md"
        );
    }
    Ok(())
}
