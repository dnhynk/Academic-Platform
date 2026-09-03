//! The sealed optimistic update, and the receipt that is its only exit.
//!
//! ADR-001: "A UI optimistic update is not canonical until the core returns an
//! immutable object/event ID and local acceptance receipt." The seal is the
//! absence of an exit, so what is deliberately missing is part of the contract:
//!
//! - no `value`, `get` or `into_inner` on [`Optimistic`] -- nothing returns `T`;
//! - no [`Deref`](std::ops::Deref), [`AsRef`] or [`Borrow`](std::borrow::Borrow);
//! - no `From<Optimistic<T>> for T`, and therefore no `Into`;
//! - no `map`/`and_then` taking a caller closure, which would hand `T` out
//!   under another name;
//! - no [`serde::Serialize`], so it cannot leave as bytes either;
//! - a [`Debug`] that redacts the value rather than printing it.
//!
//! [`Optimistic::confirm`] is the exit. It consumes the wrapper, compares every
//! field the core bound the request to, and returns [`Canonical`] only when all
//! four match. `tests/compile_fail/` holds the exits that must not compile.
//!
//! This is the same *kind* of seal as `academic_scenario::Proposed<T>`, in
//! `crates/scenario/src/proposed.rs`, and is deliberately not that type.
//! `Proposed<T>` has no promotion at all, because a projection becomes
//! canonical only through a user decision recorded as its own event; giving it
//! one would weaken a contract this task does not own. An optimistic update has
//! exactly one promotion, and it is a receipt. There is no dependency edge
//! between the two crates and there should not be one.

use core::fmt;

use academic_rpc::generated::ImmutableReceipt;
use thiserror::Error;

/// The request identity an optimistic update was submitted under.
///
/// Every field is one the core binds its receipt to. They are compared as a
/// whole, so a receipt that matches three of the four promotes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedRequest {
    /// Sixteen opaque bytes.
    pub request_id: [u8; 16],
    /// Sixteen opaque bytes.
    pub client_instance_id: [u8; 16],
    /// Thirty-two bytes.
    pub idempotency_key: [u8; 32],
    /// Thirty-two SHA-256 bytes.
    pub request_digest: [u8; 32],
}

/// Why a receipt did not promote an optimistic update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NotCanonical {
    /// The receipt answers a different request.
    #[error("the receipt names a different request")]
    RequestIdMismatch,
    /// The receipt answers a different client.
    #[error("the receipt names a different client instance")]
    ClientInstanceIdMismatch,
    /// The receipt answers a different submission of the same request.
    #[error("the receipt carries a different idempotency key")]
    IdempotencyKeyMismatch,
    /// The receipt answers a different payload.
    #[error("the receipt carries a different request digest")]
    RequestDigestMismatch,
}

/// A value the user has seen and the core has not accepted.
pub struct Optimistic<T> {
    value: T,
    request: SubmittedRequest,
}

impl<T> Optimistic<T> {
    /// Seals a value the surface is showing ahead of acceptance.
    #[must_use]
    pub const fn new(value: T, request: SubmittedRequest) -> Self {
        Self { value, request }
    }

    /// The request identity a receipt has to match.
    #[must_use]
    pub const fn request(&self) -> &SubmittedRequest {
        &self.request
    }

    /// Presents a receipt, consuming the wrapper.
    ///
    /// This is the only path from an optimistic value to a canonical one. It
    /// takes `self` by value, so a refused receipt does not leave a wrapper
    /// behind that a second, luckier receipt could be tried against.
    ///
    /// # Errors
    ///
    /// Returns the first field that did not match, and no value.
    pub fn confirm(self, receipt: &ImmutableReceipt) -> Result<Canonical<T>, NotCanonical> {
        if receipt.request_id != self.request.request_id {
            return Err(NotCanonical::RequestIdMismatch);
        }
        if receipt.client_instance_id != self.request.client_instance_id {
            return Err(NotCanonical::ClientInstanceIdMismatch);
        }
        if receipt.idempotency_key != self.request.idempotency_key {
            return Err(NotCanonical::IdempotencyKeyMismatch);
        }
        if receipt.request_digest != self.request.request_digest {
            return Err(NotCanonical::RequestDigestMismatch);
        }
        Ok(Canonical {
            value: self.value,
            receipt: receipt.clone(),
        })
    }
}

/// `Debug` redacts the optimistic value.
///
/// `missing_debug_implementations` is denied workspace-wide, so this type needs
/// a `Debug`. A derived one would print the unaccepted value into every log
/// line that formats a pending edit, and a value recovered from a log is
/// exactly the leak the seal exists to prevent.
impl<T> fmt::Debug for Optimistic<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Optimistic")
            .field("value", &"<unaccepted>")
            .field("request", &self.request)
            .finish()
    }
}

/// A value the core accepted, with the receipt that says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canonical<T> {
    value: T,
    receipt: ImmutableReceipt,
}

impl<T> Canonical<T> {
    /// The accepted value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// The receipt that made it canonical.
    #[must_use]
    pub const fn receipt(&self) -> &ImmutableReceipt {
        &self.receipt
    }

    /// Takes the accepted value out.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}
