//! A derivative's retention has one producer and no setter.
//!
//! Every retention pair in this crate comes out of `inherit_terms`, which calls
//! `P2-G6`'s one inheritance function. `RedactedDerivative` has no public
//! field, no `&mut self` method and no constructor, so terms written onto one
//! after the fact are a program that does not compile.

use academic_consent::{RetentionBound, RetentionTerms};
use academic_student_voice::RedactedDerivative;

fn widen(derivative: &mut RedactedDerivative) {
    derivative.terms = RetentionTerms::new(
        RetentionBound::Until(u64::MAX),
        RetentionBound::Until(u64::MAX),
    );
    derivative.set_terms(RetentionTerms::new(
        RetentionBound::Until(u64::MAX),
        RetentionBound::Until(u64::MAX),
    ));
}

fn main() {}
