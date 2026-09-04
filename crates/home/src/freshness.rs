//! Section 25.2's eighth line: `freshness 알림은 실제 upcoming use가 있을 때만`.
//!
//! # The rule is held by there being no other constructor
//!
//! [`FreshnessAlert::raise`] takes a [`crate::UpcomingUse`] by value. The
//! fields are private, there is no `Default`, no setter, and no second
//! constructor, so there is no state in which an alert exists and no upcoming
//! use justifies it. `tests/compile_fail/a_freshness_alert_needs_an_upcoming_use.rs`
//! is the compiled half, and `freshness_alert_requires_an_upcoming_use` drives
//! the behaviour: an occasion that is not ahead of the reference instant is
//! refused one step earlier, by [`crate::UpcomingUse::declare`], so the alert
//! has nothing to be built from.
//!
//! # Why this matters beyond this screen
//!
//! `P2-N3` fixes that time decay reaches a freshness projection and never a
//! mastery, and that a `STALE` band is a statement about immediate retrieval
//! rather than a demotion. A first screen that raised `you have forgotten this`
//! on a timer would make that discipline invisible whatever the crate below it
//! did, because the timer would be the thing the user actually experienced.
//! Requiring an occasion is what keeps the alert a statement about *use*.
//!
//! # This crate cannot name a mastery
//!
//! [`FreshnessAlert`] carries `academic_domain::FreshnessBand` and nothing
//! else about the user's state. `academic_domain::MasteryLevel` is one `use`
//! away and is not written anywhere in this crate;
//! `the_home_surface_cannot_name_a_mastery` is what measures that, with the
//! same shape and the same control `academic-freshness` uses.

use academic_domain::{EntityId, FreshnessBand};

use crate::UpcomingUse;

/// A freshness alert about one concept, with the use that justifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshnessAlert {
    concept: EntityId,
    band: FreshnessBand,
    upcoming: UpcomingUse,
}

impl FreshnessAlert {
    /// Raises an alert about a concept that is about to be needed.
    ///
    /// The upcoming use is a parameter, so the alert cannot exist without one.
    #[must_use]
    pub const fn raise(concept: EntityId, band: FreshnessBand, upcoming: UpcomingUse) -> Self {
        Self {
            concept,
            band,
            upcoming,
        }
    }

    /// The concept the alert is about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// `P2-N3`'s band, carried rather than recomputed.
    #[must_use]
    pub const fn band(&self) -> FreshnessBand {
        self.band
    }

    /// The use that made the alert admissible.
    #[must_use]
    pub const fn upcoming(&self) -> UpcomingUse {
        self.upcoming
    }
}
