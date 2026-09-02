//! Named acceptance evidence for P2-N1 ontology import and curation.

use std::str::FromStr as _;

use academic_domain::{
    Actor, ArtifactId, AuthorityClass, Claim, ClaimId, ClaimObject, ContentDigest, DomainError,
    EntityId, EpistemicStatus, EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole,
    EvidenceStrength, PredicateId, ScopeId, TimestampMillis, ValidInterval,
    entity_registry::{EntityKind, OntologyChangeProposal, OntologyImpactSnapshot},
    ontology::{
        BaseTaxonomyMix, CURATOR_APPROVAL_OBJECT, CURATOR_APPROVAL_PREDICATE, Concept,
        ConceptPromotion, ConceptPromotionCriterion, ConceptPromotionGate, Field,
        GRANULARITY_EXAMPLES, GranularityStatus, Mention, OntologyChangeReview, OntologyError,
        OntologyMetricName, OntologyMetricObservation, OntologyMetricValue, OntologyQualityMetrics,
        Operation, PromotionAbstention, TaxonomyMixSelection, TaxonomyNode, TaxonomySource,
        TaxonomyVersionIdentity, VerifiedCuratorApproval, VersionedTaxonomyImport,
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn entity(suffix: u32) -> Result<EntityId, DomainError> {
    EntityId::from_str(&format!("01910000-0000-7000-8000-{suffix:012x}"))
}

fn evidence_id(suffix: u32) -> Result<EvidenceId, DomainError> {
    EvidenceId::from_str(&format!("01910000-0000-7000-8000-{suffix:012x}"))
}

fn artifact_id(suffix: u32) -> Result<ArtifactId, DomainError> {
    ArtifactId::from_str(&format!("01910000-0000-7000-8000-{suffix:012x}"))
}

fn claim_id(suffix: u32) -> Result<ClaimId, DomainError> {
    ClaimId::from_str(&format!("01910000-0000-7000-8000-{suffix:012x}"))
}

fn scope_id(suffix: u32) -> Result<ScopeId, DomainError> {
    ScopeId::from_str(&format!("01910000-0000-7000-8000-{suffix:012x}"))
}

fn interval() -> Result<ValidInterval, DomainError> {
    ValidInterval::new(TimestampMillis::new(1_700_000_000_000), None)
}

fn example_nodes() -> Result<Vec<TaxonomyNode>, OntologyError> {
    Ok(vec![
        TaxonomyNode::Field(Field::new(entity(1)?, "Database Systems")?),
        TaxonomyNode::Concept(Concept::new(entity(2)?, "Serializability", entity(1)?)?),
        TaxonomyNode::Concept(Concept::new(entity(3)?, "B+ Tree", entity(1)?)?),
        TaxonomyNode::Operation(Operation::new(
            entity(4)?,
            "B+ Tree node split",
            entity(3)?,
        )?),
    ])
}

fn taxonomy(release: &str) -> Result<VersionedTaxonomyImport, OntologyError> {
    VersionedTaxonomyImport::from_nodes(
        entity(100)?,
        TaxonomySource::Curriculum,
        release,
        example_nodes()?,
    )
}

fn repeated_candidate() -> Result<academic_domain::ontology::ConceptCandidate, OntologyError> {
    let mention = Mention::new(
        "Serializability",
        [evidence_id(1)?, evidence_id(2)?],
        [ConceptPromotionCriterion::IndependentExplanation(
            evidence_id(3)?,
        )],
    )?;
    match ConceptPromotionGate::evaluate(&mention, entity(20)?, entity(1)?, scope_id(1)?)? {
        ConceptPromotion::GranularityUnderReview(candidate) => Ok(candidate),
        ConceptPromotion::Mention { .. } => Err(OntologyError::EmptyValue("concept candidate")),
    }
}

fn approval_material(
    subject: EntityId,
    review_digest: ContentDigest,
    suffix: u32,
    authority_class: AuthorityClass,
    epistemic_status: EpistemicStatus,
) -> Result<(Claim, EvidenceItem), DomainError> {
    let evidence = EvidenceItem {
        id: evidence_id(suffix)?,
        artifact_id: artifact_id(suffix)?,
        locator: EvidenceLocator::Page { page_number: 1 },
        excerpt_digest: review_digest,
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "synthetic-curator-preview".to_owned(),
        extractor_version: "1".to_owned(),
    };
    let claim = Claim {
        id: claim_id(suffix)?,
        subject_entity_id: subject,
        predicate_id: PredicateId::parse(CURATOR_APPROVAL_PREDICATE)?,
        object: ClaimObject::Text(CURATOR_APPROVAL_OBJECT.to_owned()),
        scope_id: scope_id(1)?,
        authority_class,
        epistemic_status,
        confidence: None,
        prediction_metadata: None,
        valid_time: interval()?,
        evidence_ids: vec![evidence.id],
    };
    Ok((claim, evidence))
}

#[test]
fn field_concept_separation() -> TestResult {
    let imported = taxonomy("curriculum-2026.1")?;
    assert_eq!(imported.nodes().len(), 4);
    assert_eq!(imported.nodes()[0].entity_kind(), EntityKind::Field);
    assert_eq!(imported.nodes()[1].entity_kind(), EntityKind::Concept);
    assert_eq!(imported.nodes()[3].entity_kind(), EntityKind::Operation);

    let wrong_concept_parent = vec![TaxonomyNode::Concept(Concept::new(
        entity(30)?,
        "Orphan concept",
        entity(999)?,
    )?)];
    assert!(matches!(
        VersionedTaxonomyImport::from_nodes(
            entity(101)?,
            TaxonomySource::Acm,
            "2026",
            wrong_concept_parent,
        ),
        Err(OntologyError::InvalidParent {
            child_kind: "CONCEPT",
            parent_kind: "FIELD",
            ..
        })
    ));

    let wrong_operation_parent = vec![
        TaxonomyNode::Field(Field::new(entity(40)?, "Database Systems")?),
        TaxonomyNode::Operation(Operation::new(
            entity(41)?,
            "B+ Tree node split",
            entity(40)?,
        )?),
    ];
    assert!(matches!(
        VersionedTaxonomyImport::from_nodes(
            entity(102)?,
            TaxonomySource::UserDerived,
            "user-v1",
            wrong_operation_parent,
        ),
        Err(OntologyError::InvalidParent {
            child_kind: "OPERATION",
            parent_kind: "CONCEPT",
            ..
        })
    ));
    Ok(())
}

#[test]
fn concept_granularity_gate() -> TestResult {
    let criteria = [
        ConceptPromotionCriterion::IndependentExplanation(evidence_id(10)?),
        ConceptPromotionCriterion::Question(entity(11)?),
        ConceptPromotionCriterion::Evidence(evidence_id(12)?),
        ConceptPromotionCriterion::Prerequisite(entity(13)?),
    ];
    for (index, criterion) in criteria.into_iter().enumerate() {
        let mention = Mention::new(
            "Serializability",
            [evidence_id(20)?, evidence_id(21)?],
            [criterion],
        )?;
        let outcome = ConceptPromotionGate::evaluate(
            &mention,
            entity(100 + u32::try_from(index)?)?,
            entity(1)?,
            scope_id(1)?,
        )?;
        assert_eq!(
            outcome.status().as_str(),
            "GRANULARITY_UNDER_REVIEW",
            "each one-of-four criterion independently admits review"
        );
        let ConceptPromotion::GranularityUnderReview(candidate) = outcome else {
            return Err("eligible mention did not enter review".into());
        };
        assert_eq!(candidate.criteria().len(), 1);
    }

    let unsupported = Mention::new(
        "Repeated noun phrase",
        [evidence_id(30)?, evidence_id(31)?],
        [],
    )?;
    assert_eq!(
        ConceptPromotionGate::evaluate(&unsupported, entity(120)?, entity(1)?, scope_id(1)?)?,
        ConceptPromotion::Mention {
            reason: PromotionAbstention::MissingIndependentAttachment,
        }
    );
    Ok(())
}

#[test]
fn single_mention_abstention() -> TestResult {
    let strongest_single = Mention::new(
        "one-off-detail",
        [evidence_id(40)?],
        [
            ConceptPromotionCriterion::IndependentExplanation(evidence_id(41)?),
            ConceptPromotionCriterion::Question(entity(42)?),
            ConceptPromotionCriterion::Evidence(evidence_id(43)?),
            ConceptPromotionCriterion::Prerequisite(entity(44)?),
        ],
    )?;
    let outcome =
        ConceptPromotionGate::evaluate(&strongest_single, entity(130)?, entity(1)?, scope_id(1)?)?;
    assert_eq!(outcome.status(), GranularityStatus::Mention);
    assert_eq!(
        outcome,
        ConceptPromotion::Mention {
            reason: PromotionAbstention::SingleOccurrence,
        },
        "even all four attachments cannot turn one occurrence into a concept"
    );
    Ok(())
}

#[test]
fn granularity_examples_contract() -> TestResult {
    let examples = GRANULARITY_EXAMPLES;
    assert_eq!(examples[0].label, "Database Systems");
    assert_eq!(examples[0].kind, EntityKind::Field);
    assert_eq!(examples[0].parent_label, None);
    assert_eq!(examples[1].label, "Serializability");
    assert_eq!(examples[1].kind, EntityKind::Concept);
    assert_eq!(examples[1].parent_label, Some("Database Systems"));
    assert_eq!(examples[2].label, "B+ Tree");
    assert_eq!(examples[2].kind, EntityKind::Concept);
    assert_eq!(examples[3].label, "B+ Tree node split");
    assert_eq!(examples[3].kind, EntityKind::Operation);
    assert_eq!(examples[3].parent_label, Some("B+ Tree"));
    Ok(())
}

#[test]
fn ontology_change_preview_gate() -> TestResult {
    let imported = taxonomy("curriculum-2026.1")?;
    let source = entity(2)?;
    let target = entity(3)?;
    let mut snapshot = OntologyImpactSnapshot::default();
    snapshot.states.insert(source, 2);
    snapshot.states.insert(target, 3);
    snapshot.edges.insert(source, 5);
    snapshot.edges.insert(target, 7);
    snapshot.questions.insert(source, 11);
    snapshot.questions.insert(target, 13);
    snapshot.evidence.insert(source, [evidence_id(100)?].into());
    snapshot
        .evidence
        .insert(target, [evidence_id(101)?, evidence_id(102)?].into());

    let proposal = OntologyChangeProposal::Merge { source, target };
    let review = OntologyChangeReview::new(
        imported.identity().clone(),
        scope_id(1)?,
        proposal.clone(),
        &snapshot,
    )?;
    assert_eq!(review.status(), GranularityStatus::GranularityUnderReview);
    assert_eq!(review.preview().impact().state_count, 5);
    assert_eq!(review.preview().impact().edge_count, 12);
    assert_eq!(review.preview().impact().question_count, 24);
    assert_eq!(review.preview().impact().evidence_count, 3);

    // Injection: a digest of the unversioned registry preview is not enough.
    let (unversioned_claim, unversioned_evidence) = approval_material(
        source,
        review.preview().impact().digest(),
        110,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
    )?;
    let user = Actor::User {
        user_id: entity(900)?,
    };
    assert!(matches!(
        VerifiedCuratorApproval::for_change_review(
            &user,
            &unversioned_claim,
            &unversioned_evidence,
            &review,
        ),
        Err(OntologyError::ApprovalReviewMismatch)
    ));

    // Injection one layer out: old counts cannot approve a freshly recomputed preview.
    let old_digest = review.preview().digest();
    snapshot.states.insert(target, 4);
    let changed_review = OntologyChangeReview::new(
        imported.identity().clone(),
        scope_id(1)?,
        proposal.clone(),
        &snapshot,
    )?;
    let (stale_claim, stale_evidence) = approval_material(
        source,
        old_digest,
        111,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
    )?;
    assert!(matches!(
        VerifiedCuratorApproval::for_change_review(
            &user,
            &stale_claim,
            &stale_evidence,
            &changed_review,
        ),
        Err(OntologyError::ApprovalReviewMismatch)
    ));

    // The same counts under another taxonomy release also need a new approval.
    let other_version = TaxonomyVersionIdentity::new(
        imported.identity().taxonomy_id(),
        imported.identity().source(),
        "curriculum-2026.2",
        imported.identity().content_digest(),
    )?;
    let other_review =
        OntologyChangeReview::new(other_version, scope_id(1)?, proposal.clone(), &snapshot)?;
    assert_ne!(
        changed_review.preview().digest(),
        other_review.preview().digest()
    );
    let (cross_version_claim, cross_version_evidence) = approval_material(
        source,
        changed_review.preview().digest(),
        113,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
    )?;
    assert!(matches!(
        VerifiedCuratorApproval::for_change_review(
            &user,
            &cross_version_claim,
            &cross_version_evidence,
            &other_review,
        ),
        Err(OntologyError::ApprovalReviewMismatch)
    ));

    // Revert to an exact, current preview digest and observe admission.
    let (claim, evidence) = approval_material(
        source,
        changed_review.preview().digest(),
        112,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
    )?;
    let approval =
        VerifiedCuratorApproval::for_change_review(&user, &claim, &evidence, &changed_review)?;
    let approved = changed_review.approve(approval)?;
    assert_eq!(approved.status(), GranularityStatus::Curated);
    assert_eq!(approved.approved_by(), entity(900)?);
    Ok(())
}

#[test]
fn orphan_and_near_duplicate_metrics_do_not_expose_content() -> TestResult {
    let clean = || {
        [
            OntologyMetricObservation {
                name: OntologyMetricName::OrphanCount,
                value: OntologyMetricValue::Count(7),
            },
            OntologyMetricObservation {
                name: OntologyMetricName::NearDuplicatePairCount,
                value: OntologyMetricValue::Count(4),
            },
        ]
    };
    let metrics = OntologyQualityMetrics::observe(clean())?;
    assert_eq!(metrics.orphan_count(), 7);
    assert_eq!(metrics.near_duplicate_pair_count(), 4);
    assert_eq!(
        serde_json::to_value(metrics)?,
        serde_json::json!({"orphan_count": 7, "near_duplicate_pair_count": 4})
    );

    for metric in [
        OntologyMetricName::OrphanCount,
        OntologyMetricName::NearDuplicatePairCount,
    ] {
        let secret = "synthetic-sensitive-concept-label";
        let injection = OntologyMetricObservation {
            name: metric,
            value: OntologyMetricValue::Content(secret.to_owned()),
        };
        assert!(
            !format!("{injection:?}").contains(secret),
            "the observation's debug path must redact injected content"
        );
        let error = match OntologyQualityMetrics::observe([injection]) {
            Ok(_) => return Err("content injection passed the metrics guard".into()),
            Err(error) => error,
        };
        assert!(matches!(error, OntologyError::MetricContentForbidden(_)));
        assert!(
            !error.to_string().contains(secret),
            "the rejection error must not become a content leak"
        );
    }

    let restored = OntologyQualityMetrics::observe(clean())?;
    assert_eq!(
        restored, metrics,
        "reverting the injection restores admission"
    );
    Ok(())
}

#[test]
fn curator_approval_is_a_non_delegable_user_action() -> TestResult {
    let candidate = repeated_candidate()?;
    let digest = candidate.review_digest();
    let (forged_user_claim, evidence) = approval_material(
        candidate.concept().id(),
        digest,
        200,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
    )?;
    let automatic_actors = [
        Actor::DeterministicEngine {
            name: "synthetic-engine".to_owned(),
            version: "1".to_owned(),
        },
        Actor::ModelRun {
            run_id: entity(901)?,
        },
        Actor::Importer {
            name: "synthetic-importer".to_owned(),
            version: "1".to_owned(),
        },
    ];
    for actor in &automatic_actors {
        assert!(matches!(
            VerifiedCuratorApproval::for_concept(actor, &forged_user_claim, &evidence, &candidate,),
            Err(OntologyError::Domain(
                DomainError::ActorAuthorityMismatch { .. }
            ))
        ));
    }

    // Injection one layer out: each automatic actor's own valid authority/status
    // pair is still not a curator approval.
    let automatic_pairs = [
        (
            &automatic_actors[0],
            AuthorityClass::DeterministicEngine,
            EpistemicStatus::DeterministicDerived,
        ),
        (
            &automatic_actors[1],
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
        ),
        (
            &automatic_actors[2],
            AuthorityClass::Curated,
            EpistemicStatus::DeterministicDerived,
        ),
    ];
    for (index, (actor, authority, status)) in automatic_pairs.into_iter().enumerate() {
        let (claim, automatic_evidence) = approval_material(
            candidate.concept().id(),
            digest,
            210 + u32::try_from(index)?,
            authority,
            status,
        )?;
        assert!(matches!(
            VerifiedCuratorApproval::for_concept(actor, &claim, &automatic_evidence, &candidate,),
            Err(OntologyError::InvalidApprovalAction)
        ));
    }

    let mut wrong_scope_claim = forged_user_claim.clone();
    wrong_scope_claim.scope_id = scope_id(2)?;
    let user = Actor::User {
        user_id: entity(902)?,
    };
    assert!(matches!(
        VerifiedCuratorApproval::for_concept(&user, &wrong_scope_claim, &evidence, &candidate,),
        Err(OntologyError::ApprovalScopeMismatch)
    ));

    // Revert to the sole admitted pairing and observe a typed approval token.
    let approval =
        VerifiedCuratorApproval::for_concept(&user, &forged_user_claim, &evidence, &candidate)?;
    let approved = candidate.approve(approval)?;
    assert_eq!(approved.status(), GranularityStatus::Curated);
    assert_eq!(approved.approved_by(), entity(902)?);
    Ok(())
}

#[test]
fn taxonomy_import_is_versioned_and_base_mix_remains_unselected() -> TestResult {
    let first = taxonomy("curriculum-2026.1")?;
    let changed_nodes = vec![
        TaxonomyNode::Field(Field::new(entity(1)?, "Database Systems")?),
        TaxonomyNode::Concept(Concept::new(entity(2)?, "Serializability", entity(1)?)?),
    ];
    assert!(matches!(
        VersionedTaxonomyImport::with_identity(first.identity().clone(), changed_nodes),
        Err(OntologyError::TaxonomyDigestMismatch { .. })
    ));

    let second = taxonomy("curriculum-2026.2")?;
    assert_ne!(first.identity(), second.identity());
    let mix = BaseTaxonomyMix::Unselected;
    assert_eq!(mix, BaseTaxonomyMix::Unselected);

    let explicit =
        TaxonomyMixSelection::new(1, vec![first.identity().clone(), second.identity().clone()])?;
    assert_eq!(explicit.configuration_version(), 1);
    assert_eq!(explicit.versions().len(), 2);
    Ok(())
}
