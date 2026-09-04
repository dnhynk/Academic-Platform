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

/// The floor the inventory walk must reach, so an empty walk fails as a walk.
const INVENTORY_FILE_FLOOR: usize = 15;

/// Every function this package declares, as `<file> [vis] <signature>`.
const DECLARATIONS: &[&str] = &[
    "examples/emit_harness.rs [priv] fn main() -> Result<(), Box<dyn std::error::Error>>",
    "src/attempt.rs [priv] fn push( &mut self, attempt: CourseAttempt, supersedes: Option<AttemptId>, relation: Option<ClaimRelation>, ) -> Result<(), RecordError>",
    "src/attempt.rs [pub] fn all(&self) -> &[AttemptEntry]",
    "src/attempt.rs [pub] fn append(&mut self, attempt: CourseAttempt) -> Result<(), RecordError>",
    "src/attempt.rs [pub] fn append_correction( &mut self, attempt: CourseAttempt, supersedes: AttemptId, scope_id: ScopeId, source_claim_id: academic_domain::ClaimId, target_claim_id: academic_domain::ClaimId, ) -> Result<(), RecordError>",
    "src/attempt.rs [pub] fn as_original(mut self) -> Self",
    "src/attempt.rs [pub] fn as_repeat_of(mut self, earlier: AttemptId) -> Self",
    "src/attempt.rs [pub] fn as_str(self) -> &'static str",
    "src/attempt.rs [pub] fn as_str(self) -> &'static str",
    "src/attempt.rs [pub] fn attempt(&self) -> &CourseAttempt",
    "src/attempt.rs [pub] fn contains(&self, id: AttemptId) -> bool",
    "src/attempt.rs [pub] fn course_code(&self) -> &str",
    "src/attempt.rs [pub] fn course_code(&self) -> &str",
    "src/attempt.rs [pub] fn credits_attempted(&self) -> Decimal",
    "src/attempt.rs [pub] fn credits_attempted(&self) -> Decimal",
    "src/attempt.rs [pub] fn credits_earned(&self) -> Decimal",
    "src/attempt.rs [pub] fn current(&self) -> Vec<&CourseAttempt>",
    "src/attempt.rs [pub] fn evidence_ids(&self) -> &[EvidenceId]",
    "src/attempt.rs [pub] fn evidence_ids(&self) -> &[EvidenceId]",
    "src/attempt.rs [pub] fn from_confirmed_registration( id: AttemptId, confirmation: &RegistrationConfirmation, grading_scheme_id: impl Into<String>, ) -> Result<Self, RecordError>",
    "src/attempt.rs [pub] fn from_confirmed_row( id: AttemptId, course_code: impl Into<String>, term: TermKey, status: SettledStatus, origin: AttemptOrigin, credits_attempted: Decimal, credits_earned: Decimal, grade: Option<GradeSymbol>, grading_scheme_id: impl Into<String>, evidence_ids: Vec<EvidenceId>, ) -> Result<Self, RecordError>",
    "src/attempt.rs [pub] fn get(&self, id: AttemptId) -> Option<&CourseAttempt>",
    "src/attempt.rs [pub] fn grade(&self) -> Option<GradeSymbol>",
    "src/attempt.rs [pub] fn grading_scheme_id(&self) -> &str",
    "src/attempt.rs [pub] fn id(&self) -> AttemptId",
    "src/attempt.rs [pub] fn into_status(self) -> AttemptStatus",
    "src/attempt.rs [pub] fn is_settled(self) -> bool",
    "src/attempt.rs [pub] fn new( course_code: impl Into<String>, term: TermKey, credits_attempted: Decimal, evidence_ids: Vec<EvidenceId>, ) -> Result<Self, RecordError>",
    "src/attempt.rs [pub] fn new() -> Self",
    "src/attempt.rs [pub] fn origin(&self) -> AttemptOrigin",
    "src/attempt.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/attempt.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/attempt.rs [pub] fn recognition(&self) -> RecognitionDecision",
    "src/attempt.rs [pub] fn relation(&self) -> Option<&ClaimRelation>",
    "src/attempt.rs [pub] fn repeat_of(&self) -> Option<AttemptId>",
    "src/attempt.rs [pub] fn repeat_status(&self) -> RepeatStatus",
    "src/attempt.rs [pub] fn status(&self) -> AttemptStatus",
    "src/attempt.rs [pub] fn supersedes(&self) -> Option<AttemptId>",
    "src/attempt.rs [pub] fn term(&self) -> TermKey",
    "src/attempt.rs [pub] fn term(&self) -> TermKey",
    "src/attempt.rs [pub] fn with_recognition(mut self, decision: RecognitionDecision) -> Self",
    "src/classify.rs [pub] fn as_str(&self) -> &str",
    "src/classify.rs [pub] fn as_str(self) -> &'static str",
    "src/classify.rs [pub] fn category(&self) -> RequirementCategory",
    "src/classify.rs [pub] fn classification_claim( classification: &RequirementClassification, claim_id: ClaimId, subject_entity_id: EntityId, scope_id: ScopeId, valid_time: ValidInterval, evidence_ids: Vec<EvidenceId>, ) -> Result<(Claim, Actor), RecordError>",
    "src/classify.rs [pub] fn classify(&self, attempt: &CourseAttempt) -> Vec<RequirementClassification>",
    "src/classify.rs [pub] fn id(&self) -> &str",
    "src/classify.rs [pub] fn is_major(self) -> bool",
    "src/classify.rs [pub] fn new(value: impl Into<String>) -> Result<Self, RecordError>",
    "src/classify.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/classify.rs [pub] fn program(&self) -> &ProgramId",
    "src/classify.rs [pub] fn programs(&self) -> Vec<ProgramId>",
    "src/classify.rs [pub] fn publish( id: impl Into<String>, rules: Vec<ClassificationRule>, ) -> Result<Self, RecordError>",
    "src/classify.rs [pub] fn rule_id(&self) -> &str",
    "src/classify.rs [pub] fn rules(&self) -> &[ClassificationRule]",
    "src/classify.rs [pub] fn ruleset_id(&self) -> &str",
    "src/corpus.rs [priv] fn credits(whole: i128) -> Result<Decimal, RecordError>",
    "src/corpus.rs [priv] fn settled( index: u8, course_code: &str, term: &str, status: SettledStatus, origin: AttemptOrigin, grade: Option<GradeSymbol>, attempted: i128, earned: i128, ) -> Result<CourseAttempt, RecordError>",
    "src/corpus.rs [pub] fn baseline_history() -> Result<AttemptHistory, RecordError>",
    "src/corpus.rs [pub] fn baseline_rules() -> Result<RuleBook, RecordError>",
    "src/corpus.rs [pub] fn baseline_rules_scale3() -> Result<RuleBook, RecordError>",
    "src/corpus.rs [pub] fn classification_v1() -> Result<ClassificationRuleSet, RecordError>",
    "src/corpus.rs [pub] fn confirmed_policy_ceiling_from(term: &str) -> Result<PolicyBook, RecordError>",
    "src/corpus.rs [pub] fn confirmed_policy_v1() -> Result<PolicyBook, RecordError>",
    "src/corpus.rs [pub] fn history_with_conflicting_records() -> Result<AttemptHistory, RecordError>",
    "src/corpus.rs [pub] fn history_with_undated_external() -> Result<AttemptHistory, RecordError>",
    "src/corpus.rs [pub] fn published_rules() -> Result<RuleBook, RecordError>",
    "src/corpus.rs [pub] fn single_grade_history(grade: GradeSymbol) -> Result<AttemptHistory, RecordError>",
    "src/corpus.rs [pub] fn synthetic_attempt_id(index: u8) -> Result<AttemptId, RecordError>",
    "src/corpus.rs [pub] fn synthetic_evidence_id(index: u8) -> Result<EvidenceId, RecordError>",
    "src/decimal.rs [priv] fn align(left: Decimal, right: Decimal) -> Result<(i128, i128, u8), RecordError>",
    "src/decimal.rs [priv] fn pow10(exponent: u32) -> Result<i128, RecordError>",
    "src/decimal.rs [pub] fn add(left: Decimal, right: Decimal) -> Result<Decimal, RecordError>",
    "src/decimal.rs [pub] fn compare(left: Decimal, right: Decimal) -> Result<Ordering, RecordError>",
    "src/decimal.rs [pub] fn div_round_half_up( numerator: Decimal, denominator: Decimal, scale: u8, ) -> Result<Decimal, RecordError>",
    "src/decimal.rs [pub] fn integer(value: i128) -> Result<Decimal, RecordError>",
    "src/decimal.rs [pub] fn is_zero(value: Decimal) -> bool",
    "src/decimal.rs [pub] fn mul(left: Decimal, right: Decimal) -> Result<Decimal, RecordError>",
    "src/decimal.rs [pub] fn parse(text: &str) -> Result<Decimal, RecordError>",
    "src/decimal.rs [pub] fn render(value: Decimal) -> String",
    "src/decimal.rs [pub] fn rescale(value: Decimal, target: u8) -> Result<Decimal, RecordError>",
    "src/decimal.rs [pub] fn sub(left: Decimal, right: Decimal) -> Result<Decimal, RecordError>",
    "src/decimal.rs [pub] fn zero() -> Result<Decimal, RecordError>",
    "src/engine.rs [priv] fn attempt_input_keys( inputs: &FrozenInputs, attempt: &AttemptFacts, ) -> Result<Vec<InputKey>, RecordError>",
    "src/engine.rs [priv] fn duplicate_records(facts: &[AttemptFacts]) -> BTreeSet<academic_domain::AttemptId>",
    "src/engine.rs [priv] fn engine_id(&self) -> &'static str",
    "src/engine.rs [priv] fn engine_id(&self) -> &'static str",
    "src/engine.rs [priv] fn engine_version(&self) -> EngineVersion",
    "src/engine.rs [priv] fn engine_version(&self) -> EngineVersion",
    "src/engine.rs [priv] fn evaluate( &self, inputs: &FrozenInputs, rule_set_hash: RuleSetHash, _engine_version: EngineVersion, ) -> Result<EngineOutcome, EngineError>",
    "src/engine.rs [priv] fn evaluate( &self, inputs: &FrozenInputs, rule_set_hash: RuleSetHash, _engine_version: EngineVersion, ) -> Result<EngineOutcome, EngineError>",
    "src/engine.rs [priv] fn rank(status: ProofStatus) -> u8",
    "src/engine.rs [priv] fn scope_input_keys(inputs: &FrozenInputs) -> Result<Vec<InputKey>, RecordError>",
    "src/engine.rs [priv] fn worsen(current: ProofStatus, candidate: ProofStatus) -> ProofStatus",
    "src/engine.rs [pub] fn evaluate_record( &self, inputs: &FrozenInputs, rule_set_hash: RuleSetHash, ) -> Result<EngineOutcome, RecordError>",
    "src/engine.rs [pub] fn evaluate_record( &self, inputs: &FrozenInputs, rule_set_hash: RuleSetHash, ) -> Result<EngineOutcome, RecordError>",
    "src/engine.rs [pub] fn new(rules: RuleBook, version: EngineVersion) -> Self",
    "src/engine.rs [pub] fn new(rules: RuleBook, version: EngineVersion) -> Self",
    "src/engine.rs [pub] fn rule_set_hash(&self) -> RuleSetHash",
    "src/engine.rs [pub] fn rule_set_hash(&self) -> RuleSetHash",
    "src/engine.rs [pub] fn rules(&self) -> &RuleBook",
    "src/facts.rs [priv] fn integer(inputs: &FrozenInputs, key: &str) -> Result<Option<i64>, RecordError>",
    "src/facts.rs [priv] fn required_decimal(inputs: &FrozenInputs, key: &str) -> Result<Decimal, RecordError>",
    "src/facts.rs [priv] fn required_reference(inputs: &FrozenInputs, key: &str) -> Result<String, RecordError>",
    "src/facts.rs [pub] fn decode(inputs: &FrozenInputs) -> Result<(Vec<AttemptFacts>, GpaScope), RecordError>",
    "src/facts.rs [pub] fn encode(facts: &[AttemptFacts], scope: &GpaScope) -> Result<FrozenInputs, RecordError>",
    "src/facts.rs [pub] fn from_attempt(attempt: &CourseAttempt, classification: &ClassificationRuleSet) -> Self",
    "src/facts.rs [pub] fn is_major_for(&self, program: &ProgramId) -> bool",
    "src/facts.rs [pub] fn tag(&self) -> &'static str",
    "src/grade.rs [priv] fn snu_table(id: &str, published_scale: u8) -> Result<Self, RecordError>",
    "src/grade.rs [pub] fn as_str(self) -> &'static str",
    "src/grade.rs [pub] fn as_token(self) -> &'static str",
    "src/grade.rs [pub] fn canonical_text(&self) -> String",
    "src/grade.rs [pub] fn citation(&self) -> &str",
    "src/grade.rs [pub] fn earned_not_graded() -> Self",
    "src/grade.rs [pub] fn earns_credit(&self) -> bool",
    "src/grade.rs [pub] fn grade_points(&self) -> Option<Decimal>",
    "src/grade.rs [pub] fn graded(grade_points: Decimal, earns_credit: bool) -> Self",
    "src/grade.rs [pub] fn id(&self) -> &str",
    "src/grade.rs [pub] fn is_unresolved(&self) -> bool",
    "src/grade.rs [pub] fn new( id: impl Into<String>, treatments: BTreeMap<GradeSymbol, GradeTreatment>, published_scale: u8, citation: impl Into<String>, ) -> Result<Self, RecordError>",
    "src/grade.rs [pub] fn not_earned_not_graded() -> Self",
    "src/grade.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/grade.rs [pub] fn parse_token(text: &str) -> Option<Self>",
    "src/grade.rs [pub] fn participates_in_average(&self) -> bool",
    "src/grade.rs [pub] fn published_scale(&self) -> u8",
    "src/grade.rs [pub] fn snu_4_3_v1() -> Result<Self, RecordError>",
    "src/grade.rs [pub] fn snu_4_3_v2_scale3() -> Result<Self, RecordError>",
    "src/grade.rs [pub] fn treatment(&self, symbol: GradeSymbol) -> GradeTreatment",
    "src/grade.rs [pub] fn unresolved() -> Self",
    "src/harness.rs [priv] fn bounds_text() -> String",
    "src/harness.rs [priv] fn credit_cases() -> Result<Vec<(String, FrozenInputs)>, RecordError>",
    "src/harness.rs [priv] fn gpa_cases() -> Result<Vec<(String, FrozenInputs)>, RecordError>",
    "src/harness.rs [priv] fn inputs_for(history: &AttemptHistory, scope: &GpaScope) -> Result<FrozenInputs, RecordError>",
    "src/harness.rs [pub] fn corpus_files() -> Result<Vec<CorpusFile>, RecordError>",
    "src/ingest.rs [priv] fn claim_object_text(claim: &academic_domain::Claim) -> Option<String>",
    "src/ingest.rs [priv] fn row_object_text(row: &TranscriptRow) -> String",
    "src/ingest.rs [pub] fn attempt_from_confirmed_row( id: AttemptId, row: &TranscriptRow, confirmed: &ConfirmedRowClaim, status: SettledStatus, origin: AttemptOrigin, grading_scheme_id: impl Into<String>, evidence_ids: Vec<EvidenceId>, ) -> Result<CourseAttempt, RecordError>",
    "src/lib.rs [priv] fn check_identifier(value: &str) -> bool",
    "src/lib.rs [pub] fn into_engine_error(self) -> EngineError",
    "src/plan.rs [pub] fn choices(&self) -> &[PlanScenarioChoice]",
    "src/plan.rs [pub] fn course_code(&self) -> &str",
    "src/plan.rs [pub] fn delete_scenario( store: &mut PlanStore, history: &AttemptHistory, scenario_id: EntityId, ) -> Result<PlanDeletion, RecordError>",
    "src/plan.rs [pub] fn get(&self, id: EntityId) -> Option<&PlanScenario>",
    "src/plan.rs [pub] fn id(&self) -> EntityId",
    "src/plan.rs [pub] fn insert(&mut self, scenario: PlanScenario) -> Result<(), RecordError>",
    "src/plan.rs [pub] fn intended_term(&self) -> TermKey",
    "src/plan.rs [pub] fn is_empty(&self) -> bool",
    "src/plan.rs [pub] fn label(&self) -> &str",
    "src/plan.rs [pub] fn len(&self) -> usize",
    "src/plan.rs [pub] fn new( course_code: impl Into<String>, intended_term: TermKey, ) -> Result<Self, RecordError>",
    "src/plan.rs [pub] fn new( id: EntityId, label: impl Into<String>, choices: Vec<PlanScenarioChoice>, ) -> Result<Self, RecordError>",
    "src/plan.rs [pub] fn new() -> Self",
    "src/policy.rs [pub] fn as_str(self) -> &'static str",
    "src/policy.rs [pub] fn as_str(self) -> &'static str",
    "src/policy.rs [pub] fn as_str(self) -> &'static str",
    "src/policy.rs [pub] fn canonical_text(&self) -> String",
    "src/policy.rs [pub] fn canonical_text(&self) -> String",
    "src/policy.rs [pub] fn classification_ruleset_id(&self) -> &str",
    "src/policy.rs [pub] fn digest(&self) -> ContentDigest",
    "src/policy.rs [pub] fn external_row_at(&self, term: TermKey) -> Option<&ExternalGradePolicyRow>",
    "src/policy.rs [pub] fn external_rows(&self) -> &[ExternalGradePolicyRow]",
    "src/policy.rs [pub] fn is_external(self) -> bool",
    "src/policy.rs [pub] fn new( mut repeat_rows: Vec<RepeatPolicyRow>, mut external_rows: Vec<ExternalGradePolicyRow>, ) -> Result<Self, RecordError>",
    "src/policy.rs [pub] fn new( scheme: GradingScheme, policies: PolicyBook, classification_ruleset_id: impl Into<String>, ) -> Self",
    "src/policy.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/policy.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/policy.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/policy.rs [pub] fn policies(&self) -> &PolicyBook",
    "src/policy.rs [pub] fn published_v1() -> Result<Self, RecordError>",
    "src/policy.rs [pub] fn repeat_row_at(&self, term: TermKey) -> Option<&RepeatPolicyRow>",
    "src/policy.rs [pub] fn repeat_rows(&self) -> &[RepeatPolicyRow]",
    "src/policy.rs [pub] fn scheme(&self) -> &GradingScheme",
    "src/term.rs [priv] fn cmp(&self, other: &Self) -> Ordering",
    "src/term.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "src/term.rs [priv] fn partial_cmp(&self, other: &Self) -> Option<Ordering>",
    "src/term.rs [pub] fn as_str(self) -> &'static str",
    "src/term.rs [pub] fn canonical_text(self) -> String",
    "src/term.rs [pub] fn new(year: u16, semester: Semester) -> Result<Self, RecordError>",
    "src/term.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/term.rs [pub] fn parse(text: &str) -> Result<Self, RecordError>",
    "src/term.rs [pub] fn parse_transcript_term(text: &str) -> Result<Self, RecordError>",
    "src/term.rs [pub] fn semester(self) -> Semester",
    "src/term.rs [pub] fn year(self) -> u16",
    "src/views.rs [priv] fn average_contribution( attempt: &AttemptFacts, grade: GradeSymbol, treatment: GradeTreatment, ceilinged: &BTreeMap<AttemptId, GradeSymbol>, rules: &RuleBook, ) -> Result<AverageContribution, RecordError>",
    "src/views.rs [priv] fn average_over<'a>( &self, dispositions: impl Iterator<Item = &'a AttemptDisposition>, ) -> Result<GpaValue, RecordError>",
    "src/views.rs [priv] fn disposition_for( attempt: &AttemptFacts, rules: &RuleBook, undecided_groups: &BTreeSet<AttemptId>, displaced: &BTreeSet<AttemptId>, ceilinged: &BTreeMap<AttemptId, GradeSymbol>, ) -> Result<AttemptDisposition, RecordError>",
    "src/views.rs [priv] fn exceeds_ceiling(grade: GradeSymbol, ceiling: GradeSymbol, rules: &RuleBook) -> bool",
    "src/views.rs [priv] fn highest_graded<'a>( group: &[&'a AttemptFacts], rules: &RuleBook, ) -> Result<Option<&'a AttemptFacts>, RecordError>",
    "src/views.rs [priv] fn resolve_repeat_groups( facts: &[AttemptFacts], rules: &RuleBook, ) -> Result<Vec<RepeatProof>, RecordError>",
    "src/views.rs [pub] fn as_str(self) -> &'static str",
    "src/views.rs [pub] fn attempt_id(&self) -> AttemptId",
    "src/views.rs [pub] fn average(&self) -> AverageContribution",
    "src/views.rs [pub] fn categories( &self, attempt_id: AttemptId, ) -> Option<&BTreeMap<ProgramId, RequirementCategory>>",
    "src/views.rs [pub] fn complete(&self) -> Option<Decimal>",
    "src/views.rs [pub] fn compute( history: &AttemptHistory, rules: &RuleBook, classification: &ClassificationRuleSet, ) -> Result<Self, RecordError>",
    "src/views.rs [pub] fn contributes_to_actual_progress(status: AttemptStatus) -> bool",
    "src/views.rs [pub] fn course_code(&self) -> &str",
    "src/views.rs [pub] fn credit(&self) -> CreditContribution",
    "src/views.rs [pub] fn cumulative_gpa(&self) -> Result<GpaValue, RecordError>",
    "src/views.rs [pub] fn cumulative_included(&self) -> Vec<AttemptId>",
    "src/views.rs [pub] fn dispositions(&self) -> &[AttemptDisposition]",
    "src/views.rs [pub] fn earned_credits(&self) -> Result<CreditTotal, RecordError>",
    "src/views.rs [pub] fn from_facts(facts: &[AttemptFacts], rules: &RuleBook) -> Result<Self, RecordError>",
    "src/views.rs [pub] fn gpa_denominator(&self) -> Result<CreditTotal, RecordError>",
    "src/views.rs [pub] fn is_unknown(self) -> bool",
    "src/views.rs [pub] fn known(&self) -> Option<Decimal>",
    "src/views.rs [pub] fn major_gpa(&self, program: &ProgramId) -> Result<GpaValue, RecordError>",
    "src/views.rs [pub] fn parse(text: &str) -> Option<Self>",
    "src/views.rs [pub] fn partial(&self) -> Decimal",
    "src/views.rs [pub] fn policy_row_id(&self) -> Option<&str>",
    "src/views.rs [pub] fn programs(&self) -> Vec<ProgramId>",
    "src/views.rs [pub] fn published_scale(&self) -> u8",
    "src/views.rs [pub] fn quality_points(&self) -> Result<Decimal, RecordError>",
    "src/views.rs [pub] fn reason(&self) -> DispositionReason",
    "src/views.rs [pub] fn recorded_grade(&self) -> Option<GradeSymbol>",
    "src/views.rs [pub] fn repeat_proofs(&self) -> &[RepeatProof]",
    "src/views.rs [pub] fn term(&self) -> TermKey",
    "src/views.rs [pub] fn term_gpa(&self, term: TermKey) -> Result<GpaValue, RecordError>",
    "src/views.rs [pub] fn terms(&self) -> Vec<TermKey>",
    "src/views.rs [pub] fn unknown(&self) -> &[AttemptId]",
];

