//! Section 22.4: `하나의 "추천 점수"를 기본으로 표시하지 않는다`.
//!
//! `DimensionPriority` has no `Default` and no constant. A shipped neutral
//! ordering would be this product answering the importance question on the
//! user's behalf, which is what that sentence refuses.

use academic_what_if::DimensionPriority;

fn main() {
    let _priority = DimensionPriority::default();
}
