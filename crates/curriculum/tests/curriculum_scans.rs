//! Source scans for the `P2-U1` curriculum boundary.
//!
//! Four of this task's claims are shapes of the source rather than behaviours,
//! so nothing at run time would notice the day they stopped being true:
//! that the three section 9 boundaries are the specification's own lists, that
//! no relation derives another, that no course identity is inferred, and that
//! the publication has one rewind every failure takes.
//! `docs/contracts/policy-source-scans.md` is the page those scans are
//! enumerated on, and this file is written against all five of the empty-scan
//! shapes it names.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends the whole
//! package, not `src` by name, with a floor, a `mod`/`#[path]` tripwire, and a
//! rule that this crate's product source is under `src` and nowhere else.
//! `S-12` on that page is the row about a walk rooted at `<crate>/src`.
//!
//! **The checks are not token lists.** The forbidden-field lists are read out
//! of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, so a name dropped
//! from a Rust list fails against the specification rather than against a list
//! written here twice. The relation independence is the whole `impl` set and
//! the whole signature set of `relation.rs`, so a conversion nobody predicted
//! fails as an extra key.
//!
//! **The pins fix their callers.** [`WHOLE_PUBLISH`] is accompanied by a call
//! count of [`CurriculumPublisher::append`] and by
//! [`WHOLE_REWIND`], because `T141` found a pinned check skipped by a
//! condition wrapped around it and `T149` found a second path that never
//! called one.
//!
//! [`CurriculumPublisher::append`]: academic_curriculum::CurriculumPublisher
//!
//! **Every inventory counts a name, not a spelling.** The counts here are
//! whole-identifier counts of the function's own name with declarations
//! subtracted, so a call written through the type path counts the same as a
//! method call.
//!
//! **The floors bound the coverage.** A walk that returned nothing would pass
//! every loop below it, so each loop has a floor and each whole-set comparison
//! fails on a missing key as well as an extra one.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_curriculum::{
    CohortTransition, CourseCodeReuse, CourseRelationKind, CurriculumCategory, GradingMode,
    OfferingStatus, OpenGate, PublicationStatus, PublishCheckpoint,
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
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships: everything outside `tests`.
///
/// `benches` was excluded beside `tests` when this file was written. `S-14`
/// closed that for the five walks that existed then, on the measurement that a
/// bench target has no feature gate and `cargo clippy --workspace
/// --all-targets` compiles it — the test `T146` applied to `examples/`. This
/// walk arrived in the same window and is widened for the same reason. No
/// `benches` tree exists in this repository.
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

