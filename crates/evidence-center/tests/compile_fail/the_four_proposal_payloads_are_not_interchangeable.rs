//! `the_four_proposal_payloads_are_not_interchangeable`.
//!
//! If the four classes were one shape with a tag, every line below would
//! compile and the tag would be the only thing keeping a concept merge out of
//! the relation arm. They are four types, so none of them does.

use academic_evidence_center::{
    ConceptMergeProposal, InboxEntry, ProjectClassificationProposal, RelationProposal,
    StateUpdateProposal,
};

fn main() {
    // A merge is not a relation, in either arm.
    let _mislabelled = InboxEntry::Relation(merge());
    let _also = InboxEntry::ConceptMerge(relation());

    // A classification is not a state update.
    let _third = InboxEntry::StateUpdate(classification());

    // And there is no coercion between any two of them.
    let _into: RelationProposal = merge().into();
    let _from = ConceptMergeProposal::from(state_update());
}

// Never reached: the calls above do not compile. Declared so the diagnostics
// are about the payload types and not about missing names.
fn relation() -> RelationProposal {
    unimplemented!()
}

fn merge() -> ConceptMergeProposal {
    unimplemented!()
}

fn classification() -> ProjectClassificationProposal {
    unimplemented!()
}

fn state_update() -> StateUpdateProposal {
    unimplemented!()
}
