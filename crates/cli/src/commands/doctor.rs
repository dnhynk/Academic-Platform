//! `academic doctor` — pinned toolchain checks plus optional profile health.
//!
//! Without `--profile` this is the Phase 0 developer-prerequisite check.
//! With `--profile` it adds the physical store identity and canonical
//! watermarks; `--deep` adds `integrity_check`, `foreign_key_check`,
//! unpublished vault temp entries, quarantine disposition, and projection lag
//! against the canonical outbox head.

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use academic_core::operations::{ProfileDiagnosis, diagnose_profile};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    commands::{classify, display},
    output::{CliFailure, CommandResult, ExitClass},
};

const TOOL_VERSION_CORPUS_JSON: &str =
    include_str!("../../../../tools/fixtures/tool-version-conformance-v1.json");

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

/// Runs the doctor and returns its structured report.
pub fn run(profile_root: Option<&Path>, deep: bool) -> CommandResult {
    let corpus: ToolVersionCorpus = serde_json::from_str(TOOL_VERSION_CORPUS_JSON)
        .map_err(|error| CliFailure::internal("TOOL_CORPUS_UNREADABLE", error))?;
    validate_tool_version_corpus(&corpus)?;
    let checks = corpus.tools.iter().map(check_tool).collect::<Vec<_>>();
    let toolchain_ready = checks.iter().all(|check| check.supported);

    let profile = match profile_root {
        Some(root) => Some(
            diagnose_profile(root, deep)
                .map_err(|error| classify("PROFILE_READ_FAILED", &error))?,
        ),
        None => None,
    };

    let repair_required = profile
        .as_ref()
        .is_some_and(ProfileDiagnosis::repair_required);
    let ready = toolchain_ready && !repair_required;

    let report = json!({
        "ready": ready,
        "phase": "PHASE_1_SYNTHETIC_LOCAL_CORE",
        "deep": deep,
        "toolchain_ready": toolchain_ready,
        "network_egress": "PRODUCT_RUNTIME_NONE",
        "checks": checks,
        "profile_root": profile_root.map(display),
        "profile": profile,
    });

    if repair_required {
        return Err(CliFailure::new(
            ExitClass::RepairRequired,
            "PROFILE_REPAIR_REQUIRED",
            "deep doctor found a condition that must be repaired before the profile is served",
        )
        .with_result(report));
    }
    if !toolchain_ready {
        return Err(CliFailure::new(
            ExitClass::Incompatible,
            "TOOLCHAIN_MISMATCH",
            "developer prerequisites do not match repository pins",
        )
        .with_result(report));
    }
    Ok(report)
}

/// Renders the human lines for `doctor`.
pub fn lines(value: &serde_json::Value) -> Vec<String> {
    let mut lines = vec![
        "Academic OS Phase 1 doctor".to_owned(),
        format!(
            "runtime egress: {}",
            value["network_egress"].as_str().unwrap_or("")
        ),
    ];
    if let Some(checks) = value["checks"].as_array() {
        for check in checks {
            let supported = check["supported"].as_bool().unwrap_or(false);
            lines.push(format!(
                "- {}: {} ({}, expected {})",
                check["tool"].as_str().unwrap_or(""),
                check["observed"].as_str().unwrap_or("missing"),
                if supported { "ok" } else { "unsupported" },
                check["expected"].as_str().unwrap_or("")
            ));
            if !supported {
                lines.push(format!(
                    "  remediation: {}",
                    check["remediation"].as_str().unwrap_or("")
                ));
            }
        }
    }
    if value["profile"].is_object() {
        let profile = &value["profile"];
        lines.push(format!(
            "profile: {}",
            value["profile_root"].as_str().unwrap_or("")
        ));
        lines.push(format!(
            "  synthetic marker present: {}",
            profile["synthetic_marker_present"]
        ));
        lines.push(format!(
            "  store schema: {} ({})",
            profile["store"]["schema_version"],
            profile["store"]["schema_semver"].as_str().unwrap_or("")
        ));
        lines.push(format!(
            "  accept_seq head: {} outbox head: {} revision: {}",
            profile["canonical"]["accept_seq_head"],
            profile["canonical"]["outbox_head"],
            profile["canonical"]["profile_revision"]
        ));
        if profile["deep"].as_bool().unwrap_or(false) {
            lines.push(format!(
                "  integrity check: {} foreign key check: {}",
                profile["integrity_check"], profile["foreign_key_check"]
            ));
            lines.push(format!(
                "  orphan temp entries: {}",
                profile["orphan_temp_entries"]
                    .as_array()
                    .map_or(0, Vec::len)
            ));
            lines.push(format!(
                "  quarantined entries: {}",
                profile["quarantined_entries"]
                    .as_array()
                    .map_or(0, Vec::len)
            ));
            if let Some(projections) = profile["projections"].as_array() {
                for projection in projections {
                    lines.push(format!(
                        "  projection {}: active={} lag={}",
                        projection["kind"].as_str().unwrap_or(""),
                        projection["active"],
                        projection["lag"]
                    ));
                }
            }
        }
        if let Some(findings) = profile["findings"].as_array() {
            for finding in findings {
                lines.push(format!(
                    "  finding {} [{}]: {}",
                    finding["code"].as_str().unwrap_or(""),
                    finding["severity"].as_str().unwrap_or(""),
                    finding["detail"].as_str().unwrap_or("")
                ));
            }
        }
    }
    lines
}

