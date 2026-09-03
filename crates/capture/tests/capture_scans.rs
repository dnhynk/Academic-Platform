//! Source scans for the `P2-L2` capture subsystem.
//!
//! Five of this crate's claims are shapes of the source rather than
//! behaviours, so nothing at run time would notice the day they stopped being
//! true: that every instant comes from one clock, that no wall clock is read,
//! that no path moves a mark, that the journal only ever extends, and that the
//! four thresholds are rows rather than constants.
//! `docs/contracts/policy-source-scans.md` is the page those scans are
//! enumerated on, and this file is written against all five of the empty-scan
//! shapes it names.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends the whole
//! package, not `src` by name, with a floor, a `mod`/`#[path]` tripwire, and a
//! rule that this crate's product source is under `src` and nowhere else.
//!
//! **The checks are not token lists.** The ones that could have been are whole
//! sets: the construction sites of [`academic_capture::SessionTick`], the
//! `impl` blocks naming `Mark`, the public `&mut self` methods on
//! `ChunkJournal`, and every closed vocabulary read out of its own enum. A key
//! nobody predicted fails as an extra entry.
//!
//! **The pins fix their callers.** Eight whole-text pins hold the eight seams
//! that mint an instant, each beside the count of `self.clock.tick` calls it
//! makes, and their sum is compared with the whole file's, because `T141` found
//! a pinned check skipped by a condition wrapped around it.
//!
//! **Every inventory counts an identifier, not a spelling.** `P2-RF10` reached
//! a fourth exposure site in another crate by writing `Untrusted::expose(d)`
//! instead of `d.expose()`. The counts here are whole-identifier counts with
//! declarations subtracted.
//!
//! **Each check is run against evasions inside the test.** A guard that has
//! never refused anything is a guard nobody has tested, so every rule below is
//! applied to samples written to slip past it and each one must be caught.

mod common;

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_capture::{
    FAULT_SELECTORS, FailureKind, GapCause, MarkLabelKind, Orientation, SignalDelivery,
};

type TestResult = Result<(), Box<dyn Error>>;

/// The crate root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The workspace root.
fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Every `.rs` file anywhere under this crate's package.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships: everything outside `tests`.
fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    Ok(crate_all_sources()?
        .into_iter()
        .filter(|path| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            !relative.starts_with("tests")
        })
        .collect())
}

/// Every `.rs` file under every workspace package, less each package's `tests`.
fn workspace_product_sources() -> Result<Vec<(String, PathBuf)>, Box<dyn Error>> {
    let crates = workspace_root().join("crates");
    let mut found = Vec::new();
    for entry in fs::read_dir(&crates)? {
        let package = entry?.path();
        if !package.is_dir() {
            continue;
        }
        let name = package
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        let mut inside = Vec::new();
        walk(&package, &mut inside)?;
        for path in inside {
            let relative = path.strip_prefix(&package).unwrap_or(&path).to_path_buf();
            if relative.starts_with("tests") {
                continue;
            }
            found.push((name.clone(), path));
        }
    }
    found.sort();
    Ok(found)
}

fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/capture-gate/tests/capture_scans.rs`, raw strings and
/// nested block comments included. `P2-G4` found that a lexer without raw
/// strings desynchronizes and reads every literal after one as code.
fn strip_non_code(source: &str) -> String {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();

        if current == '/' && next == Some('/') {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            out.push('\n');
            continue;
        }
        if current == '/' && next == Some('*') {
            let mut depth = 1_usize;
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index] == '/' && bytes.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if bytes[index] == '*' && bytes.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            out.push(' ');
            continue;
        }
        if current == 'r' && matches!(next, Some('"') | Some('#')) {
            let mut probe = index + 1;
            let mut hashes = 0_usize;
            while bytes.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if bytes.get(probe) == Some(&'"') {
                let terminator: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = bytes[probe + 1..].iter().collect();
                let end = rest.find(&terminator).map_or(bytes.len(), |at| {
                    probe + 1 + rest[..at].chars().count() + terminator.chars().count()
                });
                index = end;
                out.push(' ');
                continue;
            }
        }
        if current == '"' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == '\\' {
                    index += 2;
                    continue;
                }
                if bytes[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            out.push(' ');
            continue;
        }
        if current == '\'' {
            let closes_two_on = bytes.get(index + 2) == Some(&'\'');
            let closes_three_on = bytes.get(index + 3) == Some(&'\'');
            if closes_two_on || (bytes.get(index + 1) == Some(&'\\') && closes_three_on) {
                index += if closes_two_on { 3 } else { 4 };
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

fn code_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(path)?))
}

fn relative(path: &Path) -> String {
    path.strip_prefix(crate_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Extracts one method's text, comment lines dropped and whitespace collapsed.
///
/// It closes on a brace at the method's own indentation, so a pin over a method
/// inside an `impl` block ends where the method does rather than where the
/// block does.
fn declared_method(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let indent: String = source[..start]
        .chars()
        .rev()
        .take_while(|character| *character == ' ')
        .collect();
    let closing = format!("\n{indent}}}");
    let end = source[start..]
        .find(&closing)
        .ok_or_else(|| format!("{signature} has no closing brace at its own indentation"))?;
    let body = &source[start..start + end + closing.len()];
    let kept: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    Ok(kept
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

/// How many times `needle` appears in `code`.
fn occurrences(code: &str, needle: &str) -> usize {
    code.split(needle).count().saturating_sub(1)
}

/// Counts whole-identifier occurrences of `name` in already-stripped code.
fn uses_of(code: &str, name: &str) -> usize {
    let bytes = code.as_bytes();
    code.match_indices(name)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + name.len()).copied().unwrap_or(b' ');
            before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
        })
        .count()
}

/// How many times `name` is called in `code`, declarations subtracted.
fn calls_of(code: &str, name: &str) -> usize {
    uses_of(code, name).saturating_sub(occurrences(code, &format!("fn {name}(")))
}

/// Every `pub` function signature in `code`, whitespace-collapsed.
fn public_signatures(code: &str) -> Vec<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if ![
            "pub fn ",
            "pub const fn ",
            "pub async fn ",
            "pub unsafe fn ",
        ]
        .iter()
        .any(|start| trimmed.starts_with(start))
        {
            continue;
        }
        let mut signature = String::new();
        for follow in lines.iter().skip(index) {
            signature.push(' ');
            signature.push_str(follow.trim());
            if follow.contains('{') || follow.trim_end().ends_with(';') {
                break;
            }
        }
        found.push(signature.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    found
}

/// Splits a signature into its parameter list and its return type.
fn parameters_and_return(signature: &str) -> Option<(&str, &str)> {
    let open = signature.find('(')?;
    let mut depth = 0_usize;
    for (offset, character) in signature.get(open..)?.char_indices() {
        let at = open.saturating_add(offset);
        match character {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let (parameters, rest) = signature.split_at(at.saturating_add(1));
                    let returns = rest.split_once("->").map_or("", |(_, tail)| tail);
                    return Some((parameters, returns));
                }
            }
            _ => (),
        }
    }
    None
}

/// The variant names an enum declares, in declaration order.
fn enum_variants(source: &str, header: &str) -> Vec<String> {
    source
        .lines()
        .skip_while(|line| !line.contains(header))
        .skip(1)
        .take_while(|line| !line.starts_with('}'))
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("///")
                && !line.starts_with("//")
                && !line.starts_with("#[")
        })
        .filter_map(|line| {
            let name: String = line
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// How many struct literals of `name` appear in `code`.
///
/// A return type, an `impl` header and a type declaration all spell the same
/// characters as a literal, so each is subtracted. `T141`'s lesson applies here
/// in miniature: the spelling is not the structure.
fn literals_of(code: &str, name: &str) -> usize {
    let declarations: usize = ["-> ", "impl ", "struct ", "enum ", "union "]
        .iter()
        .map(|prefix| occurrences(code, &format!("{prefix}{name} {{")))
        .sum();
    occurrences(code, &format!("{name} {{")).saturating_sub(declarations)
}

/// Every `impl` block header in `code` that names `needle` as an identifier.
fn impl_headers_naming(code: &str, needle: &str) -> Vec<String> {
    code.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("impl"))
        .filter(|line| uses_of(line, needle) > 0)
        .map(|line| line.trim_end_matches('{').trim().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let all = crate_all_sources()?;
    let product = crate_product_sources()?;
    assert!(
        all.len() >= 13,
        "the crate walk read only {} files",
        all.len()
    );
    assert!(
        product.len() >= 9,
        "the product walk read only {} files",
        product.len()
    );

    // Product source sits under `src` and nowhere else. This crate declares no
    // binary target, so unlike `academic-capture-gate` there is no `probes`
    // tree, and a file that appeared outside `src` would be product-shaped code
    // that `cargo clippy --all-targets` compiles with no feature gate.
    let root = crate_root();
    for path in &product {
        let relative = path.strip_prefix(&root).unwrap_or(path);
        assert!(
            relative.starts_with("src"),
            "{} is product source outside src",
            relative.display()
        );
    }

    // The tripwire: every module this crate declares must be a file the walk
    // read, and `#[path]` may not appear at all.
    let read: BTreeSet<String> = all.iter().map(|path| relative(path)).collect();
    let mut declared = 0_usize;
    for path in &all {
        let code = code_of(path)?;
        assert_eq!(
            occurrences(&code, "#[path"),
            0,
            "{}: #[path] moves a module out of the walk's reach",
            relative(path)
        );
        for line in code.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))
            else {
                continue;
            };
            let Some(name) = rest.strip_suffix(';') else {
                continue;
            };
            declared = declared.saturating_add(1);
            let directory = path.parent().unwrap_or(&root);
            let sibling = relative(&directory.join(format!("{name}.rs")));
            let nested = relative(&directory.join(name).join("mod.rs"));
            assert!(
                read.contains(&sibling) || read.contains(&nested),
                "module {name} declared in {} was not read by the walk",
                relative(path)
            );
        }
    }
    assert!(declared >= 9, "only {declared} modules were declared");
    Ok(())
}

// ---------------------------------------------------------------------------
// One clock
// ---------------------------------------------------------------------------

/// The whole set of public signatures in this crate whose return type names a
/// [`academic_capture::SessionTick`], with why each one exists.
///
/// Counted as a set rather than as a number, because a producer nobody
/// predicted has to fail as an extra key. Every entry below but the first two
/// is an *accessor*: it hands back a tick that already exists.
/// `SessionClock::tick` mints one and `SessionClock::admit` returns the tick it
/// was handed after comparing its domain.
const TICK_RETURNING_SIGNATURES: [(&str, &str); 10] = [
    ("clock.rs SessionClock::tick", "mints; the one producer"),
    (
        "clock.rs SessionClock::admit",
        "returns the tick it was handed, or refuses it",
    ),
    ("mark.rs Mark::at", "the mark's own instant"),
    (
        "mark.rs MarkLabel::applied_at",
        "the label's own instant, which is not a mark's",
    ),
    (
        "mark.rs LabelledMark::at",
        "the mark's instant, whichever labels exist",
    ),
    (
        "align.rs Anchor::session_tick",
        "where the anchor sits on the session clock",
    ),
    (
        "journal.rs JournalRecord::at",
        "the instant the frame was recorded at",
    ),
    (
        "preflight.rs FailureSignal::raised_at",
        "when the signal reached the timeline",
    ),
    (
        "recorder.rs CaptureRecorder::origin",
        "the tick the session opened at",
    ),
    (
        "recorder.rs CaptureRecorder::audio_epoch",
        "the instant audio offsets are measured from",
    ),
];

/// Every seam that mints an instant, and how many it may mint.
///
/// The list is closed in both directions: each entry is pinned whole, and the
/// number of `self.clock.tick` calls in the whole recorder must equal the sum
/// of the counts here, so a ninth seam fails even if nobody adds it to a pin.
const CLOCK_CALL_SITES: [(&str, usize); 8] = [
    ("fn start_session(", 1),
    ("pub fn record_audio_chunk(", 1),
    ("pub fn capture_image(", 1),
    ("pub fn mark(", 1),
    ("pub fn label_mark(", 1),
    ("pub fn observe(", 1),
    ("pub fn realign(", 1),
    ("fn open_gap(", 1),
];

