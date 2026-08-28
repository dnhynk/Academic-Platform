//! Headless Phase 1 CLI for daemon, doctor, ingest, export, backup, restore,
//! crash-replay, and deterministic fixture operations.
//!
//! Three invariants shape this binary:
//!
//! 1. Every path emits the policy banner and the policy object. There is no
//!    quiet flag, no environment override, no configuration key, and no debug
//!    path that suppresses either or admits real data.
//! 2. It never opens the canonical writer. Ingest — the only canonical mutation
//!    — travels over local IPC to the daemon, which is the sole writer.
//! 3. Failures are classified, so a caller can distinguish a policy denial from
//!    a conflict, a profile needing repair, an incompatible artefact, an
//!    unavailable daemon, and an internal fault by exit code alone.

mod client;
mod commands;
mod output;
mod policy_banner;

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

use crate::{
    client::default_runtime_root,
    output::{
        CliFailure, CommandResult, ExitClass, OutputFormat, emit_failure, emit_success,
        write_banner,
    },
};

#[derive(Debug, Parser)]
#[command(
    name = "academic",
    version,
    about = "Academic OS Phase 1 synthetic local-core CLI",
    long_about = "Operates a synthetic, plaintext, throwaway Phase 1 profile. \
                  Real or production data is forbidden and no flag enables it."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Hosts or inspects the local-core daemon.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    /// Checks pinned prerequisites and, with `--profile`, profile health.
    Doctor {
        /// Synthetic profile root to inspect.
        #[arg(long)]
        profile: Option<PathBuf>,
        /// Adds integrity, foreign-key, vault, and projection checks.
        #[arg(long)]
        deep: bool,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Accepts one allowlisted synthetic fixture through the daemon.
    Ingest {
        /// Synthetic profile root the daemon owns.
        #[arg(long)]
        profile: PathBuf,
        /// Allowlisted fixture identifier. This is never a file path.
        #[arg(long)]
        fixture: String,
        /// Current-user runtime root holding the daemon session metadata.
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Optimistic concurrency guard on the profile revision.
        #[arg(long)]
        expected_revision: Option<u64>,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Writes one deterministic open export directory.
    Export {
        /// Synthetic profile root to export.
        #[arg(long)]
        profile: PathBuf,
        /// Destination directory. It must not already exist.
        #[arg(long)]
        destination: PathBuf,
        /// Current-user runtime root holding the daemon session metadata.
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Publishes one plaintext synthetic backup directory.
    Backup {
        /// Synthetic profile root to back up.
        #[arg(long)]
        profile: PathBuf,
        /// Destination directory. It must not already exist.
        #[arg(long)]
        destination: PathBuf,
        /// Current-user runtime root holding the daemon session metadata.
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Restores one verified backup into a new empty profile.
    Restore {
        /// Published backup directory to restore from.
        #[arg(long)]
        backup: PathBuf,
        /// New empty profile directory to publish.
        #[arg(long)]
        new_profile: PathBuf,
        /// Current-user runtime root used only to refuse an owned destination.
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Reports the enumerated Phase 1 fault matrix.
    ///
    /// This command cannot terminate a process. It describes each fault and the
    /// outcome a restart must produce, so a harness can check a real profile
    /// against the contract.
    #[command(name = "crash-replay")]
    CrashReplay {
        /// One enumerated fault identifier, for example `DB07`.
        #[arg(long, conflicts_with = "all", required_unless_present = "all")]
        fault: Option<String>,
        /// Reports every enumerated fault.
        #[arg(long)]
        all: bool,
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
enum DaemonCommand {
    /// Hosts one foreground daemon until the terminal interrupts it.
    Serve {
        /// Synthetic profile root to serve.
        #[arg(long)]
        profile: PathBuf,
        /// Current-user runtime root for the endpoint and session metadata.
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Reports negotiated versions and current watermarks.
    Status {
        /// Synthetic profile root a daemon owns.
        #[arg(long)]
        profile: PathBuf,
        /// Current-user runtime root holding the daemon session metadata.
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum FixtureCommand {
    /// Emits the current v2 deterministic fixture JSON.
    Emit {
        /// Writes the deterministic bytes to this file when supplied.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Verifies exact bytes, signature, expected replay, and builder drift.
    Verify {
        /// Fixture document to verify.
        path: PathBuf,
    },
    /// Verifies the signature and replays accepted events from sequence zero.
    Replay {
        /// Fixture document to replay.
        path: PathBuf,
    },
}

type Renderer = fn(&serde_json::Value) -> Vec<String>;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (name, format) = describe(&cli.command);

    // The banner precedes every result on every path, including failures.
    if let Err(error) = write_banner(format) {
        eprintln!("failed to write the mandatory policy banner: {error}");
        return ExitCode::from(u8::try_from(ExitClass::Internal.code()).unwrap_or(20));
    }

    let (outcome, render): (CommandResult, Renderer) = dispatch(cli.command);
    let class = match &outcome {
        Ok(value) => {
            let lines = render(value);
            if let Err(error) = emit_success(name, format, value, &lines) {
                eprintln!("failed to write the command result: {error}");
                return exit_code(ExitClass::Internal);
            }
            ExitClass::Ok
        }
        Err(failure) => {
            if let Err(error) = emit_failure(name, format, failure) {
                eprintln!("failed to write the command failure: {error}");
                return exit_code(ExitClass::Internal);
            }
            failure.class()
        }
    };
    exit_code(class)
}

fn exit_code(class: ExitClass) -> ExitCode {
    ExitCode::from(u8::try_from(class.code()).unwrap_or(20))
}

const fn describe(command: &Commands) -> (&'static str, OutputFormat) {
    match command {
        Commands::Daemon { command } => match command {
            DaemonCommand::Serve { format, .. } => ("daemon serve", *format),
            DaemonCommand::Status { format, .. } => ("daemon status", *format),
        },
        Commands::Doctor { format, .. } => ("doctor", *format),
        Commands::Ingest { format, .. } => ("ingest", *format),
        Commands::Export { format, .. } => ("export", *format),
        Commands::Backup { format, .. } => ("backup", *format),
        Commands::Restore { format, .. } => ("restore", *format),
        Commands::CrashReplay { format, .. } => ("crash-replay", *format),
        // The fixture commands predate `--format` and always render JSON.
        Commands::Fixture { .. } => ("fixture", OutputFormat::Human),
    }
}

fn resolve_runtime(runtime: Option<PathBuf>) -> Result<PathBuf, CliFailure> {
    let root = match runtime {
        Some(root) => root,
        None => default_runtime_root()?,
    };
    commands::native_path(&root)
}

fn dispatch(command: Commands) -> (CommandResult, Renderer) {
    match command {
        Commands::Daemon { command } => match command {
            DaemonCommand::Serve {
                profile, runtime, ..
            } => (
                block_on(|| async {
                    let runtime_root = resolve_runtime(runtime)?;
                    commands::daemon::serve(&commands::native_path(&profile)?, &runtime_root).await
                }),
                commands::daemon::serve_lines,
            ),
            DaemonCommand::Status {
                profile, runtime, ..
            } => (
                block_on(|| async {
                    let runtime_root = resolve_runtime(runtime)?;
                    commands::daemon::status(&commands::native_path(&profile)?, &runtime_root).await
                }),
                commands::daemon::status_lines,
            ),
        },
        Commands::Doctor { profile, deep, .. } => (
            match profile.as_deref().map(commands::native_path).transpose() {
                Ok(profile) => commands::doctor::run(profile.as_deref(), deep),
                Err(failure) => Err(failure),
            },
            commands::doctor::lines,
        ),
        Commands::Ingest {
            profile,
            fixture,
            runtime,
            expected_revision,
            ..
        } => (
            block_on(|| async {
                let runtime_root = resolve_runtime(runtime)?;
                commands::ingest::run(
                    &commands::native_path(&profile)?,
                    &runtime_root,
                    &fixture,
                    expected_revision,
                )
                .await
            }),
            commands::ingest::lines,
        ),
        Commands::Export {
            profile,
            destination,
            runtime,
            ..
        } => (
            block_on(|| async {
                let runtime_root = resolve_runtime(runtime)?;
                commands::export::run(
                    &commands::native_path(&profile)?,
                    &commands::native_path(&destination)?,
                    &runtime_root,
                )
                .await
            }),
            commands::export::lines,
        ),
        Commands::Backup {
            profile,
            destination,
            runtime,
            ..
        } => (
            block_on(|| async {
                let runtime_root = resolve_runtime(runtime)?;
                commands::backup::run(
                    &commands::native_path(&profile)?,
                    &commands::native_path(&destination)?,
                    &runtime_root,
                )
                .await
            }),
            commands::backup::lines,
        ),
        Commands::Restore {
            backup,
            new_profile,
            runtime,
            ..
        } => (
            (|| {
                let runtime_root = resolve_runtime(runtime)?;
                commands::restore::run(
                    &commands::native_path(&backup)?,
                    &commands::native_path(&new_profile)?,
                    &runtime_root,
                )
            })(),
            commands::restore::lines,
        ),
        Commands::CrashReplay { fault, all, .. } => (
            commands::crash_replay::run(fault.as_deref(), all),
            commands::crash_replay::lines,
        ),
        Commands::Fixture { command } => (
            match command {
                FixtureCommand::Emit { output } => commands::fixture::emit(output.as_deref()),
                FixtureCommand::Verify { path } => commands::fixture::verify(&path),
                FixtureCommand::Replay { path } => commands::fixture::replay(&path),
            },
            commands::fixture::lines,
        ),
    }
}

/// Runs one asynchronous command on a current-thread runtime.
///
/// The CLI performs exactly one local IPC exchange per invocation, so a
/// multi-threaded runtime would buy nothing and only widen the surface.
fn block_on<F, Fut>(future: F) -> CommandResult
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = CommandResult>,
{
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return Err(CliFailure::internal("RUNTIME_START_FAILED", error)),
    };
    runtime.block_on(future())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_surface_is_exactly_the_phase_one_contract()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut names = Cli::command()
            .get_subcommands()
            .map(|command| command.get_name().to_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            [
                "backup",
                "crash-replay",
                "daemon",
                "doctor",
                "export",
                "fixture",
                "ingest",
                "restore",
            ]
        );
        Ok(())
    }

    #[test]
    fn daemon_exposes_only_serve_and_status() -> Result<(), Box<dyn std::error::Error>> {
        let binding = Cli::command();
        let daemon = binding
            .get_subcommands()
            .find(|command| command.get_name() == "daemon")
            .ok_or("the daemon subcommand must exist")?;
        let mut names = daemon
            .get_subcommands()
            .map(|command| command.get_name().to_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, ["serve", "status"]);
        Ok(())
    }

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
            Cli::try_parse_from(["academic", "fixture", "emit", "--fixture-version", "1"]).is_err(),
            "the CLI must not expose a caller-selectable legacy writer"
        );
        Ok(())
    }

    #[test]
    fn crash_replay_requires_exactly_one_selection() {
        assert!(Cli::try_parse_from(["academic", "crash-replay"]).is_err());
        assert!(
            Cli::try_parse_from(["academic", "crash-replay", "--fault", "DB07", "--all"]).is_err()
        );
        assert!(Cli::try_parse_from(["academic", "crash-replay", "--all"]).is_ok());
        assert!(Cli::try_parse_from(["academic", "crash-replay", "--fault", "DB07"]).is_ok());
    }

    #[test]
    fn no_flag_on_any_command_can_admit_real_data() {
        // A rejected flag is a usage error, which is exactly the point: the
        // parser has no such option to offer.
        for arguments in [
            ["academic", "ingest", "--allow-real-data"].as_slice(),
            ["academic", "ingest", "--production"].as_slice(),
            ["academic", "doctor", "--allow-real-data"].as_slice(),
            ["academic", "doctor", "--no-banner"].as_slice(),
            ["academic", "doctor", "--quiet"].as_slice(),
            ["academic", "export", "--allow-real-data"].as_slice(),
            ["academic", "backup", "--sqlcipher-key"].as_slice(),
            ["academic", "backup", "--key"].as_slice(),
            ["academic", "restore", "--in-place"].as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(arguments).is_err(),
                "{arguments:?} must not parse"
            );
        }
    }

    #[test]
    fn the_parser_offers_no_key_or_override_option() {
        let binding = Cli::command();
        for command in binding.get_subcommands() {
            for argument in command.get_arguments() {
                let name = argument.get_id().as_str();
                assert!(
                    !name.contains("key")
                        && !name.contains("real")
                        && !name.contains("production")
                        && !name.contains("sqlcipher")
                        && !name.contains("quiet"),
                    "{}::{name} is a forbidden option name",
                    command.get_name()
                );
            }
        }
    }
}
