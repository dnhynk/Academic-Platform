//! Section 13.4: `사용자가 직접 확인한 state는 AI가 낮추거나 높이지 못한다`.
//!
//! `UserConfirmation`'s fields are private and `verify` is its one constructor,
//! so a model run cannot assemble one out of the values it already holds. This
//! is `P2-N1`'s `VerifiedCuratorApproval` shape for a different action.

use academic_domain::{EntityId, MasteryLevel, ScopeId, TimestampMillis};
use academic_knowledge_state::UserConfirmation;

fn forge(user: EntityId, concept: EntityId, scope: ScopeId) -> UserConfirmation {
    UserConfirmation {
        user_id: user,
        concept,
        scope_id: scope,
        level: MasteryLevel::Fluent,
        confirmed_at: TimestampMillis::new(0),
    }
}

fn main() {}
