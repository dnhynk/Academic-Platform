//! The one door between them is `ContributionKind::authorship_mode`, which
//! answers `None` for a review.
//!
//! There is no conversion beside it: a `ContributionKind` does not coerce, does
//! not `Into`, and is not the type any constructor here takes where an
//! `AuthorshipMode` belongs. So the two vocabularies cannot be confused by a
//! caller who never reads the table.

use academic_repository_competency::{AuthorshipMode, ContributionKind};

fn takes_a_mode(_mode: AuthorshipMode) {}

fn main() {
    takes_a_mode(ContributionKind::Reviewed);
    let _converted: AuthorshipMode = ContributionKind::Authored.into();
}
