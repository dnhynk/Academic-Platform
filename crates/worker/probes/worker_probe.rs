//! The sandboxed process: the adversary, not the product.
//!
//! This binary is what runs *inside* the sandbox. It enters the sandbox before
//! it reads a job byte, then interprets the job script and attempts each
//! operation for real — including opening a socket, reading the home and vault
//! canaries, and creating a child process. What it writes into the report is
//! what the operating system answered.
//!
//! # Why it lives outside `src`
//!
//! Three of its operations name a socket construct, and a socket construct in
//! a workspace crate's `src` is what `phase1_exit_has_no_product_network`
//! refuses. Naming one is exactly this file's job, so it is a `[[bin]]` with an
//! explicit `path` outside `src`, gated on the non-default `native-sandbox`
//! feature, and it is registered in `SOCKET_ALLOWANCE` with exactly the
//! spellings below. `probes_are_not_in_any_default_build` is what keeps the
//! gate real; `only_egress_crate_has_a_socket` reads this file and compares its
//! whole spelling set against that table.
//!
//! This is a widening of that scan's allowance and it is deliberate: a test
//! that proves the operating system refuses a socket has to ask for one. What
//! keeps it scoped is that the allowance is exact, the target is not in any
//! default build, and no product crate depends on this binary.

use std::{
    fmt::Write as _,
    fs,
    io::Write as _,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use academic_worker::{
    JobOperation, JobRequest, WireDescriptor,
    capability::CapabilityDescriptor,
    sandbox::{
        self, DESCRIPTOR_FILE, HOME_CANARY_VAR, INPUT_DIR_VAR, JOB_FILE, REPORT_DIR_VAR,
        REPORT_FILE, VAULT_CANARY_VAR,
    },
};

/// A chunk big enough that a bounded address space refuses one quickly, and
/// small enough that an unbounded one does not swap the machine.
const MEMORY_CHUNK_BYTES: usize = 16 * 1024 * 1024;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let mut report = String::new();
    let code = match mode.as_str() {
        // The contained run. There is exactly one call site of
        // `sandbox::enter` in this file and it is on this path, before the job
        // script is read. No argument, variable, or file reaches past it.
        "run" => match run(&mut report) {
            Ok(()) => 0,
            Err(detail) => {
                let _ = writeln!(report, "fatal {detail}");
                2
            }
        },
        // The positive control. It installs no sandbox and it also runs no job
        // script: it attempts the refusable operations and reports what an
        // uncontained process on this machine gets for them. Without it, a
        // refusal observed inside the sandbox could be a refusal the machine
        // would have given anyway.
        "baseline" => {
            baseline(&mut report);
            0
        }
        other => {
            let _ = writeln!(report, "fatal unknown mode {other}");
            2
        }
    };
    if let Ok(directory) = std::env::var(REPORT_DIR_VAR) {
        flush(Path::new(&directory), &report);
    }
    std::process::exit(code);
}

/// What an uncontained process on this machine gets for each refusable
/// operation.
fn baseline(report: &mut String) {
    let _ = writeln!(report, "backend BASELINE_NONE");
    for operation in [
        JobOperation::ReadHome,
        JobOperation::ReadVault,
        JobOperation::OpenSocket,
        JobOperation::SpawnChild,
    ] {
        let outcome = attempt_unstaged(&operation)
            .unwrap_or_else(|| String::from("REFUSED -1 not a baseline operation"));
        let _ = writeln!(report, "op {} -> {outcome}", operation.to_line());
    }
}

/// The contained run: sandbox first, job second.
///
/// The order in this function is the whole claim. Nothing between the process
/// start and `sandbox::enter` reads a job byte, and nothing after it can
/// remove what `enter` installed.
fn run(report: &mut String) -> Result<(), String> {
    let input_dir =
        PathBuf::from(std::env::var(INPUT_DIR_VAR).map_err(|_| format!("{INPUT_DIR_VAR} unset"))?);
    let report_dir = PathBuf::from(
        std::env::var(REPORT_DIR_VAR).map_err(|_| format!("{REPORT_DIR_VAR} unset"))?,
    );
    let wire = fs::read_to_string(input_dir.join(DESCRIPTOR_FILE))
        .map_err(|error| format!("descriptor unreadable: {error}"))?;
    let descriptor =
        WireDescriptor::parse(&wire).map_err(|error| format!("descriptor invalid: {error}"))?;

    let backend = sandbox::enter(&descriptor, &report_dir)
        .map_err(|error| format!("sandbox refused to install: {error}"))?;
    let _ = writeln!(report, "backend {backend}");

    let script = fs::read_to_string(input_dir.join(JOB_FILE))
        .map_err(|error| format!("job script unreadable: {error}"))?;
    let job = JobRequest::parse(&script).map_err(|error| format!("job invalid: {error}"))?;

    for operation in job.operations() {
        let outcome = attempt(operation, &descriptor);
        let _ = writeln!(report, "op {} -> {outcome}", operation.to_line());
        // The report is rewritten after every operation, because a run killed
        // for a bound never reaches the end of this loop and its report has to
        // survive anyway.
        flush(&report_dir, report);
    }
    Ok(())
}

