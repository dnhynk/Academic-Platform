//! `an_executable_rule_has_no_public_constructor`, second half.
//!
//! This case is one route and nothing else, on purpose. The privacy checker
//! that emits `E0451` runs *after* type checking, so a file whose other lines
//! already failed to type-check never reaches it and the diagnostic this case
//! exists for never appears. The first version of this suite had the literal
//! sitting beside `ExecutableRule::new` and three other routes, and its
//! committed `.stderr` recorded three errors, none of them about the literal:
//! the half that was supposed to prove the fields are private proved nothing.
//!
//! One route per file is the rule that follows from it, and it is the same rule
//! `docs/contracts/policy-source-scans.md` states for injections -- one at a
//! time, each in its own build, so one failure cannot stand in for another.

use academic_domain::ContentDigest;
use academic_requirement::{CreditAmount, CreditCategory, ExecutableRule, RuleBody, RuleId};

fn main() {
    // Every field of `ExecutableRule` is private, so there is no literal.
    let _literal = ExecutableRule {
        id: RuleId::new("total_credits").unwrap(),
        source_rule: academic_domain::engines::RuleId::new("r-12-1").unwrap(),
        body: RuleBody::CreditMinimum {
            category: CreditCategory::new("ALL_RECOGNIZED").unwrap(),
            threshold: CreditAmount::new(130).unwrap(),
        },
        source_digest: ContentDigest::sha256(b"official"),
    };
}
