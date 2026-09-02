use std::{
    error::Error,
    sync::atomic::{AtomicUsize, Ordering},
};

use academic_policy::{
    BrokerError, ContentDigest, Decision, DeletionReceiptDraft, EgressRule, ObjectRange,
    PermissionBroker, PermissionRequest, PolicySnapshot, ProviderIdentity, ProviderPolicyDraft,
    ProviderPolicySnapshot, ProviderSurface, ProviderUserPolicy, ReasonCode, RuntimeToolCall,
};

const PAYLOAD: &[u8] = b"minimum";

fn digest(label: &str) -> ContentDigest {
    ContentDigest::of(label.as_bytes())
}

fn identity(surface: ProviderSurface) -> Result<ProviderIdentity, BrokerError> {
    ProviderIdentity::new("synthetic-vendor", surface)
}

fn provider_draft(
    identity: ProviderIdentity,
    last_verified_at: u64,
    ttl_millis: u64,
) -> ProviderPolicyDraft {
    ProviderPolicyDraft {
        identity: Some(identity),
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
        policy_source_digest: Some(digest("synthetic-provider-policy-source")),
        last_verified_at: Some(last_verified_at),
        ttl_millis: Some(ttl_millis),
    }
}

fn range() -> Result<ObjectRange, BrokerError> {
    ObjectRange::new("synthetic-object", 10, 17, digest("synthetic-slice"))
}

fn rule(provider: &ProviderPolicySnapshot) -> Result<EgressRule, BrokerError> {
    Ok(EgressRule {
        actor_process_class: "repo-analyzer".to_owned(),
        data_class: "synthetic-private-code".to_owned(),
        operation: "classify".to_owned(),
        purpose_id: "architecture-classification".to_owned(),
        destination_id: provider.destination_id().to_owned(),
        retention_terms_hash: provider.retention_terms_hash(),
        consent_evidence_id: "synthetic-consent".to_owned(),
        valid_from: 0,
        valid_until: 10_000,
        minimal_ranges: vec![range()?],
        payload_digest: ContentDigest::of(PAYLOAD),
        provider_policy_snapshot_digest: provider.snapshot_digest().clone(),
        training_use_allowed: false,
        redaction_policy_hash: digest("synthetic-redaction"),
    })
}

fn request(
    policy: academic_policy::PolicyVersion,
    provider: &ProviderPolicySnapshot,
    requested_at: u64,
) -> Result<PermissionRequest, BrokerError> {
    Ok(PermissionRequest {
        actor_process_class: Some("repo-analyzer".to_owned()),
        data_class: Some("synthetic-private-code".to_owned()),
        object_range_digest_set: Some(vec![range()?]),
        operation: Some("classify".to_owned()),
        purpose_id: Some("architecture-classification".to_owned()),
        destination_id: Some(provider.destination_id().to_owned()),
        retention_terms_hash: Some(provider.retention_terms_hash()),
        requested_at: Some(requested_at),
        consent_evidence_id: Some("synthetic-consent".to_owned()),
        policy_version: Some(policy),
    })
}

fn install_rule(
    broker: &PermissionBroker,
    provider: &ProviderPolicySnapshot,
) -> Result<academic_policy::PolicyVersion, BrokerError> {
    broker.install_policy(PolicySnapshot::from_rules(vec![rule(provider)?])?)
}

fn runtime(provider: &ProviderPolicySnapshot) -> Result<RuntimeToolCall<'static>, BrokerError> {
    RuntimeToolCall::new(
        "repo-analyzer",
        "classify",
        "architecture-classification",
        provider.destination_id(),
        vec![range()?],
        PAYLOAD,
    )
}

