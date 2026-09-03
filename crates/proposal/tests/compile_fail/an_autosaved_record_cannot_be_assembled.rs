//! An autosaved record cannot be assembled field by field.
//!
//! `Autosaved` has private fields and no `Default`. A caller that could build
//! one could stamp `AI_INFERRED` on a candidate no door had let out.

use academic_proposal::{Autosaved, ProposalId};

fn main() {
    let _autosaved = Autosaved {
        id: ProposalId::new(1),
        value: String::from("candidate"),
    };
}
