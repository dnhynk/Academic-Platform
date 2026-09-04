//! Section 16.2: a preference orders the vectors and does not rewrite them.
//! `CostVector` has private fields and no method taking `&mut self`, so a fact
//! edited where it stands has no method to be edited by.

use academic_critical_path::{CostComponent, CostEstimate, CostVector};

fn cheapen(vector: &mut CostVector, replacement: CostEstimate) {
    vector.set_component(CostComponent::LearningEffort, replacement);
}

fn main() {}
