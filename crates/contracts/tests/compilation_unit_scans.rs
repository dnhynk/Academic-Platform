//! The set of files this repository's source scans read, against the set the
//! compiler reads.
//!
//! Every source scan in this repository walks a directory and keeps the entries
//! whose extension is `rs`. `docs/contracts/policy-source-scans.md` calls that
//! walk total over the crate, and the inventories built on it say so in their
//! own words: *"There is no spelling of that injection that this test does not
//! see, because it does not look for spellings: it compares the set."*
//!
//! Rust does not require a compiled file to be named `*.rs`. `include!` takes
//! any path, `#[path]` takes any path, and neither is bounded by the directory
//! the declaring file is in. A file the compiler reads and the walk does not is
//! not a weaker check — it is no check, and every inventory, `Default` sweep,
//! producer count and determinism scan in the workspace is total over a set
//! that does not contain it. `P2-A3`'s second audit measured that: one
//! `include!("witness_ext.inc")` line put four different injections past
//! `academic-audit`, `academic-offering` and `academic-review` with the
//! workspace reporting 279 `test result: ok` blocks and exit 0.
//!
//! So the two sets are enumerated and compared here. The reader below resolves
//! a crate's compilation unit the way `rustc` does — from each target root,
//! through `mod name;`, `#[path = "…"] mod name;` and `include!("…")`,
//! recursively — and the tests state what that closure may contain:
//!
//! * every file in it is a `*.rs` file, so a `.inc` fails by name;
//! * every file a crate's **product** targets pull in is under that crate's own
//!   `src`, so a `#[path]` reaching sideways fails by name;
//! * every `*.rs` file under a crate's `src` is in that crate's product
//!   closure, so a file that no target compiles fails as well — the direction
//!   that makes this an equality rather than a floor;
//! * every path handed to the compiler is one this reader could resolve, so a
//!   computed path fails rather than being skipped.
//!
//! It is not a list of forbidden spellings. `include!` stays legal and so does
//! `#[path]`: what fails is a compilation unit that the `*.rs` walks cannot
//! see. An injection renamed to `witness_ext.rs` passes here and is then read
//! by the crate's own walk, where it arrives at the declaration and impl
//! inventories as an entry nobody wrote down. Both routes end in a named
//! failure, which is the property the inventories claim.
//!
//! **What this reader deliberately over-approximates.** `cfg` is not
//! evaluated, so a platform or feature module is in the closure on every
//! platform. That is the direction a scan needs: the set a scan must read is
//! every file the compiler could read under any configuration, not the subset
//! one build happened to compile.
//!
//! **What it cannot see.** A path built by `concat!`/`env!` is not decidable
//! from the text. There are four such sites, pinned whole below, and the one
//! that injects *items* — `crates/rpc/src/generated.rs` — carries its own
//! byte fingerprint of the generated file (`EXPECTED_CODEGEN_FNV1A64`). A
//! procedural macro can also read a file; none in the dependency closure does,
//! and that is a statement about `Cargo.lock`, which
//! `tools/cargo-lock-source-policy.mjs` holds, rather than about this walk.

mod support;

use std::{collections::BTreeSet, fs, path::PathBuf};

use support::{
    TestResult, all_roots, crate_directories, product_roots, relative, repository_root, resolve,
    rust_files,
};

// ---------------------------------------------------------------------------
// The pinned exceptions, each with the reason it is one
// ---------------------------------------------------------------------------

/// Product target roots that do not live under their crate's `src`.
///
/// `S-12` in `docs/contracts/policy-source-scans.md` is the row about a scan
/// that walks `<crate>/src` and stops seeing product-shaped code beside it.
/// These four are that shape, and the three probes are why the row exists:
/// they are the only files in the workspace that name a socket type, each a
/// `[[bin]]` behind `required-features` with a `path` outside `src`. A fifth
/// arriving fails here rather than becoming a tree that no `src` walk reads —
/// which is what happened: `crates/process-sandbox/probes/enforcement_probe.rs`
/// arrived from `P2-RF21` while this list held three, and it failed as an extra
/// key with no edit to any scan.
const PRODUCT_ROOTS_OUTSIDE_SRC: [&str; 4] = [
    "crates/capture-gate/probes/capture_probe.rs",
    "crates/process-sandbox/probes/enforcement_probe.rs",
    "crates/rpc/build.rs",
    "crates/worker/probes/worker_probe.rs",
];

