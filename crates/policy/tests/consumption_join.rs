//! `egress_consumption` resolves `egress_audit.grant_id` for a consumed grant.
//!
//! `egress_audit.grant_id` carries identifiers from two tables and has no
//! foreign key of its own: `P2-G7` removed one so process-activity rows could be
//! written at all, and `T146` measured that the typed
//! `(process_class, capability)` pair does not discriminate them --
//! `EGRESS_PROXY` x `OPEN_OUTBOUND_SOCKET` is the cell where the two namespaces
//! overlap exactly.
//!
//! What resolves it for a transfer that happened is `egress_consumption`, whose
//! two foreign keys hold together: `grant_id` references `egress_grant`, and
//! `(egress_audit_seq, grant_id)` references `egress_audit(audit_seq,
//! grant_id)`, which `egress_audit`'s own `UNIQUE (audit_seq, grant_id)` makes
//! declarable. `P2-M1`'s reconciliation keys on that join, so these tests are
//! what stop the join being assumed rather than enforced. Deleting either
//! foreign key from the schema fails one of them.

use std::error::Error;

use academic_policy::POLICY_SCHEMA_SQL;
use rusqlite::{Connection, params};

const AUDIT_GRANT: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const OTHER_GRANT: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
const PROCESS_TOKEN: &str = "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";
const SNAPSHOT_DIGEST: &str = "d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4";

/// The applied schema with foreign keys on, as `PermissionBroker::open` runs it.
fn schema() -> Result<Connection, Box<dyn Error>> {
    let connection = Connection::open_in_memory()?;
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    connection.execute_batch(POLICY_SCHEMA_SQL)?;
    let enabled: i64 = connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
    assert_eq!(
        enabled, 1,
        "foreign keys are off, so nothing below is checked"
    );
    Ok(connection)
}

/// Inserts one audit row naming `grant_id`, and returns its sequence.
fn audit_row(connection: &Connection, grant_id: &str) -> Result<i64, Box<dyn Error>> {
    connection.execute(
        concat!(
            "INSERT INTO egress_audit (grant_id, decision, reason_code, actor_id, ",
            "actor_process_class, capability, payload_digest, byte_count, destination_id, ",
            "started_at, finished_at, retention_policy_id) ",
            "VALUES (?1, 'ALLOW', NULL, 'synthetic-user', 'EGRESS_PROXY', ",
            "'OPEN_OUTBOUND_SOCKET', NULL, 8, 'provider-y', 1, 1, ",
            "'SECURITY_AUDIT_APPEND_ONLY')"
        ),
        params![grant_id],
    )?;
    Ok(connection.last_insert_rowid())
}

/// Registers the provider snapshot both grants below reference.
fn provider_snapshot(connection: &Connection) -> Result<(), Box<dyn Error>> {
    connection.execute(
        concat!(
            "INSERT INTO provider_policy_snapshot (snapshot_digest, destination_id, vendor_id, ",
            "surface, training_use_enabled, training_opt_out_applied, server_retention_millis, ",
            "abuse_logging_enabled, transit_encryption_declared, at_rest_encryption_declared, ",
            "deletion_api_available, deletion_receipt_capable, maximum_input_bytes, ",
            "logging_configuration, policy_source_digest, last_verified_at, ttl_millis, ",
            "registered_at) VALUES (?1, 'provider-y', 'provider-y', 'ENTERPRISE_API', 0, 0, 0, ",
            "0, 1, 1, 1, 1, 1024, 'content-logging-disabled', ?1, 0, 1000, 0)"
        ),
        params![SNAPSHOT_DIGEST],
    )?;
    Ok(())
}

/// Mints one real `egress_grant` row.
fn mint_grant(connection: &Connection, grant_id: &str) -> Result<(), Box<dyn Error>> {
    connection.execute(
        concat!(
            "INSERT INTO egress_grant (grant_id, request_digest, payload_digest, ",
            "byte_ranges_canonical, purpose_id, provider_id, provider_policy_snapshot_digest, ",
            "retention_terms_hash, training_use_allowed, redaction_policy_hash, issued_at, ",
            "expires_at, max_uses, consumed_at, consent_event_id) ",
            "VALUES (?1, ?2, ?2, '10..18', 'concept-extraction', 'provider-y', ?2, ?2, 0, ?2, ",
            "1, 10, 1, NULL, 'synthetic-consent-event')"
        ),
        params![grant_id, SNAPSHOT_DIGEST],
    )?;
    Ok(())
}

fn is_foreign_key_violation(error: &rusqlite::Error) -> bool {
    error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation)
        && error.to_string().contains("FOREIGN KEY constraint failed")
}

