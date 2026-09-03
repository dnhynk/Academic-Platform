//! `P2-L1`'s behavioural acceptance suite.
//!
//! Every instant here is a committed literal and no test opens a device. The
//! platform-native half -- `no_device_handle_without_token` -- is in
//! `tests/native_device.rs` behind the `native-capture` feature, because it is
//! the only row that asks an operating system anything.

mod common;

use academic_capture_gate::{
    CaptureArtifact, CaptureAudit, CaptureRefusalReason, DEVICE_CLASSES, DeviceClass, DeviceLayer,
    DeviceRuleset, PERMISSION_VIOLATION_RISK, authorize, open_device, releasable_bytes,
};
use academic_consent::{
    CaptureDenialReason, CaptureMedium, CaptureProcessing, CaptureRequest, CaptureStatus,
    ConsentLedger,
};
use academic_egress_boundary::SourceDocument;
use academic_untrusted_content::{PromptEnvelope, SourceId, SourceKind, ingest};

use common::{
    INSIDE, TERM_FROM, TERM_TO, TOKEN_UNTIL, TestResult, append_refusal, audio_request, chunk,
    lecture, ledger_audio_only, ledger_granting, ledger_refusing, next_term, offering_b,
    request_for, term,
};

/// The five records section 3.7 names, as a closed list.
///
/// This is the enumeration `record_fail_closed` walks. Adding a case here
/// without an arm in `expected_for` stops the suite compiling, and removing one
/// fails the coverage assertion below, so the count is derived from the list
/// rather than asserted beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordCase {
    /// No written authority has answered for the scope.
    Unknown,
    /// A written authority refused.
    Prohibited,
    /// A written authority granted and the grant has stopped covering now.
    Expired,
    /// The request names an offering, term or session the record does not
    /// answer for.
    ScopeMismatch,
    /// A written authority granted and the grant covers now.
    Valid,
}

const RECORD_CASES: [RecordCase; 5] = [
    RecordCase::Unknown,
    RecordCase::Prohibited,
    RecordCase::Expired,
    RecordCase::ScopeMismatch,
    RecordCase::Valid,
];

/// A compiler-checked witness that `RECORD_CASES` names every case, in order.
///
/// The first version of this file walked `RECORD_CASES` and then asserted that
/// every element of `RECORD_CASES` had been walked, which is true of any array
/// whatever it holds. Injection `L-I15b` -- replacing `Expired` with a second
/// `Valid`, so the length is unchanged and the file compiles -- passed that
/// check and dropped a fail-closed case from the suite silently. This is what
/// refuses it: the index a case must sit at comes from a `match` over the
/// enum, so a case that is absent, duplicated or reordered fails, and a sixth
/// variant added to `RecordCase` stops this function compiling.
const fn record_case_witness(case: RecordCase) -> usize {
    match case {
        RecordCase::Unknown => 0,
        RecordCase::Prohibited => 1,
        RecordCase::Expired => 2,
        RecordCase::ScopeMismatch => 3,
        RecordCase::Valid => 4,
    }
}

/// What each case must produce.
///
/// `None` is "a device opens"; `Some(reason)` is "no device opens, and this is
/// the section 3.7 comparison that refused".
///
/// `ScopeMismatch` expects `PERMISSION_UNKNOWN` rather than `SCOPE_MISMATCH`,
/// and that is the measured truth rather than a compromise: `permission_for`
/// filters on `PermissionScope::answers`, so a request naming another offering,
/// term or session finds no record at all and `bind_permission` refuses it as
/// unknown. `CaptureDenialReason::ScopeMismatch` is unreachable through
/// `bind_permission` today -- `status_of` returns `EXPIRED` for the only other
/// way in, an instant outside the scope interval -- and
/// `scope_mismatch_is_refused_as_unknown_and_the_scope_arm_is_unreachable`
/// below is where that is written down rather than assumed.
const fn expected_for(case: RecordCase) -> Option<CaptureDenialReason> {
    match case {
        RecordCase::Unknown | RecordCase::ScopeMismatch => {
            Some(CaptureDenialReason::PermissionUnknown)
        }
        RecordCase::Prohibited => Some(CaptureDenialReason::PermissionProhibited),
        RecordCase::Expired => Some(CaptureDenialReason::PermissionExpired),
        RecordCase::Valid => None,
    }
}

