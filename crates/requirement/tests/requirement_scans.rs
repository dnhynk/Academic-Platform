//! Source scans for the `P2-U2` rule DSL.
//!
//! Four of this task's claims are shapes of the source rather than behaviours,
//! so nothing at run time would notice the day they stopped being true: that
//! the rule-type set is the specification's own, that the production audit path
//! has no free-text interpretation and no model in reach, that the review gate
//! is the only route to an executable rule, and that no open section 38 cell
//! has a default. `docs/contracts/policy-source-scans.md` is the page those
//! scans are enumerated on, and this file is written against all five of the
//! empty-scan shapes it names.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends the whole
//! package, not `src` by name, with a floor, a `mod`/`#[path]` tripwire, and a
//! rule that this crate's product source is under `src` and nowhere else.
//!
//! **The checks are not token lists.** The rule-type set is read out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, so a type dropped
//! from the Rust list fails against the specification rather than against a
//! list written here twice. The free-text rule is the *whole set* of `String`
//! and `&str` fields and public parameters in the crate, each with a written
//! reason, so a sentence smuggled in under a name nobody predicted fails as an
//! extra key rather than being searched for by spelling.
//!
//! **The pins fix their callers.** [`WHOLE_ADMIT`] is accompanied by
//! [`WHOLE_INCLUDE`] and by struct-literal counts on both private-field types,
//! because `T141` found a pinned check skipped by a condition wrapped around
//! it and `T149` found a second path that never called one.
//!
//! **Every inventory counts a name, not a spelling.** The counts here are
//! whole-identifier counts with declarations subtracted, so a call written
//! through the type path counts the same as a method call.
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

