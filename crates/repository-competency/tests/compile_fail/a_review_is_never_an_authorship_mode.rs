//! Section 17.6's fourth bullet, as a value that does not exist.
//!
//! A personal claim serializes what the user did into an `AuthorshipMode`, and
//! that enumeration names two things. `REVIEWED` and `READ` are
//! `ContributionKind` values a connector may report and they have no spelling
//! here, so a review cannot be written into the field a claim reads its
//! authorship out of — not rejected at runtime, absent.

use academic_repository_competency::AuthorshipMode;

fn main() {
    let _reviewed = AuthorshipMode::Reviewed;
    let _read = AuthorshipMode::Read;
}
