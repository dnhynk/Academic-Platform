//! What the operating system actually refuses.
//!
//! Every test here launches a real process, attempts a real syscall inside it,
//! and reads what the kernel answered. Nothing in this file is a source scan.
//!
//! # Every refusal is paired with a permission
//!
//! A refusal on its own proves nothing: a connect to an address the machine
//! cannot route fails whether or not there is a sandbox, and a file that does
//! not exist is unreadable to everybody. So each of the three containment tests
//! runs the *same* probe binary twice — once uncontained, through the probe's
//! `baseline` mode, and once inside the sandbox — and asserts that the
//! uncontained run was permitted and the contained one refused. The difference
//! between the two runs is the sandbox and nothing else.
//!
//! # `TcpStream` is not in this file
//!
//! The socket attempt lives in the probe, which is the process being contained.
//! This file only reads the answer.

// Two conditions, and both are load-bearing. The platform condition is
// obvious. The feature condition is not, and it was found by running the README
// block: `cargo test --workspace --all-targets` builds this target with default
// features, and without `native-sandbox` there is no backend — so every test
// here failed on a lane where nothing was owed. The feature is what compiles a
// sandbox, so it is also what decides whether this file has anything to say.
#![cfg(all(feature = "native-sandbox", any(target_os = "linux", windows)))]

use std::path::{Path, PathBuf};

use academic_worker::{
    DescriptorRegistry, JobCapability, JobCapabilitySet, JobId, JobOperation, JobPlan, JobRequest,
    LimitKind, OperationOutcome, ProbeReport, ResourceLimits, ResourceReceipt, RunOutcome,
    StagedJobDirs, StagedOutput, StagingAuthority,
    sandbox::{self, Availability, BackendId, LaunchSpec},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PROBE: &str = env!("CARGO_BIN_EXE_academic-worker-probe");
const SECRET: [u8; 32] = [0x5a; 32];
const HOME_CANARY_BYTES: &[u8] = b"SYNTHETIC-HOME-CANARY-P2-G4-0123456789";
const VAULT_CANARY_BYTES: &[u8] = b"SYNTHETIC-VAULT-CANARY-P2-G4-0123456789";

/// One job's world: a staged pair, a report directory, and the two canaries.
struct Harness {
    root: tempfile::TempDir,
    home_canary_dir: PathBuf,
    home_canary: PathBuf,
    vault_canary: PathBuf,
}

impl Harness {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let home = home_directory()?;
        let home_canary_dir = home.join(format!(".academic-worker-g4-{label}"));
        std::fs::create_dir_all(&home_canary_dir)?;
        let home_canary = home_canary_dir.join("home-canary.bin");
        std::fs::write(&home_canary, HOME_CANARY_BYTES)?;
        let vault_dir = root.path().join("vault");
        std::fs::create_dir_all(&vault_dir)?;
        let vault_canary = vault_dir.join("vault-canary.bin");
        std::fs::write(&vault_canary, VAULT_CANARY_BYTES)?;
        Ok(Self {
            root,
            home_canary_dir,
            home_canary,
            vault_canary,
        })
    }

    fn report_dir(&self) -> PathBuf {
        self.root.path().join("report")
    }

    fn staged(&self) -> Result<StagedJobDirs, Box<dyn std::error::Error>> {
        StagedJobDirs::create_under(self.root.path()).map_err(Into::into)
    }

    /// Runs the probe uncontained, so a refusal inside the sandbox has
    /// something to be different from.
    fn baseline(&self) -> Result<ProbeReport, Box<dyn std::error::Error>> {
        let report_dir = self.root.path().join("baseline-report");
        std::fs::create_dir_all(&report_dir)?;
        let status = std::process::Command::new(PROBE)
            .arg("baseline")
            .env(sandbox::REPORT_DIR_VAR, &report_dir)
            .env(sandbox::HOME_CANARY_VAR, &self.home_canary)
            .env(sandbox::VAULT_CANARY_VAR, &self.vault_canary)
            .stdin(std::process::Stdio::null())
            .status()?;
        assert!(status.success(), "the uncontained probe did not finish");
        Ok(sandbox::read_report(&report_dir))
    }

    fn run(
        &self,
        operations: Vec<JobOperation>,
        limits: ResourceLimits,
    ) -> Result<
        (
            ResourceReceipt,
            ProbeReport,
            academic_worker::CapabilityDescriptor,
        ),
        Box<dyn std::error::Error>,
    > {
        let staged = self.staged()?;
        let mut registry = DescriptorRegistry::new();
        let descriptor = registry.issue(
            JobId::new("contained")?,
            JobCapabilitySet::new(JobCapability::ALL),
            &staged,
            limits,
            1_000,
            2_000,
        )?;
        registry.consume(&descriptor, 1_500)?;
        let spec = LaunchSpec {
            program: PathBuf::from(PROBE),
            plan: JobPlan {
                descriptor: descriptor.clone(),
                request: JobRequest::new(operations),
                home_canary: self.home_canary.clone(),
                vault_canary: self.vault_canary.clone(),
            },
            report_dir: self.report_dir(),
        };
        let (receipt, report) = sandbox::launch(&spec)?;
        Ok((receipt, report, descriptor))
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home_canary_dir);
    }
}

