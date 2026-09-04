//! Section 16.2's `engine은 먼저 Pareto-dominated path를 제거하고`.
//!
//! ## `먼저` is a type, not a comment
//!
//! [`crate::preference::rank`] takes a `&`[`ParetoFront`]. A [`ParetoFront`] has
//! private fields and **one** constructor, [`ParetoFront::eliminate`], which
//! takes the whole candidate list and removes the dominated members. There is
//! no `ParetoFront::of`, no `From<Vec<Candidate>>`, no `Default` and no
//! `push`, so a caller cannot assemble a front that skipped the elimination and
//! hand it to the ranker.
//!
//! `dominated_paths_are_removed_before_ranking` therefore has two halves: the
//! behavioural one, that a strictly dominated candidate is absent from the
//! front and absent from every ranking; and the structural one, that no other
//! route into the ranker exists, which `crates/critical-path/tests/compile_fail/`
//! holds.
//!
//! ## Domination on intervals never forms a midpoint
//!
//! A cost axis of `A` weakly beats the same axis of `B` when **both** ends are
//! no worse: `A.low <= B.low && A.high <= B.high`. A benefit axis is the same
//! comparison reversed. Two overlapping intervals whose ends cross --
//! `[10, 40]` against `[20, 30]` -- are **incomparable**, and a candidate is not
//! dominated on that axis in either direction.
//!
//! That is what makes the relation genuinely partial, and it is the point: a
//! rule that collapsed each interval to its midpoint would make every pair
//! comparable and would delete an alternative on the strength of a number
//! nobody measured. `REQ-16-005` leaves `incomparable/uncertain vector
//! dominance` open as a gate candidate; this is the reading, and it errs toward
//! keeping a path.
//!
//! ## Elimination is not ranking
//!
//! Domination uses no preference at all -- it is a statement about the vectors,
//! true or false before any slider exists. That is why it runs first and why
//! `slider_changes_order_not_facts` can compare two rankings over the *same*
//! front.

use crate::{
    plan::Candidate,
    vector::{BENEFIT_COMPONENTS, COST_COMPONENTS},
};

/// How two candidates compare on the pair of vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dominance {
    /// The left candidate is no worse on every axis and strictly better on at
    /// least one.
    LeftDominates,
    /// The mirror.
    RightDominates,
    /// Neither: every axis agrees, or the axes disagree with each other, or an
    /// axis's intervals cross.
    Incomparable,
}

/// Whether one candidate Pareto-dominates the other.
///
/// Pure, total, and free of any preference. See the module note.
#[must_use]
pub fn dominance(left: &Candidate, right: &Candidate) -> Dominance {
    let mut left_better_somewhere = false;
    let mut right_better_somewhere = false;
    for component in COST_COMPONENTS {
        let a = left.cost().component(component);
        let b = right.cost().component(component);
        // Lower is better on a cost axis, on both ends.
        if a.low() <= b.low() && a.high() <= b.high() && (a.low() < b.low() || a.high() < b.high())
        {
            left_better_somewhere = true;
        } else if b.low() <= a.low()
            && b.high() <= a.high()
            && (b.low() < a.low() || b.high() < a.high())
        {
            right_better_somewhere = true;
        } else if a != b {
            // The intervals cross: neither is no-worse on this axis, so neither
            // candidate can dominate.
            return Dominance::Incomparable;
        }
    }
    for component in BENEFIT_COMPONENTS {
        let a = left.benefit().component(component);
        let b = right.benefit().component(component);
        // Higher is better on a benefit axis, on both ends.
        if a.low() >= b.low() && a.high() >= b.high() && (a.low() > b.low() || a.high() > b.high())
        {
            left_better_somewhere = true;
        } else if b.low() >= a.low()
            && b.high() >= a.high()
            && (b.low() > a.low() || b.high() > a.high())
        {
            right_better_somewhere = true;
        } else if a != b {
            return Dominance::Incomparable;
        }
    }
    match (left_better_somewhere, right_better_somewhere) {
        (true, false) => Dominance::LeftDominates,
        (false, true) => Dominance::RightDominates,
        (true, true) | (false, false) => Dominance::Incomparable,
    }
}

/// One candidate that elimination removed, and which candidate removed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dominated {
    candidate: Candidate,
    dominated_by: usize,
}

impl Dominated {
    /// The removed candidate. Kept, not discarded: section 16.5 discloses
    /// `제외된 목표`, and a path removed for being dominated is one of them.
    #[must_use]
    pub const fn candidate(&self) -> &Candidate {
        &self.candidate
    }

    /// The index into [`ParetoFront::candidates`] of the candidate that
    /// dominated it.
    #[must_use]
    pub const fn dominated_by(&self) -> usize {
        self.dominated_by
    }
}

/// The candidates that survive elimination, and the ones that did not.
///
/// One constructor. See the module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParetoFront {
    candidates: Vec<Candidate>,
    dominated: Vec<Dominated>,
}

impl ParetoFront {
    /// Removes every Pareto-dominated candidate.
    ///
    /// The surviving order is the input order, so this function decides
    /// membership and never rank. Ranking is [`crate::preference::rank`]'s job
    /// and it needs one of these to do it.
    #[must_use]
    pub fn eliminate(candidates: Vec<Candidate>) -> Self {
        let mut kept: Vec<Candidate> = Vec::new();
        let mut removed: Vec<(Candidate, usize)> = Vec::new();
        for (index, candidate) in candidates.iter().enumerate() {
            let mut dominator: Option<usize> = None;
            for (other_index, other) in candidates.iter().enumerate() {
                if other_index == index {
                    continue;
                }
                if dominance(other, candidate) == Dominance::LeftDominates {
                    dominator = Some(other_index);
                    break;
                }
            }
            match dominator {
                Some(by) => removed.push((candidate.clone(), by)),
                None => kept.push(candidate.clone()),
            }
        }
        // The recorded dominator index has to point into the surviving list, so
        // it is remapped once the survivors are known. A dominator is itself
        // never dominated -- domination is transitive over these vectors -- so
        // the lookup always resolves.
        let dominated = removed
            .into_iter()
            .map(|(candidate, original)| {
                let dominator = candidates
                    .get(original)
                    .and_then(|winner| kept.iter().position(|survivor| survivor == winner))
                    .unwrap_or(0);
                Dominated {
                    candidate,
                    dominated_by: dominator,
                }
            })
            .collect();
        Self {
            candidates: kept,
            dominated,
        }
    }

    /// The surviving candidates, in the order they were offered.
    #[must_use]
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// The candidates elimination removed, and what removed each.
    #[must_use]
    pub fn dominated(&self) -> &[Dominated] {
        &self.dominated
    }

    /// How many candidates survived.
    #[must_use]
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Whether nothing survived.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}
