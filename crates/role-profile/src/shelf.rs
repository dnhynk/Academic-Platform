//! Where bundles live together, and why nothing on the shelf overwrites anything.
//!
//! ## Two organisations, one label, two bundles
//!
//! Section 24.2 ends `role 이름을 시장의 단일 진리로 두지 않는다`, and the shelf
//! is where that stops being a sentence. It is keyed on
//! [`crate::RoleProfileRef`] — the lineage-and-version pair — so two
//! organisations that both write `Backend Engineer` on a bundle occupy two
//! keys. [`BundleShelf::shelve`] **refuses** to replace an occupied key rather
//! than overwriting it, which is `P2-R4`'s `ClassificationConflict` and
//! `P2-N5`'s tied roots one stage over: the second value is kept and the
//! collision is reported, not resolved.
//!
//! [`BundleShelf::by_label`] therefore returns a [`LabelReading`] and never one
//! bundle. There is no `Option<&RoleProfile>` accessor keyed on a label
//! anywhere in this crate, because a caller handed one bundle for a job title
//! would have been told the market truth section 24.2 refuses. When a label
//! reaches more than one bundle the reading carries a [`LabelAmbiguity`] naming
//! the distinct lineages and scopes it reached, which is the diagnostic instead
//! of the resolution.
//!
//! ## Absence is reported by name
//!
//! [`BundleShelf::directions_covered`] returns a row for **every one** of
//! section 24.2's twelve named directions, including the ones the shelf holds
//! nothing for, plus a row for every `등` direction the user named. A map that
//! omitted its empty rows would leave ten of the twelve silently missing, and
//! `twelve_role_directions_are_representable_or_explicitly_absent` is what refuses that: this build ships
//! no bundle at all, so the empty rows are the normal case rather than an edge
//! one.
//!
//! ## The shelf takes `self`
//!
//! [`BundleShelf::shelve`] consumes and returns. No public function in this
//! crate takes `&mut self`, so *a bundle is never edited in place* holds for
//! the collection as well as for the bundle — which is the same discipline as
//! `P2-N2`'s assertions, which are replaced rather than changed.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    RoleError, RoleProfile,
    direction::{NO_SHIPPED_BUNDLES, RoleDirection},
    identity::{RoleLabel, RoleProfileId, RoleProfileRef},
};

/// Bundles held together, keyed on the lineage-and-version pair.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundleShelf {
    profiles: BTreeMap<RoleProfileRef, RoleProfile>,
}

