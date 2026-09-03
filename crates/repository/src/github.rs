//! Repo-scoped, read-only, short-lived GitHub access, held by the operating
//! system.
//!
//! Sections 29.6 and 32.4 fix three properties of the credential this system
//! uses to reach a GitHub repository, and this module makes each one a separate
//! thing to test:
//!
//! * **Repo-scoped.** [`TokenScope`] names exactly one [`GitHubRepository`] and
//!   [`TokenScope::covers`] is exact equality. There is no wildcard, no owner
//!   scope and no "all repositories" arm.
//! * **Read-only.** [`TokenPermission`] has no write variant. Every arm maps to
//!   [`Access::Read`] through an exhaustive `match`, so a write permission
//!   added later stops `github_token_is_repo_scoped_read_only_and_expiring`
//!   compiling rather than passing.
//! * **Expiring.** [`TokenLifetime::new`] refuses a lifetime that is empty,
//!   backwards, or longer than [`MAX_TOKEN_LIFETIME_MILLIS`], and
//!   [`FineGrainedToken::is_valid_at`] is half-open, so the expiry instant is
//!   already outside.
//!
//! ## Where the material is
//!
//! Nowhere in this crate for longer than one call. [`CredentialStore`] wraps
//! `P2-K1`'s [`DeviceKeystore`] and is the only thing that holds a token: what
//! it hands back to the rest of this crate is a [`SealedCredential`], which is
//! the broker's blob plus the scope and lifetime in the clear.
//! [`CredentialStore::borrow`] is the one call that opens it, and what it
//! returns is a `Zeroizing` buffer inside a value whose `Debug` is hand-written.
//!
//! ## Why nothing here reaches the network
//!
//! [`GitHubRepositoryReader`] is a trait and this crate ships no implementation
//! of it, the way `academic-egress-boundary` ships no transport. Every test in
//! this crate supplies its own in-memory reader. `only_egress_crate_has_a_socket`
//! in `tools/phase1-scaffold-policy.test.mjs` reads that as the absence of a
//! `SOCKET_ALLOWANCE` entry for this package and as a link closure holding
//! nothing that can open one.

use academic_crypto::{DeviceKeystore, KeystoreFailure};
use zeroize::Zeroizing;

use crate::{SourceEntry, source::CommitId};

/// The longest a fine-grained token this system mints may live.
///
/// One hour. Section 32.4 says the connector borrows a token *briefly*; a
/// number is what makes that testable, and the constructor refuses anything
/// longer rather than trusting the caller to pick well.
pub const MAX_TOKEN_LIFETIME_MILLIS: u64 = 60 * 60 * 1000;

/// Why a credential was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GitHubError {
    /// The owner or the repository name was empty or malformed.
    #[error("a repository is owner/name in [A-Za-z0-9._-]")]
    MalformedRepository,
    /// The lifetime was empty, backwards, or longer than the bound.
    #[error("a token lifetime is a non-empty interval no longer than the bound")]
    MalformedLifetime,
    /// The scope named no permission.
    #[error("a token scope names at least one permission")]
    EmptyScope,
    /// The token has expired, or is not yet valid.
    #[error("the token is not valid at this instant")]
    Expired,
    /// The token is scoped to a different repository.
    #[error("the token is scoped to another repository")]
    OutOfScope,
    /// The token does not carry the permission this read needs.
    #[error("the token does not carry the permission this read needs")]
    MissingPermission,
    /// The operating-system credential store refused.
    #[error("the operating-system credential store refused: {0}")]
    Keystore(KeystoreFailure),
}

/// One GitHub repository, as `owner/name`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitHubRepository {
    owner: String,
    name: String,
}

