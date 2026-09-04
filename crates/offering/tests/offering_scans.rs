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
    collections::BTreeSet,
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
