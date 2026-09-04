//! The fail-closed rule, as a value that does not exist.
//!
//! `RedactionMode::Automatic` carries an `AccuracyWitness` by value. There is
//! no unit form of the variant and no argument that stands in for a
//! measurement, so an automatic redaction claim made without one is a program
//! that does not compile.

use academic_student_voice::RedactionMode;

fn main() {
    let _claimed = RedactionMode::Automatic();
    let _asserted = RedactionMode::Automatic(967);
}
