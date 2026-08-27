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
