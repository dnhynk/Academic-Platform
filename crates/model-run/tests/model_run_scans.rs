//! The source half of `P2-M1`'s two calibration prohibitions.
//!
//! `tests/model_run.rs` observes what the types do and `tests/compile_fail/`
//! observes what they refuse. Neither notices the day somebody adds the trait
//! or the accessor that would make those observations stop meaning anything,
//! because a new `impl PartialOrd for RawScore` makes the compile-fail case
//! compile and the runtime test still pass. That is what this file is for.
//!
//! Nothing here is a forbidden-token list. `docs/contracts/policy-source-scans.md`
//! records why: a list of spellings refuses the edits somebody thought of in
//! advance and admits every edit spelled differently. So the rules below are
//! whole-set comparisons, whole-text pins, and counts of identifiers rather than
//! of spellings.

use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

// ---------------------------------------------------------------------------
// Walk
// ---------------------------------------------------------------------------

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// Every `.rs` file anywhere under a directory.
///
/// The walk is over the *package*, not over `<crate>/src`. `S-12` recorded what
/// a walk rooted at `src` misses -- `crates/worker/probes/` and
/// `crates/record/examples/` are both compiled by
/// `cargo clippy --workspace --all-targets` and neither is under `src` -- and
/// `P2-RF10` widened four scans for it. This one starts wide.
///
/// `benches` is walked for the reason `S-14` gives: a bench target has no
/// feature gate and that same clippy command compiles it, so a rule that skips
/// it is a rule with a tree in it. Only `tests` is excluded from the product
/// half, because this crate's own suites name the types on purpose.
fn rust_sources(root: &Path) -> Result<Vec<Source>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                if entry.file_name() != "target" {
                    pending.push(path);
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let relative = path
                    .strip_prefix(repository_root())
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((relative, fs::read_to_string(&path)?));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Comments and string literals removed, so a rule reads code and not prose.
///
/// `r#"..."#` is modelled because `P2-G4` found a lexer that was not: a raw
/// string containing one quote left the quote count odd, and from there every
/// literal in the file was read as code and every stretch of code as a literal.
fn rust_code_only(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        let rest = &source[index..];
        if rest.starts_with("//") {
            let end = rest.find('\n').unwrap_or(rest.len());
            index += end;
        } else if rest.starts_with("/*") {
            let end = rest.find("*/").map_or(rest.len(), |offset| offset + 2);
            index += end;
        } else if let Some(hashes) = raw_string_open(rest) {
            let terminator = format!("\"{}", "#".repeat(hashes));
            let body = &rest[2 + hashes..];
            let end = body
                .find(&terminator)
                .map_or(rest.len(), |offset| 2 + hashes + offset + terminator.len());
            out.push_str("\"\"");
            index += end;
        } else if rest.starts_with('"') {
            let mut cursor = 1;
            while cursor < rest.len() {
                let character = rest.as_bytes()[cursor];
                if character == b'\\' {
                    cursor += 2;
                    continue;
                }
                cursor += 1;
                if character == b'"' {
                    break;
                }
            }
            out.push_str("\"\"");
            index += cursor;
        } else {
            let character = rest.chars().next().unwrap_or('\0');
            out.push(character);
            index += character.len_utf8();
        }
    }
    out
}

fn raw_string_open(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&b'r') {
        return None;
    }
    let mut hashes = 0;
    while bytes.get(1 + hashes) == Some(&b'#') {
        hashes += 1;
    }
    (bytes.get(1 + hashes) == Some(&b'"')).then_some(hashes)
}

/// Whitespace collapsed, so `cargo fmt` decides layout and a pin decides content.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The named item's whole text, comment lines dropped before collapsing.
fn whole_item(source: &str, header: &str) -> Option<String> {
    let start = source.find(header)?;
    let body_start = source[start..].find('{')? + start;
    let mut depth = 0_i32;
    let bytes = source.as_bytes();
    for (offset, byte) in bytes.iter().enumerate().skip(body_start) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let text = &source[start..=offset];
                    let stripped = text
                        .lines()
                        .filter(|line| !line.trim_start().starts_with("//"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    return Some(collapse(&stripped));
                }
            }
            _ => {}
        }
    }
    None
}

