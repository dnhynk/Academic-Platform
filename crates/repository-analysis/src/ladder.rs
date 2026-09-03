//! Section 17.3's evidence ladder: five observations, three tier values.
//!
//! ## The fold is the contract
//!
//! Section 17.3's table has five rows and one column headed `OBSERVED 가능
//! 여부`. `REQ-34-081` names the three values a reader is shown. Five onto
//! three is a lossy fold, so *which row becomes which value* is the thing that
//! has to be written down and executed rather than inferred:
//!
//! | Section 17.3 row | That column | Tier |
//! |---|---|---|
//! | `manifest에 dependency만 있음` | `불가` | `PRESENT_ONLY` |
//! | `import만 있고 reachable use 없음` | `보류` | `POSSIBLE` |
//! | `reachable call + config 존재` | `가능, confidence 표시` | `OBSERVED` |
//! | `test에서만 사용` | `scope를 제한해 가능` | `OBSERVED`, scope `TEST` |
//! | `runtime trace/production config와 일치` | `가능` | `OBSERVED`, strong |
//!
//! The sixth row of that table — `사용자 직접 구현·debugging 확인` — is not
//! here. It is `User APPLIED Concept` rather than `ProjectSnapshot OBSERVES
//! Concept`, which section 17.6 separates and `P2-R5` owns.
//!
//! Rows three, four and five are all `OBSERVED`; what separates them is what
//! else the finding carries. Row four narrows [`ArtifactScope`] to `TEST`, and
//! row five raises [`EvidenceStrength`] to `STRONG`. Neither is a higher tier,
//! and treating row four as one would be exactly the over-claim
//! `test_only_use_is_test_scoped` is written against.
//!
//! ## The rungs are tried downwards, and the order is load-bearing
//!
//! [`classify`] tries row five, then four, then three, then two, then one.
//! Trying three before four would classify a subject used only in a test
//! harness — a reachable call plus a test configuration — as a production
//! observation, which is section 34.4's `test 도구를 운영 사용으로 오인`.
//!
//! ## What does not count
//!
//! A site in a vendored, generated or example path is recorded and never
//! counted, and a site in another package of a monorepo is recorded against
//! that package rather than this one. Both stay on the finding as
//! [`crate::ExcludedSite`]s with the reason, because a reader who is told
//! `PRESENT_ONLY` and can see the vendored copy needs to be told the analyzer
//! saw it too.

use std::collections::BTreeMap;

use academic_model_run::{
    CalibrationRegistry, DisplayedConfidence, ModelVersion, ProviderId, Purpose, RawScore,
};

use crate::{
    AnalysisError, RepositoryAnalysis,
    finding::{
        ComponentCoverage, EvidenceStrength, ExcludedSite, ExclusionReason, Finding, FindingScope,
        LadderRung, Locator, TierEvidence,
    },
    index::{SourceSpan, SymbolFingerprint},
    paths::{ArtifactScope, ComponentId, PackageId},
};

/// The caller's own identifier for what a finding is about.
///
/// It is the caller's, and that is the point. A dependency name, an import
/// specifier and a configuration key read out of a repository are untrusted
/// bytes; a subject identifier is a value this system chose. Matching is
/// therefore *untrusted text selects from a trusted set*, and what a finding
/// carries is the trusted half.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubjectId {
    identifier: String,
}

impl SubjectId {
    /// Validates and takes a subject identifier.
    ///
    /// # Errors
    ///
    /// [`AnalysisError::InvalidSubject`] when it is empty, over 64 bytes, or
    /// holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, AnalysisError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        if !valid {
            return Err(AnalysisError::InvalidSubject(value));
        }
        Ok(Self { identifier: value })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// One subject and the trusted needles that recognise it in a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    id: SubjectId,
    manifest_names: Vec<String>,
    import_names: Vec<String>,
    call_names: Vec<String>,
    config_keys: Vec<String>,
}

