//! `a_candidate_cannot_be_evaluated`.
//!
//! Section 11.2 forbids a production audit that interprets free text, and
//! `REQ-11-018` states it as an architectural property rather than a check.
//!
//! `RuleSet::evaluate` takes a `RuleId` that names a rule *in that published
//! set*, so a body that was never published cannot be evaluated as though it
//! had been. The free function `evaluate` takes a `&RuleSet` for the same
//! reason. A candidate reaches neither, and its `quoted_source` reaches nothing
//! at all: no function in the crate takes one.

use academic_domain::{Actor, ContentDigest, EntityId, TimestampMillis};
use academic_requirement::{
    AcademicFacts, CreditAmount, CreditCategory, RuleBody, RuleCandidate, RuleId,
};

fn candidate() -> RuleCandidate {
    let run_id: EntityId = "01900000-0000-7000-8000-000000000002".parse().unwrap();
    RuleCandidate::extracted(
        RuleId::new("total_credits").unwrap(),
        academic_domain::engines::RuleId::new("r-12-1").unwrap(),
        RuleBody::CreditMinimum {
            category: CreditCategory::new("ALL_RECOGNIZED").unwrap(),
            threshold: CreditAmount::new(130).unwrap(),
        },
        Actor::ModelRun { run_id },
        "the page says at least 130 credits".to_owned(),
        ContentDigest::sha256(b"official"),
    )
}

fn main() {
    let candidate = candidate();
    let facts = AcademicFacts::new(TimestampMillis::new(1_800_000_000_000));

    // A candidate has no `evaluate`. It is not a rule; it is a proposal.
    let _direct = candidate.evaluate(&facts);

    // Nor does its proposed body: evaluation is a method on the published set,
    // so a body that never entered one has nowhere to be evaluated from.
    let _body = candidate.proposed_body().evaluate(&facts);

    // The free function takes the published set first, and a candidate is not
    // a set.
    let _free = academic_requirement::evaluate(
        &candidate,
        candidate.id(),
        candidate.proposed_body(),
        &facts,
    );

    // And the one sentence in the crate reaches no interpreter: nothing takes
    // the quoted source.
    let _interpreted = academic_requirement::evaluate_text(candidate.quoted_source(), &facts);
}
