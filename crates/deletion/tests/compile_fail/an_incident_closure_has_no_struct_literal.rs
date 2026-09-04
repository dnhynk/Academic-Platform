//! `an_incident_closure_has_no_struct_literal`.
//!
//! Section 34.6's fifth principle is held by `IncidentClosure` having exactly
//! one producer — `ExternalLeakIncident::close`, which refuses until every
//! recovery step has happened. A caller that could write the literal would be
//! able to declare a leak contained without containing it.

use academic_deletion::IncidentClosure;
use academic_domain::TimestampMillis;

fn main() {
    let _forged = IncidentClosure {
        steps: unimplemented!(),
        scope: unimplemented!(),
        closed_at: TimestampMillis::new(0),
    };
}
