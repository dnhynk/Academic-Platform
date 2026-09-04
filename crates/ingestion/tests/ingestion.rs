//! `P2-U6`'s named acceptance evidence, less the three that are source scans
//! and the four that are compile failures.
//!
//! `no_numeric_source_winner`, `credentials_never_reach_a_general_crawler` and
//! `no_captcha_or_access_control_bypass_module_exists` are in
//! `tests/ingestion_scans.rs`, because each is a statement about what the
//! source does not contain. `unscoped_official_source_cannot_publish` has a
//! behavioural half here and a type-level half in `tests/compile_fail.rs`; the
//! type-level half is the one that matters.

mod support;

use std::{error::Error, fs, path::PathBuf};

use academic_domain::engines::RuleId;
use academic_ingestion::{
    Acquisition, Appropriateness, AuditDisposition, ConflictCase, ConflictDimension,
    ConnectorManifest, ContendingSource, Corpus, DateComparison, DateRelation, Denial,
    DenialReason, DenialRoute, Dependency, DependencyGraph, DependentId, DependentKind,
    DependentNode, DimensionOutcome, DocumentChange, Fallback, FetchOutcome, HierarchyRelation,
    IngestSeq, LegalAuthority, OpenGate, Publication, QueueReason, RetrievalInstant, RunOutcome,
    ScopeRelation, Side, SnapshotError, SourceDiff, Stage, TermsLedger, TermsStatus,
    TransitionRelation, UNSCOPED_OFFICIAL_SOURCE, UserResolution, phase2_shipped_fallbacks, stage,
    unreviewed_status,
};
use academic_untrusted_content::{SourceId, SourceKind};
use support::{
    BYLAW, CATALOGUE, DocumentFixture, FixtureSource, PARSER, RETRIEVED_AT, UNDECLARED, body,
    connector, corpus, draft, manifest, not_modified, permitting_ledger, torn_body,
};

type TestResult = Result<(), Box<dyn Error>>;

const CONNECTOR: &str = "snu.cse.official";

