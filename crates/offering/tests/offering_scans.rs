//! What a behavioural test cannot observe: that this crate's vocabularies are
//! the specification's, and that its product source reaches nothing it must
//! not.
//!
//! Each scan below reads a file's text. `docs/contracts/policy-source-scans.md`
//! enumerates every such scan in this repository, and
//! `tools/policy-source-scan-inventory.test.mjs` executes that sentence, so a
//! new one has a row on that page or the inventory fails.
//!
//! The four that read the design document are the halves this task could most
//! easily have got wrong by writing a list twice: section 8.3's four status
//! rows, its six named features, its three `UNCERTAIN` grounds, and section
//! 38.2's seventh bullet. Each is compared **in both directions**, so a name
//! dropped from the Rust side fails against the document and a name the
//! document does not carry fails against the Rust side.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use academic_curriculum::OfferingStatus;
use academic_offering::{
    AbstentionReason, FeatureFamily, OpenGate,
    standing::{
        CancelledStanding, ConfirmedStanding, HistoricallyLikelyStanding, UncertainStanding,
    },
};
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
    let base = repository_root()
        .join("crates")
        .join("offering")
        .join("src");
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push((relative_name(&base, &path)?, fs::read_to_string(&path)?));
            }
        }
    }
    found.sort();
    Ok(found)
}

fn relative_name(base: &Path, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(path
        .strip_prefix(base)?
        .to_string_lossy()
        .replace('\\', "/"))
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
// Section 8.3's four rows
// ---------------------------------------------------------------------------

/// One row of section 8.3's table.
struct SpecRow {
    status: String,
    requirement: String,
    ui_copy: String,
    planner: String,
}

/// Section 8.3's table, read out of the design document.
fn section_8_3_rows() -> Result<Vec<SpecRow>, Box<dyn std::error::Error>> {
    let specification = specification()?;
    let block = specification
        .split_once("### 8.3 개설 상태")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("공식 개설 확인은").map(|(block, _)| block))
        .ok_or("section 8.3's table is not in the document")?;
    let mut rows = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || trimmed.starts_with("|---") {
            continue;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| {
                cell.trim()
                    .trim_matches('`')
                    .trim_matches('“')
                    .trim_matches('”')
                    .to_owned()
            })
            .collect();
        let [status, requirement, ui_copy, planner] = cells.as_slice() else {
            continue;
        };
        if status == "상태" {
            continue;
        }
        rows.push(SpecRow {
            status: status.clone(),
            requirement: requirement.clone(),
            ui_copy: ui_copy.clone(),
            planner: planner.clone(),
        });
    }
    Ok(rows)
}

