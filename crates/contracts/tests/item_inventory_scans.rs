//! The items a compilation unit holds, against the items a contract names.
//!
//! `compilation_unit_scans.rs` closed the question "which *files* does the
//! compiler read that a scan does not". This file closes the next one: **which
//! *items* does a file hold that an inventory does not**.
//!
//! `P2-A4`'s second audit measured that gap three times in one crate. The
//! inventories in `crates/student-voice/tests/student_voice_scans.rs` are
//! whole-set comparisons and say so, but the sets they are whole over are sets
//! of *spellings*:
//!
//! * `signatures_in_blocks` keeps a line beginning `pub fn `, `pub const fn `,
//!   `pub async fn ` or `pub unsafe fn `, or a bare `fn ` inside a trait
//!   `impl`;
//! * `impl_headers` keeps a line whose trimmed form begins `impl`.
//!
//! Three forms are neither, and each was measured passing that crate's whole
//! suite on both hosts and the workspace's 1652 tests besides:
//!
//! 1. `pub const SOURCE_LINES: fn(&RestrictedOriginal) -> Vec<String> = …` —
//!    handed out three students' removed utterances with no `RawAccessGrant`
//!    and no `RawAccessLog` row;
//! 2. `pub const WITHOUT_REMOVED: fn(&RestrictedOriginal) -> RestrictedOriginal`
//!    — minted a second original carrying the real one's digest with its
//!    `removed` list emptied, so a rights-request read of it disclosed nothing
//!    and wrote an audit row saying nothing was there;
//! 3. `#[allow(unused)] impl From<&RestrictedOriginal> for Vec<String>` — the
//!    trait impl `T213` closed, with an attribute in front of it on one line,
//!    which moves the keyword off the line start.
//!
//! Each round of that has been one spelling further out, so this file does not
//! add a fourth. It reads **items**.
//!
//! # What an item is, and why the enumeration is closed
//!
//! [`support::ITEM_KEYWORDS`] carries the list and the argument. Two
//! properties make the argument checkable rather than asserted, and both are
//! tests below:
//!
//! * [`the_reader_refuses_an_item_form_it_has_no_rule_for`] — a construct in
//!   item position that is none of them and is not a macro invocation makes
//!   the reader return `Err`. **Default-deny**: an unforeseen form stops the
//!   scan rather than passing through it.
//! * [`the_items_tile_every_file_the_workspace_compiles`] — the extents the
//!   reader returns cover every non-whitespace character of all 568 product
//!   files of the workspace, disjointly. An item it failed to see would leave
//!   a hole, and the hole is what the test reads. A spelling sweep can never
//!   have that property: `signatures_in_blocks` covers a few dozen lines of a
//!   900-line file and nothing says what the rest of it is.
//!
//! # What is pinned
//!
//! Two rules, and the first is the backstop under the second.
//!
//! * [`every_item_in_these_packages_is_pinned`] — the **whole item set** of
//!   every package that keys an inventory on a line prefix: 23 packages,
//!   6131 items, derived from
//!   [`the_inventories_still_keyed_on_a_line_prefix_are_named`] rather than
//!   written down. Keyed on nothing: an item added anywhere in one of them
//!   fails whatever it is called and whatever kind it is. The pins are in
//!   `pinned-items/<package>.items`, one key to a line.
//! * [`every_item_that_reaches_a_closed_type_is_pinned`] — for each type in
//!   [`CLOSED_TYPES`], the set of items whose own text names it *or* that sit
//!   inside something that does. This is the rule that fails **by name**: an
//!   injected route is reported as a new route to `RestrictedOriginal` rather
//!   than as an item nobody wrote down.
//!
//! The owner half of "reaches" is what makes the second a rule about a type
//! rather than about a signature. `pub fn bytes(&self) -> &[u8]` inside
//! `impl ReleasableArtifact` names no type at all, and it is the accessor
//! `crates/capture-gate/src/lib.rs` calls the only one in that crate.
//!
//! The second rule pins [`support::Item::sealed_key`] rather than
//! [`support::Item::key`], and the difference is a measured hole: a key is a
//! declaration, an `impl` block written inside a function body is globally
//! effective Rust, and this reader does not descend into a body. That
//! injection passed **both** rules here and was caught only by `T213`'s
//! line-anchored `impl_headers` one file over. A leaf carries a fingerprint of
//! its own text; a container does not, because its contents are enumerated as
//! items of their own.
//!
//! # What this does not claim
//!
//! The pin reaches the 23 packages whose own inventories are keyed on a line
//! prefix. **It does not reach the other 45 crates of the workspace**, which
//! have no such inventory and no item pin either;
//! `docs/contracts/policy-source-scans.md` names them and says when that
//! starts to matter. What does reach all 68 is
//! [`every_item_that_reaches_a_closed_type_is_pinned`], which is workspace-wide
//! per closed type, and [`the_items_tile_every_file_the_workspace_compiles`],
//! which is workspace-wide over files.
//!
//! And [`every_item_in_these_packages_is_pinned`] is over **declarations**: an
//! item nested inside a function body of those two packages is not in it. What
//! covers that for the four closed types is the fingerprint above; what covers
//! it for the rest of those packages is the line-anchored `impl_headers` this
//! file supplements rather than replaces. The two are complementary, and the
//! injection that showed it is the reason both are still here.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use support::{
    ITEM_KEYWORDS, Item, TestResult, crate_directories, items_of, lex, product_roots, relative,
    repository_root, resolve, restored_literals,
};

/// A type whose routes out are the subject of a sentence in a contract.
struct ClosedType {
    /// The package directory under `crates/`.
    package: &'static str,
    /// The type's name.
    name: &'static str,
    /// The pinned set of items that reach it.
    items: &'static [&'static str],
    /// The sentence the pin is what makes true.
    contract: &'static str,
}

