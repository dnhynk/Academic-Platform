//! Section 11.4's three gates, second half.
//!
//! One route per file: `E0451` comes from the privacy pass, which rustc does
//! not reach once type checking has already failed, so a literal sharing a file
//! with any other refused route is invisible.

use academic_audit::{
    ConflictFreeWitness, CoverageWitness, DeterminateVerdict, FreshnessWitness, GraduationOutcome,
};

fn main() {
    let coverage: CoverageWitness = unimplemented!();
    let conflict_free: ConflictFreeWitness = unimplemented!();
    let freshness: FreshnessWitness = unimplemented!();

    let _literal = DeterminateVerdict {
        outcome: GraduationOutcome::Possible,
        coverage,
        conflict_free,
        freshness,
    };
}
