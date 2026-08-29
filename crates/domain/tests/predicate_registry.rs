//! Named acceptance evidence for the §7.2 predicate and edge registry.

use std::str::FromStr as _;

use academic_domain::predicates::{
    Cardinality, EdgeAssertion, EdgeDirection, EdgeEvidence, EdgeKey, EvidenceLocatorKind,
    MinimumEvidence, NodeType, OPEN_GATES, PREDICATE_REGISTRY, PREDICATE_REGISTRY_VERSION,
    PredicateName, PrerequisiteStrength, Qualifier, QualifierKind, QualifierValue, RegistryError,
    inverse_neighbours, personal_mastery_ceiling, prerequisite_descriptor, supports_mastery,
};
use academic_domain::{
    ArtifactId, AuthorityClass, DomainError, EntityId, EvidenceRole, EvidenceStrength,
    MasteryLevel, PredicateId,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const REGISTRY_SOURCE: &str = include_str!("../../../schemas/registry/predicate-registry-v1.json");

fn entity(suffix: u32) -> Result<EntityId, DomainError> {
    EntityId::from_str(&format!("01900000-0000-7000-8000-{suffix:012x}"))
}

fn artifact(suffix: u32) -> Result<ArtifactId, DomainError> {
    ArtifactId::from_str(&format!("01900000-0000-7000-8000-{suffix:012x}"))
}

fn item(artifact_id: ArtifactId, strength: EvidenceStrength) -> EdgeEvidence {
    EdgeEvidence {
        artifact_id,
        role: EvidenceRole::Supports,
        strength,
        locator_kind: EvidenceLocatorKind::TextBytes,
    }
}

fn strength(value: PrerequisiteStrength) -> Qualifier {
    Qualifier {
        key: "prerequisite_strength".to_owned(),
        value: QualifierValue::Strength(value),
    }
}

fn enumerated(key: &str, value: &str) -> Qualifier {
    Qualifier {
        key: key.to_owned(),
        value: QualifierValue::Enumerated(value.to_owned()),
    }
}

/// Builds an assertion whose ends are the first admitted types of `predicate`.
fn assertion<'a>(
    predicate: PredicateName,
    authority: AuthorityClass,
    qualifiers: &'a [Qualifier],
    evidence: &'a [EdgeEvidence],
) -> Result<EdgeAssertion<'a>, Box<dyn std::error::Error>> {
    let descriptor = predicate.descriptor();
    Ok(EdgeAssertion {
        key: EdgeKey::new(predicate, entity(1)?, entity(2)?)?,
        subject_type: descriptor.subject_types[0],
        object_type: descriptor.object_types[0],
        authority_class: authority,
        qualifiers,
        evidence,
    })
}

#[test]
fn every_edge_has_direction_type_and_cardinality() -> TestResult {
    let source: serde_json::Value = serde_json::from_str(REGISTRY_SOURCE)?;
    let rows = source["predicates"]
        .as_array()
        .ok_or("registry must list predicates")?;

    assert_eq!(rows.len(), 20, "§7.2 fixes exactly twenty edges");
    assert_eq!(PREDICATE_REGISTRY.len(), rows.len());
    assert_eq!(PredicateName::ALL.len(), rows.len());
    assert_eq!(
        PREDICATE_REGISTRY_VERSION,
        u16::try_from(source["registry_version"].as_u64().ok_or("version")?)?
    );

    for (index, predicate) in PredicateName::ALL.into_iter().enumerate() {
        let descriptor = predicate.descriptor();
        let row = &rows[index];

        assert_eq!(descriptor.name, predicate, "registry is indexed by name");
        assert_eq!(index, predicate as usize, "index must equal discriminant");
        assert_eq!(
            descriptor.name.as_str(),
            row["name"].as_str().ok_or("name")?
        );
        assert_eq!(
            descriptor.spec_direction,
            row["spec_direction"].as_str().ok_or("direction")?,
            "{} must quote its §7.2 direction cell",
            predicate.as_str()
        );
        assert_eq!(
            descriptor.predicate_id,
            format!(
                "graph.{}",
                predicate.as_str().to_lowercase().replace('_', ".")
            ),
            "{} must derive its claim predicate id",
            predicate.as_str()
        );
        PredicateId::parse(descriptor.predicate_id)?;

        assert!(
            !descriptor.subject_types.is_empty(),
            "{} must declare a subject type",
            predicate.as_str()
        );
        assert!(
            !descriptor.object_types.is_empty(),
            "{} must declare an object type",
            predicate.as_str()
        );
        assert!(
            descriptor
                .subject_types
                .iter()
                .chain(descriptor.object_types)
                .all(|node| NodeType::ALL.contains(node)),
            "{} must use §7.1 node types only",
            predicate.as_str()
        );
        assert_eq!(descriptor.cardinality, Cardinality::ManyToMany);
        assert_eq!(
            descriptor.direction == EdgeDirection::UndirectedCanonical,
            predicate == PredicateName::RelatedTo,
            "RELATED_TO is the only undirected §7.2 edge"
        );
        assert_eq!(
            descriptor.prerequisite,
            !descriptor.strengths.is_empty(),
            "{} must carry a strength exactly when it is a prerequisite edge",
            predicate.as_str()
        );
        assert!(!descriptor.inverse_label.is_empty());
        assert_eq!(
            descriptor.since_registry_version,
            PREDICATE_REGISTRY_VERSION
        );
    }

    assert_eq!(OPEN_GATES, ["GATE-38-022"], "the taxonomy mix stays open");
    Ok(())
}