use academic_requirement::{
    OpenGate, RuleType, SPEC_PROSE_CATEGORIES, SPEC_YAML_TYPES, SpellingSource,
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

/// Every `.rs` file under every workspace package, less each package's `tests`.
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

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/curriculum/tests/curriculum_scans.rs` by way of
/// `crates/record/tests/record_scans.rs`, raw strings and nested block comments
/// included. `P2-G4` found that a lexer without raw strings desynchronizes and
/// reads every literal after one as code.
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

/// Every `impl` block header in `code` that names `type_name` as a whole
/// identifier, whitespace-collapsed.
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

/// The authoritative specification.
fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

// ---------------------------------------------------------------------------
// Whole-text pins. Each is compared against the item as the source declares it,
// comment lines dropped and whitespace collapsed, so `cargo fmt` decides layout
// and the pin decides content.
// ---------------------------------------------------------------------------

/// The whole review gate. Two attestations, both naming this candidate, both
/// filed by a user, the two users different, and the body compiled -- then and
/// only then the one `ReviewedRule` this crate builds.
const WHOLE_ADMIT: &str = "pub fn admit( candidate: RuleCandidate, first: ReviewAttestation, second: ReviewAttestation, ) -> Result<ReviewedRule, RequirementError> { for attestation in [&first, &second] { if attestation.candidate() != candidate.id() { return Err(RequirementError::AttestationNamesAnotherCandidate { named: attestation.candidate().as_str().to_owned(), under_review: candidate.id().as_str().to_owned(), }); } } let first_user = first.user_id()?; let second_user = second.user_id()?; if first_user == second_user { return Err(RequirementError::OneReviewerTwice); } candidate.body.compile(&candidate.id)?; Ok(ReviewedRule { id: candidate.id, body: candidate.body, first, second, source_digest: candidate.source_digest, }) }";

/// The gate's signature alone, as the public-signature sweep renders it.
///
/// Pinned separately from [`WHOLE_ADMIT`] because the sweep compares signatures
/// and the pin above compares bodies: an inventory of doors has to be able to
/// say *which* door, and a body is not what a caller sees.
const GATE_SIGNATURE: &str = "pub fn admit( candidate: RuleCandidate, first: ReviewAttestation, second: ReviewAttestation, ) -> Result<ReviewedRule, RequirementError> {";

/// The whole reviewer check. Only `Actor::User` files an attestation.
const WHOLE_USER_ID: &str = "fn user_id(&self) -> Result<EntityId, RequirementError> { match &self.reviewer { Actor::User { user_id } => Ok(*user_id), other => Err(RequirementError::ReviewerIsNotAUser { actor: other.kind_name(), }), } }";

/// The whole admission of a reviewed rule into a draft: the one place an
/// `ExecutableRule` is built, and the fixtures are evaluated rather than
/// counted.
const WHOLE_INCLUDE: &str = "pub fn include( mut self, reviewed: ReviewedRule, official: &OfficialExampleFixtures, synthetic: &SyntheticTranscriptFixtures, ) -> Result<Self, RequirementError> { if self .rules .iter() .any(|existing| existing.id() == reviewed.id()) { return Err(RequirementError::DuplicateRule { rule: reviewed.id().as_str().to_owned(), }); } let rule = ExecutableRule { id: reviewed.id().clone(), body: reviewed.body().clone(), source_digest: reviewed.source_digest(), }; rule.body.compile(&rule.id)?; let staged = self.as_evaluable(); for case in official.cases().iter().chain(synthetic.cases()) { let outcome = evaluate(&staged, &rule.id, &rule.body, &case.facts)?; if outcome.status != case.expected { return Err(RequirementError::ReleaseFixturesMissing { rule: rule.id.as_str().to_owned(), missing: \"a regression fixture disagrees with the rule\", }); } } self.rules.push(rule); Ok(self) }";

/// The whole ledger publication: the version must be new and the supersession
/// must name the head.
const WHOLE_PUBLISH: &str = "pub fn publish(&mut self, set: RuleSet) -> Result<(), RequirementError> { if self .versions .iter() .any(|existing| existing.version() == set.version()) { return Err(RequirementError::VersionAlreadyPublished { version: set.version().to_string(), }); } let head = self.versions.last().map(RuleSet::version); if set.supersedes() != head { return Err(RequirementError::SupersedesTheWrongVersion { claimed: set .supersedes() .map_or_else(|| \"none\".to_owned(), |version| version.to_string()), actual: head.map_or_else(|| \"none\".to_owned(), |version| version.to_string()), }); } self.versions.push(set); Ok(()) }";

/// Section 11.2's prose sentence, whole. The rule-type categories are read out
/// of this and nothing else.
const SECTION_11_2_SENTENCE: &str = "rule type에는 credit minimum, course set, area distribution, co-requisite, mutually exclusive, equivalency, maximum recognition, GPA, non-credit training, language-of-instruction, thesis/research, exception approval를 포함한다. 자유 텍스트를 LLM이 매번 해석해 졸업 여부를 판단하는 구조는 금지한다. LLM은 원문에서 rule 후보를 추출할 수 있으나 사람이 검토한 executable rule만 production audit에 사용한다.";

/// This crate's transitive workspace dependency closure, computed from the
/// manifests and compared whole.
///
/// Two product edges are declared here; the other ten arrive through
/// `academic-ingestion`. Each is named so an addition of any kind is a review:
///
/// * `academic-domain` -- the identifiers, the exact decimal, the actor, and
///   the five-value proof status.
/// * `academic-ingestion` -- `PublishedRules`, so a rule set founded on an
///   undated official source is not a value that exists.
/// * `academic-egress-boundary`, `academic-policy`, `academic-untrusted-content`
///   -- `academic-ingestion`'s own edges. The third is the crate whose whole
///   purpose is that a provider response cannot be unwrapped into a string
///   without naming the exposure site; it runs no model and calls none.
/// * `hex`, `hmac`, `rusqlite`, `serde`, `sha2`, `thiserror`, `uuid` -- the
///   external crates those reach.
///
/// **No crate here runs, wraps or transports a model call.**
/// `academic-model-run` -- section 27.3's provenance aggregate, which is where a
/// model execution is recorded -- is absent, as is every HTTP client. That is
/// what `production_audit_no_llm` rests on, and it is a fact about the
/// dependency graph rather than a rule inside a function.
const PRODUCT_CLOSURE: [&str; 12] = [
    "academic-domain",
    "academic-egress-boundary",
    "academic-ingestion",
    "academic-policy",
    "academic-untrusted-content",
    "hex",
    "hmac",
    "rusqlite",
    "serde",
    "sha2",
    "thiserror",
    "uuid",
];

/// Every `String` or `&str`-typed field this crate's product source declares,
/// as an owning-type/field pair.
///
/// The pair rather than the name is load-bearing, and for the reason `U-I5`
/// made load-bearing for `P2-U1`: a name compared on its own is satisfied by
/// the same name one type over, which is exactly the move that would put a
/// sentence on the executable side while leaving this list intact.
///
/// Two owners, and only two:
///
/// * `RuleCandidate` -- one field, the official sentence a model read. It is
///   the one type that never reaches an evaluation; it is not forwarded to
///   `ReviewedRule`, and `ExecutableRule` has no field it fits in.
/// * `RequirementError` -- thirteen, every one a diagnostic payload on the
///   refusal path. An error is what a caller is told *instead of* a verdict, so
///   nothing here is read by an evaluation. They are enumerated rather than
///   exempted, because "it is only an error variant" is precisely the sentence
///   under which a free-text rule value would arrive.
///
/// A field on any third type -- a `note` on a rule, a `description` on a set, a
/// `raw` on a fact -- fails as an extra key however it is spelled.
///
/// The identifier newtypes are deliberately not on this list and deliberately
/// not exempt: they hold a `String`, and each is caught by the same sweep and
/// justified in [`IDENTIFIER_NEWTYPES`] instead, because what makes them safe
/// is the validator rather than the field.
const FREE_TEXT_FIELDS: [(&str, &str); 14] = [
    ("RuleCandidate", "quoted_source"),
    ("RequirementError", "actor"),
    ("RequirementError", "actual"),
    ("RequirementError", "claimed"),
    ("RequirementError", "fact"),
    ("RequirementError", "identifier"),
    ("RequirementError", "kind"),
    ("RequirementError", "missing"),
    ("RequirementError", "named"),
    ("RequirementError", "reason"),
    ("RequirementError", "rule"),
    ("RequirementError", "under_review"),
    ("RequirementError", "value"),
    ("RequirementError", "version"),
];

/// The types a rule value can reach an evaluation through.
///
/// None of them may own a free-text field. This is the half that says *where*
/// the two owners above are allowed to be: an inventory alone would still pass
/// if `quoted_source` moved onto `RuleBody`, because the pair would change on
/// one side and could be updated to match. This list cannot be satisfied by
/// updating it, because the whole point is that these types have no such field.
const AUDIT_PATH_TYPES: [&str; 8] = [
    "ReviewedRule",
    "ExecutableRule",
    "RuleBody",
    "RuleSet",
    "RuleSetDraft",
    "AcademicFacts",
    "AttemptFact",
    "RuleOutcome",
];

/// The whole identifier newtype macro.
///
/// Pinned because the six types it generates have no names in the source: they
/// exist after expansion, so no source sweep can see them. What a sweep *can*
/// see is this template and the list of invocations, and together they say what
/// the six types are and that each is a `String` behind [`is_identifier`].
const WHOLE_IDENTIFIER_MACRO: &str = "macro_rules! identifier_newtype { ($name:ident, $kind:literal, $doc:literal) => { #[doc = $doc] #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct $name(String); impl $name { pub fn new(value: &str) -> Result<Self, RequirementError> { if is_identifier(value) { Ok(Self(value.to_owned())) } else { Err(RequirementError::InvalidIdentifier { kind: $kind, value: value.to_owned(), }) } } #[must_use] pub fn as_str(&self) -> &str { &self.0 } } impl core::fmt::Display for $name { fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { formatter.write_str(&self.0) } } }; }";

/// The identifier newtypes, each a `String` behind a validator that admits no
/// space.
const IDENTIFIER_NEWTYPES: [&str; 6] = [
    "RuleId",
    "CreditCategory",
    "AreaId",
    "GpaScope",
    "ProgramId",
    "ApprovalAuthority",
];

// ---------------------------------------------------------------------------
// the_walk_reads_every_module_in_this_crate
// ---------------------------------------------------------------------------

/// The walk every scan below reads through, with a floor and a tripwire.
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

    // Product source lives under `src` and nowhere else, which is the condition
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
    // eight modules there.
    assert!(
        sources
            .iter()
            .any(|path| path.ends_with("requirement_scans.rs")),
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

    // The tripwire. Every `mod name;` and every `#[path = "…"]` in the crate
    // has to name a file the walk read. It fails the day the walk is narrowed,
    // and the day a module is added somewhere the walk does not descend into.
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
                        "{} includes {}, which does not exist",
                        relative(path),
                        target.display()
                    );
                    // A target inside this package has to be a file the walk
                    // read; one outside it -- `P2-U6`'s fixture module -- only
                    // has to exist, because this crate's walk does not descend
                    // into another package.
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
    assert!(declared >= 8, "the crate declares only {declared} modules");
    Ok(())
}

