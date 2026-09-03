//! The ten `P2-G6` acceptance rows.
//!
//! Each `#[test]` below carries one of the names the execution plan's section 5
//! `P2-G6` entry lists, and nothing else is named that way in this crate.

mod common;

use academic_consent::{
    AttestationKind, AuthorityGrant, CaptureDenialReason, CaptureMedium, CaptureProcessing,
    CaptureStatus, Checklist, ChecklistDimension, ChecklistEntry, Condition, ConsentEventKind,
    ConsentLedger, DERIVATIVE_CLASSES, DerivativeClass, Disposition, ExpiryPlan, ExpiryRefusal,
    GrantAuthority, LegalQuestion, OpenGate, PermissionRecord, PermissionScope, PermittedUse,
    ReferralTarget, RetentionBound, RetentionTerms, ScopeGrain, SubjectInventory, apply_expiry,
    bind_permission, continue_capture, mint_capture_capability, open_external_review,
    preview_expiry,
};
use academic_domain::ContentDigest;

use common::{
    INSIDE, TERM_FROM, TERM_TO, TestResult, artifact, audio_local_use, checklist_missing,
    complete_checklist, grant, lecture, ledger_with_grant, ledger_with_refusal, next_term,
    offering_a, offering_b, oral_attestation, other_lecture, other_permission_id, permission_id,
    personal_use_attestation, split_retention, term, whole_term_scope, written_syllabus,
};

/// A whole, valid request against the fixture grant.
fn request() -> Result<academic_consent::CaptureRequest, Box<dyn std::error::Error>> {
    Ok(academic_consent::CaptureRequest {
        offering_id: Some(offering_a()?),
        lecture_id: Some(lecture()?),
        term: Some(term()?),
        media: Some(vec![CaptureMedium::Audio]),
        processing: Some(vec![CaptureProcessing::LocalStt]),
        requested_at: Some(INSIDE),
        not_after: Some(TERM_TO),
    })
}

/// A new offering has no permission and resolves to `UNKNOWN`.
///
/// Three surfaces are read, because a default that is only absent from one of
/// them is still a default: the status, the capability, and the section 38 cell
/// report.
#[test]
fn new_offering_permission_defaults_unknown() -> TestResult {
    let mut ledger = ConsentLedger::new();
    assert!(ledger.records().is_empty());
    assert_eq!(
        ledger.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Unknown
    );
    assert_eq!(
        ledger.status(offering_b()?, next_term()?, INSIDE),
        CaptureStatus::Unknown
    );

    let denial = mint_capture_capability(&mut ledger, &request()?, INSIDE)
        .err()
        .ok_or("a new offering must mint nothing")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionUnknown);
    assert_eq!(denial.status(), CaptureStatus::Unknown);

    let cells = ledger.unfilled_cells(offering_a()?, term()?, INSIDE);
    assert_eq!(
        cells.iter().map(|cell| cell.gate()).collect::<Vec<_>>(),
        vec![OpenGate::RecordingPermissionPerOffering]
    );
    assert_eq!(
        OpenGate::RecordingPermissionPerOffering.identifier(),
        "GATE-38-009"
    );
    assert_eq!(
        OpenGate::CaptureAndTranscriptionConditions.identifier(),
        "GATE-38-019"
    );

    // An incomplete request is refused as incomplete rather than falling
    // through to a permissive default: this is `P2-G1`'s missing-tuple-field
    // denial, varied one field at a time.
    let whole = request()?;
    let mut varied = 0_usize;
    for index in 0..7 {
        let mut request = whole.clone();
        match index {
            0 => request.offering_id = None,
            1 => request.lecture_id = None,
            2 => request.term = None,
            3 => request.media = None,
            4 => request.processing = None,
            5 => request.requested_at = None,
            _ => request.not_after = None,
        }
        let denial = bind_permission(&ledger, &request, INSIDE)
            .err()
            .ok_or("a request missing a field must be refused")?;
        assert_eq!(denial.reason(), CaptureDenialReason::IncompleteRequest);
        varied += 1;
    }
    assert_eq!(varied, 7);
    Ok(())
}

