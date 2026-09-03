//! The `P2-M2` proposal boundary: risk tiers, the review queue, and reversible
//! dispositions.
//!
//! This crate holds four things and nothing else:
//!
//! * [`Proposed<T>`] -- the boundary a model-authored candidate sits behind
//!   until a human, or section 27.4's one exception to a human, lets it out;
//! * [`RiskTier`] -- section 27.4's four rows, and [`RiskTier::workflow`], the
//!   total mapping from each row to what it requires;
//! * [`ReviewQueue`] -- one queue with four doors, one per [`Workflow`], plus
//!   section 29.7's confidence/impact batching under a versioned
//!   [`BatchingThresholds`]; and
//! * an append-only disposition history, which an undo extends rather than
//!   edits, over the frozen `DecisionAction` of `academic-domain` rather than
//!   over a second vocabulary beside it.
//!
//! It persists nothing. The typed rows are `academic-store`'s, written by
//! migration `0009`, and the deliberate absence of a Cargo edge to that crate
//! is what makes `proposed_type_cannot_reach_canonical_writer` a compile error
//! rather than a source scan.
//!
//! # What this crate is not
//!
//! It is not an inference pipeline and it parses nothing. `P2-M1`
//! (`academic-model-run`) records what a model execution did and interprets its
//! confidence; `P2-G5` (`academic-untrusted-content`) validates a model output
//! against a schema, resolves its provenance, and produces the `Proposal`
//! record that a caller wraps in a [`Proposed`] here. Those two are upstream by
//! composition rather than by type: this crate has an edge to neither, and
//! nothing in this repository observes that a caller performed the sequence.
//!
//! `Proposed<T>` is generic for that reason. The payload it holds is whatever a
//! caller has: this crate's rules are about the tier, the disposition and the
//! actor, none of which depend on what the proposal says.

mod batching;
mod disposition;
mod error;
mod proposed;
mod queue;
mod tier;

pub use batching::{Batch, BatchKey, BatchingThresholds};
pub use disposition::{
    DispositionRecord, DispositionSeq, ExplicitApproval, UserDecision, disposition_token,
    releases_the_payload,
};
pub use error::{DispositionState, ThresholdError, WorkflowError};
pub use proposed::{Approved, Autosaved, ImpactOutOfRange, ImpactPermille, ProposalId, Proposed};
pub use queue::ReviewQueue;
pub use tier::{RiskTier, Workflow};
