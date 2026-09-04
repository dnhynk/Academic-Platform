//! Section 13.3: `전파는 한 단계`.
//!
//! A second hop needs a `NeighborUse`, and a `NeighborUse` is built from dated
//! evidence. A contribution one concept already received is not evidence about
//! it, so feeding one back in is a program that does not compile.

use academic_domain::TimestampMillis;
use academic_freshness::{NeighborUse, Spillover, UNCALIBRATED_PRIOR_V1};

fn second_hop(subject: academic_domain::EntityId, received: Spillover) -> Option<Spillover> {
    let carried: NeighborUse = received;
    Spillover::toward(subject, carried)
}

fn main() {
    let _ = second_hop;
    let _ = (TimestampMillis::new(0), &UNCALIBRATED_PRIOR_V1);
}