/// `*.rs` files under a crate's `src` that no target of that crate compiles.
///
/// `academic-test-support`'s `lib.rs` declares no module. These six files are
/// text that other crates' **test** targets pull in through `#[path]` —
/// `crates/crypto/tests/key_hierarchy.rs` and
/// `crates/recovery/tests/recovery_admission.rs` share
/// `word_level_entry_points.rs` that way, which is the row `S-14` records. They
/// are read by a walk of `crates/test-support/src`; they are not compiled by
/// `academic-test-support`.
const SOURCE_NO_TARGET_OF_ITS_CRATE_COMPILES: [&str; 6] = [
    "crates/test-support/src/encrypted_artifacts.rs",
    "crates/test-support/src/fault_driver.rs",
    "crates/test-support/src/oracle.rs",
    "crates/test-support/src/process.rs",
    "crates/test-support/src/synthetic_artifacts.rs",
    "crates/test-support/src/word_level_entry_points.rs",
];

/// Every path handed to the compiler that this reader cannot resolve.
///
/// A computed path is the one shape that defeats a textual reader, so the set
/// is pinned whole and each entry is pinned as its own text. Exactly one of
/// them injects **items**: `crates/rpc/src/generated.rs`, whose `include!`
/// names the file `prost-build` writes into `OUT_DIR`. That file is not a hole
/// — the same module fingerprints both the schema it was generated from and
/// the generated bytes, and `academic-rpc` refuses to agree with a build whose
/// fingerprints have moved.
///
/// The `!` of the bare `include` is written `\u{21}` in the two entries that
/// carry one. `only_egress_crate_has_a_socket` in
/// `tools/phase1-scaffold-policy.test.mjs` enumerates every `include!` site in
/// the repository against a pinned map and reads the source with its string
/// literals in place, so a pin quoting the text of an `include!` would arrive
/// there as a second site in a file that has none. The runtime values are the
/// text they pin, byte for byte, which is what the comparisons below use.
const COMPUTED_INCLUDE_PATHS: [&str; 4] = [
    "crates/rpc/src/generated.rs: include\u{21}(concat!(env!(\"OUT_DIR\"), \"/academic.v1.rs\"))",
    "crates/rpc/src/generated.rs: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/academic.v1.rs\"))",
    "crates/transcript/tests/support/mod.rs: include_str!(concat!( env!(\"CARGO_MANIFEST_DIR\"), \
     \"/../../testdata/transcript-canary/canaries.txt\" ))",
    "crates/transcript/tests/transcript_ingestion.rs: include_str!(concat!( \
     env!(\"CARGO_MANIFEST_DIR\"), \"/../../testdata/admission/incomplete-receipt.cbor.hex\" ))",
];

/// The one `include!` of a computed path, pinned as whole text.
const GENERATED_MODULE_INCLUDE: &str =
    "include\u{21}(concat!(env!(\"OUT_DIR\"), \"/academic.v1.rs\"));";

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// The crates this file reads are the crates the workspace compiles.
///
/// The floor every test below rests on. A walk that returned nothing would
/// satisfy every "no closure contains X" assertion in this file, so the crate
/// set is compared against the workspace member list in both directions: a
/// member with no directory fails, and a directory no member names fails.
#[test]
fn the_walk_reads_every_crate_the_workspace_compiles() -> TestResult {
    let repository = repository_root()?;
    let manifest = fs::read_to_string(repository.join("Cargo.toml"))?;
    let mut declared: BTreeSet<String> = BTreeSet::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "members = [" {
            inside = true;
            continue;
        }
        if inside {
            if trimmed == "]" {
                break;
            }
            let member = trimmed.trim_end_matches(',').trim_matches('"');
            declared.insert(member.to_owned());
        }
    }
    let walked: BTreeSet<String> = crate_directories(&repository)?
        .iter()
        .map(|path| relative(&repository, path))
        .collect();
    assert_eq!(
        walked, declared,
        "the crate directories and the workspace members are not the same set"
    );
    assert!(
        declared.len() >= 60,
        "the member list holds only {} crates",
        declared.len()
    );
    Ok(())
}

