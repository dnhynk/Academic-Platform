//! Section 29.1's order is the argument types.
//!
//! `schema_validation` produces the `Validated` that
//! `ai_proposal_where_appropriate` takes, and nothing else does. Handing stage
//! seven the output of stage five skips stage six, and it does not compile.

use academic_ingestion::stage::{Appropriateness, Parsed, ai_proposal_where_appropriate};

fn main() {
    let parsed: Parsed = unreachable_value();
    let _skipped = ai_proposal_where_appropriate(parsed, Appropriateness::NotAppropriate);
}

fn unreachable_value() -> Parsed {
    loop {}
}
