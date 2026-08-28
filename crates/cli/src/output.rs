//! Output representation, the failure taxonomy, and its exit codes.
//!
//! Every command emits the same envelope. In `human` mode the policy banner is
//! the first line on standard output and results follow it. In `json` mode
//! standard output carries exactly one JSON document so a caller can parse it
//! without stripping a preamble, the banner goes to standard error, and the
//! document itself repeats both the banner and the policy object.

use std::io::{self, Write};

use clap::ValueEnum;
use serde::Serialize;

use crate::policy_banner::{DataPolicy, PHASE1_POLICY_BANNER, data_policy};

/// Output representation selected by `--format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum OutputFormat {
    /// Banner first, then human-readable results.
    Human,
    /// Exactly one JSON document on standard output.
    Json,
}

/// Outcome class of one command, and the process exit code it produces.
///
/// The classes are distinguished so a caller can branch on *why* a command
/// failed without parsing prose. `tests/cli.rs` asserts each mapping against
/// the real binary rather than trusting this table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExitClass {
    /// The command completed.
    Ok,
    /// The synthetic-only data policy refused the request.
    PolicyDenied,
    /// An expected revision, idempotency key, or destination conflicted.
    Conflict,
    /// The profile must be repaired before it can be served or published.
    RepairRequired,
    /// A protocol major version or capability could not be negotiated.
    Incompatible,
    /// No daemon owns the profile, so an IPC-only command cannot proceed.
    Unavailable,
    /// The command failed for a reason none of the above describes.
    Internal,
}

impl ExitClass {
    /// Returns the process exit code for this class.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Ok => 0,
            Self::PolicyDenied => 10,
            Self::Conflict => 11,
            Self::RepairRequired => 12,
            Self::Incompatible => 13,
            Self::Unavailable => 14,
            Self::Internal => 20,
        }
    }

    /// Returns the stable machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::PolicyDenied => "POLICY_DENIED",
            Self::Conflict => "CONFLICT",
            Self::RepairRequired => "REPAIR_REQUIRED",
            Self::Incompatible => "INCOMPATIBLE",
            Self::Unavailable => "UNAVAILABLE",
            Self::Internal => "INTERNAL",
        }
    }
}

/// A command failure carrying its class, a stable reason code, and detail.
#[derive(Debug)]
pub struct CliFailure {
    class: ExitClass,
    reason: String,
    detail: String,
    result: Option<serde_json::Value>,
}

impl CliFailure {
    /// Builds one classified failure.
    pub fn new(class: ExitClass, reason: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            class,
            reason: reason.into(),
            detail: detail.into(),
            result: None,
        }
    }

    /// Attaches the structured report a failing command still produced.
    ///
    /// A doctor that exits with `REPAIR_REQUIRED` must still show which
    /// findings demanded the repair, so the failure document carries them.
    #[must_use]
    pub fn with_result(mut self, result: serde_json::Value) -> Self {
        self.result = Some(result);
        self
    }

    /// Builds an internal failure from an arbitrary error.
    pub fn internal(reason: impl Into<String>, error: impl std::fmt::Display) -> Self {
        Self::new(ExitClass::Internal, reason, error.to_string())
    }

    /// Returns the outcome class.
    #[must_use]
    pub const fn class(&self) -> ExitClass {
        self.class
    }

    /// Returns the stable machine-readable reason code.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl std::fmt::Display for CliFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.reason, self.detail)
    }
}

impl std::error::Error for CliFailure {}

/// Result alias used by every command entry point.
pub type CommandResult = Result<serde_json::Value, CliFailure>;

#[derive(Debug, Serialize)]
struct Envelope<'a> {
    command: &'a str,
    status: &'a str,
    exit_code: i32,
    banner: &'a str,
    policy: DataPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBlock<'a>>,
}

#[derive(Debug, Serialize)]
struct ErrorBlock<'a> {
    reason: &'a str,
    detail: &'a str,
}

/// Writes the banner as the first human-readable line.
///
/// `json` mode sends it to standard error so standard output stays parseable.
pub fn write_banner(format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Human => {
            let mut stdout = io::stdout().lock();
            writeln!(stdout, "{PHASE1_POLICY_BANNER}")?;
            stdout.flush()
        }
        OutputFormat::Json => {
            let mut stderr = io::stderr().lock();
            writeln!(stderr, "{PHASE1_POLICY_BANNER}")?;
            stderr.flush()
        }
    }
}

