//! `P2-R3`: section 17.3's fourth stage — `cross-artifact correlation, spec ↔
//! ADR ↔ code ↔ config ↔ test ↔ runtime/incident` — and the drift lanes section
//! 17.5 and section 19 fix over it.
//!
//! `P2-R2` produced the code half: findings at one of section 17.3's five
//! rungs, folded onto `PRESENT_ONLY`/`POSSIBLE`/`OBSERVED`, scoped to a symbol
//! or a component and never to the repository. This crate takes those findings
//! and the artifacts that are not code, and produces section 17.5's seven typed
//! relations, its `ImplementationDrift`, and section 19's two diff channels.
//!
//! ## It builds on `P2-R2` and does not go around it
//!
//! A relation about code is derived from a [`Finding`] and from nothing else.
//! There is no second reader here, no second tier vocabulary, and no route from
//! repository bytes to a relation that does not pass through the ladder:
//! `PROJECT_CODE_USES` requires [`EvidenceTier::Observed`], which is exactly
//! `P2-R2`'s answer to *is there evidence of use*, and section 17.3's second row
//! — `import만 있고 reachable use 없음` — is `보류` there, so an unreachable
//! import produces no implementation-lane relation here either.
//!
//! ## It opens nothing
//!
//! Like `P2-R2`, and for the same reason. Every artifact that is not code — a
//! specification, an architecture decision, a document describing behaviour, an
//! incident record — arrives as an argument naming [`SubjectId`]s, which are
//! values this system chose rather than text lifted out of a repository. This
//! crate never sees a `SourceUnit` and holds no analyzed byte at all.
//! `the_correlation_crate_touches_no_file_and_no_socket` compares the whole set
//! of its `use` items, the whole set of the paths it reaches through a crate
//! root, and the whole set of the macros it invokes against pinned inventories,
//! in both directions.
//!
//! ## The two lanes are not mixed
//!
//! Section 30.3's rows four and five are two questions, not two answers to one.
//! [`authority`] is where a piece of evidence is admitted into one row's table
//! — and refused entry to the other's — and the ordering inside a row is
//! `academic-ledger`'s, unchanged. A disagreement produces an
//! [`ImplementationDrift`] beside both lanes and rewrites neither, which is
//! `CONTRIBUTING.md` rule 2 applied to correlation.

pub mod artifact;
pub mod authority;
pub mod compare;
pub mod drift;
pub mod edge;
pub mod relation;

use std::collections::{BTreeMap, BTreeSet};

use academic_repository::RepositorySnapshot;
use academic_repository_analysis::{
    AnalyzerIdentity, ArtifactScope, EvidenceTier, FileKind, Finding, LadderRung, SubjectId,
};

pub use artifact::{
    ApprovalStatus, BehaviorDocument, DeploymentRecord, DeploymentTarget, DocumentId,
    FeatureFlagRecord, FlagKey, FlagState, IncidentId, IncidentRecord, IntentDocument,
    IntentDocumentKind,
};
pub use authority::{AnswerSource, Candidate, LaneAnswer, RankedCandidate, active_view};
pub use compare::{
    ChangeCause, DependencyChange, PresenceChange, SemanticChange, SemanticTransition,
    SnapshotComparison, compare,
};
pub use drift::{
    BranchDifference, DeprecatedSpec, DriftKind, DriftScopeKind, DriftScopes, GatingFlag,
    ImplementationDrift, UndeployedCode,
};
pub use edge::{EdgeEvidence, RelationEdge};
pub use relation::{AuthorityLane, EvidenceRelation};

