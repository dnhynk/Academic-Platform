//! Writes the diarization corpus and its measurement.
//!
//! `cargo run -p academic-student-voice --example emit_corpus`. The suite
//! renders the same bytes and compares them, so this exists to update the
//! committed files after a deliberate change to the corpus or the scorer, and
//! never as the source of truth for what they should contain.

use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    for file in academic_student_voice::harness::corpus_files()? {
        let path = root.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.bytes)?;
        println!("{}", file.path);
    }
    Ok(())
}
