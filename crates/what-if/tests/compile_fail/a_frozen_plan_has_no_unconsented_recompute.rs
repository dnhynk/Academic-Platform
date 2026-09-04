//! Section 22.5: `재계산 동의를 받는다`.
//!
//! `FrozenPlan::recompute` takes a `RecomputeConsent`, whose one constructor
//! takes `P2-M2`'s `UserDecision`. A recomputation without a consent is not a
//! call that fails; it is a call with an argument missing.

use academic_what_if::FrozenPlan;

fn recompute(frozen: FrozenPlan, inputs: &academic_what_if::PlanInputs) {
    let _ = frozen.recompute(inputs);
}

fn main() {
    let _ = recompute;
}
