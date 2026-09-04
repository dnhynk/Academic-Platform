//! Writes the `GRADUATION_AUDIT` harness corpus.
//!
//! `cargo run -p academic-audit --example emit_harness`
//!
//! The builder lives in the test tree, because a published `RuleSet` needs an
//! `academic_ingestion::PublishedRules` and the only producer of one is that
//! crate's stage nine over that crate's own fixture documents. An example is
//! compiled with dev-dependencies, so it can reach the same module the
//! executing half of the harness does -- and both therefore render from one
//! builder rather than from two that agree by inspection.

#[path = "../tests/support/mod.rs"]
mod support;

use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    for file in support::harness::corpus_files()? {
        let path = root.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.bytes)?;
        println!("wrote {}", file.path);
    }
    Ok(())
}
