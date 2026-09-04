//! The IDE adapter: local symbol context, deep links, opt-in watching, and no
//! writes.
//!
//! Section 33's IDE row keeps a question, a finding and an evidence locator, and
//! its boundary column is two sentences: there is no write action, and the
//! changed scope is confirmed before a snapshot.
//!
//! ## How "no writes" is established
//!
//! [`IdeWorkspace`] is the whole seam between this crate and an editor, and it
//! has three methods. Each takes `&self` and returns an owned value, so there is
//! no argument through which a caller could hand a mutation in and no `&mut`
//! through which one could be made. `ide_adapter_performs_no_writes` compares
//! the trait's whole method set against a pinned inventory in both directions,
//! and separately requires the whole set of `fs::` names this crate's product
//! source spells to be **empty** -- so a write reached without the trait needs
//! either a fourth method or a filesystem call, and both are extra keys rather
//! than names on a list.
//!
//! The runtime half is a recording workspace whose counters are read after a
//! full adapter session. What that adds to the structural half is that the
//! three methods are the three that actually ran.
//!
//! ## Why a confirmation carries a digest rather than a flag
//!
//! A boolean "the user confirmed" is true forever once it is set, and the thing
//! being confirmed is a *set of paths that is still changing*. So
//! [`ScopeConfirmation::record`] takes the [`ChangedScope`] itself and stores
//! its digest, and [`IdeAdapter::request_snapshot`] compares the digest of the
//! scope it is given with the digest the confirmation carries. A file changed
//! between the confirmation and the snapshot moves the digest and the snapshot
//! is refused -- which is the same binding `P2-R1` uses when an admission
//! carries the digest of the request it was decided for.

use academic_domain::{ContentDigest, TimestampMillis};
use sha2::{Digest as _, Sha256};

/// Why an adapter call was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IdeError {
    /// A workspace path was empty, absolute, or held a parent segment.
    #[error("a workspace path is relative, non-empty, and holds no '..' segment")]
    MalformedPath,
    /// File watching was not opted into, so there is no changed scope to read.
    #[error("file watching is opt-in and this adapter has not opted in")]
    WatchNotOptedIn,
    /// The confirmed scope is not the scope being snapshotted.
    #[error("the confirmed scope is not the scope this snapshot would take")]
    ScopeChanged,
    /// A symbol range was empty or backwards.
    #[error("a symbol range is a non-empty half-open interval")]
    MalformedRange,
}

/// One path inside the editor's workspace.
///
/// Relative, forward-slashed, and holding no parent segment. It is a value
/// rather than a handle: nothing here can be opened.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspacePath(String);

impl WorkspacePath {
    /// Validates and takes a relative path.
    ///
    /// # Errors
    ///
    /// [`IdeError::MalformedPath`] when the value is empty, starts with `/`,
    /// holds a backslash, or holds a `..` segment.
    pub fn new(value: impl Into<String>) -> Result<Self, IdeError> {
        let value = value.into();
        if value.is_empty() || value.starts_with('/') || value.contains('\\') {
            return Err(IdeError::MalformedPath);
        }
        if value.split('/').any(|segment| segment == "..") {
            return Err(IdeError::MalformedPath);
        }
        Ok(Self(value))
    }

    /// The path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One symbol the editor located, as a half-open byte range in one file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolRef {
    path: WorkspacePath,
    name: String,
    start: u32,
    end: u32,
}

impl SymbolRef {
    /// Records one located symbol.
    ///
    /// # Errors
    ///
    /// [`IdeError::MalformedRange`] when the interval is empty or backwards.
    pub fn new(
        path: WorkspacePath,
        name: impl Into<String>,
        start: u32,
        end: u32,
    ) -> Result<Self, IdeError> {
        if start >= end {
            return Err(IdeError::MalformedRange);
        }
        Ok(Self {
            path,
            name: name.into(),
            start,
            end,
        })
    }

