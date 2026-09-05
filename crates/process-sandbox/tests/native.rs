//! The measurement: one process per class, and what the operating system let
//! it do.
//!
//! `enter` is irreversible and applies to the process that calls it, so this
//! suite cannot call it. It launches the probe binary instead, once per
//! [`ProcessClass`], and reads back what the probe's own attempts returned.
//! Every expectation is derived from `ProcessClass::capabilities()` — the
//! declaration — and compared against an outcome the kernel produced, so the
//! two sides of "declared equals enforced" come from two different places.
//!
//! On a platform with no backend the expectation is the other half of the same
//! contract: the probe refuses to start and reports that it attempted nothing.

#![cfg(feature = "native-enforcement")]

use std::collections::BTreeMap;

use academic_policy::{ProcessCapability, ProcessClass};

const PROBE: &str = env!("CARGO_BIN_EXE_academic-process-sandbox-probe");

/// `EPERM`, which is what the seccomp filter returns.
const EPERM: i32 = 1;
/// `EACCES`, which is what a Landlock refusal returns.
const EACCES: i32 = 13;

struct ProbeRun {
    code: Option<i32>,
    outcomes: BTreeMap<String, String>,
    stdout: String,
}

impl ProbeRun {
    fn outcome(&self, operation: &str) -> &str {
        self.outcomes
            .get(operation)
            .map_or("<absent>", String::as_str)
    }

    /// `Some(errno)` when the operation was refused with one.
    fn refusal_errno(&self, operation: &str) -> Option<i32> {
        let outcome = self.outcome(operation);
        let rest = outcome.strip_prefix("REFUSED errno=")?;
        let digits: String = rest
            .chars()
            .take_while(|character| character.is_ascii_digit() || *character == '-')
            .collect();
        digits.parse().ok()
    }

    fn succeeded(&self, operation: &str) -> bool {
        self.outcome(operation).starts_with("SUCCEEDED")
    }
}

