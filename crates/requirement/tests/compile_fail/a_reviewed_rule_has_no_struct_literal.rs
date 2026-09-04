//! `a_candidate_cannot_be_published`, second half.
//!
//! `ReviewedRule` is the value `ReviewGate::admit` returns and the only thing
//! `RuleSetDraft::include` accepts. If a caller could write one as a struct
//! literal, the gate would be a suggestion.
//!
//! One route per file, for the reason
//! `an_executable_rule_has_no_struct_literal.rs` records: `E0451` comes from a
//! pass that does not run once type checking has already failed, so a literal
//! sharing a file with any other refused route is invisible.

use academic_domain::{Actor, ContentDigest, EntityId, TimestampMillis};
use academic_requirement::{
    CreditAmount, CreditCategory, ReviewAttestation, ReviewedRule, RuleBody, RuleId,
};

fn main() {
    let rule = RuleId::new("total_credits").unwrap();
    let at = TimestampMillis::new(1_800_000_000_000);
    let first: EntityId = "01900000-0000-7000-8000-00000000000b".parse().unwrap();
    let second: EntityId = "01900000-0000-7000-8000-00000000000c".parse().unwrap();

    // Every field of `ReviewedRule` is private, so there is no literal -- and
    // therefore no way to assemble one out of two attestations a caller wrote
    // for itself.
    let _literal = ReviewedRule {
        id: rule.clone(),
        source_rule: academic_domain::engines::RuleId::new("r-12-1").unwrap(),
        body: RuleBody::CreditMinimum {
            category: CreditCategory::new("ALL_RECOGNIZED").unwrap(),
            threshold: CreditAmount::new(130).unwrap(),
        },
        first: ReviewAttestation::file(Actor::User { user_id: first }, rule.clone(), at),
        second: ReviewAttestation::file(Actor::User { user_id: second }, rule, at),
        source_digest: ContentDigest::sha256(b"official"),
    };
}
