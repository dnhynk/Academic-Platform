//! T087 diagnostic. Temporary: removed before the fix is pushed.
//!
//! Prints the two axes the `doctor` exit code composes — ambient toolchain and
//! profile health — plus the runtime layout facts a hosted runner differs on.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use assert_cmd::cargo::cargo_bin;
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn binary() -> PathBuf {
    cargo_bin("academic")
}

fn probe(tool: &str) -> String {
    let version = Command::new(tool)
        .arg("--version")
        .output()
        .map(|output| {
            format!(
                "status={:?} stdout={:?} stderr={:?}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            )
        })
        .unwrap_or_else(|error| format!("spawn failed: {error}"));
    format!("  {tool}: {version}")
}

fn shell(command: &str) -> String {
    if cfg!(windows) {
        return "  (skipped on windows)".to_owned();
    }
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map(|output| {
            format!(
                "  $ {command}\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
        .unwrap_or_else(|error| format!("  $ {command} -> spawn failed: {error}"))
}

#[test]
fn t087_toolchain_axis() -> TestResult {
    let output = Command::new(binary())
        .args(["doctor", "--format", "json"])
        .output()?;
    println!(
        "T087 TOOLCHAIN AXIS\n\
         profile-less doctor exit: {:?}\n\
         stdout:\n{}\n\
         stderr:\n{}\n\
         ambient tools:\n{}\n{}\n{}\n{}\n\
         PATH={:?}\n",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        probe("rustc"),
        probe("cargo"),
        probe("node"),
        probe("pnpm"),
        std::env::var("PATH").unwrap_or_default()
    );
    Err("diagnostic only".into())
}

#[test]
fn t087_profile_axis() -> TestResult {
    let root = TempDir::new()?;
    let profile = root.path().join("profile");
    let runtime = root.path().join("runtime");
    fs::create_dir(&runtime)?;
    let stdout_path = root.path().join("daemon.stdout");
    let stderr_path = root.path().join("daemon.stderr");

    let mut child = Command::new(binary())
        .args([
            "daemon",
            "serve",
            "--profile",
            &profile.to_string_lossy(),
            "--runtime",
            &runtime.to_string_lossy(),
            "--format",
            "json",
        ])
        .stdout(Stdio::from(fs::File::create(&stdout_path)?))
        .stderr(Stdio::from(fs::File::create(&stderr_path)?))
        .spawn()?;

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut ready = false;
    while Instant::now() < deadline {
        if read(&stderr_path).contains("READY endpoint=") {
            ready = true;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let mut deep = String::from("(daemon never became ready; no seeded profile)");
    let mut ingest = String::from("(skipped)");
    if ready {
        let accepted = Command::new(binary())
            .args([
                "ingest",
                "--profile",
                &profile.to_string_lossy(),
                "--runtime",
                &runtime.to_string_lossy(),
                "--fixture",
                "phase0-synthetic-bitemporal-ledger-v2",
                "--format",
                "json",
            ])
            .output()?;
        ingest = format!(
            "exit={:?} stdout={}",
            accepted.status.code(),
            String::from_utf8_lossy(&accepted.stdout)
        );
        let _ignored = child.kill();
        let _ignored = child.wait();
        let doctor = Command::new(binary())
            .args([
                "doctor",
                "--profile",
                &profile.to_string_lossy(),
                "--deep",
                "--format",
                "json",
            ])
            .output()?;
        deep = format!(
            "exit={:?}\nstdout={}\nstderr={}",
            doctor.status.code(),
            String::from_utf8_lossy(&doctor.stdout),
            String::from_utf8_lossy(&doctor.stderr)
        );
    } else {
        let _ignored = child.kill();
        let _ignored = child.wait();
    }

    let temp_dir = std::env::temp_dir();
    println!(
        "T087 PROFILE AXIS\n\
         temp_dir={temp_dir:?}\n\
         canonical temp_dir={:?}\n\
         profile={profile:?} (len {})\n\
         runtime={runtime:?} (len {})\n\
         daemon ready={ready}\n\
         daemon stdout:\n{}\n\
         daemon stderr:\n{}\n\
         ingest: {ingest}\n\
         deep doctor: {deep}\n\
         mount facts:\n{}{}{}",
        fs::canonicalize(&temp_dir),
        profile.to_string_lossy().len(),
        runtime.to_string_lossy().len(),
        read(&stdout_path),
        read(&stderr_path),
        shell(&format!(
            "df -T {} 2>&1 || df {}",
            quoted(&runtime),
            quoted(&runtime)
        )),
        shell(&format!(
            "stat -f -c '%T %i' {} 2>&1 || stat -f {}",
            quoted(&runtime),
            quoted(&runtime)
        )),
        shell("mount | head -20"),
    );
    Err("diagnostic only".into())
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy())
}

fn read(path: &Path) -> String {
    let mut contents = String::new();
    if let Ok(mut file) = fs::File::open(path) {
        let _ignored = file.read_to_string(&mut contents);
    }
    contents
}
