//! Writes both engines' harness corpora from the deterministic builder.
//!
//! `CONTRIBUTING.md` rule 5 admits a golden fixture only through a
//! deterministic builder, and this is the entry point that runs it:
//!
//! ```text
//! cargo run -p academic-record --example emit_harness
//! ```
//!
//! `harness_corpus_matches_a_fresh_render` re-renders the same bytes and
//! byte-compares them against what is committed, so a fixture edited by hand
//! into agreement with a broken engine fails rather than passes.

use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let mut written = 0_usize;
    for file in academic_record::harness::corpus_files()? {
        let path = root.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.bytes)?;
        written += 1;
    }
    println!("wrote {written} harness files");
    Ok(())
}
