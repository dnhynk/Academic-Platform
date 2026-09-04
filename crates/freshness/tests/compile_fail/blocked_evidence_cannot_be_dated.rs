//! Section 13.4's four checks bound this axis too.
//!
//! `DatedEvidence` wraps an `EligibleEvidence`, whose one producer is
//! `EligibilityOutcome::admit`. Evidence that failed a check has no value of the
//! type this crate accepts, so it cannot freshen a concept it cannot promote.

use academic_domain::TimestampMillis;
use academic_freshness::DatedEvidence;
use academic_knowledge_state::BlockedEvidence;

fn date(blocked: BlockedEvidence) -> DatedEvidence {
    DatedEvidence::at(blocked, TimestampMillis::new(0))
}

fn main() {
    let _ = date;
}
