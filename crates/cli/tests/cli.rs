//! Behavioural evidence for the Phase 1 CLI.
//!
//! These tests drive the real `academic` binary as a child process and, where
//! the surface requires it, a real foreground daemon in a disposable profile.
//! Nothing here asserts against an in-process stub: the exit codes, the banner
//! ordering, and the JSON documents are the ones a caller actually observes.

use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use assert_cmd::cargo::cargo_bin;
use predicates::{Predicate, str::contains};
use serde_json::Value;
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BANNER: &str = "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN";
const DATA_POLICY: &str = "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED";
const ALLOWLISTED_FIXTURE: &str = "phase0-synthetic-bitemporal-ledger-v2";
/// Accepted events in the committed synthetic fixture.
const FIXTURE_EVENT_COUNT: u64 = 14;

/// Exit codes the CLI promises to distinguish.
mod exit {
    pub const OK: i32 = 0;
    pub const USAGE: i32 = 2;
    pub const POLICY_DENIED: i32 = 10;
    pub const CONFLICT: i32 = 11;
    pub const REPAIR_REQUIRED: i32 = 12;
    pub const INCOMPATIBLE: i32 = 13;
    pub const UNAVAILABLE: i32 = 14;
    pub const PATH_REJECTED: i32 = 15;
    pub const INTERNAL: i32 = 20;
}

#[derive(Debug)]
struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Output {
    fn json(&self) -> TestResult<Value> {
        Ok(serde_json::from_str(&self.stdout)?)
    }
}

fn binary() -> PathBuf {
    cargo_bin("academic")
}

