//! The platform device layer.
//!
//! # The parent installs it, on both platforms
//!
//! [`launch`] takes the [`DeviceRuleset`] the daemon derived from a token and
//! runs the capture process under it. The child installs nothing and is handed
//! nothing it could widen: on Linux the Landlock ruleset is installed between
//! `fork` and `exec`, and on Windows the AppContainer is applied by
//! `CreateProcessW`. So there is no wire form of a ruleset for a contained
//! process to misparse in its own favour, and [`DeviceRuleset`] keeps the one
//! constructor that takes a token.
//!
//! # What is measured
//!
//! With `native-capture` off this module compiles to functions that say so:
//! [`availability`] reports [`DeviceLayer::Bookkeeping`] and [`launch`]
//! refuses. Nothing in the default lane opens a device, installs a ruleset, or
//! names a device path.
//!
//! With the feature on, each backend turns a ruleset into an operating-system
//! restriction over a set of [`DeviceTree`]s -- one path per device class,
//! supplied by the caller rather than compiled in, so the acceptance rows can
//! name the paths they were measured against and a host with different ones is
//! a different row rather than a silent skip.
//!
//! The two platforms refuse for different reasons and one of them cannot widen,
//! and the contract page says so rather than averaging them:
//!
//! * **Linux** adds a `path_beneath` rule for each tree whose class the ruleset
//!   permits and for the report directory, and for nothing else, so a path
//!   under an unruled tree is `EACCES` and a path under a ruled one is not.
//!   The split by media is the kernel's.
//! * **Windows** applies an AppContainer holding no capability SID. A kernel
//!   streaming capture filter's DACL grants no AppContainer, so the open is
//!   `ERROR_ACCESS_DENIED`. There is no user-mode way to widen it back: a
//!   device object's DACL needs `WRITE_DAC`, which the driver grants to
//!   `SYSTEM` and administrators. The Windows container therefore refuses every
//!   class rather than the ungranted ones, and the media split there is this
//!   crate's own comparison rather than the kernel's.
//!
//! That asymmetry is the same shape as `academic-worker`'s socket row and it is
//! written down for the same reason: the unqualified sentence is true on one
//! platform and not on the other, so neither the code nor the contract says it
//! for both.

use std::path::PathBuf;

use crate::device::{DeviceClass, DeviceLayer, DeviceRuleset};

#[cfg(all(feature = "native-capture", target_os = "linux"))]
mod linux;
#[cfg(all(feature = "native-capture", target_os = "windows"))]
mod windows;

/// One device class and the operating-system path its devices live under.
///
/// The path is an argument. A compiled-in device path would make every row in
/// the contract table a claim about the machine that wrote it rather than about
/// the machine that ran it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceTree {
    class: DeviceClass,
    path: PathBuf,
}

impl DeviceTree {
    /// Names a tree.
    #[must_use]
    pub const fn new(class: DeviceClass, path: PathBuf) -> Self {
        Self { class, path }
    }

    /// Which class lives under it.
    #[must_use]
    pub const fn class(&self) -> DeviceClass {
        self.class
    }

    /// Where.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Why a backend could not install.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NativeError {
    /// This build or this platform has no backend.
    #[error("no device-layer backend is compiled for this target: {0}")]
    Unavailable(String),
    /// A syscall the backend makes failed.
    #[error("{step} failed with code {code}")]
    Syscall {
        /// Which call.
        step: &'static str,
        /// What it reported.
        code: i64,
    },
    /// A path the backend was given could not be used, or the probe would not
    /// run.
    #[error("device layer could not run the probe: {0}")]
    Path(String),
}

/// What this build can enforce on this host.
#[must_use]
pub fn availability() -> DeviceLayer {
    #[cfg(all(feature = "native-capture", target_os = "linux"))]
    {
        linux::availability()
    }
    #[cfg(all(feature = "native-capture", target_os = "windows"))]
    {
        windows::availability()
    }
    #[cfg(not(all(
        feature = "native-capture",
        any(target_os = "linux", target_os = "windows")
    )))]
    {
        DeviceLayer::Bookkeeping
    }
}

/// What a run needs.
///
/// `ruleset` is `None` for the run that holds no token at all, which is the one
/// `no_device_handle_without_token` is named for. `Some(ruleset)` is a run
/// under a token, and the classes it permits are the token's own media set.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    /// The probe binary.
    pub program: PathBuf,
    /// The device paths it is asked to open.
    pub targets: Vec<String>,
    /// The device trees the restriction is expressed over.
    pub trees: Vec<DeviceTree>,
    /// The ruleset a token derived, or `None` for no token.
    pub ruleset: Option<DeviceRuleset>,
    /// The directory the probe writes its report into.
    pub report_dir: PathBuf,
    /// Whether the platform restriction is applied. `false` is the paired
    /// permission: the same binary, the same targets, no restriction.
    pub contained: bool,
}

impl LaunchSpec {
    /// Whether the run's ruleset permits `class`. No ruleset permits nothing.
    #[must_use]
    pub fn permits(&self, class: DeviceClass) -> bool {
        self.ruleset
            .as_ref()
            .is_some_and(|ruleset| ruleset.permits(class))
    }
}

/// Runs `spec` and returns the report the probe wrote.
///
/// # Errors
///
/// [`NativeError::Unavailable`] when no backend is compiled in, and the other
/// variants when one is and the launch failed.
pub fn launch(spec: &LaunchSpec) -> Result<String, NativeError> {
    #[cfg(all(feature = "native-capture", target_os = "windows"))]
    {
        windows::launch(spec)
    }
    #[cfg(all(feature = "native-capture", target_os = "linux"))]
    {
        linux::launch(spec)
    }
    #[cfg(not(all(
        feature = "native-capture",
        any(target_os = "linux", target_os = "windows")
    )))]
    {
        let _ = spec;
        Err(NativeError::Unavailable(String::from(
            "native-capture is off",
        )))
    }
}

/// The file the probe writes its answers into, inside the report directory.
pub const REPORT_FILE: &str = "capture-probe.report";

/// The environment variable naming the report directory.
pub const REPORT_DIR_VAR: &str = "ACADEMIC_CAPTURE_REPORT_DIR";

/// The operating-system paths this host exposes for `class`, if any.
///
/// On Windows these are device interface paths read from the configuration
/// manager, present ones only. On Linux they are the conventional device tree
/// roots, filtered to the ones that exist. On a host with none, the list is
/// empty and the acceptance row that would have used it records `NOT_RUN` with
/// that as its reason rather than passing on nothing.
#[must_use]
pub fn device_paths(class: DeviceClass) -> Vec<String> {
    #[cfg(all(feature = "native-capture", target_os = "windows"))]
    {
        windows::device_interface_paths(class)
    }
    #[cfg(not(all(feature = "native-capture", target_os = "windows")))]
    {
        let candidates: &[&str] = match class {
            DeviceClass::Microphone => &["/dev/snd"],
            DeviceClass::Camera => &["/dev/video0", "/dev/video1"],
            DeviceClass::Screen => &[],
        };
        candidates
            .iter()
            .filter(|candidate| std::path::Path::new(candidate).exists())
            .map(|candidate| (*candidate).to_string())
            .collect()
    }
}
