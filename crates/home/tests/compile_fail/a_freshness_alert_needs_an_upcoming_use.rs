//! `a_freshness_alert_needs_an_upcoming_use`.
//!
//! Section 25.2's eighth line: `실제 upcoming use가 있을 때만`. The alert takes
//! an `UpcomingUse` by value, its fields are private, and `UpcomingUse` has one
//! fallible constructor, so an alert with nothing behind it is not writable.

use academic_domain::{FreshnessBand, TimestampMillis};
use academic_home::{FreshnessAlert, ScheduledOccasion, UpcomingUse};

fn main() {
    let concept: academic_domain::EntityId = "0190ffff-0000-7000-8000-000000000002"
        .parse()
        .unwrap_or_else(|_| panic!("a valid identifier"));

    // There is no constructor that leaves the upcoming use out.
    let _unjustified = FreshnessAlert::raise(concept, FreshnessBand::Stale);

    // Nor a struct literal that could omit it.
    let _assembled = FreshnessAlert {
        concept,
        band: FreshnessBand::Stale,
    };

    // Nor a `Default` to obtain one from.
    let _defaulted: FreshnessAlert = FreshnessAlert::default();

    // And an upcoming use is not assembled either: its fields are private, so
    // an occasion in the past cannot be dressed up as one.
    let _forged = UpcomingUse {
        occasion: ScheduledOccasion::Class,
        at: TimestampMillis::new(0),
    };
}
