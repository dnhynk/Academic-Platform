//! The running capture, the boundary it stops at, and the seal that decides
//! which artefact it becomes.
//!
//! # Three checks, and each one is a different question
//!
//! [`open_device`] asks whether this token opens this device class.
//! [`CaptureSession::record_chunk`] asks, for every chunk, whether the section
//! 3.7 permission still holds -- by re-running the whole binding through
//! `continue_capture`, not by comparing the token's own `not_after`, because a
//! token minted at one instant says nothing about a later one.
//! [`CaptureSession::seal`] asks the question neither of the first two can:
//! whether every chunk that *was* recorded re-binds at its own instant.
//!
//! The third is what makes the second falsifiable. Delete the
//! `continue_capture` call from `record_chunk` and chunks keep being appended
//! past the boundary; `seal` then finds the first one that does not re-bind and
//! quarantines the artefact, so the injection is observed twice by two
//! independent mechanisms rather than once by the check that was removed.
//!
//! # The session holds no operating-system handle in the default lane
//!
//! With `native-capture` off there is no device handle anywhere in this crate,
//! and [`DeviceLayer::Bookkeeping`] is what a session records. What the feature
//! adds is the operating system refusing the open, measured per platform in
//! [the capture device gate contract](../../../docs/contracts/capture-device-gate.md).

use academic_consent::{
    CaptureCapabilityToken, CaptureDenial, ConsentLedger, RetentionTerms, bind_permission,
    continue_capture,
};
use academic_domain::{ContentDigest, LectureSessionId, OfferingId};

use crate::{
    artifact::{CaptureArtifact, ChunkRecord, TimelineGap, ViolationRisk},
    audit::{AuditSubject, CaptureAudit, CaptureRefusal, CaptureRefusalReason},
    daemon::CaptureAuthorization,
    device::{DeviceClass, DeviceLayer},
};

/// A capture that has a device open.
///
/// The fields are private and [`open_device`] is the only place a value is
/// built, so holding one is proof that a token opened this class. There is no
/// public constructor and `tests/compile_fail/` holds the program that shows
/// there is none.
#[derive(Debug)]
pub struct CaptureSession {
    token: CaptureCapabilityToken,
    class: DeviceClass,
    layer: DeviceLayer,
    offering_id: OfferingId,
    lecture_id: LectureSessionId,
    retention: RetentionTerms,
    chunks: Vec<ChunkRecord>,
    bytes: Vec<u8>,
    gap: Option<TimelineGap>,
}

/// Opens one device class under one authorization.
///
/// The two ways this refuses are the two questions the device layer owns: the
/// platform is not enforcing a ruleset it was asked to enforce, and the class
/// is not one this token's media set opens. The second is
/// `audio_only_permission_denies_camera`: a grant listing `AUDIO` derives a
/// ruleset holding `MICROPHONE`, and `CAMERA` is refused here rather than
/// wherever a camera would have been opened.
pub fn open_device(
    ledger: &mut ConsentLedger,
    audit: &mut CaptureAudit,
    authorization: CaptureAuthorization,
    class: DeviceClass,
    layer: DeviceLayer,
    now: u64,
) -> Result<CaptureSession, CaptureRefusal> {
    let token = authorization.token();
    let subject = AuditSubject {
        offering_id: Some(token.bound().offering_id()),
        lecture_id: Some(token.bound().lecture_id()),
        digest: Some(*token.token_id()),
    };
    if layer == DeviceLayer::Unavailable {
        return Err(audit.record_refusal(
            CaptureRefusal::of(CaptureRefusalReason::DeviceLayerUnavailable, Some(class)),
            subject,
            now,
        ));
    }
    if !authorization.ruleset().permits(class) {
        return Err(audit.record_refusal(
            CaptureRefusal::of(CaptureRefusalReason::MediumNotOnToken, Some(class)),
            subject,
            now,
        ));
    }
    if let Err(denial) = continue_capture(ledger, token, now) {
        return Err(audit.record_refusal(
            CaptureRefusal::from_denial(denial, Some(class)),
            subject,
            now,
        ));
    }
    let offering_id = token.bound().offering_id();
    let lecture_id = token.bound().lecture_id();
    let retention = token.bound().retention();
    Ok(CaptureSession {
        token: authorization.into_token(),
        class,
        layer,
        offering_id,
        lecture_id,
        retention,
        chunks: Vec::new(),
        bytes: Vec::new(),
        gap: None,
    })
}

impl CaptureSession {
    /// Which device class is open.
    #[must_use]
    pub const fn class(&self) -> DeviceClass {
        self.class
    }

    /// What is enforcing the ruleset.
    #[must_use]
    pub const fn layer(&self) -> DeviceLayer {
        self.layer
    }

    /// The opaque identifier of the token behind it.
    #[must_use]
    pub const fn token_id(&self) -> &ContentDigest {
        self.token.token_id()
    }

    /// When the token stops.
    #[must_use]
    pub const fn not_after(&self) -> u64 {
        self.token.not_after()
    }

    /// How many chunks have been recorded.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// The gap, once the boundary has stopped this capture.
    #[must_use]
    pub const fn gap(&self) -> Option<TimelineGap> {
        self.gap
    }