fn flush(report_dir: &Path, report: &str) {
    if let Ok(mut file) = fs::File::create(report_dir.join(REPORT_FILE)) {
        let _ = file.write_all(report.as_bytes());
        let _ = file.flush();
    }
}

fn refused(error: &std::io::Error, what: &str) -> String {
    format!(
        "REFUSED {} {what}: {error}",
        error.raw_os_error().unwrap_or(-1)
    )
}

/// The four operations that need no staged directory, so the positive control
/// can attempt exactly the same code as the contained run.
fn attempt_unstaged(operation: &JobOperation) -> Option<String> {
    match operation {
        JobOperation::ReadHome => Some(canary_read(HOME_CANARY_VAR, "home read")),
        JobOperation::ReadVault => Some(canary_read(VAULT_CANARY_VAR, "vault read")),
        JobOperation::OpenSocket => Some(open_socket()),
        JobOperation::SpawnChild => Some(spawn_child()),
        _ => None,
    }
}

/// Attempts to create a child process, and nothing else.
///
/// The three standard streams are deliberately inherited rather than redirected
/// to the null device. `Stdio::null()` makes the parent open `/dev/null`, and a
/// Landlock ruleset that grants only the staged directories refuses that with
/// `EACCES` *before* any process is created — so the operation came back
/// refused while `clone` was never reached, and this test passed for the wrong
/// reason until the redirection was removed. Inheriting an already-open
/// descriptor opens nothing, so what the refusal answers is process creation.
fn spawn_child() -> String {
    let current =
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("academic-worker-probe"));
    match std::process::Command::new(current)
        .arg("spawned-child")
        .spawn()
    {
        Ok(mut child) => {
            let _ = child.kill();
            let _ = child.wait();
            String::from("PERMITTED spawned a child process")
        }
        Err(error) => refused(&error, "child process"),
    }
}

#[allow(clippy::too_many_lines)]
fn attempt(operation: &JobOperation, descriptor: &CapabilityDescriptor) -> String {
    if let Some(answer) = attempt_unstaged(operation) {
        return answer;
    }
    match operation {
        JobOperation::ReadStagedInput { name } => {
            match fs::read(descriptor.staged_input().join(name)) {
                Ok(bytes) => format!("PERMITTED read {} bytes", bytes.len()),
                Err(error) => refused(&error, "staged input read"),
            }
        }
        JobOperation::WriteStagedOutput { name, bytes } => {
            let payload = vec![b'o'; usize::try_from(*bytes).unwrap_or(0)];
            match fs::write(descriptor.staged_output().join(name), &payload) {
                Ok(()) => format!("PERMITTED wrote {} bytes", payload.len()),
                Err(error) => refused(&error, "staged output write"),
            }
        }
        JobOperation::ReadHome
        | JobOperation::ReadVault
        | JobOperation::OpenSocket
        | JobOperation::SpawnChild => {
            String::from("REFUSED -1 unreached: attempt_unstaged answers these four")
        }
        JobOperation::WriteOutsideStagedOutput { path } => {
            match fs::write(path, b"academic-worker escaped the staged output directory") {
                Ok(()) => format!("PERMITTED wrote {}", path.display()),
                Err(error) => refused(&error, "write outside the staged output"),
            }
        }
        JobOperation::BurnCpu => {
            let mut accumulator = 0_u64;
            let mut spins = 0_u64;
            loop {
                accumulator = accumulator
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                spins = spins.wrapping_add(1);
                if spins.is_multiple_of(4_000_000) && accumulator == 1 {
                    // Unreachable in practice; it exists so the optimizer cannot
                    // delete the loop and so the loop has one exit.
                    return format!("PERMITTED burned {spins} spins");
                }
            }
        }
        JobOperation::ExhaustMemory => {
            let mut held: Vec<Vec<u8>> = Vec::new();
            let mut total = 0_usize;
            loop {
                let mut chunk: Vec<u8> = Vec::new();
                if chunk.try_reserve_exact(MEMORY_CHUNK_BYTES).is_err() {
                    return format!("REFUSED 12 allocation refused after {total} bytes");
                }
                chunk.resize(MEMORY_CHUNK_BYTES, 1);
                total = total.saturating_add(MEMORY_CHUNK_BYTES);
                held.push(chunk);
                if total > 64 * 1024 * 1024 * 1024 {
                    return format!("PERMITTED allocated {total} bytes");
                }
            }
        }
        JobOperation::SleepUntilKilled => {
            let started = Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(50));
                if started.elapsed() > Duration::from_secs(600) {
                    return String::from("PERMITTED slept without being killed");
                }
            }
        }
        JobOperation::OverrunOutput { name } => {
            let path = descriptor.staged_output().join(name);
            let bound = usize::try_from(descriptor.limits().output_bytes()).unwrap_or(usize::MAX);
            let payload = vec![b'x'; bound.saturating_mul(4).max(4)];
            match fs::write(&path, &payload) {
                Ok(()) => format!("PERMITTED wrote {} bytes past the bound", payload.len()),
                Err(error) => refused(&error, "output overrun"),
            }
        }
    }
}

