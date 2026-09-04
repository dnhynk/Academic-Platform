//! Section 25.3's ten lenses and section 26.3's composition rule.
//!
//! # One base and at most two overlays, held by a signature
//!
//! [`LensComposition::overlay`] takes `self` **by value**. A rejected third
//! overlay therefore consumes the composition and returns an error holding
//! nothing a caller can retry with: there is no `&mut self` anywhere, no
//! `push`, no public `Vec<MapLens>` field and no constructor from a list. That
//! is `P2-X1`'s `Optimistic::confirm` shape, chosen for the same reason —
//! a refusal that leaves the value behind is a refusal a loop defeats.
//!
//! # Which channel a lens claims is measured, not decided here
//!
//! Section 25.3 names ten lenses. Section 26.2 names eight channels. The two
//! lists were written independently and they overlap on **four** lenses:
//! `Freshness` appears in the outer-ring bullet, `Project` and `Question` in the
//! glyph bullet, and `Critical Path` in the halo bullet. The other six lenses
//! appear in no bullet at all.
//!
//! So [`MapLens::claimed_channel`] returns `Some` for exactly those four, and
//! `lens_channel_claims_are_named_in_the_encoding_bullets` derives both halves
//! by searching each lens's own spec name in each bullet of the design document,
//! in both directions. Nothing here decides that `Coursework` has no channel;
//! the document does, and the test fails if the document changes its mind.
//!
//! One consequence is worth stating because it is the thing section 26.3 warns
//! about: **the specification's own example composition collides.** Its
//! `Base: Knowledge State / Overlay 1: Project A / Overlay 2: Open Questions`
//! puts `Project` and `Question` on the same glyph, so
//! `layer_collision_warns_and_pins_legend` runs that exact composition.

use std::collections::{BTreeMap, BTreeSet};

use academic_domain::{EntityId, predicates::NodeType};
use serde::Serialize;

use crate::{
    CsMapError,
    encoding::{LensRelevance, VISUAL_CHANNELS, VisualChannel},
};

/// Section 25.3's ten lenses, in the specification's own reading order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MapLens {
    /// `Knowledge`.
    Knowledge,
    /// `Freshness`.
    Freshness,
    /// `Coursework`.
    Coursework,
    /// `Current Semester`.
    CurrentSemester,
    /// `Project`.
    Project,
    /// `Career`.
    Career,
    /// `Question`.
    Question,
    /// `Critical Path`.
    CriticalPath,
    /// `Blind Spot`.
    BlindSpot,
    /// `Graduation`.
    Graduation,
}

/// The ten lenses, in section 25.3's order.
pub const MAP_LENSES: [MapLens; 10] = [
    MapLens::Knowledge,
    MapLens::Freshness,
    MapLens::Coursework,
    MapLens::CurrentSemester,
    MapLens::Project,
    MapLens::Career,
    MapLens::Question,
    MapLens::CriticalPath,
    MapLens::BlindSpot,
    MapLens::Graduation,
];

/// The most overlays section 26.3 admits beside the base lens.
pub const MAX_OVERLAYS: usize = 2;