#[test]
fn provider_snapshot_schema_and_digest_are_fixed() -> Result<(), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile()?;
    let provider = broker.register_provider_policy(
        provider_draft(identity(ProviderSurface::EnterpriseApi)?, 100, 50),
        100,
    )?;
    assert_eq!(
        provider.snapshot_digest().as_str(),
        "be78060ce064b30131e058ee747cbc5ed8068cbddc6127757ad2e4e8790f1025"
    );
    assert_eq!(provider.identity().vendor_id(), "synthetic-vendor");
    assert_eq!(
        provider.identity().surface(),
        ProviderSurface::EnterpriseApi
    );
    assert!(!provider.training_use_enabled());
    assert!(!provider.training_opt_out_applied());
    assert_eq!(provider.server_retention_millis(), 0);
    assert!(!provider.abuse_logging_enabled());
    assert_eq!(provider.residency_regions(), ["us-east"]);
    assert_eq!(provider.subprocessors(), ["synthetic-subprocessor"]);
    assert!(provider.transit_encryption_declared());
    assert!(provider.at_rest_encryption_declared());
    assert!(provider.deletion_api_available());
    assert!(provider.deletion_receipt_capable());
    assert_eq!(provider.maximum_input_bytes(), 64);
    assert_eq!(provider.logging_configuration(), "content-logging-disabled");
    assert_eq!(
        provider.policy_source_digest(),
        &digest("synthetic-provider-policy-source")
    );
    assert_eq!(provider.last_verified_at(), 100);
    assert_eq!(provider.ttl_millis(), 50);
    assert_eq!(provider.verified_until(), 150);

    let schema = rusqlite::Connection::open_in_memory()?;
    schema.execute_batch(academic_policy::POLICY_SCHEMA_SQL)?;
    let mut columns = schema
        .prepare("SELECT name FROM pragma_table_info('provider_policy_snapshot') ORDER BY cid")?;
    let column_names = columns
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(
        column_names,
        [
            "snapshot_seq",
            "snapshot_digest",
            "destination_id",
            "vendor_id",
            "surface",
            "training_use_enabled",
            "training_opt_out_applied",
            "server_retention_millis",
            "abuse_logging_enabled",
            "transit_encryption_declared",
            "at_rest_encryption_declared",
            "deletion_api_available",
            "deletion_receipt_capable",
            "maximum_input_bytes",
            "logging_configuration",
            "policy_source_digest",
            "last_verified_at",
            "ttl_millis",
            "registered_at",
        ]
    );
    Ok(())
}

#[test]
fn provider_missing_privacy_field_cannot_register() -> Result<(), Box<dyn Error>> {
    let complete = provider_draft(identity(ProviderSurface::EnterpriseApi)?, 100, 50);
    let mut omissions = Vec::new();

    let mut draft = complete.clone();
    draft.identity = None;
    omissions.push(("identity", draft));
    let mut draft = complete.clone();
    draft.training_use_enabled = None;
    omissions.push(("training_use_enabled", draft));
    let mut draft = complete.clone();
    draft.training_opt_out_applied = None;
    omissions.push(("training_opt_out_applied", draft));
    let mut draft = complete.clone();
    draft.server_retention_millis = None;
    omissions.push(("server_retention_millis", draft));
    let mut draft = complete.clone();
    draft.abuse_logging_enabled = None;
    omissions.push(("abuse_logging_enabled", draft));
    let mut draft = complete.clone();
    draft.residency_regions = None;
    omissions.push(("residency_regions", draft));
    let mut draft = complete.clone();
    draft.subprocessors = None;
    omissions.push(("subprocessors", draft));
    let mut draft = complete.clone();
    draft.transit_encryption_declared = None;
    omissions.push(("transit_encryption_declared", draft));
    let mut draft = complete.clone();
    draft.at_rest_encryption_declared = None;
    omissions.push(("at_rest_encryption_declared", draft));
    let mut draft = complete.clone();
    draft.deletion_api_available = None;
    omissions.push(("deletion_api_available", draft));
    let mut draft = complete.clone();
    draft.deletion_receipt_capable = None;
    omissions.push(("deletion_receipt_capable", draft));
    let mut draft = complete.clone();
    draft.maximum_input_bytes = None;
    omissions.push(("maximum_input_bytes", draft));
    let mut draft = complete.clone();
    draft.logging_configuration = None;
    omissions.push(("logging_configuration", draft));
    let mut draft = complete.clone();
    draft.policy_source_digest = None;
    omissions.push(("policy_source_digest", draft));
    let mut draft = complete.clone();
    draft.last_verified_at = None;
    omissions.push(("last_verified_at", draft));
    let mut draft = complete;
    draft.ttl_millis = None;
    omissions.push(("ttl_millis", draft));

    for (expected, draft) in omissions {
        let broker = PermissionBroker::new_profile()?;
        assert!(matches!(
            broker.register_provider_policy(draft, 100),
            Err(BrokerError::MissingProviderPrivacyField(field)) if field == expected
        ));
    }
    Ok(())
}

