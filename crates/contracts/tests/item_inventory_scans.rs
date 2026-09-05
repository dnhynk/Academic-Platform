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
//!   files of the workspace, disjointly: every character belongs to exactly
//!   one **top-level** item. An item the reader missed *at top level* would
//!   leave a hole, and the hole is what the test reads. An item written inside
//!   a leaf's body leaves **no** hole — the leaf's own extent already covers
//!   it, and the reader does not descend into a body — so the tiling is total
//!   over the text and not over the items. What carries a body is the
//!   fingerprint in the next section, and `P2-A4`'s third audit is what the
//!   difference cost while only one of the two rules took one. A spelling
//!   sweep can never have even the text property: `signatures_in_blocks`
//!   covers a few dozen lines of a 900-line file and nothing says what the
//!   rest of it is.
//!
//! # What is pinned
//!
//! Two rules, and the first is the backstop under the second.
//!
//! * [`every_item_in_these_packages_is_pinned`] — the **whole item set** of
//!   every package that keys an inventory on a line prefix **or owns a closed
//!   type**: 25 packages, 6792 items, derived from
//!   [`the_inventories_still_keyed_on_a_line_prefix_are_named`] and from
//!   [`CLOSED_TYPES`] rather than written down, and taken as a union by
//!   [`packages_that_need_an_item_pin`]. Keyed on nothing: an item added
//!   anywhere in one of them fails whatever it is called and whatever kind it
//!   is. The pins are in `pinned-items/<package>.items`, one key to a line.
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
//! **Both rules pin [`support::Item::sealed_key`] rather than
//! [`support::Item::key`]**, and the difference is a measured hole: a key is a
//! declaration, an `impl` block written inside a function body is globally
//! effective Rust, and this reader does not descend into a body. A leaf
//! carries a fingerprint of its own text; a container does not, because its
//! contents are enumerated as items of their own.
//!
//! The whole-set rule read `key` until `P2-A4`'s third audit wrote
//! `#[rustfmt::skip] #[allow(non_local_definitions)] impl From<&Redaction> for
//! Vec<String>` into the body of an existing method of
//! `crates/student-voice/src/derivative.rs`. That handed three students'
//! removed utterances to any crate in the workspace with no `RawAccessGrant`
//! and no `RawAccessLog` row, and it passed both rules here, `impl_headers`
//! one file over, `cargo fmt`, `pnpm test` and
//! `cargo clippy --workspace --all-targets -- -D warnings` on both hosts: the
//! item count was 376 before and after and every key was byte-identical. The
//! closed-type rule was blind for a second reason — `Redaction` is not one of
//! the six [`CLOSED_TYPES`], so `reaches` was `false` and that rule never
//! looked at the item at all.
//!
//! # What this does not claim
//!
//! The pin reaches **every package the workspace compiles**, and that number
//! is **asserted** by [`the_pin_names_what_it_covers_and_what_it_does_not`]
//! rather than written here, because `P2-A5`'s fifth audit found this
//! paragraph claiming 45 unpinned crates and 68 in the workspace where the
//! tree held 47 and 70, and recorded that it survived precisely because
//! nothing asserted either number. It moved again under `P2-RF29`, `P2-X3` and
//! `P2-RF30` while that was true. Since `P2-RF31` there is no unpinned
//! remainder to keep in step: the selection is the crate walk, the two
//! derivations that used to select are a control on that walk, and the
//! remainder is asserted empty by name.
//!
//! A body is therefore in the pin for every package. What a body is **not** is
//! whatever `#[allow(non_local_definitions)]` switches off: written on a
//! declaration the attribute is part of the key already, written inside a body
//! it is not, and it removes the one thing between a body and a globally
//! effective trait impl. [`every_item_that_reaches_a_closed_type_is_pinned`]
//! is what is over the whole workspace *per type* rather than per package, and
//! [`the_items_tile_every_file_the_workspace_compiles`] is what is over files.
//!
//! A fingerprint is over [`support::Item::text`] **and**
//! [`support::Item::literals`]. The first is the view with comments and
//! literal bodies blanked, which is what makes a name found in it a name the
//! compiler sees rather than one a doc comment mentions; the second carries
//! the literal values themselves, length-prefixed, because the first erases a
//! literal's content *and* its length. `P2-A5`'s sixth audit measured what
//! that cost: the two domain-separation constants of
//! `crates/repository-competency` were set equal to one another and the whole
//! workspace stayed green on both hosts. That form now fails here naming the
//! constant, and so does exchanging one byte of a character class for another.
//! **A comment is still free and so is whitespace** — [`support::Item::text`]
//! is the whitespace-collapsed blanked view, so a reflow moves no key, which
//! is measured rather than assumed and is the reason a pin line is worth
//! reading when it does move.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use support::{
    ITEM_KEYWORDS, ITEM_MODIFIERS, Item, TestResult, crate_directories, items_of, lex,
    product_roots, relative, repository_root, resolve, restored_literals,
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
///
/// The last four came from `P2-X3`, which is the first crate in this repository
/// to write such a sentence as an item-based rule from the start: it keys no
/// inventory on a line prefix, so
/// [`the_inventories_still_keyed_on_a_line_prefix_are_named`] never reports it,
/// which is the seam `docs/contracts/policy-source-scans.md` records. Its
/// entries here are also what put its **whole** item set under
/// [`every_item_in_these_packages_is_pinned`], through
/// [`packages_that_need_an_item_pin`].
const CLOSED_TYPES: [ClosedType; 12] = [
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
    ClosedType {
        package: "record",
        name: "RegistrationConfirmation",
        items: &REGISTRATION_CONFIRMATION_ITEMS,
        contract: "section 25.5's last sentence is 사용자의 실제 수강신청을 자동 수행하지 \
                   않는다 and `P2-M4` made confirming one non-delegable; this is the whole \
                   set of items anywhere in the workspace that reach it, so a route added \
                   from a planner, a plan snapshot or anywhere else fails by name",
    },
    ClosedType {
        package: "dashboard",
        name: "SecondaryPercentage",
        items: &SECONDARY_PERCENTAGE_ITEMS,
        contract: "section 25.4's last line is 상세 breakdown이 항상 붙는다, and \
                   `SecondaryPercentage::over` is the one producer and takes the breakdown \
                   by value; a second producer is an entry here",
    },
    ClosedType {
        package: "dashboard",
        name: "GpaFigure",
        items: &GPA_FIGURE_ITEMS,
        contract: "section 25.4's first line asks each of three averages for its own proof \
                   and section 10 forbids folding a grade average and a knowledge map into \
                   one score; no item returns a number over more than one figure",
    },
    ClosedType {
        package: "dashboard",
        name: "PlanSnapshot",
        items: &PLAN_SNAPSHOT_ITEMS,
        contract: "section 25.5 fixes a saved plan and licenses 무엇이 stale해졌는지만 \
                   표시한다; `restate` takes `&self` and returns a marking, and no item \
                   takes a snapshot by mutable reference",
    },
    ClosedType {
        package: "competency",
        name: "StageEvidence",
        items: &STAGE_EVIDENCE_ITEMS,
        contract: "two doors and there is no third: a filled rubric cell is founded on \
                   `P2-N2`'s admitted evidence or on `P2-R5`'s personal claim, both values \
                   another crate produced under its own checks, and there is no arm at all \
                   for a `ProjectObservationClaim`, so a repository fact alone promotes \
                   nothing",
    },
    ClosedType {
        package: "competency",
        name: "PromotingEvidence",
        items: &PROMOTING_EVIDENCE_ITEMS,
        contract: "one producer, `PromotingEvidence::of`, and it refuses \
                   `EvidenceCeiling::NoPromotion`, which is how section 13.2's \
                   `dependency/install/import만 존재` row cannot become stage evidence",
    },
];