/// `UNKNOWN` and `PROHIBITED` both refuse, and both leave an audit row.
#[test]
fn unknown_or_prohibited_denies_recorder_capability() -> TestResult {
    let mut unknown = ConsentLedger::new();
    let denial = mint_capture_capability(&mut unknown, &request()?, INSIDE)
        .err()
        .ok_or("UNKNOWN must refuse")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionUnknown);
    assert!(
        unknown
            .entries()
            .iter()
            .any(|entry| entry.kind() == ConsentEventKind::CaptureCapabilityDenied)
    );

    let mut prohibited = ledger_with_refusal()?;
    assert_eq!(
        prohibited.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Prohibited
    );
    let denial = mint_capture_capability(&mut prohibited, &request()?, INSIDE)
        .err()
        .ok_or("PROHIBITED must refuse")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionProhibited);
    assert_eq!(denial.status(), CaptureStatus::Prohibited);
    assert!(
        prohibited
            .entries()
            .iter()
            .any(|entry| entry.kind() == ConsentEventKind::CaptureCapabilityDenied)
    );
    // A refusal is not a thing to keep asking about.
    assert!(!denial.queues_recheck());
    assert!(prohibited.rechecks().is_empty());

    // The control: the same request against a written grant does mint, so the
    // two refusals above are the status and not the fixture.
    let mut granted = ledger_with_grant()?;
    let token = mint_capture_capability(&mut granted, &request()?, INSIDE)?;
    assert_eq!(token.media(), [CaptureMedium::Audio]);
    assert_eq!(token.not_after(), TERM_TO);
    Ok(())
}

/// An oral attestation is filed as evidence and moves no status.
#[test]
fn oral_attestation_cannot_create_permission() -> TestResult {
    let mut ledger = ConsentLedger::new();
    let before = ledger.status(offering_a()?, term()?, INSIDE);

    let digest = ledger.record_attestation(offering_a()?, term()?, &oral_attestation(), INSIDE);
    assert_eq!(
        digest,
        ContentDigest::sha256(b"heard the instructor say it was fine")
    );

    assert_eq!(ledger.status(offering_a()?, term()?, INSIDE), before);
    assert_eq!(
        ledger.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Unknown
    );
    assert!(
        ledger.records().is_empty(),
        "filing an attestation must record no permission"
    );
    assert_eq!(
        ledger
            .entries()
            .iter()
            .map(academic_consent::LedgerEntry::kind)
            .collect::<Vec<_>>(),
        vec![ConsentEventKind::AttestationRecorded]
    );
    assert_eq!(
        oral_attestation().kind(),
        AttestationKind::OralInstructorPermission
    );

    let denial = mint_capture_capability(&mut ledger, &request()?, INSIDE)
        .err()
        .ok_or("an attestation must mint nothing")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionUnknown);

    // Filing every attestation kind, repeatedly, changes nothing.
    for _ in 0..3 {
        ledger.record_attestation(offering_a()?, term()?, &oral_attestation(), INSIDE);
        ledger.record_attestation(offering_a()?, term()?, &personal_use_attestation(), INSIDE);
    }
    assert_eq!(
        ledger.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Unknown
    );
    assert!(ledger.records().is_empty());
    Ok(())
}

/// A personal-use belief is the same: evidence, never a transition.
#[test]
fn personal_use_text_cannot_create_permission() -> TestResult {
    let mut ledger = ConsentLedger::new();
    ledger.record_attestation(offering_a()?, term()?, &personal_use_attestation(), INSIDE);
    assert_eq!(
        personal_use_attestation().kind(),
        AttestationKind::PersonalUseBelief
    );
    assert_eq!(
        ledger.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Unknown
    );

    // Filing supporting evidence beside it does not help either: an artifact is
    // an artifact, and a status comes from a `PermissionRecord`.
    ledger.record_evidence(offering_a()?, term()?, &artifact("my own notes")?, INSIDE);
    assert_eq!(
        ledger.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Unknown
    );
    assert!(ledger.records().is_empty());

    let denial = mint_capture_capability(&mut ledger, &request()?, INSIDE)
        .err()
        .ok_or("a personal-use belief must mint nothing")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionUnknown);

    // And the cell stays open, so the user is asked rather than assumed for.
    assert_eq!(
        ledger
            .unfilled_cells(offering_a()?, term()?, INSIDE)
            .iter()
            .map(|cell| cell.gate())
            .collect::<Vec<_>>(),
        vec![OpenGate::RecordingPermissionPerOffering]
    );
    Ok(())
}