impl GitHubRepository {
    /// Validates an owner and a repository name.
    ///
    /// # Errors
    ///
    /// [`GitHubError::MalformedRepository`] when either half is empty, over 100
    /// bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(owner: impl Into<String>, name: impl Into<String>) -> Result<Self, GitHubError> {
        let owner = owner.into();
        let name = name.into();
        for part in [&owner, &name] {
            if part.is_empty() || part.len() > 100 {
                return Err(GitHubError::MalformedRepository);
            }
            if !part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            {
                return Err(GitHubError::MalformedRepository);
            }
        }
        Ok(Self { owner, name })
    }

    /// The owner.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The repository name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The `owner/name` spelling, which is also the credential-store label.
    #[must_use]
    pub fn as_label(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// What a permission lets a holder do.
///
/// One variant. It exists so [`TokenPermission::access`] is an exhaustive
/// `match` returning a *type* rather than a boolean: a write permission added
/// to [`TokenPermission`] would have no arm to return here, and the crate stops
/// compiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Access {
    /// Read.
    Read,
}

/// The permissions a fine-grained token this system mints may carry.
///
/// Three, and all three are reads. Section 29.6 says "minimal metadata
/// permission"; these are the three a snapshot needs and there is deliberately
/// no fourth, no `Admin`, and no `*Write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenPermission {
    /// Repository metadata: default branch, visibility, HEAD.
    MetadataRead,
    /// File contents at a named commit.
    ContentsRead,
    /// Issue and pull-request bodies, which section 17.1 lists as input.
    IssuesRead,
}

impl TokenPermission {
    /// Exhaustive permission order.
    pub const ALL: [Self; 3] = [Self::MetadataRead, Self::ContentsRead, Self::IssuesRead];

    /// Stable spelling, which is GitHub's own `<resource>:<access>` shape.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataRead => "metadata:read",
            Self::ContentsRead => "contents:read",
            Self::IssuesRead => "issues:read",
        }
    }

    /// What this permission lets a holder do.
    ///
    /// Exhaustive, and every arm is [`Access::Read`]. This is the read-only
    /// claim: it is a total function over the enum rather than a search for
    /// forbidden spellings, so a variant added without an arm is a compile
    /// error.
    #[must_use]
    pub const fn access(self) -> Access {
        match self {
            Self::MetadataRead | Self::ContentsRead | Self::IssuesRead => Access::Read,
        }
    }
}

/// One repository and the permissions a token holds over it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenScope {
    repository: GitHubRepository,
    permissions: Vec<TokenPermission>,
}

impl TokenScope {
    /// Scopes a token to one repository and at least one permission.
    ///
    /// # Errors
    ///
    /// [`GitHubError::EmptyScope`] when the permission list is empty.
    pub fn new(
        repository: GitHubRepository,
        mut permissions: Vec<TokenPermission>,
    ) -> Result<Self, GitHubError> {
        if permissions.is_empty() {
            return Err(GitHubError::EmptyScope);
        }
        permissions.sort();
        permissions.dedup();
        Ok(Self {
            repository,
            permissions,
        })
    }

    /// The one repository.
    #[must_use]
    pub const fn repository(&self) -> &GitHubRepository {
        &self.repository
    }

    /// The permissions, sorted and deduplicated.
    #[must_use]
    pub fn permissions(&self) -> &[TokenPermission] {
        &self.permissions
    }

    /// Whether this scope covers `repository`.
    ///
    /// Exact equality. There is no prefix, no wildcard, and no owner-wide arm,
    /// which is what "repo-scoped" means here.
    #[must_use]
    pub fn covers(&self, repository: &GitHubRepository) -> bool {
        &self.repository == repository
    }
}

/// The half-open interval a token is valid over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenLifetime {
    issued_at: u64,
    expires_at: u64,
}

impl TokenLifetime {
    /// Validates an issue and expiry instant.
    ///
    /// # Errors
    ///
    /// [`GitHubError::MalformedLifetime`] when the interval is empty, runs
    /// backwards, or is longer than [`MAX_TOKEN_LIFETIME_MILLIS`].
    pub const fn new(issued_at: u64, expires_at: u64) -> Result<Self, GitHubError> {
        if expires_at <= issued_at {
            return Err(GitHubError::MalformedLifetime);
        }
        let Some(span) = expires_at.checked_sub(issued_at) else {
            return Err(GitHubError::MalformedLifetime);
        };
        if span > MAX_TOKEN_LIFETIME_MILLIS {
            return Err(GitHubError::MalformedLifetime);
        }
        Ok(Self {
            issued_at,
            expires_at,
        })
    }

