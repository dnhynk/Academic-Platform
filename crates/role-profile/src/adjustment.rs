//! Section 24.2's `userAdjustments`, stored beside the base bundle rather than in it.
//!
//! ## Why this is a separate document
//!
//! The specification's YAML block shows `userAdjustments` as a key of the role
//! profile, which is what a *rendered* profile looks like. What is stored is
//! two documents: [`crate::RoleProfile`], which has no adjustment field at all
//! and refuses an unknown key on the wire, and [`AdjustmentLayer`], which names
//! the base it adjusts by its exact [`crate::RoleProfileRef`].
//!
//! That separation is the contract `P2-Y2` fixes, and it buys two things a
//! merged document cannot have. An organisation's bundle stays byte-identical
//! whatever the user did to it, so `two_org_bundles_coexist_with_scope_and_source`
//! compares two bases that no adjustment can have touched. And a layer names
//! the **version** it was written against, so a layer written over version
//! three is not silently applied to version four: [`crate::revise`] refuses the
//! mismatch by identity rather than by lineage.
//!
//! ## An adjustment carries a reason
//!
//! [`UserAdjustment::because`] is required and non-empty. A bundle is the
//! user's own claim about what a role needs, and section 24.2's constraint —
//! `role 이름을 시장의 단일 진리로 두지 않는다` — is only inspectable if the
//! reason a competency was added, dropped or reweighted is on the record beside
//! the change.

use academic_competency::CompetencyId;
use serde::{Deserialize, Serialize};

use crate::{
    RoleError,
    bundle::BundleImportance,
    identity::{RoleProfileRef, non_empty},
};

/// One change to a base bundle's entries.
///
/// Three arms. There is no arm that renames the bundle, restates its label or
/// moves its direction: an adjustment is about which competencies a role needs
/// and how much, which is what section 24.2's `competencies` block holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Adjustment {
    /// Adds a competency the base does not name.
    Add {
        /// Which competency.
        competency: CompetencyId,
        /// How much the user says it matters.
        importance: BundleImportance,
    },
    /// Drops one the base names.
    Remove {
        /// Which competency.
        competency: CompetencyId,
    },
    /// Restates the importance of one the base names.
    Reweight {
        /// Which competency.
        competency: CompetencyId,
        /// The importance the user gives it instead.
        importance: BundleImportance,
    },
}

impl Adjustment {
    /// Which competency this adjustment is about.
    #[must_use]
    pub const fn competency(&self) -> &CompetencyId {
        match self {
            Self::Add { competency, .. }
            | Self::Remove { competency }
            | Self::Reweight { competency, .. } => competency,
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Add { .. } => "ADD",
            Self::Remove { .. } => "REMOVE",
            Self::Reweight { .. } => "REWEIGHT",
        }
    }
}

/// One adjustment and the user's reason for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AdjustmentWire", into = "AdjustmentWire")]
pub struct UserAdjustment {
    adjustment: Adjustment,
    because: String,
}

impl UserAdjustment {
    /// Records one adjustment with its reason.
    ///
    /// # Errors
    ///
    /// [`RoleError::EmptyText`] when the reason carries nothing.
    pub fn of(adjustment: Adjustment, because: impl Into<String>) -> Result<Self, RoleError> {
        Ok(Self {
            adjustment,
            because: non_empty(because.into(), "adjustment reason")?,
        })
    }

    /// The change.
    #[must_use]
    pub const fn adjustment(&self) -> &Adjustment {
        &self.adjustment
    }

    /// Why the user made it.
    #[must_use]
    pub fn because(&self) -> &str {
        &self.because
    }
}

/// The serialized shape of a [`UserAdjustment`].
///
/// Two keys rather than the change flattened beside its reason: `serde`'s
/// `deny_unknown_fields` and `flatten` do not compose, and a wire that accepted
/// keys nobody declared is the door this crate closes everywhere else.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdjustmentWire {
    adjustment: Adjustment,
    because: String,
}

impl TryFrom<AdjustmentWire> for UserAdjustment {
    type Error = RoleError;

    fn try_from(wire: AdjustmentWire) -> Result<Self, Self::Error> {
        Self::of(wire.adjustment, wire.because)
    }
}

impl From<UserAdjustment> for AdjustmentWire {
    fn from(value: UserAdjustment) -> Self {
        Self {
            adjustment: value.adjustment,
            because: value.because,
        }
    }
}

/// Section 24.2's `userAdjustments`, as its own document.
///
/// Bound to the exact base version it was written against. It holds no label,
/// no scope and no source: it is what the user changed, and nothing about it is
/// a second copy of the bundle it adjusts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "LayerWire", into = "LayerWire")]
pub struct AdjustmentLayer {
    base: RoleProfileRef,
    adjustments: Vec<UserAdjustment>,
}

impl AdjustmentLayer {
    /// Records the adjustments a user wrote over one base version.
    ///
    /// # Errors
    ///
    /// [`RoleError::LayerAdjustsNothing`] when the list is empty, because a
    /// layer that changes nothing would still take a version if it were
    /// applied; and [`RoleError::CompetencyAdjustedTwice`] when two adjustments
    /// name one competency, because the outcome would depend on the order two
    /// changes to one subject were written in.
    pub fn over(base: RoleProfileRef, adjustments: Vec<UserAdjustment>) -> Result<Self, RoleError> {
        if adjustments.is_empty() {
            return Err(RoleError::LayerAdjustsNothing(base.rendered()));
        }
        for (index, entry) in adjustments.iter().enumerate() {
            if adjustments[..index]
                .iter()
                .any(|earlier| earlier.adjustment().competency() == entry.adjustment().competency())
            {
                return Err(RoleError::CompetencyAdjustedTwice(
                    entry.adjustment().competency().as_str().to_owned(),
                ));
            }
        }
        Ok(Self { base, adjustments })
    }

    /// The exact base version this layer was written against.
    #[must_use]
    pub const fn base(&self) -> &RoleProfileRef {
        &self.base
    }

    /// The adjustments, in the order the user wrote them.
    #[must_use]
    pub fn adjustments(&self) -> &[UserAdjustment] {
        &self.adjustments
    }
}

/// The serialized shape of an [`AdjustmentLayer`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayerWire {
    base: RoleProfileRef,
    user_adjustments: Vec<UserAdjustment>,
}

impl TryFrom<LayerWire> for AdjustmentLayer {
    type Error = RoleError;

    fn try_from(wire: LayerWire) -> Result<Self, Self::Error> {
        Self::over(wire.base, wire.user_adjustments)
    }
}

impl From<AdjustmentLayer> for LayerWire {
    fn from(value: AdjustmentLayer) -> Self {
        Self {
            base: value.base,
            user_adjustments: value.adjustments,
        }
    }
}
