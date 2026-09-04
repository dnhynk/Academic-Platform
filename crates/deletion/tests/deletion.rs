//! `P2-P2`'s six named acceptance rows.
//!
//! t068 section 5 names them: `dry_run_enumerates_every_derivative_class`,
//! `impact_preview_precedes_confirmation`,
//! `protected_artifact_returns_a_policy_reason`,
//! `provider_deletion_receipt_is_stored_and_linked`,
//! `leak_incident_cannot_be_closed_by_claim_supersession` and
//! `deletion_confirmation_is_non_delegable`. Faults `RB01`-`RB04` are in
//! `deletion_faults.rs` and the whole-set inventories in `deletion_scans.rs`.

mod support;

use std::{collections::BTreeSet, error::Error, fs};

use academic_deletion::{
    ClassTargets, DeletionConfirmation, DeletionDryRun, DeletionFlowError, DeletionImpactPreview,
    EvidenceCitations, ExposureScope, ExternalLeakIncident, FilesystemExecutor, IncidentError,
    LeakIncidentState, ProtectionDecision, ProtectionPolicyKind, ProtectionReason,
    ProviderErasureLog, ProviderErasureRequest, RecoveryStep, SPEC_DERIVATIVE_SENTENCE_HEAD,
    SPEC_DERIVATIVE_SENTENCE_TAIL, SPEC_DERIVATIVE_WORDS, execute_deletion,
};
use academic_domain::{Actor, EgressDecisionId, TimestampMillis};
use academic_evidence_center::{
    ConflictCase, ConflictClass, ConflictLane, ConflictSide, CorrectionOutcome, DeletionReceiptRef,
    ReceiptState, user_receipt,
};
use academic_policy::{
    ContentDigest as PolicyDigest, DeletionReceiptDraft, EgressRule, ObjectRange, PermissionBroker,
    PermissionRequest, PolicySnapshot, ProcessClass, ProviderIdentity, ProviderPolicyDraft,
    ProviderPolicySnapshot, ProviderSurface, RuntimeToolCall,
};
use academic_retention::{
    ActionId, AppendOnlyJournal, DELETION_JOURNAL_RELATIVE_PATH, DERIVATIVE_CLASSES,
    DerivativeClass, RetentionOutcome,
};
use academic_student_voice::{AffectedProjectionKind, EvidenceIndex, ProjectionRecord};

use support::{
    DECIDING_USER, MODEL_RUN, NothingProtected, RecordingShredder, SHARED_A, SHARED_B,
    SUBJECT_ARTIFACT, StatedIndex, StatedProtection, TestResult, TestRoot, artifact, digest,
    entity, locator, paths_for, target, touch,
};

const DESIGN_DOCUMENT: &str = "../../PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md";

// ---------------------------------------------------------------------------
// dry_run_enumerates_every_derivative_class
// ---------------------------------------------------------------------------

