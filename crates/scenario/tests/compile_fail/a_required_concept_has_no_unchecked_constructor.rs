//! A whole field cannot become a project's requirement by another door.
//!
//! `RequiredConcept::realizing` refuses `EntityKind::Field` and
//! `EntityKind::Alias`, and `broad_category_cannot_require_a_whole_field`
//! measures that. What makes it the only door is here: the fields are private,
//! there is no `Default`, and there is no `new`.

use academic_repository_classification::RequiredConcept;

fn main() {
    let _by_default: RequiredConcept = RequiredConcept::default();
    let _by_literal = RequiredConcept {
        concept: "distributed-systems".to_owned(),
    };
    let _by_new = RequiredConcept::new();
}