/// Every `.rs` file under every workspace package, less each package's `tests`.
///
/// The package rather than its `src`, for `S-12`'s reason: `crates/record`
/// ships an `examples/` tree and `crates/worker` a `probes/` tree, and both are
/// product-shaped code a walk rooted at `src` never reads. `benches` is walked
/// for `S-14`'s reason, which is the same one a layer out.
fn workspace_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let crates = workspace_root().join("crates");
    let mut found = Vec::new();
    for entry in fs::read_dir(&crates)? {
        let package = entry?.path();
        if !package.is_dir() {
            continue;
        }
        let mut inside = Vec::new();
        walk(&package, &mut inside)?;
        for path in inside {
            let relative = path.strip_prefix(&package).unwrap_or(&path).to_path_buf();
            if relative.starts_with("tests") {
                continue;
            }
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
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
/// Copied from `crates/record/tests/record_scans.rs` by way of
/// `crates/untrusted-content/tests/trust_scans.rs`, raw strings and nested
/// block comments included. `P2-G4` found that a lexer without raw strings
/// desynchronizes and reads every literal after one as code.
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
///
/// `crates/consent/tests/consent_scans.rs`'s `declared_item` ends at a closing
/// brace in column zero, which is what a file-scope item has. A method inside
/// an `impl` block does not: its brace is indented, and ending at column zero
/// would swallow every method after it.
/// `crates/egress-boundary/tests/byte_path_pin.rs` passes the terminator for
/// the same reason.
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

/// How many times `needle` appears in `code`.
fn occurrences(code: &str, needle: &str) -> usize {
    code.split(needle).count().saturating_sub(1)
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

/// Drops every `use` item, so a re-export is not counted as a caller.
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

/// Every `pub` function signature in `code`, whitespace-collapsed.
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
fn parameters_and_return(signature: &str) -> Option<(&str, &str)> {
    let open = signature.find('(')?;
    let mut depth = 0_usize;
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

/// How many times `name` is called in `code`, declarations subtracted.
///
/// The uses are counted by whole identifier, so a call written through the
/// module path counts the same as a bare one -- `P2-RF10` reached a fourth site
/// in another crate by changing only the spelling of a call. The declarations
/// are subtracted by the `fn name(` prefix rather than by `fn name`, because
/// `fn name` is a substring of `fn named` and this crate has both an `inherit`
/// and an `inherited`.
fn calls_of(code: &str, name: &str) -> usize {
    uses_of(code, name).saturating_sub(occurrences(code, &format!("fn {name}(")))
}

/// The variant names an enum declares, in declaration order.
///
/// Read out of the source rather than derived from the type, so a variant added
/// to the enum without being added to whatever else names it fails here.
fn enum_variants(source: &str, header: &str) -> Vec<String> {
    source
        .lines()
        .skip_while(|line| !line.contains(header))
        .skip(1)
        .take_while(|line| !line.starts_with('}'))
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("///")
                && !line.starts_with("//")
                && !line.starts_with('#')
        })
        .map(|line| line.trim_end_matches(',').to_owned())
        .collect()
}

/// The quoted spellings inside the first `<column> IN ( … )` list in `sql`.
///
/// Returns them sorted, so the comparison is against a set rather than against
/// the order somebody happened to write the `CHECK` in.
fn sql_check_list(sql: &str, column: &str) -> Vec<String> {
    let Some(start) = sql.find(&format!("{column} IN (")) else {
        return Vec::new();
    };
    let rest = &sql[start..];
    let Some(end) = rest.find(')') else {
        return Vec::new();
    };
    let mut found: Vec<String> = rest[..end]
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    found.sort();
    found
}

/// One file, as code with comments and literals removed.
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

/// Every `impl` block header in `code` that names `type_name` as a whole
/// identifier, whitespace-collapsed.
///
/// The whole set is compared against a pinned list, so an implementation of a
/// trait nobody predicted appears as an extra key. An `impl` in another crate
/// is refused by the orphan rule instead: both the trait and the type would be
/// foreign there.
fn impl_headers_naming(code: &str, type_name: &str) -> Vec<String> {
    let mut found = Vec::new();
    let lines: Vec<&str> = code.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with("impl") {
            continue;
        }
        let mut header = String::new();
        for follow in lines.iter().skip(index) {
            header.push(' ');
            header.push_str(follow.trim());
            if follow.contains('{') {
                break;
            }
        }
        let header = header.split_whitespace().collect::<Vec<_>>().join(" ");
        if uses_of(&header, type_name) > 0 {
            found.push(header);
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Whole-text pins. Each is compared against the item as the source declares it,
// comment lines dropped and whitespace collapsed, so `cargo fmt` decides layout
// and the pin decides content. What editing one costs is in
// `docs/contracts/policy-source-scans.md`.
// ---------------------------------------------------------------------------

/// The whole identity lookup. What it reads, and what it returns when nothing
/// addresses the pair.
const WHOLE_SAME_COURSE: &str = "pub fn same_course( &self, earlier: CourseId, later: CourseId, instant: TimestampMillis, ) -> CourseCodeReuse { self.identities .iter() .find(|decision| { decision.earlier == earlier && decision.later == later && decision.valid_time.contains(instant) }) .map_or(CourseCodeReuse::Unknown, |decision| decision.verdict()) }";

/// The whole equivalence lookup. Directional, and effective-dated.
const WHOLE_EQUIVALENT: &str = "pub fn equivalent(&self, source: CourseId, target: CourseId, instant: TimestampMillis) -> bool { self.equivalences.iter().any(|relation| { relation.source == source && relation.target == target && relation.valid_time.contains(instant) }) }";

/// The whole retirement lookup. It reads one set and names no replacement.
const WHOLE_RETIRED: &str = "pub fn retired(&self, course: CourseId, instant: TimestampMillis) -> bool { self.retirements .iter() .any(|relation| relation.course == course && relation.valid_time.contains(instant)) }";

/// The whole replacement lookup. It reads one set and returns no verdict.
const WHOLE_REPLACEMENTS_FOR: &str = "pub fn replacements_for( &self, retired: CourseId, instant: TimestampMillis, ) -> BTreeSet<CourseId> { self.replacements .iter() .filter(|relation| relation.retired == retired && relation.valid_time.contains(instant)) .map(|relation| relation.replacement) .collect() }";

/// The whole publish entry point: every outcome of `append` goes through the
/// rewind or through the receipt, and there is no third arm.
const WHOLE_PUBLISH: &str = "pub fn publish( &self, ledger: &mut CurriculumLedger, publication: CurriculumPublication, ) -> Result<PublishReceipt, CurriculumError> { let mark = ledger.mark(); match self.append(ledger, publication) { Ok(receipt) => Ok(receipt), Err(failure) => { ledger.rewind_to(mark); Err(failure) } } }";

/// The whole rewind. Every vector a publication appends to is truncated here.
const WHOLE_REWIND: &str = "fn rewind_to(&mut self, mark: LedgerMark) { self.versions.truncate(mark.versions); self.courses.truncate(mark.courses); self.revisions.truncate(mark.revisions); self.offerings.truncate(mark.offerings); self.sources.truncate(mark.sources); self.relations.truncate_to( mark.identities, mark.equivalences, mark.replacements, mark.retirements, ); }";

/// The whole relation truncation, which is what a rewind reaches the four
/// relation vectors through.
const WHOLE_TRUNCATE_TO: &str = "pub(crate) fn truncate_to( &mut self, identities: usize, equivalences: usize, replacements: usize, retirements: usize, ) { self.identities.truncate(identities); self.equivalences.truncate(equivalences); self.replacements.truncate(replacements); self.retirements.truncate(retirements); }";

/// Section 11.4's sentence, whole. The four independent rules are read out of
/// this and nothing else.
const SECTION_11_4_SENTENCE: &str =
    "- 동일·대체·폐지·경과조치는 독립 rule이며 양방향 동일성으로 단순화하지 않는다.";

/// Section 8.2's closing sentence, whole. The three aggregates and the separate
/// effective-dated edge are read out of this.
const SECTION_8_2_SENTENCE: &str = "Course는 시간에 걸친 교과목 정체성이고, CourseRevision은 명칭·학점·분류가 유효한 버전이며, CourseOffering은 실제 학기의 분반이다. 같은 code라도 revision이 바뀔 수 있고, 동일·대체 관계는 별도 effective-dated edge다.";

/// Section 9's three boundary rows, whole. Each says what one aggregate does
/// **not** contain, and each is quoted in the module that owns that aggregate.
const SECTION_9_ROWS: [(&str, &str); 3] = [
    (
        "course.rs",
        "| `Course` | 대학이 지속적으로 식별하는 과목 | 특정 교수·학기·시간표·실제 설명 |",
    ),
    (
        "revision.rs",
        "| `CourseRevision` | 일정 기간 유효한 제목·학점·공식 분류·설계된 coverage | 특정 분반의 현실 |",
    ),
    (
        "offering.rs",
        "| `CourseOffering` | 학기·분반·교수자·시간·정원·syllabus의 실제 개설 | 매 수업시간의 실제 발화 |",
    ),
];

/// The `sourceSnapshot` mapping, and the one specification key that maps onto
/// no accessor.
///
/// Every other key of every section 8.2 block maps onto exactly one Rust
/// accessor, listed below. `effectiveFrom` and `effectiveTo` are the two ends
/// of one half-open interval, which ADR-003 requires to travel as one value, so
/// both map onto `valid_time`.
const SPECIFICATION_KEYS: [(&str, &str, &str); 36] = [
    // CurriculumVersion
    ("CurriculumVersion", "id", "id"),
    ("CurriculumVersion", "institutionPath", "institution_path"),
    (
        "CurriculumVersion",
        "admissionYearRange",
        "admission_year_range",
    ),
    ("CurriculumVersion", "effectiveFrom", "valid_time"),
    ("CurriculumVersion", "effectiveTo", "valid_time"),
    ("CurriculumVersion", "status", "status"),
    ("CurriculumVersion", "sourceSnapshot", "source_snapshot"),
    ("CurriculumVersion", "supersedes", "supersedes"),
    // Course
    ("Course", "id", "id"),
    ("Course", "courseCode", "code"),
    ("Course", "canonicalIdentity", "canonical_identity"),
    // CourseRevision
    ("CourseRevision", "course", "course"),
    ("CourseRevision", "titleKo", "title"),
    ("CourseRevision", "credits", "credits"),
    (
        "CourseRevision",
        "curriculumCategory",
        "curriculum_category",
    ),
    (
        "CourseRevision",
        "officialPrerequisiteRules",
        "official_prerequisites",
    ),
    (
        "CourseRevision",
        "recommendedPrerequisiteClaims",
        "recommended_prerequisites",
    ),
    (
        "CourseRevision",
        "designedConceptCoverage",
        "designed_concept_coverage",
    ),
    (
        "CourseRevision",
        "designedCompetencyCoverage",
        "designed_competency_coverage",
    ),
    ("CourseRevision", "validFrom", "valid_time"),
    ("CourseRevision", "sourceSnapshot", "source_snapshot"),
    // CourseOffering
    ("CourseOffering", "id", "id"),
    ("CourseOffering", "courseRevision", "course_revision"),
    ("CourseOffering", "term", "term"),
    ("CourseOffering", "section", "section"),
    ("CourseOffering", "instructors", "instructors"),
    ("CourseOffering", "meetings", "meetings"),
    ("CourseOffering", "capacity", "capacity"),
    ("CourseOffering", "gradingMode", "grading_mode"),
    ("CourseOffering", "syllabusArtifact", "syllabus_artifact"),
    ("CourseOffering", "materialRefs", "material_refs"),
    ("CourseOffering", "lectureRefs", "lecture_refs"),
    ("CourseOffering", "assessmentRefs", "assessment_refs"),
    ("CourseOffering", "reviewRefs", "review_refs"),
    ("CourseOffering", "officialStatus", "official_status"),
    ("CourseOffering", "observedAt", "observed_at"),
];

/// Which module owns which section 8.2 block.
const BLOCK_MODULES: [(&str, &str); 4] = [
    ("CurriculumVersion", "version.rs"),
    ("Course", "course.rs"),
    ("CourseRevision", "revision.rs"),
    ("CourseOffering", "offering.rs"),
];

/// Accessor names that legitimately appear on more than one aggregate, with the
/// reason each does.
///
/// A name on this list is exempt from the forbidden-field sweep and from
/// nothing else. The list is short on purpose: every entry is a name section
/// 8.2 itself puts on two blocks, or an identity every aggregate has.
const SHARED_ACCESSORS: [(&str, &str); 5] = [
    ("id", "every aggregate has one"),
    (
        "code",
        "section 8.2 puts `courseCode` on Course and prints it on a revision too",
    ),
    (
        "valid_time",
        "ADR-003 requires the half-open interval to travel as one value",
    ),
    (
        "credits",
        "`Credits::value` is the newtype's own accessor and is not the revision's",
    ),
    (
        "source_snapshot",
        "section 8.2 writes `sourceSnapshot` on CurriculumVersion and on CourseRevision",
    ),
];

/// Section 12.4's `TranscriptSegment` keys, as Rust would spell them.
///
/// This is the list `offering_boundary_rejects_session_transcript` is about:
/// section 9 says a `CourseOffering` does not contain 매 수업시간의 실제 발화,
/// and section 12.4 is where the specification writes down what one of those
/// is. `id` and `versions` are dropped because they are generic; the rest are
/// the session content itself.
const TRANSCRIPT_KEYS: [(&str, &str); 8] = [
    ("lectureId", "lecture_id"),
    ("startMs", "start_ms"),
    ("endMs", "end_ms"),
    ("speaker", "speaker"),
    ("verbatimText", "verbatim_text"),
    ("tokens", "tokens"),
    ("sourceAudioChunks", "source_audio_chunks"),
    ("correctionStatus", "correction_status"),
];

/// The authoritative specification's text.
fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// Migration 0014's text.
fn migration() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "migrations/store/0014_phase2_curriculum_aggregates.sql",
    ))?)
}

/// One module's source, as code.
fn module(name: &str) -> Result<String, Box<dyn Error>> {
    code_of(&crate_root().join("src").join(name))
}

/// The two-space-indented keys of one `<Block>:` yaml stanza in `text`.
///
/// The stanza ends at the first line that is neither blank nor indented, so a
/// nested list under a key is skipped rather than read as a key.
fn yaml_keys(text: &str, block: &str) -> Vec<String> {
    let Some(start) = text.find(&format!("\n{block}:\n")) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for line in text[start + 1..].lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("  ") else {
            break;
        };
        if rest.starts_with(' ') || rest.starts_with('-') {
            continue;
        }
        let Some((key, _)) = rest.split_once(':') else {
            continue;
        };
        found.push(key.trim().to_owned());
    }
    found
}

/// The text of one `impl <Type> {` block, up to its closing brace at column
/// zero.
///
/// The existence half of the forbidden-field sweep reads this rather than the
/// whole module. `U-I5` in the injection matrix is why: renaming
/// `CourseRevision::source_snapshot` left `CourseRevisionDraft::source_snapshot`
/// spelling the same name one type over, and a per-file name set could not tell
/// the two apart — so the accessor the specification requires could be removed
/// while the check still passed. The forbidden half stays per-module, because a
/// setter on a draft is as much a boundary crossing as an accessor on the
/// aggregate.
fn impl_block(code: &str, header: &str) -> Result<String, Box<dyn Error>> {
    let start = code
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let end = code[start..]
        .find(
            "
}",
        )
        .ok_or_else(|| format!("{header} has no closing brace at column zero"))?;
    Ok(code[start..start + end + 2].to_owned())
}