/// The floor under the workspace walk.
///
/// A walk that returned nothing would satisfy every "no file holds" assertion
/// in this file. `T217` measured 568 product files at `29f66d5`.
///
/// 568 is the **compiled** count, not a count of files on disk: a walk of
/// non-test `.rs` files under `crates/` finds 579. The eleven are five
/// `crates/*/examples/emit_*.rs` and the six files of
/// `crates/test-support/src`, and what governs the difference is
/// `compilation_unit_scans.rs`'s `every_product_file_is_compiled_by_its_own_crate`,
/// which pins them. Nothing here reads a file no target compiles.
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

    // The three forms, each present as an item with its own key. `key` rather
    // than `sealed_key` on purpose: this reads what the reader made of a
    // sample it is handed, and the two pins are what compare a fingerprint
    // against the tree. It is the only remaining `key` call site.
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

/// Nothing a scanned package compiles is outside the pinned item set, and no
/// leaf of one holds text nobody wrote down.
///
/// The backstop. It is keyed on nothing at all: not a visibility, not a
/// keyword, not a type name. An item added anywhere in one of these packages
/// is an extra entry whatever it is, which is the statement `P2-A4`'s second
/// audit found three counterexamples to in one afternoon and `P2-A5`'s fourth
/// found two more, in `repository-competency` and `build-learn`, through the
/// `pub const NAME: fn(&T) -> u32` form.
///
/// **Which packages.** Derived, not written down, from **two** sources, and
/// [`packages_that_need_an_item_pin`] is their union:
///
/// * every package that holds a file
///   [`the_inventories_still_keyed_on_a_line_prefix_are_named`] reports, which
///   is the set whose contract rests on a reader that cannot see an item form;
/// * every package that owns a type in [`CLOSED_TYPES`], which is where a
///   package says that its own surface is the subject of a contract sentence.
///
/// The second source closes the seam `docs/contracts/policy-source-scans.md`
/// records: *a crate that writes such a sentence as an item-based rule from the
/// start never appears on the line-prefix list and never gets a pin.* `P2-X3`
/// is the first such crate — `academic-dashboard` keys no inventory on a line
/// prefix and its contract says what may and may not come out of three of its
/// types — and adding it to `CLOSED_TYPES` is now what puts its **whole** item
/// set under this rule as well. For the six types that were here before, the
/// union changes nothing: all four of their packages were already on the first
/// list.
///
/// A package that grows either has to grow a pin file with it, and the failure
/// names the package.
///
/// **Where the pins live.** `crates/contracts/tests/pinned-items/<package>.items`,
/// one [`Item::sealed_key`] to a line, sorted. Six thousand keys are a table
/// rather than a source file: a product edit shows up in `git diff` as the
/// lines it added, which is the review this pin exists to force.
///
/// **Why a sealed key and not a key.** A key is a declaration and says nothing
/// about a body. `P2-A4`'s third audit wrote
/// `#[rustfmt::skip] #[allow(non_local_definitions)] impl From<&Redaction> for
/// Vec<String>` into the body of an existing method of
/// `crates/student-voice/src/derivative.rs`; that handed three students'
/// removed utterances to any crate in the workspace with no `RawAccessGrant`
/// and no `RawAccessLog` row, and this pin did not move, because the item
/// **count was 376 before and after** and every key was byte-identical. A leaf
/// now carries a fingerprint of its own text, so the body is in the pin. The
/// price is that a body edit moves a line here — 5697 of the 6792 keys carry a
/// fingerprint and the other 1095 are containers, whose contents are enumerated
/// as items of their own — and that price is the point: a diff that touches a
/// body in one of these packages is a diff a reviewer should read. `P2-RF29`
/// measured 5134 of 6131 over the 23 packages it derived; `P2-X3`'s two are the
/// difference.
#[test]
fn every_item_in_these_packages_is_pinned() -> TestResult {
    let repository = repository_root()?;
    let packages = packages_that_need_an_item_pin(&repository)?;
    assert!(
        packages.len() >= PINNED_PACKAGE_FLOOR,
        "the derivation found only {} packages to pin",
        packages.len()
    );
    let empty: [&str; 0] = [];
    let mut total = 0_usize;
    for package in &packages {
        let items = product_items(package)?;
        let mut keys: Vec<String> = items.iter().map(Item::sealed_key).collect();
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

/// The pin directory holds one file per derived package and no others, and the
/// numbers this file used to state in prose are read out of the tree.
///
/// **The half that closes the derivation seam.** A package either derivation
/// reports with no pin file fails naming the package; a pin file for a package
/// neither derivation reports fails naming the file. Without the second
/// direction a pin could be added for a package nobody scans and counted as
/// coverage, and without the first a package could join a derivation and be
/// pinned by nothing.
///
/// **The counts.** `P2-A5`'s fifth audit found the module documentation saying
/// "the other 45 crates" and "all 68" where the tree held 47 and 70, and
/// recorded that it survived because **nothing asserted either number**. Both
/// have moved twice since — `P2-RF29` and `P2-X3` — while the prose stayed put.
/// They are asserted here, each stated once, each compared with a walk: the
/// workspace's package count, the pinned count, and the unpinned remainder as
/// the difference of the two rather than as a third number somebody keeps in
/// step by hand.
#[test]
fn the_pin_names_what_it_covers_and_what_it_does_not() -> TestResult {
    let repository = repository_root()?;
    let derived = packages_that_need_an_item_pin(&repository)?;
    let mut pinned: Vec<String> = Vec::new();
    for entry in fs::read_dir(repository.join("crates/contracts/tests/pinned-items"))? {
        let path = entry?.path();
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("items"),
            "{} is not a pin file",
            relative(&repository, &path)
        );
        pinned.push(
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_owned(),
        );
    }
    pinned.sort();
    let held: BTreeSet<&str> = pinned.iter().map(String::as_str).collect();
    let wanted: BTreeSet<&str> = derived.iter().map(String::as_str).collect();
    let empty: [&str; 0] = [];
    let missing: Vec<&str> = wanted.difference(&held).copied().collect();
    let orphaned: Vec<&str> = held.difference(&wanted).copied().collect();
    assert_eq!(
        missing, empty,
        "a package the workspace compiles has no item pin"
    );
    assert_eq!(
        orphaned, empty,
        "a pin file names a package the workspace does not compile"
    );

    // The two derivations are the control on the walk rather than the
    // selection. Every package either of them reports must be in the pinned
    // set, so a walk that returned a truncated list fails here naming what it
    // lost instead of making every comparison above pass over a smaller world.
    let mut derived_by_rule: BTreeSet<String> = packages_keyed_on_a_line_prefix(&repository)?
        .into_iter()
        .collect();
    derived_by_rule.extend(CLOSED_TYPES.iter().map(|closed| closed.package.to_owned()));
    assert!(
        derived_by_rule.len() >= 43,
        "the two derivations report only {} packages",
        derived_by_rule.len()
    );
    let unreported: Vec<&str> = derived_by_rule
        .iter()
        .map(String::as_str)
        .filter(|package| !wanted.contains(*package))
        .collect();
    assert_eq!(
        unreported, empty,
        "a package a derivation reports is outside the walk that selects the pin"
    );

    let mut packages: Vec<String> = Vec::new();
    for directory in crate_directories(&repository)? {
        let package = directory
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{} has no name", directory.display()))?;
        packages.push(package.to_owned());
    }
    packages.sort();
    let mut items = 0_usize;
    for package in &derived {
        items = items.saturating_add(pinned_items(&repository, package)?.len());
    }
    assert_eq!(
        packages.len(),
        WORKSPACE_PACKAGES,
        "the workspace's package count changed"
    );
    assert_eq!(
        derived.len(),
        PINNED_PACKAGES,
        "the set of packages the derivations report changed"
    );
    assert_eq!(items, PINNED_ITEMS, "the pinned item count changed");
    // The remainder is empty, and it is enumerated rather than counted: a
    // package outside the pin fails here by name. `P2-RF30` left 28 packages
    // in this remainder and named them; one of the 28 was the analyzer
    // process, and `P2-A5`'s sixth audit reached the network from inside it
    // with the whole workspace green.
    let unpinned: Vec<&str> = packages
        .iter()
        .map(String::as_str)
        .filter(|package| !wanted.contains(*package))
        .collect();
    assert_eq!(
        unpinned, empty,
        "a package the workspace compiles is outside the pin"
    );
    assert_eq!(
        derived.len(),
        WORKSPACE_PACKAGES,
        "the pinned set is no longer every package the workspace compiles"
    );
    Ok(())
}

/// The packages the workspace compiles, counted at `5f5c39c`.
const WORKSPACE_PACKAGES: usize = 71;

/// The packages the pin covers. Since `P2-RF31` this is all of them.
const PINNED_PACKAGES: usize = 71;

/// The keys the pin directory holds, counted at `5f5c39c` plus this branch.
///
/// 11 748 over 43 packages before `P2-RF31`. The 28 packages that joined and
/// the literal values that joined the fingerprint are the difference.
const PINNED_ITEMS: usize = 18_117;

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

/// The floor under the walk, so a truncated one cannot satisfy the pin.
///
/// Equal to [`WORKSPACE_PACKAGES`] now, and that equality is the repair.
/// While the pinned set was a derivation this floor tracked the derivation's
/// own answer, so `P2-A5`'s sixth audit could record that "the floor still
/// cannot see that the answer should be 71". The selection is the walk, the
/// answer is every package, and a walk that returns fewer fails here.
const PINNED_PACKAGE_FLOOR: usize = 71;

/// The floor under the pinned item count. `P2-RF29` measured 6131 over 23
/// packages, `P2-X3` 6792 over 25, `P2-RF30` 11 748 over 43.
const PINNED_ITEM_FLOOR: usize = 17_000;

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
///
/// **What "keyed on a line prefix" is, and what it was.** `T227` looked for
/// three assembled literals: `starts_with("pub fn ")`, `starts_with("pub const
/// fn ")` and `starts_with("impl ")`. `P2-A5`'s fifth audit measured that
/// keying the detector on three spellings is the same failure the item reader
/// exists to replace, one level up, and named the four packages it misses:
/// `consent`, `curriculum` and `untrusted-content` write `starts_with("impl")`
/// with no trailing space, and `what-if` writes `strip_prefix("pub fn ")`.
///
/// The question is asked without a list of literals now. A collector is keyed
/// on a line prefix when it hands a **string literal whose first word is an
/// item keyword, an item modifier or a visibility** to one of the three
/// operations that take a prefix. The keyword list is [`ITEM_KEYWORDS`] and
/// [`ITEM_MODIFIERS`] -- the reader's own closed enumeration, argued where it
/// is declared -- so this detector and the reader move together instead of
/// drifting apart.
fn files_keyed_on_a_line_prefix(repository: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    // Assembled, so this file's own code is not a match for the scan it runs.
    let operations = [
        format!("starts{}with(", '_'),
        format!("strip{}prefix(", '_'),
        "find(".to_owned(),
    ];
    let heads: BTreeSet<&str> = ITEM_KEYWORDS
        .iter()
        .chain(ITEM_MODIFIERS.iter())
        .copied()
        .chain(["pub"])
        .collect();
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
                if operations
                    .iter()
                    .any(|operation| keys_on_an_item_head(&code, operation, &heads))
                {
                    found.push(relative(repository, &path));
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Every package whose whole item set is pinned: every package the workspace
/// compiles.
///
/// **There is no derivation in the selection role any more, and that is the
/// point.** Until `P2-RF31` this was the union of the packages that key an
/// inventory on a line prefix and the packages that own a closed type, and
/// `P2-RF30` enumerated the 28 packages on neither arm and left them open.
/// `P2-A5`'s sixth audit then put one live `std::net` name resolution into one
/// of the 28 — `crates/repository-analyzer`, the analyzer process itself —
/// and measured the whole workspace green on both hosts. Every round of this
/// Run had found the next thing the selection rule did not spell, and a
/// selection rule that reports a crate only when the crate already carries an
/// inventory or a closed type cannot report a crate that carries neither.
///
/// So the selection is now the walk, and what the derivations do instead is
/// **control** it: [`the_pin_names_what_it_covers_and_what_it_does_not`]
/// requires every package either of them reports to be in this set, so a walk
/// that returned a truncated or empty list fails naming the packages it lost
/// rather than passing over a smaller world.
fn packages_that_need_an_item_pin(repository: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut found: Vec<String> = Vec::new();
    for directory in crate_directories(repository)? {
        found.push(
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("{} has no name", directory.display()))?
                .to_owned(),
        );
    }
    found.sort();
    Ok(found)
}

/// Whether `code` hands `operation` a literal whose first word is an item head.
///
/// The literal is read by scanning rather than by pattern, so an escape inside
/// it cannot end it early. A literal that does not open immediately after the
/// parenthesis is not a prefix key -- `starts_with(marker)` passes a variable
/// and says nothing about a spelling.
fn keys_on_an_item_head(code: &str, operation: &str, heads: &BTreeSet<&str>) -> bool {
    let bytes: Vec<char> = code.chars().collect();
    let mut cursor = 0;
    while let Some(at) = code[cursor..].find(operation).map(|at| at + cursor) {
        cursor = at + operation.len();
        let opens = code[..cursor].chars().count();
        if bytes.get(opens) != Some(&'"') {
            continue;
        }
        let mut word = String::new();
        let mut index = opens + 1;
        while let Some(character) = bytes.get(index) {
            if *character == '"' || character.is_whitespace() {
                break;
            }
            if *character == '\\' {
                index += 2;
                continue;
            }
            word.push(*character);
            index += 1;
        }
        if heads.contains(word.as_str()) {
            return true;
        }
    }
    false
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
/// says it is in many more. Every one of them carries a whole-set item pin as
/// well -- this is the first arm of [`packages_that_need_an_item_pin`] -- so
/// the list is not the remainder, it is half the derivation. What it still
/// records is that these files read a line: the two readers are complementary,
/// because a line-anchored `impl_headers` sees inside a function body and an
/// item key does not.
///
/// **The detector is no longer a list of literals.** `T227` searched for
/// `starts_with("pub fn ")`, `starts_with("pub const fn ")` and
/// `starts_with("impl ")`, and `P2-A5`'s fifth audit measured that keying it on
/// three spellings is the same failure the item reader exists to replace, one
/// level up: `consent`, `curriculum` and `untrusted-content` write
/// `starts_with("impl")` with no trailing space and `what-if` writes
/// `strip_prefix`, so four packages held the shape and were outside the arm.
/// The question is asked about the **literal's first word** now, against
/// [`ITEM_KEYWORDS`] and [`ITEM_MODIFIERS`] -- the reader's own closed
/// enumeration, argued where it is declared -- so the detector and the reader
/// move together instead of drifting apart, and eight controls below fix what
/// it must see and what it must refuse.
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
    // Every package either derivation reports has a pin file, and every pin
    // file names a package one of them reports. The second direction is what
    // stops a pin being added for a package nobody scans and counted as
    // coverage.
    let packages = packages_that_need_an_item_pin(&repository)?;
    for package in &packages {
        let path = pin_path(&repository, package);
        assert!(
            path.is_file(),
            "{} needs an item pin and has none at {}",
            package,
            relative(&repository, &path)
        );
    }
    // The detector answers for the four spellings `P2-A5` measured it missing,
    // and refuses a literal that is not an item head, so "accept more" is not
    // what this became. `find` is included because `crates/competency` keys
    // `public_signatures` on one, which is how the crate holding claim 5's
    // sentence stayed off this list.
    let heads: BTreeSet<&str> = ITEM_KEYWORDS
        .iter()
        .chain(ITEM_MODIFIERS.iter())
        .copied()
        .chain(["pub"])
        .collect();
    let starts = format!("starts{}with(", '_');
    let strips = format!("strip{}prefix(", '_');
    let finds = "find(".to_owned();
    for (code, operation) in [
        (
            format!("line.starts{}with({}impl{})", '_', '"', '"'),
            &starts,
        ),
        (
            format!("trimmed.strip{}prefix({}pub fn {})", '_', '"', '"'),
            &strips,
        ),
        (format!("code.find({}pub fn {})", '"', '"'), &finds),
        (
            format!("line.starts{}with({}pub const fn {})", '_', '"', '"'),
            &starts,
        ),
    ] {
        assert!(
            keys_on_an_item_head(&code, operation, &heads),
            "the detector does not see {code}"
        );
    }
    for (code, operation) in [
        // A path prefix, a message and a variable are none of them item heads.
        (
            format!("rest.strip{}prefix({}crates/{})", '_', '"', '"'),
            &strips,
        ),
        (
            format!("line.starts{}with({}let roots = [{})", '_', '"', '"'),
            &starts,
        ),
        (format!("code.find(marker.as{}str())", '_'), &finds),
        (
            format!("name.starts{}with({}academic{}{})", '_', '"', '_', '"'),
            &starts,
        ),
    ] {
        assert!(
            !keys_on_an_item_head(&code, operation, &heads),
            "the detector reads {code} as a line-prefix collector"
        );
    }
    Ok(())
}

/// One reader, copied into every crate that scans its own reaches, and the
/// copies are held to being one text.
///
/// `P2-R2` repaired a reach guard that a token list walked past; `P2-A5`
/// measured the repaired one walked past by
/// `<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)`, which resolves
/// a name. [`REACH_READERS`] is the whole set of files holding a copy, read
/// out of the tree and compared here in both directions, so the count lives in
/// that array rather than in this sentence: an audit that counted the carriers
/// with `crates/*/tests/*.rs` missed the two that keep theirs one directory
/// further down in `tests/support/mod.rs`.
///
/// **Why they are not one function.** They could be: a dev-dependency crate
/// holding the helpers would give one copy. It would also add an edge to the
/// dependency closure of every carrier crate, and that closure is the subject of
/// `tools/phase1-scaffold-policy.test.mjs`'s dependency map, its acyclic graph
/// and each crate's own `USE_ITEMS` inventory -- scans whose whole point is
/// that the closure does not move. Paying in the thing being protected to
/// deduplicate the thing protecting it is the wrong trade, and the copy is
/// deliberate for a second reason the crates state: `P2-G4` found that a lexer
/// without raw strings desynchronizes, so each crate carries a lexer it can
/// read rather than one it imports.
///
/// **What replaces one copy.** This: the bodies are compared against each
/// other, so every copy is one text, and every carrier crate is required to
/// hold the driving control. One driven copy plus textual identity is the same
/// guarantee as one function, and a copy that arrives with the old body fails
/// here by name instead of waiting for an audit.
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
// The eight binaries the workspace ships
// ---------------------------------------------------------------------------

/// The crates that ship a `src/main.rs`.
///
/// Read out of the tree by [`shipped_binaries`] and compared here in both
/// directions, so a ninth binary crate cannot arrive outside the two rules
/// below. `P2-A5`'s sixth audit put one live `std::net` name resolution above
/// the sandbox entry in the shipped `academic-repository-analyzer` and
/// measured the whole workspace green on both hosts. The reason nothing saw
/// it: all eight of these were outside every reach reader and outside the item
/// pin, and the derivation that chose what to watch selected on what a crate
/// already had, so it could not report a crate that had neither.
const SHIPPED_BINARIES: [&str; 8] = [
    "capture-client",
    "cli",
    "connector",
    "daemon",
    "egress",
    "export-job",
    "indexer",
    "repository-analyzer",
];

/// The four binaries that enter the process sandbox, and the class each binds.
///
/// The two process classes not here are `CONNECTOR` and `INDEXER`, and they
/// are absent for a reason this repository already measured rather than for
/// one nobody wrote down:
/// `a_declared_capability_is_not_defined_by_one_the_boundary_would_refuse` in
/// `crates/process-sandbox/tests/enforcement.rs` pins them as the whole set of
/// classes whose declaration argues with their own boundary. A third arriving
/// there fails there.
const CLASS_BINARIES: [(&str, &str); 4] = [
    ("capture-client", "CaptureClient"),
    ("egress", "EgressProxy"),
    ("export-job", "ExportJob"),
    ("repository-analyzer", "RepositoryAnalyzer"),
];

/// Every product file of a shipped binary that declares a top-level `fn main`.
///
/// Both directions, and the four class binaries are deliberately **not** here.
/// Their whole `main` is one expansion of
/// `academic_process_sandbox::class_main!`, so each crate declares no `fn main`
/// and there is no statement position above the sandbox entry to write into.
/// One appearing here again is the shape the sixth audit measured, and it
/// fails naming the file.
const FILES_DECLARING_FN_MAIN: [&str; 4] = [
    "crates/cli/src/main.rs",
    "crates/connector/src/main.rs",
    "crates/daemon/src/main.rs",
    "crates/indexer/src/main.rs",
];

/// Every crate directory holding a `src/main.rs`.
fn shipped_binaries(repository: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut found = Vec::new();
    for directory in crate_directories(repository)? {
        if !directory.join("src").join("main.rs").is_file() {
            continue;
        }
        found.push(
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("{} has no name", directory.display()))?
                .to_owned(),
        );
    }
    found.sort();
    Ok(found)
}

/// One shipped binary's whole product closure, as its files' text.
fn binary_sources(
    repository: &Path,
    package: &str,
) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let directory = repository.join("crates").join(package);
    let mut found = BTreeMap::new();
    for root in product_roots(&directory)? {
        for file in resolve(&root, repository)?.files {
            found.insert(relative(repository, &file), fs::read_to_string(&file)?);
        }
    }
    Ok(found)
}