#[test]
fn related_to_is_canonically_ordered_and_non_prerequisite() -> TestResult {
    let smaller = entity(0x10)?;
    let larger = entity(0x20)?;
    assert!(smaller < larger);

    let forward = EdgeKey::new(PredicateName::RelatedTo, smaller, larger)?;
    let reversed = EdgeKey::new(PredicateName::RelatedTo, larger, smaller)?;
    assert_eq!(forward, reversed, "either order is one stored row");
    assert_eq!(forward.subject(), smaller);
    assert_eq!(forward.object(), larger);

    let directed = EdgeKey::new(PredicateName::Requires, larger, smaller)?;
    assert_eq!(
        directed.subject(),
        larger,
        "a directed edge keeps the asserted order"
    );

    assert_eq!(
        prerequisite_descriptor(PredicateName::RelatedTo),
        Err(RegistryError::NotAPrerequisitePredicate("RELATED_TO")),
        "a path engine may not traverse RELATED_TO as a prerequisite"
    );
    assert!(prerequisite_descriptor(PredicateName::Requires).is_ok());
    assert!(prerequisite_descriptor(PredicateName::BuildsOn).is_ok());

    let descriptor = PredicateName::RelatedTo.descriptor();
    assert!(descriptor.strengths.is_empty());
    assert!(descriptor.qualifiers.is_empty());
    assert!(!descriptor.prerequisite);

    assert_eq!(
        EdgeKey::new(PredicateName::RelatedTo, smaller, smaller),
        Err(RegistryError::SelfEdge("RELATED_TO"))
    );
    Ok(())
}

#[test]
fn inverse_is_a_view_not_a_row() -> TestResult {
    for predicate in PredicateName::ALL {
        let label = predicate.descriptor().inverse_label;
        let as_name = label.to_uppercase().replace(' ', "_");
        assert!(
            PredicateName::parse(&as_name).is_none(),
            "{} declares an inverse label that is itself a predicate",
            predicate.as_str()
        );
    }

    // Where the two ends admit disjoint node types the reverse row is not even
    // constructible: direction enforcement rejects it.
    let mut asymmetric = 0_usize;
    for predicate in PredicateName::ALL {
        let descriptor = predicate.descriptor();
        if descriptor
            .subject_types
            .iter()
            .any(|node| descriptor.object_types.contains(node))
        {
            continue;
        }
        asymmetric += 1;
        let reversed = EdgeAssertion {
            key: EdgeKey::new(predicate, entity(1)?, entity(2)?)?,
            subject_type: descriptor.object_types[0],
            object_type: descriptor.subject_types[0],
            authority_class: AuthorityClass::Official,
            qualifiers: &[],
            evidence: &[],
        };
        assert!(
            matches!(
                reversed.validate(),
                Err(RegistryError::SubjectTypeNotAdmitted { .. })
                    | Err(RegistryError::ObjectTypeNotAdmitted { .. })
            ),
            "{} must reject its reversed assertion",
            predicate.as_str()
        );
    }
    assert!(asymmetric >= 10, "most §7.2 edges are type-asymmetric");

    let concept = entity(0x30)?;
    let lecture = entity(0x31)?;
    let other_concept = entity(0x32)?;
    let stored = vec![
        EdgeKey::new(PredicateName::TaughtIn, concept, lecture)?,
        EdgeKey::new(PredicateName::TaughtIn, other_concept, lecture)?,
    ];
    assert_eq!(
        inverse_neighbours(&stored, PredicateName::TaughtIn, lecture),
        vec![concept, other_concept],
        "the inverse reading is derived from the forward rows"
    );
    assert_eq!(stored.len(), 2, "reading the inverse stores nothing");
    assert!(
        inverse_neighbours(&stored, PredicateName::TaughtIn, concept).is_empty(),
        "a directed edge has no reverse row to find"
    );

    let related = vec![EdgeKey::new(PredicateName::RelatedTo, concept, lecture)?];
    assert_eq!(
        inverse_neighbours(&related, PredicateName::RelatedTo, concept),
        vec![lecture],
        "an undirected edge is readable from either end of its single row"
    );
    assert_eq!(
        inverse_neighbours(&related, PredicateName::RelatedTo, lecture),
        vec![concept]
    );
    Ok(())
}

