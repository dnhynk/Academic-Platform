//! Section 18.4's second bullet, as the absence of a value.
//!
//! `REQUIRED와 WOULD_BENEFIT_FROM은 같은 goal/scope에서는 동시에 둘 수 없다`.
//! A `ConceptStance` holds one `Outlook`, `Outlook` has two variants, and a
//! slot holds one value — so there is no both-at-once variant to select and no
//! second field to put the other one in.
//! `a_concept_stance_cannot_be_assembled` is the other half: the three real
//! fields cannot be filled from outside either.

use academic_repository_classification::{
    BenefitContract, ClassificationKey, ConceptStance, ObservedProof, Outlook, ProofChain,
};

fn chain() -> ProofChain {
    loop {}
}

fn contract() -> BenefitContract {
    loop {}
}

fn key() -> ClassificationKey {
    loop {}
}

fn observed() -> Option<ObservedProof> {
    loop {}
}

fn both_at_once() -> Outlook {
    Outlook::RequiredAndBeneficial {
        chain: chain(),
        contract: contract(),
    }
}

fn two_slots() -> ConceptStance {
    ConceptStance {
        key: key(),
        observed: observed(),
        required: Some(chain()),
        beneficial: Some(contract()),
    }
}

fn main() {
    let _both = both_at_once();
    let _two = two_slots();
}
