use std::{
    error::Error,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use academic_policy::{
    BrokerError, ContentDigest, Decision, EgressRule, ObjectRange, PermissionBroker,
    PermissionRequest, PolicySnapshot, PolicyVersion, ProviderIdentity, ProviderPolicyDraft,
    ProviderPolicySnapshot, ProviderSurface, ReasonCode, RuntimeToolCall,
};
use proptest::prelude::*;

const PAYLOAD: &[u8] = b"allowed!";

fn digest(label: &str) -> ContentDigest {
    ContentDigest::of(label.as_bytes())
}

fn range(start: u64, end: u64, label: &str) -> Result<ObjectRange, BrokerError> {
    ObjectRange::new("synthetic-object", start, end, digest(label))
}

fn provider_identity() -> Result<ProviderIdentity, BrokerError> {
    ProviderIdentity::new("provider-y", ProviderSurface::EnterpriseApi)
}

fn provider_destination() -> String {
    provider_identity()
        .map(|identity| identity.destination_id())
        .unwrap_or_else(|_| "invalid-provider-identity".to_owned())
}

fn provider_draft() -> Result<ProviderPolicyDraft, BrokerError> {
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

fn rule(minimal_ranges: Vec<ObjectRange>, payload: &[u8]) -> EgressRule {
    EgressRule {
        actor_process_class: "repo-analyzer".to_owned(),
        data_class: "synthetic-private-code".to_owned(),
        operation: "classify".to_owned(),
        purpose_id: "architecture-classification".to_owned(),
        destination_id: provider_destination(),
        retention_terms_hash: digest("zero-day-retention"),
        consent_evidence_id: "synthetic-consent-event".to_owned(),
        valid_from: 100,
        valid_until: 1_000,
        minimal_ranges,
        payload_digest: ContentDigest::of(payload),
        provider_policy_snapshot_digest: digest("provider-policy-v1"),
        training_use_allowed: false,
        redaction_policy_hash: digest("redaction-policy-v1"),
    }
}

fn request(version: PolicyVersion, ranges: Vec<ObjectRange>) -> PermissionRequest {
    PermissionRequest {
        actor_process_class: Some("repo-analyzer".to_owned()),
        data_class: Some("synthetic-private-code".to_owned()),
        object_range_digest_set: Some(ranges),
        operation: Some("classify".to_owned()),
        purpose_id: Some("architecture-classification".to_owned()),
        destination_id: Some(provider_destination()),
        retention_terms_hash: Some(digest("zero-day-retention")),
        requested_at: Some(120),
        consent_evidence_id: Some("synthetic-consent-event".to_owned()),
        policy_version: Some(version),
    }
}

fn provider_request(
    version: PolicyVersion,
    ranges: Vec<ObjectRange>,
    provider: &ProviderPolicySnapshot,
) -> PermissionRequest {
    let mut request = request(version, ranges);
    request.destination_id = Some(provider.destination_id().to_owned());
    request.retention_terms_hash = Some(provider.retention_terms_hash());
    request
}

fn configured_broker(
    ttl: u64,
) -> Result<(PermissionBroker, PolicyVersion, ProviderPolicySnapshot), BrokerError> {
    let broker = PermissionBroker::new_profile_with_ttl(ttl)?;
    let provider = broker.register_provider_policy(provider_draft()?, 0)?;
    let mut configured_rule = rule(vec![range(10, 18, "slice")?], PAYLOAD);
    configured_rule.destination_id = provider.destination_id().to_owned();
    configured_rule.provider_policy_snapshot_digest = provider.snapshot_digest().clone();
    configured_rule.retention_terms_hash = provider.retention_terms_hash();
    let snapshot = PolicySnapshot::from_rules(vec![configured_rule])?;
    let version = broker.install_policy(snapshot)?;
    Ok((broker, version, provider))
}

proptest! {
    #[test]
    fn missing_tuple_field_denies_and_audits(salt in any::<u64>()) {
        // §32.3 names eight semantic fields; §3.5 splits data/object-range and adds
        // the policy pin, producing ten concrete required entries. Exercise every
        // concrete entry so neither interpretation leaves a permissive absence.
        for missing in 0..10 {
            let broker = PermissionBroker::new_profile().map_err(|error| TestCaseError::fail(error.to_string()))?;
            let snapshot = PolicySnapshot::from_rules(vec![rule(
                vec![range(10, 18, "slice").map_err(|error| TestCaseError::fail(error.to_string()))?],
                PAYLOAD,
            )]).map_err(|error| TestCaseError::fail(error.to_string()))?;
            let version = broker.install_policy(snapshot).map_err(|error| TestCaseError::fail(error.to_string()))?;
            let mut candidate = request(
                version,
                vec![range(10, 18, "slice").map_err(|error| TestCaseError::fail(error.to_string()))?],
            );
            candidate.requested_at = Some(120 + (salt % 10));
            match missing {
                0 => candidate.actor_process_class = None,
                1 => candidate.data_class = None,
                2 => candidate.object_range_digest_set = None,
                3 => candidate.operation = None,
                4 => candidate.purpose_id = None,
                5 => candidate.destination_id = None,
                6 => candidate.retention_terms_hash = None,
                7 => candidate.requested_at = None,
                8 => candidate.consent_evidence_id = None,
                9 => candidate.policy_version = None,
                _ => unreachable!(),
            }
            let outcome = broker.evaluate(candidate, 200)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(outcome.receipt.fingerprint().decision, Decision::Deny);
            prop_assert_eq!(outcome.receipt.fingerprint().reason_code, Some(ReasonCode::NoGrant));
            prop_assert!(outcome.capability.is_none());
            let rows = broker.audit_rows().map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(rows.len(), 1);
            prop_assert_eq!(rows[0].decision, Decision::Deny);
        }
    }
}

#[test]
fn broad_request_is_minimized_or_rejected() -> Result<(), Box<dyn Error>> {
    let (broker, version, provider) = configured_broker(100)?;
    let broad = provider_request(
        version.clone(),
        vec![range(0, 100, "whole-object")?],
        &provider,
    );
    let allowed = broker.evaluate(broad, 200)?;
    assert_eq!(allowed.receipt.fingerprint().decision, Decision::Allow);
    let grant = broker
        .grant_row(allowed.receipt.grant_id().ok_or("missing grant id")?)?
        .ok_or("missing grant")?;
    assert_eq!(
        grant.byte_ranges_canonical,
        format!("synthetic-object:10-18@{}", digest("slice").as_str())
    );

    let too_narrow = provider_request(version, vec![range(0, 9, "wrong-slice")?], &provider);
    let denied = broker.evaluate(too_narrow, 201)?;
    assert_eq!(denied.receipt.fingerprint().decision, Decision::Deny);
    assert_eq!(
        denied.receipt.fingerprint().reason_code,
        Some(ReasonCode::ScopeMismatch)
    );
    Ok(())
}

#[test]
fn grant_is_single_use_and_expiring() -> Result<(), Box<dyn Error>> {
    let (broker, version, provider) = configured_broker(10)?;
    let outcome = broker.evaluate(
        provider_request(version.clone(), vec![range(10, 18, "slice")?], &provider),
        200,
    )?;
    let capability = Arc::new(outcome.capability.ok_or("missing capability")?);
    let broker = Arc::new(broker);
    let calls = Arc::new(AtomicUsize::new(0));
    let start = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let broker = Arc::clone(&broker);
        let capability = Arc::clone(&capability);
        let calls = Arc::clone(&calls);
        let start = Arc::clone(&start);
        workers.push(thread::spawn(move || {
            let runtime = RuntimeToolCall::new(
                "repo-analyzer",
                "classify",
                "architecture-classification",
                provider_destination(),
                vec![range(10, 18, "slice").map_err(|error| error.to_string())?],
                PAYLOAD,
            )
            .map_err(|error| error.to_string())?;
            start.wait();
            broker
                .execute(&capability, runtime, 205, |_| {
                    calls.fetch_add(1, Ordering::SeqCst);
                })
                .map_err(|error| error.to_string())
        }));
    }
    start.wait();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().map_err(|_| "worker panicked"))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(results.iter().any(|result| {
        result
            .as_ref()
            .is_err_and(|error| error.contains("GrantConsumed"))
    }));

    let expired = broker.evaluate(
        provider_request(version, vec![range(10, 18, "slice")?], &provider),
        300,
    )?;
    let expired_capability = expired.capability.ok_or("missing capability")?;
    let runtime = RuntimeToolCall::new(
        "repo-analyzer",
        "classify",
        "architecture-classification",
        provider_destination(),
        vec![range(10, 18, "slice")?],
        PAYLOAD,
    )?;
    assert!(matches!(
        broker.execute(&expired_capability, runtime, 310, |_| ()),
        Err(BrokerError::Denied(ReasonCode::GrantExpired))
    ));
    Ok(())
}

