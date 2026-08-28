mod support;

use std::fs;

use academic_domain::ContentDigest;
use academic_ledger::AuthorityPolicy;
use academic_projections::{
    PROJECTION_SCHEMA_VERSION,
    generation::{ProjectionAvailability, ProjectionKind},
    runner::{
        MIGRATION_0002_SQL, MIGRATION_0003_SQL, PROJECTION_ALGORITHM_VERSION,
        PROJECTION_APPLICATION_ID, PROJECTION_DATABASE_VERSION, ProjectionError, ProjectionRunner,
    },
};
use academic_store::queries::canonical_snapshot;
use rusqlite::Connection;

use support::{
    Fixture, TestResult, claim_id, entity, importer_actor, observed_entity_claim, policies,
};

const PARENT_MIGRATION_0002_SQL: &str =
    include_str!("../../../migrations/store/fixtures/0002_phase1_projections_parent.sql");
const SQLITEX_TRIGGER_NAME: &str = "sqliteX_block_generation";
const SQLITEX_TRIGGER_SQL: &str = concat!(
    "CREATE TRIGGER sqliteX_block_generation ",
    "BEFORE INSERT ON projection_generation BEGIN ",
    "SELECT RAISE(ABORT, 'blocked by sqliteX trigger'); END;"
);

#[derive(Debug, PartialEq, Eq)]
struct SidecarState {
    application_id: i64,
    user_version: i64,
    schema: Vec<(String, String, String)>,
    generation_count: Option<i64>,
}

#[test]
fn exact_parent_v2_is_replaced_from_canonical_without_touching_vault() -> TestResult {
    let mut fixture = Fixture::new("exact-parent-v2")?;
    let evidence = fixture.register_scope_evidence(10, 1, b"parent v2 vault evidence")?;
    let subject = entity(10_001)?;
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        observed_entity_claim(
            claim_id(10_001)?,
            subject,
            "graph.related",
            entity(10_011)?,
            evidence.scope_id,
            evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    let before_canonical = canonical_snapshot(&fixture.store_reader()?)?;
    let before_vault = fs::read(&evidence.vault_object_path)?;

    let parent = Connection::open(fixture.sidecar_path())?;
    parent.execute_batch(PARENT_MIGRATION_0002_SQL)?;
    assert_eq!(
        parent.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?,
        2
    );
    drop(parent);

    let policies = policies(&[("graph.related", AuthorityPolicy::ImplementationObservation)])?;
    let runner = fixture.runner()?;
    let current = Connection::open(fixture.sidecar_path())?;
    assert_eq!(
        current.query_row("PRAGMA application_id", [], |row| row.get::<_, i64>(0))?,
        i64::from(PROJECTION_APPLICATION_ID)
    );
    assert_eq!(
        current.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?,
        i64::from(PROJECTION_DATABASE_VERSION)
    );
    let source_digest_columns = current.query_row(
        concat!(
            "SELECT count(*) FROM pragma_table_info('projection_generation') ",
            "WHERE name = 'source_ledger_digest'"
        ),
        [],
        |row| row.get::<_, i64>(0),
    )?;
    assert_eq!(source_digest_columns, 1);
    drop(current);

    let coordinates = fixture.coordinates(100);
    let receipt = runner.rebuild_at(
        ProjectionKind::Graph,
        evidence.domain_id,
        coordinates,
        &policies,
    )?;
    assert_eq!(receipt.metadata.schema_version, PROJECTION_SCHEMA_VERSION);
    assert_eq!(
        receipt.metadata.algorithm_version,
        PROJECTION_ALGORITHM_VERSION
    );
    let page = fixture.projection_reader()?.graph_neighbors(
        evidence.domain_id,
        subject,
        coordinates,
        &policies,
    )?;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Current { .. }
    ));
    assert_eq!(page.records.len(), 1);
    assert_eq!(
        canonical_snapshot(&fixture.store_reader()?)?,
        before_canonical
    );
    assert_eq!(fs::read(&evidence.vault_object_path)?, before_vault);
    Ok(())
}

