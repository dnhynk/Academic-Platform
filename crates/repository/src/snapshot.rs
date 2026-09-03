//! Section 17.2's `RepositorySnapshot`, and the values it is built from.
//!
//! ## Why the snapshot cannot change when the source does
//!
//! Every field is owned and every one is filled at freeze time. The type holds
//! no path, no handle, no closure and no borrow of the tree it was taken from,
//! so there is nothing for a later edit to reach: rereading a field cannot
//! consult the filesystem because the field is the answer rather than a way to
//! get one. There is no `&mut self` method, no public field, no `Default` and
//! no setter, and `crates/repository/tests/repository_scans.rs` pins the whole
//! `impl RepositorySnapshot` block as text, so an accessor that returned a path
//! or a method that mutated a field fails the pin rather than passing quietly.
//!
//! That is a claim about *this value*, and it is the whole of what
//! `snapshot_is_immutable_after_source_change` asserts. It is not a claim that
//! the bytes on disk are protected: the operating system is what decides that,
//! and `docs/contracts/worker-sandbox.md` is where a measured claim about the
//! operating system lives.

use academic_policy::ContentDigest;

use crate::{
    gate::{AdmittedPaths, ExcludedPath, SecretFinding, SecretScanResult},
    github::GitHubError,
    source::{CommitId, RepositorySource, SnapshotType},
};

/// Why a capture did not produce a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RepositoryError {
    /// A path under the root could not be read.
    #[error("the source path {0} could not be read")]
    UnreadablePath(String),
    /// A path escaped the root, or held a byte a manifest cannot carry.
    #[error("the source path {0} is not a relative forward-slashed path")]
    MalformedPath(String),
    /// A `.gitignore` line this parser does not model.
    #[error("the ignore rule !{0} is not modelled by this parser")]
    UnsupportedIgnoreRule(String),
    /// The content scan found something; section 32.4 point 5 is fail-closed.
    #[error("the secret gate blocked this source")]
    SecretGateBlocked,
    /// The inventory was handed an admission decided for another request.
    #[error("the admission names another request")]
    AdmissionMismatch,
    /// A recorded decision was missing a field.
    #[error("a recorded decision has an empty field")]
    EmptyDecisionField,
    /// A snapshot identifier component was empty.
    #[error("a snapshot identifier component is empty")]
    EmptyIdentifier,
    /// The request named a GitHub source and the credential was refused.
    #[error("the GitHub credential was refused: {0}")]
    Credential(#[from] GitHubError),
}

/// A repository identity, as this system names it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepositoryId {
    identifier: String,
}

impl RepositoryId {
    /// Validates and takes a repository identity.
    ///
    /// # Errors
    ///
    /// [`RepositoryError::EmptyIdentifier`] when the identity is empty or over
    /// 128 bytes, or holds a byte outside `[A-Za-z0-9._/-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, RepositoryError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err(RepositoryError::EmptyIdentifier);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
        {
            return Err(RepositoryError::EmptyIdentifier);
        }
        Ok(Self { identifier: value })
    }

    /// The identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// The language a manifest entry was classified as.
///
/// A closed vocabulary with an explicit `Unknown`, because section 17.3's
/// coverage-gap rule needs the absence of a classification to be a value rather
/// than a missing row. `P2-R2` owns the analysis that acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {
    /// Rust.
    Rust,
    /// TypeScript or JavaScript.
    TypeScript,
    /// Python.
    Python,
    /// SQL.
    Sql,
    /// Markdown or another prose format.
    Markdown,
    /// A configuration format: TOML, YAML, JSON.
    Configuration,
    /// Recognised as none of the above.
    Unknown,
}

impl Language {
    /// Exhaustive order.
    pub const ALL: [Self; 7] = [
        Self::Rust,
        Self::TypeScript,
        Self::Python,
        Self::Sql,
        Self::Markdown,
        Self::Configuration,
        Self::Unknown,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "RUST",
            Self::TypeScript => "TYPESCRIPT",
            Self::Python => "PYTHON",
            Self::Sql => "SQL",
            Self::Markdown => "MARKDOWN",
            Self::Configuration => "CONFIGURATION",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Classifies by extension. Anything unrecognised is [`Language::Unknown`].
    #[must_use]
    pub fn of_path(path: &str) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path);
        let extension = name.rsplit_once('.').map_or("", |(_, tail)| tail);
        match extension {
            "rs" => Self::Rust,
            "ts" | "tsx" | "js" | "mjs" | "cjs" => Self::TypeScript,
            "py" => Self::Python,
            "sql" => Self::Sql,
            "md" | "txt" => Self::Markdown,
            "toml" | "yaml" | "yml" | "json" => Self::Configuration,
            _ => Self::Unknown,
        }
    }
}

