//! What a bundle holds: its entries, its scope, its sources and where it came from.
//!
//! ## Section 24.2's `importance` is not the registry's
//!
//! Section 7.2's `ENABLES_COMPETENCY` row carries a `contribution_importance`
//! qualifier whose values are `CRITICAL`, `SUBSTANTIAL` and `MINOR`, and
//! `P2-Y1` reads those from the registry rather than restating them. The
//! `RELEVANT_TO_ROLE` row carries **no** importance qualifier: its closed
//! schema is one key, `role_profile_version`. So [`BundleImportance`] is
//! section 24.2's own vocabulary, read out of the specification's YAML block
//! rather than out of the registry, and `the_version_qualifier_is_the_registry_s`
//! asserts that qualifier set in both directions — a registry that later grows
//! an importance qualifier fails this crate instead of leaving two vocabularies
//! for one thing.
//!
//! ## A source is recorded, and that is the whole of what Phase 2 does with it
//!
//! `GATE-38-029` asks what governance keeps a bundle current without
//! over-weighting labour-market fashion. Phase 2 does not answer it. What it
//! does is record, for every source, the user's own citation and the day they
//! consulted it — [`BundleSource`] has those two fields and no third. There is
//! no fetch, no feed, no refresh and no staleness verdict anywhere in this
//! crate, and [`crate::direction::NO_SHIPPED_BUNDLES`] is the sentence that
//! says the absence is deliberate.
//!
//! ## A date here is valid time
//!
//! [`RecordedOn`] wraps `academic_ingestion::Date`, whose module owns the
//! separation of a document's own dates from the wall clock. Nothing in this
//! crate reads a clock, so every date arrives as an argument; the only thing
//! this type adds is section 24.2's `2026-08-26` spelling on the wire.

use academic_competency::CompetencyId;
use academic_ingestion::Date;
use serde::{Deserialize, Serialize};

use crate::{
    RoleError,
    identity::{RoleProfileRef, non_empty, validated},
};

/// A calendar date, in section 24.2's `validAt` spelling.
///
/// The calendar rule is `academic_ingestion::Date`'s; what is here is the text
/// form and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RecordedOn(Date);

impl RecordedOn {
    /// Takes a date already validated by `academic_ingestion`.
    #[must_use]
    pub const fn on(date: Date) -> Self {
        Self(date)
    }

    /// The date.
    #[must_use]
    pub const fn date(self) -> Date {
        self.0
    }

    /// Reads section 24.2's `YYYY-MM-DD` spelling.
    ///
    /// # Errors
    ///
    /// [`RoleError::NotACalendarDate`] when the text is not three
    /// hyphen-separated numbers, or when `academic_ingestion::Date` refuses the
    /// day it names.
    pub fn parse(text: &str) -> Result<Self, RoleError> {
        let refused = || RoleError::NotACalendarDate(text.to_owned());
        let mut parts = text.split('-');
        let year = parts.next().ok_or_else(refused)?;
        let month = parts.next().ok_or_else(refused)?;
        let day = parts.next().ok_or_else(refused)?;
        if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
            return Err(refused());
        }
        let year = year.parse::<u16>().map_err(|_| refused())?;
        let month = month.parse::<u8>().map_err(|_| refused())?;
        let day = day.parse::<u8>().map_err(|_| refused())?;
        Date::new(year, month, day).map(Self).map_err(|_| refused())
    }
}

impl TryFrom<String> for RecordedOn {
    type Error = RoleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<RecordedOn> for String {
    fn from(value: RecordedOn) -> Self {
        value.0.to_string()
    }
}

/// Section 24.2's `importance` on a bundle entry.
///
/// Three values, read off the specification's own YAML block. See this module's
/// first section for why they are not the predicate registry's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BundleImportance {
    /// `CORE`.
    Core,
    /// `COMMON`.
    Common,
    /// `CONTEXT_DEPENDENT`.
    ContextDependent,
}

impl BundleImportance {
    /// Exhaustive, in the specification's own order of first appearance.
    pub const ALL: [Self; 3] = [Self::Core, Self::Common, Self::ContextDependent];

    /// The specification's spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "CORE",
            Self::Common => "COMMON",
            Self::ContextDependent => "CONTEXT_DEPENDENT",
        }
    }
}