/// Every `impl` block header this package ships, as `<file>: <header>`.
const IMPL_HEADERS: &[&str] = &[
    "src/attempt.rs: impl AttemptEntry",
    "src/attempt.rs: impl AttemptHistory",
    "src/attempt.rs: impl AttemptStatus",
    "src/attempt.rs: impl CourseAttempt",
    "src/attempt.rs: impl Into<String>, ) -> Result<Self, RecordError>",
    "src/attempt.rs: impl Into<String>, evidence_ids: Vec<EvidenceId>, ) -> Result<Self, RecordError>",
    "src/attempt.rs: impl Into<String>, term: TermKey, credits_attempted: Decimal, evidence_ids: Vec<EvidenceId>, ) -> Result<Self, RecordError>",
    "src/attempt.rs: impl Into<String>, term: TermKey, status: SettledStatus, origin: AttemptOrigin, credits_attempted: Decimal, credits_earned: Decimal, grade: Option<GradeSymbol>, grading_scheme_id: impl Into<String>, evidence_ids: Vec<EvidenceId>, ) -> Result<Self, RecordError>",
    "src/attempt.rs: impl RegistrationConfirmation",
    "src/attempt.rs: impl RepeatStatus",
    "src/attempt.rs: impl SettledStatus",
    "src/classify.rs: impl ClassificationRuleSet",
    "src/classify.rs: impl Into<String>) -> Result<Self, RecordError>",
    "src/classify.rs: impl Into<String>, rules: Vec<ClassificationRule>, ) -> Result<Self, RecordError>",
    "src/classify.rs: impl ProgramId",
    "src/classify.rs: impl RequirementCategory",
    "src/classify.rs: impl RequirementClassification",
    "src/engine.rs: impl CreditAccountingEngine",
    "src/engine.rs: impl DeterministicEngine for CreditAccountingEngine",
    "src/engine.rs: impl DeterministicEngine for GpaEngine",
    "src/engine.rs: impl GpaEngine",
    "src/facts.rs: impl AttemptFacts",
    "src/facts.rs: impl GpaScope",
    "src/grade.rs: impl GradeSymbol",
    "src/grade.rs: impl GradeTreatment",
    "src/grade.rs: impl GradingScheme",
    "src/grade.rs: impl Into<String>, ) -> Result<Self, RecordError>",
    "src/grade.rs: impl Into<String>, treatments: BTreeMap<GradeSymbol, GradeTreatment>, published_scale: u8, citation: impl Into<String>, ) -> Result<Self, RecordError>",
    "src/ingest.rs: impl Into<String>, evidence_ids: Vec<EvidenceId>, ) -> Result<CourseAttempt, RecordError>",
    "src/lib.rs: impl RecordError",
    "src/plan.rs: impl Into<String>, choices: Vec<PlanScenarioChoice>, ) -> Result<Self, RecordError>",
    "src/plan.rs: impl Into<String>, intended_term: TermKey, ) -> Result<Self, RecordError>",
    "src/plan.rs: impl PlanScenario",
    "src/plan.rs: impl PlanScenarioChoice",
    "src/plan.rs: impl PlanStore",
    "src/policy.rs: impl AttemptOrigin",
    "src/policy.rs: impl Into<String>, ) -> Self",
    "src/policy.rs: impl PolicyBook",
    "src/policy.rs: impl RecognitionDecision",
    "src/policy.rs: impl RepeatRecognition",
    "src/policy.rs: impl RuleBook",
    "src/term.rs: impl Ord for TermKey",
    "src/term.rs: impl PartialOrd for TermKey",
    "src/term.rs: impl Semester",
    "src/term.rs: impl TermKey",
    "src/term.rs: impl fmt::Display for TermKey",
    "src/views.rs: impl AttemptDisposition",
    "src/views.rs: impl CreditTotal",
    "src/views.rs: impl DispositionReason",
    "src/views.rs: impl GpaValue",
    "src/views.rs: impl Iterator<Item = &'a AttemptDisposition>, ) -> Result<GpaValue, RecordError>",
    "src/views.rs: impl RecordViews",
];

