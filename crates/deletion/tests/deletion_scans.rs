//! Whole-set inventories over this crate's own product source.
//!
//! Every guard here is a **set comparison in both directions**, never a list of
//! forbidden names. This run measured three bypass classes that a name list
//! does not catch, and each one has a guard below:
//!
//! * `P2-Y3` — a `From`/`Into` conversion escapes every public-function sweep,
//!   because it is neither a `pub fn` nor a method with a name anybody listed.
//!   `the_impl_blocks_naming_the_gate_types_are_these` pins the whole `impl`
//!   header set instead.
//! * `P2-N7` — a public function that spells none of the names on a list still
//!   hands the value out. `the_public_signature_set_is_this` inventories every
//!   public signature and compares the whole set.
//! * `P2-N8` — a type name shared with another crate pollutes that crate's
//!   guards. `no_type_this_crate_declares_is_named_by_another_crate` measures
//!   it, and every scan here reads only this crate's own files.
//!
//! `P2-U8` measured that the shared `tools/secret-debug-policy.test.mjs`
//! matches field *names*, and `T197` measured that its text classification
//! reaches only the two crates `TEXT_CLASSIFIED_CRATES` names. Neither reaches
//! this crate, so the field guard below reads **declared types** and is this
//! crate's own.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

const CRATE_SOURCE: &str = "src";
const WORKSPACE_CRATES: &str = "../";

// ---------------------------------------------------------------------------
// every_field_type_in_this_crate_is_reviewed
// ---------------------------------------------------------------------------

/// The declared type of every named field, compared as a whole set.
///
/// A deletion flow sits beside the two places this run measured `Debug` leaks:
/// the wrapped VMK and the OS-keystore blob in `academic-crypto`, the backup
/// root in `academic-recovery`, and the recipient CBOR a restore recovers a VMK
/// from in `academic-portability`. Eight leaks were measured in this run and
/// four were in those three crates.
///
/// So this crate closes it at its own boundary rather than trusting the shared
/// tool: every named field of every struct and enum in `src` is collected with
/// its **declared type**, and the set of declared types is compared against
/// `REVIEWED_FIELD_TYPES` in both directions. A field holding `Vec<u8>` fails as
/// an unreviewed type whatever it is called — the exact shape `P2-U8` showed
/// passing the shared tool under the name `excerpt`. A reviewed type that is no
/// longer used fails too, so the list cannot rot into a permission slip.
///
/// The extractor is checked against four shapes before it is trusted, including
/// the one `P2-R2`'s first repair missed.
#[test]
fn every_field_type_in_this_crate_is_reviewed() -> TestResult {
    let mut declared: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for (name, ty) in named_fields(&code) {
            declared.entry(ty).or_default().insert(name);
        }
    }
    let found: BTreeSet<String> = declared.keys().cloned().collect();
    let reviewed: BTreeSet<String> = REVIEWED_FIELD_TYPES
        .iter()
        .map(|ty| (*ty).to_owned())
        .collect();
    assert_eq!(
        found, reviewed,
        "the set of declared field types in this crate changed"
    );

    // The extractor is not vacuous: it reads these four shapes, and the last is
    // the one a line-prefix match misses.
    let samples = [
        ("struct A { pub x: Vec<u8>, }", vec![("x", "Vec<u8>")]),
        ("enum B { V { y: [u8; 32] }, }", vec![("y", "[u8; 32]")]),
        (
            "struct C { z: BTreeMap<DeletionTarget, PathBuf>, }",
            vec![("z", "BTreeMap<DeletionTarget, PathBuf>")],
        ),
        (
            "struct D {\n    w:\n        Option<TimestampMillis>,\n}",
            vec![("w", "Option<TimestampMillis>")],
        ),
    ];
    for (code, expected) in samples {
        let got: Vec<(String, String)> = named_fields(code);
        let expected: Vec<(String, String)> = expected
            .into_iter()
            .map(|(name, ty)| (name.to_owned(), ty.to_owned()))
            .collect();
        assert_eq!(got, expected, "the field extractor missed {code:?}");
    }

    // And no reviewed type is byte-capable, which is the claim the set makes.
    for ty in REVIEWED_FIELD_TYPES {
        for forbidden in [
            "u8]",
            "[u8",
            "Vec<u8",
            "&[u8",
            "Bytes",
            "Payload",
            "Plaintext",
        ] {
            assert!(
                !ty.contains(forbidden) || ty.eq(&"[u8; 32]"),
                "{ty} is on the reviewed list and can hold {forbidden}"
            );
        }
    }
    // `[u8; 32]` is the one array on the list and it is a locator: a fixed-width
    // opaque name that is already the filename of the object it points at. It
    // is not a payload, and it is the only exception, stated here rather than
    // left to a reader to notice.
    assert_eq!(
        declared
            .get("[u8; 32]")
            .map(|names| names.iter().cloned().collect::<Vec<_>>()),
        Some(vec!["locator".to_owned()]),
        "an array field other than the vault locator appeared"
    );

    // `String` is the one type on the reviewed list that a caller could put
    // anything into, so it is closed at the field *name* as well — a whole set,
    // in both directions, with a reason for each:
    //
    // | field | what it holds | why it is not a payload |
    // |---|---|---|
    // | `action_id` | the retention action's 32-hex identity | it is the identity `P2-K5`'s journal and tombstone already carry in the clear |
    // | `detail` | the executor's or the registry's own sentence about a refusal | it is written by this workspace's own code about a failure, and a `PARTIAL` result that could not say why would be the "mostly deleted" result `P2-K5` refuses |
    // | `reason` | why a derivative class holds nothing, or could not be answered for | the same, one layer up: `P2-K5` refuses an empty class that cannot say why it is empty |
    //
    // A fourth name is a new decision and fails here.
    assert_eq!(
        declared.get("String"),
        Some(
            &["action_id", "detail", "reason"]
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<String>>()
        ),
        "a String field with a name nobody reviewed appeared"
    );
    Ok(())
}

