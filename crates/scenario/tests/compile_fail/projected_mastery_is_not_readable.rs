//! A projected mastery level cannot be read back out of its seal.
//!
//! Every spelling of "give me the value" is attempted here. If any one of
//! them compiled, a caller could hand the result straight to a canonical
//! writer and the type isolation would be decorative.

use academic_domain::{ContentDigest, MasteryLevel, ModelRunId, TimestampMillis};
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

    let _by_into_inner: MasteryLevel = projected.into_inner();
    let _by_value: MasteryLevel = projected.value();
    let _by_get: MasteryLevel = projected.get();
    let _by_field: MasteryLevel = projected.value;
}