    /// The file it is in.
    #[must_use]
    pub const fn path(&self) -> &WorkspacePath {
        &self.path
    }

    /// The symbol's name, as the editor reported it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// First byte.
    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// One past the last byte.
    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end
    }
}

/// A link back into the editor, as a value.
///
/// It is a string this system hands to the user's editor and never a command
/// this system runs: nothing in this crate spawns a process or opens a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepLink(String);

impl DeepLink {
    /// The link.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether the adapter watches files.
///
/// Section 33 says the file watcher is opt-in, so the default is
/// [`WatchMode::Disabled`] and [`IdeAdapter::changed_scope`] refuses under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WatchMode {
    /// No file watching. The adapter reads what it is asked for and no more.
    Disabled,
    /// The user opted in.
    OptedIn,
}

impl WatchMode {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Disabled, Self::OptedIn];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "DISABLED",
            Self::OptedIn => "OPTED_IN",
        }
    }
}

/// The set of paths that changed since an instant, and its digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedScope {
    since: TimestampMillis,
    paths: Vec<WorkspacePath>,
    digest: ContentDigest,
}

impl ChangedScope {
    /// Freezes a changed set. Paths are sorted and deduplicated first, so two
    /// readings of one change produce one digest.
    #[must_use]
    pub fn new(since: TimestampMillis, mut paths: Vec<WorkspacePath>) -> Self {
        paths.sort();
        paths.dedup();
        let mut hasher = Sha256::new();
        hasher.update(b"academic-integrations/changed-scope/v1\0");
        hasher.update(since.value().to_be_bytes());
        hasher.update(u64::try_from(paths.len()).unwrap_or(u64::MAX).to_be_bytes());
        for path in &paths {
            let bytes = path.as_str().as_bytes();
            hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
            hasher.update(bytes);
        }
        Self {
            since,
            paths,
            digest: ContentDigest::from_sha256_bytes(hasher.finalize().into()),
        }
    }

    /// The instant the change set is measured from.
    #[must_use]
    pub const fn since(&self) -> TimestampMillis {
        self.since
    }

    /// The changed paths, sorted and deduplicated.
    #[must_use]
    pub fn paths(&self) -> &[WorkspacePath] {
        &self.paths
    }

    /// The digest a confirmation is recorded against.
    #[must_use]
    pub const fn digest(&self) -> ContentDigest {
        self.digest
    }
}

/// The user's confirmation of one exact changed scope.
///
/// It carries the digest of the scope it was recorded for, and there is no
/// constructor that takes a digest directly -- [`ScopeConfirmation::record`]
/// takes the [`ChangedScope`] and reads it. So a confirmation cannot be
/// recorded for a scope nobody computed, and it stops matching the moment the
/// scope changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeConfirmation {
    scope_digest: ContentDigest,
    actor_id: String,
    at: TimestampMillis,
}

impl ScopeConfirmation {
    /// Records that `actor_id` confirmed exactly `scope`.
    #[must_use]
    pub fn record(scope: &ChangedScope, actor_id: impl Into<String>, at: TimestampMillis) -> Self {
        Self {
            scope_digest: scope.digest(),
            actor_id: actor_id.into(),
            at,
        }
    }

    /// The digest of the scope that was confirmed.
    #[must_use]
    pub const fn scope_digest(&self) -> ContentDigest {
        self.scope_digest
    }

    /// Who confirmed it.
    #[must_use]
    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    /// When.
    #[must_use]
    pub const fn at(&self) -> TimestampMillis {
        self.at
    }
}

/// A snapshot the user has confirmed the scope of.
///
/// Private fields and one producer, [`IdeAdapter::request_snapshot`], so a
/// request that skipped the confirmation is not a value that exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRequest {
    scope: ChangedScope,
    confirmed_by: String,
    confirmed_at: TimestampMillis,
}

impl SnapshotRequest {
    /// The exact scope that was confirmed and is being snapshotted.
    #[must_use]
    pub const fn scope(&self) -> &ChangedScope {
        &self.scope
    }

