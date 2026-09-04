//! Section 3: *알 수 없는 필드는 빈 문자열이 아니라 `UNKNOWN`으로 저장한다.*
//!
//! `Recorded<T>` is deliberately not `Option`: `unwrap_or`,
//! `unwrap_or_default` and `map_or` all take a value to stand in when there is
//! none, and standing something in is the one move section 3 forbids. Nor does
//! a profile have a `Default`, because a profile that arrives by defaulting is
//! one nobody decided to leave empty.

use academic_audit::{InstitutionId, Recorded, StudentProfile};

fn main() {
    let field: Recorded<InstitutionId> = Recorded::Unknown;

    let _stood_in = field.unwrap_or(InstitutionId::new("SNU").unwrap());
    let _defaulted = field.unwrap_or_default();

    let _profile = StudentProfile::default();
}
