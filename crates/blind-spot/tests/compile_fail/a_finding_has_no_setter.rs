//! Section 39: `evidence 부족 분류와 user disposition을 별도 저장한다`.
//!
//! A finding is recomputed, never edited: it has no `&mut self` method, no
//! setter and no public field, so a classification changed where it stands has
//! no representation.

use academic_blind_spot::{BlindSpotFinding, BlindSpotState};

fn overwrite(finding: &mut BlindSpotFinding) {
    finding.classification = BlindSpotState::Gap;
}

fn main() {}