/// Why a correlation or a comparison was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CorrelationError {
    /// An identifier was empty, too long, or held a forbidden byte.
    #[error("the {0} identifier {1:?} is not [A-Za-z0-9._-] within 64 bytes")]
    InvalidIdentifier(&'static str, String),
    /// A finding names a snapshot other than the one being correlated.
    #[error("the finding for {0} is about snapshot {1}, not the correlated one")]
    FindingIsAboutAnotherSnapshot(String, String),
    /// A document names a path the frozen manifest does not hold.
    #[error("the document path {0} is not in the snapshot's manifest")]
    DocumentNotInSnapshot(String),
    /// An incident names a snapshot other than the one being correlated.
    #[error("the incident {0} is about snapshot {1}, not the correlated one")]
    IncidentIsAboutAnotherSnapshot(String, String),
    /// The snapshot's `toolVersions` does not name this analyzer.
    #[error("the snapshot does not record the analyzer {0} at version {1}")]
    AnalyzerNotInSnapshot(String, String),
    /// Section 30.3 has no row for the description lane.
    #[error("section 30.3 has no authority row for the {0} lane")]
    LaneHasNoAuthorityRow(AuthorityLane),
    /// Both the snapshot and the analyzer moved, so no difference between the
    /// two runs can be attributed to either.
    #[error("snapshots {0} and {1} were read by different analyzers; the comparison is confounded")]
    ConfoundedComparison(String, String),
    /// Neither the snapshot nor the analyzer moved, so there is no axis.
    #[error("both runs are snapshot {0} under one analyzer; there is no axis to attribute along")]
    NoComparisonAxis(String),
}

impl core::fmt::Display for AuthorityLane {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One correlation request: one snapshot, one analyzer, and every artifact.
///
/// Public fields, the way `academic-repository`'s own `SnapshotRequest` has
/// them: this is the argument list of [`correlate`] and every field is
/// required. The checking happens in [`correlate`], which is the one place a
/// correlation is built.
#[derive(Debug)]
pub struct CorrelationInput<'a> {
    /// The frozen snapshot every artifact is checked against.
    pub snapshot: &'a RepositorySnapshot,
    /// The analyzer build the findings came from. Section 19's
    /// `ANALYSIS_CHANGED` lane needs it named.
    pub analyzer: &'a AnalyzerIdentity,
    /// `P2-R2`'s findings, from any number of `EvidenceLadder::classify` calls.
    pub findings: &'a [Finding],
    /// Specifications and architecture decisions.
    pub intent_documents: &'a [IntentDocument],
    /// Documents describing current behaviour.
    pub behavior_documents: &'a [BehaviorDocument],
    /// Incident records.
    pub incidents: &'a [IncidentRecord],
    /// Feature flags and what they gate.
    pub feature_flags: &'a [FeatureFlagRecord],
    /// Which snapshot each deployment target is running.
    pub deployments: &'a [DeploymentRecord],
}

/// What one correlation run produced.
///
/// No method takes `&mut self`. A correction is a new run over new evidence,
/// which is `CONTRIBUTING.md` rule 2; `no_public_function_mutates_in_place`
/// holds it over the whole crate rather than over this type alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correlation {
    snapshot_id: String,
    analyzer_tool: String,
    analyzer_version: String,
    edges: Vec<RelationEdge>,
    drifts: Vec<ImplementationDrift>,
    dependencies: BTreeSet<String>,
}

impl Correlation {
    /// Which snapshot was correlated.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// The analyzer build the findings came from.
    #[must_use]
    pub fn analyzer_tool(&self) -> &str {
        &self.analyzer_tool
    }

    /// The analyzer version. Section 19's second comparison axis.
    #[must_use]
    pub fn analyzer_version(&self) -> &str {
        &self.analyzer_version
    }

    /// Every typed relation this run produced.
    #[must_use]
    pub fn relations(&self) -> &[RelationEdge] {
        &self.edges
    }

    /// Every drift this run produced.
    #[must_use]
    pub fn drifts(&self) -> &[ImplementationDrift] {
        &self.drifts
    }