/// A consumption row cannot name an identifier from the other namespace, and
/// cannot name an audit row that carries a different identifier.
#[test]
fn a_consumption_row_cannot_name_a_grant_the_audit_row_does_not() -> Result<(), Box<dyn Error>> {
    let connection = schema()?;

    // Two audit rows in the shape the two namespaces both produce: same
    // decision, same class, same capability, 64-hex identifier. One names an
    // egress grant that exists; the other names a process-capability token.
    let audit_seq = audit_row(&connection, AUDIT_GRANT)?;
    let token_audit_seq = audit_row(&connection, PROCESS_TOKEN)?;

    // The process-capability token really is a row in its own table, so what
    // the assertions below measure is the namespace and not a dangling value.
    connection.execute(
        concat!(
            "INSERT INTO process_capability_grant (token_id, actor_id, process_class, ",
            "capability, issued_at, expires_at, max_uses, consumed_at) ",
            "VALUES (?1, 'synthetic-user', 'EGRESS_PROXY', 'OPEN_OUTBOUND_SOCKET', 1, 2, 1, NULL)"
        ),
        params![PROCESS_TOKEN],
    )?;

    // A consumption naming the process token is refused: `grant_id` is a
    // foreign key to `egress_grant`, which holds no such row.
    let foreign_namespace = connection.execute(
        "INSERT INTO egress_consumption (grant_id, egress_audit_seq, consumed_at) VALUES (?1, ?2, 1)",
        params![PROCESS_TOKEN, token_audit_seq],
    );
    assert!(
        foreign_namespace
            .as_ref()
            .err()
            .is_some_and(is_foreign_key_violation),
        "a process-capability token was accepted as a consumed egress grant: {foreign_namespace:?}"
    );

    // A consumption naming a grant the audit row does not. Both grants are real
    // `egress_grant` rows here, so `grant_id`'s own foreign key is satisfied and
    // the composite one is the only thing left to refuse it. `M-I2` measured
    // that this case was reaching the simpler key instead: with `OTHER_GRANT`
    // unminted, dropping the composite key changed nothing and the test passed.
    provider_snapshot(&connection)?;
    mint_grant(&connection, AUDIT_GRANT)?;
    mint_grant(&connection, OTHER_GRANT)?;
    let mismatched = connection.execute(
        "INSERT INTO egress_consumption (grant_id, egress_audit_seq, consumed_at) VALUES (?1, ?2, 1)",
        params![OTHER_GRANT, audit_seq],
    );
    assert!(
        mismatched
            .as_ref()
            .err()
            .is_some_and(is_foreign_key_violation),
        "a consumption named an audit row carrying another grant: {mismatched:?}"
    );

    // And with both keys satisfied the row is accepted, so the refusals above
    // measure the mismatch and not a schema that refuses every consumption row.
    connection.execute(
        "INSERT INTO egress_consumption (grant_id, egress_audit_seq, consumed_at) VALUES (?1, ?2, 1)",
        params![AUDIT_GRANT, audit_seq],
    )?;
    Ok(())
}

/// A consumption cannot name an egress grant nothing minted, even when the
/// audit row it points at carries exactly that identifier.
#[test]
fn a_consumption_row_cannot_name_a_grant_nothing_minted() -> Result<(), Box<dyn Error>> {
    let connection = schema()?;
    let audit_seq = audit_row(&connection, AUDIT_GRANT)?;
    let unminted = connection.execute(
        "INSERT INTO egress_consumption (grant_id, egress_audit_seq, consumed_at) VALUES (?1, ?2, 1)",
        params![AUDIT_GRANT, audit_seq],
    );
    assert!(
        unminted
            .as_ref()
            .err()
            .is_some_and(is_foreign_key_violation),
        "a consumption named a grant nothing minted: {unminted:?}"
    );
    Ok(())
}

/// The control: with both halves real, the same insert is accepted.
///
/// Without this the three refusals above would be consistent with a schema that
/// refuses every consumption row, which would make the join vacuous rather than
/// exact.
#[test]
fn a_consumption_row_naming_a_real_grant_and_its_audit_row_is_accepted()
-> Result<(), Box<dyn Error>> {
    let connection = schema()?;
    provider_snapshot(&connection)?;
    mint_grant(&connection, AUDIT_GRANT)?;
    let audit_seq = audit_row(&connection, AUDIT_GRANT)?;
    connection.execute(
        "INSERT INTO egress_consumption (grant_id, egress_audit_seq, consumed_at) VALUES (?1, ?2, 1)",
        params![AUDIT_GRANT, audit_seq],
    )?;
    let joined: String = connection.query_row(
        concat!(
            "SELECT audit.grant_id FROM egress_consumption AS consumption ",
            "JOIN egress_audit AS audit ON audit.audit_seq = consumption.egress_audit_seq ",
            "WHERE consumption.grant_id = ?1"
        ),
        params![AUDIT_GRANT],
        |row| row.get(0),
    )?;
    assert_eq!(joined, AUDIT_GRANT);
    Ok(())
}