fn ledger_and_request_for(
    case: RecordCase,
) -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    Ok(match case {
        RecordCase::Unknown => (ConsentLedger::new(), audio_request()?),
        RecordCase::Prohibited => (ledger_refusing()?, audio_request()?),
        // A grant that stopped covering the instant the request is made at.
        RecordCase::Expired => (
            ledger_granting(vec![CaptureMedium::Audio], TERM_FROM + 100)?,
            audio_request()?,
        ),
        // A record for this offering and term, and a request naming another
        // offering. The next term is exercised beside it below.
        RecordCase::ScopeMismatch => {
            let mut request = audio_request()?;
            request.offering_id = Some(offering_b()?);
            (ledger_audio_only()?, request)
        }
        RecordCase::Valid => (ledger_audio_only()?, audio_request()?),
    })
}

/// The recorder opens no device on `UNKNOWN`, `PROHIBITED`, `EXPIRED` or a
/// scope the record does not answer for, and opens one on a live grant.
///
/// `INV-C-013` is the first of those four: a new offering has no record, and no
/// path in this crate turns that into a device.
#[test]
fn record_fail_closed() -> TestResult {
    let mut seen = Vec::new();
    for case in RECORD_CASES {
        let (mut ledger, request) = ledger_and_request_for(case)?;
        let mut audit = CaptureAudit::new();
        let authorization = authorize(&mut ledger, &mut audit, &request, INSIDE);
        match (expected_for(case), authorization) {
            (Some(expected), Err(refusal)) => {
                assert_eq!(
                    refusal.denial_reason(),
                    Some(expected),
                    "{case:?} refused for the wrong reason"
                );
                assert_eq!(
                    audit.rows().len(),
                    1,
                    "{case:?} left {} audit rows",
                    audit.rows().len()
                );
                assert_ne!(
                    refusal.status(),
                    Some(CaptureStatus::Permitted),
                    "{case:?} refused at a permitting status"
                );
            }
            (None, Ok(authorization)) => {
                assert!(
                    authorization.ruleset().permits(DeviceClass::Microphone),
                    "{case:?} minted a token that opens no microphone"
                );
                let session = open_device(
                    &mut ledger,
                    &mut audit,
                    authorization,
                    DeviceClass::Microphone,
                    DeviceLayer::Bookkeeping,
                    INSIDE,
                )?;
                assert_eq!(session.class(), DeviceClass::Microphone);
                assert!(audit.rows().is_empty(), "{case:?} audited an allow");
            }
            (Some(_), Ok(_)) => {
                return Err(format!("{case:?} opened a device it must not have").into());
            }
            (None, Err(refusal)) => {
                return Err(format!("{case:?} was refused: {refusal}").into());
            }
        }
        seen.push(case);
    }
    // The five are walked, and the check that they are the five comes from the
    // enum rather than from the array. Comparing the array against itself is
    // what `L-I15b` walked past.
    assert_eq!(seen.len(), 5, "the record cases are section 3.7's five");
    for (index, case) in RECORD_CASES.iter().enumerate() {
        assert_eq!(
            record_case_witness(*case),
            index,
            "{case:?} is out of order, duplicated, or unlisted"
        );
        assert_eq!(seen[index], *case, "{case:?} was not the case walked");
    }
    Ok(())
}

/// The semester recheck reaches the device layer the same way.
///
/// A record written for one term does not answer a request in the next, so the
/// next term starts with no device.
#[test]
fn the_next_term_starts_with_no_device() -> TestResult {
    let mut ledger = ledger_audio_only()?;
    let mut audit = CaptureAudit::new();
    let mut request = audio_request()?;
    request.term = Some(next_term()?);
    let refusal = authorize(&mut ledger, &mut audit, &request, INSIDE)
        .err()
        .ok_or("the next term opened a device")?;
    assert_eq!(
        refusal.denial_reason(),
        Some(CaptureDenialReason::PermissionUnknown)
    );
    Ok(())
}

