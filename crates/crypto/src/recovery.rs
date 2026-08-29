//! The recovery recipient: a pinned Argon2id profile over a 256-bit secret.
//!
//! ADR-005 requires "a versioned, reviewed parameter profile pinned in the
//! recipient record". Pinning here means two things together: the profile is
//! written into the record verbatim and read back on every unlock, and the
//! reader accepts *only* a profile from [`PINNED_PROFILES`]. An unknown
//! identifier, or a known identifier carrying weakened costs, is refused rather
//! than honoured -- so a record edited on disk cannot downgrade the KDF.
//!
//! This crate exposes no word-level entry point. The 24-word encoding of the
//! recovery secret belongs to `P2-K4` with the recovery profiles, so no API
//! here can be asked about an individual word and none can answer which word of
//! a phrase was wrong.

use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::{
    keys::{IDENTIFIER_BYTES, KEY_BYTES, RecipientWrapKey, RecoverySecret},
    recipient::RecordError,
};

/// A reviewed, versioned Argon2id parameter set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2idProfile {
    /// Stable profile identifier written into the recipient record.
    pub identifier: &'static str,
    /// Memory cost in kibibytes.
    pub memory_kib: u32,
    /// Time cost (passes).
    pub iterations: u32,
    /// Lanes.
    pub parallelism: u32,
}

/// The profile `P2-K1` pins.
///
/// The input is a 256-bit secret, so the KDF is defence in depth rather than
/// the security boundary; the cost is chosen so a *replacement* machine can
/// always run it, because a recovery that fails for want of memory defeats the
/// purpose of a recovery recipient.
pub const RECOVERY_ARGON2ID_V1: Argon2idProfile = Argon2idProfile {
    identifier: "RECOVERY_ARGON2ID_V1",
    memory_kib: 65_536,
    iterations: 3,
    parallelism: 1,
};

/// Every profile this build will accept from a record.
pub const PINNED_PROFILES: &[Argon2idProfile] = &[RECOVERY_ARGON2ID_V1];

impl Argon2idProfile {
    /// Resolves a profile read back from a record against the pinned set.
    ///
    /// Both the identifier and every cost must match; a record claiming a
    /// pinned identifier with weakened costs is refused.
    pub(crate) fn from_record(
        identifier: &str,
        memory_kib: u32,
        iterations: u32,
        parallelism: u32,
    ) -> Result<Self, RecordError> {
        PINNED_PROFILES
            .iter()
            .copied()
            .find(|pinned| {
                pinned.identifier == identifier
                    && pinned.memory_kib == memory_kib
                    && pinned.iterations == iterations
                    && pinned.parallelism == parallelism
            })
            .ok_or(RecordError::UnpinnedKdfProfile)
    }

    /// Derives the wrapping key for a recovery recipient.
    ///
    /// Public because `P2-K4` wraps its own backup root under the same pinned
    /// profile: a backup recipient must be derived by exactly this KDF with
    /// exactly these costs, and a second implementation of it would be a
    /// second place the parameters could drift.
    pub fn derive_wrap_key(
        self,
        secret: &RecoverySecret,
        salt: &[u8; IDENTIFIER_BYTES],
    ) -> Result<RecipientWrapKey, RecoveryError> {
        let parameters = Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(KEY_BYTES),
        )
        .map_err(|_| RecoveryError::Parameters)?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters);
        let mut derived = Zeroizing::new([0_u8; KEY_BYTES]);
        argon
            .hash_password_into(secret.expose_secret(), salt, derived.as_mut())
            .map_err(|_| RecoveryError::Derivation)?;
        Ok(RecipientWrapKey::from_zeroizing(derived))
    }
}

/// Failure of the recovery key derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RecoveryError {
    /// The pinned parameters were rejected by the KDF implementation.
    #[error("the pinned Argon2id parameters were rejected")]
    Parameters,
    /// The KDF failed to produce output.
    #[error("the Argon2id derivation failed")]
    Derivation,
}

/// Base wait after the first failed recovery unlock.
const THROTTLE_BASE_MS: u64 = 250;
/// Longest wait the throttle will impose.
const THROTTLE_CAP_MS: u64 = 30_000;
/// Doublings after which the wait is capped.
const THROTTLE_MAX_DOUBLINGS: u32 = 7;