/// Every `pub fn`/`pub const fn` name declared in one module's code.
fn public_names(code: &str) -> BTreeSet<String> {
    public_signatures(code)
        .iter()
        .filter_map(|signature| {
            let after = signature.split("fn ").nth(1)?;
            let name: String = after
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// The walk reads the whole package, and nothing declares a module it misses.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 10,
        "the walk found only {} files under the package",
        sources.len()
    );

    // Product source lives under `src` and nowhere else. That is the condition
    // `S-12` says a crate has to keep if it does not want to widen every scan
    // that reads it.
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
    // is what says it descended the package rather than `src` by name: a walk
    // narrowed to `src` still clears the floor above, because this crate has
    // eleven modules there. `U-I20` in the injection matrix is that narrowing.
    assert!(
        sources
            .iter()
            .any(|path| path.ends_with("curriculum_scans.rs")),
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

    // The tripwire. Every `mod name;` and every `#[path = "…"]` in the crate has
    // to name a file the walk read. It fails the day the walk is narrowed, and
    // the day a module is added somewhere the walk does not descend into.
    let mut declared = 0_usize;
    for path in &sources {
        let source = fs::read_to_string(path)?;
        // A `#[path = "…"]` names the file the next `mod` declaration resolves
        // to, so it is carried forward rather than checked on its own line:
        // `crates/curriculum/tests/curriculum.rs` reaches `P2-U6`'s fixture
        // module that way, and a tripwire that did not follow it would fire on
        // a module that is read, just not from this package.
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
                        "{} includes {}, which does not exist",
                        relative(path),
                        target.display()
                    );
                    // A target inside this package has to be a file the walk
                    // read; one outside it -- `P2-U6`'s fixture module -- only
                    // has to exist, because this crate's walk does not descend
                    // into another package. The comparison is on canonical
                    // paths, because `tests/../../ingestion/...` starts with
                    // the package root as text and does not as a location.
                    let resolved = fs::canonicalize(&target)?;
                    if resolved.starts_with(fs::canonicalize(&root)?) {
                        let read_targets: Vec<PathBuf> = sources
                            .iter()
                            .filter_map(|source| fs::canonicalize(source).ok())
                            .collect();
                        assert!(
                            read_targets.contains(&resolved),
                            "{} includes {}, which the walk never read",
                            relative(path),
                            target.display()
                        );
                    }
                } else {
                    assert!(
                        read.contains(name),
                        "`{name}` is declared in {} and the walk never read it",
                        relative(path)
                    );
                }
            }
        }
    }
    assert!(declared >= 10, "the crate declares only {declared} modules");
    Ok(())
}

// ---------------------------------------------------------------------------
// The three section 9 boundaries
// ---------------------------------------------------------------------------

/// `course_boundary_rejects_offering_fields`,
/// `revision_boundary_rejects_section_fields` and
/// `offering_boundary_rejects_session_transcript`, as the source half.
///
/// The compile-fail cases are the type half: they observe that one named field
/// is absent. This is the half that says the list of named fields is the
/// specification's own — every key of every section 8.2 block is mapped to
/// exactly one Rust accessor, every mapped accessor exists on its own
/// aggregate, and every one of them is absent from every other aggregate that
/// section 9 says must not have it.
///
/// No count is asserted. Dropping a key from [`SPECIFICATION_KEYS`] fails
/// because the specification still names it; adding a Rust accessor that
/// belongs to another aggregate fails because the forbidden sweep finds it.
#[test]
fn the_forbidden_fields_are_the_specifications_own() -> TestResult {
    let specification = specification()?;

    // 1. Every key the specification writes is mapped, and every mapping names
    //    a key the specification writes. Compared as whole sets, both ways.
    for (block, _) in BLOCK_MODULES {
        let written: Vec<String> = yaml_keys(&specification, block);
        assert!(
            !written.is_empty(),
            "section 8.2 has no {block} block, or this walk stopped reading it"
        );
        let mapped: Vec<String> = SPECIFICATION_KEYS
            .iter()
            .filter(|(owner, _, _)| *owner == block)
            .map(|(_, key, _)| (*key).to_owned())
            .collect();
        assert_eq!(
            mapped, written,
            "{block}'s mapped key list is not section 8.2's own, in order"
        );
    }

    // 2. Every mapped accessor is declared by its own aggregate's own `impl`
    //    block -- not merely somewhere in the module. `U-I5` is the injection
    //    that made this distinction load-bearing.
    let mut names: Vec<(&str, BTreeSet<String>)> = Vec::new();
    for (block, file) in BLOCK_MODULES {
        let code = module(file)?;
        let own = public_names(&impl_block(&code, &format!("impl {block} {{"))?);
        assert!(
            own.len() >= 3,
            "impl {block} declares only {} public functions",
            own.len()
        );
        for (owner, key, accessor) in SPECIFICATION_KEYS {
            if owner != block {
                continue;
            }
            assert!(
                own.contains(accessor),
                "{block}.{key} maps to `{accessor}`, which impl {block} does not declare"
            );
        }
        // The forbidden half reads the whole module, because a setter on the
        // draft carries a field just as an accessor on the aggregate does.
        let declared = public_names(&code);
        assert!(
            declared.len() >= 5,
            "{file} declares only {} public functions",
            declared.len()
        );
        names.push((block, declared));
    }

    // 3. The forbidden sweep. For every ordered pair of aggregates, no accessor
    //    that belongs to one appears on the other, less the shared names each
    //    of which carries its own written reason.
    let shared: BTreeSet<&str> = SHARED_ACCESSORS.iter().map(|(name, _)| *name).collect();
    let mut swept = 0_usize;
    for (owner, _, accessor) in SPECIFICATION_KEYS {
        if shared.contains(accessor) {
            continue;
        }
        for (block, declared) in &names {
            if *block == owner {
                continue;
            }
            swept += 1;
            assert!(
                !declared.contains(accessor),
                "{block} declares `{accessor}`, which section 8.2 puts on {owner}"
            );
        }
    }
    assert!(
        swept >= 60,
        "the forbidden sweep compared only {swept} pairs"
    );

    // 4. `offering_boundary_rejects_session_transcript`'s own list, which is
    //    section 12.4's `TranscriptSegment` rather than a vocabulary invented
    //    here. Every key it writes is mapped, and every mapped name is absent
    //    from every module in this crate.
    let transcript_written: Vec<String> = yaml_keys(&specification, "TranscriptSegment");
    let transcript_mapped: Vec<String> = std::iter::once("id".to_owned())
        .chain(TRANSCRIPT_KEYS.iter().map(|(key, _)| (*key).to_owned()))
        .chain(std::iter::once("versions".to_owned()))
        .collect();
    assert_eq!(
        transcript_mapped, transcript_written,
        "section 12.4's TranscriptSegment key list is not the one this test maps"
    );
    for path in crate_product_sources()? {
        let declared = public_names(&code_of(&path)?);
        for (key, rust_name) in TRANSCRIPT_KEYS {
            assert!(
                !declared.contains(rust_name),
                "{} declares `{rust_name}`, which is section 12.4's TranscriptSegment.{key}",
                relative(&path)
            );
        }
    }

    // 5. Each section 9 row is quoted whole in the module that owns it, so the
    //    module cannot state a narrower boundary than the specification does.
    for (file, row) in SECTION_9_ROWS {
        assert!(
            specification.contains(row),
            "section 9's row for {file} is not in the specification as this pins it"
        );
        let text = fs::read_to_string(crate_root().join("src").join(file))?;
        let exclusion = row
            .rsplit('|')
            .nth(1)
            .ok_or("a section 9 row has no exclusion cell")?
            .trim();
        assert!(
            text.contains(exclusion),
            "{file} does not quote section 9's exclusion `{exclusion}`"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The four independent relations
// ---------------------------------------------------------------------------

/// The four relations are what section 11.4 names, and no code derives one from
/// another.
#[test]
fn no_relation_derives_another() -> TestResult {
    let specification = specification()?;
    assert!(
        specification.contains(SECTION_11_4_SENTENCE),
        "section 11.4's sentence is not in the specification as this pins it"
    );
    assert!(
        specification.contains(SECTION_8_2_SENTENCE),
        "section 8.2's closing sentence is not in the specification as this pins it"
    );

    // The four specification words, walked forwards through the sentence, so a
    // relation dropped from `CourseRelationKind` fails against section 11.4
    // rather than against a number written here.
    let mut cursor = 0_usize;
    for word in ["동일", "대체", "폐지", "경과조치"] {
        let at = SECTION_11_4_SENTENCE[cursor..]
            .find(word)
            .ok_or_else(|| format!("section 11.4 does not name {word} after {cursor}"))?;
        cursor += at + word.len();
    }
    let course_level: BTreeSet<&str> = CourseRelationKind::ALL
        .iter()
        .map(|kind| kind.specification_word())
        .collect();
    assert_eq!(
        course_level,
        BTreeSet::from(["동일", "대체", "폐지"]),
        "the course-level relations are not section 11.4's first three words"
    );
    // 경과조치 is the fourth, and it is not a course relation: it names a
    // cohort and a curriculum version, so there is nowhere on a course relation
    // to put it. It lives in `version.rs`.
    assert!(
        !course_level.contains("경과조치"),
        "a course relation claims to be the transitional measure"
    );
    let version_code = module("version.rs")?;
    assert!(
        public_names(&version_code).contains("transition_for"),
        "version.rs does not answer the transitional measure"
    );
    assert!(
        !public_names(&module("relation.rs")?).contains("transition_for"),
        "relation.rs answers the transitional measure, which is not a course relation"
    );

    // The whole `impl` set of the four relation types, over **every product
    // file in this crate** rather than over `relation.rs`. Both the type and
    // the trait would be local to this crate, so the orphan rule refuses a
    // conversion written outside it and refuses nothing written in a sibling
    // module: a `From` in `publish.rs` compiles exactly as one in `relation.rs`
    // does. A `From`, a `TryFrom`, a `Deref` or an `AsRef` between any two of
    // them appears here as an extra key rather than being searched for by
    // spelling.
    let relation_code = module("relation.rs")?;
    let relation_types = [
        "IdentityDecision",
        "EquivalenceRelation",
        "ReplacementRelation",
        "RetirementRelation",
    ];
    let mut headers: Vec<String> = Vec::new();
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        scanned += 1;
        for type_name in relation_types {
            headers.extend(impl_headers_naming(&code, type_name));
        }
    }
    assert!(
        scanned >= 10,
        "the impl sweep read only {scanned} product files"
    );
    headers.sort();
    headers.dedup();
    assert_eq!(
        headers,
        vec![
            "impl EquivalenceRelation {".to_owned(),
            "impl IdentityDecision {".to_owned(),
            "impl ReplacementRelation {".to_owned(),
            "impl RetirementRelation {".to_owned(),
        ],
        "the set of impl blocks naming a relation changed; a conversion is how one derives another"
    );

    // The whole signature set of every product file in this crate: no signature
    // takes one relation type and returns another. Widened past `relation.rs`
    // for the reason the `impl` sweep above gives -- a conversion in a sibling
    // module compiles just as well.
    let mut signature_count = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            signature_count += 1;
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            for taken in relation_types {
                if uses_of(parameters, taken) == 0 {
                    continue;
                }
                for produced in relation_types {
                    if produced == taken {
                        continue;
                    }
                    assert_eq!(
                        uses_of(returns, produced),
                        0,
                        "{}: `{signature}` takes a {taken} and returns a {produced}",
                        relative(&path)
                    );
                }
            }
        }
    }
    assert!(
        signature_count >= 100,
        "the signature sweep read only {signature_count} public functions"
    );

    // Each of the four lookups is pinned whole, so a lookup that started
    // reading a second set fails as changed text rather than as a behaviour
    // nobody wrote a case for.
    for (signature, pinned, what) in [
        ("pub fn same_course(", WHOLE_SAME_COURSE, "identity"),
        ("pub fn equivalent(", WHOLE_EQUIVALENT, "equivalence"),
        (
            "pub fn replacements_for(",
            WHOLE_REPLACEMENTS_FOR,
            "replacement",
        ),
        ("pub fn retired(", WHOLE_RETIRED, "retirement"),
    ] {
        assert_eq!(
            declared_member(
                &relation_code,
                signature,
                "
    }"
            )?,
            pinned,
            "the {what} lookup changed"
        );
    }

    // And each reads exactly one of the four vectors. Counted over the pinned
    // text, so a second read is a changed pin as well as a changed count.
    for (pinned, own) in [
        (WHOLE_SAME_COURSE, "identities"),
        (WHOLE_EQUIVALENT, "equivalences"),
        (WHOLE_REPLACEMENTS_FOR, "replacements"),
        (WHOLE_RETIRED, "retirements"),
    ] {
        for field in ["identities", "equivalences", "replacements", "retirements"] {
            let expected = usize::from(field == own);
            assert_eq!(
                uses_of(pinned, field),
                expected,
                "a relation lookup reads `{field}` {} time(s)",
                uses_of(pinned, field)
            );
        }
    }
    Ok(())
}