/// The types this file holds a whole item set for.
///
/// Four came from `P2-A4`'s second audit, which measured three routes out of
/// `student-voice` that two whole-set inventories over spellings could not
/// see. The last two came from `P2-A5`'s fourth, which measured the same class
/// one crate over in each direction: `pub const STANDING_TOTAL: fn(&PromotionSet)
/// -> u32` folded section 17.6's project half and its personal half into one
/// number, and `pub const EMPHASIS: fn(&MotivationDisplay) -> u32` folded
/// section 20.3's three motivation edges into one, and each passed the whole
/// workspace. A type is here when a contract sentence says what may and may
/// not come out of it.
const CLOSED_TYPES: [ClosedType; 6] = [
    ClosedType {
        package: "student-voice",
        name: "RestrictedOriginal",
        items: &RESTRICTED_ORIGINAL_ITEMS,
        contract: "the removed speech leaves through `open`, which takes a `RawAccessGrant` \
                   by value and appends to a `RawAccessLog` before it returns",
    },
    ClosedType {
        package: "student-voice",
        name: "DisclosedOriginal",
        items: &DISCLOSED_ORIGINAL_ITEMS,
        contract: "a disclosure borrows the original, has no owned form, and no route \
                   carries it back into a derivative",
    },
    ClosedType {
        package: "student-voice",
        name: "AccuracyWitness",
        items: &ACCURACY_WITNESS_ITEMS,
        contract: "the one producer of a witness is `DiarizationMeasurement::witness`, \
                   which compares both axes against the threshold",
    },
    ClosedType {
        package: "capture-gate",
        name: "ReleasableArtifact",
        items: &RELEASABLE_ARTIFACT_ITEMS,
        contract: "`ReleasableArtifact::bytes` is the one accessor in this crate",
    },
    ClosedType {
        package: "repository-competency",
        name: "PromotionSet",
        items: &PROMOTION_SET_ITEMS,
        contract: "a project observation and a personal application are two claims and \
                   nothing folds them into one number: a reader of the number could not tell \
                   three repository observations from one observation and two applications, \
                   which is section 17.6's whole point",
    },
    ClosedType {
        package: "build-learn",
        name: "MotivationDisplay",
        items: &MOTIVATION_DISPLAY_ITEMS,
        contract: "the three motivation edges come out as rows in `MOTIVATIONS`' order and no \
                   signature folds them into a number, because a number ranks one motivation \
                   above another and section 20.3 says none is",
    },
];

/// The floor under the workspace walk.
///
/// A walk that returned nothing would satisfy every "no file holds" assertion
/// in this file. `T217` measured 568 product files at `29f66d5`.
const PRODUCT_FILE_FLOOR: usize = 500;

/// The product items of one package, read from its compilation unit.
///
/// Not from a `*.rs` walk. `compilation_unit_scans.rs` is the test that makes
/// the two equal; reading the closure here means this file does not rest on
/// that equality but on the same reader, so a `#[path]` or an `include!` that
/// reaches sideways is followed rather than missed.
fn product_items(package: &str) -> Result<Vec<Item>, Box<dyn Error>> {
    let repository = repository_root()?;
    let directory = repository.join("crates").join(package);
    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    for root in product_roots(&directory)? {
        files.extend(resolve(&root, &repository)?.files);
    }
    let mut found = Vec::new();
    for file in files {
        let name = relative(&repository, &file);
        found.extend(items_of(&name, &fs::read_to_string(&file)?)?);
    }
    Ok(found)
}

/// Every product item of every package in `crates/`.
///
/// The closed-type rule is workspace-wide rather than package-wide, and the
/// difference is not hypothetical: `crates/readiness/src/score.rs` names
/// `AccuracyWitness`, so a route written there would have been outside a rule
/// that read only the declaring package — a hole of exactly the shape this
/// file exists to close, one crate over. It contributes no entry today because
/// the mention is in a doc comment and `Item::text` is the blanked view, which
/// is the reason that view exists.
fn workspace_items() -> Result<Vec<Item>, Box<dyn Error>> {
    let repository = repository_root()?;
    let mut found = Vec::new();
    for directory in crate_directories(&repository)? {
        let package = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        found.extend(product_items(&package)?);
    }
    Ok(found)
}

