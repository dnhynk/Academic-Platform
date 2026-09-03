//! `P2-R1`: the repository snapshot and the gate that runs before it.
//!
//! Section 17.3 of the authoritative specification draws four stages with
//! `permission + secret gate` at the top. This crate owns the first three of
//! them — the gate, the inventory, and the immutable snapshot — plus the seam
//! the indexer is reached through. `P2-R2` owns what the indexer then does.
//!
//! ## The ordering is a type, a count, and an observation
//!
//! [`SnapshotStages`] is the four-stage seam and [`capture`] is the only driver
//! of it. Three independent things hold the order:
//!
//! * [`AdmittedPaths`] and [`Inventory`] have crate-private constructors, so an
//!   implementation of [`SnapshotStages`] written in another crate cannot
//!   return either without calling this crate's stage that produces it. An
//!   admission also carries the digest of the request it was decided for, and
//!   [`LocalStages::inventory`] refuses one that names a different request, so
//!   an admission cannot be carried from one capture into another.
//! * `crates/repository/tests/repository_scans.rs` pins [`capture`] whole and
//!   counts the call sites of every stage and of the gate's two internals over
//!   the whole package, with the file each may be called from. A count of one
//!   is what says there is no second path.
//! * `secret_gate_precedes_indexer` drives the real [`LocalStages`] through a
//!   spy that records each stage as it is entered. On a repository the gate
//!   blocks, the spy's index count is zero.
//!
//! ## Everything a repository holds is untrusted
//!
//! What survives a read is an `Untrusted<IngestedDocument>` from `P2-G5`, held
//! in that crate's `SourceIndex`. This crate keeps no other copy of a file's
//! text: the manifest holds a digest, a length and a language, and the bytes
//! themselves exist only inside the closure that read them and inside the
//! sealed wrapper.
//!
//! ## What "the analyzer cannot mutate the source or open a socket" means here
//!
//! Three levels, and they are not the same claim:
//!
//! * **This crate.** [`analyze`] takes a frozen [`RepositorySnapshot`] by
//!   reference and returns a value. It holds no path and no handle; there is
//!   nothing in its argument to write through. `crates/repository/tests/
//!   repository_scans.rs` compares the crate's whole `std::fs` surface against
//!   a pinned read-only set, so a mutating call appears as an extra key rather
//!   than as a missing token, and `only_egress_crate_has_a_socket` in
//!   `tools/phase1-scaffold-policy.test.mjs` reads the absence of a socket
//!   spelling and of a socket-capable link edge for this package.
//! * **The process class.** `ProcessClass::RepositoryAnalyzer` holds
//!   `ReadArtifactRange`, `AnalyzeRepository` and `CreateClaim` and nothing
//!   else. `P2-G1`'s broker answers a request for `OpenOutboundSocket` or
//!   `WriteStagedArtifact` with a denial and a `DENY` audit row, and
//!   `analyzer_cannot_mutate_source_or_open_a_socket` observes that rather than
//!   restating the matrix.
//! * **The operating system.** Not this crate's claim. `P2-G4` measured what a
//!   kernel refuses a sandboxed process, on which platform, with which error
//!   number; [`docs/contracts/worker-sandbox.md`] is where that lives, and this
//!   contract cites it rather than repeating it at a strength nothing here
//!   executes.
//!
//! [`docs/contracts/worker-sandbox.md`]: https://example.invalid/worker-sandbox

pub mod gate;
pub mod github;
pub mod snapshot;
pub mod source;

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use academic_policy::ContentDigest;
use academic_untrusted_content::{
    IngestedDocument, SourceId, SourceIndex, SourceKind, Untrusted, ingest,
};

pub use gate::{
    AdmittedPaths, ContentVerdict, DisclosureDecision, ExcludedPath, ExclusionReason, PathPolicy,
    PathRule, SecretFinding, SecretScanResult,
};
pub use github::{
    Access, CredentialStore, FineGrainedToken, GitHubError, GitHubRepository,
    GitHubRepositoryReader, MAX_TOKEN_LIFETIME_MILLIS, SealedCredential, TokenLifetime,
    TokenPermission, TokenScope,
};
pub use snapshot::{
    DirtyKind, DirtyManifest, Inventory, Language, ManifestEntry, RepositoryError, RepositoryId,
    RepositorySnapshot, SnapshotIdentity, SubmoduleRef, ToolVersion,
};
pub use source::{CommitId, CommitIdError, RepositorySource, SnapshotType, WorkingTreeFacts};