fn write_json(envelope: &Envelope<'_>) -> io::Result<()> {
    let rendered = serde_json::to_string_pretty(envelope)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{rendered}")?;
    stdout.flush()
}

/// Emits one successful result in the selected representation.
///
/// `human` receives the already-rendered lines; `json` receives the structured
/// value. Both carry the policy object.
pub fn emit_success(
    command: &str,
    format: OutputFormat,
    value: &serde_json::Value,
    human: &[String],
) -> io::Result<()> {
    match format {
        OutputFormat::Human => {
            let mut stdout = io::stdout().lock();
            for line in human {
                writeln!(stdout, "{line}")?;
            }
            writeln!(stdout, "data policy: {}", data_policy().data_policy)?;
            writeln!(
                stdout,
                "production data allowed: {}",
                data_policy().production_data_allowed
            )?;
            stdout.flush()
        }
        OutputFormat::Json => write_json(&Envelope {
            command,
            status: ExitClass::Ok.as_str(),
            exit_code: ExitClass::Ok.code(),
            banner: PHASE1_POLICY_BANNER,
            policy: data_policy(),
            result: Some(value),
            error: None,
        }),
    }
}

/// Emits one classified failure in the selected representation.
pub fn emit_failure(command: &str, format: OutputFormat, failure: &CliFailure) -> io::Result<()> {
    match format {
        OutputFormat::Human => {
            let mut stderr = io::stderr().lock();
            writeln!(
                stderr,
                "{}: {} ({})",
                failure.class.as_str(),
                failure.detail,
                failure.reason
            )?;
            stderr.flush()
        }
        OutputFormat::Json => write_json(&Envelope {
            command,
            status: failure.class.as_str(),
            exit_code: failure.class.code(),
            banner: PHASE1_POLICY_BANNER,
            policy: data_policy(),
            result: failure.result.as_ref(),
            error: Some(ErrorBlock {
                reason: &failure.reason,
                detail: &failure.detail,
            }),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_class_has_a_distinct_nonzero_code() {
        let classes = [
            ExitClass::PolicyDenied,
            ExitClass::Conflict,
            ExitClass::RepairRequired,
            ExitClass::Incompatible,
            ExitClass::Unavailable,
            ExitClass::Internal,
        ];
        let mut codes = classes.map(ExitClass::code).to_vec();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), classes.len(), "exit codes must be distinct");
        assert!(classes.iter().all(|class| class.code() != 0));
        assert_eq!(ExitClass::Ok.code(), 0);
    }

    #[test]
    fn exit_codes_do_not_collide_with_the_clap_usage_code() {
        // clap exits 2 on a usage error, so no outcome class may claim 2.
        assert!(
            [
                ExitClass::Ok,
                ExitClass::PolicyDenied,
                ExitClass::Conflict,
                ExitClass::RepairRequired,
                ExitClass::Incompatible,
                ExitClass::Unavailable,
                ExitClass::Internal,
            ]
            .iter()
            .all(|class| class.code() != 2)
        );
    }

    #[test]
    fn a_failure_envelope_always_carries_the_policy_object()
    -> Result<(), Box<dyn std::error::Error>> {
        let failure = CliFailure::new(ExitClass::PolicyDenied, "FIXTURE_NOT_ALLOWLISTED", "no");
        let envelope = Envelope {
            command: "ingest",
            status: failure.class.as_str(),
            exit_code: failure.class.code(),
            banner: PHASE1_POLICY_BANNER,
            policy: data_policy(),
            result: failure.result.as_ref(),
            error: Some(ErrorBlock {
                reason: &failure.reason,
                detail: &failure.detail,
            }),
        };
        let rendered = serde_json::to_value(&envelope)?;
        assert_eq!(
            rendered["policy"]["data_policy"],
            "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED"
        );
        assert_eq!(rendered["policy"]["production_data_allowed"], false);
        assert_eq!(rendered["exit_code"], 10);
        Ok(())
    }
}
