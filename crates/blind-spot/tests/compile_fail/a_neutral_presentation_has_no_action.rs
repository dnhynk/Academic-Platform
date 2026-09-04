//! Section 23: `진로 목표와 무관하면 행동 요구를 만들지 않는다`.
//!
//! The neutral presentation has no field an action could occupy, so reading one
//! off it is a compile error rather than a `None` somebody has to check for.

use academic_blind_spot::{FindingPresentation, TastePath};

fn action(presentation: &FindingPresentation) -> &TastePath {
    match presentation {
        FindingPresentation::Neutral { path, .. } => path,
        FindingPresentation::Explore { path, .. } => path,
    }
}

fn main() {}