#[test]
fn the_only_instant_type_comes_from_one_clock() -> TestResult {
    let root = crate_root();
    let clock = fs::read_to_string(root.join("src/clock.rs"))?;
    let recorder = fs::read_to_string(root.join("src/recorder.rs"))?;
    let clock_code = strip_non_code(&clock);
    let recorder_code = strip_non_code(&recorder);

    // A tick's three fields are private, so only `clock.rs` can build one. That
    // is the compiler's rule and this is what keeps it: a `pub` on any of them
    // would open the type to a literal written anywhere in the workspace.
    assert_eq!(
        declared_method(&clock, "pub struct SessionTick {")?,
        "pub struct SessionTick { domain: SessionClockDomain, seq: u32, elapsed_nanos: u64, }",
        "the tick's fields changed; a public field opens the type to a literal"
    );

    // Inside `clock.rs`, the whole inventory of construction sites. `tick`
    // spells the type and `recorded` spells `Self`, so both spellings are
    // counted rather than the one somebody thought of.
    let impl_tick = declared_method(&clock, "impl SessionTick {")?;
    let impl_clock = declared_method(&clock, "impl SessionClock {")?;
    assert_eq!(
        literals_of(&impl_tick, "Self"),
        1,
        "impl SessionTick builds a tick somewhere beside its one crate-private constructor"
    );
    assert_eq!(
        literals_of(&impl_tick, "SessionTick"),
        0,
        "impl SessionTick names the type as a literal as well as Self"
    );
    assert_eq!(
        literals_of(&impl_clock, "SessionTick"),
        1,
        "the minting site in impl SessionClock is no longer the only one"
    );
    assert_eq!(
        literals_of(&impl_clock, "Self"),
        1,
        "impl SessionClock builds something beside the clock itself"
    );

    // The whole set of signatures that hand back a tick, compared against the
    // reviewed inventory. A producer written anywhere in the crate appears here
    // as a key nobody wrote a reason for.
    let mut returning: BTreeSet<String> = BTreeSet::new();
    let mut returning_count = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        let file = relative(&path);
        for signature in public_signatures(&code) {
            let Some((_, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if uses_of(returns, "SessionTick") == 0 {
                continue;
            }
            returning_count = returning_count.saturating_add(1);
            let name = signature
                .split_once("fn ")
                .and_then(|(_, tail)| tail.split('(').next())
                .unwrap_or("?")
                .to_owned();
            returning.insert(format!("{file}:{name}"));
        }
    }
    assert_eq!(
        returning,
        [
            "src/align.rs:session_tick",
            "src/clock.rs:admit",
            "src/clock.rs:tick",
            "src/journal.rs:at",
            "src/mark.rs:applied_at",
            "src/mark.rs:at",
            "src/preflight.rs:raised_at",
            "src/recorder.rs:audio_epoch",
            "src/recorder.rs:origin",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<String>>(),
        "the set of signatures that hand back a session tick changed; \
         the reviewed inventory is {TICK_RETURNING_SIGNATURES:?}"
    );
    // `Mark::at` and `LabelledMark::at` are the same name in one file, so the
    // set above collapses them onto one key. The count is what keeps that pair
    // from silently becoming one, and what fails if a tenth appears.
    assert_eq!(
        returning_count,
        TICK_RETURNING_SIGNATURES.len(),
        "the number of tick-returning signatures and the reviewed inventory disagree"
    );

    // And there is exactly one clock. `SessionClock::start` is called once in
    // the whole product tree.
    let mut starts = 0_usize;
    let mut starting_files = BTreeSet::new();
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        let count = occurrences(&code, "SessionClock::start");
        if count > 0 {
            starts = starts.saturating_add(count);
            starting_files.insert(relative(&path));
        }
    }
    assert_eq!(starts, 1, "a second session clock is started somewhere");
    assert_eq!(
        starting_files,
        ["src/recorder.rs".to_owned()].into_iter().collect(),
        "the clock is started outside the one place that builds a recorder"
    );
    // A `use` alias would spell no `SessionClock::`, so the imports are read
    // too: nothing renames the type.
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        assert_eq!(
            occurrences(&code, "SessionClock as "),
            0,
            "{}: the session clock is aliased",
            relative(&path)
        );
    }

    // The pins, each beside the count of `self.clock.tick` calls it makes. A
    // seam that stopped deriving its instant from the recorder's own clock
    // fails the count; one that was edited at all fails the pin.
    let pins: [(&str, &str); 8] = [
        ("fn start_session(", WHOLE_START_SESSION),
        ("pub fn record_audio_chunk(", WHOLE_RECORD_AUDIO_CHUNK),
        ("pub fn capture_image(", WHOLE_CAPTURE_IMAGE),
        ("pub fn mark(", WHOLE_MARK),
        ("pub fn label_mark(", WHOLE_LABEL_MARK),
        ("pub fn observe(", WHOLE_OBSERVE),
        ("pub fn realign(", WHOLE_REALIGN),
        ("fn open_gap(", WHOLE_OPEN_GAP),
    ];
    for (signature, pinned) in pins {
        let text = declared_method(&recorder, signature)?;
        assert_eq!(text, pinned, "{signature} is no longer its pinned text");
        let expected_calls = CLOCK_CALL_SITES
            .iter()
            .find(|(name, _)| *name == signature)
            .map(|(_, count)| *count)
            .ok_or_else(|| format!("{signature} has no reviewed clock-call count"))?;
        assert_eq!(
            occurrences(&text, "clock.tick("),
            expected_calls,
            "{signature} does not take its instant from the recorder's own clock"
        );
    }
    // The inventory and the pin list are the same list, in both directions.
    assert_eq!(
        CLOCK_CALL_SITES
            .iter()
            .map(|(name, _)| *name)
            .collect::<BTreeSet<_>>(),
        pins.iter().map(|(name, _)| *name).collect::<BTreeSet<_>>()
    );
    // Every `clock.tick(` in the recorder is inside one of the pinned seams, so
    // an eighth seam cannot mint an instant outside the reviewed list.
    let pinned_calls: usize = pins
        .iter()
        .map(|(_, text)| occurrences(text, "clock.tick("))
        .sum();
    assert_eq!(
        occurrences(&recorder_code, "clock.tick("),
        pinned_calls,
        "the recorder mints an instant outside the pinned seams"
    );
    // And the recorder holds exactly one clock field.
    assert_eq!(
        occurrences(&recorder_code, "clock: SessionClock"),
        1,
        "the recorder holds more than one session clock"
    );

    // The evasions. Each is a way to reach a second clock that the rules above
    // are written against, and each must be caught.
    let evasions: [(&str, &str); 4] = [
        (
            "a second start under a path alias",
            "fn shadow() { let c = crate::clock::SessionClock::start(l, &t, None); }",
        ),
        (
            "a second start through a use alias",
            "use crate::clock::SessionClock as Wall; fn shadow() { Wall::start(l, &t, None); }",
        ),
        (
            "a public field that opens the tick to a literal",
            "pub struct SessionTick { pub domain: SessionClockDomain, seq: u32, elapsed_nanos: u64, }",
        ),
        (
            "an eighth seam that hands back an instant",
            "impl CaptureRecorder {\n    pub fn last_tick(&self) -> SessionTick { self.origin }\n}",
        ),
    ];
    for (name, sample) in evasions {
        let code = strip_non_code(sample);
        let extra_start = occurrences(&code, "SessionClock::start") > 0;
        let aliased = occurrences(&code, "SessionClock as ") > 0;
        let opened = code.contains("pub struct SessionTick {") && code.contains("pub domain");
        let extra_returner = public_signatures(&code).iter().any(|signature| {
            parameters_and_return(signature)
                .is_some_and(|(_, returns)| uses_of(returns, "SessionTick") > 0)
        });
        assert!(
            extra_start || aliased || opened || extra_returner,
            "the evasion `{name}` slipped past every rule"
        );
    }
    // And the checks are not vacuous against the real files.
    assert!(recorder_code.contains("clock.tick("));
    assert!(clock_code.contains("SessionTick {"));
    Ok(())
}

