//! Section 18.3's contract cannot be built from the concept alone.
//!
//! `trigger와 trade-off 없는 "있으면 좋은 기술" 목록은 만들지 않는다`.
//! `generic_nice_to_have_list_produces_zero_findings` measures the door a
//! model-authored list arrives through. This is why there is no second one: a
//! `BenefitContract` has private fields, no `Default`, and one constructor that
//! takes all four parts.

use academic_repository_analysis::SubjectId;
use academic_repository_classification::BenefitContract;

fn concept() -> SubjectId {
    loop {}
}

fn main() {
    let _by_default: BenefitContract = BenefitContract::default();
    let _by_literal = BenefitContract {
        concept: "graphql".to_owned(),
    };
    let _by_concept_alone = BenefitContract::new(&concept());
}
