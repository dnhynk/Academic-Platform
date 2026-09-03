//! `no_device_handle_without_token`, measured rather than scanned.
//!
//! Every row here launches a process, asks the operating system for a device
//! handle inside it, and reads the answer. None of it is a source scan, and
//! none of it records: the probe opens a handle and drops it.
//!
//! # Every refusal is paired with a permission
//!
//! A refusal on its own is not evidence -- a path that does not exist is
//! unopenable to everybody, and a device node the user is not in the group for
//! is refused with or without a ruleset. So every contained row runs the same
//! probe with the same targets uncontained first, and requires the uncontained
//! run to have been *permitted* what the contained one is refused. A row whose
//! baseline is refused is reported `NOT_RUN` with that as its reason, per
//! section 8.4 of the execution plan, and never coerced to a pass.
//!
//! # What is claimed per platform
//!
//! Linux and Windows do not refuse the same thing and the suite does not
//! pretend they do. Linux splits by media, because a Landlock rule is added per
//! granted device tree; Windows refuses every class, because a device object's
//! DACL is not the caller's to widen. Each assertion below is guarded on its
//! own platform and the contract page carries both rows.

// There is a backend for two targets. On any other, this crate compiles to
// bookkeeping whether or not the feature is on, and there is nothing here to
// ask an operating system -- `a_lane_with_no_backend_reports_bookkeeping` in
// `tests/capture.rs` is what runs there instead. `academic-worker`s
// containment suite is gated the same way and for the same reason.
#![cfg(all(feature = "native-capture", any(target_os = "linux", windows)))]

mod common;

#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;

use academic_capture_gate::{
    BackendId, CaptureAudit, DeviceClass, DeviceLayer, DeviceRuleset, authorize,
    native::{self, DeviceTree, LaunchSpec},
};
use academic_consent::CaptureMedium;

use common::{INSIDE, TERM_TO, TestResult, ledger_granting, request_for};

const PROBE: &str = env!("CARGO_BIN_EXE_academic-capture-probe");

/// The stand-in used when this host exposes no real node for a class.
///
/// Both are real character device nodes an uncontained process opens, and
/// neither is a capture device. A row measured on one of them says exactly that
/// -- what it establishes is that the ruleset refuses a device node by path and
/// admits one the token names, not that a microphone was present.
/// `the_measured_device_nodes_are_reported` prints which path each row used.
#[cfg(target_os = "linux")]
const LINUX_STAND_IN_GRANTED: &str = "/dev/urandom";
#[cfg(target_os = "linux")]
const LINUX_STAND_IN_UNGRANTED: &str = "/dev/null";

/// The path this host exposes for `class`, or the stand-in.
///
/// A real device path is preferred, and only one whose *baseline* open is
/// permitted: `/dev/snd/timer` on this repository's measuring host is `EACCES`
/// to a user outside the `audio` group with or without a ruleset, so a row
/// measured on it would be a refusal with no paired permission.
#[cfg(target_os = "linux")]
fn linux_path_for(class: DeviceClass, stand_in: &str) -> String {
    native::device_paths(class)
        .into_iter()
        .find(|path| std::fs::File::open(path).is_ok())
        .unwrap_or_else(|| stand_in.to_string())
}

/// Mints a real token and returns the ruleset it derives.
fn ruleset_for(media: Vec<CaptureMedium>) -> Result<DeviceRuleset, Box<dyn std::error::Error>> {
    let mut ledger = ledger_granting(media.clone(), TERM_TO)?;
    let mut audit = CaptureAudit::new();
    let request = request_for(media, common::TOKEN_UNTIL)?;
    let authorization = authorize(&mut ledger, &mut audit, &request, INSIDE)?;
    Ok(authorization.ruleset().clone())
}

fn run(
    ruleset: Option<DeviceRuleset>,
    trees: Vec<DeviceTree>,
    targets: Vec<String>,
    contained: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let report_dir = tempfile::tempdir()?;
    let spec = LaunchSpec {
        program: PathBuf::from(PROBE),
        targets,
        trees,
        ruleset,
        report_dir: report_dir.path().to_path_buf(),
        contained,
    };
    Ok(native::launch(&spec)?)
}

/// What the probe reported for one target.
fn outcome<'report>(report: &'report str, target: &str) -> Option<&'report str> {
    report
        .lines()
        .find(|line| line.starts_with(&format!("open {target} -> ")))
        .and_then(|line| line.split_once(" -> "))
        .map(|(_, outcome)| outcome)
}

