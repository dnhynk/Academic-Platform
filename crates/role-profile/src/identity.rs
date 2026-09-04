//! The identity of a versioned bundle, and why it is a pair rather than a name.
//!
//! ## Section 24.2's `id` is rendered, not stored
//!
//! The specification's example writes `id: backend_engineer_profile_v4`. That
//! spelling folds two different things — which lineage, and which version of it
//! — into one string, and `P2-R4` measured what that costs one stage over: a
//! classification key built by joining several values and truncating the join
//! collided two findings whose *goal version alone* differed, and `P2-A1` had
//! already caught the same shape as a P1 defect.
//!
//! So the identity here is [`RoleProfileRef`], the **ordered pair** of a
//! [`RoleProfileId`] and a [`RoleProfileVersion`], compared field by field.
//! [`RoleProfileRef::rendered`] produces section 24.2's spelling for a reader,
//! and it is a display-side function with no inverse: there is no parser from
//! that string back to a pair, and no `From` or `TryFrom` between the rendered
//! text and any type here. `an_identity_is_a_pair_and_not_a_rendered_name`
//! exhibits two bundles that render the same string and are not equal, which is
//! the collision the pair cannot have.
//!
//! ## The version's shape is the predicate registry's, not this crate's
//!
//! Section 7.2's `RELEVANT_TO_ROLE` row carries exactly one qualifier,
//! `role_profile_version`, and its kind is `PositiveInteger` and it is
//! required. [`RoleProfileVersion`] is that qualifier: a non-zero `u32`, with
//! zero refused at every door including deserialization.
//! `the_version_qualifier_is_the_registry_s` reads the descriptor and compares
//! both the qualifier set and that kind in both directions, so a registry that
//! grows a second qualifier — an importance, say — fails this crate rather than
//! passing unnoticed.
//!
//! ## An identifier is classified byte by byte over the whole value
//!
//! [`validated`] admits `[A-Za-z0-9._-]` within [`MAX_IDENTIFIER`] bytes by
//! classifying **every** byte, not by searching for listed characters. `P2-Y1`
//! measured why: the same rule stated as a comment in its own `validated` was
//! never executed, and thirty-one injections found exactly one that failed —
//! the one that removed the check. A name list admits any byte nobody thought
//! to list.

use core::{fmt, num::NonZeroU32};

use academic_domain::predicates::{PredicateName, QualifierKind};
use serde::{Deserialize, Serialize};

use crate::RoleError;

/// Longest identifier this crate admits, in bytes.
pub const MAX_IDENTIFIER: usize = 64;

/// Section 7.2's qualifier key that carries a bundle's version.
pub const VERSION_QUALIFIER: &str = "role_profile_version";

/// Checks one identifier against `[A-Za-z0-9._-]` within [`MAX_IDENTIFIER`].
///
/// Every byte is classified. Nothing here searches for a listed character, so a
/// byte nobody enumerated is refused because it was not admitted rather than
/// admitted because it was not listed.
pub(crate) fn validated(value: String, what: &'static str) -> Result<String, RoleError> {
    let legal = !value.is_empty()
        && value.len() <= MAX_IDENTIFIER
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if legal {
        Ok(value)
    } else {
        Err(RoleError::InvalidIdentifier(what, value))
    }
}

/// Checks one prose field for emptiness.
///
/// A label, a scope and a source citation are the user's own words, so they are
/// not identifiers and are not reshaped. What is refused is the empty one,
/// because a bundle whose source nobody wrote has no recorded source, and
/// `GATE-38-029` stays open on the strength of the recording.
pub(crate) fn non_empty(value: String, what: &'static str) -> Result<String, RoleError> {
    if value.trim().is_empty() {
        Err(RoleError::EmptyText(what))
    } else {
        Ok(value)
    }
}

/// One bundle lineage's identity.
///
/// **Not a version, and not a label.** This is the half of section 24.2's `id`
/// that stays the same when the bundle is revised: revising
/// `backend_engineer_profile` produces another `backend_engineer_profile` at
/// the next version, and forking it produces a different value here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RoleProfileId(String);