    /// One lane's edges for one subject.
    ///
    /// The lane filter is [`EvidenceRelation::lane`] and nothing else, so a
    /// drift cannot change what a lane view returns — which is what *neither
    /// side is overwritten* means when a reader asks each lane its own
    /// question.
    #[must_use]
    pub fn lane_view(&self, lane: AuthorityLane, subject: &str) -> Vec<&RelationEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.lane() == lane && edge.subject() == subject)
            .collect()
    }

    /// The subjects a dependency manifest or a lock file declares.
    ///
    /// Section 19's `단순 dependency diff` is over this and nothing else. A
    /// site counts when `P2-R2`'s own [`FileKind::of_path`] calls its path a
    /// manifest or a lock file, so the classification is the analyzer's rather
    /// than a second one written here.
    #[must_use]
    pub const fn declared_dependencies(&self) -> &BTreeSet<String> {
        &self.dependencies
    }
}

/// Section 17.3's fourth stage, over one validated request.
///
/// # Errors
///
/// [`CorrelationError::AnalyzerNotInSnapshot`],
/// [`CorrelationError::FindingIsAboutAnotherSnapshot`],
/// [`CorrelationError::DocumentNotInSnapshot`] or
/// [`CorrelationError::IncidentIsAboutAnotherSnapshot`]. Each is a way a
/// correlation could otherwise be about something other than the snapshot it
/// names.
pub fn correlate(input: &CorrelationInput<'_>) -> Result<Correlation, CorrelationError> {
    let snapshot_id = input.snapshot.snapshot_id();
    let recorded = input.snapshot.tool_versions().iter().any(|tool| {
        tool.tool() == input.analyzer.tool() && tool.version() == input.analyzer.version()
    });
    if !recorded {
        return Err(CorrelationError::AnalyzerNotInSnapshot(
            input.analyzer.tool().to_owned(),
            input.analyzer.version().to_owned(),
        ));
    }
    let manifest: BTreeSet<&str> = input
        .snapshot
        .manifest()
        .iter()
        .map(academic_repository::ManifestEntry::path)
        .collect();

    let mut edges = Vec::new();
    let mut dependencies = BTreeSet::new();
    for finding in input.findings {
        if finding.snapshot_id() != snapshot_id {
            return Err(CorrelationError::FindingIsAboutAnotherSnapshot(
                finding.subject().to_owned(),
                finding.snapshot_id().to_owned(),
            ));
        }
        if finding
            .locators()
            .iter()
            .any(|locator| declares_dependency(FileKind::of_path(locator.path())))
        {
            dependencies.insert(finding.subject().to_owned());
        }
        for relation in code_relations(finding) {
            edges.push(RelationEdge::seal(
                relation,
                finding.subject().to_owned(),
                snapshot_id.to_owned(),
                EdgeEvidence::Analysis {
                    rung: finding.rung(),
                    tier: finding.tier(),
                    artifact_scope: finding.artifact_scope(),
                    locators: finding.locators().to_vec(),
                },
            ));
        }
    }

    for document in input.intent_documents {
        if !manifest.contains(document.path()) {
            return Err(CorrelationError::DocumentNotInSnapshot(
                document.path().to_owned(),
            ));
        }
        for subject in document.mentions() {
            edges.push(RelationEdge::seal(
                document.kind().relation(),
                subject.as_str().to_owned(),
                snapshot_id.to_owned(),
                EdgeEvidence::Document {
                    document: document.id().clone(),
                    status: document.status(),
                    revision: document.revision(),
                    path: document.path().to_owned(),
                },
            ));
        }
    }

    for document in input.behavior_documents {
        if !manifest.contains(document.path()) {
            return Err(CorrelationError::DocumentNotInSnapshot(
                document.path().to_owned(),
            ));
        }
        for subject in document.explains() {
            edges.push(RelationEdge::seal(
                EvidenceRelation::DocExplains,
                subject.as_str().to_owned(),
                snapshot_id.to_owned(),
                EdgeEvidence::Document {
                    document: document.id().clone(),
                    // A behaviour document describes rather than approves, so
                    // it carries no approval of its own. `APPROVED` here is the
                    // absence of an approval question, not an approval: the
                    // description lane has no section 30.3 row and
                    // `authority::active_view` refuses it.
                    status: ApprovalStatus::Approved,
                    revision: 0,
                    path: document.path().to_owned(),
                },
            ));
        }
    }

    for incident in input.incidents {
        if incident.snapshot_id() != snapshot_id {
            return Err(CorrelationError::IncidentIsAboutAnotherSnapshot(
                incident.id().as_str().to_owned(),
                incident.snapshot_id().to_owned(),
            ));
        }
        for subject in incident.exposed() {
            edges.push(RelationEdge::seal(
                EvidenceRelation::IncidentExposed,
                subject.as_str().to_owned(),
                snapshot_id.to_owned(),
                EdgeEvidence::Incident {
                    incident: incident.id().clone(),
                    occurred_at: incident.occurred_at(),
                },
            ));
        }
    }

    edges.sort_by(|left, right| {
        left.subject()
            .cmp(right.subject())
            .then_with(|| left.relation().cmp(&right.relation()))
    });
    let drifts = drifts_of(input, snapshot_id, &edges);

    Ok(Correlation {
        snapshot_id: snapshot_id.to_owned(),
        analyzer_tool: input.analyzer.tool().to_owned(),
        analyzer_version: input.analyzer.version().to_owned(),
        edges,
        drifts,
        dependencies,
    })
}