// ---------------------------------------------------------------------------
// every_declaration_and_impl_in_this_crate_is_pinned
// ---------------------------------------------------------------------------
//
// `P2-A3` measured this crate's blind spot directly: four `impl From<..>` blocks
// appended to a product file gave an external crate a route to a value the
// crate's own doc says has one construction site, and every acceptance test in
// this crate stayed green. A `trait impl` declares no `pub fn`, so a scan built
// on public signatures does not see it, and no scan here counted `impl` blocks
// at all.
//
// `P2-X5` measured the same class as six invisible injections out of nineteen,
// and `P2-Y3` closed it in `crates/cs-map` by pinning the whole set of `impl`
// headers. `academic-review` and `academic-ingestion` were the only two U crates
// carrying that defence. This is it, ported: two whole sets, compared in both
// directions, over every `.rs` file this package ships.
//
// It is deliberately not a list of forbidden spellings. A new function, a new
// method, a new inherent `impl`, a new trait `impl` and a new file all fail as
// an entry nobody wrote down, whatever they are called.

/// Every `.rs` file this package ships: everything outside `tests`.
///
/// The whole package rather than `src`, because `S-12` in
/// `docs/contracts/policy-source-scans.md` is the row about a walk that reads
/// `<crate>/src` and stops seeing product-shaped code beside it --
/// `examples/`, `benches/` and `probes/` are all compiled by
/// `cargo clippy --workspace --all-targets`.
fn inventory_sources() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|name| name == "tests" || name == "target")
                {
                    continue;
                }
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let name = path
                    .strip_prefix(&base)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((name, std::fs::read_to_string(&path)?));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Removes comments, string literals and character literals.
