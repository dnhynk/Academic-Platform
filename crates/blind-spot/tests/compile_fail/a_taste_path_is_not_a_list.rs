//! Section 23: `작은 taste path—한 강의, 한 chapter, 한 toy experiment—`.
//!
//! A `TastePath` holds one step and not a list, so a second step is not a value
//! this crate refuses at the end of a constructor — it is a value that cannot be
//! written.

use academic_blind_spot::{TastePath, TasteStep, UserDispositionChoice};
use academic_domain::EntityId;

fn two_steps(
    choice: &UserDispositionChoice,
    key: EntityId,
) -> Result<TastePath, academic_blind_spot::BlindSpotError> {
    TastePath::for_explore(choice, key, vec![TasteStep::OneLecture, TasteStep::OneChapter])
}

fn main() {}