/// The repository root, for the two tests that read the authoritative spec.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The authoritative specification's text.
fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(repository_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

// ---------------------------------------------------------------------------
// The stage order
// ---------------------------------------------------------------------------

/// Everything one hand-driven run produced.
struct Driven {
    reached: Vec<Stage>,
    publication: Option<Publication>,
    failure: Option<stage::StageFailure>,
}

/// Drives the nine stages in order, arranging for `fail_at` to fail.
///
/// Hand-driven rather than through [`academic_ingestion::run`] because `IN06`
/// needs the terms ledger to change *between* two stages, which a single
/// borrowed ledger cannot express. Driving the nine functions is also the
/// clearest statement of what the type chain does: each call takes the value
/// the call before it returned, and there is no other way to write the sequence.
fn drive(fail_at: Option<Stage>) -> Result<Driven, Box<dyn Error>> {
    let manifest = if fail_at == Some(Stage::SourceMetadataAndRetrievalTime) {
        // An overdue declaration: `next verification` is in the past at the
        // retrieval instant, which is the field section 29.1 asks for.
        draft(CONNECTOR)?
            .next_verification(academic_ingestion::NextVerification::due_at(
                RetrievalInstant::at(RETRIEVED_AT.seconds() - 1),
            ))
            .build()?
    } else {
        manifest(CONNECTOR)?
    };

    let permitting = permitting_ledger(CONNECTOR)?;
    let empty = TermsLedger::new();
    let ledger = if matches!(
        fail_at,
        Some(Stage::DiscoverFetchImport | Stage::PolicyAndTermsCheck)
    ) {
        &empty
    } else {
        &permitting
    };

    let bytes = match fail_at {
        Some(Stage::DeterministicParse) => b"this line has no directive\n".to_vec(),
        Some(Stage::SchemaValidation) => DocumentFixture::dated()
            .with_extra_rule("art-13", "r-12-1")
            .bytes(),
        Some(Stage::AiProposalWhereAppropriate) => DocumentFixture::dated()
            .with_rule_text("r-12-1", &"x".repeat(1_100_000))
            .bytes(),
        _ => DocumentFixture::dated().bytes(),
    };

    let outcome = if fail_at == Some(Stage::ImmutableRawSnapshot) {
        torn_body(bytes, b"the source changed under the read", "\"v1\"")?
    } else {
        body(bytes, "\"v1\"")?
    };

    let source = FixtureSource::holding(Vec::new(), "\"v1\"");
    let acquisition = if fail_at == Some(Stage::DiscoverFetchImport) {
        Acquisition::Fetch {
            transport: &source,
            request: academic_ingestion::ConditionalRequest::anonymous(
                &manifest,
                CATALOGUE,
                academic_ingestion::Validators::none(),
            )?,
        }
    } else {
        Acquisition::Import {
            target: CATALOGUE,
            outcome,
        }
    };

    let appropriateness = if fail_at == Some(Stage::AiProposalWhereAppropriate) {
        Appropriateness::SealForModel {
            source_id: SourceId::new("official-cse-graduation")?,
            kind: SourceKind::Syllabus,
        }
    } else {
        Appropriateness::NotAppropriate
    };

    let known = if fail_at == Some(Stage::ReconciliationAndEntityResolution) {
        Corpus::new()
    } else {
        corpus()?
    };

    let mut reached = Vec::new();
    macro_rules! step {
        ($stage:expr, $call:expr) => {{
            reached.push($stage);
            match $call {
                Ok(value) => value,
                Err(failure) => {
                    return Ok(Driven {
                        reached,
                        publication: None,
                        failure: Some(failure),
                    });
                }
            }
        }};
    }

    let fetched = step!(
        Stage::DiscoverFetchImport,
        stage::discover_fetch_import(&manifest, ledger, RETRIEVED_AT, acquisition)
    );
    let cleared = step!(
        Stage::PolicyAndTermsCheck,
        stage::policy_and_terms_check(fetched, &manifest, ledger)
    );
    let snapshotted = step!(
        Stage::ImmutableRawSnapshot,
        stage::immutable_raw_snapshot(cleared, &manifest)
    );
    let described = step!(
        Stage::SourceMetadataAndRetrievalTime,
        stage::source_metadata_and_retrieval_time(snapshotted, &manifest, IngestSeq::at(1))
    );
    let parsed = step!(
        Stage::DeterministicParse,
        stage::deterministic_parse(described)
    );
    let validated = step!(Stage::SchemaValidation, stage::schema_validation(parsed));
    let proposed = step!(
        Stage::AiProposalWhereAppropriate,
        stage::ai_proposal_where_appropriate(validated, appropriateness)
    );
    let reconciled = step!(
        Stage::ReconciliationAndEntityResolution,
        stage::reconciliation_and_entity_resolution(proposed, &known)
    );

    // `IN06`. The permission is withdrawn between stage eight and stage nine,
    // which is the only place a mid-run revocation can be expressed.
    let mut at_publication = permitting_ledger(CONNECTOR)?;
    if fail_at == Some(Stage::ClaimPublicationOrReviewQueue) {
        at_publication.record(connector(CONNECTOR)?, TermsStatus::Revoked);
    }
    let publication = step!(
        Stage::ClaimPublicationOrReviewQueue,
        stage::claim_publication_or_review_queue(reconciled, &at_publication)
    );

    Ok(Driven {
        reached,
        publication: Some(publication),
        failure: None,
    })
}

/// A failed stage means no publication, and no stage after it runs.
///
/// The stages are enumerated, not counted: the loop is over [`Stage::ALL`], and
/// nothing here asserts how long that list is. Adding a stage adds a case.
#[test]
fn ingestion_stage_order_is_strict() -> TestResult {
    // The positive control. Without it every assertion below would also hold
    // for a pipeline that publishes nothing ever.
    let complete = drive(None)?;
    assert_eq!(
        complete.reached,
        Stage::ALL.to_vec(),
        "the unpoisoned run did not reach every stage"
    );
    assert!(
        complete
            .publication
            .as_ref()
            .and_then(Publication::published)
            .is_some(),
        "the unpoisoned run published nothing, so the cases below prove nothing"
    );

    for stage in Stage::ALL {
        let driven = drive(Some(stage))?;
        let failure = driven
            .failure
            .as_ref()
            .ok_or_else(|| format!("{} was arranged to fail and did not", stage.as_str()))?;
        assert_eq!(
            failure.stage(),
            stage,
            "the failure was reported against another stage"
        );
        assert!(
            driven.publication.is_none(),
            "{} failed and something was published anyway",
            stage.as_str()
        );

        let expected: Vec<Stage> = Stage::ALL
            .into_iter()
            .take_while(|candidate| *candidate != stage)
            .chain(core::iter::once(stage))
            .collect();
        assert_eq!(
            driven.reached,
            expected,
            "{} failed and a later stage ran anyway",
            stage.as_str()
        );
    }
    Ok(())
}

/// The stage list is section 29.1's own block, in its order.
#[test]
fn the_stage_list_is_section_29_1s_own() -> TestResult {
    let specification = specification()?;
    let start = specification
        .find("### 29.1")
        .ok_or("the specification has no section 29.1")?;
    let block_start = specification[start..]
        .find("```text")
        .ok_or("section 29.1 has no fenced block")?
        + start;
    let block_end = specification[block_start + "```text".len()..]
        .find("```")
        .ok_or("section 29.1's fenced block does not close")?
        + block_start
        + "```text".len();
    let block = &specification[block_start + "```text".len()..block_end];

    let lines: Vec<String> = block
        .lines()
        .map(|line| line.trim().trim_start_matches('→').trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    let declared: Vec<String> = Stage::ALL
        .into_iter()
        .map(|stage| stage.spec_line().to_owned())
        .collect();
    assert_eq!(
        lines, declared,
        "the stage list is not section 29.1's block; the specification is authoritative"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------

/// Dropping any one of section 29.1's nine fields refuses the manifest.
#[test]
fn connector_manifest_requires_every_field() -> TestResult {
    use academic_ingestion::{ManifestError, ManifestField};

    // The positive control.
    manifest(CONNECTOR)?;

    for field in ManifestField::ALL {
        // The draft is rebuilt with this one field left empty. Every setter is
        // called except the one under test, so the refusal is about the field
        // and not about the order the setters ran in.
        let mut draft = academic_ingestion::ManifestDraft::for_connector(
            connector(CONNECTOR)?,
            academic_ingestion::SourceCategory::DepartmentPage,
        )
        .declaring(CATALOGUE);
        let filled = manifest(CONNECTOR)?;
        if field != ManifestField::SourceOwnership {
            draft = draft.source_ownership(filled.source_ownership());
        }
        if field != ManifestField::AuthenticationMethod {
            draft = draft.authentication_method(filled.authentication_method());
        }
        if field != ManifestField::AllowedFrequency {
            draft = draft.allowed_frequency(filled.allowed_frequency());
        }
        if field != ManifestField::TermsStatus {
            draft = draft.terms_status(filled.terms_status());
        }
        if field != ManifestField::PersonalDataClass {
            draft = draft.personal_data_class(filled.personal_data_class());
        }
        if field != ManifestField::Completeness {
            draft = draft.completeness(filled.completeness());
        }
        if field != ManifestField::LastSuccess {
            draft = draft.last_success(filled.last_success());
        }
        if field != ManifestField::NextVerification {
            draft = draft.next_verification(filled.next_verification());
        }
        if field != ManifestField::ParserVersion {
            draft = draft.parser_version(filled.parser_version());
        }
        assert_eq!(
            draft.build().err(),
            Some(ManifestError::Missing(field)),
            "a manifest built without its {} was accepted",
            field.as_str()
        );
    }

    // And a manifest that declares nothing to retrieve is not a manifest
    // either: a connector with no target is a crawler waiting for one.
    let no_target = academic_ingestion::ManifestDraft::for_connector(
        connector(CONNECTOR)?,
        academic_ingestion::SourceCategory::DepartmentPage,
    );
    let filled = manifest(CONNECTOR)?;
    assert_eq!(
        no_target
            .source_ownership(filled.source_ownership())
            .authentication_method(filled.authentication_method())
            .allowed_frequency(filled.allowed_frequency())
            .terms_status(filled.terms_status())
            .personal_data_class(filled.personal_data_class())
            .completeness(filled.completeness())
            .last_success(filled.last_success())
            .next_verification(filled.next_verification())
            .parser_version(filled.parser_version())
            .build()
            .err(),
        Some(ManifestError::NoDeclaredTarget)
    );
    Ok(())
}

/// The nine field names are section 29.1's own sentence.
#[test]
fn the_manifest_fields_are_section_29_1s_own() -> TestResult {
    use academic_ingestion::ManifestField;
    let specification = specification()?;
    // The sentence, whole. An edit to it fails here rather than drifting.
    const SENTENCE: &str = "모든 connector는 source ownership, authentication method, allowed frequency, robots/terms status, personal-data class, completeness, last success, next verification과 parser version을 선언한다.";
    assert!(
        specification.contains(SENTENCE),
        "section 29.1's connector-declaration sentence changed; the field list must change with it"
    );
    let mut cursor = 0;
    for field in ManifestField::ALL {
        let at = SENTENCE[cursor..]
            .find(field.as_str())
            .ok_or_else(|| format!("the sentence does not name {}", field.as_str()))?;
        cursor += at + field.as_str().len();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The fetch
// ---------------------------------------------------------------------------

/// A conditional request is answered `304`, and a hash diff sees past a
/// changed validator.
#[test]
fn conditional_fetch_and_hash_diff() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let known = corpus()?;
    let document = DocumentFixture::dated().bytes();
    let source = FixtureSource::holding(document.clone(), "\"v1\"");

    // First fetch: unconditional, and a snapshot is stored.
    let first = one_run(
        &manifest,
        &ledger,
        &known,
        Acquisition::Fetch {
            transport: &source,
            request: academic_ingestion::ConditionalRequest::anonymous(
                &manifest,
                CATALOGUE,
                academic_ingestion::Validators::none(),
            )?,
        },
    )?;
    assert!(first.published().is_some());

    // Second fetch: conditional on what the first recorded, answered `304`.
    // A not-modified response creates no snapshot version at all.
    let cleared = stage::policy_and_terms_check(
        stage::discover_fetch_import(
            &manifest,
            &ledger,
            RETRIEVED_AT,
            Acquisition::Fetch {
                transport: &source,
                request: academic_ingestion::ConditionalRequest::anonymous(
                    &manifest,
                    CATALOGUE,
                    academic_ingestion::HttpMetadata::new(
                        Some(200),
                        Some(academic_ingestion::HeaderValue::new("\"v1\"")?),
                        None,
                        None,
                    )
                    .next_validators(),
                )?,
            },
        )?,
        &manifest,
        &ledger,
    )?;
    assert_eq!(
        stage::immutable_raw_snapshot(cleared, &manifest)
            .err()
            .map(|failure| failure.reason().to_string()),
        Some(SnapshotError::NotModified.to_string()),
        "a 304 created a snapshot version"
    );
    assert_eq!(
        source.conditional_requests(),
        vec![false, true],
        "the second request was not conditional"
    );

    // The hash half. The same bytes under a different entity tag are the same
    // document, and a validator that changed does not make them a new one.
    let unchanged = snapshot_of(&manifest, &ledger, body(document.clone(), "\"v2\"")?)?;
    let original = snapshot_of(&manifest, &ledger, body(document.clone(), "\"v1\"")?)?;
    assert!(
        original.has_same_content_as(&unchanged),
        "a changed entity tag was read as changed content"
    );

    // And different bytes are a different document whatever the tag says.
    let changed = snapshot_of(
        &manifest,
        &ledger,
        body(
            DocumentFixture::dated()
                .with_rule_text("r-12-1", "major electives require thirty-three credits")
                .bytes(),
            "\"v1\"",
        )?,
    )?;
    assert!(
        !original.has_same_content_as(&changed),
        "changed content was read as unchanged"
    );
    Ok(())
}

/// One complete run, for the tests that only need the end of it.
fn one_run(
    manifest: &ConnectorManifest,
    ledger: &TermsLedger,
    known: &Corpus,
    acquisition: Acquisition<'_>,
) -> Result<Publication, Box<dyn Error>> {
    let record = academic_ingestion::run(
        manifest,
        ledger,
        known,
        RETRIEVED_AT,
        acquisition,
        IngestSeq::at(1),
        Appropriateness::NotAppropriate,
    );
    match record.outcome() {
        RunOutcome::Completed(publication) => Ok(publication.clone()),
        RunOutcome::Halted(failure) => Err(Box::new(failure.clone())),
    }
}

/// One snapshot, through stages one to four.
fn snapshot_of(
    manifest: &ConnectorManifest,
    ledger: &TermsLedger,
    outcome: FetchOutcome,
) -> Result<academic_ingestion::RawSnapshot, Box<dyn Error>> {
    let fetched = stage::discover_fetch_import(
        manifest,
        ledger,
        RETRIEVED_AT,
        Acquisition::Import {
            target: CATALOGUE,
            outcome,
        },
    )?;
    let cleared = stage::policy_and_terms_check(fetched, manifest, ledger)?;
    let snapshotted = stage::immutable_raw_snapshot(cleared, manifest)?;
    let described =
        stage::source_metadata_and_retrieval_time(snapshotted, manifest, IngestSeq::at(1))?;
    Ok(described.into_snapshot())
}

/// `IN01`. Bytes that changed under the read are refused, and the retry stores
/// a new immutable version.
#[test]
fn in01_source_bytes_changed_under_the_read() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let document = DocumentFixture::dated().bytes();

    let refused = snapshot_of(
        &manifest,
        &ledger,
        torn_body(document.clone(), b"a shorter reading", "\"v1\"")?,
    );
    let failure = refused.err().ok_or("a torn read was stored")?;
    assert!(
        failure.to_string().contains("changed under the read"),
        "the refusal did not name the torn read: {failure}"
    );

    // The retry. An ordinary second fetch produces a new version, which is
    // what the fault matrix asks for.
    let retried = snapshot_of(&manifest, &ledger, body(document.clone(), "\"v1\"")?)?;
    assert_eq!(retried.byte_len(), document.len());
    Ok(())
}

// ---------------------------------------------------------------------------
// The snapshot
// ---------------------------------------------------------------------------

/// A snapshot retains all five things section 29.1 asks it to.
#[test]
fn rule_source_snapshot_metadata() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let document = DocumentFixture::dated().bytes();
    let snapshot = snapshot_of(&manifest, &ledger, body(document.clone(), "\"v1\"")?)?;

    // 1. The retrieval time.
    assert_eq!(snapshot.retrieved_at(), RETRIEVED_AT);
    // 2. The HTTP metadata.
    assert_eq!(snapshot.http().status(), Some(200));
    assert_eq!(
        snapshot.http().entity_tag().map(|tag| tag.as_str()),
        Some("\"v1\"")
    );
    assert!(snapshot.http().last_modified().is_some());
    assert!(snapshot.http().content_type().is_some());
    // 3. The raw bytes, which are retained and reachable only through the
    //    `P2-G5` seal.
    assert_eq!(snapshot.byte_len(), document.len());
    let sealed = snapshot.seal(
        SourceId::new("official-cse-graduation")?,
        SourceKind::Syllabus,
        1,
    )?;
    assert_eq!(sealed.byte_len(), document.len());
    assert_eq!(sealed.provenance().kind(), SourceKind::Syllabus);
    // 4. The content hash.
    assert_eq!(
        snapshot.digest(),
        &academic_domain::ContentDigest::sha256(&document)
    );
    // 5. The parser version.
    assert_eq!(snapshot.parser_version(), PARSER);

    // The sealed document prints no payload: `P2-G5`'s hand-written `Debug`,
    // and this crate's, both reduce bytes to a count.
    let printed = format!("{snapshot:?}");
    assert!(
        !printed.contains("capstone"),
        "the snapshot's Debug printed document text: {printed}"
    );
    assert!(printed.contains("byte_len"));
    Ok(())
}

/// The three time axes are three types, and none converts into another.
#[test]
fn the_three_time_axes_are_distinct_types() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let snapshot = snapshot_of(
        &manifest,
        &ledger,
        body(DocumentFixture::dated().bytes(), "\"v1\"")?,
    )?;

    // Retrieval time: a wall clock.
    let retrieval: RetrievalInstant = snapshot.retrieved_at();
    // Origin order: a position.
    let order: IngestSeq = IngestSeq::at(1);
    // Valid time: the document's own date.
    let document = academic_ingestion::document::parse(&snapshot)?;
    let valid = document
        .dating()
        .effective_date()
        .ok_or("the dated fixture has no effective date")?;

    // Each reports its own unit and there is no arithmetic between them. The
    // compile-fail suite is where mixing them is observed to be a type error;
    // this is the statement that the three values are three different things.
    assert_eq!(retrieval.seconds(), RETRIEVED_AT.seconds());
    assert_eq!(order.get(), 1);
    assert_eq!(valid.date().year(), 2026);
    Ok(())
}

// ---------------------------------------------------------------------------
// Dating and publication
// ---------------------------------------------------------------------------

/// `IN02`. An undated official document is `UNSCOPED_OFFICIAL_SOURCE` and
/// publishes no rule.
#[test]
fn unscoped_official_source_cannot_publish() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let known = corpus()?;

    let publication = one_run(
        &manifest,
        &ledger,
        &known,
        Acquisition::Import {
            target: CATALOGUE,
            outcome: body(DocumentFixture::undated().bytes(), "\"v1\"")?,
        },
    )?;
    assert!(
        publication.published().is_none(),
        "an undated document published rules"
    );
    let queued = publication.queued().ok_or("nothing was queued either")?;
    assert_eq!(queued.reason(), QueueReason::UnscopedOfficialSource);
    assert_eq!(queued.reason().as_str(), UNSCOPED_OFFICIAL_SOURCE);
    assert_eq!(
        queued.rules().len(),
        3,
        "the queued document lost its rules on the way"
    );

    // The dated control. Same bytes plus one `EFFECTIVE:` line, and it
    // publishes -- so the refusal above is about the dating and not about the
    // fixture.
    let dated = one_run(
        &manifest,
        &ledger,
        &known,
        Acquisition::Import {
            target: CATALOGUE,
            outcome: body(DocumentFixture::dated().bytes(), "\"v1\"")?,
        },
    )?;
    let published = dated
        .published()
        .ok_or("the dated control published nothing")?;
    assert_eq!(published.effective().date().year(), 2026);
    Ok(())
}

// ---------------------------------------------------------------------------
// Change propagation
// ---------------------------------------------------------------------------

/// The diff names exactly the rules a change moves.
#[test]
fn rule_change_impact_identifies_exact_rules() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let parse = |fixture: &DocumentFixture| -> Result<_, Box<dyn Error>> {
        let snapshot = snapshot_of(&manifest, &ledger, body(fixture.bytes(), "\"v1\"")?)?;
        Ok(academic_ingestion::document::parse(&snapshot)?)
    };

    let base = parse(&DocumentFixture::dated())?;

    // Nothing changed.
    let same = parse(&DocumentFixture::dated())?;
    let diff = SourceDiff::between(&base, &same);
    assert!(diff.is_empty());
    assert_eq!(diff.impacted_rules(), Vec::<RuleId>::new());

    // One rule's text changed. Exactly that rule.
    let edited =
        parse(&DocumentFixture::dated().with_rule_text("r-12-2", "a capstone is optional"))?;
    let diff = SourceDiff::between(&base, &edited);
    assert_eq!(diff.impacted_rules(), vec![RuleId::new("r-12-2")?]);
    assert!(matches!(
        diff.rule_changes(),
        [academic_ingestion::RuleChange::TextChanged { .. }]
    ));

    // One rule moved sections. The structural half, and exactly that rule.
    let moved = parse(&DocumentFixture::dated().moving_rule("r-13-1", "art-12"))?;
    let diff = SourceDiff::between(&base, &moved);
    assert_eq!(diff.impacted_rules(), vec![RuleId::new("r-13-1")?]);
    assert!(matches!(
        diff.rule_changes(),
        [academic_ingestion::RuleChange::Moved { .. }]
    ));

    // One added and one removed.
    let churned = parse(
        &DocumentFixture::dated()
            .without_rule("r-12-1")
            .with_extra_rule("art-13", "r-13-2"),
    )?;
    let mut expected = vec![RuleId::new("r-12-1")?, RuleId::new("r-13-2")?];
    expected.sort();
    assert_eq!(
        SourceDiff::between(&base, &churned).impacted_rules(),
        expected
    );

    // A header change moves every rule the document carries, and says which
    // header changed rather than reporting a rule change that did not happen.
    let redated = parse(&DocumentFixture::dated().effective_on("2027-03-01"))?;
    let diff = SourceDiff::between(&base, &redated);
    assert_eq!(diff.document_changes(), [DocumentChange::EffectiveDate]);
    assert!(diff.rule_changes().is_empty());
    let mut every = vec![
        RuleId::new("r-12-1")?,
        RuleId::new("r-12-2")?,
        RuleId::new("r-13-1")?,
    ];
    every.sort();
    assert_eq!(diff.impacted_rules(), every);
    Ok(())
}