/// The set of crates shipping an executable is the set these rules watch.
#[test]
fn the_shipped_binaries_are_the_ones_this_file_watches() -> TestResult {
    let repository = repository_root()?;
    assert_eq!(
        shipped_binaries(&repository)?,
        SHIPPED_BINARIES
            .iter()
            .map(|package| (*package).to_owned())
            .collect::<Vec<String>>(),
        "the set of crates shipping a src/main.rs changed"
    );
    Ok(())
}

/// A process-class binary runs nothing of its own before the sandbox is in.
///
/// **This is the rule the sixth audit's F1 is about, and it is not a scan over
/// names.** The injection it measured was one statement above
/// `academic_process_sandbox::enter`. A reach reader would have caught that
/// particular one, and would not have caught a second reach through a crate
/// root the binary already names — the key is a root and one segment, so
/// `academic_rpc::listen` beside `academic_rpc::connect` is not a new key.
/// What closes the window is that there is no window: the four class binaries
/// declare no `fn main` at all, `class_main!` is the whole of it, and its
/// argument is an `ident` fragment so `class_main!({ reach(); PROCESS_CLASS })`
/// does not compile.
///
/// Two directions, so neither half can be dodged. A `fn main` written into a
/// class binary is a file this rule does not list; an item of any other kind
/// written into one of their `main.rs` files is an extra key in the whole-set
/// comparison below.
#[test]
fn no_process_class_binary_authors_a_statement_before_it_enters() -> TestResult {
    let repository = repository_root()?;

    let mut declaring: Vec<String> = Vec::new();
    for package in shipped_binaries(&repository)? {
        for (name, source) in binary_sources(&repository, &package)? {
            if items_of(&name, &source)?
                .iter()
                .any(|item| item.owner.is_empty() && item.kind == "fn" && item.name == "main")
            {
                declaring.push(name);
            }
        }
    }
    declaring.sort();
    assert_eq!(
        declaring,
        FILES_DECLARING_FN_MAIN
            .iter()
            .map(|file| (*file).to_owned())
            .collect::<Vec<String>>(),
        "a shipped binary declares a `fn main` this rule does not cover"
    );

    for (package, class) in CLASS_BINARIES {
        let name = format!("crates/{package}/src/main.rs");
        let source = fs::read_to_string(repository.join(&name))?;
        let keys: Vec<String> = items_of(&name, &source)?.iter().map(Item::key).collect();
        assert_eq!(
            keys,
            vec![
                format!("{name} [priv] use academic_policy::ProcessClass"),
                format!("{name} [priv] const PROCESS_CLASS: ProcessClass"),
                format!("{name} [priv] academic_process_sandbox::class_main!(PROCESS_CLASS)"),
            ],
            "{name} is not the three items a class binary is allowed to hold"
        );
        assert!(
            source.contains(&format!("ProcessClass::{class};")),
            "{name} no longer binds {class}"
        );
    }
    Ok(())
}

