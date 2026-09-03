//! Section 18.2's fifth step has no value meaning *the user already knows it*.
//!
//! A concept whose evidence the user has, current and confirmed, is not a
//! concept this project requires them to learn. `UserEvidenceGap` therefore has
//! exactly two variants and its one constructor answers `None` for a state that
//! is neither — so the refusal is a value that does not exist rather than a
//! comparison somebody performs, and a caller cannot name their way past it.

use academic_domain::MasteryLevel;
use academic_repository_classification::UserEvidenceGap;

fn main() {
    let _sufficient = UserEvidenceGap::Sufficient {
        mastery: MasteryLevel::Fluent,
    };
    let _none = UserEvidenceGap::None;
}
