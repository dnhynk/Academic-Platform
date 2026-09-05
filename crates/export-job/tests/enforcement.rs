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

const BINARY: &str = env!("CARGO_BIN_EXE_academic-export-job");
const CLASS: ProcessClass = ProcessClass::ExportJob;

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

#[test]
fn this_binary_is_bound_to_exactly_one_process_class() {
    // The binding is a `const` in `main.rs`; what this reads is that the class
    // named here is the one whose declaration the receipt above is checked
    // against, and that it is one of the six.
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
    assert!(
        source.contains("const PROCESS_CLASS: ProcessClass = ProcessClass::ExportJob;"),
        "src/main.rs no longer binds ExportJob"
    );
    assert!(
        source.contains("academic_process_sandbox::enter(PROCESS_CLASS)"),
        "src/main.rs no longer enters the process sandbox"
    );
}
