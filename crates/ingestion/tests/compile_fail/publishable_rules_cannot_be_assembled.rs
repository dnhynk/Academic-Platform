//! The publishable value has no public constructor.
//!
//! If it could be written by hand, the `None` arm of `Reconciled::publishable`
//! would be advice rather than the rule, because a caller with an undated
//! document could build one anyway.

use academic_ingestion::PublishableRules;

fn main() {
    let _forged = PublishableRules {};
}
