//! `GATE-38-026`, as the absence of a variant.
//!
//! Whether student voices may be removed from an original recording is an open
//! decision for the user and their institution. `RedactionScope` has one value
//! and it is the derivative, so a policy authorising an edit to the original
//! has no spelling in this crate at all.

use academic_student_voice::RedactionScope;

fn main() {
    let _original = RedactionScope::Original;
    let _both = RedactionScope::OriginalAndDerivative;
}