    /// When the token was issued.
    #[must_use]
    pub const fn issued_at(&self) -> u64 {
        self.issued_at
    }

    /// When it stops being valid. Half-open: this instant is already outside.
    #[must_use]
    pub const fn expires_at(&self) -> u64 {
        self.expires_at
    }

    /// Whether `now` lies in `[issued_at, expires_at)`.
    #[must_use]
    pub const fn contains(&self, now: u64) -> bool {
        self.issued_at <= now && now < self.expires_at
    }
}

/// A fine-grained token: one scope, one lifetime, and the material.
///
/// The material field is named `secret` deliberately.
/// `tools/secret-debug-policy.test.mjs` classifies a field by its name and its
/// type together, and that name with a byte buffer under it is one the existing
/// net already refuses a derived `Debug` over — so the hand-written `Debug`
/// below is required by a rule this task did not invent and does not widen.
pub struct FineGrainedToken {
    scope: TokenScope,
    lifetime: TokenLifetime,
    secret: Zeroizing<Vec<u8>>,
}

impl core::fmt::Debug for FineGrainedToken {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FineGrainedToken")
            .field("scope", &self.scope)
            .field("lifetime", &self.lifetime)
            .field(
                "secret",
                &format_args!("<redacted:{} bytes>", self.secret.len()),
            )
            .finish()
    }
}

impl FineGrainedToken {
    /// Takes a token's scope, lifetime and material.
    #[must_use]
    pub fn new(scope: TokenScope, lifetime: TokenLifetime, secret: Vec<u8>) -> Self {
        Self {
            scope,
            lifetime,
            secret: Zeroizing::new(secret),
        }
    }

    /// The scope.
    #[must_use]
    pub const fn scope(&self) -> &TokenScope {
        &self.scope
    }

    /// The lifetime.
    #[must_use]
    pub const fn lifetime(&self) -> TokenLifetime {
        self.lifetime
    }

    /// Whether the token is valid at `now`.
    #[must_use]
    pub const fn is_valid_at(&self, now: u64) -> bool {
        self.lifetime.contains(now)
    }

    /// Checks this token against one read of one repository.
    ///
    /// The three properties are checked in a fixed order and each has its own
    /// error, so a refusal says which property failed rather than reporting a
    /// generic denial that a test could satisfy for the wrong reason.
    ///
    /// # Errors
    ///
    /// [`GitHubError::Expired`], [`GitHubError::OutOfScope`], or
    /// [`GitHubError::MissingPermission`].
    pub fn authorize(
        &self,
        repository: &GitHubRepository,
        permission: TokenPermission,
        now: u64,
    ) -> Result<(), GitHubError> {
        if !self.is_valid_at(now) {
            return Err(GitHubError::Expired);
        }
        if !self.scope.covers(repository) {
            return Err(GitHubError::OutOfScope);
        }
        if !self.scope.permissions().contains(&permission) {
            return Err(GitHubError::MissingPermission);
        }
        Ok(())
    }

    /// The material. Crate-private; the only caller is [`CredentialStore::seal`].
    pub(crate) fn material(&self) -> &[u8] {
        &self.secret
    }
}

/// A token as it rests: the operating system's blob plus public metadata.
///
/// The blob is whatever `P2-K1`'s broker returned. It is not the token, and
/// this type holds no field the token's bytes could be read out of — but what
/// it *is* is the other half of recovering one, and it is a byte buffer under a
/// field name `tools/secret-debug-policy.test.mjs` did not hold when this crate
/// was written. That is `S-10`'s shape, and the decision this crate made rather
/// than deferred is below: the `Debug` here is hand-written and prints a
/// length, and `blob` was added to that net's `SECRET_FIELD_NAMES` in the same
/// commit, because the measured cost of widening it by that one name is this
/// one site and this site is already redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct SealedCredential {
    label: String,
    provider: String,
    scope: TokenScope,
    lifetime: TokenLifetime,
    blob: Vec<u8>,
}