#[test]
fn single_source_hard_prerequisite_is_rejected() -> TestResult {
    let one_source = [
        item(artifact(0x40)?, EvidenceStrength::Direct),
        item(artifact(0x40)?, EvidenceStrength::Direct),
    ];
    let hard = [strength(PrerequisiteStrength::Hard)];
    assert_eq!(
        assertion(
            PredicateName::Requires,
            AuthorityClass::Curated,
            &hard,
            &one_source
        )?
        .validate(),
        Err(RegistryError::InsufficientIndependentSources {
            predicate: "REQUIRES",
            required: 2,
            actual: 1,
        }),
        "a HARD prerequisite may not rest on one source"
    );

    let two_sources = [
        item(artifact(0x40)?, EvidenceStrength::Direct),
        item(artifact(0x41)?, EvidenceStrength::Direct),
    ];
    assert_eq!(
        assertion(
            PredicateName::Requires,
            AuthorityClass::Curated,
            &hard,
            &two_sources
        )?
        .validate(),
        Ok(()),
        "two independent sources admit the same HARD edge"
    );

    let strong = [strength(PrerequisiteStrength::Strong)];
    let single = [item(artifact(0x40)?, EvidenceStrength::Corroborating)];
    assert_eq!(
        assertion(
            PredicateName::Requires,
            AuthorityClass::Curated,
            &strong,
            &single
        )?
        .validate(),
        Ok(()),
        "the second source is demanded by HARD, not by REQUIRES itself"
    );

    let helpful = [strength(PrerequisiteStrength::Helpful)];
    assert_eq!(
        assertion(
            PredicateName::Requires,
            AuthorityClass::Curated,
            &helpful,
            &two_sources
        )?
        .validate(),
        Err(RegistryError::StrengthNotAdmitted {
            predicate: "REQUIRES",
            strength: PrerequisiteStrength::Helpful,
        }),
        "REQUIRES is not a preference edge"
    );
    let hard_builds_on = [strength(PrerequisiteStrength::Hard)];
    assert_eq!(
        assertion(
            PredicateName::BuildsOn,
            AuthorityClass::Curated,
            &hard_builds_on,
            &two_sources
        )?
        .validate(),
        Err(RegistryError::StrengthNotAdmitted {
            predicate: "BUILDS_ON",
            strength: PrerequisiteStrength::Hard,
        }),
        "BUILDS_ON is distinguished from REQUIRES by refusing HARD"
    );

    assert_eq!(
        assertion(
            PredicateName::Requires,
            AuthorityClass::Curated,
            &[],
            &two_sources
        )?
        .validate(),
        Err(RegistryError::MissingQualifier {
            predicate: "REQUIRES",
            key: "prerequisite_strength",
        }),
        "a prerequisite edge must declare its strength"
    );
    Ok(())
}

