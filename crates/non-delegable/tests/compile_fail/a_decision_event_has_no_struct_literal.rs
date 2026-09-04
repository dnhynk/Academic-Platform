//! `a_decision_event_has_no_struct_literal`.
//!
//! `the_non_delegable_set_refuses_every_automatic_actor` drives every automatic
//! actor through `DecisionEvent::recorded` and observes the refusal. That test
//! cannot observe the route that skips `recorded` altogether: a struct literal
//! naming the fields. This is that route, and it does not exist.

use academic_domain::TimestampMillis;
use academic_non_delegable::{DecisionEvent, NonDelegableAction};

fn main() {
    let _forged = DecisionEvent {
        action: NonDelegableAction::ApproveEgress,
        decision: unimplemented!(),
        subject: unimplemented!(),
        decided_at: TimestampMillis::new(0),
    };
}