/// Every declared field type in `crates/deletion/src`, reviewed one by one.
const REVIEWED_FIELD_TYPES: &[&str] = &[
    "ArtifactId",
    "BTreeMap<(DeletionTarget, EgressDecisionId), ProviderErasureEntry>",
    "BTreeMap<DeletionTarget, ArtifactDescriptor>",
    "BTreeMap<DeletionTarget, ContentDigest>",
    "BTreeMap<DeletionTarget, PathBuf>",
    "BTreeMap<DerivativeClass, String>",
    "BTreeMap<DerivativeClass, Vec<DeletionTarget>>",
    "Box<DeletionTarget>",
    "Box<ProtectionReason>",
    "ClassResolution",
    "ContentDigest",
    "DeletionDryRun",
    "DeletionImpactPreview",
    "DeletionPaths",
    "DeletionTarget",
    "DerivativeClass",
    "EgressDecisionId",
    "ExposureScope",
    "Option<DeletionTarget>",
    "Option<IncidentClosure>",
    "Option<TimestampMillis>",
    "ProtectionDecision",
    "ProtectionPolicyKind",
    "ProviderErasureLog",
    "ProviderErasureRequest",
    "ReceiptState",
    "RetentionOutcome",
    "String",
    "TimestampMillis",
    "UnresolvedReason",
    "UserDecision",
    "Vec<(DerivativeClass, DeletionTarget)>",
    "Vec<AffectedProjection>",
    "Vec<BackupTombstone>",
    "Vec<ContentDigest>",
    "Vec<CorrectionChoice>",
    "Vec<DeletionTarget>",
    "Vec<DryRunNode>",
    "Vec<PathBuf>",
    "Vec<RecoveryStep>",
    "Vec<UnresolvedTarget>",
    "[RecoveryStep; 4]",
    "[u8; 32]",
    "bool",
    "u32",
    "u64",
    "usize",
];

// ---------------------------------------------------------------------------
// the_public_signature_set_is_this
// ---------------------------------------------------------------------------

