//! The half of `P2-G4` that is true without an operating system.
//!
//! Everything here runs in `cargo test --workspace`: the descriptor's expiry
//! and single use, the closed capability set, the acceptance boundary, the
//! receipt binding, and three source scans over this crate's own text.
//!
//! The scans are here rather than in `tools/` because what they read is this
//! crate's structure — where `unsafe` may appear, which targets a default build
//! links, and that the sandbox is entered on the one path that runs a job.
//! [policy source scans](../../../docs/contracts/policy-source-scans.md) is
//! where they are registered.

use std::path::{Path, PathBuf};

use academic_worker::{
    AcceptError, CapabilityDescriptor, DescriptorError, DescriptorRegistry, JobCapability,
    JobCapabilitySet, JobId, LimitKind, ResourceLimits, ResourceReceipt, RunOutcome, StagedJobDirs,
    StagedOutput, StagingAuthority, WireDescriptor, sandbox::BackendId,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SECRET: [u8; 32] = [0x5a; 32];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

fn limits() -> ResourceLimits {
    ResourceLimits::new(2_000, 512 * 1024 * 1024, 10_000, 4_096)
}

fn dirs(root: &Path) -> StagedJobDirs {
    StagedJobDirs::new(root.join("in"), root.join("out"))
}

fn issue(
    registry: &mut DescriptorRegistry,
    root: &Path,
    capabilities: &[JobCapability],
) -> Result<CapabilityDescriptor, Box<dyn std::error::Error>> {
    registry
        .issue(
            JobId::new("job-1")?,
            JobCapabilitySet::new(capabilities.iter().copied()),
            &dirs(root),
            limits(),
            1_000,
            2_000,
        )
        .map_err(Into::into)
}

fn absolute_root() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("C:\\staged\\job-1")
    } else {
        PathBuf::from("/staged/job-1")
    }
}

fn completed_receipt(output_bytes: u64) -> ResourceReceipt {
    ResourceReceipt::new(
        BackendId::None,
        limits(),
        1,
        1,
        1,
        output_bytes,
        RunOutcome::Completed,
    )
}

#[test]
fn capability_expires_and_cannot_replay() -> TestResult {
    let root = absolute_root();
    let mut registry = DescriptorRegistry::new();
    let descriptor = issue(&mut registry, &root, &JobCapability::ALL)?;

    // One use before the expiry succeeds.
    registry.consume(&descriptor, 1_500)?;
    assert_eq!(registry.consumed_at(descriptor.job()), Some(1_500));

    // A second use of the same descriptor is refused as consumed.
    let replay = registry.consume(&descriptor, 1_600);
    assert!(
        matches!(replay, Err(DescriptorError::AlreadyConsumed { .. })),
        "a replayed descriptor was accepted: {replay:?}"
    );

    // A descriptor that was never used is still refused once its expiry passes,
    // and is reported as expired rather than as consumed.
    let mut second = DescriptorRegistry::new();
    let fresh = second.issue(
        JobId::new("job-2")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &dirs(&root),
        limits(),
        1_000,
        2_000,
    )?;
    let at_expiry = second.consume(&fresh, 2_000);
    assert!(
        matches!(at_expiry, Err(DescriptorError::Expired { .. })),
        "the expiry is inclusive, so a use at it must be refused: {at_expiry:?}"
    );
    let past_expiry = second.consume(&fresh, 9_999);
    assert!(matches!(past_expiry, Err(DescriptorError::Expired { .. })));

    // Re-encoding the descriptor with a later expiry is a digest mismatch
    // rather than a fresh grant: the registry, not the value, holds the truth.
    let mut forged_text = fresh.to_wire().as_str().to_owned();
    forged_text = forged_text.replace("expires_at=2000", "expires_at=999000");
    let forged = WireDescriptor::parse(&forged_text)?;
    assert_eq!(forged.expires_at(), 999_000);
    let forgery = second.consume(&forged, 1_500);
    assert!(
        matches!(forgery, Err(DescriptorError::DigestMismatch(_))),
        "a descriptor with an edited expiry was accepted: {forgery:?}"
    );
    Ok(())
}

