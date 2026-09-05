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
//!   `academic-student-voice` (376) and `academic-capture-gate` (223). Keyed
//!   on nothing: an item added anywhere in either package fails whatever it is
//!   called and whatever kind it is.
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
//! # What this does not claim
//!
//! The two pinned packages are the two whose contract sentences the audit
//! measured broken. The other twenty-two packages that key an inventory on a
//! line prefix are enumerated by
//! [`the_inventories_still_keyed_on_a_line_prefix_are_named`], so what is left
//! open is a list somebody has to edit rather than a silence.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::PathBuf,
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
const CLOSED_TYPES: [ClosedType; 4] = [
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
];

/// The packages whose whole item set is pinned, with the pin.
const PINNED_PACKAGES: [(&str, &[&str]); 2] = [
    ("student-voice", &STUDENT_VOICE_ITEMS),
    ("capture-gate", &CAPTURE_GATE_ITEMS),
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
    for refused in [
        "pub struct Held;\nHeld;\n",
        "pub oddity Thing { }\n",
        "pub fn ready() {}\n7\n",
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

/// Nothing these two packages compile is outside the pinned item set.
///
/// The backstop. It is keyed on nothing at all: not a visibility, not a
/// keyword, not a type name. An item added anywhere in either package is an
/// extra entry whatever it is, which is the statement `P2-A4`'s second audit
/// found three counterexamples to in one afternoon.
#[test]
fn every_item_in_these_packages_is_pinned() -> TestResult {
    for (package, pinned) in PINNED_PACKAGES {
        let items = product_items(package)?;
        let mut keys: Vec<String> = items.iter().map(Item::key).collect();
        keys.sort();
        assert_eq!(
            keys,
            pinned
                .iter()
                .map(|entry| (*entry).to_owned())
                .collect::<Vec<_>>(),
            "the item set of `academic-{package}` changed"
        );
    }
    Ok(())
}

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
        let mut keys: Vec<String> = workspace
            .iter()
            .filter(|item| item.reaches(closed.name))
            .map(Item::key)
            .collect();
        keys.sort();
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

/// What is still keyed on a line prefix, enumerated.
///
/// `P2-A4`'s F2 records that the gap this file closes is in all six `P2-L`
/// packages by construction, and reading for the collector's own shape says it
/// is in seventeen more. Two are closed here. The rest are a list rather than a
/// silence: a package that grows such a collector fails as an extra key, and
/// one rewritten onto the item reader fails as a missing one.
#[test]
fn the_inventories_still_keyed_on_a_line_prefix_are_named() -> TestResult {
    let repository = repository_root()?;
    let markers = [
        format!("starts_with({}pub fn {})", '"', '"'),
        format!("starts_with({}pub const fn {})", '"', '"'),
        format!("starts_with({}impl {})", '"', '"'),
    ];
    let mut found: Vec<String> = Vec::new();
    for directory in crate_directories(&repository)? {
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
                    found.push(relative(&repository, &path));
                }
            }
        }
    }
    found.sort();
    assert_eq!(
        found,
        INVENTORIES_KEYED_ON_A_LINE_PREFIX
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "the set of inventories keyed on a line prefix changed"
    );
    // Neither of the two packages this file pins is off the hook by having
    // been listed: both are still on it, because their own scan files still
    // carry those collectors beside the rules this file adds.
    for package in PINNED_PACKAGES.map(|(package, _)| package) {
        assert!(
            found
                .iter()
                .any(|entry| entry.starts_with(&format!("crates/{package}/"))),
            "the pin claims {package} was rewritten off the line-prefix shape"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The pinned sets
// ---------------------------------------------------------------------------

/// Every item `academic-student-voice`'s product targets compile.
///
/// The backstop under the four sets below. They are keyed on a type's name and
/// this is not keyed on anything: an item added anywhere in this package fails
/// here whatever it is called, whatever kind it is and whatever it names. That
/// is the property `P2-A4`'s second audit found missing three times, once per
/// spelling.
const STUDENT_VOICE_ITEMS: [&str; 376] = [
    "crates/student-voice/src/corpus.rs [priv] fn check_timeline( case: &str, timeline: &'static str, spans: &[VoiceSpan], ) -> Result<(), CorpusFault>",
    "crates/student-voice/src/corpus.rs [priv] fn push_span(text: &mut String, timeline: &str, span: VoiceSpan)",
    "crates/student-voice/src/corpus.rs [priv] impl DiarizationCase",
    "crates/student-voice/src/corpus.rs [priv] impl DiarizationCorpus",
    "crates/student-voice/src/corpus.rs [priv] impl VoiceClass",
    "crates/student-voice/src/corpus.rs [priv] impl VoiceSpan",
    "crates/student-voice/src/corpus.rs [priv] use academic_domain::ContentDigest",
    "crates/student-voice/src/corpus.rs [priv] use academic_transcription::Speaker",
    "crates/student-voice/src/corpus.rs [priv] use crate::fault::CorpusFault",
    "crates/student-voice/src/corpus.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] enum VoiceClass",
    "crates/student-voice/src/corpus.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] struct VoiceSpan",
    "crates/student-voice/src/corpus.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct DiarizationCase",
    "crates/student-voice/src/corpus.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct DiarizationCorpus",
    "crates/student-voice/src/corpus.rs [pub] const CORPUS_ID: &str",
    "crates/student-voice/src/corpus.rs [pub] const CORPUS_ROOT: &str",
    "crates/student-voice/src/corpus.rs [pub] const CORPUS_VERSION: u32",
    "crates/student-voice/src/corpus.rs [pub] fn corpus_v1() -> Result<DiarizationCorpus, CorpusFault>",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCase :: #[must_use] fn canonical_bytes(&self) -> Vec<u8>",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCase :: #[must_use] fn hypothesis(&self) -> &[VoiceSpan]",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCase :: #[must_use] fn name(&self) -> &str",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCase :: #[must_use] fn reference(&self) -> &[VoiceSpan]",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCase :: #[must_use] fn reference_ms(&self) -> u64",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCase :: #[must_use] fn reference_student_ms(&self) -> u64",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCase :: fn new( name: &str, reference: Vec<VoiceSpan>, hypothesis: Vec<VoiceSpan>, ) -> Result<Self, CorpusFault>",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCorpus :: #[must_use] const fn version(&self) -> u32",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCorpus :: #[must_use] fn canonical_bytes(&self) -> Vec<u8>",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCorpus :: #[must_use] fn cases(&self) -> &[DiarizationCase]",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCorpus :: #[must_use] fn digest(&self) -> ContentDigest",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCorpus :: #[must_use] fn id(&self) -> &str",
    "crates/student-voice/src/corpus.rs [pub] impl DiarizationCorpus :: fn new(id: &str, version: u32, cases: Vec<DiarizationCase>) -> Result<Self, CorpusFault>",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceClass :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceClass :: #[must_use] const fn of(speaker: Speaker) -> Self",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceClass :: const ALL: [Self; 3]",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceSpan :: #[must_use] const fn duration_ms(self) -> u64",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceSpan :: #[must_use] const fn end_ms(self) -> u64",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceSpan :: #[must_use] const fn new(start_ms: u64, end_ms: u64, speaker: Speaker) -> Self",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceSpan :: #[must_use] const fn overlap_ms(self, other: Self) -> u64",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceSpan :: #[must_use] const fn speaker(self) -> Speaker",
    "crates/student-voice/src/corpus.rs [pub] impl VoiceSpan :: #[must_use] const fn start_ms(self) -> u64",
    "crates/student-voice/src/derivative.rs [priv] fn excluded_indices( plan: &RedactionPlan, source: &LectureSource<'_>, ) -> Result<Vec<usize>, RedactionFault>",
    "crates/student-voice/src/derivative.rs [priv] fn original_digest(source: &LectureSource<'_>) -> ContentDigest",
    "crates/student-voice/src/derivative.rs [priv] impl DerivedArtifact",
    "crates/student-voice/src/derivative.rs [priv] impl DisclosedOriginal<'_>",
    "crates/student-voice/src/derivative.rs [priv] impl ExclusionRecord",
    "crates/student-voice/src/derivative.rs [priv] impl KeptUtterance",
    "crates/student-voice/src/derivative.rs [priv] impl ManualExclusion",
    "crates/student-voice/src/derivative.rs [priv] impl RawAccessGrant",
    "crates/student-voice/src/derivative.rs [priv] impl RawAccessLog",
    "crates/student-voice/src/derivative.rs [priv] impl RawAccessRecord",
    "crates/student-voice/src/derivative.rs [priv] impl RedactedDerivative",
    "crates/student-voice/src/derivative.rs [priv] impl Redaction",
    "crates/student-voice/src/derivative.rs [priv] impl RedactionMode",
    "crates/student-voice/src/derivative.rs [priv] impl RedactionPlan",
    "crates/student-voice/src/derivative.rs [priv] impl RestrictedOriginal",
    "crates/student-voice/src/derivative.rs [priv] impl fmt::Debug for KeptUtterance",
    "crates/student-voice/src/derivative.rs [priv] impl fmt::Debug for KeptUtterance :: fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/student-voice/src/derivative.rs [priv] impl fmt::Debug for RemovedUtterance",
    "crates/student-voice/src/derivative.rs [priv] impl fmt::Debug for RemovedUtterance :: fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/student-voice/src/derivative.rs [priv] impl fmt::Debug for SourceUtterance<'_>",
    "crates/student-voice/src/derivative.rs [priv] impl fmt::Debug for SourceUtterance<'_> :: fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/student-voice/src/derivative.rs [priv] impl<'a> LectureSource<'a>",
    "crates/student-voice/src/derivative.rs [priv] impl<'a> SourceUtterance<'a>",
    "crates/student-voice/src/derivative.rs [priv] use academic_consent::{DerivativeClass, RetentionTerms}",
    "crates/student-voice/src/derivative.rs [priv] use academic_domain::{Actor, ContentDigest, LectureSessionId}",
    "crates/student-voice/src/derivative.rs [priv] use academic_lecture_document::RedactionPolicyRef",
    "crates/student-voice/src/derivative.rs [priv] use academic_transcription::{Speaker, TranscriptLineage}",
    "crates/student-voice/src/derivative.rs [priv] use crate::{ fault::{AccessRefusal, RedactionFault}, measure::AccuracyWitness, policy::RedactionPolicy, }",
    "crates/student-voice/src/derivative.rs [priv] use std::fmt",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Clone, Copy, PartialEq, Eq)] struct SourceUtterance<'a>",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Clone, PartialEq, Eq)] struct KeptUtterance",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Clone, PartialEq, Eq)] struct RemovedUtterance",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq)] struct ExclusionRecord",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, Default, PartialEq, Eq)] struct RawAccessLog",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum RedactionMode",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct DerivedArtifact",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct LectureSource<'a>",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct ManualExclusion",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct RawAccessRecord",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct RedactedDerivative",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct Redaction",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct RedactionPlan",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct RestrictedOriginal",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, PartialEq, Eq)] struct DisclosedOriginal<'a>",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, PartialEq, Eq)] struct RawAccessGrant",
    "crates/student-voice/src/derivative.rs [pub] #[must_use] fn inherit_terms(parent: RetentionTerms, requested: RetentionTerms) -> RetentionTerms",
    "crates/student-voice/src/derivative.rs [pub] const ORIGINAL_CLASSIFICATION: &str",
    "crates/student-voice/src/derivative.rs [pub] fn redact( plan: &RedactionPlan, reference: &RedactionPolicyRef, source: &LectureSource<'_>, requested: RetentionTerms, ) -> Result<Redaction, RedactionFault>",
    "crates/student-voice/src/derivative.rs [pub] impl DerivedArtifact :: #[must_use] const fn class(&self) -> DerivativeClass",
    "crates/student-voice/src/derivative.rs [pub] impl DerivedArtifact :: #[must_use] const fn parent_digest(&self) -> &ContentDigest",
    "crates/student-voice/src/derivative.rs [pub] impl DerivedArtifact :: #[must_use] const fn terms(&self) -> RetentionTerms",
    "crates/student-voice/src/derivative.rs [pub] impl DerivedArtifact :: #[must_use] fn digest(&self) -> ContentDigest",
    "crates/student-voice/src/derivative.rs [pub] impl DerivedArtifact :: #[must_use] fn of_artifact(parent: &Self, class: DerivativeClass, requested: RetentionTerms) -> Self",
    "crates/student-voice/src/derivative.rs [pub] impl DerivedArtifact :: #[must_use] fn of_derivative( parent: &RedactedDerivative, class: DerivativeClass, requested: RetentionTerms, ) -> Self",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] const fn is_empty(&self) -> bool",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] const fn len(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn source_index(&self, position: usize) -> Option<usize>",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn speaker(&self, position: usize) -> Option<Speaker>",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn verbatim(&self, position: usize) -> Option<&str>",
    "crates/student-voice/src/derivative.rs [pub] impl ExclusionRecord :: #[must_use] const fn duration_nanos(&self) -> u64",
    "crates/student-voice/src/derivative.rs [pub] impl ExclusionRecord :: #[must_use] const fn end_nanos(&self) -> u64",
    "crates/student-voice/src/derivative.rs [pub] impl ExclusionRecord :: #[must_use] const fn index(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl ExclusionRecord :: #[must_use] const fn speaker(&self) -> Speaker",
    "crates/student-voice/src/derivative.rs [pub] impl ExclusionRecord :: #[must_use] const fn start_nanos(&self) -> u64",
    "crates/student-voice/src/derivative.rs [pub] impl KeptUtterance :: #[must_use] const fn end_nanos(&self) -> u64",
    "crates/student-voice/src/derivative.rs [pub] impl KeptUtterance :: #[must_use] const fn index(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl KeptUtterance :: #[must_use] const fn speaker(&self) -> Speaker",
    "crates/student-voice/src/derivative.rs [pub] impl KeptUtterance :: #[must_use] const fn start_nanos(&self) -> u64",
    "crates/student-voice/src/derivative.rs [pub] impl KeptUtterance :: #[must_use] fn text(&self) -> &str",
    "crates/student-voice/src/derivative.rs [pub] impl ManualExclusion :: #[must_use] const fn decided_by(&self) -> &Actor",
    "crates/student-voice/src/derivative.rs [pub] impl ManualExclusion :: #[must_use] const fn index(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl ManualExclusion :: fn decided(index: usize, decided_by: Actor) -> Result<Self, RedactionFault>",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessGrant :: #[must_use] const fn original_digest(&self) -> &ContentDigest",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessGrant :: #[must_use] const fn requested_by(&self) -> &Actor",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessGrant :: #[must_use] fn purpose(&self) -> &str",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessGrant :: fn issued( original: &RestrictedOriginal, requested_by: Actor, purpose: &str, at: u64, ) -> Result<Self, AccessRefusal>",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessLog :: #[must_use] const fn new() -> Self",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessLog :: #[must_use] fn entries(&self) -> &[RawAccessRecord]",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessRecord :: #[must_use] const fn at(&self) -> u64",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessRecord :: #[must_use] const fn opened_by(&self) -> &Actor",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessRecord :: #[must_use] const fn original_digest(&self) -> &ContentDigest",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessRecord :: #[must_use] const fn utterances_disclosed(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessRecord :: #[must_use] fn purpose(&self) -> &str",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] const fn lecture(&self) -> LectureSessionId",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] const fn mode(&self) -> &RedactionMode",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] const fn policy_digest(&self) -> &ContentDigest",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] const fn source_version(&self) -> u32",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] const fn terms(&self) -> RetentionTerms",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] fn canonical_bytes(&self) -> Vec<u8>",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] fn digest(&self) -> ContentDigest",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] fn excluded(&self) -> &[ExclusionRecord]",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] fn inherit_for_child(&self, requested: RetentionTerms) -> RetentionTerms",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] fn keeps_a_targeted_speaker(&self, policy: &RedactionPolicy) -> bool",
    "crates/student-voice/src/derivative.rs [pub] impl RedactedDerivative :: #[must_use] fn kept(&self) -> &[KeptUtterance]",
    "crates/student-voice/src/derivative.rs [pub] impl Redaction :: #[must_use] const fn derivative(&self) -> &RedactedDerivative",
    "crates/student-voice/src/derivative.rs [pub] impl Redaction :: #[must_use] const fn original(&self) -> &RestrictedOriginal",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionMode :: #[must_use] const fn as_str(&self) -> &'static str",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionMode :: #[must_use] const fn witness(&self) -> Option<&AccuracyWitness>",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionPlan :: #[must_use] const fn mode(&self) -> &RedactionMode",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionPlan :: #[must_use] const fn policy(&self) -> &RedactionPolicy",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionPlan :: #[must_use] fn automatic(policy: RedactionPolicy, witness: AccuracyWitness) -> Self",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionPlan :: #[must_use] fn manual_exclusions(&self) -> &[ManualExclusion]",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionPlan :: fn manual( policy: RedactionPolicy, exclusions: Vec<ManualExclusion>, ) -> Result<Self, RedactionFault>",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn classification(&self) -> &'static str",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn digest(&self) -> &ContentDigest",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn lecture(&self) -> LectureSessionId",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn source_version(&self) -> u32",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn terms(&self) -> RetentionTerms",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] fn removed_count(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: fn open( &self, grant: RawAccessGrant, log: &mut RawAccessLog, ) -> Result<DisclosedOriginal<'_>, AccessRefusal>",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> LectureSource<'a> :: #[must_use] const fn lecture(&self) -> LectureSessionId",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> LectureSource<'a> :: #[must_use] const fn terms(&self) -> RetentionTerms",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> LectureSource<'a> :: #[must_use] const fn version(&self) -> u32",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> LectureSource<'a> :: #[must_use] fn utterances(&self) -> &[SourceUtterance<'a>]",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> LectureSource<'a> :: fn of( lineage: &'a TranscriptLineage, version: u32, terms: RetentionTerms, ) -> Result<Self, RedactionFault>",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> SourceUtterance<'a> :: #[must_use] const fn end_nanos(&self) -> u64",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> SourceUtterance<'a> :: #[must_use] const fn index(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> SourceUtterance<'a> :: #[must_use] const fn speaker(&self) -> Speaker",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> SourceUtterance<'a> :: #[must_use] const fn start_nanos(&self) -> u64",
    "crates/student-voice/src/derivative.rs [pub] impl<'a> SourceUtterance<'a> :: #[must_use] const fn verbatim(&self) -> &'a str",
    "crates/student-voice/src/fault.rs [priv] use academic_domain::ContentDigest",
    "crates/student-voice/src/fault.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] enum AccessRefusal",
    "crates/student-voice/src/fault.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] enum AccuracyRefusal",
    "crates/student-voice/src/fault.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] enum DeletionFault",
    "crates/student-voice/src/fault.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] enum ThresholdFault",
    "crates/student-voice/src/fault.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] enum CorpusFault",
    "crates/student-voice/src/fault.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] enum HoldRefusal",
    "crates/student-voice/src/fault.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] enum RedactionFault",
    "crates/student-voice/src/harness.rs [priv] use crate::{ corpus::{CORPUS_ROOT, DiarizationCorpus, corpus_v1}, fault::CorpusFault, measure::{measure, measure_case}, }",
    "crates/student-voice/src/harness.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct CorpusFile",
    "crates/student-voice/src/harness.rs [pub] #[must_use] fn corpus_dir(corpus: &DiarizationCorpus) -> String",
    "crates/student-voice/src/harness.rs [pub] fn corpus_files() -> Result<Vec<CorpusFile>, CorpusFault>",
    "crates/student-voice/src/hold.rs [priv] impl CaptureUnderReview",
    "crates/student-voice/src/hold.rs [priv] impl HoldState",
    "crates/student-voice/src/hold.rs [priv] impl IngestionJobKind",
    "crates/student-voice/src/hold.rs [priv] impl IngestionReceipt",
    "crates/student-voice/src/hold.rs [priv] impl PiiClass",
    "crates/student-voice/src/hold.rs [priv] impl PiiFinding",
    "crates/student-voice/src/hold.rs [priv] impl ReviewDecision",
    "crates/student-voice/src/hold.rs [priv] impl ReviewOutcome",
    "crates/student-voice/src/hold.rs [priv] impl ReviewedCapture<'_>",
    "crates/student-voice/src/hold.rs [priv] trait IngestionStage :: fn ingest(&mut self, capture: &ReviewedCapture<'_>)",
    "crates/student-voice/src/hold.rs [priv] use academic_capture::CaptureBytes",
    "crates/student-voice/src/hold.rs [priv] use academic_domain::{Actor, ContentDigest}",
    "crates/student-voice/src/hold.rs [priv] use crate::fault::HoldRefusal",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq)] struct IngestionReceipt",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] enum IngestionJobKind",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] enum PiiClass",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] enum ReviewOutcome",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum HoldState",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct CaptureUnderReview",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct PiiFinding",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct ReviewDecision",
    "crates/student-voice/src/hold.rs [pub] #[derive(Debug, PartialEq, Eq)] struct ReviewedCapture<'a>",
    "crates/student-voice/src/hold.rs [pub] fn dispatch<S: IngestionStage + ?Sized>( stage: &mut S, kind: IngestionJobKind, capture: &CaptureUnderReview, ) -> Result<IngestionReceipt, HoldRefusal>",
    "crates/student-voice/src/hold.rs [pub] impl CaptureUnderReview :: #[must_use] const fn byte_len(&self) -> usize",
    "crates/student-voice/src/hold.rs [pub] impl CaptureUnderReview :: #[must_use] const fn digest(&self) -> &ContentDigest",
    "crates/student-voice/src/hold.rs [pub] impl CaptureUnderReview :: #[must_use] const fn review(&self) -> Option<&ReviewDecision>",
    "crates/student-voice/src/hold.rs [pub] impl CaptureUnderReview :: #[must_use] fn findings(&self) -> &[PiiFinding]",
    "crates/student-voice/src/hold.rs [pub] impl CaptureUnderReview :: #[must_use] fn hold_state(&self) -> HoldState",
    "crates/student-voice/src/hold.rs [pub] impl CaptureUnderReview :: #[must_use] fn screened(bytes: CaptureBytes, findings: Vec<PiiFinding>) -> Self",
    "crates/student-voice/src/hold.rs [pub] impl CaptureUnderReview :: fn record_review(&mut self, decision: ReviewDecision) -> Result<(), HoldRefusal>",
    "crates/student-voice/src/hold.rs [pub] impl HoldState :: #[must_use] const fn is_held(&self) -> bool",
    "crates/student-voice/src/hold.rs [pub] impl IngestionJobKind :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/student-voice/src/hold.rs [pub] impl IngestionJobKind :: #[must_use] const fn spec_word(self) -> &'static str",
    "crates/student-voice/src/hold.rs [pub] impl IngestionJobKind :: const ALL: [Self; 2]",
    "crates/student-voice/src/hold.rs [pub] impl IngestionReceipt :: #[must_use] const fn digest(&self) -> &ContentDigest",
    "crates/student-voice/src/hold.rs [pub] impl IngestionReceipt :: #[must_use] const fn kind(&self) -> IngestionJobKind",
    "crates/student-voice/src/hold.rs [pub] impl PiiClass :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/student-voice/src/hold.rs [pub] impl PiiClass :: #[must_use] const fn spec_phrase(self) -> &'static str",
    "crates/student-voice/src/hold.rs [pub] impl PiiClass :: const ALL: [Self; 3]",
    "crates/student-voice/src/hold.rs [pub] impl PiiFinding :: #[must_use] const fn class(&self) -> PiiClass",
    "crates/student-voice/src/hold.rs [pub] impl PiiFinding :: #[must_use] const fn detected_by(&self) -> &Actor",
    "crates/student-voice/src/hold.rs [pub] impl PiiFinding :: #[must_use] const fn found(class: PiiClass, detected_by: Actor) -> Self",
    "crates/student-voice/src/hold.rs [pub] impl ReviewDecision :: #[must_use] const fn at(&self) -> u64",
    "crates/student-voice/src/hold.rs [pub] impl ReviewDecision :: #[must_use] const fn capture_digest(&self) -> &ContentDigest",
    "crates/student-voice/src/hold.rs [pub] impl ReviewDecision :: #[must_use] const fn outcome(&self) -> ReviewOutcome",
    "crates/student-voice/src/hold.rs [pub] impl ReviewDecision :: #[must_use] const fn reviewed_by(&self) -> &Actor",
    "crates/student-voice/src/hold.rs [pub] impl ReviewDecision :: #[must_use] fn addressed(&self) -> &[PiiClass]",
    "crates/student-voice/src/hold.rs [pub] impl ReviewDecision :: fn recorded( capture_digest: ContentDigest, addressed: Vec<PiiClass>, outcome: ReviewOutcome, reviewed_by: Actor, at: u64, ) -> Result<Self, HoldRefusal>",
    "crates/student-voice/src/hold.rs [pub] impl ReviewOutcome :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/student-voice/src/hold.rs [pub] impl ReviewOutcome :: const ALL: [Self; 2]",
    "crates/student-voice/src/hold.rs [pub] impl ReviewedCapture<'_> :: #[must_use] const fn digest(&self) -> &ContentDigest",
    "crates/student-voice/src/hold.rs [pub] impl ReviewedCapture<'_> :: #[must_use] const fn kind(&self) -> IngestionJobKind",
    "crates/student-voice/src/hold.rs [pub] impl ReviewedCapture<'_> :: #[must_use] fn bytes(&self) -> &[u8]",
    "crates/student-voice/src/hold.rs [pub] trait IngestionStage",
    "crates/student-voice/src/lib.rs [priv] mod corpus",
    "crates/student-voice/src/lib.rs [priv] mod derivative",
    "crates/student-voice/src/lib.rs [priv] mod fault",
    "crates/student-voice/src/lib.rs [priv] mod hold",
    "crates/student-voice/src/lib.rs [priv] mod measure",
    "crates/student-voice/src/lib.rs [priv] mod policy",
    "crates/student-voice/src/lib.rs [priv] mod preview",
    "crates/student-voice/src/lib.rs [pub] mod harness",
    "crates/student-voice/src/lib.rs [pub] use corpus::{ CORPUS_ID, CORPUS_ROOT, CORPUS_VERSION, DiarizationCase, DiarizationCorpus, VoiceClass, VoiceSpan, corpus_v1, }",
    "crates/student-voice/src/lib.rs [pub] use derivative::{ DerivedArtifact, DisclosedOriginal, ExclusionRecord, KeptUtterance, LectureSource, ManualExclusion, ORIGINAL_CLASSIFICATION, RawAccessGrant, RawAccessLog, RawAccessRecord, RedactedDerivative, Redaction, RedactionMode, RedactionPlan, RestrictedOriginal, SourceUtterance, inherit_terms, redact, }",
    "crates/student-voice/src/lib.rs [pub] use fault::{ AccessRefusal, AccuracyRefusal, CorpusFault, DeletionFault, HoldRefusal, RedactionFault, ThresholdFault, }",
    "crates/student-voice/src/lib.rs [pub] use hold::{ CaptureUnderReview, HoldState, IngestionJobKind, IngestionReceipt, IngestionStage, PiiClass, PiiFinding, ReviewDecision, ReviewOutcome, ReviewedCapture, dispatch, }",
    "crates/student-voice/src/lib.rs [pub] use measure::{ ABSOLUTE_ACCURACY_FLOOR, ABSOLUTE_MISSED_STUDENT_CEILING, AccuracyWitness, CaseMeasurement, DIARIZATION_THRESHOLD_V1, DiarizationMeasurement, DiarizationThreshold, SCORER_VERSION, measure, measure_case, }",
    "crates/student-voice/src/lib.rs [pub] use policy::{GATE_38_026_OPEN, RedactionPolicy, RedactionScope, SpeakerTargeting}",
    "crates/student-voice/src/lib.rs [pub] use preview::{ AffectedProjection, AffectedProjectionKind, DeletionOutcome, EvidenceIndex, LectureDeletionPlan, LectureDeletionPreview, ProjectionEffect, ProjectionRecord, affected_projections, apply_deletion, preview_deletion, unreferenced_objects, }",
    "crates/student-voice/src/measure.rs [priv] const fn permille(numerator: u64, denominator: u64) -> u64",
    "crates/student-voice/src/measure.rs [priv] fn attribute( measured: &mut CaseMeasurement, reference_class: VoiceClass, hypothesis: VoiceSpan, overlap: u64, )",
    "crates/student-voice/src/measure.rs [priv] impl AccuracyWitness",
    "crates/student-voice/src/measure.rs [priv] impl CaseMeasurement",
    "crates/student-voice/src/measure.rs [priv] impl DiarizationMeasurement",
    "crates/student-voice/src/measure.rs [priv] impl DiarizationThreshold",
    "crates/student-voice/src/measure.rs [priv] use academic_domain::ContentDigest",
    "crates/student-voice/src/measure.rs [priv] use crate::{ corpus::{DiarizationCase, DiarizationCorpus, VoiceClass, VoiceSpan}, fault::{AccuracyRefusal, ThresholdFault}, }",
    "crates/student-voice/src/measure.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] struct DiarizationThreshold",
    "crates/student-voice/src/measure.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct AccuracyWitness",
    "crates/student-voice/src/measure.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct CaseMeasurement",
    "crates/student-voice/src/measure.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct DiarizationMeasurement",
    "crates/student-voice/src/measure.rs [pub] #[must_use] fn measure(corpus: &DiarizationCorpus) -> DiarizationMeasurement",
    "crates/student-voice/src/measure.rs [pub] #[must_use] fn measure_case(case: &DiarizationCase) -> CaseMeasurement",
    "crates/student-voice/src/measure.rs [pub] const ABSOLUTE_ACCURACY_FLOOR: u64",
    "crates/student-voice/src/measure.rs [pub] const ABSOLUTE_MISSED_STUDENT_CEILING: u64",
    "crates/student-voice/src/measure.rs [pub] const DIARIZATION_THRESHOLD_V1: DiarizationThreshold",
    "crates/student-voice/src/measure.rs [pub] const SCORER_VERSION: u32",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn accuracy_permille(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn corpus_digest(&self) -> &ContentDigest",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn corpus_version(&self) -> u32",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn missed_student_permille(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn scorer_version(&self) -> u32",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn threshold(&self) -> DiarizationThreshold",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] fn corpus_id(&self) -> &str",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn agreed_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn instructor_as_student_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn partition_reconciles(&self) -> bool",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn reference_student_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn scored_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn student_agreed_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn student_as_instructor_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn unattributed_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] const fn uncovered_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] fn canonical_bytes(&self) -> Vec<u8>",
    "crates/student-voice/src/measure.rs [pub] impl CaseMeasurement :: #[must_use] fn case(&self) -> &str",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn accuracy_permille(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn agreed_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn corpus_digest(&self) -> &ContentDigest",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn corpus_version(&self) -> u32",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn instructor_as_student_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn missed_student_permille(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn partition_reconciles(&self) -> bool",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn reference_student_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn scored_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn scorer_version(&self) -> u32",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn student_agreed_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn student_as_instructor_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn student_recall_permille(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn unattributed_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] const fn uncovered_ms(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] fn canonical_bytes(&self) -> Vec<u8>",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] fn cases(&self) -> &[CaseMeasurement]",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: #[must_use] fn corpus_id(&self) -> &str",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: fn witness( &self, threshold: DiarizationThreshold, ) -> Result<AccuracyWitness, AccuracyRefusal>",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationThreshold :: #[must_use] const fn max_missed_student_permille(self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationThreshold :: #[must_use] const fn min_accuracy_permille(self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationThreshold :: #[must_use] const fn version(self) -> u32",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationThreshold :: const fn new( version: u32, min_accuracy_permille: u64, max_missed_student_permille: u64, ) -> Result<Self, ThresholdFault>",
    "crates/student-voice/src/policy.rs [priv] impl RedactionPolicy",
    "crates/student-voice/src/policy.rs [priv] impl RedactionScope",
    "crates/student-voice/src/policy.rs [priv] impl SpeakerTargeting",
    "crates/student-voice/src/policy.rs [priv] use academic_domain::{Actor, ContentDigest}",
    "crates/student-voice/src/policy.rs [priv] use academic_lecture_document::{RedactionBasis, RedactionPolicyRef}",
    "crates/student-voice/src/policy.rs [priv] use academic_transcription::Speaker",
    "crates/student-voice/src/policy.rs [priv] use crate::{corpus::VoiceClass, fault::RedactionFault}",
    "crates/student-voice/src/policy.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] enum RedactionScope",
    "crates/student-voice/src/policy.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive] enum SpeakerTargeting",
    "crates/student-voice/src/policy.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct RedactionPolicy",
    "crates/student-voice/src/policy.rs [pub] const GATE_38_026_OPEN: &str",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] const fn basis(&self) -> RedactionBasis",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] const fn decided_by(&self) -> &Actor",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] const fn scope(&self) -> RedactionScope",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] const fn targeting(&self) -> &SpeakerTargeting",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] const fn version(&self) -> u32",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] fn canonical_bytes(&self) -> Vec<u8>",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] fn digest(&self) -> ContentDigest",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] fn resolves(&self, reference: &RedactionPolicyRef) -> bool",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: #[must_use] fn targets(&self, speaker: Speaker) -> bool",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: fn published( version: u32, basis: RedactionBasis, targeting: SpeakerTargeting, scope: RedactionScope, decided_by: Actor, ) -> Result<Self, RedactionFault>",
    "crates/student-voice/src/policy.rs [pub] impl RedactionPolicy :: fn resolve(&self, reference: &RedactionPolicyRef) -> Result<(), RedactionFault>",
    "crates/student-voice/src/policy.rs [pub] impl RedactionScope :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/student-voice/src/policy.rs [pub] impl RedactionScope :: const ALL: [Self; 1]",
    "crates/student-voice/src/policy.rs [pub] impl SpeakerTargeting :: #[must_use] const fn kind_str(&self) -> &'static str",
    "crates/student-voice/src/policy.rs [pub] impl SpeakerTargeting :: #[must_use] fn targets(&self, speaker: Speaker) -> bool",
    "crates/student-voice/src/preview.rs [priv] impl AffectedProjection",
    "crates/student-voice/src/preview.rs [priv] impl AffectedProjectionKind",
    "crates/student-voice/src/preview.rs [priv] impl DeletionOutcome",
    "crates/student-voice/src/preview.rs [priv] impl EvidenceIndex",
    "crates/student-voice/src/preview.rs [priv] impl LectureDeletionPlan",
    "crates/student-voice/src/preview.rs [priv] impl LectureDeletionPreview",
    "crates/student-voice/src/preview.rs [priv] impl ProjectionEffect",
    "crates/student-voice/src/preview.rs [priv] impl ProjectionRecord",
    "crates/student-voice/src/preview.rs [priv] use academic_consent::{ ConsentLedger, DeletionImpact, ExpiryPlan, ExpiryRefusal, SubjectInventory, apply_expiry, preview_expiry, }",
    "crates/student-voice/src/preview.rs [priv] use academic_domain::ContentDigest",
    "crates/student-voice/src/preview.rs [priv] use crate::fault::DeletionFault",
    "crates/student-voice/src/preview.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq)] struct DeletionOutcome",
    "crates/student-voice/src/preview.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] enum AffectedProjectionKind",
    "crates/student-voice/src/preview.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] enum ProjectionEffect",
    "crates/student-voice/src/preview.rs [pub] #[derive(Debug, Clone, Default, PartialEq, Eq)] struct EvidenceIndex",
    "crates/student-voice/src/preview.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct AffectedProjection",
    "crates/student-voice/src/preview.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct LectureDeletionPlan",
    "crates/student-voice/src/preview.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct LectureDeletionPreview",
    "crates/student-voice/src/preview.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct ProjectionRecord",
    "crates/student-voice/src/preview.rs [pub] #[must_use] fn affected_projections( index: &EvidenceIndex, deleted: &[ContentDigest], ) -> Vec<AffectedProjection>",
    "crates/student-voice/src/preview.rs [pub] #[must_use] fn preview_deletion( ledger: &mut ConsentLedger, subject: &SubjectInventory, index: &EvidenceIndex, deleted: &[ContentDigest], at: u64, ) -> LectureDeletionPreview",
    "crates/student-voice/src/preview.rs [pub] #[must_use] fn unreferenced_objects( index: &EvidenceIndex, deleted: &[ContentDigest], ) -> Vec<ContentDigest>",
    "crates/student-voice/src/preview.rs [pub] fn apply_deletion( ledger: &mut ConsentLedger, plan: &LectureDeletionPlan, shown: &ContentDigest, at: u64, ) -> Result<DeletionOutcome, DeletionFault>",
    "crates/student-voice/src/preview.rs [pub] impl AffectedProjection :: #[must_use] const fn cited_deleted(&self) -> usize",
    "crates/student-voice/src/preview.rs [pub] impl AffectedProjection :: #[must_use] const fn cited_total(&self) -> usize",
    "crates/student-voice/src/preview.rs [pub] impl AffectedProjection :: #[must_use] const fn effect(&self) -> ProjectionEffect",
    "crates/student-voice/src/preview.rs [pub] impl AffectedProjection :: #[must_use] const fn kind(&self) -> AffectedProjectionKind",
    "crates/student-voice/src/preview.rs [pub] impl AffectedProjection :: #[must_use] fn id(&self) -> &str",
    "crates/student-voice/src/preview.rs [pub] impl AffectedProjectionKind :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/student-voice/src/preview.rs [pub] impl AffectedProjectionKind :: #[must_use] const fn spec_word(self) -> &'static str",
    "crates/student-voice/src/preview.rs [pub] impl AffectedProjectionKind :: const ALL: [Self; 2]",
    "crates/student-voice/src/preview.rs [pub] impl DeletionOutcome :: #[must_use] const fn objects_reached(&self) -> u64",
    "crates/student-voice/src/preview.rs [pub] impl DeletionOutcome :: #[must_use] const fn projections_affected(&self) -> usize",
    "crates/student-voice/src/preview.rs [pub] impl EvidenceIndex :: #[must_use] fn projections(&self) -> &[ProjectionRecord]",
    "crates/student-voice/src/preview.rs [pub] impl EvidenceIndex :: fn of(projections: Vec<ProjectionRecord>) -> Result<Self, DeletionFault>",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPlan :: #[must_use] const fn from_preview(preview: LectureDeletionPreview) -> Self",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPlan :: #[must_use] const fn preview(&self) -> &LectureDeletionPreview",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPreview :: #[must_use] const fn digest(&self) -> &ContentDigest",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPreview :: #[must_use] const fn impact(&self) -> &DeletionImpact",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPreview :: #[must_use] const fn previewed_at(&self) -> u64",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPreview :: #[must_use] fn canonical_bytes(&self) -> Vec<u8>",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPreview :: #[must_use] fn deleted(&self) -> &[ContentDigest]",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPreview :: #[must_use] fn partition_reconciles(&self, index: &EvidenceIndex) -> bool",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPreview :: #[must_use] fn projections(&self) -> &[AffectedProjection]",
    "crates/student-voice/src/preview.rs [pub] impl LectureDeletionPreview :: #[must_use] fn unreferenced(&self) -> &[ContentDigest]",
    "crates/student-voice/src/preview.rs [pub] impl ProjectionEffect :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/student-voice/src/preview.rs [pub] impl ProjectionEffect :: const ALL: [Self; 2]",
    "crates/student-voice/src/preview.rs [pub] impl ProjectionRecord :: #[must_use] const fn kind(&self) -> AffectedProjectionKind",
    "crates/student-voice/src/preview.rs [pub] impl ProjectionRecord :: #[must_use] fn cites(&self) -> &[ContentDigest]",
    "crates/student-voice/src/preview.rs [pub] impl ProjectionRecord :: #[must_use] fn id(&self) -> &str",
    "crates/student-voice/src/preview.rs [pub] impl ProjectionRecord :: fn citing( kind: AffectedProjectionKind, id: &str, cites: Vec<ContentDigest>, ) -> Result<Self, DeletionFault>",
];