/// One scratch directory per call, not per class: these tests run in parallel
/// threads of one process, so a name built from the process id alone had three
/// of them sharing a directory and deleting each other's.
static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn run(class: ProcessClass) -> ProbeRun {
    let serial = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let scratch = std::env::temp_dir().join(format!(
        "t215-probe-{}-{}-{serial}",
        class.as_str(),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    let created = std::fs::create_dir_all(&scratch);
    assert!(created.is_ok(), "the scratch directory could not be made");
    let output = std::process::Command::new(PROBE)
        .arg(class.as_str())
        .arg(&scratch)
        .output();
    // Reported rather than swallowed: a probe that cannot be launched is not a
    // probe that observed a refusal.
    assert!(
        output.is_ok(),
        "the probe at {PROBE} could not be launched: {output:?}"
    );
    let Ok(output) = output else {
        return ProbeRun {
            code: None,
            outcomes: BTreeMap::new(),
            stdout: String::new(),
        };
    };
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut outcomes = BTreeMap::new();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("PROBE ")
            && let Some((operation, outcome)) = rest.split_once(" = ")
        {
            outcomes.insert(operation.trim().to_owned(), outcome.trim().to_owned());
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    ProbeRun {
        code: output.status.code(),
        outcomes,
        stdout,
    }
}

#[test]
fn every_class_holds_the_socket_it_declares_and_no_other() {
    for class in ProcessClass::ALL {
        let run = run(class);
        let declared = class.allows(ProcessCapability::OpenOutboundSocket);
        if !cfg!(target_os = "linux") {
            assert_eq!(
                run.code,
                Some(3),
                "{} did not refuse to start on a platform with no backend:\n{}",
                class.as_str(),
                run.stdout
            );
            continue;
        }
        assert_eq!(
            run.code,
            Some(0),
            "{} did not enter:\n{}",
            class.as_str(),
            run.stdout
        );
        assert_eq!(
            run.succeeded("net/listen"),
            declared,
            "{} declares OpenOutboundSocket = {declared} and net/listen was {}",
            class.as_str(),
            run.outcome("net/listen")
        );
        assert_eq!(
            run.succeeded("net/connect-loopback"),
            declared,
            "{} declares OpenOutboundSocket = {declared} and net/connect-loopback was {}",
            class.as_str(),
            run.outcome("net/connect-loopback")
        );
        if !declared {
            // The refusal has to be the filter's, not the host's: a loopback
            // bind that failed for any other reason would satisfy a bare
            // "did not succeed" check while proving nothing.
            assert_eq!(
                run.refusal_errno("net/listen"),
                Some(EPERM),
                "{}'s net/listen refusal is not the seccomp filter's: {}",
                class.as_str(),
                run.outcome("net/listen")
            );
            assert_eq!(
                run.refusal_errno("net/connect-loopback"),
                Some(EPERM),
                "{}'s net/connect-loopback refusal is not the seccomp filter's: {}",
                class.as_str(),
                run.outcome("net/connect-loopback")
            );
        }
    }
}

#[test]
fn every_class_holds_the_write_it_declares_and_no_other() {
    for class in ProcessClass::ALL {
        let run = run(class);
        let declared = class.allows(ProcessCapability::WriteStagedArtifact);
        if !cfg!(target_os = "linux") {
            assert_eq!(
                run.code,
                Some(3),
                "{} did not refuse to start on a platform with no backend:\n{}",
                class.as_str(),
                run.stdout
            );
            continue;
        }
        for operation in ["fs/write-new", "fs/append-existing"] {
            assert_eq!(
                run.succeeded(operation),
                declared,
                "{} declares WriteStagedArtifact = {declared} and {operation} was {}",
                class.as_str(),
                run.outcome(operation)
            );
            if !declared {
                assert_eq!(
                    run.refusal_errno(operation),
                    Some(EACCES),
                    "{}'s {operation} refusal is not the Landlock ruleset's: {}",
                    class.as_str(),
                    run.outcome(operation)
                );
            }
        }
    }
}

#[test]
fn no_class_reaches_the_second_abi_under_the_same_arch_token() {
    // `P2-A5` measured a `REPOSITORY_ANALYZER` process that printed
    // `Seccomp=2` and then completed a TCP handshake through the x32 ABI: the
    // filter checked the arch token, and `AUDIT_ARCH_X86_64` is the token for
    // two ABIs. What closes it is a rule in the filter, and what proves the
    // rule is in force is a syscall on that ABI.
    //
    // The syscall is made inside `enter`, not here and not in the probe: this
    // process cannot make it (an `enter` is irreversible and applies to the
    // caller) and the probe may hold no `unsafe`. So `enter` makes it, requires
    // `EPERM`, and puts *the answer* in the receipt — `-1` being `-EPERM` —
    // which means a run that reached this assertion at all is a run in which
    // the syscall was made and the kernel refused it. A filter that stopped
    // covering x32 does not print a different receipt here; it fails `enter`,
    // and the probe exits 3. A check that made the call and dropped its result
    // would print the pid it got instead, which is why the number is read and
    // not a word saying it was asked.
    let expects_answer = cfg!(target_arch = "x86_64");
    for class in ProcessClass::ALL {
        let run = run(class);
        if !cfg!(target_os = "linux") {
            assert_eq!(run.code, Some(3), "{} entered", class.as_str());
            continue;
        }
        assert_eq!(
            run.code,
            Some(0),
            "{} did not enter, so the x32 refusal was not confirmed:\n{}",
            class.as_str(),
            run.stdout
        );
        // Both directions from the declaration: the classes whose socket is
        // refused carry the answer, and the one that declares a socket has no
        // filter to answer with.
        let filtered = !class.allows(ProcessCapability::OpenOutboundSocket);
        assert_eq!(
            run.outcome("enter").contains("x32(getpid)=-1"),
            filtered && expects_answer,
            "{} declares OpenOutboundSocket = {} and its receipt is {}",
            class.as_str(),
            !filtered,
            run.outcome("enter")
        );
    }
}

#[test]
fn the_kernel_reports_the_filter_the_receipt_claims() {
    // The receipt line is this crate talking. `/proc/self/status` is the
    // kernel talking, read by the probe after the installation and printed
    // beside it. A run whose receipt says a filter is installed and whose
    // status line says `Seccomp: 0` fails here.
    for class in ProcessClass::ALL {
        let run = run(class);
        if !cfg!(target_os = "linux") {
            assert_eq!(run.code, Some(3), "{} entered", class.as_str());
            assert_eq!(
                run.outcome("operations"),
                "NOT_ATTEMPTED",
                "{} attempted an operation after refusing to start",
                class.as_str()
            );
            continue;
        }
        let filtered = !class.allows(ProcessCapability::OpenOutboundSocket);
        let seccomp = run
            .stdout
            .lines()
            .find(|line| line.starts_with("PROBE proc/Seccomp:"))
            .unwrap_or("<absent>")
            .to_owned();
        assert_eq!(
            seccomp.contains(" 2"),
            filtered,
            "{} expected a seccomp filter = {filtered} and the kernel said {seccomp:?}",
            class.as_str()
        );
        assert!(
            run.outcome("enter").starts_with("OK"),
            "{} did not enter: {}",
            class.as_str(),
            run.outcome("enter")
        );
        assert!(
            run.outcome("enter").contains("NoNewPrivs=1"),
            "{}'s receipt does not carry the kernel's NoNewPrivs answer: {}",
            class.as_str(),
            run.outcome("enter")
        );
    }
}