/// One file of a source tree that is already in memory.
///
/// The byte field is named `source_bytes` for the reason
/// `academic-untrusted-content`'s own document type names its field that:
/// `tools/secret-debug-policy.test.mjs` holds that name in `SECRET_FIELD_NAMES`,
/// so the derived `Debug` this struct would otherwise have is refused by the
/// existing discovery net rather than by a rule this crate invented. The
/// hand-written one below prints a length.
pub struct SourceEntry {
    path: String,
    source_bytes: Vec<u8>,
}

impl core::fmt::Debug for SourceEntry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SourceEntry")
            .field("path", &self.path)
            .field(
                "source_bytes",
                &format_args!("<untrusted:{} bytes>", self.source_bytes.len()),
            )
            .finish()
    }
}

impl SourceEntry {
    /// Names one file and its bytes.
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

    /// How many bytes the file holds.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.source_bytes.len()
    }
}

/// Where the bytes of a source come from.
///
/// Four of the eight inputs are a directory this machine already holds; the
/// other four arrive as entries someone else produced — an archive reader, a
/// `GitHubRepositoryReader`, or a specification-only project's document set.
/// Both arms are read-only: neither offers a write, a create or a remove.
#[derive(Debug)]
pub enum SourceTree<'a> {
    /// A directory on this machine, walked recursively.
    Directory(&'a Path),
    /// Entries already in memory.
    Entries(&'a [SourceEntry]),
}

impl SourceTree<'_> {
    /// Every relative path in the tree, sorted.
    ///
    /// A symbolic link is skipped rather than followed. A link can point
    /// outside the root, and a snapshot that read through one would hold bytes
    /// the gate never classified because the gate classifies the link's path.
    ///
    /// # Errors
    ///
    /// [`RepositoryError::UnreadablePath`] when a directory cannot be listed,
    /// and [`RepositoryError::MalformedPath`] when a path holds a component a
    /// relative forward-slashed path cannot carry.
    pub fn paths(&self) -> Result<Vec<String>, RepositoryError> {
        match self {
            Self::Entries(entries) => {
                let mut found: Vec<String> =
                    entries.iter().map(|entry| entry.path.clone()).collect();
                for path in &found {
                    check_relative(path)?;
                }
                found.sort();
                Ok(found)
            }
            Self::Directory(root) => {
                let mut found = Vec::new();
                walk(root, root, &mut found)?;
                found.sort();
                Ok(found)
            }
        }
    }

    /// The bytes of one path.
    ///
    /// # Errors
    ///
    /// [`RepositoryError::UnreadablePath`] when the path is not in the tree or
    /// cannot be read.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, RepositoryError> {
        check_relative(path)?;
        match self {
            Self::Entries(entries) => entries
                .iter()
                .find(|entry| entry.path == path)
                .map(|entry| entry.source_bytes.clone())
                .ok_or_else(|| RepositoryError::UnreadablePath(path.to_owned())),
            Self::Directory(root) => {
                let mut absolute = PathBuf::from(root);
                for segment in path.split('/') {
                    absolute.push(segment);
                }
                fs::read(&absolute).map_err(|_| RepositoryError::UnreadablePath(path.to_owned()))
            }
        }
    }
}

/// Refuses a path that is not relative, forward-slashed and free of `..`.
fn check_relative(path: &str) -> Result<(), RepositoryError> {
    let malformed = path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..");
    if malformed {
        return Err(RepositoryError::MalformedPath(path.to_owned()));
    }
    Ok(())
}

/// Collects every regular file under `directory`, relative to `root`.
fn walk(root: &Path, directory: &Path, found: &mut Vec<String>) -> Result<(), RepositoryError> {
    let entries = fs::read_dir(directory)
        .map_err(|_| RepositoryError::UnreadablePath(directory.to_string_lossy().into_owned()))?;
    for entry in entries {
        let entry = entry.map_err(|_| {
            RepositoryError::UnreadablePath(directory.to_string_lossy().into_owned())
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| RepositoryError::UnreadablePath(path.to_string_lossy().into_owned()))?;
        if metadata.is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            walk(root, &path, found)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| RepositoryError::MalformedPath(path.to_string_lossy().into_owned()))?;
        let mut segments = Vec::new();
        for component in relative.components() {
            match component {
                Component::Normal(segment) => segments.push(
                    segment
                        .to_str()
                        .ok_or_else(|| {
                            RepositoryError::MalformedPath(relative.to_string_lossy().into_owned())
                        })?
                        .to_owned(),
                ),
                _ => {
                    return Err(RepositoryError::MalformedPath(
                        relative.to_string_lossy().into_owned(),
                    ));
                }
            }
        }
        let joined = segments.join("/");
        check_relative(&joined)?;
        found.push(joined);
    }
    Ok(())
}