fn home_directory() -> Result<PathBuf, Box<dyn std::error::Error>> {
    for variable in ["USERPROFILE", "HOME"] {
        if std::env::var(variable).is_ok_and(|value| !value.is_empty()) {
            return Ok(PathBuf::from(std::env::var(variable)?));
        }
    }
    Err("neither USERPROFILE nor HOME names a home directory".into())
}

fn generous() -> ResourceLimits {
    ResourceLimits::new(30_000, 1024 * 1024 * 1024, 60_000, 1024 * 1024)
}

/// The backend, or an error saying the claims in this file were not measured.
///
/// A backend this kernel cannot run is neither a pass nor a silent skip. §8.4
/// of the execution plan says a privileged negative test that cannot run is
/// recorded `NOT_RUN` with a reason and never coerced to `PASS`. The reason is
/// what this returns; the row is written in the task report, not by this file
/// deciding for itself that nothing was owed.
fn require_backend() -> Result<BackendId, Box<dyn std::error::Error>> {
    match sandbox::availability() {
        Availability::Available(backend) => Ok(backend),
        Availability::Unavailable(unavailable) => Err(format!(
            "the {} backend is unavailable on this kernel, so no containment claim in \
             this file was measured. Record it NOT_RUN with this reason rather than \
             reading the suite as green: {}",
            unavailable.backend, unavailable.reason
        )
        .into()),
    }
}

/// Turns a permitted outcome into the error that says so.
fn refusal_failure(what: &str, outcome: &OperationOutcome) -> Box<dyn std::error::Error> {
    format!("the contained run was allowed to {what}: {outcome}").into()
}

fn permitted(report: &ProbeReport, operation: &JobOperation, what: &str) {
    let outcome = report.outcome_of(operation);
    assert!(
        matches!(outcome, OperationOutcome::Permitted { .. }),
        "the uncontained baseline did not {what}, so the contained refusal is not \
         attributable to the sandbox: {outcome}"
    );
}

fn refused(report: &ProbeReport, operation: &JobOperation, what: &str) {
    let outcome = report.outcome_of(operation);
    assert!(
        outcome.is_refused(),
        "the contained run was allowed to {what}: {outcome}"
    );
}

/// Refused, *and* refused by the mechanism this backend is supposed to use.
///
/// A refusal with the wrong error number is a refusal for another reason, and
/// this suite has already had one: with the probe redirecting its streams to
/// the null device, `worker_cannot_spawn_a_child` passed on Linux with
/// `EACCES`, because Landlock refused the `/dev/null` open before `clone` was
/// ever reached. The code is what tells those two apart.
fn refused_with(
    report: &ProbeReport,
    operation: &JobOperation,
    expected: i64,
    what: &str,
) -> TestResult {
    let outcome = report.outcome_of(operation);
    let OperationOutcome::Refused { code, detail } = &outcome else {
        return Err(refusal_failure(what, &outcome));
    };
    assert_eq!(
        *code, expected,
        "the contained run was refused {what} with {code} rather than {expected},          so the refusal came from somewhere other than the backend this test          names: {detail}"
    );
    Ok(())
}

