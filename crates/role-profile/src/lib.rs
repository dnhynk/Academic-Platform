//! `P2-Y2`: section 24.2's `RoleProfile` as a versioned competency bundle.
//!
//! `P2-Y1` fixed what a competency is. This crate is the step that puts
//! competencies into a bundle somebody can name, version, disagree with and
//! fork — without the bundle's name becoming a claim about the world.
//!
//! ## A role name is not a market truth
//!
//! Section 24.2's last two sentences are the whole constraint:
//!
//! > `Backend, Systems, Database, Distributed Systems, Infrastructure/Platform,
//! > SRE, Cloud, Security, ML/AI, Data, Compiler/PL, Research 등을 지원하되 role
//! > 이름을 시장의 단일 진리로 두지 않는다. 사용자가 목표 조직·연구실·project에
//! > 맞춰 bundle을 fork할 수 있다.`
//!
//! There is no market feed here, and there is no bundle shipped by this
//! repository for any direction — `GATE-38-029` is open and
//! [`direction::NO_SHIPPED_BUNDLES`] is the sentence that says so. What this
//! crate holds is the user's own bundles, each with its sources recorded. Four
//! separations carry the rest:
//!
//! | The thing that must not happen | What stops it |
//! |---|---|
//! | a label read as a direction | no function from [`RoleLabel`] to [`RoleDirection`] and none back; [`RoleLabel`] is prose, the direction is a field the user set |
//! | a label resolved to one bundle | [`BundleShelf::by_label`] returns a [`LabelReading`], never one bundle, and carries a [`LabelAmbiguity`] when it reached several |
//! | one organisation's bundle overwriting another's | the shelf is keyed on the lineage-and-version pair and [`BundleShelf::shelve`] refuses an occupied key |
//! | a favourite becoming a plan | [`RoleInterest`] holds a lineage and a standing, no function takes one, and [`InterestStanding`] has no arm meaning *chosen* |
//!
//! ## An edit is a new version, and a fork is a new lineage
//!
//! [`RoleProfile`] has no public field, no setter and no `&mut self` method —
//! and neither does anything else in this crate, [`BundleShelf`] included.
//!
//! * [`revise`] takes `&RoleProfile` and an [`AdjustmentLayer`] and returns a
//!   **new** profile at the next version of the same lineage.
//! * [`fork`] takes `&RoleProfile` and returns a new profile at version one of
//!   a **different** lineage, recording the base by its exact pair.
//!
//! Neither can touch its base, and [`BundleShelf::shelve`] refuses to replace an
//! occupied pair, so a change that wants to be stored has to take a version it
//! did not have. That is `P2-N2`'s `assertion 은 제자리에서 변경되지 않는다`
//! one stage over.
//!
//! ## The identity is a pair, not a name
//!
//! Section 24.2 writes `id: backend_engineer_profile_v4`, which folds the
//! lineage and the version into one string. [`RoleProfileRef`] is the pair, and
//! that spelling is [`RoleProfileRef::rendered`] — display only, with no parser
//! back. See [`crate::identity`] for the collision this avoids and the `P2-R4`
//! and `P2-A1` measurements behind it.
//!
//! ## User adjustments are a second document
//!
//! Section 24.2's `userAdjustments` key is not a field of [`RoleProfile`], and
//! the wire refuses unknown keys, so it cannot arrive through JSON either. It
//! is [`AdjustmentLayer`], bound to the exact base version it was written
//! against. See [`crate::adjustment`].
//!
//! ## It opens nothing and persists nothing
//!
//! No file, no socket, no clock, no `academic-store` edge and no migration.
//! Every date arrives as an argument, through `academic_ingestion`'s `Date`,
//! whose module owns the separation of a document's own dates from the wall
//! clock.
//!
//! ## What this task does not decide
//!
//! * **The readiness matrix.** `P2-Y3` owns the competency × evidence view, the
//!   six axes, auxiliary scores and the non-guarantee notice. Nothing here
//!   scores anything or reads an evidence rubric.
//! * **The Career Explorer.** Section 25.11's graph, comparison view and
//!   acquisition options are a `P2-X`-stage surface. This crate has no compare
//!   function.
//! * **Freshness.** `P2-N3` owns the bands. A bundle's `validAt` and a source's
//!   consultation date are recorded and read by nothing here.
//! * **§38.** `GATE-38-029` stays **open**: bundle-currency governance that
//!   does not overweight labour-market fashion is a user decision. Phase 2
//!   ships user-owned bundles with recorded sources and no market feed.

