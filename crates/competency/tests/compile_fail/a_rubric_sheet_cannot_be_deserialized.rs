//! A filled cell is a derivation, not a document.
//!
//! `RubricSheet` and `StageEvidence` are `Serialize` and not `Deserialize`:
//! reading one back would be a third door into a filled cell, past both of the
//! producers that check anything.

use academic_competency::RubricSheet;

fn main() {
    let _: RubricSheet = serde_json::from_str("{}").unwrap_or_else(|_| unreachable!());
}
