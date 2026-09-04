//! Section 11.3's four parts are four parameters.
//!
//! *모든 leaf에는 적용 rule ID, source page/paragraph, 사용한 CourseAttempt,
//! equivalency decision이 붙는다.* A leaf missing one of them is not a leaf
//! this crate refuses -- it is a call that cannot be written.

use academic_audit::{AttemptUsage, EquivalencyDecision, NoAttemptReason, ProofLeaf};
use academic_domain::engines::ProofStatus;
use academic_requirement::{RuleId, RuleType};

fn main() {
    let rule = RuleId::new("total_credits").unwrap();

    // No form without the source span.
    let _short = ProofLeaf::new(
        rule.clone(),
        AttemptUsage::NoneUsed(NoAttemptReason::NoMatchingAttempt),
        EquivalencyDecision::NoneApplied,
        RuleType::CreditMinimum,
        ProofStatus::Needs,
        None,
        None,
        None,
    );

    let leaf: ProofLeaf = unimplemented!();

    // And no setter puts one on afterwards.
    let _mutated = leaf.with_source(unimplemented!());
    let _also = leaf.set_attempts(AttemptUsage::NoneUsed(NoAttemptReason::RuleNotEvaluated));
}
