//! The four relations are independent, and the compiler is one half of that.
//!
//! Section 11.4: *동일·대체·폐지·경과조치는 독립 rule이며 양방향 동일성으로
//! 단순화하지 않는다*. A conversion between any two of them would be the
//! simplification that sentence forbids, so none exists: no `From`, no
//! `TryFrom`, no `Into`, and no accessor on one that returns another.
//!
//! `no_relation_derives_another` in `tests/curriculum_scans.rs` is the other
//! half: it compares the whole set of `impl` blocks and the whole set of
//! signatures in `relation.rs`, so a conversion added under a name this case
//! does not spell fails there.

use academic_curriculum::{
    CourseCodeReuse, EquivalenceRelation, IdentityDecision, ReplacementRelation, RetirementRelation,
};
use academic_domain::{CourseId, DecisionId, TimestampMillis, ValidInterval};

fn main() {
    let earlier: CourseId = "01900000-0000-7000-8000-000000000001".parse().unwrap();
    let later: CourseId = "01900000-0000-7000-8000-000000000002".parse().unwrap();
    let decision: DecisionId = "01900000-0000-7000-8000-000000000003".parse().unwrap();
    let interval = ValidInterval::open_ended(TimestampMillis::new(0));

    let identity =
        IdentityDecision::record(earlier, later, CourseCodeReuse::Same, decision, interval).unwrap();
    let equivalence = EquivalenceRelation::record(earlier, later, interval).unwrap();
    let replacement = ReplacementRelation::record(earlier, later, interval).unwrap();
    let retirement = RetirementRelation::record(earlier, interval);

    // A replacement is not an identity, and there is no route from one to the
    // other.
    let _identity_from_replacement: IdentityDecision = replacement.into();
    let _identity_from_equivalence = IdentityDecision::from(equivalence);
    let _equivalence_from_replacement = EquivalenceRelation::from(replacement);
    let _retirement_from_identity = RetirementRelation::from(identity);
    let _replacement_from_retirement = ReplacementRelation::from(retirement);

    // Nor is there an accessor on one that produces another.
    let _implied = replacement.identity();
    let _reverse = equivalence.reverse();
    let _named = retirement.replacement();
    let _widened = identity.equivalence();
}