#[test]
fn audited_base_v2_sidecar_is_also_replaced() -> TestResult {
    let fixture = Fixture::new("audited-base-v2")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(MIGRATION_0002_SQL)?;
    drop(connection);
    drop(open_runner(&fixture)?);
    let replacement = Connection::open(fixture.sidecar_path())?;
    assert_eq!(
        replacement.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?,
        i64::from(PROJECTION_DATABASE_VERSION)
    );
    Ok(())
}

#[test]
fn exact_v2_schema_literal_mutations_are_not_replaced() -> TestResult {
    for (fixture_name, migration) in [
        ("parent", PARENT_MIGRATION_0002_SQL),
        ("audited", MIGRATION_0002_SQL),
    ] {
        for (mutation_name, from, to) in [
            (
                "case",
                "state IN ('BUILDING', 'VERIFIED', 'FAILED')",
                "state IN ('building', 'verified', 'failed')",
            ),
            (
                "whitespace",
                "state IN ('BUILDING', 'VERIFIED', 'FAILED')",
                "state IN ('BUILDING ', 'VERIFIED', 'FAILED')",
            ),
        ] {
            let fixture = Fixture::new(&format!("v2-{fixture_name}-{mutation_name}"))?;
            let mutated = migration.replace(from, to);
            assert_ne!(mutated, migration);
            let connection = Connection::open(fixture.sidecar_path())?;
            connection.execute_batch(&mutated)?;
            drop(connection);

            let Err(error) = open_runner(&fixture) else {
                return Err(format!(
                    "{fixture_name} v2 {mutation_name} literal mutation was replaced"
                )
                .into());
            };
            assert!(matches!(
                error,
                ProjectionError::UnsupportedProjectionFormat {
                    application_id,
                    user_version: 2,
                    ..
                } if application_id == i64::from(PROJECTION_APPLICATION_ID)
            ));
        }
    }
    Ok(())
}

#[test]
fn current_v3_sqlitex_trigger_is_corrupt_and_unchanged() -> TestResult {
    let fixture = Fixture::new("current-sqlitex-trigger")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(MIGRATION_0003_SQL)?;
    connection.execute_batch(SQLITEX_TRIGGER_SQL)?;
    assert_eq!(
        named_schema_object_count(&connection, "trigger", SQLITEX_TRIGGER_NAME)?,
        1
    );
    assert_sqlitex_trigger_blocks_generation_insert(&connection)?;
    let before = sidecar_state(&connection)?;
    assert_eq!(before.user_version, i64::from(PROJECTION_DATABASE_VERSION));
    assert_eq!(before.generation_count, Some(0));
    drop(connection);

    let Err(error) = open_runner(&fixture) else {
        return Err("current v3 sidecar with a sqliteX trigger was accepted".into());
    };
    assert!(matches!(error, ProjectionError::Corrupt(reason) if reason.contains("exactly")));

    let connection = Connection::open(fixture.sidecar_path())?;
    assert_eq!(sidecar_state(&connection)?, before);
    assert_eq!(
        named_schema_object_count(&connection, "trigger", SQLITEX_TRIGGER_NAME)?,
        1
    );
    assert_sqlitex_trigger_blocks_generation_insert(&connection)?;
    Ok(())
}

