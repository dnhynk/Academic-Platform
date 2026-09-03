//! `P2-R2`: static analysis over a frozen repository snapshot, and the evidence
//! ladder that decides what may be claimed from it.
//!
//! `P2-R1` owns the gate, the inventory and the frozen [`RepositorySnapshot`];
//! its `analyze` is the seam at which the read-only argument type is fixed.
//! This crate is what the seam was fixed for: section 17.3's third stage — AST,
//! symbols, call and data flow, schema, config and IaC indexing — and section
//! 17.3's own tier table over the result.
//!
//! ## What holds "the analyzer cannot mutate the source or open a socket"
//!
//! Not this crate, and this crate does not restate it. `P2-R1`'s
//! `analyzer_cannot_mutate_source_or_open_a_socket` mints
//! `ProcessClass::RepositoryAnalyzer`'s three capabilities through `P2-G1`'s
//! broker and observes `OpenOutboundSocket`, `WriteStagedArtifact` and
//! `ReadKeyMaterial` refused; `P2-G4` measured what a kernel refuses a
//! sandboxed process. What this crate adds is that it needs neither: it spells
//! no `std::fs` name, opens nothing, and takes its bytes as an argument.
//! `the_analysis_crate_touches_no_file_and_no_socket` compares the whole set of
//! its `use` items against a pinned inventory, so a filesystem or transport
//! import appears as an extra key rather than as a token nobody listed.
//!
//! ## Everything analyzed is untrusted, and stays that way
//!
//! `Untrusted::seal` is private to `academic-untrusted-content`, so this crate
//! cannot label a value and therefore must not hold one that needs a label.
//! What it holds instead is digests, spans, and closed vocabularies:
//!
//! * a declaration is a [`SymbolFingerprint`], which is section 17.4's own word
//!   — *blob hash, symbol fingerprint, syntax span* — rather than a name;
//! * a dependency, import or configuration token read out of a file is compared
//!   against a needle the caller supplied and then dropped; what a
//!   [`Finding`] carries is the caller's [`SubjectId`];
//! * a path is text, because `P2-R1`'s own manifest already hands paths out and
//!   the gate classified every one of them before anything opened a file.
//!
//! [`AnalysisInput::of`] additionally refuses any unit whose bytes are not
//! already sealed in `P2-G5`'s `SourceIndex`, so the analyzer reads exactly the
//! bytes that were ingested as untrusted and nothing beside them.
//!
//! ## The coverage report is total
//!
//! Every path in the snapshot's manifest gets one [`PathCoverage`], and every
//! coverage holds one outcome per [`IndexKind`]. A file this analyzer has no
//! reader for produces [`CoverageOutcome::Gap`] for all seven, which is
//! `REQ-17-011`'s *unsupported kind explicitly reports coverage gap*. There is
//! no path through [`analyze`] that skips a file quietly, because there is no
//! partially-filled coverage value to skip it into.

pub mod extract;
pub mod finding;
pub mod index;
pub mod ladder;
pub mod paths;

use std::collections::{BTreeMap, BTreeSet};

use academic_model_run::{ModelVersion, ProviderId};
use academic_policy::ContentDigest;
use academic_repository::{ManifestEntry, RepositorySnapshot};
use academic_untrusted_content::SourceIndex;

pub use finding::{
    ComponentCoverage, EvidenceStrength, EvidenceTier, ExcludedSite, ExclusionReason, Finding,
    FindingScope, LadderRung, Locator,
};
pub use index::{
    CoverageGapReason, CoverageOutcome, FileKind, IndexKind, PathCoverage, SourceSpan, Support,
    SymbolFingerprint, SymbolKind, support,
};
pub use ladder::{AnalyzerIdentity, EvidenceLadder, RuntimeTrace, Subject, SubjectId};
pub use paths::{
    ArtifactScope, ComponentError, ComponentId, PackageId, PackageMap, PathClass,
    PathClassification,
};

use extract::{CallSite, Declaration, DependencySite, FileFacts, TokenSite};

