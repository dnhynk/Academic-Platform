//! `RootCandidate::of` refuses an explanation about another concept or another
//! kind. Its fields are private, so a candidate carrying somebody else's
//! explanation has no representation.

use academic_gap::RootCandidate;

fn main() {
    let _ = RootCandidate {
        reason: String::new(),
    };
}