/// Nothing infers a course identity, and `UNKNOWN` is the only thing an absent
/// decision produces.
#[test]
fn nothing_infers_a_course_identity() -> TestResult {
    // The whole set of signatures anywhere in this crate that produce a
    // `CourseCodeReuse`. Three today: the lookup, the ledger's delegate, and
    // the enum's own spelling. A fourth producer -- a heuristic on the code
    // string, a rule reading a replacement -- fails here as an extra key.
    let mut producers: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            let Some((_, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if uses_of(returns, "CourseCodeReuse") > 0 {
                producers.push(format!("{}: {signature}", relative(&path)));
            }
        }
    }
    producers.sort();
    assert_eq!(
        producers,
        vec![
            "crates/curriculum/src/publish.rs: pub fn same_course( &self, earlier: CourseId, later: CourseId, instant: TimestampMillis, ) -> crate::relation::CourseCodeReuse {".to_owned(),
            "crates/curriculum/src/relation.rs: pub const fn verdict(self) -> CourseCodeReuse {".to_owned(),
            "crates/curriculum/src/relation.rs: pub fn same_course( &self, earlier: CourseId, later: CourseId, instant: TimestampMillis, ) -> CourseCodeReuse {".to_owned(),
        ],
        "the set of functions that produce a course identity verdict changed"
    );

    // `CourseCodeReuse::Unknown` is produced in exactly one place -- the
    // `map_or` fallback inside `same_course` -- and refused in exactly one --
    // `IdentityDecision::record`. A second producer would be a second way to
    // say "no decision"; a lost refusal would let a decision record one.
    let relation_code = module("relation.rs")?;
    assert_eq!(
        occurrences(&relation_code, "CourseCodeReuse::Unknown"),
        2,
        "the absent identity reading is named somewhere other than the lookup and the refusal"
    );
    assert!(
        WHOLE_SAME_COURSE.contains("map_or(CourseCodeReuse::Unknown"),
        "the identity lookup no longer falls back to UNKNOWN"
    );
    assert!(
        declared_member(
            &relation_code,
            "    pub fn record(",
            "
    }"
        )?
        .contains("matches!(verdict, CourseCodeReuse::Unknown)"),
        "IdentityDecision::record no longer refuses UNKNOWN as a recorded verdict"
    );

    // The course code is a label, and nothing compares two of them. The whole
    // set of signatures in this crate that take two `CourseCode` values is
    // empty.
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            let Some((parameters, _)) = parameters_and_return(&signature) else {
                continue;
            };
            assert!(
                uses_of(parameters, "CourseCode") < 2,
                "{}: `{signature}` compares two course codes",
                relative(&path)
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The publication
// ---------------------------------------------------------------------------

/// One rewind, taken by every failure, reached from one place.
#[test]
fn the_publish_path_has_one_rewind_and_every_failure_takes_it() -> TestResult {
    let publish_code = module("publish.rs")?;
    let relation_code = module("relation.rs")?;

    assert_eq!(
        declared_member(
            &publish_code,
            "    pub fn publish(",
            "
    }"
        )?,
        WHOLE_PUBLISH,
        "the publish entry point changed"
    );
    assert_eq!(
        declared_member(
            &publish_code,
            "    fn rewind_to(",
            "
    }"
        )?,
        WHOLE_REWIND,
        "the rewind changed"
    );
    assert_eq!(
        declared_member(
            &relation_code,
            "    pub(crate) fn truncate_to(",
            "
    }"
        )?,
        WHOLE_TRUNCATE_TO,
        "the relation truncation changed"
    );

    // The pin fixes the body; these fix its callers. `append` is called once,
    // from `publish`, so there is no second entry point that writes without
    // taking the mark; `rewind_to` is called once, from the failure arm.
    let code = without_use_items(&publish_code);
    assert_eq!(
        calls_of(&code, "append"),
        1,
        "the appending body is called from more than one place"
    );
    assert_eq!(
        calls_of(&code, "rewind_to"),
        1,
        "the rewind is called from more than one place"
    );
    // The mark is produced in one place. Counted as the call rather than as the
    // identifier, because `rewind_to` names its parameter `mark` and reads its
    // fields, and those are the value being spent rather than made.
    assert_eq!(
        occurrences(&code, "fn mark("),
        1,
        "the ledger mark has more than one producer"
    );
    assert_eq!(
        occurrences(&code, "ledger.mark()"),
        1,
        "the ledger mark is taken somewhere other than publish"
    );
    assert_eq!(
        calls_of(&without_use_items(&relation_code), "truncate_to"),
        0,
        "the relation truncation is called from inside its own crate module"
    );

    // Every vector `append` pushes to is truncated by the rewind. Enumerated
    // from the appending body rather than written down, so a fifth vector added
    // to the ledger fails here until the rewind reaches it.
    let append_body = declared_member(
        &publish_code,
        "    fn append(",
        "
    }",
    )?;
    let mut pushed: BTreeSet<String> = BTreeSet::new();
    for fragment in append_body.split("ledger.").skip(1) {
        let name: String = fragment
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if append_body.contains(&format!("ledger.{name}.push(")) {
            pushed.insert(name);
        }
    }
    assert!(
        pushed.len() >= 4,
        "the appending body pushes to only {} ledger vectors",
        pushed.len()
    );
    for name in &pushed {
        assert!(
            WHOLE_REWIND.contains(&format!("self.{name}.truncate(")),
            "`{name}` is appended to and the rewind does not truncate it"
        );
    }
    // The four relation vectors are reached through `truncate_to` instead.
    for name in ["identities", "equivalences", "replacements", "retirements"] {
        assert!(
            WHOLE_TRUNCATE_TO.contains(&format!("self.{name}.truncate(")),
            "`{name}` is not truncated by the relation rewind"
        );
    }

    // Every checkpoint the fault type declares is consulted in the appending
    // body, and each is consulted through the injector rather than by a
    // condition written inline.
    for point in PublishCheckpoint::ALL {
        let variant = format!("{point:?}");
        assert!(
            append_body.contains(&format!("PublishCheckpoint::{variant}")),
            "the appending body never reaches PublishCheckpoint::{variant}"
        );
    }
    assert_eq!(
        occurrences(&append_body, "self.faults.hit("),
        occurrences(&append_body, "PublishCheckpoint::"),
        "a checkpoint is named somewhere other than an injector call"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The migration and the open gates
// ---------------------------------------------------------------------------

/// Migration 0014's vocabularies are this crate's enums.
#[test]
fn the_migration_vocabularies_are_the_rust_ones() -> TestResult {
    let sql = migration()?;

    // The vocabularies that carry `UNKNOWN` on both sides, because the absence
    // of a record is a column value there.
    for (column, rust) in [
        (
            "curriculum_category",
            CurriculumCategory::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect::<Vec<_>>(),
        ),
        (
            "grading_mode",
            GradingMode::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
        ),
        (
            "official_status",
            OfferingStatus::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
        ),
        (
            "publication_status",
            PublicationStatus::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
        ),
    ] {
        let mut wanted = rust;
        wanted.sort();
        assert_eq!(
            sql_check_list(&sql, column),
            wanted,
            "migration 0014's {column} vocabulary is not this crate's"
        );
    }

    // The two where the schema deliberately omits `UNKNOWN`, because there the
    // absence of a record is the absence of a row.
    for (column, rust) in [
        (
            "verdict",
            CourseCodeReuse::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect::<Vec<_>>(),
        ),
        (
            "disposition",
            CohortTransition::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
        ),
    ] {
        let mut wanted: Vec<String> = rust
            .into_iter()
            .filter(|value| value != "UNKNOWN")
            .collect();
        wanted.sort();
        assert_eq!(
            sql_check_list(&sql, column),
            wanted,
            "migration 0014's {column} vocabulary is not this crate's, less UNKNOWN"
        );
    }

    // And every variant renamed on one side fails on the other, which is what
    // reading the variant names out of the source rather than the type gives.
    let revision_code = fs::read_to_string(crate_root().join("src/revision.rs"))?;
    let variants = enum_variants(&revision_code, "pub enum CurriculumCategory {");
    assert_eq!(
        variants.len(),
        CurriculumCategory::ALL.len(),
        "CurriculumCategory's declared variants are not the ones ALL lists"
    );
    Ok(())
}

/// The three open gates are stated where they bite, and nothing defaults past
/// them.
#[test]
fn the_open_gates_have_no_default() -> TestResult {
    let identifiers: Vec<&str> = OpenGate::ALL.iter().map(|gate| gate.identifier()).collect();
    assert_eq!(identifiers, ["GATE-38-013", "GATE-38-014", "GATE-38-018"]);

    // No `Default` anywhere in this crate produces a value that stands in for
    // an official fact. The two `Default` implementations are the empty ledger
    // and the empty relation set; both are emptiness rather than a value.
    let mut defaults: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl Default for") || trimmed.contains("derive(Debug, Default")
            {
                defaults.push(format!("{}: {trimmed}", relative(&path)));
            }
            if trimmed.contains("Default")
                && trimmed.starts_with("#[derive")
                && !trimmed.contains("Debug, Default")
            {
                defaults.push(format!("{}: {trimmed}", relative(&path)));
            }
        }
    }
    defaults.sort();
    assert_eq!(
        defaults,
        vec![
            "crates/curriculum/src/fault.rs: #[derive(Debug, Default, Clone, Copy)]".to_owned(),
            "crates/curriculum/src/publish.rs: #[derive(Debug, Clone, Default, PartialEq, Eq)]"
                .to_owned(),
            "crates/curriculum/src/publish.rs: impl Default for CurriculumPublisher<'static> {"
                .to_owned(),
            "crates/curriculum/src/relation.rs: #[derive(Debug, Clone, Default, PartialEq, Eq)]"
                .to_owned(),
        ],
        "a Default appeared in this crate; an official fact with a default is a fact invented"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// One step out
// ---------------------------------------------------------------------------

/// The same shape, one layer outside this crate.
///
/// Every rule above is about `crates/curriculum`. A module elsewhere that named
/// these types could derive one relation from another, or answer a course
/// identity from something other than a decision, and no scan in this file
/// would see it. This is the inventory that does: the whole set of files
/// outside this package that name any of the four relation types, the identity
/// verdict, or the publisher. It is empty today, and a file added to it is a
/// review rather than a silent second implementation.
///
/// `academic_ingestion`'s `no_captcha_or_access_control_bypass_module_exists`
/// closes its own boundary with the same walk, for the same reason.
#[test]
fn no_file_outside_this_crate_names_a_curriculum_relation() -> TestResult {
    let package = crate_root();
    let mut named: Vec<String> = Vec::new();
    let mut scanned = 0_usize;
    for path in workspace_product_sources()? {
        if path.starts_with(&package) {
            continue;
        }
        scanned += 1;
        let code = code_of(&path)?;
        for type_name in [
            "IdentityDecision",
            "EquivalenceRelation",
            "ReplacementRelation",
            "RetirementRelation",
            "CourseCodeReuse",
            "CurriculumPublisher",
            "CurriculumLedger",
            "TransitionArrangement",
        ] {
            if uses_of(&code, type_name) > 0 {
                named.push(format!("{}: {type_name}", relative(&path)));
            }
        }
    }
    named.sort();
    // The floor. A walk that returned nothing would satisfy the comparison
    // below over an empty set.
    assert!(
        scanned >= 150,
        "the workspace walk read only {scanned} files outside this package"
    );
    assert_eq!(
        named,
        Vec::<String>::new(),
        "a file outside this package names a curriculum relation type"
    );

    // And the walk does reach this package's own files, so `starts_with` above
    // is excluding something rather than everything.
    let inside = workspace_product_sources()?
        .into_iter()
        .filter(|path| path.starts_with(&package))
        .count();
    assert!(
        inside >= 10,
        "the workspace walk read only {inside} files inside this package"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// `S-20`
// ---------------------------------------------------------------------------

/// Section 38's cells, in the order the document writes them.
///
/// Section 38.1's ten lines, then section 38.2's eleven bullets, then section
/// 38.3's ten numbered questions. `GATE-38-{:03}` is one-based over that
/// concatenation, so a cell's identifier is a fact about where the document
/// puts it and not a string anybody chose.
fn section_38_cells() -> Result<Vec<String>, Box<dyn Error>> {
    let specification = specification()?;
    let block = specification
        .split_once("Admission Year")
        .map(|(_, rest)| format!("Admission Year{rest}"))
        .and_then(|rest| rest.split_once("```").map(|(block, _)| block.to_owned()))
        .ok_or("section 38.1's block is not in the document")?;
    let mut cells: Vec<String> = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(cells.len(), 10, "section 38.1 lists {} lines", cells.len());

    let bullets = specification
        .split_once("### 38.2 공식적으로 추가 확인할 항목")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("### 38.3").map(|(block, _)| block))
        .ok_or("section 38.2's list is not in the document")?;
    let bullets: Vec<String> = bullets
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- ").map(str::to_owned))
        .collect();
    assert_eq!(
        bullets.len(),
        11,
        "section 38.2 lists {} bullets",
        bullets.len()
    );

    let questions = specification
        .split_once("### 38.3 아직 결정할 제품·아키텍처 질문")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("\n---").map(|(block, _)| block))
        .ok_or("section 38.3's list is not in the document")?;
    let questions: Vec<String> = questions
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.split_once(". ")
                .and_then(|(number, text)| number.parse::<usize>().ok().map(|_| text.to_owned()))
        })
        .collect();
    assert_eq!(
        questions.len(),
        10,
        "section 38.3 lists {} questions",
        questions.len()
    );

    cells.extend(bullets);
    cells.extend(questions);
    Ok(cells)
}