/// Why an analysis or a classification was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AnalysisError {
    /// A unit named a path the frozen manifest does not hold.
    #[error("the analyzed path {0} is not in the snapshot's manifest")]
    PathNotInSnapshot(String),
    /// A unit's bytes are not the bytes the snapshot froze.
    #[error("the bytes offered for {0} are not the bytes the snapshot froze")]
    BytesDoNotMatchSnapshot(String),
    /// A unit's bytes were never ingested through `P2-G5`'s boundary.
    #[error("the bytes offered for {0} are not sealed in the untrusted-content index")]
    BytesNotSealed(String),
    /// Two units named the same path.
    #[error("the path {0} was offered twice")]
    DuplicatePath(String),
    /// The snapshot's `toolVersions` does not name this analyzer.
    #[error("the snapshot does not record the analyzer {0} at version {1}")]
    AnalyzerNotInSnapshot(String, String),
    /// An analyzer identity half was empty.
    #[error("the analyzer identity {0:?} is empty")]
    InvalidAnalyzerIdentity(String),
    /// A subject identifier was empty, too long, or held a forbidden byte.
    #[error("the subject identifier {0:?} is not [A-Za-z0-9._-] within 64 bytes")]
    InvalidSubject(String),
    /// Nothing outside a vendored, generated or example path named the subject.
    #[error("no promoting evidence names the subject {0}")]
    NoPromotingEvidence(String),
    /// A rung that section 17.3 requires a confidence for had none to show.
    #[error("the confidence could not be calibrated: {0}")]
    UncalibratedConfidence(String),
    /// A path in the snapshot's manifest is not a relative forward-slashed one.
    #[error("the manifest path {0} cannot be classified")]
    MalformedPath(String),
}

/// One file's bytes, offered to the analyzer by whoever read them.
///
/// The analyzer opens nothing. A unit is how the bytes the gate admitted reach
/// it, and [`AnalysisInput::of`] is where a unit is checked against the frozen
/// manifest and against `P2-G5`'s sealed index before anything reads it.
///
/// The byte field is named `source_bytes` for the reason
/// `academic-repository`'s own entry type names its field that:
/// `tools/secret-debug-policy.test.mjs` holds that name, so the derived `Debug`
/// this struct would otherwise have is refused by the existing net rather than
/// by a rule this crate invented.
pub struct SourceUnit {
    path: String,
    source_bytes: Vec<u8>,
}

impl core::fmt::Debug for SourceUnit {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SourceUnit")
            .field("path", &self.path)
            .field(
                "source_bytes",
                &format_args!("<untrusted:{} bytes>", self.source_bytes.len()),
            )
            .finish()
    }
}

impl SourceUnit {
    /// Names one file and the bytes that were read for it.
    #[must_use]
    pub fn new(path: impl Into<String>, source_bytes: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            source_bytes,
        }
    }

    /// The relative, forward-slashed path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// How many bytes the unit holds.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.source_bytes.len()
    }
}

/// A validated analysis request: one frozen snapshot and the bytes for it.
///
/// The one constructor checks four things, and each of them is a way an
/// analysis could otherwise be about something other than the snapshot it
/// names:
///
/// 1. the snapshot's `toolVersions` records this analyzer at this version, so
///    the confidence a finding shows is calibrated for the build the snapshot
///    says produced it;
/// 2. every unit's path is a manifest row, so the analyzer cannot be handed a
///    path the gate excluded;
/// 3. every unit's bytes hash to that row's `blobHash`, so it cannot be handed
///    different bytes for an admitted path; and
/// 4. every unit's bytes are sealed in `P2-G5`'s index, so it reads only what
///    was ingested as untrusted content.
#[derive(Debug)]
pub struct AnalysisInput<'a> {
    snapshot: &'a RepositorySnapshot,
    identity: AnalyzerIdentity,
    units: Vec<SourceUnit>,
}

impl<'a> AnalysisInput<'a> {
    /// Validates a request.
    ///
    /// # Errors
    ///
    /// One of [`AnalysisError::AnalyzerNotInSnapshot`],
    /// [`AnalysisError::PathNotInSnapshot`],
    /// [`AnalysisError::BytesDoNotMatchSnapshot`],
    /// [`AnalysisError::BytesNotSealed`] or [`AnalysisError::DuplicatePath`].
    pub fn of(
        snapshot: &'a RepositorySnapshot,
        sealed: &SourceIndex,
        identity: AnalyzerIdentity,
        units: Vec<SourceUnit>,
    ) -> Result<Self, AnalysisError> {
        let recorded = snapshot
            .tool_versions()
            .iter()
            .any(|tool| tool.tool() == identity.tool() && tool.version() == identity.version());
        if !recorded {
            return Err(AnalysisError::AnalyzerNotInSnapshot(
                identity.tool().to_owned(),
                identity.version().to_owned(),
            ));
        }
        let manifest: BTreeMap<&str, &ManifestEntry> = snapshot
            .manifest()
            .iter()
            .map(|entry| (entry.path(), entry))
            .collect();
        let sealed_digests: BTreeSet<&str> = academic_repository::sealed_documents(sealed)
            .iter()
            .map(|document| document.digest())
            .collect();
        let mut seen = BTreeSet::new();
        for unit in &units {
            if !seen.insert(unit.path.clone()) {
                return Err(AnalysisError::DuplicatePath(unit.path.clone()));
            }
            let Some(entry) = manifest.get(unit.path.as_str()) else {
                return Err(AnalysisError::PathNotInSnapshot(unit.path.clone()));
            };
            let digest = ContentDigest::of(&unit.source_bytes);
            if &digest != entry.blob_digest() {
                return Err(AnalysisError::BytesDoNotMatchSnapshot(unit.path.clone()));
            }
            if !sealed_digests.contains(digest.as_str()) {
                return Err(AnalysisError::BytesNotSealed(unit.path.clone()));
            }
        }
        Ok(Self {
            snapshot,
            identity,
            units,
        })
    }
}

