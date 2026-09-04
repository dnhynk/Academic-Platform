//! Running a confirmed deletion, and reporting it in `P2-K5`'s four words.
//!
//! # The vocabulary is not restated
//!
//! [`academic_retention::settle`] decides the word and writes the journal. This
//! module does not re-derive `COMPLETE`, `PARTIAL` or `REPAIR_REQUIRED`: it
//! hands `settle` an executor and takes its answer, so a build that changed the
//! rule in one place would not have two places to change.
//!
//! # What it does add: the unresolved list names artifacts
//!
//! `PlannedAction` carries a class, a kind and a locator, and item `P3-G10` of
//! the rotation contract records what that costs — a locator is shared by every
//! artifact in one domain holding the same bytes, so two registrations deleted
//! together produce two actions that differ in nothing. The fifth `P2-A1` audit
//! measured the same shape one layer down (`P1-G1`): the second tombstone
//! replaced the first and the restore receipt reported a resurrected artifact
//! as one it had spared.
//!
//! [`TargetAdapter`] closes it positionally. `DeletionPlan::build` asks the dry
//! run for each class in registry order and `ClassResolution::Locators` keeps
//! the order the index answered in, so the actions `settle` walks are the
//! targets the dry run holds, in the same order and with the same multiplicity.
//! The adapter walks both at once and compares each action's locator with the
//! target it is about to run; a mismatch is
//! [`DeletionFlowError::PlanDrifted`] rather than a silently mis-attributed
//! failure. What comes back is an unresolved list keyed by artifact **and**
//! locator, and [`ArtifactDeletionReceipt`] checks it against the journal's own
//! rows so the two cannot drift.

use academic_retention::{
    ActionId, ActionKind, AppendOnlyJournal, DerivativeClass, ExecutionFailure, PlannedAction,
    RetentionExecutor, RetentionOutcome, UnresolvedReason, settle,
};

use crate::{
    confirm::DeletionConfirmation, error::DeletionFlowError, provider::ProviderErasureLog,
    target::DeletionTarget,
};

/// Performs one deletion action against one artifact.
///
/// The seam `P2-K5` left for this task, with the identity fixed: its own
/// `RetentionExecutor` takes a locator and this takes the artifact as well.
pub trait TargetExecutor {
    /// Deletes exactly what `kind` says about `target`, or says why it did not.
    ///
    /// The journal is `P2-K5`'s own, already carrying this action's
    /// `RetentionPlanned` record. An executor that destroys a key slot appends
    /// its `ArtifactShredded` fact through it, so the record lands between the
    /// write it describes and the settlement that closes the run.
    fn execute(
        &mut self,
        journal: &mut AppendOnlyJournal,
        kind: ActionKind,
        class: DerivativeClass,
        target: &DeletionTarget,
    ) -> Result<(), ExecutionFailure>;
}

/// One artifact a deletion did not reach, and why.
///
/// The row `P2-K5`'s `UnresolvedLocator` carries plus the artifact it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedTarget {
    /// Which class it belongs to.
    pub class: DerivativeClass,
    /// The artifact and the locator, or the class node when there is no
    /// artifact yet.
    pub target: Option<DeletionTarget>,
    /// Why it is still there.
    pub reason: UnresolvedReason,
    /// The executor's or the index's own words.
    pub detail: String,
}

impl UnresolvedTarget {
    /// The row a report shows.
    ///
    /// Rendered by `P2-K5`'s own `UnresolvedLocator::to_row`, with the
    /// artifact-and-locator pair standing where its locator would. A second
    /// spelling of the four reason words here would be a second vocabulary,
    /// and `UnresolvedReason` is `#[non_exhaustive]`, so restating it would
    /// need a wildcard arm — the exact shape that turns a fifth reason into a
    /// row nobody can read.
    #[must_use]
    pub fn to_row(&self) -> String {
        academic_retention::UnresolvedLocator {
            class: self.class,
            locator: self.target.as_ref().map_or_else(
                || format!("<class {}>", self.class.as_str()),
                DeletionTarget::to_row,
            ),
            reason: self.reason,
            detail: self.detail.clone(),
        }
        .to_row()
    }
}

/// Walks a plan's actions and a dry run's targets together.
///
/// Public because the positional invariant is the load-bearing claim and a
/// caller with its own executor has to be able to hold it the same way.
#[derive(Debug)]
pub struct TargetAdapter<'e, E: TargetExecutor + ?Sized + std::fmt::Debug> {
    executor: &'e mut E,
    pending: Vec<(DerivativeClass, DeletionTarget)>,
    position: usize,
    drifted: bool,
    failures: Vec<UnresolvedTarget>,
}

impl<'e, E: TargetExecutor + ?Sized + std::fmt::Debug> TargetAdapter<'e, E> {
    /// Binds an executor to the targets a confirmed deletion reaches.
    #[must_use]
    pub fn over(executor: &'e mut E, confirmation: &DeletionConfirmation) -> Self {
        let pending = confirmation
            .preview()
            .dry_run()
            .nodes()
            .iter()
            .flat_map(|node| {
                node.targets()
                    .iter()
                    .map(move |target| (node.class(), *target))
            })
            .collect();
        Self {
            executor,
            pending,
            position: 0,
            drifted: false,
            failures: Vec::new(),
        }
    }

    /// Whether the plan's actions and the dry run's targets stayed in step.
    #[must_use]
    pub const fn drifted(&self) -> bool {
        self.drifted
    }

    /// Every artifact this run did not reach, in plan order.
    #[must_use]
    pub fn failures(&self) -> &[UnresolvedTarget] {
        &self.failures
    }
}