#[test]
fn mention_cannot_promote_to_taught_or_understood() -> TestResult {
    assert_eq!(
        personal_mastery_ceiling(PredicateName::MentionedIn),
        Ok(MasteryLevel::Exposed)
    );
    assert!(supports_mastery(
        PredicateName::MentionedIn,
        MasteryLevel::Exposed
    ));
    for level in [
        MasteryLevel::Understood,
        MasteryLevel::Practiced,
        MasteryLevel::Applied,
        MasteryLevel::Fluent,
    ] {
        assert!(
            !supports_mastery(PredicateName::MentionedIn, level),
            "a mention may not reach {level:?}"
        );
    }

    // The evidence that admits a mention does not admit a teaching claim.
    let mention_evidence = [EdgeEvidence {
        artifact_id: artifact(0x50)?,
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Weak,
        locator_kind: EvidenceLocatorKind::TextBytes,
    }];
    assert_eq!(
        assertion(
            PredicateName::MentionedIn,
            AuthorityClass::ModelInference,
            &[],
            &mention_evidence
        )?
        .validate(),
        Ok(())
    );
    assert_eq!(
        assertion(
            PredicateName::TaughtIn,
            AuthorityClass::ModelInference,
            &[],
            &mention_evidence
        )?
        .validate(),
        Err(RegistryError::InsufficientEvidence {
            predicate: "TAUGHT_IN",
            required: 1,
            actual: 0,
        }),
        "a weak mention is not transcript or material evidence"
    );

    // Nor may the same row be restated as a teaching edge: a mention points at
    // a source segment, a teaching claim at a lecture.
    let restated = EdgeAssertion {
        key: EdgeKey::new(PredicateName::TaughtIn, entity(1)?, entity(2)?)?,
        subject_type: NodeType::Concept,
        object_type: NodeType::EvidenceItem,
        authority_class: AuthorityClass::DirectObservation,
        qualifiers: &[],
        evidence: &[item(artifact(0x50)?, EvidenceStrength::Direct)],
    };
    assert_eq!(
        restated.validate(),
        Err(RegistryError::ObjectTypeNotAdmitted {
            predicate: "TAUGHT_IN",
            object: NodeType::EvidenceItem,
        })
    );
    Ok(())
}

#[test]
fn used_in_creates_no_personal_claim() -> TestResult {
    assert_eq!(
        personal_mastery_ceiling(PredicateName::UsedIn),
        Err(RegistryError::NotPersonalStateBearing("USED_IN")),
        "where a concept is generally used is not a claim about the user"
    );
    for level in [
        MasteryLevel::Unseen,
        MasteryLevel::Exposed,
        MasteryLevel::Understood,
        MasteryLevel::Practiced,
        MasteryLevel::Applied,
        MasteryLevel::Fluent,
    ] {
        assert!(!supports_mastery(PredicateName::UsedIn, level));
    }

    // The edge itself is still assertable; only the personal reading is refused.
    assert_eq!(
        assertion(
            PredicateName::UsedIn,
            AuthorityClass::Curated,
            &[],
            &[item(artifact(0x60)?, EvidenceStrength::Corroborating)]
        )?
        .validate(),
        Ok(())
    );

    assert_eq!(
        personal_mastery_ceiling(PredicateName::AppliedIn),
        Ok(MasteryLevel::Applied),
        "the personal reading belongs to APPLIED_IN"
    );
    for predicate in [
        PredicateName::Requires,
        PredicateName::BuildsOn,
        PredicateName::RelatedTo,
        PredicateName::DesignedToTeach,
        PredicateName::EnablesCompetency,
        PredicateName::RelevantToRole,
    ] {
        assert!(
            personal_mastery_ceiling(predicate).is_err(),
            "{} describes the world, not the user",
            predicate.as_str()
        );
    }
    Ok(())
}

#[test]
fn designed_to_teach_requires_official_or_reviewed_source() -> TestResult {
    let evidence = [item(artifact(0x70)?, EvidenceStrength::Corroborating)];
    for authority in [AuthorityClass::Official, AuthorityClass::Curated] {
        assert_eq!(
            assertion(PredicateName::DesignedToTeach, authority, &[], &evidence)?.validate(),
            Ok(()),
            "{authority:?} is an official or reviewed curriculum source"
        );
    }
    for authority in [
        AuthorityClass::ModelInference,
        AuthorityClass::UserExplicit,
        AuthorityClass::Prediction,
        AuthorityClass::Unknown,
    ] {
        assert_eq!(
            assertion(PredicateName::DesignedToTeach, authority, &[], &evidence)?.validate(),
            Err(RegistryError::AuthorityNotPermitted {
                predicate: "DESIGNED_TO_TEACH",
                authority,
            }),
            "{authority:?} may not state curriculum intent"
        );
    }
    Ok(())
}

