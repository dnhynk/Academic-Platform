//! Section 15.1: `Gap은 낮은 Knowledge State 자체가 아니라 활성 목표의 성공을
//! 가로막는, 근거가 있는 prerequisite 부족이다`.
//!
//! `search` takes an `&ActiveGoal`. A concept identity is not one, so a gap
//! search over a concept with no goal is a program that does not compile.

use academic_domain::EntityId;
use academic_gap::{PrerequisiteGraph, search};

fn main() {
    let concept = EntityId::try_from_uuid(uuid::Uuid::now_v7()).unwrap();
    let graph = PrerequisiteGraph::new();
    let _ = search(&concept, &graph, &[], None);
}
