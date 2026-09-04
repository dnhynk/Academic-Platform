//! A review is not attached to a course.
//!
//! There is no `course` accessor on a scope, because there is no course field
//! on one. Section 34's *Course와 Offering 혼동* row is the failure that
//! absence is about.

use academic_review::ReviewScope;

fn course_of(scope: &ReviewScope) {
    let _ = scope.course();
}

fn main() {}
