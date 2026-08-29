//! Executing a deletion plan and settling it into one of four words.
//!
//! The executor seam is one trait with one method. `P2-K5` fixes what a result
//! means; `P2-P2` supplies the implementation that reaches the real transcript,
//! embedding, claim, document, cache, and replica subsystems once they exist.

use crate::{
    entry::JournalEntry,
    journal::{AppendOnlyJournal, JournalError},
    plan::{
        ClassResolution, DeletionPlan, PlannedAction, RetentionOutcome, UnresolvedLocator,
        UnresolvedReason, UnresolvedSet,
    },
};

/// Why one planned action did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFailure {
    /// Which reason code the report carries.
    pub reason: UnresolvedReason,
    /// The executor's own words about this locator.
    pub detail: String,
}

/// Performs one planned deletion action.
pub trait RetentionExecutor {
    /// Deletes exactly what `action` names, or says why it did not.
    fn execute(&mut self, action: &PlannedAction) -> Result<(), ExecutionFailure>;
}

/// One retention action's identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionId([u8; 16]);

impl ActionId {
    /// Wraps the caller's action identity bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Returns the lowercase hex spelling written into the journal.
    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

/// Records the plan, runs every action, and settles the result.
///
/// The order matters: the plan is journalled **before** anything is deleted, so
/// a kill during execution leaves a durable record of what was going to be
/// reached. `RB03` is decided before execution starts — a plan with an
/// unresolved class does not run at all, because a deletion that skipped a
/// class it could not resolve would be reporting on a subset of itself.
pub fn settle<E: RetentionExecutor + ?Sized>(
    journal: &mut AppendOnlyJournal,
    action_id: ActionId,
    plan: &DeletionPlan,
    executor: &mut E,
) -> Result<RetentionOutcome, JournalError> {
    let action_hex = action_id.to_hex();
    let unresolved_nodes = plan.unresolved_nodes();
    journal.append(JournalEntry::RetentionPlanned {
        action_id: action_hex.clone(),
        subject_locator: plan.subject().locator_hex(),
        classes: plan
            .enumerated_classes()
            .iter()
            .map(|class| class.as_str().to_owned())
            .collect(),
        unresolved: unresolved_nodes
            .iter()
            .map(UnresolvedLocator::to_row)
            .collect(),
    })?;

    // `RB03`: a class the planner could not answer for stops the deletion
    // before it starts, and the node is named.
    if let Some(set) = UnresolvedSet::new(unresolved_nodes) {
        let outcome = RetentionOutcome::RepairRequired(set);
        record_settlement(journal, &action_hex, &outcome)?;
        return Ok(outcome);
    }

    let mut unresolved = Vec::new();
    for action in plan.actions() {
        if let Err(failure) = executor.execute(&action) {
            unresolved.push(UnresolvedLocator {
                class: action.class,
                locator: action.locator_hex(),
                reason: failure.reason,
                detail: failure.detail,
            });
        }
    }

    let outcome = match UnresolvedSet::new(unresolved) {
        // `RB02` is repair-required rather than partial: a deletion whose
        // backup tombstone did not land is not "mostly done", it is a deletion
        // that will not re-apply on restore and needs an operator.
        Some(set)
            if set
                .locators()
                .iter()
                .any(|row| row.reason == UnresolvedReason::TombstoneWriteFailed) =>
        {
            RetentionOutcome::RepairRequired(set)
        }
        // `RB04`: a partial cache or replica purge is `PARTIAL` with the exact
        // locators that are still there.
        Some(set) => RetentionOutcome::Partial(set),
        None => RetentionOutcome::Complete,
    };
    record_settlement(journal, &action_hex, &outcome)?;
    Ok(outcome)
}

fn record_settlement(
    journal: &mut AppendOnlyJournal,
    action_hex: &str,
    outcome: &RetentionOutcome,
) -> Result<(), JournalError> {
    journal.append(JournalEntry::RetentionSettled {
        action_id: action_hex.to_owned(),
        outcome: outcome.as_str().to_owned(),
        unresolved: outcome
            .unresolved()
            .iter()
            .map(UnresolvedLocator::to_row)
            .collect(),
    })?;
    Ok(())
}

/// Returns every locator a plan would act on, in plan order.
///
/// A caller that wants to preview an action reads this; it is the same list
/// `settle` walks, so a preview cannot drift from what runs.
#[must_use]
pub fn planned_locators(plan: &DeletionPlan) -> Vec<String> {
    plan.actions()
        .iter()
        .map(PlannedAction::locator_hex)
        .collect()
}

/// Returns each class's own reason for holding nothing, in registry order.
#[must_use]
pub fn empty_class_reasons(plan: &DeletionPlan) -> Vec<(&'static str, String)> {
    plan.nodes()
        .iter()
        .filter_map(|node| match &node.resolution {
            ClassResolution::NothingToDelete { reason } => {
                Some((node.class.as_str(), reason.clone()))
            }
            _ => None,
        })
        .collect()
}
