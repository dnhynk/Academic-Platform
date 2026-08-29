//! A projected workload range does not hand out the hours it proposes.
//!
//! A weekly-hours range is the projected value most easily mistaken for a
//! measurement, because it is already a number. The seal is what keeps it from
//! being read out and written as a canonical integer claim.

use academic_domain::{ContentDigest, ModelRunId, TimestampMillis};
use academic_scenario::{ProposalProvenance, Proposed, WorkloadHoursRange};

fn provenance() -> ProposalProvenance {
    let model_run_id: ModelRunId = "01936f2a-0000-7000-8000-000000000001".parse().unwrap();
    let inputs_digest: ContentDigest =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .parse()
            .unwrap();
    ProposalProvenance::new(model_run_id, inputs_digest, 1, TimestampMillis::new(0))
}

fn projected_workload() -> Proposed<WorkloadHoursRange> {
    Proposed::new(WorkloadHoursRange::new(34, 46).unwrap(), provenance())
}

fn main() {
    let workload = projected_workload();

    let _by_accessor: u16 = workload.high_hours();
    let _by_inner: WorkloadHoursRange = workload.into_inner();
}
