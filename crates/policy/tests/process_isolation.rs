use std::error::Error;

use academic_policy::{
    AUDIT_RETENTION_POLICY_ID, BrokerError, ContentDigest, Decision, ObjectRange, PermissionBroker,
    ProcessActivity, ProcessCapability, ProcessClass, ReasonCode,
};

type TestResult = Result<(), Box<dyn Error>>;

const ALLOWED_CELLS: &[(ProcessClass, ProcessCapability)] = &[
    (
        ProcessClass::CaptureClient,
        ProcessCapability::CaptureDevice,
    ),
    (
        ProcessClass::CaptureClient,
        ProcessCapability::WriteStagedArtifact,
    ),
    (ProcessClass::Indexer, ProcessCapability::ReadArtifactRange),
    (ProcessClass::Indexer, ProcessCapability::WriteSearchIndex),
    (
        ProcessClass::RepositoryAnalyzer,
        ProcessCapability::ReadArtifactRange,
    ),
    (
        ProcessClass::RepositoryAnalyzer,
        ProcessCapability::AnalyzeRepository,
    ),
    (
        ProcessClass::RepositoryAnalyzer,
        ProcessCapability::CreateClaim,
    ),
    (
        ProcessClass::Connector,
        ProcessCapability::BorrowConnectorCredential,
    ),
    (
        ProcessClass::Connector,
        ProcessCapability::StageExternalPayload,
    ),
    (
        ProcessClass::EgressProxy,
        ProcessCapability::OpenOutboundSocket,
    ),
    (
        ProcessClass::ExportJob,
        ProcessCapability::ReadArtifactRange,
    ),
    (ProcessClass::ExportJob, ProcessCapability::AssembleExport),
];

fn range(label: &str, bytes: &[u8]) -> Result<ObjectRange, BrokerError> {
    ObjectRange::new(
        format!("synthetic-{label}"),
        0,
        u64::try_from(bytes.len()).map_err(|_| BrokerError::InvalidRange)?,
        ContentDigest::of(bytes),
    )
}

fn activity_for(capability: ProcessCapability) -> Result<ProcessActivity<'static>, BrokerError> {
    match capability {
        ProcessCapability::ReadArtifactRange => {
            ProcessActivity::artifact_read(vec![range("read", b"read")?])
        }
        ProcessCapability::OpenOutboundSocket => ProcessActivity::external_transmission(
            vec![range("transmit", b"send")?],
            "synthetic-provider",
            b"send",
        ),
        ProcessCapability::CreateClaim => ProcessActivity::claims_created(
            vec![range("claim", b"claim")?],
            vec!["synthetic-claim".to_owned()],
        ),
        ProcessCapability::CaptureDevice
        | ProcessCapability::WriteStagedArtifact
        | ProcessCapability::WriteSearchIndex
        | ProcessCapability::AnalyzeRepository
        | ProcessCapability::BorrowConnectorCredential
        | ProcessCapability::StageExternalPayload
        | ProcessCapability::AssembleExport
        | ProcessCapability::ReadKeyMaterial => ProcessActivity::capability_use(capability),
    }
}

