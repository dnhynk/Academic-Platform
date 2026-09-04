//! Whole-set inventories over this crate's own product source.
//!
//! Every guard here is a **set comparison in both directions**, never a list of
//! forbidden names. This run measured three bypass classes that a name list does
//! not catch, and each one has a guard below:
//!
//! * `P2-X5` and `P2-Y3` — a `trait impl` declares no `pub fn`, so a
//!   `impl From<X> for u32` walks past every public-signature sweep. `P2-X5`
//!   measured six of nineteen injections invisible to all twenty-three of its
//!   acceptance tests for exactly this reason.
//!   `the_impl_blocks_naming_the_gate_types_are_these` pins the whole `impl`
//!   header set, so a header is a new element whether or not it adds a function.
//! * `P2-N7` — a public function that spells none of the names on a list still
//!   hands the value out. `the_public_signature_set_is_this` inventories every
//!   public signature and compares the whole set, then states two properties of
//!   that set rather than searching it for names.
//! * `P2-N8` — a type name shared with another crate pollutes that crate's
//!   guards. `no_type_this_crate_declares_is_named_by_another_crate` measures
//!   it, and every scan here reads only this crate's own files.

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
// the_public_signature_set_is_this
// ---------------------------------------------------------------------------

/// Every public signature, compared as a whole set, plus two properties of it.
///
/// `P2-N7` found five public functions that handed a value out while spelling
/// none of the names the guard listed. A list of names cannot catch the function
/// nobody predicted, so this collects **every** `pub fn` and `pub const fn` in
/// `src`, in file order, and compares the whole set.
///
/// Two claims rest on it, and both are stated as properties of the set:
///
/// 1. **Nothing hands out a decision without an actor.** Every signature whose
///    return type can hold a [`academic_non_delegable::DecisionEvent`] — either
///    directly or inside an `AuthorizedCommand` — takes an `Actor` or an
///    `ActionCommand`, which owns one. A new `pub fn` returning a decision from
///    a subject digest alone is a new element **and** fails this property.
/// 2. **Nothing exposes the receipt's construction.** No signature returns a
///    `UserDecision`; the only way one exists here is inside a `DecisionEvent`
///    built by `DecisionEvent::recorded`.
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

    // `Self` in `decision.rs` is `DecisionEvent`, because that file declares
    // exactly one type. Without this the property below would read
    // `-> Result<Self, NonDelegableError>` as returning nothing of interest,
    // which is how a producer disappears from an inventory that looks complete.
    assert_eq!(
        declared_types_under(Path::new(CRATE_SOURCE))?
            .into_iter()
            .filter(|name| name == "DecisionEvent")
            .count(),
        1
    );
    let decision_file_types = declared_types_in(&fs::read_to_string(
        Path::new(CRATE_SOURCE).join("decision.rs"),
    )?);
    assert_eq!(
        decision_file_types,
        BTreeSet::from(["DecisionEvent".to_owned()]),
        "decision.rs declares more than one type, so `Self` there is ambiguous"
    );

    let mut decision_producers = Vec::new();
    for signature in &signatures {
        let (head, tail) = signature
            .split_once("->")
            .unwrap_or((signature.as_str(), ""));
        let returns_a_decision = tail.contains("DecisionEvent")
            || tail.contains("AuthorizedCommand")
            || (signature.starts_with("decision.rs:") && tail.contains("Self"));
        if returns_a_decision {
            decision_producers.push(signature.clone());
            assert!(
                head.contains("&Actor") || head.contains("ActionCommand"),
                "a public signature produces a decision without an actor: {signature}"
            );
        }
        assert!(
            !tail.contains("UserDecision") || head.contains("&self"),
            "a public signature mints a user decision receipt: {signature}"
        );
    }
    // The property is not vacuous: the producers exist and are these.
    assert_eq!(
        decision_producers,
        vec![
            "command.rs: pub fn authorise(command: ActionCommand) -> Result<AuthorizedCommand, NonDelegableError>".to_owned(),
            "decision.rs: pub fn recorded( action: NonDelegableAction, actor: &Actor, subject: ContentDigest, decided_at: TimestampMillis, ) -> Result<Self, NonDelegableError>".to_owned(),
        ],
        "the set of public functions that can produce a decision changed"
    );

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

