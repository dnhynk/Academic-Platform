//! A warrant is its three steps and nothing else.
//!
//! `GeneratedCodeWarrant` has one private field, no `Default`, and one
//! constructor taking the whole chain. There is no way to name a warrant that
//! carries only what a caller happens to have.

use academic_repository_competency::{ExplainedByUser, GeneratedCodeWarrant};

fn main() {
    let explained: ExplainedByUser = unimplemented!();
    let _literal = GeneratedCodeWarrant { explained };
    let _default = GeneratedCodeWarrant::default();
    let _new = GeneratedCodeWarrant::new();
}