#[test]
fn applied_in_requires_user_evidence() -> TestResult {
    let evidence = [item(artifact(0x80)?, EvidenceStrength::Direct)];
    for authority in [
        AuthorityClass::UserExplicit,
        AuthorityClass::DirectObservation,
    ] {
        assert_eq!(
            assertion(PredicateName::AppliedIn, authority, &[], &evidence)?.validate(),
            Ok(())
        );
    }
    for authority in [
        AuthorityClass::Official,
        AuthorityClass::Curated,
        AuthorityClass::ModelInference,
        AuthorityClass::Prediction,
    ] {
        assert_eq!(
            assertion(PredicateName::AppliedIn, authority, &[], &evidence)?.validate(),
            Err(RegistryError::AuthorityNotPermitted {
                predicate: "APPLIED_IN",
                authority,
            }),
            "{authority:?} is not the user's own application evidence"
        );
    }
    assert_eq!(
        assertion(
            PredicateName::AppliedIn,
            AuthorityClass::UserExplicit,
            &[],
            &[item(artifact(0x80)?, EvidenceStrength::Corroborating)]
        )?
        .validate(),
        Err(RegistryError::InsufficientEvidence {
            predicate: "APPLIED_IN",
            required: 1,
            actual: 0,
        }),
        "an applied claim needs direct evidence, not a corroborating hint"
    );
    Ok(())
}

#[test]
fn enables_competency_requires_importance_and_necessity() -> TestResult {
    let evidence = [item(artifact(0x90)?, EvidenceStrength::Corroborating)];
    let both = [
        enumerated("contribution_importance", "CRITICAL"),
        enumerated("necessity", "NECESSARY"),
    ];
    assert_eq!(
        assertion(
            PredicateName::EnablesCompetency,
            AuthorityClass::Curated,
            &both,
            &evidence
        )?
        .validate(),
        Ok(())
    );

    for (qualifiers, missing) in [
        (
            vec![enumerated("necessity", "OPTIONAL")],
            "contribution_importance",
        ),
        (
            vec![enumerated("contribution_importance", "MINOR")],
            "necessity",
        ),
        (Vec::new(), "contribution_importance"),
    ] {
        assert_eq!(
            assertion(
                PredicateName::EnablesCompetency,
                AuthorityClass::Curated,
                &qualifiers,
                &evidence
            )?
            .validate(),
            Err(RegistryError::MissingQualifier {
                predicate: "ENABLES_COMPETENCY",
                key: missing,
            })
        );
    }

    let unknown = [
        enumerated("contribution_importance", "CRITICAL"),
        enumerated("necessity", "NECESSARY"),
        enumerated("weight", "0.5"),
    ];
    assert_eq!(
        assertion(
            PredicateName::EnablesCompetency,
            AuthorityClass::Curated,
            &unknown,
            &evidence
        )?
        .validate(),
        Err(RegistryError::UnknownQualifier {
            predicate: "ENABLES_COMPETENCY",
            key: "weight".to_owned(),
        }),
        "the qualifier schema is closed"
    );

    let outside = [
        enumerated("contribution_importance", "ESSENTIAL"),
        enumerated("necessity", "NECESSARY"),
    ];
    assert_eq!(
        assertion(
            PredicateName::EnablesCompetency,
            AuthorityClass::Curated,
            &outside,
            &evidence
        )?
        .validate(),
        Err(RegistryError::QualifierValueNotAdmitted {
            key: "contribution_importance",
        }),
        "importance is a closed enumeration, not free text"
    );
    Ok(())
}

