//! The explicit aggregation section 29.5 requires has no public constructor.
//!
//! If a claim could be written by hand, the named method would be a field a
//! caller fills in rather than the thing `AggregationClaim::asserting` makes
//! them state, and a promotion could be reached without asserting anything.

use academic_review::AggregationClaim;

fn main() {
    let _forged = AggregationClaim {};
}