// ---------------------------------------------------------------------------
// the_rule_types_are_the_specifications_own
// ---------------------------------------------------------------------------

/// Section 11.2's own two readings, parsed out of the document and compared
/// with [`RuleType::ALL`] in both directions.
///
/// No count is asserted anywhere in this test. `t068` says *thirteen* and lists
/// fourteen; what decides the set here is the specification, read twice.
#[test]
fn the_rule_types_are_the_specifications_own() -> TestResult {
    let specification = specification()?;

    // The prose sentence, pinned whole. A rule type dropped from the sentence
    // fails here rather than silently shrinking the list below it.
    assert!(
        specification.contains(SECTION_11_2_SENTENCE),
        "section 11.2's sentence is not in the specification as pinned"
    );

    // ---- The yaml reading ------------------------------------------------
    //
    // The fenced block between the `### 11.2` heading and the sentence above.
    let heading = specification
        .find("### 11.2 Typed rule DSL")
        .ok_or("section 11.2 has no heading")?;
    let sentence_at = specification
        .find(SECTION_11_2_SENTENCE)
        .ok_or("section 11.2 has no prose sentence")?;
    let block = &specification[heading..sentence_at];
    let mut yaml_types: Vec<String> = Vec::new();
    for line in block.lines() {
        if let Some(value) = line.trim().strip_prefix("type: ") {
            let value = value.trim().to_owned();
            if !yaml_types.contains(&value) {
                yaml_types.push(value);
            }
        }
    }
    assert!(
        !yaml_types.is_empty(),
        "the yaml walk found no `type:` line, so every comparison below it is empty"
    );
    assert_eq!(
        yaml_types,
        SPEC_YAML_TYPES
            .iter()
            .map(|(spelling, _)| (*spelling).to_owned())
            .collect::<Vec<_>>(),
        "section 11.2's yaml types and SPEC_YAML_TYPES disagree, in order"
    );
    // Every yaml identifier resolves to a rule type, and that type says the
    // specification supplied its spelling.
    for (spelling, rule_type) in SPEC_YAML_TYPES {
        assert_eq!(
            RuleType::parse(spelling),
            Some(rule_type),
            "{spelling} does not parse back to its rule type"
        );
        assert_eq!(
            rule_type.spelling_source(),
            SpellingSource::SpecYaml,
            "{spelling} is written in the specification's yaml and is not recorded as such"
        );
        assert_eq!(rule_type.as_str(), spelling);
    }

    // ---- The prose reading -----------------------------------------------
    let listed = SECTION_11_2_SENTENCE
        .strip_prefix("rule type에는 ")
        .and_then(|rest| rest.split("를 포함한다.").next())
        .ok_or("section 11.2's sentence does not have the shape it is parsed with")?;
    let prose: Vec<&str> = listed.split(", ").map(str::trim).collect();
    assert!(
        prose.len() > 1,
        "the prose split found one item, so the comparison below is vacuous"
    );
    assert_eq!(
        prose,
        SPEC_PROSE_CATEGORIES
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        "section 11.2's prose categories and SPEC_PROSE_CATEGORIES disagree, in order"
    );

    // ---- The two readings together are exactly RuleType::ALL -------------
    //
    // Every rule type is claimed by at least one reading, and every reading
    // lands on a declared rule type. A type this crate invented fails as an
    // extra key; one the specification writes and this crate dropped fails as a
    // missing one.
    let mut claimed: BTreeSet<&str> = BTreeSet::new();
    for (_, types) in SPEC_PROSE_CATEGORIES {
        for rule_type in types {
            claimed.insert(rule_type.as_str());
        }
    }
    for (_, rule_type) in SPEC_YAML_TYPES {
        claimed.insert(rule_type.as_str());
    }
    let declared: BTreeSet<&str> = RuleType::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(
        claimed, declared,
        "the specification's readings and RuleType::ALL are not the same set"
    );

    // ---- The nine prose-only spellings are mechanically derived ----------
    //
    // The document names no identifier for them, so the spelling is this
    // crate's. What keeps it from being an invention is that it is *derived*:
    // upper-case, with each space, hyphen or slash becoming an underscore. A
    // spelling that stopped being that derivation fails here.
    let mut derived = 0_usize;
    for (name, types) in SPEC_PROSE_CATEGORIES {
        if types.len() != 1 {
            // `course set` opens into three yaml types and has no single
            // identifier to derive. Its members are checked by the yaml half.
            continue;
        }
        let rule_type = types[0];
        if rule_type.spelling_source() != SpellingSource::SpecProse {
            continue;
        }
        let mechanical: String = name
            .to_uppercase()
            .chars()
            .map(|character| match character {
                ' ' | '-' | '/' => '_',
                other => other,
            })
            .collect();
        assert_eq!(
            rule_type.as_str(),
            mechanical,
            "{name} is not spelled as the derivation rule produces"
        );
        derived += 1;
    }
    assert_eq!(
        derived,
        RuleType::ALL
            .iter()
            .filter(|kind| kind.spelling_source() == SpellingSource::SpecProse)
            .count(),
        "a prose-spelled rule type is not reached by the derivation check"
    );

    // ---- Each rule type has its own requirement and its own named test ----
    //
    // This is the independent reading: `t001` derived one row per rule type
    // from the specification without reference to `t068`'s count, and `t068`
    // named one `dsl_*` test per row. Both are injective, so neither can absorb
    // a rule type that lost its own identity.
    let requirements: BTreeSet<&str> = RuleType::ALL
        .iter()
        .map(|kind| kind.requirement())
        .collect();
    assert_eq!(
        requirements.len(),
        RuleType::ALL.len(),
        "two rule types share a requirement id"
    );
    let tests: BTreeSet<&str> = RuleType::ALL
        .iter()
        .map(|kind| kind.acceptance_test())
        .collect();
    assert_eq!(
        tests.len(),
        RuleType::ALL.len(),
        "two rule types share an acceptance test name"
    );
    // And every one of those tests exists in this crate's suite.
    let suite = fs::read_to_string(crate_root().join("tests").join("requirement.rs"))?;
    for rule_type in RuleType::ALL {
        assert!(
            suite.contains(&format!("fn {}()", rule_type.acceptance_test())),
            "{} names {} and the suite does not declare it",
            rule_type.as_str(),
            rule_type.acceptance_test()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// production_audit_no_llm
// ---------------------------------------------------------------------------

/// `REQ-11-018`: a production graduation decision cannot be made by having a
/// model reinterpret free text on every run.
///
/// Three halves, in the two shapes section 2.3-14 already establishes for a
/// capability, plus the one that is specific to this task.
///
/// **Available.** The transitive workspace closure is computed from the
/// manifests and compared whole against [`PRODUCT_CLOSURE`]. No crate that
/// runs, wraps or transports a model call is in it, and an addition of any kind
/// fails as an extra key rather than having to be predicted.
///
/// **Used.** The product source is scanned for the API spellings of a model
/// call, a clock, an RNG and a socket, with the samples run through the check
/// inside the test so a rule that matches nothing fails.
///
/// **Interpreted.** The whole set of `String` and `&str` fields in the product
/// source is compared against [`FREE_TEXT_FIELDS`], and the whole set of public
/// functions taking one is compared against a list with a reason each. That is
/// the half a token list cannot do: a sentence parked on a new field under a
/// name nobody predicted appears as an extra key.
#[test]
fn production_audit_no_llm() -> TestResult {
    // ---- Available: the dependency closure -------------------------------
    let closure = workspace_closure("academic-requirement")?;
    assert_eq!(
        closure,
        PRODUCT_CLOSURE
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate's product dependency closure changed; a model, a clock or a \
         socket may now be in reach of the audit path"
    );
    // The closure walk is not vacuous: it reached past the direct edges.
    assert!(
        closure.contains("academic-untrusted-content"),
        "the closure walk stopped at the direct edges"
    );
    // And the crate that records a model execution is not in it.
    for model_crate in [
        "academic-model-run",
        "academic-worker",
        "academic-connector",
        "reqwest",
        "hyper",
        "tokio",
    ] {
        assert!(
            !closure.contains(model_crate),
            "{model_crate} is in the audit path's dependency closure"
        );
    }

    // ---- Used: the API spellings -----------------------------------------
    let forbidden: [(&str, &[&str]); 4] = [
        (
            "model",
            &[
                "ModelRun",
                "ModelProvider",
                "InferenceRun",
                "AcceptedResponse",
            ],
        ),
        (
            "clock",
            &["SystemTime", "Instant::", "std::time", "chrono::", "now_v7"],
        ),
        (
            "RNG",
            &["getrandom", "rand::", "thread_rng", "OsRng", "new_v4"],
        ),
        (
            "network",
            &[
                "TcpStream",
                "TcpListener",
                "UdpSocket",
                "std::net",
                "reqwest",
            ],
        ),
    ];
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for (capability, spellings) in forbidden {
            for spelling in spellings {
                assert!(
                    !code.contains(spelling),
                    "{} reaches for a {capability} capability: {spelling}",
                    relative(&path)
                );
            }
        }
        scanned += 1;
    }
    assert!(
        scanned >= 8,
        "the capability scan read only {scanned} product files"
    );
    // The scan is not vacuous: each rule matches the call it forbids.
    for (capability, spellings) in forbidden {
        for spelling in spellings {
            let sample = format!("let value = {spelling}call();");
            assert!(
                sample.contains(spelling),
                "the {capability} rule matches nothing"
            );
        }
    }

    // ---- Interpreted: the whole free-text inventory ----------------------
    //
    // Every struct field whose declared type is `String`, `&str` or `str`,
    // across the product source, read as `name: Type` at any indentation.
    let mut fields: BTreeSet<(String, String)> = BTreeSet::new();
    let mut newtypes: BTreeSet<String> = BTreeSet::new();
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let mut current_type = String::new();
        for line in code.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed
                .strip_prefix("pub enum ")
                .or_else(|| trimmed.strip_prefix("enum "))
                .or_else(|| trimmed.strip_prefix("pub struct "))
                .or_else(|| trimmed.strip_prefix("struct "))
            {
                current_type = rest
                    .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                // A tuple newtype over `String` is a field too, spelled
                // without a name. A macro *template* -- `pub struct
                // $name(String);` -- is not a type and is skipped here; the
                // types it generates are collected from its invocations below,
                // because a name that only exists after expansion is not in the
                // source at all. Reading the template as though it were a type
                // is how the first version of this sweep reported one newtype
                // called the empty string and none of the six real ones.
                if rest.contains("(String)")
                    && current_type
                        .chars()
                        .next()
                        .is_some_and(|first| first.is_ascii_uppercase())
                {
                    newtypes.insert(current_type.clone());
                }
                continue;
            }
            let Some((name, declared)) = trimmed.trim_end_matches(',').split_once(": ") else {
                continue;
            };
            let name = name.trim_start_matches("pub ").trim();
            if !name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
                || name.is_empty()
            {
                continue;
            }
            let declared = declared.trim();
            if matches!(declared, "String" | "&str" | "&'static str" | "str") {
                fields.insert((current_type.clone(), name.to_owned()));
            }
        }
    }
    assert!(
        !fields.is_empty(),
        "the field sweep found nothing, so the comparison below is vacuous"
    );
    assert_eq!(
        fields,
        FREE_TEXT_FIELDS
            .iter()
            .map(|(owner, name)| ((*owner).to_owned(), (*name).to_owned()))
            .collect::<BTreeSet<_>>(),
        "a free-text field entered this crate; every one has to be named against \
         its owning type in FREE_TEXT_FIELDS"
    );
    // And no type a rule value reaches an evaluation through owns one. This is
    // the half that cannot be satisfied by editing the table above.
    for (owner, name) in &fields {
        assert!(
            !AUDIT_PATH_TYPES.contains(&owner.as_str()),
            "{owner} is on the audit path and owns a free-text field `{name}`"
        );
    }
    // The sweep is not vacuous about owners either: it found the candidate's
    // field under the candidate's own name, which is the pairing `U-I5` says a
    // name-only comparison would lose.
    assert!(
        fields.contains(&("RuleCandidate".to_owned(), "quoted_source".to_owned())),
        "the sweep did not attribute quoted_source to RuleCandidate"
    );

    // The identifier newtypes are a `String` behind a validator that admits no
    // space, so they are enumerated rather than exempted. They are generated by
    // a macro, so the sweep above cannot see their names: what is compared is
    // the macro's own invocation list, and the macro body is pinned whole so
    // the invocations are known to produce a validated `String` newtype and not
    // something else.
    let dsl = fs::read_to_string(crate_root().join("src").join("dsl.rs"))?;
    assert_eq!(
        declared_member(&dsl, "macro_rules! identifier_newtype {", "\n}")?,
        WHOLE_IDENTIFIER_MACRO,
        "the identifier newtype macro changed"
    );
    // Read over the whitespace-collapsed file rather than line by line: `cargo
    // fmt` puts each invocation's arguments on their own lines, so the name
    // does not sit on the line the macro is named on and a per-line reader
    // finds nothing.
    let collapsed = strip_non_code(&dsl)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut invocations = 0_usize;
    for (at, _) in collapsed.match_indices("identifier_newtype!(") {
        let rest = &collapsed[at + "identifier_newtype!(".len()..];
        let name: String = rest
            .trim_start()
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        assert!(
            !name.is_empty(),
            "an identifier_newtype! invocation names nothing"
        );
        newtypes.insert(name);
        invocations += 1;
    }
    assert!(
        invocations > 0,
        "the invocation reader found no identifier_newtype! call, so the \
         comparison below would pass on an empty set"
    );
    assert_eq!(
        newtypes,
        IDENTIFIER_NEWTYPES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "a String newtype entered this crate outside the identifier set"
    );
    let validator = declared_member(&dsl, "fn is_identifier(value: &str) -> bool", "\n}")?;
    assert!(
        validator.contains("is_ascii_alphanumeric")
            && validator.contains("b'.' | b'_' | b'-'")
            && validator.contains("value.len() <= 128"),
        "the identifier validator stopped refusing everything but the narrow set: {validator}"
    );
    // And every one of them is validated: the macro body is the only
    // constructor, and it calls the validator above.
    assert!(
        WHOLE_IDENTIFIER_MACRO.contains("if is_identifier(value)"),
        "the identifier newtype macro stopped validating"
    );

    // And the executable half carries no sentence at all: neither the reviewed
    // rule nor the executable rule declares the one allowed field.
    let candidate = fs::read_to_string(crate_root().join("src").join("candidate.rs"))?;
    let publish = fs::read_to_string(crate_root().join("src").join("publish.rs"))?;
    let reviewed = declared_member(&candidate, "pub struct ReviewedRule {", "\n}")?;
    let executable = declared_member(&publish, "pub struct ExecutableRule {", "\n}")?;
    for (label, declaration) in [("ReviewedRule", &reviewed), ("ExecutableRule", &executable)] {
        // Whole identifiers, not substrings. `contains("str")` is true of every
        // declaration ever written, because `struct` contains it -- which is
        // the "a spelling, not a structure" defect one page over, arriving as a
        // false positive instead of a miss.
        assert!(
            uses_of(declaration, "String") == 0
                && uses_of(declaration, "str") == 0
                && uses_of(declaration, "Cow") == 0,
            "{label} declares a free-text field: {declaration}"
        );
        assert!(
            !declaration.contains("quoted_source"),
            "{label} carries the candidate's quotation forward"
        );
    }
    Ok(())
}