/// Every path a shipped binary reaches through a crate root, with a reason.
///
/// The third comparison `crates/repository/tests/repository_scans.rs` gained
/// in `P2-RF30`, widened to the eight crates that ship an executable — the
/// eight the sixth audit found on no reach reader at all. A capability written
/// as an absolute path inside a shipped process is an extra key here whatever
/// it is named, which is the property a forbidden-token list cannot have.
///
/// **What this cannot close, measured rather than assumed.** The key is the
/// crate root and the segment after it, so a third segment is not a new key:
/// `std::fs::write`, already listed for `cli`, and `std::fs::remove_dir_all`
/// are one `std::fs`. And an import is stripped before the pass, so a reach
/// spelled through an imported name — `use std::net::ToSocketAddrs;` and
/// then a bare method call — carries no `::` at all and yields nothing
/// here. What sees both is the item pin: since `P2-RF31` every package the
/// workspace compiles has one, a `use` item is an item, and a body edit moves
/// a fingerprint. This rule is the layer that names *what* was reached rather
/// than saying that something changed, and the four class binaries rest on
/// neither —
/// [`no_process_class_binary_authors_a_statement_before_it_enters`] is what
/// says they run nothing of their own before the sandbox is installed.
#[test]
fn every_path_a_shipped_binary_reaches_has_a_reason() -> TestResult {
    let repository = repository_root()?;
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut files = 0_usize;
    for package in shipped_binaries(&repository)? {
        for (_, source) in binary_sources(&repository, &package)? {
            files += 1;
            let code: String = lex(&source).code.into_iter().collect();
            for path in absolute_paths(&without_use_items(&code)) {
                reached.insert(format!("{package} {path}"));
            }
        }
    }
    assert!(files >= 30, "the walk read only {files} product files");
    assert_eq!(
        reached,
        BINARY_REACHES
            .iter()
            .map(|(package, path, _)| format!("{package} {path}"))
            .collect::<BTreeSet<String>>(),
        "a shipped binary reaches a path outside its inventory; every entry needs a reason"
    );

    // This copy of the reader is held to being the same text as the other
    // eighteen by `the_reach_readers_are_one_reader`, and it is driven here
    // through the form that walked past the repaired one, so it does not rest
    // on the other carriers' evidence.
    assert!(
        absolute_paths("let _ = <str as ::std::net::ToSocketAddrs>::to_socket_addrs(h);")
            .contains("std::net")
    );
    assert!(absolute_paths("let _: &dyn ::core::fmt::Debug = &v;").contains("core::fmt"));
    assert!(!absolute_paths("std::alloc::Layout::new::<u8>()").contains("alloc::Layout"));
    Ok(())
}

