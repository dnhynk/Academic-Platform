//! `an_executable_rule_has_no_public_constructor`.
//!
//! `ExecutableRule` is what a published set holds and what an audit runs. Its
//! fields are private and the one expression in the crate that builds it is
//! inside `RuleSetDraft::include`, which takes a `ReviewedRule` by value.
//!
//! So the last route from a candidate to an audit is to build the executable
//! rule directly, or to mutate a published set into holding one. Neither is a
//! call that exists: `RuleSet` has no `&mut self` method and no public field,
//! and `ExecutableRule` has no constructor at all.

use academic_domain::{Actor, ContentDigest, EntityId};
use academic_requirement::{
    CreditAmount, CreditCategory, ExecutableRule, RuleBody, RuleCandidate, RuleId, RuleSet,
};

fn candidate() -> RuleCandidate {
    let run_id: EntityId = "01900000-0000-7000-8000-000000000002".parse().unwrap();
    RuleCandidate::extracted(
        RuleId::new("total_credits").unwrap(),
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

    // No constructor takes a body.
    let _built = ExecutableRule::new(
        candidate.id().clone(),
        candidate.proposed_body().clone(),
        candidate.source_digest(),
    );

    // And a published set cannot be opened up to receive one.
    //
    // The struct-literal route is `an_executable_rule_has_no_struct_literal.rs`
    // and not a fourth line here: `E0451` comes from the privacy pass, which
    // does not run once the lines above have failed to type-check, so a literal
    // in this file would produce no diagnostic at all.
    let mut set = published();
    set.push_rule(_built);
    set.rules.push(rule());
}

// Never reached: the calls above do not compile.
fn published() -> RuleSet {
    unimplemented!()
}

fn rule() -> ExecutableRule {
    unimplemented!()
}