#[test]
fn stale_policy_does_not_auto_renew_grant() -> Result<(), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile_with_ttl(1_000)?;
    let draft = provider_draft(identity(ProviderSurface::EnterpriseApi)?, 100, 10);
    let provider = broker.register_provider_policy(draft.clone(), 100)?;
    let policy = install_rule(&broker, &provider)?;
    let outcome = broker.evaluate(request(policy.clone(), &provider, 105)?, 105)?;
    let grant_id = outcome
        .receipt
        .grant_id()
        .ok_or("missing grant")?
        .to_owned();
    let capability = outcome.capability.ok_or("missing capability")?;
    assert_eq!(
        broker
            .grant_row(&grant_id)?
            .ok_or("missing grant row")?
            .expires_at,
        110,
        "provider TTL must cap the broker grant TTL",
    );

    let repeated = broker.register_provider_policy(draft, 108)?;
    assert_eq!(repeated.snapshot_digest(), provider.snapshot_digest());
    assert_eq!(
        broker.provider_policy_versions(provider.identity())?.len(),
        1
    );
    let stale = broker.evaluate(request(policy, &provider, 110)?, 110)?;
    assert_eq!(stale.receipt.fingerprint().decision, Decision::Deny);
    assert_eq!(
        stale.receipt.fingerprint().reason_code,
        Some(ReasonCode::PolicyStale),
        "re-registering identical facts must not mint a later grant",
    );
    assert!(stale.capability.is_none());
    assert!(matches!(
        broker.execute(&capability, runtime(&provider)?, 110, |_| ()),
        Err(BrokerError::Denied(ReasonCode::GrantExpired))
    ));
    assert_eq!(
        broker
            .grant_row(&grant_id)?
            .ok_or("missing grant row")?
            .expires_at,
        110,
        "re-registering identical facts must not extend an issued grant",
    );
    Ok(())
}

#[test]
fn changed_policy_hash_invalidates_future_grants() -> Result<(), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile_with_ttl(1_000)?;
    let provider_v1 = broker.register_provider_policy(
        provider_draft(identity(ProviderSurface::EnterpriseApi)?, 0, 1_000),
        0,
    )?;
    let policy = install_rule(&broker, &provider_v1)?;
    let issued = broker.evaluate(request(policy.clone(), &provider_v1, 100)?, 100)?;
    let expected_replay = issued.receipt.fingerprint().clone();
    let capability = issued.capability.ok_or("missing capability")?;

    let mut changed = provider_draft(identity(ProviderSurface::EnterpriseApi)?, 101, 1_000);
    changed.subprocessors = Some(vec!["changed-synthetic-subprocessor".to_owned()]);
    let provider_v2 = broker.register_provider_policy(changed, 101)?;
    assert_ne!(provider_v1.snapshot_digest(), provider_v2.snapshot_digest());
    let later = broker.evaluate(request(policy, &provider_v1, 102)?, 102)?;
    assert_eq!(later.receipt.fingerprint().decision, Decision::Deny);
    assert_eq!(
        later.receipt.fingerprint().reason_code,
        Some(ReasonCode::ProviderPolicyIncompatible)
    );
    let calls = AtomicUsize::new(0);
    assert!(matches!(
        broker.execute(&capability, runtime(&provider_v1)?, 102, |_| {
            calls.fetch_add(1, Ordering::SeqCst);
        }),
        Err(BrokerError::Denied(ReasonCode::ProviderPolicyIncompatible))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(broker.replay(&issued.receipt, 103)?, expected_replay);
    Ok(())
}

#[test]
fn same_vendor_two_surfaces_are_distinct_records() -> Result<(), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile()?;
    let enterprise = broker.register_provider_policy(
        provider_draft(identity(ProviderSurface::EnterpriseApi)?, 0, 1_000),
        0,
    )?;
    let consumer = broker.register_provider_policy(
        provider_draft(identity(ProviderSurface::ConsumerUi)?, 0, 1_000),
        0,
    )?;
    assert_eq!(
        enterprise.identity().vendor_id(),
        consumer.identity().vendor_id()
    );
    assert_ne!(enterprise.destination_id(), consumer.destination_id());
    assert_ne!(enterprise.snapshot_digest(), consumer.snapshot_digest());
    assert_eq!(
        broker
            .provider_policy_versions(enterprise.identity())?
            .len(),
        1
    );
    assert_eq!(
        broker.provider_policy_versions(consumer.identity())?.len(),
        1
    );

    let mut crossed = rule(&enterprise)?;
    crossed.destination_id = consumer.destination_id().to_owned();
    let policy = broker.install_policy(PolicySnapshot::from_rules(vec![crossed])?)?;
    let denied = broker.evaluate(request(policy, &consumer, 10)?, 10)?;
    assert_eq!(
        denied.receipt.fingerprint().reason_code,
        Some(ReasonCode::ProviderPolicyIncompatible)
    );
    Ok(())
}

