//! `CONTRIBUTING.md` rule 2: canonical events are append-only and a correction
//! is a new event.
//!
//! Every method that changes what a history holds consumes it and returns a new
//! one, so a caller cannot keep a handle to the old value and go on using it as
//! though the retraction had not happened.

use academic_domain::TimestampMillis;
use academic_knowledge_state::{EvidenceRetraction, FreshnessInput, KnowledgeStateHistory};

fn both(
    history: KnowledgeStateHistory,
    retraction: EvidenceRetraction,
    freshness: FreshnessInput,
) {
    let _next = history.retract(retraction, freshness, TimestampMillis::new(0));
    let _stale = history.current();
}

fn main() {}
