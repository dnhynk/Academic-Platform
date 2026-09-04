//! Section 12.7 separates the evidence and confidence of the expected concept,
//! the prerequisite edge and the user state. `PrepUncertainty` answers with
//! three readings of three different types and with no confidence of its own,
//! so folding the three into one has no method to be folded by.

use academic_domain::ConfidencePermille;
use academic_next_lecture::PrepUncertainty;

fn overall(uncertainty: &PrepUncertainty) -> ConfidencePermille {
    uncertainty.confidence()
}

fn main() {}