#[test]
fn known_v2_shapes_with_sqlitex_trigger_are_not_replaced() -> TestResult {
    for (label, migration) in [
        ("parent", PARENT_MIGRATION_0002_SQL),
        ("audited", MIGRATION_0002_SQL),
    ] {
        let fixture = Fixture::new(&format!("v2-{label}-sqlitex-trigger"))?;
        let connection = Connection::open(fixture.sidecar_path())?;
        connection.execute_batch(migration)?;
        connection.execute_batch(SQLITEX_TRIGGER_SQL)?;
        assert_eq!(
            named_schema_object_count(&connection, "trigger", SQLITEX_TRIGGER_NAME)?,
            1
        );
        assert_sqlitex_trigger_blocks_generation_insert(&connection)?;
        let before = sidecar_state(&connection)?;
        assert_eq!(before.user_version, 2);
        assert_eq!(before.generation_count, Some(0));
        drop(connection);

        let Err(error) = open_runner(&fixture) else {
            return Err(format!("{label} v2 sidecar with a sqliteX trigger was replaced").into());
        };
        assert!(matches!(
            error,
            ProjectionError::UnsupportedProjectionFormat {
                application_id,
                user_version: 2,
                ..
            } if application_id == i64::from(PROJECTION_APPLICATION_ID)
        ));

        let connection = Connection::open(fixture.sidecar_path())?;
        assert_eq!(sidecar_state(&connection)?, before);
        assert_eq!(
            named_schema_object_count(&connection, "trigger", SQLITEX_TRIGGER_NAME)?,
            1
        );
        assert_sqlitex_trigger_blocks_generation_insert(&connection)?;
    }
    Ok(())
}

#[test]
fn version_zero_sqlitex_object_is_not_initialized() -> TestResult {
    let fixture = Fixture::new("version-zero-sqlitex-object")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(
        "CREATE TABLE sqliteXunexpected(value TEXT NOT NULL); \
         INSERT INTO sqliteXunexpected(value) VALUES ('sentinel');",
    )?;
    let before = sidecar_state(&connection)?;
    assert_eq!(before.application_id, 0);
    assert_eq!(before.user_version, 0);
    assert_eq!(before.generation_count, None);
    assert_eq!(
        named_schema_object_count(&connection, "table", "sqliteXunexpected")?,
        1
    );
    drop(connection);

    let Err(error) = open_runner(&fixture) else {
        return Err("version-zero database with a sqliteX object was initialized".into());
    };
    assert!(matches!(
        error,
        ProjectionError::UnsupportedProjectionFormat {
            application_id: 0,
            user_version: 0,
            ..
        }
    ));

    let connection = Connection::open(fixture.sidecar_path())?;
    assert_eq!(sidecar_state(&connection)?, before);
    assert_eq!(
        named_schema_object_count(&connection, "table", "sqliteXunexpected")?,
        1
    );
    assert_eq!(
        connection.query_row("SELECT value FROM sqliteXunexpected", [], |row| {
            row.get::<_, String>(0)
        })?,
        "sentinel"
    );
    Ok(())
}

#[test]
fn sqlite_owned_objects_remain_excluded_for_exact_known_schemas() -> TestResult {
    let expected = {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(MIGRATION_0003_SQL)?;
        assert!(sqlite_owned_schema_object_count(&connection)? > 0);
        sidecar_state(&connection)?
    };

    for (label, migration) in [
        ("current", MIGRATION_0003_SQL),
        ("parent-v2", PARENT_MIGRATION_0002_SQL),
        ("audited-v2", MIGRATION_0002_SQL),
    ] {
        let fixture = Fixture::new(&format!("sqlite-owned-{label}"))?;
        let connection = Connection::open(fixture.sidecar_path())?;
        connection.execute_batch(migration)?;
        assert!(sqlite_owned_schema_object_count(&connection)? > 0);
        drop(connection);

        drop(open_runner(&fixture)?);
        let connection = Connection::open(fixture.sidecar_path())?;
        assert!(sqlite_owned_schema_object_count(&connection)? > 0);
        assert_eq!(sidecar_state(&connection)?, expected);
    }
    Ok(())
}