/// Occurrences of `name` as a whole identifier, not as a substring.
///
/// `P2-RF10`'s finding: an inventory that counts a spelling is not an
/// inventory. `Untrusted::expose(document)` is the same call as
/// `document.expose()` and contains no `.expose()` substring, so the count that
/// held the exposure sites open was a count of one way to write the call.
fn identifier_occurrences(code: &str, name: &str) -> usize {
    let mut count = 0;
    let mut from = 0;
    while let Some(offset) = code[from..].find(name) {
        let start = from + offset;
        let end = start + name.len();
        let before = code[..start].chars().next_back();
        let after = code[end..].chars().next();
        let boundary = |character: Option<char>| {
            character.is_none_or(|value| !value.is_alphanumeric() && value != '_')
        };
        if boundary(before) && boundary(after) {
            count += 1;
        }
        from = end;
    }
    count
}

// ---------------------------------------------------------------------------
// Floors and tripwires
// ---------------------------------------------------------------------------

/// One file the walk read: its repository-relative path and its whole text.
type Source = (String, String);

/// This package's sources, split into product source and all source.
fn package_sources() -> Result<(Vec<Source>, Vec<Source>), Box<dyn Error>> {
    let all = rust_sources(&repository_root().join("crates/model-run"))?;
    assert!(
        all.len() >= 7,
        "the walk read {} files under crates/model-run; it has stopped descending",
        all.len()
    );
    let product = all
        .iter()
        .filter(|(path, _)| !path.contains("/tests/"))
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        product.len() >= 5,
        "the walk read {} product files; it has stopped descending",
        product.len()
    );
    Ok((product, all))
}

