//! Section 13.1: `estimateConfidence`는 사용자의 실력 점수가 아니다.
//!
//! `EvidenceSufficiency` has no ordering and no conversion in either direction
//! with a mastery level, so it cannot be ranked between users or read as a
//! level. Both attempts fail to compile.

use academic_domain::MasteryLevel;
use academic_knowledge_state::EvidenceSufficiency;

fn rank(a: &EvidenceSufficiency, b: &EvidenceSufficiency) -> bool {
    a < b
}

fn as_level(sufficiency: EvidenceSufficiency) -> MasteryLevel {
    MasteryLevel::from(sufficiency)
}

fn main() {}