impl BundleShelf {
    /// A shelf holding nothing.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            profiles: BTreeMap::new(),
        }
    }

    /// Puts one bundle on the shelf.
    ///
    /// # Errors
    ///
    /// [`RoleError::VersionAlreadyShelved`] when the pair is occupied. Nothing
    /// is overwritten: an edit that wants to be stored takes a new version, and
    /// two organisations' bundles occupy two keys.
    pub fn shelve(mut self, profile: RoleProfile) -> Result<Self, RoleError> {
        let key = profile.reference();
        if self.profiles.contains_key(&key) {
            return Err(RoleError::VersionAlreadyShelved(key.rendered()));
        }
        self.profiles.insert(key, profile);
        Ok(self)
    }

    /// One bundle, by its exact pair.
    #[must_use]
    pub fn get(&self, reference: &RoleProfileRef) -> Option<&RoleProfile> {
        self.profiles.get(reference)
    }

    /// How many bundles the shelf holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Whether the shelf holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// Every bundle, in key order.
    #[must_use]
    pub fn bundles(&self) -> Vec<&RoleProfile> {
        self.profiles.values().collect()
    }

    /// Every version of one lineage, in version order.
    #[must_use]
    pub fn versions_of(&self, profile: &RoleProfileId) -> Vec<&RoleProfile> {
        self.profiles
            .iter()
            .filter(|(key, _)| key.profile() == profile)
            .map(|(_, value)| value)
            .collect()
    }

    /// What one label reaches.
    ///
    /// Never one bundle. See this module's first section.
    #[must_use]
    pub fn by_label(&self, label: &RoleLabel) -> LabelReading {
        let reached: Vec<RoleProfileRef> = self
            .profiles
            .iter()
            .filter(|(_, profile)| profile.label() == label)
            .map(|(key, _)| key.clone())
            .collect();
        let lineages: BTreeSet<RoleProfileId> =
            reached.iter().map(|key| key.profile().clone()).collect();
        let scopes: BTreeSet<String> = reached
            .iter()
            .filter_map(|key| self.profiles.get(key))
            .map(|profile| profile.scope().as_str().to_owned())
            .collect();
        let ambiguity = if reached.len() > 1 {
            Some(LabelAmbiguity {
                lineages: lineages.into_iter().collect(),
                scopes: scopes.into_iter().collect(),
            })
        } else {
            None
        };
        LabelReading {
            label: label.clone(),
            reached,
            ambiguity,
        }
    }

    /// What the shelf holds for every direction, including the empty ones.
    ///
    /// Section 24.2's twelve named directions always appear. A `등` direction
    /// appears when the shelf holds a bundle pointed at it.
    #[must_use]
    pub fn directions_covered(&self) -> Vec<DirectionCoverage> {
        let mut held: BTreeMap<RoleDirection, Vec<RoleProfileRef>> = RoleDirection::NAMED
            .iter()
            .map(|direction| (direction.clone(), Vec::new()))
            .collect();
        for (key, profile) in &self.profiles {
            held.entry(profile.direction().clone())
                .or_default()
                .push(key.clone());
        }
        held.into_iter()
            .map(|(direction, held)| DirectionCoverage { direction, held })
            .collect()
    }
}

/// What one label reached on a shelf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelReading {
    label: RoleLabel,
    reached: Vec<RoleProfileRef>,
    ambiguity: Option<LabelAmbiguity>,
}

impl LabelReading {
    /// The label that was looked up.
    #[must_use]
    pub const fn label(&self) -> &RoleLabel {
        &self.label
    }

    /// Every bundle carrying it, in key order.
    #[must_use]
    pub fn reached(&self) -> &[RoleProfileRef] {
        &self.reached
    }

    /// The diagnostic, when the label reached more than one bundle.
    #[must_use]
    pub const fn ambiguity(&self) -> Option<&LabelAmbiguity> {
        self.ambiguity.as_ref()
    }
}

/// One label, more than one bundle.
///
/// A diagnostic and not a resolution: it says what the label reached so a
/// reader can pick, and nothing here picks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelAmbiguity {
    lineages: Vec<RoleProfileId>,
    scopes: Vec<String>,
}

impl LabelAmbiguity {
    /// The distinct lineages the label reached.
    #[must_use]
    pub fn lineages(&self) -> &[RoleProfileId] {
        &self.lineages
    }

    /// The distinct scopes they were curated under.
    #[must_use]
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }
}

/// What a shelf holds for one direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionCoverage {
    direction: RoleDirection,
    held: Vec<RoleProfileRef>,
}

impl DirectionCoverage {
    /// Which direction.
    #[must_use]
    pub const fn direction(&self) -> &RoleDirection {
        &self.direction
    }

    /// The bundles the shelf holds for it, in key order. Often empty.
    #[must_use]
    pub fn held(&self) -> &[RoleProfileRef] {
        &self.held
    }

    /// Whether the shelf holds anything for it.
    #[must_use]
    pub fn is_covered(&self) -> bool {
        !self.held.is_empty()
    }

    /// Why an uncovered direction is uncovered.
    ///
    /// One sentence, the same for every direction, naming `GATE-38-029`. This
    /// build ships no bundle, so the only thing that can cover a direction is
    /// the user's own curation.
    #[must_use]
    pub const fn absent_because() -> &'static str {
        NO_SHIPPED_BUNDLES
    }
}
