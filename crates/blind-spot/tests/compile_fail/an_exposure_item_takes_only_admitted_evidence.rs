//! Section 13.4's four checks, read on section 23's axis.
//!
//! An `ExposureItem` wraps an `EligibleEvidence` and has no other constructor,
//! so evidence that failed admission cannot be counted as exposure.

use academic_blind_spot::{ExposureItem, ExposureSource};
use academic_knowledge_state::BlockedEvidence;
use academic_domain::TimestampMillis;

fn count(blocked: BlockedEvidence, observed_at: TimestampMillis) -> ExposureItem {
    ExposureItem::of(blocked, ExposureSource::Lecture, observed_at)
}

fn main() {}
