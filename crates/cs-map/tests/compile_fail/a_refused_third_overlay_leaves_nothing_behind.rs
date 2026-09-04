//! Section 26.3's `보조 overlay 두 개까지만`, held by ownership.
//!
//! `LensComposition::overlay` takes `self` by value, so a refused third overlay
//! consumes the composition. There is nothing left to try a fourth lens on,
//! which is `P2-X1`'s `Optimistic::confirm` shape: a refusal that leaves the
//! value behind is a refusal a loop defeats.

use academic_cs_map::{LensComposition, MapLens};

fn main() {
    let full = LensComposition::base(MapLens::Knowledge)
        .overlay(MapLens::Project)
        .and_then(|one| one.overlay(MapLens::Question))
        .unwrap_or_else(|_| LensComposition::base(MapLens::Knowledge));
    let _refused = full.overlay(MapLens::BlindSpot);
    let _retry = full.overlay(MapLens::Graduation);
}