/// Rate limiter for recovery unlock attempts.
///
/// The caller supplies a monotonic millisecond reading; this type holds no
/// clock, so its behaviour is fully determined by the test that drives it.
/// It counts attempts only -- it never learns anything about the secret, and
/// it treats every wrong secret identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnlockThrottle {
    consecutive_failures: u32,
    blocked_until_ms: u64,
}

impl Default for UnlockThrottle {
    fn default() -> Self {
        Self::new()
    }
}

impl UnlockThrottle {
    /// Creates an unthrottled counter.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            consecutive_failures: 0,
            blocked_until_ms: 0,
        }
    }

    /// Returns the wait still owed at `now_ms`, or `None` when an attempt is allowed.
    #[must_use]
    pub const fn wait_remaining_ms(&self, now_ms: u64) -> Option<u64> {
        if now_ms < self.blocked_until_ms {
            Some(self.blocked_until_ms - now_ms)
        } else {
            None
        }
    }

    /// Records one failed attempt and extends the wait.
    pub const fn record_failure(&mut self, now_ms: u64) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        let doublings = if self.consecutive_failures > 0 {
            self.consecutive_failures - 1
        } else {
            0
        };
        let shift = if doublings > THROTTLE_MAX_DOUBLINGS {
            THROTTLE_MAX_DOUBLINGS
        } else {
            doublings
        };
        let delay = match THROTTLE_BASE_MS.checked_shl(shift) {
            Some(value) if value < THROTTLE_CAP_MS => value,
            _ => THROTTLE_CAP_MS,
        };
        self.blocked_until_ms = now_ms.saturating_add(delay);
    }

    /// Clears the counter after a successful unlock.
    pub const fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.blocked_until_ms = 0;
    }

    /// Returns how many consecutive failures have been recorded.
    #[must_use]
    pub const fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pinned_profile_is_versioned_and_exact() {
        assert_eq!(RECOVERY_ARGON2ID_V1.identifier, "RECOVERY_ARGON2ID_V1");
        assert_eq!(RECOVERY_ARGON2ID_V1.memory_kib, 65_536);
        assert_eq!(RECOVERY_ARGON2ID_V1.iterations, 3);
        assert_eq!(RECOVERY_ARGON2ID_V1.parallelism, 1);
        assert_eq!(PINNED_PROFILES.len(), 1);
    }

    #[test]
    fn a_record_cannot_downgrade_or_invent_a_profile() {
        assert_eq!(
            Argon2idProfile::from_record("RECOVERY_ARGON2ID_V1", 65_536, 3, 1),
            Ok(RECOVERY_ARGON2ID_V1)
        );
        for (identifier, memory, iterations, parallelism) in [
            ("RECOVERY_ARGON2ID_V1", 8, 3, 1),
            ("RECOVERY_ARGON2ID_V1", 65_536, 1, 1),
            ("RECOVERY_ARGON2ID_V1", 65_536, 3, 4),
            ("RECOVERY_ARGON2ID_V0", 65_536, 3, 1),
            ("", 65_536, 3, 1),
        ] {
            assert_eq!(
                Argon2idProfile::from_record(identifier, memory, iterations, parallelism),
                Err(RecordError::UnpinnedKdfProfile),
                "{identifier} {memory} {iterations} {parallelism}"
            );
        }
    }

    #[test]
    fn the_throttle_backs_off_and_clears_on_success() {
        let mut throttle = UnlockThrottle::new();
        assert_eq!(throttle.wait_remaining_ms(0), None);

        throttle.record_failure(0);
        assert_eq!(throttle.wait_remaining_ms(0), Some(THROTTLE_BASE_MS));
        assert_eq!(throttle.wait_remaining_ms(THROTTLE_BASE_MS), None);

        throttle.record_failure(THROTTLE_BASE_MS);
        assert_eq!(
            throttle.wait_remaining_ms(THROTTLE_BASE_MS),
            Some(THROTTLE_BASE_MS * 2)
        );

        for _ in 0..20 {
            throttle.record_failure(0);
        }
        assert_eq!(throttle.wait_remaining_ms(0), Some(THROTTLE_CAP_MS));

        throttle.record_success();
        assert_eq!(throttle.wait_remaining_ms(0), None);
        assert_eq!(throttle.consecutive_failures(), 0);
    }
}
