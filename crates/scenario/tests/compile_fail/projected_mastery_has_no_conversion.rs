//! A projected mastery level has no conversion into the canonical type.
//!
//! `Deref`, `AsRef`, and `From`/`Into` are the three conversions Rust applies
//! without the caller naming them, so each one is an implicit path out of the
//! seal and each one must be absent.

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

    let _by_deref: MasteryLevel = *projected;
    let _by_as_ref: &MasteryLevel = projected.as_ref();
    let _by_from: MasteryLevel = MasteryLevel::from(projected);
    let _by_into: MasteryLevel = projected.into();
}
