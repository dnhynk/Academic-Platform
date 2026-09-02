//! Named acceptance evidence for P2-N4 question graph contracts.

use std::{error::Error, num::NonZeroU64, str::FromStr as _};

use academic_domain::{
    Actor, ArtifactId, AuthorityClass, Claim, ClaimId, ClaimObject, ContentDigest, DomainError,
    EntityId, EpistemicStatus, EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole,
    EvidenceStrength, LogicalPath, PredicateId, ScopeId, TimestampMillis, ValidInterval,
    question::{
        AiExplanationPreference, ContextLocator, GeneratedExplanation, ObsolescenceReasonCode,
        PredeclaredQuestionValidation, QUESTION_LIFECYCLE_TRANSITIONS, QUESTION_RESOLUTION_OBJECT,
        QUESTION_RESOLUTION_PREDICATE, QUESTION_VALIDATION_APPROVAL_PREDICATE,
        QUESTION_WORKSPACE_REGION_ORDER, Question, QuestionDeferral, QuestionDeferralReason,
        QuestionError, QuestionImportance, QuestionObsolescence, QuestionOrigin,
        QuestionRelationKind, QuestionStatus, QuestionTransition, QuestionValidationCompletion,
        QuestionWorkspace, QuestionWorkspaceRegion, ReuseObservation, ReuseSummary, ReuseTarget,
        UncountedReuseReason, VerifiedQuestionResolution, lifecycle_transition_is_allowed,
        validate_lifecycle_definition,
    },
};

type TestResult = Result<(), Box<dyn Error>>;

fn entity(suffix: u32) -> Result<EntityId, DomainError> {
    EntityId::from_str(&format!("01920000-0000-7000-8000-{suffix:012x}"))
}

fn scope(suffix: u32) -> Result<ScopeId, DomainError> {
    ScopeId::from_str(&format!("01920000-0000-7000-8000-{suffix:012x}"))
}

fn evidence_id(suffix: u32) -> Result<EvidenceId, DomainError> {
    EvidenceId::from_str(&format!("01920000-0000-7000-8000-{suffix:012x}"))
}

fn artifact_id(suffix: u32) -> Result<ArtifactId, DomainError> {
    ArtifactId::from_str(&format!("01920000-0000-7000-8000-{suffix:012x}"))
}

fn claim_id(suffix: u32) -> Result<ClaimId, DomainError> {
    ClaimId::from_str(&format!("01920000-0000-7000-8000-{suffix:012x}"))
}

fn evidence(
    suffix: u32,
    artifact_suffix: u32,
    digest: ContentDigest,
) -> Result<EvidenceItem, DomainError> {
    Ok(EvidenceItem {
        id: evidence_id(suffix)?,
        artifact_id: artifact_id(artifact_suffix)?,
        locator: EvidenceLocator::Page { page_number: 1 },
        excerpt_digest: digest,
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "synthetic-question-fixture".to_owned(),
        extractor_version: "1".to_owned(),
    })
}

fn claim(
    suffix: u32,
    question_id: EntityId,
    scope_id: ScopeId,
    predicate: &str,
    evidence_ids: Vec<EvidenceId>,
    authority_and_status: (AuthorityClass, EpistemicStatus),
    at: i64,
) -> Result<Claim, DomainError> {
    Ok(Claim {
        id: claim_id(suffix)?,
        subject_entity_id: question_id,
        predicate_id: PredicateId::parse(predicate)?,
        object: ClaimObject::Text(QUESTION_RESOLUTION_OBJECT.to_owned()),
        scope_id,
        authority_class: authority_and_status.0,
        epistemic_status: authority_and_status.1,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(at)),
        evidence_ids,
    })
}

fn lecture_question(suffix: u32) -> Result<Question, QuestionError> {
    Question::new(
        entity(suffix)?,
        scope(1)?,
        "Why does a B+ Tree improve fan-out?",
        TimestampMillis::new(10),
        QuestionOrigin::Lecture {
            entity: entity(500)?,
            locator: ContextLocator::parse("audio@42:18")?,
        },
        [entity(600)?, entity(601)?],
        QuestionImportance::UserSet,
    )
}