impl MapLens {
    /// The lens's own words in section 25.3's list.
    #[must_use]
    pub const fn spec_name(self) -> &'static str {
        match self {
            Self::Knowledge => "Knowledge",
            Self::Freshness => "Freshness",
            Self::Coursework => "Coursework",
            Self::CurrentSemester => "Current Semester",
            Self::Project => "Project",
            Self::Career => "Career",
            Self::Question => "Question",
            Self::CriticalPath => "Critical Path",
            Self::BlindSpot => "Blind Spot",
            Self::Graduation => "Graduation",
        }
    }

    /// The stable wire discriminant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Knowledge => "KNOWLEDGE",
            Self::Freshness => "FRESHNESS",
            Self::Coursework => "COURSEWORK",
            Self::CurrentSemester => "CURRENT_SEMESTER",
            Self::Project => "PROJECT",
            Self::Career => "CAREER",
            Self::Question => "QUESTION",
            Self::CriticalPath => "CRITICAL_PATH",
            Self::BlindSpot => "BLIND_SPOT",
            Self::Graduation => "GRADUATION",
        }
    }

    /// The section 26.2 channel whose bullet names this lens, when one does.
    ///
    /// Four of the ten are named by a bullet. The other six are not, and `None`
    /// is that fact rather than an omission: a lens that claims no channel never
    /// collides with anything, which is why a composition of six of these ten
    /// can be built freely and one of the remaining four cannot.
    #[must_use]
    pub const fn claimed_channel(self) -> Option<VisualChannel> {
        match self {
            Self::Freshness => Some(VisualChannel::OuterRing),
            Self::Project | Self::Question => Some(VisualChannel::Glyph),
            Self::CriticalPath => Some(VisualChannel::Halo),
            Self::Knowledge
            | Self::Coursework
            | Self::CurrentSemester
            | Self::Career
            | Self::BlindSpot
            | Self::Graduation => None,
        }
    }
}

/// What a lens is asked about one node.
///
/// Every field is the caller's own reading of what each lens names. There is no
/// mastery here, no freshness band and no confidence: [`relevance_of`] cannot
/// see any of them, which is the signature half of
/// `opacity_tracks_relevance_not_mastery`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensSubject {
    /// The node asked about.
    pub node: EntityId,
    /// Section 7.1's type of that node.
    pub node_type: NodeType,
    /// The lenses this node is directly about.
    pub named_by: BTreeSet<MapLens>,
    /// The lenses that reach this node through one relation.
    pub reached_by: BTreeSet<MapLens>,
}

/// How relevant `subject` is to `lens`.
///
/// The **only** producer of a [`LensRelevance`] in this crate. Its parameters
/// are a lens and a subject, and no mastery, freshness, confidence or claim
/// status is reachable from either.
#[must_use]
pub fn relevance_of(lens: MapLens, subject: &LensSubject) -> LensRelevance {
    if subject.named_by.contains(&lens) {
        LensRelevance::Central
    } else if subject.reached_by.contains(&lens) {
        LensRelevance::Related
    } else if subject.named_by.is_empty() && subject.reached_by.is_empty() {
        LensRelevance::Outside
    } else {
        LensRelevance::Peripheral
    }
}

/// A base lens with up to two overlays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensComposition {
    base: MapLens,
    overlays: Vec<MapLens>,
}

impl LensComposition {
    /// Starts a composition from its base lens.
    #[must_use]
    pub const fn base(base: MapLens) -> Self {
        Self {
            base,
            overlays: Vec::new(),
        }
    }

    /// Adds one overlay, consuming the composition.
    ///
    /// # Errors
    ///
    /// * [`CsMapError::ThirdOverlayRejected`] when two overlays are already
    ///   present. Section 26.3 admits `보조 overlay 두 개까지만`.
    /// * [`CsMapError::LensAlreadyComposed`] when the lens is already the base
    ///   or already an overlay. This one is **this crate's** decision and not
    ///   section 26.3's: the specification says nothing about a repeat, and a
    ///   composition that spent one of its two overlay slots redrawing the base
    ///   would report a collision with itself.
    pub fn overlay(self, lens: MapLens) -> Result<Self, CsMapError> {
        if self.base == lens || self.overlays.contains(&lens) {
            return Err(CsMapError::LensAlreadyComposed {
                lens: lens.as_str(),
            });
        }
        if self.overlays.len() >= MAX_OVERLAYS {
            return Err(CsMapError::ThirdOverlayRejected {
                base: self.base.as_str(),
                first: self.overlays[0].as_str(),
                second: self.overlays[1].as_str(),
                refused: lens.as_str(),
            });
        }
        let mut overlays = self.overlays;
        overlays.push(lens);
        Ok(Self {
            base: self.base,
            overlays,
        })
    }

