//! A course-level value cannot be made out of offering aggregates alone.
//!
//! `CourseAggregate::promote` takes the claim first. Handing it the aggregates
//! is the shape a promotion without an explicit aggregation would have, and it
//! is a type error rather than a check.

use academic_review::{BiasDisclosure, CourseAggregate, OfferingAggregate};

fn promote(aggregates: &[OfferingAggregate], disclosure: BiasDisclosure) {
    let _ = CourseAggregate::promote(aggregates, aggregates, disclosure);
}

fn main() {}