/// The error number each backend's refusal is supposed to carry.
const fn denied_code() -> i64 {
    if cfg!(target_os = "linux") {
        13 // EACCES, from a Landlock ruleset that does not cover the path.
    } else {
        5 // ERROR_ACCESS_DENIED: no ACE for the container SID.
    }
}

const fn no_child_code() -> i64 {
    if cfg!(target_os = "linux") {
        1 // EPERM, from the seccomp filter refusing clone/fork/vfork.
    } else {
        1816 // ERROR_NOT_ENOUGH_QUOTA, from the job's ActiveProcessLimit.
    }
}

#[test]
fn the_compiled_backend_is_the_one_this_platform_names() -> TestResult {
    let backend = require_backend()?;
    assert_eq!(backend, BackendId::compiled());
    if cfg!(target_os = "linux") {
        assert_eq!(backend, BackendId::LinuxSeccompLandlock);
    } else {
        assert_eq!(backend, BackendId::WindowsAppContainerJob);
    }
    println!("measured backend: {backend}");
    Ok(())
}

#[test]
fn worker_cannot_read_home_or_vault() -> TestResult {
    let backend = require_backend()?;
    let harness = Harness::new("read")?;
    let baseline = harness.baseline()?;
    permitted(&baseline, &JobOperation::ReadHome, "read the home canary");
    permitted(&baseline, &JobOperation::ReadVault, "read the vault canary");

    let (receipt, report, _) = harness.run(
        vec![JobOperation::ReadHome, JobOperation::ReadVault],
        generous(),
    )?;
    assert_eq!(receipt.backend(), backend);
    refused_with(
        &report,
        &JobOperation::ReadHome,
        denied_code(),
        "read the home canary",
    )?;
    refused_with(
        &report,
        &JobOperation::ReadVault,
        denied_code(),
        "read the vault canary",
    )?;
    println!(
        "home: {} | vault: {}",
        report.outcome_of(&JobOperation::ReadHome),
        report.outcome_of(&JobOperation::ReadVault)
    );
    Ok(())
}

/// What each backend refuses when a job asks for a socket, exactly.
///
/// The two backends do not refuse the same thing, and this test says which is
/// which rather than averaging them into one sentence.
///
/// Linux refuses `socket(2)`. Nothing is created, so the loopback listener the
/// probe tries first fails with `EPERM` and the off-host connect never has a
/// handle to use. That is the unqualified reading of this test's name.
///
/// Windows does not. `\Device\Afd` grants `ALL APPLICATION PACKAGES`, so the
/// handle is created; the platform's loopback exemption then lets an
/// AppContainer connect to an endpoint it owns itself, and the probe's
/// listener is one. What is refused is every address off this host, with
/// `WSAEACCES` — a permission answer no routing failure produces. So on
/// Windows the measured claim is "reaches no off-host address", and this test
/// asserts that and not more.
#[test]
fn worker_cannot_open_a_socket() -> TestResult {
    require_backend()?;
    let harness = Harness::new("socket")?;
    let baseline = harness.baseline()?;
    let uncontained = baseline.outcome_of(&JobOperation::OpenSocket);
    let uncontained_detail = format!("{uncontained}");
    assert!(
        uncontained_detail.contains("loopback_round_trip=CONNECTED"),
        "an uncontained process on this machine could not complete a loopback \
         round trip, so nothing below is attributable to the sandbox: \
         {uncontained_detail}"
    );

    let (_, report, _) = harness.run(vec![JobOperation::OpenSocket], generous())?;
    let contained = report.outcome_of(&JobOperation::OpenSocket);
    refused(
        &report,
        &JobOperation::OpenSocket,
        "reach an off-host address",
    );
    let OperationOutcome::Refused { detail, .. } = &contained else {
        return Err(refusal_failure("reach an off-host address", &contained));
    };

    if cfg!(target_os = "linux") {
        // `socket(2)` itself is refused, so the listener is never created and
        // the round trip the uncontained run completed does not happen.
        assert!(
            detail.contains("listener=1"),
            "the seccomp filter did not refuse socket(2) with EPERM: {detail}"
        );
        assert!(
            !detail.contains("loopback_round_trip=CONNECTED"),
            "a socket was created inside the Linux sandbox: {detail}"
        );
    } else {
        // The handle exists and loopback to the container's own endpoint works.
        // What is refused is everything off this host.
        assert!(
            detail.contains("documentation=10013"),
            "the off-host connect was not refused WSAEACCES, so the refusal is a \
             routing failure rather than the sandbox: {detail}"
        );
        assert!(
            detail.contains("loopback_round_trip=CONNECTED"),
            "the Windows backend refused loopback as well; this test records \
             that it does not, so update it and the contract together: {detail}"
        );
    }
    assert_ne!(
        uncontained_detail,
        format!("{contained}"),
        "the contained and uncontained socket answers are identical, so the \
         sandbox changed nothing"
    );
    println!("uncontained: {uncontained}\n  contained: {contained}");
    Ok(())
}

