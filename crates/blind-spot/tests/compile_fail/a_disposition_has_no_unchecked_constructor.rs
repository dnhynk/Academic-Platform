//! Section 25.12: `새로운 AI run이 경고를 되살리지 않는다`.
//!
//! `UserDispositionChoice`'s fields are private and `verify` is its one
//! constructor, which runs ADR-003's actor matrix. A disposition minted past it
//! has no representation, so a model run cannot record, change or clear one.

use academic_blind_spot::{UserDisposition, UserDispositionChoice};
use academic_domain::{EntityId, ScopeId, TimestampMillis};

fn forge(
    user_id: EntityId,
    field: EntityId,
    scope_id: ScopeId,
    chosen_at: TimestampMillis,
) -> UserDispositionChoice {
    UserDispositionChoice {
        user_id,
        field,
        scope_id,
        disposition: UserDisposition::NotRelevant,
        hidden_until: None,
        chosen_at,
    }
}

fn main() {}
