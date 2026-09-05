//! `an_audit_state_is_not_built_from_a_word`.
//!
//! Section 25.4 names four display states and no fifth, and a reading is
//! derived from `P2-U3`'s engine status rather than chosen. Every route from
//! text or from a display word into a reading is tried here.

use std::str::FromStr;

use academic_dashboard::{AuditState, AuditStateReading};

fn main() {
    // There is no `FromStr` on the display word.
    let _parsed = AuditState::from_str("REMAINING");

    // Nor a `str::parse` through it.
    let _turbofished = "SATISFIED".parse::<AuditState>();

    // Nor a `TryFrom<&str>`, nor a `From<&str>`.
    let _tried = AuditState::try_from("UNKNOWN");
    let _converted = AuditState::from("CONFLICT");

    // And a reading is not built from a display word: the only producer takes
    // the engine's own status, so a reading whose word is not the image of its
    // status is unrepresentable, and the difference between `NEEDS` and
    // `NOT_SATISFIED` cannot be discarded on the way in.
    let _from_word = AuditStateReading::of(AuditState::Remaining);

    // Nor is the status settable afterwards.
    let mut reading = AuditStateReading::of(academic_domain::engines::ProofStatus::Needs);
    reading.set_state(AuditState::Satisfied);

    // `spec_word` reads in one direction only.
    let _backwards: AuditState = AuditState::spec_word("PARTIAL");
}
