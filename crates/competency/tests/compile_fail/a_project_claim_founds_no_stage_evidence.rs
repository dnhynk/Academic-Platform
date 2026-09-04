//! `P2-R5`'s separation, one layer up.
//!
//! `ProjectSnapshot OBSERVES Concept` and `User APPLIED Concept` are two
//! claims, and only the second says anything about a person. `StageEvidence`
//! takes the second, and there is no overload, no trait and no conversion that
//! takes the first — so a repository observation cannot become a rubric cell by
//! being handed to the door the personal claim uses.

use academic_competency::{EvidenceStage, RecordId, StageEvidence};
use academic_repository_competency::ProjectObservationClaim;

fn found(observation: &ProjectObservationClaim) {
    let _ = StageEvidence::of_personal_claim(
        RecordId::new("r-1").unwrap_or_else(|_| unreachable!()),
        EvidenceStage::Used,
        observation,
    );
}

fn main() {}
