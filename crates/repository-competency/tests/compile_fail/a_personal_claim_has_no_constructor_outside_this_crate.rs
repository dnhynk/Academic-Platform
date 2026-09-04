//! A personal claim is what `promote` derived, and nothing else may write one.
//!
//! `PersonalApplicationClaim::seal` is crate-private and every field is
//! private, so a caller cannot mint a `User APPLIED Concept` beside the
//! evidence that would have justified it — which is the whole separation
//! section 17.6's last sentence draws.

use academic_repository_competency::PersonalApplicationClaim;

fn main() {
    let _default = PersonalApplicationClaim::default();
    let _sealed = PersonalApplicationClaim::seal();
}