/// The transitive workspace dependency closure of one package, from manifests.
///
/// `cargo tree` is what `tools/phase1-scaffold-policy.test.mjs` uses for the
/// same question, and it runs outside `cargo test`. Reading the manifests is
/// the same measurement without a nested cargo invocation: every `[dependencies]`
/// entry of every workspace crate reachable from the root, plus the external
/// names those entries spell. `[dev-dependencies]` is deliberately outside it,
/// because a test dependency is not in the product graph -- the same boundary
/// `docs/contracts/engine-harness.md` draws.
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
// the_only_route_to_an_executable_rule_is_the_gate
// ---------------------------------------------------------------------------

/// The source half of `rule_candidate_review_gate`.
///
/// The compile-fail cases observe that five named routes are absent. This is
/// the half that says there is no sixth: the gate is pinned whole, the two
/// private-field types are each built in exactly one place, and the whole
/// `impl` set naming either of them is compared against a pinned list, so a
/// `From`, a `Deref` or an `AsRef` nobody predicted fails as an extra key.
#[test]
fn the_only_route_to_an_executable_rule_is_the_gate() -> TestResult {
    let candidate_src = fs::read_to_string(crate_root().join("src").join("candidate.rs"))?;
    let publish_src = fs::read_to_string(crate_root().join("src").join("publish.rs"))?;

    // The pins. Editing any of them is a review.
    assert_eq!(
        declared_member(&candidate_src, "pub fn admit(", "\n    }")?,
        WHOLE_ADMIT,
        "the review gate changed"
    );
    assert_eq!(
        declared_member(&candidate_src, "fn user_id(&self)", "\n    }")?,
        WHOLE_USER_ID,
        "the reviewer check changed"
    );
    assert_eq!(
        declared_member(&publish_src, "pub fn include(", "\n    }")?,
        WHOLE_INCLUDE,
        "the admission of a reviewed rule into a draft changed"
    );
    assert_eq!(
        declared_member(
            &publish_src,
            "pub fn publish(&mut self, set: RuleSet)",
            "\n    }"
        )?,
        WHOLE_PUBLISH,
        "the ledger publication changed"
    );

    // The construction sites. Counted over the whole product source, so a
    // second literal in a sibling module fails even though the pins above are
    // byte-identical -- which is the shape `U-I24` made load-bearing for
    // `P2-U1`.
    let mut literals: BTreeMap<&str, usize> = BTreeMap::new();
    let mut files_read = 0_usize;
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for type_name in ["ReviewedRule", "ExecutableRule"] {
            *literals.entry(type_name).or_default() += code
                .match_indices(&format!("{type_name} {{"))
                .filter(|(at, _)| {
                    // A struct literal, not a `struct`/`impl` declaration.
                    let before = code[..*at].trim_end();
                    !before.ends_with("struct") && !before.ends_with("impl")
                })
                .count();
        }
        files_read += 1;
    }
    assert!(
        files_read >= 8,
        "the literal sweep read only {files_read} files"
    );
    assert_eq!(
        literals.get("ReviewedRule").copied(),
        Some(1),
        "ReviewedRule is built somewhere other than the review gate"
    );
    assert_eq!(
        literals.get("ExecutableRule").copied(),
        Some(1),
        "ExecutableRule is built somewhere other than RuleSetDraft::include"
    );

    // The whole `impl` set naming either type, over every product file. The
    // type and the trait would both be local, so the orphan rule refuses a
    // conversion written in another crate and refuses nothing written in a
    // sibling module here.
    let mut headers: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        let code = fs::read_to_string(&path)?;
        for type_name in ["ReviewedRule", "ExecutableRule"] {
            headers.extend(impl_headers_naming(&code, type_name));
        }
    }
    headers.sort();
    assert_eq!(
        headers,
        vec![
            "impl ExecutableRule {".to_owned(),
            "impl ReviewedRule {".to_owned(),
        ],
        "a conversion into or out of a gated type was added"
    );

    // The whole set of public signatures anywhere in the crate that take a
    // `RuleCandidate` and return a gated type, compared against a one-entry
    // list. A blanket refusal would have been wrong -- the gate is exactly that
    // signature, and it is the route that must exist. What must not exist is a
    // *second* one, and an inventory says that where a prohibition cannot.
    //
    // This is the shape that catches a conversion whose name spells no trait --
    // `P2-U1`'s `U-I7` was a `ReplacementRelation::implied_identity`.
    let mut doors: Vec<String> = Vec::new();
    let mut signatures = 0_usize;
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for signature in public_signatures(&code) {
            signatures += 1;
            let (parameters, returns) = split_signature(&signature);
            if uses_of(parameters, "RuleCandidate") > 0
                && (uses_of(returns, "ReviewedRule") > 0 || uses_of(returns, "ExecutableRule") > 0)
            {
                doors.push(signature);
            }
        }
    }
    assert!(
        signatures >= 40,
        "the signature sweep found only {signatures} public signatures"
    );
    doors.sort();
    assert_eq!(
        doors,
        vec![GATE_SIGNATURE.to_owned()],
        "the routes from a candidate to a gated value are not exactly the review gate"
    );

    // The sweep is not vacuous: a second door injected into the check here is
    // seen as one.
    let injected = "pub fn widen(candidate: RuleCandidate) -> ReviewedRule {";
    let (parameters, returns) = split_signature(injected);
    assert!(
        uses_of(parameters, "RuleCandidate") > 0 && uses_of(returns, "ReviewedRule") > 0,
        "the signature rule does not catch the shape it is written against"
    );
    Ok(())
}

