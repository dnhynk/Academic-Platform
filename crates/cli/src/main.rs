//! Headless Phase 0 CLI for doctor and deterministic fixture workflows.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use academic_core::{
    FINAL_VALID_AT, FixtureDocument, build_fixture_document, fixture_json, replay_fixture_document,
    verify_fixture_document,
};
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

const EXPECTED_RUSTC: &str = "rustc 1.98.0";
const EXPECTED_CARGO: &str = "cargo 1.98.0";
const EXPECTED_NODE: &str = "v24.19.0";
const EXPECTED_PNPM: &str = "11.22.0";

#[derive(Debug, Parser)]
#[command(
    name = "academic",
    version,
    about = "Academic OS Phase 0 invariant CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Checks pinned local development prerequisites without network access.
    Doctor {
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Encodes, verifies, and replays the deterministic signed fixture.
    Fixture {
        #[command(subcommand)]
        command: FixtureCommand,
    },
}

#[derive(Debug, Subcommand)]
enum FixtureCommand {
    /// Emits the current v2 deterministic fixture JSON to stdout or an explicit path.
    Emit {
        /// Writes the deterministic bytes directly to this file when supplied.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Verifies exact bytes, signature, expected replay, and builder drift.
    Verify { path: PathBuf },
    /// Verifies the signature and replays accepted events from sequence zero.
    Replay { path: PathBuf },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    ready: bool,
    phase: &'static str,
    data_policy: &'static str,
    network_egress: &'static str,
    checks: Vec<ToolCheck>,
}

#[derive(Debug, Serialize)]
struct ToolCheck {
    tool: &'static str,
    expected: &'static str,
    observed: Option<String>,
    resolved_path: Option<PathBuf>,
    supported: bool,
    remediation: &'static str,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Doctor { format } => doctor(format),
        Commands::Fixture { command } => fixture(command),
    }
}

fn fixture(command: FixtureCommand) -> Result<()> {
    match command {
        FixtureCommand::Emit { output } => {
            let document = build_fixture_document()?;
            let json = fixture_json(&document)?;
            if let Some(path) = output {
                fs::write(&path, json)
                    .with_context(|| format!("failed to write fixture {}", path.display()))?;
            } else {
                print!("{json}");
            }
        }
        FixtureCommand::Verify { path } => {
            let document = read_fixture(&path)?;
            let replay = verify_fixture_document(&document)?;
            println!("{}", serde_json::to_string_pretty(&replay)?);
        }
        FixtureCommand::Replay { path } => {
            let document = read_fixture(&path)?;
            let replay = replay_fixture_document(&document, FINAL_VALID_AT, u64::MAX)?;
            println!("{}", serde_json::to_string_pretty(&replay)?);
        }
    }
    Ok(())
}

fn read_fixture(path: &Path) -> Result<FixtureDocument> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read fixture {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse fixture {}", path.display()))
}

fn doctor(format: OutputFormat) -> Result<()> {
    let checks = vec![
        check_tool(
            "rustc",
            EXPECTED_RUSTC,
            &["--version"],
            "Install rustup, then run: rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy",
        ),
        check_tool(
            "cargo",
            EXPECTED_CARGO,
            &["--version"],
            "Restart the shell after rustup, or add %USERPROFILE%\\.cargo\\bin to PATH.",
        ),
        check_tool(
            "node",
            EXPECTED_NODE,
            &["--version"],
            "Use Node 24.19.0; with nvm-windows run: nvm use 24.19.0",
        ),
        check_tool(
            "pnpm",
            EXPECTED_PNPM,
            &["--version"],
            "Install the pinned package manager: npm install --global pnpm@11.22.0",
        ),
    ];
    let report = DoctorReport {
        ready: checks.iter().all(|check| check.supported),
        phase: "PHASE_0_EXECUTABLE_INVARIANTS",
        data_policy: "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
        network_egress: "PRODUCT_RUNTIME_NONE",
        checks,
    };
    match format {
        OutputFormat::Human => {
            println!("Academic OS Phase 0 doctor");
            println!("data policy: {}", report.data_policy);
            println!("runtime egress: {}", report.network_egress);
            for check in &report.checks {
                let state = if check.supported { "ok" } else { "unsupported" };
                println!(
                    "- {}: {} ({}, expected {})",
                    check.tool,
                    check.observed.as_deref().unwrap_or("missing"),
                    state,
                    check.expected
                );
                if !check.supported {
                    println!("  remediation: {}", check.remediation);
                }
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }
    if !report.ready {
        bail!("developer prerequisites do not match repository pins");
    }
    Ok(())
}

fn check_tool(
    tool: &'static str,
    expected: &'static str,
    arguments: &[&str],
    remediation: &'static str,
) -> ToolCheck {
    let resolved_path = resolve_executable(tool);
    let observed = Command::new(tool)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned());
    let supported = observed
        .as_deref()
        .is_some_and(|value| value.starts_with(expected));
    ToolCheck {
        tool,
        expected,
        observed,
        resolved_path,
        supported,
        remediation,
    }
}

fn resolve_executable(tool: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let extensions: Vec<String> = if cfg!(windows) {
        env::var_os("PATHEXT")
            .and_then(|value| value.into_string().ok())
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_owned())
            .split(';')
            .map(str::to_ascii_lowercase)
            .collect()
    } else {
        vec![String::new()]
    };
    for directory in env::split_paths(&path) {
        for extension in &extensions {
            let candidate = if extension.is_empty() {
                directory.join(tool)
            } else {
                directory.join(format!("{tool}{extension}"))
            };
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t010_fixture_emit_is_v2_only_and_rejects_version_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        let parsed = Cli::try_parse_from(["academic", "fixture", "emit"])?;
        assert!(matches!(
            parsed.command,
            Commands::Fixture {
                command: FixtureCommand::Emit { output: None }
            }
        ));
        assert!(
            Cli::try_parse_from(["academic", "fixture", "emit", "--fixture-version", "1",])
                .is_err(),
            "the CLI must not expose a caller-selectable legacy writer"
        );
        Ok(())
    }
}
