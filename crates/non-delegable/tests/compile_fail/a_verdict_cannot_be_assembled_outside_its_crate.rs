//! `a_verdict_cannot_be_assembled_outside_its_crate`.
//!
//! Section 27.2's ninth bullet forbids deciding graduation pass/fail by free
//! text generation. `graduation_result_cannot_come_from_generation` measures
//! the two halves a running test can reach — section 27.1 has no graduation row,
//! and a sentence cannot become a frozen engine input. This is the third half:
//! `academic_audit::DeterminateVerdict::new` is `pub(crate)`, so no crate
//! outside `P2-U3` can assemble a verdict from anything at all, generated or
//! otherwise.

use academic_audit::{DeterminateVerdict, GraduationOutcome};

fn main() {
    let _forged = DeterminateVerdict::new(
        GraduationOutcome::Possible,
        unimplemented!(),
        unimplemented!(),
        unimplemented!(),
    );
}