///
/// The raw-string-aware reader from `crates/record/tests/record_scans.rs`,
/// copied deliberately: `P2-G4` found that a lexer without raw strings
/// desynchronizes and reads every literal after one as code.
fn inventory_strip(source: &str) -> String {
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
                let terminator: String = core::iter::once('"')
                    .chain(core::iter::repeat_n('#', hashes))
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

/// Collapses whitespace runs to single spaces.
fn inventory_collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every function declaration in `code`, as a public flag and a signature.
///
/// Visibility is read off the text before `fn` on the same line: `pub(` is
/// crate-private however it continues, a bare `pub` is public, anything else is
/// private. Reading **signatures** rather than names is what makes the pin a
/// statement about what a function takes and returns, so a widened parameter
/// fails as loudly as a new function.
///
/// The `>` of a `->` is skipped: `crates/review`'s copy of this reader records
/// that treating it as a closing bracket truncated `fn counts(self) -> [u32; 5]`
/// to `fn counts(self) -> [u32`, and a pin on a truncated signature is a pin two
/// different signatures satisfy.
fn inventory_declarations(code: &str) -> Vec<(bool, String)> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("fn ") {
        if !(at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_')) {
            continue;
        }
        let line_start = code[..at].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &code[line_start..at];
        let public = prefix.contains("pub") && !prefix.contains("pub(");
        let mut depth = 0_i32;
        let mut end = None;
        let region = &code[at..];
        let region_bytes = region.as_bytes();
        for (offset, character) in region.char_indices() {
            match character {
                '(' | '<' | '[' => depth += 1,
                '>' if offset > 0 && region_bytes[offset - 1] == b'-' => {}
                ')' | '>' | ']' => depth -= 1,
                '{' | ';' if depth <= 0 => {
                    end = Some(at + offset);
                    break;
                }
                _ => {}
            }
        }
        if let Some(end) = end {
            found.push((public, inventory_collapse(&code[at..end])));
        }
    }
    found
}