/// `code` with every `use` item removed, so an import is not read as a reach.
fn without_use_items(code: &str) -> String {
    let mut kept = String::with_capacity(code.len());
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim_start();
        let opens = trimmed.starts_with("use ")
            || (trimmed.starts_with("pub") && trimmed.contains(" use "));
        if inside || opens {
            inside = !line.trim_end().ends_with(';');
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    kept
}

/// `code` with the whitespace that sits inside a path or a macro call removed.
///
/// Rust allows whitespace inside a path and between a macro's `!` and its
/// delimiter, and both were measured slipping past the two extractors below:
/// `std :: path :: Path::new(p).metadata()` opens the filesystem and
/// `include_str! ("x")` reads a file, and each compiled and passed.
///
/// It closes exactly those two gaps and nothing wider. Deleting **all**
/// whitespace was tried first and is wrong in the one direction that matters:
/// it joins unrelated tokens, and `… Formatter and core::str …` becomes
/// `…Formatterandcore::str…`, where `core` is no longer a whole identifier and
/// the key **disappears**. A transform that can hide a key is worse than the
/// hole it closes. `the_helpers_are_not_vacuous` carries that case.
fn tighten(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find(char::is_whitespace) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let stop = tail
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(tail.len());
        let after = &tail[stop..];
        // The run is inside a path or a macro call exactly when a `::` or a `!`
        // ends what came before it, or a `::` or a `!` starts what follows.
        // `foo ! (x)` and `foo! (x)` are both macro calls, so both sides of the
        // `!` are tightened; `a != b` and `if !flag` survive it, because the
        // extractor still requires a delimiter immediately after the `!`.
        let joins = out.ends_with("::")
            || out.ends_with('!')
            || after.starts_with("::")
            || after.starts_with('!');
        if !joins {
            out.push(' ');
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Every two-segment path `code` spells through a crate root.
///
/// The first segment has to be a crate root this package can name, so a field
/// access such as `self.path` is not a path and `Self::Variant` is not one
/// either. What it catches is the absolute form — `std::env::var`,
/// `std::path::Path` — which is the shape that needs no `use` item.
fn absolute_paths(code: &str) -> BTreeSet<String> {
    let roots = ["std", "core", "alloc", "thiserror"];
    let code = &tighten(code);
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    let mut taken = 0_usize;
    for (at, _) in code.match_indices("::") {
        let mut start = at;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        if start == at {
            continue;
        }
        // A whole identifier: the byte before it cannot continue one, which is
        // what stops `a::b::c` being read as a second root at `b`.
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        // A middle segment of a longer path -- the `b` of `a::b::c` -- is not a
        // crate root, and skipping it is what stops one path yielding two keys.
        // What decides it is whether this segment already sits inside a key
        // this pass took, not the byte three positions back. `tighten` glues
        // `as ::std` shut, so that byte is the `s` of a keyword and the leading
        // `::` of a qualified path read as a middle one: `P2-A5` measured
        // `<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)` resolving
        // a name from a live function while this pass reported nothing. Every
        // segment outside a key already taken is a root, and a root nobody
        // admits fails as an extra key rather than passing.
        if start < taken {
            continue;
        }
        let root = &code[start..at];
        if !roots.contains(&root) && !root.starts_with("academic_") {
            continue;
        }
        let mut end = at + 2;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if end > at + 2 {
            found.insert(code[start..end].to_owned());
            taken = end;
        }
    }
    found
}

/// Every path a shipped binary reaches through a crate root, with a reason.
///
/// The `(package, path, reason)` shape `crates/repository/tests/repository_scans.rs`
/// uses, over the eight crates that ship an executable. `connector` and
/// `indexer` are absent because they reach nothing: their whole `main` reads
/// `PROCESS_CLASS.capabilities()` through an import.
const BINARY_REACHES: [(&str, &str, &str); 19] = [
    (
        "capture-client",
        "academic_process_sandbox::class_main",
        "the whole of `main`; the crate authors no statement of its own",
    ),
    (
        "cli",
        "academic_admission::AdmissionVerifier",
        "the receipt posture the banner and the `admission` subcommand print",
    ),
    (
        "cli",
        "academic_core::operations",
        "the backup format constant and the operation types the subcommands hand to the daemon",
    ),
    (
        "cli",
        "academic_daemon::SESSION_NONCE_CAPABILITY_PREFIX",
        "the session-nonce prefix, restated as this crate's own `const` so the two agree",
    ),
    (
        "cli",
        "academic_rpc::generated",
        "the generated request and response types; ingest is the only mutation and it travels over IPC",
    ),
    (
        "cli",
        "std::env",
        "`var_os` for `LOCALAPPDATA` and `XDG_RUNTIME_DIR`, to find the current user's runtime root",
    ),
    (
        "cli",
        "std::error",
        "`Box<dyn Error>` in the doctor's own helpers",
    ),
    (
        "cli",
        "std::fmt",
        "the hand-written `Display` for `CliFailure`",
    ),
    (
        "cli",
        "std::fs",
        "the doctor's `--deep` fixture writes, under a caller-named profile root",
    ),
    (
        "cli",
        "std::future",
        "the `Future` bound on the async command dispatcher",
    ),
    (
        "cli",
        "std::path",
        "`absolute`, which resolves `..` against the filesystem rather than lexically",
    ),
    (
        "daemon",
        "academic_rpc::PHASE1_POLICY_BANNER",
        "the banner this binary prints before it starts",
    ),
    (
        "daemon",
        "academic_rpc::generated",
        "the wire types the transport carries",
    ),
    (
        "daemon",
        "std::fs",
        "`symlink_metadata` and `create_dir` on the Windows transport's runtime root",
    ),
    (
        "daemon",
        "std::slice",
        "`from_raw_parts` over the Windows security-descriptor bytes, inside the crate's `unsafe`",
    ),
    (
        "daemon",
        "std::time",
        "`SystemTime::now`, the writer's one clock reading",
    ),
    (
        "egress",
        "academic_process_sandbox::class_main",
        "the whole of `main`; the crate authors no statement of its own",
    ),
    (
        "export-job",
        "academic_process_sandbox::class_main",
        "the whole of `main`; the crate authors no statement of its own",
    ),
    (
        "repository-analyzer",
        "academic_process_sandbox::class_main",
        "the whole of `main`; the crate authors no statement of its own",
    ),
];

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
    "crates/capture-gate/src/artifact.rs [priv] impl fmt::Debug for ReleasableArtifact :: fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result |9d401845ea198532",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl CaptureArtifact :: const fn releasable(manifest: CaptureManifest, bytes: Vec<u8>) -> Self |732d2d152c6adf48",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Clone, PartialEq, Eq)] struct ReleasableArtifact |56e39015c24901ba",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum CaptureArtifact |f806e5314858e29b",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureArtifact :: #[must_use] const fn as_releasable(&self) -> Option<&ReleasableArtifact> |5a55f780817bfa34",
    "crates/capture-gate/src/artifact.rs [pub] impl ReleasableArtifact :: #[must_use] const fn manifest(&self) -> &CaptureManifest |5048569a0c02e48a",
    "crates/capture-gate/src/artifact.rs [pub] impl ReleasableArtifact :: #[must_use] fn bytes(&self) -> &[u8] |a3abc5b93da5a4c9",
    "crates/capture-gate/src/lib.rs [pub] use artifact::{ CaptureArtifact, CaptureManifest, ChunkRecord, PERMISSION_VIOLATION_RISK, QuarantinedArtifact, ReleasableArtifact, TimelineGap, ViolationRisk, } |5f1c50d32a5909a1",
];

/// Every item of the workspace that reaches a `StageEvidence`.
///
/// The type `P2-R5`'s claim becomes a filled rubric cell through, and the
/// crate `P2-A5`'s fifth audit opened a third door into. One method added
/// inside the existing `impl StageEvidence` block -- spelling none of the two
/// names `no_product_file_names_a_project_observation_claim` lists, adding no
/// `use` item, no field and no re-export -- filled a personal competency cell
/// at the `USED` stage from a snapshot identifier and a classification token
/// alone, and passed the whole workspace on both hosts.
///
/// It passed because `crates/competency` had no item pin. It keys no inventory
/// on a line prefix, so the first arm of
/// [`packages_that_need_an_item_pin`] never reported it; declaring the two
/// types its module documentation is about is what puts it on the second, which
/// is the arm `P2-X3` built for exactly this — *where a package says that its
/// own surface is the subject of a contract sentence*. The whole item set of
/// `academic-competency` is pinned as a consequence, and the route is **named**
/// here rather than reported as an item nobody wrote down.
const STAGE_EVIDENCE_ITEMS: [&str; 22] = [
    "crates/competency/src/evidence.rs [priv] impl StageEvidence",
    "crates/competency/src/evidence.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, Serialize)] struct StageEvidence |82b280888ee90cb4",
    "crates/competency/src/evidence.rs [pub] impl StageEvidence :: #[must_use] const fn concept(&self) -> &ConceptRef |44b249370155f49a",
    "crates/competency/src/evidence.rs [pub] impl StageEvidence :: #[must_use] const fn id(&self) -> &RecordId |dbe79657cd535cff",
    "crates/competency/src/evidence.rs [pub] impl StageEvidence :: #[must_use] const fn source(&self) -> &EvidenceSource |af80cdba2ed29bd7",
    "crates/competency/src/evidence.rs [pub] impl StageEvidence :: #[must_use] const fn stage(&self) -> EvidenceStage |eb4a0d8a3b324ed0",
    "crates/competency/src/evidence.rs [pub] impl StageEvidence :: #[must_use] fn of_knowledge_state( id: RecordId, stage: EvidenceStage, evidence: &PromotingEvidence, ) -> Self |9db8de3bfc5c39a1",
    "crates/competency/src/evidence.rs [pub] impl StageEvidence :: fn of_personal_claim( id: RecordId, stage: EvidenceStage, claim: &PersonalApplicationClaim, ) -> Result<Self, CompetencyError> |55789112b3f1f1fc",
    "crates/competency/src/lib.rs [pub] use evidence::{EvidenceOrigin, EvidenceSource, PromotingEvidence, StageEvidence} |12576c44d75a36e7",
    "crates/competency/src/sheet.rs [priv] impl CellState",
    "crates/competency/src/sheet.rs [priv] impl RubricSheet",
    "crates/competency/src/sheet.rs [priv] use crate::{ Competency, evidence::StageEvidence, identity::{CompetencyId, CriterionId}, stage::EvidenceStage, } |dbbb41d57fd12764",
    "crates/competency/src/sheet.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, Serialize)] #[serde( tag = \"state\", content = \"records\", rename_all = \"SCREAMING_SNAKE_CASE\" )] enum CellState |7ab2831ae1a419d1",
    "crates/competency/src/sheet.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, Serialize)] struct RubricSheet |d0992a0fc4f4eb40",
    "crates/competency/src/sheet.rs [pub] #[must_use] fn fill(competency: &Competency, records: &[StageEvidence]) -> RubricSheet |c50d6bd5b19637f0",
    "crates/competency/src/sheet.rs [pub] impl CellState :: #[must_use] fn records(&self) -> &[StageEvidence] |9c70e27c2eb5176f",
    "crates/competency/src/sheet.rs [pub] impl RubricSheet :: #[must_use] fn unmatched(&self) -> &[StageEvidence] |834f983e9ea2779f",
    "crates/readiness/src/cell.rs [priv] impl AxisEvidence",
    "crates/readiness/src/cell.rs [priv] use academic_competency::{Competency, CriterionId, EvidenceStage, StageEvidence} |d00c8bd7741f1a9f",
    "crates/readiness/src/cell.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, Serialize)] struct AxisEvidence |c24bba2f34ecebaf",
    "crates/readiness/src/cell.rs [pub] impl AxisEvidence :: #[must_use] const fn record(&self) -> &StageEvidence |6c85860cd8571836",
    "crates/readiness/src/cell.rs [pub] impl AxisEvidence :: fn place( axis: ReadinessAxis, criterion: CriterionId, locator: EvidenceLocatorId, record: &StageEvidence, ) -> Result<Self, ReadinessError> |61c7f825205ca14e",
];

/// Every item of the workspace that reaches a `PromotingEvidence`.
///
/// Door one. `PromotingEvidence::of` is the one producer and it refuses the
/// `NoPromotion` ceiling, which is what makes a dependency declaration a value
/// that cannot become stage evidence at all.
const PROMOTING_EVIDENCE_ITEMS: [&str; 10] = [
    "crates/competency/src/evidence.rs [priv] impl PromotingEvidence",
    "crates/competency/src/evidence.rs [priv] impl StageEvidence",
    "crates/competency/src/evidence.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct PromotingEvidence |f07ce1b5763eb033",
    "crates/competency/src/evidence.rs [pub] impl PromotingEvidence :: #[must_use] const fn admitted(&self) -> &EligibleEvidence |c85dbaca903b6253",
    "crates/competency/src/evidence.rs [pub] impl PromotingEvidence :: #[must_use] const fn concept(&self) -> EntityId |bae9bbef3f4351be",
    "crates/competency/src/evidence.rs [pub] impl PromotingEvidence :: #[must_use] const fn evidence_id(&self) -> EvidenceId |58d7c4992d779fc0",
    "crates/competency/src/evidence.rs [pub] impl PromotingEvidence :: #[must_use] const fn kind(&self) -> EvidenceKind |ca633d0c025f0f95",
    "crates/competency/src/evidence.rs [pub] impl PromotingEvidence :: fn of(inner: EligibleEvidence) -> Result<Self, CompetencyError> |25ce42c42d593803",
    "crates/competency/src/evidence.rs [pub] impl StageEvidence :: #[must_use] fn of_knowledge_state( id: RecordId, stage: EvidenceStage, evidence: &PromotingEvidence, ) -> Self |9db8de3bfc5c39a1",
    "crates/competency/src/lib.rs [pub] use evidence::{EvidenceOrigin, EvidenceSource, PromotingEvidence, StageEvidence} |12576c44d75a36e7",
];

/// Every file holding a copy of the reach reader.
///
/// Sixteen, not the fourteen `P2-A5` counted: `crates/*/tests/*.rs` does not
/// reach `tests/support/mod.rs`, and two crates keep theirs there.
const REACH_READERS: [&str; 19] = [
    "crates/blind-spot/tests/blind_spot_scans.rs",
    "crates/build-learn/tests/build_learn_scans.rs",
    "crates/competency/tests/competency_scans.rs",
    "crates/contracts/tests/item_inventory_scans.rs",
    "crates/critical-path/tests/critical_path_scans.rs",
    "crates/cs-map/tests/cs_map_scans.rs",
    "crates/dashboard/tests/dashboard_scans.rs",
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
    "crates/repository/tests/repository_scans.rs",
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
const INVENTORIES_KEYED_ON_A_LINE_PREFIX: [&str; 50] = [
    "crates/audit/tests/audit_scans.rs",
    "crates/blind-spot/tests/blind_spot_scans.rs",
    "crates/build-learn/tests/build_learn_scans.rs",
    "crates/capture-gate/tests/capture_scans.rs",
    "crates/capture/tests/capture_scans.rs",
    "crates/competency/tests/competency_scans.rs",
    "crates/consent/tests/consent_scans.rs",
    "crates/contracts/tests/item_inventory_scans.rs",
    "crates/critical-path/tests/critical_path_scans.rs",
    "crates/cs-map/tests/cs_map_scans.rs",
    "crates/curriculum/tests/curriculum_scans.rs",
    "crates/dashboard/tests/dashboard_scans.rs",
    "crates/deletion/tests/deletion_scans.rs",
    "crates/egress-boundary/tests/byte_path_pin.rs",
    "crates/evidence-center/tests/evidence_center_scans.rs",
    "crates/export/tests/export_scans.rs",
    "crates/freshness/tests/freshness_scans.rs",
    "crates/gap/tests/gap_scans.rs",
    "crates/home/tests/home.rs",
    "crates/ingestion/tests/ingestion_scans.rs",
    "crates/integrations/tests/integration_scans.rs",
    "crates/integrations/tests/integrations.rs",
    "crates/integrations/tests/support/mod.rs",
    "crates/keystore-platform/tests/facade.rs",
    "crates/knowledge-state/tests/knowledge_state_scans.rs",
    "crates/lecture-document/tests/lecture_document_scans.rs",
    "crates/model-run/tests/model_run_scans.rs",
    "crates/next-lecture/tests/next_lecture_scans.rs",
    "crates/next-lecture/tests/support/mod.rs",
    "crates/non-delegable/tests/non_delegable_scans.rs",
    "crates/offering/tests/offering_scans.rs",
    "crates/process-sandbox/tests/scans.rs",
    "crates/proposal/tests/proposal_scans.rs",
    "crates/readiness/tests/readiness_matrix.rs",
    "crates/readiness/tests/readiness_scans.rs",
    "crates/record/tests/record_scans.rs",
    "crates/repository-analysis/tests/analysis_scans.rs",
    "crates/repository-classification/tests/classification_scans.rs",
    "crates/repository-competency/tests/competency_scans.rs",
    "crates/repository-correlation/tests/correlation_scans.rs",
    "crates/repository/tests/repository_scans.rs",
    "crates/requirement/tests/requirement_scans.rs",
    "crates/review/tests/review_scans.rs",
    "crates/role-profile/tests/role_scans.rs",
    "crates/student-voice/tests/student_voice_scans.rs",
    "crates/transcription/tests/transcription_scans.rs",
    "crates/untrusted-content/tests/trust_scans.rs",
    "crates/what-if/tests/support/mod.rs",
    "crates/what-if/tests/what_if.rs",
    "crates/what-if/tests/what_if_scans.rs",
];

/// Every item of the workspace that reaches `RegistrationConfirmation`.
///
/// The type `P2-M4` made non-delegable and section 25.5's last sentence is
/// about. `P2-X3` links `academic-record`, so it *can* name this type, and it
/// appears here not at all — which is what makes
/// `planner_has_no_registration_endpoint` a measurement rather than a statement
/// about a type that crate could not have named anyway.
const REGISTRATION_CONFIRMATION_ITEMS: [&str; 11] = [
    "crates/record/src/attempt.rs [priv] impl CourseAttempt",
    "crates/record/src/attempt.rs [priv] impl RegistrationConfirmation",
    "crates/record/src/attempt.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct RegistrationConfirmation |a9305b32a56a1e84",
    "crates/record/src/attempt.rs [pub] impl CourseAttempt :: fn from_confirmed_registration( id: AttemptId, confirmation: &RegistrationConfirmation, grading_scheme_id: impl Into<String>, ) -> Result<Self, RecordError> |f8e5810e841e4f18",
    "crates/record/src/attempt.rs [pub] impl RegistrationConfirmation :: #[must_use] const fn credits_attempted(&self) -> Decimal |977d2e0de233c5d8",
    "crates/record/src/attempt.rs [pub] impl RegistrationConfirmation :: #[must_use] const fn term(&self) -> TermKey |c33462f2f1f0b6a4",
    "crates/record/src/attempt.rs [pub] impl RegistrationConfirmation :: #[must_use] fn course_code(&self) -> &str |36272f9287cb4db3",
    "crates/record/src/attempt.rs [pub] impl RegistrationConfirmation :: #[must_use] fn evidence_ids(&self) -> &[EvidenceId] |ab983fd29f7b2410",
    "crates/record/src/attempt.rs [pub] impl RegistrationConfirmation :: fn new( course_code: impl Into<String>, term: TermKey, credits_attempted: Decimal, evidence_ids: Vec<EvidenceId>, ) -> Result<Self, RecordError> |8610f6becc4ad5e1",
    "crates/record/src/corpus.rs [priv] use crate::{ CanonicalIdentifier, RecordError, attempt::{AttemptHistory, CourseAttempt, RegistrationConfirmation, SettledStatus}, classify::{ClassificationRule, ClassificationRuleSet, ProgramId, RequirementCategory}, grade::{GradeSymbol, GradingScheme}, policy::{ AttemptOrigin, ExternalGradePolicyRow, PolicyBook, RecognitionDecision, RepeatPolicyRow, RepeatRecognition, RuleBook, }, term::TermKey, } |e613f509f28ed902",
    "crates/record/src/corpus.rs [pub] fn baseline_history() -> Result<AttemptHistory, RecordError> |9fac65d9555dec71",
];

/// Every item of the workspace that reaches `SecondaryPercentage`.
const SECONDARY_PERCENTAGE_ITEMS: [&str; 11] = [
    "crates/dashboard/src/lib.rs [pub] use percentage::{BreakdownPart, RequirementBreakdown, SecondaryPercentage} |af8d76a07c0b036c",
    "crates/dashboard/src/percentage.rs [priv] impl SecondaryPercentage",
    "crates/dashboard/src/percentage.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct SecondaryPercentage |372005a4eb9506a2",
    "crates/dashboard/src/percentage.rs [pub] impl SecondaryPercentage :: #[must_use] const fn breakdown(&self) -> &RequirementBreakdown |d2758e5192d265b1",
    "crates/dashboard/src/percentage.rs [pub] impl SecondaryPercentage :: #[must_use] const fn permille(&self) -> u32 |734625577c057347",
    "crates/dashboard/src/percentage.rs [pub] impl SecondaryPercentage :: fn over(breakdown: RequirementBreakdown) -> Result<Self, DashboardError> |63c66580f8274120",
    "crates/dashboard/src/screen.rs [priv] impl AcademicDashboard",
    "crates/dashboard/src/screen.rs [priv] use crate::{ AttemptTimeline, AuditStateReading, DashboardError, GpaFigure, GpaScope, OpenGate, SecondaryPercentage, } |279f0ee1255a771b",
    "crates/dashboard/src/screen.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct AcademicDashboard |bef9b928345b8b52",
    "crates/dashboard/src/screen.rs [pub] impl AcademicDashboard :: #[must_use] const fn secondary_percentage(&self) -> Option<&SecondaryPercentage> |49bc9a551b885f74",
    "crates/dashboard/src/screen.rs [pub] impl AcademicDashboard :: fn assemble( filled: [DashboardSection; DashboardLine::ALL.len()], open: &[OpenGate], secondary: Option<SecondaryPercentage>, ) -> Result<Self, DashboardError> |c8cafe4c4fd6e649",
];

/// Every item of the workspace that reaches `GpaFigure`.
const GPA_FIGURE_ITEMS: [&str; 12] = [
    "crates/dashboard/src/gpa.rs [priv] impl GpaFigure",
    "crates/dashboard/src/gpa.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct GpaFigure |dab5538633effba5",
    "crates/dashboard/src/gpa.rs [pub] impl GpaFigure :: #[must_use] const fn proof(&self) -> &GpaProof |7390c4384530985b",
    "crates/dashboard/src/gpa.rs [pub] impl GpaFigure :: #[must_use] const fn scope(&self) -> GpaScope |c0615f21db41ad07",
    "crates/dashboard/src/gpa.rs [pub] impl GpaFigure :: #[must_use] const fn value(&self) -> &GpaValue |089ea646c3591110",
    "crates/dashboard/src/gpa.rs [pub] impl GpaFigure :: fn publish( scope: GpaScope, value: GpaValue, proof: GpaProof, ) -> Result<Self, DashboardError> |61b7c7cdafdaaccc",
    "crates/dashboard/src/lib.rs [pub] use gpa::{GpaFigure, GpaProof, GpaScope} |78a9542aca8eb23a",
    "crates/dashboard/src/screen.rs [priv] impl AcademicDashboard",
    "crates/dashboard/src/screen.rs [priv] use crate::{ AttemptTimeline, AuditStateReading, DashboardError, GpaFigure, GpaScope, OpenGate, SecondaryPercentage, } |279f0ee1255a771b",
    "crates/dashboard/src/screen.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum DashboardSection |4abdf15e7693fbfe",
    "crates/dashboard/src/screen.rs [pub] impl AcademicDashboard :: #[must_use] fn average(&self, scope: GpaScope) -> Option<&GpaFigure> |2344fdd5bc112d0e",
    "crates/dashboard/src/screen.rs [pub] impl AcademicDashboard :: #[must_use] fn averages(&self) -> Option<&[GpaFigure]> |451976619ac4a002",
];

/// Every item of the workspace that reaches `PlanSnapshot`.
const PLAN_SNAPSHOT_ITEMS: [&str; 8] = [
    "crates/dashboard/src/lib.rs [pub] use planner::{ AxisReading, CandidateOffering, DragOutcome, MeetingSlot, PlanSnapshot, PlannerBoard, PlannerDimension, RequirementContribution, StaleInput, StaleMarking, WorkloadRange, } |30925f475050a2fa",
    "crates/dashboard/src/planner.rs [priv] impl PlanSnapshot",
    "crates/dashboard/src/planner.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct PlanSnapshot |53c4b696b8c99f55",
    "crates/dashboard/src/planner.rs [pub] impl PlanSnapshot :: #[must_use] const fn outcome(&self) -> &DragOutcome |7107f390d3dc7e17",
    "crates/dashboard/src/planner.rs [pub] impl PlanSnapshot :: #[must_use] fn label(&self) -> &str |1afa254cecf1df0d",
    "crates/dashboard/src/planner.rs [pub] impl PlanSnapshot :: #[must_use] fn placed(&self) -> &[CandidateOffering] |c4fa92c637ce6cf1",
    "crates/dashboard/src/planner.rs [pub] impl PlanSnapshot :: #[must_use] fn restate(&self, official: &[CandidateOffering]) -> StaleMarking |7ccb8a2b0ffb6ec4",
    "crates/dashboard/src/planner.rs [pub] impl PlanSnapshot :: fn save(label: impl Into<String>, board: &PlannerBoard) -> Result<Self, DashboardError> |5fc3006ac0bee0cc",
];
