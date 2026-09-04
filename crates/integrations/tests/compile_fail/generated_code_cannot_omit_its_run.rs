//! `generated_code_cannot_omit_its_run`.
//!
//! Section 33's coding-assistant row keeps generated-code provenance. The
//! record's fields are private and its one producer takes a `P2-M1` `ModelRun`,
//! so every route to a record without one is tried here.

use academic_domain::TimestampMillis;
use academic_integrations::{AssistantUse, GeneratedCode};
use academic_model_run::{Digest32, ModelRunId};

fn main() {
    // The fields are private, so a record cannot be written as a literal with a
    // run identifier nobody minted.
    let _literal = GeneratedCode {
        model_run: ModelRunId::from_bytes([0; 16]),
        run_digest: Digest32::of(b""),
        context_digest: Digest32::of(b""),
        output_digest: Digest32::of(b""),
        produced_at: TimestampMillis::new(0),
        use_kind: AssistantUse::GeneratedCode,
    };

    // Nor recorded from a bare identifier: `record` takes the run itself, so
    // the digest it stores is the run's own.
    let _from_id = GeneratedCode::record(
        &ModelRunId::from_bytes([0; 16]),
        b"context",
        b"output",
        TimestampMillis::new(0),
        AssistantUse::GeneratedCode,
    );

    // And the recorded provenance is read-only: there is no setter that could
    // point a record at a different run after the fact.
    let mut record = _literal;
    record.model_run = ModelRunId::from_bytes([1; 16]);
}
