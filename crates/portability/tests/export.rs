//! Deterministic open-directory export behaviour.

#![cfg(feature = "plaintext-portability")]

mod support;

use std::{collections::BTreeSet, fs};

use academic_portability::{
    MAX_PORTABLE_RELATIVE_PATH_BYTES,
    export::{
        CANONICAL_FILES, INVENTORY_FILE, LEDGER_BATCH_DIRECTORY, LEDGER_EVENTS_FILE, MANIFEST_FILE,
        MANIFEST_SCHEMA_FILE, STORE_SCHEMA_FILE, export_profile, read_exported_artifacts,
        verify_export_directory,
    },
    restore::PROJECTION_SIDECAR_FILE,
    verify::{CanonicalDatabase, read_canonical_rows},
};
use rusqlite::Connection;
use support::{Fixture, TestResult};

/// Names Windows refuses as a path component in any directory, with or without
/// an extension.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[test]
fn two_exports_have_same_semantic_digest() -> TestResult {
    let fixture = Fixture::new("export-determinism")?;
    let first = export_profile(
        fixture.profile_root(),
        &fixture.work_path("export-a"),
        fixture.keyring()?,
    )?;
    let second = export_profile(
        fixture.profile_root(),
        &fixture.work_path("export-b"),
        fixture.keyring()?,
    )?;

    assert_eq!(
        first.manifest.semantic_digest, second.manifest.semantic_digest,
        "two exports at the same watermark disagreed on the semantic digest"
    );
    assert_eq!(first.manifest.semantic, second.manifest.semantic);
    assert_eq!(
        first.manifest.semantic.files,
        second.manifest.semantic.files
    );
    assert!(!first.manifest.semantic.files.is_empty());

    for entry in &first.manifest.semantic.files {
        let left = fs::read(fixture.work_path("export-a").join(&entry.path))?;
        let right = fs::read(fixture.work_path("export-b").join(&entry.path))?;
        assert_eq!(
            left, right,
            "exported file {} was not byte-identical",
            entry.path
        );
    }

    verify_export_directory(&fixture.work_path("export-a"))?;
    verify_export_directory(&fixture.work_path("export-b"))?;

    let first_manifest = fs::read(fixture.work_path("export-a").join(MANIFEST_FILE))?;
    let second_manifest = fs::read(fixture.work_path("export-b").join(MANIFEST_FILE))?;
    assert!(
        first.manifest.volatile.generated_at_unix_ms >= 0,
        "the volatile generation instant must be representable"
    );
    let strip_volatile = |bytes: &[u8]| -> TestResult<serde_json::Value> {
        let mut value: serde_json::Value = serde_json::from_slice(bytes)?;
        value
            .as_object_mut()
            .ok_or("manifest is not a JSON object")?
            .remove("volatile");
        Ok(value)
    };
    assert_eq!(
        strip_volatile(&first_manifest)?,
        strip_volatile(&second_manifest)?,
        "only the volatile generation instant may differ between exports"
    );

    // Host-independent evidence. Running this test with `--nocapture` on two
    // hosts and diffing these lines proves the cross-platform claim with the
    // same bytes the manifest commits to.
    println!(
        "EXPORT-DETERMINISM semantic_digest {}",
        first.manifest.semantic_digest
    );
    println!(
        "EXPORT-DETERMINISM canonical_semantic_digest {}",
        first.manifest.semantic.canonical_semantic_digest
    );
    for entry in &first.manifest.semantic.files {
        println!(
            "EXPORT-DETERMINISM file {} {} {}",
            entry.path, entry.byte_length, entry.sha256
        );
    }
    Ok(())
}

