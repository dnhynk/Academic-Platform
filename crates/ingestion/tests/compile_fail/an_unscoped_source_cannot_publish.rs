//! Section 29.2's rule, as a type rather than as a check.
//!
//! `publish` takes a `PublishableRules`. `Reconciled::publishable` is its only
//! producer and returns `None` for `Dating::Unscoped`, so an undated official
//! document is not a value this call can be made with. A runtime check would
//! sit one layer inside a function a later caller can stop calling; this does
//! not.

use academic_ingestion::{publish, stage::Reconciled};

fn main() {
    let reconciled: Reconciled = unreachable_value();
    let _published = publish(&reconciled);
}

fn unreachable_value() -> Reconciled {
    loop {}
}
