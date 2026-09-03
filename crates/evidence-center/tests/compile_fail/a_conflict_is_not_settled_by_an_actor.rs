//! `a_conflict_is_not_settled_by_an_actor`.
//!
//! Section 30.4: *`사용자가 유지·수정·scope 종료를 선택한다`*. `settle` takes
//! `P2-M2`'s `UserDecision`, which `UserDecision::by` issues only for
//! `Actor::User`, so there is no actor for `settle` to refuse and no way to
//! reach it with one. Every route past that door is tried here.

use academic_domain::{Actor, EntityId, TimestampMillis};
use academic_evidence_center::{ConflictCase, CorrectionOutcome, Resolution};

fn main() {
    let mut case = open_case();
    let model = Actor::ModelRun { run_id: run() };

    // An actor is not a receipt.
    case.settle(CorrectionOutcome::Keep, model, TimestampMillis::new(0));

    // A receipt is not something a caller assembles either. That case is
    // `academic-proposal`'s own `a_user_decision_cannot_be_assembled.rs`; it is
    // not repeated here, because a second copy would drift from the first.

    // And a resolution cannot be written onto a case: the field is private, and
    // there is no field at all -- the resolution is computed from the history.
    case.resolution = Resolution::Settled(academic_evidence_center::CorrectionChoice::Keep);
}

// Never reached: the lines above do not compile.
fn open_case() -> ConflictCase {
    unimplemented!()
}

fn run() -> EntityId {
    unimplemented!()
}