/// Whether a file of this kind declares a dependency.
///
/// Total over [`FileKind`] with no default arm: a fifteenth kind has to answer
/// this rather than inherit an answer. A configuration document is not here —
/// `P2-R2`'s contract already separates a dependency entry from a configuration
/// key, and section 19's first channel is `단순 dependency diff`.
const fn declares_dependency(kind: FileKind) -> bool {
    match kind {
        FileKind::CargoManifest
        | FileKind::NodeManifest
        | FileKind::PythonManifest
        | FileKind::LockFile => true,
        FileKind::RustSource
        | FileKind::TypeScriptSource
        | FileKind::PythonSource
        | FileKind::SqlScript
        | FileKind::ConfigDocument
        | FileKind::ContainerFile
        | FileKind::ComposeFile
        | FileKind::CiWorkflow
        | FileKind::Prose
        | FileKind::Unsupported => false,
    }
}

/// The section 17.5 relations one `P2-R2` finding produces.
///
/// Three rules, each reading `P2-R2`'s pinned fold rather than restating it:
///
/// * `PROJECT_CODE_USES` needs [`EvidenceTier::Observed`]. Section 17.3's first
///   row is `불가` and its second is `보류`, so manifest presence and an
///   unreachable import are not observations of use — which is what makes a
///   specification-mentioned subject with a dead import
///   `INTENDED_NOT_IMPLEMENTED` rather than implemented.
/// * `PROJECT_TEST_EXERCISES` is that same observation at
///   [`ArtifactScope::Test`], which is section 17.3's fourth row: `scope를
///   제한해 가능`.
/// * `PROJECT_CONFIG_ENABLES` is section 17.3's fifth row, `runtime
///   trace/production config와 일치`, which is section 17.5's `실행 구성에서
///   활성화`: the configuration a trace agreed with is the one that enables the
///   subject in the running system.
fn code_relations(finding: &Finding) -> Vec<EvidenceRelation> {
    let mut relations = Vec::new();
    if finding.tier() == EvidenceTier::Observed {
        if finding.artifact_scope() == ArtifactScope::Test {
            relations.push(EvidenceRelation::TestExercises);
        } else {
            relations.push(EvidenceRelation::CodeUses);
        }
    }
    if finding.rung() == LadderRung::RuntimeAndProductionConfig {
        relations.push(EvidenceRelation::ConfigEnables);
    }
    relations
}

