//! The one decision, and the two paths that both have to run it.
//!
//! # `bind_permission` is where everything is compared
//!
//! Section 3.7 lists five ways a capture fails closed -- `UNKNOWN`,
//! `PROHIBITED`, `EXPIRED`, scope mismatch, and a stale verification -- and
//! adds that the token is bound to `(offering_id, lecture_id, media set,
//! processing set, not_after)`. All of that is one function,
//! [`bind_permission`], and both entry points call it as their first statement.
//!
//! `P2-RF10` is why. `EgressProxy::transmit_without_completion` was a second
//! public path that skipped a check the first one made, and nothing counted the
//! call sites, so deleting the check outright left the whole suite green. The
//! shape that closed it -- one binding function, pinned as whole text, its call
//! sites counted -- is the shape here: `WHOLE_BIND_PERMISSION` fixes the body,
//! `WHOLE_MINT` and `WHOLE_CONTINUE` fix the two callers, and a call-site count
//! of two fixes that a third path cannot appear without editing the count.
//!
//! # Absence is a denial, and the mechanism is `P2-G1`'s
//!
//! [`CaptureRequest`] is seven `Option` fields and [`ResolvedRequest::resolve`]
//! turns each `None` into [`CaptureDenialReason::IncompleteRequest`]. That is
//! `CompleteRequest::resolve` in `academic-policy`, deliberately reused rather
//! than reinvented: a request field that is missing denies, an empty list is
//! not the same as an absent one, and nothing downstream ever sees an `Option`
//! it could treat as permissive.
//!
//! # What the token carries and why
//!
//! The token holds the exact [`CaptureRequest`] it was minted from.
//! [`continue_capture`] binds against that tuple rather than against a narrower
//! one assembled at the call site, so a capture cannot outlive the terms it
//! started under by being re-checked against less than it claimed.

use core::fmt;

use academic_domain::{CapturePermissionId, ContentDigest, LectureSessionId, OfferingId};

use crate::{
    checklist::ChecklistDimension,
    ledger::ConsentLedger,
    permission::{CaptureMedium, CaptureProcessing, Condition, TermKey},
    retention::RetentionTerms,
    status::{CaptureStatus, status_of},
};

/// Why a capture was refused.
///
/// Closed. Section 3.7 names five of these; the rest are the request-side
/// halves of the same five, kept apart so an audit row says which comparison
/// failed rather than only that one did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CaptureDenialReason {
    /// A request field was absent. `P2-G1`'s missing-tuple-field denial.
    IncompleteRequest,
    /// No written authority has answered for this scope.
    PermissionUnknown,
    /// A written authority refused.
    PermissionProhibited,
    /// The grant no longer covers this instant, or was verified before its term.
    PermissionExpired,
    /// The record does not answer for this offering, term, or session.
    ScopeMismatch,
    /// A medium was requested that the grant does not list.
    MediumNotGranted,
    /// A processing step was requested that the grant does not list.
    ProcessingNotGranted,
    /// A processing step leaves the device and the grant does not allow that.
    ExternalProcessingNotGranted,
    /// The requested token lifetime reaches past the grant or the scope, or
    /// is already over at the instant it was asked for.
    LifetimeExceedsGrant,
}

impl CaptureDenialReason {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IncompleteRequest => "INCOMPLETE_REQUEST",
            Self::PermissionUnknown => "PERMISSION_UNKNOWN",
            Self::PermissionProhibited => "PERMISSION_PROHIBITED",
            Self::PermissionExpired => "PERMISSION_EXPIRED",
            Self::ScopeMismatch => "SCOPE_MISMATCH",
            Self::MediumNotGranted => "MEDIUM_NOT_GRANTED",
            Self::ProcessingNotGranted => "PROCESSING_NOT_GRANTED",
            Self::ExternalProcessingNotGranted => "EXTERNAL_PROCESSING_NOT_GRANTED",
            Self::LifetimeExceedsGrant => "LIFETIME_EXCEEDS_GRANT",
        }
    }
}

/// A refusal, with the status that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("capture refused: {reason:?} at status {status:?}")]
pub struct CaptureDenial {
    reason: CaptureDenialReason,
    status: CaptureStatus,
}

