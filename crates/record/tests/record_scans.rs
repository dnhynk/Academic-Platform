//! Source scans for the `P2-U4` arithmetic path.
//!
//! This crate's whole risk is a wrong number that no behavioural test notices.
//! `docs/contracts/policy-source-scans.md` records the three shapes that make a
//! scan of this repository empty, and this file is written against all three.
//!
//! **The walk does not stop short.** [`crate_sources`] descends into every
//! subdirectory of the package, less `tests`, and the floor below
//! it fails if it returns fewer files than the crate has modules. A tripwire
//! additionally requires every `pub mod name;` in `lib.rs` to be a file the
//! walk actually read, so adding a module without the walk reaching it is a
//! failure rather than a silent gap. The package rather than `src`: `examples/`
//! is product-shaped code with no feature gate that `cargo clippy
//! --workspace --all-targets` compiles and `pnpm harness:emit` runs, and a
//! walk rooted at `src` never read it.
//!
//! **The float check is not a token list.** A list of forbidden spellings
//! refuses `f64` and `f32` and admits `let ratio = 33.9 / 12.0;`, which reaches
//! `f64` by inference and names neither token. The check is therefore over
//! *literals*: any decimal-point or exponent literal in the crate's code is a
//! floating-point value in Rust, whatever it is called, and there are none.
//! Comments and string literals are removed before the check so prose that
//! writes `2.825` — as this crate's documentation does, deliberately — does not
//! trip it and does not have to be avoided.
//!
//! **The one rounding decision is pinned as whole text.** `div_round_half_up`
//! is the only place in the crate where a quotient is rounded, and the scale
//! and the rule are its arguments rather than its constants. A token list could
//! not see a truncation replacing the rounding, or a fixed `2` replacing the
//! scale parameter's use. [`WHOLE_DIVISION`] is compared against the whole
//! function, so any edit to it must edit the constant in the same commit.
//! `docs/contracts/policy-source-scans.md` calls that the pin's cost, and this
//! is one of the two decision sites in this crate worth spending it on.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