    /// Who confirmed it.
    #[must_use]
    pub fn confirmed_by(&self) -> &str {
        &self.confirmed_by
    }

    /// When.
    #[must_use]
    pub const fn confirmed_at(&self) -> TimestampMillis {
        self.confirmed_at
    }
}

/// Everything this crate may ask an editor for.
///
/// Three methods, `&self` each, owned answers. There is no `write`, no `apply`,
/// no `open` and no handle: `ide_adapter_performs_no_writes` compares this
/// trait's whole method set against a pinned inventory, so a fourth method is a
/// failure whatever it is called.
pub trait IdeWorkspace {
    /// The files the editor currently has open.
    fn open_paths(&self) -> Vec<WorkspacePath>;

    /// The symbols the editor located in one file.
    fn symbols(&self, path: &WorkspacePath) -> Vec<SymbolRef>;

    /// The files that changed since `since`.
    fn changed_paths(&self, since: TimestampMillis) -> Vec<WorkspacePath>;
}

/// The read-only adapter over one editor workspace.
#[derive(Debug)]
pub struct IdeAdapter<'workspace, W: IdeWorkspace + ?Sized> {
    workspace: &'workspace W,
    watch: WatchMode,
}

impl<'workspace, W: IdeWorkspace + ?Sized> IdeAdapter<'workspace, W> {
    /// Attaches to a workspace with file watching off.
    #[must_use]
    pub const fn attach(workspace: &'workspace W) -> Self {
        Self {
            workspace,
            watch: WatchMode::Disabled,
        }
    }

    /// The same adapter with the watch mode replaced.
    #[must_use]
    pub const fn with_watch(mut self, watch: WatchMode) -> Self {
        self.watch = watch;
        self
    }

    /// Whether this adapter watches files.
    #[must_use]
    pub const fn watch(&self) -> WatchMode {
        self.watch
    }

    /// The files the editor has open.
    #[must_use]
    pub fn open_paths(&self) -> Vec<WorkspacePath> {
        self.workspace.open_paths()
    }

    /// The symbols in one file.
    #[must_use]
    pub fn symbols(&self, path: &WorkspacePath) -> Vec<SymbolRef> {
        self.workspace.symbols(path)
    }

    /// A link back into the editor at one symbol.
    #[must_use]
    pub fn deep_link(&self, symbol: &SymbolRef) -> DeepLink {
        DeepLink(format!(
            "ide://open?path={}&start={}&end={}",
            symbol.path().as_str(),
            symbol.start(),
            symbol.end()
        ))
    }

    /// The scope that changed since `since`.
    ///
    /// # Errors
    ///
    /// [`IdeError::WatchNotOptedIn`] when the adapter has not opted in. Section
    /// 33's watcher is opt-in, and the refusal is what makes that a property of
    /// the adapter rather than of whoever calls it.
    pub fn changed_scope(&self, since: TimestampMillis) -> Result<ChangedScope, IdeError> {
        if self.watch != WatchMode::OptedIn {
            return Err(IdeError::WatchNotOptedIn);
        }
        Ok(ChangedScope::new(
            since,
            self.workspace.changed_paths(since),
        ))
    }

    /// Turns a confirmed scope into a snapshot request.
    ///
    /// # Errors
    ///
    /// [`IdeError::ScopeChanged`] when the confirmation was recorded against a
    /// different scope -- which is what a file changed after the confirmation
    /// produces, because the digest covers the whole path set.
    pub fn request_snapshot(
        &self,
        scope: &ChangedScope,
        confirmation: &ScopeConfirmation,
    ) -> Result<SnapshotRequest, IdeError> {
        if confirmation.scope_digest() != scope.digest() {
            return Err(IdeError::ScopeChanged);
        }
        Ok(SnapshotRequest {
            scope: scope.clone(),
            confirmed_by: confirmation.actor_id().to_owned(),
            confirmed_at: confirmation.at(),
        })
    }
}
