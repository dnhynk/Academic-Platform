//! `AuthoredWork` is what section 17.6's checks produced, and there is exactly
//! one thing that produces it.
//!
//! Every field is private, there is no `Default`, and `ContributionDraft::seal`
//! is the only constructor. A caller holding a report and an opinion cannot
//! write the result the checks would have written.

use academic_repository_competency::{AuthoredWork, AuthorshipMode, ChangeId, UserId};

fn main() {
    let _literal = AuthoredWork {
        change: ChangeId::new("c-1").unwrap(),
        snapshot_id: String::new(),
        user: UserId::new("user-1").unwrap(),
        mode: AuthorshipMode::Authored,
    };
    let _default = AuthoredWork::default();
    let _new = AuthoredWork::new();
}
