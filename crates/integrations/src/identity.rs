//! `ExternalIdentity`: a mapping, and never a canonical identifier.
//!
//! Section 33's last paragraph fixes two rules in one sentence. An external
//! identifier is stored as an `ExternalIdentity` mapping rather than as a
//! canonical identifier, and a sync conflict is resolved by source authority
//! and valid time without either side being silently overwritten.
//!
//! ## Why the first is a type rather than a rule
//!
//! `P2-R4` and `P2-Y2` both measured what happens when an identity is *folded*
//! into the thing it names: two sources that disagree collide, and the record
//! keeps whichever one was written last. The defence here is that there is no
//! function that takes an [`ExternalId`] and produces a [`CanonicalRef`].
//!
//! [`CanonicalRef`] has one constructor per arm and each takes a
//! `academic_domain` identifier **by value**. Those identifiers are UUIDv7 and
//! their own constructors refuse anything else, so a provider's opaque string
//! cannot become one by parsing; and nothing in this crate holds a
//! `DomainError` path that would let it try. `external_id_is_never_canonical`
//! reads the whole public signature set of this crate and requires every
//! function that *returns* a canonical reference to have received one -- or a
//! stored mapping holding one -- as an argument. A conversion added later
//! appears as a signature that returns a canonical and took only text, which is
//! a classification over the whole set rather than a list of forbidden names.
//!
//! `crates/integrations/tests/compile_fail/` holds the type half: a
//! `CanonicalRef` built from a string, from an `ExternalId`, and by struct
//! literal are three compiler diagnostics.

use std::collections::BTreeMap;

use academic_domain::{ArtifactId, CourseId, EntityId, OfferingId, RepositoryId, TimestampMillis};

use crate::ConnectorKind;

/// Why an identity was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IdentityError {
    /// The external identifier was empty or longer than the bound.
    #[error("an external identifier is 1..=200 bytes")]
    Length,
    /// The external identifier held a byte no provider identifier uses.
    #[error("an external identifier holds only printable ASCII without whitespace")]
    Charset,
}

/// The longest external identifier this system stores.
pub const MAX_EXTERNAL_ID_BYTES: usize = 200;

/// One identifier as an external system spells it.
///
/// It is text because a provider chooses its own shape. What it deliberately is
/// **not** is anything the core can be addressed by: it has no `Deref`, no
/// `From`/`Into` toward a canonical type, and no constructor on
/// [`CanonicalRef`] accepts one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalId(String);

impl ExternalId {
    /// Validates and takes an identifier.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Length`] when the value is empty or over
    /// [`MAX_EXTERNAL_ID_BYTES`], and [`IdentityError::Charset`] when it holds a
    /// byte outside printable non-whitespace ASCII.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_EXTERNAL_ID_BYTES {
            return Err(IdentityError::Length);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"')
        {
            return Err(IdentityError::Charset);
        }
        Ok(Self(value))
    }

    /// The identifier as the external system spells it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Which canonical aggregate a mapping points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalKind {
    /// A canonical entity.
    Entity,
    /// A course.
    Course,
    /// A course offering.
    Offering,
    /// An artifact.
    Artifact,
    /// A repository.
    Repository,
}

impl CanonicalKind {
    /// Exhaustive order.
    pub const ALL: [Self; 5] = [
        Self::Entity,
        Self::Course,
        Self::Offering,
        Self::Artifact,
        Self::Repository,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "ENTITY",
            Self::Course => "COURSE",
            Self::Offering => "OFFERING",
            Self::Artifact => "ARTIFACT",
            Self::Repository => "REPOSITORY",
        }
    }
}

/// A reference to something the core already minted.
///
/// Every arm carries a `academic_domain` identifier, and every constructor
/// takes one by value. There is no arm holding text and no constructor taking
/// text, which is what makes "an external identifier never becomes canonical"
/// a property of the type rather than a rule someone has to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalRef {
    /// A canonical entity.
    Entity(EntityId),
    /// A course.
    Course(CourseId),
    /// A course offering.
    Offering(OfferingId),
    /// An artifact.
    Artifact(ArtifactId),
    /// A repository.
    Repository(RepositoryId),
}