pub mod adjustment;
pub mod bundle;
pub mod direction;
pub mod identity;
pub mod interest;
pub mod shelf;

use std::collections::BTreeSet;

use academic_competency::CompetencyId;
use academic_domain::predicates::{NodeType, PredicateName};
use serde::{Deserialize, Serialize};

pub use adjustment::{Adjustment, AdjustmentLayer, UserAdjustment};
pub use bundle::{
    BundleEntry, BundleImportance, BundleOrigin, BundleScope, BundleSource, RecordedOn,
};
pub use direction::{DirectionName, NO_SHIPPED_BUNDLES, RoleDirection};
pub use identity::{RoleLabel, RoleProfileId, RoleProfileRef, RoleProfileVersion};
pub use interest::{InterestStanding, REFUSED_STANDINGS, RoleInterest};
pub use shelf::{BundleShelf, DirectionCoverage, LabelAmbiguity, LabelReading};

/// Why a role-bundle operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RoleError {
    /// An identifier was empty, too long, or held a forbidden byte.
    #[error("the {0} identifier {1:?} is not [A-Za-z0-9._-] within 64 bytes")]
    InvalidIdentifier(&'static str, String),
    /// A required piece of prose carried nothing.
    #[error("the {0} carries no text")]
    EmptyText(&'static str),
    /// Section 24.2's `validAt` spelling was not a calendar date.
    #[error("{0:?} is not a YYYY-MM-DD calendar date")]
    NotACalendarDate(String),
    /// Section 7.2's `role_profile_version` qualifier is a positive integer.
    #[error("a role profile version is a positive integer, and zero is not one")]
    VersionIsNotPositive,
    /// The lineage has run out of versions.
    #[error("the next version would overflow, and wrapping would claim to be the first")]
    VersionWouldOverflow,
    /// A bundle with no competency bundles nothing.
    #[error("role profile {0} bundles no competency")]
    BundleNamesNoCompetency(String),
    /// One competency was listed twice in one bundle.
    #[error("role profile {profile} names competency {competency} twice")]
    DuplicateCompetency {
        /// Which bundle.
        profile: String,
        /// Which competency.
        competency: String,
    },
    /// `GATE-38-029`: Phase 2 ships bundles whose sources are recorded.
    #[error("role profile {0} records no source")]
    BundleRecordsNoSource(String),
    /// An adjustment layer that changes nothing would still take a version.
    #[error("the adjustment layer over {0} adjusts nothing")]
    LayerAdjustsNothing(String),
    /// Two adjustments named one competency, so the outcome would depend on
    /// the order they were written in.
    #[error("competency {0} is adjusted twice in one layer")]
    CompetencyAdjustedTwice(String),
    /// The layer was written against another version.
    #[error("the adjustment layer was written over {layer_base}, not over {profile}")]
    LayerIsForAnotherVersion {
        /// Which version the layer names.
        layer_base: String,
        /// Which version it was offered against.
        profile: String,
    },
    /// An `ADD` named a competency the bundle already has.
    #[error("role profile {profile} already names competency {competency}")]
    AddedCompetencyAlreadyPresent {
        /// Which bundle.
        profile: String,
        /// Which competency.
        competency: String,
    },
    /// A `REMOVE` or `REWEIGHT` named a competency the bundle does not have.
    #[error("role profile {profile} does not name competency {competency}")]
    AdjustedCompetencyIsNotInTheBundle {
        /// Which bundle.
        profile: String,
        /// Which competency.
        competency: String,
    },
    /// A fork has to be a different lineage, or it is a revision.
    #[error("forking {0} into its own lineage is a revision, not a fork")]
    ForkIntoTheSameLineage(String),
    /// The shelf already holds that lineage at that version.
    #[error("the shelf already holds {0}")]
    VersionAlreadyShelved(String),
    /// A deserialized bundle's origin disagreed with its own version.
    #[error("role profile {profile}'s {origin} origin does not match its version")]
    OriginDoesNotMatchTheVersion {
        /// Which bundle.
        profile: String,
        /// Which origin it claimed.
        origin: &'static str,
    },
}

/// Section 24.2's `RoleProfile`: a versioned competency bundle.
///
/// No public field, no setter and no `&mut self` method. An edit is [`revise`]
/// and a fork is [`fork`], and both return a new value at a version their base
/// did not occupy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RoleProfileWire", into = "RoleProfileWire")]
pub struct RoleProfile {
    id: RoleProfileId,
    version: RoleProfileVersion,
    label: RoleLabel,
    direction: RoleDirection,
    valid_at: RecordedOn,
    scope: BundleScope,
    competencies: Vec<BundleEntry>,
    sources: Vec<BundleSource>,
    origin: BundleOrigin,
}

impl RoleProfile {
    /// The lineage.
    #[must_use]
    pub const fn id(&self) -> &RoleProfileId {
        &self.id
    }

    /// The version.
    #[must_use]
    pub const fn version(&self) -> RoleProfileVersion {
        self.version
    }

    /// The identity: the lineage-and-version pair.
    #[must_use]
    pub fn reference(&self) -> RoleProfileRef {
        RoleProfileRef::of(self.id.clone(), self.version)
    }

    /// Section 24.2's `label`. The user's display words, and not an identity.
    #[must_use]
    pub const fn label(&self) -> &RoleLabel {
        &self.label
    }

    /// Which of section 24.2's directions the user pointed this bundle at.
    ///
    /// A field, not a reading of [`RoleProfile::label`]: there is no function
    /// from one to the other.
    #[must_use]
    pub const fn direction(&self) -> &RoleDirection {
        &self.direction
    }

    /// Section 24.2's `validAt`.
    #[must_use]
    pub const fn valid_at(&self) -> RecordedOn {
        self.valid_at
    }

    /// Section 24.2's `scope`.
    #[must_use]
    pub const fn scope(&self) -> &BundleScope {
        &self.scope
    }

    /// Section 24.2's `competencies`, in the order they were written.
    #[must_use]
    pub fn competencies(&self) -> &[BundleEntry] {
        &self.competencies
    }

    /// One entry, by competency.
    #[must_use]
    pub fn entry(&self, competency: &CompetencyId) -> Option<&BundleEntry> {
        self.competencies
            .iter()
            .find(|entry| entry.competency() == competency)
    }

    /// Section 24.2's `sources`.
    #[must_use]
    pub fn sources(&self) -> &[BundleSource] {
        &self.sources
    }

    /// Where this version came from.
    #[must_use]
    pub const fn origin(&self) -> &BundleOrigin {
        &self.origin
    }

    /// Section 7.1's node type for this entity, from the shared vocabulary.
    #[must_use]
    pub const fn node_type() -> NodeType {
        NodeType::RoleProfile
    }

    /// The section 7.2 predicate a bundle entry asserts.
    #[must_use]
    pub const fn entry_predicate() -> PredicateName {
        PredicateName::RelevantToRole
    }
}

/// Checks a bundle's entries and sources.
fn checked(
    id: &RoleProfileId,
    competencies: &[BundleEntry],
    sources: &[BundleSource],
) -> Result<(), RoleError> {
    if competencies.is_empty() {
        return Err(RoleError::BundleNamesNoCompetency(id.as_str().to_owned()));
    }
    let mut seen = BTreeSet::new();
    for entry in competencies {
        if !seen.insert(entry.competency().clone()) {
            return Err(RoleError::DuplicateCompetency {
                profile: id.as_str().to_owned(),
                competency: entry.competency().as_str().to_owned(),
            });
        }
    }
    if sources.is_empty() {
        return Err(RoleError::BundleRecordsNoSource(id.as_str().to_owned()));
    }
    Ok(())
}

/// Declares the first version of a bundle.
///
/// The version is [`RoleProfileVersion::FIRST`] and the origin is
/// [`BundleOrigin::Authored`]: this is the door for a bundle nothing preceded.
/// A later version comes from [`revise`] and another lineage from [`fork`], and
/// neither is reachable from here.
///
/// # Errors
///
/// [`RoleError::BundleNamesNoCompetency`] for an empty bundle;
/// [`RoleError::DuplicateCompetency`] when one competency is named twice; and
/// [`RoleError::BundleRecordsNoSource`] when nothing is cited, because
/// `GATE-38-029` stays open on the strength of the recording.
pub fn declare(
    id: RoleProfileId,
    label: RoleLabel,
    direction: RoleDirection,
    valid_at: RecordedOn,
    scope: BundleScope,
    competencies: Vec<BundleEntry>,
    sources: Vec<BundleSource>,
) -> Result<RoleProfile, RoleError> {
    checked(&id, &competencies, &sources)?;
    Ok(RoleProfile {
        id,
        version: RoleProfileVersion::FIRST,
        label,
        direction,
        valid_at,
        scope,
        competencies,
        sources,
        origin: BundleOrigin::Authored,
    })
}

/// Applies one adjustment layer and returns the next version.
///
/// `base` is borrowed and unchanged. What comes back is a new value at
/// `base`'s version plus one, in `base`'s lineage, whose origin names `base` by
/// its exact pair. The layer stays where it is: it is the record of *why* the
/// versions differ, and it is stored beside the bundles rather than inside one.
///
/// # Errors
///
/// [`RoleError::LayerIsForAnotherVersion`] when the layer was written against a
/// different pair — a layer over version three is not applied to version four;
/// [`RoleError::AddedCompetencyAlreadyPresent`] and
/// [`RoleError::AdjustedCompetencyIsNotInTheBundle`] when an adjustment
/// disagrees with the base about what is in it;
/// [`RoleError::BundleNamesNoCompetency`] when the adjustments would empty the
/// bundle; and [`RoleError::VersionWouldOverflow`] at the top of the range.
pub fn revise(
    base: &RoleProfile,
    layer: &AdjustmentLayer,
    valid_at: RecordedOn,
) -> Result<RoleProfile, RoleError> {
    let reference = base.reference();
    if layer.base() != &reference {
        return Err(RoleError::LayerIsForAnotherVersion {
            layer_base: layer.base().rendered(),
            profile: reference.rendered(),
        });
    }
    let mut competencies = base.competencies.clone();
    for entry in layer.adjustments() {
        let competency = entry.adjustment().competency();
        let at = competencies
            .iter()
            .position(|held| held.competency() == competency);
        match entry.adjustment() {
            Adjustment::Add { importance, .. } => {
                if at.is_some() {
                    return Err(RoleError::AddedCompetencyAlreadyPresent {
                        profile: base.id.as_str().to_owned(),
                        competency: competency.as_str().to_owned(),
                    });
                }
                competencies.push(BundleEntry::of(competency.clone(), *importance));
            }
            Adjustment::Remove { .. } => {
                let at = at.ok_or_else(|| RoleError::AdjustedCompetencyIsNotInTheBundle {
                    profile: base.id.as_str().to_owned(),
                    competency: competency.as_str().to_owned(),
                })?;
                competencies.remove(at);
            }
            Adjustment::Reweight { importance, .. } => {
                let at = at.ok_or_else(|| RoleError::AdjustedCompetencyIsNotInTheBundle {
                    profile: base.id.as_str().to_owned(),
                    competency: competency.as_str().to_owned(),
                })?;
                competencies[at] = BundleEntry::of(competency.clone(), *importance);
            }
        }
    }
    checked(&base.id, &competencies, &base.sources)?;
    Ok(RoleProfile {
        id: base.id.clone(),
        version: base.version.next()?,
        label: base.label.clone(),
        direction: base.direction.clone(),
        valid_at,
        scope: base.scope.clone(),
        competencies,
        sources: base.sources.clone(),
        origin: BundleOrigin::Revised(reference),
    })
}

/// Forks a bundle into a new lineage.
///
/// `base` is borrowed and unchanged. The fork carries the base's entries and
/// its direction, and states its own label, scope, date and sources: it cites
/// its base once, by the exact pair in [`BundleOrigin::Forked`], rather than
/// copying the base's citations and claiming them as its own.
///
/// # Errors
///
/// [`RoleError::ForkIntoTheSameLineage`] when the new identity is the base's,
/// because that is [`revise`]; plus [`declare`]'s own refusals over the fork's
/// entries and sources.
pub fn fork(
    base: &RoleProfile,
    into: RoleProfileId,
    label: RoleLabel,
    valid_at: RecordedOn,
    scope: BundleScope,
    sources: Vec<BundleSource>,
) -> Result<RoleProfile, RoleError> {
    if into == base.id {
        return Err(RoleError::ForkIntoTheSameLineage(
            base.id.as_str().to_owned(),
        ));
    }
    let competencies = base.competencies.clone();
    checked(&into, &competencies, &sources)?;
    Ok(RoleProfile {
        id: into,
        version: RoleProfileVersion::FIRST,
        label,
        direction: base.direction.clone(),
        valid_at,
        scope,
        competencies,
        sources,
        origin: BundleOrigin::Forked(base.reference()),
    })
}

/// The serialized shape, with section 24.2's own key names.
///
/// `deny_unknown_fields` is what keeps `userAdjustments` out: a document
/// carrying one is refused rather than silently ignored, because adjustments
/// are [`AdjustmentLayer`] and live in their own document.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RoleProfileWire {
    id: RoleProfileId,
    version: RoleProfileVersion,
    label: RoleLabel,
    direction: RoleDirection,
    valid_at: RecordedOn,
    scope: BundleScope,
    competencies: Vec<BundleEntry>,
    sources: Vec<BundleSource>,
    origin: BundleOrigin,
}

