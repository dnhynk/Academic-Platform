//! Source scans for `P2-R2`.
//!
//! `docs/contracts/policy-source-scans.md` is this repository's inventory of
//! files that read another file's Rust source text; this is one of them, and it
//! is written against the five shapes that page says make a scan empty.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends from the
//! package root rather than into `src` by name, has a floor, and carries a
//! tripwire requiring every `mod name;` and every `#[path = "…"]` in the package
//! to be a file the walk read. That is `S-12`: a walk rooted at `<crate>/src`
//! reads `examples/`, `benches/`, `probes/` and `tests/` not at all.
//!
//! **The checks are not token lists.** The decisions this crate makes are
//! pinned as whole text — [`WHOLE_CLASSIFY`] is the one producer of a finding,
//! [`WHOLE_SEAL_FINDING`] is the rung decision, [`WHOLE_SUPPORT`] is the
//! coverage matrix, [`WHOLE_COMPONENT_ID`] is the refusal of the repository
//! root — and the two inventories that could have been token lists are whole
//! sets instead: [`USE_ITEMS`] is every `use` in the crate's product code and
//! [`TEXT_ACCESSORS`] is every public function that returns text. A filesystem
//! import, a transport import, or an accessor handing out a symbol name appears
//! in one of them as an extra key whatever it is called.
//!
//! **The pins fix their callers too.** [`CALL_SITE_COUNTS`] counts each guarded
//! name's call sites over every file the walk read and names the one file each
//! may be called from, because a pin on a body says nothing about whether a
//! second body exists beside it.
//!
//! **Every inventory counts an identifier, not a spelling.** [`calls_of`]
//! counts a whole identifier and subtracts that name's own declarations, so a
//! function whose name merely starts with a guarded one does not cancel it.
//!
//! The helpers are copied from `crates/repository/tests/repository_scans.rs`,
//! which copied its stripper from `crates/record/tests/record_scans.rs`. The
//! copy is deliberate: `P2-G4` found that a lexer without raw strings
//! desynchronizes and reads every literal after one as code.

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

/// Every `.rs` file that ships, which is every one outside `tests`.
fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    Ok(crate_all_sources()?
        .into_iter()
        .filter(|path| {
            !path
                .strip_prefix(&root)
                .unwrap_or(path)
                .starts_with("tests")
        })
        .collect())
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
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth = depth.saturating_add(1),
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(collapse(&source[start..open + offset + 1]));
                }
            }
            _ => (),
        }
    }
    Err(format!("{header} is not brace-balanced").into())
}

/// Drops comment lines and collapses whitespace.
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
// The pins.
// ---------------------------------------------------------------------------

/// The one producer of a [`crate::Finding`], whole.
///
/// The grouping by component, the package-level join, the exclusion of another
/// package's sites, and the leftover pass are all here. A finding that spanned
/// components, or a grouping that fell back to one wide scope, edits this
/// constant.
const WHOLE_CLASSIFY: &str = "impl EvidenceLadder { pub fn classify( analysis: &RepositoryAnalysis, subject: &Subject, calibration: &CalibrationRegistry, purpose: &Purpose, traces: &[RuntimeTrace], now: u64, ) -> Result<Vec<Finding>, AnalysisError> { let (sites, excluded) = collect(analysis, subject); if sites.is_empty() { return Err(AnalysisError::NoPromotingEvidence( subject.id.as_str().to_owned(), )); } let package_sites: Vec<&Site> = sites.iter().filter(|site| site.is_package()).collect(); let mut by_component: BTreeMap<ComponentId, Vec<&Site>> = BTreeMap::new(); for site in sites.iter().filter(|site| !site.is_package()) { by_component .entry(site.component.clone()) .or_default() .push(site); } let traced = traces.iter().any(|trace| { trace.subject == subject.id && trace.snapshot_id == analysis.snapshot_id() }); let mut findings = Vec::new(); let mut packages_covered: Vec<Option<PackageId>> = Vec::new(); for (component, component_sites) in &by_component { let package = component_sites .first() .and_then(|site| site.package.clone()); packages_covered.push(package.clone()); let mut counted: Vec<&Site> = component_sites.clone(); counted.extend( package_sites .iter() .copied() .filter(|site| site.package == package), ); let mut excluded_here = excluded.clone(); for site in package_sites.iter().filter(|site| site.package != package) { excluded_here.push(ExcludedSite::new( site.locator.clone(), ExclusionReason::OtherPackage, )); } findings.push(seal_finding( analysis, subject, component.clone(), &counted, excluded_here, traced, calibration, purpose, now, )?); } let mut leftover: BTreeMap<Option<PackageId>, Vec<&Site>> = BTreeMap::new(); for site in &package_sites { if packages_covered.contains(&site.package) { continue; } leftover.entry(site.package.clone()).or_default().push(site); } for counted in leftover.values() { let component = counted .iter() .map(|site| site.component.clone()) .min() .ok_or_else(|| { AnalysisError::NoPromotingEvidence(subject.id.as_str().to_owned()) })?; findings.push(seal_finding( analysis, subject, component, counted, excluded.clone(), traced, calibration, purpose, now, )?); } if findings.is_empty() { return Err(AnalysisError::NoPromotingEvidence( subject.id.as_str().to_owned(), )); } Ok(findings) } }";