/// Every `impl` block header in `code`, whole.
///
/// The header is everything from `impl` to the opening brace, so
/// `impl From<usize> for CoverageWitness` and `impl CoverageWitness` are
/// different entries and a trait implementation cannot arrive as an edit to an
/// inherent one.
fn inventory_impl_headers(code: &str) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("impl") {
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        if code[at + 4..]
            .starts_with(|character: char| character.is_alphanumeric() || character == '_')
        {
            continue;
        }
        let Some(end) = code[at..].find(['{', ';']) else {
            continue;
        };
        found.push(inventory_collapse(&code[at..at + end]));
    }
    found
}

/// Nothing this crate declares is outside the two pinned sets.
///
/// Two whole sets, each compared in both directions:
///
/// 1. every function declaration this package ships, as a file, a visibility
///    and a full signature;
/// 2. every `impl` block header this package ships, as a file and a header.
///
/// The second is the one `P2-A3` walked through. Its injection was four
/// `impl From<..>` blocks in a product file -- no `pub fn`, no new name on any
/// forbidden list, no change to any other file -- and it handed an external
/// crate a value the crate's own documentation says it cannot construct. There
/// is no spelling of that injection that this test does not see, because it does
/// not look for spellings: it compares the set.
#[test]
fn every_declaration_and_impl_in_this_crate_is_pinned() -> TestResult {
    let sources = inventory_sources()?;
    assert!(
        sources.len() >= INVENTORY_FILE_FLOOR,
        "the inventory walk read only {} files",
        sources.len()
    );

    let mut declared = Vec::new();
    let mut headers = Vec::new();
    for (name, text) in &sources {
        let code = inventory_strip(text);
        for (public, signature) in inventory_declarations(&code) {
            let visibility = if public { "pub" } else { "priv" };
            declared.push(format!("{name} [{visibility}] {signature}"));
        }
        for header in inventory_impl_headers(&code) {
            headers.push(format!("{name}: {header}"));
        }
    }
    declared.sort();
    headers.sort();

    assert_eq!(
        declared,
        DECLARATIONS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's declaration set changed"
    );
    assert_eq!(
        headers,
        IMPL_HEADERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's impl inventory changed"
    );
    Ok(())
}