#[test]
fn export_preserves_signed_envelope_bytes() -> TestResult {
    let fixture = Fixture::new("export-envelopes")?;
    let destination = fixture.work_path("export");
    let receipt = export_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    assert!(!receipt.manifest.semantic.batches.is_empty());

    let connection = rusqlite::Connection::open_with_flags(
        fixture.database_path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    for batch in &receipt.manifest.semantic.batches {
        let stored: Vec<u8> = connection.query_row(
            "SELECT signed_envelope FROM ledger_batch WHERE hex(batch_id) = upper(?1)",
            [batch.batch_id.replace('-', "")],
            |row| row.get(0),
        )?;
        let exported = fs::read(
            destination
                .join(LEDGER_BATCH_DIRECTORY)
                .join(format!("{}.cbor", batch.batch_id)),
        )?;
        assert_eq!(
            exported, stored,
            "exported envelope for {} was not byte-for-byte",
            batch.batch_id
        );
        assert_eq!(
            u64::try_from(exported.len())?,
            batch.envelope_byte_length,
            "manifest envelope length disagreed with the exported bytes"
        );
    }
    Ok(())
}

#[test]
fn export_excludes_projection_truth() -> TestResult {
    let fixture = Fixture::new("export-projections")?;
    let source_checksums = fixture.source_projection_checksums()?;
    assert_eq!(source_checksums.len(), 3);
    let sidecar = fixture.profile_root().join(PROJECTION_SIDECAR_FILE);
    assert!(
        sidecar.is_file(),
        "the fixture must own a disposable projection sidecar for this test to mean anything"
    );

    let destination = fixture.work_path("export");
    let receipt = export_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    assert!(!receipt.manifest.semantic.projections_included);

    let paths: BTreeSet<&str> = receipt
        .manifest
        .semantic
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert!(!paths.contains(PROJECTION_SIDECAR_FILE));
    for path in &paths {
        assert!(
            !path.contains("projection"),
            "export contained a projection artefact at {path}"
        );
    }
    for path in &paths {
        let bytes = fs::read(destination.join(path))?;
        let text = String::from_utf8_lossy(&bytes);
        for forbidden in [
            "projection_generation",
            "projection_graph_edge",
            "projection_search_content",
            "projection_active",
        ] {
            assert!(
                !text.contains(forbidden),
                "export file {path} leaked projection table {forbidden}"
            );
        }
    }
    Ok(())
}

#[test]
fn export_layout_matches_the_phase1_contract() -> TestResult {
    let fixture = Fixture::new("export-layout")?;
    let destination = fixture.work_path("export");
    let receipt = export_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;

    let paths: BTreeSet<&str> = receipt
        .manifest
        .semantic
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    for required in [
        INVENTORY_FILE,
        MANIFEST_SCHEMA_FILE,
        STORE_SCHEMA_FILE,
        LEDGER_EVENTS_FILE,
    ] {
        assert!(paths.contains(required), "export omitted {required}");
    }
    for required in CANONICAL_FILES {
        assert!(paths.contains(required), "export omitted {required}");
    }
    assert!(!paths.contains(MANIFEST_FILE), "the manifest lists itself");
    assert!(
        receipt
            .manifest
            .semantic
            .files
            .windows(2)
            .all(|window| window[0].path < window[1].path),
        "exported files are not strictly sorted by canonical path"
    );
    assert!(
        paths.iter().any(|path| path.starts_with("objects/")),
        "export omitted the sealed object namespace"
    );

    let artifacts = read_exported_artifacts(&destination)?;
    assert_eq!(
        u64::try_from(artifacts.len())?,
        receipt.manifest.semantic.counts.artifacts
    );
    assert!(
        artifacts
            .windows(2)
            .all(|window| window[0].artifact_id < window[1].artifact_id),
        "canonical rows are not sorted by canonical identifier"
    );

    let events = fs::read_to_string(destination.join(LEDGER_EVENTS_FILE))?;
    assert!(events.ends_with('\n'));
    assert!(!events.contains('\r'), "exported records must use LF only");
    assert_eq!(
        u64::try_from(events.lines().count())?,
        receipt.manifest.semantic.counts.events
    );
    Ok(())
}

#[test]
fn export_paths_are_portable_on_windows_and_linux() -> TestResult {
    let fixture = Fixture::new("export-paths")?;
    let destination = fixture.work_path("export");
    let receipt = export_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;

    for entry in &receipt.manifest.semantic.files {
        assert!(
            !entry.path.contains('\\'),
            "path {} is not portable",
            entry.path
        );
        assert!(!entry.path.starts_with('/'));
        for component in entry.path.split('/') {
            assert!(!component.is_empty());
            assert!(!component.ends_with('.') && !component.ends_with(' '));
            let stem = component.split('.').next().unwrap_or(component);
            assert!(
                !WINDOWS_RESERVED_NAMES
                    .iter()
                    .any(|reserved| reserved.eq_ignore_ascii_case(stem)),
                "path component {component} is a Windows reserved device name"
            );
            for forbidden in ['<', '>', ':', '"', '|', '?', '*'] {
                assert!(
                    !component.contains(forbidden),
                    "path component {component} contains a Windows-invalid character"
                );
            }
        }
        assert!(
            entry.path.len() <= MAX_PORTABLE_RELATIVE_PATH_BYTES,
            "produced relative path {} exceeds the portable format budget",
            entry.path
        );
    }
    Ok(())
}

#[test]
fn export_refuses_an_existing_destination() -> TestResult {
    let fixture = Fixture::new("export-existing")?;
    let destination = fixture.work_path("export");
    export_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    let repeated = export_profile(fixture.profile_root(), &destination, fixture.keyring()?);
    assert!(
        repeated.is_err(),
        "export overwrote an existing destination directory"
    );
    verify_export_directory(&destination)?;
    Ok(())
}

#[test]
fn export_verification_rejects_a_tampered_record() -> TestResult {
    let fixture = Fixture::new("export-tamper")?;
    let destination = fixture.work_path("export");
    export_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    verify_export_directory(&destination)?;

    let events = destination.join(LEDGER_EVENTS_FILE);
    let mut bytes = fs::read(&events)?;
    bytes.extend_from_slice(b"{}\n");
    fs::write(&events, &bytes)?;
    assert!(
        verify_export_directory(&destination).is_err(),
        "a tampered canonical record survived export verification"
    );
    Ok(())
}

/// A canonical read set must describe exactly one commit.
///
/// SQLite gives every autocommit statement its own read snapshot, so a
/// sequence of `SELECT`s against a live profile can straddle a writer's commit
/// and yield a manifest whose watermark, counts, and rows describe different
/// states. Export has nothing to compare its read set against — the manifest is
/// written from the first read and never re-checked — so the property has to
/// hold at the read boundary itself.
///
/// The interleaving cannot be driven through a product command: no Phase 1
/// command can perform a second acceptance while an export reads, which is why
/// the concurrent writer here is a direct connection. What is asserted is the
/// boundary that owns the property: one reader, two read sets, a commit
/// between them.
#[test]
fn a_canonical_read_set_does_not_straddle_a_concurrent_commit() -> TestResult {
    let fixture = Fixture::new("export-read-snapshot")?;

    // The writer opens and touches the database first so the WAL index exists
    // before the read-only handle attaches to it; a reader that attached to a
    // checkpointed database with no index would not be reading the same file
    // the writer is about to append to.
    let writer = Connection::open(fixture.database_path())?;
    let _: i64 = writer.query_row("SELECT count(*) FROM replica_state", [], |row| row.get(0))?;

    let database = CanonicalDatabase::open_source(fixture.database_path())?;
    let snapshot = database.begin_read()?;
    let first = read_canonical_rows(&database)?;
    writer.execute(
        "UPDATE replica_state SET profile_revision = profile_revision + 1 WHERE singleton = 1",
        [],
    )?;
    let second = read_canonical_rows(&database)?;
    assert_eq!(
        first.watermark, second.watermark,
        "a read inside the snapshot observed a commit that landed after it opened"
    );
    drop(snapshot);

    // Without the snapshot the same reader sees the commit, which is what makes
    // the assertion above evidence rather than a statement about a writer that
    // never wrote.
    let third = read_canonical_rows(&database)?;
    assert_eq!(
        third.watermark.profile_revision,
        first.watermark.profile_revision + 1,
        "the concurrent commit was never observable, so the snapshot proved nothing"
    );
    Ok(())
}