/// The rung decision, whole.
///
/// Section 17.3's five rows are tried downwards, and the order is what stops a
/// test-only use being read as a production one. A reordering, a widened
/// condition, or an `OBSERVED` arm that stopped requiring a calibrated number
/// edits this constant.
const WHOLE_SEAL_FINDING: &str = "fn seal_finding( analysis: &RepositoryAnalysis, subject: &Subject, component: ComponentId, counted: &[&Site], excluded: Vec<ExcludedSite>, traced: bool, calibration: &CalibrationRegistry, purpose: &Purpose, now: u64, ) -> Result<Finding, AnalysisError> { let has = |kind: SiteKind| counted.iter().any(|site| site.kind == kind); let uses: Vec<&&Site> = counted .iter() .filter(|site| site.kind != SiteKind::Manifest) .collect(); let production_config = counted.iter().any(|site| { site.kind == SiteKind::Config && site.locator.scope() == ArtifactScope::Production }); let test_only = has(SiteKind::ReachableCall) && has(SiteKind::Config) && uses .iter() .all(|site| site.locator.scope() == ArtifactScope::Test); let rung = if traced && production_config { LadderRung::RuntimeAndProductionConfig } else if test_only { LadderRung::TestScopedUse } else if has(SiteKind::ReachableCall) && has(SiteKind::Config) { LadderRung::ReachableCallWithConfig } else if !uses.is_empty() { LadderRung::UnreachableImport } else { LadderRung::ManifestPresence }; let artifact_scope = match rung { LadderRung::TestScopedUse => ArtifactScope::Test, LadderRung::ManifestPresence | LadderRung::UnreachableImport | LadderRung::ReachableCallWithConfig | LadderRung::RuntimeAndProductionConfig => counted .iter() .map(|site| site.locator.scope()) .fold(ArtifactScope::Test, ArtifactScope::max), }; let strength = match rung { LadderRung::RuntimeAndProductionConfig => EvidenceStrength::Strong, LadderRung::ManifestPresence | LadderRung::UnreachableImport | LadderRung::ReachableCallWithConfig | LadderRung::TestScopedUse => EvidenceStrength::Static, }; let tier = match rung.tier() { crate::finding::EvidenceTier::PresentOnly => TierEvidence::PresentOnly, crate::finding::EvidenceTier::Possible => TierEvidence::Possible, crate::finding::EvidenceTier::Observed => TierEvidence::Observed( calibrated_confidence(analysis, counted, traced, calibration, purpose, now)?, ), }; let enclosing: Vec<&SymbolFingerprint> = counted .iter() .filter(|site| site.kind != SiteKind::Manifest) .filter_map(|site| site.enclosing.as_ref()) .collect(); let one_symbol = enclosing.len() == uses.len() && !enclosing.is_empty() && enclosing.windows(2).all(|pair| pair[0] == pair[1]); let scope = if one_symbol { FindingScope::Symbol { component, symbol: enclosing[0].clone(), } } else { FindingScope::Component { component } }; Ok(Finding::seal( analysis.snapshot_id().to_owned(), subject.id.as_str().to_owned(), scope, tier, rung, artifact_scope, strength, counted.iter().map(|site| site.locator.clone()).collect(), excluded, ComponentCoverage::new(1, analysis.analyzed_component_count()), )) }";

/// The one route from evidence to a number a reader may see, whole.
///
/// The raw unit is a count of corroborating evidence kinds; what makes it
/// displayable is `P2-M1`'s registry. A weighting invented here, or a
/// `DisplayedConfidence` built from anything but an interpreted score, edits
/// this constant.
const WHOLE_CALIBRATED_CONFIDENCE: &str = "fn calibrated_confidence( analysis: &RepositoryAnalysis, counted: &[&Site], traced: bool, calibration: &CalibrationRegistry, purpose: &Purpose, now: u64, ) -> Result<DisplayedConfidence, AnalysisError> { let kinds = [ SiteKind::Manifest, SiteKind::Import, SiteKind::ReachableCall, SiteKind::Config, ]; let corroborating = u32::try_from( kinds .iter() .filter(|kind| counted.iter().any(|site| site.kind == **kind)) .count(), ) .unwrap_or(0) + u32::from(traced); let units = corroborating.saturating_mul(200); let score = RawScore::new( analysis.provider().clone(), analysis.model_version().clone(), units, ); let calibrated = calibration .interpret(&score, purpose, now) .map_err(|error| AnalysisError::UncalibratedConfidence(error.to_string()))?; Ok(DisplayedConfidence::of(&calibrated)) }";