impl Subject {
    /// Names a subject and the four needle sets that find it.
    ///
    /// Each needle is lowercased on the way in, because every token the readers
    /// produce is lowercased, and a needle that differed only in case would
    /// match nothing while looking correct.
    #[must_use]
    pub fn new(
        id: SubjectId,
        manifest_names: &[&str],
        import_names: &[&str],
        call_names: &[&str],
        config_keys: &[&str],
    ) -> Self {
        let lower = |values: &[&str]| -> Vec<String> {
            values.iter().map(|v| v.to_ascii_lowercase()).collect()
        };
        Self {
            id,
            manifest_names: lower(manifest_names),
            import_names: lower(import_names),
            call_names: lower(call_names),
            config_keys: lower(config_keys),
        }
    }

    /// The identifier.
    #[must_use]
    pub const fn id(&self) -> &SubjectId {
        &self.id
    }
}

/// Whether a token lifted from a repository selects a needle.
///
/// Equality, or equality of one of the token's separator-delimited segments.
/// `redis.url` and `redis` both select `redis`; `myredis` selects nothing.
fn selects(token: &str, needles: &[String]) -> bool {
    needles.iter().any(|needle| {
        token == needle
            || token
                .split(['.', '/', '-', ':', '_'])
                .any(|segment| segment == needle)
    })
}

/// A runtime observation the caller supplies.
///
/// This crate runs no program and reads no trace file. Section 17.3's fifth row
/// is about a trace agreeing with production configuration, so the trace is an
/// argument, and the only thing this crate decides is whether it agrees: a
/// trace that names another snapshot is not evidence about this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTrace {
    snapshot_id: String,
    subject: SubjectId,
}

impl RuntimeTrace {
    /// Records that a subject was seen executing in one snapshot.
    #[must_use]
    pub fn new(snapshot_id: impl Into<String>, subject: SubjectId) -> Self {
        Self {
            snapshot_id: snapshot_id.into(),
            subject,
        }
    }

    /// Which snapshot the trace was taken against.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Which subject it names.
    #[must_use]
    pub const fn subject(&self) -> &SubjectId {
        &self.subject
    }
}

/// What kind of evidence one site is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum SiteKind {
    Manifest,
    Import,
    ReachableCall,
    UnreachableCall,
    Config,
}

/// One matched site, before it is decided whether it counts.
#[derive(Debug, Clone)]
struct Site {
    kind: SiteKind,
    locator: Locator,
    component: ComponentId,
    package: Option<PackageId>,
    enclosing: Option<SymbolFingerprint>,
}

impl Site {
    /// Whether this site speaks about the whole package rather than about the
    /// component it happens to sit in.
    ///
    /// Total over [`SiteKind`] with no default arm, so a sixth kind has to
    /// answer this question rather than inherit an answer.
    const fn is_package(&self) -> bool {
        match self.kind {
            SiteKind::Manifest | SiteKind::Config => true,
            SiteKind::Import | SiteKind::ReachableCall | SiteKind::UnreachableCall => false,
        }
    }
}

/// The five-rung ladder of section 17.3, and the one producer of a [`Finding`].
///
/// It is a unit type rather than a free function so the entry point has a name
/// a call-site count can be taken of: `crates/repository-analysis/tests/
/// analysis_scans.rs` holds the count of `classify` at one, which is what says
/// there is no second route to a finding beside this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceLadder;

