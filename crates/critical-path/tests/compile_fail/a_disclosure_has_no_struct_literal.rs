//! Section 16.5: `계산 snapshot, 비용 가정, 제외된 목표, 불확실 edge, 대안이
//! 항상 노출된다`. `Disclosure` has five private fields and one constructor
//! taking all five, so a disclosure assembled with a group missing has no
//! literal to be assembled by.

use academic_critical_path::{ComputationSnapshot, CostAssumptions, Disclosure};

fn partial(snapshot: ComputationSnapshot, assumptions: CostAssumptions) -> Disclosure {
    Disclosure {
        snapshot,
        cost_assumptions: assumptions,
    }
}

fn main() {}