    /// Records one chunk, if the permission still holds at `now`.
    ///
    /// The first statement re-runs the whole section 3.7 binding. It is not a
    /// comparison against the token's own `not_after`: the grant can expire,
    /// the scope interval can end, and a superseding record can arrive, and
    /// only the binding sees all three.
    ///
    /// A refusal stops the capture. The gap is opened at `now`, the row is
    /// appended, and every later chunk is refused as
    /// [`CaptureRefusalReason::SessionAlreadyStopped`], so a caller that
    /// ignores the error does not resume across the boundary.
    pub fn record_chunk(
        &mut self,
        ledger: &mut ConsentLedger,
        audit: &mut CaptureAudit,
        bytes: &[u8],
        now: u64,
    ) -> Result<(), CaptureRefusal> {
        let subject = self.subject();
        if self.gap.is_some() {
            return Err(audit.record_refusal(
                CaptureRefusal::of(
                    CaptureRefusalReason::SessionAlreadyStopped,
                    Some(self.class),
                ),
                subject,
                now,
            ));
        }
        if let Err(denial) = continue_capture(ledger, &self.token, now) {
            self.gap = Some(TimelineGap::opened(
                now,
                CaptureRefusalReason::PermissionRefused,
                Some(denial.reason()),
            ));
            return Err(audit.record_refusal(
                CaptureRefusal::from_denial(denial, Some(self.class)),
                subject,
                now,
            ));
        }
        let seq = u32::try_from(self.chunks.len()).unwrap_or(u32::MAX);
        self.chunks.push(ChunkRecord::build(
            seq,
            now,
            bytes.len(),
            ContentDigest::sha256(bytes),
        ));
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    /// Seals the capture and decides which artefact it is.
    ///
    /// Every recorded chunk is re-bound at its own instant. A chunk that does
    /// not re-bind is a chunk that was recorded outside the permission that was
    /// supposed to cover it -- one past the boundary, or one whose offering an
    /// authority has since refused in writing -- and the artefact is
    /// quarantined in the section 34.1 state. Chunks that all re-bind are
    /// releasable, including a capture the boundary stopped: fault `CP01` says
    /// those are retained under the expired scope with an explicit gap, and the
    /// gap travels on the manifest.
    pub fn seal(
        self,
        ledger: &ConsentLedger,
        audit: &mut CaptureAudit,
        now: u64,
    ) -> CaptureArtifact {
        let digest = ContentDigest::sha256(&self.bytes);
        let byte_len = self.bytes.len();
        let subject = AuditSubject {
            offering_id: Some(self.offering_id),
            lecture_id: Some(self.lecture_id),
            digest: Some(digest),
        };
        let violation = self.first_unbound_chunk(ledger);
        let class = self.class;
        let manifest =
            CaptureArtifact::manifest_of(self.chunks, byte_len, digest, self.retention, self.gap);
        match violation {
            Some((risk, denial)) => {
                let _ = audit.record_refusal(
                    CaptureRefusal::from_denial(denial, Some(class)),
                    subject,
                    now,
                );
                CaptureArtifact::quarantined(manifest, risk)
            }
            None => CaptureArtifact::releasable(manifest, self.bytes),
        }
    }

    /// The first recorded chunk whose instant does not re-bind.
    fn first_unbound_chunk(
        &self,
        ledger: &ConsentLedger,
    ) -> Option<(ViolationRisk, CaptureDenial)> {
        for chunk in &self.chunks {
            if let Err(denial) = bind_permission(ledger, self.token.request(), chunk.started_at()) {
                return Some((
                    ViolationRisk::raised(
                        chunk.seq(),
                        chunk.started_at(),
                        denial.reason(),
                        denial.status(),
                    ),
                    denial,
                ));
            }
        }
        None
    }

    fn subject(&self) -> AuditSubject {
        AuditSubject {
            offering_id: Some(self.offering_id),
            lecture_id: Some(self.lecture_id),
            digest: Some(*self.token.token_id()),
        }
    }
}

/// The one place a caller asks a sealed capture for its bytes.
///
/// A quarantined artefact has no byte accessor at all, so this function exists
/// for the caller holding the sum type rather than one of its arms: it is the
/// behavioural half of the block, and the row it appends is what
/// `capture_audit_records_every_denial` reads for
/// [`CaptureRefusalReason::ArtifactQuarantined`].
pub fn releasable_bytes<'artifact>(
    artifact: &'artifact CaptureArtifact,
    audit: &mut CaptureAudit,
    now: u64,
) -> Result<&'artifact [u8], CaptureRefusal> {
    match artifact {
        CaptureArtifact::Releasable(releasable) => Ok(releasable.bytes()),
        CaptureArtifact::Quarantined(quarantined) => Err(audit.record_refusal(
            CaptureRefusal::of(CaptureRefusalReason::ArtifactQuarantined, None),
            AuditSubject {
                offering_id: None,
                lecture_id: None,
                digest: Some(*quarantined.manifest().digest()),
            },
            now,
        )),
    }
}
