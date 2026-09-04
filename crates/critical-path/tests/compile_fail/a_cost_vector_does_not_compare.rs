//! Section 16.2: the vectors are never collapsed to a scalar in the API. A
//! total order over a whole vector *is* that collapse, so `CostVector` derives
//! neither `PartialOrd` nor `Ord` and two vectors have no `<` between them.

use academic_critical_path::CostVector;

fn cheaper(left: &CostVector, right: &CostVector) -> bool {
    left < right
}

fn main() {}
