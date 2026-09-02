//! `P2-M1`'s named acceptance evidence for the record, the reconciliation, and
//! the calibration boundary.
//!
//! The two remaining rows -- `reanalysis_creates_new_candidate_not_mutation`
//! and `reanalysis_diff_links_both_model_runs` -- are in
//! `crates/store/src/model_run_closure_tests.rs`, because what they assert is
//! that migration `0007`'s tables append and never edit, and those tables exist
//! only on a migrated database.

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_model_run::{
    ArtifactId, CalibratedConfidence, CalibrationBin, CalibrationDataset, CalibrationDatasetId,
    CalibrationRegistry, Cost, Digest32, DisplayedConfidence, EgressGrantId, InputArtifactRef,
    InputArtifactRefs, ModelRun, ModelRunError, ModelRunId, ModelVersion, ProviderId, Purpose,
    RawScore, RetentionDeclaration, Transmission, TransmittedRange, reconcile::ReconciliationError,
    reconcile_transmitted_ranges,
};
use academic_policy::{
    AuditRow, ConsumptionRow, ContentDigest, Decision, EgressRule, ObjectRange, PermissionBroker,
    PermissionRequest, PolicySnapshot, PolicyVersion, ProcessActivity, ProcessCapability,
    ProcessClass, ProviderIdentity, ProviderPolicyDraft, ProviderPolicySnapshot, ProviderSurface,
    RuntimeToolCall,
};

const PAYLOAD: &[u8] = b"allowed!";
const OBJECT_ID: &str = "synthetic-object";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .map(PathBuf::from)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// model_run_requires_every_field
// ---------------------------------------------------------------------------

/// Where each section 27.3 field is stored by migration `0007`.
///
/// The Rust half needs no table -- the struct's field names are the spec's YAML
/// keys in snake case, so the expected set is derived from the spec rather than
/// transcribed. The storage half does need one, because two of the twelve are
/// lists and one is an enumeration over two columns. Each entry names every
/// site that carries the field; the test requires the whole map to be exactly
/// the spec's key set and every named site to be present in the migration.
const STORAGE_SITES: [(&str, &[&str]); 12] = [
    ("id", &["model_run_provenance.model_run_id"]),
    ("purpose", &["model_run_provenance.purpose_id"]),
    ("provider", &["model_run_provenance.provider_id"]),
    ("modelVersion", &["model_run_provenance.model_version"]),
    (
        "promptTemplateHash",
        &["model_run_provenance.prompt_template_hash"],
    ),
    ("inputArtifactRefs", &["model_run_input_artifact"]),
    (
        "transmittedByteRanges",
        &[
            "model_run_provenance.transmission_kind",
            "model_run_provenance.transmitted_grant_id",
            "model_run_transmitted_range",
        ],
    ),
    (
        "redactionPolicyHash",
        &["model_run_provenance.redaction_policy_hash"],
    ),
    (
        "outputArtifact",
        &["model_run_provenance.output_artifact_id"],
    ),
    ("startedAt", &["model_run_provenance.started_at"]),
    (
        "cost",
        &[
            "model_run_provenance.cost_micros",
            "model_run_provenance.cost_currency",
        ],
    ),
    (
        "retentionDeclaration",
        &["model_run_provenance.retention_declaration_id"],
    ),
];

/// The section 27.3 `ModelRun` YAML keys, read out of the authoritative spec.
///
/// Nothing here counts the keys. The set is whatever the spec block holds, and
/// the two comparisons below are set equality in both directions, so a
/// thirteenth key added to the spec fails this test rather than passing it.
fn spec_model_run_keys() -> Result<Vec<String>, Box<dyn Error>> {
    let spec = fs::read_to_string(
        repository_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?;
    let block = spec
        .split("```yaml\nModelRun:\n")
        .nth(1)
        .ok_or("the spec has no ModelRun YAML block")?;
    let block = block.split("```").next().ok_or("unterminated YAML block")?;
    let keys = block
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("   "))
        .filter_map(|line| line.trim().split(':').next())
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(
        keys.len() > 1,
        "the ModelRun block parsed to {} keys; the parser stopped reading the spec",
        keys.len()
    );
    Ok(keys)
}