fn canary_read(variable: &str, what: &str) -> String {
    let Ok(path) = std::env::var(variable) else {
        return format!("REFUSED -1 {what}: {variable} unset");
    };
    match fs::read(&path) {
        Ok(bytes) => format!("PERMITTED read {} canary bytes", bytes.len()),
        Err(error) => refused(&error, what),
    }
}

/// Creates a socket and attempts to use it.
///
/// Two attempts, and the first is the positive control.
///
/// A loopback listener plus a connect to the port it just chose is a complete
/// TCP round trip that needs no service, no route, and no network: an
/// uncontained process on any machine this suite runs on completes it. So a
/// contained process that cannot is different because of the sandbox and not
/// because of the machine. Without it, the only evidence would be a connect
/// failing to an address nothing answers, which fails everywhere.
///
/// The second is a documentation address from RFC 5737, and it is the
/// containment claim. The two backends refuse at different points: Linux
/// refuses `socket(2)` itself with `EPERM`, so the listener is never created
/// either; Windows creates the handle, permits a loopback connection to the
/// container's own endpoint, and refuses the off-host connect with `WSAEACCES`
/// — a permission answer rather than a routing one.
///
/// `PERMITTED` is therefore reserved for reaching the off-host address. A
/// loopback round trip that succeeds is reported in the detail and is not a
/// pass: it is the control, and on Windows it is also a measured limit of the
/// backend.
fn open_socket() -> String {
    let mut answers = Vec::new();
    let mut round_trip = false;
    match TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))) {
        Ok(listener) => match listener.local_addr() {
            Ok(address) => {
                answers.push(format!("listener={address}"));
                match TcpStream::connect_timeout(&address, Duration::from_millis(2_000)) {
                    Ok(_) => {
                        round_trip = true;
                        answers.push(String::from("loopback_round_trip=CONNECTED"));
                    }
                    Err(error) => answers.push(format!(
                        "loopback_round_trip={}",
                        error.raw_os_error().unwrap_or(-1)
                    )),
                }
            }
            Err(error) => answers.push(format!(
                "listener_addr={}",
                error.raw_os_error().unwrap_or(-1)
            )),
        },
        Err(error) => answers.push(format!("listener={}", error.raw_os_error().unwrap_or(-1))),
    }
    let documentation = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 80));
    let mut reached_off_host = false;
    match TcpStream::connect_timeout(&documentation, Duration::from_millis(700)) {
        Ok(_) => {
            reached_off_host = true;
            answers.push(String::from("documentation=CONNECTED"));
        }
        Err(error) => answers.push(format!(
            "documentation={}",
            error.raw_os_error().unwrap_or(-1)
        )),
    }
    let _ = round_trip;
    if reached_off_host {
        return format!(
            "PERMITTED socket reached an off-host address: {}",
            answers.join(" ")
        );
    }
    format!(
        "REFUSED -1 socket reached no off-host address: {}",
        answers.join(" ")
    )
}