impl CaptureDenial {
    /// Which comparison failed.
    #[must_use]
    pub const fn reason(&self) -> CaptureDenialReason {
        self.reason
    }

    /// The section 3.7 status at the moment of the refusal.
    #[must_use]
    pub const fn status(&self) -> CaptureStatus {
        self.status
    }

    /// Whether this refusal is one that should put the scope back in the
    /// recheck queue.
    ///
    /// `UNKNOWN` and `EXPIRED` are the two states a user can clear by
    /// confirming the offering's permission again, which is what section 38.1
    /// asks for every term. `PROHIBITED` is not: an authority said no, and
    /// queueing a recheck for it would be the system asking the user to keep
    /// trying.
    #[must_use]
    pub const fn queues_recheck(&self) -> bool {
        matches!(self.status, CaptureStatus::Unknown | CaptureStatus::Expired)
    }
}

/// What a caller asks for. Every field optional, every absence a denial.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CaptureRequest {
    /// Which offering.
    pub offering_id: Option<OfferingId>,
    /// Which session of it.
    pub lecture_id: Option<LectureSessionId>,
    /// Which term the caller believes that session sits in.
    pub term: Option<TermKey>,
    /// The media the capture will use.
    pub media: Option<Vec<CaptureMedium>>,
    /// The processing the capture will be put through.
    pub processing: Option<Vec<CaptureProcessing>>,
    /// When the request was made.
    pub requested_at: Option<u64>,
    /// When the caller wants the capability to stop.
    pub not_after: Option<u64>,
}

/// The same request with nothing optional left.
struct ResolvedRequest<'a> {
    offering_id: OfferingId,
    lecture_id: LectureSessionId,
    term: TermKey,
    media: &'a [CaptureMedium],
    processing: &'a [CaptureProcessing],
    not_after: u64,
}

impl<'a> ResolvedRequest<'a> {
    /// Turns each absent field into a denial.
    ///
    /// The shape is `CompleteRequest::resolve` in `academic-policy`: a struct
    /// of `Option`s in, a struct of values out, and the only way past it is for
    /// every field to be there. An empty `media` list resolves -- it is a
    /// present, empty set, and [`bind_permission`] refuses it as
    /// `MEDIUM_NOT_GRANTED` rather than as an incomplete request, which keeps
    /// "asked for nothing" distinct from "did not say". An empty `processing`
    /// list is not refused: a capture recorded now and processed later asks for
    /// exactly that.
    fn resolve(request: &'a CaptureRequest) -> Result<Self, CaptureDenialReason> {
        let (Some(offering_id), Some(lecture_id), Some(term)) =
            (request.offering_id, request.lecture_id, request.term)
        else {
            return Err(CaptureDenialReason::IncompleteRequest);
        };
        let (Some(media), Some(processing)) =
            (request.media.as_deref(), request.processing.as_deref())
        else {
            return Err(CaptureDenialReason::IncompleteRequest);
        };
        let (Some(_requested_at), Some(not_after)) = (request.requested_at, request.not_after)
        else {
            return Err(CaptureDenialReason::IncompleteRequest);
        };
        Ok(Self {
            offering_id,
            lecture_id,
            term,
            media,
            processing,
            not_after,
        })
    }
}

/// A permission that has been compared against a request and survived.
///
/// The fields are private and this module is the only place a value is built,
/// so holding one is proof that [`bind_permission`] ran. It owns its data
/// rather than borrowing the ledger, so a caller can append the audit row that
/// section 3.7 requires while still holding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundPermission {
    permission_id: CapturePermissionId,
    permission_seq: u32,
    offering_id: OfferingId,
    lecture_id: LectureSessionId,
    status: CaptureStatus,
    media: Vec<CaptureMedium>,
    processing: Vec<CaptureProcessing>,
    not_after: u64,
    conditions: Vec<Condition>,
    unanswered: Vec<ChecklistDimension>,
    retention: RetentionTerms,
}

impl BoundPermission {
    /// The aggregate row this binding read.
    #[must_use]
    pub const fn permission_id(&self) -> CapturePermissionId {
        self.permission_id
    }

    /// The second half of the section 3.7 key.
    #[must_use]
    pub const fn permission_seq(&self) -> u32 {
        self.permission_seq
    }

