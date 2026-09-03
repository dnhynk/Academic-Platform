//! A component identifier cannot be assembled past the refusal in its
//! constructor.
//!
//! `ComponentId::new` refuses every spelling of the repository root — `""`,
//! `"."`, `"/"` and `"./"` — so the coarsest scope a finding can have is a
//! named part of the tree. That refusal is only worth something if the
//! constructor is the only door, which is what this case fails if it stops
//! being true.

use academic_repository_analysis::ComponentId;

fn main() {
    let _root = ComponentId {
        directory: String::new(),
    };
}