/// Invalidation reaches exactly the dependents, transitively, and no others.
#[test]
fn source_change_invalidates_exact_dependents() -> TestResult {
    let node = |kind, name: &str| -> Result<DependentNode, Box<dyn Error>> {
        Ok(DependentNode::new(kind, DependentId::new(name)?))
    };

    let requirement = node(DependentKind::Requirement, "req.major-electives")?;
    let scenario = node(DependentKind::Scenario, "scn.2027-graduation")?;
    let mapping = node(DependentKind::CourseMapping, "map.cse-substitution")?;
    let untouched_requirement = node(DependentKind::Requirement, "req.liberal-arts")?;
    let untouched_scenario = node(DependentKind::Scenario, "scn.minor-plan")?;

    let mut graph = DependencyGraph::new();
    graph.record(
        requirement.clone(),
        Dependency::Rule(RuleId::new("r-12-1")?),
    );
    graph.record(mapping.clone(), Dependency::Rule(RuleId::new("r-13-1")?));
    // Transitive: the scenario cites the requirement, not the rule.
    graph.record(scenario.clone(), Dependency::Node(requirement.clone()));
    // A branch nothing impacted reaches.
    graph.record(
        untouched_requirement.clone(),
        Dependency::Rule(RuleId::new("r-99-9")?),
    );
    graph.record(
        untouched_scenario.clone(),
        Dependency::Node(untouched_requirement.clone()),
    );

    // One impacted rule. Its requirement and the scenario above it, and
    // nothing else -- the course mapping hangs off another rule.
    let mut expected = vec![requirement.clone(), scenario.clone()];
    expected.sort();
    assert_eq!(
        graph.invalidate(&[RuleId::new("r-12-1")?]).nodes(),
        expected,
        "the invalidation is not exactly the transitive dependents"
    );

    // The other rule reaches only the mapping. Under-invalidation and
    // over-invalidation are both visible because the whole set is compared.
    assert_eq!(
        graph.invalidate(&[RuleId::new("r-13-1")?]).nodes(),
        core::slice::from_ref(&mapping)
    );

    // A rule nothing depends on invalidates nothing.
    assert!(graph.invalidate(&[RuleId::new("r-00-0")?]).is_empty());

    // Both rules together reach the union and still not the untouched branch.
    let mut both = vec![requirement.clone(), scenario.clone(), mapping.clone()];
    both.sort();
    assert_eq!(
        graph
            .invalidate(&[RuleId::new("r-12-1")?, RuleId::new("r-13-1")?])
            .nodes(),
        both
    );

    // Every one of section 29.2's three kinds is reachable, so the graph is
    // not silently a requirement-only index.
    let all = graph.invalidate(&[
        RuleId::new("r-12-1")?,
        RuleId::new("r-13-1")?,
        RuleId::new("r-99-9")?,
    ]);
    for kind in DependentKind::ALL {
        assert!(
            !all.of_kind(kind).is_empty(),
            "no {} was ever invalidated",
            kind.as_str()
        );
    }

    // A cycle terminates rather than looping.
    let mut cyclic = DependencyGraph::new();
    cyclic.record(
        requirement.clone(),
        Dependency::Rule(RuleId::new("r-12-1")?),
    );
    cyclic.record(scenario.clone(), Dependency::Node(requirement.clone()));
    cyclic.record(requirement.clone(), Dependency::Node(scenario.clone()));
    assert_eq!(
        cyclic.invalidate(&[RuleId::new("r-12-1")?]).nodes(),
        expected
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Conflicts
// ---------------------------------------------------------------------------

/// Builds the two contenders one pair of fixtures produces.
fn contenders(
    left: &DocumentFixture,
    right: &DocumentFixture,
    rule: &str,
) -> Result<(ContendingSource, ContendingSource), Box<dyn Error>> {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let read = |fixture: &DocumentFixture| -> Result<_, Box<dyn Error>> {
        let snapshot = snapshot_of(&manifest, &ledger, body(fixture.bytes(), "\"v1\"")?)?;
        Ok(academic_ingestion::document::parse(&snapshot)?)
    };
    let id = RuleId::new(rule)?;
    let first = read(left)?;
    let second = read(right)?;
    Ok((
        ContendingSource::from_document(connector(CONNECTOR)?, CATALOGUE, &first, &id)
            .ok_or("the left fixture has no such rule")?,
        ContendingSource::from_document(connector(CONNECTOR)?, BYLAW, &second, &id)
            .ok_or("the right fixture has no such rule")?,
    ))
}

/// A conflict case carries one finding for each of section 8.4's five
/// dimensions, and nothing that names a winner.
#[test]
fn conflict_case_dimensions() -> TestResult {
    let (left, right) = contenders(
        &DocumentFixture::dated().issued_by("UNIVERSITY_STATUTE"),
        &DocumentFixture::dated()
            .issued_by("DEPARTMENT_RULE")
            .effective_on("2026-09-01")
            .for_cohorts("2023-2025")
            .transitioning("SILENT")
            .with_rule_text("r-12-1", "major electives require twenty-four credits"),
        "r-12-1",
    )?;

    let case =
        academic_ingestion::detect(left, right).ok_or("the two documents did not conflict")?;

    // The five, in the specification's order, once each.
    let dimensions: Vec<ConflictDimension> = case
        .findings()
        .iter()
        .map(academic_ingestion::DimensionFinding::dimension)
        .collect();
    assert_eq!(
        dimensions,
        ConflictDimension::ALL.to_vec(),
        "a conflict case does not compare section 8.4's five dimensions"
    );

    // And each finding is the named relation the two documents really stand in.
    let outcome = |dimension| {
        case.finding(dimension)
            .map(academic_ingestion::DimensionFinding::outcome)
    };
    assert_eq!(
        outcome(ConflictDimension::LegalHierarchy),
        Some(DimensionOutcome::Hierarchy(
            HierarchyRelation::LeftIsSuperior
        ))
    );
    assert_eq!(
        outcome(ConflictDimension::IssuanceDate),
        Some(DimensionOutcome::Issuance(DateComparison::Stated(
            DateRelation::Same
        )))
    );
    assert_eq!(
        outcome(ConflictDimension::EffectiveDate),
        Some(DimensionOutcome::Effective(DateComparison::Stated(
            DateRelation::Earlier
        )))
    );
    assert_eq!(
        outcome(ConflictDimension::TargetScope),
        Some(DimensionOutcome::Scope(ScopeRelation::LeftContainsRight))
    );
    assert_eq!(
        outcome(ConflictDimension::TransitionalMeasures),
        Some(DimensionOutcome::Transition(
            TransitionRelation::OnlyLeftProvides
        ))
    );

    // `IN05`. The case is open, and a dependent audit may not conclude.
    assert_eq!(
        case.resolution(),
        &academic_ingestion::Resolution::Unresolved
    );
    assert_eq!(case.disposition(), AuditDisposition::Indeterminate);
    assert_eq!(case.disposition().as_str(), "INDETERMINATE");

    // Only a recorded human decision closes it.
    let mut resolved = case.clone();
    resolved.resolve(UserResolution::recorded(
        Side::Left,
        DependentId::new("user")?,
    ));
    assert_eq!(resolved.disposition(), AuditDisposition::Determinate);
    Ok(())
}

/// The five dimensions are section 8.4's own, and so is the sentence that
/// refuses a numeric winner.
#[test]
fn the_conflict_dimensions_are_section_8_4s_own() -> TestResult {
    let specification = specification()?;
    const SENTENCE: &str = "source가 충돌하면 더 높은 번호/낮은 번호로 기계적 승자를 정하지 않는다. 규정의 법적 위계, 발령일, 적용일, 대상 scope, 경과조치를 비교하고 `ConflictCase`를 만든다.";
    assert!(
        specification.contains(SENTENCE),
        "section 8.4's conflict sentence changed; the five dimensions must change with it"
    );

    // The specification writes them in Korean; this is the mapping, and the
    // order is checked by walking the sentence forwards.
    const NAMED: [(&str, ConflictDimension); 5] = [
        ("규정의 법적 위계", ConflictDimension::LegalHierarchy),
        ("발령일", ConflictDimension::IssuanceDate),
        ("적용일", ConflictDimension::EffectiveDate),
        ("대상 scope", ConflictDimension::TargetScope),
        ("경과조치", ConflictDimension::TransitionalMeasures),
    ];
    let declared: Vec<ConflictDimension> = NAMED.iter().map(|(_, dimension)| *dimension).collect();
    assert_eq!(
        declared,
        ConflictDimension::ALL.to_vec(),
        "the mapping does not cover the five dimensions in order"
    );
    let mut cursor = 0;
    for (korean, _) in NAMED {
        let at = SENTENCE[cursor..]
            .find(korean)
            .ok_or_else(|| format!("the sentence does not name {korean}"))?;
        cursor += at + korean.len();
    }
    Ok(())
}

/// `IN05`. A conflict queues the document instead of publishing it.
#[test]
fn in05_two_official_sources_conflict() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let (_, established) = contenders(
        &DocumentFixture::dated(),
        &DocumentFixture::dated()
            .issued_by("UNIVERSITY_STATUTE")
            .with_rule_text("r-12-1", "major electives require twenty-four credits"),
        "r-12-1",
    )?;
    let known = corpus()?.with_contender(established);

    let publication = one_run(
        &manifest,
        &ledger,
        &known,
        Acquisition::Import {
            target: CATALOGUE,
            outcome: body(DocumentFixture::dated().bytes(), "\"v1\"")?,
        },
    )?;
    assert!(
        publication.published().is_none(),
        "a document that conflicts with an official source published anyway"
    );
    let queued = publication.queued().ok_or("nothing was queued")?;
    assert_eq!(queued.reason(), QueueReason::UnresolvedConflict);
    let case = queued.conflicts().first().ok_or("no case was opened")?;
    assert_eq!(case.disposition(), AuditDisposition::Indeterminate);

    // The control: an established source that says the same thing is not a
    // conflict, and the document publishes.
    let (_, agreeing) = contenders(
        &DocumentFixture::dated(),
        &DocumentFixture::dated().issued_by("UNIVERSITY_STATUTE"),
        "r-12-1",
    )?;
    let agreeing_corpus = corpus()?.with_contender(agreeing);
    let published = one_run(
        &manifest,
        &ledger,
        &agreeing_corpus,
        Acquisition::Import {
            target: CATALOGUE,
            outcome: body(DocumentFixture::dated().bytes(), "\"v1\"")?,
        },
    )?;
    assert!(
        published.published().is_some(),
        "two sources that agree were read as a conflict"
    );

    // And a disjoint scope is not a conflict either: different cohorts are
    // different rules, not competing ones.
    let (_, other_cohort) = contenders(
        &DocumentFixture::dated(),
        &DocumentFixture::dated()
            .for_cohorts("2019-2021")
            .with_rule_text("r-12-1", "major electives require twenty-four credits"),
        "r-12-1",
    )?;
    let disjoint = corpus()?.with_contender(other_cohort);
    assert!(
        one_run(
            &manifest,
            &ledger,
            &disjoint,
            Acquisition::Import {
                target: CATALOGUE,
                outcome: body(DocumentFixture::dated().bytes(), "\"v1\"")?,
            },
        )?
        .published()
        .is_some(),
        "two documents about different cohorts were read as a conflict"
    );
    Ok(())
}

/// Neither side of a case is privileged by the order it was passed in.
///
/// The behavioural half of `no_numeric_source_winner`: swapping the arguments
/// mirrors every finding and changes nothing else, so "the first one" is not a
/// tiebreak the caller can reach by argument order.
#[test]
fn a_conflict_case_has_no_privileged_side() -> TestResult {
    let (left, right) = contenders(
        &DocumentFixture::dated().issued_by("UNIVERSITY_STATUTE"),
        &DocumentFixture::dated()
            .issued_by("DEPARTMENT_RULE")
            .effective_on("2026-09-01")
            .with_rule_text("r-12-1", "major electives require twenty-four credits"),
        "r-12-1",
    )?;
    let forwards = ConflictCase::open(left.clone(), right.clone());
    let backwards = ConflictCase::open(right, left);

    assert_eq!(forwards.disposition(), backwards.disposition());
    assert_eq!(
        forwards.disposition(),
        AuditDisposition::Indeterminate,
        "one ordering of the same two documents concluded"
    );
    assert_eq!(
        forwards
            .finding(ConflictDimension::LegalHierarchy)
            .map(academic_ingestion::DimensionFinding::outcome),
        Some(DimensionOutcome::Hierarchy(
            HierarchyRelation::LeftIsSuperior
        ))
    );
    assert_eq!(
        backwards
            .finding(ConflictDimension::LegalHierarchy)
            .map(academic_ingestion::DimensionFinding::outcome),
        Some(DimensionOutcome::Hierarchy(
            HierarchyRelation::RightIsSuperior
        )),
        "the hierarchy relation did not mirror; one side is privileged"
    );
    Ok(())
}

/// The declared legal hierarchy is a strict partial order, and it really has an
/// incomparable pair.
#[test]
fn the_legal_hierarchy_is_a_reviewed_relation() -> TestResult {
    for left in LegalAuthority::ALL {
        assert_eq!(
            left.hierarchy_relation(left),
            HierarchyRelation::SameLevel,
            "an authority is superior to itself"
        );
        for right in LegalAuthority::ALL {
            let forwards = left.hierarchy_relation(right);
            let backwards = right.hierarchy_relation(left);
            let mirrored = match forwards {
                HierarchyRelation::LeftIsSuperior => HierarchyRelation::RightIsSuperior,
                HierarchyRelation::RightIsSuperior => HierarchyRelation::LeftIsSuperior,
                other => other,
            };
            assert_eq!(backwards, mirrored, "the relation is not antisymmetric");

            // Transitivity, spelled out rather than assumed from an order.
            if forwards == HierarchyRelation::LeftIsSuperior {
                for lower in LegalAuthority::ALL {
                    if right.hierarchy_relation(lower) == HierarchyRelation::LeftIsSuperior {
                        assert_eq!(
                            left.hierarchy_relation(lower),
                            HierarchyRelation::LeftIsSuperior,
                            "the table is not transitive"
                        );
                    }
                }
            }
        }
    }

    // The incomparable pair is real, so `NotComparable` is not an unreachable
    // arm that makes the relation a total order in disguise.
    assert_eq!(
        LegalAuthority::UniversityStatute
            .hierarchy_relation(LegalAuthority::ExternalAccreditationStandard),
        HierarchyRelation::NotComparable
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Denials and fallbacks
// ---------------------------------------------------------------------------

/// Every denial offers the four fallbacks and routes nowhere else.
#[test]
fn manual_and_export_fallbacks_are_offered_when_denied() -> TestResult {
    // Every reason, not one of them.
    for reason in DenialReason::ALL {
        let denial: Denial = academic_ingestion::terms::deny(connector(CONNECTOR)?, reason);
        assert_eq!(
            denial.fallbacks(),
            Fallback::ALL,
            "a {} denial offered a different set of fallbacks",
            reason.as_str()
        );
        assert_eq!(
            denial.route(),
            DenialRoute::ManualOrStop,
            "a {} denial routed somewhere other than the fallbacks",
            reason.as_str()
        );
    }

    // And the denials a real run produces carry them too, which is what makes
    // the loop above a statement about the pipeline rather than about `deny`.
    for stage in [
        Stage::DiscoverFetchImport,
        Stage::PolicyAndTermsCheck,
        Stage::SourceMetadataAndRetrievalTime,
        Stage::ClaimPublicationOrReviewQueue,
    ] {
        let driven = drive(Some(stage))?;
        let failure = driven.failure.ok_or("the stage did not fail")?;
        let denial = failure
            .denial()
            .ok_or_else(|| format!("{} failed without a denial", stage.as_str()))?;
        assert_eq!(denial.fallbacks(), Fallback::ALL);
        assert_eq!(denial.route(), DenialRoute::ManualOrStop);
    }

    // The four are what Phase 2 ships against `GATE-38-027`, and each is an
    // action a person takes.
    assert_eq!(phase2_shipped_fallbacks(), Fallback::ALL);
    assert_eq!(
        Fallback::ALL.map(Fallback::as_str),
        [
            "MANUAL_PASTE",
            "USER_PROVIDED_EXPORT",
            "SAVE_FROM_YOUR_OWN_BROWSER",
            "LOW_FREQUENCY_MANUAL_SYNC",
        ]
    );
    Ok(())
}

/// `IN06`. A permission withdrawn during a run stops that run, disables the
/// connector, and offers the fallbacks.
#[test]
fn in06_connector_terms_revoked_mid_run() -> TestResult {
    let driven = drive(Some(Stage::ClaimPublicationOrReviewQueue))?;
    let failure = driven
        .failure
        .ok_or("the run published under a revocation")?;
    assert_eq!(failure.stage(), Stage::ClaimPublicationOrReviewQueue);
    assert!(driven.publication.is_none());

    let denial = failure
        .denial()
        .ok_or("the revocation produced no denial")?;
    assert_eq!(denial.reason(), DenialReason::TermsRevoked);
    assert!(
        denial.connector_disabled(),
        "a revoked connector stayed enabled"
    );
    assert_eq!(denial.fallbacks(), Fallback::ALL);

    // A cadence denial does not disable the connector: the terms still permit
    // the source, the clock does not permit the fetch yet.
    let too_soon = academic_ingestion::terms::deny(connector(CONNECTOR)?, DenialReason::TooSoon);
    assert!(!too_soon.connector_disabled());
    Ok(())
}

/// `GATE-38-020` and `GATE-38-027` are open, and an unreviewed source denies.
#[test]
fn the_two_open_gates_have_no_default() -> TestResult {
    assert_eq!(
        OpenGate::ALL.map(OpenGate::identifier),
        ["GATE-38-020", "GATE-38-027"]
    );
    for gate in OpenGate::ALL {
        assert!(
            gate.statement().contains(gate.identifier()),
            "a gate's statement does not name it"
        );
    }

    // An empty ledger is what "open" looks like: no record, and no fetch.
    let empty = TermsLedger::new();
    assert_eq!(empty.status(&connector(CONNECTOR)?), unreviewed_status());
    assert!(!unreviewed_status().permits_a_fetch());
    for status in TermsStatus::ALL {
        assert_eq!(
            status.permits_a_fetch(),
            status == TermsStatus::PermittedForDeclaredMethod,
            "{} permits a fetch",
            status.as_str()
        );
    }
    Ok(())
}

/// A target the manifest does not declare is refused at both places it can
/// arrive.
#[test]
fn an_undeclared_target_is_refused() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    assert_eq!(
        academic_ingestion::ConditionalRequest::anonymous(
            &manifest,
            UNDECLARED,
            academic_ingestion::Validators::none(),
        )
        .err()
        .map(|denial| denial.reason()),
        Some(DenialReason::UndeclaredTarget)
    );

    // And an import of an undeclared document is refused at stage two, which
    // is the route that does not pass through a request at all.
    let ledger = permitting_ledger(CONNECTOR)?;
    let fetched = stage::discover_fetch_import(
        &manifest,
        &ledger,
        RETRIEVED_AT,
        Acquisition::Import {
            target: UNDECLARED,
            outcome: body(DocumentFixture::dated().bytes(), "\"v1\"")?,
        },
    )?;
    let refused = stage::policy_and_terms_check(fetched, &manifest, &ledger);
    assert_eq!(
        refused
            .err()
            .and_then(|failure| failure.denial().map(Denial::reason)),
        Some(DenialReason::UndeclaredTarget)
    );
    Ok(())
}

/// The whole pipeline, once, through `run`.
#[test]
fn a_complete_run_publishes_what_the_document_says() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let known = corpus()?;
    let source = FixtureSource::holding(DocumentFixture::dated().bytes(), "\"v1\"");
    let record = academic_ingestion::run(
        &manifest,
        &ledger,
        &known,
        RETRIEVED_AT,
        Acquisition::Fetch {
            transport: &source,
            request: academic_ingestion::ConditionalRequest::anonymous(
                &manifest,
                CATALOGUE,
                academic_ingestion::Validators::none(),
            )?,
        },
        IngestSeq::at(7),
        Appropriateness::NotAppropriate,
    );
    let published = record.published().ok_or("the run published nothing")?;
    assert_eq!(record.reached(), Stage::ALL);
    assert_eq!(published.connector().as_str(), CONNECTOR);
    assert_eq!(published.parser_version(), PARSER);
    assert_eq!(published.retrieved_at(), RETRIEVED_AT);
    assert_eq!(published.scope().program().as_str(), "cse");
    assert_eq!(
        published.rules(),
        [
            RuleId::new("r-12-1")?,
            RuleId::new("r-12-2")?,
            RuleId::new("r-13-1")?
        ]
    );
    assert_eq!(published.effective().to_string(), "2026-03-01");

    // The same bytes read twice produce the same document: the parse is
    // deterministic in the sense section 29.1 means.
    let again = one_run(
        &manifest,
        &ledger,
        &known,
        Acquisition::Import {
            target: CATALOGUE,
            outcome: body(DocumentFixture::dated().bytes(), "\"v1\"")?,
        },
    )?;
    assert_eq!(again.published(), Some(published));
    Ok(())
}

/// A credential is bound to one connector's declarations, and a connector that
/// holds none cannot mint one.
#[test]
fn a_credential_is_bound_to_one_declaration() -> TestResult {
    let public = manifest(CONNECTOR)?;
    assert!(
        public.credential_binding().is_none(),
        "a public-page connector minted a credential binding"
    );

    let api = draft(CONNECTOR)?
        .authentication_method(academic_ingestion::AuthenticationMethod::ScopedOfficialApiToken)
        .build()?;
    let binding = api
        .credential_binding()
        .ok_or("an official-API connector minted no binding")?;
    assert_eq!(binding.connector(), api.connector());

    // Bound to a document this connector declares.
    academic_ingestion::ConditionalRequest::credentialed(
        &api,
        api.credential_binding().ok_or("no binding")?,
        CATALOGUE,
        academic_ingestion::Validators::none(),
    )?;

    // Not to one it does not.
    assert_eq!(
        academic_ingestion::ConditionalRequest::credentialed(
            &api,
            api.credential_binding().ok_or("no binding")?,
            UNDECLARED,
            academic_ingestion::Validators::none(),
        )
        .err()
        .map(|denial| denial.reason()),
        Some(DenialReason::UndeclaredTarget)
    );

    // And not to another connector's document.
    let other = draft("snu.eng.official")?
        .authentication_method(academic_ingestion::AuthenticationMethod::ScopedOfficialApiToken)
        .build()?;
    assert_eq!(
        academic_ingestion::ConditionalRequest::credentialed(
            &api,
            other.credential_binding().ok_or("no binding")?,
            CATALOGUE,
            academic_ingestion::Validators::none(),
        )
        .err()
        .map(|denial| denial.reason()),
        Some(DenialReason::UndeclaredTarget)
    );

    // The binding prints the connector and nothing else.
    let printed = format!("{binding:?}");
    assert!(printed.contains(CONNECTOR));
    assert!(!printed.contains("token"));

    // A user-supplied export holds no credential: the person authenticated,
    // not this system.
    assert!(
        draft(CONNECTOR)?
            .authentication_method(academic_ingestion::AuthenticationMethod::UserSuppliedExport)
            .build()?
            .credential_binding()
            .is_none()
    );
    Ok(())
}

/// A header value read out of a response cannot carry a separator back into a
/// request.
#[test]
fn a_validator_cannot_carry_a_separator() -> TestResult {
    use academic_ingestion::HeaderValue;
    assert!(HeaderValue::new("\"v1\"").is_ok());
    for refused in [
        "tag\r\nX-Injected: 1",
        "tag\nX-Injected: 1",
        "tag\0",
        "\u{1f600}",
    ] {
        assert!(
            HeaderValue::new(refused).is_err(),
            "a header value accepted {refused:?}"
        );
    }
    assert!(HeaderValue::new("").is_err());
    assert!(HeaderValue::new("v".repeat(200)).is_err());
    Ok(())
}

/// The declared cadence refuses a fetch that is too soon, and an import never.
///
/// A cadence nothing compares against a clock is a declaration rather than a
/// limit, and section 29.2 asks for a *low-frequency* fetch.
#[test]
fn the_declared_cadence_limits_a_fetch_and_not_an_import() -> TestResult {
    use academic_ingestion::{AllowedFrequency, LastSuccess};

    let known = corpus()?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let a_day_ago = RetrievalInstant::at(RETRIEVED_AT.seconds() - 86_400);
    let weekly = draft(CONNECTOR)?
        .allowed_frequency(AllowedFrequency::Weekly)
        .last_success(LastSuccess::At(a_day_ago))
        .build()?;
    let source = FixtureSource::holding(DocumentFixture::dated().bytes(), "\"v1\"");
    let fetch = |manifest: &academic_ingestion::ConnectorManifest| {
        academic_ingestion::ConditionalRequest::anonymous(
            manifest,
            CATALOGUE,
            academic_ingestion::Validators::none(),
        )
    };

    // A day after the last success, under a weekly cadence, is too soon.
    let record = academic_ingestion::run(
        &weekly,
        &ledger,
        &known,
        RETRIEVED_AT,
        Acquisition::Fetch {
            transport: &source,
            request: fetch(&weekly)?,
        },
        IngestSeq::at(1),
        Appropriateness::NotAppropriate,
    );
    let failure = record
        .failure()
        .ok_or("a weekly connector fetched after a day")?;
    assert_eq!(failure.stage(), Stage::DiscoverFetchImport);
    let denial = failure.denial().ok_or("the cadence produced no denial")?;
    assert_eq!(denial.reason(), DenialReason::TooSoon);
    assert_eq!(denial.fallbacks(), Fallback::ALL);
    // The terms still permit the source, so the connector stays enabled.
    assert!(!denial.connector_disabled());
    assert!(record.published().is_none());

    // A week later it is not too soon, and the same run publishes.
    let later = RetrievalInstant::at(a_day_ago.seconds() + 8 * 86_400);
    assert!(
        academic_ingestion::run(
            &weekly,
            &ledger,
            &known,
            later,
            Acquisition::Fetch {
                transport: &source,
                request: fetch(&weekly)?,
            },
            IngestSeq::at(2),
            Appropriateness::NotAppropriate,
        )
        .published()
        .is_some(),
        "a fetch a week after the last success was still refused"
    );

    // An import is a person handing over a file, and the cadence is a rule
    // about how often this system asks a source. It publishes at the same
    // instant the fetch was refused at.
    assert!(
        academic_ingestion::run(
            &weekly,
            &ledger,
            &known,
            RETRIEVED_AT,
            Acquisition::Import {
                target: CATALOGUE,
                outcome: body(DocumentFixture::dated().bytes(), "\"v1\"")?,
            },
            IngestSeq::at(3),
            Appropriateness::NotAppropriate,
        )
        .published()
        .is_some(),
        "the cadence refused a file a person handed over"
    );

    // A connector that has never succeeded has nothing to count from.
    let never = draft(CONNECTOR)?
        .allowed_frequency(AllowedFrequency::Weekly)
        .last_success(LastSuccess::Never)
        .build()?;
    assert!(
        academic_ingestion::run(
            &never,
            &ledger,
            &known,
            RETRIEVED_AT,
            Acquisition::Fetch {
                transport: &source,
                request: fetch(&never)?,
            },
            IngestSeq::at(4),
            Appropriateness::NotAppropriate,
        )
        .published()
        .is_some(),
        "a connector that has never succeeded was refused as too soon"
    );

    // And one with no schedule is never early: a run is the user asking.
    let on_request = draft(CONNECTOR)?
        .allowed_frequency(AllowedFrequency::OnUserRequestOnly)
        .last_success(LastSuccess::At(a_day_ago))
        .build()?;
    assert!(
        academic_ingestion::run(
            &on_request,
            &ledger,
            &known,
            RETRIEVED_AT,
            Acquisition::Fetch {
                transport: &source,
                request: fetch(&on_request)?,
            },
            IngestSeq::at(5),
            Appropriateness::NotAppropriate,
        )
        .published()
        .is_some(),
        "a connector with no schedule was refused for being early"
    );
    Ok(())
}

/// The cadence is named, and the one that has no schedule says so.
#[test]
fn the_allowed_frequency_has_no_hidden_default() -> TestResult {
    use academic_ingestion::AllowedFrequency;
    let last = RetrievalInstant::at(1_000_000);
    assert_eq!(
        AllowedFrequency::OnUserRequestOnly.earliest_next(last),
        None,
        "a connector with no schedule was given one"
    );
    for cadence in [
        AllowedFrequency::Daily,
        AllowedFrequency::Weekly,
        AllowedFrequency::PerTerm,
    ] {
        let next = cadence
            .earliest_next(last)
            .ok_or("a scheduled cadence produced no next time")?;
        assert!(next.seconds() > last.seconds());
    }
    Ok(())
}

/// A transport that produces nothing halts the run at stage one.
///
/// The detail is the caller's, kept as opaque text: nothing in this crate reads
/// a transport error beyond stopping there, and a run that stopped at stage one
/// has published nothing by construction.
#[test]
fn a_transport_failure_halts_at_stage_one() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let known = corpus()?;
    let source = FixtureSource::failing("the fixture transport was asked for nothing");
    let record = academic_ingestion::run(
        &manifest,
        &ledger,
        &known,
        RETRIEVED_AT,
        Acquisition::Fetch {
            transport: &source,
            request: academic_ingestion::ConditionalRequest::anonymous(
                &manifest,
                CATALOGUE,
                academic_ingestion::Validators::none(),
            )?,
        },
        IngestSeq::at(1),
        Appropriateness::NotAppropriate,
    );
    assert_eq!(record.reached(), [Stage::DiscoverFetchImport]);
    assert!(record.published().is_none());
    let failure = record.failure().ok_or("the run did not halt")?;
    assert_eq!(failure.stage(), Stage::DiscoverFetchImport);
    assert!(
        matches!(
            failure.reason(),
            academic_ingestion::FailureReason::Transport(_)
        ),
        "the halt was not attributed to the transport"
    );
    Ok(())
}