#[test]
fn no_wall_clock_reaches_the_session_clock() -> TestResult {
    // The contract says a session clock never steps back, and the reason it
    // matters is that a wall-clock reading can. This crate reads no clock at
    // all, so there is nothing to compare against: every elapsed reading is an
    // argument, and a call to a clock inside it would be the second source the
    // whole design exists to refuse.
    //
    // The list is a token list, which the scans page warns about, but it is a
    // complete one rather than a list of spellings somebody predicted.
    // `std::time` is the whole of the standard library's clock and its two
    // entry points are `SystemTime` and `Instant`; `Uuid::now_v7` is the third
    // wall clock this repository has, reached through `academic-domain`; and
    // `chrono` is not in the workspace at all. This crate's only two product
    // dependencies are `academic-consent` and `academic-domain`, whose own
    // surfaces are types and digests, so there is no fourth route to reach for.
    // A crate added to the product edge later is the case this would not see,
    // and `workspace_dependency_direction_is_acyclic` is what fails then.
    const CLOCK_SPELLINGS: [&str; 7] = [
        "SystemTime",
        "Instant",
        "UNIX_EPOCH",
        "std::time",
        "chrono",
        "now_v7",
        "elapsed",
    ];
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        scanned = scanned.saturating_add(1);
        let code = code_of(&path)?;
        for spelling in CLOCK_SPELLINGS {
            // `elapsed` is on the list as a *call*: `Instant::elapsed` and
            // `SystemTime::elapsed` are the two shapes that read a clock
            // without naming one at the call site. The field and accessor this
            // crate spells are `elapsed_nanos`, which `uses_of` does not match.
            let found = if spelling == "elapsed" {
                occurrences(&code, ".elapsed()")
            } else {
                uses_of(&code, spelling)
            };
            assert_eq!(
                found,
                0,
                "{}: this crate reads a clock ({spelling})",
                relative(&path)
            );
        }
    }
    assert!(scanned >= 9, "the clock scan read only {scanned} files");

    // The positive half: the reading is a parameter everywhere it enters.
    let recorder = fs::read_to_string(crate_root().join("src/recorder.rs"))?;
    for signature in [
        "pub fn record_audio_chunk(",
        "pub fn capture_image(",
        "pub fn mark(",
        "pub fn label_mark(",
        "pub fn observe(",
        "pub fn realign(",
    ] {
        assert!(
            declared_method(&recorder, signature)?.contains("elapsed_nanos: u64"),
            "{signature} no longer takes its reading as an argument"
        );
    }

    // The evasions, run through the same check.
    for (name, sample) in [
        (
            "a fully qualified read",
            "let n = std::time::Instant::now();",
        ),
        (
            "an aliased import",
            "use std::time::SystemTime as S; let n = S::now();",
        ),
        (
            "a duration read off a stored instant",
            "let d = self.started.elapsed().as_nanos();",
        ),
        ("a uuid clock", "let id = Uuid::now_v7();"),
    ] {
        let code = strip_non_code(sample);
        let caught = CLOCK_SPELLINGS.iter().any(|spelling| {
            if *spelling == "elapsed" {
                occurrences(&code, ".elapsed()") > 0
            } else {
                uses_of(&code, spelling) > 0
            }
        });
        assert!(caught, "the evasion `{name}` slipped past the clock scan");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// A label never moves a mark
// ---------------------------------------------------------------------------

#[test]
fn a_label_has_no_path_that_moves_a_mark() -> TestResult {
    let mark_source = fs::read_to_string(crate_root().join("src/mark.rs"))?;
    let mark_code = strip_non_code(&mark_source);

    // The whole set of `impl` blocks whose header names `Mark` exactly. A
    // `DerefMut`, an `AsMut`, or any other trait through which a mark could be
    // written fails as an extra key rather than being missed by a token list.
    // An `impl` written in another crate is refused by the orphan rule instead.
    let headers: BTreeSet<String> = crate_product_sources()?
        .iter()
        .map(|path| code_of(path))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .flat_map(|code| impl_headers_naming(code, "Mark"))
        .collect();
    assert_eq!(
        headers,
        ["impl Mark".to_owned()]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "the set of impl blocks naming Mark changed"
    );

    // No method on a mark takes `&mut self`, so a value of the type cannot
    // change after it is built.
    let mark_impl = declared_method(&mark_source, "impl Mark {")?;
    assert_eq!(
        occurrences(&mark_impl, "&mut self"),
        0,
        "a mark gained a mutating method"
    );

    // No public signature in this crate turns a label into an instant, in
    // either direction: a function that takes a `MarkLabel` and returns a
    // `SessionTick` is the shape that would let a label be read as a mark's
    // time. `LabelledMark::at` is the function this rule exists to constrain
    // and it takes neither.
    let mut checked = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            checked = checked.saturating_add(1);
            assert!(
                !(uses_of(parameters, "MarkLabel") > 0 && uses_of(returns, "SessionTick") > 0),
                "{}: {signature} turns a label into an instant",
                relative(&path)
            );
        }
    }
    assert!(checked >= 40, "only {checked} public signatures were read");

    // One step outside. The types are public, so any crate in the workspace
    // could declare the accessor this one does not. `P2-RF10`'s
    // `no_public_signature_hands_out_ingested_text` is the same rule for the
    // other quarantine and this is it for a mark's instant.
    let workspace = workspace_product_sources()?;
    let mut packages = BTreeSet::new();
    let mut workspace_signatures = 0_usize;
    for (package, path) in &workspace {
        packages.insert(package.clone());
        let code = code_of(path)?;
        for signature in public_signatures(&code) {
            workspace_signatures = workspace_signatures.saturating_add(1);
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            assert!(
                !(uses_of(parameters, "MarkLabel") > 0 && uses_of(returns, "SessionTick") > 0),
                "{}: {signature} turns a label into an instant",
                path.display()
            );
        }
    }
    assert!(packages.len() >= 25, "only {} packages", packages.len());
    assert!(
        workspace_signatures >= 1_200,
        "only {workspace_signatures} public signatures across the workspace"
    );

    // The evasions.
    for (name, sample) in [
        (
            "a mutating accessor on the mark",
            "impl Mark {\n    pub fn set_at(&mut self, at: SessionTick) { self.at = at; }\n}",
        ),
        (
            "a trait implementation that hands out a mutable mark",
            "impl AsMut<Mark> for MarkLedger {\n    fn as_mut(&mut self) -> &mut Mark { }\n}",
        ),
        (
            "a free function that reads a label as an instant",
            "pub fn moment_of(label: MarkLabel) -> SessionTick { label.applied_at() }",
        ),
        (
            "the same through a reference and a lifetime",
            "pub fn moment_of<'a>(label: &'a MarkLabel) -> &'a SessionTick { }",
        ),
    ] {
        let code = strip_non_code(sample);
        let extra_impl = impl_headers_naming(&code, "Mark")
            .iter()
            .any(|header| header != "impl Mark");
        let mutating = occurrences(&code, "&mut self") > 0 && code.contains("impl Mark {");
        let converting = public_signatures(&code).iter().any(|signature| {
            parameters_and_return(signature).is_some_and(|(parameters, returns)| {
                uses_of(parameters, "MarkLabel") > 0 && uses_of(returns, "SessionTick") > 0
            })
        });
        assert!(
            extra_impl || mutating || converting,
            "the evasion `{name}` slipped past every rule"
        );
    }
    assert!(mark_code.contains("impl Mark {"));
    Ok(())
}