/// Every item `academic-capture-gate`'s product targets compile.
///
/// `crates/capture-gate/src/lib.rs` says "`ReleasableArtifact::bytes` is the
/// one accessor in this crate, and a workspace-wide signature rule refuses a
/// second one written anywhere else". `P2-A4`'s second audit wrote a second one
/// as a `pub const` function pointer and measured 27 passed, 0 failed on both
/// hosts. The sentence is true of this set.
const CAPTURE_GATE_ITEMS: [&str; 223] = [
    "crates/capture-gate/probes/capture_probe.rs [priv] fn attempt(target: &str) -> String",
    "crates/capture-gate/probes/capture_probe.rs [priv] fn main()",
    "crates/capture-gate/probes/capture_probe.rs [priv] use academic_capture_gate::native::{REPORT_DIR_VAR, REPORT_FILE}",
    "crates/capture-gate/probes/capture_probe.rs [priv] use std::{fmt::Write as _, fs, path::Path}",
    "crates/capture-gate/src/artifact.rs [priv] impl CaptureArtifact",
    "crates/capture-gate/src/artifact.rs [priv] impl CaptureManifest",
    "crates/capture-gate/src/artifact.rs [priv] impl ChunkRecord",
    "crates/capture-gate/src/artifact.rs [priv] impl QuarantinedArtifact",
    "crates/capture-gate/src/artifact.rs [priv] impl ReleasableArtifact",
    "crates/capture-gate/src/artifact.rs [priv] impl TimelineGap",
    "crates/capture-gate/src/artifact.rs [priv] impl ViolationRisk",
    "crates/capture-gate/src/artifact.rs [priv] impl fmt::Debug for ReleasableArtifact",
    "crates/capture-gate/src/artifact.rs [priv] impl fmt::Debug for ReleasableArtifact :: fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/capture-gate/src/artifact.rs [priv] use academic_consent::{CaptureDenialReason, CaptureStatus, RetentionTerms}",
    "crates/capture-gate/src/artifact.rs [priv] use academic_domain::ContentDigest",
    "crates/capture-gate/src/artifact.rs [priv] use crate::audit::CaptureRefusalReason",
    "crates/capture-gate/src/artifact.rs [priv] use std::fmt",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl CaptureArtifact :: const fn quarantined(manifest: CaptureManifest, risk: ViolationRisk) -> Self",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl CaptureArtifact :: const fn releasable(manifest: CaptureManifest, bytes: Vec<u8>) -> Self",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl CaptureArtifact :: fn manifest_of( chunks: Vec<ChunkRecord>, byte_len: usize, digest: ContentDigest, retention: RetentionTerms, gap: Option<TimelineGap>, ) -> CaptureManifest",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl ChunkRecord :: const fn build( seq: u32, started_at: u64, byte_len: usize, digest: ContentDigest, ) -> Self",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl TimelineGap :: const fn opened( from: u64, cause: CaptureRefusalReason, denial: Option<CaptureDenialReason>, ) -> Self",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl ViolationRisk :: const fn raised( chunk_seq: u32, chunk_at: u64, denial: CaptureDenialReason, status: CaptureStatus, ) -> Self",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Clone, PartialEq, Eq)] struct ReleasableArtifact",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq)] struct TimelineGap",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq)] struct ViolationRisk",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum CaptureArtifact",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct CaptureManifest",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct ChunkRecord",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct QuarantinedArtifact",
    "crates/capture-gate/src/artifact.rs [pub] const PERMISSION_VIOLATION_RISK: &str",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureArtifact :: #[must_use] const fn as_quarantined(&self) -> Option<&QuarantinedArtifact>",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureArtifact :: #[must_use] const fn as_releasable(&self) -> Option<&ReleasableArtifact>",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureArtifact :: #[must_use] const fn is_quarantined(&self) -> bool",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureArtifact :: #[must_use] const fn manifest(&self) -> &CaptureManifest",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureManifest :: #[must_use] const fn byte_len(&self) -> usize",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureManifest :: #[must_use] const fn digest(&self) -> &ContentDigest",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureManifest :: #[must_use] const fn gap(&self) -> Option<TimelineGap>",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureManifest :: #[must_use] const fn retention(&self) -> RetentionTerms",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureManifest :: #[must_use] fn chunks(&self) -> &[ChunkRecord]",
    "crates/capture-gate/src/artifact.rs [pub] impl ChunkRecord :: #[must_use] const fn byte_len(&self) -> usize",
    "crates/capture-gate/src/artifact.rs [pub] impl ChunkRecord :: #[must_use] const fn digest(&self) -> &ContentDigest",
    "crates/capture-gate/src/artifact.rs [pub] impl ChunkRecord :: #[must_use] const fn seq(&self) -> u32",
    "crates/capture-gate/src/artifact.rs [pub] impl ChunkRecord :: #[must_use] const fn started_at(&self) -> u64",
    "crates/capture-gate/src/artifact.rs [pub] impl QuarantinedArtifact :: #[must_use] const fn manifest(&self) -> &CaptureManifest",
    "crates/capture-gate/src/artifact.rs [pub] impl QuarantinedArtifact :: #[must_use] const fn risk(&self) -> ViolationRisk",
    "crates/capture-gate/src/artifact.rs [pub] impl QuarantinedArtifact :: #[must_use] const fn state(&self) -> &'static str",
    "crates/capture-gate/src/artifact.rs [pub] impl ReleasableArtifact :: #[must_use] const fn manifest(&self) -> &CaptureManifest",
    "crates/capture-gate/src/artifact.rs [pub] impl ReleasableArtifact :: #[must_use] fn bytes(&self) -> &[u8]",
    "crates/capture-gate/src/artifact.rs [pub] impl TimelineGap :: #[must_use] const fn cause(&self) -> CaptureRefusalReason",
    "crates/capture-gate/src/artifact.rs [pub] impl TimelineGap :: #[must_use] const fn denial(&self) -> Option<CaptureDenialReason>",
    "crates/capture-gate/src/artifact.rs [pub] impl TimelineGap :: #[must_use] const fn from(&self) -> u64",
    "crates/capture-gate/src/artifact.rs [pub] impl ViolationRisk :: #[must_use] const fn chunk_at(&self) -> u64",
    "crates/capture-gate/src/artifact.rs [pub] impl ViolationRisk :: #[must_use] const fn chunk_seq(&self) -> u32",
    "crates/capture-gate/src/artifact.rs [pub] impl ViolationRisk :: #[must_use] const fn denial(&self) -> CaptureDenialReason",
    "crates/capture-gate/src/artifact.rs [pub] impl ViolationRisk :: #[must_use] const fn state(&self) -> &'static str",
    "crates/capture-gate/src/artifact.rs [pub] impl ViolationRisk :: #[must_use] const fn status(&self) -> CaptureStatus",
    "crates/capture-gate/src/audit.rs [priv] impl CaptureAudit",
    "crates/capture-gate/src/audit.rs [priv] impl CaptureAuditRow",
    "crates/capture-gate/src/audit.rs [priv] impl CaptureRefusal",
    "crates/capture-gate/src/audit.rs [priv] impl CaptureRefusalReason",
    "crates/capture-gate/src/audit.rs [priv] use academic_consent::{CaptureDenial, CaptureDenialReason, CaptureStatus}",
    "crates/capture-gate/src/audit.rs [priv] use academic_domain::{ContentDigest, LectureSessionId, OfferingId}",
    "crates/capture-gate/src/audit.rs [priv] use crate::device::DeviceClass",
    "crates/capture-gate/src/audit.rs [pub(crate)] #[derive(Debug, Clone, Copy, Default)] struct AuditSubject",
    "crates/capture-gate/src/audit.rs [pub(crate)] impl CaptureAudit :: fn record_refusal( &mut self, refusal: CaptureRefusal, subject: AuditSubject, now: u64, ) -> CaptureRefusal",
    "crates/capture-gate/src/audit.rs [pub(crate)] impl CaptureRefusal :: const fn from_denial(denial: CaptureDenial, class: Option<DeviceClass>) -> Self",
    "crates/capture-gate/src/audit.rs [pub(crate)] impl CaptureRefusal :: const fn of(reason: CaptureRefusalReason, class: Option<DeviceClass>) -> Self",
    "crates/capture-gate/src/audit.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] #[non_exhaustive] enum CaptureRefusalReason",
    "crates/capture-gate/src/audit.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)] #[error(\"capture refused at the device layer: {reason:?}\")] struct CaptureRefusal",
    "crates/capture-gate/src/audit.rs [pub] #[derive(Debug, Clone, Default)] struct CaptureAudit",
    "crates/capture-gate/src/audit.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct CaptureAuditRow",
    "crates/capture-gate/src/audit.rs [pub] const REFUSAL_REASONS: [CaptureRefusalReason; 6]",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAudit :: #[must_use] const fn new() -> Self",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAudit :: #[must_use] fn count_of(&self, reason: CaptureRefusalReason) -> usize",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAudit :: #[must_use] fn rows(&self) -> &[CaptureAuditRow]",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAuditRow :: #[must_use] const fn class(&self) -> Option<DeviceClass>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAuditRow :: #[must_use] const fn denial_reason(&self) -> Option<CaptureDenialReason>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAuditRow :: #[must_use] const fn lecture_id(&self) -> Option<LectureSessionId>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAuditRow :: #[must_use] const fn offering_id(&self) -> Option<OfferingId>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAuditRow :: #[must_use] const fn reason(&self) -> CaptureRefusalReason",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAuditRow :: #[must_use] const fn recorded_at(&self) -> u64",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAuditRow :: #[must_use] const fn status(&self) -> Option<CaptureStatus>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureAuditRow :: #[must_use] const fn subject_digest(&self) -> Option<&ContentDigest>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureRefusal :: #[must_use] const fn class(&self) -> Option<DeviceClass>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureRefusal :: #[must_use] const fn denial(&self) -> Option<CaptureDenial>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureRefusal :: #[must_use] const fn reason(&self) -> CaptureRefusalReason",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureRefusal :: #[must_use] fn denial_reason(&self) -> Option<CaptureDenialReason>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureRefusal :: #[must_use] fn status(&self) -> Option<CaptureStatus>",
    "crates/capture-gate/src/audit.rs [pub] impl CaptureRefusalReason :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/capture-gate/src/daemon.rs [priv] impl CaptureAuthorization",
    "crates/capture-gate/src/daemon.rs [priv] use academic_consent::{CaptureRequest, ConsentLedger, mint_capture_capability}",
    "crates/capture-gate/src/daemon.rs [priv] use crate::{ audit::{AuditSubject, CaptureAudit, CaptureRefusal}, device::DeviceRuleset, }",
    "crates/capture-gate/src/daemon.rs [pub(crate)] impl CaptureAuthorization :: fn into_token(self) -> academic_consent::CaptureCapabilityToken",
    "crates/capture-gate/src/daemon.rs [pub] #[derive(Debug)] struct CaptureAuthorization",
    "crates/capture-gate/src/daemon.rs [pub] fn authorize( ledger: &mut ConsentLedger, audit: &mut CaptureAudit, request: &CaptureRequest, now: u64, ) -> Result<CaptureAuthorization, CaptureRefusal>",
    "crates/capture-gate/src/daemon.rs [pub] impl CaptureAuthorization :: #[must_use] const fn ruleset(&self) -> &DeviceRuleset",
    "crates/capture-gate/src/daemon.rs [pub] impl CaptureAuthorization :: #[must_use] const fn token(&self) -> &academic_consent::CaptureCapabilityToken",
    "crates/capture-gate/src/device.rs [priv] impl BackendId",
    "crates/capture-gate/src/device.rs [priv] impl DeviceClass",
    "crates/capture-gate/src/device.rs [priv] impl DeviceLayer",
    "crates/capture-gate/src/device.rs [priv] impl DeviceRuleset",
    "crates/capture-gate/src/device.rs [priv] use academic_consent::{CaptureCapabilityToken, CaptureMedium}",
    "crates/capture-gate/src/device.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq)] enum DeviceLayer",
    "crates/capture-gate/src/device.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] #[non_exhaustive] enum BackendId",
    "crates/capture-gate/src/device.rs [pub] #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] #[non_exhaustive] enum DeviceClass",
    "crates/capture-gate/src/device.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct DeviceRuleset",
    "crates/capture-gate/src/device.rs [pub] const DEVICE_CLASSES: [DeviceClass; 3]",
    "crates/capture-gate/src/device.rs [pub] impl BackendId :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/capture-gate/src/device.rs [pub] impl DeviceClass :: #[must_use] const fn as_str(self) -> &'static str",
    "crates/capture-gate/src/device.rs [pub] impl DeviceClass :: #[must_use] const fn of(medium: CaptureMedium) -> Option<Self>",
    "crates/capture-gate/src/device.rs [pub] impl DeviceLayer :: #[must_use] const fn backend(self) -> BackendId",
    "crates/capture-gate/src/device.rs [pub] impl DeviceLayer :: #[must_use] const fn is_enforced(self) -> bool",
    "crates/capture-gate/src/device.rs [pub] impl DeviceRuleset :: #[must_use] fn classes(&self) -> &[DeviceClass]",
    "crates/capture-gate/src/device.rs [pub] impl DeviceRuleset :: #[must_use] fn for_token(token: &CaptureCapabilityToken) -> Self",
    "crates/capture-gate/src/device.rs [pub] impl DeviceRuleset :: #[must_use] fn is_empty(&self) -> bool",
    "crates/capture-gate/src/device.rs [pub] impl DeviceRuleset :: #[must_use] fn permits(&self, class: DeviceClass) -> bool",
    "crates/capture-gate/src/device.rs [pub] impl DeviceRuleset :: #[must_use] fn unclassified(&self) -> &[CaptureMedium]",
    "crates/capture-gate/src/lib.rs [pub] mod artifact",
    "crates/capture-gate/src/lib.rs [pub] mod audit",
    "crates/capture-gate/src/lib.rs [pub] mod daemon",
    "crates/capture-gate/src/lib.rs [pub] mod device",
    "crates/capture-gate/src/lib.rs [pub] mod native",
    "crates/capture-gate/src/lib.rs [pub] mod session",
    "crates/capture-gate/src/lib.rs [pub] use artifact::{ CaptureArtifact, CaptureManifest, ChunkRecord, PERMISSION_VIOLATION_RISK, QuarantinedArtifact, ReleasableArtifact, TimelineGap, ViolationRisk, }",
    "crates/capture-gate/src/lib.rs [pub] use audit::{ CaptureAudit, CaptureAuditRow, CaptureRefusal, CaptureRefusalReason, REFUSAL_REASONS, }",
    "crates/capture-gate/src/lib.rs [pub] use daemon::{CaptureAuthorization, authorize}",
    "crates/capture-gate/src/lib.rs [pub] use device::{BackendId, DEVICE_CLASSES, DeviceClass, DeviceLayer, DeviceRuleset}",
    "crates/capture-gate/src/lib.rs [pub] use session::{CaptureSession, open_device, releasable_bytes}",
    "crates/capture-gate/src/native/linux.rs [priv] #[allow(unsafe_code)] fn enter(rules: &[RuleFd], handled: u64) -> Result<(), NativeError>",
    "crates/capture-gate/src/native/linux.rs [priv] #[allow(unsafe_code)] fn landlock_abi() -> i64",
    "crates/capture-gate/src/native/linux.rs [priv] #[allow(unsafe_code)] fn resolve(path: &Path, abi: i64, writable: bool, executable: bool) -> Result<RuleFd, NativeError>",
    "crates/capture-gate/src/native/linux.rs [priv] #[derive(Debug, Clone, Copy)] struct RuleFd",
    "crates/capture-gate/src/native/linux.rs [priv] #[repr(C)] #[derive(Debug, Default)] struct LandlockRulesetAttr",
    "crates/capture-gate/src/native/linux.rs [priv] #[repr(C, packed)] #[derive(Debug, Clone, Copy)] struct LandlockPathBeneathAttr",
    "crates/capture-gate/src/native/linux.rs [priv] const ABI1_HANDLED: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const ACCESS_EXECUTE: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const ACCESS_IOCTL_DEV: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const ACCESS_MAKE_REG: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const ACCESS_READ_DIR: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const ACCESS_READ_FILE: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const ACCESS_REFER: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const ACCESS_TRUNCATE: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const ACCESS_WRITE_FILE: u64",
    "crates/capture-gate/src/native/linux.rs [priv] const LANDLOCK_CREATE_RULESET_VERSION: u32",
    "crates/capture-gate/src/native/linux.rs [priv] const LANDLOCK_RULE_PATH_BENEATH: u32",
    "crates/capture-gate/src/native/linux.rs [priv] const RUNTIME_IMAGE_DIRECTORIES: [&str; 4]",
    "crates/capture-gate/src/native/linux.rs [priv] fn errno() -> i64",
    "crates/capture-gate/src/native/linux.rs [priv] fn handled_mask(abi: i64) -> u64",
    "crates/capture-gate/src/native/linux.rs [priv] use crate::device::{BackendId, DeviceLayer}",
    "crates/capture-gate/src/native/linux.rs [priv] use std::{ ffi::CString, io, os::unix::{ffi::OsStrExt as _, process::CommandExt as _}, path::Path, process::Command, }",
    "crates/capture-gate/src/native/linux.rs [priv] use super::{LaunchSpec, NativeError, REPORT_DIR_VAR, REPORT_FILE}",
    "crates/capture-gate/src/native/linux.rs [pub(super)] #[allow(unsafe_code)] fn launch(spec: &LaunchSpec) -> Result<String, NativeError>",
    "crates/capture-gate/src/native/linux.rs [pub(super)] fn availability() -> DeviceLayer",
    "crates/capture-gate/src/native/mod.rs [priv] #[cfg(all(feature = \"native-capture\", target_os = \"linux\"))] mod linux",
    "crates/capture-gate/src/native/mod.rs [priv] #[cfg(all(feature = \"native-capture\", target_os = \"windows\"))] mod windows",
    "crates/capture-gate/src/native/mod.rs [priv] impl DeviceTree",
    "crates/capture-gate/src/native/mod.rs [priv] impl LaunchSpec",
    "crates/capture-gate/src/native/mod.rs [priv] use crate::device::{DeviceClass, DeviceLayer, DeviceRuleset}",
    "crates/capture-gate/src/native/mod.rs [priv] use std::path::PathBuf",
    "crates/capture-gate/src/native/mod.rs [pub] #[derive(Debug, Clone)] struct LaunchSpec",
    "crates/capture-gate/src/native/mod.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct DeviceTree",
    "crates/capture-gate/src/native/mod.rs [pub] #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] enum NativeError",
    "crates/capture-gate/src/native/mod.rs [pub] #[must_use] fn availability() -> DeviceLayer",
    "crates/capture-gate/src/native/mod.rs [pub] #[must_use] fn device_paths(class: DeviceClass) -> Vec<String>",
    "crates/capture-gate/src/native/mod.rs [pub] const REPORT_DIR_VAR: &str",
    "crates/capture-gate/src/native/mod.rs [pub] const REPORT_FILE: &str",
    "crates/capture-gate/src/native/mod.rs [pub] fn launch(spec: &LaunchSpec) -> Result<String, NativeError>",
    "crates/capture-gate/src/native/mod.rs [pub] impl DeviceTree :: #[must_use] const fn class(&self) -> DeviceClass",
    "crates/capture-gate/src/native/mod.rs [pub] impl DeviceTree :: #[must_use] const fn new(class: DeviceClass, path: PathBuf) -> Self",
    "crates/capture-gate/src/native/mod.rs [pub] impl DeviceTree :: #[must_use] fn path(&self) -> &std::path::Path",
    "crates/capture-gate/src/native/mod.rs [pub] impl LaunchSpec :: #[must_use] fn permits(&self, class: DeviceClass) -> bool",
    "crates/capture-gate/src/native/windows.rs [priv] #[allow(unsafe_code)] fn container_sid() -> Result<PSID, NativeError>",
    "crates/capture-gate/src/native/windows.rs [priv] #[allow(unsafe_code)] fn grant(path: &Path, sid: PSID, rights: u32, inherit: u32) -> Result<(), NativeError>",
    "crates/capture-gate/src/native/windows.rs [priv] #[allow(unsafe_code)] fn last_error() -> i64",
    "crates/capture-gate/src/native/windows.rs [priv] #[derive(Debug)] struct ProfileLock",
    "crates/capture-gate/src/native/windows.rs [priv] const CONTAINER_NAME: &str",
    "crates/capture-gate/src/native/windows.rs [priv] const INHERIT_ALL: u32",
    "crates/capture-gate/src/native/windows.rs [priv] const KSCATEGORY_CAPTURE: GUID",
    "crates/capture-gate/src/native/windows.rs [priv] const KSCATEGORY_VIDEO_CAMERA: GUID",
    "crates/capture-gate/src/native/windows.rs [priv] const PROFILE_LOCK_FILE: &str",
    "crates/capture-gate/src/native/windows.rs [priv] const PROFILE_LOCK_WAIT: Duration",
    "crates/capture-gate/src/native/windows.rs [priv] const RIGHTS_READ_EXECUTE: u32",
    "crates/capture-gate/src/native/windows.rs [priv] const RIGHTS_READ_WRITE: u32",
    "crates/capture-gate/src/native/windows.rs [priv] const SHARING_VIOLATION: i32",
    "crates/capture-gate/src/native/windows.rs [priv] fn environment_block(report_dir: &Path) -> Vec<u16>",
    "crates/capture-gate/src/native/windows.rs [priv] fn profile_lock_path() -> Option<PathBuf>",
    "crates/capture-gate/src/native/windows.rs [priv] fn read_report(report_dir: &Path) -> Result<String, NativeError>",
    "crates/capture-gate/src/native/windows.rs [priv] fn wide(value: &str) -> Vec<u16>",
    "crates/capture-gate/src/native/windows.rs [priv] impl Drop for OwnedHandle",
    "crates/capture-gate/src/native/windows.rs [priv] impl Drop for OwnedHandle :: #[allow(unsafe_code)] fn drop(&mut self)",
    "crates/capture-gate/src/native/windows.rs [priv] impl ProfileLock",
    "crates/capture-gate/src/native/windows.rs [priv] impl ProfileLock :: fn acquire() -> Self",
    "crates/capture-gate/src/native/windows.rs [priv] static PROFILE_CREATION: Mutex<()>",
    "crates/capture-gate/src/native/windows.rs [priv] struct OwnedHandle(HANDLE)",
    "crates/capture-gate/src/native/windows.rs [priv] use crate::device::{BackendId, DeviceClass, DeviceLayer}",
    "crates/capture-gate/src/native/windows.rs [priv] use std::{ ffi::c_void, fs::OpenOptions, mem::{size_of, zeroed}, os::windows::fs::OpenOptionsExt as _, path::{Path, PathBuf}, ptr::{null, null_mut}, sync::{Mutex, PoisonError}, time::{Duration, Instant}, }",
    "crates/capture-gate/src/native/windows.rs [priv] use super::{LaunchSpec, NativeError, REPORT_DIR_VAR, REPORT_FILE}",
    "crates/capture-gate/src/native/windows.rs [priv] use windows_sys::{ Win32::{ Devices::DeviceAndDriverInstallation::{ CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW, CM_Get_Device_Interface_ListW, }, Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_TIMEOUT}, Security::{ ACL, Authorization::{ EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, SE_FILE_OBJECT, SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_WELL_KNOWN_GROUP, }, DACL_SECURITY_INFORMATION, Isolation::{CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName}, PSID, SECURITY_CAPABILITIES, }, System::Threading::{ CREATE_SUSPENDED, CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, ResumeThread, STARTUPINFOEXW, UpdateProcThreadAttribute, WaitForSingleObject, }, }, core::GUID, }",
    "crates/capture-gate/src/native/windows.rs [pub(super)] #[allow(dead_code)] fn enter() -> Result<BackendId, NativeError>",
    "crates/capture-gate/src/native/windows.rs [pub(super)] #[allow(unsafe_code)] fn device_interface_paths(class: DeviceClass) -> Vec<String>",
    "crates/capture-gate/src/native/windows.rs [pub(super)] #[allow(unsafe_code)] fn launch(spec: &LaunchSpec) -> Result<String, NativeError>",
    "crates/capture-gate/src/native/windows.rs [pub(super)] fn availability() -> DeviceLayer",
    "crates/capture-gate/src/session.rs [priv] impl CaptureSession",
    "crates/capture-gate/src/session.rs [priv] impl CaptureSession :: fn first_unbound_chunk( &self, ledger: &ConsentLedger, ) -> Option<(ViolationRisk, CaptureDenial)>",
    "crates/capture-gate/src/session.rs [priv] impl CaptureSession :: fn subject(&self) -> AuditSubject",
    "crates/capture-gate/src/session.rs [priv] impl fmt::Debug for CaptureSession",
    "crates/capture-gate/src/session.rs [priv] impl fmt::Debug for CaptureSession :: fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/capture-gate/src/session.rs [priv] use academic_consent::{ CaptureCapabilityToken, CaptureDenial, ConsentLedger, RetentionTerms, bind_permission, continue_capture, }",
    "crates/capture-gate/src/session.rs [priv] use academic_domain::{ContentDigest, LectureSessionId, OfferingId}",
    "crates/capture-gate/src/session.rs [priv] use crate::{ artifact::{CaptureArtifact, ChunkRecord, TimelineGap, ViolationRisk}, audit::{AuditSubject, CaptureAudit, CaptureRefusal, CaptureRefusalReason}, daemon::CaptureAuthorization, device::{DeviceClass, DeviceLayer}, }",
    "crates/capture-gate/src/session.rs [priv] use std::fmt",
    "crates/capture-gate/src/session.rs [pub] fn open_device( ledger: &mut ConsentLedger, audit: &mut CaptureAudit, authorization: CaptureAuthorization, class: DeviceClass, layer: DeviceLayer, now: u64, ) -> Result<CaptureSession, CaptureRefusal>",
    "crates/capture-gate/src/session.rs [pub] fn releasable_bytes<'artifact>( artifact: &'artifact CaptureArtifact, audit: &mut CaptureAudit, now: u64, ) -> Result<&'artifact [u8], CaptureRefusal>",
    "crates/capture-gate/src/session.rs [pub] impl CaptureSession :: #[must_use] const fn class(&self) -> DeviceClass",
    "crates/capture-gate/src/session.rs [pub] impl CaptureSession :: #[must_use] const fn gap(&self) -> Option<TimelineGap>",
    "crates/capture-gate/src/session.rs [pub] impl CaptureSession :: #[must_use] const fn layer(&self) -> DeviceLayer",
    "crates/capture-gate/src/session.rs [pub] impl CaptureSession :: #[must_use] const fn not_after(&self) -> u64",
    "crates/capture-gate/src/session.rs [pub] impl CaptureSession :: #[must_use] const fn token_id(&self) -> &ContentDigest",
    "crates/capture-gate/src/session.rs [pub] impl CaptureSession :: #[must_use] fn chunk_count(&self) -> usize",
    "crates/capture-gate/src/session.rs [pub] impl CaptureSession :: fn record_chunk( &mut self, ledger: &mut ConsentLedger, audit: &mut CaptureAudit, bytes: &[u8], now: u64, ) -> Result<(), CaptureRefusal>",
    "crates/capture-gate/src/session.rs [pub] impl CaptureSession :: fn seal( self, ledger: &ConsentLedger, audit: &mut CaptureAudit, now: u64, ) -> CaptureArtifact",
    "crates/capture-gate/src/session.rs [pub] struct CaptureSession",
];

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
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct Redaction",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct RestrictedOriginal",
    "crates/student-voice/src/derivative.rs [pub] fn redact( plan: &RedactionPlan, reference: &RedactionPolicyRef, source: &LectureSource<'_>, requested: RetentionTerms, ) -> Result<Redaction, RedactionFault>",
    "crates/student-voice/src/derivative.rs [pub] impl RawAccessGrant :: fn issued( original: &RestrictedOriginal, requested_by: Actor, purpose: &str, at: u64, ) -> Result<Self, AccessRefusal>",
    "crates/student-voice/src/derivative.rs [pub] impl Redaction :: #[must_use] const fn original(&self) -> &RestrictedOriginal",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn classification(&self) -> &'static str",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn digest(&self) -> &ContentDigest",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn lecture(&self) -> LectureSessionId",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn source_version(&self) -> u32",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] const fn terms(&self) -> RetentionTerms",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: #[must_use] fn removed_count(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: fn open( &self, grant: RawAccessGrant, log: &mut RawAccessLog, ) -> Result<DisclosedOriginal<'_>, AccessRefusal>",
    "crates/student-voice/src/lib.rs [pub] use derivative::{ DerivedArtifact, DisclosedOriginal, ExclusionRecord, KeptUtterance, LectureSource, ManualExclusion, ORIGINAL_CLASSIFICATION, RawAccessGrant, RawAccessLog, RawAccessRecord, RedactedDerivative, Redaction, RedactionMode, RedactionPlan, RestrictedOriginal, SourceUtterance, inherit_terms, redact, }",
];

