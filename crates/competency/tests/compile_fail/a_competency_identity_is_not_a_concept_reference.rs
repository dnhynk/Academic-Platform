//! Section 24.1's first sentence, in the other direction.
//!
//! A criterion is about concepts. Handing it the competency it belongs to is a
//! program that does not compile, rather than a self-referential edge somebody
//! has to notice.

use academic_competency::{CompetencyId, CriterionId, PerformanceCriterion};

fn main() {
    let competency = CompetencyId::new("cache_staleness_diagnosis")
        .unwrap_or_else(|_| unreachable!());
    let _ = PerformanceCriterion::of(
        CriterionId::new("c-1").unwrap_or_else(|_| unreachable!()),
        "invalidates on write",
        vec![competency],
    );
}
