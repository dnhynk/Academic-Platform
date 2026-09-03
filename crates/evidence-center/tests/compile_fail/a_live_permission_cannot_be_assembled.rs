//! `a_live_permission_cannot_be_assembled`.
//!
//! Section 32.6: *`Provider 정책이 바뀌거나 마지막 확인이 오래되면 permission을
//! 자동 연장하지 않는다`*. A lapsed permission blocks its dependents by failing
//! to produce a value, not by failing a check, so every route to that value
//! other than `PermissionQueue::gate` is tried here and none of them exists.

use academic_domain::{CapturePermissionId, TimestampMillis};
use academic_evidence_center::{LivePermission, PermissionRef};

fn main() {
    let reference = PermissionRef::Capture(permission_id());

    // There is no constructor. The struct literal is
    // `a_live_permission_has_no_struct_literal.rs`, in its own file, because a
    // privacy diagnostic is suppressed when the same file already has a type
    // error and would silently stop proving anything here.
    let _built = LivePermission::new(reference, TimestampMillis::new(0));

    // And no conversion from the expiring record.
    let _converted: LivePermission = reference.into();

    // A gated permission is not `Clone`, so one gate call cannot authorise two
    // dependents.
    let _twice = gated().clone();
}

// Never reached: the lines above do not compile.
fn permission_id() -> CapturePermissionId {
    unimplemented!()
}

fn gated() -> LivePermission {
    unimplemented!()
}