/// Section 17.5's drift, one per subject that has one.
///
/// Both kinds read the edge set and remove nothing from it. The edges a drift
/// carries are clones of the edges the correlation still holds, which is why
/// `conflict_creates_drift_without_overwrite` can compare the two.
fn drifts_of(
    input: &CorrelationInput<'_>,
    snapshot_id: &str,
    edges: &[RelationEdge],
) -> Vec<ImplementationDrift> {
    let mut by_subject: BTreeMap<&str, Vec<&RelationEdge>> = BTreeMap::new();
    for edge in edges {
        by_subject.entry(edge.subject()).or_default().push(edge);
    }

    let mut drifts = Vec::new();
    for (subject, subject_edges) in by_subject {
        let side = |lane: AuthorityLane| -> Vec<RelationEdge> {
            subject_edges
                .iter()
                .filter(|edge| edge.lane() == lane)
                .map(|edge| (*edge).clone())
                .collect()
        };
        let intent = side(AuthorityLane::Intent);
        let implementation = side(AuthorityLane::Implementation);
        let description = side(AuthorityLane::Description);
        let code_uses = implementation
            .iter()
            .any(|edge| edge.relation() == EvidenceRelation::CodeUses);

        let kind = if !intent.is_empty() && !code_uses {
            Some(DriftKind::IntendedNotImplemented)
        } else if code_uses && description.is_empty() {
            Some(DriftKind::ImplementedNotDocumented)
        } else {
            None
        };
        let Some(kind) = kind else {
            continue;
        };
        drifts.push(ImplementationDrift::seal(
            kind,
            subject.to_owned(),
            snapshot_id.to_owned(),
            intent,
            implementation,
            description,
            scopes_of(input, snapshot_id, subject),
        ));
    }
    drifts
}

/// Section 17.5's four drift scopes for one subject.
///
/// Each is read from a different argument and none is read from another's, so
/// no two can be established by the same evidence.
fn scopes_of(input: &CorrelationInput<'_>, snapshot_id: &str, subject: &str) -> DriftScopes {
    let names =
        |mentions: &[SubjectId]| -> bool { mentions.iter().any(|value| value.as_str() == subject) };

    let deprecated_spec = input
        .intent_documents
        .iter()
        .filter(|document| {
            document.status() == ApprovalStatus::Deprecated && names(document.mentions())
        })
        .max_by_key(|document| document.revision())
        .map(|document| DeprecatedSpec::seal(document.id().clone(), document.revision()));

    let feature_flag = input
        .feature_flags
        .iter()
        .find(|flag| names(flag.gates()))
        .map(|flag| GatingFlag::seal(flag.key().clone(), flag.state()));

    // Undeployed only when nothing runs this snapshot. One target running it is
    // enough to make the code deployed, and no deployment record at all is a
    // question this input does not answer rather than an absence of deployment.
    let deployed_here = input
        .deployments
        .iter()
        .any(|record| record.deployed_snapshot() == snapshot_id);
    let undeployed_code = if deployed_here {
        None
    } else {
        input
            .deployments
            .iter()
            .min_by_key(|record| record.target().as_str().to_owned())
            .map(|record| {
                UndeployedCode::seal(
                    record.target().clone(),
                    record.deployed_snapshot().to_owned(),
                )
            })
    };

    let snapshot_branch = input.snapshot.branch();
    let branch_difference = input
        .intent_documents
        .iter()
        .filter(|document| names(document.mentions()))
        .find_map(|document| {
            let named = document.branch()?;
            if Some(named) == snapshot_branch {
                None
            } else {
                Some(BranchDifference::seal(
                    named.to_owned(),
                    snapshot_branch.map(str::to_owned),
                ))
            }
        });

    DriftScopes::seal(
        deprecated_spec,
        feature_flag,
        undeployed_code,
        branch_difference,
    )
}
