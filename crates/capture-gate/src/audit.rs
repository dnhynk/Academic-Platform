//! The capture audit, and the closed set of reasons a device stays shut.
//!
//! # Every refusing path appends its row before it returns
//!
//! [`CaptureAudit::record_refusal`] returns the refusal it was handed. That is
//! `academic-consent`'s `record_capture_denial` shape, taken deliberately: a
//! function that returns the value the caller is about to return leaves no
//! early exit that skips the row on its way out, and the call sites are counted
//! by `the_capture_gate_records_every_refusal_it_returns` so a path that
//! returns a refusal without one fails the count rather than passing quietly.
//!
//! # The row carries no captured byte
//!
//! A row holds identifiers, a digest, a length and a time. It holds no chunk,
//! no sample and no frame, for the reason `audit_contains_no_raw_canary` gives
//! in `academic-egress-boundary`: an audit that copies what it is auditing is a
//! second place the bytes live.

use academic_consent::{CaptureDenial, CaptureDenialReason, CaptureStatus};
use academic_domain::{ContentDigest, LectureSessionId, OfferingId};

use crate::device::DeviceClass;

/// Why the device layer refused.
///
/// Closed, and disjoint from `academic-consent`'s [`CaptureDenialReason`]: that
/// enum says which of section 3.7's comparisons failed, and this one says which
/// of *this* layer's did. [`CaptureRefusalReason::PermissionRefused`] is the
/// one arm that carries the other enum, so a row can name both without either
/// vocabulary growing an arm belonging to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CaptureRefusalReason {
    /// The section 3.7 binding refused. The row carries which comparison.
    PermissionRefused,
    /// The class is not one the token's media set opens.
    MediumNotOnToken,
    /// A chunk was offered to a session the boundary had already stopped.
    SessionAlreadyStopped,
    /// A quarantined artefact was asked for its bytes.
    ArtifactQuarantined,
    /// The platform backend could not install the ruleset, so there was no
    /// device layer in force to open a device behind.
    DeviceLayerUnavailable,
}

/// Every refusal reason, in declaration order.
pub const REFUSAL_REASONS: [CaptureRefusalReason; 5] = [
    CaptureRefusalReason::PermissionRefused,
    CaptureRefusalReason::MediumNotOnToken,
    CaptureRefusalReason::SessionAlreadyStopped,
    CaptureRefusalReason::ArtifactQuarantined,
    CaptureRefusalReason::DeviceLayerUnavailable,
];

impl CaptureRefusalReason {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PermissionRefused => "PERMISSION_REFUSED",
            Self::MediumNotOnToken => "MEDIUM_NOT_ON_TOKEN",
            Self::SessionAlreadyStopped => "SESSION_ALREADY_STOPPED",
            Self::ArtifactQuarantined => "ARTIFACT_QUARANTINED",
            Self::DeviceLayerUnavailable => "DEVICE_LAYER_UNAVAILABLE",
        }
    }
}

/// A refusal, with everything a row needs to say which comparison failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("capture refused at the device layer: {reason:?}")]
pub struct CaptureRefusal {
    reason: CaptureRefusalReason,
    class: Option<DeviceClass>,
    denial: Option<CaptureDenial>,
}

impl CaptureRefusal {
    /// Builds a refusal that carries a section 3.7 denial.
    pub(crate) const fn from_denial(denial: CaptureDenial, class: Option<DeviceClass>) -> Self {
        Self {
            reason: CaptureRefusalReason::PermissionRefused,
            class,
            denial: Some(denial),
        }
    }

    /// Builds a refusal this layer reached on its own.
    pub(crate) const fn of(reason: CaptureRefusalReason, class: Option<DeviceClass>) -> Self {
        Self {
            reason,
            class,
            denial: None,
        }
    }

    /// Which of this layer's comparisons failed.
    #[must_use]
    pub const fn reason(&self) -> CaptureRefusalReason {
        self.reason
    }

    /// Which device class was asked for, when one was.
    #[must_use]
    pub const fn class(&self) -> Option<DeviceClass> {
        self.class
    }