impl EvidenceLadder {
    /// Classifies one subject over one analysis, one finding per component.
    ///
    /// Evidence spanning three components produces three findings rather than
    /// one wider one. That is `REQ-34-091` — *a new finding cannot default to
    /// repository-wide scope* — held in the shape of the output rather than in
    /// a check on an argument.
    ///
    /// # Errors
    ///
    /// [`AnalysisError::NoPromotingEvidence`] when nothing outside a vendored,
    /// generated or example path names the subject, and
    /// [`AnalysisError::UncalibratedConfidence`] when a rung that section 17.3
    /// requires a confidence for has no fresh calibration dataset to interpret
    /// its score. Both are refusals rather than downgrades: a finding shown a
    /// tier lower than its evidence, or shown with no number where section 17.3
    /// asks for one, is a different wrong answer rather than a safe one.
    pub fn classify(
        analysis: &RepositoryAnalysis,
        subject: &Subject,
        calibration: &CalibrationRegistry,
        purpose: &Purpose,
        traces: &[RuntimeTrace],
        now: u64,
    ) -> Result<Vec<Finding>, AnalysisError> {
        let (sites, excluded) = collect(analysis, subject);
        if sites.is_empty() {
            return Err(AnalysisError::NoPromotingEvidence(
                subject.id.as_str().to_owned(),
            ));
        }

        // Two kinds of site, and the difference is what they are *about*. An
        // import and a call are about the file they sit in, so they name a
        // component. A manifest entry and a configuration key are about the
        // package: a manifest installs a dependency for every module beside it,
        // and a configuration file configures the program rather than the
        // directory it happens to sit in. Grouping configuration by its own
        // directory would mean section 17.3's third row -- `reachable call +
        // config` -- could only be satisfied by a configuration file that sat
        // next to the call.
        let package_sites: Vec<&Site> = sites.iter().filter(|site| site.is_package()).collect();
        let mut by_component: BTreeMap<ComponentId, Vec<&Site>> = BTreeMap::new();
        for site in sites.iter().filter(|site| !site.is_package()) {
            by_component
                .entry(site.component.clone())
                .or_default()
                .push(site);
        }

        let traced = traces.iter().any(|trace| {
            trace.subject == subject.id && trace.snapshot_id == analysis.snapshot_id()
        });

        let mut findings = Vec::new();
        let mut packages_covered: Vec<Option<PackageId>> = Vec::new();
        for (component, component_sites) in &by_component {
            let package = component_sites
                .first()
                .and_then(|site| site.package.clone());
            packages_covered.push(package.clone());
            let mut counted: Vec<&Site> = component_sites.clone();
            counted.extend(
                package_sites
                    .iter()
                    .copied()
                    .filter(|site| site.package == package),
            );
            let mut excluded_here = excluded.clone();
            for site in package_sites.iter().filter(|site| site.package != package) {
                excluded_here.push(ExcludedSite::new(
                    site.locator.clone(),
                    ExclusionReason::OtherPackage,
                ));
            }
            findings.push(seal_finding(
                analysis,
                subject,
                component.clone(),
                &counted,
                excluded_here,
                traced,
                calibration,
                purpose,
                now,
            )?);
        }

        // A package whose only evidence is package-level gets one finding of
        // its own, scoped to the first such site's component. One per package
        // rather than one per site, because a manifest entry and a lock entry
        // for the same dependency are one fact written twice.
        let mut leftover: BTreeMap<Option<PackageId>, Vec<&Site>> = BTreeMap::new();
        for site in &package_sites {
            if packages_covered.contains(&site.package) {
                continue;
            }
            leftover.entry(site.package.clone()).or_default().push(site);
        }
        for counted in leftover.values() {
            let component = counted
                .iter()
                .map(|site| site.component.clone())
                .min()
                .ok_or_else(|| {
                    AnalysisError::NoPromotingEvidence(subject.id.as_str().to_owned())
                })?;
            findings.push(seal_finding(
                analysis,
                subject,
                component,
                counted,
                excluded.clone(),
                traced,
                calibration,
                purpose,
                now,
            )?);
        }

        if findings.is_empty() {
            return Err(AnalysisError::NoPromotingEvidence(
                subject.id.as_str().to_owned(),
            ));
        }
        Ok(findings)
    }
}