/// A `304` and an unchanged body are different answers to the same question.
#[test]
fn a_not_modified_response_creates_no_version() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    assert!(snapshot_of(&manifest, &ledger, not_modified("\"v1\"")?).is_err());
    assert!(
        snapshot_of(
            &manifest,
            &ledger,
            body(DocumentFixture::dated().bytes(), "\"v1\"")?
        )
        .is_ok()
    );
    Ok(())
}

/// The parse is a total function into a document or a named error.
#[test]
fn the_parse_reports_where_it_stopped() -> TestResult {
    let manifest = manifest(CONNECTOR)?;
    let ledger = permitting_ledger(CONNECTOR)?;
    let read = |bytes: Vec<u8>| -> Result<_, Box<dyn Error>> {
        let snapshot = snapshot_of(&manifest, &ledger, body(bytes, "\"v1\"")?)?;
        Ok(academic_ingestion::document::parse(&snapshot).err())
    };

    assert!(read(b"a line with no directive\n".to_vec())?.is_some());
    assert!(read(b"AUTHORITY: NOT_AN_AUTHORITY\n".to_vec())?.is_some());
    assert!(read(b"AUTHORITY: DEPARTMENT_RULE\nPROGRAM: cse\nRULE: r-1 | x\n".to_vec())?.is_some());
    assert!(read(b"PROGRAM: cse\nSECTION: a\nRULE: r-1 | x\n".to_vec())?.is_some());
    assert!(read(DocumentFixture::dated().bytes())?.is_none());

    // A rule identifier that is not `[A-Za-z0-9._-]` is refused, so a name
    // lifted out of an untrusted document cannot carry a separator.
    assert!(
        read(
            b"AUTHORITY: DEPARTMENT_RULE\nPROGRAM: cse\nSECTION: a\nRULE: r 1\nis bad | x\n"
                .to_vec()
        )?
        .is_some()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// `S-20`
// ---------------------------------------------------------------------------

/// Section 38's cells, in the order the document writes them.
///
/// Section 38.1's ten lines, then section 38.2's eleven bullets, then section
/// 38.3's ten numbered questions. `GATE-38-{:03}` is one-based over that
/// concatenation, so a cell's identifier is a fact about where the document
/// puts it and not a string anybody chose.
fn section_38_cells() -> Result<Vec<String>, Box<dyn Error>> {
    let specification = specification()?;
    let block = specification
        .split_once("Admission Year")
        .map(|(_, rest)| format!("Admission Year{rest}"))
        .and_then(|rest| rest.split_once("```").map(|(block, _)| block.to_owned()))
        .ok_or("section 38.1's block is not in the document")?;
    let mut cells: Vec<String> = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(cells.len(), 10, "section 38.1 lists {} lines", cells.len());

    let bullets = specification
        .split_once("### 38.2 공식적으로 추가 확인할 항목")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("### 38.3").map(|(block, _)| block))
        .ok_or("section 38.2's list is not in the document")?;
    let bullets: Vec<String> = bullets
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- ").map(str::to_owned))
        .collect();
    assert_eq!(
        bullets.len(),
        11,
        "section 38.2 lists {} bullets",
        bullets.len()
    );

    let questions = specification
        .split_once("### 38.3 아직 결정할 제품·아키텍처 질문")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("\n---").map(|(block, _)| block))
        .ok_or("section 38.3's list is not in the document")?;
    let questions: Vec<String> = questions
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.split_once(". ")
                .and_then(|(number, text)| number.parse::<usize>().ok().map(|_| text.to_owned()))
        })
        .collect();
    assert_eq!(
        questions.len(),
        10,
        "section 38.3 lists {} questions",
        questions.len()
    );

    cells.extend(bullets);
    cells.extend(questions);
    Ok(cells)
}