#[test]
fn cross_capability_matrix_denies_every_disallowed_cell() -> TestResult {
    assert_eq!(
        ProcessClass::ALL,
        [
            ProcessClass::CaptureClient,
            ProcessClass::Indexer,
            ProcessClass::RepositoryAnalyzer,
            ProcessClass::Connector,
            ProcessClass::EgressProxy,
            ProcessClass::ExportJob,
        ]
    );
    assert_eq!(
        ProcessCapability::ALL,
        [
            ProcessCapability::CaptureDevice,
            ProcessCapability::WriteStagedArtifact,
            ProcessCapability::ReadArtifactRange,
            ProcessCapability::WriteSearchIndex,
            ProcessCapability::AnalyzeRepository,
            ProcessCapability::BorrowConnectorCredential,
            ProcessCapability::StageExternalPayload,
            ProcessCapability::OpenOutboundSocket,
            ProcessCapability::CreateClaim,
            ProcessCapability::AssembleExport,
            ProcessCapability::ReadKeyMaterial,
        ]
    );

    // The six sets are distinct. Without this, two classes could hold the same
    // privileges and the enumeration below would still pass, which would make
    // the split six names for one privilege level rather than a split.
    for (index, left) in ProcessClass::ALL.iter().enumerate() {
        for right in ProcessClass::ALL.iter().skip(index + 1) {
            assert_ne!(
                left.capabilities(),
                right.capabilities(),
                "{} and {} have the same capability set",
                left.as_str(),
                right.as_str(),
            );
        }
    }

    let broker = PermissionBroker::new_profile_with_ttl(10_000)?;
    let mut now = 100_u64;
    for process_class in ProcessClass::ALL {
        for capability in ProcessCapability::ALL {
            now += 1;
            let expected = ALLOWED_CELLS.contains(&(process_class, capability));
            assert_eq!(
                process_class.allows(capability),
                expected,
                "matrix drift at {} × {}",
                process_class.as_str(),
                capability.as_str(),
            );
            if !expected {
                assert!(matches!(
                    broker.mint_process_capability(
                        "synthetic-user",
                        process_class,
                        capability,
                        now,
                    ),
                    Err(BrokerError::Denied(ReasonCode::NoGrant))
                ));
                continue;
            }

            let exact =
                broker.mint_process_capability("synthetic-user", process_class, capability, now)?;
            broker.use_process_capability(
                &exact,
                "synthetic-user",
                process_class,
                capability,
                activity_for(capability)?,
                now + 1,
            )?;

            for injected_class in ProcessClass::ALL {
                if injected_class == process_class {
                    continue;
                }
                let token = broker.mint_process_capability(
                    "synthetic-user",
                    process_class,
                    capability,
                    now + 2,
                )?;
                assert!(matches!(
                    broker.use_process_capability(
                        &token,
                        "synthetic-user",
                        injected_class,
                        capability,
                        activity_for(capability)?,
                        now + 3,
                    ),
                    Err(BrokerError::Denied(ReasonCode::ScopeMismatch))
                ));
            }

            for injected_capability in ProcessCapability::ALL {
                if injected_capability == capability {
                    continue;
                }
                let token = broker.mint_process_capability(
                    "synthetic-user",
                    process_class,
                    capability,
                    now + 4,
                )?;
                assert!(matches!(
                    broker.use_process_capability(
                        &token,
                        "synthetic-user",
                        process_class,
                        injected_capability,
                        activity_for(injected_capability)?,
                        now + 5,
                    ),
                    Err(BrokerError::Denied(ReasonCode::ScopeMismatch))
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn audit_records_actor_range_transmission_and_claims() -> TestResult {
    let broker = PermissionBroker::new_profile_with_ttl(1_000)?;
    let read_bytes = b"synthetic-index-input";
    let read_range = range("index-input", read_bytes)?;
    let read = broker.mint_process_capability(
        "synthetic-user",
        ProcessClass::Indexer,
        ProcessCapability::ReadArtifactRange,
        100,
    )?;
    broker.use_process_capability(
        &read,
        "synthetic-user",
        ProcessClass::Indexer,
        ProcessCapability::ReadArtifactRange,
        ProcessActivity::artifact_read(vec![read_range.clone()])?,
        101,
    )?;
    let transmitted = b"synthetic-minimized-slice";
    let transmitted_range = range("egress-slice", transmitted)?;
    let egress = broker.mint_process_capability(
        "synthetic-user",
        ProcessClass::EgressProxy,
        ProcessCapability::OpenOutboundSocket,
        110,
    )?;
    broker.use_process_capability(
        &egress,
        "synthetic-user",
        ProcessClass::EgressProxy,
        ProcessCapability::OpenOutboundSocket,
        ProcessActivity::external_transmission(
            vec![transmitted_range.clone()],
            "synthetic-provider",
            transmitted,
        )?,
        111,
    )?;

    let claim_range = range("claim-source", b"claim-source")?;
    let claims = broker.mint_process_capability(
        "synthetic-user",
        ProcessClass::RepositoryAnalyzer,
        ProcessCapability::CreateClaim,
        120,
    )?;
    broker.use_process_capability(
        &claims,
        "synthetic-user",
        ProcessClass::RepositoryAnalyzer,
        ProcessCapability::CreateClaim,
        ProcessActivity::claims_created(
            vec![claim_range.clone()],
            vec![
                "claim-synthetic-1".to_owned(),
                "claim-synthetic-2".to_owned(),
            ],
        )?,
        121,
    )?;

    let rows = broker.audit_rows()?;
    let read_row = rows
        .iter()
        .find(|row| {
            row.capability == ProcessCapability::ReadArtifactRange
                && !row.artifact_ranges.is_empty()
        })
        .ok_or("missing read audit")?;
    assert_eq!(read_row.actor_id, "synthetic-user");
    assert_eq!(read_row.process_class, ProcessClass::Indexer);
    assert_eq!(read_row.artifact_ranges, [read_range]);
    assert_eq!(read_row.retention_policy_id, AUDIT_RETENTION_POLICY_ID);

    let transmission_row = rows
        .iter()
        .find(|row| row.external_transmission_digest.is_some())
        .ok_or("missing transmission audit")?;
    assert_eq!(transmission_row.process_class, ProcessClass::EgressProxy);
    assert_eq!(transmission_row.destination_id, "synthetic-provider");
    assert_eq!(transmission_row.artifact_ranges, [transmitted_range]);
    assert_eq!(
        transmission_row.external_transmission_digest.as_deref(),
        Some(ContentDigest::of(transmitted).as_str())
    );
    assert_eq!(transmission_row.byte_count, transmitted.len() as u64);

    let claim_row = rows
        .iter()
        .find(|row| !row.created_claim_ids.is_empty())
        .ok_or("missing claim audit")?;
    assert_eq!(claim_row.process_class, ProcessClass::RepositoryAnalyzer);
    assert_eq!(claim_row.artifact_ranges, [claim_range]);
    assert_eq!(
        claim_row.created_claim_ids,
        ["claim-synthetic-1", "claim-synthetic-2"]
    );
    assert_eq!(claim_row.decision, Decision::Allow);
    Ok(())
}

/// One table name and its column names, in the order the schema declares them.
type TableColumns = (String, Vec<String>);

/// The complete applied schema: every table and every one of its columns.
///
/// `T126`'s `P1-1` finding was a guard that read a list of forbidden tokens out
/// of one file, and five real bypasses walked past it. A blocklist of column
/// names has the same hole: it cannot see a column, or a whole side table,
/// whose name nobody thought to forbid. So this enumerates what the schema
/// actually applies and compares the whole thing. A new raw-content column or
/// table fails here whatever it is called.
fn applied_schema_columns() -> Result<Vec<TableColumns>, Box<dyn Error>> {
    let connection = rusqlite::Connection::open_in_memory()?;
    connection.execute_batch(academic_policy::POLICY_SCHEMA_SQL)?;
    let mut tables_statement = connection.prepare(concat!(
        "SELECT name FROM sqlite_master WHERE type = 'table' ",
        "AND name NOT LIKE 'sqlite_%' ORDER BY name"
    ))?;
    let tables = tables_statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(tables_statement);
    let mut schema = Vec::new();
    for table in tables {
        let mut columns_statement =
            connection.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
        let columns = columns_statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(columns_statement);
        schema.push((table, columns));
    }
    Ok(schema)
}

fn expected_schema_columns() -> Vec<TableColumns> {
    [
        (
            "audit_artifact_range",
            &[
                "audit_seq",
                "range_ordinal",
                "object_id",
                "byte_start",
                "byte_end",
                "content_digest",
            ][..],
        ),
        (
            "audit_created_claim",
            &["audit_seq", "claim_ordinal", "claim_id"][..],
        ),
        (
            "egress_audit",
            &[
                "audit_seq",
                "grant_id",
                "decision",
                "reason_code",
                "actor_id",
                "actor_process_class",
                "capability",
                "payload_digest",
                "byte_count",
                "destination_id",
                "started_at",
                "finished_at",
                "provider_response_digest",
                "deletion_receipt_id",
                "external_transmission_digest",
                "retention_policy_id",
            ][..],
        ),
        (
            "egress_consumption",
            &["grant_id", "egress_audit_seq", "consumed_at"][..],
        ),
        (
            "egress_grant",
            &[
                "grant_id",
                "request_digest",
                "payload_digest",
                "byte_ranges_canonical",
                "purpose_id",
                "provider_id",
                "provider_policy_snapshot_digest",
                "retention_terms_hash",
                "training_use_allowed",
                "redaction_policy_hash",
                "issued_at",
                "expires_at",
                "max_uses",
                "consumed_at",
                "consent_event_id",
            ][..],
        ),
        (
            "policy_schema_meta",
            &["singleton", "schema_version", "audit_retention_policy_id"][..],
        ),
        (
            "process_capability_grant",
            &[
                "token_id",
                "actor_id",
                "process_class",
                "capability",
                "issued_at",
                "expires_at",
                "max_uses",
                "consumed_at",
            ][..],
        ),
        (
            "provider_deletion_receipt",
            &[
                "receipt_seq",
                "receipt_id",
                "grant_id",
                "egress_audit_seq",
                "provider_policy_snapshot_digest",
                "provider_receipt_digest",
                "requested_at",
                "received_at",
            ][..],
        ),
        (
            "provider_policy_residency",
            &["snapshot_digest", "region"][..],
        ),
        (
            "provider_policy_snapshot",
            &[
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
            ][..],
        ),
        (
            "provider_policy_subprocessor",
            &["snapshot_digest", "subprocessor"][..],
        ),
        (
            "provider_user_policy",
            &[
                "user_policy_seq",
                "policy_id",
                "destination_id",
                "provider_policy_snapshot_digest",
                "allow_without_deletion_api",
                "require_transit_encryption",
                "require_at_rest_encryption",
                "decision_evidence_id",
                "recorded_at",
            ][..],
        ),
        (
            "provider_user_policy_residency",
            &["policy_id", "region"][..],
        ),
    ]
    .into_iter()
    .map(|(table, columns)| {
        (
            table.to_owned(),
            columns.iter().map(|column| (*column).to_owned()).collect(),
        )
    })
    .collect()
}

/// Each corpus canary paired with the exact, prefixed, case-changed, and
/// reversed spelling the audit path must not retain.
fn canary_variants() -> Vec<(String, String)> {
    include_str!("../../../testdata/sqlcipher-canary/canaries.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .flat_map(|canary| {
            [
                canary.to_owned(),
                format!("prefix::{canary}::suffix"),
                canary.to_ascii_lowercase(),
                canary.chars().rev().collect::<String>(),
            ]
            .into_iter()
            .map(|variant| (canary.to_owned(), variant))
        })
        .collect()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[test]
fn audit_contains_no_raw_canary() -> TestResult {
    // The schema may hold no place to put raw content, whatever it is named.
    assert_eq!(
        applied_schema_columns()?,
        expected_schema_columns(),
        "the policy schema gained or lost a table or column; a new one must be reviewed for raw content"
    );

    let variants = canary_variants();
    assert!(variants.len() >= 20, "the canary corpus shrank");

    // What the crate hands a caller back carries digests and counts only.
    for (index, (canary, variant)) in variants.iter().enumerate() {
        let broker = PermissionBroker::new_profile()?;
        let token = broker.mint_process_capability(
            "synthetic-canary-actor",
            ProcessClass::EgressProxy,
            ProcessCapability::OpenOutboundSocket,
            100,
        )?;
        let object_range = ObjectRange::new(
            format!("canary-{index}"),
            0,
            u64::try_from(variant.len())?,
            ContentDigest::of(variant.as_bytes()),
        )?;
        broker.use_process_capability(
            &token,
            "synthetic-canary-actor",
            ProcessClass::EgressProxy,
            ProcessCapability::OpenOutboundSocket,
            ProcessActivity::external_transmission(
                vec![object_range],
                "synthetic-provider",
                variant.as_bytes(),
            )?,
            101,
        )?;
        let rendered = format!("{:?}", broker.audit_rows()?);
        assert!(
            !rendered.contains(canary.as_str()),
            "raw corpus canary leaked"
        );
        assert!(
            !rendered.contains(variant.as_str()),
            "raw canary variant leaked"
        );
        assert!(rendered.contains(ContentDigest::of(variant.as_bytes()).as_str()));
    }

    // And so does what is actually retained. A row the read API does not
    // project back is still a copy of the bytes, so this reads the whole
    // retained database rather than the crate's own view of it.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "academic-policy-canary-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root)?;
    let path = root.join("egress.sqlite3");
    let broker = PermissionBroker::open(&path)?;
    for (index, (_, variant)) in variants.iter().enumerate() {
        let issued_at = 1_000 + u64::try_from(index)? * 10;
        let token = broker.mint_process_capability(
            "synthetic-canary-actor",
            ProcessClass::EgressProxy,
            ProcessCapability::OpenOutboundSocket,
            issued_at,
        )?;
        broker.use_process_capability(
            &token,
            "synthetic-canary-actor",
            ProcessClass::EgressProxy,
            ProcessCapability::OpenOutboundSocket,
            ProcessActivity::external_transmission(
                vec![ObjectRange::new(
                    format!("canary-disk-{index}"),
                    0,
                    u64::try_from(variant.len())?,
                    ContentDigest::of(variant.as_bytes()),
                )?],
                "synthetic-provider",
                variant.as_bytes(),
            )?,
            issued_at + 1,
        )?;
    }
    drop(broker);

    let mut retained = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let file = entry.path();
        let bytes = std::fs::read(&file)?;
        retained.push((file, bytes));
    }
    assert!(
        !retained.is_empty(),
        "the retained audit database is missing"
    );
    for (file, bytes) in &retained {
        for (canary, variant) in &variants {
            assert!(
                !contains(bytes, canary.as_bytes()),
                "raw corpus canary is in {}",
                file.display()
            );
            assert!(
                !contains(bytes, variant.as_bytes()),
                "raw canary variant is in {}",
                file.display()
            );
        }
    }
    // The digest that replaces those bytes is retained, so the scan above read
    // a database that actually recorded the transmissions.
    let digest = ContentDigest::of(variants[0].1.as_bytes());
    assert!(
        retained
            .iter()
            .any(|(_, bytes)| contains(bytes, digest.as_str().as_bytes())),
        "the retained database has no transmission digest to have scanned"
    );

    for (file, _) in &retained {
        std::fs::remove_file(file)?;
    }
    std::fs::remove_dir(&root)?;
    Ok(())
}

#[test]
fn audit_retention_is_append_only_and_survives_reopen() -> TestResult {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "academic-policy-retention-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root)?;
    let path = root.join("egress.sqlite3");

    let broker = PermissionBroker::open(&path)?;
    let token = broker.mint_process_capability(
        "synthetic-user",
        ProcessClass::Indexer,
        ProcessCapability::ReadArtifactRange,
        100,
    )?;
    broker.use_process_capability(
        &token,
        "synthetic-user",
        ProcessClass::Indexer,
        ProcessCapability::ReadArtifactRange,
        ProcessActivity::artifact_read(vec![range("retained", b"retained")?])?,
        101,
    )?;
    let claim_token = broker.mint_process_capability(
        "synthetic-user",
        ProcessClass::RepositoryAnalyzer,
        ProcessCapability::CreateClaim,
        102,
    )?;
    broker.use_process_capability(
        &claim_token,
        "synthetic-user",
        ProcessClass::RepositoryAnalyzer,
        ProcessCapability::CreateClaim,
        ProcessActivity::claims_created(
            vec![range("retained-claim", b"claim")?],
            vec!["retained-claim".to_owned()],
        )?,
        103,
    )?;
    let expected = broker.audit_rows()?;
    drop(broker);

    let connection = rusqlite::Connection::open(&path)?;
    assert!(connection.execute("DELETE FROM egress_audit", []).is_err());
    assert!(
        connection
            .execute("DELETE FROM audit_artifact_range", [])
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM audit_created_claim", [])
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM process_capability_grant", [])
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM policy_schema_meta", [])
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE egress_audit SET actor_id = 'replacement' WHERE audit_seq = 1",
                [],
            )
            .is_err()
    );
    drop(connection);

    let reopened = PermissionBroker::open(&path)?;
    assert_eq!(reopened.audit_rows()?, expected);
    drop(reopened);
    std::fs::remove_file(&path)?;
    std::fs::remove_dir(&root)?;
    Ok(())
}
