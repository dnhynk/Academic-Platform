//! A published score is a derivation, and a document is not a fifth disclosure.
//!
//! Reading one back would be a score that ran neither the weighting check nor
//! the three re-derivations, exactly as `P2-Y1`'s filled cell would be a cell
//! that ran neither of its two doors.

use academic_readiness::AuxiliaryScore;

fn shape(document: &str) -> Result<AuxiliaryScore, serde_json::Error> {
    serde_json::from_str(document)
}

fn main() {
    let _ = shape;
}
