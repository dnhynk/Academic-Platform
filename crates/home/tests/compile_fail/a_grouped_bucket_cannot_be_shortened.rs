//! `a_grouped_bucket_cannot_be_shortened`.
//!
//! Section 25.2: `숨기지 않고 … 묶는다`. Grouping hands back three borrowed
//! slices of lists the value owns, so there is nothing for a caller to
//! truncate, drain or retain over, and no mutable route to the lists at all.

use academic_home::{AlertBucket, DayWindow, GroupedAlerts};

fn main() {
    let window = DayWindow::new(
        academic_domain::TimestampMillis::new(0),
        academic_domain::TimestampMillis::new(1),
    )
    .unwrap_or_else(|_| panic!("a well ordered day"));
    let grouped = GroupedAlerts::group(Vec::new(), window);

    // The accessor hands out a shared slice, so nothing on it can shorten it.
    grouped.bucket(AlertBucket::Today).truncate(0);

    // There is no mutable accessor to reach for instead.
    grouped.bucket_mut(AlertBucket::Soon).clear();

    // Nor a field to reach past the accessor.
    let _lists = grouped.today;

    // And the total is derived from the buckets, so it cannot be set apart
    // from them.
    grouped.set_total(0);
}