#[test]
fn a_descriptor_round_trips_through_the_wire_form() -> TestResult {
    let root = absolute_root();
    let mut registry = DescriptorRegistry::new();
    let descriptor = issue(&mut registry, &root, &JobCapability::ALL)?;
    let parsed = WireDescriptor::parse(descriptor.to_wire().as_str())?;
    assert_eq!(parsed, descriptor);
    assert_eq!(parsed.digest(), descriptor.digest());

    // A key this build does not know is refused rather than ignored, so a
    // descriptor written by a newer parent is never read as a smaller one.
    let extended = format!("{}\nescalate=true\n", descriptor.to_wire().as_str());
    assert!(matches!(
        WireDescriptor::parse(&extended),
        Err(DescriptorError::MalformedWire(_))
    ));
    let unknown_capability = descriptor
        .to_wire()
        .as_str()
        .replace("capability=READ_STAGED_INPUT", "capability=CREATE_CLAIM");
    assert!(matches!(
        WireDescriptor::parse(&unknown_capability),
        Err(DescriptorError::UnknownCapability)
    ));
    Ok(())
}

#[test]
fn worker_cannot_publish_a_canonical_claim() -> TestResult {
    // (1) The capability set is closed and contains nothing that publishes.
    //     The witness `match` is compiler-checked: a variant added to
    //     `JobCapability` stops this file compiling.
    for capability in JobCapability::ALL {
        let spelled = match capability {
            JobCapability::ReadStagedInput => "READ_STAGED_INPUT",
            JobCapability::WriteStagedOutput => "WRITE_STAGED_OUTPUT",
        };
        assert_eq!(capability.as_str(), spelled);
    }
    assert_eq!(JobCapability::ALL.len(), 2);
    for name in [
        "CREATE_CLAIM",
        "PUBLISH_CLAIM",
        "WRITE_CANONICAL_STORE",
        "OPEN_OUTBOUND_SOCKET",
        "READ_KEY_MATERIAL",
    ] {
        assert!(
            matches!(
                JobCapability::parse(name),
                Err(DescriptorError::UnknownCapability)
            ),
            "{name} parsed as a job capability"
        );
    }

    // (2) Staged bytes are not a result until an authority the worker does not
    //     hold accepts them. The bytes here are a well-formed claim; nothing
    //     about their content earns them acceptance.
    let temporary = tempfile::tempdir()?;
    let staged = StagedJobDirs::create_under(temporary.path())?;
    let mut registry = DescriptorRegistry::new();
    let descriptor = registry.issue(
        JobId::new("claim-job")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &staged,
        limits(),
        1_000,
        2_000,
    )?;
    let claim = br#"{"kind":"CLAIM_PUBLISHED","subject":"synthetic"}"#;
    std::fs::write(staged.output().join("claim.json"), claim)?;
    let output = StagedOutput::read(&descriptor, Path::new("claim.json"))?;
    assert_eq!(output.len(), claim.len());

    let authority = StagingAuthority::from_secret(SECRET);
    let accepted =
        authority.accept(&descriptor, &completed_receipt(output.len() as u64), output)?;
    assert_eq!(accepted.bytes(), claim);

    // (3) A descriptor without the write capability could not have produced a
    //     staged output, so bytes offered under one are refused.
    let mut narrow_registry = DescriptorRegistry::new();
    let read_only = narrow_registry.issue(
        JobId::new("claim-job")?,
        JobCapabilitySet::new([JobCapability::ReadStagedInput]),
        &staged,
        limits(),
        1_000,
        2_000,
    )?;
    let again = StagedOutput::read(&read_only, Path::new("claim.json"))?;
    let refused = authority.accept(&read_only, &completed_receipt(claim.len() as u64), again);
    assert!(
        matches!(
            refused,
            Err(AcceptError::Descriptor(
                DescriptorError::CapabilityNotHeld { .. }
            ))
        ),
        "a read-only descriptor's staged bytes were accepted: {refused:?}"
    );
    Ok(())
}