#[test]
fn the_walk_reads_every_module_in_this_crate() -> Result<(), Box<dyn Error>> {
    let (product, all) = package_sources()?;
    let read = all
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<std::collections::BTreeSet<_>>();

    // Product source lives under `src` and nowhere else. `G-I13` is the shape:
    // a module beside `src`, reached by `#[path]`, that a `src`-rooted walk
    // never read.
    for (path, _) in &product {
        assert!(
            path.starts_with("crates/model-run/src/"),
            "{path} is product source outside src"
        );
    }

    // Every `mod` and every `#[path]` target resolves to a file the walk read.
    for (path, source) in &all {
        let code = rust_code_only(source);
        let directory = Path::new(path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        for line in code.lines().map(str::trim) {
            let declaration = line
                .strip_prefix("pub mod ")
                .or_else(|| line.strip_prefix("mod "));
            let Some(name) = declaration.and_then(|rest| rest.strip_suffix(';')) else {
                continue;
            };
            let beside = directory
                .join(format!("{name}.rs"))
                .to_string_lossy()
                .replace('\\', "/");
            let nested = directory
                .join(name)
                .join("mod.rs")
                .to_string_lossy()
                .replace('\\', "/");
            assert!(
                read.contains(&beside) || read.contains(&nested),
                "{path} declares mod {name}, which the walk did not read"
            );
        }
        assert!(
            !code.contains("#[path"),
            "{path} uses #[path]; the walk resolves no such target"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// cross_provider_raw_scores_are_not_ordered -- the source half
// ---------------------------------------------------------------------------

/// Every `impl` block in the workspace whose header names `RawScore`.
///
/// Compared as a whole set rather than searched for forbidden trait names. An
/// implementation of a trait nobody predicted fails as an extra entry, which a
/// list of nine trait names would not have caught -- `G-I2` is that
/// observation, a *local* trait handing the value back that spelled none of
/// the listed names.
const RAW_SCORE_IMPLS: [&str; 2] = ["impl RawScore", "impl fmt::Debug for RawScore"];

/// Every `impl` header in `code` that names `type_name`.
///
/// Read out of the code text rather than off line starts: an `impl` that does
/// not begin its line is still an `impl`, and a rule that only sees formatted
/// input is a rule that depends on `cargo fmt` having run.
fn impl_headers(code: &str, type_name: &str) -> Vec<String> {
    let mut headers = Vec::new();
    let mut from = 0;
    while let Some(offset) = code[from..].find("impl") {
        let start = from + offset;
        from = start + 4;
        let before = code[..start].chars().next_back();
        if before.is_some_and(|value| value.is_alphanumeric() || value == '_') {
            continue;
        }
        let Some(end) = code[start..].find(['{', ';']) else {
            continue;
        };
        let header = collapse(&code[start..start + end]);
        if header.contains(type_name) {
            headers.push(header);
        }
    }
    headers
}

#[test]
fn raw_score_has_no_ordering_implementation_anywhere() -> Result<(), Box<dyn Error>> {
    // This crate's own set, compared whole.
    let (product, _) = package_sources()?;
    let mut declared = Vec::new();
    for (_, source) in &product {
        declared.extend(impl_headers(&rust_code_only(source), "RawScore"));
    }
    declared.sort();
    assert_eq!(
        declared,
        RAW_SCORE_IMPLS.map(str::to_owned).to_vec(),
        "the set of impl blocks naming RawScore changed; a new one has to be reviewed \
         for whether it hands back a number or orders two"
    );

    // And the orphan half: no other crate may name the type in an `impl` header
    // either. `RawScore` is a public type any crate can name, and the harm
    // `P2-RF10` measured for `Untrusted<T>` was one crate out.
    let mut foreign = Vec::new();
    for package in fs::read_dir(repository_root().join("crates"))? {
        let package = package?.path();
        if package.file_name().is_some_and(|name| name == "model-run") {
            continue;
        }
        for (path, source) in rust_sources(&package)? {
            for header in impl_headers(&rust_code_only(&source), "RawScore") {
                foreign.push(format!("{path}: {header}"));
            }
        }
    }
    assert_eq!(
        foreign,
        Vec::<String>::new(),
        "a crate outside academic-model-run implements a trait for RawScore"
    );
    Ok(())
}

#[test]
fn raw_score_hands_back_no_number() -> Result<(), Box<dyn Error>> {
    // Counting the impl set says which traits exist, not what they hand back.
    // So the second rule, on the shape `no_public_signature_hands_out_ingested_text`
    // uses: no public signature anywhere in the workspace takes or returns a
    // `RawScore` beside a bare number type. A lifetime cannot hide one and a
    // type alias for `u32` still names an integer.
    const NUMERIC: [&str; 10] = [
        "u8", "u16", "u32", "u64", "u128", "usize", "i32", "i64", "f32", "f64",
    ];
    let mut offenders = Vec::new();
    for package in fs::read_dir(repository_root().join("crates"))? {
        for (path, source) in rust_sources(&package?.path())? {
            if path.contains("/tests/") {
                continue;
            }
            let code = rust_code_only(&source);
            for signature in code.split("pub fn ").skip(1) {
                let Some(head) = signature.split(&['{', ';'][..]).next() else {
                    continue;
                };
                if !head.contains("RawScore") {
                    continue;
                }
                let Some((_, returned)) = head.split_once("->") else {
                    continue;
                };
                let words = returned
                    .split(|character: char| !character.is_alphanumeric() && character != '_')
                    .collect::<Vec<_>>();
                if NUMERIC.iter().any(|numeric| words.contains(numeric)) {
                    offenders.push(format!("{path}: pub fn {}", collapse(head)));
                }
            }
        }
    }
    assert_eq!(
        offenders,
        Vec::<String>::new(),
        "a public signature turns a RawScore into a number, which is an ordering \
         one call away"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// uncalibrated_score_cannot_be_displayed -- the source half
// ---------------------------------------------------------------------------

/// Every public signature in the workspace that touches a calibrated value.
///
/// A whole inventory with a written reason for each, not a spelling count. It
/// covers signatures that take one as well as ones that return it, because the
/// claim is about the whole path from a raw number to a reader: a second
/// producer means the display no longer goes through the registry, and a second
/// consumer is a second display surface.
const CALIBRATED_SITES: [(&str, &str); 4] = [
    (
        "crates/model-run/src/calibration.rs: pub fn interpret",
        "the registry: the only producer, reading a raw score through the dataset registered for that exact provider, model version and purpose",
    ),
    (
        "crates/model-run/src/calibration.rs: pub fn of",
        "the display constructor: the only consumer, taking a calibrated value and returning a displayable one, so nothing uninterpreted reaches a reader",
    ),
    (
        "crates/offering/src/forecast.rs: pub fn calibrated",
        "P2-U5's accessor on ScoredForecast: it borrows the value `interpret` issued and produces no second one, and the type it returns is the calibrated type rather than a number, so the only thing a caller can do with it is hand it to `of`",
    ),
    (
        "crates/offering/src/standing.rs: pub fn calibrated",
        "P2-U5's accessor on HistoricallyLikelyStanding: the same borrow one level out, so section 8.3's 과거 패턴상 가능성 row can show its probability through the display constructor and by no other route",
    ),
];

#[test]
fn every_calibrated_value_comes_from_the_registry() -> Result<(), Box<dyn Error>> {
    let mut producers = Vec::new();
    for package in fs::read_dir(repository_root().join("crates"))? {
        for (path, source) in rust_sources(&package?.path())? {
            if path.contains("/tests/") {
                continue;
            }
            let code = rust_code_only(&source);
            for signature in code
                .split("pub fn ")
                .skip(1)
                .chain(code.split("pub const fn ").skip(1))
            {
                let Some(head) = signature.split(&['{', ';'][..]).next() else {
                    continue;
                };
                let Some((name, rest)) = head.split_once('(') else {
                    continue;
                };
                if rest.contains("CalibratedConfidence") {
                    producers.push(format!("{path}: pub fn {}", name.trim()));
                }
            }
        }
    }
    producers.sort();
    let mut reviewed = CALIBRATED_SITES
        .iter()
        .map(|(site, _)| (*site).to_owned())
        .collect::<Vec<_>>();
    reviewed.sort();
    assert_eq!(
        producers, reviewed,
        "a public signature takes or returns a CalibratedConfidence and is not in \
         CALIBRATED_SITES; add it with the reason it is allowed, in this commit"
    );
    for (site, reason) in CALIBRATED_SITES {
        assert!(
            reason.len() > 30,
            "{site} has no written reason; an inventory entry without one is a list"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Whole-text pins, and the call sites that reach them
// ---------------------------------------------------------------------------

/// `RawScore`'s `Debug`. The last formatting trait that could print the number.
const WHOLE_RAW_DEBUG: &str = "impl fmt::Debug for RawScore { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter .debug_struct(\"RawScore\") .field(\"provider\", &self.provider.as_str()) .field(\"model_version\", &self.model_version.as_str()) .field(\"units\", &\"<uncalibrated>\") .finish() } }";

/// The display constructor. Its parameter list is the claim that a displayed
/// confidence has been interpreted.
const WHOLE_DISPLAY_OF: &str = "pub fn of(calibrated: &CalibratedConfidence) -> Self { Self { permille: calibrated.confidence().value(), dataset: calibrated.dataset().clone(), } }";

/// The reconciliation's grant lookup, which is where the namespace is read.
///
/// Pinned for the `T141` reason as well as its own: a check that is not run
/// refuses nothing, so `reconcile_transmitted_ranges` is pinned beside it and
/// its call site counted, rather than only the decision it reaches.
const WHOLE_RECONCILE_DISPATCH: &str = "pub fn reconcile_transmitted_ranges( run: &ModelRun, audit_rows: &[AuditRow], consumptions: &[ConsumptionRow], ) -> Result<Reconciliation, ReconciliationError> { match run.transmitted_byte_ranges() { Transmission::LocalOnly => reconcile_local_only(run, audit_rows), Transmission::Egressed { grant_id, ranges } => { reconcile_egressed(grant_id.as_str(), ranges, audit_rows, consumptions) } } }";

/// The two foreign keys that make the consumption join exact.
///
/// `P2-M1`'s reconciliation keys on `egress_consumption` rather than on
/// `egress_audit.grant_id`, which carries identifiers from two tables and has no
/// foreign key of its own. What makes the join safe is this table: `grant_id`
/// references `egress_grant`, and `(egress_audit_seq, grant_id)` references
/// `egress_audit(audit_seq, grant_id)`. Pinned as whole text rather than
/// searched for by name, because a foreign key edited to reference something
/// weaker keeps its name.
const WHOLE_CONSUMPTION_TABLE: &str = "CREATE TABLE IF NOT EXISTS egress_consumption ( grant_id TEXT PRIMARY KEY REFERENCES egress_grant(grant_id) ON UPDATE RESTRICT ON DELETE RESTRICT, egress_audit_seq INTEGER NOT NULL UNIQUE REFERENCES egress_audit(audit_seq) ON UPDATE RESTRICT ON DELETE RESTRICT, consumed_at INTEGER NOT NULL CHECK (consumed_at >= 0), UNIQUE (egress_audit_seq, grant_id), FOREIGN KEY (egress_audit_seq, grant_id) REFERENCES egress_audit(audit_seq, grant_id) ON UPDATE RESTRICT ON DELETE RESTRICT ) STRICT;";

fn source_of(relative: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(repository_root().join(relative))?)
}

#[test]
fn the_calibration_and_reconciliation_decisions_are_pinned() -> Result<(), Box<dyn Error>> {
    let calibration = source_of("crates/model-run/src/calibration.rs")?;
    let reconcile = source_of("crates/model-run/src/reconcile.rs")?;

    let pins: [(&str, &str, &str); 3] = [
        ("WHOLE_RAW_DEBUG", &calibration, WHOLE_RAW_DEBUG),
        ("WHOLE_DISPLAY_OF", &calibration, WHOLE_DISPLAY_OF),
        (
            "WHOLE_RECONCILE_DISPATCH",
            &reconcile,
            WHOLE_RECONCILE_DISPATCH,
        ),
    ];
    for (name, source, expected) in pins {
        let header = match name {
            "WHOLE_RAW_DEBUG" => "impl fmt::Debug for RawScore",
            "WHOLE_DISPLAY_OF" => "pub fn of(calibrated:",
            _ => "pub fn reconcile_transmitted_ranges(",
        };
        let actual = whole_item(source, header)
            .ok_or_else(|| format!("{name}'s item is no longer in its file"))?;
        assert_eq!(
            actual, expected,
            "{name} changed; update the pin in this commit"
        );
    }

    // A pin fixes an item's text and says nothing about whether it runs. So the
    // one route into the grant comparison is counted: `reconcile_egressed` is
    // declared once and called once, from the pinned dispatch above.
    let code = rust_code_only(&reconcile);
    assert_eq!(
        identifier_occurrences(&code, "reconcile_egressed"),
        2,
        "reconcile_egressed is declared and called exactly once; a second caller \
         reaches the grant comparison by a route this pin does not fix"
    );
    Ok(())
}

#[test]
fn the_record_constructor_takes_every_field() -> Result<(), Box<dyn Error>> {
    // `ModelRun::record` is the only constructor, and every field is a
    // parameter. Counted rather than promised: the struct's fields and the
    // constructor's parameters are compared as sets, so a field that gains a
    // default is a failure.
    let source = source_of("crates/model-run/src/record.rs")?;
    let declaration = source
        .split("pub struct ModelRun {\n")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .ok_or("record.rs no longer declares ModelRun")?;
    let fields = declaration
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .filter_map(|line| line.split(':').next())
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();

    let signature = whole_item(&source, "pub const fn record(")
        .ok_or("record.rs no longer declares ModelRun::record")?;
    let parameters = signature
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(") -> Self"))
        .map(|(list, _)| list.to_owned())
        .ok_or("ModelRun::record's parameter list is unreadable")?;
    let named = parameters
        .split(',')
        .filter_map(|parameter| parameter.split(':').next())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        named, fields,
        "ModelRun::record's parameters are not the struct's fields; a field the \
         constructor does not take is a field the type does not require"
    );

    // And there is exactly one constructor: `record` is the only function in
    // the file returning `Self` for this type.
    let code = rust_code_only(&source);
    let constructors = code
        .split("pub const fn ")
        .skip(1)
        .chain(code.split("pub fn ").skip(1))
        .filter_map(|signature| signature.split('{').next())
        .filter(|head| head.contains("-> Self") && !head.contains("&self"))
        .count();
    assert!(
        constructors >= 1,
        "record.rs declares no constructor at all; the split above stopped reading"
    );

    // The twelve are also the twelve the digest covers, so a field added to the
    // struct without being hashed is a record whose digest does not describe it.
    let digest = whole_item(&source, "pub fn record_digest(")
        .ok_or("record.rs no longer declares record_digest")?;
    for field in &fields {
        assert!(
            digest.contains(&format!("self.{field}")),
            "record_digest does not cover {field}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The audit-namespace discriminator's own source half
// ---------------------------------------------------------------------------

#[test]
fn the_consumption_join_is_the_only_key_into_the_audit() -> Result<(), Box<dyn Error>> {
    let schema = source_of("crates/policy/src/schema.sql")?;

    // `egress_audit.grant_id` still has no foreign key, and that is deliberate:
    // `P2-G7` removed one so process-activity rows could be written at all. The
    // claim this file holds is not that the column is unambiguous -- it is not
    // -- but that the join through `egress_consumption` does not read it
    // ambiguously.
    let audit_start = schema
        .find("CREATE TABLE IF NOT EXISTS egress_audit")
        .ok_or("egress_audit is gone")?;
    let audit_end = schema[audit_start..]
        .find(") STRICT;")
        .map(|offset| audit_start + offset)
        .ok_or("egress_audit is unterminated")?;
    let audit = collapse(&schema[audit_start..audit_end]);
    assert!(
        audit.contains("UNIQUE (audit_seq, grant_id)"),
        "egress_audit lost the UNIQUE that makes the composite foreign key declarable"
    );

    let consumption_start = schema
        .find("CREATE TABLE IF NOT EXISTS egress_consumption")
        .ok_or("egress_consumption is gone")?;
    let consumption_end = schema[consumption_start..]
        .find(") STRICT;")
        .map(|offset| consumption_start + offset + ") STRICT;".len())
        .ok_or("egress_consumption is unterminated")?;
    assert_eq!(
        collapse(&schema[consumption_start..consumption_end]),
        WHOLE_CONSUMPTION_TABLE,
        "WHOLE_CONSUMPTION_TABLE changed; update the pin in this commit"
    );

    // A pin fixes the table and says nothing about whether a consumption row is
    // written, or which audit row it names. So the one write site is counted,
    // and it must take its sequence from the allow audit rather than from a
    // decision row: `execute` writes both in one transaction, and the row this
    // join reaches has to be the transmission.
    // Read out of the raw source rather than the stripped code, because the
    // statement is a string literal and `rust_code_only` removes those.
    let broker = source_of("crates/policy/src/lib.rs")?;
    assert_eq!(
        broker.matches("INSERT INTO egress_consumption").count(),
        1,
        "egress_consumption has more than one write site; each one has to be          reviewed for which audit row it names"
    );
    let write_site = broker
        .find("INSERT INTO egress_consumption")
        .ok_or("the consumption write site is gone")?;
    let preceding = &broker[..write_site];
    let binding = preceding
        .rfind("let consumption_audit_seq = insert_runtime_audit(")
        .ok_or("the consumption no longer takes its sequence from insert_runtime_audit")?;
    assert!(
        collapse(&preceding[binding..]).contains("Decision::Allow"),
        "the consumption names an audit row that is not the allow transmission"
    );

    // And the reconciliation reads that projection rather than the raw column.
    let reconcile = source_of("crates/model-run/src/reconcile.rs")?;
    let reconcile_code = rust_code_only(&reconcile);
    assert_eq!(
        identifier_occurrences(&reconcile_code, "ConsumptionRow"),
        3,
        "ConsumptionRow is named in the import and in the two functions that take          it; a fourth or a missing site changes what the reconciliation keys on"
    );
    assert!(
        !reconcile_code.contains("row.grant_id"),
        "the reconciliation reads egress_audit.grant_id directly, which is the          polymorphic column the join exists to avoid"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The evasions this file's own checks are required to catch
// ---------------------------------------------------------------------------

#[test]
fn the_checks_catch_the_evasions_they_are_written_against() -> Result<(), Box<dyn Error>> {
    // Applied to the checks inside this test, in the shape
    // `no_float_reaches_the_gpa_path` uses for its five float evasions: a rule
    // nobody has watched refuse anything is a rule nobody has tested.
    let ordering_evasions = [
        "impl PartialOrd for RawScore {",
        "impl Ord for RawScore {",
        "impl core::cmp::PartialOrd<RawScore> for RawScore {",
        "impl<T> Rank<T> for RawScore {",
        "impl Deref for RawScore {",
    ];
    for evasion in ordering_evasions {
        let headers = impl_headers(&rust_code_only(evasion), "RawScore");
        assert_eq!(headers.len(), 1, "{evasion} was not seen as an impl block");
        assert!(
            !RAW_SCORE_IMPLS.contains(&headers[0].as_str()),
            "{evasion} is already on the reviewed list, so injecting it proves nothing"
        );
    }

    // The number-handing rule reads the return type as identifiers, so a type
    // that is a number under another spelling is still refused, and one that is
    // not a number is still allowed.
    let refused = "pub fn units(&self) -> u32 { self.units }";
    let allowed = "pub fn provider(&self) -> &ProviderId { &self.provider }";
    for (source, expected) in [(refused, true), (allowed, false)] {
        let head = source.split("pub fn ").nth(1).unwrap_or_default();
        let head = head.split('{').next().unwrap_or_default();
        let returned = head
            .split_once("->")
            .map(|(_, rest)| rest)
            .unwrap_or_default();
        let words = returned
            .split(|character: char| !character.is_alphanumeric() && character != '_')
            .collect::<Vec<_>>();
        assert_eq!(
            words.contains(&"u32"),
            expected,
            "{source} was read wrongly"
        );
    }

    // The raw-string lexer stays synchronised, which is `P2-G4`'s `I3`.
    let with_raw = "let banner = r#\"a \" quote\"#; impl Ord for RawScore {}";
    assert_eq!(
        impl_headers(&rust_code_only(with_raw), "RawScore").len(),
        1,
        "a raw string desynchronised the lexer, so code after it was read as a literal"
    );

    // And the identifier counter counts names, not spellings.
    assert_eq!(
        identifier_occurrences("a.expose(); Untrusted::expose(a)", "expose"),
        2
    );
    assert_eq!(identifier_occurrences("exposed exposure", "expose"), 0);
    Ok(())
}

#[test]
fn the_migration_is_applied_and_guarded() -> Result<(), Box<dyn Error>> {
    // Migration 0007's tables are canonical history on both enforcement layers.
    // The trigger pairs are in the migration; the authorizer list is in
    // `academic-store`. A table in only one of them has a single point of
    // enforcement, so the two are compared here as well as in
    // `authorizer_covers_every_canonical_table`.
    let migration = source_of("migrations/store/0007_phase2_model_run_provenance.sql")?;
    let authorizer = source_of("crates/store/src/authorizer.rs")?;
    let mut tables = BTreeMap::new();
    for line in migration.lines() {
        if let Some(rest) = line.trim().strip_prefix("CREATE TABLE ") {
            let name = rest.split_whitespace().next().unwrap_or_default();
            tables.insert(name.to_owned(), ());
        }
    }
    assert!(
        tables.len() >= 4,
        "migration 0007 creates {} tables; the parser stopped reading",
        tables.len()
    );
    for table in tables.keys() {
        assert!(
            migration.contains(&format!("guard_{table}_update")),
            "{table} has no append-only UPDATE trigger"
        );
        assert!(
            migration.contains(&format!("guard_{table}_delete")),
            "{table} has no append-only DELETE trigger"
        );
        assert!(
            authorizer.contains(&format!("\"{table}\"")),
            "{table} is not in CANONICAL_TABLES, so it has one enforcement layer"
        );
    }

    // And it is in the lane's migration set, so an encrypted profile carries
    // these tables from creation rather than acquiring them later.
    let migration_rs = source_of("crates/store/src/migration.rs")?;
    assert!(migration_rs.contains("MIGRATION_0007_SQL"));
    assert_eq!(
        identifier_occurrences(&rust_code_only(&migration_rs), "MIGRATION_0007_SQL"),
        3,
        "MIGRATION_0007_SQL is declared, listed in STORE_MIGRATION_SQL, and applied \
         pre-listen; a fourth or a missing site changes which profiles carry it"
    );
    Ok(())
}