/// A grant for one offering and term answers for no other, and a
/// single-lecture grain answers for no other session.
#[test]
fn permission_scope_does_not_cross_offering_or_term() -> TestResult {
    let mut ledger = ledger_with_grant()?;
    // The control: the exact scope mints.
    mint_capture_capability(&mut ledger, &request()?, INSIDE)?;

    let mut other_offering = request()?;
    other_offering.offering_id = Some(offering_b()?);
    let denial = bind_permission(&ledger, &other_offering, INSIDE)
        .err()
        .ok_or("another offering must not be answered")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionUnknown);
    assert_eq!(denial.status(), CaptureStatus::Unknown);

    let mut other_term = request()?;
    other_term.term = Some(next_term()?);
    let denial = bind_permission(&ledger, &other_term, INSIDE)
        .err()
        .ok_or("another term must not be answered")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionUnknown);
    assert_eq!(
        ledger.status(offering_a()?, next_term()?, INSIDE),
        CaptureStatus::Unknown
    );

    // A single-lecture grain answers for its own session and nothing else.
    let mut single = ConsentLedger::new();
    single.record_permission(
        PermissionRecord::record(
            other_permission_id()?,
            1,
            PermissionScope::new(
                offering_a()?,
                term()?,
                ScopeGrain::SingleLecture(lecture()?),
                TERM_FROM,
                TERM_TO,
            )?,
            Disposition::Granted(grant(Vec::new())?),
            complete_checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    bind_permission(&single, &request()?, INSIDE)?;
    let mut another_session = request()?;
    another_session.lecture_id = Some(other_lecture()?);
    let denial = bind_permission(&single, &another_session, INSIDE)
        .err()
        .ok_or("another session must not be answered")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionUnknown);
    Ok(())
}

/// Expiry refuses and queues a recheck; a stale verification does the same.
#[test]
fn expired_permission_denies_and_queues_recheck() -> TestResult {
    let mut ledger = ledger_with_grant()?;
    // Inside the interval, the same request mints.
    mint_capture_capability(&mut ledger, &request()?, INSIDE)?;
    assert!(ledger.rechecks().is_empty());

    // Past the interval, it does not.
    let mut late = request()?;
    late.requested_at = Some(TERM_TO);
    late.not_after = Some(TERM_TO);
    let denial = mint_capture_capability(&mut ledger, &late, TERM_TO)
        .err()
        .ok_or("an expired grant must refuse")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionExpired);
    assert_eq!(denial.status(), CaptureStatus::Expired);
    assert!(denial.queues_recheck());

    let queued = ledger.rechecks();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].offering_id(), offering_a()?);
    assert_eq!(queued[0].term(), term()?);
    assert_eq!(queued[0].status(), CaptureStatus::Expired);
    assert!(
        ledger
            .entries()
            .iter()
            .any(|entry| entry.kind() == ConsentEventKind::RecheckQueued)
    );
    // Queued once, not once per attempt.
    let _ = mint_capture_capability(&mut ledger, &late, TERM_TO);
    assert_eq!(ledger.rechecks().len(), 1);

    // A live capture is re-bound rather than trusted: the same token stops
    // working at the boundary.
    let mut running = ledger_with_grant()?;
    let token = mint_capture_capability(&mut running, &request()?, INSIDE)?;
    continue_capture(&mut running, &token, INSIDE)?;
    let denial = continue_capture(&mut running, &token, TERM_TO)
        .err()
        .ok_or("a capture must not outlive its grant")?;
    assert_eq!(denial.status(), CaptureStatus::Expired);

    // A verification recorded before the interval it covers is stale, which
    // section 3.7 lists beside expiry and which is the semester recheck: the
    // same grant carried into the next term reads `EXPIRED`.
    let mut stale = ConsentLedger::new();
    stale.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Granted(grant(Vec::new())?),
            complete_checklist()?,
            TERM_FROM - 1,
            ContentDigest::sha256(b"last term's syllabus"),
        )?,
        TERM_FROM,
    )?;
    assert_eq!(
        stale.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Expired
    );
    let denial = mint_capture_capability(&mut stale, &request()?, INSIDE)
        .err()
        .ok_or("a stale verification must refuse")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionExpired);
    assert_eq!(stale.rechecks().len(), 1);
    Ok(())
}

