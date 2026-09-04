//! `historically_likely_cannot_enter_determinate_plan`, as an absence.
//!
//! Section 8.3's Planner 취급 cell for `HISTORICALLY_LIKELY` is
//! *placeholder만, 졸업계획 확정에 사용 금지*. `ConfirmedSeat` has one producer
//! -- `ConfirmedStanding::seat` -- and the other three standings have no such
//! method, so there is no run-time refusal here: there is nothing to refuse.

use academic_offering::{CancelledStanding, HistoricallyLikelyStanding, UncertainStanding};

fn seats(
    likely: &HistoricallyLikelyStanding,
    uncertain: &UncertainStanding,
    cancelled: &CancelledStanding,
) {
    let _from_likely = likely.seat();
    let _from_uncertain = uncertain.seat();
    let _from_cancelled = cancelled.seat();
}

fn main() {}