/// Every site the subject's needles select, split into counted and excluded.
fn collect(analysis: &RepositoryAnalysis, subject: &Subject) -> (Vec<Site>, Vec<ExcludedSite>) {
    let mut sites = Vec::new();
    let mut excluded = Vec::new();
    for file in analysis.files() {
        let Ok(component) = ComponentId::containing(file.path()) else {
            continue;
        };
        let mut matched: Vec<(SiteKind, SourceSpan, ArtifactScope)> = Vec::new();
        for dependency in file.dependencies() {
            if selects(&dependency.token, &subject.manifest_names) {
                let scope = if dependency.development_only {
                    ArtifactScope::Test
                } else {
                    file.scope()
                };
                matched.push((SiteKind::Manifest, dependency.span, scope));
            }
        }
        for import in file.imports() {
            if selects(&import.token, &subject.import_names) {
                matched.push((SiteKind::Import, import.span, file.scope()));
            }
        }
        for call in file.calls() {
            if !(selects(&call.callee, &subject.import_names)
                || selects(&call.leaf, &subject.call_names))
            {
                continue;
            }
            let kind = if file.call_is_reachable(call.span) {
                SiteKind::ReachableCall
            } else {
                SiteKind::UnreachableCall
            };
            matched.push((kind, call.span, file.scope()));
        }
        for token in file.config_tokens().chain(file.iac_tokens()) {
            if selects(&token.token, &subject.config_keys) {
                matched.push((SiteKind::Config, token.span, file.scope()));
            }
        }
        for (kind, span, scope) in matched {
            let enclosing = if kind == SiteKind::Manifest || kind == SiteKind::Config {
                None
            } else {
                file.enclosing(span)
            };
            let locator = file.locator(span, enclosing, scope);
            let fingerprint = enclosing.map(|declaration| declaration.fingerprint.clone());
            if file.class().promotes() {
                sites.push(Site {
                    kind,
                    locator,
                    component: component.clone(),
                    package: file.package().cloned(),
                    enclosing: fingerprint,
                });
            } else {
                excluded.push(ExcludedSite::new(
                    locator,
                    ExclusionReason::NonPromotingPath,
                ));
            }
        }
    }
    (sites, excluded)
}

/// Decides the rung for one component's counted sites and seals the finding.
///
/// Named apart from `Finding::seal` so a call-site count of `seal` reads the
/// finding constructor alone: `analysis_scans.rs` holds that at one over the
/// whole package, which is what says there is no second producer of a finding.
#[expect(
    clippy::too_many_arguments,
    reason = "the arguments are the ladder's whole input; grouping them into a struct would \
              move the same list one line up without removing a decision from this function"
)]
fn seal_finding(
    analysis: &RepositoryAnalysis,
    subject: &Subject,
    component: ComponentId,
    counted: &[&Site],
    excluded: Vec<ExcludedSite>,
    traced: bool,
    calibration: &CalibrationRegistry,
    purpose: &Purpose,
    now: u64,
) -> Result<Finding, AnalysisError> {
    let has = |kind: SiteKind| counted.iter().any(|site| site.kind == kind);
    let uses: Vec<&&Site> = counted
        .iter()
        .filter(|site| site.kind != SiteKind::Manifest)
        .collect();
    let production_config = counted.iter().any(|site| {
        site.kind == SiteKind::Config && site.locator.scope() == ArtifactScope::Production
    });
    let test_only = !uses.is_empty()
        && uses
            .iter()
            .all(|site| site.locator.scope() == ArtifactScope::Test);

    let rung = if traced && production_config {
        LadderRung::RuntimeAndProductionConfig
    } else if test_only {
        LadderRung::TestScopedUse
    } else if has(SiteKind::ReachableCall) && has(SiteKind::Config) {
        LadderRung::ReachableCallWithConfig
    } else if !uses.is_empty() {
        LadderRung::UnreachableImport
    } else {
        LadderRung::ManifestPresence
    };

    let artifact_scope = match rung {
        LadderRung::TestScopedUse => ArtifactScope::Test,
        LadderRung::ManifestPresence
        | LadderRung::UnreachableImport
        | LadderRung::ReachableCallWithConfig
        | LadderRung::RuntimeAndProductionConfig => counted
            .iter()
            .map(|site| site.locator.scope())
            .fold(ArtifactScope::Test, ArtifactScope::max),
    };
    let strength = match rung {
        LadderRung::RuntimeAndProductionConfig => EvidenceStrength::Strong,
        LadderRung::ManifestPresence
        | LadderRung::UnreachableImport
        | LadderRung::ReachableCallWithConfig
        | LadderRung::TestScopedUse => EvidenceStrength::Static,
    };

    let tier =
        match rung.tier() {
            crate::finding::EvidenceTier::PresentOnly => TierEvidence::PresentOnly,
            crate::finding::EvidenceTier::Possible => TierEvidence::Possible,
            crate::finding::EvidenceTier::Observed => TierEvidence::Observed(
                calibrated_confidence(analysis, counted, traced, calibration, purpose, now)?,
            ),
        };

    let enclosing: Vec<&SymbolFingerprint> = counted
        .iter()
        .filter(|site| site.kind != SiteKind::Manifest)
        .filter_map(|site| site.enclosing.as_ref())
        .collect();
    let one_symbol = enclosing.len() == uses.len()
        && !enclosing.is_empty()
        && enclosing.windows(2).all(|pair| pair[0] == pair[1]);
    let scope = if one_symbol {
        FindingScope::Symbol {
            component,
            symbol: enclosing[0].clone(),
        }
    } else {
        FindingScope::Component { component }
    };

    Ok(Finding::seal(
        analysis.snapshot_id().to_owned(),
        subject.id.as_str().to_owned(),
        scope,
        tier,
        rung,
        artifact_scope,
        strength,
        counted.iter().map(|site| site.locator.clone()).collect(),
        excluded,
        ComponentCoverage::new(1, analysis.analyzed_component_count()),
    ))
}