#[test]
fn token_range_overflow_is_blocked_at_runtime() -> Result<(), Box<dyn Error>> {
    let (broker, version, provider) = configured_broker(100)?;
    let outcome = broker.evaluate(
        provider_request(version, vec![range(0, 100, "whole-object")?], &provider),
        200,
    )?;
    let capability = outcome.capability.ok_or("missing capability")?;
    let tool_calls = AtomicUsize::new(0);
    let overflowing = RuntimeToolCall::new(
        "repo-analyzer",
        "classify",
        "architecture-classification",
        provider_destination(),
        vec![range(9, 19, "overflow")?],
        b"notallowed",
    )?;
    assert!(matches!(
        broker.execute(&capability, overflowing, 205, |_| {
            tool_calls.fetch_add(1, Ordering::SeqCst);
        }),
        Err(BrokerError::Denied(ReasonCode::ScopeMismatch))
    ));
    assert_eq!(tool_calls.load(Ordering::SeqCst), 0);
    let rows = broker.audit_rows()?;
    assert_eq!(
        rows.last().ok_or("missing audit")?.reason_code,
        Some(ReasonCode::ScopeMismatch)
    );
    Ok(())
}

#[test]
fn policy_version_replay_is_deterministic() -> Result<(), Box<dyn Error>> {
    let (broker, version, provider) = configured_broker(100)?;
    let outcome = broker.evaluate(
        provider_request(version, vec![range(0, 100, "whole-object")?], &provider),
        200,
    )?;
    let expected = outcome.receipt.fingerprint().clone();

    let changed = PolicySnapshot::from_rules(vec![rule(
        vec![range(11, 17, "different-slice")?],
        b"change",
    )])?;
    let changed_version = broker.install_policy(changed)?;
    assert_ne!(
        expected.policy_version.as_deref(),
        Some(changed_version.as_str())
    );
    let replayed = broker.replay(&outcome.receipt, 250)?;
    assert_eq!(replayed, expected);
    Ok(())
}

