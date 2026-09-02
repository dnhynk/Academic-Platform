//! Headless CLI for admission, daemon, doctor, ingest, export, backup, restore,
//! crash-replay, and deterministic fixture operations.
//!
//! Three invariants shape this binary:
//!
//! 1. Every path emits the receipt-derived posture. The current unprovisioned
//!    acceptance key keeps that posture synthetic.
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
    about = "Academic OS local-core CLI",
    long_about = "Operates local profiles and verifies the compiled admission contract. \
                  This build has no provisioned acceptance key, so production data remains denied."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Verifies or shows the signed data-admission receipt posture.
    Admission {
        #[command(subcommand)]
        command: AdmissionCommand,
    },
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
enum AdmissionCommand {
    /// Verifies the receipt and fails when admission is denied.
    Verify {
        /// Profile root containing `admission/receipt.cbor`.
        #[arg(long)]
        profile: PathBuf,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Shows the emitted posture and any denial reason.
    Show {
        /// Profile root containing `admission/receipt.cbor`.
        #[arg(long)]
        profile: PathBuf,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
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
    let posture = policy_banner::posture_for_profile(command_profile(&cli.command));

    // The banner precedes every result on every path, including failures.
    if let Err(error) = write_banner(format, &posture) {
        eprintln!("failed to write the mandatory policy banner: {error}");
        return ExitCode::from(u8::try_from(ExitClass::Internal.code()).unwrap_or(20));
    }

    let (outcome, render): (CommandResult, Renderer) = dispatch(cli.command);
    let class = match &outcome {
        Ok(value) => {
            let lines = render(value);
            if let Err(error) = emit_success(name, format, value, &lines, &posture) {
                eprintln!("failed to write the command result: {error}");
                return exit_code(ExitClass::Internal);
            }
            ExitClass::Ok
        }
        Err(failure) => {
            if let Err(error) = emit_failure(name, format, failure, &posture) {
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
        Commands::Admission { command } => match command {
            AdmissionCommand::Verify { format, .. } => ("admission verify", *format),
            AdmissionCommand::Show { format, .. } => ("admission show", *format),
        },
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

fn command_profile(command: &Commands) -> Option<&std::path::Path> {
    match command {
        Commands::Admission { command } => match command {
            AdmissionCommand::Verify { profile, .. } | AdmissionCommand::Show { profile, .. } => {
                Some(profile.as_path())
            }
        },
        Commands::Daemon { command } => match command {
            DaemonCommand::Serve { profile, .. } | DaemonCommand::Status { profile, .. } => {
                Some(profile.as_path())
            }
        },
        Commands::Doctor { profile, .. } => match profile {
            Some(profile) => Some(profile.as_path()),
            None => None,
        },
        Commands::Ingest { profile, .. }
        | Commands::Export { profile, .. }
        | Commands::Backup { profile, .. } => Some(profile.as_path()),
        Commands::Restore { .. } | Commands::CrashReplay { .. } | Commands::Fixture { .. } => None,
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
        Commands::Admission { command } => match command {
            AdmissionCommand::Verify { profile, .. } => (
                match commands::native_path(&profile) {
                    Ok(profile) => commands::admission::verify(&profile),
                    Err(failure) => Err(failure),
                },
                commands::admission::lines,
            ),
            AdmissionCommand::Show { profile, .. } => (
                match commands::native_path(&profile) {
                    Ok(profile) => Ok(commands::admission::show(&profile)),
                    Err(failure) => Err(failure),
                },
                commands::admission::lines,
            ),
        },
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
                "admission",
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

    /// Tokens the admission crate spells, how many times it may spell each one,
    /// and whether any other crate's `src` tree may spell it at all.
    ///
    /// This is what replaced the walk's old `!= "admission"` skip. That skip
    /// excused the entire admission tree from the scan, so a second key source
    /// or a second admitted constructor placed beside `lib.rs` was invisible to
    /// the only guard that claims to forbid one.
    ///
    /// Three of the counts are *type paths* rather than field-initialiser
    /// spellings, because the `T141` audit built an admitted posture without
    /// writing the spelling that was counted. `kind: PostureKind::Admitted {`
    /// only exists while the value is written directly into the struct literal;
    /// `let admitted = PostureKind::Admitted { .. }; Posture { kind: admitted }`
    /// is the same value, spells it zero times, and passed. A type path
    /// survives that rewrite, so `PostureKind::Admitted`, `VerifiedAdmission`
    /// and `Posture {` are counted, and the four narrower spellings stay under
    /// them: a rewrite that keeps the type-path count and changes a
    /// construction site still moves one of those four.
    ///
    /// `Posture {` is the one entry no other crate is forbidden: `Posture`'s
    /// field is private, so nothing outside `academic-admission` can construct
    /// one, and the same three characters occur there as a return type
    /// (`-> &Posture {`) and inside `DataPosture {`. Counting it *inside* the
    /// admission tree is what bounds the construction sites; forbidding it
    /// outside would only refuse an unrelated spelling.
    const ADMISSION_AUTHORITY_TOKENS: [(&str, usize, bool); 10] = [
        ("pub const ACCEPTANCE_PUBLIC_KEY", 1, true),
        ("AcceptancePublicKey::Provisioned", 1, true),
        ("fn verify_with_compiled_acceptance_key", 1, true),
        ("PostureKind::Admitted", 12, true),
        ("VerifiedAdmission", 7, true),
        ("Posture {", 6, false),
        ("kind: PostureKind::Admitted {", 1, true),
        ("Posture::from_verified(", 1, true),
        ("Ok(VerifiedAdmission {", 1, true),
        ("VerifiedAdmission {", 3, true),
    ];

    /// Directories under a crate root that hold test targets, not product code.
    ///
    /// The admission walk reads the whole crate rather than only `src`, because
    /// `#[path = "../authority.rs"]` reaches a product module that `src` does
    /// not contain. These three are what Cargo compiles as separate test,
    /// benchmark and example targets, so they are not product and their text
    /// would blow every allowance above. A `#[path]` or a `mod` reaching into
    /// one of them is refused by the tripwire instead, which is where a reach
    /// into unread source belongs.
    const CARGO_TEST_TARGET_DIRECTORIES: [&str; 3] = ["benches", "examples", "tests"];

    /// Key and override seams forbidden anywhere in the admission crate's
    /// product source — every `*.rs` under `crates/admission/src`, not one file.
    const ADMISSION_FORBIDDEN_SEAMS: [&str; 12] = [
        "std::env",
        "env!(",
        "env::var(",
        "env::var_os(",
        "debug_assertions",
        "include_bytes!",
        "include_str!",
        "set_acceptance_key",
        "with_acceptance_key",
        "override_acceptance",
        "acceptance_key_from",
        "verify_with_test_key",
    ];

    /// Line starts that declare an item at file scope.
    const FILE_SCOPE_ITEM_STARTS: [&str; 17] = [
        "#[",
        "async ",
        "const ",
        "enum ",
        "extern ",
        "fn ",
        "impl ",
        "macro_rules!",
        "mod ",
        "pub ",
        "static ",
        "struct ",
        "trait ",
        "type ",
        "union ",
        "unsafe ",
        "use ",
    ];

    /// The compiled acceptance key, spelled out. Nothing else may declare it.
    ///
    /// Provisioning replaces `Unprovisioned` with one 32-byte literal and
    /// updates this constant in the same commit. That is why the whole
    /// declaration is pinned instead of a token being forbidden: `option_env!`,
    /// a `const` computed in another item, and a `match` on the build profile
    /// all leave every token list untouched and still choose the key.
    const WHOLE_ACCEPTANCE_KEY: &str = concat!(
        "pub const ACCEPTANCE_PUBLIC_KEY: AcceptancePublicKey = ",
        "AcceptancePublicKey::Unprovisioned;"
    );

    /// The whole compiled-key check, whitespace-collapsed. Nothing else may be
    /// in it — a runtime key file read added inside this function is a key
    /// source that no forbidden token names.
    const WHOLE_KEY_CHECK: &str = concat!(
        "fn verify_with_compiled_acceptance_key(decoded: &DecodedEnvelope) ",
        "-> Result<(), AdmissionError> { ",
        "let AcceptancePublicKey::Provisioned(expected) = ACCEPTANCE_PUBLIC_KEY else { ",
        "return Err(AdmissionError::AcceptanceKeyUnprovisioned); }; ",
        "if expected == [0_u8; 32] { return Err(AdmissionError::InvalidAcceptanceKey); } ",
        "if decoded.public_key != expected { return Err(AdmissionError::SignerKeyMismatch); } ",
        "let verifying = VerifyingKey::from_bytes(&expected)",
        ".map_err(|_| AdmissionError::InvalidAcceptanceKey)?; ",
        "verifying .verify_strict(&decoded.payload, ",
        "&Signature::from_bytes(&decoded.signature)) ",
        ".map_err(|_| AdmissionError::InvalidSignature) }"
    );

    /// The whole receipt verifier, whitespace-collapsed.
    ///
    /// A pin fixes the text of the item it names and nothing about who calls
    /// it. `WHOLE_KEY_CHECK` above holds the compiled-key check to five exact
    /// lines, and the `T141` audit went around it without editing one
    /// character: it wrapped the *call* in
    /// `if !profile_root.join("admission/field-trust").is_file() { .. }`.
    /// Signature verification was then skipped whenever a marker file existed,
    /// and an audit probe observed a receipt signed by a key this build has
    /// never heard of receiving an admitted posture with
    /// `production_data_allowed()` true. Nothing else saw it: the marker does
    /// not exist in the shipped tree, so no behaviour changed, and no forbidden
    /// token was spelled.
    ///
    /// What the contract actually is, therefore, is not the check's text but
    /// this function's: five steps, in this order, each unconditional. So the
    /// whole of it is pinned — the same shape `rotation_gate.rs` uses when it
    /// fixes the first statement of all seven gated entry points rather than
    /// only the gate they call.
    const WHOLE_VERIFY: &str = concat!(
        "pub fn verify(profile_root: &Path) -> Result<VerifiedAdmission, AdmissionError> { ",
        "let path = profile_root.join(ADMISSION_RECEIPT_RELATIVE_PATH); ",
        "let bytes = match fs::read(&path) { Ok(bytes) => bytes, ",
        "Err(source) if source.kind() == std::io::ErrorKind::NotFound => { ",
        "return Err(AdmissionError::ReceiptAbsent { path }); } ",
        "Err(source) => { return Err(AdmissionError::ReadReceipt { path, source }); } }; ",
        "if bytes.len() > MAX_RECEIPT_BYTES { ",
        "return Err(AdmissionError::ReceiptTooLarge { ",
        "actual: bytes.len(), maximum: MAX_RECEIPT_BYTES, }); } ",
        "let decoded = decode_envelope(&bytes)?; ",
        "verify_with_compiled_acceptance_key(&decoded)?; ",
        "let payload = decode_payload(&decoded.payload)?; ",
        "let verified = validate_payload(payload, &bytes)?; ",
        "require_encrypted_profile_marker(profile_root)?; ",
        "Ok(verified) }"
    );

    /// Returns `signature` and everything up to the first `closer`, whitespace
    /// collapsed, with comment-only lines dropped so a pin fixes code and not
    /// prose.
    fn whole(source: &str, signature: &str, closer: &str) -> Option<String> {
        let (_, rest) = source.split_once(signature)?;
        let (body, _) = rest.split_once(closer)?;
        Some(
            format!("{signature}{body}{closer}")
                .lines()
                .map(str::trim)
                .filter(|line| !line.starts_with("//"))
                .flat_map(str::split_whitespace)
                .collect::<Vec<_>>()
                .join(" "),
        )
    }

    /// Names of the out-of-line modules one source file declares.
    ///
    /// A `mod name;` carrying its own `#[path]` is left to
    /// [`path_attribute_targets`], which resolves what it actually names.
    fn declared_modules(text: &str) -> Vec<String> {
        let mut previous = "";
        let mut names = Vec::new();
        for line in text.lines().map(str::trim) {
            let declared = line.strip_suffix(';').and_then(|item| {
                item.strip_prefix("mod ")
                    .or_else(|| item.strip_prefix("pub mod "))
            });
            if let Some(name) = declared.filter(|_| !previous.starts_with("#[path")) {
                names.push(name.to_owned());
            }
            if !line.is_empty() {
                previous = line;
            }
        }
        names
    }

    /// The two paths a `mod name;` in `declaring` may live at.
    fn module_files(declaring: &std::path::Path, name: &str) -> [std::path::PathBuf; 2] {
        let directory = match declaring.file_stem().and_then(|stem| stem.to_str()) {
            Some("lib" | "mod") => declaring
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default(),
            _ => declaring.with_extension(""),
        };
        [
            directory.join(format!("{name}.rs")),
            directory.join(name).join("mod.rs"),
        ]
    }

    /// Every file a `#[path = "…"]` in `declaring` pulls into the crate.
    ///
    /// `..` is resolved textually against the declaring file's directory rather
    /// than through the filesystem, so a target that does not exist still
    /// produces a path — and a path the walk did not read is what the caller
    /// refuses. A target that climbs out of `crates/` resolves to something the
    /// walk cannot have read either, so it fails the same way.
    fn path_attribute_targets(declaring: &std::path::Path, text: &str) -> Vec<std::path::PathBuf> {
        let mut targets = Vec::new();
        for (at, _) in text.match_indices("#[path") {
            let Some((_, rest)) = text[at..].split_once('"') else {
                continue;
            };
            let Some((quoted, _)) = rest.split_once('"') else {
                continue;
            };
            let mut resolved = declaring
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default();
            for component in quoted.split(['/', '\\']) {
                match component {
                    "" | "." => {}
                    ".." => {
                        resolved.pop();
                    }
                    other => resolved.push(other),
                }
            }
            targets.push(resolved);
        }
        targets
    }

    /// Reads one tree: every `*.rs` under it, product halves only.
    ///
    /// Directories named in `skip` are not descended into when they sit
    /// directly under `root`; a directory of the same name deeper in the tree
    /// is read, because only the crate root's `tests`, `benches` and `examples`
    /// are Cargo test targets.
    ///
    /// A file's product half is everything above its test module. Anything
    /// declared at file scope *below* that module is product code the split
    /// would hide, so this refuses the file rather than returning a half that
    /// does not cover it.
    ///
    /// "At file scope" is decided by the test module's closing brace and not by
    /// a line's indentation. The rule used to read `line.starts_with(start)`,
    /// which is a rule about column 0 and not about scope: the `T141` audit
    /// indented the same item by two spaces and it passed. `cargo fmt --check`
    /// refused the indentation, so nothing shipped — but that made `cargo fmt`,
    /// not this scan, the thing carrying a contract this repository states as a
    /// property of the scan. The module's own body is skipped rather than
    /// trimmed, because every line in it is an indented item.
    fn product_sources(
        root: &std::path::Path,
        skip: &[&str],
    ) -> Result<Vec<(std::path::PathBuf, String)>, Box<dyn std::error::Error>> {
        let mut pending = vec![root.to_path_buf()];
        let mut sources = Vec::new();
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory)? {
                let path = entry?.path();
                if path.is_dir() {
                    let skipped = path.parent() == Some(root)
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| skip.contains(&name));
                    if !skipped {
                        pending.push(path);
                    }
                    continue;
                }
                if path.extension().is_none_or(|extension| extension != "rs") {
                    continue;
                }
                let source = std::fs::read_to_string(&path)?;
                let product = match source.split_once("#[cfg(test)]\nmod tests") {
                    None => source,
                    Some((product, below)) => {
                        let after =
                            below
                                .split_once("\n}")
                                .map(|(_, rest)| rest)
                                .ok_or_else(|| {
                                    format!(
                                        "{}'s test module never closes at file scope",
                                        path.display()
                                    )
                                })?;
                        for line in after.lines() {
                            assert!(
                                !FILE_SCOPE_ITEM_STARTS
                                    .iter()
                                    .any(|start| line.trim_start().starts_with(start)),
                                "{} declares {line} at file scope below its test module",
                                path.display()
                            );
                        }
                        product.to_owned()
                    }
                };
                sources.push((path, product));
            }
        }
        sources.sort();
        Ok(sources)
    }

    /// No build input, no file, and no flag selects the acceptance key.
    ///
    /// The `P2-K6` audit put five different key substitutions past the earlier
    /// shape of this test — a build environment variable read through
    /// `option_env!`, a runtime key file, a second module file in
    /// `crates/admission/src`, product code below `lib.rs`'s test module, and a
    /// `debug_assertions` branch. Each one is a shipped-shaped change that
    /// alters nothing observable without its trigger, so a source scan is the
    /// only thing that can refuse it, and the scan read one file's head against
    /// a token list while the walk skipped the admission directory.
    ///
    /// So this reads all of it: every `*.rs` under every crate's `src` and,
    /// for `crates/admission`, every `*.rs` under the crate — with a tripwire
    /// saying every module those files pull in is a file that was read. The
    /// authority tokens are counted against an explicit per-crate allowance,
    /// and the three places the key is obtained, checked, and used are pinned
    /// as whole text rather than by the names they happen to use today.
    #[test]
    fn no_environment_or_flag_override_exists() -> Result<(), Box<dyn std::error::Error>> {
        use std::fs;

        let crates_root =
            fs::canonicalize(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."))?;
        let mut admission_product = String::new();
        let mut admission_seen = false;
        for entry in fs::read_dir(&crates_root)? {
            let crate_root = entry?.path();
            let source_root = crate_root.join("src");
            if !source_root.is_dir() {
                continue;
            }
            let is_admission = crate_root
                .file_name()
                .is_some_and(|name| name == "admission");
            admission_seen |= is_admission;
            // The admission crate is read whole, not just its `src`. A
            // `#[path = "../authority.rs"]` reaches a product module beside
            // `src`, and a walk rooted at `src` reads neither that file's seams
            // nor its authority tokens; the `T141` audit built an admitted
            // posture there and no check in this repository saw it.
            let (scanned_root, sources) = if is_admission {
                (
                    crate_root.clone(),
                    product_sources(&crate_root, &CARGO_TEST_TARGET_DIRECTORIES)?,
                )
            } else {
                (source_root.clone(), product_sources(&source_root, &[])?)
            };
            if is_admission {
                // Widening the walk is not the same as covering the crate. What
                // makes it cover the crate is that every module the read files
                // pull in is itself a file that was read — the tripwire `P2-RF8`
                // put on `academic-recovery` and `academic-record`, plus the
                // `#[path]` half neither of those needed.
                let read: Vec<&std::path::Path> =
                    sources.iter().map(|(path, _)| path.as_path()).collect();
                for (path, product) in &sources {
                    for target in path_attribute_targets(path, product) {
                        assert!(
                            read.contains(&target.as_path()),
                            "{} pulls in {} through #[path], which the scan never read",
                            path.display(),
                            target.display()
                        );
                    }
                    for name in declared_modules(product) {
                        let candidates = module_files(path, &name);
                        assert!(
                            candidates
                                .iter()
                                .any(|candidate| read.contains(&&**candidate)),
                            "{} declares `mod {name};` but the scan read neither {} nor {}",
                            path.display(),
                            candidates[0].display(),
                            candidates[1].display()
                        );
                    }
                    for forbidden in ADMISSION_FORBIDDEN_SEAMS {
                        assert!(
                            !product.contains(forbidden),
                            "{} contains product key/override seam {forbidden}",
                            path.display()
                        );
                    }
                    admission_product.push_str(product);
                    admission_product.push('\n');
                }
            }
            for (token, allowance, exclusive) in ADMISSION_AUTHORITY_TOKENS {
                if !is_admission && !exclusive {
                    continue;
                }
                let allowed = if is_admission { allowance } else { 0 };
                let found = sources
                    .iter()
                    .map(|(_, product)| product.matches(token).count())
                    .sum::<usize>();
                assert_eq!(
                    found,
                    allowed,
                    "{} spells the admission-authority token {token} {found} times, not {allowed}",
                    scanned_root.display()
                );
            }
        }
        assert!(
            admission_seen,
            "the walk never reached crates/admission, so it proved nothing"
        );

        let declaration = admission_product
            .lines()
            .find(|line| line.starts_with("pub const ACCEPTANCE_PUBLIC_KEY"))
            .ok_or("the admission crate does not declare the compiled acceptance key")?;
        assert_eq!(
            declaration.split_whitespace().collect::<Vec<_>>().join(" "),
            WHOLE_ACCEPTANCE_KEY,
            "the compiled acceptance key is chosen by something other than this declaration"
        );

        let check = whole(
            &admission_product,
            "fn verify_with_compiled_acceptance_key",
            "\n}",
        )
        .ok_or("the admission crate has no compiled-key check")?;
        assert_eq!(
            check, WHOLE_KEY_CHECK,
            "the compiled-key check takes a key from somewhere other than the constant"
        );

        // The call sites, not only the item. `WHOLE_KEY_CHECK` above fixes what
        // the compiled-key check does and says nothing about whether it runs;
        // this fixes the five steps that run, their order, and that none of
        // them is behind a condition.
        let verify = whole(
            &admission_product,
            "pub fn verify(profile_root: &Path)",
            "\n    }",
        )
        .ok_or("the admission crate has no receipt verifier")?;
        assert_eq!(
            verify, WHOLE_VERIFY,
            "AdmissionVerifier::verify no longer runs its five steps unconditionally, in order"
        );

        fn scan_command(command: &clap::Command) {
            let forbidden = [
                "acceptance-key",
                "public-key",
                "allow-real",
                "production",
                "override",
                "unsafe",
                "debug",
                "quiet",
            ];
            for argument in command.get_arguments() {
                let id = argument.get_id().as_str();
                let long = argument.get_long().unwrap_or_default();
                for token in forbidden {
                    assert!(
                        !id.contains(token) && !long.contains(token),
                        "{} exposes forbidden CLI argument {id}/{long}",
                        command.get_name()
                    );
                }
            }
            for child in command.get_subcommands() {
                scan_command(child);
            }
        }
        scan_command(&Cli::command());
        Ok(())
    }
}