impl RoleProfileId {
    /// Checks and takes one identifier.
    ///
    /// # Errors
    ///
    /// [`RoleError::InvalidIdentifier`] when it is not `[A-Za-z0-9._-]` within
    /// 64 bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, RoleError> {
        Ok(Self(validated(value.into(), "role profile")?))
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RoleProfileId {
    type Error = RoleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<RoleProfileId> for String {
    fn from(value: RoleProfileId) -> Self {
        value.0
    }
}

/// Section 7.2's `role_profile_version` qualifier.
///
/// A positive integer, which is the registry's own `QualifierKind` for that
/// key. Zero is not a version: a bundle exists from its first publication, and
/// [`RoleProfileVersion::FIRST`] is what [`crate::declare`] starts at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct RoleProfileVersion(NonZeroU32);

impl RoleProfileVersion {
    /// The version a newly declared bundle carries.
    pub const FIRST: Self = Self(NonZeroU32::MIN);

    /// Checks and takes one version.
    ///
    /// # Errors
    ///
    /// [`RoleError::VersionIsNotPositive`] for zero.
    pub const fn new(value: u32) -> Result<Self, RoleError> {
        match NonZeroU32::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(RoleError::VersionIsNotPositive),
        }
    }

    /// The version as a number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// The version after this one.
    ///
    /// # Errors
    ///
    /// [`RoleError::VersionWouldOverflow`] at the top of the range, which is
    /// refused rather than wrapped: a bundle that wrapped to one would claim to
    /// be the first version of its own lineage.
    pub const fn next(self) -> Result<Self, RoleError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(RoleError::VersionWouldOverflow),
        }
    }

    /// The section 7.2 qualifier key a `RELEVANT_TO_ROLE` edge carries this
    /// under, from the shared registry.
    #[must_use]
    pub const fn qualifier_key() -> &'static str {
        VERSION_QUALIFIER
    }

    /// The registry's own kind for that key.
    ///
    /// Read from `PredicateName::RelevantToRole`'s descriptor, so this is the
    /// registry's answer rather than a second statement of it.
    #[must_use]
    pub fn registry_kind() -> Option<QualifierKind> {
        PredicateName::RelevantToRole
            .descriptor()
            .qualifiers
            .iter()
            .find(|schema| schema.key == VERSION_QUALIFIER)
            .map(|schema| schema.kind)
    }
}

impl TryFrom<u32> for RoleProfileVersion {
    type Error = RoleError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<RoleProfileVersion> for u32 {
    fn from(value: RoleProfileVersion) -> Self {
        value.0.get()
    }
}

/// One version of one bundle, by identity.
///
/// The pair is the identity. Equality, ordering and hashing are over both
/// fields, so `backend_engineer_profile` at version four and
/// `backend_engineer_profile_v4` at version one are two values however section
/// 24.2's `id` spelling renders them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RoleProfileRef {
    profile: RoleProfileId,
    version: RoleProfileVersion,
}

impl RoleProfileRef {
    /// Names one version of one lineage.
    #[must_use]
    pub const fn of(profile: RoleProfileId, version: RoleProfileVersion) -> Self {
        Self { profile, version }
    }

    /// Which lineage.
    #[must_use]
    pub const fn profile(&self) -> &RoleProfileId {
        &self.profile
    }

    /// Which version of it.
    #[must_use]
    pub const fn version(&self) -> RoleProfileVersion {
        self.version
    }

    /// Section 24.2's `id` spelling, for a reader.
    ///
    /// **Display only.** There is no parser back: two different pairs may
    /// render the same text, which is precisely why the pair and not this
    /// string is the identity.
    #[must_use]
    pub fn rendered(&self) -> String {
        format!("{}_v{}", self.profile.as_str(), self.version.get())
    }
}

impl fmt::Display for RoleProfileRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.rendered())
    }
}

/// The user's display words for a bundle: section 24.2's `label`.
///
/// **Not an identity and not a direction.** `Backend Engineer` is what the user
/// wrote on this bundle; two organisations may write it on bundles that agree
/// about nothing. Nothing in this crate reads a label to decide anything —
/// there is no function from a label to a [`crate::RoleDirection`], to a
/// competency, or to a single bundle — because section 24.2's constraint is
/// `role 이름을 시장의 단일 진리로 두지 않는다`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RoleLabel(String);

impl RoleLabel {
    /// Takes one label.
    ///
    /// # Errors
    ///
    /// [`RoleError::EmptyText`] when it carries nothing.
    pub fn new(value: impl Into<String>) -> Result<Self, RoleError> {
        Ok(Self(non_empty(value.into(), "label")?))
    }

    /// The label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RoleLabel {
    type Error = RoleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<RoleLabel> for String {
    fn from(value: RoleLabel) -> Self {
        value.0
    }
}