/// The coverage support matrix, whole.
///
/// Total over `FileKind` and `IndexKind` with no default arm. A file kind
/// quietly given `NotApplicable` where it should be a gap edits this constant.
const WHOLE_SUPPORT: &str = "pub const fn support(file: FileKind, index: IndexKind) -> Support { match file { FileKind::Unsupported => Support::Unsupported, FileKind::RustSource | FileKind::TypeScriptSource | FileKind::PythonSource => match index { IndexKind::Ast | IndexKind::Symbol | IndexKind::CallFlow | IndexKind::DataFlow => { Support::Analyzed } IndexKind::Config => Support::Analyzed, IndexKind::Schema | IndexKind::Iac => Support::NotApplicable, }, FileKind::SqlScript => match index { IndexKind::Ast | IndexKind::Schema => Support::Analyzed, IndexKind::Symbol | IndexKind::CallFlow | IndexKind::DataFlow | IndexKind::Config | IndexKind::Iac => Support::NotApplicable, }, FileKind::CargoManifest | FileKind::NodeManifest | FileKind::PythonManifest | FileKind::LockFile => match index { IndexKind::Ast | IndexKind::Config => Support::Analyzed, IndexKind::Symbol | IndexKind::CallFlow | IndexKind::DataFlow | IndexKind::Schema | IndexKind::Iac => Support::NotApplicable, }, FileKind::ConfigDocument => match index { IndexKind::Ast | IndexKind::Config => Support::Analyzed, IndexKind::Symbol | IndexKind::CallFlow | IndexKind::DataFlow | IndexKind::Schema | IndexKind::Iac => Support::NotApplicable, }, FileKind::ContainerFile | FileKind::ComposeFile | FileKind::CiWorkflow => match index { IndexKind::Ast | IndexKind::Config | IndexKind::Iac => Support::Analyzed, IndexKind::Symbol | IndexKind::CallFlow | IndexKind::DataFlow | IndexKind::Schema => { Support::NotApplicable } }, FileKind::Prose => match index { IndexKind::Ast | IndexKind::Symbol | IndexKind::CallFlow | IndexKind::DataFlow | IndexKind::Schema | IndexKind::Config | IndexKind::Iac => Support::NotApplicable, }, } }";

/// The refusal of the repository root, whole.
///
/// `ComponentId::new` refuses every spelling of the root and `containing`
/// never widens a root-level file to it. This is the runtime half of
/// `new_finding_cannot_default_to_repository_scope`.
const WHOLE_COMPONENT_ID: &str = "impl ComponentId { pub fn new(value: impl Into<String>) -> Result<Self, ComponentError> { let value = value.into(); if value.is_empty() || value == \".\" || value == \"/\" || value == \"./\" { return Err(ComponentError::RepositoryRoot(value)); } let malformed = value.starts_with('/') || value.contains('\\\\') || value.contains(':') || value .split('/') .any(|segment| segment.is_empty() || segment == \".\" || segment == \"..\"); if malformed { return Err(ComponentError::Malformed(value)); } Ok(Self { directory: value }) } pub fn containing(path: &str) -> Result<Self, ComponentError> { let (parent, _) = split_parent(path); if parent == \".\" { return Self::new(path); } Self::new(parent) } #[must_use] pub fn as_str(&self) -> &str { &self.directory } }";

/// The fold from section 17.3's five rows onto `REQ-34-081`'s three badges,
/// whole.
///
/// Which row becomes which value is the contract, so it is one total function
/// and it is pinned. A row moved from `POSSIBLE` to `OBSERVED` is the exact
/// over-claim this task is about, and it edits this constant.
const WHOLE_LADDER_RUNG: &str = "impl LadderRung { pub const ALL: [Self; 5] = [ Self::ManifestPresence, Self::UnreachableImport, Self::ReachableCallWithConfig, Self::TestScopedUse, Self::RuntimeAndProductionConfig, ]; #[must_use] pub const fn as_str(self) -> &'static str { match self { Self::ManifestPresence => \"MANIFEST_PRESENCE\", Self::UnreachableImport => \"UNREACHABLE_IMPORT\", Self::ReachableCallWithConfig => \"REACHABLE_CALL_WITH_CONFIG\", Self::TestScopedUse => \"TEST_SCOPED_USE\", Self::RuntimeAndProductionConfig => \"RUNTIME_AND_PRODUCTION_CONFIG\", } } #[must_use] pub const fn tier(self) -> EvidenceTier { match self { Self::ManifestPresence => EvidenceTier::PresentOnly, Self::UnreachableImport => EvidenceTier::Possible, Self::ReachableCallWithConfig | Self::TestScopedUse | Self::RuntimeAndProductionConfig => EvidenceTier::Observed, } } }";

/// The promotion axis, whole.
///
/// Four classes and exactly one of them promotes. A `Vendored` arm that
/// started returning `true` edits this constant.
const WHOLE_PATH_CLASS: &str = "impl PathClass { pub const ALL: [Self; 4] = [ Self::FirstParty, Self::Vendored, Self::Generated, Self::Example, ]; #[must_use] pub const fn as_str(self) -> &'static str { match self { Self::FirstParty => \"FIRST_PARTY\", Self::Vendored => \"VENDORED\", Self::Generated => \"GENERATED\", Self::Example => \"EXAMPLE\", } } #[must_use] pub const fn promotes(self) -> bool { match self { Self::FirstParty => true, Self::Vendored | Self::Generated | Self::Example => false, } } }";