#[test]
fn question_schema_round_trip() -> TestResult {
    let question = lecture_question(1)?.revise(
        "Why does B+ Tree fan-out reduce random I/O?",
        TimestampMillis::new(11),
    )?;
    let json = serde_json::to_value(&question)?;
    assert_eq!(json["createdAt"], 10);
    assert_eq!(json["origin"]["type"], "LECTURE");
    assert_eq!(json["status"], "OPEN");
    assert_eq!(json["resolutionDecision"], serde_json::Value::Null);
    assert_eq!(
        json["revisions"][0]["previousText"],
        "Why does a B+ Tree improve fan-out?"
    );
    assert_eq!(serde_json::from_value::<Question>(json)?, question);
    Ok(())
}

#[test]
fn repo_origin_requires_snapshot_path_line() -> TestResult {
    let question = Question::new(
        entity(2)?,
        scope(1)?,
        "Where is retry ownership fixed?",
        TimestampMillis::new(10),
        QuestionOrigin::Repository {
            entity: entity(501)?,
            snapshot: ContentDigest::sha256(b"synthetic repository snapshot"),
            path: LogicalPath::parse("src/retry.rs")?,
            line: NonZeroU64::new(42).ok_or("line fixture must be non-zero")?,
        },
        [entity(602)?],
        QuestionImportance::ContextDerived,
    )?;
    let clean = serde_json::to_value(question)?;
    assert!(serde_json::from_value::<Question>(clean.clone()).is_ok());

    for required in ["snapshot", "path", "line"] {
        let mut injected = clean.clone();
        injected["origin"]
            .as_object_mut()
            .ok_or("origin fixture must be an object")?
            .remove(required);
        assert!(
            serde_json::from_value::<Question>(injected).is_err(),
            "repository origin admitted after removing {required}"
        );
    }

    let mut zero_line = clean.clone();
    zero_line["origin"]["line"] = serde_json::json!(0);
    assert!(serde_json::from_value::<Question>(zero_line).is_err());
    let mut absolute_path = clean;
    absolute_path["origin"]["path"] = serde_json::json!("C:/private/retry.rs");
    assert!(serde_json::from_value::<Question>(absolute_path).is_err());
    Ok(())
}