/// Inheritance narrows or equals on both axes, over the whole grid.
#[test]
fn derivative_expiry_is_equal_or_stricter() -> TestResult {
    let bounds = [
        RetentionBound::Prohibited,
        RetentionBound::Until(1),
        RetentionBound::Until(1_000),
        RetentionBound::Until(1_000_000),
        RetentionBound::Until(u64::MAX),
    ];
    let mut pairs = 0_usize;
    let mut strictly_narrowed = 0_usize;
    for parent_audio in bounds {
        for parent_transcript in bounds {
            let parent = RetentionTerms::new(parent_audio, parent_transcript);
            for requested_audio in bounds {
                for requested_transcript in bounds {
                    let requested = RetentionTerms::new(requested_audio, requested_transcript);
                    let derived = parent.inherit(requested);
                    pairs += 1;
                    assert!(
                        derived.is_no_wider_than(parent),
                        "a derivative widened its parent: {parent:?} inherit {requested:?} = \
                         {derived:?}"
                    );
                    assert!(
                        derived.is_no_wider_than(requested),
                        "a derivative widened its own request: {parent:?} inherit {requested:?} \
                         = {derived:?}"
                    );
                    if derived != parent {
                        strictly_narrowed += 1;
                    }
                    // Inheriting twice is inheriting once: the rule has a fixed
                    // point, so a chain of derivatives cannot drift wider.
                    assert_eq!(derived.inherit(requested), derived);
                    assert_eq!(parent.inherit(derived), derived);
                }
            }
        }
    }
    assert_eq!(pairs, bounds.len().pow(4));
    assert!(
        strictly_narrowed > 0,
        "the grid must contain pairs that actually narrow, or it proves nothing"
    );

    // `Prohibited` is strictly below every instant, so it wins every pairing.
    for bound in bounds {
        assert_eq!(
            RetentionBound::Prohibited.stricter(bound),
            RetentionBound::Prohibited
        );
        assert_eq!(
            bound.stricter(RetentionBound::Prohibited),
            RetentionBound::Prohibited
        );
    }

    // The same rule, through the preview a user actually reads: every
    // derivative node is no wider than the parent on both axes.
    let mut ledger = ledger_with_grant()?;
    let subject = SubjectInventory::new(
        offering_a()?,
        term()?,
        permission_id()?,
        split_retention(),
        1,
        1,
        DERIVATIVE_CLASSES
            .iter()
            .map(|class| {
                (
                    *class,
                    2,
                    RetentionTerms::new(
                        RetentionBound::Until(u64::MAX),
                        RetentionBound::Until(u64::MAX),
                    ),
                )
            })
            .collect(),
    );
    let impact = preview_expiry(&mut ledger, &subject, INSIDE);
    assert_eq!(impact.derivatives().len(), DERIVATIVE_CLASSES.len());
    for node in impact.derivatives() {
        assert!(
            node.inherited().is_no_wider_than(split_retention()),
            "{:?} inherited wider terms than its parent",
            node.class()
        );
    }
    Ok(())
}

/// The two bounds are carried, compared, and reported separately.
#[test]
fn audio_and_transcript_retention_are_independent() -> TestResult {
    let audio_only =
        RetentionTerms::new(RetentionBound::Until(10), RetentionBound::Until(u64::MAX));
    let transcript_only =
        RetentionTerms::new(RetentionBound::Until(u64::MAX), RetentionBound::Until(10));
    assert_ne!(audio_only, transcript_only);
    assert_eq!(audio_only.audio(), transcript_only.transcript());
    assert_eq!(audio_only.transcript(), transcript_only.audio());

    // Reported separately at an instant between the two bounds.
    let mut ledger = ledger_with_grant()?;
    let offering = offering_a()?;
    let this_term = term()?;
    let permission = permission_id()?;
    let subject = |terms| {
        SubjectInventory::new(
            offering,
            this_term,
            permission,
            terms,
            3,
            5,
            vec![(
                DerivativeClass::Transcript,
                7,
                RetentionTerms::new(
                    RetentionBound::Until(u64::MAX),
                    RetentionBound::Until(u64::MAX),
                ),
            )],
        )
    };
    let audio_gone = preview_expiry(&mut ledger, &subject(audio_only), 100);
    assert!(audio_gone.audio().expires_now());
    assert!(!audio_gone.transcript().expires_now());
    assert_eq!(audio_gone.audio().object_count(), 3);
    assert_eq!(audio_gone.transcript().object_count(), 5);

    let transcript_gone = preview_expiry(&mut ledger, &subject(transcript_only), 100);
    assert!(!transcript_gone.audio().expires_now());
    assert!(transcript_gone.transcript().expires_now());

    // The two previews must not be the same value. A model that carried one
    // bound for both media would produce one preview for both of these.
    assert_ne!(audio_gone.digest(), transcript_gone.digest());
    assert_ne!(
        audio_gone.objects_reached(),
        transcript_gone.objects_reached()
    );

    // And the derivative node reports both axes, not a single verdict.
    let node = audio_gone
        .derivatives()
        .iter()
        .find(|node| node.class() == DerivativeClass::Transcript)
        .ok_or("the transcript class must appear in every preview")?;
    assert!(node.audio_expires_now());
    assert!(!node.transcript_expires_now());

    // Every pair in a small grid produces a distinct digest, so no two
    // (audio, transcript) pairs collapse onto one preview.
    let instants = [5_u64, 10, 50];
    let mut digests = Vec::new();
    for audio in instants {
        for transcript in instants {
            let terms = RetentionTerms::new(
                RetentionBound::Until(audio),
                RetentionBound::Until(transcript),
            );
            digests.push(*preview_expiry(&mut ledger, &subject(terms), 1).digest());
        }
    }
    let mut unique = digests.clone();
    unique.sort_unstable_by_key(|digest| *digest.as_bytes());
    unique.dedup();
    assert_eq!(unique.len(), digests.len(), "two retention pairs collapsed");
    Ok(())
}