#[test]
fn audit_row_contains_no_payload_bytes() -> Result<(), Box<dyn Error>> {
    let (broker, version, provider) = configured_broker(100)?;
    let outcome = broker.evaluate(
        provider_request(version, vec![range(10, 18, "slice")?], &provider),
        200,
    )?;
    let capability = outcome.capability.ok_or("missing capability")?;
    let runtime = RuntimeToolCall::new(
        "repo-analyzer",
        "classify",
        "architecture-classification",
        provider_destination(),
        vec![range(10, 18, "slice")?],
        PAYLOAD,
    )?;
    broker.execute(&capability, runtime, 205, |authorized| {
        assert_eq!(authorized.payload(), PAYLOAD);
    })?;
    let rendered = format!("{:?}", broker.audit_rows()?);
    assert!(!rendered.contains(std::str::from_utf8(PAYLOAD)?));
    assert!(!academic_policy::POLICY_SCHEMA_SQL.contains("payload_bytes"));
    assert!(!academic_policy::POLICY_SCHEMA_SQL.contains("prompt_text"));
    assert!(!academic_policy::POLICY_SCHEMA_SQL.contains("provider_response_text"));

    let schema = rusqlite::Connection::open_in_memory()?;
    schema.execute_batch(academic_policy::POLICY_SCHEMA_SQL)?;
    let mut columns =
        schema.prepare("SELECT name FROM pragma_table_info('egress_audit') ORDER BY cid")?;
    let column_names = columns
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(
        column_names,
        [
            "audit_seq",
            "grant_id",
            "decision",
            "reason_code",
            "actor_process_class",
            "payload_digest",
            "byte_count",
            "destination_id",
            "started_at",
            "finished_at",
            "provider_response_digest",
            "deletion_receipt_id",
        ]
    );
    assert!(
        schema
            .execute(
                "INSERT INTO egress_audit (decision, reason_code, actor_process_class, payload_digest, payload_bytes, byte_count, destination_id, started_at, finished_at) VALUES ('DENY', 'NO_GRANT', 'repo-analyzer', NULL, ?1, 0, 'provider-y-api', 1, 1)",
                [PAYLOAD],
            )
            .is_err(),
        "the fixed audit table unexpectedly accepted raw payload bytes",
    );
    Ok(())
}