#[test]
fn worker_cannot_spawn_a_child() -> TestResult {
    require_backend()?;
    let harness = Harness::new("spawn")?;
    let baseline = harness.baseline()?;
    permitted(&baseline, &JobOperation::SpawnChild, "spawn a child");

    let (_, report, _) = harness.run(vec![JobOperation::SpawnChild], generous())?;
    refused_with(
        &report,
        &JobOperation::SpawnChild,
        no_child_code(),
        "spawn a child",
    )?;
    println!("spawn: {}", report.outcome_of(&JobOperation::SpawnChild));
    Ok(())
}

#[test]
fn worker_cannot_publish_a_canonical_claim() -> TestResult {
    require_backend()?;
    let harness = Harness::new("publish")?;
    let escape = harness.root.path().join("escaped-claim.json");
    let operations = vec![
        JobOperation::WriteStagedOutput {
            name: String::from("candidate.json"),
            bytes: 64,
        },
        JobOperation::WriteOutsideStagedOutput {
            path: escape.clone(),
        },
    ];
    let (receipt, report, descriptor) = harness.run(operations.clone(), generous())?;

    // The staged write is permitted, because that is what a job is for.
    permitted(
        &report,
        &operations[0],
        "write its own staged output directory",
    );
    // Writing anywhere else is refused by the operating system.
    refused_with(
        &report,
        &operations[1],
        denied_code(),
        "write outside its staged output",
    )?;
    assert!(
        !escape.exists(),
        "the contained job created a file outside its staged output directory"
    );

    // And the bytes it did stage are still not a result until the core accepts
    // them. The worker holds no `StagingAuthority`; the test process does.
    let staged = StagedOutput::read(&descriptor, Path::new("candidate.json"))?;
    let authority = StagingAuthority::from_secret(SECRET);
    let accepted = authority.accept(&descriptor, &receipt, staged)?;
    assert_eq!(accepted.bytes().len(), 64);
    Ok(())
}