/// Every `pub fn` signature in already-stripped code, whitespace-collapsed.
fn public_signatures(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let lines: Vec<&str> = code.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("pub fn ") || trimmed.starts_with("pub const fn ")) {
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
fn split_signature(signature: &str) -> (&str, &str) {
    let open = signature.find('(').map_or(0, |at| at + 1);
    let close = signature.rfind(')').unwrap_or(signature.len());
    let parameters = signature.get(open..close).unwrap_or_default();
    let returns = signature
        .split("->")
        .nth(1)
        .unwrap_or("")
        .trim_end_matches('{')
        .trim();
    (parameters, returns)
}

// ---------------------------------------------------------------------------
// the_open_gates_have_no_default
// ---------------------------------------------------------------------------

/// The four open section 38 cells have no value standing in for them.
#[test]
fn the_open_gates_have_no_default() -> TestResult {
    // Every `Default` implementation in the crate, compared whole.
    let mut headers: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        let code = fs::read_to_string(&path)?;
        headers.extend(impl_headers_naming(&code, "Default"));
        // A derive is the other way a `Default` arrives, and it does not spell
        // `impl`.
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[derive(") && uses_of(trimmed, "Default") > 0 {
                headers.push(format!("derive on the item after: {trimmed}"));
            }
        }
    }
    headers.sort();
    assert_eq!(
        headers,
        vec!["derive on the item after: #[derive(Debug, Clone, Default)]".to_owned()],
        "a Default entered this crate; an open section 38 cell must have no value \
         standing in for it"
    );
    // The one that exists is the empty ledger, which is emptiness rather than a
    // value.
    let publish = fs::read_to_string(crate_root().join("src").join("publish.rs"))?;
    let derive_at = publish
        .find("#[derive(Debug, Clone, Default)]")
        .ok_or("the one Default derive is not in publish.rs")?;
    assert!(
        publish[derive_at..]
            .starts_with("#[derive(Debug, Clone, Default)]\npub struct RuleSetLedger"),
        "the one Default is no longer on the empty ledger"
    );

    // Each of the four cells names its identifier and states where it bites.
    let mut identifiers: BTreeSet<&str> = BTreeSet::new();
    for gate in OpenGate::ALL {
        assert!(
            gate.statement().contains(gate.identifier()),
            "{} does not state its own identifier",
            gate.identifier()
        );
        assert!(
            gate.statement().contains("UNKNOWN"),
            "{} does not say that absence reads UNKNOWN",
            gate.identifier()
        );
        identifiers.insert(gate.identifier());
    }
    assert_eq!(
        identifiers,
        BTreeSet::from(["GATE-38-011", "GATE-38-012", "GATE-38-015", "GATE-38-016"]),
        "the open cells are not the four this task leaves open"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// no_float_reaches_a_requirement_verdict
// ---------------------------------------------------------------------------

/// No float type, decimal-point literal or exponent literal in the product
/// source.
///
/// `crates/record/tests/record_scans.rs` holds the same rule over the
/// grade-point engine. It is here as well because a requirement threshold is
/// compared against a grade-point reading, and a comparison done in binary
/// floating point would put 2.0 on the wrong side of itself for some readings.
/// The comparison in `evaluate.rs` is a cross-multiplication over `i128`, so
/// nothing divides and nothing rounds.
#[test]
fn no_float_reaches_a_requirement_verdict() -> TestResult {
    let offending = |code: &str| -> Vec<String> {
        let mut found = Vec::new();
        for word in code.split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '.')
        }) {
            if matches!(word, "f32" | "f64") {
                found.push(word.to_owned());
                continue;
            }
            // A decimal-point literal: digits, a dot, digits.
            let mut parts = word.splitn(2, '.');
            let (Some(before), Some(after)) = (parts.next(), parts.next()) else {
                continue;
            };
            if !before.is_empty()
                && before.chars().all(|character| character.is_ascii_digit())
                && !after.is_empty()
                && after.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                found.push(word.to_owned());
            }
        }
        // An exponent literal. It is scanned by index rather than by word,
        // because the sign is not a word character: splitting on non-word
        // characters cuts `20e-1` into `20e` and `1`, and neither half is an
        // exponent. The first version of this check did exactly that and let
        // its own declared evasion through.
        let characters: Vec<char> = code.chars().collect();
        for (at, character) in characters.iter().enumerate() {
            if *character != 'e' && *character != 'E' {
                continue;
            }
            // At least one digit before, and what precedes those digits is not
            // an identifier character -- so `size` and `base64encode` are not
            // exponents.
            let mut back = at;
            while back > 0 && characters[back - 1].is_ascii_digit() {
                back -= 1;
            }
            if back == at {
                continue;
            }
            if back > 0 && (characters[back - 1].is_alphanumeric() || characters[back - 1] == '_') {
                continue;
            }
            // Optionally a sign, then at least one digit.
            let mut forward = at + 1;
            if matches!(characters.get(forward), Some('+' | '-')) {
                forward += 1;
            }
            if characters.get(forward).is_some_and(char::is_ascii_digit) {
                let literal: String = characters[back..=forward].iter().collect();
                found.push(literal);
            }
        }
        found
    };

    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let found = offending(&code);
        assert!(
            found.is_empty(),
            "{} reaches for binary floating point: {found:?}",
            relative(&path)
        );
        scanned += 1;
    }
    assert!(scanned >= 8, "the float scan read only {scanned} files");

    // The check is not vacuous: five evasions are run through it here and each
    // must be caught.
    for evasion in [
        "let threshold: f64 = 2.0;",
        "let threshold = 2.0_f32;",
        "let threshold = 20e-1;",
        "type Grade = f64; fn threshold() -> Grade { 2.0 }",
        "let ratio = points as f64 / credits as f64;",
    ] {
        assert!(
            !offending(evasion).is_empty(),
            "the float rule does not catch {evasion}"
        );
    }
    // And four benign shapes are not caught, so the rule is usable.
    for benign in [
        "let threshold = Decimal::new(20, 1)?;",
        "let scaled = 10_i128.checked_pow(u32::from(scale));",
        "self.versions.last()",
        "let e = 1; let value = e + 1;",
    ] {
        assert!(
            offending(benign).is_empty(),
            "the float rule falsely catches {benign}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// no_file_outside_this_crate_names_a_requirement_rule
// ---------------------------------------------------------------------------

/// The one-step-out inventory.
///
/// Every rule above is about `crates/requirement`. This walks every product
/// file in every other workspace package and requires none of them to name the
/// gated types, the rule type, or the published set. It is empty today, and a
/// file added to it is a review rather than a silent second implementation.
#[test]
fn no_file_outside_this_crate_names_a_requirement_rule() -> TestResult {
    let root = crate_root();
    let mut naming: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut outside = 0_usize;
    let mut inside = 0_usize;
    for path in workspace_product_sources()? {
        if path.starts_with(&root) {
            inside += 1;
            continue;
        }
        outside += 1;
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for type_name in [
            "RuleCandidate",
            "ReviewedRule",
            "ExecutableRule",
            "ReviewGate",
            "ReviewAttestation",
            "RuleSetLedger",
            "OfficialExampleFixtures",
            "SyntheticTranscriptFixtures",
        ] {
            if uses_of(&code, type_name) > 0 {
                naming
                    .entry(type_name.to_owned())
                    .or_default()
                    .push(relative(&path));
            }
        }
    }
    assert!(
        outside >= 150,
        "the workspace walk read only {outside} files outside this crate"
    );
    assert!(
        inside >= 8,
        "the workspace walk read only {inside} files inside this crate, so it is \
         not reaching this package and the exclusion above proves nothing"
    );
    assert_eq!(
        naming,
        BTreeMap::new(),
        "a file outside this crate names a gated rule type"
    );
    Ok(())
}
