//! Refusals at the non-delegable boundary.

use crate::action::NonDelegableAction;

/// Why the command layer refused.
///
/// Every arm names the action it refused, so a refusal a caller records or
/// renders says which of the six it was without the caller having to carry the
/// action alongside the error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NonDelegableError {
    /// A deterministic engine, a model run or an importer submitted a
    /// non-delegable action.
    #[error("{action} is the user's own decision and {actor} is an automatic actor")]
    AutomaticActor {
        /// The refused action.
        action: NonDelegableAction,
        /// The refused actor's stable variant name, from
        /// `academic_domain::Actor::kind_name`.
        actor: &'static str,
    },
    /// A decision event was offered for an action or a subject other than the
    /// one it was recorded for.
    #[error("the decision event for {recorded} does not authorise {offered}")]
    DecisionNamesAnotherAction {
        /// The action the event was recorded for.
        recorded: NonDelegableAction,
        /// The action it was offered for.
        offered: NonDelegableAction,
    },
    /// A decision event was offered for another subject.
    #[error("the decision event for {action} names another subject")]
    DecisionNamesAnotherSubject {
        /// The action both sides agree on.
        action: NonDelegableAction,
    },
}

impl core::fmt::Display for NonDelegableAction {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
