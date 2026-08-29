//! `academic fixture` — deterministic committed-fixture workflows.
//!
//! These commands operate on the repository's own deterministic builder output.
//! `emit` writes the current v2 bytes; there is no caller-selectable legacy
//! writer, so no public path can mint a v1 envelope.

use std::{fs, path::Path};

use academic_core::{
    FINAL_VALID_AT, FixtureDocument, build_fixture_document, fixture_json,
    parse_fixture_document_json, replay_fixture_document, verify_fixture_document,
};
use serde_json::json;

use crate::{
    commands::display,
    output::{CliFailure, CommandResult, ExitClass},
};

fn read_fixture(path: &Path) -> Result<FixtureDocument, CliFailure> {
    let bytes = fs::read(path).map_err(|error| {
        CliFailure::new(
            ExitClass::Internal,
            "FIXTURE_UNREADABLE",
            format!("{}: {error}", path.display()),
        )
    })?;
    parse_fixture_document_json(&bytes).map_err(|error| {
        CliFailure::new(
            ExitClass::Incompatible,
            "FIXTURE_UNPARSEABLE",
            format!("{}: {error}", path.display()),
        )
    })
}

/// Emits the deterministic v3 fixture to standard output or an explicit path.
pub fn emit(output: Option<&Path>) -> CommandResult {
    let document = build_fixture_document()
        .map_err(|error| CliFailure::internal("FIXTURE_BUILD_FAILED", error))?;
    let rendered = fixture_json(&document)
        .map_err(|error| CliFailure::internal("FIXTURE_ENCODE_FAILED", error))?;
    match output {
        Some(path) => {
            fs::write(path, &rendered).map_err(|error| {
                CliFailure::new(
                    ExitClass::Internal,
                    "FIXTURE_WRITE_FAILED",
                    format!("{}: {error}", path.display()),
                )
            })?;
            Ok(json!({
                "written_to": display(path),
                "byte_length": rendered.len(),
                "name": document.name,
            }))
        }
        None => Ok(json!({
            "document": serde_json::from_str::<serde_json::Value>(&rendered)
                .map_err(|error| CliFailure::internal("FIXTURE_ENCODE_FAILED", error))?,
            "byte_length": rendered.len(),
            "name": document.name,
        })),
    }
}

/// Verifies exact bytes, signature, expected replay, and builder drift.
pub fn verify(path: &Path) -> CommandResult {
    let document = read_fixture(path)?;
    let replay = verify_fixture_document(&document).map_err(|error| {
        CliFailure::new(
            ExitClass::PolicyDenied,
            "FIXTURE_VERIFICATION_FAILED",
            error.to_string(),
        )
    })?;
    serde_json::to_value(&replay)
        .map_err(|error| CliFailure::internal("REPLAY_ENCODE_FAILED", error))
}

/// Verifies the signature and replays accepted events from sequence zero.
pub fn replay(path: &Path) -> CommandResult {
    let document = read_fixture(path)?;
    let replay = replay_fixture_document(&document, FINAL_VALID_AT, u64::MAX).map_err(|error| {
        CliFailure::new(
            ExitClass::PolicyDenied,
            "FIXTURE_REPLAY_FAILED",
            error.to_string(),
        )
    })?;
    serde_json::to_value(&replay)
        .map_err(|error| CliFailure::internal("REPLAY_ENCODE_FAILED", error))
}

/// Renders the human lines for a fixture command.
pub fn lines(value: &serde_json::Value) -> Vec<String> {
    match serde_json::to_string_pretty(value) {
        Ok(rendered) => rendered.lines().map(str::to_owned).collect(),
        Err(_) => vec!["<fixture result could not be rendered>".to_owned()],
    }
}