/// Every product file of the workspace, with its text.
fn product_files() -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let repository = repository_root()?;
    let mut found = BTreeMap::new();
    for directory in crate_directories(&repository)? {
        for root in product_roots(&directory)? {
            for file in resolve(&root, &repository)?.files {
                let Ok(source) = fs::read_to_string(&file) else {
                    continue;
                };
                found.insert(relative(&repository, &file), source);
            }
        }
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// The reader is total over the text, and refuses what it cannot read
// ---------------------------------------------------------------------------

/// Every character the compiler reads belongs to an item this reader returned.
///
/// The completeness property, and the one a sweep over spellings cannot have.
/// `impl_headers` returns 46 headers out of a 900-line file and says nothing
/// about the other 850 lines; this says the items it returned **tile** the
/// file — every non-whitespace character is inside exactly one top-level item,
/// and nothing is inside two.
///
/// An item form nobody predicted therefore cannot be silently skipped. Either
/// the reader has a rule for it, in which case it is an item and the pins see
/// it, or it does not, in which case [`items_of`] returns `Err` and this test
/// names the file.
#[test]
fn the_items_tile_every_file_the_workspace_compiles() -> TestResult {
    let files = product_files()?;
    assert!(
        files.len() >= PRODUCT_FILE_FLOOR,
        "the walk read only {} product files",
        files.len()
    );
    let mut holes: Vec<String> = Vec::new();
    let mut overlaps: Vec<String> = Vec::new();
    let mut items = 0_usize;
    for (name, source) in &files {
        let read = items_of(name, source)?;
        items = items.saturating_add(read.len());
        let code = lex(source).code;
        let mut owners: Vec<usize> = vec![0; code.len()];
        for item in read.iter().filter(|item| item.owner.is_empty()) {
            for slot in owners
                .iter_mut()
                .take(item.end.min(code.len()))
                .skip(item.start)
            {
                *slot = slot.saturating_add(1);
            }
        }
        for (at, character) in code.iter().enumerate() {
            if character.is_whitespace() {
                continue;
            }
            if owners[at] == 0 {
                holes.push(format!("{name}: nothing covers offset {at}"));
                break;
            }
            if owners[at] > 1 {
                overlaps.push(format!("{name}: {} items cover offset {at}", owners[at]));
                break;
            }
        }
    }
    assert_eq!(
        holes,
        Vec::<String>::new(),
        "an item the reader did not see"
    );
    assert_eq!(
        overlaps,
        Vec::<String>::new(),
        "two items over one character"
    );
    assert!(
        items > 10_000,
        "the reader returned {items} items over {} files",
        files.len()
    );
    Ok(())
}

/// The reader stops at a form it has no rule for, and reads the ones it has.
///
/// The vacuity control on the test above. A reader that returned one item per
/// file covering the whole file would tile every file and see nothing.
#[test]
fn the_reader_refuses_an_item_form_it_has_no_rule_for() -> TestResult {
    // One of every kind the enumeration names, in one file, plus the three
    // forms `P2-A4`'s second audit walked two whole-set inventories past.
    //
    // It is text handed to the reader and is never compiled, which is why it
    // can carry `pub macro twice() {}`: declarative macros 2.0 are unstable and
    // this repository builds on stable, but `macro` is on the keyword list and
    // a control that skipped it would be a control over thirteen of fourteen.
    let sample = concat!(
        "#![allow(dead_code)]\n",
        "extern crate alloc;\n",
        "use core::fmt;\n",
        "pub mod inner { pub fn nested() {} }\n",
        "mod terse;\n",
        "pub type Alias = u8;\n",
        "pub struct Unit;\n",
        "pub struct Tuple(u8);\n",
        "pub struct Braced { field: u8 }\n",
        "pub enum Choice { One }\n",
        "pub union Overlaid { one: u8 }\n",
        "pub const NUMBER: u8 = 1;\n",
        "pub static mut COUNT: u8 = 0;\n",
        "pub trait Named { const TAG: u8; type Out; fn call(&self) -> Self::Out; }\n",
        "impl Named for Unit { const TAG: u8 = 1; type Out = u8; fn call(&self) -> u8 { 1 } }\n",
        "unsafe extern \"C\" { fn abroad(value: u8) -> u8; }\n",
        "macro_rules! shout { () => {}; }\n",
        "macro_rules! whisper ( () => {}; );\n",
        "macro_rules! yell [ () => {}; ];\n",
        "unsafe extern \"C\" { pub safe fn nearby(value: u8) -> u8; }\n",
        "pub macro twice() {}\n",
        "shout!();\n",
        "pub const POINTER: fn(&Unit) -> u8 = |_| 1;\n",
        "#[allow(unused)] impl From<&Unit> for u8 { fn from(_: &Unit) -> Self { 1 } }\n",
        "pub async unsafe fn later() {}\n",
    );
    let read = items_of("sample.rs", sample)?;
    let kinds: BTreeSet<&str> = read.iter().map(|item| item.kind.as_str()).collect();
    let mut expected: BTreeSet<&str> = ITEM_KEYWORDS.into_iter().collect();
    expected.insert("macro-call");
    expected.insert("attribute");
    assert_eq!(
        kinds, expected,
        "the sample does not exercise every kind the enumeration names"
    );

    // The three forms, each present as an item with its own key.
    let keys: Vec<String> = read.iter().map(|item| item.key()).collect();
    for wanted in [
        "sample.rs [pub] const POINTER: fn(&Unit) -> u8",
        "sample.rs [priv] #[allow(unused)] impl From<&Unit> for u8",
        "sample.rs [priv] macro_rules! shout",
        "sample.rs [priv] shout!()",
        // The three forms `P2-A5` measured this reader refusing while
        // `rustc --edition 2024` compiled them. Only the brace form of
        // `macro_rules` omits the terminating `;`, and `safe` on a foreign
        // function is stable in this workspace's own edition.
        "sample.rs [priv] macro_rules! whisper",
        "sample.rs [priv] macro_rules! yell",
        "sample.rs [pub] unsafe extern \"C\" :: safe fn nearby(value: u8) -> u8",
    ] {
        assert!(
            keys.iter().any(|key| key == wanted),
            "the reader did not return `{wanted}`; it returned {keys:?}"
        );
    }
    // The attribute is part of the item, so the impl above is not two items
    // and the `impl` is not read as a line.
    assert_eq!(
        read.iter().filter(|item| item.kind == "impl").count(),
        2,
        "an attribute in front of an impl split it"
    );

    // Default-deny. None of these is an item, and each stops the reader.
    //
    // The last two are the control on the repair above: `P2-A5` asked which
    // of the forms this reader refuses are legal Rust, and these two are the
    // ones that are **not**. `rustc` answers `error: expected item, found ';'`
    // for the first, and `gen` is unstable while this workspace builds on
    // stable, so admitting either to make the reader more permissive would be
    // the wrong direction.
    for refused in [
        "pub struct Held;\nHeld;\n",
        "pub oddity Thing { }\n",
        "pub fn ready() {}\n7\n",
        "pub struct Held {};\n",
        "pub gen fn later() {}\n",
    ] {
        let outcome = items_of("refused.rs", refused);
        assert!(
            outcome.is_err(),
            "the reader accepted a construct that is not an item: {refused:?}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The pins
// ---------------------------------------------------------------------------

/// Nothing a scanned package compiles is outside the pinned item set.
///
/// The backstop. It is keyed on nothing at all: not a visibility, not a
/// keyword, not a type name. An item added anywhere in one of these packages
/// is an extra entry whatever it is, which is the statement `P2-A4`'s second
/// audit found three counterexamples to in one afternoon and `P2-A5`'s fourth
/// found two more, in `repository-competency` and `build-learn`, through the
/// `pub const NAME: fn(&T) -> u32` form.
///
/// **Which packages.** Derived, not written down: every package that holds a
/// file [`the_inventories_still_keyed_on_a_line_prefix_are_named`] reports,
/// which is exactly the set whose contract rests on a reader that cannot see
/// an item form. A package that grows such a collector has to grow a pin file
/// with it, and the failure names the package.
///
/// **Where the pins live.** `crates/contracts/tests/pinned-items/<package>.items`,
/// one [`Item::key`] to a line, sorted. Six thousand keys are a table rather
/// than a source file: a product edit shows up in `git diff` as the lines it
/// added, which is the review this pin exists to force.
#[test]
fn every_item_in_these_packages_is_pinned() -> TestResult {
    let repository = repository_root()?;
    let packages = packages_keyed_on_a_line_prefix(&repository)?;
    assert!(
        packages.len() >= PINNED_PACKAGE_FLOOR,
        "the derivation found only {} packages to pin",
        packages.len()
    );
    let empty: [&str; 0] = [];
    let mut total = 0_usize;
    for package in &packages {
        let items = product_items(package)?;
        let mut keys: Vec<String> = items.iter().map(Item::key).collect();
        keys.sort();
        total = total.saturating_add(keys.len());
        let pinned = pinned_items(&repository, package)?;
        let held: BTreeSet<&str> = pinned.iter().map(String::as_str).collect();
        let read: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
        let extra: Vec<&str> = read.difference(&held).copied().collect();
        let gone: Vec<&str> = held.difference(&read).copied().collect();
        assert_eq!(
            extra, empty,
            "`academic-{package}` compiles items nobody wrote down"
        );
        assert_eq!(
            gone, empty,
            "`academic-{package}` no longer compiles items that are pinned"
        );
        assert_eq!(keys, pinned, "the item set of `academic-{package}` changed");
    }
    assert!(
        total >= PINNED_ITEM_FLOOR,
        "the pinned packages hold only {total} items"
    );
    Ok(())
}

/// The file holding one package's pinned item set.
fn pin_path(repository: &Path, package: &str) -> PathBuf {
    repository
        .join("crates/contracts/tests/pinned-items")
        .join(format!("{package}.items"))
}

/// The keys pinned for one package, one to a line, in file order.
///
/// A missing file is an error naming the path rather than an empty set: a pin
/// that reads as empty would make every assertion above vacuous, which is this
/// Run's dominant defect.
fn pinned_items(repository: &Path, package: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let path = pin_path(repository, package);
    let text = fs::read_to_string(&path)
        .map_err(|why| format!("{}: {why}", relative(repository, &path)))?;
    let held: Vec<String> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect();
    if held.is_empty() {
        return Err(format!("{} pins nothing", relative(repository, &path)).into());
    }
    Ok(held)
}

/// The floor under the derivation, so an empty walk cannot satisfy the pin.
const PINNED_PACKAGE_FLOOR: usize = 23;

/// The floor under the pinned item count, measured at `4ac7701`: 6131.
const PINNED_ITEM_FLOOR: usize = 6_000;

/// Every route to a closed type is one somebody wrote down.
///
/// This is the rule that fails **by name**. `every_item_in_these_packages_is_pinned`
/// would catch the same injection, but it would report it as an item nobody
/// wrote down; this reports it as a new route to `RestrictedOriginal`, which
/// is the sentence the reader has to go and check.
#[test]
fn every_item_that_reaches_a_closed_type_is_pinned() -> TestResult {
    let workspace = workspace_items()?;
    for closed in &CLOSED_TYPES {
        // The names the type is reachable under, closed over aliasing. A rule
        // keyed on one name is a rule about a spelling until this runs: an item
        // written against `type Removed = RestrictedOriginal;` names `Removed`
        // and not the closed type. `P2-A5`'s F4 recorded this closure as
        // documented and called by nothing.
        let mut names: BTreeSet<String> = BTreeSet::from([closed.name.to_owned()]);
        loop {
            let grown: BTreeSet<String> = workspace
                .iter()
                .filter(|item| names.iter().any(|name| item.reaches(name)))
                .flat_map(Item::introduced_type_names)
                .filter(|name| !name.is_empty())
                .collect();
            let before = names.len();
            names.extend(grown);
            if names.len() == before {
                break;
            }
        }
        let mut keys: Vec<String> = workspace
            .iter()
            .filter(|item| names.iter().any(|name| item.reaches(name)))
            .map(Item::sealed_key)
            .collect();
        keys.sort();
        keys.dedup();
        assert_eq!(
            keys,
            closed
                .items
                .iter()
                .map(|entry| (*entry).to_owned())
                .collect::<Vec<_>>(),
            "the items that reach `{}`, declared in {}, changed; the contract on them is: {}",
            closed.name,
            closed.package,
            closed.contract
        );
    }
    Ok(())
}

/// Every `crates/*/tests/**.rs` file that keys a collector on a line prefix.
///
/// Derived from the code of each file -- comments blanked and string bodies
/// restored -- rather than written down, so a file that grows such a collector
/// arrives here and one rewritten off the shape leaves.
fn files_keyed_on_a_line_prefix(repository: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let markers = [
        format!("starts_with({}pub fn {})", '"', '"'),
        format!("starts_with({}pub const fn {})", '"', '"'),
        format!("starts_with({}impl {})", '"', '"'),
    ];
    let mut found: Vec<String> = Vec::new();
    for directory in crate_directories(repository)? {
        let mut pending = vec![directory.join("tests")];
        while let Some(current) = pending.pop() {
            let Ok(entries) = fs::read_dir(&current) else {
                continue;
            };
            for entry in entries {
                let path = entry?.path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                if !path.extension().is_some_and(|value| value == "rs") {
                    continue;
                }
                let source = fs::read_to_string(&path)?;
                // Read as code with the literal bodies written back: this file
                // names the three prefixes itself, and a raw-text check would
                // fire on the scan rather than on what it scans -- which is
                // why the markers above are assembled rather than written.
                let code: String = restored_literals(&source).into_iter().collect();
                if markers.iter().any(|marker| code.contains(marker.as_str())) {
                    found.push(relative(repository, &path));
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

/// The packages those files belong to, deduplicated.
fn packages_keyed_on_a_line_prefix(repository: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for file in files_keyed_on_a_line_prefix(repository)? {
        let package = file
            .strip_prefix("crates/")
            .and_then(|rest| rest.split('/').next())
            .ok_or_else(|| format!("{file} is not under crates/"))?;
        found.insert(package.to_owned());
    }
    Ok(found.into_iter().collect())
}

/// What is still keyed on a line prefix, enumerated.
///
/// `P2-A4`'s F2 records that the gap the item reader closes is in all six
/// `P2-L` packages by construction, and reading for the collector's own shape
/// says it is in seventeen more. Every one of them now carries a whole-set
/// item pin as well ([`every_item_in_these_packages_is_pinned`] derives its
/// package list from this one), so the list is no longer the remainder -- it
/// is the derivation. What it still records is that these files read a line:
/// the two readers are complementary, because a line-anchored `impl_headers`
/// sees inside a function body and an item key does not.
#[test]
fn the_inventories_still_keyed_on_a_line_prefix_are_named() -> TestResult {
    let repository = repository_root()?;
    let found = files_keyed_on_a_line_prefix(&repository)?;
    assert_eq!(
        found,
        INVENTORIES_KEYED_ON_A_LINE_PREFIX
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "the set of inventories keyed on a line prefix changed"
    );
    // Every one of them has a pin file, and every pin file names a package
    // that is on this list. The second direction is what stops a pin being
    // added for a package nobody scans and counted as coverage.
    let packages = packages_keyed_on_a_line_prefix(&repository)?;
    for package in &packages {
        let path = pin_path(&repository, package);
        assert!(
            path.is_file(),
            "{} keys an inventory on a line prefix and has no item pin at {}",
            package,
            relative(&repository, &path)
        );
    }
    let mut pinned: Vec<String> = Vec::new();
    for entry in fs::read_dir(repository.join("crates/contracts/tests/pinned-items"))? {
        let path = entry?.path();
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        pinned.push(name);
    }
    pinned.sort();
    assert_eq!(
        pinned, packages,
        "a pin file names a package that keys no inventory on a line prefix"
    );
    Ok(())
}

/// One reader, copied sixteen times, and the copies are held to being one.
///
/// `P2-R2` repaired a reach guard that a token list walked past; `P2-A5`
/// measured the repaired one walked past by
/// `<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)`, which resolves
/// a name. The helper it repaired is copied into every crate that scans its own
/// reaches, and the audit counted fourteen with `crates/*/tests/*.rs`; there are
/// **sixteen**, because `academic-integrations` and `academic-next-lecture` keep
/// theirs one directory further down in `tests/support/mod.rs`.
///
/// **Why they are not one function.** They could be: a dev-dependency crate
/// holding the helpers would give one copy. It would also add an edge to the
/// dependency closure of sixteen crates, and that closure is the subject of
/// `tools/phase1-scaffold-policy.test.mjs`'s dependency map, its acyclic graph
/// and each crate's own `USE_ITEMS` inventory -- scans whose whole point is
/// that the closure does not move. Paying in the thing being protected to
/// deduplicate the thing protecting it is the wrong trade, and the copy is
/// deliberate for a second reason the crates state: `P2-G4` found that a lexer
/// without raw strings desynchronizes, so each crate carries a lexer it can
/// read rather than one it imports.
///
/// **What replaces one copy.** This: the bodies are compared against each
/// other, so sixteen copies are one text, and every carrier crate is required
/// to hold the driving control. One driven copy plus textual identity is the
/// same guarantee as one function, and a seventeenth copy that arrives with the
/// old body fails here by name instead of waiting for an audit.
#[test]
fn the_reach_readers_are_one_reader() -> TestResult {
    let repository = repository_root()?;
    // Assembled, and read from the blanked view, so this file's own mention of
    // the name is neither a carrier nor a match.
    let declaration = format!("fn absolute{}paths(code: &str)", '_');
    let control = format!(
        "{}::net::ToSocketAddrs>::to{}socket{}addrs",
        "std", '_', '_'
    );
    let mut carriers: Vec<String> = Vec::new();
    let mut bodies: Vec<(String, String)> = Vec::new();
    let mut crates_with_a_control: BTreeSet<String> = BTreeSet::new();
    for directory in crate_directories(&repository)? {
        let package = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        let mut pending = vec![directory.join("tests")];
        while let Some(current) = pending.pop() {
            let Ok(entries) = fs::read_dir(&current) else {
                continue;
            };
            for entry in entries {
                let path = entry?.path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                if !path.extension().is_some_and(|value| value == "rs") {
                    continue;
                }
                let source = fs::read_to_string(&path)?;
                let code: String = lex(&source).code.into_iter().collect();
                // The control is an argument, so it lives in a literal and the
                // blanked view has erased it; the declaration is code, so the
                // blanked view is what keeps this file from matching itself.
                let text: String = restored_literals(&source).into_iter().collect();
                if text.contains(control.as_str()) {
                    crates_with_a_control.insert(package.clone());
                }
                let Some(at) = code.find(declaration.as_str()) else {
                    continue;
                };
                let name = relative(&repository, &path);
                carriers.push(name.clone());
                let tail = &code[at..];
                let end = tail
                    .find("\n}\n")
                    .ok_or_else(|| format!("{name}: the reader has no closing brace"))?;
                // Two lines legitimately differ and neither is the rule: the
                // crate root list, and whatever a blanked comment leaves behind
                // -- `lex` blanks in place, so a longer comment is a longer run
                // of spaces. Comparing trimmed non-empty lines is therefore a
                // comparison of the code and nothing else.
                let body: String = tail[..end]
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty() && !line.starts_with("let roots = ["))
                    .collect::<Vec<_>>()
                    .join("\n");
                bodies.push((name, body));
            }
        }
    }
    carriers.sort();
    assert_eq!(
        carriers,
        REACH_READERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "the set of files holding a reach reader changed"
    );
    let (first, canonical) = bodies
        .first()
        .cloned()
        .ok_or("no reach reader was read at all")?;
    let differing: Vec<&str> = bodies
        .iter()
        .filter(|(_, body)| *body != canonical)
        .map(|(name, _)| name.as_str())
        .collect();
    let empty: [&str; 0] = [];
    assert_eq!(
        differing, empty,
        "these reach readers are not the same text as {first}"
    );
    assert!(
        canonical.contains("if start < taken {"),
        "the canonical reach reader decides a middle segment on a byte offset again"
    );
    let carrier_crates: BTreeSet<String> = carriers
        .iter()
        .filter_map(|file| file.strip_prefix("crates/"))
        .filter_map(|rest| rest.split('/').next())
        .map(str::to_owned)
        .collect();
    let undriven: Vec<&str> = carrier_crates
        .iter()
        .filter(|package| !crates_with_a_control.contains(*package))
        .map(String::as_str)
        .collect();
    assert_eq!(
        undriven, empty,
        "these crates copy the reach reader and drive nothing through the form that bypassed it"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The pinned sets
// ---------------------------------------------------------------------------

/// Every item that reaches a `RestrictedOriginal`.
///
/// Sixteen. The struct and its `impl`, the six read-only accessors, `open`,
/// the grant that `open` consumes and the constructor that binds one, the
/// `Redaction` that holds an original and the `redact` that returns one, and
/// the re-export that makes the type public. Exactly one of them returns the
/// removed speech and it is `open`, which takes the grant **by value** and
/// appends to the log before it returns.
const RESTRICTED_ORIGINAL_ITEMS: [&str; 16] = [
    "crates/student-voice/src/derivative.rs [priv] impl RawAccessGrant",
    "crates/student-voice/src/derivative.rs [priv] impl Redaction",
    "crates/student-voice/src/derivative.rs [priv] impl RestrictedOriginal",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct Redaction |ffdd23c51bd7b565",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct RestrictedOriginal |972ae59264bb72ac",
    "crates/student-voice/src/derivative.rs [pub] fn redact( plan: &RedactionPlan, reference: &RedactionPolicyRef, source: &LectureSource<'_>, requested: RetentionTerms, ) -> Result<Redaction, RedactionFault> |c2aa9a77fda6aec6",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessGrant :: fn issued( original: &RestrictedOriginal, requested_by: Actor, purpose: &str, at: u64, ) -> Result<Self, AccessRefusal> |83570c8d297cd3c3",
    "crates/student-voice/src/derivative.rs [pub] impl Redaction :: #[must_use] const fn original(&self) -> &RestrictedOriginal |a7b7e25b10cc1bd5",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn classification(&self) -> &'static str |6337ea6081699b55",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn digest(&self) -> &ContentDigest |6f9c919d8ee114e4",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn lecture(&self) -> LectureSessionId |bd075c887d3d5d9c",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn source_version(&self) -> u32 |aa69af4c9348861f",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn terms(&self) -> RetentionTerms |b79645f296391abe",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] fn removed_count(&self) -> usize |7572e56ac85aad9c",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: fn open( &self, grant: RawAccessGrant, log: &mut RawAccessLog, ) -> Result<DisclosedOriginal<'_>, AccessRefusal> |a51c73ec10b66572",
    "crates/student-voice/src/lib.rs [pub] use derivative::{ DerivedArtifact, DisclosedOriginal, ExclusionRecord, KeptUtterance, LectureSource, ManualExclusion, ORIGINAL_CLASSIFICATION, RawAccessGrant, RawAccessLog, RawAccessRecord, RedactedDerivative, Redaction, RedactionMode, RedactionPlan, RestrictedOriginal, SourceUtterance, inherit_terms, redact, } |37ae7633d17ed02b",
];

/// Every item that reaches a `DisclosedOriginal`.
///
/// Ten. `open` is where one comes from; the rest are the reads a holder may
/// make, each by position rather than in bulk, and the re-export. There is no
/// owned form and no route back into a derivative.
const DISCLOSED_ORIGINAL_ITEMS: [&str; 10] = [
    "crates/student-voice/src/derivative.rs [priv] impl DisclosedOriginal<'_>",
    "crates/student-voice/src/derivative.rs [priv] impl RestrictedOriginal",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, PartialEq, Eq)] struct DisclosedOriginal<'a> |775891ec1cfd006e",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] const fn is_empty(&self) -> bool |35d5d87bd2b25e5e",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] const fn len(&self) -> usize |9e2d9b5eac5c14a2",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn source_index(&self, position: usize) -> Option<usize> |8ea035a19872b839",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn speaker(&self, position: usize) -> Option<Speaker> |ca6a899119946e9e",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn verbatim(&self, position: usize) -> Option<&str> |4c29c83c60552899",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: fn open( &self, grant: RawAccessGrant, log: &mut RawAccessLog, ) -> Result<DisclosedOriginal<'_>, AccessRefusal> |a51c73ec10b66572",
    "crates/student-voice/src/lib.rs [pub] use derivative::{ DerivedArtifact, DisclosedOriginal, ExclusionRecord, KeptUtterance, LectureSource, ManualExclusion, ORIGINAL_CLASSIFICATION, RawAccessGrant, RawAccessLog, RawAccessRecord, RedactedDerivative, Redaction, RedactionMode, RedactionPlan, RestrictedOriginal, SourceUtterance, inherit_terms, redact, } |37ae7633d17ed02b",
];

/// Every item that reaches an `AccuracyWitness`.
///
/// Eighteen. One produces a witness -- `DiarizationMeasurement::witness`,
/// which compares both axes against the threshold -- one consumes it by value
/// into an automatic redaction mode, one hands back a reference to the one a
/// plan already holds, and the rest are the witness's own read-only accessors
/// and the types that carry it.
const ACCURACY_WITNESS_ITEMS: [&str; 18] = [
    "crates/student-voice/src/derivative.rs [priv] impl RedactionMode",
    "crates/student-voice/src/derivative.rs [priv] impl RedactionPlan",
    "crates/student-voice/src/derivative.rs [priv] use crate::{ fault::{AccessRefusal, RedactionFault}, measure::AccuracyWitness, policy::RedactionPolicy, } |adf116db77df2eae",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum RedactionMode |b3f0693a45b96031",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionMode :: #[must_use] const fn witness(&self) -> Option<&AccuracyWitness> |e159a494bd523327",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionPlan :: #[must_use] fn automatic(policy: RedactionPolicy, witness: AccuracyWitness) -> Self |965a2c7345c9d096",
    "crates/student-voice/src/lib.rs [pub] use measure::{ ABSOLUTE_ACCURACY_FLOOR, ABSOLUTE_MISSED_STUDENT_CEILING, AccuracyWitness, CaseMeasurement, DIARIZATION_THRESHOLD_V1, DiarizationMeasurement, DiarizationThreshold, SCORER_VERSION, measure, measure_case, } |3613d4518178ec0e",
    "crates/student-voice/src/measure.rs [priv] impl AccuracyWitness",
    "crates/student-voice/src/measure.rs [priv] impl DiarizationMeasurement",
    "crates/student-voice/src/measure.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct AccuracyWitness |a7ad39d88a79e114",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn accuracy_permille(&self) -> u64 |edcc8baac5268490",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn corpus_digest(&self) -> &ContentDigest |bdaee52689365efe",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn corpus_version(&self) -> u32 |02e7c461e14c31eb",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn missed_student_permille(&self) -> u64 |0e209ec800e8346c",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn scorer_version(&self) -> u32 |df8aba4bf6231867",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn threshold(&self) -> DiarizationThreshold |df7ad132e787824c",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] fn corpus_id(&self) -> &str |a1f51e08ddf65289",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: fn witness( &self, threshold: DiarizationThreshold, ) -> Result<AccuracyWitness, AccuracyRefusal> |3747a8b57bbf08a7",
];

/// Every item that reaches a `ReleasableArtifact`.
///
/// Eleven, and two of them are accessors: `bytes`, which the crate's note
/// calls the only one, and `manifest`, which hands out no bytes. A third
/// written in any form -- a `pub fn`, a trait impl, a `Deref`, a `pub const`
/// holding a function pointer, an item a macro expands to -- is a twelfth
/// entry here, because the rule is over the items of the compilation unit and
/// not over the spellings a collector was taught.
const RELEASABLE_ARTIFACT_ITEMS: [&str; 11] = [
    "crates/capture-gate/src/artifact.rs [priv] impl CaptureArtifact",
    "crates/capture-gate/src/artifact.rs [priv] impl ReleasableArtifact",
    "crates/capture-gate/src/artifact.rs [priv] impl fmt::Debug for ReleasableArtifact",
    "crates/capture-gate/src/artifact.rs [priv] impl fmt::Debug for ReleasableArtifact :: fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result |5d5a884dd8b40afc",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl CaptureArtifact :: const fn releasable(manifest: CaptureManifest, bytes: Vec<u8>) -> Self |732d2d152c6adf48",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Clone, PartialEq, Eq)] struct ReleasableArtifact |56e39015c24901ba",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum CaptureArtifact |f806e5314858e29b",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureArtifact :: #[must_use] const fn as_releasable(&self) -> Option<&ReleasableArtifact> |5a55f780817bfa34",
    "crates/capture-gate/src/artifact.rs [pub] impl ReleasableArtifact :: #[must_use] const fn manifest(&self) -> &CaptureManifest |5048569a0c02e48a",
    "crates/capture-gate/src/artifact.rs [pub] impl ReleasableArtifact :: #[must_use] fn bytes(&self) -> &[u8] |a3abc5b93da5a4c9",
    "crates/capture-gate/src/lib.rs [pub] use artifact::{ CaptureArtifact, CaptureManifest, ChunkRecord, PERMISSION_VIOLATION_RISK, QuarantinedArtifact, ReleasableArtifact, TimelineGap, ViolationRisk, } |5f1c50d32a5909a1",
];

/// Every file holding a copy of the reach reader.
///
/// Sixteen, not the fourteen `P2-A5` counted: `crates/*/tests/*.rs` does not
/// reach `tests/support/mod.rs`, and two crates keep theirs there.
const REACH_READERS: [&str; 16] = [
    "crates/blind-spot/tests/blind_spot_scans.rs",
    "crates/build-learn/tests/build_learn_scans.rs",
    "crates/competency/tests/competency_scans.rs",
    "crates/critical-path/tests/critical_path_scans.rs",
    "crates/cs-map/tests/cs_map_scans.rs",
    "crates/freshness/tests/freshness_scans.rs",
    "crates/gap/tests/gap_scans.rs",
    "crates/integrations/tests/support/mod.rs",
    "crates/knowledge-state/tests/knowledge_state_scans.rs",
    "crates/next-lecture/tests/support/mod.rs",
    "crates/readiness/tests/readiness_scans.rs",
    "crates/repository-analysis/tests/analysis_scans.rs",
    "crates/repository-classification/tests/classification_scans.rs",
    "crates/repository-competency/tests/competency_scans.rs",
    "crates/repository-correlation/tests/correlation_scans.rs",
    "crates/role-profile/tests/role_scans.rs",
];

/// Every item of the workspace that reaches `PromotionSet`.
///
/// Section 17.6's two claim kinds live behind this type, and the contract on
/// it is that they stay two. `P2-A5` measured `pub const STANDING_TOTAL:
/// fn(&PromotionSet) -> u32` counting the project claims and the personal
/// claims into one number and passing the whole workspace: `impl_headers`
/// keeps a line beginning `impl` and `public_signatures` keeps `pub fn ` and
/// `pub const fn `, and a `pub const NAME: fn(...) -> ...` is neither. It is
/// an item, so it is here.
const PROMOTION_SET_ITEMS: [&str; 8] = [
    "crates/repository-competency/src/lib.rs [priv] impl PromotionSet",
    "crates/repository-competency/src/lib.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct PromotionSet |6212f72bcfffdb3f",
    "crates/repository-competency/src/lib.rs [pub] fn promote(input: &PromotionInput<'_>) -> Result<PromotionSet, CompetencyError> |bbfd7ea14f2aec1d",
    "crates/repository-competency/src/lib.rs [pub] impl PromotionSet :: #[must_use] fn personal_claim(&self, concept: &str) -> Option<&PersonalApplicationClaim> |b1f69778b0d1b76d",
    "crates/repository-competency/src/lib.rs [pub] impl PromotionSet :: #[must_use] fn personal_claims(&self) -> &[PersonalApplicationClaim] |42ca5bebcb9604c2",
    "crates/repository-competency/src/lib.rs [pub] impl PromotionSet :: #[must_use] fn project_claim(&self, concept: &str) -> Option<&ProjectObservationClaim> |93bbe3e16bc440b0",
    "crates/repository-competency/src/lib.rs [pub] impl PromotionSet :: #[must_use] fn project_claims(&self) -> &[ProjectObservationClaim] |91559c5182a212d1",
    "crates/repository-competency/src/lib.rs [pub] impl PromotionSet :: #[must_use] fn snapshot_id(&self) -> &str |f3d2b3f8490921f9",
];

/// Every item of the workspace that reaches `MotivationDisplay`.
///
/// `no_signature_folds_the_motivation_edges` compares the whole set of public
/// signatures that name a motivation type and return a number against the
/// empty set, and that set is whole over the same line reader's output.
/// `P2-A5` measured `pub const EMPHASIS: fn(&MotivationDisplay) -> u32`
/// adding no key to it, because the reader never emits one for a `const`.
/// This set is over items, so the form is in it whatever it is called.
const MOTIVATION_DISPLAY_ITEMS: [&str; 11] = [
    "crates/build-learn/src/lib.rs [pub] use motivation::{MOTIVATIONS, Motivation, MotivationDisplay, MotivationEdge, MotivationRow} |a53ab0bdd3111e1a",
    "crates/build-learn/src/motivation.rs [priv] impl MotivationDisplay",
    "crates/build-learn/src/motivation.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] struct MotivationDisplay |acf73a31544c9eed",
    "crates/build-learn/src/motivation.rs [pub] impl MotivationDisplay :: #[must_use] const fn concept(&self) -> EntityId |a234d26f3e4fe82b",
    "crates/build-learn/src/motivation.rs [pub] impl MotivationDisplay :: #[must_use] fn carries(&self, motivation: Motivation) -> bool |5476beea5e5903d7",
    "crates/build-learn/src/motivation.rs [pub] impl MotivationDisplay :: #[must_use] fn rows(&self) -> &[MotivationRow] |dd0ea27f50a8481e",
    "crates/build-learn/src/motivation.rs [pub] impl MotivationDisplay :: fn of(concept: EntityId, edges: &[MotivationEdge]) -> Result<Self, BuildLearnError> |fa217152b4d514c9",
    "crates/build-learn/src/plan.rs [priv] impl PlanDraft<'_>",
    "crates/build-learn/src/plan.rs [priv] use crate::{ branch::ArchitectureBranch, learning::LearningItem, motivation::MotivationDisplay, readiness::ReadinessFinding, text::{NonEmptyText, PartId}, } |eeca5926216b9579",
    "crates/build-learn/src/plan.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct PlanDraft<'a> |2f0b8d43d196542a",
    "crates/build-learn/src/plan.rs [pub] impl PlanDraft<'_> :: #[must_use] fn motivation(&self, concept: EntityId) -> Option<&MotivationDisplay> |bf9b3c0c6c1b148a",
];

/// Every scan file that still keys an inventory on a line prefix.
///
/// Twenty-four files in twenty-three packages hold a collector that reads a
/// line and keeps it when it starts with `pub fn `, `pub const fn ` or
/// `impl `. Each is the shape `P2-A4`'s second audit walked past, and this
/// task did not rewrite them: it built the item reader and moved the two
/// packages whose contract sentences the audit measured broken.
///
/// The list is here so the remaining exposure is something somebody has to
/// edit rather than a silence. A file that grows such a collector fails as an
/// extra key; one that is rewritten onto the item reader fails as a missing
/// one, which is how it comes off the list.
///
/// Derived, not asserted: the set is every `crates/*/tests/**.rs` whose code
/// -- comments blanked, string bodies restored -- holds one of the three
/// prefixes as a `starts_with` argument.
const INVENTORIES_KEYED_ON_A_LINE_PREFIX: [&str; 24] = [
    "crates/build-learn/tests/build_learn_scans.rs",
    "crates/capture-gate/tests/capture_scans.rs",
    "crates/capture/tests/capture_scans.rs",
    "crates/cs-map/tests/cs_map_scans.rs",
    "crates/deletion/tests/deletion_scans.rs",
    "crates/evidence-center/tests/evidence_center_scans.rs",
    "crates/home/tests/home.rs",
    "crates/ingestion/tests/ingestion_scans.rs",
    "crates/keystore-platform/tests/facade.rs",
    "crates/lecture-document/tests/lecture_document_scans.rs",
    "crates/non-delegable/tests/non_delegable_scans.rs",
    "crates/offering/tests/offering_scans.rs",
    "crates/process-sandbox/tests/scans.rs",
    "crates/proposal/tests/proposal_scans.rs",
    "crates/readiness/tests/readiness_matrix.rs",
    "crates/readiness/tests/readiness_scans.rs",
    "crates/repository-analysis/tests/analysis_scans.rs",
    "crates/repository-classification/tests/classification_scans.rs",
    "crates/repository-competency/tests/competency_scans.rs",
    "crates/repository-correlation/tests/correlation_scans.rs",
    "crates/repository/tests/repository_scans.rs",
    "crates/requirement/tests/requirement_scans.rs",
    "crates/student-voice/tests/student_voice_scans.rs",
    "crates/transcription/tests/transcription_scans.rs",
];
