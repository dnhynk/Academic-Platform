//! `P2-R4`'s proof chain has no constructor that takes fewer than five steps.
//!
//! Section 18.2's chain is five steps and `REQ-18-006` is that removing any one
//! of them blocks the publish. The runtime half of that is
//! `ChainDraft::seal`'s per-step missing code. This is the type half: there is
//! no way to build a `ProofChain` at all except `ProofChain::closed_by`, which
//! takes the fourth step by value — and the fourth step is only reachable
//! through the third, the second and the first.
//!
//! The three shapes a caller would reach for — a `Default`, a struct literal,
//! and a `new` — are each absent, and this case fails if any one of them
//! appears.

use academic_repository_classification::{ProofChain, UserEvidenceGap};

fn gap() -> UserEvidenceGap {
    loop {}
}

fn main() {
    let _by_default: ProofChain = ProofChain::default();
    let _by_literal = ProofChain { gap: gap() };
    let _by_new = ProofChain::new();
}