#[test]
fn default_new_profile_is_local_first_default_deny() -> Result<(), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile()?;
    let snapshot = broker.default_policy_snapshot()?;
    assert!(snapshot.local_processing_preferred());
    assert_eq!(snapshot.configured_egress_rule_count(), 0);

    let candidate = request(
        broker.default_policy_version().clone(),
        vec![range(10, 18, "slice")?],
    );
    let denied = broker.evaluate(candidate, 200)?;
    assert_eq!(denied.receipt.fingerprint().decision, Decision::Deny);
    assert_eq!(
        denied.receipt.fingerprint().reason_code,
        Some(ReasonCode::NoGrant)
    );
    assert_eq!(broker.audit_rows()?.len(), 1);
    Ok(())
}

#[test]
fn policy_hash_ignores_rule_insertion_order() -> Result<(), Box<dyn Error>> {
    let first = rule(vec![range(10, 18, "slice")?], PAYLOAD);
    let mut second = first.clone();
    second.purpose_id = "second-purpose".to_owned();
    assert_eq!(
        PolicySnapshot::from_rules(vec![first.clone(), second.clone()])?.version(),
        PolicySnapshot::from_rules(vec![second, first])?.version()
    );
    Ok(())
}

#[test]
fn reason_code_enum_is_exactly_the_fixed_set() {
    let codes = [
        ReasonCode::NoGrant,
        ReasonCode::GrantExpired,
        ReasonCode::GrantConsumed,
        ReasonCode::ScopeMismatch,
        ReasonCode::PolicyStale,
        ReasonCode::ProviderPolicyIncompatible,
        ReasonCode::ScannerError,
        ReasonCode::SecretPattern,
        ReasonCode::SecretEntropy,
        ReasonCode::PiiDetected,
        ReasonCode::UnknownBinary,
        ReasonCode::Oversize,
        ReasonCode::RedactionDestroysMeaning,
        ReasonCode::CanaryInResponse,
        ReasonCode::NoDeletionReceipt,
    ];
    assert_eq!(
        codes.map(ReasonCode::as_str),
        [
            "NO_GRANT",
            "GRANT_EXPIRED",
            "GRANT_CONSUMED",
            "SCOPE_MISMATCH",
            "POLICY_STALE",
            "PROVIDER_POLICY_INCOMPATIBLE",
            "SCANNER_ERROR",
            "SECRET_PATTERN",
            "SECRET_ENTROPY",
            "PII_DETECTED",
            "UNKNOWN_BINARY",
            "OVERSIZE",
            "REDACTION_DESTROYS_MEANING",
            "CANARY_IN_RESPONSE",
            "NO_DELETION_RECEIPT",
        ]
    );
}
