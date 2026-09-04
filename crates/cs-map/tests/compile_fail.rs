//! Section 25.3's, 26.2's, 26.3's and 26.5's separations, held by compilation
//! rather than by a check.
//!
//! Every case here tries to do something this task exists to prevent, and every
//! one of them fails to compile. The suite passes only when each case fails
//! **and** fails with the committed diagnostic, so a case that stopped proving
//! anything — because a type grew a conversion, or because the case itself was
//! mistyped into a different error — is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `an_opacity_is_not_a_mastery` | a lens relevance built out of a mastery band |
//! | `a_mastery_is_not_an_opacity` | the same conversion the other way |
//! | `a_transition_is_not_a_change_origin` | a display setting recorded as canonical history |
//! | `a_refused_third_overlay_leaves_nothing_behind` | a composition retried after its refusal |
//! | `a_you_anchor_is_not_a_node` | `YOU` declared into the graph |
//! | `a_reveal_cannot_be_assembled_without_a_path` | a search result that teleports |
//! | `a_hop_count_is_not_a_number` | section 26.4's `1–3 hop` bypassed |
//! | `a_graph_cannot_be_edited_after_it_is_declared` | a node added behind the layout's back |
//!
//! One case per shape, for `P2-R4`'s measured reason: bundling two refusals into
//! one program produced **no** diagnostic for one of them, because the first
//! error suppressed the second. A bundled case can hide one of its own halves.

/// The section 25.3, 26.2, 26.3 and 26.5 separations that are types.
#[test]
fn an_opacity_a_transition_and_an_anchor_cannot_be_written_by_a_caller() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