    /// The offering this binding was compared against.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// The session this binding was compared against.
    #[must_use]
    pub const fn lecture_id(&self) -> LectureSessionId {
        self.lecture_id
    }

    /// The status that permitted it.
    #[must_use]
    pub const fn status(&self) -> CaptureStatus {
        self.status
    }

    /// The dimensions nobody answered, carried forward to the device layer.
    #[must_use]
    pub fn unanswered(&self) -> &[ChecklistDimension] {
        &self.unanswered
    }

    /// The two retention bounds this capture's output inherits.
    #[must_use]
    pub const fn retention(&self) -> RetentionTerms {
        self.retention
    }
}

/// Compares one request against the ledger and returns the binding or the
/// reason there is none.
///
/// Every comparison section 3.7 asks for is here and nowhere else. The order is
/// the order of the failure the caller most needs to see: a request that is not
/// whole cannot be compared at all; a scope that does not answer is not the
/// same as one that answered `UNKNOWN`; and the media, processing and lifetime
/// comparisons come after the status, so a `PROHIBITED` offering is refused as
/// prohibited rather than as an unlisted medium.
pub fn bind_permission(
    ledger: &ConsentLedger,
    request: &CaptureRequest,
    now: u64,
) -> Result<BoundPermission, CaptureDenial> {
    let deny =
        |reason: CaptureDenialReason, status: CaptureStatus| CaptureDenial { reason, status };
    let resolved = match ResolvedRequest::resolve(request) {
        Ok(resolved) => resolved,
        Err(reason) => return Err(deny(reason, CaptureStatus::Unknown)),
    };
    let Some(record) =
        ledger.permission_for(resolved.offering_id, resolved.term, resolved.lecture_id)
    else {
        return Err(deny(
            CaptureDenialReason::PermissionUnknown,
            CaptureStatus::Unknown,
        ));
    };
    let status = status_of(record, now);
    match status {
        CaptureStatus::Unknown => {
            return Err(deny(CaptureDenialReason::PermissionUnknown, status));
        }
        CaptureStatus::Prohibited => {
            return Err(deny(CaptureDenialReason::PermissionProhibited, status));
        }
        CaptureStatus::Expired => {
            return Err(deny(CaptureDenialReason::PermissionExpired, status));
        }
        CaptureStatus::Permitted | CaptureStatus::PermittedWithConditions => (),
    }
    if !record.scope().contains(now)
        || !record
            .scope()
            .answers(resolved.offering_id, resolved.term, resolved.lecture_id)
    {
        return Err(deny(CaptureDenialReason::ScopeMismatch, status));
    }
    let Some(grant) = record.grant() else {
        return Err(deny(CaptureDenialReason::PermissionUnknown, status));
    };
    if resolved.media.is_empty()
        || resolved
            .media
            .iter()
            .any(|medium| !grant.allowed_media().contains(medium))
    {
        return Err(deny(CaptureDenialReason::MediumNotGranted, status));
    }
    if resolved
        .processing
        .iter()
        .any(|step| !grant.allowed_processing().contains(step))
    {
        return Err(deny(CaptureDenialReason::ProcessingNotGranted, status));
    }
    if !grant.external_processing_allowed()
        && resolved
            .processing
            .iter()
            .any(|step| step.leaves_the_device())
    {
        return Err(deny(
            CaptureDenialReason::ExternalProcessingNotGranted,
            status,
        ));
    }
    if resolved.not_after > grant.not_after()
        || resolved.not_after > record.scope().valid_to()
        || resolved.not_after <= now
    {
        return Err(deny(CaptureDenialReason::LifetimeExceedsGrant, status));
    }
    Ok(BoundPermission {
        permission_id: record.permission_id(),
        permission_seq: record.permission_seq(),
        offering_id: resolved.offering_id,
        lecture_id: resolved.lecture_id,
        status,
        media: resolved.media.to_vec(),
        processing: resolved.processing.to_vec(),
        not_after: resolved.not_after,
        conditions: grant.conditions().to_vec(),
        unanswered: record.checklist().unanswered(),
        retention: *grant.retention(),
    })
}

/// The section 3.7 token. It cannot be constructed outside this module.
pub struct CaptureCapabilityToken {
    token_id: ContentDigest,
    request: CaptureRequest,
    bound: BoundPermission,
}

