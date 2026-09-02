//! SQLite authorizer for the product writer's append-only canonical tables.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use rusqlite::{
    Connection,
    hooks::{AuthAction, AuthContext, Authorization},
};

use crate::error::StoreResult;

/// Tables whose rows are canonical history and may only be appended.
pub const CANONICAL_TABLES: &[&str] = &[
    "schema_meta",
    "ledger_batch",
    "ledger_event",
    "scope",
    "artifact_descriptor",
    "artifact_representation",
    "evidence_item",
    "claim",
    "claim_evidence",
    "claim_relation",
    "user_decision",
    "projection_outbox",
    "command_receipt",
    // Migration 0004 aggregate closure tables, in event schema v3 Proto tag
    // order. They are canonical history on exactly the same terms as the set
    // above: the trigger pair in the migration is the first enforcement layer,
    // and this list is the second. A table missing from here would keep its
    // triggers but lose the authorizer's blanket denial of DROP and ALTER.
    "curriculum_version",
    "course_revision",
    "offering",
    "attempt",
    "requirement_set",
    "audit",
    "capture_permission",
    "lecture_session",
    "transcript_version",
    "lecture_document",
    "snapshot",
    "finding",
    "model_run",
    "proposal_disposition",
    "egress_decision",
    "consent",
    "entity_identity_change",
    "retention_action",
    // Migration 0005's typed columns for the RETENTION_ACTION_RECORDED
    // aggregate. It is canonical history: the reference an artifact resolves to
    // is decided by the rows here, so an UPDATE or a DELETE would move an
    // object reference without an event authorizing it.
    "artifact_descriptor_migration",
    // Migration 0006's typed columns for the CAPTURE_PERMISSION_RECORDED and
    // CONSENT_RECORDED aggregates. They are canonical history for the same
    // reason: whether a recorder may run at all is decided by the rows here, so
    // an UPDATE or a DELETE would move a permission without an event
    // authorizing it.
    "capture_permission_terms",
    "capture_permission_medium",
    "capture_permission_processing",
    "capture_permission_checklist",
    "consent_record",
];

/// Installs the product-writer guard after migration and identity verification.
pub(crate) fn install_canonical_authorizer(
    connection: &Connection,
    acceptance_authorized: Arc<AtomicBool>,
) -> StoreResult<()> {
    connection.authorizer(Some(move |context: AuthContext<'_>| -> Authorization {
        authorize_product_statement(context, acceptance_authorized.load(Ordering::Acquire))
    }))?;
    Ok(())
}

/// Keeps callers from disabling query-only mode or using temporary write surfaces.
pub(crate) fn install_reader_authorizer(connection: &Connection) -> StoreResult<()> {
    connection.authorizer(Some(authorize_reader_statement))?;
    Ok(())
}

fn authorize_product_statement(
    context: AuthContext<'_>,
    acceptance_authorized: bool,
) -> Authorization {
    match context.action {
        AuthAction::Select
        | AuthAction::Read { .. }
        | AuthAction::Function { .. }
        | AuthAction::Recursive
        | AuthAction::Transaction { .. }
        | AuthAction::Savepoint { .. }
        | AuthAction::Pragma {
            pragma_value: None, ..
        } => Authorization::Allow,
        AuthAction::Insert { table_name }
            if acceptance_authorized && is_acceptance_insert_table(table_name) =>
        {
            Authorization::Allow
        }
        AuthAction::Update { table_name, .. }
            if acceptance_authorized && is_acceptance_update_table(table_name) =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }
}

fn is_canonical_table(table_name: &str) -> bool {
    CANONICAL_TABLES
        .iter()
        .any(|canonical| table_name.eq_ignore_ascii_case(canonical))
}

fn is_acceptance_insert_table(table_name: &str) -> bool {
    (is_canonical_table(table_name) && !table_name.eq_ignore_ascii_case("schema_meta"))
        || table_name.eq_ignore_ascii_case("device_head")
}

fn is_acceptance_update_table(table_name: &str) -> bool {
    table_name.eq_ignore_ascii_case("device_head")
        || table_name.eq_ignore_ascii_case("replica_state")
}

fn authorize_reader_statement(context: AuthContext<'_>) -> Authorization {
    match context.action {
        AuthAction::Select
        | AuthAction::Read { .. }
        | AuthAction::Function { .. }
        | AuthAction::Recursive
        | AuthAction::Transaction { .. }
        | AuthAction::Savepoint { .. }
        | AuthAction::Pragma {
            pragma_value: None, ..
        } => Authorization::Allow,
        _ => Authorization::Deny,
    }
}
