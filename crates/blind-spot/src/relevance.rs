//! Section 23's `relevanceToActiveGoals`, and section 34.5's `goal relevance`.
//!
//! The design document exhibits one token, `LOW`, in the schema example.
//! [`GoalRelevance`] is therefore not an invented ordinal scale: it carries the
//! active goals that reach this key and `LOW` is the empty case, which is the
//! only reading under which the example's `LOW` and its `likelyCause` — course
//! and project choices concentrated somewhere else — are the same fact.
//! `RELATED` is this crate's name for the complement, and
//! `docs/contracts/blind-spot-detector.md` records that the document does not
//! spell it.
//!
//! Which goals are active is not this crate's question. `P2-N5` owns
//! `ActiveGoal`, this crate has no edge to it, and the identities arrive as an
//! argument — which is also what makes `모든 분야를 균등하게 채우라는 목표를
//! 만들지 않는다` a graph fact: a goal this engine emitted would first have to
//! be a goal it could name.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use academic_domain::EntityId;

/// How far one aggregation key is from what the user is currently working on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalRelevance {
    citing_goals: BTreeSet<EntityId>,
}

impl GoalRelevance {
    /// Records which active goals reach this key.
    #[must_use]
    pub const fn of(citing_goals: BTreeSet<EntityId>) -> Self {
        Self { citing_goals }
    }

    /// No active goal reaches this key.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            citing_goals: BTreeSet::new(),
        }
    }

    /// The goals that reach it, in identity order.
    #[must_use]
    pub const fn citing_goals(&self) -> &BTreeSet<EntityId> {
        &self.citing_goals
    }

    /// Section 23's `LOW`: no active goal reaches this key.
    #[must_use]
    pub fn is_low(&self) -> bool {
        self.citing_goals.is_empty()
    }

    /// Stable spelling. `LOW` is the document's; `RELATED` is this crate's name
    /// for its complement.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        if self.is_low() { "LOW" } else { "RELATED" }
    }
}