/// The generated constants and the registry file are one contract.
#[test]
fn generated_constants_match_the_registry_source() -> TestResult {
    fn names(values: &serde_json::Value) -> Result<Vec<&str>, Box<dyn std::error::Error>> {
        Ok(values
            .as_array()
            .ok_or("expected a list of names")?
            .iter()
            .map(|value| value.as_str().unwrap_or_default())
            .collect())
    }

    fn locator(kind: EvidenceLocatorKind) -> &'static str {
        match kind {
            EvidenceLocatorKind::Page => "PAGE",
            EvidenceLocatorKind::TextBytes => "TEXT_BYTES",
            EvidenceLocatorKind::TranscriptTime => "TRANSCRIPT_TIME",
            EvidenceLocatorKind::RepositoryBytes => "REPOSITORY_BYTES",
        }
    }

    fn wire<T: serde::Serialize>(value: T) -> Result<String, Box<dyn std::error::Error>> {
        Ok(serde_json::to_value(value)?
            .as_str()
            .ok_or("expected a string discriminant")?
            .to_owned())
    }

    fn rule(
        expected: &serde_json::Value,
        actual: &MinimumEvidence,
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            u64::from(actual.supporting),
            expected["supporting"].as_u64().ok_or("n")?
        );
        assert_eq!(
            u64::from(actual.independent_sources),
            expected["independent_sources"].as_u64().ok_or("sources")?
        );
        assert_eq!(wire(actual.min_strength)?, expected["min_strength"]);
        assert_eq!(
            actual
                .authority
                .iter()
                .map(|class| wire(class))
                .collect::<Result<Vec<_>, _>>()?,
            names(&expected["authority"])?
        );
        assert_eq!(
            actual
                .locator_kinds
                .iter()
                .map(|kind| locator(*kind))
                .collect::<Vec<_>>(),
            names(&expected["locator_kinds"])?
        );
        Ok(())
    }

    let source: serde_json::Value = serde_json::from_str(REGISTRY_SOURCE)?;
    let node_types = names(&source["node_types"])?;
    assert_eq!(node_types.len(), NodeType::ALL.len());
    for (node, expected) in NodeType::ALL.into_iter().zip(node_types) {
        assert_eq!(node.as_str(), expected);
    }

    for (index, predicate) in PredicateName::ALL.into_iter().enumerate() {
        let descriptor = predicate.descriptor();
        let row = &source["predicates"][index];
        assert_eq!(descriptor.spec_meaning, row["spec_meaning"]);
        assert_eq!(descriptor.inverse_label, row["inverse_label"]);
        assert_eq!(descriptor.prerequisite, row["prerequisite"]);
        assert_eq!(
            descriptor
                .subject_types
                .iter()
                .map(|node| node.as_str())
                .collect::<Vec<_>>(),
            names(&row["subject_types"])?
        );
        assert_eq!(
            descriptor
                .object_types
                .iter()
                .map(|node| node.as_str())
                .collect::<Vec<_>>(),
            names(&row["object_types"])?
        );
        assert_eq!(
            descriptor
                .personal_state_ceiling
                .map(wire)
                .transpose()?
                .unwrap_or_default(),
            row["personal_state_ceiling"].as_str().unwrap_or_default()
        );
        assert_eq!(
            descriptor
                .strengths
                .iter()
                .map(|value| match value {
                    PrerequisiteStrength::Hard => "HARD",
                    PrerequisiteStrength::Strong => "STRONG",
                    PrerequisiteStrength::Helpful => "HELPFUL",
                })
                .collect::<Vec<_>>(),
            names(&row["strengths"])?
        );

        let qualifiers = row["qualifiers"].as_array().ok_or("qualifiers")?;
        assert_eq!(descriptor.qualifiers.len(), qualifiers.len());
        for (schema, expected) in descriptor.qualifiers.iter().zip(qualifiers) {
            assert_eq!(schema.key, expected["key"]);
            assert_eq!(schema.required, expected["required"]);
            match schema.kind {
                QualifierKind::Enumeration(values) => {
                    assert_eq!(expected["kind"], "ENUMERATION");
                    assert_eq!(values.to_vec(), names(&expected["values"])?);
                }
                QualifierKind::PrerequisiteStrength => {
                    assert_eq!(expected["kind"], "PREREQUISITE_STRENGTH");
                }
                QualifierKind::EntityReference => assert_eq!(expected["kind"], "ENTITY_REFERENCE"),
                QualifierKind::PositiveInteger => assert_eq!(expected["kind"], "POSITIVE_INTEGER"),
            }
        }

        rule(
            &row["minimum_evidence"]["base"],
            &descriptor.minimum_evidence.base,
        )?;
        let overrides = row["minimum_evidence"]["by_strength"]
            .as_array()
            .ok_or("overrides")?;
        assert_eq!(
            descriptor.minimum_evidence.by_strength.len(),
            overrides.len()
        );
        for ((asserted, minimum), expected) in descriptor
            .minimum_evidence
            .by_strength
            .iter()
            .zip(overrides)
        {
            assert_eq!(
                match asserted {
                    PrerequisiteStrength::Hard => "HARD",
                    PrerequisiteStrength::Strong => "STRONG",
                    PrerequisiteStrength::Helpful => "HELPFUL",
                },
                expected["strength"]
            );
            rule(&expected["rule"], minimum)?;
        }
    }
    Ok(())
}