/// The whole `impl` header set for the five types that gate something.
///
/// `P2-X5` measured that a `trait impl` declares no `pub fn` and so escapes
/// every public-function sweep; its still-open example is
/// `impl From<FieldCoverage> for u32` in another crate, which passes that
/// crate's whole suite. So the types whose absence of a constructor *is* the
/// contract have their whole `impl` header set pinned. An
/// `impl From<Actor> for DecisionEvent` is a new header and fails here even
/// though it adds no `pub fn` and spells no forbidden name.
#[test]
fn the_impl_blocks_naming_the_gate_types_are_these() -> TestResult {
    let gates = [
        "DecisionEvent",
        "AuthorizedCommand",
        "AuthorizedProposal",
        "ActionCommand",
        "NonDelegableAction",
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
    // None of them is a conversion or a default, which is the class `P2-X5` and
    // `P2-Y3` both measured. `Display` for the action is present and is listed
    // above; it carries a token, not a decision.
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

/// Every `impl` header in `src` naming one of the five gate types.
const GATE_IMPL_HEADERS: &[&str] = &[
    "impl ActionCommand",
    "impl DecisionEvent",
    "impl NonDelegableAction",
    "impl core::fmt::Display for NonDelegableAction",
    "impl AuthorizedProposal",
];

// ---------------------------------------------------------------------------
// every_field_type_in_this_crate_is_reviewed
// ---------------------------------------------------------------------------

/// The declared type of every named field, compared as a whole set.
///
/// A field is the other half of a signature: a type this crate can hold without
/// any function naming it. The set of **declared types** is compared against
/// `REVIEWED_FIELD_TYPES` in both directions, so a field holding a `String` or a
/// `bool` fails as an unreviewed type whatever it is called, and a reviewed type
/// that is no longer used fails too.
///
/// What the property says: no field in this crate can hold a payload byte, a
/// free-text answer, or an authority class. This crate carries decisions, not
/// content and not provenance.
#[test]
fn every_field_type_in_this_crate_is_reviewed() -> TestResult {
    let mut declared: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for (name, declared_type) in named_fields(&code) {
            declared.entry(declared_type).or_default().insert(name);
        }
    }
    let types: BTreeSet<String> = declared.keys().cloned().collect();
    assert_eq!(
        types,
        REVIEWED_FIELD_TYPES
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        "this crate's declared field types changed"
    );
    for declared_type in &types {
        for byte_capable in ["Vec<u8", "[u8", "String", "&str", "AuthorityClass"] {
            assert!(
                !declared_type.contains(byte_capable),
                "a field can hold {byte_capable}: {declared_type}"
            );
        }
    }
    // The extractor reads a struct field, an enum struct variant's field, and
    // skips a tuple struct and a match arm.
    assert_eq!(
        named_fields("struct A { x: u8, y: Vec<u8> }"),
        vec![
            ("x".to_owned(), "u8".to_owned()),
            ("y".to_owned(), "Vec<u8>".to_owned())
        ]
    );
    assert_eq!(
        named_fields("enum B { V { z: u16 } }"),
        vec![("z".to_owned(), "u16".to_owned())]
    );
    assert!(named_fields("struct C(u8);").is_empty());
    Ok(())
}

/// Every declared field type in `crates/non-delegable/src`.
const REVIEWED_FIELD_TYPES: &[&str] = &[
    "Action",
    "Actor",
    "CandidateGeneration",
    "ContentDigest",
    "NonDelegableAction",
    "TimestampMillis",
    "UserDecision",
    "&'static str",
];

// ---------------------------------------------------------------------------
// no_type_this_crate_declares_is_named_by_another_crate
// ---------------------------------------------------------------------------

/// This crate's type names are its own.
///
/// `P2-N8` measured a shared type name polluting another crate's guards. Every
/// scan in this file reads only `src`, so a collision would not make a scan here
/// wrong — but it would make a *reader* wrong about which crate a guard covers,
/// and it would make the workspace-wide inventories ambiguous.
#[test]
fn no_type_this_crate_declares_is_named_by_another_crate() -> TestResult {
    let mine = declared_types_under(Path::new(CRATE_SOURCE))?;
    assert!(
        mine.contains("DecisionEvent") && mine.contains("NonDelegableAction"),
        "the declared-type extractor read nothing"
    );
    let mut collisions: BTreeSet<String> = BTreeSet::new();
    for entry in fs::read_dir(WORKSPACE_CRATES)? {
        let crate_root = entry?.path();
        let source = crate_root.join("src");
        if !source.is_dir()
            || crate_root.file_name().and_then(|n| n.to_str()) == Some("non-delegable")
        {
            continue;
        }
        for name in declared_types_under(&source)? {
            if mine.contains(&name) {
                collisions.insert(name);
            }
        }
    }
    assert_eq!(
        collisions,
        BTreeSet::new(),
        "another crate declares a type this crate declares"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// this_crate_reads_no_clock_and_no_environment
// ---------------------------------------------------------------------------

/// A decision is timed by its caller, not by this crate.
///
/// Every instant here arrives as a `TimestampMillis` argument. A crate that read
/// a clock could record a decision at a time nobody supplied, and a crate that
/// read the environment could be told to skip a refusal.
#[test]
fn this_crate_reads_no_clock_and_no_environment() -> TestResult {
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for construct in [
            "SystemTime",
            "Instant::now",
            "std::env",
            "env::var",
            "option_env!",
            "std::fs",
            "std::process",
        ] {
            assert!(
                !code.contains(construct),
                "{} names {construct}",
                path.display()
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// every_test_this_crates_docs_cite_exists
// ---------------------------------------------------------------------------

/// Every test name a product doc comment cites is a test that exists.
///
/// This guard exists because this crate's own documentation had **three** of
/// them wrong: `src/lib.rs` and `src/command.rs` cited
/// `an_enrollment_confirmation_takes_no_actor`,
/// `an_authority_grant_takes_no_actor` and
/// `the_broker_cannot_tell_a_user_from_a_model_run`, none of which was ever
/// written — they were working names from a design the tests moved away from,
/// and nothing in the crate noticed. A sentence that cites evidence which does
/// not exist is the same defect as a guard that checks nothing: it reads as
/// proof and is not.
///
/// So every backticked lower-snake identifier of three or more words in a
/// product doc comment has to be a `fn` this crate's test targets declare, or be
/// listed in [`CITED_ELSEWHERE`] with the crate that owns it. A name that is
/// neither fails here.
#[test]
fn every_test_this_crates_docs_cite_exists() -> TestResult {
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for file in [
        "tests/non_delegable.rs",
        "tests/non_delegable_scans.rs",
        "tests/compile_fail.rs",
    ] {
        for line in fs::read_to_string(file)?.lines() {
            if let Some(rest) = line.trim().strip_prefix("fn ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    declared.insert(name);
                }
            }
        }
    }
    // The extractor is not vacuous: it found the seven the execution plan names.
    for named in [
        "ai_cannot_resolve_a_question",
        "ai_cannot_confirm_mastery",
        "ai_cannot_decide_enrollment_or_career",
        "ai_cannot_attest_permission",
        "ai_cannot_approve_egress",
        "ai_cannot_confirm_deletion",
        "graduation_result_cannot_come_from_generation",
    ] {
        assert!(
            declared.contains(named),
            "the execution plan's acceptance test {named} is not declared"
        );
    }

    let mut cited: BTreeSet<String> = BTreeSet::new();
    for path in product_sources()? {
        for line in fs::read_to_string(&path)?.lines() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("//") {
                continue;
            }
            for token in trimmed.split('`').skip(1).step_by(2) {
                if looks_like_a_test_name(token) {
                    cited.insert(token.to_owned());
                }
            }
        }
    }
    assert!(
        cited.len() >= 5,
        "the citation extractor read {} names; the docs cite more than that",
        cited.len()
    );
    let allowed: BTreeSet<String> = CITED_ELSEWHERE
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    let missing: Vec<&String> = cited
        .iter()
        .filter(|name| !declared.contains(*name) && !allowed.contains(*name))
        .collect();
    assert!(
        missing.is_empty(),
        "a doc comment cites a test that does not exist: {missing:?}"
    );
    // And the allowlist cannot rot into a permission slip: a name on it that
    // this crate does declare is a stale entry.
    for (name, owner) in CITED_ELSEWHERE {
        assert!(
            !declared.contains(*name),
            "{name} is declared here, so its {owner} entry is stale"
        );
    }

    // The token rule is checked against what it must and must not match.
    assert!(looks_like_a_test_name("ai_cannot_resolve_a_question"));
    assert!(!looks_like_a_test_name("academic_policy"));
    assert!(!looks_like_a_test_name("kind_name"));
    assert!(!looks_like_a_test_name("AutomaticLevel"));
    Ok(())
}

/// A backticked token that is a lower-snake name of three or more words.
///
/// Three, not two, because two-word names are mostly crate and type paths —
/// `academic_policy`, `kind_name` — and a rule that caught those would have
/// needed an allowlist longer than the thing it guards.
fn looks_like_a_test_name(token: &str) -> bool {
    token.len() >= 12
        && token.matches('_').count() >= 2
        && token
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !token.starts_with('_')
        && !token.ends_with('_')
}

/// Names this crate's docs cite that another crate declares, and which one.
///
/// Empty, and that is the current fact rather than a shape nobody uses: every
/// test this crate's product documentation names is one of its own.
const CITED_ELSEWHERE: &[(&str, &str)] = &[];

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
            let inner: String = field[open + 1..].trim().trim_end_matches('}').to_owned();
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
        if ty.is_empty() || ty.starts_with('&') && !ty.starts_with("&'static") {
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
        found.extend(declared_types_in(&fs::read_to_string(&path)?));
    }
    Ok(found)
}

/// Every type name one file declares.
fn declared_types_in(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let code = strip_non_code(source);
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
    found
}

fn collapse(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every public signature in `crates/non-delegable/src`, as the extractor reads
/// it.
const PUBLIC_SIGNATURES: &[&str] = &[
    "action.rs: pub const fn as_str(self) -> &'static str",
    "action.rs: pub const fn declared_phrase(self) -> &'static str",
    "action.rs: pub const fn declared_tier(self) -> Option<RiskTier>",
    "action.rs: pub const fn delegability(self) -> Delegability",
    "action.rs: pub const fn non_delegable(self) -> Option<NonDelegableAction>",
    "action.rs: pub const fn spec_row(self) -> &'static str",
    "action.rs: pub fn all() -> Vec<Self>",
    "action.rs: pub fn parse(value: &str) -> Option<Self>",
    "command.rs: pub const fn action(&self) -> Action",
    "command.rs: pub const fn actor(&self) -> &Actor",
    "command.rs: pub const fn actor(&self) -> &Actor",
    "command.rs: pub const fn generation(&self) -> CandidateGeneration",
    "command.rs: pub const fn subject(&self) -> ContentDigest",
    "command.rs: pub const fn subject(&self) -> ContentDigest",
    "command.rs: pub const fn submitted( action: Action, actor: Actor, subject: ContentDigest, submitted_at: TimestampMillis, ) -> Self",
    "command.rs: pub const fn submitted_at(&self) -> TimestampMillis",
    "command.rs: pub const fn submitted_at(&self) -> TimestampMillis",
    "command.rs: pub fn authorise(command: ActionCommand) -> Result<AuthorizedCommand, NonDelegableError>",
    "decision.rs: pub const fn action(&self) -> NonDelegableAction",
    "decision.rs: pub const fn decided_at(&self) -> TimestampMillis",
    "decision.rs: pub const fn decision(&self) -> &UserDecision",
    "decision.rs: pub const fn subject(&self) -> ContentDigest",
    "decision.rs: pub fn authorises( &self, action: NonDelegableAction, subject: ContentDigest, ) -> Result<(), NonDelegableError>",
    "decision.rs: pub fn recorded( action: NonDelegableAction, actor: &Actor, subject: ContentDigest, decided_at: TimestampMillis, ) -> Result<Self, NonDelegableError>",
];
