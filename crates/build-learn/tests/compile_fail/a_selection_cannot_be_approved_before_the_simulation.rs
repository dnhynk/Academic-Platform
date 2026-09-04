//! Section 20.2's four-step order, held by three by-value arguments.
//!
//! `SelectionApproved::after` takes a `SimulationPassed`, which takes an
//! `ExplainedByHand`, which takes a `ReadingDone`. Approving a selection
//! straight after the explanation skips `최소 simulation test`, and there is no
//! value of the right type to pass.

use academic_build_learn::{ExplainedByHand, PartId, SelectionApproved};

fn skip_the_test(
    explained: ExplainedByHand,
    decision: PartId,
    alternative: PartId,
) -> SelectionApproved {
    SelectionApproved::after(explained, decision, alternative)
}

fn main() {
    let _ = skip_the_test;
}
