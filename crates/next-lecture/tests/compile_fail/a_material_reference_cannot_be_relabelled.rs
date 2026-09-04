//! Which of section 12.7's seven places a material is decides whether it needs
//! a `P2-L4` document node, and `MaterialReference::of` is where that pair is
//! checked. The fields are private and nothing takes `&mut self`, so a
//! reference relabelled after the check has no method to be relabelled by.

use academic_next_lecture::{ExpectedConceptSource, MaterialReference};

fn relabel(reference: &mut MaterialReference) {
    reference.set_source(ExpectedConceptSource::Syllabus);
}

fn main() {}