/// The four standings are section 8.3's four rows, cell for cell.
///
/// Read out of the document rather than transcribed twice, and compared in both
/// directions: a fifth row appearing in the specification fails here rather
/// than being folded into the nearest type, and a row this crate writes that
/// the document does not fails against the document.
///
/// **The fourth row's name is `CANCELLED/WITHDRAWN` and the enumeration's is
/// `CANCELLED`.** That divergence is asserted exactly rather than papered over:
/// `t068` section 2.3-4 writes the status as `CANCELLED`, migration `0014`'s
/// `CHECK` admits `CANCELLED`, and `academic_curriculum::OfferingStatus`
/// declares four variants, so the withdrawal half of the row's name is a
/// synonym rather than a fifth status.
#[test]
fn the_four_standings_are_section_8_3s_own() -> TestResult {
    let rows = section_8_3_rows()?;
    assert_eq!(rows.len(), 4, "section 8.3 lists {} rows", rows.len());

    let declared = [
        (
            OfferingStatus::Confirmed,
            ConfirmedStanding::UI_COPY,
            ConfirmedStanding::PLANNER_TREATMENT,
        ),
        (
            OfferingStatus::HistoricallyLikely,
            HistoricallyLikelyStanding::UI_COPY,
            HistoricallyLikelyStanding::PLANNER_TREATMENT,
        ),
        (
            OfferingStatus::Uncertain,
            UncertainStanding::UI_COPY,
            UncertainStanding::PLANNER_TREATMENT,
        ),
        (
            OfferingStatus::Cancelled,
            CancelledStanding::UI_COPY,
            CancelledStanding::PLANNER_TREATMENT,
        ),
    ];
    assert_eq!(declared.len(), OfferingStatus::ALL.len());

    for (index, (status, ui_copy, planner)) in declared.iter().enumerate() {
        let row = rows
            .get(index)
            .ok_or_else(|| format!("section 8.3 has no row {index}"))?;
        assert!(
            row.status.starts_with(status.as_str()),
            "row {} reads {} and this crate writes {}",
            index + 1,
            row.status,
            status.as_str()
        );
        // The one row whose document name is longer than the enumeration's.
        if row.status != status.as_str() {
            assert_eq!(row.status, "CANCELLED/WITHDRAWN");
            assert_eq!(*status, OfferingStatus::Cancelled);
        }
        assert_eq!(row.ui_copy, *ui_copy, "row {}'s UI copy", index + 1);
        assert_eq!(row.planner, *planner, "row {}'s planner cell", index + 1);
        assert!(!row.requirement.is_empty());
        // The likely row's requirement is a conjunction and both halves are
        // implemented: a reproducible pattern, **and** no future official
        // notice. The second is `OfficialTermReading::Announced`, and a row
        // that stopped writing it would fail here.
        if *status == OfferingStatus::HistoricallyLikely {
            assert!(row.requirement.contains("재현 가능한 패턴"));
            assert!(row.requirement.contains("미래 공식 공지 없음"));
        }
    }

    // The other direction: nothing this crate writes is absent from the table.
    let document_copies: BTreeSet<&str> = rows.iter().map(|row| row.ui_copy.as_str()).collect();
    for (_, ui_copy, _) in declared {
        assert!(
            document_copies.contains(ui_copy),
            "{ui_copy} is not a cell section 8.3 writes"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Section 8.3's feature sentence
// ---------------------------------------------------------------------------

/// The feature families are section 8.3's own words.
///
/// **The sentence names six features and this crate implements seven.** That
/// is not a transcription error: `t068` section 5's `P2-U5` entry and `t001`'s
/// `REQ-08-029` row both say *seven feature families* and both resolve the
/// seventh as the history window, which the same sentence requires be recorded
/// as 표본 window. This compares the six in order and in both directions and
/// requires the seventh to be the phrase the sentence writes -- so the
/// divergence is executed rather than described, and a seventh unit appearing
/// in the sentence fails here.
#[test]
fn the_feature_families_are_section_8_3s_own() -> TestResult {
    let specification = specification()?;
    let sentence = specification
        .lines()
        .find(|line| line.starts_with("역사 기반 예측은"))
        .ok_or("section 8.3's feature sentence is not in the document")?;

    let (before, after) = sentence
        .split_once("를 feature로 사용하고")
        .ok_or("the sentence does not name its features")?;
    let listed: Vec<&str> = before
        .split_once("아니다. ")
        .map(|(_, rest)| rest)
        .ok_or("the sentence does not refuse a majority vote")?
        .split(", ")
        .map(str::trim)
        .collect();

    let named: Vec<&str> = FeatureFamily::ALL
        .iter()
        .map(|family| family.spec_phrase())
        .collect();

    assert_eq!(
        listed.len() + 1,
        named.len(),
        "section 8.3 names {} features and this crate declares {}",
        listed.len(),
        named.len()
    );
    for (position, unit) in listed.iter().enumerate() {
        let family = FeatureFamily::ALL
            .get(position)
            .ok_or_else(|| format!("this crate has no family {position}"))?;
        assert_eq!(
            family.spec_phrase(),
            *unit,
            "family {position} is not the unit section 8.3 writes"
        );
    }
    // And the other direction: every phrase this crate writes is in the
    // sentence, including the seventh.
    for family in FeatureFamily::ALL {
        assert!(
            sentence.contains(family.spec_phrase()),
            "{} quotes a phrase section 8.3 does not write: {}",
            family.as_str(),
            family.spec_phrase()
        );
    }
    // The seventh is the one the sentence names as an output rather than as a
    // feature, and it is the one after the split.
    let seventh = FeatureFamily::ALL
        .last()
        .ok_or("this crate declares no families")?;
    assert_eq!(*seventh, FeatureFamily::HistoryWindow);
    assert!(after.contains(seventh.spec_phrase()));
    assert!(!before.contains(seventh.spec_phrase()));

    // The sentence's own refusal is what the seasonal window executes.
    assert!(sentence.contains("단순 다수결이 아니다"));

    // Every family has its own frozen-input key, so two families cannot share
    // one input and read as two.
    let keys: BTreeSet<&str> = FeatureFamily::ALL
        .iter()
        .map(|family| family.input_key())
        .collect();
    assert_eq!(keys.len(), FeatureFamily::ALL.len());
    Ok(())
}

// ---------------------------------------------------------------------------
// Section 8.3's `UNCERTAIN` grounds
// ---------------------------------------------------------------------------

/// The abstention reasons that quote section 8.3 are the `UNCERTAIN` row's own
/// `·`-delimited grounds, in both directions.
///
/// The three that quote nothing are the recorded criteria this repository has
/// no number for. They carry no phrase because the specification writes none,
/// and the test requires them to carry none: a reason that invented a ground
/// would fail against the row.
#[test]
fn the_abstention_reasons_are_section_8_3s_own() -> TestResult {
    let rows = section_8_3_rows()?;
    let uncertain = rows
        .iter()
        .find(|row| row.status == OfferingStatus::Uncertain.as_str())
        .ok_or("section 8.3 has no UNCERTAIN row")?;
    let grounds: Vec<&str> = uncertain.requirement.split('·').map(str::trim).collect();
    assert_eq!(
        grounds.len(),
        3,
        "the UNCERTAIN row names {} grounds",
        grounds.len()
    );

    let quoted: BTreeSet<&str> = AbstentionReason::ALL
        .iter()
        .filter_map(|reason| reason.spec_phrase())
        .collect();
    let written: BTreeSet<&str> = grounds.iter().copied().collect();
    assert_eq!(quoted, written, "the two lists of grounds differ");

    // Which reasons quote nothing, and why: each is a criterion no official
    // source states.
    let unquoted: Vec<&str> = AbstentionReason::ALL
        .iter()
        .filter(|reason| reason.spec_phrase().is_none())
        .map(|reason| reason.as_str())
        .collect();
    assert_eq!(
        unquoted,
        vec![
            "FORECAST_POLICY_ABSENT",
            "NO_FRESH_CALIBRATION_DATASET",
            "BELOW_RECORDED_LIKELY_FLOOR",
            // Not a ground the `UNCERTAIN` row names: it is the
            // `HISTORICALLY_LIKELY` row losing its second conjunct, 미래 공식
            // 공지 없음, which lands the offering here because no listing has
            // been verified.
            "ANNOUNCED_BUT_NOT_VERIFIED"
        ]
    );

    // Every reason has its own spelling.
    let spellings: BTreeSet<&str> = AbstentionReason::ALL
        .iter()
        .map(|reason| reason.as_str())
        .collect();
    assert_eq!(spellings.len(), AbstentionReason::ALL.len());
    Ok(())
}

// ---------------------------------------------------------------------------
// Section 38's open cell
// ---------------------------------------------------------------------------

/// `GATE-38-017` is section 38.2's seventh bullet, and the identifier is
/// derived from that position rather than compared against a list written
/// twice.
///
/// `P2-U3` found that eleven of the eighteen `OpenGate::identifier` arms in
/// this workspace were hand-written strings checked only against a hand-written
/// list in the same test. This crate's one arm is derived from the start.
#[test]
fn the_open_gate_is_section_38s_own() -> TestResult {
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

    // Section 38.1's block holds ten lines, so section 38.2's bullets are
    // numbered from eleven. Both counts are read out of the document.
    let block = specification
        .split_once("Admission Year")
        .map(|(_, rest)| format!("Admission Year{rest}"))
        .and_then(|rest| rest.split_once("```").map(|(block, _)| block.to_owned()))
        .ok_or("section 38.1's block is not in the document")?;
    let lines = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .count();
    assert_eq!(lines, 10, "section 38.1 lists {lines} lines");

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

    let position = 6_usize;
    let bullet = bullets
        .get(position)
        .ok_or_else(|| format!("section 38.2 has no bullet {position}"))?;
    assert_eq!(
        *bullet,
        OpenGate::CurrentTermOfferingFacts.spec_line(),
        "GATE-38-017 is section 38.2's bullet {}",
        position + 1
    );
    assert_eq!(
        OpenGate::CurrentTermOfferingFacts.identifier(),
        format!("GATE-38-{:03}", position + lines + 1),
        "the identifier does not follow the bullet's position"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// What the product source may not reach
// ---------------------------------------------------------------------------

/// No product file reaches a clock, an RNG, a socket, or a model call.
///
/// The forecast is a `(frozen_inputs, rule_set_hash, engine_version)` function,
/// so the instant it compares a calibration dataset against is a caller's
/// value. `ModelRun` is here because the calibration edge is to `P2-M1`'s
/// *registry* -- the crate may be named, an execution record may not.
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
        "ModelRun::",
    ];

    let sources = product_sources()?;
    assert!(sources.len() >= 12, "the walk read {} files", sources.len());
    for (name, text) in &sources {
        let code = stripped(text);
        for spelling in FORBIDDEN {
            assert!(
                !code.contains(spelling),
                "{name} reaches {spelling} on the forecast path"
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
        "let run = ModelRun::record(purpose, provider);",
    ] {
        let code = stripped(evasion);
        assert!(
            FORBIDDEN.iter().any(|spelling| code.contains(spelling)),
            "the rule does not catch {evasion}"
        );
    }
    Ok(())
}

/// No floating point reaches a forecast, a probability, or a metric.
///
/// A Brier score computed in binary floating point would depend on the platform
/// that computed it, which is the opposite of what a calibration record is for.
/// The sweep is over every product file, on stripped code, and it is run
/// against four evasions inside the test so a rule that matched nothing fails.
#[test]
fn no_floating_point_reaches_a_forecast() -> TestResult {
    const FORBIDDEN: [&str; 6] = ["f32", "f64", "as f", "0.0", "1.0", "0.5"];

    let sources = product_sources()?;
    assert!(sources.len() >= 12);
    for (name, text) in &sources {
        let code = stripped(text);
        for spelling in FORBIDDEN {
            assert!(
                !code.contains(spelling),
                "{name} spells {spelling}, which is floating point"
            );
        }
    }
    for evasion in [
        "let rate = positive as f64 / terms as f64;",
        "let brier: f32 = sum / count;",
        "let half = 0.5;",
        "let one = 1.0_f64;",
    ] {
        let code = stripped(evasion);
        assert!(
            FORBIDDEN.iter().any(|spelling| code.contains(spelling)),
            "the rule does not catch {evasion}"
        );
    }
    Ok(())
}

/// Nothing in this crate turns a prediction into a confirmation.
///
/// Two whole sets rather than a token list. The first is every signature that
/// names a prediction-side type: none of them may also name a confirmation-side
/// one, in either position, so a `fn promote(forecast: &Forecast) ->
/// ConfirmationEvidence` fails whatever it is called. The second is every
/// `impl` header in the crate: none may declare a conversion trait between the
/// two sides.
///
/// The one place where both sides are legitimately named is
/// `standing::resolve`, which takes an official reading *and* a forecast and
/// returns a `Resolution` holding both. It is named here so the allowance is a
/// row rather than a hole.
#[test]
fn no_product_file_promotes_a_prediction() -> TestResult {
    const PREDICTION_SIDE: [&str; 5] = [
        "Forecast",
        "ScoredForecast",
        "ForecastVerdict",
        "FeatureVector",
        "CourseHistory",
    ];
    const CONFIRMATION_SIDE: [&str; 3] =
        ["ConfirmationEvidence", "ConfirmedStanding", "ConfirmedSeat"];
    /// The one signature that legitimately names both sides.
    const ALLOWED: [&str; 1] = ["pub fn resolve("];

    let sources = product_sources()?;
    let mut signatures = 0_usize;
    let mut impl_headers = 0_usize;
    for (name, text) in &sources {
        let code = stripped(text);
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub const fn ")
                || trimmed.starts_with("const fn ")
            {
                signatures += 1;
                if ALLOWED.iter().any(|allowed| trimmed.starts_with(allowed)) {
                    continue;
                }
                let predicts = PREDICTION_SIDE.iter().any(|side| trimmed.contains(side));
                let confirms = CONFIRMATION_SIDE.iter().any(|side| trimmed.contains(side));
                assert!(
                    !(predicts && confirms),
                    "{name} names both sides in one signature: {trimmed}"
                );
            }
            if trimmed.starts_with("impl ") {
                impl_headers += 1;
                for conversion in ["From<", "TryFrom<", "Into<", "AsRef<", "Deref"] {
                    if !trimmed.contains(conversion) {
                        continue;
                    }
                    let predicts = PREDICTION_SIDE.iter().any(|side| trimmed.contains(side));
                    let confirms = CONFIRMATION_SIDE.iter().any(|side| trimmed.contains(side));
                    assert!(
                        !(predicts && confirms),
                        "{name} declares a conversion between the two sides: {trimmed}"
                    );
                }
            }
        }
    }
    // Floors: a sweep that stopped finding signatures would satisfy every
    // assertion inside the loop.
    assert!(signatures >= 100, "the sweep read {signatures} signatures");
    assert!(impl_headers >= 12, "the sweep read {impl_headers} impls");

    // And the sweep bites: each line below is a promotion the rule must catch.
    for promotion in [
        "pub fn promote(forecast: &ScoredForecast) -> ConfirmationEvidence {",
        "pub fn upgrade(evidence: ConfirmedStanding, forecast: Forecast) -> ConfirmedSeat {",
        "impl From<ScoredForecast> for ConfirmationEvidence {",
    ] {
        let trimmed = promotion.trim();
        let predicts = PREDICTION_SIDE.iter().any(|side| trimmed.contains(side));
        let confirms = CONFIRMATION_SIDE.iter().any(|side| trimmed.contains(side));
        assert!(predicts && confirms, "the rule does not catch {promotion}");
    }

    // The seat has exactly one construction site in the whole crate, and it is
    // `ConfirmedStanding::seat`. A struct-literal line is one that opens
    // `ConfirmedSeat {` on a type-name boundary and is neither the declaration,
    // the inherent `impl`, nor a return type -- so `NoConfirmedSeat {`, which
    // is `PlanRefusal`'s arm and merely contains the same letters, is not one.
    let mut literal_sites: Vec<String> = Vec::new();
    for (name, text) in &sources {
        for line in stripped(text).lines() {
            if !opens_a_seat_literal(line) {
                continue;
            }
            literal_sites.push(format!("{name}: {}", line.trim()));
        }
    }
    assert_eq!(
        literal_sites.len(),
        1,
        "a ConfirmedSeat is built at {} sites: {literal_sites:?}",
        literal_sites.len()
    );
    assert!(
        literal_sites
            .first()
            .is_some_and(|site| site.starts_with("standing.rs:")),
        "the one construction site is not in standing.rs: {literal_sites:?}"
    );

    // And the rule bites: the three lines below are the three shapes it has to
    // tell apart, and only the first is a construction.
    assert!(opens_a_seat_literal("        ConfirmedSeat {"));
    assert!(!opens_a_seat_literal("pub struct ConfirmedSeat {"));
    assert!(!opens_a_seat_literal("impl ConfirmedSeat {"));
    assert!(!opens_a_seat_literal(
        "    pub fn seat(&self) -> ConfirmedSeat {"
    ));
    assert!(!opens_a_seat_literal(
        "                        None => refusals.push(PlanRefusal::NoConfirmedSeat {"
    ));
    Ok(())
}

/// Whether one line opens a `ConfirmedSeat` struct literal.
fn opens_a_seat_literal(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("pub struct ") || trimmed.starts_with("impl ") || trimmed.contains("-> ")
    {
        return false;
    }
    let Some(index) = trimmed.find("ConfirmedSeat {") else {
        return false;
    };
    // A type name is not part of a longer identifier: `NoConfirmedSeat` is a
    // different type that merely ends in the same letters.
    index == 0
        || !trimmed
            .as_bytes()
            .get(index - 1)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b':')
}

/// Nothing in this crate has a `Default`.
///
/// The three recorded criteria -- the verification bound, the likely floor and
/// the minimum window -- are numbers no official source states, so a `Default`
/// on any of them would be exactly the manufactured verdict `P2-U3` refused for
/// `SourceFreshnessPolicy`. The behavioural test beside this one checks the
/// constructor's range refusals and **would not catch a `Default` at all**:
/// that is why the absence is swept here rather than asserted there.
///
/// The sweep is over the whole `#[derive(...)]` set and over every `impl
/// Default` header, in both directions, with a floor -- so a derive list that
/// shrank to nothing would fail rather than pass over an empty set.
#[test]
fn nothing_in_this_crate_has_a_default() -> TestResult {
    let sources = product_sources()?;
    let mut derives = 0_usize;
    for (name, text) in &sources {
        let code = stripped(text);
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[derive(") {
                derives += 1;
                assert!(
                    !trimmed.contains("Default"),
                    "{name} derives Default: {trimmed}"
                );
            }
            assert!(
                !trimmed.starts_with("impl Default"),
                "{name} hand-writes a Default: {trimmed}"
            );
        }
    }
    assert!(derives >= 20, "the sweep read {derives} derive lines");

    // The rule bites on both shapes.
    for evasion in [
        "#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]",
        "impl Default for ForecastPolicy {",
    ] {
        let trimmed = evasion.trim();
        assert!(
            (trimmed.starts_with("#[derive(") && trimmed.contains("Default"))
                || trimmed.starts_with("impl Default"),
            "the rule does not catch {evasion}"
        );
    }
    Ok(())
}

/// This crate names no store, no migration and no §28 registry entry.
///
/// It persists nothing: migration `0014` already holds
/// `offering_detail.official_status` with the four-value `CHECK` and migration
/// `0001` already holds `prediction_metadata_version`, and both are somebody
/// else's rows. And the §28 table names twelve engines, none of them an
/// offering forecast, so this engine's identifier is deliberately outside the
/// `engine.` namespace the registry uses.
#[test]
fn this_crate_persists_nothing_and_registers_no_engine() -> TestResult {
    let sources = product_sources()?;
    for (name, text) in &sources {
        let code = stripped(text);
        for spelling in ["academic_store", "STORE_MIGRATION_SQL", "rusqlite"] {
            assert!(!code.contains(spelling), "{name} names {spelling}");
        }
    }

    // The engine identifier is not one the §28 registry holds.
    let registry = fs::read_to_string(
        repository_root()
            .join("schemas")
            .join("registry")
            .join("engine-registry-v1.json"),
    )?;
    assert!(
        !registry.contains(academic_offering::OFFERING_FORECAST_ENGINE_ID),
        "the §28 registry names this engine"
    );
    assert!(
        !academic_offering::OFFERING_FORECAST_ENGINE_ID.starts_with("engine."),
        "this engine claims the registry's namespace"
    );
    // And the twelve are still twelve, so the absence above is a measurement
    // rather than an assumption about a file that shrank.
    assert_eq!(registry.matches("\"engine_id\"").count(), 12);

    // Nothing of this crate's sits under the harness root the registry owns.
    let harness_root = repository_root().join("testdata").join("engines");
    let harness_dirs: BTreeSet<String> = fs::read_dir(&harness_root)?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!harness_dirs.contains("offering_forecast"));
    assert!(
        repository_root()
            .join("testdata")
            .join("offering-forecast")
            .join("oracle.expected")
            .exists()
    );
    Ok(())
}

/// The floor the inventory walk must reach, so an empty walk fails as a walk.
const INVENTORY_FILE_FLOOR: usize = 13;

/// Every function this package declares, as `<file> [vis] <signature>`.
const DECLARATIONS: &[&str] = &[
    "src/claims.rs [pub] fn announcement_claim( id: ClaimId, subject: &ClaimSubject, announcement: &OfferingAnnouncement, evidence_ids: Vec<EvidenceId>, ) -> Result<Claim, OfferingError>",
    "src/claims.rs [pub] fn as_str(self) -> &'static str",
    "src/claims.rs [pub] fn as_str(self) -> &'static str",
    "src/claims.rs [pub] fn confirmation_claim( id: ClaimId, subject: &ClaimSubject, evidence: &ConfirmationEvidence, evidence_ids: Vec<EvidenceId>, ) -> Result<Claim, OfferingError>",
    "src/claims.rs [pub] fn forecast_claim( id: ClaimId, subject: &ClaimSubject, scored: &ScoredForecast, evidence_ids: Vec<EvidenceId>, ) -> Result<Claim, OfferingError>",
    "src/claims.rs [pub] fn official(official: Claim) -> Self",
    "src/claims.rs [pub] fn official_arrived(self, official: Claim) -> Self",
    "src/claims.rs [pub] fn official_claim(&self) -> Option<&Claim>",
    "src/claims.rs [pub] fn official_standing(&self) -> Option<DecisionStanding>",
    "src/claims.rs [pub] fn predicted(prediction: Claim) -> Self",
    "src/claims.rs [pub] fn prediction(&self) -> Option<&Claim>",
    "src/claims.rs [pub] fn prediction_standing(&self) -> Option<DecisionStanding>",
    "src/corpus.rs [priv] fn code(value: &str) -> Result<CourseCode, OfferingError>",
    "src/corpus.rs [priv] fn every_other_spring() -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn every_other_spring_coded(course: &str) -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn every_spring(course: &str) -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn gap_two() -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn instructor_volatile() -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn irregular_only() -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn never_observed() -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn read_at(index: i64) -> TimestampMillis",
    "src/corpus.rs [priv] fn retired() -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn sparse() -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn suspended_notice() -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [priv] fn teacher(value: &str) -> Result<InstructorName, OfferingError>",
    "src/corpus.rs [pub] fn calibration_dataset(refreshed_at: u64) -> Result<CalibrationDataset, OfferingError>",
    "src/corpus.rs [pub] fn calibration_registry(refreshed_at: u64) -> Result<CalibrationRegistry, OfferingError>",
    "src/corpus.rs [pub] fn history(case: &str) -> Result<CourseHistory, OfferingError>",
    "src/corpus.rs [pub] fn policy() -> Result<ForecastPolicy, OfferingError>",
    "src/corpus.rs [pub] fn realized(case: &str) -> Option<crate::metrics::RealizedOutcome>",
    "src/corpus.rs [pub] fn verification_recency() -> Result<VerificationRecency, OfferingError>",
    "src/corpus.rs [pub] fn window(case: &str) -> Result<ObservationWindow, OfferingError>",
    "src/corpus.rs [pub] fn window_start() -> Result<TermKey, OfferingError>",
    "src/error.rs [priv] fn from(error: RecordError) -> Self",
    "src/feature.rs [priv] fn history_window(seasonal_terms: u32) -> FeatureSignal",
    "src/feature.rs [priv] fn instructor_change(seasonal: &[&crate::observation::TermObservation]) -> FeatureSignal",
    "src/feature.rs [priv] fn irregular_special(seasonal: &[&crate::observation::TermObservation]) -> FeatureSignal",
    "src/feature.rs [priv] fn lifecycle_status(lifecycle: &CourseLifecycle, target: TermKey) -> FeatureSignal",
    "src/feature.rs [priv] fn offering_gap(seasonal: &[&crate::observation::TermObservation]) -> FeatureSignal",
    "src/feature.rs [priv] fn recent_notices(history: &CourseHistory, window: ObservationWindow) -> FeatureSignal",
    "src/feature.rs [priv] fn seasonality(seasonal_terms: u32, positive_samples: u32) -> FeatureSignal",
    "src/feature.rs [priv] fn signal(family: FeatureFamily, value: i64, contribution: i32) -> FeatureSignal",
    "src/feature.rs [pub] fn as_str(self) -> &'static str",
    "src/feature.rs [pub] fn contains(self, term: TermKey) -> bool",
    "src/feature.rs [pub] fn contribution(self) -> i32",
    "src/feature.rs [pub] fn extract(history: &CourseHistory, window: ObservationWindow) -> Self",
    "src/feature.rs [pub] fn family(self) -> FeatureFamily",
    "src/feature.rs [pub] fn from(self) -> TermKey",
    "src/feature.rs [pub] fn input_key(self) -> &'static str",
    "src/feature.rs [pub] fn new(from: TermKey, to: TermKey) -> Result<Self, OfferingError>",
    "src/feature.rs [pub] fn positive_samples(&self) -> u32",
    "src/feature.rs [pub] fn raw_units(&self) -> u32",
    "src/feature.rs [pub] fn seasonal_terms( self, history: &CourseHistory, ) -> Vec<&crate::observation::TermObservation>",
    "src/feature.rs [pub] fn seasonal_terms(&self) -> u32",
    "src/feature.rs [pub] fn semester(self) -> Semester",
    "src/feature.rs [pub] fn signal(&self, family: FeatureFamily) -> FeatureSignal",
    "src/feature.rs [pub] fn signals(&self) -> &[FeatureSignal; FeatureFamily::ALL.len()]",
    "src/feature.rs [pub] fn spec_phrase(self) -> &'static str",
    "src/feature.rs [pub] fn to(self) -> TermKey",
    "src/feature.rs [pub] fn value(self) -> i64",
    "src/feature.rs [pub] fn window(&self) -> ObservationWindow",
    "src/forecast.rs [priv] fn decide( history: &CourseHistory, window: ObservationWindow, features: &FeatureVector, raw_units: u32, policy: ForecastPolicy, registry: &CalibrationRegistry, now: TimestampMillis, ) -> Result<ForecastVerdict, OfferingError>",
    "src/forecast.rs [priv] fn disclosed_window( seasonal: &[&crate::observation::TermObservation], ) -> Result<PredictionObservationWindow, OfferingError>",
    "src/forecast.rs [priv] fn engine_result( features: &FeatureVector, raw_units: u32, verdict: &ForecastVerdict, root: ProofStatus, ) -> Result<EngineResult, OfferingError>",
    "src/forecast.rs [priv] fn family_status(contribution: i32, has_evidence: bool) -> ProofStatus",
    "src/forecast.rs [priv] fn frozen_inputs( history: &CourseHistory, window: ObservationWindow, features: &FeatureVector, raw_units: u32, policy: ForecastPolicy, ) -> Result<FrozenInputs, OfferingError>",
    "src/forecast.rs [priv] fn key(name: &str) -> Result<InputKey, OfferingError>",
    "src/forecast.rs [priv] fn lower(value: &str) -> String",
    "src/forecast.rs [priv] fn node_id(name: &str) -> Result<NodeId, OfferingError>",
    "src/forecast.rs [priv] fn proof_tree( features: &FeatureVector, verdict: &ForecastVerdict, policy: ForecastPolicy, root: ProofStatus, ) -> Result<ProofNode, OfferingError>",
    "src/forecast.rs [priv] fn root_status(verdict: &ForecastVerdict, policy: ForecastPolicy) -> ProofStatus",
    "src/forecast.rs [priv] fn rule_id(name: &str) -> Result<RuleId, OfferingError>",
    "src/forecast.rs [priv] fn whole(value: i128) -> Result<Decimal, OfferingError>",
    "src/forecast.rs [pub] fn as_str(self) -> &'static str",
    "src/forecast.rs [pub] fn calibrated(&self) -> &CalibratedConfidence",
    "src/forecast.rs [pub] fn canonical_bytes(&self) -> Result<Vec<u8>, OfferingError>",
    "src/forecast.rs [pub] fn confidence(&self) -> ConfidencePermille",
    "src/forecast.rs [pub] fn course(&self) -> &CourseCode",
    "src/forecast.rs [pub] fn features(&self) -> &FeatureVector",
    "src/forecast.rs [pub] fn forecast( history: &CourseHistory, window: ObservationWindow, policy: ForecastPolicy, registry: &CalibrationRegistry, now: TimestampMillis, ) -> Result<Forecast, OfferingError>",
    "src/forecast.rs [pub] fn inputs(&self) -> &FrozenInputs",
    "src/forecast.rs [pub] fn metadata(&self) -> PredictionMetadata",
    "src/forecast.rs [pub] fn outcome(&self) -> &EngineOutcome",
    "src/forecast.rs [pub] fn raw_units(&self) -> u32",
    "src/forecast.rs [pub] fn rule_set_hash() -> RuleSetHash",
    "src/forecast.rs [pub] fn spec_phrase(self) -> Option<&'static str>",
    "src/forecast.rs [pub] fn target_term(&self) -> TermKey",
    "src/forecast.rs [pub] fn verdict(&self) -> &ForecastVerdict",
    "src/gate.rs [pub] fn identifier(self) -> &'static str",
    "src/gate.rs [pub] fn spec_line(self) -> &'static str",
    "src/gate.rs [pub] fn statement(self) -> &'static str",
    "src/metrics.rs [priv] fn ratio_permille(part: usize, whole: usize) -> u32",
    "src/metrics.rs [pub] fn abstained(&self) -> Option<AbstentionReason>",
    "src/metrics.rs [pub] fn abstained(&self) -> usize",
    "src/metrics.rs [pub] fn abstention_permille(&self) -> u32",
    "src/metrics.rs [pub] fn as_str(self) -> &'static str",
    "src/metrics.rs [pub] fn brier_denominator(&self) -> Option<usize>",
    "src/metrics.rs [pub] fn brier_numerator(&self) -> Option<u64>",
    "src/metrics.rs [pub] fn brier_per_million_floor(&self) -> Option<u64>",
    "src/metrics.rs [pub] fn course(&self) -> &CourseCode",
    "src/metrics.rs [pub] fn coverage_permille(&self) -> u32",
    "src/metrics.rs [pub] fn entries(&self) -> &[EvaluationEntry]",
    "src/metrics.rs [pub] fn from_forecast(forecast: &Forecast, realized: Option<RealizedOutcome>) -> Self",
    "src/metrics.rs [pub] fn measure(&self) -> TermForecastMetrics",
    "src/metrics.rs [pub] fn missing_outcomes(&self) -> &[CourseCode]",
    "src/metrics.rs [pub] fn new(term: TermKey, entries: Vec<EvaluationEntry>) -> Result<Self, OfferingError>",
    "src/metrics.rs [pub] fn permille(self) -> i64",
    "src/metrics.rs [pub] fn realized(&self) -> Option<RealizedOutcome>",
    "src/metrics.rs [pub] fn resolved(&self) -> usize",
    "src/metrics.rs [pub] fn scored(&self) -> usize",
    "src/metrics.rs [pub] fn scored_permille(&self) -> Option<u16>",
    "src/metrics.rs [pub] fn term(&self) -> TermKey",
    "src/metrics.rs [pub] fn term(&self) -> TermKey",
    "src/metrics.rs [pub] fn total(&self) -> usize",
    "src/observation.rs [pub] fn as_str(&self) -> &'static str",
    "src/observation.rs [pub] fn as_str(self) -> &'static str",
    "src/observation.rs [pub] fn as_str(self) -> &'static str",
    "src/observation.rs [pub] fn course(&self) -> &CourseCode",
    "src/observation.rs [pub] fn effect(&self) -> NoticeEffect",
    "src/observation.rs [pub] fn effective_from(&self) -> Option<TermKey>",
    "src/observation.rs [pub] fn instructors(&self) -> &[InstructorName]",
    "src/observation.rs [pub] fn is_irregular(&self) -> bool",
    "src/observation.rs [pub] fn issued_in(&self) -> TermKey",
    "src/observation.rs [pub] fn lifecycle(&self) -> &CourseLifecycle",
    "src/observation.rs [pub] fn new(course: CourseCode) -> Self",
    "src/observation.rs [pub] fn new(issued_in: TermKey, effect: NoticeEffect) -> Self",
    "src/observation.rs [pub] fn not_offered(term: TermKey, read_at: TimestampMillis) -> Self",
    "src/observation.rs [pub] fn notice(&mut self, notice: RecentNotice)",
    "src/observation.rs [pub] fn notices(&self) -> &[RecentNotice]",
    "src/observation.rs [pub] fn observation(&self, term: TermKey) -> Option<&TermObservation>",
    "src/observation.rs [pub] fn observations(&self) -> impl Iterator<Item = &TermObservation>",
    "src/observation.rs [pub] fn observe(&mut self, observation: TermObservation) -> Result<(), OfferingError>",
    "src/observation.rs [pub] fn offered( term: TermKey, read_at: TimestampMillis, instructors: Vec<InstructorName>, irregular: bool, ) -> Self",
    "src/observation.rs [pub] fn outcome(&self) -> Offered",
    "src/observation.rs [pub] fn read_at(&self) -> TimestampMillis",
    "src/observation.rs [pub] fn set_lifecycle(&mut self, lifecycle: CourseLifecycle)",
    "src/observation.rs [pub] fn term(&self) -> TermKey",
    "src/plan.rs [pub] fn action(&self) -> &'static str",
    "src/plan.rs [pub] fn alternative_paths_required(&self) -> Vec<&str>",
    "src/plan.rs [pub] fn as_str(&self) -> &'static str",
    "src/plan.rs [pub] fn commit(scenario: &PlanScenario, seats: Vec<ConfirmedSeat>) -> PlanOutcome",
    "src/plan.rs [pub] fn course(&self) -> &str",
    "src/plan.rs [pub] fn is_empty(&self) -> bool",
    "src/plan.rs [pub] fn len(&self) -> usize",
    "src/plan.rs [pub] fn new(first: PlanRefusal, rest: Vec<PlanRefusal>) -> Self",
    "src/plan.rs [pub] fn refusals(&self) -> &[PlanRefusal]",
    "src/plan.rs [pub] fn requires_alternative_path(&self) -> bool",
    "src/plan.rs [pub] fn seats(&self) -> &[ConfirmedSeat]",
    "src/policy.rs [pub] fn likely_floor_permille(self) -> u16",
    "src/policy.rs [pub] fn minimum_window_terms(self) -> u32",
    "src/policy.rs [pub] fn new( likely_floor_permille: u16, minimum_window_terms: u32, ) -> Result<Self, OfferingError>",
    "src/policy.rs [pub] fn new(within_millis: u64) -> Result<Self, OfferingError>",
    "src/policy.rs [pub] fn within_millis(self) -> u64",
    "src/source.rs [pub] fn announced_capacity(&self) -> Option<Capacity>",
    "src/source.rs [pub] fn basis(&self) -> &OfficialListing",
    "src/source.rs [pub] fn capacity(mut self, capacity: Capacity) -> Self",
    "src/source.rs [pub] fn connector(&self) -> &ConnectorId",
    "src/source.rs [pub] fn connector(&self) -> &ConnectorId",
    "src/source.rs [pub] fn connector(&self) -> &ConnectorId",
    "src/source.rs [pub] fn connector(&self) -> &ConnectorId",
    "src/source.rs [pub] fn course(&self) -> &CourseCode",
    "src/source.rs [pub] fn course(&self) -> &CourseCode",
    "src/source.rs [pub] fn course(&self) -> &CourseCode",
    "src/source.rs [pub] fn cross_sources(&self) -> &[OfficialListing]",
    "src/source.rs [pub] fn disagreements(&self) -> &[CrossSourceDisagreement]",
    "src/source.rs [pub] fn from_registration_system( basis: OfficialListing, cross_sources: Vec<OfficialListing>, recency: VerificationRecency, verified_at: TimestampMillis, ) -> Result<Self, OfferingError>",
    "src/source.rs [pub] fn instructor(mut self, name: InstructorName) -> Self",
    "src/source.rs [pub] fn instructors(&self) -> &[InstructorName]",
    "src/source.rs [pub] fn issued_at(&self) -> TimestampMillis",
    "src/source.rs [pub] fn issued_at(&self) -> TimestampMillis",
    "src/source.rs [pub] fn lists_a_section(&self) -> bool",
    "src/source.rs [pub] fn meeting(mut self, meeting: Meeting) -> Self",
    "src/source.rs [pub] fn meetings(&self) -> &[Meeting]",
    "src/source.rs [pub] fn new( source: SourceCategory, connector: ConnectorId, retrieved_at: TimestampMillis, term: TermKey, course: CourseCode, lists_a_section: bool, ) -> Self",
    "src/source.rs [pub] fn official( source: SourceCategory, connector: ConnectorId, issued_at: TimestampMillis, term: TermKey, course: CourseCode, ) -> Result<Self, OfferingError>",
    "src/source.rs [pub] fn official( source: SourceCategory, connector: ConnectorId, issued_at: TimestampMillis, term: TermKey, course: CourseCode, ) -> Result<Self, OfferingError>",
    "src/source.rs [pub] fn recency(&self) -> VerificationRecency",
    "src/source.rs [pub] fn retrieved_at(&self) -> TimestampMillis",
    "src/source.rs [pub] fn retrieved_at(&self) -> TimestampMillis",
    "src/source.rs [pub] fn said_a_section_exists(&self) -> bool",
    "src/source.rs [pub] fn source(&self) -> SourceCategory",
    "src/source.rs [pub] fn source(&self) -> SourceCategory",
    "src/source.rs [pub] fn source(&self) -> SourceCategory",
    "src/source.rs [pub] fn source(&self) -> SourceCategory",
    "src/source.rs [pub] fn term(&self) -> TermKey",
    "src/source.rs [pub] fn term(&self) -> TermKey",
    "src/source.rs [pub] fn term(&self) -> TermKey",
    "src/source.rs [pub] fn verified_at(&self) -> TimestampMillis",
    "src/standing.rs [pub] fn announced(&self) -> Option<&AnnouncedStanding>",
    "src/standing.rs [pub] fn announcement(&self) -> &OfferingAnnouncement",
    "src/standing.rs [pub] fn calibrated(&self) -> &CalibratedConfidence",
    "src/standing.rs [pub] fn capacity(&self) -> Option<Capacity>",
    "src/standing.rs [pub] fn course(&self) -> &CourseCode",
    "src/standing.rs [pub] fn displayed(&self) -> DisplayedConfidence",
    "src/standing.rs [pub] fn evidence(&self) -> &ConfirmationEvidence",
    "src/standing.rs [pub] fn forecast(&self) -> Option<&Forecast>",
    "src/standing.rs [pub] fn meetings(&self) -> &[Meeting]",
    "src/standing.rs [pub] fn notice(&self) -> &CancellationNotice",
    "src/standing.rs [pub] fn planner_treatment(&self) -> &'static str",
    "src/standing.rs [pub] fn reason(&self) -> AbstentionReason",
    "src/standing.rs [pub] fn resolve( history: &CourseHistory, window: ObservationWindow, official: Option<&OfficialTermReading>, policy: Option<ForecastPolicy>, registry: &CalibrationRegistry, now: TimestampMillis, ) -> Result<Resolution, OfferingError>",
    "src/standing.rs [pub] fn scored(&self) -> &ScoredForecast",
    "src/standing.rs [pub] fn scored(&self) -> Option<&ScoredForecast>",
    "src/standing.rs [pub] fn seat(&self) -> ConfirmedSeat",
    "src/standing.rs [pub] fn seat(&self) -> Option<ConfirmedSeat>",
    "src/standing.rs [pub] fn standing(&self) -> &OfferingStanding",
    "src/standing.rs [pub] fn status(&self) -> OfferingStatus",
    "src/standing.rs [pub] fn term(&self) -> TermKey",
    "src/standing.rs [pub] fn ui_copy(&self) -> &'static str",
    "src/standing.rs [pub] fn verified_at(&self) -> TimestampMillis",
    "src/standing.rs [pub] fn verified_at(&self) -> TimestampMillis",
];

/// Every `impl` block header this package ships, as `<file>: <header>`.
const IMPL_HEADERS: &[&str] = &[
    "src/claims.rs: impl DecisionStanding",
    "src/claims.rs: impl OfferingAssertion",
    "src/claims.rs: impl OfferingClaimSet",
    "src/error.rs: impl From<RecordError> for OfferingError",
    "src/feature.rs: impl FeatureFamily",
    "src/feature.rs: impl FeatureSignal",
    "src/feature.rs: impl FeatureVector",
    "src/feature.rs: impl ObservationWindow",
    "src/forecast.rs: impl AbstentionReason",
    "src/forecast.rs: impl Forecast",
    "src/forecast.rs: impl ScoredForecast",
    "src/gate.rs: impl OpenGate",
    "src/metrics.rs: impl EvaluationEntry",
    "src/metrics.rs: impl RealizedOutcome",
    "src/metrics.rs: impl TermEvaluation",
    "src/metrics.rs: impl TermForecastMetrics",
    "src/observation.rs: impl CourseHistory",
    "src/observation.rs: impl CourseLifecycle",
    "src/observation.rs: impl Iterator<Item = &TermObservation>",
    "src/observation.rs: impl NoticeEffect",
    "src/observation.rs: impl Offered",
    "src/observation.rs: impl RecentNotice",
    "src/observation.rs: impl TermObservation",
    "src/plan.rs: impl DeterminatePlan",
    "src/plan.rs: impl IndeterminatePlan",
    "src/plan.rs: impl PlanRefusal",
    "src/policy.rs: impl ForecastPolicy",
    "src/policy.rs: impl VerificationRecency",
    "src/source.rs: impl CancellationNotice",
    "src/source.rs: impl ConfirmationEvidence",
    "src/source.rs: impl CrossSourceDisagreement",
    "src/source.rs: impl OfferingAnnouncement",
    "src/source.rs: impl OfficialListing",
    "src/standing.rs: impl AnnouncedStanding",
    "src/standing.rs: impl CancelledStanding",
    "src/standing.rs: impl ConfirmedSeat",
    "src/standing.rs: impl ConfirmedStanding",
    "src/standing.rs: impl HistoricallyLikelyStanding",
    "src/standing.rs: impl OfferingStanding",
    "src/standing.rs: impl Resolution",
    "src/standing.rs: impl UncertainStanding",
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
// each_confirmation_value_has_one_producer
// ---------------------------------------------------------------------------

/// Every expression in this crate that builds a confirmation value.
///
/// One entry per site, as `<type> <file>: <line>`. Three types, three sites.
const PRODUCERS: &[&str] = &[
    "ConfirmedSeat standing.rs x1",
    "ConfirmedStanding standing.rs x1",
    "DeterminatePlan plan.rs x1",
];

/// The `impl` blocks in `code`, as a self type and the block's body.
///
/// The self type is the last path segment before the opening brace, so
/// `impl ConfirmedSeat`, `impl From<&Resolution> for Option<ConfirmedSeat>` and
/// `impl core::fmt::Debug for ConfirmedSeat` all report a self type -- which is
/// what makes a `Self { .. }` inside any of them a construction of that type.
fn impl_blocks(code: &str) -> Vec<(String, String)> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("impl") {
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        if code[at + 4..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let Some(open) = code[at..].find('{').map(|offset| at + offset) else {
            continue;
        };
        let header = &code[at + 4..open];
        let subject = header.rsplit(" for ").next().unwrap_or(header);
        let name: String = subject
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
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

/// Each confirmation value is built in exactly one place, `Self` included.
///
/// `no_product_file_promotes_a_prediction` counts `ConfirmedSeat {` on a
/// type-name boundary. `P2-A3` walked past it with twenty-five lines that spell
/// the literal `Self { .. }` inside `impl ConfirmedSeat` and then convert a
/// forecast into one through `impl From<&Resolution> for Option<ConfirmedSeat>`
/// -- naming only the confirmation side in the first and neither side in the
/// second. The offering suite stayed green at 33 passed, and a
/// `HISTORICALLY_LIKELY` offering yielded a seat that entered a
/// `DeterminatePlan`.
///
/// So the producer set is counted rather than the spelling: a `Self {` inside
/// **any** `impl` block whose self type is one of the three, plus every
/// type-named literal, and the whole set compared against the pin in both
/// directions. `Self` cannot be renamed and an `impl` block cannot hide its
/// self type, so there is no spelling of that injection this does not see.
#[test]
fn each_confirmation_value_has_one_producer() -> TestResult {
    const CONFIRMATION_VALUES: [&str; 3] =
        ["ConfirmedSeat", "ConfirmedStanding", "DeterminatePlan"];

    let sources = product_sources()?;
    let mut sites: Vec<String> = Vec::new();
    let mut blocks_read = 0_usize;
    for (name, text) in &sources {
        let code = stripped(text);
        // A literal that names the type.
        for line in code.lines() {
            for value in CONFIRMATION_VALUES {
                if opens_a_literal(line, value) {
                    sites.push(format!("{value} {name}"));
                }
            }
        }
        // A literal inside that type's own `impl`, spelled `Self`.
        for (subject, body) in impl_blocks(&code) {
            blocks_read += 1;
            let Some(value) = CONFIRMATION_VALUES.iter().find(|value| subject == **value) else {
                continue;
            };
            for _ in body.match_indices("Self {") {
                sites.push(format!("{value} {name}"));
            }
        }
    }
    assert!(
        blocks_read >= 12,
        "the producer sweep read {blocks_read} impl blocks"
    );
    // One entry per site, counted by type and file rather than by line text, so
    // the pin does not move when a line wraps -- and a second site in a file
    // that already has one moves the count rather than hiding behind it.
    let mut counted: BTreeMap<String, usize> = BTreeMap::new();
    for site in sites {
        *counted.entry(site).or_default() += 1;
    }
    let rendered: Vec<String> = counted
        .iter()
        .map(|(site, count)| format!("{site} x{count}"))
        .collect();
    assert_eq!(
        rendered,
        PRODUCERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "the set of expressions that build a confirmation value changed"
    );
    Ok(())
}

/// Whether `line` opens a struct literal naming `value`.
fn opens_a_literal(line: &str, value: &str) -> bool {
    let trimmed = line.trim();
    let Some(at) = trimmed.find(&format!("{value} {{")) else {
        return false;
    };
    if at > 0 {
        let before = trimmed.as_bytes()[at - 1];
        if before.is_ascii_alphanumeric() || before == b'_' {
            return false;
        }
    }
    let prefix = trimmed[..at].trim_end();
    !(prefix.ends_with("struct")
        || prefix.ends_with("impl")
        || prefix.ends_with("enum")
        || prefix.ends_with("for")
        || prefix.ends_with("->"))
}
