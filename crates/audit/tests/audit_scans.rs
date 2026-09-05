//! What a behavioural test cannot observe: that this crate's vocabularies are
//! the specification's, and that its product source reaches nothing it must not.
//!
//! Each scan below reads a file's text. `docs/contracts/policy-source-scans.md`
//! enumerates every such scan in this repository, and
//! `tools/policy-source-scan-inventory.test.mjs` executes that sentence, so a
//! new one has a row on that page or the inventory fails.
//!
//! The three that read the design document are the halves this task could most
//! easily have got wrong by writing a list twice: section 11.1's eight selector
//! inputs, section 3's profile keys, and section 38's open cells. Each is
//! compared **in both directions**, so a name dropped from the Rust side fails
//! against the document and a name the document does not carry fails against
//! the Rust side.

mod support;

use std::{collections::BTreeSet, fs, path::PathBuf};

use academic_audit::{DegreeMode, OpenGate, ProfileField, SelectorDimension};
use academic_domain::engines::ProofStatus;
use support::TestResult;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn specification() -> Result<String, Box<dyn std::error::Error>> {
    Ok(fs::read_to_string(repository_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// Every product file of this crate, recursively.
fn product_sources() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let base = repository_root().join("crates").join("audit").join("src");
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let name = path
                    .strip_prefix(&base)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((name, fs::read_to_string(&path)?));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Strips `//` comments and string literals, so a scan does not match prose.
fn stripped(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or_default();
        let mut in_string = false;
        let mut escaped = false;
        for character in code.chars() {
            match (in_string, escaped, character) {
                (_, true, _) => escaped = false,
                (true, false, '\\') => escaped = true,
                (_, false, '"') => in_string = !in_string,
                (false, false, _) => out.push(character),
                (true, false, _) => {}
            }
        }
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// The walk reaches every module
// ---------------------------------------------------------------------------

/// Every `pub mod` this crate declares is a file the sweeps below read.
///
/// The tripwire the other scans rest on: a module added without a file the walk
/// reaches would make every sweep below quietly narrower.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = product_sources()?;
    let names: BTreeSet<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
    let lib = sources
        .iter()
        .find(|(name, _)| name == "lib.rs")
        .map(|(_, text)| text.clone())
        .ok_or("the walk did not reach lib.rs")?;

    let mut declared = 0_usize;
    for line in lib.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("pub mod ") else {
            continue;
        };
        let module = rest.trim_end_matches(';');
        assert!(
            names.contains(format!("{module}.rs").as_str()),
            "{module} is declared and the walk does not read it"
        );
        declared += 1;
    }
    assert!(declared >= 12, "only {declared} modules were declared");
    assert_eq!(
        declared + 1,
        sources.len(),
        "the walk read a file no module declares, or missed one"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Section 11.1's eight selector inputs
// ---------------------------------------------------------------------------

/// The selector's dimensions are the specification's own words.
///
/// Section 11.1 writes them as one `·`-delimited list. This splits that list
/// out of the document and compares it with [`SelectorDimension::ALL`] in both
/// directions, in order. **Nothing here asserts how many there are**: dropping
/// a dimension *and* its declared length together still fails, because the
/// document still writes the unit.
#[test]
fn the_selector_dimensions_are_the_specifications_own() -> TestResult {
    let specification = specification()?;
    let sentence = specification
        .lines()
        .find(|line| line.starts_with("selector는 "))
        .ok_or("section 11.1's selector sentence is not in the document")?;
    let listed = sentence
        .strip_prefix("selector는 ")
        .and_then(|rest| rest.split_once("을 함께 사용한다"))
        .map(|(list, _)| list)
        .ok_or("the selector sentence no longer ends the way it did")?;

    let from_specification: Vec<&str> = listed.split('·').collect();
    let from_crate: Vec<&str> = SelectorDimension::ALL
        .into_iter()
        .map(SelectorDimension::spec_words)
        .collect();
    assert_eq!(
        from_specification, from_crate,
        "the selector's dimensions are not section 11.1's, in section 11.1's order"
    );

    // The sixth unit's `/`-separated alternatives are the degree modes, in the
    // same order. One is the yaml's own identifier; the other four are derived
    // by upper-casing the English name, and the derivation is what this pins.
    let sixth = from_specification
        .get(5)
        .ok_or("section 11.1's sixth unit is gone")?;
    let alternatives: Vec<&str> = sixth.split('/').collect();
    let modes: Vec<&str> = DegreeMode::ALL
        .into_iter()
        .map(DegreeMode::spec_word)
        .collect();
    assert_eq!(
        alternatives, modes,
        "the degree modes are not the sixth unit's alternatives"
    );

    // `SINGLE_MAJOR` is the yaml's own spelling and is compared against it.
    assert!(
        specification.contains("majorMode: SINGLE_MAJOR"),
        "section 11.1's yaml no longer spells SINGLE_MAJOR"
    );
    assert_eq!(DegreeMode::SingleMajor.as_str(), "SINGLE_MAJOR");

    // The split between the dimensions a published scope narrows and the two it
    // does not is the yaml's, so the yaml is required to declare exactly the
    // four fields that split rests on and no field for the other two.
    for field in [
        "institutionPath",
        "admissionYear",
        "selectedGraduationStandardRange",
        "majorMode",
    ] {
        assert!(
            specification.contains(&format!("{field}:")),
            "section 11.1's yaml no longer declares {field}"
        );
    }
    for absent in ["exchangeOrTransferScope", "exceptionApprovalScope"] {
        assert!(
            !specification.contains(absent),
            "the specification now declares {absent}; the narrowing split must move with it"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Section 3's profile keys
// ---------------------------------------------------------------------------

/// The profile's fields are section 3's own keys, and the three it leaves out
/// are left out on purpose.
#[test]
fn the_profile_fields_are_the_specifications_own() -> TestResult {
    let specification = specification()?;
    let block = specification
        .split_once("StudentProfile:")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("```"))
        .map(|(block, _)| block)
        .ok_or("section 3's StudentProfile block is not in the document")?;

    let keys: Vec<&str> = block
        .lines()
        .filter_map(|line| line.trim().split_once(':').map(|(key, _)| key.trim()))
        .filter(|key| !key.is_empty())
        .collect();
    assert!(keys.len() >= 11, "the profile block shrank to {keys:?}");

    // Every field that claims a section 3 key has one.
    let mut claimed = BTreeSet::new();
    for field in ProfileField::ALL {
        if let Some(key) = field.spec_key() {
            assert!(
                keys.contains(&key),
                "{field:?} claims section 3's {key} and section 3 does not write it"
            );
            assert!(claimed.insert(key), "{key} is claimed by two fields");
        }
    }

    // And every section 3 key is either claimed or deliberately not a selector
    // input. The three below select no rule: `gradingContext` is the versioned
    // `GradingScheme` bound to the grade-point reading, and `interests` and
    // `privacyPolicy` bear on no requirement set. A field that selects nothing
    // and was hashed into the audit identity would make an audit change when a
    // privacy preference did.
    let not_selector_inputs = ["gradingContext", "interests", "privacyPolicy"];
    for key in &keys {
        assert!(
            claimed.contains(key) || not_selector_inputs.contains(key),
            "section 3 writes {key} and nothing here claims it or excludes it"
        );
    }

    // One field has no section 3 key, and it is section 11.1's sentence that
    // names it. Recording the difference here is what stops this comparison
    // from needing an exception.
    let unkeyed: Vec<ProfileField> = ProfileField::ALL
        .into_iter()
        .filter(|field| field.spec_key().is_none())
        .collect();
    assert_eq!(unkeyed, vec![ProfileField::ExceptionApprovals]);
    assert_eq!(
        ProfileField::ExceptionApprovals.dimension(),
        SelectorDimension::ExceptionApproval
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Section 38's open cells
// ---------------------------------------------------------------------------

/// Every cell this crate leaves open is a line section 38 really writes.
#[test]
fn the_open_gates_are_section_38s_own() -> TestResult {
    let specification = specification()?;
    for gate in OpenGate::ALL {
        assert!(
            specification.contains(gate.spec_line()),
            "{} quotes a line section 38 does not write: {}",
            gate.identifier(),
            gate.spec_line()
        );
        assert!(gate.identifier().starts_with("GATE-38-"));
        assert!(!gate.statement().is_empty());
    }

    // Section 38.1's ten lines are numbered by position, and this crate's five
    // profile cells are its first four and its sixth. Reading the position back
    // out of the block is what makes the identifiers derived rather than
    // asserted.
    let block = specification
        .split_once("Admission Year")
        .map(|(_, rest)| format!("Admission Year{rest}"))
        .and_then(|rest| rest.split_once("```").map(|(block, _)| block.to_owned()))
        .ok_or("section 38.1's block is not in the document")?;
    let lines: Vec<&str> = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(lines.len(), 10, "section 38.1 lists {} lines", lines.len());

    for (position, gate) in [
        (0_usize, OpenGate::ProfileAdmissionYear),
        (1, OpenGate::ProfileGraduationStandard),
        (2, OpenGate::ProfileDegreeMode),
        (3, OpenGate::ProfileAdditionalMajor),
        (5, OpenGate::ProfileExchangeOrTransfer),
    ] {
        let line = lines
            .get(position)
            .ok_or_else(|| format!("section 38.1 has no line {position}"))?;
        assert!(
            line.starts_with(gate.spec_line()),
            "{} is section 38.1's line {} and that line reads {line}",
            gate.identifier(),
            position + 1
        );
        let expected = format!("GATE-38-{:03}", position + 1);
        assert_eq!(
            gate.identifier(),
            expected,
            "the identifier does not follow the line's position"
        );
    }

    // Section 38.2's bullets are numbered from eleven, continuing section
    // 38.1's ten. The two rule cells this crate forwards are its first two, and
    // the identifier is derived from the position here rather than compared
    // against a list written twice.
    let bullets_block = specification
        .split_once("### 38.2 공식적으로 추가 확인할 항목")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("### 38.3").map(|(block, _)| block))
        .ok_or("section 38.2's list is not in the document")?;
    let bullets: Vec<&str> = bullets_block
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .collect();
    assert_eq!(
        bullets.len(),
        11,
        "section 38.2 lists {} bullets",
        bullets.len()
    );
    for (position, gate) in [
        (0_usize, OpenGate::RuleCohortApplicability),
        (1, OpenGate::RuleThesisScope),
    ] {
        let bullet = bullets
            .get(position)
            .ok_or_else(|| format!("section 38.2 has no bullet {position}"))?;
        assert_eq!(
            *bullet,
            gate.spec_line(),
            "{} is section 38.2's bullet {}",
            gate.identifier(),
            position + 1
        );
        // Ten section 38.1 lines come first, so section 38.2's first bullet is
        // the eleventh cell.
        assert_eq!(gate.identifier(), format!("GATE-38-{:03}", position + 11));
    }

    // The two rule cells are `academic-requirement`'s own -- forwarded, not
    // restated.
    assert_eq!(
        OpenGate::from_rule_gate(academic_requirement::OpenGate::CohortApplicability),
        Some(OpenGate::RuleCohortApplicability)
    );
    assert_eq!(
        OpenGate::from_rule_gate(academic_requirement::OpenGate::ThesisRuleScope),
        Some(OpenGate::RuleThesisScope)
    );
    // And the two that stay that crate's map to no cell here.
    assert_eq!(
        OpenGate::from_rule_gate(academic_requirement::OpenGate::MultiMajorDoubleCounting),
        None
    );
    assert_eq!(
        OpenGate::from_rule_gate(academic_requirement::OpenGate::ExternalCreditRecognition),
        None
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Section 11.3's rendered vocabulary
// ---------------------------------------------------------------------------

/// Section 11.3's tree writes five leaf tokens, and each maps to exactly one
/// harness status.
///
/// The two lists are **not** the same five. `PASS_PARTIAL` is section 11.3's
/// and the harness has no such value; `CONFLICT` is the harness's and section
/// 11.3's example does not print one. The mapping below is where that is
/// written down, and the test refuses a token the mapping does not cover -- so
/// a specification edit that introduces a sixth reading fails here rather than
/// being folded into the nearest status.
#[test]
fn the_proof_statuses_cover_section_11_3s_own_tree() -> TestResult {
    let specification = specification()?;
    let tree = specification
        .split_once("DegreeAudit: INDETERMINATE")
        .map(|(_, rest)| format!("DegreeAudit: INDETERMINATE{rest}"))
        .and_then(|rest| rest.split_once("```").map(|(block, _)| block.to_owned()))
        .ok_or("section 11.3's tree is not in the document")?;

    // Every SCREAMING_SNAKE token the tree prints.
    let mut tokens: BTreeSet<String> = BTreeSet::new();
    let mut current = String::new();
    for character in tree.chars() {
        if character.is_ascii_uppercase() || character == '_' {
            current.push(character);
        } else {
            if current.len() >= 4 {
                tokens.insert(current.clone());
            }
            current.clear();
        }
    }
    if current.len() >= 4 {
        tokens.insert(current);
    }

    // The mapping, written down rather than derived.
    let mapping: Vec<(&str, Option<ProofStatus>)> = vec![
        // The root reading. Not a leaf status at all: it is the audit's
        // verdict, and `DegreeVerdict` is where it lives.
        ("INDETERMINATE", None),
        ("PASS", Some(ProofStatus::Satisfied)),
        // Section 11.3 labels two structurally identical credit rows
        // differently -- `93 / 130 PASS_PARTIAL` and `51 / 63 NEEDS 12`. They
        // are one reading: a floor short of its threshold, with the shortfall
        // quantified. The harness spells it `NEEDS` and `Measure` carries the
        // quantity, so both rows render as `NEEDS` here.
        ("PASS_PARTIAL", Some(ProofStatus::Needs)),
        ("NEEDS", Some(ProofStatus::Needs)),
        ("NOT_SATISFIED", Some(ProofStatus::NotSatisfied)),
        ("UNKNOWN", Some(ProofStatus::Unknown)),
    ];
    let covered: BTreeSet<&str> = mapping.iter().map(|(token, _)| *token).collect();
    for token in &tokens {
        assert!(
            covered.contains(token.as_str()),
            "section 11.3's tree prints {token} and the mapping does not cover it"
        );
    }
    for (token, _) in &mapping {
        assert!(
            tokens.contains(*token),
            "the mapping covers {token} and section 11.3's tree does not print it"
        );
    }

    // `CONFLICT` is the harness's fifth and section 11.3's example prints none.
    // Section 11.4 requires it anyway -- *unresolved conflict 0* -- and the
    // audit produces one, which `mixed_proof_tree` observes.
    assert!(
        !tokens.contains("CONFLICT"),
        "section 11.3's tree now prints CONFLICT; this note is stale"
    );
    let statuses: BTreeSet<&str> = ProofStatus::ALL
        .into_iter()
        .map(ProofStatus::as_str)
        .collect();
    assert!(statuses.contains("CONFLICT"));
    assert!(
        !statuses.contains("PASS_PARTIAL"),
        "the harness now has a PASS_PARTIAL; the mapping above must move"
    );

    // The reading the `PASS_PARTIAL` row of the mapping rests on: section 11.3
    // labels two structurally identical credit rows differently. Both are a
    // count short of a threshold -- `93 / 130` and `51 / 63` -- and one reads
    // `PASS_PARTIAL` while the other reads `NEEDS 12`. That is why both render
    // as `NEEDS` here, and if the document ever makes the two rows agree the
    // justification is stale rather than wrong, so it fails here instead of
    // being carried forward unread.
    let credit_rows: Vec<(u32, u32, String)> = tree
        .lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" / ")?;
            let have: u32 = before
                .rsplit(' ')
                .next()
                .and_then(|word| word.parse().ok())?;
            let mut rest = after.split_whitespace();
            let threshold: u32 = rest.next().and_then(|word| word.parse().ok())?;
            let token = rest.next()?.to_owned();
            Some((have, threshold, token))
        })
        .collect();
    assert_eq!(
        credit_rows.len(),
        2,
        "section 11.3 prints {} credit rows, not the two this mapping was written against: {credit_rows:?}",
        credit_rows.len()
    );
    for (have, threshold, _) in &credit_rows {
        assert!(
            have < threshold,
            "section 11.3's credit row {have} / {threshold} is no longer short of its threshold"
        );
    }
    let labels: Vec<&str> = credit_rows
        .iter()
        .map(|(_, _, token)| token.as_str())
        .collect();
    assert_ne!(
        labels.first(),
        labels.last(),
        "section 11.3 now labels its two credit rows the same way ({labels:?}); the mapping's PASS_PARTIAL note is stale"
    );

    // Both labels are in the mapping, and both send to the same status. That
    // is the whole of the resolution, checked rather than described.
    let rendered: Vec<Option<ProofStatus>> = labels
        .iter()
        .map(|label| {
            mapping
                .iter()
                .find(|(token, _)| token == label)
                .and_then(|(_, status)| *status)
        })
        .collect();
    assert_eq!(
        rendered,
        vec![Some(ProofStatus::Needs), Some(ProofStatus::Needs)],
        "section 11.3's two credit rows no longer render as one status: {labels:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// No clock, RNG, socket or model on the graduation path
// ---------------------------------------------------------------------------

/// The product source reaches no clock, RNG, socket or model.
///
/// The samples are run through the same check inside the test, so a rule that
/// matched nothing fails rather than passing over a source it cannot see.
#[test]
fn no_product_file_reaches_a_clock_rng_socket_or_model() -> TestResult {
    const FORBIDDEN: [&str; 14] = [
        "SystemTime",
        "Instant::now",
        "Utc::now",
        "Local::now",
        "chrono",
        "rand::",
        "thread_rng",
        "getrandom",
        "Uuid::now_v7",
        "Uuid::new_v4",
        "TcpStream",
        "UdpSocket",
        "reqwest",
        "ModelRun",
    ];

    let sources = product_sources()?;
    assert!(sources.len() >= 12, "the walk read {} files", sources.len());
    for (name, text) in &sources {
        let code = stripped(text);
        for spelling in FORBIDDEN {
            assert!(
                !code.contains(spelling),
                "{name} reaches {spelling} on the graduation path"
            );
        }
    }

    // The check bites. Each evasion below is a spelling the rule must catch,
    // run through the same predicate the sweep uses.
    for evasion in [
        "let now = std::time::SystemTime::now();",
        "let seed = rand::random::<u64>();",
        "let id = Uuid::new_v4();",
        "let socket = std::net::TcpStream::connect(host)?;",
        "let run: ModelRun = provider.execute(prompt)?;",
    ] {
        let code = stripped(evasion);
        assert!(
            FORBIDDEN.iter().any(|spelling| code.contains(spelling)),
            "the rule does not catch {evasion}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The isolations this crate rests on
// ---------------------------------------------------------------------------

/// No product file names a projection, and only one names a plan.
///
/// `P2-C7`'s seal works here by absence: there is no `academic-scenario` edge,
/// so a `Proposed<T>` is not a value a product file can hold. The plan types
/// are nameable -- `academic-record` is a product edge -- so the second half is
/// where they may be named, which is the module that turns one into labels and
/// nowhere else.
#[test]
fn no_product_file_names_a_projection_and_only_one_names_a_plan() -> TestResult {
    let sources = product_sources()?;
    let mut plan_sites = BTreeSet::new();
    for (name, text) in &sources {
        let code = stripped(text);
        for projection in [
            "academic_scenario",
            "Proposed<",
            "ProjectedEvidenceOpportunity",
        ] {
            assert!(
                !code.contains(projection),
                "{name} names {projection}; this crate has no projection edge"
            );
        }
        if code.contains("PlanScenario") || code.contains("PlannedCoursework") {
            plan_sites.insert(name.clone());
        }
    }
    assert_eq!(
        plan_sites,
        BTreeSet::from(["lib.rs".to_owned(), "plan.rs".to_owned()]),
        "a plan type is named outside the module that turns one into labels"
    );

    // `lib.rs` is in that set because it re-exports the type, and a re-export
    // is not a use. It is required to name it in exactly one place, and that
    // place is required to be the `pub use` line.
    let lib = sources
        .iter()
        .find(|(name, _)| name == "lib.rs")
        .map(|(_, text)| stripped(text))
        .ok_or("the walk did not reach lib.rs")?;
    let naming: Vec<&str> = lib
        .lines()
        .filter(|line| line.contains("PlannedCoursework") || line.contains("PlanScenario"))
        .collect();
    assert_eq!(naming.len(), 1, "lib.rs names a plan type more than once");
    assert!(
        naming
            .first()
            .is_some_and(|line| line.contains("PlanAnnotatedView, PlanNote, PlannedCoursework")),
        "lib.rs names a plan type outside its re-export"
    );

    // And the audit function itself has no plan parameter. Read off the source
    // rather than asserted, because the compile-fail case proves the absence
    // and this proves the case is still about the right function.
    let engine = sources
        .iter()
        .find(|(name, _)| name == "engine.rs")
        .map(|(_, text)| stripped(text))
        .ok_or("the walk did not reach engine.rs")?;
    let signature = engine
        .split_once("pub fn evaluate(")
        .and_then(|(_, rest)| rest.split_once(')').map(|(args, _)| args.to_owned()))
        .ok_or("DegreeAudit::evaluate is gone")?;
    for plan in ["plan", "Plan"] {
        assert!(
            !signature.contains(plan),
            "DegreeAudit::evaluate now takes a plan: {signature}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The three witnesses, and the one route to a determination
// ---------------------------------------------------------------------------

/// The three gates and the determination they assemble into.
const WITNESS_VALUES: [&str; 4] = [
    "CoverageWitness",
    "ConflictFreeWitness",
    "FreshnessWitness",
    "DeterminateVerdict",
];

/// Every expression in this crate that builds one of them.
///
/// One per value, each inside that value's own crate-private constructor. The
/// `impl From<..>` injection `P2-A3` used adds four entries here.
const WITNESS_CONSTRUCTIONS: [&str; 4] = [
    "ConflictFreeWitness verdict.rs Self",
    "CoverageWitness verdict.rs Self",
    "DeterminateVerdict verdict.rs Self",
    "FreshnessWitness verdict.rs Self",
];

/// The `impl` blocks in `code`, as a self type and the block's body.
///
/// The self type is the last path segment before the opening brace, so
/// `impl CoverageWitness`, `impl From<usize> for CoverageWitness` and
/// `impl core::fmt::Debug for CoverageWitness` all report the same one.
fn impl_blocks(code: &str) -> Vec<(String, String)> {
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
        let Some(open) = code[at..].find('{').map(|offset| at + offset) else {
            continue;
        };
        let header = &code[at + 4..open];
        let subject = header.rsplit(" for ").next().unwrap_or(header);
        let name: String = subject
            .chars()
            .filter(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        let mut depth = 0_i32;
        let mut end = open + 1;
        for (offset, character) in code[open + 1..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' if depth == 0 => {
                    end = open + 1 + offset;
                    break;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        found.push((name, code[open + 1..end].to_owned()));
    }
    found
}

/// Each gate has exactly one construction site, and the verdict has one route.
#[test]
fn the_three_witnesses_have_one_construction_site_each() -> TestResult {
    let sources = product_sources()?;
    let verdict = sources
        .iter()
        .find(|(name, _)| name == "verdict.rs")
        .map(|(_, text)| stripped(text))
        .ok_or("the walk did not reach verdict.rs")?;

    for witness in ["CoverageWitness", "ConflictFreeWitness", "FreshnessWitness"] {
        let declarations = verdict.matches(&format!("pub struct {witness}")).count();
        assert_eq!(
            declarations, 1,
            "{witness} is declared {declarations} times"
        );
    }
    let establishes = verdict.matches("pub(crate) fn establish").count();
    assert_eq!(establishes, 3, "there are {establishes} establish sites");

    // And the number of expressions that **build** a witness value, which is
    // the count the comment above this block used to claim and nothing
    // checked. `P2-A3` walked through exactly that gap: four `impl From<..>`
    // blocks added a construction site for each of the three witnesses and for
    // the verdict itself, and every assertion in this test still passed —
    // none of them counts a construction.
    //
    // A construction is a `Self {` inside an `impl` block whose self type is
    // the value, or a literal naming the type outside its declaration. `Self`
    // cannot be renamed and an `impl` block cannot hide its self type, so this
    // does not depend on how the site is spelled.
    let mut built: Vec<String> = Vec::new();
    for (name, text) in &sources {
        let code = stripped(text);
        for (subject, body) in impl_blocks(&code) {
            for value in WITNESS_VALUES {
                if subject != value {
                    continue;
                }
                for (at, _) in body.match_indices("Self {") {
                    // `-> Self {` is a return type and the brace that opens the
                    // function, not a construction.
                    if body[..at].trim_end().ends_with("->") {
                        continue;
                    }
                    built.push(format!("{value} {name} Self"));
                }
            }
        }
        for line in code.lines() {
            for value in WITNESS_VALUES {
                let trimmed = line.trim();
                let Some(at) = trimmed.find(&format!("{value} {{")) else {
                    continue;
                };
                let before = trimmed[..at].trim_end();
                // A declaration, an `impl` header, a trait's `for` clause and a
                // return type are all the type's *name*, not a value of it.
                if before.ends_with("struct")
                    || before.ends_with("impl")
                    || before.ends_with("for")
                    || before.ends_with("->")
                {
                    continue;
                }
                built.push(format!("{value} {name} literal"));
            }
        }
    }
    built.sort();
    assert_eq!(
        built,
        WITNESS_CONSTRUCTIONS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "the set of expressions that build a witness or a determination changed"
    );

    // No `pub fn establish` anywhere: a public one would be a witness a caller
    // could mint.
    assert!(
        !verdict.contains("pub fn establish"),
        "a witness can be established from outside this crate"
    );

    // The verdict's constructor takes all three by value, and is crate-private.
    let constructor = verdict
        .split_once("pub(crate) const fn new(\n        outcome: GraduationOutcome,")
        .and_then(|(_, rest)| {
            rest.split_once(") -> Self")
                .map(|(args, _)| args.to_owned())
        })
        .ok_or("DeterminateVerdict::new no longer has the pinned shape")?;
    for witness in [
        "coverage: CoverageWitness",
        "conflict_free: ConflictFreeWitness",
        "freshness: FreshnessWitness",
    ] {
        assert!(
            constructor.contains(witness),
            "DeterminateVerdict::new no longer takes {witness}"
        );
    }

    // The engine reaches a determination in exactly one expression, and that
    // expression is the three-witness match.
    let engine = sources
        .iter()
        .find(|(name, _)| name == "engine.rs")
        .map(|(_, text)| stripped(text))
        .ok_or("the walk did not reach engine.rs")?;
    assert_eq!(
        engine.matches("DegreeVerdict::Determinate").count(),
        1,
        "there is more than one route to a determination"
    );
    assert!(
        engine.contains("(Some(coverage), Some(conflict_free), Some(freshness))"),
        "the determination is no longer gated on all three witnesses"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// No default stands in for a value
// ---------------------------------------------------------------------------

/// Every `Default` in this crate is emptiness, never a value.
#[test]
fn the_only_defaults_are_empty_collections() -> TestResult {
    let sources = product_sources()?;
    let mut derived = BTreeSet::new();
    for (name, text) in &sources {
        let code = stripped(text);
        assert!(
            !code.contains("impl Default for"),
            "{name} hand-writes a Default; a hand-written one can carry a value"
        );
        for (index, line) in code.lines().enumerate() {
            if line.contains("#[derive(") && line.contains("Default") {
                // The type declared on the next non-attribute line.
                let declared = code
                    .lines()
                    .skip(index + 1)
                    .find(|following| {
                        following.trim_start().starts_with("pub struct")
                            || following.trim_start().starts_with("pub enum")
                    })
                    .and_then(|following| following.split_whitespace().nth(2))
                    .map(|word| word.trim_end_matches('{').trim_end_matches('<').to_owned())
                    .unwrap_or_else(|| format!("{name}:{index}"));
                derived.insert(declared);
            }
        }
    }
    assert_eq!(
        derived,
        BTreeSet::from([
            "CourseFactsIndex".to_owned(),
            "PlannedCoursework".to_owned(),
            "RuleSetCatalog".to_owned(),
            "RuleSourceIndex".to_owned(),
        ]),
        "a type gained a Default; every one here must be an empty collection"
    );

    // And each of the four defaults to a value that answers nothing rather than
    // to a value that answers something.
    assert!(
        academic_audit::RuleSourceIndex::new()
            .entries()
            .next()
            .is_none()
    );
    assert!(
        academic_audit::CourseFactsIndex::new()
            .facts("4190.101")
            .is_none()
    );
    assert!(academic_audit::RuleSetCatalog::new().entries().is_empty());
    assert!(academic_audit::PlannedCoursework::none().is_empty());
    Ok(())
}

/// The floor the inventory walk must reach, so an empty walk fails as a walk.
const INVENTORY_FILE_FLOOR: usize = 14;

/// Every function this package declares, as `<file> [vis] <signature>`.
const DECLARATIONS: &[&str] = &[
    "examples/emit_harness.rs [priv] fn main() -> Result<(), Box<dyn Error>>",
    "src/engine.rs [priv] fn academic_facts(facts: &AuditFacts) -> Result<academic_requirement::AcademicFacts, AuditError>",
    "src/engine.rs [priv] fn assemble( engine: &GraduationAuditEngine, facts: AuditFacts, inputs: &FrozenInputs, rule_set_hash: RuleSetHash, ) -> Result<Self, AuditError>",
    "src/engine.rs [priv] fn build_node( rule: &RuleId, body: &RuleBody, outcome: &RuleOutcome, span: &RuleSourceSpan, transcript: &TranscriptSnapshot, set: &RuleSet, academic: &academic_requirement::AcademicFacts, ) -> Result<AuditNode, AuditError>",
    "src/engine.rs [priv] fn collect_missing(outcome: &RuleOutcome, missing: &mut Vec<MissingCheck>)",
    "src/engine.rs [priv] fn engine_id(&self) -> &'static str",
    "src/engine.rs [priv] fn engine_version(&self) -> EngineVersion",
    "src/engine.rs [priv] fn evaluate( &self, inputs: &FrozenInputs, rule_set_hash: RuleSetHash, engine_version: EngineVersion, ) -> Result<EngineOutcome, EngineError>",
    "src/engine.rs [priv] fn fold(leaves: &[ProofLeaf]) -> ProofStatus",
    "src/engine.rs [priv] fn no_attempt_reason(rule_type: RuleType) -> NoAttemptReason",
    "src/engine.rs [priv] fn not_fresh(engine: &GraduationAuditEngine, facts: &AuditFacts) -> MissingCheck",
    "src/engine.rs [priv] fn operand_children( rule: &RuleId, operands: &[Operand], span: &RuleSourceSpan, transcript: &TranscriptSnapshot, set: &RuleSet, academic: &academic_requirement::AcademicFacts, ) -> Result<Vec<AuditNode>, AuditError>",
    "src/engine.rs [priv] fn outcome_of(status: ProofStatus) -> GraduationOutcome",
    "src/engine.rs [priv] fn proof_tree( nodes: &[AuditNode], facts: &AuditFacts, inputs: &FrozenInputs, ) -> Result<ProofNode, AuditError>",
    "src/engine.rs [priv] fn published_values( status: ProofStatus, leaves: &[ProofLeaf], verdict: &DegreeVerdict, ) -> BTreeMap<String, Decimal>",
    "src/engine.rs [priv] fn render(node: &AuditNode, facts: &AuditFacts) -> Result<ProofNode, AuditError>",
    "src/engine.rs [priv] fn root_inputs(inputs: &FrozenInputs) -> Result<Vec<InputKey>, AuditError>",
    "src/engine.rs [priv] fn source_index_digest(facts: &AuditFacts) -> ContentDigest",
    "src/engine.rs [priv] fn substitution( set: &RuleSet, transcript: &TranscriptSnapshot, academic: &academic_requirement::AcademicFacts, operand: &Operand, ) -> Option<(academic_domain::AttemptId, RuleId)>",
    "src/engine.rs [pub] fn audit_id(&self) -> Option<AuditId>",
    "src/engine.rs [pub] fn binding(&self) -> AuditInputBinding",
    "src/engine.rs [pub] fn canonical_text(self) -> String",
    "src/engine.rs [pub] fn children(&self) -> &[AuditNode]",
    "src/engine.rs [pub] fn credit_explanation(&self, rule: &RuleId) -> Option<&CreditExplanation>",
    "src/engine.rs [pub] fn credit_explanations(&self) -> &[CreditExplanation]",
    "src/engine.rs [pub] fn digest(self) -> ContentDigest",
    "src/engine.rs [pub] fn evaluate( engine: &GraduationAuditEngine, inputs: &FrozenInputs, ) -> Result<Self, AuditError>",
    "src/engine.rs [pub] fn evaluate_audit( &self, inputs: &FrozenInputs, rule_set_hash: RuleSetHash, ) -> Result<DegreeAudit, AuditError>",
    "src/engine.rs [pub] fn frozen_inputs_digest(self) -> ContentDigest",
    "src/engine.rs [pub] fn leaf(&self) -> &ProofLeaf",
    "src/engine.rs [pub] fn new(selected: SelectedRuleSet, version: EngineVersion) -> Self",
    "src/engine.rs [pub] fn node_id(&self) -> &NodeId",
    "src/engine.rs [pub] fn nodes(&self) -> &[AuditNode]",
    "src/engine.rs [pub] fn outcome(&self) -> &EngineOutcome",
    "src/engine.rs [pub] fn profile_digest(self) -> ContentDigest",
    "src/engine.rs [pub] fn root_status(&self) -> ProofStatus",
    "src/engine.rs [pub] fn rule_set_hash(&self) -> RuleSetHash",
    "src/engine.rs [pub] fn rule_set_hash(self) -> RuleSetHash",
    "src/engine.rs [pub] fn selected(&self) -> &SelectedRuleSet",
    "src/engine.rs [pub] fn source_index_digest(self) -> ContentDigest",
    "src/engine.rs [pub] fn transcript(&self) -> &TranscriptSnapshot",
    "src/engine.rs [pub] fn transcript_digest(self) -> ContentDigest",
    "src/engine.rs [pub] fn unevaluated(&self) -> &[RuleId]",
    "src/engine.rs [pub] fn verdict(&self) -> &DegreeVerdict",
    "src/engine.rs [pub] fn walk(&self) -> Vec<&AuditNode>",
    "src/engine.rs [pub] fn walk(&self) -> Vec<&Self>",
    "src/engine.rs [pub] fn with_audit_id(mut self, audit_id: AuditId) -> Self",
    "src/explain.rs [pub] fn attempt(&self) -> AttemptId",
    "src/explain.rs [pub] fn build( rule: RuleId, category: CreditCategory, source: RuleSourceSpan, transcript: &TranscriptSnapshot, ) -> Self",
    "src/explain.rs [pub] fn category(&self) -> &CreditCategory",
    "src/explain.rs [pub] fn course_code(&self) -> &str",
    "src/explain.rs [pub] fn included_credits(&self) -> u32",
    "src/explain.rs [pub] fn is_included(self) -> bool",
    "src/explain.rs [pub] fn kind(self) -> &'static str",
    "src/explain.rs [pub] fn lines(&self) -> &[CreditLine]",
    "src/explain.rs [pub] fn reason_text(self) -> String",
    "src/explain.rs [pub] fn rule(&self) -> &RuleId",
    "src/explain.rs [pub] fn source(&self) -> &RuleSourceSpan",
    "src/explain.rs [pub] fn verdict(&self) -> CreditVerdict",
    "src/facts.rs [priv] fn count(value: usize) -> Result<i64, AuditError>",
    "src/facts.rs [priv] fn decode_conflicts(inputs: &FrozenInputs) -> Result<Option<Vec<ConflictReference>>, AuditError>",
    "src/facts.rs [priv] fn decode_language(token: &str) -> Result<LanguageEvidence, AuditError>",
    "src/facts.rs [priv] fn decode_profile(inputs: &FrozenInputs) -> Result<StudentProfile, AuditError>",
    "src/facts.rs [priv] fn decode_reason(token: &str) -> Result<DispositionReason, AuditError>",
    "src/facts.rs [priv] fn decode_sources(inputs: &FrozenInputs) -> Result<RuleSourceIndex, AuditError>",
    "src/facts.rs [priv] fn decode_transcript(inputs: &FrozenInputs) -> Result<TranscriptSnapshot, AuditError>",
    "src/facts.rs [priv] fn digest_from_reference(value: &str) -> Result<ContentDigest, AuditError>",
    "src/facts.rs [priv] fn digest_reference(digest: ContentDigest) -> String",
    "src/facts.rs [priv] fn encode_conflicts( conflicts: Option<&[ConflictReference]>, push: &mut impl FnMut(String, InputValue) -> Result<(), AuditError>, ) -> Result<(), AuditError>",
    "src/facts.rs [priv] fn encode_profile( profile: &StudentProfile, push: &mut impl FnMut(String, InputValue) -> Result<(), AuditError>, ) -> Result<(), AuditError>",
    "src/facts.rs [priv] fn encode_sources( sources: &RuleSourceIndex, push: &mut impl FnMut(String, InputValue) -> Result<(), AuditError>, ) -> Result<(), AuditError>",
    "src/facts.rs [priv] fn encode_transcript( transcript: &TranscriptSnapshot, push: &mut impl FnMut(String, InputValue) -> Result<(), AuditError>, ) -> Result<(), AuditError>",
    "src/facts.rs [priv] fn index_count(value: i64) -> Result<usize, AuditError>",
    "src/facts.rs [priv] fn optional_integer(inputs: &FrozenInputs, key: &str) -> Result<Option<i64>, AuditError>",
    "src/facts.rs [priv] fn optional_reference(inputs: &FrozenInputs, key: &str) -> Result<Option<String>, AuditError>",
    "src/facts.rs [priv] fn reference(value: Option<&str>) -> InputValue",
    "src/facts.rs [priv] fn required_decimal(inputs: &FrozenInputs, key: &str) -> Result<Decimal, AuditError>",
    "src/facts.rs [priv] fn required_integer(inputs: &FrozenInputs, key: &str) -> Result<i64, AuditError>",
    "src/facts.rs [priv] fn required_reference(inputs: &FrozenInputs, key: &str) -> Result<String, AuditError>",
    "src/facts.rs [priv] fn value_of<'inputs>( inputs: &'inputs FrozenInputs, key: &str, ) -> Result<&'inputs InputValue, AuditError>",
    "src/facts.rs [pub] fn decode(inputs: &FrozenInputs) -> Result<AuditFacts, AuditError>",
    "src/facts.rs [pub] fn encode(facts: &AuditFacts) -> Result<FrozenInputs, AuditError>",
    "src/facts.rs [pub] fn entry_keys(index: usize) -> Vec<String>",
    "src/gate.rs [pub] fn from_rule_gate(gate: RuleGate) -> Option<Self>",
    "src/gate.rs [pub] fn identifier(self) -> &'static str",
    "src/gate.rs [pub] fn spec_line(self) -> &'static str",
    "src/gate.rs [pub] fn statement(self) -> &'static str",
    "src/leaf.rs [pub] fn as_str(self) -> &'static str",
    "src/leaf.rs [pub] fn attempts(&self) -> &AttemptUsage",
    "src/leaf.rs [pub] fn attempts(&self) -> &[AttemptId]",
    "src/leaf.rs [pub] fn canonical_text(&self) -> String",
    "src/leaf.rs [pub] fn canonical_text(&self) -> String",
    "src/leaf.rs [pub] fn equivalency(&self) -> &EquivalencyDecision",
    "src/leaf.rs [pub] fn is_complete(&self) -> bool",
    "src/leaf.rs [pub] fn measure(&self) -> Option<Measure>",
    "src/leaf.rs [pub] fn new( rule: RuleId, source: RuleSourceSpan, attempts: AttemptUsage, equivalency: EquivalencyDecision, rule_type: RuleType, status: ProofStatus, measure: Option<Measure>, open_gate: Option<OpenGate>, rule_gate: Option<RuleGate>, ) -> Self",
    "src/leaf.rs [pub] fn of(attempts: Vec<AttemptId>, when_empty: NoAttemptReason) -> Self",
    "src/leaf.rs [pub] fn of(rules: Vec<RuleId>) -> Self",
    "src/leaf.rs [pub] fn open_gate(&self) -> Option<OpenGate>",
    "src/leaf.rs [pub] fn rule(&self) -> &RuleId",
    "src/leaf.rs [pub] fn rule_gate(&self) -> Option<RuleGate>",
    "src/leaf.rs [pub] fn rule_type(&self) -> RuleType",
    "src/leaf.rs [pub] fn rules(&self) -> &[RuleId]",
    "src/leaf.rs [pub] fn source(&self) -> &RuleSourceSpan",
    "src/leaf.rs [pub] fn status(&self) -> ProofStatus",
    "src/plan.rs [pub] fn as_str(self) -> &'static str",
    "src/plan.rs [pub] fn course_codes(&self) -> impl Iterator<Item = &str>",
    "src/plan.rs [pub] fn from_scenario(scenario: &PlanScenario) -> Self",
    "src/plan.rs [pub] fn intended_term(&self, course_code: &str) -> Option<TermKey>",
    "src/plan.rs [pub] fn is_empty(&self) -> bool",
    "src/plan.rs [pub] fn new( audit: &'audit crate::engine::DegreeAudit, plan: &'audit PlannedCoursework, ) -> Self",
    "src/plan.rs [pub] fn none() -> Self",
    "src/plan.rs [pub] fn note_for(&self, course_code: &str) -> PlanNote",
    "src/plan.rs [pub] fn planned_only(&self) -> Vec<&'audit str>",
    "src/profile.rs [priv] fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result",
    "src/profile.rs [priv] fn is_identifier(value: &str) -> bool",
    "src/profile.rs [priv] fn render<T>(value: Option<&T>, accessor: fn(&T) -> &str) -> String",
    "src/profile.rs [priv] fn rendered_field(&self, field: ProfileField) -> String",
    "src/profile.rs [priv] fn unknown() -> String",
    "src/profile.rs [pub] fn action(self) -> &'static str",
    "src/profile.rs [pub] fn additional_majors(&self) -> &Recorded<Vec<ProgrammeId>>",
    "src/profile.rs [pub] fn admission_year(&self) -> &Recorded<AdmissionYear>",
    "src/profile.rs [pub] fn as_str(&self) -> &str",
    "src/profile.rs [pub] fn as_str(self) -> &'static str",
    "src/profile.rs [pub] fn as_str(self) -> &'static str",
    "src/profile.rs [pub] fn canonical_text(&self) -> String",
    "src/profile.rs [pub] fn college(&self) -> &Recorded<InstitutionId>",
    "src/profile.rs [pub] fn degree_mode(&self) -> &Recorded<DegreeMode>",
    "src/profile.rs [pub] fn department(&self) -> &Recorded<InstitutionId>",
    "src/profile.rs [pub] fn digest(&self) -> ContentDigest",
    "src/profile.rs [pub] fn dimension(self) -> SelectorDimension",
    "src/profile.rs [pub] fn exception_approvals(&self) -> &Recorded<Vec<ApprovalFact>>",
    "src/profile.rs [pub] fn exchange_or_transfer(&self) -> &Recorded<ExchangeOrTransfer>",
    "src/profile.rs [pub] fn gate(self) -> Option<OpenGate>",
    "src/profile.rs [pub] fn graduation_standard(&self) -> &Recorded<GraduationStandard>",
    "src/profile.rs [pub] fn is_known(&self) -> bool",
    "src/profile.rs [pub] fn is_recorded(&self, field: ProfileField) -> bool",
    "src/profile.rs [pub] fn known(&self) -> Option<&T>",
    "src/profile.rs [pub] fn narrows_the_catalogue(self) -> bool",
    "src/profile.rs [pub] fn new(value: &str) -> Result<Self, AuditError>",
    "src/profile.rs [pub] fn spec_key(self) -> Option<&'static str>",
    "src/profile.rs [pub] fn spec_word(self) -> &'static str",
    "src/profile.rs [pub] fn spec_words(self) -> &'static str",
    "src/profile.rs [pub] fn university(&self) -> &Recorded<InstitutionId>",
    "src/profile.rs [pub] fn unrecorded() -> Self",
    "src/profile.rs [pub] fn with_additional_majors(mut self, value: Vec<ProgrammeId>) -> Self",
    "src/profile.rs [pub] fn with_admission_year(mut self, value: AdmissionYear) -> Self",
    "src/profile.rs [pub] fn with_college(mut self, value: InstitutionId) -> Self",
    "src/profile.rs [pub] fn with_degree_mode(mut self, value: DegreeMode) -> Self",
    "src/profile.rs [pub] fn with_department(mut self, value: InstitutionId) -> Self",
    "src/profile.rs [pub] fn with_exception_approvals(mut self, value: Vec<ApprovalFact>) -> Self",
    "src/profile.rs [pub] fn with_exchange_or_transfer(mut self, value: ExchangeOrTransfer) -> Self",
    "src/profile.rs [pub] fn with_graduation_standard(mut self, value: GraduationStandard) -> Self",
    "src/profile.rs [pub] fn with_university(mut self, value: InstitutionId) -> Self",
    "src/select.rs [priv] fn render(value: Option<&InstitutionId>) -> String",
    "src/select.rs [priv] fn rendered_profile(profile: &StudentProfile) -> String",
    "src/select.rs [pub] fn admission_year(&self) -> AdmissionYear",
    "src/select.rs [pub] fn canonical_text(&self) -> String",
    "src/select.rs [pub] fn category(&self) -> &CreditCategory",
    "src/select.rs [pub] fn college(&self) -> &InstitutionId",
    "src/select.rs [pub] fn covers(&self, profile: &StudentProfile) -> Option<bool>",
    "src/select.rs [pub] fn department(&self) -> &InstitutionId",
    "src/select.rs [pub] fn entries(&self) -> &[CatalogEntry]",
    "src/select.rs [pub] fn floors(&self) -> &[CommonRuleExample]",
    "src/select.rs [pub] fn major_mode(&self) -> DegreeMode",
    "src/select.rs [pub] fn missing(&self) -> &[MissingCheck]",
    "src/select.rs [pub] fn new( university: InstitutionId, college: InstitutionId, department: InstitutionId, admission_year: AdmissionYear, standard_from: GraduationStandard, standard_to: GraduationStandard, major_mode: DegreeMode, ) -> Result<Self, AuditError>",
    "src/select.rs [pub] fn new() -> Self",
    "src/select.rs [pub] fn new(scope: RuleSetScope, rules: RuleSet) -> Self",
    "src/select.rs [pub] fn of(rules: &RuleSet) -> Result<Self, AuditError>",
    "src/select.rs [pub] fn rule(&self) -> &RuleId",
    "src/select.rs [pub] fn rules(&self) -> &RuleSet",
    "src/select.rs [pub] fn rules(&self) -> &RuleSet",
    "src/select.rs [pub] fn scope(&self) -> &RuleSetScope",
    "src/select.rs [pub] fn scope(&self) -> &RuleSetScope",
    "src/select.rs [pub] fn select(profile: &StudentProfile, catalog: &RuleSetCatalog) -> Selection",
    "src/select.rs [pub] fn selected(&self) -> Option<&SelectedRuleSet>",
    "src/select.rs [pub] fn standard_range(&self) -> (&GraduationStandard, &GraduationStandard)",
    "src/select.rs [pub] fn threshold(&self) -> u16",
    "src/select.rs [pub] fn university(&self) -> &InstitutionId",
    "src/select.rs [pub] fn version(&self) -> RuleSetVersion",
    "src/select.rs [pub] fn with(mut self, entry: CatalogEntry) -> Self",
    "src/source.rs [pub] fn artifact(&self) -> ArtifactId",
    "src/source.rs [pub] fn canonical_text(&self) -> String",
    "src/source.rs [pub] fn entries(&self) -> impl Iterator<Item = (&RuleId, &RuleSourceSpan)>",
    "src/source.rs [pub] fn locators(&self) -> Vec<SourceLocator>",
    "src/source.rs [pub] fn new( artifact: ArtifactId, source_digest: ContentDigest, page: u32, paragraph_start: u64, paragraph_end: u64, ) -> Result<Self, AuditError>",
    "src/source.rs [pub] fn new() -> Self",
    "src/source.rs [pub] fn page(&self) -> u32",
    "src/source.rs [pub] fn paragraph(&self) -> (u64, u64)",
    "src/source.rs [pub] fn placement(&self, rules: &RuleSet, rule: &RuleId) -> Placement<'_>",
    "src/source.rs [pub] fn source_digest(&self) -> ContentDigest",
    "src/source.rs [pub] fn with(mut self, rule: RuleId, span: RuleSourceSpan) -> Self",
    "src/transcript.rs [priv] fn admission_text(&self) -> String",
    "src/transcript.rs [priv] fn as_attempt(entity: EntityId) -> Option<AttemptId>",
    "src/transcript.rs [priv] fn as_entity(attempt: AttemptId) -> Result<EntityId, AuditError>",
    "src/transcript.rs [priv] fn canonical_text(&self) -> String",
    "src/transcript.rs [priv] fn category_text(&self) -> String",
    "src/transcript.rs [priv] fn decoded( attempt: AttemptId, course_code: String, course: CourseId, term: TermKey, record_status: RecordAttemptStatus, admission: EntryAdmission, categories: Vec<CreditCategory>, area: Option<AreaId>, is_major: bool, language: LanguageEvidence, ) -> Self",
    "src/transcript.rs [priv] fn decoded( entries: Vec<TranscriptEntry>, readings: BTreeMap<String, GpaReading>, ) -> Self",
    "src/transcript.rs [priv] fn language_token(language: LanguageEvidence) -> &'static str",
    "src/transcript.rs [priv] fn reading_over<'a>( dispositions: impl Iterator<Item = &'a AttemptDisposition>, ) -> Result<Option<GpaReading>, AuditError>",
    "src/transcript.rs [priv] fn reason_token(reason: DispositionReason) -> &'static str",
    "src/transcript.rs [priv] fn rendered(value: Decimal) -> String",
    "src/transcript.rs [priv] fn term_ordinal(term: TermKey) -> TermOrdinal",
    "src/transcript.rs [priv] fn whole_credits(credits: Decimal, attempt: AttemptId) -> Result<CreditAmount, AuditError>",
    "src/transcript.rs [priv] fn whole_denominator(credits: Decimal) -> Result<u32, AuditError>",
    "src/transcript.rs [priv] fn whole_units(value: Decimal) -> Option<i128>",
    "src/transcript.rs [pub] fn admission(&self) -> EntryAdmission",
    "src/transcript.rs [pub] fn area(&self) -> Option<&AreaId>",
    "src/transcript.rs [pub] fn as_rule_fact(&self) -> Result<AttemptFact, AuditError>",
    "src/transcript.rs [pub] fn as_str(self) -> &'static str",
    "src/transcript.rs [pub] fn attempt(&self) -> AttemptId",
    "src/transcript.rs [pub] fn canonical_text(&self) -> String",
    "src/transcript.rs [pub] fn categories(&self) -> &[CreditCategory]",
    "src/transcript.rs [pub] fn course(&self) -> CourseId",
    "src/transcript.rs [pub] fn course_code(&self) -> &str",
    "src/transcript.rs [pub] fn digest(&self) -> ContentDigest",
    "src/transcript.rs [pub] fn entries(&self) -> &[TranscriptEntry]",
    "src/transcript.rs [pub] fn facts(&self, course_code: &str) -> Option<&CourseRequirementFacts>",
    "src/transcript.rs [pub] fn from_record( history: &AttemptHistory, classification: &ClassificationRuleSet, rules: &RuleBook, primary_program: &RecordProgramId, courses: &CourseFactsIndex, ) -> Result<Self, AuditError>",
    "src/transcript.rs [pub] fn is_major(&self) -> bool",
    "src/transcript.rs [pub] fn language(&self) -> LanguageEvidence",
    "src/transcript.rs [pub] fn new() -> Self",
    "src/transcript.rs [pub] fn pending(&self) -> Vec<&TranscriptEntry>",
    "src/transcript.rs [pub] fn reading(&self, scope: &GpaScope) -> Option<GpaReading>",
    "src/transcript.rs [pub] fn readings(&self) -> impl Iterator<Item = (&String, &GpaReading)>",
    "src/transcript.rs [pub] fn reason(self) -> DispositionReason",
    "src/transcript.rs [pub] fn record_status(&self) -> RecordAttemptStatus",
    "src/transcript.rs [pub] fn rule_status(&self) -> RuleAttemptStatus",
    "src/transcript.rs [pub] fn term(&self) -> TermKey",
    "src/transcript.rs [pub] fn with(mut self, course_code: impl Into<String>, facts: CourseRequirementFacts) -> Self",
    "src/verdict.rs [priv] fn coverage_refuses_an_empty_leaf_set()",
    "src/verdict.rs [priv] fn decoded( rule: String, left_connector: String, right_connector: String, resolved: bool, ) -> Self",
    "src/verdict.rs [priv] fn establish( leaves: &[ProofLeaf], cases: Option<&[&ConflictReference]>, ) -> Option<Self>",
    "src/verdict.rs [priv] fn establish( policy: Option<SourceFreshnessPolicy>, retrieved_at: RetrievalInstant, as_of: TimestampMillis, ) -> Option<Self>",
    "src/verdict.rs [priv] fn establish(leaves: &[ProofLeaf], unevaluated: &[RuleId]) -> Option<Self>",
    "src/verdict.rs [priv] fn freshness_refuses_a_retrieval_after_the_audit_instant()",
    "src/verdict.rs [priv] fn from_checks(missing: Vec<MissingCheck>) -> Option<Self>",
    "src/verdict.rs [priv] fn new( outcome: GraduationOutcome, coverage: CoverageWitness, conflict_free: ConflictFreeWitness, freshness: FreshnessWitness, ) -> Self",
    "src/verdict.rs [priv] fn new(first: MissingCheck, rest: Vec<MissingCheck>) -> Self",
    "src/verdict.rs [priv] fn the_conflict_gate_separates_an_unread_store_from_an_empty_one()",
    "src/verdict.rs [pub] fn action(&self) -> String",
    "src/verdict.rs [pub] fn age_seconds(self) -> u64",
    "src/verdict.rs [pub] fn as_str(&self) -> &'static str",
    "src/verdict.rs [pub] fn as_str(self) -> &'static str",
    "src/verdict.rs [pub] fn canonical_text(&self) -> String",
    "src/verdict.rs [pub] fn cases_examined(self) -> usize",
    "src/verdict.rs [pub] fn conflict_free(self) -> ConflictFreeWitness",
    "src/verdict.rs [pub] fn coverage(self) -> CoverageWitness",
    "src/verdict.rs [pub] fn determinate(&self) -> Option<DeterminateVerdict>",
    "src/verdict.rs [pub] fn dimension(&self) -> Option<SelectorDimension>",
    "src/verdict.rs [pub] fn freshness(self) -> FreshnessWitness",
    "src/verdict.rs [pub] fn is_resolved(&self) -> bool",
    "src/verdict.rs [pub] fn kind(&self) -> &'static str",
    "src/verdict.rs [pub] fn left_connector(&self) -> &str",
    "src/verdict.rs [pub] fn limit_seconds(self) -> u64",
    "src/verdict.rs [pub] fn max_age_seconds(max_age_seconds: u64) -> Self",
    "src/verdict.rs [pub] fn missing(&self) -> &[MissingCheck]",
    "src/verdict.rs [pub] fn missing(&self) -> &[MissingCheck]",
    "src/verdict.rs [pub] fn of(case: &ConflictCase) -> Self",
    "src/verdict.rs [pub] fn outcome(self) -> GraduationOutcome",
    "src/verdict.rs [pub] fn right_connector(&self) -> &str",
    "src/verdict.rs [pub] fn rule(&self) -> &str",
    "src/verdict.rs [pub] fn rules_covered(self) -> usize",
];

/// Every `impl` block header this package ships, as `<file>: <header>`.
const IMPL_HEADERS: &[&str] = &[
    "src/engine.rs: impl AuditInputBinding",
    "src/engine.rs: impl AuditNode",
    "src/engine.rs: impl DegreeAudit",
    "src/engine.rs: impl DeterministicEngine for GraduationAuditEngine",
    "src/engine.rs: impl GraduationAuditEngine",
    "src/explain.rs: impl CreditExplanation",
    "src/explain.rs: impl CreditLine",
    "src/explain.rs: impl CreditVerdict",
    "src/facts.rs: impl FnMut(String, InputValue) -> Result<(), AuditError>, ) -> Result<(), AuditError>",
    "src/facts.rs: impl FnMut(String, InputValue) -> Result<(), AuditError>, ) -> Result<(), AuditError>",
    "src/facts.rs: impl FnMut(String, InputValue) -> Result<(), AuditError>, ) -> Result<(), AuditError>",
    "src/facts.rs: impl FnMut(String, InputValue) -> Result<(), AuditError>, ) -> Result<(), AuditError>",
    "src/gate.rs: impl OpenGate",
    "src/leaf.rs: impl AttemptUsage",
    "src/leaf.rs: impl EquivalencyDecision",
    "src/leaf.rs: impl NoAttemptReason",
    "src/leaf.rs: impl ProofLeaf",
    "src/plan.rs: impl Iterator<Item = &str>",
    "src/plan.rs: impl PlanNote",
    "src/plan.rs: impl PlannedCoursework",
    "src/plan.rs: impl<'audit> PlanAnnotatedView<'audit>",
    "src/profile.rs: impl $name",
    "src/profile.rs: impl DegreeMode",
    "src/profile.rs: impl ProfileField",
    "src/profile.rs: impl SelectorDimension",
    "src/profile.rs: impl StudentProfile",
    "src/profile.rs: impl core::fmt::Display for $name",
    "src/profile.rs: impl<T> Recorded<T>",
    "src/select.rs: impl CatalogEntry",
    "src/select.rs: impl CommonRuleExample",
    "src/select.rs: impl CommonRuleExamples",
    "src/select.rs: impl RuleSetCatalog",
    "src/select.rs: impl RuleSetScope",
    "src/select.rs: impl SelectedRuleSet",
    "src/select.rs: impl Selection",
    "src/source.rs: impl Iterator<Item = (&RuleId, &RuleSourceSpan)>",
    "src/source.rs: impl RuleSourceIndex",
    "src/source.rs: impl RuleSourceSpan",
    "src/transcript.rs: impl CourseFactsIndex",
    "src/transcript.rs: impl EntryAdmission",
    "src/transcript.rs: impl Into<String>, facts: CourseRequirementFacts) -> Self",
    "src/transcript.rs: impl Iterator<Item = &'a AttemptDisposition>, ) -> Result<Option<GpaReading>, AuditError>",
    "src/transcript.rs: impl Iterator<Item = (&String, &GpaReading)>",
    "src/transcript.rs: impl TranscriptEntry",
    "src/transcript.rs: impl TranscriptSnapshot",
    "src/verdict.rs: impl ConflictFreeWitness",
    "src/verdict.rs: impl ConflictReference",
    "src/verdict.rs: impl CoverageWitness",
    "src/verdict.rs: impl DegreeVerdict",
    "src/verdict.rs: impl DeterminateVerdict",
    "src/verdict.rs: impl FreshnessWitness",
    "src/verdict.rs: impl GraduationOutcome",
    "src/verdict.rs: impl IndeterminateVerdict",
    "src/verdict.rs: impl MissingCheck",
    "src/verdict.rs: impl SourceFreshnessPolicy",
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

// ---------------------------------------------------------------------------
// the_scans_these_documents_enumerate_are_the_ones_this_file_holds
// ---------------------------------------------------------------------------

/// The number words the two contracts use for a count of things in this
/// repository.
///
/// Spelled out rather than digits because that is how both documents write
/// them, and a count nobody can read back is a count nobody checks:
/// `policy-source-scans.md` said *nine scans* in one paragraph while its own
/// injection matrix named ten, and `graduation-audit.md` repeated the nine.
const NUMBER_WORDS: [(&str, usize); 12] = [
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
];

/// The number word standing where `{}` is in `template`, resolved to a count.
fn stated_count(document: &str, prefix: &str, suffix: &str) -> Option<usize> {
    let at = document.find(prefix)? + prefix.len();
    let rest = &document[at..];
    let end = rest.find(suffix)?;
    let word = rest[..end].trim();
    NUMBER_WORDS
        .iter()
        .find(|(spelling, _)| *spelling == word)
        .map(|(_, count)| *count)
}

/// Every backticked identifier in a fragment that looks like a test name.
fn backticked_names(fragment: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for piece in fragment.split('`').skip(1).step_by(2) {
        if piece.starts_with("the_")
            || piece.starts_with("no_")
            || piece.starts_with("every_")
            || piece.starts_with("each_")
        {
            names.insert(piece.to_owned());
        }
    }
    names
}

/// The scans this file declares are the scans the two contracts enumerate, and
/// the number each states is that many.
///
/// Three comparisons and two counts, all against the `#[test]` functions this
/// file actually declares. `P2-A3`'s second audit measured the drift this
/// closes: two documents said *nine scans* while the file held ten and the
/// injection matrix in one of those documents named all ten, so the same page
/// contradicted itself and a reader auditing "the nine scans" never asked what
/// the tenth compared.
///
/// A count is not asserted anywhere here. The number word is read out of the
/// prose and the names are read out of the two tables, so a scan added without
/// a row fails, a row naming a scan that does not exist fails, and a number
/// word that stops matching fails.
#[test]
fn the_scans_these_documents_enumerate_are_the_ones_this_file_holds() -> TestResult {
    let own = fs::read_to_string(
        repository_root()
            .join("crates")
            .join("audit")
            .join("tests")
            .join("audit_scans.rs"),
    )?;
    let mut declared: BTreeSet<String> = BTreeSet::new();
    let mut lines = own.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim() != "#[test]" {
            continue;
        }
        let signature = lines.peek().ok_or("a #[test] with nothing after it")?;
        let name = signature
            .trim()
            .strip_prefix("fn ")
            .and_then(|rest| rest.split('(').next())
            .ok_or("a #[test] that is not followed by a function")?;
        declared.insert(name.to_owned());
    }
    assert!(
        declared.contains("the_scans_these_documents_enumerate_are_the_ones_this_file_holds"),
        "the reader did not find this test, so it found nothing"
    );

    let scans = fs::read_to_string(
        repository_root()
            .join("docs")
            .join("contracts")
            .join("policy-source-scans.md"),
    )?;
    let audit = fs::read_to_string(
        repository_root()
            .join("docs")
            .join("contracts")
            .join("graduation-audit.md"),
    )?;

    // The two number words.
    assert_eq!(
        stated_count(
            &scans,
            "`crates/audit/tests/audit_scans.rs` holds ",
            " scans."
        ),
        Some(declared.len()),
        "policy-source-scans.md states a different number of scans than this file declares"
    );
    assert_eq!(
        stated_count(
            &audit,
            "- `crates/audit/tests/audit_scans.rs` holds the ",
            " source scans,"
        ),
        Some(declared.len()),
        "graduation-audit.md states a different number of scans than this file declares"
    );

    // The injection matrix's own row for this file.
    let marker = " — `crates/audit/tests/audit_scans.rs` |";
    let row = scans
        .lines()
        .find(|line| line.contains(marker))
        .ok_or("policy-source-scans.md has no injection-matrix row for this file")?;
    let listed = backticked_names(row.split(marker).next().unwrap_or_default());
    assert_eq!(
        listed, declared,
        "the injection matrix names a different set of scans than this file declares"
    );

    // The "What the `P2-U3` scans hold" table.
    let heading = "## What the `P2-U3` scans hold";
    let section_at = scans
        .find(heading)
        .ok_or("policy-source-scans.md has no P2-U3 scan table")?;
    let section = &scans[section_at + heading.len()..];
    let section = &section[..section.find("\n### ").unwrap_or(section.len())];
    let mut tabled: BTreeSet<String> = BTreeSet::new();
    for line in section.lines() {
        let Some(first) = line
            .strip_prefix("| `")
            .and_then(|rest| rest.split('`').next())
        else {
            continue;
        };
        tabled.insert(first.to_owned());
    }
    assert_eq!(
        tabled, declared,
        "the P2-U3 scan table names a different set of scans than this file declares"
    );
    Ok(())
}
