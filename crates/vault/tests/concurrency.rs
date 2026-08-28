#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

use academic_domain::RetentionClass;
use academic_vault::{SealDisposition, Vault};
use synthetic_artifacts::{DOMAIN_ID, PERMISSION_LINEAGE_ID, open_test_vault, request_with};

const THREADS: usize = 8;
const ROUNDS: usize = 10;
const DIRECTORY_BARRIER_FILE: &str = ".academic-vault-directory-barrier";

#[test]
fn concurrent_distinct_ingest_leaves_no_error_or_stale_partial() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("concurrent-distinct-ingest")?;
    let vault = Arc::new(vault);
    let outcomes = ingest_in_parallel(&vault, |thread_index, round| thread_index * ROUNDS + round)?;

    let (published, failures) = split_outcomes(outcomes);
    assert!(
        failures.is_empty(),
        "{} of {} concurrent ingests failed: {failures:#?}",
        failures.len(),
        THREADS * ROUNDS
    );
    assert_eq!(published.len(), THREADS * ROUNDS);

    let objects = collect_suffixed(vault.layout().objects_root(), ".obj")?;
    assert_eq!(
        objects.len(),
        published.len(),
        "every durably published object must be covered by exactly one receipt"
    );
    let partials = collect_suffixed(vault.layout().temp_dir(), ".partial")?;
    assert!(
        partials.is_empty(),
        "concurrent ingest left {} stale partial files: {partials:#?}",
        partials.len()
    );
    Ok(())
}

#[test]
fn concurrent_identical_ingest_adopts_exactly_one_object() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("concurrent-identical-ingest")?;
    let vault = Arc::new(vault);
    let outcomes = ingest_in_parallel(&vault, |_thread_index, _round| 0)?;

    let (published, failures) = split_outcomes(outcomes);
    assert!(
        failures.is_empty(),
        "{} of {} concurrent identical ingests failed: {failures:#?}",
        failures.len(),
        THREADS * ROUNDS
    );
    assert_eq!(published.len(), THREADS * ROUNDS);
    let new_publications = published
        .iter()
        .filter(|(_, disposition)| *disposition == SealDisposition::PublishedNew)
        .count();
    assert_eq!(new_publications, 1);

    let objects = collect_suffixed(vault.layout().objects_root(), ".obj")?;
    assert_eq!(objects.len(), 1);
    let partials = collect_suffixed(vault.layout().temp_dir(), ".partial")?;
    assert!(
        partials.is_empty(),
        "concurrent identical ingest left {} stale partial files: {partials:#?}",
        partials.len()
    );
    Ok(())
}

type Outcome = Result<(PathBuf, SealDisposition), String>;

fn ingest_in_parallel(
    vault: &Arc<Vault>,
    index_of: fn(usize, usize) -> usize,
) -> Result<Vec<Outcome>, Box<dyn Error>> {
    let mut handles = Vec::with_capacity(THREADS);
    for thread_index in 0..THREADS {
        let vault = Arc::clone(vault);
        handles.push(thread::spawn(move || {
            (0..ROUNDS)
                .map(|round| {
                    ingest_one(&vault, index_of(thread_index, round))
                        .map_err(|error| error.to_string())
                })
                .collect::<Vec<Outcome>>()
        }));
    }
    let mut outcomes = Vec::with_capacity(THREADS * ROUNDS);
    for handle in handles {
        outcomes.extend(handle.join().map_err(|_| "an ingest thread panicked")?);
    }
    Ok(outcomes)
}

fn ingest_one(vault: &Vault, index: usize) -> Result<(PathBuf, SealDisposition), Box<dyn Error>> {
    let request = request_with(
        &format!("01900000-0000-7000-8000-{index:012x}"),
        DOMAIN_ID,
        RetentionClass::UserManaged,
        PERMISSION_LINEAGE_ID,
    )?;
    let bytes = format!("synthetic concurrent artifact {index}\n").into_bytes();
    let receipt = vault.ingest(&request, bytes.as_slice())?;
    Ok((receipt.object_path().to_path_buf(), receipt.disposition()))
}

fn split_outcomes(outcomes: Vec<Outcome>) -> (Vec<(PathBuf, SealDisposition)>, Vec<String>) {
    let mut published = Vec::new();
    let mut failures = Vec::new();
    for outcome in outcomes {
        match outcome {
            Ok(value) => published.push(value),
            Err(message) => failures.push(message),
        }
    }
    (published, failures)
}

fn collect_suffixed(directory: &Path, suffix: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(directory, suffix, &mut found)?;
    found.sort();
    Ok(found)
}

fn walk(directory: &Path, suffix: &str, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name() == DIRECTORY_BARRIER_FILE {
            continue;
        }
        let path = entry.path();
        if fs::symlink_metadata(&path)?.file_type().is_dir() {
            walk(&path, suffix, found)?;
        } else if path.to_string_lossy().ends_with(suffix) {
            found.push(path);
        }
    }
    Ok(())
}
