//! The process on the other side of the device layer.
//!
//! It asks the operating system for a device handle and writes what the
//! operating system answered. It installs nothing: the Landlock ruleset was
//! installed between `fork` and `exec` and the AppContainer was applied by
//! `CreateProcessW`, both by the parent, so there is no argument to this binary
//! that widens what it may reach.
//!
//! # Why it lives outside `src`
//!
//! It names a device open, and it opens one. `no_device_handle_without_token`
//! is measured by asking for a handle and being refused, so a file that asks
//! has to exist; keeping it outside `src` as a `[[bin]]` with
//! `required-features` is how `academic-worker`'s probe is kept out of every
//! default build, and `probe_targets_are_not_in_any_default_build` reads both
//! facts out of the manifest rather than taking them on trust.
//!
//! # It records nothing
//!
//! The handle is opened for read and dropped. No stream is started, no sample
//! is read, no frame is pulled, and nothing is written anywhere but the report.
//! `the_probe_opens_a_handle_and_reads_no_sample` pins the whole of `attempt`,
//! so a read added here has to be added to the pin in the same commit.

use std::{fmt::Write as _, fs, path::Path};

use academic_capture_gate::native::{REPORT_DIR_VAR, REPORT_FILE};

fn main() {
    let targets: Vec<String> = std::env::args().skip(1).collect();
    let mut report = String::new();
    for target in &targets {
        let _ = writeln!(report, "open {target} -> {}", attempt(target));
    }
    if let Ok(directory) = std::env::var(REPORT_DIR_VAR) {
        let _ = fs::write(Path::new(&directory).join(REPORT_FILE), &report);
    }
    std::process::exit(0);
}

/// Asks the operating system for a handle on `target` and drops it.
///
/// This is the whole of what the probe does to a device. It opens for read and
/// closes; it starts no stream and reads no byte, on either platform.
fn attempt(target: &str) -> String {
    match fs::File::open(target) {
        Ok(handle) => {
            drop(handle);
            String::from("OPENED")
        }
        Err(error) => format!("REFUSED {}", error.raw_os_error().unwrap_or(-1)),
    }
}