/// One row of section 17.2's `manifest`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManifestEntry {
    path: String,
    blob_digest: ContentDigest,
    language: Language,
    byte_len: u64,
}

impl ManifestEntry {
    /// The relative, forward-slashed path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// SHA-256 over the file's bytes as they were at freeze time.
    #[must_use]
    pub const fn blob_digest(&self) -> &ContentDigest {
        &self.blob_digest
    }

    /// The classification.
    #[must_use]
    pub const fn language(&self) -> Language {
        self.language
    }

    /// Length in bytes as at freeze time.
    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

/// Whether a dirty entry is tracked by version control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DirtyKind {
    /// Version control tracks the path and its bytes differ from the commit's.
    Tracked,
    /// Version control does not track the path.
    Untracked,
}

impl DirtyKind {
    /// Exhaustive order. Both arms are manifested; neither is a default.
    pub const ALL: [Self; 2] = [Self::Tracked, Self::Untracked];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tracked => "TRACKED",
            Self::Untracked => "UNTRACKED",
        }
    }
}

/// The explicit tracked and untracked manifest section 17.2 requires.
///
/// The two lists are separate fields with separate accessors, and the
/// constructor takes both. There is no combined list and no flag on a single
/// list: a caller that wanted to record only one half would have to pass an
/// empty vector for the other, which the manifest then reports as empty rather
/// than as absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyManifest {
    tracked: Vec<ManifestEntry>,
    untracked: Vec<ManifestEntry>,
    patch_digest: ContentDigest,
}

impl DirtyManifest {
    /// The tracked paths whose working-tree bytes differ from the commit's.
    #[must_use]
    pub fn tracked(&self) -> &[ManifestEntry] {
        &self.tracked
    }

    /// The paths version control does not track.
    #[must_use]
    pub fn untracked(&self) -> &[ManifestEntry] {
        &self.untracked
    }

    /// Section 17.2's `dirtyPatchArtifact`, as a digest over both halves.
    ///
    /// It covers the tracked and untracked lists together, so a change to
    /// either moves it. That is what makes it usable as the identity of the
    /// difference between this working tree and its HEAD.
    #[must_use]
    pub const fn patch_digest(&self) -> &ContentDigest {
        &self.patch_digest
    }

    /// Every entry, tracked first, each labelled with which half it came from.
    #[must_use]
    pub fn entries(&self) -> Vec<(DirtyKind, &ManifestEntry)> {
        self.tracked
            .iter()
            .map(|entry| (DirtyKind::Tracked, entry))
            .chain(
                self.untracked
                    .iter()
                    .map(|entry| (DirtyKind::Untracked, entry)),
            )
            .collect()
    }
}

/// A submodule reference section 17.2 records beside the manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubmoduleRef {
    path: String,
    commit: CommitId,
}

impl SubmoduleRef {
    /// Names a submodule at a path and the commit it is pinned to.
    #[must_use]
    pub const fn new(path: String, commit: CommitId) -> Self {
        Self { path, commit }
    }

    /// The submodule's path inside this repository.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The commit it is pinned to.
    #[must_use]
    pub const fn commit(&self) -> &CommitId {
        &self.commit
    }
}

/// One tool and the version of it that produced part of this snapshot.
///
/// Section 17.2 lists `toolVersions` as a snapshot field, and section 17.5's
/// `ANALYSIS_CHANGED` lane — which `P2-R3` owns — needs it to separate a change
/// in the code from a change in the analyzer. Recording it here is what makes
/// that separation possible later.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ToolVersion {
    tool: String,
    version: String,
}