// ---------------------------------------------------------------------------
// The journal appends
// ---------------------------------------------------------------------------

/// Every public method on the journal that takes `&mut self`, and why.
const JOURNAL_MUTATORS: [(&str, &str); 1] = [(
    "pub fn append( &mut self, at: SessionTick, body: RecordBody, ) -> Result<&JournalRecord, JournalFault> {",
    "the one operation that reaches the file, and it only ever extends it",
)];

#[test]
fn the_journal_appends_and_never_rewrites() -> TestResult {
    let source = fs::read_to_string(crate_root().join("src/journal.rs"))?;
    let code = strip_non_code(&source);

    // The whole set of public `&mut self` methods, compared against a table
    // with a written reason for each. A `rewrite`, a `truncate` or a
    // `replace_record` fails as an extra key whatever it is called.
    let mutators: BTreeSet<String> = public_signatures(&code)
        .into_iter()
        .filter(|signature| signature.contains("&mut self"))
        .collect();
    assert_eq!(
        mutators,
        JOURNAL_MUTATORS
            .iter()
            .map(|(signature, _)| (*signature).to_owned())
            .collect::<BTreeSet<_>>(),
        "the journal's mutating surface changed"
    );

    // The file is shortened in exactly one place, and it is pinned whole
    // together with the recovery it depends on, because a pin on a decision
    // needs a pin on the sequence that reaches it.
    assert_eq!(
        calls_of(&code, "set_len"),
        1,
        "the journal shortens its file somewhere other than recovery"
    );
    assert_eq!(
        declared_method(&source, "pub fn reopen(")?,
        WHOLE_REOPEN,
        "reopen is no longer its pinned text"
    );
    assert_eq!(
        declared_method(&source, "pub fn append(")?,
        WHOLE_APPEND,
        "append is no longer its pinned text"
    );

    // A write reaches the file from `append` and from `create` and nowhere
    // else, and neither seeks backwards.
    assert_eq!(calls_of(&code, "write_all"), 3, "an extra write site");
    assert_eq!(
        occurrences(&code, "SeekFrom::Start"),
        0,
        "the journal seeks to an absolute position"
    );
    assert_eq!(
        occurrences(&code, "SeekFrom::End(0)"),
        1,
        "the journal's only seek is no longer to the end"
    );

    // The evasions.
    for (name, sample) in [
        (
            "a rewrite method",
            "impl ChunkJournal {\n    pub fn rewrite(&mut self, seq: u32) -> Result<(), JournalFault> { }\n}",
        ),
        (
            "a mutating accessor with an innocuous name",
            "impl ChunkJournal {\n    pub fn records_mut(&mut self) -> &mut Vec<JournalRecord> { }\n}",
        ),
        (
            "a second truncation",
            "fn compact(&self) -> Result<(), JournalFault> { self.file.set_len(0)?; Ok(()) }",
        ),
        (
            "a seek back over a written frame",
            "self.file.seek(SeekFrom::Start(104))?; self.file.write_all(&frame)?;",
        ),
    ] {
        let sample_code = strip_non_code(sample);
        let extra_mutator = public_signatures(&sample_code)
            .iter()
            .any(|signature| signature.contains("&mut self"));
        let extra_truncation = calls_of(&sample_code, "set_len") > 0;
        let absolute_seek = occurrences(&sample_code, "SeekFrom::Start") > 0;
        assert!(
            extra_mutator || extra_truncation || absolute_seek,
            "the evasion `{name}` slipped past every rule"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The thresholds are rows
// ---------------------------------------------------------------------------

#[test]
fn the_thresholds_are_versioned_rows_and_not_constants() -> TestResult {
    // Four numbers decide what this crate does and none of them may be spelled
    // in an `if`. `P2-U4`'s `repeat_ceiling_effective_date` is the precedent:
    // a constant cannot be superseded and cannot be dated.
    const THRESHOLDS: [&str; 4] = [
        "drift_tolerance_nanos",
        "storage_floor_bytes",
        "battery_floor_percent",
        "notification_within_nanos",
    ];
    let root = crate_root();
    let mut reached = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        let is_policy = path == root.join("src/policy.rs");
        for threshold in THRESHOLDS {
            let uses = uses_of(&code, threshold);
            if uses == 0 {
                continue;
            }
            if is_policy {
                continue;
            }
            reached = reached.saturating_add(uses);
            // Outside the policy module a threshold may only be *read off a
            // row*. The spelling that does that is `.threshold()`, so a field
            // access, a local binding or a constant of the same name fails.
            assert_eq!(
                occurrences(&code, &format!(".{threshold}()")),
                uses,
                "{}: {threshold} is reached other than as a row's accessor",
                relative(&path)
            );
        }
    }
    assert!(
        reached >= 4,
        "only {reached} threshold reads outside policy.rs"
    );

    // Every numeric literal that decides one lives in `policy.rs`, and the
    // shipped row is the only place they are written down.
    let policy = fs::read_to_string(root.join("src/policy.rs"))?;
    assert_eq!(
        declared_method(&policy, "pub fn published(")?,
        WHOLE_PUBLISHED,
        "the shipped policy book is no longer its pinned text"
    );
    // The lookup is a dated select and returns `None` rather than a default.
    assert_eq!(
        declared_method(&policy, "pub fn effective_at(")?,
        WHOLE_EFFECTIVE_AT,
        "the effective-row lookup is no longer its pinned text"
    );
    // And it is reached from the one place a capture chooses its thresholds.
    let recorder = strip_non_code(&fs::read_to_string(root.join("src/recorder.rs"))?);
    assert_eq!(
        calls_of(&recorder, "effective_at"),
        2,
        "the recorder selects its policy row somewhere other than begin and resume"
    );

    // The evasions.
    for (name, sample) in [
        (
            "a constant beside the row",
            "const DRIFT_TOLERANCE_NANOS: u64 = 2_000_000_000; fn f() { let drift_tolerance_nanos = DRIFT_TOLERANCE_NANOS; }",
        ),
        (
            "a field read instead of an accessor",
            "fn f(row: CapturePolicyRow) -> u64 { row.drift_tolerance_nanos }",
        ),
        (
            "a local shadowing the accessor",
            "fn f() { let storage_floor_bytes = 0; if free < storage_floor_bytes { } }",
        ),
    ] {
        let code = strip_non_code(sample);
        let caught = THRESHOLDS.iter().any(|threshold| {
            uses_of(&code, threshold) > occurrences(&code, &format!(".{threshold}()"))
        });
        assert!(
            caught,
            "the evasion `{name}` slipped past the threshold rule"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The closed vocabularies
// ---------------------------------------------------------------------------

#[test]
fn every_closed_vocabulary_is_the_list_its_enum_declares() -> TestResult {
    // Six enums carry a frame byte, a contract spelling and an `ALL` array, and
    // each of those is a second transcription of the variant list. A variant
    // added without a row would be invisible to a check that walked `ALL`, so
    // the variants are read out of the source and compared with what the type
    // reports at run time.
    let root = crate_root();
    let files: [(&str, &str, Vec<String>, Vec<u8>); 5] = [
        (
            "src/preflight.rs",
            "pub enum SignalDelivery {",
            SignalDelivery::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
            SignalDelivery::ALL
                .iter()
                .map(|value| value.code())
                .collect(),
        ),
        (
            "src/preflight.rs",
            "pub enum FailureKind {",
            FailureKind::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
            FailureKind::ALL.iter().map(|value| value.code()).collect(),
        ),
        (
            "src/journal.rs",
            "pub enum GapCause {",
            GapCause::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
            GapCause::ALL.iter().map(|value| value.code()).collect(),
        ),
        (
            "src/mark.rs",
            "pub enum MarkLabelKind {",
            MarkLabelKind::ALL
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
            MarkLabelKind::ALL
                .iter()
                .map(|value| value.code())
                .collect(),
        ),
        (
            "src/capture.rs",
            "pub enum Orientation {",
            Orientation::ALL
                .iter()
                .map(|value| format!("{value:?}"))
                .collect(),
            Orientation::ALL.iter().map(|value| value.code()).collect(),
        ),
    ];
    for (file, header, spellings, codes) in files {
        let source = fs::read_to_string(root.join(file))?;
        let variants = enum_variants(&source, header);
        assert!(!variants.is_empty(), "{header} declares no variants");
        assert_eq!(
            variants.len(),
            spellings.len(),
            "{header}: the declared variants and its ALL array disagree"
        );
        // Every code is distinct and non-zero, so a byte cannot decode as two
        // variants and a zeroed frame decodes as none.
        let distinct: BTreeSet<u8> = codes.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            codes.len(),
            "{header}: two variants share a code"
        );
        assert!(!distinct.contains(&0), "{header}: a variant is coded zero");
        // Every spelling is distinct too.
        let distinct_spellings: BTreeSet<&String> = spellings.iter().collect();
        assert_eq!(
            distinct_spellings.len(),
            spellings.len(),
            "{header}: two variants share a spelling"
        );
    }

    // The delivery vocabulary is the one that carries a claim: both forms are
    // silent, and there is no third form for an intrusive one to be.
    let preflight = fs::read_to_string(root.join("src/preflight.rs"))?;
    assert_eq!(
        enum_variants(&preflight, "pub enum SignalDelivery {"),
        vec!["SilentBanner".to_owned(), "SilentHaptic".to_owned()],
        "the signal delivery vocabulary changed"
    );
    for delivery in SignalDelivery::ALL {
        assert!(
            delivery.as_str().starts_with("SILENT_"),
            "{} is not a silent delivery",
            delivery.as_str()
        );
    }

    // The fault selectors are the failpoints the source declares, so a
    // failpoint added without a row in the inventory fails.
    let fault = fs::read_to_string(root.join("src/fault.rs"))?;
    assert_eq!(
        enum_variants(&fault, "pub(crate) enum FaultPoint {").len(),
        FAULT_SELECTORS.len(),
        "the failpoint enum and the selector inventory disagree"
    );
    for selector in FAULT_SELECTORS {
        assert!(
            selector.starts_with("CP05:"),
            "{selector} is not a CP05 selector"
        );
    }
    Ok(())
}

#[test]
fn the_default_lane_compiles_no_failpoint() -> TestResult {
    // A product build contains no environment lookup and no crash switch. The
    // whole of the environment surface sits inside the feature-gated half of
    // one file, and nothing else in the crate reads the environment at all.
    let root = crate_root();
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        scanned = scanned.saturating_add(1);
        if path == root.join("src/fault.rs") {
            continue;
        }
        let code = code_of(&path)?;
        for spelling in ["std::env", "env::var", "env::var_os", "process::abort"] {
            assert_eq!(
                uses_of(&code, spelling),
                0,
                "{}: reads the environment or aborts ({spelling})",
                relative(&path)
            );
        }
    }
    assert!(scanned >= 9);

    let fault = fs::read_to_string(root.join("src/fault.rs"))?;
    let code = strip_non_code(&fault);
    // Every environment read and the abort sit inside the one feature-gated
    // function, which is pinned whole beside the attribute that gates it. A
    // second reader added elsewhere in the file fails the equality below, and
    // an edit to the gate itself fails the pin.
    assert!(
        fault.contains("#[cfg(feature = \"phase2-fault-injection\")]\npub(crate) fn trip("),
        "the failpoint is not directly under the feature gate"
    );
    let trip = declared_method(
        &fault,
        "pub(crate) fn trip(point: FaultPoint, frame_seq: u32) {",
    )?;
    assert_eq!(
        trip, WHOLE_TRIP,
        "the failpoint is no longer its pinned text"
    );
    for spelling in ["env::var", "env::var_os", "process::abort"] {
        assert_eq!(
            uses_of(&code, spelling),
            uses_of(&trip, spelling),
            "{spelling} appears outside the gated failpoint"
        );
    }
    // And the default arm is a no-op that takes the same arguments.
    assert!(
        code.contains("pub(crate) const fn trip(_point: FaultPoint, _frame_seq: u32) {}"),
        "the default-lane failpoint is not a no-op"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Whole-text pins
// ---------------------------------------------------------------------------

const WHOLE_START_SESSION: &str = "fn start_session( token: CaptureCapabilityToken, policy: CapturePolicyRow, journal_path: &Path, recovered: Option<ChunkJournal>, ) -> Result<CaptureRecorder, CaptureFault> { let lecture_id = token.bound().lecture_id(); let token_id = *token.token_id(); let predecessor = recovered .as_ref() .filter(|journal| !journal.records().is_empty()) .map(|journal| *journal.tail()); let mut clock = SessionClock::start(lecture_id, &token_id, predecessor.as_ref()); let origin = clock.tick(0)?; let mut journal = match recovered { Some(journal) => journal, None => ChunkJournal::create(journal_path, clock.domain(), policy.digest(), token_id)?, }; if !journal.records().is_empty() { journal.append( origin, RecordBody::Gap { cause: GapCause::SessionResumed, resumed_domain: Some(clock.domain()), }, )?; } Ok(CaptureRecorder { token, clock, journal, policy, origin, first_audio: None, marks: MarkLedger::new(), mapping: MappingLedger::new(), signals: Vec::new(), stopped: None, }) }";
const WHOLE_RECORD_AUDIO_CHUNK: &str = "pub fn record_audio_chunk( &mut self, ledger: &mut ConsentLedger, bytes: Vec<u8>, elapsed_nanos: u64, now: u64, ) -> Result<&JournalRecord, CaptureFault> { self.still_running()?; self.rebind(ledger, now)?; let at = self.clock.tick(elapsed_nanos)?; if self.first_audio.is_none() { self.first_audio = Some(at); } let record = self.journal.append( at, RecordBody::AudioChunk { bytes: CaptureBytes::of(bytes), }, )?; Ok(record) }";
const WHOLE_CAPTURE_IMAGE: &str = "pub fn capture_image( &mut self, ledger: &mut ConsentLedger, bytes: Vec<u8>, orientation: Orientation, elapsed_nanos: u64, now: u64, ) -> Result<&JournalRecord, CaptureFault> { self.still_running()?; self.rebind(ledger, now)?; let epoch = self.audio_epoch(); let at = self.clock.tick(elapsed_nanos)?; let audio_clock_offset_nanos = at.offset_from(epoch).ok_or(ClockFault::ForeignDomain)?; let record = self.journal.append( at, RecordBody::ImageCapture { bytes: CaptureBytes::of(bytes), orientation, audio_clock_offset_nanos, }, )?; Ok(record) }";
const WHOLE_MARK: &str = "pub fn mark( &mut self, ledger: &mut ConsentLedger, elapsed_nanos: u64, now: u64, ) -> Result<Mark, CaptureFault> { self.still_running()?; self.rebind(ledger, now)?; let at = self.clock.tick(elapsed_nanos)?; let mark = self.marks.append_mark(at); self.journal.append( at, RecordBody::Mark { mark_seq: mark.seq(), }, )?; Ok(mark) }";
const WHOLE_LABEL_MARK: &str = "pub fn label_mark( &mut self, ledger: &mut ConsentLedger, mark_seq: u32, kind: MarkLabelKind, elapsed_nanos: u64, now: u64, ) -> Result<LabelledMark, CaptureFault> { self.still_running()?; self.rebind(ledger, now)?; let at = self.clock.tick(elapsed_nanos)?; self.marks.append_label(mark_seq, kind, at)?; self.journal .append(at, RecordBody::MarkLabel { mark_seq, kind })?; self.marks .resolve(mark_seq) .ok_or(CaptureFault::Mark(MarkFault::UnknownMark { seq: mark_seq })) }";
const WHOLE_OBSERVE: &str = "pub fn observe( &mut self, reading: PreflightReading, observed_at_nanos: u64, elapsed_nanos: u64, ) -> Result<Vec<FailureSignal>, CaptureFault> { self.still_running()?; let failures = reading.failures(self.policy); if failures.is_empty() { return Ok(Vec::new()); } let mut raised = Vec::with_capacity(failures.len()); for kind in failures { let at = self.clock.tick(elapsed_nanos)?; let signal = FailureSignal::raised(kind, at, observed_at_nanos); self.journal.append( at, RecordBody::FailureSignal { kind, delivery: signal.delivery(), observed_at_nanos, }, )?; self.signals.push(signal); raised.push(signal); } self.open_gap(GapCause::ResourceFailure, elapsed_nanos)?; Ok(raised) }";
const WHOLE_REALIGN: &str = "pub fn realign( &mut self, first: Anchor, second: Anchor, elapsed_nanos: u64, ) -> Result<MappingVersion, CaptureFault> { let version = self .mapping .append_realignment(&self.clock, first, second, self.policy)?; let at = self.clock.tick(elapsed_nanos)?; self.journal.append(at, mapping_version_body(version))?; Ok(version) }";
const WHOLE_REOPEN: &str = "pub fn reopen(path: &Path) -> Result<(Self, JournalRecovery), JournalFault> { let recovery = Self::recover(path)?; let file = OpenOptions::new().read(true).write(true).open(path)?; let complete = path .metadata()? .len() .saturating_sub(recovery.partial_tail_bytes); if recovery.partial_tail_bytes > 0 { file.set_len(complete)?; file.sync_all()?; } let mut file = file; file.seek(SeekFrom::End(0))?; let tail = recovery .records .last() .map_or_else(genesis, |record| *record.digest()); Ok(( Self { path: path.to_path_buf(), file, header: recovery.header, records: recovery.records.clone(), tail, }, recovery, )) }";
const WHOLE_APPEND: &str = "pub fn append( &mut self, at: SessionTick, body: RecordBody, ) -> Result<&JournalRecord, JournalFault> { let encoded = body.encode(); if encoded.len() > MAX_BODY_BYTES { return Err(JournalFault::BodyTooLarge { len: encoded.len() }); } let seq = u32::try_from(self.records.len()).unwrap_or(u32::MAX); let body_len = u32::try_from(encoded.len()).unwrap_or(u32::MAX); let mut frame_header = Vec::with_capacity(FRAME_HEADER_LEN); frame_header.extend_from_slice(&seq.to_be_bytes()); frame_header.push(body.kind_code()); frame_header.extend_from_slice(&at.seq().to_be_bytes()); frame_header.extend_from_slice(&at.elapsed_nanos().to_be_bytes()); frame_header.extend_from_slice(&body_len.to_be_bytes()); frame_header.extend_from_slice(self.tail.as_bytes()); let digest = frame_digest(&frame_header, &encoded); let mut frame = Vec::with_capacity( FRAME_HEADER_LEN .saturating_add(encoded.len()) .saturating_add(32), ); frame.extend_from_slice(&frame_header); frame.extend_from_slice(&encoded); frame.extend_from_slice(digest.as_bytes()); fault::trip(FaultPoint::BeforeFrameWrite, seq); let split = frame.len().saturating_sub(32); self.file.write_all(frame.get(..split).unwrap_or(&frame))?; fault::trip(FaultPoint::AfterBodyBeforeTrailer, seq); self.file.write_all(frame.get(split..).unwrap_or(&[]))?; self.file.sync_all()?; fault::trip(FaultPoint::AfterFrameSynced, seq); self.tail = digest; self.records.push(JournalRecord { seq, at, body, parent: ContentDigest::from_sha256_bytes( frame_header .get(FRAME_HEADER_LEN.saturating_sub(32)..) .and_then(|slice| slice.try_into().ok()) .unwrap_or([0_u8; 32]), ), digest, }); self.records.last().ok_or(JournalFault::HeaderIncomplete) }";
const WHOLE_PUBLISHED: &str = "pub fn published() -> Self { Self::of(vec![CapturePolicyRow::declare( \"capture.thresholds.2026_first\", PUBLISHED_EFFECTIVE_FROM, 2_000_000_000, 67_108_864, 5, 2_000_000_000, )]) }";
const WHOLE_EFFECTIVE_AT: &str = "pub fn effective_at(&self, at: u64) -> Option<CapturePolicyRow> { self.rows .iter() .rev() .find(|row| row.effective_from <= at) .copied() }";
const WHOLE_TRIP: &str = "pub(crate) fn trip(point: FaultPoint, frame_seq: u32) { use std::{env, fs::OpenOptions, io::Write as _, path::PathBuf}; if env::var(FAULT_SELECTION_VARIABLE).ok().as_deref() != Some(point.as_str()) { return; } if let Ok(selected) = env::var(FAULT_FRAME_VARIABLE) && selected.parse::<u32>().ok() != Some(frame_seq) { return; } if let Some(path) = env::var_os(FAULT_READY_MARKER_VARIABLE).map(PathBuf::from) && let Ok(mut marker) = OpenOptions::new().create_new(true).write(true).open(path) { let _ = marker.write_all(point.as_str().as_bytes()); let _ = marker.sync_all(); } std::process::abort(); }";
const WHOLE_OPEN_GAP: &str = "fn open_gap(&mut self, cause: GapCause, elapsed_nanos: u64) -> Result<(), CaptureFault> { let at = self.clock.tick(elapsed_nanos)?; self.journal.append( at, RecordBody::Gap { cause, resumed_domain: None, }, )?; self.stopped = Some(cause); Ok(()) }";
