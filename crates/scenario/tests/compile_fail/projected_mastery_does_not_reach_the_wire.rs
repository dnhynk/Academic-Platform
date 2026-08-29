//! A projected mastery level cannot be sealed into a projection envelope.
//!
//! The envelope is the one supported way a projection leaves this process.
//! It carries a `ScenarioProjection`, which has no mastery field, so there is
//! no wire form of a projected mastery level for a receiver to re-admit.

use academic_domain::{ContentDigest, MasteryLevel, ModelRunId, TimestampMillis};
use academic_scenario::{ProjectionEnvelope, ProposalProvenance, Proposed};

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

    let _sealed = ProjectionEnvelope::seal(projected);
}