/// One capture request: the whole of what a snapshot is taken from.
#[derive(Debug)]
pub struct SnapshotRequest<'a> {
    /// Which repository.
    pub repository: RepositoryId,
    /// Which of the eight inputs was named.
    pub source: RepositorySource,
    /// Where the bytes are.
    pub tree: SourceTree<'a>,
    /// What the caller's version control reported.
    pub facts: &'a WorkingTreeFacts,
    /// The allow/deny, `.gitignore` and user-exclusion rules.
    pub policy: &'a PathPolicy,
    /// The caller's recorded capture time in milliseconds.
    pub captured_at: u64,
    /// Snapshots this one follows.
    pub parent_snapshots: Vec<String>,
    /// Submodules and the commits they are pinned to.
    pub submodule_refs: Vec<SubmoduleRef>,
    /// The digest of the analysis policy in force.
    pub analysis_policy_hash: ContentDigest,
    /// The tools and versions taking this snapshot.
    pub tool_versions: Vec<ToolVersion>,
}

impl SnapshotRequest<'_> {
    /// A digest over what identifies this request.
    ///
    /// The admission the gate returns carries it, and the inventory compares
    /// it, so an admission decided for one request cannot admit another. The
    /// tree's *contents* are deliberately not in it: what it identifies is the
    /// request, and a gate decision is about the bytes the gate itself read.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let mut preimage = b"academic-repository-request-v1\0".to_vec();
        preimage.extend_from_slice(self.repository.as_str().as_bytes());
        preimage.push(0);
        preimage.extend_from_slice(self.source.as_str().as_bytes());
        preimage.push(0);
        preimage.extend_from_slice(&self.captured_at.to_be_bytes());
        preimage.extend_from_slice(
            self.facts
                .head()
                .map_or("nohead", CommitId::as_str)
                .as_bytes(),
        );
        preimage.push(0);
        preimage.extend_from_slice(self.analysis_policy_hash.as_str().as_bytes());
        ContentDigest::of(&preimage)
    }
}

/// What the indexer recorded about one snapshot.
///
/// `P2-R2` owns the index itself; what this crate fixes is that producing one
/// requires a frozen snapshot and therefore a completed gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReceipt {
    snapshot_id: String,
    indexed_paths: usize,
    languages: BTreeMap<&'static str, usize>,
}

impl IndexReceipt {
    /// Which snapshot was indexed.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// How many manifest rows the index covered.
    #[must_use]
    pub const fn indexed_paths(&self) -> usize {
        self.indexed_paths
    }

    /// Rows per language.
    #[must_use]
    pub const fn languages(&self) -> &BTreeMap<&'static str, usize> {
        &self.languages
    }
}

/// The read-only analysis entry point.
///
/// It takes a frozen snapshot by reference and returns a value. There is no
/// path in the argument, no handle, and no closure: an analysis cannot reach
/// the tree the snapshot was taken from, because the snapshot does not carry a
/// way back to it.
///
/// `P2-R2` owns real static analysis. What this function is, is the seam at
/// which the read-only argument type is fixed.
#[must_use]
pub fn analyze(snapshot: &RepositorySnapshot) -> IndexReceipt {
    let mut languages: BTreeMap<&'static str, usize> = BTreeMap::new();
    for entry in snapshot.manifest() {
        *languages.entry(entry.language().as_str()).or_insert(0) += 1;
    }
    IndexReceipt {
        snapshot_id: snapshot.snapshot_id().to_owned(),
        indexed_paths: snapshot.manifest().len(),
        languages,
    }
}

/// What one capture produced.
#[derive(Debug)]
pub struct Capture {
    /// The frozen snapshot.
    pub snapshot: RepositorySnapshot,
    /// What the indexer recorded.
    pub receipt: IndexReceipt,
}

/// Section 17.3's four stages, as one seam.
///
/// The trait is public so a caller can observe or substitute a stage; what it
/// cannot do is skip one, because [`AdmittedPaths`] and [`Inventory`] are only
/// constructible inside this crate. An implementation written elsewhere can
/// only obtain them by calling [`LocalStages`], which is the gate.
pub trait SnapshotStages {
    /// Section 17.3's first stage.
    ///
    /// # Errors
    ///
    /// Whatever reading the tree reports.
    fn permission_and_secret_gate(
        &mut self,
        request: &SnapshotRequest<'_>,
    ) -> Result<AdmittedPaths, RepositoryError>;