/// Where section 38 writes one cell, one-based.
///
/// The identifier is never compared against a list somebody typed: it is
/// rebuilt from the position and the two have to agree.
fn section_38_position(
    cells: &[String],
    identifier: &str,
    spec_line: &str,
) -> Result<usize, Box<dyn Error>> {
    Ok(cells
        .iter()
        .position(|cell| cell.starts_with(spec_line))
        .ok_or_else(|| {
            format!("{identifier} quotes a line section 38 does not write: {spec_line}")
        })?
        .saturating_add(1))
}

/// The `GATE-38-xxx` identifiers are section 38's own numbering, derived from
/// each cell's position in the document rather than compared against a list
/// written twice.
///
/// `S-20`: eleven of this workspace's eighteen `OpenGate::identifier` arms were
/// hand-written strings whose only check was a hand-written list in the same
/// test, so the first edit to section 38 that inserts, removes or reorders a
/// cell renumbered the ones after it silently. `P2-U3` closed it for `academic-audit`'s seven cells; these are this
/// crate's three.
#[test]
fn the_open_gates_are_section_38s_own() -> TestResult {
    let cells = section_38_cells()?;
    let mut derived: Vec<&'static str> = Vec::new();
    for (gate, spec_line) in [
        (
            OpenGate::RecognitionList,
            "공대 공통교과목 인정 list의 최신판과 전필/전선 배분.",
        ),
        (
            OpenGate::SubstitutionRules,
            "동일·대체·유사과목, 타과 전선 인정, 산업공학과 제외 등 사용자의 이수 시점별 적용.",
        ),
        (
            OpenGate::OfficialVersusRecommendedPrerequisite,
            "Course별 공식 prerequisite와 담당교수의 권장 선수지식 차이.",
        ),
    ] {
        let position = section_38_position(&cells, gate.identifier(), spec_line)?;
        assert_eq!(
            gate.identifier(),
            format!("GATE-38-{position:03}"),
            "{} is section 38's cell {position}, so its identifier does not follow its position",
            gate.identifier()
        );
        derived.push(gate.identifier());
    }

    // Both directions: a variant added to `ALL` without a section 38 cell above
    // is a missing key here, and a cell derived for a variant `ALL` dropped is
    // an extra one.
    let declared: Vec<&'static str> = OpenGate::ALL.iter().map(|gate| gate.identifier()).collect();
    assert_eq!(
        derived, declared,
        "the derived cells are not the ones this crate declares"
    );
    Ok(())
}

