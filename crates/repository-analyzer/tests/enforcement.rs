//! The process-level half of `P2-RF21` for this binary.
//!
//! `academic-process-sandbox` measures its backend against a probe. This suite
//! measures the shipped executable: it launches it and reads the exit status,
//! because a declaration that is enforced in a probe and not in the binary
//! anybody runs is the defect `P2-A5` and `P2-A4` both reported.
//!
//! There are two outcomes and the contract admits no third. A build that has an
//! enforcement backend for this platform must exit `0` having written a receipt
//! that names the capabilities its class does not declare. A build that does
//! not must exit non-zero having written nothing to standard output, because a
//! process whose declaration is not enforced does not run.

use std::process::Command;

use academic_policy::{ProcessCapability, ProcessClass};

const BINARY: &str = env!("CARGO_BIN_EXE_academic-repository-analyzer");
const CLASS: ProcessClass = ProcessClass::RepositoryAnalyzer;

/// The two capabilities `academic-process-sandbox` enforces at the process
/// boundary, restated here so this suite's expectation does not come from the
/// crate it is checking.
const ENFORCED: [ProcessCapability; 2] = [
    ProcessCapability::WriteStagedArtifact,
    ProcessCapability::OpenOutboundSocket,
];

const ENFORCING_BUILD: bool = cfg!(all(feature = "native-enforcement", target_os = "linux"));

fn run() -> (Option<i32>, String, String) {
    let output = Command::new(BINARY).output();
    assert!(output.is_ok(), "{BINARY} could not be launched: {output:?}");
    let Ok(output) = output else {
        return (None, String::new(), String::new());
    };
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_binary_runs_only_while_its_declaration_is_enforced() {
    let (code, stdout, stderr) = run();
    if ENFORCING_BUILD {
        assert_eq!(code, Some(0), "the binary refused to start: {stderr}");
        assert!(
            stderr.is_empty(),
            "an enforced run wrote to standard error: {stderr}"
        );
        let expected: Vec<&str> = ENFORCED
            .into_iter()
            .filter(|capability| !CLASS.allows(*capability))
            .map(ProcessCapability::as_str)
            .collect();
        let expected = if expected.is_empty() {
            String::from("<none>")
        } else {
            expected.join(",")
        };
        assert!(
            stdout.starts_with(&format!(
                "{} enforced by LINUX_LANDLOCK_SECCOMP refusing [{expected}] verified by ",
                CLASS.as_str()
            )),
            "the receipt does not name this class's refusals: {stdout}"
        );
        assert!(
            stdout.contains("NoNewPrivs=1"),
            "the receipt carries no kernel answer: {stdout}"
        );
    } else {
        assert_eq!(
            code,
            Some(1),
            "the binary ran without an enforcement backend:
stdout: {stdout}
stderr: {stderr}"
        );
        assert!(
            stdout.is_empty(),
            "a refused run still wrote to standard output: {stdout}"
        );
        assert!(
            stderr.starts_with(&format!("{} refuses to start: ", CLASS.as_str())),
            "the refusal does not name this class: {stderr}"
        );
        assert!(
            stderr.contains("no enforcement backend:"),
            "the refusal does not say what is missing: {stderr}"
        );
    }
}

/// The whole of this crate's `src/main.rs`, comments and blank lines removed.
///
/// A pin, not two `contains`. `P2-A5`'s sixth audit put
/// `<str as ::std::net::ToSocketAddrs>::to_socket_addrs("example.invalid:80")`
/// above the sandbox entry in `academic-repository-analyzer`'s `main` and
/// measured the whole workspace, both hosts, reporting no difference at all.
/// The reason this suite did not see it is that it asked whether two
/// substrings were **present** and nothing about what else the file held.
///
/// The file now holds three items and no `fn main`:
/// `academic_process_sandbox::class_main!` is the whole of `main`, so a
/// statement above the entry has no position to occupy, and any line added to
/// this file at all fails here naming the file.
const MAIN_RS: [&str; 3] = [
    "use academic_policy::ProcessClass;",
    "const PROCESS_CLASS: ProcessClass = ProcessClass::RepositoryAnalyzer;",
    "academic_process_sandbox::class_main!(PROCESS_CLASS);",
];

#[test]
fn this_binary_is_bound_to_exactly_one_process_class() {
    // The binding is a `const` in `main.rs`; what this reads is that the class
    // named here is the one whose declaration the receipt above is checked
    // against, that it is one of the six, and that the file binds it and
    // enters and does nothing else.
    assert!(
        ProcessClass::ALL.contains(&CLASS),
        "{} is not one of the six process classes",
        CLASS.as_str()
    );
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("main.rs"),
    );
    assert!(source.is_ok(), "src/main.rs could not be read");
    let Ok(source) = source else {
        return;
    };
    let code: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .collect();
    assert_eq!(
        code, MAIN_RS,
        "src/main.rs is not the three items this binary is allowed to hold"
    );
}