/// Every derivative class, every time, and the list is the specification's.
///
/// Two halves, and neither is a hand-written list of seven names.
///
/// **The list.** Section 32.10's first bullet is parsed out of the design
/// document, split on its own separators, and compared with
/// `SPEC_DERIVATIVE_WORDS` in both directions. A class with no sentence fails,
/// a phrase with no class fails, and a document that stops saying it fails.
/// The count is taken from the parse rather than asserted, so "seven" is what
/// the document has rather than what this file remembers.
///
/// **The walk.** Every assignment of the three `ClassTargets` shapes to the
/// seven classes is driven — 3^7 = 2187 resolvers — and the node list has to be
/// `DERIVATIVE_CLASSES`, in registry order, in all of them. A build that
/// dropped an empty class, or that reordered on one resolver answer and not
/// another, fails on the case that reaches it. `RB03`'s row is inside this
/// sweep: every case with at least one `Unresolved` class has to name exactly
/// those classes and no others.
#[test]
fn dry_run_enumerates_every_derivative_class() -> TestResult {
    // -- the list is the specification's, in both directions ----------------
    let document = fs::read_to_string(DESIGN_DOCUMENT)?;
    let head = document
        .find(SPEC_DERIVATIVE_SENTENCE_HEAD)
        .ok_or("section 32.10 no longer states the derivative dependency plan")?
        + SPEC_DERIVATIVE_SENTENCE_HEAD.len();
    let tail = document[head..]
        .find(SPEC_DERIVATIVE_SENTENCE_TAIL)
        .ok_or("section 32.10's derivative sentence no longer ends where it did")?;
    let listed: Vec<&str> = document[head..head + tail]
        .split(", ")
        .map(str::trim)
        .collect();

    let claimed: BTreeSet<&str> = SPEC_DERIVATIVE_WORDS
        .iter()
        .map(|(_, word)| *word)
        .collect();
    let stated: BTreeSet<&str> = listed.iter().copied().collect();
    assert_eq!(
        stated, claimed,
        "section 32.10's derivative list and this crate's are not the same set"
    );
    assert_eq!(
        listed.len(),
        SPEC_DERIVATIVE_WORDS.len(),
        "section 32.10 lists a phrase twice, or this crate does"
    );
    assert_eq!(
        DERIVATIVE_CLASSES.len(),
        listed.len(),
        "P2-K5's registry and section 32.10's sentence hold different numbers of classes"
    );
    let mapped: Vec<DerivativeClass> = SPEC_DERIVATIVE_WORDS
        .iter()
        .map(|(class, _)| *class)
        .collect();
    assert_eq!(
        mapped,
        DERIVATIVE_CLASSES.to_vec(),
        "the specification mapping is not P2-K5's registry, in registry order"
    );
    // The order the document states is the order the registry reports.
    let ordered: Vec<&str> = SPEC_DERIVATIVE_WORDS
        .iter()
        .map(|(_, word)| *word)
        .collect();
    assert_eq!(
        listed, ordered,
        "the registry order is not the order section 32.10 reads in"
    );

    // -- the walk enumerates all of them, for every resolver behaviour ------
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let derived = target(SHARED_A, 0x22)?;
    let registry = DERIVATIVE_CLASSES.to_vec();
    let shapes = 3_usize.pow(u32::try_from(registry.len())?);
    for case in 0..shapes {
        let mut index = StatedIndex::new();
        let mut expected_unresolved = Vec::new();
        let mut digits = case;
        for class in &registry {
            let shape = digits % 3;
            digits /= 3;
            match shape {
                0 => {
                    index.state(*class, ClassTargets::Targets(vec![derived]));
                }
                1 => {
                    index.state(
                        *class,
                        ClassTargets::NothingToDelete {
                            reason: format!("nothing of class {} exists here", class.as_str()),
                        },
                    );
                }
                _ => {
                    index.state(
                        *class,
                        ClassTargets::Unresolved {
                            reason: format!("{} could not be answered for", class.as_str()),
                        },
                    );
                    expected_unresolved.push(*class);
                }
            }
        }
        let dry_run = DeletionDryRun::of(subject, &index, &NothingProtected);
        assert_eq!(
            dry_run.enumerated_classes(),
            registry,
            "case {case} did not enumerate every class in registry order"
        );
        assert_eq!(
            dry_run.nodes().len(),
            registry.len(),
            "case {case} produced a node count that is not the registry's"
        );
        assert_eq!(
            dry_run.unresolved_classes(),
            expected_unresolved,
            "case {case} named the wrong unresolved classes"
        );
        // `P2-K5`'s plan is built from the same nodes, so a class that vanished
        // from one would vanish from both rather than from neither.
        assert_eq!(
            dry_run.plan().enumerated_classes(),
            registry,
            "case {case}'s plan is not the dry run's enumeration"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// impact_preview_precedes_confirmation
// ---------------------------------------------------------------------------

/// A confirmation cannot exist before a preview, and the preview is total.
///
/// Three things:
///
/// 1. The only way to a `DeletionConfirmation` is `given`, which takes a
///    `DeletionImpactPreview` by value. `tests/compile_fail` holds the struct
///    literal that would go round it.
/// 2. The preview shows section 32.5's concept and evidence projections, and it
///    covers every artifact the dry run reaches: a citation map that is short
///    is a refusal, not a shorter list.
/// 3. The confirmation is bound to that exact preview's digest, so confirming
///    one deletion does not authorise another.
#[test]
fn impact_preview_precedes_confirmation() -> TestResult {
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let transcript = target(SHARED_A, 0x22)?;
    let embedding = target(SHARED_B, 0x22)?;

    let mut index = StatedIndex::all_empty("this synthetic subject has no derivative of this kind");
    index
        .state(
            DerivativeClass::Transcript,
            ClassTargets::Targets(vec![transcript]),
        )
        .state(
            DerivativeClass::Embedding,
            ClassTargets::Targets(vec![embedding]),
        );
    let dry_run = DeletionDryRun::of(subject, &index, &NothingProtected);

    let subject_evidence = digest("subject-bytes");
    let transcript_evidence = digest("transcript-bytes");
    let embedding_evidence = digest("embedding-bytes");
    let untouched = digest("some-other-evidence");

    let projections = EvidenceIndex::of(vec![
        ProjectionRecord::citing(
            AffectedProjectionKind::Concept,
            "concept:transitive-closure",
            vec![subject_evidence, untouched],
        )?,
        ProjectionRecord::citing(
            AffectedProjectionKind::Evidence,
            "evidence:lecture-04-claim",
            vec![transcript_evidence],
        )?,
    ])?;

    // A map that is short refuses rather than shortening the preview.
    let mut short = EvidenceCitations::new();
    short.cite(subject, subject_evidence);
    short.cite(transcript, transcript_evidence);
    let refused = DeletionImpactPreview::of(dry_run.clone(), &projections, &short, 1_000);
    assert!(
        matches!(
            refused,
            Err(DeletionFlowError::EvidenceCitationMissing(ref missing))
                if **missing == embedding
        ),
        "a preview was produced for a deletion it could not account for"
    );

    let mut citations = short;
    citations.cite(embedding, embedding_evidence);
    let preview = DeletionImpactPreview::of(dry_run, &projections, &citations, 1_000)?;

    // Section 32.5's two families, and what each one loses.
    let kinds: Vec<AffectedProjectionKind> = preview
        .projections()
        .iter()
        .map(academic_student_voice::AffectedProjection::kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            AffectedProjectionKind::Concept,
            AffectedProjectionKind::Evidence
        ],
        "the preview did not report both projection families"
    );
    assert_eq!(
        preview.projections()[0].effect(),
        academic_student_voice::ProjectionEffect::LosesSomeEvidence
    );
    assert_eq!(
        preview.projections()[1].effect(),
        academic_student_voice::ProjectionEffect::LosesAllEvidence
    );
    // The partition: every reached artifact is cited or unreferenced, never both.
    assert!(
        preview.partition_reconciles(&projections),
        "the preview left a reached artifact in neither set, or in both"
    );
    assert_eq!(preview.unreferenced(), &[embedding_evidence]);
    assert_eq!(preview.reached().len(), 3);
    assert_eq!(preview.reached()[0], subject);

    // The confirmation is bound to this preview and to no other.
    let user = Actor::User {
        user_id: entity(DECIDING_USER)?,
    };
    let wrong = DeletionConfirmation::given(
        preview.clone(),
        &user,
        digest("a-preview-nobody-was-shown"),
        TimestampMillis::new(2_000),
    );
    assert!(
        matches!(wrong, Err(DeletionFlowError::ConfirmedAnotherPreview)),
        "a confirmation was accepted for a preview that was not shown"
    );
    let confirmation = DeletionConfirmation::given(
        preview.clone(),
        &user,
        preview.digest(),
        TimestampMillis::new(2_000),
    )?;
    assert_eq!(confirmation.preview().digest(), preview.digest());
    assert_eq!(
        confirmation.preview().dry_run().enumerated_classes(),
        DERIVATIVE_CLASSES.to_vec(),
        "the confirmed preview lost the class enumeration"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// protected_artifact_returns_a_policy_reason
// ---------------------------------------------------------------------------

/// A protected artifact refuses with a policy, a section, and words.
///
/// Every arm of `ProtectionPolicyKind` is driven, because a registry that
/// refused under one policy and returned a bare `NotProtected` under another
/// would pass a case that only tried the first. The refusal reaches the caller
/// as a typed error carrying the reason, and the dry run still enumerates every
/// class: a user told "this cannot be deleted" and shown nothing has been told
/// less than the previous screen showed.
#[test]
fn protected_artifact_returns_a_policy_reason() -> TestResult {
    let document = fs::read_to_string(DESIGN_DOCUMENT)?;
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let index = StatedIndex::all_empty("nothing derived from this synthetic subject");

    for kind in ProtectionPolicyKind::ALL {
        assert!(
            document.contains(kind.spec_words()),
            "{} cites words section {} no longer holds",
            kind.as_str(),
            kind.spec_section()
        );
        let mut protection = StatedProtection::new();
        protection.protect(
            subject,
            ProtectionReason::under(
                kind,
                format!("this synthetic subject is held by {}", kind.as_str()),
                Some(TimestampMillis::new(9_000)),
            ),
        );
        let dry_run = DeletionDryRun::of(subject, &index, &protection);

        // The decision carries the reason; there is no arm that refuses without
        // one, and the reason renders the policy and the section every time.
        let ProtectionDecision::Protected(reason) = dry_run.protection() else {
            return Err(format!("{} did not refuse", kind.as_str()).into());
        };
        assert_eq!(reason.kind(), kind);
        let row = reason.to_row();
        assert!(
            row.contains(kind.as_str()),
            "the row lost the policy: {row}"
        );
        assert!(
            row.contains(kind.spec_section()),
            "the row lost the section: {row}"
        );
        assert_eq!(reason.revisit_at(), Some(TimestampMillis::new(9_000)));

        // The dry run is still whole.
        assert_eq!(dry_run.enumerated_classes(), DERIVATIVE_CLASSES.to_vec());

        // The refusal reaches the caller as a reason, not as an absence.
        let refused = DeletionImpactPreview::of(
            dry_run,
            &EvidenceIndex::default(),
            &EvidenceCitations::new(),
            1_000,
        );
        match refused {
            Err(DeletionFlowError::Protected {
                target: refused_target,
                reason: refused_reason,
            }) => {
                assert_eq!(*refused_target, subject);
                assert_eq!(refused_reason.kind(), kind);
            }
            other => {
                return Err(format!(
                    "{} produced {other:?} instead of a policy reason",
                    kind.as_str()
                )
                .into());
            }
        }
    }

    // An unprotected artifact is not refused, so the guard is not passing
    // because everything refuses.
    let dry_run = DeletionDryRun::of(subject, &index, &NothingProtected);
    assert_eq!(dry_run.protection().reason(), None);
    let mut citations = EvidenceCitations::new();
    citations.cite(subject, digest("subject-bytes"));
    DeletionImpactPreview::of(dry_run, &EvidenceIndex::default(), &citations, 1_000)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// provider_deletion_receipt_is_stored_and_linked
// ---------------------------------------------------------------------------

/// The provider's own receipt is stored, and it is linked to this deletion.
///
/// The row is `P2-G3`'s: the broker persists it against the grant and the exact
/// allow-audit row of the transmission it deletes, and this suite drives the
/// real broker through a dev edge rather than restating that. What `P2-P2` adds
/// is the fourth link — the artifact deletion that caused the request — and the
/// rule that a local `COMPLETE` beside an unsettled provider copy is not "it is
/// gone".
#[test]
fn provider_deletion_receipt_is_stored_and_linked() -> TestResult {
    // -- the broker's own row, stored and read back -------------------------
    let broker = PermissionBroker::new_profile()?;
    let provider = broker.register_provider_policy(provider_draft()?, 0)?;
    let policy = broker.install_policy(PolicySnapshot::from_rules(vec![rule(&provider)?])?)?;
    let issued = broker.evaluate(request(policy, &provider, 10)?, 10)?;
    let grant_id = issued.receipt.grant_id().ok_or("missing grant")?.to_owned();
    let capability = issued.capability.ok_or("missing capability")?;
    broker.execute(&capability, runtime(&provider)?, 10, |_| ())?;
    let audit_seq = broker
        .audit_rows()?
        .last()
        .ok_or("missing allow audit")?
        .audit_seq;
    let stored = broker.store_deletion_receipt(DeletionReceiptDraft {
        receipt_id: "synthetic-provider-receipt".to_owned(),
        grant_id: grant_id.clone(),
        egress_audit_seq: audit_seq,
        provider_receipt_digest: PolicyDigest::of(b"synthetic-provider-receipt-bytes"),
        requested_at: 20,
        received_at: 21,
    })?;
    assert_eq!(
        broker.deletion_receipt("synthetic-provider-receipt")?,
        Some(stored.clone()),
        "the broker did not persist the receipt it returned"
    );

    // -- the same fact, linked to this deletion -----------------------------
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let other = target(SHARED_B, 0x11)?;
    let decision =
        EgressDecisionId::try_from_uuid("01900000-0000-7000-8000-0000000002d1".parse()?)?;
    let unrelated =
        EgressDecisionId::try_from_uuid("01900000-0000-7000-8000-0000000002d2".parse()?)?;

    let mut log = ProviderErasureLog::new();
    log.request(
        ProviderErasureRequest::new(subject, decision, TimestampMillis::new(20)),
        ReceiptState::Requested {
            requested_at: TimestampMillis::new(20),
        },
    );
    // A second registration of the same bytes, at the same locator, sent to the
    // same provider under a different decision: two entries, never one.
    log.request(
        ProviderErasureRequest::new(other, unrelated, TimestampMillis::new(20)),
        ReceiptState::NotOffered,
    );
    assert_eq!(log.entries().len(), 2);
    assert_eq!(log.outstanding().len(), 2);

    let receipt = DeletionReceiptRef::new(
        domain_digest(stored.provider_receipt_digest.as_str())?,
        domain_digest(stored.provider_policy_snapshot_digest.as_str())?,
        TimestampMillis::new(i64::try_from(stored.requested_at)?),
        TimestampMillis::new(i64::try_from(stored.received_at)?),
    );

    // A receipt for something nobody asked about is refused.
    let stray = log.clone().record_receipt(subject, unrelated, receipt);
    assert!(
        matches!(stray, Err(DeletionFlowError::ReceiptWithoutRequest)),
        "a receipt was accepted against a request this deletion never made"
    );

    log.record_receipt(subject, decision, receipt)?;
    let entry = log
        .entry(&subject, decision)
        .ok_or("the entry lost its request")?;
    assert!(entry.is_settled());
    assert_eq!(
        entry.receipt().map(DeletionReceiptRef::receipt_digest),
        Some(domain_digest(stored.provider_receipt_digest.as_str())?),
        "the linked receipt is not the digest the broker stored"
    );
    assert_eq!(entry.request().target(), &subject);
    assert_eq!(entry.request().decision(), decision);

    // -- and the deletion result says so ------------------------------------
    let root = TestRoot::new("provider-link")?;
    let mut journal = AppendOnlyJournal::open(&root.path().join(DELETION_JOURNAL_RELATIVE_PATH))?;
    let index = StatedIndex::all_empty("nothing derived from this synthetic subject");
    let dry_run = DeletionDryRun::of(subject, &index, &NothingProtected);
    let mut citations = EvidenceCitations::new();
    citations.cite(subject, digest("subject-bytes"));
    let preview = DeletionImpactPreview::of(dry_run, &EvidenceIndex::default(), &citations, 1_000)?;
    let user = Actor::User {
        user_id: entity(DECIDING_USER)?,
    };
    let shown = preview.digest();
    let confirmation =
        DeletionConfirmation::given(preview, &user, shown, TimestampMillis::new(2_000))?;
    let mut shredder = RecordingShredder::default();
    let mut executor = FilesystemExecutor::new(
        &mut shredder,
        paths_for(&[], &[]),
        "0102030405060708090a0b0c0d0e0f10".to_owned(),
        3_000,
    );
    let result = execute_deletion(
        &mut journal,
        ActionId::from_bytes([0x51; 16]),
        &confirmation,
        &mut executor,
        log,
    )?;
    assert_eq!(result.outcome_word(), "COMPLETE");
    assert!(
        !result.is_fully_erased(),
        "a local COMPLETE reported the artifact gone while a provider copy is unsettled"
    );
    let rows = result.report_rows();
    assert_eq!(rows.len(), 1, "the outstanding provider copy was not named");
    assert!(rows[0].contains(&other.to_row()), "{rows:?}");
    assert!(rows[0].contains("NO_RECEIPT_OFFERED"), "{rows:?}");
    Ok(())
}

// ---------------------------------------------------------------------------
// leak_incident_cannot_be_closed_by_claim_supersession
// ---------------------------------------------------------------------------

/// Section 34.6's fifth principle, held by the type.
///
/// The first four principles are followed: the claim describing the leaked
/// artifact is superseded through `P2-X7`'s own correction board, and the
/// incident records that it was. The fifth is what this asserts — the incident
/// does not move. All three `CorrectionOutcome` arms are driven, including
/// `Modify`, which *is* supersession, and after every one of them the state is
/// still `OPEN` and `close` still refuses by naming the missing recovery step.
///
/// Then each recovery step is recorded in turn and `close` keeps refusing until
/// the last one, so the guard is not passing because `close` refuses
/// unconditionally.
#[test]
fn leak_incident_cannot_be_closed_by_claim_supersession() -> TestResult {
    let document = fs::read_to_string(DESIGN_DOCUMENT)?;
    assert!(
        document.contains(academic_deletion::EXTERNAL_LEAKAGE_PRINCIPLE),
        "section 34.6 no longer states that external leakage is not a correction"
    );

    let scope = ExposureScope::new(4_096, artifact(SUBJECT_ARTIFACT)?, digest("provider-x"), 30);
    let mut incident = ExternalLeakIncident::reported(scope, TimestampMillis::new(1_000));
    assert_eq!(incident.state(), LeakIncidentState::Open);

    let replacement = "01900000-0000-7000-8000-0000000002e1".parse()?;
    for outcome in [
        CorrectionOutcome::Keep,
        CorrectionOutcome::Modify { replacement },
        CorrectionOutcome::EndScope {
            ends_at: TimestampMillis::new(1_500),
        },
    ] {
        let record = settle_a_conflict(outcome)?;
        incident.record_claim_correction(&record);
        assert_eq!(
            incident.state(),
            LeakIncidentState::Open,
            "a {:?} correction moved the incident",
            record.choice()
        );
        assert!(
            matches!(
                incident.close(TimestampMillis::new(2_000)),
                Err(IncidentError::RecoveryStepMissing(_))
            ),
            "a {:?} correction closed the incident",
            record.choice()
        );
        assert_eq!(incident.closure(), None);
    }
    assert_eq!(incident.claim_corrections().len(), 3);
    assert_eq!(incident.missing_steps(), RecoveryStep::ALL.to_vec());

    // The lifecycle that does close it, one step at a time.
    for (index, step) in RecoveryStep::ALL.into_iter().enumerate() {
        incident.record_recovery(step);
        let remaining = RecoveryStep::ALL.len() - index - 1;
        if remaining > 0 {
            assert_eq!(incident.state(), LeakIncidentState::Open);
            assert!(
                matches!(
                    incident.close(TimestampMillis::new(2_000)),
                    Err(IncidentError::RecoveryStepMissing(_))
                ),
                "the incident closed with {remaining} recovery steps outstanding"
            );
        }
    }
    assert_eq!(incident.state(), LeakIncidentState::Contained);
    let closure = incident.close(TimestampMillis::new(3_000))?.clone();
    assert_eq!(closure.steps(), &RecoveryStep::ALL);
    assert_eq!(closure.scope().exposed_bytes(), 4_096);
    assert_eq!(incident.state(), LeakIncidentState::Closed);
    assert!(matches!(
        incident.close(TimestampMillis::new(4_000)),
        Err(IncidentError::AlreadyClosed)
    ));

    // The four steps are section 34.4's own, in both directions.
    let cell = document
        .lines()
        .find(|line| line.contains("private code 또는 lecture data 유출"))
        .ok_or("section 34.4 no longer holds the leak row")?;
    let recovery = cell
        .split('|')
        .map(str::trim)
        .nth(6)
        .ok_or("the leak row no longer has a recovery column")?;
    let stated: BTreeSet<&str> = recovery.split(", ").map(str::trim).collect();
    let claimed: BTreeSet<&str> = RecoveryStep::ALL
        .into_iter()
        .map(RecoveryStep::spec_words)
        .collect();
    assert_eq!(
        stated, claimed,
        "section 34.4's recovery steps and this crate's are not the same set"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// deletion_confirmation_is_non_delegable
// ---------------------------------------------------------------------------

/// Only a user confirms a deletion, and the refusal is exhaustive over `Actor`.
///
/// `P2-M2`'s `UserDecision::by` matches exhaustively over `academic-domain`'s
/// closed actor enum, so a fifth variant stops that crate compiling rather than
/// slipping past a negated list here. This drives every non-user variant, and
/// the `tests/compile_fail` cases hold the two ways round the door: a struct
/// literal, and a constructor that takes an actor without the receipt.
#[test]
fn deletion_confirmation_is_non_delegable() -> TestResult {
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let index = StatedIndex::all_empty("nothing derived from this synthetic subject");
    let dry_run = DeletionDryRun::of(subject, &index, &NothingProtected);
    let mut citations = EvidenceCitations::new();
    citations.cite(subject, digest("subject-bytes"));
    let preview = DeletionImpactPreview::of(dry_run, &EvidenceIndex::default(), &citations, 1_000)?;

    for actor in [
        Actor::ModelRun {
            run_id: entity(MODEL_RUN)?,
        },
        Actor::DeterministicEngine {
            name: "retention".to_owned(),
            version: "1".to_owned(),
        },
        Actor::Importer {
            name: "transcript-importer".to_owned(),
            version: "1".to_owned(),
        },
    ] {
        let refused = DeletionConfirmation::given(
            preview.clone(),
            &actor,
            preview.digest(),
            TimestampMillis::new(2_000),
        );
        match refused {
            Err(DeletionFlowError::AutomaticActor { actor: named }) => {
                assert_eq!(named, actor.kind_name());
            }
            other => {
                return Err(
                    format!("{} confirmed a deletion: {other:?}", actor.kind_name()).into(),
                );
            }
        }
    }

    let user = Actor::User {
        user_id: entity(DECIDING_USER)?,
    };
    let confirmation = DeletionConfirmation::given(
        preview.clone(),
        &user,
        preview.digest(),
        TimestampMillis::new(2_000),
    )?;
    assert_eq!(
        confirmation.decision().user_id(),
        u128::from_be_bytes(*entity(DECIDING_USER)?.as_bytes())
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// the deletion runs, end to end, over real files
// ---------------------------------------------------------------------------

/// The whole flow over a real cache file, a real replica file and a real
/// backup, ending in `COMPLETE` with nothing left.
///
/// It is the control for `deletion_faults.rs`: every fault row asserts a
/// deletion that did **not** complete, and a suite with no completing case
/// could pass while nothing ever worked.
#[test]
fn a_confirmed_deletion_reaches_every_class_and_completes() -> TestResult {
    let root = TestRoot::new("complete")?;
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let cache = target(SHARED_A, 0x33)?;
    let replica = target(SHARED_B, 0x33)?;
    let backup_target = target(SUBJECT_ARTIFACT, 0x44)?;

    let cache_path = touch(root.path(), "cache.bin")?;
    let replica_path = touch(root.path(), "replica.bin")?;
    let backup_root = root.path().join("backup");
    fs::create_dir_all(&backup_root)?;

    let mut index = StatedIndex::all_empty("nothing of this class derives from the subject");
    index
        .state(DerivativeClass::Cache, ClassTargets::Targets(vec![cache]))
        .state(
            DerivativeClass::Replica,
            ClassTargets::Targets(vec![replica]),
        )
        .state(
            DerivativeClass::BackupExpiry,
            ClassTargets::Targets(vec![backup_target]),
        );
    let receipt = run_flow(
        &root,
        subject,
        &index,
        paths_for(
            &[(cache, cache_path.clone()), (replica, replica_path.clone())],
            &[(backup_target, backup_root.clone())],
        ),
        &[subject, cache, replica, backup_target],
    )?;

    assert_eq!(receipt.outcome_word(), "COMPLETE");
    assert!(matches!(receipt.outcome(), RetentionOutcome::Complete));
    assert!(receipt.unresolved().is_empty());
    assert!(receipt.is_fully_erased());
    assert!(!cache_path.exists(), "the cache file is still there");
    assert!(!replica_path.exists(), "the replica file is still there");
    let stones = academic_retention::tombstone::read_from_backup(&backup_root)?;
    assert_eq!(stones.len(), 1);
    assert_eq!(stones[0].artifact_id, backup_target.artifact_hex());
    Ok(())
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// Runs the whole flow and returns the receipt.
fn run_flow(
    root: &TestRoot,
    subject: academic_deletion::DeletionTarget,
    index: &StatedIndex,
    paths: academic_deletion::DeletionPaths,
    cited: &[academic_deletion::DeletionTarget],
) -> Result<academic_deletion::ArtifactDeletionReceipt, Box<dyn Error>> {
    let dry_run = DeletionDryRun::of(subject, index, &NothingProtected);
    let mut citations = EvidenceCitations::new();
    for (position, target) in cited.iter().enumerate() {
        citations.cite(*target, digest(&format!("evidence-{position}")));
    }
    let preview = DeletionImpactPreview::of(dry_run, &EvidenceIndex::default(), &citations, 1_000)?;
    let user = Actor::User {
        user_id: entity(DECIDING_USER)?,
    };
    let shown = preview.digest();
    let confirmation =
        DeletionConfirmation::given(preview, &user, shown, TimestampMillis::new(2_000))?;
    let mut journal = AppendOnlyJournal::open(&root.path().join(DELETION_JOURNAL_RELATIVE_PATH))?;
    let mut shredder = RecordingShredder::default();
    let mut executor = FilesystemExecutor::new(
        &mut shredder,
        paths,
        "0102030405060708090a0b0c0d0e0f10".to_owned(),
        3_000,
    );
    Ok(execute_deletion(
        &mut journal,
        ActionId::from_bytes([0x51; 16]),
        &confirmation,
        &mut executor,
        ProviderErasureLog::new(),
    )?)
}

/// Settles one synthetic conflict through `P2-X7`'s own board.
///
/// The record is not built here: `ConflictCase::settle` is the only producer of
/// a `CorrectionRecord`, and it takes a `UserDecision`, so the correction this
/// incident is told about is one a user actually made through the correction
/// centre. That is the point — the strongest possible ordinary correction still
/// does not move the incident.
fn settle_a_conflict(
    outcome: CorrectionOutcome,
) -> Result<academic_evidence_center::CorrectionRecord, Box<dyn Error>> {
    let applies = academic_domain::ValidInterval::open_ended(TimestampMillis::new(0));
    let held = ConflictSide::new(
        ConflictLane::Held,
        "01900000-0000-7000-8000-0000000002f1".parse()?,
        academic_domain::EpistemicStatus::UserConfirmed,
        academic_domain::AuthorityClass::UserExplicit,
        TimestampMillis::new(500),
        applies,
        None,
    );
    let incoming = ConflictSide::new(
        ConflictLane::Incoming,
        "01900000-0000-7000-8000-0000000002f2".parse()?,
        academic_domain::EpistemicStatus::AiInferred,
        academic_domain::AuthorityClass::ModelInference,
        TimestampMillis::new(900),
        applies,
        None,
    );
    let mut case = ConflictCase::open(
        ConflictClass::OverrideVersusNewEvidence,
        held,
        incoming,
        TimestampMillis::new(1_000),
    );
    let user = Actor::User {
        user_id: entity(DECIDING_USER)?,
    };
    case.settle(outcome, user_receipt(&user)?, TimestampMillis::new(1_200));
    Ok(case
        .history()
        .last()
        .ok_or("the conflict board recorded no correction")?
        .clone())
}

fn domain_digest(value: &str) -> Result<academic_domain::ContentDigest, Box<dyn Error>> {
    let mut bytes = [0_u8; 32];
    hex_decode(value, &mut bytes)?;
    Ok(academic_domain::ContentDigest::from_sha256_bytes(bytes))
}

fn hex_decode(value: &str, out: &mut [u8; 32]) -> Result<(), Box<dyn Error>> {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() != 64 {
        return Err("a provider digest is not 32 bytes of hex".into());
    }
    for (index, pair) in chars.chunks(2).enumerate() {
        let high = pair[0].to_digit(16).ok_or("not hex")?;
        let low = pair[1].to_digit(16).ok_or("not hex")?;
        out[index] = u8::try_from(high * 16 + low)?;
    }
    Ok(())
}

// -- the broker fixture, adapted from `academic-policy`'s own suite ---------

const PAYLOAD: &[u8] = b"minimum";

fn policy_digest(label: &str) -> PolicyDigest {
    PolicyDigest::of(label.as_bytes())
}

fn provider_draft() -> Result<ProviderPolicyDraft, Box<dyn Error>> {
    Ok(ProviderPolicyDraft {
        identity: Some(ProviderIdentity::new(
            "synthetic-vendor",
            ProviderSurface::EnterpriseApi,
        )?),
        training_use_enabled: Some(false),
        training_opt_out_applied: Some(false),
        server_retention_millis: Some(0),
        abuse_logging_enabled: Some(false),
        residency_regions: Some(vec!["us-east".to_owned()]),
        subprocessors: Some(vec!["synthetic-subprocessor".to_owned()]),
        transit_encryption_declared: Some(true),
        at_rest_encryption_declared: Some(true),
        deletion_api_available: Some(true),
        deletion_receipt_capable: Some(true),
        maximum_input_bytes: Some(64),
        logging_configuration: Some("content-logging-disabled".to_owned()),
        policy_source_digest: Some(policy_digest("synthetic-provider-policy-source")),
        last_verified_at: Some(0),
        ttl_millis: Some(1_000),
    })
}

fn object_range() -> Result<ObjectRange, Box<dyn Error>> {
    Ok(ObjectRange::new(
        "synthetic-object",
        10,
        17,
        policy_digest("synthetic-slice"),
    )?)
}

fn rule(provider: &ProviderPolicySnapshot) -> Result<EgressRule, Box<dyn Error>> {
    Ok(EgressRule {
        actor_id: "synthetic-user".to_owned(),
        process_class: ProcessClass::EgressProxy,
        data_class: "synthetic-private-code".to_owned(),
        operation: "classify".to_owned(),
        purpose_id: "architecture-classification".to_owned(),
        destination_id: provider.destination_id().to_owned(),
        retention_terms_hash: provider.retention_terms_hash(),
        consent_evidence_id: "synthetic-consent".to_owned(),
        valid_from: 0,
        valid_until: 10_000,
        minimal_ranges: vec![object_range()?],
        payload_digest: PolicyDigest::of(PAYLOAD),
        provider_policy_snapshot_digest: provider.snapshot_digest().clone(),
        training_use_allowed: false,
        redaction_policy_hash: policy_digest("synthetic-redaction"),
    })
}

fn request(
    policy: academic_policy::PolicyVersion,
    provider: &ProviderPolicySnapshot,
    requested_at: u64,
) -> Result<PermissionRequest, Box<dyn Error>> {
    Ok(PermissionRequest {
        actor_id: Some("synthetic-user".to_owned()),
        process_class: ProcessClass::EgressProxy,
        data_class: Some("synthetic-private-code".to_owned()),
        object_range_digest_set: Some(vec![object_range()?]),
        operation: Some("classify".to_owned()),
        purpose_id: Some("architecture-classification".to_owned()),
        destination_id: Some(provider.destination_id().to_owned()),
        retention_terms_hash: Some(provider.retention_terms_hash()),
        requested_at: Some(requested_at),
        consent_evidence_id: Some("synthetic-consent".to_owned()),
        policy_version: Some(policy),
    })
}

fn runtime(provider: &ProviderPolicySnapshot) -> Result<RuntimeToolCall<'static>, Box<dyn Error>> {
    Ok(RuntimeToolCall::new(
        "synthetic-user",
        ProcessClass::EgressProxy,
        "classify",
        "architecture-classification",
        provider.destination_id(),
        vec![object_range()?],
        PAYLOAD,
    )?)
}

// `locator` and `SHARED_*` are used by the shared-locator rows above; the
// import list is checked by the compiler rather than by a comment.
#[allow(dead_code)]
fn unused_marker() -> [u8; 32] {
    locator(0)
}