/// The device layer this build and this host can install.
#[test]
fn the_backend_is_the_one_this_platform_installs() -> TestResult {
    let layer = native::availability();
    let expected = if cfg!(target_os = "linux") {
        BackendId::LinuxLandlock
    } else {
        BackendId::WindowsAppContainer
    };
    match layer {
        DeviceLayer::Enforced(backend) => assert_eq!(backend, expected),
        // A kernel without Landlock, or a Windows build that cannot create an
        // AppContainer profile. Recorded rather than coerced.
        DeviceLayer::Unavailable => {
            eprintln!("NOT_RUN: this host installs no device layer ({layer:?})");
        }
        DeviceLayer::Bookkeeping => {
            return Err("the native lane must not report bookkeeping".into());
        }
    }
    Ok(())
}

/// A process holding no token obtains no device handle, and the same process
/// holding no restriction obtains one.
///
/// This is the row the task names. It is measured on a real character device
/// node on Linux and on a real kernel-streaming capture filter on Windows; what
/// each platform actually refuses is asserted separately below, because the two
/// answers differ and the difference is the honest part.
#[test]
fn no_device_handle_without_token() -> TestResult {
    if native::availability() == DeviceLayer::Unavailable {
        eprintln!("NOT_RUN: no device layer installs on this host");
        return Ok(());
    }
    let targets = measurable_targets();
    if targets.is_empty() {
        eprintln!(
            "NOT_RUN: this host exposes no device path whose baseline open is permitted; \
             a refusal with no paired permission is not evidence"
        );
        return Ok(());
    }
    let trees = trees_for(&targets);

    // The paired permission. Every target must be permitted here or the row
    // below measures nothing.
    eprintln!("measured on: {}", targets.join(", "));
    let baseline = run(None, trees.clone(), targets.clone(), false)?;
    for target in &targets {
        assert_eq!(
            outcome(&baseline, target),
            Some("OPENED"),
            "the uncontained run was refused {target}; the contained row is not evidence"
        );
    }

    // No token at all: no ruleset, so no rule for any device tree.
    let contained = run(None, trees, targets.clone(), true)?;
    for target in &targets {
        let answer = outcome(&contained, target).unwrap_or("<missing>");
        assert!(
            answer.starts_with("REFUSED"),
            "a process holding no token opened {target}: {answer}"
        );
        assert_eq!(
            answer,
            expected_refusal(),
            "{target} was refused with an unexpected code"
        );
    }
    Ok(())
}

/// On Linux, the token's media set is what the kernel enforces.
///
/// An audio-only token adds a rule for the microphone tree and none for the
/// camera tree, so the same process opens one and is refused the other. That is
/// `audio_only_permission_denies_camera` measured at the kernel rather than in
/// this crate's own comparison.
#[cfg(target_os = "linux")]
#[test]
fn the_kernel_splits_by_the_tokens_media_set() -> TestResult {
    if native::availability() == DeviceLayer::Unavailable {
        eprintln!("NOT_RUN: no device layer installs on this host");
        return Ok(());
    }
    let microphone = linux_path_for(DeviceClass::Microphone, LINUX_STAND_IN_GRANTED);
    let camera = linux_path_for(DeviceClass::Camera, LINUX_STAND_IN_UNGRANTED);
    let trees = vec![
        DeviceTree::new(DeviceClass::Microphone, PathBuf::from(&microphone)),
        DeviceTree::new(DeviceClass::Camera, PathBuf::from(&camera)),
    ];
    let targets = vec![microphone.clone(), camera.clone()];

    eprintln!("measured on: {}", targets.join(", "));
    let baseline = run(None, trees.clone(), targets.clone(), false)?;
    assert_eq!(outcome(&baseline, &microphone), Some("OPENED"));
    assert_eq!(outcome(&baseline, &camera), Some("OPENED"));

    let audio_only = ruleset_for(vec![CaptureMedium::Audio])?;
    assert!(audio_only.permits(DeviceClass::Microphone));
    assert!(!audio_only.permits(DeviceClass::Camera));
    let contained = run(Some(audio_only), trees.clone(), targets.clone(), true)?;
    assert_eq!(
        outcome(&contained, &microphone),
        Some("OPENED"),
        "the granted tree was refused, so the split is a closed door and not a rule"
    );
    assert_eq!(
        outcome(&contained, &camera),
        Some("REFUSED 13"),
        "the ungranted tree was reachable"
    );

    // And a token that names both opens both, so the refusal above is the media
    // set rather than a ruleset that refuses whatever it is given second.
    let both = ruleset_for(vec![CaptureMedium::Audio, CaptureMedium::Video])?;
    let widened = run(Some(both), trees, targets, true)?;
    assert_eq!(outcome(&widened, &microphone), Some("OPENED"));
    assert_eq!(outcome(&widened, &camera), Some("OPENED"));
    Ok(())
}