/// The finding's whole surface.
///
/// One crate-private constructor and accessors that hand back owned data. A
/// public constructor, a setter, or a `confidence` that stopped being absent
/// for the two tiers that have no number edits this constant.
const WHOLE_FINDING: &str = "impl Finding { #[expect( clippy::too_many_arguments, reason = \"every field of section 17.4's finding is required at construction; a builder \\ would reintroduce the partially-built value this type exists to refuse\" )] pub(crate) const fn seal( snapshot_id: String, subject: String, scope: FindingScope, tier: TierEvidence, rung: LadderRung, artifact_scope: ArtifactScope, strength: EvidenceStrength, locators: Vec<Locator>, excluded: Vec<ExcludedSite>, coverage: ComponentCoverage, ) -> Self { Self { snapshot_id, subject, scope, tier, rung, artifact_scope, strength, locators, excluded, coverage, } } #[must_use] pub fn snapshot_id(&self) -> &str { &self.snapshot_id } #[must_use] pub fn subject(&self) -> &str { &self.subject } #[must_use] pub const fn scope(&self) -> &FindingScope { &self.scope } #[must_use] pub const fn tier(&self) -> EvidenceTier { self.tier.tier() } #[must_use] pub const fn rung(&self) -> LadderRung { self.rung } #[must_use] pub const fn artifact_scope(&self) -> ArtifactScope { self.artifact_scope } #[must_use] pub const fn strength(&self) -> EvidenceStrength { self.strength } #[must_use] pub const fn confidence(&self) -> Option<&DisplayedConfidence> { match &self.tier { TierEvidence::Observed(confidence) => Some(confidence), TierEvidence::PresentOnly | TierEvidence::Possible => None, } } #[must_use] pub fn locators(&self) -> &[Locator] { &self.locators } #[must_use] pub fn excluded_sites(&self) -> &[ExcludedSite] { &self.excluded } #[must_use] pub const fn coverage(&self) -> ComponentCoverage { self.coverage } }";

/// Every `use` item this crate's product code spells, as a whole set.
///
/// Compared in both directions. A filesystem, process or transport import
/// appears here as an extra key whatever it is named, which is what makes "this
/// crate opens nothing" a statement about the crate rather than about the four
/// spellings somebody thought of. `std::collections` and `core::fmt` are the
/// only standard-library edges; the four workspace edges are the boundaries
/// this crate reuses rather than rebuilds.
const USE_ITEMS: [(&str, &str); 20] = [
    (
        "crates/repository-analysis/src/extract.rs",
        "use crate::index::{FileKind, SourceSpan, SymbolFingerprint, SymbolKind};",
    ),
    (
        "crates/repository-analysis/src/finding.rs",
        "use academic_model_run::DisplayedConfidence;",
    ),
    (
        "crates/repository-analysis/src/finding.rs",
        "use academic_policy::ContentDigest;",
    ),
    (
        "crates/repository-analysis/src/finding.rs",
        "use crate::{ index::{SourceSpan, SymbolFingerprint, SymbolKind}, paths::{ArtifactScope, ComponentId, PathClass}, };",
    ),
    (
        "crates/repository-analysis/src/index.rs",
        "use academic_policy::ContentDigest;",
    ),
    (
        "crates/repository-analysis/src/index.rs",
        "use crate::paths::{ArtifactScope, PathClass};",
    ),
    (
        "crates/repository-analysis/src/ladder.rs",
        "use std::collections::BTreeMap;",
    ),
    (
        "crates/repository-analysis/src/ladder.rs",
        "use academic_model_run::{ CalibrationRegistry, DisplayedConfidence, ModelVersion, ProviderId, Purpose, RawScore, };",
    ),
    (
        "crates/repository-analysis/src/ladder.rs",
        "use crate::{ AnalysisError, RepositoryAnalysis, finding::{ ComponentCoverage, EvidenceStrength, ExcludedSite, ExclusionReason, Finding, FindingScope, LadderRung, Locator, TierEvidence, }, index::{SourceSpan, SymbolFingerprint}, paths::{ArtifactScope, ComponentId, PackageId}, };",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "use std::collections::{BTreeMap, BTreeSet};",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "use academic_model_run::{ModelVersion, ProviderId};",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "use academic_policy::ContentDigest;",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "use academic_repository::{ManifestEntry, RepositorySnapshot};",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "use academic_untrusted_content::SourceIndex;",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "pub use finding::{ ComponentCoverage, EvidenceStrength, EvidenceTier, ExcludedSite, ExclusionReason, Finding, FindingScope, LadderRung, Locator, };",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "pub use index::{ CoverageGapReason, CoverageOutcome, FileKind, IndexKind, PathCoverage, SourceSpan, Support, SymbolFingerprint, SymbolKind, support, };",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "pub use ladder::{AnalyzerIdentity, EvidenceLadder, RuntimeTrace, Subject, SubjectId};",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "pub use paths::{ ArtifactScope, ComponentError, ComponentId, PackageId, PackageMap, PathClass, PathClassification, };",
    ),
    (
        "crates/repository-analysis/src/lib.rs",
        "use extract::{CallSite, Declaration, DependencySite, FileFacts, TokenSite};",
    ),
    (
        "crates/repository-analysis/src/paths.rs",
        "use std::collections::BTreeSet;",
    ),
];