/// Every public signature, compared as a whole set.
///
/// `P2-N7` found five public functions that handed a value out while spelling
/// none of the names the guard listed. A list of names cannot catch the
/// function nobody predicted, so this collects **every** `pub fn` and
/// `pub const fn` in `src`, in file order, and compares the whole set.
///
/// Two claims rest on it, and both are stated as properties of the set rather
/// than as a search for a name:
///
/// 1. **No signature closes a leak incident from a correction.** No public
///    signature mentions `CorrectionRecord`, `CorrectionOutcome` or
///    `CorrectionChoice` *and* `IncidentClosure` or `LeakIncidentState`.
///    `record_claim_correction` takes a `CorrectionRecord` and returns `()`,
///    which is the one that must exist and the reason a name list would pass
///    the wrong thing.
/// 2. **No signature is byte-capable.** No parameter or return type in the set
///    can hold a payload byte.
#[test]
fn the_public_signature_set_is_this() -> TestResult {
    let mut signatures = Vec::new();
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let file = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for signature in public_signatures(&code) {
            signatures.push(format!("{file}: {signature}"));
        }
    }
    signatures.sort();
    let expected: Vec<String> = PUBLIC_SIGNATURES.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(
        signatures, expected,
        "the public signature set of this crate changed"
    );

    let correction_words = ["CorrectionRecord", "CorrectionOutcome", "CorrectionChoice"];
    let closure_words = ["IncidentClosure", "LeakIncidentState"];
    for signature in &signatures {
        let names_correction = correction_words.iter().any(|word| signature.contains(word));
        let names_closure = closure_words.iter().any(|word| signature.contains(word));
        assert!(
            !(names_correction && names_closure),
            "a public signature turns a claim correction into an incident state: {signature}"
        );
        // The last two are path-qualified: this crate has no product edge to
        // the crates that own them, so a bare word would also match its own
        // `DeletionImpactPreview`, which is a count of projections and not a
        // byte.
        for byte_capable in [
            "Vec<u8",
            "&[u8]",
            "&mut [u8]",
            "::StagedPayload",
            "::Preview",
        ] {
            assert!(
                !signature.contains(byte_capable),
                "a public signature can carry a payload byte: {signature}"
            );
        }
    }

    // The extractor is not vacuous: it reads a multi-line signature and a
    // `pub const fn`, and it stops at the body brace.
    let sample = "pub const fn a(\n    b: u8,\n) -> u8 {\n    b\n}\npub fn c() {}\n";
    assert_eq!(
        public_signatures(sample),
        vec![
            "pub const fn a( b: u8, ) -> u8".to_owned(),
            "pub fn c()".to_owned()
        ]
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// the_impl_blocks_naming_the_gate_types_are_these
// ---------------------------------------------------------------------------

/// The whole `impl` header set for the four types that gate something.
///
/// `P2-Y3` measured that a `From`/`Into` conversion escapes every public-`fn`
/// sweep. So the four types whose absence of a constructor *is* the contract —
/// the confirmation, the incident closure, the protection decision and the
/// deletion target — have their whole `impl` header set pinned. A
/// `impl From<CorrectionRecord> for IncidentClosure` is a new header and fails
/// here even though it adds no `pub fn` and spells no forbidden name.
#[test]
fn the_impl_blocks_naming_the_gate_types_are_these() -> TestResult {
    let gates = [
        "DeletionConfirmation",
        "IncidentClosure",
        "ProtectionDecision",
        "DeletionTarget",
    ];
    let mut headers: BTreeSet<String> = BTreeSet::new();
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for header in impl_headers(&code) {
            if gates.iter().any(|gate| header.contains(gate)) {
                headers.insert(header);
            }
        }
    }
    assert_eq!(
        headers,
        GATE_IMPL_HEADERS.iter().map(|h| (*h).to_owned()).collect(),
        "the impl blocks naming this crate's gate types changed"
    );
    // None of them is a conversion, which is the class `P2-Y3` measured.
    for header in &headers {
        for conversion in ["From<", "Into<", "TryFrom<", "Deref", "AsRef", "Default"] {
            assert!(
                !header.contains(conversion),
                "a gate type gained a {conversion} implementation: {header}"
            );
        }
    }
    // The extractor reads a generic header and a trait header.
    assert_eq!(
        impl_headers("impl<'a, T: X> Foo for Bar<'a, T> {\n}\nimpl Baz {\n}\n"),
        vec![
            "impl<'a, T: X> Foo for Bar<'a, T>".to_owned(),
            "impl Baz".to_owned()
        ]
    );
    Ok(())
}

