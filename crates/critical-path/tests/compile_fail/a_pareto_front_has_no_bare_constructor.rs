//! Section 16.2: `engine은 **먼저** Pareto-dominated path를 제거하고`. The only
//! constructor of a `ParetoFront` is `eliminate`, so a candidate list handed to
//! the ranker without elimination has no value of that type to be handed as.

use academic_critical_path::{Candidate, ParetoFront};

fn rank_without_eliminating(candidates: Vec<Candidate>) -> ParetoFront {
    ParetoFront {
        candidates,
        dominated: Vec::new(),
    }
}

fn main() {}
