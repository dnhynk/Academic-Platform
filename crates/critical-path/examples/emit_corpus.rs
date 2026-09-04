//! Writes this crate's `P2-C5` determinism corpus.
//!
//! `cargo run -p academic-critical-path --example emit_corpus`
//!
//! The builder lives in the test tree, because a case needs a real `P2-N5`
//! `GapCase` and the only producer of one is `academic_gap::search` over the
//! fixture chain in `tests/common/mod.rs`. An example is compiled with
//! dev-dependencies, so it reaches the same module the executing half of the
//! harness does -- and both therefore render from one builder rather than from
//! two that agree by inspection.
//!
//! The corpus sits at `testdata/critical-path/`, not under `testdata/engines/`:
//! everything there belongs to one of `P2-C5`'s twelve registered engines and
//! section 16 is not one of them.

#[path = "../tests/corpus/mod.rs"]
mod corpus;

use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    for file in corpus::corpus_files()? {
        let path = root.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.bytes)?;
        println!("wrote {}", file.path);
    }
    Ok(())
}