#[test]
fn a_staged_path_cannot_climb_out_of_the_staged_output() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let staged = StagedJobDirs::create_under(temporary.path())?;
    let mut registry = DescriptorRegistry::new();
    let descriptor = registry.issue(
        JobId::new("escape")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &staged,
        limits(),
        1_000,
        2_000,
    )?;
    std::fs::write(temporary.path().join("outside.txt"), b"outside")?;
    let climbing = StagedOutput::read(&descriptor, Path::new("../outside.txt"));
    assert!(matches!(climbing, Err(AcceptError::EscapesStagedOutput(_))));
    let absolute = StagedOutput::read(&descriptor, &temporary.path().join("outside.txt"));
    assert!(matches!(absolute, Err(AcceptError::EscapesStagedOutput(_))));
    Ok(())
}

#[test]
fn the_wire_descriptor_carries_no_authority_secret() -> TestResult {
    let root = absolute_root();
    let mut registry = DescriptorRegistry::new();
    let descriptor = issue(&mut registry, &root, &JobCapability::ALL)?;
    let wire = descriptor.to_wire();
    let authority = StagingAuthority::from_secret(SECRET);

    // The literal secret, its hex spelling, and the authority identity derived
    // from it are all absent. The child gets paths, capabilities, bounds and
    // times, and nothing that would let it accept its own output.
    let hex: String = SECRET.iter().map(|byte| format!("{byte:02x}")).collect();
    for needle in [hex.as_str(), authority.identity().as_str()] {
        assert!(
            !wire.as_str().contains(needle),
            "the wire descriptor carries the authority secret"
        );
    }
    assert!(!wire.as_str().as_bytes().windows(32).any(|w| w == SECRET));

    // And what it does carry is exactly this list of keys, compared whole. A
    // field added to the descriptor has to be added here in the same commit,
    // which is the review that keeps a secret from becoming one of them.
    let keys: Vec<&str> = wire
        .as_str()
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.split_once('=').map(|(key, _)| key))
        .collect();
    assert_eq!(
        keys,
        vec![
            "job",
            "capability",
            "capability",
            "input",
            "output",
            "cpu_millis",
            "memory_bytes",
            "wall_millis",
            "output_bytes",
            "issued_at",
            "expires_at",
        ]
    );
    Ok(())
}

#[test]
fn resource_receipt_is_recorded_per_run() -> TestResult {
    // Every outcome carries a receipt, including the ones that produced
    // nothing. The type is what makes this true rather than a convention: there
    // is no constructor that takes an identifier without one.
    let outcomes = [
        RunOutcome::Completed,
        RunOutcome::KilledByLimit(LimitKind::Cpu),
        RunOutcome::KilledByLimit(LimitKind::Memory),
        RunOutcome::KilledByLimit(LimitKind::WallTime),
        RunOutcome::KilledByLimit(LimitKind::OutputBytes),
        RunOutcome::Failed { exit_code: 3 },
        RunOutcome::NotStarted {
            detail: String::from("no backend"),
        },
    ];
    assert_eq!(outcomes.len(), 3 + LimitKind::ALL.len());
    for outcome in outcomes {
        let acceptable = outcome.is_acceptable();
        let receipt = ResourceReceipt::new(BackendId::None, limits(), 7, 8, 9, 10, outcome.clone());
        let run = academic_worker::WorkerRun::new(model_run_id()?, receipt.clone());
        assert_eq!(run.receipt(), &receipt);
        assert_eq!(run.receipt().cpu_millis(), 7);
        assert_eq!(run.receipt().peak_memory_bytes(), 8);
        assert_eq!(run.receipt().wall_millis(), 9);
        assert_eq!(run.receipt().output_bytes(), 10);
        assert_eq!(run.receipt().backend(), BackendId::None);
        assert_eq!(acceptable, matches!(outcome, RunOutcome::Completed));
    }
    Ok(())
}