/// Every `impl` header in `src` naming one of the four gate types.
const GATE_IMPL_HEADERS: &[&str] = &[
    "impl DeletionConfirmation",
    "impl DeletionTarget",
    "impl IncidentClosure",
    "impl ProtectionDecision",
];

// ---------------------------------------------------------------------------
// no_type_this_crate_declares_is_named_by_another_crate
// ---------------------------------------------------------------------------

/// This crate's type names are its own, workspace-wide.
///
/// `P2-N8` measured a guard in `academic-review` reporting five files belonging
/// to other crates, because `DimensionReading` was declared in two places and
/// the guard searched by name. Every scan in this file reads only
/// `crates/deletion/src`, so it cannot be polluted that way — and this asserts
/// the other direction, so a future guard anywhere that searches for one of
/// these names finds only this crate.
#[test]
fn no_type_this_crate_declares_is_named_by_another_crate() -> TestResult {
    let mine = declared_types_under(Path::new(CRATE_SOURCE))?;
    assert!(
        mine.len() > 20,
        "the type extractor found only {} declarations in this crate",
        mine.len()
    );
    let mut collisions: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in fs::read_dir(WORKSPACE_CRATES)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "deletion" || !entry.path().join("src").is_dir() {
            continue;
        }
        for declared in declared_types_under(&entry.path().join("src"))? {
            if mine.contains(&declared) {
                collisions.entry(declared).or_default().insert(name.clone());
            }
        }
    }
    assert!(
        collisions.is_empty(),
        "these type names are declared here and elsewhere: {collisions:?}"
    );

    // The cross-crate extractor is not vacuous: a name that really is declared
    // in another crate is found by it.
    let elsewhere = declared_types_under(Path::new("../retention/src"))?;
    assert!(
        elsewhere.contains("DeletionPlan"),
        "the cross-crate extractor did not find a type that is there"
    );
    assert!(
        !mine.contains("DeletionPlan"),
        "this crate declares a type P2-K5 already declares"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// this_crate_reads_no_clock_and_no_environment
// ---------------------------------------------------------------------------

/// Every instant this crate compares is an argument, and no product file reads
/// the environment.
///
/// A deletion that read a wall clock would not replay to the same bytes, and a
/// deletion flow with an environment switch would be the quiet flag t068
/// section 3.1 forbids. Both are checked over the whole product source rather
/// than over a list of files.
#[test]
fn this_crate_reads_no_clock_and_no_environment() -> TestResult {
    let mut checked = 0_usize;
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        checked += 1;
        for forbidden in [
            "SystemTime",
            "Instant::now",
            "std::env",
            "env::var",
            "var_os",
            "debug_assertions",
            "process::abort",
            "process::exit",
        ] {
            assert!(
                !code.contains(forbidden),
                "{} names {forbidden}",
                path.display()
            );
        }
    }
    assert!(checked >= 10, "only {checked} product files were scanned");
    Ok(())
}

// ---------------------------------------------------------------------------
// product_source_is_under_src_and_declares_no_path_module
// ---------------------------------------------------------------------------

/// Product code lives under `src` and no module is redirected out of it.
///
/// A `#[path]` attribute would let a product file live where none of the scans
/// above walk, which makes every whole-set claim in this file conditional on
/// somebody remembering not to use one.
#[test]
fn product_source_is_under_src_and_declares_no_path_module() -> TestResult {
    let sources = product_sources()?;
    for path in &sources {
        assert!(
            path.starts_with(CRATE_SOURCE),
            "{} is product source outside src",
            path.display()
        );
        let code = fs::read_to_string(path)?;
        assert!(
            !code.contains("#[path"),
            "{} redirects a module out of src",
            path.display()
        );
    }
    let names: BTreeSet<String> = sources
        .iter()
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect();
    assert_eq!(
        names,
        [
            "confirm.rs",
            "dry_run.rs",
            "engine.rs",
            "error.rs",
            "execute.rs",
            "executors.rs",
            "incident.rs",
            "lib.rs",
            "preview.rs",
            "protection.rs",
            "provider.rs",
            "target.rs",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        "the set of product source files changed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// extractors
// ---------------------------------------------------------------------------

fn product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    collect_rust(Path::new(CRATE_SOURCE), &mut found)?;
    found.sort();
    Ok(found)
}

fn collect_rust(root: &Path, into: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust(&path, into)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            into.push(path);
        }
    }
    Ok(())
}