/// The floor the inventory walk must reach, so an empty walk fails as a walk.
const INVENTORY_FILE_FLOOR: usize = 11;

/// Every function this package declares, as `<file> [vis] <signature>`.
const DECLARATIONS: &[&str] = &[
    "src/course.rs [pub] fn build(self) -> Result<Course, CurriculumError>",
    "src/course.rs [pub] fn canonical_identity(&self) -> EntityId",
    "src/course.rs [pub] fn canonical_identity(mut self, entity: EntityId) -> Self",
    "src/course.rs [pub] fn code(&self) -> &CourseCode",
    "src/course.rs [pub] fn id(&self) -> CourseId",
    "src/course.rs [pub] fn new(id: CourseId, code: CourseCode) -> Self",
    "src/fault.rs [priv] fn hit(&self, _point: PublishCheckpoint) -> Result<(), CurriculumError>",
    "src/fault.rs [priv] fn hit(&self, point: PublishCheckpoint) -> Result<(), CurriculumError>",
    "src/fault.rs [pub] fn as_str(self) -> &'static str",
    "src/gate.rs [pub] fn identifier(self) -> &'static str",
    "src/gate.rs [pub] fn statement(self) -> &'static str",
    "src/gate.rs [pub] fn unknown_readings() -> [(&'static str, &'static str); 5]",
    "src/offering.rs [pub] fn as_str(self) -> &'static str",
    "src/offering.rs [pub] fn as_str(self) -> &'static str",
    "src/offering.rs [pub] fn assessment_ref(mut self, reference: EntityId) -> Self",
    "src/offering.rs [pub] fn assessment_refs(&self) -> &[EntityId]",
    "src/offering.rs [pub] fn build(self) -> CourseOffering",
    "src/offering.rs [pub] fn capacity(&self) -> Option<Capacity>",
    "src/offering.rs [pub] fn capacity(mut self, capacity: Capacity) -> Self",
    "src/offering.rs [pub] fn course_revision(&self) -> CourseRevisionId",
    "src/offering.rs [pub] fn from_minute(self) -> u16",
    "src/offering.rs [pub] fn grading_mode(&self) -> GradingMode",
    "src/offering.rs [pub] fn grading_mode(mut self, mode: GradingMode) -> Self",
    "src/offering.rs [pub] fn id(&self) -> OfferingId",
    "src/offering.rs [pub] fn instructor(mut self, name: InstructorName) -> Self",
    "src/offering.rs [pub] fn instructors(&self) -> &[InstructorName]",
    "src/offering.rs [pub] fn lecture_ref(mut self, reference: EntityId) -> Self",
    "src/offering.rs [pub] fn lecture_refs(&self) -> &[EntityId]",
    "src/offering.rs [pub] fn material_ref(mut self, reference: EntityId) -> Self",
    "src/offering.rs [pub] fn material_refs(&self) -> &[EntityId]",
    "src/offering.rs [pub] fn meeting(mut self, meeting: Meeting) -> Self",
    "src/offering.rs [pub] fn meetings(&self) -> &[Meeting]",
    "src/offering.rs [pub] fn new( id: OfferingId, course_revision: CourseRevisionId, term: TermCode, section: SectionCode, official_status: OfferingStatus, observed_at: TimestampMillis, ) -> Self",
    "src/offering.rs [pub] fn new( weekday: Weekday, from_minute: u16, to_minute: u16, ) -> Result<Self, CurriculumError>",
    "src/offering.rs [pub] fn new(seats: u16) -> Self",
    "src/offering.rs [pub] fn observed_at(&self) -> TimestampMillis",
    "src/offering.rs [pub] fn official_status(&self) -> OfferingStatus",
    "src/offering.rs [pub] fn review_ref(mut self, reference: EntityId) -> Self",
    "src/offering.rs [pub] fn review_refs(&self) -> &[EntityId]",
    "src/offering.rs [pub] fn seats(self) -> u16",
    "src/offering.rs [pub] fn section(&self) -> &SectionCode",
    "src/offering.rs [pub] fn syllabus_artifact(&self) -> Option<ArtifactId>",
    "src/offering.rs [pub] fn syllabus_artifact(mut self, artifact: ArtifactId) -> Self",
    "src/offering.rs [pub] fn term(&self) -> &TermCode",
    "src/offering.rs [pub] fn to_minute(self) -> u16",
    "src/offering.rs [pub] fn weekday(self) -> Weekday",
    "src/publish.rs [priv] fn append( &self, ledger: &mut CurriculumLedger, publication: CurriculumPublication, ) -> Result<PublishReceipt, CurriculumError>",
    "src/publish.rs [priv] fn default() -> Self",
    "src/publish.rs [priv] fn mark(&self) -> LedgerMark",
    "src/publish.rs [priv] fn rewind_to(&mut self, mark: LedgerMark)",
    "src/publish.rs [priv] fn validate( ledger: &CurriculumLedger, publication: &CurriculumPublication, ) -> Result<(), CurriculumError>",
    "src/publish.rs [pub] fn connector(&self) -> &ConnectorId",
    "src/publish.rs [pub] fn course(&self, id: CourseId) -> Option<&Course>",
    "src/publish.rs [pub] fn courses(&self) -> &[CourseId]",
    "src/publish.rs [pub] fn courses(&self) -> &[Course]",
    "src/publish.rs [pub] fn effective(&self) -> EffectiveDate",
    "src/publish.rs [pub] fn equivalent(&self, source: CourseId, target: CourseId, instant: TimestampMillis) -> bool",
    "src/publish.rs [pub] fn from_official_source(published: &PublishedRules, version: CurriculumVersion) -> Self",
    "src/publish.rs [pub] fn new() -> Self",
    "src/publish.rs [pub] fn new() -> Self",
    "src/publish.rs [pub] fn offering(&self, id: OfferingId) -> Option<&CourseOffering>",
    "src/publish.rs [pub] fn offerings(&self) -> &[CourseOffering]",
    "src/publish.rs [pub] fn offerings(&self) -> &[OfferingId]",
    "src/publish.rs [pub] fn offerings_for_course(&self, course: CourseId) -> Vec<&CourseOffering>",
    "src/publish.rs [pub] fn offerings_of(&self, revision: CourseRevisionId) -> Vec<&CourseOffering>",
    "src/publish.rs [pub] fn parser_version(&self) -> ParserVersion",
    "src/publish.rs [pub] fn publish( &self, ledger: &mut CurriculumLedger, publication: CurriculumPublication, ) -> Result<PublishReceipt, CurriculumError>",
    "src/publish.rs [pub] fn relations(&self) -> &CourseRelations",
    "src/publish.rs [pub] fn replacements_for( &self, retired: CourseId, instant: TimestampMillis, ) -> std::collections::BTreeSet<CourseId>",
    "src/publish.rs [pub] fn retired(&self, course: CourseId, instant: TimestampMillis) -> bool",
    "src/publish.rs [pub] fn retrieved_at(&self) -> RetrievalInstant",
    "src/publish.rs [pub] fn revision(&self, id: CourseRevisionId) -> Option<&CourseRevision>",
    "src/publish.rs [pub] fn revisions(&self) -> &[CourseRevisionId]",
    "src/publish.rs [pub] fn revisions(&self) -> &[CourseRevision]",
    "src/publish.rs [pub] fn revisions_of(&self, course: CourseId) -> Vec<&CourseRevision>",
    "src/publish.rs [pub] fn same_course( &self, earlier: CourseId, later: CourseId, instant: TimestampMillis, ) -> crate::relation::CourseCodeReuse",
    "src/publish.rs [pub] fn source(&self) -> &OfficialSourceBinding",
    "src/publish.rs [pub] fn sources(&self) -> &[OfficialSourceBinding]",
    "src/publish.rs [pub] fn version(&self) -> &CurriculumVersion",
    "src/publish.rs [pub] fn version(&self) -> CurriculumVersionId",
    "src/publish.rs [pub] fn version(&self, id: CurriculumVersionId) -> Option<&CurriculumVersion>",
    "src/publish.rs [pub] fn versions(&self) -> &[CurriculumVersion]",
    "src/publish.rs [pub] fn with_course(mut self, course: Course) -> Self",
    "src/publish.rs [pub] fn with_equivalence(mut self, relation: EquivalenceRelation) -> Self",
    "src/publish.rs [pub] fn with_faults(faults: &'injector dyn PublishFaultInjector) -> Self",
    "src/publish.rs [pub] fn with_identity(mut self, decision: IdentityDecision) -> Self",
    "src/publish.rs [pub] fn with_offering(mut self, offering: CourseOffering) -> Self",
    "src/publish.rs [pub] fn with_replacement(mut self, relation: ReplacementRelation) -> Self",
    "src/publish.rs [pub] fn with_retirement(mut self, relation: RetirementRelation) -> Self",
    "src/publish.rs [pub] fn with_revision(mut self, revision: CourseRevision) -> Self",
    "src/relation.rs [priv] fn truncate_to( &mut self, identities: usize, equivalences: usize, replacements: usize, retirements: usize, )",
    "src/relation.rs [pub] fn as_str(self) -> &'static str",
    "src/relation.rs [pub] fn as_str(self) -> &'static str",
    "src/relation.rs [pub] fn course(self) -> CourseId",
    "src/relation.rs [pub] fn decision(self) -> DecisionId",
    "src/relation.rs [pub] fn earlier(self) -> CourseId",
    "src/relation.rs [pub] fn equivalences(&self) -> &[EquivalenceRelation]",
    "src/relation.rs [pub] fn equivalent(&self, source: CourseId, target: CourseId, instant: TimestampMillis) -> bool",
    "src/relation.rs [pub] fn identities(&self) -> &[IdentityDecision]",
    "src/relation.rs [pub] fn later(self) -> CourseId",
    "src/relation.rs [pub] fn new() -> Self",
    "src/relation.rs [pub] fn record( earlier: CourseId, later: CourseId, verdict: CourseCodeReuse, decision: DecisionId, valid_time: ValidInterval, ) -> Result<Self, CurriculumError>",
    "src/relation.rs [pub] fn record( retired: CourseId, replacement: CourseId, valid_time: ValidInterval, ) -> Result<Self, CurriculumError>",
    "src/relation.rs [pub] fn record( source: CourseId, target: CourseId, valid_time: ValidInterval, ) -> Result<Self, CurriculumError>",
    "src/relation.rs [pub] fn record(course: CourseId, valid_time: ValidInterval) -> Self",
    "src/relation.rs [pub] fn record_equivalence(&mut self, relation: EquivalenceRelation)",
    "src/relation.rs [pub] fn record_identity(&mut self, decision: IdentityDecision)",
    "src/relation.rs [pub] fn record_replacement(&mut self, relation: ReplacementRelation)",
    "src/relation.rs [pub] fn record_retirement(&mut self, relation: RetirementRelation)",
    "src/relation.rs [pub] fn replacement(self) -> CourseId",
    "src/relation.rs [pub] fn replacements(&self) -> &[ReplacementRelation]",
    "src/relation.rs [pub] fn replacements_for( &self, retired: CourseId, instant: TimestampMillis, ) -> BTreeSet<CourseId>",
    "src/relation.rs [pub] fn retired(&self, course: CourseId, instant: TimestampMillis) -> bool",
    "src/relation.rs [pub] fn retired(self) -> CourseId",
    "src/relation.rs [pub] fn retirements(&self) -> &[RetirementRelation]",
    "src/relation.rs [pub] fn same_course( &self, earlier: CourseId, later: CourseId, instant: TimestampMillis, ) -> CourseCodeReuse",
    "src/relation.rs [pub] fn source(self) -> CourseId",
    "src/relation.rs [pub] fn specification_word(self) -> &'static str",
    "src/relation.rs [pub] fn target(self) -> CourseId",
    "src/relation.rs [pub] fn valid_time(self) -> ValidInterval",
    "src/relation.rs [pub] fn valid_time(self) -> ValidInterval",
    "src/relation.rs [pub] fn valid_time(self) -> ValidInterval",
    "src/relation.rs [pub] fn valid_time(self) -> ValidInterval",
    "src/relation.rs [pub] fn verdict(self) -> CourseCodeReuse",
    "src/revision.rs [pub] fn as_str(self) -> &'static str",
    "src/revision.rs [pub] fn build(self) -> Result<CourseRevision, CurriculumError>",
    "src/revision.rs [pub] fn code(&self) -> &CourseCode",
    "src/revision.rs [pub] fn course(&self) -> CourseId",
    "src/revision.rs [pub] fn course(self) -> CourseId",
    "src/revision.rs [pub] fn course(self) -> CourseId",
    "src/revision.rs [pub] fn credits(&self) -> Credits",
    "src/revision.rs [pub] fn credits(mut self, credits: Credits) -> Self",
    "src/revision.rs [pub] fn curriculum_category(&self) -> CurriculumCategory",
    "src/revision.rs [pub] fn curriculum_category(mut self, category: CurriculumCategory) -> Self",
    "src/revision.rs [pub] fn curriculum_version(&self) -> CurriculumVersionId",
    "src/revision.rs [pub] fn designed_competency(mut self, competency: EntityId) -> Self",
    "src/revision.rs [pub] fn designed_competency_coverage(&self) -> &[EntityId]",
    "src/revision.rs [pub] fn designed_concept(mut self, concept: EntityId) -> Self",
    "src/revision.rs [pub] fn designed_concept_coverage(&self) -> &[EntityId]",
    "src/revision.rs [pub] fn id(&self) -> CourseRevisionId",
    "src/revision.rs [pub] fn is_known(self) -> bool",
    "src/revision.rs [pub] fn new( id: CourseRevisionId, course: CourseId, curriculum_version: CurriculumVersionId, code: CourseCode, valid_time: ValidInterval, ) -> Self",
    "src/revision.rs [pub] fn new(value: u8) -> Result<Self, CurriculumError>",
    "src/revision.rs [pub] fn official_prerequisite(mut self, entry: OfficialPrerequisite) -> Self",
    "src/revision.rs [pub] fn official_prerequisites(&self) -> &[OfficialPrerequisite]",
    "src/revision.rs [pub] fn on(course: CourseId) -> Self",
    "src/revision.rs [pub] fn on(course: CourseId) -> Self",
    "src/revision.rs [pub] fn recommended_prerequisite(mut self, entry: RecommendedPrerequisite) -> Self",
    "src/revision.rs [pub] fn recommended_prerequisites(&self) -> &[RecommendedPrerequisite]",
    "src/revision.rs [pub] fn source_snapshot(&self) -> Option<&ContentDigest>",
    "src/revision.rs [pub] fn source_snapshot(mut self, digest: ContentDigest) -> Self",
    "src/revision.rs [pub] fn title(&self) -> &CourseTitle",
    "src/revision.rs [pub] fn title(mut self, title: CourseTitle) -> Self",
    "src/revision.rs [pub] fn valid_time(&self) -> ValidInterval",
    "src/revision.rs [pub] fn value(self) -> u8",
    "src/text.rs [priv] fn bounded(field: &'static str, value: &str) -> Result<(), CurriculumError>",
    "src/text.rs [pub] fn as_str(&self) -> &str",
    "src/text.rs [pub] fn parse(value: &str) -> Result<Self, CurriculumError>",
    "src/version.rs [pub] fn admission_year_range(&self) -> (&AdmissionCohort, &AdmissionCohort)",
    "src/version.rs [pub] fn as_str(self) -> &'static str",
    "src/version.rs [pub] fn as_str(self) -> &'static str",
    "src/version.rs [pub] fn build(self) -> Result<CurriculumVersion, CurriculumError>",
    "src/version.rs [pub] fn cohort(&self) -> &AdmissionCohort",
    "src/version.rs [pub] fn disposition(&self) -> CohortTransition",
    "src/version.rs [pub] fn id(&self) -> CurriculumVersionId",
    "src/version.rs [pub] fn institution_path(&self) -> &[String]",
    "src/version.rs [pub] fn institution_segment(mut self, segment: &str) -> Self",
    "src/version.rs [pub] fn new( id: CurriculumVersionId, admission_year_range: (AdmissionCohort, AdmissionCohort), valid_time: ValidInterval, ) -> Self",
    "src/version.rs [pub] fn record( cohort: AdmissionCohort, disposition: CohortTransition, valid_time: ValidInterval, ) -> Result<Self, CurriculumError>",
    "src/version.rs [pub] fn source_snapshot(&self) -> Option<&ContentDigest>",
    "src/version.rs [pub] fn source_snapshot(mut self, digest: ContentDigest) -> Self",
    "src/version.rs [pub] fn status(&self) -> PublicationStatus",
    "src/version.rs [pub] fn status(mut self, status: PublicationStatus) -> Self",
    "src/version.rs [pub] fn supersedes(&self) -> Option<CurriculumVersionId>",
    "src/version.rs [pub] fn supersedes(mut self, earlier: CurriculumVersionId) -> Self",
    "src/version.rs [pub] fn transition(mut self, arrangement: TransitionArrangement) -> Self",
    "src/version.rs [pub] fn transition_for(&self, cohort: &AdmissionCohort) -> CohortTransition",
    "src/version.rs [pub] fn transitions(&self) -> &[TransitionArrangement]",
    "src/version.rs [pub] fn valid_time(&self) -> ValidInterval",
    "src/version.rs [pub] fn valid_time(&self) -> ValidInterval",
];