#[test]
fn cpu_memory_time_output_limits_are_enforced() -> TestResult {
    require_backend()?;

    // CPU: a spin loop under a one-second bound is killed by the kernel.
    let cpu_harness = Harness::new("cpu")?;
    let (cpu, _, _) = cpu_harness.run(
        vec![JobOperation::BurnCpu],
        ResourceLimits::new(1_000, 1024 * 1024 * 1024, 120_000, 1024 * 1024),
    )?;
    assert_eq!(
        cpu.outcome(),
        &RunOutcome::KilledByLimit(LimitKind::Cpu),
        "a spin loop outlived its CPU bound: {cpu:?}"
    );

    // Wall time: a sleeping job is killed by the parent's deadline, and the
    // receipt says wall time rather than CPU, because it spent none.
    let wall_harness = Harness::new("wall")?;
    let (wall, _, _) = wall_harness.run(
        vec![JobOperation::SleepUntilKilled],
        ResourceLimits::new(60_000, 1024 * 1024 * 1024, 1_500, 1024 * 1024),
    )?;
    assert_eq!(
        wall.outcome(),
        &RunOutcome::KilledByLimit(LimitKind::WallTime),
        "a sleeping job outlived its wall bound: {wall:?}"
    );
    // The measurement and the kill decision come from the same clock on one
    // platform and from two clocks on the other, so the claim is not the same
    // on both.
    //
    // Linux compares `started.elapsed()` to the deadline and then records
    // `started.elapsed()` again, later. One `Instant`, sampled twice in order,
    // so the recorded value cannot be below the bound.
    //
    // Windows waits with `WaitForSingleObject(handle, wall)`, whose timeout is
    // counted in system timer ticks, and records `Instant`, which is the
    // performance counter. The wait can report `WAIT_TIMEOUT` before the
    // counter reaches the bound: measured on this repository's own x86-64 host,
    // one wait in twelve returned at 1499 ms for a 1500 ms bound, and the
    // `windows-11-arm` runner, whose tick is coarser, produced 1492. So what a
    // Windows receipt carries is a measurement at the bound within one system
    // timer tick — 15.625 ms at the default resolution — not at or past it.
    //
    // One tick is still far from what this assertion is for: a receipt that
    // claims a wall-time kill while the job barely ran.
    if cfg!(target_os = "linux") {
        assert!(
            wall.wall_millis() >= 1_500,
            "the wall measurement is below the bound it hit: {wall:?}"
        );
    } else {
        const TIMER_TICK_MILLIS: u64 = 16;
        assert!(
            wall.wall_millis() + TIMER_TICK_MILLIS >= 1_500,
            "the wall measurement is more than one system timer tick below the \
             bound it hit: {wall:?}"
        );
    }

    // Memory: the allocation the bound refuses comes back as an error rather
    // than as memory. Both backends refuse the mapping instead of killing, so
    // this is read from the operation and not from the outcome.
    let memory_harness = Harness::new("memory")?;
    let (memory, memory_report, _) = memory_harness.run(
        vec![JobOperation::ExhaustMemory],
        ResourceLimits::new(60_000, 256 * 1024 * 1024, 60_000, 1024 * 1024),
    )?;
    refused(
        &memory_report,
        &JobOperation::ExhaustMemory,
        "allocate past its memory bound",
    );
    assert!(
        memory.peak_memory_bytes() <= 4 * 256 * 1024 * 1024,
        "the run's peak memory is far past its bound: {memory:?}"
    );

    // Output: a job that writes past its staged-output bound produces a receipt
    // that says so, and its bytes are refused at the acceptance boundary.
    let output_harness = Harness::new("output")?;
    let bound = 1_024;
    let (output, _, descriptor) = output_harness.run(
        vec![JobOperation::OverrunOutput {
            name: String::from("flood.bin"),
        }],
        ResourceLimits::new(60_000, 1024 * 1024 * 1024, 60_000, bound),
    )?;
    assert_eq!(
        output.outcome(),
        &RunOutcome::KilledByLimit(LimitKind::OutputBytes),
        "a job wrote past its output bound and the receipt did not say so: {output:?}"
    );
    let staged = StagedOutput::read(&descriptor, Path::new("flood.bin"));
    if let Ok(staged) = staged {
        let authority = StagingAuthority::from_secret(SECRET);
        assert!(
            authority.accept(&descriptor, &output, staged).is_err(),
            "an over-bound staged output was accepted"
        );
    }
    println!(
        "cpu: {:?} | wall: {:?} | memory: {:?} | output: {:?}",
        cpu.outcome(),
        wall.outcome(),
        memory.outcome(),
        output.outcome()
    );
    Ok(())
}

