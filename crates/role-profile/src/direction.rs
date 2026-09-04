//! Section 24.2's role directions: the twelve it names, and the `등` it ends with.
//!
//! The specification's sentence is one line:
//!
//! > `Backend, Systems, Database, Distributed Systems, Infrastructure/Platform,
//! > SRE, Cloud, Security, ML/AI, Data, Compiler/PL, Research 등을 지원하되 role
//! > 이름을 시장의 단일 진리로 두지 않는다.`
//!
//! Twelve names and then `등` — *and so on*. Both halves are represented, and
//! neither is a count this crate asserts on its own:
//! `twelve_role_directions_are_representable_or_explicitly_absent` reads that
//! sentence out of the specification, splits it, and compares the names against
//! [`RoleDirection::NAMED`] **in both directions**, so a specification that
//! renames, drops or adds one fails this crate instead of drifting past it.
//!
//! [`RoleDirection::UserNamed`] is the `등`. It is not a thirteenth direction
//! and it is not in [`RoleDirection::NAMED`]: it is the openness of the
//! specification's own list, carried as a value so that a user curating a
//! bundle for something the sentence does not name has somewhere to put it
//! without anybody widening the closed part.
//!
//! ## Absence is named, not silent
//!
//! `표현 가능하거나 명시적으로 부재` is the acceptance sentence, and the second
//! half is the one that needs machinery. **This build ships no bundle for any
//! of the twelve** — see [`NO_SHIPPED_BUNDLES`] — because `GATE-38-029` is
//! open and a shipped default bundle would be this repository asserting what a
//! Backend Engineer is. So the only bundles that exist are the user's own, and
//! [`crate::BundleShelf::directions_covered`] reports **every** named direction
//! with the count the shelf holds, including the zeros. A reader is told which
//! ten of the twelve nothing covers, by name; a map that omitted its zero rows
//! would leave that absence silent, and `twelve_role_directions_are_representable_or_explicitly_absent`
//! removes exactly that.
//!
//! ## A label is not a direction
//!
//! There is no function from [`crate::RoleLabel`] to a direction and none back.
//! Reading `Backend Engineer` as [`RoleDirection::Backend`] is the inference
//! section 24.2 forbids — `role 이름을 시장의 단일 진리로 두지 않는다` — so a
//! bundle carries its direction as a separate field the user set, and two
//! bundles labelled the same may be pointed at different directions.

use serde::{Deserialize, Serialize};

use crate::{RoleError, identity::validated};

/// What this build ships for each of section 24.2's named directions.
///
/// Nothing, and this constant is the sentence that says so. `GATE-38-029` asks
/// what governance keeps a bundle current without over-weighting labour-market
/// fashion; that is a user decision, so Phase 2 ships user-owned bundles with
/// recorded sources and no market feed, and therefore ships no bundle of its
/// own for any direction.
pub const NO_SHIPPED_BUNDLES: &str = "GATE-38-029 is open: bundle-currency governance that does not \
     overweight labour-market fashion is a user decision, so this build ships \
     no bundle for any named direction and every bundle is the user's own, \
     with its sources recorded and no market feed";

/// The user's own name for a direction section 24.2's list does not carry.
///
/// The `등`'s payload. It is checked as an identifier so it is a stable key a
/// shelf can group on, and it is **not** a label: what the user displays is
/// [`crate::RoleLabel`], which is prose and is never read as one of these.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DirectionName(String);

impl DirectionName {
    /// Checks and takes one name.
    ///
    /// # Errors
    ///
    /// [`RoleError::InvalidIdentifier`] when it is not `[A-Za-z0-9._-]` within
    /// 64 bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, RoleError> {
        Ok(Self(validated(value.into(), "direction")?))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DirectionName {
    type Error = RoleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<DirectionName> for String {
    fn from(value: DirectionName) -> Self {
        value.0
    }
}

/// Section 24.2's role directions.
///
/// Twelve named arms, in the specification's own order, plus the `등`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoleDirection {
    /// `Backend`.
    Backend,
    /// `Systems`.
    Systems,
    /// `Database`.
    Database,
    /// `Distributed Systems`.
    DistributedSystems,
    /// `Infrastructure/Platform`.
    InfrastructurePlatform,
    /// `SRE`.
    Sre,
    /// `Cloud`.
    Cloud,
    /// `Security`.
    Security,
    /// `ML/AI`.
    MlAi,
    /// `Data`.
    Data,
    /// `Compiler/PL`.
    CompilerPl,
    /// `Research`.
    Research,
    /// Section 24.2's `등`: a direction the sentence does not name.
    ///
    /// Not one of [`RoleDirection::NAMED`], and not a thirteenth name. The
    /// specification's list is open, and this arm is that openness rather than
    /// a value this crate added to it.
    UserNamed(DirectionName),
}

impl RoleDirection {
    /// The twelve section 24.2 names, in the specification's order.
    ///
    /// [`RoleDirection::UserNamed`] is deliberately absent: it is the `등`, not
    /// a name the sentence carries.
    pub const NAMED: [Self; 12] = [
        Self::Backend,
        Self::Systems,
        Self::Database,
        Self::DistributedSystems,
        Self::InfrastructurePlatform,
        Self::Sre,
        Self::Cloud,
        Self::Security,
        Self::MlAi,
        Self::Data,
        Self::CompilerPl,
        Self::Research,
    ];

    /// The specification's own spelling, for a named direction.
    ///
    /// [`None`] for the `등`, which the sentence does not spell.
    #[must_use]
    pub const fn spec_name(&self) -> Option<&'static str> {
        match self {
            Self::Backend => Some("Backend"),
            Self::Systems => Some("Systems"),
            Self::Database => Some("Database"),
            Self::DistributedSystems => Some("Distributed Systems"),
            Self::InfrastructurePlatform => Some("Infrastructure/Platform"),
            Self::Sre => Some("SRE"),
            Self::Cloud => Some("Cloud"),
            Self::Security => Some("Security"),
            Self::MlAi => Some("ML/AI"),
            Self::Data => Some("Data"),
            Self::CompilerPl => Some("Compiler/PL"),
            Self::Research => Some("Research"),
            Self::UserNamed(_) => None,
        }
    }

    /// Whether section 24.2's sentence names this direction.
    #[must_use]
    pub const fn is_named_by_the_specification(&self) -> bool {
        self.spec_name().is_some()
    }
}