#[test]
fn exact_previous_ranking_algorithm_is_replaced() -> TestResult {
    let fixture = Fixture::new("previous-ranking-algorithm")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(MIGRATION_0003_SQL)?;
    connection.execute_batch(concat!(
        "INSERT INTO projection_generation(",
        "generation_seq, generation_id, projection_kind, schema_version, ",
        "builder_binary_digest, algorithm_version, tokenizer_version, effective_config_hash, ",
        "known_at_accept_seq, valid_at_unix_ms, source_outbox_seq, source_ledger_digest, ",
        "resolver_version, policy_registry_version, policy_registry_hash, security_domain, ",
        "built_at_unix_ms, state, record_count, canonical_checksum, failure_reason) VALUES(",
        "1, zeroblob(16), 'fts5-unicode61-v1', 2, zeroblob(32), ",
        "'phase1-full-generation-v2', 'sqlite-fts5-unicode61-v1', zeroblob(32), ",
        "0, 0, 0, zeroblob(32), 'resolver', 'policy', zeroblob(32), zeroblob(16), ",
        "0, 'FAILED', NULL, NULL, 'old ranking algorithm');"
    ))?;
    drop(connection);

    drop(open_runner(&fixture)?);
    let replacement = Connection::open(fixture.sidecar_path())?;
    let generation_count =
        replacement.query_row("SELECT count(*) FROM projection_generation", [], |row| {
            row.get::<_, i64>(0)
        })?;
    assert_eq!(generation_count, 0);
    Ok(())
}

#[test]
fn unknown_non_projection_database_fails_with_typed_format_error() -> TestResult {
    let fixture = Fixture::new("unknown-non-projection")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch("CREATE TABLE unrelated(value INTEGER);")?;
    drop(connection);
    let Err(error) = open_runner(&fixture) else {
        return Err("non-projection database did not fail closed".into());
    };
    assert!(matches!(
        error,
        ProjectionError::UnsupportedProjectionFormat {
            application_id: 0,
            user_version: 0,
            ..
        }
    ));
    Ok(())
}

#[test]
fn unknown_projection_version_fails_with_typed_format_error() -> TestResult {
    let fixture = Fixture::new("unknown-projection-version")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(&format!(
        "PRAGMA application_id = {PROJECTION_APPLICATION_ID}; PRAGMA user_version = 999;"
    ))?;
    drop(connection);
    let Err(error) = open_runner(&fixture) else {
        return Err("unknown projection version did not fail closed".into());
    };
    assert!(matches!(
        error,
        ProjectionError::UnsupportedProjectionFormat {
            application_id,
            user_version: 999,
            ..
        } if application_id == i64::from(PROJECTION_APPLICATION_ID)
    ));
    Ok(())
}

#[test]
fn unknown_v2_shape_is_not_mistaken_for_the_exact_parent() -> TestResult {
    let fixture = Fixture::new("unknown-v2-shape")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(&format!(
        concat!(
            "PRAGMA application_id = {}; PRAGMA user_version = 2; ",
            "CREATE TABLE projection_generation(generation_seq INTEGER);"
        ),
        PROJECTION_APPLICATION_ID
    ))?;
    drop(connection);
    let Err(error) = open_runner(&fixture) else {
        return Err("unknown v2 projection shape did not fail closed".into());
    };
    assert!(matches!(
        error,
        ProjectionError::UnsupportedProjectionFormat {
            application_id,
            user_version: 2,
            ..
        } if application_id == i64::from(PROJECTION_APPLICATION_ID)
    ));
    Ok(())
}

#[test]
fn current_identity_with_missing_columns_is_rejected() -> TestResult {
    let fixture = Fixture::new("current-missing-columns")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(&format!(
        concat!(
            "PRAGMA application_id = {}; PRAGMA user_version = {}; ",
            "CREATE TABLE projection_generation(generation_seq INTEGER);"
        ),
        PROJECTION_APPLICATION_ID, PROJECTION_DATABASE_VERSION
    ))?;
    drop(connection);
    let Err(error) = open_runner(&fixture) else {
        return Err("missing version-3 columns did not fail closed".into());
    };
    assert!(matches!(error, ProjectionError::Corrupt(reason) if reason.contains("schema")));
    Ok(())
}