/// Every `impl` block header this package ships, as `<file>: <header>`.
const IMPL_HEADERS: &[&str] = &[
    "src/course.rs: impl Course",
    "src/course.rs: impl CourseDraft",
    "src/fault.rs: impl PublishCheckpoint",
    "src/fault.rs: impl PublishFaultInjector for NoFault",
    "src/gate.rs: impl OpenGate",
    "src/offering.rs: impl Capacity",
    "src/offering.rs: impl CourseOffering",
    "src/offering.rs: impl CourseOfferingDraft",
    "src/offering.rs: impl GradingMode",
    "src/offering.rs: impl Meeting",
    "src/offering.rs: impl OfferingStatus",
    "src/offering.rs: impl Weekday",
    "src/publish.rs: impl CurriculumLedger",
    "src/publish.rs: impl CurriculumLedger",
    "src/publish.rs: impl CurriculumPublication",
    "src/publish.rs: impl CurriculumPublisher<'static>",
    "src/publish.rs: impl Default for CurriculumPublisher<'static>",
    "src/publish.rs: impl OfficialSourceBinding",
    "src/publish.rs: impl PublishReceipt",
    "src/publish.rs: impl<'injector> CurriculumPublisher<'injector>",
    "src/relation.rs: impl CourseCodeReuse",
    "src/relation.rs: impl CourseRelationKind",
    "src/relation.rs: impl CourseRelations",
    "src/relation.rs: impl EquivalenceRelation",
    "src/relation.rs: impl IdentityDecision",
    "src/relation.rs: impl ReplacementRelation",
    "src/relation.rs: impl RetirementRelation",
    "src/revision.rs: impl CourseRevision",
    "src/revision.rs: impl CourseRevisionDraft",
    "src/revision.rs: impl Credits",
    "src/revision.rs: impl CurriculumCategory",
    "src/revision.rs: impl OfficialPrerequisite",
    "src/revision.rs: impl RecommendedPrerequisite",
    "src/text.rs: impl $name",
    "src/version.rs: impl CohortTransition",
    "src/version.rs: impl CurriculumVersion",
    "src/version.rs: impl CurriculumVersionDraft",
    "src/version.rs: impl PublicationStatus",
    "src/version.rs: impl TransitionArrangement",
];