#[test]
fn lifecycle_transition_table_rejects_every_non_edge() -> TestResult {
    let expected_status_names = [
        "OPEN",
        "PARTIALLY_RESOLVED",
        "RESOLVED",
        "REFRAMED",
        "OBSOLETE",
        "REOPENED",
    ];
    assert_eq!(
        QuestionStatus::ALL.map(QuestionStatus::as_str),
        expected_status_names
    );
    let status_source = include_str!("../src/question.rs");
    let (_, after_status_start) = status_source
        .split_once("// QUESTION_STATUS_SCHEMA_BEGIN")
        .ok_or("status schema start marker missing")?;
    let (status_schema, _) = after_status_start
        .split_once("// QUESTION_STATUS_SCHEMA_END")
        .ok_or("status schema end marker missing")?;
    let variants = status_schema
        .split_once("pub enum QuestionStatus {")
        .ok_or("status enum declaration missing")?
        .1
        .split_once('}')
        .ok_or("status enum closing brace missing")?
        .0
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_suffix(','))
        .collect::<Vec<_>>();
    assert_eq!(
        variants,
        [
            "Open",
            "PartiallyResolved",
            "Resolved",
            "Reframed",
            "Obsolete",
            "Reopened",
        ],
        "adding or removing a status must fail the source-backed enumeration"
    );

    let expected_transitions = [
        QuestionTransition {
            from: QuestionStatus::Open,
            to: QuestionStatus::PartiallyResolved,
        },
        QuestionTransition {
            from: QuestionStatus::Open,
            to: QuestionStatus::Reframed,
        },
        QuestionTransition {
            from: QuestionStatus::Open,
            to: QuestionStatus::Obsolete,
        },
        QuestionTransition {
            from: QuestionStatus::PartiallyResolved,
            to: QuestionStatus::Resolved,
        },
        QuestionTransition {
            from: QuestionStatus::PartiallyResolved,
            to: QuestionStatus::Reframed,
        },
        QuestionTransition {
            from: QuestionStatus::Resolved,
            to: QuestionStatus::Reopened,
        },
        QuestionTransition {
            from: QuestionStatus::Resolved,
            to: QuestionStatus::Reframed,
        },
    ];
    assert_eq!(QUESTION_LIFECYCLE_TRANSITIONS, expected_transitions);
    validate_lifecycle_definition(&QuestionStatus::ALL, &QUESTION_LIFECYCLE_TRANSITIONS)?;

    for omitted in QuestionStatus::ALL {
        let injection = QuestionStatus::ALL
            .into_iter()
            .filter(|status| *status != omitted)
            .collect::<Vec<_>>();
        assert_eq!(
            validate_lifecycle_definition(&injection, &QUESTION_LIFECYCLE_TRANSITIONS),
            Err(QuestionError::InvalidLifecycleDefinition),
            "omitting status {omitted:?} did not trip the definition guard"
        );
    }
    let mut extra_status = QuestionStatus::ALL.to_vec();
    extra_status.push(QuestionStatus::Open);
    assert_eq!(
        validate_lifecycle_definition(&extra_status, &QUESTION_LIFECYCLE_TRANSITIONS),
        Err(QuestionError::InvalidLifecycleDefinition)
    );

    for allowed in QUESTION_LIFECYCLE_TRANSITIONS {
        let injection = QUESTION_LIFECYCLE_TRANSITIONS
            .into_iter()
            .filter(|edge| *edge != allowed)
            .collect::<Vec<_>>();
        assert_eq!(
            validate_lifecycle_definition(&QuestionStatus::ALL, &injection),
            Err(QuestionError::InvalidLifecycleDefinition),
            "removing allowed edge {allowed:?} did not trip the table guard"
        );
    }

    for from in QuestionStatus::ALL {
        for to in QuestionStatus::ALL {
            if from == to {
                continue;
            }
            let candidate = QuestionTransition { from, to };
            let specified = expected_transitions.contains(&candidate);
            assert_eq!(lifecycle_transition_is_allowed(from, to), specified);
            if !specified {
                let mut injection = QUESTION_LIFECYCLE_TRANSITIONS.to_vec();
                injection.push(candidate);
                assert_eq!(
                    validate_lifecycle_definition(&QuestionStatus::ALL, &injection),
                    Err(QuestionError::InvalidLifecycleDefinition),
                    "non-edge {candidate:?} passed after explicit admission injection"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn ai_proposal_leaves_status_unchanged() -> TestResult {
    let question = lecture_question(3)?;
    let explanation = GeneratedExplanation::new(
        artifact_id(700)?,
        question.id(),
        entity(701)?,
        TimestampMillis::new(12),
    );
    let before = serde_json::to_value(&question)?;
    let candidate = question.propose_resolution(explanation, [evidence_id(702)?])?;
    assert_eq!(candidate.question_id(), question.id());
    assert_eq!(question.status(), QuestionStatus::Open);
    assert_eq!(serde_json::to_value(&question)?, before);
    Ok(())
}

#[test]
fn resolution_requires_user_decision() -> TestResult {
    let partial =
        lecture_question(4)?.partially_resolve(TimestampMillis::new(12), [evidence_id(710)?])?;
    let decision_evidence = evidence(711, 711, ContentDigest::sha256(b"synthetic answer"))?;
    let forged_user_claim = claim(
        711,
        partial.id(),
        partial.scope_id(),
        QUESTION_RESOLUTION_PREDICATE,
        vec![decision_evidence.id],
        (AuthorityClass::UserExplicit, EpistemicStatus::UserConfirmed),
        13,
    )?;
    let automatic_actors = [
        Actor::ModelRun {
            run_id: entity(712)?,
        },
        Actor::DeterministicEngine {
            name: "synthetic-engine".to_owned(),
            version: "1".to_owned(),
        },
        Actor::Importer {
            name: "synthetic-importer".to_owned(),
            version: "1".to_owned(),
        },
    ];
    for actor in &automatic_actors {
        assert!(matches!(
            VerifiedQuestionResolution::user_decision(
                &partial,
                actor,
                &forged_user_claim,
                &decision_evidence,
            ),
            Err(QuestionError::Domain(
                DomainError::ActorAuthorityMismatch { .. }
            ))
        ));
    }

    let automatic_pairs = [
        (AuthorityClass::ModelInference, EpistemicStatus::AiInferred),
        (
            AuthorityClass::DeterministicEngine,
            EpistemicStatus::DeterministicDerived,
        ),
        (
            AuthorityClass::Curated,
            EpistemicStatus::DeterministicDerived,
        ),
    ];
    for (index, (actor, (authority, status))) in
        automatic_actors.iter().zip(automatic_pairs).enumerate()
    {
        let native_claim = claim(
            720 + u32::try_from(index)?,
            partial.id(),
            partial.scope_id(),
            QUESTION_RESOLUTION_PREDICATE,
            vec![decision_evidence.id],
            (authority, status),
            13,
        )?;
        assert_eq!(
            VerifiedQuestionResolution::user_decision(
                &partial,
                actor,
                &native_claim,
                &decision_evidence,
            )
            .map(|_| ()),
            Err(QuestionError::InvalidResolutionAction)
        );
    }
    assert_eq!(partial.status(), QuestionStatus::PartiallyResolved);

    let user = Actor::User {
        user_id: entity(730)?,
    };
    let approval = VerifiedQuestionResolution::user_decision(
        &partial,
        &user,
        &forged_user_claim,
        &decision_evidence,
    )?;
    let resolved = partial.resolve(approval, TimestampMillis::new(14))?;
    assert_eq!(resolved.status(), QuestionStatus::Resolved);
    let mut forged_persisted_resolution = serde_json::to_value(&resolved)?;
    forged_persisted_resolution["resolutionDecision"]["actor"] = serde_json::json!({
        "kind": "MODEL_RUN",
        "run_id": entity(731)?.to_string(),
    });
    assert!(
        serde_json::from_value::<Question>(forged_persisted_resolution).is_err(),
        "deserialization must repeat the actor/authority/status validation"
    );

    let validation_question =
        lecture_question(5)?.partially_resolve(TimestampMillis::new(20), [evidence_id(740)?])?;
    let expected_result = ContentDigest::sha256(b"synthetic validation result");
    let declaration = PredeclaredQuestionValidation::new(
        entity(741)?,
        validation_question.id(),
        validation_question.scope_id(),
        expected_result,
        TimestampMillis::new(21),
    );
    let completion_evidence = evidence(742, 742, expected_result)?;
    let completion = QuestionValidationCompletion::new(
        declaration,
        TimestampMillis::new(22),
        completion_evidence.clone(),
    )?;
    assert_eq!(
        validation_question.status(),
        QuestionStatus::PartiallyResolved,
        "mechanical completion alone leaves the question partial"
    );
    let approval_evidence = evidence(743, 743, ContentDigest::sha256(b"synthetic approval"))?;
    let validation_claim = claim(
        743,
        validation_question.id(),
        validation_question.scope_id(),
        QUESTION_VALIDATION_APPROVAL_PREDICATE,
        vec![approval_evidence.id, completion_evidence.id],
        (AuthorityClass::UserExplicit, EpistemicStatus::UserConfirmed),
        23,
    )?;
    let validation_approval = VerifiedQuestionResolution::validated_then_approved(
        &validation_question,
        completion,
        &user,
        &validation_claim,
        &approval_evidence,
    )?;
    let validated = validation_question.resolve(validation_approval, TimestampMillis::new(24))?;
    assert_eq!(validated.status(), QuestionStatus::Resolved);
    Ok(())
}

#[test]
fn obsolete_requires_reason_and_evidence() -> TestResult {
    assert_eq!(
        QuestionObsolescence::new(ObsolescenceReasonCode::FalsePremise, []),
        Err(QuestionError::ObsolescenceEvidenceMissing)
    );

    let question = lecture_question(6)?;
    let deferral = QuestionDeferral::new(
        QuestionDeferralReason::AttentionUnavailable,
        TimestampMillis::new(11),
    );
    assert_eq!(
        deferral.reason(),
        QuestionDeferralReason::AttentionUnavailable
    );
    assert_eq!(question.status(), QuestionStatus::Open);

    let mut injected = serde_json::to_value(&question)?;
    injected["status"] = serde_json::json!("OBSOLETE");
    assert!(serde_json::from_value::<Question>(injected).is_err());

    let record = QuestionObsolescence::new(
        ObsolescenceReasonCode::TechnologyChanged,
        [evidence_id(750)?],
    )?;
    let obsolete = question.mark_obsolete(record, TimestampMillis::new(12))?;
    assert_eq!(obsolete.status(), QuestionStatus::Obsolete);
    assert_eq!(obsolete.lifecycle()[0].evidence_ids(), [evidence_id(750)?]);
    let mut avoidance_injection = serde_json::to_value(obsolete)?;
    avoidance_injection["obsolescence"]["reason"] = serde_json::json!("NOT_NOW");
    assert!(
        serde_json::from_value::<Question>(avoidance_injection).is_err(),
        "a deferral reason must not deserialize as obsolescence"
    );
    Ok(())
}

#[test]
fn reframe_preserves_old_text_and_links() -> TestResult {
    let original = lecture_question(7)?;
    let old_text = original.canonical_text().to_owned();
    let old_id = original.id();
    let replacement_id = entity(8)?;
    let reframe = original.reframe(
        replacement_id,
        "Under which workloads does B+ Tree fan-out reduce random I/O?",
        TimestampMillis::new(12),
    )?;
    assert_eq!(reframe.original().id(), old_id);
    assert_eq!(reframe.original().canonical_text(), old_text);
    assert_eq!(reframe.original().status(), QuestionStatus::Reframed);
    assert_eq!(reframe.replacement().id(), replacement_id);
    assert_eq!(reframe.replacement().status(), QuestionStatus::Open);
    assert_eq!(reframe.relation().from(), old_id);
    assert_eq!(reframe.relation().to(), replacement_id);
    assert_eq!(reframe.relation().kind(), QuestionRelationKind::ReframedAs);
    assert_eq!(
        lecture_question(9)?
            .reframe(entity(9)?, "A different wording", TimestampMillis::new(12),)
            .map(|_| ()),
        Err(QuestionError::ReframeIdentityReused)
    );
    Ok(())
}

#[test]
fn workspace_region_order_is_exact() {
    assert_eq!(
        QUESTION_WORKSPACE_REGION_ORDER,
        [
            QuestionWorkspaceRegion::OriginContext,
            QuestionWorkspaceRegion::ConceptsAndPrerequisites,
            QuestionWorkspaceRegion::RelevantEvidence,
            QuestionWorkspaceRegion::RecurrenceLocations,
            QuestionWorkspaceRegion::ResolutionSources,
            QuestionWorkspaceRegion::AiExplanation,
        ]
    );
}

#[test]
fn ai_explanation_is_region_six_and_opt_in() -> TestResult {
    let question = lecture_question(10)?;
    let explanation = GeneratedExplanation::new(
        artifact_id(760)?,
        question.id(),
        entity(761)?,
        TimestampMillis::new(12),
    );
    assert_eq!(
        QuestionWorkspace::new(
            question.id(),
            AiExplanationPreference::Hidden,
            Some(explanation.clone()),
        ),
        Err(QuestionError::AiExplanationNotRequested)
    );
    let hidden = QuestionWorkspace::new(question.id(), AiExplanationPreference::Hidden, None)?;
    assert_eq!(hidden.regions()[5], QuestionWorkspaceRegion::AiExplanation);
    assert!(hidden.ai_explanation().is_none());

    let requested = QuestionWorkspace::new(
        question.id(),
        AiExplanationPreference::Requested,
        Some(explanation.clone()),
    )?;
    assert_eq!(requested.ai_explanation(), Some(&explanation));

    let explanation_evidence = evidence(
        762,
        760,
        ContentDigest::sha256(b"synthetic generated explanation"),
    )?;
    assert!(explanation.is_resolution_evidence(&explanation_evidence));
    let _candidate = question.propose_resolution(explanation, [explanation_evidence.id])?;
    assert_eq!(
        question.status(),
        QuestionStatus::Open,
        "the generated artifact is evidence-capable but its proposal does not close the question"
    );

    let partial = question
        .clone()
        .partially_resolve(TimestampMillis::new(13), [evidence_id(763)?])?;
    let user = Actor::User {
        user_id: entity(764)?,
    };
    let decision = claim(
        765,
        partial.id(),
        partial.scope_id(),
        QUESTION_RESOLUTION_PREDICATE,
        vec![explanation_evidence.id],
        (AuthorityClass::UserExplicit, EpistemicStatus::UserConfirmed),
        14,
    )?;
    let verified = VerifiedQuestionResolution::user_decision(
        &partial,
        &user,
        &decision,
        &explanation_evidence,
    )?;
    assert_eq!(
        partial
            .resolve(verified, TimestampMillis::new(15))?
            .status(),
        QuestionStatus::Resolved,
        "generated material becomes resolution evidence only with the user's decision"
    );
    Ok(())
}

#[test]
fn growth_descriptors_contain_no_scalar_score() -> TestResult {
    fn descriptor_schema(source: &str) -> Result<&str, Box<dyn Error>> {
        let (_, after_start) = source
            .split_once("// QUESTION_GROWTH_SCHEMA_BEGIN")
            .ok_or("growth schema start marker missing")?;
        let (schema, _) = after_start
            .split_once("// QUESTION_GROWTH_SCHEMA_END")
            .ok_or("growth schema end marker missing")?;
        Ok(schema)
    }

    fn scalar_field_is_present(schema: &str) -> bool {
        schema
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .filter(|identifier| !identifier.is_empty())
            .any(|identifier| {
                let identifier = identifier.to_ascii_lowercase();
                identifier.contains("difficulty")
                    || identifier
                        .split('_')
                        .any(|part| matches!(part, "score" | "rating" | "rank" | "percentile"))
            })
    }

    let source = include_str!("../src/question.rs");
    let schema = descriptor_schema(source)?;
    assert!(!scalar_field_is_present(schema));

    let injected = schema.replace(
        "pub target_scope: QuestionTargetScope,",
        "pub difficulty_score: u32,\n    pub target_scope: QuestionTargetScope,",
    );
    assert_ne!(
        injected, schema,
        "mutation injection did not alter the schema"
    );
    assert!(
        scalar_field_is_present(&injected),
        "the schema scan missed an injected scalar field"
    );
    Ok(())
}

#[test]
fn reuse_count_deduplicates() -> TestResult {
    let shared_id = entity(770)?;
    let another_project = entity(771)?;
    let summary = ReuseSummary::from_observations([
        ReuseObservation::Identified(ReuseTarget::Project { id: shared_id }),
        ReuseObservation::Identified(ReuseTarget::Project { id: shared_id }),
        ReuseObservation::Identified(ReuseTarget::Concept { id: shared_id }),
        ReuseObservation::Identified(ReuseTarget::Project {
            id: another_project,
        }),
        ReuseObservation::Uncounted(UncountedReuseReason::TargetIdentityMissing),
        ReuseObservation::Uncounted(UncountedReuseReason::TargetKindUnresolved),
    ]);
    assert_eq!(summary.reuse_count(), 3);
    assert_eq!(summary.targets().len(), 3);
    assert_eq!(summary.uncounted_reasons().len(), 2);
    assert!(
        summary
            .uncounted_reasons()
            .contains(&UncountedReuseReason::TargetIdentityMissing)
    );
    Ok(())
}
