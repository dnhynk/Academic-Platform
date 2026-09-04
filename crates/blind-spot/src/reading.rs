//! What one aggregation key looks like before it is classified.
//!
//! Everything here arrived from a boundary this crate reads and does not
//! compute: the admitted items are `P2-N2`'s, the band is `P2-N3`'s, the active
//! goals and the goal block are `P2-N5`'s, and the taste step is the one the
//! product offers when the user asks to explore. Nothing in this file decides
//! any of them.

use std::collections::BTreeSet;

use academic_domain::{EntityId, FreshnessBand};

use crate::{coverage::ExposureItem, relevance::GoalRelevance, state::GoalBlock, taste::TasteStep};

/// One key's inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyReading {
    key: EntityId,
    items: Vec<ExposureItem>,
    band: Option<FreshnessBand>,
    relevance: GoalRelevance,
    goal_block: Option<GoalBlock>,
    taste_step: Option<TasteStep>,
}

impl KeyReading {
    /// A key with its admitted items and nothing else known about it.
    #[must_use]
    pub const fn of(key: EntityId, items: Vec<ExposureItem>) -> Self {
        Self {
            key,
            items,
            band: None,
            relevance: GoalRelevance::of(BTreeSet::new()),
            goal_block: None,
            taste_step: None,
        }
    }

    /// Carries `P2-N3`'s band for this key's newest admitted evidence.
    #[must_use]
    pub const fn with_band(mut self, band: FreshnessBand) -> Self {
        self.band = Some(band);
        self
    }

    /// Carries which active goals reach this key.
    #[must_use]
    pub fn with_relevance(mut self, relevance: GoalRelevance) -> Self {
        self.relevance = relevance;
        self
    }

    /// Carries `P2-N5`'s finding that an active goal is blocked here.
    #[must_use]
    pub const fn with_goal_block(mut self, block: GoalBlock) -> Self {
        self.goal_block = Some(block);
        self
    }

    /// Offers the one step an `EXPLORE` choice would open.
    #[must_use]
    pub const fn with_taste_step(mut self, step: TasteStep) -> Self {
        self.taste_step = Some(step);
        self
    }

    /// Which key.
    #[must_use]
    pub const fn key(&self) -> EntityId {
        self.key
    }

    /// The admitted items offered for it.
    #[must_use]
    pub fn items(&self) -> &[ExposureItem] {
        &self.items
    }

    /// `P2-N3`'s band, if one was carried.
    #[must_use]
    pub const fn band(&self) -> Option<FreshnessBand> {
        self.band
    }

    /// Which active goals reach it.
    #[must_use]
    pub const fn relevance(&self) -> &GoalRelevance {
        &self.relevance
    }

    /// `P2-N5`'s goal block, if one was carried.
    #[must_use]
    pub const fn goal_block(&self) -> Option<GoalBlock> {
        self.goal_block
    }

    /// The offered taste step, if one was.
    #[must_use]
    pub const fn taste_step(&self) -> Option<TasteStep> {
        self.taste_step
    }
}
