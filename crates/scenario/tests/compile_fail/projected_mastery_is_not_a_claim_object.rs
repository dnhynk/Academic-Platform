//! A projected mastery level cannot be built into a canonical claim object.
//!
//! `ClaimObject` is what an acceptance batch carries and therefore what a
//! canonical writer ultimately accepts. A projection that could be placed in
//! one would be an actual-state write in every sense that matters.

use academic_domain::{ClaimObject, ContentDigest, MasteryLevel, ModelRunId, TimestampMillis};
use academic_scenario::{ProposalProvenance, Proposed};

fn provenance() -> ProposalProvenance {
    let model_run_id: ModelRunId = "01936f2a-0000-7000-8000-000000000001".parse().unwrap();
    let inputs_digest: ContentDigest =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .parse()
            .unwrap();
    ProposalProvenance::new(model_run_id, inputs_digest, 1, TimestampMillis::new(0))
}

fn projected_mastery() -> Proposed<MasteryLevel> {
    Proposed::new(MasteryLevel::Fluent, provenance())
}

fn main() {
    let projected = projected_mastery();

    let _direct = ClaimObject::Mastery(projected);
}
