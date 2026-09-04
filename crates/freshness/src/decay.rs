//! Section 13.3's `시간 decay는 freshness projection에만 적용한다`.
//!
//! ## The separation is the signature
//!
//! [`decay`] takes an elapsed span and a persistence window. That is all it
//! takes and all it can take: **this crate has no name for a mastery level.**
//! `academic-knowledge-state` re-exports `LADDER`, `rung`, `level_token`,
//! `AutomaticLevel` and `MasteryProjection`, this crate imports none of them,
//! and `academic_domain::MasteryLevel` is not among its `use` items either.
//! `time_decay_touches_freshness_only` observes that as a whole-set comparison
//! rather than as a token search, and
//! `crates/freshness/tests/compile_fail/decay_cannot_take_a_mastery.rs` observes
//! the same thing as a compiler diagnostic.
//!
//! So *time decay touches freshness only* is not a rule inside a function that a
//! later edit could widen. It is a property of the dependency graph: the value
//! this crate hands back to `P2-N2` is a `FreshnessInput`, which carries a band
//! and a confidence and has no third field, and every other route from here to a
//! stored mastery would first need a mastery to name.
//!
//! ## One band per doubling
//!
//! Five of the six bands are computed from elapsed time; the sixth,
//! `UNKNOWN`, is not a computation at all — it is what a concept reads when
//! nothing datable was ever admitted about it, and [`decay`] is never reached
//! for one. The five split at 0.5, 1, 2 and 4 windows, one boundary per
//! doubling. There is no finer structure because section 13.3 licenses none: it
//! names the inputs and says a half-life is a prior rather than a truth, and the
//! prior is [`crate::persistence::UNCALIBRATED_PRIOR_V1`], where `GATE-38-024`
//! is.

use academic_domain::FreshnessBand;

use crate::persistence::PersistenceWindow;

/// Elapsed time as permille of a window, saturating at `i64::MAX`.
///
/// Saturating is the safe direction: a span so long that the multiplication
/// overflows is a span far past the last boundary, and it lands in `STALE`.
#[must_use]
fn permille_of_window(elapsed_millis: i64, window: PersistenceWindow) -> i64 {
    elapsed_millis
        .saturating_mul(1000)
        .checked_div(window.millis())
        .unwrap_or(i64::MAX)
}

/// The band `elapsed_millis` of disuse leaves, given a persistence window.
///
/// Takes a span and a window. There is no argument here through which a mastery
/// level, a projection or an assertion could arrive, and no return value that
/// could carry one back.
#[must_use]
pub fn decay(elapsed_millis: i64, window: PersistenceWindow) -> FreshnessBand {
    match permille_of_window(elapsed_millis, window) {
        ..500 => FreshnessBand::VeryHigh,
        500..1000 => FreshnessBand::High,
        1000..2000 => FreshnessBand::Moderate,
        2000..4000 => FreshnessBand::Low,
        4000.. => FreshnessBand::Stale,
    }
}