    /// Section 17.3's second stage, over exactly what the gate admitted.
    ///
    /// # Errors
    ///
    /// Whatever reading the tree reports, and
    /// [`RepositoryError::AdmissionMismatch`] when the admission names another
    /// request.
    fn inventory(
        &mut self,
        request: &SnapshotRequest<'_>,
        admitted: &AdmittedPaths,
    ) -> Result<Inventory, RepositoryError>;

    /// Freezes the inventory into a snapshot.
    ///
    /// # Errors
    ///
    /// Whatever the identity construction reports.
    fn freeze(
        &mut self,
        request: &SnapshotRequest<'_>,
        listed: Inventory,
    ) -> Result<RepositorySnapshot, RepositoryError>;

    /// Section 17.3's third stage.
    ///
    /// # Errors
    ///
    /// Whatever the indexer reports.
    fn index(&mut self, snapshot: &RepositorySnapshot) -> Result<IndexReceipt, RepositoryError>;
}

/// The stages, as this crate implements them.
#[derive(Debug, Default)]
pub struct LocalStages {
    documents: SourceIndex,
    ingest_seq: u64,
}

impl LocalStages {
    /// A fresh set of stages.
    #[must_use]
    pub fn new() -> Self {
        Self {
            documents: SourceIndex::new(),
            ingest_seq: 0,
        }
    }

    /// The documents sealed so far.
    #[must_use]
    pub const fn documents(&self) -> &SourceIndex {
        &self.documents
    }

    /// Takes the sealed documents out.
    #[must_use]
    pub fn into_documents(self) -> SourceIndex {
        self.documents
    }
}

/// Which `P2-G5` source kind a repository path's bytes are tagged as.
///
/// Three of that crate's six arms apply to a repository: prose is a README,
/// code is what a code comment is lifted out of, and an issue body is an issue.
/// This crate adds no seventh arm — that would be a change to
/// `academic-untrusted-content`'s closed enum and to every `match` over it —
/// so a source file is tagged `CODE_COMMENT`, which is the arm that crate fixed
/// for bytes that came out of source code.
#[must_use]
fn source_kind_of(language: Language) -> SourceKind {
    match language {
        Language::Rust
        | Language::TypeScript
        | Language::Python
        | Language::Sql
        | Language::Configuration => SourceKind::CodeComment,
        Language::Markdown | Language::Unknown => SourceKind::Readme,
    }
}

/// A `SourceId` for one path: `p` and the first 32 hex digits of its digest.
///
/// A path holds `/` and can exceed 64 bytes, and `SourceId` accepts neither, so
/// the identifier is derived rather than taken. It is a function of the path
/// alone, so two captures of the same tree produce the same identifiers.
fn source_id_for(path: &str) -> Result<SourceId, RepositoryError> {
    let digest = ContentDigest::of(path.as_bytes());
    let short = digest
        .as_str()
        .get(..32)
        .ok_or_else(|| RepositoryError::MalformedPath(path.to_owned()))?;
    SourceId::new(format!("p{short}")).map_err(|_| RepositoryError::MalformedPath(path.to_owned()))
}

impl SnapshotStages for LocalStages {
    fn permission_and_secret_gate(
        &mut self,
        request: &SnapshotRequest<'_>,
    ) -> Result<AdmittedPaths, RepositoryError> {
        let paths = request.tree.paths()?;
        let tree = &request.tree;
        let mut read = |path: &str| tree.read(path);
        gate::run_gate(request.digest(), request.policy, &paths, &mut read)
    }

