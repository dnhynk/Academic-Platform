//! Reanalysis appends a candidate beside the earlier one and never over it.
//!
//! ADR-003's rule is the one that applies -- "corrections append a new
//! assertion" -- so this module adds no second mechanism. A later run's
//! candidate names the earlier candidate it supersedes, both rows stay, and the
//! diff is read from the pair. Migration `0007` is the enforcement half:
//! `model_run_candidate` is INSERT-only under a trigger pair and the SQLite
//! authorizer, `guard_model_run_candidate_supersession` refuses a supersession
//! from the same run or over another subject, and `UNIQUE
//! (supersedes_candidate_id)` refuses a fork.

use crate::record::{Digest32, ModelRunId};

/// A 16-byte candidate identifier.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CandidateId([u8; 16]);

impl CandidateId {
    /// Wraps an exact 16-byte identifier.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The raw identifier bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Why two candidates do not form a reanalysis pair.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReanalysisError {
    /// The revision does not name the prior candidate.
    #[error("the revision does not supersede the candidate it is compared with")]
    NotASupersession,
    /// The two candidates are about different sources.
    #[error("a reanalysis addresses the subject the prior candidate addressed")]
    DifferentSubject,
    /// Both candidates came from the same model run.
    #[error("a model run cannot revise its own candidate; only a later run can")]
    SameModelRun,
}

/// One candidate a model run produced about one source.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    id: CandidateId,
    model_run: ModelRunId,
    subject_digest: Digest32,
    candidate_digest: Digest32,
    supersedes: Option<CandidateId>,
}

impl Candidate {
    /// A first candidate about a source, superseding nothing.
    #[must_use]
    pub const fn first(
        id: CandidateId,
        model_run: ModelRunId,
        subject_digest: Digest32,
        candidate_digest: Digest32,
    ) -> Self {
        Self {
            id,
            model_run,
            subject_digest,
            candidate_digest,
            supersedes: None,
        }
    }

    /// A reanalysis candidate, naming the candidate it supersedes.
    #[must_use]
    pub const fn reanalysis(
        id: CandidateId,
        model_run: ModelRunId,
        subject_digest: Digest32,
        candidate_digest: Digest32,
        supersedes: CandidateId,
    ) -> Self {
        Self {
            id,
            model_run,
            subject_digest,
            candidate_digest,
            supersedes: Some(supersedes),
        }
    }

    /// The candidate identifier.
    #[must_use]
    pub const fn id(&self) -> &CandidateId {
        &self.id
    }

    /// The model run that produced it.
    #[must_use]
    pub const fn model_run(&self) -> &ModelRunId {
        &self.model_run
    }

    /// Digest of the source the candidate is about.
    #[must_use]
    pub const fn subject_digest(&self) -> &Digest32 {
        &self.subject_digest
    }

    /// Digest of the candidate value itself.
    #[must_use]
    pub const fn candidate_digest(&self) -> &Digest32 {
        &self.candidate_digest
    }

    /// The candidate this one supersedes, if it is a reanalysis.
    #[must_use]
    pub const fn supersedes(&self) -> Option<&CandidateId> {
        self.supersedes.as_ref()
    }
}

/// The difference between two candidates about one source, naming both runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReanalysisDiff {
    prior_model_run: ModelRunId,
    revised_model_run: ModelRunId,
    prior_candidate: CandidateId,
    revised_candidate: CandidateId,
    subject_digest: Digest32,
    prior_digest: Digest32,
    revised_digest: Digest32,
}

impl ReanalysisDiff {
    /// Builds the diff between a prior candidate and the one that supersedes it.
    pub fn between(prior: &Candidate, revised: &Candidate) -> Result<Self, ReanalysisError> {
        if revised.supersedes() != Some(prior.id()) {
            return Err(ReanalysisError::NotASupersession);
        }
        if prior.subject_digest() != revised.subject_digest() {
            return Err(ReanalysisError::DifferentSubject);
        }
        if prior.model_run() == revised.model_run() {
            return Err(ReanalysisError::SameModelRun);
        }
        Ok(Self {
            prior_model_run: *prior.model_run(),
            revised_model_run: *revised.model_run(),
            prior_candidate: *prior.id(),
            revised_candidate: *revised.id(),
            subject_digest: *prior.subject_digest(),
            prior_digest: *prior.candidate_digest(),
            revised_digest: *revised.candidate_digest(),
        })
    }

    /// The model run that produced the earlier candidate.
    #[must_use]
    pub const fn prior_model_run(&self) -> &ModelRunId {
        &self.prior_model_run
    }

    /// The model run that produced the revision.
    #[must_use]
    pub const fn revised_model_run(&self) -> &ModelRunId {
        &self.revised_model_run
    }

    /// The earlier candidate, which the revision did not touch.
    #[must_use]
    pub const fn prior_candidate(&self) -> &CandidateId {
        &self.prior_candidate
    }

    /// The appended candidate.
    #[must_use]
    pub const fn revised_candidate(&self) -> &CandidateId {
        &self.revised_candidate
    }

    /// The source both candidates are about.
    #[must_use]
    pub const fn subject_digest(&self) -> &Digest32 {
        &self.subject_digest
    }

    /// Whether the reanalysis actually changed the candidate value.
    #[must_use]
    pub fn value_changed(&self) -> bool {
        self.prior_digest != self.revised_digest
    }
}
