//! Each of section 18.2's five steps is the next one's argument.
//!
//! `removing_any_chain_step_blocks_publish` measures the runtime door. This is
//! what makes that door the only one: a link constructor called without its
//! predecessor is not a value that fails validation, it is a program that does
//! not compile. One call per arrow of
//!
//! ```text
//! current code/goal → concrete responsibility or failure scenario
//!   → mechanism that controls it → required concept
//!   → user's insufficient/uncertain evidence
//! ```

use academic_domain::entity_registry::EntityKind;
use academic_repository_analysis::{Locator, SubjectId};
use academic_repository_classification::{
    ConcreteNeed, ControllingMechanism, NeedKind, ProofChain, RequiredConcept, UserEvidenceGap,
};

fn name() -> SubjectId {
    loop {}
}

fn sites() -> Vec<Locator> {
    loop {}
}

fn gap() -> UserEvidenceGap {
    loop {}
}

fn main() {
    // Step two without step one.
    let _need = ConcreteNeed::shown_by(NeedKind::FailureScenario, &name(), sites());
    // Step three without step two.
    let _mechanism = ControllingMechanism::controlling(&name());
    // Step four without step three.
    let _concept = RequiredConcept::realizing(&name(), EntityKind::Concept);
    // Step five without step four.
    let _chain = ProofChain::closed_by(gap());
}