impl CanonicalRef {
    /// Which aggregate this reference names.
    #[must_use]
    pub const fn kind(self) -> CanonicalKind {
        match self {
            Self::Entity(_) => CanonicalKind::Entity,
            Self::Course(_) => CanonicalKind::Course,
            Self::Offering(_) => CanonicalKind::Offering,
            Self::Artifact(_) => CanonicalKind::Artifact,
            Self::Repository(_) => CanonicalKind::Repository,
        }
    }

    /// The sixteen opaque identifier bytes, whichever arm this is.
    ///
    /// Used for ordering and for the calendar payload's encoding. It is a
    /// one-way read: there is no constructor taking sixteen bytes, so this is
    /// not a route back into a canonical value either.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        match self {
            Self::Entity(id) => id.as_bytes(),
            Self::Course(id) => id.as_bytes(),
            Self::Offering(id) => id.as_bytes(),
            Self::Artifact(id) => id.as_bytes(),
            Self::Repository(id) => id.as_bytes(),
        }
    }
}

/// How much authority the source of a mapping carries.
///
/// Section 33 says a sync conflict is resolved by "source-wise authority and
/// valid time". This is the first half; the ordering is the enum's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceAuthority {
    /// A connector inferred the mapping from a name or a heuristic.
    Inferred,
    /// A connector reported the mapping from its own API.
    Connector,
    /// The user confirmed the mapping.
    UserConfirmed,
    /// An official institutional record carries it.
    Official,
}

impl SourceAuthority {
    /// Exhaustive order, weakest first.
    pub const ALL: [Self; 4] = [
        Self::Inferred,
        Self::Connector,
        Self::UserConfirmed,
        Self::Official,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inferred => "INFERRED",
            Self::Connector => "CONNECTOR",
            Self::UserConfirmed => "USER_CONFIRMED",
            Self::Official => "OFFICIAL",
        }
    }
}

/// One external identifier, and the canonical thing it maps onto.
///
/// The two halves are separate fields of separate types. Reading the external
/// half back gives an [`ExternalId`]; reading the canonical half gives a
/// [`CanonicalRef`]; and there is no operation from one to the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalIdentity {
    system: ConnectorKind,
    external_id: ExternalId,
    canonical: CanonicalRef,
    authority: SourceAuthority,
    valid_from: TimestampMillis,
}

impl ExternalIdentity {
    /// Records that `external_id` in `system` refers to `canonical`.
    ///
    /// `canonical` arrives by value from the caller. There is no overload of
    /// this constructor that derives one.
    #[must_use]
    pub const fn map(
        system: ConnectorKind,
        external_id: ExternalId,
        canonical: CanonicalRef,
        authority: SourceAuthority,
        valid_from: TimestampMillis,
    ) -> Self {
        Self {
            system,
            external_id,
            canonical,
            authority,
            valid_from,
        }
    }

    /// Which external system spells the identifier this way.
    #[must_use]
    pub const fn system(&self) -> ConnectorKind {
        self.system
    }

    /// The external identifier.
    #[must_use]
    pub const fn external_id(&self) -> &ExternalId {
        &self.external_id
    }

    /// The canonical reference this mapping points at.
    #[must_use]
    pub const fn canonical(&self) -> CanonicalRef {
        self.canonical
    }

    /// How much authority the source carries.
    #[must_use]
    pub const fn authority(&self) -> SourceAuthority {
        self.authority
    }

    /// When the mapping became true in the world.
    #[must_use]
    pub const fn valid_from(&self) -> TimestampMillis {
        self.valid_from
    }
}

/// Which rule decided a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConflictBasis {
    /// One side's source carries more authority.
    SourceAuthority,
    /// Authority is equal and one side is later in valid time.
    ValidTime,
    /// Authority and valid time are both equal. Neither side is preferred.
    Tie,
}

impl ConflictBasis {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::SourceAuthority, Self::ValidTime, Self::Tie];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceAuthority => "SOURCE_AUTHORITY",
            Self::ValidTime => "VALID_TIME",
            Self::Tie => "TIE",
        }
    }
}

