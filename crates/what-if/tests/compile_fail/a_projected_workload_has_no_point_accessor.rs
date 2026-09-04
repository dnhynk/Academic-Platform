//! Section 22.4 shows a workload as `34–46 h/week`, never as one number.
//!
//! `ProjectedWorkload` hands out a range and its bias. There is no `midpoint`,
//! no `expected`, no `mean` and no `hours`: a point estimate reads as a
//! measurement of a quantity nobody measured.

use academic_what_if::ProjectedWorkload;

fn take(workload: &ProjectedWorkload) -> u16 {
    workload.midpoint()
}

fn main() {
    let _ = take;
}
