//! Section 25.2's third line: `녹음 permission 상태: `허용`, `조건부`, `확인 필요`, `금지``.
//!
//! # Four values, closed by the type
//!
//! [`RecordingPermission`] has four arms and no fifth is representable. There
//! is no `FromStr`, no `TryFrom<&str>`, no `From<&str>` and no arm carrying a
//! free-form word, so there is no route from text into a status;
//! `tests/compile_fail/a_recording_permission_is_not_built_from_a_string.rs` is
//! the compiled half, and `permission_status_is_exactly_four_values` reads the
//! four spellings out of the specification's own back quotes and compares them
//! with [`RecordingPermission::ALL`] in both directions and in order.
//!
//! # And they are the image of `P2-G6`'s five, not a second vocabulary
//!
//! `academic_consent::CaptureStatus` is the section 3.7 status: `UNKNOWN`,
//! `PROHIBITED`, `PERMITTED`, `PERMITTED_WITH_CONDITIONS`, `EXPIRED`.
//! [`RecordingPermission::of`] names all five and maps them onto section 25.2's
//! four words, so this crate declares no second status vocabulary.
//!
//! **The compiler does not hold the sixth-status case, and saying it did was
//! wrong.** `CaptureStatus` is `#[non_exhaustive]`, so a `match` on it outside
//! `academic-consent` *must* carry a wildcard arm; a sixth status would compile
//! here and quietly inherit whatever that arm answers. What holds it instead is
//! `permission_status_is_exactly_four_values`, which reads the arms of
//! `CaptureStatus::as_str` out of `crates/consent/src/status.rs` and compares
//! them, in both directions, against the five this function names explicitly. A
//! status added there fails here as an unmapped arm, and the row for that scan
//! is on `docs/contracts/policy-source-scans.md` for exactly that reason.
//!
//! The wildcard answers `확인 필요`. That is the default-deny reading and the
//! same one `CaptureStatus::Unknown` is `Default` for: an unrecognised status
//! must not read as `허용` or `조건부`, and it must not read as `금지` either,
//! because nobody refused.
//!
//! **The map is onto and it is not injective, and both halves are deliberate.**
//! `UNKNOWN` and `EXPIRED` are both `확인 필요`: nobody refused, so neither is
//! `금지`; nothing currently grants, so neither is `허용` or `조건부`; and what
//! the user has to do about either is the same, which is check before
//! recording. Folding them is what section 25.2 asks for by naming four words
//! for a five-valued status. The two are still distinct where the difference is
//! recorded — `academic-consent` keeps them apart, and this crate has no way to
//! change that.
//!
//! # A status is not a permission
//!
//! This crate has no edge to `academic-policy` and no edge to
//! `academic-capture-gate`. It holds no grant, no token and no capability, so
//! nothing here can be mistaken for authority to record: `P2-G6`'s
//! `bind_permission` is still the only thing that turns a recorded permission
//! into a capture capability, and `허용` on this screen is a report of that
//! crate's answer rather than a second one.

use academic_consent::CaptureStatus;

/// The four words section 25.2's third line shows a recording permission as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordingPermission {
    /// `허용`.
    Allowed,
    /// `조건부`.
    Conditional,
    /// `확인 필요`.
    CheckNeeded,
    /// `금지`.
    Forbidden,
}

impl RecordingPermission {
    /// Exhaustive listing, in the order section 25.2's third line names them.
    pub const ALL: [Self; 4] = [
        Self::Allowed,
        Self::Conditional,
        Self::CheckNeeded,
        Self::Forbidden,
    ];

    /// The specification's own word for this status.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::Allowed => "허용",
            Self::Conditional => "조건부",
            Self::CheckNeeded => "확인 필요",
            Self::Forbidden => "금지",
        }
    }

    /// The word section 25.2 shows one of `P2-G6`'s five statuses as.
    ///
    /// All five are named. The wildcard is not a choice: `CaptureStatus` is
    /// `#[non_exhaustive]`, so a `match` on it from outside that crate cannot
    /// omit one, and the compiler therefore says nothing about a sixth status.
    /// `permission_status_is_exactly_four_values` reads `academic-consent`'s
    /// own arms and fails on one this function does not name.
    #[must_use]
    pub const fn of(status: CaptureStatus) -> Self {
        match status {
            CaptureStatus::Permitted => Self::Allowed,
            CaptureStatus::PermittedWithConditions => Self::Conditional,
            CaptureStatus::Unknown | CaptureStatus::Expired => Self::CheckNeeded,
            CaptureStatus::Prohibited => Self::Forbidden,
            // Default deny: not `허용`, not `조건부`, and not a refusal nobody
            // made either.
            _ => Self::CheckNeeded,
        }
    }
}