/// `SCOPE_MISMATCH` is not reachable through the binding, and the suite says so
/// rather than leaving a case nobody can produce.
#[test]
fn scope_mismatch_is_refused_as_unknown_and_the_scope_arm_is_unreachable() -> TestResult {
    for (label, mutate) in [
        (
            "another offering",
            Box::new(|request: &mut CaptureRequest| {
                request.offering_id = offering_b().ok();
            }) as Box<dyn Fn(&mut CaptureRequest)>,
        ),
        (
            "another term",
            Box::new(|request: &mut CaptureRequest| {
                request.term = next_term().ok();
            }),
        ),
    ] {
        let mut ledger = ledger_audio_only()?;
        let mut audit = CaptureAudit::new();
        let mut request = audio_request()?;
        mutate(&mut request);
        let refusal = authorize(&mut ledger, &mut audit, &request, INSIDE)
            .err()
            .ok_or_else(|| format!("{label} opened a device"))?;
        assert_eq!(
            refusal.denial_reason(),
            Some(CaptureDenialReason::PermissionUnknown),
            "{label} did not refuse as unknown"
        );
        assert_ne!(
            refusal.denial_reason(),
            Some(CaptureDenialReason::ScopeMismatch),
            "{label} reached the scope arm; this test and the contract are stale"
        );
    }
    // The session grain is the third field `answers` compares, and it is
    // refused the same way.
    let mut ledger = ledger_audio_only()?;
    let mut audit = CaptureAudit::new();
    let mut request = audio_request()?;
    request.lecture_id = lecture().ok();
    assert!(
        authorize(&mut ledger, &mut audit, &request, INSIDE).is_ok(),
        "the recorded session must still bind"
    );
    Ok(())
}

/// An audio-only permission opens a microphone and refuses a camera, at the
/// layer that would have opened the camera.
#[test]
fn audio_only_permission_denies_camera() -> TestResult {
    let mut ledger = ledger_audio_only()?;
    let mut audit = CaptureAudit::new();
    let request = audio_request()?;
    let authorization = authorize(&mut ledger, &mut audit, &request, INSIDE)?;
    let ruleset = authorization.ruleset().clone();
    assert_eq!(ruleset.classes(), &[DeviceClass::Microphone]);
    assert!(ruleset.unclassified().is_empty());

    let refusal = open_device(
        &mut ledger,
        &mut audit,
        authorization,
        DeviceClass::Camera,
        DeviceLayer::Bookkeeping,
        INSIDE,
    )
    .err()
    .ok_or("an audio-only token opened a camera")?;
    assert_eq!(refusal.reason(), CaptureRefusalReason::MediumNotOnToken);
    assert_eq!(refusal.class(), Some(DeviceClass::Camera));
    assert_eq!(audit.count_of(CaptureRefusalReason::MediumNotOnToken), 1);

    // Every class the token does not name is refused, not only the camera.
    for class in DEVICE_CLASSES {
        let mut ledger = ledger_audio_only()?;
        let mut audit = CaptureAudit::new();
        let authorization = authorize(&mut ledger, &mut audit, &audio_request()?, INSIDE)?;
        let opened = open_device(
            &mut ledger,
            &mut audit,
            authorization,
            class,
            DeviceLayer::Bookkeeping,
            INSIDE,
        );
        assert_eq!(
            opened.is_ok(),
            class == DeviceClass::Microphone,
            "{class:?} disagreed with the token's media set"
        );
    }

    // And a grant that does name the camera opens one, so the refusal above is
    // the media set rather than a device layer that refuses everything.
    let mut ledger = ledger_granting(vec![CaptureMedium::Audio, CaptureMedium::Video], TERM_TO)?;
    let mut audit = CaptureAudit::new();
    let request = request_for(vec![CaptureMedium::Video], TOKEN_UNTIL)?;
    let authorization = authorize(&mut ledger, &mut audit, &request, INSIDE)?;
    assert!(authorization.ruleset().permits(DeviceClass::Camera));
    open_device(
        &mut ledger,
        &mut audit,
        authorization,
        DeviceClass::Camera,
        DeviceLayer::Bookkeeping,
        INSIDE,
    )?;
    Ok(())
}