impl<E: TargetExecutor + ?Sized + std::fmt::Debug> RetentionExecutor for TargetAdapter<'_, E> {
    fn execute(
        &mut self,
        journal: &mut AppendOnlyJournal,
        action: &PlannedAction,
    ) -> Result<(), ExecutionFailure> {
        let Some((class, target)) = self.pending.get(self.position).copied() else {
            self.drifted = true;
            return Err(ExecutionFailure {
                reason: UnresolvedReason::NotResolved,
                detail: "the plan holds more actions than the dry run holds targets".to_owned(),
            });
        };
        self.position += 1;
        if class != action.class || target.locator() != &action.locator {
            self.drifted = true;
            return Err(ExecutionFailure {
                reason: UnresolvedReason::NotResolved,
                detail: "the plan's action is not the dry run's target".to_owned(),
            });
        }
        match self.executor.execute(journal, action.kind, class, &target) {
            Ok(()) => Ok(()),
            Err(failure) => {
                self.failures.push(UnresolvedTarget {
                    class,
                    target: Some(target),
                    reason: failure.reason,
                    detail: failure.detail.clone(),
                });
                Err(failure)
            }
        }
    }
}

/// What a deletion ended in, locally and at every provider it reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDeletionReceipt {
    subject: DeletionTarget,
    outcome: RetentionOutcome,
    unresolved: Vec<UnresolvedTarget>,
    provider: ProviderErasureLog,
}

impl ArtifactDeletionReceipt {
    /// The artifact the user asked to delete.
    #[must_use]
    pub const fn subject(&self) -> &DeletionTarget {
        &self.subject
    }

    /// `P2-K5`'s word for the local half.
    #[must_use]
    pub const fn outcome(&self) -> &RetentionOutcome {
        &self.outcome
    }

    /// Which of the four words this is.
    #[must_use]
    pub const fn outcome_word(&self) -> &'static str {
        self.outcome.as_str()
    }

    /// Every artifact the local half did not reach, in plan order.
    #[must_use]
    pub fn unresolved(&self) -> &[UnresolvedTarget] {
        &self.unresolved
    }

    /// The rendered unresolved rows, in plan order.
    #[must_use]
    pub fn unresolved_rows(&self) -> Vec<String> {
        self.unresolved
            .iter()
            .map(UnresolvedTarget::to_row)
            .collect()
    }

    /// Every provider erasure this deletion asked for.
    #[must_use]
    pub const fn provider(&self) -> &ProviderErasureLog {
        &self.provider
    }

    /// Whether nothing is left anywhere.
    ///
    /// Local `COMPLETE` **and** a settled receipt from every provider the
    /// artifact was transmitted to. A provider that offers no receipt
    /// (`EG07`) therefore keeps this `false` forever, which is the truth: this
    /// build cannot observe that copy being erased.
    #[must_use]
    pub fn is_fully_erased(&self) -> bool {
        matches!(self.outcome, RetentionOutcome::Complete) && self.provider.outstanding().is_empty()
    }

    /// Every row a user is shown: the local ones, then the provider ones.
    #[must_use]
    pub fn report_rows(&self) -> Vec<String> {
        let mut rows = self.unresolved_rows();
        rows.extend(self.provider.outstanding_rows());
        rows
    }
}

/// Runs a confirmed deletion and settles it.
///
/// # Errors
///
/// [`DeletionFlowError::PlanDrifted`] when the plan's actions and the dry run's
/// targets stop being the same list, and [`DeletionFlowError::Journal`] when the
/// append-only journal refuses a record.
pub fn execute_deletion<E: TargetExecutor + ?Sized + std::fmt::Debug>(
    journal: &mut AppendOnlyJournal,
    action_id: ActionId,
    confirmation: &DeletionConfirmation,
    executor: &mut E,
    provider: ProviderErasureLog,
) -> Result<ArtifactDeletionReceipt, DeletionFlowError> {
    let dry_run = confirmation.preview().dry_run();
    let plan = dry_run.plan();
    let mut adapter = TargetAdapter::over(executor, confirmation);
    let outcome = settle(journal, action_id, &plan, &mut adapter)
        .map_err(|error| DeletionFlowError::Journal(error.to_string()))?;
    if adapter.drifted() {
        return Err(DeletionFlowError::PlanDrifted);
    }
    let mut unresolved: Vec<UnresolvedTarget> = dry_run
        .nodes()
        .iter()
        .filter(|node| node.is_unresolved())
        .map(|node| UnresolvedTarget {
            class: node.class(),
            target: None,
            reason: UnresolvedReason::NotResolved,
            detail: match node.resolution() {
                academic_retention::ClassResolution::Unresolved { reason } => reason.clone(),
                _ => String::new(),
            },
        })
        .collect();
    unresolved.extend(adapter.failures().iter().cloned());

    // The list a user reads and the list the journal holds are the same list.
    // `settle` builds its own from the plan; if the two ever stop agreeing on
    // how many rows there are and what each one says, the receipt is wrong and
    // saying so is the only honest answer.
    let journalled = outcome.unresolved();
    if journalled.len() != unresolved.len()
        || journalled
            .iter()
            .zip(&unresolved)
            .any(|(row, mine)| row.class != mine.class || row.reason != mine.reason)
    {
        return Err(DeletionFlowError::PlanDrifted);
    }

    Ok(ArtifactDeletionReceipt {
        subject: *dry_run.subject(),
        outcome,
        unresolved,
        provider,
    })
}