#[test]
fn pj01_a_killed_run_offers_no_staged_output() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let staged = StagedJobDirs::create_under(temporary.path())?;
    let mut registry = DescriptorRegistry::new();
    let descriptor = registry.issue(
        JobId::new("killed")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &staged,
        limits(),
        1_000,
        2_000,
    )?;
    std::fs::write(staged.output().join("partial.json"), b"{\"half\":true}")?;
    let output = StagedOutput::read(&descriptor, Path::new("partial.json"))?;
    let authority = StagingAuthority::from_secret(SECRET);

    for kind in LimitKind::ALL {
        let receipt = ResourceReceipt::new(
            BackendId::None,
            limits(),
            1,
            1,
            1,
            13,
            RunOutcome::KilledByLimit(kind),
        );
        let refused = authority.accept(&descriptor, &receipt, output.clone());
        assert!(
            matches!(refused, Err(AcceptError::RunNotCompleted { .. })),
            "a run killed for {} had its partial output accepted",
            kind.as_str()
        );
    }

    // And a completed run that still wrote past the bound is refused too, so
    // "completed" alone is not the whole gate.
    let over = ResourceReceipt::new(
        BackendId::None,
        limits(),
        1,
        1,
        1,
        limits().output_bytes() + 1,
        RunOutcome::Completed,
    );
    assert!(matches!(
        authority.accept(&descriptor, &over, output),
        Err(AcceptError::OverOutputBound { .. })
    ));
    Ok(())
}

#[test]
fn a_descriptor_refuses_a_relative_or_aliased_staged_directory() -> TestResult {
    let mut registry = DescriptorRegistry::new();
    let relative = registry.issue(
        JobId::new("relative")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &StagedJobDirs::new("in", "out"),
        limits(),
        1_000,
        2_000,
    );
    assert!(matches!(
        relative,
        Err(DescriptorError::RelativeStagedDirectory(_))
    ));
    let root = absolute_root();
    let aliased = registry.issue(
        JobId::new("aliased")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &StagedJobDirs::new(root.join("same"), root.join("same")),
        limits(),
        1_000,
        2_000,
    );
    assert!(matches!(
        aliased,
        Err(DescriptorError::StagedDirectoriesAlias)
    ));
    let backwards = registry.issue(
        JobId::new("backwards")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &dirs(&root),
        limits(),
        2_000,
        1_000,
    );
    assert!(matches!(
        backwards,
        Err(DescriptorError::ExpiryNotAfterIssue { .. })
    ));
    Ok(())
}

/// `unsafe` lives in the two sandbox backends and nowhere else in this crate.
///
/// The crate sets `unsafe_code = "deny"` rather than the workspace's `forbid`,
/// because a filter, a ruleset, a token and a job object are syscalls. This is
/// what keeps that relaxation from reaching anything else: the walk is
/// recursive over the whole of `src` and over `probes`, it counts what it read,
/// and the set of files permitted to hold an `unsafe` block is compared whole
/// rather than iterated, so a file that stops being read fails as a missing
/// entry.
#[test]
fn unsafe_is_confined_to_the_sandbox_backends() -> TestResult {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut with_unsafe = Vec::new();
    let mut scanned = 0_usize;
    for directory in ["src", "probes", "tests"] {
        for (path, source) in rust_sources(&crate_root.join(directory)) {
            scanned += 1;
            let relative = path
                .strip_prefix(&crate_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if names_unsafe(&code_only(&source)) {
                with_unsafe.push(relative);
            }
        }
    }
    assert!(
        scanned >= 8,
        "the walk found only {scanned} files, so it proved nothing"
    );
    with_unsafe.sort();
    assert_eq!(
        with_unsafe,
        vec!["src/sandbox/linux.rs", "src/sandbox/windows.rs"],
        "an `unsafe` block appeared outside the two sandbox backends"
    );
    Ok(())
}

/// The probe binaries are not in any default build.
///
/// The probe names a socket construct on purpose. That is only safe while it
/// cannot reach a default build, which is two facts read out of the manifest:
/// every `[[bin]]` outside `src` carries `required-features`, and the feature
/// it requires is not in `default`.
#[test]
fn probe_targets_are_not_in_any_default_build() -> TestResult {
    let manifest =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))?;
    let normalized = manifest.replace("\r\n", "\n");
    assert!(
        normalized.contains("default = []"),
        "the default feature set is no longer empty"
    );
    let mut binaries = 0_usize;
    for section in normalized.split("[[bin]]").skip(1) {
        binaries += 1;
        assert!(
            section.contains("path = \"probes/"),
            "a binary target was added outside probes/: {section}"
        );
        assert!(
            section.contains("required-features = [\"native-sandbox\"]"),
            "a probe binary is buildable without the native-sandbox feature: {section}"
        );
    }
    assert_eq!(binaries, 1, "the probe binary inventory changed");
    Ok(())
}