    /// The base lens.
    ///
    /// Opacity is a function of this and of nothing else, which is why there is
    /// exactly one of it.
    #[must_use]
    pub const fn base_lens(&self) -> MapLens {
        self.base
    }

    /// The overlays, in the order they were added.
    #[must_use]
    pub fn overlays(&self) -> &[MapLens] {
        &self.overlays
    }

    /// The base followed by the overlays.
    #[must_use]
    pub fn composed(&self) -> Vec<MapLens> {
        let mut all = vec![self.base];
        all.extend(self.overlays.iter().copied());
        all
    }

    /// Opacity for one subject under this composition.
    ///
    /// It reads [`Self::base_lens`] and never an overlay: section 26.2 says
    /// opacity is `현재 lens relevance`, singular, and a composition has exactly
    /// one current lens.
    #[must_use]
    pub fn relevance(&self, subject: &LensSubject) -> LensRelevance {
        relevance_of(self.base, subject)
    }

    /// The collision this composition produces, when it produces one.
    ///
    /// A collision is two or more composed lenses claiming one channel. It is
    /// reported rather than refused: section 26.3 says
    /// `layer collision warning을 주고 legend를 고정한다`, which is a warning and
    /// a pin, not a rejection.
    #[must_use]
    pub fn collision(&self) -> Option<LayerCollision> {
        let mut by_channel: BTreeMap<VisualChannel, Vec<MapLens>> = BTreeMap::new();
        for lens in self.composed() {
            if let Some(channel) = lens.claimed_channel() {
                by_channel.entry(channel).or_default().push(lens);
            }
        }
        by_channel
            .into_iter()
            .find(|(_, claimants)| claimants.len() > 1)
            .map(|(channel, claimants)| LayerCollision { channel, claimants })
    }

    /// The legend for this composition.
    ///
    /// Every one of section 26.2's eight channels is listed, always, in section
    /// 26.2's order: a legend that hid a channel would defeat the redundancy
    /// section 26.2 requires of every encoding. What a collision changes is that
    /// the legend is **pinned** — see [`Legend::is_pinned`].
    #[must_use]
    pub fn legend(&self) -> Legend {
        let mut by_channel: BTreeMap<VisualChannel, Vec<MapLens>> = BTreeMap::new();
        for lens in self.composed() {
            if let Some(channel) = lens.claimed_channel() {
                by_channel.entry(channel).or_default().push(lens);
            }
        }
        let entries = VISUAL_CHANNELS
            .into_iter()
            .map(|channel| LegendEntry {
                channel,
                claimed_by: by_channel.get(&channel).cloned().unwrap_or_default(),
            })
            .collect();
        Legend {
            entries,
            pinned: self.collision().is_some(),
        }
    }
}

/// Two or more composed lenses drawing on one channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayerCollision {
    /// The channel they share.
    pub channel: VisualChannel,
    /// The lenses claiming it, in composition order.
    pub claimants: Vec<MapLens>,
}

/// One row of the legend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegendEntry {
    /// The channel this row explains.
    pub channel: VisualChannel,
    /// The composed lenses drawing on it, in composition order. Empty when the
    /// channel is not claimed by any composed lens, which is most of them.
    #[serde(rename = "claimedBy")]
    pub claimed_by: Vec<MapLens>,
}

/// The legend of one composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Legend {
    entries: Vec<LegendEntry>,
    pinned: bool,
}

impl Legend {
    /// The rows, in section 26.2's channel order.
    #[must_use]
    pub fn entries(&self) -> &[LegendEntry] {
        &self.entries
    }

    /// The channels, in order.
    #[must_use]
    pub fn channels(&self) -> Vec<VisualChannel> {
        self.entries.iter().map(|entry| entry.channel).collect()
    }

    /// Whether this legend is pinned open.
    ///
    /// True exactly when the composition collides. The shell that honours it —
    /// a viewer able to collapse an unpinned legend — is `P2-X6`'s and is not in
    /// this crate: what is fixed here is *which* compositions pin it and that
    /// the row order never moves.
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        self.pinned
    }
}