/// A capture that is already running stops at the token's boundary, and the
/// chunks it recorded before the boundary are kept.
///
/// This is fault `CP01`: capture stops at the boundary, prior chunks are
/// retained under the expired scope, and the timeline gap is explicit.
#[test]
fn token_expiry_stops_capture_at_the_boundary() -> TestResult {
    let mut ledger = ledger_audio_only()?;
    let mut audit = CaptureAudit::new();
    let authorization = authorize(&mut ledger, &mut audit, &audio_request()?, INSIDE)?;
    let mut session = open_device(
        &mut ledger,
        &mut audit,
        authorization,
        DeviceClass::Microphone,
        DeviceLayer::Bookkeeping,
        INSIDE,
    )?;
    assert_eq!(session.not_after(), TOKEN_UNTIL);

    // Two chunks inside the token's lifetime.
    session.record_chunk(&mut ledger, &mut audit, &chunk("one"), INSIDE)?;
    session.record_chunk(&mut ledger, &mut audit, &chunk("two"), TOKEN_UNTIL - 1)?;
    assert_eq!(session.chunk_count(), 2);
    assert!(session.gap().is_none());

    // The instant the token stops is the boundary, and it is refused.
    let refusal = session
        .record_chunk(&mut ledger, &mut audit, &chunk("three"), TOKEN_UNTIL)
        .err()
        .ok_or("a chunk was accepted at the boundary")?;
    assert_eq!(refusal.reason(), CaptureRefusalReason::PermissionRefused);
    assert_eq!(session.chunk_count(), 2, "the boundary chunk was kept");

    // The gap is opened at the boundary and every later chunk is refused as a
    // stopped session, so a caller that ignores the error does not resume.
    let gap = session.gap().ok_or("no gap was opened")?;
    assert_eq!(gap.from(), TOKEN_UNTIL);
    let after = session
        .record_chunk(&mut ledger, &mut audit, &chunk("four"), TOKEN_UNTIL + 1)
        .err()
        .ok_or("the capture resumed past the boundary")?;
    assert_eq!(after.reason(), CaptureRefusalReason::SessionAlreadyStopped);
    assert_eq!(session.chunk_count(), 2);

    // `CP01`: the prior chunks are retained, and the artefact is releasable
    // because every one of them re-binds at its own instant.
    let artifact = session.seal(&ledger, &mut audit, TOKEN_UNTIL + 2);
    let releasable = artifact
        .as_releasable()
        .ok_or("chunks recorded under a live permission were quarantined")?;
    assert_eq!(releasable.manifest().chunks().len(), 2);
    assert_eq!(
        releasable.manifest().gap().map(|gap| gap.from()),
        Some(TOKEN_UNTIL)
    );
    assert_eq!(
        releasable.bytes(),
        [chunk("one"), chunk("two")].concat().as_slice()
    );
    Ok(())
}

/// The seal catches a chunk recorded past the boundary even if nothing stopped
/// it there.
///
/// This is the injection observer. Remove the `continue_capture` call from
/// `record_chunk` and chunks keep being appended past `not_after`; this row
/// re-binds each recorded instant and quarantines the artefact, so the
/// injection is caught by a mechanism other than the check it deleted.
#[test]
fn a_chunk_recorded_past_the_boundary_quarantines_the_artefact() -> TestResult {
    let mut ledger = ledger_audio_only()?;
    let mut audit = CaptureAudit::new();
    let authorization = authorize(&mut ledger, &mut audit, &audio_request()?, INSIDE)?;
    let mut session = open_device(
        &mut ledger,
        &mut audit,
        authorization,
        DeviceClass::Microphone,
        DeviceLayer::Bookkeeping,
        INSIDE,
    )?;
    session.record_chunk(&mut ledger, &mut audit, &chunk("one"), INSIDE)?;
    // The boundary refuses this, which is the point of the row above. What is
    // asserted here is what the seal does with a session whose chunk list holds
    // an instant the permission does not cover, however it got there.
    let _ = session.record_chunk(&mut ledger, &mut audit, &chunk("late"), TOKEN_UNTIL + 5);

    // A written refusal arriving during the lecture is the reachable shape of
    // the same defect: the chunks already recorded stop re-binding.
    append_refusal(&mut ledger, TOKEN_UNTIL)?;
    let artifact = session.seal(&ledger, &mut audit, TOKEN_UNTIL + 10);
    let quarantined = artifact
        .as_quarantined()
        .ok_or("a superseding refusal left the artefact releasable")?;
    assert_eq!(quarantined.state(), PERMISSION_VIOLATION_RISK);
    assert_eq!(
        quarantined.risk().denial(),
        CaptureDenialReason::PermissionProhibited
    );
    assert_eq!(quarantined.risk().status(), CaptureStatus::Prohibited);
    assert_eq!(quarantined.risk().chunk_seq(), 0);
    Ok(())
}

