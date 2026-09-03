//! The daemon-side evaluation.
//!
//! # What it is
//!
//! [`authorize`] is the whole of it: the section 3.7 `capture_permission`
//! aggregate that the ledger holds, compared against one request, turned into a
//! time-bounded `CaptureCapabilityToken` and the [`DeviceRuleset`] derived from
//! it. It runs in the privileged parent. The capture process runs on the other
//! side of [`crate::native`], holds no ledger, and receives a ruleset it did not
//! choose.
//!
//! # Why it has no comparison of its own
//!
//! `academic-consent`'s `bind_permission` is where every section 3.7 comparison
//! happens, and `P2-RF10` is why there is exactly one of it. Adding a second
//! comparison here -- a status test, a lifetime test, a media test -- would be
//! the shape that defect had: two paths, one of which can lose a check without
//! anything noticing. So [`authorize`] calls `mint_capture_capability` and adds
//! nothing but the derivation of the ruleset, and its whole text is pinned so a
//! comparison added here has to be added to the pin in the same commit.
//!
//! # What it is not
//!
//! It does not read a database. The `capture_permission` row this evaluation
//! reads is `academic-consent`'s `PermissionRecord`, held in a `ConsentLedger`;
//! writing that record into migration `0006`'s `capture_permission_terms` is
//! open item `C-2` in
//! [the consent contract](../../../docs/contracts/consent-and-capture-permission.md)
//! and this task did not close it. So the aggregate's durable form is still
//! asserted structurally rather than exercised as a round trip.

use academic_consent::{CaptureRequest, ConsentLedger, mint_capture_capability};

use crate::{
    audit::{AuditSubject, CaptureAudit, CaptureRefusal},
    device::DeviceRuleset,
};

/// A minted token and the device ruleset derived from it.
///
/// The fields are private and [`authorize`] is the only place a value is built,
/// so holding one is proof that `mint_capture_capability` returned -- which is
/// proof that `bind_permission` ran.
#[derive(Debug)]
pub struct CaptureAuthorization {
    token: academic_consent::CaptureCapabilityToken,
    ruleset: DeviceRuleset,
}

impl CaptureAuthorization {
    /// The section 3.7 token.
    #[must_use]
    pub const fn token(&self) -> &academic_consent::CaptureCapabilityToken {
        &self.token
    }

    /// The device classes it opens.
    #[must_use]
    pub const fn ruleset(&self) -> &DeviceRuleset {
        &self.ruleset
    }

    /// Moves the token onto a session.
    ///
    /// Crate-private, and consuming: an authorization opens one session, and
    /// there is no public way to take the token off one and put it somewhere
    /// that never ran [`authorize`].
    pub(crate) fn into_token(self) -> academic_consent::CaptureCapabilityToken {
        self.token
    }
}

/// Turns a live permission into a token and the ruleset it opens, or records
/// why it did not.
///
/// The audit is taken by mutable reference for the same reason the ledger is:
/// a refusal here leaves a row on this side of the boundary as well as in the
/// consent ledger, because the two record different things. The ledger records
/// that a capability was refused for a scope; this audit records that a device
/// layer refused to open, which is the row a capture surface reads.
pub fn authorize(
    ledger: &mut ConsentLedger,
    audit: &mut CaptureAudit,
    request: &CaptureRequest,
    now: u64,
) -> Result<CaptureAuthorization, CaptureRefusal> {
    let subject = AuditSubject {
        offering_id: request.offering_id,
        lecture_id: request.lecture_id,
        digest: None,
    };
    let token = match mint_capture_capability(ledger, request, now) {
        Ok(token) => token,
        Err(denial) => {
            return Err(audit.record_refusal(
                CaptureRefusal::from_denial(denial, None),
                subject,
                now,
            ));
        }
    };
    let ruleset = DeviceRuleset::for_token(&token);
    Ok(CaptureAuthorization { token, ruleset })
}
