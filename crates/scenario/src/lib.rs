//! Projected, hypothetical what-if values, isolated from canonical actual state.
//!
//! Every projected value the semester simulator produces lives here, and this
//! crate has no Cargo edge of any kind — normal, build, or dev — to the
//! canonical writer crate. That absence is the isolation: a projected value
//! cannot name the writer, so no code path in this crate can reach an
//! actual-state write.
//!
//! The isolation has two halves and needs both.
//!
//! The type half is [`Proposed<T>`]: a sealed wrapper with no `into_inner`, no
//! `Deref`, no `From`/`Into` back to `T`, no closure combinator that hands `T`
//! out, and no [`serde`] implementation. A caller holding a projected mastery,
//! opportunity, or workload value has no expression that yields the canonical
//! type a writer accepts. `tests/compile_fail` proves that by compiling the
//! attempts and requiring each one to fail.
//!
//! The runtime half is [`envelope::admit_projection_payload`]. Types do not
//! travel over a wire; bytes do. A forged payload can spell any field it likes,
//! so the deserializing edge re-derives the projection binding and refuses a
//! payload that claims canonical authority, drops the hypothetical marker,
//! carries an unknown field, or no longer matches its binding.
//!
//! The only future-knowledge output is
//! [`ProjectedEvidenceOpportunity`](opportunity::ProjectedEvidenceOpportunity).
//! Projecting a future mastery level is not a supported output: a completed
//! course yields exposure, practice, and assessment opportunity, and the
//! mastery that follows is decided later by the evidence actually produced.

pub mod envelope;
pub mod error;
pub mod opportunity;
pub mod proposed;
pub mod simulate;
pub mod workload;

pub use envelope::{
    PROJECTION_ENVELOPE_VERSION, ProjectionAdmissionError, ProjectionEnvelope,
    admit_projection_payload,
};
pub use error::ScenarioError;
pub use opportunity::{
    LikelihoodBand, OpportunityBasis, OpportunityKind, ProjectedEvidenceOpportunity,
};
pub use proposed::{ProjectedMastery, ProjectionCalibration, ProposalProvenance, Proposed};
pub use simulate::{
    ScenarioAssumption, ScenarioChoice, ScenarioInputs, ScenarioProjection, SyllabusConceptSignal,
    project, scenario_inputs_digest,
};
pub use workload::{ProjectedWorkloadRange, WorkloadHoursRange};

/// Engine version stamped into every projection this crate emits.
///
/// The projection binding covers it, so a payload produced by one engine
/// version can never be replayed as the output of another.
pub const SCENARIO_ENGINE_VERSION: u32 = 1;