/// The sandbox is entered on the one path that runs a job, and only there.
///
/// A second call site, or a call site that a flag or an environment variable
/// could skip, is the shape this refuses. The probe's whole `run` function is
/// compared against a constant for the same reason
/// `crates/retention/tests/rotation_gate.rs` pins `require_rotation_accepted`:
/// a token list would admit every spelling nobody thought of.
#[test]
fn the_probe_enters_the_sandbox_before_it_reads_a_job() -> TestResult {
    let probe = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("probes/worker_probe.rs");
    let source = std::fs::read_to_string(&probe)?;
    let code = code_only(&source);
    // The identifier, not the path spelling. `sandbox::enter` is one way to
    // write this call; `use crate::sandbox::enter; enter(..)` is another, and a
    // count of the first sees neither a second call written the second way nor a
    // first call moved to it. `T146` reached a guarded function by that exact
    // substitution in `academic-untrusted-content`.
    assert_eq!(
        names_identifier(&code, "enter"),
        1,
        "the probe has more than one sandbox entry point"
    );
    let body = collapse(
        code.split("fn run(report: &mut String) -> Result<(), String> {")
            .nth(1)
            .and_then(|rest| rest.split("\nfn ").next())
            .ok_or("the probe has no run function")?,
    );
    assert_eq!(body, WHOLE_PROBE_RUN, "the probe's run path changed");
    let enter_at = body
        .find("sandbox::enter")
        .ok_or("the probe's run path has no sandbox entry")?;
    let job_at = body
        .find("JOB_FILE")
        .ok_or("the probe's run path never reads its job")?;
    assert!(
        enter_at < job_at,
        "the probe reads its job before it enters the sandbox"
    );
    Ok(())
}

/// The probe's whole contained path, whitespace-collapsed, comments dropped.
const WHOLE_PROBE_RUN: &str = "let input_dir = PathBuf::from(std::env::var(INPUT_DIR_VAR).map_err(|_| \
format!(\"\"))?); let report_dir = PathBuf::from( std::env::var(REPORT_DIR_VAR).map_err(|_| format!(\"\"))?, ); \
let wire = fs::read_to_string(input_dir.join(DESCRIPTOR_FILE)) .map_err(|error| format!(\"\"))?; \
let descriptor = WireDescriptor::parse(&wire).map_err(|error| format!(\"\"))?; \
let backend = sandbox::enter(&descriptor, &report_dir) .map_err(|error| format!(\"\"))?; \
let _ = writeln!(report, \"\"); let script = fs::read_to_string(input_dir.join(JOB_FILE)) \
.map_err(|error| format!(\"\"))?; let job = JobRequest::parse(&script).map_err(|error| format!(\"\"))?; \
for operation in job.operations() { let outcome = attempt(operation, &descriptor); \
let _ = writeln!(report, \"\", operation.to_line()); flush(&report_dir, report); } Ok(()) }";

/// Whether the stripped source declares an `unsafe` block or item.
///
/// A substring test is not enough: `with_unsafe = ` contains `unsafe ` and this
/// scan reported itself because of it. The preceding byte has to be something
/// that cannot continue an identifier.
/// Counts whole-identifier occurrences of `name` in already-stripped code.
///
/// The same boundary test as [`names_unsafe`], counting instead of answering
/// yes or no, so a call-site count reads the function's name rather than one
/// spelling of the path it is reached through.
fn names_identifier(code: &str, name: &str) -> usize {
    let bytes = code.as_bytes();
    code.match_indices(name)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + name.len()).copied().unwrap_or(b' ');
            before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
        })
        .count()
}

