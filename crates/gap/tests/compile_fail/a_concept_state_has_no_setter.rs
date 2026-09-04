//! Section 15.2 step 3 overlays four dimensions onto one concept, and
//! `ConceptState::overlay` is where each of them is checked against that
//! concept. Nothing takes `&mut self`, so a dimension edited after the check has
//! no method to be edited by.

use academic_domain::FreshnessBand;
use academic_gap::ConceptState;

fn raise(state: &mut ConceptState) {
    state.set_freshness(FreshnessBand::VeryHigh);
}

fn main() {}