/// The `ModelRun` struct's field names, read out of this crate's own source.
fn struct_field_names() -> Result<Vec<String>, Box<dyn Error>> {
    let source = fs::read_to_string(repository_root().join("crates/model-run/src/record.rs"))?;
    let declaration = source
        .split("pub struct ModelRun {\n")
        .nth(1)
        .ok_or("record.rs no longer declares ModelRun")?;
    let body = declaration
        .split("\n}")
        .next()
        .ok_or("unterminated ModelRun declaration")?;
    Ok(body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .filter_map(|line| line.split(':').next())
        .map(str::to_owned)
        .collect())
}

fn snake_case(camel: &str) -> String {
    let mut snake = String::with_capacity(camel.len() + 4);
    for character in camel.chars() {
        if character.is_ascii_uppercase() {
            snake.push('_');
            snake.push(character.to_ascii_lowercase());
        } else {
            snake.push(character);
        }
    }
    snake
}

#[test]
fn model_run_requires_every_field() -> Result<(), Box<dyn Error>> {
    let spec_keys = spec_model_run_keys()?;
    let expected_fields = spec_keys
        .iter()
        .map(|key| snake_case(key))
        .collect::<BTreeSet<_>>();
    let declared_fields = struct_field_names()?.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        declared_fields, expected_fields,
        "the ModelRun type's fields are not the section 27.3 keys in snake case"
    );

    // The storage map is the spec's key set, exactly, in both directions.
    let mapped_keys = STORAGE_SITES
        .iter()
        .map(|(key, _)| (*key).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        mapped_keys,
        spec_keys.iter().cloned().collect::<BTreeSet<_>>(),
        "the storage map and the spec's ModelRun keys differ"
    );

    // Every named storage site exists in migration 0007.
    let migration = fs::read_to_string(
        repository_root().join("migrations/store/0007_phase2_model_run_provenance.sql"),
    )?;
    let statements = migration
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for (key, sites) in STORAGE_SITES {
        for site in sites {
            let needle = site
                .split_once('.')
                .map_or_else(|| (*site).to_owned(), |(_, column)| column.to_owned());
            assert!(
                statements.contains(&needle),
                "section 27.3 field {key} names storage site {site}, which migration 0007 does not create"
            );
        }
    }

    // Drop one key at a time and require each comparison to notice. "Every"
    // means every: this loop is what stops the assertions above from passing
    // with a field silently missing from one of the three descriptions.
    for key in &spec_keys {
        let field = snake_case(key);
        let mut without_field = declared_fields.clone();
        assert!(
            without_field.remove(&field),
            "{field} is not among the declared fields"
        );
        assert_ne!(
            without_field, expected_fields,
            "dropping {field} from the struct's fields left the comparison passing"
        );

        let mut without_key = mapped_keys.clone();
        assert!(without_key.remove(key), "{key} is not in the storage map");
        assert_ne!(
            without_key,
            spec_keys.iter().cloned().collect::<BTreeSet<_>>(),
            "dropping {key} from the storage map left the comparison passing"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// transmitted_ranges_reconcile_with_egress_audit
// ---------------------------------------------------------------------------

fn digest(label: &str) -> ContentDigest {
    ContentDigest::of(label.as_bytes())
}

fn digest32(label: &str) -> Digest32 {
    Digest32::of(label.as_bytes())
}

fn policy_range(start: u64, end: u64, label: &str) -> Result<ObjectRange, Box<dyn Error>> {
    Ok(ObjectRange::new(OBJECT_ID, start, end, digest(label))?)
}

fn provider_identity() -> Result<ProviderIdentity, Box<dyn Error>> {
    Ok(ProviderIdentity::new(
        "provider-y",
        ProviderSurface::EnterpriseApi,
    )?)
}

fn provider_draft() -> Result<ProviderPolicyDraft, Box<dyn Error>> {
    Ok(ProviderPolicyDraft {
        identity: Some(provider_identity()?),
        training_use_enabled: Some(false),
        training_opt_out_applied: Some(false),
        server_retention_millis: Some(0),
        abuse_logging_enabled: Some(false),
        residency_regions: Some(vec!["kr".to_owned()]),
        subprocessors: Some(Vec::new()),
        transit_encryption_declared: Some(true),
        at_rest_encryption_declared: Some(true),
        deletion_api_available: Some(true),
        deletion_receipt_capable: Some(true),
        maximum_input_bytes: Some(1_024),
        logging_configuration: Some("content-logging-disabled".to_owned()),
        policy_source_digest: Some(digest("provider-y-policy-source")),
        last_verified_at: Some(0),
        ttl_millis: Some(10_000),
    })
}

fn configured_broker()
-> Result<(PermissionBroker, PolicyVersion, ProviderPolicySnapshot), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile_with_ttl(10_000)?;
    let provider = broker.register_provider_policy(provider_draft()?, 0)?;
    let configured = EgressRule {
        actor_id: "synthetic-user".to_owned(),
        process_class: ProcessClass::EgressProxy,
        data_class: "synthetic-private-code".to_owned(),
        operation: "classify".to_owned(),
        purpose_id: "concept-extraction".to_owned(),
        destination_id: provider.destination_id().to_owned(),
        retention_terms_hash: provider.retention_terms_hash(),
        consent_evidence_id: "synthetic-consent-event".to_owned(),
        valid_from: 100,
        valid_until: 1_000,
        minimal_ranges: vec![policy_range(10, 18, "slice")?],
        payload_digest: ContentDigest::of(PAYLOAD),
        provider_policy_snapshot_digest: provider.snapshot_digest().clone(),
        training_use_allowed: false,
        redaction_policy_hash: digest("redaction-policy-v1"),
    };
    let version = broker.install_policy(PolicySnapshot::from_rules(vec![configured])?)?;
    Ok((broker, version, provider))
}

fn egress_request(
    version: PolicyVersion,
    provider: &ProviderPolicySnapshot,
) -> Result<PermissionRequest, Box<dyn Error>> {
    Ok(PermissionRequest {
        actor_id: Some("synthetic-user".to_owned()),
        process_class: ProcessClass::EgressProxy,
        data_class: Some("synthetic-private-code".to_owned()),
        object_range_digest_set: Some(vec![policy_range(10, 18, "slice")?]),
        operation: Some("classify".to_owned()),
        purpose_id: Some("concept-extraction".to_owned()),
        destination_id: Some(provider.destination_id().to_owned()),
        retention_terms_hash: Some(provider.retention_terms_hash()),
        requested_at: Some(120),
        consent_evidence_id: Some("synthetic-consent-event".to_owned()),
        policy_version: Some(version),
    })
}

/// What one run of the broker projects back, plus the grant that was spent.
type Projected = (Vec<AuditRow>, Vec<ConsumptionRow>, String);

/// Runs one real transmission and returns the projections plus the grant spent.
fn transmitted_audit() -> Result<Projected, Box<dyn Error>> {
    let (broker, version, provider) = configured_broker()?;
    let outcome = broker.evaluate(egress_request(version, &provider)?, 200)?;
    let capability = outcome.capability.ok_or("no capability minted")?;
    let grant_id = capability.grant_id().to_owned();
    let runtime = RuntimeToolCall::new(
        "synthetic-user",
        ProcessClass::EgressProxy,
        "classify",
        "concept-extraction",
        provider.destination_id().to_owned(),
        vec![policy_range(10, 18, "slice")?],
        PAYLOAD,
    )?;
    broker.execute(&capability, runtime, 205, |_| {})?;
    Ok((broker.audit_rows()?, broker.consumption_rows()?, grant_id))
}

fn model_run_for(transmission: Transmission) -> Result<ModelRun, Box<dyn Error>> {
    Ok(ModelRun::record(
        ModelRunId::from_bytes([1; 16]),
        Purpose::new("CONCEPT_EXTRACTION")?,
        ProviderId::new("provider-y")?,
        ModelVersion::new("concept-extractor-3")?,
        digest32("prompt-template-v1"),
        InputArtifactRefs::new(vec![InputArtifactRef::new(
            ArtifactId::from_bytes([2; 16]),
            Digest32::from_bytes(
                *ContentDigest::of(b"slice")
                    .as_str()
                    .as_bytes()
                    .first_chunk::<32>()
                    .unwrap_or(&[0; 32]),
            ),
        )])?,
        transmission,
        digest32("redaction-policy-v1"),
        ArtifactId::from_bytes([3; 16]),
        205,
        Cost::new(1_250, "KRW")?,
        RetentionDeclaration::new("ZERO_DAY")?,
    ))
}

fn transmitted_range_matching_audit(
    rows: &[AuditRow],
    consumptions: &[ConsumptionRow],
    grant_id: &str,
) -> Vec<TransmittedRange> {
    let consumed = consumptions
        .iter()
        .find(|consumption| consumption.grant_id == grant_id)
        .map(|consumption| consumption.egress_audit_seq);
    rows.iter()
        .find(|row| Some(row.audit_seq) == consumed && row.decision == Decision::Allow)
        .map(|row| {
            row.artifact_ranges
                .iter()
                .filter_map(|range| {
                    let mut bytes = [0_u8; 32];
                    hex_into(range.content_digest().as_str(), &mut bytes)?;
                    TransmittedRange::new(
                        range.object_id(),
                        range.start(),
                        range.end(),
                        Digest32::from_bytes(bytes),
                    )
                    .ok()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn hex_into(value: &str, out: &mut [u8; 32]) -> Option<()> {
    if value.len() != 64 {
        return None;
    }
    for (index, slot) in out.iter_mut().enumerate() {
        let pair = value.get(index * 2..index * 2 + 2)?;
        *slot = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(())
}

/// A reconciliation that keys on `egress_audit.grant_id` alone.
///
/// This is the shape `P2-M1` would have had if it had read the audit rows
/// directly instead of joining through `egress_consumption`, and it is applied
/// to the same inputs as the product reconciliation inside
/// `an_audit_row_from_the_other_namespace_is_not_the_grant`, so what the join
/// buys is executed rather than described. This function is in the test and
/// reaches no product build.
fn namespace_blind_reconciliation(
    rows: &[AuditRow],
    grant_id: &str,
    ranges: &[TransmittedRange],
) -> bool {
    let mut transmissions = rows.iter().filter(|row| {
        row.grant_id.as_deref() == Some(grant_id)
            && row.external_transmission_digest.is_some()
            && row.decision == Decision::Allow
    });
    let Some(row) = transmissions.next() else {
        return false;
    };
    if transmissions.next().is_some() {
        return false;
    }
    let recorded = ranges
        .iter()
        .map(|range| {
            (
                range.object_id().to_owned(),
                range.start(),
                range.end(),
                range.content_digest().to_lower_hex(),
            )
        })
        .collect::<BTreeSet<_>>();
    let audited = row
        .artifact_ranges
        .iter()
        .map(|range| {
            (
                range.object_id().to_owned(),
                range.start(),
                range.end(),
                range.content_digest().as_str().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    recorded == audited
        && ranges.iter().map(TransmittedRange::length).sum::<u64>() == row.byte_count
}

#[test]
fn transmitted_ranges_reconcile_with_egress_audit() -> Result<(), Box<dyn Error>> {
    let (rows, consumptions, grant_id) = transmitted_audit()?;
    let ranges = transmitted_range_matching_audit(&rows, &consumptions, &grant_id);
    assert!(!ranges.is_empty(), "the audit recorded no artifact range");
    assert_eq!(
        consumptions.len(),
        1,
        "one transfer must leave exactly one consumption record"
    );

    let run = model_run_for(Transmission::egressed(
        EgressGrantId::new(grant_id.clone())?,
        ranges.clone(),
    )?)?;
    let reconciled = reconcile_transmitted_ranges(&run, &rows, &consumptions)?;
    assert_eq!(reconciled.grant_id(), Some(grant_id.as_str()));
    assert_eq!(reconciled.byte_count(), PAYLOAD.len() as u64);

    // A range the audit does not carry is refused.
    let mut widened = ranges.clone();
    widened.push(TransmittedRange::new(
        OBJECT_ID,
        100,
        108,
        digest32("never-transmitted"),
    )?);
    let widened_run = model_run_for(Transmission::egressed(
        EgressGrantId::new(grant_id.clone())?,
        widened,
    )?)?;
    assert_eq!(
        reconcile_transmitted_ranges(&widened_run, &rows, &consumptions),
        Err(ReconciliationError::RangesDiffer(grant_id.clone()))
    );

    // A grant nothing audited is refused.
    let unknown = "0".repeat(64);
    let unaudited_run = model_run_for(Transmission::egressed(
        EgressGrantId::new(unknown.clone())?,
        ranges,
    )?)?;
    assert_eq!(
        reconcile_transmitted_ranges(&unaudited_run, &rows, &consumptions),
        Err(ReconciliationError::GrantNotConsumed(unknown))
    );
    Ok(())
}

#[test]
fn an_audit_row_from_the_other_namespace_is_not_the_grant() -> Result<(), Box<dyn Error>> {
    let (broker, version, provider) = configured_broker()?;
    let outcome = broker.evaluate(egress_request(version, &provider)?, 200)?;
    let capability = outcome.capability.ok_or("no capability minted")?;
    let runtime = RuntimeToolCall::new(
        "synthetic-user",
        ProcessClass::EgressProxy,
        "classify",
        "concept-extraction",
        provider.destination_id().to_owned(),
        vec![policy_range(10, 18, "slice")?],
        PAYLOAD,
    )?;
    broker.execute(&capability, runtime, 205, |_| {})?;

    // The exact cell the two namespaces overlap in: an egress proxy holding the
    // outbound-socket capability. The token identifier is a 64-hex value in the
    // same column, on an allow row with the same class and capability as the
    // egress grant's rows.
    let token = broker.mint_process_capability(
        "synthetic-user",
        ProcessClass::EgressProxy,
        ProcessCapability::OpenOutboundSocket,
        210,
    )?;
    broker.use_process_capability(
        &token,
        "synthetic-user",
        ProcessClass::EgressProxy,
        ProcessCapability::OpenOutboundSocket,
        ProcessActivity::external_transmission(
            vec![policy_range(10, 18, "slice")?],
            provider.destination_id().to_owned(),
            PAYLOAD,
        )?,
        215,
    )?;
    let rows = broker.audit_rows()?;
    let consumptions = broker.consumption_rows()?;
    drop(token);

    // Two transmission rows, one per namespace. The consumption names the
    // egress one; the other is the process-capability token's, and nothing in
    // `egress_audit` alone says which is which.
    let transmissions = rows
        .iter()
        .filter(|row| row.external_transmission_digest.is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        transmissions.len(),
        2,
        "the fixture must produce one transmission row in each namespace"
    );
    let consumed_seq = consumptions
        .first()
        .map(|consumption| consumption.egress_audit_seq)
        .ok_or("no consumption record")?;
    let egress_row = transmissions
        .iter()
        .find(|row| row.audit_seq == consumed_seq)
        .ok_or("no egress-grant transmission row")?;
    let token_row = transmissions
        .iter()
        .find(|row| row.audit_seq != consumed_seq)
        .ok_or("no process-capability transmission row")?;
    let token_id = token_row
        .grant_id
        .clone()
        .ok_or("the process-capability row carries no identifier")?;
    assert_eq!(token_id.len(), 64);
    assert_eq!(token_row.decision, Decision::Allow);
    assert_eq!(token_row.process_class, ProcessClass::EgressProxy);
    assert_eq!(token_row.capability, ProcessCapability::OpenOutboundSocket);
    assert!(
        broker.grant_row(&token_id)?.is_none(),
        "the token identifier must not resolve as an egress grant"
    );
    assert!(
        consumptions
            .iter()
            .all(|consumption| consumption.grant_id != token_id),
        "a process-capability token reached egress_consumption"
    );

    // Every column a reader keying on `egress_audit` alone could have used is
    // the same on both rows: decision, class, capability, byte count,
    // destination, ranges, and the external-transmission digest. Only the
    // identifier differs, and only the join says which namespace it is from.
    assert_eq!(egress_row.process_class, token_row.process_class);
    assert_eq!(egress_row.capability, token_row.capability);
    assert_eq!(egress_row.decision, token_row.decision);
    assert_eq!(egress_row.byte_count, token_row.byte_count);
    assert_eq!(egress_row.destination_id, token_row.destination_id);
    assert_eq!(egress_row.artifact_ranges, token_row.artifact_ranges);
    assert_eq!(
        egress_row.external_transmission_digest,
        token_row.external_transmission_digest
    );
    assert_ne!(egress_row.grant_id, token_row.grant_id);

    // The injection: a model run naming the process token as the grant it spent.
    let forged_ranges = transmitted_range_matching_audit(
        &rows,
        &consumptions,
        egress_row.grant_id.as_deref().unwrap_or_default(),
    );
    let forged = model_run_for(Transmission::egressed(
        EgressGrantId::new(token_id.clone())?,
        forged_ranges.clone(),
    )?)?;
    assert_eq!(
        reconcile_transmitted_ranges(&forged, &rows, &consumptions),
        Err(ReconciliationError::GrantNotConsumed(token_id.clone())),
        "the reconciliation accepted a process-capability token as an egress grant"
    );

    // And the observation that the join is what refuses it: the reconciliation
    // that keys on `egress_audit.grant_id` alone accepts it.
    assert!(
        namespace_blind_reconciliation(&rows, &token_id, &forged_ranges),
        "the grant_id-only reconciliation no longer accepts the forged grant, \
         so this test no longer measures what the consumption join buys"
    );
    Ok(())
}

#[test]
fn a_local_only_run_whose_bytes_were_transmitted_is_refused() -> Result<(), Box<dyn Error>> {
    let (rows, consumptions, _) = transmitted_audit()?;
    let transmitted_digest = rows
        .iter()
        .find(|row| row.decision == Decision::Allow && row.byte_count > 0)
        .and_then(|row| row.artifact_ranges.first())
        .map(|range| range.content_digest().as_str().to_owned())
        .ok_or("no transmitted range")?;
    let mut bytes = [0_u8; 32];
    hex_into(&transmitted_digest, &mut bytes).ok_or("digest is not 64 hex characters")?;

    let run = ModelRun::record(
        ModelRunId::from_bytes([9; 16]),
        Purpose::new("CONCEPT_EXTRACTION")?,
        ProviderId::new("local-model-x")?,
        ModelVersion::new("local-1")?,
        digest32("prompt-template-v1"),
        InputArtifactRefs::new(vec![InputArtifactRef::new(
            ArtifactId::from_bytes([2; 16]),
            Digest32::from_bytes(bytes),
        )])?,
        Transmission::LocalOnly,
        digest32("redaction-policy-v1"),
        ArtifactId::from_bytes([3; 16]),
        205,
        Cost::new(0, "KRW")?,
        RetentionDeclaration::new("ZERO_DAY")?,
    );
    assert_eq!(
        reconcile_transmitted_ranges(&run, &rows, &consumptions),
        Err(ReconciliationError::LocalOnlyRunTransmitted(
            transmitted_digest
        ))
    );

    // A local-only run that read something else reconciles.
    let untouched = model_run_for(Transmission::LocalOnly)?;
    assert_eq!(
        reconcile_transmitted_ranges(&untouched, &rows, &consumptions)?.byte_count(),
        0
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// uncalibrated_score_cannot_be_displayed
// ---------------------------------------------------------------------------

fn dataset(
    id: &str,
    provider: &str,
    version: &str,
    refreshed_at: u64,
) -> Result<CalibrationDataset, Box<dyn Error>> {
    Ok(CalibrationDataset::new(
        CalibrationDatasetId::new(id)?,
        ProviderId::new(provider)?,
        ModelVersion::new(version)?,
        Purpose::new("CONCEPT_EXTRACTION")?,
        digest32(id),
        512,
        refreshed_at,
        10_000,
        vec![
            CalibrationBin::new(250, 100)?,
            CalibrationBin::new(500, 400)?,
            CalibrationBin::new(1_000, 900)?,
        ],
    )?)
}

#[test]
fn uncalibrated_score_cannot_be_displayed() -> Result<(), Box<dyn Error>> {
    let purpose = Purpose::new("CONCEPT_EXTRACTION")?;
    let score = RawScore::new(
        ProviderId::new("provider-y")?,
        ModelVersion::new("concept-extractor-3")?,
        400,
    );

    // Nothing is registered, so there is no calibrated value to display.
    let empty = CalibrationRegistry::new();
    assert_eq!(
        empty.interpret(&score, &purpose, 1_000),
        Err(ModelRunError::NoCalibrationDataset("provider-y".to_owned()))
    );

    // A dataset for another model does not interpret this one's numbers.
    let mut registry = CalibrationRegistry::new();
    registry.register(dataset("cal-other", "provider-z", "other-1", 1_000)?)?;
    assert!(registry.interpret(&score, &purpose, 1_000).is_err());

    // A stale dataset does not either. Displaying through an expired curve is
    // the failure this refuses: the number would look interpreted and be a
    // measurement of a model version that has since moved.
    registry.register(dataset(
        "cal-y",
        "provider-y",
        "concept-extractor-3",
        1_000,
    )?)?;
    assert_eq!(
        registry.interpret(&score, &purpose, 11_000),
        Err(ModelRunError::StaleCalibrationDataset("cal-y".to_owned()))
    );

    // Fresh, and only then is there something to display.
    let calibrated = registry.interpret(&score, &purpose, 5_000)?;
    assert_eq!(calibrated.confidence().value(), 400);
    let displayed = DisplayedConfidence::of(&calibrated);
    assert_eq!(displayed.dataset().as_str(), "cal-y");
    assert_eq!(displayed.to_string(), "40.0% (calibrated by cal-y)");

    // The raw score has no formatting path that yields the number. `Debug` is
    // the last one that could, and it prints the provider and not the units.
    let rendered = format!("{score:?}");
    assert!(rendered.contains("provider-y"));
    assert!(rendered.contains("<uncalibrated>"));
    assert!(!rendered.contains("400"));
    Ok(())
}

#[test]
fn calibration_datasets_carry_refresh_metadata() -> Result<(), Box<dyn Error>> {
    let set = dataset("cal-y", "provider-y", "concept-extractor-3", 1_000)?;
    assert_eq!(set.sample_count(), 512);
    assert_eq!(set.refreshed_at(), 1_000);
    assert_eq!(set.refresh_interval_millis(), 10_000);
    assert!(!set.is_stale(1_000));
    assert!(!set.is_stale(10_999));
    assert!(set.is_stale(11_000), "the refresh interval must expire");
    assert!(set.is_stale(999), "a clock before the refresh is not fresh");

    let mut registry = CalibrationRegistry::new();
    registry.register(dataset(
        "cal-y",
        "provider-y",
        "concept-extractor-3",
        1_000,
    )?)?;
    assert_eq!(
        registry.register(dataset(
            "cal-y2",
            "provider-y",
            "concept-extractor-3",
            1_000
        )?),
        Err(ModelRunError::DuplicateCalibrationDataset)
    );
    assert_eq!(registry.datasets().len(), 1);
    Ok(())
}

// ---------------------------------------------------------------------------
// cross_provider_raw_scores_are_not_ordered
// ---------------------------------------------------------------------------

#[test]
fn cross_provider_raw_scores_are_not_ordered() -> Result<(), Box<dyn Error>> {
    // The type half is `tests/compile_fail/`: `<`, `max`, `sort`, `cmp` and
    // `BTreeSet` on a `RawScore` are each a separate case there, and the source
    // half in `model_run_scans.rs` compares the whole set of `impl` blocks that
    // name the type. What runs here is the consequence: two providers' numbers
    // become comparable only after each has been read through its own dataset,
    // and the ranking that comes out is not the ranking of the raw units.
    let purpose = Purpose::new("CONCEPT_EXTRACTION")?;
    let mut registry = CalibrationRegistry::new();
    registry.register(CalibrationDataset::new(
        CalibrationDatasetId::new("cal-generous")?,
        ProviderId::new("provider-generous")?,
        ModelVersion::new("g-1")?,
        purpose.clone(),
        digest32("cal-generous"),
        512,
        0,
        10_000,
        vec![CalibrationBin::new(1_000, 200)?],
    )?)?;
    registry.register(CalibrationDataset::new(
        CalibrationDatasetId::new("cal-strict")?,
        ProviderId::new("provider-strict")?,
        ModelVersion::new("s-1")?,
        purpose.clone(),
        digest32("cal-strict"),
        512,
        0,
        10_000,
        vec![CalibrationBin::new(1_000, 800)?],
    )?)?;

    let generous = RawScore::new(
        ProviderId::new("provider-generous")?,
        ModelVersion::new("g-1")?,
        900,
    );
    let strict = RawScore::new(
        ProviderId::new("provider-strict")?,
        ModelVersion::new("s-1")?,
        300,
    );

    let generous_calibrated = registry.interpret(&generous, &purpose, 1_000)?;
    let strict_calibrated = registry.interpret(&strict, &purpose, 1_000)?;

    // Raw, the generous provider's 900 looks like the stronger claim. Read
    // through each model's own dataset, it is the weaker one. That inversion is
    // exactly why the raw numbers may not be ranked against each other.
    assert!(strict_calibrated > generous_calibrated);
    assert_eq!(strict_calibrated.confidence().value(), 800);
    assert_eq!(generous_calibrated.confidence().value(), 200);

    // Calibrated values order by the permille and by nothing else, so the
    // ordering does not smuggle a provider name in as a tie-breaker.
    registry.register(CalibrationDataset::new(
        CalibrationDatasetId::new("cal-third")?,
        ProviderId::new("provider-aaa")?,
        ModelVersion::new("a-1")?,
        purpose.clone(),
        digest32("cal-third"),
        512,
        0,
        10_000,
        vec![CalibrationBin::new(1_000, 200)?],
    )?)?;
    let other_provider = registry.interpret(
        &RawScore::new(
            ProviderId::new("provider-aaa")?,
            ModelVersion::new("a-1")?,
            10,
        ),
        &purpose,
        1_000,
    )?;
    assert_eq!(
        other_provider.cmp(&generous_calibrated),
        core::cmp::Ordering::Equal,
        "two calibrated values with the same permille must compare equal whatever          provider or dataset produced them"
    );
    assert_ne!(other_provider, generous_calibrated);
    Ok(())
}

#[test]
fn a_calibrated_confidence_names_the_dataset_that_produced_it() -> Result<(), Box<dyn Error>> {
    let purpose = Purpose::new("CONCEPT_EXTRACTION")?;
    let mut registry = CalibrationRegistry::new();
    let set = dataset("cal-y", "provider-y", "concept-extractor-3", 0)?;
    let expected_digest = *set.digest();
    registry.register(set)?;
    let calibrated: CalibratedConfidence = registry.interpret(
        &RawScore::new(
            ProviderId::new("provider-y")?,
            ModelVersion::new("concept-extractor-3")?,
            900,
        ),
        &purpose,
        1_000,
    )?;
    assert_eq!(calibrated.dataset().as_str(), "cal-y");
    assert_eq!(calibrated.dataset_digest(), &expected_digest);
    assert_eq!(calibrated.provider().as_str(), "provider-y");
    assert_eq!(calibrated.model_version().as_str(), "concept-extractor-3");
    Ok(())
}
