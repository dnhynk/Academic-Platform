//! TEMPORARY. `P2-RF31`'s pin regenerator. Deleted before the branch is
//! pushed; it exists only so the 71 pin files are written by the same reader
//! the two rules read with, rather than transcribed by hand.
//!
//! Run with `T235_REGENERATE=1 cargo test -p academic-contracts --test
//! t235_regenerate -- --nocapture`.

mod support;

use std::{env, fs, time::Instant};

use support::{
    Item, TestResult, crate_directories, items_of, product_roots, relative, repository_root,
    resolve,
};

#[test]
fn regenerate() -> TestResult {
    if env::var("T235_REGENERATE").as_deref() != Ok("1") {
        return Ok(());
    }
    let repository = repository_root()?;
    let started = Instant::now();
    let mut packages = 0_usize;
    let mut total = 0_usize;
    for directory in crate_directories(&repository)? {
        let package = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        let mut files = std::collections::BTreeSet::new();
        for root in product_roots(&directory)? {
            files.extend(resolve(&root, &repository)?.files);
        }
        let mut items = Vec::new();
        for file in files {
            let name = relative(&repository, &file);
            items.extend(items_of(&name, &fs::read_to_string(&file)?)?);
        }
        let mut keys: Vec<String> = items.iter().map(Item::sealed_key).collect();
        keys.sort();
        total += keys.len();
        packages += 1;
        let path = repository
            .join("crates/contracts/tests/pinned-items")
            .join(format!("{package}.items"));
        let mut text = keys.join("\n");
        text.push('\n');
        fs::write(&path, text)?;
    }
    println!(
        "T235 packages={packages} items={total} seconds={:.2}",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
