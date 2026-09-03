//! The eight kinds of input and the four kinds of snapshot they resolve to.
//!
//! Section 17.1 of the authoritative specification names eight things a user can
//! point this system at; section 17.2's `sourceType` field has four values. The
//! two lists are different vocabularies and this module keeps them apart: one is
//! *how the repository was named*, the other is *what the resulting snapshot is
//! of*. Collapsing them is what would let a dirty working tree be recorded as
//! the commit it was checked out from.

/// One of the eight inputs section 17.1 names.
///
/// The list is exhaustive and [`RepositorySource::ALL`] is the order every
/// enumeration in this crate's tests walks. There is no `Other` arm: a ninth
/// kind of input is an edit here and to every `match` below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RepositorySource {
    /// A directory on this machine.
    LocalDirectory,
    /// A public GitHub repository.
    GitHubPublic,
    /// A private GitHub repository.
    GitHubPrivate,
    /// An archive file holding a project tree.
    Archive,
    /// A named branch of a repository.
    Branch,
    /// A named commit of a repository.
    Commit,
    /// The working tree as it stands, which is not its HEAD.
    DirtyWorktree,
    /// A project that carries specifications and no code.
    SpecOnly,
}

impl RepositorySource {
    /// Exhaustive input order.
    pub const ALL: [Self; 8] = [
        Self::LocalDirectory,
        Self::GitHubPublic,
        Self::GitHubPrivate,
        Self::Archive,
        Self::Branch,
        Self::Commit,
        Self::DirtyWorktree,
        Self::SpecOnly,
    ];

    /// Stable spelling, recorded in the snapshot row.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalDirectory => "LOCAL_DIRECTORY",
            Self::GitHubPublic => "GITHUB_PUBLIC",
            Self::GitHubPrivate => "GITHUB_PRIVATE",
            Self::Archive => "ARCHIVE",
            Self::Branch => "BRANCH",
            Self::Commit => "COMMIT",
            Self::DirtyWorktree => "DIRTY_WORKTREE",
            Self::SpecOnly => "SPEC_ONLY",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|source| source.as_str() == value)
    }

    /// Whether reaching this input needs a GitHub credential.
    ///
    /// Both GitHub arms do; a branch or a commit is named inside a repository
    /// this system already has locally, and the other four are local by
    /// construction.
    #[must_use]
    pub const fn needs_github_credential(self) -> bool {
        matches!(self, Self::GitHubPublic | Self::GitHubPrivate)
    }
}

/// Section 17.2's `sourceType`: what the frozen snapshot is a snapshot *of*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnapshotType {
    /// Exactly the tree of one commit.
    GitCommit,
    /// A working tree that differs from its HEAD.
    DirtyWorktree,
    /// The contents of an archive.
    Archive,
    /// A specification-only project with no committed code tree.
    SpecOnly,
}

impl SnapshotType {
    /// Exhaustive snapshot-type order.
    pub const ALL: [Self; 4] = [
        Self::GitCommit,
        Self::DirtyWorktree,
        Self::Archive,
        Self::SpecOnly,
    ];

    /// Stable spelling, which is the one section 17.2 writes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitCommit => "GIT_COMMIT",
            Self::DirtyWorktree => "DIRTY_WORKTREE",
            Self::Archive => "ARCHIVE",
            Self::SpecOnly => "SPEC_ONLY",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// A commit identifier, as the caller's version control reported it.
///
/// This crate reads no object database. What it holds is the identifier its
/// caller supplied, restricted to lowercase hexadecimal so it cannot itself
/// carry a path or a directive into a manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitId {
    identifier: String,
}

/// Why a commit identifier was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CommitIdError {
    /// The identifier was not 7..=64 characters.
    #[error("a commit identifier is 7..=64 characters")]
    Length,
    /// The identifier held something other than lowercase hexadecimal.
    #[error("a commit identifier holds only lowercase hexadecimal digits")]
    Charset,
}

