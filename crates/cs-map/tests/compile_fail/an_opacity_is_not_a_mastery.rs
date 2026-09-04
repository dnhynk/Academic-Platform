//! Section 26.2's `opacity: 현재 lens relevance이지 mastery가 아님`, as a type
//! error.
//!
//! There is no `From<MasteryFill> for LensRelevance`, no `Into`, no `AsRef` and
//! no free function. The whole set of `impl` headers this crate declares is
//! pinned by `every_impl_header_in_this_crate_is_in_the_inventory`, so adding
//! one is an edit to a reviewed list rather than a new file nobody reads.

use academic_cs_map::{LensRelevance, MasteryFill};

fn shade(fill: MasteryFill) -> LensRelevance {
    fill.into()
}

fn main() {
    let _ = shade;
}
