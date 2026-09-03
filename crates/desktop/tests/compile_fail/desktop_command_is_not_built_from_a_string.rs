//! The command allowlist is not constructible from a string.
//!
//! A `TryFrom<&str>`, a `FromStr`, or a variant carrying a free-form capability
//! identifier would let anything the user or a plugin could name become a
//! local-core request, which is the whole point of the enum being closed.

use std::str::FromStr;

use academic_desktop::DesktopCommand;

fn main() {
    let _by_try_from = DesktopCommand::try_from("learning-platform.local.diagnostics.v1");
    let _by_from_str = DesktopCommand::from_str("learning-platform.local.diagnostics.v1");
    let _by_variant = DesktopCommand::Raw("anything at all".to_owned());
}