#[test]
fn incompatible_residency_denies() -> Result<(), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile()?;
    let provider = broker.register_provider_policy(
        provider_draft(identity(ProviderSurface::EnterpriseApi)?, 0, 1_000),
        0,
    )?;
    broker.record_provider_user_policy(ProviderUserPolicy {
        policy_id: "synthetic-residency-policy".to_owned(),
        provider_identity: provider.identity().clone(),
        provider_policy_snapshot_digest: provider.snapshot_digest().clone(),
        allowed_residency_regions: vec!["eu-west".to_owned()],
        allow_without_deletion_api: false,
        require_transit_encryption: true,
        require_at_rest_encryption: true,
        decision_evidence_id: "synthetic-user-decision".to_owned(),
        recorded_at: 1,
    })?;
    let policy = install_rule(&broker, &provider)?;
    let denied = broker.evaluate(request(policy, &provider, 2)?, 2)?;
    assert_eq!(denied.receipt.fingerprint().decision, Decision::Deny);
    assert_eq!(
        denied.receipt.fingerprint().reason_code,
        Some(ReasonCode::ProviderPolicyIncompatible)
    );
    Ok(())
}

#[test]
fn provider_without_deletion_api_requires_explicit_user_policy() -> Result<(), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile()?;
    let mut draft = provider_draft(identity(ProviderSurface::EnterpriseApi)?, 0, 1_000);
    draft.deletion_api_available = Some(false);
    draft.deletion_receipt_capable = Some(false);
    let provider = broker.register_provider_policy(draft, 0)?;
    let policy = install_rule(&broker, &provider)?;
    let denied = broker.evaluate(request(policy.clone(), &provider, 10)?, 10)?;
    let denied_fingerprint = denied.receipt.fingerprint().clone();
    assert_eq!(denied.receipt.fingerprint().decision, Decision::Deny);
    assert_eq!(
        denied.receipt.fingerprint().reason_code,
        Some(ReasonCode::NoDeletionReceipt)
    );

    broker.record_provider_user_policy(ProviderUserPolicy {
        policy_id: "synthetic-no-deletion-api-exception".to_owned(),
        provider_identity: provider.identity().clone(),
        provider_policy_snapshot_digest: provider.snapshot_digest().clone(),
        allowed_residency_regions: vec!["us-east".to_owned()],
        allow_without_deletion_api: true,
        require_transit_encryption: true,
        require_at_rest_encryption: true,
        decision_evidence_id: "synthetic-explicit-user-policy".to_owned(),
        // Deliberately backdate to the original evaluation time. The receipt's
        // registry revision ceiling must still keep replay deterministic.
        recorded_at: 10,
    })?;
    assert_eq!(broker.replay(&denied.receipt, 11)?, denied_fingerprint);
    let allowed = broker.evaluate(request(policy, &provider, 12)?, 12)?;
    assert_eq!(allowed.receipt.fingerprint().decision, Decision::Allow);
    assert!(allowed.capability.is_some());
    Ok(())
}