/// One analyzed file: its classification, its facts, and its reachability.
///
/// Crate-private accessors, and deliberately so. A dependency name, an import
/// specifier and a configuration key are untrusted bytes; the only thing this
/// crate does with one is compare it against a needle the caller supplied. A
/// public accessor returning one would be an unlabelled copy of repository
/// content, which is the shape `no_public_signature_hands_out_ingested_text`
/// refuses one step outside this crate and
/// `no_analyzed_byte_reaches_a_text_accessor` refuses inside it.
///
/// `Debug` is hand-written for the same reason `Untrusted<T>`'s is: an accessor
/// is not the only way a `String` reaches a log. A derived one here would print
/// every symbol name, import specifier and configuration key the analyzer read,
/// through the derived `Debug` of every public value that holds one.
#[derive(Clone)]
pub(crate) struct AnalyzedFile {
    path: String,
    blob_digest: ContentDigest,
    class: PathClass,
    scope: ArtifactScope,
    package: Option<PackageId>,
    facts: FileFacts,
    reachable: Vec<bool>,
}

impl core::fmt::Debug for AnalyzedFile {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AnalyzedFile")
            .field("path", &self.path)
            .field("class", &self.class)
            .field("scope", &self.scope)
            .field("package", &self.package)
            .field(
                "facts",
                &format_args!(
                    "<untrusted: {} declarations, {} calls, {} imports, {} config, {} dependencies>",
                    self.facts.declarations.len(),
                    self.facts.calls.len(),
                    self.facts.imports.len(),
                    self.facts.config_tokens.len(),
                    self.facts.dependencies.len()
                ),
            )
            .finish()
    }
}

impl AnalyzedFile {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn class(&self) -> PathClass {
        self.class
    }

    pub(crate) const fn scope(&self) -> ArtifactScope {
        self.scope
    }

    pub(crate) const fn package(&self) -> Option<&PackageId> {
        self.package.as_ref()
    }

    pub(crate) fn dependencies(&self) -> &[DependencySite] {
        &self.facts.dependencies
    }

    pub(crate) fn imports(&self) -> &[TokenSite] {
        &self.facts.imports
    }

    pub(crate) fn calls(&self) -> &[CallSite] {
        &self.facts.calls
    }

    pub(crate) fn config_tokens(&self) -> impl Iterator<Item = &TokenSite> {
        self.facts.config_tokens.iter()
    }

    pub(crate) fn iac_tokens(&self) -> impl Iterator<Item = &TokenSite> {
        self.facts.iac_tokens.iter()
    }

    /// The innermost declaration whose body contains `span`.
    pub(crate) fn enclosing(&self, span: SourceSpan) -> Option<&Declaration> {
        self.facts
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.span.start() <= span.start() && span.end() <= declaration.span.end()
            })
            .min_by_key(|declaration| declaration.span.end() - declaration.span.start())
    }

    /// Whether a call at `span` sits somewhere an entry point reaches.
    ///
    /// A call outside every declaration is module-level code, which runs when
    /// the module is loaded, so it is reachable. A call inside a declaration is
    /// reachable exactly when that declaration is.
    pub(crate) fn call_is_reachable(&self, span: SourceSpan) -> bool {
        self.enclosing(span).is_none_or(|declaration| {
            self.facts
                .declarations
                .iter()
                .position(|candidate| candidate.fingerprint == declaration.fingerprint)
                .is_some_and(|at| self.reachable.get(at).copied().unwrap_or(false))
        })
    }

    /// Builds a locator into this file.
    pub(crate) fn locator(
        &self,
        span: SourceSpan,
        enclosing: Option<&Declaration>,
        scope: ArtifactScope,
    ) -> Locator {
        Locator::new(
            self.path.clone(),
            enclosing.map(|declaration| declaration.fingerprint.clone()),
            enclosing.map(|declaration| declaration.kind),
            span,
            self.blob_digest.clone(),
            self.class,
            scope,
        )
    }
}

