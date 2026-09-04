//! Section 22.5: the hypothetical and actual graduation modes are distinct.
//!
//! `P2-U3` owns the actual mode, and this crate has no edge to it of any kind,
//! so a plan cannot reach a determinate verdict by any expression at all — not
//! because a check refuses one, but because the module does not resolve.

use academic_audit::DeterminateVerdict;

fn main() {
    let _verdict: Option<DeterminateVerdict> = None;
}
