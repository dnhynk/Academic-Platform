//! What happened to a score, kept rather than undone.
//!
//! Section 34.5's `career readiness 과도한 점수화` row gives two recoveries —
//! `score 숨김` and `가중치 초기화` — and section 34.6's first principle is that
//! the original is preserved. This repository holds that everywhere: `P2-Y2`'s
//! fork does not touch its base, `P2-N2`'s assertion is not changed in place,
//! and `P2-R5`'s claim is superseded rather than edited.
//!
//! So neither recovery is a mutation. [`crate::view::ReadinessView::hide_score`]
//! and [`crate::view::ReadinessView::reset_weights`] both take `&self` and
//! return a **new** view; nothing in this crate takes `&mut self` at all. The
//! prior score and the prior weighting travel into the new view's history, so
//! the numbers that were once displayed stay openable — which is what section
//! 34.6's fourth principle asks for, a correction marker on the past rather
//! than a past that quietly reads correct.
//!
//! There is no arm meaning *the score was deleted* and no function that shortens
//! a history. `score_hide_and_weight_reset_preserve_history` compares the two
//! histories as sequences and requires the older to be a prefix of the newer,
//! which a deletion could not satisfy.

use serde::Serialize;

use crate::score::{ScoreValue, WeightDisclosure};

/// One thing that happened to this view's auxiliary score.
///
/// Three arms and no fourth. Each carries what it replaced, so nothing is lost
/// by the event that records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReadinessEvent {
    /// A score was published, under this weighting.
    ScorePublished {
        /// The number that was published.
        value: ScoreValue,
        /// The weighting it was computed under.
        weights: WeightDisclosure,
    },
    /// The published score was hidden. It stays here.
    ScoreHidden {
        /// The number that stopped being displayed.
        value: ScoreValue,
        /// The weighting it had been computed under.
        weights: WeightDisclosure,
    },
    /// The weighting was reset, and the score recomputed under the new one.
    WeightsReset {
        /// What the weighting had been.
        from: WeightDisclosure,
        /// What it became.
        to: WeightDisclosure,
        /// The number under the old weighting.
        previous_value: ScoreValue,
    },
}

impl ReadinessEvent {
    /// Stable spelling of which of the three this is.
    ///
    /// Total, with no wildcard arm.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ScorePublished { .. } => "SCORE_PUBLISHED",
            Self::ScoreHidden { .. } => "SCORE_HIDDEN",
            Self::WeightsReset { .. } => "WEIGHTS_RESET",
        }
    }

    /// Every spelling, in this enumeration's own order.
    pub const KINDS: [&'static str; 3] = ["SCORE_PUBLISHED", "SCORE_HIDDEN", "WEIGHTS_RESET"];
}