/// An omission downgrades to conditional with a grant, and stays unknown
/// without one.
#[test]
fn checklist_omission_yields_conditional_or_unknown() -> TestResult {
    // With a written grant and a whole checklist: `PERMITTED`.
    let ledger = ledger_with_grant()?;
    assert_eq!(
        ledger.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Permitted
    );

    // With a written grant and any one dimension missing:
    // `PERMITTED_WITH_CONDITIONS`, and the exact dimension travels to the token.
    let mut omitted_count = 0_usize;
    for omitted in academic_consent::CHECKLIST_DIMENSIONS {
        let mut ledger = ConsentLedger::new();
        ledger.record_permission(
            PermissionRecord::record(
                permission_id()?,
                1,
                whole_term_scope(offering_a()?, term()?)?,
                Disposition::Granted(grant(Vec::new())?),
                checklist_missing(omitted)?,
                TERM_FROM,
                ContentDigest::sha256(b"verification"),
            )?,
            TERM_FROM,
        )?;
        assert_eq!(
            ledger.status(offering_a()?, term()?, INSIDE),
            CaptureStatus::PermittedWithConditions,
            "omitting {omitted:?} must downgrade the status"
        );
        let token = mint_capture_capability(&mut ledger, &request()?, INSIDE)?;
        assert_eq!(token.bound().unanswered(), [omitted]);
        assert_eq!(
            token.bound().status(),
            CaptureStatus::PermittedWithConditions
        );
        omitted_count += 1;
    }
    assert_eq!(omitted_count, academic_consent::CHECKLIST_DIMENSIONS.len());

    // With no written grant, an answered checklist changes nothing.
    let mut without_grant = ConsentLedger::new();
    assert!(complete_checklist()?.is_complete());
    assert_eq!(
        without_grant.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Unknown
    );
    let denial = mint_capture_capability(&mut without_grant, &request()?, INSIDE)
        .err()
        .ok_or("a checklist alone must mint nothing")?;
    assert_eq!(denial.status(), CaptureStatus::Unknown);

    // An empty checklist names all seven, and answering is one-way.
    let mut checklist = Checklist::new();
    assert_eq!(
        checklist.unanswered(),
        academic_consent::CHECKLIST_DIMENSIONS.to_vec()
    );
    checklist.answer(
        ChecklistDimension::Copyright,
        ChecklistEntry::Evidenced(artifact("copyright")?),
    )?;
    assert!(
        checklist
            .answer(
                ChecklistDimension::Copyright,
                ChecklistEntry::Evidenced(artifact("again")?),
            )
            .is_err()
    );
    Ok(())
}