fn names_unsafe(code: &str) -> bool {
    let bytes = code.as_bytes();
    code.match_indices("unsafe").any(|(at, _)| {
        let before_ok =
            at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        let after = bytes.get(at + 6).copied().unwrap_or(b' ');
        before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
    })
}

/// Removes `//` and `/* */` comments and the contents of every literal.
///
/// The raw-string and character-literal arms are not decoration. A lexer that
/// does not model `r#"..."#` leaves the quote count odd from the first raw
/// string onward, and one that does not model `'"'` inverts it from that
/// character on; in both cases every later literal in the file is read as code
/// and every stretch of code as a literal. This scan reported an `unsafe` block
/// in a file whose only occurrence of the word is inside a string, twice, once
/// for each of those two omissions, before both arms existed.
fn code_only(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::new();
    let mut cursor = 0_usize;
    while cursor < bytes.len() {
        let two = &bytes[cursor..(cursor + 2).min(bytes.len())];
        if two == b"//" {
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            continue;
        }
        if two == b"/*" {
            cursor += 2;
            while cursor + 1 < bytes.len() && !(bytes[cursor] == b'*' && bytes[cursor + 1] == b'/')
            {
                cursor += 1;
            }
            cursor = (cursor + 2).min(bytes.len());
            out.push(' ');
            continue;
        }
        // A raw string: `r`, any run of `#`, a quote, then everything up to the
        // matching quote-and-run.
        if bytes[cursor] == b'r' {
            let mut hashes = 0_usize;
            while bytes.get(cursor + 1 + hashes) == Some(&b'#') {
                hashes += 1;
            }
            if bytes.get(cursor + 1 + hashes) == Some(&b'"') {
                let mut end = cursor + 2 + hashes;
                loop {
                    if end >= bytes.len() {
                        break;
                    }
                    if bytes[end] == b'"'
                        && bytes[end + 1..(end + 1 + hashes).min(bytes.len())]
                            .iter()
                            .filter(|byte| **byte == b'#')
                            .count()
                            == hashes
                    {
                        end += 1 + hashes;
                        break;
                    }
                    end += 1;
                }
                cursor = end.min(bytes.len());
                out.push_str("\"\"");
                continue;
            }
        }
        // A character literal, recognized before the quote arm below. A
        // lifetime is left alone: `'a` has no closing quote where one would be.
        if bytes[cursor] == b'\'' {
            let escaped = bytes.get(cursor + 1) == Some(&b'\\');
            let close = if escaped { cursor + 3 } else { cursor + 2 };
            if bytes.get(close) == Some(&b'\'') {
                out.push_str("''");
                cursor = close + 1;
                continue;
            }
        }
        if bytes[cursor] == b'"' {
            out.push_str("\"\"");
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor += 2;
                    continue;
                }
                if bytes[cursor] == b'"' {
                    cursor += 1;
                    break;
                }
                cursor += 1;
            }
            continue;
        }
        out.push(char::from(bytes[cursor]));
        cursor += 1;
    }
    out
}

fn collapse(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rust_sources(root: &Path) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && let Ok(source) = std::fs::read_to_string(&path)
            {
                found.push((path, source));
            }
        }
    }
    found.sort_by(|left, right| left.0.cmp(&right.0));
    found
}

/// A fixed v7 model-run identity, so the suite has no clock and no randomness.
fn model_run_id() -> Result<academic_domain::ModelRunId, Box<dyn std::error::Error>> {
    use std::str::FromStr as _;
    academic_domain::ModelRunId::from_str("018f2a3b-4c5d-7000-8000-000000000001")
        .map_err(Into::into)
}