/// Where section 38 writes one cell, one-based.
///
/// The identifier is never compared against a list somebody typed: it is
/// rebuilt from the position and the two have to agree.
fn section_38_position(
    cells: &[String],
    identifier: &str,
    spec_line: &str,
) -> Result<usize, Box<dyn Error>> {
    Ok(cells
        .iter()
        .position(|cell| cell.starts_with(spec_line))
        .ok_or_else(|| {
            format!("{identifier} quotes a line section 38 does not write: {spec_line}")
        })?
        .saturating_add(1))
}

/// The `GATE-38-xxx` identifiers are section 38's own numbering, derived from
/// each cell's position in the document rather than compared against a list
/// written twice.
///
/// `S-20`: eleven of this workspace's eighteen `OpenGate::identifier` arms were
/// hand-written strings whose only check was a hand-written list in the same
/// test, so the first edit to section 38 that inserts, removes or reorders a
/// cell renumbered the ones after it silently. `P2-U3` closed it for `academic-audit`'s seven cells; these are this
/// crate's two, and `GATE-38-027` is the one of the eighteen that section
/// 38.3 numbers rather than 38.1 or 38.2.
#[test]
fn the_open_gates_are_section_38s_own() -> TestResult {
    let cells = section_38_cells()?;
    let mut derived: Vec<&'static str> = Vec::new();
    for (gate, spec_line) in [
        (
            OpenGate::SourceTermsAndRateLimits,
            "LMS와 수강신청 사이트의 자동화/API/export 이용약관·robots·rate limit.",
        ),
        (
            OpenGate::ManualExportVersusAssistedCapture,
            "합법적·안정적인 SNU official data interface가 없다면 manual export와 browser-assisted capture 중 허용 가능한 경계는 어디인가?",
        ),
    ] {
        let position = section_38_position(&cells, gate.identifier(), spec_line)?;
        assert_eq!(
            gate.identifier(),
            format!("GATE-38-{position:03}"),
            "{} is section 38's cell {position}, so its identifier does not follow its position",
            gate.identifier()
        );
        derived.push(gate.identifier());
    }

    // Both directions: a variant added to `ALL` without a section 38 cell above
    // is a missing key here, and a cell derived for a variant `ALL` dropped is
    // an extra one.
    let declared: Vec<&'static str> = OpenGate::ALL.iter().map(|gate| gate.identifier()).collect();
    assert_eq!(
        derived, declared,
        "the derived cells are not the ones this crate declares"
    );
    Ok(())
}
