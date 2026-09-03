//! A stance is derived by the classifier and cannot be assembled beside it.
//!
//! `required_and_benefit_cannot_share_one_scope` shows there is no second
//! outlook field to fill. This is the other half: even the real three fields
//! cannot be filled from outside, so the single-slot shape is not something a
//! caller can route around by building the stance themselves.

use academic_repository_classification::{
    ClassificationKey, ConceptStance, ObservedProof, Outlook, ProofChain,
};

fn chain() -> ProofChain {
    loop {}
}

fn key() -> ClassificationKey {
    loop {}
}

fn observed() -> Option<ObservedProof> {
    loop {}
}

fn main() {
    let _real = ConceptStance {
        key: key(),
        observed: observed(),
        outlook: Some(Outlook::Required(chain())),
    };
}