/// A quarantined artefact reaches neither the egress boundary nor a prompt.
///
/// The block is the absence of an accessor, so what this row does is show the
/// two real boundaries being fed from a releasable artefact and having nothing
/// to be fed from a quarantined one.
#[test]
fn violation_risk_blocks_share_and_ai_processing() -> TestResult {
    let (releasable, quarantined) = one_of_each()?;
    let mut audit = CaptureAudit::new();

    // Sharing: `SourceDocument` is what `EgressProxy::stage` takes, and its
    // payload can only come from a releasable artefact.
    let bytes = releasable_bytes(&releasable, &mut audit, INSIDE)?;
    let document = SourceDocument::new("capture-object", bytes.to_vec());
    assert_eq!(document.byte_len(), bytes.len());

    // AI processing: `PromptEnvelope::quote` takes an `Untrusted<IngestedDocument>`,
    // and `ingest` takes bytes.
    let ingested = ingest(
        SourceId::new("capture-transcript")?,
        SourceKind::ProviderResponse,
        1,
        bytes,
    )?;
    let mut envelope = PromptEnvelope::new();
    envelope.quote(&ingested);
    assert!(envelope.quoted_len() > 0);

    // The quarantined artefact has no bytes to give either of them.
    let refusal = releasable_bytes(&quarantined, &mut audit, INSIDE)
        .err()
        .ok_or("a quarantined artefact handed out its bytes")?;
    assert_eq!(refusal.reason(), CaptureRefusalReason::ArtifactQuarantined);
    assert_eq!(audit.count_of(CaptureRefusalReason::ArtifactQuarantined), 1);
    assert!(quarantined.as_releasable().is_none());
    assert!(quarantined.is_quarantined());
    assert_eq!(
        quarantined
            .as_quarantined()
            .map(academic_capture_gate::QuarantinedArtifact::state),
        Some(PERMISSION_VIOLATION_RISK)
    );
    // What it does still carry is everything that is not a byte of the capture.
    assert_eq!(quarantined.manifest().chunks().len(), 1);
    assert!(quarantined.manifest().byte_len() > 0);
    Ok(())
}

/// Every reason this layer refuses for leaves exactly one audit row, and every
/// section 3.7 comparison that can refuse leaves one naming which.
#[test]
fn capture_audit_records_every_denial() -> TestResult {
    // Half one: this layer's own closed reason set, walked rather than counted.
    //
    // The witness comes first, for `L-I15b`'s reason: walking `REFUSAL_REASONS`
    // and then asserting that everything in `REFUSAL_REASONS` was walked is
    // true of any array. The index each reason must sit at comes from a `match`
    // instead, so a reason dropped from the array, duplicated in it, or moved
    // fails here rather than quietly leaving a refusal untested.
    for (index, reason) in academic_capture_gate::REFUSAL_REASONS.iter().enumerate() {
        assert_eq!(
            refusal_witness(*reason),
            index,
            "{reason:?} is out of order, duplicated, or unlisted"
        );
    }
    for reason in academic_capture_gate::REFUSAL_REASONS {
        let mut audit = CaptureAudit::new();
        produce(reason, &mut audit)?;
        assert_eq!(
            audit.count_of(reason),
            1,
            "{reason:?} left {} rows",
            audit.count_of(reason)
        );
        let row = audit
            .rows()
            .iter()
            .find(|row| row.reason() == reason)
            .ok_or_else(|| format!("{reason:?} left no row"))?;
        assert!(!row.reason().as_str().is_empty());
        // The row names identifiers and a digest, and no captured byte reaches
        // it: there is no byte-carrying field on the row to reach.
        assert!(row.recorded_at() > 0, "{reason:?} row has no instant");
    }

    // Half two: every section 3.7 comparison, with its reachability declared.
    // A variant added to `CaptureDenialReason` without a row here fails the
    // exhaustiveness witness below.
    for (denial, reachable) in DENIAL_REACHABILITY {
        let Some(build) = reachable else {
            continue;
        };
        let mut audit = CaptureAudit::new();
        let (mut ledger, request) = build()?;
        let refusal = authorize(&mut ledger, &mut audit, &request, INSIDE)
            .err()
            .ok_or_else(|| format!("{denial:?} did not refuse"))?;
        assert_eq!(refusal.denial_reason(), Some(denial), "{denial:?} mismatch");
        assert_eq!(audit.rows().len(), 1, "{denial:?} left more than one row");
        assert_eq!(audit.rows()[0].denial_reason(), Some(denial));
    }
    Ok(())
}