    fn inventory(
        &mut self,
        request: &SnapshotRequest<'_>,
        admitted: &AdmittedPaths,
    ) -> Result<Inventory, RepositoryError> {
        if admitted.request_digest() != &request.digest() {
            return Err(RepositoryError::AdmissionMismatch);
        }
        if admitted.result() == SecretScanResult::Blocked {
            return Err(RepositoryError::SecretGateBlocked);
        }
        let modified: Vec<&str> = request
            .facts
            .modified()
            .iter()
            .map(String::as_str)
            .collect();
        let untracked: Vec<&str> = request
            .facts
            .untracked()
            .iter()
            .map(String::as_str)
            .collect();

        let mut entries = Vec::new();
        let mut dirty_tracked = Vec::new();
        let mut dirty_untracked = Vec::new();
        let mut read_paths = Vec::new();
        for path in admitted.admitted() {
            let bytes = request.tree.read(path)?;
            read_paths.push(path.clone());
            let entry = snapshot::manifest_entry(path, &bytes);
            if modified.contains(&path.as_str()) {
                dirty_tracked.push(entry.clone());
            } else if untracked.contains(&path.as_str()) {
                dirty_untracked.push(entry.clone());
            }
            if !admitted.opaque().contains(path) {
                let document = ingest(
                    source_id_for(path)?,
                    source_kind_of(entry.language()),
                    self.ingest_seq,
                    &bytes,
                )
                .map_err(|_| RepositoryError::UnreadablePath(path.clone()))?;
                self.ingest_seq = self.ingest_seq.saturating_add(1);
                self.documents
                    .insert(document)
                    .map_err(|_| RepositoryError::MalformedPath(path.clone()))?;
            }
            entries.push(entry);
        }
        Ok(Inventory::of(
            admitted,
            entries,
            dirty_tracked,
            dirty_untracked,
            read_paths,
        ))
    }

    fn freeze(
        &mut self,
        request: &SnapshotRequest<'_>,
        listed: Inventory,
    ) -> Result<RepositorySnapshot, RepositoryError> {
        let identity = SnapshotIdentity {
            repository: request.repository.clone(),
            source: request.source,
            snapshot_type: source::resolve_snapshot_type(request.source, request.facts),
            branch: request.facts.branch().map(str::to_owned),
            commit: request.facts.head().cloned(),
            parent_snapshots: request.parent_snapshots.clone(),
            captured_at: request.captured_at,
            submodule_refs: request.submodule_refs.clone(),
            analysis_policy_hash: request.analysis_policy_hash.clone(),
            tool_versions: request.tool_versions.clone(),
        };
        Ok(RepositorySnapshot::freeze(identity, listed))
    }

    fn index(&mut self, snapshot: &RepositorySnapshot) -> Result<IndexReceipt, RepositoryError> {
        Ok(analyze(snapshot))
    }
}

/// The one driver of [`SnapshotStages`]. Section 17.3's order, entire.
///
/// The gate runs first and its result decides whether anything else runs at
/// all: a blocked source returns before the inventory, so no path is opened for
/// a manifest and the indexer is never reached.
///
/// `crates/repository/tests/repository_scans.rs` pins this function as whole
/// text and counts each stage's call sites across the package, because a pin on
/// a body says nothing about whether a second body exists beside it.
///
/// # Errors
///
/// [`RepositoryError::SecretGateBlocked`] when the content scan found
/// something, and whatever a stage reports.
pub fn capture<S: SnapshotStages + ?Sized>(
    stages: &mut S,
    request: &SnapshotRequest<'_>,
) -> Result<Capture, RepositoryError> {
    let admitted = stages.permission_and_secret_gate(request)?;
    if admitted.result() == SecretScanResult::Blocked {
        return Err(RepositoryError::SecretGateBlocked);
    }
    let listed = stages.inventory(request, &admitted)?;
    let snapshot = stages.freeze(request, listed)?;
    let receipt = stages.index(&snapshot)?;
    Ok(Capture { snapshot, receipt })
}

/// Captures with [`LocalStages`], returning the documents it sealed beside it.
///
/// [`capture`] is generic over the stage set and knows nothing about where a
/// substituted one keeps its documents; this is the concrete path, and it is
/// the one every acceptance test drives.
///
/// # Errors
///
/// As [`capture`].
pub fn capture_local(
    request: &SnapshotRequest<'_>,
) -> Result<(Capture, SourceIndex), RepositoryError> {
    let mut stages = LocalStages::new();
    let captured = capture(&mut stages, request)?;
    Ok((captured, stages.into_documents()))
}

/// The number of documents an index holds, for callers outside this crate.
///
/// `SourceIndex` is `P2-G5`'s type and its accessors are that crate's; this is
/// here so a test can count without naming `Untrusted` at all.
#[must_use]
pub fn sealed_document_count(sealed: &SourceIndex) -> usize {
    sealed.len()
}

/// The sealed documents, for a caller that wants their provenance.
#[must_use]
pub fn sealed_documents(sealed: &SourceIndex) -> &[Untrusted<IngestedDocument>] {
    sealed.documents()
}
