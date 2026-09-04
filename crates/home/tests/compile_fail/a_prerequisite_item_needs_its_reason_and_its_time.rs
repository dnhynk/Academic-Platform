//! `a_prerequisite_item_needs_its_reason_and_its_time`.
//!
//! Section 25.2's second line asks for `“왜 지금”과 예상 시간`. Both are
//! parameters of the one constructor and both fields are private, so an item
//! without either is not a value that can be written down.

use academic_home::{EstimatedMinutes, PrerequisiteItem, UpcomingUse};

fn main() {
    let concept: academic_domain::EntityId = "0190ffff-0000-7000-8000-000000000001"
        .parse()
        .unwrap_or_else(|_| panic!("a valid identifier"));

    // There is no constructor that leaves the reason out.
    let _reasonless = PrerequisiteItem::offer(concept);

    // Nor a struct literal that could omit either field.
    let _assembled = PrerequisiteItem {
        concept,
        why_now: UpcomingUse::declare,
        estimated: EstimatedMinutes::new,
    };

    // Nor a setter to fill one in afterwards.
    let mut item = PrerequisiteItem::offer(concept);
    item.set_estimated(EstimatedMinutes::new(10));
}