fn run(arguments: &[&str]) -> TestResult<Output> {
    let output = Command::new(binary()).args(arguments).output()?;
    Ok(Output {
        code: output
            .status
            .code()
            .ok_or("the CLI was terminated by a signal")?,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn run_with_env(arguments: &[&str], environment: &[(&str, &str)]) -> TestResult<Output> {
    let mut command = Command::new(binary());
    command.args(arguments);
    for (name, value) in environment {
        command.env(name, value);
    }
    let output = command.output()?;
    Ok(Output {
        code: output
            .status
            .code()
            .ok_or("the CLI was terminated by a signal")?,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// One disposable lane holding a profile root, a runtime root, and work paths.
/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the native path
/// facade refuses to follow a link component, so the tests address the real
/// directory. This mirrors `crates/daemon/tests/support`.
#[cfg(unix)]
fn temporary_base() -> std::io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

/// Windows must not canonicalize: that yields the Win32 verbatim device
/// spelling the facade rejects, trading one refused spelling for another.
#[cfg(windows)]
fn temporary_base() -> std::io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

/// Base for the runtime lane, which the Unix endpoint bound constrains.
///
/// The whole assembled socket path has to fit `sun_path`, and macOS
/// canonicalizes `$TMPDIR` to a 56-byte private path that leaves no room for
/// it. `/tmp` canonicalizes into the same link-free tree in 12 bytes, so the
/// runtime lane is reserved there.
#[cfg(unix)]
fn runtime_base() -> std::io::Result<PathBuf> {
    fs::canonicalize("/tmp").or_else(|_| temporary_base())
}

/// Windows named-pipe endpoints carry no comparable path bound.
#[cfg(windows)]
fn runtime_base() -> std::io::Result<PathBuf> {
    temporary_base()
}

#[derive(Debug)]
struct Lane {
    root: TempDir,
    runtime: TempDir,
}

impl Lane {
    fn new() -> TestResult<Self> {
        let root = TempDir::new_in(temporary_base()?)?;
        let runtime = TempDir::new_in(runtime_base()?)?;
        Ok(Self { root, runtime })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    fn profile(&self) -> PathBuf {
        self.path("profile")
    }

    fn runtime(&self) -> PathBuf {
        self.runtime.path().to_path_buf()
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn read_stream(path: &Path) -> TestResult<String> {
    let mut contents = String::new();
    fs::File::open(path)?.read_to_string(&mut contents)?;
    Ok(contents)
}

/// A real foreground daemon, terminated when the guard is dropped.
#[derive(Debug)]
struct Daemon {
    child: Child,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl Daemon {
    /// Starts `academic daemon serve` and waits for its readiness line.
    fn start(lane: &Lane) -> TestResult<Self> {
        // The refusal a daemon prints before it can serve travels on standard
        // output, so discarding that stream turns a typed refusal into a bare
        // readiness timeout. It is captured and reported instead.
        let stdout_path = lane.path("daemon.stdout");
        let stderr_path = lane.path("daemon.stderr");
        let stdout = fs::File::create(&stdout_path)?;
        let stderr = fs::File::create(&stderr_path)?;
        let child = Command::new(binary())
            .args([
                "daemon",
                "serve",
                "--profile",
                &text(&lane.profile()),
                "--runtime",
                &text(&lane.runtime()),
                "--format",
                "json",
            ])
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()?;
        let daemon = Self {
            child,
            stdout_path,
            stderr_path,
        };
        daemon.wait_for_ready()?;
        Ok(daemon)
    }

    fn wait_for_ready(&self) -> TestResult<()> {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if self.stderr()?.contains("READY endpoint=") {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(format!(
            "the daemon never became ready; stdout was:\n{}\nstderr was:\n{}",
            read_stream(&self.stdout_path)?,
            self.stderr()?
        )
        .into())
    }

    fn stderr(&self) -> TestResult<String> {
        read_stream(&self.stderr_path)
    }

    /// Terminates the daemon abruptly, the way a fault point would.
    fn terminate(&mut self) -> TestResult<()> {
        self.child.kill()?;
        self.child.wait()?;
        Ok(())
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ignored = self.child.kill();
        let _ignored = self.child.wait();
    }
}

fn ingest(lane: &Lane, fixture: &str) -> TestResult<Output> {
    run(&[
        "ingest",
        "--profile",
        &text(&lane.profile()),
        "--runtime",
        &text(&lane.runtime()),
        "--fixture",
        fixture,
        "--format",
        "json",
    ])
}

/// Starts a daemon, accepts the committed fixture, and stops the daemon.
fn seeded_lane() -> TestResult<Lane> {
    let lane = Lane::new()?;
    let mut daemon = Daemon::start(&lane)?;
    let accepted = ingest(&lane, ALLOWLISTED_FIXTURE)?;
    assert_eq!(
        accepted.code,
        exit::OK,
        "seed ingest failed.
 stdout: {}
 daemon stderr: {}",
        accepted.stdout,
        daemon.stderr()?
    );
    daemon.terminate()?;
    Ok(lane)
}

// ---------------------------------------------------------------------------
// Named tests required by the Phase 1 execution contract
// ---------------------------------------------------------------------------

#[test]
fn cli_banner_precedes_human_output() -> TestResult {
    // Every human-readable path must open with the banner, including the paths
    // that fail. A caller can never see a result before the warning.
    let lane = Lane::new()?;
    let cases: Vec<Vec<String>> = vec![
        vec!["crash-replay".into(), "--all".into()],
        vec!["crash-replay".into(), "--fault".into(), "DB07".into()],
        vec!["doctor".into()],
        vec!["fixture".into(), "emit".into()],
        // Failing paths keep the ordering guarantee.
        vec![
            "ingest".into(),
            "--profile".into(),
            text(&lane.profile()),
            "--runtime".into(),
            text(&lane.runtime()),
            "--fixture".into(),
            "definitely-not-allowlisted".into(),
        ],
        vec![
            "daemon".into(),
            "status".into(),
            "--profile".into(),
            text(&lane.profile()),
            "--runtime".into(),
            text(&lane.runtime()),
        ],
    ];
    for case in &cases {
        let arguments = case.iter().map(String::as_str).collect::<Vec<_>>();
        let output = run(&arguments)?;
        let first = output
            .stdout
            .lines()
            .next()
            .ok_or_else(|| format!("{arguments:?} produced no standard output"))?;
        assert_eq!(first, BANNER, "{arguments:?} did not open with the banner");
    }
    Ok(())
}

#[test]
fn cli_json_always_contains_data_policy() -> TestResult {
    // Machine-readable output must carry the whole policy object on success and
    // on failure, and standard output must stay parseable as exactly one
    // document so the banner cannot be mistaken for content.
    let lane = Lane::new()?;
    let cases: Vec<Vec<String>> = vec![
        vec![
            "crash-replay".into(),
            "--all".into(),
            "--format".into(),
            "json".into(),
        ],
        vec!["doctor".into(), "--format".into(), "json".into()],
        vec![
            "ingest".into(),
            "--profile".into(),
            text(&lane.profile()),
            "--runtime".into(),
            text(&lane.runtime()),
            "--fixture".into(),
            "definitely-not-allowlisted".into(),
            "--format".into(),
            "json".into(),
        ],
        vec![
            "daemon".into(),
            "status".into(),
            "--profile".into(),
            text(&lane.profile()),
            "--runtime".into(),
            text(&lane.runtime()),
            "--format".into(),
            "json".into(),
        ],
        vec![
            "export".into(),
            "--profile".into(),
            text(&lane.profile()),
            "--destination".into(),
            text(&lane.path("export")),
            "--runtime".into(),
            text(&lane.runtime()),
            "--format".into(),
            "json".into(),
        ],
    ];
    for case in &cases {
        let arguments = case.iter().map(String::as_str).collect::<Vec<_>>();
        let output = run(&arguments)?;
        let document = output
            .json()
            .map_err(|error| format!("{arguments:?} did not emit one JSON document: {error}"))?;
        assert_eq!(
            document["policy"]["data_policy"], DATA_POLICY,
            "{arguments:?}"
        );
        assert_eq!(
            document["policy"]["storage_mode"],
            "PLAINTEXT_TEMPORARY_SQLITE"
        );
        assert_eq!(document["policy"]["storage_encryption"], "NONE");
        assert_eq!(
            document["policy"]["production_data_allowed"], false,
            "{arguments:?}"
        );
        assert_eq!(document["policy"]["product_network"], "NONE");
        assert_eq!(document["banner"], BANNER, "{arguments:?}");
        // The banner goes to standard error in JSON mode so it cannot corrupt
        // the document a caller parses.
        assert!(contains(BANNER).eval(&output.stderr), "{arguments:?}");
    }
    Ok(())
}

#[test]
fn cli_ingest_rejects_non_allowlisted_fixture() -> TestResult {
    // The refusal is a policy denial with its own exit code, and it happens
    // before any connection is opened, so it does not depend on a daemon.
    let lane = Lane::new()?;
    for candidate in [
        "definitely-not-allowlisted",
        "phase0-synthetic-bitemporal-ledger-v1",
        "../../etc/passwd",
        "",
    ] {
        let output = ingest(&lane, candidate)?;
        assert_eq!(
            output.code,
            exit::POLICY_DENIED,
            "{candidate:?} must be denied; stdout: {}",
            output.stdout
        );
        let document = output.json()?;
        assert_eq!(document["status"], "POLICY_DENIED");
        assert_eq!(document["error"]["reason"], "FIXTURE_NOT_ALLOWLISTED");
        assert_eq!(document["policy"]["production_data_allowed"], false);
    }
    Ok(())
}

#[test]
fn cli_ingest_returns_acceptance_receipt() -> TestResult {
    // A real daemon, a real acceptance, and a real immutable receipt. The
    // repeat proves the deterministic idempotency key returns the stored
    // receipt instead of accepting the batch twice.
    let lane = Lane::new()?;
    let mut daemon = Daemon::start(&lane)?;

    let first = ingest(&lane, ALLOWLISTED_FIXTURE)?;
    assert_eq!(first.code, exit::OK, "stderr: {}", first.stderr);
    let document = first.json()?;
    let acceptance = &document["result"]["acceptance"];
    assert_eq!(acceptance["status"], "ACCEPTED");
    assert_eq!(acceptance["acceptance_range"]["accept_seq_start"], 1);
    assert_eq!(
        acceptance["acceptance_range"]["accept_seq_end"],
        FIXTURE_EVENT_COUNT
    );
    let receipt_id = acceptance["receipt"]["receipt_id"]
        .as_str()
        .ok_or("the acceptance carried no receipt identifier")?
        .to_owned();
    assert!(!receipt_id.is_empty());
    assert_eq!(
        acceptance["response_digest"]
            .as_str()
            .map(str::len)
            .unwrap_or_default(),
        64,
        "the response digest must be a 32-byte SHA-256 value"
    );
    assert_eq!(document["result"]["transport"], "LOCAL_IPC");

    let repeat = ingest(&lane, ALLOWLISTED_FIXTURE)?;
    assert_eq!(repeat.code, exit::OK, "stderr: {}", repeat.stderr);
    let repeated = repeat.json()?;
    let repeated_acceptance = &repeated["result"]["acceptance"];
    assert_eq!(
        repeated_acceptance["status"], "DUPLICATE",
        "a retry must replay the stored receipt rather than accept again"
    );
    assert_eq!(repeated_acceptance["receipt"]["receipt_id"], receipt_id);

    daemon.terminate()?;
    Ok(())
}

#[test]
fn cli_daemon_status_reports_versions_and_watermarks() -> TestResult {
    let lane = Lane::new()?;
    let mut daemon = Daemon::start(&lane)?;

    let before = run(&[
        "daemon",
        "status",
        "--profile",
        &text(&lane.profile()),
        "--runtime",
        &text(&lane.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(before.code, exit::OK, "stderr: {}", before.stderr);
    let document = before.json()?;
    let handshake = &document["result"]["handshake"];
    assert_eq!(handshake["protocol_name"], "learning-platform.local-core");
    assert_eq!(handshake["protocol_version"]["major"], 1);
    assert_eq!(handshake["negotiated_protocol_version"]["major"], 1);
    assert_eq!(handshake["minimum_client_version"]["major"], 1);
    assert_eq!(handshake["storage_schema"]["number"], 1);
    assert_eq!(handshake["storage_schema"]["semantic_version"], "1.0.0");
    assert_eq!(handshake["vault_write_format"], "PLAINTEXT_SYNTHETIC_V1");
    assert_eq!(handshake["lock_state"], "UNLOCKED");
    assert!(
        handshake["daemon_build"]
            .as_str()
            .is_some_and(|build| build.starts_with("academicd/")),
        "the handshake must identify the daemon build"
    );
    assert_eq!(document["result"]["watermarks"]["accept_seq_head"], 0);
    assert_eq!(document["result"]["running"], true);

    // The watermarks must actually move with accepted canonical state.
    let accepted = ingest(&lane, ALLOWLISTED_FIXTURE)?;
    assert_eq!(accepted.code, exit::OK, "stderr: {}", accepted.stderr);
    let after = run(&[
        "daemon",
        "status",
        "--profile",
        &text(&lane.profile()),
        "--runtime",
        &text(&lane.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(after.code, exit::OK);
    let moved = after.json()?;
    assert_eq!(
        moved["result"]["watermarks"]["accept_seq_head"],
        FIXTURE_EVENT_COUNT
    );
    assert_eq!(moved["result"]["canonical"]["events"], FIXTURE_EVENT_COUNT);
    assert_eq!(moved["result"]["canonical"]["batches"], 1);
    assert!(
        moved["result"]["projections"]
            .as_array()
            .is_some_and(|projections| projections.len() == 3),
        "status must report every projection generation watermark"
    );

    daemon.terminate()?;

    // With the daemon gone the command reports an unavailable daemon rather
    // than pretending to have spoken to one.
    let stopped = run(&[
        "daemon",
        "status",
        "--profile",
        &text(&lane.profile()),
        "--runtime",
        &text(&lane.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(stopped.code, exit::UNAVAILABLE);
    // The daemon was killed abruptly, so it could not remove its own session
    // file. The profile is unowned either way; the reason says which it was.
    assert_eq!(stopped.json()?["error"]["reason"], "DAEMON_NOT_RUNNING");

    // A profile no daemon ever served reports the other reason.
    let untouched = Lane::new()?;
    let never = run(&[
        "daemon",
        "status",
        "--profile",
        &text(&untouched.profile()),
        "--runtime",
        &text(&untouched.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(never.code, exit::UNAVAILABLE);
    assert_eq!(never.json()?["error"]["reason"], "NO_DAEMON_OWNS_PROFILE");
    Ok(())
}

#[test]
fn cli_doctor_detects_orphan_temp_and_projection_lag() -> TestResult {
    let lane = seeded_lane()?;

    // The daemon does not run the projection consumer, so an accepted batch
    // leaves every generation behind the canonical outbox head. That lag is a
    // reportable condition, not a failure.
    let healthy = run(&[
        "doctor",
        "--profile",
        &text(&lane.profile()),
        "--deep",
        "--format",
        "json",
    ])?;
    // The doctor composes two independent axes into one exit code: the ambient
    // developer toolchain and profile health. A profile assertion can only be
    // read off the exit code on a host that satisfies the pinned baseline, so
    // that precondition is checked here rather than assumed.
    let report = healthy.json()?;
    assert_eq!(
        report["result"]["toolchain_ready"], true,
        "this host does not satisfy docs/development/bootstrap.md; checks were {}",
        report["result"]["checks"]
    );
    assert_eq!(healthy.code, exit::OK, "stderr: {}", healthy.stderr);
    let profile = &report["result"]["profile"];
    assert_eq!(profile["deep"], true);
    assert_eq!(profile["integrity_check"], true);
    assert_eq!(profile["foreign_key_check"], true);
    assert_eq!(profile["synthetic_marker_present"], true);
    let lag_findings = finding_codes(profile);
    assert!(
        lag_findings.contains("PROJECTION_LAG"),
        "deep doctor must report projection lag; findings were {lag_findings:?}"
    );
    assert!(
        profile["projections"]
            .as_array()
            .is_some_and(|projections| projections
                .iter()
                .all(|projection| projection["lag"].as_u64().unwrap_or(0) > 0)),
        "every generation should be behind the canonical outbox head here"
    );

    // Plant an unpublished ingest temp, which is exactly what a fault between
    // the stream write and the publish rename leaves behind.
    let temp = lane.profile().join("vault").join("tmp");
    fs::write(
        temp.join("1700000000000-00000000000000000000000000000000.partial"),
        b"unpublished synthetic ingest temp\n",
    )?;

    let damaged = run(&[
        "doctor",
        "--profile",
        &text(&lane.profile()),
        "--deep",
        "--format",
        "json",
    ])?;
    assert_eq!(
        damaged.code,
        exit::REPAIR_REQUIRED,
        "an orphan temp must demand repair; stdout: {}",
        damaged.stdout
    );
    let damaged_report = damaged.json()?;
    assert_eq!(damaged_report["status"], "REPAIR_REQUIRED");
    assert_eq!(damaged_report["error"]["reason"], "PROFILE_REPAIR_REQUIRED");
    let codes = finding_codes(&damaged_report["result"]["profile"]);
    assert!(
        codes.contains("ORPHAN_TEMP_PRESENT"),
        "findings were {codes:?}"
    );
    assert!(codes.contains("PROJECTION_LAG"), "findings were {codes:?}");
    assert_eq!(
        damaged_report["result"]["profile"]["orphan_temp_entries"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the vault directory barrier must never be counted as residue"
    );
    Ok(())
}

fn finding_codes(profile: &Value) -> BTreeSet<String> {
    profile["findings"]
        .as_array()
        .map(|findings| {
            findings
                .iter()
                .filter_map(|finding| finding["code"].as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn cli_export_is_deterministic() -> TestResult {
    let lane = seeded_lane()?;
    let mut digests = Vec::new();
    let mut inventories = Vec::new();
    for name in ["export-a", "export-b"] {
        let destination = lane.path(name);
        let output = run(&[
            "export",
            "--profile",
            &text(&lane.profile()),
            "--destination",
            &text(&destination),
            "--runtime",
            &text(&lane.runtime()),
            "--format",
            "json",
        ])?;
        assert_eq!(output.code, exit::OK, "stderr: {}", output.stderr);
        let document = output.json()?;
        assert_eq!(document["result"]["projections_included"], false);
        assert_eq!(document["result"]["encrypted"], false);
        assert_eq!(
            document["result"]["ownership"]["mode"], "OFFLINE",
            "a crashed daemon must not block a read-only export"
        );
        assert_eq!(
            document["result"]["ownership"]["stale_session_metadata"],
            true
        );
        digests.push((
            document["result"]["semantic_digest"].clone(),
            document["result"]["canonical_semantic_digest"].clone(),
        ));
        inventories.push(manifest_file_hashes(&destination)?);
    }
    assert_eq!(
        digests[0], digests[1],
        "two exports of one watermark must agree on both digests"
    );
    assert_eq!(
        inventories[0], inventories[1],
        "two exports of one watermark must have identical per-file hashes"
    );
    assert!(
        !inventories[0].is_empty(),
        "the export inventory must not be empty"
    );
    Ok(())
}

#[test]
fn posture_object_is_byte_exact_on_every_surface() -> TestResult {
    let expected = academic_admission::Posture::synthetic().canonical_json_bytes();
    let empty = tempfile::tempdir()?;
    let profile = text(empty.path());

    let shown = run(&[
        "admission",
        "show",
        "--profile",
        &profile,
        "--format",
        "json",
    ])?;
    assert_eq!(shown.code, exit::OK, "stderr: {}", shown.stderr);
    let expected_text = String::from_utf8(expected.clone())?;
    assert!(
        shown
            .stdout
            .contains(&format!("\"policy\":{expected_text}")),
        "CLI JSON did not embed the exact compact posture: {}",
        shown.stdout
    );

    let human = run(&["admission", "show", "--profile", &profile])?;
    assert!(
        human
            .stdout
            .lines()
            .any(|line| line == format!("posture: {expected_text}")),
        "human output did not carry the exact compact posture: {}",
        human.stdout
    );

    let client = academic_rpc::generated::ClientHandshake {
        protocol_name: academic_rpc::LOCAL_CORE_PROTOCOL_NAME.to_owned(),
        protocol_version: Some(academic_rpc::generated::ProtocolVersion { major: 1, minor: 0 }),
        capability_ids: vec!["learning-platform.local.diagnostics.v1".to_owned()],
    };
    let handshake = academic_rpc::negotiate_handshake(
        &client,
        &academic_rpc::ServerHandshakeConfig::default(),
    )?;
    let ipc = handshake.policy.ok_or("IPC posture was absent")?;
    assert_eq!(ipc.canonical_json, expected);

    let lane = seeded_lane()?;
    let destination = lane.path("posture-export");
    let exported = run(&[
        "export",
        "--profile",
        &text(&lane.profile()),
        "--destination",
        &text(&destination),
        "--runtime",
        &text(&lane.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(exported.code, exit::OK, "stderr: {}", exported.stderr);
    assert_eq!(fs::read(destination.join("posture.json"))?, expected);
    Ok(())
}

/// Reads the exported manifest's file inventory as `(path, sha256)` pairs.
fn manifest_file_hashes(export_root: &Path) -> TestResult<Vec<(String, String)>> {
    let manifest: Value = serde_json::from_slice(&fs::read(export_root.join("manifest.json"))?)?;
    let files = manifest["semantic"]["files"]
        .as_array()
        .ok_or("the export manifest carried no file inventory")?;
    let mut inventory = files
        .iter()
        .map(|entry| {
            (
                entry["path"].as_str().unwrap_or_default().to_owned(),
                entry["sha256"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    inventory.sort();
    Ok(inventory)
}

#[test]
fn cli_restore_requires_empty_profile() -> TestResult {
    let lane = seeded_lane()?;
    let backup = lane.path("backup");
    let created = run(&[
        "backup",
        "--profile",
        &text(&lane.profile()),
        "--destination",
        &text(&backup),
        "--runtime",
        &text(&lane.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(created.code, exit::OK, "stderr: {}", created.stderr);
    assert_eq!(created.json()?["result"]["encrypted"], false);

    // A destination that already holds something is refused as a conflict, and
    // the occupant is left exactly as it was.
    let occupied = lane.path("occupied");
    fs::create_dir(&occupied)?;
    fs::write(occupied.join("existing.txt"), b"pre-existing content\n")?;
    let refused = run(&[
        "restore",
        "--backup",
        &text(&backup),
        "--new-profile",
        &text(&occupied),
        "--runtime",
        &text(&lane.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(
        refused.code,
        exit::CONFLICT,
        "restore must refuse a non-empty destination; stdout: {}",
        refused.stdout
    );
    assert_eq!(refused.json()?["status"], "CONFLICT");
    assert_eq!(
        fs::read_to_string(occupied.join("existing.txt"))?,
        "pre-existing content\n",
        "a refused restore must not touch the destination"
    );

    // The same backup restores into a genuinely new destination.
    let restored = run(&[
        "restore",
        "--backup",
        &text(&backup),
        "--new-profile",
        &text(&lane.path("restored")),
        "--runtime",
        &text(&lane.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(
        restored.code,
        exit::OK,
        "stdout: {} stderr: {}",
        restored.stdout,
        restored.stderr
    );
    let document = restored.json()?;
    assert_eq!(document["result"]["mode"], "OFFLINE_NEW_EMPTY_PROFILE");
    assert_eq!(document["result"]["replay"]["verified_batches"], 1);
    assert_eq!(
        document["result"]["replay"]["verified_events"],
        FIXTURE_EVENT_COUNT
    );
    assert!(
        document["result"]["projections"]
            .as_array()
            .is_some_and(|projections| !projections.is_empty()
                && projections
                    .iter()
                    .all(|projection| projection["activated"] == true)),
        "a restore must rebuild and activate every generation from empty"
    );
    Ok(())
}

#[test]
fn cli_crash_replay_all_is_machine_readable() -> TestResult {
    let output = run(&["crash-replay", "--all", "--format", "json"])?;
    assert_eq!(output.code, exit::OK, "stderr: {}", output.stderr);
    let document = output.json()?;
    let faults = document["result"]["faults"]
        .as_array()
        .ok_or("crash-replay emitted no fault array")?;

    let identifiers = faults
        .iter()
        .filter_map(|fault| fault["id"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        identifiers,
        [
            "V01", "V02", "V03", "V04", "V05", "V06", "DB01", "DB02", "DB03", "DB04", "DB05",
            "DB06", "DB07", "PR01", "PR02", "PR03", "BK01", "BK02", "BK03", "BK04", "RS01", "RS02",
            "RS03", "RS04", "IPC01", "IPC02",
        ],
        "the reported matrix must be the enumerated Phase 1 fault contract"
    );
    assert_eq!(document["result"]["matrix_size"], 26);
    assert_eq!(document["result"]["fault_count"], 26);

    for fault in faults {
        let id = fault["id"].as_str().unwrap_or_default();
        assert!(
            !fault["owner"].as_str().unwrap_or_default().is_empty(),
            "{id}"
        );
        assert!(
            !fault["termination_point"]
                .as_str()
                .unwrap_or_default()
                .is_empty(),
            "{id}"
        );
        assert!(
            fault["required_restart_outcomes"]
                .as_array()
                .is_some_and(|outcomes| !outcomes.is_empty()),
            "{id}"
        );
        // A product build must never advertise an injectable crash switch.
        assert_eq!(fault["injectable_by_this_build"], false, "{id}");
    }
    assert_eq!(document["result"]["injection_available"], false);

    // One row can be selected, and an unknown identifier is incompatible
    // rather than silently empty.
    let single = run(&["crash-replay", "--fault", "db07", "--format", "json"])?;
    assert_eq!(single.code, exit::OK);
    assert_eq!(single.json()?["result"]["fault_count"], 1);
    let unknown = run(&["crash-replay", "--fault", "ZZ99", "--format", "json"])?;
    assert_eq!(unknown.code, exit::INCOMPATIBLE);
    assert_eq!(unknown.json()?["error"]["reason"], "UNKNOWN_FAULT_ID");
    Ok(())
}

#[test]
fn cli_has_no_real_data_override() -> TestResult {
    // 1. No source in the CLI or in the operational surface it composes may
    //    contain a switch that admits real data.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut scanned = 0_usize;
    for directory in [manifest.join("src"), manifest.join("../core/src")] {
        for file in rust_sources(&directory)? {
            let contents = product_source(&fs::read_to_string(&file)?);
            scanned += 1;
            for forbidden in [
                "allow-real-data",
                "allow_real_data",
                "ALLOW_REAL_DATA",
                "production_data_allowed: true",
                "production_data_allowed = true",
                "PRODUCTION_DATA_ALLOWED",
                "sqlcipher_key",
                "sqlcipher-key",
                "--no-banner",
                "suppress_banner",
            ] {
                assert!(
                    !contents.contains(forbidden),
                    "{} contains the forbidden token {forbidden}",
                    file.display()
                );
            }
        }
    }
    assert!(
        scanned >= 10,
        "the source scan covered only {scanned} files"
    );

    // 2. No environment variable moves the posture. The battery covers the
    //    plausible spellings a caller might reach for.
    let environment = [
        ("ACADEMIC_ALLOW_REAL_DATA", "1"),
        ("ACADEMIC_ALLOW_REAL_DATA", "true"),
        ("ACADEMIC_PRODUCTION", "1"),
        ("ACADEMIC_PRODUCTION_DATA_ALLOWED", "true"),
        ("ACADEMIC_DATA_POLICY", "PRODUCTION"),
        ("ACADEMIC_STORAGE_ENCRYPTION", "AES256"),
        ("ACADEMIC_STORAGE_MODE", "SQLCIPHER"),
        ("ACADEMIC_PRODUCT_NETWORK", "ENABLED"),
        ("ACADEMIC_SQLCIPHER_KEY", "hunter2"),
        ("ACADEMIC_UNSAFE", "1"),
        ("ACADEMIC_DEBUG", "1"),
        ("ACADEMIC_NO_BANNER", "1"),
    ];
    for pair in &environment {
        let output = run_with_env(
            &["crash-replay", "--all", "--format", "json"],
            std::slice::from_ref(pair),
        )?;
        assert_eq!(output.code, exit::OK);
        let document = output.json()?;
        assert_eq!(document["policy"]["data_policy"], DATA_POLICY, "{pair:?}");
        assert_eq!(
            document["policy"]["production_data_allowed"], false,
            "{pair:?}"
        );
        assert_eq!(document["policy"]["storage_encryption"], "NONE", "{pair:?}");
        assert_eq!(document["policy"]["product_network"], "NONE", "{pair:?}");
        assert_eq!(document["banner"], BANNER, "{pair:?}");
    }
    // The whole battery applied at once fares no better.
    let output = run_with_env(&["doctor", "--format", "json"], &environment)?;
    let document = output.json()?;
    assert_eq!(document["policy"]["production_data_allowed"], false);
    assert_eq!(document["policy"]["data_policy"], DATA_POLICY);

    // 3. No flag admits real data, disables the banner, or supplies a key.
    let lane = Lane::new()?;
    for arguments in [
        vec!["ingest", "--allow-real-data"],
        vec!["ingest", "--production"],
        vec!["ingest", "--real"],
        vec!["doctor", "--allow-real-data"],
        vec!["doctor", "--no-banner"],
        vec!["doctor", "--quiet"],
        vec!["export", "--allow-real-data"],
        vec!["backup", "--sqlcipher-key", "hunter2"],
        vec!["backup", "--key", "hunter2"],
        vec!["backup", "--encrypt"],
        vec!["restore", "--in-place"],
        vec!["crash-replay", "--inject", "DB07"],
        vec!["profile", "convert"],
        vec!["import", "--path", "."],
    ] {
        let output = run(&arguments)?;
        assert_eq!(
            output.code,
            exit::USAGE,
            "{arguments:?} must be rejected as a usage error, not accepted"
        );
    }

    // 4. Even the allowlisted-fixture flag cannot be pointed at a file, and a
    //    real-looking path is denied rather than read.
    for candidate in [
        "C:/Users/someone/transcript.pdf",
        "/home/someone/grades.csv",
        "./Cargo.toml",
    ] {
        let output = ingest(&lane, candidate)?;
        assert_eq!(output.code, exit::POLICY_DENIED, "{candidate}");
        assert_eq!(
            output.json()?["error"]["reason"],
            "FIXTURE_NOT_ALLOWLISTED",
            "{candidate}"
        );
    }
    Ok(())
}

/// Returns the source ahead of any `#[cfg(test)]` module.
///
/// A test module has to spell the forbidden options in order to assert that
/// they are rejected, so scanning it would flag exactly the code proving the
/// property holds. Only shipped code is scanned.
fn product_source(contents: &str) -> String {
    match contents.find("#[cfg(test)]") {
        Some(offset) => contents[..offset].to_owned(),
        None => contents.to_owned(),
    }
}

fn rust_sources(directory: &Path) -> TestResult<Vec<PathBuf>> {
    let mut sources = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|value| value == "rs") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    Ok(sources)
}

// ---------------------------------------------------------------------------
// Exit-code taxonomy
// ---------------------------------------------------------------------------

#[test]
fn cli_exit_codes_distinguish_every_failure_class() -> TestResult {
    // The contract is that a caller can branch on *why* a command failed. Each
    // class is produced by a real command, not asserted against a table.
    let lane = Lane::new()?;

    // POLICY_DENIED: the synthetic-only allowlist refused the input.
    assert_eq!(
        ingest(&lane, "definitely-not-allowlisted")?.code,
        exit::POLICY_DENIED
    );

    // UNAVAILABLE: no daemon owns the profile, so an IPC-only command stops.
    assert_eq!(ingest(&lane, ALLOWLISTED_FIXTURE)?.code, exit::UNAVAILABLE);

    // INCOMPATIBLE: the fault identifier is not in the enumerated matrix.
    assert_eq!(
        run(&["crash-replay", "--fault", "NOPE", "--format", "json"])?.code,
        exit::INCOMPATIBLE
    );

    // USAGE: clap owns exit code 2 and no outcome class may claim it.
    assert_eq!(run(&["doctor", "--allow-real-data"])?.code, exit::USAGE);

    // PATH_REJECTED: the location the caller named is refused, in both of the
    // ways it can be. These are decisions about a path, not faults, and a
    // caller that cannot tell them from INTERNAL cannot tell "fix the path"
    // from "file a bug".
    let occupied = lane.path("not-a-profile");
    fs::create_dir(&occupied)?;
    fs::write(occupied.join("stray.txt"), b"not profile content\n")?;
    let unsafe_path = run(&[
        "daemon",
        "serve",
        "--profile",
        &text(&occupied),
        "--runtime",
        &text(&lane.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(
        unsafe_path.code,
        exit::PATH_REJECTED,
        "stdout: {}",
        unsafe_path.stdout
    );

    let empty = lane.path("empty-directory");
    fs::create_dir(&empty)?;
    let not_a_profile = run(&["doctor", "--profile", &text(&empty), "--format", "json"])?;
    assert_eq!(
        not_a_profile.code,
        exit::PATH_REJECTED,
        "stdout: {}",
        not_a_profile.stdout
    );

    let seeded = seeded_lane()?;

    // CONFLICT: the destination already exists.
    let destination = seeded.path("export-conflict");
    fs::create_dir(&destination)?;
    let conflict = run(&[
        "export",
        "--profile",
        &text(&seeded.profile()),
        "--destination",
        &text(&destination),
        "--runtime",
        &text(&seeded.runtime()),
        "--format",
        "json",
    ])?;
    assert_eq!(conflict.code, exit::CONFLICT, "stdout: {}", conflict.stdout);

    // REPAIR_REQUIRED: a planted orphan temp demands repair.
    fs::write(
        seeded
            .profile()
            .join("vault")
            .join("tmp")
            .join("1700000000001-11111111111111111111111111111111.partial"),
        b"unpublished\n",
    )?;
    assert_eq!(
        run(&[
            "doctor",
            "--profile",
            &text(&seeded.profile()),
            "--deep",
            "--format",
            "json",
        ])?
        .code,
        exit::REPAIR_REQUIRED
    );

    // INTERNAL is reserved for faults none of the above describe, and must not
    // collide with any of them.
    for code in [
        exit::OK,
        exit::USAGE,
        exit::POLICY_DENIED,
        exit::CONFLICT,
        exit::REPAIR_REQUIRED,
        exit::INCOMPATIBLE,
        exit::UNAVAILABLE,
        exit::PATH_REJECTED,
    ] {
        assert_ne!(code, exit::INTERNAL);
    }
    Ok(())
}

#[test]
fn cli_accepts_forward_slash_paths_on_every_host() -> TestResult {
    // Regression guard. The durability layer addresses files through verbatim
    // `\\?\` paths on Windows, and Windows does not normalise a verbatim path:
    // a forward slash a caller typed would be read as a filename character and
    // every open below the profile would fail with ERROR_PATH_NOT_FOUND. The
    // CLI normalises at the argument boundary, so a forward-slash profile must
    // behave exactly like a native one.
    let lane = Lane::new()?;
    let profile = text(&lane.profile()).replace('\\', "/");
    let runtime = text(&lane.runtime()).replace('\\', "/");

    let stdout_path = lane.path("daemon.stdout");
    let stderr_path = lane.path("daemon.stderr");
    let mut child = Command::new(binary())
        .args([
            "daemon",
            "serve",
            "--profile",
            &profile,
            "--runtime",
            &runtime,
            "--format",
            "json",
        ])
        .stdout(Stdio::from(fs::File::create(&stdout_path)?))
        .stderr(Stdio::from(fs::File::create(&stderr_path)?))
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut ready = false;
    while Instant::now() < deadline {
        if fs::read_to_string(&stderr_path)?.contains("READY endpoint=") {
            ready = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        ready,
        "the daemon never became ready on a forward-slash path; stdout was: {}",
        read_stream(&stdout_path)?
    );

    let accepted = run(&[
        "ingest",
        "--profile",
        &profile,
        "--runtime",
        &runtime,
        "--fixture",
        ALLOWLISTED_FIXTURE,
        "--format",
        "json",
    ])?;
    let _ignored = child.kill();
    let _ignored = child.wait();

    assert_eq!(
        accepted.code,
        exit::OK,
        "a forward-slash profile path must accept an ingest; stdout: {}",
        accepted.stdout
    );
    assert_eq!(
        accepted.json()?["result"]["acceptance"]["status"],
        "ACCEPTED"
    );
    Ok(())
}
