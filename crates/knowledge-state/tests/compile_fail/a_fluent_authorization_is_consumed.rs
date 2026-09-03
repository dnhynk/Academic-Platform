//! Section 13.1: `반복된 강한 evidence와 사용자 확인 필요`.
//!
//! `FluentAuthorization` is taken by value and is not `Clone`, so one
//! authorization produces one promotion. A caller cannot mint one and apply it
//! to a second concept.

use academic_domain::EntityId;
use academic_knowledge_state::{FluentAuthorization, MasteryProjection};

fn twice(
    projection: &MasteryProjection,
    authorization: FluentAuthorization,
    first: EntityId,
    second: EntityId,
) {
    let _one = projection.with_fluency(authorization, first);
    let _two = projection.with_fluency(authorization, second);
}

fn main() {}
