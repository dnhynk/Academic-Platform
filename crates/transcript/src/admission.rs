//! The admission gate every profile-touching import passes through.
//!
//! `P2-K6` built the verifier; it did not open admission. The compiled
//! acceptance public key is unprovisioned and the candidate receipt carries two
//! of five platform rows, so `AdmissionVerifier::verify` fails closed on every
//! profile in this repository. Refusal is therefore not an error path this
//! module hopes to reach — it is the behaviour of every import today, and
//! `import_without_admission_receipt_is_refused` is the row that fixes it.
//!
//! Only `AdmissionVerifier::verify`'s public contract is used here: the
//! `Result`, and `AdmissionError::code`. Nothing reads the receipt bytes, the
//! platform set, the five stages, or the posture emitter, so the repair
//! `T131/P2-RF7` is making to `crates/admission` cannot change what this gate
//! does — only whether it opens.
//!
//! # Where the gate is, exactly
//!
//! It is on the operations that reach a profile: beginning a durable import
//! session, and sealing a transcript original into a vault. It is **not** on
//! parsing bytes into an in-memory [`crate::record::NormalizedTranscript`],
//! which writes nothing and touches no profile. Saying "no import is possible"
//! would be stronger than the code: what is true is that nothing durable
//! happens without a verified receipt.

use std::path::Path;

use academic_admission::AdmissionVerifier;

use crate::TranscriptError;

/// Proof that one profile's admission receipt verified.
///
/// Unforgeable outside this module: the fields are private and the only
/// constructor is [`Self::open`]. Every gated entry point takes one by
/// reference, so "this import was admitted" is carried by the type rather than
/// re-checked at each call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedImport {
    receipt_digest: String,
    platforms: Vec<String>,
}

impl AdmittedImport {
    /// Verifies the profile's admission receipt, or refuses.
    ///
    /// # Errors
    ///
    /// Returns [`TranscriptError::AdmissionRefused`] carrying the verifier's
    /// own stable error code. Today that is `ACCEPTANCE_KEY_UNPROVISIONED` for
    /// a profile that has a candidate receipt and `RECEIPT_ABSENT` for one that
    /// does not.
    pub fn open(profile_root: &Path) -> Result<Self, TranscriptError> {
        match AdmissionVerifier::verify(profile_root) {
            Ok(verified) => Ok(Self {
                receipt_digest: verified.receipt_digest().to_owned(),
                platforms: verified.platforms().to_vec(),
            }),
            Err(error) => Err(TranscriptError::AdmissionRefused { code: error.code() }),
        }
    }

    /// Returns the verified receipt digest.
    #[must_use]
    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }

    /// Returns the platform set the receipt authenticated.
    #[must_use]
    pub fn platforms(&self) -> &[String] {
        &self.platforms
    }

    /// Fabricates the capability, for the fault lane only.
    ///
    /// Admission is closed, so every gated entry point in this crate is
    /// unreachable in a build that does not select this feature — including the
    /// import session the `IN04` kill matrix has to drive and the seal
    /// `transcript_original_is_ciphertext_at_rest` has to observe. A machine
    /// nothing can start is not evidence about the machine, so the hole is
    /// here: one constructor, greppable by name, compiled only by
    /// `phase2-fault-injection`.
    ///
    /// It is deliberately *not* an ungated mechanism sitting beside each gated
    /// entry point. That shape leaves the gate bypassable by a future caller in
    /// a product build; this one cannot be called from a build that does not
    /// select a test-only feature, and `phase1-scaffold-policy.test.mjs` checks
    /// that no product binary selects it.
    #[cfg(feature = "phase2-fault-injection")]
    #[must_use]
    pub fn for_fault_injection_only() -> Self {
        Self {
            receipt_digest: String::from("fault-injection-lane"),
            platforms: Vec::new(),
        }
    }
}