impl TryFrom<RoleProfileWire> for RoleProfile {
    type Error = RoleError;

    /// Re-runs the entry and source checks, and then checks the origin against
    /// the version.
    ///
    /// The second half is what stops deserialization being a door the two
    /// constructors are not: an `AUTHORED` bundle at version nine, a `REVISED`
    /// one that does not name its own predecessor, and a `FORKED` one that
    /// names its own lineage are all refused here.
    fn try_from(wire: RoleProfileWire) -> Result<Self, Self::Error> {
        checked(&wire.id, &wire.competencies, &wire.sources)?;
        let consistent = match &wire.origin {
            BundleOrigin::Authored => wire.version == RoleProfileVersion::FIRST,
            BundleOrigin::Revised(base) => {
                base.profile() == &wire.id && base.version().next() == Ok(wire.version)
            }
            BundleOrigin::Forked(base) => {
                base.profile() != &wire.id && wire.version == RoleProfileVersion::FIRST
            }
        };
        if !consistent {
            return Err(RoleError::OriginDoesNotMatchTheVersion {
                profile: wire.id.as_str().to_owned(),
                origin: wire.origin.as_str(),
            });
        }
        Ok(Self {
            id: wire.id,
            version: wire.version,
            label: wire.label,
            direction: wire.direction,
            valid_at: wire.valid_at,
            scope: wire.scope,
            competencies: wire.competencies,
            sources: wire.sources,
            origin: wire.origin,
        })
    }
}

impl From<RoleProfile> for RoleProfileWire {
    fn from(value: RoleProfile) -> Self {
        Self {
            id: value.id,
            version: value.version,
            label: value.label,
            direction: value.direction,
            valid_at: value.valid_at,
            scope: value.scope,
            competencies: value.competencies,
            sources: value.sources,
            origin: value.origin,
        }
    }
}
