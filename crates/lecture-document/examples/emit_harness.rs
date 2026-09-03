//! Writes the `TRANSCRIPT_COVERAGE` harness corpus.
//!
//! `cargo run -p academic-lecture-document --example emit_harness`. The suite
//! renders the same bytes and compares them, so this exists to update the
//! committed files after a deliberate semantic change and never as the source
//! of truth for what they should contain.

use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    for file in academic_lecture_document::harness::corpus_files()? {
        let path = root.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.bytes)?;
        println!("{}", file.path);
    }
    Ok(())
}
