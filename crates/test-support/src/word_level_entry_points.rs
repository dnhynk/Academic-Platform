//! `KY06`'s structural half, in one place for the two crates that carry it.
//!
//! `KY06` says no public API can be asked about, or answer with, an individual
//! word of a recovery phrase — that is what keeps a wrong phrase from being
//! narrowed word by word. Two crates hold part of it: `RecoverySecret` and the
//! key schedule are in `academic-crypto`, and the backup recipient set that
//! opens with one is in `academic-recovery`.
//!
//! The `T141` audit found the two halves scanning differently — five spellings
//! and three fixed `include_str!` paths on the crypto side, thirteen spellings
//! and a recursive walk with a floor and a module tripwire on the recovery
//! side, and neither spelling list a subset of the other. A word-level entry
//! point named `mnemonic_at` was refused in one crate and admitted in the
//! other, which is not one contract but two. So the list and the walk are one
//! module, included by both scans through `#[path]` the way this crate's other
//! shared test vocabulary is, and a spelling added here is refused in both.

#![allow(dead_code)]

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

/// Every spelling that would name a word, a word list, or an index into one.
///
/// The union of what the two halves held separately. Neither crate's `src`
/// spells any of them today, which is the property.
pub const WORD_LEVEL_ENTRY_POINTS: [&str; 16] = [
    "BIP39",
    "Bip39",
    "MNEMONIC",
    "Mnemonic",
    "WORDLIST",
    "WORD_COUNT",
    "WordList",
    "bip39",
    "fn word",
    "from_words",
    "mnemonic",
    "phrase_word",
    "to_words",
    "word_index",
    "wordlist",
    "words(",
];

/// Reads every `*.rs` under one crate's `src`, at any depth.
///
/// `manifest` is the calling crate's `CARGO_MANIFEST_DIR` and `floor` is how
/// many files that tree holds today. The walk is recursive because a flat
/// `read_dir` reads a flat tree correctly and reads a subdirectory module not
/// at all, and the floor is here because a walk that silently returns nothing
/// passes every assertion made over its result.
pub fn read_crate_sources(
    manifest: &str,
    floor: usize,
) -> Result<Vec<(PathBuf, String)>, Box<dyn Error>> {
    let source_root = Path::new(manifest).join("src");
    let mut sources = Vec::new();
    let mut pending = vec![source_root];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = fs::read_to_string(&path)?;
                sources.push((path, text));
            }
        }
    }
    sources.sort();
    assert!(
        sources.len() >= floor,
        "the source scan found only {} files, not {floor}",
        sources.len()
    );

    // Descending is the property, so it is checked rather than assumed: an
    // out-of-line module declared on its own line has to be a file the scan
    // read. A walk that stops descending leaves a declared module unread, and
    // this fails then rather than passing quietly the way the flat walk did.
    //
    // It sees `mod name;` and `pub mod name;` and nothing else — not a `#[path]`
    // attribute, and not a declaration sharing a line with an attribute. It is a
    // tripwire on the walk above, not a second way of finding files.
    let read: Vec<&Path> = sources.iter().map(|(path, _)| path.as_path()).collect();
    for (path, text) in &sources {
        for name in declared_modules(text) {
            let candidates = module_files(path, &name);
            assert!(
                candidates
                    .iter()
                    .any(|candidate| read.contains(&&**candidate)),
                "{} declares `mod {name};` but the source scan read neither {} nor {}",
                path.display(),
                candidates[0].display(),
                candidates[1].display()
            );
        }
    }
    Ok(sources)
}

/// Names of the out-of-line modules one source file declares.
fn declared_modules(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_suffix(';'))
        .filter_map(|line| {
            line.strip_prefix("mod ")
                .or_else(|| line.strip_prefix("pub mod "))
        })
        .map(str::to_owned)
        .collect()
}

/// The two paths a `mod name;` in `declaring` may live at.
fn module_files(declaring: &Path, name: &str) -> [PathBuf; 2] {
    let directory = match declaring.file_stem().and_then(|stem| stem.to_str()) {
        Some("lib" | "mod") => declaring
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        _ => declaring.with_extension(""),
    };
    [
        directory.join(format!("{name}.rs")),
        directory.join(name).join("mod.rs"),
    ]
}
