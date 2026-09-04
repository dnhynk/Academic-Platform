//! Section 20.1's `기술 목록`, and the order the type system enforces.
//!
//! > 시스템은 이를 바로 기술 목록으로 바꾸지 않고 성공 조건과 선택 지점을
//! > 추출한다.
//!
//! ## `criteria_and_choices_precede_technology` is a chain of by-value arguments
//!
//! ```text
//! GoalInput  --normalize-->  NormalizedIntent
//!                                  |  ProjectGoal::state(&intent, SuccessCriteria, ..)
//!                                  v      ^ by value, `of` returns None for []
//!                            ProjectGoal
//!                                  |  TechnologySlate::under(&goal)
//!                                  v
//!                            TechnologySlate
//! ```
//!
//! [`TechnologySlate`] has private fields, no `Default`, no public field and
//! exactly one producer: [`TechnologySlate::under`], which takes a
//! [`crate::goal::ProjectGoal`]. A `ProjectGoal` cannot be stated without a
//! [`crate::goal::SuccessCriteria`], and that value cannot be built from an
//! empty list. So a technology list that precedes the criteria is not a check
//! that fails — it is a program that does not compile.
//!
//! `crates/build-learn/tests/compile_fail/` holds the compiled half, and
//! `every_public_signature_is_in_the_inventory` holds the other half a name list
//! cannot: a **second** producer added later would be an entry in that pinned
//! inventory whatever it was called, and
//! `the_only_producer_of_a_technology_slate_takes_a_goal` compares the whole set
//! of public functions returning one against exactly that producer.
//!
//! ## Every entry is conditional on a decision
//!
//! A slate entry is not a recommendation. It names one alternative of one open
//! decision, and carries the decision it belongs to, so the answer to `what
//! technology` is always `one of these, once you decide that`. There is no entry
//! that is not conditional, because [`TechnologySlate::under`] reads
//! [`crate::goal::UnresolvedDecisions`] and has no other source. A goal with no
//! open decision yields an **empty** slate, which is the design document's own
//! position: the system does not hand back a technology list it was not asked
//! to choose.

use serde::{Deserialize, Serialize};

use crate::{
    goal::ProjectGoal,
    text::{NonEmptyText, PartId},
};

/// One named technology, and the open decision it is an alternative of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TechnologyEntry {
    decision: PartId,
    alternative: PartId,
    name: NonEmptyText,
}

impl TechnologyEntry {
    /// Which decision this is an alternative of.
    #[must_use]
    pub const fn decision(&self) -> &PartId {
        &self.decision
    }

    /// Which alternative of that decision it is.
    #[must_use]
    pub const fn alternative(&self) -> &PartId {
        &self.alternative
    }

    /// What it is called.
    #[must_use]
    pub const fn name(&self) -> &NonEmptyText {
        &self.name
    }
}

/// Section 20.1's `기술 목록`, derivable only from a stated goal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TechnologySlate {
    entries: Vec<TechnologyEntry>,
}

impl TechnologySlate {
    /// Every alternative of every open decision of `goal`, in the goal's order.
    ///
    /// The one producer. See the module note.
    #[must_use]
    pub fn under(goal: &ProjectGoal) -> Self {
        let mut entries = Vec::new();
        for decision in goal.unresolved_decisions().decisions() {
            for alternative in decision.alternatives() {
                entries.push(TechnologyEntry {
                    decision: decision.id().clone(),
                    alternative: alternative.id().clone(),
                    name: alternative.name().clone(),
                });
            }
        }
        Self { entries }
    }

    /// The entries, in the goal's declaration order.
    #[must_use]
    pub fn entries(&self) -> &[TechnologyEntry] {
        &self.entries
    }

    /// Whether the goal offered no open decision at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