impl ToolVersion {
    /// Names a tool and its version.
    ///
    /// # Errors
    ///
    /// [`RepositoryError::EmptyIdentifier`] when either half is empty.
    pub fn new(
        tool: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, RepositoryError> {
        let tool = tool.into();
        let version = version.into();
        if tool.is_empty() || version.is_empty() {
            return Err(RepositoryError::EmptyIdentifier);
        }
        Ok(Self { tool, version })
    }

    /// The tool.
    #[must_use]
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// Its version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// What the gate admitted, read and digested, before anything froze it.
///
/// The only constructor is crate-private and takes an [`AdmittedPaths`], so
/// this value cannot exist without the gate having decided. It is consumed by
/// value when a snapshot is frozen, so one inventory produces one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    request_digest: ContentDigest,
    entries: Vec<ManifestEntry>,
    dirty_tracked: Vec<ManifestEntry>,
    dirty_untracked: Vec<ManifestEntry>,
    excluded: Vec<ExcludedPath>,
    findings: Vec<SecretFinding>,
    scan_result: SecretScanResult,
    read_paths: Vec<String>,
}

impl Inventory {
    pub(crate) fn of(
        admitted: &AdmittedPaths,
        entries: Vec<ManifestEntry>,
        dirty_tracked: Vec<ManifestEntry>,
        dirty_untracked: Vec<ManifestEntry>,
        read_paths: Vec<String>,
    ) -> Self {
        Self {
            request_digest: admitted.request_digest().clone(),
            entries,
            dirty_tracked,
            dirty_untracked,
            excluded: admitted.excluded().to_vec(),
            findings: admitted.findings().to_vec(),
            scan_result: admitted.result(),
            read_paths,
        }
    }

    /// Every path this inventory opened, in the order it opened them.
    ///
    /// `analyzer_never_sees_an_excluded_path` compares this against the set the
    /// policy admits, which is computed independently of the walk.
    #[must_use]
    pub fn read_paths(&self) -> &[String] {
        &self.read_paths
    }

    /// The manifest rows.
    #[must_use]
    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }
}

/// Section 17.2's `RepositorySnapshot`, frozen.
///
/// Every field is owned and set once. See the module documentation for why that
/// is the whole of the immutability claim and what it deliberately does not
/// claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySnapshot {
    snapshot_id: String,
    repository: RepositoryId,
    source: RepositorySource,
    snapshot_type: SnapshotType,
    branch: Option<String>,
    commit: Option<CommitId>,
    parent_snapshots: Vec<String>,
    captured_at: u64,
    manifest: Vec<ManifestEntry>,
    manifest_digest: ContentDigest,
    dirty: Option<DirtyManifest>,
    submodule_refs: Vec<SubmoduleRef>,
    analysis_policy_hash: ContentDigest,
    tool_versions: Vec<ToolVersion>,
    secret_scan_result: SecretScanResult,
    secret_findings: Vec<SecretFinding>,
    excluded: Vec<ExcludedPath>,
}

/// Everything the freeze step needs that the inventory does not carry.
///
/// A separate value rather than a long parameter list, because the fields are
/// what section 17.2 names and reading them at the call site is the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotIdentity {
    /// The repository this is a snapshot of.
    pub repository: RepositoryId,
    /// Which of the eight inputs was named.
    pub source: RepositorySource,
    /// What the snapshot turned out to be of.
    pub snapshot_type: SnapshotType,
    /// The branch, when the working-tree facts reported one.
    pub branch: Option<String>,
    /// The commit, when there is one.
    pub commit: Option<CommitId>,
    /// Earlier snapshots this one follows.
    pub parent_snapshots: Vec<String>,
    /// The caller's recorded capture time in milliseconds.
    pub captured_at: u64,
    /// Submodules and the commits they are pinned to.
    pub submodule_refs: Vec<SubmoduleRef>,
    /// The digest of the analysis policy that produced this snapshot.
    pub analysis_policy_hash: ContentDigest,
    /// The tools and versions that produced it.
    pub tool_versions: Vec<ToolVersion>,
}