/// A legal question leaves the system and comes back as nothing.
#[test]
fn legal_exception_is_an_external_task_not_an_inference() -> TestResult {
    let mut ledger = ConsentLedger::new();
    let before = ledger.status(offering_a()?, term()?, INSIDE);
    assert_eq!(before, CaptureStatus::Unknown);

    let task = open_external_review(
        offering_a()?,
        term()?,
        LegalQuestion::CopyrightExceptionApplies,
        ReferralTarget::InstitutionalLegalOffice,
        INSIDE,
    );
    ledger.record_external_review(&task, INSIDE);

    assert_eq!(task.question(), LegalQuestion::CopyrightExceptionApplies);
    assert_eq!(task.referred_to(), ReferralTarget::InstitutionalLegalOffice);
    assert_eq!(ledger.status(offering_a()?, term()?, INSIDE), before);
    assert!(ledger.records().is_empty());
    assert_eq!(
        ledger
            .entries()
            .iter()
            .map(academic_consent::LedgerEntry::kind)
            .collect::<Vec<_>>(),
        vec![ConsentEventKind::ExternalReviewOpened]
    );

    let denial = mint_capture_capability(&mut ledger, &request()?, INSIDE)
        .err()
        .ok_or("an open review must mint nothing")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionUnknown);

    // Opening every question, against a written refusal, changes nothing
    // either: an exception is not a route around an authority's "no".
    let mut refused = ledger_with_refusal()?;
    for question in [
        LegalQuestion::CopyrightExceptionApplies,
        LegalQuestion::AccommodationOverridesRefusal,
        LegalQuestion::StudentSpeechRetentionIsLawful,
        LegalQuestion::InstitutionalRulePermitsWhereInstructorIsSilent,
        LegalQuestion::CrossBorderProcessingIsLawful,
    ] {
        let task = open_external_review(
            offering_a()?,
            term()?,
            question,
            ReferralTarget::ExternalProfessional,
            INSIDE,
        );
        refused.record_external_review(&task, INSIDE);
    }
    assert_eq!(
        refused.status(offering_a()?, term()?, INSIDE),
        CaptureStatus::Prohibited
    );
    let denial = mint_capture_capability(&mut refused, &request()?, INSIDE)
        .err()
        .ok_or("a refusal survives every referral")?;
    assert_eq!(denial.reason(), CaptureDenialReason::PermissionProhibited);
    Ok(())
}

/// An expiry cannot be applied without the preview it describes.
///
/// Not one of the ten named rows: the outcome sentence that requires "a preview
/// of deletion impact before any expiry action" has no name of its own, so this
/// carries it.
#[test]
fn expiry_requires_the_preview_it_was_shown_for() -> TestResult {
    let mut ledger = ledger_with_grant()?;
    let subject = SubjectInventory::new(
        offering_a()?,
        term()?,
        permission_id()?,
        split_retention(),
        4,
        6,
        vec![(DerivativeClass::Embedding, 9, split_retention())],
    );

    // Before either bound, nothing has expired and there is nothing to apply.
    let early = preview_expiry(&mut ledger, &subject, 1_500_000);
    assert_eq!(early.objects_reached(), 0);
    assert_eq!(
        apply_expiry(
            &mut ledger,
            &ExpiryPlan::from_preview(early.clone()),
            1_500_000
        )
        .err(),
        Some(ExpiryRefusal::NothingHasExpired)
    );

    // After the audio bound only, the audio and the embedding go.
    let audio_due = preview_expiry(&mut ledger, &subject, 1_700_000);
    assert!(audio_due.audio().expires_now());
    assert!(!audio_due.transcript().expires_now());
    assert_eq!(audio_due.objects_reached(), 4 + 9);

    // A plan taken at one instant cannot be applied at another.
    let plan = ExpiryPlan::from_preview(audio_due.clone());
    assert_eq!(
        apply_expiry(&mut ledger, &plan, 1_800_000).err(),
        Some(ExpiryRefusal::PreviewIsForAnotherInstant)
    );
    assert_eq!(apply_expiry(&mut ledger, &plan, 1_700_000)?, 13);
    assert!(
        ledger
            .entries()
            .iter()
            .any(|entry| entry.kind() == ConsentEventKind::ExpiryApplied)
    );
    let previews = ledger
        .entries()
        .iter()
        .filter(|entry| entry.kind() == ConsentEventKind::ExpiryPreviewed)
        .count();
    assert_eq!(previews, 2, "every preview leaves its own row");
    Ok(())
}