fn check_tool(specification: &ToolVersionSpecification) -> ToolCheck {
    let resolved_path = resolve_executable(&specification.name);
    let observed = observe_tool_version(&specification.name, resolved_path.as_deref());
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

/// Reads `--version` from the tool, preferring the platform's own resolution.
///
/// Windows program resolution appends only `.exe` and never consults
/// `PATHEXT`, so a tool installed as a `.cmd` shim cannot be spawned by bare
/// name at all. That is exactly what `npm install --global pnpm@11.22.0` — the
/// remediation this command prints, and the documented Windows bootstrap step —
/// writes. The PATHEXT-aware search has already located the shim, so it answers
/// when the name cannot; a conforming host must not read as a missing tool.
fn observe_tool_version(name: &str, resolved_path: Option<&Path>) -> Option<String> {
    tool_version(Path::new(name)).or_else(|| resolved_path.and_then(tool_version))
}

fn tool_version(program: &Path) -> Option<String> {
    Command::new(program)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
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

fn validate_tool_version_corpus(corpus: &ToolVersionCorpus) -> Result<(), CliFailure> {
    let invalid = |detail: &str| {
        CliFailure::new(
            ExitClass::Internal,
            "TOOL_CORPUS_INVALID",
            detail.to_owned(),
        )
    };
    if corpus.schema_version != 1
        || corpus
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .ne(["rustc", "cargo", "node", "pnpm"])
    {
        return Err(invalid(
            "tool-version conformance corpus has an unsupported shape",
        ));
    }
    for tool in &corpus.tools {
        if tool.expected.is_empty()
            || tool.remediation.is_empty()
            || !tool.cases.iter().any(|test_case| test_case.supported)
            || !tool.cases.iter().any(|test_case| !test_case.supported)
        {
            return Err(invalid(&format!(
                "tool-version conformance corpus is incomplete for {}",
                tool.name
            )));
        }
        for test_case in &tool.cases {
            if is_supported_tool_version(tool, &test_case.output) != test_case.supported {
                return Err(invalid(&format!(
                    "tool-version conformance disagreement for {}: {}",
                    tool.name, test_case.name
                )));
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

    /// A tool the platform's bare-name resolution cannot reach must still be
    /// observed through the path the `PATHEXT`-aware search resolved.
    ///
    /// On Windows that is not hypothetical: `npm install --global` writes a
    /// `.cmd` shim and no `.exe`, and a bare-name spawn there appends only
    /// `.exe`. Reporting such a tool as absent fails a conforming host.
    #[test]
    fn a_shim_only_tool_is_observed_through_its_resolved_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::TempDir::new()?;
        let shim = write_version_shim(directory.path(), "11.22.0")?;

        assert_eq!(
            observe_tool_version("academic-tool-absent-from-every-path", Some(&shim)),
            Some("11.22.0".to_owned()),
            "the resolved shim must answer when the bare name cannot be spawned"
        );
        assert_eq!(
            observe_tool_version("academic-tool-absent-from-every-path", None),
            None,
            "an unresolvable tool stays unobserved rather than inventing a version"
        );
        Ok(())
    }

    /// Writes an executable that prints `version` and nothing else, in the one
    /// form the host can run without an interpreter argument.
    #[cfg(windows)]
    fn write_version_shim(
        directory: &Path,
        version: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let shim = directory.join("academic-version-shim.cmd");
        std::fs::write(
            &shim,
            format!(
                "@ECHO OFF
ECHO {version}
"
            ),
        )?;
        Ok(shim)
    }

    #[cfg(unix)]
    fn write_version_shim(
        directory: &Path,
        version: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let shim = directory.join("academic-version-shim");
        std::fs::write(
            &shim,
            format!(
                "#!/bin/sh
echo {version}
"
            ),
        )?;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755))?;
        Ok(shim)
    }
}