/// Every public function of this crate whose return type names text, and why
/// each one is allowed to.
///
/// This is the half of the untrusted-content boundary that lives one step
/// outside `no_public_signature_hands_out_ingested_text`. That scan refuses a
/// `pub fn` that *takes* an `Untrusted<…>` and returns `str`, `String` or
/// `u8`; this crate takes no `Untrusted<…>` at all, so nothing there covers it,
/// and a symbol name lifted out of a repository and handed back as a `&str`
/// would be exactly the leak that scan exists to stop.
///
/// So the whole set is pinned, in both directions, with the reason each entry
/// is not analyzed content. There are only three reasons and they are the
/// column below: a fixed spelling of a closed vocabulary, a path — which
/// `academic-repository`'s own manifest already hands out and which the gate
/// classified before anything opened a file — or an identifier the caller
/// supplied. `no_analyzed_byte_reaches_a_text_accessor` in `evidence_tiers.rs`
/// is the executed half: it runs the analyzer over a corpus whose every
/// identifier is a canary and requires the canary in none of these outputs.
const TEXT_ACCESSORS: [(&str, &str, &str); 14] = [
    (
        "crates/repository-analysis/src/finding.rs",
        "as_str",
        "closed vocabulary",
    ),
    ("crates/repository-analysis/src/finding.rs", "path", "path"),
    (
        "crates/repository-analysis/src/finding.rs",
        "snapshot_id",
        "system-derived identifier",
    ),
    (
        "crates/repository-analysis/src/finding.rs",
        "subject",
        "caller-supplied identifier",
    ),
    (
        "crates/repository-analysis/src/index.rs",
        "as_str",
        "closed vocabulary",
    ),
    ("crates/repository-analysis/src/index.rs", "path", "path"),
    (
        "crates/repository-analysis/src/ladder.rs",
        "as_str",
        "caller-supplied identifier",
    ),
    (
        "crates/repository-analysis/src/ladder.rs",
        "snapshot_id",
        "system-derived identifier",
    ),
    (
        "crates/repository-analysis/src/ladder.rs",
        "tool",
        "caller-supplied identifier",
    ),
    (
        "crates/repository-analysis/src/ladder.rs",
        "version",
        "caller-supplied identifier",
    ),
    ("crates/repository-analysis/src/lib.rs", "gaps", "path"),
    ("crates/repository-analysis/src/lib.rs", "path", "path"),
    (
        "crates/repository-analysis/src/lib.rs",
        "snapshot_id",
        "system-derived identifier",
    ),
    (
        "crates/repository-analysis/src/paths.rs",
        "as_str",
        "closed vocabulary",
    ),
];

/// Every path this crate's product code spells through a crate root, as a
/// whole set.
///
/// This is the repair for a hole an injection found. The forbidden-token pass
/// below is a blocklist, and a blocklist is pierced by a name that is not on
/// it: `std::path::Path::new(p).metadata()` opens the filesystem,
/// `include_str!` reads a file at compile time, and `std::env::var` reads the
/// environment — and all three spell none of the eleven constructs, add no
/// `use` item, and were each observed **passing** this file's guard before this
/// inventory existed.
///
/// So the primary net is an allowlist and not a blocklist. Every two-segment
/// path whose first segment is a crate root — `std`, `core`, `alloc`,
/// `thiserror`, or a workspace crate — is compared against this list in both
/// directions, over the product code with `use` items removed. A capability
/// reached by writing its absolute path appears here as an extra key whatever
/// it is called, which is what the token list could not do.
///
/// It is deliberately two segments and not one: `std::path` and `std::fs` are
/// different capabilities and collapsing them to `std` would admit both.
const REACHED_PATHS: [(&str, &str); 4] = [
    (
        "academic_repository::sealed_documents",
        "P2-G5's sealed index, read through P2-R1's accessor so this crate names \
         no Untrusted type at all",
    ),
    ("core::fmt", "the hand-written Debug impls"),
    ("core::str", "from_utf8 on bytes the caller handed in"),
    ("thiserror::Error", "the error enumeration's derive"),
];

/// Every macro this crate's product code invokes, as a whole set.
///
/// A macro is not a path, so `REACHED_PATHS` cannot see one. `include_str!` and
/// `include_bytes!` read a file at compile time while spelling no `std` path
/// and needing no `use`; the first was observed passing this file's guard
/// before this inventory existed. Compared in both directions, so a macro
/// nobody predicted appears as an extra key.
const MACROS_SPELLED: [(&str, &str); 4] = [
    ("format", "building an owned identifier for a lookup"),
    (
        "format_args",
        "the hand-written Debug impls, which allocate nothing",
    ),
    ("matches", "total matches over closed enumerations"),
    ("vec", "building the fact lists"),
];

/// The constructs the forbidden-token pass refuses anywhere in the package.
///
/// Assembled from halves rather than written whole, and that is not obfuscation:
/// two other scans in this repository read raw source for these exact
/// spellings. One of them refuses a subprocess construct outside its reviewed
/// list and the other reads transport spellings, and neither can tell a needle
/// in a forbidden-token list from a call. A file that spelled them whole would have
/// to be added to a *reviewed sites* list as a file that spawns a subprocess,
/// which it does not — a false row in somebody else's contract, bought to make
/// this one readable. The `concat!` is evaluated at compile time, so the value
/// compared against the source is the whole spelling either way.
///
/// This pass is the **third** net and the weakest of the three, kept because it
/// names the shapes a reader most expects to see refused. [`USE_ITEMS`] is the
/// whole set of imports, [`REACHED_PATHS`] and [`MACROS_SPELLED`] are the whole
/// sets of everything reached without one, and those three together are what
/// makes "this crate opens nothing" a statement about the crate rather than
/// about eleven spellings. Each was injected separately and each was observed
/// refusing what the others admit.
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

