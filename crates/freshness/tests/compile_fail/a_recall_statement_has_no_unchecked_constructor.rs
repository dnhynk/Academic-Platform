//! Section 13.3's fifth input is the **user's** own confirmation.
//!
//! `RecallStatement`'s fields are private and `verify` is its one constructor,
//! so a model run cannot assemble one out of the values it already holds. This
//! is `P2-N2`'s `UserConfirmation` shape for the other axis.

use academic_domain::{EntityId, ScopeId, TimestampMillis};
use academic_freshness::{RecallStatement, UserRecall};

fn forge(user: EntityId, concept: EntityId, scope: ScopeId) -> RecallStatement {
    RecallStatement {
        user_id: user,
        concept,
        scope_id: scope,
        statement: UserRecall::CanUseNow,
        stated_at: TimestampMillis::new(0),
    }
}

fn main() {}