impl CommitId {
    /// Validates and takes a commit identifier.
    ///
    /// # Errors
    ///
    /// [`CommitIdError`] when the identifier is outside 7..=64 characters or
    /// holds a character outside the lowercase hexadecimal digits.
    pub fn new(value: impl Into<String>) -> Result<Self, CommitIdError> {
        let value = value.into();
        if value.len() < 7 || value.len() > 64 {
            return Err(CommitIdError::Length);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(CommitIdError::Charset);
        }
        Ok(Self { identifier: value })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// What the caller's version control reported about the working tree.
///
/// This crate runs no version-control command and reads no `.git` directory.
/// The connector that has that authority hands these facts in, and everything
/// this crate concludes about tracked, untracked and dirty state is a function
/// of this value and the bytes under the root.
///
/// The path lists are relative and forward-slashed. A path is tracked or
/// untracked and never both; `modified` is a subset of `tracked`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingTreeFacts {
    head: Option<CommitId>,
    branch: Option<String>,
    tracked: Vec<String>,
    modified: Vec<String>,
    untracked: Vec<String>,
}

impl WorkingTreeFacts {
    /// The facts for a source with no version control at all.
    ///
    /// An archive and a specification-only project both use this: no HEAD, no
    /// branch, nothing tracked, and therefore nothing dirty.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            head: None,
            branch: None,
            tracked: Vec::new(),
            modified: Vec::new(),
            untracked: Vec::new(),
        }
    }

    /// Facts for a checkout at `head` on `branch` with the given path lists.
    #[must_use]
    pub const fn checkout(
        head: CommitId,
        branch: Option<String>,
        tracked: Vec<String>,
        modified: Vec<String>,
        untracked: Vec<String>,
    ) -> Self {
        Self {
            head: Some(head),
            branch,
            tracked,
            modified,
            untracked,
        }
    }

    /// The commit the working tree was checked out from, when there is one.
    #[must_use]
    pub const fn head(&self) -> Option<&CommitId> {
        self.head.as_ref()
    }

    /// The branch name, when the caller reported one.
    #[must_use]
    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    /// Paths version control tracks.
    #[must_use]
    pub fn tracked(&self) -> &[String] {
        &self.tracked
    }

    /// Tracked paths whose working-tree bytes differ from the commit's.
    #[must_use]
    pub fn modified(&self) -> &[String] {
        &self.modified
    }

    /// Paths present in the working tree and not tracked.
    #[must_use]
    pub fn untracked(&self) -> &[String] {
        &self.untracked
    }

    /// Whether the working tree differs from its HEAD.
    ///
    /// Either a tracked file changed or an untracked one is present. An
    /// untracked path the gate later excludes is still a difference: the
    /// exclusion decides what an analyzer may read, not whether the tree is the
    /// commit.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        !self.modified.is_empty() || !self.untracked.is_empty()
    }
}

/// The one derivation of a snapshot type from a request and its facts.
///
/// Two of the eight inputs fix the answer because they name something that is
/// not a commit tree at all. For the other six the answer is the tree's rather
/// than the request's: a dirty working tree resolves to
/// [`SnapshotType::DirtyWorktree`] however it was named, which is section 17.2's
/// rule that a dirty working tree is not implicitly identified with HEAD,
/// written as code rather than as prose.
///
/// This function has one call site and
/// `crates/repository/tests/repository_scans.rs` counts it.
#[must_use]
pub(crate) fn resolve_snapshot_type(
    source: RepositorySource,
    facts: &WorkingTreeFacts,
) -> SnapshotType {
    match source {
        RepositorySource::Archive => SnapshotType::Archive,
        RepositorySource::SpecOnly => SnapshotType::SpecOnly,
        RepositorySource::LocalDirectory
        | RepositorySource::GitHubPublic
        | RepositorySource::GitHubPrivate
        | RepositorySource::Branch
        | RepositorySource::Commit
        | RepositorySource::DirtyWorktree => {
            if facts.is_dirty() {
                SnapshotType::DirtyWorktree
            } else {
                SnapshotType::GitCommit
            }
        }
    }
}
