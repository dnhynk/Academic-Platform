//! The two producers, as the only two.
//!
//! `StageEvidence` reads its concept out of the value that founded it. A struct
//! literal would let a caller record evidence about one concept and file it
//! under another, which is the join defect `P2-R5` measured. Every field is
//! private.

use academic_competency::{ConceptRef, EvidenceSource, EvidenceStage, RecordId, StageEvidence};
use academic_domain::EvidenceId;

fn main() {
    let _ = StageEvidence {
        id: RecordId::new("r-1").unwrap_or_else(|_| unreachable!()),
        stage: EvidenceStage::DebuggedIncident,
        concept: ConceptRef::classification("express").unwrap_or_else(|_| unreachable!()),
        source: EvidenceSource::KnowledgeState(
            EvidenceId::try_from_uuid(uuid::Uuid::nil()).unwrap_or_else(|_| unreachable!()),
        ),
    };
}
