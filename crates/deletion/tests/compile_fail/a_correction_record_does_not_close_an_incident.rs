//! `a_correction_record_does_not_close_an_incident`.
//!
//! The conversion route. `P2-Y3` measured that a `From`/`Into` implementation
//! escapes every public-function sweep, so the absence of one is written down
//! as a program that does not compile as well as pinned in
//! `the_impl_blocks_naming_the_gate_types_are_these`.
//!
//! Both directions are tried: turning a correction into a closure, and turning
//! one into the closed state. Neither exists, and section 34.6's fifth
//! principle is why.

use academic_deletion::{IncidentClosure, LeakIncidentState};
use academic_evidence_center::CorrectionRecord;

fn main() {
    let record: CorrectionRecord = unimplemented!();
    let _closure: IncidentClosure = IncidentClosure::from(record.clone());
    let _closed: LeakIncidentState = LeakIncidentState::from(record);
}