/// The section 3.7 fields a grant carries, read back through the aggregate.
///
/// Not one of the ten named rows either. It is here because the two flags
/// section 3.7 defaults to `0` are the ones a caller is most likely to get
/// wrong, and nothing else reads them back.
#[test]
fn grant_carries_every_section_37_field() -> TestResult {
    let conditional = grant(vec![
        Condition::NoStudentVoices,
        Condition::LocalProcessingOnly,
    ])?;
    assert_eq!(
        conditional.authority().authority(),
        GrantAuthority::Instructor
    );
    assert_eq!(conditional.allowed_media(), [CaptureMedium::Audio]);
    assert_eq!(
        conditional.allowed_processing(),
        [CaptureProcessing::LocalStt]
    );
    assert!(!conditional.external_processing_allowed());
    assert!(!conditional.sharing_allowed());
    assert_eq!(conditional.retention(), &split_retention());
    assert_eq!(conditional.not_after(), TERM_TO);
    assert_eq!(conditional.conditions().len(), 2);
    let empty = grant(Vec::new())?;
    assert_ne!(conditional.conditions_digest(), empty.conditions_digest());

    // `GATE-38-019` is empty when a grant lists nothing, and the cell says so.
    let mut ledger = ConsentLedger::new();
    ledger.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Granted(AuthorityGrant::record(
                written_syllabus()?,
                PermittedUse::new(Vec::new(), Vec::new(), false, false),
                split_retention(),
                Vec::new(),
                TERM_TO,
            )),
            complete_checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    assert_eq!(
        ledger
            .unfilled_cells(offering_a()?, term()?, INSIDE)
            .iter()
            .map(|cell| cell.gate())
            .collect::<Vec<_>>(),
        vec![OpenGate::CaptureAndTranscriptionConditions]
    );
    let denial = bind_permission(&ledger, &request()?, INSIDE)
        .err()
        .ok_or("an empty media set must match no request")?;
    assert_eq!(denial.reason(), CaptureDenialReason::MediumNotGranted);

    // And a request that asks for no medium at all is refused too, so an
    // unconfirmed offering is not reachable by asking for nothing.
    let mut nothing = request()?;
    nothing.media = Some(Vec::new());
    let denial = bind_permission(&ledger, &nothing, INSIDE)
        .err()
        .ok_or("a request for no medium must be refused")?;
    assert_eq!(denial.reason(), CaptureDenialReason::MediumNotGranted);
    // An empty processing list is not the same thing: a capture recorded now
    // and processed later asks for exactly that, and the fixture grant covers
    // it.
    let mut record_only = request()?;
    record_only.processing = Some(Vec::new());
    bind_permission(&ledger_with_grant()?, &record_only, INSIDE)?;

    // External processing is refused by the flag even when it is on the list.
    let mut external = ConsentLedger::new();
    external.record_permission(
        PermissionRecord::record(
            permission_id()?,
            1,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Granted(AuthorityGrant::record(
                written_syllabus()?,
                PermittedUse::new(
                    vec![CaptureMedium::Audio],
                    vec![CaptureProcessing::ExternalStt],
                    false,
                    false,
                ),
                split_retention(),
                Vec::new(),
                TERM_TO,
            )),
            complete_checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )?,
        TERM_FROM,
    )?;
    let mut cloud = request()?;
    cloud.processing = Some(vec![CaptureProcessing::ExternalStt]);
    let denial = bind_permission(&external, &cloud, INSIDE)
        .err()
        .ok_or("external processing must need its own flag")?;
    assert_eq!(
        denial.reason(),
        CaptureDenialReason::ExternalProcessingNotGranted
    );
    // A token whose lifetime is already over at the instant it is asked for is
    // refused rather than minted dead, so `mint_capture_capability` and
    // `continue_capture` agree about what a live token is.
    let mut already_over = request()?;
    already_over.not_after = Some(INSIDE);
    let denial = bind_permission(&ledger_with_grant()?, &already_over, INSIDE)
        .err()
        .ok_or("a token whose lifetime is already over must be refused")?;
    assert_eq!(denial.reason(), CaptureDenialReason::LifetimeExceedsGrant);

    // The section 3.7 key starts at one on both sides. Migration 0006 CHECKs it
    // and so does the constructor, so a record the database would refuse is not
    // representable here either.
    assert!(
        PermissionRecord::record(
            permission_id()?,
            0,
            whole_term_scope(offering_a()?, term()?)?,
            Disposition::Granted(grant(Vec::new())?),
            complete_checklist()?,
            TERM_FROM,
            ContentDigest::sha256(b"verification"),
        )
        .is_err()
    );

    let _ = audio_local_use();
    Ok(())
}
