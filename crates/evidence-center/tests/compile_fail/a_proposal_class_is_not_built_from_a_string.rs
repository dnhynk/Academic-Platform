//! `a_proposal_class_is_not_built_from_a_string`.
//!
//! Section 25.13 names four proposal classes. They are four payload types, and
//! `ProposalClass` is a label read *out of* an entry. Every route from text
//! into one is tried here, and none of them exists.

use std::str::FromStr;

use academic_evidence_center::ProposalClass;

fn main() {
    // There is no `FromStr`.
    let _parsed = ProposalClass::from_str("RELATION");

    // Nor a `str::parse` through it.
    let _turbofished = "CONCEPT_MERGE".parse::<ProposalClass>();

    // Nor a `TryFrom<&str>`.
    let _tried = ProposalClass::try_from("PROJECT_CLASSIFICATION");

    // Nor a `From<&str>`.
    let _converted = ProposalClass::from("STATE_UPDATE");

    // And `spec_words` reads in the other direction only: it takes a class and
    // returns text, so it cannot be run backwards.
    let _backwards: ProposalClass = ProposalClass::spec_words("relation");
}
