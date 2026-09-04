//! Section 11.4's three gates, as an absence.
//!
//! `DeterminateVerdict::new` takes all three witnesses by value and is
//! crate-private, and each witness's `establish` is crate-private too. A caller
//! outside this crate has no expression that produces a determination -- not a
//! wrong one, none at all.

use academic_audit::{
    ConflictFreeWitness, CoverageWitness, DeterminateVerdict, FreshnessWitness, GraduationOutcome,
};

fn main() {
    // No witness can be established from outside.
    let coverage = CoverageWitness::establish(&[], &[]);
    let conflict_free = ConflictFreeWitness::establish(&[], &[]);
    let freshness = FreshnessWitness::establish(None, unimplemented!(), unimplemented!());

    // And the verdict's own constructor is not public either.
    let _verdict = DeterminateVerdict::new(
        GraduationOutcome::Possible,
        coverage,
        conflict_free,
        freshness,
    );
}