/// One declaration, as a reader outside this crate may see it.
///
/// The fingerprint and the kind, and no name. See the crate documentation for
/// why a name is not a value this crate may hand out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRecord {
    path: String,
    fingerprint: SymbolFingerprint,
    kind: SymbolKind,
    span: SourceSpan,
    reachable: bool,
}

impl SymbolRecord {
    /// Which file it is declared in.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Section 17.4's symbol fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> &SymbolFingerprint {
        &self.fingerprint
    }

    /// What kind of declaration it is.
    #[must_use]
    pub const fn kind(&self) -> SymbolKind {
        self.kind
    }

    /// Where it sits.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    /// Whether an entry point reaches it.
    #[must_use]
    pub const fn reachable(&self) -> bool {
        self.reachable
    }
}

/// What one run of the analyzer produced.
#[derive(Debug, Clone)]
pub struct RepositoryAnalysis {
    snapshot_id: String,
    provider: ProviderId,
    model_version: ModelVersion,
    packages: PackageMap,
    files: Vec<AnalyzedFile>,
    coverage: Vec<PathCoverage>,
    analyzed_components: u32,
}

impl RepositoryAnalysis {
    /// Which snapshot was analyzed.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// The analyzer, as `P2-M1` names a producer of scores.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// The analyzer version a calibration dataset is registered for.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// The packages the frozen manifest showed.
    #[must_use]
    pub const fn packages(&self) -> &PackageMap {
        &self.packages
    }

    /// One coverage row per manifest path, each total over [`IndexKind`].
    #[must_use]
    pub fn coverage(&self) -> &[PathCoverage] {
        &self.coverage
    }

    /// Every coverage gap, as `REQ-17-011` requires them to be reported.
    #[must_use]
    pub fn gaps(&self) -> Vec<(&str, IndexKind, CoverageGapReason)> {
        self.coverage
            .iter()
            .flat_map(|row| {
                row.gaps()
                    .into_iter()
                    .map(move |(kind, reason)| (row.path(), kind, reason))
            })
            .collect()
    }

    /// Every declaration, by fingerprint.
    #[must_use]
    pub fn symbols(&self) -> Vec<SymbolRecord> {
        let mut records = Vec::new();
        for file in &self.files {
            for (at, declaration) in file.facts.declarations.iter().enumerate() {
                records.push(SymbolRecord {
                    path: file.path.clone(),
                    fingerprint: declaration.fingerprint.clone(),
                    kind: declaration.kind,
                    span: declaration.span,
                    reachable: file.reachable.get(at).copied().unwrap_or(false),
                });
            }
            for object in &file.facts.schema_objects {
                records.push(SymbolRecord {
                    path: file.path.clone(),
                    fingerprint: object.fingerprint.clone(),
                    kind: object.kind,
                    span: object.span,
                    reachable: true,
                });
            }
        }
        records
    }

    /// How many components this run read at all: `REQ-34-093`'s denominator.
    #[must_use]
    pub const fn analyzed_component_count(&self) -> u32 {
        self.analyzed_components
    }

    /// The file kinds this analyzer has a reader for, as a coverage report
    /// prints them beside the gaps.
    #[must_use]
    pub fn supported_file_kinds() -> Vec<FileKind> {
        FileKind::ALL
            .into_iter()
            .filter(|&kind| {
                IndexKind::ALL
                    .iter()
                    .any(|&index| support(kind, index) != Support::Unsupported)
            })
            .collect()
    }

    pub(crate) fn files(&self) -> &[AnalyzedFile] {
        &self.files
    }
}