impl core::fmt::Debug for SealedCredential {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SealedCredential")
            .field("label", &self.label)
            .field("provider", &self.provider)
            .field("scope", &self.scope)
            .field("lifetime", &self.lifetime)
            .field(
                "blob",
                &format_args!("<redacted:{} bytes>", self.blob.len()),
            )
            .finish()
    }
}

impl SealedCredential {
    /// The credential-store label, which is the repository's `owner/name`.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Which operating-system broker holds it.
    #[must_use]
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// The scope, in the clear. It names a repository and reads; it is not
    /// material.
    #[must_use]
    pub const fn scope(&self) -> &TokenScope {
        &self.scope
    }

    /// The lifetime, in the clear.
    #[must_use]
    pub const fn lifetime(&self) -> TokenLifetime {
        self.lifetime
    }
}

/// The seam between a fine-grained token and the operating system.
///
/// Generic over `P2-K1`'s [`DeviceKeystore`] rather than over a keystore this
/// crate defines, so the reviewed native broker and the in-memory test double
/// are the same two things every other crate in this repository uses.
#[derive(Debug)]
pub struct CredentialStore<K: DeviceKeystore + ?Sized> {
    keystore: K,
}

impl<K: DeviceKeystore> CredentialStore<K> {
    /// Binds a keystore.
    #[must_use]
    pub const fn new(keystore: K) -> Self {
        Self { keystore }
    }

    /// Hands the token's material to the operating system and keeps the blob.
    ///
    /// # Errors
    ///
    /// [`GitHubError::Keystore`] when the broker refuses.
    pub fn seal(&self, token: &FineGrainedToken) -> Result<SealedCredential, GitHubError> {
        let label = token.scope.repository.as_label();
        let blob = self
            .keystore
            .seal(&label, token.material())
            .map_err(GitHubError::Keystore)?;
        Ok(SealedCredential {
            label,
            provider: self.keystore.provider().to_owned(),
            scope: token.scope.clone(),
            lifetime: token.lifetime,
            blob,
        })
    }

    /// Borrows the token back, refusing an expired one before opening it.
    ///
    /// The expiry is checked *first*, so an expired credential is not opened at
    /// all: the material never leaves the broker for a token that could not be
    /// used anyway. That ordering is the reason this is one function rather
    /// than an `open` a caller is trusted to check afterwards.
    ///
    /// # Errors
    ///
    /// [`GitHubError::Expired`] when `now` is outside the lifetime, or
    /// [`GitHubError::Keystore`] when the broker refuses.
    pub fn borrow(
        &self,
        sealed: &SealedCredential,
        now: u64,
    ) -> Result<FineGrainedToken, GitHubError> {
        if !sealed.lifetime.contains(now) {
            return Err(GitHubError::Expired);
        }
        let recovered = self
            .keystore
            .open(&sealed.label, &sealed.blob)
            .map_err(GitHubError::Keystore)?;
        Ok(FineGrainedToken {
            scope: sealed.scope.clone(),
            lifetime: sealed.lifetime,
            secret: Zeroizing::new(recovered.to_vec()),
        })
    }
}

/// Reads a GitHub repository at a commit.
///
/// **No implementation of this trait ships.** It is the contract and the type;
/// every caller in this repository supplies its own in-memory reader, which is
/// the same shape `academic-egress-boundary` uses for its transport. What a
/// real implementation would need — an outbound socket — belongs to
/// `ProcessClass::EgressProxy`, and `ProcessClass::RepositoryAnalyzer` does not
/// hold that capability.
pub trait GitHubRepositoryReader {
    /// Reads the tree at `commit`, having first authorized `token`.
    ///
    /// # Errors
    ///
    /// Whatever the token check refuses, and whatever the reader itself
    /// reports.
    fn read_tree(
        &self,
        token: &FineGrainedToken,
        repository: &GitHubRepository,
        commit: &CommitId,
        now: u64,
    ) -> Result<Vec<SourceEntry>, GitHubError>;
}
