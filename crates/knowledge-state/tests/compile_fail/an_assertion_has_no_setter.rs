//! Section 13.4: `new KnowledgeStateAssertion (never in-place mutation)`.
//!
//! Every field is private and there is no setter of any name, so a program that
//! edits a standing assertion does not compile. `revise` returns a new value and
//! leaves this one alone; that is the only way forward.

use academic_domain::MasteryLevel;
use academic_knowledge_state::KnowledgeStateAssertion;

fn edit(assertion: &mut KnowledgeStateAssertion) {
    assertion.mastery_level = MasteryLevel::Fluent;
    assertion.set_mastery_level(MasteryLevel::Fluent);
}

fn main() {}