/// On Windows the container refuses every class, and this row is what says so
/// rather than a sentence that reads as if the split were enforced there too.
#[cfg(target_os = "windows")]
#[test]
fn the_windows_container_refuses_every_class_including_the_granted_one() -> TestResult {
    if native::availability() == DeviceLayer::Unavailable {
        eprintln!("NOT_RUN: no AppContainer profile can be created on this host");
        return Ok(());
    }
    let targets = measurable_targets();
    if targets.is_empty() {
        eprintln!("NOT_RUN: this host exposes no openable capture device interface");
        return Ok(());
    }
    let trees = trees_for(&targets);
    let audio_only = ruleset_for(vec![CaptureMedium::Audio])?;
    assert!(audio_only.permits(DeviceClass::Microphone));
    let contained = run(Some(audio_only), trees, targets.clone(), true)?;
    for target in &targets {
        assert_eq!(
            outcome(&contained, target),
            Some("REFUSED 5"),
            "the Windows container's answer changed; this suite and the contract are stale"
        );
    }
    Ok(())
}

/// Which device paths this host actually exposes, and what the baseline gets
/// for each.
///
/// Printed rather than asserted. It is what a contract row is written from, and
/// it is why the rows above are per-host rather than per-repository.
#[test]
fn the_measured_device_nodes_are_reported() -> TestResult {
    for class in academic_capture_gate::DEVICE_CLASSES {
        let paths = native::device_paths(class);
        if paths.is_empty() {
            eprintln!("{}: no path on this host", class.as_str());
            continue;
        }
        for path in paths {
            let report = run(None, Vec::new(), vec![path.clone()], false)?;
            eprintln!(
                "{}: {path} baseline {}",
                class.as_str(),
                outcome(&report, &path).unwrap_or("<missing>")
            );
        }
    }
    Ok(())
}

/// Ensures nothing in this suite reached a device through the default lane's
/// bookkeeping value by accident.
#[test]
fn the_native_lane_never_reports_bookkeeping() {
    assert_ne!(
        native::availability(),
        DeviceLayer::Bookkeeping,
        "the native lane reported the default lane's value"
    );
}

/// The paths whose baseline open this host permits.
///
/// On Linux these are the control nodes: the capture nodes are usually either
/// absent or refused by the group they belong to, and a row measured on one of
/// those would be a refusal with no paired permission. On Windows they are the
/// kernel-streaming capture filters the configuration manager reports as
/// present.
fn measurable_targets() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        [
            linux_path_for(DeviceClass::Microphone, LINUX_STAND_IN_GRANTED),
            linux_path_for(DeviceClass::Camera, LINUX_STAND_IN_UNGRANTED),
        ]
        .into_iter()
        .filter(|path| Path::new(path).exists() && std::fs::File::open(path).is_ok())
        .collect()
    }
    #[cfg(target_os = "windows")]
    {
        native::device_paths(DeviceClass::Microphone)
            .into_iter()
            .filter(|path| std::fs::File::open(path).is_ok())
            .take(1)
            .collect()
    }
}

/// One tree per target, so the contained run's rules are expressed over exactly
/// the paths it is measured against.
fn trees_for(targets: &[String]) -> Vec<DeviceTree> {
    targets
        .iter()
        .map(|target| DeviceTree::new(DeviceClass::Microphone, PathBuf::from(target)))
        .collect()
}

/// What a refusal looks like on this platform.
const fn expected_refusal() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        // `EACCES`: the path is under no Landlock rule.
        "REFUSED 13"
    }
    #[cfg(target_os = "windows")]
    {
        // `ERROR_ACCESS_DENIED`: the device object's DACL grants no
        // AppContainer, and the container holds no capability SID.
        "REFUSED 5"
    }
}
