//! Section 23: the same rule on the other half of a coverage reading.
//!
//! `evidenceDiversity` says which of the five sources are present. It is not a
//! rung, so two of them cannot be ranked either.

use academic_blind_spot::EvidenceDiversity;

fn ranked(a: EvidenceDiversity, b: EvidenceDiversity) -> bool {
    a < b
}

fn main() {}