/// Section 17.3's third stage, over one validated request.
///
/// # Errors
///
/// [`AnalysisError::MalformedPath`] when a manifest path cannot be classified.
pub fn analyze(input: &AnalysisInput<'_>) -> Result<RepositoryAnalysis, AnalysisError> {
    let manifest = input.snapshot.manifest();
    let packages = PackageMap::of_paths(manifest.iter().map(ManifestEntry::path));
    let offered: BTreeMap<&str, &SourceUnit> = input
        .units
        .iter()
        .map(|unit| (unit.path.as_str(), unit))
        .collect();

    let mut files = Vec::new();
    let mut coverage = Vec::new();
    for entry in manifest {
        let path = entry.path();
        let classification = paths::classify_path(path, &packages);
        let file_kind = FileKind::of_path(path);
        let Some(unit) = offered.get(path) else {
            coverage.push(PathCoverage::build(
                path.to_owned(),
                file_kind,
                classification.class(),
                classification.scope(),
                |_| CoverageOutcome::Gap(CoverageGapReason::BytesNotIngested),
            ));
            continue;
        };
        let facts = extract::read(path, file_kind, &unit.source_bytes);
        coverage.push(PathCoverage::build(
            path.to_owned(),
            file_kind,
            classification.class(),
            classification.scope(),
            |kind| match support(file_kind, kind) {
                Support::Unsupported => {
                    CoverageOutcome::Gap(CoverageGapReason::UnsupportedLanguage)
                }
                Support::NotApplicable => CoverageOutcome::NotApplicable,
                Support::Analyzed => CoverageOutcome::Analyzed(fact_count(&facts, kind)),
            },
        ));
        let reachable = vec![false; facts.declarations.len()];
        files.push(AnalyzedFile {
            path: path.to_owned(),
            blob_digest: entry.blob_digest().clone(),
            class: classification.class(),
            scope: classification.scope(),
            package: classification.package().cloned(),
            facts,
            reachable,
        });
    }

    resolve_reachability(&mut files);

    let analyzed_components = u32::try_from(
        coverage
            .iter()
            .filter(|row| !row.gaps().len().eq(&IndexKind::COUNT))
            .filter_map(|row| ComponentId::containing(row.path()).ok())
            .collect::<BTreeSet<_>>()
            .len(),
    )
    .unwrap_or(u32::MAX);

    Ok(RepositoryAnalysis {
        snapshot_id: input.snapshot.snapshot_id().to_owned(),
        provider: input.identity.provider().clone(),
        model_version: input.identity.model_version().clone(),
        packages,
        files,
        coverage,
        analyzed_components,
    })
}

/// How many facts one index kind produced for one file.
fn fact_count(facts: &FileFacts, kind: IndexKind) -> u32 {
    let count = match kind {
        IndexKind::Ast => facts.declarations.len() + facts.config_tokens.len(),
        IndexKind::Symbol => facts.declarations.len(),
        IndexKind::CallFlow => facts.calls.len(),
        IndexKind::DataFlow => facts.data_flow.len(),
        IndexKind::Schema => facts.schema_objects.len(),
        IndexKind::Config => facts.config_tokens.len() + facts.dependencies.len(),
        IndexKind::Iac => facts.iac_tokens.len(),
    };
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Marks every declaration an entry point reaches.
///
/// Resolution is by name across the analyzed set: a call whose leaf identifier
/// equals a declaration's name reaches that declaration. That is exact for the
/// synthetic corpora this crate is tested against and an over-approximation in
/// general — two functions with one name in two files are both marked. It is an
/// over-approximation on purpose: under-approximating reachability would move a
/// live call to section 17.3's second row and report a use as merely possible,
/// which is the failure the ladder's other direction already guards.
fn resolve_reachability(files: &mut [AnalyzedFile]) {
    let mut names: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    for (file_at, file) in files.iter().enumerate() {
        for (declaration_at, declaration) in file.facts.declarations.iter().enumerate() {
            names
                .entry(declaration.name.clone())
                .or_default()
                .push((file_at, declaration_at));
        }
    }

    let mut pending: Vec<(usize, usize)> = Vec::new();
    for (file_at, file) in files.iter().enumerate() {
        for (declaration_at, declaration) in file.facts.declarations.iter().enumerate() {
            if declaration.is_root {
                pending.push((file_at, declaration_at));
            }
        }
    }
    // Module-level calls run when the file loads, so what they name is reached
    // without any declaration being reached first.
    let mut top_level: Vec<String> = Vec::new();
    for file in files.iter() {
        for call in &file.facts.calls {
            if file.enclosing(call.span).is_none() {
                top_level.push(call.leaf.clone());
            }
        }
    }
    for leaf in &top_level {
        if let Some(targets) = names.get(leaf) {
            pending.extend(targets.iter().copied());
        }
    }

    while let Some((file_at, declaration_at)) = pending.pop() {
        if files[file_at].reachable[declaration_at] {
            continue;
        }
        files[file_at].reachable[declaration_at] = true;
        let span = files[file_at].facts.declarations[declaration_at].span;
        let leaves: Vec<String> = files[file_at]
            .facts
            .calls
            .iter()
            .filter(|call| span.start() <= call.span.start() && call.span.end() <= span.end())
            .map(|call| call.leaf.clone())
            .collect();
        for leaf in leaves {
            if let Some(targets) = names.get(&leaf) {
                pending.extend(targets.iter().copied());
            }
        }
    }
}
