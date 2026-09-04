//! Section 11.3's four parts, second half.
//!
//! One route per file, for the reason
//! `a_determinate_verdict_has_no_struct_literal.rs` records.

use academic_audit::{AttemptUsage, EquivalencyDecision, NoAttemptReason, ProofLeaf};
use academic_domain::engines::ProofStatus;
use academic_requirement::{RuleId, RuleType};

fn main() {
    let _literal = ProofLeaf {
        rule: RuleId::new("total_credits").unwrap(),
        source: unimplemented!(),
        attempts: AttemptUsage::NoneUsed(NoAttemptReason::NoMatchingAttempt),
        equivalency: EquivalencyDecision::NoneApplied,
        rule_type: RuleType::CreditMinimum,
        status: ProofStatus::Needs,
        measure: None,
        open_gate: None,
        rule_gate: None,
    };
}
