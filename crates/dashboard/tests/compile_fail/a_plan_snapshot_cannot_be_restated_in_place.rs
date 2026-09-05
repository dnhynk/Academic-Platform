//! `a_plan_snapshot_cannot_be_restated_in_place`.
//!
//! Section 25.5: 안 A/B/C를 고정 snapshot으로 저장하고, 공식 정보가 바뀌면
//! 무엇이 stale해졌는지만 표시한다. A snapshot is fixed, so every route that
//! would move one is tried here.
//!
//! The snapshot arrives as a parameter rather than out of `save`, which returns
//! a `Result`: an error reported against `Result<PlanSnapshot, _>` would say
//! nothing about `PlanSnapshot`, and the first version of this case did exactly
//! that. The private-field probe is not here either: E0451 is reported by the
//! privacy pass, which does not run once type checking has failed, so a literal
//! written beside these three method probes produces no diagnostic at all.
//! `no_academic_surface_type_has_a_public_field` is that probe, alone in a case
//! of its own where it is the only error and does report.

use academic_dashboard::{CandidateOffering, PlanSnapshot, StaleMarking};

fn routes(snapshot: &mut PlanSnapshot) {
    // There is no setter for the label.
    snapshot.set_label("안 B");

    // Nor a mutable view of what was placed.
    let _placed: &mut Vec<CandidateOffering> = snapshot.placed_mut();

    // Nor a `restate` that takes the snapshot by mutable reference and applies
    // what it found.
    let _applied: StaleMarking = snapshot.restate_in_place(&[]);

}

fn main() {}
