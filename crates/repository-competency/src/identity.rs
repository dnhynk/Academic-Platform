//! Section 17.6's first bullet needs to know whose change it is looking at, and
//! a repository does not say that.
//!
//! What a version-control system records beside a change is an author string —
//! a name and an address a committer chose, or a forge account. Section 33's
//! own rule for every such value is `외부 ID는 canonical ID가 아니라
//! ExternalIdentity mapping으로 저장한다`, so this crate never treats one as the
//! user. [`ExternalAuthorId`] is the outside value, [`UserId`] is this system's
//! own subject, and [`AuthorshipMap`] is the recorded, versioned mapping between
//! them.
//!
//! ## The mapping answers by membership and by nothing else
//!
//! [`AuthorshipMap::resolve`] is a set lookup on the whole pair. It does not
//! case-fold, does not trim, does not compare display names, and does not fall
//! back to the value when the source does not match. An identity the user has
//! not recorded is not the user, which is the direction that fails closed: an
//! unrecorded identity costs a personal claim that could have been made, and a
//! guessed one credits the user with someone else's work.
//!
//! Two things follow that a string comparison would not give:
//!
//! * an address differing only in ASCII case is a different identity, because
//!   the mapping holds what the user recorded rather than what a normalizer
//!   thinks two addresses have in common; and
//! * one string under two [`IdentitySource`]s is two identities, so a forge
//!   login that happens to read like an address resolves to nobody.
//!
//! ## The mapping is versioned
//!
//! A personal claim records which mapping version admitted it, the way a
//! classification records the goal version that produced it. A user who later
//! adds a work address does not silently change what an earlier claim rested
//! on: the earlier claim still names the version it was decided under.

use std::collections::BTreeSet;

use crate::CompetencyError;

/// Whether `value` is an identifier this system may hold and hand back.
///
/// `[A-Za-z0-9._-]` within 64 bytes, which is
/// [`academic_repository_analysis::SubjectId`]'s own shape and the shape
/// `P2-R4` validates a goal identifier against. An external author value is
/// **not** validated against it — see [`ExternalAuthorId`].
pub(crate) fn validated(value: String, field: &'static str) -> Result<String, CompetencyError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(value)
    } else {
        Err(CompetencyError::InvalidIdentifier(field, value))
    }
}

/// This system's own subject: the person the record belongs to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId {
    identifier: String,
}

impl UserId {
    /// Validates and takes a user identifier.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when it is empty, over 64 bytes,
    /// or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self {
            identifier: validated(value.into(), "user")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// Where an external author value came from.
///
/// Part of the identity rather than metadata beside it: the same characters
/// mean different things in different namespaces, and a mapping that ignored
/// the namespace would let a forge login stand in for a commit address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdentitySource {
    /// The `author` field of a commit.
    GitAuthorEmail,
    /// The `committer` field of a commit, which a rebase or a merge rewrites.
    GitCommitterEmail,
    /// An account name at a hosting service.
    ForgeLogin,
}

impl IdentitySource {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [
        Self::GitAuthorEmail,
        Self::GitCommitterEmail,
        Self::ForgeLogin,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitAuthorEmail => "GIT_AUTHOR_EMAIL",
            Self::GitCommitterEmail => "GIT_COMMITTER_EMAIL",
            Self::ForgeLogin => "FORGE_LOGIN",
        }
    }
}

/// One identity as the outside world spells it.
///
/// The value is **not** put through [`validated`]: an address holds `@` and a
/// display name holds spaces, and rejecting those would silently drop the
/// identities this type exists to carry. What is bounded is its length, so a
/// mapping cannot be made to hold an arbitrarily large blob under an identity's
/// name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalAuthorId {
    source: IdentitySource,
    value: String,
}

impl ExternalAuthorId {
    /// Names an identity in one external namespace.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when the value is empty or longer
    /// than 320 bytes, which is the longest address RFC 5321 admits.
    pub fn new(source: IdentitySource, value: impl Into<String>) -> Result<Self, CompetencyError> {
        let value = value.into();
        if value.is_empty() || value.len() > 320 {
            return Err(CompetencyError::InvalidIdentifier("external author", value));
        }
        Ok(Self { source, value })
    }

    /// Which namespace it is in.
    #[must_use]
    pub const fn source(&self) -> IdentitySource {
        self.source
    }

    /// The value, exactly as it was recorded.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// The user's own recorded answer to `which of these outside names are me`.
///
/// One user, one version, and a set. There is no constructor that omits the
/// version and none that omits the user, so a mapping cannot be read as an
/// answer about a person it was not written about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorshipMap {
    user: UserId,
    version: u64,
    identities: BTreeSet<ExternalAuthorId>,
}

impl AuthorshipMap {
    /// Records the identities `user` says are theirs, at `version`.
    #[must_use]
    pub fn of(user: UserId, version: u64, identities: Vec<ExternalAuthorId>) -> Self {
        Self {
            user,
            version,
            identities: identities.into_iter().collect(),
        }
    }

    /// Whose mapping it is.
    #[must_use]
    pub const fn user(&self) -> &UserId {
        &self.user
    }

    /// Which version of it. A personal claim records this.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Every identity it holds, in identity order.
    #[must_use]
    pub fn identities(&self) -> Vec<&ExternalAuthorId> {
        self.identities.iter().collect()
    }

    /// The user this external identity is, when the mapping records one.
    ///
    /// Whole-pair set membership. [`None`] is the answer for every identity the
    /// user has not recorded, including one that differs from a recorded one
    /// only in ASCII case and one that repeats a recorded value under another
    /// [`IdentitySource`].
    #[must_use]
    pub fn resolve(&self, external: &ExternalAuthorId) -> Option<&UserId> {
        if self.identities.contains(external) {
            Some(&self.user)
        } else {
            None
        }
    }
}
