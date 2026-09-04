//! Section 16.2: `사용자의 slider는 이 벡터를 정렬하는 preference일 뿐 진리를
//! 바꾸지 않는다`. A `Ranking` borrows its front through a shared reference and
//! hands one out, so there is no mutable path from an ordering back to a fact --
//! not even through a `&mut Ranking`.

use academic_critical_path::Ranking;

fn rewrite(ranking: &mut Ranking<'_>) {
    ranking.front_mut();
}

fn main() {}