/// Removes line comments and string literals, so a scan reads code.
fn strip_non_code(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    for line in code.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            out.push('\n');
            continue;
        }
        let mut in_string = false;
        let mut escaped = false;
        for character in line.chars() {
            match character {
                '\\' if in_string => escaped = !escaped,
                '"' if !escaped => {
                    in_string = !in_string;
                    out.push('"');
                }
                _ if in_string => escaped = false,
                _ => out.push(character),
            }
        }
        out.push('\n');
    }
    out
}

/// Every named field, as `(name, declared type)`, in declaration order.
///
/// It reads **declaration bodies only** — the brace-balanced body that follows
/// a `struct X` or `enum X` header — and descends into an enum's struct
/// variants. An expression that looks like a field (a struct literal, a match
/// arm) is not a declaration and is not counted, which is what makes the set
/// below the set of types this crate can actually hold.
fn named_fields(code: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for body in declaration_bodies(code) {
        collect_fields(&body, &mut found);
    }
    found
}

/// The brace-balanced body of every `struct` or `enum` declaration.
fn declaration_bodies(code: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    let flat = code.replace('\n', " ");
    let bytes: Vec<char> = flat.chars().collect();
    let mut index = 0_usize;
    while index < bytes.len() {
        let rest: String = bytes[index..].iter().collect();
        let Some(offset) = next_declaration(&rest) else {
            break;
        };
        let after = index + offset;
        let tail: String = bytes[after..].iter().collect();
        let Some(open) = tail.find('{') else {
            break;
        };
        // A tuple struct or a unit struct ends before any brace of its own.
        if tail[..open].contains(';') {
            index = after + 1;
            continue;
        }
        let mut depth = 0_usize;
        let mut body = String::new();
        let mut consumed = 0_usize;
        for character in tail[open..].chars() {
            consumed += character.len_utf8();
            match character {
                '{' => {
                    depth += 1;
                    if depth == 1 {
                        continue;
                    }
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            body.push(character);
        }
        bodies.push(body);
        index = after + open + consumed.max(1);
    }
    bodies
}

/// The offset just past the next `struct` or `enum` keyword, if any.
fn next_declaration(code: &str) -> Option<usize> {
    let mut best: Option<usize> = None;
    for keyword in [" struct ", " enum ", "struct ", "enum "] {
        if let Some(at) = code.find(keyword) {
            let candidate = at + keyword.len();
            best = Some(best.map_or(candidate, |current: usize| current.min(candidate)));
        }
    }
    best
}

/// Collects `name: type` pairs from a declaration body, descending into enum
/// struct variants.
fn collect_fields(body: &str, into: &mut Vec<(String, String)>) {
    for field in split_fields(body) {
        if let Some(open) = field.find('{') {
            // An enum struct variant: its own body is a set of fields.
            let inner: String = field[open + 1..].trim_end_matches('}').to_owned();
            collect_fields(&inner, into);
            continue;
        }
        let Some((name, ty)) = field.split_once(':') else {
            continue;
        };
        let name = name.trim().trim_start_matches("pub ").trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        let ty = collapse(ty.trim());
        if ty.is_empty() || ty.starts_with('&') {
            continue;
        }
        into.push((name.to_owned(), ty));
    }
}

/// Splits a struct or enum body on top-level commas.
fn split_fields(body: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for character in body.chars() {
        match character {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                fields.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    fields.push(current);
    fields
}

/// Every `pub fn` and `pub const fn` head, with its whitespace collapsed.
fn public_signatures(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let flat: Vec<&str> = code.lines().collect();
    for (index, line) in flat.iter().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("pub fn ") || trimmed.starts_with("pub const fn ")) {
            continue;
        }
        let mut signature = String::from(trimmed);
        let mut cursor = index;
        while !signature.contains('{') && !signature.contains(';') && cursor + 1 < flat.len() {
            cursor += 1;
            signature.push(' ');
            signature.push_str(flat[cursor].trim());
        }
        // A signature ends at its body brace when it has one, and at the
        // semicolon of a trait declaration when it has not. Splitting on the
        // semicolon first truncates `locator: [u8; 32]` mid-parameter, which
        // is how an array-typed argument disappears from an inventory.
        let head = signature.split_once('{').map_or_else(
            || {
                signature
                    .split_once(';')
                    .map_or(signature.as_str(), |(head, _)| head)
            },
            |(head, _)| head,
        );
        found.push(collapse(head));
    }
    found
}

/// Every `impl` header, with its whitespace collapsed.
fn impl_headers(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let flat: Vec<&str> = code.lines().collect();
    for (index, line) in flat.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("impl") {
            continue;
        }
        let mut header = String::from(trimmed);
        let mut cursor = index;
        while !header.contains('{') && cursor + 1 < flat.len() {
            cursor += 1;
            header.push(' ');
            header.push_str(flat[cursor].trim());
        }
        let head = header
            .split_once('{')
            .map_or(header.as_str(), |(head, _)| head);
        found.push(collapse(head));
    }
    found
}

/// Every type name a crate's source declares.
fn declared_types_under(root: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut found = BTreeSet::new();
    let mut sources = Vec::new();
    collect_rust(root, &mut sources)?;
    for path in sources {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for line in code.lines() {
            let trimmed = line.trim();
            for prefix in [
                "pub struct ",
                "pub enum ",
                "pub trait ",
                "struct ",
                "enum ",
                "trait ",
            ] {
                if let Some(rest) = trimmed.strip_prefix(prefix) {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        found.insert(name);
                    }
                    break;
                }
            }
        }
    }
    Ok(found)
}

