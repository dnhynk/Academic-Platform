//! `a_live_permission_has_no_struct_literal`.
//!
//! The other half of `a_live_permission_cannot_be_assembled`, in its own file.
//! A field-privacy diagnostic is suppressed when the same file already carries
//! a type error, so a struct literal written beside the constructor cases would
//! compile away to nothing and prove nothing.

use academic_domain::TimestampMillis;
use academic_evidence_center::{LivePermission, PermissionRef};

fn main() {
    let _forged = LivePermission {
        reference: reference(),
        proved_at: TimestampMillis::new(0),
    };
}

// Never reached: the literal above does not compile.
fn reference() -> PermissionRef {
    unimplemented!()
}