#[test]
fn current_v3_with_missing_index_is_rejected_by_exact_fingerprint() -> TestResult {
    let fixture = Fixture::new("current-missing-index")?;
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(MIGRATION_0003_SQL)?;
    connection.execute("DROP INDEX idx_projection_generation_authority", [])?;
    drop(connection);
    let Err(error) = open_runner(&fixture) else {
        return Err("v3 sidecar with a missing required index did not fail closed".into());
    };
    assert!(matches!(error, ProjectionError::Corrupt(reason) if reason.contains("exactly")));
    Ok(())
}

#[test]
fn current_v3_schema_literal_case_mutation_is_corrupt() -> TestResult {
    assert_current_v3_literal_mutation_is_corrupt(
        "current-literal-case",
        "state IN ('BUILDING', 'VERIFIED', 'FAILED')",
        "state IN ('building', 'verified', 'failed')",
    )
}

#[test]
fn current_v3_schema_literal_whitespace_mutation_is_corrupt() -> TestResult {
    assert_current_v3_literal_mutation_is_corrupt(
        "current-literal-whitespace",
        "state IN ('BUILDING', 'VERIFIED', 'FAILED')",
        "state IN ('BUILDING ', 'VERIFIED', 'FAILED')",
    )
}

fn assert_current_v3_literal_mutation_is_corrupt(label: &str, from: &str, to: &str) -> TestResult {
    let fixture = Fixture::new(label)?;
    let mutated = MIGRATION_0003_SQL.replace(from, to);
    assert_ne!(mutated, MIGRATION_0003_SQL);
    let connection = Connection::open(fixture.sidecar_path())?;
    connection.execute_batch(&mutated)?;
    drop(connection);
    let Err(error) = open_runner(&fixture) else {
        return Err(format!("current v3 literal mutation {label} was accepted").into());
    };
    assert!(matches!(error, ProjectionError::Corrupt(reason) if reason.contains("exactly")));
    Ok(())
}

fn sidecar_state(connection: &Connection) -> TestResult<SidecarState> {
    let application_id = connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    let user_version = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let mut statement = connection
        .prepare("SELECT type, name, coalesce(sql, '') FROM sqlite_schema ORDER BY type, name")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let schema = rows.collect::<Result<Vec<_>, _>>()?;
    let generation_count =
        if named_schema_object_count(connection, "table", "projection_generation")? == 1 {
            Some(
                connection.query_row("SELECT count(*) FROM projection_generation", [], |row| {
                    row.get(0)
                })?,
            )
        } else {
            None
        };
    Ok(SidecarState {
        application_id,
        user_version,
        schema,
        generation_count,
    })
}

fn named_schema_object_count(
    connection: &Connection,
    object_type: &str,
    name: &str,
) -> TestResult<i64> {
    Ok(connection.query_row(
        concat!(
            "SELECT count(*) FROM sqlite_schema ",
            "WHERE type = ?1 AND name = ?2 COLLATE BINARY"
        ),
        [object_type, name],
        |row| row.get(0),
    )?)
}

fn sqlite_owned_schema_object_count(connection: &Connection) -> TestResult<i64> {
    Ok(connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name GLOB 'sqlite_*'",
        [],
        |row| row.get(0),
    )?)
}

fn assert_sqlitex_trigger_blocks_generation_insert(connection: &Connection) -> TestResult {
    let Err(error) = connection.execute("INSERT INTO projection_generation DEFAULT VALUES", [])
    else {
        return Err("sqliteX trigger did not block a generation insert".into());
    };
    assert!(error.to_string().contains("blocked by sqliteX trigger"));
    assert_eq!(
        connection.query_row("SELECT count(*) FROM projection_generation", [], |row| {
            row.get::<_, i64>(0)
        })?,
        0
    );
    Ok(())
}

fn open_runner(fixture: &Fixture) -> Result<ProjectionRunner, ProjectionError> {
    let reader = academic_store::connection::open_reader(fixture.canonical_path())?;
    ProjectionRunner::open(
        &reader,
        fixture.sidecar_path(),
        ContentDigest::sha256(b"projection-format-test-builder"),
        ContentDigest::sha256(b"projection-format-test-config"),
    )
}