impl RepositorySnapshot {
    /// The one constructor. Crate-private, and called only from `freeze`.
    pub(crate) fn freeze(identity: SnapshotIdentity, listed: Inventory) -> Self {
        let manifest_digest = digest_of_manifest(&listed.entries);
        let dirty = if listed.dirty_tracked.is_empty() && listed.dirty_untracked.is_empty() {
            None
        } else {
            let mut preimage = Vec::new();
            for entry in listed
                .dirty_tracked
                .iter()
                .chain(listed.dirty_untracked.iter())
            {
                preimage.extend_from_slice(entry.blob_digest.as_str().as_bytes());
                preimage.push(0);
                preimage.extend_from_slice(entry.path.as_bytes());
                preimage.push(0);
            }
            Some(DirtyManifest {
                tracked: listed.dirty_tracked,
                untracked: listed.dirty_untracked,
                patch_digest: ContentDigest::of(&preimage),
            })
        };
        let snapshot_id = format!(
            "snap_{}_{}_{}",
            identity.repository.as_str().replace('/', "-"),
            identity
                .commit
                .as_ref()
                .map_or("nocommit", CommitId::as_str),
            manifest_digest.as_str()
        );
        Self {
            snapshot_id,
            repository: identity.repository,
            source: identity.source,
            snapshot_type: identity.snapshot_type,
            branch: identity.branch,
            commit: identity.commit,
            parent_snapshots: identity.parent_snapshots,
            captured_at: identity.captured_at,
            manifest: listed.entries,
            manifest_digest,
            dirty,
            submodule_refs: identity.submodule_refs,
            analysis_policy_hash: identity.analysis_policy_hash,
            tool_versions: identity.tool_versions,
            secret_scan_result: listed.scan_result,
            secret_findings: listed.findings,
            excluded: listed.excluded,
        }
    }

    /// The snapshot identity.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Which repository.
    #[must_use]
    pub const fn repository(&self) -> &RepositoryId {
        &self.repository
    }

    /// Which of the eight inputs was named.
    #[must_use]
    pub const fn source(&self) -> RepositorySource {
        self.source
    }

    /// Section 17.2's `sourceType`.
    #[must_use]
    pub const fn snapshot_type(&self) -> SnapshotType {
        self.snapshot_type
    }

    /// The branch, when one was reported.
    #[must_use]
    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    /// The commit, when there is one.
    #[must_use]
    pub const fn commit(&self) -> Option<&CommitId> {
        self.commit.as_ref()
    }

    /// Earlier snapshots this one follows.
    #[must_use]
    pub fn parent_snapshots(&self) -> &[String] {
        &self.parent_snapshots
    }

    /// The caller's recorded capture time.
    #[must_use]
    pub const fn captured_at(&self) -> u64 {
        self.captured_at
    }

    /// The manifest rows.
    #[must_use]
    pub fn manifest(&self) -> &[ManifestEntry] {
        &self.manifest
    }

    /// A digest over the whole manifest, path and blob digest per row.
    #[must_use]
    pub const fn manifest_digest(&self) -> &ContentDigest {
        &self.manifest_digest
    }

    /// The tracked and untracked manifest, when the tree was dirty.
    #[must_use]
    pub const fn dirty(&self) -> Option<&DirtyManifest> {
        self.dirty.as_ref()
    }

    /// Submodules and the commits they are pinned to.
    #[must_use]
    pub fn submodule_refs(&self) -> &[SubmoduleRef] {
        &self.submodule_refs
    }

    /// The analysis policy that produced this snapshot.
    #[must_use]
    pub const fn analysis_policy_hash(&self) -> &ContentDigest {
        &self.analysis_policy_hash
    }

    /// The tools and versions that produced it.
    #[must_use]
    pub fn tool_versions(&self) -> &[ToolVersion] {
        &self.tool_versions
    }

    /// Section 17.2's `secretScanResult`.
    #[must_use]
    pub const fn secret_scan_result(&self) -> SecretScanResult {
        self.secret_scan_result
    }

    /// What the content scan found, each with whatever disclosure it carries.
    #[must_use]
    pub fn secret_findings(&self) -> &[SecretFinding] {
        &self.secret_findings
    }

    /// The paths the policy removed before anything read them.
    #[must_use]
    pub fn excluded(&self) -> &[ExcludedPath] {
        &self.excluded
    }
}

/// A digest over a manifest: path and blob digest per row, length-delimited.
pub(crate) fn digest_of_manifest(entries: &[ManifestEntry]) -> ContentDigest {
    let mut preimage = b"academic-repository-manifest-v1\0".to_vec();
    for entry in entries {
        preimage.extend_from_slice(entry.path.as_bytes());
        preimage.push(0);
        preimage.extend_from_slice(entry.blob_digest.as_str().as_bytes());
        preimage.push(0);
    }
    ContentDigest::of(&preimage)
}

/// Builds one manifest row from a path and its bytes.
pub(crate) fn manifest_entry(path: &str, bytes: &[u8]) -> ManifestEntry {
    ManifestEntry {
        path: path.to_owned(),
        blob_digest: ContentDigest::of(bytes),
        language: Language::of_path(path),
        byte_len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    }
}
