//! Section 16.2: `근거가 없으면 범위로 표시한다`. A `CostEstimate` hands out its
//! two ends and nothing between them, so an unknown cost read as a single
//! number has no accessor to be read by.

use academic_critical_path::CostEstimate;

fn narrow(estimate: &CostEstimate) -> u32 {
    estimate.midpoint()
}

fn main() {}
