//! Section 24.3's two sixes are two vocabularies.
//!
//! A stage says how deep a performance went and an axis says which column it is
//! displayed in. `P2-Y1` recorded that a total map between section 13.2's rows
//! and section 24.3's stages would have to invent three of its six answers; one
//! layer up the same map would have to invent the `설계 선택` column's.

use academic_competency::{CriterionId, EvidenceStage, StageEvidence};
use academic_readiness::{AxisEvidence, EvidenceLocatorId, ReadinessError};

fn shape(
    criterion: CriterionId,
    locator: EvidenceLocatorId,
    record: &StageEvidence,
) -> Result<AxisEvidence, ReadinessError> {
    AxisEvidence::place(EvidenceStage::MadeDesignChoice, criterion, locator, record)
}

fn main() {
    let _ = shape;
}