fn collapse(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every public signature in `crates/deletion/src`, as the extractor reads it.
const PUBLIC_SIGNATURES: &[&str] = &[
    "confirm.rs: pub const fn confirmed_at(&self) -> TimestampMillis",
    "confirm.rs: pub const fn decision(&self) -> &UserDecision",
    "confirm.rs: pub const fn preview(&self) -> &DeletionImpactPreview",
    "confirm.rs: pub const fn shown_digest(&self) -> ContentDigest",
    "confirm.rs: pub fn given( preview: DeletionImpactPreview, actor: &Actor, shown: ContentDigest, confirmed_at: TimestampMillis, ) -> Result<Self, DeletionFlowError>",
    "dry_run.rs: pub const fn class(&self) -> DerivativeClass",
    "dry_run.rs: pub const fn is_unresolved(&self) -> bool",
    "dry_run.rs: pub const fn protection(&self) -> &ProtectionDecision",
    "dry_run.rs: pub const fn resolution(&self) -> &ClassResolution",
    "dry_run.rs: pub const fn subject(&self) -> &DeletionTarget",
    "dry_run.rs: pub fn enumerated_classes(&self) -> Vec<DerivativeClass>",
    "dry_run.rs: pub fn nodes(&self) -> &[DryRunNode]",
    "dry_run.rs: pub fn of<I, P>(subject: DeletionTarget, index: &I, protection: &P) -> Self where I: DerivativeIndex + ?Sized, P: ProtectionRegistry + ?Sized,",
    "dry_run.rs: pub fn plan(&self) -> academic_retention::DeletionPlan",
    "dry_run.rs: pub fn reached(&self) -> Vec<DeletionTarget>",
    "dry_run.rs: pub fn targets(&self) -> &[DeletionTarget]",
    "dry_run.rs: pub fn unresolved_classes(&self) -> Vec<DerivativeClass>",
    "engine.rs: pub const fn new() -> Self",
    "engine.rs: pub const fn over( vault: &'a EncryptedVault, descriptors: BTreeMap<DeletionTarget, ArtifactDescriptor>, ) -> Self",
    "engine.rs: pub fn empty(&mut self, class: DerivativeClass, reason: String)",
    "engine.rs: pub fn holding(&mut self, class: DerivativeClass, targets: Vec<DeletionTarget>)",
    "execute.rs: pub const fn drifted(&self) -> bool",
    "execute.rs: pub const fn outcome(&self) -> &RetentionOutcome",
    "execute.rs: pub const fn outcome_word(&self) -> &'static str",
    "execute.rs: pub const fn provider(&self) -> &ProviderErasureLog",
    "execute.rs: pub const fn subject(&self) -> &DeletionTarget",
    "execute.rs: pub fn execute_deletion<E: TargetExecutor + ?Sized + std::fmt::Debug>( journal: &mut AppendOnlyJournal, action_id: ActionId, confirmation: &DeletionConfirmation, executor: &mut E, provider: ProviderErasureLog, ) -> Result<ArtifactDeletionReceipt, DeletionFlowError>",
    "execute.rs: pub fn failures(&self) -> &[UnresolvedTarget]",
    "execute.rs: pub fn is_fully_erased(&self) -> bool",
    "execute.rs: pub fn over(executor: &'e mut E, confirmation: &DeletionConfirmation) -> Self",
    "execute.rs: pub fn report_rows(&self) -> Vec<String>",
    "execute.rs: pub fn to_row(&self) -> String",
    "execute.rs: pub fn unresolved(&self) -> &[UnresolvedTarget]",
    "execute.rs: pub fn unresolved_rows(&self) -> Vec<String>",
    "executors.rs: pub const fn new( shredder: &'s mut S, paths: DeletionPaths, action_id: String, shredded_at_ms: u64, ) -> Self",
    "executors.rs: pub const fn new() -> Self",
    "executors.rs: pub fn backup_at(&mut self, target: DeletionTarget, path: PathBuf)",
    "executors.rs: pub fn backup_root(&self, target: &DeletionTarget) -> Option<&PathBuf>",
    "executors.rs: pub fn purge_at(&mut self, target: DeletionTarget, path: PathBuf)",
    "executors.rs: pub fn purge_path(&self, target: &DeletionTarget) -> Option<&PathBuf>",
    "executors.rs: pub fn purged(&self) -> &[PathBuf]",
    "executors.rs: pub fn tombstones(&self) -> &[BackupTombstone]",
    "incident.rs: pub const fn as_str(self) -> &'static str",
    "incident.rs: pub const fn as_str(self) -> &'static str",
    "incident.rs: pub const fn closed_at(&self) -> TimestampMillis",
    "incident.rs: pub const fn closure(&self) -> Option<&IncidentClosure>",
    "incident.rs: pub const fn destination(&self) -> ContentDigest",
    "incident.rs: pub const fn exposed_bytes(&self) -> u64",
    "incident.rs: pub const fn new( exposed_bytes: u64, source: ArtifactId, destination: ContentDigest, provider_retention_days: u32, ) -> Self",
    "incident.rs: pub const fn opened_at(&self) -> TimestampMillis",
    "incident.rs: pub const fn provider_retention_days(&self) -> u32",
    "incident.rs: pub const fn reported(scope: ExposureScope, opened_at: TimestampMillis) -> Self",
    "incident.rs: pub const fn scope(&self) -> ExposureScope",
    "incident.rs: pub const fn scope(&self) -> ExposureScope",
    "incident.rs: pub const fn source(&self) -> ArtifactId",
    "incident.rs: pub const fn spec_words(self) -> &'static str",
    "incident.rs: pub const fn steps(&self) -> &[RecoveryStep; 4]",
    "incident.rs: pub fn claim_corrections(&self) -> &[CorrectionChoice]",
    "incident.rs: pub fn close(&mut self, closed_at: TimestampMillis) -> Result<&IncidentClosure, IncidentError>",
    "incident.rs: pub fn missing_steps(&self) -> Vec<RecoveryStep>",
    "incident.rs: pub fn record_claim_correction(&mut self, record: &CorrectionRecord)",
    "incident.rs: pub fn record_recovery(&mut self, step: RecoveryStep)",
    "incident.rs: pub fn recorded_steps(&self) -> &[RecoveryStep]",
    "incident.rs: pub fn state(&self) -> LeakIncidentState",
    "preview.rs: pub const fn digest(&self) -> ContentDigest",
    "preview.rs: pub const fn dry_run(&self) -> &DeletionDryRun",
    "preview.rs: pub const fn new() -> Self",
    "preview.rs: pub const fn previewed_at(&self) -> u64",
    "preview.rs: pub fn cite(&mut self, target: DeletionTarget, digest: ContentDigest)",
    "preview.rs: pub fn digest_of(&self, target: &DeletionTarget) -> Option<ContentDigest>",
    "preview.rs: pub fn of( dry_run: DeletionDryRun, index: &EvidenceIndex, citations: &EvidenceCitations, previewed_at: u64, ) -> Result<Self, DeletionFlowError>",
    "preview.rs: pub fn partition_reconciles(&self, index: &EvidenceIndex) -> bool",
    "preview.rs: pub fn projections(&self) -> &[AffectedProjection]",
    "preview.rs: pub fn reached(&self) -> &[DeletionTarget]",
    "preview.rs: pub fn unreferenced(&self) -> &[ContentDigest]",
    "protection.rs: pub const fn as_str(self) -> &'static str",
    "protection.rs: pub const fn kind(&self) -> ProtectionPolicyKind",
    "protection.rs: pub const fn reason(&self) -> Option<&ProtectionReason>",
    "protection.rs: pub const fn revisit_at(&self) -> Option<TimestampMillis>",
    "protection.rs: pub const fn spec_section(self) -> &'static str",
    "protection.rs: pub const fn spec_words(self) -> &'static str",
    "protection.rs: pub fn detail(&self) -> &str",
    "protection.rs: pub fn to_row(&self) -> String",
    "protection.rs: pub fn under( kind: ProtectionPolicyKind, detail: String, revisit_at: Option<TimestampMillis>, ) -> Self",
    "provider.rs: pub const fn decision(&self) -> EgressDecisionId",
    "provider.rs: pub const fn is_settled(&self) -> bool",
    "provider.rs: pub const fn new( target: DeletionTarget, decision: EgressDecisionId, requested_at: TimestampMillis, ) -> Self",
    "provider.rs: pub const fn new() -> Self",
    "provider.rs: pub const fn receipt(&self) -> Option<&DeletionReceiptRef>",
    "provider.rs: pub const fn request(&self) -> &ProviderErasureRequest",
    "provider.rs: pub const fn requested_at(&self) -> TimestampMillis",
    "provider.rs: pub const fn state(&self) -> ReceiptState",
    "provider.rs: pub const fn target(&self) -> &DeletionTarget",
    "provider.rs: pub fn entries(&self) -> Vec<&ProviderErasureEntry>",
    "provider.rs: pub fn entry( &self, target: &DeletionTarget, decision: EgressDecisionId, ) -> Option<&ProviderErasureEntry>",
    "provider.rs: pub fn outstanding(&self) -> Vec<&ProviderErasureEntry>",
    "provider.rs: pub fn outstanding_rows(&self) -> Vec<String>",
    "provider.rs: pub fn record_receipt( &mut self, target: DeletionTarget, decision: EgressDecisionId, receipt: DeletionReceiptRef, ) -> Result<(), DeletionFlowError>",
    "provider.rs: pub fn request(&mut self, request: ProviderErasureRequest, state: ReceiptState)",
    "provider.rs: pub fn to_row(&self) -> String",
    "target.rs: pub const fn artifact(&self) -> ArtifactId",
    "target.rs: pub const fn locator(&self) -> &[u8; 32]",
    "target.rs: pub const fn new(artifact: ArtifactId, locator: [u8; 32]) -> Self",
    "target.rs: pub fn artifact_hex(&self) -> String",
    "target.rs: pub fn locator_hex(&self) -> String",
    "target.rs: pub fn to_row(&self) -> String",
];