impl CaptureCapabilityToken {
    /// The request this token was minted from, and the tuple it is re-checked
    /// against.
    #[must_use]
    pub const fn request(&self) -> &CaptureRequest {
        &self.request
    }

    /// The permission behind it.
    #[must_use]
    pub const fn bound(&self) -> &BoundPermission {
        &self.bound
    }

    /// The media the device may open.
    #[must_use]
    pub fn media(&self) -> &[CaptureMedium] {
        &self.bound.media
    }

    /// The processing the capture may be put through.
    #[must_use]
    pub fn processing(&self) -> &[CaptureProcessing] {
        &self.bound.processing
    }

    /// The conditions the written authority attached.
    #[must_use]
    pub fn conditions(&self) -> &[Condition] {
        &self.bound.conditions
    }

    /// When the token stops.
    #[must_use]
    pub const fn not_after(&self) -> u64 {
        self.bound.not_after
    }

    /// The opaque identifier an audit row names.
    #[must_use]
    pub const fn token_id(&self) -> &ContentDigest {
        &self.token_id
    }
}

impl fmt::Debug for CaptureCapabilityToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CaptureCapabilityToken(<opaque>)")
    }
}

/// Mints a capability, or records why it was refused.
///
/// The ledger is taken by mutable reference because section 3.7 requires an
/// audit row on the refusing paths as well as on the allowing one, and because
/// an `UNKNOWN` or `EXPIRED` refusal is what puts the scope back in the recheck
/// queue. Both happen here rather than in the caller, so no path exists that
/// refuses without leaving a record.
pub fn mint_capture_capability(
    ledger: &mut ConsentLedger,
    request: &CaptureRequest,
    now: u64,
) -> Result<CaptureCapabilityToken, CaptureDenial> {
    let bound = match bind_permission(ledger, request, now) {
        Ok(bound) => bound,
        Err(denial) => return Err(ledger.record_capture_denial(request, denial, now)),
    };
    let token_id = token_id(&bound, now);
    ledger.record_capture_mint(&bound, &token_id, now);
    Ok(CaptureCapabilityToken {
        token_id,
        request: request.clone(),
        bound,
    })
}

/// Re-runs the whole binding for a capture that is already running.
///
/// A token minted at one instant says nothing about a later one: the grant can
/// expire, the scope interval can end, and a superseding record can arrive. So
/// this is not a comparison against the token's own `not_after` -- that alone
/// would be the second path `P2-RF10` found -- but the same
/// [`bind_permission`] call against the same tuple, plus the token's own bound.
pub fn continue_capture(
    ledger: &mut ConsentLedger,
    token: &CaptureCapabilityToken,
    now: u64,
) -> Result<(), CaptureDenial> {
    let bound = match bind_permission(ledger, token.request(), now) {
        Ok(bound) => bound,
        Err(denial) => return Err(ledger.record_capture_denial(token.request(), denial, now)),
    };
    if now >= token.not_after() || bound.permission_id() != token.bound().permission_id() {
        let denial = CaptureDenial {
            reason: CaptureDenialReason::LifetimeExceedsGrant,
            status: bound.status(),
        };
        return Err(ledger.record_capture_denial(token.request(), denial, now));
    }
    Ok(())
}

/// The opaque token identifier, over everything the token is bound to.
///
/// The offering and the session come from the binding rather than from the
/// request, because the binding is the pair that was actually compared: a
/// request field the resolver rejected never reaches this function, and reading
/// it back out of the `Option` here would put an `unwrap` on the path.
fn token_id(bound: &BoundPermission, now: u64) -> ContentDigest {
    let mut material = b"academic-capture-capability-v1\0".to_vec();
    material.extend_from_slice(bound.permission_id.as_bytes());
    material.extend_from_slice(&bound.permission_seq.to_be_bytes());
    material.extend_from_slice(bound.offering_id.as_bytes());
    material.extend_from_slice(bound.lecture_id.as_bytes());
    for medium in &bound.media {
        material.extend_from_slice(medium.as_str().as_bytes());
    }
    for step in &bound.processing {
        material.extend_from_slice(step.as_str().as_bytes());
    }
    material.extend_from_slice(&bound.not_after.to_be_bytes());
    material.extend_from_slice(&now.to_be_bytes());
    ContentDigest::sha256(&material)
}