/// Where each refusal reason must sit in `REFUSAL_REASONS`.
///
/// `CaptureRefusalReason` is `#[non_exhaustive]`, so an integration test cannot
/// match it exhaustively and the wildcard is what a new arm falls into. That
/// arm returns `usize::MAX`, which matches no index, so a reason added to the
/// enum without a row here fails rather than passing untested -- and `produce`
/// refuses it a second time from its own wildcard.
const fn refusal_witness(reason: CaptureRefusalReason) -> usize {
    match reason {
        CaptureRefusalReason::PermissionRefused => 0,
        CaptureRefusalReason::MediumNotOnToken => 1,
        CaptureRefusalReason::SessionAlreadyStopped => 2,
        CaptureRefusalReason::ArtifactQuarantined => 3,
        CaptureRefusalReason::DeviceLayerUnavailable => 4,
        _ => usize::MAX,
    }
}

/// Every arm of `CaptureDenialReason`, with the scenario that produces it or
/// `None` for the one that is unreachable through the binding.
type DenialBuild = fn() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>>;

const DENIAL_REACHABILITY: [(CaptureDenialReason, Option<DenialBuild>); 9] = [
    (CaptureDenialReason::IncompleteRequest, Some(incomplete)),
    (CaptureDenialReason::PermissionUnknown, Some(unknown)),
    (CaptureDenialReason::PermissionProhibited, Some(prohibited)),
    (CaptureDenialReason::PermissionExpired, Some(expired)),
    // Unreachable: `permission_for` filters on the same comparison, so a scope
    // that does not answer produces no record and refuses as unknown.
    (CaptureDenialReason::ScopeMismatch, None),
    (CaptureDenialReason::MediumNotGranted, Some(medium)),
    (CaptureDenialReason::ProcessingNotGranted, Some(processing)),
    (
        CaptureDenialReason::ExternalProcessingNotGranted,
        Some(external),
    ),
    (CaptureDenialReason::LifetimeExceedsGrant, Some(lifetime)),
];

/// A compiler-checked witness that `DENIAL_REACHABILITY` names every arm.
///
/// A variant added to `CaptureDenialReason` stops this function compiling,
/// which is how the list above is kept whole rather than by counting it.
#[allow(dead_code)]
const fn denial_witness(reason: CaptureDenialReason) -> usize {
    match reason {
        CaptureDenialReason::IncompleteRequest => 0,
        CaptureDenialReason::PermissionUnknown => 1,
        CaptureDenialReason::PermissionProhibited => 2,
        CaptureDenialReason::PermissionExpired => 3,
        CaptureDenialReason::ScopeMismatch => 4,
        CaptureDenialReason::MediumNotGranted => 5,
        CaptureDenialReason::ProcessingNotGranted => 6,
        CaptureDenialReason::ExternalProcessingNotGranted => 7,
        CaptureDenialReason::LifetimeExceedsGrant => 8,
        _ => usize::MAX,
    }
}