/// The raw score, and its interpretation through `P2-M1`'s registry.
///
/// The raw unit is *how many independent kinds of evidence corroborate*, on a
/// scale of five: a manifest entry, an import, a reachable call, a
/// configuration site, and a runtime trace. It is a count and not a weighting,
/// because a weighting would be a number this task invented and then displayed.
/// What turns it into something a reader may see is
/// `CalibrationRegistry::interpret`, which is the only producer of a
/// `CalibratedConfidence` in this workspace.
fn calibrated_confidence(
    analysis: &RepositoryAnalysis,
    counted: &[&Site],
    traced: bool,
    calibration: &CalibrationRegistry,
    purpose: &Purpose,
    now: u64,
) -> Result<DisplayedConfidence, AnalysisError> {
    let kinds = [
        SiteKind::Manifest,
        SiteKind::Import,
        SiteKind::ReachableCall,
        SiteKind::Config,
    ];
    let corroborating = u32::try_from(
        kinds
            .iter()
            .filter(|kind| counted.iter().any(|site| site.kind == **kind))
            .count(),
    )
    .unwrap_or(0)
        + u32::from(traced);
    let units = corroborating.saturating_mul(200);
    let score = RawScore::new(
        analysis.provider().clone(),
        analysis.model_version().clone(),
        units,
    );
    let calibrated = calibration
        .interpret(&score, purpose, now)
        .map_err(|error| AnalysisError::UncalibratedConfidence(error.to_string()))?;
    Ok(DisplayedConfidence::of(&calibrated))
}

/// The analyzer's identity as `P2-M1` names a producer of scores.
///
/// The provider is the analyzer's tool name and the model version is its
/// version, so a calibration dataset is registered for an exact analyzer build.
/// `AnalysisInput::of` refuses a snapshot whose `toolVersions` does not name
/// this pair, which is what binds the number a reader sees to the analyzer the
/// snapshot says produced it — and what section 17.5's `ANALYSIS_CHANGED` lane
/// needs in order to tell an analyzer change from a code change later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzerIdentity {
    tool: String,
    version: String,
    provider: ProviderId,
    model_version: ModelVersion,
}

impl AnalyzerIdentity {
    /// Names the analyzer and its version.
    ///
    /// # Errors
    ///
    /// [`AnalysisError::InvalidAnalyzerIdentity`] when either half is empty.
    pub fn new(tool: impl Into<String>, version: impl Into<String>) -> Result<Self, AnalysisError> {
        let tool = tool.into();
        let version = version.into();
        let provider = ProviderId::new(tool.clone())
            .map_err(|_| AnalysisError::InvalidAnalyzerIdentity(tool.clone()))?;
        let model_version = ModelVersion::new(version.clone())
            .map_err(|_| AnalysisError::InvalidAnalyzerIdentity(version.clone()))?;
        Ok(Self {
            tool,
            version,
            provider,
            model_version,
        })
    }

    /// The tool name, as the snapshot's `toolVersions` spells it.
    #[must_use]
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// The version, as the snapshot's `toolVersions` spells it.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The provider a calibration dataset is registered for.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// The model version a calibration dataset is registered for.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }
}