/// Every file the compiler pulls into a crate is a `*.rs` file a walk reads.
///
/// The statement `P2-A3` falsified. Two conditions, and the second is the one
/// the audit's `include!("witness_ext.inc")` walked past:
///
/// * the closure of a crate's **product** targets stays under that crate's own
///   `src`, so a `#[path]` reaching into `tests/`, into another crate, or out
///   of the repository fails by name;
/// * every file in every closure, product and test alike, is a `*.rs` file, so
///   a compiled file that no extension filter admits fails by name.
#[test]
fn every_file_the_compiler_compiles_is_a_rust_file_a_walk_reads() -> TestResult {
    let repository = repository_root()?;
    let crates = crate_directories(&repository)?;
    let mut product_total = 0_usize;
    let mut every_total = 0_usize;
    let mut outside_src: BTreeSet<String> = BTreeSet::new();

    for crate_directory in &crates {
        let source_root = crate_directory.join("src");
        let mut product: BTreeSet<PathBuf> = BTreeSet::new();
        for root in product_roots(crate_directory)? {
            product.extend(resolve(&root, &repository)?.files);
        }
        for file in &product {
            assert!(
                file.is_file(),
                "{} is compiled into {} and is not a file",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
            assert!(
                file.extension().is_some_and(|extension| extension == "rs"),
                "{} is compiled into {} and no `*.rs` walk reads it",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
            if !file.starts_with(&source_root) {
                outside_src.insert(relative(&repository, file));
            }
        }
        product_total += product.len();

        let mut every: BTreeSet<PathBuf> = BTreeSet::new();
        for root in all_roots(crate_directory)? {
            every.extend(resolve(&root, &repository)?.files);
        }
        for file in &every {
            assert!(
                file.is_file(),
                "{} is compiled into a target of {} and is not a file",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
            assert!(
                file.extension().is_some_and(|extension| extension == "rs"),
                "{} is compiled into a target of {} and no `*.rs` walk reads it",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
            assert!(
                file.starts_with(repository.join("crates")),
                "{} is compiled into a target of {} and lives outside `crates/`",
                relative(&repository, file),
                relative(&repository, crate_directory)
            );
        }
        every_total += every.len();
    }

    assert_eq!(
        outside_src,
        PRODUCT_ROOTS_OUTSIDE_SRC
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "a crate compiles product source that a walk of its `src` does not read"
    );
    // The floors. Both counts are sums over crates, so an empty walk in one
    // crate is not enough to trip them on its own; the crate-set equality above
    // is what carries that half.
    assert!(
        product_total >= 500,
        "the product closures hold only {product_total} files"
    );
    assert!(
        every_total >= 750,
        "the target closures hold only {every_total} files"
    );
    Ok(())
}

/// Every `*.rs` file under a crate's `src` is in that crate's product closure.
///
/// The other direction, and what makes the pair an equality rather than a
/// floor. Without it a file could sit in `src` compiled by nothing, be read by
/// every scan, and mean nothing — or, the shape that matters, a module could be
/// moved out of the tree the walks read while still compiling.
#[test]
fn every_product_file_is_compiled_by_its_own_crate() -> TestResult {
    let repository = repository_root()?;
    let mut unreached: BTreeSet<String> = BTreeSet::new();
    let mut examined = 0_usize;
    for crate_directory in crate_directories(&repository)? {
        let mut product: BTreeSet<PathBuf> = BTreeSet::new();
        for root in product_roots(&crate_directory)? {
            product.extend(resolve(&root, &repository)?.files);
        }
        for file in rust_files(&crate_directory.join("src"))? {
            examined += 1;
            if !product.contains(&file) {
                unreached.insert(relative(&repository, &file));
            }
        }
    }
    assert_eq!(
        unreached,
        SOURCE_NO_TARGET_OF_ITS_CRATE_COMPILES
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "a file under a crate's `src` is compiled by no target of that crate"
    );
    assert!(
        examined >= 500,
        "the `src` walk found only {examined} files"
    );
    Ok(())
}

/// Every path the compiler is given is one this reader resolved.
///
/// A computed path is where a textual reader stops being able to decide, so the
/// set of them is pinned whole rather than skipped, and the literal targets are
/// required to stay inside this repository — the escape an embedded file has
/// that a module does not.
#[test]
fn every_path_handed_to_the_compiler_is_one_this_reader_resolved() -> TestResult {
    let repository = repository_root()?;
    let mut computed: BTreeSet<String> = BTreeSet::new();
    let mut embedded: BTreeSet<(String, PathBuf)> = BTreeSet::new();
    for crate_directory in crate_directories(&repository)? {
        for root in all_roots(&crate_directory)? {
            let closure = resolve(&root, &repository)?;
            computed.extend(closure.computed);
            embedded.extend(closure.embedded);
        }
    }
    assert_eq!(
        computed,
        COMPUTED_INCLUDE_PATHS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<BTreeSet<_>>(),
        "a file is included under a path this reader cannot resolve"
    );
    for (site, target) in &embedded {
        assert!(
            target.starts_with(&repository),
            "{site} embeds {}, which is outside this repository",
            target.display()
        );
        assert!(
            target.is_file(),
            "{site} embeds {}, which is not a file",
            target.display()
        );
    }
    assert!(
        embedded.len() >= 25,
        "the walk found only {} embedded files",
        embedded.len()
    );

    // The one computed path that injects items, pinned as its own text.
    let generated = fs::read_to_string(repository.join("crates/rpc/src/generated.rs"))?;
    let line = generated
        .lines()
        .find(|line| line.trim_start().starts_with("include!"))
        .ok_or("crates/rpc/src/generated.rs no longer includes a generated module")?;
    assert_eq!(
        line.trim(),
        GENERATED_MODULE_INCLUDE,
        "the generated module's include changed; the pin must change with it"
    );
    Ok(())
}