    /// The section 3.7 denial behind it, when the binding is what refused.
    #[must_use]
    pub const fn denial(&self) -> Option<CaptureDenial> {
        self.denial
    }

    /// The section 3.7 comparison that failed, when the binding is what
    /// refused.
    #[must_use]
    pub fn denial_reason(&self) -> Option<CaptureDenialReason> {
        self.denial.map(|denial| denial.reason())
    }

    /// The section 3.7 status at the moment of the refusal.
    #[must_use]
    pub fn status(&self) -> Option<CaptureStatus> {
        self.denial.map(|denial| denial.status())
    }
}

/// One appended audit row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureAuditRow {
    reason: CaptureRefusalReason,
    denial_reason: Option<CaptureDenialReason>,
    status: Option<CaptureStatus>,
    class: Option<DeviceClass>,
    offering_id: Option<OfferingId>,
    lecture_id: Option<LectureSessionId>,
    subject_digest: Option<ContentDigest>,
    recorded_at: u64,
}

impl CaptureAuditRow {
    /// Which of this layer's comparisons failed.
    #[must_use]
    pub const fn reason(&self) -> CaptureRefusalReason {
        self.reason
    }

    /// The section 3.7 comparison behind it, when there is one.
    #[must_use]
    pub const fn denial_reason(&self) -> Option<CaptureDenialReason> {
        self.denial_reason
    }

    /// The section 3.7 status at the moment of the refusal.
    #[must_use]
    pub const fn status(&self) -> Option<CaptureStatus> {
        self.status
    }

    /// Which device class was asked for.
    #[must_use]
    pub const fn class(&self) -> Option<DeviceClass> {
        self.class
    }

    /// Which offering.
    #[must_use]
    pub const fn offering_id(&self) -> Option<OfferingId> {
        self.offering_id
    }

    /// Which session.
    #[must_use]
    pub const fn lecture_id(&self) -> Option<LectureSessionId> {
        self.lecture_id
    }

    /// The digest of the thing the row is about -- a token identifier or an
    /// artefact identifier, never a captured byte.
    #[must_use]
    pub const fn subject_digest(&self) -> Option<&ContentDigest> {
        self.subject_digest.as_ref()
    }

    /// When the row was appended.
    #[must_use]
    pub const fn recorded_at(&self) -> u64 {
        self.recorded_at
    }
}

/// What a row is about, gathered at the call site so the row cannot be built
/// without it.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AuditSubject {
    pub(crate) offering_id: Option<OfferingId>,
    pub(crate) lecture_id: Option<LectureSessionId>,
    pub(crate) digest: Option<ContentDigest>,
}

/// The append-only capture audit.
///
/// It has one push and no removal, which is `CONTRIBUTING.md`'s second rule.
#[derive(Debug, Clone, Default)]
pub struct CaptureAudit {
    rows: Vec<CaptureAuditRow>,
}

impl CaptureAudit {
    /// A new audit with no rows.
    #[must_use]
    pub const fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Appends the row for `refusal` and returns the refusal.
    ///
    /// Returning the argument is the whole mechanism: a caller writes
    /// `return Err(audit.record_refusal(..))` and there is no shape of that
    /// statement that returns the refusal without appending the row.
    pub(crate) fn record_refusal(
        &mut self,
        refusal: CaptureRefusal,
        subject: AuditSubject,
        now: u64,
    ) -> CaptureRefusal {
        self.rows.push(CaptureAuditRow {
            reason: refusal.reason(),
            denial_reason: refusal.denial_reason(),
            status: refusal.status(),
            class: refusal.class(),
            offering_id: subject.offering_id,
            lecture_id: subject.lecture_id,
            subject_digest: subject.digest,
            recorded_at: now,
        });
        refusal
    }

    /// Every row, in append order.
    #[must_use]
    pub fn rows(&self) -> &[CaptureAuditRow] {
        &self.rows
    }

    /// How many rows carry `reason`.
    #[must_use]
    pub fn count_of(&self, reason: CaptureRefusalReason) -> usize {
        self.rows.iter().filter(|row| row.reason == reason).count()
    }
}