/// Each guarded name, its call count over the package, the one file it may be
/// called from, and what a different count would mean.
const CALL_SITE_COUNTS: [(&str, usize, &str, &str); 4] = [
    (
        // `Finding::seal` is crate-private and this is the whole claim behind
        // `new_finding_cannot_default_to_repository_scope`: one producer, in
        // the ladder, which derives the scope from the evidence. A second
        // caller anywhere — including one that took a scope as an argument —
        // fails here rather than passing every pin on the ladder's own body.
        "seal",
        1,
        "crates/repository-analysis/src/ladder.rs",
        "a finding is constructed somewhere other than the ladder",
    ),
    (
        // The one route to a displayable number. `P2-M1`'s contract is that
        // `CalibrationRegistry::interpret` is the only producer of a
        // `CalibratedConfidence`; this is what says this crate reaches it from
        // one place, so a second path could not skip the freshness check.
        "interpret",
        1,
        "crates/repository-analysis/src/ladder.rs",
        "a confidence is calibrated from more than one place",
    ),
    (
        // The one place the promotion axis is applied. A site whose class was
        // read somewhere else would be a second policy for vendored, generated
        // and example paths.
        "promotes",
        1,
        "crates/repository-analysis/src/ladder.rs",
        "the promotion class is consulted from more than one place",
    ),
    (
        // Two: the gap branch and the analyzed branch of `analyze`. Both are in
        // `lib.rs`, and a third anywhere would be a coverage row built outside
        // the loop that walks the whole manifest.
        "build",
        2,
        "crates/repository-analysis/src/lib.rs",
        "a coverage row is built outside the manifest walk",
    ),
];

// ---------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 7,
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

    // The tripwire.
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
        }
        for line in source.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("#[path") else {
                continue;
            };
            let spelling = rest
                .split('"')
                .nth(1)
                .ok_or("a #[path] attribute names no file")?;
            declared += 1;
            let resolved = path
                .parent()
                .map(|parent| parent.join(spelling))
                .ok_or("a #[path] has no parent directory")?;
            assert!(
                sources.iter().any(|read| read == &resolved),
                "{} pulls in {spelling}, which the walk never read",
                relative(path)
            );
        }
    }
    assert!(
        declared >= 5,
        "the tripwire read only {declared} module declarations"
    );
    Ok(())
}