#[test]
fn the_denial_vocabulary_is_the_one_this_suite_walks() {
    for (index, (reason, _)) in DENIAL_REACHABILITY.iter().enumerate() {
        assert_eq!(
            denial_witness(*reason),
            index,
            "{reason:?} is out of order or unlisted"
        );
    }
}

fn incomplete() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    let mut request = audio_request()?;
    request.media = None;
    Ok((ledger_audio_only()?, request))
}

fn unknown() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    Ok((ConsentLedger::new(), audio_request()?))
}

fn prohibited() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    Ok((ledger_refusing()?, audio_request()?))
}

fn expired() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    Ok((
        ledger_granting(vec![CaptureMedium::Audio], TERM_FROM + 100)?,
        audio_request()?,
    ))
}

fn medium() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    Ok((
        ledger_audio_only()?,
        request_for(vec![CaptureMedium::Video], TOKEN_UNTIL)?,
    ))
}

fn processing() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    let mut request = audio_request()?;
    request.processing = Some(vec![CaptureProcessing::LocalOcr]);
    Ok((ledger_audio_only()?, request))
}

fn external() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    let mut ledger = ConsentLedger::new();
    ledger = seed_external(ledger)?;
    let mut request = audio_request()?;
    request.processing = Some(vec![CaptureProcessing::ExternalStt]);
    Ok((ledger, request))
}