/// One row of section 24.2's `competencies`.
///
/// A competency named by the identity `P2-Y1` issues, and how much this bundle
/// says it matters. The competency half cannot be a concept: `CompetencyId` and
/// `ConceptRef` have no conversion in either direction and this crate adds
/// none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleEntry {
    competency: CompetencyId,
    importance: BundleImportance,
}

impl BundleEntry {
    /// Records one entry.
    #[must_use]
    pub const fn of(competency: CompetencyId, importance: BundleImportance) -> Self {
        Self {
            competency,
            importance,
        }
    }

    /// Which competency.
    #[must_use]
    pub const fn competency(&self) -> &CompetencyId {
        &self.competency
    }

    /// How much this bundle says it matters.
    #[must_use]
    pub const fn importance(&self) -> BundleImportance {
        self.importance
    }
}

/// Section 24.2's `scope`.
///
/// The specification names one value, [`BundleScope::USER_CURATED_GENERAL`],
/// and then says `사용자가 목표 조직·연구실·project에 맞춰 bundle을 fork할 수
/// 있다`. So the set is **open on purpose**: an organisation's, a laboratory's
/// or a project's scope is named by the user, because a closed list of
/// organisations shipped by this repository would be the market claim section
/// 24.2 refuses. What is closed is the shape — an identifier, so a shelf can
/// group on it — and the one spelling the specification itself gives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BundleScope(String);

impl BundleScope {
    /// The one scope section 24.2's YAML block spells.
    pub const USER_CURATED_GENERAL: &'static str = "user_curated_general_profile";

    /// Checks and takes one scope.
    ///
    /// # Errors
    ///
    /// [`RoleError::InvalidIdentifier`] when it is not `[A-Za-z0-9._-]` within
    /// 64 bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, RoleError> {
        Ok(Self(validated(value.into(), "scope")?))
    }

    /// The scope.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BundleScope {
    type Error = RoleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<BundleScope> for String {
    fn from(value: BundleScope) -> Self {
        value.0
    }
}

/// One row of section 24.2's `sources`.
///
/// Two fields and no third: what the user cited, and the day they consulted it.
/// There is no URL fetched here, no feed identifier and no refresh interval —
/// see this module's second section and `GATE-38-029`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleSource {
    cited_as: String,
    consulted_on: RecordedOn,
}

impl BundleSource {
    /// Records one source.
    ///
    /// # Errors
    ///
    /// [`RoleError::EmptyText`] when the citation carries nothing, because a
    /// source nobody wrote is not a recorded source.
    pub fn cited(cited_as: impl Into<String>, consulted_on: RecordedOn) -> Result<Self, RoleError> {
        Ok(Self {
            cited_as: non_empty(cited_as.into(), "source citation")?,
            consulted_on,
        })
    }

    /// What the user cited.
    #[must_use]
    pub fn cited_as(&self) -> &str {
        &self.cited_as
    }

    /// The day they consulted it.
    #[must_use]
    pub const fn consulted_on(&self) -> RecordedOn {
        self.consulted_on
    }
}

/// Where a bundle version came from.
///
/// Three arms, and each one is a different claim about provenance. There is no
/// arm that means *changed in place*, because this repository has none: `P2-N2`
/// replaces an assertion rather than editing it, `P2-R5` replaces a claim, and
/// a bundle takes a new version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "from", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BundleOrigin {
    /// The user wrote it. The first version of a lineage nothing preceded.
    Authored,
    /// The version before it, in the same lineage.
    ///
    /// [`crate::revise`] is the one producer, and it fixes both halves: the
    /// lineage is the base's and the version is the base's plus one.
    Revised(RoleProfileRef),
    /// A version of a **different** lineage.
    ///
    /// [`crate::fork`] is the one producer. The base is named by its exact
    /// pair, so forking version three and forking version four record different
    /// things, and neither touches the base.
    Forked(RoleProfileRef),
}

impl BundleOrigin {
    /// The base this version came from, when there is one.
    #[must_use]
    pub const fn base(&self) -> Option<&RoleProfileRef> {
        match self {
            Self::Authored => None,
            Self::Revised(base) | Self::Forked(base) => Some(base),
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Authored => "AUTHORED",
            Self::Revised(_) => "REVISED",
            Self::Forked(_) => "FORKED",
        }
    }
}
