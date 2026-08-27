//! SQLite authorizer for the product writer's append-only canonical tables.

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
pub(crate) fn install_canonical_authorizer(connection: &Connection) -> StoreResult<()> {
    connection.authorizer(Some(authorize_product_statement))?;
    Ok(())
}

/// Keeps callers from disabling query-only mode or using temporary write surfaces.
pub(crate) fn install_reader_authorizer(connection: &Connection) -> StoreResult<()> {
    connection.authorizer(Some(authorize_reader_statement))?;
    Ok(())
}

fn authorize_product_statement(context: AuthContext<'_>) -> Authorization {
    if matches!(
        context.action,
        AuthAction::Pragma {
            pragma_value: Some(_),
            ..
        } | AuthAction::Attach { .. }
            | AuthAction::Detach { .. }
    ) {
        return Authorization::Deny;
    }
    let protected_table = match context.action {
        AuthAction::Update { table_name, .. }
        | AuthAction::Delete { table_name }
        | AuthAction::DropTable { table_name }
        | AuthAction::AlterTable { table_name, .. }
        | AuthAction::DropIndex { table_name, .. }
        | AuthAction::DropTrigger { table_name, .. } => Some(table_name),
        _ => None,
    };
    if protected_table.is_some_and(is_canonical_table) {
        Authorization::Deny
    } else {
        Authorization::Allow
    }
}

fn is_canonical_table(table_name: &str) -> bool {
    CANONICAL_TABLES
        .iter()
        .any(|canonical| table_name.eq_ignore_ascii_case(canonical))
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