// ---------------------------------------------------------------------------
// every_declaration_and_impl_in_this_crate_is_pinned
// ---------------------------------------------------------------------------
//
// `P2-A3` measured this crate's blind spot directly: four `impl From<..>` blocks
// appended to a product file gave an external crate a route to a value the
// crate's own doc says has one construction site, and every acceptance test in
// this crate stayed green. A `trait impl` declares no `pub fn`, so a scan built
// on public signatures does not see it, and no scan here counted `impl` blocks
// at all.
//
// `P2-X5` measured the same class as six invisible injections out of nineteen,
// and `P2-Y3` closed it in `crates/cs-map` by pinning the whole set of `impl`
// headers. `academic-review` and `academic-ingestion` were the only two U crates
// carrying that defence. This is it, ported: two whole sets, compared in both
// directions, over every `.rs` file this package ships.
//
// It is deliberately not a list of forbidden spellings. A new function, a new
// method, a new inherent `impl`, a new trait `impl` and a new file all fail as
// an entry nobody wrote down, whatever they are called.

/// Every `.rs` file this package ships: everything outside `tests`.
///
/// The whole package rather than `src`, because `S-12` in
/// `docs/contracts/policy-source-scans.md` is the row about a walk that reads
/// `<crate>/src` and stops seeing product-shaped code beside it --
/// `examples/`, `benches/` and `probes/` are all compiled by
/// `cargo clippy --workspace --all-targets`.
fn inventory_sources() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|name| name == "tests" || name == "target")
                {
                    continue;
                }
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let name = path
                    .strip_prefix(&base)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((name, std::fs::read_to_string(&path)?));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Removes comments, string literals and character literals.
///
/// The raw-string-aware reader from `crates/record/tests/record_scans.rs`,
/// copied deliberately: `P2-G4` found that a lexer without raw strings
/// desynchronizes and reads every literal after one as code.
fn inventory_strip(source: &str) -> String {
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
                let terminator: String = core::iter::once('"')
                    .chain(core::iter::repeat_n('#', hashes))
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

/// Collapses whitespace runs to single spaces.
fn inventory_collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every function declaration in `code`, as a public flag and a signature.
///
/// Visibility is read off the text before `fn` on the same line: `pub(` is
/// crate-private however it continues, a bare `pub` is public, anything else is
/// private. Reading **signatures** rather than names is what makes the pin a
/// statement about what a function takes and returns, so a widened parameter
/// fails as loudly as a new function.
///
/// The `>` of a `->` is skipped: `crates/review`'s copy of this reader records
/// that treating it as a closing bracket truncated `fn counts(self) -> [u32; 5]`
/// to `fn counts(self) -> [u32`, and a pin on a truncated signature is a pin two
/// different signatures satisfy.
fn inventory_declarations(code: &str) -> Vec<(bool, String)> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("fn ") {
        if !(at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_')) {
            continue;
        }
        let line_start = code[..at].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &code[line_start..at];
        let public = prefix.contains("pub") && !prefix.contains("pub(");
        let mut depth = 0_i32;
        let mut end = None;
        let region = &code[at..];
        let region_bytes = region.as_bytes();
        for (offset, character) in region.char_indices() {
            match character {
                '(' | '<' | '[' => depth += 1,
                '>' if offset > 0 && region_bytes[offset - 1] == b'-' => {}
                ')' | '>' | ']' => depth -= 1,
                '{' | ';' if depth <= 0 => {
                    end = Some(at + offset);
                    break;
                }
                _ => {}
            }
        }
        if let Some(end) = end {
            found.push((public, inventory_collapse(&code[at..end])));
        }
    }
    found
}

/// Every `impl` block header in `code`, whole.
///
/// The header is everything from `impl` to the opening brace, so
/// `impl From<usize> for CoverageWitness` and `impl CoverageWitness` are
/// different entries and a trait implementation cannot arrive as an edit to an
/// inherent one.
fn inventory_impl_headers(code: &str) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("impl") {
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        if code[at + 4..]
            .starts_with(|character: char| character.is_alphanumeric() || character == '_')
        {
            continue;
        }
        let Some(end) = code[at..].find(['{', ';']) else {
            continue;
        };
        found.push(inventory_collapse(&code[at..at + end]));
    }
    found
}

/// Nothing this crate declares is outside the two pinned sets.
///
/// Two whole sets, each compared in both directions:
///
/// 1. every function declaration this package ships, as a file, a visibility
///    and a full signature;
/// 2. every `impl` block header this package ships, as a file and a header.
///
/// The second is the one `P2-A3` walked through. Its injection was four
/// `impl From<..>` blocks in a product file -- no `pub fn`, no new name on any
/// forbidden list, no change to any other file -- and it handed an external
/// crate a value the crate's own documentation says it cannot construct. There
/// is no spelling of that injection that this test does not see, because it does
/// not look for spellings: it compares the set.
#[test]
fn every_declaration_and_impl_in_this_crate_is_pinned() -> TestResult {
    let sources = inventory_sources()?;
    assert!(
        sources.len() >= INVENTORY_FILE_FLOOR,
        "the inventory walk read only {} files",
        sources.len()
    );

    let mut declared = Vec::new();
    let mut headers = Vec::new();
    for (name, text) in &sources {
        let code = inventory_strip(text);
        for (public, signature) in inventory_declarations(&code) {
            let visibility = if public { "pub" } else { "priv" };
            declared.push(format!("{name} [{visibility}] {signature}"));
        }
        for header in inventory_impl_headers(&code) {
            headers.push(format!("{name}: {header}"));
        }
    }
    declared.sort();
    headers.sort();

    assert_eq!(
        declared,
        DECLARATIONS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's declaration set changed"
    );
    assert_eq!(
        headers,
        IMPL_HEADERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's impl inventory changed"
    );
    Ok(())
}