#[test]
fn deletion_receipt_is_immutable_and_linked() -> Result<(), Box<dyn Error>> {
    let broker = PermissionBroker::new_profile()?;
    let provider = broker.register_provider_policy(
        provider_draft(identity(ProviderSurface::EnterpriseApi)?, 0, 1_000),
        0,
    )?;
    let policy = install_rule(&broker, &provider)?;
    let issued = broker.evaluate(request(policy, &provider, 10)?, 10)?;
    let grant_id = issued.receipt.grant_id().ok_or("missing grant")?.to_owned();
    let capability = issued.capability.ok_or("missing capability")?;
    let issuance_audit_seq = broker
        .audit_rows()?
        .last()
        .ok_or("missing issuance audit")?
        .audit_seq;
    // Use the issuance timestamp again so time equality cannot be mistaken for
    // evidence that the issuance audit is the runtime-consumption audit.
    broker.execute(&capability, runtime(&provider)?, 10, |_| ())?;
    let audit_seq = broker
        .audit_rows()?
        .last()
        .ok_or("missing allow audit")?
        .audit_seq;
    let draft = DeletionReceiptDraft {
        receipt_id: "synthetic-provider-receipt".to_owned(),
        grant_id: grant_id.clone(),
        egress_audit_seq: audit_seq,
        provider_receipt_digest: digest("synthetic-provider-receipt-bytes"),
        requested_at: 20,
        received_at: 21,
    };
    let mut wrong_parent = draft.clone();
    wrong_parent.receipt_id = "synthetic-mislinked-receipt".to_owned();
    wrong_parent.egress_audit_seq = issuance_audit_seq;
    assert!(matches!(
        broker.store_deletion_receipt(wrong_parent),
        Err(BrokerError::InvalidDeletionReceipt)
    ));
    let mut impossible_time = draft.clone();
    impossible_time.receipt_id = "synthetic-pre-transmission-receipt".to_owned();
    impossible_time.requested_at = 9;
    impossible_time.received_at = 9;
    assert!(matches!(
        broker.store_deletion_receipt(impossible_time),
        Err(BrokerError::InvalidDeletionReceipt)
    ));
    let stored = broker.store_deletion_receipt(draft.clone())?;
    assert_eq!(stored.grant_id, grant_id);
    assert_eq!(stored.egress_audit_seq, audit_seq);
    assert_eq!(
        stored.provider_policy_snapshot_digest,
        provider.snapshot_digest().clone()
    );
    assert_eq!(
        broker.deletion_receipt(&draft.receipt_id)?,
        Some(stored.clone())
    );

    let mut mutation = draft;
    mutation.provider_receipt_digest = digest("mutated-receipt");
    assert!(broker.store_deletion_receipt(mutation).is_err());
    assert_eq!(
        broker.deletion_receipt(&stored.receipt_id)?,
        Some(stored.clone())
    );

    let injected = rusqlite::Connection::open_in_memory()?;
    injected.execute_batch(academic_policy::POLICY_SCHEMA_SQL)?;
    injected.execute_batch("PRAGMA foreign_keys = OFF;")?;
    injected.execute(
        "INSERT INTO egress_consumption (grant_id, egress_audit_seq, consumed_at) VALUES ('grant', 1, 1)",
        [],
    )?;
    assert!(
        injected
            .execute(
                "UPDATE egress_consumption SET consumed_at = 2 WHERE grant_id = 'grant'",
                [],
            )
            .is_err()
    );
    assert!(
        injected
            .execute(
                "DELETE FROM egress_consumption WHERE grant_id = 'grant'",
                []
            )
            .is_err()
    );
    injected.execute(
        concat!(
            "INSERT INTO provider_deletion_receipt (receipt_id, grant_id, egress_audit_seq, ",
            "provider_policy_snapshot_digest, provider_receipt_digest, requested_at, received_at) ",
            "VALUES ('injected', 'grant', 1, ?1, ?2, 1, 2)"
        ),
        [digest("snapshot").as_str(), digest("receipt").as_str()],
    )?;
    assert!(
        injected
            .execute(
                "UPDATE provider_deletion_receipt SET received_at = 3 WHERE receipt_id = 'injected'",
                [],
            )
            .is_err()
    );
    assert!(
        injected
            .execute(
                "DELETE FROM provider_deletion_receipt WHERE receipt_id = 'injected'",
                [],
            )
            .is_err()
    );
    Ok(())
}