#[test]
fn resource_receipt_is_recorded_per_run() -> TestResult {
    require_backend()?;
    let harness = Harness::new("receipt")?;
    let (completed, _, _) = harness.run(
        vec![JobOperation::WriteStagedOutput {
            name: String::from("out.bin"),
            bytes: 32,
        }],
        generous(),
    )?;
    assert_eq!(completed.outcome(), &RunOutcome::Completed);
    assert_eq!(completed.output_bytes(), 32);
    assert_eq!(completed.limits(), &generous());

    let killed_harness = Harness::new("receipt-killed")?;
    let (killed, _, _) = killed_harness.run(
        vec![JobOperation::SleepUntilKilled],
        ResourceLimits::new(60_000, 1024 * 1024 * 1024, 1_200, 1024),
    )?;
    assert!(matches!(
        killed.outcome(),
        RunOutcome::KilledByLimit(LimitKind::WallTime)
    ));

    // Both runs produced a receipt, and both receipts pair with a model run in
    // one value. There is no order of calls that produces the identity without
    // the measurement.
    use std::str::FromStr as _;
    for receipt in [completed, killed] {
        let run = academic_worker::WorkerRun::new(
            academic_domain::ModelRunId::from_str("018f2a3b-4c5d-7000-8000-000000000001")?,
            receipt.clone(),
        );
        assert_eq!(run.receipt(), &receipt);
        assert_eq!(run.receipt().backend(), BackendId::compiled());
    }
    Ok(())
}

/// Every operation a malicious plugin would attempt, in one job, at once.
///
/// The corpus is composed here from the job vocabulary rather than committed as
/// a fixture, so it is synthetic by construction and cannot drift from the
/// enum. `JobOperation::must_be_refused` is what says which entries are
/// adversarial, so an operation added to the enum has to be classified before
/// this test will say anything about it.
#[test]
fn malicious_plugin_corpus_is_contained() -> TestResult {
    require_backend()?;
    let harness = Harness::new("corpus")?;
    let escape_root = harness.root.path().join("escape");
    std::fs::create_dir_all(&escape_root)?;

    let corpus = vec![
        JobOperation::ReadStagedInput {
            name: String::from("job.txt"),
        },
        JobOperation::ReadHome,
        JobOperation::ReadVault,
        JobOperation::OpenSocket,
        JobOperation::SpawnChild,
        JobOperation::WriteOutsideStagedOutput {
            path: escape_root.join("escaped.bin"),
        },
        JobOperation::WriteOutsideStagedOutput {
            path: harness.home_canary.clone(),
        },
        JobOperation::WriteOutsideStagedOutput {
            path: harness.vault_canary.clone(),
        },
        JobOperation::WriteStagedOutput {
            name: String::from("result.bin"),
            bytes: 16,
        },
    ];
    let adversarial = corpus
        .iter()
        .filter(|operation| operation.must_be_refused())
        .count();
    assert_eq!(adversarial, 7, "the corpus lost an adversarial entry");

    let (receipt, report, _) = harness.run(corpus.clone(), generous())?;
    assert_eq!(receipt.outcome(), &RunOutcome::Completed);

    for operation in &corpus {
        let outcome = report.outcome_of(operation);
        if operation.must_be_refused() {
            assert!(
                outcome.is_refused(),
                "the corpus entry `{}` was permitted: {outcome}",
                operation.to_line()
            );
        } else {
            assert!(
                matches!(outcome, OperationOutcome::Permitted { .. }),
                "the corpus entry `{}` was refused, so the sandbox is too tight \
                 to run a job at all: {outcome}",
                operation.to_line()
            );
        }
    }

    // Nothing outside the staged output changed, and both canaries still hold
    // their original bytes.
    assert!(!escape_root.join("escaped.bin").exists());
    assert_eq!(std::fs::read(&harness.home_canary)?, HOME_CANARY_BYTES);
    assert_eq!(std::fs::read(&harness.vault_canary)?, VAULT_CANARY_BYTES);
    Ok(())
}