/// The crate root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file this crate ships, recursively.
///
/// The whole package rather than `src`, less `tests`. `S-12` in
/// `docs/contracts/policy-source-scans.md` is the row about a walk that reads
/// `<crate>/src` and stops seeing product-shaped code beside it, and this
/// crate is where `T146` measured the cost: `crates/record/examples/emit_harness.rs`
/// is compiled by `cargo clippy --workspace --all-targets`, is run by the
/// documented `pnpm harness:emit` script, has no feature gate, and an `f64`
/// added to it passed `no_float_reaches_the_gpa_path` -- this crate's own
/// contract -- while the same `f64` in `src/harness.rs` failed at once.
///
/// `tests` stays out. The README sentence this scan keeps is about what the
/// crate computes with, and `tests/record.rs` names `f64` on purpose, to state
/// what the integer path is being compared against.
///
/// `benches` used to stay out beside it, on that same reason -- which was a
/// reason about `tests` and never about `benches`. A bench target meets the
/// test `T146` applied to `examples/`: no feature gate, and
/// `cargo clippy --workspace --all-targets` compiles it. `T149` measured that
/// directly, with a `crates/record/benches/` file that failed to compile and
/// took the clippy lane down with it. No `benches` tree exists today, so
/// widening this reaches nothing; it is what stops the first one from being a
/// tree no scan reads.
fn crate_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    let mut found = Vec::new();
    walk(&root, &mut found)?;
    found.retain(|path| {
        let relative = path.strip_prefix(&root).unwrap_or(path);
        !relative.starts_with("tests")
    });
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
/// What is left is code. Every removed region is replaced by a space rather
/// than deleted so nothing on either side is joined into a new token.
fn strip_non_code(source: &str) -> String {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();

        // Line comment.
        if current == '/' && next == Some('/') {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            out.push('\n');
            continue;
        }
        // Block comment, nested as Rust allows.
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
        // Raw string, with any number of hashes.
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
                let end = rest.find(&terminator).map_or(bytes.len(), |offset| {
                    probe + 1 + rest[..offset].chars().count() + terminator.chars().count()
                });
                index = end;
                out.push(' ');
                continue;
            }
        }
        // Ordinary string.
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
        // Character literal, distinguished from a lifetime by its closing quote.
        if current == '\'' {
            let closes = if next == Some('\\') {
                bytes
                    .iter()
                    .skip(index + 2)
                    .position(|character| *character == '\'')
                    .map(|offset| index + 2 + offset)
            } else {
                (bytes.get(index + 2) == Some(&'\'')).then_some(index + 2)
            };
            if let Some(end) = closes {
                index = end + 1;
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

/// Whether the code contains a floating-point value under any spelling.
///
/// Three shapes, because Rust has three ways to reach `f64` and only one of
/// them names it:
///
/// - the type, spelled anywhere (`f32`, `f64`, `core::primitive::f64`);
/// - a decimal-point literal (`33.9`, `1.`), which is `f64` by inference;
/// - an exponent literal (`1e-9`, `2E10`), likewise.
fn float_findings(code: &str) -> Vec<String> {
    let mut findings = Vec::new();
    let characters: Vec<char> = code.chars().collect();

    for (line_number, line) in code.lines().enumerate() {
        for token in ["f32", "f64"] {
            if let Some(column) = line.find(token) {
                let before = line[..column].chars().last();
                let after = line[column + token.len()..].chars().next();
                let word_before = before.is_some_and(|c| c.is_alphanumeric() || c == '_');
                let word_after = after.is_some_and(|c| c.is_alphanumeric() || c == '_');
                if !word_before && !word_after {
                    findings.push(format!("line {}: float type `{token}`", line_number + 1));
                }
            }
        }
    }

    let mut index = 0;
    while index < characters.len() {
        if !characters[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        // Do not start inside an identifier such as `attempt_000`.
        if index > 0 {
            let previous = characters[index - 1];
            if previous.is_alphanumeric() || previous == '_' || previous == '.' {
                index += 1;
                continue;
            }
        }
        let start = index;
        while index < characters.len()
            && (characters[index].is_ascii_digit() || characters[index] == '_')
        {
            index += 1;
        }
        // `1.5` and `1.` are floats. `1..5` is a range and `1.max(2)` is an
        // integer method call, so a dot followed by another dot or by an
        // identifier character is not one.
        if characters.get(index) == Some(&'.') {
            let after = characters.get(index + 1).copied();
            let is_float = match after {
                Some(character) => {
                    character.is_ascii_digit()
                        || !(character == '.' || character.is_alphabetic() || character == '_')
                }
                None => true,
            };
            if is_float {
                let literal: String = characters[start..=index.min(characters.len() - 1)]
                    .iter()
                    .collect();
                findings.push(format!("decimal-point literal near `{literal}`"));
                index += 1;
                continue;
            }
        }
        // `1e9`, `1E-9`. A hex literal cannot reach here because `0x` stops the
        // digit run at the `x`.
        if matches!(characters.get(index), Some('e') | Some('E')) {
            let mut probe = index + 1;
            if matches!(characters.get(probe), Some('+') | Some('-')) {
                probe += 1;
            }
            if characters.get(probe).is_some_and(char::is_ascii_digit) {
                let literal: String = characters[start..probe].iter().collect();
                findings.push(format!("exponent literal near `{literal}`"));
            }
        }
    }
    findings
}

/// The one rounding decision, whitespace-collapsed. Nothing else may be in it.
const WHOLE_DIVISION: &str = "pub fn div_round_half_up( numerator: Decimal, denominator: Decimal, scale: u8, ) -> Result<Decimal, RecordError> { if scale > MAX_SCALE { return Err(RecordError::DecimalScaleTooLarge(scale)); } if is_zero(denominator) { return Err(RecordError::DivisionByZero); } let net = i32::from(denominator.scale()) + i32::from(scale) - i32::from(numerator.scale()); let (mut top, mut bottom) = (numerator.coefficient(), denominator.coefficient()); if net >= 0 { let factor = pow10(u32::try_from(net).map_err(|_| RecordError::DecimalOverflow)?)?; top = top .checked_mul(factor) .ok_or(RecordError::DecimalOverflow)?; } else { let factor = pow10(u32::try_from(-net).map_err(|_| RecordError::DecimalOverflow)?)?; bottom = bottom .checked_mul(factor) .ok_or(RecordError::DecimalOverflow)?; } let negative = (top < 0) != (bottom < 0); let top_magnitude = top.checked_abs().ok_or(RecordError::DecimalOverflow)?; let bottom_magnitude = bottom.checked_abs().ok_or(RecordError::DecimalOverflow)?; let quotient = top_magnitude / bottom_magnitude; let remainder = top_magnitude % bottom_magnitude; let doubled = remainder .checked_mul(2) .ok_or(RecordError::DecimalOverflow)?; let rounded = if doubled >= bottom_magnitude { quotient .checked_add(1) .ok_or(RecordError::DecimalOverflow)? } else { quotient }; let signed = if negative { rounded.checked_neg().ok_or(RecordError::DecimalOverflow)? } else { rounded }; Ok(Decimal::new(signed, scale)?) }";

/// Extracts one item's text, comment lines dropped and whitespace collapsed.
fn declared_item(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{signature} has no closing brace at column zero"))?;
    let body = &source[start..start + end + 2];
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

/// No floating-point value exists anywhere in this crate's sources.
#[test]
fn no_float_reaches_the_gpa_path() -> TestResult {
    let sources = crate_sources()?;

    // The floor. A walk that returned nothing would pass every assertion in the
    // loop below it, which is the third empty-scan shape the contract names.
    assert!(
        sources.len() >= 11,
        "the walk found {} source files; the crate has more than that, so it stopped short",
        sources.len()
    );

    // The tripwire. Every module `lib.rs` declares must be a file the walk read,
    // so a module added in a subdirectory cannot be missed.
    let lib = fs::read_to_string(crate_root().join("src/lib.rs"))?;
    let read: BTreeSet<String> = sources
        .iter()
        .filter_map(|path| path.file_stem())
        .filter_map(|stem| stem.to_str())
        .map(str::to_owned)
        .collect();
    let mut declared = 0_usize;
    for line in lib.lines() {
        let trimmed = line.trim();
        let Some(name) = trimmed
            .strip_prefix("pub mod ")
            .or_else(|| trimmed.strip_prefix("mod "))
            .and_then(|rest| rest.strip_suffix(';'))
        else {
            continue;
        };
        declared += 1;
        assert!(
            read.contains(name) || read.contains("mod"),
            "`{name}` is declared in lib.rs and the walk never read it"
        );
    }
    assert!(declared >= 10, "lib.rs declares only {declared} modules");

    for path in &sources {
        let source = fs::read_to_string(path)?;
        let code = strip_non_code(&source);
        let findings = float_findings(&code);
        assert!(
            findings.is_empty(),
            "{} carries floating point: {findings:?}",
            path.display()
        );
    }

    // The check is not vacuous: each of the five evasions it exists to refuse
    // is run through it here, and each must be caught. Three of them spell
    // neither `f32` nor `f64`, which is what a token list would have missed.
    let evasions = [
        ("named type", "let ratio: f64 = 0;"),
        ("qualified type", "let ratio: core::primitive::f64 = x;"),
        ("decimal-point literal", "let ratio = 33.9 / 12.0;"),
        ("exponent literal", "let epsilon = 1e-9;"),
        ("trailing-point literal", "let one = 1.;"),
    ];
    for (label, sample) in evasions {
        assert!(
            !float_findings(sample).is_empty(),
            "the scan does not catch a float introduced as a {label}"
        );
    }
    // And it does not fire on the integer shapes this crate really uses.
    for benign in [
        "let range = 1000..=9999;",
        "let scale = value.scale();",
        "let coefficient = 10_i128.checked_pow(exponent);",
        "let first = tuple.0;",
        "let hex = 0xff;",
        "let index = attempt_000;",
    ] {
        assert!(
            float_findings(benign).is_empty(),
            "the scan fires on an integer expression: {benign}"
        );
    }

    // The stripper is what makes the literal rule usable, so it is checked too:
    // a float inside a comment or a string must not be reported, and the same
    // float in code must be.
    assert!(float_findings(&strip_non_code("// the answer is 2.825\n")).is_empty());
    assert!(float_findings(&strip_non_code("let text = \"2.825\";")).is_empty());
    assert!(float_findings(&strip_non_code("let r = r#\"2.825\"#;")).is_empty());
    assert!(float_findings(&strip_non_code("/* 2.825 */")).is_empty());
    assert!(!float_findings(&strip_non_code("let value = 2.825;")).is_empty());
    // A lifetime is not a character literal, and stripping it would delete code.
    assert!(
        !float_findings(&strip_non_code("fn f<'a>(x: &'a str) -> f64 { 1.0 }")).is_empty(),
        "the stripper must not swallow code after a lifetime"
    );
    Ok(())
}

/// Rounding happens in exactly one place, and that place is pinned whole.
#[test]
fn the_published_average_is_rounded_in_one_pinned_place() -> TestResult {
    let decimal_source = fs::read_to_string(crate_root().join("src/decimal.rs"))?;

    // One rounding site in the crate: the division. `%` and `/` on integers are
    // exact; what makes a rounding decision is the half-away-from-zero step,
    // and it appears once.
    let sources = crate_sources()?;
    let rounding_sites: Vec<String> = sources
        .iter()
        .filter(|path| {
            fs::read_to_string(path)
                .is_ok_and(|source| strip_non_code(&source).contains("checked_mul(2)"))
        })
        .map(|path| path.display().to_string())
        .collect();
    assert_eq!(
        rounding_sites.len(),
        1,
        "rounding must happen in exactly one file, found {rounding_sites:?}"
    );

    let declared = declared_item(&decimal_source, "pub fn div_round_half_up")?;
    assert_eq!(
        declared, WHOLE_DIVISION,
        "the rounding decision changed; the pin must change with it in the same commit"
    );

    // The pin is not the whole claim: the scale is a parameter, so a caller
    // decides it, and the versioned scheme is the caller. If the scale were a
    // constant here, `gpa_policy_version_matrix` could not move it.
    assert!(
        declared.contains("scale: u8,"),
        "the published scale must stay an argument"
    );
    assert!(
        !declared.contains("= 2"),
        "the rounding site must not hard-code a published scale"
    );

    // The other half of the arithmetic contract: every function in this module
    // takes and returns the canonical `Decimal`. A second numeric type would
    // show up as a different return.
    let code = strip_non_code(&decimal_source);
    for forbidden in ["struct ", "enum ", "union ", "type "] {
        assert!(
            !code.contains(forbidden),
            "`{forbidden}` declares a type in the arithmetic module; \
             the canonical Decimal is the only numeric type this crate has"
        );
    }
    Ok(())
}