/// Every item that reaches a `DisclosedOriginal`.
///
/// Ten. `open` is where one comes from; the rest are the reads a holder may
/// make, each by position rather than in bulk, and the re-export. There is no
/// owned form and no route back into a derivative.
const DISCLOSED_ORIGINAL_ITEMS: [&str; 10] = [
    "crates/student-voice/src/derivative.rs [priv] impl DisclosedOriginal<'_>",
    "crates/student-voice/src/derivative.rs [priv] impl RestrictedOriginal",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, PartialEq, Eq)] struct DisclosedOriginal<'a>",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] const fn is_empty(&self) -> bool",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] const fn len(&self) -> usize",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn source_index(&self, position: usize) -> Option<usize>",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn speaker(&self, position: usize) -> Option<Speaker>",
    "crates/student-voice/src/derivative.rs [pub] impl DisclosedOriginal<'_> :: #[must_use] fn verbatim(&self, position: usize) -> Option<&str>",
    "crates/student-voice/src/derivative.rs [pub] impl RestrictedOriginal :: fn open( &self, grant: RawAccessGrant, log: &mut RawAccessLog, ) -> Result<DisclosedOriginal<'_>, AccessRefusal>",
    "crates/student-voice/src/lib.rs [pub] use derivative::{ DerivedArtifact, DisclosedOriginal, ExclusionRecord, KeptUtterance, LectureSource, ManualExclusion, ORIGINAL_CLASSIFICATION, RawAccessGrant, RawAccessLog, RawAccessRecord, RedactedDerivative, Redaction, RedactionMode, RedactionPlan, RestrictedOriginal, SourceUtterance, inherit_terms, redact, }",
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
    "crates/student-voice/src/derivative.rs [priv] use crate::{ fault::{AccessRefusal, RedactionFault}, measure::AccuracyWitness, policy::RedactionPolicy, }",
    "crates/student-voice/src/derivative.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum RedactionMode",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionMode :: #[must_use] const fn witness(&self) -> Option<&AccuracyWitness>",
    "crates/student-voice/src/derivative.rs [pub] impl RedactionPlan :: #[must_use] fn automatic(policy: RedactionPolicy, witness: AccuracyWitness) -> Self",
    "crates/student-voice/src/lib.rs [pub] use measure::{ ABSOLUTE_ACCURACY_FLOOR, ABSOLUTE_MISSED_STUDENT_CEILING, AccuracyWitness, CaseMeasurement, DIARIZATION_THRESHOLD_V1, DiarizationMeasurement, DiarizationThreshold, SCORER_VERSION, measure, measure_case, }",
    "crates/student-voice/src/measure.rs [priv] impl AccuracyWitness",
    "crates/student-voice/src/measure.rs [priv] impl DiarizationMeasurement",
    "crates/student-voice/src/measure.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] struct AccuracyWitness",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn accuracy_permille(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn corpus_digest(&self) -> &ContentDigest",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn corpus_version(&self) -> u32",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn missed_student_permille(&self) -> u64",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn scorer_version(&self) -> u32",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] const fn threshold(&self) -> DiarizationThreshold",
    "crates/student-voice/src/measure.rs [pub] impl AccuracyWitness :: #[must_use] fn corpus_id(&self) -> &str",
    "crates/student-voice/src/measure.rs [pub] impl DiarizationMeasurement :: fn witness( &self, threshold: DiarizationThreshold, ) -> Result<AccuracyWitness, AccuracyRefusal>",
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
    "crates/capture-gate/src/artifact.rs [priv] impl fmt::Debug for ReleasableArtifact :: fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "crates/capture-gate/src/artifact.rs [pub(crate)] impl CaptureArtifact :: const fn releasable(manifest: CaptureManifest, bytes: Vec<u8>) -> Self",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Clone, PartialEq, Eq)] struct ReleasableArtifact",
    "crates/capture-gate/src/artifact.rs [pub] #[derive(Debug, Clone, PartialEq, Eq)] enum CaptureArtifact",
    "crates/capture-gate/src/artifact.rs [pub] impl CaptureArtifact :: #[must_use] const fn as_releasable(&self) -> Option<&ReleasableArtifact>",
    "crates/capture-gate/src/artifact.rs [pub] impl ReleasableArtifact :: #[must_use] const fn manifest(&self) -> &CaptureManifest",
    "crates/capture-gate/src/artifact.rs [pub] impl ReleasableArtifact :: #[must_use] fn bytes(&self) -> &[u8]",
    "crates/capture-gate/src/lib.rs [pub] use artifact::{ CaptureArtifact, CaptureManifest, ChunkRecord, PERMISSION_VIOLATION_RISK, QuarantinedArtifact, ReleasableArtifact, TimelineGap, ViolationRisk, }",
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