#[test]
fn the_analysis_crate_touches_no_file_and_no_socket() -> TestResult {
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
    let expected: Vec<(String, String)> = USE_ITEMS
        .iter()
        .map(|(file, item)| ((*file).to_owned(), (*item).to_owned()))
        .collect();
    assert_eq!(
        found, expected,
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
    // — tests included, because a test that opened a file would make the crate
    // documentation's claim about the whole package false.
    for path in crate_all_sources()? {
        let code = strip_non_code(&source_of(&path)?);
        for forbidden in FORBIDDEN_CONSTRUCTS {
            // A needle ending in `::` is a path prefix and is matched as a
            // substring; every other one is an identifier and is matched whole,
            // so this file's own test name -- which ends in `no_socket` -- is
            // not read as a socket. `the_helpers_are_not_vacuous` observes that
            // distinction rather than leaving it to inspection.
            let named = if forbidden.ends_with("::") {
                code.contains(forbidden)
            } else {
                uses_of(&code, forbidden) > 0
            };
            // This file reads files: it is a source scan, and `fs::read_dir` is
            // how it reaches them. Nothing else in the package is allowed one.
            let permitted =
                relative(&path).ends_with("tests/analysis_scans.rs") && forbidden == "fs::";
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
fn the_ladder_and_the_path_classification_are_pinned() -> TestResult {
    let ladder = source_of(&crate_root().join("src/ladder.rs"))?;
    let index = source_of(&crate_root().join("src/index.rs"))?;
    let paths = source_of(&crate_root().join("src/paths.rs"))?;
    let finding = source_of(&crate_root().join("src/finding.rs"))?;

    assert_eq!(
        whole_block(&ladder, "impl EvidenceLadder {")?,
        WHOLE_CLASSIFY,
        "the one producer of a finding changed"
    );
    assert_eq!(
        free_function(&ladder, "fn seal_finding(")?,
        WHOLE_SEAL_FINDING,
        "the rung decision changed"
    );
    assert_eq!(
        free_function(&ladder, "fn calibrated_confidence(")?,
        WHOLE_CALIBRATED_CONFIDENCE,
        "the route to a displayable confidence changed"
    );
    assert_eq!(
        free_function(&index, "pub const fn support(")?,
        WHOLE_SUPPORT,
        "the coverage support matrix changed"
    );
    assert_eq!(
        whole_block(&paths, "impl ComponentId {")?,
        WHOLE_COMPONENT_ID,
        "the refusal of the repository root changed"
    );
    assert_eq!(
        whole_block(&paths, "impl PathClass {")?,
        WHOLE_PATH_CLASS,
        "the promotion axis changed"
    );
    assert_eq!(
        whole_block(&finding, "impl LadderRung {")?,
        WHOLE_LADDER_RUNG,
        "the fold from five rungs onto three tiers changed"
    );
    assert_eq!(
        whole_block(&finding, "impl Finding {")?,
        WHOLE_FINDING,
        "the finding's whole surface changed"
    );
    Ok(())
}

#[test]
fn each_guarded_name_has_exactly_its_call_sites() -> TestResult {
    for (name, expected, owner, consequence) in CALL_SITE_COUNTS {
        let mut total = 0;
        for (file, code) in product_code()? {
            let count = calls_of(&without_use_items(&code), name);
            if count > 0 {
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
fn no_public_accessor_hands_out_analyzed_text() -> TestResult {
    let mut found: Vec<(String, String)> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            let Some((_, returns)) = signature.split_once("->") else {
                continue;
            };
            if names_text(returns) {
                found.push((file.clone(), name));
            }
        }
    }
    found.sort();
    found.dedup();
    let expected: Vec<(String, String)> = {
        let mut listed: Vec<(String, String)> = TEXT_ACCESSORS
            .iter()
            .map(|(file, name, _)| ((*file).to_owned(), (*name).to_owned()))
            .collect();
        listed.sort();
        listed.dedup();
        listed
    };
    assert_eq!(
        found, expected,
        "a public function of this crate returns text that is not on the justified inventory"
    );

    // The reasons are a closed list, so a new entry has to pick one rather than
    // write a sentence that explains a leak.
    let reasons: BTreeSet<&str> = TEXT_ACCESSORS.iter().map(|(_, _, why)| *why).collect();
    assert_eq!(
        reasons,
        BTreeSet::from([
            "caller-supplied identifier",
            "closed vocabulary",
            "path",
            "system-derived identifier",
        ])
    );
    Ok(())
}

/// `code` with the whitespace that sits inside a path or a macro call removed.
///
/// Rust allows whitespace inside a path and between a macro's `!` and its
/// delimiter, and both were measured slipping past the two extractors below:
/// `std :: path :: Path::new(p).metadata()` opens the filesystem and
/// `include_str! ("x")` reads a file, and each compiled and passed.
///
/// It closes exactly those two gaps and nothing wider. Deleting **all**
/// whitespace was tried first and is wrong in the one direction that matters:
/// it joins unrelated tokens, and `… Formatter and core::str …` becomes
/// `…Formatterandcore::str…`, where `core` is no longer a whole identifier and
/// the key **disappears**. A transform that can hide a key is worse than the
/// hole it closes. `the_helpers_are_not_vacuous` carries that case.
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
        // The run is inside a path or a macro call exactly when a `::` or a `!`
        // ends what came before it, or a `::` or a `!` starts what follows.
        // `foo ! (x)` and `foo! (x)` are both macro calls, so both sides of the
        // `!` are tightened; `a != b` and `if !flag` survive it, because the
        // extractor still requires a delimiter immediately after the `!`.
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
///
/// The first segment has to be a crate root this package can name, so a field
/// access such as `self.path` is not a path and `Self::Variant` is not one
/// either. What it catches is the absolute form — `std::env::var`,
/// `std::path::Path` — which is the shape that needs no `use` item.
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
        // A whole identifier: the byte before it cannot continue one, which is
        // what stops `a::b::c` being read as a second root at `b`.
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
        // A macro name is an identifier, and a keyword is not one — `if !(x)`
        // tightens to `if!(x)` and would otherwise read as a macro called `if`.
        // Excluding keywords cannot hide a real macro, because none of these is
        // a name a macro may have.
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

/// Whether a return type names text, by whole identifier.
///
/// Whole identifiers, so `&'static str` is caught along with `&str`, and
/// `Vec<u8>`, `Box<[u8]>` and `Cow<'_, [u8]>` along with `&[u8]`. It is the
/// same rule `no_public_signature_hands_out_ingested_text` applies one step
/// outside this crate.
fn names_text(returns: &str) -> bool {
    ["str", "String", "u8"]
        .iter()
        .any(|name| uses_of(returns, name) > 0)
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for named in [
        "crates/repository-analysis/tests/analysis_scans.rs",
        "crates/repository-analysis/tests/evidence_tiers.rs",
    ] {
        assert!(
            page.contains(named),
            "{named} is not named in docs/contracts/policy-source-scans.md"
        );
    }
    let mut missing = Vec::new();
    for (name, _, _, _) in CALL_SITE_COUNTS {
        if !page.contains(name) {
            missing.push(name);
        }
    }
    assert_eq!(
        missing,
        Vec::<&str>::new(),
        "a counted call site is not described on the inventory page"
    );
    Ok(())
}

#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(declarations_of("fn seal_finding(", "seal"), 0);
    assert_eq!(declarations_of("fn seal(x)", "seal"), 1);
    assert_eq!(calls_of("fn seal(){} Finding::seal(a);", "seal"), 1);
    assert_eq!(uses_of("resealed sealed seal", "seal"), 1);
    assert_eq!(
        without_use_items("use a::b;\nlet x = b();\n").trim(),
        "let x = b();"
    );
    assert_eq!(collapse("// gone\n  a   b\n"), "a b");
    assert!(names_text("-> &'static str"));
    assert!(names_text("-> Vec<u8>"));
    assert!(names_text("-> Cow<'_, [u8]>"));
    assert!(!names_text("-> ArtifactScope"));
    assert!(!names_text("-> Option<&SymbolFingerprint>"));
    // The stripper is what makes the forbidden-token pass a statement about
    // code: a `fs::` inside a string literal or a comment is prose about the
    // rule, and this file writes both.
    assert_eq!(
        strip_non_code("let a = \"fs::read\"; // fs::read\n"),
        "let a =  ; \n\n"
    );
    // The whole-identifier half of the forbidden-token pass: this file's own
    // test name ends in `no_socket`, and reading that as a socket would make the
    // pass fire on its own description.
    assert_eq!(uses_of("fn a_name_with_no_socket() {}", "socket"), 0);
    assert_eq!(uses_of("Stream::connect(x)", "connect"), 1);
    // The two whole-set extractors. Each case is a shape an injection used.
    assert_eq!(
        absolute_paths("let _ = std::path::Path::new(p).metadata();"),
        BTreeSet::from(["std::path".to_owned()])
    );
    assert_eq!(
        absolute_paths("let _ = std::env::var(k);"),
        BTreeSet::from(["std::env".to_owned()])
    );
    // Two segments, not one: the root alone would admit every capability under
    // it, and the second segment is what separates `std::fs` from `std::path`.
    assert_eq!(
        absolute_paths("core::fmt::Formatter and core::str::from_utf8"),
        BTreeSet::from(["core::fmt".to_owned(), "core::str".to_owned()])
    );
    // A field access and an associated path are not crate-root paths.
    assert_eq!(
        absolute_paths("Self::Variant and self.field"),
        BTreeSet::new()
    );
    // A middle segment yields no second key, and a leading `::` is not a middle
    // segment. The second case was a measured bypass of this function.
    assert_eq!(
        absolute_paths("std::collections::BTreeMap"),
        BTreeSet::from(["std::collections".to_owned()])
    );
    assert_eq!(
        absolute_paths("let _ = ::std::path::Path::new(p);"),
        BTreeSet::from(["std::path".to_owned()])
    );
    // Whitespace inside a path and between a macro's `!` and its delimiter.
    // Both compiled and both passed before `without_whitespace` existed.
    assert_eq!(
        absolute_paths("let _ = std :: path :: Path::new(p);"),
        BTreeSet::from(["std::path".to_owned()])
    );
    assert_eq!(
        macros_spelled("include_str! (\"x\")"),
        BTreeSet::from(["include_str".to_owned()])
    );
    assert_eq!(tighten("a :: b !\n("), "a::b!(");
    // The case that rules out deleting all whitespace: joining `and` onto
    // `core` would stop `core` being a whole identifier and the key would
    // vanish, which is the one direction a normalisation must not fail in.
    assert_eq!(
        tighten("Formatter and core::str::from_utf8"),
        "Formatter and core::str::from_utf8"
    );
    assert_eq!(
        macros_spelled("return format!(x); if !flag { }"),
        BTreeSet::from(["format".to_owned()])
    );
    assert_eq!(
        macros_spelled("let s = include_str!(\"x\"); format!(\"y\");"),
        BTreeSet::from(["include_str".to_owned(), "format".to_owned()])
    );
    assert_eq!(macros_spelled("if a != b { }"), BTreeSet::new());
    assert_eq!(macros_spelled("if !(a || b) { }"), BTreeSet::new());
    assert_eq!(REACHED_PATHS.len(), 4);
    assert_eq!(MACROS_SPELLED.len(), 4);
    assert_eq!(FORBIDDEN_CONSTRUCTS.len(), 11);
    assert!(
        FORBIDDEN_CONSTRUCTS
            .iter()
            .any(|item| item.ends_with("::Command"))
    );
    let names: HashSet<&str> = CALL_SITE_COUNTS.iter().map(|(name, ..)| *name).collect();
    assert_eq!(names.len(), CALL_SITE_COUNTS.len());
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
const IMPL_HEADERS: [&str; 37] = [
    "impl AnalyzedFile",
    "impl AnalyzerIdentity",
    "impl ArtifactScope",
    "impl ComponentCoverage",
    "impl ComponentId",
    "impl CoverageGapReason",
    "impl CoverageOutcome",
    "impl EvidenceLadder",
    "impl EvidenceStrength",
    "impl EvidenceTier",
    "impl ExcludedSite",
    "impl ExclusionReason",
    "impl FileKind",
    "impl Finding",
    "impl FindingScope",
    "impl IndexKind",
    "impl LadderRung",
    "impl Locator",
    "impl PackageId",
    "impl PackageMap",
    "impl PathClass",
    "impl PathClassification",
    "impl PathCoverage",
    "impl RepositoryAnalysis",
    "impl RuntimeTrace",
    "impl Site",
    "impl SourceSpan",
    "impl SourceUnit",
    "impl Subject",
    "impl SubjectId",
    "impl SymbolFingerprint",
    "impl SymbolKind",
    "impl SymbolRecord",
    "impl TierEvidence",
    "impl core::fmt::Debug for AnalyzedFile",
    "impl core::fmt::Debug for SourceUnit",
    "impl<'a> AnalysisInput<'a>",
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
