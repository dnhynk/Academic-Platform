//! Section 13.1's `FLUENT` row reads `AI 단독 판정 금지`.
//!
//! `AutomaticLevel` is the type an automatic projection returns, and it has no
//! `Fluent` variant. Code on the automatic path therefore cannot name the value
//! at all, which is stronger than refusing it: there is nothing to refuse.

use academic_knowledge_state::AutomaticLevel;

fn main() {
    let _fluent = AutomaticLevel::Fluent;
}