/// Two mappings of one external identifier that disagree, and both are kept.
///
/// `held` is the mapping already in the map and `incoming` is the one that
/// arrived. `preferred` names the one a read resolves to; on a
/// [`ConflictBasis::Tie`] it is `None` and the read is refused rather than
/// guessed at, which is `P2-N5`'s rule for a tied root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncConflict {
    held: ExternalIdentity,
    incoming: ExternalIdentity,
    basis: ConflictBasis,
    prefers_incoming: Option<bool>,
}

impl SyncConflict {
    /// The mapping that was already recorded. Never dropped.
    #[must_use]
    pub const fn held(&self) -> &ExternalIdentity {
        &self.held
    }

    /// The mapping that arrived. Never dropped either.
    #[must_use]
    pub const fn incoming(&self) -> &ExternalIdentity {
        &self.incoming
    }

    /// Which rule decided.
    #[must_use]
    pub const fn basis(&self) -> ConflictBasis {
        self.basis
    }

    /// The side a read resolves to, or `None` on a tie.
    #[must_use]
    pub const fn preferred(&self) -> Option<&ExternalIdentity> {
        match self.prefers_incoming {
            None => None,
            Some(true) => Some(&self.incoming),
            Some(false) => Some(&self.held),
        }
    }
}

/// The mappings this profile holds, and every conflict it has seen.
#[derive(Debug, Clone, Default)]
pub struct IdentityMap {
    entries: BTreeMap<(ConnectorKind, String), ExternalIdentity>,
    conflicts: Vec<SyncConflict>,
}

impl IdentityMap {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one mapping.
    ///
    /// A second mapping of the same `(system, external_id)` onto the **same**
    /// canonical reference replaces nothing and reports no conflict. One onto a
    /// different canonical reference is a conflict: both sides are stored in
    /// [`IdentityMap::conflicts`], the winner is decided by source authority
    /// and then by valid time, and on a tie neither is preferred and the entry
    /// keeps the side that was already there.
    ///
    /// The return value is the conflict, when there was one.
    pub fn register(&mut self, identity: ExternalIdentity) -> Option<&SyncConflict> {
        let key = (identity.system, identity.external_id.as_str().to_owned());
        let Some(held) = self.entries.get(&key).cloned() else {
            self.entries.insert(key, identity);
            return None;
        };
        if held.canonical == identity.canonical {
            return None;
        }
        let (basis, prefers_incoming) = if identity.authority != held.authority {
            (
                ConflictBasis::SourceAuthority,
                Some(identity.authority > held.authority),
            )
        } else if identity.valid_from.value() != held.valid_from.value() {
            (
                ConflictBasis::ValidTime,
                Some(identity.valid_from.value() > held.valid_from.value()),
            )
        } else {
            (ConflictBasis::Tie, None)
        };
        if prefers_incoming == Some(true) {
            self.entries.insert(key, identity.clone());
        }
        self.conflicts.push(SyncConflict {
            held,
            incoming: identity,
            basis,
            prefers_incoming,
        });
        self.conflicts.last()
    }

    /// The mapping recorded for one external identifier, if any.
    ///
    /// The canonical value this hands back is the one a caller supplied to
    /// [`IdentityMap::register`]. It is not derived from `external_id`, and the
    /// argument's bytes reach no part of the answer.
    #[must_use]
    pub fn resolve(
        &self,
        system: ConnectorKind,
        external_id: &ExternalId,
    ) -> Option<&ExternalIdentity> {
        self.entries.get(&(system, external_id.as_str().to_owned()))
    }

    /// Every mapping pointing at one canonical reference, in map order.
    #[must_use]
    pub fn mappings_for(&self, canonical: CanonicalRef) -> Vec<&ExternalIdentity> {
        self.entries
            .values()
            .filter(|identity| identity.canonical == canonical)
            .collect()
    }

    /// Every mapping, in `(system, identifier)` order.
    #[must_use]
    pub fn mappings(&self) -> Vec<&ExternalIdentity> {
        self.entries.values().collect()
    }

    /// Every conflict this map has seen, oldest first. Both sides of each.
    #[must_use]
    pub fn conflicts(&self) -> &[SyncConflict] {
        &self.conflicts
    }
}