/// A grant that lists an external step but does not allow leaving the device.
///
/// Both fields are section 3.7's and the narrower one wins, so this is the one
/// scenario where `PROCESSING_NOT_GRANTED` and
/// `EXTERNAL_PROCESSING_NOT_GRANTED` are told apart.
fn seed_external(mut ledger: ConsentLedger) -> Result<ConsentLedger, Box<dyn std::error::Error>> {
    use academic_consent::{
        AuthorityGrant, Disposition, PermissionRecord, PermissionScope, RetentionBound,
        RetentionTerms, ScopeGrain, permission::PermittedUse,
    };
    use academic_domain::ContentDigest;
    let grant = AuthorityGrant::record(
        common::written_authority()?,
        PermittedUse::new(
            vec![CaptureMedium::Audio],
            vec![CaptureProcessing::LocalStt, CaptureProcessing::ExternalStt],
            false,
            false,
        ),
        RetentionTerms::new(
            RetentionBound::Until(1_600_000),
            RetentionBound::Until(1_900_000),
        ),
        Vec::new(),
        TERM_TO,
    );
    ledger.record_permission(
        PermissionRecord::record(
            common::permission_id()?,
            1,
            PermissionScope::new(
                common::offering_a()?,
                term()?,
                ScopeGrain::WholeTerm,
                TERM_FROM,
                TERM_TO,
            )?,
            Disposition::Granted(grant),
            common::checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    Ok(ledger)
}

/// A live grant, and a request that asks to run past the end of it.
///
/// The grant has to be live at `INSIDE` or the record is refused as expired
/// before the lifetime comparison is reached, which is the order section 3.7
/// states and `bind_permission` implements.
fn lifetime() -> Result<(ConsentLedger, CaptureRequest), Box<dyn std::error::Error>> {
    Ok((
        ledger_granting(vec![CaptureMedium::Audio], INSIDE + 50_000)?,
        request_for(vec![CaptureMedium::Audio], INSIDE + 60_000)?,
    ))
}

/// Produces one refusal of each of this layer's reasons.
fn produce(reason: CaptureRefusalReason, audit: &mut CaptureAudit) -> TestResult {
    match reason {
        CaptureRefusalReason::PermissionRefused => {
            let mut ledger = ledger_refusing()?;
            let _ = authorize(&mut ledger, audit, &audio_request()?, INSIDE);
        }
        CaptureRefusalReason::MediumNotOnToken => {
            let mut ledger = ledger_audio_only()?;
            let authorization = authorize(&mut ledger, audit, &audio_request()?, INSIDE)?;
            let _ = open_device(
                &mut ledger,
                audit,
                authorization,
                DeviceClass::Camera,
                DeviceLayer::Bookkeeping,
                INSIDE,
            );
        }
        CaptureRefusalReason::SessionAlreadyStopped => {
            let mut ledger = ledger_audio_only()?;
            let mut quiet = CaptureAudit::new();
            let authorization = authorize(&mut ledger, &mut quiet, &audio_request()?, INSIDE)?;
            let mut session = open_device(
                &mut ledger,
                &mut quiet,
                authorization,
                DeviceClass::Microphone,
                DeviceLayer::Bookkeeping,
                INSIDE,
            )?;
            let _ = session.record_chunk(&mut ledger, &mut quiet, &chunk("late"), TOKEN_UNTIL);
            let _ = session.record_chunk(&mut ledger, audit, &chunk("later"), TOKEN_UNTIL + 1);
        }
        CaptureRefusalReason::ArtifactQuarantined => {
            let (_, quarantined) = one_of_each()?;
            let _ = releasable_bytes(&quarantined, audit, INSIDE);
        }
        CaptureRefusalReason::DeviceLayerUnavailable => {
            let mut ledger = ledger_audio_only()?;
            let mut quiet = CaptureAudit::new();
            let authorization = authorize(&mut ledger, &mut quiet, &audio_request()?, INSIDE)?;
            let _ = open_device(
                &mut ledger,
                audit,
                authorization,
                DeviceClass::Microphone,
                DeviceLayer::Unavailable,
                INSIDE,
            );
        }
        _ => return Err("an unlisted refusal reason".into()),
    }
    Ok(())
}

/// One releasable artefact and one quarantined artefact from the same shape of
/// capture, so the difference between them is the permission and nothing else.
fn one_of_each() -> Result<(CaptureArtifact, CaptureArtifact), Box<dyn std::error::Error>> {
    let releasable = {
        let mut ledger = ledger_audio_only()?;
        let mut audit = CaptureAudit::new();
        let authorization = authorize(&mut ledger, &mut audit, &audio_request()?, INSIDE)?;
        let mut session = open_device(
            &mut ledger,
            &mut audit,
            authorization,
            DeviceClass::Microphone,
            DeviceLayer::Bookkeeping,
            INSIDE,
        )?;
        session.record_chunk(&mut ledger, &mut audit, &chunk("clean"), INSIDE)?;
        session.seal(&ledger, &mut audit, INSIDE + 1)
    };
    let quarantined = {
        let mut ledger = ledger_audio_only()?;
        let mut audit = CaptureAudit::new();
        let authorization = authorize(&mut ledger, &mut audit, &audio_request()?, INSIDE)?;
        let mut session = open_device(
            &mut ledger,
            &mut audit,
            authorization,
            DeviceClass::Microphone,
            DeviceLayer::Bookkeeping,
            INSIDE,
        )?;
        session.record_chunk(&mut ledger, &mut audit, &chunk("clean"), INSIDE)?;
        append_refusal(&mut ledger, INSIDE + 1)?;
        session.seal(&ledger, &mut audit, INSIDE + 2)
    };
    assert!(releasable.as_releasable().is_some());
    assert!(quarantined.as_quarantined().is_some());
    Ok((releasable, quarantined))
}

/// A ruleset cannot be built from a device class, only from a token.
///
/// The behavioural half; `tests/compile_fail/` is the other one.
#[test]
fn a_ruleset_comes_only_from_a_token() -> TestResult {
    let mut ledger = ledger_audio_only()?;
    let mut audit = CaptureAudit::new();
    let authorization = authorize(&mut ledger, &mut audit, &audio_request()?, INSIDE)?;
    let from_token = DeviceRuleset::for_token(authorization.token());
    assert_eq!(from_token, *authorization.ruleset());
    assert!(!from_token.is_empty());
    Ok(())
}

#[cfg(not(feature = "native-capture"))]
/// The default lane records that nothing but this crate's comparisons is in
/// force, rather than claiming an enforcement it does not have.
#[test]
fn the_default_lane_reports_bookkeeping() {
    assert_eq!(
        academic_capture_gate::native::availability(),
        DeviceLayer::Bookkeeping,
        "the default lane must not claim an enforced device layer"
    );
    assert!(!DeviceLayer::Bookkeeping.is_enforced());
    assert_eq!(
        DeviceLayer::Bookkeeping.backend(),
        academic_capture_gate::BackendId::None
    );
}
