//! Section 20.2's `작은 실행 evidence와 ... checkpoint를 갖는다`, held by the
//! producer's parameter list.
//!
//! `LearningItem::plan` takes both by value. There is no producer that omits
//! either, so a learning item with nothing to run is a program that does not
//! compile.

use academic_build_learn::{LearningItem, PartId, ReturnCheckpoint};
use academic_domain::EntityId;

fn without_evidence(id: PartId, concept: EntityId, checkpoint: ReturnCheckpoint) -> LearningItem {
    LearningItem::plan(id, concept, checkpoint)
}

fn main() {
    let _ = without_evidence;
}
