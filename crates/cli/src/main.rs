//! Headless Phase 0 CLI for doctor and deterministic fixture workflows.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use academic_core::{
    FINAL_VALID_AT, FixtureDocument, build_fixture_document, fixture_json,
    parse_fixture_document_json, replay_fixture_document, verify_fixture_document,
};
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

const TOOL_VERSION_CORPUS_JSON: &str =
    include_str!("../../../tools/fixtures/tool-version-conformance-v1.json");

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
    tool: String,
    expected: String,
    observed: Option<String>,
    resolved_path: Option<PathBuf>,
    supported: bool,
    remediation: String,
}

#[derive(Debug, Deserialize)]
struct ToolVersionCorpus {
    schema_version: u8,
    tools: Vec<ToolVersionSpecification>,
}

#[derive(Debug, Deserialize)]
struct ToolVersionSpecification {
    name: String,
    expected: String,
    policy: ToolVersionPolicy,
    remediation: String,
    cases: Vec<ToolVersionCase>,
}

#[derive(Debug, Deserialize)]
struct ToolVersionCase {
    name: String,
    output: String,
    supported: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ToolVersionPolicy {
    Exact,
    StableRustTool,
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
    parse_fixture_document_json(&bytes)
        .with_context(|| format!("failed to parse fixture {}", path.display()))
}

fn doctor(format: OutputFormat) -> Result<()> {
    let corpus: ToolVersionCorpus = serde_json::from_str(TOOL_VERSION_CORPUS_JSON)?;
    validate_tool_version_corpus(&corpus)?;
    let checks = corpus.tools.iter().map(check_tool).collect::<Vec<_>>();
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

fn check_tool(specification: &ToolVersionSpecification) -> ToolCheck {
    let resolved_path = resolve_executable(&specification.name);
    let observed = Command::new(&specification.name)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned());
    let supported = observed
        .as_deref()
        .is_some_and(|value| is_supported_tool_version(specification, value));
    ToolCheck {
        tool: specification.name.clone(),
        expected: specification.expected.clone(),
        observed,
        resolved_path,
        supported,
        remediation: specification.remediation.clone(),
    }
}

fn is_supported_tool_version(specification: &ToolVersionSpecification, output: &str) -> bool {
    let observed = output.trim();
    match specification.policy {
        ToolVersionPolicy::Exact => observed == specification.expected,
        ToolVersionPolicy::StableRustTool => {
            if observed == specification.expected {
                return true;
            }
            observed
                .strip_prefix(&format!("{} ", specification.expected))
                .is_some_and(has_ordinary_stable_build_metadata)
        }
    }
}

fn has_ordinary_stable_build_metadata(value: &str) -> bool {
    let Some(metadata) = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return false;
    };
    let mut fields = metadata.split(' ');
    let Some(commit) = fields.next() else {
        return false;
    };
    let Some(date) = fields.next() else {
        return false;
    };
    fields.next().is_none()
        && (9..=40).contains(&commit.len())
        && commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        && date.len() == 10
        && date.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7) && byte == b'-'
                || !matches!(index, 4 | 7) && byte.is_ascii_digit()
        })
}

fn validate_tool_version_corpus(corpus: &ToolVersionCorpus) -> Result<()> {
    if corpus.schema_version != 1
        || corpus
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .ne(["rustc", "cargo", "node", "pnpm"])
    {
        bail!("tool-version conformance corpus has an unsupported shape");
    }
    for tool in &corpus.tools {
        if tool.expected.is_empty()
            || tool.remediation.is_empty()
            || !tool.cases.iter().any(|test_case| test_case.supported)
            || !tool.cases.iter().any(|test_case| !test_case.supported)
        {
            bail!(
                "tool-version conformance corpus is incomplete for {}",
                tool.name
            );
        }
        for test_case in &tool.cases {
            if is_supported_tool_version(tool, &test_case.output) != test_case.supported {
                bail!(
                    "tool-version conformance disagreement for {}: {}",
                    tool.name,
                    test_case.name
                );
            }
        }
    }
    Ok(())
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

    #[test]
    fn t017_doctor_and_bootstrap_share_token_exact_version_conformance()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus: ToolVersionCorpus = serde_json::from_str(TOOL_VERSION_CORPUS_JSON)?;
        validate_tool_version_corpus(&corpus)?;
        for tool in &corpus.tools {
            for test_case in &tool.cases {
                assert_eq!(
                    is_supported_tool_version(tool, &test_case.output),
                    test_case.supported,
                    "{}: {}",
                    tool.name,
                    test_case.name
                );
            }
        }
        Ok(())
    }
}