#[test]
fn the_repository_root_is_where_this_crate_expects_it() -> TestResult {
    assert!(
        repository_root().join("Cargo.toml").is_file(),
        "the workspace manifest is not two directories above this crate"
    );
    Ok(())
}

/// `PJ02`, the half this task owns: nothing becomes a result by accident.
///
/// The fault table in the execution plan's section 7 assigns `PJ02` — "worker
/// output fails schema or provenance validation" — to `P2-G5`, while its
/// section 5 `P2-G4` row lists `PJ01` and `PJ02` as this task's faults. The two
/// readings are reconciled by splitting the fault: `P2-G5` owns *schema* and
/// *span provenance*, which are about the content of the bytes, and this task
/// owns the acceptance boundary those bytes have to cross first.
///
/// So this enumerates every way [`StagingAuthority::accept`] can refuse and
/// checks that each produces no `AcceptedOutput`. It is an inventory rather
/// than five separate assertions, because a sixth refusal added later without a
/// row here would leave a path this test never walked.
#[test]
fn pj02_output_that_fails_validation_is_quarantined_not_accepted() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let staged = StagedJobDirs::create_under(temporary.path())?;
    let mut registry = DescriptorRegistry::new();
    let descriptor = registry.issue(
        JobId::new("pj02")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &staged,
        limits(),
        1_000,
        2_000,
    )?;
    std::fs::write(staged.output().join("candidate.json"), b"{\"proposal\":1}")?;
    let output = StagedOutput::read(&descriptor, Path::new("candidate.json"))?;
    let authority = StagingAuthority::from_secret(SECRET);

    // 1. A path that leaves the staged output directory is never even read.
    assert!(matches!(
        StagedOutput::read(&descriptor, Path::new("../escaped.json")),
        Err(AcceptError::EscapesStagedOutput(_))
    ));
    // 2. A staged file that is not there.
    assert!(matches!(
        StagedOutput::read(&descriptor, Path::new("absent.json")),
        Err(AcceptError::Unreadable { .. })
    ));
    // 3. A run that did not complete.
    assert!(matches!(
        authority.accept(
            &descriptor,
            &ResourceReceipt::new(
                BackendId::None,
                limits(),
                1,
                1,
                1,
                14,
                RunOutcome::KilledByLimit(LimitKind::WallTime),
            ),
            output.clone(),
        ),
        Err(AcceptError::RunNotCompleted { .. })
    ));
    // 4. A run that wrote past its bound.
    assert!(matches!(
        authority.accept(
            &descriptor,
            &completed_receipt(limits().output_bytes() + 1),
            output.clone(),
        ),
        Err(AcceptError::OverOutputBound { .. })
    ));
    // 5. Bytes whose provenance is another job. This is the provenance half of
    //    `PJ02` that lives at this boundary: the staged output carries the job
    //    it came from, and a descriptor for a different job cannot accept it.
    let mut other_registry = DescriptorRegistry::new();
    let other = other_registry.issue(
        JobId::new("another-job")?,
        JobCapabilitySet::new(JobCapability::ALL),
        &staged,
        limits(),
        1_000,
        2_000,
    )?;
    assert!(matches!(
        authority.accept(&other, &completed_receipt(14), output.clone()),
        Err(AcceptError::JobMismatch { .. })
    ));
    // 6. A descriptor that never held the write capability.
    let mut narrow_registry = DescriptorRegistry::new();
    let narrow = narrow_registry.issue(
        JobId::new("pj02")?,
        JobCapabilitySet::new([JobCapability::ReadStagedInput]),
        &staged,
        limits(),
        1_000,
        2_000,
    )?;
    assert!(matches!(
        authority.accept(&narrow, &completed_receipt(14), output.clone()),
        Err(AcceptError::Descriptor(
            DescriptorError::CapabilityNotHeld { .. }
        ))
    ));

    // The same bytes, with everything right, are accepted — so the six refusals
    // above are the refusals and not a boundary that never accepts anything.
    assert_eq!(
        authority
            .accept(&descriptor, &completed_receipt(14), output)?
            .bytes(),
        b"{\"proposal\":1}"
    );
    Ok(())
}
