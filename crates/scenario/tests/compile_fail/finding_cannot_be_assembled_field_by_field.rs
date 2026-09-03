//! `P2-R2`'s finding has no constructor outside the ladder that derives it.
//!
//! `REQ-34-091` is *a new finding cannot default to repository-wide scope*.
//! One half of that is the absence of a repository variant, which
//! `finding_scope_cannot_name_the_repository` covers. This is the other half:
//! there is no way to build a `Finding` at all except
//! `EvidenceLadder::classify`, which derives the scope from the evidence rather
//! than taking it as an argument something could default.
//!
//! The three shapes a caller would reach for — a `Default`, a struct literal,
//! and a `new` — are each absent, and this case fails if any one of them
//! appears.

use academic_repository_analysis::Finding;

fn main() {
    let _by_default: Finding = Finding::default();
    let _by_literal = Finding {
        snapshot_id: "snap_a".to_owned(),
    };
    let _by_new = Finding::new();
}
